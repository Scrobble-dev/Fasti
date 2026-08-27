import { strict as assert } from "node:assert";
import { readFile } from "node:fs/promises";

import { parseDocument } from "yaml";

/**
 * Parses a JSON string with strict validation for duplicates and schema conformance.
 * @param {string} source - The JSON source string to parse.
 * @param {string} [label="JSON document"] - A label for error reporting.
 * @returns {*} The parsed JSON value.
 * @throws {AssertionError} If YAML parsing errors occur.
 * @throws {SyntaxError} If the source is not strict JSON.
 */
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

/**
 * Reads and parses a JSON file with strict validation.
 * @param {string} path - The file path to read.
 * @returns {Promise<*>} The parsed JSON content.
 * @throws {AssertionError} If YAML parsing errors occur.
 * @throws {SyntaxError} If the file content is not strict JSON.
 */
export const readStrictJson = async (path) =>
  parseStrictJson(await readFile(path, "utf8"), path);
