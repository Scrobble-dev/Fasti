export const deploymentModes = [
  "native",
  "podman",
  "docker",
  "trusted-proxy",
  "production",
] as const;

export type DeploymentMode = (typeof deploymentModes)[number];

export interface DeploymentInput {
  mode: DeploymentMode;
  port: number;
  dataRoot: string;
  publicUrl?: string;
}

export interface DeploymentPlan {
  mode: DeploymentMode;
  available: boolean;
  label: string;
  blockers: string[];
  warnings: string[];
  environment: Readonly<Record<string, string>>;
  command: readonly string[];
  verification: readonly string[];
  rollback: readonly string[];
}

const labels: Record<DeploymentMode, string> = {
  native: "Native loopback review",
  podman: "Podman loopback review",
  docker: "Docker loopback review",
  "trusted-proxy": "Trusted HTTPS proxy review",
  production: "Supported production deployment",
};

function commonBlockers(input: DeploymentInput): string[] {
  const blockers: string[] = [];
  if (
    !Number.isInteger(input.port) ||
    input.port < 1024 ||
    input.port > 65535
  ) {
    blockers.push("Port must be an integer from 1024 through 65535.");
  }
  if (!input.dataRoot.trim())
    blockers.push("Set an explicit private data root.");
  if (input.dataRoot.includes("\0"))
    blockers.push("Data root must not contain a null byte.");
  return blockers;
}

function proxyBlockers(input: DeploymentInput): string[] {
  try {
    const url = new URL(input.publicUrl ?? "");
    return url.protocol === "https:" &&
      url.username === "" &&
      url.password === ""
      ? []
      : ["Public URL must be an absolute HTTPS URL without credentials."];
  } catch {
    return ["Public URL must be an absolute HTTPS URL without credentials."];
  }
}

export function createDeploymentPlan(input: DeploymentInput): DeploymentPlan {
  if (!deploymentModes.includes(input.mode))
    throw new TypeError("Unknown deployment mode.");
  const blockers = commonBlockers(input);
  const environment: Record<string, string> = {};
  let command: string[] = [];

  if (input.mode === "production") {
    return {
      mode: input.mode,
      available: false,
      label: labels[input.mode],
      blockers: [
        "Fasti has no supported public release or production deployment profile.",
      ],
      warnings: [
        "Use a bounded local review mode. Do not treat review evidence as release support.",
      ],
      environment,
      command,
      verification: [],
      rollback: [],
    };
  }

  if (input.mode === "native") {
    Object.assign(environment, {
      FASTI_LISTEN: `127.0.0.1:${input.port}`,
      FASTI_DATA_ROOT: input.dataRoot,
      FASTI_PORT_FALLBACK: "fail",
    });
    command = ["cargo", "run", "--locked", "-p", "fastid"];
  } else if (input.mode === "podman" || input.mode === "docker") {
    command = [
      input.mode,
      "run",
      "--rm",
      "--name",
      "fasti-review",
      "--publish",
      `127.0.0.1:${input.port}:8420`,
      "--volume",
      `${input.dataRoot}:/data:Z`,
      "--env",
      "FASTI_DATA_ROOT=/data",
      "fasti:b0",
    ];
  } else {
    blockers.push(...proxyBlockers(input));
    Object.assign(environment, {
      FASTI_LISTEN: `0.0.0.0:${input.port}`,
      FASTI_DATA_ROOT: input.dataRoot,
      FASTI_REMOTE_TRUSTED_PROXY: "true",
      FASTI_PUBLIC_URL: input.publicUrl ?? "",
      FASTI_PORT_FALLBACK: "fail",
    });
    command = ["cargo", "run", "--locked", "-p", "fastid"];
  }

  return {
    mode: input.mode,
    available: blockers.length === 0,
    label: labels[input.mode],
    blockers,
    warnings: [
      "This plan is for review. It is not a supported production profile.",
      "Keep initialization proofs and credentials out of URLs, browser storage, commands, and reports.",
    ],
    environment,
    command,
    verification: [
      `curl --fail --show-error http://127.0.0.1:${input.port}/health`,
      "Confirm the capability response before using any durable route.",
    ],
    rollback: [
      "Stop only the process or container started for this review.",
      "Keep the data root unless its owner explicitly authorizes removal.",
    ],
  };
}

export function renderPosixCommand(plan: DeploymentPlan): string {
  if (!plan.available) return "";
  const quote = (value: string) => `'${value.replaceAll("'", `'"'"'`)}'`;
  const environment = Object.entries(plan.environment).map(
    ([key, value]) => `${key}=${quote(value)}`,
  );
  return [...environment, ...plan.command.map(quote)].join(" ");
}
