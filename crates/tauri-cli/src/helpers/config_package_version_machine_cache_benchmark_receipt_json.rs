// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use super::super::fixture::{current_unix_ms, PackageVersionFixture, ReceiptResult};
use super::PackageVersionTimingSignature;
use std::path::Path;

pub(super) struct PackageVersionReceiptInput<'a> {
  pub(super) output_dir: &'a Path,
  pub(super) fixture: &'a PackageVersionFixture,
  pub(super) machine_path: &'a Path,
  pub(super) machine_before: &'a [u8],
  pub(super) machine_after: &'a [u8],
  pub(super) machine_generation_setup_ns: u64,
  pub(super) source_warmups: usize,
  pub(super) machine_warmups: usize,
  pub(super) source_iterations: usize,
  pub(super) machine_iterations: usize,
  pub(super) source_package_version_read_hits: usize,
  pub(super) timed_machine_package_version_hit_count: usize,
  pub(super) expected_machine_package_version_hit_count: usize,
  pub(super) timed_machine_package_version_hits_verified: bool,
  pub(super) expected_signature: &'a PackageVersionTimingSignature,
  pub(super) source_summary: serde_json::Value,
  pub(super) machine_summary: serde_json::Value,
  pub(super) ratio_percent: f64,
  pub(super) speedup_x: f64,
  pub(super) project_config_machine_present: bool,
  pub(super) package_version_machine_hit_verified: bool,
  pub(super) benchmark_integrity_verified: bool,
  pub(super) package_version_helper_speed_claim_allowed: bool,
  pub(super) package_version_helper_meets_requested_10x: bool,
  pub(super) allowed_claims: Vec<&'static str>,
  pub(super) test_filter: &'a str,
}

pub(super) fn package_version_receipt(
  input: PackageVersionReceiptInput<'_>,
) -> ReceiptResult<serde_json::Value> {
  let mut receipt = serde_json::Map::new();
  {
    let mut insert = |key: &str, value: serde_json::Value| {
      receipt.insert(key.to_string(), value);
    };
    insert(
      "schema",
      serde_json::json!("dx.tauri.cli.package_version_source_vs_machine_receipt"),
    );
    insert("schema_version", serde_json::json!(1));
    insert("created_unix_ms", serde_json::json!(current_unix_ms()));
    insert(
      "fixture",
      serde_json::json!("representative generated package.json version path"),
    );
    insert(
      "fixture_root",
      serde_json::json!(input.fixture.project_root.display().to_string()),
    );
    insert(
      "config_dir",
      serde_json::json!(input.fixture.config_dir.display().to_string()),
    );
    insert(
      "config_path",
      serde_json::json!(input.fixture.config_path.display().to_string()),
    );
    insert(
      "package_json_path",
      serde_json::json!(input.fixture.package_json_path.display().to_string()),
    );
    insert(
      "cache_boundary",
      serde_json::json!("load_config package.version path"),
    );
    insert(
      "baseline",
      serde_json::json!("tauri.conf.json/package.json source parse"),
    );
    insert(
      "machine",
      serde_json::json!("pre-generated dx-serializer .machine read"),
    );
    insert(
      "command",
      serde_json::json!(format!("$env:DX_TEST_OUTPUT_DIR='{}'; cargo test --manifest-path .\\crates\\tauri-cli\\Cargo.toml --no-default-features --features dx-machine-cache --lib {} -j1 --color never -- --ignored --exact --test-threads=1 --nocapture", input.output_dir.display(), input.test_filter)),
    );
    insert(
      "machine_path",
      serde_json::json!(input.machine_path.display().to_string()),
    );
    insert(
      "project_config_machine_path",
      serde_json::json!(input
        .fixture
        .project_config_machine_path
        .display()
        .to_string()),
    );
    insert(
      "config_bytes",
      serde_json::json!(file_len(&input.fixture.config_path)?),
    );
    insert(
      "package_json_bytes",
      serde_json::json!(file_len(&input.fixture.package_json_path)?),
    );
    insert(
      "machine_bytes",
      serde_json::json!(input.machine_before.len()),
    );
    insert(
      "machine_generation_setup_ns",
      serde_json::json!(input.machine_generation_setup_ns),
    );
    insert("source_warmups", serde_json::json!(input.source_warmups));
    insert("machine_warmups", serde_json::json!(input.machine_warmups));
    insert(
      "source_iterations",
      serde_json::json!(input.source_iterations),
    );
    insert(
      "machine_iterations",
      serde_json::json!(input.machine_iterations),
    );
    insert("metadata_signature", input.expected_signature.to_json());
    insert("source_parse", input.source_summary);
    insert("validated_machine_read", input.machine_summary);
    insert(
      "machine_to_source_median_ratio_percent",
      serde_json::json!(input.ratio_percent),
    );
    insert(
      "source_to_machine_median_speedup_x",
      serde_json::json!(input.speedup_x),
    );
    insert("machine_cache_enabled_for_timing", serde_json::json!(true));
    insert("machine_cache_env_for_timing", serde_json::json!("1"));
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
    insert(
      "source_cache_read_env_for_timing",
      serde_json::json!("removed"),
    );
    insert("source_cache_write_env_for_timing", serde_json::json!("0"));
    insert("machine_cache_write_env_for_timing", serde_json::json!("0"));
    insert(
      "machine_file_unchanged_during_timing",
      serde_json::json!(input.machine_before == input.machine_after),
    );
    insert(
      "receipt_paths_under_test_outputs_verified",
      serde_json::json!(true),
    );
    insert("timed_machine_env_verified", serde_json::json!(true));
    insert(
      "package_version_machine_hit_verified",
      serde_json::json!(input.package_version_machine_hit_verified),
    );
    insert(
      "source_package_version_machine_hit_count",
      serde_json::json!(input.source_package_version_read_hits),
    );
    insert(
      "timed_machine_package_version_hit_count",
      serde_json::json!(input.timed_machine_package_version_hit_count),
    );
    insert(
      "expected_timed_machine_package_version_hit_count",
      serde_json::json!(input.expected_machine_package_version_hit_count),
    );
    insert(
      "timed_machine_package_version_hits_verified",
      serde_json::json!(input.timed_machine_package_version_hits_verified),
    );
    insert(
      "cache_hit_verified_before_timing",
      serde_json::json!(input.package_version_machine_hit_verified),
    );
    insert("output_equivalence_verified", serde_json::json!(true));
    insert(
      "benchmark_integrity_verified",
      serde_json::json!(input.benchmark_integrity_verified),
    );
    insert(
      "project_config_machine_cache_present_during_timing",
      serde_json::json!(input.project_config_machine_present),
    );
    insert("fallback_used", serde_json::json!(false));
    insert("source_config_authoritative", serde_json::json!(true));
    insert("source_package_json_authoritative", serde_json::json!(true));
    insert("machine_read_hashes_package_json", serde_json::json!(true));
    insert("same_process_helper_timing", serde_json::json!(true));
    insert("official_tauri_binary_measured", serde_json::json!(false));
    insert("full_cli_speed_claimed", serde_json::json!(false));
    insert("full_cli_speed_claim_allowed", serde_json::json!(false));
    insert("upstream_baseline_measured", serde_json::json!(false));
    insert("faster_than_upstream_claimed", serde_json::json!(false));
    insert(
      "faster_than_upstream_claim_allowed",
      serde_json::json!(false),
    );
    insert("default_on_readiness_claimed", serde_json::json!(false));
    insert(
      "default_on_readiness_claim_allowed",
      serde_json::json!(false),
    );
    insert("product_level_speed_claimed", serde_json::json!(false));
    insert(
      "product_level_speed_claim_allowed",
      serde_json::json!(false),
    );
    insert(
      "package_version_helper_speed_claim_allowed",
      serde_json::json!(input.package_version_helper_speed_claim_allowed),
    );
    insert(
      "package_version_helper_speed_claim_metric",
      serde_json::json!("median"),
    );
    insert(
      "package_version_helper_speed_claim_threshold_x",
      serde_json::json!(1.0),
    );
    insert(
      "package_version_helper_meets_requested_10x",
      serde_json::json!(input.package_version_helper_meets_requested_10x),
    );
    insert(
      "package_version_helper_10x_claim_allowed",
      serde_json::json!(false),
    );
    insert("release_readiness_claimed", serde_json::json!(false));
    insert("release_build_run", serde_json::json!(false));
    insert("app_runtime_measured", serde_json::json!(false));
    insert("webview_startup_measured", serde_json::json!(false));
    insert("ipc_measured", serde_json::json!(false));
    insert("dev_workflow_measured", serde_json::json!(false));
    insert("build_workflow_measured", serde_json::json!(false));
    insert("watch_workflow_measured", serde_json::json!(false));
    insert("bundle_or_installer_measured", serde_json::json!(false));
    insert(
      "app_runtime_webview_ipc_build_bundle_claim_allowed",
      serde_json::json!(false),
    );
    insert("allowed_claims", serde_json::json!(input.allowed_claims));
    insert(
      "blocked_claims",
      serde_json::json!([
        "upstream_superiority",
        "default_on",
        "full_cli",
        "product_level",
        "app_runtime",
        "webview_startup",
        "ipc",
        "build",
        "dev",
        "watch",
        "bundle_or_installer"
      ]),
    );
    insert(
      "notes",
      serde_json::json!([
        "Measures current-fork same-process load_config on a generated package.json version-path fixture.",
        "The broader project-config .machine sidecar is kept absent during timing so this isolates the package-version.machine path.",
        "The package-version .machine file is generated during setup before timing; cache writes are disabled during timed samples.",
        "This receipt is not an official binary, upstream-source, full CLI, app runtime, build, bundle, or product-level comparison."
      ]),
    );
  }
  Ok(serde_json::Value::Object(receipt))
}

fn file_len(path: &Path) -> ReceiptResult<u64> {
  std::fs::metadata(path)
    .map(|metadata| metadata.len())
    .map_err(|error| format!("failed to stat {}: {error}", path.display()))
}
