//! Endpoint dispatch: POST /api/decx/{endpoint}, GET /health.
//! Mirrors the 26-endpoint DECX contract (kind strings match the JVM server) plus the
//! native-only `taint_scan` endpoint.

use crate::envelope::{error_envelope, success};
use decx_core::java;
use decx_core::project::Project;
use decx_core::regex::Regex;
use decx_core::sources;
use decx_json::Json;
use std::sync::Arc;

pub struct AppState {
    pub project: Arc<Project>,
    pub started: std::time::Instant,
    pub calls: std::sync::atomic::AtomicU64,
    /// rule set for the native-only taint_scan endpoint: `--taint-rules`
    /// / `DECX_TAINT_RULES` file when provided, embedded defaults otherwise
    pub taint_rules: decx_taint::Rules,
}

/// filter object {limit, includes[], excludes[], regex, caseSensitive}
pub struct Filter {
    pub limit: Option<usize>,
    includes: Vec<String>,
    excludes: Vec<String>,
    regex: bool,
    case_sensitive: bool,
}

impl Filter {
    pub fn from_json(j: Option<&Json>) -> Filter {
        let mut f = Filter {
            limit: None,
            includes: Vec::new(),
            excludes: Vec::new(),
            regex: false,
            // Kotlin contract default: case-insensitive unless caseSensitive=true
            case_sensitive: false,
        };
        let Some(j) = j else { return f };
        if let Some(l) = j.get("limit").and_then(|v| v.as_i64()) {
            f.limit = Some(l.max(1) as usize);
        }
        if let Some(Json::Arr(v)) = j.get("includes") {
            f.includes = v
                .iter()
                .filter_map(|s| s.as_str().map(|s| s.to_string()))
                .collect();
        }
        if let Some(Json::Arr(v)) = j.get("excludes") {
            f.excludes = v
                .iter()
                .filter_map(|s| s.as_str().map(|s| s.to_string()))
                .collect();
        }
        if let Some(r) = j.get("regex").and_then(|v| v.as_bool()) {
            f.regex = r;
        }
        if let Some(c) = j.get("caseSensitive").and_then(|v| v.as_bool()) {
            f.case_sensitive = c;
        }
        f
    }

    fn matches_one(pat: &str, s: &str, regex: bool, cs: bool) -> bool {
        let hay = if cs { s.to_string() } else { s.to_lowercase() };
        let needle = if cs { pat.to_string() } else { pat.to_lowercase() };
        let pat_owned = if cs { pat.to_string() } else { pat.to_lowercase() };
        if regex {
            match Regex::new(&pat_owned) {
                Ok(r) => r.is_match(&hay),
                Err(_) => hay.contains(&needle),
            }
        } else {
            hay.contains(&needle)
        }
    }

    pub fn accepts(&self, s: &str) -> bool {
        for e in &self.excludes {
            if Self::matches_one(e, s, self.regex, self.case_sensitive) {
                return false;
            }
        }
        if self.includes.is_empty() {
            return true;
        }
        self.includes
            .iter()
            .any(|i| Self::matches_one(i, s, self.regex, self.case_sensitive))
    }
}

fn dotted(desc: &str) -> String {
    desc.trim_start_matches('L')
        .trim_end_matches(';')
        .replace('/', ".")
}

pub fn health(st: &AppState, port: u16) -> Json {
    let calls = st.calls.load(std::sync::atomic::Ordering::Relaxed);
    Json::obj(vec![
        ("status", Json::str("ok")),
        ("version", Json::str("0.1.0")),
        ("url", Json::str(&format!("http://127.0.0.1:{port}"))),
        ("port", Json::Int(port as i64)),
        (
            "timestamp",
            Json::Int(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)),
        ),
        ("active_operations", Json::Int(0)),
        ("uptime_secs", Json::Int(st.started.elapsed().as_secs() as i64)),
        (
            "cache",
            Json::obj(vec![
                ("classes", Json::Int(st.project.total_classes() as i64)),
                ("methods", Json::Int(st.project.total_methods() as i64)),
                ("strings", Json::Int(st.project.dexes.iter().map(|d| d.strings.len()).sum::<usize>() as i64)),
                ("requests", Json::Int(calls as i64)),
            ]),
        ),
        (
            "native",
            Json::obj(vec![
                ("engine", Json::str("decx-native")),
                ("project", Json::str(&st.project.name)),
                ("dexes", Json::Int(st.project.dexes.len() as i64)),
            ]),
        ),
    ])
}

fn q_str(body: &Json, key: &str) -> String {
    body.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

/// Contract uses short param names (cls/mth/fld/iface/res); accept the long
/// human-friendly spellings as fallback.
fn q_any(body: &Json, short: &str, long: &str) -> String {
    let s = q_str(body, short);
    if s.is_empty() { q_str(body, long) } else { s }
}

fn item_symbol(id: &str, title: &str, meta: Vec<(&str, Json)>) -> Json {
    let mut kv = vec![
        ("id".to_string(), Json::str(id)),
        ("kind".to_string(), Json::str("symbol")),
        ("title".to_string(), Json::str(title)),
    ];
    for (k, v) in meta {
        kv.push((k.to_string(), v));
    }
    Json::Obj(kv)
}

fn item_code(id: &str, title: &str, content: String) -> Json {
    Json::obj(vec![
        ("id", Json::str(id)),
        ("kind", Json::str("code")),
        ("title", Json::str(title)),
        ("content", Json::str(&content)),
    ])
}

fn item_xref(id: &str, title: &str, meta: Vec<(&str, Json)>) -> Json {
    let mut kv = vec![
        ("id".to_string(), Json::str(id)),
        ("kind".to_string(), Json::str("xref")),
        ("title".to_string(), Json::str(title)),
    ];
    for (k, v) in meta {
        kv.push((k.to_string(), v));
    }
    Json::Obj(kv)
}

/// Dispatch one endpoint. `endpoint` like "get_classes".
pub fn dispatch(st: &AppState, endpoint: &str, body: &Json) -> Json {
    st.calls
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let p = &st.project;
    let filter_j = body.get("filter");
    let f = Filter::from_json(filter_j);
    // contract: 1-based page number (decx-cli sends `page`)
    let page = body
        .get("page")
        .and_then(|v| v.as_i64())
        .unwrap_or(1)
        .max(1) as i64;
    let empty_q = Json::obj(vec![("page", Json::Int(page))]);
    let plain_q = Json::obj(vec![]);

    let miss = |kind: &str, code: &str, msg: &str| error_envelope(kind, &plain_q, code, msg);

    match endpoint {
        "get_classes" => {
            let mut items = Vec::new();
            for (di, d) in p.dexes.iter().enumerate() {
                for def in &d.class_defs {
                    let desc = d.type_descriptor(def.class_idx);
                    let name = dotted(desc);
                    if !f.accepts(&name) {
                        continue;
                    }
                    let cd = d.class_data(def);
                    items.push(item_symbol(
                        &name,
                        &name,
                        vec![
                            ("dex", Json::str(&p.dex_names[di])),
                            ("methods", Json::Int((cd.direct_methods.len() + cd.virtual_methods.len()) as i64)),
                            (
                                "fields",
                                Json::Int((cd.static_fields.len() + cd.instance_fields.len()) as i64),
                            ),
                            ("flags", Json::Int(def.access_flags as i64)),
                        ],
                    ));
                    if let Some(l) = f.limit {
                        if items.len() >= l {
                            break;
                        }
                    }
                }
                if let Some(l) = f.limit {
                    if items.len() >= l {
                        break;
                    }
                }
            }
            success("classes", empty_q, items, Json::obj(vec![]))
        }
        "get_class_context" => {
            let key = q_any(body, "cls", "class");
            let Some((di, ci)) = p.find_class(&key) else {
                return miss("class_context", "CLASS_NOT_FOUND", &format!("class '{key}' not found"));
            };
            let d = &p.dexes[di];
            let def = &d.class_defs[ci];
            let desc = d.type_descriptor(def.class_idx);
            let cd = d.class_data(def);
            let fields: Vec<Json> = cd
                .static_fields
                .iter()
                .chain(cd.instance_fields.iter())
                .map(|ef| Json::str(&d.field_full(ef.field_idx)))
                .collect();
            let methods: Vec<Json> = cd
                .direct_methods
                .iter()
                .chain(cd.virtual_methods.iter())
                .map(|em| Json::str(&d.method_full(em.method_idx)))
                .collect();
            success(
                "class_context",
                Json::obj(vec![("page", Json::Int(page)), ("class", Json::str(&dotted(desc)))]),
                vec![item_symbol(
                    &dotted(desc),
                    &dotted(desc),
                    vec![
                        ("super", Json::str(&dotted(d.type_descriptor(def.superclass_idx)))),
                        (
                            "interfaces",
                            Json::Arr(d.interfaces(def).iter().map(|i| Json::str(&dotted(i))).collect()),
                        ),
                        ("static_fields", Json::Arr(fields)),
                        ("instance_methods", Json::Arr(methods)),
                        ("xref_callers", Json::Int(p.index.type_refs.get(desc).map(|v| v.len()).unwrap_or(0) as i64)),
                    ],
                )],
                Json::obj(vec![]),
            )
        }
        "get_class_source" => {
            let key = q_any(body, "cls", "class");
            let Some((di, ci)) = p.find_class(&key) else {
                return miss("class_source", "CLASS_NOT_FOUND", &format!("class '{key}' not found"));
            };
            let d = &p.dexes[di];
            let def = &d.class_defs[ci];
            let name = dotted(d.type_descriptor(def.class_idx));
            let language_request = q_str(body, "language");
            // dexdec full pipeline first, structural IR as fallback
            let (mut content, mode, lang) = sources::try_dexdec_class(p, &name, &language_request)
                .unwrap_or_else(|| (java::emit_class(d, def, &p.dex_names[di]), "structural-ir", "java"));
            if let Some(l) = filter_j.and_then(|j| j.get("limit")).and_then(|v| v.as_i64()) {
                let lines: Vec<&str> = content.lines().collect();
                let cut = (l as usize).min(lines.len());
                let joined: Vec<String> = lines[..cut].iter().map(|s| s.to_string()).collect();
                content = joined.join("\n");
            }
            success(
                "class_source",
                Json::obj(vec![("page", Json::Int(page)), ("class", Json::str(&name))]),
                vec![item_code(&name, &name, content)],
                Json::obj(vec![("mode", Json::str(mode)), ("language", Json::str(lang))]),
            )
        }
        "search_global_key" => {
            // contract: {key, search:{includes,excludes,caseSensitive,regex,limit}}
            let key = q_str(body, "key");
            if key.trim().is_empty() {
                return miss("search_global_key", "EMPTY_SEARCH_KEY", "key is empty");
            }
            let s = body.get("search").cloned().unwrap_or(Json::obj(vec![]));
            let g = Filter::from_json(Some(&s));
            let mut q = s;
            q.set("page", Json::Int(page));
            let mut items = Vec::new();
            let kmatch = |full: &str| -> bool {
                if g.case_sensitive {
                    full.contains(&key)
                } else {
                    full.to_lowercase().contains(&key.to_lowercase())
                }
            };
            for (di, d) in p.dexes.iter().enumerate() {
                for mi in 0..d.method_ids.len() as u32 {
                    let full = d.method_full(mi);
                    if kmatch(&full) && g.accepts(&full) {
                        items.push(item_symbol(
                            &full,
                            &full,
                            vec![("dex", Json::str(&p.dex_names[di])), ("symbol_type", Json::str("method"))],
                        ));
                    }
                    if let Some(l) = g.limit {
                        if items.len() >= l {
                            break;
                        }
                    }
                }
                for fi in 0..d.field_ids.len() as u32 {
                    let full = d.field_full(fi);
                    if kmatch(&full) && g.accepts(&full) {
                        items.push(item_symbol(
                            &full,
                            &full,
                            vec![("dex", Json::str(&p.dex_names[di])), ("symbol_type", Json::str("field"))],
                        ));
                    }
                    if let Some(l) = g.limit {
                        if items.len() >= l {
                            break;
                        }
                    }
                }
            }
            success("search_global_key", q, items, Json::obj(vec![]))
        }
        "search_class_key" => {
            // contract: {cls, key, grep:{limit,caseSensitive,regex}}
            let key = q_any(body, "cls", "class");
            let needle = q_str(body, "key");
            if needle.trim().is_empty() {
                return miss("search_class_key", "EMPTY_SEARCH_KEY", "key is empty");
            }
            let grep = body.get("grep").cloned().unwrap_or(Json::obj(vec![]));
            let Some((di, ci)) = p.find_class(&key) else {
                return miss("search_class_key", "CLASS_NOT_FOUND", &format!("class '{key}' not found"));
            };
            let g = Filter::from_json(Some(&grep));
            let d = &p.dexes[di];
            let cd = d.class_data(&d.class_defs[ci]);
            let kmatch = |name: &str| -> bool {
                if g.case_sensitive {
                    name.contains(&needle)
                } else {
                    name.to_lowercase().contains(&needle.to_lowercase())
                }
            };
            let mut items = Vec::new();
            for em in cd.direct_methods.iter().chain(cd.virtual_methods.iter()) {
                let full = d.method_full(em.method_idx);
                if kmatch(&full) && (g.includes.is_empty() || g.accepts(&full)) {
                    items.push(item_symbol(&full, &full, vec![("symbol_type", Json::str("method"))]));
                }
            }
            for ef in cd.static_fields.iter().chain(cd.instance_fields.iter()) {
                let full = d.field_full(ef.field_idx);
                if kmatch(&full) && (g.includes.is_empty() || g.accepts(&full)) {
                    items.push(item_symbol(&full, &full, vec![("symbol_type", Json::str("field"))]));
                }
            }
            let mut q = grep;
            q.set("page", Json::Int(page));
            success(
                "search_class_key",
                q,
                items,
                Json::obj(vec![]),
            )
        }
        "search_method" => {
            let key = q_any(body, "mth", "method");
            if key.is_empty() {
                return miss("search_method", "EMPTY_SEARCH_KEY", "method key is empty");
            }
            let mut items = Vec::new();
            for (di, d) in p.dexes.iter().enumerate() {
                for mi in 0..d.method_ids.len() as u32 {
                    let full = d.method_full(mi);
                    let matched = if f.includes.is_empty() {
                        full.contains(&key)
                    } else {
                        f.accepts(&full)
                    };
                    if matched {
                        items.push(item_symbol(
                            &full,
                            &full,
                            vec![("dex", Json::str(&p.dex_names[di])), ("symbol_type", Json::str("method"))],
                        ));
                    }
                    if let Some(l) = f.limit {
                        if items.len() >= l {
                            break;
                        }
                    }
                }
            }
            success("search_method", Json::obj(vec![("page", Json::Int(page)), ("method", Json::str(&key))]), items, Json::obj(vec![]))
        }
        "get_method_source" => {
            let key = q_any(body, "mth", "method");
            let Some((di, mi)) = p.find_method(&key) else {
                return miss("method_source", "METHOD_NOT_FOUND", &format!("method '{key}' not found"));
            };
            let d = &p.dexes[di];
            let cd_owner = d
                .class_defs
                .iter()
                .find(|def| {
                    let cd = d.class_data(def);
                    cd.direct_methods.iter().chain(cd.virtual_methods.iter()).any(|m| m.method_idx == mi)
                });
            let Some(def) = cd_owner else {
                return miss("method_source", "METHOD_NOT_FOUND", "method has no defining class here");
            };
            let cd = d.class_data(def);
            let em = cd
                .direct_methods
                .iter()
                .chain(cd.virtual_methods.iter())
                .find(|m| m.method_idx == mi)
                .unwrap();
            let full = d.method_full(mi);
            let smali = body.get("smali").and_then(|v| v.as_bool()).unwrap_or(false);
            let language = q_str(body, "language");

            // structural fallback body; also the only path for smali output
            let structural = || {
                let mut content = format!("// {full}\n// dex: {}\n", p.dex_names[di]);
                if em.code_off == 0 {
                    content.push_str("// abstract or native: no code\n");
                } else if let Some(body) = java::emit_method_body(d, mi, em.code_off) {
                    content.push_str(&body);
                }
                content
            };

            let (content, mode, lang) = if smali {
                (structural(), "structural-ir", "java")
            } else {
                let owner_dotted = dotted(d.method_class(mi));
                let mname = d.method_name(mi).to_string();
                let descriptor = full
                    .split_once("->")
                    .and_then(|(_, rest)| rest.find('(').map(|i| rest[i..].to_string()));
                sources::try_dexdec_method(p, &owner_dotted, &mname, descriptor, &language)
                    .unwrap_or_else(|| (structural(), "structural-ir", "java"))
            };

            let mut meta = vec![("mode", Json::str(mode)), ("language", Json::str(lang))];
            if smali {
                meta.push(("smali", Json::Bool(true)));
            }
            success(
                "method_source",
                Json::obj(vec![("page", Json::Int(page)), ("method", Json::str(&full))]),
                vec![item_code(&full, &full, content)],
                Json::obj(meta),
            )
        }
        "get_method_context" => {
            let key = q_any(body, "mth", "method");
            let Some((di, mi)) = p.find_method(&key) else {
                return miss("method_context", "METHOD_NOT_FOUND", &format!("method '{key}' not found"));
            };
            let d = &p.dexes[di];
            let full = d.method_full(mi);
            let callers: Vec<Json> =
                p.index.callers_of(&full).iter().map(|c| Json::str(&c.caller)).collect();
            let callees: Vec<Json> =
                p.index.callees_of(&full).iter().map(|c| Json::str(c)).collect();
            success(
                "method_context",
                Json::obj(vec![("page", Json::Int(page)), ("method", Json::str(&full))]),
                vec![item_symbol(
                    &full,
                    &full,
                    vec![
                        ("owner", Json::str(&dotted(d.method_class(mi)))),
                        ("callers", Json::Arr(callers)),
                        ("callees", Json::Arr(callees)),
                    ],
                )],
                Json::obj(vec![]),
            )
        }
        "get_method_cfg" => {
            let key = q_any(body, "mth", "method");
            let Some((di, mi)) = p.find_method(&key) else {
                return miss("method_cfg", "METHOD_NOT_FOUND", &format!("method '{key}' not found"));
            };
            let d = &p.dexes[di];
            let full = d.method_full(mi);
            let mut blocks = String::new();
            if let Some(def) = d.class_defs.iter().find(|def| {
                let cd = d.class_data(def);
                cd.direct_methods.iter().chain(cd.virtual_methods.iter()).any(|m| m.method_idx == mi)
            }) {
                let cd = d.class_data(def);
                if let Some(em) = cd
                    .direct_methods
                    .iter()
                    .chain(cd.virtual_methods.iter())
                    .find(|m| m.method_idx == mi)
                {
                    if let Some(code) = d.code_item(em.code_off) {
                        let insns = decx_core::code::walk(&code.insns);
                        for (l, r) in decx_core::code::basic_blocks(&insns, code.insns.len()) {
                            blocks.push_str(&format!("block {:#06x}-{:#06x}\n", l * 2, r * 2));
                        }
                    }
                }
            }
            success(
                "method_cfg",
                Json::obj(vec![("page", Json::Int(page)), ("method", Json::str(&full))]),
                vec![item_code(&full, &full, blocks)],
                Json::obj(vec![]),
            )
        }
        "get_method_xref" => {
            let key = q_any(body, "mth", "method");
            let Some((_, mi)) = p.find_method(&key) else {
                return miss("method_xref", "METHOD_NOT_FOUND", &format!("method '{key}' not found"));
            };
            let _ = mi;
            let full = p
                .find_method(&key)
                .and_then(|(di, mid)| Some(p.dexes[di].method_full(mid)))
                .unwrap_or_else(|| key.to_string());
            let callers = p.index.callers_of(&full);
            let items: Vec<Json> = callers
                .iter()
                .map(|c| {
                    item_xref(
                        &c.caller,
                        &c.caller,
                        vec![
                            ("direction", Json::str("caller")),
                            ("at", Json::Int(c.pc as i64)),
                            ("kind", Json::str(c.kind)),
                        ],
                    )
                })
                .collect();
            success(
                "method_xref",
                Json::obj(vec![("page", Json::Int(page)), ("method", Json::str(&full))]),
                items,
                Json::obj(vec![]),
            )
        }
        "get_field_xref" => {
            let key = q_any(body, "fld", "field");
            let sites = p.index.field_accessors.get(&key).cloned().unwrap_or_default();
            let items: Vec<Json> = sites
                .iter()
                .map(|s| {
                    item_xref(
                        &s.accessor,
                        &s.accessor,
                        vec![
                            ("direction", Json::str("accessor")),
                            ("at", Json::Int(s.pc as i64)),
                            ("kind", Json::str(s.kind)),
                        ],
                    )
                })
                .collect();
            if items.is_empty() {
                return miss("field_xref", "FIELD_NOT_FOUND", &format!("field '{key}' not found"));
            }
            success("field_xref", Json::obj(vec![("page", Json::Int(page)), ("field", Json::str(&key))]), items, Json::obj(vec![]))
        }
        "get_class_xref" => {
            let key = q_any(body, "cls", "class");
            let Some((di, ci)) = p.find_class(&key) else {
                return miss("class_xref", "CLASS_NOT_FOUND", &format!("class '{key}' not found"));
            };
            let desc = p.dexes[di].type_descriptor(p.dexes[di].class_defs[ci].class_idx);
            let sites = p.index.type_refs.get(desc).cloned().unwrap_or_default();
            let mut items = Vec::new();
            for s in sites.iter().take(f.limit.unwrap_or(500)) {
                if !f.accepts(&s.method) {
                    continue;
                }
                items.push(item_xref(
                    &s.method,
                    &s.method,
                    vec![
                        ("direction", Json::str("reference")),
                        ("at", Json::Int(s.pc as i64)),
                        ("kind", Json::str(s.kind)),
                    ],
                ));
            }
            success(
                "class_xref",
                Json::obj(vec![("page", Json::Int(page)), ("class", Json::str(&dotted(desc)))]),
                items,
                Json::obj(vec![]),
            )
        }
        "get_implementations" => {
            // contract: {iface: "android.content.DialogInterface$OnClickListener"}
            let key = q_any(body, "iface", "interface");
            if key.is_empty() {
                return miss("implementations", "INVALID_PARAMETER", "iface is empty");
            }
            let iface_desc = if key.starts_with('L') && key.ends_with(';') {
                key.clone()
            } else {
                format!("L{};", key.replace('.', "/"))
            };
            let iface_dotted = dotted(&iface_desc);
            let impls = p.implementors_of(&iface_desc);
            if impls.is_empty() {
                return miss(
                    "implementations",
                    "INTERFACE_NOT_FOUND",
                    &format!("interface '{key}' not found"),
                );
            }
            let items: Vec<Json> = impls
                .iter()
                .filter(|s| f.accepts(&dotted(s)))
                .map(|s| {
                    item_symbol(
                        &dotted(s),
                        &dotted(s),
                        vec![("interface", Json::str(&iface_dotted))],
                    )
                })
                .collect();
            success(
                "implementations",
                Json::obj(vec![("page", Json::Int(page)), ("iface", Json::str(&iface_dotted))]),
                items,
                Json::obj(vec![]),
            )
        }
        "get_subclasses" => {
            let key = q_any(body, "cls", "class");
            // parent may be a library class only REFERENCED (not defined) in the
            // app dexes: fall back to descriptor-normalized reference matching
            let desc = if let Some((di, ci)) = p.find_class(&key) {
                p.dexes[di].type_descriptor(p.dexes[di].class_defs[ci].class_idx).to_string()
            } else if key.starts_with('L') && key.ends_with(';') {
                key.clone()
            } else {
                format!("L{};", key.replace('.', "/"))
            };
            let mut all = Vec::new();
            let mut queue = vec![desc.to_string()];
            while let Some(o) = queue.pop() {
                for sub in p.subclasses_of(&o) {
                    if !all.contains(&sub) {
                        all.push(sub.clone());
                        queue.push(sub);
                    }
                }
            }
            let items: Vec<Json> = all
                .iter()
                .filter(|s| f.accepts(&dotted(s)))
                .map(|s| item_symbol(&dotted(s), &dotted(s), vec![("relation", Json::str("subclass"))]))
                .collect();
            success(
                "subclasses",
                Json::obj(vec![("page", Json::Int(page)), ("class", Json::str(&dotted(&desc)))]),
                items,
                Json::obj(vec![]),
            )
        }
        "get_aidl_interfaces" => {
            let mut items = Vec::new();
            for (di, d) in p.dexes.iter().enumerate() {
                for def in &d.class_defs {
                    let desc = d.type_descriptor(def.class_idx);
                    let is_binder = d.type_descriptor(def.superclass_idx) == "Landroid/os/Binder;"
                        || d.type_descriptor(def.superclass_idx) == "Landroid/os/IInterface;"
                        || desc.contains("$Stub");
                    if is_binder && f.accepts(&dotted(desc)) {
                        items.push(item_symbol(
                            &dotted(desc),
                            &dotted(desc),
                            vec![
                                ("dex", Json::str(&p.dex_names[di])),
                                ("role", Json::str("aidl-stub")),
                            ],
                        ));
                    }
                }
            }
            success("aidl_interfaces", empty_q, items, Json::obj(vec![]))
        }
        "get_app_manifest" => {
            let Some(apk) = p.apk.as_ref() else {
                return miss("app_manifest", "MANIFEST_NOT_FOUND", "input is not an apk");
            };
            let Some(m) = apk.manifest.as_ref() else {
                return miss("app_manifest", "MANIFEST_NOT_FOUND", "no AndroidManifest.xml");
            };
            let comps: Vec<Json> = m
                .components
                .iter()
                .map(|c| {
                    Json::obj(vec![
                        ("kind", Json::str(&c.kind)),
                        ("name", Json::str(&c.name)),
                        ("exported", c.exported.map(Json::Bool).unwrap_or(Json::Null)),
                        ("permission", c.permission.clone().map(|p| Json::str(&p)).unwrap_or(Json::Null)),
                        ("actions", Json::Arr(c.intent_actions.iter().map(|a| Json::str(a)).collect())),
                    ])
                })
                .collect();
            success(
                "app_manifest",
                Json::obj(vec![("page", Json::Int(page)), ("package", Json::str(&m.package))]),
                vec![
                    item_symbol(
                        &m.package,
                        &m.package,
                        vec![
                            ("version_name", m.version_name.clone().map(|v| Json::str(&v)).unwrap_or(Json::Null)),
                            ("version_code", m.version_code.map(|v| Json::Int(v)).unwrap_or(Json::Null)),
                            ("min_sdk", m.min_sdk.map(|v| Json::Int(v)).unwrap_or(Json::Null)),
                            ("target_sdk", m.target_sdk.map(|v| Json::Int(v)).unwrap_or(Json::Null)),
                            ("application", m.app_name.clone().map(|v| Json::str(&v)).unwrap_or(Json::Null)),
                            ("permissions", Json::Arr(m.permissions.iter().map(|x| Json::str(x)).collect())),
                            ("components", Json::Arr(comps)),
                            ("xml", apk.manifest_xml.clone().map(|x| Json::str(&x)).unwrap_or(Json::Null)),
                        ],
                    ),
                ],
                Json::obj(vec![]),
            )
        }
        "get_main_activity" => {
            let Some(apk) = p.apk.as_ref() else {
                return miss("main_activity", "MANIFEST_NOT_FOUND", "input is not an apk");
            };
            let Some(m) = apk.manifest.as_ref() else {
                return miss("main_activity", "MANIFEST_NOT_FOUND", "no manifest");
            };
            match m.main_activity() {
                Some(c) => success(
                    "main_activity",
                    Json::obj(vec![("page", Json::Int(page)), ("package", Json::str(&m.package))]),
                    vec![item_symbol(&c.name, &c.name, vec![("kind", Json::str("activity"))])],
                    Json::obj(vec![]),
                ),
                None => miss("main_activity", "NO_MAIN_ACTIVITY", "no launcher activity found"),
            }
        }
        "get_application" => {
            let Some(apk) = p.apk.as_ref() else {
                return miss("application", "NO_APPLICATION", "input is not an apk");
            };
            let Some(m) = apk.manifest.as_ref() else {
                return miss("application", "MANIFEST_NOT_FOUND", "no manifest");
            };
            match &m.app_name {
                Some(a) => success(
                    "application",
                    Json::obj(vec![("page", Json::Int(page)), ("package", Json::str(&m.package))]),
                    vec![item_symbol(a, a, vec![("kind", Json::str("application"))])],
                    Json::obj(vec![]),
                ),
                None => miss("application", "NO_APPLICATION", "android:name not set on <application>"),
            }
        }
        "get_exported_components" => {
            let Some(apk) = p.apk.as_ref() else {
                return miss("exported_components", "MANIFEST_NOT_FOUND", "input is not an apk");
            };
            let Some(m) = apk.manifest.as_ref() else {
                return miss("exported_components", "MANIFEST_NOT_FOUND", "no manifest");
            };
            let items: Vec<Json> = m
                .exported()
                .into_iter()
                .filter(|c| f.accepts(&c.name))
                .map(|c| {
                    item_symbol(
                        &c.name,
                        &c.name,
                        vec![
                            ("kind", Json::str(&c.kind)),
                            ("exported", Json::Bool(true)),
                            (
                                "actions",
                                Json::Arr(c.intent_actions.iter().map(|a| Json::str(a)).collect()),
                            ),
                        ],
                    )
                })
                .collect();
            success(
                "exported_components",
                Json::obj(vec![("page", Json::Int(page)), ("package", Json::str(&m.package))]),
                items,
                Json::obj(vec![]),
            )
        }
        "get_deep_links" => {
            let Some(apk) = p.apk.as_ref() else {
                return miss("deep_links", "MANIFEST_NOT_FOUND", "input is not an apk");
            };
            let Some(m) = apk.manifest.as_ref() else {
                return miss("deep_links", "MANIFEST_NOT_FOUND", "no manifest");
            };
            let items: Vec<Json> = m
                .deep_links()
                .into_iter()
                .map(|(comp, scheme, host)| {
                    item_symbol(
                        &format!("{scheme}://{host}"),
                        &format!("{scheme}://{host}"),
                        vec![("component", Json::str(&comp)), ("scheme", Json::str(&scheme)), ("host", Json::str(&host))],
                    )
                })
                .collect();
            success(
                "deep_links",
                Json::obj(vec![("page", Json::Int(page)), ("package", Json::str(&m.package))]),
                items,
                Json::obj(vec![]),
            )
        }
        "get_dynamic_receivers" => {
            // classes that call registerReceiver (bounded by filter if given)
            let mut items = Vec::new();
            let mut seen = std::collections::BTreeSet::new();
            for (callee, sites) in &p.index.method_callers {
                if callee.contains("registerReceiver") {
                    for s in sites {
                        let owner = s
                            .caller
                            .split("->")
                            .next()
                            .unwrap_or(&s.caller)
                            .to_string();
                        let dotted_owner = dotted(&owner);
                        if !f.accepts(&dotted_owner) || !seen.insert(owner.clone()) {
                            continue;
                        }
                        items.push(item_symbol(
                            &dotted_owner,
                            &dotted_owner,
                            vec![
                                ("kind", Json::str("dynamic-receiver")),
                                ("registered_in", Json::str(&s.caller)),
                            ],
                        ));
                        if let Some(l) = f.limit {
                            if items.len() >= l {
                                break;
                            }
                        }
                    }
                }
            }
            success("dynamic_receivers", empty_q, items, Json::obj(vec![]))
        }
        "get_all_resources" => {
            let mut items = Vec::new();
            for (name, size) in &p.resource_entries {
                if !f.accepts(name) {
                    continue;
                }
                items.push(item_symbol(
                    name,
                    name,
                    vec![("size", Json::Int(*size as i64)), ("kind", Json::str("resource"))],
                ));
                if let Some(l) = f.limit {
                    if items.len() >= l {
                        break;
                    }
                }
            }
            success("all_resources", empty_q, items, Json::obj(vec![]))
        }
        "get_resource_file" => {
            let key = q_any(body, "res", "file");
            if key.is_empty() {
                return miss("resource_file", "INVALID_PARAMETER", "file is empty");
            }
            let Some(apk) = p.apk.as_ref() else {
                return miss("resource_file", "RESOURCE_NOT_FOUND", "input is not an apk");
            };
            match apk.zip.read_by_name(&key) {
                Some(Ok(bytes)) => {
                    let printable = bytes.len() < 512 * 1024
                        && bytes.iter().take(4096).all(|b| *b == b'\n' || *b == b'\r' || *b == b'\t' || (0x20..0x7f).contains(b));
                    let content = if printable {
                        String::from_utf8_lossy(&bytes).into_owned()
                    } else {
                        format!("<binary {} bytes>", bytes.len())
                    };
                    success(
                        "resource_file",
                        Json::obj(vec![("page", Json::Int(page)), ("file", Json::str(&key))]),
                        vec![item_code(&key, &key, content)],
                        Json::obj(vec![("binary", Json::Bool(!printable))]),
                    )
                }
                _ => miss("resource_file", "RESOURCE_NOT_FOUND", &format!("'{key}' not in archive")),
            }
        }
        "get_strings" => {
            let mut items = Vec::new();
            for s in p.all_strings() {
                if !f.accepts(s) {
                    continue;
                }
                let refs = p.index.string_refs.get(s).map(|v| v.len()).unwrap_or(0) as i64;
                items.push(item_symbol(s, s, vec![("refs", Json::Int(refs))]));
                if let Some(l) = f.limit {
                    if items.len() >= l {
                        break;
                    }
                }
            }
            if items.is_empty() {
                return miss("strings", "NO_STRINGS_FOUND", "no strings match the filter");
            }
            success("strings", empty_q, items, Json::obj(vec![]))
        }
        "get_system_service_impl" => {
            // contract: {iface} -> binder stub implementors (framework track)
            let service = q_any(body, "iface", "service");
            if service.is_empty() {
                return miss("system_service_impl", "INVALID_PARAMETER", "iface is empty");
            }
            let iface_desc = if service.starts_with('L') && service.ends_with(';') {
                service.clone()
            } else {
                format!("L{};", service.replace('.', "/"))
            };
            let stub_desc = format!("{}$Stub;", iface_desc.trim_end_matches(';'));
            let impls = p.implementors_of(&stub_desc);
            let mut items = Vec::new();
            if let Some((di, ci)) = p.find_class(&stub_desc) {
                let d = &p.dexes[di];
                let def = &d.class_defs[ci];
                let name = dotted(&stub_desc);
                let content = sources::try_dexdec_class(p, &name, "auto")
                    .map(|(s, _, _)| s)
                    .unwrap_or_else(|| java::emit_class(d, def, &p.dex_names[di]));
                items.push(item_code(&name, &name, content));
            }
            for s in impls {
                if s == stub_desc {
                    continue;
                }
                items.push(item_symbol(
                    &dotted(&s),
                    &dotted(&s),
                    vec![("relation", Json::str("impl"))],
                ));
            }
            if items.is_empty() {
                return miss(
                    "system_service_impl",
                    "SERVICE_IMPL_NOT_FOUND",
                    &format!("no stub/impl for '{service}'"),
                );
            }
            success(
                "system_service_impl",
                Json::obj(vec![("page", Json::Int(page)), ("service", Json::str(&dotted(&iface_desc)))]),
                items,
                Json::obj(vec![]),
            )
        }
        "taint_scan" => {
            // native-only: inter-procedural taint scan (sources/sinks/propagators/sanitizers)
            if p.dexes.is_empty() {
                return miss("taint_scan", "RESOURCE_NOT_FOUND", "no dex loaded");
            }
            let mut opts = decx_taint::ScanOptions::default();
            if let Some(n) = body.get("maxRounds").and_then(|v| v.as_i64()) {
                if (1..=64).contains(&n) {
                    opts.max_rounds = n as usize;
                }
            }
            if let Some(n) = body.get("maxFindings").and_then(|v| v.as_i64()) {
                if (1..=10_000).contains(&n) {
                    opts.max_findings = n as usize;
                }
            }
            let rules = st.taint_rules.clone();
            let out = decx_taint::scan_to_json(p, &opts, &rules);
            let items: Vec<Json> = out
                .get("items")
                .and_then(|v| v.as_arr().map(<[Json]>::to_vec))
                .unwrap_or_default();
            let meta = out.get("meta").cloned().unwrap_or(Json::obj(vec![]));
            success("taint_scan", Json::obj(vec![("page", Json::Int(page))]), items, meta)
        }
        _ => miss("unknown", "UNKNOWN_ENDPOINT", &format!("unknown endpoint '{endpoint}'")),
    }
}
