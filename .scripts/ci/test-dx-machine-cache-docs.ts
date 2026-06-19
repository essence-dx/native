#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const fs = require("node:fs");
const path = require("node:path");

const REPO_ROOT = path.resolve(__dirname, "..", "..");
const DOC_PATH = path.join(REPO_ROOT, "crates", "tauri-cli", "ENVIRONMENT_VARIABLES.md");
const PLAN_PATH = path.join(REPO_ROOT, "PLAN.md");

function assertContains(source, needle) {
  if (!source.includes(needle)) {
    throw new Error(`machine-cache docs are missing: ${needle}`);
  }
}

function main() {
  const source = fs.readFileSync(DOC_PATH, "utf8");
  const plan = fs.readFileSync(PLAN_PATH, "utf8");
  const requiredSnippets = [
    "`TAURI_DX_MACHINE_CACHE`",
    "`TAURI_DX_MACHINE_CACHE_WRITE`",
    "`dx-machine-cache`",
    "`dx-machine-cache-mmap`",
    "default-off",
    "pre-generated `.machine`",
    "source files remain authoritative",
    "local `dx-serializer` path dependency",
    "Rust 1.85 / edition 2024",
    "derive crate also declares Rust 1.85 / edition 2024",
    "The current release policy is local-only",
    "not crates.io/MSRV-ready",
    "publish/default-on readiness",
    "does not prove broad Tauri CLI, app runtime, WebView, IPC, build, bundle, or installer speedups",
    "do not prove a generic config speedup or a 10x serializer claim",
  ];

  for (const snippet of requiredSnippets) {
    assertContains(source, snippet);
  }

  const requiredPlanSnippets = [
    "Stage 47's tiny full project-config fixture and Stage 48's tiny leaf-config fixture are negative timing results",
    "Stage 49's representative generated project-config fixture is positive at `45%` of source median, or about `2.22x` faster",
    "cannot support generic config speedup, broad Tauri superiority, or a 10x product-level win",
    "It must not claim broad Tauri superiority",
  ];

  for (const snippet of requiredPlanSnippets) {
    assertContains(plan, snippet);
  }

  console.log(`machine-cache docs ok: ${DOC_PATH}`);
}

main();
