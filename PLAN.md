# Tauri Serializer Cache Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` or `superpowers:executing-plans` before changing code. Use focused tests and checks only unless the user explicitly approves heavy builds.

Last reviewed: 2026-05-31
Status: Active, evidence-gated
Canonical repo: `G:\Dx\tauri`
Evidence root: `G:\Dx\test-outputs`

**Goal:** Make selected cache-crossing Tauri CLI/config hot paths measurably faster than explicitly measured baselines by using DX serializer `.machine` caches, without changing public behavior or implying whole-product upstream superiority.

**Architecture:** Human-readable Tauri config, Cargo metadata inputs, and CLI receipts remain the source of truth. Local `.machine` sidecars are opt-in, source-fingerprint validated, schema-aware, and always treated as disposable acceleration artifacts. The DX ecosystem is expected to generate `.machine` files before the user-facing Tauri command runs; governed timing must compare normal JSON/TOML parse cost against pre-generated DX serializer binary read cost, not include `.machine` generation time. Cache reads must fall back to the existing parser or live probe on any stale, corrupt, unsupported, or unsafe state.

**Current Readiness:** Selected hot-path and governance evidence is strong for named surfaces; whole-product upstream superiority remains unclaimable.

The foundation is locally committed on `dev`, not upstreamed or release-published, and now has a safe direct project-config projection slice with redundant archived-tree validation removed, cheaper missing-sidecar read paths, path-indexed CLI source validation, stronger benchmark-governance checks, a read-only measured-cache mode, all-surface write-disabled hit/miss behavior tests, receipt-backed SHA-bound release timing for `inspect wix-upgrade-code` and guarded `migrate-stable-v2-noop`, feature-gated developer docs, a governed allowlisted command-surface registry for benchmark broadening, a measured full `cargo metadata --no-deps` sidecar, a measured dev/watch folder-discovery boundary, a measured package manifest/lock source-vs-machine boundary, a measured package-version source-vs-machine boundary, stronger full-metadata fallback tests, a central `.machine` surface matrix, mmap coverage for both CLI and tauri-utils config readers, indexed full-metadata cached-source preflight validation, indexed Cargo config source-dir preflight validation, exact full-metadata cached source-set completeness checks, indexed workspace cached-source preflight validation, HashSet-backed source-snapshot insertion across the multi-source cache builders, direct archived-field reads for package-version, workspace-discovery, package manifest/lock, and full Cargo metadata cache hits, benchmark sample guards that keep broad claim gates fail-closed, and an MSRV CI boundary that keeps local DX machine-cache features out of Rust 1.77.2 tauri-utils all-feature lanes. The broad goal is still not proven: this does not yet prove full Tauri product superiority across app runtime, WebView startup, IPC, build, dev, bundle, install, or watch workflows.

---

## Current Decision

Stage 10 fixed the `inspect wix-upgrade-code` JSON-cache regression by skipping the direct project-config projection attempt for schema-validated JSON/JSON5 configs, avoiding an unnecessary failed `.machine` read before the full validated cached read. Stage 10's receipt-backed command benchmarks at git `af391adbd1e0fbc07ca4e84f3c77dc62abb2c695` remain the latest official-release comparison source in this plan. In the receipt-backed `inspect wix-upgrade-code` run, local current-source cache-on measured `50.854 ms` median versus local cache-off `64.078 ms` and official release `155.504 ms`; the checker allowed the same-binary claim at `79.363%` of cache-off median and the official-release comparison at `32.703%` of official median. In the receipt-backed `migrate-stable-v2-noop` run on the fixed stable-v2 fixture, local current-source cache-on measured `41.027 ms` median versus local cache-off `89.453 ms` and official release `94.448 ms`; the checker allowed the same-binary claim at `45.864%` of cache-off median and the official-release comparison at `43.439%` of official median. Stage 51 refreshed the current-fork parser-vs-machine hot-path evidence after adding narrow Cargo metadata projections for watch-folder and target-directory callers: `cargo metadata --no-deps` measured `118.234 ms` source median versus `10.447 ms` validated `.machine` median, or `11.317x` faster by median, and the latest labeled `get_watch_folders` receipt measured `141.635 ms` source median versus `10.333 ms` machine median, or `13.708x` faster by median. These are strong hot-path results, not a whole-product Tauri victory claim.
Stage 11 closed the feature-gated developer-docs gap by documenting the `TAURI_DX_MACHINE_CACHE` and `TAURI_DX_MACHINE_CACHE_WRITE` gates, `dx-machine-cache` / `dx-machine-cache-mmap` feature requirements, pre-generated `.machine` timing boundary, default-off behavior, source authority, fallback policy, and claim limits.
Stage 12 tightened benchmark governance for future command-benchmark runs: command-benchmark metadata now uses schema `dx.tauri.wave7.command_benchmark.v2`, new runs record `machine_cache_write_env_for_all_samples = "0"`, the runner sets `TAURI_DX_MACHINE_CACHE_WRITE=0` for cache-on, cache-off, and unset samples, and the checker preserves legacy v1 receipt compatibility while requiring the stricter v2 policy.
Stage 13 removed repeated archived JSON tree validation from the project-config projection hot path. The projection reader still validates the archived tree once after envelope/source-set validation; each requested projected value then uses a private unchecked lookup helper instead of re-walking the whole tree for every path.
Stage 14 moved project/config cache candidate checks ahead of source fingerprinting for JSON value, merged project-config, and project-config projection reads. Missing, directory, or oversized `.machine` sidecars now return a cache miss before hashing the authoritative source file.
Stage 15 replaced repeated linear source matching in four CLI machine-cache validators with path-indexed matching. This removes O(n²) validation scans on full Cargo metadata, workspace discovery, Cargo config, and package manifest/lock cache hits while preserving the existing source equality semantics.
Stage 16 fixed a benchmark-governance receipt overclaim: v2 samples with cache-write env mismatches now make `sample_invocations_verified` and `benchmark_integrity_verified` false instead of leaving those booleans true while the overall check status is failed.
Stage 17 removed the now-dead checked project-config projection helper after Stage 13 moved the only hot-path caller to the single-validation unchecked helper. Focused tauri-utils checks now run without the dead-code warning.
Stage 18 tightened command-benchmark v2 evidence again: new runner outputs include structured `argv_json`, v2 `binaries.csv` rows must carry allowlisted role/source/invocation metadata, executable/script basenames are checked against the reviewed case registry, and late v2 run-provenance failures can no longer leave `run_provenance_verified` true.
Stage 19 replaced text-marker-only process-sweep trust for command-benchmark v2 with structured JSON receipts. Future v2 runs now emit preflight/final process-sweep JSON with roots, command names, matched processes, clean status, and log hashes; the checker rejects missing JSON or `clean: true` receipts that still contain matched processes.
Stage 20 added the dirty-git benchmark gate for command-benchmark v2. Non-plan runner executions now require a valid clean git HEAD before timing, and the checker rejects v2 receipts whose run provenance records a dirty worktree or non-empty `status_short`.
Stage 21 moved remaining CLI `.machine` fallback reads behind a shared bounded IO helper. The six CLI typed readers that still used direct `fs::read(&paths.machine)` now preflight regular-file/size candidacy and read through `File::take(max + 1)`, closing the metadata/read growth race that tauri-utils had already avoided.
Stage 22 tightened CLI source-set equality for indexed validators. The source matcher now rejects duplicate expected or current paths instead of relying only on equal vector length plus membership checks, and the package manifest/lock cache has a direct regression test for duplicate cached sources.
Stage 23 strengthened local current-source build-log validation. Build receipts now need a real `build_log` file record whose file is under `G:\Dx\test-outputs`, whose byte/hash identity matches the recorded file, and whose text proves a completed `tauri-cli` release build through either the expected cargo build command or `tauri-cli` compile evidence plus the release-profile completion line. The checker and writer reject logs containing build failure output, and receipt-copying scripts now copy the build log beside the copied receipt and rewrite its file record.
Stage 24 tightened governance booleans so failed sample execution, stale output-file rehashes, missing summary rows, and failed receipts cannot still export misleading verified/allowed fields. `sample_invocations_verified`, `output_equivalence_verified`, `summary_recomputed_from_samples`, and `official_release_snapshot_allowed` now turn false when their underlying receipt evidence is broken.
Stage 25 tightened the tauri-utils manual timing receipt output guard. The ignored receipt writer now rejects non-absolute output paths, parent-directory traversal, sibling-prefix paths, and existing ancestor escapes before accepting a path under canonical `G:\Dx\test-outputs`.
Stage 26 started Rust-side cached-source preflight validation in the CLI readers. `cargo-workspace.machine` now rejects oversized source lists, unsafe path shapes, workspace roots outside the app ancestry, unrelated absolute paths, and non-`Cargo.toml` cached sources before fingerprinting cached paths.
Stage 27 extended cached-source preflight validation to `cargo-config.machine`. The reader now rejects oversized source lists, unsafe path shapes, unrelated absolute paths, and non-Cargo-config filenames before fingerprinting cached config source paths.
Stage 28 completed the current CLI cached-source preflight family by hardening `cargo-metadata.machine`. The full metadata reader now rejects oversized source lists, unsafe path shapes, workspace roots outside the app ancestry, unrelated absolute paths, and unexpected metadata source filenames before fingerprinting cached paths.
Stage 29 removed one manifest-level publication prerequisite for the optional local `dx-serializer` path dependency by adding the explicit crate version in both Tauri manifests. This does not solve publish/default-on readiness because the dependency still resolves to `G:\Dx\serializer` locally and that crate still has Rust 1.85 / edition 2024 metadata.
Stage 30 added a release-boundary guard for the DX machine-cache feature. A focused TypeScript check now verifies the optional serializer dependency metadata, default-off feature wiring, local serializer and derive version/MSRV/edition metadata, official publish workflow feature usage, and docs/plan wording that blocks publish/default-on claims while the local path dependency remains unresolved.
Stage 31 indexed the full `cargo-metadata.machine` cached-source preflight. The cache read path now builds the allowed metadata source set once per archive read and uses set membership for every cached source, preserving the same workspace-root, package-manifest, ancestor-manifest, and unsafe-path rejection behavior without re-scanning the package list for each source path.
Stage 32 indexed the `cargo-config.machine` cached-source preflight. The cache read path now builds the allowed ancestor `.cargo` directory and `CARGO_HOME` directory set once per archive read and uses set membership for every cached config source, preserving the same allowed `config`/`config.toml` and unsafe-path rejection behavior.
Stage 33 refreshed the two existing current-fork source-vs-machine timing receipts after Stage 31 and Stage 32. The refreshed `cargo metadata --no-deps` receipt remains above the requested 10x hot-path threshold at `11.687x` faster by median with `.machine` generation excluded, and the refreshed `get_watch_folders` boundary remains `16.062x` faster by median with cache writes disabled during timing.
Stage 34 tightened cached-source validation and trimmed repeated hot-path normalization. Full `cargo-metadata.machine` reads now require the cached source set to exactly match the expected metadata source set, so an otherwise valid sidecar that omits `Cargo.lock`, a required package manifest, a workspace config slot, or an ancestor manifest is rejected before fingerprint comparison. Cached metadata package manifest paths are also required to be safe workspace `Cargo.toml` paths. Cargo metadata and Cargo config readers precompute app-manifest exclusion paths once per cache read instead of re-normalizing them inside every cached-source filter iteration.
Stage 35 wired release fail-closed checks into the release path. The covector workflow now runs the release-boundary guard before covector, and Rust prepublish now runs the same guard in publish mode so publication is blocked until serializer registry/MSRV release readiness is proven.
Stage 36 tightened workspace-discovery cached-source validation and trimmed repeated hot-path normalization. `cargo-workspace.machine` reads now build the expected workspace `Cargo.toml` source set once per archive read, reject incomplete or unexpected cached source sets before fingerprint comparison, and precompute app/tauri manifest exclusions once per read instead of re-normalizing those paths inside the cached-source loop.
Stage 37 removed remaining per-insert linear duplicate scans from the multi-source snapshot builders. Full metadata, workspace, Cargo config, and package manifest/lock cache snapshots now keep a local `HashSet` of seen normalized source paths while preserving insertion order in the final source vector, and the source-matching guard now fails if those helpers regress to `sources.iter().any(...)` scans.
Stage 38 removed the full-payload deserialize from `package-version.machine` cache hits. The reader now validates `package_json_path` and semver directly from the archived fields after typed-cache validation, then returns an owned version string; the mmap coverage guard fails if this path regresses to the old deserialize helper.
Stage 39 tightened benchmark claim governance in the sample-invocation guard. The current-source build receipt fixture must now prove that `faster_than_upstream`, `default_on`, `full_cli`, and app/runtime/WebView/IPC/build/bundle claim gates stay false, that the matching blocked-claim records are present, and that allowed claims do not contain broad product-superiority wording.
Stage 40 added an MSRV/all-features CI boundary for `tauri-utils`. The `test-core` workflow now runs a focused guard and uses an explicit public tauri-utils feature list for the Rust 1.77.2 all-feature lane, excluding local-only `dx-machine-cache` and `dx-machine-cache-mmap` while leaving the normal `tauri` feature matrix intact.
Stage 41 removed the full-cache deserialize from `cargo-workspace.machine` hits. The reader now validates the archived `tauri_dir`, reads the archived `workspace_dir` directly, reconstructs only the archived source fingerprints needed for existing source validation, and the mmap coverage guard fails if this path regresses to the old workspace deserialize helper.
Stage 42 registered the DX machine-cache guard suite so every `.scripts/ci/test-dx-machine-cache-*.ts` guard is tracked against broad-claim, release-boundary, MSRV-boundary, untracked-cache-surface, write-disabled-timing, read-order, or projection-validation risk coverage.
Stage 43 ensures the serializer release policy now fails closed while `dx-serializer` remains a local-only, unpublished, MSRV-incompatible dependency: publish mode is blocked by release policy even if `DX_ALLOW_MACHINE_CACHE_PUBLICATION=1` is present.
Stage 44 removes wrapper-level deserialization from `cargo-settings.machine` cache hits. The reader now compares the archived `cargo_toml_path` directly and deserializes only the owned settings payload after typed-cache/source validation; the mmap coverage guard fails if this path reintroduces the old full-wrapper deserialize helper.
Stage 45 removes full-wrapper deserialization from `cargo-config.machine` cache hits. The reader now validates archived `tauri_dir`, `cargo_home`, and cached source fingerprints directly, then materializes only the target string needed for the returned Cargo build config.
Stage 46 removes full-wrapper deserialization from full project-config `.machine` reads. The reader now validates the archived config path and source fingerprints directly, materializes merged/platform JSON values from archived JSON trees, and keeps the existing projection direct-archive path intact.
Stage 47 adds a no-build project-config source-vs-machine timing receipt and records an honest negative result for the tiny fixture: source parse measured `421,500 ns` median while validated `.machine` read measured `487,400 ns` median, so this fixture does not prove a project-config speedup.
Stage 48 reruns the existing leaf-config JSON source-vs-machine timing receipt and records another negative tiny-fixture result: source parse measured `110,000 ns` median while validated `.machine` read measured `301,800 ns` median, so tiny config reads should not be used for speedup claims.
Stage 49 adds a representative generated project-config timing receipt and records a positive but bounded config result: a `250,713` byte source fixture measured `10.603 ms` source parse median versus `4.783 ms` validated `.machine` read median, so this fixture is about `2.22x` faster with `.machine` generation excluded. This supports a fixture-local representative config-read claim only, not a generic config speedup, broad Tauri superiority, or a 10x product-level win.
Stage 50 removed wrapper-level deserialization from full `cargo-metadata.machine` cache hits and refreshed the then-current positive hot-path receipts: `cargo metadata --no-deps` measured `15.572x` faster by median, and `get_watch_folders` measured `14.421x` faster by median, with `.machine` generation excluded and cache writes disabled during timing.
Stage 51 adds a narrow validated projection for Cargo metadata cache hits so watch-folder and target-directory callers can avoid full metadata materialization. Focused no-spawn tests prove both projection callers avoid Cargo on warm cache hits. The latest projection-era receipts remain above the requested 10x hot-path threshold: `cargo metadata --no-deps` measured `11.317x` faster by median, and the latest labeled `get_watch_folders` receipt measured `13.708x` faster by median.
Stage 52 removes wrapper-level deserialization from `cargo-package-metadata.machine` package manifest/lock cache hits, materializes manifest dependencies and lock packages directly from archived fields, and adds archive-path tests for dependency/lock fidelity plus unsafe archived workspace and lock paths. This is a cache hit-path cleanup, not a new benchmark or product-level speed claim.
Stage 53 adds a focused same-process package manifest/lock timing receipt after the Stage 52 direct-archive cleanup. With `cargo-package-metadata.machine` generated during setup before machine timing and `TAURI_DX_MACHINE_CACHE_WRITE=0` during timed machine samples, source parsing measured `50.553 ms` median versus `2.894 ms` validated `.machine` read median, or `17.470x` faster by median. This is current-fork helper-level hot-path evidence only, not a full CLI, official-release, upstream-source, default-on, release-readiness, or product-level claim.
Stage 54 adds a focused same-process package-version timing receipt after the Stage 38 direct-archive cleanup. With `package-version.machine` generated during setup before machine timing, the broader `project-config.machine` sidecar kept absent, and `TAURI_DX_MACHINE_CACHE_WRITE=0` during timed machine samples, source parsing measured `9.741 ms` median versus `1.957 ms` validated `.machine` read median, or `4.978x` faster by median. This is a real package-version helper-level win. It is below the requested 10x threshold and is not a full CLI, official-release, upstream-source, default-on, release-readiness, config-wide, mmap, or product-level claim.
Stage 55 adds a no-spawn isolated `tauri info` app/config section harness. It crosses the existing project-config and package-version config paths through `get_config(Target::current(), &[], tauri_dir)` plus `info::app::items`, keeps cache writes disabled during the warm-cache assertion, proves the package-version `.machine` hit count, asserts the App section has no action callbacks, and verifies `project-config.machine` and `package-version.machine` remain byte-stable. This is behavior evidence for the command-adjacent app/config section only, not a full `tauri info` command benchmark and not a timing result.
Stage 56 removes wrapper-level deserialization from `tauri-conf-*.machine` leaf config cache hits. The reader now validates the archived `source_path` directly and materializes only the archived JSON tree payload, matching the direct-archive style already used by the other `.machine` cache hit paths. This is a cache hit-path cleanup, not a new benchmark or product-level speed claim.

## Current State Audit

### What Has Been Tried

- Added typed DX serializer `.machine` cache integration for Tauri config values and merged project config.
- Added cache boundaries for Cargo package metadata, package version resolution, Cargo settings, workspace discovery, and target-bearing Cargo config.
- Added a narrow `inspect wix-upgrade-code` shortcut so explicit `productName` configs avoid unnecessary Rust app settings work.
- Added a read-only project-config cache-hit path for `inspect wix-upgrade-code`.
- Added no-build benchmark governance scripts for existing-binary comparison runs.
- Reduced avoidable cache churn around source fingerprinting, repeated writes, mtime-only drift, leaf-cache writes, and non-mmap machine-file reads.
- Added focused regression coverage for stale sources, default-off behavior, platform config changes, invalid cache payloads, project-cache source sets, and `inspect` projection fallback behavior.
- Moved cache-heavy CLI unit tests into a dedicated `machine_cache_tests` module so `interface/rust.rs` stays focused.
- Committed the verified cache foundation as `16a5c0831 feat(cli): add DX machine cache acceleration for Tauri config`.
- Added direct archived project-config string projection that reuses machine-cache envelope validation and Tauri source-set validation without reconstructing the full config tree.
- Wired `inspect wix-upgrade-code` to use the direct projection for non-schema config sources, while JSON/JSON5 still fall back to the full cached config path.
- Added benchmark checker validation for sample-level command, args, and cwd, backed by a green/red fixture helper that writes receipts under `G:\Dx\test-outputs`.
- Added a TypeScript no-build Stage 4 comparison preparer for official/global Tauri CLI, official release binary, and local current-source release binary cases.
- Added focused TypeScript/Node coverage for preparation output: comparison receipt, case config, report, binary/script hashes, same-local-binary cache on/off identity, disabled faster-than-upstream claim gates, and runner rejection of stale prepared hashes.
- Extended the serializer mmap/open fast path to CLI `.machine` readers for Cargo package metadata, package version, Cargo config, Cargo settings, and Cargo workspace discovery while preserving the existing `fs::read` fallback.
- Added `TAURI_DX_MACHINE_CACHE_WRITE=0` support so benchmarked user-command runs can read pre-generated `.machine` files without generating or mutating cache artifacts during timing.
- Built the local current-source release binary with `dx-machine-cache-mmap`, manually pre-generated the fixture `.machine` file as setup, and ran two governed post-generation benchmark passes.
- Added a benchmark-governance path for `local-current-source-build-receipt.json` so current-source release speed claims can be tied to a clean git SHA, a release build command, and the measured local binary identity.
- Added a full typed `cargo metadata --no-deps` `.machine` sidecar for the CLI metadata hot path, including read-only command behavior, source invalidation, `CARGO_TARGET_DIR` invalidation, and mmap/source-scan coverage.
- Added an ignored current-fork cargo-metadata benchmark receipt that manually writes `cargo-metadata.machine` before timing, then compares source Cargo JSON parsing against read-only `.machine` hits.
- Added an ignored current-fork dev/watch folder-discovery benchmark receipt that uses pre-generated `cargo-metadata.machine` during `get_watch_folders` and keeps cache writes disabled during timing.
- Added focused full cargo-metadata fallback tests for corrupt, oversized, and wrong-schema `.machine` files, proving they fall back to live Cargo metadata and refresh the sidecar instead of trusting unsafe cache state.
- Added a central `.machine` surface matrix contract covering all current reader, writer, schema, path-constructor, artifact, feature-forwarding, ignore-policy, and benchmark-governance surfaces.
- Extended the mmap fast-path coverage contract from the six CLI readers to all ten typed readers, including tauri-utils config-value, project-config, and project-config projection readers.
- Added real write-disabled behavior tests across the current cache surfaces so `TAURI_DX_MACHINE_CACHE_WRITE=0` still reads pre-generated hits but does not create sidecars on misses.
- Added a small local release build receipt writer and ran a fresh governed current-source release-binary benchmark for `inspect wix-upgrade-code` against official/global baselines.
- Added a small allowlisted benchmark-surface registry so command benchmark broadening stays data-driven instead of accepting arbitrary command strings.
- Added `migrate-stable-v2-noop` as a guarded candidate surface for the fixed stable-v2 fixture only, with explicit claim limits around Cargo workspace discovery and no migration rewrite claims.
- Committed the governed surface registry as `26ceae4b0 test(ci): add governed benchmark surface registry`.
- Built the current HEAD local release binary, wrote a clean build receipt, and reran receipt-backed official-release comparisons for both `inspect wix-upgrade-code` and `migrate-stable-v2-noop`.
- Committed `af391adbd perf(cli): skip unused inspect projection for schema configs` after tracing the `inspect` cache-on regression to an avoidable JSON/JSON5 projection attempt.
- Rebuilt the latest HEAD, wrote a clean build receipt, and reran receipt-backed official-release comparisons for both `inspect wix-upgrade-code` and `migrate-stable-v2-noop`.
- Added feature-gated developer documentation for DX machine-cache environment variables and a focused TypeScript docs contract.
- Added command-benchmark schema v2 so every future measured sample has `TAURI_DX_MACHINE_CACHE_WRITE=0`, while legacy v1 benchmark receipts remain checkable.
- Removed duplicate archived JSON tree validation from the project-config projection path and added a TypeScript source guard for the single-validation contract.
- Moved bounded `.machine` file candidate checks before source fingerprinting in tauri-utils config readers and added a TypeScript read-order guard.
- Replaced nested source matching in four CLI cache validators with local `HashMap`-indexed helpers and added a TypeScript source-matching guard.
- Tightened benchmark-governance receipt booleans so sample environment failures cannot be reported as verified sample invocations or benchmark integrity.
- Removed the dead checked projection helper and updated the source guard to require the single-validation projection path directly.
- Added benchmark case allowlisting and structured `argv_json` proof for command-benchmark v2 receipts.
- Fixed a late v2 run-provenance overclaim where `run_provenance_verified` could stay true after a v2-only provenance failure was appended.
- Added structured process-sweep JSON receipts for command-benchmark v2 and made the checker trust the JSON structure instead of text markers.
- Added a command-benchmark v2 dirty-git gate in both the runner and checker.
- Added a shared CLI bounded `.machine` read helper and routed remaining CLI fallback reads through it.
- Tightened indexed CLI source matching so duplicate source paths cannot mask an omitted source.
- Strengthened local-current-source build receipt validation around build-log file identity, release-build evidence, failure-output rejection, and copied build-log locality.

### What Is Proven

- `TAURI_DX_MACHINE_CACHE` is default-off for the implemented cache paths.
- Corrupt, stale, missing, changed-source, and unsupported caches fall back instead of becoming authority in the focused covered paths.
- Focused Rust checks pass for the current config/project-cache and inspect-projection slices.
- The moved CLI cache tests pass for Cargo settings and workspace-discovery machine-cache behavior.
- Direct projection falls back when the cache is disabled, when base or platform sources change, when a platform file appears, when an ancestor path is the wrong type, and when JSON/JSON5 schema validation is still required.
- The benchmark checker rejects samples whose recorded command, args, or cwd do not match the expected invocation derived from `binaries.csv` and the fixture path.
- The Stage 4 preparation path can assemble comparison inputs without running a build or benchmark.
- Prepared comparison cases record selected binary/script SHA-256 identities and keep faster-than-upstream claim gates disabled.
- The no-build runner rejects a prepared case config when recorded binary or script identity no longer matches the filesystem.
- The no-build runner now snapshots `.machine` artifacts before warmups, requires pre-generated `.machine` files, and fails if they are created or mutated during warmups or timed samples.
- The `dx-machine-cache-mmap` feature now enables `serializer/typed-cache-mmap`, and focused compile/source checks prove the CLI cache readers are wired to mmap before `fs::read` fallback.
- In two governed `inspect wix-upgrade-code` release-binary runs, local cache-on was fastest after `.machine` setup:
  - Run 1 median: official release `238.804 ms`, local cache-off `74.915 ms`, local cache-on `59.722 ms`.
  - Run 2 median: official release `254.965 ms`, local cache-off `79.693 ms`, local cache-on `71.332 ms`.
- The `.machine` artifact was generated before timing and stayed byte-for-byte stable through warmups and timed samples.
- A warm full cargo-metadata `.machine` sidecar can satisfy `get_cargo_metadata()` without spawning Cargo in focused tests.
- On the current fork's `crates/tauri-cli` package, the refreshed pre-generated cargo metadata `.machine` path measured `5.910 ms` median versus `69.070 ms` median for Cargo spawn plus `serde_json` parse, an `8.556%` machine/source ratio and `11.687x` source-to-machine median speedup. The `.machine` file was unchanged during timing.
- On the current fork's `crates/tauri-cli` package, refreshed `get_watch_folders` measured `7.140 ms` median with the pre-generated `.machine` path versus `114.677 ms` median with cache disabled, a `6.226%` machine/source ratio and `16.062x` source-to-machine median speedup. The `.machine` file was unchanged during timing.
- Stage 50 was the previous full-metadata current-fork hot-path timing refresh. The pre-generated cargo metadata `.machine` path measured `6.398 ms` median versus `99.627 ms` median for Cargo spawn plus `serde_json` parse, a `6.422%` machine/source ratio and `15.572x` source-to-machine median speedup. Refreshed `get_watch_folders` measured `5.628 ms` median with the pre-generated `.machine` path versus `81.168 ms` median with cache disabled, a `6.934%` machine/source ratio and `14.421x` source-to-machine median speedup. The `.machine` file was unchanged during timing.
- Stage 51 is the latest current-fork parser-vs-machine hot-path timing source. The refreshed `cargo metadata --no-deps` receipt measured `118.234 ms` source median versus `10.447 ms` validated `.machine` median, or `11.317x` faster by median, and the latest labeled `get_watch_folders` receipt measured `141.635 ms` source median versus `10.333 ms` machine median, or `13.708x` faster by median.
- Stage 53 is the latest package manifest/lock helper-level timing source. The `cargo_manifest_and_lock` source parser receipt measured `50.553 ms` source median versus `2.894 ms` validated `.machine` median, a `5.724%` machine/source ratio and `17.470x` source-to-machine median speedup. The `.machine` file was unchanged during timing, and cache writes were disabled during timed machine samples.
- Stage 54 is the latest package-version helper-level timing source and a real narrow win. The `load_config` package-version fixture measured `9.741 ms` source median versus `1.957 ms` validated `.machine` median, a `20.087%` machine/source ratio and `4.978x` source-to-machine median speedup. The package-version `.machine` file was unchanged during timing, cache writes were disabled, the broader project-config `.machine` sidecar was kept absent during timing, source timing recorded `0` package-version machine hits, and timed machine warmups/samples recorded the expected `140` package-version machine hits.
- Stage 55 is a no-spawn isolated `tauri info` app/config section harness. It proves the current fork can execute the App section's config-facing path with `TAURI_DX_MACHINE_CACHE_WRITE=0`, a validated package-version cache hit, byte-stable `project-config.machine` and `package-version.machine`, and no App `SectionItem` actions. It is not a full `tauri info` command benchmark and avoids the full command's environment probes, network lookups, package-manager calls, and mobile/toolchain noise.
- Stage 56 removes a full-wrapper deserialize from `tauri-conf-*.machine` leaf config cache hits. The mmap coverage guard now fails if this reader reintroduces `deserialize_tauri_config_machine_archive` instead of using direct archived-field access.
- Full cargo-metadata fallback behavior is now covered for missing/read-only miss, stale workspace manifest, changed `CARGO_TARGET_DIR`, corrupt bytes, oversized sidecar, and unsupported schema envelope.
- The surface matrix now fails if a new typed reader, typed writer, `MachineCacheSchema`, `paths_for_project_cache` constructor, or quoted `.machine` artifact appears in `tauri-cli` or `tauri-utils` source without being cataloged and tied to coverage checks.
- The mmap coverage script now fails if any of the ten typed readers stops using `open_typed_machine_cache` before its bounded/file-read fallback.
- `TAURI_DX_MACHINE_CACHE_WRITE=0` behavior is now proven by focused Rust tests for Tauri config/project projection, Cargo package metadata, package-version, Cargo config, Cargo settings, workspace discovery, and full Cargo metadata.
- A Stage 7 SHA-bound governed command benchmark verified binary hashes, output equivalence, sample invocations, cache artifact stability, process sweeps, and local build receipt integrity; local current-source cache-on measured `30.765%` of official-release median for `inspect wix-upgrade-code`.
- The benchmark runner/checker can now preserve and validate the selected allowlisted benchmark surface, and the checker still passes when rechecking the Stage 7 `inspect wix-upgrade-code` evidence.
- The accepted no-build `migrate-stable-v2-noop` run verified binary hashes, output equivalence, sample invocations, cache artifact stability, and clean process sweeps; for the same local binary and same fixed fixture, cache-on measured `30.622 ms` median versus cache-off at `104.122 ms`, or `29.41%` of cache-off median.
- A historical Stage 9 current-HEAD-at-that-time `inspect wix-upgrade-code` run verified a clean build receipt and official-release comparison: local current-source cache-on measured `66.013 ms` median versus official release `163.470 ms`, or `40.382%` of official median.
- A historical Stage 9 current-HEAD-at-that-time `migrate-stable-v2-noop` run verified a clean build receipt and official-release comparison: local current-source cache-on measured `30.645 ms` median versus local cache-off `104.224 ms` and official release `115.638 ms`, or `29.403%` of local cache-off median and `26.501%` of official median.
- The Stage 10 `inspect wix-upgrade-code` run at git `af391adbd1e0fbc07ca4e84f3c77dc62abb2c695` remains the latest official-release comparison receipt for that surface: local current-source cache-on measured `50.854 ms` median versus local cache-off `64.078 ms` and official release `155.504 ms`, or `79.363%` of local cache-off median and `32.703%` of official median.
- The Stage 10 `migrate-stable-v2-noop` run at git `af391adbd1e0fbc07ca4e84f3c77dc62abb2c695` remains the latest official-release comparison receipt for that surface: local current-source cache-on measured `41.027 ms` median versus local cache-off `89.453 ms` and official release `94.448 ms`, or `45.864%` of local cache-off median and `43.439%` of official median.
- Developer-facing docs now state that the cache is feature-gated, default-off, read/write controlled, based on pre-generated `.machine` artifacts, and limited to narrow proven hot paths rather than broad Tauri product claims.
- Future command-benchmark v2 receipts prove writes are disabled for all measured samples, not just cache-on samples; the checker rejects v2 samples with missing or different `TAURI_DX_MACHINE_CACHE_WRITE` values.
- The project-config projection hot path now validates the archived merged config tree once per cache hit before projecting all requested paths, and focused tauri-utils projection tests still pass.
- Missing, non-file, or oversized JSON/project-config `.machine` sidecars now miss before hashing source files in the covered tauri-utils readers; focused project-config machine-cache tests still pass.
- Full Cargo metadata, workspace discovery, Cargo config, and package manifest/lock cache validators now build a path-indexed map for current source snapshots instead of scanning every current source for every cached source.
- The benchmark checker now treats `TAURI_DX_MACHINE_CACHE` / `TAURI_DX_MACHINE_CACHE_WRITE` sample environment mismatches as sample verification failures and requires zero failures for benchmark integrity.
- The tauri-utils machine-cache check for the projection cleanup now completes without the `archived_json_tree_projected_value_at_path` dead-code warning.
- Future command-benchmark v2 receipts now prove the exact spawned argv shape through `argv_json`; the checker rejects collapsed or otherwise mismatched argument arrays.
- Future command-benchmark v2 receipts now validate every binary case against an allowlisted target/cache/kind, role, source kind, invocation kind, executable basename, and wrapper script basename.
- Late v2 run-provenance failures now make `run_provenance_verified` false in both the manifest and governance check.
- Future command-benchmark v2 receipts now require preflight/final process-sweep JSON; the checker rejects missing JSON and receipts where `clean: true` conflicts with a non-empty `matched_processes` array.
- Future non-plan command-benchmark v2 runs must start from a clean git worktree, and the checker rejects v2 receipts with dirty run provenance.
- CLI machine-cache fallback reads are now bounded after candidate preflight across Cargo package metadata, package version, Cargo config, Cargo settings, full Cargo metadata, and workspace discovery readers.
- CLI source validators for full Cargo metadata, workspace discovery, Cargo config, and package manifest/lock now require unique expected and current source path sets before comparing fingerprints.
- Current-source release speed claims now require verified build-log evidence; missing, stale, weak, or failure-containing build logs block `local_current_source_build_receipt_verified` and the current-source claim gate.
- Governance receipt booleans now fail closed for timed-out samples, stale output files, missing summary rows, and failed receipts that merely contain an official release binary snapshot.
- The tauri-utils manual source-vs-machine timing receipt guard now rejects parent traversal, sibling-prefix, and existing ancestor escapes before writing evidence under `G:\Dx\test-outputs`.
- The workspace-discovery `.machine` reader now validates cached source path plausibility before hashing current source files.
- The Cargo config `.machine` reader now validates cached config source path plausibility before hashing current source files.
- The full Cargo metadata `.machine` reader now validates cached metadata source path plausibility before hashing current source files.
- The full Cargo metadata `.machine` reader now precomputes allowed cached source paths once per archive read, avoiding repeated package-manifest scans during source preflight.
- The Cargo config `.machine` reader now precomputes allowed `.cargo` and `CARGO_HOME` directories once per archive read, avoiding repeated ancestor-dir scans during source preflight.
- The full Cargo metadata `.machine` reader now rejects incomplete cached source sets instead of comparing only the subset listed by a malformed sidecar.
- The full Cargo metadata `.machine` reader now rejects unsafe package manifest paths embedded in cached metadata.
- Cargo metadata and Cargo config cached-source loops now precompute app-manifest source exclusions once per archive read.
- The workspace-discovery `.machine` reader now precomputes the expected workspace `Cargo.toml` source set once per archive read, avoiding repeated ancestor checks during source preflight.
- The workspace-discovery `.machine` reader now rejects incomplete, duplicate, or unexpected cached source sets before fingerprint comparison.
- The workspace-discovery cached-source loop now precomputes app/tauri manifest source exclusions once per archive read.
- Full metadata, workspace discovery, Cargo config, and package manifest/lock snapshot builders now de-duplicate source paths with a local `HashSet` instead of scanning the accumulated source vector before every insert.
- The source-matching guard now fails if those snapshot push helpers reintroduce `sources.iter().any(...)` duplicate checks.
- Package-version cache hits now read the archived `package_json_path` and `version` fields directly after typed-cache validation instead of deserializing the full cache payload.
- The mmap coverage guard now fails if the package-version reader stops using direct archived-field access or reintroduces the old deserialize helper.
- Benchmark sample-invocation coverage now asserts that broad upstream, default-on, full CLI, and app/runtime/WebView/IPC/build/bundle gates stay false even when the narrow current-source build receipt is valid.
- Allowed benchmark claims are now checked for broad product-superiority wording in the sample guard.
- The `test-core` workflow now runs a DX machine-cache MSRV boundary guard before Rust setup.
- The Rust 1.77.2 `tauri-utils` all-feature lane now uses an explicit public feature list that excludes local-only DX machine-cache features.
- Workspace-discovery cache hits now read archived `tauri_dir` and `workspace_dir` fields directly after typed-cache validation instead of deserializing the full cache payload.
- The workspace direct archive path still reuses the existing cached-source completeness and fingerprint matching semantics by reconstructing only source fingerprints from the archived source list.
- The mmap coverage guard now fails if the workspace reader stops using direct archived-field access or reintroduces the old deserialize helper.
- The machine-cache guard suite now registers every `.scripts/ci/test-dx-machine-cache-*.ts` guard against an explicit governance risk category.
- Publish mode now fails closed while `dx-serializer` remains local-only, unpublished, or MSRV-incompatible, even if the old `DX_ALLOW_MACHINE_CACHE_PUBLICATION=1` override is present.
- Cargo settings cache hits now compare the archived `cargo_toml_path` directly before deserializing the settings payload, avoiding full wrapper materialization on valid hits.
- The mmap coverage guard now fails if the Cargo settings reader stops using direct archived-path access or reintroduces the old full-wrapper deserialize helper.
- Cargo config cache hits now validate archived path, cargo-home, and source-fingerprint fields directly instead of deserializing the full cache wrapper before source validation.
- The mmap coverage guard now fails if the Cargo config reader stops using direct archived-field access or reintroduces the old full-wrapper deserialize helper.
- Full project-config cache hits now validate archived config path/source fingerprints directly and materialize merged/platform `serde_json::Value`s from archived JSON trees without deserializing the full project-config wrapper.
- The mmap coverage guard now fails if the full project-config reader stops using direct archived-field access or reintroduces the old full-wrapper deserialize helper.
- A full project-config timing receipt now exists, and its first tiny-fixture result shows validated `.machine` read slower than source parse. This is evidence to narrow future benchmarking, not a speed claim.
- The existing leaf-config JSON timing receipt also shows validated `.machine` read slower than source parse on its tiny fixture. Small config files are currently bad evidence for serializer speed claims.
- The covector release workflow now runs the DX machine-cache release-boundary guard before covector.
- Rust prepublish now runs the release-boundary guard in publish mode, which currently fails closed because DX cache features are present on publishable crates while serializer registry/MSRV readiness is unresolved.
- The optional `dx-serializer` dependency declarations now include an explicit `0.1.0` version alongside the local path, which is a prerequisite for a future published dependency path.
- Release-boundary docs and a TypeScript guard now keep the local `dx-serializer` path dependency, Rust 1.85 / edition 2024 mismatch, the derive crate also declares Rust 1.85 / edition 2024, and blocked publish/default-on readiness visible.
- Preparation receipts are governance inputs only; they are not timing evidence and do not prove any speedup.
- Existing old-binary runs show narrow `inspect wix-upgrade-code` cache-on improvements in some clean runs.
- Broad official/upstream superiority remains blocked. Only fixture-local official-release binary comparisons for `inspect wix-upgrade-code`/`migrate-stable-v2-noop` and current-fork parser-vs-machine hot-path measurements for cargo-metadata/watch-folder are claimable.

### What Is Not Proven

- No broad current-source release benchmark proves this fork is faster than official/upstream. The claimable evidence is limited to fixture-local official-release binary comparisons for `inspect wix-upgrade-code`/`migrate-stable-v2-noop` plus current-fork parser-vs-machine hot-path measurements for cargo-metadata/watch-folder.
- Stage 31 is a validation-cost hardening slice, not a new benchmark result or product-level speed claim.
- Stage 32 is also a validation-cost hardening slice, not a new benchmark result or product-level speed claim.
- Stage 33 refreshes current-fork hot-path timings only; it still does not measure an official Tauri binary, upstream-source checkout, full CLI workflow, release build, app runtime, WebView startup, IPC, bundling, dev loop, installer, or default-on distribution artifact.
- Stage 34 is a cache-correctness and validation-cost hardening slice, not a new benchmark result or product-level speed claim.
- Stage 35 blocks accidental release publication; it does not make the serializer publishable, MSRV-compatible, cargo-vetted, default-on, or upstream-ready.
- Stage 36 is a workspace cache-correctness and validation-cost hardening slice, not a new benchmark result or product-level speed claim.
- Stage 37 is a source-snapshot construction cleanup, not a new benchmark result or product-level speed claim.
- Stage 38 is a small package-version hit-path cleanup, not a new benchmark result or product-level speed claim.
- Stage 39 is benchmark-governance hardening, not a new benchmark result or product-level speed claim.
- Stage 40 is CI/release-lane hardening, not publish readiness or a guarantee that the local serializer is MSRV-compatible.
- Stage 41 is a workspace cache hit-path cleanup, not a new benchmark result or product-level speed claim.
- Stage 42 is guard registration and claim-hygiene hardening, not a new performance result.
- Stage 43 is release-policy hardening, not a published serializer, MSRV fix, cargo-vet receipt, or upstream/default-on readiness proof.
- Stage 44 is a Cargo settings cache hit-path cleanup, not a new benchmark result or product-level speed claim.
- Stage 45 is a Cargo config cache hit-path cleanup, not a new benchmark result or product-level speed claim.
- Stage 46 is a full project-config cache hit-path cleanup, not a new benchmark result or product-level speed claim.
- Stage 47 is a no-build timing receipt for one tiny project-config fixture, and its measured result is negative for `.machine` speed on that fixture.
- Stage 48 is a no-build timing receipt for one tiny leaf-config fixture, and its measured result is also negative for `.machine` speed on that fixture.
- Stage 49 is a no-build timing receipt for one representative generated project-config fixture, and its measured result is positive but bounded to that fixture: `2.22x` faster by median, not a generic config-speedup or product-level claim.
- Stage 50 is a full Cargo metadata cache hit-path cleanup plus refreshed current-fork timing receipts, not official-release or product-level evidence.
- Stage 51 is a Cargo metadata projection cleanup plus refreshed current-fork timing receipts, not official-release or product-level evidence.
- Stage 52 is a package manifest/lock cache hit-path cleanup with focused direct-archive tests, not a new timing receipt, official-release evidence, or product-level speed claim.
- Stage 53 is a package manifest/lock source-vs-machine timing receipt, not official-release, upstream-source, full CLI, default-on, release-readiness, or product-level evidence.
- Stage 54 is a package-version source-vs-machine timing receipt, not official-release, upstream-source, full CLI, default-on, release-readiness, mmap, config-wide, or product-level evidence; it is a real `4.978x` narrow win but below 10x on its current representative generated package.json fixture.
- Stage 55 is current-fork app/config-section behavior evidence only, not a timing receipt, full CLI, official-release, upstream-source, default-on, release-readiness, app runtime, WebView, IPC, build, dev, bundle, watch, installer, mobile/toolchain, or product-level claim.
- Stage 56 is a leaf config cache-hit cleanup only, not a timing receipt, generic config speedup, official-release comparison, upstream-source comparison, default-on readiness proof, full CLI benchmark, app runtime, WebView, IPC, build, dev, bundle, watch, installer, mobile/toolchain, or product-level claim.
- No broad Tauri app runtime, WebView startup, IPC, build, dev, bundle, install, or watch speedup is proven.
- No default-on readiness is proven.
- No npm/crates.io publish readiness is proven while the DX serializer dependency is local and packaging/MSRV constraints are unresolved.
- Direct projection is not yet used for JSON/JSON5 configs because the existing schema validator requires the materialized JSON value or a future schema-validation receipt.
- DX serializer is not upstream/default-release ready in this fork shape: it is a local path dependency outside this workspace, uses Rust 1.85/edition 2024, its derive crate also declares Rust 1.85 / edition 2024, while Tauri's workspace MSRV remains Rust 1.77.2.
- Read-only audit inputs so far found no safe third full CLI benchmark surface yet: `info` touches cache boundaries but is contaminated by env probes and network lookups; completions and permission listing are stable but do not cross `.machine` cache boundaries.
- Release engineering remains blocked by the local `dx-serializer` path dependency, MSRV mismatch, cache-enabled artifact packaging, and supply-chain vet/audit policy.
- Broader default-on, publish, runtime, and full-CLI claim families still need explicit receipt families before broader evidence claims should be accepted.

---

## Production Standards

- Do not add decorative names, marketing labels, or unnecessary revision suffixes to code APIs.
- New project-specific script work should be TypeScript (`.ts`) and split by responsibility before files become hard to maintain. Existing upstream JavaScript CI scripts may remain as compatibility surfaces until they are deliberately migrated.
- Prefer domain names such as `MachineCacheSource`, `ProjectConfigProjection`, `CacheReadReport`, and `BenchmarkIntegrityReport`.
- Use schema revision numbers only inside cache envelopes or metadata where compatibility requires them.
- Keep Tauri commands thin; put cache and parsing logic in `tauri-utils` or focused CLI helper modules.
- Keep source JSON/TOML/Cargo files authoritative. `.machine` files are never authority.
- Do not count `.machine` generation time in Tauri performance claims; generation belongs to the DX ecosystem before the measured user command.
- Keep public CLI JSON output and Tauri config behavior unchanged.
- Do not cache package-manager output, registry responses, Node wrapper package checks, WebView/OS probes, signing secrets, mobile certificates, watcher state, dev-server output, or generated build artifacts.

---

## Completion Plan

### Stage 1: Stabilize And Commit Current Work

**Purpose:** Stop accumulating uncommitted experimental changes and lock in the current safe cache foundation.

- [x] Re-run the smallest meaningful checks:
  - Focused `rustfmt --check` on touched Rust files. Do not use `cargo fmt --all -- --check` on this Windows checkout because the broader repo currently reports newline-style drift outside the DX cache files.
  - `cargo check -p tauri-utils --no-default-features --features dx-machine-cache,config-json5,config-toml --lib --color never`
  - `cargo check -p tauri-cli --features dx-machine-cache --lib --color never`
  - `cargo test -p tauri-utils --no-default-features --features dx-machine-cache,config-json5,config-toml --lib dx_machine_cache_project_config -j1 --color never -- --test-threads=1`
  - `cargo test -p tauri-cli --features dx-machine-cache --lib inspect_wix_projection -j1 --color never -- --test-threads=1`
  - `node --check .\.scripts\ci\run-dx-no-build-benchmark.ts`
  - `node --check .\.scripts\ci\check-dx-benchmark-manifest.ts`
  - `git diff --check`
- [x] Confirm no `G:\Dx\bun` or stale Tauri compile processes are contaminating verification.
- [x] Commit only repository source and scripts. Do not commit `G:\Dx\test-outputs` artifacts.
- [x] Use one professional commit if the tree is interdependent:
  - `feat(cli): add DX machine cache acceleration for Tauri config`

### Stage 2: Add Direct Archived Project-Config Projection Safely

**Purpose:** Reduce warm-cache overhead in `inspect wix-upgrade-code` by reading only required fields from the validated archived tree.

- [x] Add a private `TauriConfigJsonTree` path reader that can return string leaves without reconstructing a full `serde_json::Value`.
- [x] Cover it with focused tests for string leaves, nested strings, missing paths, non-string values, malformed ancestry, and existing malformed-tree guards.
- [x] Add a read-only project-config projection helper that reuses the existing serializer envelope validation and Tauri source-set validation.
- [x] Wire it into `inspect` only for config sources that do not require JSON-schema validation.
- [x] Keep JSON/JSON5 on the full cached config path because schema validation cannot currently be preserved without materializing JSON.

### Stage 3: Harden Benchmark Integrity Before New Claims

**Purpose:** Make benchmark output trustworthy enough to support or reject speed claims.

- [x] Make the no-build runner fail when final process sweeps are dirty, even without checker mode.
- [x] Record structured preflight/final process sweep status in provenance.
- [x] Require sample integrity columns in the checker: effective cache env, timeout, signal, spawn error, command, args, and cwd.
- [x] Reject claim eligibility if any sample timed out, signaled, spawned incorrectly, or ran with mismatched cache env.
- [x] Validate that sample-level command, args, and cwd match the expected benchmark invocation.
- [x] Keep benchmark outputs under `G:\Dx\test-outputs`.

Latest receipts:

- `G:\Dx\test-outputs\tauri-stage3-semantic-validation-20260530-a\valid-sample-invocation-check\wave7-benchmark-manifest-check.json`
- `G:\Dx\test-outputs\tauri-stage3-semantic-validation-20260530-a\bad-sample-invocation-check\wave7-benchmark-manifest-check.json`
- `G:\Dx\test-outputs\tauri-stage3-semantic-validation-20260530-a\20-green-red-sample-invocation-helper.log`

### Stage 4: Governed Current-Source Comparison

**Purpose:** Produce the first honest current-source speed answer.

- [x] Prepare the Stage 4 current-source comparison inputs without building or benchmarking: official/global Tauri CLI baselines, official release binary baseline, local current-source release binary case, identical fixture, command metadata, raw-receipt locations, binary hashes, process-sweep requirements, and disabled claim gates.
- [x] Enforce pre-generated cache semantics in the runner: `.machine` artifacts must exist before warmups and must stay stable through warmups and measured samples.
- [x] Use official prebuilt/global Tauri CLI and official release binary as baselines in governed timing runs.
- [x] Use the local current-source binary only after a deliberate release build window.
- [x] Compare identical fixtures, binary hashes, git SHAs, command lines, cache state, raw logs, median, p95, and process sweeps for `inspect wix-upgrade-code`.
- [x] Claim only the scopes the checker allows: same-local-binary cache-on/off improvement is allowed; broad faster-than-upstream, default-on, app/WebView/IPC/build/bundle claims remain blocked.
- [x] Record the actual result without reframing it: local cache-on won the measured `inspect wix-upgrade-code` path, while wider Tauri speed claims remain unproven.

Latest preparation receipts:

- `G:\Dx\test-outputs\tauri-stage4-current-source-comparison-prep-20260530-a\stage4-config-receipt.json`
- `G:\Dx\test-outputs\tauri-stage4-current-source-comparison-prep-20260530-a\binaries.csv`
- `G:\Dx\test-outputs\tauri-stage4-current-source-comparison-prep-20260530-a\runner-plan-only\wave12-run-plan.json`

Latest pre-generated cache-boundary receipts:

- `G:\Dx\test-outputs\tauri-stage4-comparison-prep-20260530-a\310-comparison-prep-helper-pre-generated-ts.log`
- `G:\Dx\test-outputs\tauri-stage4-comparison-prep-20260530-a\313-real-runner-plan-only-pre-generated-ts.log`
- `G:\Dx\test-outputs\tauri-stage4-comparison-prep-20260530-a\390-verification-summary-pre-generated.json`

### Stage 5: Production Hardening

**Purpose:** Move from local experiment to production-ready fork work.

- [ ] Decide whether DX serializer is a permanent local dependency, vendored workspace crate, or upstream-safe optional feature.
- [x] Enable serializer mmap reads across CLI and tauri-utils `.machine` readers where the cache already exists and keep non-mmap fallback behavior intact.
- [ ] Confirm MSRV and packaging impact.
- [x] Add feature-gated docs for developers.
- [ ] Keep default-off until current-source benchmark evidence and safety review justify any wider rollout.
- [ ] Split large helper files only when the split improves ownership and testability.
- [ ] Add focused CI lanes for `dx-machine-cache` and `dx-machine-cache-mmap` on modern stable Rust, separate from the Tauri MSRV lane.
- [ ] Resolve cargo-vet/audit policy for `dx-serializer`, `rkyv 0.8.x`, and `memmap2 0.9.x` before any release-channel claim.

Latest mmap fast-path receipts:

- `G:\Dx\test-outputs\tauri-stage5-mmap-fast-path-20260530-a\030-node-mmap-coverage-test.log`
- `G:\Dx\test-outputs\tauri-stage5-mmap-fast-path-20260530-a\040-cargo-check-tauri-cli-mmap.log`
- `G:\Dx\test-outputs\tauri-stage5-mmap-fast-path-20260530-a\099-verification-summary-mmap-fast-path.json`
- `G:\Dx\test-outputs\tauri-stage6-tauri-utils-mmap-coverage-20260531-a\010-red-tauri-utils-mmap-gap.log`
- `G:\Dx\test-outputs\tauri-stage6-tauri-utils-mmap-coverage-20260531-a\020-green-tauri-utils-mmap-coverage.log`
- `G:\Dx\test-outputs\tauri-stage6-tauri-utils-mmap-coverage-20260531-a\030-green-surface-matrix-after-tauri-utils-mmap.log`

Latest honest benchmark receipts:

- `G:\Dx\test-outputs\tauri-honest-machine-benchmark-20260530-a\022-pre-generate-machine-cache-setup-real-cwd.log`
- `G:\Dx\test-outputs\tauri-honest-machine-benchmark-20260530-a\023-pre-generated-machine-artifacts.json`
- `G:\Dx\test-outputs\tauri-honest-machine-benchmark-20260530-a\benchmark-output-rerun1\summary.csv`
- `G:\Dx\test-outputs\tauri-honest-machine-benchmark-20260530-a\benchmark-output-rerun2\summary.csv`
- `G:\Dx\test-outputs\tauri-honest-machine-benchmark-20260530-a\044-BRUTAL-RESULT.md`

Latest build-receipt governance receipts:

- `G:\Dx\test-outputs\tauri-stage6-build-receipt-governance-20260531-a\070-green-dirty-build-receipt-test.log`
- `G:\Dx\test-outputs\tauri-stage6-build-receipt-governance-20260531-a\090-green-prep-runner-receipt-handoff-test.log`

Latest developer-doc receipts:

- `G:\Dx\test-outputs\tauri-stage11-docs-hardening-20260531-a\010-red-machine-cache-docs.log`
- `G:\Dx\test-outputs\tauri-stage11-docs-hardening-20260531-a\020-green-machine-cache-docs.log`
- `G:\Dx\test-outputs\tauri-stage11-docs-hardening-20260531-a\050-node-check-docs-test-final-rerun.log`
- `G:\Dx\test-outputs\tauri-stage11-docs-hardening-20260531-a\060-green-machine-cache-docs-final-rerun.log`
- `G:\Dx\test-outputs\tauri-stage11-docs-hardening-20260531-a\070-surface-matrix-after-docs-final-rerun.log`

### Stage 6: Next Speed Evidence Gate

**Purpose:** Broaden from the proven inspect path into a higher-value CLI hot path without counting `.machine` generation time.

- [x] Build a full `cargo metadata --no-deps` sidecar for current-source local runs.
- [x] Fingerprint workspace manifests, package manifests, lockfile presence/content, relevant `.cargo/config*`, and target-directory-affecting inputs.
- [x] Prove a cache hit avoids spawning `cargo metadata` with a fake `cargo` regression test.
- [x] Benchmark official cargo-metadata spawn plus JSON parse against pre-generated `.machine` mmap reads on the same current-fork package.
- [x] Benchmark a command-level dev/watch folder-discovery boundary that naturally uses `get_cargo_metadata`.
- [x] Keep fallback behavior exact on missing, stale, corrupt, unsupported, or unsafe cache state.
- [x] Add a central `.machine` surface matrix so future cache readers, writers, schemas, path constructors, and artifact names cannot skip coverage.
- [x] Prove write-disabled hit/miss behavior across every current cache surface.

Latest cargo metadata sidecar receipts:

- `G:\Dx\test-outputs\tauri-stage6-cargo-metadata-sidecar-20260531-a\010-red-full-cargo-metadata-cache-hit.log`
- `G:\Dx\test-outputs\tauri-stage6-cargo-metadata-sidecar-20260531-a\130-red-target-dir-env-invalidation.log`
- `G:\Dx\test-outputs\tauri-stage6-cargo-metadata-sidecar-20260531-a\150-green-full-cargo-metadata-cache-focused-tests-final.log`
- `G:\Dx\test-outputs\tauri-stage6-cargo-metadata-sidecar-20260531-a\120-cargo-check-tauri-cli-mmap-full-metadata.log`
- `G:\Dx\test-outputs\tauri-stage6-cargo-metadata-benchmark-20260531-a\020-red-cargo-metadata-benchmark-contract.log`
- `G:\Dx\test-outputs\tauri-stage6-cargo-metadata-benchmark-20260531-a\260-cargo-metadata-current-fork-benchmark-after-module-split.log`
- `G:\Dx\test-outputs\tauri-stage6-cargo-metadata-benchmark-20260531-a\cargo-metadata-source-vs-machine-receipt.json`
- `G:\Dx\test-outputs\tauri-stage6-cargo-metadata-benchmark-20260531-a\170-BRUTAL-CARGO-METADATA-RESULT.md`
- `G:\Dx\test-outputs\tauri-stage6-watch-folders-benchmark-20260531-a\010-red-watch-folders-benchmark-contract.log`
- `G:\Dx\test-outputs\tauri-stage6-watch-folders-benchmark-20260531-a\040-watch-folders-current-fork-benchmark.log`
- `G:\Dx\test-outputs\tauri-stage6-watch-folders-benchmark-20260531-a\watch-folders-source-vs-machine-receipt.json`
- `G:\Dx\test-outputs\tauri-stage6-watch-folders-benchmark-20260531-a\050-BRUTAL-WATCH-FOLDERS-RESULT.md`
- `G:\Dx\test-outputs\tauri-stage6-cargo-metadata-fallback-20260531-a\090-red-unsupported-schema-fallback-contract.log`
- `G:\Dx\test-outputs\tauri-stage6-cargo-metadata-fallback-20260531-a\120-green-fallback-coverage-unsupported-schema.log`
- `G:\Dx\test-outputs\tauri-stage6-cargo-metadata-fallback-20260531-a\130-full-cargo-metadata-cache-fallback-tests-unsupported-schema.log`
- `G:\Dx\test-outputs\tauri-stage6-cargo-metadata-fallback-20260531-a\cargo-metadata-fallback-coverage.json`

Latest surface matrix receipts:

- `G:\Dx\test-outputs\tauri-stage6-machine-surface-matrix-20260531-a\010-red-empty-surface-matrix.log`
- `G:\Dx\test-outputs\tauri-stage6-machine-surface-matrix-20260531-a\021-green-surface-matrix-after-split.log`
- `G:\Dx\test-outputs\tauri-stage6-machine-surface-matrix-20260531-a\022-green-surface-matrix-global-checks.log`
- `G:\Dx\test-outputs\tauri-stage6-machine-surface-matrix-20260531-a\030-existing-mmap-coverage.log`
- `G:\Dx\test-outputs\tauri-stage6-machine-surface-matrix-20260531-a\031-existing-read-only-coverage.log`
- `G:\Dx\test-outputs\tauri-stage6-machine-surface-matrix-20260531-a\032-existing-cargo-metadata-fallback-coverage.log`
- `G:\Dx\test-outputs\tauri-stage6-machine-surface-matrix-20260531-a\033-existing-cargo-metadata-benchmark-contract.log`
- `G:\Dx\test-outputs\tauri-stage6-machine-surface-matrix-20260531-a\machine-cache-surface-matrix.json`

Latest read-only behavior receipts:

- `G:\Dx\test-outputs\tauri-stage6-read-only-behavior-20260531-a\010-green-read-only-source-contract.log`
- `G:\Dx\test-outputs\tauri-stage6-read-only-behavior-20260531-a\020-tauri-cli-write-env-zero-tests.log`
- `G:\Dx\test-outputs\tauri-stage6-read-only-behavior-20260531-a\030-tauri-utils-read-only-tests.log`
- `G:\Dx\test-outputs\tauri-stage6-read-only-behavior-20260531-a\031-tauri-utils-cached-project-read-only-test.log`
- `G:\Dx\test-outputs\tauri-stage6-read-only-behavior-20260531-a\040-tauri-cli-write-env-zero-tests-final.log`
- `G:\Dx\test-outputs\tauri-stage6-read-only-behavior-20260531-a\041-tauri-utils-write-env-tests-final.log`
- `G:\Dx\test-outputs\tauri-stage6-read-only-behavior-20260531-a\042-tauri-utils-cached-project-read-only-test-final.log`
- `G:\Dx\test-outputs\tauri-stage6-read-only-behavior-20260531-a\050-green-read-only-source-contract-final.log`
- `G:\Dx\test-outputs\tauri-stage6-read-only-behavior-20260531-a\051-green-surface-matrix-after-read-only-behavior.log`

Highest-priority test gaps before broadening:

- No remaining source/behavior coverage gap blocks the next governed command benchmark. Remaining blockers are benchmark scope, packaging/MSRV, cargo-vet/audit, and release-channel policy.

### Stage 7: Governed Fresh Current-Source Command Benchmark

**Purpose:** Re-run the official/local release-binary command comparison against the current clean git SHA after the safety and coverage hardening checkpoints.

- [x] Add a receipt writer for `local-current-source-build-receipt.json`.
- [x] Run the expected local release build command once and record the build log under `G:\Dx\test-outputs`.
- [x] Pre-generate `.machine` artifacts as setup before timing.
- [x] Prepare official/global/local existing-binary comparison inputs with the fresh local binary and build receipt.
- [x] Run the governed no-build benchmark with writes disabled during timed cache-on samples.
- [x] Run the checker and record only the claims it allows.

Latest Stage 7 receipts:

- `G:\Dx\test-outputs\tauri-stage7-current-source-command-benchmark-20260531-a\010-cargo-build-local-release-dx-machine-cache-mmap.log`
- `G:\Dx\test-outputs\tauri-stage7-current-source-command-benchmark-20260531-a\020-write-local-release-build-receipt.log`
- `G:\Dx\test-outputs\tauri-stage7-current-source-command-benchmark-20260531-a\030-pre-generate-machine-cache-setup.log`
- `G:\Dx\test-outputs\tauri-stage7-current-source-command-benchmark-20260531-a\040-prepare-current-source-comparison.log`
- `G:\Dx\test-outputs\tauri-stage7-current-source-command-benchmark-20260531-a\050-run-current-source-command-benchmark.log`
- `G:\Dx\test-outputs\tauri-stage7-current-source-command-benchmark-20260531-a\060-BRUTAL-CURRENT-SOURCE-RESULT.md`
- `G:\Dx\test-outputs\tauri-stage7-current-source-command-benchmark-20260531-a\benchmark-output\summary.csv`
- `G:\Dx\test-outputs\tauri-stage7-current-source-command-benchmark-20260531-a\benchmark-output-manifest-check\wave7-benchmark-manifest-check.json`

### Stage 8: Governed Second Command Surface

**Purpose:** Broaden command-level evidence without letting benchmark tooling become an arbitrary-command loophole.

- [x] Add a TypeScript allowlist for benchmark surfaces instead of free-form command strings.
- [x] Keep legacy `inspect wix-upgrade-code` evidence valid through the generalized checker.
- [x] Add plan/checker plumbing for the guarded `migrate-stable-v2-noop` surface.
- [x] Prove the runner can plan the real migrate surface with existing binaries and pre-generated `.machine` artifacts.
- [x] Run the actual governed `migrate-stable-v2-noop` timing pass once the benchmark process sweep is clean.
- [x] Keep any `migrate` claim restricted to the fixed stable-v2 no-op fixture and `cargo-workspace.machine`; do not claim generic migration speed.

Latest Stage 8 receipts:

- `G:\Dx\test-outputs\tauri-stage4-comparison-prep-20260530-a\runner-plan-migrate-surface\wave12-run-plan.json`
- `G:\Dx\test-outputs\tauri-stage8-migrate-command-benchmark-20260531-a\runner-plan-only\wave12-run-plan.json`
- `G:\Dx\test-outputs\tauri-stage8-migrate-command-benchmark-20260531-a\benchmark-output\wave7-process-check-preflight.log`
- `G:\Dx\test-outputs\tauri-stage8-migrate-command-benchmark-20260531-a\benchmark-output-clean\wave7-process-check-preflight.log`
- `G:\Dx\test-outputs\tauri-stage8-migrate-command-benchmark-20260531-a\benchmark-output-final\wave7-process-check-preflight.log`
- `G:\Dx\test-outputs\tauri-stage8-migrate-command-benchmark-20260531-a\benchmark-output-final-retry1\summary.csv`
- `G:\Dx\test-outputs\tauri-stage8-migrate-command-benchmark-20260531-a\benchmark-output-final-retry1\wave7-process-check-final.log`
- `G:\Dx\test-outputs\tauri-stage8-migrate-command-benchmark-20260531-a\benchmark-output-final-retry1-manifest-check\wave7-benchmark-manifest-check.json`
- `G:\Dx\test-outputs\tauri-stage8-migrate-command-benchmark-20260531-a\benchmark-output-final-retry2\summary.csv`
- `G:\Dx\test-outputs\tauri-stage8-migrate-command-benchmark-20260531-a\benchmark-output-final-retry2\wave7-process-check-final.log`
- `G:\Dx\test-outputs\tauri-stage8-migrate-command-benchmark-20260531-a\benchmark-output-final-retry2-manifest-check\wave7-benchmark-manifest-check.json`
- `G:\Dx\test-outputs\tauri-stage8-regression-check-20260531-a\stage7-inspect-manifest-recheck\wave7-benchmark-manifest-check.json`
- `G:\Dx\test-outputs\tauri-stage8-regression-check-20260531-b\stage7-inspect-manifest-recheck\wave7-benchmark-manifest-check.json`

Stage 8 follow-up:

- Stage 8's same-binary migrate proof was superseded by Stage 9's cleaner current-HEAD receipt-backed migrate proof. The rejected retry's median numbers were local cache-on `29.637 ms`, local cache-off `92.256 ms`, and official release `101.966 ms`, but those numbers remain diagnostic only and must not be used as a speed claim.

### Stage 9: Current-HEAD Receipt-Backed Command Evidence

**Purpose:** Rebuild the local release binary after the benchmark-tooling commit and make current-source official comparisons SHA-bound again.

- [x] Wait for the unrelated `G:\Dx\bun` test/release-proof processes to clear before starting the release build.
- [x] Build `cargo-tauri` release for the committed current HEAD with `dx-machine-cache-mmap`.
- [x] Write a clean `local-current-source-build-receipt.json` under `G:\Dx\test-outputs`.
- [x] Prepare a fresh comparison plan with updated binary hashes and the new receipt.
- [x] Run receipt-backed `inspect wix-upgrade-code` benchmark and checker.
- [x] Run receipt-backed `migrate-stable-v2-noop` benchmark and checker.
- [x] Investigate why the latest `inspect wix-upgrade-code` cache-on median was slower than cache-off while still beating official release.

Latest Stage 9 receipts:

- `G:\Dx\test-outputs\tauri-stage9-current-head-release-20260531-a\010-cargo-build-release.log`
- `G:\Dx\test-outputs\tauri-stage9-current-head-release-20260531-a\local-current-source-build-receipt.json`
- `G:\Dx\test-outputs\tauri-stage9-current-head-release-20260531-a\comparison-plan\stage4-config-receipt.json`
- `G:\Dx\test-outputs\tauri-stage9-current-head-release-20260531-a\comparison-plan\binaries.csv`
- `G:\Dx\test-outputs\tauri-stage9-current-head-release-20260531-a\benchmark-inspect\summary.csv`
- `G:\Dx\test-outputs\tauri-stage9-current-head-release-20260531-a\benchmark-inspect-manifest-check\wave7-benchmark-manifest-check.json`
- `G:\Dx\test-outputs\tauri-stage9-current-head-release-20260531-a\benchmark-migrate\summary.csv`
- `G:\Dx\test-outputs\tauri-stage9-current-head-release-20260531-a\benchmark-migrate-manifest-check\wave7-benchmark-manifest-check.json`

### Stage 10: Inspect Schema-Config Projection Skip

**Purpose:** Remove the avoidable JSON/JSON5 projection cache read that made the Stage 9 `inspect wix-upgrade-code` cache-on path slower than cache-off on the same local binary.

- [x] Reproduce the regression in Stage 9 evidence: `inspect` cache-on `66.013 ms`, cache-off `58.017 ms`.
- [x] Trace the root cause to the projection path reading `project-config.machine` for JSON, rejecting it for schema validation, then reading the full project-config cache.
- [x] Add a focused red/green test for schema-validated base config format detection.
- [x] Skip direct project-config projection for JSON/JSON5 before touching the projection `.machine` file.
- [x] Run focused Rust tests/checks for the inspect slice.
- [x] Commit the fix as `af391adbd perf(cli): skip unused inspect projection for schema configs`.
- [x] Build a fresh release binary for git `af391adbd1e0fbc07ca4e84f3c77dc62abb2c695` and write a clean build receipt.
- [x] Rerun receipt-backed `inspect wix-upgrade-code` benchmark and checker.
- [x] Rerun receipt-backed `migrate-stable-v2-noop` benchmark and checker for the same git `af391adbd1e0fbc07ca4e84f3c77dc62abb2c695` binary.

Latest Stage 10 receipts:

- `G:\Dx\test-outputs\tauri-stage10-inspect-projection-skip-20260531-a\010-cargo-build-release.log`
- `G:\Dx\test-outputs\tauri-stage10-inspect-projection-skip-20260531-a\local-current-source-build-receipt.json`
- `G:\Dx\test-outputs\tauri-stage10-inspect-projection-skip-20260531-a\comparison-plan\stage4-config-receipt.json`
- `G:\Dx\test-outputs\tauri-stage10-inspect-projection-skip-20260531-a\comparison-plan\binaries.csv`
- `G:\Dx\test-outputs\tauri-stage10-inspect-projection-skip-20260531-a\benchmark-inspect-retry1\summary.csv`
- `G:\Dx\test-outputs\tauri-stage10-inspect-projection-skip-20260531-a\benchmark-inspect-retry1-manifest-check\wave7-benchmark-manifest-check.json`
- `G:\Dx\test-outputs\tauri-stage10-inspect-projection-skip-20260531-a\benchmark-migrate\summary.csv`
- `G:\Dx\test-outputs\tauri-stage10-inspect-projection-skip-20260531-a\benchmark-migrate-manifest-check\wave7-benchmark-manifest-check.json`

### Stage 11: Feature-Gated Developer Docs

**Purpose:** Make the local DX serializer cache behavior understandable and auditable for maintainers without turning narrow benchmark evidence into broad product claims.

- [x] Add a TypeScript docs contract that fails if the developer docs omit cache gates, feature gates, pre-generation semantics, source authority, or claim limits.
- [x] Document `TAURI_DX_MACHINE_CACHE`, `TAURI_DX_MACHINE_CACHE_WRITE`, `dx-machine-cache`, and `dx-machine-cache-mmap` in the CLI environment variable reference.
- [x] State that the cache is default-off and source files remain authoritative.
- [x] State that benchmark timing uses pre-generated `.machine` artifacts and must not count generation time.
- [x] State that stale, corrupt, oversized, unsupported, or unsafe cache state falls back to normal Tauri parsing/probing.
- [x] Keep docs honest: current evidence covers selected repeated CLI/config hot paths only, not broad Tauri CLI, app runtime, WebView, IPC, build, bundle, or installer speedups.

Latest Stage 11 receipts:

- `G:\Dx\test-outputs\tauri-stage11-docs-hardening-20260531-a\010-red-machine-cache-docs.log`
- `G:\Dx\test-outputs\tauri-stage11-docs-hardening-20260531-a\020-green-machine-cache-docs.log`
- `G:\Dx\test-outputs\tauri-stage11-docs-hardening-20260531-a\030-node-check-docs-test.log`
- `G:\Dx\test-outputs\tauri-stage11-docs-hardening-20260531-a\040-surface-matrix-after-docs.log`
- `G:\Dx\test-outputs\tauri-stage11-docs-hardening-20260531-a\050-node-check-docs-test-final-rerun.log`
- `G:\Dx\test-outputs\tauri-stage11-docs-hardening-20260531-a\060-green-machine-cache-docs-final-rerun.log`
- `G:\Dx\test-outputs\tauri-stage11-docs-hardening-20260531-a\070-surface-matrix-after-docs-final-rerun.log`

### Stage 12: Benchmark Write-Disable Governance

**Purpose:** Ensure future command benchmarks compare normal source parsing/probing against pre-generated `.machine` reads only, with no measured command allowed to create or refresh machine sidecars.

- [x] Add a failing TypeScript contract showing that command benchmark schema v2 must require `TAURI_DX_MACHINE_CACHE_WRITE=0` for every measured sample.
- [x] Update the no-build benchmark runner to emit command-benchmark schema v2.
- [x] Set `TAURI_DX_MACHINE_CACHE_WRITE=0` for cache-on, cache-off, and unset measured cases.
- [x] Record `machine_cache_write_env_for_all_samples = "0"` in benchmark meta and run provenance.
- [x] Update the checker to preserve legacy v1 receipt compatibility while enforcing the stricter v2 write-disable policy.
- [x] Add a negative fixture proving the checker rejects a v2 sample with `<unset>` write env.

Latest Stage 12 receipts:

- `G:\Dx\test-outputs\tauri-stage12-benchmark-write-env-hardening-20260531-a\010-red-global-write-env-contract.log`
- `G:\Dx\test-outputs\tauri-stage12-benchmark-write-env-hardening-20260531-a\020-red-v2-global-write-env-contract.log`
- `G:\Dx\test-outputs\tauri-stage12-benchmark-write-env-hardening-20260531-a\030-node-check-runner.log`
- `G:\Dx\test-outputs\tauri-stage12-benchmark-write-env-hardening-20260531-a\040-node-check-checker.log`
- `G:\Dx\test-outputs\tauri-stage12-benchmark-write-env-hardening-20260531-a\050-node-check-sample-invocation-test.log`
- `G:\Dx\test-outputs\tauri-stage12-benchmark-write-env-hardening-20260531-a\060-green-global-write-env-contract.log`
- `G:\Dx\test-outputs\tauri-stage12-benchmark-write-env-hardening-20260531-a\070-node-check-runner-final.log`
- `G:\Dx\test-outputs\tauri-stage12-benchmark-write-env-hardening-20260531-a\080-node-check-checker-final.log`
- `G:\Dx\test-outputs\tauri-stage12-benchmark-write-env-hardening-20260531-a\090-node-check-sample-invocation-test-final.log`
- `G:\Dx\test-outputs\tauri-stage12-benchmark-write-env-hardening-20260531-a\100-surface-matrix-after-write-env.log`
- `G:\Dx\test-outputs\tauri-stage12-benchmark-write-env-hardening-20260531-a\110-green-global-write-env-contract-final.log`
- `G:\Dx\test-outputs\tauri-stage12-benchmark-write-env-hardening-20260531-a\120-legacy-stage10-inspect-v1-check.log`
- `G:\Dx\test-outputs\tauri-stage12-benchmark-write-env-hardening-20260531-a\130-git-diff-check.log`

### Stage 13: Project-Config Projection Validation Cleanup

**Purpose:** Reduce overhead inside the proven project-config projection cache hit path without changing config behavior or authority boundaries.

- [x] Add a TypeScript source guard that fails unless project-config projection performs one archived JSON tree validation before per-path lookups.
- [x] Keep the checked projection helper available for callers that have not already validated the archived tree.
- [x] Add a private unchecked per-path projection helper for callers that have already validated the tree.
- [x] Route `project_config_projection_from_archive` through the unchecked helper after envelope, source-set, and tree validation.
- [x] Run focused source, format, Rust projection, and tauri-utils machine-cache checks.

Latest Stage 13 receipts:

- `G:\Dx\test-outputs\tauri-stage13-projection-validation-20260531-a\010-red-projection-validation-guard.log`
- `G:\Dx\test-outputs\tauri-stage13-projection-validation-20260531-a\020-node-check-projection-validation-guard.log`
- `G:\Dx\test-outputs\tauri-stage13-projection-validation-20260531-a\030-green-projection-validation-guard.log`
- `G:\Dx\test-outputs\tauri-stage13-projection-validation-20260531-a\040-rustfmt-machine-cache-check.log`
- `G:\Dx\test-outputs\tauri-stage13-projection-validation-20260531-a\050-cargo-test-project-config-projection.log`
- `G:\Dx\test-outputs\tauri-stage13-projection-validation-20260531-a\060-cargo-check-tauri-utils-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage13-projection-validation-20260531-a\070-green-projection-validation-guard-final.log`
- `G:\Dx\test-outputs\tauri-stage13-projection-validation-20260531-a\080-git-diff-check.log`

### Stage 14: Cache-Miss Read Order Cleanup

**Purpose:** Avoid source fingerprint work when a config `.machine` cache file is missing, not a file, or already over the bounded cache-size limit.

- [x] Add a TypeScript source-order guard for the JSON value, merged project-config, and project-config projection read paths.
- [x] Add a shared `machine_cache_file_is_candidate` helper.
- [x] Check candidate file existence/type/size before hashing the authoritative source file.
- [x] Keep mmap and bounded read validation in place after the source fingerprint is available.
- [x] Run focused source, format, Rust project-config, and tauri-utils machine-cache checks.

Latest Stage 14 receipts:

- `G:\Dx\test-outputs\tauri-stage14-cache-miss-order-20260531-a\010-red-read-order-guard.log`
- `G:\Dx\test-outputs\tauri-stage14-cache-miss-order-20260531-a\020-node-check-read-order-guard.log`
- `G:\Dx\test-outputs\tauri-stage14-cache-miss-order-20260531-a\030-green-read-order-guard.log`
- `G:\Dx\test-outputs\tauri-stage14-cache-miss-order-20260531-a\040-rustfmt-machine-cache-check.log`
- `G:\Dx\test-outputs\tauri-stage14-cache-miss-order-20260531-a\050-green-read-order-guard-after-format.log`
- `G:\Dx\test-outputs\tauri-stage14-cache-miss-order-20260531-a\060-rustfmt-machine-cache-check-after-format.log`
- `G:\Dx\test-outputs\tauri-stage14-cache-miss-order-20260531-a\070-node-check-read-order-guard-after-format.log`
- `G:\Dx\test-outputs\tauri-stage14-cache-miss-order-20260531-a\080-cargo-test-project-config-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage14-cache-miss-order-20260531-a\090-cargo-check-tauri-utils-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage14-cache-miss-order-20260531-a\100-git-diff-check.log`

### Stage 15: CLI Source Matching Cleanup

**Purpose:** Remove avoidable O(n²) source-validation scans from cache-hit paths whose source sets can grow with workspace size.

- [x] Add a TypeScript source guard covering full Cargo metadata, workspace discovery, Cargo config, and package manifest/lock cache validators.
- [x] Keep helpers local to each module instead of introducing a cross-module trait across private fingerprint structs.
- [x] Replace nested `cache.sources` by `snapshot.sources` scans with path-indexed `HashMap` lookup.
- [x] Preserve existing equality semantics: path identity plus the existing `matches` behavior, which ignores mtime-only drift and checks presence, byte length, and hash.
- [x] Run focused source, format, CLI cache behavior, and CLI machine-cache checks.

Latest Stage 15 receipts:

- `G:\Dx\test-outputs\tauri-stage15-source-matching-20260531-a\010-red-source-matching-guard.log`
- `G:\Dx\test-outputs\tauri-stage15-source-matching-20260531-a\020-node-check-source-matching-guard.log`
- `G:\Dx\test-outputs\tauri-stage15-source-matching-20260531-a\030-green-source-matching-guard.log`
- `G:\Dx\test-outputs\tauri-stage15-source-matching-20260531-a\040-rustfmt-cli-source-matching-check.log`
- `G:\Dx\test-outputs\tauri-stage15-source-matching-20260531-a\050-cargo-test-full-cargo-metadata-cache.log`
- `G:\Dx\test-outputs\tauri-stage15-source-matching-20260531-a\060-cargo-test-workspace-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage15-source-matching-20260531-a\070-cargo-test-cargo-config-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage15-source-matching-20260531-a\080-cargo-test-cargo-package-metadata-cache.log`
- `G:\Dx\test-outputs\tauri-stage15-source-matching-20260531-a\090-cargo-check-tauri-cli-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage15-source-matching-20260531-a\100-green-source-matching-guard-final.log`
- `G:\Dx\test-outputs\tauri-stage15-source-matching-20260531-a\110-surface-matrix-after-source-matching.log`
- `G:\Dx\test-outputs\tauri-stage15-source-matching-20260531-a\120-git-diff-check.log`

### Stage 16: Benchmark Integrity Receipt Honesty

**Purpose:** Ensure failed v2 sample-environment checks cannot still appear verified in the machine-readable governance receipt.

- [x] Add a regression assertion to the existing v2 write-env negative fixture.
- [x] Mark sample invocation verification false when `TAURI_DX_MACHINE_CACHE` or `TAURI_DX_MACHINE_CACHE_WRITE` sample env checks fail.
- [x] Require zero checker failures for `benchmark_integrity_verified`.
- [x] Run focused TypeScript syntax and governance regression checks.

Latest Stage 16 receipts:

- `G:\Dx\test-outputs\tauri-stage16-governance-integrity-20260531-a\010-red-write-env-integrity-overclaim.log`
- `G:\Dx\test-outputs\tauri-stage16-governance-integrity-20260531-a\020-node-check-checker.log`
- `G:\Dx\test-outputs\tauri-stage16-governance-integrity-20260531-a\030-node-check-sample-invocation-test.log`
- `G:\Dx\test-outputs\tauri-stage16-governance-integrity-20260531-a\040-green-write-env-integrity-overclaim.log`
- `G:\Dx\test-outputs\tauri-stage16-governance-integrity-20260531-a\050-green-write-env-integrity-final.log`
- `G:\Dx\test-outputs\tauri-stage16-governance-integrity-20260531-a\060-git-diff-check.log`

### Stage 17: Projection Helper Warning Cleanup

**Purpose:** Keep the Stage 13 projection cleanup warning-free under normal tauri-utils checks.

- [x] Update the TypeScript projection source guard so it requires the single-validation unchecked path and rejects the dead checked helper.
- [x] Remove the private checked projection helper that no longer had a call site.
- [x] Run focused source, format, Rust projection, and tauri-utils machine-cache checks.

Latest Stage 17 receipts:

- `G:\Dx\test-outputs\tauri-stage17-dead-projection-helper-20260531-a\010-red-dead-projection-helper-guard.log`
- `G:\Dx\test-outputs\tauri-stage17-dead-projection-helper-20260531-a\020-node-check-projection-guard.log`
- `G:\Dx\test-outputs\tauri-stage17-dead-projection-helper-20260531-a\030-green-dead-projection-helper-guard.log`
- `G:\Dx\test-outputs\tauri-stage17-dead-projection-helper-20260531-a\040-rustfmt-machine-cache-check.log`
- `G:\Dx\test-outputs\tauri-stage17-dead-projection-helper-20260531-a\050-cargo-test-project-config-projection.log`
- `G:\Dx\test-outputs\tauri-stage17-dead-projection-helper-20260531-a\060-cargo-check-tauri-utils-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage17-dead-projection-helper-20260531-a\070-git-diff-check.log`

### Stage 18: Benchmark Argv And Case Allowlist Governance

**Purpose:** Make future command-benchmark v2 receipts prove the exact spawned argv shape and the reviewed executable/script case shape before any timing claim can pass.

- [x] Add an executable/script case policy registry for current benchmark targets.
- [x] Have the no-build benchmark runner apply policy defaults for old prepared rows, validate every case before plan/run output, and preserve `role`, `source_kind`, and `invocation_kind` in `binaries.csv`.
- [x] Have the runner emit `argv_json` beside the human-readable `args` string for each sample.
- [x] Make the checker require and validate `argv_json` plus case metadata for command-benchmark v2 receipts while keeping legacy v1 receipts checkable.
- [x] Fix the late v2 run-provenance overclaim so a v2-only provenance failure cannot leave `run_provenance_verified` true.
- [x] Run focused TypeScript syntax checks, governance green/red checks, comparison-prep compatibility, legacy Stage 10 receipt rechecks, surface-matrix/source-matching guards, and `git diff --check`.

Latest Stage 18 receipts:

- `G:\Dx\test-outputs\tauri-stage18-benchmark-argv-allowlist-20260531-a\020-node-check-dx-benchmark-command-surfaces.log`
- `G:\Dx\test-outputs\tauri-stage18-benchmark-argv-allowlist-20260531-a\021-node-check-run-dx-no-build-benchmark.log`
- `G:\Dx\test-outputs\tauri-stage18-benchmark-argv-allowlist-20260531-a\022-node-check-check-dx-benchmark-manifest.log`
- `G:\Dx\test-outputs\tauri-stage18-benchmark-argv-allowlist-20260531-a\023-node-check-test-dx-benchmark-manifest-sample-invocation.log`
- `G:\Dx\test-outputs\tauri-stage18-benchmark-argv-allowlist-20260531-a\030-test-dx-benchmark-manifest-sample-invocation.log`
- `G:\Dx\test-outputs\tauri-stage18-benchmark-argv-allowlist-20260531-a\031-test-dx-current-source-comparison-prep.log`
- `G:\Dx\test-outputs\tauri-stage18-benchmark-argv-allowlist-20260531-a\032-stage10-inspect-v1-recheck.log`
- `G:\Dx\test-outputs\tauri-stage18-benchmark-argv-allowlist-20260531-a\033-stage10-migrate-v1-recheck.log`
- `G:\Dx\test-outputs\tauri-stage18-benchmark-argv-allowlist-20260531-a\034-red-green-receipt-spotcheck.log`
- `G:\Dx\test-outputs\tauri-stage18-benchmark-argv-allowlist-20260531-a\035-git-diff-check.log`
- `G:\Dx\test-outputs\tauri-stage18-benchmark-argv-allowlist-20260531-a\036-test-dx-machine-cache-surface-matrix.log`
- `G:\Dx\test-outputs\tauri-stage18-benchmark-argv-allowlist-20260531-a\037-test-dx-machine-cache-source-matching.log`
- `G:\Dx\test-outputs\tauri-stage18-benchmark-argv-allowlist-20260531-a\038-repeat-test-dx-benchmark-manifest-sample-invocation.log`

### Stage 19: Structured Process Sweep Receipts

**Purpose:** Replace text-marker-only heavy-process checks for command-benchmark v2 with machine-readable process-sweep receipts that can block contaminated timing evidence.

- [x] Add preflight/final process-sweep JSON receipt emission to the no-build benchmark runner.
- [x] Include roots, watched command names, self PID, clean status, matched process list, text-log filename, and text-log hash in each receipt.
- [x] Record JSON receipt paths in run provenance for command-benchmark v2.
- [x] Make the checker require and validate process-sweep JSON for command-benchmark v2 while keeping legacy v1 text-log receipts checkable.
- [x] Add red fixtures for `clean: true` with matched processes and missing process-sweep JSON.
- [x] Run focused TypeScript syntax checks, governance green/red checks, comparison-prep compatibility, legacy Stage 10 receipt rechecks, spot checks, and `git diff --check`.

Latest Stage 19 receipts:

- `G:\Dx\test-outputs\tauri-stage19-process-sweep-json-20260531-a\010-node-check-runner.log`
- `G:\Dx\test-outputs\tauri-stage19-process-sweep-json-20260531-a\011-node-check-checker.log`
- `G:\Dx\test-outputs\tauri-stage19-process-sweep-json-20260531-a\012-node-check-sample-invocation-test.log`
- `G:\Dx\test-outputs\tauri-stage19-process-sweep-json-20260531-a\020-test-dx-benchmark-manifest-sample-invocation.log`
- `G:\Dx\test-outputs\tauri-stage19-process-sweep-json-20260531-a\021-test-dx-current-source-comparison-prep.log`
- `G:\Dx\test-outputs\tauri-stage19-process-sweep-json-20260531-a\022-stage10-inspect-v1-recheck.log`
- `G:\Dx\test-outputs\tauri-stage19-process-sweep-json-20260531-a\023-stage10-migrate-v1-recheck.log`
- `G:\Dx\test-outputs\tauri-stage19-process-sweep-json-20260531-a\024-process-sweep-red-green-spotcheck.log`
- `G:\Dx\test-outputs\tauri-stage19-process-sweep-json-20260531-a\025-git-diff-check.log`

### Stage 20: Dirty Git Benchmark Gate

**Purpose:** Prevent command-benchmark v2 timing evidence from passing when the measured run was captured from a dirty source checkout.

- [x] Add a runner-side clean-git check for non-plan benchmark executions before any warmups or timed samples run.
- [x] Reuse the verified git metadata in `run-provenance.json` instead of taking a second git snapshot after timing.
- [x] Make the checker reject command-benchmark v2 receipts whose run provenance has `git.dirty !== false`.
- [x] Make the checker reject command-benchmark v2 receipts whose run provenance has missing or non-empty `git.status_short`.
- [x] Add a dirty-run-provenance red fixture and verify `run_provenance_verified=false` plus `benchmark_integrity_verified=false`.
- [x] Run focused TypeScript syntax checks, governance green/red checks, comparison-prep compatibility, legacy Stage 10 receipt rechecks, and dirty-git spot checks.

Latest Stage 20 receipts:

- `G:\Dx\test-outputs\tauri-stage20-dirty-git-gate-20260531-a\010-node-check-runner.log`
- `G:\Dx\test-outputs\tauri-stage20-dirty-git-gate-20260531-a\011-node-check-checker.log`
- `G:\Dx\test-outputs\tauri-stage20-dirty-git-gate-20260531-a\012-node-check-sample-invocation-test.log`
- `G:\Dx\test-outputs\tauri-stage20-dirty-git-gate-20260531-a\020-test-dx-benchmark-manifest-sample-invocation.log`
- `G:\Dx\test-outputs\tauri-stage20-dirty-git-gate-20260531-a\021-test-dx-current-source-comparison-prep.log`
- `G:\Dx\test-outputs\tauri-stage20-dirty-git-gate-20260531-a\022-stage10-inspect-v1-recheck.log`
- `G:\Dx\test-outputs\tauri-stage20-dirty-git-gate-20260531-a\023-stage10-migrate-v1-recheck.log`
- `G:\Dx\test-outputs\tauri-stage20-dirty-git-gate-20260531-a\024-dirty-git-red-green-spotcheck.log`

### Stage 21: CLI Bounded Machine Reads

**Purpose:** Close the remaining CLI metadata/read race where a `.machine` sidecar could grow after size preflight and before fallback allocation.

- [x] Add a shared `helpers::machine_cache_io` helper with `machine_cache_file_is_candidate` and `read_machine_file_bounded`.
- [x] Cover exact-limit, over-limit, and non-file candidate behavior with targeted Rust unit tests.
- [x] Route Cargo package metadata, package version, Cargo config, Cargo settings, full Cargo metadata, and workspace discovery fallback reads through the bounded helper.
- [x] Keep mmap/open fast paths ahead of bounded fallback reads.
- [x] Update mmap and surface-matrix guards so future readers cannot silently reintroduce direct unbounded `fs::read(&paths.machine)` fallback reads.
- [x] Run focused Rust formatting, Rust unit tests, mmap coverage, source-matching guard, surface-matrix guard, and no-unbounded-read search.

Latest Stage 21 receipts:

- `G:\Dx\test-outputs\tauri-stage21-cli-bounded-machine-reads-20260531-a\010-rustfmt-touched-files.log`
- `G:\Dx\test-outputs\tauri-stage21-cli-bounded-machine-reads-20260531-a\011-rustfmt-check-touched-files.log`
- `G:\Dx\test-outputs\tauri-stage21-cli-bounded-machine-reads-20260531-a\012-node-check-mmap-coverage.log`
- `G:\Dx\test-outputs\tauri-stage21-cli-bounded-machine-reads-20260531-a\013-test-dx-machine-cache-mmap-coverage.log`
- `G:\Dx\test-outputs\tauri-stage21-cli-bounded-machine-reads-20260531-a\014-test-dx-machine-cache-source-matching.log`
- `G:\Dx\test-outputs\tauri-stage21-cli-bounded-machine-reads-20260531-a\020-cargo-test-machine-cache-io.log`
- `G:\Dx\test-outputs\tauri-stage21-cli-bounded-machine-reads-20260531-a\022-cargo-test-machine-cache-io-after-warning-fix.log`
- `G:\Dx\test-outputs\tauri-stage21-cli-bounded-machine-reads-20260531-a\023-rustfmt-check-final.log`
- `G:\Dx\test-outputs\tauri-stage21-cli-bounded-machine-reads-20260531-a\024-test-dx-machine-cache-mmap-coverage-final.log`
- `G:\Dx\test-outputs\tauri-stage21-cli-bounded-machine-reads-20260531-a\027-test-dx-machine-cache-surface-matrix-after-fix.log`
- `G:\Dx\test-outputs\tauri-stage21-cli-bounded-machine-reads-20260531-a\028-cargo-test-machine-cache-io-final.log`
- `G:\Dx\test-outputs\tauri-stage21-cli-bounded-machine-reads-20260531-a\030-test-dx-machine-cache-surface-matrix-final.log`
- `G:\Dx\test-outputs\tauri-stage21-cli-bounded-machine-reads-20260531-a\031-test-dx-machine-cache-mmap-coverage-after-matrix-data.log`
- `G:\Dx\test-outputs\tauri-stage21-cli-bounded-machine-reads-20260531-a\032-rg-no-unbounded-paths-machine.log`
- `G:\Dx\test-outputs\tauri-stage21-cli-bounded-machine-reads-20260531-a\033-git-diff-check.log`

### Stage 22: CLI Source-Set Equality

**Purpose:** Ensure indexed source-fingerprint validators compare unique source path sets, not just same-length lists with membership-style lookups.

- [x] Tighten the shared matcher shape in full Cargo metadata, workspace discovery, Cargo config, and package manifest/lock validators.
- [x] Reject duplicate current source paths and duplicate expected cached source paths before fingerprint comparison.
- [x] Add a package manifest/lock regression test where a duplicate cached source replaces another expected source at the same list length.
- [x] Strengthen the TypeScript source-matching guard so it requires `HashMap` construction plus duplicate-count guards.
- [x] Run focused Rust formatting, TypeScript guard checks, the source-matching guard, the duplicate cached source Rust test, and `git diff --check`.

Latest Stage 22 receipts:

- `G:\Dx\test-outputs\tauri-stage22-source-set-equality-20260531-a\010-rustfmt-source-equality-files.log`
- `G:\Dx\test-outputs\tauri-stage22-source-set-equality-20260531-a\011-node-check-source-matching.log`
- `G:\Dx\test-outputs\tauri-stage22-source-set-equality-20260531-a\012-test-dx-machine-cache-source-matching.log`
- `G:\Dx\test-outputs\tauri-stage22-source-set-equality-20260531-a\020-cargo-test-duplicate-cached-sources.log`
- `G:\Dx\test-outputs\tauri-stage22-source-set-equality-20260531-a\021-rustfmt-check-source-equality-files.log`
- `G:\Dx\test-outputs\tauri-stage22-source-set-equality-20260531-a\022-git-diff-check.log`

### Stage 23: Build Log Receipt Validation

**Purpose:** Ensure current-source speed claims are tied to a real completed release build log, not only to a JSON receipt that asserts a build happened.

- [x] Add red fixtures for missing build-log evidence, stale build-log hash identity, and weak build-log content.
- [x] Make the benchmark checker require `local-current-source-build-receipt.json.build_log`.
- [x] Require the build-log path to stay under `G:\Dx\test-outputs`.
- [x] Recompute build-log bytes and SHA-256 from disk and reject mismatched receipt identity.
- [x] Require build-log text to prove a completed `tauri-cli` release build through the release-profile completion line plus either the expected cargo build command or `tauri-cli` compile evidence.
- [x] Reject build logs that contain failure output even if they also contain a release-finished line.
- [x] Strengthen the receipt writer's build-log preflight to reject weak logs before writing a receipt.
- [x] Copy build logs beside copied local-current-source build receipts in comparison prep and no-build runner outputs, then rewrite the receipt's `build_log` file record to the local copy.
- [x] Recheck the Stage 10 `inspect wix-upgrade-code` and `migrate-stable-v2-noop` claim-source receipts under the stronger rule.

Latest Stage 23 receipts:

- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\010-node-check-sample-invocation-test-red.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\011-red-build-log-validation-contract.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\020-node-check-checker.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\021-node-check-build-receipt-writer.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\022-node-check-sample-invocation-test.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\030-test-dx-benchmark-manifest-sample-invocation.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\031-stage10-inspect-recheck.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\032-stage10-migrate-recheck.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\033-build-log-red-green-spotcheck.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\034-writer-rejects-weak-build-log.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\034-writer-rejects-weak-build-log-exit.txt`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\035-red-failed-build-log-contract.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\040-node-check-checker-after-failure-output.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\041-node-check-build-receipt-writer-after-failure-output.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\042-node-check-sample-invocation-test-after-failure-output.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\043-node-check-prepare-current-source-comparison.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\044-node-check-runner.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\045-node-check-current-source-comparison-prep-test.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\046-node-check-sample-invocation-test-final.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\050-test-dx-benchmark-manifest-sample-invocation-final.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\051-test-dx-current-source-comparison-prep-final.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\052-stage10-inspect-recheck-final.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\053-stage10-migrate-recheck-final.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\054-writer-rejects-failed-build-log.log`
- `G:\Dx\test-outputs\tauri-stage23-build-log-validation-20260531-a\054-writer-rejects-failed-build-log-exit.txt`

### Stage 24: Claim Boolean Fail-Closed Governance

**Purpose:** Keep machine-readable governance booleans honest when a failed receipt contains partial evidence that could otherwise be over-read by scripts or dashboards.

- [x] Add red fixtures for timed-out samples, stale output files, and missing summary rows.
- [x] Make `sample_invocations_verified` false for sample execution failures, including timeouts, signals, spawn errors, invalid elapsed values, non-zero exits, wrong phase, wrong sample count, or bad iteration shape.
- [x] Make `output_equivalence_verified` false when per-sample output files fail rehash validation.
- [x] Make `summary_recomputed_from_samples` false for missing `summary.csv` rows as well as numeric summary mismatches.
- [x] Make `official_release_snapshot_allowed` false when the overall receipt has failures, even if the official release binary hash itself is present.
- [x] Recheck the Stage 10 `inspect wix-upgrade-code` and `migrate-stable-v2-noop` claim-source receipts under the stronger boolean rules.

Latest Stage 24 receipts:

- `G:\Dx\test-outputs\tauri-stage24-claim-booleans-20260531-a\010-red-claim-boolean-contract.log`
- `G:\Dx\test-outputs\tauri-stage24-claim-booleans-20260531-a\041-node-check-checker-final.log`
- `G:\Dx\test-outputs\tauri-stage24-claim-booleans-20260531-a\042-node-check-sample-invocation-test-final.log`
- `G:\Dx\test-outputs\tauri-stage24-claim-booleans-20260531-a\050-test-dx-benchmark-manifest-sample-invocation-final.log`
- `G:\Dx\test-outputs\tauri-stage24-claim-booleans-20260531-a\051-stage10-inspect-recheck-final.log`
- `G:\Dx\test-outputs\tauri-stage24-claim-booleans-20260531-a\052-stage10-migrate-recheck-final.log`
- `G:\Dx\test-outputs\tauri-stage24-claim-booleans-20260531-a\060-git-diff-check-final.log`

### Stage 25: Manual Receipt Output Path Guard

**Purpose:** Ensure ignored Rust receipt generators cannot accidentally write timing evidence outside `G:\Dx\test-outputs` through parent traversal, sibling-prefix paths, or existing ancestor escapes.

- [x] Add a focused red Rust test for `G:\Dx\test-outputs\..\test-outputs-bad`.
- [x] Route both the ignored manual receipt test and its receipt writer through a single output-dir guard.
- [x] Reject non-absolute paths, parent-directory components, sibling prefixes, and existing ancestor escapes before accepting paths under canonical `G:\Dx\test-outputs`.
- [x] Add a sibling-prefix assertion for `G:\Dx\test-outputs-bad`.

Latest Stage 25 receipts:

- `G:\Dx\test-outputs\tauri-stage25-receipt-path-guard-20260531-a\010-red-receipt-output-parent-traversal.log`
- `G:\Dx\test-outputs\tauri-stage25-receipt-path-guard-20260531-a\040-rustfmt-check-machine-cache-final.log`
- `G:\Dx\test-outputs\tauri-stage25-receipt-path-guard-20260531-a\041-test-receipt-output-parent-traversal-final.log`

### Stage 26: Workspace Cached-Source Preflight

**Purpose:** Prevent poisoned `cargo-workspace.machine` payloads from making the reader fingerprint implausible source paths before it can reject the cache.

- [x] Add a focused red Rust test for implausible workspace cached source paths.
- [x] Cap cached workspace source path count before reconstructing current fingerprints.
- [x] Require cached workspace roots to be absolute, parent-component-free, and ancestors of the app Tauri directory.
- [x] Require cached workspace source paths to be absolute, parent-component-free `Cargo.toml` paths from the workspace root or app ancestry.
- [x] Keep the existing source fingerprint equality check as the final cache authority after preflight passes.

Latest Stage 26 receipts:

- `G:\Dx\test-outputs\tauri-stage26-workspace-source-preflight-20260531-a\010-red-workspace-source-preflight.log`
- `G:\Dx\test-outputs\tauri-stage26-workspace-source-preflight-20260531-a\020-rustfmt-check-workspace-source-preflight.log`
- `G:\Dx\test-outputs\tauri-stage26-workspace-source-preflight-20260531-a\021-test-workspace-source-preflight.log`

### Stage 27: Cargo Config Cached-Source Preflight

**Purpose:** Prevent poisoned `cargo-config.machine` payloads from making the reader fingerprint implausible config paths before it can reject the cache.

- [x] Add a focused red Rust test for implausible Cargo config cached source paths.
- [x] Cap cached Cargo config source path count before reconstructing current fingerprints.
- [x] Require cached config source paths to be absolute and parent-component-free.
- [x] Allow only `.cargo/config`, `.cargo/config.toml`, or current `CARGO_HOME` config slots before fingerprinting.
- [x] Keep the existing ambiguous config-pair and source fingerprint equality checks after preflight passes.

Latest Stage 27 receipts:

- `G:\Dx\test-outputs\tauri-stage27-cargo-config-source-preflight-20260531-a\010-red-cargo-config-source-preflight.log`
- `G:\Dx\test-outputs\tauri-stage27-cargo-config-source-preflight-20260531-a\020-rustfmt-check-cargo-config-source-preflight.log`
- `G:\Dx\test-outputs\tauri-stage27-cargo-config-source-preflight-20260531-a\021-test-cargo-config-source-preflight.log`

### Stage 28: Cargo Metadata Cached-Source Preflight

**Purpose:** Prevent poisoned `cargo-metadata.machine` payloads from making the reader fingerprint implausible metadata source paths before it can reject the cache.

- [x] Add a focused red Rust test for implausible Cargo metadata cached source paths.
- [x] Cap cached Cargo metadata source path count before reconstructing current fingerprints.
- [x] Require cached metadata workspace roots to be absolute, parent-component-free, and ancestors of the app Tauri directory.
- [x] Allow only workspace `Cargo.toml`, workspace `Cargo.lock`, workspace `.cargo/config*`, workspace package `Cargo.toml`, or app-ancestry `Cargo.toml` paths before fingerprinting.
- [x] Run the combined CLI preflight guard test filter for workspace, Cargo config, and full Cargo metadata readers.

Latest Stage 28 receipts:

- `G:\Dx\test-outputs\tauri-stage28-cargo-metadata-source-preflight-20260531-a\010-red-cargo-metadata-source-preflight.log`
- `G:\Dx\test-outputs\tauri-stage28-cargo-metadata-source-preflight-20260531-a\022-rustfmt-check-cargo-metadata-source-preflight-final.log`
- `G:\Dx\test-outputs\tauri-stage28-cargo-metadata-source-preflight-20260531-a\023-test-cargo-metadata-source-preflight-final.log`
- `G:\Dx\test-outputs\tauri-stage28-cargo-metadata-source-preflight-20260531-a\024-test-all-source-preflight-guards.log`

### Stage 29: Serializer Dependency Version Metadata

**Purpose:** Reduce one release-portability blocker without pretending the local serializer dependency is publish-ready.

- [x] Add `version = "0.1.0"` beside the optional local `dx-serializer` path dependency in `tauri-cli`.
- [x] Add `version = "0.1.0"` beside the optional local `dx-serializer` path dependency in `tauri-utils`.
- [x] Verify the workspace manifest still resolves with `cargo metadata --no-deps`.
- [x] Record that publish/default-on readiness is still blocked by the local path dependency and serializer Rust 1.85 / edition 2024 metadata.

Latest Stage 29 receipts:

- `G:\Dx\test-outputs\tauri-stage29-serializer-portability-20260531-a\010-cargo-metadata-no-deps.log`
- `G:\Dx\test-outputs\tauri-stage29-serializer-portability-20260531-a\012-rg-serializer-version-fixed.log`

### Stage 30: Machine-Cache Release Boundary Guard

**Purpose:** Make release-portability limits machine-checkable instead of relying only on plan prose.

- [x] Add `.scripts/ci/test-dx-machine-cache-release-boundary.ts`.
- [x] Verify `tauri-cli` and `tauri-utils` keep the optional `dx-serializer` dependency versioned, path-scoped, default-features disabled, and opt-in.
- [x] Verify `dx-machine-cache` and `dx-machine-cache-mmap` do not enter default feature sets.
- [x] Verify Cargo metadata default feature sets for `tauri-utils`, `tauri-cli`, and `tauri-cli-node`.
- [x] Verify the sibling serializer crate version metadata matches the dependency declaration without hard-coding the version in the guard.
- [x] Verify serializer-internal path dependencies carry explicit versions for `dx-serializer-derive` and the derive crate's dev dependency back to `dx-serializer`.
- [x] Verify official publish workflows do not pass `dx-machine-cache` or `dx-machine-cache-mmap`.
- [x] Add a publish-mode fail gate that blocks normal publication while DX cache features are still present on publishable crates. Stage 43 supersedes the old override path while the serializer remains local-only.
- [x] Require docs/plan warnings when the serializer MSRV or edition remains incompatible with the Tauri workspace MSRV.
- [x] Extend the docs guard so the release-boundary warning cannot silently disappear.
- [x] Add explicit versions to `dx-serializer`'s own derive path dependency and the derive crate's dev dependency back to `dx-serializer`.

Latest Stage 30 receipts:

- `G:\Dx\test-outputs\tauri-stage30-release-boundary-20260531-a\010-red-release-boundary.log`
- `G:\Dx\test-outputs\tauri-stage30-release-boundary-20260531-a\011-red-docs-boundary.log`
- `G:\Dx\test-outputs\tauri-stage30-release-boundary-20260531-a\070-test-release-boundary-final.log`
- `G:\Dx\test-outputs\tauri-stage30-release-boundary-20260531-a\071-test-machine-cache-docs-final.log`
- `G:\Dx\test-outputs\tauri-stage30-release-boundary-20260531-a\072-git-diff-check-final.log`
- `G:\Dx\test-outputs\tauri-stage30-release-boundary-20260531-a\073-red-publish-mode-final.log`
- `G:\Dx\test-outputs\tauri-stage30-release-boundary-20260531-a\074-git-diff-check-after-receipts.log`
- `G:\Dx\test-outputs\tauri-stage30-release-boundary-20260531-a\064-cargo-metadata-serializer-no-deps.log`
- `G:\Dx\test-outputs\tauri-stage30-release-boundary-20260531-a\065-cargo-metadata-tauri-no-deps.log`
- `G:\Dx\test-outputs\tauri-stage30-release-boundary-20260531-a\067-rg-serializer-internal-versions.log`
- `G:\Dx\test-outputs\tauri-stage30-release-boundary-20260531-a\080-node-check-release-boundary-precommit.log`
- `G:\Dx\test-outputs\tauri-stage30-release-boundary-20260531-a\081-test-release-boundary-precommit.log`
- `G:\Dx\test-outputs\tauri-stage30-release-boundary-20260531-a\082-test-machine-cache-docs-precommit.log`
- `G:\Dx\test-outputs\tauri-stage30-release-boundary-20260531-a\083-git-diff-check-precommit.log`
- `G:\Dx\test-outputs\tauri-stage30-release-boundary-20260531-a\084-test-machine-cache-docs-after-precommit-receipts.log`
- `G:\Dx\test-outputs\tauri-stage30-release-boundary-20260531-a\085-git-diff-check-after-precommit-receipts.log`

### Stage 31: Cargo Metadata Source-Index Preflight

**Purpose:** Keep full `cargo-metadata.machine` source validation safe while avoiding repeated package-list scans on large workspaces.

- [x] Add a focused red test for a precomputed allowed cached-source index.
- [x] Build `CargoMetadataAllowedSourcePaths` once per archive read after workspace-root validation.
- [x] Preserve allowed workspace manifest, lockfile, `.cargo/config`, `.cargo/config.toml`, package manifest, and ancestor manifest behavior.
- [x] Preserve rejection for unrelated absolute paths, parent-directory traversal, and non-`Cargo.toml` metadata source files.
- [x] Gate `std::fs` in `cargo_settings_machine_cache.rs` behind `cfg(test)` so focused test runs no longer emit the normal-library unused-import warning.
- [x] Verify with the focused cargo-metadata cache unit family, the full cargo-metadata machine-cache subset, the source-matching guard, and touched-file rustfmt.

Latest Stage 31 receipts:

- `G:\Dx\test-outputs\tauri-stage31-cargo-metadata-source-index-20260531-a\010-red-index-helper.log`
- `G:\Dx\test-outputs\tauri-stage31-cargo-metadata-source-index-20260531-a\020-green-index-helper.log`
- `G:\Dx\test-outputs\tauri-stage31-cargo-metadata-source-index-20260531-a\052-rustfmt-check-touched-files.log`
- `G:\Dx\test-outputs\tauri-stage31-cargo-metadata-source-index-20260531-a\053-green-cargo-metadata-source-index-unit-family-final.log`
- `G:\Dx\test-outputs\tauri-stage31-cargo-metadata-source-index-20260531-a\054-green-full-cargo-metadata-machine-cache-final.log`
- `G:\Dx\test-outputs\tauri-stage31-cargo-metadata-source-index-20260531-a\055-source-matching-guard-final.log`

### Stage 32: Cargo Config Source-Dir Preflight

**Purpose:** Keep `cargo-config.machine` source validation safe while avoiding repeated ancestor and `CARGO_HOME` directory scans.

- [x] Add a focused red test for a precomputed allowed source-dir index.
- [x] Build `CargoConfigAllowedSourceDirs` once per archive read.
- [x] Preserve allowed ancestor `.cargo/config`, ancestor `.cargo/config.toml`, `CARGO_HOME/config`, and `CARGO_HOME/config.toml` behavior.
- [x] Preserve rejection for unrelated absolute paths, parent-directory traversal, and non-config source files.
- [x] Verify with the focused Cargo config cache unit family, the Cargo config machine-cache subset, the source-matching guard, and touched-file rustfmt.

Latest Stage 32 receipts:

- `G:\Dx\test-outputs\tauri-stage32-cargo-config-source-index-20260531-a\010-red-config-source-dir-index.log`
- `G:\Dx\test-outputs\tauri-stage32-cargo-config-source-index-20260531-a\020-green-config-source-dir-index.log`
- `G:\Dx\test-outputs\tauri-stage32-cargo-config-source-index-20260531-a\040-rustfmt-check-cargo-config-machine-cache-final.log`
- `G:\Dx\test-outputs\tauri-stage32-cargo-config-source-index-20260531-a\041-green-cargo-config-cache-unit-family-final.log`
- `G:\Dx\test-outputs\tauri-stage32-cargo-config-source-index-20260531-a\042-green-cargo-config-machine-cache-subset-final.log`
- `G:\Dx\test-outputs\tauri-stage32-cargo-config-source-index-20260531-a\043-source-matching-guard-final.log`

### Stage 33: Indexed Cache Timing Refresh

**Purpose:** Refresh current-fork parser-vs-machine timing evidence after Stage 31 and Stage 32 without counting `.machine` generation or claiming upstream/product-level superiority.

- [x] Run the existing ignored `cargo metadata --no-deps` source-vs-machine receipt test with `dx-machine-cache-mmap`, `TAURI_DX_MACHINE_CACHE_WRITE=0`, and `DX_TEST_OUTPUT_DIR` under `G:\Dx\test-outputs`.
- [x] Run the existing ignored `get_watch_folders` source-vs-machine receipt test against the same pre-generated cargo metadata sidecar.
- [x] Verify both receipts keep `.machine` generation recorded as setup but excluded from timing.
- [x] Verify both receipts keep `faster_than_upstream_claimed = false` and `official_tauri_binary_measured = false`.
- [x] Record the refreshed medians: cargo metadata `11.687x` faster by median; watch-folder boundary `16.062x` faster by median.

Latest Stage 33 receipts:

- `G:\Dx\test-outputs\tauri-stage33-indexed-cache-measurement-20260531-a\010-cargo-metadata-source-vs-machine-test.log`
- `G:\Dx\test-outputs\tauri-stage33-indexed-cache-measurement-20260531-a\cargo-metadata-source-vs-machine-receipt.json`
- `G:\Dx\test-outputs\tauri-stage33-indexed-cache-measurement-20260531-a\020-watch-folders-source-vs-machine-test.log`
- `G:\Dx\test-outputs\tauri-stage33-indexed-cache-measurement-20260531-a\watch-folders-source-vs-machine-receipt.json`

### Stage 34: Metadata Source-Set Completeness

**Purpose:** Reject malformed full metadata sidecars that omit required source fingerprints while trimming repeated source-exclusion normalization from metadata/config cache reads.

- [x] Add red tests for precomputed source exclusions in cargo metadata and Cargo config cache readers.
- [x] Add a red regression test showing that removing `Cargo.lock` from an otherwise valid `cargo-metadata.machine` source set was previously accepted.
- [x] Add a red regression test showing unsafe cached package manifest paths were previously not rejected by the allowed-source index builder.
- [x] Precompute app-manifest source exclusions once per archive read in `cargo_metadata_machine_cache.rs`.
- [x] Precompute app-manifest source exclusions once per archive read in `cargo_config/machine_cache.rs`.
- [x] Require exact cached source-set equality against the expected full metadata source set before hashing cached metadata paths.
- [x] Reject unsafe package manifest paths embedded in cached metadata before accepting the source set.
- [x] Verify with focused metadata/config cache families, full metadata machine-cache tests, the source-matching guard, touched-file rustfmt, and diff hygiene.

Latest Stage 34 receipts:

- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\010-red-metadata-source-exclusions.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\011-red-config-source-exclusions.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\033-red-incomplete-cached-source-set-assertion.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\040-green-incomplete-cached-source-set.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\051-rustfmt-check-touched-files-final.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\063-red-invalid-package-manifest-path.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\064-green-invalid-package-manifest-path.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\065-rustfmt-check-after-invalid-manifest.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\066-green-cargo-metadata-cache-family-final.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\067-green-full-cargo-metadata-machine-cache-final.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\068-green-cargo-config-cache-family-final.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\069-green-cargo-config-machine-cache-final.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\070-source-matching-guard-final.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\071-test-machine-cache-docs-final.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\072-test-release-boundary-final.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\073-git-diff-check-final.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\074-test-machine-cache-docs-after-final-plan.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\075-test-release-boundary-after-final-plan.log`
- `G:\Dx\test-outputs\tauri-stage34-source-exclusion-precompute-20260531-a\076-git-diff-check-after-final-plan.log`

### Stage 35: Release Boundary Publish Wiring

**Purpose:** Make release/default-on readiness fail closed in the actual covector release path while the DX serializer remains local/MSRV-incompatible.

- [x] Extend `.scripts/ci/test-dx-machine-cache-release-boundary.ts` to require covector workflow wiring before the covector action.
- [x] Extend the release-boundary guard to require a Rust prepublish command that runs with `DX_MACHINE_CACHE_RELEASE_BOUNDARY_MODE=publish`.
- [x] Add a normal release-boundary workflow step before covector.
- [x] Add the publish-mode release-boundary command to `.changes/config.json` Rust prepublish.
- [x] Keep official publish workflow feature scans blocking `dx-machine-cache` / `dx-machine-cache-mmap`, while allowing the guard script path itself.
- [x] Verify normal guard mode passes and publish mode fails closed with the expected serializer registry/MSRV readiness message.

Latest Stage 35 receipts:

- `G:\Dx\test-outputs\tauri-stage35-release-boundary-wiring-20260531-a\010-red-release-boundary-wiring.log`
- `G:\Dx\test-outputs\tauri-stage35-release-boundary-wiring-20260531-a\020-node-check-release-boundary.log`
- `G:\Dx\test-outputs\tauri-stage35-release-boundary-wiring-20260531-a\021-green-release-boundary-wiring.log`
- `G:\Dx\test-outputs\tauri-stage35-release-boundary-wiring-20260531-a\022-red-publish-mode-release-boundary.log`
- `G:\Dx\test-outputs\tauri-stage35-release-boundary-wiring-20260531-a\030-json-parse-changes-config.log`
- `G:\Dx\test-outputs\tauri-stage35-release-boundary-wiring-20260531-a\031-green-release-boundary-final.log`
- `G:\Dx\test-outputs\tauri-stage35-release-boundary-wiring-20260531-a\032-red-publish-mode-final.log`
- `G:\Dx\test-outputs\tauri-stage35-release-boundary-wiring-20260531-a\033-test-machine-cache-docs-final.log`
- `G:\Dx\test-outputs\tauri-stage35-release-boundary-wiring-20260531-a\034-git-diff-check-final.log`
- `G:\Dx\test-outputs\tauri-stage35-release-boundary-wiring-20260531-a\035-test-machine-cache-docs-after-receipt-list.log`
- `G:\Dx\test-outputs\tauri-stage35-release-boundary-wiring-20260531-a\036-git-diff-check-after-receipt-list.log`

### Stage 36: Workspace Source-Set Indexing

**Purpose:** Bring workspace-discovery cache validation up to the full metadata/config standard by indexing expected source paths once per archive read and rejecting malformed workspace sidecars that omit required source fingerprints.

- [x] Add red tests for indexed workspace allowed source paths and precomputed workspace source exclusions.
- [x] Add a red regression test showing an otherwise valid `cargo-workspace.machine` sidecar with a missing workspace manifest source is rejected.
- [x] Build `WorkspaceAllowedSourcePaths` once per cached workspace read and use set membership for every cached source.
- [x] Require exact cached source-set equality against the expected workspace source set before fingerprint comparison.
- [x] Precompute app/tauri manifest source exclusions once per cached workspace read.
- [x] Keep existing workspace-dir cache behavior stable when `Cargo.lock` appears or disappears.
- [x] Verify with focused workspace cache families, the source-matching guard, touched-file rustfmt, and diff hygiene.

Latest Stage 36 receipts:

- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\010-red-workspace-source-index.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\011-red-workspace-source-exclusions.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\012-red-workspace-incomplete-source-set.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\020-green-workspace-source-index.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\021-green-workspace-source-exclusions.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\022-green-workspace-incomplete-source-set.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\030-green-workspace-cache-unit-family.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\031-green-workspace-dir-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\032-source-matching-guard.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\033-rustfmt-check-workspace-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\034-git-diff-check.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\035-rustfmt-check-after-format.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\036-git-diff-check-after-format.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\037-green-workspace-cache-unit-family-after-format.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\038-green-workspace-dir-machine-cache-after-format.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\040-test-machine-cache-docs-after-stage36-plan.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\041-test-release-boundary-after-stage36-plan.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\042-git-diff-check-after-stage36-plan.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\043-test-machine-cache-docs-after-receipt-list.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\044-test-release-boundary-after-receipt-list.log`
- `G:\Dx\test-outputs\tauri-stage36-workspace-source-index-20260531-a\045-git-diff-check-after-receipt-list.log`

### Stage 37: Source Snapshot Builder De-Duplication

**Purpose:** Remove the remaining per-insert duplicate scans from multi-source cache snapshot construction while keeping source order, fingerprint semantics, and cache payload shape unchanged.

- [x] Add a red source-matching guard assertion that fails while snapshot push helpers still use `sources.iter().any(...)` for duplicate checks.
- [x] Replace full Cargo metadata snapshot insertion scans with a local `SourceSnapshotBuilder` that tracks normalized seen paths in a `HashSet`.
- [x] Replace workspace-discovery snapshot insertion scans with the same local builder pattern.
- [x] Replace Cargo config snapshot insertion scans with the same local builder pattern.
- [x] Replace package manifest/lock snapshot insertion scans with the same local builder pattern behind `dx-machine-cache`.
- [x] Verify the guard, metadata/config/workspace cache families, full metadata cache behavior, touched-file rustfmt, and diff hygiene.

Latest Stage 37 receipts:

- `G:\Dx\test-outputs\tauri-stage37-source-snapshot-builder-20260531-a\010-red-source-snapshot-builder-guard.log`
- `G:\Dx\test-outputs\tauri-stage37-source-snapshot-builder-20260531-a\020-green-source-snapshot-builder-guard.log`
- `G:\Dx\test-outputs\tauri-stage37-source-snapshot-builder-20260531-a\021-source-matching-guard-after-format.log`
- `G:\Dx\test-outputs\tauri-stage37-source-snapshot-builder-20260531-a\030-green-cargo-metadata-cache-family.log`
- `G:\Dx\test-outputs\tauri-stage37-source-snapshot-builder-20260531-a\031-green-full-cargo-metadata-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage37-source-snapshot-builder-20260531-a\032-green-cargo-config-cache-unit-family.log`
- `G:\Dx\test-outputs\tauri-stage37-source-snapshot-builder-20260531-a\033-green-cargo-config-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage37-source-snapshot-builder-20260531-a\034-green-workspace-cache-unit-family.log`
- `G:\Dx\test-outputs\tauri-stage37-source-snapshot-builder-20260531-a\035-green-workspace-dir-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage37-source-snapshot-builder-20260531-a\036-rustfmt-check-touched-files.log`
- `G:\Dx\test-outputs\tauri-stage37-source-snapshot-builder-20260531-a\037-git-diff-check.log`
- `G:\Dx\test-outputs\tauri-stage37-source-snapshot-builder-20260531-a\040-test-machine-cache-docs-after-stage37-plan.log`
- `G:\Dx\test-outputs\tauri-stage37-source-snapshot-builder-20260531-a\041-test-release-boundary-after-stage37-plan.log`
- `G:\Dx\test-outputs\tauri-stage37-source-snapshot-builder-20260531-a\042-source-matching-guard-after-stage37-plan.log`
- `G:\Dx\test-outputs\tauri-stage37-source-snapshot-builder-20260531-a\043-git-diff-check-after-stage37-plan.log`

### Stage 38: Package Version Direct Archive Read

**Purpose:** Remove an avoidable full-payload deserialize from valid `package-version.machine` hits while preserving typed-cache validation, source-path validation, semver validation, and owned return values.

- [x] Add a red mmap coverage guard assertion that requires archived-field access for package-version reads.
- [x] Make `package_version_from_archive` read `archived.package_json_path.as_str()` and `archived.version.as_str()` directly.
- [x] Remove the package-version-specific deserialize helper from the hit path.
- [x] Keep the returned version owned so mmap or bounded byte-buffer lifetimes cannot escape the helper.
- [x] Verify mmap coverage, focused package-version cache tests, touched-file rustfmt, and diff hygiene.

Latest Stage 38 receipts:

- `G:\Dx\test-outputs\tauri-stage38-package-version-direct-archive-20260531-a\010-red-package-version-direct-archive-guard.log`
- `G:\Dx\test-outputs\tauri-stage38-package-version-direct-archive-20260531-a\020-green-package-version-direct-archive-guard.log`
- `G:\Dx\test-outputs\tauri-stage38-package-version-direct-archive-20260531-a\030-green-package-version-machine-cache-family.log`
- `G:\Dx\test-outputs\tauri-stage38-package-version-direct-archive-20260531-a\031-rustfmt-check-package-version.log`
- `G:\Dx\test-outputs\tauri-stage38-package-version-direct-archive-20260531-a\032-git-diff-check.log`
- `G:\Dx\test-outputs\tauri-stage38-package-version-direct-archive-20260531-a\040-test-machine-cache-docs-after-stage38-plan.log`
- `G:\Dx\test-outputs\tauri-stage38-package-version-direct-archive-20260531-a\041-test-release-boundary-after-stage38-plan.log`
- `G:\Dx\test-outputs\tauri-stage38-package-version-direct-archive-20260531-a\042-mmap-coverage-after-stage38-plan.log`
- `G:\Dx\test-outputs\tauri-stage38-package-version-direct-archive-20260531-a\043-surface-matrix-after-stage38-plan.log`
- `G:\Dx\test-outputs\tauri-stage38-package-version-direct-archive-20260531-a\044-git-diff-check-after-stage38-plan.log`

### Stage 39: Benchmark Broad-Claim Gates

**Purpose:** Make the command-benchmark sample guard prove narrow current-source claims do not accidentally open broad upstream/default-on/full-product claim families.

- [x] Add a shared assertion that broad benchmark claim gates stay false for valid current-source build-receipt evidence.
- [x] Require blocked-claim records for `upstream_superiority`, `default_on`, `app_runtime`, and `full_cli`.
- [x] Scan allowed benchmark claims for broad product-superiority wording.
- [x] Verify the sample-invocation guard, benchmark-manifest syntax, and diff hygiene.

Latest Stage 39 receipts:

- `G:\Dx\test-outputs\tauri-stage39-broad-claim-gates-20260531-a\010-node-check-sample-invocation.log`
- `G:\Dx\test-outputs\tauri-stage39-broad-claim-gates-20260531-a\020-green-sample-invocation-claim-gates.log`
- `G:\Dx\test-outputs\tauri-stage39-broad-claim-gates-20260531-a\021-node-check-benchmark-manifest.log`
- `G:\Dx\test-outputs\tauri-stage39-broad-claim-gates-20260531-a\022-git-diff-check.log`
- `G:\Dx\test-outputs\tauri-stage39-broad-claim-gates-20260531-a\030-test-machine-cache-docs-after-stage39-plan.log`
- `G:\Dx\test-outputs\tauri-stage39-broad-claim-gates-20260531-a\031-test-release-boundary-after-stage39-plan.log`
- `G:\Dx\test-outputs\tauri-stage39-broad-claim-gates-20260531-a\032-green-sample-invocation-after-stage39-plan.log`
- `G:\Dx\test-outputs\tauri-stage39-broad-claim-gates-20260531-a\033-git-diff-check-after-stage39-plan.log`

### Stage 40: MSRV Feature Boundary

**Purpose:** Prevent the Rust 1.77.2 `tauri-utils` all-feature CI lane from accidentally enabling local-only DX serializer features that require newer Rust/edition support.

- [x] Add a focused TypeScript guard for the `test-core` MSRV/DX machine-cache workflow boundary.
- [x] Add the guard to `test-core.yml` so the boundary is checked in the workflow it protects.
- [x] Replace `tauri-utils` use of `${{ matrix.features.args }}` with `${{ matrix.features.tauri_utils_args }}`.
- [x] Keep `tauri` on the normal matrix feature args while `tauri-utils` all-feature mode uses an explicit public feature list that excludes `dx-machine-cache` and `dx-machine-cache-mmap`.
- [x] Verify the guard, release-boundary guard, docs guard, and diff hygiene.

Latest Stage 40 receipts:

- `G:\Dx\test-outputs\tauri-stage40-msrv-boundary-20260531-a\010-red-msrv-boundary-guard.log`
- `G:\Dx\test-outputs\tauri-stage40-msrv-boundary-20260531-a\020-node-check-msrv-boundary-guard.log`
- `G:\Dx\test-outputs\tauri-stage40-msrv-boundary-20260531-a\021-green-msrv-boundary-guard.log`
- `G:\Dx\test-outputs\tauri-stage40-msrv-boundary-20260531-a\022-release-boundary-after-msrv-workflow.log`
- `G:\Dx\test-outputs\tauri-stage40-msrv-boundary-20260531-a\023-git-diff-check.log`
- `G:\Dx\test-outputs\tauri-stage40-msrv-boundary-20260531-a\030-test-machine-cache-docs-after-stage40-plan.log`
- `G:\Dx\test-outputs\tauri-stage40-msrv-boundary-20260531-a\031-test-release-boundary-after-stage40-plan.log`
- `G:\Dx\test-outputs\tauri-stage40-msrv-boundary-20260531-a\032-green-msrv-boundary-after-stage40-plan.log`
- `G:\Dx\test-outputs\tauri-stage40-msrv-boundary-20260531-a\033-git-diff-check-after-stage40-plan.log`

No live GitHub CI/MSRV cargo-run receipt is recorded here for Stage 40.

### Stage 41: Workspace Direct Archive Read

**Purpose:** Remove an avoidable full-cache deserialize from valid `cargo-workspace.machine` hits while preserving typed-cache validation, workspace-dir validation, cached-source completeness checks, and source fingerprint comparison.

- [x] Add a red mmap coverage guard assertion that requires archived-field access for workspace cache reads.
- [x] Make `workspace_dir_from_archive` read `archived.tauri_dir.as_str()` and `archived.workspace_dir.as_str()` directly.
- [x] Replace the full workspace-cache deserialize helper with archived source-fingerprint reconstruction for the existing validation path.
- [x] Keep the returned workspace path owned so mmap or bounded byte-buffer lifetimes cannot escape the helper.
- [x] Verify mmap coverage, focused workspace cache tests, workspace-dir behavior tests, Node syntax, touched-file rustfmt, and diff hygiene.

Latest Stage 41 receipts:

- `G:\Dx\test-outputs\tauri-stage41-workspace-direct-archive-20260531-a\010-red-workspace-direct-archive-guard.log`
- `G:\Dx\test-outputs\tauri-stage41-workspace-direct-archive-20260531-a\020-green-workspace-direct-archive-guard.log`
- `G:\Dx\test-outputs\tauri-stage41-workspace-direct-archive-20260531-a\030-green-workspace-cache-unit-family.log`
- `G:\Dx\test-outputs\tauri-stage41-workspace-direct-archive-20260531-a\031-green-workspace-dir-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage41-workspace-direct-archive-20260531-a\032-node-check-mmap-coverage.log`
- `G:\Dx\test-outputs\tauri-stage41-workspace-direct-archive-20260531-a\033-rustfmt-check-workspace-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage41-workspace-direct-archive-20260531-a\034-git-diff-check.log`
- `G:\Dx\test-outputs\tauri-stage41-workspace-direct-archive-20260531-a\040-test-machine-cache-docs-after-stage41-plan.log`
- `G:\Dx\test-outputs\tauri-stage41-workspace-direct-archive-20260531-a\041-test-release-boundary-after-stage41-plan.log`
- `G:\Dx\test-outputs\tauri-stage41-workspace-direct-archive-20260531-a\042-mmap-coverage-after-stage41-plan.log`
- `G:\Dx\test-outputs\tauri-stage41-workspace-direct-archive-20260531-a\043-git-diff-check-after-stage41-plan.log`

### Stage 42: Machine Cache Guard Suite

**Purpose:** Keep DX machine-cache governance checks visible and registered so future guard scripts cannot drift into manual-only, untracked work.

- [x] Add a TypeScript guard-suite registry covering broad-claim, release-boundary, MSRV-boundary, untracked-cache-surface, write-disabled-timing, read-order, and projection-validation risks.
- [x] Discover every `.scripts/ci/test-dx-machine-cache-*.ts` file and fail if it is not registered.
- [x] Require the release workflow, Rust prepublish config, MSRV workflow, `PLAN.md`, and `ENVIRONMENT_VARIABLES.md` to keep their core DX machine-cache governance wording.
- [x] Apply claim-hygiene fixes from read-only audit inputs: remove numeric readiness score precision, make benchmark baselines wording broader than official-release-only evidence, and mark historical benchmark receipts as historical.
- [x] Verify the guard-suite registry and the existing lightweight guard suite.

Latest Stage 42 receipts:

- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\010-green-guard-suite-initial.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\020-node-check-guard-suite.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\021-green-guard-suite.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\022-release-boundary.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\023-msrv-boundary.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\024-docs.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\025-surface-matrix.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\026-read-only-mode.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\027-mmap-coverage.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\028-red-source-matching-before-guard-fix.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\028-source-matching-after-guard-fix.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\029-git-diff-check.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\030-test-machine-cache-docs-after-stage42-plan.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\031-test-release-boundary-after-stage42-plan.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\032-green-guard-suite-after-stage42-plan.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\033-source-matching-after-stage42-plan.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\034-git-diff-check-after-stage42-plan.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\035-test-machine-cache-docs-after-receipt-fix.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\036-test-release-boundary-after-receipt-fix.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\037-green-guard-suite-after-receipt-fix.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\038-source-matching-after-receipt-fix.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\039-git-diff-check-after-receipt-fix.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\040-test-machine-cache-docs-final.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\041-test-release-boundary-final.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\042-green-guard-suite-final.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\043-source-matching-final.log`
- `G:\Dx\test-outputs\tauri-stage42-guard-suite-20260531-a\044-git-diff-check-final.log`

### Stage 43: Serializer Local-Only Release Policy

**Purpose:** Make publish-mode release checks fail closed while the DX serializer remains a local-only, unpublished, MSRV-incompatible dependency.

- [x] Turn the release-boundary guard's local serializer facts into an explicit release-policy status.
- [x] Block publish mode when the serializer policy is `local-only-unpublished`, even when `DX_ALLOW_MACHINE_CACHE_PUBLICATION=1` is present.
- [x] Require docs and plan wording to keep the local-only / not-crates.io/MSRV-ready status visible.
- [x] Verify the normal release-boundary path, docs guard, guard-suite registry, and script syntax without running heavy builds.

Latest Stage 43 receipts:

- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\010-red-publish-override-still-passes.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\020-green-publish-override-blocked.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\020-green-publish-override-blocked-exit.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\021-green-release-boundary.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\022-docs.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\023-guard-suite.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\024-node-check-release-boundary.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\025-node-check-docs.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\026-node-check-guard-suite.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\030-release-boundary-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\031-docs-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\032-guard-suite-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\033-git-diff-check-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\034-publish-override-blocked-final.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\034-publish-override-blocked-final-exit.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\040-release-boundary-final.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\041-docs-final.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\042-guard-suite-final.log`
- `G:\Dx\test-outputs\tauri-stage43-serializer-local-policy-20260531-a\043-git-diff-check-final.log`

### Stage 44: Cargo Settings Direct Archive Path

**Purpose:** Avoid full wrapper deserialization on `cargo-settings.machine` cache hits while preserving the existing source-path validation and owned settings materialization.

- [x] Add mmap-coverage guard requirements for direct archived `cargo_toml_path` access.
- [x] Compare `archived.cargo_toml_path.as_str()` before deserializing the settings payload.
- [x] Replace the full-wrapper deserialize helper with a settings-field deserialize helper.
- [x] Verify focused Cargo settings cache tests and lightweight guard checks without running a full build.
- [x] Record the initial parallel cargo-test timeout as inconclusive lock contention; use the sequential cargo-test receipts as the passing Rust evidence.

Latest Stage 44 receipts:

- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\010-red-cargo-settings-direct-archive-guard.log`
- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\010-red-cargo-settings-direct-archive-guard-exit.log`
- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\020-green-mmap-coverage.log`
- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\021-node-check-mmap-coverage.log`
- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\022-cargo-settings-cache-tests.log`
- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\023-cargo-settings-machine-cache-module-tests.log`
- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\024-cargo-timeout-process-watch.log`
- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\030-cargo-settings-machine-cache-module-tests-sequential.log`
- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\031-cargo-settings-cache-tests-sequential.log`
- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\040-mmap-coverage-final.log`
- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\041-guard-suite-final.log`
- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\042-rustfmt-check-cargo-settings.log`
- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\043-git-diff-check.log`
- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\050-mmap-coverage-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\051-guard-suite-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\052-release-boundary-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage44-cargo-settings-direct-archive-20260531-a\053-git-diff-check-after-plan.log`

### Stage 45: Cargo Config Direct Archive Path

**Purpose:** Avoid full wrapper deserialization on `cargo-config.machine` cache hits while preserving cargo-home, source-path, source-fingerprint, and ambiguous-config validation.

- [x] Add mmap-coverage guard requirements for direct archived Cargo config field access.
- [x] Compare archived `tauri_dir` and `cargo_home` before accepting the cache hit.
- [x] Reconstruct only cached source fingerprints from archived fields for the existing source-validation path.
- [x] Materialize only the archived target string needed for the returned `Config`.
- [x] Verify focused Cargo config cache tests and lightweight guard checks without running a full build.
- [x] Record the intermediate guard-snippet and rustfmt adjustments; use the post-format receipts as the passing final evidence.

Latest Stage 45 receipts:

- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\010-red-cargo-config-direct-archive-guard.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\010-red-cargo-config-direct-archive-guard-exit.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\020-green-mmap-coverage.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\021-green-mmap-coverage-after-target-snippet.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\030-cargo-config-machine-cache-tests.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\031-cargo-config-machine-cache-module-tests.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\040-node-check-mmap-coverage.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\041-guard-suite.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\042-rustfmt-check-cargo-config-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\043-git-diff-check.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\050-cargo-config-machine-cache-tests-after-format.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\051-cargo-config-machine-cache-module-tests-after-format.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\052-mmap-coverage-after-format.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\053-node-check-mmap-coverage-after-format.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\054-rustfmt-check-after-format.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\055-git-diff-check-after-format.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\060-mmap-coverage-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\061-guard-suite-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\062-release-boundary-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage45-cargo-config-direct-archive-20260531-a\063-git-diff-check-after-plan.log`

### Stage 46: Project Config Direct Archive Path

**Purpose:** Avoid full wrapper deserialization on full project-config `.machine` cache hits while preserving config-path validation, source-fingerprint validation, platform config pairing, and JSON tree safety checks.

- [x] Add mmap-coverage guard requirements for direct archived full project-config reads.
- [x] Reuse the archived source-validation path already used by project-config projection reads.
- [x] Materialize merged and platform `serde_json::Value`s from archived JSON trees after archived tree validation.
- [x] Keep the existing leaf-value cache reader and project-config projection behavior unchanged.
- [x] Verify focused tauri-utils project-config, JSON-tree, projection, guard, and formatting checks without running a full build.
- [x] Record the initial archived-object iterator compile failure; use the post-fix tauri-utils receipts as the passing Rust evidence.

Latest Stage 46 receipts:

- `G:\Dx\test-outputs\tauri-stage46-project-config-direct-archive-20260531-a\010-red-project-config-direct-archive-guard.log`
- `G:\Dx\test-outputs\tauri-stage46-project-config-direct-archive-20260531-a\010-red-project-config-direct-archive-guard-exit.log`
- `G:\Dx\test-outputs\tauri-stage46-project-config-direct-archive-20260531-a\020-green-mmap-coverage.log`
- `G:\Dx\test-outputs\tauri-stage46-project-config-direct-archive-20260531-a\030-tauri-utils-project-config-cache-tests.log`
- `G:\Dx\test-outputs\tauri-stage46-project-config-direct-archive-20260531-a\031-tauri-utils-project-config-cache-tests-after-iterator-fix.log`
- `G:\Dx\test-outputs\tauri-stage46-project-config-direct-archive-20260531-a\032-tauri-utils-json-tree-tests.log`
- `G:\Dx\test-outputs\tauri-stage46-project-config-direct-archive-20260531-a\033-tauri-utils-project-config-projection-tests.log`
- `G:\Dx\test-outputs\tauri-stage46-project-config-direct-archive-20260531-a\040-mmap-coverage-final.log`
- `G:\Dx\test-outputs\tauri-stage46-project-config-direct-archive-20260531-a\041-node-check-mmap-coverage.log`
- `G:\Dx\test-outputs\tauri-stage46-project-config-direct-archive-20260531-a\042-rustfmt-check-tauri-utils-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage46-project-config-direct-archive-20260531-a\043-git-diff-check.log`
- `G:\Dx\test-outputs\tauri-stage46-project-config-direct-archive-20260531-a\044-guard-suite.log`
- `G:\Dx\test-outputs\tauri-stage46-project-config-direct-archive-20260531-a\050-mmap-coverage-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage46-project-config-direct-archive-20260531-a\051-guard-suite-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage46-project-config-direct-archive-20260531-a\052-release-boundary-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage46-project-config-direct-archive-20260531-a\053-git-diff-check-after-plan.log`

### Stage 47: Project Config Timing Receipt

**Purpose:** Add a receipt-backed no-build timing harness for full project-config source parsing versus a pre-generated validated `.machine` read.

- [x] Add an ignored manual timing test for full project-config source-vs-machine reads.
- [x] Generate the `.machine` file during setup before warmups and timed samples.
- [x] Set `TAURI_DX_MACHINE_CACHE_WRITE=0` during machine timing and record that cache generation is excluded.
- [x] Record the first result honestly: source parse median `421,500 ns`, validated machine read median `487,400 ns`, machine/source median ratio `115%`.
- [x] Do not claim a project-config speedup from this tiny fixture.

Latest Stage 47 receipts:

- `G:\Dx\test-outputs\tauri-stage47-project-config-timing-20260531-a\010-project-config-source-vs-machine-receipt-test.log`
- `G:\Dx\test-outputs\tauri-stage47-project-config-timing-20260531-a\011-project-config-source-vs-machine-receipt-test-after-helper-fix.log`
- `G:\Dx\test-outputs\tauri-stage47-project-config-timing-20260531-a\012-project-config-source-vs-machine-receipt-test-after-schema-fix.log`
- `G:\Dx\test-outputs\tauri-stage47-project-config-timing-20260531-a\tauri-project-config-source-vs-machine-receipt.json`
- `G:\Dx\test-outputs\tauri-stage47-project-config-timing-20260531-a\020-rustfmt-check-tauri-utils-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage47-project-config-timing-20260531-a\021-guard-suite.log`
- `G:\Dx\test-outputs\tauri-stage47-project-config-timing-20260531-a\022-git-diff-check.log`
- `G:\Dx\test-outputs\tauri-stage47-project-config-timing-20260531-a\030-project-config-receipt-test-compile-no-run.log`
- `G:\Dx\test-outputs\tauri-stage47-project-config-timing-20260531-a\031-rustfmt-check-after-format.log`
- `G:\Dx\test-outputs\tauri-stage47-project-config-timing-20260531-a\032-guard-suite-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage47-project-config-timing-20260531-a\033-git-diff-check-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage47-project-config-timing-20260531-a\034-release-boundary-after-plan.log`

### Stage 48: Leaf Config Timing Refresh

**Purpose:** Refresh the existing leaf-config JSON source-vs-machine receipt after the direct archive cleanup work, without adding new implementation changes.

- [x] Run the existing ignored leaf-config source-vs-machine receipt with `.machine` generation completed before timing.
- [x] Keep machine timing read-only with cache writes excluded from timing.
- [x] Record the measured result honestly: source parse median `110,000 ns`, validated machine read median `301,800 ns`, machine/source median ratio `274%`.
- [x] Do not claim a leaf-config speedup from this tiny fixture.

Latest Stage 48 receipts:

- `G:\Dx\test-outputs\tauri-stage48-leaf-config-timing-20260531-a\010-leaf-config-source-vs-machine-receipt-test.log`
- `G:\Dx\test-outputs\tauri-stage48-leaf-config-timing-20260531-a\tauri-config-source-vs-machine-json-receipt.json`
- `G:\Dx\test-outputs\tauri-stage48-leaf-config-timing-20260531-a\020-guard-suite-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage48-leaf-config-timing-20260531-a\021-release-boundary-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage48-leaf-config-timing-20260531-a\022-git-diff-check-after-plan.log`

### Stage 49: Representative Project Config Timing Receipt

**Purpose:** Add a larger generated project-config timing receipt after the tiny config fixtures showed validation overhead can dominate small files.

- [x] Add a deterministic representative project-config fixture generator in a small test-only module.
- [x] Generate the `.machine` file during setup before warmups and timed samples.
- [x] Remove `TAURI_DX_MACHINE_CACHE` and set `TAURI_DX_MACHINE_CACHE_WRITE=0` during source timing.
- [x] Set `TAURI_DX_MACHINE_CACHE_WRITE=0` during machine timing and verify the `.machine` file is unchanged.
- [x] Record the measured result honestly: source parse median `10.603 ms`, validated machine read median `4.783 ms`, machine/source median ratio `45%`, about `2.22x` faster for this fixture.
- [x] Do not claim a generic config speedup, broad Tauri superiority, or a 10x product-level win from this fixture.

Latest Stage 49 receipts:

- `G:\Dx\test-outputs\tauri-stage49-representative-project-config-timing-20260531-a\010-cargo-fmt-tauri-utils.log`
- `G:\Dx\test-outputs\tauri-stage49-representative-project-config-timing-20260531-a\020-representative-project-config-timing.log`
- `G:\Dx\test-outputs\tauri-stage49-representative-project-config-timing-20260531-a\tauri-representative-project-config-source-vs-machine-receipt.json`
- `G:\Dx\test-outputs\tauri-stage49-representative-project-config-timing-20260531-b\010-cargo-fmt-tauri-utils.log`
- `G:\Dx\test-outputs\tauri-stage49-representative-project-config-timing-20260531-b\020-representative-project-config-timing.log`
- `G:\Dx\test-outputs\tauri-stage49-representative-project-config-timing-20260531-b\tauri-representative-project-config-source-vs-machine-receipt.json`
- `G:\Dx\test-outputs\tauri-stage49-representative-project-config-timing-20260531-c\010-cargo-fmt-tauri-utils.log`
- `G:\Dx\test-outputs\tauri-stage49-representative-project-config-timing-20260531-c\020-representative-project-config-timing.log`
- `G:\Dx\test-outputs\tauri-stage49-representative-project-config-timing-20260531-c\030-machine-cache-docs-guard.log`
- `G:\Dx\test-outputs\tauri-stage49-representative-project-config-timing-20260531-c\031-machine-cache-guard-suite.log`
- `G:\Dx\test-outputs\tauri-stage49-representative-project-config-timing-20260531-c\032-git-diff-check.log`
- `G:\Dx\test-outputs\tauri-stage49-representative-project-config-timing-20260531-c\tauri-representative-project-config-source-vs-machine-receipt.json`

### Stage 50: Cargo Metadata Direct Archive Refresh

**Purpose:** Remove wrapper-level deserialization from full `cargo-metadata.machine` cache hits, then refresh the strongest current-fork hot-path receipts.

- [x] Validate archived `tauri_dir`, `CARGO_TARGET_DIR`, cached source set, and package manifest paths directly from the archived cache.
- [x] Materialize returned `CargoMetadata` directly from archived metadata after validation.
- [x] Guard the mmap/direct-archive coverage script against reintroducing `deserialize_cargo_metadata_machine_archive`.
- [x] Run focused Cargo metadata cache unit tests and full metadata cache behavior tests.
- [x] Refresh the current-fork `cargo metadata --no-deps` source-vs-machine receipt: source median `99.627 ms`, machine median `6.398 ms`, about `15.572x` faster.
- [x] Refresh the current-fork `get_watch_folders` source-vs-machine receipt: source median `81.168 ms`, machine median `5.628 ms`, about `14.421x` faster.
- [x] Keep claim scope narrow: these are current-fork parser/probe-vs-pre-generated-machine hot-path receipts, not official release, upstream-source, full CLI, or product-level evidence.

Latest Stage 50 receipts:

- `G:\Dx\test-outputs\tauri-stage50-cargo-metadata-direct-archive-20260531-a\010-cargo-fmt-tauri-cli.log`
- `G:\Dx\test-outputs\tauri-stage50-cargo-metadata-direct-archive-20260531-a\020-mmap-coverage-guard.log`
- `G:\Dx\test-outputs\tauri-stage50-cargo-metadata-direct-archive-20260531-a\030-cargo-metadata-cache-unit-tests.log`
- `G:\Dx\test-outputs\tauri-stage50-cargo-metadata-direct-archive-20260531-a\034-cargo-metadata-cache-unit-tests-after-archived-source-fix.log`
- `G:\Dx\test-outputs\tauri-stage50-cargo-metadata-direct-archive-20260531-a\041-cargo-metadata-cache-unit-tests-after-dead-helper-removal.log`
- `G:\Dx\test-outputs\tauri-stage50-cargo-metadata-direct-archive-20260531-a\050-full-cargo-metadata-machine-cache-tests.log`
- `G:\Dx\test-outputs\tauri-stage50-cargo-metadata-direct-archive-20260531-a\060-mmap-coverage-final.log`
- `G:\Dx\test-outputs\tauri-stage50-cargo-metadata-direct-archive-20260531-a\061-guard-suite.log`
- `G:\Dx\test-outputs\tauri-stage50-cargo-metadata-direct-archive-20260531-a\062-git-diff-check.log`
- `G:\Dx\test-outputs\tauri-stage50-cargo-metadata-direct-archive-20260531-a\070-cargo-metadata-source-vs-machine-receipt.log`
- `G:\Dx\test-outputs\tauri-stage50-cargo-metadata-direct-archive-20260531-a\071-watch-folders-source-vs-machine-receipt.log`
- `G:\Dx\test-outputs\tauri-stage50-cargo-metadata-direct-archive-20260531-a\cargo-metadata-source-vs-machine-receipt.json`
- `G:\Dx\test-outputs\tauri-stage50-cargo-metadata-direct-archive-20260531-a\watch-folders-source-vs-machine-receipt.json`

### Stage 51: Cargo Metadata Projection Reads

**Purpose:** Add a narrow validated projection from `cargo-metadata.machine` for callers that only need target-directory or in-workspace dependency paths.

- [x] Add `cargo_metadata_machine_cache::read_projection` using the same typed-cache validation, source-set validation, and mmap-before-fallback policy as full metadata reads.
- [x] Use the projection in `get_in_workspace_dependency_paths` and `get_cargo_target_dir` before falling back to full Cargo metadata.
- [x] Add no-spawn regression tests for warm-cache watch-folder and target-directory projection hits.
- [x] Track the projection reader in the mmap coverage guard.
- [x] Update the watch-folder timing receipt label so it records the projection path.
- [x] Refresh the current-fork `cargo metadata --no-deps` receipt after this change: source median `118.234 ms`, machine median `10.447 ms`, about `11.317x` faster.
- [x] Refresh the latest labeled current-fork `get_watch_folders` receipt after this change: source median `141.635 ms`, machine median `10.333 ms`, about `13.708x` faster.
- [x] Keep claim scope narrow: these are current-fork parser/probe-vs-pre-generated-machine hot-path receipts, not official release, upstream-source, full CLI, or product-level evidence.

Latest Stage 51 receipts:

- `G:\Dx\test-outputs\tauri-stage51-cargo-metadata-projections-20260531-a\010-cargo-fmt-tauri-cli.log`
- `G:\Dx\test-outputs\tauri-stage51-cargo-metadata-projections-20260531-a\020-full-cargo-metadata-machine-cache-tests.log`
- `G:\Dx\test-outputs\tauri-stage51-cargo-metadata-projections-20260531-a\030-cargo-fmt-after-projection-tests.log`
- `G:\Dx\test-outputs\tauri-stage51-cargo-metadata-projections-20260531-a\040-projection-no-cargo-tests.log`
- `G:\Dx\test-outputs\tauri-stage51-cargo-metadata-projections-20260531-a\050-mmap-coverage-with-projection.log`
- `G:\Dx\test-outputs\tauri-stage51-cargo-metadata-projections-20260531-a\051-full-cargo-metadata-machine-cache-tests.log`
- `G:\Dx\test-outputs\tauri-stage51-cargo-metadata-projections-20260531-a\060-cargo-metadata-source-vs-machine-receipt.log`
- `G:\Dx\test-outputs\tauri-stage51-cargo-metadata-projections-20260531-a\061-watch-folders-source-vs-machine-receipt.log`
- `G:\Dx\test-outputs\tauri-stage51-cargo-metadata-projections-20260531-a\cargo-metadata-source-vs-machine-receipt.json`
- `G:\Dx\test-outputs\tauri-stage51-cargo-metadata-projections-20260531-a\watch-folders-source-vs-machine-receipt.json`
- `G:\Dx\test-outputs\tauri-stage51-cargo-metadata-projections-20260531-b\010-cargo-fmt-tauri-cli.log`
- `G:\Dx\test-outputs\tauri-stage51-cargo-metadata-projections-20260531-b\020-watch-folders-source-vs-machine-receipt.log`
- `G:\Dx\test-outputs\tauri-stage51-cargo-metadata-projections-20260531-b\watch-folders-source-vs-machine-receipt.json`

### Stage 52: Package Manifest Lock Direct Archive Reads

**Purpose:** Remove wrapper-level deserialization from `cargo-package-metadata.machine` package manifest/lock cache hits while preserving source-fingerprint validation, read-only behavior, and safe fallback.

- [x] Read archived `tauri_dir`, manifest, and lock fields directly on cache hits instead of deserializing the whole machine-cache wrapper.
- [x] Materialize manifest dependencies and lock package sources from archived fields, and remove stale owned conversion/matcher helpers from the read path.
- [x] Add archive-path regression tests for envelope-source reuse, duplicate cached sources, dependency/lock fidelity, unsafe archived workspace paths, and unsafe archived lock paths.
- [x] Tighten the mmap coverage guard so the package manifest/lock reader tracks exact direct-archive snippets and forbids reintroducing wrapper deserialize helpers.
- [x] Keep claim scope narrow: this is a focused hit-path cleanup for the package manifest/lock cache, not a new timing result, official-release comparison, upstream-source comparison, full CLI benchmark, or product-level evidence.

Latest Stage 52 receipts:

- `G:\Dx\test-outputs\tauri-stage52-package-metadata-direct-archive-20260531-a\040-cargo-fmt-tauri-cli-after-cleanup.log`
- `G:\Dx\test-outputs\tauri-stage52-package-metadata-direct-archive-20260531-a\041-package-metadata-cache-tests-after-cleanup.log`
- `G:\Dx\test-outputs\tauri-stage52-package-metadata-direct-archive-20260531-a\042-mmap-coverage-guard-after-cleanup.log`
- `G:\Dx\test-outputs\tauri-stage52-package-metadata-direct-archive-20260531-a\043-guard-suite-after-cleanup.log`
- `G:\Dx\test-outputs\tauri-stage52-package-metadata-direct-archive-20260531-a\044-docs-guard-after-cleanup.log`
- `G:\Dx\test-outputs\tauri-stage52-package-metadata-direct-archive-20260531-a\045-git-diff-check-after-cleanup.log`
- `G:\Dx\test-outputs\tauri-stage52-package-metadata-direct-archive-20260531-a\046-package-metadata-no-default-dx-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage52-package-metadata-direct-archive-20260531-a\047-package-metadata-no-default-dx-machine-cache-mmap.log`
- `G:\Dx\test-outputs\tauri-stage52-package-metadata-direct-archive-20260531-a\048-package-metadata-no-default-dx-machine-cache-mmap-no-debug.log`
- `G:\Dx\test-outputs\tauri-stage52-package-metadata-direct-archive-20260531-a\054-package-metadata-no-default-dx-machine-cache-mmap-no-debug-retry.log`

Stage 52 verification note: focused default-feature and no-default `dx-machine-cache` Rust tests passed. The no-default `dx-machine-cache-mmap` command did not complete in this turn: MSVC first hit `LNK1140` PDB limits, the first debug-info-disabled retry hit disk-full `os error 112` with only about `0.14 GB` free on `G:`, and the later retry timed out after 300 seconds while still compiling dependencies before tests started. This is recorded as environment-blocked/incomplete verification, not a code failure.

### Stage 53: Package Manifest Lock Timing Receipt

**Purpose:** Add a focused helper-level timing receipt for `cargo_manifest_and_lock` after the Stage 52 direct-archive cleanup, with `.machine` generated during setup before machine timing and writes disabled during timed machine samples.

- [x] Add a lightweight TypeScript receipt-contract guard for the package manifest/lock timing receipt.
- [x] Keep the ignored Rust receipt generator in a small test-only child module instead of expanding `cargo_manifest.rs`.
- [x] Register the receipt guard in the `.machine` surface matrix for `cargo-package-metadata.machine`.
- [x] Run the focused timing receipt: source median `50.553 ms`, validated `.machine` median `2.894 ms`, machine/source ratio `5.724%`, source-to-machine median speedup `17.470x`.
- [x] Keep claim scope narrow: this is current-fork helper-level package manifest/lock hot-path evidence, not an official-release comparison, upstream-source comparison, full CLI benchmark, default-on/release-readiness proof, mmap receipt, or product-level speed claim.

Latest Stage 53 receipts:

- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\010-package-metadata-benchmark-guard-red.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\030-package-metadata-benchmark-guard-green.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\040-package-metadata-source-vs-machine-receipt.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\051-package-metadata-benchmark-guard-after-module-path.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\052-surface-matrix-after-receipt-guard.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\061-package-metadata-benchmark-guard-after-source-baseline.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\062-surface-matrix-after-reader-count.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\063-surface-matrix-after-writer-count.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\064-surface-matrix-after-ignored-fixture-artifact.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\070-package-metadata-source-vs-machine-receipt.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\080-package-metadata-benchmark-guard-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\081-surface-matrix-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\082-guard-suite-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\083-docs-guard-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\084-git-diff-check-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\085-package-metadata-benchmark-guard-after-command-tighten.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\086-surface-matrix-after-command-tighten.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\087-git-diff-check-after-command-tighten.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\088-guard-suite-after-command-tighten.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\089-cargo-fmt-final.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\090-docs-guard-final.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\091-git-diff-check-final.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\092-plan-receipt-paths-check.log`
- `G:\Dx\test-outputs\tauri-stage53-package-metadata-timing-receipt-20260531-a\cargo-package-metadata-source-vs-machine-receipt.json`

### Stage 54: Package Version Timing Receipt

**Purpose:** Add a focused helper-level timing receipt for the `load_config` `package.version` path after the Stage 38 direct-archive cleanup, with `package-version.machine` generated during setup before machine timing and writes disabled during timed machine samples.

- [x] Add a lightweight TypeScript receipt-contract guard for the package-version timing receipt.
- [x] Keep the ignored Rust receipt generator in a small test-only child module instead of expanding `config.rs`.
- [x] Register the receipt guard in the `.machine` surface matrix for `package-version.machine`.
- [x] Keep the broader `project-config.machine` sidecar absent during timing so the receipt isolates the package-version cache path.
- [x] Run the focused timing receipt: source median `9.741 ms`, validated `.machine` median `1.957 ms`, machine/source ratio `20.087%`, source-to-machine median speedup `4.978x`, source machine-hit count `0`, timed machine-hit count `140/140`.
- [x] Keep claim scope narrow: this is a real current-fork helper-level package-version hot-path win, not an official-release comparison, upstream-source comparison, full CLI benchmark, default-on/release-readiness proof, mmap receipt, config-wide speedup, 10x result, or product-level speed claim.

Latest Stage 54 receipts:

- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\010-package-version-benchmark-guard-red.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\020-package-version-benchmark-guard-green.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\030-cargo-fmt.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\040-package-version-benchmark-guard-after-fmt.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\041-surface-matrix-after-package-version-guard.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\042-git-diff-check-after-fmt.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\050-package-version-source-vs-machine-receipt.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\060-cargo-fmt-after-json-map.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\061-package-version-benchmark-guard-after-json-map.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\070-package-version-source-vs-machine-receipt.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\080-read-only-mode-after-package-version.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\081-mmap-coverage-after-package-version.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\082-surface-matrix-after-receipt.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\083-guard-suite-after-package-version.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\084-package-version-cache-family.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\090-package-version-benchmark-guard-final.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\091-surface-matrix-final.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\092-guard-suite-final.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\093-docs-guard-final.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\094-git-diff-check-final.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\095-plan-stage54-receipt-paths-check.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\100-package-version-narrow-claim-guard-red.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\101-cargo-fmt-after-narrow-claim.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\102-package-version-narrow-claim-guard-green.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\110-package-version-source-vs-machine-receipt-narrow-claim.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\111-cargo-fmt-after-assertions.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\112-wait-for-compiler-idle.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\120-package-version-source-vs-machine-receipt-idle.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\130-cargo-fmt-after-hit-counter.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\131-package-version-guard-after-hit-counter.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\132-compiler-idle-before-final-receipt.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\140-package-version-source-vs-machine-receipt-hit-counter.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\150-cargo-fmt-after-module-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\151-package-version-guard-after-module-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\152-compiler-idle-before-post-split-receipt.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\153-active-rust-processes-before-post-split-receipt.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\154-wait-for-compiler-idle-post-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\160-package-version-source-vs-machine-receipt-after-module-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\161-package-version-cache-family-after-module-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\170-package-version-benchmark-guard-final.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\171-surface-matrix-after-module-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\172-guard-suite-after-module-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\173-docs-guard-after-module-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\174-read-only-mode-after-module-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\175-mmap-coverage-after-module-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\176-cargo-fmt-check-after-module-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\177-plan-stage54-receipt-paths-check.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\178-git-diff-check-after-module-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\180-cargo-fmt-after-receipt-json-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\181-compiler-idle-before-receipt-json-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\182-wait-for-compiler-idle-before-receipt-json-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\183-active-rust-processes-after-wait.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\184-package-version-benchmark-guard-after-receipt-json-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\185-surface-matrix-after-receipt-json-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\186-guard-suite-after-receipt-json-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\187-mmap-coverage-after-receipt-json-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\188-wait-for-compiler-idle-before-final-rust-check.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\189-package-version-cache-family-after-receipt-json-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\190-compiler-idle-before-final-receipt-after-json-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\191-package-version-source-vs-machine-receipt-after-receipt-json-split.log`
- `G:\Dx\test-outputs\tauri-stage54-package-version-timing-receipt-20260531-a\package-version-source-vs-machine-receipt.json`

### Stage 55: Info App Config Harness

**Purpose:** Add a no-spawn isolated `tauri info` app/config section harness that crosses the existing project-config and package-version config paths without the full command's environment probes, network lookups, package-manager calls, or mobile/toolchain noise.

- [x] Add a private `app_config_section_items` seam for the App section so the full `info::command` behavior is preserved while tests can exercise only `get_config(Target::current(), &[], tauri_dir)` plus `info::app::items`.
- [x] Add a focused Stage 55 unit test that primes `.machine` files, reruns the App config section with `TAURI_DX_MACHINE_CACHE_WRITE=0`, verifies package-version `.machine` hit count, asserts `project-config.machine` and `package-version.machine` bytes are unchanged, and confirms the App section items have no action callbacks.
- [x] Add a lightweight TypeScript guard so Stage 55 remains a no-spawn app/config-section harness and does not become a full `tauri info` command benchmark or broad product claim.
- [x] Keep claim scope narrow: this is current-fork app/config-section evidence only, not a full CLI, official-release, upstream-source, default-on, release-readiness, app runtime, WebView, IPC, build, dev, bundle, watch, installer, mobile/toolchain, timing, or product-level claim.

Latest Stage 55 receipts:

- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\010-red-info-app-config-harness.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\021-green-info-app-config-harness-exact.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\030-cargo-fmt-after-info-app-config.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\040-info-app-config-harness-exact-after-fmt.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\041-package-version-cache-family-after-info-harness.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\042-surface-matrix-after-info-harness.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\043-read-only-mode-after-info-harness.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\050-cargo-fmt-check-final.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\051-info-app-config-harness-final.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\056-cargo-fmt-after-guard-fix.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\057-cargo-fmt-check-final.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\059-info-app-config-harness-final-after-fmt.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\060-guard-suite-final.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\062-git-diff-check-final.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\063-info-app-config-guard-final.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\064-surface-matrix-final.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\065-guard-suite-final-after-guard-fix.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\066-git-diff-check-final-after-guard-fix.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\067-plan-stage55-receipt-paths-check.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\068-docs-guard-after-stage55-plan.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\069-info-app-config-guard-after-stage55-plan.log`
- `G:\Dx\test-outputs\tauri-stage55-info-app-config-harness-20260531-a\070-plan-stage55-receipt-paths-check-final.log`

### Stage 56: Config Value Direct Archive Read

**Purpose:** Remove the remaining full-wrapper deserialize from `tauri-conf-*.machine` leaf config cache hits and guard the direct archived-field read style.

- [x] Add a mmap/direct-archive guard that fails if `tauri-conf-*.machine` cache hits reintroduce `deserialize_tauri_config_machine_archive`.
- [x] Change the leaf config reader to compare `archived.source_path.as_str()` directly and materialize only `archived.value` through the existing archived JSON tree reader.
- [x] Keep claim scope narrow: this is current-fork cache hit-path cleanup only, not a timing receipt, generic config-speedup claim, official-release comparison, upstream-source comparison, default-on proof, full CLI benchmark, app runtime, WebView, IPC, build, dev, bundle, watch, installer, mobile/toolchain, or product-level claim.

Latest Stage 56 receipts:

- `G:\Dx\test-outputs\tauri-stage56-config-value-direct-archive-20260531-a\010-mmap-coverage-red.log`
- `G:\Dx\test-outputs\tauri-stage56-config-value-direct-archive-20260531-a\020-rustfmt-machine-cache.log`
- `G:\Dx\test-outputs\tauri-stage56-config-value-direct-archive-20260531-a\030-mmap-coverage-green.log`
- `G:\Dx\test-outputs\tauri-stage56-config-value-direct-archive-20260531-a\040-tauri-utils-config-cache-tests.log`
- `G:\Dx\test-outputs\tauri-stage56-config-value-direct-archive-20260531-a\050-surface-matrix.log`
- `G:\Dx\test-outputs\tauri-stage56-config-value-direct-archive-20260531-a\060-guard-suite.log`
- `G:\Dx\test-outputs\tauri-stage56-config-value-direct-archive-20260531-a\070-git-diff-check.log`
- `G:\Dx\test-outputs\tauri-stage56-config-value-direct-archive-20260531-a\080-mmap-coverage-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage56-config-value-direct-archive-20260531-a\081-surface-matrix-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage56-config-value-direct-archive-20260531-a\082-docs-guard-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage56-config-value-direct-archive-20260531-a\083-guard-suite-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage56-config-value-direct-archive-20260531-a\084-git-diff-check-after-plan.log`
- `G:\Dx\test-outputs\tauri-stage56-config-value-direct-archive-20260531-a\090-docs-guard-final.log`
- `G:\Dx\test-outputs\tauri-stage56-config-value-direct-archive-20260531-a\091-guard-suite-final.log`
- `G:\Dx\test-outputs\tauri-stage56-config-value-direct-archive-20260531-a\092-git-diff-check-final.log`

---

## Claim Boundary

Stage 10's receipt-backed command benchmarks at git `af391adbd1e0fbc07ca4e84f3c77dc62abb2c695` remain the latest official-release comparison source in this plan. This repository may claim the exact measured command results only: on the benchmarked inspect fixture, after `.machine` generation was completed before timing, local current-source cache-on measured `50.854 ms` median versus local cache-off `64.078 ms` and the official v2.11.2 release binary measured `155.504 ms`, or `79.363%` of local cache-off median and `32.703%` of official median. On the fixed stable-v2 no-op migrate fixture, after `.machine` generation was completed before timing, local current-source cache-on measured `41.027 ms` median versus local cache-off `89.453 ms` and official release `94.448 ms`, or `45.864%` of local cache-off median and `43.439%` of official median. Stage 51 is the latest current-fork parser-vs-machine cargo-metadata hot-path source: after `cargo-metadata.machine` was generated before timing, the read-only `.machine` metadata path measured `11.317x` faster by median than the current-fork cache-disabled source parser path, not an official binary or upstream-source benchmark, and the latest labeled `get_watch_folders` receipt measured `13.708x` faster by median. Stage 53 is the latest package manifest/lock helper-level timing source: after `cargo-package-metadata.machine` was generated during setup before machine timing, the current-fork `cargo_manifest_and_lock` machine path measured `2.894 ms` median versus `50.553 ms` same-process source parse median, or `17.470x` faster by median. Stage 54 is the latest package-version helper-level timing source and a real narrow win: after `package-version.machine` was generated during setup before machine timing, the current-fork `load_config` package-version path measured `1.957 ms` median versus `9.741 ms` same-process source parse median, or `4.978x` faster by median. Stage 55 is a no-spawn isolated `tauri info` app/config section harness, not a full `tauri info` command benchmark; it proves warm-cache app/config behavior with writes disabled, byte-stable `.machine` artifacts, and a package-version machine hit. Stage 56 is a direct-archive cleanup for `tauri-conf-*.machine` leaf config hits, not a timing result. Stage 54 misses the 10x bar and is not an mmap receipt, and it does not add a full CLI, official-release, upstream-source, default-on, release-readiness, config-wide, or product-level claim. Stage 55 and Stage 56 do not add full CLI, official-release, upstream-source, default-on, release-readiness, app runtime, WebView, IPC, build, dev, bundle, watch, installer, mobile/toolchain, timing, generic config-speedup, or product-level claims. Stage 47's tiny full project-config fixture and Stage 48's tiny leaf-config fixture are negative timing results: validated `.machine` read measured `115%` and `274%` of source parse median respectively, so they cannot support config speedup claims. Stage 49's representative generated project-config fixture is positive at `45%` of source median, or about `2.22x` faster, but it is a single fixture-local same-process result and cannot support generic config speedup, broad Tauri superiority, or a 10x product-level win. The inspect/migrate official-release comparisons are fixture-local binary comparisons, not upstream-source, default-install, or full-workflow Tauri comparisons. The cargo-metadata/watch-folder/package-manifest/package-version numbers are current-fork parser-vs-machine hot-path measurements only. It must not claim broad Tauri superiority, default-on readiness, app runtime speed, WebView startup speed, IPC speed, full `tauri dev`, build/bundle/watch speed, publish readiness, generic migration speed, config-wide speedup, or a 10x product-level win.

## Completion Audit

Last audited: 2026-05-31

- Completed: `PLAN.md` exists in `G:\Dx\tauri`, records the serializer-cache architecture, the evidence root, current readiness, staged receipts, claim boundaries, and next action.
- Completed: Superpowers workflows and six GPT-5.5 xhigh sidecar agents were used for planning, review, and evidence cleanup; their useful findings are reflected in the Stage 55 wording cleanup.
- Completed: The project has professional commits on `dev` for the latest checkpoints, including `66b396b64 test(cli): cover info app config cache path`, `6cb5beb66 docs(plan): tighten info cache claim wording`, and `45def4285 docs(plan): add completion audit`.
- Completed: Focused checks are recorded under `G:\Dx\test-outputs`; the latest finalization pass avoided heavy builds and used targeted Rust/TypeScript guards.
- Completed: The measured official-release command fixtures are faster in the local fork: `inspect wix-upgrade-code` is `3.06x` faster by median, and `migrate-stable-v2-noop` is `2.30x` faster by median.
- Completed: Current-fork parser-vs-`.machine` hot paths have stronger narrow wins: `17.470x`, `13.708x`, `11.317x`, and `4.978x` by median on the recorded surfaces.
- Completed: Stage 56 continues production cache-hit cleanup by removing wrapper-level deserialization from `tauri-conf-*.machine` leaf config hits; this is not a new timing claim.
- Not complete: whole-product upstream superiority remains unproven. The evidence does not cover full `tauri dev`, build, bundle, watch, app runtime, WebView startup, IPC, installer, mobile/toolchain, default-install, upstream-source checkout, or default-on release behavior.
- Not complete: public release-readiness remains blocked while the local `dx-serializer` path dependency, MSRV/edition mismatch, cargo-vet/audit evidence, and packaging policy are unresolved.
- Goal status: do not mark the broad goal complete until either the scope is explicitly narrowed to the measured surfaces, or the missing full-product/upstream/default-on evidence is gathered.

## Next Immediate Action

After Stage 56, do not add another harness-only stage unless it produces a real timing receipt or removes a release-readiness blocker. The next performance lane should identify a stable, non-network `.machine`-crossing command or helper and record a source-vs-machine or official-release timing receipt with `.machine` generation excluded from timing; otherwise stop performance expansion and move to release-readiness. The best release-readiness work remains deciding the `dx-serializer` path: publish it, vendor it, or keep the cache feature explicitly local-only. Public release-readiness remains blocked until `dx-serializer` is safely vendored or published with compatible MSRV, cargo-vet/audit receipts, and modern-stable CI receipts. Keep broad claims blocked unless the checker explicitly allows them, and keep cache behavior default-off until packaging, MSRV, cargo-vet, and command-level regression evidence are resolved.
