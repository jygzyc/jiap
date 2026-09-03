//! Shared harness: decompile fixtures DEX/APK, extract methods, compare to Java sources.

use dex_decompiler::{load_dexes_from_path, parse_dex, Decompiler, DecompilerOptions};
use std::collections::HashSet;
use std::path::PathBuf;

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/decompiler_fixtures")
}

pub fn fixtures_dex_path() -> PathBuf {
    fixtures_dir().join("classes.dex")
}

pub fn fixtures_apk_path() -> PathBuf {
    fixtures_dir().join("decompiler_fixtures.apk")
}

pub fn java_source_path(class_simple: &str) -> PathBuf {
    fixtures_dir().join(format!(
        "java/com/androguard/decompilefixtures/{class_simple}.java"
    ))
}

pub fn load_java_source(class_simple: &str) -> String {
    let path = java_source_path(class_simple);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read Java source {}: {e}", path.display()))
}

const FIXTURES_PACKAGE: &str = "com.androguard.decompilefixtures";

fn decompile_dex_file(dex: &dex_decompiler::DexFile) -> String {
    let options = DecompilerOptions {
        only_package: Some(FIXTURES_PACKAGE.to_string()),
        exclude: vec![],
        ..Default::default()
    };
    Decompiler::with_options(dex, options)
        .decompile()
        .expect("decompile fixtures")
}

pub fn decompile_fixtures_dex() -> String {
    let path = fixtures_dex_path();
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let dex = parse_dex(&data).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    decompile_dex_file(&dex)
}

pub fn decompile_fixtures_apk() -> String {
    let path = fixtures_apk_path();
    let dexes =
        load_dexes_from_path(&path).unwrap_or_else(|e| panic!("load {}: {e}", path.display()));
    assert!(!dexes.is_empty(), "APK produced no DEX");
    decompile_dex_file(&dexes[0])
}

/// Extract one method declaration + balanced `{ … }` body from decompiled or source Java.
pub fn method_region(java: &str, method_name: &str) -> String {
    let needle = format!("{method_name}(");
    let mut idx = 0;
    let sig_start = loop {
        let Some(rel) = java[idx..].find(&needle) else {
            panic!("method {method_name} not found in:\n{java}");
        };
        let start = idx + rel;
        let line_start = java[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line = &java[line_start..start];
        let before_name = line.trim_end();
        let is_call = before_name.ends_with('.');
        let is_decl = line.contains("public")
            || line.contains("private")
            || line.contains("protected")
            || line.contains("static");
        if is_decl && !is_call {
            break start;
        }
        idx = start + needle.len();
    };
    let open_rel = java[sig_start..]
        .find('{')
        .unwrap_or_else(|| panic!("method {method_name} has no opening brace"));
    let open = sig_start + open_rel;
    let mut depth = 0i32;
    for (i, ch) in java[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return java[sig_start..=open + i].to_string();
                }
            }
            _ => {}
        }
    }
    panic!("method {method_name} has unbalanced braces");
}

/// Collapse whitespace for fuzzy text comparison.
pub fn normalize_java_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

const JAVA_KEYWORDS: &[&str] = &[
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "void",
    "volatile",
    "while",
    "true",
    "false",
];

const JAVA_TYPES: &[&str] = &[
    "String",
    "Object",
    "Integer",
    "List",
    "Arrays",
    "Math",
    "Runnable",
    "Closeable",
    "IOException",
    "IllegalStateException",
    "ArithmeticException",
    "RuntimeException",
    "Exception",
    "NullPointerException",
    "ClassCastException",
    "StubResource",
    "Color",
    "Token",
    "Context",
    "ContextLike",
    "Outer",
    "Inner",
    "StaticInner",
    "IntUnaryOperator",
    "MessageDigest",
    "Mac",
    "Cipher",
    "SecretKeySpec",
    "IvParameterSpec",
    "SecureRandom",
    "PBEKeySpec",
    "SecretKeyFactory",
    "NoSuchAlgorithmException",
    "Queue",
    "ArrayDeque",
    "System",
    "Override",
];

const JAVA_MISC: &[&str] = &["println", "out"];

fn is_java_keyword(s: &str) -> bool {
    JAVA_KEYWORDS.contains(&s) || JAVA_TYPES.contains(&s) || JAVA_MISC.contains(&s)
}

/// Identifiers from a Java method (params + body) that should appear in faithful decompilation.
pub fn source_identifiers(method_src: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut word = String::new();
    for ch in method_src.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_alphabetic() || ch == '_' || (!word.is_empty() && ch.is_ascii_digit()) {
            word.push(ch);
        } else if !word.is_empty() {
            if word.len() > 1 || word.chars().all(|c| c.is_ascii_alphabetic()) {
                if !is_java_keyword(&word) {
                    out.insert(word.clone());
                }
            }
            word.clear();
        }
    }
    out
}

static SSA_TEMP_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

fn ssa_temp_re() -> &'static regex::Regex {
    SSA_TEMP_RE.get_or_init(|| {
        regex::Regex::new(r"\b(i\d+|local\d+|v\d+|z\d+|j\d+|s\d+)\b").expect("ssa temp regex")
    })
}

/// Fail if decompiled output uses SSA temp names not present in the Java source.
pub fn assert_no_leaked_ssa_temps(decompiled: &str, allowed: &HashSet<String>, ctx: &str) {
    for cap in ssa_temp_re().find_iter(decompiled) {
        let name = cap.as_str();
        if !allowed.contains(name) {
            panic!("leaked SSA temp `{name}` not in Java source — {ctx}\n{decompiled}");
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompareTier {
    /// Require source identifiers + no SSA temps + must_contain.
    SourceLike,
    /// Structural checks only (must_contain / must_not_contain).
    Structural,
    /// Method decompiles; minimal checks.
    Smoke,
}

#[derive(Clone, Copy, Debug)]
pub struct FixtureMethodSpec {
    pub class: &'static str,
    pub method: &'static str,
    pub tier: CompareTier,
    pub source_ids: bool,
    pub must_contain: &'static [&'static str],
    pub must_not_contain: &'static [&'static str],
    /// Identifiers from source to ignore (desugared away, e.g. enhanced-for `row`).
    pub skip_source_ids: &'static [&'static str],
}

impl FixtureMethodSpec {
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.class, self.method)
    }

    /// Returns all assertion failures for this method (empty = pass).
    pub fn check_errors(&self, decompiled_java: &str, source_java: &str) -> Vec<String> {
        let decompiled = method_region(decompiled_java, self.method);
        let source = method_region(source_java, self.method);
        let ctx = self.full_name();
        let mut errors = Vec::new();

        for needle in self.must_contain {
            if !decompiled.contains(needle) {
                errors.push(format!(
                    "{ctx}: expected `{needle}` in decompiled output:\n{decompiled}"
                ));
            }
        }
        for needle in self.must_not_contain {
            if decompiled.contains(needle) {
                errors.push(format!("{ctx}: must not contain `{needle}`:\n{decompiled}"));
            }
        }

        if self.tier == CompareTier::Smoke {
            return errors;
        }

        let mut allowed: HashSet<String> = source_identifiers(&source);
        for s in self.skip_source_ids {
            allowed.remove(*s);
        }

        if self.source_ids {
            for id in &allowed {
                if id.len() <= 1 {
                    continue;
                }
                if !decompiled.contains(id.as_str()) {
                    errors.push(format!(
                        "{ctx}: Java identifier `{id}` missing from decompiled output:\n{decompiled}\n\nsource:\n{source}"
                    ));
                }
            }
        }

        if self.tier == CompareTier::SourceLike {
            for cap in ssa_temp_re().find_iter(&decompiled) {
                let name = cap.as_str();
                if !allowed.contains(name) {
                    errors.push(format!(
                        "leaked SSA temp `{name}` not in Java source — {ctx}\n{decompiled}"
                    ));
                }
            }
        }

        errors
    }

    pub fn check(&self, decompiled_java: &str, source_java: &str) {
        let errors = self.check_errors(decompiled_java, source_java);
        if let Some(first) = errors.first() {
            panic!("{first}");
        }
    }
}

/// Run all manifest entries against decompiled output.
pub fn check_all_fixtures(decompiled: &str, manifest: &[FixtureMethodSpec]) {
    let mut by_class: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    for spec in manifest {
        by_class
            .entry(spec.class)
            .or_insert_with(|| load_java_source(spec.class));
    }
    let mut all_errors = Vec::new();
    for spec in manifest {
        let source = by_class.get(spec.class).expect("source loaded");
        all_errors.extend(spec.check_errors(decompiled, source));
    }
    if !all_errors.is_empty() {
        panic!(
            "{} fixture method(s) failed source-fidelity checks:\n\n{}",
            all_errors.len(),
            all_errors.join("\n\n---\n\n")
        );
    }
}

/// List `public static` methods declared in fixture Java sources (excluding MainActivity).
pub fn catalog_public_static_methods() -> Vec<(String, String)> {
    let java_root = fixtures_dir().join("java/com/androguard/decompilefixtures");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&java_root).expect("read fixture java dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("java") {
            continue;
        }
        let class = path.file_stem().unwrap().to_string_lossy().to_string();
        if class == "MainActivity" {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read java");
        for line in text.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with("public static") {
                continue;
            }
            if trimmed.contains(" class ") {
                continue;
            }
            let Some(name_start) = trimmed.find(" static ") else {
                continue;
            };
            let rest = &trimmed[name_start + 8..];
            let name_end = rest
                .find('(')
                .unwrap_or_else(|| panic!("no ( in {trimmed}"));
            let method = rest[..name_end]
                .split_whitespace()
                .last()
                .unwrap_or("")
                .to_string();
            if !method.is_empty() {
                out.push((class.clone(), method));
            }
        }
    }
    out.sort();
    out
}

pub fn assert_manifest_covers_catalog(manifest: &[FixtureMethodSpec]) {
    let catalog = catalog_public_static_methods();
    let covered: HashSet<String> = manifest
        .iter()
        .map(|s| format!("{}.{}", s.class, s.method))
        .collect();
    let mut missing = Vec::new();
    for (class, method) in catalog {
        let key = format!("{class}.{method}");
        if !covered.contains(&key) {
            missing.push(key);
        }
    }
    assert!(
        missing.is_empty(),
        "fixture manifest missing public static methods (add to fixture_manifest.rs):\n{}",
        missing.join("\n")
    );
}

/// APK vs standalone DEX: method bodies should match for listed methods.
pub fn assert_apk_matches_dex_methods(methods: &[(&str, &str)]) {
    let dex_out = decompile_fixtures_dex();
    let apk_out = decompile_fixtures_apk();
    for (class, method) in methods {
        let dex_body = method_region(&dex_out, method);
        let apk_body = method_region(&apk_out, method);
        assert_eq!(
            normalize_java_text(&dex_body),
            normalize_java_text(&apk_body),
            "{class}.{method}: APK decompilation differs from classes.dex"
        );
    }
}
