import type { WorkbenchHost } from "./types.js";

export type IntegrationRuntimeState =
  | "available"
  | "setup_required"
  | "active"
  | "degraded"
  | "disabled"
  | "unsupported"
  | "error";

export interface IntegrationRuntimeStatus {
  readonly id: string;
  readonly label: string;
  readonly state: IntegrationRuntimeState;
  readonly available: boolean;
  readonly endpoint_ready: boolean;
  readonly setup_action: string;
  readonly detail: string;
}

export interface IntegrationStatusResponse {
  readonly integrations: IntegrationRuntimeStatus[];
}

export interface IntegrationStatusHost {
  listIntegrations(): Promise<IntegrationRuntimeStatus[]>;
}

export function hasIntegrationStatusHost(
  host: WorkbenchHost | undefined,
): host is WorkbenchHost & IntegrationStatusHost {
  return Boolean(
    host &&
    "listIntegrations" in host &&
    typeof (host as Partial<IntegrationStatusHost>).listIntegrations ===
      "function",
  );
}
