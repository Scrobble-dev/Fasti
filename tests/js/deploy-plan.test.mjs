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
  for (const publicUrl of [
    "https://fasti.internal/path",
    "https://fasti.internal/?query=yes",
    "https://fasti.internal/#fragment",
    "https://fasti.internal:0",
  ]) {
    assert.equal(
      createDeploymentPlan({
        mode: "trusted-proxy",
        port: 8420,
        dataRoot: "/private/fasti",
        publicUrl,
      }).available,
      false,
    );
  }
  assert.equal(
    createDeploymentPlan({
      mode: "trusted-proxy",
      port: 8420,
      dataRoot: "/private/fasti",
      publicUrl: "https://fasti.internal",
    }).available,
    true,
  );

  const podman = createDeploymentPlan({
    mode: "podman",
    port: 8420,
    dataRoot: "/private/fasti",
  });
  assert.ok(podman.command.includes("FASTI_EXTERNAL_BIND_IP=127.0.0.1"));
  assert.equal(
    podman.verification[0],
    "curl --fail --show-error http://127.0.0.1:8420/api/v1/health",
  );

  const production = createDeploymentPlan({
    mode: "production",
    port: 8420,
    dataRoot: "/private/fasti",
  });
  assert.equal(production.available, false);
  assert.deepEqual(production.command, []);
});
