export interface RetainedSearchCandidateRoute {
  kind: "retained";
  providerId: string;
  grain: string;
  candidateReceiptId: string;
  slug: string;
}

export interface LiveSearchCandidateRoute {
  kind: "live";
  providerId: string;
  grain: string;
  providerRecordId: string;
  locale?: string;
}

export type SearchCandidateRoute =
  RetainedSearchCandidateRoute | LiveSearchCandidateRoute;

export function parseSearchCandidateRoute(url: URL): SearchCandidateRoute {
  const segments = url.pathname.split("/");
  const live = segments.length === 7 && segments[2] === "live";
  if ((!live && segments.length !== 6) || segments[1] !== "explore")
    throw new Error("Invalid candidate route");
  const offset = live ? 1 : 0;
  const providerId = decodeURIComponent(segments[2 + offset]);
  const grain = decodeURIComponent(segments[3 + offset]);
  if (
    !/^[a-z0-9][a-z0-9._-]{0,63}$/i.test(providerId) ||
    !/^[a-z0-9][a-z0-9._-]{0,63}$/i.test(grain)
  )
    throw new Error("Invalid candidate route");
  if (live) {
    const providerRecordId = decodeURIComponent(segments[5]);
    const locales = url.searchParams.getAll("locale");
    const locale = locales[0]?.toLowerCase();
    if (
      segments[6] !== "candidate" ||
      !providerRecordId ||
      new TextEncoder().encode(providerRecordId).length > 256 ||
      /[\u0000-\u001f\u007f-\u009f]/.test(providerRecordId) ||
      [...url.searchParams.keys()].some((key) => key !== "locale") ||
      locales.length > 1 ||
      (locale !== undefined &&
        (locale.length < 2 ||
          locale.length > 16 ||
          !/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/.test(locale)))
    )
      throw new Error("Invalid candidate route");
    return { kind: "live", providerId, grain, providerRecordId, locale };
  }
  const candidateReceiptId = decodeURIComponent(segments[4]);
  const slug = decodeURIComponent(segments[5]);
  if (
    !/^scr_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$/.test(
      candidateReceiptId,
    ) ||
    !slug
  )
    throw new Error("Invalid candidate route");
  return { kind: "retained", providerId, grain, candidateReceiptId, slug };
}

export function canonicalLiveCandidatePath(
  providerId: string,
  grain: string,
  providerRecordId: string,
  locale?: string,
): string {
  const path = `/explore/live/${encodeURIComponent(providerId)}/${encodeURIComponent(grain)}/${encodeURIComponent(providerRecordId)}/candidate`;
  return locale ? `${path}?${new URLSearchParams({ locale })}` : path;
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
