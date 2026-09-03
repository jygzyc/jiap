//! User-defined renames for decompiled output: package, class, method, field, and local variables.
//!
//! Keys for method/field/variable use the format `ClassName#memberName` (e.g. `com.example.Main#onCreate`).

use std::collections::HashMap;

/// Map of old identifier -> new identifier for renaming in decompiled Java source.
/// Keys for method/field/variable use format `FullClassName#memberName` (e.g. `Simple#foo`).
#[derive(Clone, Default)]
pub struct RenameMap {
    /// Package renames: old package -> new package (e.g. `"com.example"` -> `"com.myname"`).
    pub package: HashMap<String, String>,
    /// Class renames: old fully-qualified class name -> new (e.g. `"com.example.Main"` -> `"com.myname.MainActivity"`).
    pub class: HashMap<String, String>,
    /// Method renames: key `ClassName#methodName` (e.g. `com.example.Main#onCreate`) -> new method name.
    pub method: HashMap<String, String>,
    /// Field renames: key `ClassName#fieldName` -> new field name.
    pub field: HashMap<String, String>,
    /// Variable renames per method: key `ClassName#methodName` -> map of old var name -> new var name.
    /// Var names are those emitted by the decompiler (e.g. `p0`, `p1`, `result`, `i`, `tmp`).
    pub variable: HashMap<String, HashMap<String, String>>,
}

impl RenameMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace only whole identifiers (Java identifiers: letters, digits, `_`, `$`, `.` for FQ names).
    /// Replaces from longest to shortest to avoid partial replacements.
    pub fn apply_to_java(&self, java: &str, replacements: &[(String, String)]) -> String {
        if replacements.is_empty() {
            return java.to_string();
        }
        let mut sorted: Vec<_> = replacements.iter().collect();
        sorted.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        let mut out = java.to_string();
        for (old, new) in sorted {
            out = replace_identifier_occurrences(&out, old, new);
        }
        out
    }

    /// Build replacement list for one class: package, class, and per-class method/field renames.
    pub fn replacements_for_class(
        &self,
        class_name: &str,
        method_names: &[String],
        field_names: &[String],
    ) -> Vec<(String, String)> {
        let mut replacements = Vec::new();
        for (old_pkg, new_pkg) in &self.package {
            replacements.push((old_pkg.clone(), new_pkg.clone()));
        }
        for (old_class, new_class) in &self.class {
            replacements.push((old_class.clone(), new_class.clone()));
            // For the class we're emitting, also replace simple name in "class SimpleName extends ..."
            if old_class == class_name {
                let old_simple = old_class.rsplit('.').next().unwrap_or(old_class);
                let new_simple = new_class.rsplit('.').next().unwrap_or(new_class);
                if old_simple != new_simple {
                    replacements.push((old_simple.to_string(), new_simple.to_string()));
                }
            }
        }
        for method_name in method_names {
            let key = format!("{}#{}", class_name, method_name);
            if let Some(new_name) = self.method.get(&key) {
                replacements.push((method_name.clone(), new_name.clone()));
            }
        }
        for field_name in field_names {
            let key = format!("{}#{}", class_name, field_name);
            if let Some(new_name) = self.field.get(&key) {
                replacements.push((field_name.clone(), new_name.clone()));
            }
        }
        replacements
    }
}

/// Replace all whole-identifier occurrences of `old` with `new` in `s`.
fn replace_identifier_occurrences(s: &str, old: &str, new: &str) -> String {
    if old.is_empty() {
        return s.to_string();
    }
    let mut result = String::with_capacity(s.len());
    let mut search_start = 0;
    while let Some(start) = s[search_start..].find(old) {
        let start = search_start + start;
        let end = start + old.len();
        let char_before = if start == 0 {
            false
        } else {
            s[start - 1..]
                .chars()
                .next()
                .map(|c| is_identifier_char_utf8(c))
                .unwrap_or(false)
        };
        let char_after = if end >= s.len() {
            false
        } else {
            s[end..]
                .chars()
                .next()
                .map(|c| is_identifier_char_utf8(c))
                .unwrap_or(false)
        };
        if !char_before && !char_after {
            result.push_str(&s[search_start..start]);
            result.push_str(new);
            search_start = end;
        } else {
            result.push_str(&s[search_start..=start]);
            search_start = start + 1;
        }
    }
    result.push_str(&s[search_start..]);
    result
}

fn is_identifier_char_utf8(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$' || c == '.'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_identifier_whole_word() {
        let s = "void foo() { int foo = 1; bar(foo); }";
        let out = replace_identifier_occurrences(s, "foo", "myFoo");
        assert_eq!(out, "void myFoo() { int myFoo = 1; bar(myFoo); }");
    }

    #[test]
    fn replace_identifier_no_partial() {
        let s = "foobar foo foob";
        let out = replace_identifier_occurrences(s, "foo", "x");
        assert_eq!(out, "foobar x foob");
    }

    #[test]
    fn replace_identifier_fq_class() {
        let s = "import com.example.Main; com.example.Main x;";
        let out = replace_identifier_occurrences(s, "com.example.Main", "com.myname.MainActivity");
        assert_eq!(
            out,
            "import com.myname.MainActivity; com.myname.MainActivity x;"
        );
    }
}
