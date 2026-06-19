#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const REPO_ROOT = path.resolve(__dirname, "..", "..");
const TEST_OUTPUT_ROOT = path.resolve("G:\\Dx\\test-outputs");
const RECEIPT_SCHEMA = "dx.tauri.local_release_build_receipt.v1";
const EXPECTED_COMMAND =
  "cargo build -p tauri-cli --features dx-machine-cache-mmap --release --bin cargo-tauri --color never";
const EXPECTED_ARGV = [
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

function parseArgs(argv) {
  const args = {
    out: null,
    binary: path.join(REPO_ROOT, "target", "release", process.platform === "win32" ? "cargo-tauri.exe" : "cargo-tauri"),
    buildLog: null,
  };
  for (let index = 2; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--out") {
      args.out = argv[++index];
    } else if (arg === "--binary") {
      args.binary = argv[++index];
    } else if (arg === "--build-log") {
      args.buildLog = argv[++index];
    } else if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  if (!args.out) throw new Error("--out is required");
  if (!args.buildLog) throw new Error("--build-log is required");
  return args;
}

function printHelp() {
  console.log(`Usage:
  node .scripts/ci/write-dx-local-release-build-receipt.ts --out <output-dir> \\
    --build-log <cargo-build-log> [--binary <target/release/cargo-tauri.exe>]`);
}

function assertUnderTestOutputs(candidate, label) {
  const resolved = path.resolve(candidate);
  const rootWithSep = TEST_OUTPUT_ROOT.endsWith(path.sep) ? TEST_OUTPUT_ROOT : `${TEST_OUTPUT_ROOT}${path.sep}`;
  if (resolved === TEST_OUTPUT_ROOT || !resolved.toLowerCase().startsWith(rootWithSep.toLowerCase())) {
    throw new Error(`${label} must stay under ${TEST_OUTPUT_ROOT}: ${resolved}`);
  }
  return resolved;
}

function requireFile(candidate, label) {
  const resolved = path.resolve(candidate);
  if (!fs.existsSync(resolved)) throw new Error(`${label} does not exist: ${resolved}`);
  if (!fs.statSync(resolved).isFile()) throw new Error(`${label} must be a file: ${resolved}`);
  return resolved;
}

function sha256(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function fileRecord(file) {
  const stat = fs.statSync(file);
  return {
    path: file,
    bytes: stat.size,
    sha256: sha256(file),
    last_write_utc: stat.mtime.toISOString(),
  };
}

function git(args) {
  const result = spawnSync("git", args, {
    cwd: REPO_ROOT,
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.status !== 0) {
    throw new Error(`git ${args.join(" ")} failed: ${result.stderr || result.stdout}`);
  }
  return result.stdout.trim();
}

function gitState() {
  const status = git(["status", "--short"]);
  return {
    branch: git(["branch", "--show-current"]),
    head_sha: git(["rev-parse", "HEAD"]),
    dirty: status !== "",
    status_short: status ? status.split(/\r?\n/) : [],
  };
}

function assertCleanGit(state) {
  if (state.dirty || state.status_short.length > 0) {
    throw new Error("local release build receipt requires a clean git tree");
  }
}

function assertBuildLogLooksValid(buildLog) {
  const text = fs.readFileSync(buildLog, "utf8");
  const hasReleaseFinish = text.includes("Finished `release` profile");
  const hasBuildTargetEvidence = text.includes("Compiling tauri-cli") || text.includes(EXPECTED_COMMAND);
  const hasFailureOutput = /(^|\n)\s*error:/i.test(text) || text.includes("could not compile");
  if (!hasReleaseFinish || !hasBuildTargetEvidence || hasFailureOutput) {
    throw new Error(`build log does not look like a completed release build: ${buildLog}`);
  }
}

function main() {
  const args = parseArgs(process.argv);
  const outDir = assertUnderTestOutputs(args.out, "--out");
  const binary = requireFile(args.binary, "--binary");
  const buildLog = requireFile(args.buildLog, "--build-log");
  assertBuildLogLooksValid(buildLog);

  const gitMetadata = gitState();
  assertCleanGit(gitMetadata);

  fs.mkdirSync(outDir, { recursive: true });
  const receiptPath = path.join(outDir, "local-current-source-build-receipt.json");
  const receipt = {
    schema: RECEIPT_SCHEMA,
    target: "local-current-source",
    release_build_run: true,
    command: EXPECTED_COMMAND,
    argv: EXPECTED_ARGV,
    cwd: REPO_ROOT,
    repo_root: REPO_ROOT,
    profile: "release",
    package: "tauri-cli",
    binary_name: "cargo-tauri",
    features: ["dx-machine-cache-mmap"],
    git: gitMetadata,
    binary: fileRecord(binary),
    build_log: fileRecord(buildLog),
  };
  fs.writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(`local_release_build_receipt=${receiptPath}`);
}

try {
  main();
} catch (error) {
  console.error(error && error.stack ? error.stack : String(error));
  process.exit(1);
}
