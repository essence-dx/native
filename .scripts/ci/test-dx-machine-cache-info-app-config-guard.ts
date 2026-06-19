#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const fs = require("node:fs");
const path = require("node:path");

const REPO_ROOT = path.resolve(__dirname, "..", "..");

const INFO_MOD = "crates/tauri-cli/src/info/mod.rs";
const INFO_APP = "crates/tauri-cli/src/info/app.rs";
const INFO_TEST = "crates/tauri-cli/src/info/app_config_machine_cache_tests.rs";
const CONFIG_HELPER = "crates/tauri-cli/src/helpers/config.rs";
const PLAN = "PLAN.md";

function readRepoFile(relativePath) {
  return fs.readFileSync(path.join(REPO_ROOT, relativePath), "utf8");
}

function assertContains(source, needle, label) {
  if (!source.includes(needle)) {
    throw new Error(`${label} is missing: ${needle}`);
  }
}

function assertNotContains(source, needle, label) {
  if (source.includes(needle)) {
    throw new Error(`${label} must not contain: ${needle}`);
  }
}

function main() {
  const infoMod = readRepoFile(INFO_MOD);
  const infoApp = readRepoFile(INFO_APP);
  const infoTest = readRepoFile(INFO_TEST);
  const configHelper = readRepoFile(CONFIG_HELPER);
  const plan = readRepoFile(PLAN);

  assertContains(infoMod, "fn app_config_section_items(", INFO_MOD);
  assertContains(infoMod, "fn app_config_section(", INFO_MOD);
  assertContains(
    infoMod,
    "crate::helpers::config::get_config(Target::current(), &[], tauri_dir)?",
    INFO_MOD,
  );
  assertContains(infoMod, "app::items(&config, frontend_dir)", INFO_MOD);
  assertContains(infoMod, "frontend_dir.as_deref()", INFO_MOD);
  assertContains(infoApp, "pub(super) fn items(", INFO_APP);
  assertContains(infoApp, "Option<&Path>", INFO_APP);

  assertContains(
    infoTest,
    "dx_info_app_config_section_uses_warm_machine_cache_without_spawning",
    INFO_TEST,
  );
  assertContains(infoTest, "project-config.machine", INFO_TEST);
  assertContains(infoTest, "package_version_machine_path", INFO_TEST);
  assertContains(infoTest, "TAURI_DX_MACHINE_CACHE_WRITE_ENV", INFO_TEST);
  assertContains(infoTest, 'Some("0")', INFO_TEST);
  assertContains(infoTest, "package_version_machine_cache_read_hits_for_tests()", INFO_TEST);
  assertContains(infoTest, "assert_eq!(signature.action_count, 0)", INFO_TEST);
  assertContains(infoTest, "project_before", INFO_TEST);
  assertContains(infoTest, "read_file(&fixture.project_config_machine_path)", INFO_TEST);
  assertContains(infoTest, "package_before", INFO_TEST);
  assertContains(infoTest, "read_file(&fixture.package_version_machine_path)", INFO_TEST);
  assertNotContains(infoTest, '"PATH"', INFO_TEST);
  assertNotContains(infoTest, '"Path"', INFO_TEST);

  assertContains(
    configHelper,
    "pub(crate) const TAURI_DX_MACHINE_CACHE_ENV_FOR_TESTS",
    CONFIG_HELPER,
  );
  assertContains(
    configHelper,
    "pub(crate) fn package_version_machine_cache_read_hits_for_tests()",
    CONFIG_HELPER,
  );

  assertContains(
    plan,
    "Stage 55 is a no-spawn isolated `tauri info` app/config section harness",
    PLAN,
  );
  assertContains(
    plan,
    "without the full command's environment probes, network lookups, package-manager calls, or mobile/toolchain noise",
    PLAN,
  );
  assertContains(plan, "not a full `tauri info` command benchmark", PLAN);
  assertContains(plan, "not a full CLI, official-release, upstream-source", PLAN);
  assertContains(plan, "default-on, release-readiness", PLAN);
  assertContains(plan, "product-level claim", PLAN);

  console.log("info app/config machine-cache guard ok");
}

main();
