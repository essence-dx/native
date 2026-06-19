// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  fs,
  path::{Path, PathBuf},
};

pub(super) const PACKAGE_VERSION: &str = "7.8.9";
pub(super) type ReceiptResult<T> = Result<T, String>;

#[derive(Debug)]
pub(super) struct PackageVersionFixture {
  pub(super) project_root: PathBuf,
  pub(super) config_dir: PathBuf,
  pub(super) config_path: PathBuf,
  pub(super) package_json_path: PathBuf,
  pub(super) project_config_machine_path: PathBuf,
}

impl PackageVersionFixture {
  pub(super) fn create(output_dir: &Path) -> ReceiptResult<Self> {
    let project_root = output_dir.join(format!(
      "tauri-dx-package-version-benchmark-{}",
      current_unix_ms()
    ));
    let config_dir = project_root.join("src-tauri");
    fs::create_dir_all(&config_dir).map_err(|error| {
      format!(
        "failed to create package-version benchmark fixture {}: {error}",
        config_dir.display()
      )
    })?;
    let config_path = config_dir.join("tauri.conf.json");
    let package_json_path = project_root.join("package.json");
    write_tauri_config_with_package_version(&config_path)?;
    write_representative_package_json(&package_json_path)?;
    let project_config_machine_path = package_version_project_machine_path(&config_dir);
    Ok(Self {
      project_root,
      config_dir,
      config_path,
      package_json_path,
      project_config_machine_path,
    })
  }

  pub(super) fn remove_project_config_machine_cache(&self) -> ReceiptResult<()> {
    if self.project_config_machine_path.exists() {
      fs::remove_file(&self.project_config_machine_path).map_err(|error| {
        format!(
          "failed to remove project-config machine cache {}: {error}",
          self.project_config_machine_path.display()
        )
      })?;
    }
    Ok(())
  }
}

fn package_version_project_machine_path(config_dir: &Path) -> PathBuf {
  config_dir
    .join(".dx")
    .join("tauri")
    .join("project-config.machine")
}

fn write_tauri_config_with_package_version(config_path: &Path) -> ReceiptResult<()> {
  fs::write(
    config_path,
    r#"{
  "identifier": "com.dx.packageversion.benchmark",
  "productName": "DX Package Version Benchmark",
  "version": "../package.json",
  "build": {
    "devUrl": "http://localhost:3000"
  },
  "app": {
    "windows": [
      {
        "title": "DX Package Version Benchmark",
        "width": 800,
        "height": 600
      }
    ]
  }
}
"#,
  )
  .map_err(|error| format!("failed to write {}: {error}", config_path.display()))
}

fn write_representative_package_json(package_json_path: &Path) -> ReceiptResult<()> {
  let mut package = serde_json::Map::new();
  package.insert(
    "name".into(),
    serde_json::json!("dx-package-version-benchmark"),
  );
  package.insert("version".into(), serde_json::json!(PACKAGE_VERSION));
  package.insert(
    "productName".into(),
    serde_json::json!("DX Package Version Benchmark"),
  );
  package.insert("private".into(), serde_json::json!(true));
  package.insert(
    "scripts".into(),
    generated_string_map("script", 240, "tauri"),
  );
  package.insert(
    "dependencies".into(),
    generated_string_map("dependency", 1_400, "^1.0.0"),
  );
  package.insert(
    "devDependencies".into(),
    generated_string_map("dev-dependency", 1_400, "^2.0.0"),
  );
  package.insert(
    "optionalDependencies".into(),
    generated_string_map("optional-dependency", 400, "^3.0.0"),
  );
  fs::write(
    package_json_path,
    serde_json::to_vec_pretty(&serde_json::Value::Object(package))
      .expect("serialize representative package.json"),
  )
  .map_err(|error| format!("failed to write {}: {error}", package_json_path.display()))
}

fn generated_string_map(prefix: &str, count: usize, value: &str) -> serde_json::Value {
  let mut map = serde_json::Map::new();
  for index in 0..count {
    map.insert(format!("@dx/{prefix}-{index:04}"), serde_json::json!(value));
  }
  serde_json::Value::Object(map)
}

pub(super) fn current_unix_ms() -> u128 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .expect("system clock before unix epoch")
    .as_millis()
}
