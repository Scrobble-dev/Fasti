#!/usr/bin/env node

import {
  closeSync,
  constants,
  fstatSync,
  openSync,
  readFileSync,
  realpathSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";

const here = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(here, "../../..");
const artifactRoot = resolve(here, "evidence/artifacts");
const schema = JSON.parse(
  readFileSync(join(here, "evidence.schema.json"), "utf8"),
);
const performanceSchema = JSON.parse(
  readFileSync(join(here, "..", "evidence.schema.json"), "utf8"),
);
const fixturePolicySchema = JSON.parse(
  readFileSync(join(here, "fixture-policy.schema.json"), "utf8"),
);
const fixturePolicyBytes = readFileSync(join(here, "fixture-policy.json"));
const fixturePolicy = JSON.parse(fixturePolicyBytes.toString("utf8"));
const fixturePolicySha256 = createHash("sha256")
  .update(fixturePolicyBytes)
  .digest("hex");
const ajv = new Ajv2020({ allErrors: true, strict: true });
addFormats(ajv);
ajv.addSchema(performanceSchema);
const validateSchema = ajv.compile(schema);
const validateFixturePolicy = ajv.compile(fixturePolicySchema);

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function median(values) {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
}

function summary(values) {
  return {
    minimum: Math.min(...values),
    median: median(values),
    maximum: Math.max(...values),
  };
}

function shellQuote(value) {
  return /^[A-Za-z0-9_@%+=:,./-]+$/.test(value)
    ? value
    : `'${value.replaceAll("'", `'"'"'`)}'`;
}

function readRetainedArtifactOnce(artifact) {
  assert(
    Number.isInteger(constants.O_NOFOLLOW),
    "qualifying artifact verification requires O_NOFOLLOW",
  );
  const candidate = resolve(repositoryRoot, artifact.path);
  const lexicalRelative = relative(artifactRoot, candidate);
  assert(
    lexicalRelative &&
      !lexicalRelative.startsWith("..") &&
      !isAbsolute(lexicalRelative),
    "Tauri artifact path escapes the private evidence package",
  );
  const artifactRootReal = realpathSync(artifactRoot);
  const candidateRealBefore = realpathSync(candidate);
  const realRelative = relative(artifactRootReal, candidateRealBefore);
  assert(
    realRelative && !realRelative.startsWith("..") && !isAbsolute(realRelative),
    "Tauri artifact resolves outside the private evidence package",
  );
  const descriptor = openSync(
    candidate,
    constants.O_RDONLY | constants.O_NOFOLLOW,
  );
  try {
    assert(
      fstatSync(descriptor).isFile(),
      "Tauri artifact is not a regular file",
    );
    const bytes = readFileSync(descriptor);
    assert(
      realpathSync(candidate) === candidateRealBefore,
      "Tauri artifact path changed during verification",
    );
    return bytes;
  } finally {
    closeSync(descriptor);
  }
}

export function validateEvidence(
  evidence,
  { allowTestFixture = false, verifyArtifact = true } = {},
) {
  if (!validateSchema(evidence)) {
    throw new Error(ajv.errorsText(validateSchema.errors, { separator: "\n" }));
  }
  assert(
    evidence.status === "complete" || allowTestFixture,
    "only a complete captured Tauri receipt can pass standalone validation",
  );
  assert(
    evidence.scope.fixture_policy_sha256 === fixturePolicySha256,
    "Tauri receipt is not bound to the canonical fixture policy",
  );
  assert(
    evidence.samples.length === evidence.harness.repetitions,
    "sample count must equal harness repetitions",
  );
  assert(
    evidence.samples.every((sample, index) => sample.run === index + 1),
    "run numbers must be consecutive and one-based",
  );
  assert(
    new Set(evidence.samples.map((sample) => sample.systemd_unit)).size ===
      evidence.samples.length,
    "every Tauri repetition must use an independent systemd scope",
  );
  for (const sample of evidence.samples) {
    const expectedArgv = [
      "systemd-run",
      "--user",
      "--scope",
      "--quiet",
      `--unit=${sample.systemd_unit}`,
      "--property=MemoryAccounting=yes",
      "unshare",
      "--user",
      "--map-root-user",
      "--net",
      "--",
      resolve(repositoryRoot, evidence.artifact.measurement_path),
    ];
    const expectedCommand = [
      `FASTI_TAURI_BENCHMARK_READY_FILE=${sample.ready_file}`,
      ...expectedArgv,
    ]
      .map(shellQuote)
      .join(" ");
    assert(
      isDeepStrictEqual(sample.argv, expectedArgv) &&
        sample.command === expectedCommand,
      `Tauri run ${sample.run} command is not bound to its scope, network namespace, and artifact`,
    );
  }
  if (verifyArtifact && !allowTestFixture) {
    const artifactBytes = readRetainedArtifactOnce(evidence.artifact);
    assert(
      artifactBytes.length === evidence.artifact.size_bytes,
      "Tauri retained artifact size does not recompute",
    );
    assert(
      createHash("sha256").update(artifactBytes).digest("hex") ===
        evidence.artifact.sha256,
      "Tauri retained artifact SHA-256 does not recompute",
    );
  }
  for (const field of [
    "startup_ms",
    "steady_cgroup_memory_bytes",
    "peak_cgroup_memory_bytes",
    "process_count_peak",
  ]) {
    assert(
      isDeepStrictEqual(
        evidence.summary[field],
        summary(evidence.samples.map((sample) => sample[field])),
      ),
      `${field} summary is not derived from samples`,
    );
  }
  assert(
    evidence.samples.every(
      (sample) =>
        sample.steady_cgroup_memory_bytes <= sample.peak_cgroup_memory_bytes,
    ),
    "steady memory must not exceed peak memory",
  );
  const expectedDisplayServer =
    evidence.runner.display_evidence.session_type === "wayland"
      ? evidence.runner.display_server === "wayland_and_x11"
        ? "wayland_and_x11"
        : "wayland"
      : "x11";
  assert(
    evidence.runner.display_server === expectedDisplayServer,
    "display server is not derived from the governed login session",
  );
  assert(
    isDeepStrictEqual(
      evidence.runner.display_evidence.connected_drm_connectors,
      [...evidence.runner.display_evidence.connected_drm_connectors].sort(),
    ),
    "connected DRM connectors must be in canonical order",
  );
  const measured = evidence.summary.peak_cgroup_memory_bytes.maximum;
  assert(
    evidence.verdict.measured_bytes === measured,
    "verdict measurement is not the worst cgroup-v2 peak",
  );
  assert(
    evidence.verdict.status ===
      (measured <= evidence.verdict.limit_bytes ? "pass" : "fail"),
    "verdict status is not derived from the absolute ceiling",
  );
  const targetPass = measured <= evidence.verdict.low_ram_target_bytes;
  assert(
    evidence.verdict.low_ram_target_status === (targetPass ? "pass" : "fail"),
    "low-RAM target status is not derived from the worst process-tree peak",
  );
  const disposition = targetPass
    ? "within_low_ram_target"
    : measured <= evidence.verdict.limit_bytes
      ? "target_miss_requires_b8_tuning"
      : "absolute_breach_blocks_b8";
  assert(
    evidence.verdict.disposition === disposition,
    "packaging disposition is not derived from both RAM thresholds",
  );
}

function selfTest() {
  const fixture = {
    $schema:
      "https://fasti.scrobble.dev/schemas/benchmarks/b1/tauri-shell-evidence.schema.json",
    schema_version: "fasti.b1.tauri-shell-evidence.v1",
    body: "B1",
    status: "test_fixture",
    captured_at: "2026-08-22T00:00:00Z",
    scope: {
      benchmark_only: true,
      product_surface: false,
      window_visible: false,
      served_web: "not_applicable",
      design_review: "not_applicable_non_product_hidden_benchmark_fixture",
      qualifying_runner: "governed_linux_desktop_cgroup_v2",
      measurement_boundary: "dedicated_transient_user_scope",
      fixture_policy_sha256: fixturePolicySha256,
    },
    source: {
      git_commit: "1".repeat(40),
      git_tree: "2".repeat(40),
      tree_state: "clean",
      fixture_tree: "3".repeat(40),
      cargo_lock_sha256: "4".repeat(64),
      harness_script_sha256: "6".repeat(64),
    },
    runner: {
      runner_id: "self-test",
      os: "linux",
      os_version: "self-test",
      kernel: "self-test",
      architecture: "arm64",
      cpu_model: "self-test",
      logical_cpu_count: 1,
      total_memory_bytes: 1,
      display_server: "wayland",
      display_evidence: {
        session_id: "2",
        session_type: "wayland",
        session_class: "user",
        session_remote: false,
        session_state: "active",
        seat: "seat0",
        connected_drm_connectors: ["card0-HDMI-A-1"],
        simulation_scan: "none_detected",
      },
      systemd_user_scope: true,
    },
    environment: {
      os_image: {
        pretty_name: "Test Linux",
        id: "test",
        version_id: "1",
        version_codename: "test",
        build_id: null,
        image_id: null,
        image_version: null,
        claim_scope: "runtime_os_release_fields_only",
        retained_image: {
          file_name: "test-linux.iso",
          size_bytes: 1,
          sha256: "7".repeat(64),
        },
        approval: "retained_digest_recorded_no_profile_allowlist",
      },
      firmware: {
        source: "test firmware source",
        description: "test firmware",
        sha256: "8".repeat(64),
      },
      root_filesystem: {
        source: "/dev/test",
        type: "ext4",
        mount_options: ["rw"],
      },
      storage: {
        root_source: "/dev/test",
        root_filesystem_type: "ext4",
        root_mount_options: ["rw"],
        physical_device: "/dev/test",
        device_type: "disk",
        transport: "test",
        storage_class: "unknown_non_rotational",
        classification_evidence: ["lsblk.ROTA=0", "no_exact_ssd_marker"],
        rotational: false,
        size_bytes: 1,
        model: "test",
        usb_link_speed_mbps: null,
        identity_sha256: "9".repeat(64),
        raw_serial_recorded: false,
      },
      cpu_governor: {
        source: "/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor",
        observed: ["test"],
        cpu_count_observed: 1,
      },
      temperature: {
        preflight: {
          source: "/sys/class/thermal/test",
          sensor: "test",
          celsius: 30,
        },
        post_capture: {
          source: "/sys/class/thermal/test",
          sensor: "test",
          celsius: 31,
        },
      },
      container_runtime: {
        status: "not_applicable",
        reason: "native packaging fixture",
      },
      cgroup: {
        version: "v2",
        manager: "systemd_user_transient_scope",
        controller: "memory",
      },
      corpus: {
        status: "not_applicable",
        seed: null,
        digest: null,
        reason: "empty shell has no generated corpus",
      },
      webkit_runtime: { package: "webkit2gtk-4.1", version: "1" },
      fingerprint_commands: [
        "command 1",
        "command 2",
        "command 3",
        "command 4",
        "command 5",
        "command 6",
        "command 7",
        "command 8",
      ],
    },
    harness: {
      version: "fasti-b1-tauri-shell.v1",
      repetitions: 5,
      steady_window_seconds: 3,
      sample_interval_ms: 25,
      measurement_backend: "linux_cgroup_v2_memory_controller",
      network_isolation: "route_less_user_network_namespace",
      commands: ["cargo build fixture", "run fixture"],
    },
    artifact: {
      measurement_path:
        "benchmarks/b1/tauri-shell/src-tauri/target/release/fasti-b1-tauri-shell",
      path:
        "benchmarks/b1/tauri-shell/evidence/artifacts/sha256-" +
        "5".repeat(64) +
        "-fasti-b1-tauri-shell",
      sha256: "5".repeat(64),
      size_bytes: 1,
    },
    samples: [1, 2, 3, 4, 5].map((run) => {
      const systemdUnit = `fasti-b1-tauri-${String(run).repeat(32)}.scope`;
      const readyFile = `/tmp/fasti-tauri-b1-self-${run}/ready`;
      const argv = [
        "systemd-run",
        "--user",
        "--scope",
        "--quiet",
        `--unit=${systemdUnit}`,
        "--property=MemoryAccounting=yes",
        "unshare",
        "--user",
        "--map-root-user",
        "--net",
        "--",
        resolve(
          repositoryRoot,
          "benchmarks/b1/tauri-shell/src-tauri/target/release/fasti-b1-tauri-shell",
        ),
      ];
      return {
        run,
        systemd_unit: systemdUnit,
        ready_file: readyFile,
        argv,
        command: [`FASTI_TAURI_BENCHMARK_READY_FILE=${readyFile}`, ...argv]
          .map(shellQuote)
          .join(" "),
        startup_ms: 10 + run,
        steady_cgroup_memory_bytes: 100 + run,
        peak_cgroup_memory_bytes: 200 + run,
        process_count_peak: run,
      };
    }),
    summary: {
      startup_ms: { minimum: 11, median: 13, maximum: 15 },
      steady_cgroup_memory_bytes: { minimum: 101, median: 103, maximum: 105 },
      peak_cgroup_memory_bytes: { minimum: 201, median: 203, maximum: 205 },
      process_count_peak: { minimum: 1, median: 3, maximum: 5 },
    },
    verdict: {
      budget: "absolute_ceiling",
      low_ram_target_bytes: 100663296,
      low_ram_target_status: "pass",
      limit_bytes: 201326592,
      measured_bytes: 205,
      status: "pass",
      disposition: "within_low_ram_target",
      effect: "failure_blocks_b8_packaging_not_b1_contracts",
    },
  };
  validateEvidence(fixture, { allowTestFixture: true });
  function expectRejected(candidate, expectedMessage, label) {
    try {
      validateEvidence(candidate, { allowTestFixture: true });
    } catch (error) {
      assert(
        error.message.includes(expectedMessage),
        `unexpected ${label} mutation failure: ${error.message}`,
      );
      return;
    }
    throw new Error(`${label} mutation passed Tauri validation`);
  }
  const mutation = structuredClone(fixture);
  mutation.verdict.measured_bytes = 1;
  expectRejected(mutation, "worst cgroup-v2 peak", "invented verdict");
  const displayMutation = structuredClone(fixture);
  displayMutation.runner.display_evidence.session_remote = true;
  expectRejected(displayMutation, "session_remote", "remote display");
  const duplicateRun = structuredClone(fixture);
  duplicateRun.samples[1].systemd_unit = duplicateRun.samples[0].systemd_unit;
  duplicateRun.samples[1].ready_file = duplicateRun.samples[0].ready_file;
  duplicateRun.samples[1].argv = duplicateRun.samples[0].argv;
  duplicateRun.samples[1].command = duplicateRun.samples[0].command;
  expectRejected(
    duplicateRun,
    "independent systemd scope",
    "duplicated repetition",
  );
  const appendedCommand = structuredClone(fixture);
  appendedCommand.samples[0].command += " --unexpected-argument";
  expectRejected(
    appendedCommand,
    "not bound to its scope",
    "non-exact artifact command",
  );
  const prefixedCommand = structuredClone(fixture);
  prefixedCommand.samples[0].command = `echo forged && ${prefixedCommand.samples[0].command}`;
  expectRejected(prefixedCommand, "not bound to its scope", "prefixed command");
  const interposedArgv = structuredClone(fixture);
  interposedArgv.samples[0].argv[6] = "echo-forged";
  expectRejected(interposedArgv, "not bound to its scope", "interposed argv");
  console.log("PASS: Tauri shell evidence schema and mutation sentinels");
}

if (process.argv[2] === "--self-test") {
  selfTest();
} else if (process.argv[2] === "--policy" && process.argv.length === 3) {
  if (!validateFixturePolicy(fixturePolicy)) {
    throw new Error(
      ajv.errorsText(validateFixturePolicy.errors, { separator: "\n" }),
    );
  }
  console.log("PASS: canonical hidden Tauri fixture policy schema");
} else if (process.argv.length === 3) {
  validateEvidence(JSON.parse(readFileSync(process.argv[2], "utf8")));
  console.log(`PASS: validated Tauri shell evidence ${process.argv[2]}`);
} else {
  console.error(
    "usage: validate-evidence.mjs --self-test | --policy | <evidence.json>",
  );
  process.exit(2);
}
