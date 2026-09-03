//! Method index across one or more DEX files.

use std::collections::{HashMap, HashSet, VecDeque};

use dex_parser::{DexFile, EncodedMethod, NO_INDEX};

use crate::detectors::is_library_class;
use crate::java::descriptor_to_java;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MethodId(pub usize);

#[derive(Clone, Debug)]
pub struct MethodRef {
    pub id: MethodId,
    pub class_name: String,
    pub method_name: String,
    pub proto: String,
    pub dex_index: usize,
    pub encoded: EncodedMethod,
}

#[derive(Debug, Default)]
pub struct MethodIndex {
    pub methods: Vec<MethodRef>,
    /// `Class#method` → first matching MethodId (name-only; proto-ambiguous OK for v1).
    by_class_method: HashMap<String, Vec<MethodId>>,
    /// Direct subclasses / implementors. Library children are recorded but skipped at resolve.
    subclasses: HashMap<String, Vec<String>>,
}

impl MethodIndex {
    pub fn from_dexes(dexes: &[&DexFile]) -> Self {
        let mut idx = MethodIndex::default();
        for (dex_index, dex) in dexes.iter().enumerate() {
            for class_result in dex.class_defs() {
                let Ok(class_def) = class_result else {
                    continue;
                };
                let Ok(class_type) = dex.get_type(class_def.class_idx) else {
                    continue;
                };
                let class_name = descriptor_to_java(&class_type);
                // Hierarchy first so CHA can walk classes that have no indexed methods.
                if class_def.superclass_idx != NO_INDEX {
                    if let Ok(super_type) = dex.get_type(class_def.superclass_idx) {
                        let super_name = descriptor_to_java(&super_type);
                        idx.subclasses
                            .entry(super_name)
                            .or_default()
                            .push(class_name.clone());
                    }
                }
                for iface in parse_interfaces(dex, class_def.interfaces_off) {
                    idx.subclasses
                        .entry(iface)
                        .or_default()
                        .push(class_name.clone());
                }
                let Ok(Some(class_data)) = dex.get_class_data(&class_def) else {
                    continue;
                };
                for encoded in class_data
                    .direct_methods
                    .iter()
                    .chain(class_data.virtual_methods.iter())
                {
                    if encoded.code_off == 0 {
                        continue;
                    }
                    let Ok(info) = dex.get_method_info(encoded.method_idx) else {
                        continue;
                    };
                    let method_name = info.name.to_string();
                    let params: String = info.params.concat();
                    let proto = format!("({}){}", params, info.return_type);
                    let id = MethodId(idx.methods.len());
                    let key = format!("{class_name}#{method_name}");
                    idx.by_class_method.entry(key).or_default().push(id);
                    idx.methods.push(MethodRef {
                        id,
                        class_name: class_name.clone(),
                        method_name,
                        proto,
                        dex_index,
                        encoded: encoded.clone(),
                    });
                }
            }
        }
        idx
    }

    pub fn get(&self, id: MethodId) -> Option<&MethodRef> {
        self.methods.get(id.0)
    }

    pub fn callable_name(&self, id: MethodId) -> String {
        self.get(id)
            .map(|m| format!("{}#{}", m.class_name, m.method_name))
            .unwrap_or_else(|| format!("method#{}", id.0))
    }

    /// Resolve a callee from an invoke method ref like `com.foo.Bar.doThing`
    /// or `com.foo.Bar.doThing(Ljava/lang/String;)V`.
    /// Requires a class-qualified match — no simple-name fallback (avoids MT false edges).
    ///
    /// When a proto is present, prefer the candidate whose [`MethodRef::proto`] matches.
    /// A single name-only candidate is always used. Several candidates with no proto
    /// keep the first (legacy) to avoid dropping call-graph edges.
    pub fn resolve_callee(&self, method_ref: &str) -> Option<MethodId> {
        let (bare, proto) = match method_ref.find('(') {
            Some(i) => (method_ref[..i].trim(), Some(method_ref[i..].trim())),
            None => (method_ref.trim(), None),
        };
        let (cls, name) = bare.rsplit_once('.')?;
        let ids = self.candidates_for_class_method(cls, name).or_else(|| {
            let java_cls = cls
                .replace('/', ".")
                .trim_start_matches('L')
                .trim_end_matches(';')
                .to_string();
            self.candidates_for_class_method(&java_cls, name)
        })?;
        self.pick_callee(ids, proto)
    }

    /// CHA: declared-type callee plus app subclasses that override the same name/proto.
    ///
    /// Library subclasses (`android.*`, `java.*`, … via [`is_library_class`]) are skipped
    /// so `View#foo` cannot explode into every framework child. Cap: 4 callees,
    /// BFS depth/nodes bounded.
    pub fn resolve_callees(&self, method_ref: &str) -> Vec<MethodId> {
        const MAX_CALLEES: usize = 4;
        const MAX_NODES: usize = 32;
        const MAX_DEPTH: usize = 6;

        let mut out = Vec::new();
        let mut seen = HashSet::new();
        if let Some(id) = self.resolve_callee(method_ref) {
            seen.insert(id);
            out.push(id);
        }

        let (bare, proto) = match method_ref.find('(') {
            Some(i) => (method_ref[..i].trim(), Some(method_ref[i..].trim())),
            None => (method_ref.trim(), None),
        };
        let Some((cls, name)) = bare.rsplit_once('.') else {
            return out;
        };
        let declared = cls
            .replace('/', ".")
            .trim_start_matches('L')
            .trim_end_matches(';')
            .to_string();

        let mut q: VecDeque<(String, usize)> = VecDeque::new();
        q.push_back((declared, 0));
        let mut visited_cls = HashSet::new();
        let mut nodes = 0usize;
        while let Some((cur, depth)) = q.pop_front() {
            if depth >= MAX_DEPTH || nodes >= MAX_NODES || out.len() >= MAX_CALLEES {
                break;
            }
            if !visited_cls.insert(cur.clone()) {
                continue;
            }
            nodes += 1;
            let Some(children) = self.subclasses.get(&cur) else {
                continue;
            };
            let mut prioritized: Vec<&String> = children.iter().collect();
            prioritized.sort_by_key(|child| {
                let same_package = child
                    .rsplit_once('.')
                    .zip(cur.rsplit_once('.'))
                    .map(|((a, _), (b, _))| a == b)
                    .unwrap_or(false);
                (!same_package, child.as_str())
            });
            for child in prioritized {
                if out.len() >= MAX_CALLEES {
                    break;
                }
                // App subclasses only — skip framework/library children.
                if is_library_class(child) {
                    continue;
                }
                if let Some(ids) = self.candidates_for_class_method(child, name) {
                    if let Some(id) = self.pick_callee(ids, proto) {
                        if seen.insert(id) {
                            out.push(id);
                        }
                    }
                }
                q.push_back((child.clone(), depth + 1));
            }
        }
        out
    }

    fn candidates_for_class_method(&self, cls: &str, name: &str) -> Option<&[MethodId]> {
        self.by_class_method
            .get(&format!("{cls}#{name}"))
            .map(|v| v.as_slice())
    }

    fn pick_callee(&self, ids: &[MethodId], proto: Option<&str>) -> Option<MethodId> {
        if let Some(proto) = proto {
            if let Some(&id) = ids.iter().find(|&&id| {
                self.methods
                    .get(id.0)
                    .map(|m| m.proto == proto)
                    .unwrap_or(false)
            }) {
                return Some(id);
            }
            if ids.len() == 1 {
                return ids.first().copied();
            }
            // Several overloads and proto did not match: do not guess.
            return None;
        }
        ids.first().copied()
    }
}

/// Parse a DEX `type_list` at `interfaces_off` (u32 size + u16 type_ids).
fn parse_interfaces(dex: &DexFile, interfaces_off: u32) -> Vec<String> {
    if interfaces_off == 0 {
        return Vec::new();
    }
    let data = dex.data.as_ref();
    let off = interfaces_off as usize;
    if off + 4 > data.len() {
        return Vec::new();
    }
    let size = u32::from_le_bytes(data[off..off + 4].try_into().unwrap_or([0; 4])) as usize;
    let mut out = Vec::new();
    for i in 0..size {
        let base = off + 4 + i * 2;
        if base + 2 > data.len() {
            break;
        }
        let type_idx = u16::from_le_bytes(data[base..base + 2].try_into().unwrap_or([0; 2])) as u32;
        if let Ok(desc) = dex.get_type(type_idx) {
            out.push(descriptor_to_java(&desc));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dex_parser::EncodedMethod;

    fn dummy_encoded() -> EncodedMethod {
        EncodedMethod {
            method_idx: 0,
            access_flags: 0,
            code_off: 1,
        }
    }

    fn index_with_overloads() -> MethodIndex {
        let mut idx = MethodIndex::default();
        let class_name = "com.foo.Bar".to_string();
        let method_name = "foo".to_string();
        let key = format!("{class_name}#{method_name}");
        for (i, proto) in ["(I)V", "(Ljava/lang/String;)V"].into_iter().enumerate() {
            let id = MethodId(i);
            idx.by_class_method.entry(key.clone()).or_default().push(id);
            idx.methods.push(MethodRef {
                id,
                class_name: class_name.clone(),
                method_name: method_name.clone(),
                proto: proto.to_string(),
                dex_index: 0,
                encoded: dummy_encoded(),
            });
        }
        idx
    }

    #[test]
    fn resolve_callee_prefers_matching_proto() {
        let idx = index_with_overloads();
        let id = idx
            .resolve_callee("com.foo.Bar.foo(Ljava/lang/String;)V")
            .expect("string overload");
        assert_eq!(id, MethodId(1));
        assert_eq!(idx.get(id).unwrap().proto, "(Ljava/lang/String;)V");
        let id_int = idx
            .resolve_callee("com.foo.Bar.foo(I)V")
            .expect("int overload");
        assert_eq!(id_int, MethodId(0));
    }

    #[test]
    fn resolve_callee_name_only_keeps_first_of_several() {
        let idx = index_with_overloads();
        assert_eq!(idx.resolve_callee("com.foo.Bar.foo"), Some(MethodId(0)));
    }

    #[test]
    fn resolve_callee_single_candidate_ignores_unknown_proto() {
        let mut idx = MethodIndex::default();
        let id = MethodId(0);
        idx.by_class_method
            .insert("com.foo.Only#bar".into(), vec![id]);
        idx.methods.push(MethodRef {
            id,
            class_name: "com.foo.Only".into(),
            method_name: "bar".into(),
            proto: "()V".into(),
            dex_index: 0,
            encoded: dummy_encoded(),
        });
        assert_eq!(
            idx.resolve_callee("com.foo.Only.bar(Ljava/lang/String;)V"),
            Some(MethodId(0))
        );
    }

    #[test]
    fn resolve_callee_unknown_proto_with_several_overloads_is_none() {
        let idx = index_with_overloads();
        assert_eq!(idx.resolve_callee("com.foo.Bar.foo(J)V"), None);
    }

    fn index_base_child() -> MethodIndex {
        let mut idx = MethodIndex::default();
        for (i, class_name) in ["com.foo.Base", "com.foo.Child"].into_iter().enumerate() {
            let id = MethodId(i);
            let method_name = "foo".to_string();
            idx.by_class_method
                .entry(format!("{class_name}#{method_name}"))
                .or_default()
                .push(id);
            idx.methods.push(MethodRef {
                id,
                class_name: class_name.to_string(),
                method_name,
                proto: "()V".into(),
                dex_index: 0,
                encoded: dummy_encoded(),
            });
        }
        idx.subclasses
            .insert("com.foo.Base".into(), vec!["com.foo.Child".into()]);
        idx
    }

    #[test]
    fn resolve_callees_includes_subclass_override() {
        let idx = index_base_child();
        let ids = idx.resolve_callees("com.foo.Base.foo()V");
        assert!(ids.contains(&MethodId(0)), "declared Base: {ids:?}");
        assert!(ids.contains(&MethodId(1)), "Child override: {ids:?}");
        assert_eq!(idx.resolve_callee("com.foo.Base.foo()V"), Some(MethodId(0)));
    }

    #[test]
    fn resolve_callees_skips_library_subclasses() {
        // CHA must not walk android.* children (documented: skip is_library_class).
        let mut idx = MethodIndex::default();
        let id = MethodId(0);
        idx.by_class_method
            .insert("android.view.View#foo".into(), vec![id]);
        idx.methods.push(MethodRef {
            id,
            class_name: "android.view.View".into(),
            method_name: "foo".into(),
            proto: "()V".into(),
            dex_index: 0,
            encoded: dummy_encoded(),
        });
        // Fake a library child + an app child; only the app override is kept.
        idx.subclasses.insert(
            "android.view.View".into(),
            vec!["android.widget.Button".into(), "com.foo.MyView".into()],
        );
        let app_id = MethodId(1);
        idx.by_class_method
            .insert("com.foo.MyView#foo".into(), vec![app_id]);
        idx.methods.push(MethodRef {
            id: app_id,
            class_name: "com.foo.MyView".into(),
            method_name: "foo".into(),
            proto: "()V".into(),
            dex_index: 0,
            encoded: dummy_encoded(),
        });
        let ids = idx.resolve_callees("android.view.View.foo()V");
        assert!(ids.contains(&MethodId(0)));
        assert!(ids.contains(&app_id));
        assert!(ids.len() <= 8);
        assert!(
            !ids.iter().any(|id| idx
                .get(*id)
                .unwrap()
                .class_name
                .starts_with("android.widget")),
            "library subclass must be skipped: {ids:?}"
        );
    }
}
