export interface SearchCandidateRoute {
  providerId: string;
  grain: string;
  candidateReceiptId: string;
  slug: string;
}

export function routeSlug(title: string): string {
  return (
    title
      .normalize("NFKD")
      .replace(/[\u0300-\u036f]/g, "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "")
      .slice(0, 120)
      .replace(/-$/g, "") || "record"
  );
}
