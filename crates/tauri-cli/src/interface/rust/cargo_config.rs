// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde::Deserialize;
use std::{
  fs,
  path::{Path, PathBuf},
};

use tauri_utils::display_path;

use crate::{
  error::{Context, ErrorExt},
  Result,
};

#[cfg(feature = "dx-machine-cache")]
mod machine_cache;

struct PathAncestors<'a> {
  current: Option<&'a Path>,
}

impl<'a> PathAncestors<'a> {
  fn new(path: &'a Path) -> PathAncestors<'a> {
    PathAncestors {
      current: Some(path),
    }
  }
}

impl<'a> Iterator for PathAncestors<'a> {
  type Item = &'a Path;

  fn next(&mut self) -> Option<&'a Path> {
    if let Some(path) = self.current {
      self.current = path.parent();

      Some(path)
    } else {
      None
    }
  }
}

#[derive(Default, Deserialize)]
pub struct BuildConfig {
  target: Option<String>,
}

#[derive(Deserialize)]
pub struct ConfigSchema {
  build: Option<BuildConfig>,
}

#[derive(Default)]
pub struct Config {
  build: BuildConfig,
}

impl Config {
  pub fn load(path: &Path) -> Result<Self> {
    #[cfg(feature = "dx-machine-cache")]
    if machine_cache::enabled() {
      if let Some(config) = machine_cache::read(path) {
        return Ok(config);
      }
    }

    let config = Self::load_from_source(path)?;

    #[cfg(feature = "dx-machine-cache")]
    if machine_cache::enabled()
      && tauri_utils::config::machine_cache::machine_cache_writes_enabled()
    {
      let _ = machine_cache::write(path, &config);
    }

    Ok(config)
  }

  fn load_from_source(path: &Path) -> Result<Self> {
    let mut config = Self::default();

    let get_config = |path: PathBuf| -> Result<ConfigSchema> {
      let contents =
        fs::read_to_string(&path).fs_context("failed to read configuration file", path.clone())?;
      toml::from_str(&contents).context(format!(
        "could not parse TOML configuration in `{}`",
        display_path(&path)
      ))
    };

    for current in PathAncestors::new(path) {
      if let Some(path) = get_file_path(&current.join(".cargo"), "config", true)? {
        let toml = get_config(path)?;
        if let Some(target) = toml.build.and_then(|b| b.target) {
          config.build.target = Some(target);
          break;
        }
      }
    }

    if config.build.target.is_none() {
      if let Ok(cargo_home) = std::env::var("CARGO_HOME") {
        if let Some(path) = get_file_path(&PathBuf::from(cargo_home), "config", true)? {
          let toml = get_config(path)?;
          if let Some(target) = toml.build.and_then(|b| b.target) {
            config.build.target = Some(target);
          }
        }
      }
    }

    Ok(config)
  }

  pub fn build(&self) -> &BuildConfig {
    &self.build
  }
}

impl BuildConfig {
  pub fn target(&self) -> Option<&str> {
    self.target.as_deref()
  }
}

/// The purpose of this function is to aid in the transition to using
/// .toml extensions on Cargo's config files, which were historically not used.
/// Both 'config.toml' and 'credentials.toml' should be valid with or without extension.
/// When both exist, we want to prefer the one without an extension for
/// backwards compatibility, but warn the user appropriately.
fn get_file_path(
  dir: &Path,
  filename_without_extension: &str,
  warn: bool,
) -> Result<Option<PathBuf>> {
  let possible = dir.join(filename_without_extension);
  let possible_with_extension = dir.join(format!("{filename_without_extension}.toml"));

  if possible.exists() {
    if warn && possible_with_extension.exists() {
      // We don't want to print a warning if the version
      // without the extension is just a symlink to the version
      // WITH an extension, which people may want to do to
      // support multiple Cargo versions at once and not
      // get a warning.
      let skip_warning = if let Ok(target_path) = fs::read_link(&possible) {
        target_path == possible_with_extension
      } else {
        false
      };

      if !skip_warning {
        log::warn!(
          "Both `{}` and `{}` exist. Using `{}`",
          display_path(&possible),
          display_path(&possible_with_extension),
          display_path(&possible)
        );
      }
    }

    Ok(Some(possible))
  } else if possible_with_extension.exists() {
    Ok(Some(possible_with_extension))
  } else {
    Ok(None)
  }
}

#[cfg(all(test, feature = "dx-machine-cache"))]
mod tests {
  use super::*;
  use std::sync::{Mutex, MutexGuard, OnceLock};

  const TAURI_DX_MACHINE_CACHE_ENV: &str = "TAURI_DX_MACHINE_CACHE";
  const CARGO_HOME_ENV: &str = "CARGO_HOME";
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
  fn dx_cargo_config_machine_cache_default_off_writes_no_machine() {
    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
    let project = temp_cargo_config_project("default-off");
    write_local_cargo_config(project.path(), "source-target");

    let config = Config::load(project.path()).expect("load Cargo config");

    assert_eq!(config.build().target(), Some("source-target"));
    assert!(!machine_cache::machine_path(project.path()).exists());
  }

  #[test]
  fn dx_cargo_config_machine_cache_hit_reads_cached_target_before_source_parse() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_cargo_config_project("cache-hit");
    let cargo_dir = project.path().join(".cargo");
    std::fs::create_dir_all(&cargo_dir).expect("create .cargo");
    std::fs::write(cargo_dir.join("config"), "this is not valid TOML")
      .expect("write malformed Cargo config");
    let cached = Config {
      build: BuildConfig {
        target: Some("cached-target".into()),
      },
    };
    machine_cache::write(project.path(), &cached).expect("write Cargo config machine cache");

    let config = Config::load(project.path()).expect("load cached Cargo config");

    assert_eq!(config.build().target(), Some("cached-target"));
  }

  #[test]
  fn dx_cargo_config_machine_cache_write_env_zero_still_reads_hit() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_cargo_config_project("read-only-hit");
    write_local_cargo_config(project.path(), "source-target");
    let cached = Config {
      build: BuildConfig {
        target: Some("cached-target".into()),
      },
    };
    machine_cache::write(project.path(), &cached).expect("write Cargo config machine cache");
    let machine_path = machine_cache::machine_path(project.path());
    let machine_before = std::fs::read(&machine_path).expect("read machine cache before hit");
    drop(_env);

    let _env = EnvGuard::set_many(&[
      (TAURI_DX_MACHINE_CACHE_ENV, Some("1")),
      (
        tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
        Some("0"),
      ),
    ]);

    let config = Config::load(project.path()).expect("load cached Cargo config");

    assert_eq!(config.build().target(), Some("cached-target"));
    assert_eq!(
      machine_before,
      std::fs::read(&machine_path).expect("read machine cache after hit")
    );
  }

  #[test]
  fn dx_cargo_config_machine_cache_write_env_zero_does_not_write_on_miss() {
    let project = temp_cargo_config_project("read-only-miss");
    write_local_cargo_config(project.path(), "source-target");
    let _env = EnvGuard::set_many(&[
      (TAURI_DX_MACHINE_CACHE_ENV, Some("1")),
      (
        tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
        Some("0"),
      ),
    ]);

    let config = Config::load(project.path()).expect("load Cargo config");

    assert_eq!(config.build().target(), Some("source-target"));
    assert!(!machine_cache::machine_path(project.path()).exists());
  }

  #[test]
  fn dx_cargo_config_machine_cache_rejects_changed_local_config() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_cargo_config_project("stale-local-config");
    write_local_cargo_config(project.path(), "old-target");

    let config = Config::load(project.path()).expect("warm Cargo config cache");
    assert_eq!(config.build().target(), Some("old-target"));
    assert!(machine_cache::machine_path(project.path()).exists());

    write_local_cargo_config(project.path(), "new-target");

    assert!(machine_cache::read(project.path()).is_none());
    let config = Config::load(project.path()).expect("refresh Cargo config cache");
    assert_eq!(config.build().target(), Some("new-target"));
    assert!(machine_cache::read(project.path()).is_some());
  }

  #[test]
  fn dx_cargo_config_machine_cache_rejects_changed_cargo_home_config() {
    let project = temp_cargo_config_project("stale-cargo-home-config");
    let cargo_home = project.path().join("cargo-home");
    std::fs::create_dir_all(&cargo_home).expect("create CARGO_HOME");
    write_cargo_config_at(&cargo_home, "old-home-target");
    let cargo_home_value = cargo_home.to_string_lossy().to_string();
    let _env = EnvGuard::set_many(&[
      (TAURI_DX_MACHINE_CACHE_ENV, Some("1")),
      (CARGO_HOME_ENV, Some(cargo_home_value.as_str())),
    ]);

    let config = Config::load(project.path()).expect("warm Cargo config cache");
    assert_eq!(config.build().target(), Some("old-home-target"));

    write_cargo_config_at(&cargo_home, "new-home-target");

    assert!(machine_cache::read(project.path()).is_none());
    let config = Config::load(project.path()).expect("refresh Cargo config cache");
    assert_eq!(config.build().target(), Some("new-home-target"));
  }

  #[test]
  fn dx_cargo_config_machine_cache_skips_ambiguous_config_pair() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_cargo_config_project("ambiguous-config-pair");
    let cargo_dir = project.path().join(".cargo");
    std::fs::create_dir_all(&cargo_dir).expect("create .cargo");
    write_cargo_config_at(&cargo_dir, "legacy-target");
    std::fs::write(
      cargo_dir.join("config.toml"),
      r#"[build]
target = "toml-target"
"#,
    )
    .expect("write config.toml");

    let config = Config::load(project.path()).expect("load ambiguous Cargo config");

    assert_eq!(config.build().target(), Some("legacy-target"));
    assert!(!machine_cache::machine_path(project.path()).exists());
  }

  #[test]
  fn dx_cargo_config_machine_cache_does_not_write_empty_config() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_cargo_config_project("empty-config");

    let config = Config::load(project.path()).expect("load empty Cargo config");

    assert_eq!(config.build().target(), None);
    assert!(!machine_cache::machine_path(project.path()).exists());
  }

  fn temp_cargo_config_project(name: &str) -> tempfile::TempDir {
    let base = std::env::var_os("DX_TEST_OUTPUT_DIR")
      .map(PathBuf::from)
      .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&base).expect("create test output base");
    tempfile::Builder::new()
      .prefix(&format!("tauri-dx-cargo-config-{name}-"))
      .tempdir_in(base)
      .map(|project| {
        std::fs::write(
          project.path().join("Cargo.toml"),
          r#"[package]
name = "cargo-config-fixture"
version = "1.0.0"
"#,
        )
        .expect("write Cargo.toml");
        project
      })
      .expect("create Cargo config temp project")
  }

  fn write_local_cargo_config(dir: &Path, target: &str) {
    let cargo_dir = dir.join(".cargo");
    std::fs::create_dir_all(&cargo_dir).expect("create local .cargo");
    write_cargo_config_at(&cargo_dir, target);
  }

  fn write_cargo_config_at(dir: &Path, target: &str) {
    std::fs::create_dir_all(dir).expect("create Cargo config dir");
    std::fs::write(
      dir.join("config"),
      format!(
        r#"[build]
target = "{target}"
"#
      ),
    )
    .expect("write Cargo config");
  }
}
