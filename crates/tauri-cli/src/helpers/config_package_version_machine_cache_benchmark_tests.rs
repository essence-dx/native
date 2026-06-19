// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::fs;

#[path = "config_package_version_machine_cache_benchmark_fixture.rs"]
mod fixture;
#[path = "config_package_version_machine_cache_benchmark_receipt.rs"]
mod receipt;

#[test]
#[ignore = "writes a local timing receipt under DX_TEST_OUTPUT_DIR for manual inspection"]
fn dx_package_version_machine_cache_writes_source_vs_machine_receipt() {
  let receipt_path = receipt::write_package_version_source_vs_machine_receipt(
    "package-version-source-vs-machine-receipt.json",
    "helpers::config::config_package_version_machine_cache_benchmark_tests::dx_package_version_machine_cache_writes_source_vs_machine_receipt",
  )
  .expect("write package-version timing receipt");
  let receipt: serde_json::Value =
    serde_json::from_slice(&fs::read(&receipt_path).expect("read timing receipt"))
      .expect("parse timing receipt");

  assert_eq!(
    receipt["schema"],
    "dx.tauri.cli.package_version_source_vs_machine_receipt"
  );
  assert_eq!(
    receipt["cache_boundary"],
    "load_config package.version path"
  );
  assert_eq!(receipt["machine_cache_generation_measured"], false);
  assert_eq!(receipt["cache_write_included_in_timing"], false);
  assert_eq!(receipt["machine_cache_write_env_for_timing"], "0");
  assert_eq!(receipt["machine_file_unchanged_during_timing"], true);
  assert_eq!(
    receipt["project_config_machine_cache_present_during_timing"],
    false
  );
  assert_eq!(receipt["package_version_machine_hit_verified"], true);
  assert_eq!(receipt["timed_machine_package_version_hits_verified"], true);
  assert_eq!(receipt["package_version_helper_speed_claim_allowed"], true);
  assert_eq!(
    receipt["package_version_helper_speed_claim_metric"],
    "median"
  );
  assert_eq!(receipt["package_version_helper_meets_requested_10x"], false);
  assert_eq!(
    receipt["allowed_claims"],
    serde_json::json!(["package_version_helper_machine_read_faster_than_source_parse"])
  );
  assert_eq!(receipt["full_cli_speed_claimed"], false);
  assert_eq!(receipt["full_cli_speed_claim_allowed"], false);
  assert_eq!(receipt["faster_than_upstream_claimed"], false);
  assert_eq!(receipt["faster_than_upstream_claim_allowed"], false);
  assert_eq!(receipt["default_on_readiness_claim_allowed"], false);
  assert_eq!(receipt["product_level_speed_claim_allowed"], false);
  assert_eq!(
    receipt["app_runtime_webview_ipc_build_bundle_claim_allowed"],
    false
  );
  assert!(receipt["source_parse"]["median_ns"].as_u64().unwrap_or(0) > 0);
  assert!(
    receipt["validated_machine_read"]["median_ns"]
      .as_u64()
      .unwrap_or(0)
      > 0
  );
  assert!(
    receipt["source_to_machine_median_speedup_x"]
      .as_f64()
      .unwrap_or(0.0)
      > 1.0
  );
  assert!(
    receipt["source_to_machine_median_speedup_x"]
      .as_f64()
      .unwrap_or(0.0)
      < 10.0
  );
}
