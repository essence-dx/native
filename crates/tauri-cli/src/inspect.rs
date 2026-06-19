// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
use std::path::Path;

#[cfg(feature = "dx-machine-cache")]
use serde_json::Value as JsonValue;

use crate::Result;
use clap::{Parser, Subcommand};

#[cfg(feature = "dx-machine-cache")]
use crate::error::Context;
use crate::interface::{AppInterface, AppSettings};

#[derive(Debug, Parser)]
#[clap(about = "Inspect values used by Tauri")]
pub struct Cli {
  #[clap(subcommand)]
  command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
  /// Print the default Upgrade Code used by MSI installer derived from productName.
  WixUpgradeCode,
}

pub fn command(cli: Cli) -> Result<()> {
  let dirs = crate::helpers::app_paths::resolve_dirs();
  match cli.command {
    Commands::WixUpgradeCode => wix_upgrade_code(dirs.tauri),
  }
}

// NOTE: if this is ever changed, make sure to also update Wix upgrade code generation in tauri-bundler
fn wix_upgrade_code(tauri_dir: &Path) -> Result<()> {
  #[cfg(feature = "dx-machine-cache")]
  if let Some(projection) = wix_upgrade_code_projection_from_machine_cache(tauri_dir)? {
    log_wix_upgrade_code(&projection.product_name, projection.upgrade_code);
    return Ok(());
  }

  let target = tauri_utils::platform::Target::Windows;
  let config = crate::helpers::config::get_config(target, &[], tauri_dir)?;

  let product_name = product_name_for_wix_upgrade_code(&config, tauri_dir)?;
  let upgrade_code = config
    .bundle
    .windows
    .wix
    .as_ref()
    .and_then(|wix| wix.upgrade_code);

  log_wix_upgrade_code(&product_name, upgrade_code);

  Ok(())
}

fn log_wix_upgrade_code(product_name: &str, configured_upgrade_code: Option<uuid::Uuid>) {
  let upgrade_code = uuid::Uuid::new_v5(
    &uuid::Uuid::NAMESPACE_DNS,
    format!("{product_name}.exe.app.x64").as_bytes(),
  );

  log::info!("Default WiX Upgrade Code, derived from {product_name}: {upgrade_code}");
  if let Some(code) = configured_upgrade_code {
    log::info!("Application Upgrade Code override: {code}");
  }
}

#[cfg(feature = "dx-machine-cache")]
struct WixUpgradeCodeProjection {
  product_name: String,
  upgrade_code: Option<uuid::Uuid>,
}

#[cfg(feature = "dx-machine-cache")]
fn wix_upgrade_code_projection_from_machine_cache(
  tauri_dir: &Path,
) -> Result<Option<WixUpgradeCodeProjection>> {
  let target = tauri_utils::platform::Target::Windows;
  if let Some(projection) =
    wix_upgrade_code_projection_from_project_config_projection(target, tauri_dir)?
  {
    return Ok(Some(projection));
  }

  let Some(read) =
    tauri_utils::config::machine_cache::read_cached_project_with_machine_cache(target, tauri_dir)
  else {
    return Ok(None);
  };

  crate::helpers::config::validate_config_schema(&read.config, &read.config_path, false);

  let Some(product_name) =
    config_string_any(&read.config, &[&["productName"][..], &["product-name"][..]])
  else {
    return Ok(None);
  };
  if !is_valid_product_name(product_name) {
    return Ok(None);
  }

  let upgrade_code = match config_string_any(
    &read.config,
    &[
      &["bundle", "windows", "wix", "upgradeCode"][..],
      &["bundle", "windows", "wix", "upgrade-code"][..],
    ],
  ) {
    Some(value) => Some(uuid::Uuid::parse_str(value).context("failed to parse config")?),
    None
      if config_path_exists_any(
        &read.config,
        &[
          &["bundle", "windows", "wix", "upgradeCode"][..],
          &["bundle", "windows", "wix", "upgrade-code"][..],
        ],
      ) =>
    {
      return Ok(None);
    }
    None => None,
  };

  Ok(Some(WixUpgradeCodeProjection {
    product_name: product_name.to_string(),
    upgrade_code,
  }))
}

#[cfg(feature = "dx-machine-cache")]
fn wix_upgrade_code_projection_from_project_config_projection(
  target: tauri_utils::platform::Target,
  tauri_dir: &Path,
) -> Result<Option<WixUpgradeCodeProjection>> {
  if base_config_requires_schema_validation(tauri_dir) {
    return Ok(None);
  }

  let Some(projection) =
    tauri_utils::config::machine_cache::read_cached_project_config_projection_with_machine_cache(
      target,
      tauri_dir,
      &[
        &["productName"][..],
        &["product-name"][..],
        &["bundle", "windows", "wix", "upgradeCode"][..],
        &["bundle", "windows", "wix", "upgrade-code"][..],
      ],
    )
  else {
    return Ok(None);
  };

  if config_path_requires_schema_validation(&projection.config_path) {
    return Ok(None);
  }

  let values = &projection.values;
  if values.len() != 4 {
    return Ok(None);
  }
  let Some(product_name) = projected_string_any(&values[0..2]) else {
    return Ok(None);
  };
  if !is_valid_product_name(product_name) {
    return Ok(None);
  }

  let upgrade_code = match projected_string_any(&values[2..4]) {
    Some(value) => match uuid::Uuid::parse_str(value) {
      Ok(value) => Some(value),
      Err(_) => return Ok(None),
    },
    None
      if values[2..4].iter().any(|value| {
        matches!(
          value,
          tauri_utils::config::machine_cache::TauriConfigProjectedValue::PresentNonString
        )
      }) =>
    {
      return Ok(None);
    }
    None => None,
  };

  Ok(Some(WixUpgradeCodeProjection {
    product_name: product_name.to_string(),
    upgrade_code,
  }))
}

#[cfg(feature = "dx-machine-cache")]
fn projected_string_any(
  values: &[tauri_utils::config::machine_cache::TauriConfigProjectedValue],
) -> Option<&str> {
  values.iter().find_map(|value| match value {
    tauri_utils::config::machine_cache::TauriConfigProjectedValue::String(value) => {
      Some(value.as_str())
    }
    _ => None,
  })
}

#[cfg(feature = "dx-machine-cache")]
fn config_path_requires_schema_validation(config_path: &Path) -> bool {
  matches!(
    config_path
      .extension()
      .and_then(|extension| extension.to_str()),
    Some("json" | "json5")
  )
}

#[cfg(feature = "dx-machine-cache")]
fn base_config_requires_schema_validation(tauri_dir: &Path) -> bool {
  tauri_utils::config::parse::SUPPORTED_FORMATS
    .iter()
    .map(|format| tauri_dir.join(format.into_file_name()))
    .find(|path| path.exists())
    .is_some_and(|path| config_path_requires_schema_validation(&path))
}

#[cfg(feature = "dx-machine-cache")]
fn config_string_any<'a>(config: &'a JsonValue, paths: &[&[&str]]) -> Option<&'a str> {
  paths.iter().find_map(|path| config_string(config, path))
}

#[cfg(feature = "dx-machine-cache")]
fn config_string<'a>(config: &'a JsonValue, path: &[&str]) -> Option<&'a str> {
  match config_path_state(config, path) {
    ConfigPathState::Present(value) => value.as_str(),
    ConfigPathState::Missing | ConfigPathState::BlockedByNonObject => None,
  }
}

#[cfg(feature = "dx-machine-cache")]
fn config_path_exists_any(config: &JsonValue, paths: &[&[&str]]) -> bool {
  paths
    .iter()
    .any(|path| !matches!(config_path_state(config, path), ConfigPathState::Missing))
}

#[cfg(feature = "dx-machine-cache")]
enum ConfigPathState<'a> {
  Missing,
  Present(&'a JsonValue),
  BlockedByNonObject,
}

#[cfg(feature = "dx-machine-cache")]
fn config_path_state<'a>(config: &'a JsonValue, path: &[&str]) -> ConfigPathState<'a> {
  let mut current = config;
  for segment in path {
    let Some(object) = current.as_object() else {
      return ConfigPathState::BlockedByNonObject;
    };
    let Some(next) = object.get(*segment) else {
      return ConfigPathState::Missing;
    };
    current = next;
  }
  ConfigPathState::Present(current)
}

#[cfg(feature = "dx-machine-cache")]
fn is_valid_product_name(product_name: &str) -> bool {
  !product_name.is_empty()
    && !product_name.chars().any(|character| {
      matches!(
        character,
        '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
      )
    })
}

fn product_name_for_wix_upgrade_code(
  config: &crate::helpers::config::Config,
  tauri_dir: &Path,
) -> Result<String> {
  if let Some(product_name) = &config.product_name {
    return Ok(product_name.clone());
  }

  let interface = AppInterface::new(config, None, tauri_dir)?;
  Ok(interface.app_settings().get_package_settings().product_name)
}

#[cfg(test)]
mod tests {
  use super::*;
  #[cfg(feature = "dx-machine-cache")]
  use std::sync::{Mutex, MutexGuard, OnceLock};

  #[cfg(feature = "dx-machine-cache")]
  static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

  #[cfg(feature = "dx-machine-cache")]
  struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
    _lock: MutexGuard<'static, ()>,
  }

  #[cfg(feature = "dx-machine-cache")]
  impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
      let lock = ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
      let previous = std::env::var(key).ok();
      std::env::set_var(key, value);
      Self {
        key,
        previous,
        _lock: lock,
      }
    }
  }

  #[cfg(feature = "dx-machine-cache")]
  impl Drop for EnvGuard {
    fn drop(&mut self) {
      if let Some(previous) = &self.previous {
        std::env::set_var(self.key, previous);
      } else {
        std::env::remove_var(self.key);
      }
    }
  }

  #[test]
  fn inspect_wix_product_name_uses_config_without_loading_rust_interface() {
    let project = tempfile::tempdir().expect("create temp project");
    let config = crate::helpers::config::Config {
      product_name: Some("Config Product".into()),
      ..Default::default()
    };

    let product_name =
      product_name_for_wix_upgrade_code(&config, project.path()).expect("resolve product name");

    assert_eq!(product_name, "Config Product");
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn inspect_wix_projection_identifies_schema_validated_base_config_formats() {
    let json_project = temp_project("inspect-wix-json-schema-validation-format");
    std::fs::write(json_project.path().join("tauri.conf.json"), "{}").expect("write json config");
    assert!(base_config_requires_schema_validation(json_project.path()));

    let toml_project = temp_project("inspect-wix-toml-schema-validation-format");
    std::fs::write(toml_project.path().join("Tauri.toml"), "").expect("write toml config");
    assert!(!base_config_requires_schema_validation(toml_project.path()));
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn inspect_wix_projection_reads_valid_toml_project_machine_projection() {
    let _env = EnvGuard::set(
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      "1",
    );
    let project = temp_project("inspect-wix-direct-toml-projection-hit");
    let tauri_dir = project.path();
    std::fs::write(
      tauri_dir.join("Tauri.toml"),
      r#"
identifier = "com.dx.inspect"
productName = "DX Inspect Toml"

[bundle.windows.wix]
upgradeCode = "11111111-2222-3333-4444-555555555555"
"#,
    )
    .expect("write Tauri.toml");

    tauri_utils::config::machine_cache::read_project_with_machine_cache(
      tauri_utils::platform::Target::Windows,
      tauri_dir,
    )
    .expect("prime project config machine cache");

    let projection = wix_upgrade_code_projection_from_machine_cache(tauri_dir)
      .expect("read cached projection")
      .expect("projection cache hit");

    assert_eq!(projection.product_name, "DX Inspect Toml");
    assert_eq!(
      projection
        .upgrade_code
        .map(|code| code.to_string())
        .as_deref(),
      Some("11111111-2222-3333-4444-555555555555")
    );
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn inspect_wix_projection_falls_back_when_wix_parent_shape_is_invalid() {
    let _env = EnvGuard::set(
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      "1",
    );
    let project = temp_project("inspect-wix-invalid-parent-shape");
    let tauri_dir = project.path();
    std::fs::write(
      tauri_dir.join("Tauri.toml"),
      r#"
identifier = "com.dx.inspect"
productName = "DX Inspect"

[bundle.windows]
wix = 123
"#,
    )
    .expect("write Tauri.toml");

    tauri_utils::config::machine_cache::read_project_with_machine_cache(
      tauri_utils::platform::Target::Windows,
      tauri_dir,
    )
    .expect("prime project config machine cache");

    assert!(wix_upgrade_code_projection_from_machine_cache(tauri_dir)
      .expect("read cached projection")
      .is_none());
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn inspect_wix_projection_reads_valid_project_machine_cache_hit() {
    let _env = EnvGuard::set(
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      "1",
    );
    let project = temp_project("inspect-wix-projection-hit");
    let tauri_dir = project.path();
    std::fs::write(
      tauri_dir.join("tauri.conf.json"),
      r#"{
  "identifier": "com.dx.inspect",
  "productName": "DX Inspect",
  "bundle": {
    "windows": {
      "wix": {
        "upgradeCode": "11111111-2222-3333-4444-555555555555"
      }
    }
  },
  "build": {
    "devUrl": "http://localhost:3000"
  },
  "app": {
    "windows": [
      {
        "title": "DX Inspect",
        "width": 800,
        "height": 600
      }
    ]
  }
}
"#,
    )
    .expect("write tauri config");

    assert!(wix_upgrade_code_projection_from_machine_cache(tauri_dir)
      .expect("read projection before cache exists")
      .is_none());

    tauri_utils::config::machine_cache::read_project_with_machine_cache(
      tauri_utils::platform::Target::Windows,
      tauri_dir,
    )
    .expect("prime project config machine cache");
    let projection = wix_upgrade_code_projection_from_machine_cache(tauri_dir)
      .expect("read cached projection")
      .expect("projection cache hit");

    assert_eq!(projection.product_name, "DX Inspect");
    assert_eq!(
      projection
        .upgrade_code
        .map(|code| code.to_string())
        .as_deref(),
      Some("11111111-2222-3333-4444-555555555555")
    );
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn inspect_wix_projection_falls_back_when_product_name_is_missing() {
    let _env = EnvGuard::set(
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      "1",
    );
    let project = temp_project("inspect-wix-projection-missing-name");
    let tauri_dir = project.path();
    std::fs::write(
      tauri_dir.join("tauri.conf.json"),
      r#"{
  "identifier": "com.dx.inspect",
  "build": {
    "devUrl": "http://localhost:3000"
  },
  "app": {
    "windows": [
      {
        "title": "DX Inspect",
        "width": 800,
        "height": 600
      }
    ]
  }
}
"#,
    )
    .expect("write tauri config");

    tauri_utils::config::machine_cache::read_project_with_machine_cache(
      tauri_utils::platform::Target::Windows,
      tauri_dir,
    )
    .expect("prime project config machine cache");

    assert!(wix_upgrade_code_projection_from_machine_cache(tauri_dir)
      .expect("read cached projection")
      .is_none());
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn inspect_wix_projection_falls_back_when_upgrade_code_is_not_a_string() {
    let _env = EnvGuard::set(
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      "1",
    );
    let project = temp_project("inspect-wix-non-string-upgrade-code");
    let tauri_dir = project.path();
    std::fs::write(
      tauri_dir.join("Tauri.toml"),
      r#"
identifier = "com.dx.inspect"
productName = "DX Inspect"

[bundle.windows.wix]
upgradeCode = 123
"#,
    )
    .expect("write Tauri.toml");

    tauri_utils::config::machine_cache::read_project_with_machine_cache(
      tauri_utils::platform::Target::Windows,
      tauri_dir,
    )
    .expect("prime project config machine cache");

    assert!(wix_upgrade_code_projection_from_machine_cache(tauri_dir)
      .expect("read cached projection")
      .is_none());
  }

  #[cfg(feature = "dx-machine-cache")]
  fn temp_project(name: &str) -> tempfile::TempDir {
    let base = std::env::var_os("DX_TEST_OUTPUT_DIR")
      .map(std::path::PathBuf::from)
      .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&base).expect("test output base");
    tempfile::Builder::new()
      .prefix(&format!("tauri-dx-{name}-"))
      .tempdir_in(base)
      .expect("temp project")
  }
}
