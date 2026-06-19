// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const GLOBAL_CHECKS = [
  {
    file: ".gitignore",
    snippets: [".dx/", "**/.dx/", "*.machine"],
  },
  {
    file: "crates/tauri-utils/Cargo.toml",
    snippets: [
      'dx-machine-cache = ["dep:serializer", "dep:rkyv"]',
      'dx-machine-cache-mmap = ["dx-machine-cache", "serializer/typed-cache-mmap"]',
    ],
  },
  {
    file: "crates/tauri-cli/Cargo.toml",
    snippets: [
      'dx-machine-cache = ["tauri-utils/dx-machine-cache", "dep:serializer", "dep:rkyv"]',
      '"serializer/typed-cache-mmap"',
      '"tauri-utils/dx-machine-cache-mmap"',
    ],
  },
  {
    file: "packages/cli/Cargo.toml",
    snippets: [
      'dx-machine-cache = ["tauri-cli/dx-machine-cache"]',
      'dx-machine-cache-mmap = ["dx-machine-cache", "tauri-cli/dx-machine-cache-mmap"]',
    ],
  },
  {
    file: ".scripts/ci/run-dx-no-build-benchmark.ts",
    snippets: [
      'env.TAURI_DX_MACHINE_CACHE_WRITE = "0";',
      "machine_cache_generation_measured: false",
      "machine_cache_command_writes_disabled: true",
      'path.join(outDir, "cache-artifacts-before.csv")',
      'path.join(outDir, "cache-artifacts-after.csv")',
    ],
  },
  {
    file: ".scripts/ci/check-dx-benchmark-manifest.ts",
    snippets: [
      "run-provenance.json machine_cache_generation_measured must be false",
      "run-provenance.json machine_cache_command_writes_disabled must be true",
      "TAURI_DX_MACHINE_CACHE_WRITE mismatch",
      "faster_than_upstream_claim_allowed: false",
      "default_on_readiness_claim_allowed: false",
    ],
  },
  {
    file: ".scripts/ci/test-dx-machine-cache-mmap-coverage.ts",
    snippets: [
      'file: "crates/tauri-utils/src/config/machine_cache.rs"',
      "TauriConfigValueMachineCache",
      "TauriProjectConfigMachineCache",
      "checked mmap fast-path coverage for ${CACHE_READERS.length} machine cache readers",
    ],
  },
];

const SURFACES = [
  {
    id: "tauri-config-value",
    status: "covered-with-local-source-contracts",
    artifacts: ["{cache_name}.machine", "tauri-conf-json.machine", "tauri-conf-json5.machine", "tauri-toml.machine"],
    readerCount: 1,
    writerCount: 1,
    schemaCount: 1,
    pathConstructors: [
      {
        file: "crates/tauri-utils/src/config/machine_cache.rs",
        snippets: [
          "fn tauri_config_machine_cache_paths(",
          'paths_for_project_cache(project_root, "tauri", &cache_name, source_path)',
          "fn cache_name_for_source_path(source_path: &Path)",
          'format!("{cache_name}.machine")',
        ],
      },
    ],
    sourceChecks: [
      {
        file: "crates/tauri-utils/src/config/machine_cache.rs",
        snippets: [
          "struct TauriConfigValueMachineCache",
          "pub fn read_json_value_machine_cache(project_root: &Path, source_path: &Path) -> Option<Value>",
          "pub fn write_json_value_machine_cache(",
          "open_typed_machine_cache::<TauriConfigValueMachineCache>",
          "read_machine_file_bounded(&paths.machine, TAURI_CONFIG_MACHINE_CACHE_MAX_BYTES)?",
          "access_typed_machine_cache::<TauriConfigValueMachineCache>",
          "write_typed_machine_cache(",
          "config_value_machine_schema_name()",
          '"dx.tauri.config_value.normalized.with_json5_toml"',
        ],
      },
    ],
    coverageChecks: [
      {
        file: "crates/tauri-utils/src/config/machine_cache.rs",
        snippets: [
          "fn dx_machine_cache_round_trips_json_config_value_when_source_matches()",
          "fn dx_machine_cache_rejects_stale_source_and_parse_uses_authoritative_json()",
          "fn dx_machine_cache_writes_json5_and_toml_config_value_caches()",
          "fn dx_machine_cache_write_env_disables_automatic_config_writes()",
        ],
      },
    ],
  },
  {
    id: "tauri-project-config",
    status: "covered-with-local-source-contracts",
    artifacts: ["project-config.machine"],
    readerCount: 2,
    writerCount: 1,
    schemaCount: 1,
    pathConstructors: [
      {
        file: "crates/tauri-utils/src/config/machine_cache.rs",
        snippets: [
          "fn tauri_project_config_machine_cache_paths(",
          'paths_for_project_cache(project_root, "tauri", "project-config", source_path)',
        ],
      },
    ],
    sourceChecks: [
      {
        file: "crates/tauri-utils/src/config/machine_cache.rs",
        snippets: [
          "struct TauriProjectConfigMachineCache",
          "pub fn read_cached_project_with_machine_cache(",
          "pub fn read_cached_project_config_projection_with_machine_cache(",
          "fn read_project_config_machine_cache(",
          "fn read_project_config_projection_machine_cache(",
          "fn write_project_config_machine_cache_with_source(",
          "open_typed_machine_cache::<TauriProjectConfigMachineCache>",
          "access_typed_machine_cache::<TauriProjectConfigMachineCache>",
          "project_config_machine_schema_name()",
          '"dx.tauri.project_config.normalized.with_json5_toml"',
        ],
      },
      {
        file: "crates/tauri-utils/src/acl/build.rs",
        snippets: ['.join("project-config.machine")'],
      },
    ],
    coverageChecks: [
      {
        file: "crates/tauri-utils/src/config/machine_cache.rs",
        snippets: [
          "fn dx_machine_cache_project_config_projection_reads_requested_paths()",
          "fn dx_machine_cache_project_config_projection_rejects_stale_base_source()",
          "fn dx_machine_cache_project_config_rejects_cache_when_platform_file_appears()",
          "fn dx_machine_cache_cached_project_read_is_read_only()",
        ],
      },
      {
        file: "crates/tauri-cli/src/inspect.rs",
        snippets: [
          "fn inspect_wix_projection_reads_valid_project_machine_cache_hit()",
          "fn inspect_wix_projection_falls_back_when_product_name_is_missing()",
        ],
      },
      {
        file: ".scripts/ci/test-dx-machine-cache-info-app-config-guard.ts",
        snippets: [
          "dx_info_app_config_section_uses_warm_machine_cache_without_spawning",
          "project-config.machine",
          "not a full `tauri info` command benchmark",
        ],
      },
    ],
  },
  {
    id: "cargo-package-metadata",
    status: "covered-by-cli-mmap-and-read-only-contracts",
    artifacts: ["cargo-package-metadata.machine"],
    readerCount: 1,
    writerCount: 2,
    schemaCount: 1,
    pathConstructors: [
      {
        file: "crates/tauri-cli/src/helpers/cargo_manifest.rs",
        snippets: [
          "fn cargo_metadata_machine_cache_paths(",
          'paths_for_project_cache(',
          '"cargo-package-metadata"',
          "&tauri_dir.join(\"Cargo.toml\")",
        ],
      },
    ],
    sourceChecks: [
      {
        file: "crates/tauri-cli/src/helpers/cargo_manifest.rs",
        snippets: [
          "struct CargoMetadataMachineCache",
          "fn read_cargo_metadata_machine_cache(",
          "fn write_cargo_metadata_machine_cache(",
          "open_typed_machine_cache::<CargoMetadataMachineCache>",
          "cargo_metadata_from_archive(mapped.archived(), tauri_dir, &source)",
          "read_machine_file_bounded(&paths.machine, CARGO_METADATA_MACHINE_CACHE_MAX_BYTES)?",
          '"dx.tauri.cli.cargo_package_metadata"',
        ],
      },
    ],
    coverageChecks: [
      {
        file: ".scripts/ci/test-dx-machine-cache-mmap-coverage.ts",
        snippets: ['file: "crates/tauri-cli/src/helpers/cargo_manifest.rs"'],
      },
      {
        file: ".scripts/ci/test-dx-machine-cache-read-only-mode.ts",
        snippets: [
          'file: "crates/tauri-cli/src/helpers/cargo_manifest.rs"',
          "write_cargo_metadata_machine_cache(tauri_dir, &parsed.0, &parsed.1)",
        ],
      },
      {
        file: "crates/tauri-cli/src/helpers/cargo_manifest.rs",
        snippets: [
          "fn dx_cargo_metadata_cache_hit_does_not_require_cargo_metadata()",
          "fn dx_cargo_metadata_cache_rejects_changed_manifest()",
          "fn dx_cargo_metadata_cache_rejects_corrupt_machine_file()",
        ],
      },
      {
        file: ".scripts/ci/test-dx-cargo-package-metadata-benchmark-receipt.ts",
        snippets: [
          "dx_cargo_package_metadata_machine_cache_writes_source_vs_machine_receipt",
          '"pre-generated dx-serializer .machine read"',
          '"machine_cache_generation_measured"',
        ],
      },
    ],
  },
  {
    id: "package-version",
    status: "covered-by-cli-mmap-and-read-only-contracts",
    artifacts: ["package-version.machine"],
    readerCount: 1,
    writerCount: 1,
    schemaCount: 1,
    pathConstructors: [
      {
        file: "crates/tauri-cli/src/helpers/config/package_version_machine_cache.rs",
        snippets: [
          "fn package_version_machine_cache_paths(",
          'paths_for_project_cache(',
          '"package-version"',
          "&candidate.package_json_path",
        ],
      },
    ],
    sourceChecks: [
      {
        file: "crates/tauri-cli/src/helpers/config/package_version_machine_cache.rs",
        snippets: [
          "struct PackageVersionMachineCache",
          "fn read_package_version_machine_cache(",
          "pub(super) fn write_package_version_machine_cache(",
          "open_typed_machine_cache::<PackageVersionMachineCache>",
          "package_version_from_archive(mapped.archived(), &source)",
          "read_machine_file_bounded(&paths.machine, PACKAGE_VERSION_MACHINE_CACHE_MAX_BYTES)?",
          '"dx.tauri.cli.package_version"',
        ],
      },
    ],
    coverageChecks: [
      {
        file: ".scripts/ci/test-dx-machine-cache-mmap-coverage.ts",
        snippets: ['file: "crates/tauri-cli/src/helpers/config/package_version_machine_cache.rs"'],
      },
      {
        file: ".scripts/ci/test-dx-machine-cache-read-only-mode.ts",
        snippets: [
          'file: "crates/tauri-cli/src/helpers/config.rs"',
          "package_version_machine_cache::write_package_version_machine_cache(candidate, version)",
        ],
      },
      {
        file: "crates/tauri-cli/src/helpers/config.rs",
        snippets: [
          "fn dx_package_version_machine_cache_replaces_version_path_on_hit()",
          "fn dx_package_version_machine_cache_rejects_stale_package_json()",
          "fn dx_package_version_machine_cache_load_config_does_not_rewrite_after_valid_hit()",
        ],
      },
      {
        file: ".scripts/ci/test-dx-package-version-benchmark-receipt.ts",
        snippets: [
          "dx_package_version_machine_cache_writes_source_vs_machine_receipt",
          '"pre-generated dx-serializer .machine read"',
          '"machine_cache_generation_measured"',
        ],
      },
      {
        file: ".scripts/ci/test-dx-machine-cache-info-app-config-guard.ts",
        snippets: [
          "dx_info_app_config_section_uses_warm_machine_cache_without_spawning",
          "package_version_machine_path",
          "package_version_machine_cache_read_hits_for_tests()",
        ],
      },
    ],
  },
  {
    id: "cargo-config",
    status: "covered-by-cli-mmap-and-read-only-contracts",
    artifacts: ["cargo-config.machine"],
    readerCount: 1,
    writerCount: 1,
    schemaCount: 1,
    pathConstructors: [
      {
        file: "crates/tauri-cli/src/interface/rust/cargo_config/machine_cache.rs",
        snippets: [
          "fn cargo_config_machine_cache_paths(",
          'paths_for_project_cache(',
          '"cargo-config"',
          "&tauri_dir.join(\"Cargo.toml\")",
        ],
      },
    ],
    sourceChecks: [
      {
        file: "crates/tauri-cli/src/interface/rust/cargo_config/machine_cache.rs",
        snippets: [
          "struct CargoConfigMachineCache",
          "pub(super) fn read(tauri_dir: &Path) -> Option<Config>",
          "pub(super) fn write(tauri_dir: &Path, config: &Config)",
          "open_typed_machine_cache::<CargoConfigMachineCache>",
          "cargo_config_from_archive(mapped.archived(), tauri_dir, &source)",
          "read_machine_file_bounded(&paths.machine, CARGO_CONFIG_MACHINE_CACHE_MAX_BYTES)?",
          '"dx.tauri.cli.cargo_config"',
        ],
      },
    ],
    coverageChecks: [
      {
        file: ".scripts/ci/test-dx-machine-cache-mmap-coverage.ts",
        snippets: ['file: "crates/tauri-cli/src/interface/rust/cargo_config/machine_cache.rs"'],
      },
      {
        file: ".scripts/ci/test-dx-machine-cache-read-only-mode.ts",
        snippets: [
          'file: "crates/tauri-cli/src/interface/rust/cargo_config.rs"',
          "machine_cache::write(path, &config)",
        ],
      },
      {
        file: "crates/tauri-cli/src/interface/rust/cargo_config.rs",
        snippets: [
          "fn dx_cargo_config_machine_cache_hit_reads_cached_target_before_source_parse()",
          "fn dx_cargo_config_machine_cache_rejects_changed_local_config()",
          "fn dx_cargo_config_machine_cache_does_not_write_empty_config()",
        ],
      },
    ],
  },
  {
    id: "full-cargo-metadata",
    status: "benchmarked-and-fallback-covered",
    artifacts: ["cargo-metadata.machine"],
    readerCount: 2,
    writerCount: 1,
    schemaCount: 1,
    pathConstructors: [
      {
        file: "crates/tauri-cli/src/interface/rust/cargo_metadata_machine_cache.rs",
        snippets: [
          "fn cargo_metadata_machine_cache_paths(",
          'paths_for_project_cache(',
          '"cargo-metadata"',
          "&tauri_dir.join(\"Cargo.toml\")",
        ],
      },
    ],
    sourceChecks: [
      {
        file: "crates/tauri-cli/src/interface/rust/cargo_metadata_machine_cache.rs",
        snippets: [
          "struct CargoMetadataMachineCache",
          "pub(super) fn read(tauri_dir: &Path) -> Option<CargoMetadata>",
          "pub(super) fn write(",
          "open_typed_machine_cache::<CargoMetadataMachineCache>",
          "cargo_metadata_from_archive(mapped.archived(), tauri_dir, &source)",
          "read_machine_file_bounded(&paths.machine, CARGO_METADATA_MACHINE_CACHE_MAX_BYTES)?",
          '"dx.tauri.cli.cargo_metadata_full"',
        ],
      },
    ],
    coverageChecks: [
      {
        file: ".scripts/ci/test-dx-machine-cache-mmap-coverage.ts",
        snippets: ['file: "crates/tauri-cli/src/interface/rust/cargo_metadata_machine_cache.rs"'],
      },
      {
        file: ".scripts/ci/test-dx-machine-cache-read-only-mode.ts",
        snippets: [
          'file: "crates/tauri-cli/src/interface/rust.rs"',
          "cargo_metadata_machine_cache::write(tauri_dir, &metadata)",
        ],
      },
      {
        file: ".scripts/ci/test-dx-cargo-metadata-fallback-coverage.ts",
        snippets: [
          "dx_full_cargo_metadata_machine_cache_corrupt_file_falls_back_and_refreshes",
          "dx_full_cargo_metadata_machine_cache_oversized_file_falls_back_without_trusting_cache",
          "dx_full_cargo_metadata_machine_cache_unsupported_schema_falls_back_and_refreshes",
        ],
      },
      {
        file: ".scripts/ci/test-dx-cargo-metadata-machine-benchmark-receipt.ts",
        snippets: [
          "dx_full_cargo_metadata_machine_cache_writes_current_fork_benchmark_receipt",
          '"pre-generated dx-serializer .machine read"',
          '"machine_cache_generation_measured"',
        ],
      },
    ],
  },
  {
    id: "cargo-settings",
    status: "covered-by-cli-mmap-and-read-only-contracts",
    artifacts: ["cargo-settings.machine"],
    readerCount: 1,
    writerCount: 1,
    schemaCount: 1,
    pathConstructors: [
      {
        file: "crates/tauri-cli/src/interface/rust/cargo_settings_machine_cache.rs",
        snippets: [
          "fn cargo_settings_machine_cache_paths(",
          'paths_for_project_cache(dir, "tauri", "cargo-settings", &dir.join("Cargo.toml"))',
        ],
      },
    ],
    sourceChecks: [
      {
        file: "crates/tauri-cli/src/interface/rust/cargo_settings_machine_cache.rs",
        snippets: [
          "struct CargoSettingsMachineCache",
          "pub(super) fn read(dir: &Path) -> Option<CargoSettings>",
          "pub(super) fn write(dir: &Path, settings: &CargoSettings)",
          "open_typed_machine_cache::<CargoSettingsMachineCache>",
          "cargo_settings_from_archive(mapped.archived(), &source)",
          "read_machine_file_bounded(&paths.machine, CARGO_SETTINGS_MACHINE_CACHE_MAX_BYTES)?",
          '"dx.tauri.cli.cargo_settings"',
        ],
      },
    ],
    coverageChecks: [
      {
        file: ".scripts/ci/test-dx-machine-cache-mmap-coverage.ts",
        snippets: ['file: "crates/tauri-cli/src/interface/rust/cargo_settings_machine_cache.rs"'],
      },
      {
        file: ".scripts/ci/test-dx-machine-cache-read-only-mode.ts",
        snippets: [
          'file: "crates/tauri-cli/src/interface/rust.rs"',
          "cargo_settings_machine_cache::write(dir, &settings)",
        ],
      },
      {
        file: "crates/tauri-cli/src/interface/rust/machine_cache_tests.rs",
        snippets: [
          "fn dx_cargo_settings_cache_load_reads_valid_machine_before_source_parse()",
          "fn dx_cargo_settings_cache_rejects_stale_manifest_source()",
          "fn dx_cargo_settings_cache_round_trips_workspace_and_bin_projection()",
        ],
      },
    ],
  },
  {
    id: "cargo-workspace",
    status: "covered-by-cli-mmap-and-read-only-contracts",
    artifacts: ["cargo-workspace.machine"],
    readerCount: 1,
    writerCount: 1,
    schemaCount: 1,
    pathConstructors: [
      {
        file: "crates/tauri-cli/src/interface/rust/workspace_machine_cache.rs",
        snippets: [
          "fn cargo_workspace_machine_cache_paths(",
          'paths_for_project_cache(',
          '"cargo-workspace"',
          "&tauri_dir.join(\"Cargo.toml\")",
        ],
      },
    ],
    sourceChecks: [
      {
        file: "crates/tauri-cli/src/interface/rust/workspace_machine_cache.rs",
        snippets: [
          "struct CargoWorkspaceMachineCache",
          "pub(super) fn read(tauri_dir: &Path) -> Option<PathBuf>",
          "pub(super) fn write(tauri_dir: &Path, workspace_dir: &Path)",
          "open_typed_machine_cache::<CargoWorkspaceMachineCache>",
          "workspace_dir_from_archive(mapped.archived(), tauri_dir, &source)",
          "read_machine_file_bounded(&paths.machine, CARGO_WORKSPACE_MACHINE_CACHE_MAX_BYTES)?",
          '"dx.tauri.cli.cargo_workspace_dir"',
        ],
      },
    ],
    coverageChecks: [
      {
        file: ".scripts/ci/test-dx-machine-cache-mmap-coverage.ts",
        snippets: ['file: "crates/tauri-cli/src/interface/rust/workspace_machine_cache.rs"'],
      },
      {
        file: ".scripts/ci/test-dx-machine-cache-read-only-mode.ts",
        snippets: [
          'file: "crates/tauri-cli/src/interface/rust.rs"',
          "workspace_machine_cache::write(tauri_dir, &workspace_dir)",
        ],
      },
      {
        file: "crates/tauri-cli/src/interface/rust/machine_cache_tests.rs",
        snippets: [
          "fn dx_workspace_dir_machine_cache_hit_does_not_require_cargo_metadata()",
          "fn dx_workspace_dir_machine_cache_rejects_changed_workspace_manifest()",
          "fn dx_workspace_dir_machine_cache_rejects_corrupt_machine_file()",
        ],
      },
    ],
  },
];


module.exports = { GLOBAL_CHECKS, SURFACES };
