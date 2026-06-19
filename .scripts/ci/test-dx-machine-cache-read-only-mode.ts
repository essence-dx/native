#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const fs = require("node:fs");
const path = require("node:path");

const REPO_ROOT = path.resolve(__dirname, "..", "..");
const TEST_OUTPUT_ROOT = path.resolve("G:\\Dx\\test-outputs");
const RECEIPT_DIR = path.join(TEST_OUTPUT_ROOT, "tauri-stage5-read-only-cache-20260530-a");

const WRITE_GUARDS = [
  {
    file: "crates/tauri-cli/src/helpers/cargo_manifest.rs",
    writeCalls: ["write_cargo_metadata_machine_cache(tauri_dir, &parsed.0, &parsed.1)"],
    snippets: [
      "cargo_metadata_machine_cache_enabled()",
      "tauri_utils::config::machine_cache::machine_cache_writes_enabled()",
      "write_cargo_metadata_machine_cache",
    ],
  },
  {
    file: "crates/tauri-cli/src/helpers/config.rs",
    writeCalls: [
      "package_version_machine_cache::write_package_version_machine_cache(candidate, version)",
    ],
    snippets: [
      "package_version_machine_cache::machine_cache_enabled()",
      "tauri_utils::config::machine_cache::machine_cache_writes_enabled()",
      "write_package_version_machine_cache",
    ],
  },
  {
    file: "crates/tauri-cli/src/interface/rust.rs",
    writeCalls: [
      "cargo_metadata_machine_cache::write(tauri_dir, &metadata)",
      "cargo_settings_machine_cache::write(dir, &settings)",
      "workspace_machine_cache::write(tauri_dir, &workspace_dir)",
    ],
    snippets: [
      "cargo_metadata_machine_cache::enabled()",
      "cargo_settings_machine_cache::enabled()",
      "workspace_machine_cache::enabled()",
      "tauri_utils::config::machine_cache::machine_cache_writes_enabled()",
    ],
  },
  {
    file: "crates/tauri-cli/src/interface/rust/cargo_config.rs",
    writeCalls: ["machine_cache::write(path, &config)"],
    snippets: [
      "machine_cache::enabled()",
      "tauri_utils::config::machine_cache::machine_cache_writes_enabled()",
      "machine_cache::write",
    ],
  },
];

const BEHAVIOR_TESTS = [
  {
    file: "crates/tauri-utils/src/config/machine_cache.rs",
    tests: [
      "dx_machine_cache_write_env_disables_automatic_config_writes",
      "dx_machine_cache_cached_project_read_is_read_only",
      "dx_machine_cache_project_config_projection_write_env_zero_still_reads_hit",
    ],
  },
  {
    file: "crates/tauri-cli/src/helpers/cargo_manifest.rs",
    tests: [
      "dx_cargo_metadata_cache_write_env_zero_still_reads_hit",
      "dx_cargo_metadata_cache_write_env_zero_does_not_write_on_miss",
    ],
  },
  {
    file: "crates/tauri-cli/src/helpers/config.rs",
    tests: [
      "dx_package_version_machine_cache_write_env_zero_still_reads_hit",
      "dx_package_version_machine_cache_write_env_zero_does_not_write_on_miss",
    ],
  },
  {
    file: "crates/tauri-cli/src/interface/rust/cargo_config.rs",
    tests: [
      "dx_cargo_config_machine_cache_write_env_zero_still_reads_hit",
      "dx_cargo_config_machine_cache_write_env_zero_does_not_write_on_miss",
    ],
  },
  {
    file: "crates/tauri-cli/src/interface/rust/machine_cache_tests.rs",
    tests: [
      "dx_cargo_settings_cache_write_env_zero_still_reads_hit",
      "dx_cargo_settings_cache_write_env_zero_does_not_write_on_miss",
      "dx_workspace_dir_machine_cache_write_env_zero_still_reads_hit",
      "dx_workspace_dir_machine_cache_write_env_zero_does_not_write_on_miss",
      "dx_full_cargo_metadata_machine_cache_write_env_zero_still_reads_hit",
      "dx_full_cargo_metadata_machine_cache_write_env_zero_does_not_write_on_miss",
    ],
  },
];

function readRepoFile(relativePath) {
  return fs.readFileSync(path.join(REPO_ROOT, relativePath), "utf8");
}

function assertContains(text, needle, label) {
  if (!text.includes(needle)) {
    throw new Error(`${label} is missing: ${needle}`);
  }
}

function assertOrdered(text, first, second, label) {
  const firstIndex = text.indexOf(first);
  const secondIndex = text.indexOf(second);
  if (firstIndex < 0 || secondIndex < 0 || firstIndex > secondIndex) {
    throw new Error(`${label} must contain ${first} before ${second}`);
  }
}

function main() {
  const configCache = readRepoFile("crates/tauri-utils/src/config/machine_cache.rs");
  assertContains(configCache, 'pub const TAURI_DX_MACHINE_CACHE_WRITE_ENV: &str = "TAURI_DX_MACHINE_CACHE_WRITE";', "tauri-utils cache env");
  assertContains(configCache, "pub fn machine_cache_writes_enabled() -> bool", "tauri-utils cache env");
  assertContains(configCache, "&& machine_cache_writes_enabled()", "tauri-utils automatic writes");

  for (const guard of WRITE_GUARDS) {
    const source = readRepoFile(guard.file);
    for (const snippet of guard.snippets) {
      assertContains(source, snippet, guard.file);
    }
    for (const writeCall of guard.writeCalls) {
      assertOrdered(
        source,
        "tauri_utils::config::machine_cache::machine_cache_writes_enabled()",
        writeCall,
        guard.file,
      );
    }
  }

  for (const behavior of BEHAVIOR_TESTS) {
    const source = readRepoFile(behavior.file);
    for (const testName of behavior.tests) {
      assertContains(source, `fn ${testName}()`, behavior.file);
    }
  }

  const runner = readRepoFile(".scripts/ci/run-dx-no-build-benchmark.ts");
  assertContains(runner, 'env.TAURI_DX_MACHINE_CACHE_WRITE = "0";', "benchmark runner read-only env");
  assertContains(runner, "tauri_dx_machine_cache_write_env", "benchmark runner sample evidence");
  assertContains(runner, "machine_cache_command_writes_disabled: true", "benchmark runner provenance");
  assertContains(runner, 'machine_cache_write_env_for_cache_on: "0"', "benchmark runner provenance");

  const checker = readRepoFile(".scripts/ci/check-dx-benchmark-manifest.ts");
  assertContains(checker, "tauri_dx_machine_cache_write_env", "benchmark checker sample evidence");
  assertContains(checker, "expectedCacheWriteEnvValue", "benchmark checker write env policy");
  assertContains(checker, "machine_cache_command_writes_disabled", "benchmark checker provenance");

  fs.mkdirSync(RECEIPT_DIR, { recursive: true });
  fs.writeFileSync(
    path.join(RECEIPT_DIR, "read-only-cache-coverage.json"),
    `${JSON.stringify(
      {
        schema: "dx.tauri.machine_cache_read_only_coverage",
        checked_at: new Date().toISOString(),
        repo_root: REPO_ROOT,
        write_guards: WRITE_GUARDS,
        behavior_tests: BEHAVIOR_TESTS,
      },
      null,
      2,
    )}\n`,
  );

  console.log(
    `checked read-only cache write guards for ${WRITE_GUARDS.length} CLI sources and behavior tests in ${BEHAVIOR_TESTS.length} files`,
  );
}

main();
