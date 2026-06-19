#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const REPO_ROOT = path.resolve(__dirname, "..", "..");

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

function assertBefore(source, first, second, label) {
  const firstIndex = source.indexOf(first);
  const secondIndex = source.indexOf(second);
  if (firstIndex === -1) {
    throw new Error(`${label} is missing: ${first}`);
  }
  if (secondIndex === -1) {
    throw new Error(`${label} is missing: ${second}`);
  }
  if (firstIndex > secondIndex) {
    throw new Error(`${label} must place ${first} before ${second}`);
  }
}

function readManifestString(source, key, label) {
  const escapedKey = key.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = source.match(new RegExp(`^${escapedKey}\\s*=\\s*"([^"]+)"`, "m"));
  if (!match) {
    throw new Error(`${label} missing manifest string: ${key}`);
  }
  return match[1];
}

function readFeatureEntries(source, featureName, label) {
  const escapedFeature = featureName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = source.match(new RegExp(`^${escapedFeature}\\s*=\\s*\\[([\\s\\S]*?)\\]`, "m"));
  if (!match) {
    return [];
  }
  return [...match[1].matchAll(/"([^"]+)"/g)].map((entry) => entry[1]);
}

function compareRustVersions(left, right) {
  const leftParts = left.split(".").map((part) => Number.parseInt(part, 10));
  const rightParts = right.split(".").map((part) => Number.parseInt(part, 10));
  for (let index = 0; index < Math.max(leftParts.length, rightParts.length); index += 1) {
    const leftValue = leftParts[index] ?? 0;
    const rightValue = rightParts[index] ?? 0;
    if (leftValue !== rightValue) {
      return leftValue - rightValue;
    }
  }
  return 0;
}

function cargoMetadata() {
  const result = spawnSync("cargo", ["metadata", "--no-deps", "--format-version", "1"], {
    cwd: REPO_ROOT,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(`cargo metadata --no-deps failed\n${result.stdout}\n${result.stderr}`);
  }
  return JSON.parse(result.stdout);
}

function packageByName(metadata, packageName) {
  const found = metadata.packages.find((pkg) => pkg.name === packageName);
  if (!found) {
    throw new Error(`cargo metadata missing package: ${packageName}`);
  }
  return found;
}

function assertExactDefaultFeatures(pkg, expected) {
  const actual = pkg.features.default ?? [];
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${pkg.name} default features expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}

function assertDxFeatureIsolation(pkg) {
  for (const [feature, entries] of Object.entries(pkg.features)) {
    const isDxFeature = feature === "dx-machine-cache" || feature === "dx-machine-cache-mmap";
    for (const entry of entries) {
      if (!isDxFeature && entry.includes("dx-machine-cache")) {
        throw new Error(`${pkg.name} feature ${feature} unexpectedly references ${entry}`);
      }
    }
  }
  if (pkg.features["dx-machine-cache-mmap"] && !pkg.features["dx-machine-cache-mmap"].includes("dx-machine-cache")) {
    throw new Error(`${pkg.name} dx-machine-cache-mmap must include base dx-machine-cache`);
  }
}

function assertDxSerializerDependency(manifestPath, expectedVersion) {
  const source = readRepoFile(manifestPath);
  assertContains(
    source,
    `serializer = { package = "dx-serializer", version = "${expectedVersion}", path = "../../../serializer", default-features = false, features = [`,
    manifestPath,
  );
  assertContains(source, '"typed-cache"', manifestPath);
  assertContains(source, "optional = true", manifestPath);
  assertContains(source, 'dx-machine-cache = [', manifestPath);
  assertContains(source, 'dx-machine-cache-mmap = [', manifestPath);

  const defaultFeatures = readFeatureEntries(source, "default", manifestPath);
  for (const forbidden of [
    "dx-machine-cache",
    "dx-machine-cache-mmap",
    "dep:serializer",
    "serializer/typed-cache-mmap",
  ]) {
    if (defaultFeatures.includes(forbidden)) {
      throw new Error(`${manifestPath} default feature must not enable ${forbidden}`);
    }
  }
}

function dxSerializerUsesLocalPath(manifestPath) {
  const source = readRepoFile(manifestPath);
  return (
    source.includes('serializer = { package = "dx-serializer"') &&
    source.includes('path = "../../../serializer"')
  );
}

function localSerializerReleaseBlockers(context) {
  const blockers = [];
  if (dxSerializerUsesLocalPath("crates/tauri-cli/Cargo.toml")) {
    blockers.push("tauri-cli uses local dx-serializer path dependency");
  }
  if (dxSerializerUsesLocalPath("crates/tauri-utils/Cargo.toml")) {
    blockers.push("tauri-utils uses local dx-serializer path dependency");
  }
  if (compareRustVersions(context.serializerRustVersion, context.workspaceRustVersion) > 0) {
    blockers.push(
      `dx-serializer requires Rust ${context.serializerRustVersion} while workspace MSRV is ${context.workspaceRustVersion}`,
    );
  }
  if (context.serializerEdition !== "2021") {
    blockers.push(`dx-serializer uses edition ${context.serializerEdition}`);
  }
  if (compareRustVersions(context.serializerDeriveRustVersion, context.workspaceRustVersion) > 0) {
    blockers.push(
      `dx-serializer-derive requires Rust ${context.serializerDeriveRustVersion} while workspace MSRV is ${context.workspaceRustVersion}`,
    );
  }
  if (context.serializerDeriveEdition !== "2021") {
    blockers.push(`dx-serializer-derive uses edition ${context.serializerDeriveEdition}`);
  }
  return blockers;
}

function serializerReleasePolicyStatus(blockers) {
  return blockers.length === 0 ? "release-ready" : "local-only-unpublished";
}

function main() {
  const rootCargo = readRepoFile("Cargo.toml");
  const metadata = cargoMetadata();
  const workspaceRustVersion = readManifestString(rootCargo, "rust-version", "Cargo.toml");
  const nodeCliCargo = readRepoFile("packages/cli/Cargo.toml");
  const docs = readRepoFile("crates/tauri-cli/ENVIRONMENT_VARIABLES.md");
  const plan = readRepoFile("PLAN.md");
  const serializerPath = path.resolve(REPO_ROOT, "..", "serializer", "Cargo.toml");
  const serializerDerivePath = path.resolve(REPO_ROOT, "..", "serializer", "derive", "Cargo.toml");
  const serializerCargo = fs.readFileSync(serializerPath, "utf8");
  const serializerDeriveCargo = fs.readFileSync(serializerDerivePath, "utf8");
  const serializerVersion = readManifestString(serializerCargo, "version", serializerPath);
  const serializerRustVersion = readManifestString(serializerCargo, "rust-version", serializerPath);
  const serializerEdition = readManifestString(serializerCargo, "edition", serializerPath);
  const serializerDeriveVersion = readManifestString(serializerDeriveCargo, "version", serializerDerivePath);
  const serializerDeriveRustVersion = readManifestString(serializerDeriveCargo, "rust-version", serializerDerivePath);
  const serializerDeriveEdition = readManifestString(serializerDeriveCargo, "edition", serializerDerivePath);
  assertContains(
    serializerCargo,
    `dx-serializer-derive = { version = "${serializerDeriveVersion}", path = "derive", optional = true }`,
    serializerPath,
  );
  assertContains(
    serializerDeriveCargo,
    `dx-serializer = { version = "${serializerVersion}", path = ".." }`,
    serializerDerivePath,
  );

  const utilsPackage = packageByName(metadata, "tauri-utils");
  const cliPackage = packageByName(metadata, "tauri-cli");
  const nodeCliPackage = packageByName(metadata, "tauri-cli-node");
  assertExactDefaultFeatures(utilsPackage, []);
  assertExactDefaultFeatures(cliPackage, ["rustls", "platform-certs"]);
  assertExactDefaultFeatures(nodeCliPackage, ["tauri-cli/default"]);
  for (const pkg of [utilsPackage, cliPackage, nodeCliPackage]) {
    assertDxFeatureIsolation(pkg);
  }

  assertDxSerializerDependency("crates/tauri-cli/Cargo.toml", serializerVersion);
  assertDxSerializerDependency("crates/tauri-utils/Cargo.toml", serializerVersion);
  const serializerReleaseBlockers = localSerializerReleaseBlockers({
    workspaceRustVersion,
    serializerRustVersion,
    serializerEdition,
    serializerDeriveRustVersion,
    serializerDeriveEdition,
  });
  const serializerReleasePolicy = serializerReleasePolicyStatus(serializerReleaseBlockers);
  assertContains(nodeCliCargo, 'default = ["tauri-cli/default"]', "packages/cli/Cargo.toml");
  const nodeDefaultFeatures = readFeatureEntries(nodeCliCargo, "default", "packages/cli/Cargo.toml");
  for (const forbidden of ["dx-machine-cache", "dx-machine-cache-mmap", "tauri-cli/dx-machine-cache"]) {
    if (nodeDefaultFeatures.includes(forbidden)) {
      throw new Error(`packages/cli default feature must not enable ${forbidden}`);
    }
  }

  const releaseBoundaryScript = "node .scripts/ci/test-dx-machine-cache-release-boundary.ts";
  const covectorWorkflow = readRepoFile(".github/workflows/covector-version-or-publish.yml");
  assertContains(covectorWorkflow, "check DX machine cache release boundary", ".github/workflows/covector-version-or-publish.yml");
  assertContains(covectorWorkflow, releaseBoundaryScript, ".github/workflows/covector-version-or-publish.yml");
  assertBefore(
    covectorWorkflow,
    releaseBoundaryScript,
    "uses: jbolda/covector/packages/action@covector-v0",
    ".github/workflows/covector-version-or-publish.yml",
  );

  for (const workflow of [
    ".github/workflows/publish-cli-rs.yml",
    ".github/workflows/publish-cli-js.yml",
    ".github/workflows/covector-version-or-publish.yml",
  ]) {
    const source = readRepoFile(workflow);
    const featureScanSource = source.replaceAll("test-dx-machine-cache-release-boundary.ts", "test-release-boundary.ts");
    assertNotContains(featureScanSource, "dx-machine-cache", workflow);
    assertNotContains(featureScanSource, "dx-machine-cache-mmap", workflow);
  }
  const changesConfig = JSON.parse(readRepoFile(".changes/config.json"));
  const rustPrepublishCommands = (changesConfig.pkgManagers?.rust?.prepublish ?? [])
    .map((entry) => (typeof entry === "string" ? entry : entry.command))
    .filter(Boolean)
    .join("\n");
  assertContains(
    rustPrepublishCommands,
    "DX_MACHINE_CACHE_RELEASE_BOUNDARY_MODE=publish node .scripts/ci/test-dx-machine-cache-release-boundary.ts",
    ".changes/config.json rust prepublish",
  );
  const publishedPackageNames = Object.keys(changesConfig.packages ?? {});
  for (const packageName of ["tauri-cli", "tauri-utils"]) {
    if (!publishedPackageNames.includes(packageName)) {
      throw new Error(`covector publish config is missing expected package: ${packageName}`);
    }
  }
  if (
    process.env.DX_MACHINE_CACHE_RELEASE_BOUNDARY_MODE === "publish" &&
    serializerReleasePolicy !== "release-ready"
  ) {
    throw new Error(
      `DX machine-cache publication is blocked by serializer release policy (${serializerReleasePolicy}): ${serializerReleaseBlockers.join("; ")}`,
    );
  }
  if (
    process.env.DX_MACHINE_CACHE_RELEASE_BOUNDARY_MODE === "publish" &&
    process.env.DX_ALLOW_MACHINE_CACHE_PUBLICATION !== "1"
  ) {
    throw new Error(
      "DX machine-cache publication requires DX_ALLOW_MACHINE_CACHE_PUBLICATION=1 after serializer registry/MSRV release readiness is proven",
    );
  }

  assertContains(docs, "default-off", "crates/tauri-cli/ENVIRONMENT_VARIABLES.md");
  assertContains(docs, "Official Tauri release binaries do not enable this local fork feature set", "crates/tauri-cli/ENVIRONMENT_VARIABLES.md");
  assertContains(docs, "The current release policy is local-only", "crates/tauri-cli/ENVIRONMENT_VARIABLES.md");
  assertContains(docs, "not crates.io/MSRV-ready", "crates/tauri-cli/ENVIRONMENT_VARIABLES.md");
  assertContains(plan, "serializer release policy now fails closed", "PLAN.md");
  assertContains(plan, "publish/default-on readiness is still blocked", "PLAN.md");

  const serializerRaisesMsrv = compareRustVersions(serializerRustVersion, workspaceRustVersion) > 0;
  const serializerUsesNewerEdition = serializerEdition !== "2021";
  const deriveRaisesMsrv = compareRustVersions(serializerDeriveRustVersion, workspaceRustVersion) > 0;
  const deriveUsesNewerEdition = serializerDeriveEdition !== "2021";
  if (serializerRaisesMsrv || serializerUsesNewerEdition || deriveRaisesMsrv || deriveUsesNewerEdition) {
    assertContains(docs, "local `dx-serializer` path dependency", "crates/tauri-cli/ENVIRONMENT_VARIABLES.md");
    assertContains(docs, `Rust ${serializerRustVersion} / edition ${serializerEdition}`, "crates/tauri-cli/ENVIRONMENT_VARIABLES.md");
    assertContains(docs, `derive crate also declares Rust ${serializerDeriveRustVersion} / edition ${serializerDeriveEdition}`, "crates/tauri-cli/ENVIRONMENT_VARIABLES.md");
    assertContains(docs, "publish/default-on readiness", "crates/tauri-cli/ENVIRONMENT_VARIABLES.md");
    assertContains(plan, `Rust ${serializerRustVersion} / edition ${serializerEdition}`, "PLAN.md");
    assertContains(plan, `derive crate also declares Rust ${serializerDeriveRustVersion} / edition ${serializerDeriveEdition}`, "PLAN.md");
  }

  console.log(
    [
      "machine-cache release boundary ok",
      `workspace_msrv=${workspaceRustVersion}`,
      `serializer_version=${serializerVersion}`,
      `serializer_msrv=${serializerRustVersion}`,
      `serializer_edition=${serializerEdition}`,
      `serializer_derive_version=${serializerDeriveVersion}`,
      `serializer_derive_msrv=${serializerDeriveRustVersion}`,
      `serializer_derive_edition=${serializerDeriveEdition}`,
    ].join("; "),
  );
}

main();
