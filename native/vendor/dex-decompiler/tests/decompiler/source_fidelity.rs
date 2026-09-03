//! Source-fidelity loop: compare decompiled fixture output to Java sources + manifest.

use super::fixture_harness::{
    assert_apk_matches_dex_methods, assert_manifest_covers_catalog, check_all_fixtures,
    decompile_fixtures_apk, decompile_fixtures_dex,
};
use super::fixture_manifest::fixture_manifest;

#[test]
fn fixture_manifest_covers_all_public_static_methods() {
    assert_manifest_covers_catalog(fixture_manifest());
}

#[test]
fn all_fixture_methods_match_source_expectations_dex() {
    let decompiled = decompile_fixtures_dex();
    check_all_fixtures(&decompiled, fixture_manifest());
}

#[test]
fn all_fixture_methods_match_source_expectations_apk() {
    let decompiled = decompile_fixtures_apk();
    check_all_fixtures(&decompiled, fixture_manifest());
}

#[test]
fn apk_decompile_matches_dex_for_representative_methods() {
    assert_apk_matches_dex_methods(&[
        ("TryCatchFixtures", "nestedTry"),
        ("TryCatchFixtures", "tryWithResourcesTwo"),
        ("AlgorithmFixtures", "fibonacciIterative"),
        ("AlgorithmFixtures", "bubbleSort"),
        ("ArrayFixtures", "sum2d"),
        ("ControlFlowFixtures", "forLoopClassic"),
        ("AlgorithmFixtures", "fibonacciIterative"),
    ]);
}
