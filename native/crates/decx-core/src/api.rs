//! Endpoint dispatcher mirroring `DecxRoutes`/`RouteHandler` from the Kotlin
//! core. The server stays thin: `POST /api/decx/<endpoint>` → [`dispatch`].
//!
//! Every response uses the shared `DecxApiResult` envelope ([`crate::envelope`]):
//! success `{ok, kind, query, summary, items, page}` and error
//! `{ok, kind, query, error:{code, message}}`, with pagination semantics
//! matching the Kotlin `AnalysisResultUtils`.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use dexdec::api::{MemberKind, ReferenceTarget};

use crate::error::{DecxError, Result};
use crate::envelope::{error_response, Item, SuccessResponse};
use crate::names::{descriptor_to_java, split_method_descriptor};
use crate::project::{ClassEntry, Project};

/// Kind string for an endpoint (mirrors Kotlin `DecxKind`), used by the HTTP
/// layer to label error envelopes for unknown/failing endpoints.
pub fn endpoint_kind(endpoint: &str) -> Option<&'static str> {
    Some(match endpoint {
        "get_classes" => "classes",
        "get_class_context" => "class_context",
        "get_class_source" => "class_source",
        "search_global_key" => "search_global",
        "search_class_key" => "search_class",
        "search_method" => "search_method",
        "get_method_source" => "method_source",
        "get_method_context" => "method_context",
        "get_method_cfg" => "method_cfg",
        "get_method_xref" => "method_xref",
        "get_field_xref" => "field_xref",
        "get_class_xref" => "class_xref",
        "get_implementations" => "implementations",
        "get_subclasses" => "subclasses",
        "get_aidl_interfaces" => "aidl_interfaces",
        "get_app_manifest" => "app_manifest",
        "get_main_activity" => "main_activity",
        "get_application" => "application",
        "get_exported_components" => "exported_components",
        "get_deep_links" => "deep_links",
        "get_dynamic_receivers" => "dynamic_receivers",
        "get_all_resources" => "all_resources",
        "get_resource_file" => "resource_file",
        "get_strings" => "strings",
        "get_system_service_impl" => "system_service_impl",
        _ => return None,
    })
}

/// Dispatch one API call. Success returns the full paginated envelope JSON;
/// errors surface as [`DecxError`] for the HTTP layer to wrap.
pub fn dispatch(project: &Project, endpoint: &str, body: &Value) -> Result<Value> {
    if endpoint_kind(endpoint).is_none() {
        return Err(DecxError::new(
            "UNKNOWN_ENDPOINT",
            format!("unknown endpoint: {endpoint}"),
        ));
    }
    match endpoint {
        "get_classes" => get_classes(project, body),
        "get_class_context" => get_class_context(project, body),
        "get_class_source" => get_class_source(project, body),
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
        "get_aidl_interfaces" => get_aidl_interfaces(project, body),
        "get_app_manifest" => get_app_manifest(project, body),
        "get_main_activity" => get_main_activity(project, body),
        "get_application" => get_application(project, body),
        "get_exported_components" => get_exported_components(project, body),
        "get_deep_links" => get_deep_links(project, body),
        "get_dynamic_receivers" => get_dynamic_receivers(project, body),
        "get_all_resources" => get_all_resources(project, body),
        "get_resource_file" => get_resource_file(project, body),
        "get_strings" => get_strings(project, body),
        "get_system_service_impl" => get_system_service_impl(project, body),
        _ => unreachable!("endpoint_kind filtered unknown endpoints"),
    }
}

/// Error envelope for a failed dispatch (query = raw request arguments).
pub fn error_envelope(endpoint: &str, body: &Value, err: &DecxError) -> Value {
    let kind = endpoint_kind(endpoint).unwrap_or("unknown");
    let query = Value::Object(
        body.as_object()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|(k, v)| k != "page" && !v.is_null())
            .collect(),
    );
    error_response(kind, &query, &err.code, &err.message)
}

// ── request helpers ─────────────────────────────────────────────────────────

struct ListFilter {
    includes: Vec<String>,
    excludes: Vec<String>,
    regex: bool,
    limit: Option<usize>,
}

fn parse_filter_at(body: &Value, key: &str) -> ListFilter {
    let f = body.get(key);
    ListFilter {
        includes: str_list(f, "includes"),
        excludes: str_list(f, "excludes"),
        regex: f
            .and_then(|f| f.get("regex"))
            .and_then(Value::as_bool)
            .unwrap_or(true),
        limit: f
            .and_then(|f| f.get("limit"))
            .and_then(Value::as_u64)
            .map(|v| v as usize),
    }
}

/// `get_exported_components` sends its type filter at the top level.
fn parse_top_level_filter(body: &Value) -> ListFilter {
    ListFilter {
        includes: str_list(Some(body), "includes"),
        excludes: str_list(Some(body), "excludes"),
        regex: body
            .get("regex")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        limit: body.get("limit").and_then(Value::as_u64).map(|v| v as usize),
    }
}

fn str_list(obj: Option<&Value>, key: &str) -> Vec<String> {
    obj.and_then(|o| o.get(key))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

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
        (inc.is_empty() || inc.iter().any(|r| r.is_match(name)))
            && !exc.iter().any(|r| r.is_match(name))
    })
}

fn compile_pattern(pattern: &str, regex_mode: bool) -> Result<regex::Regex> {
    let src = if regex_mode {
        pattern.to_string()
    } else {
        format!("^{}$", regex::escape(pattern.trim_end_matches('*')).replace("\\*", ".*"))
    };
    regex::Regex::new(&src)
        .map_err(|e| DecxError::invalid_parameter(format!("bad pattern {pattern:?}: {e}")))
}

fn required_str(body: &Value, key: &str) -> Result<String> {
    body.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| DecxError::invalid_parameter(format!("missing required field: {key}")))
}

fn page_of(body: &Value) -> u64 {
    body.get("page").and_then(Value::as_u64).unwrap_or(1).max(1)
}

fn bool_of(body: &Value, key: &str) -> bool {
    body.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// Kotlin `DecxFilter.toQuery()` shape for the object stored under `key`
/// (`filter`/`search`/`grep`): include limit/includes/excludes when set,
/// `caseSensitive` only when true, `regex` only when false.
fn filter_query(body: &Value, key: &str) -> Vec<(String, Value)> {
    let f = if key.is_empty() {
        // Top-level filter shape (`get_exported_components`).
        return top_level_filter_query(body);
    } else {
        match body.get(key) {
            Some(f) => f,
            None => return vec![],
        }
    };
    let mut out = Vec::new();
    if let Some(limit) = f.get("limit").and_then(Value::as_u64) {
        out.push(("limit".into(), json!(limit)));
    }
    let includes = str_list(Some(f), "includes");
    if !includes.is_empty() {
        out.push(("includes".into(), json!(includes)));
    }
    let excludes = str_list(Some(f), "excludes");
    if !excludes.is_empty() {
        out.push(("excludes".into(), json!(excludes)));
    }
    if f.get("caseSensitive").and_then(Value::as_bool) == Some(true) {
        out.push(("caseSensitive".into(), json!(true)));
    }
    if f.get("regex").and_then(Value::as_bool) == Some(false) {
        out.push(("regex".into(), json!(false)));
    }
    out
}

/// Kotlin top-level `ExportedComponentOptions.toQuery()` shape.
fn top_level_filter_query(body: &Value) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    let includes = str_list(Some(body), "includes");
    if !includes.is_empty() {
        out.push(("includes".into(), json!(includes)));
    }
    let excludes = str_list(Some(body), "excludes");
    if !excludes.is_empty() {
        out.push(("excludes".into(), json!(excludes)));
    }
    if body.get("regex").and_then(Value::as_bool) == Some(false) {
        out.push(("regex".into(), json!(false)));
    }
    out
}

fn query_from(pairs: Vec<(String, Value)>) -> Value {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k, v);
    }
    Value::Object(m)
}

fn simple_name(java_name: &str) -> &str {
    java_name.rsplit('.').next().unwrap_or(java_name)
}

/// jadx-style method signature: `owner#name(param,types):return`.
fn method_signature(owner_java: &str, name: &str, descriptor: &str) -> String {
    let (params, ret) = split_method_descriptor(descriptor);
    let params_java: Vec<String> = params.iter().map(|d| descriptor_to_java(d)).collect();
    format!(
        "{}#{name}({}):{}",
        owner_java,
        params_java.join(","),
        descriptor_to_java(&ret)
    )
}

/// jadx-style field signature: `owner#name:type`.
fn field_signature(owner_java: &str, name: &str, descriptor: &str) -> String {
    format!("{}#{name}:{}", owner_java, descriptor_to_java(descriptor))
}

/// Strip the `(params):ret` tail from a signature method name part.
fn strip_signature_tail(name: &str) -> String {
    name.split('(').next().unwrap_or(name).to_string()
}

/// Resolve a `mth` spec — `owner#name(params):ret`, `owner#name`, `owner.name`,
/// or bare `name` — to (class entry, method name).
fn resolve_method<'a>(project: &'a Project, mth: &str) -> Result<(&'a ClassEntry, String)> {
    let spec = mth.trim().replace(' ', "");
    let (cls_part, mth_part) = if let Some((c, m)) = spec.split_once('#') {
        (Some(c.to_string()), strip_signature_tail(&m))
    } else if let Some((c, m)) = spec.rsplit_once('.') {
        (Some(c.to_string()), m.to_string())
    } else {
        (None, strip_signature_tail(&spec))
    };

    if let Some(c) = cls_part {
        let entry = project
            .lookup_fuzzy(&c)
            .ok_or_else(|| DecxError::class_not_found(c))?;
        let outline = project.class_outline(entry)?;
        if outline.methods.iter().any(|m| m.name == mth_part) {
            return Ok((entry, mth_part));
        }
        return Err(DecxError::method_not_found(mth));
    }
    // Bare method name: one metadata pass over the archive member catalog
    // (no per-class outline loads, which are far more expensive).
    for member in project.members()? {
        if member.kind == dexdec::api::MemberKind::Method && member.name == mth_part {
            let owner_java = crate::names::descriptor_to_java(&member.owner);
            if let Some(entry) = project.lookup(&owner_java) {
                return Ok((entry, mth_part));
            }
        }
    }
    Err(DecxError::method_not_found(mth))
}

/// Resolve a `fld` spec to (class entry, field name).
fn resolve_field<'a>(project: &'a Project, fld: &str) -> Result<(&'a ClassEntry, String)> {
    let spec = fld.trim();
    let (cls_part, fld_part) = if let Some((c, f)) = spec.split_once('#') {
        (Some(c.to_string()), strip_signature_tail(&f))
    } else if let Some((c, f)) = spec.rsplit_once('.') {
        (Some(c.to_string()), f.to_string())
    } else {
        (None, spec.to_string())
    };

    if let Some(c) = cls_part {
        let entry = project
            .lookup_fuzzy(&c)
            .ok_or_else(|| DecxError::class_not_found(c))?;
        let outline = project.class_outline(entry)?;
        if outline.fields.iter().any(|f| f.name == fld_part) {
            return Ok((entry, fld_part));
        }
        return Err(DecxError::field_not_found(fld));
    }
    for entry in project.entries() {
        let Ok(outline) = project.class_outline(entry) else {
            continue;
        };
        if outline.fields.iter().any(|f| f.name == fld_part) {
            return Ok((entry, fld_part));
        }
    }
    Err(DecxError::field_not_found(fld))
}

/// Find the `ordinal`-th (0-based) source line in `entry` mentioning any of
/// `tokens`, skipping `import` lines. Used to attach a source line + snippet
/// to each xref location.
fn usage_line(
    project: &Project,
    entry: &ClassEntry,
    tokens: &[&str],
    ordinal: usize,
) -> Option<(usize, String)> {
    if tokens.is_empty() {
        return None;
    }
    let source = project.class_source(entry).ok()?;
    let mut seen = 0usize;
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("import ") || trimmed.starts_with("package ") {
            continue;
        }
        if tokens.iter().any(|t| !t.is_empty() && line.contains(t)) {
            if seen == ordinal {
                return Some((idx + 1, trimmed.trim_end().to_string()));
            }
            seen += 1;
        }
    }
    None
}

/// Xref items from reference locations. `title_prefix` is "Caller" for methods
/// and "Usage" for fields/classes (Kotlin `buildXrefItems`).
fn xref_items(
    project: &Project,
    locations: &[dexdec::api::ReferenceLocation],
    tokens: &[&str],
    title_prefix: &str,
) -> Result<Vec<Item>> {
    let mut ordinals: HashMap<(String, String), usize> = HashMap::new();
    let mut items = Vec::with_capacity(locations.len());
    for loc in locations {
        let owner_java = descriptor_to_java(&loc.class);
        let member = loc.method.clone();
        let key = (owner_java.clone(), member.clone());
        let ordinal = *ordinals.entry(key).and_modify(|c| *c += 1).or_insert(0);
        let (line, content) = project
            .lookup(&owner_java)
            .and_then(|entry| usage_line(project, entry, tokens, ordinal))
            .unwrap_or_else(|| {
                (
                    1,
                    format!("// referenced at bytecode offset 0x{:x} in {}", loc.offset, member),
                )
            });
        let mut meta = Map::new();
        meta.insert("owner".into(), json!(owner_java));
        meta.insert("member".into(), json!(member));
        meta.insert("line".into(), json!(line));
        items.push(Item::xref(
            format!("{}#{line}", member),
            format!("{title_prefix}: {member}"),
            content,
            meta,
        ));
    }
    Ok(items)
}

/// Build the `ReferenceTarget` for a resolved method.
fn method_target(entry: &ClassEntry, name: &str, descriptor: &str) -> ReferenceTarget {
    ReferenceTarget::method(entry.descriptor.clone(), name.to_string(), descriptor.to_string())
}

/// Method-IR callee extraction. The dexdec IR text renders invokes as
/// `invoke-virtual Owner.name` / `invoke-direct Foo.<init>` (owner shown with
/// its short name), so callees keep that rendering.
fn extract_callees(ir_text: &str) -> Vec<(String, String, usize, Vec<String>)> {
    let re = match regex::Regex::new(
        r"(?P<insn>\binvoke-(?:virtual|super|static|direct|interface))\s+(?P<owner>[\w.$]+)\.(?P<name>[\w$<>]+)",
    ) {
        Ok(re) => re,
        Err(_) => return vec![],
    };
    let mut order: Vec<String> = Vec::new();
    let mut counts: HashMap<String, (String, usize, Vec<String>)> = HashMap::new();
    for cap in re.captures_iter(ir_text) {
        let owner = cap
            .name("owner")
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        let name = cap
            .name("name")
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        let insn = cap
            .name("insn")
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        if owner.is_empty() || name.is_empty() {
            continue;
        }
        let key = format!("{owner}.{name}");
        let entry = counts
            .entry(key.clone())
            .or_insert_with(|| {
                order.push(key.clone());
                (owner.clone(), 0, Vec::new())
            });
        entry.1 += 1;
        if !entry.2.contains(&insn) {
            entry.2.push(insn);
        }
    }
    order
        .into_iter()
        .filter_map(|key| {
            counts.remove(&key).map(|(owner, count, insns)| {
                let name = key.rsplit('.').next().unwrap_or("").to_string();
                (key.clone(), format!("{owner}#{name}"), count, insns)
            })
        })
        .collect()
}

// ── endpoints: common code analysis ─────────────────────────────────────────

fn get_classes(project: &Project, body: &Value) -> Result<Value> {
    let filter = parse_filter_at(body, "filter");
    let pred = name_filter(&filter)?;
    let mut names: Vec<&ClassEntry> = project
        .entries()
        .iter()
        .filter(|e| pred(&e.java_name))
        .collect();
    names.sort_by(|a, b| a.java_name.cmp(&b.java_name));
    if let Some(limit) = filter.limit {
        names.truncate(limit);
    }
    let items: Vec<Item> = names
        .into_iter()
        .map(|e| Item::symbol(e.java_name.clone(), format!("Class: {}", simple_name(&e.java_name)), e.java_name.clone()))
        .collect();
    Ok(SuccessResponse {
        kind: "classes",
        query: query_from(filter_query(body, "filter")),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_class_context(project: &Project, body: &Value) -> Result<Value> {
    let cls = required_str(body, "cls")?;
    let entry = project
        .lookup_fuzzy(&cls)
        .ok_or_else(|| DecxError::class_not_found(&cls))?;
    let outline = project.class_outline(entry)?;
    let owner = entry.java_name.clone();

    let mut items = Vec::new();
    let mut class_meta = Map::new();
    class_meta.insert("method_count".into(), json!(outline.methods.len()));
    class_meta.insert("field_count".into(), json!(outline.fields.len()));
    items.push(Item::symbol_meta(
        owner.clone(),
        format!("Class: {}", simple_name(&owner)),
        owner.clone(),
        class_meta,
    ));
    for m in &outline.methods {
        let sig = method_signature(&owner, &m.name, &m.descriptor);
        items.push(Item::symbol(sig.clone(), format!("Method: {sig}"), sig));
    }
    for f in &outline.fields {
        let sig = field_signature(&owner, &f.name, &f.descriptor);
        items.push(Item::symbol(sig.clone(), format!("Field: {sig}"), sig));
    }
    Ok(SuccessResponse {
        kind: "class_context",
        query: query_from(vec![("target".into(), json!(cls))]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_class_source(project: &Project, body: &Value) -> Result<Value> {
    let cls = required_str(body, "cls")?;
    let smali = bool_of(body, "smali");
    let entry = project
        .lookup_fuzzy(&cls)
        .ok_or_else(|| DecxError::class_not_found(&cls))?;
    let limit = body
        .get("filter")
        .and_then(|f| f.get("limit"))
        .and_then(Value::as_u64)
        .map(|v| v as usize);

    let (source, language, fallback) = if smali {
        (project.class_ir_listing(entry)?, "smali", false)
    } else {
        match project.class_source(entry) {
            Ok(s) if !s.trim().is_empty() => ((*s).clone(), "java", false),
            _ => (project.class_ir_listing(entry)?, "smali", true),
        }
    };
    if source.trim().is_empty() {
        return Err(DecxError::decompilation_skipped(format!(
            "decompiled to empty source: {cls}"
        )));
    }
    let total_lines = source.lines().count();
    let content = match limit {
        Some(limit) => source.lines().take(limit).collect::<Vec<_>>().join("\n"),
        None => source.clone(),
    };
    let returned_lines = content.lines().count();
    let mut meta = Map::new();
    meta.insert("language".into(), json!(language));
    meta.insert("total_lines".into(), json!(total_lines));
    meta.insert("returned_lines".into(), json!(returned_lines));
    if fallback {
        meta.insert("smali_fallback".into(), json!(true));
    }
    let items = vec![Item::code(entry.java_name.clone(), entry.java_name.clone(), content, meta)];
    Ok(SuccessResponse {
        kind: "class_source",
        query: query_from(
            vec![("target".into(), json!(cls)), ("smali".into(), json!(smali))]
                .into_iter()
                .chain(filter_query(body, "filter"))
                .collect(),
        ),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn search_global_key(project: &Project, body: &Value) -> Result<Value> {
    let key = required_str(body, "key")?;
    if key.trim().is_empty() {
        return Err(DecxError::empty_search_key());
    }
    let search = body.get("search");
    let case_sensitive = search
        .and_then(|s| s.get("caseSensitive"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let use_regex = search
        .and_then(|s| s.get("regex"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let limit = search
        .and_then(|s| s.get("limit"))
        .and_then(Value::as_u64)
        .map(|v| v as usize);
    let matcher = compile_matcher(key.clone(), case_sensitive, use_regex)?;
    let filter = parse_filter_at(body, "filter");
    let pred = name_filter(&filter)?;

    let matching: Vec<usize> = project
        .entries()
        .iter()
        .enumerate()
        .filter(|(_, e)| pred(&e.java_name))
        .map(|(i, _)| i)
        .collect();
    let (_ok, _failures) = project.decompile_batch(&matching);

    let mut items: Vec<Item> = Vec::new();
    let mut skipped_decompile = 0usize;
    for &i in &matching {
        let e = &project.entries()[i];
        let name_match = matcher(&e.java_name);
        let source = project.class_source(e).ok();
        let source_match = source
            .as_deref()
            .map(|s| !s.trim().is_empty() && matcher(s))
            .unwrap_or(false);
        if name_match || source_match {
            items.push(Item::symbol(
                e.java_name.clone(),
                format!("Class match: {}", simple_name(&e.java_name)),
                e.java_name.clone(),
            ));
            if let Some(limit) = limit {
                if items.len() >= limit {
                    break;
                }
            }
        } else {
            skipped_decompile += 1;
        }
    }
    Ok(SuccessResponse {
        kind: "search_global",
        query: query_from(
            vec![("target".into(), json!(key))]
                .into_iter()
                .chain(filter_query(body, "search"))
                .chain(filter_query(body, "filter"))
                .collect(),
        ),
        items,
        summary_extra: vec![("skipped_decompile_count".into(), json!(skipped_decompile))],
        page: page_of(body),
    }
    .paginate())
}

/// Kotlin matcher semantics: regex → `containsMatchIn`, literal → `contains`
/// with case folding unless case-sensitive. Boxed because the branches produce
/// different concrete closure types; owns the key so callers can keep using it.
fn compile_matcher(key: String, case_sensitive: bool, use_regex: bool) -> Result<Box<dyn Fn(&str) -> bool>> {
    if use_regex {
        let src = if case_sensitive {
            key.clone()
        } else {
            format!("(?i){key}")
        };
        let re = regex::Regex::new(&src)
            .map_err(|e| DecxError::invalid_parameter(format!("bad pattern {key:?}: {e}")))?;
        Ok(Box::new(move |s: &str| re.is_match(s)))
    } else if case_sensitive {
        Ok(Box::new(move |s: &str| s.contains(key.as_str())))
    } else {
        let k = key.to_lowercase();
        Ok(Box::new(move |s: &str| s.to_lowercase().contains(k.as_str())))
    }
}

fn search_class_key(project: &Project, body: &Value) -> Result<Value> {
    let cls = required_str(body, "cls")?;
    let key = required_str(body, "key")?;
    if key.trim().is_empty() {
        return Err(DecxError::empty_search_key());
    }
    let entry = project
        .lookup_fuzzy(&cls)
        .ok_or_else(|| DecxError::class_not_found(&cls))?;
    let grep = body.get("grep");
    let limit = grep
        .and_then(|g| g.get("limit"))
        .and_then(Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(0);
    let case_sensitive = grep
        .and_then(|g| g.get("caseSensitive"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let use_regex = grep
        .and_then(|g| g.get("regex"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let matcher = compile_matcher(key.clone(), case_sensitive, use_regex)?;

    let outline = project.class_outline(entry)?;
    let owner = entry.java_name.clone();
    let mut items: Vec<Item> = Vec::new();
    if limit == 0 {
        return Ok(SuccessResponse {
            kind: "search_class",
            query: query_from(
                vec![("target".into(), json!(key)), ("class".into(), json!(cls))]
                    .into_iter()
                    .chain(filter_query(body, "grep"))
                    .collect(),
            ),
            items,
            summary_extra: vec![],
            page: page_of(body),
        }
        .paginate());
    }
    for m in &outline.methods {
        let src = match project.method_source(entry, &m.name) {
            Ok(s) => s,
            Err(_) => continue,
        };
        for (idx, line) in src.lines().enumerate() {
            if matcher(line) {
                let sig = method_signature(&owner, &m.name, &m.descriptor);
                let mut meta = Map::new();
                meta.insert("line".into(), json!(idx + 1));
                items.push(Item::code(
                    format!("{sig}#{}", idx + 1),
                    sig.clone(),
                    line.trim().to_string(),
                    meta,
                ));
                if items.len() >= limit {
                    return Ok(finish_search_class(body, cls, key, items));
                }
            }
        }
    }
    Ok(finish_search_class(body, cls, key, items))
}

fn finish_search_class(body: &Value, cls: String, key: String, items: Vec<Item>) -> Value {
    SuccessResponse {
        kind: "search_class",
        query: query_from(
            vec![("target".into(), json!(key)), ("class".into(), json!(cls))]
                .into_iter()
                .chain(filter_query(body, "grep"))
                .collect(),
        ),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate()
}

fn search_method(project: &Project, body: &Value) -> Result<Value> {
    let mth = required_str(body, "mth")?;
    let lower = mth.to_lowercase();
    let mut items: Vec<Item> = Vec::new();
    for member in project.members()? {
        if member.kind != MemberKind::Method {
            continue;
        }
        if !member.name.to_lowercase().contains(&lower) {
            continue;
        }
        let owner_java = descriptor_to_java(&member.owner);
        let sig = method_signature(&owner_java, &member.name, &member.descriptor);
        items.push(Item::symbol(sig.clone(), format!("Method: {sig}"), sig));
    }
    Ok(SuccessResponse {
        kind: "search_method",
        query: query_from(vec![("target".into(), json!(mth))]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

// ── endpoints: method context ───────────────────────────────────────────────

fn get_method_source(project: &Project, body: &Value) -> Result<Value> {
    let mth = required_str(body, "mth")?;
    let smali = bool_of(body, "smali");
    let (entry, name) = resolve_method(project, &mth)?;
    let outline = project.class_outline(entry)?;
    let desc = outline
        .methods
        .iter()
        .find(|m| m.name == name)
        .map(|m| m.descriptor.clone())
        .unwrap_or_else(|| "()V".to_string());
    let sig = method_signature(&entry.java_name, &name, &desc);
    let source = if smali {
        project.method_ir_text(entry, &name)?
    } else {
        project.method_source(entry, &name)?
    };
    if source.trim().is_empty() {
        return Err(DecxError::decompilation_skipped(format!(
            "decompiled to empty source: {mth}"
        )));
    }
    let mut meta = Map::new();
    meta.insert("language".into(), json!(if smali { "smali" } else { "java" }));
    let items = vec![Item::code(sig.clone(), sig, source, meta)];
    Ok(SuccessResponse {
        kind: "method_source",
        query: query_from(vec![("target".into(), json!(mth)), ("smali".into(), json!(smali))]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_method_context(project: &Project, body: &Value) -> Result<Value> {
    let mth = required_str(body, "mth")?;
    let (entry, name) = resolve_method(project, &mth)?;
    let outline = project.class_outline(entry)?;
    let outline_method = outline
        .methods
        .iter()
        .find(|m| m.name == name)
        .ok_or_else(|| DecxError::method_not_found(&mth))?;
    let desc = outline_method.descriptor.clone();
    let (params, ret) = split_method_descriptor(&desc);
    let sig = method_signature(&entry.java_name, &name, &desc);

    let locations = project.references(method_target(entry, &name, &desc))?;
    let caller_items = xref_items(project, &locations, &[&name], "Caller")?;

    let ir_text = project.method_ir_text(entry, &name).unwrap_or_default();
    let callees = extract_callees(&ir_text);

    let mut items = Vec::new();
    let mut sig_meta = Map::new();
    sig_meta.insert("owner".into(), json!(entry.java_name));
    sig_meta.insert(
        "return_type".into(),
        json!(descriptor_to_java(&ret)),
    );
    sig_meta.insert("argument_count".into(), json!(params.len()));
    sig_meta.insert("caller_count".into(), json!(locations.len()));
    sig_meta.insert("callee_count".into(), json!(callees.len()));
    items.push(Item::symbol_meta(
        sig.clone(),
        format!("Method signature: {sig}"),
        sig.clone(),
        sig_meta,
    ));
    items.extend(caller_items);
    for (i, (key, title, call_count, invoke_types)) in callees.into_iter().enumerate() {
        let owner = key
            .rsplit_once('.')
            .map(|(o, _)| o.to_string())
            .unwrap_or_else(|| key.clone());
        let mut meta = Map::new();
        meta.insert("owner".into(), json!(owner));
        meta.insert("call_count".into(), json!(call_count));
        meta.insert("invoke_types".into(), json!(invoke_types));
        items.push(Item::symbol_meta(
            format!("{sig}#callee-{i}"),
            format!("Callee: {title}"),
            title,
            meta,
        ));
    }
    Ok(SuccessResponse {
        kind: "method_context",
        query: query_from(vec![("target".into(), json!(mth))]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_method_cfg(project: &Project, body: &Value) -> Result<Value> {
    let mth = required_str(body, "mth")?;
    let (entry, name) = resolve_method(project, &mth)?;
    let outline = project.class_outline(entry)?;
    let desc = outline
        .methods
        .iter()
        .find(|m| m.name == name)
        .map(|m| m.descriptor.clone())
        .unwrap_or_else(|| "()V".to_string());
    let sig = method_signature(&entry.java_name, &name, &desc);
    let (_nodes, _edges, text) = project.method_cfg(entry, &name)?;
    let mut meta = Map::new();
    meta.insert("language".into(), json!("dot"));
    let items = vec![Item::code(
        format!("{sig}#cfg-dot"),
        format!("CFG DOT: {sig}"),
        text,
        meta,
    )];
    Ok(SuccessResponse {
        kind: "method_cfg",
        query: query_from(vec![("target".into(), json!(mth))]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_method_xref(project: &Project, body: &Value) -> Result<Value> {
    let mth = required_str(body, "mth")?;
    let (entry, name) = resolve_method(project, &mth)?;
    let outline = project.class_outline(entry)?;
    let desc = outline
        .methods
        .iter()
        .find(|m| m.name == name)
        .map(|m| m.descriptor.clone())
        .ok_or_else(|| DecxError::method_not_found(&mth))?;
    let locations = project.references(method_target(entry, &name, &desc))?;
    let items = xref_items(project, &locations, &[&name], "Caller")?;
    Ok(SuccessResponse {
        kind: "method_xref",
        query: query_from(vec![("target".into(), json!(mth))]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_field_xref(project: &Project, body: &Value) -> Result<Value> {
    let fld = required_str(body, "fld")?;
    let (entry, name) = resolve_field(project, &fld)?;
    let outline = project.class_outline(entry)?;
    let field_desc = outline
        .fields
        .iter()
        .find(|f| f.name == name)
        .map(|f| f.descriptor.clone());
    let target = match field_desc {
        Some(d) => ReferenceTarget::field(&entry.descriptor, &name, d),
        None => ReferenceTarget::field_name(&entry.descriptor, &name),
    };
    let locations = project.references(target)?;
    let items = xref_items(project, &locations, &[&name], "Usage")?;
    Ok(SuccessResponse {
        kind: "field_xref",
        query: query_from(vec![("target".into(), json!(fld))]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_class_xref(project: &Project, body: &Value) -> Result<Value> {
    let cls = required_str(body, "cls")?;
    let entry = project
        .lookup_fuzzy(&cls)
        .ok_or_else(|| DecxError::class_not_found(&cls))?;
    let locations = project
        .references(ReferenceTarget::class(entry.descriptor.clone()))?;
    let tokens = [entry.java_name.as_str(), simple_name(&entry.java_name)];
    let items = xref_items(project, &locations, &tokens, "Usage")?;
    Ok(SuccessResponse {
        kind: "class_xref",
        query: query_from(vec![("target".into(), json!(cls))]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_implementations(project: &Project, body: &Value) -> Result<Value> {
    let iface = required_str(body, "iface")?;
    let entry = project
        .lookup_fuzzy(&iface)
        .ok_or_else(|| DecxError::interface_not_found(&iface))?;
    let hierarchy = project.hierarchy()?;
    let iface_java = entry.java_name.clone();
    let mut impls: Vec<&String> = hierarchy
        .iter()
        .filter(|(_, (_, ifaces))| ifaces.iter().any(|i| i == &iface_java))
        .map(|(name, _)| name)
        .collect();
    impls.sort();
    let items: Vec<Item> = impls
        .into_iter()
        .map(|name| {
            let mut meta = Map::new();
            meta.insert("interface".into(), json!(iface_java));
            Item::symbol_meta(
                name.clone(),
                format!("Implementation: {}", simple_name(name)),
                format!("{name} implements {iface_java}"),
                meta,
            )
        })
        .collect();
    Ok(SuccessResponse {
        kind: "implementations",
        query: query_from(vec![("target".into(), json!(iface))]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_subclasses(project: &Project, body: &Value) -> Result<Value> {
    let cls = required_str(body, "cls")?;
    let entry = project
        .lookup_fuzzy(&cls)
        .ok_or_else(|| DecxError::class_not_found(&cls))?;
    let hierarchy = project.hierarchy()?;
    let base = entry.java_name.clone();
    let mut subs: Vec<&String> = hierarchy
        .iter()
        .filter(|(_, (sup, _))| sup.as_deref() == Some(base.as_str()))
        .map(|(name, _)| name)
        .collect();
    subs.sort();
    let items: Vec<Item> = subs
        .into_iter()
        .map(|name| {
            let mut meta = Map::new();
            meta.insert("superclass".into(), json!(base));
            Item::symbol_meta(
                name.clone(),
                format!("Subclass: {}", simple_name(name)),
                format!("{name} extends {base}"),
                meta,
            )
        })
        .collect();
    Ok(SuccessResponse {
        kind: "subclasses",
        query: query_from(vec![("target".into(), json!(cls))]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

// ── endpoints: android app analysis ─────────────────────────────────────────

fn get_aidl_interfaces(project: &Project, body: &Value) -> Result<Value> {
    let filter = parse_filter_at(body, "filter");
    let pred = name_filter(&filter)?;
    let hierarchy = project.hierarchy()?;

    let mut items: Vec<Item> = Vec::new();
    for entry in project.entries() {
        if !entry.java_name.ends_with(".Stub") {
            continue;
        }
        let iface_name = match entry.java_name.strip_suffix(".Stub") {
            Some(name) if !name.is_empty() => name.to_string(),
            _ => continue,
        };
        if !pred(&iface_name) {
            continue;
        }
        let mut impls: Vec<&String> = hierarchy
            .iter()
            .filter(|(name, (sup, _))| {
                sup.as_deref() == Some(entry.java_name.as_str())
                    && !name.ends_with(".Proxy")
                    && name.as_str() != entry.java_name.as_str()
            })
            .map(|(name, _)| name)
            .collect();
        impls.sort();
        let impls: Vec<String> = impls.into_iter().cloned().collect();
        let mut meta = Map::new();
        meta.insert("stub".into(), json!(entry.java_name));
        meta.insert("implementations".into(), json!(impls));
        items.push(Item::symbol_bare(
            iface_name.clone(),
            format!("AIDL: {}", simple_name(&iface_name)),
            meta,
        ));
        if let Some(limit) = filter.limit {
            if items.len() >= limit {
                break;
            }
        }
    }
    Ok(SuccessResponse {
        kind: "aidl_interfaces",
        query: query_from(filter_query(body, "filter")),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_app_manifest(project: &Project, body: &Value) -> Result<Value> {
    let res = project.resources().ok_or_else(DecxError::manifest_not_found)?;
    let text = res
        .manifest_text()
        .ok_or_else(DecxError::manifest_not_found)?
        .to_string();
    let mut meta = Map::new();
    meta.insert("language".into(), json!("xml"));
    let items = vec![Item::code(
        "AndroidManifest.xml",
        "AndroidManifest.xml",
        text,
        meta,
    )];
    Ok(SuccessResponse {
        kind: "app_manifest",
        query: query_from(vec![]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_main_activity(project: &Project, body: &Value) -> Result<Value> {
    let res = project.resources().ok_or_else(DecxError::manifest_not_found)?;
    let name = res
        .main_activity()
        .ok_or_else(DecxError::no_main_activity)?;
    let entry = project
        .lookup(&name)
        .ok_or_else(|| DecxError::class_not_found(&name))?;
    let items = vec![Item::symbol(
        entry.java_name.clone(),
        format!("Main activity: {}", simple_name(&entry.java_name)),
        entry.java_name.clone(),
    )];
    Ok(SuccessResponse {
        kind: "main_activity",
        query: query_from(vec![]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_application(project: &Project, body: &Value) -> Result<Value> {
    let res = project.resources().ok_or_else(DecxError::manifest_not_found)?;
    let name = res.application().ok_or_else(DecxError::no_application)?;
    let items = vec![Item::symbol(
        name.clone(),
        format!("Application: {}", simple_name(&name)),
        name,
    )];
    Ok(SuccessResponse {
        kind: "application",
        query: query_from(vec![]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_exported_components(project: &Project, body: &Value) -> Result<Value> {
    let res = project.resources().ok_or_else(DecxError::manifest_not_found)?;
    let filter = parse_top_level_filter(body);
    let pred = name_filter(&filter)?;

    let mut items: Vec<Item> = Vec::new();
    for (name, tag, meta_map) in res.exported_components() {
        if !pred(&tag) {
            continue;
        }
        items.push(Item::symbol_meta(
            name.clone(),
            format!("Exported {tag}: {name}"),
            name,
            meta_map,
        ));
        if let Some(limit) = filter.limit {
            if items.len() >= limit {
                break;
            }
        }
    }
    Ok(SuccessResponse {
        kind: "exported_components",
        query: query_from(top_level_filter_query(body)),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_deep_links(project: &Project, body: &Value) -> Result<Value> {
    let res = project.resources().ok_or_else(DecxError::manifest_not_found)?;
    let items: Vec<Item> = res
        .deep_links()
        .into_iter()
        .map(|(component, uri, meta_map)| {
            Item::symbol_meta(
                format!("{component}#{uri}"),
                format!("Deep link: {uri}"),
                uri,
                meta_map,
            )
        })
        .collect();
    Ok(SuccessResponse {
        kind: "deep_links",
        query: query_from(vec![]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_dynamic_receivers(project: &Project, body: &Value) -> Result<Value> {
    let filter = parse_filter_at(body, "filter");
    let pred = name_filter(&filter)?;

    // Decompile candidates through the parallel batch pipeline first so the
    // per-class source cache is warm; `class_source` then hits the LRU cache.
    let candidates: Vec<usize> = project
        .entries()
        .iter()
        .enumerate()
        .filter(|(_, e)| pred(&e.java_name))
        .map(|(i, _)| i)
        .collect();
    let _ = project.decompile_batch(&candidates);

    let mut items: Vec<Item> = Vec::new();
    for &i in &candidates {
        let entry = &project.entries()[i];
        // Cheap owner-level pre-filter before touching per-method sources.
        let class_src = match project.class_source(entry) {
            Ok(s) if s.contains("registerReceiver") => s,
            _ => continue,
        };
        let _ = class_src;
        let Ok(outline) = project.class_outline(entry) else {
            continue;
        };
        for m in &outline.methods {
            let Ok(src) = project.method_source(entry, &m.name) else {
                continue;
            };
            if !src.contains("registerReceiver") {
                continue;
            }
            let mut meta = Map::new();
            meta.insert("class".into(), json!(entry.java_name));
            meta.insert("method".into(), json!(m.name));
            meta.insert("total_lines".into(), json!(src.lines().count()));
            items.push(Item::code(
                format!("{}#{}", entry.java_name, m.name),
                format!("Dynamic receiver: {}", m.name),
                src,
                meta,
            ));
            if let Some(limit) = filter.limit {
                if items.len() >= limit {
                    break;
                }
            }
        }
        if let Some(limit) = filter.limit {
            if items.len() >= limit {
                break;
            }
        }
    }
    Ok(SuccessResponse {
        kind: "dynamic_receivers",
        query: query_from(filter_query(body, "filter")),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_all_resources(project: &Project, body: &Value) -> Result<Value> {
    let res = project.resources().ok_or_else(|| DecxError::resource_not_found("<none>"))?;
    let filter = parse_filter_at(body, "filter");
    let pred = name_filter(&filter)?;
    let mut names: Vec<String> = res
        .resource_file_names()
        .into_iter()
        .filter(|n| pred(n))
        .collect();
    names.sort();
    if names.is_empty() {
        return Err(DecxError::resource_not_found("<none>"));
    }
    if let Some(limit) = filter.limit {
        names.truncate(limit);
    }
    let items: Vec<Item> = names
        .into_iter()
        .map(|name| {
            let simple = name.rsplit('/').next().unwrap_or(&name).to_string();
            Item::symbol(name.clone(), format!("Resource: {simple}"), name)
        })
        .collect();
    Ok(SuccessResponse {
        kind: "all_resources",
        query: query_from(filter_query(body, "filter")),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_resource_file(project: &Project, body: &Value) -> Result<Value> {
    let target = required_str(body, "res")?;
    let res = project.resources().ok_or_else(|| DecxError::resource_not_found(&target))?;
    let content = res
        .read_resource_file(&target)
        .ok_or_else(|| DecxError::resource_not_found(&target))?;
    let items = vec![Item::code_bare(target.clone(), target.clone(), content)];
    Ok(SuccessResponse {
        kind: "resource_file",
        query: query_from(vec![("target".into(), json!(target))]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_strings(project: &Project, body: &Value) -> Result<Value> {
    let res = project.resources().ok_or_else(|| DecxError::resource_not_found("<none>"))?;
    let strings = res.strings();
    if strings.is_empty() {
        return Err(DecxError::no_strings_found());
    }
    let file = "res/values/strings.xml";
    let items: Vec<Item> = strings
        .into_iter()
        .map(|(name, value)| {
            let mut meta = Map::new();
            meta.insert("file".into(), json!(file));
            Item::symbol_meta(
                format!("{file}#{name}"),
                format!("String: {name}"),
                value,
                meta,
            )
        })
        .collect();
    Ok(SuccessResponse {
        kind: "strings",
        query: query_from(vec![]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}

fn get_system_service_impl(project: &Project, body: &Value) -> Result<Value> {
    let iface = required_str(body, "iface")?;
    let entry = project
        .lookup_fuzzy(&iface)
        .ok_or_else(|| DecxError::interface_not_found(&iface))?;
    let stub_java = format!("{}$Stub", entry.java_name);
    let hierarchy = project.hierarchy()?;
    let mut impls: Vec<&String> = hierarchy
        .iter()
        .filter(|(_, (sup, _))| sup.as_deref() == Some(stub_java.as_str()))
        .map(|(name, _)| name)
        .collect();
    impls.sort();
    let impl_name = impls
        .first()
        .cloned()
        .cloned()
        .ok_or_else(|| DecxError::service_impl_not_found(&iface))?;
    let impl_entry = project
        .lookup(&impl_name)
        .ok_or_else(|| DecxError::class_not_found(&impl_name))?;
    let outline = project.class_outline(impl_entry)?;

    let mut items = Vec::new();
    let mut service_meta = Map::new();
    service_meta.insert("interface".into(), json!(entry.java_name));
    service_meta.insert("method_count".into(), json!(outline.methods.len()));
    service_meta.insert("field_count".into(), json!(outline.fields.len()));
    items.push(Item::symbol_meta(
        impl_name.clone(),
        format!("Service implementation: {}", simple_name(&impl_name)),
        format!("{impl_name} implements {}", entry.java_name),
        service_meta,
    ));
    for m in &outline.methods {
        let sig = method_signature(&impl_name, &m.name, &m.descriptor);
        items.push(Item::symbol(sig.clone(), format!("Method: {sig}"), sig));
    }
    for f in &outline.fields {
        let sig = field_signature(&impl_name, &f.name, &f.descriptor);
        items.push(Item::symbol(sig.clone(), format!("Field: {sig}"), sig));
    }
    Ok(SuccessResponse {
        kind: "system_service_impl",
        query: query_from(vec![("target".into(), json!(iface))]),
        items,
        summary_extra: vec![],
        page: page_of(body),
    }
    .paginate())
}
