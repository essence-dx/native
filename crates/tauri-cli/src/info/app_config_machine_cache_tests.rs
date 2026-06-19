// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  fs,
  path::{Path, PathBuf},
  sync::{Mutex, MutexGuard, OnceLock},
};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

const PACKAGE_VERSION: &str = "7.8.9";

struct EnvGuard {
  vars: Vec<(&'static str, Option<String>)>,
  _lock: MutexGuard<'static, ()>,
}

impl EnvGuard {
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
fn dx_info_app_config_section_uses_warm_machine_cache_without_spawning() {
  let fixture = InfoAppConfigFixture::create();
  let cache_env = [
    (
      crate::helpers::config::TAURI_DX_MACHINE_CACHE_ENV_FOR_TESTS,
      Some("1"),
    ),
    (
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
      None,
    ),
  ];
  {
    let _env = EnvGuard::set_many(&cache_env);
    let signature = super::app_config_section_signature_for_tests(
      &fixture.config_dir,
      Some(fixture.project_root.as_path()),
    )
    .expect("prime app config machine caches");
    assert_eq!(signature.package_version.as_deref(), Some(PACKAGE_VERSION));
  }

  assert!(fixture.project_config_machine_path.exists());
  assert!(fixture.package_version_machine_path.exists());

  crate::helpers::config::reset_package_version_machine_cache_read_hits_for_tests();
  let project_before = read_file(&fixture.project_config_machine_path);
  let package_before = read_file(&fixture.package_version_machine_path);
  let read_only_env = [
    (
      crate::helpers::config::TAURI_DX_MACHINE_CACHE_ENV_FOR_TESTS,
      Some("1"),
    ),
    (
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
      Some("0"),
    ),
  ];

  let signature = {
    let _env = EnvGuard::set_many(&read_only_env);
    super::app_config_section_signature_for_tests(
      &fixture.config_dir,
      Some(fixture.project_root.as_path()),
    )
    .expect("read app config section from warm machine caches")
  };

  assert_eq!(signature.package_version.as_deref(), Some(PACKAGE_VERSION));
  assert!(signature.descriptions.contains(&"build-type: build".into()));
  assert!(signature.descriptions.contains(&"CSP: unset".into()));
  assert!(signature
    .descriptions
    .iter()
    .any(|description| description.starts_with("frontendDist:")));
  assert!(signature
    .descriptions
    .iter()
    .any(|description| description.starts_with("devUrl:")));
  assert!(signature.descriptions.contains(&"framework: React".into()));
  assert!(signature.descriptions.contains(&"bundler: Vite".into()));
  assert_eq!(signature.action_count, 0);
  assert_eq!(
    crate::helpers::config::package_version_machine_cache_read_hits_for_tests(),
    1
  );
  assert_eq!(
    project_before,
    read_file(&fixture.project_config_machine_path)
  );
  assert_eq!(
    package_before,
    read_file(&fixture.package_version_machine_path)
  );
}

struct InfoAppConfigFixture {
  project_root: PathBuf,
  config_dir: PathBuf,
  project_config_machine_path: PathBuf,
  package_version_machine_path: PathBuf,
}

impl InfoAppConfigFixture {
  fn create() -> Self {
    let root = test_output_root().join(format!(
      "tauri-stage55-info-app-config-{}",
      current_unix_ms()
    ));
    let config_dir = root.join("src-tauri");
    fs::create_dir_all(&config_dir).expect("create fixture config dir");
    write_tauri_config(&config_dir.join("tauri.conf.json"));
    write_package_json(&root.join("package.json"));
    let project_config_machine_path = config_dir
      .join(".dx")
      .join("tauri")
      .join("project-config.machine");
    let package_version_machine_path =
      crate::helpers::config::package_version_machine_path_for_tests(&config_dir);
    Self {
      project_root: root,
      config_dir,
      project_config_machine_path,
      package_version_machine_path,
    }
  }
}

fn test_output_root() -> PathBuf {
  std::env::var_os("DX_TEST_OUTPUT_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(std::env::temp_dir)
}

fn write_tauri_config(path: &Path) {
  fs::write(
    path,
    r#"{
  "identifier": "com.dx.info.appconfig",
  "productName": "DX Info App Config",
  "version": "../package.json",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:3000"
  },
  "bundle": {
    "active": false
  },
  "app": {
    "windows": [
      {
        "title": "DX Info App Config",
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

fn write_package_json(path: &Path) {
  fs::write(
    path,
    format!(
      r#"{{
  "name": "dx-info-app-config",
  "version": "{PACKAGE_VERSION}",
  "private": true,
  "dependencies": {{
    "@vitejs/plugin-react": "^5.0.0",
    "react": "^19.0.0",
    "vite": "^7.0.0"
  }}
}}
"#
    ),
  )
  .expect("write package.json");
}

fn read_file(path: &Path) -> Vec<u8> {
  fs::read(path).unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn current_unix_ms() -> u128 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .expect("system clock before unix epoch")
    .as_millis()
}
