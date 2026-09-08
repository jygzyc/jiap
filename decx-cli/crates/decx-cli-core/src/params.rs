//! Request-body builders mirroring the server contract (`DecxApi`).
//!
//! These mirror the option objects in the TypeScript client (`filter`,
//! `search`, `grep`) so endpoints receive exactly the shapes the Kotlin and
//! native servers expect.

use serde_json::{json, Value};

/// `filter` object for class-style endpoints (limit + regex package filters).
#[derive(Debug, Clone, Default)]
pub struct ClassFilter {
    pub limit: Option<u64>,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub regex: Option<bool>,
}

impl ClassFilter {
    pub fn to_value(&self) -> Value {
        let mut filter = json!({
            "includes": self.includes,
            "excludes": self.excludes,
        });
        if let Some(limit) = self.limit {
            filter["limit"] = json!(limit);
        }
        if let Some(regex) = self.regex {
            filter["regex"] = json!(regex);
        }
        json!({ "filter": filter })
    }
}

/// `filter` object for source endpoints (line limit + optional language).
#[derive(Debug, Clone, Default)]
pub struct SourceFilter {
    pub limit: Option<u64>,
    pub language: Option<String>,
}

impl SourceFilter {
    pub fn to_value(&self) -> Value {
        let mut filter = json!({});
        if let Some(limit) = self.limit {
            filter["limit"] = json!(limit);
        }
        let mut body = json!({ "filter": filter });
        if let Some(lang) = &self.language {
            body["language"] = json!(lang);
        }
        body
    }
}

/// `search` object for `search_global_key`.
#[derive(Debug, Clone, Default)]
pub struct GlobalSearch {
    pub limit: Option<u64>,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub case_sensitive: bool,
    pub regex: bool,
}

impl GlobalSearch {
    pub fn to_value(&self) -> Value {
        let mut search = json!({
            "includes": self.includes,
            "excludes": self.excludes,
            "caseSensitive": self.case_sensitive,
            "regex": self.regex,
        });
        if let Some(limit) = self.limit {
            search["limit"] = json!(limit);
        }
        json!({ "search": search })
    }
}

/// `grep` object for `search_class_key`.
#[derive(Debug, Clone, Default)]
pub struct ClassGrep {
    pub limit: u64,
    pub case_sensitive: bool,
    pub regex: bool,
}

impl ClassGrep {
    pub fn to_value(&self) -> Value {
        json!({
            "grep": {
                "limit": self.limit,
                "caseSensitive": self.case_sensitive,
                "regex": self.regex,
            }
        })
    }
}

/// `includes`/`excludes` object for `get_exported_components`.
#[derive(Debug, Clone, Default)]
pub struct ComponentFilter {
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub regex: Option<bool>,
}

impl ComponentFilter {
    pub fn to_value(&self) -> Value {
        let mut body = json!({
            "includes": self.includes,
            "excludes": self.excludes,
        });
        if let Some(regex) = self.regex {
            body["regex"] = json!(regex);
        }
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn class_filter_shape() {
        let f = ClassFilter {
            limit: Some(10),
            includes: vec!["com.a".into()],
            excludes: vec![],
            regex: Some(false),
        };
        assert_eq!(
            f.to_value(),
            json!({ "filter": { "limit": 10, "includes": ["com.a"], "excludes": [], "regex": false } })
        );
    }

    #[test]
    fn source_filter_with_language() {
        let f = SourceFilter {
            limit: Some(5),
            language: Some("kotlin".into()),
        };
        assert_eq!(
            f.to_value(),
            json!({ "filter": { "limit": 5 }, "language": "kotlin" })
        );
    }

    #[test]
    fn global_search_shape() {
        let f = GlobalSearch {
            limit: Some(20),
            ..Default::default()
        };
        assert_eq!(
            f.to_value(),
            json!({ "search": { "includes": [], "excludes": [], "caseSensitive": false, "regex": false, "limit": 20 } })
        );
    }
}
