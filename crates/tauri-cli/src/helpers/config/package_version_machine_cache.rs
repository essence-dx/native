// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
  env,
  path::{Path, PathBuf},
  str::FromStr,
};

use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use semver::Version;
use serde_json::Value;
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

pub(super) const TAURI_DX_MACHINE_CACHE_ENV: &str = "TAURI_DX_MACHINE_CACHE";

const PACKAGE_VERSION_MACHINE_CACHE_SCHEMA_VERSION: u32 = 1;
const PACKAGE_VERSION_MACHINE_CACHE_MAX_BYTES: u64 = 1024 * 1024;

#[cfg(test)]
static PACKAGE_VERSION_MACHINE_CACHE_WRITE_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static PACKAGE_VERSION_MACHINE_CACHE_READ_HITS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone)]
pub(super) struct PackageVersionMachineCandidate {
  config_dir: PathBuf,
  package_json_path: PathBuf,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct PackageVersionMachineCache {
  package_json_path: String,
  version: String,
}

pub(super) fn package_version_cache_candidate(
  config: &Value,
  config_dir: &Path,
) -> Option<PackageVersionMachineCandidate> {
  let version = config.get("version")?.as_str()?;
  let path = PathBuf::from(version);
  let package_json_path = if path.is_absolute() {
    path
  } else {
    config_dir.join(path)
  };

  (package_json_path.is_file()
    && package_json_path
      .file_name()
      .is_some_and(|name| name == "package.json"))
  .then_some(PackageVersionMachineCandidate {
    config_dir: config_dir.to_path_buf(),
    package_json_path,
  })
}

pub(super) fn apply_cached_package_version(
  config: &mut Value,
  candidate: &PackageVersionMachineCandidate,
) -> bool {
  if !machine_cache_enabled() {
    return false;
  }

  let Some(version) = read_package_version_machine_cache(candidate) else {
    return false;
  };
  let Some(config) = config.as_object_mut() else {
    return false;
  };
  config.insert("version".into(), Value::String(version));
  #[cfg(test)]
  PACKAGE_VERSION_MACHINE_CACHE_READ_HITS.fetch_add(1, Ordering::SeqCst);
  true
}

pub(super) fn write_package_version_machine_cache(
  candidate: &PackageVersionMachineCandidate,
  version: &str,
) -> Result<PathBuf, MachineCacheError> {
  #[cfg(test)]
  PACKAGE_VERSION_MACHINE_CACHE_WRITE_ATTEMPTS.fetch_add(1, Ordering::SeqCst);

  Version::from_str(version)
    .map_err(|_| MachineCacheError::InvalidCachePath("invalid package version".into()))?;

  let paths = package_version_machine_cache_paths(candidate)?;
  let source = source_fingerprint(&candidate.package_json_path)?;
  let payload = PackageVersionMachineCache {
    package_json_path: normalized_path(&source.path),
    version: version.to_string(),
  };
  let receipt = write_typed_machine_cache(
    &payload,
    &source,
    &paths,
    package_version_machine_cache_schema(),
    MachineCacheWriteOptions {
      codec: MachineCacheCodec::None,
    },
  )?;
  Ok(receipt.machine)
}

#[cfg(test)]
pub(super) fn package_version_machine_path(config_dir: &Path) -> PathBuf {
  config_dir
    .join(".dx")
    .join("tauri")
    .join("package-version.machine")
}

fn read_package_version_machine_cache(
  candidate: &PackageVersionMachineCandidate,
) -> Option<String> {
  let paths = package_version_machine_cache_paths(candidate).ok()?;
  if !machine_cache_file_is_candidate(&paths.machine, PACKAGE_VERSION_MACHINE_CACHE_MAX_BYTES) {
    return None;
  }
  let source = source_fingerprint(&candidate.package_json_path).ok()?;

  #[cfg(feature = "dx-machine-cache-mmap")]
  if let Ok(mapped) = open_typed_machine_cache::<PackageVersionMachineCache>(
    &paths,
    &source,
    package_version_machine_cache_schema(),
  ) {
    return package_version_from_archive(mapped.archived(), &source);
  }

  let bytes = read_machine_file_bounded(&paths.machine, PACKAGE_VERSION_MACHINE_CACHE_MAX_BYTES)?;
  let archived = access_typed_machine_cache::<PackageVersionMachineCache>(
    &bytes,
    &source,
    package_version_machine_cache_schema(),
  )
  .ok()?;
  package_version_from_archive(archived, &source)
}

fn package_version_from_archive(
  archived: &ArchivedPackageVersionMachineCache,
  source: &MachineCacheSource,
) -> Option<String> {
  let package_json_path = archived.package_json_path.as_str();
  let version = archived.version.as_str();
  if package_json_path != normalized_path(&source.path) || Version::from_str(version).is_err() {
    return None;
  }
  Some(version.to_string())
}

fn package_version_machine_cache_paths(
  candidate: &PackageVersionMachineCandidate,
) -> Result<serializer::machine::MachineCachePaths, MachineCacheError> {
  paths_for_project_cache(
    &candidate.config_dir,
    "tauri",
    "package-version",
    &candidate.package_json_path,
  )
}

fn package_version_machine_cache_schema() -> MachineCacheSchema {
  MachineCacheSchema {
    name: "dx.tauri.cli.package_version",
    version: PACKAGE_VERSION_MACHINE_CACHE_SCHEMA_VERSION,
    kind: MachineCacheKind::Config,
  }
}

pub(super) fn machine_cache_enabled() -> bool {
  env::var(TAURI_DX_MACHINE_CACHE_ENV)
    .map(|value| {
      matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
      )
    })
    .unwrap_or(false)
}

fn normalized_path(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
pub(super) fn reset_package_version_machine_cache_write_attempts() {
  PACKAGE_VERSION_MACHINE_CACHE_WRITE_ATTEMPTS.store(0, Ordering::SeqCst);
}

#[cfg(test)]
pub(super) fn package_version_machine_cache_write_attempts() -> usize {
  PACKAGE_VERSION_MACHINE_CACHE_WRITE_ATTEMPTS.load(Ordering::SeqCst)
}

#[cfg(test)]
pub(super) fn reset_package_version_machine_cache_read_hits() {
  PACKAGE_VERSION_MACHINE_CACHE_READ_HITS.store(0, Ordering::SeqCst);
}

#[cfg(test)]
pub(super) fn package_version_machine_cache_read_hits() -> usize {
  PACKAGE_VERSION_MACHINE_CACHE_READ_HITS.load(Ordering::SeqCst)
}
