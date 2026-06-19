#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const fs = require("node:fs");
const path = require("node:path");

const REPO_ROOT = path.resolve(__dirname, "..", "..");
const TEST_OUTPUT_ROOT = path.resolve("G:\\Dx\\test-outputs");
const RECEIPT_DIR = path.join(TEST_OUTPUT_ROOT, "tauri-stage6-machine-surface-matrix-20260531-a");

require.extensions[".ts"] = require.extensions[".js"];
const { GLOBAL_CHECKS, SURFACES } = require("./dx-machine-cache-surface-matrix-data.ts");

const DISCOVERY_ROOTS = ["crates/tauri-cli/src", "crates/tauri-utils/src"];
const IGNORED_MACHINE_ARTIFACTS = new Set(["oversized.machine", "{prefix}-{index:03}.machine"]);

function readRepoFile(relativePath) {
  return fs.readFileSync(path.join(REPO_ROOT, relativePath), "utf8");
}

function toRepoPath(filePath) {
  return path.relative(REPO_ROOT, filePath).replaceAll("\\", "/");
}

function walkRustFiles(relativeRoot) {
  const root = path.join(REPO_ROOT, relativeRoot);
  const files = [];
  const stack = [root];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const fullPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(fullPath);
      } else if (entry.isFile() && entry.name.endsWith(".rs")) {
        files.push(fullPath);
      }
    }
  }
  return files;
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

function assertUnderTestOutputs(directory) {
  const relative = path.relative(TEST_OUTPUT_ROOT, directory);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new Error(`receipt directory must stay under ${TEST_OUTPUT_ROOT}`);
  }
}

function discoverPathConstructors() {
  return discoverRustMatches("paths_for_project_cache\\s*\\(");
}

function discoverRustMatches(patternText) {
  const matches = [];
  const pattern = new RegExp(patternText, "g");
  for (const relativeRoot of DISCOVERY_ROOTS) {
    for (const file of walkRustFiles(relativeRoot)) {
      const source = fs.readFileSync(file, "utf8");
      for (const match of source.matchAll(pattern)) {
        matches.push({
          file: toRepoPath(file),
          offset: match.index,
        });
      }
    }
  }
  return matches;
}

function discoverMachineArtifacts() {
  const artifacts = new Set();
  const pattern = /"([^"\n]*\.machine)"/g;
  for (const relativeRoot of DISCOVERY_ROOTS) {
    for (const file of walkRustFiles(relativeRoot)) {
      const source = fs.readFileSync(file, "utf8");
      for (const match of source.matchAll(pattern)) {
        const artifact = match[1].replaceAll("\\", "/").split("/").pop();
        if (!IGNORED_MACHINE_ARTIFACTS.has(artifact)) {
          artifacts.add(artifact);
        }
      }
    }
  }
  return [...artifacts].sort();
}

function assertSnippets(check) {
  const source = readRepoFile(check.file);
  for (const snippet of check.snippets) {
    assertContains(source, snippet, check.file);
  }
}

function main() {
  const constructors = discoverPathConstructors();
  const readers = discoverRustMatches("open_typed_machine_cache::<");
  const writers = discoverRustMatches("write_typed_machine_cache\\s*\\(");
  const schemas = discoverRustMatches("->\\s*MachineCacheSchema\\s*\\{");
  const discoveredArtifacts = discoverMachineArtifacts();

  assertUnderTestOutputs(RECEIPT_DIR);

  if (SURFACES.length === 0 && constructors.length > 0) {
    throw new Error(
      `surface matrix is empty but ${constructors.length} paths_for_project_cache constructors were found`,
    );
  }

  assertUnique(
    SURFACES.map((surface) => surface.id),
    "surface ids",
  );

  const expectedConstructorCount = SURFACES.reduce(
    (total, surface) => total + surface.pathConstructors.length,
    0,
  );
  const expectedReaderCount = SURFACES.reduce((total, surface) => total + surface.readerCount, 0);
  const expectedWriterCount = SURFACES.reduce((total, surface) => total + surface.writerCount, 0);
  const expectedSchemaCount = SURFACES.reduce((total, surface) => total + surface.schemaCount, 0);

  if (constructors.length !== expectedConstructorCount) {
    throw new Error(
      `expected ${expectedConstructorCount} paths_for_project_cache constructors, found ${constructors.length}`,
    );
  }
  if (readers.length !== expectedReaderCount) {
    throw new Error(`expected ${expectedReaderCount} mmap readers, found ${readers.length}`);
  }
  if (writers.length !== expectedWriterCount) {
    throw new Error(`expected ${expectedWriterCount} typed writers, found ${writers.length}`);
  }
  if (schemas.length !== expectedSchemaCount) {
    throw new Error(`expected ${expectedSchemaCount} machine schemas, found ${schemas.length}`);
  }

  const coveredArtifacts = new Set(SURFACES.flatMap((surface) => surface.artifacts));
  for (const artifact of discoveredArtifacts) {
    if (!coveredArtifacts.has(artifact)) {
      throw new Error(`discovered uncataloged .machine artifact: ${artifact}`);
    }
  }

  for (const check of GLOBAL_CHECKS) {
    assertSnippets(check);
  }

  for (const surface of SURFACES) {
    for (const check of surface.pathConstructors) {
      assertSnippets(check);
    }
    for (const check of surface.sourceChecks) {
      assertSnippets(check);
    }
    for (const check of surface.coverageChecks) {
      assertSnippets(check);
    }
  }

  fs.mkdirSync(RECEIPT_DIR, { recursive: true });
  const receiptPath = path.join(RECEIPT_DIR, "machine-cache-surface-matrix.json");
  fs.writeFileSync(
    receiptPath,
    `${JSON.stringify(
      {
        schema: "dx.tauri.machine_cache_surface_matrix",
        checked_at: new Date().toISOString(),
        repo_root: REPO_ROOT,
        discovery_roots: DISCOVERY_ROOTS,
        surface_count: SURFACES.length,
        path_constructor_count: constructors.length,
        reader_count: readers.length,
        writer_count: writers.length,
        schema_count: schemas.length,
        discovered_machine_artifacts: discoveredArtifacts,
        surfaces: SURFACES.map((surface) => ({
          id: surface.id,
          artifacts: surface.artifacts,
          reader_count: surface.readerCount,
          writer_count: surface.writerCount,
          schema_count: surface.schemaCount,
          status: surface.status,
        })),
      },
      null,
      2,
    )}\n`,
  );

  console.log(
    `checked ${SURFACES.length} machine-cache surfaces and ${constructors.length} path constructors; receipt=${receiptPath}`,
  );
}

main();
