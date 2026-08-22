#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  closeSync,
  constants as fsConstants,
  fstatSync,
  mkdtempSync,
  openSync,
  readFileSync,
  realpathSync,
  renameSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import Ajv2020 from "ajv/dist/2020.js";

const here = dirname(fileURLToPath(import.meta.url));
const evidenceSchemaPath = join(here, "evidence.schema.json");
const budgetsPath = join(here, "budgets.json");
const budgetsSchemaPath = join(here, "budgets.schema.json");
const ledgerPath = join(here, "device-hypotheses.json");
const ledgerSchemaPath = join(here, "device-hypotheses.schema.json");
const governedBuildRecipePath = join(here, "Dockerfile");
const physicalProfilesPath = join(here, "physical-profiles.json");
const physicalProfilesSchemaPath = join(here, "physical-profiles.schema.json");

function loadJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function requireNoFollowSupport(value) {
  assert(
    Number.isInteger(value),
    "retained artifact verification requires O_NOFOLLOW support",
  );
  return value;
}

function containedPath(root, target) {
  const value = relative(root, target);
  return value && !value.startsWith("..") && !isAbsolute(value);
}

function readContainedRegularFileOnce(
  root,
  target,
  label,
  { afterOpen = () => {} } = {},
) {
  const noFollow = requireNoFollowSupport(fsConstants.O_NOFOLLOW);
  const lexicalRoot = resolve(root);
  const lexicalTarget = resolve(target);
  assert(
    containedPath(lexicalRoot, lexicalTarget),
    `${label} path escapes its governed directory`,
  );
  const realRoot = realpathSync(lexicalRoot);
  const realParent = realpathSync(dirname(lexicalTarget));
  assert(
    realParent === realRoot || containedPath(realRoot, realParent),
    `${label} parent resolves outside its governed directory`,
  );
  let descriptor;
  try {
    descriptor = openSync(lexicalTarget, fsConstants.O_RDONLY | noFollow);
    const metadata = fstatSync(descriptor);
    assert(metadata.isFile(), `${label} is not a regular file`);
    afterOpen();
    const bytes = readFileSync(descriptor);
    assert(
      bytes.length === metadata.size,
      `${label} changed while it was read`,
    );
    return {
      bytes,
      digest: createHash("sha256").update(bytes).digest("hex"),
      size: metadata.size,
    };
  } finally {
    if (descriptor !== undefined) closeSync(descriptor);
  }
}

function readRetainedArtifactOnce(receiptPath, reference, label) {
  const receiptDirectory = dirname(resolve(receiptPath));
  const target = resolve(receiptDirectory, reference.path);
  const snapshot = readContainedRegularFileOnce(
    receiptDirectory,
    target,
    `${label} retained artifact`,
  );
  assert(
    snapshot.size === reference.size_bytes &&
      snapshot.digest === reference.sha256 &&
      reference.path.endsWith(`/${snapshot.digest}.tar.gz`),
    `${label} retained artifact bytes, size, digest, and content-addressed path do not match`,
  );
  return snapshot.bytes;
}

function validateRetainedArtifacts(evidence, evidencePath, label) {
  const image = readRetainedArtifactOnce(
    evidencePath,
    evidence.retained_artifacts.oci_image_compressed,
    `${label} OCI image`,
  );
  const contracts = readRetainedArtifactOnce(
    evidencePath,
    evidence.retained_artifacts.contract_pack_compressed,
    `${label} contract pack`,
  );
  assert(
    image.length === evidence.artifact_sizes.oci_image_compressed_bytes &&
      evidence.retained_artifacts.oci_image_compressed.sha256 ===
        evidence.artifact_sizes.oci_image_compressed_sha256 &&
      contracts.length ===
        evidence.artifact_sizes.contract_pack_compressed_bytes &&
      evidence.retained_artifacts.contract_pack_compressed.sha256 ===
        evidence.artifact_sizes.contract_pack_compressed_sha256,
    `${label} retained artifact references do not correlate to artifact sizes`,
  );
}

function canonicalBudgetSnapshot(budgets) {
  return {
    source: "benchmarks/b1/budgets.json",
    sha256: sha256(budgetsPath),
    memory_bytes: budgets.memory_bytes,
    idle_cpu_percent_one_core: budgets.idle_cpu_percent_one_core,
    timing_seconds: budgets.timing_seconds,
    artifact_bytes: budgets.artifact_bytes,
  };
}

const ajv = new Ajv2020({
  allErrors: true,
  allowUnionTypes: true,
  strict: true,
});

const validatorCache = new Map();

function schemaValidator(schemaPath) {
  if (!validatorCache.has(schemaPath)) {
    if (schemaPath === ledgerSchemaPath) schemaValidator(evidenceSchemaPath);
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

function round6(value) {
  return Math.round((value + Number.EPSILON) * 1_000_000) / 1_000_000;
}

function p95NearestRank(values) {
  const ordered = [...values].sort((left, right) => left - right);
  const index = Math.max(
    0,
    Math.min(ordered.length - 1, Math.ceil(ordered.length * 0.95) - 1),
  );
  return ordered[index];
}

function assertSampleDerivedFromObservations(
  sample,
  scenarioId,
  withCgroup,
  idleExpected,
  budgets,
) {
  const observations = sample.observations;
  observations.forEach((observation, index) => {
    assert(
      observation.sequence === index + 1,
      `${scenarioId} raw observation sequence is not consecutive`,
    );
    if (index > 0) {
      assert(
        observation.elapsed_ns > observations[index - 1].elapsed_ns,
        `${scenarioId} raw observations contain a duplicate or non-monotonic timestamp`,
      );
      assert(
        observation.process_tree_cpu_runtime_ns >=
          observations[index - 1].process_tree_cpu_runtime_ns,
        `${scenarioId} process schedstat runtime counter regressed`,
      );
      if (withCgroup) {
        assert(
          observation.cgroup_cpu_runtime_ns >=
            observations[index - 1].cgroup_cpu_runtime_ns,
          `${scenarioId} cgroup CPU runtime counter regressed`,
        );
      }
    }
    assert(
      observation.steady ===
        observation.elapsed_ns >= sample.steady_started_elapsed_ns,
      `${scenarioId} raw observation steady-window marker is forged`,
    );
    const cgroupRequiredFields = [
      "cgroup_memory_current_bytes",
      "cgroup_memory_peak_bytes",
      "cgroup_cpu_runtime_ns",
      "cgroup_oom_kill_count",
    ];
    assert(
      cgroupRequiredFields.every((field) =>
        withCgroup ? observation[field] !== null : observation[field] === null,
      ) &&
        (withCgroup ||
          (observation.cgroup_memory_limit_bytes === null &&
            observation.cgroup_swap_limit_bytes === null)),
      `${scenarioId} raw cgroup observation scope is inconsistent`,
    );
  });
  assert(
    observations.at(-1).elapsed_ns <= sample.finished_elapsed_ns,
    `${scenarioId} raw observation exceeds the recorded finish time`,
  );
  const steady = observations.filter((observation) => observation.steady);
  assert(
    steady.length >= 2,
    `${scenarioId} has insufficient raw steady observations`,
  );
  const steadyRss = steady.map(
    (observation) => observation.process_tree_rss_bytes,
  );
  assertSummary(
    sample.steady_process_tree_rss_statistics,
    summarize(steadyRss),
    `${scenarioId} raw steady process-tree RSS`,
  );
  assert(
    sample.steady_process_tree_rss_bytes === Math.max(...steadyRss) &&
      sample.peak_process_tree_rss_bytes ===
        Math.max(
          ...observations.map(
            (observation) => observation.process_tree_rss_bytes,
          ),
        ) &&
      sample.process_count_peak ===
        Math.max(
          ...observations.map((observation) => observation.process_count),
        ),
    `${scenarioId} memory/process aggregates are not derived from raw observations`,
  );
  const final = observations.at(-1);
  const expectedProcessSeconds = round6(
    final.process_tree_cpu_runtime_ns / 1_000_000_000,
  );
  const expectedProcessPercent = round6(
    (final.process_tree_cpu_runtime_ns / sample.finished_elapsed_ns) * 100,
  );
  assert(
    closeEnough(sample.process_tree_cpu_seconds, expectedProcessSeconds) &&
      closeEnough(sample.process_tree_cpu_percent, expectedProcessPercent),
    `${scenarioId} process CPU aggregate is not derived from schedstat nanoseconds`,
  );

  if (withCgroup) {
    const steadyMemory = steady.map(
      (observation) => observation.cgroup_memory_current_bytes,
    );
    assertSummary(
      sample.cgroup.steady_memory_current_statistics,
      summarize(steadyMemory),
      `${scenarioId} raw steady cgroup memory`,
    );
    assert(
      sample.cgroup.steady_memory_current_bytes === Math.max(...steadyMemory) &&
        sample.cgroup.peak_memory_bytes ===
          Math.max(
            ...observations.map(
              (observation) => observation.cgroup_memory_peak_bytes,
            ),
          ) &&
        closeEnough(
          sample.cgroup.cpu_seconds,
          round6(final.cgroup_cpu_runtime_ns / 1_000_000_000),
        ) &&
        closeEnough(
          sample.cgroup.cpu_percent,
          round6(
            (final.cgroup_cpu_runtime_ns / sample.finished_elapsed_ns) * 100,
          ),
        ) &&
        observations.every(
          (observation) =>
            observation.cgroup_memory_limit_bytes ===
              sample.cgroup.memory_limit_bytes &&
            observation.cgroup_swap_limit_bytes ===
              sample.cgroup.swap_limit_bytes,
        ) &&
        sample.cgroup.oom_kill_count ===
          Math.max(
            ...observations.map(
              (observation) => observation.cgroup_oom_kill_count,
            ),
          ),
      `${scenarioId} cgroup aggregates are not derived from raw observations`,
    );
  }

  if (idleExpected) {
    const observedWarmupNs =
      sample.steady_started_elapsed_ns - sample.startup_ms * 1_000_000;
    assert(
      observedWarmupNs >=
        budgets.timing_seconds.idle_warmup * 1_000_000_000 - 1_000_000,
      `${scenarioId} raw timing does not prove the locked idle warm-up`,
    );
    const counter = withCgroup
      ? "cgroup_cpu_runtime_ns"
      : "process_tree_cpu_runtime_ns";
    const intervals = steady.slice(1).map((observation, index) => {
      const previous = steady[index];
      return (
        ((observation[counter] - previous[counter]) /
          (observation.elapsed_ns - previous.elapsed_ns)) *
        100
      );
    });
    const measuredNs = steady.at(-1).elapsed_ns - steady[0].elapsed_ns;
    const totalCpuNs = steady.at(-1)[counter] - steady[0][counter];
    assert(
      measuredNs >= budgets.timing_seconds.idle_measurement * 1_000_000_000 &&
        sample.idle_cpu.interval_count === intervals.length &&
        closeEnough(
          sample.idle_cpu.measurement_seconds,
          round6(measuredNs / 1_000_000_000),
        ) &&
        closeEnough(
          sample.idle_cpu.average_percent_one_core,
          round6((totalCpuNs / measuredNs) * 100),
        ) &&
        closeEnough(
          sample.idle_cpu.p95_percent_one_core,
          round6(p95NearestRank(intervals)),
        ),
      `${scenarioId} idle CPU result is not derived from raw runtime observations`,
    );
  }
}

function assertSummary(actual, expected, label) {
  for (const key of ["minimum", "median", "maximum"]) {
    assert(
      closeEnough(actual[key], expected[key]),
      `${label}.${key} is not derived from the samples`,
    );
  }
}

function assertOrderedSummary(summary, label) {
  assert(
    summary.minimum <= summary.median && summary.median <= summary.maximum,
    `${label} must satisfy minimum <= median <= maximum`,
  );
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

const physicalProfiles = loadJson(physicalProfilesPath);
const piProfile = physicalProfiles.profiles.raspberry_pi_5_champion;
const j4125Profile = physicalProfiles.profiles.j4125_calibrated;

function deriveHardwareProfile(runner) {
  const tree = runner.physicality?.device_tree;
  if (
    tree &&
    new RegExp(piProfile.device_tree.model_pattern).test(tree.model) &&
    piProfile.device_tree.required_compatible.every((item) =>
      tree.compatible.includes(item),
    )
  ) {
    return "raspberry_pi_5_champion";
  }
  if (new RegExp(j4125Profile.cpu_model_pattern, "i").test(runner.cpu_model)) {
    return "j4125_calibrated";
  }
  return "unclassified";
}

const canonicalProfiles = [
  "raspberry_pi_5_champion",
  "j4125_calibrated",
  "ugoos_am6b_plus",
  "xiaomi_box_m3",
  "nvidia_shield",
  "representative_tv",
];

const performanceGateOwner = "Ryan Winkler";

const packagingSpikeRequirements = {
  ugoos_am6b_plus: {
    model: "UGOOS AM6B Plus",
    os_target: "Android 9.0",
    firmware_target: null,
    memory: "4 GB LPDDR4",
    internal_storage: "32 GB eMMC",
    required_service_markers: ["UGOOS", "Google Play", "Cast", "voice"],
    authoritative_source_urls: [
      "https://ugoos.com/files/uploads/63a38e470077cb39a0f8ca6933db3cbb.pdf",
    ],
  },
  xiaomi_box_m3: {
    model: "Mi Box 3 MDZ-16-AB",
    os_target: "Factory Android TV",
    firmware_target: null,
    memory: "2 GB DDR3",
    internal_storage: "8 GB eMMC",
    required_service_markers: ["Xiaomi", "Google Play", "Cast", "voice"],
    authoritative_source_urls: [
      "https://ams-go.buy.mi.com/es/servicecenter/file/Mi_Box_es/?binaryId=11780&namespaceId=2&publicationId=18032",
      "https://www.mi.com/global/support/terms/declaration/",
    ],
  },
  nvidia_shield: {
    model: "NVIDIA SHIELD TV Pro (2019, 16 GB)",
    os_target: "Android 11",
    firmware_target: null,
    memory: "3 GB RAM",
    internal_storage: "16 GB",
    required_service_markers: [
      "NVIDIA",
      "Google Play",
      "Cast",
      "voice",
      "Plex",
    ],
    authoritative_source_urls: [
      "https://www.nvidia.com/en-gb/shield/shield-tv-pro/",
      "https://www.nvidia.com/en-eu/shield/support/shield-tv-pro/",
    ],
  },
  representative_tv: {
    model: "Sony BRAVIA 3 K-43S30 (2024)",
    os_target: "Google TV / Android TV",
    firmware_target: "6120800301",
    memory: "Record installed and available RAM at spike execution",
    internal_storage: "16 GB",
    required_service_markers: ["Sony", "Google Play", "Cast", "voice"],
    authoritative_source_urls: [
      "https://www.sony.com/electronics/support/televisions-projectors-lcd-tvs-android-/k-43s30/specifications",
      "https://www.sony.com/electronics/support/product/k-43s30/downloads",
    ],
  },
};

function runnerFingerprint(evidence) {
  const runner = evidence.runner;
  return {
    runner_id: runner.runner_id,
    machine_fingerprint_sha256: runner.machine_fingerprint_sha256,
    hardware_profile: runner.hardware_profile,
    hardware_profile_derivation: runner.hardware_profile_derivation,
    profile_policy_sha256: runner.profile_policy_sha256,
    physicality: runner.physicality,
    custodian: runner.custodian,
    os_release: runner.os_release,
    os_image: runner.os_image,
    kernel_release: runner.kernel_release,
    architecture: runner.architecture,
    cpu_model: runner.cpu_model,
    device_model: runner.device_model,
    logical_cpu_count: runner.logical_cpu_count,
    total_memory_bytes: runner.total_memory_bytes,
    firmware: runner.firmware,
    root_filesystem: runner.root_filesystem,
    storage: runner.storage,
    cpu_governor: runner.cpu_governor,
    temperature: runner.temperature,
    profile_requirements: runner.profile_requirements,
    cgroup_version: runner.cgroup_version,
    cgroup: runner.cgroup,
    container_engine: runner.container_engine,
  };
}

function artifactRef(evidence) {
  const source = evidence.source;
  return {
    git_commit: source.git_commit,
    git_tree: source.git_tree,
    native_fastid_sha256: source.native_fastid_sha256,
    oci_image_id: source.oci_image_id,
    oci_source_labels: source.oci_source_labels,
    build_recipe_path: source.build_recipe_path,
    build_recipe_sha256: source.build_recipe_sha256,
    build_context_archive_sha256: source.build_context_archive_sha256,
  };
}

function evidenceResults(evidence) {
  const budget_statuses = Object.fromEntries(
    evidence.budget_verdicts.map((verdict) => [verdict.budget, verdict.status]),
  );
  const artifact_budget_statuses = Object.fromEntries(
    evidence.artifact_budget_verdicts.map((verdict) => [
      verdict.budget,
      verdict.status,
    ]),
  );
  const idle_cpu_statuses = Object.fromEntries(
    evidence.idle_cpu_verdicts.map((verdict) => [
      verdict.scenario,
      verdict.status,
    ]),
  );
  const applicableStatuses = [
    ...Object.values(budget_statuses),
    ...Object.values(artifact_budget_statuses),
    ...Object.values(idle_cpu_statuses),
  ].filter((status) => status !== "not_applicable");
  return {
    budget_statuses,
    artifact_budget_statuses,
    idle_cpu_statuses,
    all_applicable_budgets_passed:
      applicableStatuses.length > 0 &&
      applicableStatuses.every((status) => status === "pass"),
  };
}

function calibrationSettings(evidence) {
  return {
    git_tree: evidence.source.git_tree,
    contract_ref: evidence.source.contract_ref,
    build_recipe_sha256: evidence.source.build_recipe_sha256,
    corpus: evidence.corpus,
    harness: {
      version: evidence.harness.version,
      repetitions: evidence.harness.repetitions,
      steady_window_seconds: evidence.harness.steady_window_seconds,
      idle_warmup_seconds: evidence.harness.idle_warmup_seconds,
      idle_measurement_seconds: evidence.harness.idle_measurement_seconds,
      sample_interval_ms: evidence.harness.sample_interval_ms,
      baseline_subtraction: evidence.harness.baseline_subtraction,
    },
    budget_snapshot: evidence.budget_snapshot,
    scenarios: evidence.scenarios.map((scenario) => ({
      id: scenario.id,
      subject: scenario.subject,
      measurement_scope: scenario.measurement_scope,
      network_required: scenario.network_denied.required,
      network_mechanism: scenario.network_denied.mechanism,
      workload_expectation: scenario.workload_exit.expectation,
    })),
  };
}

function measuredBudget(evidence, budget) {
  return evidence.budget_verdicts.find((verdict) => verdict.budget === budget)
    .measured_bytes;
}

function calibrationRelation(j4125, champion) {
  return {
    idle_target_ratio_j4125_to_champion:
      measuredBudget(j4125, "idle_target") /
      measuredBudget(champion, "idle_target"),
    absolute_ceiling_ratio_j4125_to_champion:
      measuredBudget(j4125, "absolute_ceiling") /
      measuredBudget(champion, "absolute_ceiling"),
  };
}

function assertJsonEqual(actual, expected, message) {
  assert(isDeepStrictEqual(actual, expected), message);
}

const placeholderPattern =
  /(?:^|[^a-z0-9])(tbd|todo|placeholder|unknown|unassigned|example)(?:$|[^a-z0-9])/i;

function assertNoPlaceholders(value, label) {
  if (typeof value === "string") {
    assert(
      !placeholderPattern.test(value) &&
        !["n/a", "na", "null", "runner", "custodian", "test"].includes(
          value.trim().toLowerCase(),
        ),
      `${label} contains a placeholder or generic value`,
    );
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((item, index) =>
      assertNoPlaceholders(item, `${label}[${index}]`),
    );
    return;
  }
  if (value !== null && typeof value === "object") {
    for (const [key, item] of Object.entries(value)) {
      if (item !== null) assertNoPlaceholders(item, `${label}.${key}`);
    }
  }
}

function validateRunnerEnvironment(runner, label) {
  const profilePolicySha256 = sha256(physicalProfilesPath);
  assert(
    runner.hardware_profile_derivation ===
      physicalProfiles.hardware_profile_derivation &&
      runner.profile_policy_sha256 === profilePolicySha256,
    `${label} runner is not bound to the canonical physical profile policy`,
  );
  assert(
    runner.root_filesystem.source === runner.storage.root_source &&
      runner.root_filesystem.type === runner.storage.root_filesystem_type &&
      isDeepStrictEqual(
        runner.root_filesystem.mount_options,
        runner.storage.root_mount_options,
      ),
    `${label} root filesystem does not correlate to the storage fingerprint`,
  );
  assert(
    runner.cpu_governor.cpu_count_observed === runner.logical_cpu_count,
    `${label} CPU governor fingerprint does not cover every logical CPU`,
  );
  assert(
    runner.temperature.preflight.source ===
      runner.temperature.post_capture.source &&
      runner.temperature.preflight.sensor ===
        runner.temperature.post_capture.sensor,
    `${label} thermal readings do not use the same sensor`,
  );
  assert(
    runner.cgroup.version === runner.cgroup_version,
    `${label} runner cgroup record does not match cgroup_version`,
  );

  if (runner.hardware_profile === "raspberry_pi_5_champion") {
    const requirements = runner.profile_requirements;
    assert(
      requirements?.profile === "raspberry_pi_5_champion" &&
        requirements.profile_policy_sha256 === profilePolicySha256 &&
        piProfile.architectures.includes(runner.architecture) &&
        runner.logical_cpu_count === piProfile.logical_cpu_count &&
        runner.total_memory_bytes >= piProfile.memory_bytes.minimum &&
        runner.total_memory_bytes <= piProfile.memory_bytes.maximum &&
        runner.os_image.id === piProfile.running_os_release.id &&
        runner.os_image.version_codename ===
          piProfile.running_os_release.version_codename &&
        isDeepStrictEqual(
          requirements.running_os_release,
          piProfile.running_os_release,
        ) &&
        requirements.retained_os_image_approval ===
          "approved_by_canonical_digest_policy" &&
        requirements.memory === piProfile.memory_bytes.label &&
        requirements.storage === piProfile.storage.label &&
        runner.storage.storage_class === piProfile.storage.class &&
        runner.storage.transport === piProfile.storage.transport &&
        runner.storage.usb_link_speed_mbps >=
          piProfile.storage.minimum_link_speed_mbps &&
        requirements.cooling?.status === "active" &&
        Array.isArray(requirements.cooling.fan_types) &&
        requirements.cooling.fan_types.length > 0 &&
        requirements.overclock?.status === piProfile.overclock.status &&
        requirements.overclock?.policy_sha256 === profilePolicySha256 &&
        runner.os_image.approval === "approved_by_canonical_digest_policy" &&
        piProfile.approved_image_sha256.length > 0 &&
        piProfile.approved_image_sha256.includes(
          runner.os_image.retained_image.sha256,
        ) &&
        runner.cgroup.oci_memory_limit_bytes ===
          piProfile.oci.memory_limit_bytes &&
        runner.cgroup.oci_swap_limit_bytes === piProfile.oci.swap_limit_bytes,
      `${label} does not satisfy the locked Raspberry Pi 5 champion profile`,
    );
  } else if (runner.hardware_profile === "j4125_calibrated") {
    const requirements = runner.profile_requirements;
    assert(
      requirements?.profile === "j4125_calibrated" &&
        requirements.profile_policy_sha256 === profilePolicySha256 &&
        j4125Profile.architectures.includes(runner.architecture) &&
        runner.logical_cpu_count === j4125Profile.logical_cpu_count &&
        requirements.cpu === "physical_j4125_four_core" &&
        requirements.storage === j4125Profile.storage.label &&
        requirements.retained_os_image_approval ===
          "retained_digest_recorded_no_profile_allowlist" &&
        requirements.oci_memory_limit_bytes ===
          j4125Profile.oci.memory_limit_bytes &&
        requirements.oci_swap_limit_bytes ===
          j4125Profile.oci.swap_limit_bytes &&
        runner.storage.storage_class === j4125Profile.storage.class &&
        j4125Profile.storage.accepted_transports.includes(
          runner.storage.transport,
        ) &&
        runner.os_image.approval ===
          "retained_digest_recorded_no_profile_allowlist" &&
        runner.cgroup.oci_memory_limit_bytes ===
          j4125Profile.oci.memory_limit_bytes &&
        runner.cgroup.oci_swap_limit_bytes ===
          j4125Profile.oci.swap_limit_bytes,
      `${label} does not satisfy the locked J4125 calibration profile`,
    );
  } else {
    assert(
      runner.profile_requirements === null &&
        runner.cgroup.oci_memory_limit_bytes === null &&
        runner.cgroup.oci_swap_limit_bytes === null,
      `${label} test fixture must not claim a physical profile configuration`,
    );
  }
}

function validateAssignedRunnerFingerprint(runner, custodian, label) {
  assertNoPlaceholders(
    { custodian, runner_fingerprint: runner },
    `${label} assignment`,
  );
  assert(
    runner.hardware_profile === deriveHardwareProfile(runner),
    `${label} assigned hardware profile is not derived from its fingerprint`,
  );
  validatePhysicality(runner, `${label} assigned fingerprint`);
  validateRunnerEnvironment(runner, `${label} assigned fingerprint`);
}

function validateAssignedArtifactRef(artifact, contractRef, label) {
  const labels = artifact.oci_source_labels;
  assert(
    labels["org.opencontainers.image.revision"] === artifact.git_commit &&
      labels["dev.scrobble.fasti.source.tree"] === artifact.git_tree &&
      labels["dev.scrobble.fasti.contracts"] === contractRef &&
      labels["dev.scrobble.fasti.build.recipe.sha256"] ===
        artifact.build_recipe_sha256 &&
      labels["dev.scrobble.fasti.build.context.archive.sha256"] ===
        artifact.build_context_archive_sha256,
    `${label} assigned artifact labels do not correlate to source and contract refs`,
  );
  assert(
    artifact.build_recipe_path === "benchmarks/b1/Dockerfile" &&
      artifact.build_recipe_sha256 === sha256(governedBuildRecipePath),
    `${label} assigned artifact does not bind the governed build recipe`,
  );
}

function validatePhysicality(runner, label) {
  const proof = runner.physicality;
  if (runner.hardware_profile === "unclassified") {
    assert(
      proof.status === "test_fixture" && proof.mechanism === "test_fixture",
      `${label} unclassified runner must remain explicit test-fixture evidence`,
    );
    return;
  }
  assert(
    proof.status === "physical" && !proof.cpu_hypervisor_flag,
    `${label} does not establish non-virtual physical hardware`,
  );
  if (runner.hardware_profile === "raspberry_pi_5_champion") {
    const tree = proof.device_tree;
    assert(
      proof.mechanism === "raspberry_pi_systemd_device_tree_cross_check" &&
        proof.systemd_detect_virt === piProfile.systemd_detect_virt &&
        proof.dmi === null &&
        tree !== null &&
        tree.model === runner.device_model &&
        new RegExp(piProfile.device_tree.model_pattern).test(tree.model) &&
        piProfile.device_tree.required_compatible.every((item) =>
          tree.compatible.includes(item),
        ),
      `${label} lacks exact physical Raspberry Pi 5 Model B and BCM2712 proof`,
    );
  }
  if (runner.hardware_profile === "j4125_calibrated") {
    const dmi = Object.values(proof.dmi ?? {})
      .join(" ")
      .toLowerCase();
    const virtualMarkers = [
      "bhyve",
      "bochs",
      "kvm",
      "microsoft corporation virtual machine",
      "parallels",
      "qemu",
      "virtualbox",
      "vmware",
      "xen",
    ];
    assert(
      proof.mechanism === "j4125_systemd_cpu_dmi_cross_check" &&
        proof.systemd_detect_virt === j4125Profile.systemd_detect_virt &&
        proof.device_tree === null &&
        proof.dmi?.sys_vendor &&
        proof.dmi?.product_name &&
        !virtualMarkers.some((marker) => dmi.includes(marker)),
      `${label} lacks fail-closed physical J4125 virtualization evidence`,
    );
  }
}

function fileEvidenceResolver(reference) {
  const evidenceRoot = resolve(here, "evidence");
  const path = resolve(here, reference.path);
  const snapshot = readContainedRegularFileOnce(
    evidenceRoot,
    path,
    "device evidence receipt",
  );
  return {
    evidence: JSON.parse(snapshot.bytes.toString("utf8")),
    digest: snapshot.digest,
    path,
  };
}

function correlateDeviceEvidence(device, reference, resolver, label) {
  const resolved = resolver(reference);
  assert(
    resolved !== undefined,
    `${label} evidence reference cannot be resolved`,
  );
  assert(
    resolved.digest === reference.sha256,
    `${label} evidence digest does not match its referenced bytes`,
  );
  validateEvidence(
    resolved.evidence,
    `${label} evidence`,
    false,
    resolved.path ?? null,
  );
  const evidence = resolved.evidence;
  assert(
    evidence.status === "complete",
    `${label} cannot qualify from test-fixture evidence`,
  );
  assert(
    evidence.runner.hardware_profile === device.profile,
    `${label} evidence hardware profile does not match the ledger profile`,
  );
  assert(
    evidence.runner.custodian === device.custodian,
    `${label} evidence custodian does not match the ledger assignment`,
  );
  assertJsonEqual(
    device.runner_fingerprint,
    runnerFingerprint(evidence),
    `${label} runner fingerprint does not match validated evidence`,
  );
  assertJsonEqual(
    device.artifact_ref,
    artifactRef(evidence),
    `${label} artifact reference does not match validated evidence`,
  );
  assert(
    device.contract_ref === evidence.source.contract_ref,
    `${label} contract reference does not match validated evidence`,
  );
  assertJsonEqual(
    device.results,
    evidenceResults(evidence),
    `${label} results do not match validated evidence`,
  );
  return evidence;
}

function validateLedgerDocument(ledger, resolver = fileEvidenceResolver) {
  assertSchema(
    schemaValidator(ledgerSchemaPath),
    ledger,
    "device-hypotheses.json",
  );
  const profiles = ledger.devices.map((device) => device.profile);
  assertJsonEqual(
    profiles,
    canonicalProfiles,
    "device hypotheses must appear once in canonical profile order",
  );
  assert(
    ledger.performance_gate_owner === performanceGateOwner,
    `performance gate owner must remain ${performanceGateOwner}`,
  );
  const devicesByProfile = Object.fromEntries(
    ledger.devices.map((device) => [device.profile, device]),
  );
  const expectedRoles = {
    raspberry_pi_5_champion: "champion",
    j4125_calibrated: "calibrated_secondary",
    ugoos_am6b_plus: "packaging_hypothesis",
    xiaomi_box_m3: "packaging_hypothesis",
    nvidia_shield: "packaging_hypothesis",
    representative_tv: "packaging_hypothesis",
  };

  for (const device of ledger.devices) {
    const label = `device ${device.profile}`;
    assert(
      device.role === expectedRoles[device.profile],
      `${label} has the wrong qualification role`,
    );
    const spikeRequirement = packagingSpikeRequirements[device.profile];
    if (device.role === "packaging_hypothesis") {
      assert(device.spike !== null, `${label} requires a packaging spike`);
      assert(
        device.spike.owner === ledger.performance_gate_owner &&
          isDeepStrictEqual(device.spike.phases, ["B4", "B8"]) &&
          device.spike.status === "documented_unverified",
        `${label} packaging spike ownership, phases, or unverified status changed`,
      );
      assertJsonEqual(
        {
          model: device.spike.target.model,
          os_target: device.spike.target.os_target,
          firmware_target: device.spike.target.firmware_target,
          memory: device.spike.storage_constraints.memory,
          internal_storage: device.spike.storage_constraints.internal_storage,
          authoritative_source_urls: device.spike.authoritative_source_urls,
        },
        {
          model: spikeRequirement.model,
          os_target: spikeRequirement.os_target,
          firmware_target: spikeRequirement.firmware_target,
          memory: spikeRequirement.memory,
          internal_storage: spikeRequirement.internal_storage,
          authoritative_source_urls: spikeRequirement.authoritative_source_urls,
        },
        `${label} packaging target, constraints, or authoritative sources changed`,
      );
      assert(
        device.spike.install_route.package_format === "locally signed APK" &&
          isDeepStrictEqual(device.spike.install_route.routes, [
            "ADB over USB",
            "device-local package installer",
          ]) &&
          device.spike.install_route.post_install_network_policy ===
            "network-denied",
        `${label} install route must remain local and network-denied after installation`,
      );
      assert(
        /record the exact installed/i.test(
          device.spike.target.runtime_capture_requirement,
        ),
        `${label} must capture the actual installed runtime rather than infer it from specifications`,
      );
      const services = device.spike.background_environment.services_to_capture;
      for (const marker of spikeRequirement.required_service_markers) {
        assert(
          services.some((service) => service.includes(marker)),
          `${label} background environment omits ${marker}`,
        );
      }
      assert(
        /resident-memory attribution/i.test(
          device.spike.background_environment.measurement_requirement,
        ) &&
          /do not assume/i.test(
            device.spike.background_environment.measurement_requirement,
          ),
        `${label} must measure background services without assuming absence`,
      );
    } else {
      assert(
        device.spike === null,
        `${label} physical qualification profile must not contain a packaging spike`,
      );
    }
    const facts = [
      device.custodian,
      device.runner_fingerprint,
      device.artifact_ref,
      device.contract_ref,
    ];
    const evidenceFacts = [device.evidence_ref, device.results];
    if (device.profile === "j4125_calibrated") {
      assert(
        device.calibration !== null,
        `${label} requires calibration state`,
      );
    } else {
      assert(
        device.calibration === null,
        `${label} must not claim calibration`,
      );
    }

    if (device.qualification_state === "blocking_unassigned") {
      assert(
        [...facts, ...evidenceFacts].every((value) => value === null) &&
          device.blocking_reason !== null,
        `${label} unassigned state must remain null-backed and explicitly blocking`,
      );
      if (device.calibration !== null) {
        assert(
          device.calibration.state === "not_started" &&
            device.calibration.method === null &&
            device.calibration.reference_evidence_ref === null &&
            device.calibration.measured_relation === null,
          `${label} cannot claim calibration before assignment`,
        );
      }
      continue;
    }

    assert(
      facts.every((value) => value !== null),
      `${label} assignment is missing custodian, fingerprint, artifact, or contract facts`,
    );
    assert(
      device.runner_fingerprint.hardware_profile === device.profile,
      `${label} assigned fingerprint has the wrong profile`,
    );
    validateAssignedRunnerFingerprint(
      device.runner_fingerprint,
      device.custodian,
      label,
    );
    validateAssignedArtifactRef(
      device.artifact_ref,
      device.contract_ref,
      label,
    );

    if (device.qualification_state === "assigned_pending_evidence") {
      assert(
        evidenceFacts.every((value) => value === null) &&
          device.blocking_reason !== null,
        `${label} pending evidence must remain blocking without evidence or results`,
      );
      continue;
    }

    assert(
      evidenceFacts.every((value) => value !== null),
      `${label} validated state requires evidence and results`,
    );
    const deviceEvidence = correlateDeviceEvidence(
      device,
      device.evidence_ref,
      resolver,
      label,
    );

    if (device.qualification_state === "evidence_validated") {
      assert(
        device.blocking_reason !== null,
        `${label} evidence-only state must explain why qualification remains blocked`,
      );
      continue;
    }

    assert(
      device.results.all_applicable_budgets_passed &&
        device.blocking_reason === null,
      `${label} qualification requires every applicable budget to pass and no blocker`,
    );
    if (device.qualification_state === "qualified") {
      assert(
        device.role !== "calibrated_secondary",
        `${label} secondary hardware must use calibrated state`,
      );
      continue;
    }

    assert(
      device.qualification_state === "calibrated" &&
        device.role === "calibrated_secondary" &&
        device.calibration.state === "validated" &&
        device.calibration.method !== null &&
        device.calibration.reference_evidence_ref !== null &&
        device.calibration.measured_relation !== null,
      `${label} calibrated state lacks method or champion reference evidence`,
    );
    const champion = devicesByProfile.raspberry_pi_5_champion;
    assert(
      champion.qualification_state === "qualified" &&
        isDeepStrictEqual(
          champion.evidence_ref,
          device.calibration.reference_evidence_ref,
        ),
      `${label} calibration must reference the ledger's qualified champion evidence`,
    );
    const reference = resolver(device.calibration.reference_evidence_ref);
    assert(
      reference !== undefined,
      `${label} champion reference evidence cannot be resolved`,
    );
    assert(
      reference.digest === device.calibration.reference_evidence_ref.sha256,
      `${label} champion reference evidence digest does not match`,
    );
    validateEvidence(
      reference.evidence,
      `${label} champion reference`,
      false,
      reference.path ?? null,
    );
    assert(
      reference.evidence.runner.hardware_profile ===
        "raspberry_pi_5_champion" &&
        evidenceResults(reference.evidence).all_applicable_budgets_passed,
      `${label} calibration reference is not qualifying Raspberry Pi 5 evidence`,
    );
    assertJsonEqual(
      calibrationSettings(deviceEvidence),
      calibrationSettings(reference.evidence),
      `${label} calibration evidence does not share git tree, contracts, harness, budgets, and scenario settings with champion evidence`,
    );
    const expectedRelation = calibrationRelation(
      deviceEvidence,
      reference.evidence,
    );
    assert(
      closeEnough(
        device.calibration.measured_relation
          .idle_target_ratio_j4125_to_champion,
        expectedRelation.idle_target_ratio_j4125_to_champion,
      ) &&
        closeEnough(
          device.calibration.measured_relation
            .absolute_ceiling_ratio_j4125_to_champion,
          expectedRelation.absolute_ceiling_ratio_j4125_to_champion,
        ),
      `${label} measured calibration relation is not derived from both receipts`,
    );
  }
}

function validateStaticFiles() {
  const budgets = loadJson(budgetsPath);
  const ledger = loadJson(ledgerPath);
  assertSchema(schemaValidator(budgetsSchemaPath), budgets, "budgets.json");
  assertSchema(
    schemaValidator(physicalProfilesSchemaPath),
    physicalProfiles,
    "physical-profiles.json",
  );
  assert(
    (piProfile.approved_image_sha256.length === 0 &&
      piProfile.os_image_policy_status ===
        "blocking_until_official_digest_pinned") ||
      (piProfile.approved_image_sha256.length > 0 &&
        piProfile.os_image_policy_status === "approved_digest_allowlist"),
    "Raspberry Pi image approval status does not match its exact digest allowlist",
  );
  validateLedgerDocument(ledger);

  return budgets;
}

function validateEvidence(
  evidence,
  label = "evidence",
  validateStatic = true,
  evidencePath = null,
) {
  const budgets = validateStatic
    ? validateStaticFiles()
    : loadJson(budgetsPath);
  assertSchema(schemaValidator(evidenceSchemaPath), evidence, label);

  if (evidence.status === "complete") {
    assertNoPlaceholders(evidence.runner, `${label}.runner`);
  }

  assert(
    evidence.runner.hardware_profile === deriveHardwareProfile(evidence.runner),
    `${label} hardware profile is not derived from its observed CPU/device fingerprint`,
  );
  validatePhysicality(evidence.runner, label);
  validateRunnerEnvironment(evidence.runner, label);
  assert(
    evidence.source.profile_policy_path ===
      "benchmarks/b1/physical-profiles.json" &&
      evidence.source.profile_policy_sha256 === sha256(physicalProfilesPath) &&
      evidence.runner.profile_policy_sha256 ===
        evidence.source.profile_policy_sha256,
    `${label} canonical physical-profile policy binding is stale or substituted`,
  );
  assert(
    evidence.source.oci_source_labels["org.opencontainers.image.revision"] ===
      evidence.source.git_commit &&
      evidence.source.oci_source_labels["dev.scrobble.fasti.source.tree"] ===
        evidence.source.git_tree &&
      evidence.source.oci_source_labels["dev.scrobble.fasti.contracts"] ===
        evidence.source.contract_ref &&
      evidence.source.oci_source_labels[
        "dev.scrobble.fasti.build.recipe.sha256"
      ] === evidence.source.build_recipe_sha256 &&
      evidence.source.oci_source_labels[
        "dev.scrobble.fasti.build.context.archive.sha256"
      ] === evidence.source.build_context_archive_sha256,
    `${label} OCI source labels do not bind the recorded commit, tree, and contracts`,
  );
  assert(
    evidence.source.build_recipe_path === "benchmarks/b1/Dockerfile" &&
      evidence.source.build_recipe_sha256 === sha256(governedBuildRecipePath),
    `${label} governed build-recipe digest is stale or substituted`,
  );
  assert(
    evidence.source.build_context.method ===
      "verifier_owned_git_archive_head" &&
      evidence.source.build_context.git_archive_sha256 ===
        evidence.source.build_context_archive_sha256,
    `${label} build context is not bound to verifier-owned exact HEAD archive bytes`,
  );
  const governedBuild = evidence.harness.governed_build_commands;
  const dockerBuilds = governedBuild.filter((command) =>
    command.startsWith("docker build "),
  );
  assert(
    governedBuild.some((command) => command.startsWith("git archive ")) &&
      dockerBuilds.length === 1 &&
      dockerBuilds[0].includes("benchmarks/b1/Dockerfile") &&
      [
        evidence.source.git_commit,
        evidence.source.git_tree,
        evidence.source.contract_ref,
        evidence.source.build_recipe_sha256,
        evidence.source.build_context_archive_sha256,
      ].every((value) => dockerBuilds[0].includes(value)) &&
      !dockerBuilds[0].endsWith(` ${resolve(here, "../..")}`),
    `${label} governed build command does not bind the recipe and source identities`,
  );
  assert(
    evidence.corpus.status === "not_applicable" &&
      evidence.corpus.seed === null &&
      evidence.corpus.digest === null &&
      /B1.*(?:empty-process|idle).*(?:no|not).*corpus/i.test(
        evidence.corpus.reason,
      ),
    `${label} must record the B1 corpus as explicitly not applicable`,
  );

  assertJsonEqual(
    evidence.budget_snapshot,
    canonicalBudgetSnapshot(budgets),
    `${label} budget snapshot differs from the full canonical budgets or has a stale digest`,
  );

  const scenarioIds = evidence.scenarios.map((scenario) => scenario.id);
  assert(
    isDeepStrictEqual(scenarioIds, expectedScenarioIds),
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
      assert(
        scenario.samples.every((sample) => sample.container_identity === null),
        `${scenario.id} must not claim a Docker container identity`,
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
      assert(
        scenario.samples.every(
          (sample) =>
            sample.container_identity !== null &&
            sample.container_identity.cgroup_path.includes(
              sample.container_identity.container_id,
            ),
        ),
        `${scenario.id} State.Pid is not correlated to its exact local container cgroup`,
      );
      assert(
        scenario.samples.every(
          (sample) =>
            sample.cgroup.memory_limit_bytes ===
              evidence.runner.cgroup.oci_memory_limit_bytes &&
            sample.cgroup.swap_limit_bytes ===
              evidence.runner.cgroup.oci_swap_limit_bytes,
        ),
        `${scenario.id} cgroup limits do not correlate to the runner profile`,
      );
    }

    for (const sample of scenario.samples) {
      const idleExpected =
        scenario.id === "native_fastid_idle" ||
        scenario.id === "oci_fastid_idle";
      assertSampleDerivedFromObservations(
        sample,
        scenario.id,
        ociScenarioIds.has(scenario.id),
        idleExpected,
        budgets,
      );
      assertOrderedSummary(
        sample.steady_process_tree_rss_statistics,
        `${scenario.id} steady process-tree observations`,
      );
      assert(
        sample.steady_process_tree_rss_bytes ===
          sample.steady_process_tree_rss_statistics.maximum,
        `${scenario.id} steady process-tree gate must use the maximum observation`,
      );
      assert(
        sample.steady_process_tree_rss_bytes <=
          sample.peak_process_tree_rss_bytes,
        `${scenario.id} steady process-tree RSS exceeds its peak`,
      );
      if (sample.cgroup !== null) {
        assertOrderedSummary(
          sample.cgroup.steady_memory_current_statistics,
          `${scenario.id} steady cgroup observations`,
        );
        assert(
          sample.cgroup.steady_memory_current_bytes ===
            sample.cgroup.steady_memory_current_statistics.maximum,
          `${scenario.id} steady cgroup gate must use the maximum observation`,
        );
        assert(
          sample.cgroup.steady_memory_current_bytes <=
            sample.cgroup.peak_memory_bytes,
          `${scenario.id} steady cgroup memory exceeds its peak`,
        );
      }
      if (!idleExpected) {
        assert(
          sample.idle_cpu === null,
          `${scenario.id} must not claim an idle CPU measurement`,
        );
      } else {
        const expectedScope = nativeScenarioIds.has(scenario.id)
          ? "native_process_tree_schedstat"
          : "cgroup_v2_usage_usec";
        assert(
          sample.idle_cpu !== null &&
            sample.idle_cpu.counter_scope === expectedScope &&
            sample.idle_cpu.measurement_seconds >=
              budgets.timing_seconds.idle_measurement,
          `${scenario.id} idle CPU measurement has the wrong scope or duration`,
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
  for (const id of ociScenarioIds) {
    const dockerRuns = byId[id].commands.filter((command) =>
      command.startsWith("docker run "),
    );
    assert(
      dockerRuns.length > 0 &&
        dockerRuns.every((command) =>
          command.includes(evidence.source.oci_image_id),
        ),
      `${id} was not run from the recorded immutable image ID`,
    );
  }
  const artifactDockerRuns = evidence.harness.artifact_size_commands.filter(
    (command) => command.startsWith("docker run "),
  );
  assert(
    artifactDockerRuns.length > 0 &&
      artifactDockerRuns.every((command) =>
        command.includes(evidence.source.oci_image_id),
      ),
    "artifact size probes were not run from the recorded immutable image ID",
  );
  const nativeCreates = evidence.harness.native_artifact_commands.filter(
    (command) => command.startsWith("docker create "),
  );
  assert(
    nativeCreates.length === 1 &&
      nativeCreates[0].includes(evidence.source.oci_image_id) &&
      evidence.harness.native_artifact_commands.some((command) =>
        command.startsWith("docker cp "),
      ) &&
      evidence.artifact_sizes.native_fastid_binary_bytes ===
        evidence.artifact_sizes.oci_fastid_binary_bytes,
    "native fastid was not extracted byte-for-byte from the recorded immutable image",
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
    isDeepStrictEqual(Object.keys(verdicts), [
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

  const artifactOrder = [
    "native_runtime_installed",
    "native_archive_compressed",
    "oci_image_compressed",
    "oci_image_unpacked",
    "contract_pack_compressed",
  ];
  assertJsonEqual(
    evidence.artifact_budget_verdicts.map((verdict) => verdict.budget),
    artifactOrder,
    "artifact budget verdicts must appear once in canonical order",
  );
  const artifactMeasurements = {
    native_runtime_installed:
      evidence.artifact_sizes.native_runtime_installed_bytes,
    native_archive_compressed:
      evidence.artifact_sizes.native_archive_compressed_bytes,
    oci_image_compressed: evidence.artifact_sizes.oci_image_compressed_bytes,
    oci_image_unpacked: evidence.artifact_sizes.oci_image_bytes,
    contract_pack_compressed:
      evidence.artifact_sizes.contract_pack_compressed_bytes,
  };
  if (evidence.status === "complete" && evidencePath !== null) {
    validateRetainedArtifacts(evidence, evidencePath, label);
  }
  for (const verdict of evidence.artifact_budget_verdicts) {
    const budget = verdict.budget;
    const measured = artifactMeasurements[budget];
    assert(
      verdict.limit_bytes === budgets.artifact_bytes[budget] &&
        verdict.measured_bytes === measured,
      `${budget} artifact verdict is not correlated to its budget and measured bytes`,
    );
    if (measured === null) {
      assert(
        verdict.status === "not_applicable" &&
          /B1.*(?:does not|benchmark-only)/i.test(verdict.reason),
        `${budget} must be explicitly not applicable in B1`,
      );
    } else {
      assert(
        verdict.status ===
          (measured <= budgets.artifact_bytes[budget] ? "pass" : "fail"),
        `${budget} artifact verdict is not derived from its limit`,
      );
    }
  }

  const idleCpuOrder = ["native_fastid_idle", "oci_fastid_idle"];
  assertJsonEqual(
    evidence.idle_cpu_verdicts.map((verdict) => verdict.scenario),
    idleCpuOrder,
    "idle CPU verdicts must appear once in canonical order",
  );
  for (const verdict of evidence.idle_cpu_verdicts) {
    const measurements = byId[verdict.scenario].samples.map(
      (sample) => sample.idle_cpu,
    );
    const worstAverage = Math.max(
      ...measurements.map(
        (measurement) => measurement.average_percent_one_core,
      ),
    );
    const worstP95 = Math.max(
      ...measurements.map((measurement) => measurement.p95_percent_one_core),
    );
    const expectedStatus =
      worstAverage <= budgets.idle_cpu_percent_one_core.average &&
      worstP95 <= budgets.idle_cpu_percent_one_core.p95
        ? "pass"
        : "fail";
    assert(
      verdict.warmup_seconds === budgets.timing_seconds.idle_warmup &&
        verdict.measurement_seconds ===
          budgets.timing_seconds.idle_measurement &&
        verdict.average_limit_percent_one_core ===
          budgets.idle_cpu_percent_one_core.average &&
        verdict.p95_limit_percent_one_core ===
          budgets.idle_cpu_percent_one_core.p95 &&
        closeEnough(
          verdict.measured_worst_average_percent_one_core,
          worstAverage,
        ) &&
        closeEnough(verdict.measured_worst_p95_percent_one_core, worstP95) &&
        verdict.status === expectedStatus,
      `${verdict.scenario} idle CPU verdict is not derived from the worst independent run`,
    );
  }
}

function fixtureSummary(samples, field) {
  return summarize(samples.map((sample) => sample[field]));
}

function makeSelfTestEvidence() {
  const budgets = loadJson(budgetsPath);
  const imageId = "sha256:" + "4".repeat(64);
  const buildRecipeSha256 = sha256(governedBuildRecipePath);
  const profilePolicySha256 = sha256(physicalProfilesPath);
  const buildContextArchiveSha256 = "e".repeat(64);
  const samples = (withCgroup, idle) =>
    [1, 2, 3, 4, 5].map((run) => {
      const steadyStart = idle ? 600_020_000_000 : 0;
      const elapsed = idle
        ? [
            0,
            steadyStart,
            steadyStart + 450_000_000_000,
            steadyStart + 900_000_000_000,
          ]
        : [0, 1_500_000_000, 3_000_000_000];
      const processRss = idle
        ? [9_000_000 + run, 7_000_000 + run, 7_500_000 + run, 8_000_000 + run]
        : [7_000_000 + run, 7_500_000 + run, 8_000_000 + run];
      const cgroupCurrent = idle
        ? [11_000_000 + run, 9_000_000 + run, 9_500_000 + run, 10_000_000 + run]
        : [9_000_000 + run, 9_500_000 + run, 10_000_000 + run];
      const observations = elapsed.map((elapsed_ns, index) => ({
        sequence: index + 1,
        elapsed_ns,
        steady: elapsed_ns >= steadyStart,
        process_tree_rss_bytes: processRss[index],
        process_tree_cpu_runtime_ns: run * 100_000 + index * 1_000_000,
        process_count: 1,
        cgroup_memory_current_bytes: withCgroup ? cgroupCurrent[index] : null,
        cgroup_memory_peak_bytes: withCgroup ? 11_000_000 + run : null,
        cgroup_cpu_runtime_ns: withCgroup
          ? run * 200_000 + index * 2_000_000
          : null,
        cgroup_memory_limit_bytes: null,
        cgroup_swap_limit_bytes: null,
        cgroup_oom_kill_count: withCgroup ? 0 : null,
      }));
      const steady = observations.filter((observation) => observation.steady);
      const final = observations.at(-1);
      const idleCounter = withCgroup
        ? "cgroup_cpu_runtime_ns"
        : "process_tree_cpu_runtime_ns";
      const intervals = steady.slice(1).map((observation, index) => {
        const previous = steady[index];
        return (
          ((observation[idleCounter] - previous[idleCounter]) /
            (observation.elapsed_ns - previous.elapsed_ns)) *
          100
        );
      });
      return {
        run,
        startup_ms: 10 + run,
        steady_process_tree_rss_bytes: Math.max(
          ...steady.map((observation) => observation.process_tree_rss_bytes),
        ),
        steady_process_tree_rss_statistics: summarize(
          steady.map((observation) => observation.process_tree_rss_bytes),
        ),
        peak_process_tree_rss_bytes: Math.max(...processRss),
        process_tree_cpu_seconds: round6(
          final.process_tree_cpu_runtime_ns / 1_000_000_000,
        ),
        process_tree_cpu_percent: round6(
          (final.process_tree_cpu_runtime_ns / final.elapsed_ns) * 100,
        ),
        process_count_peak: 1,
        steady_started_elapsed_ns: steadyStart,
        finished_elapsed_ns: final.elapsed_ns,
        observations,
        cgroup: withCgroup
          ? {
              steady_memory_current_bytes: Math.max(
                ...steady.map(
                  (observation) => observation.cgroup_memory_current_bytes,
                ),
              ),
              steady_memory_current_statistics: summarize(
                steady.map(
                  (observation) => observation.cgroup_memory_current_bytes,
                ),
              ),
              peak_memory_bytes: 11_000_000 + run,
              cpu_seconds: round6(final.cgroup_cpu_runtime_ns / 1_000_000_000),
              cpu_percent: round6(
                (final.cgroup_cpu_runtime_ns / final.elapsed_ns) * 100,
              ),
              memory_limit_bytes: null,
              swap_limit_bytes: null,
              oom_kill_count: 0,
            }
          : null,
        container_identity: withCgroup
          ? {
              container_id: "6".repeat(64),
              host_pid: 1234 + run,
              cgroup_path: `/sys/fs/cgroup/system.slice/docker-${"6".repeat(64)}.scope`,
            }
          : null,
        idle_cpu: idle
          ? {
              counter_scope: withCgroup
                ? "cgroup_v2_usage_usec"
                : "native_process_tree_schedstat",
              measurement_seconds:
                (steady.at(-1).elapsed_ns - steady[0].elapsed_ns) /
                1_000_000_000,
              average_percent_one_core: round6(
                ((steady.at(-1)[idleCounter] - steady[0][idleCounter]) /
                  (steady.at(-1).elapsed_ns - steady[0].elapsed_ns)) *
                  100,
              ),
              p95_percent_one_core: round6(p95NearestRank(intervals)),
              interval_count: intervals.length,
            }
          : null,
      };
    });

  const scenario = (id, withCgroup, guarded = false) => {
    const idle = id === "native_fastid_idle" || id === "oci_fastid_idle";
    const values = samples(withCgroup, idle);
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
      commands: withCgroup
        ? [`docker run --network none ${imageId} self-test`]
        : ["self-test fixture only"],
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

  const idleMeasured = 10_000_005;
  const absoluteMeasured = 11_000_005;
  return {
    $schema:
      "https://fasti.scrobble.dev/schemas/benchmarks/b1/evidence.schema.json",
    schema_version: "fasti.b1.performance-evidence.v3",
    body: "B1",
    status: "test_fixture",
    captured_at: "2026-08-22T00:00:00Z",
    runner: {
      runner_id: "fixture-ci-linux-01",
      machine_fingerprint_sha256: "a".repeat(64),
      hardware_profile: "unclassified",
      hardware_profile_derivation: physicalProfiles.hardware_profile_derivation,
      profile_policy_sha256: profilePolicySha256,
      physicality: {
        status: "test_fixture",
        mechanism: "test_fixture",
        systemd_detect_virt: null,
        cpu_hypervisor_flag: false,
        dmi: null,
        device_tree: null,
      },
      custodian: "Fasti fixture maintainer",
      os_release: "Fixture Linux 1",
      os_image: {
        pretty_name: "Fixture Linux 1",
        id: "fixture-linux",
        version_id: "1",
        version_codename: "fixture",
        build_id: null,
        image_id: null,
        image_version: null,
        claim_scope: "runtime_os_release_fields_only",
        retained_image: {
          file_name: "fixture.img",
          size_bytes: 1,
          sha256: "b".repeat(64),
        },
        approval: "retained_digest_recorded_no_profile_allowlist",
      },
      kernel_release: "6.12.1-fixture",
      architecture: "x86_64",
      cpu_model: "Synthetic CI CPU",
      device_model: null,
      logical_cpu_count: 1,
      total_memory_bytes: 1_073_741_824,
      firmware: {
        source: "/fixture/firmware",
        description: "Fixture firmware 1",
        sha256: "c".repeat(64),
      },
      root_filesystem: {
        source: "/dev/vda1",
        type: "ext4",
        mount_options: ["rw", "relatime"],
      },
      storage: {
        root_source: "/dev/vda1",
        root_filesystem_type: "ext4",
        root_mount_options: ["rw", "relatime"],
        physical_device: "/dev/vda",
        device_type: "disk",
        transport: "virtio",
        storage_class: "unknown_non_rotational",
        classification_evidence: ["lsblk.ROTA=0", "no_exact_ssd_marker"],
        rotational: false,
        size_bytes: 8_589_934_592,
        model: "Fixture Disk 1",
        usb_link_speed_mbps: null,
        identity_sha256: "d".repeat(64),
        raw_serial_recorded: false,
      },
      cpu_governor: {
        source: "/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor",
        observed: ["performance"],
        cpu_count_observed: 1,
      },
      temperature: {
        preflight: {
          source: "/sys/class/thermal/thermal_zone0/temp",
          sensor: "fixture-cpu",
          celsius: 40,
        },
        post_capture: {
          source: "/sys/class/thermal/thermal_zone0/temp",
          sensor: "fixture-cpu",
          celsius: 41,
        },
      },
      profile_requirements: null,
      cgroup_version: "v2",
      cgroup: {
        version: "v2",
        oci_memory_limit_bytes: null,
        oci_swap_limit_bytes: null,
      },
      container_engine: {
        name: "docker",
        version: "27.1.1",
        context: "fixture-local",
        endpoint: "unix:///fixture/docker.sock",
        socket_path: "/fixture/docker.sock",
        locality: "verified_local_unix_socket",
      },
    },
    source: {
      git_commit: "1".repeat(40),
      git_tree: "2".repeat(40),
      tree_state: "clean",
      native_fastid_sha256: "3".repeat(64),
      native_artifact_origin: "extracted_from_immutable_oci_image",
      oci_image_ref: "self-test:fixture",
      oci_image_id: imageId,
      oci_source_labels: {
        "org.opencontainers.image.revision": "1".repeat(40),
        "dev.scrobble.fasti.source.tree": "2".repeat(40),
        "dev.scrobble.fasti.contracts": "5".repeat(40),
        "dev.scrobble.fasti.build.recipe.sha256": buildRecipeSha256,
        "dev.scrobble.fasti.build.context.archive.sha256":
          buildContextArchiveSha256,
      },
      contract_ref: "5".repeat(40),
      build_recipe_path: "benchmarks/b1/Dockerfile",
      build_recipe_sha256: buildRecipeSha256,
      profile_policy_path: "benchmarks/b1/physical-profiles.json",
      profile_policy_sha256: profilePolicySha256,
      build_context_archive_sha256: buildContextArchiveSha256,
      build_context: {
        method: "verifier_owned_git_archive_head",
        git_archive_sha256: buildContextArchiveSha256,
        git_archive_size_bytes: 1,
        archive_command:
          "git archive --format=tar --output /verifier/context.tar HEAD",
        archive_entry_count: 1,
      },
    },
    corpus: {
      status: "not_applicable",
      seed: null,
      digest: null,
      reason:
        "B1 measures empty-process and idle feasibility baselines; no synthetic or provider corpus is loaded.",
    },
    budget_snapshot: canonicalBudgetSnapshot(budgets),
    harness: {
      version: "fasti-b1-benchmark.v3",
      repetitions: 5,
      steady_window_seconds: 3,
      idle_warmup_seconds: budgets.timing_seconds.idle_warmup,
      idle_measurement_seconds: budgets.timing_seconds.idle_measurement,
      sample_interval_ms: budgets.timing_seconds.sample_interval_ms,
      baseline_subtraction: false,
      capture_command: "python3 scripts/benchmark-b1.py --fixture",
      governed_build_commands: [
        "git archive --format=tar --output /verifier/context.tar HEAD",
        `docker build --file /verifier/context/benchmarks/b1/Dockerfile --build-arg FASTI_SOURCE_COMMIT=${"1".repeat(40)} --build-arg FASTI_SOURCE_TREE=${"2".repeat(40)} --build-arg FASTI_CONTRACT_REF=${"5".repeat(40)} --build-arg FASTI_BUILD_RECIPE_SHA256=${buildRecipeSha256} --build-arg FASTI_BUILD_CONTEXT_ARCHIVE_SHA256=${buildContextArchiveSha256} /verifier/context`,
      ],
      fingerprint_commands: ["self-test fixture only"],
      native_artifact_commands: [
        `docker create --network none ${imageId}`,
        "docker cp self-test:/usr/local/bin/fastid /tmp/fastid",
      ],
      artifact_size_commands: [
        `docker run --rm --network none ${imageId} self-test`,
      ],
      source_recheck_commands: ["self-test fixture only"],
    },
    scenarios,
    artifact_sizes: {
      native_fastid_binary_bytes: 1,
      oci_fastid_binary_bytes: 1,
      oci_fasti_cli_binary_bytes: 1,
      oci_image_bytes: 1,
      native_runtime_installed_bytes: null,
      native_archive_compressed_bytes: null,
      oci_image_compressed_bytes: 2,
      oci_image_compressed_sha256: "7".repeat(64),
      contract_pack_compressed_bytes: 3,
      contract_pack_compressed_sha256: "8".repeat(64),
    },
    retained_artifacts: {
      oci_image_compressed: {
        path: `artifacts/sha256/${"7".repeat(64)}.tar.gz`,
        sha256: "7".repeat(64),
        size_bytes: 2,
      },
      contract_pack_compressed: {
        path: `artifacts/sha256/${"8".repeat(64)}.tar.gz`,
        sha256: "8".repeat(64),
        size_bytes: 3,
      },
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
    artifact_budget_verdicts: [
      {
        budget: "native_runtime_installed",
        limit_bytes: budgets.artifact_bytes.native_runtime_installed,
        measured_bytes: null,
        status: "not_applicable",
        reason:
          "B1 uses a benchmark-only extraction and does not produce an installed native runtime.",
      },
      {
        budget: "native_archive_compressed",
        limit_bytes: budgets.artifact_bytes.native_archive_compressed,
        measured_bytes: null,
        status: "not_applicable",
        reason: "B1 does not produce a supported native archive.",
      },
      ...[
        ["oci_image_compressed", 2],
        ["oci_image_unpacked", 1],
        ["contract_pack_compressed", 3],
      ].map(([budget, measured_bytes]) => ({
        budget,
        limit_bytes: budgets.artifact_bytes[budget],
        measured_bytes,
        status: "pass",
        reason: `Fixture-derived ${budget} size.`,
      })),
    ],
    idle_cpu_verdicts: ["native_fastid_idle", "oci_fastid_idle"].map(
      (scenarioId) => {
        const measurements = scenarios
          .find((scenario) => scenario.id === scenarioId)
          .samples.map((sample) => sample.idle_cpu);
        return {
          scenario: scenarioId,
          warmup_seconds: budgets.timing_seconds.idle_warmup,
          measurement_seconds: budgets.timing_seconds.idle_measurement,
          average_limit_percent_one_core:
            budgets.idle_cpu_percent_one_core.average,
          p95_limit_percent_one_core: budgets.idle_cpu_percent_one_core.p95,
          measured_worst_average_percent_one_core: Math.max(
            ...measurements.map(
              (measurement) => measurement.average_percent_one_core,
            ),
          ),
          measured_worst_p95_percent_one_core: Math.max(
            ...measurements.map(
              (measurement) => measurement.p95_percent_one_core,
            ),
          ),
          status: "pass",
          reason:
            "Worst independent fixture run after the locked warm-up and idle window.",
        };
      },
    ),
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

function expectLedgerFailure(ledger, label) {
  try {
    validateLedgerDocument(ledger);
  } catch {
    return;
  }
  throw new Error(`invalid device ledger passed after removing ${label}`);
}

function validatePackagingSpikeRemovalSentinels() {
  const ledger = loadJson(ledgerPath);
  let sentinels = 0;

  const missingGateOwner = clone(ledger);
  delete missingGateOwner.performance_gate_owner;
  expectLedgerFailure(missingGateOwner, "performance_gate_owner");
  sentinels += 1;

  const requiredSpikePaths = [
    ["owner"],
    ["phases"],
    ["status"],
    ["target"],
    ["target", "model"],
    ["target", "os_target"],
    ["target", "firmware_target"],
    ["target", "runtime_capture_requirement"],
    ["install_route"],
    ["install_route", "package_format"],
    ["install_route", "routes"],
    ["install_route", "post_install_network_policy"],
    ["storage_constraints"],
    ["storage_constraints", "memory"],
    ["storage_constraints", "internal_storage"],
    ["storage_constraints", "expansion_and_capacity_requirement"],
    ["background_environment"],
    ["background_environment", "services_to_capture"],
    ["background_environment", "measurement_requirement"],
    ["authoritative_source_urls"],
  ];

  for (const device of ledger.devices.filter(
    (candidate) => candidate.role === "packaging_hypothesis",
  )) {
    const deviceIndex = ledger.devices.findIndex(
      (candidate) => candidate.profile === device.profile,
    );
    const missingSpike = clone(ledger);
    delete missingSpike.devices[deviceIndex].spike;
    expectLedgerFailure(missingSpike, `${device.profile}.spike`);
    sentinels += 1;

    for (const path of requiredSpikePaths) {
      const missingField = clone(ledger);
      let parent = missingField.devices[deviceIndex].spike;
      for (const segment of path.slice(0, -1)) parent = parent[segment];
      delete parent[path.at(-1)];
      expectLedgerFailure(
        missingField,
        `${device.profile}.spike.${path.join(".")}`,
      );
      sentinels += 1;
    }

    for (
      let sourceIndex = 0;
      sourceIndex < device.spike.authoritative_source_urls.length;
      sourceIndex += 1
    ) {
      const missingSource = clone(ledger);
      missingSource.devices[deviceIndex].spike.authoritative_source_urls.splice(
        sourceIndex,
        1,
      );
      expectLedgerFailure(
        missingSource,
        `${device.profile}.spike.authoritative_source_urls[${sourceIndex}]`,
      );
      sentinels += 1;
    }
  }

  return sentinels;
}

function fixtureReference(name, evidence) {
  return {
    path: `evidence/${name}.json`,
    sha256: createHash("sha256").update(JSON.stringify(evidence)).digest("hex"),
  };
}

function assignDeviceFromEvidence(device, evidence, evidence_ref, state) {
  device.qualification_state = state;
  device.custodian = evidence.runner.custodian;
  device.runner_fingerprint = runnerFingerprint(evidence);
  device.artifact_ref = artifactRef(evidence);
  device.contract_ref = evidence.source.contract_ref;
  device.evidence_ref = evidence_ref;
  device.results = evidenceResults(evidence);
  device.blocking_reason = null;
}

function configureJ4125Fixture(evidence, custodian) {
  evidence.status = "complete";
  evidence.runner.hardware_profile = "j4125_calibrated";
  evidence.runner.cpu_model = "Intel(R) Celeron(R) CPU J4125 @ 2.00GHz";
  evidence.runner.custodian = custodian;
  evidence.runner.architecture = j4125Profile.architectures[0];
  evidence.runner.logical_cpu_count = j4125Profile.logical_cpu_count;
  evidence.runner.cpu_governor.cpu_count_observed =
    j4125Profile.logical_cpu_count;
  evidence.runner.storage.transport =
    j4125Profile.storage.accepted_transports.find(
      (transport) => transport === "sata",
    ) ?? j4125Profile.storage.accepted_transports[0];
  evidence.runner.storage.storage_class = j4125Profile.storage.class;
  evidence.runner.storage.classification_evidence = [
    "lsblk.ROTA=0",
    "udev.ID_ATA_ROTATION_RATE_RPM=0",
  ];
  evidence.runner.os_image.approval =
    "retained_digest_recorded_no_profile_allowlist";
  evidence.runner.profile_requirements = {
    profile: "j4125_calibrated",
    profile_policy_sha256: sha256(physicalProfilesPath),
    cpu: "physical_j4125_four_core",
    retained_os_image_approval: "retained_digest_recorded_no_profile_allowlist",
    oci_memory_limit_bytes: j4125Profile.oci.memory_limit_bytes,
    oci_swap_limit_bytes: j4125Profile.oci.swap_limit_bytes,
    storage: j4125Profile.storage.label,
    mechanical_scope_note:
      "Fixture fingerprints only the benchmark root filesystem and no future data path.",
  };
  evidence.runner.cgroup.oci_memory_limit_bytes =
    j4125Profile.oci.memory_limit_bytes;
  evidence.runner.cgroup.oci_swap_limit_bytes =
    j4125Profile.oci.swap_limit_bytes;
  for (const scenario of evidence.scenarios.filter((candidate) =>
    ociScenarioIds.has(candidate.id),
  )) {
    for (const sample of scenario.samples) {
      sample.cgroup.memory_limit_bytes = j4125Profile.oci.memory_limit_bytes;
      sample.cgroup.swap_limit_bytes = j4125Profile.oci.swap_limit_bytes;
      for (const observation of sample.observations) {
        observation.cgroup_memory_limit_bytes =
          j4125Profile.oci.memory_limit_bytes;
        observation.cgroup_swap_limit_bytes = j4125Profile.oci.swap_limit_bytes;
      }
    }
  }
}

function makeJ4125SelfTestEvidence() {
  const evidence = makeSelfTestEvidence();
  configureJ4125Fixture(evidence, "J4125 lab custodian");
  evidence.runner.physicality = {
    status: "physical",
    mechanism: "j4125_systemd_cpu_dmi_cross_check",
    systemd_detect_virt: j4125Profile.systemd_detect_virt,
    cpu_hypervisor_flag: false,
    dmi: {
      sys_vendor: "Physical Lab Vendor",
      product_name: "J4125 Appliance",
    },
    device_tree: null,
  };
  return evidence;
}

function validateLedgerStateTransitions(validEvidence) {
  const piEvidence = clone(validEvidence);
  // The checked-in Pi image allowlist is intentionally empty until an official
  // digest is pinned. This in-memory-only digest exercises ledger mechanics
  // without turning a fixture digest into repository policy.
  piProfile.approved_image_sha256.push(
    piEvidence.runner.os_image.retained_image.sha256,
  );
  piEvidence.status = "complete";
  piEvidence.runner.hardware_profile = "raspberry_pi_5_champion";
  piEvidence.runner.device_model = "Raspberry Pi 5 Model B Rev 1.0";
  piEvidence.runner.custodian = "Pi lab custodian";
  piEvidence.runner.architecture = piProfile.architectures[0];
  piEvidence.runner.logical_cpu_count = piProfile.logical_cpu_count;
  piEvidence.runner.total_memory_bytes = 4_294_967_296;
  piEvidence.runner.os_image.id = piProfile.running_os_release.id;
  piEvidence.runner.os_image.version_codename =
    piProfile.running_os_release.version_codename;
  piEvidence.runner.storage.transport = piProfile.storage.transport;
  piEvidence.runner.storage.storage_class = piProfile.storage.class;
  piEvidence.runner.storage.classification_evidence = [
    "lsblk.ROTA=0",
    "udev.ID_ATA_ROTATION_RATE_RPM=0",
  ];
  piEvidence.runner.storage.usb_link_speed_mbps =
    piProfile.storage.minimum_link_speed_mbps;
  piEvidence.runner.os_image.approval = "approved_by_canonical_digest_policy";
  piEvidence.runner.cpu_governor.cpu_count_observed =
    piProfile.logical_cpu_count;
  piEvidence.runner.profile_requirements = {
    profile: "raspberry_pi_5_champion",
    profile_policy_sha256: sha256(physicalProfilesPath),
    running_os_release: piProfile.running_os_release,
    retained_os_image_approval: "approved_by_canonical_digest_policy",
    memory: piProfile.memory_bytes.label,
    storage: piProfile.storage.label,
    cooling: { status: piProfile.cooling_status, fan_types: ["pwm-fan"] },
    overclock: {
      status: piProfile.overclock.status,
      policy_sha256: sha256(physicalProfilesPath),
      checked_keys: [
        ...Object.keys(piProfile.overclock.allowed_exact_values),
        ...piProfile.overclock.forbidden_nonzero_prefixes,
      ].sort(),
    },
    mechanical_scope_note: "Fixture exercises every champion correlation.",
  };
  piEvidence.runner.physicality = {
    status: "physical",
    mechanism: "raspberry_pi_systemd_device_tree_cross_check",
    systemd_detect_virt: piProfile.systemd_detect_virt,
    cpu_hypervisor_flag: false,
    dmi: null,
    device_tree: {
      source: "/proc/device-tree",
      model: "Raspberry Pi 5 Model B Rev 1.0",
      compatible: ["raspberrypi,5-model-b", "brcm,bcm2712"],
    },
  };

  const j4125Evidence = makeJ4125SelfTestEvidence();

  validateEvidence(piEvidence, "assigned Pi fixture", false);
  validateEvidence(j4125Evidence, "assigned J4125 fixture", false);
  const piRef = fixtureReference("self-test-pi", piEvidence);
  const j4125Ref = fixtureReference("self-test-j4125", j4125Evidence);
  const fixtures = new Map([
    [piRef.path, { evidence: piEvidence, digest: piRef.sha256 }],
    [j4125Ref.path, { evidence: j4125Evidence, digest: j4125Ref.sha256 }],
  ]);
  const resolver = (reference) => fixtures.get(reference.path);

  const pendingLedger = loadJson(ledgerPath);
  const pendingPi = pendingLedger.devices[0];
  pendingPi.qualification_state = "assigned_pending_evidence";
  pendingPi.custodian = piEvidence.runner.custodian;
  pendingPi.runner_fingerprint = runnerFingerprint(piEvidence);
  pendingPi.artifact_ref = artifactRef(piEvidence);
  pendingPi.contract_ref = piEvidence.source.contract_ref;
  pendingPi.blocking_reason =
    "Assigned physical runner and artifacts still require a validated receipt.";
  validateLedgerDocument(pendingLedger, resolver);

  const mismatchedPendingArtifact = clone(pendingLedger);
  mismatchedPendingArtifact.devices[0].artifact_ref.oci_source_labels[
    "dev.scrobble.fasti.contracts"
  ] = "9".repeat(40);
  expectLedgerFailure(
    mismatchedPendingArtifact,
    "assigned-pending artifact correlation",
  );

  const ledger = loadJson(ledgerPath);
  const pi = ledger.devices[0];
  assignDeviceFromEvidence(pi, piEvidence, piRef, "qualified");
  const j4125 = ledger.devices[1];
  assignDeviceFromEvidence(j4125, j4125Evidence, j4125Ref, "calibrated");
  j4125.calibration = {
    reference_profile: "raspberry_pi_5_champion",
    state: "validated",
    method:
      "Same immutable B1 workload and budget contract measured on both named runners.",
    reference_evidence_ref: piRef,
    measured_relation: calibrationRelation(j4125Evidence, piEvidence),
  };
  validateLedgerDocument(ledger, resolver);

  const reorderedKeys = clone(ledger);
  for (const device of reorderedKeys.devices.slice(0, 2)) {
    device.runner_fingerprint = Object.fromEntries(
      Object.entries(device.runner_fingerprint).reverse(),
    );
    device.artifact_ref = Object.fromEntries(
      Object.entries(device.artifact_ref).reverse(),
    );
    device.results = Object.fromEntries(
      Object.entries(device.results).reverse(),
    );
    device.results.budget_statuses = Object.fromEntries(
      Object.entries(device.results.budget_statuses).reverse(),
    );
  }
  validateLedgerDocument(reorderedKeys, resolver);

  const mismatchedSettingsEvidence = clone(j4125Evidence);
  mismatchedSettingsEvidence.harness.steady_window_seconds = 4;
  const mismatchedSettingsRef = fixtureReference(
    "self-test-j4125-mismatched-settings",
    mismatchedSettingsEvidence,
  );
  const mismatchedSettingsLedger = clone(ledger);
  mismatchedSettingsLedger.devices[1].evidence_ref = mismatchedSettingsRef;
  const mismatchedSettingsResolver = (reference) => {
    if (reference.path === mismatchedSettingsRef.path) {
      return {
        evidence: mismatchedSettingsEvidence,
        digest: mismatchedSettingsRef.sha256,
      };
    }
    return fixtures.get(reference.path);
  };
  let settingsRejected = false;
  try {
    validateLedgerDocument(
      mismatchedSettingsLedger,
      mismatchedSettingsResolver,
    );
  } catch (error) {
    assert(
      error.message.includes("does not share git tree"),
      `unexpected calibration-settings failure: ${error.message}`,
    );
    settingsRejected = true;
  }
  assert(settingsRejected, "ledger accepted mismatched calibration settings");

  const mismatchedRelation = clone(ledger);
  mismatchedRelation.devices[1].calibration.measured_relation.idle_target_ratio_j4125_to_champion += 0.25;
  let relationRejected = false;
  try {
    validateLedgerDocument(mismatchedRelation, resolver);
  } catch (error) {
    assert(
      error.message.includes("measured calibration relation"),
      `unexpected calibration-relation failure: ${error.message}`,
    );
    relationRejected = true;
  }
  assert(relationRejected, "ledger accepted an invented calibration relation");

  const mismatchedArtifact = clone(ledger);
  mismatchedArtifact.devices[1].artifact_ref.native_fastid_sha256 = "9".repeat(
    64,
  );
  try {
    validateLedgerDocument(mismatchedArtifact, resolver);
  } catch (error) {
    assert(
      error.message.includes("artifact reference does not match"),
      `unexpected ledger-correlation failure: ${error.message}`,
    );
    piProfile.approved_image_sha256.pop();
    return;
  }
  piProfile.approved_image_sha256.pop();
  throw new Error(
    "ledger accepted an artifact reference unrelated to evidence",
  );
}

function validateContainedReceiptReaderSentinels() {
  const root = mkdtempSync(join(tmpdir(), "fasti-b1-receipt-reader-"));
  try {
    const receipt = join(root, "receipt.json");
    const openedReceipt = join(root, "opened-receipt.json");
    const original = Buffer.from('{"value":"descriptor-owned"}\n');
    writeFileSync(receipt, original, { mode: 0o600 });
    const snapshot = readContainedRegularFileOnce(
      root,
      receipt,
      "self-test receipt",
      {
        afterOpen: () => {
          renameSync(receipt, openedReceipt);
          writeFileSync(receipt, '{"value":"swapped-path"}\n', {
            mode: 0o600,
          });
        },
      },
    );
    assert(
      snapshot.bytes.equals(original) &&
        snapshot.digest === createHash("sha256").update(original).digest("hex"),
      "receipt reader did not derive JSON bytes and digest from one descriptor snapshot",
    );

    const linkedReceipt = join(root, "linked-receipt.json");
    symlinkSync(openedReceipt, linkedReceipt);
    let symlinkRejected = false;
    try {
      readContainedRegularFileOnce(root, linkedReceipt, "symlinked receipt");
    } catch {
      symlinkRejected = true;
    }
    assert(
      symlinkRejected,
      "ledger receipt reader followed a symlinked evidence path",
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function runSelfTest() {
  let missingNoFollowRejected = false;
  try {
    requireNoFollowSupport(undefined);
  } catch (error) {
    assert(
      error.message.includes("requires O_NOFOLLOW"),
      `unexpected O_NOFOLLOW sentinel failure: ${error.message}`,
    );
    missingNoFollowRejected = true;
  }
  assert(
    missingNoFollowRejected,
    "retained artifact verifier accepted a platform without O_NOFOLLOW",
  );
  const valid = makeSelfTestEvidence();
  validateEvidence(valid, "valid in-memory self-test fixture");
  validateContainedReceiptReaderSentinels();
  const packagingRemovalSentinels = validatePackagingSpikeRemovalSentinels();
  validateLedgerStateTransitions(valid);

  const missing = clone(valid);
  delete missing.scenarios[1].samples[0].peak_process_tree_rss_bytes;
  expectFailure(missing, "JSON Schema validation");

  const networked = clone(valid);
  networked.scenarios[3].network_denied.observed = false;
  expectFailure(networked, "JSON Schema validation");

  const staleBudget = clone(valid);
  staleBudget.budget_snapshot.memory_bytes.idle_target += 1;
  expectFailure(staleBudget, "budget snapshot differs");

  const staleArtifactBudget = clone(valid);
  staleArtifactBudget.budget_snapshot.artifact_bytes.oci_image_compressed += 1;
  expectFailure(staleArtifactBudget, "JSON Schema validation");

  const staleBudgetDigest = clone(valid);
  staleBudgetDigest.budget_snapshot.sha256 = "9".repeat(64);
  expectFailure(staleBudgetDigest, "stale digest");

  const missingWarmup = clone(valid);
  const idleSample = missingWarmup.scenarios.find(
    (scenario) => scenario.id === "native_fastid_idle",
  ).samples[0];
  idleSample.steady_started_elapsed_ns = Math.round(
    idleSample.startup_ms * 1_000_000,
  );
  for (const observation of idleSample.observations) {
    observation.steady =
      observation.elapsed_ns >= idleSample.steady_started_elapsed_ns;
  }
  expectFailure(missingWarmup, "locked idle warm-up");

  const inventedWorkload = clone(valid);
  inventedWorkload.budget_verdicts[1].status = "pass";
  inventedWorkload.budget_verdicts[1].measured_bytes = 1;
  expectFailure(inventedWorkload, "must not claim a B1 workload result");

  const assertedHardware = clone(valid);
  assertedHardware.runner.hardware_profile = "raspberry_pi_5_champion";
  expectFailure(assertedHardware, "JSON Schema validation");

  const virtualJ4125 = clone(valid);
  configureJ4125Fixture(virtualJ4125, "Virtualization sentinel custodian");
  virtualJ4125.runner.physicality = {
    status: "physical",
    mechanism: "j4125_systemd_cpu_dmi_cross_check",
    systemd_detect_virt: "kvm",
    cpu_hypervisor_flag: true,
    dmi: { sys_vendor: "QEMU", product_name: "Standard PC" },
    device_tree: null,
  };
  expectFailure(
    virtualJ4125,
    "does not establish non-virtual physical hardware",
  );

  const remoteDocker = clone(valid);
  remoteDocker.runner.container_engine.endpoint =
    "tcp://benchmark.example:2376";
  expectFailure(remoteDocker, "JSON Schema validation");

  const unrelatedStatePid = clone(valid);
  unrelatedStatePid.scenarios[2].samples[0].container_identity.cgroup_path = `/sys/fs/cgroup/system.slice/docker-${"7".repeat(64)}.scope`;
  expectFailure(
    unrelatedStatePid,
    "not correlated to its exact local container cgroup",
  );

  const staleImageLabel = clone(valid);
  staleImageLabel.source.oci_source_labels[
    "org.opencontainers.image.revision"
  ] = "9".repeat(40);
  expectFailure(staleImageLabel, "OCI source labels do not bind");

  const staleBuildLabel = clone(valid);
  staleBuildLabel.source.oci_source_labels[
    "dev.scrobble.fasti.build.recipe.sha256"
  ] = "9".repeat(64);
  expectFailure(staleBuildLabel, "OCI source labels do not bind");

  const substitutedBuildRecipe = clone(valid);
  substitutedBuildRecipe.source.build_recipe_sha256 = "9".repeat(64);
  substitutedBuildRecipe.source.oci_source_labels[
    "dev.scrobble.fasti.build.recipe.sha256"
  ] = "9".repeat(64);
  expectFailure(substitutedBuildRecipe, "build-recipe digest");

  const detachedBuildCommand = clone(valid);
  detachedBuildCommand.harness.governed_build_commands = [
    "docker build --file benchmarks/b1/Dockerfile .",
  ];
  expectFailure(detachedBuildCommand, "governed build command");

  const inventedCorpus = clone(valid);
  inventedCorpus.corpus.reason = "A corpus was loaded for this fixture.";
  expectFailure(inventedCorpus, "corpus as explicitly not applicable");

  const mutableImageRun = clone(valid);
  mutableImageRun.scenarios[3].commands = [
    "docker run --network none fasti:mutable self-test",
  ];
  expectFailure(mutableImageRun, "recorded immutable image ID");

  const medianIdleGate = clone(valid);
  medianIdleGate.scenarios[1].samples[0].steady_process_tree_rss_bytes =
    medianIdleGate.scenarios[1].samples[0].steady_process_tree_rss_statistics.median;
  expectFailure(medianIdleGate, "memory/process aggregates");

  const mutatedRawMemory = clone(valid);
  mutatedRawMemory.scenarios[1].samples[0].observations.at(
    -1,
  ).process_tree_rss_bytes += 1;
  expectFailure(mutatedRawMemory, "raw steady process-tree RSS");

  const duplicatedRawObservation = clone(valid);
  duplicatedRawObservation.scenarios[1].samples[0].observations[2].elapsed_ns =
    duplicatedRawObservation.scenarios[1].samples[0].observations[1].elapsed_ns;
  expectFailure(
    duplicatedRawObservation,
    "duplicate or non-monotonic timestamp",
  );

  assert(
    valid.scenarios[1].samples.every(
      (sample) =>
        sample.idle_cpu.p95_percent_one_core > 0 &&
        sample.idle_cpu.p95_percent_one_core < 1,
    ),
    "schedstat fixture does not exercise measurable sub-1% native CPU p95",
  );

  const reorderedScenarios = clone(valid);
  [reorderedScenarios.scenarios[0], reorderedScenarios.scenarios[1]] = [
    reorderedScenarios.scenarios[1],
    reorderedScenarios.scenarios[0],
  ];
  expectFailure(reorderedScenarios, "canonical order");

  const reorderedMemoryVerdicts = clone(valid);
  [
    reorderedMemoryVerdicts.budget_verdicts[0],
    reorderedMemoryVerdicts.budget_verdicts[1],
  ] = [
    reorderedMemoryVerdicts.budget_verdicts[1],
    reorderedMemoryVerdicts.budget_verdicts[0],
  ];
  expectFailure(reorderedMemoryVerdicts, "canonical order");

  const reorderedArtifactVerdicts = clone(valid);
  [
    reorderedArtifactVerdicts.artifact_budget_verdicts[2],
    reorderedArtifactVerdicts.artifact_budget_verdicts[3],
  ] = [
    reorderedArtifactVerdicts.artifact_budget_verdicts[3],
    reorderedArtifactVerdicts.artifact_budget_verdicts[2],
  ];
  expectFailure(reorderedArtifactVerdicts, "canonical order");

  const forgedArtifactMeasurement = clone(valid);
  forgedArtifactMeasurement.artifact_budget_verdicts[2].measured_bytes += 1;
  expectFailure(forgedArtifactMeasurement, "measured bytes");

  const forgedArtifactStatus = clone(valid);
  forgedArtifactStatus.artifact_budget_verdicts[2].status = "fail";
  expectFailure(forgedArtifactStatus, "not derived from its limit");

  const reorderedIdleCpuVerdicts = clone(valid);
  reorderedIdleCpuVerdicts.idle_cpu_verdicts.reverse();
  expectFailure(reorderedIdleCpuVerdicts, "canonical order");

  const forgedIdleCpuWorst = clone(valid);
  forgedIdleCpuWorst.idle_cpu_verdicts[0].measured_worst_p95_percent_one_core = 0.2;
  expectFailure(forgedIdleCpuWorst, "worst independent run");

  const idleCpuOnNonIdleScenario = clone(valid);
  idleCpuOnNonIdleScenario.scenarios[0].samples[0].idle_cpu = clone(
    valid.scenarios[1].samples[0].idle_cpu,
  );
  expectFailure(idleCpuOnNonIdleScenario, "JSON Schema validation");

  const placeholderEnvironment = clone(valid);
  configureJ4125Fixture(
    placeholderEnvironment,
    "Environment sentinel custodian",
  );
  placeholderEnvironment.runner.physicality = {
    status: "physical",
    mechanism: "j4125_systemd_cpu_dmi_cross_check",
    systemd_detect_virt: "none",
    cpu_hypervisor_flag: false,
    dmi: {
      sys_vendor: "Physical Lab Vendor",
      product_name: "J4125 Appliance",
    },
    device_tree: null,
  };
  placeholderEnvironment.runner.os_release = "unknown";
  expectFailure(placeholderEnvironment, "placeholder or generic value");

  const mismatchedJ4125Cgroup = clone(placeholderEnvironment);
  mismatchedJ4125Cgroup.runner.os_release = "Fixture Linux 1";
  mismatchedJ4125Cgroup.scenarios[3].samples[0].cgroup.memory_limit_bytes = 1;
  expectFailure(mismatchedJ4125Cgroup, "cgroup limits do not correlate");

  console.log(
    `PASS: static schemas, assignable device ledger, v3 evidence sentinels, and ${packagingRemovalSentinels} packaging-ledger removal sentinels`,
  );
}

const args = process.argv.slice(2);
if (args.length === 1 && args[0] === "--self-test") {
  runSelfTest();
} else if (args.length === 1 && args[0] === "--emit-j4125-test-fixture") {
  process.stdout.write(
    `${JSON.stringify(makeJ4125SelfTestEvidence(), null, 2)}\n`,
  );
} else if (args.length === 1 && args[0] === "--static") {
  validateStaticFiles();
  console.log("PASS: B1 benchmark budgets and device hypothesis ledger");
} else if (args.length === 1 && !args[0].startsWith("--")) {
  const path = resolve(args[0]);
  validateEvidence(loadJson(path), path, true, path);
  console.log(`PASS: ${path}`);
} else {
  console.error(
    "Usage: node benchmarks/b1/validate-evidence.mjs --static|--self-test|--emit-j4125-test-fixture|<evidence.json>",
  );
  process.exit(2);
}
