#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const fs = require("node:fs");
const path = require("node:path");

const REPO_ROOT = path.resolve(__dirname, "..", "..");
const TEST_OUTPUT_ROOT = "G:\\Dx\\test-outputs";
const RUST_TESTS = path.join(
  REPO_ROOT,
  "crates",
  "tauri-cli",
  "src",
  "interface",
  "rust",
  "machine_cache_benchmark_tests.rs",
);

function assertContains(source, needle, label) {
  if (!source.includes(needle)) {
    throw new Error(`${label} is missing: ${needle}`);
  }
}

function main() {
  const resolvedOutputRoot = path.resolve(TEST_OUTPUT_ROOT);
  if (resolvedOutputRoot !== path.resolve("G:\\Dx\\test-outputs")) {
    throw new Error(`benchmark receipts must stay under ${TEST_OUTPUT_ROOT}`);
  }

  const source = fs.readFileSync(RUST_TESTS, "utf8");
  assertContains(
    source,
    "fn dx_full_cargo_metadata_machine_cache_writes_current_fork_benchmark_receipt()",
    "ignored benchmark receipt test",
  );
  assertContains(
    source,
    "fn dx_watch_folders_machine_cache_writes_current_fork_benchmark_receipt()",
    "ignored watcher benchmark receipt test",
  );
  assertContains(
    source,
    '"dx.tauri.cli.cargo_metadata_source_vs_machine_receipt"',
    "receipt schema",
  );
  assertContains(
    source,
    '"dx.tauri.cli.watch_folders_source_vs_machine_receipt"',
    "watch folder receipt schema",
  );
  assertContains(
    source,
    '"cargo metadata --no-deps"',
    "cache boundary",
  );
  assertContains(
    source,
    '"cargo metadata spawn plus serde_json parse"',
    "official-style baseline",
  );
  assertContains(
    source,
    '"pre-generated dx-serializer .machine read"',
    "pre-generated machine benchmark mode",
  );
  assertContains(
    source,
    '"get_watch_folders"',
    "watch folder command-level boundary",
  );
  assertContains(
    source,
    '"machine_cache_generation_measured"',
    "generation excluded from timing",
  );
  assertContains(source, "serde_json::json!(false)", "false claim flags");
  assertContains(
    source,
    'tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV',
    "read-only timing env",
  );
  assertContains(
    source,
    '"machine_file_unchanged_during_timing"',
    "machine artifact stability proof",
  );
  assertContains(source, "machine_before == machine_after", "machine artifact byte equality");
  assertContains(
    source,
    '"faster_than_upstream_claimed"',
    "no broad upstream claim",
  );

  console.log("cargo metadata machine benchmark receipt contract is present");
}

main();
