#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const TEST_OUTPUT_ROOT = path.resolve("G:\\Dx\\test-outputs");
const RECEIPT_ROOT = path.join(TEST_OUTPUT_ROOT, "tauri-stage4-comparison-prep-20260530-a");
const FIXTURE_DIR = path.join(RECEIPT_ROOT, "fixture-app");
const BIN_DIR = path.join(RECEIPT_ROOT, "fake-binaries");
const OUT_DIR = path.join(RECEIPT_ROOT, "prepared-comparison");
const REPO_ROOT = path.resolve(__dirname, "..", "..");
const PREPARER = path.join(REPO_ROOT, ".scripts", "ci", "prepare-dx-current-source-comparison.ts");
const RUNNER = path.join(REPO_ROOT, ".scripts", "ci", "run-dx-no-build-benchmark.ts");
const LOCAL_BUILD_RECEIPT_FILE = "local-current-source-build-receipt.json";

function assertUnderTestOutputs(candidate) {
  const resolved = path.resolve(candidate);
  const rootWithSep = TEST_OUTPUT_ROOT.endsWith(path.sep) ? TEST_OUTPUT_ROOT : `${TEST_OUTPUT_ROOT}${path.sep}`;
  if (resolved !== TEST_OUTPUT_ROOT && !resolved.toLowerCase().startsWith(rootWithSep.toLowerCase())) {
    throw new Error(`test path must stay under ${TEST_OUTPUT_ROOT}: ${resolved}`);
  }
  return resolved;
}

function resetDir(dir) {
  const safeDir = assertUnderTestOutputs(dir);
  fs.rmSync(safeDir, { recursive: true, force: true });
  fs.mkdirSync(safeDir, { recursive: true });
}

function sha256File(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function fileRecord(file) {
  const stat = fs.statSync(file);
  return {
    path: file,
    bytes: stat.size,
    sha256: sha256File(file),
    last_write_utc: stat.mtime.toISOString(),
  };
}

function parseCsvRows(text) {
  const rows = [];
  let row = [];
  let field = "";
  let inQuotes = false;

  for (let index = 0; index < text.length; index += 1) {
    const char = text[index];
    const next = text[index + 1];
    if (inQuotes) {
      if (char === '"' && next === '"') {
        field += '"';
        index += 1;
      } else if (char === '"') {
        inQuotes = false;
      } else {
        field += char;
      }
    } else if (char === '"') {
      inQuotes = true;
    } else if (char === ",") {
      row.push(field);
      field = "";
    } else if (char === "\n") {
      row.push(field);
      rows.push(row);
      row = [];
      field = "";
    } else if (char !== "\r") {
      field += char;
    }
  }

  if (field.length > 0 || row.length > 0) {
    row.push(field);
    rows.push(row);
  }

  return rows.filter((entry) => entry.some((value) => value !== ""));
}

function parseCsv(text) {
  const [headers, ...rows] = parseCsvRows(text);
  return rows.map((row) => Object.fromEntries(headers.map((header, index) => [header, row[index] ?? ""])));
}

function escapeCsvCell(value) {
  const text = String(value ?? "");
  return /[",\r\n]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
}

function writeCsvRows(file, rows) {
  fs.writeFileSync(file, `${rows.map((row) => row.map(escapeCsvCell).join(",")).join("\n")}\n`, "utf8");
}

function corruptFirstSha256(sourceCsv, targetCsv) {
  const rows = parseCsvRows(fs.readFileSync(sourceCsv, "utf8").replace(/^\uFEFF/, ""));
  const headers = rows[0];
  const shaIndex = headers.indexOf("sha256");
  if (shaIndex === -1 || rows.length < 2) throw new Error("cannot corrupt sha256 in case config");
  rows[1][shaIndex] = "0".repeat(64);
  writeCsvRows(targetCsv, rows);
}

function writeFakeFile(file, label) {
  fs.writeFileSync(file, `fake ${label}\n`, "utf8");
}

function writeLocalBuildReceipt(file, localRelease) {
  const stat = fs.statSync(localRelease);
  const buildLog = path.join(path.dirname(file), "local-current-source-build.log");
  fs.writeFileSync(
    buildLog,
    [
      "   Compiling tauri-cli v2.11.2 (G:\\Dx\\tauri\\crates\\tauri-cli)",
      "    Finished `release` profile [optimized] target(s) in 1s",
      "",
    ].join("\n"),
    "utf8",
  );
  fs.writeFileSync(
    file,
    `${JSON.stringify(
      {
        schema: "dx.tauri.local_release_build_receipt.v1",
        target: "local-current-source",
        release_build_run: true,
        command: "cargo build -p tauri-cli --features dx-machine-cache-mmap --release --bin cargo-tauri --color never",
        argv: [
          "cargo",
          "build",
          "-p",
          "tauri-cli",
          "--features",
          "dx-machine-cache-mmap",
          "--release",
          "--bin",
          "cargo-tauri",
          "--color",
          "never",
        ],
        cwd: REPO_ROOT,
        repo_root: REPO_ROOT,
        profile: "release",
        package: "tauri-cli",
        binary_name: "cargo-tauri",
        features: ["dx-machine-cache-mmap"],
        git: {
          branch: "dev",
          head_sha: "b".repeat(40),
          dirty: false,
          status_short: [],
        },
        binary: {
          path: localRelease,
          bytes: stat.size,
          sha256: sha256File(localRelease),
          last_write_utc: stat.mtime.toISOString(),
        },
        build_log: fileRecord(buildLog),
      },
      null,
      2,
    )}\n`,
    "utf8",
  );
}

function assertRunnerRequiresPreGeneratedMachineArtifacts() {
  const runnerPath = path.join(REPO_ROOT, ".scripts", "ci", "run-dx-no-build-benchmark.ts");
  const source = fs.readFileSync(runnerPath, "utf8");
  const snapshotIndex = source.indexOf("const cacheBefore = snapshotMachineArtifacts(fixture);");
  const warmupIndex = source.indexOf("for (const row of cases) {\n    for (let warmup");
  if (snapshotIndex === -1 || warmupIndex === -1 || snapshotIndex > warmupIndex) {
    throw new Error("runner must snapshot .machine artifacts before warmups so generation time is excluded");
  }
  for (const requiredText of [
    "pre-generated .machine cache artifacts are required before benchmark warmups",
    ".machine cache artifacts changed during ${phase}",
    "assertStableMachineArtifacts(cacheBefore, snapshotMachineArtifacts(fixture), \"warmups\")",
    "assertStableMachineArtifacts(cacheBefore, cacheAfter, \"sample timing\")",
    "machine_cache_generation_measured: false",
    "machine_cache_command_writes_disabled: true",
    "TAURI_DX_MACHINE_CACHE_WRITE = \"0\"",
  ]) {
    if (!source.includes(requiredText)) throw new Error(`runner is missing pre-generated cache guard: ${requiredText}`);
  }
}

function main() {
  assertRunnerRequiresPreGeneratedMachineArtifacts();
  fs.mkdirSync(RECEIPT_ROOT, { recursive: true });
  resetDir(FIXTURE_DIR);
  resetDir(BIN_DIR);
  resetDir(OUT_DIR);
  fs.mkdirSync(FIXTURE_DIR, { recursive: true });
  fs.mkdirSync(BIN_DIR, { recursive: true });
  fs.writeFileSync(path.join(FIXTURE_DIR, "tauri.conf.json"), '{"productName":"DX Cache Fixture"}\n', "utf8");

  const paths = {
    globalTauri: path.join(BIN_DIR, "global-tauri.exe"),
    node: path.join(BIN_DIR, "node.exe"),
    tauriScript: path.join(BIN_DIR, "tauri.js"),
    officialRelease: path.join(BIN_DIR, "official-cargo-tauri.exe"),
    localRelease: path.join(BIN_DIR, "local-cargo-tauri.exe"),
    localBuildReceipt: path.join(BIN_DIR, LOCAL_BUILD_RECEIPT_FILE),
  };
  for (const [key, file] of Object.entries(paths)) {
    if (key === "localBuildReceipt") continue;
    writeFakeFile(file, key);
  }
  writeLocalBuildReceipt(paths.localBuildReceipt, paths.localRelease);

  const result = spawnSync(
    process.execPath,
    [
      PREPARER,
      "--out",
      OUT_DIR,
      "--fixture",
      FIXTURE_DIR,
      "--global-tauri",
      paths.globalTauri,
      "--node",
      paths.node,
      "--node-tauri-script",
      paths.tauriScript,
      "--official-release",
      paths.officialRelease,
      "--local-release",
      paths.localRelease,
      "--local-release-build-receipt",
      paths.localBuildReceipt,
    ],
    { cwd: REPO_ROOT, encoding: "utf8" },
  );
  fs.writeFileSync(
    path.join(RECEIPT_ROOT, "comparison-prep-test.log"),
    [result.stdout, result.stderr, `exit_code=${result.status}`, `signal=${result.signal ?? ""}`, ""].join("\n"),
    "utf8",
  );
  if (result.status !== 0) {
    throw new Error(`comparison preparer failed\n${result.stderr}`);
  }

  const planPath = path.join(OUT_DIR, "stage4-config-receipt.json");
  const binariesPath = path.join(OUT_DIR, "binaries.csv");
  const reportPath = path.join(OUT_DIR, "REPORT.md");
  const copiedBuildReceiptPath = path.join(OUT_DIR, LOCAL_BUILD_RECEIPT_FILE);
  const copiedBuildLogPath = path.join(OUT_DIR, "local-current-source-build.log");
  for (const file of [planPath, binariesPath, reportPath, copiedBuildReceiptPath]) {
    if (!fs.existsSync(file)) throw new Error(`expected preparer output is missing: ${file}`);
  }
  if (!fs.existsSync(copiedBuildLogPath)) {
    throw new Error(`comparison plan did not copy the local current-source build log: ${copiedBuildLogPath}`);
  }

  const plan = JSON.parse(fs.readFileSync(planPath, "utf8"));
  if (plan.schema !== "dx.tauri.current_source_comparison_plan.v1") {
    throw new Error(`unexpected plan schema: ${plan.schema}`);
  }
  if (
    plan.mode !== "config_only_no_build_no_benchmark" ||
    plan.no_build !== true ||
    plan.no_benchmark_run !== true ||
    plan.build_run !== false ||
    plan.benchmark_run !== false ||
    plan.current_source_release_measured !== false
  ) {
    throw new Error("comparison plan must prove it did not build or benchmark");
  }
  if (plan.claim_gates.faster_than_upstream_claim_allowed !== false) {
    throw new Error("comparison prep must not allow faster-than-upstream claims");
  }
  if (
    plan.benchmark_surface !== "inspect-wix-upgrade-code" ||
    plan.benchmark_command !== "inspect wix-upgrade-code" ||
    !Array.isArray(plan.benchmark_command_args)
  ) {
    throw new Error("comparison plan did not preserve the default allowlisted benchmark surface");
  }
  if (path.resolve(plan.case_config) !== path.resolve(binariesPath)) {
    throw new Error("comparison plan case_config does not point at binaries.csv");
  }
  if (plan.binary_identity_checks.local_current_source_cache_on_off_same_binary !== true) {
    throw new Error("comparison plan did not prove local cache on/off rows share one binary identity");
  }
  if (!plan.git || !plan.git.repo_root || !Array.isArray(plan.git.status_short)) {
    throw new Error("comparison plan must include lightweight git metadata");
  }
  if (!plan.source_paths || path.resolve(plan.source_paths.local_release) !== path.resolve(paths.localRelease)) {
    throw new Error("comparison plan source_paths.local_release does not match the selected local binary");
  }
  if (
    !plan.local_current_source_build_receipt ||
    path.resolve(plan.local_current_source_build_receipt.path) !== path.resolve(copiedBuildReceiptPath)
  ) {
    throw new Error("comparison plan did not preserve the local current-source build receipt path");
  }
  if (
    !plan.local_current_source_build_receipt.build_log ||
    path.resolve(plan.local_current_source_build_receipt.build_log.path) !== path.resolve(copiedBuildLogPath)
  ) {
    throw new Error("comparison plan did not preserve copied local build-log evidence");
  }
  const copiedBuildReceipt = JSON.parse(fs.readFileSync(copiedBuildReceiptPath, "utf8"));
  if (
    !copiedBuildReceipt.build_log ||
    path.resolve(copiedBuildReceipt.build_log.path) !== path.resolve(copiedBuildLogPath) ||
    copiedBuildReceipt.build_log.sha256 !== sha256File(copiedBuildLogPath)
  ) {
    throw new Error("copied local build receipt did not rewrite build_log to the copied log");
  }
  if (!plan.runner_command.includes("--local-release-build-receipt")) {
    throw new Error("comparison plan runner command does not pass the local build receipt to the benchmark runner");
  }
  if (!plan.runner_command.includes("--benchmark-surface") || !plan.runner_command.includes("inspect-wix-upgrade-code")) {
    throw new Error("comparison plan runner command does not pass the benchmark surface to the benchmark runner");
  }

  const rows = parseCsv(fs.readFileSync(binariesPath, "utf8"));
  const keys = rows.map((row) => `${row.target}/${row.cache_env}/${row.kind}`).sort();
  const expectedKeys = [
    "local-current-source/off/direct-rust",
    "local-current-source/on/direct-rust",
    "official-global-tauri/off/direct-cli",
    "official-node-tauri-js/off/node-wrapper",
    "official-release-binary/off/direct-rust",
  ];
  if (keys.join("\n") !== expectedKeys.join("\n")) {
    throw new Error(`unexpected comparison cases:\n${keys.join("\n")}`);
  }

  for (const row of rows) {
    if (row.sha256 !== sha256File(row.path)) {
      throw new Error(`binary sha256 mismatch for ${row.target}/${row.cache_env}`);
    }
    if (row.script && row.script_sha256 !== sha256File(row.script)) {
      throw new Error(`script sha256 mismatch for ${row.target}/${row.cache_env}`);
    }
  }

  const localRows = rows.filter((row) => row.target === "local-current-source");
  if (localRows.length !== 2 || localRows[0].path !== localRows[1].path || localRows[0].sha256 !== localRows[1].sha256) {
    throw new Error("local cache on/off rows must point to the same current-source binary identity");
  }

  const runnerPlanOut = path.join(RECEIPT_ROOT, "runner-plan-with-build-receipt");
  resetDir(runnerPlanOut);
  const runnerPlanResult = spawnSync(
    process.execPath,
    [
      RUNNER,
      "--plan-only",
      "--fixture",
      FIXTURE_DIR,
      "--case-config",
      binariesPath,
      "--out",
      runnerPlanOut,
      "--local-release-build-receipt",
      copiedBuildReceiptPath,
    ],
    { cwd: REPO_ROOT, encoding: "utf8" },
  );
  fs.writeFileSync(
    path.join(RECEIPT_ROOT, "runner-plan-preserves-build-receipt.log"),
    [runnerPlanResult.stdout, runnerPlanResult.stderr, `exit_code=${runnerPlanResult.status}`, `signal=${runnerPlanResult.signal ?? ""}`, ""].join("\n"),
    "utf8",
  );
  if (runnerPlanResult.status !== 0) {
    throw new Error(`runner plan did not accept the local build receipt\n${runnerPlanResult.stderr}`);
  }
  const runnerCopiedReceipt = path.join(runnerPlanOut, LOCAL_BUILD_RECEIPT_FILE);
  const runnerCopiedBuildLog = path.join(runnerPlanOut, "local-current-source-build.log");
  if (!fs.existsSync(runnerCopiedReceipt)) {
    throw new Error("runner plan did not copy the local build receipt into its output");
  }
  if (!fs.existsSync(runnerCopiedBuildLog)) {
    throw new Error("runner plan did not copy the local build log into its output");
  }
  const runnerPlan = JSON.parse(fs.readFileSync(path.join(runnerPlanOut, "wave12-run-plan.json"), "utf8"));
  if (
    !runnerPlan.local_current_source_build_receipt ||
    path.resolve(runnerPlan.local_current_source_build_receipt.path) !== path.resolve(runnerCopiedReceipt)
  ) {
    throw new Error("runner plan did not record the copied local build receipt");
  }
  if (
    !runnerPlan.local_current_source_build_receipt.build_log ||
    path.resolve(runnerPlan.local_current_source_build_receipt.build_log.path) !== path.resolve(runnerCopiedBuildLog)
  ) {
    throw new Error("runner plan did not record the copied local build-log evidence");
  }
  if (runnerPlan.benchmark_surface !== "inspect-wix-upgrade-code" || runnerPlan.command !== "inspect wix-upgrade-code") {
    throw new Error("runner plan did not record the default benchmark surface");
  }

  const migrateRunnerPlanOut = path.join(RECEIPT_ROOT, "runner-plan-migrate-surface");
  resetDir(migrateRunnerPlanOut);
  const migrateRunnerPlanResult = spawnSync(
    process.execPath,
    [
      RUNNER,
      "--plan-only",
      "--fixture",
      FIXTURE_DIR,
      "--case-config",
      binariesPath,
      "--out",
      migrateRunnerPlanOut,
      "--benchmark-surface",
      "migrate-stable-v2-noop",
    ],
    { cwd: REPO_ROOT, encoding: "utf8" },
  );
  fs.writeFileSync(
    path.join(RECEIPT_ROOT, "runner-plan-migrate-surface.log"),
    [migrateRunnerPlanResult.stdout, migrateRunnerPlanResult.stderr, `exit_code=${migrateRunnerPlanResult.status}`, `signal=${migrateRunnerPlanResult.signal ?? ""}`, ""].join("\n"),
    "utf8",
  );
  if (migrateRunnerPlanResult.status !== 0) {
    throw new Error(`runner plan did not accept the migrate benchmark surface\n${migrateRunnerPlanResult.stderr}`);
  }
  const migrateRunnerPlan = JSON.parse(fs.readFileSync(path.join(migrateRunnerPlanOut, "wave12-run-plan.json"), "utf8"));
  if (migrateRunnerPlan.benchmark_surface !== "migrate-stable-v2-noop" || migrateRunnerPlan.command !== "migrate") {
    throw new Error("runner plan did not record the migrate benchmark surface");
  }

  console.log(`comparison prep receipt=${planPath}`);
  console.log(`comparison case config=${binariesPath}`);

  const missingResult = spawnSync(
    process.execPath,
    [
      PREPARER,
      "--out",
      path.join(RECEIPT_ROOT, "missing-local-release"),
      "--fixture",
      FIXTURE_DIR,
      "--global-tauri",
      paths.globalTauri,
      "--node",
      paths.node,
      "--node-tauri-script",
      paths.tauriScript,
      "--official-release",
      paths.officialRelease,
      "--local-release",
      path.join(BIN_DIR, "missing-local-release.exe"),
    ],
    { cwd: REPO_ROOT, encoding: "utf8" },
  );
  fs.writeFileSync(
    path.join(RECEIPT_ROOT, "comparison-prep-missing-local-release-test.log"),
    [missingResult.stdout, missingResult.stderr, `exit_code=${missingResult.status}`, `signal=${missingResult.signal ?? ""}`, ""].join("\n"),
    "utf8",
  );
  if (missingResult.status === 0 || !missingResult.stderr.includes("--local-release does not exist")) {
    throw new Error("comparison preparer did not reject a missing local current-source release binary");
  }

  const corruptCaseConfig = path.join(RECEIPT_ROOT, "corrupt-binaries.csv");
  corruptFirstSha256(binariesPath, corruptCaseConfig);
  const runnerResult = spawnSync(
    process.execPath,
    [
      path.join(REPO_ROOT, ".scripts", "ci", "run-dx-no-build-benchmark.ts"),
      "--plan-only",
      "--fixture",
      FIXTURE_DIR,
      "--case-config",
      corruptCaseConfig,
      "--out",
      path.join(RECEIPT_ROOT, "runner-rejects-corrupt-case-config"),
    ],
    { cwd: REPO_ROOT, encoding: "utf8" },
  );
  fs.writeFileSync(
    path.join(RECEIPT_ROOT, "runner-rejects-corrupt-case-config.log"),
    [runnerResult.stdout, runnerResult.stderr, `exit_code=${runnerResult.status}`, `signal=${runnerResult.signal ?? ""}`, ""].join("\n"),
    "utf8",
  );
  if (runnerResult.status === 0 || !runnerResult.stderr.includes("case binary sha256 mismatch")) {
    throw new Error("no-build runner did not reject a case config with a stale binary sha256");
  }

  const rootOutResult = spawnSync(
    process.execPath,
    [
      PREPARER,
      "--out",
      TEST_OUTPUT_ROOT,
      "--fixture",
      FIXTURE_DIR,
      "--global-tauri",
      paths.globalTauri,
      "--node",
      paths.node,
      "--node-tauri-script",
      paths.tauriScript,
      "--official-release",
      paths.officialRelease,
      "--local-release",
      paths.localRelease,
    ],
    { cwd: REPO_ROOT, encoding: "utf8" },
  );
  fs.writeFileSync(
    path.join(RECEIPT_ROOT, "comparison-prep-rejects-output-root.log"),
    [rootOutResult.stdout, rootOutResult.stderr, `exit_code=${rootOutResult.status}`, `signal=${rootOutResult.signal ?? ""}`, ""].join("\n"),
    "utf8",
  );
  if (rootOutResult.status === 0 || !rootOutResult.stderr.includes("out must be a child directory")) {
    throw new Error("comparison preparer did not reject the test-output root as an output directory");
  }
}

main();
