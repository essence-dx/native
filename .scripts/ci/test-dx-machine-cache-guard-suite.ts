#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const fs = require("node:fs");
const path = require("node:path");

const REPO_ROOT = path.resolve(__dirname, "..", "..");
const GUARD_DIR = path.join(REPO_ROOT, ".scripts", "ci");

const REQUIRED_RISKS = [
  "broad-claim",
  "release-boundary",
  "msrv-boundary",
  "untracked-cache-surface",
  "write-disabled-timing",
  "read-order",
  "projection-validation",
  "negative-config-timing",
];

const GUARDS = [
  {
    risk: "broad-claim",
    file: ".scripts/ci/test-dx-benchmark-manifest-sample-invocation.ts",
  },
  {
    risk: "broad-claim",
    file: ".scripts/ci/test-dx-machine-cache-docs.ts",
  },
  {
    risk: "broad-claim",
    file: ".scripts/ci/test-dx-machine-cache-info-app-config-guard.ts",
  },
  {
    risk: "negative-config-timing",
    file: ".scripts/ci/test-dx-machine-cache-docs.ts",
  },
  {
    risk: "release-boundary",
    file: ".scripts/ci/test-dx-machine-cache-release-boundary.ts",
  },
  {
    risk: "msrv-boundary",
    file: ".scripts/ci/test-dx-machine-cache-msrv-boundary.ts",
  },
  {
    risk: "projection-validation",
    file: ".scripts/ci/test-dx-machine-cache-projection-validation.ts",
  },
  {
    risk: "write-disabled-timing",
    file: ".scripts/ci/test-dx-machine-cache-read-only-mode.ts",
  },
  {
    risk: "write-disabled-timing",
    file: ".scripts/ci/test-dx-machine-cache-info-app-config-guard.ts",
  },
  {
    risk: "read-order",
    file: ".scripts/ci/test-dx-machine-cache-read-order.ts",
  },
  {
    risk: "untracked-cache-surface",
    file: ".scripts/ci/test-dx-machine-cache-mmap-coverage.ts",
  },
  {
    risk: "untracked-cache-surface",
    file: ".scripts/ci/test-dx-machine-cache-source-matching.ts",
  },
  {
    risk: "untracked-cache-surface",
    file: ".scripts/ci/test-dx-machine-cache-surface-matrix.ts",
  },
  {
    risk: "untracked-cache-surface",
    file: ".scripts/ci/test-dx-machine-cache-guard-suite.ts",
  },
];

function readRepoFile(relativePath) {
  return fs.readFileSync(path.join(REPO_ROOT, relativePath), "utf8");
}

function assertContains(source, needle, label) {
  if (!source.includes(needle)) {
    throw new Error(`${label} is missing: ${needle}`);
  }
}

function assertUnique(values, label) {
  const seen = new Set();
  for (const value of values) {
    if (seen.has(value)) {
      throw new Error(`${label} contains duplicate entry: ${value}`);
    }
    seen.add(value);
  }
}

function discoverMachineCacheGuardScripts() {
  return fs
    .readdirSync(GUARD_DIR)
    .filter((entry) => entry.startsWith("test-dx-machine-cache-") && entry.endsWith(".ts"))
    .map((entry) => `.scripts/ci/${entry}`)
    .sort();
}

function main() {
  const registeredEntries = GUARDS.map((guard) => `${guard.risk}:${guard.file}`).sort();
  const registeredFiles = [...new Set(GUARDS.map((guard) => guard.file))].sort();
  const discoveredFiles = discoverMachineCacheGuardScripts();
  assertUnique(registeredEntries, "DX machine-cache guard registry");

  for (const risk of REQUIRED_RISKS) {
    if (!GUARDS.some((guard) => guard.risk === risk)) {
      throw new Error(`DX machine-cache guard suite has no guard for risk: ${risk}`);
    }
  }

  for (const file of discoveredFiles) {
    if (!registeredFiles.includes(file)) {
      throw new Error(`DX machine-cache guard is not registered: ${file}`);
    }
  }
  for (const file of registeredFiles) {
    if (!fs.existsSync(path.join(REPO_ROOT, file))) {
      throw new Error(`registered DX machine-cache guard does not exist: ${file}`);
    }
  }

  assertContains(
    readRepoFile(".github/workflows/test-core.yml"),
    "node .scripts/ci/test-dx-machine-cache-msrv-boundary.ts",
    ".github/workflows/test-core.yml",
  );
  assertContains(
    readRepoFile(".github/workflows/covector-version-or-publish.yml"),
    "node .scripts/ci/test-dx-machine-cache-release-boundary.ts",
    ".github/workflows/covector-version-or-publish.yml",
  );
  assertContains(
    readRepoFile(".changes/config.json"),
    "DX_MACHINE_CACHE_RELEASE_BOUNDARY_MODE=publish node .scripts/ci/test-dx-machine-cache-release-boundary.ts",
    ".changes/config.json",
  );
  assertContains(readRepoFile("PLAN.md"), "must not claim broad Tauri superiority", "PLAN.md");
  assertContains(
    readRepoFile("crates/tauri-cli/ENVIRONMENT_VARIABLES.md"),
    "does not prove broad Tauri CLI, app runtime, WebView, IPC, build, bundle, or installer speedups",
    "crates/tauri-cli/ENVIRONMENT_VARIABLES.md",
  );

  console.log(`machine-cache guard suite ok; registered=${registeredFiles.length}`);
}

main();
