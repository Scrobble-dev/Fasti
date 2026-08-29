import assert from "node:assert/strict";
import test from "node:test";

import { projectRecordSummary } from "../../packages/ui/src/record-projection.ts";

test("canonical provider grains project to their display kinds", () => {
  for (const [grain, expected] of [
    ["film", "movie"],
    ["series", "show"],
    ["edition", "book"],
  ]) {
    const record = projectRecordSummary({
      record_id: `record-${grain}`,
      grain,
      status: "active",
      title: { tier: "fallback_provider_claim", value: "Title" },
      poster: { tier: "empty", value: null },
      identifiers: [],
      latest_activity: null,
    });

    assert.equal(record.mediaKind, expected);
  }
});
