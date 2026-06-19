// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use super::*;
use std::{
  collections::BTreeMap,
  fs,
  path::{Path, PathBuf},
  sync::{Mutex, MutexGuard, OnceLock},
};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

const TEST_OUTPUT_ROOT: &str = "G:\\Dx\\test-outputs";

type ReceiptResult<T> = Result<T, String>;

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
#[ignore = "writes a local timing receipt under DX_TEST_OUTPUT_DIR for manual inspection"]
fn dx_cargo_package_metadata_machine_cache_writes_source_vs_machine_receipt() {
  let tauri_dir = cargo_package_metadata_benchmark_tauri_dir();
  let receipt_path = write_cargo_package_metadata_source_vs_machine_receipt(
    &tauri_dir,
    "cargo-package-metadata-source-vs-machine-receipt.json",
    "helpers::cargo_manifest::cargo_manifest_machine_cache_benchmark_tests::dx_cargo_package_metadata_machine_cache_writes_source_vs_machine_receipt",
  )
  .expect("write cargo package metadata timing receipt");
  let receipt: serde_json::Value =
    serde_json::from_slice(&std::fs::read(&receipt_path).expect("read timing receipt"))
      .expect("parse timing receipt");

  assert_eq!(
    receipt["schema"],
    "dx.tauri.cli.cargo_package_metadata_source_vs_machine_receipt"
  );
  assert_eq!(receipt["cache_boundary"], "cargo_manifest_and_lock");
  assert_eq!(receipt["machine_cache_generation_measured"], false);
  assert_eq!(receipt["cache_write_included_in_timing"], false);
  assert_eq!(receipt["machine_cache_write_env_for_timing"], "0");
  assert_eq!(receipt["machine_file_unchanged_during_timing"], true);
  assert_eq!(receipt["full_cli_speed_claimed"], false);
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

fn cargo_package_metadata_benchmark_tauri_dir() -> PathBuf {
  std::env::var_os("DX_TAURI_CARGO_PACKAGE_METADATA_BENCH_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}

fn write_cargo_package_metadata_source_vs_machine_receipt(
  tauri_dir: &Path,
  receipt_file_name: &str,
  test_filter: &str,
) -> ReceiptResult<PathBuf> {
  let output_dir = required_dx_test_output_dir()?;
  let source_iterations = read_iteration_env("DX_TAURI_CARGO_PACKAGE_SOURCE_ITERATIONS", 12);
  let machine_iterations = read_iteration_env("DX_TAURI_CARGO_PACKAGE_MACHINE_ITERATIONS", 80);
  let source_warmups = 2;
  let machine_warmups = 10;

  let setup_started = std::time::Instant::now();
  let workspace_dir = crate::interface::rust::get_workspace_dir(tauri_dir)
    .map_err(|error| format!("failed to resolve benchmark workspace directory: {error}"))?;
  let setup_parsed = read_cargo_package_metadata_source_for_benchmark(tauri_dir, &workspace_dir)?;
  let expected_signature = CargoPackageMetadataBenchmarkSignature::from(&setup_parsed);
  let machine_path =
    write_cargo_metadata_machine_cache(tauri_dir, &setup_parsed.0, &setup_parsed.1)
      .map_err(|error| format!("failed to write cargo package metadata machine cache: {error}"))?;
  let machine_generation_setup_ns = elapsed_ns(setup_started);
  let machine_before = read_file_bytes_for_receipt(&machine_path)?;
  if read_cargo_metadata_machine_cache(tauri_dir).is_none() {
    return Err("pre-generated cargo package metadata machine cache did not validate".into());
  }

  {
    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
    for _ in 0..source_warmups {
      let parsed = read_cargo_package_metadata_source_for_benchmark(tauri_dir, &workspace_dir)
        .expect("read source package metadata warmup");
      assert_eq!(
        CargoPackageMetadataBenchmarkSignature::from(&parsed),
        expected_signature
      );
    }
  }

  let source_samples = {
    let _env = EnvGuard::remove(TAURI_DX_MACHINE_CACHE_ENV);
    measure_cargo_package_metadata_samples(source_iterations, &expected_signature, || {
      read_cargo_package_metadata_source_for_benchmark(tauri_dir, &workspace_dir)
    })?
  };

  let machine_env = [
    (TAURI_DX_MACHINE_CACHE_ENV, Some("1")),
    (
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
      Some("0"),
    ),
  ];
  {
    let _env = EnvGuard::set_many(&machine_env);
    for _ in 0..machine_warmups {
      let parsed = cargo_manifest_and_lock(tauri_dir);
      assert_eq!(
        CargoPackageMetadataBenchmarkSignature::from(&parsed),
        expected_signature
      );
    }
  }

  let machine_samples = {
    let _env = EnvGuard::set_many(&machine_env);
    measure_cargo_package_metadata_samples(machine_iterations, &expected_signature, || {
      Ok(cargo_manifest_and_lock(tauri_dir))
    })?
  };

  let machine_after = read_file_bytes_for_receipt(&machine_path)?;
  let source_summary = timing_summary(&source_samples);
  let machine_summary = timing_summary(&machine_samples);
  let source_median = median_ns(&source_samples);
  let machine_median = median_ns(&machine_samples);
  let ratio_percent = (machine_median as f64 / source_median as f64) * 100.0;
  let speedup_x = source_median as f64 / machine_median as f64;

  let receipt = serde_json::json!({
    "schema": "dx.tauri.cli.cargo_package_metadata_source_vs_machine_receipt",
    "schema_version": 1,
    "created_unix_ms": current_unix_ms(),
    "fixture": "current-fork-tauri-cli-crate",
    "tauri_dir": tauri_dir.display().to_string(),
    "cache_boundary": "cargo_manifest_and_lock",
    "baseline": "Cargo.toml/Cargo.lock source parse",
    "machine": "pre-generated dx-serializer .machine read",
    "command": format!("$env:DX_TEST_OUTPUT_DIR='{}'; cargo test --manifest-path .\\crates\\tauri-cli\\Cargo.toml --no-default-features --features dx-machine-cache --lib {test_filter} -j1 --color never -- --ignored --exact --test-threads=1 --nocapture", output_dir.display()),
    "machine_path": machine_path.display().to_string(),
    "machine_bytes": machine_before.len(),
    "machine_generation_setup_ns": machine_generation_setup_ns,
    "source_warmups": source_warmups,
    "machine_warmups": machine_warmups,
    "source_iterations": source_iterations,
    "machine_iterations": machine_iterations,
    "metadata_signature": expected_signature.to_json(),
    "source_parse": source_summary,
    "validated_machine_read": machine_summary,
    "machine_to_source_median_ratio_percent": ratio_percent,
    "source_to_machine_median_speedup_x": speedup_x,
    "machine_cache_enabled_for_timing": true,
    "machine_cache_warmed_before_timing": true,
    "machine_cache_generation_measured": false,
    "machine_cache_generation_manual_setup": true,
    "cache_write_included_in_timing": false,
    "machine_cache_write_env_for_timing": "0",
    "machine_file_unchanged_during_timing": machine_before == machine_after,
    "cache_hit_verified_before_timing": true,
    "fallback_used": false,
    "official_tauri_binary_measured": false,
    "full_cli_speed_claimed": false,
    "upstream_baseline_measured": false,
    "faster_than_upstream_claimed": false,
    "release_build_run": false,
    "app_runtime_measured": false,
    "webview_startup_measured": false,
    "bundle_or_installer_measured": false,
    "notes": [
      "Measures current-fork same-process Cargo.toml/Cargo.lock source parsing with workspace directory resolved during setup against a pre-generated dx-serializer .machine read.",
      "The .machine file is generated during setup before timing; cache writes are disabled during timing.",
      "This receipt is not an official binary, upstream-source, full CLI, app runtime, build, bundle, or product-level comparison."
    ]
  });

  std::fs::create_dir_all(&output_dir).map_err(|error| {
    format!(
      "failed to create cargo package metadata receipt output dir {}: {error}",
      output_dir.display()
    )
  })?;
  let receipt_path = output_dir.join(receipt_file_name);
  std::fs::write(
    &receipt_path,
    serde_json::to_vec_pretty(&receipt).expect("serialize cargo package metadata timing receipt"),
  )
  .map_err(|error| {
    format!(
      "failed to write cargo package metadata timing receipt {}: {error}",
      receipt_path.display()
    )
  })?;
  Ok(receipt_path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CargoPackageMetadataBenchmarkSignature {
  manifest_package_version: Option<String>,
  manifest_dependencies: Vec<(String, String)>,
  lock_packages: Vec<(String, String, Option<String>)>,
}

impl CargoPackageMetadataBenchmarkSignature {
  fn from(parsed: &(Option<CargoManifest>, Option<CargoLock>)) -> Self {
    let manifest_package_version = parsed
      .0
      .as_ref()
      .map(|manifest| manifest.package.version.clone());
    let manifest_dependencies = parsed
      .0
      .as_ref()
      .map(|manifest| {
        let dependencies = manifest
          .dependencies
          .iter()
          .map(|(name, dependency)| (name.clone(), dependency_signature(dependency)))
          .collect::<BTreeMap<_, _>>();
        dependencies.into_iter().collect::<Vec<_>>()
      })
      .unwrap_or_default();
    let lock_packages = parsed
      .1
      .as_ref()
      .map(|lock| {
        lock
          .package
          .iter()
          .map(|package| {
            (
              package.name.clone(),
              package.version.clone(),
              package.source.clone(),
            )
          })
          .collect::<Vec<_>>()
      })
      .unwrap_or_default();
    Self {
      manifest_package_version,
      manifest_dependencies,
      lock_packages,
    }
  }

  fn to_json(&self) -> serde_json::Value {
    serde_json::json!({
      "manifest_package_version": self.manifest_package_version,
      "manifest_dependency_count": self.manifest_dependencies.len(),
      "manifest_dependencies": self.manifest_dependencies,
      "lock_package_count": self.lock_packages.len(),
      "lock_packages": self.lock_packages
    })
  }
}

fn dependency_signature(dependency: &CargoManifestDependency) -> String {
  match dependency {
    CargoManifestDependency::Version(version) => format!("version:{version}"),
    CargoManifestDependency::Package(package) => format!(
      "package:version={};git={};branch={};rev={};path={}",
      package.version.as_deref().unwrap_or_default(),
      package.git.as_deref().unwrap_or_default(),
      package.branch.as_deref().unwrap_or_default(),
      package.rev.as_deref().unwrap_or_default(),
      package
        .path
        .as_deref()
        .map(cargo_package_metadata_signature_path)
        .unwrap_or_default()
    ),
  }
}

fn cargo_package_metadata_signature_path(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

fn read_cargo_package_metadata_source_for_benchmark(
  tauri_dir: &Path,
  workspace_dir: &Path,
) -> ReceiptResult<(Option<CargoManifest>, Option<CargoLock>)> {
  let manifest = fs::read_to_string(tauri_dir.join("Cargo.toml"))
    .ok()
    .and_then(|manifest_contents| toml::from_str(&manifest_contents).ok());
  let lock_path = workspace_dir.join("Cargo.lock");
  let lock = if lock_path.exists() {
    Some(
      fs::read_to_string(&lock_path)
        .map_err(|error| format!("failed to read {}: {error}", lock_path.display()))
        .and_then(|contents| {
          toml::from_str(&contents)
            .map_err(|error| format!("failed to parse {}: {error}", lock_path.display()))
        })?,
    )
  } else {
    None
  };
  Ok((manifest, lock))
}

fn measure_cargo_package_metadata_samples<F>(
  iterations: usize,
  expected_signature: &CargoPackageMetadataBenchmarkSignature,
  mut read: F,
) -> ReceiptResult<Vec<u64>>
where
  F: FnMut() -> ReceiptResult<(Option<CargoManifest>, Option<CargoLock>)>,
{
  let mut samples = Vec::with_capacity(iterations);
  for _ in 0..iterations {
    let started = std::time::Instant::now();
    let parsed = read()?;
    let elapsed = elapsed_ns(started);
    let signature = CargoPackageMetadataBenchmarkSignature::from(&parsed);
    if &signature != expected_signature {
      return Err("cargo package metadata timing sample changed parsed output".into());
    }
    samples.push(elapsed);
  }
  Ok(samples)
}

fn required_dx_test_output_dir() -> ReceiptResult<PathBuf> {
  let output_dir = std::env::var_os("DX_TEST_OUTPUT_DIR")
    .map(PathBuf::from)
    .ok_or_else(|| "DX_TEST_OUTPUT_DIR must be set for benchmark receipts".to_string())?;
  let output_dir = if output_dir.is_absolute() {
    output_dir
  } else {
    std::env::current_dir()
      .map_err(|error| format!("failed to resolve current directory: {error}"))?
      .join(output_dir)
  };
  let receipt_root = PathBuf::from(TEST_OUTPUT_ROOT);
  if !output_dir.starts_with(&receipt_root) {
    return Err(format!(
      "benchmark receipt output must stay under {}",
      receipt_root.display()
    ));
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

fn read_file_bytes_for_receipt(path: &Path) -> ReceiptResult<Vec<u8>> {
  std::fs::read(path).map_err(|error| {
    format!(
      "failed to read benchmark machine artifact {}: {error}",
      path.display()
    )
  })
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

fn median_ns(samples: &[u64]) -> u64 {
  let mut sorted = samples.to_vec();
  sorted.sort_unstable();
  sorted[sorted.len() / 2]
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
