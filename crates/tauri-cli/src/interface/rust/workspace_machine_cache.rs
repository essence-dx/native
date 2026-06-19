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

const TAURI_DX_MACHINE_CACHE_ENV: &str = "TAURI_DX_MACHINE_CACHE";
const CARGO_WORKSPACE_MACHINE_CACHE_SCHEMA_VERSION: u32 = 1;
const CARGO_WORKSPACE_MACHINE_CACHE_MAX_BYTES: u64 = 1024 * 1024;
const CARGO_WORKSPACE_MAX_CACHED_SOURCE_PATHS: usize = 256;

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct CargoWorkspaceMachineCache {
  tauri_dir: String,
  workspace_dir: String,
  sources: Vec<CargoWorkspaceSourceFingerprint>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct CargoWorkspaceSourceFingerprint {
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

pub(super) fn read(tauri_dir: &Path) -> Option<PathBuf> {
  let paths = cargo_workspace_machine_cache_paths(tauri_dir).ok()?;
  if !machine_cache_file_is_candidate(&paths.machine, CARGO_WORKSPACE_MACHINE_CACHE_MAX_BYTES) {
    return None;
  }
  let source = source_fingerprint(&paths.source).ok()?;

  #[cfg(feature = "dx-machine-cache-mmap")]
  if let Ok(mapped) = open_typed_machine_cache::<CargoWorkspaceMachineCache>(
    &paths,
    &source,
    cargo_workspace_machine_cache_schema(),
  ) {
    return workspace_dir_from_archive(mapped.archived(), tauri_dir, &source);
  }

  let bytes = read_machine_file_bounded(&paths.machine, CARGO_WORKSPACE_MACHINE_CACHE_MAX_BYTES)?;
  let archived = access_typed_machine_cache::<CargoWorkspaceMachineCache>(
    &bytes,
    &source,
    cargo_workspace_machine_cache_schema(),
  )
  .ok()?;
  workspace_dir_from_archive(archived, tauri_dir, &source)
}

pub(super) fn write(tauri_dir: &Path, workspace_dir: &Path) -> Result<PathBuf, MachineCacheError> {
  let paths = cargo_workspace_machine_cache_paths(tauri_dir)?;
  let source = source_fingerprint(&paths.source)?;
  let payload =
    CargoWorkspaceMachineCache::from_current_sources(tauri_dir, workspace_dir, &source)?;
  let receipt = write_typed_machine_cache(
    &payload,
    &source,
    &paths,
    cargo_workspace_machine_cache_schema(),
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
    .join("cargo-workspace.machine")
}

fn workspace_dir_from_archive(
  archived: &ArchivedCargoWorkspaceMachineCache,
  tauri_dir: &Path,
  app_manifest_source: &MachineCacheSource,
) -> Option<PathBuf> {
  if archived.tauri_dir.as_str() != normalized_path(tauri_dir)
    || !current_workspace_sources_match_archived(archived, tauri_dir, app_manifest_source)
  {
    return None;
  }
  Some(PathBuf::from(archived.workspace_dir.as_str()))
}

fn current_workspace_sources_match_archived(
  cache: &ArchivedCargoWorkspaceMachineCache,
  tauri_dir: &Path,
  app_manifest_source: &MachineCacheSource,
) -> bool {
  let Some(sources) = cache
    .sources
    .iter()
    .map(workspace_source_fingerprint_from_archive)
    .collect::<Option<Vec<_>>>()
  else {
    return false;
  };
  let cache = CargoWorkspaceMachineCache {
    tauri_dir: cache.tauri_dir.as_str().to_string(),
    workspace_dir: cache.workspace_dir.as_str().to_string(),
    sources,
  };
  current_workspace_sources_match(&cache, tauri_dir, app_manifest_source)
}

fn workspace_source_fingerprint_from_archive(
  source: &ArchivedCargoWorkspaceSourceFingerprint,
) -> Option<CargoWorkspaceSourceFingerprint> {
  let mut blake3 = [0; 32];
  for (target, value) in blake3.iter_mut().zip(source.blake3.iter()) {
    *target = *value;
  }

  Some(CargoWorkspaceSourceFingerprint {
    path: source.path.as_str().to_string(),
    present: source.present,
    bytes: source.bytes.to_native(),
    modified_unix_ms: source
      .modified_unix_ms
      .as_ref()
      .map(|value| value.to_native()),
    blake3,
  })
}

fn current_workspace_sources_match(
  cache: &CargoWorkspaceMachineCache,
  tauri_dir: &Path,
  app_manifest_source: &MachineCacheSource,
) -> bool {
  let Ok(snapshot) =
    CargoWorkspaceSourceSnapshot::from_cached_paths(cache, tauri_dir, app_manifest_source)
  else {
    return false;
  };
  source_fingerprints_match(&cache.sources, &snapshot.sources)
}

fn source_fingerprints_match(
  expected: &[CargoWorkspaceSourceFingerprint],
  current: &[CargoWorkspaceSourceFingerprint],
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

struct CargoWorkspaceSourceSnapshot {
  sources: Vec<CargoWorkspaceSourceFingerprint>,
}

impl CargoWorkspaceSourceSnapshot {
  fn from_cached_paths(
    cache: &CargoWorkspaceMachineCache,
    tauri_dir: &Path,
    app_manifest_source: &MachineCacheSource,
  ) -> Result<Self, MachineCacheError> {
    if cache.sources.len() > CARGO_WORKSPACE_MAX_CACHED_SOURCE_PATHS {
      return Err(MachineCacheError::InvalidCachePath(
        "cargo workspace cache records too many source paths".into(),
      ));
    }

    let manifest_path = tauri_dir.join("Cargo.toml");
    if normalized_path(&manifest_path) != normalized_path(&app_manifest_source.path) {
      return Err(MachineCacheError::InvalidCachePath(
        "cargo workspace cache source path does not match app Cargo.toml".into(),
      ));
    }
    let workspace_dir = PathBuf::from(&cache.workspace_dir);
    if !cached_workspace_dir_allowed(&workspace_dir, tauri_dir) {
      return Err(MachineCacheError::InvalidCachePath(
        "cargo workspace cache workspace_dir is outside the app workspace ancestry".into(),
      ));
    }
    let allowed_sources = WorkspaceAllowedSourcePaths::from_context(tauri_dir, &workspace_dir)
      .ok_or_else(|| {
        MachineCacheError::InvalidCachePath(
          "cargo workspace cache workspace_dir is outside the app workspace ancestry".into(),
        )
      })?;
    if !allowed_sources.matches_cached_sources(&cache.sources) {
      return Err(MachineCacheError::InvalidCachePath(
        "cargo workspace cache source set is incomplete or unexpected".into(),
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
          "cargo workspace cache source path is outside the expected workspace source set".into(),
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
    workspace_dir: &Path,
    app_manifest_source: &MachineCacheSource,
  ) -> Result<Self, MachineCacheError> {
    let manifest_path = tauri_dir.join("Cargo.toml");
    if normalized_path(&manifest_path) != normalized_path(&app_manifest_source.path) {
      return Err(MachineCacheError::InvalidCachePath(
        "cargo workspace cache source path does not match app Cargo.toml".into(),
      ));
    }

    let mut sources = SourceSnapshotBuilder::new();
    push_source_snapshot(&mut sources, app_manifest_source);
    push_current_path_snapshot(&mut sources, &workspace_dir.join("Cargo.toml"))?;
    for ancestor in tauri_dir.ancestors().skip(1) {
      push_current_path_snapshot(&mut sources, &ancestor.join("Cargo.toml"))?;
    }
    Ok(Self {
      sources: sources.into_sources(),
    })
  }
}

impl CargoWorkspaceMachineCache {
  fn from_current_sources(
    tauri_dir: &Path,
    workspace_dir: &Path,
    app_manifest_source: &MachineCacheSource,
  ) -> Result<Self, MachineCacheError> {
    let snapshot =
      CargoWorkspaceSourceSnapshot::current(tauri_dir, workspace_dir, app_manifest_source)?;
    Ok(Self {
      tauri_dir: normalized_path(tauri_dir),
      workspace_dir: normalized_path(workspace_dir),
      sources: snapshot.sources,
    })
  }
}

struct SourceSnapshotBuilder {
  sources: Vec<CargoWorkspaceSourceFingerprint>,
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
      .push(CargoWorkspaceSourceFingerprint::current(path)?);
    Ok(())
  }

  fn push_source(&mut self, source: &MachineCacheSource) {
    if !self.seen_paths.insert(normalized_path(&source.path)) {
      return;
    }
    self
      .sources
      .push(CargoWorkspaceSourceFingerprint::from_source(source));
  }

  fn into_sources(self) -> Vec<CargoWorkspaceSourceFingerprint> {
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

fn cached_workspace_dir_allowed(workspace_dir: &Path, tauri_dir: &Path) -> bool {
  cached_source_path_looks_safe(workspace_dir) && tauri_dir.starts_with(workspace_dir)
}

struct WorkspaceAllowedSourcePaths {
  normalized_sources: HashSet<String>,
}

impl WorkspaceAllowedSourcePaths {
  fn from_context(tauri_dir: &Path, workspace_dir: &Path) -> Option<Self> {
    if !cached_workspace_dir_allowed(workspace_dir, tauri_dir) {
      return None;
    }
    let mut allowed = Self {
      normalized_sources: HashSet::new(),
    };
    allowed.insert(&workspace_dir.join("Cargo.toml"));
    for ancestor in tauri_dir.ancestors() {
      allowed.insert(&ancestor.join("Cargo.toml"));
    }
    Some(allowed)
  }

  fn contains(&self, source_path: &Path) -> bool {
    cached_workspace_source_path_looks_safe(source_path)
      && self
        .normalized_sources
        .contains(&normalized_path(source_path))
  }

  fn matches_cached_sources(&self, sources: &[CargoWorkspaceSourceFingerprint]) -> bool {
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

  fn insert(&mut self, path: &Path) {
    self.normalized_sources.insert(normalized_path(path));
  }
}

#[cfg(test)]
fn cached_workspace_source_path_allowed(
  source_path: &Path,
  tauri_dir: &Path,
  workspace_dir: &Path,
) -> bool {
  WorkspaceAllowedSourcePaths::from_context(tauri_dir, workspace_dir)
    .is_some_and(|allowed_sources| allowed_sources.contains(source_path))
}

fn cached_workspace_source_path_looks_safe(path: &Path) -> bool {
  cached_source_path_looks_safe(path)
    && path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml")
}

fn cached_source_path_looks_safe(path: &Path) -> bool {
  path.is_absolute()
    && !path
      .components()
      .any(|component| matches!(component, std::path::Component::ParentDir))
}

impl CargoWorkspaceSourceFingerprint {
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
}

fn cargo_workspace_machine_cache_paths(
  tauri_dir: &Path,
) -> Result<serializer::machine::MachineCachePaths, MachineCacheError> {
  paths_for_project_cache(
    tauri_dir,
    "tauri",
    "cargo-workspace",
    &tauri_dir.join("Cargo.toml"),
  )
}

fn cargo_workspace_machine_cache_schema() -> MachineCacheSchema {
  MachineCacheSchema {
    name: "dx.tauri.cli.cargo_workspace_dir",
    version: CARGO_WORKSPACE_MACHINE_CACHE_SCHEMA_VERSION,
    kind: MachineCacheKind::Config,
  }
}

fn normalized_path(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
  use super::{
    cached_workspace_dir_allowed, cached_workspace_source_path_allowed,
    current_workspace_sources_match, normalized_path, CargoWorkspaceMachineCache,
    CargoWorkspaceSourceFingerprint, CargoWorkspaceSourceSnapshot, SourcePathExclusions,
    WorkspaceAllowedSourcePaths,
  };
  use serializer::machine::source_fingerprint;
  use std::path::{Path, PathBuf};

  #[test]
  fn dx_workspace_source_fingerprint_matches_when_only_mtime_drifts() {
    let cached = CargoWorkspaceSourceFingerprint {
      path: "G:/Dx/app/Cargo.toml".into(),
      present: true,
      bytes: 42,
      modified_unix_ms: Some(1),
      blake3: [7; 32],
    };
    let current = CargoWorkspaceSourceFingerprint {
      modified_unix_ms: Some(2),
      ..cached.clone()
    };

    assert!(current.matches(&cached));
  }

  #[test]
  fn dx_workspace_cache_rejects_implausible_cached_source_path() {
    let tauri_dir = Path::new(r"G:\Dx\app\src-tauri");
    let workspace_dir = Path::new(r"G:\Dx\app");

    assert!(cached_workspace_dir_allowed(workspace_dir, tauri_dir));
    assert!(cached_workspace_source_path_allowed(
      &workspace_dir.join("Cargo.toml"),
      tauri_dir,
      workspace_dir
    ));
    assert!(cached_workspace_source_path_allowed(
      &tauri_dir.join("Cargo.toml"),
      tauri_dir,
      workspace_dir
    ));
    assert!(!cached_workspace_dir_allowed(
      Path::new(r"G:\Dx\app\src-tauri\child"),
      tauri_dir
    ));
    assert!(!cached_workspace_source_path_allowed(
      Path::new(r"G:\Dx\unrelated\Cargo.toml"),
      tauri_dir,
      workspace_dir
    ));
    assert!(!cached_workspace_source_path_allowed(
      Path::new(r"G:\Dx\app\src-tauri\..\outside\Cargo.toml"),
      tauri_dir,
      workspace_dir
    ));
    assert!(!cached_workspace_source_path_allowed(
      Path::new(r"G:\Dx\app\Cargo.lock"),
      tauri_dir,
      workspace_dir
    ));
  }

  #[test]
  fn dx_workspace_cache_indexes_allowed_source_paths_once() {
    let tauri_dir = Path::new(r"G:\Dx\app\src-tauri");
    let workspace_dir = Path::new(r"G:\Dx\app");
    let allowed_sources = WorkspaceAllowedSourcePaths::from_context(tauri_dir, workspace_dir)
      .expect("workspace paths should produce an allowed source index");

    assert!(allowed_sources.contains(&workspace_dir.join("Cargo.toml")));
    assert!(allowed_sources.contains(&tauri_dir.join("Cargo.toml")));
    assert!(!allowed_sources.contains(Path::new(r"G:\Dx\unrelated\Cargo.toml")));
    assert!(!allowed_sources.contains(Path::new(r"G:\Dx\app\src-tauri\..\outside\Cargo.toml")));
    assert!(!allowed_sources.contains(Path::new(r"G:\Dx\app\Cargo.lock")));
  }

  #[test]
  fn dx_workspace_cache_precomputes_source_exclusions() {
    let tauri_dir = Path::new(r"G:\Dx\app\src-tauri");
    let app_manifest_path = tauri_dir.join("Cargo.toml");
    let exclusions = SourcePathExclusions::new(tauri_dir, &app_manifest_path);

    assert!(exclusions.contains(&normalized_path(&app_manifest_path)));
    assert!(exclusions.contains(&normalized_path(&tauri_dir.join("Cargo.toml"))));
    assert!(!exclusions.contains(&normalized_path(
      &Path::new(r"G:\Dx\app").join("Cargo.toml")
    )));
  }

  #[test]
  fn dx_workspace_cache_rejects_incomplete_cached_source_set() {
    let fixture = temp_workspace_project("incomplete-source-set");
    let workspace_dir = fixture.path().join("workspace");
    let tauri_dir = workspace_dir.join("app").join("src-tauri");
    let app_manifest_source =
      source_fingerprint(&tauri_dir.join("Cargo.toml")).expect("fingerprint app manifest");
    let snapshot =
      CargoWorkspaceSourceSnapshot::current(&tauri_dir, &workspace_dir, &app_manifest_source)
        .expect("snapshot current workspace sources");
    let mut cache = CargoWorkspaceMachineCache {
      tauri_dir: normalized_path(&tauri_dir),
      workspace_dir: normalized_path(&workspace_dir),
      sources: snapshot.sources,
    };
    let removed_source = normalized_path(&workspace_dir.join("Cargo.toml"));

    cache.sources.retain(|source| source.path != removed_source);

    assert!(!current_workspace_sources_match(
      &cache,
      &tauri_dir,
      &app_manifest_source
    ));
  }

  fn temp_workspace_project(name: &str) -> tempfile::TempDir {
    let base = std::env::var_os("DX_TEST_OUTPUT_DIR")
      .map(PathBuf::from)
      .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&base).expect("create test output base");
    let project = tempfile::Builder::new()
      .prefix(&format!("tauri-dx-workspace-cache-{name}-"))
      .tempdir_in(base)
      .expect("create workspace temp project");
    let workspace_dir = project.path().join("workspace");
    let tauri_dir = workspace_dir.join("app").join("src-tauri");
    write_file(
      &workspace_dir.join("Cargo.toml"),
      r#"[workspace]
members = ["app/src-tauri"]
"#,
    );
    write_file(
      &tauri_dir.join("Cargo.toml"),
      r#"[package]
name = "workspace-cache-app"
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
