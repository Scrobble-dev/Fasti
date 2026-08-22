import { strict as assert } from "node:assert";
import { readFile, readdir, stat } from "node:fs/promises";
import { dirname, extname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import { parse as parseYaml } from "yaml";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const okfRoot = resolve(repositoryRoot, "contracts/okf/v1");
const registryPath = resolve(
  repositoryRoot,
  "contracts/registry/v1/capabilities.yaml",
);
const uatCsvPath = resolve(repositoryRoot, "tests/conformance/uat-matrix.csv");
const uatOwnershipPath = resolve(
  repositoryRoot,
  "tests/conformance/uat-ownership.v1.json",
);

const RESERVED_OKF_FILES = new Set(["index.md", "log.md"]);
const ALLOWED_OKF_STATUS = new Set(["draft", "stable", "deprecated"]);
const ALLOWED_UAT_STATUS = new Set([
  "executable-b1",
  "contract-b1",
  "deferred",
]);
const ALLOWED_BODIES = new Set([
  "b1",
  "b2",
  "b3",
  "b4",
  "b5",
  "b6",
  "b7",
  "b8",
  "post-b8",
]);

function setDifference(left, right) {
  return [...left].filter((value) => !right.has(value)).sort();
}

function assertSameSet(actualValues, expectedValues, label) {
  const actualList = [...actualValues];
  const expectedList = [...expectedValues];
  const actual = new Set(actualList);
  const expected = new Set(expectedList);
  assert.equal(
    actual.size,
    actualList.length,
    `${label} contains duplicate identifiers`,
  );
  assert.deepEqual(
    setDifference(actual, expected),
    [],
    `${label} contains unexpected identifiers`,
  );
  assert.deepEqual(
    setDifference(expected, actual),
    [],
    `${label} omits governed identifiers`,
  );
}

function parseFrontmatter(source, label) {
  const match = source.match(/^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/u);
  assert.ok(match, `${label} must begin with YAML frontmatter`);
  const frontmatter = parseYaml(match[1]);
  assert.ok(
    frontmatter &&
      typeof frontmatter === "object" &&
      !Array.isArray(frontmatter),
    `${label} frontmatter must be a mapping`,
  );
  return { frontmatter, body: source.slice(match[0].length) };
}

async function markdownFiles(root) {
  const files = [];
  for (const entry of await readdir(root, { withFileTypes: true })) {
    const path = resolve(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await markdownFiles(path)));
    } else if (entry.isFile() && extname(entry.name) === ".md") {
      files.push(path);
    }
  }
  return files.sort();
}

function markdownTargets(body) {
  const targets = [];
  const expression = /!?\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/gu;
  for (const match of body.matchAll(expression)) {
    targets.push(match[1].replace(/^<|>$/gu, ""));
  }
  return targets;
}

async function assertLocalTargetExists(sourcePath, target) {
  if (target.startsWith("#") || /^[a-z][a-z0-9+.-]*:/iu.test(target)) {
    return;
  }
  const pathPart = decodeURIComponent(target.split("#", 1)[0]);
  assert.ok(
    pathPart,
    `${relative(repositoryRoot, sourcePath)} has an empty link`,
  );
  const resolvedTarget = target.startsWith("/")
    ? resolve(okfRoot, `.${pathPart}`)
    : resolve(dirname(sourcePath), pathPart);
  const repositoryPrefix = `${repositoryRoot}${sep}`;
  assert.ok(
    resolvedTarget.startsWith(repositoryPrefix),
    `${relative(repositoryRoot, sourcePath)} links outside the repository: ${target}`,
  );
  const targetStat = await stat(resolvedTarget).catch(() => null);
  assert.ok(
    targetStat,
    `${relative(repositoryRoot, sourcePath)} has a missing local link: ${target}`,
  );
}

function assertStringList(value, label) {
  assert.ok(Array.isArray(value), `${label} must be a YAML list`);
  assert.ok(value.length > 0, `${label} must not be empty`);
  for (const item of value) {
    assert.equal(typeof item, "string", `${label} values must be strings`);
    assert.ok(item.trim(), `${label} values must not be blank`);
  }
}

async function validateOkf(registry) {
  const files = await markdownFiles(okfRoot);
  assert.ok(files.length > 1, "OKF bundle must include concepts and an index");
  const rootIndexPath = resolve(okfRoot, "index.md");
  assert.ok(
    files.includes(rootIndexPath),
    "OKF bundle requires a root index.md",
  );

  const documents = new Map();
  for (const path of files) {
    const label = relative(repositoryRoot, path);
    const source = await readFile(path, "utf8");
    const { frontmatter, body } = parseFrontmatter(source, label);
    documents.set(path, { frontmatter, body });

    const basename = path.slice(path.lastIndexOf(sep) + 1);
    if (RESERVED_OKF_FILES.has(basename)) {
      assert.equal(
        path,
        rootIndexPath,
        `${label} is an unexpected reserved file`,
      );
      assert.deepEqual(
        Object.keys(frontmatter),
        ["okf_version"],
        "root index frontmatter may only declare okf_version",
      );
      assert.equal(frontmatter.okf_version, "0.2");
    } else {
      assert.equal(typeof frontmatter.type, "string", `${label} needs type`);
      assert.ok(frontmatter.type.trim(), `${label} type must not be blank`);
      if (frontmatter.description !== undefined) {
        assert.equal(
          typeof frontmatter.description,
          "string",
          `${label} description must be a string`,
        );
        assert.ok(
          !/[\r\n]/u.test(frontmatter.description),
          `${label} description must be one line`,
        );
      }
      if (frontmatter.tags !== undefined) {
        assertStringList(frontmatter.tags, `${label} tags`);
      }
      if (frontmatter.status !== undefined) {
        assert.ok(
          ALLOWED_OKF_STATUS.has(frontmatter.status),
          `${label} has invalid OKF lifecycle status`,
        );
      }
      if (frontmatter.sources !== undefined) {
        assert.ok(
          Array.isArray(frontmatter.sources),
          `${label} sources must be a list`,
        );
        for (const sourceEntry of frontmatter.sources) {
          assert.ok(
            sourceEntry && typeof sourceEntry === "object",
            `${label} source entries must be mappings`,
          );
          assert.equal(
            typeof sourceEntry.resource,
            "string",
            `${label} source resource is required`,
          );
          assert.ok(
            sourceEntry.resource.trim(),
            `${label} source resource is blank`,
          );
          await assertLocalTargetExists(path, sourceEntry.resource);
        }
      }
      const sourceIds = new Set(
        (frontmatter.sources ?? [])
          .map(({ id }) => id)
          .filter((id) => typeof id === "string"),
      );
      for (const match of body.matchAll(/\[\^([^\]]+)\]/gu)) {
        assert.ok(
          sourceIds.has(match[1]),
          `${label} footnote ${match[1]} has no matching sources id`,
        );
      }
      assert.ok(
        !/\bplayer\b|chronicle/iu.test(body),
        `${label} must not introduce player or Chronicle UI claims`,
      );
    }

    for (const target of markdownTargets(body)) {
      await assertLocalTargetExists(path, target);
    }
  }

  const concepts = files.filter(
    (path) => !RESERVED_OKF_FILES.has(path.slice(path.lastIndexOf(sep) + 1)),
  );
  const indexTargets = new Set(
    markdownTargets(documents.get(rootIndexPath).body)
      .filter((target) => target.endsWith(".md"))
      .map((target) => resolve(okfRoot, target)),
  );
  assertSameSet(indexTargets, concepts, "OKF root index concept links");

  const finalizedB1 = registry.capabilities.filter(
    ({ contract_body: contractBody, lifecycle }) =>
      contractBody === "b1" && lifecycle.contract_state === "finalized",
  );
  const catalogueDefinitions = [
    {
      name: "capabilities.md",
      key: "identifiers",
      values: finalizedB1.map(({ id }) => id),
    },
    {
      name: "problems.md",
      key: "identifiers",
      values: [
        ...new Set(finalizedB1.flatMap(({ problems }) => problems)),
      ].sort(),
    },
    {
      name: "scopes.md",
      key: "identifiers",
      values: [...new Set(finalizedB1.flatMap(({ scopes }) => scopes))].sort(),
    },
    {
      name: "lifecycle.md",
      key: "contract_states",
      values: [
        ...new Set(
          registry.capabilities.map(
            ({ lifecycle }) => lifecycle.contract_state,
          ),
        ),
      ].sort(),
    },
    {
      name: "lifecycle.md",
      key: "runtime_availabilities",
      values: [
        ...new Set(
          registry.capabilities.map(
            ({ lifecycle }) => lifecycle.runtime_availability,
          ),
        ),
      ].sort(),
    },
    {
      name: "lifecycle.md",
      key: "body_ids",
      values: [
        ...new Set(
          registry.capabilities.flatMap(
            ({
              contract_body: contractBody,
              runtime_body: runtimeBody,
              lifecycle,
            }) => [contractBody, runtimeBody, lifecycle.introduced_in],
          ),
        ),
      ].sort(),
    },
  ];

  for (const { name, key, values } of catalogueDefinitions) {
    const path = resolve(okfRoot, name);
    const document = documents.get(path);
    assert.ok(document, `OKF catalogue is missing ${name}`);
    assertStringList(document.frontmatter[key], `${name} ${key}`);
    assertSameSet(document.frontmatter[key], values, `${name} ${key}`);
    for (const value of values) {
      assert.ok(
        document.body.includes(`\`${value}\``),
        `${name} body does not explain ${value}`,
      );
    }
  }

  return { conceptCount: concepts.length };
}

function parseCsv(source) {
  const rows = [];
  let row = [];
  let field = "";
  let quoted = false;
  for (let index = 0; index < source.length; index += 1) {
    const character = source[index];
    if (quoted) {
      if (character === '"' && source[index + 1] === '"') {
        field += '"';
        index += 1;
      } else if (character === '"') {
        quoted = false;
      } else {
        field += character;
      }
    } else if (character === '"') {
      assert.equal(field, "", "CSV quote must begin an empty field");
      quoted = true;
    } else if (character === ",") {
      row.push(field);
      field = "";
    } else if (character === "\n") {
      row.push(field.replace(/\r$/u, ""));
      rows.push(row);
      row = [];
      field = "";
    } else {
      field += character;
    }
  }
  assert.equal(quoted, false, "CSV has an unterminated quoted field");
  if (field || row.length) {
    row.push(field.replace(/\r$/u, ""));
    rows.push(row);
  }
  return rows;
}

async function validateUat(registry) {
  const rows = parseCsv(await readFile(uatCsvPath, "utf8"));
  assert.deepEqual(rows[0], [
    "ID",
    "Area",
    "Scenario",
    "Expected result",
    "Release gate",
    "Priority",
    "Test type",
  ]);
  const sourceRows = rows.slice(1);
  assert.equal(sourceRows.length, 80, "source UAT matrix must contain 80 rows");
  assert.ok(
    sourceRows.every((row) => row.length === rows[0].length),
    "every source UAT row must have the header field count",
  );
  const expectedIds = Array.from(
    { length: 80 },
    (_, index) => `ID-${String(index + 1).padStart(3, "0")}`,
  );
  assert.deepEqual(
    sourceRows.map(([id]) => id),
    expectedIds,
    "source UAT IDs must be the complete ordered ID-001 through ID-080 set",
  );

  const ownership = JSON.parse(await readFile(uatOwnershipPath, "utf8"));
  assert.match(ownership.version, /^\d+\.\d+\.\d+$/u);
  assert.equal(ownership.source, "uat-matrix.csv");
  assert.ok(
    Array.isArray(ownership.cases),
    "UAT ownership cases must be a list",
  );
  assert.equal(
    ownership.cases.length,
    80,
    "UAT ownership must contain 80 cases",
  );
  assert.deepEqual(
    ownership.cases.map(({ id }) => id),
    expectedIds,
    "UAT ownership must map every source ID exactly once and in source order",
  );

  for (const entry of ownership.cases) {
    assert.ok(
      ALLOWED_UAT_STATUS.has(entry.status),
      `${entry.id} has invalid status`,
    );
    assert.ok(
      ALLOWED_BODIES.has(entry.owner_body),
      `${entry.id} has invalid owner body`,
    );
    assert.equal(
      typeof entry.reason,
      "string",
      `${entry.id} reason must be a string`,
    );
    assert.ok(
      entry.reason.trim().length >= 40,
      `${entry.id} needs a specific reason`,
    );
    if (entry.status === "deferred") {
      assert.notEqual(
        entry.owner_body,
        "b1",
        `${entry.id} deferred owner cannot be B1`,
      );
    }
    if (entry.status === "executable-b1") {
      assert.equal(
        entry.owner_body,
        "b1",
        `${entry.id} executable B1 owner must be B1`,
      );
      assertStringList(entry.evidence, `${entry.id} executable evidence`);
    }
  }

  const registryUatIds = new Set(
    registry.capabilities.flatMap(({ uat }) => uat.map(({ id }) => id)),
  );
  const b1TraceIds = ownership.cases
    .filter(
      ({ status }) => status === "contract-b1" || status === "executable-b1",
    )
    .map(({ id }) => id);
  assertSameSet(
    b1TraceIds,
    registryUatIds,
    "B1 UAT ownership and capability-registry trace IDs",
  );

  return {
    caseCount: ownership.cases.length,
    executableCount: ownership.cases.filter(
      ({ status }) => status === "executable-b1",
    ).length,
    contractCount: ownership.cases.filter(
      ({ status }) => status === "contract-b1",
    ).length,
    deferredCount: ownership.cases.filter(({ status }) => status === "deferred")
      .length,
  };
}

const registry = parseYaml(await readFile(registryPath, "utf8"));
assert.ok(
  Array.isArray(registry.capabilities),
  "capability registry is malformed",
);
const [okf, uat] = await Promise.all([
  validateOkf(registry),
  validateUat(registry),
]);

console.log(
  `PASS: OKF 0.2 catalogue concepts=${okf.conceptCount}; UAT cases=${uat.caseCount} executable-b1=${uat.executableCount} contract-b1=${uat.contractCount} deferred=${uat.deferredCount}`,
);
