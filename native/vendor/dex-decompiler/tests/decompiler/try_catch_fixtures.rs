//! Strong regression tests for try/catch/finally/try-with-resources on real javac→d8 fixtures.
//!
//! Locks both DEX exception-table metadata and decompiled Java shape (standalone classes.dex path).

use dex_decompiler::{parse_dex, Decompiler, DecompilerOptions, DexFile};
use dex_parser::CodeItem;

use super::try_catch_assertions::{
    assert_all_try_catch_fixtures, fixtures_dex_path, method_region, TRY_CATCH_CLASS,
};

fn load_fixtures_dex() -> DexFile {
    let data = std::fs::read(fixtures_dex_path()).unwrap();
    parse_dex(&data).unwrap()
}

fn decompile_try_catch_fixtures() -> String {
    let dex = load_fixtures_dex();
    let options = DecompilerOptions {
        only_package: Some("com.androguard.decompilefixtures".to_string()),
        exclude: vec![],
        ..Default::default()
    };
    Decompiler::with_options(&dex, options)
        .decompile()
        .expect("decompile fixtures dex")
}

fn try_catch_fixture_code(dex: &DexFile, name: &str) -> CodeItem {
    for class_def in dex.class_defs().flatten() {
        let class_type = dex.get_type(class_def.class_idx).unwrap();
        if dex_decompiler::java::descriptor_to_java(&class_type) != TRY_CATCH_CLASS {
            continue;
        }
        let Some(cd) = dex.get_class_data(&class_def).unwrap() else {
            continue;
        };
        for enc in cd.direct_methods.iter().chain(cd.virtual_methods.iter()) {
            let info = dex.get_method_info(enc.method_idx).unwrap();
            if info.name == name {
                return dex.get_code_item(enc.code_off).unwrap();
            }
        }
    }
    panic!("method {name} not found in {TRY_CATCH_CLASS}");
}

#[test]
fn fixtures_dex_retains_exception_tables_for_try_methods() {
    let dex = load_fixtures_dex();
    for name in [
        "simpleTryCatch",
        "nestedTry",
        "tryFinally",
        "tryWithResources",
        "tryWithResourcesTwo",
        "multiCatch",
    ] {
        let code = try_catch_fixture_code(&dex, name);
        assert!(
            code.tries_size > 0,
            "{name} must retain DEX try/catch metadata (tries_size > 0)"
        );
    }
}

#[test]
fn standalone_dex_decompiles_all_exception_fixtures() {
    let java = decompile_try_catch_fixtures();
    assert!(java.contains("class TryCatchFixtures"));
    assert_all_try_catch_fixtures(&java);
}

#[test]
fn nested_try_method_region_is_stable() {
    let java = decompile_try_catch_fixtures();
    let body = method_region(&java, "nestedTry");
    assert!(
        !body.contains("simpleTryCatch") && !body.contains("tryFinally"),
        "method region should not include following methods; got:\n{body}"
    );
    super::try_catch_assertions::assert_nested_try(&body);
}
