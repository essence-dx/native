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
  getBenchmarkSurface,
  listBenchmarkSurfaceIds,
} = require("./dx-benchmark-command-surfaces.ts");

const TEST_OUTPUT_ROOT = path.resolve("G:\\Dx\\test-outputs");
const SCHEMA = "dx.tauri.current_source_comparison_plan.v1";
const DEFAULT_FIXTURE =
  "G:\\Dx\\test-outputs\\tauri-dx-cli-cache-benchmark-20260530-wave6u-prep1\\fixture-app";
const DEFAULT_WARMUPS = 5;
const DEFAULT_SAMPLES = 30;
const DEFAULT_TIMEOUT_MS = 120000;
const RUNNER_SCRIPT = path.join(".scripts", "ci", "run-dx-no-build-benchmark.ts");
const LOCAL_BUILD_RECEIPT_FILE = "local-current-source-build-receipt.json";

function parseArgs(argv) {
  const args = {
    out: defaultOutDir(),
    fixture: DEFAULT_FIXTURE,
    globalTauri: null,
    node: null,
    nodeTauriScript: null,
    officialRelease: null,
    localRelease: null,
    localReleaseBuildReceipt: null,
    warmups: DEFAULT_WARMUPS,
    samples: DEFAULT_SAMPLES,
    timeoutMs: DEFAULT_TIMEOUT_MS,
    benchmarkSurface: DEFAULT_BENCHMARK_SURFACE_ID,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--out") {
      args.out = argv[++index];
    } else if (arg === "--fixture") {
      args.fixture = argv[++index];
    } else if (arg === "--global-tauri") {
      args.globalTauri = argv[++index];
    } else if (arg === "--node") {
      args.node = argv[++index];
    } else if (arg === "--node-tauri-script") {
      args.nodeTauriScript = argv[++index];
    } else if (arg === "--official-release") {
      args.officialRelease = argv[++index];
    } else if (arg === "--local-release") {
      args.localRelease = argv[++index];
    } else if (arg === "--local-release-build-receipt") {
      args.localReleaseBuildReceipt = argv[++index];
    } else if (arg === "--warmups") {
      args.warmups = parsePositiveInteger(argv[++index], "warmups");
    } else if (arg === "--samples") {
      args.samples = parsePositiveInteger(argv[++index], "samples");
    } else if (arg === "--timeout-ms") {
      args.timeoutMs = parsePositiveInteger(argv[++index], "timeout-ms");
    } else if (arg === "--benchmark-surface") {
      args.benchmarkSurface = argv[++index];
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
  node .scripts/ci/prepare-dx-current-source-comparison.ts --out <output-dir> \\
    --fixture <fixture-app> \\
    --global-tauri <tauri.exe> \\
    --node <node.exe> --node-tauri-script <tauri.js> \\
    --official-release <official-cargo-tauri.exe> \\
    --local-release <local-current-source-cargo-tauri.exe> \\
     [--local-release-build-receipt <local-current-source-build-receipt.json>]
     [--benchmark-surface <id>]

Writes a no-build current-source comparison plan and binaries.csv under
G:\\Dx\\test-outputs. This script hashes existing binaries only; it never builds
or benchmarks.

Allowed surfaces:
  ${listBenchmarkSurfaceIds().join("\n  ")}`);
}

function defaultOutDir() {
  const stamp = new Date()
    .toISOString()
    .replace(/[-:]/g, "")
    .replace(/\..+$/, "")
    .replace("T", "-");
  return path.join(TEST_OUTPUT_ROOT, `tauri-current-source-comparison-${stamp}`);
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
  const rootWithSep = TEST_OUTPUT_ROOT.endsWith(path.sep) ? TEST_OUTPUT_ROOT : `${TEST_OUTPUT_ROOT}${path.sep}`;
  if (resolved === TEST_OUTPUT_ROOT) {
    throw new Error(`${label} must be a child directory under ${TEST_OUTPUT_ROOT}, not the root itself`);
  }
  if (!resolved.toLowerCase().startsWith(rootWithSep.toLowerCase())) {
    throw new Error(`${label} must stay under ${TEST_OUTPUT_ROOT}: ${resolved}`);
  }
  let current = TEST_OUTPUT_ROOT;
  const relativeParts = path.relative(TEST_OUTPUT_ROOT, resolved).split(path.sep).filter(Boolean);
  for (const part of relativeParts) {
    current = path.join(current, part);
    if (!fs.existsSync(current)) break;
    const stat = fs.lstatSync(current);
    if (stat.isSymbolicLink()) {
      throw new Error(`${label} must not pass through a symlink or junction: ${current}`);
    }
  }
  return resolved;
}

function resolveRequiredFile(candidate, label) {
  if (!candidate) throw new Error(`${label} is required`);
  const resolved = path.resolve(candidate);
  if (!fs.existsSync(resolved)) throw new Error(`${label} does not exist: ${resolved}`);
  const stat = fs.statSync(resolved);
  if (!stat.isFile()) throw new Error(`${label} must be a file: ${resolved}`);
  return resolved;
}

function resolveRequiredDirectory(candidate, label) {
  if (!candidate) throw new Error(`${label} is required`);
  const resolved = path.resolve(candidate);
  if (!fs.existsSync(resolved)) throw new Error(`${label} does not exist: ${resolved}`);
  const stat = fs.statSync(resolved);
  if (!stat.isDirectory()) throw new Error(`${label} must be a directory: ${resolved}`);
  return resolved;
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

function writeJson(file, value) {
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function copyOptionalLocalBuildReceipt(source, outDir) {
  if (!source) return null;
  const sourcePath = resolveRequiredFile(source, "--local-release-build-receipt");
  const targetPath = path.join(outDir, LOCAL_BUILD_RECEIPT_FILE);
  if (path.resolve(sourcePath) !== path.resolve(targetPath)) {
    fs.copyFileSync(sourcePath, targetPath);
  }
  const receipt = JSON.parse(fs.readFileSync(targetPath, "utf8"));
  if (receipt.build_log?.path) {
    const sourceBuildLog = resolveRequiredFile(receipt.build_log.path, "local release build receipt build_log.path");
    const targetBuildLog = path.join(outDir, path.basename(sourceBuildLog));
    if (path.resolve(sourceBuildLog) !== path.resolve(targetBuildLog)) {
      fs.copyFileSync(sourceBuildLog, targetBuildLog);
    }
    receipt.build_log = fileRecord(targetBuildLog);
    writeJson(targetPath, receipt);
  }
  return {
    source_path: sourcePath,
    path: targetPath,
    file: fileRecord(targetPath),
    build_log: receipt.build_log ?? null,
  };
}

function runGit(args) {
  const result = spawnSync("git", args, {
    cwd: path.resolve(__dirname, "..", ".."),
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.status !== 0) return null;
  return result.stdout.trim();
}

function gitMetadata() {
  const branch = runGit(["branch", "--show-current"]);
  const headSha = runGit(["rev-parse", "HEAD"]);
  const statusShort = runGit(["status", "--short"]);
  return {
    repo_root: path.resolve(__dirname, "..", ".."),
    branch,
    head_sha: headSha,
    dirty: statusShort !== "",
    status_short: statusShort ? statusShort.split(/\r?\n/) : [],
  };
}

function selectedEnvironment() {
  return {
    platform: process.platform,
    arch: process.arch,
    node: process.version,
    TAURI_DX_MACHINE_CACHE: process.env.TAURI_DX_MACHINE_CACHE ?? null,
  };
}

function binaryRow({ target, kind, invocationKind, cacheEnv, file, script = "", role, sourceKind }) {
  const binary = fileRecord(file);
  const scriptRecord = script ? fileRecord(script) : null;
  return {
    target,
    kind,
    cache_env: cacheEnv,
    path: binary.path,
    script,
    bytes: binary.bytes,
    sha256: binary.sha256,
    last_write_utc: binary.last_write_utc,
    script_bytes: scriptRecord ? scriptRecord.bytes : "",
    script_sha256: scriptRecord ? scriptRecord.sha256 : "",
    script_last_write_utc: scriptRecord ? scriptRecord.last_write_utc : "",
    role,
    source_kind: sourceKind,
    invocation_kind: invocationKind,
  };
}

function makeRows(args) {
  const globalTauri = resolveRequiredFile(args.globalTauri, "--global-tauri");
  const node = resolveRequiredFile(args.node, "--node");
  const nodeTauriScript = resolveRequiredFile(args.nodeTauriScript, "--node-tauri-script");
  const officialRelease = resolveRequiredFile(args.officialRelease, "--official-release");
  const localRelease = resolveRequiredFile(args.localRelease, "--local-release");

  return [
    binaryRow({
      target: "official-global-tauri",
      kind: "direct-cli",
      invocationKind: "direct-executable",
      cacheEnv: "off",
      file: globalTauri,
      role: "official_global_cli",
      sourceKind: "prebuilt_global_package",
    }),
    binaryRow({
      target: "official-node-tauri-js",
      kind: "node-wrapper",
      invocationKind: "node-script",
      cacheEnv: "off",
      file: node,
      script: nodeTauriScript,
      role: "official_node_cli_script",
      sourceKind: "prebuilt_global_package",
    }),
    binaryRow({
      target: "official-release-binary",
      kind: "direct-rust",
      invocationKind: "direct-executable",
      cacheEnv: "off",
      file: officialRelease,
      role: "official_release_baseline",
      sourceKind: "official_release_binary",
    }),
    binaryRow({
      target: "local-current-source",
      kind: "direct-rust",
      invocationKind: "direct-executable",
      cacheEnv: "off",
      file: localRelease,
      role: "local_current_source_cache_off",
      sourceKind: "local_current_source_release_binary",
    }),
    binaryRow({
      target: "local-current-source",
      kind: "direct-rust",
      invocationKind: "direct-executable",
      cacheEnv: "on",
      file: localRelease,
      role: "local_current_source_cache_on",
      sourceKind: "local_current_source_release_binary",
    }),
  ];
}

function writeReport(file, plan) {
  const lines = [
    "# DX Tauri Current-Source Comparison Prep",
    "",
    `Fixture: ${plan.fixture}`,
    `Case config: ${plan.case_config}`,
    `Cases: ${plan.cases.length}`,
    `Warmups per case: ${plan.warmups_per_case}`,
    `Samples per case: ${plan.samples_per_case}`,
    "",
    "## Boundary",
    "",
    "- Existing binaries only.",
    "- No build was run.",
    "- No benchmark was run.",
    "- No speed or upstream-superiority claim is allowed from this preparation receipt alone.",
    "",
    "## Next Command",
    "",
    "```powershell",
    plan.runner_command.map((part) => (part.includes(" ") ? `"${part}"` : part)).join(" "),
    "```",
    "",
  ];
  fs.writeFileSync(file, lines.join("\n"), "utf8");
}

function main() {
  const args = parseArgs(process.argv);
  const benchmarkSurface = getBenchmarkSurface(args.benchmarkSurface);
  const outDir = assertUnderTestOutputs(args.out, "out");
  const fixture = resolveRequiredDirectory(args.fixture, "--fixture");
  const rows = makeRows(args);
  fs.mkdirSync(outDir, { recursive: true });
  const localBuildReceipt = copyOptionalLocalBuildReceipt(args.localReleaseBuildReceipt, outDir);

  const caseConfig = path.join(outDir, "binaries.csv");
  const planPath = path.join(outDir, "stage4-config-receipt.json");
  const reportPath = path.join(outDir, "REPORT.md");
  const runnerOutput = path.join(outDir, "benchmark-output");
  const runnerCommand = [
    "node",
    RUNNER_SCRIPT,
    "--fixture",
    fixture,
    "--case-config",
    caseConfig,
    "--out",
    runnerOutput,
    "--warmups",
    String(args.warmups),
    "--samples",
    String(args.samples),
    "--timeout-ms",
    String(args.timeoutMs),
    "--benchmark-surface",
    benchmarkSurface.id,
  ];
  if (localBuildReceipt) {
    runnerCommand.push("--local-release-build-receipt", localBuildReceipt.path);
  }
  runnerCommand.push("--check");
  const localOff = rows.find((row) => row.target === "local-current-source" && row.cache_env === "off");
  const localOn = rows.find((row) => row.target === "local-current-source" && row.cache_env === "on");
  const localSameBinary = Boolean(
    localOff &&
      localOn &&
      localOff.path === localOn.path &&
      localOff.bytes === localOn.bytes &&
      localOff.sha256 === localOn.sha256,
  );

  const headers = [
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
  ];
  writeCsv(caseConfig, headers, rows);

  const plan = {
    schema: SCHEMA,
    created_utc: new Date().toISOString(),
    stage: "current_source_comparison",
    mode: "config_only_no_build_no_benchmark",
    status: "pass",
    no_build: true,
    no_benchmark_run: true,
    existing_binaries_only: true,
    build_run: false,
    release_build_run: false,
    cargo_test_run: false,
    install_run: false,
    heavy_benchmark_run: false,
    benchmark_run: false,
    current_source_release_measured: false,
    benchmark_surface: benchmarkSurface.id,
    benchmark_command: benchmarkSurface.command,
    benchmark_command_args: benchmarkSurface.command_args,
    cache_boundary: benchmarkSurface.cache_boundary,
    fixture,
    output_dir: outDir,
    out_dir: outDir,
    case_config: caseConfig,
    case_config_csv: caseConfig,
    report: reportPath,
    report_md: reportPath,
    runner_script: RUNNER_SCRIPT,
    runner_command: runnerCommand,
    warmups_per_case: args.warmups,
    samples_per_case: args.samples,
    timeout_ms: args.timeoutMs,
    source_paths: {
      global_tauri: rows.find((row) => row.target === "official-global-tauri")?.path ?? null,
      node: rows.find((row) => row.target === "official-node-tauri-js")?.path ?? null,
      node_tauri_script: rows.find((row) => row.target === "official-node-tauri-js")?.script ?? null,
      official_release: rows.find((row) => row.target === "official-release-binary")?.path ?? null,
      local_release: localOff?.path ?? null,
      local_release_build_receipt: localBuildReceipt?.path ?? null,
    },
    local_current_source_build_receipt: localBuildReceipt,
    cases: rows,
    binary_identity_checks: {
      local_current_source_cache_on_off_same_binary: localSameBinary,
    },
    git: gitMetadata(),
    env: selectedEnvironment(),
    claim_gates: {
      current_source_release_speed_claim_allowed: false,
      faster_than_upstream_claim_allowed: false,
      default_on_readiness_claim_allowed: false,
      full_cli_speed_claim_allowed: false,
      app_runtime_webview_ipc_build_bundle_claim_allowed: false,
    },
    allowed_claims: [],
    blocked_claims: [
      {
        claim_id: "current_source_speed",
        claim_eligible: false,
        status: "blocked_unproven",
        reason: "This receipt prepares binary identities only and does not run the governed timing comparison.",
      },
      {
        claim_id: "upstream_superiority",
        claim_eligible: false,
        status: "blocked_unproven",
        reason: "No current-source timing evidence has been collected by this preparation script.",
      },
    ],
    failures: [],
    warnings: [],
    limitations: [
      "This receipt prepares a no-build comparison case config only.",
      "It does not prove speed, current-source release performance, default-on readiness, app runtime, WebView, IPC, bundle, installer, watcher, or upstream superiority.",
    ],
  };
  writeJson(planPath, plan);
  writeReport(reportPath, plan);
  fs.writeFileSync(path.join(TEST_OUTPUT_ROOT, "latest-tauri-current-source-comparison-plan-dir.txt"), `${outDir}\n`, "utf8");

  console.log(`comparison_plan=${planPath}`);
  console.log(`case_config=${caseConfig}`);
  console.log(`report=${reportPath}`);
  console.log("build_run=false");
  console.log("benchmark_run=false");
}

try {
  main();
} catch (error) {
  console.error(error && error.stack ? error.stack : String(error));
  process.exit(1);
}
