// Copyright 2019-2025 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use itertools::Itertools;
use json_patch::merge;
use serde_json::Value as JsonValue;

use tauri_utils::acl::REMOVE_UNUSED_COMMANDS_ENV_VAR;
pub use tauri_utils::{config::*, platform::Target};

use std::{
  collections::HashMap,
  env::{current_dir, set_current_dir, set_var},
  ffi::{OsStr, OsString},
  path::Path,
  process::exit,
  sync::OnceLock,
};

use crate::error::Context;

#[cfg(feature = "dx-machine-cache")]
mod package_version_machine_cache;

#[cfg(all(test, feature = "dx-machine-cache"))]
#[path = "config_package_version_machine_cache_benchmark_tests.rs"]
mod config_package_version_machine_cache_benchmark_tests;

pub const MERGE_CONFIG_EXTENSION_NAME: &str = "--config";

pub struct ConfigMetadata {
  /// The current target.
  target: Target,

  original_identifier: Option<String>,
  /// The actual configuration, merged with any extension.
  inner: Config,
  /// The config extensions (platform-specific config files or the config CLI argument).
  /// Maps the extension name to its value.
  extensions: HashMap<OsString, JsonValue>,
}

impl std::ops::Deref for ConfigMetadata {
  type Target = Config;

  #[inline(always)]
  fn deref(&self) -> &Config {
    &self.inner
  }
}

impl ConfigMetadata {
  /// The original bundle identifier from the config file.
  /// This does not take any extensions into account.
  pub fn original_identifier(&self) -> Option<&str> {
    self.original_identifier.as_deref()
  }

  /// Checks which config is overwriting the bundle identifier.
  pub fn find_bundle_identifier_overwriter(&self) -> Option<OsString> {
    for (ext, config) in &self.extensions {
      if let Some(identifier) = config
        .as_object()
        .and_then(|bundle_config| bundle_config.get("identifier")?.as_str())
      {
        if identifier == self.inner.identifier {
          return Some(ext.clone());
        }
      }
    }
    None
  }
}

pub fn wix_settings(config: WixConfig) -> tauri_bundler::WixSettings {
  tauri_bundler::WixSettings {
    version: config.version,
    upgrade_code: config.upgrade_code,
    fips_compliant: std::env::var_os("TAURI_BUNDLER_WIX_FIPS_COMPLIANT")
      .map(|v| v == "true")
      .unwrap_or(config.fips_compliant),
    language: tauri_bundler::WixLanguage(match config.language {
      WixLanguage::One(lang) => vec![(lang, Default::default())],
      WixLanguage::List(languages) => languages
        .into_iter()
        .map(|lang| (lang, Default::default()))
        .collect(),
      WixLanguage::Localized(languages) => languages
        .into_iter()
        .map(|(lang, config)| {
          (
            lang,
            tauri_bundler::WixLanguageConfig {
              locale_path: config.locale_path.map(Into::into),
            },
          )
        })
        .collect(),
    }),
    template: config.template,
    fragment_paths: config.fragment_paths,
    component_group_refs: config.component_group_refs,
    component_refs: config.component_refs,
    feature_group_refs: config.feature_group_refs,
    feature_refs: config.feature_refs,
    merge_refs: config.merge_refs,
    enable_elevated_update_task: config.enable_elevated_update_task,
    banner_path: config.banner_path,
    dialog_image_path: config.dialog_image_path,
  }
}

pub fn nsis_settings(config: NsisConfig) -> tauri_bundler::NsisSettings {
  tauri_bundler::NsisSettings {
    template: config.template,
    header_image: config.header_image,
    sidebar_image: config.sidebar_image,
    installer_icon: config.installer_icon,
    uninstaller_icon: config.uninstaller_icon,
    uninstaller_header_image: config.uninstaller_header_image,
    install_mode: config.install_mode,
    languages: config.languages,
    custom_language_files: config.custom_language_files,
    display_language_selector: config.display_language_selector,
    compression: config.compression,
    start_menu_folder: config.start_menu_folder,
    installer_hooks: config.installer_hooks,
    #[allow(deprecated)]
    minimum_webview2_version: config.minimum_webview2_version,
  }
}

pub fn custom_sign_settings(
  config: CustomSignCommandConfig,
) -> tauri_bundler::CustomSignCommandSettings {
  match config {
    CustomSignCommandConfig::Command(command) => {
      let mut tokens = command.split(' ');
      tauri_bundler::CustomSignCommandSettings {
        cmd: tokens.next().unwrap().to_string(), // split always has at least one element
        args: tokens.map(String::from).collect(),
      }
    }
    CustomSignCommandConfig::CommandWithOptions { cmd, args } => {
      tauri_bundler::CustomSignCommandSettings { cmd, args }
    }
  }
}

fn config_schema_validator() -> &'static jsonschema::Validator {
  // TODO: Switch to `LazyLock` when we bump MSRV to above 1.80
  static CONFIG_SCHEMA_VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
  CONFIG_SCHEMA_VALIDATOR.get_or_init(|| {
    let schema: JsonValue = serde_json::from_str(include_str!("../../config.schema.json"))
      .expect("Failed to parse config schema bundled in the tauri-cli");
    jsonschema::validator_for(&schema).expect("Config schema bundled in the tauri-cli is invalid")
  })
}

pub(crate) fn validate_config_schema(config: &JsonValue, config_path: &Path, reload: bool) {
  let Some(config_file_name) = config_path.file_name() else {
    return;
  };

  if config_path.extension() == Some(OsStr::new("json"))
    || config_path.extension() == Some(OsStr::new("json5"))
  {
    let mut errors = config_schema_validator().iter_errors(config).peekable();
    if errors.peek().is_some() {
      for error in errors {
        let path = error.instance_path.into_iter().join(" > ");
        if path.is_empty() {
          log::error!("`{config_file_name:?}` error: {error}");
        } else {
          log::error!("`{config_file_name:?}` error on `{path}`: {error}");
        }
      }
      if !reload {
        exit(1);
      }
    }
  }
}

fn load_config(
  merge_configs: &[&serde_json::Value],
  reload: bool,
  target: Target,
  tauri_dir: &Path,
) -> crate::Result<ConfigMetadata> {
  #[cfg(feature = "dx-machine-cache")]
  let (
    mut config,
    config_path,
    platform_config,
    original_identifier,
    platform_config_already_merged,
  ) = if merge_configs.is_empty() {
    let read =
      tauri_utils::config::machine_cache::read_project_with_machine_cache(target, tauri_dir)
        .context("failed to parse config")?;
    (
      read.config,
      read.config_path,
      read.platform_config,
      read.original_identifier,
      true,
    )
  } else {
    let (config, config_path) = tauri_utils::config::machine_cache::parse_value_with_machine_cache(
      target,
      tauri_dir.join("tauri.conf.json"),
    )
    .context("failed to parse config")?;
    let original_identifier = config
      .as_object()
      .and_then(|config| config.get("identifier")?.as_str())
      .map(ToString::to_string);
    let platform_config =
      tauri_utils::config::machine_cache::read_platform_with_machine_cache(target, tauri_dir)
        .context("failed to parse platform config")?;
    (
      config,
      config_path,
      platform_config,
      original_identifier,
      false,
    )
  };
  #[cfg(not(feature = "dx-machine-cache"))]
  let (mut config, config_path) =
    tauri_utils::config::parse::parse_value(target, tauri_dir.join("tauri.conf.json"))
      .context("failed to parse config")?;
  let mut extensions = HashMap::new();

  #[cfg(not(feature = "dx-machine-cache"))]
  let original_identifier = config
    .as_object()
    .and_then(|config| config.get("identifier")?.as_str())
    .map(ToString::to_string);

  #[cfg(not(feature = "dx-machine-cache"))]
  let platform_config = tauri_utils::config::parse::read_platform(target, tauri_dir)
    .context("failed to parse platform config")?;
  #[cfg(not(feature = "dx-machine-cache"))]
  let platform_config_already_merged = false;

  if let Some((platform_config, config_path)) = platform_config {
    if !platform_config_already_merged {
      merge(&mut config, &platform_config);
    }
    extensions.insert(config_path.file_name().unwrap().into(), platform_config);
  }

  if !merge_configs.is_empty() {
    let mut merge_config = serde_json::Value::Object(Default::default());
    for conf in merge_configs {
      merge_patches(&mut merge_config, conf);
    }

    let merge_config_str = serde_json::to_string(&merge_config).unwrap();
    set_var("TAURI_CONFIG", merge_config_str);
    merge(&mut config, &merge_config);
    extensions.insert(MERGE_CONFIG_EXTENSION_NAME.into(), merge_config);
  }

  validate_config_schema(&config, &config_path, reload);

  // the `Config` deserializer for `package > version` can resolve the version from a path relative to the config path
  // so we actually need to change the current working directory here
  #[cfg(feature = "dx-machine-cache")]
  let package_version_candidate = if merge_configs.is_empty() {
    package_version_machine_cache::package_version_cache_candidate(
      &config,
      config_path.parent().unwrap(),
    )
  } else {
    None
  };
  #[cfg(feature = "dx-machine-cache")]
  let package_version_cache_hit = package_version_candidate.as_ref().is_some_and(|candidate| {
    package_version_machine_cache::apply_cached_package_version(&mut config, candidate)
  });

  let current_dir = current_dir().context("failed to resolve current directory")?;
  set_current_dir(config_path.parent().unwrap()).context("failed to set current directory")?;
  let config: Config = serde_json::from_value(config).context("failed to parse config")?;
  // revert to previous working directory
  set_current_dir(current_dir).context("failed to set current directory")?;

  #[cfg(feature = "dx-machine-cache")]
  if !package_version_cache_hit
    && package_version_machine_cache::machine_cache_enabled()
    && tauri_utils::config::machine_cache::machine_cache_writes_enabled()
  {
    if let (Some(candidate), Some(version)) =
      (&package_version_candidate, config.version.as_deref())
    {
      let _ =
        package_version_machine_cache::write_package_version_machine_cache(candidate, version);
    }
  }

  for (plugin, conf) in &config.plugins.0 {
    set_var(
      format!(
        "TAURI_{}_PLUGIN_CONFIG",
        plugin.to_uppercase().replace('-', "_")
      ),
      serde_json::to_string(&conf).context("failed to serialize config")?,
    );
  }

  if config.build.remove_unused_commands {
    std::env::set_var(REMOVE_UNUSED_COMMANDS_ENV_VAR, tauri_dir);
  }

  Ok(ConfigMetadata {
    target,
    original_identifier,
    inner: config,
    extensions,
  })
}

pub fn get_config(
  target: Target,
  merge_configs: &[&serde_json::Value],
  tauri_dir: &Path,
) -> crate::Result<ConfigMetadata> {
  load_config(merge_configs, false, target, tauri_dir)
}

#[cfg(all(test, feature = "dx-machine-cache"))]
pub(crate) const TAURI_DX_MACHINE_CACHE_ENV_FOR_TESTS: &str =
  package_version_machine_cache::TAURI_DX_MACHINE_CACHE_ENV;

#[cfg(all(test, feature = "dx-machine-cache"))]
pub(crate) fn package_version_machine_path_for_tests(config_dir: &Path) -> std::path::PathBuf {
  package_version_machine_cache::package_version_machine_path(config_dir)
}

#[cfg(all(test, feature = "dx-machine-cache"))]
pub(crate) fn reset_package_version_machine_cache_read_hits_for_tests() {
  package_version_machine_cache::reset_package_version_machine_cache_read_hits();
}

#[cfg(all(test, feature = "dx-machine-cache"))]
pub(crate) fn package_version_machine_cache_read_hits_for_tests() -> usize {
  package_version_machine_cache::package_version_machine_cache_read_hits()
}

pub fn reload_config(
  config: &mut ConfigMetadata,
  merge_configs: &[&serde_json::Value],
  tauri_dir: &Path,
) -> crate::Result<()> {
  let target = config.target;
  *config = load_config(merge_configs, true, target, tauri_dir)?;
  Ok(())
}

/// merges the loaded config with the given value
pub fn merge_config_with(
  config: &mut ConfigMetadata,
  merge_configs: &[&serde_json::Value],
) -> crate::Result<()> {
  if merge_configs.is_empty() {
    return Ok(());
  }

  let mut merge_config = serde_json::Value::Object(Default::default());
  for conf in merge_configs {
    merge_patches(&mut merge_config, conf);
  }

  let merge_config_str = serde_json::to_string(&merge_config).unwrap();
  set_var("TAURI_CONFIG", merge_config_str);

  let mut value =
    serde_json::to_value(config.inner.clone()).context("failed to serialize config")?;
  merge(&mut value, &merge_config);
  config.inner = serde_json::from_value(value).context("failed to parse config")?;
  Ok(())
}

/// Same as [`json_patch::merge`] but doesn't delete the key when the patch's value is `null`
fn merge_patches(doc: &mut serde_json::Value, patch: &serde_json::Value) {
  use serde_json::{Map, Value};

  if !patch.is_object() {
    *doc = patch.clone();
    return;
  }

  if !doc.is_object() {
    *doc = Value::Object(Map::new());
  }
  let map = doc.as_object_mut().unwrap();
  for (key, value) in patch.as_object().unwrap() {
    merge_patches(map.entry(key.as_str()).or_insert(Value::Null), value);
  }
}

#[cfg(test)]
mod tests {
  #[cfg(feature = "dx-machine-cache")]
  use super::package_version_machine_cache::{
    apply_cached_package_version, package_version_cache_candidate,
    package_version_machine_cache_write_attempts, package_version_machine_path,
    reset_package_version_machine_cache_write_attempts, write_package_version_machine_cache,
    TAURI_DX_MACHINE_CACHE_ENV,
  };

  #[cfg(feature = "dx-machine-cache")]
  use std::{
    fs,
    path::Path,
    sync::{Mutex, MutexGuard, OnceLock},
  };

  #[cfg(feature = "dx-machine-cache")]
  static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

  #[cfg(feature = "dx-machine-cache")]
  struct EnvGuard {
    vars: Vec<(&'static str, Option<String>)>,
    _lock: MutexGuard<'static, ()>,
  }

  #[cfg(feature = "dx-machine-cache")]
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
      let mut previous_vars = Vec::with_capacity(vars.len());
      for (key, value) in vars {
        let previous = std::env::var(key).ok();
        previous_vars.push((*key, previous));
        if let Some(value) = value {
          std::env::set_var(key, value);
        } else {
          std::env::remove_var(key);
        }
      }
      Self {
        vars: previous_vars,
        _lock: lock,
      }
    }
  }

  #[cfg(feature = "dx-machine-cache")]
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
  fn merge_patches() {
    let mut json = serde_json::Value::Object(Default::default());

    super::merge_patches(
      &mut json,
      &serde_json::json!({
        "app": {
          "withGlobalTauri": true,
          "windows": []
        },
        "plugins": {
          "test": "tauri"
        },
        "build": {
          "devUrl": "http://localhost:8080"
        }
      }),
    );

    super::merge_patches(
      &mut json,
      &serde_json::json!({
        "app": { "withGlobalTauri": null }
      }),
    );

    super::merge_patches(
      &mut json,
      &serde_json::json!({
        "app": { "windows": null }
      }),
    );

    super::merge_patches(
      &mut json,
      &serde_json::json!({
        "plugins": { "updater": {
          "endpoints": ["https://tauri.app"]
        } }
      }),
    );

    assert_eq!(
      json,
      serde_json::json!({
        "app": {
          "withGlobalTauri": null,
          "windows": null
        },
        "plugins": {
          "test": "tauri",
          "updater": {
            "endpoints": ["https://tauri.app"]
          }
        },
        "build": {
          "devUrl": "http://localhost:8080"
        }
      })
    )
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn dx_package_version_machine_cache_replaces_version_path_on_hit() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_package_version_project("version-cache-hit", "1.2.3");
    let config_dir = project.path().join("src-tauri");
    let mut config = serde_json::json!({
      "version": "../package.json"
    });
    let candidate =
      package_version_cache_candidate(&config, &config_dir).expect("package version candidate");

    write_package_version_machine_cache(&candidate, "1.2.3").expect("write package version cache");

    assert!(package_version_machine_path(&config_dir).exists());
    assert!(apply_cached_package_version(&mut config, &candidate));
    assert_eq!(config["version"], serde_json::json!("1.2.3"));
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn dx_package_version_machine_cache_rejects_stale_package_json() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_package_version_project("version-cache-stale", "1.2.3");
    let config_dir = project.path().join("src-tauri");
    let mut config = serde_json::json!({
      "version": "../package.json"
    });
    let candidate =
      package_version_cache_candidate(&config, &config_dir).expect("package version candidate");

    write_package_version_machine_cache(&candidate, "1.2.3").expect("write package version cache");
    write_package_json(project.path(), "2.0.0");

    assert!(!apply_cached_package_version(&mut config, &candidate));
    assert_eq!(config["version"], serde_json::json!("../package.json"));
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn dx_package_version_machine_cache_rejects_package_json_content_drift() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_package_version_project("version-cache-content-drift", "1.2.3");
    let config_dir = project.path().join("src-tauri");
    let mut config = serde_json::json!({
      "version": "../package.json"
    });
    let candidate =
      package_version_cache_candidate(&config, &config_dir).expect("package version candidate");

    write_package_version_machine_cache(&candidate, "1.2.3").expect("write package version cache");
    fs::write(
      project.path().join("package.json"),
      r#"{
  "name": "dx-package-version-fixture-renamed",
  "version": "1.2.3"
}
"#,
    )
    .expect("rewrite package.json");

    assert!(!apply_cached_package_version(&mut config, &candidate));
    assert_eq!(config["version"], serde_json::json!("../package.json"));
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn dx_package_version_machine_cache_ignores_semver_literals() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_package_version_project("version-cache-literal", "1.2.3");
    let config_dir = project.path().join("src-tauri");
    let config = serde_json::json!({
      "version": "3.0.0"
    });

    assert!(package_version_cache_candidate(&config, &config_dir).is_none());
    assert!(!package_version_machine_path(&config_dir).exists());
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn dx_package_version_machine_cache_ignores_non_package_json_paths() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_package_version_project("version-cache-non-package-json", "1.2.3");
    let config_dir = project.path().join("src-tauri");
    fs::write(
      project.path().join("version.json"),
      r#"{
  "version": "1.2.3"
}
"#,
    )
    .expect("write version.json");
    let config = serde_json::json!({
      "version": "../version.json"
    });

    assert!(package_version_cache_candidate(&config, &config_dir).is_none());
    assert!(!package_version_machine_path(&config_dir).exists());
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn dx_package_version_machine_cache_default_off_ignores_cache() {
    let project = temp_package_version_project("version-cache-default-off", "1.2.3");
    let config_dir = project.path().join("src-tauri");
    let mut config = serde_json::json!({
      "version": "../package.json"
    });
    {
      let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
      let candidate =
        package_version_cache_candidate(&config, &config_dir).expect("package version candidate");
      write_package_version_machine_cache(&candidate, "1.2.3")
        .expect("write package version cache");
      assert!(package_version_machine_path(&config_dir).exists());
    }
    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
    let candidate =
      package_version_cache_candidate(&config, &config_dir).expect("package version candidate");

    assert!(!apply_cached_package_version(&mut config, &candidate));
    assert_eq!(config["version"], serde_json::json!("../package.json"));
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn dx_package_version_machine_cache_load_config_default_off_does_not_write_cache() {
    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
    let project = temp_package_version_project("version-cache-load-config-default-off", "1.2.3");
    let config_dir = project.path().join("src-tauri");
    write_tauri_config_with_package_version(&config_dir);

    let config = super::load_config(&[], false, super::Target::current(), &config_dir)
      .expect("load config with package version path");

    assert_eq!(config.version.as_deref(), Some("1.2.3"));
    assert!(!package_version_machine_path(&config_dir).exists());
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn dx_package_version_machine_cache_skips_dynamic_merge_inputs() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_package_version_project("version-cache-merge-input", "1.2.3");
    let config_dir = project.path().join("src-tauri");
    write_tauri_config_with_literal_version(&config_dir, "0.1.0");
    let merge_config = serde_json::json!({
      "version": "../package.json"
    });

    let config = super::load_config(
      &[&merge_config],
      false,
      super::Target::current(),
      &config_dir,
    )
    .expect("load config with dynamic package version path");

    assert_eq!(config.version.as_deref(), Some("1.2.3"));
    assert!(!package_version_machine_path(&config_dir).exists());
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn dx_package_version_machine_cache_load_config_writes_cache_from_package_json() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_package_version_project("version-cache-load-config", "1.2.3");
    let config_dir = project.path().join("src-tauri");
    write_tauri_config_with_package_version(&config_dir);

    let config = super::load_config(&[], false, super::Target::current(), &config_dir)
      .expect("load config with package version path");

    assert_eq!(config.version.as_deref(), Some("1.2.3"));
    assert!(package_version_machine_path(&config_dir).exists());
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn dx_package_version_machine_cache_load_config_does_not_rewrite_after_valid_hit() {
    let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
    let project = temp_package_version_project("version-cache-load-config-hit", "1.2.3");
    let config_dir = project.path().join("src-tauri");
    write_tauri_config_with_package_version(&config_dir);

    reset_package_version_machine_cache_write_attempts();
    let first = super::load_config(&[], false, super::Target::current(), &config_dir)
      .expect("prime package version cache");
    assert_eq!(first.version.as_deref(), Some("1.2.3"));
    assert_eq!(package_version_machine_cache_write_attempts(), 1);

    let second = super::load_config(&[], false, super::Target::current(), &config_dir)
      .expect("load config from package version cache");
    assert_eq!(second.version.as_deref(), Some("1.2.3"));
    assert_eq!(package_version_machine_cache_write_attempts(), 1);
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn dx_package_version_machine_cache_write_env_zero_still_reads_hit() {
    let project = temp_package_version_project("version-cache-read-only-hit", "1.2.3");
    let config_dir = project.path().join("src-tauri");
    write_tauri_config_with_package_version(&config_dir);
    {
      let _env = EnvGuard::set(TAURI_DX_MACHINE_CACHE_ENV, "1");
      let config = super::load_config(&[], false, super::Target::current(), &config_dir)
        .expect("prime package version cache");
      assert_eq!(config.version.as_deref(), Some("1.2.3"));
    }

    reset_package_version_machine_cache_write_attempts();
    let _env = EnvGuard::set_many(&[
      (TAURI_DX_MACHINE_CACHE_ENV, Some("1")),
      (
        tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
        Some("0"),
      ),
    ]);

    let config = super::load_config(&[], false, super::Target::current(), &config_dir)
      .expect("load package version cache with writes disabled");

    assert_eq!(config.version.as_deref(), Some("1.2.3"));
    assert_eq!(package_version_machine_cache_write_attempts(), 0);
    assert!(package_version_machine_path(&config_dir).exists());
  }

  #[cfg(feature = "dx-machine-cache")]
  #[test]
  fn dx_package_version_machine_cache_write_env_zero_does_not_write_on_miss() {
    let project = temp_package_version_project("version-cache-read-only-miss", "1.2.3");
    let config_dir = project.path().join("src-tauri");
    write_tauri_config_with_package_version(&config_dir);
    let _env = EnvGuard::set_many(&[
      (TAURI_DX_MACHINE_CACHE_ENV, Some("1")),
      (
        tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
        Some("0"),
      ),
    ]);

    let config = super::load_config(&[], false, super::Target::current(), &config_dir)
      .expect("load package version from source with writes disabled");

    assert_eq!(config.version.as_deref(), Some("1.2.3"));
    assert!(!package_version_machine_path(&config_dir).exists());
  }

  #[cfg(feature = "dx-machine-cache")]
  fn temp_package_version_project(name: &str, version: &str) -> tempfile::TempDir {
    let base = std::env::var_os("DX_TEST_OUTPUT_DIR")
      .map(std::path::PathBuf::from)
      .unwrap_or_else(std::env::temp_dir);
    fs::create_dir_all(&base).expect("test output base");
    let project = tempfile::Builder::new()
      .prefix(&format!("tauri-dx-package-version-{name}-"))
      .tempdir_in(base)
      .expect("temp package version project");
    fs::create_dir_all(project.path().join("src-tauri")).expect("src-tauri dir");
    write_package_json(project.path(), version);
    project
  }

  #[cfg(feature = "dx-machine-cache")]
  fn write_tauri_config_with_package_version(config_dir: &Path) {
    fs::write(
      config_dir.join("tauri.conf.json"),
      r#"{
  "identifier": "com.dx.packageversion",
  "productName": "DX Package Version",
  "version": "../package.json",
  "build": {
    "devUrl": "http://localhost:3000"
  },
  "app": {
    "windows": [
      {
        "title": "DX Package Version",
        "width": 800,
        "height": 600
      }
    ]
  }
}
"#,
    )
    .expect("write tauri.conf.json");
  }

  #[cfg(feature = "dx-machine-cache")]
  fn write_tauri_config_with_literal_version(config_dir: &Path, version: &str) {
    fs::write(
      config_dir.join("tauri.conf.json"),
      format!(
        r#"{{
  "identifier": "com.dx.packageversion",
  "productName": "DX Package Version",
  "version": "{version}",
  "build": {{
    "devUrl": "http://localhost:3000"
  }},
  "app": {{
    "windows": [
      {{
        "title": "DX Package Version",
        "width": 800,
        "height": 600
      }}
    ]
  }}
}}
"#
      ),
    )
    .expect("write tauri.conf.json");
  }

  #[cfg(feature = "dx-machine-cache")]
  fn write_package_json(project: &Path, version: &str) {
    fs::write(
      project.join("package.json"),
      format!(
        r#"{{
  "name": "dx-package-version-fixture",
  "version": "{version}"
}}
"#
      ),
    )
    .expect("write package.json");
  }
}
