# decx-engine: provenance and attribution

`decx-engine` is the in-house DEX decompiler engine of the decx-native
workspace. Its core is **adapted third-party code**; this file records where
every part came from and what was changed, as required by Apache-2.0 §4(b).

| | |
|---|---|
| Crate | `decx-engine` — in-house name; see *Consolidation history* below |
| Origin 1 | [asLody/dexdec](https://github.com/asLody/dexdec) `v1.0.2` (annotated tag, commit `6c083d91f5b5541ca56b4d2775228b5123bdb72f`) — decompiler core → `src/**` |
| Origin 2 | [rusty-rs/rusty-dex](https://github.com/rusty-rs/rusty-dex) `0.2.0`, baseline = the copy bundled in the dexdec `v1.0.2` workspace — DEX parser → `src/rusty_dex/**`, `tests/multidex.rs` |
| Original code | `src/log.rs`, `src/rayon.rs`, `src/zstd.rs`, `src/crc32fast.rs` (dependency-free replacements written for decx-native; not upstream-derived) |
| Licenses | **Apache-2.0** — full texts kept verbatim in [`LICENSE.dexdec`](LICENSE.dexdec) and [`LICENSE.rusty-dex`](LICENSE.rusty-dex) |

The Apache-2.0 license permits inclusion in this (GPL-3.0) repository; the
Apache-2.0 terms continue to apply to the adapted code. Apache-2.0 §4(b)
requires modified files to carry prominent notices — that is what the file
headers below exist for.

## Consolidation history

Originally the engine was vendored as two third-party-looking crates,
`crates/dexdec` and `crates/rusty-dex`, plus four no-op stub crates under
`crates/shims/` (same package names as the `rayon` / `zstd` / `crc32fast` /
`log` dependencies they replace) so upstream `use` lines could stay untouched.

The workspace was then consolidated into the single in-house `decx-engine`
crate:

- `rusty-dex` folded in as the `src/rusty_dex/` module;
- the four shims folded in as crate-root modules (`src/log.rs` etc.);
- absolute paths inside `rusty_dex` rebased (`crate::…` →
  `crate::rusty_dex::…`) and shim references rewritten to crate-relative
  paths (`use log::…` → `use crate::log::…`, `rusty_dex::…` →
  `crate::rusty_dex::…`, `zstd::…` → `crate::zstd::…`, …).

Files whose content changed in that pass carry the *"folded into decx-engine
modules"* flavor of the MODIFIED header.

## File-header convention

Every adapted `.rs` file starts with a header naming the in-house crate, the
upstream origin and license, and one of exactly two states:

- *byte-identical to upstream apart from this header*;
- *MODIFIED for decx-native*, with a summary of what changed.

Current classification:

| Region | Total | MODIFIED | byte-identical |
|---|---|---|---|
| `src/**` (dexdec core) | 280 | 92 | 188 |
| `src/rusty_dex/**` | 22 | 20 | 2 |
| `tests/multidex.rs` | 1 | 1 | 0 |

When editing an adapted file, keep its header accurate and mark new changes
in-line with a `decx-native:` comment so future upstream syncs can find them.

## What was changed and why — dexdec core (`src/**`)

1. **Dependency replacement via root modules** — upstream `use rayon/zstd/
   crc32fast/log` lines resolve to the local no-op/identity modules instead
   of external crates (sequential / identity / stderr-gated implementations,
   zero external dependencies). Since the consolidation these are modules of
   this crate rather than stub crates.
2. **Dropped upstream-only surfaces** (not vendored at all):
   `src/bin/dexdec.rs` (CLI binary), all of `src/cli/` (24 files),
   `src/decoder/insn_decoder.rs`, and the upstream integration-test suite
   (`tests/*.rs` except the `tests/testcases/` fixture tree, which is kept
   byte-identical for future test work).
3. **`resources/symbols/platform.dexsym` repacked uncompressed** — upstream
   ships it zstd-compressed (2,085,124 bytes, sha256
   `480aa31680c9c3da89f4bcff435081a7f6d2eec509de96d9cc2dc186904cc365`);
   this copy is stored raw (14,283,619 bytes, sha256
   `2ad32a8d8e192629324f35b7a58c613e5278f81450560ab493857a45e4b3e7da`)
   because the `zstd` module is an identity passthrough (self-consistent
   read/write). Repacked once by `analysis/dexsym-repack`.
4. **Functional modifications across the modified files listed below**, in
   service of the decx-native server: batch class loading
   (`DexReader::load_classes`), render-input plumbing (`ClassRenderInput`)
   and `retain_decoded_methods` caching in `api.rs` / `api/decompiler.rs`,
   per-method parallelism toggle (`parallel_methods`) in the java/kotlin
   backends, value-recovery flow caches (`source_flow_cache` and friends),
   streaming `impl IntoIterator` signatures replacing `Vec` in the flow
   combinator APIs, small rustc-compat fixes (`.as_deref()` adjustments), and
   additional regression tests.

## What was changed and why — DEX parser (`src/rusty_dex/**`)

External dependencies were removed entirely (nothing here is shimmable
facade-style, so the code itself was rewritten):

- `zip` → `decx_apk::ZipArchive` (pure-std zip reader in this workspace;
  the engine crate depends on `decx-apk` for this);
- `thiserror` derives → hand-written `Display`/`Error` impls in `error.rs`;
- `regex` + `lazy_static` method-prototype parsing in `dex/classes.rs` →
  hand-written `method_name_from_proto` string scanning;
- `byteorder` reads in `dex/reader.rs` → plain `Read::read_exact` +
  `from_le_bytes`/`from_be_bytes`.

Plus small local fixes accumulated while validating against real-device APKs,
and the consolidation pass (`crate::` rebasing) described above.

## Modified files — dexdec core (92)

- `analysis/jadx_local_names.rs`
- `analysis/java_backend/declaration_lowering.rs`
- `analysis/java_backend/exception_contract.rs`
- `analysis/java_backend/generator.rs`
- `analysis/java_backend/java_model/method.rs`
- `analysis/java_backend/java_model/source_abi.rs`
- `analysis/java_backend/method_pipeline.rs`
- `analysis/java_backend/mod.rs`
- `analysis/java_backend/semantic_naming.rs`
- `analysis/java_backend/signature_inference.rs`
- `analysis/kotlin_backend/declaration_lowering.rs`
- `analysis/kotlin_backend/exception_contract.rs`
- `analysis/kotlin_backend/generator.rs`
- `analysis/kotlin_backend/kotlin_model/default_mask_flow.rs`
- `analysis/kotlin_backend/kotlin_model/method.rs`
- `analysis/kotlin_backend/kotlin_model/nullability.rs`
- `analysis/kotlin_backend/kotlin_model/source_abi.rs`
- `analysis/kotlin_backend/method_pipeline.rs`
- `analysis/kotlin_backend/mod.rs`
- `analysis/kotlin_backend/semantic_naming.rs`
- `analysis/kotlin_backend/signature_inference.rs`
- `analysis/method_override/tests.rs`
- `analysis/method_override.rs`
- `analysis/value_recovery/flow/collector.rs`
- `analysis/value_recovery/flow/gated.rs`
- `analysis/value_recovery/flow/planner.rs`
- `analysis/value_recovery/flow/reaching.rs`
- `analysis/value_recovery/flow.rs`
- `analysis/value_recovery/initialization.rs`
- `analysis/value_recovery/loops.rs`
- `analysis/value_recovery/numbering.rs`
- `analysis/value_recovery/predicate_regions/distribution.rs`
- `analysis/value_recovery/predicate_regions/guarded_assignment.rs`
- `analysis/value_recovery/predicate_regions.rs`
- `analysis/value_recovery/schedule.rs`
- `analysis/value_recovery/source.rs`
- `analysis/value_recovery/ssa_constants.rs`
- `analysis/value_recovery.rs`
- `api/decompiler.rs`
- `api.rs`
- `decoder/method_decoder.rs`
- `decoder/mod.rs`
- `frontend/dex_reader.rs`
- `frontend/metadata.rs`
- `ir/analysis/effects.rs`
- `ir/analysis/evaluation.rs`
- `ir/analysis/objects.rs`
- `ir/analysis/semantic_flow.rs`
- `ir/analysis/source_variables/edge_arguments.rs`
- `ir/analysis/source_variables/phi.rs`
- `ir/analysis/ssa.rs`
- `ir/analysis/termination.rs`
- `ir/analysis/types/constraints.rs`
- `ir/analysis/types/hierarchy.rs`
- `ir/analysis/variable_semantics.rs`
- `ir/block.rs`
- `ir/cfg.rs`
- `ir/exception/cleanup.rs`
- `ir/exception.rs`
- `ir/insn.rs`
- `ir/instruction_tree.rs`
- `ir/mod.rs`
- `ir/passes/bind_results.rs`
- `ir/passes/recover_constructors.rs`
- `ir/passes/split_monitor_entries.rs`
- `ir/region/synchronization.rs`
- `ir/region/tree.rs`
- `ir/semantic/completion.rs`
- `ir/semantic/expression.rs`
- `ir/semantic/instructions.rs`
- `ir/semantic/normalize.rs`
- `ir/semantic/sites.rs`
- `ir/semantic/string_building.rs`
- `ir/semantic.rs`
- `ir/splitter.rs`
- `language/java/dex.rs`
- `language/java/literals.rs`
- `language/java/mod.rs`
- `language/java/normalize.rs`
- `language/java/source_types.rs`
- `language/java/syntax/conditions.rs`
- `language/java/syntax/expression.rs`
- `language/java/syntax/loops/facts.rs`
- `language/kotlin/dex.rs`
- `language/kotlin/normalize.rs`
- `language/kotlin/source_types.rs`
- `language/kotlin/syntax/conditions.rs`
- `language/kotlin/syntax/expression.rs`
- `language/kotlin/syntax/loops/facts.rs`
- `platform_symbols/codec.rs`
- `profiling.rs`
- `visualizer/cfg_dot.rs`

## Modified files — rusty_dex (20)

- `adler32.rs`
- `dex/access_flags.rs`
- `dex/classes.rs`
- `dex/code_item.rs`
- `dex/debug_info.rs`
- `dex/declarations.rs`
- `dex/encoded_value.rs`
- `dex/fields.rs`
- `dex/file.rs`
- `dex/header.rs`
- `dex/instructions.rs`
- `dex/methods.rs`
- `dex/opcodes.rs`
- `dex/protos.rs`
- `dex/reader.rs`
- `dex/references.rs`
- `dex/strings.rs`
- `dex/types.rs`
- `error.rs`
- `mod.rs`

## Re-diffing against upstream

```bash
curl -sSL https://github.com/asLody/dexdec/archive/refs/tags/v1.0.2.tar.gz | tar xz
# dexdec core (note: src/rusty_dex/, the four root shim modules and
# tests/multidex.rs have no upstream counterpart there):
diff -r dexdec-1.0.2/dexdec/src crates/decx-engine/src || true
# DEX parser:
diff -r dexdec-1.0.2/rusty-dex/src crates/decx-engine/src/rusty_dex
# every hunk beyond the provenance header block is a deliberate local change
```

`tools/verify-vendor-headers.sh <upstream-checkout>` automates the
classification check; `tools/stamp-vendor-headers.sh <upstream-checkout>`
re-stamps headers after a sync.
