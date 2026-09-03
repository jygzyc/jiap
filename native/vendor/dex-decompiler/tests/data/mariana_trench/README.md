# Mariana Trench tests in dex-decompiler

This directory vendors and ports Meta’s [Mariana Trench](https://github.com/facebook/mariana-trench) test suite.

## Layout

| Path | Source |
|------|--------|
| `e2e/` | `source/tests/integration/end-to-end/code/*` (74 cases) |
| `e2e/*/models.json` | MT method models (generations/sinks/propagation/sanitizers) |
| `e2e/*/rules.json` | MT rules |
| `e2e/*/expected_issues.json` | Issues extracted from MT `expected_output.json` |
| `e2e/MANIFEST.json` | Case index (issue counts) |
| `local_flow_e2e/` | `source/tests/integration/local_flow_e2e/code/*` |
| `UNIT_TESTS.txt` | Names of MT C++ unit tests |

## Rust tests

| File | Role |
|------|------|
| `tests/mariana_trench_e2e.rs` | Load/convert all 74 cases; catalog + rule matching |
| `tests/mariana_trench_unit.rs` | Kind/Rule/Sanitizer/CallGraph/Port unit ports |
| `tests/mariana_trench_solver.rs` | Per-case expected counts + solver smoke |

Models are converted via `dex_decompiler::load_mt_case_config` / `convert_models_json`.

```bash
cargo test --test mariana_trench_e2e --test mariana_trench_unit --test mariana_trench_solver
```

## Notes

- Full binary parity with MT’s Redex-based runner would require compiling each Java/Kotlin case to DEX (needs JDK + `d8`). Fixtures + model conversion + expected-issue catalogs are checked for **all** cases today.
- Solver execution against Origin.source→sink DEX fixtures can be extended once a multi-method DEX builder lands; smoke tests already run `solve_dexes` with MT configs.
