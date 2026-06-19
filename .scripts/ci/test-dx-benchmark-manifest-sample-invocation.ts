#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const { spawnSync } = require("node:child_process");

const TEST_OUTPUT_ROOT = path.resolve("G:\\Dx\\test-outputs");
const FIXTURE_SOURCE = path.resolve(
  process.env.TAURI_DX_SAMPLE_INVOCATION_FIXTURE ||
    "G:\\Dx\\test-outputs\\tauri-dx-machine-cache-20260530-wave13-runner-smoke1",
);
const RECEIPT_ROOT = path.join(TEST_OUTPUT_ROOT, "tauri-stage3-semantic-validation-20260530-a");
const GREEN_INPUT_DIR = path.join(RECEIPT_ROOT, "valid-sample-invocation");
const GREEN_OUT_DIR = path.join(RECEIPT_ROOT, "valid-sample-invocation-check");
const CURRENT_SOURCE_INPUT_DIR = path.join(RECEIPT_ROOT, "valid-current-source-target");
const CURRENT_SOURCE_OUT_DIR = path.join(RECEIPT_ROOT, "valid-current-source-target-check");
const CURRENT_SOURCE_BUILD_RECEIPT_OUT_DIR = path.join(RECEIPT_ROOT, "valid-current-source-build-receipt-check");
const DIRTY_BUILD_RECEIPT_INPUT_DIR = path.join(RECEIPT_ROOT, "dirty-current-source-build-receipt");
const DIRTY_BUILD_RECEIPT_OUT_DIR = path.join(RECEIPT_ROOT, "dirty-current-source-build-receipt-check");
const MISSING_BUILD_LOG_INPUT_DIR = path.join(RECEIPT_ROOT, "missing-current-source-build-log");
const MISSING_BUILD_LOG_OUT_DIR = path.join(RECEIPT_ROOT, "missing-current-source-build-log-check");
const STALE_BUILD_LOG_INPUT_DIR = path.join(RECEIPT_ROOT, "stale-current-source-build-log");
const STALE_BUILD_LOG_OUT_DIR = path.join(RECEIPT_ROOT, "stale-current-source-build-log-check");
const WEAK_BUILD_LOG_INPUT_DIR = path.join(RECEIPT_ROOT, "weak-current-source-build-log");
const WEAK_BUILD_LOG_OUT_DIR = path.join(RECEIPT_ROOT, "weak-current-source-build-log-check");
const FAILED_BUILD_LOG_INPUT_DIR = path.join(RECEIPT_ROOT, "failed-current-source-build-log");
const FAILED_BUILD_LOG_OUT_DIR = path.join(RECEIPT_ROOT, "failed-current-source-build-log-check");
const DIRTY_RUN_PROVENANCE_INPUT_DIR = path.join(RECEIPT_ROOT, "dirty-run-provenance");
const DIRTY_RUN_PROVENANCE_OUT_DIR = path.join(RECEIPT_ROOT, "dirty-run-provenance-check");
const WRITE_ENV_RED_INPUT_DIR = path.join(RECEIPT_ROOT, "bad-cache-write-env");
const WRITE_ENV_RED_OUT_DIR = path.join(RECEIPT_ROOT, "bad-cache-write-env-check");
const SAMPLE_TIMEOUT_RED_INPUT_DIR = path.join(RECEIPT_ROOT, "bad-sample-timeout");
const SAMPLE_TIMEOUT_RED_OUT_DIR = path.join(RECEIPT_ROOT, "bad-sample-timeout-check");
const OUTPUT_REHASH_RED_INPUT_DIR = path.join(RECEIPT_ROOT, "bad-output-rehash");
const OUTPUT_REHASH_RED_OUT_DIR = path.join(RECEIPT_ROOT, "bad-output-rehash-check");
const MISSING_SUMMARY_RED_INPUT_DIR = path.join(RECEIPT_ROOT, "bad-missing-summary-row");
const MISSING_SUMMARY_RED_OUT_DIR = path.join(RECEIPT_ROOT, "bad-missing-summary-row-check");
const ARGV_RED_INPUT_DIR = path.join(RECEIPT_ROOT, "bad-argv-json");
const ARGV_RED_OUT_DIR = path.join(RECEIPT_ROOT, "bad-argv-json-check");
const CASE_ALLOWLIST_RED_INPUT_DIR = path.join(RECEIPT_ROOT, "bad-case-allowlist");
const CASE_ALLOWLIST_RED_OUT_DIR = path.join(RECEIPT_ROOT, "bad-case-allowlist-check");
const PROCESS_SWEEP_RED_INPUT_DIR = path.join(RECEIPT_ROOT, "bad-process-sweep");
const PROCESS_SWEEP_RED_OUT_DIR = path.join(RECEIPT_ROOT, "bad-process-sweep-check");
const PROCESS_SWEEP_MISSING_INPUT_DIR = path.join(RECEIPT_ROOT, "missing-process-sweep-json");
const PROCESS_SWEEP_MISSING_OUT_DIR = path.join(RECEIPT_ROOT, "missing-process-sweep-json-check");
const RED_INPUT_DIR = path.join(RECEIPT_ROOT, "bad-sample-invocation");
const RED_OUT_DIR = path.join(RECEIPT_ROOT, "bad-sample-invocation-check");
const REPO_ROOT = path.resolve(__dirname, "..", "..");
const CHECKER = path.join(REPO_ROOT, ".scripts", "ci", "check-dx-benchmark-manifest.ts");
const EXPECTED_SAMPLES_PER_CASE = 30;
const PROCESS_SWEEP_SCHEMA = "dx.tauri.process_sweep.v1";

function assertUnderTestOutputs(candidate) {
  const resolved = path.resolve(candidate);
  const rootWithSep = TEST_OUTPUT_ROOT.endsWith(path.sep) ? TEST_OUTPUT_ROOT : `${TEST_OUTPUT_ROOT}${path.sep}`;
  if (resolved !== TEST_OUTPUT_ROOT && !resolved.toLowerCase().startsWith(rootWithSep.toLowerCase())) {
    throw new Error(`test receipt path must stay under ${TEST_OUTPUT_ROOT}: ${resolved}`);
  }
  return resolved;
}

function resetDir(dir) {
  const safeDir = assertUnderTestOutputs(dir);
  fs.rmSync(safeDir, { recursive: true, force: true });
  fs.mkdirSync(safeDir, { recursive: true });
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

function escapeCsvCell(value) {
  const text = String(value ?? "");
  return /[",\r\n]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
}

function writeCsvRows(file, rows) {
  fs.writeFileSync(file, `${rows.map((row) => row.map(escapeCsvCell).join(",")).join("\n")}\n`, "utf8");
}

function readCsvObjects(file) {
  const [headers, ...rows] = parseCsvRows(fs.readFileSync(file, "utf8").replace(/^\uFEFF/, ""));
  return {
    headers,
    rows: rows.map((row) => Object.fromEntries(headers.map((header, index) => [header, row[index] ?? ""]))),
  };
}

function writeCsvObjects(file, headers, rows) {
  writeCsvRows(file, [headers, ...rows.map((row) => headers.map((header) => row[header] ?? ""))]);
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

function caseKey(row) {
  return `${row.target}\u0000${row.cache_env}\u0000${row.target_kind}`;
}

function cacheWriteEnvValue(cacheEnv) {
  return "0";
}

function casePolicy(row) {
  const key = `${row.target}\u0000${row.cache_env}\u0000${row.kind ?? row.target_kind}`;
  const policies = {
    "bun-global-tauri\u0000off\u0000node-wrapper": {
      role: "official_global_cli",
      source_kind: "prebuilt_global_package",
      invocation_kind: "direct-executable",
    },
    "node-direct-tauri-js\u0000off\u0000node-wrapper": {
      role: "official_node_cli_script",
      source_kind: "prebuilt_global_package",
      invocation_kind: "node-script",
    },
    "official-release-zip\u0000off\u0000direct-rust": {
      role: "official_release_baseline",
      source_kind: "official_release_binary",
      invocation_kind: "direct-executable",
    },
    "post6v-current-no-dx\u0000off\u0000direct-rust": {
      role: "legacy_current_source_no_dx",
      source_kind: "local_current_source_release_binary",
      invocation_kind: "direct-executable",
    },
    "post6v-dx-feature\u0000off\u0000direct-rust": {
      role: "local_current_source_cache_off",
      source_kind: "local_current_source_release_binary",
      invocation_kind: "direct-executable",
    },
    "post6v-dx-feature\u0000on\u0000direct-rust": {
      role: "local_current_source_cache_on",
      source_kind: "local_current_source_release_binary",
      invocation_kind: "direct-executable",
    },
    "local-current-source\u0000off\u0000direct-rust": {
      role: "local_current_source_cache_off",
      source_kind: "local_current_source_release_binary",
      invocation_kind: "direct-executable",
    },
    "local-current-source\u0000on\u0000direct-rust": {
      role: "local_current_source_cache_on",
      source_kind: "local_current_source_release_binary",
      invocation_kind: "direct-executable",
    },
  };
  return policies[key] ?? {
    role: "unreviewed",
    source_kind: "unreviewed",
    invocation_kind: "unreviewed",
  };
}

function sampleArgv(row, binaryRow) {
  const command = binaryRow.script
    ? [binaryRow.script, "inspect", "wix-upgrade-code"]
    : ["inspect", "wix-upgrade-code"];
  return JSON.stringify(command);
}

function writeProcessSweepJson(inputDir, phase) {
  const textLogName = `wave7-process-check-${phase}.log`;
  const jsonPath = path.join(inputDir, `wave7-process-check-${phase}.json`);
  fs.writeFileSync(
    jsonPath,
    `${JSON.stringify(
      {
        schema: PROCESS_SWEEP_SCHEMA,
        phase,
        captured_utc: "2026-05-31T00:00:00.000Z",
        self_pid: process.pid,
        roots: [REPO_ROOT, TEST_OUTPUT_ROOT],
        command_names: ["cargo", "rustc", "rustup", "node", "bun", "tauri", "cargo-tauri"],
        clean: true,
        exit_code: 0,
        text_log: textLogName,
        text_log_sha256: sha256File(path.join(inputDir, textLogName)),
        matched_processes: [],
      },
      null,
      2,
    )}\n`,
    "utf8",
  );
  return jsonPath;
}

function normalizeFixture(inputDir) {
  const metaPath = path.join(inputDir, "wave7-meta.json");
  const provenancePath = path.join(inputDir, "run-provenance.json");
  const samplesPath = path.join(inputDir, "samples.csv");
  const summaryPath = path.join(inputDir, "summary.csv");
  const binariesPath = path.join(inputDir, "binaries.csv");
  const cleanProcessLog = "no-matching-heavy-bun-build-processes\n";

  const meta = JSON.parse(fs.readFileSync(metaPath, "utf8"));
  meta.schema = "dx.tauri.wave7.command_benchmark.v2";
  meta.warmups_per_case = 5;
  meta.samples_per_case = EXPECTED_SAMPLES_PER_CASE;
  meta.samples_csv = samplesPath;
  meta.summary_csv = summaryPath;
  meta.binaries_csv = binariesPath;
  meta.machine_cache_write_env_for_all_samples = "0";
  fs.writeFileSync(metaPath, `${JSON.stringify(meta, null, 2)}\n`, "utf8");

  const provenance = JSON.parse(fs.readFileSync(provenancePath, "utf8"));
  provenance.run_id = path.basename(inputDir);
  provenance.timeout_ms = provenance.timeout_ms || 60000;
  provenance.pre_generated_machine_cache_required = true;
  provenance.machine_cache_generation_measured = false;
  provenance.machine_cache_command_writes_disabled = true;
  provenance.machine_cache_write_env_for_cache_on = "0";
  provenance.machine_cache_write_env_for_all_samples = "0";
  provenance.cache_artifact_snapshot_timing = "before_warmups_and_after_samples";
  provenance.process_sweeps = {
    roots: [REPO_ROOT, TEST_OUTPUT_ROOT],
    preflight: { path: "wave7-process-check-preflight.log", json_path: "wave7-process-check-preflight.json", clean: true },
    final: { path: "wave7-process-check-final.log", json_path: "wave7-process-check-final.json", clean: true },
  };
  provenance.git = {
    repo_root: REPO_ROOT,
    branch: "dev",
    head_sha: "a".repeat(40),
    dirty: false,
    status_short: [],
  };
  fs.writeFileSync(provenancePath, `${JSON.stringify(provenance, null, 2)}\n`, "utf8");
  fs.writeFileSync(path.join(inputDir, "wave7-process-check-preflight.log"), cleanProcessLog, "utf8");
  fs.writeFileSync(path.join(inputDir, "wave7-process-check-final.log"), cleanProcessLog, "utf8");
  writeProcessSweepJson(inputDir, "preflight");
  writeProcessSweepJson(inputDir, "final");

  const binaries = readCsvObjects(binariesPath);
  for (const field of ["role", "source_kind", "invocation_kind"]) {
    if (!binaries.headers.includes(field)) binaries.headers.push(field);
  }
  for (const row of binaries.rows) {
    const policy = casePolicy(row);
    row.role = policy.role;
    row.source_kind = policy.source_kind;
    row.invocation_kind = policy.invocation_kind;
  }
  writeCsvObjects(binariesPath, binaries.headers, binaries.rows);

  const samples = readCsvObjects(samplesPath);
  if (!samples.headers.includes("tauri_dx_machine_cache_write_env")) {
    const envIndex = samples.headers.indexOf("tauri_dx_machine_cache_env");
    samples.headers.splice(envIndex + 1, 0, "tauri_dx_machine_cache_write_env");
  }
  if (!samples.headers.includes("argv_json")) {
    const argsIndex = samples.headers.indexOf("args");
    samples.headers.splice(argsIndex + 1, 0, "argv_json");
  }
  const binaryByCase = new Map(
    binaries.rows.map((row) => [`${row.target}\u0000${row.cache_env}\u0000${row.kind}`, row]),
  );
  const firstByCase = new Map();
  for (const row of samples.rows) {
    row.tauri_dx_machine_cache_write_env = cacheWriteEnvValue(row.cache_env);
    const binaryRow = binaryByCase.get(caseKey(row));
    row.argv_json = binaryRow ? sampleArgv(row, binaryRow) : "[]";
    if (!firstByCase.has(caseKey(row))) firstByCase.set(caseKey(row), row);
  }
  const normalizedSamples = [];
  for (const row of firstByCase.values()) {
    for (let index = 1; index <= EXPECTED_SAMPLES_PER_CASE; index += 1) {
      normalizedSamples.push({
        ...row,
        phase: "sample",
        iteration: String(index),
      });
    }
  }
  writeCsvObjects(samplesPath, samples.headers, normalizedSamples);

  const summary = readCsvObjects(summaryPath);
  const normalizedSummary = [...firstByCase.values()].map((row) => ({
    target: row.target,
    cache_env: row.cache_env,
    target_kind: row.target_kind,
    runs: String(EXPECTED_SAMPLES_PER_CASE),
    median_ms: row.elapsed_ms,
    p95_ms: row.elapsed_ms,
    min_ms: row.elapsed_ms,
    max_ms: row.elapsed_ms,
    mean_ms: row.elapsed_ms,
  }));
  writeCsvObjects(summaryPath, summary.headers, normalizedSummary);
}

function runChecker(inputDir, outDir) {
  const result = spawnSync(process.execPath, [CHECKER, "--input", inputDir, "--out", outDir], {
    cwd: REPO_ROOT,
    encoding: "utf8",
  });
  fs.writeFileSync(
    path.join(RECEIPT_ROOT, `${path.basename(outDir)}.log`),
    [
      result.stdout,
      result.stderr,
      `exit_code=${result.status}`,
      `signal=${result.signal ?? ""}`,
      "",
    ].join("\n"),
    "utf8",
  );
  return result;
}

function corruptFirstSampleInvocation(samplesPath) {
  const rows = parseCsvRows(fs.readFileSync(samplesPath, "utf8").replace(/^\uFEFF/, ""));
  if (rows.length < 2) throw new Error("samples.csv does not contain a sample row to corrupt");
  const headers = rows[0];
  const firstSample = rows[1];
  for (const column of ["command", "args", "cwd"]) {
    if (!headers.includes(column)) throw new Error(`samples.csv missing ${column} column`);
  }

  firstSample[headers.indexOf("command")] = path.join(RECEIPT_ROOT, "not-the-recorded-binary.exe");
  firstSample[headers.indexOf("args")] = "inspect app-version";
  firstSample[headers.indexOf("cwd")] = path.join(RECEIPT_ROOT, "wrong-cwd");
  writeCsvRows(samplesPath, rows);
}

function corruptFirstCacheWriteEnv(samplesPath) {
  const rows = parseCsvRows(fs.readFileSync(samplesPath, "utf8").replace(/^\uFEFF/, ""));
  if (rows.length < 2) throw new Error("samples.csv does not contain a sample row to corrupt");
  const headers = rows[0];
  const writeEnvIndex = headers.indexOf("tauri_dx_machine_cache_write_env");
  if (writeEnvIndex === -1) {
    throw new Error("samples.csv missing tauri_dx_machine_cache_write_env column");
  }

  rows[1][writeEnvIndex] = "<unset>";
  writeCsvRows(samplesPath, rows);
}

function corruptFirstSampleTimeout(samplesPath) {
  const rows = parseCsvRows(fs.readFileSync(samplesPath, "utf8").replace(/^\uFEFF/, ""));
  if (rows.length < 2) throw new Error("samples.csv does not contain a sample row to corrupt");
  const headers = rows[0];
  const timedOutIndex = headers.indexOf("timed_out");
  if (timedOutIndex === -1) {
    throw new Error("samples.csv missing timed_out column");
  }

  rows[1][timedOutIndex] = "true";
  writeCsvRows(samplesPath, rows);
}

function corruptFirstOutputFile(inputDir) {
  const samplesPath = path.join(inputDir, "samples.csv");
  const rows = parseCsvRows(fs.readFileSync(samplesPath, "utf8").replace(/^\uFEFF/, ""));
  if (rows.length < 2) throw new Error("samples.csv does not contain a sample row to corrupt");
  const headers = rows[0];
  const stderrPathIndex = headers.indexOf("stderr_path");
  if (stderrPathIndex === -1) {
    throw new Error("samples.csv missing stderr_path column");
  }

  const outputPath = path.resolve(inputDir, rows[1][stderrPathIndex]);
  fs.writeFileSync(outputPath, "mutated stderr output that no longer matches samples.csv\n", "utf8");
}

function removeFirstSummaryRow(summaryPath) {
  const rows = parseCsvRows(fs.readFileSync(summaryPath, "utf8").replace(/^\uFEFF/, ""));
  if (rows.length < 2) throw new Error("summary.csv does not contain a row to remove");
  rows.splice(1, 1);
  writeCsvRows(summaryPath, rows);
}

function corruptFirstArgvJson(samplesPath) {
  const rows = parseCsvRows(fs.readFileSync(samplesPath, "utf8").replace(/^\uFEFF/, ""));
  if (rows.length < 2) throw new Error("samples.csv does not contain a sample row to corrupt");
  const headers = rows[0];
  const argvIndex = headers.indexOf("argv_json");
  if (argvIndex === -1) throw new Error("samples.csv missing argv_json column");

  rows[1][argvIndex] = JSON.stringify(["inspect wix-upgrade-code"]);
  writeCsvRows(samplesPath, rows);
}

function corruptFirstBenchmarkCase(inputDir) {
  for (const fileName of ["binaries.csv", "samples.csv", "summary.csv"]) {
    const file = path.join(inputDir, fileName);
    const rows = parseCsvRows(fs.readFileSync(file, "utf8").replace(/^\uFEFF/, ""));
    const headers = rows[0];
    const targetIndex = headers.indexOf("target");
    const roleIndex = headers.indexOf("role");
    const sourceKindIndex = headers.indexOf("source_kind");
    const invocationKindIndex = headers.indexOf("invocation_kind");
    if (targetIndex === -1) throw new Error(`${fileName} missing target column`);
    const originalTarget = rows[1][targetIndex];
    for (const row of rows.slice(1)) {
      if (row[targetIndex] !== originalTarget) continue;
      row[targetIndex] = "unreviewed-binary";
      if (roleIndex !== -1) row[roleIndex] = "unreviewed";
      if (sourceKindIndex !== -1) row[sourceKindIndex] = "unreviewed";
      if (invocationKindIndex !== -1) row[invocationKindIndex] = "unreviewed";
    }
    writeCsvRows(file, rows);
  }
}

function corruptPreflightProcessSweepJson(inputDir) {
  const file = path.join(inputDir, "wave7-process-check-preflight.json");
  const receipt = JSON.parse(fs.readFileSync(file, "utf8"));
  receipt.clean = true;
  receipt.matched_processes = [
    {
      pid: 12345,
      name: "cargo.exe",
      executable_path: path.join(REPO_ROOT, "target", "debug", "cargo.exe"),
      command_line: `cargo test --manifest-path ${path.join(REPO_ROOT, "Cargo.toml")}`,
    },
  ];
  fs.writeFileSync(file, `${JSON.stringify(receipt, null, 2)}\n`, "utf8");
}

function corruptRunProvenanceGitDirty(inputDir) {
  const file = path.join(inputDir, "run-provenance.json");
  const provenance = JSON.parse(fs.readFileSync(file, "utf8"));
  provenance.git.dirty = true;
  provenance.git.status_short = [" M crates/tauri-cli/src/lib.rs"];
  fs.writeFileSync(file, `${JSON.stringify(provenance, null, 2)}\n`, "utf8");
}

function renameTargetInCsv(file, fromTarget, toTarget) {
  const rows = parseCsvRows(fs.readFileSync(file, "utf8").replace(/^\uFEFF/, ""));
  const headers = rows[0];
  const targetIndex = headers.indexOf("target");
  if (targetIndex === -1) throw new Error(`${file} has no target column`);
  for (const row of rows.slice(1)) {
    if (row[targetIndex] === fromTarget) row[targetIndex] = toTarget;
  }
  writeCsvRows(file, rows);
}

function removeTargetFromCsv(file, target) {
  const rows = parseCsvRows(fs.readFileSync(file, "utf8").replace(/^\uFEFF/, ""));
  const headers = rows[0];
  const targetIndex = headers.indexOf("target");
  if (targetIndex === -1) throw new Error(`${file} has no target column`);
  writeCsvRows(file, [headers, ...rows.slice(1).filter((row) => row[targetIndex] !== target)]);
}

function rewriteLegacyLocalTarget(inputDir) {
  for (const fileName of ["samples.csv", "summary.csv", "binaries.csv"]) {
    const file = path.join(inputDir, fileName);
    renameTargetInCsv(file, "post6v-dx-feature", "local-current-source");
    removeTargetFromCsv(file, "post6v-current-no-dx");
  }
}

function writeLocalBuildReceipt(inputDir) {
  const binaries = readCsvObjects(path.join(inputDir, "binaries.csv")).rows;
  const localRows = binaries.filter((row) => row.target === "local-current-source");
  if (localRows.length === 0) throw new Error("fixture has no local-current-source binary rows");
  const localRow = localRows[0];
  const provenance = JSON.parse(fs.readFileSync(path.join(inputDir, "run-provenance.json"), "utf8"));
  const buildLogPath = path.join(inputDir, "local-current-source-build.log");
  fs.writeFileSync(
    buildLogPath,
    [
      "   Compiling tauri-cli v2.11.2 (G:\\Dx\\tauri\\crates\\tauri-cli)",
      "    Finished `release` profile [optimized] target(s) in 7m 05s",
      "",
    ].join("\n"),
    "utf8",
  );
  fs.writeFileSync(
    path.join(inputDir, "local-current-source-build-receipt.json"),
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
          branch: provenance.git?.branch,
          head_sha: provenance.git?.head_sha,
          dirty: provenance.git?.dirty,
          status_short: provenance.git?.status_short ?? [],
        },
        binary: {
          path: localRow.path,
          bytes: Number.parseInt(localRow.bytes, 10),
          sha256: localRow.sha256,
          last_write_utc: localRow.last_write_utc,
        },
        build_log: fileRecord(buildLogPath),
      },
      null,
      2,
    )}\n`,
    "utf8",
  );
}

function mutateLocalBuildReceipt(inputDir, mutate) {
  const receiptPath = path.join(inputDir, "local-current-source-build-receipt.json");
  const receipt = JSON.parse(fs.readFileSync(receiptPath, "utf8"));
  mutate(receipt);
  fs.writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, "utf8");
  return receiptPath;
}

function assertBroadClaimGatesBlocked(check, receiptPath) {
  const broadGateNames = [
    "faster_than_upstream_claim_allowed",
    "default_on_readiness_claim_allowed",
    "full_cli_speed_claim_allowed",
    "app_runtime_webview_ipc_build_bundle_claim_allowed",
  ];
  for (const gateName of broadGateNames) {
    if (check.claim_gate_results[gateName] !== false) {
      throw new Error(`checker allowed broad claim gate ${gateName}\nreceipt=${receiptPath}`);
    }
  }

  const blockedClaimIds = new Set((check.blocked_claims ?? []).map((claim) => claim.claim_id));
  for (const claimId of ["upstream_superiority", "default_on", "app_runtime", "full_cli"]) {
    if (!blockedClaimIds.has(claimId)) {
      throw new Error(`checker did not block ${claimId}\nreceipt=${receiptPath}`);
    }
  }

  const forbiddenAllowedClaimPattern =
    /\b(upstream superiority|default-on|default on|full cli|full product|product-level|app runtime|webview|ipc|tauri build|build speed|bundle|installer|watcher)\b/i;
  for (const claim of check.allowed_claims ?? []) {
    const claimText = [claim.claim_id, claim.scope, claim.subject, claim.text].filter(Boolean).join(" ");
    if (forbiddenAllowedClaimPattern.test(claimText)) {
      throw new Error(`checker allowed broad claim wording: ${claimText}\nreceipt=${receiptPath}`);
    }
  }
}

function main() {
  if (!fs.existsSync(FIXTURE_SOURCE)) {
    throw new Error(`source benchmark fixture is missing: ${FIXTURE_SOURCE}`);
  }

  fs.mkdirSync(RECEIPT_ROOT, { recursive: true });
  resetDir(GREEN_INPUT_DIR);
  resetDir(GREEN_OUT_DIR);
  resetDir(CURRENT_SOURCE_INPUT_DIR);
  resetDir(CURRENT_SOURCE_OUT_DIR);
  resetDir(CURRENT_SOURCE_BUILD_RECEIPT_OUT_DIR);
  resetDir(DIRTY_BUILD_RECEIPT_INPUT_DIR);
  resetDir(DIRTY_BUILD_RECEIPT_OUT_DIR);
  resetDir(MISSING_BUILD_LOG_INPUT_DIR);
  resetDir(MISSING_BUILD_LOG_OUT_DIR);
  resetDir(STALE_BUILD_LOG_INPUT_DIR);
  resetDir(STALE_BUILD_LOG_OUT_DIR);
  resetDir(WEAK_BUILD_LOG_INPUT_DIR);
  resetDir(WEAK_BUILD_LOG_OUT_DIR);
  resetDir(FAILED_BUILD_LOG_INPUT_DIR);
  resetDir(FAILED_BUILD_LOG_OUT_DIR);
  resetDir(DIRTY_RUN_PROVENANCE_INPUT_DIR);
  resetDir(DIRTY_RUN_PROVENANCE_OUT_DIR);
  resetDir(WRITE_ENV_RED_INPUT_DIR);
  resetDir(WRITE_ENV_RED_OUT_DIR);
  resetDir(SAMPLE_TIMEOUT_RED_INPUT_DIR);
  resetDir(SAMPLE_TIMEOUT_RED_OUT_DIR);
  resetDir(OUTPUT_REHASH_RED_INPUT_DIR);
  resetDir(OUTPUT_REHASH_RED_OUT_DIR);
  resetDir(MISSING_SUMMARY_RED_INPUT_DIR);
  resetDir(MISSING_SUMMARY_RED_OUT_DIR);
  resetDir(ARGV_RED_INPUT_DIR);
  resetDir(ARGV_RED_OUT_DIR);
  resetDir(CASE_ALLOWLIST_RED_INPUT_DIR);
  resetDir(CASE_ALLOWLIST_RED_OUT_DIR);
  resetDir(PROCESS_SWEEP_RED_INPUT_DIR);
  resetDir(PROCESS_SWEEP_RED_OUT_DIR);
  resetDir(PROCESS_SWEEP_MISSING_INPUT_DIR);
  resetDir(PROCESS_SWEEP_MISSING_OUT_DIR);
  resetDir(RED_INPUT_DIR);
  resetDir(RED_OUT_DIR);
  fs.cpSync(FIXTURE_SOURCE, GREEN_INPUT_DIR, { recursive: true });
  normalizeFixture(GREEN_INPUT_DIR);

  const greenResult = runChecker(GREEN_INPUT_DIR, GREEN_OUT_DIR);
  const greenCheckPath = path.join(GREEN_OUT_DIR, "wave7-benchmark-manifest-check.json");
  if (greenResult.status !== 0) {
    throw new Error(`checker rejected the normalized valid fixture\nreceipt=${greenCheckPath}`);
  }
  const greenCheck = JSON.parse(fs.readFileSync(greenCheckPath, "utf8"));
  if (greenCheck.status !== "pass" || greenCheck.sample_invocations_verified !== true) {
    throw new Error(`checker valid fixture did not prove sample invocations\nreceipt=${greenCheckPath}`);
  }

  fs.cpSync(GREEN_INPUT_DIR, CURRENT_SOURCE_INPUT_DIR, { recursive: true });
  normalizeFixture(CURRENT_SOURCE_INPUT_DIR);
  rewriteLegacyLocalTarget(CURRENT_SOURCE_INPUT_DIR);
  const currentSourceResult = runChecker(CURRENT_SOURCE_INPUT_DIR, CURRENT_SOURCE_OUT_DIR);
  const currentSourceCheckPath = path.join(CURRENT_SOURCE_OUT_DIR, "wave7-benchmark-manifest-check.json");
  if (currentSourceResult.status !== 0) {
    throw new Error(`checker rejected the professional local-current-source target\nreceipt=${currentSourceCheckPath}`);
  }
  const currentSourceCheck = JSON.parse(fs.readFileSync(currentSourceCheckPath, "utf8"));
  if (currentSourceCheck.status !== "pass" || currentSourceCheck.sample_invocations_verified !== true) {
    throw new Error(`checker current-source fixture did not prove sample invocations\nreceipt=${currentSourceCheckPath}`);
  }
  if (currentSourceCheck.claim_gate_results.current_source_release_speed_claim_allowed !== false) {
    throw new Error("checker allowed a current-source speed claim without a build receipt");
  }

  writeLocalBuildReceipt(CURRENT_SOURCE_INPUT_DIR);
  const buildReceiptResult = runChecker(CURRENT_SOURCE_INPUT_DIR, CURRENT_SOURCE_BUILD_RECEIPT_OUT_DIR);
  const buildReceiptCheckPath = path.join(CURRENT_SOURCE_BUILD_RECEIPT_OUT_DIR, "wave7-benchmark-manifest-check.json");
  if (buildReceiptResult.status !== 0) {
    throw new Error(`checker rejected the current-source build receipt fixture\nreceipt=${buildReceiptCheckPath}`);
  }
  const buildReceiptCheck = JSON.parse(fs.readFileSync(buildReceiptCheckPath, "utf8"));
  if (buildReceiptCheck.claim_gate_results.current_source_release_speed_claim_allowed !== true) {
    throw new Error(`checker did not allow the narrow current-source speed claim\nreceipt=${buildReceiptCheckPath}`);
  }
  if (buildReceiptCheck.local_current_source_build_receipt_verified !== true) {
    throw new Error(`checker did not expose verified local build receipt status\nreceipt=${buildReceiptCheckPath}`);
  }
  assertBroadClaimGatesBlocked(buildReceiptCheck, buildReceiptCheckPath);

  fs.cpSync(CURRENT_SOURCE_INPUT_DIR, DIRTY_BUILD_RECEIPT_INPUT_DIR, { recursive: true });
  normalizeFixture(DIRTY_BUILD_RECEIPT_INPUT_DIR);
  const dirtyReceiptPath = path.join(DIRTY_BUILD_RECEIPT_INPUT_DIR, "local-current-source-build-receipt.json");
  const dirtyReceipt = JSON.parse(fs.readFileSync(dirtyReceiptPath, "utf8"));
  dirtyReceipt.git.dirty = true;
  dirtyReceipt.git.status_short = [" M crates/tauri-cli/src/lib.rs"];
  fs.writeFileSync(dirtyReceiptPath, `${JSON.stringify(dirtyReceipt, null, 2)}\n`, "utf8");
  const dirtyReceiptResult = runChecker(DIRTY_BUILD_RECEIPT_INPUT_DIR, DIRTY_BUILD_RECEIPT_OUT_DIR);
  if (dirtyReceiptResult.status === 0) {
    throw new Error("checker unexpectedly accepted a dirty local current-source build receipt");
  }

  fs.cpSync(CURRENT_SOURCE_INPUT_DIR, MISSING_BUILD_LOG_INPUT_DIR, { recursive: true });
  normalizeFixture(MISSING_BUILD_LOG_INPUT_DIR);
  mutateLocalBuildReceipt(MISSING_BUILD_LOG_INPUT_DIR, (receipt) => {
    delete receipt.build_log;
  });
  const missingBuildLogResult = runChecker(MISSING_BUILD_LOG_INPUT_DIR, MISSING_BUILD_LOG_OUT_DIR);
  if (missingBuildLogResult.status === 0) {
    throw new Error("checker unexpectedly accepted a current-source build receipt without build_log evidence");
  }
  const missingBuildLogCheckPath = path.join(MISSING_BUILD_LOG_OUT_DIR, "wave7-benchmark-manifest-check.json");
  const missingBuildLogCheck = JSON.parse(fs.readFileSync(missingBuildLogCheckPath, "utf8"));
  if (!missingBuildLogCheck.failures.some((failure) => String(failure).includes("missing build_log evidence"))) {
    throw new Error(`checker did not reject missing build_log evidence\nreceipt=${missingBuildLogCheckPath}`);
  }

  fs.cpSync(CURRENT_SOURCE_INPUT_DIR, STALE_BUILD_LOG_INPUT_DIR, { recursive: true });
  normalizeFixture(STALE_BUILD_LOG_INPUT_DIR);
  mutateLocalBuildReceipt(STALE_BUILD_LOG_INPUT_DIR, (receipt) => {
    receipt.build_log.sha256 = "0".repeat(64);
  });
  const staleBuildLogResult = runChecker(STALE_BUILD_LOG_INPUT_DIR, STALE_BUILD_LOG_OUT_DIR);
  if (staleBuildLogResult.status === 0) {
    throw new Error("checker unexpectedly accepted a stale current-source build log identity");
  }
  const staleBuildLogCheckPath = path.join(STALE_BUILD_LOG_OUT_DIR, "wave7-benchmark-manifest-check.json");
  const staleBuildLogCheck = JSON.parse(fs.readFileSync(staleBuildLogCheckPath, "utf8"));
  if (!staleBuildLogCheck.failures.some((failure) => String(failure).includes("build_log identity does not match the recorded file"))) {
    throw new Error(`checker did not reject stale build_log identity\nreceipt=${staleBuildLogCheckPath}`);
  }

  fs.cpSync(CURRENT_SOURCE_INPUT_DIR, WEAK_BUILD_LOG_INPUT_DIR, { recursive: true });
  normalizeFixture(WEAK_BUILD_LOG_INPUT_DIR);
  mutateLocalBuildReceipt(WEAK_BUILD_LOG_INPUT_DIR, (receipt) => {
    fs.writeFileSync(receipt.build_log.path, "Finished release\n", "utf8");
    receipt.build_log = fileRecord(receipt.build_log.path);
  });
  const weakBuildLogResult = runChecker(WEAK_BUILD_LOG_INPUT_DIR, WEAK_BUILD_LOG_OUT_DIR);
  if (weakBuildLogResult.status === 0) {
    throw new Error("checker unexpectedly accepted a weak current-source build log");
  }
  const weakBuildLogCheckPath = path.join(WEAK_BUILD_LOG_OUT_DIR, "wave7-benchmark-manifest-check.json");
  const weakBuildLogCheck = JSON.parse(fs.readFileSync(weakBuildLogCheckPath, "utf8"));
  if (!weakBuildLogCheck.failures.some((failure) => String(failure).includes("build_log does not prove a completed tauri-cli release build"))) {
    throw new Error(`checker did not reject weak build_log content\nreceipt=${weakBuildLogCheckPath}`);
  }

  fs.cpSync(CURRENT_SOURCE_INPUT_DIR, FAILED_BUILD_LOG_INPUT_DIR, { recursive: true });
  normalizeFixture(FAILED_BUILD_LOG_INPUT_DIR);
  mutateLocalBuildReceipt(FAILED_BUILD_LOG_INPUT_DIR, (receipt) => {
    fs.writeFileSync(
      receipt.build_log.path,
      [
        "   Compiling tauri-cli v2.11.2 (G:\\Dx\\tauri\\crates\\tauri-cli)",
        "error: could not compile tauri-cli",
        "    Finished `release` profile [optimized] target(s) in 7m 05s",
        "",
      ].join("\n"),
      "utf8",
    );
    receipt.build_log = fileRecord(receipt.build_log.path);
  });
  const failedBuildLogResult = runChecker(FAILED_BUILD_LOG_INPUT_DIR, FAILED_BUILD_LOG_OUT_DIR);
  if (failedBuildLogResult.status === 0) {
    throw new Error("checker unexpectedly accepted a current-source build log with failure output");
  }
  const failedBuildLogCheckPath = path.join(FAILED_BUILD_LOG_OUT_DIR, "wave7-benchmark-manifest-check.json");
  const failedBuildLogCheck = JSON.parse(fs.readFileSync(failedBuildLogCheckPath, "utf8"));
  if (!failedBuildLogCheck.failures.some((failure) => String(failure).includes("build_log contains build failure output"))) {
    throw new Error(`checker did not reject failed build_log content\nreceipt=${failedBuildLogCheckPath}`);
  }

  fs.cpSync(GREEN_INPUT_DIR, DIRTY_RUN_PROVENANCE_INPUT_DIR, { recursive: true });
  normalizeFixture(DIRTY_RUN_PROVENANCE_INPUT_DIR);
  corruptRunProvenanceGitDirty(DIRTY_RUN_PROVENANCE_INPUT_DIR);
  const dirtyRunProvenanceResult = runChecker(DIRTY_RUN_PROVENANCE_INPUT_DIR, DIRTY_RUN_PROVENANCE_OUT_DIR);
  if (dirtyRunProvenanceResult.status === 0) {
    throw new Error("checker unexpectedly accepted a dirty command-benchmark v2 run provenance");
  }
  const dirtyRunProvenanceCheckPath = path.join(DIRTY_RUN_PROVENANCE_OUT_DIR, "wave7-benchmark-manifest-check.json");
  const dirtyRunProvenanceCheck = JSON.parse(fs.readFileSync(dirtyRunProvenanceCheckPath, "utf8"));
  if (!dirtyRunProvenanceCheck.failures.some((failure) => String(failure).includes("run-provenance.json git.dirty must be false"))) {
    throw new Error(`checker did not reject dirty v2 run provenance\nreceipt=${dirtyRunProvenanceCheckPath}`);
  }
  if (dirtyRunProvenanceCheck.run_provenance_verified !== false) {
    throw new Error("checker over-reported run provenance verification for a dirty v2 run");
  }

  fs.cpSync(GREEN_INPUT_DIR, WRITE_ENV_RED_INPUT_DIR, { recursive: true });
  normalizeFixture(WRITE_ENV_RED_INPUT_DIR);
  corruptFirstCacheWriteEnv(path.join(WRITE_ENV_RED_INPUT_DIR, "samples.csv"));
  const writeEnvResult = runChecker(WRITE_ENV_RED_INPUT_DIR, WRITE_ENV_RED_OUT_DIR);
  if (writeEnvResult.status === 0) {
    throw new Error("checker unexpectedly accepted a measured sample without write-disable env");
  }
  const writeEnvCheckPath = path.join(WRITE_ENV_RED_OUT_DIR, "wave7-benchmark-manifest-check.json");
  if (!fs.existsSync(writeEnvCheckPath)) {
    throw new Error(`checker did not write its write-env JSON receipt: ${writeEnvCheckPath}`);
  }
  const writeEnvCheck = JSON.parse(fs.readFileSync(writeEnvCheckPath, "utf8"));
  const writeEnvFailures = Array.isArray(writeEnvCheck.failures) ? writeEnvCheck.failures : [];
  if (!writeEnvFailures.some((failure) => String(failure).includes("TAURI_DX_MACHINE_CACHE_WRITE mismatch"))) {
    throw new Error(`checker did not reject the write-enabled benchmark sample\nreceipt=${writeEnvCheckPath}`);
  }
  if (writeEnvCheck.sample_invocations_verified !== false) {
    throw new Error("checker over-reported sample invocation verification for a write-env failure");
  }
  if (writeEnvCheck.benchmark_integrity_verified !== false) {
    throw new Error("checker over-reported benchmark integrity for a write-env failure");
  }

  fs.cpSync(GREEN_INPUT_DIR, SAMPLE_TIMEOUT_RED_INPUT_DIR, { recursive: true });
  normalizeFixture(SAMPLE_TIMEOUT_RED_INPUT_DIR);
  corruptFirstSampleTimeout(path.join(SAMPLE_TIMEOUT_RED_INPUT_DIR, "samples.csv"));
  const timeoutResult = runChecker(SAMPLE_TIMEOUT_RED_INPUT_DIR, SAMPLE_TIMEOUT_RED_OUT_DIR);
  if (timeoutResult.status === 0) {
    throw new Error("checker unexpectedly accepted a timed-out benchmark sample");
  }
  const timeoutCheckPath = path.join(SAMPLE_TIMEOUT_RED_OUT_DIR, "wave7-benchmark-manifest-check.json");
  const timeoutCheck = JSON.parse(fs.readFileSync(timeoutCheckPath, "utf8"));
  if (!timeoutCheck.failures.some((failure) => String(failure).includes("sample timed out"))) {
    throw new Error(`checker did not reject the timed-out benchmark sample\nreceipt=${timeoutCheckPath}`);
  }
  if (timeoutCheck.sample_invocations_verified !== false) {
    throw new Error("checker over-reported sample invocation verification for a timed-out sample");
  }
  if (timeoutCheck.claim_gate_results.official_release_snapshot_allowed !== false) {
    throw new Error("checker over-reported official release snapshot allowance for a failed receipt");
  }

  fs.cpSync(GREEN_INPUT_DIR, OUTPUT_REHASH_RED_INPUT_DIR, { recursive: true });
  normalizeFixture(OUTPUT_REHASH_RED_INPUT_DIR);
  corruptFirstOutputFile(OUTPUT_REHASH_RED_INPUT_DIR);
  const outputRehashResult = runChecker(OUTPUT_REHASH_RED_INPUT_DIR, OUTPUT_REHASH_RED_OUT_DIR);
  if (outputRehashResult.status === 0) {
    throw new Error("checker unexpectedly accepted a benchmark fixture with stale output files");
  }
  const outputRehashCheckPath = path.join(OUTPUT_REHASH_RED_OUT_DIR, "wave7-benchmark-manifest-check.json");
  const outputRehashCheck = JSON.parse(fs.readFileSync(outputRehashCheckPath, "utf8"));
  if (!outputRehashCheck.failures.some((failure) => String(failure).includes("output stderr_sha256 mismatch"))) {
    throw new Error(`checker did not reject stale output file hashes\nreceipt=${outputRehashCheckPath}`);
  }
  if (outputRehashCheck.output_equivalence_verified !== false) {
    throw new Error("checker over-reported output equivalence after output file rehash failure");
  }
  if (outputRehashCheck.full_stdout_stderr_hashes_verified_from_files !== false) {
    throw new Error("checker over-reported output file rehash verification after output file mutation");
  }

  fs.cpSync(GREEN_INPUT_DIR, MISSING_SUMMARY_RED_INPUT_DIR, { recursive: true });
  normalizeFixture(MISSING_SUMMARY_RED_INPUT_DIR);
  removeFirstSummaryRow(path.join(MISSING_SUMMARY_RED_INPUT_DIR, "summary.csv"));
  const missingSummaryResult = runChecker(MISSING_SUMMARY_RED_INPUT_DIR, MISSING_SUMMARY_RED_OUT_DIR);
  if (missingSummaryResult.status === 0) {
    throw new Error("checker unexpectedly accepted a benchmark fixture with a missing summary row");
  }
  const missingSummaryCheckPath = path.join(MISSING_SUMMARY_RED_OUT_DIR, "wave7-benchmark-manifest-check.json");
  const missingSummaryCheck = JSON.parse(fs.readFileSync(missingSummaryCheckPath, "utf8"));
  if (!missingSummaryCheck.failures.some((failure) => String(failure).includes("summary.csv missing"))) {
    throw new Error(`checker did not reject the missing summary row\nreceipt=${missingSummaryCheckPath}`);
  }
  if (missingSummaryCheck.summary_recomputed_from_samples !== false) {
    throw new Error("checker over-reported summary recomputation verification after missing summary row");
  }

  fs.cpSync(GREEN_INPUT_DIR, ARGV_RED_INPUT_DIR, { recursive: true });
  normalizeFixture(ARGV_RED_INPUT_DIR);
  corruptFirstArgvJson(path.join(ARGV_RED_INPUT_DIR, "samples.csv"));
  const argvResult = runChecker(ARGV_RED_INPUT_DIR, ARGV_RED_OUT_DIR);
  if (argvResult.status === 0) {
    throw new Error("checker unexpectedly accepted a benchmark fixture with corrupted argv_json");
  }
  const argvCheckPath = path.join(ARGV_RED_OUT_DIR, "wave7-benchmark-manifest-check.json");
  const argvCheck = JSON.parse(fs.readFileSync(argvCheckPath, "utf8"));
  if (!argvCheck.failures.some((failure) => String(failure).includes("sample invocation argv_json mismatch"))) {
    throw new Error(`checker did not reject corrupted argv_json\nreceipt=${argvCheckPath}`);
  }

  fs.cpSync(GREEN_INPUT_DIR, CASE_ALLOWLIST_RED_INPUT_DIR, { recursive: true });
  normalizeFixture(CASE_ALLOWLIST_RED_INPUT_DIR);
  corruptFirstBenchmarkCase(CASE_ALLOWLIST_RED_INPUT_DIR);
  const caseAllowlistResult = runChecker(CASE_ALLOWLIST_RED_INPUT_DIR, CASE_ALLOWLIST_RED_OUT_DIR);
  if (caseAllowlistResult.status === 0) {
    throw new Error("checker unexpectedly accepted a benchmark fixture with an unallowlisted case");
  }
  const caseAllowlistCheckPath = path.join(CASE_ALLOWLIST_RED_OUT_DIR, "wave7-benchmark-manifest-check.json");
  const caseAllowlistCheck = JSON.parse(fs.readFileSync(caseAllowlistCheckPath, "utf8"));
  if (!caseAllowlistCheck.failures.some((failure) => String(failure).includes("binaries.csv case is not allowlisted"))) {
    throw new Error(`checker did not reject the unallowlisted benchmark case\nreceipt=${caseAllowlistCheckPath}`);
  }

  fs.cpSync(GREEN_INPUT_DIR, PROCESS_SWEEP_RED_INPUT_DIR, { recursive: true });
  normalizeFixture(PROCESS_SWEEP_RED_INPUT_DIR);
  corruptPreflightProcessSweepJson(PROCESS_SWEEP_RED_INPUT_DIR);
  const processSweepResult = runChecker(PROCESS_SWEEP_RED_INPUT_DIR, PROCESS_SWEEP_RED_OUT_DIR);
  if (processSweepResult.status === 0) {
    throw new Error("checker unexpectedly accepted a process sweep JSON with matched processes");
  }
  const processSweepCheckPath = path.join(PROCESS_SWEEP_RED_OUT_DIR, "wave7-benchmark-manifest-check.json");
  const processSweepCheck = JSON.parse(fs.readFileSync(processSweepCheckPath, "utf8"));
  if (!processSweepCheck.failures.some((failure) => String(failure).includes("preflight process sweep JSON clean=true but matched_processes is not empty"))) {
    throw new Error(`checker did not reject the contaminated process sweep JSON\nreceipt=${processSweepCheckPath}`);
  }
  if (processSweepCheck.process_sweeps_verified !== false || processSweepCheck.benchmark_integrity_verified !== false) {
    throw new Error("checker over-reported process sweep or benchmark integrity for contaminated process JSON");
  }

  fs.cpSync(GREEN_INPUT_DIR, PROCESS_SWEEP_MISSING_INPUT_DIR, { recursive: true });
  normalizeFixture(PROCESS_SWEEP_MISSING_INPUT_DIR);
  fs.rmSync(path.join(PROCESS_SWEEP_MISSING_INPUT_DIR, "wave7-process-check-final.json"), { force: true });
  const missingProcessSweepResult = runChecker(PROCESS_SWEEP_MISSING_INPUT_DIR, PROCESS_SWEEP_MISSING_OUT_DIR);
  if (missingProcessSweepResult.status === 0) {
    throw new Error("checker unexpectedly accepted a v2 fixture with missing process sweep JSON");
  }
  const missingProcessSweepCheckPath = path.join(PROCESS_SWEEP_MISSING_OUT_DIR, "wave7-benchmark-manifest-check.json");
  const missingProcessSweepCheck = JSON.parse(fs.readFileSync(missingProcessSweepCheckPath, "utf8"));
  if (!missingProcessSweepCheck.failures.some((failure) => String(failure).includes("missing required final process sweep JSON"))) {
    throw new Error(`checker did not reject the missing process sweep JSON\nreceipt=${missingProcessSweepCheckPath}`);
  }

  fs.cpSync(GREEN_INPUT_DIR, RED_INPUT_DIR, { recursive: true });
  normalizeFixture(RED_INPUT_DIR);
  corruptFirstSampleInvocation(path.join(RED_INPUT_DIR, "samples.csv"));

  const redResult = runChecker(RED_INPUT_DIR, RED_OUT_DIR);
  if (redResult.status === 0) {
    throw new Error("checker unexpectedly accepted a benchmark fixture with a corrupted sample invocation");
  }

  const checkPath = path.join(RED_OUT_DIR, "wave7-benchmark-manifest-check.json");
  if (!fs.existsSync(checkPath)) {
    throw new Error(`checker did not write its JSON receipt: ${checkPath}`);
  }
  const check = JSON.parse(fs.readFileSync(checkPath, "utf8"));
  const failures = Array.isArray(check.failures) ? check.failures : [];
  const expectedFailures = [
    "sample invocation command mismatch",
    "sample invocation args mismatch",
    "sample invocation cwd mismatch",
  ];
  const missing = expectedFailures.filter(
    (expected) => !failures.some((failure) => String(failure).includes(expected)),
  );
  if (missing.length > 0) {
    throw new Error(
      `checker did not report expected sample invocation failures: ${missing.join(", ")}\nreceipt=${checkPath}`,
    );
  }

  console.log(`valid sample invocation receipt=${greenCheckPath}`);
  console.log(`current-source target receipt=${currentSourceCheckPath}`);
  console.log(`current-source build receipt=${buildReceiptCheckPath}`);
  console.log(`dirty build receipt rejected=${path.join(DIRTY_BUILD_RECEIPT_OUT_DIR, "wave7-benchmark-manifest-check.json")}`);
  console.log(`missing build log rejected=${missingBuildLogCheckPath}`);
  console.log(`stale build log rejected=${staleBuildLogCheckPath}`);
  console.log(`weak build log rejected=${weakBuildLogCheckPath}`);
  console.log(`failed build log rejected=${failedBuildLogCheckPath}`);
  console.log(`dirty run provenance rejected=${dirtyRunProvenanceCheckPath}`);
  console.log(`bad cache write env receipt=${writeEnvCheckPath}`);
  console.log(`bad sample timeout receipt=${timeoutCheckPath}`);
  console.log(`bad output rehash receipt=${outputRehashCheckPath}`);
  console.log(`missing summary row receipt=${missingSummaryCheckPath}`);
  console.log(`bad argv_json receipt=${path.join(ARGV_RED_OUT_DIR, "wave7-benchmark-manifest-check.json")}`);
  console.log(`bad case allowlist receipt=${path.join(CASE_ALLOWLIST_RED_OUT_DIR, "wave7-benchmark-manifest-check.json")}`);
  console.log(`bad process sweep receipt=${processSweepCheckPath}`);
  console.log(`missing process sweep json receipt=${missingProcessSweepCheckPath}`);
  console.log(`bad sample invocation receipt=${checkPath}`);
}

main();
