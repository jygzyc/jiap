// decx-engine rusty_dex module: DEX parser (decx-native).
// Adapted from rusty-rs/rusty-dex 0.2.0 (Apache-2.0): https://github.com/rusty-rs/rusty-dex
// Baseline: the copy bundled in the asLody/dexdec v1.0.2 workspace.
// MODIFIED for decx-native: folded into decx-engine tests; crate reference rewritten.
// Provenance and modification policy: crates/decx-engine/VENDORED.md

use decx_engine::rusty_dex;

#[test]
/// Simple test to make sure we get all the methods
/// for apps using multidex
///
/// This test simply parse the test APK and checks the
/// number of methods in the `manymethods` package.
fn test_multidex() {
    let parsed = rusty_dex::parse("tests/multidex.apk").unwrap();

    let mut many_methods_count = 0;
    for method in parsed.methods.items.iter() {
        if method.starts_with("Lcom/example/multidextestapp/manymethods/") {
            many_methods_count += 1;
        }
    }

    /* 80 classes with 100 methods plus 1 constructor each */
    assert_eq!(many_methods_count, 80080);
}
