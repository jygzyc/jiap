# jadx parity regression catalog

Maps [jadx integration tests](https://github.com/skylot/jadx/tree/master/jadx-core/src/test/java/jadx/tests/integration)
to `tests/decompiler/jadx_parity.rs` (and related unit tests).

| jadx test | Area | Coverage here |
| --------- | ---- | ------------- |
| `conditions/TestElseIf` | if / else-if | `jadx_conditions_TestElseIf_style`, `jadx_conditions_else_if_emit` (unit) |
| `conditions/TestConditions` | boolean CF | `jadx_conditions_TestConditions_style`, `jadx_conditions_short_circuit_and` / `_or` |
| `conditions` ternary assign | `x = c ? a : b` | `jadx_ternary_assign` |
| `loops/TestBreakInLoop` | loops | `jadx_loops_TestBreakInLoop_style` |
| `loops` for-each iterator | `for (T x : coll)` | `jadx_loops_foreach_iterator` |
| `loops` do-while | `do { } while` | `jadx_loops_do_while_text`, `jadx_loops_do_while_break_positive` / `_oneliner` / `_break_in_else` (unit) |
| `switches/TestSwitchSimple` | switch cases | `jadx_switches_TestSwitchSimple` |
| `switches/TestSwitchBreak` | case `break` | same (break count ≥ 5) |
| `switches/TestSwitchOverStrings` | string switch | `jadx_switches_TestSwitchOverStrings_restore` |
| `trycatch/TestTryCatch` | try/catch | `jadx_trycatch_TestTryCatch` |
| `trycatch/TestTryCatch2` | multi-try | `jadx_trycatch_multiple_methods_have_try` |
| `trycatch` multi-catch | `A \| B` | `jadx_trycatch_multi_catch` |
| `generics/TestGenerics` | wildcards | `jadx_generics_TestGenerics_wildcards` |
| `generics/*` Signature | class/method | `jadx_generics_*_signature*` |
| `inner/TestInnerClass` | nested named | `jadx_inner_TestInnerClass_nested`, `jadx_inner_TestSynthetic_Bridge_nested` |
| `inner/TestAnonymousClass` | anon Thread | `jadx_inner_TestAnonymousClass_thread_site` |
| (style) requireNonNull strip | Objects / Intrinsics | `jadx_strip_requireNonNull` |
| (style) fill-array-data | `new int[]{…}` | `simplify_fill_array_new_type` |
| `switches` enum `$SwitchMap` | `switch (e)` + `case Color.RED` | `jadx_switches_enum_switchmap` |
| `trycatch` try-with-resources | `try (R r = …)` | `jadx_try_with_resources` |
| `trycatch` multi-resource TWR | `try (A a; B b)` | `jadx_try_with_resources_multi` |
| (style) redundant casts | `(T) new T` / `(T)(T)` | `jadx_strip_redundant_cast` |
| (style) StringConcatFactory | `a + b` | `jadx_string_concat_indy` (unit) |
| Kotlin `@Metadata` | d1/d2 + NameResolver | `kotlin::tests` (BitEncoding, base64, wire walk, NameResolver, comment_prefix) |
| finally heuristic | cleanup vs handle | `looks_like_finally_*` in `try_catch.rs` |
| duplicate-path finally | peel common suffix → `finally` | `merge_duplicate_finally_peels_close` |
| string-switch harden | lit-first / hash temp / verify | `restores_literal_first_equals`, `folds_hashcode_temp_disc`, `rejects_wrong_hash_keeps_numeric` |
| mapping save | PG/Tiny/Enigma export | `proguard_roundtrip`, `tiny_and_enigma_roundtrip` |

DEX fixture: `testdata/androguard_test_classes.dex` (androguard `TestIfs`, `TestLoops`, `TestExceptions`, `TestSynthetic`, `TestDefaultPackage`).

To extend: pick a jadx `IntegrationTest`, port `contains` / `doesNotContain` / `countString` assertions, and either use the androguard DEX or a Signature/text-level unit test when compiling Java→DEX is unavailable.
