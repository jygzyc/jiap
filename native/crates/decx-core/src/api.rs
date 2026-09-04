//! Endpoint dispatcher mirroring `DecxRoutes`/`RouteHandler` from the Kotlin
//! core. The server stays thin: route `/api/decx/<endpoint>` → [`dispatch`].
//!
//! Every capability below is served by the fused dexdec engine through
//! [`crate::project::Project`]; no second parser or decompiler exists.

use serde_json::{json, Value};

use dexdec::api::ReferenceTarget;

use crate::error::{DecxError, Result};
use crate::project::Project;

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
        other => Err(DecxError::new(
            "UNKNOWN_ENDPOINT",
            format!("unknown endpoint: {other}"),
        )),
    }
}

// ── request helpers ─────────────────────────────────────────────────────────

struct ListFilter {
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
        includes,
        excludes,
        regex: f.and_then(|f| f.get("regex")).and_then(Value::as_bool).unwrap_or(true),
    }
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
    regex::Regex::new(&src)
        .map_err(|e| DecxError::invalid_parameter(format!("bad pattern {pattern:?}: {e}")))
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
/// "method") to (class entry, method name).
fn resolve_method<'a>(
    project: &'a Project,
    mth: &str,
) -> Result<(&'a crate::project::ClassEntry, String)> {
    let spec = mth.trim();
    let (cls_part, mth_part) = if let Some((c, m)) = spec.split_once('#') {
        (Some(c.to_string()), m.to_string())
    } else if let Some((c, m)) = spec.rsplit_once('.') {
        // the dot prefix only counts as a class if it indexes to one
        (Some(c.to_string()), m.to_string())
    } else {
        (None, spec.to_string())
    };

    if let Some(c) = cls_part {
        let entry = project.lookup_fuzzy(&c).ok_or_else(|| DecxError::class_not_found(c))?;
        let outline = project.class_outline(entry)?;
        if outline.methods.iter().any(|m| m.name == mth_part) {
            return Ok((entry, mth_part));
        }
        return Err(DecxError::method_not_found(mth));
    }
    for entry in project.entries() {
        let Ok(outline) = project.class_outline(entry) else {
            continue;
        };
        if outline.methods.iter().any(|m| m.name == mth_part) {
            return Ok((entry, mth_part));
        }
    }
    Err(DecxError::method_not_found(mth))
}

/// Resolve a `fld` spec to (class entry, field name).
fn resolve_field<'a>(
    project: &'a Project,
    fld: &str,
) -> Result<(&'a crate::project::ClassEntry, String)> {
    let spec = fld.trim();
    let (cls_part, fld_part) = if let Some((c, f)) = spec.split_once('#') {
        (Some(c.to_string()), f.to_string())
    } else if let Some((c, f)) = spec.rsplit_once('.') {
        (Some(c.to_string()), f.to_string())
    } else {
        (None, spec.to_string())
    };

    if let Some(c) = cls_part {
        let entry = project.lookup_fuzzy(&c).ok_or_else(|| DecxError::class_not_found(c))?;
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

// ── endpoints ───────────────────────────────────────────────────────────────

fn get_classes(project: &Project, body: &Value) -> Result<Value> {
    let filter = parse_filter(body);
    let pred = name_filter(&filter)?;
    let page = page_of(body);
    let mut names: Vec<&crate::project::ClassEntry> =
        project.entries().iter().filter(|e| pred(&e.java_name)).collect();
    names.sort_by(|a, b| a.java_name.cmp(&b.java_name));
    let total = names.len();
    let limit = body
        .get("filter")
        .and_then(|f| f.get("limit"))
        .and_then(Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(200)
        .min(2000);
    let start = ((page - 1) * limit as u64) as usize;
    let page_items: Vec<Value> = names
        .into_iter()
        .skip(start)
        .take(limit)
        .map(|e| {
            json!({
                "name": e.java_name,
                "descriptor": e.descriptor,
                "package": e.package,
                "nested": e.nested,
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
    let entry = project
        .lookup_fuzzy(&cls)
        .ok_or_else(|| DecxError::class_not_found(cls))?;
    let smali = body.get("smali").and_then(Value::as_bool).unwrap_or(false);
    let limit = body
        .get("filter")
        .and_then(|f| f.get("limit"))
        .and_then(Value::as_u64)
        .map(|v| v as usize);

    let source = if smali {
        project.class_ir_listing(entry)?
    } else {
        (*project.class_source(entry)?).clone()
    };
    let source = truncate_lines(source, limit);
    Ok(json!({ "cls": entry.java_name, "source": source }))
}

fn truncate_lines(mut source: String, limit: Option<usize>) -> String {
    if let Some(limit) = limit {
        let line_count = source.lines().count();
        let kept: Vec<String> = source.lines().take(limit).map(str::to_string).collect();
        if line_count > limit {
            let mut kept = kept;
            kept.push(format!("// ... truncated at {limit} lines (filter.limit)"));
            source = kept.join("\n");
        } else {
            source = kept.join("\n");
        }
    }
    source
}

fn get_class_context(project: &Project, body: &Value) -> Result<Value> {
    let cls = required_str(body, "cls")?;
    let entry = project
        .lookup_fuzzy(&cls)
        .ok_or_else(|| DecxError::class_not_found(cls))?;
    let outline = project.class_outline(entry)?;
    let source = project.class_source(entry)?;
    Ok(json!({
        "cls": entry.java_name,
        "descriptor": entry.descriptor,
        "kind": format!("{:?}", outline.kind),
        "superclass": outline.super_class.as_deref().map(crate::names::descriptor_to_java),
        "interfaces": outline.interfaces.iter().map(|i| crate::names::descriptor_to_java(i)).collect::<Vec<_>>(),
        "source": &*source,
        "methods": outline.methods.iter().map(|m| json!({
            "name": m.name,
            "signature": m.display_signature,
            "descriptor": m.descriptor,
            "accessFlags": m.access_flags,
            "hasCode": m.has_code,
        })).collect::<Vec<_>>(),
        "fields": outline.fields.iter().map(|f| json!({
            "name": f.name,
            "type": f.display_type,
            "descriptor": f.descriptor,
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
    let use_regex = search
        .and_then(|s| s.get("regex"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let matcher = compile_pattern(
        &if case_sensitive || !use_regex {
            key.clone()
        } else {
            format!("(?i){key}")
        },
        use_regex,
    )?;
    let filter = parse_filter(body);
    let pred = name_filter(&filter)?;

    // Decompile all matching classes through the dexdec batch pipeline, then
    // grep the cached sources.
    let matching: Vec<usize> = project
        .entries()
        .iter()
        .enumerate()
        .filter(|(_, e)| pred(&e.java_name))
        .map(|(i, _)| i)
        .collect();
    let (_ok, _failures) = project.decompile_batch(&matching);

    let page = page_of(body);
    let skip = ((page - 1) * limit as u64) as usize;
    let mut hits: Vec<Value> = Vec::new();
    for &i in &matching {
        let e = &project.entries()[i];
        let Ok(source) = project.class_source(e) else {
            continue;
        };
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

fn search_class_key(project: &Project, body: &Value) -> Result<Value> {
    let cls = required_str(body, "cls")?;
    let key = required_str(body, "key")?;
    let entry = project
        .lookup_fuzzy(&cls)
        .ok_or_else(|| DecxError::class_not_found(cls))?;
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
    let use_regex = grep
        .and_then(|g| g.get("regex"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
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
    for member in project.members()? {
        if member.kind != dexdec::api::MemberKind::Method {
            continue;
        }
        if !member.name.contains(&name_part) {
            continue;
        }
        let owner_java = crate::names::descriptor_to_java(&member.owner);
        if let Some(cf) = &cls_filter {
            if &owner_java != cf {
                continue;
            }
        }
        hits.push(json!({
            "cls": owner_java,
            "name": member.name,
            "descriptor": member.descriptor,
            "hasCode": member.has_code,
        }));
        if hits.len() >= limit {
            break;
        }
    }
    Ok(json!({ "mth": mth, "page": page, "total": hits.len(), "methods": hits }))
}

fn get_method_source(project: &Project, body: &Value) -> Result<Value> {
    let mth = required_str(body, "mth")?;
    let (entry, method_name) = resolve_method(project, &mth)?;
    let smali = body.get("smali").and_then(Value::as_bool).unwrap_or(false);
    let source = if smali {
        project.method_ir_text(entry, &method_name)?
    } else {
        project.method_source(entry, &method_name)?
    };
    Ok(json!({ "mth": mth, "cls": entry.java_name, "source": source }))
}

fn get_method_context(project: &Project, body: &Value) -> Result<Value> {
    let mth = required_str(body, "mth")?;
    let (entry, method_name) = resolve_method(project, &mth)?;
    let source = project.method_source(entry, &method_name)?;
    let (nodes, edges, ir_text) = project.method_cfg(entry, &method_name)?;
    Ok(json!({
        "mth": mth,
        "cls": entry.java_name,
        "source": source,
        "cfg": { "nodes": nodes, "edges": edges },
        "ir": ir_text,
    }))
}

fn get_method_cfg(project: &Project, body: &Value) -> Result<Value> {
    let mth = required_str(body, "mth")?;
    let (entry, method_name) = resolve_method(project, &mth)?;
    let (nodes, edges, text) = project.method_cfg(entry, &method_name)?;
    Ok(json!({
        "mth": mth,
        "cls": entry.java_name,
        "cfg": { "nodes": nodes, "edges": edges },
        "ir": text,
    }))
}

fn get_method_xref(project: &Project, body: &Value) -> Result<Value> {
    let mth = required_str(body, "mth")?;
    let (entry, method_name) = resolve_method(project, &mth)?;
    let outline = project.class_outline(entry)?;
    let outline_method = outline
        .methods
        .iter()
        .find(|m| m.name == method_name)
        .ok_or_else(|| DecxError::method_not_found(&mth))?;
    let (params, _ret) = crate::names::split_method_descriptor(&outline_method.descriptor);
    let target = if params.is_empty() {
        ReferenceTarget::method_arity(&entry.descriptor, &method_name, 0)
    } else {
        ReferenceTarget::method_parameters(&entry.descriptor, &method_name, params)
    };
    let locations = project.references(target)?;
    let refs: Vec<Value> = locations
        .iter()
        .map(|l| {
            json!({
                "cls": crate::names::descriptor_to_java(&l.class),
                "method": l.method,
                "offset": l.offset,
            })
        })
        .collect();
    Ok(json!({
        "mth": mth,
        "cls": entry.java_name,
        "total": refs.len(),
        "callers": refs,
    }))
}

fn get_field_xref(project: &Project, body: &Value) -> Result<Value> {
    let fld = required_str(body, "fld")?;
    let (entry, field_name) = resolve_field(project, &fld)?;
    let outline = project.class_outline(entry)?;
    let field_desc = outline
        .fields
        .iter()
        .find(|f| f.name == field_name)
        .map(|f| f.descriptor.clone());
    let target = match field_desc {
        Some(d) => ReferenceTarget::field(&entry.descriptor, &field_name, d),
        None => ReferenceTarget::field_name(&entry.descriptor, &field_name),
    };
    let locations = project.references(target)?;
    let refs: Vec<Value> = locations
        .iter()
        .map(|l| {
            json!({
                "cls": crate::names::descriptor_to_java(&l.class),
                "method": l.method,
                "offset": l.offset,
            })
        })
        .collect();
    Ok(json!({
        "fld": fld,
        "cls": entry.java_name,
        "total": refs.len(),
        "references": refs,
    }))
}

fn get_class_xref(project: &Project, body: &Value) -> Result<Value> {
    let cls = required_str(body, "cls")?;
    let entry = project
        .lookup_fuzzy(&cls)
        .ok_or_else(|| DecxError::class_not_found(cls))?;
    let locations = project.references(ReferenceTarget::class(entry.descriptor.clone()))?;
    let refs: Vec<Value> = locations
        .iter()
        .map(|l| {
            json!({
                "cls": crate::names::descriptor_to_java(&l.class),
                "method": l.method,
                "offset": l.offset,
            })
        })
        .collect();
    Ok(json!({
        "cls": entry.java_name,
        "total": refs.len(),
        "referencedBy": refs,
    }))
}

fn get_implementations(project: &Project, body: &Value) -> Result<Value> {
    let iface = required_str(body, "iface")?;
    let entry = project
        .lookup_fuzzy(&iface)
        .ok_or_else(|| DecxError::class_not_found(iface))?;
    let hierarchy = project.hierarchy()?;
    let iface_java = entry.java_name.as_str();
    let direct: Vec<String> = hierarchy
        .iter()
        .filter(|(_, (_, ifaces))| ifaces.iter().any(|i| i == iface_java))
        .map(|(name, _)| name.clone())
        .collect();
    let mut indirect: Vec<String> = Vec::new();
    for (name, (sup, _)) in hierarchy.iter() {
        if let Some(sup) = sup {
            if direct.iter().any(|d| d == sup) && !direct.contains(name) {
                indirect.push(name.clone());
            }
        }
    }
    indirect.sort();
    Ok(json!({
        "iface": entry.java_name,
        "direct": direct,
        "indirect": indirect,
    }))
}

fn get_subclasses(project: &Project, body: &Value) -> Result<Value> {
    let cls = required_str(body, "cls")?;
    let entry = project
        .lookup_fuzzy(&cls)
        .ok_or_else(|| DecxError::class_not_found(cls))?;
    let hierarchy = project.hierarchy()?;
    let mut subs: Vec<String> = hierarchy
        .iter()
        .filter(|(_, (sup, _))| sup.as_deref() == Some(entry.java_name.as_str()))
        .map(|(name, _)| name.clone())
        .collect();
    subs.sort();
    Ok(json!({ "cls": entry.java_name, "subclasses": subs }))
}

fn get_app_manifest(_project: &Project) -> Result<Value> {
    // AXML decoding is not implemented in the native stack yet; documented
    // gap in RESEARCH.md §7.
    Err(DecxError::new(
        "MANIFEST_NOT_FOUND",
        "binary AndroidManifest.xml decoding is not implemented in the native core yet",
    ))
}
