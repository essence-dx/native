#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const fs = require("node:fs");
const path = require("node:path");

const REPO_ROOT = path.resolve(__dirname, "..", "..");
const TEST_OUTPUT_ROOT = path.resolve("G:\\Dx\\test-outputs");
const RECEIPT_DIR = path.join(TEST_OUTPUT_ROOT, "tauri-stage6-cargo-metadata-fallback-20260531-a");
const RUST_TESTS = path.join(
  REPO_ROOT,
  "crates",
  "tauri-cli",
  "src",
  "interface",
  "rust",
  "machine_cache_tests.rs",
);

function assertContains(source, needle, label) {
  if (!source.includes(needle)) {
    throw new Error(`${label} is missing: ${needle}`);
  }
}

function main() {
  const source = fs.readFileSync(RUST_TESTS, "utf8");
  const requiredTests = [
    [
      "dx_full_cargo_metadata_machine_cache_corrupt_file_falls_back_and_refreshes",
      "corrupt full cargo metadata cache fallback",
    ],
    [
      "dx_full_cargo_metadata_machine_cache_oversized_file_falls_back_without_trusting_cache",
      "oversized full cargo metadata cache fallback",
    ],
    [
      "dx_full_cargo_metadata_machine_cache_unsupported_schema_falls_back_and_refreshes",
      "unsupported full cargo metadata cache schema fallback",
    ],
  ];

  for (const [testName, label] of requiredTests) {
    assertContains(source, `fn ${testName}()`, label);
  }
  assertContains(source, "cargo_metadata_machine_cache::machine_path", "machine path coverage");
  assertContains(source, "std::fs::write(&machine_path", "corrupt cache write coverage");
  assertContains(source, "set_len(64 * 1024 * 1024 + 1)", "oversized cache coverage");
  assertContains(source, "cargo_settings_machine_path(&fixture.app_dir)", "unsupported schema source cache coverage");
  assertContains(source, "std::fs::copy(settings_machine_path, &machine_path)", "unsupported schema copy coverage");
  assertContains(source, "get_cargo_metadata(&fixture.app_dir)", "fallback uses public metadata helper");
  assertContains(source, "cargo_metadata_machine_cache::read(&fixture.app_dir).is_some()", "fallback refreshes cache");

  fs.mkdirSync(RECEIPT_DIR, { recursive: true });
  const receiptPath = path.join(RECEIPT_DIR, "cargo-metadata-fallback-coverage.json");
  fs.writeFileSync(
    receiptPath,
    `${JSON.stringify(
      {
        schema: "dx.tauri.cargo_metadata_fallback_coverage",
        checked_file: path.relative(REPO_ROOT, RUST_TESTS).replaceAll("\\", "/"),
        required_tests: requiredTests.map(([testName]) => testName),
      },
      null,
      2,
    )}\n`,
  );
  console.log(`cargo metadata fallback coverage receipt=${receiptPath}`);
}

main();
