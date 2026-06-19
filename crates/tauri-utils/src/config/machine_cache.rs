// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Optional typed `.machine` caches for parsed Tauri JSON configuration values.

use std::{
  env, fs,
  io::Read,
  path::{Path, PathBuf},
};

use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde_json::Value;
#[cfg(feature = "dx-machine-cache-mmap")]
use serializer::machine::open_typed_machine_cache;
use serializer::machine::{
  access_typed_machine_cache, paths_for_project_cache, source_fingerprint,
  write_typed_machine_cache, MachineCacheCodec, MachineCacheError, MachineCacheKind,
  MachineCacheSchema, MachineCacheSource, MachineCacheWriteOptions,
};

use crate::{config::parse, platform::Target};

/// Environment variable that enables local Tauri config `.machine` cache reads and writes.
pub const TAURI_DX_MACHINE_CACHE_ENV: &str = "TAURI_DX_MACHINE_CACHE";
/// Environment variable that disables automatic cache writes when set to a false-like value.
pub const TAURI_DX_MACHINE_CACHE_WRITE_ENV: &str = "TAURI_DX_MACHINE_CACHE_WRITE";

const TAURI_CONFIG_MACHINE_CACHE_SCHEMA_VERSION: u32 = 1;
const TAURI_CONFIG_MACHINE_CACHE_MAX_BYTES: u64 = 16 * 1024 * 1024;
const TAURI_CONFIG_JSON_TREE_MAX_NODES: usize = 32 * 1024;
const TAURI_CONFIG_JSON_TREE_MAX_DEPTH: usize = 256;

/// A merged Tauri project config read from source files or a validated `.machine` cache.
#[derive(Debug, Clone, PartialEq)]
pub struct TauriProjectConfigMachineRead {
  /// The merged configuration value.
  pub config: Value,
  /// The authoritative base configuration path.
  pub config_path: PathBuf,
  /// The platform-specific extension value and source path, when present.
  pub platform_config: Option<(Value, PathBuf)>,
  /// The bundle identifier from the base config before platform or CLI extension merging.
  pub original_identifier: Option<String>,
}

/// A projected value read from a validated project config `.machine` cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TauriConfigProjectedValue {
  /// The requested path does not exist.
  Missing,
  /// The requested path exists and contains a string.
  String(String),
  /// The requested path exists, but does not contain a string.
  PresentNonString,
}

/// A small projection from a validated project config `.machine` cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TauriProjectConfigMachineProjection {
  /// The authoritative base configuration path.
  pub config_path: PathBuf,
  /// Projected values in the same order as the requested paths.
  pub values: Vec<TauriConfigProjectedValue>,
  /// The bundle identifier from the base config before platform or CLI extension merging.
  pub original_identifier: Option<String>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct TauriConfigValueMachineCache {
  source_path: String,
  value: TauriConfigJsonTree,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct TauriProjectConfigMachineCache {
  config_path: String,
  platform_config_path: Option<String>,
  sources: Vec<TauriConfigSourceFingerprint>,
  original_identifier: Option<String>,
  merged_config: TauriConfigJsonTree,
  platform_config: Option<TauriConfigJsonTree>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct TauriConfigSourceFingerprint {
  path: String,
  bytes: u64,
  modified_unix_ms: Option<u64>,
  blake3: [u8; 32],
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct TauriConfigJsonTree {
  root: u32,
  nodes: Vec<TauriConfigJsonValue>,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
enum TauriConfigJsonValue {
  Null,
  Bool(bool),
  Number(String),
  String(String),
  Array(Vec<u32>),
  Object(Vec<TauriConfigObjectField>),
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq)]
#[rkyv(compare(PartialEq), derive(Debug))]
struct TauriConfigObjectField {
  key: String,
  value: u32,
}

/// Parse a Tauri config value, using a local typed `.machine` cache only when explicitly enabled.
pub fn parse_value_with_machine_cache(
  target: Target,
  path: impl Into<PathBuf>,
) -> Result<(Value, PathBuf), parse::ConfigError> {
  parse_value_with_machine_cache_with_options(target, path, true)
}

fn parse_value_with_machine_cache_with_options(
  target: Target,
  path: impl Into<PathBuf>,
  write_leaf_cache_on_miss: bool,
) -> Result<(Value, PathBuf), parse::ConfigError> {
  let path = path.into();
  if !machine_cache_enabled() {
    return parse::parse_value(target, path);
  }

  if let Some(source_path) = parse::existing_config_source_path(target, &path) {
    let cacheable = cacheable_existing_config_source_path(&source_path);
    if cacheable {
      if let Some(project_root) = source_path.parent() {
        if let Some(value) = read_json_value_machine_cache(project_root, &source_path) {
          return Ok((value, source_path));
        }
      }
    }

    let (value, config_path) = parse::parse_existing_value(target, source_path)?;
    if cacheable && write_leaf_cache_on_miss && machine_cache_writes_enabled() {
      if let Some(project_root) = config_path.parent() {
        let _ = write_json_value_machine_cache(project_root, &config_path, &value);
      }
    }
    return Ok((value, config_path));
  }

  let (value, config_path) = parse::parse_value(target, path)?;
  if cacheable_existing_config_source_path(&config_path)
    && write_leaf_cache_on_miss
    && machine_cache_writes_enabled()
  {
    if let Some(project_root) = config_path.parent() {
      let _ = write_json_value_machine_cache(project_root, &config_path, &value);
    }
  }
  Ok((value, config_path))
}

/// Read a platform-specific Tauri config value with the same cache policy as
/// [`parse_value_with_machine_cache`].
pub fn read_platform_with_machine_cache(
  target: Target,
  root_dir: &Path,
) -> Result<Option<(Value, PathBuf)>, parse::ConfigError> {
  read_platform_with_machine_cache_with_options(target, root_dir, true)
}

fn read_platform_with_machine_cache_with_options(
  target: Target,
  root_dir: &Path,
  write_leaf_cache_on_miss: bool,
) -> Result<Option<(Value, PathBuf)>, parse::ConfigError> {
  let platform_config_path = parse::platform_config_file_path(target, root_dir);
  if parse::does_supported_file_name_exist(target, &platform_config_path) {
    parse_value_with_machine_cache_with_options(
      target,
      platform_config_path,
      write_leaf_cache_on_miss,
    )
    .map(Some)
  } else {
    Ok(None)
  }
}

/// Read the base and platform JSON configuration as one merged value.
///
/// This cache is used only for the common no-`--config` path. Dynamic CLI merge
/// patches stay on the existing parser path because they are process-local input,
/// not stable source files.
pub fn read_project_with_machine_cache(
  target: Target,
  root_dir: &Path,
) -> Result<TauriProjectConfigMachineRead, parse::ConfigError> {
  if !machine_cache_enabled() {
    return read_project_from_source(target, root_dir);
  }

  let base_source = if let Some(config_path) = parse::existing_base_config_path(root_dir) {
    if cacheable_existing_config_source_path(&config_path) {
      if let Some(read) = read_project_config_machine_cache(target, root_dir, &config_path) {
        return Ok(read);
      }
      source_fingerprint(&config_path).ok()
    } else {
      None
    }
  } else {
    None
  };

  let read = read_project_from_source(target, root_dir)?;
  if project_config_is_cacheable(&read) && machine_cache_writes_enabled() {
    let _ = match &base_source {
      Some(source)
        if normalized_source_path(&source.path) == normalized_source_path(&read.config_path) =>
      {
        write_project_config_machine_cache_with_source(root_dir, &read, source)
      }
      _ => write_project_config_machine_cache(root_dir, &read),
    };
  }
  Ok(read)
}

/// Read an already-materialized merged project config from `.machine`.
///
/// Unlike [`read_project_with_machine_cache`], this is a read-only fast path: it
/// never parses source files and never writes cache files. Callers that cannot
/// use the cache hit should fall back to the normal config loader.
pub fn read_cached_project_with_machine_cache(
  target: Target,
  root_dir: &Path,
) -> Option<TauriProjectConfigMachineRead> {
  if !machine_cache_enabled() {
    return None;
  }

  let config_path = parse::existing_base_config_path(root_dir)?;
  if !cacheable_existing_config_source_path(&config_path) {
    return None;
  }

  read_project_config_machine_cache(target, root_dir, &config_path)
}

/// Read selected project config values from a validated `.machine` cache without
/// reconstructing the full JSON value.
pub fn read_cached_project_config_projection_with_machine_cache(
  target: Target,
  root_dir: &Path,
  paths: &[&[&str]],
) -> Option<TauriProjectConfigMachineProjection> {
  if !machine_cache_enabled() {
    return None;
  }

  let config_path = parse::existing_base_config_path(root_dir)?;
  if !cacheable_existing_config_source_path(&config_path) {
    return None;
  }

  read_project_config_projection_machine_cache(target, root_dir, &config_path, paths)
}

/// Read a cached JSON configuration value when the source fingerprint and schema match.
pub fn read_json_value_machine_cache(project_root: &Path, source_path: &Path) -> Option<Value> {
  let paths = tauri_config_machine_cache_paths(project_root, source_path).ok()?;
  if !machine_cache_file_is_candidate(&paths.machine, TAURI_CONFIG_MACHINE_CACHE_MAX_BYTES) {
    return None;
  }
  let source = source_fingerprint(source_path).ok()?;

  #[cfg(feature = "dx-machine-cache-mmap")]
  {
    if let Ok(mapped) = open_typed_machine_cache::<TauriConfigValueMachineCache>(
      &paths,
      &source,
      tauri_config_machine_cache_schema(),
    ) {
      return value_from_archive(mapped.archived(), source_path);
    }
  }

  let bytes = read_machine_file_bounded(&paths.machine, TAURI_CONFIG_MACHINE_CACHE_MAX_BYTES)?;
  let archived = access_typed_machine_cache::<TauriConfigValueMachineCache>(
    &bytes,
    &source,
    tauri_config_machine_cache_schema(),
  )
  .ok()?;
  value_from_archive(archived, source_path)
}

/// Write a typed JSON configuration value cache beside a Tauri project.
pub fn write_json_value_machine_cache(
  project_root: &Path,
  source_path: &Path,
  value: &Value,
) -> Result<PathBuf, MachineCacheError> {
  let source = source_fingerprint(source_path)?;
  let paths = tauri_config_machine_cache_paths(project_root, source_path)?;
  let payload = TauriConfigValueMachineCache::from_value(source_path, value).ok_or_else(|| {
    MachineCacheError::InvalidCachePath("Tauri config JSON value tree is too large".into())
  })?;
  let receipt = write_typed_machine_cache(
    &payload,
    &source,
    &paths,
    tauri_config_machine_cache_schema(),
    MachineCacheWriteOptions {
      codec: MachineCacheCodec::None,
    },
  )?;
  Ok(receipt.machine)
}

fn read_project_from_source(
  target: Target,
  root_dir: &Path,
) -> Result<TauriProjectConfigMachineRead, parse::ConfigError> {
  let (mut config, config_path) = parse_value_with_machine_cache_with_options(
    target,
    parse::base_config_file_path(root_dir),
    false,
  )?;
  let original_identifier = config
    .as_object()
    .and_then(|config| config.get("identifier")?.as_str())
    .map(ToString::to_string);
  let platform_config = read_platform_with_machine_cache_with_options(target, root_dir, false)?;
  if let Some((platform_config, _)) = &platform_config {
    json_patch::merge(&mut config, platform_config);
  }

  Ok(TauriProjectConfigMachineRead {
    config,
    config_path,
    platform_config,
    original_identifier,
  })
}

fn read_project_config_machine_cache(
  target: Target,
  project_root: &Path,
  config_path: &Path,
) -> Option<TauriProjectConfigMachineRead> {
  let paths = tauri_project_config_machine_cache_paths(project_root, config_path).ok()?;
  if !machine_cache_file_is_candidate(&paths.machine, TAURI_CONFIG_MACHINE_CACHE_MAX_BYTES) {
    return None;
  }
  let source = source_fingerprint(config_path).ok()?;

  #[cfg(feature = "dx-machine-cache-mmap")]
  {
    if let Ok(mapped) = open_typed_machine_cache::<TauriProjectConfigMachineCache>(
      &paths,
      &source,
      tauri_project_config_machine_cache_schema(),
    ) {
      return project_config_from_archive(
        mapped.archived(),
        target,
        project_root,
        config_path,
        &source,
      );
    }
  }

  let bytes = read_machine_file_bounded(&paths.machine, TAURI_CONFIG_MACHINE_CACHE_MAX_BYTES)?;
  let archived = access_typed_machine_cache::<TauriProjectConfigMachineCache>(
    &bytes,
    &source,
    tauri_project_config_machine_cache_schema(),
  )
  .ok()?;
  project_config_from_archive(archived, target, project_root, config_path, &source)
}

fn read_project_config_projection_machine_cache(
  target: Target,
  project_root: &Path,
  config_path: &Path,
  paths: &[&[&str]],
) -> Option<TauriProjectConfigMachineProjection> {
  let cache_paths = tauri_project_config_machine_cache_paths(project_root, config_path).ok()?;
  if !machine_cache_file_is_candidate(&cache_paths.machine, TAURI_CONFIG_MACHINE_CACHE_MAX_BYTES) {
    return None;
  }
  let source = source_fingerprint(config_path).ok()?;

  #[cfg(feature = "dx-machine-cache-mmap")]
  {
    if let Ok(mapped) = open_typed_machine_cache::<TauriProjectConfigMachineCache>(
      &cache_paths,
      &source,
      tauri_project_config_machine_cache_schema(),
    ) {
      return project_config_projection_from_archive(
        mapped.archived(),
        target,
        project_root,
        config_path,
        &source,
        paths,
      );
    }
  }

  let bytes =
    read_machine_file_bounded(&cache_paths.machine, TAURI_CONFIG_MACHINE_CACHE_MAX_BYTES)?;
  let archived = access_typed_machine_cache::<TauriProjectConfigMachineCache>(
    &bytes,
    &source,
    tauri_project_config_machine_cache_schema(),
  )
  .ok()?;
  project_config_projection_from_archive(
    archived,
    target,
    project_root,
    config_path,
    &source,
    paths,
  )
}

fn write_project_config_machine_cache(
  project_root: &Path,
  read: &TauriProjectConfigMachineRead,
) -> Result<PathBuf, MachineCacheError> {
  let source = source_fingerprint(&read.config_path)?;
  write_project_config_machine_cache_with_source(project_root, read, &source)
}

fn write_project_config_machine_cache_with_source(
  project_root: &Path,
  read: &TauriProjectConfigMachineRead,
  source: &MachineCacheSource,
) -> Result<PathBuf, MachineCacheError> {
  let paths = tauri_project_config_machine_cache_paths(project_root, &read.config_path)?;
  let payload = TauriProjectConfigMachineCache::from_read(read, source).ok_or_else(|| {
    MachineCacheError::InvalidCachePath(
      "Tauri project config machine payload is invalid or too large".into(),
    )
  })?;
  let receipt = write_typed_machine_cache(
    &payload,
    source,
    &paths,
    tauri_project_config_machine_cache_schema(),
    MachineCacheWriteOptions {
      codec: MachineCacheCodec::None,
    },
  )?;
  Ok(receipt.machine)
}

/// Returns whether local Tauri `.machine` cache reads and writes are enabled.
pub fn machine_cache_enabled() -> bool {
  env::var(TAURI_DX_MACHINE_CACHE_ENV)
    .map(|value| env_flag_is_truthy(&value))
    .unwrap_or(false)
}

/// Returns whether command paths may write `.machine` files on cache misses.
pub fn machine_cache_writes_enabled() -> bool {
  env::var(TAURI_DX_MACHINE_CACHE_WRITE_ENV)
    .map(|value| !env_flag_is_falsey(&value))
    .unwrap_or(true)
}

fn env_flag_is_truthy(value: &str) -> bool {
  matches!(
    value.trim().to_ascii_lowercase().as_str(),
    "1" | "true" | "yes" | "on"
  )
}

fn env_flag_is_falsey(value: &str) -> bool {
  matches!(
    value.trim().to_ascii_lowercase().as_str(),
    "0" | "false" | "no" | "off"
  )
}

fn cacheable_existing_config_source_path(path: &Path) -> bool {
  path
    .extension()
    .and_then(|extension| extension.to_str())
    .is_some_and(cacheable_config_extension)
}

fn cacheable_config_extension(extension: &str) -> bool {
  extension.eq_ignore_ascii_case("json")
    || (cfg!(feature = "config-json5") && extension.eq_ignore_ascii_case("json5"))
    || (cfg!(feature = "config-toml") && extension.eq_ignore_ascii_case("toml"))
}

fn project_config_is_cacheable(read: &TauriProjectConfigMachineRead) -> bool {
  cacheable_existing_config_source_path(&read.config_path)
    && read
      .platform_config
      .as_ref()
      .map(|(_, path)| cacheable_existing_config_source_path(path))
      .unwrap_or(true)
}

fn tauri_config_machine_cache_paths(
  project_root: &Path,
  source_path: &Path,
) -> Result<serializer::machine::MachineCachePaths, MachineCacheError> {
  let cache_name = cache_name_for_source_path(source_path)?;
  paths_for_project_cache(project_root, "tauri", &cache_name, source_path)
}

fn tauri_project_config_machine_cache_paths(
  project_root: &Path,
  source_path: &Path,
) -> Result<serializer::machine::MachineCachePaths, MachineCacheError> {
  paths_for_project_cache(project_root, "tauri", "project-config", source_path)
}

fn cache_name_for_source_path(source_path: &Path) -> Result<String, MachineCacheError> {
  let file_name = source_path
    .file_name()
    .and_then(|file_name| file_name.to_str())
    .ok_or_else(|| MachineCacheError::InvalidCacheName(source_path.display().to_string()))?;

  let mut cache_name = String::with_capacity(file_name.len());
  let mut last_was_dash = false;
  for character in file_name.chars() {
    if character.is_ascii_alphanumeric() {
      cache_name.push(character.to_ascii_lowercase());
      last_was_dash = false;
    } else if !last_was_dash {
      cache_name.push('-');
      last_was_dash = true;
    }
  }

  let cache_name = cache_name.trim_matches('-').to_string();
  if cache_name.is_empty() {
    Err(MachineCacheError::InvalidCacheName(
      source_path.display().to_string(),
    ))
  } else {
    Ok(cache_name)
  }
}

fn tauri_config_machine_cache_schema() -> MachineCacheSchema {
  MachineCacheSchema {
    name: config_value_machine_schema_name(),
    version: TAURI_CONFIG_MACHINE_CACHE_SCHEMA_VERSION,
    kind: MachineCacheKind::Config,
  }
}

fn tauri_project_config_machine_cache_schema() -> MachineCacheSchema {
  MachineCacheSchema {
    name: project_config_machine_schema_name(),
    version: TAURI_CONFIG_MACHINE_CACHE_SCHEMA_VERSION,
    kind: MachineCacheKind::Config,
  }
}

fn config_value_machine_schema_name() -> &'static str {
  match (
    cfg!(feature = "config-json5"),
    cfg!(feature = "config-toml"),
  ) {
    (true, true) => "dx.tauri.config_value.normalized.with_json5_toml",
    (true, false) => "dx.tauri.config_value.normalized.with_json5",
    (false, true) => "dx.tauri.config_value.normalized.with_toml",
    (false, false) => "dx.tauri.config_value.normalized.json",
  }
}

fn project_config_machine_schema_name() -> &'static str {
  match (
    cfg!(feature = "config-json5"),
    cfg!(feature = "config-toml"),
  ) {
    (true, true) => "dx.tauri.project_config.normalized.with_json5_toml",
    (true, false) => "dx.tauri.project_config.normalized.with_json5",
    (false, true) => "dx.tauri.project_config.normalized.with_toml",
    (false, false) => "dx.tauri.project_config.normalized.json",
  }
}

fn value_from_archive(
  archived: &ArchivedTauriConfigValueMachineCache,
  source_path: &Path,
) -> Option<Value> {
  let archived_source_path = archived.source_path.as_str();
  if archived_source_path != normalized_source_path(source_path) {
    return None;
  }
  archived_json_tree_to_value(&archived.value)
}

fn project_config_from_archive(
  archived: &ArchivedTauriProjectConfigMachineCache,
  target: Target,
  project_root: &Path,
  config_path: &Path,
  base_source: &MachineCacheSource,
) -> Option<TauriProjectConfigMachineRead> {
  let archived_config_path = archived.config_path.as_str();
  if archived_config_path != normalized_source_path(config_path) {
    return None;
  }
  if !current_project_sources_match_archived(archived, target, project_root, base_source) {
    return None;
  }

  let config = archived_json_tree_to_value(&archived.merged_config)?;
  let platform_config_path = archived
    .platform_config_path
    .as_ref()
    .map(|path| PathBuf::from(path.as_str()));
  let platform_config = match (archived.platform_config.as_ref(), platform_config_path) {
    (Some(config), Some(path)) => Some((archived_json_tree_to_value(config)?, path)),
    (None, None) => None,
    _ => return None,
  };

  Some(TauriProjectConfigMachineRead {
    config,
    config_path: config_path.to_path_buf(),
    platform_config,
    original_identifier: archived
      .original_identifier
      .as_ref()
      .map(|identifier| identifier.as_str().to_string()),
  })
}

fn project_config_projection_from_archive(
  archived: &ArchivedTauriProjectConfigMachineCache,
  target: Target,
  project_root: &Path,
  config_path: &Path,
  base_source: &MachineCacheSource,
  paths: &[&[&str]],
) -> Option<TauriProjectConfigMachineProjection> {
  let archived_config_path = archived.config_path.as_str();
  if archived_config_path != normalized_source_path(config_path) {
    return None;
  }
  if !current_project_sources_match_archived(archived, target, project_root, base_source) {
    return None;
  }
  if !archived_json_tree_is_well_formed(&archived.merged_config) {
    return None;
  }

  let values = paths
    .iter()
    .map(|path| archived_json_tree_projected_value_at_path_unchecked(&archived.merged_config, path))
    .collect::<Option<Vec<_>>>()?;

  Some(TauriProjectConfigMachineProjection {
    config_path: config_path.to_path_buf(),
    values,
    original_identifier: archived
      .original_identifier
      .as_ref()
      .map(|identifier| identifier.as_str().to_string()),
  })
}

fn current_project_sources_match_archived(
  cache: &ArchivedTauriProjectConfigMachineCache,
  target: Target,
  project_root: &Path,
  base_source: &MachineCacheSource,
) -> bool {
  let cache_config_path = cache.config_path.as_str();
  let current_platform = parse::existing_platform_config_path(target, project_root)
    .map(|path| normalized_source_path(&path));
  if current_platform.as_deref()
    != cache
      .platform_config_path
      .as_ref()
      .map(|path| path.as_str())
  {
    return false;
  }

  let mut expected_sources = vec![cache_config_path.to_string()];
  if let Some(platform_config_path) = &current_platform {
    expected_sources.push(platform_config_path.clone());
  }
  if cache.sources.len() != expected_sources.len()
    || !expected_sources.iter().all(|expected| {
      cache
        .sources
        .iter()
        .any(|source| source.path.as_str() == expected)
    })
  {
    return false;
  }

  cache.sources.iter().all(|source| {
    let Some(source) = source_fingerprint_from_archive(source) else {
      return false;
    };
    if source.path == cache_config_path {
      source.matches_source(base_source)
    } else {
      source.matches_current()
    }
  })
}

fn source_fingerprint_from_archive(
  source: &ArchivedTauriConfigSourceFingerprint,
) -> Option<TauriConfigSourceFingerprint> {
  let mut blake3 = [0; 32];
  for (target, value) in blake3.iter_mut().zip(source.blake3.iter()) {
    *target = *value;
  }

  Some(TauriConfigSourceFingerprint {
    path: source.path.as_str().to_string(),
    bytes: source.bytes.to_native(),
    modified_unix_ms: source
      .modified_unix_ms
      .as_ref()
      .map(|value| value.to_native()),
    blake3,
  })
}

#[cfg(test)]
fn current_project_sources_match(
  cache: &TauriProjectConfigMachineCache,
  target: Target,
  project_root: &Path,
  base_source: &MachineCacheSource,
) -> bool {
  let current_platform = parse::existing_platform_config_path(target, project_root)
    .map(|path| normalized_source_path(&path));
  if current_platform != cache.platform_config_path {
    return false;
  }

  let mut expected_sources = vec![cache.config_path.clone()];
  if let Some(platform_config_path) = &current_platform {
    expected_sources.push(platform_config_path.clone());
  }
  if cache.sources.len() != expected_sources.len()
    || !expected_sources
      .iter()
      .all(|expected| cache.sources.iter().any(|source| &source.path == expected))
  {
    return false;
  }

  cache.sources.iter().all(|source| {
    if source.path == cache.config_path {
      source.matches_source(base_source)
    } else {
      source.matches_current()
    }
  })
}

fn archived_json_tree_to_value(tree: &ArchivedTauriConfigJsonTree) -> Option<Value> {
  if !archived_json_tree_is_well_formed(tree) {
    return None;
  }
  archived_json_value_at_depth(tree, tree.root.to_native(), 0)
}

fn archived_json_value_at_depth(
  tree: &ArchivedTauriConfigJsonTree,
  index: u32,
  depth: usize,
) -> Option<Value> {
  if depth > TAURI_CONFIG_JSON_TREE_MAX_DEPTH {
    return None;
  }
  match tree.nodes.get(index as usize)? {
    ArchivedTauriConfigJsonValue::Null => Some(Value::Null),
    ArchivedTauriConfigJsonValue::Bool(value) => Some(Value::Bool(*value)),
    ArchivedTauriConfigJsonValue::Number(value) => value
      .as_str()
      .parse::<serde_json::Number>()
      .ok()
      .map(Value::Number),
    ArchivedTauriConfigJsonValue::String(value) => Some(Value::String(value.as_str().to_string())),
    ArchivedTauriConfigJsonValue::Array(values) => values
      .iter()
      .map(|value| archived_json_value_at_depth(tree, value.to_native(), depth + 1))
      .collect::<Option<Vec<_>>>()
      .map(Value::Array),
    ArchivedTauriConfigJsonValue::Object(fields) => {
      let mut object = serde_json::Map::with_capacity(fields.len());
      for field in fields.iter() {
        object.insert(
          field.key.as_str().to_string(),
          archived_json_value_at_depth(tree, field.value.to_native(), depth + 1)?,
        );
      }
      Some(Value::Object(object))
    }
  }
}

fn archived_json_tree_projected_value_at_path_unchecked(
  tree: &ArchivedTauriConfigJsonTree,
  path: &[&str],
) -> Option<TauriConfigProjectedValue> {
  let mut current = tree.root.to_native();
  for segment in path {
    let Some(node) = tree.nodes.get(current as usize) else {
      return Some(TauriConfigProjectedValue::PresentNonString);
    };
    let ArchivedTauriConfigJsonValue::Object(fields) = node else {
      return Some(TauriConfigProjectedValue::PresentNonString);
    };
    let Some(field) = fields.iter().find(|field| field.key.as_str() == *segment) else {
      return Some(TauriConfigProjectedValue::Missing);
    };
    current = field.value.to_native();
  }

  match tree.nodes.get(current as usize)? {
    ArchivedTauriConfigJsonValue::String(value) => Some(TauriConfigProjectedValue::String(
      value.as_str().to_string(),
    )),
    _ => Some(TauriConfigProjectedValue::PresentNonString),
  }
}

fn archived_json_tree_is_well_formed(tree: &ArchivedTauriConfigJsonTree) -> bool {
  let root = tree.root.to_native();
  let Ok(root_index) = usize::try_from(root) else {
    return false;
  };
  root_index < tree.nodes.len()
    && tree.nodes.len() <= TAURI_CONFIG_JSON_TREE_MAX_NODES
    && archived_json_node_is_well_formed(&tree.nodes, root, 0)
}

fn archived_json_node_is_well_formed(
  nodes: &[ArchivedTauriConfigJsonValue],
  index: u32,
  depth: usize,
) -> bool {
  if depth > TAURI_CONFIG_JSON_TREE_MAX_DEPTH {
    return false;
  }
  let Some(node) = nodes.get(index as usize) else {
    return false;
  };
  match node {
    ArchivedTauriConfigJsonValue::Null
    | ArchivedTauriConfigJsonValue::Bool(_)
    | ArchivedTauriConfigJsonValue::Number(_)
    | ArchivedTauriConfigJsonValue::String(_) => true,
    ArchivedTauriConfigJsonValue::Array(values) => values.iter().all(|value| {
      let value = value.to_native();
      value < index && archived_json_node_is_well_formed(nodes, value, depth + 1)
    }),
    ArchivedTauriConfigJsonValue::Object(fields) => fields.iter().all(|field| {
      let value = field.value.to_native();
      value < index && archived_json_node_is_well_formed(nodes, value, depth + 1)
    }),
  }
}

fn normalized_source_path(source_path: &Path) -> String {
  source_path.to_string_lossy().replace('\\', "/")
}

fn read_machine_file_bounded(path: &Path, max_bytes: u64) -> Option<Vec<u8>> {
  let file = fs::File::open(path).ok()?;
  let capacity = usize::try_from(max_bytes.min(256 * 1024)).ok()?;
  let mut bytes = Vec::with_capacity(capacity);
  file
    .take(max_bytes.saturating_add(1))
    .read_to_end(&mut bytes)
    .ok()?;
  if u64::try_from(bytes.len()).ok()? > max_bytes {
    None
  } else {
    Some(bytes)
  }
}

fn machine_cache_file_is_candidate(path: &Path, max_bytes: u64) -> bool {
  fs::metadata(path)
    .ok()
    .is_some_and(|metadata| metadata.is_file() && metadata.len() <= max_bytes)
}

impl TauriConfigValueMachineCache {
  fn from_value(source_path: &Path, value: &Value) -> Option<Self> {
    Some(Self {
      source_path: normalized_source_path(source_path),
      value: TauriConfigJsonTree::from_value(value)?,
    })
  }
}

impl TauriProjectConfigMachineCache {
  fn from_read(
    read: &TauriProjectConfigMachineRead,
    base_source: &MachineCacheSource,
  ) -> Option<Self> {
    if normalized_source_path(&read.config_path) != normalized_source_path(&base_source.path) {
      return None;
    }
    let mut sources = vec![TauriConfigSourceFingerprint::from_source(base_source)];
    if let Some((_, platform_config_path)) = &read.platform_config {
      let platform_source = source_fingerprint(platform_config_path).ok()?;
      sources.push(TauriConfigSourceFingerprint::from_source(&platform_source));
    }

    Some(Self {
      config_path: normalized_source_path(&read.config_path),
      platform_config_path: read
        .platform_config
        .as_ref()
        .map(|(_, path)| normalized_source_path(path)),
      sources,
      original_identifier: read.original_identifier.clone(),
      merged_config: TauriConfigJsonTree::from_value(&read.config)?,
      platform_config: match &read.platform_config {
        Some((value, _)) => Some(TauriConfigJsonTree::from_value(value)?),
        None => None,
      },
    })
  }
}

impl TauriConfigSourceFingerprint {
  fn from_source(source: &MachineCacheSource) -> Self {
    Self {
      path: normalized_source_path(&source.path),
      bytes: source.bytes,
      modified_unix_ms: source.modified_unix_ms,
      blake3: source.blake3,
    }
  }

  fn matches_current(&self) -> bool {
    let Ok(source) = source_fingerprint(Path::new(&self.path)) else {
      return false;
    };
    self.matches_source(&source)
  }

  fn matches_source(&self, source: &MachineCacheSource) -> bool {
    self.path == normalized_source_path(&source.path)
      && self.bytes == source.bytes
      && self.blake3 == source.blake3
  }
}

impl TauriConfigJsonTree {
  fn from_value(value: &Value) -> Option<Self> {
    let mut nodes = Vec::new();
    let root = push_tauri_config_json_node(value, &mut nodes, 0)?;
    Some(Self { root, nodes })
  }

  fn into_json_value(self) -> Option<Value> {
    if !self.is_well_formed() {
      return None;
    }
    self.value_at(self.root)
  }

  #[cfg(test)]
  fn projected_value_at_path(&self, path: &[&str]) -> TauriConfigProjectedValue {
    if !self.is_well_formed() {
      return TauriConfigProjectedValue::PresentNonString;
    }

    let mut current = self.root;
    for segment in path {
      let Some(node) = self.nodes.get(current as usize) else {
        return TauriConfigProjectedValue::PresentNonString;
      };
      let TauriConfigJsonValue::Object(fields) = node else {
        return TauriConfigProjectedValue::PresentNonString;
      };
      let Some(field) = fields.iter().find(|field| field.key == *segment) else {
        return TauriConfigProjectedValue::Missing;
      };
      current = field.value;
    }

    match self.nodes.get(current as usize) {
      Some(TauriConfigJsonValue::String(value)) => TauriConfigProjectedValue::String(value.clone()),
      Some(_) => TauriConfigProjectedValue::PresentNonString,
      None => TauriConfigProjectedValue::PresentNonString,
    }
  }

  fn is_well_formed(&self) -> bool {
    let Ok(root) = usize::try_from(self.root) else {
      return false;
    };
    root < self.nodes.len()
      && self.nodes.len() <= TAURI_CONFIG_JSON_TREE_MAX_NODES
      && self.node_is_well_formed(self.root, 0)
  }

  fn node_is_well_formed(&self, index: u32, depth: usize) -> bool {
    if depth > TAURI_CONFIG_JSON_TREE_MAX_DEPTH {
      return false;
    }
    let Some(node) = self.nodes.get(index as usize) else {
      return false;
    };
    match node {
      TauriConfigJsonValue::Null
      | TauriConfigJsonValue::Bool(_)
      | TauriConfigJsonValue::Number(_)
      | TauriConfigJsonValue::String(_) => true,
      TauriConfigJsonValue::Array(values) => values
        .iter()
        .all(|value| *value < index && self.node_is_well_formed(*value, depth + 1)),
      TauriConfigJsonValue::Object(fields) => fields
        .iter()
        .all(|field| field.value < index && self.node_is_well_formed(field.value, depth + 1)),
    }
  }

  fn value_at(&self, index: u32) -> Option<Value> {
    self.value_at_depth(index, 0)
  }

  fn value_at_depth(&self, index: u32, depth: usize) -> Option<Value> {
    if depth > TAURI_CONFIG_JSON_TREE_MAX_DEPTH {
      return None;
    }
    match self.nodes.get(index as usize)? {
      TauriConfigJsonValue::Null => Some(Value::Null),
      TauriConfigJsonValue::Bool(value) => Some(Value::Bool(*value)),
      TauriConfigJsonValue::Number(value) => {
        value.parse::<serde_json::Number>().ok().map(Value::Number)
      }
      TauriConfigJsonValue::String(value) => Some(Value::String(value.clone())),
      TauriConfigJsonValue::Array(values) => values
        .iter()
        .map(|value| self.value_at_depth(*value, depth + 1))
        .collect::<Option<Vec<_>>>()
        .map(Value::Array),
      TauriConfigJsonValue::Object(fields) => fields
        .iter()
        .map(|field| {
          Some((
            field.key.clone(),
            self.value_at_depth(field.value, depth + 1)?,
          ))
        })
        .collect::<Option<serde_json::Map<_, _>>>()
        .map(Value::Object),
    }
  }
}

fn push_tauri_config_json_node(
  value: &Value,
  nodes: &mut Vec<TauriConfigJsonValue>,
  depth: usize,
) -> Option<u32> {
  if depth > TAURI_CONFIG_JSON_TREE_MAX_DEPTH || nodes.len() >= TAURI_CONFIG_JSON_TREE_MAX_NODES {
    return None;
  }
  let node = match value {
    Value::Null => TauriConfigJsonValue::Null,
    Value::Bool(value) => TauriConfigJsonValue::Bool(*value),
    Value::Number(value) => TauriConfigJsonValue::Number(value.to_string()),
    Value::String(value) => TauriConfigJsonValue::String(value.clone()),
    Value::Array(values) => {
      let children = values
        .iter()
        .map(|value| push_tauri_config_json_node(value, nodes, depth + 1))
        .collect::<Option<Vec<_>>>()?;
      TauriConfigJsonValue::Array(children)
    }
    Value::Object(fields) => {
      let fields = fields
        .iter()
        .map(|(key, value)| {
          Some(TauriConfigObjectField {
            key: key.clone(),
            value: push_tauri_config_json_node(value, nodes, depth + 1)?,
          })
        })
        .collect::<Option<Vec<_>>>()?;
      TauriConfigJsonValue::Object(fields)
    }
  };
  if nodes.len() >= TAURI_CONFIG_JSON_TREE_MAX_NODES {
    return None;
  }
  let index = u32::try_from(nodes.len()).ok()?;
  nodes.push(node);
  Some(index)
}

#[cfg(test)]
#[path = "machine_cache_representative_fixture.rs"]
mod machine_cache_representative_fixture;

#[cfg(test)]
mod tests {
  use super::*;
  use machine_cache_representative_fixture::{
    representative_platform_config_fixture, representative_project_config_fixture,
  };
  use serde_json::json;
  use serial_test::serial;
  use std::time::{Instant, SystemTime, UNIX_EPOCH};

  struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
  }

  impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
      let previous = env::var(key).ok();
      env::set_var(key, value);
      Self { key, previous }
    }

    fn remove(key: &'static str) -> Self {
      let previous = env::var(key).ok();
      env::remove_var(key);
      Self { key, previous }
    }
  }

  impl Drop for EnvGuard {
    fn drop(&mut self) {
      if let Some(previous) = &self.previous {
        env::set_var(self.key, previous);
      } else {
        env::remove_var(self.key);
      }
    }
  }

  #[test]
  #[serial]
  fn dx_machine_cache_json_tree_projects_string_paths() {
    let tree = TauriConfigJsonTree::from_value(&json!({
      "productName": "DX Inspect",
      "bundle": {
        "windows": {
          "wix": {
            "upgradeCode": "11111111-2222-3333-4444-555555555555",
            "template": 42
          }
        }
      }
    }))
    .expect("cacheable JSON tree");

    assert_eq!(
      tree.projected_value_at_path(&["productName"]),
      TauriConfigProjectedValue::String("DX Inspect".into())
    );
    assert_eq!(
      tree.projected_value_at_path(&["bundle", "windows", "wix", "upgradeCode"]),
      TauriConfigProjectedValue::String("11111111-2222-3333-4444-555555555555".into())
    );
    assert_eq!(
      tree.projected_value_at_path(&["bundle", "windows", "wix", "template"]),
      TauriConfigProjectedValue::PresentNonString
    );
    assert_eq!(
      tree.projected_value_at_path(&["bundle", "windows", "wix", "missing"]),
      TauriConfigProjectedValue::Missing
    );
    assert_eq!(
      tree.projected_value_at_path(&["bundle", "windows", "wix", "template", "path"]),
      TauriConfigProjectedValue::PresentNonString
    );
  }

  #[test]
  #[serial]
  fn dx_machine_cache_project_config_projection_reads_requested_paths() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("project-config-projection");
    let config_path = project.path().join("tauri.conf.json");
    write_json(
      &config_path,
      &json!({
        "identifier": "com.dx.cache",
        "productName": "DX Inspect",
        "bundle": {
          "windows": {
            "wix": {
              "upgradeCode": "11111111-2222-3333-4444-555555555555",
              "template": 42
            }
          }
        },
        "build": { "devUrl": "http://localhost:3000" },
        "app": { "windows": [{ "title": "DX Cache", "width": 800, "height": 600 }] }
      }),
    );

    read_project_with_machine_cache(Target::current(), project.path())
      .expect("prime project cache");
    let projection = read_cached_project_config_projection_with_machine_cache(
      Target::current(),
      project.path(),
      &[
        &["productName"][..],
        &["bundle", "windows", "wix", "upgradeCode"][..],
        &["bundle", "windows", "wix", "template"][..],
        &["bundle", "windows", "wix", "missing"][..],
      ],
    )
    .expect("read projection");

    assert_eq!(projection.config_path, config_path);
    assert_eq!(
      projection.values,
      vec![
        TauriConfigProjectedValue::String("DX Inspect".into()),
        TauriConfigProjectedValue::String("11111111-2222-3333-4444-555555555555".into()),
        TauriConfigProjectedValue::PresentNonString,
        TauriConfigProjectedValue::Missing,
      ]
    );
  }

  #[test]
  #[serial]
  fn dx_machine_cache_project_config_projection_uses_merged_platform_config() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("project-config-projection-platform");
    write_json(
      &project.path().join("tauri.conf.json"),
      &json!({
        "identifier": "com.dx.cache",
        "productName": "Base Product",
        "build": { "devUrl": "http://localhost:3000" },
        "app": { "windows": [{ "title": "DX Cache", "width": 800, "height": 600 }] }
      }),
    );
    write_json(
      &project.path().join("tauri.windows.conf.json"),
      &json!({
        "productName": "Windows Product",
        "bundle": {
          "windows": {
            "wix": {
              "upgradeCode": "11111111-2222-3333-4444-555555555555"
            }
          }
        }
      }),
    );

    read_project_with_machine_cache(Target::Windows, project.path()).expect("prime project cache");
    let projection = read_cached_project_config_projection_with_machine_cache(
      Target::Windows,
      project.path(),
      &[
        &["productName"][..],
        &["bundle", "windows", "wix", "upgradeCode"][..],
      ],
    )
    .expect("read projection");

    assert_eq!(
      projection.values,
      vec![
        TauriConfigProjectedValue::String("Windows Product".into()),
        TauriConfigProjectedValue::String("11111111-2222-3333-4444-555555555555".into()),
      ]
    );
  }

  #[test]
  #[serial]
  fn dx_machine_cache_project_config_projection_write_env_zero_still_reads_hit() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let _write_env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_WRITE_ENV, "0");
    let project = temp_project("project-config-projection-read-only-hit");
    write_json(
      &project.path().join("tauri.conf.json"),
      &json!({
        "identifier": "com.dx.cache",
        "productName": "DX Inspect",
        "bundle": {
          "windows": {
            "wix": {
              "upgradeCode": "11111111-2222-3333-4444-555555555555"
            }
          }
        },
        "build": { "devUrl": "http://localhost:3000" },
        "app": { "windows": [{ "title": "DX Cache", "width": 800, "height": 600 }] }
      }),
    );
    let read = read_project_from_source(Target::current(), project.path())
      .expect("read project source without automatic write");
    write_project_config_machine_cache(project.path(), &read).expect("prime project cache");

    let projection = read_cached_project_config_projection_with_machine_cache(
      Target::current(),
      project.path(),
      &[
        &["productName"][..],
        &["bundle", "windows", "wix", "upgradeCode"][..],
      ],
    )
    .expect("read projection with writes disabled");

    assert_eq!(
      projection.values,
      vec![
        TauriConfigProjectedValue::String("DX Inspect".into()),
        TauriConfigProjectedValue::String("11111111-2222-3333-4444-555555555555".into()),
      ]
    );
  }

  #[test]
  #[serial]
  fn dx_machine_cache_project_config_projection_default_off_returns_none() {
    let enabled = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("project-config-projection-default-off");
    write_json(
      &project.path().join("tauri.conf.json"),
      &json!({
        "identifier": "com.dx.cache",
        "productName": "DX Inspect",
        "build": { "devUrl": "http://localhost:3000" },
        "app": { "windows": [{ "title": "DX Cache", "width": 800, "height": 600 }] }
      }),
    );
    read_project_with_machine_cache(Target::current(), project.path())
      .expect("prime project cache");
    drop(enabled);
    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);

    assert!(read_cached_project_config_projection_with_machine_cache(
      Target::current(),
      project.path(),
      &[&["productName"][..]],
    )
    .is_none());
  }

  #[test]
  #[serial]
  fn dx_machine_cache_project_config_projection_rejects_stale_base_source() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("project-config-projection-base-stale");
    let config_path = project.path().join("tauri.conf.json");
    write_json(
      &config_path,
      &json!({
        "identifier": "com.dx.cache",
        "productName": "First Product",
        "build": { "devUrl": "http://localhost:3000" },
        "app": { "windows": [{ "title": "DX Cache", "width": 800, "height": 600 }] }
      }),
    );
    read_project_with_machine_cache(Target::current(), project.path())
      .expect("prime project cache");
    write_json(
      &config_path,
      &json!({
        "identifier": "com.dx.cache",
        "productName": "Changed Product",
        "build": { "devUrl": "http://localhost:3000" },
        "app": { "windows": [{ "title": "DX Cache", "width": 800, "height": 600 }] }
      }),
    );

    assert!(read_cached_project_config_projection_with_machine_cache(
      Target::current(),
      project.path(),
      &[&["productName"][..]],
    )
    .is_none());
  }

  #[test]
  #[serial]
  fn dx_machine_cache_project_config_projection_rejects_when_platform_file_appears() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("project-config-projection-platform-appears");
    write_json(
      &project.path().join("tauri.conf.json"),
      &json!({
        "identifier": "com.dx.cache",
        "productName": "DX Inspect",
        "build": { "devUrl": "http://localhost:3000" },
        "app": { "windows": [{ "title": "DX Cache", "width": 800, "height": 600 }] }
      }),
    );
    read_project_with_machine_cache(Target::Windows, project.path()).expect("prime project cache");
    write_json(
      &project.path().join("tauri.windows.conf.json"),
      &json!({ "productName": "Windows Product" }),
    );

    assert!(read_cached_project_config_projection_with_machine_cache(
      Target::Windows,
      project.path(),
      &[&["productName"][..]],
    )
    .is_none());
  }

  #[test]
  #[serial]
  fn dx_machine_cache_project_config_projection_rejects_changed_platform_source() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("project-config-projection-platform-stale");
    write_json(
      &project.path().join("tauri.conf.json"),
      &json!({
        "identifier": "com.dx.cache",
        "productName": "Base Product",
        "build": { "devUrl": "http://localhost:3000" },
        "app": { "windows": [{ "title": "DX Cache", "width": 800, "height": 600 }] }
      }),
    );
    write_json(
      &project.path().join("tauri.windows.conf.json"),
      &json!({ "productName": "Windows Product" }),
    );

    read_project_with_machine_cache(Target::Windows, project.path()).expect("prime project cache");
    write_json(
      &project.path().join("tauri.windows.conf.json"),
      &json!({ "productName": "Changed Product" }),
    );

    assert!(read_cached_project_config_projection_with_machine_cache(
      Target::Windows,
      project.path(),
      &[&["productName"][..]],
    )
    .is_none());
  }

  #[test]
  #[serial]
  fn dx_machine_cache_bounded_machine_read_rejects_oversized_files() {
    let project = temp_project("bounded-machine-read");
    let machine_path = project.path().join("oversized.machine");

    fs::write(&machine_path, b"abcd").expect("write oversized machine fixture");

    assert_eq!(read_machine_file_bounded(&machine_path, 3), None);
    assert_eq!(
      read_machine_file_bounded(&machine_path, 4),
      Some(b"abcd".to_vec())
    );
  }

  #[test]
  #[serial]
  fn dx_machine_cache_round_trips_json_config_value_when_source_matches() {
    let project = temp_project("round-trip");
    let config_path = project.path().join("tauri.conf.json");
    let config = json!({
      "identifier": "com.dx.cache",
      "productName": "DX Cache",
      "version": "1.0.0",
      "build": { "beforeDevCommand": "", "devUrl": "http://localhost:3000" },
      "app": { "windows": [{ "title": "DX Cache", "width": 800, "height": 600 }] }
    });
    write_json(&config_path, &config);

    let machine = write_json_value_machine_cache(project.path(), &config_path, &config)
      .expect("write config machine cache");

    assert_eq!(
      machine,
      tauri_machine_path(project.path(), "tauri-conf-json")
    );
    assert!(machine.exists());
    assert_eq!(
      read_json_value_machine_cache(project.path(), &config_path),
      Some(config)
    );
  }

  #[test]
  #[serial]
  fn dx_machine_cache_rejects_stale_source_and_parse_uses_authoritative_json() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("stale-source");
    let config_path = project.path().join("tauri.conf.json");
    let first = json!({"identifier": "com.dx.first", "productName": "First"});
    write_json(&config_path, &first);
    write_json_value_machine_cache(project.path(), &config_path, &first)
      .expect("write config machine cache");

    let second = json!({"identifier": "com.dx.second", "productName": "Second"});
    write_json(&config_path, &second);

    assert_eq!(
      read_json_value_machine_cache(project.path(), &config_path),
      None
    );
    let (parsed, parsed_path) = parse_value_with_machine_cache(Target::current(), &config_path)
      .expect("parse authoritative JSON");
    assert_eq!(parsed, second);
    assert_eq!(parsed_path, config_path);
  }

  #[test]
  #[serial]
  fn dx_machine_cache_parse_is_default_off_until_env_enables_it() {
    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
    let project = temp_project("default-off");
    let config_path = project.path().join("tauri.conf.json");
    let config = json!({"identifier": "com.dx.defaultoff", "productName": "Default Off"});
    write_json(&config_path, &config);

    let (cached_parse, _) = parse_value_with_machine_cache(Target::current(), &config_path)
      .expect("parse JSON with helper");
    let (plain_parse, _) =
      parse::parse_value(Target::current(), &config_path).expect("parse JSON normally");

    assert_eq!(cached_parse, plain_parse);
    assert!(!tauri_machine_path(project.path(), "tauri-conf-json").exists());
  }

  #[test]
  #[serial]
  fn dx_machine_cache_default_off_ignores_matching_cache_payload() {
    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
    let project = temp_project("default-off-ignores-cache");
    let config_path = project.path().join("tauri.conf.json");
    let source_config = json!({"identifier": "com.dx.source", "productName": "Source"});
    let cached_config = json!({"identifier": "com.dx.cached", "productName": "Cached"});
    write_json(&config_path, &source_config);
    write_json_value_machine_cache(project.path(), &config_path, &cached_config)
      .expect("write deliberately different machine payload");

    let (parsed, parsed_path) = parse_value_with_machine_cache(Target::current(), &config_path)
      .expect("parse source with cache disabled");

    assert_eq!(parsed, source_config);
    assert_eq!(parsed_path, config_path);
    assert!(tauri_machine_path(project.path(), "tauri-conf-json").exists());
  }

  #[test]
  #[serial]
  fn dx_machine_cache_writes_json5_and_toml_config_value_caches() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("json5-toml");

    let json5_path = project.path().join("tauri.conf.json5");
    fs::write(
      &json5_path,
      "{identifier:'com.dx.json5',productName:'Json5'}",
    )
    .expect("write json5 config");
    let (json5_value, json5_used_path) =
      parse_value_with_machine_cache(Target::current(), project.path().join("tauri.conf.json"))
        .expect("parse JSON5 config");
    assert_eq!(json5_value["identifier"], "com.dx.json5");
    assert_eq!(json5_used_path, json5_path);
    assert!(tauri_machine_path(project.path(), "tauri-conf-json5").exists());

    fs::remove_file(&json5_used_path).expect("remove json5 config");
    let toml_path = project.path().join("Tauri.toml");
    fs::write(
      &toml_path,
      r#"
identifier = "com.dx.toml"
productName = "Toml"
"#,
    )
    .expect("write toml config");
    let (toml_value, toml_used_path) =
      parse_value_with_machine_cache(Target::current(), project.path().join("tauri.conf.json"))
        .expect("parse TOML config");
    assert_eq!(toml_value["identifier"], "com.dx.toml");
    assert_eq!(toml_used_path, toml_path);
    assert!(tauri_machine_path(project.path(), "tauri-toml").exists());
  }

  #[test]
  #[serial]
  fn dx_machine_cache_rejects_stale_json5_source() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("stale-json5");
    let json5_path = project.path().join("tauri.conf.json5");
    fs::write(
      &json5_path,
      "{identifier:'com.dx.json5.one',productName:'Json5 One'}",
    )
    .expect("write json5 config");

    let (first, _) =
      parse_value_with_machine_cache(Target::current(), project.path().join("tauri.conf.json"))
        .expect("parse JSON5 config");

    assert_eq!(first["identifier"], "com.dx.json5.one");
    assert!(tauri_machine_path(project.path(), "tauri-conf-json5").exists());

    fs::write(
      &json5_path,
      "{identifier:'com.dx.json5.two',productName:'Json5 Two'}",
    )
    .expect("rewrite json5 config");

    assert_eq!(
      read_json_value_machine_cache(project.path(), &json5_path),
      None
    );
    let (second, _) =
      parse_value_with_machine_cache(Target::current(), project.path().join("tauri.conf.json"))
        .expect("parse changed JSON5 config");
    assert_eq!(second["identifier"], "com.dx.json5.two");
  }

  #[test]
  #[serial]
  fn dx_machine_cache_reads_json5_cache_from_json_seed_path() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("json5-cache-hit-from-json-seed");
    let seed_path = project.path().join("tauri.conf.json");
    let json5_path = project.path().join("tauri.conf.json5");
    fs::write(
      &json5_path,
      "{identifier:'com.dx.json5.source',productName:'Json5 Source'}",
    )
    .expect("write json5 config");
    write_json_value_machine_cache(
      project.path(),
      &json5_path,
      &json!({"identifier": "com.dx.json5.cached", "productName": "Json5 Cached"}),
    )
    .expect("write deliberately different JSON5 cache");

    let (value, used_path) =
      parse_value_with_machine_cache(Target::current(), &seed_path).expect("parse JSON5 cache");

    assert_eq!(value["identifier"], "com.dx.json5.cached");
    assert_eq!(used_path, json5_path);
  }

  #[test]
  #[serial]
  fn dx_machine_cache_reads_toml_cache_from_json_seed_path() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("toml-cache-hit-from-json-seed");
    let seed_path = project.path().join("tauri.conf.json");
    let toml_path = project.path().join("Tauri.toml");
    fs::write(
      &toml_path,
      r#"
identifier = "com.dx.toml.source"
productName = "Toml Source"
"#,
    )
    .expect("write TOML config");
    write_json_value_machine_cache(
      project.path(),
      &toml_path,
      &json!({"identifier": "com.dx.toml.cached", "productName": "Toml Cached"}),
    )
    .expect("write deliberately different TOML cache");

    let (value, used_path) =
      parse_value_with_machine_cache(Target::current(), &seed_path).expect("parse TOML cache");

    assert_eq!(value["identifier"], "com.dx.toml.cached");
    assert_eq!(used_path, toml_path);
  }

  fn receipt_output_dir_stays_under_dx_test_outputs(output_dir: &Path) -> bool {
    let root = Path::new(r"G:\Dx\test-outputs");
    if !output_dir.is_absolute()
      || output_dir
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
      return false;
    }
    if !output_dir.starts_with(root) {
      return false;
    }

    let Ok(root_canonical) = fs::canonicalize(root) else {
      return false;
    };
    let mut ancestor = output_dir;
    loop {
      if ancestor.exists() {
        return fs::canonicalize(ancestor)
          .map(|path| path.starts_with(&root_canonical))
          .unwrap_or(false);
      }

      let Some(parent) = ancestor.parent() else {
        return false;
      };
      ancestor = parent;
    }
  }

  #[test]
  fn dx_machine_cache_rejects_receipt_output_parent_traversal() {
    let escaped = PathBuf::from(r"G:\Dx\test-outputs\..\test-outputs-bad");
    let sibling = PathBuf::from(r"G:\Dx\test-outputs-bad");
    assert!(
      !receipt_output_dir_stays_under_dx_test_outputs(&escaped),
      "receipt output guard must reject parent traversal outside G:\\Dx\\test-outputs"
    );
    assert!(
      !receipt_output_dir_stays_under_dx_test_outputs(&sibling),
      "receipt output guard must reject sibling prefixes outside G:\\Dx\\test-outputs"
    );
  }

  #[test]
  #[ignore = "manual DX receipt generator; writes under G:\\Dx\\test-outputs"]
  #[serial]
  fn dx_machine_cache_writes_source_parse_vs_machine_read_receipt() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let output_dir =
      PathBuf::from(env::var_os("DX_TEST_OUTPUT_DIR").expect("DX_TEST_OUTPUT_DIR must be set"));
    assert!(
      receipt_output_dir_stays_under_dx_test_outputs(&output_dir),
      "receipt output must stay under G:\\Dx\\test-outputs"
    );

    let project = temp_project("source-vs-machine-receipt");
    let config_path = project.path().join("tauri.conf.json");
    write_json(
      &config_path,
      &json!({
        "identifier": "com.dx.receipt",
        "productName": "Receipt",
        "version": "1.0.0",
        "build": { "devUrl": "http://localhost:3000" },
        "app": { "windows": [{ "title": "Receipt", "width": 800, "height": 600 }] }
      }),
    );

    let receipt_path = write_config_source_vs_machine_timing_receipt(
      project.path(),
      &config_path,
      &output_dir,
      8,
      2,
    )
    .expect("write source-vs-machine timing receipt");
    let receipt: Value = serde_json::from_slice(&fs::read(&receipt_path).expect("read receipt"))
      .expect("receipt JSON");

    assert!(receipt_path.starts_with(&output_dir));
    assert_eq!(
      receipt["schema"],
      "dx.tauri.config_source_parse_vs_machine_read_receipt"
    );
    assert_eq!(receipt["format"], "json");
    assert_eq!(receipt["source_json_authoritative"], true);
    assert_eq!(receipt["machine_read_validated"], true);
    assert_eq!(receipt["machine_read_includes_source_fingerprint"], true);
    assert_eq!(receipt["faster_than_upstream_claimed"], false);
    assert_eq!(receipt["upstream_baseline_measured"], false);
    assert_eq!(receipt["full_cli_speed_claimed"], false);
    assert_eq!(receipt["release_build_run"], false);
    assert!(receipt.get("upstream_speedup").is_none());
    assert!(receipt.get("speedup_vs_upstream").is_none());
    assert_eq!(receipt["source_parse"]["label"], "source_parse");
    assert_eq!(receipt["source_parse"]["iterations"], 8);
    assert_eq!(
      receipt["validated_machine_read"]["label"],
      "validated_machine_read"
    );
    assert_eq!(receipt["validated_machine_read"]["iterations"], 8);
    assert_eq!(receipt["validated_machine_read"]["validated"], true);
    assert_eq!(
      receipt["validated_machine_read"]["source_fingerprint_checked"],
      true
    );
    assert!(receipt["source_bytes"].as_u64().expect("source bytes") > 0);
    assert!(receipt["machine_bytes"].as_u64().expect("machine bytes") > 0);
    assert_eq!(
      receipt["source_bytes"].as_u64(),
      Some(fs::metadata(&config_path).expect("source metadata").len())
    );
    assert_eq!(
      receipt["machine_bytes"].as_u64(),
      Some(
        fs::metadata(tauri_machine_path(project.path(), "tauri-conf-json"))
          .expect("machine metadata")
          .len()
      )
    );
    assert_eq!(receipt["cache_write_included_in_timing"], false);
    assert_eq!(receipt["fallback_used"], false);
    assert!(
      receipt["validated_machine_read"]["median_ns"]
        .as_u64()
        .expect("machine median")
        > 0
    );
  }

  #[test]
  #[ignore = "manual DX receipt generator; writes under G:\\Dx\\test-outputs"]
  #[serial]
  fn dx_machine_cache_writes_project_config_source_vs_machine_read_receipt() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let output_dir =
      PathBuf::from(env::var_os("DX_TEST_OUTPUT_DIR").expect("DX_TEST_OUTPUT_DIR must be set"));
    assert!(
      receipt_output_dir_stays_under_dx_test_outputs(&output_dir),
      "receipt output must stay under G:\\Dx\\test-outputs"
    );

    let project = temp_project("project-source-vs-machine-receipt");
    let config_path = project.path().join("tauri.conf.json");
    let platform_path = parse::platform_config_file_path(Target::current(), project.path());
    write_json(
      &config_path,
      &json!({
        "identifier": "com.dx.project.receipt",
        "productName": "Project Receipt",
        "version": "1.0.0",
        "build": { "devUrl": "http://localhost:3000" },
        "app": { "windows": [{ "title": "Base", "width": 800, "height": 600 }] }
      }),
    );
    write_json(
      &platform_path,
      &json!({
        "productName": "Project Platform",
        "app": { "windows": [{ "title": "Platform", "width": 1024, "height": 768 }] }
      }),
    );

    let receipt_path = write_project_config_source_vs_machine_timing_receipt(
      project.path(),
      &config_path,
      &output_dir,
      8,
      2,
    )
    .expect("write project-config source-vs-machine timing receipt");
    let receipt: Value = serde_json::from_slice(&fs::read(&receipt_path).expect("read receipt"))
      .expect("receipt JSON");

    assert!(receipt_path.starts_with(&output_dir));
    assert_eq!(
      receipt["schema"],
      "dx.tauri.project_config_source_vs_machine_read_receipt"
    );
    assert_eq!(receipt["cache_boundary"], "full project config");
    assert_eq!(receipt["machine_cache_generation_measured"], false);
    assert_eq!(receipt["cache_write_included_in_timing"], false);
    assert_eq!(receipt["machine_cache_write_env_for_timing"], "0");
    assert_eq!(receipt["machine_file_unchanged_during_timing"], true);
    assert_eq!(receipt["machine_read_validated"], true);
    assert_eq!(receipt["full_cli_speed_claimed"], false);
    assert_eq!(receipt["faster_than_upstream_claimed"], false);
    assert!(receipt["source_parse"]["median_ns"].as_u64().unwrap_or(0) > 0);
    assert!(
      receipt["validated_machine_read"]["median_ns"]
        .as_u64()
        .unwrap_or(0)
        > 0
    );
  }

  #[test]
  #[ignore = "manual DX receipt generator; writes under G:\\Dx\\test-outputs"]
  #[serial]
  fn dx_machine_cache_writes_representative_project_config_source_vs_machine_read_receipt() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let output_dir =
      PathBuf::from(env::var_os("DX_TEST_OUTPUT_DIR").expect("DX_TEST_OUTPUT_DIR must be set"));
    assert!(
      receipt_output_dir_stays_under_dx_test_outputs(&output_dir),
      "receipt output must stay under G:\\Dx\\test-outputs"
    );

    let project = temp_project("representative-project-source-vs-machine-receipt");
    let config_path = project.path().join("tauri.conf.json");
    let platform_path = parse::platform_config_file_path(Target::current(), project.path());
    write_json(&config_path, &representative_project_config_fixture());
    write_json(&platform_path, &representative_platform_config_fixture());

    let receipt_path = write_project_config_source_vs_machine_timing_receipt_with_metadata(
      project.path(),
      &config_path,
      &output_dir,
      10,
      2,
      ProjectConfigTimingReceiptMetadata {
        fixture_label: "representative generated project config",
        format: "representative-project-json-with-platform-json",
        receipt_file_name: "tauri-representative-project-config-source-vs-machine-receipt.json",
        command: "cargo test -p tauri-utils --jobs 1 --features dx-machine-cache,dx-machine-cache-mmap,config-json5,config-toml dx_machine_cache_writes_representative_project_config_source_vs_machine_read_receipt --lib -- --ignored --test-threads=1 --nocapture",
      },
    )
    .expect("write representative project-config source-vs-machine timing receipt");
    let receipt: Value = serde_json::from_slice(&fs::read(&receipt_path).expect("read receipt"))
      .expect("receipt JSON");

    assert!(receipt_path.starts_with(&output_dir));
    assert_eq!(
      receipt["schema"],
      "dx.tauri.project_config_source_vs_machine_read_receipt"
    );
    assert_eq!(
      receipt["fixture_label"],
      "representative generated project config"
    );
    assert_eq!(
      receipt["format"],
      "representative-project-json-with-platform-json"
    );
    assert_eq!(receipt["machine_cache_generation_measured"], false);
    assert_eq!(receipt["cache_write_included_in_timing"], false);
    assert_eq!(receipt["machine_cache_write_env_for_timing"], "0");
    assert_eq!(receipt["machine_file_unchanged_during_timing"], true);
    assert_eq!(receipt["machine_read_validated"], true);
    assert_eq!(receipt["full_cli_speed_claimed"], false);
    assert_eq!(receipt["faster_than_upstream_claimed"], false);
    assert!(
      receipt["source_total_bytes"].as_u64().unwrap_or(0) > 50_000,
      "representative fixture should be large enough to avoid tiny-config conclusions"
    );
    assert!(receipt["source_parse"]["median_ns"].as_u64().unwrap_or(0) > 0);
    assert!(
      receipt["validated_machine_read"]["median_ns"]
        .as_u64()
        .unwrap_or(0)
        > 0
    );
  }

  #[test]
  #[serial]
  fn dx_machine_cache_default_off_does_not_write_json5_or_toml_caches() {
    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
    let project = temp_project("json5-toml-default-off");

    let json5_path = project.path().join("tauri.conf.json5");
    fs::write(
      &json5_path,
      "{identifier:'com.dx.defaultoff.json5',productName:'Json5'}",
    )
    .expect("write json5 config");
    parse_value_with_machine_cache(Target::current(), project.path().join("tauri.conf.json"))
      .expect("parse JSON5 config");
    assert!(!tauri_machine_path(project.path(), "tauri-conf-json5").exists());

    fs::remove_file(&json5_path).expect("remove json5 config");
    let toml_path = project.path().join("Tauri.toml");
    fs::write(
      &toml_path,
      r#"
identifier = "com.dx.defaultoff.toml"
productName = "Toml"
"#,
    )
    .expect("write toml config");
    parse_value_with_machine_cache(Target::current(), project.path().join("tauri.conf.json"))
      .expect("parse TOML config");
    assert!(!tauri_machine_path(project.path(), "tauri-toml").exists());
  }

  #[test]
  #[serial]
  fn dx_machine_cache_write_env_disables_automatic_config_writes() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let _write_env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_WRITE_ENV, "0");
    let project = temp_project("write-env-disabled");
    let config_path = project.path().join("tauri.conf.json");
    let config = json!({"identifier": "com.dx.readonly", "productName": "Read Only"});
    write_json(&config_path, &config);

    let (parsed, parsed_path) = parse_value_with_machine_cache(Target::current(), &config_path)
      .expect("parse JSON with write disabled");
    let read = read_project_with_machine_cache(Target::current(), project.path())
      .expect("read project with write disabled");

    assert_eq!(parsed, config);
    assert_eq!(parsed_path, config_path);
    assert_eq!(read.config["productName"], "Read Only");
    assert!(!tauri_machine_path(project.path(), "tauri-conf-json").exists());
    assert!(!tauri_project_machine_path(project.path()).exists());
  }

  #[test]
  #[serial]
  fn dx_machine_cache_project_config_merges_json5_base_and_toml_platform() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("project-json5-toml");
    let config_path = project.path().join("tauri.conf.json5");
    let platform_path = project
      .path()
      .join(toml_platform_file_name(Target::current()));
    fs::write(
      &config_path,
      "{identifier:'com.dx.project.json5',productName:'Json5 Base'}",
    )
    .expect("write json5 base config");
    fs::write(
      &platform_path,
      r#"
productName = "Toml Platform"
"#,
    )
    .expect("write toml platform config");

    let read = read_project_with_machine_cache(Target::current(), project.path())
      .expect("read merged JSON5/TOML project config");

    assert_eq!(read.config["identifier"], "com.dx.project.json5");
    assert_eq!(read.config["productName"], "Toml Platform");
    assert_eq!(read.config_path, config_path);
    assert_eq!(
      read.platform_config.as_ref().map(|(_, path)| path),
      Some(&platform_path)
    );
    assert!(tauri_project_machine_path(project.path()).exists());
    assert_eq!(
      read_project_config_machine_cache(Target::current(), project.path(), &config_path)
        .expect("read cached JSON5/TOML project config"),
      read
    );
  }

  #[test]
  #[serial]
  fn dx_machine_cache_project_config_merges_base_and_platform_json() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("project-merge");
    let config_path = project.path().join("tauri.conf.json");
    let platform_path = parse::platform_config_file_path(Target::current(), project.path());
    write_json(
      &config_path,
      &json!({
        "identifier": "com.dx.project",
        "productName": "Base",
        "app": { "windows": [{ "title": "Base", "width": 800, "height": 600 }] }
      }),
    );
    write_json(
      &platform_path,
      &json!({
        "productName": "Platform",
        "app": { "windows": [{ "title": "Platform", "width": 1024, "height": 768 }] }
      }),
    );

    let read = read_project_with_machine_cache(Target::current(), project.path())
      .expect("read merged project config");

    assert_eq!(read.config["identifier"], "com.dx.project");
    assert_eq!(read.config["productName"], "Platform");
    assert_eq!(read.original_identifier.as_deref(), Some("com.dx.project"));
    assert_eq!(
      read.platform_config.as_ref().map(|(_, path)| path),
      Some(&platform_path)
    );
    assert!(tauri_project_machine_path(project.path()).exists());
    assert_eq!(
      read_project_config_machine_cache(Target::current(), project.path(), &config_path)
        .expect("read cached merged project config"),
      read
    );
  }

  #[test]
  #[serial]
  fn dx_machine_cache_project_config_write_skips_leaf_value_cache_writes() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("project-skip-leaf-writes");
    let config_path = project.path().join("tauri.conf.json");
    let platform_path = parse::platform_config_file_path(Target::current(), project.path());
    write_json(
      &config_path,
      &json!({"identifier": "com.dx.project", "productName": "Base"}),
    );
    write_json(&platform_path, &json!({"productName": "Platform"}));

    let read = read_project_with_machine_cache(Target::current(), project.path())
      .expect("read merged project config");

    assert_eq!(read.config["productName"], "Platform");
    assert!(tauri_project_machine_path(project.path()).exists());
    assert!(!tauri_machine_path(project.path(), "tauri-conf-json").exists());
    assert!(!tauri_machine_path(
      project.path(),
      &cache_name_for_source_path(&platform_path).expect("platform cache name")
    )
    .exists());
  }

  #[test]
  #[serial]
  fn dx_machine_cache_cached_project_read_is_read_only() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("project-read-only-hit");
    let config_path = project.path().join("tauri.conf.json");
    write_json(
      &config_path,
      &json!({"identifier": "com.dx.project", "productName": "Cached Read"}),
    );

    assert_eq!(
      read_cached_project_with_machine_cache(Target::current(), project.path()),
      None
    );
    assert!(!tauri_project_machine_path(project.path()).exists());

    let source_read = read_project_with_machine_cache(Target::current(), project.path())
      .expect("prime project config machine cache");
    let cached_read = read_cached_project_with_machine_cache(Target::current(), project.path())
      .expect("read cached project config");

    assert_eq!(cached_read, source_read);
    assert!(tauri_project_machine_path(project.path()).exists());
  }

  #[test]
  #[serial]
  fn dx_machine_cache_project_config_reuses_envelope_source_for_base_fingerprint() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("project-envelope-source");
    let config_path = project.path().join("tauri.conf.json");
    let platform_path = parse::platform_config_file_path(Target::current(), project.path());
    write_json(
      &config_path,
      &json!({"identifier": "com.dx.project", "productName": "Base"}),
    );
    write_json(&platform_path, &json!({"productName": "Platform"}));

    let read = read_project_with_machine_cache(Target::current(), project.path())
      .expect("read project config");
    let envelope_source = source_fingerprint(&config_path).expect("base source fingerprint");
    let cache = TauriProjectConfigMachineCache::from_read(&read, &envelope_source)
      .expect("project cache payload");

    write_json(
      &config_path,
      &json!({"identifier": "com.dx.changed", "productName": "Changed"}),
    );
    assert!(current_project_sources_match(
      &cache,
      Target::current(),
      project.path(),
      &envelope_source
    ));

    let mut stale_envelope_source = envelope_source.clone();
    stale_envelope_source.blake3[0] ^= 1;
    assert!(!current_project_sources_match(
      &cache,
      Target::current(),
      project.path(),
      &stale_envelope_source
    ));
  }

  #[test]
  fn dx_machine_cache_source_fingerprint_matches_when_only_mtime_drifts() {
    let current = TauriConfigSourceFingerprint {
      path: "G:/Dx/app/tauri.conf.json".into(),
      bytes: 42,
      modified_unix_ms: Some(2000),
      blake3: [7; 32],
    };
    let cached = MachineCacheSource {
      path: PathBuf::from("G:/Dx/app/tauri.conf.json"),
      bytes: 42,
      modified_unix_ms: Some(1000),
      blake3: [7; 32],
    };

    assert!(current.matches_source(&cached));

    let mut changed = cached;
    changed.blake3[0] ^= 1;
    assert!(!current.matches_source(&changed));
  }

  #[test]
  #[serial]
  fn dx_machine_cache_project_config_write_reuses_envelope_source_for_base_fingerprint() {
    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
    let project = temp_project("project-write-envelope-source");
    let config_path = project.path().join("tauri.conf.json");
    let platform_path = parse::platform_config_file_path(Target::current(), project.path());
    write_json(
      &config_path,
      &json!({"identifier": "com.dx.project", "productName": "Base"}),
    );
    write_json(&platform_path, &json!({"productName": "Platform"}));

    let read =
      read_project_from_source(Target::current(), project.path()).expect("read project config");
    let envelope_source = source_fingerprint(&config_path).expect("base source fingerprint");

    write_json(
      &config_path,
      &json!({"identifier": "com.dx.changed", "productName": "Changed"}),
    );
    let cache = TauriProjectConfigMachineCache::from_read(&read, &envelope_source)
      .expect("project cache payload");

    let base_source = cache
      .sources
      .iter()
      .find(|source| source.path == normalized_source_path(&config_path))
      .expect("base source entry");
    assert!(base_source.matches_source(&envelope_source));
    let changed_source = source_fingerprint(&config_path).expect("changed base source fingerprint");
    assert_ne!(changed_source.blake3, envelope_source.blake3);
    assert!(!base_source.matches_source(&changed_source));

    let mut stale_envelope_source = envelope_source.clone();
    stale_envelope_source.blake3[0] ^= 1;
    assert!(!base_source.matches_source(&stale_envelope_source));
  }

  #[test]
  #[serial]
  fn dx_machine_cache_project_config_rejects_changed_platform_source() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("project-stale-platform");
    let config_path = project.path().join("tauri.conf.json");
    let platform_path = parse::platform_config_file_path(Target::current(), project.path());
    write_json(
      &config_path,
      &json!({"identifier": "com.dx.project", "productName": "Base"}),
    );
    write_json(&platform_path, &json!({"productName": "Platform One"}));

    read_project_with_machine_cache(Target::current(), project.path())
      .expect("write merged project cache");
    write_json(&platform_path, &json!({"productName": "Platform Two"}));

    assert_eq!(
      read_project_config_machine_cache(Target::current(), project.path(), &config_path),
      None
    );
    let read = read_project_with_machine_cache(Target::current(), project.path())
      .expect("read changed platform source");
    assert_eq!(read.config["productName"], "Platform Two");
  }

  #[test]
  #[serial]
  fn dx_machine_cache_project_config_rejects_cache_when_platform_file_appears() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_project("project-platform-appears");
    let config_path = project.path().join("tauri.conf.json");
    let platform_path = parse::platform_config_file_path(Target::current(), project.path());
    write_json(
      &config_path,
      &json!({"identifier": "com.dx.project", "productName": "Base"}),
    );

    read_project_with_machine_cache(Target::current(), project.path())
      .expect("write base-only merged project cache");
    write_json(&platform_path, &json!({"productName": "Platform"}));

    assert_eq!(
      read_project_config_machine_cache(Target::current(), project.path(), &config_path),
      None
    );
    let read = read_project_with_machine_cache(Target::current(), project.path())
      .expect("read platform after it appears");
    assert_eq!(read.config["productName"], "Platform");
    assert!(read.platform_config.is_some());
  }

  #[test]
  fn dx_machine_cache_json_tree_rejects_self_references() {
    let tree = TauriConfigJsonTree {
      root: 0,
      nodes: vec![TauriConfigJsonValue::Array(vec![0])],
    };

    assert_eq!(tree.into_json_value(), None);
  }

  #[test]
  fn dx_machine_cache_json_tree_rejects_too_many_source_nodes() {
    let value = Value::Array(vec![Value::Null; TAURI_CONFIG_JSON_TREE_MAX_NODES]);

    assert!(TauriConfigJsonTree::from_value(&value).is_none());
  }

  #[test]
  fn dx_machine_cache_json_tree_rejects_too_deep_source_values() {
    let mut value = Value::Null;
    for _ in 0..=TAURI_CONFIG_JSON_TREE_MAX_DEPTH {
      value = Value::Array(vec![value]);
    }

    assert!(TauriConfigJsonTree::from_value(&value).is_none());
  }

  #[test]
  fn dx_machine_cache_json_tree_rejects_too_deep_archived_values() {
    let mut nodes = vec![TauriConfigJsonValue::Null];
    for index in 0..=TAURI_CONFIG_JSON_TREE_MAX_DEPTH {
      nodes.push(TauriConfigJsonValue::Array(vec![index as u32]));
    }
    let tree = TauriConfigJsonTree {
      root: u32::try_from(nodes.len() - 1).expect("root index"),
      nodes,
    };

    assert_eq!(tree.into_json_value(), None);
  }

  fn write_json(path: &Path, value: &Value) {
    fs::write(
      path,
      serde_json::to_vec_pretty(value).expect("serialize JSON config"),
    )
    .expect("write JSON config");
  }

  fn temp_project(name: &str) -> tempfile::TempDir {
    let base = env::var_os("DX_TEST_OUTPUT_DIR")
      .map(PathBuf::from)
      .unwrap_or_else(env::temp_dir);
    fs::create_dir_all(&base).expect("test output base");
    tempfile::Builder::new()
      .prefix(&format!("tauri-dx-machine-cache-{name}-"))
      .tempdir_in(base)
      .expect("temp project")
  }

  fn write_config_source_vs_machine_timing_receipt(
    project_root: &Path,
    config_path: &Path,
    output_dir: &Path,
    iterations: usize,
    warmups: usize,
  ) -> Result<PathBuf, Box<dyn std::error::Error>> {
    assert!(
      iterations > 0,
      "timing receipt needs at least one iteration"
    );
    if !receipt_output_dir_stays_under_dx_test_outputs(output_dir) {
      return Err(
        std::io::Error::new(
          std::io::ErrorKind::InvalidInput,
          "receipt output must stay under G:\\Dx\\test-outputs",
        )
        .into(),
      );
    }
    fs::create_dir_all(output_dir)?;

    let (source_value, source_path) = parse::parse_value(Target::current(), config_path)?;
    let (cached_value, cached_path) =
      parse_value_with_machine_cache(Target::current(), config_path)?;
    assert_eq!(source_path, cached_path);
    assert_eq!(source_value, cached_value);
    assert_eq!(
      read_json_value_machine_cache(project_root, config_path),
      Some(source_value.clone())
    );

    for _ in 0..warmups {
      let _ = parse::parse_value(Target::current(), config_path)?;
      let _ = read_json_value_machine_cache(project_root, config_path).ok_or_else(|| {
        std::io::Error::new(
          std::io::ErrorKind::NotFound,
          "missing validated machine cache",
        )
      })?;
    }

    let source_parse_ns = measure_config_read(iterations, || {
      let (value, _) = parse::parse_value(Target::current(), config_path)?;
      Ok(value)
    })?;
    let machine_read_ns = measure_config_read(iterations, || {
      read_json_value_machine_cache(project_root, config_path).ok_or_else(|| {
        std::io::Error::new(
          std::io::ErrorKind::NotFound,
          "missing validated machine cache",
        )
        .into()
      })
    })?;

    let machine_path = tauri_config_machine_cache_paths(project_root, config_path)?.machine;
    let source_summary = timing_summary("source_parse", &source_parse_ns, false, false);
    let machine_summary = timing_summary("validated_machine_read", &machine_read_ns, true, true);
    let source_median = source_summary["median_ns"].as_u64().unwrap_or(0);
    let machine_median = machine_summary["median_ns"].as_u64().unwrap_or(0);
    let generated_unix_ms = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map(|duration| duration.as_millis())
      .unwrap_or_default();
    let median_ratio_percent = if source_median == 0 {
      Value::Null
    } else {
      json!(machine_median.saturating_mul(100) / source_median)
    };

    let receipt = json!({
      "schema": "dx.tauri.config_source_parse_vs_machine_read_receipt",
      "version": 1,
      "timing_scope": "same-process helper timing for one local fixture",
      "generated_unix_ms": generated_unix_ms,
      "command": "cargo test --manifest-path G:\\Dx\\tauri\\crates\\tauri-utils\\Cargo.toml --no-default-features --features dx-machine-cache,config-json5,config-toml --lib dx_machine_cache_writes_source_parse_vs_machine_read_receipt -j1 --color never -- --ignored --test-threads=1",
      "profile": "test",
      "features": ["dx-machine-cache", "config-json5", "config-toml"],
      "format": config_format_label(config_path),
      "source_path": normalized_source_path(config_path),
      "machine_path": normalized_source_path(&machine_path),
      "source_bytes": fs::metadata(config_path)?.len(),
      "machine_bytes": fs::metadata(&machine_path)?.len(),
      "warmups": warmups,
      "source_parse": source_summary,
      "validated_machine_read": machine_summary,
      "machine_to_source_median_ratio_percent": median_ratio_percent,
      "cache_write_included_in_timing": false,
      "fallback_used": false,
      "source_json_authoritative": true,
      "source_config_authoritative": true,
      "machine_read_validated": true,
      "machine_read_includes_source_fingerprint": true,
      "machine_read_hashes_source_bytes": true,
      "machine_read_parses_source_config": false,
      "same_machine_only": true,
      "faster_than_upstream_claimed": false,
      "upstream_baseline_measured": false,
      "full_cli_speed_claimed": false,
      "release_build_run": false
    });

    let receipt_path = output_dir.join(format!(
      "tauri-config-source-vs-machine-{}-receipt.json",
      config_format_label(config_path)
    ));
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    Ok(receipt_path)
  }

  fn write_project_config_source_vs_machine_timing_receipt(
    project_root: &Path,
    config_path: &Path,
    output_dir: &Path,
    iterations: usize,
    warmups: usize,
  ) -> Result<PathBuf, Box<dyn std::error::Error>> {
    write_project_config_source_vs_machine_timing_receipt_with_metadata(
      project_root,
      config_path,
      output_dir,
      iterations,
      warmups,
      ProjectConfigTimingReceiptMetadata {
        fixture_label: "tiny project config",
        format: "project-json-with-platform-json",
        receipt_file_name: "tauri-project-config-source-vs-machine-receipt.json",
        command: "cargo test --manifest-path G:\\Dx\\tauri\\crates\\tauri-utils\\Cargo.toml --no-default-features --features dx-machine-cache,dx-machine-cache-mmap,config-json5,config-toml --lib dx_machine_cache_writes_project_config_source_vs_machine_read_receipt -j1 --color never -- --ignored --test-threads=1",
      },
    )
  }

  struct ProjectConfigTimingReceiptMetadata<'a> {
    fixture_label: &'a str,
    format: &'a str,
    receipt_file_name: &'a str,
    command: &'a str,
  }

  fn write_project_config_source_vs_machine_timing_receipt_with_metadata(
    project_root: &Path,
    config_path: &Path,
    output_dir: &Path,
    iterations: usize,
    warmups: usize,
    metadata: ProjectConfigTimingReceiptMetadata<'_>,
  ) -> Result<PathBuf, Box<dyn std::error::Error>> {
    assert!(
      iterations > 0,
      "timing receipt needs at least one iteration"
    );
    if !receipt_output_dir_stays_under_dx_test_outputs(output_dir) {
      return Err(
        std::io::Error::new(
          std::io::ErrorKind::InvalidInput,
          "receipt output must stay under G:\\Dx\\test-outputs",
        )
        .into(),
      );
    }
    fs::create_dir_all(output_dir)?;

    let setup_started = Instant::now();
    let source_read = read_project_from_source(Target::current(), project_root)?;
    let envelope_source = source_fingerprint(config_path)?;
    let machine_path =
      write_project_config_machine_cache_with_source(project_root, &source_read, &envelope_source)?;
    let machine_generation_setup_ns =
      u64::try_from(setup_started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let machine_before = fs::read(&machine_path)?;
    let source_bytes = fs::metadata(config_path)?.len();
    let platform_source_bytes = match &source_read.platform_config {
      Some((_, path)) => Some(fs::metadata(path)?.len()),
      None => None,
    };
    let source_total_bytes = source_bytes + platform_source_bytes.unwrap_or(0);
    let cached_read =
      read_project_config_machine_cache(Target::current(), project_root, config_path).ok_or_else(
        || {
          std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "missing validated project-config machine cache",
          )
        },
      )?;
    assert_eq!(source_read, cached_read);

    for _ in 0..warmups {
      let _read_env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
      let _write_env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_WRITE_ENV, "0");
      let source = read_project_from_source(Target::current(), project_root)?;
      assert_eq!(source.config_path, config_path);
      drop(_write_env);
      drop(_read_env);

      let _read_env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
      let _write_env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_WRITE_ENV, "0");
      let cached = read_cached_project_with_machine_cache(Target::current(), project_root)
        .ok_or_else(|| {
          std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "missing validated project-config machine cache",
          )
        })?;
      assert_eq!(source, cached);
    }

    let source_parse_ns = measure_config_read(iterations, || {
      let _read_env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
      let _write_env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_WRITE_ENV, "0");
      let read = read_project_from_source(Target::current(), project_root)?;
      Ok(read.config)
    })?;
    let machine_read_ns = {
      let _read_env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
      let _write_env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_WRITE_ENV, "0");
      measure_config_read(iterations, || {
        read_cached_project_with_machine_cache(Target::current(), project_root)
          .map(|read| read.config)
          .ok_or_else(|| {
            std::io::Error::new(
              std::io::ErrorKind::NotFound,
              "missing validated project-config machine cache",
            )
            .into()
          })
      })?
    };

    let machine_after = fs::read(&machine_path)?;
    let source_summary = timing_summary("source_parse", &source_parse_ns, false, false);
    let machine_summary = timing_summary("validated_machine_read", &machine_read_ns, true, true);
    let source_median = source_summary["median_ns"].as_u64().unwrap_or(0);
    let machine_median = machine_summary["median_ns"].as_u64().unwrap_or(0);
    let median_ratio_percent = if source_median == 0 {
      Value::Null
    } else {
      json!(machine_median.saturating_mul(100) / source_median)
    };
    let generated_unix_ms = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .map(|duration| duration.as_millis())
      .unwrap_or_default();

    let receipt = json!({
      "schema": "dx.tauri.project_config_source_vs_machine_read_receipt",
      "version": 1,
      "timing_scope": "same-process helper timing for one local fixture",
      "generated_unix_ms": generated_unix_ms,
      "command": metadata.command,
      "profile": "test",
      "features": ["dx-machine-cache", "dx-machine-cache-mmap", "config-json5", "config-toml"],
      "cache_boundary": "full project config",
      "fixture_label": metadata.fixture_label,
      "format": metadata.format,
      "source_path": normalized_source_path(config_path),
      "machine_path": normalized_source_path(&machine_path),
      "source_bytes": source_bytes,
      "platform_source_bytes": platform_source_bytes,
      "source_total_bytes": source_total_bytes,
      "machine_bytes": fs::metadata(&machine_path)?.len(),
      "warmups": warmups,
      "source_parse": source_summary,
      "validated_machine_read": machine_summary,
      "machine_to_source_median_ratio_percent": median_ratio_percent,
      "machine_generation_setup_ns": machine_generation_setup_ns,
      "machine_cache_generation_measured": false,
      "cache_write_included_in_timing": false,
      "source_cache_read_env_for_timing": "removed",
      "source_cache_write_env_for_timing": "0",
      "machine_cache_write_env_for_timing": "0",
      "machine_file_unchanged_during_timing": machine_before == machine_after,
      "fallback_used": false,
      "source_json_authoritative": true,
      "source_config_authoritative": true,
      "machine_read_validated": true,
      "machine_read_includes_source_fingerprint": true,
      "machine_read_hashes_source_bytes": true,
      "machine_read_parses_source_config": false,
      "same_machine_only": true,
      "faster_than_upstream_claimed": false,
      "upstream_baseline_measured": false,
      "full_cli_speed_claimed": false,
      "release_build_run": false
    });

    let receipt_path = output_dir.join(metadata.receipt_file_name);
    fs::write(&receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    Ok(receipt_path)
  }

  fn measure_config_read<F>(
    iterations: usize,
    mut read: F,
  ) -> Result<Vec<u64>, Box<dyn std::error::Error>>
  where
    F: FnMut() -> Result<Value, Box<dyn std::error::Error>>,
  {
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
      let started = Instant::now();
      let value = read()?;
      assert!(
        value.is_object(),
        "config timing fixture must parse an object"
      );
      let elapsed = started.elapsed().as_nanos();
      samples.push(u64::try_from(elapsed).unwrap_or(u64::MAX));
    }
    Ok(samples)
  }

  fn timing_summary(
    label: &str,
    samples: &[u64],
    validated: bool,
    source_fingerprint_checked: bool,
  ) -> Value {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let sum = sorted
      .iter()
      .map(|sample| u128::from(*sample))
      .sum::<u128>();
    json!({
      "label": label,
      "iterations": sorted.len(),
      "min_ns": sorted[0],
      "median_ns": sorted[sorted.len() / 2],
      "p95_ns": sorted[percentile_index(sorted.len(), 95)],
      "max_ns": sorted[sorted.len() - 1],
      "mean_ns": (sum / sorted.len() as u128) as u64,
      "validated": validated,
      "source_fingerprint_checked": source_fingerprint_checked,
      "samples_ns": samples
    })
  }

  fn percentile_index(len: usize, percentile: usize) -> usize {
    (((len * percentile) + 99) / 100)
      .saturating_sub(1)
      .min(len.saturating_sub(1))
  }

  fn config_format_label(path: &Path) -> &'static str {
    match path
      .extension()
      .and_then(|extension| extension.to_str())
      .map(|extension| extension.to_ascii_lowercase())
      .as_deref()
    {
      Some("json5") => "json5",
      Some("toml") => "toml",
      _ => "json",
    }
  }

  fn tauri_machine_path(project: &Path, cache_name: &str) -> PathBuf {
    project
      .join(".dx")
      .join("tauri")
      .join(format!("{cache_name}.machine"))
  }

  fn tauri_project_machine_path(project: &Path) -> PathBuf {
    tauri_machine_path(project, "project-config")
  }

  fn toml_platform_file_name(target: Target) -> &'static str {
    match target {
      Target::MacOS => "Tauri.macos.toml",
      Target::Windows => "Tauri.windows.toml",
      Target::Linux => "Tauri.linux.toml",
      Target::Android => "Tauri.android.toml",
      Target::Ios => "Tauri.ios.toml",
    }
  }
}
