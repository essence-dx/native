#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const fs = require("node:fs");
const path = require("node:path");

const REPO_ROOT = path.resolve(__dirname, "..", "..");
const PUBLIC_TAURI_UTILS_FEATURES =
  "--no-default-features --features build,build-2,compression,schema,isolation,process-relaunch-dangerous-allow-symlink-macos,config-json5,config-toml,resources,html-manipulation,html-manipulation-2";

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
  const rootCargo = readRepoFile("Cargo.toml");
  const tauriUtilsCargo = readRepoFile("crates/tauri-utils/Cargo.toml");
  const workflow = readRepoFile(".github/workflows/test-core.yml");

  assertContains(rootCargo, 'rust-version = "1.77.2"', "Cargo.toml");
  assertContains(
    tauriUtilsCargo,
    'serializer = { package = "dx-serializer", version = "0.1.0", path = "../../../serializer"',
    "crates/tauri-utils/Cargo.toml",
  );
  assertContains(tauriUtilsCargo, 'dx-machine-cache = ["dep:serializer", "dep:rkyv"]', "crates/tauri-utils/Cargo.toml");
  assertContains(
    tauriUtilsCargo,
    'dx-machine-cache-mmap = ["dx-machine-cache", "serializer/typed-cache-mmap"]',
    "crates/tauri-utils/Cargo.toml",
  );

  assertContains(workflow, "check DX machine cache MSRV boundary", ".github/workflows/test-core.yml");
  assertContains(workflow, "node .scripts/ci/test-dx-machine-cache-msrv-boundary.ts", ".github/workflows/test-core.yml");
  assertContains(
    workflow,
    `tauri_utils_args: '${PUBLIC_TAURI_UTILS_FEATURES}'`,
    ".github/workflows/test-core.yml",
  );
  assertContains(
    workflow,
    "cargo ${{ matrix.platform.command }} --target ${{ matrix.platform.target }} ${{ matrix.features.tauri_utils_args }} --lib --bins --tests --manifest-path crates/tauri-utils/Cargo.toml",
    ".github/workflows/test-core.yml",
  );
  assertContains(
    workflow,
    "cross ${{ matrix.platform.command }} --target ${{ matrix.platform.target }} ${{ matrix.features.tauri_utils_args }} --lib --bins --tests --manifest-path crates/tauri-utils/Cargo.toml",
    ".github/workflows/test-core.yml",
  );
  assertNotContains(
    workflow,
    "matrix.features.args }} --lib --bins --tests --manifest-path crates/tauri-utils/Cargo.toml",
    ".github/workflows/test-core.yml",
  );
  assertContains(
    workflow,
    "tauri-utils MSRV all-feature lanes intentionally exclude local-only DX machine-cache features",
    ".github/workflows/test-core.yml",
  );

  console.log("machine-cache MSRV boundary ok");
}

main();
