#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const fs = require("node:fs");
const path = require("node:path");

const REPO_ROOT = path.resolve(__dirname, "..", "..");
const TEST_OUTPUT_ROOT = path.resolve("G:\\Dx\\test-outputs");
const RECEIPT_DIR = path.join(TEST_OUTPUT_ROOT, "tauri-stage5-mmap-fast-path-20260530-a");

const CACHE_READERS = [
  {
    file: "crates/tauri-utils/src/config/machine_cache.rs",
    anchor: "pub fn read_json_value_machine_cache(project_root: &Path, source_path: &Path) -> Option<Value>",
    typeName: "TauriConfigValueMachineCache",
    projection: "value_from_archive(mapped.archived(), source_path)",
    fallback: "read_machine_file_bounded(&paths.machine, TAURI_CONFIG_MACHINE_CACHE_MAX_BYTES)?",
    directArchiveSnippets: [
      "archived.source_path.as_str()",
      "archived_json_tree_to_value(&archived.value)",
    ],
    forbiddenSnippets: ["deserialize_tauri_config_machine_archive"],
  },
  {
    file: "crates/tauri-utils/src/config/machine_cache.rs",
    anchor: "fn read_project_config_machine_cache(",
    typeName: "TauriProjectConfigMachineCache",
    projection: "project_config_from_archive(",
    fallback: "read_machine_file_bounded(&paths.machine, TAURI_CONFIG_MACHINE_CACHE_MAX_BYTES)?",
    directArchiveSnippets: [
      "archived.config_path.as_str()",
      "current_project_sources_match_archived",
      "archived_json_tree_to_value",
    ],
    forbiddenSnippets: ["deserialize_tauri_project_config_machine_archive"],
  },
  {
    file: "crates/tauri-utils/src/config/machine_cache.rs",
    anchor: "fn read_project_config_projection_machine_cache(",
    typeName: "TauriProjectConfigMachineCache",
    projection: "project_config_projection_from_archive(",
    fallback: "read_machine_file_bounded(&cache_paths.machine, TAURI_CONFIG_MACHINE_CACHE_MAX_BYTES)?",
  },
  {
    file: "crates/tauri-cli/src/interface/rust/cargo_metadata_machine_cache.rs",
    anchor: "pub(super) fn read(tauri_dir: &Path) -> Option<CargoMetadata>",
    typeName: "CargoMetadataMachineCache",
    projection: "cargo_metadata_from_archive(mapped.archived(), tauri_dir, &source)",
    fallback: "read_machine_file_bounded(&paths.machine, CARGO_METADATA_MACHINE_CACHE_MAX_BYTES)?",
    directArchiveSnippets: [
      "archived.tauri_dir.as_str()",
      "current_cargo_metadata_sources_match_archived",
      "cargo_metadata_from_archived_metadata",
    ],
    forbiddenSnippets: ["deserialize_cargo_metadata_machine_archive"],
  },
  {
    file: "crates/tauri-cli/src/interface/rust/cargo_metadata_machine_cache.rs",
    anchor: "pub(super) fn read_projection(tauri_dir: &Path) -> Option<CargoMetadataProjection>",
    typeName: "CargoMetadataMachineCache",
    projection: "cargo_metadata_projection_from_archive(mapped.archived(), tauri_dir, &source)",
    fallback: "read_machine_file_bounded(&paths.machine, CARGO_METADATA_MACHINE_CACHE_MAX_BYTES)?",
    directArchiveSnippets: [
      "cargo_metadata_projection_from_archive",
      "in_workspace_dependency_paths_from_archived_metadata",
    ],
    forbiddenSnippets: ["deserialize_cargo_metadata_machine_archive"],
  },
  {
    file: "crates/tauri-cli/src/helpers/cargo_manifest.rs",
    anchor: "fn read_cargo_metadata_machine_cache(",
    typeName: "CargoMetadataMachineCache",
    projection: "cargo_metadata_from_archive(mapped.archived(), tauri_dir, &source)",
    fallback: "read_machine_file_bounded(&paths.machine, CARGO_METADATA_MACHINE_CACHE_MAX_BYTES)?",
    directArchiveSnippets: [
      "archived.tauri_dir.as_str()",
      "current_cargo_sources_match_archived",
      "archived.manifest.as_ref().map(cargo_manifest_from_archive)",
      "archived.lock.as_ref().map(cargo_lock_from_archive)",
      "cargo_manifest_dependency_from_archive",
      "cargo_lock_package_from_archive",
    ],
    forbiddenSnippets: [
      "deserialize_cargo_metadata_machine_archive",
      "deserialize_cargo_package_metadata_machine_archive",
    ],
  },
  {
    file: "crates/tauri-cli/src/helpers/config/package_version_machine_cache.rs",
    anchor: "fn read_package_version_machine_cache(",
    typeName: "PackageVersionMachineCache",
    projection: "package_version_from_archive(mapped.archived(), &source)",
    fallback: "read_machine_file_bounded(&paths.machine, PACKAGE_VERSION_MACHINE_CACHE_MAX_BYTES)?",
    directArchiveSnippets: ["archived.package_json_path.as_str()", "archived.version.as_str()"],
    forbiddenSnippets: ["deserialize_package_version_machine_archive"],
  },
  {
    file: "crates/tauri-cli/src/interface/rust/cargo_config/machine_cache.rs",
    anchor: "pub(super) fn read(tauri_dir: &Path) -> Option<Config>",
    typeName: "CargoConfigMachineCache",
    projection: "cargo_config_from_archive(mapped.archived(), tauri_dir, &source)",
    fallback: "read_machine_file_bounded(&paths.machine, CARGO_CONFIG_MACHINE_CACHE_MAX_BYTES)?",
    directArchiveSnippets: [
      "archived.tauri_dir.as_str()",
      "let target = archived",
      "cargo_config_source_fingerprint_from_archive",
    ],
    forbiddenSnippets: ["deserialize_cargo_config_machine_archive"],
  },
  {
    file: "crates/tauri-cli/src/interface/rust/cargo_settings_machine_cache.rs",
    anchor: "pub(super) fn read(dir: &Path) -> Option<CargoSettings>",
    typeName: "CargoSettingsMachineCache",
    projection: "cargo_settings_from_archive(mapped.archived(), &source)",
    fallback: "read_machine_file_bounded(&paths.machine, CARGO_SETTINGS_MACHINE_CACHE_MAX_BYTES)?",
    directArchiveSnippets: ["archived.cargo_toml_path.as_str()"],
    forbiddenSnippets: ["deserialize_cargo_settings_machine_archive"],
  },
  {
    file: "crates/tauri-cli/src/interface/rust/workspace_machine_cache.rs",
    anchor: "pub(super) fn read(tauri_dir: &Path) -> Option<PathBuf>",
    typeName: "CargoWorkspaceMachineCache",
    projection: "workspace_dir_from_archive(mapped.archived(), tauri_dir, &source)",
    fallback: "read_machine_file_bounded(&paths.machine, CARGO_WORKSPACE_MACHINE_CACHE_MAX_BYTES)?",
    directArchiveSnippets: [
      "archived.tauri_dir.as_str()",
      "archived.workspace_dir.as_str()",
      "workspace_source_fingerprint_from_archive",
    ],
    forbiddenSnippets: ["deserialize_cargo_workspace_machine_archive"],
  },
];

function readRepoFile(relativePath) {
  return fs.readFileSync(path.join(REPO_ROOT, relativePath), "utf8");
}

function assertContains(text, needle, label) {
  if (!text.includes(needle)) {
    throw new Error(`${label} is missing: ${needle}`);
  }
}

function assertMmapFastPath(reader) {
  const source = readRepoFile(reader.file);
  const label = reader.file;
  const mmapCall = `open_typed_machine_cache::<${reader.typeName}>`;
  const anchorIndex = source.indexOf(reader.anchor);
  const mmapIndex = source.indexOf(mmapCall, anchorIndex);
  const fallbackIndex = source.indexOf(reader.fallback, anchorIndex);

  assertContains(source, '#[cfg(feature = "dx-machine-cache-mmap")]', label);
  assertContains(source, "use serializer::machine::open_typed_machine_cache;", label);
  assertContains(source, reader.anchor, label);
  assertContains(source, mmapCall, label);
  assertContains(source, reader.projection, label);
  assertContains(source, reader.fallback, label);

  if (anchorIndex < 0 || mmapIndex < 0 || fallbackIndex < 0 || mmapIndex > fallbackIndex) {
    throw new Error(`${label} must try mmap/open before fallback after ${reader.anchor}`);
  }

  for (const snippet of reader.directArchiveSnippets ?? []) {
    assertContains(source, snippet, label);
  }
  for (const snippet of reader.forbiddenSnippets ?? []) {
    if (source.includes(snippet)) {
      throw new Error(`${label} still contains forbidden source: ${snippet}`);
    }
  }
}

function main() {
  const cargoToml = readRepoFile("crates/tauri-cli/Cargo.toml");
  assertContains(cargoToml, "dx-machine-cache-mmap = [", "tauri-cli feature map");
  assertContains(cargoToml, '"serializer/typed-cache-mmap"', "tauri-cli feature map");
  assertContains(cargoToml, '"tauri-utils/dx-machine-cache-mmap"', "tauri-cli feature map");

  for (const reader of CACHE_READERS) {
    assertMmapFastPath(reader);
  }

  fs.mkdirSync(RECEIPT_DIR, { recursive: true });
  fs.writeFileSync(
    path.join(RECEIPT_DIR, "mmap-fast-path-coverage.json"),
    `${JSON.stringify(
      {
        schema: "dx.tauri.machine_cache_mmap_coverage",
        checked_at: new Date().toISOString(),
        repo_root: REPO_ROOT,
        cache_readers: CACHE_READERS,
      },
      null,
      2,
    )}\n`,
  );

  console.log(`checked mmap fast-path coverage for ${CACHE_READERS.length} machine cache readers`);
}

main();
