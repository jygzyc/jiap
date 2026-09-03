//! APK → DEX load → decompiler pipeline (same entry as CLI `-i decompiler_fixtures.apk`).

use dex_decompiler::{load_dexes_from_path, Decompiler, DecompilerOptions};
use dex_parser::CodeItem;

use super::try_catch_assertions::{
    assert_all_try_catch_fixtures, fixtures_apk_path, fixtures_dex_path, TRY_CATCH_CLASS,
};

fn decompile_apk_fixtures() -> String {
    let path = fixtures_apk_path();
    let dexes =
        load_dexes_from_path(&path).unwrap_or_else(|e| panic!("load {}: {e}", path.display()));
    assert!(!dexes.is_empty(), "APK produced no DEX");
    let options = DecompilerOptions {
        only_package: Some("com.androguard.decompilefixtures".to_string()),
        exclude: vec![],
        ..Default::default()
    };
    Decompiler::with_options(&dexes[0], options)
        .decompile()
        .expect("decompile APK dex")
}

fn try_catch_fixture_code_from_dex(name: &str) -> CodeItem {
    let data = std::fs::read(fixtures_dex_path()).unwrap();
    let dex = dex_decompiler::parse_dex(&data).unwrap();
    for class_def in dex.class_defs().flatten() {
        let class_type = dex.get_type(class_def.class_idx).unwrap();
        if dex_decompiler::java::descriptor_to_java(&class_type) != TRY_CATCH_CLASS {
            continue;
        }
        let cd = dex.get_class_data(&class_def).unwrap().unwrap();
        for enc in cd.direct_methods.iter().chain(cd.virtual_methods.iter()) {
            let info = dex.get_method_info(enc.method_idx).unwrap();
            if info.name == name {
                return dex.get_code_item(enc.code_off).unwrap();
            }
        }
    }
    panic!("method {name} not found");
}

#[test]
fn apk_load_decompiles_try_catch_fixtures() {
    let java = decompile_apk_fixtures();
    assert!(
        java.contains("class TryCatchFixtures"),
        "missing TryCatchFixtures class"
    );
    assert_all_try_catch_fixtures(&java);
}

#[test]
fn apk_dex_matches_standalone_dex_for_exception_metadata() {
    // APK-extracted DEX must ship the same try metadata as classes.dex (guards stale APK rebuilds).
    for name in [
        "nestedTry",
        "simpleTryCatch",
        "tryFinally",
        "tryWithResources",
        "multiCatch",
    ] {
        let code = try_catch_fixture_code_from_dex(name);
        assert!(
            code.tries_size > 0,
            "{name}: standalone classes.dex must retain tries_size > 0"
        );
    }
    let path = fixtures_apk_path();
    let apk_dexes = load_dexes_from_path(&path).unwrap();
    let apk_dex = &apk_dexes[0];
    for name in ["nestedTry", "tryFinally", "tryWithResources"] {
        let mut apk_tries = None;
        for class_def in apk_dex.class_defs().flatten() {
            let class_type = apk_dex.get_type(class_def.class_idx).unwrap();
            if dex_decompiler::java::descriptor_to_java(&class_type) != TRY_CATCH_CLASS {
                continue;
            }
            let cd = apk_dex.get_class_data(&class_def).unwrap().unwrap();
            for enc in cd.direct_methods.iter().chain(cd.virtual_methods.iter()) {
                let info = apk_dex.get_method_info(enc.method_idx).unwrap();
                if info.name == name {
                    apk_tries = Some(apk_dex.get_code_item(enc.code_off).unwrap().tries_size);
                }
            }
        }
        let standalone = try_catch_fixture_code_from_dex(name).tries_size;
        assert_eq!(
            apk_tries,
            Some(standalone),
            "{name}: APK DEX tries_size should match classes.dex"
        );
    }
}
