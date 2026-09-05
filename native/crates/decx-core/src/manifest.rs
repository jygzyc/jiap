//! APK-level resources + AndroidManifest model — backs the Android analysis
//! endpoints (`get_app_manifest`, `get_exported_components`, `get_deep_links`,
//! `get_main_activity`, `get_application`, `get_strings`, `get_all_resources`,
//! `get_resource_file`).
//!
//! Mirrors `AndroidService` + jadx's `AndroidManifest` on the Kotlin side.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{json, Map, Value};

use crate::arsc::{self, ArscTable};
use crate::axml::{self, AxmlDocument, XmlNode, ANDROID_NS};

/// Open APK resources from an apk/zip/jar target. Returns `None` when the
/// target is a bare dex or has no AndroidManifest.xml and no resources.arsc.
pub fn open(path: &Path) -> Option<ApkResources> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "apk" | "zip" | "jar" | "apks") {
        return None;
    }
    let file = File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(file).ok()?;
    let manifest_doc = read_zip(&mut zip, "AndroidManifest.xml")
        .and_then(|bytes| axml::parse(&bytes).ok());
    let arsc = read_zip(&mut zip, "resources.arsc").and_then(|bytes| arsc::parse(&bytes).ok());
    if manifest_doc.is_none() && arsc.is_none() {
        return None;
    }
    let mut file_names: Vec<String> = zip
        .file_names()
        .filter(|n| !n.ends_with('/'))
        .map(|n| n.to_string())
        .collect();
    file_names.sort();
    Some(ApkResources {
        zip: Mutex::new(zip),
        manifest_text: manifest_doc.as_ref().map(axml::to_text_xml),
        manifest_doc,
        arsc,
        file_names,
    })
}

fn read_zip(zip: &mut zip::ZipArchive<File>, name: &str) -> Option<Vec<u8>> {
    let mut entry = zip.by_name(name).ok()?;
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf).ok()?;
    Some(buf)
}

pub struct ApkResources {
    zip: Mutex<zip::ZipArchive<File>>,
    manifest_doc: Option<AxmlDocument>,
    manifest_text: Option<String>,
    arsc: Option<ArscTable>,
    file_names: Vec<String>,
}

impl ApkResources {
    pub fn manifest_text(&self) -> Option<&str> {
        self.manifest_text.as_deref()
    }

    fn manifest_root(&self) -> Option<&XmlNode> {
        self.manifest_doc.as_ref().map(|d| &d.root)
    }

    fn package_name(&self) -> Option<String> {
        self.manifest_root()
            .and_then(|m| m.attr_str(None, "package"))
    }

    /// `<uses-sdk>` targetSdkVersion, falling back to minSdkVersion, then 1
    /// (mirrors the Kotlin exported-components logic).
    fn target_sdk(&self) -> i32 {
        let sdk = self
            .manifest_root()
            .and_then(|m| m.first_child("uses-sdk"));
        sdk.and_then(|s| s.attr_int(Some(ANDROID_NS), "targetSdkVersion"))
            .or_else(|| sdk.and_then(|s| s.attr_int(Some(ANDROID_NS), "minSdkVersion")))
            .unwrap_or(1)
    }

    /// Resolved android:name of the `<application>` element.
    pub fn application(&self) -> Option<String> {
        let app = self.manifest_root()?.first_child("application")?;
        let raw = app.attr_str(Some(ANDROID_NS), "name")?;
        let pkg = self.package_name().unwrap_or_default();
        let resolved = resolve_component_name(&pkg, &raw)?;
        Some(resolved)
    }

    /// Resolved name of the launcher activity (MAIN/LAUNCHER intent filter),
    /// including activity-alias targets.
    pub fn main_activity(&self) -> Option<String> {
        let root = self.manifest_root()?;
        let app = root.first_child("application")?;
        let pkg = root.attr_str(None, "package").unwrap_or_default();
        let mut alias_targets: Vec<String> = Vec::new();
        for alias in app.children_named("activity-alias") {
            for f in alias.children_named("intent-filter") {
                let has_main = f.children_named("action").any(|a| {
                    a.attr_str(Some(ANDROID_NS), "name").map(|n| n == "android.intent.action.MAIN").unwrap_or(false)
                });
                let has_launcher = f.children_named("category").any(|c| {
                    c.attr_str(Some(ANDROID_NS), "name")
                        .map(|n| n == "android.intent.category.LAUNCHER")
                        .unwrap_or(false)
                });
                if has_main && has_launcher {
                    if let Some(target) = alias.attr_str(Some(ANDROID_NS), "targetActivity") {
                        if let Some(resolved) = resolve_component_name(&pkg, &target) {
                            alias_targets.push(resolved);
                        }
                    }
                }
            }
        }
        if let Some(target) = alias_targets.first() {
            return Some(target.clone());
        }
        for activity in app.children_named("activity") {
            for f in activity.children_named("intent-filter") {
                let has_main = f.children_named("action").any(|a| {
                    a.attr_str(Some(ANDROID_NS), "name").map(|n| n == "android.intent.action.MAIN").unwrap_or(false)
                });
                let has_launcher = f.children_named("category").any(|c| {
                    c.attr_str(Some(ANDROID_NS), "name")
                        .map(|n| n == "android.intent.category.LAUNCHER")
                        .unwrap_or(false)
                });
                if has_main && has_launcher {
                    let raw = activity.attr_str(Some(ANDROID_NS), "name")?;
                    return resolve_component_name(&pkg, &raw);
                }
            }
        }
        None
    }

    /// All resource file names visible to DECX: real zip entries plus the
    /// synthetic `res/values/strings.xml` bridge over resources.arsc.
    pub fn resource_file_names(&self) -> Vec<String> {
        let mut names = self.file_names.clone();
        if let Some(arsc) = &self.arsc {
            if arsc.packages.iter().any(|p| !p.strings.is_empty()) {
                let pseudo = "res/values/strings.xml";
                if !names.iter().any(|n| n == pseudo) {
                    names.push(pseudo.to_string());
                    names.sort();
                }
            }
        }
        names
    }

    /// Read one resource file as text. Falls back to the arsc bridge for the
    /// synthetic strings file.
    pub fn read_resource_file(&self, target: &str) -> Option<String> {
        if let Ok(mut zip) = self.zip.lock() {
            if let Ok(mut entry) = zip.by_name(target) {
                let mut buf = Vec::with_capacity(entry.size() as usize);
                if entry.read_to_end(&mut buf).is_ok() {
                    return Some(String::from_utf8_lossy(&buf).into_owned());
                }
            }
        }
        if target.ends_with("strings.xml") {
            if let Some(text) = self.strings_xml_text() {
                return Some(text);
            }
        }
        None
    }

    pub fn strings_xml_text(&self) -> Option<String> {
        let arsc = self.arsc.as_ref()?;
        let pkg = arsc.packages.iter().rev().find(|p| !p.strings.is_empty())?;
        Some(arsc::strings_to_xml(pkg))
    }

    /// (name, value) pairs for `get_strings`.
    pub fn strings(&self) -> Vec<(String, String)> {
        self.arsc
            .as_ref()
            .map(|arsc| {
                arsc.packages
                    .iter()
                    .flat_map(|p| p.strings.iter().map(|s| (s.name.clone(), s.value.clone())))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Exported components with full metadata maps (Kotlin `get_exported_components`).
    pub fn exported_components(&self) -> Vec<(String, String, Map<String, Value>)> {
        let mut out = Vec::new();
        let Some(root) = self.manifest_root() else { return out };
        let Some(app) = root.first_child("application") else { return out };
        let pkg = root.attr_str(None, "package").unwrap_or_default();
        let target_sdk = self.target_sdk();

        for tag in ["activity", "service", "receiver", "provider"] {
            for node in app.children_named(tag) {
                let raw_name = node.attr_str(Some(ANDROID_NS), "name").unwrap_or_default();
                let Some(name) = resolve_component_name(&pkg, &raw_name) else { continue };

                let filters: Vec<&XmlNode> = node.children_named("intent-filter").collect();
                let has_action_filter = filters
                    .iter()
                    .any(|f| f.children_named("action").count() > 0);
                let exported = node
                    .attr_bool(Some(ANDROID_NS), "exported")
                    .unwrap_or_else(|| match tag {
                        "provider" => target_sdk < 17,
                        _ => has_action_filter,
                    });
                if !exported {
                    continue;
                }

                let mut meta = Map::new();
                meta.insert("name".into(), json!(name));
                meta.insert("type".into(), json!(tag));
                if let Some(v) = node.attr_str(Some(ANDROID_NS), "permission") {
                    meta.insert("permission".into(), json!(v));
                }
                if let Some(v) = attr_launch_mode(node) {
                    meta.insert("launchMode".into(), json!(v));
                }
                if let Some(v) = node.attr_str(Some(ANDROID_NS), "taskAffinity") {
                    meta.insert("taskAffinity".into(), json!(v));
                }
                if let Some(v) = node.attr_str(Some(ANDROID_NS), "authorities") {
                    meta.insert("authorities".into(), json!(v));
                }
                if let Some(v) = node.attr_str(Some(ANDROID_NS), "readPermission") {
                    meta.insert("readPermission".into(), json!(v));
                }
                if let Some(v) = node.attr_str(Some(ANDROID_NS), "writePermission") {
                    meta.insert("writePermission".into(), json!(v));
                }
                if node.attr_bool(Some(ANDROID_NS), "grantUriPermissions") == Some(true) {
                    meta.insert("grantUriPermissions".into(), json!(true));
                }

                let intent_filters: Vec<Value> = filters
                    .iter()
                    .map(|f| {
                        let mut fm = Map::new();
                        let actions: Vec<String> = f
                            .children_named("action")
                            .filter_map(|a| a.attr_str(Some(ANDROID_NS), "name"))
                            .collect();
                        fm.insert("actions".into(), json!(actions));
                        let categories: Vec<String> = f
                            .children_named("category")
                            .filter_map(|c| c.attr_str(Some(ANDROID_NS), "name"))
                            .collect();
                        if !categories.is_empty() {
                            fm.insert("categories".into(), json!(categories));
                        }
                        let data: Vec<Value> = f
                            .children_named("data")
                            .map(|d| {
                                let mut dm = Map::new();
                                for attr in ["scheme", "host", "port", "path", "pathPrefix", "pathPattern", "mimeType"] {
                                    if let Some(v) = d.attr_str(Some(ANDROID_NS), attr) {
                                        if !v.is_empty() {
                                            dm.insert(attr.into(), json!(v));
                                        }
                                    }
                                }
                                Value::Object(dm)
                            })
                            .collect();
                        if !data.is_empty() {
                            fm.insert("data".into(), json!(data));
                        }
                        Value::Object(fm)
                    })
                    .collect();
                if !intent_filters.is_empty() {
                    meta.insert("intentFilters".into(), json!(intent_filters));
                }

                out.push((name.clone(), tag.to_string(), meta));
            }
        }
        out
    }

    /// Deep links: (component, uri, meta) triples (Kotlin `get_deep_links`).
    pub fn deep_links(&self) -> Vec<(String, String, Map<String, Value>)> {
        let mut out = Vec::new();
        let Some(root) = self.manifest_root() else { return out };
        let Some(app) = root.first_child("application") else { return out };
        let pkg = root.attr_str(None, "package").unwrap_or_default();

        for tag in ["activity", "activity-alias", "service", "receiver"] {
            for node in app.children_named(tag) {
                let raw_name = node.attr_str(Some(ANDROID_NS), "name").unwrap_or_default();
                let Some(component) = resolve_component_name(&pkg, &raw_name) else { continue };
                for f in node.children_named("intent-filter") {
                    let actions: Vec<String> = f
                        .children_named("action")
                        .filter_map(|a| a.attr_str(Some(ANDROID_NS), "name"))
                        .collect();
                    let categories: Vec<String> = f
                        .children_named("category")
                        .filter_map(|c| c.attr_str(Some(ANDROID_NS), "name"))
                        .collect();
                    if !actions.iter().any(|a| a == "android.intent.action.VIEW") {
                        continue;
                    }
                    if !categories.iter().any(|c| c == "android.intent.category.BROWSABLE") {
                        continue;
                    }
                    if !categories.iter().any(|c| c == "android.intent.category.DEFAULT") {
                        continue;
                    }
                    for d in f.children_named("data") {
                        let get = |attr: &str| d.attr_str(Some(ANDROID_NS), attr).filter(|v| !v.is_empty());
                        let Some(scheme) = get("scheme") else { continue };
                        let host = get("host").unwrap_or_default();
                        let port = get("port").unwrap_or_default();
                        let path = get("path")
                            .or_else(|| get("pathPrefix"))
                            .or_else(|| get("pathPattern"))
                            .unwrap_or_default();
                        let mut uri = format!("{scheme}://");
                        if !host.is_empty() {
                            uri.push_str(&host);
                            if !port.is_empty() {
                                uri.push(':');
                                uri.push_str(&port);
                            }
                        }
                        uri.push_str(&path);

                        let mut meta = Map::new();
                        meta.insert("component".into(), json!(component));
                        if let Some(v) = get("mimeType") {
                            meta.insert("mimeType".into(), json!(v));
                        }
                        if !host.is_empty() {
                            meta.insert("host".into(), json!(host));
                        }
                        if !port.is_empty() {
                            meta.insert("port".into(), json!(port));
                        }
                        if let Some(v) = get("path") {
                            meta.insert("path".into(), json!(v));
                        }
                        if let Some(v) = get("pathPrefix") {
                            meta.insert("pathPrefix".into(), json!(v));
                        }
                        if let Some(v) = get("pathPattern") {
                            meta.insert("pathPattern".into(), json!(v));
                        }
                        out.push((component.clone(), uri, meta));
                    }
                }
            }
        }
        out
    }
}

fn attr_launch_mode(node: &XmlNode) -> Option<String> {
    let raw = node.attr_int(Some(ANDROID_NS), "launchMode")?;
    Some(
        match raw {
            0 => "standard",
            1 => "singleTop",
            2 => "singleTask",
            3 => "singleInstance",
            _ => return None,
        }
        .to_string(),
    )
}

/// Resolve a manifest component name against the package: `.Foo` →
/// `<package>.Foo`, bare `Foo` → `<package>.Foo`, dotted names pass through.
pub fn resolve_component_name(package: &str, raw: &str) -> Option<String> {
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with('.') {
        Some(format!("{package}{raw}"))
    } else if raw.contains('.') {
        Some(raw.to_string())
    } else {
        Some(format!("{package}.{raw}"))
    }
}

/// Arc helper used by Project.
pub type SharedApkResources = Arc<ApkResources>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn sieve() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../decx-cli/tests/fixtures/sieve.apk")
    }

    #[test]
    fn sieve_manifest_smoke() {
        let path = sieve();
        if !path.exists() {
            eprintln!("skipping: {} not found", path.display());
            return;
        }
        let file = File::open(&path).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();
        let bytes = read_zip(&mut zip, "AndroidManifest.xml");
        assert!(bytes.is_some(), "manifest entry unreadable");
        let bytes = bytes.unwrap();
        eprintln!("manifest bytes: {}", bytes.len());
        let doc = axml::parse(&bytes);
        match doc {
            Ok(doc) => {
                eprintln!("root: {} attrs={} children={}", doc.root.name, doc.root.attrs.len(), doc.root.children.len());
                let text = axml::to_text_xml(&doc);
                eprintln!("text head: {}", text.lines().take(3).collect::<Vec<_>>().join(" | "));
            }
            Err(e) => panic!("axml parse failed: {e}"),
        }
        let arsc_bytes = read_zip(&mut zip, "resources.arsc").expect("arsc entry");
        let table = arsc::parse(&arsc_bytes).expect("arsc parse");
        eprintln!(
            "arsc: {} packages, strings: {}",
            table.packages.len(),
            table.packages.iter().map(|p| p.strings.len()).sum::<usize>()
        );
        for p in &table.packages {
            eprintln!("  package {} ({} strings)", p.name, p.strings.len());
        }
        let res = open(&path).expect("apk resources");
        assert!(res.manifest_text().is_some(), "manifest text missing");
        let strings = res.strings();
        assert!(!strings.is_empty(), "no strings parsed");
        eprintln!("first strings: {:?}", strings.iter().take(5).collect::<Vec<_>>());
    }
}
