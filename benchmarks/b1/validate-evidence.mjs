#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";

const here = dirname(fileURLToPath(import.meta.url));
const evidenceSchemaPath = join(here, "evidence.schema.json");
const budgetsPath = join(here, "budgets.json");
const budgetsSchemaPath = join(here, "budgets.schema.json");
const ledgerPath = join(here, "device-hypotheses.json");
const ledgerSchemaPath = join(here, "device-hypotheses.schema.json");

function loadJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

const ajv = new Ajv2020({
  allErrors: true,
  allowUnionTypes: true,
  strict: true,
});

const validatorCache = new Map();

function schemaValidator(schemaPath) {
  if (!validatorCache.has(schemaPath)) {
    validatorCache.set(schemaPath, ajv.compile(loadJson(schemaPath)));
  }
  return validatorCache.get(schemaPath);
}

function assertSchema(validate, value, label) {
  if (!validate(value)) {
    throw new Error(
      `${label} failed JSON Schema validation:\n${ajv.errorsText(validate.errors, { separator: "\n" })}`,
    );
  }
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function median(values) {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
}

function summarize(values) {
  return {
    minimum: Math.min(...values),
    median: median(values),
    maximum: Math.max(...values),
  };
}

function closeEnough(left, right) {
  return Math.abs(left - right) <= 0.000001;
}

function assertSummary(actual, expected, label) {
  for (const key of ["minimum", "median", "maximum"]) {
    assert(
      closeEnough(actual[key], expected[key]),
      `${label}.${key} is not derived from the samples`,
    );
  }
}

const expectedScenarioIds = [
  "native_empty_process",
  "native_fastid_idle",
  "oci_empty_process",
  "oci_fastid_idle",
  "oci_fasti_cli_guard",
];

const nativeScenarioIds = new Set([
  "native_empty_process",
  "native_fastid_idle",
]);
const ociScenarioIds = new Set([
  "oci_empty_process",
  "oci_fastid_idle",
  "oci_fasti_cli_guard",
]);

function validateStaticFiles() {
  const budgets = loadJson(budgetsPath);
  const ledger = loadJson(ledgerPath);
  assertSchema(schemaValidator(budgetsSchemaPath), budgets, "budgets.json");
  assertSchema(
    schemaValidator(ledgerSchemaPath),
    ledger,
    "device-hypotheses.json",
  );

  const profiles = ledger.devices.map((device) => device.profile);
  assert(
    new Set(profiles).size === profiles.length,
    "device hypotheses contain a duplicate profile",
  );
  assert(
    ledger.devices.every(
      (device) =>
        device.qualification_state === "blocking_unassigned" &&
        device.custodian === null &&
        device.runner_fingerprint === null &&
        device.artifact_ref === null &&
        device.contract_ref === null &&
        device.evidence_ref === null &&
        device.results === null,
    ),
    "an unmeasured device hypothesis must remain explicitly unassigned and blocking",
  );

  return budgets;
}

function validateEvidence(evidence, label = "evidence") {
  const budgets = validateStaticFiles();
  assertSchema(schemaValidator(evidenceSchemaPath), evidence, label);

  assert(
    JSON.stringify(evidence.budget_snapshot.memory_bytes) ===
      JSON.stringify(budgets.memory_bytes),
    `${label} budget snapshot differs from the canonical budgets`,
  );
  assert(
    evidence.budget_snapshot.sha256 === sha256(budgetsPath),
    `${label} budget digest is stale`,
  );

  const scenarioIds = evidence.scenarios.map((scenario) => scenario.id);
  assert(
    JSON.stringify(scenarioIds) === JSON.stringify(expectedScenarioIds),
    `${label} scenarios must appear once in the canonical order`,
  );

  for (const scenario of evidence.scenarios) {
    assert(
      scenario.samples.length === evidence.harness.repetitions,
      `${scenario.id} sample count does not match harness.repetitions`,
    );
    assert(
      scenario.samples.every((sample, index) => sample.run === index + 1),
      `${scenario.id} run numbers must be consecutive and one-based`,
    );

    if (nativeScenarioIds.has(scenario.id)) {
      assert(
        scenario.measurement_scope === "native_process_tree",
        `${scenario.id} has the wrong scope`,
      );
      assert(
        scenario.network_denied.mechanism ===
          "linux_network_namespace_without_routes",
        `${scenario.id} lacks native network-namespace proof`,
      );
      assert(
        scenario.samples.every((sample) => sample.cgroup === null),
        `${scenario.id} must not claim cgroup isolation`,
      );
    }

    if (ociScenarioIds.has(scenario.id)) {
      assert(
        scenario.measurement_scope === "oci_process_tree_and_cgroup_v2",
        `${scenario.id} has the wrong scope`,
      );
      assert(
        scenario.network_denied.mechanism === "docker_network_none",
        `${scenario.id} lacks Docker network-none proof`,
      );
      assert(
        scenario.samples.every((sample) => sample.cgroup !== null),
        `${scenario.id} is missing cgroup measurements`,
      );
    }

    for (const sample of scenario.samples) {
      assert(
        sample.steady_process_tree_rss_bytes <=
          sample.peak_process_tree_rss_bytes,
        `${scenario.id} steady process-tree RSS exceeds its peak`,
      );
      if (sample.cgroup !== null) {
        assert(
          sample.cgroup.steady_memory_current_bytes <=
            sample.cgroup.peak_memory_bytes,
          `${scenario.id} steady cgroup memory exceeds its peak`,
        );
      }
    }

    const fields = [
      "startup_ms",
      "steady_process_tree_rss_bytes",
      "peak_process_tree_rss_bytes",
      "process_tree_cpu_seconds",
      "process_tree_cpu_percent",
      "process_count_peak",
    ];
    for (const field of fields) {
      assertSummary(
        scenario.summary[field],
        summarize(scenario.samples.map((sample) => sample[field])),
        `${scenario.id}.summary.${field}`,
      );
    }

    const cgroupFields = [
      ["steady_cgroup_memory_current_bytes", "steady_memory_current_bytes"],
      ["peak_cgroup_memory_bytes", "peak_memory_bytes"],
      ["cgroup_cpu_seconds", "cpu_seconds"],
      ["cgroup_cpu_percent", "cpu_percent"],
    ];
    for (const [summaryField, sampleField] of cgroupFields) {
      if (nativeScenarioIds.has(scenario.id)) {
        assert(
          scenario.summary[summaryField] === null,
          `${scenario.id}.${summaryField} must be null`,
        );
      } else {
        assertSummary(
          scenario.summary[summaryField],
          summarize(
            scenario.samples.map((sample) => sample.cgroup[sampleField]),
          ),
          `${scenario.id}.summary.${summaryField}`,
        );
      }
    }
  }

  const byId = Object.fromEntries(
    evidence.scenarios.map((scenario) => [scenario.id, scenario]),
  );
  const cliExit = byId.oci_fasti_cli_guard.workload_exit;
  assert(
    cliExit.expectation === "guarded_nonzero",
    "OCI CLI must assert guarded nonzero behavior",
  );
  assert(
    cliExit.observed_code !== 0 && cliExit.observed_code !== null,
    "OCI CLI did not record a nonzero guard exit",
  );
  for (const id of expectedScenarioIds.slice(0, 4)) {
    assert(
      byId[id].workload_exit.expectation === "running_until_harness_stop" &&
        byId[id].workload_exit.observed_code === null,
      `${id} must stay alive until the harness stops it`,
    );
  }

  const idleMeasured = Math.max(
    byId.native_fastid_idle.summary.steady_process_tree_rss_bytes.maximum,
    byId.oci_fastid_idle.summary.steady_process_tree_rss_bytes.maximum,
    byId.oci_fastid_idle.summary.steady_cgroup_memory_current_bytes.maximum,
  );
  const absoluteMeasured = Math.max(
    byId.native_fastid_idle.summary.peak_process_tree_rss_bytes.maximum,
    byId.oci_fastid_idle.summary.peak_process_tree_rss_bytes.maximum,
    byId.oci_fastid_idle.summary.peak_cgroup_memory_bytes.maximum,
    byId.oci_fasti_cli_guard.summary.peak_process_tree_rss_bytes.maximum,
    byId.oci_fasti_cli_guard.summary.peak_cgroup_memory_bytes.maximum,
  );

  const verdicts = Object.fromEntries(
    evidence.budget_verdicts.map((verdict) => [verdict.budget, verdict]),
  );
  assert(
    JSON.stringify(Object.keys(verdicts)) ===
      JSON.stringify([
        "idle_target",
        "normal_target",
        "heavy_target",
        "absolute_ceiling",
      ]),
    "budget verdicts must appear once in canonical order",
  );

  for (const budget of Object.keys(verdicts)) {
    assert(
      verdicts[budget].limit_bytes === budgets.memory_bytes[budget],
      `${budget} limit differs from canonical budget`,
    );
  }

  for (const [budget, measured] of [
    ["idle_target", idleMeasured],
    ["absolute_ceiling", absoluteMeasured],
  ]) {
    const verdict = verdicts[budget];
    const expectedStatus =
      measured <= budgets.memory_bytes[budget] ? "pass" : "fail";
    assert(
      verdict.measured_bytes === measured,
      `${budget} measured_bytes is not derived from scenario maxima`,
    );
    assert(
      verdict.status === expectedStatus,
      `${budget} verdict is not derived from its limit`,
    );
  }

  for (const budget of ["normal_target", "heavy_target"]) {
    const verdict = verdicts[budget];
    assert(
      verdict.status === "not_applicable",
      `${budget} must not claim a B1 workload result`,
    );
    assert(
      verdict.measured_bytes === null,
      `${budget} must not invent a measurement`,
    );
    assert(
      verdict.reason.includes("B1 has no implemented"),
      `${budget} must explain why it is not applicable`,
    );
  }
}

function fixtureSummary(samples, field) {
  return summarize(samples.map((sample) => sample[field]));
}

function makeSelfTestEvidence() {
  const budgets = loadJson(budgetsPath);
  const samples = (withCgroup) =>
    [1, 2, 3].map((run) => ({
      run,
      startup_ms: 10 + run,
      steady_process_tree_rss_bytes: 8_000_000 + run,
      peak_process_tree_rss_bytes: 9_000_000 + run,
      process_tree_cpu_seconds: 0.01 * run,
      process_tree_cpu_percent: 0.5 * run,
      process_count_peak: 1,
      cgroup: withCgroup
        ? {
            steady_memory_current_bytes: 10_000_000 + run,
            peak_memory_bytes: 11_000_000 + run,
            cpu_seconds: 0.02 * run,
            cpu_percent: 0.75 * run,
          }
        : null,
    }));

  const scenario = (id, withCgroup, guarded = false) => {
    const values = samples(withCgroup);
    return {
      id,
      subject: `self-test fixture for ${id}`,
      measurement_scope: withCgroup
        ? "oci_process_tree_and_cgroup_v2"
        : "native_process_tree",
      status: "measured",
      network_denied: {
        required: true,
        observed: true,
        mechanism: withCgroup
          ? "docker_network_none"
          : "linux_network_namespace_without_routes",
        proof: "self-test fixture only",
      },
      commands: ["self-test fixture only"],
      workload_exit: guarded
        ? { expectation: "guarded_nonzero", observed_code: 1, matched: true }
        : {
            expectation: "running_until_harness_stop",
            observed_code: null,
            matched: true,
          },
      samples: values,
      summary: {
        startup_ms: fixtureSummary(values, "startup_ms"),
        steady_process_tree_rss_bytes: fixtureSummary(
          values,
          "steady_process_tree_rss_bytes",
        ),
        peak_process_tree_rss_bytes: fixtureSummary(
          values,
          "peak_process_tree_rss_bytes",
        ),
        process_tree_cpu_seconds: fixtureSummary(
          values,
          "process_tree_cpu_seconds",
        ),
        process_tree_cpu_percent: fixtureSummary(
          values,
          "process_tree_cpu_percent",
        ),
        process_count_peak: fixtureSummary(values, "process_count_peak"),
        steady_cgroup_memory_current_bytes: withCgroup
          ? summarize(
              values.map((sample) => sample.cgroup.steady_memory_current_bytes),
            )
          : null,
        peak_cgroup_memory_bytes: withCgroup
          ? summarize(values.map((sample) => sample.cgroup.peak_memory_bytes))
          : null,
        cgroup_cpu_seconds: withCgroup
          ? summarize(values.map((sample) => sample.cgroup.cpu_seconds))
          : null,
        cgroup_cpu_percent: withCgroup
          ? summarize(values.map((sample) => sample.cgroup.cpu_percent))
          : null,
      },
    };
  };

  const scenarios = [
    scenario("native_empty_process", false),
    scenario("native_fastid_idle", false),
    scenario("oci_empty_process", true),
    scenario("oci_fastid_idle", true),
    scenario("oci_fasti_cli_guard", true, true),
  ];

  const idleMeasured = 10_000_003;
  const absoluteMeasured = 11_000_003;
  return {
    $schema:
      "https://fasti.scrobble.dev/schemas/benchmarks/b1/evidence.schema.json",
    schema_version: "fasti.b1.performance-evidence.v1",
    body: "B1",
    status: "complete",
    captured_at: "2026-08-22T00:00:00Z",
    runner: {
      runner_id: "self-test-fixture",
      hardware_profile: "raspberry_pi_5_champion",
      custodian: "self-test-fixture",
      os_release: "self-test Linux",
      kernel_release: "self-test",
      architecture: "self-test",
      cpu_model: "self-test",
      logical_cpu_count: 1,
      total_memory_bytes: 1,
      cgroup_version: "v2",
      container_engine: { name: "docker", version: "self-test" },
    },
    source: {
      git_commit: "1".repeat(40),
      git_tree: "2".repeat(40),
      tree_state: "clean",
      native_fastid_sha256: "3".repeat(64),
      oci_image_ref: "self-test:fixture",
      oci_image_id: "sha256:" + "4".repeat(64),
      contract_ref: "5".repeat(40),
    },
    budget_snapshot: {
      source: "benchmarks/b1/budgets.json",
      sha256: sha256(budgetsPath),
      memory_bytes: budgets.memory_bytes,
    },
    harness: {
      version: "fasti-b1-benchmark.v1",
      repetitions: 3,
      steady_window_seconds: 3,
      sample_interval_ms: 10,
      baseline_subtraction: false,
      fingerprint_commands: ["self-test fixture only"],
      artifact_size_commands: ["self-test fixture only"],
    },
    scenarios,
    artifact_sizes: {
      native_fastid_binary_bytes: 1,
      oci_fastid_binary_bytes: 1,
      oci_fasti_cli_binary_bytes: 1,
      oci_image_bytes: 1,
    },
    budget_verdicts: [
      {
        budget: "idle_target",
        limit_bytes: budgets.memory_bytes.idle_target,
        measured_bytes: idleMeasured,
        status: "pass",
        reason: "self-test derived idle fixture",
      },
      {
        budget: "normal_target",
        limit_bytes: budgets.memory_bytes.normal_target,
        measured_bytes: null,
        status: "not_applicable",
        reason:
          "B1 has no implemented normal-operation workload; no result is claimed.",
      },
      {
        budget: "heavy_target",
        limit_bytes: budgets.memory_bytes.heavy_target,
        measured_bytes: null,
        status: "not_applicable",
        reason:
          "B1 has no implemented heavy-operation workload; no result is claimed.",
      },
      {
        budget: "absolute_ceiling",
        limit_bytes: budgets.memory_bytes.absolute_ceiling,
        measured_bytes: absoluteMeasured,
        status: "pass",
        reason: "self-test derived absolute fixture",
      },
    ],
  };
}

function expectFailure(value, expectedFragment) {
  try {
    validateEvidence(value, "deliberately invalid self-test fixture");
  } catch (error) {
    assert(
      error.message.includes(expectedFragment),
      `unexpected self-test failure: ${error.message}`,
    );
    return;
  }
  throw new Error(`invalid self-test fixture passed: ${expectedFragment}`);
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function runSelfTest() {
  const valid = makeSelfTestEvidence();
  validateEvidence(valid, "valid in-memory self-test fixture");

  const missing = clone(valid);
  delete missing.scenarios[1].samples[0].peak_process_tree_rss_bytes;
  expectFailure(missing, "JSON Schema validation");

  const networked = clone(valid);
  networked.scenarios[3].network_denied.observed = false;
  expectFailure(networked, "JSON Schema validation");

  const staleBudget = clone(valid);
  staleBudget.budget_snapshot.memory_bytes.idle_target += 1;
  expectFailure(staleBudget, "budget snapshot differs");

  const inventedWorkload = clone(valid);
  inventedWorkload.budget_verdicts[1].status = "pass";
  inventedWorkload.budget_verdicts[1].measured_bytes = 1;
  expectFailure(inventedWorkload, "must not claim a B1 workload result");

  console.log(
    "PASS: static schemas, device ledger, evidence semantics, and four negative sentinels",
  );
}

const args = process.argv.slice(2);
if (args.length === 1 && args[0] === "--self-test") {
  runSelfTest();
} else if (args.length === 1 && args[0] === "--static") {
  validateStaticFiles();
  console.log("PASS: B1 benchmark budgets and device hypothesis ledger");
} else if (args.length === 1 && !args[0].startsWith("--")) {
  const path = resolve(args[0]);
  validateEvidence(loadJson(path), path);
  console.log(`PASS: ${path}`);
} else {
  console.error(
    "Usage: node benchmarks/b1/validate-evidence.mjs --static|--self-test|<evidence.json>",
  );
  process.exit(2);
}
