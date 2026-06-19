// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::config::Config;
use crate::platform::Target;
use json_patch::merge;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// All extensions that are possibly supported, but perhaps not enabled.
pub const EXTENSIONS_SUPPORTED: &[&str] = &["json", "json5", "toml"];

/// All configuration formats that are possibly supported, but perhaps not enabled.
pub const SUPPORTED_FORMATS: &[ConfigFormat] =
  &[ConfigFormat::Json, ConfigFormat::Json5, ConfigFormat::Toml];

/// All configuration formats that are currently enabled.
pub const ENABLED_FORMATS: &[ConfigFormat] = &[
  ConfigFormat::Json,
  #[cfg(feature = "config-json5")]
  ConfigFormat::Json5,
  #[cfg(feature = "config-toml")]
  ConfigFormat::Toml,
];

/// The available configuration formats.
#[derive(Debug, Copy, Clone)]
pub enum ConfigFormat {
  /// The default JSON (tauri.conf.json) format.
  Json,
  /// The JSON5 (tauri.conf.json5) format.
  Json5,
  /// The TOML (Tauri.toml file) format.
  Toml,
}

impl ConfigFormat {
  /// Maps the config format to its file name.
  pub fn into_file_name(self) -> &'static str {
    match self {
      Self::Json => "tauri.conf.json",
      Self::Json5 => "tauri.conf.json5",
      Self::Toml => "Tauri.toml",
    }
  }

  pub(crate) fn into_platform_file_name(self, target: Target) -> &'static str {
    match self {
      Self::Json => match target {
        Target::MacOS => "tauri.macos.conf.json",
        Target::Windows => "tauri.windows.conf.json",
        Target::Linux => "tauri.linux.conf.json",
        Target::Android => "tauri.android.conf.json",
        Target::Ios => "tauri.ios.conf.json",
      },
      Self::Json5 => match target {
        Target::MacOS => "tauri.macos.conf.json5",
        Target::Windows => "tauri.windows.conf.json5",
        Target::Linux => "tauri.linux.conf.json5",
        Target::Android => "tauri.android.conf.json5",
        Target::Ios => "tauri.ios.conf.json5",
      },
      Self::Toml => match target {
        Target::MacOS => "Tauri.macos.toml",
        Target::Windows => "Tauri.windows.toml",
        Target::Linux => "Tauri.linux.toml",
        Target::Android => "Tauri.android.toml",
        Target::Ios => "Tauri.ios.toml",
      },
    }
  }
}

/// Represents all the errors that can happen while reading the config.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigError {
  /// Failed to parse from JSON.
  #[error("unable to parse JSON Tauri config file at {path} because {error}")]
  FormatJson {
    /// The path that failed to parse into JSON.
    path: PathBuf,

    /// The parsing [`serde_json::Error`].
    error: serde_json::Error,
  },

  /// Failed to parse from JSON5.
  #[cfg(feature = "config-json5")]
  #[error("unable to parse JSON5 Tauri config file at {path} because {error}")]
  FormatJson5 {
    /// The path that failed to parse into JSON5.
    path: PathBuf,

    /// The parsing [`json5::Error`].
    error: ::json5::Error,
  },

  /// Failed to parse from TOML.
  #[cfg(feature = "config-toml")]
  #[error("unable to parse toml Tauri config file at {path} because {error}")]
  FormatToml {
    /// The path that failed to parse into TOML.
    path: PathBuf,

    /// The parsing [`toml::Error`].
    error: Box<::toml::de::Error>,
  },

  /// Unknown config file name encountered.
  #[error("unsupported format encountered {0}")]
  UnsupportedFormat(String),

  /// Known file extension encountered, but corresponding parser is not enabled (cargo features).
  #[error("supported (but disabled) format encountered {extension} - try enabling `{feature}` ")]
  DisabledFormat {
    /// The extension encountered.
    extension: String,

    /// The cargo feature to enable it.
    feature: String,
  },

  /// A generic IO error with context of what caused it.
  #[error("unable to read Tauri config file at {path} because {error}")]
  Io {
    /// The path the IO error occurred on.
    path: PathBuf,

    /// The [`std::io::Error`].
    error: std::io::Error,
  },
}

/// Determines if the given folder has a configuration file.
pub fn folder_has_configuration_file(target: Target, folder: &Path) -> bool {
  folder.join(ConfigFormat::Json.into_file_name()).exists()
      || folder.join(ConfigFormat::Json5.into_file_name()).exists()
      || folder.join(ConfigFormat::Toml.into_file_name()).exists()
       // platform file names
       || folder.join(ConfigFormat::Json.into_platform_file_name(target)).exists()
      || folder.join(ConfigFormat::Json5.into_platform_file_name(target)).exists()
      || folder.join(ConfigFormat::Toml.into_platform_file_name(target)).exists()
}

/// Determines if the given file path represents a Tauri configuration file.
pub fn is_configuration_file(target: Target, path: &Path) -> bool {
  path
    .file_name()
    .map(|file_name| {
      file_name == OsStr::new(ConfigFormat::Json.into_file_name())
        || file_name == OsStr::new(ConfigFormat::Json5.into_file_name())
        || file_name == OsStr::new(ConfigFormat::Toml.into_file_name())
      // platform file names
      || file_name == OsStr::new(ConfigFormat::Json.into_platform_file_name(target))
        || file_name == OsStr::new(ConfigFormat::Json5.into_platform_file_name(target))
        || file_name == OsStr::new(ConfigFormat::Toml.into_platform_file_name(target))
    })
    .unwrap_or_default()
}

/// Reads the configuration from the given root directory.
///
/// It first looks for a `tauri.conf.json[5]` or `Tauri.toml` file on the given directory. The file must exist.
/// Then it looks for a platform-specific configuration file:
/// - `tauri.macos.conf.json[5]` or `Tauri.macos.toml` on macOS
/// - `tauri.linux.conf.json[5]` or `Tauri.linux.toml` on Linux
/// - `tauri.windows.conf.json[5]` or `Tauri.windows.toml` on Windows
/// - `tauri.android.conf.json[5]` or `Tauri.android.toml` on Android
/// - `tauri.ios.conf.json[5]` or `Tauri.ios.toml` on iOS
///   Merging the configurations using [JSON Merge Patch (RFC 7396)].
///
/// Returns the raw configuration and used config paths.
///
/// [JSON Merge Patch (RFC 7396)]: https://datatracker.ietf.org/doc/html/rfc7396.
pub fn read_from(target: Target, root_dir: &Path) -> Result<(Value, Vec<PathBuf>), ConfigError> {
  let (mut config, config_file_path) = parse_value(target, base_config_file_path(root_dir))?;
  let mut config_paths = vec![config_file_path];
  if let Some((platform_config, path)) = read_platform(target, root_dir)? {
    config_paths.push(path);
    merge(&mut config, &platform_config);
  }
  Ok((config, config_paths))
}

/// Reads the platform-specific configuration file from the given root directory if it exists.
///
/// Check [`read_from`] for more information.
pub fn read_platform(
  target: Target,
  root_dir: &Path,
) -> Result<Option<(Value, PathBuf)>, ConfigError> {
  let platform_config_seed_path = platform_config_file_path(target, root_dir);
  if does_supported_file_name_exist(target, &platform_config_seed_path) {
    let (platform_config, path): (Value, PathBuf) = parse_value(target, platform_config_seed_path)?;
    Ok(Some((platform_config, path)))
  } else {
    Ok(None)
  }
}

/// Returns the canonical JSON seed path for base Tauri configuration.
pub(crate) fn base_config_file_path(root_dir: &Path) -> PathBuf {
  root_dir.join(ConfigFormat::Json.into_file_name())
}

/// Returns the canonical JSON seed path for target-specific Tauri configuration.
pub(crate) fn platform_config_file_path(target: Target, root_dir: &Path) -> PathBuf {
  root_dir.join(ConfigFormat::Json.into_platform_file_name(target))
}

/// Returns the first supported base configuration source that exists for a root directory.
#[cfg(any(feature = "dx-machine-cache", test))]
pub(crate) fn existing_base_config_path(root_dir: &Path) -> Option<PathBuf> {
  SUPPORTED_FORMATS
    .iter()
    .map(|format| root_dir.join(format.into_file_name()))
    .find(|path| path.exists())
}

/// Returns the first supported target-specific configuration source that exists for a root directory.
#[cfg(any(feature = "dx-machine-cache", test))]
pub(crate) fn existing_platform_config_path(target: Target, root_dir: &Path) -> Option<PathBuf> {
  existing_config_source_path(target, platform_config_file_path(target, root_dir))
}

/// Returns the first supported configuration source path represented by a base or platform seed path.
#[cfg(any(feature = "dx-machine-cache", test))]
pub(crate) fn existing_config_source_path(
  target: Target,
  path: impl Into<PathBuf>,
) -> Option<PathBuf> {
  existing_config_source_path_for_formats(target, path.into(), SUPPORTED_FORMATS)
}

fn existing_enabled_config_source_path(
  target: Target,
  path: impl Into<PathBuf>,
) -> Option<PathBuf> {
  existing_config_source_path_for_formats(target, path.into(), ENABLED_FORMATS)
}

fn existing_config_source_path_for_formats(
  target: Target,
  path: PathBuf,
  formats: &[ConfigFormat],
) -> Option<PathBuf> {
  let source_file_name = path.file_name()?;
  let lookup_platform_config = formats
    .iter()
    .any(|format| source_file_name == format.into_platform_file_name(target));

  formats
    .iter()
    .map(|format| {
      path.with_file_name(if lookup_platform_config {
        format.into_platform_file_name(target)
      } else {
        format.into_file_name()
      })
    })
    .find(|path| path.exists())
}

/// Check if an enabled config file exists at path.
///
/// The passed path is expected to be the path to the "default" configuration format, in this case
/// JSON with `.json`.
pub fn does_supported_file_name_exist(target: Target, path: impl Into<PathBuf>) -> bool {
  existing_enabled_config_source_path(target, path).is_some()
}

/// Parse the config from path, including alternative formats.
///
/// Hierarchy:
/// 1. Check if `tauri.conf.json` exists
///    a. Parse it with `serde_json`
///    b. Parse it with `json5` if `serde_json` fails
///    c. Return original `serde_json` error if all above steps failed
/// 2. Check if `tauri.conf.json5` exists
///    a. Parse it with `json5`
///    b. Return error if all above steps failed
/// 3. Check if `Tauri.json` exists
///    a. Parse it with `toml`
///    b. Return error if all above steps failed
/// 4. Return error if all above steps failed
pub fn parse(target: Target, path: impl Into<PathBuf>) -> Result<(Config, PathBuf), ConfigError> {
  do_parse(target, path.into())
}

/// See [`parse`] for specifics, returns a JSON [`Value`] instead of [`Config`].
pub fn parse_value(
  target: Target,
  path: impl Into<PathBuf>,
) -> Result<(Value, PathBuf), ConfigError> {
  do_parse(target, path.into())
}

/// Parse an already-resolved configuration source path without probing sibling formats.
#[cfg(any(feature = "dx-machine-cache", test))]
pub(crate) fn parse_existing_value(
  _target: Target,
  path: impl Into<PathBuf>,
) -> Result<(Value, PathBuf), ConfigError> {
  do_parse_existing(path.into())
}

fn do_parse<D: DeserializeOwned>(
  target: Target,
  path: PathBuf,
) -> Result<(D, PathBuf), ConfigError> {
  let file_name = path
    .file_name()
    .map(OsStr::to_string_lossy)
    .unwrap_or_default();
  let lookup_platform_config = ENABLED_FORMATS
    .iter()
    .any(|format| file_name == format.into_platform_file_name(target));

  let json5 = path.with_file_name(if lookup_platform_config {
    ConfigFormat::Json5.into_platform_file_name(target)
  } else {
    ConfigFormat::Json5.into_file_name()
  });
  let toml = path.with_file_name(if lookup_platform_config {
    ConfigFormat::Toml.into_platform_file_name(target)
  } else {
    ConfigFormat::Toml.into_file_name()
  });

  let path_ext = path
    .extension()
    .map(OsStr::to_string_lossy)
    .unwrap_or_default();

  if path.exists() {
    let raw = read_to_string(&path)?;

    // to allow us to easily use the compile-time #[cfg], we always bind
    #[allow(clippy::let_and_return)]
    let json = do_parse_json(&raw, &path);

    // we also want to support **valid** json5 in the .json extension if the feature is enabled.
    // if the json5 is not valid the serde_json error for regular json will be returned.
    // this could be a bit confusing, so we may want to encourage users using json5 to use the
    // .json5 extension instead of .json
    #[cfg(feature = "config-json5")]
    let json = json.or_else(|json_error| {
      do_parse_json5(&raw, &path).map_err(|_| {
        // assume any errors from json5 in a .json file is because it's not json5
        json_error
      })
    });

    json.map(|j| (j, path))
  } else if json5.exists() {
    #[cfg(feature = "config-json5")]
    {
      let raw = read_to_string(&json5)?;
      do_parse_json5(&raw, &json5).map(|config| (config, json5))
    }

    #[cfg(not(feature = "config-json5"))]
    Err(ConfigError::DisabledFormat {
      extension: ".json5".into(),
      feature: "config-json5".into(),
    })
  } else if toml.exists() {
    #[cfg(feature = "config-toml")]
    {
      let raw = read_to_string(&toml)?;
      do_parse_toml(&raw, &toml).map(|config| (config, toml))
    }

    #[cfg(not(feature = "config-toml"))]
    Err(ConfigError::DisabledFormat {
      extension: ".toml".into(),
      feature: "config-toml".into(),
    })
  } else if !EXTENSIONS_SUPPORTED.contains(&path_ext.as_ref()) {
    Err(ConfigError::UnsupportedFormat(path_ext.to_string()))
  } else {
    Err(ConfigError::Io {
      path,
      error: std::io::ErrorKind::NotFound.into(),
    })
  }
}

#[cfg(any(feature = "dx-machine-cache", test))]
fn do_parse_existing<D: DeserializeOwned>(path: PathBuf) -> Result<(D, PathBuf), ConfigError> {
  let path_ext = path
    .extension()
    .map(OsStr::to_string_lossy)
    .unwrap_or_default();

  if !EXTENSIONS_SUPPORTED.contains(&path_ext.as_ref()) {
    return Err(ConfigError::UnsupportedFormat(path_ext.to_string()));
  }
  if !path.exists() {
    return Err(ConfigError::Io {
      path,
      error: std::io::ErrorKind::NotFound.into(),
    });
  }

  let raw = read_to_string(&path)?;
  match path_ext.as_ref() {
    "json" => {
      #[allow(clippy::let_and_return)]
      let json = do_parse_json(&raw, &path);

      #[cfg(feature = "config-json5")]
      let json = json.or_else(|json_error| do_parse_json5(&raw, &path).map_err(|_| json_error));

      json.map(|config| (config, path))
    }
    "json5" => {
      #[cfg(feature = "config-json5")]
      {
        do_parse_json5(&raw, &path).map(|config| (config, path))
      }

      #[cfg(not(feature = "config-json5"))]
      {
        Err(ConfigError::DisabledFormat {
          extension: ".json5".into(),
          feature: "config-json5".into(),
        })
      }
    }
    "toml" => {
      #[cfg(feature = "config-toml")]
      {
        do_parse_toml(&raw, &path).map(|config| (config, path))
      }

      #[cfg(not(feature = "config-toml"))]
      {
        Err(ConfigError::DisabledFormat {
          extension: ".toml".into(),
          feature: "config-toml".into(),
        })
      }
    }
    _ => Err(ConfigError::UnsupportedFormat(path_ext.to_string())),
  }
}

/// "Low-level" helper to parse JSON into a [`Config`].
///
/// `raw` should be the contents of the file that is represented by `path`.
pub fn parse_json(raw: &str, path: &Path) -> Result<Config, ConfigError> {
  do_parse_json(raw, path)
}

/// "Low-level" helper to parse JSON into a JSON [`Value`].
///
/// `raw` should be the contents of the file that is represented by `path`.
pub fn parse_json_value(raw: &str, path: &Path) -> Result<Value, ConfigError> {
  do_parse_json(raw, path)
}

fn do_parse_json<D: DeserializeOwned>(raw: &str, path: &Path) -> Result<D, ConfigError> {
  serde_json::from_str(raw).map_err(|error| ConfigError::FormatJson {
    path: path.into(),
    error,
  })
}

/// "Low-level" helper to parse JSON5 into a [`Config`].
///
/// `raw` should be the contents of the file that is represented by `path`. This function requires
/// the `config-json5` feature to be enabled.
#[cfg(feature = "config-json5")]
pub fn parse_json5(raw: &str, path: &Path) -> Result<Config, ConfigError> {
  do_parse_json5(raw, path)
}

/// "Low-level" helper to parse JSON5 into a JSON [`Value`].
///
/// `raw` should be the contents of the file that is represented by `path`. This function requires
/// the `config-json5` feature to be enabled.
#[cfg(feature = "config-json5")]
pub fn parse_json5_value(raw: &str, path: &Path) -> Result<Value, ConfigError> {
  do_parse_json5(raw, path)
}

#[cfg(feature = "config-json5")]
fn do_parse_json5<D: DeserializeOwned>(raw: &str, path: &Path) -> Result<D, ConfigError> {
  ::json5::from_str(raw).map_err(|error| ConfigError::FormatJson5 {
    path: path.into(),
    error,
  })
}

#[cfg(feature = "config-toml")]
fn do_parse_toml<D: DeserializeOwned>(raw: &str, path: &Path) -> Result<D, ConfigError> {
  ::toml::from_str(raw).map_err(|error| ConfigError::FormatToml {
    path: path.into(),
    error: Box::new(error),
  })
}

/// Helper function to wrap IO errors from [`std::fs::read_to_string`] into a [`ConfigError`].
fn read_to_string(path: &Path) -> Result<String, ConfigError> {
  std::fs::read_to_string(path).map_err(|error| ConfigError::Io {
    path: path.into(),
    error,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[cfg(feature = "config-json5")]
  #[test]
  fn parse_existing_value_requires_the_exact_resolved_source_path() {
    let project = tempfile::tempdir().expect("create temp project");
    let json_seed = project.path().join("tauri.conf.json");
    let json5_base = project.path().join("tauri.conf.json5");
    std::fs::write(
      &json5_base,
      r#"{
        productName: "JSON5 Product",
        identifier: "com.dx.json5"
      }"#,
    )
    .expect("write JSON5 base");

    assert!(parse_value(Target::Windows, &json_seed).is_ok());
    assert!(parse_existing_value(Target::Windows, &json_seed).is_err());
    let (value, parsed_path) =
      parse_existing_value(Target::Windows, &json5_base).expect("parse exact JSON5 source");

    assert_eq!(parsed_path, json5_base);
    assert_eq!(value["productName"], "JSON5 Product");
  }

  #[cfg(all(feature = "config-json5", feature = "config-toml"))]
  #[test]
  fn parser_existing_config_source_path_uses_supported_format_precedence() {
    let project = tempfile::tempdir().expect("create temp project");
    let base_seed = project.path().join("tauri.conf.json");
    let json5_base = project.path().join("tauri.conf.json5");
    let toml_base = project.path().join("Tauri.toml");

    std::fs::write(&toml_base, "").expect("write TOML base");
    assert_eq!(
      existing_config_source_path(Target::Windows, &base_seed),
      Some(toml_base.clone())
    );

    std::fs::write(&json5_base, "").expect("write JSON5 base");
    assert_eq!(
      existing_config_source_path(Target::Windows, &base_seed),
      Some(json5_base.clone())
    );

    std::fs::write(&base_seed, "{}").expect("write JSON base");
    assert_eq!(
      existing_config_source_path(Target::Windows, &base_seed),
      Some(base_seed)
    );

    let platform_seed = project.path().join("tauri.windows.conf.json");
    let json5_platform = project.path().join("tauri.windows.conf.json5");
    let toml_platform = project.path().join("Tauri.windows.toml");

    std::fs::write(&toml_platform, "").expect("write TOML platform");
    assert_eq!(
      existing_config_source_path(Target::Windows, &platform_seed),
      Some(toml_platform.clone())
    );

    std::fs::write(&json5_platform, "").expect("write JSON5 platform");
    assert_eq!(
      existing_config_source_path(Target::Windows, &platform_seed),
      Some(json5_platform.clone())
    );

    std::fs::write(&platform_seed, "{}").expect("write JSON platform");
    assert_eq!(
      existing_config_source_path(Target::Windows, &platform_seed),
      Some(platform_seed)
    );
  }
}
