#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
require.extensions[".ts"] ||= require.extensions[".js"];
const {
  DEFAULT_BENCHMARK_SURFACE_ID,
  getBenchmarkSurface,
  validateBenchmarkCase,
} = require("./dx-benchmark-command-surfaces.ts");

const TEST_OUTPUT_ROOT = path.resolve("G:\\Dx\\test-outputs");
const DEFAULT_INPUT =
  "G:\\Dx\\test-outputs\\tauri-dx-machine-cache-20260530-wave7-inspect1";
const DEFAULT_OUT =
  "G:\\Dx\\test-outputs\\tauri-dx-machine-cache-20260530-wave7-manifest-check1";
const COMMAND_BENCHMARK_META_SCHEMA_V2 = "dx.tauri.wave7.command_benchmark.v2";
const EXPECTED_META_SCHEMAS = [
  "dx.tauri.wave7.inspect_benchmark.v1",
  "dx.tauri.wave7.command_benchmark.v1",
  COMMAND_BENCHMARK_META_SCHEMA_V2,
];
const EXPECTED_WARMUPS = 5;
const EXPECTED_SAMPLES_PER_CASE = 30;
const PROCESS_SWEEP_SCHEMA = "dx.tauri.process_sweep.v1";
const CACHE_COMPARISON_TARGETS = ["local-current-source", "post6v-dx-feature"];
const LOCAL_BUILD_RECEIPT_FILE = "local-current-source-build-receipt.json";
const LOCAL_BUILD_RECEIPT_SCHEMA = "dx.tauri.local_release_build_receipt.v1";
const EXPECTED_LOCAL_BUILD_ARGV = [
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
];
const EXPECTED_LOCAL_BUILD_COMMAND = EXPECTED_LOCAL_BUILD_ARGV.join(" ");

function parseArgs(argv) {
  const args = {
    input: DEFAULT_INPUT,
    out: DEFAULT_OUT,
  };
  for (let i = 2; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--input") {
      args.input = argv[++i];
    } else if (arg === "--out") {
      args.out = argv[++i];
    } else if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  return args;
}

function printHelp() {
  console.log(`Usage:
  node .scripts/ci/check-dx-benchmark-manifest.ts --input <benchmark-output-dir> --out <output-dir>

Reads an existing no-build Tauri benchmark output and writes a machine-readable
manifest plus governance check under G:\\Dx\\test-outputs.`);
}

function assertUnderTestOutputs(candidate, label) {
  const resolved = path.resolve(candidate);
  const withSep = TEST_OUTPUT_ROOT.endsWith(path.sep)
    ? TEST_OUTPUT_ROOT
    : `${TEST_OUTPUT_ROOT}${path.sep}`;
  if (
    resolved !== TEST_OUTPUT_ROOT &&
    !resolved.toLowerCase().startsWith(withSep.toLowerCase())
  ) {
    throw new Error(`${label} must stay under ${TEST_OUTPUT_ROOT}: ${resolved}`);
  }
  return resolved;
}

function readText(file) {
  return fs.readFileSync(file, "utf8").replace(/^\uFEFF/, "");
}

function readJson(file) {
  return JSON.parse(readText(file));
}

function sha256(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function fileRecord(file, baseDir) {
  const record = {
    path: file,
    relative_path: path.relative(baseDir, file),
    present: fs.existsSync(file),
    bytes: null,
    sha256: null,
    last_write_utc: null,
  };
  if (record.present) {
    const stat = fs.statSync(file);
    record.bytes = stat.size;
    record.sha256 = sha256(file);
    record.last_write_utc = stat.mtime.toISOString();
  }
  return record;
}

function parseCsvRows(text) {
  const rows = [];
  let row = [];
  let field = "";
  let inQuotes = false;

  for (let i = 0; i < text.length; i += 1) {
    const char = text[i];
    const next = text[i + 1];

    if (inQuotes) {
      if (char === '"' && next === '"') {
        field += '"';
        i += 1;
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
  const [headers, ...body] = parseCsvRows(text);
  if (!headers) return [];
  return body.map((entry) => {
    const object = {};
    headers.forEach((header, index) => {
      object[header] = entry[index] ?? "";
    });
    return object;
  });
}

function csvHeaders(text) {
  const [firstRow] = parseCsvRows(text);
  return firstRow ?? [];
}

function groupBy(rows, keyFn) {
  const groups = new Map();
  for (const row of rows) {
    const key = keyFn(row);
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(row);
  }
  return groups;
}

function parseMsToThousandths(value) {
  const [whole, fraction = ""] = String(value).split(".");
  const sign = whole.startsWith("-") ? -1 : 1;
  const wholeDigits = whole.replace("-", "");
  const padded = `${fraction}000`.slice(0, 3);
  return sign * (Number.parseInt(wholeDigits || "0", 10) * 1000 + Number.parseInt(padded, 10));
}

function thousandthsToNumber(value) {
  return Number((value / 1000).toFixed(3));
}

function round3(value) {
  return thousandthsToNumber(Math.floor(value * 1000 + 0.5));
}

function median(values) {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = sorted.length / 2;
  if (sorted.length % 2 === 1) {
    return thousandthsToNumber(sorted[Math.floor(middle)]);
  }
  return thousandthsToNumber(Math.floor((sorted[middle - 1] + sorted[middle]) / 2 + 0.5));
}

function p95(values) {
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.max(0, Math.min(sorted.length - 1, Math.ceil(sorted.length * 0.95) - 1));
  return thousandthsToNumber(sorted[index]);
}

function mean(values) {
  return thousandthsToNumber(Math.floor(values.reduce((sum, value) => sum + value, 0) / values.length + 0.5));
}

function minMs(values) {
  return thousandthsToNumber(Math.min(...values));
}

function maxMs(values) {
  return thousandthsToNumber(Math.max(...values));
}

function unique(values) {
  return [...new Set(values)];
}

function near(left, right) {
  return Math.abs(left - right) <= 0.001;
}

function isHexSha256(value) {
  return /^[a-f0-9]{64}$/i.test(String(value));
}

function isGitSha(value) {
  return /^[a-f0-9]{40}$/i.test(String(value));
}

function isOfficialReleaseTarget(target) {
  return target === "official-release-binary" || target === "official-release-zip";
}

function stringArraysEqual(left, right) {
  return (
    Array.isArray(left) &&
    Array.isArray(right) &&
    left.length === right.length &&
    left.every((value, index) => value === right[index])
  );
}

function parseNonnegativeInteger(value) {
  if (!/^\d+$/.test(String(value))) return null;
  const parsed = Number.parseInt(value, 10);
  return Number.isSafeInteger(parsed) ? parsed : null;
}

function missingColumns(actual, expected) {
  return expected.filter((column) => !actual.includes(column));
}

function hasAnyColumn(actual, expected) {
  return expected.some((column) => actual.includes(column));
}

function evidencePath(inputDir, rawPath, label, failures) {
  if (!rawPath) return null;
  const resolved = path.resolve(path.isAbsolute(rawPath) ? rawPath : path.join(inputDir, rawPath));
  const inputWithSep = inputDir.endsWith(path.sep) ? inputDir : `${inputDir}${path.sep}`;
  if (resolved !== inputDir && !resolved.toLowerCase().startsWith(inputWithSep.toLowerCase())) {
    failures.push(`${label} must stay under benchmark input dir: ${resolved}`);
    return null;
  }
  return resolved;
}

function readOptionalCsv(file, failures, requiredColumns, label) {
  if (!fs.existsSync(file)) return null;
  const text = readText(file);
  const columns = csvHeaders(text);
  const missing = missingColumns(columns, requiredColumns);
  if (missing.length > 0) {
    failures.push(`${label} missing required columns: ${missing.join(", ")}`);
  }
  return {
    path: file,
    file_record: fileRecord(file, path.dirname(file)),
    columns,
    rows: parseCsv(text),
  };
}

function artifactRowKey(row) {
  return row.relative_path || row.path || "";
}

function validateArtifactRows(rows, label, failures) {
  const seen = new Set();
  if (rows.length === 0) {
    failures.push(`${label} must contain at least one .machine artifact row`);
  }
  for (const row of rows) {
    const key = artifactRowKey(row);
    if (!key) {
      failures.push(`${label} has an artifact row without relative_path`);
      continue;
    }
    if (path.isAbsolute(key) || key.split(/[\\/]+/).includes("..")) {
      failures.push(`${label} has unsafe relative_path: ${key}`);
    }
    if (!key.endsWith(".machine")) {
      failures.push(`${label} has non-machine artifact row: ${key}`);
    }
    if (seen.has(key)) failures.push(`${label} has duplicate artifact row: ${key}`);
    seen.add(key);
    const bytes = parseNonnegativeInteger(row.bytes);
    if (bytes === null || bytes <= 0) {
      failures.push(`${label} has invalid bytes for ${key}: ${row.bytes}`);
    }
    if (!isHexSha256(row.sha256)) {
      failures.push(`${label} has invalid sha256 for ${key}: ${row.sha256}`);
    }
  }
  return seen;
}

function validateCacheArtifactSnapshots(inputDir, failures, warnings) {
  const requiredColumns = ["relative_path", "bytes", "sha256", "last_write_utc"];
  const beforePath = path.join(inputDir, "cache-artifacts-before.csv");
  const afterPath = path.join(inputDir, "cache-artifacts-after.csv");
  const failureCountAtStart = failures.length;
  const beforeExists = fs.existsSync(beforePath);
  const afterExists = fs.existsSync(afterPath);
  if (beforeExists !== afterExists) {
    failures.push("cache artifact mutation proof requires both cache-artifacts-before.csv and cache-artifacts-after.csv");
  }
  if (!beforeExists && !afterExists) {
    warnings.push("The input benchmark run did not record cache artifact before/after hashes; mutation-during-run cannot be proven retroactively");
    return {
      before: null,
      after: null,
      verified: false,
      stable_count: 0,
      changed: [],
    };
  }

  const before = readOptionalCsv(beforePath, failures, requiredColumns, "cache-artifacts-before.csv");
  const after = readOptionalCsv(afterPath, failures, requiredColumns, "cache-artifacts-after.csv");
  if (!before || !after) {
    return {
      before,
      after,
      verified: false,
      stable_count: 0,
      changed: [],
    };
  }

  const beforeKeys = validateArtifactRows(before.rows, "cache-artifacts-before.csv", failures);
  const afterKeys = validateArtifactRows(after.rows, "cache-artifacts-after.csv", failures);
  const changed = [];
  for (const key of beforeKeys) {
    if (!afterKeys.has(key)) {
      changed.push(`${key}: missing-after`);
      continue;
    }
    const beforeRow = before.rows.find((row) => artifactRowKey(row) === key);
    const afterRow = after.rows.find((row) => artifactRowKey(row) === key);
    for (const column of ["bytes", "sha256", "last_write_utc"]) {
      if ((beforeRow[column] ?? "") !== (afterRow[column] ?? "")) {
        changed.push(`${key}: ${column} changed`);
      }
    }
  }
  for (const key of afterKeys) {
    if (!beforeKeys.has(key)) changed.push(`${key}: missing-before`);
  }
  for (const entry of changed) {
    failures.push(`cache artifact mutation proof mismatch: ${entry}`);
  }

  return {
    before,
    after,
    verified: changed.length === 0 && failures.length === failureCountAtStart,
    stable_count: beforeKeys.size,
    changed,
  };
}

function validateRunProvenance(inputDir, failures, warnings, benchmarkSurface) {
  const provenancePath = path.join(inputDir, "run-provenance.json");
  if (!fs.existsSync(provenancePath)) {
    warnings.push("The input benchmark run did not record a full git-status/env receipt in machine-readable form");
    return {
      present: false,
      file_record: fileRecord(provenancePath, inputDir),
      data: null,
      verified: false,
    };
  }

  let data = null;
  try {
    data = readJson(provenancePath);
  } catch (error) {
    failures.push(`run-provenance.json is not valid JSON: ${error.message}`);
  }
  if (!data || typeof data !== "object") {
    return {
      present: true,
      file_record: fileRecord(provenancePath, inputDir),
      data,
      verified: false,
    };
  }

  if (data.schema !== "dx.tauri.no_build_benchmark_run_provenance.v1") {
    failures.push(`run-provenance.json schema mismatch: ${data.schema}`);
  }
  if (data.run_id !== path.basename(inputDir)) {
    failures.push(`run-provenance.json run_id mismatch: ${data.run_id}`);
  }
  for (const [key, expected] of [
    ["no_build", true],
    ["existing_binaries_only", true],
    ["build_run", false],
    ["cargo_test_run", false],
    ["install_run", false],
    ["heavy_benchmark_run", false],
  ]) {
    if (data[key] !== expected) failures.push(`run-provenance.json ${key} expected ${expected}, got ${data[key]}`);
  }
  if (data.command !== benchmarkSurface.command) {
    failures.push(`run-provenance.json command mismatch: expected ${benchmarkSurface.command}, got ${data.command}`);
  }
  if (data.benchmark_surface !== undefined && data.benchmark_surface !== benchmarkSurface.id) {
    failures.push(`run-provenance.json benchmark_surface mismatch: expected ${benchmarkSurface.id}, got ${data.benchmark_surface}`);
  }
  if (
    data.benchmark_command_args !== undefined &&
    !stringArraysEqual(data.benchmark_command_args, benchmarkSurface.command_args)
  ) {
    failures.push(`run-provenance.json benchmark_command_args mismatch for ${benchmarkSurface.id}`);
  }
  if (!Number.isSafeInteger(data.timeout_ms) || data.timeout_ms <= 0) {
    failures.push(`run-provenance.json timeout_ms is invalid: ${data.timeout_ms}`);
  }
  const sweeps = data.process_sweeps;
  if (!sweeps || typeof sweeps !== "object" || Array.isArray(sweeps)) {
    failures.push("run-provenance.json missing process_sweeps");
  } else {
    if (!sweeps.preflight || sweeps.preflight.clean !== true) {
      failures.push("run-provenance.json preflight process sweep is not clean");
    }
    if (!sweeps.final || sweeps.final.clean !== true) {
      failures.push("run-provenance.json final process sweep is not clean");
    }
    if (!Array.isArray(sweeps.roots) || sweeps.roots.length === 0) {
      failures.push("run-provenance.json process_sweeps.roots is empty");
    }
  }
  if (data.git && data.git.head_sha && !/^[a-f0-9]{40}$/i.test(String(data.git.head_sha))) {
    failures.push(`run-provenance.json git.head_sha is not a 40-char hex SHA: ${data.git.head_sha}`);
  }
  if (data.git && data.git.dirty !== undefined && typeof data.git.dirty !== "boolean") {
    failures.push("run-provenance.json git.dirty must be boolean when present");
  }
  if (data.pre_generated_machine_cache_required !== true) {
    failures.push("run-provenance.json pre_generated_machine_cache_required must be true");
  }
  if (data.machine_cache_generation_measured !== false) {
    failures.push("run-provenance.json machine_cache_generation_measured must be false");
  }
  if (data.machine_cache_command_writes_disabled !== true) {
    failures.push("run-provenance.json machine_cache_command_writes_disabled must be true");
  }
  if (data.machine_cache_write_env_for_cache_on !== "0") {
    failures.push(`run-provenance.json machine_cache_write_env_for_cache_on mismatch: ${data.machine_cache_write_env_for_cache_on}`);
  }
  if (
    data.machine_cache_write_env_for_all_samples !== undefined &&
    data.machine_cache_write_env_for_all_samples !== "0"
  ) {
    failures.push(
      `run-provenance.json machine_cache_write_env_for_all_samples mismatch: ${data.machine_cache_write_env_for_all_samples}`,
    );
  }
  if (data.cache_artifact_snapshot_timing !== "before_warmups_and_after_samples") {
    failures.push(`run-provenance.json cache_artifact_snapshot_timing mismatch: ${data.cache_artifact_snapshot_timing}`);
  }
  if (data.git && data.git.status_short !== undefined && !Array.isArray(data.git.status_short)) {
    failures.push("run-provenance.json git.status_short must be an array when present");
  }
  if (data.git && data.git.dirty === true && Array.isArray(data.git.status_short) && data.git.status_short.length === 0) {
    warnings.push("run-provenance.json marks git.dirty true but status_short is empty");
  }
  if (data.env !== undefined && (data.env === null || typeof data.env !== "object" || Array.isArray(data.env))) {
    failures.push("run-provenance.json env must be an object when present");
  }
  if (data.env && typeof data.env === "object" && !Array.isArray(data.env)) {
    for (const key of Object.keys(data.env)) {
      if (/TOKEN|SECRET|PASSWORD|AUTH|KEY/i.test(key)) {
        failures.push(`run-provenance.json env key is not allowed because it may expose secrets: ${key}`);
      }
    }
  }

  const provenanceFailures = failures.filter((failure) => failure.startsWith("run-provenance.json"));
  return {
    present: true,
    file_record: fileRecord(provenancePath, inputDir),
    data,
    verified: provenanceFailures.length === 0,
  };
}

function buildLogTextProvesLocalReleaseBuild(text) {
  return (
    text.includes("Finished `release` profile") &&
    (text.includes("Compiling tauri-cli") || text.includes(EXPECTED_LOCAL_BUILD_COMMAND))
  );
}

function buildLogTextContainsFailureOutput(text) {
  return /(^|\n)\s*error:/i.test(text) || text.includes("could not compile");
}

function pathIsUnderTestOutputs(file) {
  const resolved = path.resolve(file);
  const rootWithSep = TEST_OUTPUT_ROOT.endsWith(path.sep) ? TEST_OUTPUT_ROOT : `${TEST_OUTPUT_ROOT}${path.sep}`;
  return resolved === TEST_OUTPUT_ROOT || resolved.toLowerCase().startsWith(rootWithSep.toLowerCase());
}

function validateLocalBuildLogReceipt(buildLog, failures) {
  if (!buildLog || typeof buildLog !== "object" || Array.isArray(buildLog)) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} missing build_log evidence`);
    return;
  }
  if (!buildLog.path || typeof buildLog.path !== "string") {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} build_log.path is required`);
    return;
  }
  if (!pathIsUnderTestOutputs(buildLog.path)) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} build_log.path must stay under ${TEST_OUTPUT_ROOT}`);
    return;
  }
  if (!fs.existsSync(buildLog.path) || !fs.statSync(buildLog.path).isFile()) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} build_log file is missing: ${buildLog.path}`);
    return;
  }
  if (!Number.isSafeInteger(buildLog.bytes) || buildLog.bytes <= 0) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} build_log.bytes must be a positive integer`);
  }
  if (!isHexSha256(buildLog.sha256)) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} build_log.sha256 is invalid: ${buildLog.sha256}`);
  }

  const stat = fs.statSync(buildLog.path);
  const actualSha256 = sha256(buildLog.path);
  if (buildLog.bytes !== stat.size || buildLog.sha256 !== actualSha256) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} build_log identity does not match the recorded file`);
  }

  const text = readText(buildLog.path);
  if (buildLogTextContainsFailureOutput(text)) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} build_log contains build failure output`);
  }
  if (!buildLogTextProvesLocalReleaseBuild(text)) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} build_log does not prove a completed tauri-cli release build`);
  }
}

function validateLocalBuildReceipt(inputDir, binaries, runProvenance, failures) {
  const receiptPath = path.join(inputDir, LOCAL_BUILD_RECEIPT_FILE);
  const file_record = fileRecord(receiptPath, inputDir);
  if (!fs.existsSync(receiptPath)) {
    return {
      present: false,
      file_record,
      data: null,
      verified: false,
      status: "missing",
    };
  }

  const failureCountAtStart = failures.length;
  let data = null;
  try {
    data = readJson(receiptPath);
  } catch (error) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} is not valid JSON: ${error.message}`);
  }
  if (!data || typeof data !== "object" || Array.isArray(data)) {
    return {
      present: true,
      file_record,
      data,
      verified: false,
      status: "invalid",
    };
  }

  if (data.schema !== LOCAL_BUILD_RECEIPT_SCHEMA) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} schema mismatch: ${data.schema}`);
  }
  if (data.target !== "local-current-source") {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} target must be local-current-source`);
  }
  if (data.release_build_run !== true) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} release_build_run must be true`);
  }
  if (data.command !== EXPECTED_LOCAL_BUILD_COMMAND) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} command does not match the expected local release build command`);
  }
  if (!stringArraysEqual(data.argv, EXPECTED_LOCAL_BUILD_ARGV)) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} argv does not match the expected local release build command`);
  }
  if (data.profile !== "release") {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} profile must be release`);
  }
  if (data.package !== "tauri-cli") {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} package must be tauri-cli`);
  }
  if (data.binary_name !== "cargo-tauri") {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} binary_name must be cargo-tauri`);
  }
  if (!stringArraysEqual(data.features, ["dx-machine-cache-mmap"])) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} features must be exactly dx-machine-cache-mmap`);
  }
  validateLocalBuildLogReceipt(data.build_log, failures);

  const git = data.git;
  if (!git || typeof git !== "object" || Array.isArray(git)) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} missing git metadata`);
  } else if (!isGitSha(git.head_sha)) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} git.head_sha is not a 40-char hex SHA: ${git.head_sha}`);
  } else {
    if (git.dirty !== false) {
      failures.push(`${LOCAL_BUILD_RECEIPT_FILE} git.dirty must be false for a SHA-bound current-source claim`);
    }
    if (!Array.isArray(git.status_short) || git.status_short.length !== 0) {
      failures.push(`${LOCAL_BUILD_RECEIPT_FILE} git.status_short must be an empty array for a SHA-bound current-source claim`);
    }
  }
  const provenanceHead = runProvenance.data?.git?.head_sha;
  if (isGitSha(git?.head_sha) && provenanceHead && git.head_sha !== provenanceHead) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} git.head_sha does not match run-provenance.json git.head_sha`);
  }
  if (runProvenance.data?.git?.dirty !== false) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} requires run-provenance.json git.dirty to be false`);
  }
  if (
    runProvenance.data?.git?.status_short !== undefined &&
    (!Array.isArray(runProvenance.data.git.status_short) || runProvenance.data.git.status_short.length !== 0)
  ) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} requires run-provenance.json git.status_short to be empty`);
  }
  const provenanceRepoRoot = runProvenance.data?.git?.repo_root;
  if (data.repo_root && provenanceRepoRoot && !pathsMatch(data.repo_root, provenanceRepoRoot)) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} repo_root does not match run-provenance.json git.repo_root`);
  }
  if (data.cwd && provenanceRepoRoot && !pathsMatch(data.cwd, provenanceRepoRoot)) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} cwd does not match run-provenance.json git.repo_root`);
  }

  const receiptBinary = data.binary;
  if (!receiptBinary || typeof receiptBinary !== "object" || Array.isArray(receiptBinary)) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} missing binary identity`);
  } else {
    if (!receiptBinary.path || typeof receiptBinary.path !== "string") {
      failures.push(`${LOCAL_BUILD_RECEIPT_FILE} binary.path is required`);
    }
    if (!Number.isSafeInteger(receiptBinary.bytes) || receiptBinary.bytes <= 0) {
      failures.push(`${LOCAL_BUILD_RECEIPT_FILE} binary.bytes must be a positive integer`);
    }
    if (!isHexSha256(receiptBinary.sha256)) {
      failures.push(`${LOCAL_BUILD_RECEIPT_FILE} binary.sha256 is invalid: ${receiptBinary.sha256}`);
    }
  }

  const localBinaries = binaries.filter((binary) => binary.target === "local-current-source");
  if (localBinaries.length === 0) {
    failures.push(`${LOCAL_BUILD_RECEIPT_FILE} has no local-current-source binaries to validate against`);
  }
  if (receiptBinary && typeof receiptBinary === "object" && !Array.isArray(receiptBinary)) {
    const matchingLocalBinary = localBinaries.some(
      (binary) =>
        pathsMatch(binary.path, receiptBinary.path) &&
        binary.actual_bytes === receiptBinary.bytes &&
        binary.actual_sha256 === receiptBinary.sha256 &&
        binary.bytes_match &&
        binary.sha256_match,
    );
    if (!matchingLocalBinary) {
      failures.push(`${LOCAL_BUILD_RECEIPT_FILE} binary identity does not match the measured local-current-source binary`);
    }
  }

  return {
    present: true,
    file_record,
    data,
    verified: failures.length === failureCountAtStart,
    status: failures.length === failureCountAtStart ? "verified" : "invalid",
  };
}

function processSweepIsClean(text) {
  return (
    text.includes("no-matching-heavy-bun-build-processes") ||
    text.includes("No G:\\Dx\\bun cargo/rustc/rustup/bun build processes were present.")
  );
}

function validateProcessSweepJson(inputDir, textFile, jsonFile, phase, failures) {
  const file_record = fileRecord(jsonFile, inputDir);
  if (!fs.existsSync(jsonFile)) {
    failures.push(`missing required ${phase} process sweep JSON: ${jsonFile}`);
    return {
      present: false,
      file_record,
      data: null,
      verified: false,
    };
  }

  let data = null;
  try {
    data = readJson(jsonFile);
  } catch (error) {
    failures.push(`${phase} process sweep JSON is not valid JSON: ${error.message}`);
    return {
      present: true,
      file_record,
      data: null,
      verified: false,
    };
  }

  const failureCountAtStart = failures.length;
  if (!data || typeof data !== "object" || Array.isArray(data)) {
    failures.push(`${phase} process sweep JSON must be an object`);
  } else {
    if (data.schema !== PROCESS_SWEEP_SCHEMA) {
      failures.push(`${phase} process sweep JSON schema mismatch: ${data.schema}`);
    }
    if (data.phase !== phase) {
      failures.push(`${phase} process sweep JSON phase mismatch: ${data.phase}`);
    }
    if (data.clean !== true) {
      failures.push(`${phase} process sweep JSON clean must be true`);
    }
    if (!Array.isArray(data.matched_processes)) {
      failures.push(`${phase} process sweep JSON matched_processes must be an array`);
    } else if (data.matched_processes.length !== 0) {
      failures.push(`${phase} process sweep JSON clean=true but matched_processes is not empty`);
    }
    if (!Array.isArray(data.roots) || data.roots.length === 0) {
      failures.push(`${phase} process sweep JSON roots must be a non-empty array`);
    }
    if (!Array.isArray(data.command_names) || data.command_names.length === 0) {
      failures.push(`${phase} process sweep JSON command_names must be a non-empty array`);
    }
    if (!Number.isSafeInteger(data.self_pid) || data.self_pid <= 0) {
      failures.push(`${phase} process sweep JSON self_pid is invalid: ${data.self_pid}`);
    }
    if (data.text_log !== path.basename(textFile)) {
      failures.push(`${phase} process sweep JSON text_log mismatch: ${data.text_log}`);
    }
    if (data.text_log_sha256 !== sha256(textFile)) {
      failures.push(`${phase} process sweep JSON text_log_sha256 mismatch`);
    }
  }

  return {
    present: true,
    file_record,
    data,
    verified: failures.length === failureCountAtStart,
  };
}

function expectedCacheEnvValue(cacheEnv) {
  if (cacheEnv === "on") return "1";
  if (cacheEnv === "off") return "0";
  if (cacheEnv === "unset") return "<unset>";
  return null;
}

function expectedCacheWriteEnvValue(cacheEnv, writeEnvForAllSamples) {
  if (writeEnvForAllSamples) return "0";
  if (cacheEnv === "on") return "0";
  if (cacheEnv === "off") return "<unset>";
  if (cacheEnv === "unset") return "<unset>";
  return null;
}

function benchmarkCaseKey(target, cacheEnv, kind) {
  return `${target}\u0000${cacheEnv}\u0000${kind}`;
}

function normalizePathForComparison(value) {
  if (typeof value !== "string" || value.trim() === "") return null;
  const resolved = path.resolve(value);
  return process.platform === "win32" ? resolved.toLowerCase() : resolved;
}

function pathsMatch(left, right) {
  const normalizedLeft = normalizePathForComparison(left);
  const normalizedRight = normalizePathForComparison(right);
  return normalizedLeft !== null && normalizedRight !== null && normalizedLeft === normalizedRight;
}

function expectedSampleArgs(binaryRow, benchmarkSurface) {
  return binaryRow.script
    ? `${binaryRow.script} ${benchmarkSurface.command}`
    : benchmarkSurface.command;
}

function expectedSampleArgv(binaryRow, benchmarkSurface) {
  return binaryRow.script
    ? [binaryRow.script, ...benchmarkSurface.command_args]
    : [...benchmarkSurface.command_args];
}

function parseSampleArgvJson(value, sampleLabel, failures) {
  let parsed = null;
  try {
    parsed = JSON.parse(value);
  } catch (error) {
    failures.push(`sample invocation argv_json invalid for ${sampleLabel}: ${error.message}`);
    return null;
  }
  if (!Array.isArray(parsed) || parsed.some((entry) => typeof entry !== "string")) {
    failures.push(`sample invocation argv_json invalid for ${sampleLabel}: expected string array`);
    return null;
  }
  return parsed;
}

function findSameBinaryCachePair(binaries) {
  for (const target of CACHE_COMPARISON_TARGETS) {
    const cacheOn = binaries.find((binary) => binary.target === target && binary.cache_env === "on");
    const cacheOff = binaries.find((binary) => binary.target === target && binary.cache_env === "off");
    if (
      cacheOn &&
      cacheOff &&
      cacheOn.path === cacheOff.path &&
      cacheOn.actual_bytes === cacheOff.actual_bytes &&
      cacheOn.actual_sha256 === cacheOff.actual_sha256
    ) {
      return { target, cache_on: cacheOn, cache_off: cacheOff };
    }
  }
  return null;
}

function listMachineFiles(root) {
  const files = [];
  if (!fs.existsSync(root)) return files;
  const stack = [root];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const entryPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(entryPath);
      } else if (entry.isFile() && entry.name.endsWith(".machine")) {
        files.push(entryPath);
      }
    }
  }
  return files.sort();
}

function writeJson(file, value) {
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function writeReport(file, check, manifest) {
  const lines = [
    "# No-Build Benchmark Governance",
    "",
    `Status: ${check.status}`,
    `Evidence strength: ${manifest.future_evidence.strength}`,
    "",
    "## Verified",
    "",
    `- Summary recomputed from samples: ${check.summary_recomputed_from_samples}`,
    `- Binary hashes verified: ${check.binary_hashes_verified}`,
    `- Output equivalence basis: ${check.output_equivalence_basis}`,
    `- Cache artifacts fingerprinted: ${check.cache_artifacts_fingerprinted}`,
    `- Cache artifact mutation during run verified: ${check.cache_artifact_mutation_during_run_verified}`,
    `- Run provenance verified: ${check.run_provenance_verified}`,
    `- Local current-source build receipt verified: ${check.local_current_source_build_receipt_verified}`,
    "",
    "## Claim Gates",
    "",
    `- Same-binary cache-on/off claim allowed: ${manifest.claim_gates.cache_on_vs_off_same_binary_claim_allowed}`,
    `- Current-source release speed claim allowed: ${manifest.claim_gates.current_source_release_speed_claim_allowed}`,
    `- Official release comparison claim allowed: ${manifest.claim_gates.official_release_comparison_claim_allowed}`,
    `- Faster-than-upstream claim allowed: ${manifest.claim_gates.faster_than_upstream_claim_allowed}`,
    `- Default-on readiness claim allowed: ${manifest.claim_gates.default_on_readiness_claim_allowed}`,
    `- App/WebView/IPC/build/bundle claim allowed: ${manifest.claim_gates.app_runtime_webview_ipc_build_bundle_claim_allowed}`,
    "",
    "## Allowed Claims",
    "",
    ...(manifest.allowed_claims.length > 0
      ? manifest.allowed_claims.map((claim) => `- ${claim.text}`)
      : ["- none"]),
    "",
    "## Blocked Claims",
    "",
    ...manifest.blocked_claims.map((claim) => `- ${claim.claim_id}: ${claim.reason}`),
    "",
    "## Boundaries",
    "",
    `- This checks existing files from ${manifest.run_id}; it does not rerun benchmarks.`,
    ...manifest.limitations.map((limitation) => `- ${limitation}`),
    "",
    "## Warnings",
    "",
    ...check.warnings.map((warning) => `- ${warning}`),
    "",
    "## Failures",
    "",
    ...(check.failures.length > 0 ? check.failures.map((failure) => `- ${failure}`) : ["- none"]),
    "",
  ];
  fs.writeFileSync(file, lines.join("\n"), "utf8");
}

function main() {
  const args = parseArgs(process.argv);
  const inputDir = assertUnderTestOutputs(args.input, "input");
  const outDir = assertUnderTestOutputs(args.out, "out");
  fs.mkdirSync(outDir, { recursive: true });

  const failures = [];
  const warnings = [];
  const requiredFiles = {
    meta: path.join(inputDir, "wave7-meta.json"),
    samples: path.join(inputDir, "samples.csv"),
    summary: path.join(inputDir, "summary.csv"),
    summaryTable: path.join(inputDir, "summary-table.txt"),
    binaries: path.join(inputDir, "binaries.csv"),
    report: path.join(inputDir, "REPORT.md"),
    preflightProcessLog: path.join(inputDir, "wave7-process-check-preflight.log"),
    processLog: path.join(inputDir, "wave7-process-check-final.log"),
  };
  const processSweepJsonFiles = {
    preflight: path.join(inputDir, "wave7-process-check-preflight.json"),
    final: path.join(inputDir, "wave7-process-check-final.json"),
  };

  for (const [name, file] of Object.entries(requiredFiles)) {
    if (!fs.existsSync(file)) failures.push(`missing required ${name} file: ${file}`);
  }
  if (failures.length > 0) {
    throw new Error(failures.join("; "));
  }

  const meta = readJson(requiredFiles.meta);
  const samplesText = readText(requiredFiles.samples);
  const summaryText = readText(requiredFiles.summary);
  const binariesText = readText(requiredFiles.binaries);
  const samples = parseCsv(samplesText);
  const summaryRows = parseCsv(summaryText);
  const binaryRows = parseCsv(binariesText);
  const expectedCases = binaryRows.length;
  const expectedTotalSamples = expectedCases * EXPECTED_SAMPLES_PER_CASE;
  if (!EXPECTED_META_SCHEMAS.includes(meta.schema)) {
    failures.push(`wave7-meta.json schema mismatch: ${meta.schema}`);
  }
  const isCommandBenchmarkV2 = meta.schema === COMMAND_BENCHMARK_META_SCHEMA_V2;
  const writeEnvForAllSamples = meta.machine_cache_write_env_for_all_samples === "0";
  if (
    meta.machine_cache_write_env_for_all_samples !== undefined &&
    meta.machine_cache_write_env_for_all_samples !== "0"
  ) {
    failures.push(
      `wave7-meta.json machine_cache_write_env_for_all_samples mismatch: ${meta.machine_cache_write_env_for_all_samples}`,
    );
  }
  if (isCommandBenchmarkV2 && !writeEnvForAllSamples) {
    failures.push("wave7-meta.json machine_cache_write_env_for_all_samples must be 0 for command benchmark v2");
  }
  let benchmarkSurface = null;
  try {
    benchmarkSurface = getBenchmarkSurface(meta.benchmark_surface || DEFAULT_BENCHMARK_SURFACE_ID);
  } catch (error) {
    failures.push(error.message);
    benchmarkSurface = getBenchmarkSurface(DEFAULT_BENCHMARK_SURFACE_ID);
  }
  if (meta.benchmark_command !== undefined && meta.benchmark_command !== benchmarkSurface.command) {
    failures.push(`wave7-meta.json benchmark_command mismatch: expected ${benchmarkSurface.command}, got ${meta.benchmark_command}`);
  }
  if (
    meta.benchmark_command_args !== undefined &&
    !stringArraysEqual(meta.benchmark_command_args, benchmarkSurface.command_args)
  ) {
    failures.push(`wave7-meta.json benchmark_command_args mismatch for ${benchmarkSurface.id}`);
  }
  if (meta.no_build !== true) failures.push("wave7-meta.json no_build is not true");
  if (meta.warmups_per_case !== EXPECTED_WARMUPS) {
    failures.push(`wave7-meta.json warmups_per_case expected ${EXPECTED_WARMUPS}, got ${meta.warmups_per_case}`);
  }
  if (meta.samples_per_case !== EXPECTED_SAMPLES_PER_CASE) {
    failures.push(`wave7-meta.json samples_per_case expected ${EXPECTED_SAMPLES_PER_CASE}, got ${meta.samples_per_case}`);
  }
  for (const [key, file] of [
    ["samples_csv", requiredFiles.samples],
    ["summary_csv", requiredFiles.summary],
    ["binaries_csv", requiredFiles.binaries],
  ]) {
    if (path.resolve(meta[key] ?? "") !== path.resolve(file)) {
      failures.push(`wave7-meta.json ${key} does not match artifact path`);
    }
  }
  if (expectedCases === 0) {
    failures.push("binaries.csv must contain at least one benchmark case");
  }
  if (samples.length !== expectedTotalSamples) {
    failures.push(`samples.csv row count expected ${expectedTotalSamples}, got ${samples.length}`);
  }
  if (summaryRows.length !== expectedCases) {
    failures.push(`summary.csv row count expected ${expectedCases}, got ${summaryRows.length}`);
  }

  const requiredSampleColumns = [
    "target",
    "target_kind",
    "cache_env",
    "tauri_dx_machine_cache_env",
    "tauri_dx_machine_cache_write_env",
    "phase",
    "iteration",
    "elapsed_ms",
    "exit_code",
    "signal",
    "timed_out",
    "spawn_error",
    "command",
    "args",
    ...(isCommandBenchmarkV2 ? ["argv_json"] : []),
    "cwd",
    "stdout_first",
    "stderr_first",
  ];
  const requiredSummaryColumns = [
    "target",
    "cache_env",
    "target_kind",
    "runs",
    "median_ms",
    "p95_ms",
    "min_ms",
    "max_ms",
    "mean_ms",
  ];
  const requiredBinaryColumns = [
    "target",
    "kind",
    "cache_env",
    "path",
    "script",
    "bytes",
    "sha256",
    "last_write_utc",
    ...(isCommandBenchmarkV2 ? ["role", "source_kind", "invocation_kind"] : []),
  ];
  const optionalOutputHashColumns = ["stdout_sha256", "stderr_sha256", "stdout_bytes", "stderr_bytes"];
  const optionalOutputPathColumns = ["stdout_path", "stderr_path"];
  const sampleColumns = csvHeaders(samplesText);
  const summaryColumns = csvHeaders(summaryText);
  const binaryColumns = csvHeaders(binariesText);
  const missingSampleColumns = missingColumns(sampleColumns, requiredSampleColumns);
  if (missingSampleColumns.length > 0) failures.push(`samples.csv missing required columns: ${missingSampleColumns.join(", ")}`);
  const missingSummaryColumns = missingColumns(summaryColumns, requiredSummaryColumns);
  if (missingSummaryColumns.length > 0) failures.push(`summary.csv missing required columns: ${missingSummaryColumns.join(", ")}`);
  const missingBinaryColumns = missingColumns(binaryColumns, requiredBinaryColumns);
  if (missingBinaryColumns.length > 0) failures.push(`binaries.csv missing required columns: ${missingBinaryColumns.join(", ")}`);
  for (const column of requiredSampleColumns) {
    if (!sampleColumns.includes(column)) failures.push(`samples.csv missing column: ${column}`);
  }
  const hasAnyOutputHashColumn = hasAnyColumn(sampleColumns, optionalOutputHashColumns);
  const hasFullOutputHashColumns = missingColumns(sampleColumns, optionalOutputHashColumns).length === 0;
  if (hasAnyOutputHashColumn && !hasFullOutputHashColumns) {
    failures.push(
      `samples.csv output hash evidence is incomplete; missing ${missingColumns(sampleColumns, optionalOutputHashColumns).join(", ")}`,
    );
  }
  if (!hasFullOutputHashColumns) {
    warnings.push("samples.csv lacks full stdout/stderr hash and byte columns; output equivalence is limited to stdout_first/stderr_first");
  }
  const hasAnyOutputPathColumn = hasAnyColumn(sampleColumns, optionalOutputPathColumns);
  const hasFullOutputPathColumns = missingColumns(sampleColumns, optionalOutputPathColumns).length === 0;
  if (hasAnyOutputPathColumn && !hasFullOutputPathColumns) {
    failures.push(
      `samples.csv output path evidence is incomplete; missing ${missingColumns(sampleColumns, optionalOutputPathColumns).join(", ")}`,
    );
  }
  if (hasFullOutputHashColumns && !hasFullOutputPathColumns) {
    warnings.push("samples.csv has full output hash columns but no stdout_path/stderr_path files for content rehash verification");
  }

  const expectedSampleCwd = meta.fixture ? path.resolve(meta.fixture) : null;
  if (!expectedSampleCwd) {
    failures.push("wave7-meta.json fixture is required to validate sample cwd");
  }

  const binaryRowsByCase = new Map();
  for (const row of binaryRows) {
    const key = benchmarkCaseKey(row.target, row.cache_env, row.kind);
    if (isCommandBenchmarkV2) {
      try {
        validateBenchmarkCase(row);
      } catch (error) {
        failures.push(`binaries.csv case is not allowlisted: ${error.message}`);
      }
    }
    if (binaryRowsByCase.has(key)) {
      failures.push(`binaries.csv duplicate benchmark case: ${key.replace(/\u0000/g, "/")}`);
    } else {
      binaryRowsByCase.set(key, row);
    }
  }

  const caseSummaries = [];
  const sampleGroups = groupBy(
    samples,
    (sample) => benchmarkCaseKey(sample.target, sample.cache_env, sample.target_kind),
  );
  if (sampleGroups.size !== expectedCases) {
    failures.push(`sample group count expected ${expectedCases}, got ${sampleGroups.size}`);
  }
  for (const group of sampleGroups.values()) {
    const first = group[0];
    const elapsed = group.map((sample) => parseMsToThousandths(sample.elapsed_ms));
    if (group.length !== EXPECTED_SAMPLES_PER_CASE) {
      failures.push(
        `sample count for ${first.target}/${first.cache_env}/${first.target_kind} expected ${EXPECTED_SAMPLES_PER_CASE}, got ${group.length}`,
      );
    }
    const iterations = group.map((sample) => Number.parseInt(sample.iteration, 10)).sort((left, right) => left - right);
    const expectedIterations = Array.from({ length: EXPECTED_SAMPLES_PER_CASE }, (_, index) => index + 1);
    if (iterations.join(",") !== expectedIterations.join(",")) {
      failures.push(`sample iterations are not 1..${EXPECTED_SAMPLES_PER_CASE} for ${first.target}/${first.cache_env}`);
    }
    if (group.some((sample) => sample.phase !== "sample")) {
      failures.push(`non-sample phase found for ${first.target}/${first.cache_env}`);
    }
    const expectedEnv = expectedCacheEnvValue(first.cache_env);
    const expectedWriteEnv = expectedCacheWriteEnvValue(first.cache_env, writeEnvForAllSamples);
    if (expectedEnv === null) {
      failures.push(`unknown cache_env in samples for ${first.target}/${first.cache_env}`);
    }
    if (expectedWriteEnv === null) {
      failures.push(`unknown cache_env write policy in samples for ${first.target}/${first.cache_env}`);
    }
    for (const sample of group) {
      const sampleLabel = `${first.target}/${first.cache_env}/${first.target_kind} iteration ${sample.iteration}`;
      const binaryRow = binaryRowsByCase.get(benchmarkCaseKey(first.target, first.cache_env, first.target_kind));
      if (binaryRow) {
        const expectedArgs = expectedSampleArgs(binaryRow, benchmarkSurface);
        const expectedArgv = expectedSampleArgv(binaryRow, benchmarkSurface);
        if (!pathsMatch(sample.command, binaryRow.path)) {
          failures.push(
            `sample invocation command mismatch for ${sampleLabel}: expected ${binaryRow.path}, got ${sample.command}`,
          );
        }
        if (sample.args !== expectedArgs) {
          failures.push(`sample invocation args mismatch for ${sampleLabel}: expected ${expectedArgs}, got ${sample.args}`);
        }
        if (isCommandBenchmarkV2) {
          const actualArgv = parseSampleArgvJson(sample.argv_json, sampleLabel, failures);
          if (actualArgv && !stringArraysEqual(actualArgv, expectedArgv)) {
            failures.push(
              `sample invocation argv_json mismatch for ${sampleLabel}: expected ${JSON.stringify(expectedArgv)}, got ${JSON.stringify(actualArgv)}`,
            );
          }
        }
        if (expectedSampleCwd && !pathsMatch(sample.cwd, expectedSampleCwd)) {
          failures.push(`sample invocation cwd mismatch for ${sampleLabel}: expected ${expectedSampleCwd}, got ${sample.cwd}`);
        }
      }
      if (sample.tauri_dx_machine_cache_env !== expectedEnv) {
        failures.push(
          `TAURI_DX_MACHINE_CACHE mismatch for ${first.target}/${first.cache_env} iteration ${sample.iteration}: expected ${expectedEnv}, got ${sample.tauri_dx_machine_cache_env}`,
        );
      }
      if (sample.tauri_dx_machine_cache_write_env !== expectedWriteEnv) {
        failures.push(
          `TAURI_DX_MACHINE_CACHE_WRITE mismatch for ${first.target}/${first.cache_env} iteration ${sample.iteration}: expected ${expectedWriteEnv}, got ${sample.tauri_dx_machine_cache_write_env}`,
        );
      }
      if (sample.timed_out !== "false") {
        failures.push(`sample timed out for ${first.target}/${first.cache_env} iteration ${sample.iteration}`);
      }
      if ((sample.signal ?? "") !== "") {
        failures.push(`sample received signal for ${first.target}/${first.cache_env} iteration ${sample.iteration}: ${sample.signal}`);
      }
      if ((sample.spawn_error ?? "") !== "") {
        failures.push(`sample spawn error for ${first.target}/${first.cache_env} iteration ${sample.iteration}: ${sample.spawn_error}`);
      }
    }
    if (elapsed.some((value) => !Number.isFinite(value) || value <= 0)) {
      failures.push(`nonpositive or invalid elapsed_ms found for ${first.target}/${first.cache_env}`);
    }
    const summary = summaryRows.find(
      (row) =>
        row.target === first.target &&
        row.cache_env === first.cache_env &&
        row.target_kind === first.target_kind,
    );
    if (!summary) {
      failures.push(`summary.csv missing case: ${first.target}/${first.cache_env}/${first.target_kind}`);
      continue;
    }

    const recomputed = {
      target: first.target,
      cache_env: first.cache_env,
      target_kind: first.target_kind,
      runs: group.length,
      median_ms: median(elapsed),
      p95_ms: p95(elapsed),
      min_ms: minMs(elapsed),
      max_ms: maxMs(elapsed),
      mean_ms: mean(elapsed),
      exit_codes: unique(group.map((sample) => Number.parseInt(sample.exit_code, 10))).sort(),
      distinct_stdout_first: unique(group.map((sample) => sample.stdout_first)).length,
      distinct_stderr_first: unique(group.map((sample) => sample.stderr_first)).length,
      stdout_first: unique(group.map((sample) => sample.stdout_first))[0] ?? "",
      stderr_first: unique(group.map((sample) => sample.stderr_first))[0] ?? "",
      distinct_stdout_sha256: null,
      distinct_stderr_sha256: null,
      stdout_sha256: null,
      stderr_sha256: null,
      stdout_bytes: null,
      stderr_bytes: null,
      summary_matches: true,
    };
    if (hasFullOutputHashColumns) {
      const stdoutHashes = [];
      const stderrHashes = [];
      const stdoutBytes = [];
      const stderrBytes = [];
      for (const sample of group) {
        for (const stream of ["stdout", "stderr"]) {
          const hashColumn = `${stream}_sha256`;
          const bytesColumn = `${stream}_bytes`;
          const hash = sample[hashColumn];
          const bytes = parseNonnegativeInteger(sample[bytesColumn]);
          if (!isHexSha256(hash)) {
            failures.push(`output ${hashColumn} invalid for ${first.target}/${first.cache_env} iteration ${sample.iteration}: ${hash}`);
          }
          if (bytes === null) {
            failures.push(`output ${bytesColumn} invalid for ${first.target}/${first.cache_env} iteration ${sample.iteration}: ${sample[bytesColumn]}`);
          }
          if (hasFullOutputPathColumns) {
            const outputPath = evidencePath(
              inputDir,
              sample[`${stream}_path`],
              `samples.csv ${stream}_path`,
              failures,
            );
            if (!outputPath || !fs.existsSync(outputPath)) {
              failures.push(`output ${stream}_path missing for ${first.target}/${first.cache_env} iteration ${sample.iteration}: ${sample[`${stream}_path`]}`);
            } else {
              const outputStat = fs.statSync(outputPath);
              const outputHash = sha256(outputPath);
              if (bytes !== null && outputStat.size !== bytes) {
                failures.push(`output ${stream}_bytes mismatch for ${first.target}/${first.cache_env} iteration ${sample.iteration}`);
              }
              if (isHexSha256(hash) && outputHash !== hash) {
                failures.push(`output ${stream}_sha256 mismatch for ${first.target}/${first.cache_env} iteration ${sample.iteration}`);
              }
            }
          }
        }
        stdoutHashes.push(sample.stdout_sha256);
        stderrHashes.push(sample.stderr_sha256);
        stdoutBytes.push(sample.stdout_bytes);
        stderrBytes.push(sample.stderr_bytes);
      }
      recomputed.distinct_stdout_sha256 = unique(stdoutHashes).length;
      recomputed.distinct_stderr_sha256 = unique(stderrHashes).length;
      recomputed.stdout_sha256 = unique(stdoutHashes)[0] ?? "";
      recomputed.stderr_sha256 = unique(stderrHashes)[0] ?? "";
      recomputed.stdout_bytes = unique(stdoutBytes)[0] ?? "";
      recomputed.stderr_bytes = unique(stderrBytes)[0] ?? "";
      if (recomputed.distinct_stdout_sha256 !== 1 || recomputed.distinct_stderr_sha256 !== 1) {
        failures.push(`unstable full stdout/stderr hashes in samples for ${first.target}/${first.cache_env}`);
      }
      if (unique(stdoutBytes).length !== 1 || unique(stderrBytes).length !== 1) {
        failures.push(`unstable stdout/stderr byte counts in samples for ${first.target}/${first.cache_env}`);
      }
    }

    for (const [key, left, right] of [
      ["runs", recomputed.runs, Number.parseFloat(summary.runs)],
      ["median_ms", recomputed.median_ms, Number.parseFloat(summary.median_ms)],
      ["p95_ms", recomputed.p95_ms, Number.parseFloat(summary.p95_ms)],
      ["min_ms", recomputed.min_ms, Number.parseFloat(summary.min_ms)],
      ["max_ms", recomputed.max_ms, Number.parseFloat(summary.max_ms)],
      ["mean_ms", recomputed.mean_ms, Number.parseFloat(summary.mean_ms)],
    ]) {
      if (!near(left, right)) {
        recomputed.summary_matches = false;
        failures.push(
          `summary mismatch for ${first.target}/${first.cache_env} ${key}: recomputed=${left} summary=${right}`,
        );
      }
    }
    if (recomputed.exit_codes.length !== 1 || recomputed.exit_codes[0] !== 0) {
      failures.push(`non-zero exit code in samples for ${first.target}/${first.cache_env}`);
    }
    if (recomputed.distinct_stdout_first !== 1 || recomputed.distinct_stderr_first !== 1) {
      failures.push(`unstable stdout_first/stderr_first in samples for ${first.target}/${first.cache_env}`);
    }
    if (recomputed.stdout_first !== benchmarkSurface.expected_stdout_first) {
      failures.push(
        `stdout_first mismatch for ${first.target}/${first.cache_env}: expected ${benchmarkSurface.expected_stdout_first}, got ${recomputed.stdout_first}`,
      );
    }
    if (!recomputed.stderr_first.includes(benchmarkSurface.expected_stderr_first_includes)) {
      failures.push(
        `stderr_first does not contain expected ${benchmarkSurface.id} line for ${first.target}/${first.cache_env}`,
      );
    }
    caseSummaries.push(recomputed);
  }

  const allStdout = unique(caseSummaries.map((entry) => entry.stdout_first));
  const allStderr = unique(caseSummaries.map((entry) => entry.stderr_first));
  const firstLineOutputEquivalent = allStdout.length === 1 && allStderr.length === 1;
  if (!firstLineOutputEquivalent) {
    failures.push("stdout_first/stderr_first are not equivalent across benchmark cases");
  }
  const allStdoutHashes = hasFullOutputHashColumns ? unique(caseSummaries.map((entry) => entry.stdout_sha256)) : [];
  const allStderrHashes = hasFullOutputHashColumns ? unique(caseSummaries.map((entry) => entry.stderr_sha256)) : [];
  const fullHashOutputEquivalent =
    hasFullOutputHashColumns && allStdoutHashes.length === 1 && allStderrHashes.length === 1;
  if (hasFullOutputHashColumns && !fullHashOutputEquivalent) {
    failures.push("stdout_sha256/stderr_sha256 are not equivalent across benchmark cases");
  }
  const outputEquivalent = hasFullOutputHashColumns
    ? firstLineOutputEquivalent && fullHashOutputEquivalent
    : firstLineOutputEquivalent;

  const sampleKeys = new Set(caseSummaries.map((entry) => benchmarkCaseKey(entry.target, entry.cache_env, entry.target_kind)));
  const summaryKeys = new Set(summaryRows.map((entry) => benchmarkCaseKey(entry.target, entry.cache_env, entry.target_kind)));
  const binaryKeys = new Set(binaryRows.map((entry) => benchmarkCaseKey(entry.target, entry.cache_env, entry.kind)));
  for (const key of sampleKeys) {
    if (!summaryKeys.has(key)) failures.push(`summary.csv missing sample key: ${key.replace(/\u0000/g, "/")}`);
    if (!binaryKeys.has(key)) failures.push(`binaries.csv missing sample key: ${key.replace(/\u0000/g, "/")}`);
  }

  const binaries = binaryRows.map((row) => {
    const record = {
      target: row.target,
      kind: row.kind,
      cache_env: row.cache_env,
      role: row.role ?? "",
      source_kind: row.source_kind ?? "",
      invocation_kind: row.invocation_kind ?? "",
      path: row.path,
      script: row.script,
      expected_bytes: Number.parseInt(row.bytes, 10),
      expected_sha256: row.sha256,
      expected_last_write_utc: row.last_write_utc,
      present: false,
      bytes_match: false,
      sha256_match: false,
      actual_bytes: null,
      actual_sha256: null,
      actual_last_write_utc: null,
      script_record: row.script ? fileRecord(row.script, inputDir) : null,
    };
    if (!fs.existsSync(row.path)) {
      failures.push(`binary path missing for ${row.target}/${row.cache_env}: ${row.path}`);
      return record;
    }
    const stat = fs.statSync(row.path);
    const hash = sha256(row.path);
    record.present = true;
    record.actual_bytes = stat.size;
    record.actual_sha256 = hash;
    record.actual_last_write_utc = stat.mtime.toISOString();
    record.bytes_match = stat.size === record.expected_bytes;
    record.sha256_match = hash === row.sha256;
    if (!record.bytes_match) failures.push(`binary byte length mismatch for ${row.target}/${row.cache_env}`);
    if (!record.sha256_match) failures.push(`binary sha256 mismatch for ${row.target}/${row.cache_env}`);
    if (row.script && !record.script_record.present) {
      failures.push(`wrapper script path missing for ${row.target}/${row.cache_env}: ${row.script}`);
    }
    return record;
  });

  const sameBinaryCachePair = findSameBinaryCachePair(binaries);
  const sameBinaryCacheOnOff = sameBinaryCachePair !== null;
  if (!sameBinaryCacheOnOff) {
    failures.push("local cache on/off rows do not point to the same binary identity");
  }

  const fixturePath = expectedSampleCwd;
  const cacheArtifacts =
    fixturePath && fs.existsSync(fixturePath)
      ? listMachineFiles(fixturePath).map((file) => fileRecord(file, fixturePath))
      : [];
  if (!fixturePath || !fs.existsSync(fixturePath)) {
    warnings.push(`fixture path from wave7-meta.json is missing at governance-check time: ${fixturePath}`);
  } else if (cacheArtifacts.length === 0) {
    warnings.push("fixture exists but no .machine cache artifacts were found at governance-check time");
  }
  const cacheArtifactSnapshots = validateCacheArtifactSnapshots(inputDir, failures, warnings);
  const preflightProcessLog = readText(requiredFiles.preflightProcessLog);
  const processLog = readText(requiredFiles.processLog);
  const processSweepJson = isCommandBenchmarkV2
    ? {
        preflight: validateProcessSweepJson(
          inputDir,
          requiredFiles.preflightProcessLog,
          processSweepJsonFiles.preflight,
          "preflight",
          failures,
        ),
        final: validateProcessSweepJson(
          inputDir,
          requiredFiles.processLog,
          processSweepJsonFiles.final,
          "final",
          failures,
        ),
      }
    : {
        preflight: null,
        final: null,
      };
  const preflightProcessClean = isCommandBenchmarkV2
    ? processSweepJson.preflight.verified
    : processSweepIsClean(preflightProcessLog);
  const finalProcessClean = isCommandBenchmarkV2
    ? processSweepJson.final.verified
    : processSweepIsClean(processLog);
  if (!preflightProcessClean) {
    failures.push("wave7-process-check-preflight.log does not record a clean heavy-build process sweep");
  }
  if (!finalProcessClean) {
    failures.push("wave7-process-check-final.log does not record a clean heavy-build process sweep");
  }
  const runProvenance = validateRunProvenance(inputDir, failures, warnings, benchmarkSurface);
  if (
    isCommandBenchmarkV2 &&
    runProvenance.data?.machine_cache_write_env_for_all_samples !== "0"
  ) {
    failures.push("run-provenance.json machine_cache_write_env_for_all_samples must be 0 for command benchmark v2");
  }
  if (isCommandBenchmarkV2) {
    const sweeps = runProvenance.data?.process_sweeps;
    if (sweeps?.preflight?.json_path !== "wave7-process-check-preflight.json") {
      failures.push("run-provenance.json preflight process sweep json_path mismatch for command benchmark v2");
    }
    if (sweeps?.final?.json_path !== "wave7-process-check-final.json") {
      failures.push("run-provenance.json final process sweep json_path mismatch for command benchmark v2");
    }
    if (runProvenance.data?.git?.dirty !== false) {
      failures.push("run-provenance.json git.dirty must be false for command benchmark v2");
    }
    if (
      !Array.isArray(runProvenance.data?.git?.status_short) ||
      runProvenance.data.git.status_short.length !== 0
    ) {
      failures.push("run-provenance.json git.status_short must be empty for command benchmark v2");
    }
  }
  runProvenance.verified =
    runProvenance.verified &&
    failures.every((failure) => !failure.startsWith("run-provenance.json"));
  const localBuildReceipt = validateLocalBuildReceipt(inputDir, binaries, runProvenance, failures);

  const sampleArtifacts = {
    wave7_meta: fileRecord(requiredFiles.meta, inputDir),
    samples_csv: fileRecord(requiredFiles.samples, inputDir),
    summary_csv: fileRecord(requiredFiles.summary, inputDir),
    summary_table_txt: fileRecord(requiredFiles.summaryTable, inputDir),
    binaries_csv: fileRecord(requiredFiles.binaries, inputDir),
    report_md: fileRecord(requiredFiles.report, inputDir),
    process_check_preflight_log: fileRecord(requiredFiles.preflightProcessLog, inputDir),
    process_check_log: fileRecord(requiredFiles.processLog, inputDir),
    process_check_preflight_json: fileRecord(processSweepJsonFiles.preflight, inputDir),
    process_check_final_json: fileRecord(processSweepJsonFiles.final, inputDir),
  };

  const outputValidationFailures = failures.filter((failure) => failure.startsWith("output "));
  const sampleInvocationFailures = failures.filter((failure) => failure.startsWith("sample invocation "));
  const sampleExecutionFailures = failures.filter(
    (failure) =>
      failure.startsWith("sample timed out ") ||
      failure.startsWith("sample received signal ") ||
      failure.startsWith("sample spawn error ") ||
      failure.startsWith("nonpositive or invalid elapsed_ms ") ||
      failure.startsWith("non-zero exit code in samples ") ||
      failure.startsWith("non-sample phase found ") ||
      failure.startsWith("sample count for ") ||
      failure.startsWith("sample iterations are not "),
  );
  const sampleEnvironmentFailures = failures.filter((failure) =>
    failure.startsWith("TAURI_DX_MACHINE_CACHE"),
  );
  const sampleInvocationsVerified =
    sampleInvocationFailures.length === 0 &&
    sampleExecutionFailures.length === 0 &&
    sampleEnvironmentFailures.length === 0 &&
    expectedSampleCwd !== null;
  const fullOutputFileHashesVerified =
    hasFullOutputHashColumns && hasFullOutputPathColumns && outputValidationFailures.length === 0;
  const outputEquivalenceVerified =
    outputEquivalent && (!hasFullOutputPathColumns || fullOutputFileHashesVerified);
  const summaryFailures = failures.filter(
    (failure) =>
      failure.startsWith("summary mismatch") ||
      failure.startsWith("summary.csv ") ||
      failure.startsWith("sample group count "),
  );
  const summaryRecomputedFromSamples = summaryFailures.length === 0;
  const outputEquivalenceBasis = fullOutputFileHashesVerified
    ? "stdout/stderr output files rehashed from stdout_path/stderr_path"
    : hasFullOutputHashColumns
      ? "stdout_sha256/stderr_sha256 columns plus stdout_first/stderr_first"
      : "stdout_first/stderr_first from samples.csv";
  const benchmarkIntegrityVerified =
    preflightProcessClean &&
    finalProcessClean &&
    sampleInvocationsVerified &&
    fullOutputFileHashesVerified &&
    cacheArtifactSnapshots.verified &&
    runProvenance.verified &&
    failures.length === 0;
  const localCurrentSourceCases = caseSummaries.filter((entry) => entry.target === "local-current-source");
  const localCurrentSourceOnCase =
    localCurrentSourceCases.find((entry) => entry.cache_env === "on") ?? localCurrentSourceCases[0] ?? null;
  const officialReleaseCase = caseSummaries.find(
    (entry) => isOfficialReleaseTarget(entry.target) && entry.cache_env === "off",
  );
  const officialReleaseBinaryPresent = binaries.some(
    (binary) => isOfficialReleaseTarget(binary.target) && binary.present && binary.sha256_match,
  );
  const currentSourceReleaseSpeedClaimAllowed =
    localBuildReceipt.verified &&
    localCurrentSourceCases.length > 0 &&
    outputEquivalenceVerified &&
    benchmarkIntegrityVerified &&
    failures.length === 0;
  const officialReleaseComparisonClaimAllowed =
    currentSourceReleaseSpeedClaimAllowed &&
    officialReleaseBinaryPresent &&
    Boolean(officialReleaseCase) &&
    Boolean(localCurrentSourceOnCase);

  const claimGates = {
    cache_on_vs_off_same_binary_claim_allowed:
      sameBinaryCacheOnOff && outputEquivalenceVerified && benchmarkIntegrityVerified && failures.length === 0,
    full_stdout_stderr_equivalence_claim_allowed:
      hasFullOutputHashColumns && fullHashOutputEquivalent && failures.length === 0,
    full_stdout_stderr_rehash_claim_allowed: fullOutputFileHashesVerified && failures.length === 0,
    official_release_snapshot_allowed: officialReleaseBinaryPresent && failures.length === 0,
    current_source_release_speed_claim_allowed: currentSourceReleaseSpeedClaimAllowed,
    official_release_comparison_claim_allowed: officialReleaseComparisonClaimAllowed,
    inspect_official_release_comparison_claim_allowed:
      benchmarkSurface.id === "inspect-wix-upgrade-code" && officialReleaseComparisonClaimAllowed,
    faster_than_upstream_claim_allowed: false,
    default_on_readiness_claim_allowed: false,
    full_cli_speed_claim_allowed: false,
    app_runtime_webview_ipc_build_bundle_claim_allowed: false,
  };
  const cacheComparisonTarget = sameBinaryCachePair ? sameBinaryCachePair.target : CACHE_COMPARISON_TARGETS[0];
  const dxOnCase = caseSummaries.find((entry) => entry.target === cacheComparisonTarget && entry.cache_env === "on");
  const dxOffCase = caseSummaries.find((entry) => entry.target === cacheComparisonTarget && entry.cache_env === "off");
  const cacheOnToOffMedianRatioPercent =
    dxOnCase && dxOffCase ? round3((dxOnCase.median_ms / dxOffCase.median_ms) * 100) : null;
  const officialReleaseComparisonRatioPercent =
    localCurrentSourceOnCase && officialReleaseCase
      ? round3((localCurrentSourceOnCase.median_ms / officialReleaseCase.median_ms) * 100)
      : null;
  const officialReleaseComparisonWinner =
    localCurrentSourceOnCase && officialReleaseCase
      ? localCurrentSourceOnCase.median_ms <= officialReleaseCase.median_ms
        ? "local-current-source cache-on"
        : officialReleaseCase.target
      : null;
  const outputClaimQualifier = fullOutputFileHashesVerified
    ? "Full stdout/stderr bytes were rehashed from per-sample output files."
    : hasFullOutputHashColumns
      ? "Output equivalence was checked by stdout_sha256/stderr_sha256 columns; per-sample output files were not rehashed."
      : "Output equivalence was checked only by stdout_first/stderr_first, not full streams.";
  const allowedClaims = [];
  if (claimGates.cache_on_vs_off_same_binary_claim_allowed) {
    allowedClaims.push({
      claim_id: "same_binary_cache_on_vs_off_median",
      scope: "same_binary_cache_on_vs_off",
      subject: `${cacheComparisonTarget} ${benchmarkSurface.command}`,
      metric: "median_ms",
      ratio_percent: cacheOnToOffMedianRatioPercent,
      text:
        `For this same ${cacheComparisonTarget} binary and same fixture/input snapshot, cache-on measured ` +
        `${cacheOnToOffMedianRatioPercent}% of cache-off by median_ms. ${outputClaimQualifier}`,
    });
  }
  if (claimGates.current_source_release_speed_claim_allowed) {
    allowedClaims.push({
      claim_id: benchmarkSurface.current_source_claim_id,
      scope: benchmarkSurface.current_source_claim_scope,
      subject: `local-current-source ${benchmarkSurface.command}`,
      metric: "median_ms",
      build_git_head_sha: localBuildReceipt.data.git.head_sha,
      text:
        "The measured local-current-source release binary is tied to the recorded git head by " +
        `${LOCAL_BUILD_RECEIPT_FILE}; current-source speed claims are limited to ${benchmarkSurface.evidence_scope} in this benchmark run.`,
    });
  }
  if (claimGates.official_release_comparison_claim_allowed) {
    allowedClaims.push({
      claim_id: benchmarkSurface.official_release_claim_id,
      scope: benchmarkSurface.official_release_claim_scope,
      subject: `local-current-source cache-on vs ${officialReleaseCase.target}`,
      metric: "median_ms",
      ratio_percent: officialReleaseComparisonRatioPercent,
      winner: officialReleaseComparisonWinner,
      text:
        `For ${benchmarkSurface.evidence_scope} only, local-current-source cache-on measured ` +
        `${officialReleaseComparisonRatioPercent}% of ${officialReleaseCase.target} by median_ms; ` +
        `${officialReleaseComparisonWinner} was faster in this run.`,
    });
  }
  const blockedClaims = [];
  if (!claimGates.current_source_release_speed_claim_allowed) {
    blockedClaims.push({
      claim_id: "current_source",
      claim_eligible: false,
      status: "blocked_unproven",
      reason: localBuildReceipt.present
        ? "The local current-source build receipt was present but not verified for this benchmark input."
        : "No local current-source build receipt was present for the measured binary.",
    });
  }
  blockedClaims.push(
    {
      claim_id: "upstream_superiority",
      claim_eligible: false,
      status: "blocked_unproven",
      reason: claimGates.official_release_comparison_claim_allowed
        ? `Only ${benchmarkSurface.evidence_scope} was compared against an official release binary; broad Tauri product superiority is still unproven.`
        : "No receipt-verified current-source official-release comparison was available for this benchmark input.",
    },
    {
      claim_id: "default_on",
      claim_eligible: false,
      status: "blocked_unproven",
      reason: "The DX machine cache remains opt-in and env-gated.",
    },
    {
      claim_id: "app_runtime",
      claim_eligible: false,
      status: "blocked_unproven",
      reason: "No app runtime, WebView startup, IPC, build, bundle, installer, or watcher path was measured.",
    },
    {
      claim_id: "full_cli",
      claim_eligible: false,
      status: "blocked_unproven",
      reason: `Only ${benchmarkSurface.evidence_scope} was measured.`,
    },
  );
  const evidenceStrength =
    failures.length > 0
      ? "failed"
      : fullOutputFileHashesVerified && cacheArtifactSnapshots.verified && runProvenance.verified
        ? "future_complete_no_build"
        : "legacy_partial_no_build";
  const limitations = [];
  if (!fullOutputFileHashesVerified) {
    limitations.push("Full stdout/stderr output bytes were not rehashed from per-sample output files.");
  }
  if (!cacheArtifactSnapshots.verified) {
    limitations.push("Cache artifact before/after mutation proof is missing or incomplete.");
  }
  if (!runProvenance.verified) {
    limitations.push("Machine-readable git/env/no-build provenance is missing or incomplete.");
  }
  if (localBuildReceipt.verified) {
    limitations.push(`Current-source release speed evidence is limited to ${benchmarkSurface.evidence_scope} for the receipt-verified local binary.`);
  } else {
    limitations.push("Current-source release speed remains unproven without a verified local-current-source build receipt.");
  }
  limitations.push(
    ...benchmarkSurface.limitations,
    "This manifest does not prove default-on readiness, app runtime, WebView, IPC, bundle, installer, watcher, or broad upstream superiority.",
  );

  const manifest = {
    schema: "dx.tauri.no_build_benchmark_manifest.v1",
    created_utc: new Date().toISOString(),
    run_id: path.basename(inputDir),
    lane: "tauri-dx-machine-cache",
    input_dir: inputDir,
    output_dir: outDir,
    benchmark_scope: "existing_binary_same_machine_no_build",
    benchmark_surface: benchmarkSurface.id,
    benchmark_command: benchmarkSurface.command,
    benchmark_command_args: benchmarkSurface.command_args,
    cache_boundary: benchmarkSurface.cache_boundary,
    no_build: true,
    existing_binaries_only: true,
    release_build_run: false,
    current_source_release_measured: claimGates.current_source_release_speed_claim_allowed,
    upstream_baseline_built_from_source: false,
    source_meta: meta,
    required_sample_columns: requiredSampleColumns,
    available_sample_columns: sampleColumns,
    required_summary_columns: requiredSummaryColumns,
    available_summary_columns: summaryColumns,
    required_binary_columns: requiredBinaryColumns,
    available_binary_columns: binaryColumns,
    sample_artifacts: sampleArtifacts,
    cases: caseSummaries,
    binaries,
    fixture: {
      path: fixturePath,
      cache_artifact_count: cacheArtifacts.length,
    },
    cache_artifacts: cacheArtifacts,
    cache_artifact_snapshots: {
      before_file: cacheArtifactSnapshots.before ? cacheArtifactSnapshots.before.file_record : null,
      after_file: cacheArtifactSnapshots.after ? cacheArtifactSnapshots.after.file_record : null,
      mutation_during_run_verified: cacheArtifactSnapshots.verified,
      stable_count: cacheArtifactSnapshots.stable_count,
      changed: cacheArtifactSnapshots.changed,
    },
    process_sweeps: processSweepJson,
    run_provenance: runProvenance,
    local_current_source_build_receipt: localBuildReceipt,
    output_equivalence: {
      basis: outputEquivalenceBasis,
      verified: outputEquivalenceVerified,
      first_line_equivalent: firstLineOutputEquivalent,
      full_hash_equivalent: hasFullOutputHashColumns ? fullHashOutputEquivalent : null,
      full_stdout_stderr_hashes_available: hasFullOutputHashColumns,
      full_stdout_stderr_hashes_verified_from_files: fullOutputFileHashesVerified,
    },
    comparison: {
      same_binary: sameBinaryCacheOnOff,
      same_inputs: outputEquivalenceVerified,
      cache_toggle_only: sameBinaryCacheOnOff && outputEquivalenceVerified,
      cache_on_to_off_median_ratio_percent: cacheOnToOffMedianRatioPercent,
      local_current_source_to_official_release_median_ratio_percent: officialReleaseComparisonRatioPercent,
      official_release_winner: officialReleaseComparisonWinner,
      inspect_official_release_winner:
        benchmarkSurface.id === "inspect-wix-upgrade-code" ? officialReleaseComparisonWinner : null,
      claim_basis: "median_ms",
    },
    claim_gates: claimGates,
    allowed_claims: allowedClaims,
    blocked_claims: blockedClaims,
    future_evidence: {
      strength: evidenceStrength,
      full_output_file_hashes_verified: fullOutputFileHashesVerified,
      cache_artifact_mutation_during_run_verified: cacheArtifactSnapshots.verified,
      run_provenance_verified: runProvenance.verified,
      local_current_source_build_receipt_verified: localBuildReceipt.verified,
      process_sweeps_verified: preflightProcessClean && finalProcessClean,
      benchmark_integrity_verified: benchmarkIntegrityVerified,
    },
    limitations,
  };

  const manifestPath = path.join(outDir, "wave7-benchmark-manifest.json");
  const checkPath = path.join(outDir, "wave7-benchmark-manifest-check.json");
  const textCheckPath = path.join(outDir, "wave7-benchmark-manifest-check.txt");
  const reportPath = path.join(outDir, "REPORT.md");
  writeJson(manifestPath, manifest);
  const manifestHash = sha256(manifestPath);

  const status = failures.length > 0 ? "fail" : warnings.length > 0 ? "partial" : "pass";
  const binaryHashFailures = failures.filter((failure) => failure.startsWith("binary "));
  const check = {
    schema: "dx.tauri.no_build_benchmark_governance_check.v1",
    checked_utc: new Date().toISOString(),
    manifest_path: manifestPath,
    manifest_sha256: manifestHash,
    status,
    summary_recomputed_from_samples: summaryRecomputedFromSamples,
    artifact_hashes_verified: binaryHashFailures.length === 0,
    binary_hashes_verified: binaryHashFailures.length === 0,
    output_equivalence_verified: outputEquivalenceVerified,
    output_equivalence_basis: outputEquivalenceBasis,
    sample_invocations_verified: sampleInvocationsVerified,
    cache_artifacts_fingerprinted: cacheArtifacts.length > 0,
    cache_artifact_mutation_during_run_verified: cacheArtifactSnapshots.verified,
    full_stdout_stderr_hashes_available: hasFullOutputHashColumns,
    full_stdout_stderr_hashes_verified_from_files: fullOutputFileHashesVerified,
    run_provenance_verified: runProvenance.verified,
    local_current_source_build_receipt_verified: localBuildReceipt.verified,
    process_sweeps_verified: preflightProcessClean && finalProcessClean,
    benchmark_integrity_verified: benchmarkIntegrityVerified,
    evidence_strength: evidenceStrength,
    claim_gate_results: claimGates,
    allowed_claims: allowedClaims,
    blocked_claims: blockedClaims,
    failures,
    warnings,
  };
  writeJson(checkPath, check);
  fs.writeFileSync(
    textCheckPath,
    [
      `status=${status}`,
      `manifest=${manifestPath}`,
      `check=${checkPath}`,
      `failures=${failures.length}`,
      `warnings=${warnings.length}`,
      "",
      ...failures.map((failure) => `failure=${failure}`),
      ...warnings.map((warning) => `warning=${warning}`),
      "",
    ].join("\n"),
    "utf8",
  );
  writeReport(reportPath, check, manifest);
  if (status === "pass") {
    fs.writeFileSync(
      path.join(TEST_OUTPUT_ROOT, "latest-tauri-benchmark-manifest-dir.txt"),
      `${outDir}\n`,
      "utf8",
    );
  }

  console.log(`manifest=${manifestPath}`);
  console.log(`check=${checkPath}`);
  console.log(`status=${status}`);
  console.log(`failures=${failures.length}`);
  console.log(`warnings=${warnings.length}`);

  if (failures.length > 0) process.exitCode = 1;
}

try {
  main();
} catch (error) {
  console.error(error && error.stack ? error.stack : String(error));
  process.exit(1);
}
