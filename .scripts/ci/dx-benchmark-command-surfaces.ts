#!/usr/bin/env node
// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const DEFAULT_BENCHMARK_SURFACE_ID = "inspect-wix-upgrade-code";

const CASE_POLICIES = Object.freeze({
  "official-global-tauri\u0000off\u0000direct-cli": Object.freeze({
    role: "official_global_cli",
    source_kind: "prebuilt_global_package",
    invocation_kind: "direct-executable",
    executable_basenames: Object.freeze(["tauri", "global-tauri"]),
    script_basenames: Object.freeze([]),
  }),
  "bun-global-tauri\u0000off\u0000node-wrapper": Object.freeze({
    role: "official_global_cli",
    source_kind: "prebuilt_global_package",
    invocation_kind: "direct-executable",
    executable_basenames: Object.freeze(["tauri"]),
    script_basenames: Object.freeze([]),
  }),
  "official-node-tauri-js\u0000off\u0000node-wrapper": Object.freeze({
    role: "official_node_cli_script",
    source_kind: "prebuilt_global_package",
    invocation_kind: "node-script",
    executable_basenames: Object.freeze(["node"]),
    script_basenames: Object.freeze(["tauri.js"]),
  }),
  "node-direct-tauri-js\u0000off\u0000node-wrapper": Object.freeze({
    role: "official_node_cli_script",
    source_kind: "prebuilt_global_package",
    invocation_kind: "node-script",
    executable_basenames: Object.freeze(["node"]),
    script_basenames: Object.freeze(["tauri.js"]),
  }),
  "official-release-binary\u0000off\u0000direct-rust": Object.freeze({
    role: "official_release_baseline",
    source_kind: "official_release_binary",
    invocation_kind: "direct-executable",
    executable_basenames: Object.freeze(["cargo-tauri", "official-cargo-tauri"]),
    script_basenames: Object.freeze([]),
  }),
  "official-release-zip\u0000off\u0000direct-rust": Object.freeze({
    role: "official_release_baseline",
    source_kind: "official_release_binary",
    invocation_kind: "direct-executable",
    executable_basenames: Object.freeze(["cargo-tauri", "official-cargo-tauri"]),
    script_basenames: Object.freeze([]),
  }),
  "local-current-source\u0000off\u0000direct-rust": Object.freeze({
    role: "local_current_source_cache_off",
    source_kind: "local_current_source_release_binary",
    invocation_kind: "direct-executable",
    executable_basenames: Object.freeze(["cargo-tauri", "local-cargo-tauri", "post6v-dx-feature-cargo-tauri"]),
    script_basenames: Object.freeze([]),
  }),
  "local-current-source\u0000on\u0000direct-rust": Object.freeze({
    role: "local_current_source_cache_on",
    source_kind: "local_current_source_release_binary",
    invocation_kind: "direct-executable",
    executable_basenames: Object.freeze(["cargo-tauri", "local-cargo-tauri", "post6v-dx-feature-cargo-tauri"]),
    script_basenames: Object.freeze([]),
  }),
  "post6v-dx-feature\u0000off\u0000direct-rust": Object.freeze({
    role: "local_current_source_cache_off",
    source_kind: "local_current_source_release_binary",
    invocation_kind: "direct-executable",
    executable_basenames: Object.freeze(["post6v-dx-feature-cargo-tauri"]),
    script_basenames: Object.freeze([]),
  }),
  "post6v-dx-feature\u0000on\u0000direct-rust": Object.freeze({
    role: "local_current_source_cache_on",
    source_kind: "local_current_source_release_binary",
    invocation_kind: "direct-executable",
    executable_basenames: Object.freeze(["post6v-dx-feature-cargo-tauri"]),
    script_basenames: Object.freeze([]),
  }),
  "post6v-current-no-dx\u0000off\u0000direct-rust": Object.freeze({
    role: "legacy_current_source_no_dx",
    source_kind: "local_current_source_release_binary",
    invocation_kind: "direct-executable",
    executable_basenames: Object.freeze(["post6v-current-no-dx-cargo-tauri"]),
    script_basenames: Object.freeze([]),
  }),
});

const BENCHMARK_SURFACES = Object.freeze({
  "inspect-wix-upgrade-code": Object.freeze({
    id: "inspect-wix-upgrade-code",
    command_args: Object.freeze(["inspect", "wix-upgrade-code"]),
    expected_stdout_first: "",
    expected_stderr_first_includes:
      "Info Default WiX Upgrade Code, derived from DX Cache Fixture: e6d1e124-1133-50bf-8993-583a18c73faa",
    evidence_scope: "inspect wix-upgrade-code",
    cache_boundary: "Tauri project config projection",
    current_source_claim_id: "current_source_release_inspect_speed",
    current_source_claim_scope: "current_source_release_inspect_wix_upgrade_code",
    official_release_claim_id: "inspect_official_release_median_comparison",
    official_release_claim_scope: "inspect_wix_upgrade_code_official_release_comparison",
    limitations: Object.freeze([
      "This surface measures inspect wix-upgrade-code only.",
      "It does not measure dev, build, bundle, app runtime, WebView startup, IPC, installer, or watcher behavior.",
    ]),
  }),
  "migrate-stable-v2-noop": Object.freeze({
    id: "migrate-stable-v2-noop",
    command_args: Object.freeze(["migrate"]),
    expected_stdout_first: "",
    expected_stderr_first_includes:
      "Info Nothing to do, the tauri version is already at v2 stable",
    evidence_scope: "migrate on stable-v2 no-op fixture",
    cache_boundary: "Cargo workspace directory discovery",
    current_source_claim_id: "current_source_release_migrate_stable_v2_noop_speed",
    current_source_claim_scope: "current_source_release_migrate_stable_v2_noop",
    official_release_claim_id: "migrate_stable_v2_noop_official_release_median_comparison",
    official_release_claim_scope: "migrate_stable_v2_noop_official_release_comparison",
    limitations: Object.freeze([
      "This surface is safe only on the fixed stable-v2 no-op fixture.",
      "It measures workspace discovery for migrate, not migration rewrite performance.",
      "It does not measure dev, build, bundle, app runtime, WebView startup, IPC, installer, or watcher behavior.",
    ]),
  }),
});

function commandText(surface) {
  return surface.command_args.join(" ");
}

function listBenchmarkSurfaceIds() {
  return Object.keys(BENCHMARK_SURFACES).sort();
}

function getBenchmarkSurface(id) {
  const surface = BENCHMARK_SURFACES[id || DEFAULT_BENCHMARK_SURFACE_ID];
  if (!surface) {
    throw new Error(
      `Unknown benchmark surface: ${id}. Allowed surfaces: ${listBenchmarkSurfaceIds().join(", ")}`,
    );
  }

  return {
    ...surface,
    command: commandText(surface),
    command_args: [...surface.command_args],
    limitations: [...surface.limitations],
  };
}

function basenameWithoutExe(filePath) {
  const name = String(filePath || "").split(/[\\/]/).pop() || "";
  return name.toLowerCase().replace(/\.exe$/, "");
}

function scriptBasename(filePath) {
  return (String(filePath || "").split(/[\\/]/).pop() || "").toLowerCase();
}

function benchmarkCaseKey(row) {
  return `${row.target}\u0000${row.cache_env}\u0000${row.kind}`;
}

function executableMatches(policy, row) {
  const basename = basenameWithoutExe(row.path);
  return policy.executable_basenames.includes(basename);
}

function scriptMatches(policy, row) {
  const script = row.script || "";
  if (policy.script_basenames.length === 0) return script === "";
  return policy.script_basenames.includes(scriptBasename(script));
}

function benchmarkCasePolicy(row) {
  const policy = CASE_POLICIES[benchmarkCaseKey(row)];
  if (!policy) {
    throw new Error(`case invocation is not allowlisted: ${row.target}/${row.cache_env}/${row.kind}`);
  }
  return policy;
}

function applyBenchmarkCasePolicyDefaults(row) {
  const policy = benchmarkCasePolicy(row);
  for (const field of ["role", "source_kind", "invocation_kind"]) {
    if (!row[field]) row[field] = policy[field];
  }
  return row;
}

function validateBenchmarkCase(row) {
  const policy = benchmarkCasePolicy(row);
  for (const field of ["role", "source_kind", "invocation_kind"]) {
    if (row[field] !== policy[field]) {
      throw new Error(
        `case invocation is not allowlisted: ${row.target}/${row.cache_env}/${row.kind} ${field} expected ${policy[field]}, got ${row[field] || "<empty>"}`,
      );
    }
  }
  if (!executableMatches(policy, row) || !scriptMatches(policy, row)) {
    throw new Error(`case invocation is not allowlisted: ${row.target}/${row.cache_env}/${row.kind} executable/script shape`);
  }
  return true;
}

module.exports = {
  DEFAULT_BENCHMARK_SURFACE_ID,
  BENCHMARK_SURFACES,
  applyBenchmarkCasePolicyDefaults,
  getBenchmarkSurface,
  listBenchmarkSurfaceIds,
  validateBenchmarkCase,
};
