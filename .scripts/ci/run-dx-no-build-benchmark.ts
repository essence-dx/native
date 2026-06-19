#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
require.extensions[".ts"] ||= require.extensions[".js"];
const {
  DEFAULT_BENCHMARK_SURFACE_ID,
  applyBenchmarkCasePolicyDefaults,
  getBenchmarkSurface,
  listBenchmarkSurfaceIds,
  validateBenchmarkCase,
} = require("./dx-benchmark-command-surfaces.ts");

const TEST_OUTPUT_ROOT = path.resolve("G:\\Dx\\test-outputs");
const DEFAULT_EXISTING_BENCHMARK_INPUT_DIR =
  "G:\\Dx\\test-outputs\\tauri-dx-machine-cache-20260530-wave7-inspect1";
const DEFAULT_FIXTURE =
  "G:\\Dx\\test-outputs\\tauri-dx-cli-cache-benchmark-20260530-wave6u-prep1\\fixture-app";
const DEFAULT_CASE_CONFIG = path.join(DEFAULT_EXISTING_BENCHMARK_INPUT_DIR, "binaries.csv");
const META_SCHEMA = "dx.tauri.wave7.command_benchmark.v2";
const PROVENANCE_SCHEMA = "dx.tauri.no_build_benchmark_run_provenance.v1";
const PROCESS_SWEEP_SCHEMA = "dx.tauri.process_sweep.v1";
const PROCESS_SWEEP_CLEAN_MARKER =
  "no-matching-heavy-bun-build-processes; no-matching-heavy-build-processes";
const LOCAL_BUILD_RECEIPT_FILE = "local-current-source-build-receipt.json";

function parseArgs(argv) {
  const args = {
    out: defaultOutDir(),
    fixture: DEFAULT_FIXTURE,
    caseConfig: DEFAULT_CASE_CONFIG,
    samples: 30,
    warmups: 5,
    planOnly: false,
    check: false,
    timeoutMs: 120000,
    localReleaseBuildReceipt: null,
    benchmarkSurface: DEFAULT_BENCHMARK_SURFACE_ID,
  };
  for (let index = 2; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--out") {
      args.out = argv[++index];
    } else if (arg === "--fixture") {
      args.fixture = argv[++index];
    } else if (arg === "--case-config") {
      args.caseConfig = argv[++index];
    } else if (arg === "--samples") {
      args.samples = parsePositiveInteger(argv[++index], "samples");
    } else if (arg === "--warmups") {
      args.warmups = parsePositiveInteger(argv[++index], "warmups");
    } else if (arg === "--timeout-ms") {
      args.timeoutMs = parsePositiveInteger(argv[++index], "timeout-ms");
    } else if (arg === "--local-release-build-receipt") {
      args.localReleaseBuildReceipt = argv[++index];
    } else if (arg === "--benchmark-surface") {
      args.benchmarkSurface = argv[++index];
    } else if (arg === "--plan-only") {
      args.planOnly = true;
    } else if (arg === "--check") {
      args.check = true;
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
  node .scripts/ci/run-dx-no-build-benchmark.ts --out <output-dir> [--benchmark-surface <id>] [--plan-only] [--check]

Runs an allowlisted existing-binary Tauri CLI benchmark without building,
emits governed no-build benchmark artifacts, and keeps outputs under
G:\\Dx\\test-outputs.

Defaults:
  --fixture ${DEFAULT_FIXTURE}
  --case-config ${DEFAULT_CASE_CONFIG}
  --warmups 5 --samples 30
  --local-release-build-receipt <local-current-source-build-receipt.json>
  --benchmark-surface ${DEFAULT_BENCHMARK_SURFACE_ID}

Allowed surfaces:
  ${listBenchmarkSurfaceIds().join("\n  ")}`);
}

function defaultOutDir() {
  const stamp = new Date()
    .toISOString()
    .replace(/[-:]/g, "")
    .replace(/\..+$/, "")
    .replace("T", "-");
  return path.join(TEST_OUTPUT_ROOT, `tauri-dx-machine-cache-wave12-no-build-${stamp}`);
}

function parsePositiveInteger(value, label) {
  if (!/^\d+$/.test(String(value))) throw new Error(`${label} must be a positive integer`);
  const parsed = Number.parseInt(value, 10);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive integer`);
  }
  return parsed;
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

function csvEscape(value) {
  const text = String(value ?? "");
  if (/[",\r\n]/.test(text)) {
    return `"${text.replace(/"/g, '""')}"`;
  }
  return text;
}

function writeCsv(file, headers, rows) {
  const lines = [
    headers.map(csvEscape).join(","),
    ...rows.map((row) => headers.map((header) => csvEscape(row[header])).join(",")),
  ];
  fs.writeFileSync(file, `${lines.join("\n")}\n`, "utf8");
}

function sha256Buffer(buffer) {
  return crypto.createHash("sha256").update(buffer).digest("hex");
}

function sha256File(file) {
  return sha256Buffer(fs.readFileSync(file));
}

function fileRecord(file) {
  const stat = fs.statSync(file);
  return {
    bytes: stat.size,
    sha256: sha256File(file),
    last_write_utc: stat.mtime.toISOString(),
  };
}

function copyOptionalLocalBuildReceipt(source, outDir) {
  if (!source) return null;
  const sourcePath = path.resolve(source);
  if (!fs.existsSync(sourcePath)) throw new Error(`local release build receipt does not exist: ${sourcePath}`);
  if (!fs.statSync(sourcePath).isFile()) {
    throw new Error(`local release build receipt must be a file: ${sourcePath}`);
  }
  const targetPath = path.join(outDir, LOCAL_BUILD_RECEIPT_FILE);
  if (sourcePath !== path.resolve(targetPath)) {
    fs.copyFileSync(sourcePath, targetPath);
  }
  const receipt = JSON.parse(fs.readFileSync(targetPath, "utf8"));
  if (receipt.build_log?.path) {
    const sourceBuildLog = path.resolve(receipt.build_log.path);
    if (!fs.existsSync(sourceBuildLog)) throw new Error(`local release build log does not exist: ${sourceBuildLog}`);
    if (!fs.statSync(sourceBuildLog).isFile()) {
      throw new Error(`local release build log must be a file: ${sourceBuildLog}`);
    }
    const targetBuildLog = path.join(outDir, path.basename(sourceBuildLog));
    if (sourceBuildLog !== path.resolve(targetBuildLog)) {
      fs.copyFileSync(sourceBuildLog, targetBuildLog);
    }
    receipt.build_log = {
      path: targetBuildLog,
      ...fileRecord(targetBuildLog),
    };
    writeJson(targetPath, receipt);
  }
  return {
    source_path: sourcePath,
    path: targetPath,
    file: fileRecord(targetPath),
    build_log: receipt.build_log ?? null,
  };
}

function optionalFileRecord(file) {
  return file ? fileRecord(file) : null;
}

function validateRecordedFileIdentity(row, file, label) {
  const record = fileRecord(file);
  if (row.bytes !== undefined && row.bytes !== "" && Number.parseInt(row.bytes, 10) !== record.bytes) {
    throw new Error(`${label} byte length mismatch for ${row.target}/${row.cache_env}: expected ${row.bytes}, got ${record.bytes}`);
  }
  if (row.sha256 !== undefined && row.sha256 !== "" && row.sha256 !== record.sha256) {
    throw new Error(`${label} sha256 mismatch for ${row.target}/${row.cache_env}: expected ${row.sha256}, got ${record.sha256}`);
  }
  return record;
}

function validateRecordedScriptIdentity(row, file) {
  const record = fileRecord(file);
  if (row.script_bytes !== undefined && row.script_bytes !== "" && Number.parseInt(row.script_bytes, 10) !== record.bytes) {
    throw new Error(
      `case script byte length mismatch for ${row.target}/${row.cache_env}: expected ${row.script_bytes}, got ${record.bytes}`,
    );
  }
  if (row.script_sha256 !== undefined && row.script_sha256 !== "" && row.script_sha256 !== record.sha256) {
    throw new Error(
      `case script sha256 mismatch for ${row.target}/${row.cache_env}: expected ${row.script_sha256}, got ${record.sha256}`,
    );
  }
  return record;
}

function firstMeaningfulLine(buffer) {
  const text = buffer.toString("utf8");
  return text.split(/\r?\n/).find((line) => line.trim().length > 0) ?? "";
}

function safeName(value) {
  return String(value).replace(/[^A-Za-z0-9_.-]/g, "_");
}

function commandForCase(row, surface) {
  if (row.script) {
    return {
      command: row.path,
      args: [row.script, ...surface.command_args],
    };
  }
  return {
    command: row.path,
    args: surface.command_args,
  };
}

function validateCacheEnv(value) {
  if (!["on", "off", "unset"].includes(value)) {
    throw new Error(`cache_env must be one of on, off, unset: ${value}`);
  }
}

function cacheEnvValue(row) {
  validateCacheEnv(row.cache_env);
  if (row.cache_env === "on") return "1";
  if (row.cache_env === "off") return "0";
  return "";
}

function cacheWriteEnvValue(row) {
  validateCacheEnv(row.cache_env);
  return "0";
}

function envForCase(row) {
  const env = { ...process.env };
  const value = cacheEnvValue(row);
  const writeValue = cacheWriteEnvValue(row);
  if (value === "1") {
    env.TAURI_DX_MACHINE_CACHE = "1";
  } else if (value === "0") {
    env.TAURI_DX_MACHINE_CACHE = "0";
  } else {
    delete env.TAURI_DX_MACHINE_CACHE;
  }
  if (writeValue === "0") {
    env.TAURI_DX_MACHINE_CACHE_WRITE = "0";
  } else {
    delete env.TAURI_DX_MACHINE_CACHE_WRITE;
  }
  return env;
}

function runOne(row, fixture, timeoutMs, surface) {
  const command = commandForCase(row, surface);
  const started = process.hrtime.bigint();
  const result = spawnSync(command.command, command.args, {
    cwd: fixture,
    env: envForCase(row),
    windowsHide: true,
    shell: false,
    encoding: "buffer",
    timeout: timeoutMs,
  });
  const ended = process.hrtime.bigint();
  const elapsedMs = Number(ended - started) / 1_000_000;
  return {
    elapsed_ms: elapsedMs.toFixed(3),
    exit_code: result.status ?? (result.error ? 1 : 0),
    signal: result.signal ?? "",
    timed_out: result.error && result.error.code === "ETIMEDOUT" ? "true" : "false",
    stdout: result.stdout ?? Buffer.alloc(0),
    stderr: result.stderr ?? Buffer.alloc(0),
    error: result.error ? String(result.error.message || result.error) : "",
    command: command.command,
    args: command.args.join(" "),
    argv_json: JSON.stringify(command.args),
    cwd: fixture,
    cache_env_value: cacheEnvValue(row) || "<unset>",
    cache_write_env_value: cacheWriteEnvValue(row) || "<unset>",
  };
}

function assertRunSucceeded(result, label) {
  if (result.error || result.timed_out === "true" || result.exit_code !== 0) {
    throw new Error(
      `${label} failed: exit=${result.exit_code} signal=${result.signal || "<none>"} timeout=${result.timed_out} error=${result.error || "<none>"}`,
    );
  }
}

function snapshotMachineArtifacts(fixture) {
  const rows = [];
  const stack = [fixture];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const entryPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(entryPath);
      } else if (entry.isFile() && entry.name.endsWith(".machine")) {
        const record = fileRecord(entryPath);
        rows.push({
          relative_path: path.relative(fixture, entryPath).replace(/\\/g, "/"),
          bytes: record.bytes,
          sha256: record.sha256,
          last_write_utc: record.last_write_utc,
        });
      }
    }
  }
  rows.sort((left, right) => left.relative_path.localeCompare(right.relative_path));
  return rows;
}

function machineArtifactChangeList(before, after) {
  const beforeRows = new Map(before.map((row) => [row.relative_path, row]));
  const afterRows = new Map(after.map((row) => [row.relative_path, row]));
  const changes = [];
  for (const [key, beforeRow] of beforeRows) {
    const afterRow = afterRows.get(key);
    if (!afterRow) {
      changes.push(`${key}: missing-after`);
      continue;
    }
    for (const column of ["bytes", "sha256", "last_write_utc"]) {
      if (beforeRow[column] !== afterRow[column]) changes.push(`${key}: ${column} changed`);
    }
  }
  for (const key of afterRows.keys()) {
    if (!beforeRows.has(key)) changes.push(`${key}: missing-before`);
  }
  return changes;
}

function assertStableMachineArtifacts(before, after, phase) {
  if (before.length === 0) {
    throw new Error("pre-generated .machine cache artifacts are required before benchmark warmups");
  }
  const changes = machineArtifactChangeList(before, after);
  if (changes.length > 0) {
    throw new Error(`.machine cache artifacts changed during ${phase}: ${changes.join("; ")}`);
  }
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

function median(values) {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = sorted.length / 2;
  if (sorted.length % 2 === 1) return thousandthsToNumber(sorted[Math.floor(middle)]);
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

function summaryRows(samples) {
  const groups = new Map();
  for (const sample of samples) {
    const key = `${sample.target}\u0000${sample.cache_env}\u0000${sample.target_kind}`;
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(sample);
  }
  return [...groups.values()].map((group) => {
    const first = group[0];
    const values = group.map((sample) => parseMsToThousandths(sample.elapsed_ms));
    return {
      target: first.target,
      cache_env: first.cache_env,
      target_kind: first.target_kind,
      runs: group.length,
      median_ms: median(values),
      p95_ms: p95(values),
      min_ms: thousandthsToNumber(Math.min(...values)),
      max_ms: thousandthsToNumber(Math.max(...values)),
      mean_ms: mean(values),
    };
  });
}

function writeSummaryTable(file, summaries) {
  const lines = [
    "target | cache_env | target_kind | runs | median_ms | p95_ms | min_ms | max_ms | mean_ms",
    "--- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---:",
    ...summaries.map(
      (row) =>
        `${row.target} | ${row.cache_env} | ${row.target_kind} | ${row.runs} | ${row.median_ms} | ${row.p95_ms} | ${row.min_ms} | ${row.max_ms} | ${row.mean_ms}`,
    ),
  ];
  fs.writeFileSync(file, `${lines.join("\n")}\n`, "utf8");
}

function loadCases(caseConfig) {
  const rows = parseCsv(readText(caseConfig));
  for (const row of rows) {
    for (const column of ["target", "kind", "cache_env", "path"]) {
      if (!row[column]) throw new Error(`case config row missing ${column}`);
    }
    validateCacheEnv(row.cache_env);
    row.path = path.resolve(row.path);
    if (!fs.existsSync(row.path)) throw new Error(`case binary missing: ${row.path}`);
    validateRecordedFileIdentity(row, row.path, "case binary");
    if (row.script) {
      row.script = path.resolve(row.script);
      if (!fs.existsSync(row.script)) throw new Error(`case script missing: ${row.script}`);
      validateRecordedScriptIdentity(row, row.script);
    }
    applyBenchmarkCasePolicyDefaults(row);
    validateBenchmarkCase(row);
  }
  return rows;
}

function binaryRows(cases) {
  return cases.map((row) => {
    const record = fileRecord(row.path);
    const scriptRecord = optionalFileRecord(row.script);
    return {
      target: row.target,
      kind: row.kind,
      cache_env: row.cache_env,
      path: row.path,
      script: row.script ?? "",
      bytes: record.bytes,
      sha256: record.sha256,
      last_write_utc: record.last_write_utc,
      script_bytes: scriptRecord ? scriptRecord.bytes : "",
      script_sha256: scriptRecord ? scriptRecord.sha256 : "",
      script_last_write_utc: scriptRecord ? scriptRecord.last_write_utc : "",
      role: row.role,
      source_kind: row.source_kind,
      invocation_kind: row.invocation_kind,
    };
  });
}

function gitInfo(repoRoot) {
  const run = (args) =>
    spawnSync("git", args, {
      cwd: repoRoot,
      encoding: "utf8",
      windowsHide: true,
    });
  const branch = run(["branch", "--show-current"]).stdout.trim();
  const head = run(["rev-parse", "HEAD"]).stdout.trim();
  const status = run(["status", "--short"]).stdout.split(/\r?\n/).filter(Boolean);
  return {
    repo_root: repoRoot,
    branch,
    head_sha: head,
    dirty: status.length > 0,
    status_short: status,
  };
}

function assertCleanGitForBenchmark(repoRoot) {
  const info = gitInfo(repoRoot);
  if (!/^[a-f0-9]{40}$/i.test(info.head_sha)) {
    throw new Error("benchmark run requires a valid git HEAD receipt before timing");
  }
  if (info.dirty) {
    throw new Error(`benchmark run requires a clean git worktree before timing: ${info.status_short.join("; ")}`);
  }
  return info;
}

function psSingleQuoted(value) {
  return `'${String(value).replace(/'/g, "''")}'`;
}

function uniqueRoots(values) {
  const roots = new Set();
  for (const value of values) {
    if (!value) continue;
    const resolved = path.resolve(value);
    roots.add(fs.existsSync(resolved) && fs.statSync(resolved).isFile() ? path.dirname(resolved) : resolved);
  }
  return [...roots].sort((left, right) => left.localeCompare(right));
}

function processSweepMatches(output) {
  const text = String(output || "").trim();
  if (!text || text.includes(PROCESS_SWEEP_CLEAN_MARKER)) return [];
  const parsed = JSON.parse(text);
  return Array.isArray(parsed) ? parsed : [parsed];
}

function writeProcessSweep(file, jsonFile, roots, phase) {
  const normalizedRoots = uniqueRoots(roots);
  const quotedRoots = normalizedRoots.map(psSingleQuoted).join(",");
  const script = [
    `$selfPid = ${process.pid}`,
    `$roots = @(${quotedRoots})`,
    "$matches = Get-CimInstance Win32_Process | Where-Object {",
    "  if ($_.ProcessId -eq $selfPid) { return $false }",
    "  $nameMatches = $_.Name -match '^(cargo|rustc|rustup|node|bun|tauri|cargo-tauri)(\\.exe)?$'",
    "  if (-not $nameMatches) { return $false }",
    "  $commandLine = [string]$_.CommandLine",
    "  $executablePath = [string]$_.ExecutablePath",
    "  foreach ($root in $roots) {",
    "    if ($commandLine.Contains($root) -or $executablePath.StartsWith($root, [System.StringComparison]::OrdinalIgnoreCase)) { return $true }",
    "  }",
    "  return $false",
    "} | Select-Object ProcessId,Name,ExecutablePath,CommandLine",
    `if ($matches) { $matches | ConvertTo-Json -Depth 4 } else { '${PROCESS_SWEEP_CLEAN_MARKER}' }`,
  ].join("\n");
  const shell = process.env.ComSpec ? "powershell.exe" : "pwsh";
  const result = spawnSync(shell, ["-NoProfile", "-Command", script], {
    encoding: "utf8",
    windowsHide: true,
  });
  let matches = [];
  if (result.status === 0) {
    try {
      matches = processSweepMatches(result.stdout);
    } catch (error) {
      matches = [
        {
          ProcessId: null,
          Name: "process-sweep-parse-error",
          ExecutablePath: "",
          CommandLine: error.message,
        },
      ];
    }
  }
  fs.writeFileSync(
    file,
    result.status === 0 ? result.stdout : `process-sweep-failed\n${result.stderr || result.stdout}`,
    "utf8",
  );
  const textRecord = fileRecord(file);
  const receipt = {
    schema: PROCESS_SWEEP_SCHEMA,
    phase,
    captured_utc: new Date().toISOString(),
    self_pid: process.pid,
    roots: normalizedRoots,
    command_names: ["cargo", "rustc", "rustup", "node", "bun", "tauri", "cargo-tauri"],
    clean: result.status === 0 && matches.length === 0,
    exit_code: result.status,
    text_log: path.basename(file),
    text_log_sha256: textRecord.sha256,
    matched_processes: matches.map((entry) => ({
      pid: entry.ProcessId ?? entry.ProcessID ?? entry.pid ?? null,
      name: entry.Name ?? entry.name ?? "",
      executable_path: entry.ExecutablePath ?? entry.executable_path ?? "",
      command_line: entry.CommandLine ?? entry.command_line ?? "",
    })),
  };
  writeJson(jsonFile, receipt);
  return {
    clean: receipt.clean,
    output: result.status === 0 ? result.stdout : "",
    json: receipt,
  };
}

function writeJson(file, value) {
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function writeReport(file, details) {
  const lines = [
    "# DX Tauri No-Build Benchmark Evidence Runner",
    "",
    `Plan only: ${details.plan_only}`,
    `Benchmark surface: ${details.benchmark_surface}`,
    `Command: ${details.command}`,
    `Cache boundary: ${details.cache_boundary}`,
    `Fixture: ${details.fixture}`,
    `Cases: ${details.case_count}`,
    `Warmups per case: ${details.warmups}`,
    `Samples per case: ${details.samples}`,
    "",
    "## Boundary",
    "",
    "- Existing binaries only.",
    "- Pre-generated .machine artifacts are required before warmups; generation time is not measured.",
    "- .machine artifacts must remain byte-for-byte stable through warmups and timed samples.",
    "- No source build, install, dev server, app runtime, WebView, IPC, bundle, installer, watcher, or upstream-superiority claim.",
    "- Feed non-plan runs through check-dx-benchmark-manifest.ts before using any timing claim.",
    "",
  ];
  fs.writeFileSync(file, lines.join("\n"), "utf8");
}

function runChecker(outDir, repoRoot) {
  const checker = path.join(repoRoot, ".scripts", "ci", "check-dx-benchmark-manifest.ts");
  const checkOut = `${outDir}-manifest-check`;
  const result = spawnSync(process.execPath, [checker, "--input", outDir, "--out", checkOut], {
    cwd: repoRoot,
    encoding: "utf8",
    windowsHide: true,
  });
  fs.writeFileSync(path.join(outDir, "checker-run.log"), `${result.stdout}${result.stderr}`, "utf8");
  return {
    check_out: checkOut,
    exit_code: result.status,
  };
}

function main() {
  const args = parseArgs(process.argv);
  const benchmarkSurface = getBenchmarkSurface(args.benchmarkSurface);
  const outDir = assertUnderTestOutputs(args.out, "out");
  const fixture = path.resolve(args.fixture);
  const caseConfig = path.resolve(args.caseConfig);
  const repoRoot = path.resolve(__dirname, "..", "..");
  fs.mkdirSync(outDir, { recursive: true });
  const localBuildReceipt = copyOptionalLocalBuildReceipt(args.localReleaseBuildReceipt, outDir);

  if (!fs.existsSync(fixture)) throw new Error(`fixture does not exist: ${fixture}`);
  if (!fs.existsSync(caseConfig)) throw new Error(`case config does not exist: ${caseConfig}`);
  const cases = loadCases(caseConfig);
  const binaries = binaryRows(cases);
  const processSweepRoots = [
    "G:\\Dx\\bun",
    repoRoot,
    fixture,
    caseConfig,
    args.localReleaseBuildReceipt,
    ...cases.map((row) => row.script || row.path),
  ];
  const details = {
    schema: "dx.tauri.no_build_future_evidence_runner_plan.v1",
    created_utc: new Date().toISOString(),
    plan_only: args.planOnly,
    out_dir: outDir,
    fixture,
    case_config: caseConfig,
    case_config_sha256: sha256File(caseConfig),
    case_count: cases.length,
    warmups: args.warmups,
    samples: args.samples,
    benchmark_surface: benchmarkSurface.id,
    benchmark_command_args: benchmarkSurface.command_args,
    command: benchmarkSurface.command,
    cache_boundary: benchmarkSurface.cache_boundary,
    pre_generated_machine_cache_required: true,
    machine_cache_generation_measured: false,
    machine_cache_command_writes_disabled: true,
    machine_cache_write_env_for_cache_on: "0",
    machine_cache_write_env_for_all_samples: "0",
    cache_artifact_snapshot_timing: "before_warmups_and_after_samples",
    local_current_source_build_receipt: localBuildReceipt,
    cases: binaries.map((row) => ({
      target: row.target,
      kind: row.kind,
      cache_env: row.cache_env,
      path: row.path,
      script: row.script,
      bytes: row.bytes,
      sha256: row.sha256,
      script_bytes: row.script_bytes,
      script_sha256: row.script_sha256,
      role: row.role,
      source_kind: row.source_kind,
      invocation_kind: row.invocation_kind,
    })),
  };
  writeJson(path.join(outDir, "wave12-run-plan.json"), details);
  writeReport(path.join(outDir, "REPORT.md"), details);

  if (args.planOnly) {
    console.log(`plan=${path.join(outDir, "wave12-run-plan.json")}`);
    console.log(`report=${path.join(outDir, "REPORT.md")}`);
    console.log("plan_only=true");
    return;
  }
  const runGitInfo = assertCleanGitForBenchmark(repoRoot);

  const outputDir = path.join(outDir, "outputs");
  fs.mkdirSync(outputDir, { recursive: true });
  writeCsv(
    path.join(outDir, "binaries.csv"),
    [
      "target",
      "kind",
      "cache_env",
      "path",
      "script",
      "bytes",
      "sha256",
      "last_write_utc",
      "script_bytes",
      "script_sha256",
      "script_last_write_utc",
      "role",
      "source_kind",
      "invocation_kind",
    ],
    binaries,
  );

  const preflightSweep = writeProcessSweep(
    path.join(outDir, "wave7-process-check-preflight.log"),
    path.join(outDir, "wave7-process-check-preflight.json"),
    processSweepRoots,
    "preflight",
  );
  if (!preflightSweep.clean) {
    throw new Error("process preflight found active cargo/node/bun/tauri work touching benchmark paths");
  }

  const cacheBefore = snapshotMachineArtifacts(fixture);
  writeCsv(
    path.join(outDir, "cache-artifacts-before.csv"),
    ["relative_path", "bytes", "sha256", "last_write_utc"],
    cacheBefore,
  );

  for (const row of cases) {
    for (let warmup = 0; warmup < args.warmups; warmup += 1) {
      assertRunSucceeded(
        runOne(row, fixture, args.timeoutMs, benchmarkSurface),
        `warmup ${row.target}/${row.cache_env}/${row.kind} #${warmup + 1}`,
      );
    }
  }

  assertStableMachineArtifacts(cacheBefore, snapshotMachineArtifacts(fixture), "warmups");

  const samples = [];
  for (const row of cases) {
    for (let iteration = 1; iteration <= args.samples; iteration += 1) {
      const result = runOne(row, fixture, args.timeoutMs, benchmarkSurface);
      const prefix = safeName(`${row.target}-${row.cache_env}-${row.kind}-${iteration}`);
      const stdoutPath = path.join(outputDir, `${prefix}.stdout.txt`);
      const stderrPath = path.join(outputDir, `${prefix}.stderr.txt`);
      fs.writeFileSync(stdoutPath, result.stdout);
      fs.writeFileSync(stderrPath, result.stderr);
      samples.push({
        target: row.target,
        target_kind: row.kind,
        cache_env: row.cache_env,
        tauri_dx_machine_cache_env: result.cache_env_value,
        tauri_dx_machine_cache_write_env: result.cache_write_env_value,
        phase: "sample",
        iteration,
        elapsed_ms: result.elapsed_ms,
        exit_code: result.exit_code,
        signal: result.signal,
        timed_out: result.timed_out,
        spawn_error: result.error,
        command: result.command,
        args: result.args,
        argv_json: result.argv_json,
        cwd: result.cwd,
        stdout_first: firstMeaningfulLine(result.stdout),
        stderr_first: firstMeaningfulLine(result.stderr),
        stdout_sha256: sha256File(stdoutPath),
        stderr_sha256: sha256File(stderrPath),
        stdout_bytes: result.stdout.length,
        stderr_bytes: result.stderr.length,
        stdout_path: path.relative(outDir, stdoutPath).replace(/\\/g, "/"),
        stderr_path: path.relative(outDir, stderrPath).replace(/\\/g, "/"),
      });
      assertRunSucceeded(
        result,
        `sample ${row.target}/${row.cache_env}/${row.kind} #${iteration}`,
      );
    }
  }

  const cacheAfter = snapshotMachineArtifacts(fixture);
  assertStableMachineArtifacts(cacheBefore, cacheAfter, "sample timing");
  writeCsv(
    path.join(outDir, "cache-artifacts-after.csv"),
    ["relative_path", "bytes", "sha256", "last_write_utc"],
    cacheAfter,
  );
  writeCsv(
    path.join(outDir, "samples.csv"),
    [
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
      "argv_json",
      "cwd",
      "stdout_first",
      "stderr_first",
      "stdout_sha256",
      "stderr_sha256",
      "stdout_bytes",
      "stderr_bytes",
      "stdout_path",
      "stderr_path",
    ],
    samples,
  );
  const summaries = summaryRows(samples);
  writeCsv(
    path.join(outDir, "summary.csv"),
    ["target", "cache_env", "target_kind", "runs", "median_ms", "p95_ms", "min_ms", "max_ms", "mean_ms"],
    summaries,
  );
  writeSummaryTable(path.join(outDir, "summary-table.txt"), summaries);
  const finalSweep = writeProcessSweep(
    path.join(outDir, "wave7-process-check-final.log"),
    path.join(outDir, "wave7-process-check-final.json"),
    processSweepRoots,
    "final",
  );
  writeJson(path.join(outDir, "wave7-meta.json"), {
    schema: META_SCHEMA,
    created_utc: new Date().toISOString(),
    no_build: true,
    benchmark_surface: benchmarkSurface.id,
    benchmark_command: benchmarkSurface.command,
    benchmark_command_args: benchmarkSurface.command_args,
    cache_boundary: benchmarkSurface.cache_boundary,
    fixture,
    warmups_per_case: args.warmups,
    samples_per_case: args.samples,
    samples_csv: path.join(outDir, "samples.csv"),
    summary_csv: path.join(outDir, "summary.csv"),
    binaries_csv: path.join(outDir, "binaries.csv"),
    machine_cache_write_env_for_all_samples: "0",
  });
  writeJson(path.join(outDir, "run-provenance.json"), {
    schema: PROVENANCE_SCHEMA,
    captured_utc: new Date().toISOString(),
    run_id: path.basename(outDir),
    benchmark_surface: benchmarkSurface.id,
    benchmark_command_args: benchmarkSurface.command_args,
    command: benchmarkSurface.command,
    cache_boundary: benchmarkSurface.cache_boundary,
    no_build: true,
    existing_binaries_only: true,
    build_run: false,
    cargo_test_run: false,
    install_run: false,
    heavy_benchmark_run: false,
    pre_generated_machine_cache_required: true,
    machine_cache_generation_measured: false,
    machine_cache_command_writes_disabled: true,
    machine_cache_write_env_for_cache_on: "0",
    machine_cache_write_env_for_all_samples: "0",
    cache_artifact_snapshot_timing: "before_warmups_and_after_samples",
    timeout_ms: args.timeoutMs,
    case_config: caseConfig,
    case_config_sha256: sha256File(caseConfig),
    local_current_source_build_receipt: localBuildReceipt,
    process_sweeps: {
      roots: processSweepRoots,
      preflight: {
        path: "wave7-process-check-preflight.log",
        json_path: "wave7-process-check-preflight.json",
        clean: preflightSweep.clean,
      },
      final: {
        path: "wave7-process-check-final.log",
        json_path: "wave7-process-check-final.json",
        clean: finalSweep.clean,
      },
    },
    git: runGitInfo,
    env: {
      platform: process.platform,
      arch: process.arch,
      node: process.version,
      tauri_dx_machine_cache: "varied_by_cache_env_column",
    },
  });

  const check = args.check ? runChecker(outDir, repoRoot) : null;
  console.log(`out=${outDir}`);
  console.log(`samples=${samples.length}`);
  if (check) console.log(`check_out=${check.check_out} check_exit=${check.exit_code}`);
  if (!finalSweep.clean) {
    console.error("process final sweep found active cargo/node/bun/tauri work touching benchmark paths");
    process.exitCode = 1;
  }
  if (check && check.exit_code !== 0) {
    process.exitCode = check.exit_code || 1;
  }
}

try {
  main();
} catch (error) {
  console.error(error && error.stack ? error.stack : String(error));
  process.exit(1);
}
