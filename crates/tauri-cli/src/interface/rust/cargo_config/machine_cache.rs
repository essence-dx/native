// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  collections::{HashMap, HashSet},
  env, fs,
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

use super::{BuildConfig, Config, PathAncestors};

const TAURI_DX_MACHINE_CACHE_ENV: &str = "TAURI_DX_MACHINE_CACHE";
const CARGO_CONFIG_MACHINE_CACHE_SCHEMA_VERSION: u32 = 1;
const CARGO_CONFIG_MACHINE_CACHE_MAX_BYTES: u64 = 1024 * 1024;
const CARGO_CONFIG_MAX_CACHED_SOURCE_PATHS: usize = 512;

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct CargoConfigMachineCache {
  tauri_dir: String,
  cargo_home: Option<String>,
  target: Option<String>,
  sources: Vec<CargoConfigSourceFingerprint>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct CargoConfigSourceFingerprint {
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

pub(super) fn read(tauri_dir: &Path) -> Option<Config> {
  let paths = cargo_config_machine_cache_paths(tauri_dir).ok()?;
  if !machine_cache_file_is_candidate(&paths.machine, CARGO_CONFIG_MACHINE_CACHE_MAX_BYTES) {
    return None;
  }
  let source = source_fingerprint(&paths.source).ok()?;

  #[cfg(feature = "dx-machine-cache-mmap")]
  if let Ok(mapped) = open_typed_machine_cache::<CargoConfigMachineCache>(
    &paths,
    &source,
    cargo_config_machine_cache_schema(),
  ) {
    return cargo_config_from_archive(mapped.archived(), tauri_dir, &source);
  }

  let bytes = read_machine_file_bounded(&paths.machine, CARGO_CONFIG_MACHINE_CACHE_MAX_BYTES)?;
  let archived = access_typed_machine_cache::<CargoConfigMachineCache>(
    &bytes,
    &source,
    cargo_config_machine_cache_schema(),
  )
  .ok()?;
  cargo_config_from_archive(archived, tauri_dir, &source)
}

pub(super) fn write(tauri_dir: &Path, config: &Config) -> Result<PathBuf, MachineCacheError> {
  if config.build.target.is_none() {
    return Err(MachineCacheError::InvalidCachePath(
      "Cargo config cache skips empty target results".into(),
    ));
  }
  let paths = cargo_config_machine_cache_paths(tauri_dir)?;
  let source = source_fingerprint(&paths.source)?;
  let payload = CargoConfigMachineCache::from_current_sources(tauri_dir, config, &source)?;
  let receipt = write_typed_machine_cache(
    &payload,
    &source,
    &paths,
    cargo_config_machine_cache_schema(),
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
    .join("cargo-config.machine")
}

fn cargo_config_from_archive(
  archived: &ArchivedCargoConfigMachineCache,
  tauri_dir: &Path,
  app_manifest_source: &MachineCacheSource,
) -> Option<Config> {
  if archived.tauri_dir.as_str() != normalized_path(tauri_dir)
    || archived_optional_string(&archived.cargo_home) != normalized_cargo_home()
    || !current_cargo_config_sources_match_archived(archived, tauri_dir, app_manifest_source)
  {
    return None;
  }
  let target = archived
    .target
    .as_ref()
    .map(|target| target.as_str().to_string());
  Some(Config {
    build: BuildConfig { target },
  })
}

fn archived_optional_string(
  value: &rkyv::option::ArchivedOption<rkyv::string::ArchivedString>,
) -> Option<String> {
  value.as_ref().map(|value| value.as_str().to_string())
}

fn current_cargo_config_sources_match_archived(
  cache: &ArchivedCargoConfigMachineCache,
  tauri_dir: &Path,
  app_manifest_source: &MachineCacheSource,
) -> bool {
  let Some(sources) = cache
    .sources
    .iter()
    .map(cargo_config_source_fingerprint_from_archive)
    .collect::<Option<Vec<_>>>()
  else {
    return false;
  };
  let cache = CargoConfigMachineCache {
    tauri_dir: cache.tauri_dir.as_str().to_string(),
    cargo_home: archived_optional_string(&cache.cargo_home),
    target: archived_optional_string(&cache.target),
    sources,
  };
  current_cargo_config_sources_match(&cache, tauri_dir, app_manifest_source)
}

fn cargo_config_source_fingerprint_from_archive(
  source: &ArchivedCargoConfigSourceFingerprint,
) -> Option<CargoConfigSourceFingerprint> {
  let mut blake3 = [0; 32];
  for (target, value) in blake3.iter_mut().zip(source.blake3.iter()) {
    *target = *value;
  }

  Some(CargoConfigSourceFingerprint {
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

fn current_cargo_config_sources_match(
  cache: &CargoConfigMachineCache,
  tauri_dir: &Path,
  app_manifest_source: &MachineCacheSource,
) -> bool {
  let Ok(snapshot) =
    CargoConfigSourceSnapshot::from_cached_paths(cache, tauri_dir, app_manifest_source)
  else {
    return false;
  };
  source_fingerprints_match(&cache.sources, &snapshot.sources)
}

fn source_fingerprints_match(
  expected: &[CargoConfigSourceFingerprint],
  current: &[CargoConfigSourceFingerprint],
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

struct CargoConfigSourceSnapshot {
  sources: Vec<CargoConfigSourceFingerprint>,
}

impl CargoConfigSourceSnapshot {
  fn from_cached_paths(
    cache: &CargoConfigMachineCache,
    tauri_dir: &Path,
    app_manifest_source: &MachineCacheSource,
  ) -> Result<Self, MachineCacheError> {
    if cache.sources.len() > CARGO_CONFIG_MAX_CACHED_SOURCE_PATHS {
      return Err(MachineCacheError::InvalidCachePath(
        "Cargo config cache records too many source paths".into(),
      ));
    }

    let manifest_path = tauri_dir.join("Cargo.toml");
    if normalized_path(&manifest_path) != normalized_path(&app_manifest_source.path) {
      return Err(MachineCacheError::InvalidCachePath(
        "Cargo config cache source path does not match app Cargo.toml".into(),
      ));
    }

    let mut sources = SourceSnapshotBuilder::new();
    push_source_snapshot(&mut sources, app_manifest_source);
    let cargo_home = cache.cargo_home.as_deref().map(Path::new);
    let allowed_source_dirs = CargoConfigAllowedSourceDirs::from_context(tauri_dir, cargo_home);
    let source_exclusions = SourcePathExclusions::new(tauri_dir, &app_manifest_source.path);
    for source in cache
      .sources
      .iter()
      .filter(|source| !source_exclusions.contains(&source.path))
    {
      let source_path = Path::new(&source.path);
      if !allowed_source_dirs.contains_source_path(source_path) {
        return Err(MachineCacheError::InvalidCachePath(
          "Cargo config cache source path is outside the expected config source set".into(),
        ));
      }
      push_current_path_snapshot(&mut sources, Path::new(&source.path))?;
    }
    let sources = sources.into_sources();
    reject_ambiguous_config_pairs_from_sources(&sources)?;

    Ok(Self { sources })
  }

  fn current(
    tauri_dir: &Path,
    app_manifest_source: &MachineCacheSource,
  ) -> Result<Self, MachineCacheError> {
    let manifest_path = tauri_dir.join("Cargo.toml");
    if normalized_path(&manifest_path) != normalized_path(&app_manifest_source.path) {
      return Err(MachineCacheError::InvalidCachePath(
        "Cargo config cache source path does not match app Cargo.toml".into(),
      ));
    }

    let mut sources = SourceSnapshotBuilder::new();
    push_source_snapshot(&mut sources, app_manifest_source);
    for current in PathAncestors::new(tauri_dir) {
      push_cargo_config_dir_sources(&mut sources, &current.join(".cargo"))?;
    }
    if let Some(cargo_home) = cargo_home_path() {
      push_cargo_config_dir_sources(&mut sources, &cargo_home)?;
    }

    Ok(Self {
      sources: sources.into_sources(),
    })
  }
}

impl CargoConfigMachineCache {
  fn from_current_sources(
    tauri_dir: &Path,
    config: &Config,
    app_manifest_source: &MachineCacheSource,
  ) -> Result<Self, MachineCacheError> {
    let snapshot = CargoConfigSourceSnapshot::current(tauri_dir, app_manifest_source)?;
    Ok(Self {
      tauri_dir: normalized_path(tauri_dir),
      cargo_home: normalized_cargo_home(),
      target: config.build.target.clone(),
      sources: snapshot.sources,
    })
  }
}

fn push_cargo_config_dir_sources(
  sources: &mut SourceSnapshotBuilder,
  dir: &Path,
) -> Result<(), MachineCacheError> {
  let config = dir.join("config");
  let config_toml = dir.join("config.toml");
  if ambiguous_config_pair_without_symlink(&config, &config_toml) {
    return Err(MachineCacheError::InvalidCachePath(format!(
      "ambiguous Cargo config pair at {}",
      normalized_path(dir)
    )));
  }
  push_current_path_snapshot(sources, &config)?;
  push_current_path_snapshot(sources, &config_toml)?;
  Ok(())
}

fn reject_ambiguous_config_pairs_from_sources(
  sources: &[CargoConfigSourceFingerprint],
) -> Result<(), MachineCacheError> {
  for source in sources {
    if let Some(dir) = config_dir_for_slot(&source.path) {
      let config = dir.join("config");
      let config_toml = dir.join("config.toml");
      if ambiguous_config_pair_without_symlink(&config, &config_toml) {
        return Err(MachineCacheError::InvalidCachePath(format!(
          "ambiguous Cargo config pair at {}",
          normalized_path(&dir)
        )));
      }
    }
  }
  Ok(())
}

fn config_dir_for_slot(path: &str) -> Option<PathBuf> {
  let path = PathBuf::from(path);
  match path.file_name().and_then(|name| name.to_str()) {
    Some("config") | Some("config.toml") => path.parent().map(Path::to_path_buf),
    _ => None,
  }
}

fn ambiguous_config_pair_without_symlink(config: &Path, config_toml: &Path) -> bool {
  if !config.exists() || !config_toml.exists() {
    return false;
  }
  if let Ok(target_path) = fs::read_link(config) {
    return target_path != config_toml;
  }
  true
}

struct SourceSnapshotBuilder {
  sources: Vec<CargoConfigSourceFingerprint>,
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
      .push(CargoConfigSourceFingerprint::current(path)?);
    Ok(())
  }

  fn push_source(&mut self, source: &MachineCacheSource) {
    if !self.seen_paths.insert(normalized_path(&source.path)) {
      return;
    }
    self
      .sources
      .push(CargoConfigSourceFingerprint::from_source(source));
  }

  fn into_sources(self) -> Vec<CargoConfigSourceFingerprint> {
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

struct CargoConfigAllowedSourceDirs {
  normalized_dirs: HashSet<String>,
}

impl CargoConfigAllowedSourceDirs {
  fn from_context(tauri_dir: &Path, cargo_home: Option<&Path>) -> Self {
    let mut allowed = Self {
      normalized_dirs: HashSet::new(),
    };
    for ancestor in PathAncestors::new(tauri_dir) {
      allowed.insert_dir(&ancestor.join(".cargo"));
    }
    if let Some(home) = cargo_home {
      allowed.insert_dir(home);
    }
    allowed
  }

  fn contains_source_path(&self, source_path: &Path) -> bool {
    if !cached_source_path_looks_safe(source_path)
      || !matches!(
        source_path.file_name().and_then(|name| name.to_str()),
        Some("config" | "config.toml")
      )
    {
      return false;
    }

    source_path
      .parent()
      .is_some_and(|config_dir| self.normalized_dirs.contains(&normalized_path(config_dir)))
  }

  fn insert_dir(&mut self, dir: &Path) {
    self.normalized_dirs.insert(normalized_path(dir));
  }
}

fn cached_source_path_looks_safe(path: &Path) -> bool {
  path.is_absolute()
    && !path
      .components()
      .any(|component| matches!(component, std::path::Component::ParentDir))
}

impl CargoConfigSourceFingerprint {
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

fn cargo_config_machine_cache_paths(
  tauri_dir: &Path,
) -> Result<serializer::machine::MachineCachePaths, MachineCacheError> {
  paths_for_project_cache(
    tauri_dir,
    "tauri",
    "cargo-config",
    &tauri_dir.join("Cargo.toml"),
  )
}

fn cargo_config_machine_cache_schema() -> MachineCacheSchema {
  MachineCacheSchema {
    name: "dx.tauri.cli.cargo_config",
    version: CARGO_CONFIG_MACHINE_CACHE_SCHEMA_VERSION,
    kind: MachineCacheKind::Config,
  }
}

fn cargo_home_path() -> Option<PathBuf> {
  env::var_os("CARGO_HOME").map(PathBuf::from)
}

fn normalized_cargo_home() -> Option<String> {
  cargo_home_path().as_deref().map(normalized_path)
}

fn normalized_path(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
  use super::{
    normalized_path, CargoConfigAllowedSourceDirs, CargoConfigSourceFingerprint,
    SourcePathExclusions,
  };
  use std::path::Path;

  #[test]
  fn dx_cargo_config_source_fingerprint_matches_when_only_mtime_drifts() {
    let cached = CargoConfigSourceFingerprint {
      path: "G:/Dx/app/.cargo/config".into(),
      present: true,
      bytes: 42,
      modified_unix_ms: Some(1),
      blake3: [7; 32],
    };
    let current = CargoConfigSourceFingerprint {
      modified_unix_ms: Some(2),
      ..cached.clone()
    };

    assert!(current.matches(&cached));
  }

  #[test]
  fn dx_cargo_config_cache_rejects_implausible_cached_source_path() {
    let tauri_dir = Path::new(r"G:\Dx\app\src-tauri");
    let cargo_home = Path::new(r"G:\Users\Computer\.cargo");
    let allowed_dirs = CargoConfigAllowedSourceDirs::from_context(tauri_dir, Some(cargo_home));
    let app_config_toml = Path::new(r"G:\Dx\app").join(".cargo").join("config.toml");

    assert!(allowed_dirs.contains_source_path(&tauri_dir.join(".cargo").join("config")));
    assert!(allowed_dirs.contains_source_path(&app_config_toml));
    assert!(allowed_dirs.contains_source_path(&cargo_home.join("config.toml")));
    assert!(!allowed_dirs.contains_source_path(Path::new(r"G:\Dx\unrelated\.cargo\config")));
    assert!(!allowed_dirs
      .contains_source_path(Path::new(r"G:\Dx\app\src-tauri\..\outside\.cargo\config")));
    assert!(!allowed_dirs.contains_source_path(Path::new(r"G:\Dx\app\.cargo\Cargo.toml")));
  }

  #[test]
  fn dx_cargo_config_cache_indexes_allowed_source_dirs() {
    let tauri_dir = Path::new(r"G:\Dx\app\src-tauri");
    let cargo_home = Path::new(r"G:\Users\Computer\.cargo");
    let allowed_dirs = CargoConfigAllowedSourceDirs::from_context(tauri_dir, Some(cargo_home));
    let app_config_toml = Path::new(r"G:\Dx\app").join(".cargo").join("config.toml");

    assert!(allowed_dirs.contains_source_path(&tauri_dir.join(".cargo").join("config")));
    assert!(allowed_dirs.contains_source_path(&app_config_toml));
    assert!(allowed_dirs.contains_source_path(&cargo_home.join("config.toml")));
    assert!(!allowed_dirs.contains_source_path(Path::new(r"G:\Dx\unrelated\.cargo\config")));
    assert!(!allowed_dirs
      .contains_source_path(Path::new(r"G:\Dx\app\src-tauri\..\outside\.cargo\config")));
    assert!(!allowed_dirs.contains_source_path(Path::new(r"G:\Dx\app\.cargo\Cargo.toml")));
  }

  #[test]
  fn dx_cargo_config_cache_precomputes_source_exclusions() {
    let tauri_dir = Path::new(r"G:\Dx\app\src-tauri");
    let app_manifest_path = tauri_dir.join("Cargo.toml");
    let exclusions = SourcePathExclusions::new(tauri_dir, &app_manifest_path);

    assert!(exclusions.contains(&normalized_path(&app_manifest_path)));
    assert!(exclusions.contains(&normalized_path(&tauri_dir.join("Cargo.toml"))));
    assert!(!exclusions.contains(&normalized_path(
      &Path::new(r"G:\Dx\app").join("Cargo.toml")
    )));
  }
}
