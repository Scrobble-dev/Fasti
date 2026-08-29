import type { SidebarsConfig } from "@docusaurus/plugin-content-docs";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const sidebars = JSON.parse(
  readFileSync(
    resolve(__dirname, "../../target/docs-site/sidebars.json"),
    "utf8",
  ),
) as SidebarsConfig;

export default sidebars;
