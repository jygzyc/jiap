//! Rule model for the taint solver: sources / sinks / propagators /
//! sanitizers / excludes, loaded from an embedded JSON document
//! (`resources/taint-rules.json`). Mirrors the Mariana-Trench model shape.

use decx_json::{parse as json_parse, Json};

/// how a propagator moves taint between receiver / arguments / return value
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PropMode {
    /// tainted argument taints the receiver (StringBuilder.append, Intent.putExtra)
    ArgToRecv,
    /// tainted receiver taints the return (StringBuilder.toString)
    RecvToRet,
    /// tainted receiver or any argument taints the return (String.concat)
    RecvOrArgToRet,
    /// tainted argument taints the return (String.valueOf, Base64.encode)
    ArgToRet,
}

/// a taint entry rule; `sig` is a substring matched against the full
/// dex descriptor form (`Lcls;->name(` or a field key `Lcls;->name:T`)
#[derive(Clone, Debug)]
pub struct Entry {
    pub sig: String,
    pub label: String,
    /// method (default) or static-field source
    pub kind: &'static str,
    /// AppShark-style param anchoring (method sources only): the declared
    /// parameter at this 0-based position is a taint seed — used for
    /// framework callbacks (`onLocationChanged(Location)`, `onReceive(...)`).\n    /// `None` (default) means the return value is the source.
    pub param: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Propagator {
    pub sig: String,
    pub mode: PropMode,
}

#[derive(Clone, Debug, Default)]
pub struct Rules {
    pub sources: Vec<Entry>,
    pub sinks: Vec<Entry>,
    pub propagators: Vec<Propagator>,
    /// sanitizers: call kills the taint of the value it returns
    pub sanitizers: Vec<String>,
    /// owner-descriptor prefixes whose method bodies are skipped entirely
    pub excludes: Vec<String>,
}

fn entries(v: &Json) -> Vec<Entry> {
    let mut out = Vec::new();
    for e in v.as_arr().unwrap_or(&[]) {
        let (Some(sig), Some(label)) = (
            e.get("sig").and_then(|s| s.as_str()),
            e.get("label").and_then(|s| s.as_str()),
        ) else {
            continue;
        };
        let kind = match e.get("kind").and_then(|k| k.as_str()) {
            Some("field") => "field",
            _ => "method",
        };
        let param = e
            .get("param")
            .and_then(|p| p.as_i64())
            .filter(|p| *p >= 0)
            .map(|p| p as usize);
        out.push(Entry {
            sig: sig.to_string(),
            label: label.to_string(),
            kind,
            param: if kind == "method" { param } else { None },
        });
    }
    out
}

impl Rules {
    /// Parse from a JSON document. Unknown fields are ignored; a document
    /// without any sources AND sinks is rejected.
    pub fn from_json(doc: &Json) -> Result<Rules, String> {
        let mut r = Rules {
            sources: entries(doc.get("sources").unwrap_or(&Json::Null)),
            sinks: entries(doc.get("sinks").unwrap_or(&Json::Null)),
            ..Default::default()
        };
        for p in doc.get("propagators").and_then(|v| v.as_arr()).unwrap_or(&[]) {
            let (Some(sig), Some(mode)) = (
                p.get("sig").and_then(|s| s.as_str()),
                p.get("mode").and_then(|m| m.as_str()),
            ) else {
                continue;
            };
            let mode = match mode {
                "arg_to_recv" => PropMode::ArgToRecv,
                "recv_to_ret" => PropMode::RecvToRet,
                "recv_or_arg_to_ret" => PropMode::RecvOrArgToRet,
                "arg_to_ret" => PropMode::ArgToRet,
                _ => continue,
            };
            r.propagators.push(Propagator {
                sig: sig.to_string(),
                mode,
            });
        }
        if let Some(v) = doc.get("sanitizers").and_then(|v| v.as_arr()) {
            for s in v {
                if let Some(sig) = s.get("sig").and_then(|s| s.as_str()) {
                    r.sanitizers.push(sig.to_string());
                }
            }
        }
        if let Some(v) = doc.get("excludes").and_then(|v| v.as_arr()) {
            for s in v {
                if let Some(p) = s.as_str() {
                    r.excludes.push(p.to_string());
                }
            }
        }
        if r.sources.is_empty() && r.sinks.is_empty() {
            return Err("taint rules: no sources and no sinks".into());
        }
        Ok(r)
    }

    /// Load the embedded default rule set.
    pub fn default_rules() -> Rules {
        static DEFAULT: &str = include_str!("../resources/taint-rules.json");
        Rules::from_json(&json_parse(DEFAULT).expect("embedded taint-rules.json is valid"))
            .expect("embedded taint-rules.json parses")
    }

    /// Load a rule file (AppShark-style customization): the file fully
    /// replaces the embedded defaults. Any failure is reported with the
    /// offending path so the server can fail fast at startup.
    pub fn load_file(path: &str) -> Result<Rules, String> {
        let data =
            std::fs::read_to_string(path).map_err(|e| format!("cannot read '{path}': {e}"))?;
        let doc = json_parse(&data).map_err(|e| format!("invalid JSON in '{path}': {e}"))?;
        Rules::from_json(&doc).map_err(|e| format!("invalid rules in '{path}': {e}"))
    }

    /// owner descriptor ("Lcom/a/B;..." prefix before `->`)
    pub fn owner_excluded(&self, full_sig: &str) -> bool {
        let owner = full_sig.split("->").next().unwrap_or("");
        self.excludes.iter().any(|p| owner.starts_with(p.as_str()))
    }

    /// source rule matching a callee signature; returns (rule index, label)
    pub fn match_source(&self, callee: &str) -> Option<usize> {
        self.sources
            .iter()
            .position(|e| e.kind == "method" && callee.contains(&e.sig))
    }

    pub fn match_field_source(&self, field: &str) -> Option<usize> {
        self.sources
            .iter()
            .position(|e| e.kind == "field" && field.contains(&e.sig))
    }

    pub fn match_sink(&self, callee: &str) -> Option<usize> {
        self.sinks.iter().position(|e| e.kind == "method" && callee.contains(&e.sig))
    }

    pub fn match_field_sink(&self, field: &str) -> Option<usize> {
        self.sinks
            .iter()
            .position(|e| e.kind == "field" && field.contains(&e.sig))
    }

    pub fn match_propagator(&self, callee: &str) -> Option<PropMode> {
        self.propagators
            .iter()
            .find(|p| callee.contains(&p.sig))
            .map(|p| p.mode)
    }

    /// param-anchored source rule firing on this method signature.
    /// Callback overrides do not contain the interface-owner prefix, so
    /// matching is by owner-stripped key prefix (`->name(`) in addition to
    /// plain containment: any override of the declared callback matches.
    pub fn param_source_for(&self, sig: &str) -> Option<(usize, usize)> {
        let mkey = key_of(sig);
        self.sources
            .iter()
            .position(|e| {
                e.kind == "method"
                    && e.param.is_some()
                    && (sig.contains(&e.sig) || mkey.starts_with(key_of(&e.sig)))
            })
            .map(|i| (i, self.sources[i].param.unwrap_or(0)))
    }

    pub fn is_sanitizer(&self, callee: &str) -> bool {
        self.sanitizers.iter().any(|s| callee.contains(s.as_str()))
    }
}

/// owner-stripped key of a full method signature (shared with dispatch.rs)
pub(crate) fn key_of(sig: &str) -> &str {
    match sig.find("->") {
        Some(i) => &sig[i..],
        None => sig,
    }
}

/// severity from source/sink label pairs (kept from the call-string version)
pub fn severity_for(src: &str, sink: &str) -> &'static str {
    let privacy = matches!(
        src,
        "imei"
            | "imsi"
            | "meid"
            | "phone_number"
            | "sim_serial"
            | "gps"
            | "location"
            | "account"
            | "account_password"
            | "mac"
            | "bssid"
            | "bt_mac"
            | "ssid"
            | "serial"
            | "fingerprint"
            | "android_id_or_setting"
    );
    let exfil = matches!(
        sink,
        "network_url"
            | "http_post"
            | "http_request"
            | "http_header"
            | "http_connect"
            | "raw_socket"
            | "send_sms"
            | "webview_load"
            | "webview_post"
            | "webview_js"
            | "json_put"
            | "url_encode"
            | "mailto"
            | "intent_data"
    );
    let code_exec = matches!(sink, "command_exec" | "reflect_invoke" | "pending_intent");
    let inject = matches!(sink, "sql_exec" | "sql_query" | "content_insert" | "content_update");
    if (privacy && exfil) || code_exec {
        "high"
    } else if privacy || exfil || inject {
        "medium"
    } else {
        "low"
    }
}
