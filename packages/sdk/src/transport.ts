import {
  FastiContractParseError,
  parseHealthResponse,
  parseProblemDetails,
  parseReceiptCommittedEvent,
  RECEIPT_STREAM_CONTRACT,
  type HealthResponse,
  type ProblemDetails,
  type ReceiptCommittedEnvelope,
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
  readonly clearTimeout: () => void;
  readonly dispose: () => void;
}

interface ParsedSseEvent {
  readonly id: string;
  readonly event: string;
  readonly data: string;
}

interface ScopedResponse {
  readonly response: Response;
  readonly scope: AbortScope;
}

const DEFAULT_TIMEOUT_MS = 10_000;
const MAX_RETRY_ATTEMPTS = 10;
const MAX_RETRY_DELAY_MS = 60_000;
const MAX_SSE_CHUNK_BYTES = 64 * 1_024;
const MAX_SSE_BUFFER_CHARACTERS = 128 * 1_024;
const MAX_SSE_EVENT_CHARACTERS = 256 * 1_024;
const MAX_SSE_CURSOR_CHARACTERS = 512;

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

  async health(options: CallOptions = {}): Promise<HealthResponse> {
    const { response, scope } = await this.#getWithRetry(
      "/api/v1/health",
      options,
      normalizeRetryPolicy(options.retryPolicy, this.#retryPolicy),
      false,
    );
    try {
      if (!response.ok) {
        throw await problemOrTransportError(response);
      }
      if (!contentTypeIs(response, "application/json")) {
        throw new FastiProtocolError(
          "Health response must use application/json",
        );
      }
      return parseHealthResponse(await parseJson(response));
    } catch (error) {
      if (scope.timedOut()) throw new FastiTimeoutError();
      if (options.signal?.aborted) throw new FastiAbortError();
      throw protocolError(
        error,
        "Health response violates the generated contract",
      );
    } finally {
      scope.dispose();
    }
  }

  /**
   * Opens the governed receipt SSE fixture and performs bounded reconnects.
   * This never persists, queues, or retries a mutation.
   */
  async *receiptEvents(
    options: ReceiptStreamOptions = {},
  ): AsyncGenerator<ReceiptCommittedEnvelope, void, void> {
    let cursor = validateCursor(options.cursor);
    const retryPolicy = normalizeRetryPolicy(
      options.retryPolicy,
      this.#retryPolicy,
    );
    let attempts = 0;

    while (attempts < retryPolicy.maxAttempts) {
      attempts += 1;
      const scope = createAbortScope(
        options.signal,
        options.timeoutMs ?? this.#timeoutMs,
      );
      try {
        const headers = await this.#headers("text/event-stream");
        if (cursor !== undefined) {
          headers.set("Last-Event-ID", cursor);
        }
        let response: Response;
        try {
          response = await this.#fetch(
            this.#url(RECEIPT_STREAM_CONTRACT.path),
            {
              method: "GET",
              headers,
              signal: scope.signal,
            },
          );
        } catch (error) {
          if (scope.timedOut()) throw new FastiTimeoutError();
          if (options.signal?.aborted) throw new FastiAbortError();
          if (
            retryPolicy.retryNetworkErrors &&
            attempts < retryPolicy.maxAttempts
          ) {
            await delay(retryDelayMs(retryPolicy, attempts), options.signal);
            continue;
          }
          throw new FastiTransportError("Receipt stream connection failed");
        }

        scope.clearTimeout();
        if (!response.ok) {
          if (
            retryPolicy.transientStatusCodes.includes(response.status) &&
            attempts < retryPolicy.maxAttempts
          ) {
            const retryAfter = retryAfterMs(response, retryPolicy.maxDelayMs);
            await response.body?.cancel();
            await delay(
              retryAfter ?? retryDelayMs(retryPolicy, attempts),
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
        throw error;
      } finally {
        scope.dispose();
      }

      if (attempts < retryPolicy.maxAttempts) {
        await delay(retryDelayMs(retryPolicy, attempts), options.signal);
      }
    }

    throw new FastiTransportError(
      `Receipt stream closed after ${retryPolicy.maxAttempts} bounded attempts`,
    );
  }

  async #getWithRetry(
    path: string,
    options: CallOptions,
    retryPolicy: RetryPolicy,
    authenticated: boolean,
  ): Promise<ScopedResponse> {
    for (let attempt = 1; attempt <= retryPolicy.maxAttempts; attempt += 1) {
      const headers = await this.#headers("application/json", authenticated);
      const scope = createAbortScope(
        options.signal,
        options.timeoutMs ?? this.#timeoutMs,
      );
      let transferredScope = false;
      try {
        const response = await this.#fetch(this.#url(path), {
          method: "GET",
          headers,
          signal: scope.signal,
        });
        if (
          retryPolicy.transientStatusCodes.includes(response.status) &&
          attempt < retryPolicy.maxAttempts
        ) {
          const retryAfter = retryAfterMs(response, retryPolicy.maxDelayMs);
          await response.body?.cancel();
          await delay(
            retryAfter ?? retryDelayMs(retryPolicy, attempt),
            options.signal,
          );
          continue;
        }
        transferredScope = true;
        return { response, scope };
      } catch (error) {
        if (scope.timedOut()) throw new FastiTimeoutError();
        if (options.signal?.aborted) throw new FastiAbortError();
        if (
          !retryPolicy.retryNetworkErrors ||
          attempt === retryPolicy.maxAttempts
        ) {
          throw new FastiTransportError("Fasti request failed");
        }
        await delay(retryDelayMs(retryPolicy, attempt), options.signal);
      } finally {
        if (!transferredScope) scope.dispose();
      }
    }
    throw new FastiTransportError("Fasti request exhausted its retry policy");
  }

  async #headers(accept: string, authenticated = true): Promise<Headers> {
    const headers = new Headers({ Accept: accept });
    if (authenticated && this.#credential !== undefined) {
      const credential =
        typeof this.#credential === "function"
          ? await this.#credential()
          : this.#credential;
      if (!/^[\x21-\x7e]+$/.test(credential)) {
        throw new TypeError(
          "Credential must be a non-empty visible ASCII value",
        );
      }
      headers.set("Authorization", `Bearer ${credential}`);
    }
    return headers;
  }

  #url(path: string): URL {
    return new URL(path, this.#baseUrl);
  }
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
  url.pathname = `${url.pathname.replace(/\/+$/, "")}/`;
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
  let timeoutCleared = false;
  const clearRequestTimeout = () => {
    if (!timeoutCleared) {
      clearTimeout(timer);
      timeoutCleared = true;
    }
  };
  return {
    signal: controller.signal,
    timedOut: () => didTimeOut,
    clearTimeout: clearRequestTimeout,
    dispose: () => {
      clearRequestTimeout();
      externalSignal?.removeEventListener("abort", onAbort);
    },
  };
}

async function problemOrTransportError(response: Response): Promise<Error> {
  if (contentTypeIs(response, "application/problem+json")) {
    try {
      return new FastiProblemError(
        parseProblemDetails(await parseJson(response)),
      );
    } catch (error) {
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
  try {
    return await response.json();
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
  let buffer = "";
  let id: string | undefined;
  let event = "message";
  let data: string[] = [];
  let dataCharacters = 0;

  const dispatch = (): ParsedSseEvent | undefined => {
    if (data.length === 0) return undefined;
    if (id === undefined || id === "") {
      throw new FastiProtocolError(
        "Receipt SSE event must include a cursor id",
      );
    }
    const parsed = { id, event, data: data.join("\n") };
    id = undefined;
    event = "message";
    data = [];
    dataCharacters = 0;
    return parsed;
  };

  const consumeLine = (line: string): ParsedSseEvent | undefined => {
    if (line === "") return dispatch();
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
        dataCharacters += value.length;
        if (dataCharacters > MAX_SSE_EVENT_CHARACTERS) {
          throw new FastiProtocolError(
            "Receipt SSE event exceeds the bounded payload size",
          );
        }
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
      const { done, value } = await reader.read();
      if (!done && value.byteLength > MAX_SSE_CHUNK_BYTES) {
        throw new FastiProtocolError(
          "Receipt SSE chunk exceeds the bounded transport size",
        );
      }
      buffer += done
        ? decoder.decode()
        : decoder.decode(value, { stream: true });
      if (buffer.length > MAX_SSE_BUFFER_CHARACTERS) {
        throw new FastiProtocolError(
          "Receipt SSE line exceeds the bounded transport size",
        );
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
      if (done) break;
    }
    if (buffer !== "") {
      const parsed = consumeLine(
        buffer.endsWith("\r") ? buffer.slice(0, -1) : buffer,
      );
      if (parsed !== undefined) yield parsed;
    }
    const finalEvent = dispatch();
    if (finalEvent !== undefined) yield finalEvent;
  } catch (error) {
    if (error instanceof FastiProtocolError) throw error;
    throw new FastiProtocolError("Receipt SSE stream is not valid UTF-8", {
      cause: error,
    });
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
