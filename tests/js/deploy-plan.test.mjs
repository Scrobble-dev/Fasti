import assert from "node:assert/strict";
import test from "node:test";

import {
  createDeploymentPlan,
  renderPosixCommand,
} from "../../packages/deploy-plan/dist/index.js";

test("deployment plans fail closed and keep secrets out of their contract", () => {
  const native = createDeploymentPlan({
    mode: "native",
    port: 8420,
    dataRoot: "/private/fasti data",
  });
  assert.equal(native.available, true);
  assert.match(
    renderPosixCommand(native),
    /FASTI_DATA_ROOT='\/private\/fasti data'/u,
  );
  assert.doesNotMatch(JSON.stringify(native), /token|password|secret/iu);

  const proxy = createDeploymentPlan({
    mode: "trusted-proxy",
    port: 8420,
    dataRoot: "/private/fasti",
    publicUrl: "http://fasti.internal",
  });
  assert.equal(proxy.available, false);
  assert.match(proxy.blockers.join(" "), /HTTPS/u);

  const production = createDeploymentPlan({
    mode: "production",
    port: 8420,
    dataRoot: "/private/fasti",
  });
  assert.equal(production.available, false);
  assert.deepEqual(production.command, []);
});
