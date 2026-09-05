//! DecxApiResult envelope — byte-compatible (semantically) with the Kotlin
//! `AnalysisResultUtils` contract used by DecxServer/RouteHandler:
//!
//! success: `{ok:true, kind, query, summary:{total,returned,truncated,...}, items, page:{index,size,has_next}}`
//! error:   `{ok:false, kind, query, error:{code,message}}`
//! item:    `{id, kind, title, content?, meta?}` with kinds symbol/code/xref.
//!
//! Pagination mirrors `AnalysisResultUtils.paginate`:
//! - page 1 of a response that already serializes to <= 64 KiB is returned as-is
//! - a single `code` item is paginated by source lines (binary-searched page size)
//! - anything else is paginated by item count (binary-searched page size)

use serde_json::{json, Map, Value};

/// Maximum serialized response size before pagination kicks in (matches Kotlin).
pub const MAX_PAGE_BYTES: usize = 65_535;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemKind {
    Symbol,
    Code,
    Xref,
}

impl ItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ItemKind::Symbol => "symbol",
            ItemKind::Code => "code",
            ItemKind::Xref => "xref",
        }
    }
}

/// One analysis item. `content`/`meta` are optional because the Kotlin layer
/// simply omits absent keys from the Gson output.
pub struct Item {
    pub id: String,
    pub kind: ItemKind,
    pub title: String,
    pub content: Option<String>,
    pub meta: Option<Map<String, Value>>,
}

impl Item {
    pub fn symbol(id: impl Into<String>, title: impl Into<String>, content: impl Into<String>) -> Self {
        Item {
            id: id.into(),
            kind: ItemKind::Symbol,
            title: title.into(),
            content: Some(content.into()),
            meta: None,
        }
    }

    /// Symbol item without content (e.g. AIDL interfaces).
    pub fn symbol_bare(id: impl Into<String>, title: impl Into<String>, meta: Map<String, Value>) -> Self {
        Item {
            id: id.into(),
            kind: ItemKind::Symbol,
            title: title.into(),
            content: None,
            meta: Some(meta),
        }
    }

    pub fn symbol_meta(
        id: impl Into<String>,
        title: impl Into<String>,
        content: impl Into<String>,
        meta: Map<String, Value>,
    ) -> Self {
        Item {
            id: id.into(),
            kind: ItemKind::Symbol,
            title: title.into(),
            content: Some(content.into()),
            meta: Some(meta),
        }
    }

    pub fn code(
        id: impl Into<String>,
        title: impl Into<String>,
        content: impl Into<String>,
        meta: Map<String, Value>,
    ) -> Self {
        Item {
            id: id.into(),
            kind: ItemKind::Code,
            title: title.into(),
            content: Some(content.into()),
            meta: Some(meta),
        }
    }

    /// Code item without meta (e.g. raw resource files).
    pub fn code_bare(id: impl Into<String>, title: impl Into<String>, content: impl Into<String>) -> Self {
        Item {
            id: id.into(),
            kind: ItemKind::Code,
            title: title.into(),
            content: Some(content.into()),
            meta: None,
        }
    }

    pub fn xref(
        id: impl Into<String>,
        title: impl Into<String>,
        content: impl Into<String>,
        meta: Map<String, Value>,
    ) -> Self {
        Item {
            id: id.into(),
            kind: ItemKind::Xref,
            title: title.into(),
            content: Some(content.into()),
            meta: Some(meta),
        }
    }

    fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("id".into(), json!(self.id));
        m.insert("kind".into(), json!(self.kind.as_str()));
        m.insert("title".into(), json!(self.title));
        if let Some(content) = &self.content {
            m.insert("content".into(), json!(content));
        }
        if let Some(meta) = &self.meta {
            m.insert("meta".into(), Value::Object(meta.clone()));
        }
        Value::Object(m)
    }
}

/// Build a query object from ordered `(key, value)` pairs.
pub fn query_object(pairs: &[(&str, Value)]) -> Value {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert((*k).to_string(), v.clone());
    }
    Value::Object(m)
}

/// Success response under construction. `paginate` produces the final JSON.
pub struct SuccessResponse {
    pub kind: &'static str,
    pub query: Value,
    pub items: Vec<Item>,
    /// Extra summary keys beyond total/returned/truncated (insertion order kept
    /// by emitting them in order; serde_json itself orders object keys).
    pub summary_extra: Vec<(String, Value)>,
    /// Requested page number (>= 1; callers normalize).
    pub page: u64,
}

fn assemble(
    kind: &str,
    query: &Value,
    summary: &Value,
    items_json: &[Value],
    page_index: u64,
    page_size: usize,
    has_next: bool,
) -> Value {
    json!({
        "ok": true,
        "kind": kind,
        "query": query,
        "summary": summary,
        "items": items_json,
        "page": {
            "index": page_index,
            "size": page_size,
            "has_next": has_next,
        },
    })
}

fn summary_json(total: usize, returned: usize, truncated: bool, extra: &[(String, Value)]) -> Value {
    let mut m = Map::new();
    m.insert("total".into(), json!(total));
    m.insert("returned".into(), json!(returned));
    m.insert("truncated".into(), json!(truncated));
    for (k, v) in extra {
        m.insert(k.clone(), v.clone());
    }
    Value::Object(m)
}

impl SuccessResponse {
    fn serialized_len(&self, summary: &Value, items_json: &[Value], page_size: usize, has_next: bool) -> usize {
        let v = assemble(self.kind, &self.query, summary, items_json, self.page, page_size, has_next);
        serde_json::to_string(&v).map(|s| s.len()).unwrap_or(usize::MAX)
    }

    /// Produce the final paginated success JSON (Kotlin `AnalysisResultUtils.paginate`).
    pub fn paginate(mut self) -> Value {
        if self.page == 0 {
            self.page = 1;
        }
        let total = self.items.len();

        // Page 1 of an already-small response: return as-is.
        let full_items: Vec<Value> = self.items.iter().map(Item::to_json).collect();
        let full_summary = summary_json(total, total, false, &self.summary_extra);
        if self.page == 1
            && self.serialized_len(&full_summary, &full_items, total, false) <= MAX_PAGE_BYTES
        {
            return assemble(self.kind, &self.query, &full_summary, &full_items, 1, total, false);
        }

        // Single source item: paginate by lines.
        if total == 1 && self.items[0].kind == ItemKind::Code {
            let content = self.items[0].content.clone().unwrap_or_default();
            let lines: Vec<&str> = content.lines().collect();
            let total_lines = lines.len();
            self.summary_extra
                .push(("total_lines".into(), json!(total_lines)));

            // Measurement uses a full page (first `page_size` lines) so the
            // binary search maximizes real page size regardless of which page
            // was requested; the requested page is sliced afterwards.
            let mut render = |page_size: usize| -> usize {
                let end = page_size.min(total_lines);
                let mut tmp = Vec::with_capacity(1);
                let mut item = Map::new();
                item.insert("id".into(), json!(self.items[0].id));
                item.insert("kind".into(), json!("code"));
                item.insert("title".into(), json!(self.items[0].title));
                item.insert("content".into(), json!(lines[0..end].join("\n")));
                let mut meta = self.items[0].meta.clone().unwrap_or_default();
                meta.insert("line_start".into(), json!(1));
                meta.insert("line_end".into(), json!(end));
                meta.insert("total_lines".into(), json!(total_lines));
                item.insert("meta".into(), Value::Object(meta));
                tmp.push(Value::Object(item));
                let summary = summary_json(total, end, end < total_lines, &self.summary_extra);
                self.serialized_len(&summary, &tmp, page_size, end < total_lines)
            };

            let page_size = largest_fitting(total_lines.max(1), &mut render).max(1);
            let start = ((self.page - 1) as usize) * page_size;
            let end = (start + page_size).min(total_lines);
            let has_next = end < total_lines;

            let mut item = Map::new();
            item.insert("id".into(), json!(self.items[0].id));
            item.insert("kind".into(), json!("code"));
            item.insert("title".into(), json!(self.items[0].title));
            item.insert("content".into(), json!(lines[start.min(total_lines)..end].join("\n")));
            let mut meta = self.items[0].meta.clone().unwrap_or_default();
            meta.insert("line_start".into(), json!(start + 1));
            meta.insert("line_end".into(), json!(end));
            meta.insert("total_lines".into(), json!(total_lines));
            item.insert("meta".into(), Value::Object(meta));

            let summary = summary_json(total, end - start, has_next, &self.summary_extra);
            return assemble(
                self.kind,
                &self.query,
                &summary,
                &[Value::Object(item)],
                self.page,
                page_size,
                has_next,
            );
        }

        // List pagination: binary-search the largest page size that fits;
        // measurement uses a full first page, the requested page is sliced.
        let mut render = |count: usize| -> usize {
            let end = count.min(total);
            let sliced = &full_items[0..end];
            let summary = summary_json(total, end, true, &self.summary_extra);
            self.serialized_len(&summary, sliced, count, end < total)
        };
        let count = largest_fitting(total.max(1), &mut render).max(1);
        let start = ((self.page - 1) as usize) * count;
        let end = (start + count).min(total);
        let has_next = end < total;
        let sliced = full_items[start.min(total)..end].to_vec();
        let summary = summary_json(total, end - start, end - start < total, &self.summary_extra);
        assemble(self.kind, &self.query, &summary, &sliced, self.page, count, has_next)
    }
}

/// Binary search the largest `n` in [1, max] with `render(n) <= MAX_PAGE_BYTES`.
/// Falls back to 1 when even that overflows (Kotlin keeps page size >= 1 too).
fn largest_fitting(hard_max: usize, render: &mut dyn FnMut(usize) -> usize) -> usize {
    let mut lo = 1usize;
    let mut hi = hard_max.max(1);
    let mut best = 1usize;
    if render(hi) <= MAX_PAGE_BYTES {
        return hi;
    }
    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        if render(mid) <= MAX_PAGE_BYTES {
            best = mid;
            lo = mid + 1;
        } else {
            hi = mid.saturating_sub(1);
        }
    }
    best
}

/// Error response in the shared envelope shape.
pub fn error_response(kind: &str, query: &Value, code: &str, message: &str) -> Value {
    json!({
        "ok": false,
        "kind": kind,
        "query": query,
        "error": {
            "code": code,
            "message": message,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big_symbol(i: usize) -> Item {
        Item::symbol(format!("com.example.Class{i}"), "Class: Class", "x".repeat(200))
    }

    #[test]
    fn small_response_is_returned_as_is() {
        let resp = SuccessResponse {
            kind: "classes",
            query: query_object(&[("target", json!("classes"))]),
            items: vec![Item::symbol("a.B", "Class: B", "a.B")],
            summary_extra: vec![],
            page: 1,
        };
        let v = resp.paginate();
        assert_eq!(v["ok"], json!(true));
        assert_eq!(v["kind"], json!("classes"));
        assert_eq!(v["summary"]["total"], json!(1));
        assert_eq!(v["page"]["index"], json!(1));
        assert_eq!(v["page"]["has_next"], json!(false));
        assert_eq!(v["items"][0]["kind"], json!("symbol"));
    }

    #[test]
    fn list_pagination_splits_pages() {
        let items: Vec<Item> = (0..600).map(big_symbol).collect();
        let resp = SuccessResponse {
            kind: "classes",
            query: query_object(&[("target", json!("classes"))]),
            items,
            summary_extra: vec![],
            page: 1,
        };
        let v = resp.paginate();
        assert_eq!(v["page"]["index"], json!(1));
        assert_eq!(v["page"]["has_next"], json!(true));
        assert_eq!(v["summary"]["total"], json!(600));
        assert!(v["items"].as_array().unwrap().len() < 600);
        assert_eq!(v["summary"]["returned"], json!(v["items"].as_array().unwrap().len()));
    }

    #[test]
    fn code_item_paginates_by_lines() {
        let content = (0..12_000).map(|i| format!("line {i} of the source")).collect::<Vec<_>>().join("\n");
        let resp = SuccessResponse {
            kind: "class_source",
            query: query_object(&[("target", json!("a.B"))]),
            items: vec![Item::code("a.B", "a.B", content, {
                let mut m = Map::new();
                m.insert("language".into(), json!("java"));
                m
            })],
            summary_extra: vec![],
            page: 2,
        };
        let v = resp.paginate();
        assert_eq!(v["summary"]["total_lines"], json!(12_000));
        assert!(v["page"]["has_next"].as_bool().unwrap());
        let meta = &v["items"][0]["meta"];
        assert_eq!(meta["line_start"], json!((v["page"]["size"].as_u64().unwrap()) + 1));
        assert_eq!(meta["total_lines"], json!(12_000));
    }
}
