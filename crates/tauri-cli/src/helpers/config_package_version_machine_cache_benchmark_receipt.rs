// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use super::super::{
  load_config,
  package_version_machine_cache::{
    apply_cached_package_version, package_version_cache_candidate,
    package_version_machine_cache_read_hits, package_version_machine_path,
    reset_package_version_machine_cache_read_hits, write_package_version_machine_cache,
    PackageVersionMachineCandidate, TAURI_DX_MACHINE_CACHE_ENV,
  },
  Target,
};
use super::fixture::{PackageVersionFixture, ReceiptResult, PACKAGE_VERSION};
#[path = "config_package_version_machine_cache_benchmark_receipt_json.rs"]
mod receipt_json;
use receipt_json::{package_version_receipt, PackageVersionReceiptInput};
use std::{
  fs,
  path::{Path, PathBuf},
  sync::{Mutex, MutexGuard, OnceLock},
};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

const TEST_OUTPUT_ROOT: &str = "G:\\Dx\\test-outputs";

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

pub(super) fn write_package_version_source_vs_machine_receipt(
  receipt_file_name: &str,
  test_filter: &str,
) -> ReceiptResult<PathBuf> {
  let output_dir = required_dx_test_output_dir()?;
  let source_iterations = read_iteration_env("DX_TAURI_PACKAGE_VERSION_SOURCE_ITERATIONS", 30);
  let machine_iterations = read_iteration_env("DX_TAURI_PACKAGE_VERSION_MACHINE_ITERATIONS", 120);
  let source_warmups = 5;
  let machine_warmups = 20;

  let fixture = PackageVersionFixture::create(&output_dir)?;
  let setup_started = std::time::Instant::now();
  let config_value = read_config_json(&fixture.config_path)?;
  let candidate =
    package_version_cache_candidate(&config_value, &fixture.config_dir).ok_or_else(|| {
      "package-version benchmark fixture did not produce a cache candidate".to_string()
    })?;
  let machine_path = write_package_version_machine_cache(&candidate, PACKAGE_VERSION)
    .map_err(|error| format!("failed to write package-version machine cache: {error}"))?;
  let machine_generation_setup_ns = elapsed_ns(setup_started);
  if package_version_machine_path(&fixture.config_dir) != machine_path {
    return Err("package-version machine path helper disagreed with write receipt".into());
  }
  let machine_before = read_file_bytes_for_receipt(&machine_path)?;
  fixture.remove_project_config_machine_cache()?;

  let package_version_machine_hit_verified =
    verify_package_version_machine_hit(config_value, &candidate)?;
  let expected_signature = PackageVersionTimingSignature {
    version: PACKAGE_VERSION.to_string(),
  };

  reset_package_version_machine_cache_read_hits();
  measure_source_warmups(&fixture, source_warmups, &expected_signature)?;
  let source_samples = measure_source_samples(&fixture, source_iterations, &expected_signature)?;
  let source_package_version_read_hits = package_version_machine_cache_read_hits();
  if source_package_version_read_hits != 0 {
    return Err(format!(
      "source timing unexpectedly used package-version machine cache {source_package_version_read_hits} times"
    ));
  }

  let machine_env = [
    (TAURI_DX_MACHINE_CACHE_ENV, Some("1")),
    (
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
      Some("0"),
    ),
  ];
  fixture.remove_project_config_machine_cache()?;
  reset_package_version_machine_cache_read_hits();
  measure_machine_warmups(&fixture, machine_warmups, &machine_env, &expected_signature)?;
  fixture.remove_project_config_machine_cache()?;
  let machine_samples = measure_machine_samples(
    &fixture,
    machine_iterations,
    &machine_env,
    &expected_signature,
  )?;
  let timed_machine_package_version_hit_count = package_version_machine_cache_read_hits();
  let expected_machine_package_version_hit_count = machine_warmups + machine_iterations;
  let timed_machine_package_version_hits_verified =
    timed_machine_package_version_hit_count == expected_machine_package_version_hit_count;

  let machine_after = read_file_bytes_for_receipt(&machine_path)?;
  let source_summary = timing_summary(&source_samples);
  let machine_summary = timing_summary(&machine_samples);
  let source_median = median_ns(&source_samples);
  let machine_median = median_ns(&machine_samples);
  let ratio_percent = (machine_median as f64 / source_median as f64) * 100.0;
  let speedup_x = source_median as f64 / machine_median as f64;
  let project_config_machine_present = fixture.project_config_machine_path.exists();
  let benchmark_integrity_verified = machine_before == machine_after
    && package_version_machine_hit_verified
    && !project_config_machine_present
    && timed_machine_package_version_hits_verified;
  let package_version_helper_speed_claim_allowed =
    benchmark_integrity_verified && machine_median < source_median;
  let package_version_helper_meets_requested_10x = speedup_x >= 10.0;
  let allowed_claims = if package_version_helper_speed_claim_allowed {
    vec!["package_version_helper_machine_read_faster_than_source_parse"]
  } else {
    Vec::new()
  };

  let receipt = package_version_receipt(PackageVersionReceiptInput {
    output_dir: &output_dir,
    fixture: &fixture,
    machine_path: &machine_path,
    machine_before: &machine_before,
    machine_after: &machine_after,
    machine_generation_setup_ns,
    source_warmups,
    machine_warmups,
    source_iterations,
    machine_iterations,
    source_package_version_read_hits,
    timed_machine_package_version_hit_count,
    expected_machine_package_version_hit_count,
    timed_machine_package_version_hits_verified,
    expected_signature: &expected_signature,
    source_summary,
    machine_summary,
    ratio_percent,
    speedup_x,
    project_config_machine_present,
    package_version_machine_hit_verified,
    benchmark_integrity_verified,
    package_version_helper_speed_claim_allowed,
    package_version_helper_meets_requested_10x,
    allowed_claims,
    test_filter,
  })?;

  let receipt_path = output_dir.join(receipt_file_name);
  fs::write(
    &receipt_path,
    serde_json::to_vec_pretty(&receipt).expect("serialize package-version timing receipt"),
  )
  .map_err(|error| {
    format!(
      "failed to write package-version timing receipt {}: {error}",
      receipt_path.display()
    )
  })?;
  Ok(receipt_path)
}

fn measure_source_warmups(
  fixture: &PackageVersionFixture,
  source_warmups: usize,
  expected_signature: &PackageVersionTimingSignature,
) -> ReceiptResult<()> {
  let _env = EnvGuard::set_many(&[
    (TAURI_DX_MACHINE_CACHE_ENV, None),
    (
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
      Some("0"),
    ),
  ]);
  for _ in 0..source_warmups {
    let signature = load_config_signature(&fixture.config_dir)?;
    assert_eq!(&signature, expected_signature);
  }
  Ok(())
}

fn measure_source_samples(
  fixture: &PackageVersionFixture,
  source_iterations: usize,
  expected_signature: &PackageVersionTimingSignature,
) -> ReceiptResult<Vec<u64>> {
  let _env = EnvGuard::set_many(&[
    (TAURI_DX_MACHINE_CACHE_ENV, None),
    (
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
      Some("0"),
    ),
  ]);
  measure_package_version_samples(source_iterations, expected_signature, || {
    load_config_signature(&fixture.config_dir)
  })
}

fn measure_machine_warmups(
  fixture: &PackageVersionFixture,
  machine_warmups: usize,
  machine_env: &[(&'static str, Option<&str>)],
  expected_signature: &PackageVersionTimingSignature,
) -> ReceiptResult<()> {
  let _env = EnvGuard::set_many(machine_env);
  for _ in 0..machine_warmups {
    let signature = load_config_signature(&fixture.config_dir)?;
    assert_eq!(&signature, expected_signature);
  }
  Ok(())
}

fn measure_machine_samples(
  fixture: &PackageVersionFixture,
  machine_iterations: usize,
  machine_env: &[(&'static str, Option<&str>)],
  expected_signature: &PackageVersionTimingSignature,
) -> ReceiptResult<Vec<u64>> {
  let _env = EnvGuard::set_many(machine_env);
  measure_package_version_samples(machine_iterations, expected_signature, || {
    load_config_signature(&fixture.config_dir)
  })
}

fn read_config_json(config_path: &Path) -> ReceiptResult<serde_json::Value> {
  let source = fs::read_to_string(config_path)
    .map_err(|error| format!("failed to read {}: {error}", config_path.display()))?;
  serde_json::from_str(&source)
    .map_err(|error| format!("failed to parse {}: {error}", config_path.display()))
}

fn verify_package_version_machine_hit(
  mut config_value: serde_json::Value,
  candidate: &PackageVersionMachineCandidate,
) -> ReceiptResult<bool> {
  let _env = EnvGuard::set_many(&[
    (TAURI_DX_MACHINE_CACHE_ENV, Some("1")),
    (
      tauri_utils::config::machine_cache::TAURI_DX_MACHINE_CACHE_WRITE_ENV,
      Some("0"),
    ),
  ]);
  let applied = apply_cached_package_version(&mut config_value, candidate);
  if !applied || config_value["version"] != serde_json::json!(PACKAGE_VERSION) {
    return Err("pre-generated package-version machine cache did not validate".into());
  }
  Ok(true)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageVersionTimingSignature {
  version: String,
}

impl PackageVersionTimingSignature {
  fn to_json(&self) -> serde_json::Value {
    serde_json::json!({
      "version": self.version,
    })
  }
}

fn load_config_signature(config_dir: &Path) -> ReceiptResult<PackageVersionTimingSignature> {
  let config = load_config(&[], false, Target::current(), config_dir).map_err(|error| {
    format!(
      "failed to load config from {}: {error}",
      config_dir.display()
    )
  })?;
  let version = config
    .version
    .as_deref()
    .ok_or_else(|| "loaded config did not resolve a package version".to_string())?;
  Ok(PackageVersionTimingSignature {
    version: version.to_string(),
  })
}

fn measure_package_version_samples<F>(
  iterations: usize,
  expected_signature: &PackageVersionTimingSignature,
  mut read: F,
) -> ReceiptResult<Vec<u64>>
where
  F: FnMut() -> ReceiptResult<PackageVersionTimingSignature>,
{
  let mut samples = Vec::with_capacity(iterations);
  for _ in 0..iterations {
    let started = std::time::Instant::now();
    let signature = read()?;
    let elapsed = elapsed_ns(started);
    if &signature != expected_signature {
      return Err("package-version timing sample changed loaded config output".into());
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
  let receipt_root = PathBuf::from(TEST_OUTPUT_ROOT)
    .canonicalize()
    .map_err(|error| format!("failed to canonicalize {TEST_OUTPUT_ROOT}: {error}"))?;
  fs::create_dir_all(&output_dir).map_err(|error| {
    format!(
      "failed to create package-version receipt output dir {}: {error}",
      output_dir.display()
    )
  })?;
  let output_dir = output_dir.canonicalize().map_err(|error| {
    format!(
      "failed to canonicalize package-version receipt output dir {}: {error}",
      output_dir.display()
    )
  })?;
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
  fs::read(path).map_err(|error| {
    format!(
      "failed to read benchmark machine artifact {}: {error}",
      path.display()
    )
  })
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

fn elapsed_ns(started: std::time::Instant) -> u64 {
  u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}
