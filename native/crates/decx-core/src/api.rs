//! Endpoint dispatcher mirroring `DecxRoutes`/`RouteHandler` from the Kotlin
//! core. The server stays thin: route `/api/decx/<endpoint>` → [`dispatch`].
//!
//! Request/response bodies follow the DECX CLI contract (`decx-cli/src/core/client.ts`):
//! filters arrive as `{ filter: { limit, includes, excludes, regex } }` and
//! searches as `{ search: { limit, includes, excludes, caseSensitive, regex } }`.

use serde_json::{json, Value};

use dex_decompiler::Decompiler;
use dex_parser::DexFile;

use crate::error::{DecxError, Result};
use crate::names::{descriptor_to_java, simple_name};
use crate::project::{ClassEntry, MethodEntry, Project};

/// Dispatch one API call by endpoint name. Returns the JSON payload the HTTP
/// layer sends with status 200; errors surface as [`DecxError`].
pub fn dispatch(project: &Project, endpoint: &str, body: &Value) -> Result<Value> {
    match endpoint {
        "get_classes" => get_classes(project, body),
        "get_class_source" => get_class_source(project, body),
        "get_class_context" => get_class_context(project, body),
        "search_global_key" => search_global_key(project, body),
        "search_class_key" => search_class_key(project, body),
        "search_method" => search_method(project, body),
        "get_method_source" => get_method_source(project, body),
        "get_method_context" => get_method_context(project, body),
        "get_method_cfg" => get_method_cfg(project, body),
        "get_method_xref" => get_method_xref(project, body),
        "get_field_xref" => get_field_xref(project, body),
        "get_class_xref" => get_class_xref(project, body),
        "get_implementations" => get_implementations(project, body),
        "get_subclasses" => get_subclasses(project, body),
        "get_app_manifest" => get_app_manifest(project),
        other => Err(DecxError::new("UNKNOWN_ENDPOINT", format!("unknown endpoint: {other}"))),
    }
}

// ── request helpers ─────────────────────────────────────────────────────────

struct ListFilter {
    limit: Option<usize>,
    includes: Vec<String>,
    excludes: Vec<String>,
    regex: bool,
}

fn parse_filter(body: &Value) -> ListFilter {
    let f = body.get("filter");
    let includes = f
        .and_then(|f| f.get("includes"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();
    let excludes = f
        .and_then(|f| f.get("excludes"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();
    ListFilter {
        limit: f.and_then(|f| f.get("limit")).and_then(Value::as_u64).map(|v| v as usize),
        includes,
        excludes,
        regex: f.and_then(|f| f.get("regex")).and_then(Value::as_bool).unwrap_or(true),
    }
}

/// Compile includes/excludes into a predicate over a name. Empty includes = match all.
fn name_filter(filter: &ListFilter) -> Result<impl Fn(&str) -> bool + '_> {
    let inc: Result<Vec<regex::Regex>> = filter
        .includes
        .iter()
        .map(|p| compile_pattern(p, filter.regex))
        .collect();
    let exc: Result<Vec<regex::Regex>> = filter
        .excludes
        .iter()
        .map(|p| compile_pattern(p, filter.regex))
        .collect();
    let inc = inc?;
    let exc = exc?;
    Ok(move |name: &str| {
        let included = inc.is_empty() || inc.iter().any(|r| r.is_match(name));
        included && !exc.iter().any(|r| r.is_match(name))
    })
}

fn compile_pattern(pattern: &str, regex_mode: bool) -> Result<regex::Regex> {
    let src = if regex_mode {
        pattern.to_string()
    } else {
        format!("^{}$", regex::escape(pattern.trim_end_matches('*')).replace("\\*", ".*"))
    };
    // Substring-ish matching for plain patterns: treat as contains-regex.
    let src = if regex_mode && !src.starts_with('^') && !src.ends_with('$') {
        src
    } else if !regex_mode {
        src
    } else {
        src
    };
    regex::Regex::new(&src).map_err(|e| DecxError::invalid_parameter(format!("bad pattern {pattern:?}: {e}")))
}

fn required_str(body: &Value, key: &str) -> Result<String> {
    body.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| DecxError::invalid_parameter(format!("missing required field: {key}")))
}

fn page_of(body: &Value) -> u64 {
    body.get("page").and_then(Value::as_u64).unwrap_or(1).max(1)
}

/// Resolve a `mth` spec ("com.foo.Bar.method", "com.foo.Bar#method", or bare
/// "method") to (class entry, method entry).
fn resolve_method<'a>(project: &'a Project, mth: &str) -> Result<(&'a ClassEntry, &'a MethodEntry)> {
    let spec = mth.trim();
    let (cls_part, mth_part) = if let Some((c, m)) = spec.split_once('#') {
        (Some(c.to_string()), m.to_string())
    } else if let Some((c, m)) = spec.rsplit_once('.') {
        // A dot prefix only counts as a class if it actually indexes to a class.
        (Some(c.to_string()), m.to_string())
    } else {
        (None, spec.to_string())
    };

    let candidates: Vec<&ClassEntry> = match cls_part {
        Some(c) => {
            let entry = project.lookup_fuzzy(&c).ok_or_else(|| DecxError::class_not_found(c))?;
            vec![entry]
        }
        None => project.entries().iter().collect(),
    };
    for entry in candidates {
        let matches: Vec<&MethodEntry> = entry.methods.iter().filter(|m| m.name == mth_part).collect();
        if let Some(m) = matches.first() {
            return Ok((entry, m));
        }
    }
    Err(DecxError::method_not_found(mth))
}

/// Resolve a `fld` spec ("com.foo.Bar.field", "com.foo.Bar#field", or "field").
fn resolve_field<'a>(project: &'a Project, fld: &str) -> Result<(&'a ClassEntry, &'a crate::project::FieldEntry)> {
    let spec = fld.trim();
    let (cls_part, fld_part) = if let Some((c, f)) = spec.split_once('#') {
        (Some(c.to_string()), f.to_string())
    } else if let Some((c, f)) = spec.rsplit_once('.') {
        (Some(c.to_string()), f.to_string())
    } else {
        (None, spec.to_string())
    };
    let candidates: Vec<&ClassEntry> = match cls_part {
        Some(c) => {
            let entry = project.lookup_fuzzy(&c).ok_or_else(|| DecxError::class_not_found(c))?;
            vec![entry]
        }
        None => project.entries().iter().collect(),
    };
    for entry in candidates {
        if let Some(f) = entry.fields.iter().find(|f| f.name == fld_part) {
            return Ok((entry, f));
        }
    }
    Err(DecxError::field_not_found(fld))
}

fn method_json(entry: &ClassEntry, m: &MethodEntry) -> Value {
    json!({
        "cls": entry.java_name,
        "name": m.name,
        "signature": format!(
            "{} {}({})",
            descriptor_to_java(&m.return_descriptor),
            m.name,
            m.param_descriptors.iter().map(|p| descriptor_to_java(p)).collect::<Vec<_>>().join(", ")
        ),
        "descriptor": format!("{}{}", m.return_descriptor, m.short_descriptor()),
        "accessFlags": m.access_flags,
        "hasCode": m.code_off != 0,
    })
}

fn smali_rows_to_text(rows: &[dex_decompiler::MethodBytecodeRow]) -> String {
    let mut out = String::new();
    for r in rows {
        out.push_str(&format!("{:6x}: {:<28} {}\n", r.offset, r.mnemonic, r.operands));
    }
    out
}

/// Smali-ish rendering of a whole class (headers + method bodies as bytecode).
fn class_smali(project: &Project, entry: &ClassEntry) -> String {
    let dex = &project.dexes[entry.dex_idx];
    let Ok(class_def) = dex.get_class_def(entry.class_def_idx as u32) else {
        return format!("# failed to reparse {}", entry.java_name);
    };
    let mut out = String::new();
    out.push_str(&format!(".class {}\n", entry.dex_name));
    if let Some(sup) = &entry.superclass_java {
        out.push_str(&format!(".super {}\n", sup));
    }
    for f in &entry.fields {
        out.push_str(&format!(".field {}:{} {}\n", f.name, f.type_descriptor, f.access_flags));
    }
    let Ok(Some(class_data)) = dex.get_class_data(&class_def) else {
        return out;
    };
    let decompiler = Decompiler::new(dex);
    for m in class_data.direct_methods.iter().chain(class_data.virtual_methods.iter()) {
        let Ok(info) = dex.get_method_info(m.method_idx) else { continue };
        out.push_str(&format!(
            "\n.method {} {}({}){}\n",
            info.name,
            info.params.join(""),
            "",
            info.return_type
        ));
        if m.code_off != 0 {
            if let Ok((rows, _, _)) = decompiler.get_method_bytecode_and_cfg(m) {
                out.push_str(&smali_rows_to_text(&rows));
            }
        }
        out.push_str(".end method\n");
    }
    out
}

// ── endpoints ───────────────────────────────────────────────────────────────

fn get_classes(project: &Project, body: &Value) -> Result<Value> {
    let filter = parse_filter(body);
    let pred = name_filter(&filter)?;
    let page = page_of(body);
    let mut names: Vec<&ClassEntry> = project
        .entries()
        .iter()
        .filter(|e| pred(&e.java_name) || pred(&e.dex_name))
        .collect();
    names.sort_by(|a, b| a.java_name.cmp(&b.java_name));
    let total = names.len();
    let limit = filter.limit.unwrap_or(200).min(2000);
    let start = ((page - 1) * limit as u64) as usize;
    let page_items: Vec<Value> = names
        .into_iter()
        .skip(start)
        .take(limit)
        .map(|e| {
            json!({
                "name": e.java_name,
                "dexName": e.dex_name,
                "accessFlags": e.access_flags,
                "superclass": e.superclass_java,
                "interfaces": e.interfaces_java,
                "methodCount": e.methods.len(),
                "fieldCount": e.fields.len(),
            })
        })
        .collect();
    Ok(json!({
        "total": total,
        "page": page,
        "limit": limit,
        "classes": page_items,
    }))
}

fn get_class_source(project: &Project, body: &Value) -> Result<Value> {
    let cls = required_str(body, "cls")?;
    let entry = project.lookup_fuzzy(&cls).ok_or_else(|| DecxError::class_not_found(cls))?;
    let smali = body.get("smali").and_then(Value::as_bool).unwrap_or(false);
    let limit = body
        .get("filter")
        .and_then(|f| f.get("limit"))
        .and_then(Value::as_u64)
        .map(|v| v as usize);
    let mut source = if smali {
        class_smali(project, entry)
    } else {
        (*project.class_source(entry)?).clone()
    };
    if let Some(limit) = limit {
        let line_count = source.lines().count();
        let mut kept: Vec<String> = source.lines().take(limit).map(str::to_string).collect();
        if line_count > limit {
            kept.push(format!("// ... truncated at {limit} lines (filter.limit)"));
        }
        source = kept.join("\n");
    }
    Ok(json!({ "cls": entry.java_name, "source": source }))
}

fn get_class_context(project: &Project, body: &Value) -> Result<Value> {
    let cls = required_str(body, "cls")?;
    let entry = project.lookup_fuzzy(&cls).ok_or_else(|| DecxError::class_not_found(cls))?;
    let source = project.class_source(entry)?;
    Ok(json!({
        "cls": entry.java_name,
        "dexName": entry.dex_name,
        "superclass": entry.superclass_java,
        "interfaces": entry.interfaces_java,
        "source": &*source,
        "methods": entry.methods.iter().map(|m| method_json(entry, m)).collect::<Vec<_>>(),
        "fields": entry.fields.iter().map(|f| json!({
            "name": f.name,
            "type": descriptor_to_java(&f.type_descriptor),
            "accessFlags": f.access_flags,
        })).collect::<Vec<_>>(),
    }))
}

fn search_global_key(project: &Project, body: &Value) -> Result<Value> {
    let key = required_str(body, "key")?;
    if key.trim().is_empty() {
        return Err(DecxError::new("EMPTY_SEARCH_KEY", "search key must not be empty"));
    }
    let search = body.get("search");
    let limit = search
        .and_then(|s| s.get("limit"))
        .and_then(Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(100)
        .min(1000);
    let case_sensitive = search
        .and_then(|s| s.get("caseSensitive"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let use_regex = search.and_then(|s| s.get("regex")).and_then(Value::as_bool).unwrap_or(false);
    let matcher = SearchMatcher::compile(&key, use_regex, case_sensitive)?;
    let filter = parse_filter(body);
    let pred = name_filter(&filter)?;

    // Decompile all matching classes through the chunked batch path (reused
    // engine decompilers), then grep the cached sources. Failures are isolated.
    let matching: Vec<usize> = project
        .entries()
        .iter()
        .enumerate()
        .filter(|(_, e)| pred(&e.java_name))
        .map(|(i, _)| i)
        .collect();
    let (_ok, failures) = project.decompile_batch(&matching);

    let page = page_of(body);
    let skip = ((page - 1) * limit as u64) as usize;
    let mut hits: Vec<Value> = Vec::new();
    for &i in &matching {
        let e = &project.entries()[i];
        let Ok(source) = project.class_source(e) else { continue };
        let mut lines: Vec<Value> = Vec::new();
        for (lineno, line) in source.lines().enumerate() {
            let matched = if use_regex {
                matcher.is_match(line)
            } else if case_sensitive {
                line.contains(&key)
            } else {
                line.to_lowercase().contains(&key.to_lowercase())
            };
            if matched {
                lines.push(json!({ "line": lineno + 1, "text": line }));
            }
        }
        if !lines.is_empty() {
            hits.push(json!({ "cls": e.java_name, "matches": lines }));
        }
    }
    hits.sort_by(|a, b| {
        a.get("cls")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(b.get("cls").and_then(Value::as_str).unwrap_or(""))
    });
    let total = hits.len();
    let page_items: Vec<Value> = hits.into_iter().skip(skip).take(limit).collect();
    Ok(json!({
        "key": key,
        "page": page,
        "total": total,
        "results": page_items,
    }))
}

struct SearchMatcher {
    regex: Option<regex::Regex>,
}

impl SearchMatcher {
    fn compile(pattern: &str, use_regex: bool, case_sensitive: bool) -> Result<Self> {
        if !use_regex {
            return Ok(Self { regex: None });
        }
        let mut src = pattern.to_string();
        if !case_sensitive {
            src = format!("(?i){src}");
        }
        let re = regex::Regex::new(&src)
            .map_err(|e| DecxError::invalid_parameter(format!("bad regex {pattern:?}: {e}")))?;
        Ok(Self { regex: Some(re) })
    }

    fn is_match(&self, line: &str) -> bool {
        match &self.regex {
            Some(re) => re.is_match(line),
            None => false,
        }
    }
}

fn search_class_key(project: &Project, body: &Value) -> Result<Value> {
    let cls = required_str(body, "cls")?;
    let key = required_str(body, "key")?;
    let entry = project.lookup_fuzzy(&cls).ok_or_else(|| DecxError::class_not_found(cls))?;
    let grep = body.get("grep");
    let limit = grep
        .and_then(|g| g.get("limit"))
        .and_then(Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(100);
    let case_sensitive = grep
        .and_then(|g| g.get("caseSensitive"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let use_regex = grep.and_then(|g| g.get("regex")).and_then(Value::as_bool).unwrap_or(false);
    let source = project.class_source(entry)?;
    let mut matches = Vec::new();
    for (lineno, line) in source.lines().enumerate() {
        let hit = if use_regex {
            let re = regex::Regex::new(&if case_sensitive {
                key.clone()
            } else {
                format!("(?i){key}")
            });
            match re {
                Ok(re) => re.is_match(line),
                Err(_) => false,
            }
        } else if case_sensitive {
            line.contains(&key)
        } else {
            line.to_lowercase().contains(&key.to_lowercase())
        };
        if hit {
            matches.push(json!({ "line": lineno + 1, "text": line }));
            if matches.len() >= limit {
                break;
            }
        }
    }
    Ok(json!({ "cls": entry.java_name, "key": key, "matches": matches }))
}

fn search_method(project: &Project, body: &Value) -> Result<Value> {
    let mth = required_str(body, "mth")?;
    let page = page_of(body);
    let limit = 100usize;
    let (cls_filter, name_part) = match mth.rsplit_once('.') {
        Some((c, m)) if project.lookup(c).is_some() => (Some(c.to_string()), m.to_string()),
        _ => (None, mth.clone()),
    };
    let mut hits: Vec<Value> = Vec::new();
    for entry in project.entries() {
        if let Some(cf) = &cls_filter {
            if &entry.java_name != cf {
                continue;
            }
        }
        for m in &entry.methods {
            if m.name.contains(&name_part) {
                hits.push(method_json(entry, m));
                if hits.len() >= limit {
                    break;
                }
            }
        }
        if hits.len() >= limit {
            break;
        }
    }
    Ok(json!({ "mth": mth, "page": page, "total": hits.len(), "methods": hits }))
}

fn get_method_source(project: &Project, body: &Value) -> Result<Value> {
    let mth = required_str(body, "mth")?;
    let (entry, m) = resolve_method(project, &mth)?;
    let smali = body.get("smali").and_then(Value::as_bool).unwrap_or(false);
    let source = if smali {
        let dex = &project.dexes[entry.dex_idx];
        let class_def = dex.get_class_def(entry.class_def_idx as u32).map_err(|e| DecxError::internal(e.to_string()))?;
        let class_data = dex
            .get_class_data(&class_def)
            .map_err(|e| DecxError::internal(e.to_string()))?
            .ok_or_else(|| DecxError::internal("no class_data"))?;
        let encoded = class_data
            .direct_methods
            .iter()
            .chain(class_data.virtual_methods.iter())
            .find(|x| x.method_idx == m.method_idx)
            .ok_or_else(|| DecxError::method_not_found(&mth))?;
        let decompiler = Decompiler::new(dex);
        let (rows, _, _) = decompiler
            .get_method_bytecode_and_cfg(encoded)
            .map_err(|e| DecxError::internal(e.to_string()))?;
        smali_rows_to_text(&rows)
    } else {
        let dex = &project.dexes[entry.dex_idx];
        let class_def = dex.get_class_def(entry.class_def_idx as u32).map_err(|e| DecxError::internal(e.to_string()))?;
        let class_data = dex
            .get_class_data(&class_def)
            .map_err(|e| DecxError::internal(e.to_string()))?
            .ok_or_else(|| DecxError::internal("no class_data"))?;
        let encoded = class_data
            .direct_methods
            .iter()
            .chain(class_data.virtual_methods.iter())
            .find(|x| x.method_idx == m.method_idx)
            .ok_or_else(|| DecxError::method_not_found(&mth))?;
        let extras: Vec<&DexFile> = project
            .dexes
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != entry.dex_idx)
            .map(|(_, d)| d)
            .collect();
        let decompiler =
            Decompiler::with_options(dex, dex_decompiler::DecompilerOptions::default()).with_extra_dexes(extras);
        decompiler
            .decompile_method(
                encoded,
                Some(simple_name(&entry.java_name)),
                Some(&entry.java_name),
            )
            .map_err(|e| DecxError::new("DECOMPILATION_SKIPPED", format!("{e}")))?
    };
    Ok(json!({ "mth": mth, "cls": entry.java_name, "source": source }))
}

fn get_method_context(project: &Project, body: &Value) -> Result<Value> {
    let mth = required_str(body, "mth")?;
    let mut src_body = body.clone();
    src_body["smali"] = Value::Bool(false);
    let source = get_method_source(project, &src_body)?;
    let cfg = get_method_cfg(project, &src_body)?;
    Ok(json!({
        "mth": mth,
        "source": source.get("source").cloned().unwrap_or(Value::Null),
        "cfg": cfg.get("cfg").cloned().unwrap_or(Value::Null),
    }))
}

fn get_method_cfg(project: &Project, body: &Value) -> Result<Value> {
    let mth = required_str(body, "mth")?;
    let (entry, m) = resolve_method(project, &mth)?;
    let dex = &project.dexes[entry.dex_idx];
    let class_def = dex.get_class_def(entry.class_def_idx as u32).map_err(|e| DecxError::internal(e.to_string()))?;
    let class_data = dex
        .get_class_data(&class_def)
        .map_err(|e| DecxError::internal(e.to_string()))?
        .ok_or_else(|| DecxError::internal("no class_data"))?;
    let encoded = class_data
        .direct_methods
        .iter()
        .chain(class_data.virtual_methods.iter())
        .find(|x| x.method_idx == m.method_idx)
        .ok_or_else(|| DecxError::method_not_found(&mth))?;
    let decompiler = Decompiler::new(dex);
    let (rows, nodes, edges) = decompiler
        .get_method_bytecode_and_cfg(encoded)
        .map_err(|e| DecxError::internal(e.to_string()))?;
    Ok(json!({
        "mth": mth,
        "cfg": {
            "nodes": nodes.iter().map(|n| json!({
                "id": n.id, "startOffset": n.start_offset, "endOffset": n.end_offset, "label": n.label,
            })).collect::<Vec<_>>(),
            "edges": edges.iter().map(|e| json!({ "from": e.from_id, "to": e.to_id })).collect::<Vec<_>>(),
        },
        "bytecode": rows.iter().map(|r| json!({
            "offset": r.offset, "mnemonic": r.mnemonic, "operands": r.operands,
        })).collect::<Vec<_>>(),
    }))
}

/// Callers of a method across every dex in the project.
fn get_method_xref(project: &Project, body: &Value) -> Result<Value> {
    let mth = required_str(body, "mth")?;
    let (entry, m) = resolve_method(project, &mth)?;
    let mut callers: Vec<Value> = Vec::new();
    let mut truncated = false;
    for (dex_idx, dex) in project.dexes.iter().enumerate() {
        // The engine xref API is per-dex: re-resolve the method inside each dex.
        let mut target: Option<u32> = None;
        for i in 0..dex.header.class_defs_size {
            let Ok(cdef) = dex.get_class_def(i) else { continue };
            let Ok(Some(cdata)) = dex.get_class_data(&cdef) else { continue };
            let Some(encoded) = cdata
                .direct_methods
                .iter()
                .chain(cdata.virtual_methods.iter())
                .find(|x| x.method_idx != 0 && {
                    match dex.get_method_info(x.method_idx) {
                        Ok(info) => info.class == entry.dex_name && info.name == m.name,
                        Err(_) => false,
                    }
                })
            else {
                continue;
            };
            target = Some(encoded.method_idx);
            break;
        }
        let Some(method_idx) = target else { continue };
        let Ok(info) = dex_decompiler::find_method_callers(dex, method_idx) else { continue };
        truncated |= info.truncated;
        for c in info.callers {
            callers.push(json!({
                "cls": c.class_name,
                "method": c.method_name,
                "descriptor": c.method_descriptor,
                "offset": c.offset,
                "invokeKind": c.invoke_kind,
                "dex": dex_idx,
            }));
        }
    }
    Ok(json!({
        "mth": mth,
        "cls": entry.java_name,
        "total": callers.len(),
        "truncated": truncated,
        "callers": callers,
    }))
}

fn get_field_xref(project: &Project, body: &Value) -> Result<Value> {
    let fld = required_str(body, "fld")?;
    let (entry, f) = resolve_field(project, &fld)?;
    let mut references: Vec<Value> = Vec::new();
    for (dex_idx, dex) in project.dexes.iter().enumerate() {
        // find the per-dex field_idx matching (class, name)
        let mut target: Option<u32> = None;
        for i in 0..dex.header.field_ids_size {
            if let Ok(info) = dex.get_field_info(i) {
                if info.class == entry.dex_name && info.name == f.name {
                    target = Some(i);
                    break;
                }
            }
        }
        let Some(field_idx) = target else { continue };
        let Ok(xrefs) = dex_decompiler::find_field_xrefs(dex, field_idx) else { continue };
        for x in xrefs.xrefs {
            references.push(json!({
                "cls": x.class_name,
                "method": x.method_name,
                "offset": x.offset,
                "kind": x.access_kind,
                "dex": dex_idx,
            }));
        }
    }
    Ok(json!({
        "fld": fld,
        "cls": entry.java_name,
        "total": references.len(),
        "references": references,
    }))
}

/// Structural class references: who extends / implements / holds-a / signs-with
/// the target class. (Code-level type refs are not scanned in this version.)
fn get_class_xref(project: &Project, body: &Value) -> Result<Value> {
    let cls = required_str(body, "cls")?;
    let entry = project.lookup_fuzzy(&cls).ok_or_else(|| DecxError::class_not_found(cls))?;
    let target_desc = &entry.dex_name;
    let mut super_of = Vec::new();
    let mut implements = Vec::new();
    let mut field_type = Vec::new();
    let mut signature = Vec::new();
    for e in project.entries() {
        if e.java_name == entry.java_name {
            continue;
        }
        if e.superclass_java.as_deref() == Some(entry.java_name.as_str()) {
            super_of.push(e.java_name.clone());
        }
        if e.interfaces_java.iter().any(|i| i == &entry.java_name) {
            implements.push(e.java_name.clone());
        }
        let field_hit = e.fields.iter().any(|f| &f.type_descriptor == target_desc);
        if field_hit {
            field_type.push(e.java_name.clone());
        }
        let sig_hit = e.methods.iter().any(|m| {
            &m.return_descriptor == target_desc || m.param_descriptors.iter().any(|p| p == target_desc)
        });
        if sig_hit {
            signature.push(e.java_name.clone());
        }
    }
    Ok(json!({
        "cls": entry.java_name,
        "extendedBy": super_of,
        "implementedBy": implements,
        "usedAsFieldTypeBy": field_type,
        "usedInSignatureBy": signature,
    }))
}

fn get_implementations(project: &Project, body: &Value) -> Result<Value> {
    let iface = required_str(body, "iface")?;
    let entry = project.lookup_fuzzy(&iface).ok_or_else(|| DecxError::class_not_found(iface))?;
    // direct implementers, then one level of subclasses of those implementers
    let direct: Vec<&ClassEntry> = project
        .entries()
        .iter()
        .filter(|e| e.interfaces_java.iter().any(|i| i == &entry.java_name))
        .collect();
    let direct_names: Vec<String> = direct.iter().map(|e| e.java_name.clone()).collect();
    let mut indirect: Vec<String> = Vec::new();
    for e in project.entries() {
        if let Some(sup) = &e.superclass_java {
            if direct_names.iter().any(|d| d == sup) && !direct_names.contains(&e.java_name) {
                indirect.push(e.java_name.clone());
            }
        }
    }
    Ok(json!({
        "iface": entry.java_name,
        "direct": direct_names,
        "indirect": indirect,
    }))
}

fn get_subclasses(project: &Project, body: &Value) -> Result<Value> {
    let cls = required_str(body, "cls")?;
    let entry = project.lookup_fuzzy(&cls).ok_or_else(|| DecxError::class_not_found(cls))?;
    let subs: Vec<String> = project
        .entries()
        .iter()
        .filter(|e| e.superclass_java.as_deref() == Some(entry.java_name.as_str()))
        .map(|e| e.java_name.clone())
        .collect();
    Ok(json!({ "cls": entry.java_name, "subclasses": subs }))
}

fn get_app_manifest(project: &Project) -> Result<Value> {
    match &project.manifest_raw {
        Some(bytes) => match std::str::from_utf8(bytes) {
            Ok(text) => Ok(json!({ "manifest": text, "encoding": "utf8" })),
            Err(_) => Ok(json!({
                "manifest": base64_encode(bytes),
                "encoding": "base64(axml)",
            })),
        },
        None => Err(DecxError::new(
            "MANIFEST_NOT_FOUND",
            "target is not an APK (or has no AndroidManifest.xml)",
        )),
    }
}

fn base64_encode(data: &[u8]) -> String {
    const TBL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = u32::from_be_bytes([0, b[0], b[1], b[2]]);
        out.push(TBL[(n >> 18) as usize & 63] as char);
        out.push(TBL[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TBL[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TBL[n as usize & 63] as char } else { '=' });
    }
    out
}
