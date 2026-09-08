import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = fileURLToPath(new URL("../../", import.meta.url));

for (const handshake of ["empty", "missing-directory"]) {
  test(`envelope discovery fails closed with ${handshake} handshake diagnostics`, () => {
    const directory = mkdtempSync(join(tmpdir(), "fasti-envelope-diagnostic-"));
    try {
      // Selection probes succeed, but the scope stub never executes its argv.
      writeFileSync(
        join(directory, "systemd-run"),
        `#!/bin/bash
set -eu
if [[ "\${!#}" == true ]]; then exit 0; fi
handshake_file=""
for argument in "$@"; do
  case "$argument" in "$TMPDIR"/*/cgroup) handshake_file="$argument" ;; esac
done
[[ -n "$handshake_file" ]] || exit 97
if [[ "$FASTI_DIAGNOSTIC_FIXTURE" == missing-directory ]]; then
  printf '%s\\n' "$TMPDIR/absent-cgroup" > "$handshake_file"
fi
exit 23
`,
        { mode: 0o700 },
      );
      writeFileSync(join(directory, "unshare"), "#!/bin/bash\nexit 0\n", {
        mode: 0o700,
      });
      // Never permit a failed selection probe to reach the real privileged path.
      writeFileSync(join(directory, "sudo"), "#!/bin/bash\nexit 98\n", {
        mode: 0o700,
      });
      const workloadMarker = join(directory, "workload-must-not-run");
      const result = spawnSync(
        "bash",
        [
          "scripts/bench-envelope.sh",
          "--target",
          "idle",
          "--profile",
          "canonical-idle",
          "--",
          "touch",
          workloadMarker,
        ],
        {
          cwd: root,
          env: {
            ...process.env,
            PATH: `${directory}:${process.env.PATH}`,
            TMPDIR: directory,
            FASTI_DIAGNOSTIC_FIXTURE: handshake,
          },
          encoding: "utf8",
          timeout: 5_000,
        },
      );
      assert.ifError(result.error);
      assert.equal(result.status, 1, result.stderr);
      assert.match(result.stderr, /could not discover the enforced cgroup/u);
      assert.match(
        result.stderr,
        /discovery: runner=user elapsed_seconds=\d+/u,
      );
      assert.match(result.stderr, /cgroup_directory=absent/u);
      assert.match(result.stderr, /Canonical idle runner exit status: 23/u);
      assert.match(result.stderr, /no passing measurement is available/u);
      if (handshake === "empty") {
        assert.match(result.stderr, /handshake: bytes=0 path=''/u);
        assert.match(result.stderr, /runner_at_discovery=exited/u);
      } else {
        const path = join(directory, "absent-cgroup");
        assert.ok(
          result.stderr.includes(
            `handshake: bytes=${Buffer.byteLength(path) + 1} path=${path}`,
          ),
          result.stderr,
        );
        // Publication and process exit can race; either observed state is valid.
        assert.match(result.stderr, /runner_at_discovery=(?:alive|exited)/u);
      }
      assert.equal(existsSync(workloadMarker), false);
      assert.doesNotMatch(result.stdout + result.stderr, /PASS/u);
      assert.deepEqual(readdirSync(directory).sort(), [
        "sudo",
        "systemd-run",
        "unshare",
      ]);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });
}

test("envelope self-test retains the first 8 KiB of failure diagnostics", () => {
  const directory = mkdtempSync(join(tmpdir(), "fasti-envelope-stderr-"));
  try {
    const source = readFileSync(
      join(root, "scripts/test-bench-envelope.sh"),
      "utf8",
    );
    const check = source.match(/^check\(\) \{\n[\s\S]*?^\}/mu)?.[0];
    assert.ok(check, "the actual envelope self-test check function is present");
    // Relocate only its fixed capture filenames; never share global /tmp files.
    const isolatedCheck = check.replaceAll(
      "/tmp/bench-envelope-selftest",
      join(directory, "capture"),
    );
    const payload =
      "FIRST\nsecond\nthird\nfourth\nFIFTH\n" + "x".repeat(9_000) + "TAIL";
    const payloadPath = join(directory, "payload");
    writeFileSync(payloadPath, payload);
    const result = spawnSync(
      "bash",
      [
        "-c",
        `set -eu\n${isolatedCheck}\nfailures=0
check diagnostic-retention 0 bash -c 'cat "$1" >&2; exit 17' bash "$1"
printf '\\nfailures=%s\\n' "$failures"
`,
        "bash",
        payloadPath,
      ],
      { encoding: "utf8", timeout: 5_000 },
    );
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /FAIL.*exit 17, wanted 0/u);
    assert.match(result.stdout, /\nfailures=1\n$/u);
    const diagnostics = result.stdout
      .slice(
        result.stdout.indexOf("\n") + 1,
        result.stdout.lastIndexOf("\nfailures="),
      )
      .replace(/^ {8}/gmu, "");
    assert.equal(Buffer.byteLength(diagnostics), 8_192);
    assert.ok(
      diagnostics === payload.slice(0, 8_192),
      "stderr retains its beginning and every line within the byte cap",
    );
    assert.equal(result.stderr, "");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
