// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(test)]
use std::fs;
use std::{
  env,
  path::{Path, PathBuf},
};

use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
#[cfg(feature = "dx-machine-cache-mmap")]
use serializer::machine::open_typed_machine_cache;
use serializer::machine::{
  access_typed_machine_cache, paths_for_project_cache, source_fingerprint,
  write_typed_machine_cache, MachineCacheCodec, MachineCacheError, MachineCacheKind,
  MachineCacheSchema, MachineCacheSource, MachineCacheWriteOptions,
};

use crate::helpers::machine_cache_io::{
  machine_cache_file_is_candidate, read_machine_file_bounded,
};

use super::{
  BinarySettings, CargoPackageSettings, CargoSettings, MaybeWorkspace, TomlWorkspaceField,
  WorkspacePackageSettings, WorkspaceSettings,
};

pub(super) const TAURI_DX_MACHINE_CACHE_ENV: &str = "TAURI_DX_MACHINE_CACHE";

const CARGO_SETTINGS_MACHINE_CACHE_SCHEMA_VERSION: u32 = 1;
const CARGO_SETTINGS_MACHINE_CACHE_MAX_BYTES: u64 = 1024 * 1024;

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct CargoSettingsMachineCache {
  cargo_toml_path: String,
  settings: CargoSettingsMachine,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct CargoSettingsMachine {
  package: Option<CargoPackageSettingsMachine>,
  workspace: Option<WorkspaceSettingsMachine>,
  bin: Option<Vec<BinarySettingsMachine>>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct WorkspaceSettingsMachine {
  package: Option<WorkspacePackageSettingsMachine>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct WorkspacePackageSettingsMachine {
  authors: Option<Vec<String>>,
  description: Option<String>,
  homepage: Option<String>,
  version: Option<String>,
  license: Option<String>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct BinarySettingsMachine {
  name: String,
  filename: Option<String>,
  path: Option<String>,
  required_features: Option<Vec<String>>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct CargoPackageSettingsMachine {
  name: String,
  version: Option<MaybeWorkspaceStringMachine>,
  description: Option<MaybeWorkspaceStringMachine>,
  homepage: Option<MaybeWorkspaceStringMachine>,
  authors: Option<MaybeWorkspaceStringVecMachine>,
  license: Option<MaybeWorkspaceStringMachine>,
  default_run: Option<String>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
enum MaybeWorkspaceStringMachine {
  Workspace { workspace: bool },
  Defined(String),
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
enum MaybeWorkspaceStringVecMachine {
  Workspace { workspace: bool },
  Defined(Vec<String>),
}

pub(super) fn enabled() -> bool {
  env::var(TAURI_DX_MACHINE_CACHE_ENV)
    .map(|value| {
      matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
      )
    })
    .unwrap_or(false)
}

pub(super) fn read(dir: &Path) -> Option<CargoSettings> {
  let paths = cargo_settings_machine_cache_paths(dir).ok()?;
  if !machine_cache_file_is_candidate(&paths.machine, CARGO_SETTINGS_MACHINE_CACHE_MAX_BYTES) {
    return None;
  }
  let source = source_fingerprint(&paths.source).ok()?;

  #[cfg(feature = "dx-machine-cache-mmap")]
  if let Ok(mapped) = open_typed_machine_cache::<CargoSettingsMachineCache>(
    &paths,
    &source,
    cargo_settings_machine_cache_schema(),
  ) {
    return cargo_settings_from_archive(mapped.archived(), &source);
  }

  let bytes = read_machine_file_bounded(&paths.machine, CARGO_SETTINGS_MACHINE_CACHE_MAX_BYTES)?;
  let archived = access_typed_machine_cache::<CargoSettingsMachineCache>(
    &bytes,
    &source,
    cargo_settings_machine_cache_schema(),
  )
  .ok()?;
  cargo_settings_from_archive(archived, &source)
}

pub(super) fn write(dir: &Path, settings: &CargoSettings) -> Result<PathBuf, MachineCacheError> {
  let paths = cargo_settings_machine_cache_paths(dir)?;
  let source = source_fingerprint(&paths.source)?;
  let payload = CargoSettingsMachineCache {
    cargo_toml_path: normalized_path(&source.path),
    settings: CargoSettingsMachine::from(settings),
  };
  let receipt = write_typed_machine_cache(
    &payload,
    &source,
    &paths,
    cargo_settings_machine_cache_schema(),
    MachineCacheWriteOptions {
      codec: MachineCacheCodec::None,
    },
  )?;
  Ok(receipt.machine)
}

#[cfg(test)]
pub(super) fn machine_path(dir: &Path) -> PathBuf {
  dir.join(".dx").join("tauri").join("cargo-settings.machine")
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub(super) struct CargoSettingsMachineReadPhaseTimings {
  pub(super) source_path: String,
  pub(super) source_modified_unix_ms: Option<u64>,
  pub(super) source_blake3_hex: String,
  pub(super) machine_path: String,
  pub(super) path_resolution_ns: u64,
  pub(super) machine_size_preflight_ns: u64,
  pub(super) source_fingerprint_ns: u64,
  pub(super) machine_file_read_ns: u64,
  pub(super) machine_access_validation_ns: u64,
  pub(super) owned_projection_materialization_ns: u64,
  pub(super) total_ns: u64,
  pub(super) source_bytes: u64,
  pub(super) machine_bytes: u64,
}

#[cfg(test)]
pub(super) fn read_with_phase_timings(
  dir: &Path,
) -> Option<(CargoSettings, CargoSettingsMachineReadPhaseTimings)> {
  let total_started = std::time::Instant::now();

  let phase_started = std::time::Instant::now();
  let paths = cargo_settings_machine_cache_paths(dir).ok()?;
  let path_resolution_ns = elapsed_ns(phase_started);

  let phase_started = std::time::Instant::now();
  let machine_bytes = fs::metadata(&paths.machine).ok()?.len();
  if !machine_cache_file_is_candidate(&paths.machine, CARGO_SETTINGS_MACHINE_CACHE_MAX_BYTES) {
    return None;
  }
  let machine_size_preflight_ns = elapsed_ns(phase_started);

  let phase_started = std::time::Instant::now();
  let source = source_fingerprint(&paths.source).ok()?;
  let source_bytes = source.bytes;
  let source_path = normalized_path(&source.path);
  let source_modified_unix_ms = source.modified_unix_ms;
  let source_blake3_hex = hex_bytes(&source.blake3);
  let source_fingerprint_ns = elapsed_ns(phase_started);

  let phase_started = std::time::Instant::now();
  let bytes = read_machine_file_bounded(&paths.machine, CARGO_SETTINGS_MACHINE_CACHE_MAX_BYTES)?;
  let machine_file_read_ns = elapsed_ns(phase_started);

  let phase_started = std::time::Instant::now();
  let archived = access_typed_machine_cache::<CargoSettingsMachineCache>(
    &bytes,
    &source,
    cargo_settings_machine_cache_schema(),
  )
  .ok()?;
  let machine_access_validation_ns = elapsed_ns(phase_started);

  let phase_started = std::time::Instant::now();
  let settings = cargo_settings_from_archive(archived, &source)?;
  let owned_projection_materialization_ns = elapsed_ns(phase_started);

  let total_ns = elapsed_ns(total_started);
  Some((
    settings,
    CargoSettingsMachineReadPhaseTimings {
      source_path,
      source_modified_unix_ms,
      source_blake3_hex,
      machine_path: normalized_path(&paths.machine),
      path_resolution_ns,
      machine_size_preflight_ns,
      source_fingerprint_ns,
      machine_file_read_ns,
      machine_access_validation_ns,
      owned_projection_materialization_ns,
      total_ns,
      source_bytes,
      machine_bytes,
    },
  ))
}

#[cfg(test)]
fn elapsed_ns(started: std::time::Instant) -> u64 {
  u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

#[cfg(test)]
fn hex_bytes(bytes: &[u8]) -> String {
  let mut hex = String::with_capacity(bytes.len() * 2);
  for byte in bytes {
    use std::fmt::Write as _;
    let _ = write!(&mut hex, "{byte:02x}");
  }
  hex
}

fn cargo_settings_from_archive(
  archived: &ArchivedCargoSettingsMachineCache,
  source: &MachineCacheSource,
) -> Option<CargoSettings> {
  if archived.cargo_toml_path.as_str() != normalized_path(&source.path) {
    return None;
  }
  let settings = deserialize_cargo_settings_machine(&archived.settings).ok()?;
  Some(settings.into())
}

fn deserialize_cargo_settings_machine(
  archived: &ArchivedCargoSettingsMachine,
) -> Result<CargoSettingsMachine, rkyv::rancor::Error> {
  let mut deserializer = rkyv::de::Pool::new();
  RkyvDeserialize::deserialize(archived, rkyv::rancor::Strategy::wrap(&mut deserializer))
}

fn cargo_settings_machine_cache_paths(
  dir: &Path,
) -> Result<serializer::machine::MachineCachePaths, MachineCacheError> {
  paths_for_project_cache(dir, "tauri", "cargo-settings", &dir.join("Cargo.toml"))
}

fn cargo_settings_machine_cache_schema() -> MachineCacheSchema {
  MachineCacheSchema {
    name: "dx.tauri.cli.cargo_settings",
    version: CARGO_SETTINGS_MACHINE_CACHE_SCHEMA_VERSION,
    kind: MachineCacheKind::Config,
  }
}

fn normalized_path(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

impl From<&CargoSettings> for CargoSettingsMachine {
  fn from(settings: &CargoSettings) -> Self {
    Self {
      package: settings.package.as_ref().map(Into::into),
      workspace: settings.workspace.as_ref().map(Into::into),
      bin: settings
        .bin
        .as_ref()
        .map(|bins| bins.iter().map(Into::into).collect()),
    }
  }
}

impl From<CargoSettingsMachine> for CargoSettings {
  fn from(settings: CargoSettingsMachine) -> Self {
    Self {
      package: settings.package.map(Into::into),
      workspace: settings.workspace.map(Into::into),
      bin: settings
        .bin
        .map(|bins| bins.into_iter().map(Into::into).collect()),
    }
  }
}

impl From<&WorkspaceSettings> for WorkspaceSettingsMachine {
  fn from(settings: &WorkspaceSettings) -> Self {
    Self {
      package: settings.package.as_ref().map(Into::into),
    }
  }
}

impl From<WorkspaceSettingsMachine> for WorkspaceSettings {
  fn from(settings: WorkspaceSettingsMachine) -> Self {
    Self {
      package: settings.package.map(Into::into),
    }
  }
}

impl From<&WorkspacePackageSettings> for WorkspacePackageSettingsMachine {
  fn from(settings: &WorkspacePackageSettings) -> Self {
    Self {
      authors: settings.authors.clone(),
      description: settings.description.clone(),
      homepage: settings.homepage.clone(),
      version: settings.version.clone(),
      license: settings.license.clone(),
    }
  }
}

impl From<WorkspacePackageSettingsMachine> for WorkspacePackageSettings {
  fn from(settings: WorkspacePackageSettingsMachine) -> Self {
    Self {
      authors: settings.authors,
      description: settings.description,
      homepage: settings.homepage,
      version: settings.version,
      license: settings.license,
    }
  }
}

impl From<&BinarySettings> for BinarySettingsMachine {
  fn from(settings: &BinarySettings) -> Self {
    Self {
      name: settings.name.clone(),
      filename: settings.filename.clone(),
      path: settings.path.clone(),
      required_features: settings.required_features.clone(),
    }
  }
}

impl From<BinarySettingsMachine> for BinarySettings {
  fn from(settings: BinarySettingsMachine) -> Self {
    Self {
      name: settings.name,
      filename: settings.filename,
      path: settings.path,
      required_features: settings.required_features,
    }
  }
}

impl From<&CargoPackageSettings> for CargoPackageSettingsMachine {
  fn from(settings: &CargoPackageSettings) -> Self {
    Self {
      name: settings.name.clone(),
      version: settings.version.as_ref().map(Into::into),
      description: settings.description.as_ref().map(Into::into),
      homepage: settings.homepage.as_ref().map(Into::into),
      authors: settings.authors.as_ref().map(Into::into),
      license: settings.license.as_ref().map(Into::into),
      default_run: settings.default_run.clone(),
    }
  }
}

impl From<CargoPackageSettingsMachine> for CargoPackageSettings {
  fn from(settings: CargoPackageSettingsMachine) -> Self {
    Self {
      name: settings.name,
      version: settings.version.map(Into::into),
      description: settings.description.map(Into::into),
      homepage: settings.homepage.map(Into::into),
      authors: settings.authors.map(Into::into),
      license: settings.license.map(Into::into),
      default_run: settings.default_run,
    }
  }
}

impl From<&MaybeWorkspace<String>> for MaybeWorkspaceStringMachine {
  fn from(value: &MaybeWorkspace<String>) -> Self {
    match value {
      MaybeWorkspace::Workspace(workspace) => Self::Workspace {
        workspace: workspace.workspace,
      },
      MaybeWorkspace::Defined(value) => Self::Defined(value.clone()),
    }
  }
}

impl From<MaybeWorkspaceStringMachine> for MaybeWorkspace<String> {
  fn from(value: MaybeWorkspaceStringMachine) -> Self {
    match value {
      MaybeWorkspaceStringMachine::Workspace { workspace } => {
        Self::Workspace(TomlWorkspaceField { workspace })
      }
      MaybeWorkspaceStringMachine::Defined(value) => Self::Defined(value),
    }
  }
}

impl From<&MaybeWorkspace<Vec<String>>> for MaybeWorkspaceStringVecMachine {
  fn from(value: &MaybeWorkspace<Vec<String>>) -> Self {
    match value {
      MaybeWorkspace::Workspace(workspace) => Self::Workspace {
        workspace: workspace.workspace,
      },
      MaybeWorkspace::Defined(value) => Self::Defined(value.clone()),
    }
  }
}

impl From<MaybeWorkspaceStringVecMachine> for MaybeWorkspace<Vec<String>> {
  fn from(value: MaybeWorkspaceStringVecMachine) -> Self {
    match value {
      MaybeWorkspaceStringVecMachine::Workspace { workspace } => {
        Self::Workspace(TomlWorkspaceField { workspace })
      }
      MaybeWorkspaceStringVecMachine::Defined(value) => Self::Defined(value),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn dx_cargo_settings_machine_read_phase_timings_reports_contract() {
    let project = temp_cargo_settings_cache_project("phase-contract");
    write_manifest(project.path(), "phase-app", "1.2.3");
    let settings = sample_settings("phase-app");
    write(project.path(), &settings).expect("write cache");

    let (cached, timings) =
      read_with_phase_timings(project.path()).expect("read cache with timings");

    assert_eq!(cached.package.as_ref().expect("package").name, "phase-app");
    assert!(timings.source_path.ends_with("/Cargo.toml"));
    assert!(timings.source_bytes > 0);
    assert_eq!(timings.source_blake3_hex.len(), 64);
    assert!(timings
      .source_blake3_hex
      .chars()
      .all(|character| character.is_ascii_hexdigit()));
    assert!(timings
      .machine_path
      .ends_with("/.dx/tauri/cargo-settings.machine"));
    assert!(timings.machine_bytes > 0);
    assert!(timings.machine_size_preflight_ns > 0);
    assert!(timings.source_fingerprint_ns > 0);
    assert!(timings.machine_file_read_ns > 0);
    assert!(timings.machine_access_validation_ns > 0);
    assert!(timings.owned_projection_materialization_ns > 0);
    let phase_sum = timings.path_resolution_ns
      + timings.machine_size_preflight_ns
      + timings.source_fingerprint_ns
      + timings.machine_file_read_ns
      + timings.machine_access_validation_ns
      + timings.owned_projection_materialization_ns;
    assert!(timings.total_ns >= phase_sum);
  }

  #[test]
  fn dx_cargo_settings_machine_read_phase_timings_rejects_corrupt_or_stale_machine() {
    let project = temp_cargo_settings_cache_project("phase-rejects");
    write_manifest(project.path(), "phase-app", "1.2.3");
    let settings = sample_settings("phase-app");
    let machine = write(project.path(), &settings).expect("write cache");

    let mut bytes = fs::read(&machine).expect("read machine");
    let last = bytes.len().checked_sub(1).expect("machine is not empty");
    bytes[last] ^= 0x01;
    fs::write(&machine, bytes).expect("corrupt machine");
    assert!(read_with_phase_timings(project.path()).is_none());

    write(project.path(), &settings).expect("rewrite cache");
    write_manifest(project.path(), "phase-app", "2.0.0");
    assert!(read_with_phase_timings(project.path()).is_none());
  }

  fn temp_cargo_settings_cache_project(name: &str) -> tempfile::TempDir {
    let base = std::env::var_os("DX_TEST_OUTPUT_DIR")
      .map(PathBuf::from)
      .unwrap_or_else(std::env::temp_dir);
    fs::create_dir_all(&base).expect("create test output base");
    tempfile::Builder::new()
      .prefix(&format!("tauri-dx-cargo-settings-cache-{name}-"))
      .tempdir_in(base)
      .expect("create cargo settings cache temp project")
  }

  fn write_manifest(dir: &Path, name: &str, version: &str) {
    fs::write(
      dir.join("Cargo.toml"),
      format!(
        r#"[package]
name = "{name}"
version = "{version}"
description = "Phase timing"
homepage = "https://phase.example"
authors = ["DX"]
license = "MIT"
default-run = "phase-app"

[[bin]]
name = "phase-app"
path = "src/main.rs"
"#
      ),
    )
    .expect("write Cargo.toml");
  }

  fn sample_settings(name: &str) -> CargoSettings {
    CargoSettings {
      package: Some(CargoPackageSettings {
        name: name.into(),
        version: Some(MaybeWorkspace::Defined("1.2.3".into())),
        description: Some(MaybeWorkspace::Defined("Phase timing".into())),
        homepage: Some(MaybeWorkspace::Defined("https://phase.example".into())),
        authors: Some(MaybeWorkspace::Defined(vec!["DX".into()])),
        license: Some(MaybeWorkspace::Defined("MIT".into())),
        default_run: Some("phase-app".into()),
      }),
      workspace: None,
      bin: Some(vec![BinarySettings {
        name: "phase-app".into(),
        filename: None,
        path: Some("src/main.rs".into()),
        required_features: None,
      }]),
    }
  }
}
