export interface DesktopProblem {
  readonly code: string;
  readonly title: string;
  readonly detail: string;
  readonly next_action: string;
}

export type SetupViewState = "loading" | "needs_setup" | "ready" | "blocked";
