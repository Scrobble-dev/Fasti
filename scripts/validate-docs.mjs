import assert from "node:assert/strict";
import { readFile, realpath, stat } from "node:fs/promises";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import Ajv2020 from "ajv/dist/2020.js";
import { parse as parseYaml } from "yaml";

/* eslint-disable security/detect-non-literal-fs-filename -- manifest sources are schema-validated and physically confined before reading; context paths are fixed below */
/* eslint-disable security/detect-object-injection -- string indexes are loop-bounded and CSV column indexes are validated before use */

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function sameSet(actual, expected, label) {
  assert.deepEqual(
    [...new Set(actual)].sort(),
    [...new Set(expected)].sort(),
    label,
  );
  assert.equal(
    actual.length,
    new Set(actual).size,
    `${label}: duplicates are not allowed`,
  );
}

function assertUnique(values, label) {
  assert.equal(
    values.length,
    new Set(values).size,
    `${label}: duplicates are not allowed`,
  );
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
      } else if (character === '"') quoted = false;
      else field += character;
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
    } else field += character;
  }
  assert.equal(quoted, false, "CSV has an unterminated quoted field");
  if (field || row.length) rows.push([...row, field.replace(/\r$/u, "")]);
  return rows;
}

function assertSchema(value, schema, label) {
  const ajv = new Ajv2020({ allErrors: true, strict: true });
  const validate = ajv.compile(schema);
  assert.ok(
    validate(value),
    `${label}: ${ajv.errorsText(validate.errors, { separator: "\n" })}`,
  );
}

function isConfined(parent, child) {
  const path = relative(parent, child);
  return (
    path !== "" &&
    path !== ".." &&
    !path.startsWith(`..${sep}`) &&
    !isAbsolute(path)
  );
}

async function validate(site, personas, context) {
  assertSchema(site, context.siteSchema, "docs/site.yaml");
  assertSchema(personas, context.personasSchema, "docs/personas.yaml");

  sameSet(
    site.sections.map(({ id }) => id),
    [
      "start",
      "use",
      "operate",
      "integrate",
      "extend",
      "contribute",
      "reference",
    ],
    "section IDs",
  );
  sameSet(
    personas.tracks.map(({ id }) => id),
    ["use", "operate", "integrate", "extend", "contribute"],
    "persona tracks",
  );
  assertUnique(
    site.pages.map(({ id }) => id),
    "page IDs",
  );
  assertUnique(
    site.pages.map(({ route }) => route),
    "page routes",
  );

  const physicalRoot = await realpath(root);
  for (const page of site.pages) {
    const lexical = resolve(root, page.source);
    assert.ok(
      isConfined(root, lexical),
      `${page.id}: source escapes the repository`,
    );
    const physical = await realpath(lexical);
    assert.ok(
      isConfined(physicalRoot, physical),
      `${page.id}: source resolves outside the repository`,
    );
    assert.ok(
      (await stat(physical)).isFile(),
      `${page.id}: source is not a regular file`,
    );
    if (page.status === "unavailable") {
      assert.ok(
        page.blocked_by?.trim(),
        `${page.id}: unavailable pages require blocked_by`,
      );
      assert.ok(
        page.safe_alternative?.trim(),
        `${page.id}: unavailable pages require safe_alternative`,
      );
    }
    const source = await readFile(physical, "utf8");
    assert.match(
      source,
      /^# [^#\r\n]+/u,
      `${page.id}: source needs one leading H1`,
    );
    assert.equal(
      (source.match(/^# /gmu) ?? []).length,
      1,
      `${page.id}: source must contain one H1`,
    );
    for (const phrase of context.termbase.disallowed) {
      assert.ok(
        !source
          .toLocaleLowerCase("en")
          .includes(phrase.phrase.toLocaleLowerCase("en")),
        `${page.id}: disallowed STE phrase ${phrase.phrase}`,
      );
    }
  }

  const uat = parseCsv(context.uatCsv);
  const personaColumn = uat[0].indexOf("persona");
  const idColumn = uat[0].indexOf("test_id");
  assert.ok(
    personaColumn >= 0 && idColumn >= 0,
    "identity UAT header is missing persona or test_id",
  );
  const governed = new Map();
  for (const row of uat.slice(1)) {
    const entries = governed.get(row[personaColumn]) ?? [];
    entries.push(row[idColumn]);
    governed.set(row[personaColumn], entries);
  }
  sameSet(
    personas.personas.map(({ id }) => id),
    [...governed.keys()],
    "persona IDs",
  );

  const publishedRoutes = new Set(
    site.pages
      .filter(({ status }) => status === "published")
      .map(({ route }) => route),
  );
  const capabilityIds = new Set(
    context.capabilities.capabilities.map(({ id }) => id),
  );
  for (const persona of personas.personas) {
    assert.ok(
      publishedRoutes.has(persona.first_task),
      `${persona.id}: first_task is not published`,
    );
    assert.ok(
      publishedRoutes.has(persona.recovery_task),
      `${persona.id}: recovery_task is not published`,
    );
    sameSet(
      persona.uat_rows,
      governed.get(persona.id),
      `${persona.id} UAT rows`,
    );
    for (const id of persona.capability_ids)
      assert.ok(
        capabilityIds.has(id),
        `${persona.id}: unknown capability ${id}`,
      );
  }
  sameSet(
    personas.personas
      .filter(({ cross_cutting }) => cross_cutting)
      .map(({ id }) => id),
    ["audhd_user", "screen_reader_user"],
    "cross-cutting personas",
  );

  assert.equal(
    context.reviews.review_policy.certification_claim_allowed,
    false,
    "STE reviews must forbid certification claims",
  );
  const reviews = new Map(
    context.reviews.reviews.map((review) => [review.page_id, review]),
  );
  for (const page of site.pages.filter(
    ({ ste_review }) => ste_review === "human_reviewed",
  )) {
    assert.ok(
      reviews.has(page.id),
      `${page.id}: human_reviewed needs a review ledger entry`,
    );
  }
  return {
    pages: site.pages.length,
    personas: personas.personas.length,
    uatRows: uat.length - 1,
  };
}

async function loadContext() {
  const readJson = async (path) =>
    JSON.parse(await readFile(resolve(root, path), "utf8"));
  const readYaml = async (path) =>
    parseYaml(await readFile(resolve(root, path), "utf8"));
  return {
    site: await readYaml("docs/site.yaml"),
    personas: await readYaml("docs/personas.yaml"),
    siteSchema: await readJson("docs/schemas/site.schema.json"),
    personasSchema: await readJson("docs/schemas/personas.schema.json"),
    termbase: await readYaml("docs/style/ste/termbase.yaml"),
    reviews: await readYaml("docs/style/ste/reviews.yaml"),
    uatCsv: await readFile(
      resolve(root, "tests/conformance/identity-uat-matrix.v1.csv"),
      "utf8",
    ),
    capabilities: await readJson("contracts/generated/v1/capabilities.json"),
  };
}

const context = await loadContext();
if (process.argv.includes("--self-test")) {
  const invented = structuredClone(context.personas);
  invented.personas[0].id = "invented_persona";
  await assert.rejects(
    validate(context.site, invented, context),
    /persona IDs/u,
  );
  const traversal = structuredClone(context.site);
  traversal.pages[0].source = "docs/../../security.md";
  await assert.rejects(
    validate(traversal, context.personas, context),
    /source escapes the repository/u,
  );
  const duplicate = structuredClone(context.site);
  duplicate.pages[1].route = duplicate.pages[0].route;
  await assert.rejects(
    validate(duplicate, context.personas, context),
    /page routes/u,
  );
  const unavailable = structuredClone(context.site);
  unavailable.pages[0].status = "unavailable";
  delete unavailable.pages[0].blocked_by;
  delete unavailable.pages[0].safe_alternative;
  await assert.rejects(
    validate(unavailable, context.personas, context),
    /blocked_by|safe_alternative/u,
  );
  console.log("PASS: documentation publication mutation sentinels");
} else {
  const summary = await validate(context.site, context.personas, context);
  console.log(
    `PASS: public documentation manifest pages=${summary.pages} personas=${summary.personas} identity_uat=${summary.uatRows}`,
  );
}

/* eslint-enable security/detect-non-literal-fs-filename, security/detect-object-injection */
