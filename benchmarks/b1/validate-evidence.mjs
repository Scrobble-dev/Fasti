#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
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

function deriveHardwareProfile(runner) {
  const cpu = runner.cpu_model.toLowerCase();
  const device = (runner.device_model ?? "").toLowerCase();
  if (device.includes("raspberry pi 5")) return "raspberry_pi_5_champion";
  if (/\bj4125\b/.test(cpu)) return "j4125_calibrated";
  if (device.includes("ugoos") && device.includes("am6b")) {
    return "ugoos_am6b_plus";
  }
  if (
    device.includes("xiaomi") &&
    ["mi box 3", "mibox3", "mdz-16-ab", "mdz-19-aa"].some((marker) =>
      device.includes(marker),
    )
  ) {
    return "xiaomi_box_m3";
  }
  if (device.includes("nvidia") && device.includes("shield")) {
    return "nvidia_shield";
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
    hardware_profile: runner.hardware_profile,
    hardware_profile_derivation: runner.hardware_profile_derivation,
    physicality: runner.physicality,
    os_release: runner.os_release,
    kernel_release: runner.kernel_release,
    architecture: runner.architecture,
    cpu_model: runner.cpu_model,
    device_model: runner.device_model,
    logical_cpu_count: runner.logical_cpu_count,
    total_memory_bytes: runner.total_memory_bytes,
  };
}

function artifactRef(evidence) {
  const source = evidence.source;
  return {
    git_commit: source.git_commit,
    git_tree: source.git_tree,
    native_fastid_sha256: source.native_fastid_sha256,
    oci_image_id: source.oci_image_id,
  };
}

function evidenceResults(evidence) {
  const budget_statuses = Object.fromEntries(
    evidence.budget_verdicts.map((verdict) => [verdict.budget, verdict.status]),
  );
  return {
    budget_statuses,
    all_applicable_budgets_passed:
      budget_statuses.idle_target === "pass" &&
      budget_statuses.absolute_ceiling === "pass",
  };
}

function calibrationSettings(evidence) {
  return {
    git_tree: evidence.source.git_tree,
    contract_ref: evidence.source.contract_ref,
    harness: {
      version: evidence.harness.version,
      repetitions: evidence.harness.repetitions,
      steady_window_seconds: evidence.harness.steady_window_seconds,
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
    assert(
      proof.mechanism === "raspberry_pi_device_tree" &&
        runner.device_model?.toLowerCase().includes("raspberry pi 5") &&
        proof.systemd_detect_virt === null &&
        proof.dmi === null,
      `${label} lacks Raspberry Pi 5 device-tree physicality proof`,
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
        proof.systemd_detect_virt === "none" &&
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
  assert(
    path.startsWith(`${evidenceRoot}/`),
    `device evidence path escapes benchmarks/b1/evidence: ${reference.path}`,
  );
  return { evidence: loadJson(path), digest: sha256(path) };
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
  validateEvidence(resolved.evidence, `${label} evidence`, false);
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
    validateEvidence(reference.evidence, `${label} champion reference`, false);
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
  validateLedgerDocument(ledger);

  return budgets;
}

function validateEvidence(evidence, label = "evidence", validateStatic = true) {
  const budgets = validateStatic
    ? validateStaticFiles()
    : loadJson(budgetsPath);
  assertSchema(schemaValidator(evidenceSchemaPath), evidence, label);

  assert(
    evidence.runner.hardware_profile === deriveHardwareProfile(evidence.runner),
    `${label} hardware profile is not derived from its observed CPU/device fingerprint`,
  );
  validatePhysicality(evidence.runner, label);
  assert(
    evidence.source.oci_source_labels["org.opencontainers.image.revision"] ===
      evidence.source.git_commit &&
      evidence.source.oci_source_labels["dev.scrobble.fasti.source.tree"] ===
        evidence.source.git_tree &&
      evidence.source.oci_source_labels["dev.scrobble.fasti.contracts"] ===
        evidence.source.contract_ref,
    `${label} OCI source labels do not bind the recorded commit, tree, and contracts`,
  );

  assert(
    isDeepStrictEqual(
      evidence.budget_snapshot.memory_bytes,
      budgets.memory_bytes,
    ),
    `${label} budget snapshot differs from the canonical budgets`,
  );
  assert(
    evidence.budget_snapshot.sha256 === sha256(budgetsPath),
    `${label} budget digest is stale`,
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
    }

    for (const sample of scenario.samples) {
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
}

function fixtureSummary(samples, field) {
  return summarize(samples.map((sample) => sample[field]));
}

function makeSelfTestEvidence() {
  const budgets = loadJson(budgetsPath);
  const imageId = "sha256:" + "4".repeat(64);
  const samples = (withCgroup) =>
    [1, 2, 3].map((run) => ({
      run,
      startup_ms: 10 + run,
      steady_process_tree_rss_bytes: 8_000_000 + run,
      steady_process_tree_rss_statistics: {
        minimum: 7_000_000 + run,
        median: 7_500_000 + run,
        maximum: 8_000_000 + run,
      },
      peak_process_tree_rss_bytes: 9_000_000 + run,
      process_tree_cpu_seconds: 0.01 * run,
      process_tree_cpu_percent: 0.5 * run,
      process_count_peak: 1,
      cgroup: withCgroup
        ? {
            steady_memory_current_bytes: 10_000_000 + run,
            steady_memory_current_statistics: {
              minimum: 9_000_000 + run,
              median: 9_500_000 + run,
              maximum: 10_000_000 + run,
            },
            peak_memory_bytes: 11_000_000 + run,
            cpu_seconds: 0.02 * run,
            cpu_percent: 0.75 * run,
          }
        : null,
      container_identity: withCgroup
        ? {
            container_id: "6".repeat(64),
            host_pid: 1234 + run,
            cgroup_path: `/sys/fs/cgroup/system.slice/docker-${"6".repeat(64)}.scope`,
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

  const idleMeasured = 10_000_003;
  const absoluteMeasured = 11_000_003;
  return {
    $schema:
      "https://fasti.scrobble.dev/schemas/benchmarks/b1/evidence.schema.json",
    schema_version: "fasti.b1.performance-evidence.v2",
    body: "B1",
    status: "test_fixture",
    captured_at: "2026-08-22T00:00:00Z",
    runner: {
      runner_id: "self-test-fixture",
      hardware_profile: "unclassified",
      hardware_profile_derivation: "fingerprint_rule_v1",
      physicality: {
        status: "test_fixture",
        mechanism: "test_fixture",
        systemd_detect_virt: null,
        cpu_hypervisor_flag: false,
        dmi: null,
      },
      custodian: "self-test-fixture",
      os_release: "self-test Linux",
      kernel_release: "self-test",
      architecture: "self-test",
      cpu_model: "self-test",
      device_model: null,
      logical_cpu_count: 1,
      total_memory_bytes: 1,
      cgroup_version: "v2",
      container_engine: {
        name: "docker",
        version: "self-test",
        context: "self-test",
        endpoint: "unix:///self-test/docker.sock",
        socket_path: "/self-test/docker.sock",
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
      },
      contract_ref: "5".repeat(40),
    },
    budget_snapshot: {
      source: "benchmarks/b1/budgets.json",
      sha256: sha256(budgetsPath),
      memory_bytes: budgets.memory_bytes,
    },
    harness: {
      version: "fasti-b1-benchmark.v2",
      repetitions: 3,
      steady_window_seconds: 3,
      sample_interval_ms: 10,
      baseline_subtraction: false,
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

function validateLedgerStateTransitions(validEvidence) {
  const piEvidence = clone(validEvidence);
  piEvidence.status = "complete";
  piEvidence.runner.hardware_profile = "raspberry_pi_5_champion";
  piEvidence.runner.device_model = "Raspberry Pi 5 Model B Rev 1.0";
  piEvidence.runner.custodian = "pi-custodian";
  piEvidence.runner.physicality = {
    status: "physical",
    mechanism: "raspberry_pi_device_tree",
    systemd_detect_virt: null,
    cpu_hypervisor_flag: false,
    dmi: null,
  };

  const j4125Evidence = clone(validEvidence);
  j4125Evidence.status = "complete";
  j4125Evidence.runner.hardware_profile = "j4125_calibrated";
  j4125Evidence.runner.cpu_model = "Intel(R) Celeron(R) CPU J4125 @ 2.00GHz";
  j4125Evidence.runner.custodian = "j4125-custodian";
  j4125Evidence.runner.physicality = {
    status: "physical",
    mechanism: "j4125_systemd_cpu_dmi_cross_check",
    systemd_detect_virt: "none",
    cpu_hypervisor_flag: false,
    dmi: {
      sys_vendor: "Physical Lab Vendor",
      product_name: "J4125 Appliance",
    },
  };

  validateEvidence(piEvidence, "assigned Pi fixture", false);
  validateEvidence(j4125Evidence, "assigned J4125 fixture", false);
  const piRef = fixtureReference("self-test-pi", piEvidence);
  const j4125Ref = fixtureReference("self-test-j4125", j4125Evidence);
  const fixtures = new Map([
    [piRef.path, { evidence: piEvidence, digest: piRef.sha256 }],
    [j4125Ref.path, { evidence: j4125Evidence, digest: j4125Ref.sha256 }],
  ]);
  const resolver = (reference) => fixtures.get(reference.path);

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
  mismatchedSettingsEvidence.harness.sample_interval_ms = 20;
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
    return;
  }
  throw new Error(
    "ledger accepted an artifact reference unrelated to evidence",
  );
}

function runSelfTest() {
  const valid = makeSelfTestEvidence();
  validateEvidence(valid, "valid in-memory self-test fixture");
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

  const inventedWorkload = clone(valid);
  inventedWorkload.budget_verdicts[1].status = "pass";
  inventedWorkload.budget_verdicts[1].measured_bytes = 1;
  expectFailure(inventedWorkload, "must not claim a B1 workload result");

  const assertedHardware = clone(valid);
  assertedHardware.runner.hardware_profile = "raspberry_pi_5_champion";
  expectFailure(assertedHardware, "JSON Schema validation");

  const virtualJ4125 = clone(valid);
  virtualJ4125.status = "complete";
  virtualJ4125.runner.hardware_profile = "j4125_calibrated";
  virtualJ4125.runner.cpu_model = "Intel Celeron J4125";
  virtualJ4125.runner.physicality = {
    status: "physical",
    mechanism: "j4125_systemd_cpu_dmi_cross_check",
    systemd_detect_virt: "kvm",
    cpu_hypervisor_flag: true,
    dmi: { sys_vendor: "QEMU", product_name: "Standard PC" },
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

  const mutableImageRun = clone(valid);
  mutableImageRun.scenarios[3].commands = [
    "docker run --network none fasti:mutable self-test",
  ];
  expectFailure(mutableImageRun, "recorded immutable image ID");

  const medianIdleGate = clone(valid);
  medianIdleGate.scenarios[1].samples[0].steady_process_tree_rss_bytes =
    medianIdleGate.scenarios[1].samples[0].steady_process_tree_rss_statistics.median;
  expectFailure(medianIdleGate, "gate must use the maximum observation");

  console.log(
    `PASS: static schemas, assignable device ledger, evidence semantics, twelve evidence sentinels, and ${packagingRemovalSentinels} packaging-ledger removal sentinels`,
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
