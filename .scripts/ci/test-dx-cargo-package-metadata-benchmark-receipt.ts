#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const fs = require("node:fs");
const path = require("node:path");

const REPO_ROOT = path.resolve(__dirname, "..", "..");
const TEST_OUTPUT_ROOT = "G:\\Dx\\test-outputs";
const MODULE_DECLARATION = path.join(
  REPO_ROOT,
  "crates",
  "tauri-cli",
  "src",
  "helpers",
  "cargo_manifest.rs",
);
const RECEIPT_TESTS = path.join(
  REPO_ROOT,
  "crates",
  "tauri-cli",
  "src",
  "helpers",
  "cargo_manifest_machine_cache_benchmark_tests.rs",
);

function readFile(file) {
  return fs.readFileSync(file, "utf8");
}

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

  assertContains(
    readFile(MODULE_DECLARATION),
    [
      '#[cfg(all(test, feature = "dx-machine-cache"))]',
      '#[path = "cargo_manifest_machine_cache_benchmark_tests.rs"]',
      "mod cargo_manifest_machine_cache_benchmark_tests;",
    ].join("\n"),
    "cargo manifest benchmark test module declaration",
  );

  if (!fs.existsSync(RECEIPT_TESTS)) {
    throw new Error(`cargo package metadata benchmark receipt tests are missing: ${RECEIPT_TESTS}`);
  }

  const source = readFile(RECEIPT_TESTS);
  assertContains(
    source,
    "fn dx_cargo_package_metadata_machine_cache_writes_source_vs_machine_receipt()",
    "ignored package metadata receipt test",
  );
  assertContains(
    source,
    '"dx.tauri.cli.cargo_package_metadata_source_vs_machine_receipt"',
    "receipt schema",
  );
  assertContains(source, '"cargo_manifest_and_lock"', "cache boundary");
  assertContains(source, '"Cargo.toml/Cargo.lock source parse"', "source baseline");
  assertContains(source, '"pre-generated dx-serializer .machine read"', "machine mode");
  assertContains(source, "--no-default-features", "lightweight receipt command");
  assertContains(source, "--features dx-machine-cache", "non-mmap receipt command");
  assertContains(source, '"machine_cache_generation_measured"', "generation excluded");
  assertContains(source, '"cache_write_included_in_timing"', "write cost excluded");
  assertContains(source, '"machine_cache_write_env_for_timing"', "write disabled during timing");
  assertContains(source, '"machine_file_unchanged_during_timing"', "artifact stability proof");
  assertContains(source, "machine_before == machine_after", "machine artifact byte equality");
  assertContains(source, '"full_cli_speed_claimed"', "full CLI claim blocked");
  assertContains(source, '"faster_than_upstream_claimed"', "upstream claim blocked");
  assertContains(
    source,
    "tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV",
    "read-only timing env",
  );

  console.log("cargo package metadata benchmark receipt contract is present");
}

main();
