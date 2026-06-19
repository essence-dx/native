#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const fs = require("node:fs");
const path = require("node:path");

const REPO_ROOT = path.resolve(__dirname, "..", "..");
const SOURCE_PATH = path.join(REPO_ROOT, "crates", "tauri-utils", "src", "config", "machine_cache.rs");

function functionBody(source, name) {
  const marker = `fn ${name}`;
  const start = source.indexOf(marker);
  if (start === -1) throw new Error(`missing function: ${name}`);

  const bodyStart = source.indexOf("{", start);
  if (bodyStart === -1) throw new Error(`missing body for function: ${name}`);

  let depth = 0;
  for (let index = bodyStart; index < source.length; index += 1) {
    const char = source[index];
    if (char === "{") depth += 1;
    if (char === "}") depth -= 1;
    if (depth === 0) return source.slice(bodyStart + 1, index);
  }

  throw new Error(`unterminated function body: ${name}`);
}

function assertContains(source, needle) {
  if (!source.includes(needle)) throw new Error(`missing expected source: ${needle}`);
}

function assertNotContains(source, needle) {
  if (source.includes(needle)) throw new Error(`unexpected source: ${needle}`);
}

function main() {
  const source = fs.readFileSync(SOURCE_PATH, "utf8");
  const projection = functionBody(source, "project_config_projection_from_archive");
  const uncheckedProjection = functionBody(source, "archived_json_tree_projected_value_at_path_unchecked");

  assertContains(projection, "archived_json_tree_is_well_formed(&archived.merged_config)");
  assertContains(
    projection,
    "archived_json_tree_projected_value_at_path_unchecked(&archived.merged_config, path)",
  );
  assertNotContains(
    projection,
    "archived_json_tree_projected_value_at_path(&archived.merged_config, path)",
  );
  assertNotContains(source, "fn archived_json_tree_projected_value_at_path(\n");
  assertNotContains(uncheckedProjection, "archived_json_tree_is_well_formed");

  console.log(`project-config projection validation guard ok: ${SOURCE_PATH}`);
}

main();
