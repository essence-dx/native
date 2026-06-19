// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use super::*;
use std::sync::{Mutex as StdMutex, MutexGuard, OnceLock};

static ENV_LOCK: OnceLock<StdMutex<()>> = OnceLock::new();

const TAURI_DX_MACHINE_CACHE_ENV: &str = "TAURI_DX_MACHINE_CACHE";
const TEST_OUTPUT_ROOT: &str = "G:\\Dx\\test-outputs";

struct EnvGuard {
  vars: Vec<(&'static str, Option<String>)>,
  _lock: MutexGuard<'static, ()>,
}

impl EnvGuard {
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
#[ignore = "writes a local timing receipt under DX_TEST_OUTPUT_DIR for manual inspection"]
fn dx_full_cargo_metadata_machine_cache_writes_current_fork_benchmark_receipt() {
  let tauri_dir = cargo_metadata_benchmark_tauri_dir();
  let receipt_path = write_cargo_metadata_source_vs_machine_receipt(
    &tauri_dir,
    "cargo-metadata-source-vs-machine-receipt.json",
    "dx_full_cargo_metadata_machine_cache_writes_current_fork_benchmark_receipt",
  )
  .expect("write cargo metadata benchmark receipt");
  let receipt: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(&receipt_path).expect("read timing receipt"))
      .expect("parse timing receipt");

  assert_eq!(
    receipt["schema"],
    "dx.tauri.cli.cargo_metadata_source_vs_machine_receipt"
  );
  assert_eq!(receipt["cache_boundary"], "cargo metadata --no-deps");
  assert_eq!(receipt["machine_cache_generation_measured"], false);
  assert_eq!(receipt["cache_write_included_in_timing"], false);
  assert_eq!(receipt["machine_cache_write_env_for_timing"], "0");
  assert_eq!(receipt["machine_file_unchanged_during_timing"], true);
  assert_eq!(receipt["faster_than_upstream_claimed"], false);
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

#[test]
#[ignore = "writes a local timing receipt under DX_TEST_OUTPUT_DIR for manual inspection"]
fn dx_watch_folders_machine_cache_writes_current_fork_benchmark_receipt() {
  let tauri_dir = cargo_metadata_benchmark_tauri_dir();
  let receipt_path = write_watch_folders_source_vs_machine_receipt(
    &tauri_dir,
    "watch-folders-source-vs-machine-receipt.json",
    "dx_watch_folders_machine_cache_writes_current_fork_benchmark_receipt",
  )
  .expect("write watch folders benchmark receipt");
  let receipt: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(&receipt_path).expect("read timing receipt"))
      .expect("parse timing receipt");

  assert_eq!(
    receipt["schema"],
    "dx.tauri.cli.watch_folders_source_vs_machine_receipt"
  );
  assert_eq!(receipt["cache_boundary"], "get_watch_folders");
  assert_eq!(receipt["machine_cache_generation_measured"], false);
  assert_eq!(receipt["cache_write_included_in_timing"], false);
  assert_eq!(receipt["machine_cache_write_env_for_timing"], "0");
  assert_eq!(receipt["machine_file_unchanged_during_timing"], true);
  assert_eq!(receipt["faster_than_upstream_claimed"], false);
  assert!(
    receipt["source_watch_folders"]["median_ns"]
      .as_u64()
      .expect("source median")
      > 0
  );
  assert!(
    receipt["machine_watch_folders"]["median_ns"]
      .as_u64()
      .expect("machine median")
      > 0
  );
}

fn cargo_metadata_benchmark_tauri_dir() -> PathBuf {
  std::env::var_os("DX_TAURI_CARGO_METADATA_BENCH_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

fn write_cargo_metadata_source_vs_machine_receipt(
  tauri_dir: &Path,
  receipt_file_name: &str,
  test_filter: &str,
) -> crate::Result<PathBuf> {
  let output_dir = required_dx_test_output_dir()?;
  let source_iterations = read_iteration_env("DX_TAURI_CARGO_METADATA_SOURCE_ITERATIONS", 8);
  let machine_iterations = read_iteration_env("DX_TAURI_CARGO_METADATA_MACHINE_ITERATIONS", 50);
  let source_warmups = 2;
  let machine_warmups = 10;

  let setup_started = std::time::Instant::now();
  let setup_metadata = read_cargo_metadata_source_for_benchmark(tauri_dir)?;
  let expected_signature = CargoMetadataBenchmarkSignature::from(&setup_metadata);
  let machine_path =
    cargo_metadata_machine_cache::write(tauri_dir, &setup_metadata).map_err(|error| {
      Error::GenericError(format!(
        "failed to write cargo metadata machine cache: {error}"
      ))
    })?;
  let machine_generation_setup_ns = elapsed_ns(setup_started);
  let machine_before = read_file_bytes_for_receipt(&machine_path)?;

  {
    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
    for _ in 0..source_warmups {
      let metadata = read_cargo_metadata_source_for_benchmark(tauri_dir)?;
      assert_eq!(
        CargoMetadataBenchmarkSignature::from(&metadata),
        expected_signature
      );
    }
  }

  let source_samples = {
    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
    measure_cargo_metadata_samples(source_iterations, &expected_signature, || {
      read_cargo_metadata_source_for_benchmark(tauri_dir)
    })?
  };

  {
    let _env = EnvGuard::set_many(&[
      (TAURI_DX_MACHINE_CACHE_ENV, Some("1")),
      (
        tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
        Some("0"),
      ),
    ]);
    for _ in 0..machine_warmups {
      let metadata = get_cargo_metadata(tauri_dir)?;
      assert_eq!(
        CargoMetadataBenchmarkSignature::from(&metadata),
        expected_signature
      );
    }
  }

  let machine_samples = {
    let _env = EnvGuard::set_many(&[
      (TAURI_DX_MACHINE_CACHE_ENV, Some("1")),
      (
        tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
        Some("0"),
      ),
    ]);
    measure_cargo_metadata_samples(machine_iterations, &expected_signature, || {
      get_cargo_metadata(tauri_dir)
    })?
  };

  let machine_after = read_file_bytes_for_receipt(&machine_path)?;
  let source_summary = timing_summary(&source_samples);
  let machine_summary = timing_summary(&machine_samples);
  let source_median = source_summary["median_ns"].as_u64().unwrap_or(0);
  let machine_median = machine_summary["median_ns"].as_u64().unwrap_or(0);
  let ratio_percent = ratio_percent_value(machine_median, source_median);
  let speedup_x = ratio_value(source_median, machine_median);
  let notes = serde_json::Value::Array(vec![
    serde_json::Value::String(
      "The .machine file is written as setup before warmups; setup generation time is recorded but excluded from timing.".into(),
    ),
    serde_json::Value::String(
      "The measured source baseline is the official Tauri-style cargo metadata path: spawn cargo, read stdout, and parse JSON into CargoMetadata.".into(),
    ),
    serde_json::Value::String(
      "The measured machine path uses get_cargo_metadata with TAURI_DX_MACHINE_CACHE=1 and TAURI_DX_MACHINE_CACHE_WRITE=0, so timed samples may read but must not write the sidecar.".into(),
    ),
    serde_json::Value::String(
      "This receipt does not measure a full Tauri CLI command, app runtime, WebView startup, bundling, dev, watch, install, or an official prebuilt Tauri binary.".into(),
    ),
  ]);
  let mut receipt = serde_json::Map::new();
  {
    let mut insert = |key: &str, value: serde_json::Value| {
      receipt.insert(key.to_string(), value);
    };
    insert(
      "schema",
      serde_json::json!("dx.tauri.cli.cargo_metadata_source_vs_machine_receipt"),
    );
    insert("schema_version", serde_json::json!(1));
    insert("created_unix_ms", serde_json::json!(current_unix_ms()));
    insert("fixture", serde_json::json!("current-fork-tauri-cli-crate"));
    insert(
      "tauri_dir",
      serde_json::json!(tauri_dir.display().to_string()),
    );
    insert(
      "cache_boundary",
      serde_json::json!("cargo metadata --no-deps"),
    );
    insert(
      "baseline",
      serde_json::json!("cargo metadata spawn plus serde_json parse"),
    );
    insert(
      "machine",
      serde_json::json!("pre-generated dx-serializer .machine read"),
    );
    insert(
      "command",
      serde_json::json!(format!("$env:DX_TEST_OUTPUT_DIR='{}'; cargo test -p tauri-cli --features dx-machine-cache-mmap --lib {test_filter} -j1 --color never -- --ignored --test-threads=1", output_dir.display())),
    );
    insert(
      "cargo_metadata_command",
      serde_json::json!("cargo metadata --no-deps --format-version 1"),
    );
    insert(
      "machine_path",
      serde_json::json!(machine_path.display().to_string()),
    );
    insert("machine_bytes", serde_json::json!(machine_before.len()));
    insert(
      "machine_generation_setup_ns",
      serde_json::json!(machine_generation_setup_ns),
    );
    insert("source_warmups", serde_json::json!(source_warmups));
    insert("machine_warmups", serde_json::json!(machine_warmups));
    insert("source_iterations", serde_json::json!(source_iterations));
    insert("machine_iterations", serde_json::json!(machine_iterations));
    insert("metadata_signature", expected_signature.to_json());
    insert("source_parse", source_summary);
    insert("validated_machine_read", machine_summary);
    insert("machine_to_source_median_ratio_percent", ratio_percent);
    insert("source_to_machine_median_speedup_x", speedup_x);
    insert("machine_cache_enabled_for_timing", serde_json::json!(true));
    insert(
      "machine_cache_warmed_before_timing",
      serde_json::json!(true),
    );
    insert(
      "machine_cache_generation_measured",
      serde_json::json!(false),
    );
    insert(
      "machine_cache_generation_manual_setup",
      serde_json::json!(true),
    );
    insert("cache_write_included_in_timing", serde_json::json!(false));
    insert("machine_cache_write_env_for_timing", serde_json::json!("0"));
    insert(
      "machine_file_unchanged_during_timing",
      serde_json::json!(machine_before == machine_after),
    );
    insert("fallback_used", serde_json::json!(false));
    insert(
      "official_style_cargo_metadata_baseline_measured",
      serde_json::json!(true),
    );
    insert("official_tauri_binary_measured", serde_json::json!(false));
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

  std::fs::create_dir_all(&output_dir).fs_context(
    "failed to create cargo metadata receipt output dir",
    &output_dir,
  )?;
  let receipt_path = output_dir.join(receipt_file_name);
  std::fs::write(
    &receipt_path,
    serde_json::to_vec_pretty(&receipt).expect("serialize cargo metadata timing receipt"),
  )
  .fs_context(
    "failed to write cargo metadata timing receipt",
    &receipt_path,
  )?;
  Ok(receipt_path)
}

fn write_watch_folders_source_vs_machine_receipt(
  tauri_dir: &Path,
  receipt_file_name: &str,
  test_filter: &str,
) -> crate::Result<PathBuf> {
  let output_dir = required_dx_test_output_dir()?;
  let source_iterations = read_iteration_env("DX_TAURI_WATCH_FOLDERS_SOURCE_ITERATIONS", 8);
  let machine_iterations = read_iteration_env("DX_TAURI_WATCH_FOLDERS_MACHINE_ITERATIONS", 50);
  let source_warmups = 2;
  let machine_warmups = 10;

  let setup_started = std::time::Instant::now();
  let setup_metadata = read_cargo_metadata_source_for_benchmark(tauri_dir)?;
  let metadata_signature = CargoMetadataBenchmarkSignature::from(&setup_metadata);
  let machine_path =
    cargo_metadata_machine_cache::write(tauri_dir, &setup_metadata).map_err(|error| {
      Error::GenericError(format!(
        "failed to write cargo metadata machine cache: {error}"
      ))
    })?;
  let machine_generation_setup_ns = elapsed_ns(setup_started);
  let machine_before = read_file_bytes_for_receipt(&machine_path)?;

  let expected_signature = {
    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
    let folders = get_watch_folders(&[], tauri_dir)?;
    WatchFoldersBenchmarkSignature::from_paths(&folders)
  };

  {
    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
    for _ in 0..source_warmups {
      let folders = get_watch_folders(&[], tauri_dir)?;
      assert_eq!(
        WatchFoldersBenchmarkSignature::from_paths(&folders),
        expected_signature
      );
    }
  }

  let source_samples = {
    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
    measure_watch_folders_samples(source_iterations, &expected_signature, || {
      get_watch_folders(&[], tauri_dir)
    })?
  };

  {
    let _env = EnvGuard::set_many(&[
      (TAURI_DX_MACHINE_CACHE_ENV, Some("1")),
      (
        tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
        Some("0"),
      ),
    ]);
    for _ in 0..machine_warmups {
      let folders = get_watch_folders(&[], tauri_dir)?;
      assert_eq!(
        WatchFoldersBenchmarkSignature::from_paths(&folders),
        expected_signature
      );
    }
  }

  let machine_samples = {
    let _env = EnvGuard::set_many(&[
      (TAURI_DX_MACHINE_CACHE_ENV, Some("1")),
      (
        tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
        Some("0"),
      ),
    ]);
    measure_watch_folders_samples(machine_iterations, &expected_signature, || {
      get_watch_folders(&[], tauri_dir)
    })?
  };

  let machine_after = read_file_bytes_for_receipt(&machine_path)?;
  let source_summary = timing_summary(&source_samples);
  let machine_summary = timing_summary(&machine_samples);
  let source_median = source_summary["median_ns"].as_u64().unwrap_or(0);
  let machine_median = machine_summary["median_ns"].as_u64().unwrap_or(0);
  let ratio_percent = ratio_percent_value(machine_median, source_median);
  let speedup_x = ratio_value(source_median, machine_median);
  let notes = serde_json::Value::Array(vec![
    serde_json::Value::String(
      "The .machine file is written as setup before warmups; setup generation time is recorded but excluded from timing.".into(),
    ),
    serde_json::Value::String(
      "The measured source baseline is get_watch_folders with TAURI_DX_MACHINE_CACHE unset, so workspace dependency discovery spawns cargo metadata and parses JSON.".into(),
    ),
    serde_json::Value::String(
      "The measured machine path is get_watch_folders with TAURI_DX_MACHINE_CACHE=1 and TAURI_DX_MACHINE_CACHE_WRITE=0, so timed samples may read but must not write the sidecar.".into(),
    ),
    serde_json::Value::String(
      "This is a dev/watch folder-discovery boundary, not a full tauri dev run, app runtime, WebView startup, bundling, or official prebuilt Tauri binary benchmark.".into(),
    ),
  ]);
  let mut receipt = serde_json::Map::new();
  {
    let mut insert = |key: &str, value: serde_json::Value| {
      receipt.insert(key.to_string(), value);
    };
    insert(
      "schema",
      serde_json::json!("dx.tauri.cli.watch_folders_source_vs_machine_receipt"),
    );
    insert("schema_version", serde_json::json!(1));
    insert("created_unix_ms", serde_json::json!(current_unix_ms()));
    insert("fixture", serde_json::json!("current-fork-tauri-cli-crate"));
    insert(
      "tauri_dir",
      serde_json::json!(tauri_dir.display().to_string()),
    );
    insert("cache_boundary", serde_json::json!("get_watch_folders"));
    insert(
      "accelerated_dependency",
      serde_json::json!(
        "get_in_workspace_dependency_paths -> cargo_metadata_machine_cache::read_projection"
      ),
    );
    insert(
      "baseline",
      serde_json::json!("watch folder discovery with cargo metadata spawn plus serde_json parse"),
    );
    insert(
      "machine",
      serde_json::json!("watch folder discovery with pre-generated dx-serializer .machine read"),
    );
    insert(
      "command",
      serde_json::json!(format!("$env:DX_TEST_OUTPUT_DIR='{}'; cargo test -p tauri-cli --features dx-machine-cache-mmap --lib {test_filter} -j1 --color never -- --ignored --test-threads=1", output_dir.display())),
    );
    insert(
      "cargo_metadata_command",
      serde_json::json!("cargo metadata --no-deps --format-version 1"),
    );
    insert(
      "machine_path",
      serde_json::json!(machine_path.display().to_string()),
    );
    insert("machine_bytes", serde_json::json!(machine_before.len()));
    insert(
      "machine_generation_setup_ns",
      serde_json::json!(machine_generation_setup_ns),
    );
    insert("source_warmups", serde_json::json!(source_warmups));
    insert("machine_warmups", serde_json::json!(machine_warmups));
    insert("source_iterations", serde_json::json!(source_iterations));
    insert("machine_iterations", serde_json::json!(machine_iterations));
    insert("metadata_signature", metadata_signature.to_json());
    insert("watch_folders_signature", expected_signature.to_json());
    insert("source_watch_folders", source_summary);
    insert("machine_watch_folders", machine_summary);
    insert("machine_to_source_median_ratio_percent", ratio_percent);
    insert("source_to_machine_median_speedup_x", speedup_x);
    insert("machine_cache_enabled_for_timing", serde_json::json!(true));
    insert(
      "machine_cache_warmed_before_timing",
      serde_json::json!(true),
    );
    insert(
      "machine_cache_generation_measured",
      serde_json::json!(false),
    );
    insert(
      "machine_cache_generation_manual_setup",
      serde_json::json!(true),
    );
    insert("cache_write_included_in_timing", serde_json::json!(false));
    insert("machine_cache_write_env_for_timing", serde_json::json!("0"));
    insert(
      "machine_file_unchanged_during_timing",
      serde_json::json!(machine_before == machine_after),
    );
    insert("fallback_used", serde_json::json!(false));
    insert("dev_watch_boundary_measured", serde_json::json!(true));
    insert("full_tauri_dev_command_measured", serde_json::json!(false));
    insert("official_tauri_binary_measured", serde_json::json!(false));
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

  std::fs::create_dir_all(&output_dir).fs_context(
    "failed to create watch folders receipt output dir",
    &output_dir,
  )?;
  let receipt_path = output_dir.join(receipt_file_name);
  std::fs::write(
    &receipt_path,
    serde_json::to_vec_pretty(&receipt).expect("serialize watch folders timing receipt"),
  )
  .fs_context(
    "failed to write watch folders timing receipt",
    &receipt_path,
  )?;
  Ok(receipt_path)
}

fn read_cargo_metadata_source_for_benchmark(tauri_dir: &Path) -> crate::Result<CargoMetadata> {
  let output = Command::new("cargo")
    .args(["metadata", "--no-deps", "--format-version", "1"])
    .current_dir(tauri_dir)
    .output()
    .map_err(|error| Error::CommandFailed {
      command: "cargo metadata --no-deps --format-version 1".to_string(),
      error,
    })?;

  if !output.status.success() {
    return Err(Error::CommandFailed {
      command: "cargo metadata".to_string(),
      error: std::io::Error::other(String::from_utf8_lossy(&output.stderr)),
    });
  }

  serde_json::from_slice(&output.stdout).context("failed to parse cargo metadata")
}

fn measure_cargo_metadata_samples<F>(
  iterations: usize,
  expected_signature: &CargoMetadataBenchmarkSignature,
  mut op: F,
) -> crate::Result<Vec<u64>>
where
  F: FnMut() -> crate::Result<CargoMetadata>,
{
  let mut samples = Vec::with_capacity(iterations);
  for _ in 0..iterations {
    let started = std::time::Instant::now();
    let metadata = op()?;
    let elapsed = elapsed_ns(started);
    assert_eq!(
      &CargoMetadataBenchmarkSignature::from(&metadata),
      expected_signature
    );
    samples.push(elapsed);
  }
  Ok(samples)
}

fn measure_watch_folders_samples<F>(
  iterations: usize,
  expected_signature: &WatchFoldersBenchmarkSignature,
  mut op: F,
) -> crate::Result<Vec<u64>>
where
  F: FnMut() -> crate::Result<Vec<PathBuf>>,
{
  let mut samples = Vec::with_capacity(iterations);
  for _ in 0..iterations {
    let started = std::time::Instant::now();
    let folders = op()?;
    let elapsed = elapsed_ns(started);
    assert_eq!(
      &WatchFoldersBenchmarkSignature::from_paths(&folders),
      expected_signature
    );
    samples.push(elapsed);
  }
  Ok(samples)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CargoMetadataBenchmarkSignature {
  target_directory: String,
  workspace_root: String,
  workspace_members: usize,
  packages: usize,
}

impl CargoMetadataBenchmarkSignature {
  fn to_json(&self) -> serde_json::Value {
    serde_json::json!({
      "target_directory": self.target_directory,
      "workspace_root": self.workspace_root,
      "workspace_members": self.workspace_members,
      "packages": self.packages
    })
  }
}

impl From<&CargoMetadata> for CargoMetadataBenchmarkSignature {
  fn from(metadata: &CargoMetadata) -> Self {
    Self {
      target_directory: cargo_metadata_signature_path(&metadata.target_directory),
      workspace_root: cargo_metadata_signature_path(&metadata.workspace_root),
      workspace_members: metadata.workspace_members.len(),
      packages: metadata.packages.len(),
    }
  }
}

fn cargo_metadata_signature_path(path: &Path) -> String {
  path.display().to_string().replace('\\', "/")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WatchFoldersBenchmarkSignature {
  folders: Vec<String>,
}

impl WatchFoldersBenchmarkSignature {
  fn from_paths(paths: &[PathBuf]) -> Self {
    Self {
      folders: paths
        .iter()
        .map(|path| cargo_metadata_signature_path(path))
        .collect(),
    }
  }

  fn to_json(&self) -> serde_json::Value {
    serde_json::json!({
      "folders": self.folders,
      "folder_count": self.folders.len()
    })
  }
}

fn required_dx_test_output_dir() -> crate::Result<PathBuf> {
  let output_dir = std::env::var_os("DX_TEST_OUTPUT_DIR")
    .map(PathBuf::from)
    .ok_or_else(|| {
      Error::GenericError("DX_TEST_OUTPUT_DIR must be set for benchmark receipts".into())
    })?;
  let output_dir = if output_dir.is_absolute() {
    output_dir
  } else {
    std::env::current_dir()
      .context("failed to resolve current directory")?
      .join(output_dir)
  };
  let receipt_root = PathBuf::from(TEST_OUTPUT_ROOT);
  if !output_dir.starts_with(&receipt_root) {
    return Err(Error::GenericError(format!(
      "benchmark receipt output must stay under {}",
      receipt_root.display()
    )));
  }
  Ok(output_dir)
}

fn read_iteration_env(key: &str, default: usize) -> usize {
  std::env::var(key)
    .ok()
    .and_then(|value| value.parse::<usize>().ok())
    .filter(|value| *value > 0)
    .unwrap_or(default)
}

fn read_file_bytes_for_receipt(path: &Path) -> crate::Result<Vec<u8>> {
  std::fs::read(path).fs_context("failed to read benchmark machine artifact", path)
}

fn elapsed_ns(started: std::time::Instant) -> u64 {
  u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

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

fn percentile_index(len: usize, percentile: usize) -> usize {
  (((len * percentile) + 99) / 100)
    .saturating_sub(1)
    .min(len.saturating_sub(1))
}

fn current_unix_ms() -> u128 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .expect("system clock before unix epoch")
    .as_millis()
}

fn ratio_percent_value(numerator: u64, denominator: u64) -> serde_json::Value {
  if denominator == 0 {
    serde_json::Value::Null
  } else {
    serde_json::json!((numerator as f64 * 100.0) / denominator as f64)
  }
}

fn ratio_value(numerator: u64, denominator: u64) -> serde_json::Value {
  if denominator == 0 {
    serde_json::Value::Null
  } else {
    serde_json::json!(numerator as f64 / denominator as f64)
  }
}
