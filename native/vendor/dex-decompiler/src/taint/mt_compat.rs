//! Convert Mariana Trench `models.json` / `rules.json` into [`TaintConfig`].

use serde::Deserialize;
use serde_json::Value;

use super::config::TaintConfig;
use super::models::{Port, PropagationModel, Rule, SanitizerModel, SinkModel, SourceModel};
use crate::error::{DexDecompilerError, Result};

/// Parse MT port strings: `Return`, `This`, `Argument(0)`, `Argument(1)`, …
pub fn parse_mt_port(s: &str) -> Port {
    let s = s.trim();
    if s.eq_ignore_ascii_case("Return") {
        return Port::Return;
    }
    if s.eq_ignore_ascii_case("This") || s.eq_ignore_ascii_case("Argument(0)") {
        // MT uses Argument(0) for `this` on instance methods.
        return Port::This;
    }
    if let Some(rest) = s
        .strip_prefix("Argument(")
        .and_then(|r| r.strip_suffix(')'))
    {
        if let Ok(idx) = rest.parse::<u32>() {
            return Port::Argument { index: idx };
        }
    }
    Port::Return
}

/// Turn an MT method descriptor into substring patterns for our matcher.
///
/// `Lcom/foo/Bar;.baz:(I)V` → `["com.foo.Bar.baz", "Bar.baz", "baz"]` (deduped preference order).
pub fn method_patterns(mt_method: &str) -> Vec<String> {
    let mut out = Vec::new();
    // Strip descriptor: Lpkg/Cls;.name:(...)Ret
    let bare = mt_method.split(':').next().unwrap_or(mt_method);
    if let Some((cls, name)) = bare.split_once(';') {
        let name = name.trim_start_matches('.');
        let java_cls = cls.trim_start_matches('L').replace('/', ".");
        if !java_cls.is_empty() && !name.is_empty() {
            out.push(format!("{java_cls}.{name}"));
            if let Some(simple) = java_cls.rsplit('.').next() {
                out.push(format!("{simple}.{name}"));
            }
            out.push(name.to_string());
        }
    } else if let Some((cls, name)) = bare.rsplit_once('.') {
        out.push(format!("{cls}.{name}"));
        out.push(name.to_string());
    } else {
        out.push(bare.to_string());
    }
    out
}

#[derive(Debug, Deserialize)]
struct MtGeneration {
    kind: String,
    #[serde(default = "default_return_port")]
    port: String,
}

fn default_return_port() -> String {
    "Return".into()
}

#[derive(Debug, Deserialize)]
struct MtSink {
    kind: String,
    #[serde(default = "default_arg0_port")]
    port: String,
}

fn default_arg0_port() -> String {
    "Argument(0)".into()
}

#[derive(Debug, Deserialize)]
struct MtPropagation {
    input: String,
    output: String,
    #[serde(default)]
    features: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct MtSanitizer {
    #[serde(default)]
    sanitize: Option<String>,
    #[serde(default)]
    port: Option<String>,
    #[serde(default)]
    kinds: Vec<Value>,
}

#[derive(Debug, Deserialize)]
struct MtModel {
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    generations: Vec<MtGeneration>,
    #[serde(default)]
    sinks: Vec<MtSink>,
    #[serde(default)]
    propagation: Vec<MtPropagation>,
    #[serde(default)]
    sanitizers: Vec<MtSanitizer>,
}

#[derive(Debug, Deserialize)]
struct MtRule {
    name: String,
    code: u32,
    #[serde(default)]
    description: String,
    #[serde(default)]
    sources: Vec<String>,
    #[serde(default)]
    sinks: Vec<String>,
}

fn kind_from_value(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(map) => map.get("kind").and_then(|k| match k {
            Value::String(s) => Some(s.clone()),
            Value::Object(inner) => inner
                .get("base")
                .and_then(|b| b.as_str())
                .map(|s| s.to_string()),
            _ => None,
        }),
        _ => None,
    }
}

/// Convert a Mariana Trench models.json document into our config fragment.
pub fn convert_models_json(text: &str) -> Result<TaintConfig> {
    let models: Vec<MtModel> = serde_json::from_str(text)
        .map_err(|e| DexDecompilerError::Decompilation(format!("mt models: {e}")))?;
    let mut cfg = TaintConfig::default();
    for m in models {
        let Some(method) = m.method else { continue };
        let patterns = method_patterns(&method);
        if patterns.is_empty() {
            continue;
        }
        for g in m.generations {
            cfg.sources.push(SourceModel {
                patterns: patterns.clone(),
                port: parse_mt_port(&g.port),
                kind: g.kind,
                features: Vec::new(),
            });
        }
        for s in m.sinks {
            cfg.sinks.push(SinkModel {
                patterns: patterns.clone(),
                port: parse_mt_port(&s.port),
                kind: s.kind,
                features: Vec::new(),
            });
        }
        for p in m.propagation {
            cfg.propagations.push(PropagationModel {
                patterns: patterns.clone(),
                from: parse_mt_port(&p.input),
                to: parse_mt_port(&p.output),
                features: p.features,
            });
        }
        for san in m.sanitizers {
            let kinds: Vec<String> = if san.kinds.is_empty() {
                vec!["*".into()]
            } else {
                san.kinds.iter().filter_map(kind_from_value).collect()
            };
            // Represent MT sanitizers as our SanitizerModel on the method patterns.
            let _ = san.sanitize;
            let _ = san.port;
            cfg.sanitizers.push(SanitizerModel {
                patterns: patterns.clone(),
                kinds,
            });
        }
    }
    Ok(cfg)
}

/// Convert Mariana Trench rules.json into our [`Rule`] list wrapped in a config.
pub fn convert_rules_json(text: &str) -> Result<TaintConfig> {
    let rules: Vec<MtRule> = serde_json::from_str(text)
        .map_err(|e| DexDecompilerError::Decompilation(format!("mt rules: {e}")))?;
    Ok(TaintConfig {
        rules: rules
            .into_iter()
            .map(|r| Rule {
                name: r.name,
                code: r.code,
                description: r.description,
                sources: r.sources,
                sinks: r.sinks,
            })
            .collect(),
        ..Default::default()
    })
}

/// Load models.json + rules.json (and optional field_models.json generations as sources) from a case dir.
pub fn load_mt_case_config(
    models_path: &std::path::Path,
    rules_path: &std::path::Path,
) -> Result<TaintConfig> {
    let mut cfg = convert_models_json(
        &std::fs::read_to_string(models_path)
            .map_err(|e| DexDecompilerError::Decompilation(format!("read models: {e}")))?,
    )?;
    if rules_path.exists() {
        cfg.merge(convert_rules_json(
            &std::fs::read_to_string(rules_path)
                .map_err(|e| DexDecompilerError::Decompilation(format!("read rules: {e}")))?,
        )?);
    }
    let field_models = models_path.parent().map(|p| p.join("field_models.json"));
    if let Some(fp) = field_models {
        if fp.exists() {
            // Field models use a similar schema; attempt best-effort conversion.
            if let Ok(extra) =
                convert_models_json(&std::fs::read_to_string(&fp).unwrap_or_default())
            {
                cfg.merge(extra);
            }
        }
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ports() {
        assert_eq!(parse_mt_port("Return"), Port::Return);
        assert_eq!(parse_mt_port("This"), Port::This);
        assert_eq!(parse_mt_port("Argument(0)"), Port::This);
        assert_eq!(parse_mt_port("Argument(1)"), Port::Argument { index: 1 });
    }

    #[test]
    fn patterns_from_descriptor() {
        let p = method_patterns(
            "Lcom/facebook/marianatrench/integrationtests/Origin;.source:()Ljava/lang/Object;",
        );
        assert!(p.iter().any(|s| s.contains("Origin.source")));
        assert!(p.iter().any(|s| s == "source"));
    }
}
