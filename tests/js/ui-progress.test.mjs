import assert from "node:assert/strict";
import test from "node:test";

import { recordProgressPercent } from "../../packages/ui/src/progress.js";

const record = {
  id: "progress-test",
  title: "Progress test",
  mediaKind: "show",
  status: "in_progress",
};

test("record progress is shared and bounded", () => {
  assert.equal(recordProgressPercent({ ...record, status: "completed" }), 100);
  assert.equal(
    recordProgressPercent({
      ...record,
      progressEpisodes: 12,
      totalEpisodes: 10,
    }),
    100,
  );
  assert.equal(
    recordProgressPercent({
      ...record,
      progressEpisodes: -1,
      totalEpisodes: 10,
    }),
    0,
  );
  assert.equal(
    recordProgressPercent({
      ...record,
      progressSeconds: 45,
      totalDurationSeconds: 90,
    }),
    50,
  );
  assert.equal(
    recordProgressPercent({
      ...record,
      progressSeconds: 45,
      totalDurationSeconds: 90,
      totalEpisodes: 10,
    }),
    50,
  );
  assert.equal(
    recordProgressPercent({
      ...record,
      progressSeconds: Number.NaN,
      totalDurationSeconds: 90,
    }),
    0,
  );
});
