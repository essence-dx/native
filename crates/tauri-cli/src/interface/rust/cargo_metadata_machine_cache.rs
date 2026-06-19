// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  collections::{HashMap, HashSet},
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

use super::{CargoMetadata, Dependency, Package};

const TAURI_DX_MACHINE_CACHE_ENV: &str = "TAURI_DX_MACHINE_CACHE";
const CARGO_METADATA_MACHINE_CACHE_SCHEMA_VERSION: u32 = 1;
const CARGO_METADATA_MACHINE_CACHE_MAX_BYTES: u64 = 64 * 1024 * 1024;
const CARGO_METADATA_MAX_CACHED_SOURCE_PATHS: usize = 8192;

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct CargoMetadataMachineCache {
  tauri_dir: String,
  cargo_target_dir_env: Option<String>,
  sources: Vec<CargoMetadataSourceFingerprint>,
  metadata: CargoMetadataMachine,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct CargoMetadataMachine {
  target_directory: String,
  workspace_root: String,
  workspace_members: Vec<String>,
  packages: Vec<PackageMachine>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct PackageMachine {
  name: String,
  id: String,
  manifest_path: String,
  dependencies: Vec<DependencyMachine>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct DependencyMachine {
  name: String,
  path: Option<String>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct CargoMetadataSourceFingerprint {
  path: String,
  present: bool,
  bytes: u64,
  modified_unix_ms: Option<u64>,
  blake3: [u8; 32],
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

pub(super) fn read(tauri_dir: &Path) -> Option<CargoMetadata> {
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

pub(super) struct CargoMetadataProjection {
  pub(super) target_directory: PathBuf,
  pub(super) in_workspace_dependency_paths: Vec<PathBuf>,
}

pub(super) fn read_projection(tauri_dir: &Path) -> Option<CargoMetadataProjection> {
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
    return cargo_metadata_projection_from_archive(mapped.archived(), tauri_dir, &source);
  }

  let bytes = read_machine_file_bounded(&paths.machine, CARGO_METADATA_MACHINE_CACHE_MAX_BYTES)?;
  let archived = access_typed_machine_cache::<CargoMetadataMachineCache>(
    &bytes,
    &source,
    cargo_metadata_machine_cache_schema(),
  )
  .ok()?;
  cargo_metadata_projection_from_archive(archived, tauri_dir, &source)
}

pub(super) fn write(
  tauri_dir: &Path,
  metadata: &CargoMetadata,
) -> Result<PathBuf, MachineCacheError> {
  let paths = cargo_metadata_machine_cache_paths(tauri_dir)?;
  let source = source_fingerprint(&paths.source)?;
  let payload = CargoMetadataMachineCache::from_current_sources(tauri_dir, metadata, &source)?;
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

#[cfg(test)]
pub(super) fn machine_path(tauri_dir: &Path) -> PathBuf {
  tauri_dir
    .join(".dx")
    .join("tauri")
    .join("cargo-metadata.machine")
}

fn cargo_metadata_from_archive(
  archived: &ArchivedCargoMetadataMachineCache,
  tauri_dir: &Path,
  app_manifest_source: &MachineCacheSource,
) -> Option<CargoMetadata> {
  let current_cargo_target_dir_env = env::var("CARGO_TARGET_DIR").ok();
  if archived.tauri_dir.as_str() != normalized_path(tauri_dir)
    || archived
      .cargo_target_dir_env
      .as_ref()
      .map(|value| value.as_str())
      != current_cargo_target_dir_env.as_deref()
    || !current_cargo_metadata_sources_match_archived(archived, tauri_dir, app_manifest_source)
  {
    return None;
  }
  Some(cargo_metadata_from_archived_metadata(&archived.metadata))
}

fn cargo_metadata_projection_from_archive(
  archived: &ArchivedCargoMetadataMachineCache,
  tauri_dir: &Path,
  app_manifest_source: &MachineCacheSource,
) -> Option<CargoMetadataProjection> {
  let current_cargo_target_dir_env = env::var("CARGO_TARGET_DIR").ok();
  if archived.tauri_dir.as_str() != normalized_path(tauri_dir)
    || archived
      .cargo_target_dir_env
      .as_ref()
      .map(|value| value.as_str())
      != current_cargo_target_dir_env.as_deref()
    || !current_cargo_metadata_sources_match_archived(archived, tauri_dir, app_manifest_source)
  {
    return None;
  }
  Some(CargoMetadataProjection {
    target_directory: PathBuf::from(archived.metadata.target_directory.as_str()),
    in_workspace_dependency_paths: in_workspace_dependency_paths_from_archived_metadata(
      tauri_dir,
      &archived.metadata,
    )?,
  })
}

fn current_cargo_metadata_sources_match_archived(
  cache: &ArchivedCargoMetadataMachineCache,
  tauri_dir: &Path,
  app_manifest_source: &MachineCacheSource,
) -> bool {
  let Ok(snapshot) =
    CargoMetadataSourceSnapshot::from_archived_cached_paths(cache, tauri_dir, app_manifest_source)
  else {
    return false;
  };
  if snapshot.sources.len() != cache.sources.len() {
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

fn current_cargo_metadata_sources_match(
  cache: &CargoMetadataMachineCache,
  tauri_dir: &Path,
  app_manifest_source: &MachineCacheSource,
) -> bool {
  let Ok(snapshot) =
    CargoMetadataSourceSnapshot::from_cached_paths(cache, tauri_dir, app_manifest_source)
  else {
    return false;
  };
  source_fingerprints_match(&cache.sources, &snapshot.sources)
}

fn source_fingerprints_match(
  expected: &[CargoMetadataSourceFingerprint],
  current: &[CargoMetadataSourceFingerprint],
) -> bool {
  if current.len() != expected.len() {
    return false;
  }
  let current_by_path = current
    .iter()
    .map(|source| (source.path.as_str(), source))
    .collect::<HashMap<_, _>>();
  let expected_by_path = expected
    .iter()
    .map(|source| (source.path.as_str(), source))
    .collect::<HashMap<_, _>>();
  if current_by_path.len() != current.len() || expected_by_path.len() != expected.len() {
    return false;
  }
  expected_by_path.iter().all(|(path, expected)| {
    current_by_path
      .get(path)
      .is_some_and(|current| current.matches(expected))
  })
}

struct CargoMetadataSourceSnapshot {
  sources: Vec<CargoMetadataSourceFingerprint>,
}

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
    ensure_app_manifest_source(tauri_dir, app_manifest_source)?;
    let workspace_root = Path::new(cache.metadata.workspace_root.as_str());
    if !cached_cargo_metadata_workspace_root_allowed(workspace_root, tauri_dir) {
      return Err(MachineCacheError::InvalidCachePath(
        "cargo metadata cache workspace_root is outside the app workspace ancestry".into(),
      ));
    }
    let allowed_sources =
      CargoMetadataAllowedSourcePaths::from_archived_metadata(tauri_dir, &cache.metadata)
        .ok_or_else(|| {
          MachineCacheError::InvalidCachePath(
            "cargo metadata cache workspace_root is outside the app workspace ancestry".into(),
          )
        })?;
    if !allowed_sources.matches_archived_cached_sources(cache) {
      return Err(MachineCacheError::InvalidCachePath(
        "cargo metadata cache source set is incomplete or unexpected".into(),
      ));
    }
    let source_exclusions = SourcePathExclusions::new(tauri_dir, &app_manifest_source.path);

    let mut sources = SourceSnapshotBuilder::new();
    push_source_snapshot(&mut sources, app_manifest_source);
    for source in cache
      .sources
      .iter()
      .filter(|source| !source_exclusions.contains(source.path.as_str()))
    {
      let source_path = Path::new(source.path.as_str());
      if !allowed_sources.contains(source_path) {
        return Err(MachineCacheError::InvalidCachePath(
          "cargo metadata cache source path is outside the expected metadata source set".into(),
        ));
      }
      push_current_path_snapshot(&mut sources, source_path)?;
    }
    Ok(Self {
      sources: sources.into_sources(),
    })
  }

  fn from_cached_paths(
    cache: &CargoMetadataMachineCache,
    tauri_dir: &Path,
    app_manifest_source: &MachineCacheSource,
  ) -> Result<Self, MachineCacheError> {
    if cache.sources.len() > CARGO_METADATA_MAX_CACHED_SOURCE_PATHS {
      return Err(MachineCacheError::InvalidCachePath(
        "cargo metadata cache records too many source paths".into(),
      ));
    }
    ensure_app_manifest_source(tauri_dir, app_manifest_source)?;
    let workspace_root = Path::new(&cache.metadata.workspace_root);
    if !cached_cargo_metadata_workspace_root_allowed(workspace_root, tauri_dir) {
      return Err(MachineCacheError::InvalidCachePath(
        "cargo metadata cache workspace_root is outside the app workspace ancestry".into(),
      ));
    }
    let allowed_sources =
      CargoMetadataAllowedSourcePaths::from_metadata(tauri_dir, &cache.metadata).ok_or_else(
        || {
          MachineCacheError::InvalidCachePath(
            "cargo metadata cache workspace_root is outside the app workspace ancestry".into(),
          )
        },
      )?;
    if !allowed_sources.matches_cached_sources(&cache.sources) {
      return Err(MachineCacheError::InvalidCachePath(
        "cargo metadata cache source set is incomplete or unexpected".into(),
      ));
    }
    let source_exclusions = SourcePathExclusions::new(tauri_dir, &app_manifest_source.path);

    let mut sources = SourceSnapshotBuilder::new();
    push_source_snapshot(&mut sources, app_manifest_source);
    for source in cache
      .sources
      .iter()
      .filter(|source| !source_exclusions.contains(&source.path))
    {
      let source_path = Path::new(&source.path);
      if !allowed_sources.contains(source_path) {
        return Err(MachineCacheError::InvalidCachePath(
          "cargo metadata cache source path is outside the expected metadata source set".into(),
        ));
      }
      push_current_path_snapshot(&mut sources, Path::new(&source.path))?;
    }
    Ok(Self {
      sources: sources.into_sources(),
    })
  }

  fn current(
    tauri_dir: &Path,
    metadata: &CargoMetadata,
    app_manifest_source: &MachineCacheSource,
  ) -> Result<Self, MachineCacheError> {
    ensure_app_manifest_source(tauri_dir, app_manifest_source)?;

    let mut sources = SourceSnapshotBuilder::new();
    push_source_snapshot(&mut sources, app_manifest_source);
    push_current_path_snapshot(&mut sources, &metadata.workspace_root.join("Cargo.toml"))?;
    push_current_path_snapshot(&mut sources, &metadata.workspace_root.join("Cargo.lock"))?;
    push_current_path_snapshot(&mut sources, &metadata.workspace_root.join(".cargo/config"))?;
    push_current_path_snapshot(
      &mut sources,
      &metadata.workspace_root.join(".cargo/config.toml"),
    )?;
    for package in metadata.packages.iter() {
      push_current_path_snapshot(&mut sources, &package.manifest_path)?;
    }
    for ancestor in tauri_dir.ancestors().skip(1) {
      push_current_path_snapshot(&mut sources, &ancestor.join("Cargo.toml"))?;
    }
    Ok(Self {
      sources: sources.into_sources(),
    })
  }
}

impl CargoMetadataMachineCache {
  fn from_current_sources(
    tauri_dir: &Path,
    metadata: &CargoMetadata,
    app_manifest_source: &MachineCacheSource,
  ) -> Result<Self, MachineCacheError> {
    let snapshot = CargoMetadataSourceSnapshot::current(tauri_dir, metadata, app_manifest_source)?;
    Ok(Self {
      tauri_dir: normalized_path(tauri_dir),
      cargo_target_dir_env: env::var("CARGO_TARGET_DIR").ok(),
      sources: snapshot.sources,
      metadata: CargoMetadataMachine::from(metadata),
    })
  }
}

impl From<&CargoMetadata> for CargoMetadataMachine {
  fn from(metadata: &CargoMetadata) -> Self {
    Self {
      target_directory: normalized_path(&metadata.target_directory),
      workspace_root: normalized_path(&metadata.workspace_root),
      workspace_members: metadata.workspace_members.clone(),
      packages: metadata.packages.iter().map(PackageMachine::from).collect(),
    }
  }
}

fn cargo_metadata_from_archived_metadata(metadata: &ArchivedCargoMetadataMachine) -> CargoMetadata {
  CargoMetadata {
    target_directory: PathBuf::from(metadata.target_directory.as_str()),
    workspace_root: PathBuf::from(metadata.workspace_root.as_str()),
    workspace_members: metadata
      .workspace_members
      .iter()
      .map(|member| member.as_str().to_string())
      .collect(),
    packages: metadata.packages.iter().map(package_from_archive).collect(),
  }
}

impl From<&Package> for PackageMachine {
  fn from(package: &Package) -> Self {
    Self {
      name: package.name.clone(),
      id: package.id.clone(),
      manifest_path: normalized_path(&package.manifest_path),
      dependencies: package
        .dependencies
        .iter()
        .map(DependencyMachine::from)
        .collect(),
    }
  }
}

fn package_from_archive(package: &ArchivedPackageMachine) -> Package {
  Package {
    name: package.name.as_str().to_string(),
    id: package.id.as_str().to_string(),
    manifest_path: PathBuf::from(package.manifest_path.as_str()),
    dependencies: package
      .dependencies
      .iter()
      .map(dependency_from_archive)
      .collect(),
  }
}

impl From<&Dependency> for DependencyMachine {
  fn from(dependency: &Dependency) -> Self {
    Self {
      name: dependency.name.clone(),
      path: dependency.path.as_ref().map(|path| normalized_path(path)),
    }
  }
}

fn in_workspace_dependency_paths_from_archived_metadata(
  tauri_dir: &Path,
  metadata: &ArchivedCargoMetadataMachine,
) -> Option<Vec<PathBuf>> {
  let tauri_project_manifest_path = normalized_path(&tauri_dir.join("Cargo.toml"));
  let tauri_project_package = metadata
    .packages
    .iter()
    .find(|package| package.manifest_path.as_str() == tauri_project_manifest_path)?;

  let workspace_packages = metadata
    .workspace_members
    .iter()
    .map(|member_package_id| {
      metadata
        .packages
        .iter()
        .find(|package| package.id.as_str() == member_package_id.as_str())
    })
    .collect::<Option<Vec<_>>>()?;

  let mut found_dependency_paths = Vec::new();
  find_archived_dependencies(
    tauri_project_package,
    &workspace_packages,
    &mut found_dependency_paths,
  );
  Some(found_dependency_paths)
}

fn find_archived_dependencies(
  package: &ArchivedPackageMachine,
  workspace_packages: &[&ArchivedPackageMachine],
  found_dependency_paths: &mut Vec<PathBuf>,
) {
  for dependency in package.dependencies.iter() {
    if let Some(path) = dependency.path.as_ref() {
      let dependency_path = PathBuf::from(path.as_str());
      if let Some(package) = workspace_packages.iter().find(|workspace_package| {
        workspace_package.name.as_str() == dependency.name.as_str()
          && dependency_path.join("Cargo.toml")
            == PathBuf::from(workspace_package.manifest_path.as_str())
          && !found_dependency_paths.contains(&dependency_path)
      }) {
        found_dependency_paths.push(dependency_path);
        find_archived_dependencies(package, workspace_packages, found_dependency_paths);
      }
    }
  }
}

fn dependency_from_archive(dependency: &ArchivedDependencyMachine) -> Dependency {
  Dependency {
    name: dependency.name.as_str().to_string(),
    path: dependency
      .path
      .as_ref()
      .map(|path| PathBuf::from(path.as_str())),
  }
}

fn ensure_app_manifest_source(
  tauri_dir: &Path,
  source: &MachineCacheSource,
) -> Result<(), MachineCacheError> {
  let manifest_path = tauri_dir.join("Cargo.toml");
  if normalized_path(&manifest_path) != normalized_path(&source.path) {
    return Err(MachineCacheError::InvalidCachePath(
      "cargo metadata cache source path does not match app Cargo.toml".into(),
    ));
  }
  Ok(())
}

struct SourceSnapshotBuilder {
  sources: Vec<CargoMetadataSourceFingerprint>,
  seen_paths: HashSet<String>,
}

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

fn push_current_path_snapshot(
  sources: &mut SourceSnapshotBuilder,
  path: &Path,
) -> Result<(), MachineCacheError> {
  sources.push_current_path(path)
}

fn push_source_snapshot(sources: &mut SourceSnapshotBuilder, source: &MachineCacheSource) {
  sources.push_source(source);
}

struct SourcePathExclusions {
  app_manifest: String,
  tauri_manifest: String,
}

impl SourcePathExclusions {
  fn new(tauri_dir: &Path, app_manifest_path: &Path) -> Self {
    Self {
      app_manifest: normalized_path(app_manifest_path),
      tauri_manifest: normalized_path(&tauri_dir.join("Cargo.toml")),
    }
  }

  fn contains(&self, normalized_source_path: &str) -> bool {
    normalized_source_path == self.app_manifest || normalized_source_path == self.tauri_manifest
  }
}

fn cached_cargo_metadata_workspace_root_allowed(workspace_root: &Path, tauri_dir: &Path) -> bool {
  cached_source_path_looks_safe(workspace_root) && tauri_dir.starts_with(workspace_root)
}

struct CargoMetadataAllowedSourcePaths {
  normalized_sources: HashSet<String>,
}

impl CargoMetadataAllowedSourcePaths {
  fn from_archived_metadata(
    tauri_dir: &Path,
    metadata: &ArchivedCargoMetadataMachine,
  ) -> Option<Self> {
    let workspace_root = Path::new(metadata.workspace_root.as_str());
    if !cached_cargo_metadata_workspace_root_allowed(workspace_root, tauri_dir) {
      return None;
    }

    let mut allowed = Self {
      normalized_sources: HashSet::new(),
    };
    for expected in [
      workspace_root.join("Cargo.toml"),
      workspace_root.join("Cargo.lock"),
      workspace_root.join(".cargo").join("config"),
      workspace_root.join(".cargo").join("config.toml"),
    ] {
      allowed.insert(&expected);
    }
    for package in metadata.packages.iter() {
      let manifest_path = Path::new(package.manifest_path.as_str());
      if !cached_package_manifest_path_allowed(manifest_path, workspace_root) {
        return None;
      }
      allowed.insert(manifest_path);
    }
    for ancestor in tauri_dir.ancestors() {
      allowed.insert(&ancestor.join("Cargo.toml"));
    }
    Some(allowed)
  }

  fn from_metadata(tauri_dir: &Path, metadata: &CargoMetadataMachine) -> Option<Self> {
    let workspace_root = Path::new(&metadata.workspace_root);
    if !cached_cargo_metadata_workspace_root_allowed(workspace_root, tauri_dir) {
      return None;
    }

    let mut allowed = Self {
      normalized_sources: HashSet::new(),
    };
    for expected in [
      workspace_root.join("Cargo.toml"),
      workspace_root.join("Cargo.lock"),
      workspace_root.join(".cargo").join("config"),
      workspace_root.join(".cargo").join("config.toml"),
    ] {
      allowed.insert(&expected);
    }
    for package in &metadata.packages {
      let manifest_path = Path::new(&package.manifest_path);
      if !cached_package_manifest_path_allowed(manifest_path, workspace_root) {
        return None;
      }
      allowed.insert(manifest_path);
    }
    for ancestor in tauri_dir.ancestors() {
      allowed.insert(&ancestor.join("Cargo.toml"));
    }
    Some(allowed)
  }

  fn contains(&self, source_path: &Path) -> bool {
    cached_source_path_looks_safe(source_path)
      && self
        .normalized_sources
        .contains(&normalized_path(source_path))
  }

  fn matches_cached_sources(&self, sources: &[CargoMetadataSourceFingerprint]) -> bool {
    let cached_sources = sources
      .iter()
      .map(|source| source.path.as_str())
      .collect::<HashSet<_>>();
    cached_sources.len() == sources.len()
      && cached_sources.len() == self.normalized_sources.len()
      && self
        .normalized_sources
        .iter()
        .all(|source| cached_sources.contains(source.as_str()))
  }

  fn matches_archived_cached_sources(&self, cache: &ArchivedCargoMetadataMachineCache) -> bool {
    let cached_sources = cache
      .sources
      .iter()
      .map(|source| source.path.as_str())
      .collect::<HashSet<_>>();
    cached_sources.len() == cache.sources.len()
      && cached_sources.len() == self.normalized_sources.len()
      && self
        .normalized_sources
        .iter()
        .all(|source| cached_sources.contains(source.as_str()))
  }

  fn insert(&mut self, path: &Path) {
    self.normalized_sources.insert(normalized_path(path));
  }
}

fn cached_package_manifest_path_allowed(manifest_path: &Path, workspace_root: &Path) -> bool {
  cached_source_path_looks_safe(manifest_path)
    && manifest_path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml")
    && manifest_path.starts_with(workspace_root)
}

fn cached_source_path_looks_safe(path: &Path) -> bool {
  path.is_absolute()
    && !path
      .components()
      .any(|component| matches!(component, std::path::Component::ParentDir))
}

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

  fn matches(&self, expected: &Self) -> bool {
    self.present == expected.present
      && self.bytes == expected.bytes
      && self.blake3 == expected.blake3
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

fn cargo_metadata_machine_cache_paths(
  tauri_dir: &Path,
) -> Result<serializer::machine::MachineCachePaths, MachineCacheError> {
  paths_for_project_cache(
    tauri_dir,
    "tauri",
    "cargo-metadata",
    &tauri_dir.join("Cargo.toml"),
  )
}

fn cargo_metadata_machine_cache_schema() -> MachineCacheSchema {
  MachineCacheSchema {
    name: "dx.tauri.cli.cargo_metadata_full",
    version: CARGO_METADATA_MACHINE_CACHE_SCHEMA_VERSION,
    kind: MachineCacheKind::Config,
  }
}

fn normalized_path(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
  use super::{
    cached_cargo_metadata_workspace_root_allowed, current_cargo_metadata_sources_match,
    normalized_path, CargoMetadata, CargoMetadataAllowedSourcePaths, CargoMetadataMachine,
    CargoMetadataMachineCache, CargoMetadataSourceSnapshot, Dependency, Package,
    SourcePathExclusions,
  };
  use serializer::machine::source_fingerprint;
  use std::path::{Path, PathBuf};

  fn cargo_metadata(workspace_root: &Path) -> CargoMetadata {
    CargoMetadata {
      target_directory: workspace_root.join("target"),
      workspace_root: workspace_root.to_path_buf(),
      workspace_members: vec![],
      packages: vec![Package {
        name: "fixture".into(),
        id: "path+file:///fixture".into(),
        manifest_path: workspace_root
          .join("crates")
          .join("fixture")
          .join("Cargo.toml"),
        dependencies: vec![Dependency {
          name: "dep".into(),
          path: None,
        }],
      }],
    }
  }

  fn metadata_machine(workspace_root: &Path) -> CargoMetadataMachine {
    CargoMetadataMachine::from(&cargo_metadata(workspace_root))
  }

  #[test]
  fn dx_cargo_metadata_cache_rejects_implausible_cached_source_path() {
    let tauri_dir = Path::new(r"G:\Dx\repo\apps\fixture\src-tauri");
    let workspace_root = Path::new(r"G:\Dx\repo");
    let metadata = metadata_machine(workspace_root);
    let allowed_sources = CargoMetadataAllowedSourcePaths::from_metadata(tauri_dir, &metadata)
      .expect("workspace metadata should produce an allowed source index");

    assert!(cached_cargo_metadata_workspace_root_allowed(
      workspace_root,
      tauri_dir
    ));
    assert!(allowed_sources.contains(&workspace_root.join("Cargo.toml")));
    assert!(allowed_sources.contains(&workspace_root.join("Cargo.lock")));
    assert!(allowed_sources.contains(&workspace_root.join(".cargo").join("config.toml")));
    assert!(allowed_sources.contains(
      &workspace_root
        .join("crates")
        .join("fixture")
        .join("Cargo.toml")
    ));
    assert!(allowed_sources.contains(&tauri_dir.join("Cargo.toml")));
    assert!(!cached_cargo_metadata_workspace_root_allowed(
      Path::new(r"G:\Dx\repo\apps\fixture\src-tauri\child"),
      tauri_dir
    ));
    assert!(!allowed_sources.contains(Path::new(r"G:\Dx\unrelated\Cargo.toml")));
    assert!(!allowed_sources.contains(Path::new(
      r"G:\Dx\repo\apps\fixture\src-tauri\..\outside\Cargo.toml"
    )));
    assert!(!allowed_sources.contains(Path::new(r"G:\Dx\repo\Cargo.toml.bak")));
  }

  #[test]
  fn dx_cargo_metadata_cache_indexes_allowed_cached_source_paths_once() {
    let tauri_dir = Path::new(r"G:\Dx\repo\apps\fixture\src-tauri");
    let workspace_root = Path::new(r"G:\Dx\repo");
    let metadata = metadata_machine(workspace_root);
    let allowed_sources = CargoMetadataAllowedSourcePaths::from_metadata(tauri_dir, &metadata)
      .expect("workspace metadata should produce an allowed source index");

    assert!(allowed_sources.contains(&workspace_root.join("Cargo.toml")));
    assert!(allowed_sources.contains(
      &workspace_root
        .join("crates")
        .join("fixture")
        .join("Cargo.toml")
    ));
    assert!(allowed_sources.contains(&tauri_dir.join("Cargo.toml")));
    assert!(!allowed_sources.contains(Path::new(r"G:\Dx\unrelated\Cargo.toml")));
    assert!(!allowed_sources.contains(Path::new(
      r"G:\Dx\repo\apps\fixture\src-tauri\..\outside\Cargo.toml"
    )));
  }

  #[test]
  fn dx_cargo_metadata_cache_rejects_invalid_package_manifest_path_in_metadata() {
    let tauri_dir = Path::new(r"G:\Dx\repo\apps\fixture\src-tauri");
    let workspace_root = Path::new(r"G:\Dx\repo");
    let mut metadata = metadata_machine(workspace_root);
    metadata.packages[0].manifest_path = normalized_path(Path::new(r"G:\Dx\unrelated\Cargo.toml"));

    assert!(CargoMetadataAllowedSourcePaths::from_metadata(tauri_dir, &metadata).is_none());
  }

  #[test]
  fn dx_cargo_metadata_cache_precomputes_source_exclusions() {
    let tauri_dir = Path::new(r"G:\Dx\repo\apps\fixture\src-tauri");
    let app_manifest_path = tauri_dir.join("Cargo.toml");
    let exclusions = SourcePathExclusions::new(tauri_dir, &app_manifest_path);

    assert!(exclusions.contains(&normalized_path(&app_manifest_path)));
    assert!(exclusions.contains(&normalized_path(&tauri_dir.join("Cargo.toml"))));
    assert!(!exclusions.contains(&normalized_path(
      &Path::new(r"G:\Dx\repo").join("Cargo.toml")
    )));
  }

  #[test]
  fn dx_cargo_metadata_cache_rejects_incomplete_cached_source_set() {
    let fixture = temp_metadata_project("incomplete-source-set");
    let workspace_root = fixture.path();
    let tauri_dir = workspace_root
      .join("apps")
      .join("fixture")
      .join("src-tauri");
    let metadata = cargo_metadata(workspace_root);
    let app_manifest_source =
      source_fingerprint(&tauri_dir.join("Cargo.toml")).expect("fingerprint app manifest");
    let snapshot =
      CargoMetadataSourceSnapshot::current(&tauri_dir, &metadata, &app_manifest_source)
        .expect("snapshot current metadata sources");
    let mut cache = CargoMetadataMachineCache {
      tauri_dir: normalized_path(&tauri_dir),
      cargo_target_dir_env: std::env::var("CARGO_TARGET_DIR").ok(),
      sources: snapshot.sources,
      metadata: CargoMetadataMachine::from(&metadata),
    };
    let removed_source = normalized_path(&workspace_root.join("Cargo.lock"));

    cache.sources.retain(|source| source.path != removed_source);

    assert!(!current_cargo_metadata_sources_match(
      &cache,
      &tauri_dir,
      &app_manifest_source
    ));
  }

  fn temp_metadata_project(name: &str) -> tempfile::TempDir {
    let base = std::env::var_os("DX_TEST_OUTPUT_DIR")
      .map(PathBuf::from)
      .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&base).expect("create test output base");
    let project = tempfile::Builder::new()
      .prefix(&format!("tauri-dx-cargo-metadata-{name}-"))
      .tempdir_in(base)
      .expect("create metadata temp project");
    write_file(
      &project.path().join("Cargo.toml"),
      r#"[workspace]
members = ["crates/fixture", "apps/fixture/src-tauri"]
"#,
    );
    write_file(&project.path().join("Cargo.lock"), "# lockfile\n");
    write_file(
      &project
        .path()
        .join("crates")
        .join("fixture")
        .join("Cargo.toml"),
      r#"[package]
name = "fixture"
version = "1.0.0"
"#,
    );
    write_file(
      &project
        .path()
        .join("apps")
        .join("fixture")
        .join("src-tauri")
        .join("Cargo.toml"),
      r#"[package]
name = "fixture-app"
version = "1.0.0"
"#,
    );
    project
  }

  fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent).expect("create fixture parent");
    }
    std::fs::write(path, contents).expect("write fixture file");
  }
}
