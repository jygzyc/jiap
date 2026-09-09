//! The kuna DECX-contract service: analyzer-backed handlers behind the
//! [`decx_server_sdk::SdkService`] interface.

use std::path::Path;

use decx_cli_core::error::{DecxError, DecxResult};
use decx_server_sdk::SdkService;
use serde_json::{json, Value};

use crate::analyzer::Analyzer;

pub struct KunaService {
    pub analyzer: Analyzer,
}

const SUPPORTED: &[&str] = &[
    "get_classes",
    "get_class_source",
    "get_method_source",
    "search_method",
    "search_global_key",
    "get_strings",
];

impl KunaService {
    pub fn load(target: &Path) -> DecxResult<Self> {
        Ok(Self {
            analyzer: Analyzer::load(target)?,
        })
    }
}

impl SdkService for KunaService {
    fn server_name(&self) -> &str {
        "decx-kuna-server"
    }

    fn capabilities(&self) -> Vec<&'static str> {
        SUPPORTED.to_vec()
    }

    fn health(&self) -> Value {
        json!({
            "status": "running",
            "server": self.server_name(),
            "engine": "kuna",
            "mode": "structural",
            "file": self.analyzer.file,
            "functions": self.analyzer.function_count(),
            "strings": self.analyzer.strings.len(),
        })
    }

    fn handle(&self, endpoint: &str, body: &Value) -> DecxResult<Value> {
        let key = body
            .get("cls")
            .or_else(|| body.get("mth"))
            .or_else(|| body.get("key"))
            .or_else(|| body.get("keyword"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        match endpoint {
            "get_classes" => Ok(json!([self.analyzer.file])),
            "get_class_source" => Ok(self.analyzer.render_binary()),
            "get_method_source" => {
                let func = self.analyzer.find_function(key).ok_or_else(|| {
                    DecxError::not_found(
                        "METHOD_NOT_FOUND",
                        format!("no function named '{key}' in {}", self.analyzer.file),
                    )
                })?;
                Ok(self.analyzer.render_function(func))
            }
            "search_method" => {
                if key.is_empty() {
                    return Err(DecxError::usage("search key is required"));
                }
                let matches: Vec<Value> = self
                    .analyzer
                    .search_functions(key, 100)
                    .into_iter()
                    .map(|f| {
                        json!({
                            "name": f.name,
                            "address": format!("0x{:x}", f.address),
                            "size": f.size,
                        })
                    })
                    .collect();
                Ok(json!({ "total": matches.len(), "matches": matches }))
            }
            "search_global_key" => {
                if key.is_empty() {
                    return Err(DecxError::usage("search key is required"));
                }
                let functions = self.analyzer.search_functions(key, 50);
                let string_hits: Vec<Value> = self
                    .analyzer
                    .strings
                    .iter()
                    .filter(|s| s.value.to_lowercase().contains(&key.to_lowercase()))
                    .take(50)
                    .map(|s| json!({ "offset": format!("0x{:x}", s.offset), "string": s.value }))
                    .collect();
                Ok(json!({
                    "total": functions.len() + string_hits.len(),
                    "functions": functions.into_iter().map(|f| f.name.clone()).collect::<Vec<_>>(),
                    "strings": string_hits,
                }))
            }
            "get_strings" => {
                let strings: Vec<Value> = self
                    .analyzer
                    .strings
                    .iter()
                    .map(|s| json!({ "offset": format!("0x{:x}", s.offset), "string": s.value }))
                    .collect();
                Ok(json!({ "total": strings.len(), "strings": strings }))
            }
            other => Err(DecxError::not_found(
                "UNKNOWN_ENDPOINT",
                format!("kuna does not implement '{other}'; supported: {}", SUPPORTED.join(", ")),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::tests::build_elf;

    fn service() -> KunaService {
        let elf = build_elf();
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = std::env::temp_dir().join(format!("decx-kuna-svc-{}-{n}.elf", std::process::id()));
        std::fs::write(&tmp, &elf).unwrap();
        let service = KunaService::load(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        service
    }

    #[test]
    fn endpoint_handlers() {
        let svc = service();
        assert!(svc.handle("get_classes", &json!({})).unwrap()[0]
            .as_str()
            .unwrap()
            .ends_with(".elf"));
        let src = svc.handle("get_class_source", &json!({ "cls": "x" })).unwrap();
        assert_eq!(src["functions"], 2);
        let mth = svc
            .handle("get_method_source", &json!({ "mth": "main" }))
            .unwrap();
        assert!(mth["source"].as_str().unwrap().contains("void main(void)"));
        let found = svc.handle("search_method", &json!({ "key": "helper" })).unwrap();
        assert_eq!(found["total"], 1);
        let strings = svc.handle("get_strings", &json!({})).unwrap();
        assert!(strings["total"].as_u64().unwrap() >= 2);
    }

    #[test]
    fn method_not_found_and_unknown_endpoint() {
        let svc = service();
        let err = svc
            .handle("get_method_source", &json!({ "mth": "nope" }))
            .expect_err("missing function");
        assert_eq!(err.code, "METHOD_NOT_FOUND");
        let err = svc.handle("get_app_manifest", &json!({})).expect_err("unsupported");
        assert_eq!(err.code, "UNKNOWN_ENDPOINT");
    }

    #[test]
    fn health_reports_mode() {
        let svc = service();
        let health = svc.health();
        assert_eq!(health["status"], "running");
        assert_eq!(health["mode"], "structural");
        assert_eq!(health["functions"], 2);
    }
}
