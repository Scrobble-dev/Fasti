import {
  B1_CONFORMANCE_OPERATIONS,
  FastiContractParseError,
  parseAcceptObservationRequest,
  parseAcceptObservationResponse,
  parseCapabilityDiscoveryResponse,
  parseEnrollFirstClientRequest,
  parseEnrollFirstClientResponse,
  parseHealthResponse,
  parseInitializeNodeRequest,
  parseInitializeNodeResponse,
  parseProblemDetails,
  parseReceiptCommittedEvent,
  parseReplayReceiptResponse,
  RECEIPT_STREAM_CONTRACT,
  type AcceptObservationRequest,
  type AcceptObservationResponse,
  type CapabilityDiscoveryResponse,
  type EnrollFirstClientRequest,
  type EnrollFirstClientResponse,
  type HealthResponse,
  type InitializeNodeRequest,
  type InitializeNodeResponse,
  type ProblemDetails,
  type ReceiptCommittedEnvelope,
  type ReplayReceiptResponse,
} from "./generated.js";

export * from "./generated.js";

export interface RetryPolicy {
  /** Total attempts, including the initial request. */
  readonly maxAttempts: number;
  readonly baseDelayMs: number;
  readonly maxDelayMs: number;
  readonly transientStatusCodes: readonly number[];
  readonly retryNetworkErrors: boolean;
}

export const DEFAULT_RETRY_POLICY: RetryPolicy = Object.freeze({
  maxAttempts: 3,
  baseDelayMs: 100,
  maxDelayMs: 1_000,
  transientStatusCodes: Object.freeze([408, 425, 429, 500, 502, 503, 504]),
  retryNetworkErrors: true,
});

export type CredentialProvider = string | (() => string | Promise<string>);

export interface FastiClientOptions {
  readonly baseUrl: string;
  readonly credential?: CredentialProvider;
  readonly timeoutMs?: number;
  readonly retryPolicy?: Partial<RetryPolicy>;
  readonly fetch?: typeof globalThis.fetch;
}

export interface CallOptions {
  readonly signal?: AbortSignal;
  readonly timeoutMs?: number;
  readonly retryPolicy?: Partial<RetryPolicy>;
}

export interface ReceiptStreamOptions extends CallOptions {
  /** Last successfully handled SSE cursor. It is sent only as a header. */
  readonly cursor?: string;
}

export class FastiProblemError extends Error {
  readonly problem: ProblemDetails;

  constructor(problem: ProblemDetails) {
    super(problem.detail);
    this.name = "FastiProblemError";
    this.problem = problem;
  }
}

export class FastiTimeoutError extends Error {
  constructor() {
    super("Fasti request timed out");
    this.name = "FastiTimeoutError";
  }
}

export class FastiAbortError extends Error {
  constructor() {
    super("Fasti request was cancelled");
    this.name = "FastiAbortError";
  }
}

export class FastiTransportError extends Error {
  readonly status?: number;

  constructor(message: string, status?: number) {
    super(message);
    this.name = "FastiTransportError";
    this.status = status;
  }
}

export class FastiProtocolError extends Error {
  constructor(message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = "FastiProtocolError";
  }
}

interface AbortScope {
  readonly signal: AbortSignal;
  readonly timedOut: () => boolean;
  readonly dispose: () => void;
}

interface ParsedSseEvent {
  readonly id: string;
  readonly event: string;
  readonly data: string;
}

type JsonParser<T> = (value: unknown) => T;
type RetryMode = "safe" | "never" | "stable-idempotency";

class NetworkRequestError extends Error {}
class ResponseReadError extends Error {}
class SseNetworkReadError extends Error {}
class CredentialProviderError extends Error {}
class ScopedAbortError extends Error {}

const DEFAULT_TIMEOUT_MS = 10_000;
const MAX_RETRY_ATTEMPTS = 10;
const MAX_RETRY_DELAY_MS = 60_000;
const MAX_JSON_RESPONSE_BYTES = 512 * 1_024;
const MAX_SSE_LINE_BYTES = 64 * 1_024;
const MAX_SSE_EVENT_BYTES = 256 * 1_024;
const MAX_SSE_EVENT_LINES = 256;
const MAX_SSE_CURSOR_CHARACTERS = 512;
const RECEIPT_ID = /^rcp_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$/;

export class FastiClient {
  readonly #baseUrl: URL;
  readonly #credential?: CredentialProvider;
  readonly #timeoutMs: number;
  readonly #retryPolicy: RetryPolicy;
  readonly #fetch: typeof globalThis.fetch;

  constructor(options: FastiClientOptions) {
    this.#baseUrl = normalizeBaseUrl(options.baseUrl);
    this.#credential = options.credential;
    this.#timeoutMs = positiveInteger(
      options.timeoutMs ?? DEFAULT_TIMEOUT_MS,
      "timeoutMs",
    );
    this.#retryPolicy = normalizeRetryPolicy(options.retryPolicy);
    this.#fetch = options.fetch ?? globalThis.fetch;
    if (typeof this.#fetch !== "function") {
      throw new TypeError("A Fetch API implementation is required");
    }
  }

  health(options: CallOptions = {}): Promise<HealthResponse> {
    return this.#jsonOperation({
      method: "GET",
      path: "/api/v1/health",
      authenticated: false,
      retryMode: "safe",
      responseParser: parseHealthResponse,
      responseLabel: "Health response",
      options,
    });
  }

  discoverCapabilities(
    options: CallOptions = {},
  ): Promise<CapabilityDiscoveryResponse> {
    const operation = B1_CONFORMANCE_OPERATIONS.discoverCapabilities;
    return this.#jsonOperation({
      method: operation.method,
      path: operation.path,
      authenticated: operation.authenticated,
      retryMode: "safe",
      responseParser: parseCapabilityDiscoveryResponse,
      responseLabel: "Capability discovery response",
      options,
    });
  }

  /** Finalized B1 binding whose successful semantics remain unavailable until B2. */
  selectProfile(options: CallOptions = {}): Promise<never> {
    return this.#problemOnlyOperation(
      B1_CONFORMANCE_OPERATIONS.selectProfile,
      "Profile-selection binding",
      options,
    );
  }

  /** Finalized B1 binding whose successful semantics remain unavailable until B2. */
  rotateCredential(options: CallOptions = {}): Promise<never> {
    return this.#problemOnlyOperation(
      B1_CONFORMANCE_OPERATIONS.rotateCredential,
      "Credential-rotation binding",
      options,
    );
  }

  /** Finalized B1 binding whose successful semantics remain unavailable until B2. */
  revokeCredential(options: CallOptions = {}): Promise<never> {
    return this.#problemOnlyOperation(
      B1_CONFORMANCE_OPERATIONS.revokeCredential,
      "Credential-revocation binding",
      options,
    );
  }

  /** Finalized B1 binding whose successful semantics remain unavailable until B2. */
  configureListener(options: CallOptions = {}): Promise<never> {
    return this.#problemOnlyOperation(
      B1_CONFORMANCE_OPERATIONS.configureListener,
      "Listener-configuration binding",
      options,
    );
  }

  initializeNode(
    request: InitializeNodeRequest = {},
    options: CallOptions = {},
  ): Promise<InitializeNodeResponse> {
    const operation = B1_CONFORMANCE_OPERATIONS.initializeNode;
    const body = parseOutgoing(
      parseInitializeNodeRequest,
      request,
      "Initialize-node request",
    );
    return this.#jsonOperation({
      method: operation.method,
      path: operation.path,
      authenticated: operation.authenticated,
      retryMode: "never",
      body,
      responseParser: parseInitializeNodeResponse,
      responseLabel: "Initialize-node response",
      options,
    });
  }

  enrollFirstClient(
    request: EnrollFirstClientRequest,
    options: CallOptions = {},
  ): Promise<EnrollFirstClientResponse> {
    const operation = B1_CONFORMANCE_OPERATIONS.enrollFirstClient;
    const body = parseOutgoing(
      parseEnrollFirstClientRequest,
      request,
      "First-client enrollment request",
    );
    return this.#jsonOperation({
      method: operation.method,
      path: operation.path,
      authenticated: operation.authenticated,
      retryMode: "never",
      body,
      responseParser: parseEnrollFirstClientResponse,
      responseLabel: "First-client enrollment response",
      options,
    });
  }

  acceptObservation(
    request: AcceptObservationRequest,
    options: CallOptions = {},
  ): Promise<AcceptObservationResponse> {
    const operation = B1_CONFORMANCE_OPERATIONS.acceptObservation;
    const body = parseOutgoing(
      parseAcceptObservationRequest,
      request,
      "Accept-observation request",
    );
    return this.#jsonOperation({
      method: operation.method,
      path: operation.path,
      authenticated: operation.authenticated,
      retryMode: "stable-idempotency",
      body,
      responseParser: parseAcceptObservationResponse,
      responseLabel: "Accept-observation response",
      options,
    });
  }

  replayReceipt(
    receiptId: string,
    options: CallOptions = {},
  ): Promise<ReplayReceiptResponse> {
    const operation = B1_CONFORMANCE_OPERATIONS.replayReceipt;
    const safeReceiptId = contractPathIdentifier(
      receiptId,
      RECEIPT_ID,
      "receiptId",
    );
    return this.#jsonOperation({
      method: operation.method,
      path: operation.path.replace(
        "{receipt_id}",
        encodeURIComponent(safeReceiptId),
      ),
      authenticated: operation.authenticated,
      retryMode: "safe",
      responseParser: parseReplayReceiptResponse,
      responseLabel: "Receipt replay response",
      options,
    });
  }

  /**
   * Opens the governed receipt SSE fixture and performs bounded reconnects.
   * This never persists or queues events, and only reconnects the safe stream.
   */
  async *receiptEvents(
    options: ReceiptStreamOptions = {},
  ): AsyncGenerator<ReceiptCommittedEnvelope, void, void> {
    let cursor = validateCursor(options.cursor);
    const retryPolicy = normalizeRetryPolicy(
      options.retryPolicy,
      this.#retryPolicy,
    );

    for (let attempt = 1; attempt <= retryPolicy.maxAttempts; attempt += 1) {
      const scope = createAbortScope(
        options.signal,
        options.timeoutMs ?? this.#timeoutMs,
      );
      try {
        const headers = await this.#headers(
          "text/event-stream",
          true,
          scope.signal,
        );
        if (cursor !== undefined) headers.set("Last-Event-ID", cursor);

        let response: Response;
        try {
          response = await this.#fetch(
            this.#url(RECEIPT_STREAM_CONTRACT.path),
            { method: "GET", headers, signal: scope.signal },
          );
        } catch {
          throw new NetworkRequestError();
        }

        if (!response.ok) {
          if (
            retryPolicy.transientStatusCodes.includes(response.status) &&
            attempt < retryPolicy.maxAttempts
          ) {
            const retryAfter = retryAfterMs(response, retryPolicy.maxDelayMs);
            await response.body?.cancel();
            scope.dispose();
            await delay(
              retryAfter ?? retryDelayMs(retryPolicy, attempt),
              options.signal,
            );
            continue;
          }
          throw await problemOrTransportError(response);
        }
        if (!contentTypeIs(response, "text/event-stream")) {
          throw new FastiProtocolError(
            "Receipt stream must use text/event-stream",
          );
        }
        if (response.body === null) {
          throw new FastiProtocolError("Receipt stream has no response body");
        }

        for await (const event of parseSse(response.body)) {
          if (event.event !== RECEIPT_STREAM_CONTRACT.eventName) {
            throw new FastiProtocolError(
              "Receipt stream sent an unsupported event type",
            );
          }
          let payload: unknown;
          try {
            payload = JSON.parse(event.data);
          } catch (error) {
            throw new FastiProtocolError("Receipt event data is not JSON", {
              cause: error,
            });
          }
          let data;
          try {
            data = parseReceiptCommittedEvent(payload);
          } catch (error) {
            throw protocolError(
              error,
              "Receipt event violates the generated contract",
            );
          }
          cursor = validateCursor(event.id);
          yield {
            id: cursor,
            event: RECEIPT_STREAM_CONTRACT.eventName,
            data,
          };
        }
      } catch (error) {
        if (scope.timedOut()) throw new FastiTimeoutError();
        if (options.signal?.aborted) throw new FastiAbortError();
        const reconnectable =
          error instanceof NetworkRequestError ||
          error instanceof SseNetworkReadError;
        if (
          reconnectable &&
          retryPolicy.retryNetworkErrors &&
          attempt < retryPolicy.maxAttempts
        ) {
          await delay(retryDelayMs(retryPolicy, attempt), options.signal);
          continue;
        }
        if (reconnectable) {
          throw new FastiTransportError("Receipt stream connection failed");
        }
        if (error instanceof CredentialProviderError) {
          throw new FastiTransportError("Credential provider failed");
        }
        throw error;
      } finally {
        scope.dispose();
      }

      if (attempt < retryPolicy.maxAttempts) {
        await delay(retryDelayMs(retryPolicy, attempt), options.signal);
      }
    }

    throw new FastiTransportError(
      `Receipt stream closed after ${retryPolicy.maxAttempts} bounded attempts`,
    );
  }

  #problemOnlyOperation(
    operation: {
      readonly method: "POST" | "PUT";
      readonly path: string;
      readonly authenticated: true;
    },
    responseLabel: string,
    options: CallOptions,
  ): Promise<never> {
    return this.#jsonOperation({
      method: operation.method,
      path: operation.path,
      authenticated: operation.authenticated,
      retryMode: "never",
      responseParser: unexpectedProblemOnlySuccess,
      responseLabel,
      options,
    });
  }

  async #jsonOperation<T>(input: {
    readonly method: "GET" | "POST" | "PUT";
    readonly path: string;
    readonly authenticated: boolean;
    readonly retryMode: RetryMode;
    readonly body?: unknown;
    readonly responseParser: JsonParser<T>;
    readonly responseLabel: string;
    readonly options: CallOptions;
  }): Promise<T> {
    const retryPolicy = normalizeRetryPolicy(
      input.options.retryPolicy,
      this.#retryPolicy,
    );
    const maximumAttempts =
      input.retryMode === "never" ? 1 : retryPolicy.maxAttempts;
    const serializedBody =
      input.body === undefined ? undefined : JSON.stringify(input.body);

    for (let attempt = 1; attempt <= maximumAttempts; attempt += 1) {
      const scope = createAbortScope(
        input.options.signal,
        input.options.timeoutMs ?? this.#timeoutMs,
      );
      try {
        const headers = await this.#headers(
          "application/json",
          input.authenticated,
          scope.signal,
        );
        if (serializedBody !== undefined) {
          headers.set("Content-Type", "application/json");
        }

        let response: Response;
        try {
          response = await this.#fetch(this.#url(input.path), {
            method: input.method,
            headers,
            body: serializedBody,
            signal: scope.signal,
          });
        } catch {
          throw new NetworkRequestError();
        }

        if (
          retryPolicy.transientStatusCodes.includes(response.status) &&
          attempt < maximumAttempts
        ) {
          const retryAfter = retryAfterMs(response, retryPolicy.maxDelayMs);
          await response.body?.cancel();
          scope.dispose();
          await delay(
            retryAfter ?? retryDelayMs(retryPolicy, attempt),
            input.options.signal,
          );
          continue;
        }
        if (!response.ok) throw await problemOrTransportError(response);
        if (!contentTypeIs(response, "application/json")) {
          throw new FastiProtocolError(
            `${input.responseLabel} must use application/json`,
          );
        }
        const value = await parseJson(response);
        try {
          return input.responseParser(value);
        } catch (error) {
          throw protocolError(
            error,
            `${input.responseLabel} violates the generated contract`,
          );
        }
      } catch (error) {
        if (scope.timedOut()) throw new FastiTimeoutError();
        if (input.options.signal?.aborted) throw new FastiAbortError();
        const retryableNetworkFailure =
          error instanceof NetworkRequestError ||
          error instanceof ResponseReadError;
        if (
          retryableNetworkFailure &&
          retryPolicy.retryNetworkErrors &&
          attempt < maximumAttempts
        ) {
          await delay(retryDelayMs(retryPolicy, attempt), input.options.signal);
          continue;
        }
        if (retryableNetworkFailure) {
          throw new FastiTransportError("Fasti request failed");
        }
        if (error instanceof CredentialProviderError) {
          throw new FastiTransportError("Credential provider failed");
        }
        throw error;
      } finally {
        scope.dispose();
      }
    }
    throw new FastiTransportError("Fasti request exhausted its retry policy");
  }

  async #headers(
    accept: string,
    authenticated: boolean,
    signal: AbortSignal,
  ): Promise<Headers> {
    const headers = new Headers({ Accept: accept });
    if (authenticated && this.#credential !== undefined) {
      const credential = await resolveCredential(this.#credential, signal);
      if (!/^[\x21-\x7e]+$/.test(credential)) {
        throw new CredentialProviderError(
          "Credential must be a non-empty visible ASCII value",
        );
      }
      headers.set("Authorization", `Bearer ${credential}`);
    }
    return headers;
  }

  #url(path: string): URL {
    if (!path.startsWith("/") || path.startsWith("//")) {
      throw new TypeError("Contract paths must be absolute origin paths");
    }
    return new URL(path, this.#baseUrl);
  }
}

function parseOutgoing<T>(
  parser: JsonParser<T>,
  value: unknown,
  label: string,
): T {
  try {
    return parser(value);
  } catch (error) {
    throw protocolError(error, `${label} violates the generated contract`);
  }
}

function unexpectedProblemOnlySuccess(_value: unknown): never {
  throw new FastiProtocolError(
    "Problem-only fixture binding returned an undocumented success",
  );
}

function normalizeBaseUrl(value: string): URL {
  const url = new URL(value);
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new TypeError("baseUrl must use http or https");
  }
  if (url.username !== "" || url.password !== "") {
    throw new TypeError("baseUrl must not contain credentials");
  }
  if (url.search !== "" || url.hash !== "") {
    throw new TypeError("baseUrl must not contain a query or fragment");
  }
  if (url.pathname !== "/") {
    throw new TypeError(
      "baseUrl must be an origin URL without an application path",
    );
  }
  return url;
}

function normalizeRetryPolicy(
  override?: Partial<RetryPolicy>,
  fallback: RetryPolicy = DEFAULT_RETRY_POLICY,
): RetryPolicy {
  const maxAttempts = positiveInteger(
    override?.maxAttempts ?? fallback.maxAttempts,
    "retryPolicy.maxAttempts",
  );
  if (maxAttempts > MAX_RETRY_ATTEMPTS) {
    throw new RangeError(
      `retryPolicy.maxAttempts must not exceed ${MAX_RETRY_ATTEMPTS}`,
    );
  }
  const baseDelayMs = nonNegativeInteger(
    override?.baseDelayMs ?? fallback.baseDelayMs,
    "retryPolicy.baseDelayMs",
  );
  const maxDelayMs = nonNegativeInteger(
    override?.maxDelayMs ?? fallback.maxDelayMs,
    "retryPolicy.maxDelayMs",
  );
  if (baseDelayMs > maxDelayMs) {
    throw new RangeError("retryPolicy.baseDelayMs must not exceed maxDelayMs");
  }
  if (maxDelayMs > MAX_RETRY_DELAY_MS) {
    throw new RangeError(
      `retryPolicy.maxDelayMs must not exceed ${MAX_RETRY_DELAY_MS}`,
    );
  }
  const transientStatusCodes = Object.freeze([
    ...(override?.transientStatusCodes ?? fallback.transientStatusCodes),
  ]);
  if (
    transientStatusCodes.some(
      (status) => !Number.isInteger(status) || status < 400 || status > 599,
    )
  ) {
    throw new RangeError("retryPolicy transient statuses must be HTTP errors");
  }
  return Object.freeze({
    maxAttempts,
    baseDelayMs,
    maxDelayMs,
    transientStatusCodes,
    retryNetworkErrors:
      override?.retryNetworkErrors ?? fallback.retryNetworkErrors,
  });
}

function createAbortScope(
  externalSignal: AbortSignal | undefined,
  timeoutMs: number,
): AbortScope {
  const controller = new AbortController();
  let didTimeOut = false;
  const onAbort = () => controller.abort();
  if (externalSignal?.aborted) {
    controller.abort();
  } else {
    externalSignal?.addEventListener("abort", onAbort, { once: true });
  }
  const timer = setTimeout(
    () => {
      didTimeOut = true;
      controller.abort();
    },
    positiveInteger(timeoutMs, "timeoutMs"),
  );
  return {
    signal: controller.signal,
    timedOut: () => didTimeOut,
    dispose: () => {
      clearTimeout(timer);
      externalSignal?.removeEventListener("abort", onAbort);
    },
  };
}

async function resolveCredential(
  provider: CredentialProvider,
  signal: AbortSignal,
): Promise<string> {
  let credential: string | Promise<string>;
  try {
    credential = typeof provider === "function" ? provider() : provider;
  } catch {
    throw new CredentialProviderError("Credential provider failed");
  }
  try {
    return await abortable(Promise.resolve(credential), signal);
  } catch (error) {
    if (error instanceof ScopedAbortError) throw error;
    throw new CredentialProviderError("Credential provider failed");
  }
}

async function abortable<T>(
  promise: Promise<T>,
  signal: AbortSignal,
): Promise<T> {
  if (signal.aborted) throw new ScopedAbortError();
  return await new Promise<T>((resolve, reject) => {
    const onAbort = () => {
      signal.removeEventListener("abort", onAbort);
      reject(new ScopedAbortError());
    };
    signal.addEventListener("abort", onAbort, { once: true });
    promise.then(
      (value) => {
        signal.removeEventListener("abort", onAbort);
        resolve(value);
      },
      () => {
        signal.removeEventListener("abort", onAbort);
        reject(new CredentialProviderError("Credential provider failed"));
      },
    );
  });
}

async function problemOrTransportError(response: Response): Promise<Error> {
  if (contentTypeIs(response, "application/problem+json")) {
    try {
      return new FastiProblemError(
        parseProblemDetails(await parseJson(response)),
      );
    } catch (error) {
      if (error instanceof ResponseReadError) throw error;
      return protocolError(
        error,
        "Problem response violates RFC 9457 contract",
      );
    }
  }
  return new FastiTransportError(
    `Fasti returned HTTP ${response.status}`,
    response.status,
  );
}

async function parseJson(response: Response): Promise<unknown> {
  if (response.body === null) {
    throw new FastiProtocolError("JSON response has no body");
  }
  const contentLength = response.headers.get("content-length");
  if (
    contentLength !== null &&
    /^\d+$/.test(contentLength) &&
    Number(contentLength) > MAX_JSON_RESPONSE_BYTES
  ) {
    await response.body.cancel().catch(() => undefined);
    throw new FastiProtocolError("JSON response exceeds the bounded body size");
  }
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let totalBytes = 0;
  try {
    while (true) {
      let result: ReadableStreamReadResult<Uint8Array>;
      try {
        result = await reader.read();
      } catch {
        throw new ResponseReadError();
      }
      if (result.done) break;
      totalBytes += result.value.byteLength;
      if (totalBytes > MAX_JSON_RESPONSE_BYTES) {
        throw new FastiProtocolError(
          "JSON response exceeds the bounded body size",
        );
      }
      chunks.push(result.value);
    }
  } finally {
    await reader.cancel().catch(() => undefined);
    reader.releaseLock();
  }
  const bytes = new Uint8Array(totalBytes);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  let text: string;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch (error) {
    throw new FastiProtocolError("JSON response is not valid UTF-8", {
      cause: error,
    });
  }
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new FastiProtocolError("Response body is not valid JSON", {
      cause: error,
    });
  }
}

function protocolError(error: unknown, message: string): Error {
  if (error instanceof FastiProblemError) return error;
  if (error instanceof FastiProtocolError) return error;
  if (error instanceof FastiTransportError) return error;
  if (error instanceof FastiTimeoutError) return error;
  if (error instanceof FastiAbortError) return error;
  if (error instanceof FastiContractParseError) {
    return new FastiProtocolError(message, { cause: error });
  }
  return error instanceof Error
    ? new FastiProtocolError(message, { cause: error })
    : new FastiProtocolError(message);
}

function contentTypeIs(response: Response, expected: string): boolean {
  return (
    response.headers.get("content-type")?.split(";", 1)[0]?.trim() === expected
  );
}

function retryDelayMs(policy: RetryPolicy, failedAttempt: number): number {
  return Math.min(
    policy.maxDelayMs,
    policy.baseDelayMs * 2 ** Math.max(0, failedAttempt - 1),
  );
}

function retryAfterMs(response: Response, maximum: number): number | undefined {
  const header = response.headers.get("retry-after");
  if (header === null || !/^\d+$/.test(header)) return undefined;
  return Math.min(maximum, Number(header) * 1_000);
}

async function delay(
  milliseconds: number,
  signal?: AbortSignal,
): Promise<void> {
  if (signal?.aborted) throw new FastiAbortError();
  if (milliseconds === 0) return;
  await new Promise<void>((resolve, reject) => {
    const finish = () => {
      signal?.removeEventListener("abort", onAbort);
      resolve();
    };
    const timer = setTimeout(finish, milliseconds);
    const onAbort = () => {
      clearTimeout(timer);
      signal?.removeEventListener("abort", onAbort);
      reject(new FastiAbortError());
    };
    signal?.addEventListener("abort", onAbort, { once: true });
    if (signal?.aborted) onAbort();
  });
}

async function* parseSse(
  stream: ReadableStream<Uint8Array>,
): AsyncGenerator<ParsedSseEvent, void, void> {
  const reader = stream.getReader();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const encoder = new TextEncoder();
  let buffer = "";
  let currentLineBytes = 0;
  let id: string | undefined;
  let event = "message";
  let data: string[] = [];
  let eventBytes = 0;
  let eventLines = 0;

  const dispatch = (): ParsedSseEvent | undefined => {
    const parsed =
      data.length === 0
        ? undefined
        : (() => {
            if (id === undefined || id === "") {
              throw new FastiProtocolError(
                "Receipt SSE event must include a cursor id",
              );
            }
            return { id, event, data: data.join("\n") };
          })();
    id = undefined;
    event = "message";
    data = [];
    eventBytes = 0;
    eventLines = 0;
    return parsed;
  };

  const consumeLine = (line: string): ParsedSseEvent | undefined => {
    if (line === "") return dispatch();
    eventLines += 1;
    eventBytes += encoder.encode(line).byteLength + 1;
    if (eventLines > MAX_SSE_EVENT_LINES) {
      throw new FastiProtocolError(
        "Receipt SSE event exceeds the bounded line count",
      );
    }
    if (eventBytes > MAX_SSE_EVENT_BYTES) {
      throw new FastiProtocolError(
        "Receipt SSE event exceeds the bounded aggregate size",
      );
    }
    if (line.startsWith(":")) return undefined;
    const separator = line.indexOf(":");
    const field = separator === -1 ? line : line.slice(0, separator);
    let value = separator === -1 ? "" : line.slice(separator + 1);
    if (value.startsWith(" ")) value = value.slice(1);
    switch (field) {
      case "id":
        id = validateCursor(value);
        return undefined;
      case "event":
        event = value;
        return undefined;
      case "data":
        data.push(value);
        return undefined;
      default:
        throw new FastiProtocolError(
          "Receipt SSE event contains an unsupported field",
        );
    }
  };

  try {
    while (true) {
      let result: ReadableStreamReadResult<Uint8Array>;
      try {
        result = await reader.read();
      } catch {
        throw new SseNetworkReadError();
      }
      if (!result.done) {
        for (const byte of result.value) {
          if (byte === 0x0a) {
            currentLineBytes = 0;
          } else {
            currentLineBytes += 1;
            if (currentLineBytes > MAX_SSE_LINE_BYTES) {
              throw new FastiProtocolError(
                "Receipt SSE line exceeds the bounded transport size",
              );
            }
          }
        }
      }
      try {
        buffer += result.done
          ? decoder.decode()
          : decoder.decode(result.value, { stream: true });
      } catch (error) {
        throw new FastiProtocolError("Receipt SSE stream is not valid UTF-8", {
          cause: error,
        });
      }
      let newline = buffer.indexOf("\n");
      while (newline !== -1) {
        const rawLine = buffer.slice(0, newline);
        buffer = buffer.slice(newline + 1);
        const parsed = consumeLine(
          rawLine.endsWith("\r") ? rawLine.slice(0, -1) : rawLine,
        );
        if (parsed !== undefined) yield parsed;
        newline = buffer.indexOf("\n");
      }
      if (result.done) break;
    }
    if (buffer !== "") {
      const parsed = consumeLine(
        buffer.endsWith("\r") ? buffer.slice(0, -1) : buffer,
      );
      if (parsed !== undefined) yield parsed;
    }
    const finalEvent = dispatch();
    if (finalEvent !== undefined) yield finalEvent;
  } finally {
    await reader.cancel().catch(() => undefined);
    reader.releaseLock();
  }
}

function validateCursor(value: string): string;
function validateCursor(value: undefined): undefined;
function validateCursor(value: string | undefined): string | undefined;
function validateCursor(value: string | undefined): string | undefined {
  if (value === undefined) return undefined;
  if (
    value === "" ||
    value.length > MAX_SSE_CURSOR_CHARACTERS ||
    /[\r\n\0]/.test(value)
  ) {
    throw new TypeError("SSE cursor must be a non-empty single-line value");
  }
  return value;
}

function contractPathIdentifier(
  value: string,
  pattern: RegExp,
  label: string,
): string {
  if (!pattern.test(value)) {
    throw new TypeError(`${label} does not match the generated contract`);
  }
  return value;
}

function positiveInteger(value: number, label: string): number {
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new RangeError(`${label} must be a positive integer`);
  }
  return value;
}

function nonNegativeInteger(value: number, label: string): number {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new RangeError(`${label} must be a non-negative integer`);
  }
  return value;
}
