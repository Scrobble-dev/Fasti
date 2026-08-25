export interface StatusProblem {
  readonly title: string;
  readonly detail: string;
  readonly recovery: string;
}

export type StatusViewState = "loading" | "healthy" | "blocked";
