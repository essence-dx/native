// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use super::*;
use std::sync::{Mutex as StdMutex, MutexGuard, OnceLock};

#[cfg(feature = "dx-machine-cache")]
static ENV_LOCK: OnceLock<StdMutex<()>> = OnceLock::new();

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
      .get_or_init(|| StdMutex::new(()))
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

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_cargo_settings_cache_default_off_writes_no_machine() {
  let _env = EnvGuard::remove(cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV);
  let project = temp_cargo_settings_project("settings-default-off");
  write_cargo_settings_manifest(project.path(), "source-app", "1.2.3");

  let settings = CargoSettings::load(project.path()).expect("load cargo settings");

  assert_eq!(
    settings.package.as_ref().expect("package settings").name,
    "source-app"
  );
  assert!(!cargo_settings_machine_path(project.path()).exists());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_cargo_settings_cache_load_reads_valid_machine_before_source_parse() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let project = temp_cargo_settings_project("settings-cache-hit");
  write_cargo_settings_manifest(project.path(), "source-app", "1.2.3");
  let cached_settings = sample_cargo_settings("cached-app");
  cargo_settings_machine_cache::write(project.path(), &cached_settings)
    .expect("write cargo settings cache");

  let settings = CargoSettings::load(project.path()).expect("load cached cargo settings");

  assert_eq!(
    settings.package.as_ref().expect("package settings").name,
    "cached-app"
  );
  assert_cargo_settings_projection(&settings);
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_cargo_settings_cache_write_env_zero_still_reads_hit() {
  let project = temp_cargo_settings_project("settings-read-only-hit");
  write_cargo_settings_manifest(project.path(), "source-app", "1.2.3");
  {
    let _env = EnvGuard::set(
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      "1",
    );
    let settings = CargoSettings::load(project.path()).expect("prime cargo settings cache");
    assert_eq!(
      settings.package.as_ref().expect("package settings").name,
      "source-app"
    );
    assert!(cargo_settings_machine_path(project.path()).exists());
  }

  let machine_path = cargo_settings_machine_path(project.path());
  let machine_before = std::fs::read(&machine_path).expect("read machine cache before hit");
  let _env = EnvGuard::set_many(&[
    (
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      Some("1"),
    ),
    (
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
      Some("0"),
    ),
  ]);

  let settings = CargoSettings::load(project.path()).expect("load cached cargo settings");

  assert_eq!(
    settings.package.as_ref().expect("package settings").name,
    "source-app"
  );
  assert_eq!(
    machine_before,
    std::fs::read(&machine_path).expect("read machine cache after hit")
  );
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_cargo_settings_cache_write_env_zero_does_not_write_on_miss() {
  let project = temp_cargo_settings_project("settings-read-only-miss");
  write_cargo_settings_manifest(project.path(), "source-app", "1.2.3");
  let _env = EnvGuard::set_many(&[
    (
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      Some("1"),
    ),
    (
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
      Some("0"),
    ),
  ]);

  let settings = CargoSettings::load(project.path()).expect("load cargo settings");

  assert_eq!(
    settings.package.as_ref().expect("package settings").name,
    "source-app"
  );
  assert!(!cargo_settings_machine_path(project.path()).exists());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_cargo_settings_cache_rejects_stale_manifest_source() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let project = temp_cargo_settings_project("settings-stale-source");
  write_cargo_settings_manifest(project.path(), "source-one", "1.2.3");
  let settings = CargoSettings::load(project.path()).expect("load first cargo settings");
  assert_eq!(
    settings.package.as_ref().expect("package settings").name,
    "source-one"
  );
  assert!(cargo_settings_machine_path(project.path()).exists());

  write_cargo_settings_manifest(project.path(), "source-two", "2.0.0");

  assert!(cargo_settings_machine_cache::read(project.path()).is_none());
  let settings = CargoSettings::load(project.path()).expect("load changed cargo settings");
  assert_eq!(
    settings.package.as_ref().expect("package settings").name,
    "source-two"
  );
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_cargo_settings_cache_round_trips_workspace_and_bin_projection() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let project = temp_cargo_settings_project("settings-round-trip");
  write_cargo_settings_manifest(project.path(), "source-app", "1.2.3");
  let settings = sample_cargo_settings("cached-app");

  cargo_settings_machine_cache::write(project.path(), &settings)
    .expect("write cargo settings cache");
  let cached = cargo_settings_machine_cache::read(project.path()).expect("read cargo settings");

  assert_eq!(
    cached.package.as_ref().expect("package settings").name,
    "cached-app"
  );
  assert_cargo_settings_projection(&cached);
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_cargo_settings_cache_rejects_only_changed_workspace_root_manifest() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let workspace = temp_cargo_settings_project("settings-workspace-root");
  let app_dir = workspace.path().join("app");
  std::fs::create_dir_all(&app_dir).expect("create app dir");
  write_workspace_manifest(workspace.path(), "1.2.3");
  write_workspace_member_manifest(&app_dir);

  let member = CargoSettings::load(&app_dir).expect("load member settings");
  let root = CargoSettings::load(workspace.path()).expect("load workspace settings");

  assert!(cargo_settings_machine_path(&app_dir).exists());
  assert!(cargo_settings_machine_path(workspace.path()).exists());
  assert!(matches!(
    member
      .package
      .as_ref()
      .and_then(|package| package.version.as_ref()),
    Some(MaybeWorkspace::Workspace(TomlWorkspaceField {
      workspace: true
    }))
  ));
  assert_eq!(
    root
      .workspace
      .as_ref()
      .and_then(|workspace| workspace.package.as_ref())
      .and_then(|package| package.version.as_deref()),
    Some("1.2.3")
  );

  write_workspace_manifest(workspace.path(), "2.0.0");

  let member_cache =
    cargo_settings_machine_cache::read(&app_dir).expect("member cache still valid");
  assert_eq!(
    member_cache.package.as_ref().expect("member package").name,
    "workspace-app"
  );
  assert!(cargo_settings_machine_cache::read(workspace.path()).is_none());
  let root = CargoSettings::load(workspace.path()).expect("reload changed workspace settings");
  assert_eq!(
    root
      .workspace
      .as_ref()
      .and_then(|workspace| workspace.package.as_ref())
      .and_then(|package| package.version.as_deref()),
    Some("2.0.0")
  );
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_rust_app_settings_cached_workspace_inheritance_resolves_package_settings() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let fixture = rust_app_workspace_fixture("settings-app-workspace-inheritance");
  cargo_settings_machine_cache::write(
    &fixture.app_dir,
    &cached_workspace_member_settings("cached-workspace-app"),
  )
  .expect("write member CargoSettings cache");
  cargo_settings_machine_cache::write(
    fixture.root.path(),
    &cached_workspace_root_settings(
      "9.9.9",
      "Cached workspace description",
      "https://cached.workspace.example",
      vec!["Cached Workspace".into()],
      "BSD-3-Clause",
    ),
  )
  .expect("write workspace CargoSettings cache");

  let settings = rust_app_settings_from_fixture(&fixture);
  let package = settings.get_package_settings();

  assert_eq!(package.product_name, "cached-workspace-app");
  assert_eq!(package.version, "9.9.9");
  assert_eq!(package.description, "Cached workspace description");
  assert_eq!(
    package.homepage.as_deref(),
    Some("https://cached.workspace.example")
  );
  assert_eq!(package.authors, Some(vec!["Cached Workspace".into()]));
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_rust_app_settings_cached_direct_package_values_override_workspace_values() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let fixture = rust_app_workspace_fixture("settings-app-direct-values");
  cargo_settings_machine_cache::write(
    &fixture.app_dir,
    &cached_direct_member_settings("cached-direct-app"),
  )
  .expect("write member CargoSettings cache");
  cargo_settings_machine_cache::write(
    fixture.root.path(),
    &cached_workspace_root_settings(
      "1.0.0",
      "Workspace should not win",
      "https://workspace-should-not-win.example",
      vec!["Workspace Should Not Win".into()],
      "Apache-2.0",
    ),
  )
  .expect("write workspace CargoSettings cache");

  let settings = rust_app_settings_from_fixture(&fixture);
  let package = settings.get_package_settings();

  assert!(matches!(
    settings.cargo_package_settings.version.as_ref(),
    Some(MaybeWorkspace::Defined(version)) if version == "7.7.7"
  ));
  assert!(matches!(
    settings.cargo_package_settings.description.as_ref(),
    Some(MaybeWorkspace::Defined(description)) if description == "Cached direct description"
  ));
  assert_eq!(
    settings
      .cargo_ws_package_settings
      .as_ref()
      .and_then(|package| package.version.as_deref()),
    Some("1.0.0")
  );
  assert_eq!(package.product_name, "cached-direct-app");
  assert_eq!(package.version, "7.7.7");
  assert_eq!(package.description, "Cached direct description");
  assert_eq!(
    package.homepage.as_deref(),
    Some("https://cached-direct.example")
  );
  assert_eq!(package.authors, Some(vec!["Direct Author".into()]));
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_rust_app_settings_cached_workspace_license_reaches_bundle_settings() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let fixture = rust_app_workspace_fixture("settings-app-license-fallback");
  cargo_settings_machine_cache::write(
    &fixture.app_dir,
    &cached_workspace_member_settings("cached-license-app"),
  )
  .expect("write member CargoSettings cache");
  cargo_settings_machine_cache::write(
    fixture.root.path(),
    &cached_workspace_root_settings(
      "9.9.9",
      "Cached workspace description",
      "https://cached.workspace.example",
      vec!["Cached Workspace".into()],
      "BSD-3-Clause",
    ),
  )
  .expect("write workspace CargoSettings cache");

  let config = test_tauri_config();
  let settings = rust_app_settings_from_fixture_with_config(&fixture, &config);
  let bundle = settings
    .get_bundle_settings(&Options::default(), &config, &[], &fixture.app_dir)
    .expect("build bundle settings");

  assert_eq!(bundle.license.as_deref(), Some("BSD-3-Clause"));
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_rust_app_settings_cached_bin_projection_filters_and_selects_default_run() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let fixture = rust_app_workspace_fixture("settings-app-binaries");
  cargo_settings_machine_cache::write(
    &fixture.app_dir,
    &cached_member_settings_with_bins("cached-binary-app"),
  )
  .expect("write member CargoSettings cache");
  cargo_settings_machine_cache::write(
    fixture.root.path(),
    &cached_workspace_root_settings(
      "9.9.9",
      "Cached workspace description",
      "https://cached.workspace.example",
      vec!["Cached Workspace".into()],
      "BSD-3-Clause",
    ),
  )
  .expect("write workspace CargoSettings cache");

  let settings = rust_app_settings_from_fixture(&fixture);
  let binaries = settings
    .get_binaries(
      &Options {
        features: vec!["helper-feature".into()],
        ..Default::default()
      },
      &fixture.app_dir,
    )
    .expect("resolve binaries");

  assert_eq!(
    binaries.iter().filter(|binary| binary.main()).count(),
    1,
    "exactly one binary should be marked as main"
  );
  let desktop = binaries
    .iter()
    .find(|binary| binary.name() == "desktop")
    .expect("desktop binary");
  assert!(desktop.main());
  assert_eq!(desktop.src_path().map(String::as_str), Some("src/main.rs"));
  let helper = binaries
    .iter()
    .find(|binary| binary.name() == "helper-cli")
    .expect("helper binary");
  assert!(!helper.main());
  assert_eq!(
    helper.src_path().map(String::as_str),
    Some("src/bin/helper.rs")
  );
  assert!(binaries.iter().all(|binary| binary.name() != "gated"));
  assert!(binaries.iter().all(|binary| binary.name() != "gated-cli"));
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_rust_app_settings_cached_member_reloads_stale_workspace_root_source() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let fixture = rust_app_workspace_fixture("settings-app-stale-workspace-root");
  cargo_settings_machine_cache::write(
    &fixture.app_dir,
    &cached_workspace_member_settings("cached-stale-root-app"),
  )
  .expect("write member CargoSettings cache");
  let root = CargoSettings::load(fixture.root.path()).expect("warm workspace root cache");
  assert_eq!(
    root
      .workspace
      .as_ref()
      .and_then(|workspace| workspace.package.as_ref())
      .and_then(|package| package.version.as_deref()),
    Some("1.2.3")
  );

  write_workspace_manifest(fixture.root.path(), "2.0.0");
  assert!(cargo_settings_machine_cache::read(fixture.root.path()).is_none());

  let settings = rust_app_settings_from_fixture(&fixture);

  assert_eq!(
    settings.cargo_package_settings.name,
    "cached-stale-root-app"
  );
  assert_eq!(settings.get_package_settings().version, "2.0.0");
  assert_eq!(
    settings
      .cargo_ws_package_settings
      .as_ref()
      .and_then(|package| package.version.as_deref()),
    Some("2.0.0")
  );
  let refreshed_root =
    cargo_settings_machine_cache::read(fixture.root.path()).expect("refreshed workspace cache");
  assert_eq!(
    refreshed_root
      .workspace
      .as_ref()
      .and_then(|workspace| workspace.package.as_ref())
      .and_then(|package| package.version.as_deref()),
    Some("2.0.0")
  );
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_workspace_dir_machine_cache_hit_does_not_require_cargo_metadata() {
  let fixture = rust_app_workspace_fixture("workspace-dir-cache-hit-without-cargo");
  let fake_path_dir = fixture.root.path().join("fake-path");
  std::fs::create_dir_all(&fake_path_dir).expect("create fake PATH directory");
  std::fs::write(
    fake_path_dir.join("cargo.cmd"),
    "@echo off\r\necho cargo metadata should not run on workspace cache hits 1>&2\r\nexit /b 57\r\n",
  )
  .expect("write fake cargo");

  {
    let _env = EnvGuard::set(
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      "1",
    );
    let workspace = get_workspace_dir(&fixture.app_dir).expect("warm workspace dir cache");
    assert_eq!(workspace, fixture.root.path());
  }

  let fake_path = fake_path_dir.to_string_lossy().to_string();
  let _env = EnvGuard::set_many(&[
    (
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      Some("1"),
    ),
    ("PATH", Some(fake_path.as_str())),
    ("Path", Some(fake_path.as_str())),
  ]);

  let workspace =
    get_workspace_dir(&fixture.app_dir).expect("read cached workspace dir without cargo");
  assert_eq!(workspace, fixture.root.path());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_workspace_dir_machine_cache_write_env_zero_still_reads_hit() {
  let fixture = rust_app_workspace_fixture("workspace-dir-read-only-hit");
  {
    let _env = EnvGuard::set(
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      "1",
    );
    let workspace = get_workspace_dir(&fixture.app_dir).expect("warm workspace dir cache");
    assert_eq!(workspace, fixture.root.path());
    assert!(workspace_machine_cache::machine_path(&fixture.app_dir).exists());
  }

  let machine_path = workspace_machine_cache::machine_path(&fixture.app_dir);
  let machine_before = std::fs::read(&machine_path).expect("read machine cache before hit");
  let _env = EnvGuard::set_many(&[
    (
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      Some("1"),
    ),
    (
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
      Some("0"),
    ),
  ]);

  let workspace = get_workspace_dir(&fixture.app_dir).expect("read workspace dir cache");

  assert_eq!(workspace, fixture.root.path());
  assert_eq!(
    machine_before,
    std::fs::read(&machine_path).expect("read machine cache after hit")
  );
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_workspace_dir_machine_cache_write_env_zero_does_not_write_on_miss() {
  let fixture = rust_app_workspace_fixture("workspace-dir-read-only-miss");
  let _env = EnvGuard::set_many(&[
    (
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      Some("1"),
    ),
    (
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
      Some("0"),
    ),
  ]);

  let workspace = get_workspace_dir(&fixture.app_dir).expect("resolve workspace dir");

  assert_eq!(workspace, fixture.root.path());
  assert!(!workspace_machine_cache::machine_path(&fixture.app_dir).exists());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_full_cargo_metadata_machine_cache_hit_does_not_spawn_cargo() {
  let fixture = rust_app_workspace_fixture("full-cargo-metadata-cache-hit");
  let fake_path_dir = fixture.root.path().join("fake-full-metadata-path");
  std::fs::create_dir_all(&fake_path_dir).expect("create fake PATH directory");
  std::fs::write(
    fake_path_dir.join("cargo.cmd"),
    "@echo off\r\necho full cargo metadata should not run on cache hits 1>&2\r\nexit /b 57\r\n",
  )
  .expect("write fake cargo");

  {
    let _env = EnvGuard::set(
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      "1",
    );
    let metadata = get_cargo_metadata(&fixture.app_dir).expect("warm full cargo metadata cache");
    assert_eq!(metadata.workspace_root, fixture.root.path());
    assert_eq!(metadata.workspace_members.len(), 1);
    assert!(
      cargo_metadata_machine_cache::machine_path(&fixture.app_dir).exists(),
      "full cargo metadata cache should be generated during warmup"
    );
  }

  let fake_path = fake_path_dir.to_string_lossy().to_string();
  let _env = EnvGuard::set_many(&[
    (
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      Some("1"),
    ),
    ("PATH", Some(fake_path.as_str())),
    ("Path", Some(fake_path.as_str())),
  ]);

  let metadata = get_cargo_metadata(&fixture.app_dir)
    .expect("read full cargo metadata cache without spawning cargo");
  assert_eq!(metadata.workspace_root, fixture.root.path());
  assert_eq!(metadata.workspace_members.len(), 1);
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_full_cargo_metadata_machine_cache_write_env_zero_still_reads_hit() {
  let fixture = rust_app_workspace_fixture("full-cargo-metadata-read-only-hit");
  {
    let _env = EnvGuard::set(
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      "1",
    );
    let metadata = get_cargo_metadata(&fixture.app_dir).expect("warm full cargo metadata cache");
    assert_eq!(metadata.workspace_root, fixture.root.path());
    assert!(cargo_metadata_machine_cache::machine_path(&fixture.app_dir).exists());
  }

  let _env = EnvGuard::set_many(&[
    (
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      Some("1"),
    ),
    (
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
      Some("0"),
    ),
  ]);

  let metadata = get_cargo_metadata(&fixture.app_dir).expect("read full metadata cache");
  assert_eq!(metadata.workspace_root, fixture.root.path());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_watch_folders_machine_cache_projection_does_not_spawn_cargo() {
  let fixture = rust_app_workspace_fixture("watch-folders-projection-cache-hit");
  {
    let _env = EnvGuard::set(
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      "1",
    );
    let metadata = get_cargo_metadata(&fixture.app_dir).expect("warm full cargo metadata cache");
    assert_eq!(metadata.workspace_root, fixture.root.path());
  }

  let fake_path_dir = fixture.root.path().join("fake-watch-folders-path");
  write_failing_cargo_cmd(
    &fake_path_dir,
    "watch folder projection should not spawn cargo",
  );
  let fake_path = fake_path_dir.to_string_lossy().to_string();
  let _env = EnvGuard::set_many(&[
    (
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      Some("1"),
    ),
    ("PATH", Some(fake_path.as_str())),
    ("Path", Some(fake_path.as_str())),
  ]);

  let watch_folders =
    get_watch_folders(&[], &fixture.app_dir).expect("read watch folders from projection");
  assert_eq!(watch_folders, vec![fixture.app_dir.clone()]);
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_target_dir_machine_cache_projection_does_not_spawn_cargo() {
  let fixture = rust_app_workspace_fixture("target-dir-projection-cache-hit");
  {
    let _env = EnvGuard::set(
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      "1",
    );
    let metadata = get_cargo_metadata(&fixture.app_dir).expect("warm full cargo metadata cache");
    assert_eq!(metadata.workspace_root, fixture.root.path());
  }

  let fake_path_dir = fixture.root.path().join("fake-target-dir-path");
  write_failing_cargo_cmd(
    &fake_path_dir,
    "target dir projection should not spawn cargo",
  );
  let fake_path = fake_path_dir.to_string_lossy().to_string();
  let _env = EnvGuard::set_many(&[
    (
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      Some("1"),
    ),
    ("PATH", Some(fake_path.as_str())),
    ("Path", Some(fake_path.as_str())),
  ]);

  let target_dir = get_cargo_target_dir(&[], &fixture.app_dir)
    .expect("read cargo target directory from projection");
  assert_eq!(target_dir, fixture.root.path().join("target"));
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_full_cargo_metadata_machine_cache_write_env_zero_does_not_write_on_miss() {
  let fixture = rust_app_workspace_fixture("full-cargo-metadata-read-only-miss");
  let _env = EnvGuard::set_many(&[
    (
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      Some("1"),
    ),
    (
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
      Some("0"),
    ),
  ]);

  let metadata = get_cargo_metadata(&fixture.app_dir).expect("load cargo metadata on cache miss");

  assert_eq!(metadata.workspace_root, fixture.root.path());
  assert!(!cargo_metadata_machine_cache::machine_path(&fixture.app_dir).exists());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_full_cargo_metadata_machine_cache_rejects_changed_workspace_manifest() {
  let fixture = rust_app_workspace_fixture("full-cargo-metadata-stale-workspace");
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );

  let metadata = get_cargo_metadata(&fixture.app_dir).expect("warm full cargo metadata cache");
  assert_eq!(metadata.workspace_root, fixture.root.path());
  assert!(cargo_metadata_machine_cache::machine_path(&fixture.app_dir).exists());

  write_workspace_manifest(fixture.root.path(), "9.9.9");

  assert!(cargo_metadata_machine_cache::read(&fixture.app_dir).is_none());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_full_cargo_metadata_machine_cache_rejects_changed_cargo_target_dir_env() {
  let fixture = rust_app_workspace_fixture("full-cargo-metadata-stale-target-dir-env");
  {
    let _env = EnvGuard::set_many(&[
      (
        cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
        Some("1"),
      ),
      ("CARGO_TARGET_DIR", Some("target-one")),
    ]);
    let metadata = get_cargo_metadata(&fixture.app_dir).expect("warm full cargo metadata cache");
    assert!(metadata.target_directory.ends_with("target-one"));
    assert!(cargo_metadata_machine_cache::machine_path(&fixture.app_dir).exists());
  }

  let _env = EnvGuard::set_many(&[
    (
      cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
      Some("1"),
    ),
    ("CARGO_TARGET_DIR", Some("target-two")),
  ]);

  assert!(cargo_metadata_machine_cache::read(&fixture.app_dir).is_none());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_full_cargo_metadata_machine_cache_corrupt_file_falls_back_and_refreshes() {
  let fixture = rust_app_workspace_fixture("full-cargo-metadata-corrupt-cache");
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let machine_path = cargo_metadata_machine_cache::machine_path(&fixture.app_dir);
  std::fs::create_dir_all(machine_path.parent().expect("machine parent"))
    .expect("create machine cache dir");
  std::fs::write(&machine_path, b"not-a-dx-machine-cache").expect("write corrupt machine cache");

  let metadata = get_cargo_metadata(&fixture.app_dir).expect("fallback to cargo metadata");

  assert_eq!(metadata.workspace_root, fixture.root.path());
  assert!(cargo_metadata_machine_cache::read(&fixture.app_dir).is_some());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_full_cargo_metadata_machine_cache_oversized_file_falls_back_without_trusting_cache() {
  let fixture = rust_app_workspace_fixture("full-cargo-metadata-oversized-cache");
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let machine_path = cargo_metadata_machine_cache::machine_path(&fixture.app_dir);
  std::fs::create_dir_all(machine_path.parent().expect("machine parent"))
    .expect("create machine cache dir");
  let oversized = std::fs::File::create(&machine_path).expect("create oversized machine cache");
  oversized
    .set_len(64 * 1024 * 1024 + 1)
    .expect("size oversized machine cache");

  let metadata = get_cargo_metadata(&fixture.app_dir).expect("fallback to cargo metadata");

  assert_eq!(metadata.workspace_root, fixture.root.path());
  assert!(cargo_metadata_machine_cache::read(&fixture.app_dir).is_some());
  assert!(
    std::fs::metadata(&machine_path)
      .expect("machine metadata")
      .len()
      < 64 * 1024 * 1024,
    "fallback refresh should replace the oversized cache with a normal sidecar"
  );
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_full_cargo_metadata_machine_cache_unsupported_schema_falls_back_and_refreshes() {
  let fixture = rust_app_workspace_fixture("full-cargo-metadata-unsupported-schema");
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let settings = sample_cargo_settings("wrong-machine-schema");
  cargo_settings_machine_cache::write(&fixture.app_dir, &settings)
    .expect("write wrong-schema machine cache");
  let settings_machine_path = cargo_settings_machine_path(&fixture.app_dir);
  let machine_path = cargo_metadata_machine_cache::machine_path(&fixture.app_dir);
  std::fs::create_dir_all(machine_path.parent().expect("machine parent"))
    .expect("create machine cache dir");
  std::fs::copy(settings_machine_path, &machine_path)
    .expect("copy wrong-schema machine cache into cargo metadata path");

  let metadata = get_cargo_metadata(&fixture.app_dir).expect("fallback to cargo metadata");

  assert_eq!(metadata.workspace_root, fixture.root.path());
  assert!(cargo_metadata_machine_cache::read(&fixture.app_dir).is_some());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_workspace_dir_machine_cache_default_off_writes_no_machine() {
  let _env = EnvGuard::remove(cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV);
  let fixture = rust_app_workspace_fixture("workspace-dir-cache-default-off");

  let workspace = get_workspace_dir(&fixture.app_dir).expect("resolve workspace dir");

  assert_eq!(workspace, fixture.root.path());
  assert!(!workspace_machine_cache::machine_path(&fixture.app_dir).exists());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_workspace_dir_machine_cache_rejects_changed_workspace_manifest() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let fixture = rust_app_workspace_fixture("workspace-dir-cache-stale-workspace");

  let workspace = get_workspace_dir(&fixture.app_dir).expect("warm workspace dir cache");
  assert_eq!(workspace, fixture.root.path());
  assert!(workspace_machine_cache::machine_path(&fixture.app_dir).exists());

  write_workspace_manifest(fixture.root.path(), "2.0.0");

  assert!(workspace_machine_cache::read(&fixture.app_dir).is_none());
  let workspace = get_workspace_dir(&fixture.app_dir).expect("refresh workspace dir cache");
  assert_eq!(workspace, fixture.root.path());
  assert!(workspace_machine_cache::read(&fixture.app_dir).is_some());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_workspace_dir_machine_cache_rejects_changed_app_manifest() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let fixture = rust_app_workspace_fixture("workspace-dir-cache-stale-app");

  let workspace = get_workspace_dir(&fixture.app_dir).expect("warm workspace dir cache");
  assert_eq!(workspace, fixture.root.path());
  assert!(workspace_machine_cache::machine_path(&fixture.app_dir).exists());

  write_workspace_member_manifest(&fixture.app_dir);
  std::fs::write(
    fixture.app_dir.join("src/main.rs"),
    "fn main() { println!(\"changed\"); }\n",
  )
  .expect("write changed main");
  std::fs::write(
    fixture.app_dir.join("Cargo.toml"),
    r#"[package]
name = "workspace-app"
version.workspace = true
description.workspace = true
homepage.workspace = true
authors.workspace = true
license.workspace = true
default-run = "workspace-app"

[features]
extra = []

[[bin]]
name = "workspace-app"
path = "src/main.rs"
"#,
  )
  .expect("write changed workspace member Cargo.toml");

  assert!(workspace_machine_cache::read(&fixture.app_dir).is_none());
  let workspace = get_workspace_dir(&fixture.app_dir).expect("refresh workspace dir cache");
  assert_eq!(workspace, fixture.root.path());
  assert!(workspace_machine_cache::read(&fixture.app_dir).is_some());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_workspace_dir_machine_cache_keeps_hit_when_lockfile_appears() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let fixture = rust_app_workspace_fixture("workspace-dir-cache-lockfile-appears");

  let workspace = get_workspace_dir(&fixture.app_dir).expect("warm workspace dir cache");
  assert_eq!(workspace, fixture.root.path());
  assert!(workspace_machine_cache::machine_path(&fixture.app_dir).exists());

  std::fs::write(
    fixture.root.path().join("Cargo.lock"),
    r#"# This file is automatically @generated by Cargo.
version = 4

[[package]]
name = "workspace-app"
version = "1.2.3"
"#,
  )
  .expect("write Cargo.lock");

  assert!(workspace_machine_cache::read(&fixture.app_dir).is_some());
  let workspace = get_workspace_dir(&fixture.app_dir).expect("refresh workspace dir cache");
  assert_eq!(workspace, fixture.root.path());
  assert!(workspace_machine_cache::read(&fixture.app_dir).is_some());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_workspace_dir_machine_cache_keeps_hit_when_lockfile_disappears() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let fixture = rust_app_workspace_fixture("workspace-dir-cache-lockfile-disappears");
  std::fs::write(
    fixture.root.path().join("Cargo.lock"),
    r#"# This file is automatically @generated by Cargo.
version = 4

[[package]]
name = "workspace-app"
version = "1.2.3"
"#,
  )
  .expect("write Cargo.lock");

  let workspace = get_workspace_dir(&fixture.app_dir).expect("warm workspace dir cache");
  assert_eq!(workspace, fixture.root.path());
  assert!(workspace_machine_cache::machine_path(&fixture.app_dir).exists());

  std::fs::remove_file(fixture.root.path().join("Cargo.lock")).expect("remove Cargo.lock");

  assert!(workspace_machine_cache::read(&fixture.app_dir).is_some());
  let workspace = get_workspace_dir(&fixture.app_dir).expect("refresh workspace dir cache");
  assert_eq!(workspace, fixture.root.path());
  assert!(workspace_machine_cache::read(&fixture.app_dir).is_some());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_workspace_dir_machine_cache_rejects_new_parent_workspace_manifest() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let outer = temp_cargo_settings_project("workspace-dir-cache-new-parent");
  let inner = outer.path().join("inner");
  let app_dir = inner.join("app");
  std::fs::create_dir_all(app_dir.join("src")).expect("create nested app src dir");
  std::fs::write(app_dir.join("src/main.rs"), "fn main() {}\n").expect("write main.rs");
  write_workspace_manifest(&inner, "1.2.3");
  write_workspace_member_manifest(&app_dir);

  let workspace = get_workspace_dir(&app_dir).expect("warm workspace dir cache");
  assert_eq!(workspace, inner);
  assert!(workspace_machine_cache::machine_path(&app_dir).exists());

  std::fs::write(
    outer.path().join("Cargo.toml"),
    r#"[workspace]
members = ["inner/app"]
"#,
  )
  .expect("write new parent workspace manifest");

  assert!(workspace_machine_cache::read(&app_dir).is_none());
  let workspace = get_workspace_dir(&app_dir).expect("refresh workspace dir cache");
  assert_eq!(workspace, inner);
  assert!(workspace_machine_cache::read(&app_dir).is_some());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
fn dx_workspace_dir_machine_cache_rejects_corrupt_machine_file() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let fixture = rust_app_workspace_fixture("workspace-dir-cache-corrupt");

  let workspace = get_workspace_dir(&fixture.app_dir).expect("warm workspace dir cache");
  assert_eq!(workspace, fixture.root.path());
  std::fs::write(
    workspace_machine_cache::machine_path(&fixture.app_dir),
    b"this is not a dx machine cache",
  )
  .expect("write corrupt workspace cache");

  assert!(workspace_machine_cache::read(&fixture.app_dir).is_none());
  let workspace = get_workspace_dir(&fixture.app_dir).expect("refresh workspace dir cache");
  assert_eq!(workspace, fixture.root.path());
  assert!(workspace_machine_cache::read(&fixture.app_dir).is_some());
}

#[cfg(feature = "dx-machine-cache")]
#[test]
#[ignore = "writes a local timing receipt under DX_TEST_OUTPUT_DIR for manual inspection"]
fn dx_cargo_settings_cache_writes_source_parse_vs_machine_read_receipt() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let project = temp_cargo_settings_project("settings-source-vs-machine-receipt");
  write_cargo_settings_manifest(project.path(), "receipt-app", "1.2.3");
  let source_settings =
    CargoSettings::load_from_source(project.path()).expect("load source settings");
  cargo_settings_machine_cache::write(project.path(), &source_settings)
    .expect("write cargo settings cache");
  assert!(cargo_settings_machine_cache::read(project.path()).is_some());

  let receipt_path = write_cargo_settings_source_vs_machine_timing_receipt(
    project.path(),
    "cargo-settings-source-vs-machine-json-receipt.json",
    "dx_cargo_settings_cache_writes_source_parse_vs_machine_read_receipt",
    "tiny-cargo-settings-manifest",
    tiny_cargo_settings_fixture_stats(),
  )
  .expect("write CargoSettings timing receipt");
  let receipt: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(&receipt_path).expect("read timing receipt"))
      .expect("parse timing receipt");

  assert_eq!(
    receipt["cache_boundary"],
    "CargoSettings projection only: package, workspace.package, and bin"
  );
  assert_eq!(receipt["full_cli_speed_claimed"], false);
  assert_eq!(receipt["upstream_baseline_measured"], false);
  assert_eq!(receipt["release_build_run"], false);
  assert!(
    receipt["source_parse"]["median_ns"]
      .as_u64()
      .expect("source median")
      > 0
  );
  assert!(
    receipt["validated_machine_read"]["median_ns"]
      .as_u64()
      .expect("machine median")
      > 0
  );
}

#[cfg(feature = "dx-machine-cache")]
#[test]
#[ignore = "writes a local timing receipt under DX_TEST_OUTPUT_DIR for manual inspection"]
fn dx_cargo_settings_cache_writes_representative_source_parse_vs_machine_read_receipt() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let project = temp_cargo_settings_project("settings-representative-receipt");
  write_representative_cargo_settings_manifest(project.path(), 180, 24);
  let source_settings =
    CargoSettings::load_from_source(project.path()).expect("load source settings");
  cargo_settings_machine_cache::write(project.path(), &source_settings)
    .expect("write cargo settings cache");
  assert!(cargo_settings_machine_cache::read(project.path()).is_some());

  let receipt_path = write_cargo_settings_source_vs_machine_timing_receipt(
    project.path(),
    "cargo-settings-representative-source-vs-machine-json-receipt.json",
    "dx_cargo_settings_cache_writes_representative_source_parse_vs_machine_read_receipt",
    "representative-cargo-settings-manifest",
    representative_cargo_settings_fixture_stats(),
  )
  .expect("write representative CargoSettings timing receipt");
  let receipt: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(&receipt_path).expect("read timing receipt"))
      .expect("parse timing receipt");

  assert_eq!(receipt["fixture"], "representative-cargo-settings-manifest");
  assert_eq!(
    receipt["fixture_stats"]["source_kind"],
    "large_manifest_parse_fixture"
  );
  assert_eq!(receipt["fixture_stats"]["ignored_dependency_count"], 180);
  assert!(receipt["source_bytes"].as_u64().expect("source bytes") > 10_000);
  assert!(
    receipt["validated_machine_read"]["median_ns"]
      .as_u64()
      .expect("machine median")
      > 0
  );
}

#[cfg(feature = "dx-machine-cache")]
#[test]
#[ignore = "writes a local timing receipt under DX_TEST_OUTPUT_DIR for manual inspection"]
fn dx_cargo_settings_cache_writes_representative_machine_read_phase_receipt() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let project = temp_cargo_settings_project("settings-representative-phase-receipt");
  write_representative_cargo_settings_manifest(project.path(), 180, 24);
  let source_settings =
    CargoSettings::load_from_source(project.path()).expect("load source settings");
  cargo_settings_machine_cache::write(project.path(), &source_settings)
    .expect("write cargo settings cache");

  let receipt_path = write_cargo_settings_machine_read_phase_timing_receipt(
    project.path(),
    "cargo-settings-representative-machine-read-phase-receipt.json",
    "dx_cargo_settings_cache_writes_representative_machine_read_phase_receipt",
    "representative-cargo-settings-manifest",
    representative_cargo_settings_fixture_stats(),
  )
  .expect("write phase timing receipt");
  let receipt: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(&receipt_path).expect("read timing receipt"))
      .expect("parse timing receipt");

  assert_eq!(
    receipt["schema"],
    "dx.tauri.cli.cargo_settings_machine_read_phase_receipt"
  );
  assert_eq!(receipt["source"]["bytes"], receipt["source_bytes"]);
  assert_eq!(receipt["machine"]["bytes"], receipt["machine_bytes"]);
  assert_eq!(
    receipt["source"]["blake3"]
      .as_str()
      .expect("source blake3")
      .len(),
    64
  );
  assert_eq!(receipt["machine_read_validated"], true);
  assert_eq!(receipt["machine_read_hashes_source_bytes"], true);
  assert_eq!(receipt["machine_read_parses_source_cargo_toml"], false);
  assert!(
    receipt["phase_summaries"]["source_fingerprint"]["median_ns"]
      .as_u64()
      .expect("source fingerprint median")
      > 0
  );
  assert!(
    receipt["phase_summaries"]["machine_file_read"]["median_ns"]
      .as_u64()
      .expect("machine read median")
      > 0
  );
  assert!(
    receipt["phase_summaries"]["machine_access_validation"]["median_ns"]
      .as_u64()
      .expect("machine access median")
      > 0
  );
  assert!(
    receipt["phase_summaries"]["owned_projection_materialization"]["median_ns"]
      .as_u64()
      .expect("materialization median")
      > 0
  );
  assert!(
    receipt["phase_summaries"]["total_valid_machine_read"]["median_ns"]
      .as_u64()
      .expect("total median")
      > 0
  );
  assert_phase_sample_count(&receipt, "source_fingerprint");
  assert_phase_sample_count(&receipt, "machine_file_read");
  assert_phase_sample_count(&receipt, "machine_access_validation");
  assert_phase_sample_count(&receipt, "owned_projection_materialization");
}

#[cfg(feature = "dx-machine-cache")]
#[test]
#[ignore = "writes a local timing receipt under DX_TEST_OUTPUT_DIR for manual inspection"]
fn dx_rust_app_settings_writes_new_phase_receipt() {
  let _env = EnvGuard::set(
    cargo_settings_machine_cache::TAURI_DX_MACHINE_CACHE_ENV,
    "1",
  );
  let fixture = rust_app_workspace_fixture("settings-app-new-phase-receipt");
  let member_settings =
    CargoSettings::load_from_source(&fixture.app_dir).expect("load member settings from source");
  cargo_settings_machine_cache::write(&fixture.app_dir, &member_settings)
    .expect("write member CargoSettings cache");
  let workspace_settings = CargoSettings::load_from_source(fixture.root.path())
    .expect("load workspace settings from source");
  cargo_settings_machine_cache::write(fixture.root.path(), &workspace_settings)
    .expect("write workspace CargoSettings cache");

  let receipt_path = write_rust_app_settings_new_phase_receipt(
    &fixture,
    "rust-app-settings-new-phase-receipt.json",
    "dx_rust_app_settings_writes_new_phase_receipt",
  )
  .expect("write RustAppSettings::new phase timing receipt");
  let receipt: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(&receipt_path).expect("read timing receipt"))
      .expect("parse timing receipt");

  assert_eq!(
    receipt["schema"],
    "dx.tauri.cli.rust_app_settings_new_phase_receipt"
  );
  assert_eq!(receipt["cache_boundary"], "RustAppSettings::new");
  assert_eq!(receipt["workspace_dir_discovery_measured"], true);
  assert_eq!(
    receipt["cargo_metadata_expected_on_warm_workspace_cache_hit"],
    false
  );
  assert_eq!(receipt["cargo_config_machine_read_expected"], false);
  assert_eq!(receipt["cargo_config_empty_target_cached"], false);
  assert_eq!(receipt["rust_app_settings_new_measured"], true);
  assert_eq!(receipt["full_cli_speed_claimed"], false);
  assert_eq!(receipt["upstream_baseline_measured"], false);
  assert!(
    receipt["phase_summaries"]["total_rust_app_settings_new"]["median_ns"]
      .as_u64()
      .expect("total median")
      > 0
  );
  assert!(
    receipt["phase_summaries"]["workspace_dir_discovery"]["median_ns"]
      .as_u64()
      .expect("workspace discovery median")
      > 0
  );
  assert_phase_sample_count(&receipt, "app_cargo_settings_load");
  assert_phase_sample_count(&receipt, "workspace_dir_discovery");
  assert_phase_sample_count(&receipt, "workspace_cargo_settings_load");
  assert_phase_sample_count(&receipt, "package_settings_resolution");
  assert_phase_sample_count(&receipt, "cargo_config_load");
  assert_phase_sample_count(&receipt, "target_resolution");
  assert_phase_sample_count(&receipt, "target_platform_resolution");
  assert_phase_sample_count(&receipt, "total_rust_app_settings_new");
}

#[cfg(feature = "dx-machine-cache")]
fn temp_cargo_settings_project(name: &str) -> tempfile::TempDir {
  let base = std::env::var_os("DX_TEST_OUTPUT_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(std::env::temp_dir);
  std::fs::create_dir_all(&base).expect("create test output base");
  tempfile::Builder::new()
    .prefix(&format!("tauri-dx-cargo-settings-{name}-"))
    .tempdir_in(base)
    .expect("create cargo settings temp project")
}

#[cfg(feature = "dx-machine-cache")]
fn write_cargo_settings_manifest(dir: &Path, name: &str, version: &str) {
  std::fs::write(
    dir.join("Cargo.toml"),
    format!(
      r#"[package]
name = "{name}"
version = "{version}"
description = "Source manifest"
homepage = "https://source.example"
authors = ["Source"]
license = "MIT"
default-run = "source"

[[bin]]
name = "source"
path = "src/main.rs"
"#
    ),
  )
  .expect("write Cargo.toml");
}

#[cfg(feature = "dx-machine-cache")]
fn write_representative_cargo_settings_manifest(
  dir: &Path,
  dependency_count: usize,
  bin_count: usize,
) {
  let mut manifest = r#"[package]
name = "representative-app"
version.workspace = true
description = "Representative CargoSettings manifest with many real Cargo tables"
homepage.workspace = true
authors = ["DX", "Tauri", "Serializer"]
license.workspace = true
default-run = "desktop"

[workspace]
members = ["."]
resolver = "2"

[workspace.package]
version = "4.5.6"
description = "Workspace-provided representative description"
homepage = "https://representative.example"
authors = ["Workspace DX"]
license = "MIT"

[workspace.dependencies]
workspace-dep-core = "1.0"
workspace-dep-ui = "1.1"
workspace-dep-build = "1.2"

[features]
default = ["feature-0", "feature-1"]
"#
  .to_string();

  for index in 0..16 {
    manifest.push_str(&format!(
      "feature-{index} = [\"dep-{index:03}/feature-{index}\"]\n"
    ));
  }

  manifest.push_str(
    r#"
[dependencies]
"#,
  );
  for index in 0..dependency_count {
    manifest.push_str(&format!(
      "dep-{index:03} = {{ version = \"1.{major}.{minor}\", default-features = false, features = [\"feature-{feature}\"] }}\n",
      major = index % 30,
      minor = index % 97,
      feature = index % 16
    ));
  }

  manifest.push_str(
    r#"
[dev-dependencies]
"#,
  );
  for index in 0..18 {
    manifest.push_str(&format!(
      "dev-dep-{index:03} = {{ version = \"0.{major}.{minor}\", features = [\"test\"] }}\n",
      major = index % 7,
      minor = index % 23
    ));
  }

  manifest.push_str(
    r#"
[build-dependencies]
"#,
  );
  for index in 0..24 {
    manifest.push_str(&format!(
      "build-dep-{index:03} = {{ version = \"0.{major}.{minor}\", features = [\"build\"] }}\n",
      major = index % 9,
      minor = index % 17
    ));
  }

  manifest.push_str(
    r#"
[target.'cfg(windows)'.dependencies]
windows-only-dep = { version = "0.1", features = ["win32"] }

[package.metadata.bundle]
identifier = "com.dx.representative"
resources = ["assets/icon.png", "assets/config/default.json"]
copyright = "DX"

[[bin]]
name = "desktop"
path = "src/main.rs"
"#,
  );
  for index in 1..bin_count {
    manifest.push_str(&format!(
      r#"
[[bin]]
name = "tool-{index:03}"
filename = "tool-{index:03}-runner"
path = "src/bin/tool-{index:03}.rs"
required-features = ["feature-{feature}"]
"#,
      feature = index % 16
    ));
  }

  std::fs::write(dir.join("Cargo.toml"), manifest).expect("write representative Cargo.toml");
}

#[cfg(feature = "dx-machine-cache")]
fn tiny_cargo_settings_fixture_stats() -> serde_json::Value {
  serde_json::json!({
    "source_kind": "tiny_projection_fixture",
    "parsed_bin_count": 1,
    "ignored_dependency_count": 0,
    "ignored_build_dependency_count": 0,
    "ignored_feature_count": 0,
    "ignored_metadata_section_count": 0
  })
}

#[cfg(feature = "dx-machine-cache")]
fn representative_cargo_settings_fixture_stats() -> serde_json::Value {
  serde_json::json!({
    "source_kind": "large_manifest_parse_fixture",
    "parsed_bin_count": 24,
    "has_workspace_package": true,
    "has_workspace_inheritance": true,
    "ignored_dependency_count": 180,
    "ignored_build_dependency_count": 24,
    "ignored_dev_dependency_count": 18,
    "ignored_workspace_dependency_count": 3,
    "ignored_target_dependency_count": 1,
    "ignored_feature_count": 16,
    "ignored_metadata_section_count": 1
  })
}

#[cfg(feature = "dx-machine-cache")]
fn write_workspace_manifest(dir: &Path, version: &str) {
  std::fs::write(
    dir.join("Cargo.toml"),
    format!(
      r#"[workspace]
members = ["app"]

[workspace.package]
version = "{version}"
description = "Workspace description"
homepage = "https://workspace.example"
authors = ["Workspace"]
license = "Apache-2.0"
"#
    ),
  )
  .expect("write workspace Cargo.toml");
}

#[cfg(feature = "dx-machine-cache")]
fn write_workspace_member_manifest(dir: &Path) {
  std::fs::write(
    dir.join("Cargo.toml"),
    r#"[package]
name = "workspace-app"
version.workspace = true
description.workspace = true
homepage.workspace = true
authors.workspace = true
license.workspace = true
default-run = "workspace-app"

[[bin]]
name = "workspace-app"
path = "src/main.rs"
"#,
  )
  .expect("write workspace member Cargo.toml");
}

#[cfg(feature = "dx-machine-cache")]
struct RustAppWorkspaceFixture {
  root: tempfile::TempDir,
  app_dir: PathBuf,
}

#[cfg(feature = "dx-machine-cache")]
fn rust_app_workspace_fixture(name: &str) -> RustAppWorkspaceFixture {
  let root = temp_cargo_settings_project(name);
  let app_dir = root.path().join("app");
  std::fs::create_dir_all(app_dir.join("src")).expect("create app src dir");
  std::fs::write(app_dir.join("src/main.rs"), "fn main() {}\n").expect("write main.rs");
  write_workspace_manifest(root.path(), "1.2.3");
  write_workspace_member_manifest(&app_dir);
  RustAppWorkspaceFixture { root, app_dir }
}

#[cfg(feature = "dx-machine-cache")]
fn write_failing_cargo_cmd(dir: &Path, message: &str) {
  std::fs::create_dir_all(dir).expect("create fake PATH directory");
  std::fs::write(
    dir.join("cargo.cmd"),
    format!("@echo off\r\necho {message} 1>&2\r\nexit /b 57\r\n"),
  )
  .expect("write fake cargo");
}

#[cfg(feature = "dx-machine-cache")]
fn rust_app_settings_from_fixture(fixture: &RustAppWorkspaceFixture) -> RustAppSettings {
  let config = test_tauri_config();
  rust_app_settings_from_fixture_with_config(fixture, &config)
}

#[cfg(feature = "dx-machine-cache")]
fn rust_app_settings_from_fixture_with_config(
  fixture: &RustAppWorkspaceFixture,
  config: &Config,
) -> RustAppSettings {
  RustAppSettings::new(
    config,
    manifest_from_dir(&fixture.app_dir),
    Some(test_target_triple()),
    &fixture.app_dir,
  )
  .expect("create RustAppSettings")
}

#[cfg(feature = "dx-machine-cache")]
fn manifest_from_dir(dir: &Path) -> Manifest {
  Manifest {
    inner: std::fs::read_to_string(dir.join("Cargo.toml"))
      .expect("read Cargo.toml")
      .parse::<toml_edit::DocumentMut>()
      .expect("parse Cargo.toml"),
    tauri_features: Default::default(),
  }
}

#[cfg(feature = "dx-machine-cache")]
fn test_tauri_config() -> Config {
  Config {
    identifier: "com.dx.cached-settings".into(),
    ..Default::default()
  }
}

#[cfg(all(feature = "dx-machine-cache", windows))]
fn test_target_triple() -> String {
  "x86_64-pc-windows-msvc".into()
}

#[cfg(all(feature = "dx-machine-cache", target_os = "macos"))]
fn test_target_triple() -> String {
  "x86_64-apple-darwin".into()
}

#[cfg(all(feature = "dx-machine-cache", target_os = "linux"))]
fn test_target_triple() -> String {
  "x86_64-unknown-linux-gnu".into()
}

#[cfg(feature = "dx-machine-cache")]
fn cargo_settings_machine_path(dir: &Path) -> PathBuf {
  cargo_settings_machine_cache::machine_path(dir)
}

#[cfg(feature = "dx-machine-cache")]
fn sample_cargo_settings(name: &str) -> CargoSettings {
  CargoSettings {
    package: Some(CargoPackageSettings {
      name: name.into(),
      version: Some(MaybeWorkspace::Workspace(TomlWorkspaceField {
        workspace: true,
      })),
      description: Some(MaybeWorkspace::Defined("Cached description".into())),
      homepage: Some(MaybeWorkspace::Workspace(TomlWorkspaceField {
        workspace: true,
      })),
      authors: Some(MaybeWorkspace::Defined(vec!["DX".into(), "Tauri".into()])),
      license: Some(MaybeWorkspace::Workspace(TomlWorkspaceField {
        workspace: true,
      })),
      default_run: Some("desktop".into()),
    }),
    workspace: Some(WorkspaceSettings {
      package: Some(WorkspacePackageSettings {
        authors: Some(vec!["Workspace".into()]),
        description: Some("Workspace description".into()),
        homepage: Some("https://workspace.example".into()),
        version: Some("9.9.9".into()),
        license: Some("Apache-2.0".into()),
      }),
    }),
    bin: Some(vec![BinarySettings {
      name: "desktop".into(),
      filename: Some("dx-desktop".into()),
      path: Some("src/bin/desktop.rs".into()),
      required_features: Some(vec!["custom-protocol".into()]),
    }]),
  }
}

#[cfg(feature = "dx-machine-cache")]
fn cached_workspace_member_settings(name: &str) -> CargoSettings {
  CargoSettings {
    package: Some(CargoPackageSettings {
      name: name.into(),
      version: Some(MaybeWorkspace::Workspace(TomlWorkspaceField {
        workspace: true,
      })),
      description: Some(MaybeWorkspace::Workspace(TomlWorkspaceField {
        workspace: true,
      })),
      homepage: Some(MaybeWorkspace::Workspace(TomlWorkspaceField {
        workspace: true,
      })),
      authors: Some(MaybeWorkspace::Workspace(TomlWorkspaceField {
        workspace: true,
      })),
      license: Some(MaybeWorkspace::Workspace(TomlWorkspaceField {
        workspace: true,
      })),
      default_run: Some("desktop".into()),
    }),
    workspace: None,
    bin: Some(vec![BinarySettings {
      name: "desktop".into(),
      filename: None,
      path: Some("src/main.rs".into()),
      required_features: None,
    }]),
  }
}

#[cfg(feature = "dx-machine-cache")]
fn cached_direct_member_settings(name: &str) -> CargoSettings {
  CargoSettings {
    package: Some(CargoPackageSettings {
      name: name.into(),
      version: Some(MaybeWorkspace::Defined("7.7.7".into())),
      description: Some(MaybeWorkspace::Defined("Cached direct description".into())),
      homepage: Some(MaybeWorkspace::Defined(
        "https://cached-direct.example".into(),
      )),
      authors: Some(MaybeWorkspace::Defined(vec!["Direct Author".into()])),
      license: Some(MaybeWorkspace::Defined("MIT".into())),
      default_run: Some("desktop".into()),
    }),
    workspace: None,
    bin: Some(vec![BinarySettings {
      name: "desktop".into(),
      filename: None,
      path: Some("src/main.rs".into()),
      required_features: None,
    }]),
  }
}

#[cfg(feature = "dx-machine-cache")]
fn cached_member_settings_with_bins(name: &str) -> CargoSettings {
  let mut settings = cached_workspace_member_settings(name);
  settings.bin = Some(vec![
    BinarySettings {
      name: "desktop".into(),
      filename: None,
      path: Some("src/main.rs".into()),
      required_features: None,
    },
    BinarySettings {
      name: "helper".into(),
      filename: Some("helper-cli".into()),
      path: Some("src/bin/helper.rs".into()),
      required_features: Some(vec!["helper-feature".into()]),
    },
    BinarySettings {
      name: "gated".into(),
      filename: Some("gated-cli".into()),
      path: Some("src/bin/gated.rs".into()),
      required_features: Some(vec!["missing-feature".into()]),
    },
  ]);
  settings
}

#[cfg(feature = "dx-machine-cache")]
fn cached_workspace_root_settings(
  version: &str,
  description: &str,
  homepage: &str,
  authors: Vec<String>,
  license: &str,
) -> CargoSettings {
  CargoSettings {
    package: None,
    workspace: Some(WorkspaceSettings {
      package: Some(WorkspacePackageSettings {
        authors: Some(authors),
        description: Some(description.into()),
        homepage: Some(homepage.into()),
        version: Some(version.into()),
        license: Some(license.into()),
      }),
    }),
    bin: None,
  }
}

#[cfg(feature = "dx-machine-cache")]
fn write_cargo_settings_source_vs_machine_timing_receipt(
  dir: &Path,
  receipt_file_name: &str,
  test_filter: &str,
  fixture: &str,
  fixture_stats: serde_json::Value,
) -> crate::Result<PathBuf> {
  let iterations = 100;
  let warmups = 10;
  for _ in 0..warmups {
    let _ = CargoSettings::load_from_source(dir)?;
    let _ = read_valid_cargo_settings_machine(dir)?;
  }

  let source_samples = measure_cargo_settings_samples(iterations, || {
    let _ = CargoSettings::load_from_source(dir)?;
    Ok(())
  })?;
  let machine_samples =
    measure_cargo_settings_samples(iterations, || read_valid_cargo_settings_machine(dir))?;

  let cargo_toml_path = dir.join("Cargo.toml");
  let machine_path = cargo_settings_machine_path(dir);
  let source_bytes = std::fs::metadata(&cargo_toml_path)
    .map(|metadata| metadata.len())
    .unwrap_or_default();
  let machine_bytes = std::fs::metadata(&machine_path)
    .map(|metadata| metadata.len())
    .unwrap_or_default();
  let source_summary = timing_summary(&source_samples);
  let machine_summary = timing_summary(&machine_samples);
  let source_median = source_summary["median_ns"].as_u64().unwrap_or(0);
  let machine_median = machine_summary["median_ns"].as_u64().unwrap_or(0);
  let median_ratio_percent = if source_median == 0 {
    serde_json::Value::Null
  } else {
    serde_json::json!(machine_median.saturating_mul(100) / source_median)
  };
  let receipt = serde_json::json!({
    "schema": "dx.tauri.cli.cargo_settings_source_vs_machine_receipt",
    "schema_version": 1,
    "created_unix_ms": current_unix_ms(),
    "fixture": fixture,
    "fixture_stats": fixture_stats,
    "cache_boundary": "CargoSettings projection only: package, workspace.package, and bin",
    "command": format!("cargo test --manifest-path G:\\Dx\\tauri\\crates\\tauri-cli\\Cargo.toml --no-default-features --features dx-machine-cache --lib {test_filter} -j1 --color never -- --ignored --test-threads=1"),
    "cargo_toml_path": cargo_toml_path.display().to_string(),
    "machine_path": machine_path.display().to_string(),
    "source_bytes": source_bytes,
    "machine_bytes": machine_bytes,
    "warmups": warmups,
    "iterations": iterations,
    "source_parse": source_summary,
    "validated_machine_read": machine_summary,
    "machine_to_source_median_ratio_percent": median_ratio_percent,
    "cache_write_included_in_timing": false,
    "fallback_used": false,
    "full_cli_speed_claimed": false,
    "upstream_baseline_measured": false,
    "faster_than_upstream_claimed": false,
    "release_build_run": false,
    "app_runtime_measured": false,
    "webview_startup_measured": false,
    "bundle_or_installer_measured": false,
    "notes": [
      "Validated machine read still reads and fingerprints the source Cargo.toml before bytecheck/rkyv validation.",
      "This receipt does not measure cargo metadata, RustAppSettings::new, app startup, bundling, or official upstream Tauri."
    ]
  });

  let output_dir = std::env::var_os("DX_TEST_OUTPUT_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(std::env::temp_dir);
  std::fs::create_dir_all(&output_dir).fs_context(
    "failed to create CargoSettings receipt output dir",
    &output_dir,
  )?;
  let receipt_path = output_dir.join(receipt_file_name);
  std::fs::write(
    &receipt_path,
    serde_json::to_vec_pretty(&receipt).expect("serialize timing receipt"),
  )
  .fs_context(
    "failed to write CargoSettings timing receipt",
    &receipt_path,
  )?;
  Ok(receipt_path)
}

#[cfg(feature = "dx-machine-cache")]
fn write_cargo_settings_machine_read_phase_timing_receipt(
  dir: &Path,
  receipt_file_name: &str,
  test_filter: &str,
  fixture: &str,
  fixture_stats: serde_json::Value,
) -> crate::Result<PathBuf> {
  let iterations = 100;
  let warmups = 10;
  for _ in 0..warmups {
    let _ = read_valid_cargo_settings_machine_with_phase_timings(dir)?;
  }

  let samples = measure_cargo_settings_phase_samples(iterations, dir)?;
  let first = samples
    .first()
    .expect("phase timing samples should not be empty");
  let receipt = serde_json::json!({
    "schema": "dx.tauri.cli.cargo_settings_machine_read_phase_receipt",
    "schema_version": 1,
    "created_unix_ms": current_unix_ms(),
    "fixture": fixture,
    "fixture_stats": fixture_stats,
    "cache_boundary": "CargoSettings projection only: package, workspace.package, and bin",
    "command": format!("cargo test --manifest-path G:\\Dx\\tauri\\crates\\tauri-cli\\Cargo.toml --no-default-features --features dx-machine-cache --lib {test_filter} -j1 --color never -- --ignored --test-threads=1"),
    "source": {
      "path": first.source_path,
      "bytes": first.source_bytes,
      "modified_unix_ms": first.source_modified_unix_ms,
      "blake3": first.source_blake3_hex
    },
    "machine": {
      "path": first.machine_path,
      "bytes": first.machine_bytes
    },
    "source_bytes": first.source_bytes,
    "machine_bytes": first.machine_bytes,
    "warmups": warmups,
    "iterations": iterations,
    "phase_summaries": {
      "path_resolution": phase_timing_summary(&samples, |sample| sample.path_resolution_ns),
      "machine_size_preflight": phase_timing_summary(&samples, |sample| sample.machine_size_preflight_ns),
      "source_fingerprint": phase_timing_summary(&samples, |sample| sample.source_fingerprint_ns),
      "machine_file_read": phase_timing_summary(&samples, |sample| sample.machine_file_read_ns),
      "machine_access_validation": phase_timing_summary(&samples, |sample| sample.machine_access_validation_ns),
      "owned_projection_materialization": phase_timing_summary(&samples, |sample| sample.owned_projection_materialization_ns),
      "total_valid_machine_read": phase_timing_summary(&samples, |sample| sample.total_ns)
    },
    "machine_read_validated": true,
    "machine_read_includes_source_fingerprint": true,
    "machine_read_hashes_source_bytes": true,
    "machine_read_parses_source_cargo_toml": false,
    "cache_write_included_in_timing": false,
    "fallback_used": false,
    "cargo_metadata_measured": false,
    "rust_app_settings_new_measured": false,
    "full_cli_speed_claimed": false,
    "upstream_baseline_measured": false,
    "faster_than_upstream_claimed": false,
    "release_build_run": false,
    "app_runtime_measured": false,
    "webview_startup_measured": false,
    "bundle_or_installer_measured": false,
    "notes": [
      "Phase timing decomposes only a valid CargoSettings .machine read.",
      "The source_fingerprint phase keeps full source hashing and does not use metadata-only validation.",
      "The machine_access_validation phase includes serializer envelope validation, payload validation, and archived access.",
      "The owned_projection_materialization phase includes archive deserialization, path guard, and conversion into CargoSettings."
    ]
  });

  let output_dir = std::env::var_os("DX_TEST_OUTPUT_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(std::env::temp_dir);
  std::fs::create_dir_all(&output_dir).fs_context(
    "failed to create CargoSettings phase receipt output dir",
    &output_dir,
  )?;
  let receipt_path = output_dir.join(receipt_file_name);
  std::fs::write(
    &receipt_path,
    serde_json::to_vec_pretty(&receipt).expect("serialize phase timing receipt"),
  )
  .fs_context(
    "failed to write CargoSettings phase timing receipt",
    &receipt_path,
  )?;
  Ok(receipt_path)
}

#[cfg(feature = "dx-machine-cache")]
#[derive(Debug, Clone)]
struct RustAppSettingsNewPhaseTimings {
  app_cargo_settings_load_ns: u64,
  app_package_settings_clone_ns: u64,
  workspace_dir_discovery_ns: u64,
  workspace_cargo_settings_load_ns: u64,
  package_settings_resolution_ns: u64,
  cargo_config_load_ns: u64,
  target_resolution_ns: u64,
  target_platform_resolution_ns: u64,
  construction_ns: u64,
  total_ns: u64,
  workspace_dir: String,
}

#[cfg(feature = "dx-machine-cache")]
fn write_rust_app_settings_new_phase_receipt(
  fixture: &RustAppWorkspaceFixture,
  receipt_file_name: &str,
  test_filter: &str,
) -> crate::Result<PathBuf> {
  let iterations = 20;
  let warmups = 3;
  let config = test_tauri_config();
  for _ in 0..warmups {
    let _ = rust_app_settings_new_with_phase_timings(
      &config,
      manifest_from_dir(&fixture.app_dir),
      Some(test_target_triple()),
      &fixture.app_dir,
    )?;
  }

  let samples = measure_rust_app_settings_new_phase_samples(iterations, fixture, &config)?;
  let first = samples
    .first()
    .expect("RustAppSettings phase timing samples should not be empty");
  let member_cargo_toml_path = fixture.app_dir.join("Cargo.toml");
  let workspace_cargo_toml_path = fixture.root.path().join("Cargo.toml");
  let member_machine_path = cargo_settings_machine_path(&fixture.app_dir);
  let workspace_machine_path = cargo_settings_machine_path(fixture.root.path());
  let fixture_stats = serde_json::json!({
      "source_kind": "minimal_workspace_member_fixture",
      "workspace_members": 1,
      "member_manifest_uses_workspace_package_inheritance": true,
      "target_triple_provided": true,
      "rustc_host_probe_included": false,
      "member_cargo_toml_bytes": file_len(&member_cargo_toml_path),
      "workspace_cargo_toml_bytes": file_len(&workspace_cargo_toml_path),
      "member_machine_bytes": file_len(&member_machine_path),
      "workspace_machine_bytes": file_len(&workspace_machine_path)
  });
  let phase_summaries = serde_json::json!({
      "app_cargo_settings_load": rust_app_settings_new_phase_timing_summary(&samples, |sample| sample.app_cargo_settings_load_ns),
      "app_package_settings_clone": rust_app_settings_new_phase_timing_summary(&samples, |sample| sample.app_package_settings_clone_ns),
      "workspace_dir_discovery": rust_app_settings_new_phase_timing_summary(&samples, |sample| sample.workspace_dir_discovery_ns),
      "workspace_cargo_settings_load": rust_app_settings_new_phase_timing_summary(&samples, |sample| sample.workspace_cargo_settings_load_ns),
      "package_settings_resolution": rust_app_settings_new_phase_timing_summary(&samples, |sample| sample.package_settings_resolution_ns),
      "cargo_config_load": rust_app_settings_new_phase_timing_summary(&samples, |sample| sample.cargo_config_load_ns),
      "target_resolution": rust_app_settings_new_phase_timing_summary(&samples, |sample| sample.target_resolution_ns),
      "target_platform_resolution": rust_app_settings_new_phase_timing_summary(&samples, |sample| sample.target_platform_resolution_ns),
      "construction": rust_app_settings_new_phase_timing_summary(&samples, |sample| sample.construction_ns),
      "total_rust_app_settings_new": rust_app_settings_new_phase_timing_summary(&samples, |sample| sample.total_ns)
  });
  let notes = serde_json::json!([
      "This receipt mirrors RustAppSettings::new in a test-only helper so production behavior is not changed by measurement.",
      "The fixture passes an explicit target triple, so the rustc -vV host probe is intentionally excluded.",
      "The workspace_dir_discovery phase measures get_workspace_dir; with TAURI_DX_MACHINE_CACHE enabled and warm sidecars it is expected to hit cargo-workspace.machine instead of spawning cargo metadata.",
      "The two CargoSettings::load phases and workspace_dir_discovery phase are the parts accelerated by dx serializer .machine sidecars in this no-target fixture.",
      "CargoConfig target-bearing cache behavior is covered by focused tests, but this no-target receipt fixture intentionally skips cargo-config.machine because measured no-target reads were slower than live source probing."
  ]);
  let mut receipt = serde_json::Map::new();
  {
    let mut insert = |key: &str, value: serde_json::Value| {
      receipt.insert(key.to_string(), value);
    };
    insert(
      "schema",
      serde_json::json!("dx.tauri.cli.rust_app_settings_new_phase_receipt"),
    );
    insert("schema_version", serde_json::json!(1));
    insert("created_unix_ms", serde_json::json!(current_unix_ms()));
    insert(
      "fixture",
      serde_json::json!("tiny-rust-app-settings-workspace"),
    );
    insert("fixture_stats", fixture_stats);
    insert("cache_boundary", serde_json::json!("RustAppSettings::new"));
    insert(
      "command",
      serde_json::json!(format!("cargo test --manifest-path G:\\Dx\\tauri\\crates\\tauri-cli\\Cargo.toml --no-default-features --features dx-machine-cache --lib {test_filter} -j1 --color never -- --ignored --test-threads=1")),
    );
    insert(
      "app_dir",
      serde_json::json!(fixture.app_dir.display().to_string()),
    );
    insert(
      "workspace_dir",
      serde_json::json!(first.workspace_dir.as_str()),
    );
    insert(
      "member_cargo_toml_path",
      serde_json::json!(member_cargo_toml_path.display().to_string()),
    );
    insert(
      "workspace_cargo_toml_path",
      serde_json::json!(workspace_cargo_toml_path.display().to_string()),
    );
    insert(
      "member_machine_path",
      serde_json::json!(member_machine_path.display().to_string()),
    );
    insert(
      "workspace_machine_path",
      serde_json::json!(workspace_machine_path.display().to_string()),
    );
    insert("warmups", serde_json::json!(warmups));
    insert("iterations", serde_json::json!(iterations));
    insert("phase_summaries", phase_summaries);
    insert("machine_cache_enabled", serde_json::json!(true));
    insert(
      "machine_cache_warmed_before_timing",
      serde_json::json!(true),
    );
    insert(
      "workspace_dir_machine_cache_expected",
      serde_json::json!(true),
    );
    insert(
      "cargo_settings_machine_reads_expected",
      serde_json::json!(2),
    );
    insert(
      "cargo_config_machine_read_expected",
      serde_json::json!(false),
    );
    insert("cargo_config_empty_target_cached", serde_json::json!(false));
    insert("workspace_dir_discovery_measured", serde_json::json!(true));
    insert("cargo_metadata_measured", serde_json::json!(false));
    insert(
      "cargo_metadata_included_on_workspace_cache_miss",
      serde_json::json!(true),
    );
    insert(
      "cargo_metadata_expected_on_warm_workspace_cache_hit",
      serde_json::json!(false),
    );
    insert("rust_app_settings_new_measured", serde_json::json!(true));
    insert(
      "manifest_rewrite_or_parse_measured",
      serde_json::json!(false),
    );
    insert("rustc_host_probe_measured", serde_json::json!(false));
    insert("cache_write_included_in_timing", serde_json::json!(false));
    insert("fallback_used", serde_json::json!(false));
    insert("full_cli_speed_claimed", serde_json::json!(false));
    insert("upstream_baseline_measured", serde_json::json!(false));
    insert("faster_than_upstream_claimed", serde_json::json!(false));
    insert("release_build_run", serde_json::json!(false));
    insert("app_runtime_measured", serde_json::json!(false));
    insert("webview_startup_measured", serde_json::json!(false));
    insert("bundle_or_installer_measured", serde_json::json!(false));
    insert("notes", notes);
  }
  let receipt = serde_json::Value::Object(receipt);

  let output_dir = std::env::var_os("DX_TEST_OUTPUT_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(std::env::temp_dir);
  std::fs::create_dir_all(&output_dir).fs_context(
    "failed to create RustAppSettings phase receipt output dir",
    &output_dir,
  )?;
  let receipt_path = output_dir.join(receipt_file_name);
  std::fs::write(
    &receipt_path,
    serde_json::to_vec_pretty(&receipt).expect("serialize RustAppSettings phase receipt"),
  )
  .fs_context(
    "failed to write RustAppSettings phase timing receipt",
    &receipt_path,
  )?;
  Ok(receipt_path)
}

#[cfg(feature = "dx-machine-cache")]
fn rust_app_settings_new_with_phase_timings(
  config: &Config,
  manifest: Manifest,
  target: Option<String>,
  tauri_dir: &Path,
) -> crate::Result<(RustAppSettings, RustAppSettingsNewPhaseTimings)> {
  let total_started = std::time::Instant::now();

  let phase_started = std::time::Instant::now();
  let cargo_settings = CargoSettings::load(tauri_dir).context("failed to load Cargo settings")?;
  let app_cargo_settings_load_ns = elapsed_ns(phase_started);

  let phase_started = std::time::Instant::now();
  let cargo_package_settings = match &cargo_settings.package {
    Some(package_info) => package_info.clone(),
    None => {
      return Err(crate::Error::GenericError(
        "No package info in the config file".to_owned(),
      ))
    }
  };
  let app_package_settings_clone_ns = elapsed_ns(phase_started);

  let phase_started = std::time::Instant::now();
  let workspace_dir = get_workspace_dir(tauri_dir)?;
  let workspace_dir_discovery_ns = elapsed_ns(phase_started);

  let phase_started = std::time::Instant::now();
  let ws_package_settings = CargoSettings::load(&workspace_dir)
    .context("failed to load Cargo settings from workspace root")?
    .workspace
    .and_then(|v| v.package);
  let workspace_cargo_settings_load_ns = elapsed_ns(phase_started);

  let phase_started = std::time::Instant::now();
  let version = config.version.clone().unwrap_or_else(|| {
    cargo_package_settings
      .version
      .clone()
      .expect("Cargo manifest must have the `package.version` field")
      .resolve("version", || {
        ws_package_settings
          .as_ref()
          .and_then(|p| p.version.clone())
          .context("Couldn't inherit value for `version` from workspace")
      })
      .expect("Cargo project does not have a version")
  });

  let package_settings = PackageSettings {
    product_name: config
      .product_name
      .clone()
      .unwrap_or_else(|| cargo_package_settings.name.clone()),
    version,
    description: cargo_package_settings
      .description
      .clone()
      .map(|description| {
        description
          .resolve("description", || {
            ws_package_settings
              .as_ref()
              .and_then(|v| v.description.clone())
              .context("Couldn't inherit value for `description` from workspace")
          })
          .unwrap()
      })
      .unwrap_or_default(),
    homepage: cargo_package_settings.homepage.clone().map(|homepage| {
      homepage
        .resolve("homepage", || {
          ws_package_settings
            .as_ref()
            .and_then(|v| v.homepage.clone())
            .context("Couldn't inherit value for `homepage` from workspace")
        })
        .unwrap()
    }),
    authors: cargo_package_settings.authors.clone().map(|authors| {
      authors
        .resolve("authors", || {
          ws_package_settings
            .as_ref()
            .and_then(|v| v.authors.clone())
            .context("Couldn't inherit value for `authors` from workspace")
        })
        .unwrap()
    }),
    default_run: cargo_package_settings.default_run.clone(),
  };
  let package_settings_resolution_ns = elapsed_ns(phase_started);

  let phase_started = std::time::Instant::now();
  let cargo_config = CargoConfig::load(tauri_dir)?;
  let cargo_config_load_ns = elapsed_ns(phase_started);

  let phase_started = std::time::Instant::now();
  let target_triple = target.unwrap_or_else(|| {
    cargo_config
      .build()
      .target()
      .map(|t| t.to_string())
      .unwrap_or_else(|| {
        let output = Command::new("rustc")
          .args(["-vV"])
          .output()
          .expect("\"rustc\" could not be found, did you install Rust?");
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
          .split('\n')
          .find(|l| l.starts_with("host:"))
          .unwrap()
          .replace("host:", "")
          .trim()
          .to_string()
      })
  });
  let target_resolution_ns = elapsed_ns(phase_started);

  let phase_started = std::time::Instant::now();
  let target_platform = TargetPlatform::from_triple(&target_triple);
  let target_platform_resolution_ns = elapsed_ns(phase_started);

  let phase_started = std::time::Instant::now();
  let settings = RustAppSettings {
    manifest: Mutex::new(manifest),
    cargo_settings,
    cargo_package_settings,
    cargo_ws_package_settings: ws_package_settings,
    package_settings,
    cargo_config,
    target_triple,
    target_platform,
    workspace_dir: workspace_dir.clone(),
  };
  let construction_ns = elapsed_ns(phase_started);

  let total_ns = elapsed_ns(total_started);
  Ok((
    settings,
    RustAppSettingsNewPhaseTimings {
      app_cargo_settings_load_ns,
      app_package_settings_clone_ns,
      workspace_dir_discovery_ns,
      workspace_cargo_settings_load_ns,
      package_settings_resolution_ns,
      cargo_config_load_ns,
      target_resolution_ns,
      target_platform_resolution_ns,
      construction_ns,
      total_ns,
      workspace_dir: workspace_dir.display().to_string(),
    },
  ))
}

#[cfg(feature = "dx-machine-cache")]
fn measure_rust_app_settings_new_phase_samples(
  iterations: usize,
  fixture: &RustAppWorkspaceFixture,
  config: &Config,
) -> crate::Result<Vec<RustAppSettingsNewPhaseTimings>> {
  let mut samples = Vec::with_capacity(iterations);
  for _ in 0..iterations {
    let (settings, timings) = rust_app_settings_new_with_phase_timings(
      config,
      manifest_from_dir(&fixture.app_dir),
      Some(test_target_triple()),
      &fixture.app_dir,
    )?;
    assert_eq!(settings.get_package_settings().version, "1.2.3");
    samples.push(timings);
  }
  Ok(samples)
}

#[cfg(feature = "dx-machine-cache")]
fn rust_app_settings_new_phase_timing_summary<F>(
  samples: &[RustAppSettingsNewPhaseTimings],
  sample_value: F,
) -> serde_json::Value
where
  F: Fn(&RustAppSettingsNewPhaseTimings) -> u64,
{
  let values = samples.iter().map(sample_value).collect::<Vec<_>>();
  timing_summary(&values)
}

#[cfg(feature = "dx-machine-cache")]
fn elapsed_ns(started: std::time::Instant) -> u64 {
  u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

#[cfg(feature = "dx-machine-cache")]
fn file_len(path: &Path) -> u64 {
  std::fs::metadata(path)
    .map(|metadata| metadata.len())
    .unwrap_or_default()
}

#[cfg(feature = "dx-machine-cache")]
fn read_valid_cargo_settings_machine(dir: &Path) -> crate::Result<()> {
  cargo_settings_machine_cache::read(dir)
    .map(|_| ())
    .ok_or_else(|| Error::GenericError("CargoSettings machine cache miss".into()))
}

#[cfg(feature = "dx-machine-cache")]
fn read_valid_cargo_settings_machine_with_phase_timings(
  dir: &Path,
) -> crate::Result<cargo_settings_machine_cache::CargoSettingsMachineReadPhaseTimings> {
  cargo_settings_machine_cache::read_with_phase_timings(dir)
    .map(|(_, timings)| timings)
    .ok_or_else(|| Error::GenericError("CargoSettings machine cache miss".into()))
}

#[cfg(feature = "dx-machine-cache")]
fn measure_cargo_settings_samples<F>(iterations: usize, mut op: F) -> crate::Result<Vec<u64>>
where
  F: FnMut() -> crate::Result<()>,
{
  let mut samples = Vec::with_capacity(iterations);
  for _ in 0..iterations {
    let started = std::time::Instant::now();
    op()?;
    let elapsed = started.elapsed().as_nanos();
    samples.push(u64::try_from(elapsed).unwrap_or(u64::MAX));
  }
  Ok(samples)
}

#[cfg(feature = "dx-machine-cache")]
fn measure_cargo_settings_phase_samples(
  iterations: usize,
  dir: &Path,
) -> crate::Result<Vec<cargo_settings_machine_cache::CargoSettingsMachineReadPhaseTimings>> {
  let mut samples = Vec::with_capacity(iterations);
  for _ in 0..iterations {
    samples.push(read_valid_cargo_settings_machine_with_phase_timings(dir)?);
  }
  Ok(samples)
}

#[cfg(feature = "dx-machine-cache")]
fn timing_summary(samples: &[u64]) -> serde_json::Value {
  assert!(!samples.is_empty(), "timing samples should not be empty");
  let mut sorted = samples.to_vec();
  sorted.sort_unstable();
  let total = sorted
    .iter()
    .fold(0u128, |total, sample| total + u128::from(*sample));
  serde_json::json!({
    "min_ns": sorted[0],
    "median_ns": sorted[sorted.len() / 2],
    "p95_ns": sorted[percentile_index(sorted.len(), 95)],
    "max_ns": sorted[sorted.len() - 1],
    "mean_ns": u64::try_from(total / sorted.len() as u128).unwrap_or(u64::MAX),
    "samples_ns": samples
  })
}

#[cfg(feature = "dx-machine-cache")]
fn phase_timing_summary<F>(
  samples: &[cargo_settings_machine_cache::CargoSettingsMachineReadPhaseTimings],
  sample_value: F,
) -> serde_json::Value
where
  F: Fn(&cargo_settings_machine_cache::CargoSettingsMachineReadPhaseTimings) -> u64,
{
  let values = samples.iter().map(sample_value).collect::<Vec<_>>();
  timing_summary(&values)
}

#[cfg(feature = "dx-machine-cache")]
fn assert_phase_sample_count(receipt: &serde_json::Value, phase: &str) {
  let expected = receipt["iterations"].as_u64().expect("iterations") as usize;
  let actual = receipt["phase_summaries"][phase]["samples_ns"]
    .as_array()
    .expect("phase samples")
    .len();
  assert_eq!(actual, expected, "{phase} sample count");
}

#[cfg(feature = "dx-machine-cache")]
fn percentile_index(len: usize, percentile: usize) -> usize {
  (((len * percentile) + 99) / 100)
    .saturating_sub(1)
    .min(len.saturating_sub(1))
}

#[cfg(feature = "dx-machine-cache")]
fn current_unix_ms() -> u128 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .expect("system clock before unix epoch")
    .as_millis()
}

#[cfg(feature = "dx-machine-cache")]
fn assert_cargo_settings_projection(settings: &CargoSettings) {
  let package = settings.package.as_ref().expect("package settings");
  assert!(matches!(
    package.version.as_ref(),
    Some(MaybeWorkspace::Workspace(TomlWorkspaceField {
      workspace: true
    }))
  ));
  assert!(matches!(
    package.description.as_ref(),
    Some(MaybeWorkspace::Defined(description)) if description == "Cached description"
  ));
  assert!(matches!(
    package.authors.as_ref(),
    Some(MaybeWorkspace::Defined(authors)) if authors == &vec!["DX".to_string(), "Tauri".to_string()]
  ));
  assert_eq!(package.default_run.as_deref(), Some("desktop"));

  let workspace = settings
    .workspace
    .as_ref()
    .and_then(|workspace| workspace.package.as_ref())
    .expect("workspace package settings");
  assert_eq!(workspace.version.as_deref(), Some("9.9.9"));
  assert_eq!(workspace.license.as_deref(), Some("Apache-2.0"));

  let bin = settings
    .bin
    .as_ref()
    .and_then(|bins| bins.first())
    .expect("binary settings");
  assert_eq!(bin.file_name(), "dx-desktop");
  assert_eq!(bin.path.as_deref(), Some("src/bin/desktop.rs"));
  assert_eq!(
    bin.required_features.as_deref(),
    Some(&["custom-protocol".to_string()][..])
  );
}
