// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde::Deserialize;

#[cfg(feature = "dx-machine-cache")]
use std::collections::HashSet;
use std::{
  collections::HashMap,
  fs,
  path::{Path, PathBuf},
};

#[cfg(feature = "dx-machine-cache")]
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
#[cfg(all(feature = "dx-machine-cache", feature = "dx-machine-cache-mmap"))]
use serializer::machine::open_typed_machine_cache;
#[cfg(feature = "dx-machine-cache")]
use serializer::machine::{
  access_typed_machine_cache, paths_for_project_cache, source_fingerprint,
  write_typed_machine_cache, MachineCacheCodec, MachineCacheError, MachineCacheKind,
  MachineCacheSchema, MachineCacheSource, MachineCacheWriteOptions,
};

#[cfg(feature = "dx-machine-cache")]
use crate::helpers::machine_cache_io::{
  machine_cache_file_is_candidate, read_machine_file_bounded,
};
use crate::interface::rust::get_workspace_dir;

#[derive(Clone, Deserialize)]
pub struct CargoLockPackage {
  pub name: String,
  pub version: String,
  pub source: Option<String>,
}

#[derive(Deserialize)]
pub struct CargoLock {
  pub package: Vec<CargoLockPackage>,
}

#[derive(Clone, Deserialize)]
pub struct CargoManifestDependencyPackage {
  pub version: Option<String>,
  pub git: Option<String>,
  pub branch: Option<String>,
  pub rev: Option<String>,
  pub path: Option<PathBuf>,
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
pub enum CargoManifestDependency {
  Version(String),
  Package(CargoManifestDependencyPackage),
}

#[derive(Deserialize)]
pub struct CargoManifestPackage {
  pub version: String,
}

#[derive(Deserialize)]
pub struct CargoManifest {
  pub package: CargoManifestPackage,
  pub dependencies: HashMap<String, CargoManifestDependency>,
}

pub fn cargo_manifest_and_lock(tauri_dir: &Path) -> (Option<CargoManifest>, Option<CargoLock>) {
  #[cfg(feature = "dx-machine-cache")]
  if cargo_metadata_machine_cache_enabled() {
    if let Some(cached) = read_cargo_metadata_machine_cache(tauri_dir) {
      return cached;
    }
  }

  let parsed = cargo_manifest_and_lock_from_source(tauri_dir);
  #[cfg(feature = "dx-machine-cache")]
  if cargo_metadata_machine_cache_enabled()
    && tauri_utils::config::machine_cache::machine_cache_writes_enabled()
  {
    let _ = write_cargo_metadata_machine_cache(tauri_dir, &parsed.0, &parsed.1);
  }

  parsed
}

fn cargo_manifest_and_lock_from_source(
  tauri_dir: &Path,
) -> (Option<CargoManifest>, Option<CargoLock>) {
  let manifest: Option<CargoManifest> = fs::read_to_string(tauri_dir.join("Cargo.toml"))
    .ok()
    .and_then(|manifest_contents| toml::from_str(&manifest_contents).ok());

  let lock: Option<CargoLock> = get_workspace_dir(tauri_dir)
    .ok()
    .and_then(|p| fs::read_to_string(p.join("Cargo.lock")).ok())
    .and_then(|s| toml::from_str(&s).ok());

  (manifest, lock)
}

#[cfg(feature = "dx-machine-cache")]
const TAURI_DX_MACHINE_CACHE_ENV: &str = "TAURI_DX_MACHINE_CACHE";
#[cfg(feature = "dx-machine-cache")]
const CARGO_METADATA_MACHINE_CACHE_SCHEMA_VERSION: u32 = 2;
#[cfg(feature = "dx-machine-cache")]
const CARGO_METADATA_MACHINE_CACHE_MAX_BYTES: u64 = 16 * 1024 * 1024;
#[cfg(feature = "dx-machine-cache")]
const CARGO_METADATA_MAX_CACHED_SOURCE_PATHS: usize = 8192;

#[cfg(feature = "dx-machine-cache")]
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct CargoMetadataMachineCache {
  tauri_dir: String,
  workspace_dir: Option<String>,
  lock_path: Option<String>,
  sources: Vec<CargoMetadataSourceFingerprint>,
  manifest: Option<CargoManifestMachine>,
  lock: Option<CargoLockMachine>,
}

#[cfg(feature = "dx-machine-cache")]
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct CargoMetadataSourceFingerprint {
  path: String,
  present: bool,
  bytes: u64,
  modified_unix_ms: Option<u64>,
  blake3: [u8; 32],
}

#[cfg(feature = "dx-machine-cache")]
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct CargoManifestMachine {
  package_version: String,
  dependencies: Vec<CargoManifestDependencyEntryMachine>,
}

#[cfg(feature = "dx-machine-cache")]
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct CargoManifestDependencyEntryMachine {
  name: String,
  dependency: CargoManifestDependencyMachine,
}

#[cfg(feature = "dx-machine-cache")]
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
enum CargoManifestDependencyMachine {
  Version(String),
  Package {
    version: Option<String>,
    git: Option<String>,
    branch: Option<String>,
    rev: Option<String>,
    path: Option<String>,
  },
}

#[cfg(feature = "dx-machine-cache")]
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct CargoLockMachine {
  packages: Vec<CargoLockPackageMachine>,
}

#[cfg(feature = "dx-machine-cache")]
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct CargoLockPackageMachine {
  name: String,
  version: String,
  source: Option<String>,
}

#[cfg(feature = "dx-machine-cache")]
fn cargo_metadata_machine_cache_enabled() -> bool {
  std::env::var(TAURI_DX_MACHINE_CACHE_ENV)
    .map(|value| {
      matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
      )
    })
    .unwrap_or(false)
}

#[cfg(feature = "dx-machine-cache")]
fn read_cargo_metadata_machine_cache(
  tauri_dir: &Path,
) -> Option<(Option<CargoManifest>, Option<CargoLock>)> {
  let paths = cargo_metadata_machine_cache_paths(tauri_dir).ok()?;
  if !machine_cache_file_is_candidate(&paths.machine, CARGO_METADATA_MACHINE_CACHE_MAX_BYTES) {
    return None;
  }
  let source = source_fingerprint(&paths.source).ok()?;

  #[cfg(feature = "dx-machine-cache-mmap")]
  if let Ok(mapped) = open_typed_machine_cache::<CargoMetadataMachineCache>(
    &paths,
    &source,
    cargo_metadata_machine_cache_schema(),
  ) {
    return cargo_metadata_from_archive(mapped.archived(), tauri_dir, &source);
  }

  let bytes = read_machine_file_bounded(&paths.machine, CARGO_METADATA_MACHINE_CACHE_MAX_BYTES)?;
  let archived = access_typed_machine_cache::<CargoMetadataMachineCache>(
    &bytes,
    &source,
    cargo_metadata_machine_cache_schema(),
  )
  .ok()?;
  cargo_metadata_from_archive(archived, tauri_dir, &source)
}

#[cfg(feature = "dx-machine-cache")]
fn write_cargo_metadata_machine_cache(
  tauri_dir: &Path,
  manifest: &Option<CargoManifest>,
  lock: &Option<CargoLock>,
) -> Result<PathBuf, MachineCacheError> {
  let paths = cargo_metadata_machine_cache_paths(tauri_dir)?;
  let source = source_fingerprint(&paths.source)?;
  let payload =
    CargoMetadataMachineCache::from_current_sources(tauri_dir, manifest, lock, &source)?;
  let receipt = write_typed_machine_cache(
    &payload,
    &source,
    &paths,
    cargo_metadata_machine_cache_schema(),
    MachineCacheWriteOptions {
      codec: MachineCacheCodec::None,
    },
  )?;
  Ok(receipt.machine)
}

#[cfg(feature = "dx-machine-cache")]
fn cargo_metadata_machine_cache_paths(
  tauri_dir: &Path,
) -> Result<serializer::machine::MachineCachePaths, MachineCacheError> {
  paths_for_project_cache(
    tauri_dir,
    "tauri",
    "cargo-package-metadata",
    &tauri_dir.join("Cargo.toml"),
  )
}

#[cfg(feature = "dx-machine-cache")]
fn cargo_metadata_machine_cache_schema() -> MachineCacheSchema {
  MachineCacheSchema {
    name: "dx.tauri.cli.cargo_package_metadata",
    version: CARGO_METADATA_MACHINE_CACHE_SCHEMA_VERSION,
    kind: MachineCacheKind::Config,
  }
}

#[cfg(feature = "dx-machine-cache")]
fn cargo_metadata_from_archive(
  archived: &ArchivedCargoMetadataMachineCache,
  tauri_dir: &Path,
  app_manifest_source: &MachineCacheSource,
) -> Option<(Option<CargoManifest>, Option<CargoLock>)> {
  if archived.tauri_dir.as_str() != normalized_path(tauri_dir)
    || !current_cargo_sources_match_archived(archived, tauri_dir, app_manifest_source)
  {
    return None;
  }
  Some((
    archived.manifest.as_ref().map(cargo_manifest_from_archive),
    archived.lock.as_ref().map(cargo_lock_from_archive),
  ))
}

#[cfg(feature = "dx-machine-cache")]
fn current_cargo_sources_match_archived(
  cache: &ArchivedCargoMetadataMachineCache,
  tauri_dir: &Path,
  app_manifest_source: &MachineCacheSource,
) -> bool {
  let Ok(snapshot) =
    CargoMetadataSourceSnapshot::from_archived_cached_paths(cache, tauri_dir, app_manifest_source)
  else {
    return false;
  };
  if snapshot.workspace_dir != archived_optional_string(&cache.workspace_dir)
    || snapshot.lock_path != archived_optional_string(&cache.lock_path)
    || snapshot.sources.len() != cache.sources.len()
  {
    return false;
  }
  let current_by_path = snapshot
    .sources
    .iter()
    .map(|source| (source.path.as_str(), source))
    .collect::<HashMap<_, _>>();
  if current_by_path.len() != snapshot.sources.len() {
    return false;
  }
  let mut seen_paths = HashSet::with_capacity(cache.sources.len());
  cache.sources.iter().all(|expected| {
    let path = expected.path.as_str();
    seen_paths.insert(path)
      && current_by_path
        .get(path)
        .is_some_and(|current| current.matches_archived(expected))
  })
}

#[cfg(feature = "dx-machine-cache")]
struct CargoMetadataSourceSnapshot {
  workspace_dir: Option<String>,
  lock_path: Option<String>,
  sources: Vec<CargoMetadataSourceFingerprint>,
}

#[cfg(feature = "dx-machine-cache")]
impl CargoMetadataSourceSnapshot {
  fn from_archived_cached_paths(
    cache: &ArchivedCargoMetadataMachineCache,
    tauri_dir: &Path,
    app_manifest_source: &MachineCacheSource,
  ) -> Result<Self, MachineCacheError> {
    if cache.sources.len() > CARGO_METADATA_MAX_CACHED_SOURCE_PATHS {
      return Err(MachineCacheError::InvalidCachePath(
        "cargo metadata cache records too many source paths".into(),
      ));
    }
    let manifest_path = tauri_dir.join("Cargo.toml");
    if normalized_path(&manifest_path) != normalized_path(&app_manifest_source.path) {
      return Err(MachineCacheError::InvalidCachePath(
        "cargo metadata cache source path does not match app Cargo.toml".into(),
      ));
    }
    let workspace_dir = cache
      .workspace_dir
      .as_ref()
      .map(|workspace_dir| PathBuf::from(workspace_dir.as_str()));
    if let Some(workspace_dir) = &workspace_dir {
      if !cached_workspace_dir_allowed(workspace_dir, tauri_dir) {
        return Err(MachineCacheError::InvalidCachePath(
          "cargo metadata cache workspace_dir is outside the app workspace ancestry".into(),
        ));
      }
    }
    let workspace_manifest_path = workspace_dir
      .as_ref()
      .map(|workspace_dir| workspace_dir.join("Cargo.toml"));
    let cached_lock_path = cache
      .lock_path
      .as_ref()
      .map(|lock_path| PathBuf::from(lock_path.as_str()));
    let base_dir = workspace_dir.as_deref().unwrap_or(tauri_dir);
    if let Some(lock_path) = &cached_lock_path {
      if !cached_lock_path_allowed(lock_path, base_dir) {
        return Err(MachineCacheError::InvalidCachePath(
          "cargo metadata cache lock_path is outside the expected Cargo.lock location".into(),
        ));
      }
    }
    let lock_path = cached_lock_path.or_else(|| {
      let lock_path = base_dir.join("Cargo.lock");
      lock_path.exists().then_some(lock_path)
    });

    let mut sources = SourceSnapshotBuilder::new();
    push_unique_source_snapshot(&mut sources, app_manifest_source);
    if let Some(workspace_manifest_path) = workspace_manifest_path {
      push_unique_source(&mut sources, &workspace_manifest_path)?;
    }
    if let Some(lock_path) = &lock_path {
      push_unique_source(&mut sources, lock_path)?;
    }
    push_ancestor_manifest_snapshots(&mut sources, tauri_dir)?;

    Ok(Self {
      workspace_dir: archived_optional_string(&cache.workspace_dir),
      lock_path: lock_path.as_deref().map(normalized_path),
      sources: sources.into_sources(),
    })
  }

  #[cfg(test)]
  fn current(tauri_dir: &Path) -> Result<Self, MachineCacheError> {
    let manifest_path = tauri_dir.join("Cargo.toml");
    let app_manifest_source = source_fingerprint(&manifest_path)?;
    Self::current_with_app_manifest_source(tauri_dir, &app_manifest_source)
  }

  fn current_with_app_manifest_source(
    tauri_dir: &Path,
    app_manifest_source: &MachineCacheSource,
  ) -> Result<Self, MachineCacheError> {
    let manifest_path = tauri_dir.join("Cargo.toml");
    if normalized_path(&manifest_path) != normalized_path(&app_manifest_source.path) {
      return Err(MachineCacheError::InvalidCachePath(
        "cargo metadata cache source path does not match app Cargo.toml".into(),
      ));
    }
    let workspace_dir = get_workspace_dir(tauri_dir).ok();
    let lock_path = workspace_dir
      .as_ref()
      .map(|workspace_dir| workspace_dir.join("Cargo.lock"))
      .filter(|path| path.exists())
      .or_else(|| {
        if workspace_dir.is_none() {
          let lock_path = tauri_dir.join("Cargo.lock");
          lock_path.exists().then_some(lock_path)
        } else {
          None
        }
      });
    let workspace_manifest_path = workspace_dir
      .as_ref()
      .map(|workspace_dir| workspace_dir.join("Cargo.toml"))
      .filter(|path| path.exists());

    let mut sources = SourceSnapshotBuilder::new();
    push_unique_source_snapshot(&mut sources, app_manifest_source);
    if let Some(workspace_manifest_path) = workspace_manifest_path {
      push_unique_source(&mut sources, &workspace_manifest_path)?;
    }
    if let Some(lock_path) = &lock_path {
      push_unique_source(&mut sources, lock_path)?;
    }
    push_ancestor_manifest_snapshots(&mut sources, tauri_dir)?;

    Ok(Self {
      workspace_dir: workspace_dir.as_deref().map(normalized_path),
      lock_path: lock_path.as_deref().map(normalized_path),
      sources: sources.into_sources(),
    })
  }
}

#[cfg(feature = "dx-machine-cache")]
fn push_unique_source(
  sources: &mut SourceSnapshotBuilder,
  path: &Path,
) -> Result<(), MachineCacheError> {
  sources.push_current_path(path)
}

#[cfg(feature = "dx-machine-cache")]
fn push_ancestor_manifest_snapshots(
  sources: &mut SourceSnapshotBuilder,
  tauri_dir: &Path,
) -> Result<(), MachineCacheError> {
  for ancestor in tauri_dir.ancestors().skip(1) {
    push_unique_source(sources, &ancestor.join("Cargo.toml"))?;
  }
  Ok(())
}

#[cfg(feature = "dx-machine-cache")]
fn normalized_path(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

#[cfg(feature = "dx-machine-cache")]
fn cached_workspace_dir_allowed(workspace_dir: &Path, tauri_dir: &Path) -> bool {
  cached_source_path_looks_safe(workspace_dir) && tauri_dir.starts_with(workspace_dir)
}

#[cfg(feature = "dx-machine-cache")]
fn cached_lock_path_allowed(lock_path: &Path, base_dir: &Path) -> bool {
  cached_source_path_looks_safe(lock_path)
    && lock_path.file_name().and_then(|name| name.to_str()) == Some("Cargo.lock")
    && normalized_path(lock_path) == normalized_path(&base_dir.join("Cargo.lock"))
}

#[cfg(feature = "dx-machine-cache")]
fn cached_source_path_looks_safe(path: &Path) -> bool {
  path.is_absolute()
    && !path
      .components()
      .any(|component| matches!(component, std::path::Component::ParentDir))
}

#[cfg(feature = "dx-machine-cache")]
impl CargoMetadataMachineCache {
  fn from_current_sources(
    tauri_dir: &Path,
    manifest: &Option<CargoManifest>,
    lock: &Option<CargoLock>,
    app_manifest_source: &MachineCacheSource,
  ) -> Result<Self, MachineCacheError> {
    let snapshot = CargoMetadataSourceSnapshot::current_with_app_manifest_source(
      tauri_dir,
      app_manifest_source,
    )?;
    Ok(Self {
      tauri_dir: normalized_path(tauri_dir),
      workspace_dir: snapshot.workspace_dir,
      lock_path: snapshot.lock_path,
      sources: snapshot.sources,
      manifest: manifest.as_ref().map(Into::into),
      lock: lock.as_ref().map(Into::into),
    })
  }
}

#[cfg(feature = "dx-machine-cache")]
fn push_unique_source_snapshot(sources: &mut SourceSnapshotBuilder, source: &MachineCacheSource) {
  sources.push_source(source);
}

#[cfg(feature = "dx-machine-cache")]
struct SourceSnapshotBuilder {
  sources: Vec<CargoMetadataSourceFingerprint>,
  seen_paths: HashSet<String>,
}

#[cfg(feature = "dx-machine-cache")]
impl SourceSnapshotBuilder {
  fn new() -> Self {
    Self {
      sources: Vec::new(),
      seen_paths: HashSet::new(),
    }
  }

  fn push_current_path(&mut self, path: &Path) -> Result<(), MachineCacheError> {
    if !self.seen_paths.insert(normalized_path(path)) {
      return Ok(());
    }
    self
      .sources
      .push(CargoMetadataSourceFingerprint::current(path)?);
    Ok(())
  }

  fn push_source(&mut self, source: &MachineCacheSource) {
    if !self.seen_paths.insert(normalized_path(&source.path)) {
      return;
    }
    self
      .sources
      .push(CargoMetadataSourceFingerprint::from_source(source));
  }

  fn into_sources(self) -> Vec<CargoMetadataSourceFingerprint> {
    self.sources
  }
}

#[cfg(feature = "dx-machine-cache")]
impl CargoMetadataSourceFingerprint {
  fn current(path: &Path) -> Result<Self, MachineCacheError> {
    if !path.exists() {
      return Ok(Self {
        path: normalized_path(path),
        present: false,
        bytes: 0,
        modified_unix_ms: None,
        blake3: [0; 32],
      });
    }
    Ok(Self::from_source(&source_fingerprint(path)?))
  }

  fn from_source(source: &MachineCacheSource) -> Self {
    Self {
      path: normalized_path(&source.path),
      present: true,
      bytes: source.bytes,
      modified_unix_ms: source.modified_unix_ms,
      blake3: source.blake3,
    }
  }

  fn matches_archived(&self, expected: &ArchivedCargoMetadataSourceFingerprint) -> bool {
    self.present == expected.present
      && self.bytes == expected.bytes.to_native()
      && self
        .blake3
        .iter()
        .zip(expected.blake3.iter())
        .all(|(current, expected)| current == expected)
  }
}

#[cfg(feature = "dx-machine-cache")]
fn archived_optional_string(
  value: &rkyv::option::ArchivedOption<rkyv::string::ArchivedString>,
) -> Option<String> {
  value.as_ref().map(|value| value.as_str().to_string())
}

#[cfg(feature = "dx-machine-cache")]
impl From<&CargoManifest> for CargoManifestMachine {
  fn from(manifest: &CargoManifest) -> Self {
    let mut dependencies = manifest
      .dependencies
      .iter()
      .map(|(name, dependency)| CargoManifestDependencyEntryMachine {
        name: name.clone(),
        dependency: dependency.into(),
      })
      .collect::<Vec<_>>();
    dependencies.sort_by(|left, right| left.name.cmp(&right.name));
    Self {
      package_version: manifest.package.version.clone(),
      dependencies,
    }
  }
}

#[cfg(feature = "dx-machine-cache")]
fn cargo_manifest_from_archive(manifest: &ArchivedCargoManifestMachine) -> CargoManifest {
  CargoManifest {
    package: CargoManifestPackage {
      version: manifest.package_version.as_str().to_string(),
    },
    dependencies: manifest
      .dependencies
      .iter()
      .map(|entry| {
        (
          entry.name.as_str().to_string(),
          cargo_manifest_dependency_from_archive(&entry.dependency),
        )
      })
      .collect(),
  }
}

#[cfg(feature = "dx-machine-cache")]
impl From<&CargoManifestDependency> for CargoManifestDependencyMachine {
  fn from(dependency: &CargoManifestDependency) -> Self {
    match dependency {
      CargoManifestDependency::Version(version) => Self::Version(version.clone()),
      CargoManifestDependency::Package(package) => Self::Package {
        version: package.version.clone(),
        git: package.git.clone(),
        branch: package.branch.clone(),
        rev: package.rev.clone(),
        path: package.path.as_ref().map(|path| normalized_path(path)),
      },
    }
  }
}

#[cfg(feature = "dx-machine-cache")]
fn cargo_manifest_dependency_from_archive(
  dependency: &ArchivedCargoManifestDependencyMachine,
) -> CargoManifestDependency {
  match dependency {
    ArchivedCargoManifestDependencyMachine::Version(version) => {
      CargoManifestDependency::Version(version.as_str().to_string())
    }
    ArchivedCargoManifestDependencyMachine::Package {
      version,
      git,
      branch,
      rev,
      path,
    } => CargoManifestDependency::Package(CargoManifestDependencyPackage {
      version: archived_optional_string(version),
      git: archived_optional_string(git),
      branch: archived_optional_string(branch),
      rev: archived_optional_string(rev),
      path: archived_optional_string(path).map(PathBuf::from),
    }),
  }
}

#[cfg(feature = "dx-machine-cache")]
impl From<&CargoLock> for CargoLockMachine {
  fn from(lock: &CargoLock) -> Self {
    Self {
      packages: lock.package.iter().map(Into::into).collect(),
    }
  }
}

#[cfg(feature = "dx-machine-cache")]
fn cargo_lock_from_archive(lock: &ArchivedCargoLockMachine) -> CargoLock {
  CargoLock {
    package: lock
      .packages
      .iter()
      .map(cargo_lock_package_from_archive)
      .collect(),
  }
}

#[cfg(feature = "dx-machine-cache")]
impl From<&CargoLockPackage> for CargoLockPackageMachine {
  fn from(package: &CargoLockPackage) -> Self {
    Self {
      name: package.name.clone(),
      version: package.version.clone(),
      source: package.source.clone(),
    }
  }
}

#[cfg(feature = "dx-machine-cache")]
fn cargo_lock_package_from_archive(package: &ArchivedCargoLockPackageMachine) -> CargoLockPackage {
  CargoLockPackage {
    name: package.name.as_str().to_string(),
    version: package.version.as_str().to_string(),
    source: package
      .source
      .as_ref()
      .map(|source| source.as_str().to_string()),
  }
}

#[derive(Default)]
pub struct CrateVersion {
  pub version: Option<String>,
  pub git: Option<String>,
  pub git_branch: Option<String>,
  pub git_rev: Option<String>,
  pub path: Option<PathBuf>,
  pub lock_version: Option<String>,
}

impl CrateVersion {
  pub fn has_version(&self) -> bool {
    self.version.is_some() || self.git.is_some() || self.path.is_some()
  }
}

impl std::fmt::Display for CrateVersion {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if let Some(g) = &self.git {
      if let Some(version) = &self.version {
        write!(f, "{g} ({version})")?;
      } else {
        write!(f, "git:{g}")?;
        if let Some(branch) = &self.git_branch {
          write!(f, "&branch={branch}")?;
        } else if let Some(rev) = &self.git_rev {
          write!(f, "#rev={rev}")?;
        }
      }
    } else if let Some(p) = &self.path {
      write!(f, "path:{}", p.display())?;
      if let Some(version) = &self.version {
        write!(f, " ({version})")?;
      }
    } else if let Some(version) = &self.version {
      write!(f, "{version}")?;
    } else {
      return write!(f, "No version detected");
    }

    if let Some(lock_version) = &self.lock_version {
      write!(f, " ({lock_version})")?;
    }

    Ok(())
  }
}

// Reference: https://github.com/rust-lang/crates.io/blob/98c83c8231cbcd15d6b8f06d80a00ad462f71585/src/views.rs#L274
#[derive(serde::Deserialize)]
struct CrateMetadata {
  /// The "default" version of this crate.
  ///
  /// This version will be displayed by default on the crate's page.
  pub default_version: Option<String>,
}

// Reference: https://github.com/rust-lang/crates.io/blob/98c83c8231cbcd15d6b8f06d80a00ad462f71585/src/controllers/krate/metadata.rs#L44
#[derive(serde::Deserialize)]
struct CrateIoGetResponse {
  /// The crate metadata.
  #[serde(rename = "crate")]
  krate: CrateMetadata,
}

pub fn crate_latest_version(name: &str) -> Option<String> {
  // Reference: https://github.com/rust-lang/crates.io/blob/98c83c8231cbcd15d6b8f06d80a00ad462f71585/src/controllers/krate/metadata.rs#L88
  let url = format!("https://crates.io/api/v1/crates/{name}?include");
  let mut response = super::http::get(&url).ok()?;
  let metadata: CrateIoGetResponse =
    serde_json::from_reader(response.body_mut().as_reader()).unwrap();
  metadata.krate.default_version
}

pub fn crate_version(
  tauri_dir: &Path,
  manifest: Option<&CargoManifest>,
  lock: Option<&CargoLock>,
  name: &str,
) -> CrateVersion {
  let mut version = CrateVersion::default();

  let crate_lock_packages: Vec<CargoLockPackage> = lock
    .as_ref()
    .map(|lock| {
      lock
        .package
        .iter()
        .filter(|p| p.name == name)
        .cloned()
        .collect()
    })
    .unwrap_or_default();

  if crate_lock_packages.len() == 1 {
    let crate_lock_package = crate_lock_packages.first().unwrap();
    if let Some(s) = crate_lock_package
      .source
      .as_ref()
      .filter(|s| s.starts_with("git"))
    {
      version.git = Some(s.clone());
    }

    version.version = Some(crate_lock_package.version.clone());
  } else {
    if let Some(dep) = manifest.and_then(|m| m.dependencies.get(name).cloned()) {
      match dep {
        CargoManifestDependency::Version(v) => version.version = Some(v),
        CargoManifestDependency::Package(p) => {
          if let Some(v) = p.version {
            version.version = Some(v);
          } else if let Some(p) = p.path {
            let manifest_path = tauri_dir.join(&p).join("Cargo.toml");
            let v = fs::read_to_string(manifest_path)
              .ok()
              .and_then(|m| toml::from_str::<CargoManifest>(&m).ok())
              .map(|m| m.package.version);
            version.version = v;
            version.path = Some(p);
          } else if let Some(g) = p.git {
            version.git = Some(g);
            version.git_branch = p.branch;
            version.git_rev = p.rev;
          }
        }
      }
    }

    if lock.is_some() && crate_lock_packages.is_empty() {
      let lock_version = crate_lock_packages
        .iter()
        .map(|p| p.version.clone())
        .collect::<Vec<String>>()
        .join(", ");

      if !lock_version.is_empty() {
        version.lock_version = Some(lock_version);
      }
    }
  }

  version
}

#[cfg(all(test, feature = "dx-machine-cache"))]
#[path = "cargo_manifest_machine_cache_benchmark_tests.rs"]
mod cargo_manifest_machine_cache_benchmark_tests;

#[cfg(all(test, feature = "dx-machine-cache"))]
mod tests {
  use super::*;
  use std::sync::{Mutex, MutexGuard, OnceLock};

  const TAURI_DX_MACHINE_CACHE_ENV: &str = "TAURI_DX_MACHINE_CACHE";
  const PATH_ENV: &str = "PATH";
  const WINDOWS_PATH_ENV: &str = "Path";
  static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

  struct EnvGuard {
    vars: Vec<(&'static str, Option<String>)>,
    _lock: MutexGuard<'static, ()>,
  }

  impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
      Self::set_many(&[(key, Some(value))])
    }

    fn remove(key: &'static str) -> Self {
      Self::set_many(&[(key, None)])
    }

    fn set_many(vars: &[(&'static str, Option<&str>)]) -> Self {
      let lock = ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
      let previous = vars
        .iter()
        .map(|(key, _)| (*key, std::env::var(key).ok()))
        .collect::<Vec<_>>();
      for (key, value) in vars {
        if let Some(value) = value {
          std::env::set_var(key, value);
        } else {
          std::env::remove_var(key);
        }
      }
      Self {
        vars: previous,
        _lock: lock,
      }
    }
  }

  impl Drop for EnvGuard {
    fn drop(&mut self) {
      for (key, previous) in self.vars.iter().rev() {
        if let Some(previous) = previous {
          std::env::set_var(key, previous);
        } else {
          std::env::remove_var(key);
        }
      }
    }
  }

  #[test]
  fn dx_cargo_metadata_cache_default_off_writes_no_machine() {
    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
    let project = temp_cargo_project("default-off", "1.2.3", None);

    let (manifest, lock) = cargo_manifest_and_lock(project.path());

    assert_eq!(manifest.unwrap().package.version, "1.2.3");
    assert!(lock.is_none());
    assert!(!cargo_metadata_machine_path(project.path()).exists());
  }

  #[test]
  fn dx_cargo_metadata_cache_writes_machine_when_env_enabled() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_cargo_project("write-cache", "1.2.3", None);

    let (manifest, lock) = cargo_manifest_and_lock(project.path());

    assert_eq!(manifest.unwrap().package.version, "1.2.3");
    assert!(lock.is_none());
    assert!(cargo_metadata_machine_path(project.path()).exists());

    let (cached_manifest, cached_lock) =
      read_cargo_metadata_machine_cache(project.path()).expect("read cargo metadata machine cache");

    assert_eq!(cached_manifest.unwrap().package.version, "1.2.3");
    assert!(cached_lock.is_none());
  }

  #[test]
  fn dx_cargo_metadata_cache_default_off_ignores_existing_machine_payload() {
    let project = temp_cargo_project("default-off-existing-cache", "1.2.3", None);
    {
      let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
      let (manifest, _) = cargo_manifest_and_lock(project.path());

      assert_eq!(manifest.unwrap().package.version, "1.2.3");
      assert!(cargo_metadata_machine_path(project.path()).exists());
    }

    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
    write_cargo_toml(project.path(), "2.0.0");

    let (manifest, _) = cargo_manifest_and_lock(project.path());

    assert_eq!(manifest.unwrap().package.version, "2.0.0");
  }

  #[test]
  fn dx_cargo_metadata_cache_hit_does_not_require_cargo_metadata() {
    let project = temp_workspace_project("cache-hit-without-cargo", "1.2.3");
    let app_dir = project.path().join("app");
    let fake_path_dir = project.path().join("fake-path");
    fs::create_dir_all(&fake_path_dir).expect("fake PATH directory");
    fs::write(
      fake_path_dir.join("cargo.cmd"),
      "@echo off\r\necho cargo metadata should not run on cache hits 1>&2\r\nexit /b 57\r\n",
    )
    .expect("write fake cargo");

    {
      let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
      let snapshot = CargoMetadataSourceSnapshot::current(&app_dir).expect("workspace snapshot");
      assert_eq!(
        snapshot.workspace_dir.as_deref(),
        Some(normalized_path(project.path()).as_str())
      );
      let (manifest, lock) = cargo_manifest_and_lock(&app_dir);

      assert_eq!(manifest.unwrap().package.version, "1.2.3");
      assert!(lock.is_none());
      assert!(cargo_metadata_machine_path(&app_dir).exists());
    }

    let fake_path = fake_path_dir.to_string_lossy().to_string();
    let _env = EnvGuard::set_many(&[
      (TAURI_DX_MACHINE_CACHE_ENV, Some("1")),
      (PATH_ENV, Some(fake_path.as_str())),
      (WINDOWS_PATH_ENV, Some(fake_path.as_str())),
    ]);

    let (cached_manifest, cached_lock) =
      read_cargo_metadata_machine_cache(&app_dir).expect("cache hit without cargo metadata");

    assert_eq!(cached_manifest.unwrap().package.version, "1.2.3");
    assert!(cached_lock.is_none());
  }

  #[test]
  fn dx_cargo_metadata_cache_write_env_zero_still_reads_hit() {
    let project = temp_cargo_project("read-only-hit", "1.2.3", None);
    {
      let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
      let (manifest, lock) = cargo_manifest_and_lock(project.path());
      assert_eq!(manifest.unwrap().package.version, "1.2.3");
      assert!(lock.is_none());
    }

    let machine_path = cargo_metadata_machine_path(project.path());
    let machine_before = fs::read(&machine_path).expect("read primed machine cache");
    let _env = EnvGuard::set_many(&[
      (TAURI_DX_MACHINE_CACHE_ENV, Some("1")),
      (
        tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
        Some("0"),
      ),
    ]);

    let (manifest, lock) = cargo_manifest_and_lock(project.path());

    assert_eq!(manifest.unwrap().package.version, "1.2.3");
    assert!(lock.is_none());
    assert_eq!(
      machine_before,
      fs::read(&machine_path).expect("read machine cache after hit")
    );
  }

  #[test]
  fn dx_cargo_metadata_cache_write_env_zero_does_not_write_on_miss() {
    let project = temp_cargo_project("read-only-miss", "1.2.3", None);
    let _env = EnvGuard::set_many(&[
      (TAURI_DX_MACHINE_CACHE_ENV, Some("1")),
      (
        tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
        Some("0"),
      ),
    ]);

    let (manifest, lock) = cargo_manifest_and_lock(project.path());

    assert_eq!(manifest.unwrap().package.version, "1.2.3");
    assert!(lock.is_none());
    assert!(!cargo_metadata_machine_path(project.path()).exists());
  }

  #[test]
  fn dx_cargo_metadata_cache_reuses_envelope_source_for_app_manifest() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_workspace_project("reuse-envelope-source", "1.2.3");
    let app_dir = project.path().join("app");
    let app_manifest_path = app_dir.join("Cargo.toml");

    let (manifest, lock) = cargo_manifest_and_lock(&app_dir);
    let envelope_source = source_fingerprint(&app_manifest_path).expect("app manifest source");
    let cache =
      CargoMetadataMachineCache::from_current_sources(&app_dir, &manifest, &lock, &envelope_source)
        .expect("cargo metadata cache payload");
    let machine_path = write_cargo_metadata_machine_payload(&app_dir, &cache, &envelope_source);
    let machine_bytes = fs::read(machine_path).expect("read cargo metadata machine payload");
    let archived = archived_cargo_metadata_cache(&machine_bytes, &envelope_source);

    write_cargo_toml(&app_dir, "9.9.9");
    assert!(current_cargo_sources_match_archived(
      archived,
      &app_dir,
      &envelope_source
    ));

    let mut stale_envelope_source = envelope_source.clone();
    stale_envelope_source.blake3[0] ^= 1;
    assert!(!current_cargo_sources_match_archived(
      archived,
      &app_dir,
      &stale_envelope_source
    ));
  }

  #[test]
  fn dx_cargo_metadata_cache_rejects_duplicate_cached_sources() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_workspace_project("duplicate-cached-sources", "1.2.3");
    let app_dir = project.path().join("app");
    let app_manifest_path = app_dir.join("Cargo.toml");

    let (manifest, lock) = cargo_manifest_and_lock(&app_dir);
    let envelope_source = source_fingerprint(&app_manifest_path).expect("app manifest source");
    let mut cache =
      CargoMetadataMachineCache::from_current_sources(&app_dir, &manifest, &lock, &envelope_source)
        .expect("cargo metadata cache payload");
    assert!(
      cache.sources.len() >= 2,
      "test fixture must have more than one source"
    );

    cache.sources[1] = cache.sources[0].clone();
    write_cargo_metadata_machine_payload(&app_dir, &cache, &envelope_source);

    assert!(read_cargo_metadata_machine_cache(&app_dir).is_none());
  }

  #[test]
  fn dx_cargo_metadata_cache_rejects_archived_workspace_outside_ancestry() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_workspace_project("workspace-outside-ancestry", "1.2.3");
    let app_dir = project.path().join("app");
    let app_manifest_path = app_dir.join("Cargo.toml");

    let (manifest, lock) = cargo_manifest_and_lock(&app_dir);
    let envelope_source = source_fingerprint(&app_manifest_path).expect("app manifest source");
    let mut cache =
      CargoMetadataMachineCache::from_current_sources(&app_dir, &manifest, &lock, &envelope_source)
        .expect("cargo metadata cache payload");
    cache.workspace_dir = Some(normalized_path(&project.path().join("outside-workspace")));
    write_cargo_metadata_machine_payload(&app_dir, &cache, &envelope_source);

    assert!(read_cargo_metadata_machine_cache(&app_dir).is_none());
  }

  #[test]
  fn dx_cargo_metadata_cache_rejects_archived_lock_path_outside_base() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_cargo_project("lock-path-outside-base", "1.2.3", Some("serde 1.0.203"));
    let app_manifest_path = project.path().join("Cargo.toml");

    let (manifest, lock) = cargo_manifest_and_lock(project.path());
    let envelope_source = source_fingerprint(&app_manifest_path).expect("app manifest source");
    let mut cache = CargoMetadataMachineCache::from_current_sources(
      project.path(),
      &manifest,
      &lock,
      &envelope_source,
    )
    .expect("cargo metadata cache payload");
    cache.lock_path = Some(normalized_path(
      &project.path().join("nested").join("Cargo.lock"),
    ));
    write_cargo_metadata_machine_payload(project.path(), &cache, &envelope_source);

    assert!(read_cargo_metadata_machine_cache(project.path()).is_none());
  }

  #[test]
  fn dx_cargo_metadata_cache_preserves_archived_dependencies_and_lock_source() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_cargo_project("archive-fidelity", "1.2.3", None);
    write_cargo_toml_with_dependency_shapes(project.path(), "1.2.3");
    fs::create_dir_all(project.path().join("local-plugin").join("src"))
      .expect("local dependency src dir");
    fs::write(
      project.path().join("local-plugin").join("Cargo.toml"),
      r#"[package]
name = "tauri-plugin-local"
version = "0.9.0"
edition = "2021"
"#,
    )
    .expect("write local dependency manifest");
    fs::write(
      project
        .path()
        .join("local-plugin")
        .join("src")
        .join("lib.rs"),
      "",
    )
    .expect("write local dependency lib");
    write_cargo_lock_with_source(
      project.path(),
      "serde",
      "1.0.203",
      "registry+https://github.com/rust-lang/crates.io-index",
    );

    let mut dependencies = HashMap::new();
    dependencies.insert("serde".into(), CargoManifestDependency::Version("1".into()));
    dependencies.insert(
      "tauri-plugin-local".into(),
      CargoManifestDependency::Package(CargoManifestDependencyPackage {
        version: Some("0.9".into()),
        git: Some("https://example.invalid/tauri-plugin-local.git".into()),
        branch: Some("main".into()),
        rev: Some("abc123".into()),
        path: Some(PathBuf::from("local-plugin")),
      }),
    );
    let manifest = Some(CargoManifest {
      package: CargoManifestPackage {
        version: "1.2.3".into(),
      },
      dependencies,
    });
    let lock = Some(CargoLock {
      package: vec![CargoLockPackage {
        name: "serde".into(),
        version: "1.0.203".into(),
        source: Some("registry+https://github.com/rust-lang/crates.io-index".into()),
      }],
    });
    let envelope_source =
      source_fingerprint(&project.path().join("Cargo.toml")).expect("app manifest source");
    let cache = CargoMetadataMachineCache::from_current_sources(
      project.path(),
      &manifest,
      &lock,
      &envelope_source,
    )
    .expect("cargo metadata cache payload");
    write_cargo_metadata_machine_payload(project.path(), &cache, &envelope_source);

    let (cached_manifest, cached_lock) =
      read_cargo_metadata_machine_cache(project.path()).expect("read archived cache");
    let cached_manifest = cached_manifest.expect("cached manifest");
    assert_eq!(cached_manifest.package.version, "1.2.3");
    assert!(matches!(
      cached_manifest.dependencies.get("serde"),
      Some(CargoManifestDependency::Version(version)) if version == "1"
    ));
    let plugin = cached_manifest
      .dependencies
      .get("tauri-plugin-local")
      .expect("plugin dependency");
    let CargoManifestDependency::Package(plugin) = plugin else {
      panic!("plugin dependency should keep package shape");
    };
    assert_eq!(plugin.version.as_deref(), Some("0.9"));
    assert_eq!(
      plugin.git.as_deref(),
      Some("https://example.invalid/tauri-plugin-local.git")
    );
    assert_eq!(plugin.branch.as_deref(), Some("main"));
    assert_eq!(plugin.rev.as_deref(), Some("abc123"));
    assert_eq!(plugin.path.as_deref(), Some(Path::new("local-plugin")));

    let cached_lock = cached_lock.expect("cached lock");
    assert_eq!(cached_lock.package.len(), 1);
    assert_eq!(cached_lock.package[0].name, "serde");
    assert_eq!(cached_lock.package[0].version, "1.0.203");
    assert_eq!(
      cached_lock.package[0].source.as_deref(),
      Some("registry+https://github.com/rust-lang/crates.io-index")
    );
  }

  #[test]
  fn dx_cargo_metadata_cache_rejects_changed_manifest() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_cargo_project("changed-manifest", "1.2.3", None);

    let (manifest, _) = cargo_manifest_and_lock(project.path());

    assert_eq!(manifest.unwrap().package.version, "1.2.3");
    assert!(cargo_metadata_machine_path(project.path()).exists());

    write_cargo_toml(project.path(), "2.0.0");

    assert!(read_cargo_metadata_machine_cache(project.path()).is_none());

    let (manifest, _) = cargo_manifest_and_lock(project.path());
    assert_eq!(manifest.unwrap().package.version, "2.0.0");
  }

  #[test]
  fn dx_cargo_metadata_cache_rejects_workspace_manifest_changes() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_workspace_project("changed-workspace-manifest", "1.2.3");
    let app_dir = project.path().join("app");

    let (manifest, _) = cargo_manifest_and_lock(&app_dir);

    assert_eq!(manifest.unwrap().package.version, "1.2.3");
    assert!(cargo_metadata_machine_path(&app_dir).exists());

    write_workspace_cargo_toml(project.path(), true);

    assert!(read_cargo_metadata_machine_cache(&app_dir).is_none());
  }

  #[test]
  fn dx_cargo_metadata_cache_rejects_lockfile_appearing() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_cargo_project("lockfile-appearing", "1.2.3", None);

    let (_, lock) = cargo_manifest_and_lock(project.path());

    assert!(lock.is_none());
    assert!(cargo_metadata_machine_path(project.path()).exists());

    write_cargo_lock(project.path(), "dx-cargo-metadata-fixture", "1.2.3");

    assert!(read_cargo_metadata_machine_cache(project.path()).is_none());
  }

  #[test]
  fn dx_cargo_metadata_cache_rejects_lockfile_disappearing() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_workspace_project("lockfile-disappearing", "1.2.3");
    let app_dir = project.path().join("app");
    write_cargo_lock(project.path(), "dx-cargo-metadata-fixture", "1.2.3");

    let (_, lock) = cargo_manifest_and_lock(&app_dir);

    assert!(lock.is_some());
    assert!(cargo_metadata_machine_path(&app_dir).exists());

    fs::remove_file(project.path().join("Cargo.lock")).expect("remove Cargo.lock");

    assert!(read_cargo_metadata_machine_cache(&app_dir).is_none());
  }

  #[test]
  fn dx_cargo_metadata_cache_rejects_new_parent_workspace_manifest() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let base = std::env::var_os("DX_TEST_OUTPUT_DIR")
      .map(PathBuf::from)
      .unwrap_or_else(std::env::temp_dir);
    fs::create_dir_all(&base).expect("test output base");
    let outer = tempfile::Builder::new()
      .prefix("tauri-dx-cargo-metadata-new-parent-workspace-")
      .tempdir_in(base)
      .expect("temp outer project");
    let app_dir = outer.path().join("inner").join("app");
    fs::create_dir_all(app_dir.join("src")).expect("create nested app src dir");
    fs::write(app_dir.join("src").join("lib.rs"), "").expect("write src/lib.rs");
    write_cargo_toml(&app_dir, "1.2.3");

    let (manifest, lock) = cargo_manifest_and_lock(&app_dir);
    assert_eq!(manifest.unwrap().package.version, "1.2.3");
    assert!(lock.is_none());
    assert!(cargo_metadata_machine_path(&app_dir).exists());

    fs::write(
      outer.path().join("Cargo.toml"),
      r#"[workspace]
members = ["inner/app"]
"#,
    )
    .expect("write parent workspace manifest");

    assert!(read_cargo_metadata_machine_cache(&app_dir).is_none());
  }

  #[test]
  fn dx_cargo_metadata_cache_rejects_corrupt_machine_file() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_cargo_project("corrupt-machine", "1.2.3", None);

    let (manifest, _) = cargo_manifest_and_lock(project.path());

    assert_eq!(manifest.unwrap().package.version, "1.2.3");
    fs::write(
      cargo_metadata_machine_path(project.path()),
      b"this is not a dx machine cache",
    )
    .expect("corrupt machine cache");

    assert!(read_cargo_metadata_machine_cache(project.path()).is_none());
  }

  fn temp_cargo_project(
    name: &str,
    package_version: &str,
    lock_package: Option<&str>,
  ) -> tempfile::TempDir {
    let base = std::env::var_os("DX_TEST_OUTPUT_DIR")
      .map(PathBuf::from)
      .unwrap_or_else(std::env::temp_dir);
    fs::create_dir_all(&base).expect("test output base");
    let project = tempfile::Builder::new()
      .prefix(&format!("tauri-dx-cargo-metadata-{name}-"))
      .tempdir_in(base)
      .expect("temp cargo project");
    fs::create_dir_all(project.path().join("src")).expect("src dir");
    fs::write(project.path().join("src").join("lib.rs"), "").expect("write src/lib.rs");
    write_cargo_toml(project.path(), package_version);
    if let Some(lock_package) = lock_package {
      let mut parts = lock_package.split_whitespace();
      let name = parts.next().expect("lock package name");
      let version = parts.next().expect("lock package version");
      write_cargo_lock(project.path(), name, version);
    }
    project
  }

  fn temp_workspace_project(name: &str, package_version: &str) -> tempfile::TempDir {
    let project = temp_cargo_project(name, package_version, None);
    let app_dir = project.path().join("app");
    fs::create_dir_all(&app_dir).expect("app dir");
    fs::rename(
      project.path().join("Cargo.toml"),
      app_dir.join("Cargo.toml"),
    )
    .expect("move app Cargo.toml");
    fs::rename(project.path().join("src"), app_dir.join("src")).expect("move app src");
    fs::write(
      project.path().join("Cargo.toml"),
      workspace_cargo_toml(false),
    )
    .expect("write workspace Cargo.toml");
    project
  }

  fn write_workspace_cargo_toml(project: &Path, resolver: bool) {
    fs::write(project.join("Cargo.toml"), workspace_cargo_toml(resolver))
      .expect("write workspace Cargo.toml");
  }

  fn workspace_cargo_toml(resolver: bool) -> String {
    let resolver = if resolver { "resolver = \"2\"\n" } else { "" };
    format!(
      r#"[workspace]
{resolver}members = ["app"]
"#
    )
  }

  fn write_cargo_toml(project: &Path, package_version: &str) {
    fs::write(
      project.join("Cargo.toml"),
      format!(
        r#"[package]
name = "dx-cargo-metadata-fixture"
version = "{package_version}"
edition = "2021"

[dependencies]
"#
      ),
    )
    .expect("write Cargo.toml");
  }

  fn write_cargo_toml_with_dependency_shapes(project: &Path, package_version: &str) {
    fs::write(
      project.join("Cargo.toml"),
      format!(
        r#"[package]
name = "dx-cargo-metadata-fixture"
version = "{package_version}"
edition = "2021"

[dependencies]
serde = "1"
tauri-plugin-local = {{ version = "0.9", path = "local-plugin" }}
"#
      ),
    )
    .expect("write Cargo.toml with dependencies");
  }

  fn write_cargo_lock(project: &Path, name: &str, version: &str) {
    fs::write(
      project.join("Cargo.lock"),
      format!(
        r#"version = 4

[[package]]
name = "{name}"
version = "{version}"
"#
      ),
    )
    .expect("write Cargo.lock");
  }

  fn write_cargo_lock_with_source(project: &Path, name: &str, version: &str, source: &str) {
    fs::write(
      project.join("Cargo.lock"),
      format!(
        r#"version = 4

[[package]]
name = "{name}"
version = "{version}"
source = "{source}"
"#
      ),
    )
    .expect("write Cargo.lock with source");
  }

  fn write_cargo_metadata_machine_payload(
    tauri_dir: &Path,
    payload: &CargoMetadataMachineCache,
    source: &MachineCacheSource,
  ) -> PathBuf {
    let paths = cargo_metadata_machine_cache_paths(tauri_dir).expect("cargo metadata cache paths");
    let receipt = write_typed_machine_cache(
      payload,
      source,
      &paths,
      cargo_metadata_machine_cache_schema(),
      MachineCacheWriteOptions {
        codec: MachineCacheCodec::None,
      },
    )
    .expect("write cargo metadata machine payload");
    receipt.machine
  }

  fn archived_cargo_metadata_cache<'a>(
    bytes: &'a [u8],
    source: &MachineCacheSource,
  ) -> &'a ArchivedCargoMetadataMachineCache {
    access_typed_machine_cache::<CargoMetadataMachineCache>(
      bytes,
      source,
      cargo_metadata_machine_cache_schema(),
    )
    .expect("access archived cargo metadata machine payload")
  }

  fn cargo_metadata_machine_path(project: &Path) -> PathBuf {
    project
      .join(".dx")
      .join("tauri")
      .join("cargo-package-metadata.machine")
  }
}
