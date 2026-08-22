import { strict as assert } from "node:assert";
import { readFile } from "node:fs/promises";

import { parseDocument } from "yaml";

export const parseStrictJson = (source, label = "JSON document") => {
  const document = parseDocument(source, {
    schema: "json",
    strict: true,
    uniqueKeys: true,
  });
  assert.deepEqual(
    document.errors,
    [],
    `${label} errors:\n${document.errors.join("\n")}`,
  );
  try {
    return JSON.parse(source);
  } catch (error) {
    throw new SyntaxError(`${label} is not strict JSON`, { cause: error });
  }
};

export const readStrictJson = async (path) =>
  parseStrictJson(await readFile(path, "utf8"), path);
