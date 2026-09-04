//! Project state: loaded dexes (+ optional apk manifest), a full class index,
//! and lazy per-class decompilation behind a byte-bounded LRU cache.
//!
//! This mirrors the Kotlin server's "JADX decompiler state + DecompileGuard"
//! role: opening a project is cheap (index only), sources are produced on
//! demand, and the cache caps resident decompiled bytes so huge apps stay
//! bounded.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dex_decompiler::input::load_dexes_from_path;
use dex_parser::{ClassDef, DexFile};
use dexdec::api::{
    ClassSelector, DecompileOptions as DexDecompileOptions, Decompiler as DexDecompiler,
    SourceLanguage as DexSourceLanguage,
};

use crate::error::{DecxError, Result};
use crate::names::descriptor_to_java;

/// One indexed field of a class.
#[derive(Debug, Clone)]
pub struct FieldEntry {
    pub name: String,
    pub field_idx: u32,
    pub access_flags: u32,
    /// Field type descriptor, e.g. `Ljava/lang/String;`.
    pub type_descriptor: String,
}

/// One indexed method of a class. `idx_in_class` follows the engine xref
/// convention: direct methods first, then virtual methods.
#[derive(Debug, Clone)]
pub struct MethodEntry {
    pub name: String,
    pub method_idx: u32,
    pub idx_in_class: usize,
    pub direct: bool,
    pub access_flags: u32,
    pub code_off: u32,
    pub return_descriptor: String,
    pub param_descriptors: Vec<String>,
}

impl MethodEntry {
    /// `V(Landroid/os/Bundle;)`-style short descriptor.
    pub fn short_descriptor(&self) -> String {
        format!("({})", self.param_descriptors.join(""))
    }
}

/// One indexed class across all dexes of the project.
#[derive(Debug, Clone)]
pub struct ClassEntry {
    pub dex_idx: usize,
    pub class_def_idx: usize,
    /// `Lcom/foo/Bar;`
    pub dex_name: String,
    /// `com.foo.Bar`
    pub java_name: String,
    pub access_flags: u32,
    pub superclass_java: Option<String>,
    pub interfaces_java: Vec<String>,
    pub methods: Vec<MethodEntry>,
    pub fields: Vec<FieldEntry>,
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
            // evict least-recently-used
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

/// A loaded analysis target: one or more dexes (from a `.dex` file or an
/// `.apk`/`.zip` archive) plus the derived class index.
///
/// Two engines cooperate:
/// - **dexdec** produces all Java source (interactive + batch), with the
///   platform-symbol hierarchy analysis enabled;
/// - the androguard stack provides the class index plus bytecode-level
///   utilities (smali listing, method CFG, call/field xrefs).
pub struct Project {
    pub path: PathBuf,
    pub dexes: Vec<DexFile>,
    /// Raw `AndroidManifest.xml` bytes when the input was an APK.
    pub manifest_raw: Option<Vec<u8>>,
    entries: Vec<ClassEntry>,
    by_java_name: HashMap<String, usize>,
    cache: std::sync::Mutex<SourceCache>,
    /// Java emitter. `&mut self` API → serialized behind a mutex; the LRU
    /// source cache in front keeps repeated requests cheap.
    dexdec_engine: std::sync::Mutex<DexDecompiler>,
}

impl Project {
    /// Open a `.dex`/`.apk`/`.zip` target and build the class index.
    pub fn open(path: &Path) -> Result<Self> {
        let dexes = load_dexes_from_path(path)
            .map_err(|e| DecxError::invalid_parameter(format!("load {}: {e}", path.display())))?;
        Self::from_dexes(path, dexes)
    }

    pub fn from_dexes(path: &Path, dexes: Vec<DexFile>) -> Result<Self> {
        if dexes.is_empty() {
            return Err(DecxError::invalid_parameter("no dex content in target"));
        }
        let mut entries = Vec::new();
        let mut by_java_name = HashMap::new();
        for (dex_idx, dex) in dexes.iter().enumerate() {
            for cdef_idx in 0..dex.header.class_defs_size {
                let cdef = match dex.get_class_def(cdef_idx) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let Ok(dex_name) = dex.get_type(cdef.class_idx) else {
                    continue;
                };
                let java_name = descriptor_to_java(&dex_name);
                let superclass_java = if cdef.superclass_idx == dex_parser::NO_INDEX {
                    None
                } else {
                    dex.get_type(cdef.superclass_idx).ok().map(|d| descriptor_to_java(&d))
                };
                let interfaces_java = read_interfaces(dex, &cdef)
                    .into_iter()
                    .map(|d| descriptor_to_java(&d))
                    .collect();
                let (methods, fields) = read_members(dex, &cdef);
                let entry = ClassEntry {
                    dex_idx,
                    class_def_idx: cdef_idx as usize,
                    dex_name,
                    java_name: java_name.clone(),
                    access_flags: cdef.access_flags,
                    superclass_java,
                    interfaces_java,
                    methods,
                    fields,
                };
                by_java_name.insert(java_name, entries.len());
                entries.push(entry);
            }
        }
        let cache_max = std::env::var("DECX_NATIVE_CACHE_MAX_BYTES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_CACHE_MAX_BYTES);
        // Java emitter: dexdec with the shared-analysis batch mode. Requests
        // run against one context so cross-class type recovery works; large
        // sweeps clear the scope when they finish to release memory.
        let mut engine = DexDecompiler::open(path)
            .map_err(|e| DecxError::internal(format!("dexdec load: {e}")))?;
        engine.set_options(
            DexDecompileOptions::default()
                .with_language(DexSourceLanguage::Java)
                .with_isolated_requests(false),
        );
        Ok(Self {
            path: path.to_path_buf(),
            dexes,
            manifest_raw: None,
            entries,
            by_java_name,
            cache: std::sync::Mutex::new(SourceCache::new(cache_max)),
            dexdec_engine: std::sync::Mutex::new(engine),
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

    /// Decompile one class to Java via the dexdec engine, served through the
    /// LRU cache. Nested classes are rendered inline by their parent, so
    /// lookups for `$`-members fall back to their enclosing top-level class.
    pub fn class_source(&self, entry: &ClassEntry) -> Result<Arc<String>> {
        if let Some(hit) = self.cache.lock().ok().and_then(|mut c| c.get(&entry.java_name)) {
            return Ok(hit);
        }
        // Nested classes (`Outer$Inner`) decompile as part of `Outer`.
        let owner_java = entry.java_name.split('$').next().unwrap_or(&entry.java_name);
        let owner_dex = crate::names::java_to_descriptor(owner_java);
        let source = {
            let mut engine = self
                .dexdec_engine
                .lock()
                .map_err(|_| DecxError::internal("dexdec engine lock poisoned"))?;
            engine
                .class(owner_dex)
                .map_err(|e| DecxError::new("DECOMPILATION_SKIPPED", format!("{e}")))?
                .source
        };
        let arc = Arc::new(source);
        if let Ok(mut c) = self.cache.lock() {
            // cache under both the requested name and the owner name
            c.insert(owner_java.to_string(), Arc::clone(&arc));
            c.insert(entry.java_name.clone(), Arc::clone(&arc));
        }
        Ok(arc)
    }

    /// Decompile a single method to Java via dexdec.
    pub fn method_source(&self, entry: &ClassEntry, method_name: &str) -> Result<String> {
        // Methods live on the class that declares them.
        let owner_java = entry.java_name.split('$').next().unwrap_or(&entry.java_name);
        let owner_dex = crate::names::java_to_descriptor(owner_java);
        let mut engine = self
            .dexdec_engine
            .lock()
            .map_err(|_| DecxError::internal("dexdec engine lock poisoned"))?;
        let out = engine
            .method(dexdec::api::MethodRequest::new(
                owner_dex,
                method_name,
            ))
            .map_err(|e| DecxError::method_not_found(format!("{}.{method_name}", entry.java_name)))?;
        Ok(out.source.unwrap_or_else(|| {
            format!("// {} is abstract or native (no bytecode body)", method_name)
        }))
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
        let Ok(mut engine) = self.dexdec_engine.lock() else {
            failures.push("dexdec engine lock poisoned".to_string());
            return (0, failures);
        };
        if let Ok(batch) = engine.classes(ClassSelector::Listed(owners)) {
            for unit in batch {
                match unit {
                    Ok(u) => {
                        let java = descriptor_to_java(&u.class);
                        if let Ok(mut c) = self.cache.lock() {
                            c.insert(java, Arc::new(u.source));
                        }
                        ok += 1;
                    }
                    Err(f) => {
                        let java = descriptor_to_java(&f.class);
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
    /// Returns the number of classes processed. Failures are isolated and
    /// reported per class — one broken class never aborts the sweep.
    pub fn warm_all(&self) -> (usize, Vec<String>) {
        let all: Vec<usize> = (0..self.entries.len()).collect();
        self.decompile_batch(&all)
    }
}

/// Interfaces of a class_def, as dex type descriptors.
fn read_interfaces(dex: &DexFile, cdef: &ClassDef) -> Vec<String> {
    let mut out = Vec::new();
    if cdef.interfaces_off == 0 || cdef.interfaces_off == dex_parser::NO_INDEX {
        return out;
    }
    let data = &*dex.data;
    let off = cdef.interfaces_off as usize;
    if data.len() < off + 4 {
        return out;
    }
    let size = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]) as usize;
    for i in 0..size {
        let p = off + 4 + i * 2;
        if p + 2 > data.len() {
            break;
        }
        let idx = u16::from_le_bytes([data[p], data[p + 1]]) as u32;
        if let Ok(t) = dex.get_type(idx) {
            out.push(t);
        }
    }
    out
}

/// Methods + fields of a class, direct first (engine xref convention).
fn read_members(dex: &DexFile, cdef: &ClassDef) -> (Vec<MethodEntry>, Vec<FieldEntry>) {
    let mut methods = Vec::new();
    let mut fields = Vec::new();
    let Ok(Some(class_data)) = dex.get_class_data(cdef) else {
        return (methods, fields);
    };
    let mut idx_in_class = 0usize;
    for (direct, list) in [(true, &class_data.direct_methods), (false, &class_data.virtual_methods)] {
        for m in list {
            if let Ok(info) = dex.get_method_info(m.method_idx) {
                methods.push(MethodEntry {
                    name: info.name,
                    method_idx: m.method_idx,
                    idx_in_class: idx_in_class,
                    direct,
                    access_flags: m.access_flags,
                    code_off: m.code_off,
                    return_descriptor: info.return_type,
                    param_descriptors: info.params,
                });
            }
            idx_in_class += 1;
        }
    }
    for f in class_data.static_fields.iter().chain(class_data.instance_fields.iter()) {
        if let Ok(info) = dex.get_field_info(f.field_idx) {
            fields.push(FieldEntry {
                name: info.name,
                field_idx: f.field_idx,
                access_flags: f.access_flags,
                type_descriptor: info.typ,
            });
        }
    }
    (methods, fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dex_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor/dex-decompiler/testdata/classes.dex")
    }

    fn small_project() -> Project {
        let path = test_dex_path();
        let dexes = load_dexes_from_path(&path).expect("load testdata dex");
        Project::from_dexes(&path, dexes).expect("project")
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
        assert!(src.contains("interface INotificationSideChannel") || src.contains("interface"), "got: {}", &src[..200.min(src.len())]);
    }

    /// Find pathological classes + measure per-class cost. Run with:
    /// cargo test -p decx-core --release warm_bench -- --ignored --nocapture
    #[test]
    #[ignore]
    fn warm_bench() {
        let p = small_project();
        use dex_decompiler::{DecompilationMode, Decompiler, DecompilerOptions};
        let dex = &p.dexes[0];
        let n = 30usize;

        for (label, mode) in [("Restructure", DecompilationMode::Restructure), ("Simple", DecompilationMode::Simple), ("Fallback", DecompilationMode::Fallback)] {
            let d = Decompiler::with_options(
                dex,
                DecompilerOptions { mode, ..Default::default() },
            );
            let t0 = std::time::Instant::now();
            let mut ok = 0;
            for e in p.entries().iter().take(n) {
                let Ok(class_def) = dex.get_class_def(e.class_def_idx as u32) else { continue };
                if d.decompile_class(&class_def).is_ok() {
                    ok += 1;
                }
            }
            println!(
                "mode={label}: {ok}/{n} classes in {:.2}s ({:.1}ms/class)",
                t0.elapsed().as_secs_f64(),
                t0.elapsed().as_millis() as f64 / n as f64
            );
        }

        // micro-step: smallest class, timed stage by stage
        let (smallest_idx, _) = p
            .entries()
            .iter()
            .enumerate()
            .min_by_key(|(_, e)| e.methods.len())
            .unwrap();
        let e = &p.entries()[smallest_idx];
        println!(
            "smallest class: {} with {} methods",
            e.java_name,
            e.methods.len()
        );
        let cdef = dex.get_class_def(e.class_def_idx as u32).unwrap();
        let t = std::time::Instant::now();
        let cdata = dex.get_class_data(&cdef).unwrap();
        println!("  get_class_data: {:?}", t.elapsed());
        let t = std::time::Instant::now();
        let d1 = Decompiler::with_options(dex, DecompilerOptions::default());
        println!("  constructor: {:?}", t.elapsed());
        if let Some(m) = cdata.as_ref().and_then(|c| c.virtual_methods.first().or(c.direct_methods.first())) {
            let t = std::time::Instant::now();
            let r = d1.decompile_method(m, Some(e.java_name.as_str()), Some(&e.java_name));
            println!("  decompile_method (first): {:?} ok={}", t.elapsed(), r.is_ok());
            let t = std::time::Instant::now();
            let r = d1.decompile_method(m, Some(e.java_name.as_str()), Some(&e.java_name));
            println!("  decompile_method (cached 2nd): {:?} ok={}", t.elapsed(), r.is_ok());
        }
        let t = std::time::Instant::now();
        let r = d1.decompile_class(&cdef);
        println!("  decompile_class: {:?} ok={}", t.elapsed(), r.is_ok());
    }

    /// Full batch scan (engine-resilient). Ignored by default; used to
    /// enumerate per-class failures on a full dex:
    /// cargo test -p decx-core --release batch_all -- --ignored --nocapture
    #[test]
    #[ignore]
    fn batch_all() {
        let p = small_project();
        let all: Vec<usize> = (0..p.entries().len()).collect();
        let (ok, failed) = p.decompile_batch(&all);
        println!("ok={ok} failed={}", failed.len());
        for f in failed.iter().take(20) {
            println!("FAIL {f}");
        }
    }
}

/// dexdec engine benchmark — compares against the androguard numbers in
/// native/RESEARCH.md. Run with:
/// cargo test -p decx-core --release dexdec_bench -- --ignored --nocapture
#[cfg(test)]
mod dexdec_bench {
    use dexdec::api::{ClassSelector, DecompileOptions, Decompiler, SourceLanguage};

    fn test_dex() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../vendor/dex-decompiler/testdata/classes.dex")
    }

    #[test]
    #[ignore]
    fn dexdec_bench() {
        // 1) interactive single-class requests (lazy, isolated)
        let t = std::time::Instant::now();
        let mut d = Decompiler::open(test_dex()).expect("open");
        println!("open+index: {:.1}ms, classes={}", t.elapsed().as_millis(), d.catalog().len());

        // dexdec expects Lcom/foo/Bar; descriptors
        for cls in [
            "Landroidx/activity/result/ActivityResultRegistry;", // androguard: 31s
            "Landroidx/activity/OnBackPressedDispatcher;",       // androguard: 5.2s
            "Landroid/support/v4/app/INotificationSideChannel;", // androguard: 2.5s
            "Landroidx/annotation/AnimRes;",                     // trivial
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
