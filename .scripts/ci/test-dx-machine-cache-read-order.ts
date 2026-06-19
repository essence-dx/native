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

function assertBefore(source, earlier, later) {
  const earlierIndex = source.indexOf(earlier);
  const laterIndex = source.indexOf(later);
  if (earlierIndex === -1) throw new Error(`missing expected source: ${earlier}`);
  if (laterIndex === -1) throw new Error(`missing expected source: ${later}`);
  if (earlierIndex > laterIndex) {
    throw new Error(`expected "${earlier}" before "${later}"`);
  }
}

function main() {
  const source = fs.readFileSync(SOURCE_PATH, "utf8");
  const readJson = functionBody(source, "read_json_value_machine_cache");
  const readProject = functionBody(source, "read_project_config_machine_cache");
  const readProjection = functionBody(source, "read_project_config_projection_machine_cache");

  assertBefore(
    readJson,
    "machine_cache_file_is_candidate(&paths.machine, TAURI_CONFIG_MACHINE_CACHE_MAX_BYTES)",
    "source_fingerprint(source_path)",
  );
  assertBefore(
    readProject,
    "machine_cache_file_is_candidate(&paths.machine, TAURI_CONFIG_MACHINE_CACHE_MAX_BYTES)",
    "source_fingerprint(config_path)",
  );
  assertBefore(
    readProjection,
    "machine_cache_file_is_candidate(&cache_paths.machine, TAURI_CONFIG_MACHINE_CACHE_MAX_BYTES)",
    "source_fingerprint(config_path)",
  );

  console.log(`machine-cache read order ok: ${SOURCE_PATH}`);
}

main();
