import type { HealthResponse } from "@fasti/sdk";

export interface StatusProblem {
  readonly title: string;
  readonly detail: string;
  readonly recovery: string;
}

export type StatusPanelState =
  | { readonly view: "loading" }
  | { readonly view: "healthy"; readonly health: HealthResponse }
  | { readonly view: "blocked"; readonly problem: StatusProblem };
