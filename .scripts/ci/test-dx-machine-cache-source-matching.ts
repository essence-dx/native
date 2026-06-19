#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const fs = require("node:fs");
const path = require("node:path");

const REPO_ROOT = path.resolve(__dirname, "..", "..");

function functionBody(source, name) {
  const marker = `fn ${name}(`;
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
  if (source.includes(needle)) throw new Error(`unexpected nested source matching remains: ${needle}`);
}

function assertFunctionDoesNotContain(source, functionName, needle) {
  const body = functionBody(source, functionName);
  if (body.includes(needle)) {
    throw new Error(`${functionName} still contains ${needle}`);
  }
}

function checkFile(relativePath, validatorName) {
  const filePath = path.join(REPO_ROOT, relativePath);
  const source = fs.readFileSync(filePath, "utf8");
  const validator = functionBody(source, validatorName);
  const matcher = functionBody(source, "source_fingerprints_match");

  assertContains(source, "HashMap");
  assertContains(source, "fn source_fingerprints_match(");
  assertContains(validator, "source_fingerprints_match(&cache.sources, &snapshot.sources)");
  assertContains(matcher, "collect::<HashMap<_, _>>()");
  assertContains(matcher, "current_by_path.len() != current.len()");
  assertContains(matcher, "expected_by_path.len() != expected.len()");
  assertNotContains(validator, ".iter()\n      .any(|current| current.path == source.path && current.matches(source))");
}

function checkSnapshotPushes(relativePath, pushFunctions) {
  const filePath = path.join(REPO_ROOT, relativePath);
  const source = fs.readFileSync(filePath, "utf8");

  assertContains(source, "seen_paths: HashSet<String>");
  for (const pushFunction of pushFunctions) {
    assertFunctionDoesNotContain(source, pushFunction, "sources.iter().any");
  }
}

function main() {
  checkFile("crates/tauri-cli/src/interface/rust/cargo_metadata_machine_cache.rs", "current_cargo_metadata_sources_match");
  checkFile("crates/tauri-cli/src/interface/rust/workspace_machine_cache.rs", "current_workspace_sources_match");
  checkFile("crates/tauri-cli/src/interface/rust/cargo_config/machine_cache.rs", "current_cargo_config_sources_match");
  checkFile("crates/tauri-cli/src/helpers/cargo_manifest.rs", "current_cargo_sources_match");
  checkSnapshotPushes("crates/tauri-cli/src/interface/rust/cargo_metadata_machine_cache.rs", [
    "push_current_path_snapshot",
    "push_source_snapshot",
  ]);
  checkSnapshotPushes("crates/tauri-cli/src/interface/rust/workspace_machine_cache.rs", [
    "push_current_path_snapshot",
    "push_source_snapshot",
  ]);
  checkSnapshotPushes("crates/tauri-cli/src/interface/rust/cargo_config/machine_cache.rs", [
    "push_current_path_snapshot",
    "push_source_snapshot",
  ]);
  checkSnapshotPushes("crates/tauri-cli/src/helpers/cargo_manifest.rs", [
    "push_unique_source",
    "push_unique_source_snapshot",
  ]);

  console.log("machine-cache source matching guard ok");
}

main();
