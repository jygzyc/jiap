//! AndroidManifest model: package/version/permissions/components/deep links.
//! Parsed from AXML text output (keeps AXML decoder the single source of truth).

#[derive(Default, Clone)]
pub struct Component {
    pub kind: String, // activity | service | receiver | provider
    pub name: String,
    pub exported: Option<bool>,
    pub permission: Option<String>,
    pub intent_actions: Vec<String>,
    pub intent_categories: Vec<String>,
    pub data_schemes: Vec<String>,
    pub data_hosts: Vec<String>,
    pub has_intent_filter: bool,
}

#[derive(Default)]
pub struct Manifest {
    pub package: String,
    pub version_name: Option<String>,
    pub version_code: Option<i64>,
    pub min_sdk: Option<i64>,
    pub target_sdk: Option<i64>,
    pub permissions: Vec<String>,
    pub application_label: Option<String>,
    pub app_name: Option<String>,
    pub components: Vec<Component>,
}

fn tag_stack_push(stack: &mut Vec<String>, tag: String) {
    stack.push(tag);
}

/// Parse the decoded manifest text XML. Tiny hand parser: line-based
/// (our own AXML emitter is line-per-tag, so this is exact for our input).
pub fn parse(xml: &str) -> Manifest {
    let mut m = Manifest::default();
    let mut stack: Vec<String> = Vec::new();
    for line in xml.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix('<') else { continue };
        if rest.starts_with('?') || rest.starts_with('!') {
            continue;
        }
        if let Some(rest) = rest.strip_prefix('/') {
            // closing tag
            let tag = rest.trim_end_matches('>');
            if stack.last().map(|s| s.as_str()) == Some(tag) {
                stack.pop();
            }
            continue;
        }
        let self_closing = rest.ends_with("/>");
        let body = rest.trim_end_matches('>').trim_end_matches('/');
        let mut parts = body.split_whitespace();
        let Some(tag) = parts.next() else { continue };
        let attrs = parse_attrs(body);
        let get = |k: &str| attrs.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());

        match tag {
            "manifest" => {
                m.package = get("package").unwrap_or_default();
                if let Some(v) = get("android:versionName") {
                    m.version_name = Some(v);
                }
                if let Some(v) = get("android:versionCode") {
                    m.version_code = v.parse().ok();
                }
            }
            "uses-sdk" => {
                if let Some(v) = get("android:minSdkVersion") {
                    m.min_sdk = v.parse().ok();
                }
                if let Some(v) = get("android:targetSdkVersion") {
                    m.target_sdk = v.parse().ok();
                }
            }
            "uses-permission" => {
                if let Some(p) = get("android:name") {
                    m.permissions.push(p);
                }
            }
            "application" => {
                if let Some(l) = get("android:label") {
                    m.application_label = Some(l);
                }
                if let Some(n) = get("android:name") {
                    m.app_name = Some(n);
                }
            }
            "activity" | "activity-alias" | "service" | "receiver" | "provider" => {
                let kind = if tag == "activity-alias" {
                    "activity".to_string()
                } else {
                    tag.to_string()
                };
                let exported = get("android:exported").map(|v| v == "true");
                m.components.push(Component {
                    kind,
                    name: get("android:name").unwrap_or_default(),
                    exported,
                    permission: get("android:permission"),
                    has_intent_filter: false,
                    ..Default::default()
                });
            }
            "intent-filter" => {
                if let Some(c) = m.components.last_mut() {
                    c.has_intent_filter = true;
                }
            }
            "action" => {
                if let (Some(c), Some(n)) = (m.components.last_mut(), get("android:name")) {
                    c.intent_actions.push(n);
                }
            }
            "category" => {
                if let (Some(c), Some(n)) = (m.components.last_mut(), get("android:name")) {
                    c.intent_categories.push(n);
                }
            }
            "data" => {
                if let Some(c) = m.components.last_mut() {
                    if let Some(s) = get("android:scheme") {
                        c.data_schemes.push(s);
                    }
                    if let Some(h) = get("android:host") {
                        c.data_hosts.push(h);
                    }
                }
            }
            _ => {}
        }
        if !self_closing && !is_self_closing_tag(tag) {
            tag_stack_push(&mut stack, tag.to_string());
        }
    }
    m
}

fn is_self_closing_tag(tag: &str) -> bool {
    matches!(
        tag,
        "uses-permission" | "uses-sdk" | "action" | "category" | "data" | "meta-data"
    )
}

fn parse_attrs(body: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let bytes: Vec<char> = body.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        // scan to attr start (letter)
        if !bytes[i].is_alphabetic() && bytes[i] != ':' && bytes[i] != '_' && bytes[i] != '.' {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len()
            && (bytes[i].is_alphanumeric() || bytes[i] == ':' || bytes[i] == '-' || bytes[i] == '_' || bytes[i] == '.')
        {
            i += 1;
        }
        let name: String = bytes[start..i].iter().collect();
        // skip ws, expect =
        while i < bytes.len() && bytes[i] == ' ' {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != '=' {
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i] == ' ' {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != '"' {
            continue;
        }
        i += 1;
        let vstart = i;
        while i < bytes.len() && bytes[i] != '"' {
            i += 1;
        }
        let value: String = bytes[vstart..i].iter().collect();
        if i < bytes.len() {
            i += 1;
        }
        if !name.is_empty() {
            out.push((name, unescape(&value)));
        }
    }
    out
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
}

impl Manifest {
    pub fn main_activity(&self) -> Option<&Component> {
        self.components.iter().find(|c| {
            c.kind == "activity"
                && c.intent_actions.iter().any(|a| a == "android.intent.action.MAIN")
                && c.intent_categories
                    .iter()
                    .any(|cat| cat == "android.intent.category.LAUNCHER")
        })
    }

    pub fn deep_links(&self) -> Vec<(String, String, String)> {
        // (component, scheme, host)
        let mut out = Vec::new();
        for c in &self.components {
            for s in &c.data_schemes {
                let host = c.data_hosts.first().cloned().unwrap_or_default();
                out.push((c.name.clone(), s.clone(), host));
            }
        }
        out
    }

    pub fn exported(&self) -> Vec<&Component> {
        self.components
            .iter()
            .filter(|c| match c.exported {
                Some(e) => e,
                // implicit: exported if it has an intent-filter and no explicit flag
                None => c.has_intent_filter,
            })
            .collect()
    }
}
