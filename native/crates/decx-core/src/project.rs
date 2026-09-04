//! Project state — fused on the dexdec engine.
//!
//! dexdec is not an external helper here: it IS the core. The class index is
//! built from its [`dexdec::api::ArchiveCatalog`], members come from its
//! [`dexdec::api::ClassOutline`], Java/smali-ish output from its decompiler
//! and IR visualizer, and cross references from its own reference scanner.
//! Everything else in this crate is glue around that.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dexdec::api::{
    ClassSelector, DecompileOptions as DexDecompileOptions, Decompiler as DexDecompiler,
    DecompilerContext, ReferenceTarget, SourceLanguage as DexSourceLanguage,
};

use crate::error::{DecxError, Result};

/// One indexed class (metadata only — members load on demand through the
/// engine's `class_outline`).
#[derive(Debug, Clone)]
pub struct ClassEntry {
    /// `Lcom/foo/Bar;`
    pub descriptor: String,
    /// `com.foo.Bar`
    pub java_name: String,
    pub package: String,
    /// True when the class is a nested/inner class of another class.
    pub nested: bool,
}

/// Byte-bounded LRU cache of decompiled class sources (≈ `BoundedCodeCache`).
pub struct SourceCache {
    max_bytes: usize,
    cur_bytes: usize,
    tick: u64,
    map: HashMap<String, (Arc<String>, u64)>,
}

/// Override the cache budget in bytes (default 1 GiB).
pub const DEFAULT_CACHE_MAX_BYTES: usize = 1024 * 1024 * 1024;

impl SourceCache {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            cur_bytes: 0,
            tick: 0,
            map: HashMap::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<Arc<String>> {
        let entry = self.map.get_mut(key)?;
        self.tick += 1;
        entry.1 = self.tick;
        Some(Arc::clone(&entry.0))
    }

    pub fn insert(&mut self, key: String, value: Arc<String>) {
        let bytes = value.len();
        if bytes > self.max_bytes {
            return; // single source larger than the whole budget: don't cache
        }
        self.tick += 1;
        self.cur_bytes += bytes;
        self.map.insert(key, (value, self.tick));
        while self.cur_bytes > self.max_bytes {
            let victim = self
                .map
                .iter()
                .min_by_key(|(_, (_, t))| *t)
                .map(|(k, _)| k.clone());
            match victim {
                Some(k) => {
                    if let Some((v, _)) = self.map.remove(&k) {
                        self.cur_bytes -= v.len();
                    }
                }
                None => break,
            }
        }
    }

    pub fn len_bytes(&self) -> usize {
        self.cur_bytes
    }
}

/// Lazily built class-hierarchy table: java name → (superclass, interfaces).
type Hierarchy = HashMap<String, (Option<String>, Vec<String>)>;

/// A loaded analysis target, fused on the dexdec engine.
pub struct Project {
    pub path: PathBuf,
    entries: Vec<ClassEntry>,
    by_java_name: HashMap<String, usize>,
    cache: std::sync::Mutex<SourceCache>,
    /// Java emitter + batch pipeline (`&mut self` API → mutex-serialized).
    engine: std::sync::Mutex<DexDecompiler>,
    /// Second dexdec context dedicated to IR/CFG decoding (`decode_method`
    /// lives on the context, and `Decompiler` does not expose it).
    ir_context: std::sync::Mutex<DecompilerContext>,
    hierarchy: std::sync::Mutex<Option<Arc<Hierarchy>>>,
}

impl Project {
    /// Open a `.dex`/`.apk`/`.zip` target and build the class index.
    pub fn open(path: &Path) -> Result<Self> {
        let mut engine = DexDecompiler::open(path)
            .map_err(|e| DecxError::invalid_parameter(format!("load {}: {e}", path.display())))?;
        engine.set_options(
            DexDecompileOptions::default()
                .with_language(DexSourceLanguage::Java)
                .with_isolated_requests(false),
        );

        let mut entries = Vec::new();
        let mut by_java_name = HashMap::new();
        for summary in engine.catalog().classes() {
            let entry = ClassEntry {
                descriptor: summary.descriptor.clone(),
                java_name: summary.qualified_name.clone(),
                package: summary.package.clone(),
                nested: summary.parent_descriptor.is_some(),
            };
            by_java_name.insert(entry.java_name.clone(), entries.len());
            entries.push(entry);
        }

        let cache_max = std::env::var("DECX_NATIVE_CACHE_MAX_BYTES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_CACHE_MAX_BYTES);
        let ir_context = DecompilerContext::from_file(path)
            .map_err(|e| DecxError::internal(format!("dexdec IR context: {e}")))?;
        Ok(Self {
            path: path.to_path_buf(),
            entries,
            by_java_name,
            cache: std::sync::Mutex::new(SourceCache::new(cache_max)),
            engine: std::sync::Mutex::new(engine),
            ir_context: std::sync::Mutex::new(ir_context),
            hierarchy: std::sync::Mutex::new(None),
        })
    }

    pub fn entries(&self) -> &[ClassEntry] {
        &self.entries
    }

    pub fn lookup(&self, java_name: &str) -> Option<&ClassEntry> {
        self.by_java_name.get(java_name).map(|&i| &self.entries[i])
    }

    /// Accept both java (`com.foo.Bar`) and dex (`Lcom/foo/Bar;`) spellings.
    pub fn lookup_fuzzy(&self, name: &str) -> Option<&ClassEntry> {
        let trimmed = name.trim();
        if let Some(e) = self.lookup(trimmed) {
            return Some(e);
        }
        let java = if trimmed.starts_with('L') && trimmed.ends_with(';') {
            crate::names::descriptor_to_java(trimmed)
        } else {
            trimmed.to_string()
        };
        self.lookup(&java)
    }

    pub fn cache_len_bytes(&self) -> usize {
        self.cache.lock().map(|c| c.len_bytes()).unwrap_or(0)
    }

    fn engine_lock(&self) -> Result<std::sync::MutexGuard<'_, DexDecompiler>> {
        self.engine
            .lock()
            .map_err(|_| DecxError::internal("dexdec engine lock poisoned"))
    }

    /// Narrow to the top-level owner descriptor of a possibly-nested class:
    /// nested classes are rendered inline with their owner.
    fn owner_descriptor(&self, entry: &ClassEntry) -> String {
        let owner_java = entry.java_name.split('$').next().unwrap_or(&entry.java_name);
        if owner_java == entry.java_name {
            entry.descriptor.clone()
        } else {
            crate::names::java_to_descriptor(owner_java)
        }
    }

    /// Decompile one class to Java, served through the LRU cache.
    pub fn class_source(&self, entry: &ClassEntry) -> Result<Arc<String>> {
        // Nested classes are rendered inline with their owner: a cache hit on
        // the owner serves any `$`-member without re-rendering.
        let owner = self.owner_descriptor(entry);
        let owner_java = crate::names::descriptor_to_java(&owner);
        if let Some(hit) = self
            .cache
            .lock()
            .ok()
            .and_then(|mut c| c.get(&owner_java).or_else(|| c.get(&entry.java_name)))
        {
            return Ok(hit);
        }
        let source = {
            let mut engine = self.engine_lock()?;
            engine
                .class(owner.clone())
                .map_err(|e| DecxError::new("DECOMPILATION_SKIPPED", format!("{e}")))?
                .source
        };
        let arc = Arc::new(source);
        if let Ok(mut c) = self.cache.lock() {
            c.insert(owner_java, Arc::clone(&arc));
            c.insert(entry.java_name.clone(), Arc::clone(&arc));
        }
        Ok(arc)
    }

    /// Class declaration (members, hierarchy, flags) without method bodies.
    pub fn class_outline(
        &self,
        entry: &ClassEntry,
    ) -> Result<Arc<dexdec::api::ClassOutline>> {
        let owner = self.owner_descriptor(entry);
        let mut engine = self.engine_lock()?;
        let outline = engine
            .class_outline(owner)
            .map_err(|e| DecxError::class_not_found(format!("{e}")))?;
        Ok(Arc::new(outline))
    }

    /// Decompile a single method to Java.
    pub fn method_source(&self, entry: &ClassEntry, method_name: &str) -> Result<String> {
        let owner = self.owner_descriptor(entry);
        let mut engine = self.engine_lock()?;
        let descriptor = engine
            .class_outline(owner.clone())
            .ok()
            .and_then(|o| {
                o.methods
                    .iter()
                    .find(|m| m.name == method_name)
                    .map(|m| m.descriptor.clone())
            });
        let mut request = dexdec::api::MethodRequest::new(owner, method_name);
        if let Some(d) = descriptor {
            request = request.with_descriptor(d);
        }
        let out = engine
            .method(request)
            .map_err(|_| DecxError::method_not_found(format!("{}.{method_name}", entry.java_name)))?;
        Ok(out.source.unwrap_or_else(|| {
            format!("// {method_name} is abstract or native (no bytecode body)")
        }))
    }

    /// Semantic IR listing of one method (the dexdec counterpart of a smali
    /// body dump: blocks with structured instructions and branch kinds).
    pub fn method_ir_text(&self, entry: &ClassEntry, method_name: &str) -> Result<String> {
        let owner = self.owner_descriptor(entry);
        let descriptor = {
            let mut engine = self.engine_lock()?;
            engine
                .class_outline(owner.clone())
                .ok()
                .and_then(|o| {
                    o.methods
                        .iter()
                        .find(|m| m.name == method_name)
                        .map(|m| m.descriptor.clone())
                })
        };
        let mut ir = self
            .ir_context
            .lock()
            .map_err(|_| DecxError::internal("dexdec IR context lock poisoned"))?;
        // decode_method reads the class from the context reader: load it first.
        let _loaded = ir
            .load_class(&owner)
            .map_err(|e| DecxError::internal(e.to_string()))?;
        let cfg = ir
            .decode_method(&owner, method_name, descriptor.as_deref())
            .map_err(|e| DecxError::method_not_found(format!("{}.{method_name}: {e}", entry.java_name)))?
            .ok_or_else(|| {
                DecxError::new(
                    "DECOMPILATION_SKIPPED",
                    format!("{method_name} has no bytecode body"),
                )
            })?;
        Ok(dexdec::visualizer::method_to_text(cfg))
    }

    /// Whole-class IR listing: one decoded method body after another. This is
    /// the `--smali` rendering (dexdec IR blocks instead of raw dalvik text).
    pub fn class_ir_listing(&self, entry: &ClassEntry) -> Result<String> {
        let outline = self.class_outline(entry)?;
        let mut out = String::new();
        for m in &outline.methods {
            if !m.has_code {
                continue;
            }
            out.push_str(&format!("## {}{}\n", m.name, m.descriptor));
            match self.method_ir_text(entry, &m.name) {
                Ok(text) => {
                    out.push_str(&text);
                    out.push('\n');
                }
                Err(_) => out.push_str("  (no decodable body)\n"),
            }
        }
        if out.is_empty() {
            out.push_str("// class has no method bodies\n");
        }
        Ok(out)
    }

    /// Control-flow graph of one method as (nodes, edges) JSON fragments.
    /// Node: `{id, offset, insns}`; edge: `{from, to, kind}`.
    pub fn method_cfg(
        &self,
        entry: &ClassEntry,
        method_name: &str,
    ) -> Result<(Vec<serde_json::Value>, Vec<serde_json::Value>, String)> {
        let owner = self.owner_descriptor(entry);
        let descriptor = {
            let mut engine = self.engine_lock()?;
            engine
                .class_outline(owner.clone())
                .ok()
                .and_then(|o| {
                    o.methods
                        .iter()
                        .find(|m| m.name == method_name)
                        .map(|m| m.descriptor.clone())
                })
        };
        let mut ir = self
            .ir_context
            .lock()
            .map_err(|_| DecxError::internal("dexdec IR context lock poisoned"))?;
        let _loaded = ir
            .load_class(&owner)
            .map_err(|e| DecxError::internal(e.to_string()))?;
        let cfg = ir
            .decode_method(&owner, method_name, descriptor.as_deref())
            .map_err(|e| DecxError::method_not_found(format!("{}.{method_name}: {e}", entry.java_name)))?
            .ok_or_else(|| {
                DecxError::new(
                    "DECOMPILATION_SKIPPED",
                    format!("{method_name} has no bytecode body"),
                )
            })?;
        let mut nodes = Vec::new();
        for block in cfg.blocks_iter() {
            nodes.push(serde_json::json!({
                "id": block.id.0,
                "offset": block.offset,
                "insns": block.insns.len(),
                "synthetic": block.synthetic,
            }));
        }
        let mut edges = Vec::new();
        for block in cfg.blocks_iter() {
            for (target, kind) in cfg.successors_with_kind(block.id) {
                edges.push(serde_json::json!({
                    "from": block.id.0,
                    "to": target.0,
                    "kind": format!("{kind:?}"),
                }));
            }
        }
        let text = dexdec::visualizer::method_to_text(cfg);
        Ok((nodes, edges, text))
    }

    /// Bytecode sites referencing the target, via dexdec's reference scanner.
    pub fn references(
        &self,
        target: ReferenceTarget,
    ) -> Result<Vec<dexdec::api::ReferenceLocation>> {
        let mut engine = self.engine_lock()?;
        let results = engine
            .references(target)
            .map_err(|e| DecxError::internal(format!("reference scan: {e}")))?;
        Ok(results.locations)
    }

    /// Method/field declarations across the archive (metadata pass, no IR).
    pub fn members(
        &self,
    ) -> Result<Vec<dexdec::api::MemberSummary>> {
        let engine = self.engine_lock()?;
        let catalog = engine
            .member_catalog()
            .map_err(|e| DecxError::internal(format!("member catalog: {e}")))?;
        Ok(catalog.into_members())
    }

    /// Lazily built hierarchy table: java name → (superclass java, interfaces java).
    pub fn hierarchy(&self) -> Result<Arc<Hierarchy>> {
        if let Some(h) = self.hierarchy.lock().ok().and_then(|h| h.clone()) {
            return Ok(h);
        }
        let map: Hierarchy = {
            let mut engine = self.engine_lock()?;
            let mut map = HashMap::with_capacity(self.entries.len());
            for entry in &self.entries {
                let Ok(outline) = engine.class_outline(entry.descriptor.clone()) else {
                    continue;
                };
                map.insert(
                    entry.java_name.clone(),
                    (
                        outline
                            .super_class
                            .as_deref()
                            .map(crate::names::descriptor_to_java),
                        outline
                            .interfaces
                            .iter()
                            .map(|i| crate::names::descriptor_to_java(i))
                            .collect(),
                    ),
                );
            }
            map
        };
        let arc = Arc::new(map);
        if let Ok(mut slot) = self.hierarchy.lock() {
            *slot = Some(Arc::clone(&arc));
        }
        Ok(arc)
    }

    /// Decompile many classes through the dexdec batch pipeline (shared
    /// analysis: one archive-level pass with prefetch and cross-class type
    /// recovery). Sources land in the LRU cache; every per-class failure is
    /// isolated and reported, never aborting the sweep.
    /// Returns (ok_count, failures).
    pub fn decompile_batch(&self, indices: &[usize]) -> (usize, Vec<String>) {
        // Batch in top-level units: nested classes render inline with their
        // owner, so one owner covers all of its `$`-members.
        let mut owners: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for &i in indices {
            let entry = &self.entries[i];
            let owner_java = entry.java_name.split('$').next().unwrap_or(&entry.java_name);
            owners.insert(crate::names::java_to_descriptor(owner_java));
        }

        let mut ok = 0usize;
        let mut failures = Vec::new();
        let Ok(mut engine) = self.engine.lock() else {
            failures.push("dexdec engine lock poisoned".to_string());
            return (0, failures);
        };
        if let Ok(batch) = engine.classes(ClassSelector::Listed(owners)) {
            for unit in batch {
                match unit {
                    Ok(u) => {
                        let java = crate::names::descriptor_to_java(&u.class);
                        if let Ok(mut c) = self.cache.lock() {
                            c.insert(java, Arc::new(u.source));
                        }
                        ok += 1;
                    }
                    Err(f) => {
                        let java = crate::names::descriptor_to_java(&f.class);
                        failures.push(format!("{java}: {}", f.error));
                    }
                }
            }
        }
        // release batch-analysis memory after the sweep
        engine.clear_analysis_scope();
        (ok, failures)
    }

    /// Decompile every class to warm the cache.
    pub fn warm_all(&self) -> (usize, Vec<String>) {
        let all: Vec<usize> = (0..self.entries.len()).collect();
        self.decompile_batch(&all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dex() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/classes.dex")
    }

    fn small_project() -> Project {
        Project::open(&test_dex()).expect("open testdata dex")
    }

    #[test]
    fn indexes_all_classes() {
        let p = small_project();
        assert_eq!(p.entries().len(), 5920);
        assert!(p.lookup("android.support.v4.app.INotificationSideChannel").is_some());
    }

    #[test]
    fn decompiles_first_class() {
        let p = small_project();
        let entry = p.lookup("android.support.v4.app.INotificationSideChannel").unwrap();
        let src = p.class_source(entry).expect("decompile");
        assert!(src.contains("interface"), "got: {}", &src[..200.min(src.len())]);
    }

    #[test]
    fn outline_exposes_members() {
        let p = small_project();
        let entry = p.lookup("android.support.v4.app.INotificationSideChannel").unwrap();
        let outline = p.class_outline(entry).expect("outline");
        assert!(!outline.methods.is_empty());
        assert!(outline
            .methods
            .iter()
            .any(|m| m.name == "cancelNotification" || !m.name.is_empty()));
    }
}

/// dexdec engine benchmark. Run with:
/// cargo test -p decx-core --release dexdec_bench -- --ignored --nocapture
#[cfg(test)]
mod dexdec_bench {
    use dexdec::api::{ClassSelector, DecompileOptions, Decompiler, SourceLanguage};

    fn test_dex() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/classes.dex")
    }

    #[test]
    #[ignore]
    fn dexdec_bench() {
        // 1) interactive single-class requests (lazy, isolated)
        let t = std::time::Instant::now();
        let mut d = Decompiler::open(test_dex()).expect("open");
        println!("open+index: {:.1}ms, classes={}", t.elapsed().as_millis(), d.catalog().len());

        for cls in [
            "Landroidx/activity/result/ActivityResultRegistry;",
            "Landroidx/activity/OnBackPressedDispatcher;",
            "Landroid/support/v4/app/INotificationSideChannel;",
            "Landroidx/annotation/AnimRes;",
        ] {
            let t = std::time::Instant::now();
            let r = d.class(cls);
            match r {
                Ok(unit) => println!(
                    "class {:>55}: {:>6.1}ms  {} lines",
                    cls,
                    t.elapsed().as_millis(),
                    unit.source.lines().count()
                ),
                Err(e) => println!("class {cls}: FAILED in {:?}: {e}", t.elapsed()),
            }
        }

        // 2) full-archive batch (shared analysis, Java)
        let mut d = Decompiler::open(test_dex()).expect("open");
        d.set_options(
            DecompileOptions::default()
                .with_language(SourceLanguage::Java)
                .with_isolated_requests(false),
        );
        let t0 = std::time::Instant::now();
        let mut ok = 0usize;
        let mut failed = 0usize;
        let batch = d.classes(ClassSelector::All).expect("select");
        let total = batch.len();
        for (i, unit) in batch.enumerate() {
            match unit {
                Ok(u) => {
                    ok += 1;
                    let _ = u.source.len();
                }
                Err(f) => {
                    failed += 1;
                    if failed <= 5 {
                        println!("FAIL {}: {}", f.class, f.error);
                    }
                }
            }
            if i % 1000 == 999 {
                println!("[{}/{}] elapsed {:.1}s", i + 1, total, t0.elapsed().as_secs_f64());
                use std::io::Write;
                std::io::stdout().flush().ok();
            }
        }
        println!(
            "dexdec batch: {ok}/{total} ok, {failed} failed, total {:.1}s ({:.1}ms/class avg)",
            t0.elapsed().as_secs_f64(),
            t0.elapsed().as_millis() as f64 / total.max(1) as f64
        );
    }
}
