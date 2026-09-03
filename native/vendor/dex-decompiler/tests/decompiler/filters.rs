//! Package/class filter tests: --only-package and --exclude.

use super::helpers::minimal_dex_with_method_code;
use dex_decompiler::{parse_dex, Decompiler, DecompilerOptions, DexFile};

fn dex_with_one_class() -> (DexFile, String) {
    let dex_bytes = minimal_dex_with_method_code(&[0x0e, 0x00]); // return-void
    let dex = parse_dex(&dex_bytes).unwrap();
    // minimal_dex_with_method_code produces class LSimple; -> "Simple"
    (dex, "Simple".to_string())
}

#[test]
fn test_filter_no_options_includes_class() {
    let (dex, class_name) = dex_with_one_class();
    let dc = Decompiler::new(&dex);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains(&format!("class {}", class_name)),
        "without filter, output should contain class {}",
        class_name
    );
}

#[test]
fn test_filter_only_package_matching_includes_class() {
    let (dex, class_name) = dex_with_one_class();
    let options = DecompilerOptions {
        only_package: Some(class_name.clone()),
        exclude: vec![],
        ..Default::default()
    };
    let dc = Decompiler::with_options(&dex, options);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains(&format!("class {}", class_name)),
        "only_package matching class should include it"
    );
}

#[test]
fn test_filter_only_package_non_matching_excludes_class() {
    let (dex, class_name) = dex_with_one_class();
    let options = DecompilerOptions {
        only_package: Some("other.package".to_string()),
        exclude: vec![],
        ..Default::default()
    };
    let dc = Decompiler::with_options(&dex, options);
    let java = dc.decompile().unwrap();
    assert!(
        !java.contains(&format!("class {}", class_name)),
        "only_package not matching should exclude class"
    );
    assert!(java.trim().is_empty(), "output should be empty");
}

#[test]
fn test_filter_exclude_class_excludes_it() {
    let (dex, class_name) = dex_with_one_class();
    let options = DecompilerOptions {
        only_package: None,
        exclude: vec![class_name.clone()],
        ..Default::default()
    };
    let dc = Decompiler::with_options(&dex, options);
    let java = dc.decompile().unwrap();
    assert!(
        !java.contains(&format!("class {}", class_name)),
        "exclude class name should exclude it"
    );
    assert!(java.trim().is_empty(), "output should be empty");
}

#[test]
fn test_filter_exclude_other_keeps_class() {
    let (dex, class_name) = dex_with_one_class();
    let options = DecompilerOptions {
        only_package: None,
        exclude: vec!["android.".to_string(), "other.".to_string()],
        ..Default::default()
    };
    let dc = Decompiler::with_options(&dex, options);
    let java = dc.decompile().unwrap();
    assert!(
        java.contains(&format!("class {}", class_name)),
        "exclude other packages should keep class {}",
        class_name
    );
}
