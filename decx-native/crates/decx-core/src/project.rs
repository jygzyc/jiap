//! Project: loads apk / classes.dex / jar into dexes + apk layer, builds the global index.

use crate::dex::Dex;
use crate::xref::XrefIndex;
use decx_apk::Apk;

pub struct Project {
    pub name: String,
    pub apk: Option<Apk>,
    pub dexes: Vec<Dex>,
    pub dex_names: Vec<String>,
    pub index: XrefIndex,
    /// classes.jar entries (when input is a jar without dex)
    pub class_entries: Vec<String>,
    /// non-dex zip entries (resources listing) for apk or jar/apk
    pub resource_entries: Vec<(String, u32)>,
    /// vendored dexdec decompiler (source recovery), guarded for thread safety.
    /// `None` when the input has no dex (pure class jar) or dexdec failed to load.
    pub decompiler: std::sync::Mutex<Option<decx_engine::Decompiler>>,
}

impl Project {
    /// Take the dexdec lock. Callers must not hold it while touching `self.dexes`.
    pub fn decomp(&self) -> std::sync::MutexGuard<'_, Option<decx_engine::Decompiler>> {
        self.decompiler.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Load .dex directly, or apk/jar (dex entries inside are parsed).
    pub fn load(path: &str, data: Vec<u8>) -> Result<Project, String> {
        let name = std::path::Path::new(path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string());
        let lower = name.to_lowercase();
        if lower.ends_with(".dex") {
            let dex = Dex::parse(data)?;
            let index = XrefIndex::build(std::slice::from_ref(&dex));
            return Ok(Project {
                name: name.clone(),
                apk: None,
                dexes: vec![dex],
                dex_names: vec![name],
                index,
                class_entries: Vec::new(),
                resource_entries: Vec::new(),
                decompiler: Self::open_decompiler(path),
            });
        }
        if lower.ends_with(".apk") || lower.ends_with(".jar") || lower.ends_with(".zip") || lower.ends_with(".aar") {
            let apk = Apk::open(data)?;
            let dex_names: Vec<String> = apk
                .dex_entries()
                .into_iter()
                .map(|s| s.to_string())
                .collect();
            let mut dexes = Vec::new();
            for dn in &dex_names {
                match apk.read_dex(dn) {
                    Some(Ok(bytes)) => match Dex::parse(bytes) {
                        Ok(d) => dexes.push(d),
                        Err(e) => return Err(format!("dex parse failed in {dn}: {e}")),
                    },
                    Some(Err(e)) => return Err(format!("read {dn}: {e}")),
                    None => return Err(format!("{dn} missing")),
                }
            }
            let class_entries: Vec<String> = apk
                .zip
                .entries()
                .iter()
                .filter(|e| e.name.ends_with(".class"))
                .map(|e| e.name.clone())
                .collect();
            let resource_entries: Vec<(String, u32)> = apk
                .zip
                .entries()
                .iter()
                .filter(|e| !e.name.ends_with(".dex") && !e.name.ends_with(".class"))
                .map(|e| (e.name.clone(), e.uncompressed_size as u32))
                .collect();
            let index = XrefIndex::build(&dexes);
            return Ok(Project {
                name,
                apk: Some(apk),
                dexes,
                dex_names,
                index,
                class_entries,
                resource_entries,
                decompiler: Self::open_decompiler(path),
            });
        }
        Err(format!("unsupported input '{path}': need .dex/.apk/.jar/.zip/.aar"))
    }

    /// Open the vendored dexdec decompiler on the same input file.
    /// Only used for source recovery; every failure degrades to structural IR.
    fn open_decompiler(path: &str) -> std::sync::Mutex<Option<decx_engine::Decompiler>> {
        let opened = std::path::Path::new(path).is_file()
            .then(|| decx_engine::Decompiler::open(path).ok())
            .flatten();
        std::sync::Mutex::new(opened)
    }

    /// iterate (class, dex_idx, class_def_idx) over all dexes
    pub fn for_each_class<F: FnMut(usize, usize)>(&self, mut f: F) {
        for (di, d) in self.dexes.iter().enumerate() {
            for ci in 0..d.class_defs.len() {
                f(di, ci);
            }
        }
    }

    pub fn total_classes(&self) -> usize {
        self.dexes.iter().map(|d| d.class_defs.len()).sum()
    }

    pub fn total_methods(&self) -> usize {
        self.dexes.iter().map(|d| d.method_ids.len()).sum()
    }

    /// find class by descriptor "Lcom/a/B;" or dotted "com.a.B"
    pub fn find_class(&self, key: &str) -> Option<(usize, usize)> {
        let desc = if key.starts_with('L') && key.ends_with(';') {
            key.to_string()
        } else {
            format!("L{};", key.replace('.', "/"))
        };
        for (di, d) in self.dexes.iter().enumerate() {
            for (ci, def) in d.class_defs.iter().enumerate() {
                if d.type_descriptor(def.class_idx) == desc {
                    return Some((di, ci));
                }
            }
        }
        // fallback: suffix match (DECX accepts short names)
        for (di, d) in self.dexes.iter().enumerate() {
            for (ci, def) in d.class_defs.iter().enumerate() {
                let full = d.type_descriptor(def.class_idx);
                if full.ends_with(&format!("/{}", desc.trim_start_matches('L'))) {
                    return Some((di, ci));
                }
            }
        }
        None
    }

    /// resolve "Lcls;->name(sig)ret" or looser "name" fragment to dex-local method info
    pub fn find_method(&self, key: &str) -> Option<(usize, u32)> {
        // exact full match first
        for (di, d) in self.dexes.iter().enumerate() {
            for mi in 0..d.method_ids.len() as u32 {
                if d.method_full(mi) == key {
                    return Some((di, mi));
                }
            }
        }
        // owner#name or owner.name match
        let (owner_frag, name_frag) = match key.split_once("->") {
            Some((o, n)) => (o.trim_end_matches(';').to_string(), n.split('(').next().unwrap_or(n).to_string()),
            None => match key.rsplit_once('.') {
                Some((o, n)) => (format!("L{}", o.replace('.', "/")), n.to_string()),
                None => (String::new(), key.to_string()),
            },
        };
        for (di, d) in self.dexes.iter().enumerate() {
            for mi in 0..d.method_ids.len() as u32 {
                let owner = d.method_class(mi);
                let nm = d.method_name(mi);
                if (owner_frag.is_empty() || owner.contains(owner_frag.trim_start_matches('L').replace('.', "/").as_str()))
                    && (name_frag.is_empty() || nm == name_frag)
                {
                    return Some((di, mi));
                }
            }
        }
        None
    }

    /// all dex string pool strings (deduped across dexes, sorted)
    pub fn all_strings(&self) -> Vec<&str> {
        let mut set = std::collections::BTreeSet::new();
        for d in &self.dexes {
            for s in &d.strings {
                set.insert(s.as_str());
            }
        }
        set.into_iter().collect()
    }

    /// subtype closure: direct children (extends or implements) of a class descriptor
    pub fn subclasses_of(&self, desc: &str) -> Vec<String> {
        let mut out = Vec::new();
        for d in &self.dexes {
            for def in &d.class_defs {
                let super_is = d.type_descriptor(def.superclass_idx) == desc;
                let ifaces_is = d.interfaces(def).iter().any(|i| *i == desc);
                if super_is || ifaces_is {
                    out.push(d.type_descriptor(def.class_idx).to_string());
                }
            }
        }
        out.sort();
        out
    }

    /// classes implementing an interface descriptor (direct or via super chain)
    pub fn implementors_of(&self, iface_desc: &str) -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        let mut queue: Vec<String> = vec![iface_desc.to_string()];
        while let Some(cur) = queue.pop() {
            for d in &self.dexes {
                for def in &d.class_defs {
                    let cls = d.type_descriptor(def.class_idx).to_string();
                    let hits = d.interfaces(def).iter().any(|i| *i == cur)
                        || d.type_descriptor(def.superclass_idx) == cur && cur != iface_desc;
                    if hits && !seen.contains(&cls) {
                        seen.push(cls.clone());
                        // a class implementing an interface makes its subclasses impls too
                        queue.push(cls);
                    }
                }
            }
        }
        seen.sort();
        seen
    }
}
