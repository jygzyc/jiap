//! DecxApiResult envelope, Kotlin-server compatible.
//! success: {ok:true, kind, query, summary, items, page}
//! error:   {ok:false, kind, query, error:{code, message}}

use decx_json::Json;

pub const MAX_PAGE_BYTES: usize = 65535;

/// Page items so the serialized JSON body stays under MAX_PAGE_BYTES.
pub fn paginate(full: &Json, kind: &str, query: &Json, meta: &Json) -> Json {
    let items = full
        .get("items")
        .and_then(|i| i.as_arr())
        .map(|s| s.to_vec())
        .unwrap_or_default();
    let total = items.len();
    // contract: 1-based page carried inside the query object
    let page_1based = query
        .get("page")
        .and_then(|v| v.as_i64())
        .unwrap_or(1)
        .max(1) as usize;
    let page_idx = page_1based - 1;

    if items.is_empty() {
        return Json::obj(vec![
            ("ok", Json::Bool(true)),
            ("kind", Json::str(kind)),
            ("query", query.clone()),
            ("summary", summary(0, 0, false, meta)),
            ("items", Json::Arr(vec![])),
            (
                "page",
                Json::obj(vec![
                    ("index", Json::Int(page_idx as i64)),
                    ("size", Json::Int(0)),
                    ("has_next", Json::Bool(false)),
                ]),
            ),
        ]);
    }

    let base = Json::obj(vec![
        ("ok", Json::Bool(true)),
        ("kind", Json::str(kind)),
        ("query", query.clone()),
    ]);
    // measure one item size to pick a page size, then binary search if needed
    let item_cost = items
        .first()
        .map(|i| i.to_string().len() + 1)
        .unwrap_or(0);
    let mut page_size = if item_cost == 0 {
        items.len().max(1)
    } else {
        (MAX_PAGE_BYTES / item_cost).max(1)
    };
    page_size = page_size.min(items.len().max(1));

    let page_of = |n: usize| -> Option<Json> {
        if n == 0 || n > items.len() {
            return None;
        }
        let start = (page_idx).saturating_mul(n).min(items.len());
        let end = (start + n).min(items.len());
        if start >= items.len() {
            return None; // requested page beyond the data
        }
        let slice: Vec<Json> = items[start..end].to_vec();
        let returned = slice.len();
        let mut body = base.clone();
        body.set("summary", summary(total, returned, end < total, meta));
        body.set("items", Json::Arr(slice));
        body.set(
            "page",
            Json::obj(vec![
                ("index", Json::Int(page_idx as i64)),
                ("size", Json::Int(returned as i64)),
                ("has_next", Json::Bool(end < total)),
            ]),
        );
        if body.to_string().len() <= MAX_PAGE_BYTES {
            Some(body)
        } else {
            None
        }
    };

    if let Some(p) = page_of(page_size) {
        return p;
    }
    // binary search largest n that fits
    let mut lo = 1usize;
    let mut hi = page_size;
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        if page_of(mid).is_some() {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    page_of(lo.max(1)).unwrap_or_else(|| {
        // single oversized item: line-chunk its content (Kotlin engine does
        // line pagination for code outputs; we approximate by keeping the
        // first lines that fit and marking truncated)
        if let Some(first) = items.first() {
            if let Some(content) = first.get("content").and_then(|c| c.as_str()) {
                return truncated_content_page(kind, query, meta, first, content);
            }
        }
        error_envelope(kind, query, "INTERNAL_ERROR", "page failed")
    })
}

/// Single item whose serialized form exceeds MAX_PAGE_BYTES: keep the leading
/// lines that fit under the budget, append a truncation marker.
fn truncated_content_page(
    kind: &str,
    query: &Json,
    meta: &Json,
    proto: &Json,
    content: &str,
) -> Json {
    let page_1based = query
        .get("page")
        .and_then(|v| v.as_i64())
        .unwrap_or(1)
        .max(1) as usize;
    let page_idx = page_1based - 1;
    let all_lines: Vec<&str> = content.split('\n').collect();
    let body_with = |n: usize| -> Option<Json> {
        if n == 0 {
            return None;
        }
        let start = page_idx.saturating_mul(n).min(all_lines.len());
        let end = (start + n).min(all_lines.len());
        if start >= all_lines.len() {
            return None; // requested page beyond the data
        }
        let kept: Vec<&str> = all_lines[start..end].to_vec();
        let mut text = kept.join("\n");
        if end < all_lines.len() {
            text.push_str(&format!(
                "\n// [decx-native] truncated: showing lines {}..{}/{} (page budget {} bytes)",
                start + 1,
                end,
                all_lines.len(),
                MAX_PAGE_BYTES
            ));
        }
        let mut item = proto.clone();
        item.set("content", Json::str(&text));
        let mut sm = summary(1, 1, end < all_lines.len(), meta);
        sm.set("lines_total", Json::Int(all_lines.len() as i64));
        sm.set("lines_returned", Json::Int((end - start) as i64));
        let body = Json::obj(vec![
            ("ok", Json::Bool(true)),
            ("kind", Json::str(kind)),
            ("query", query.clone()),
            ("summary", sm),
            ("items", Json::Arr(vec![item])),
            (
                "page",
                Json::obj(vec![
                    ("index", Json::Int(page_idx as i64)),
                    ("size", Json::Int(1)),
                    ("has_next", Json::Bool(end < all_lines.len())),
                ]),
            ),
        ]);
        if body.to_string().len() <= MAX_PAGE_BYTES {
            Some(body)
        } else {
            None
        }
    };
    // find largest per-page line count that fits the budget
    let mut lo = 1usize;
    let mut hi = all_lines.len();
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        if body_with(mid).is_some() {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    body_with(lo.max(1)).unwrap_or_else(|| error_envelope(kind, query, "INTERNAL_ERROR", "page failed"))
}

fn summary(total: usize, returned: usize, truncated: bool, meta: &Json) -> Json {
    let mut o = vec![
        ("total".to_string(), Json::Int(total as i64)),
        ("returned".to_string(), Json::Int(returned as i64)),
        ("truncated".to_string(), Json::Bool(truncated)),
    ];
    if let Json::Obj(kv) = meta {
        for (k, v) in kv {
            o.push((k.clone(), v.clone()));
        }
    }
    Json::Obj(o)
}

/// Build an unpaged success (small payloads, e.g. manifest model).
pub fn success(kind: &str, query: Json, items: Vec<Json>, meta: Json) -> Json {
    paginate(
        &Json::obj(vec![("items", Json::Arr(items))]),
        kind,
        &query,
        &meta,
    )
}

pub fn error_envelope(kind: &str, query: &Json, code: &str, message: &str) -> Json {
    Json::obj(vec![
        ("ok", Json::Bool(false)),
        ("kind", Json::str(kind)),
        ("query", query.clone()),
        (
            "error",
            Json::obj(vec![("code", Json::str(code)), ("message", Json::str(message))]),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, content: &str) -> Json {
        Json::obj(vec![
            ("id", Json::str(id)),
            ("kind", Json::str("code")),
            ("title", Json::str(id)),
            ("content", Json::str(content)),
        ])
    }

    #[test]
    fn empty_items_is_valid_page() {
        let body = paginate(
            &Json::obj(vec![("items", Json::Arr(vec![]))]),
            "method_xref",
            &Json::obj(vec![("method", Json::str("La;->b()V"))]),
            &Json::obj(vec![]),
        );
        assert_eq!(body.get("ok"), Some(&Json::Bool(true)));
        assert_eq!(body.get("items").and_then(|i| i.as_arr()).map(|a| a.len()), Some(0));
        assert_eq!(body.get("summary").unwrap().get("total").cloned(), Some(Json::Int(0)));
    }

    #[test]
    fn oversized_content_line_truncated() {
        // ~100KB of lines: must NOT error, must fit budget, must mark truncated
        let content: Vec<String> = (0..2000).map(|i| format!("// line {i:04} padding padding padding padding padding"))
            .collect();
        let big = content.join("\n");
        assert!(big.len() > MAX_PAGE_BYTES);
        let body = paginate(
            &Json::obj(vec![("items", Json::Arr(vec![item("Big", &big)]))]),
            "class_source",
            &Json::obj(vec![("class", Json::str("LBig;"))]),
            &Json::obj(vec![]),
        );
        assert_eq!(body.get("ok"), Some(&Json::Bool(true)), "must not be page failed");
        assert!(body.to_string().len() <= MAX_PAGE_BYTES);
        assert_eq!(
            body.get("summary").unwrap().get("truncated").cloned(),
            Some(Json::Bool(true))
        );
        let content = body
            .get("items")
            .and_then(|i| i.as_arr())
            .and_then(|a| a.first())
            .and_then(|it| it.get("content"))
            .and_then(|c| c.as_str())
            .unwrap()
            .to_string();
        assert!(content.contains("[decx-native] truncated"));
        assert!(content.contains("// line 0000"));
    }
}
