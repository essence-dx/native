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
  "config.rs",
);
const RECEIPT_TESTS = path.join(
  REPO_ROOT,
  "crates",
  "tauri-cli",
  "src",
  "helpers",
  "config_package_version_machine_cache_benchmark_tests.rs",
);
const RECEIPT_SUPPORT = [
  RECEIPT_TESTS,
  path.join(
    REPO_ROOT,
    "crates",
    "tauri-cli",
    "src",
    "helpers",
    "config_package_version_machine_cache_benchmark_fixture.rs",
  ),
  path.join(
    REPO_ROOT,
    "crates",
    "tauri-cli",
    "src",
    "helpers",
    "config_package_version_machine_cache_benchmark_receipt.rs",
  ),
  path.join(
    REPO_ROOT,
    "crates",
    "tauri-cli",
    "src",
    "helpers",
    "config_package_version_machine_cache_benchmark_receipt_json.rs",
  ),
];

function readFile(file) {
  return fs.readFileSync(file, "utf8");
}

function readReceiptSources() {
  return RECEIPT_SUPPORT.map((file) => readFile(file)).join("\n");
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
      '#[path = "config_package_version_machine_cache_benchmark_tests.rs"]',
      "mod config_package_version_machine_cache_benchmark_tests;",
    ].join("\n"),
    "package-version benchmark test module declaration",
  );

  if (!fs.existsSync(RECEIPT_TESTS)) {
    throw new Error(`package-version benchmark receipt tests are missing: ${RECEIPT_TESTS}`);
  }

  const source = readReceiptSources();
  assertContains(
    source,
    "fn dx_package_version_machine_cache_writes_source_vs_machine_receipt()",
    "ignored package-version receipt test",
  );
  assertContains(
    source,
    '"dx.tauri.cli.package_version_source_vs_machine_receipt"',
    "receipt schema",
  );
  assertContains(source, '"load_config package.version path"', "cache boundary");
  assertContains(source, '"tauri.conf.json/package.json source parse"', "source baseline");
  assertContains(source, '"pre-generated dx-serializer .machine read"', "machine mode");
  assertContains(source, "--no-default-features", "lightweight receipt command");
  assertContains(source, "--features dx-machine-cache", "non-mmap receipt command");
  assertContains(source, '"machine_cache_generation_measured"', "generation excluded");
  assertContains(source, '"cache_write_included_in_timing"', "write cost excluded");
  assertContains(source, '"machine_cache_write_env_for_timing"', "write disabled during timing");
  assertContains(source, '"machine_file_unchanged_during_timing"', "artifact stability proof");
  assertContains(source, "machine_before == machine_after", "machine artifact byte equality");
  assertContains(source, '"full_cli_speed_claimed"', "full CLI claim blocked");
  assertContains(source, '"full_cli_speed_claim_allowed"', "full CLI claim gate blocked");
  assertContains(source, '"faster_than_upstream_claimed"', "upstream claim blocked");
  assertContains(
    source,
    '"faster_than_upstream_claim_allowed"',
    "upstream claim gate blocked",
  );
  assertContains(source, '"default_on_readiness_claimed"', "default-on claim blocked");
  assertContains(
    source,
    '"default_on_readiness_claim_allowed"',
    "default-on claim gate blocked",
  );
  assertContains(source, '"product_level_speed_claimed"', "product-level claim blocked");
  assertContains(
    source,
    '"product_level_speed_claim_allowed"',
    "product-level claim gate blocked",
  );
  assertContains(
    source,
    '"package_version_helper_speed_claim_allowed"',
    "narrow package-version helper claim gate",
  );
  assertContains(
    source,
    '"timed_machine_package_version_hits_verified"',
    "timed machine read-hit proof",
  );
  assertContains(
    source,
    '"package_version_helper_speed_claim_metric"',
    "narrow helper claim metric",
  );
  assertContains(
    source,
    '"package_version_helper_meets_requested_10x"',
    "10x claim gate stays explicit",
  );
  assertContains(
    source,
    '"package_version_helper_machine_read_faster_than_source_parse"',
    "narrow package-version helper allowed claim",
  );
  assertContains(
    source,
    "tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV",
    "read-only timing env",
  );

  console.log("package-version benchmark receipt contract is present");
}

main();
