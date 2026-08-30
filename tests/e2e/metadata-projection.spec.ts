import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

const recordId = "rec_01991f588e0070008000000000000008";
const profileId = "prf_01991f588e0070008000000000000008";
const digest = `sha256:${"a".repeat(64)}`;

const policy = {
  profile_id: profileId,
  preferred_provider_id: "tmdb",
  preferred_locale: "en-IE",
  original_locale: null,
  allow_english_fallback: true,
  last_known_good: "allow",
  region: "IE",
  enabled_field_groups: ["basic_info", "details"],
} as const;

const policyWithUnimplementedGroups = {
  ...policy,
  enabled_field_groups: [
    ...policy.enabled_field_groups,
    "credits",
    "future_group",
  ],
};

const provenance = {
  claim_id: "claim-title",
  record_id: recordId,
  field_key: "title",
  provider_id: "tmdb",
  source_namespace: "tmdb.movie",
  source_identifier: "438631",
  locale: "en-IE",
  region: "IE",
  source_version: "v3",
  evidence_digest: digest,
  fetched_at: "2026-08-30T09:00:00Z",
  expires_at: "2026-08-31T09:00:00Z",
  status: "fresh",
} as const;

const projectedField = {
  profile_id: profileId,
  record_id: recordId,
  field_key: "title",
  tier: "preferred_provider_claim",
  value: "Dune",
  source_namespace: "tmdb.movie",
  is_stale: false,
  provenance,
  projected_at: "2026-08-30T09:00:01Z",
} as const;

const rating = {
  claim_id: "claim-rating",
  record_id: recordId,
  value_millis: 8400,
  scale: { minimum_millis: 0, maximum_millis: 10000 },
  provenance: {
    ...provenance,
    claim_id: "claim-rating",
    field_key: "community_rating",
  },
} as const;

const cacheEntry = {
  key: {
    provider_id: "tmdb",
    credential_reference_version: 2,
    record_id: recordId,
    resolved_provider_route: "tmdb.movie.details",
    grain: "film",
    source_namespace: "tmdb.movie",
    source_identifier: "438631",
    locale: "en-IE",
    region: "IE",
    field_group: "details",
    settings_fingerprint: digest,
    configuration_digest: digest,
    schema_version: 1,
    purpose: "offline_read",
    terms_revision: "2026-08-01",
    classification: "public",
  },
  claim_ids: ["claim-title", "claim-rating"],
  created_at: "2026-08-30T09:00:00Z",
  fresh_until: "2026-08-31T09:00:00Z",
  stale_while_refreshing_until: "2026-08-31T10:00:00Z",
  stale_on_error_until: "2026-09-02T09:00:00Z",
  invalidation: null,
  read_state: "fresh",
} as const;

const attribution = {
  provider_id: "tmdb",
  text: "Metadata supplied by TMDB.",
  documentation_url: "https://developer.themoviedb.org/",
} as const;

test("injected browser credential authorizes every metadata operation", async ({
  page,
}) => {
  const authorizations: Array<string | undefined> = [];
  const bodies: unknown[] = [];
  await page.route("**/api/v1/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (path.includes("metadata")) {
      authorizations.push(request.headers().authorization);
      if (request.postData()) bodies.push(request.postDataJSON());
    }
    const body = path.endsWith("/metadata-projection")
      ? request.method() === "GET"
        ? {
            profile_id: profileId,
            record_id: recordId,
            policy,
            fields: [projectedField],
            ratings: [rating],
            cache_entries: [cacheEntry],
            attributions: [attribution],
          }
        : { policy, invalidated_cache_entries: 2 }
      : {
          record_id: recordId,
          provider_id: "tmdb",
          claims: [],
          ratings: [rating],
          projections: [projectedField],
          cache_entries: [cacheEntry],
          attributions: [attribution],
        };
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(body),
    });
  });

  await page.goto("/status");
  const result = await page.evaluate(
    async ({ recordId, policy }) => {
      const { createWebHost } = await import("/src/web-host.ts");
      const host = createWebHost(
        window.location.origin,
        async () => "browser-session-proof",
      );
      const projection = await host.readMetadataProjection?.(recordId);
      const { profile_id: _profileId, ...configuration } = policy;
      const configured = await host.configureMetadataProjection?.({
        ...configuration,
        overrides: [],
      });
      const refreshed = await host.refreshMetadataClaims?.({
        operation_id: "op_018f0e0e7f7b70008000000000000004",
        record_id: recordId,
        provider_id: "tmdb",
        field_groups: ["basic_info", "details"],
        locale: "en-IE",
        region: "IE",
        mode: "revalidate",
      });
      return {
        projectionRecordId: projection?.record_id,
        invalidated: configured?.invalidated_cache_entries,
        refreshProvider: refreshed?.provider_id,
      };
    },
    { recordId, policy },
  );

  expect(result).toEqual({
    projectionRecordId: recordId,
    invalidated: 2,
    refreshProvider: "tmdb",
  });
  expect(authorizations).toEqual([
    "Bearer browser-session-proof",
    "Bearer browser-session-proof",
    "Bearer browser-session-proof",
  ]);
  expect(bodies).toHaveLength(2);
});

test("browser without a credential provider exposes metadata as unavailable", async ({
  page,
}) => {
  await page.goto("/status");
  const available = await page.evaluate(async () => {
    const { createWebHost } = await import("/src/web-host.ts");
    const host = createWebHost(window.location.origin);
    return {
      read: typeof host.readMetadataProjection === "function",
      configure: typeof host.configureMetadataProjection === "function",
      refresh: typeof host.refreshMetadataClaims === "function",
    };
  });

  expect(available).toEqual({ read: false, configure: false, refresh: false });
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});

test("saving profile metadata policy refreshes an already selected Record projection", async ({
  page,
}) => {
  await page.addInitScript(
    ({ recordId, profileId, policy }) => {
      let currentPolicy = policy;
      const browserWindow = window as typeof window & {
        __METADATA_PROJECTION_READS__?: number;
        __METADATA_REFRESH_OPERATION_IDS__?: string[];
        __METADATA_REFRESH_FIELD_GROUPS__?: string[][];
        __METADATA_CONFIGURED_FIELD_GROUPS__?: string[];
        __TAURI_INTERNALS__: {
          invoke: (command: string, arguments_?: unknown) => Promise<unknown>;
        };
      };
      browserWindow.__METADATA_PROJECTION_READS__ = 0;
      browserWindow.__METADATA_REFRESH_OPERATION_IDS__ = [];
      browserWindow.__METADATA_REFRESH_FIELD_GROUPS__ = [];
      browserWindow.__TAURI_INTERNALS__ = {
        invoke: async (command, arguments_) => {
          switch (command) {
            case "setup_status":
              return { phase: "ready", proof_cleanup_pending: false };
            case "list_records":
              return {
                records: [
                  {
                    record_id: recordId,
                    grain: "film",
                    status: "active",
                    identifiers: [],
                    latest_activity: null,
                    title: {
                      value: "Dune",
                      source: "tmdb.movie",
                      tier: "preferred_provider_claim",
                      is_stale: false,
                    },
                    original_title: {
                      value: null,
                      source: null,
                      tier: "empty",
                      is_stale: false,
                    },
                    overview: {
                      value: null,
                      source: null,
                      tier: "empty",
                      is_stale: false,
                    },
                    poster: {
                      value: null,
                      source: null,
                      tier: "empty",
                      is_stale: false,
                    },
                    release_year: {
                      value: "2021",
                      source: "tmdb.movie",
                      tier: "preferred_provider_claim",
                      is_stale: false,
                    },
                  },
                ],
                truncated: false,
              };
            case "list_tracking_dispositions":
              return { states: [], truncated: false };
            case "list_reviews":
              return [];
            case "provider_credential_status":
              return [
                {
                  provider: "tmdb",
                  capability_id: "metadata.read",
                  label: "TMDB",
                  purpose: "Read film and television metadata",
                  credential_requirement: "bearer_token",
                  credential_state: "valid",
                  state: "available",
                  source: "credential_store",
                  writable: true,
                  testable: true,
                  docs_url: "https://developer.themoviedb.org/",
                },
              ];
            case "load_network_configuration":
              return {
                connection: {
                  service_url: {
                    value: "http://127.0.0.1:8420",
                    source: "default",
                    managed: false,
                  },
                  public_url: {
                    value: null,
                    source: "default",
                    managed: false,
                  },
                },
                outbound_policy: {
                  allow_providers: [],
                  deny_providers: [],
                  allow_capabilities: [],
                  deny_capabilities: [],
                  allow_hosts: [],
                  deny_hosts: [],
                  allow_networks: [],
                  deny_networks: [],
                },
              };
            case "read_metadata_projection":
              browserWindow.__METADATA_PROJECTION_READS__ =
                (browserWindow.__METADATA_PROJECTION_READS__ ?? 0) + 1;
              return {
                profile_id: profileId,
                record_id: recordId,
                policy: currentPolicy,
                fields: [],
                ratings: [],
                cache_entries: [],
                attributions: [],
              };
            case "configure_metadata_projection": {
              const request = arguments_ as {
                input?: typeof policy;
              };
              browserWindow.__METADATA_CONFIGURED_FIELD_GROUPS__ = [
                ...(request.input?.enabled_field_groups ?? []),
              ];
              currentPolicy = {
                ...currentPolicy,
                ...request.input,
                profile_id: profileId,
              };
              return { policy: currentPolicy, invalidated_cache_entries: 1 };
            }
            case "refresh_metadata_claims": {
              const request = arguments_ as {
                input?: {
                  operation_id?: string;
                  field_groups?: string[];
                };
              };
              browserWindow.__METADATA_REFRESH_OPERATION_IDS__?.push(
                request.input?.operation_id ?? "",
              );
              browserWindow.__METADATA_REFRESH_FIELD_GROUPS__?.push([
                ...(request.input?.field_groups ?? []),
              ]);
              if (
                browserWindow.__METADATA_REFRESH_OPERATION_IDS__?.length === 1
              ) {
                throw {
                  detail: "The refresh response was lost after commit.",
                  next_action: "Retry the same refresh operation.",
                };
              }
              return {
                record_id: recordId,
                provider_id: "tmdb",
                claims: [],
                ratings: [],
                projections: [],
                cache_entries: [],
                attributions: [],
              };
            }
            default:
              throw new Error(`Unexpected trusted-host command: ${command}`);
          }
        },
      };
    },
    { recordId, profileId, policy: policyWithUnimplementedGroups },
  );

  await page.goto(`/records/${recordId}`);
  await expect(
    page.getByRole("heading", { level: 1, name: "Dune" }),
  ).toBeVisible();
  await page.getByRole("button", { name: /Sources & Identity/ }).click();
  await expect(
    page.getByTestId("refresh-metadata-claims").getByRole("status"),
  ).toContainText("This refresh skips credits, future group.");
  await expect(
    page.getByRole("button", { name: "Refresh TMDB" }),
  ).toBeEnabled();
  await page.getByRole("button", { name: "Refresh TMDB" }).click();
  await expect(
    page.getByText("The refresh response was lost after commit."),
  ).toBeVisible();
  await page.getByRole("button", { name: "Refresh TMDB" }).click();
  await expect(
    page.getByText("Refreshed governed claims from TMDB."),
  ).toBeVisible();
  const refreshRequests = await page.evaluate(() => {
    const browserWindow = window as typeof window & {
      __METADATA_REFRESH_OPERATION_IDS__?: string[];
      __METADATA_REFRESH_FIELD_GROUPS__?: string[][];
    };
    return {
      operationIds: browserWindow.__METADATA_REFRESH_OPERATION_IDS__,
      fieldGroups: browserWindow.__METADATA_REFRESH_FIELD_GROUPS__,
    };
  });
  expect(refreshRequests.operationIds).toHaveLength(2);
  expect(refreshRequests.operationIds?.[0]).toMatch(/^op_[0-9a-f]{32}$/);
  expect(refreshRequests.operationIds?.[1]).toBe(
    refreshRequests.operationIds?.[0],
  );
  expect(refreshRequests.fieldGroups).toEqual([
    ["basic_info", "details"],
    ["basic_info", "details"],
  ]);
  await page.getByRole("link", { name: "Settings", exact: true }).click();
  await page
    .getByRole("navigation", { name: "Settings sections" })
    .getByRole("link", { name: "Preferences & Metadata" })
    .click();
  await expect(page.getByTestId("metadata-projection-policy")).toBeVisible();
  const readsBeforeSave = await page.evaluate(
    () =>
      (window as typeof window & { __METADATA_PROJECTION_READS__?: number })
        .__METADATA_PROJECTION_READS__ ?? 0,
  );

  await page.getByLabel("details").uncheck();
  await page.getByLabel("Preferred provider").selectOption("");
  await page.getByTestId("configure-metadata-projection").click();
  await expect(page.getByRole("status")).toContainText(
    "Saved the profile metadata policy",
  );
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as typeof window & { __METADATA_PROJECTION_READS__?: number })
            .__METADATA_PROJECTION_READS__,
      ),
    )
    .toBeGreaterThan(readsBeforeSave);
  expect(
    await page.evaluate(
      () =>
        (
          window as typeof window & {
            __METADATA_CONFIGURED_FIELD_GROUPS__?: string[];
          }
        ).__METADATA_CONFIGURED_FIELD_GROUPS__,
    ),
  ).toEqual(["basic_info", "credits", "future_group"]);

  const firstFieldGroup = page
    .getByRole("group", { name: "Enabled field groups" })
    .locator("label")
    .first();
  expect((await firstFieldGroup.boundingBox())?.height).toBeGreaterThanOrEqual(
    44,
  );

  await page.getByRole("link", { name: "Media Detail" }).click();
  await page.getByRole("button", { name: /Sources & Identity/ }).click();
  await expect(
    page.getByText(
      "Choose a preferred provider in Settings before the first refresh.",
    ),
  ).toBeVisible();

  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});
