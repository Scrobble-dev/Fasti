#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";

const here = dirname(fileURLToPath(import.meta.url));
const schema = JSON.parse(
  readFileSync(join(here, "runner-bundle.schema.json"), "utf8"),
);
const ajv = new Ajv2020({ allErrors: true, strict: true });
addFormats(ajv);
const validateSchema = ajv.compile(schema);

export function validateManifest(manifest) {
  if (!validateSchema(manifest)) {
    throw new Error(ajv.errorsText(validateSchema.errors, { separator: "\n" }));
  }
  return manifest;
}

function fixture() {
  return {
    $schema:
      "https://fasti.scrobble.dev/schemas/benchmarks/b1/runner-bundle.schema.json",
    schema_version: "fasti.b1.runner-bundle.v1",
    created_at: "2026-08-22T00:00:00Z",
    source: {
      git_commit: "1".repeat(40),
      git_tree: "2".repeat(40),
      contract_ref: "3".repeat(40),
      tree_state: "clean",
    },
    bundle: {
      filename: "fasti-b1.bundle",
      sha256: "4".repeat(64),
      size_bytes: 1,
      head_ref: "HEAD",
    },
    handoff: {
      checkout_mode: "detached_exact_commit",
      public_remote_required: false,
      bundle_scope: "self_contained_objects_reachable_from_exact_head_only",
    },
  };
}

function selfTest() {
  const valid = fixture();
  validateManifest(valid);
  const mutation = structuredClone(valid);
  mutation.source.git_tree = "invented";
  try {
    validateManifest(mutation);
  } catch {
    console.log("PASS: canonical private runner bundle schema sentinel");
    return;
  }
  throw new Error("invalid runner bundle manifest passed validation");
}

if (process.argv[2] === "--self-test") {
  selfTest();
} else if (process.argv[2] === "--stdin") {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  validateManifest(JSON.parse(Buffer.concat(chunks).toString("utf8")));
  console.log("PASS: canonical runner bundle manifest");
} else if (process.argv.length === 3) {
  validateManifest(JSON.parse(readFileSync(process.argv[2], "utf8")));
  console.log(`PASS: canonical runner bundle manifest ${process.argv[2]}`);
} else {
  console.error(
    "usage: validate-runner-bundle.mjs --self-test | --stdin | <manifest.json>",
  );
  process.exit(2);
}
