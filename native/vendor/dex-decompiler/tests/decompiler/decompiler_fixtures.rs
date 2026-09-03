//! Regression lock for the decompiler fixtures APK / classes.dex.
//!
//! Loads the checked-in `testdata/decompiler_fixtures/classes.dex` (real javac→d8 bytecode)
//! and asserts decompiled Java retains expected structure for each scenario.
//!
//! Assertions match current decompiler behavior on real Dalvik — not ideal Java source.
//! When decompilation improves, tighten expectations here and in `fixture_manifest.rs`.
//!
//! Source-fidelity loop (manifest + Java sources): see `source_fidelity.rs` and
//! `docs/FIXTURE_SOURCE_FIDELITY.md`.

use super::fixture_harness::{decompile_fixtures_dex, method_region};
use dex_decompiler::{load_dexes_from_path, Decompiler};

fn decompile_fixtures() -> String {
    decompile_fixtures_dex()
}

fn assert_region_contains(method: &str, needle: &str, java: &str) {
    let body = method_region(java, method);
    assert!(
        body.contains(needle),
        "{method} should contain `{needle}`; got:\n{body}"
    );
}

#[test]
fn fixtures_dex_parses_and_decompiles() {
    let java = decompile_fixtures();
    for class in [
        "class ArrayFixtures",
        "class ControlFlowFixtures",
        "class SwitchFixtures",
        "class TryCatchFixtures",
        "class InvokeFixtures",
        "class NamingFixtures",
        "class InnerFixtures",
        "class SyncFixtures",
        "class LambdaFixtures",
        "class ConstFixtures",
        "class AlgorithmFixtures",
        "class CryptoFixtures",
        "enum Color",
    ] {
        assert!(java.contains(class), "missing {class} in decompiled output");
    }
}

#[test]
fn control_flow_if_else_and_loops() {
    let java = decompile_fixtures();
    assert_region_contains("ifElseChain", "if (", &java);
    assert_region_contains("ifElseChain", "return", &java);
    assert_region_contains("whileLoop", "for (", &java);
    let dow = method_region(&java, "doWhileLoop");
    assert!(
        dow.contains("do {") && dow.contains("} while (") && dow.contains("return i"),
        "doWhileLoop should decompile to do-while; got:\n{dow}"
    );
    assert!(
        !dow.contains("while (true)"),
        "doWhileLoop should not use while(true); got:\n{dow}"
    );
    assert_region_contains("forLoopClassic", "for (", &java);
    let break_loop = method_region(&java, "breakInLoop");
    assert!(
        break_loop.contains("for (") || break_loop.contains("while ("),
        "breakInLoop should decompile to a loop; got:\n{break_loop}"
    );
    assert_region_contains("forEachArray", "for (", &java);
    assert_region_contains("forEachList", "iterator", &java);
    let demo = method_region(&java, "demoForEach");
    assert!(
        (demo.contains("forEachArray(new int[]{1, 2, 3})")
            || demo.contains("forEachArray(new int[]{1,2,3})"))
            && (demo.contains("Arrays.asList(4, 5)") || demo.contains("Arrays.asList(4,5)")),
        "demoForEach should match source-like foreach helpers; got:\n{demo}"
    );
    assert!(
        !demo.contains("new Integer[arr1]") && !demo.contains("arr1[k]"),
        "demoForEach must not keep broken Integer[] init; got:\n{demo}"
    );
}

#[test]
fn control_flow_short_circuit_and_ternary() {
    let java = decompile_fixtures();
    assert_region_contains("nestedIfAnd", "&&", &java);
    assert_region_contains("shortCircuitOr", "if (a || b)", &java);
    assert_region_contains("shortCircuitOr", "return true;", &java);
    assert_region_contains("shortCircuitOr", "return false;", &java);
    assert_region_contains("assignTernary", "if (", &java);
}

#[test]
fn switches_packed_sparse_string_enum() {
    let java = decompile_fixtures();
    assert_region_contains("packedSwitch", "switch (", &java);
    assert_region_contains("sparseSwitch", "switch (", &java);
    assert_region_contains("switchOnString", "switch (", &java);
    assert_region_contains("switchOnString", "hashCode", &java);
    assert_region_contains("switchOnEnum", "switch (", &java);
    assert_region_contains("switchOnEnum", "ordinal()", &java);
}

#[test]
fn enum_emission_and_values_array() {
    let java = decompile_fixtures();
    assert!(java.contains("enum Color"));
    assert_region_contains("allColors", "values()", &java);
    assert!(
        java.contains("$values()")
            && java.contains("new com.androguard.decompilefixtures.EnumFixtures$Color[]"),
        "enum $values should emit filled array"
    );
}

#[test]
fn try_catch_finally_and_multi_catch() {
    // Strong assertions live in try_catch_fixtures; keep a smoke check here.
    let java = decompile_fixtures();
    assert!(java.contains("class TryCatchFixtures"));
    for method in [
        "nestedTry",
        "simpleTryCatch",
        "multiCatch",
        "tryFinally",
        "tryWithResources",
    ] {
        assert!(
            java.contains(&format!("{method}(")),
            "missing {method} in fixtures output"
        );
    }
}

#[test]
fn try_with_resources_desugar() {
    // Covered by try_catch_fixtures::try_with_resources_single_and_multi.
    let java = decompile_fixtures();
    assert!(java.contains("tryWithResources("));
    assert!(java.contains("tryWithResourcesTwo("));
}

#[test]
fn arrays_filled_new_and_element_init() {
    let java = decompile_fixtures();
    assert_region_contains("newIntArray", "new int[]{", &java);
    assert_region_contains("newStringArray", "new String[3]", &java);
    assert_region_contains("newStringArray", "result[0]", &java);
    assert_region_contains("fillArrayData", "for (", &java);
    assert_region_contains("sum2d", "for (int[] row : m)", &java);
    let fill = method_region(&java, "fillArrayData");
    assert!(
        !fill.contains("i0") && !fill.contains("= i0"),
        "fillArrayData should inline or declare temps, not use bare i0:\n{fill}"
    );
    let demo = method_region(&java, "demoArrays");
    assert!(
        (demo.contains("copyOf") && demo.contains("a.length")) || demo.contains("sum2d(m)"),
        "demoArrays should inline length into copyOf or call sum2d; got:\n{demo}"
    );
    assert!(
        !demo.contains("int length ="),
        "demoArrays should not keep length temp; got:\n{demo}"
    );
}

#[test]
fn invoke_chains_and_null_checks() {
    let java = decompile_fixtures();
    assert_region_contains("chainedStringOps", "trim()", &java);
    assert_region_contains("nullCheckContext", "getPackageName()", &java);
    assert_region_contains("nullCheckContext", "!= null", &java);
    assert_region_contains("invokeResultInCondition", "getSystemService", &java);
    assert_region_contains("builderChain", "StringBuilder", &java);
}

#[test]
fn naming_filled_array_static_fields() {
    let java = decompile_fixtures();
    let body = method_region(&java, "tokenValuesArray");
    assert!(
        body.contains("Token.A") && body.contains("result[0]"),
        "tokenValuesArray should reference static fields and store into array; got:\n{body}"
    );
    assert_region_contains("nullCheckAfterInvoke", "getService", &java);
}

#[test]
fn inner_and_anonymous_classes() {
    let java = decompile_fixtures();
    assert!(java.contains("class Outer"));
    assert!(java.contains("class Inner") || java.contains("Outer$Inner"));
    assert_region_contains("anonymousRunnable", "InnerFixtures$1", &java);
    assert_region_contains("localClassCapture", "localClassCapture", &java);
}

#[test]
fn synchronized_monitor_enter_exit() {
    let java = decompile_fixtures();
    assert_region_contains("syncMethod", "synchronized (", &java);
    let sync_block = method_region(&java, "syncBlock");
    assert!(
        sync_block.contains("synchronized (") || sync_block.contains("monitor-enter"),
        "syncBlock should use synchronized or monitor-enter; got:\n{sync_block}"
    );
    let sync_this = method_region(&java, "syncOnThis");
    assert!(
        sync_this.contains("synchronized (") || sync_this.contains("monitor-enter"),
        "syncOnThis should use synchronized or monitor-enter; got:\n{sync_this}"
    );
}

#[test]
fn lambdas_desugared_to_synthetic_classes() {
    let java = decompile_fixtures();
    assert!(
        java.contains("ExternalSyntheticLambda") || java.contains("lambda$identityLambda"),
        "lambdas should desugar to synthetics"
    );
    assert_region_contains("applyLambda", "applyAsInt", &java);
    assert_region_contains("methodRef", "ExternalSyntheticLambda", &java);
    let lambda_fixtures = java
        .split("class LambdaFixtures")
        .nth(1)
        .and_then(|s| s.split("\npublic ").next())
        .unwrap_or("");
    assert!(
        !lambda_fixtures.contains("$r8$lambda$"),
        "R8 lambda shims should not appear in LambdaFixtures source:\n{lambda_fixtures}"
    );
}

#[test]
fn constants_casts_instanceof() {
    let java = decompile_fixtures();
    assert_region_contains("wideConst", "return", &java);
    let wide = method_region(&java, "wideConst");
    assert!(
        wide.contains("0x1234567890ABCDEFL"),
        "wideConst should format as hex long; got:\n{wide}"
    );
    let double_c = method_region(&java, "doubleConst");
    assert!(
        double_c.contains("3.141592653589793"),
        "doubleConst should decode wide bits as double; got:\n{double_c}"
    );
    assert!(
        !double_c.contains("4614256656552045848"),
        "doubleConst should not keep raw wide bits; got:\n{double_c}"
    );
    assert_region_contains("castsAndInstanceof", "instanceof", &java);
    let casts = method_region(&java, "castsAndInstanceof");
    assert!(
        casts.contains("if (o instanceof Number)")
            && casts.contains("if (o instanceof CharSequence)")
            && casts.contains("((Number) o).intValue()")
            && casts.contains("((CharSequence) o).length()")
            && casts.contains("return -1;"),
        "castsAndInstanceof should match sequential instanceof tests; got:\n{casts}"
    );
    assert!(
        !casts.contains("== null") && !casts.contains("instanceof Number == "),
        "castsAndInstanceof should not compare instanceof to null/0; got:\n{casts}"
    );
    assert_region_contains("manyStrings", "one", &java);
}

#[test]
fn demo_algorithms_no_undefined_temps() {
    let java = decompile_fixtures();
    let demo = method_region(&java, "demoAlgorithms");
    for bad in ["local2", "i11", "i12"] {
        assert!(
            !demo.contains(bad),
            "demoAlgorithms should not reference `{bad}`; got:\n{demo}"
        );
    }
    assert!(
        demo.contains("binarySearch(arr, 4)"),
        "demoAlgorithms search key; got:\n{demo}"
    );
    assert!(
        demo.contains("quickSort(arr, 0, length - 1)"),
        "demoAlgorithms quickSort bounds; got:\n{demo}"
    );
    assert!(
        demo.contains("bfsShortestPath(") && demo.contains(", 0, 2)"),
        "demoAlgorithms bfs should use start 0 and target 2; got:\n{demo}"
    );
    assert!(
        !demo.contains("; i9)") && !demo.contains("); i9"),
        "demoAlgorithms must not splice leftover tokens onto the bfs call:\n{demo}"
    );
    assert!(
        demo.contains("mergeSort(sorted)") || demo.contains("mergeSort(new int[]"),
        "demoAlgorithms mergeSort should take the array, not a scalar; got:\n{demo}"
    );
}

#[test]
fn partition_decompiles_with_for_loop() {
    let java = decompile_fixtures();
    let body = method_region(&java, "partition");
    assert!(
        body.contains("for (int j = lo; j < hi; j++)"),
        "partition should use a for-loop over j; got:\n{body}"
    );
    assert!(
        !body.contains("while (j < hi)"),
        "partition should not use j before declaration; got:\n{body}"
    );
    assert!(
        body.contains("i++;") || body.contains("i = i + 1"),
        "partition should increment i in the loop body; got:\n{body}"
    );
    for bad in ["i0", "i1", "local2"] {
        assert!(
            !body.contains(bad),
            "partition should not reference undefined `{bad}`; got:\n{body}"
        );
    }
}

#[test]
fn bubble_sort_decompiles_without_undefined_temps() {
    let java = decompile_fixtures();
    let body = method_region(&java, "bubbleSort");
    assert!(
        !body.contains("local2"),
        "bubbleSort should not reference undefined local2; got:\n{body}"
    );
}

#[test]
fn algorithm_sorting_search_and_sieve() {
    let java = decompile_fixtures();
    assert_region_contains("bubbleSort", "for (int i = 0; i < n - 1; i++)", &java);
    assert_region_contains("bubbleSort", "for (int j = 0; j < n - i - 1; j++)", &java);
    assert_region_contains("bubbleSort", "arr[", &java);
    assert_region_contains("binarySearch", "while (", &java);
    assert_region_contains("binarySearch", "arr[", &java);
    let binary = method_region(&java, "binarySearch");
    assert!(
        binary.contains("target") && !binary.contains("i7"),
        "binarySearch should name the key param and avoid stale temps; got:\n{binary}"
    );
    assert!(
        !binary.contains("!= v") && !binary.contains("< v") && !binary.contains("> v"),
        "binarySearch conditions should not keep raw vN registers; got:\n{binary}"
    );
    assert_region_contains("sieveOfEratosthenes", "while (", &java);
    assert_region_contains("gcdEuclid", "while (", &java);
    let gcd = method_region(&java, "gcdEuclid");
    assert!(
        gcd.contains("a = t") || gcd.contains("a = b"),
        "gcdEuclid should update a in the loop; got:\n{gcd}"
    );
    assert!(
        gcd.contains("int t = b") || gcd.contains("b = a % b"),
        "gcdEuclid should keep the swap temp pattern; got:\n{gcd}"
    );
    assert!(
        !gcd.contains("!= null"),
        "gcdEuclid loop should compare b to 0, not null; got:\n{gcd}"
    );
}

#[test]
fn algorithm_recursion_and_divide_conquer() {
    let java = decompile_fixtures();
    assert_region_contains("fibonacciRecursive", "fibonacciRecursive(", &java);
    assert_region_contains("quickSort", "quickSort(", &java);
    let qs = method_region(&java, "quickSort");
    assert!(
        qs.contains("p - 1") && qs.contains("p + 1") && !qs.contains("i1"),
        "quickSort should inline reused bounds temps; got:\n{qs}"
    );
    assert_region_contains("mergeSort", "mergeSort(", &java);
    let fib = method_region(&java, "fibonacciIterative");
    assert!(
        fib.contains("for (int i = 2; i <= n; i++)")
            && fib.contains("int c = a + b")
            && fib.contains("a = b")
            && fib.contains("b = c")
            && !fib.contains("while (2 <= n)"),
        "fibonacciIterative should decompile to a counting for-loop; got:\n{fib}"
    );
    assert!(
        !fib.contains("i0") && !fib.contains("i3") && !fib.contains("i4") && !fib.contains("local"),
        "fibonacciIterative must not leak SSA temps; got:\n{fib}"
    );
    let demo = method_region(&java, "demoAlgorithms");
    assert!(
        demo.contains("new int[][]{ { 0, 1, 0 }, { 1, 0, 1 }, { 0, 1, 0 } }"),
        "demoAlgorithms should fold row literals into 2D init; got:\n{demo}"
    );
    assert!(
        !demo.contains("int[] dist") && !demo.contains("int[] arr5"),
        "demoAlgorithms should not keep single-use row temps; got:\n{demo}"
    );
}

#[test]
fn fibonacci_iterative_decompiles_without_ssa_temps() {
    let fib = method_region(&decompile_fixtures(), "fibonacciIterative");
    assert!(
        fib.contains("for (int i = 2; i <= n; i++)"),
        "missing counting for-loop:\n{fib}"
    );
    assert!(
        fib.contains("int c = a + b") && fib.contains("a = b") && fib.contains("b = c"),
        "missing fibonacci swap/update:\n{fib}"
    );
    for bad in ["i0", "i1", "i2", "i3", "i4", "local0", "local1"] {
        assert!(!fib.contains(bad), "leaked SSA temp `{bad}`:\n{fib}");
    }
}

#[test]
fn algorithm_graph_bfs() {
    let java = decompile_fixtures();
    assert_region_contains("bfsShortestPath", "while (", &java);
    assert_region_contains("bfsShortestPath", "graph[", &java);
}

#[test]
fn crypto_hash_mac_and_digest() {
    let java = decompile_fixtures();
    assert_region_contains("sha256", "MessageDigest", &java);
    assert_region_contains("sha256", "SHA-256", &java);
    assert_region_contains("hmacSha256", "Mac", &java);
    assert_region_contains("hmacSha256", "HmacSHA256", &java);
    assert_region_contains("secureRandomBytes", "SecureRandom", &java);
}

#[test]
fn crypto_cipher_and_key_derivation() {
    let java = decompile_fixtures();
    assert_region_contains("aesCbcEncrypt", "Cipher", &java);
    assert_region_contains("aesCbcEncrypt", "AES", &java);
    assert_region_contains("aesCbcDecrypt", "doFinal", &java);
    assert_region_contains("pbkdf2Sha256", "PBKDF2", &java);
    assert_region_contains("pbkdf2Sha256", "SecretKeyFactory", &java);
}

#[test]
fn crypto_stream_xor_and_constant_time_compare() {
    let java = decompile_fixtures();
    assert_region_contains("xorStream", "while (", &java);
    assert_region_contains("xorStream", "^", &java);
    assert_region_contains("constantTimeEquals", "while (", &java);
    assert_region_contains("constantTimeEquals", "^", &java);
    let demo = method_region(&java, "demoCrypto");
    assert!(
        !demo.contains(".length = "),
        "demoCrypto must not assign to .length:\n{demo}"
    );
    assert!(
        demo.contains("+ mac.length + enc.length +"),
        "demoCrypto should sum crypto buffer lengths:\n{demo}"
    );
    assert!(
        demo.contains("+ xored.length + (same ? 1 : 0);"),
        "demoCrypto return should include xored length and boolean same:\n{demo}"
    );
    assert!(
        demo.contains("return hash.length +") || demo.contains("return arr1.length +"),
        "demoCrypto return should start with sha256 buffer length:\n{demo}"
    );
    assert!(
        demo.contains("+ derived.length +") || demo.contains("+ arr9.length +"),
        "demoCrypto return should include pbkdf2 buffer length:\n{demo}"
    );
}

#[test]
fn apk_loads_via_load_dexes_from_path() {
    let path = super::try_catch_assertions::fixtures_apk_path();
    let dexes =
        load_dexes_from_path(&path).unwrap_or_else(|e| panic!("load {}: {e}", path.display()));
    assert!(!dexes.is_empty(), "APK produced no DEX files");
    let java = Decompiler::new(&dexes[0])
        .decompile()
        .expect("decompile apk dex");
    assert!(java.contains("ControlFlowFixtures"));
    assert!(java.contains("TryCatchFixtures"));
}

#[test]
fn sum2d_nested_foreach() {
    let java = decompile_fixtures();
    let body = method_region(&java, "sum2d");
    assert!(
        body.contains("for (int[] row : m)"),
        "expected outer for-each in sum2d:\n{body}"
    );
    assert!(
        body.contains("for (int v : row)"),
        "expected inner for-each in sum2d:\n{body}"
    );
    assert!(
        !body.contains("while ("),
        "sum2d should not keep index while loops:\n{body}"
    );
}

#[test]
fn merge_decompiles_with_array_alloc_and_loop_indices() {
    let java = decompile_fixtures();
    let body = method_region(&java, "merge");
    assert!(
        body.contains("new int[left.length + right.length]"),
        "merge should fold output buffer size:\n{body}"
    );
    assert!(
        body.contains("int i = 0") && body.contains("int j = 0"),
        "merge should keep loop index inits:\n{body}"
    );
    assert!(
        !body.contains("i = ] out") && !body.contains("left.length + 0"),
        "merge must not corrupt array alloc or length sum:\n{body}"
    );
    assert!(
        body.contains("while (") && body.contains("left[i]") && body.contains("right[j]"),
        "merge should retain merge-loop structure:\n{body}"
    );
    assert!(
        body.contains("return out"),
        "merge should return the output array:\n{body}"
    );
    assert!(
        !body.contains("while (true)"),
        "merge should use structured while loops, not while(true):\n{body}"
    );
}

#[test]
fn merge_decompiles_full_structure() {
    let body = method_region(&decompile_fixtures(), "merge");
    assert!(
        body.contains("return out"),
        "merge must return out:\n{body}"
    );
    assert!(
        body.matches("while (").count() >= 3,
        "merge should have main + two tail while loops:\n{body}"
    );
    assert!(body.contains("left[i]"), "merge must drain left:\n{body}");
    assert!(body.contains("right[j]"), "merge must drain right:\n{body}");
    assert!(
        body.contains("out[k++] = left[i++]") && body.contains("out[k++] = right[j++]"),
        "merge should use postincrement copies:\n{body}"
    );
    assert!(
        body.contains("while (i < left.length && j < right.length)"),
        "merge main loop should test both indexes:\n{body}"
    );
    assert!(
        !body.contains("k_0") && !body.contains("j_0") && !body.contains("out[0]"),
        "merge should not leak SSA temps or out[0]:\n{body}"
    );
    assert!(
        !body.contains("while (true)"),
        "merge must not use infinite while(true):\n{body}"
    );
}
