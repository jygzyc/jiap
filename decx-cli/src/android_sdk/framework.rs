//! Android framework collection/processing pipeline — port of the TypeScript
//! `decx-cli/src/android/framework*.ts` modules.
//!
//! Stages (each a local command under `decx java android framework ...`):
//! - `collect` — tiered device pull: ready-made files from the framework dirs
//!   plus the runtime `/apex` mount, then `.apex`/`.capex` images from
//!   `/system/apex` only for modules the mount did not already cover.
//! - `process` — extract `.dex` from jars/apks, unpack APEX payload images
//!   (native ext4 + EROFS readers in [`crate::android_sdk::ext4`] /
//!   [`crate::android_sdk::erofs`]; unsupported image features fall back to
//!   external debugfs/erofs-utils from PATH — Linux/macOS only, no native
//!   Windows binaries), and namespace outputs by APEX module.
//! - `pack` — pack processed outputs into one `framework_<oem>_<vendor>.jar`.
//! - `run` — collect + process + pack. Opening the jar is NOT part of this
//!   pipeline anymore: `decx session open <framework.jar>` (the old
//!   `framework open` command was removed for exactly that reason).
//!
//! Build metadata lives in `.artifact.json` per output directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::adb::AdbClient;
use super::erofs::{ErofsError, ErofsImage};
use super::ext4::{Ext4Error, Ext4Image};
use super::zip_util::{create_zip_archive, extract_zip_entry, list_zip_entries};
use crate::commands::{Args, ToolContext};
use crate::error::{DecxError, DecxResult};

const SUPPORTED_OEMS: [&str; 6] = ["vivo", "oppo", "xiaomi", "honor", "google", "samsung"];

const DEFAULT_FRAMEWORK_DIRS: [&str; 4] = [
    "/system/framework",
    // Runtime APEX mount point: activated modules expose their payload
    // already extracted (javalib jars are ready-made files).
    "/apex",
    "/vendor/framework",
    "/system_ext/framework",
];

/// Fallback tier: images pulled (and later unpacked) only for modules the
/// /apex mount did not already provide.
const APEX_IMAGE_FALLBACK_DIRS: [&str; 1] = ["/system/apex"];

const PRIMARY_FILE_TYPES: [&str; 3] = [".apk", ".jar", ".dex"];
const APEX_IMAGE_FILE_TYPES: [&str; 2] = [".apex", ".capex"];

const FRAMEWORK_MANIFEST: &str = "Manifest-Version: 1.0\nCreated-By: decx\n";

// ── OEM / layout / artifact ────────────────────────────────────────────────

fn oem_search_paths(oem: &str) -> Vec<&'static str> {
    match oem {
        "oppo" => vec!["/system/framework", "/apex", "/system_ext/framework"],
        "xiaomi" => vec![
            "/system/framework",
            "/apex",
            "/system_ext/framework",
            "/vendor/framework",
        ],
        _ => DEFAULT_FRAMEWORK_DIRS.to_vec(),
    }
}

pub fn normalize_oem(value: &str) -> DecxResult<String> {
    let lowered = value.trim().to_lowercase();
    if SUPPORTED_OEMS.contains(&lowered.as_str()) {
        Ok(lowered)
    } else {
        Err(DecxError::usage(format!(
            "Unsupported OEM '{value}'. Supported: {}",
            SUPPORTED_OEMS.join(", ")
        )))
    }
}

/// Sanitize a name segment for artifact/jar naming (TS sanitizeArtifactSegment).
pub fn sanitize_segment(value: &str) -> String {
    let lowered = value.trim().to_lowercase();
    let mut out = String::new();
    let mut last_underscore = false;
    for ch in lowered.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '.' || ch == '_' || ch == '-' {
            out.push(ch);
            last_underscore = ch == '_';
        } else if !last_underscore {
            out.push('_');
            last_underscore = true;
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() { "unknown".into() } else { trimmed }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameworkArtifact {
    pub name: String,
    pub oem: String,
    pub vendor: String,
    pub root_dir: String,
    pub jar_path: String,
    pub updated_at: u64,
}

pub struct Layout {
    pub root_dir: PathBuf,
    pub source_dir: PathBuf,
    pub out_dir: PathBuf,
    pub out_tmp_dir: PathBuf,
    pub apex_tmp_dir: PathBuf,
    pub artifact_path: PathBuf,
    pub jar_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct FrameworkOptions {
    pub oem: Option<String>,
    pub out_dir: Option<String>,
    pub source_dir: Option<String>,
    pub adb_path: Option<String>,
    pub serial: Option<String>,
}

fn framework_jar_path_for(out_dir: &Path, oem: &str, vendor: &str) -> PathBuf {
    out_dir.join(format!(
        "framework_{}_{}.jar",
        sanitize_segment(oem),
        sanitize_segment(vendor)
    ))
}

fn read_artifact(path: &Path) -> Option<FrameworkArtifact> {
    let text = fs::read_to_string(path).ok()?;
    let mut parsed: FrameworkArtifact = serde_json::from_str(&text).ok()?;
    parsed.oem = sanitize_segment(&parsed.oem);
    parsed.vendor = sanitize_segment(&parsed.vendor);
    parsed.name = format!("framework_{}_{}", parsed.oem, parsed.vendor);
    Some(parsed)
}

fn write_artifact(path: &Path, artifact: &FrameworkArtifact) {
    let _ = fs::write(path, serde_json::to_string_pretty(artifact).unwrap() + "\n");
}

fn build_artifact(layout: &Layout, oem: &str, vendor: &str) -> FrameworkArtifact {
    let oem = sanitize_segment(oem);
    let vendor = sanitize_segment(vendor);
    FrameworkArtifact {
        name: format!("framework_{oem}_{vendor}"),
        jar_path: framework_jar_path_for(&layout.out_dir, &oem, &vendor).display().to_string(),
        root_dir: layout.root_dir.display().to_string(),
        oem,
        vendor,
        updated_at: now_millis(),
    }
}

fn ensure_dir(path: &Path) -> PathBuf {
    let _ = fs::create_dir_all(path);
    path.to_path_buf()
}

/// Resolve the output layout (TS resolveFrameworkLayout). `require_oem`
/// mirrors the collect-time check that an OEM is known.
pub fn resolve_layout(opts: &FrameworkOptions, require_oem: bool) -> DecxResult<Layout> {
    let oem = match &opts.oem {
        Some(raw) if !raw.trim().is_empty() => Some(normalize_oem(raw)?),
        _ => None,
    };
    if require_oem && oem.is_none() {
        return Err(DecxError::usage("OEM is required (pass --oem or connect a device for auto-detection)"));
    }
    let root_dir = match &opts.out_dir {
        Some(dir) => PathBuf::from(dir),
        None => crate::settings::decx_home()
            .join("framework")
            .join(oem.as_deref().unwrap_or("google")),
    };
    let out_dir = ensure_dir(&root_dir);
    let root_dir = out_dir.clone();
    let source_dir = ensure_dir(&opts.source_dir.clone().map(PathBuf::from).unwrap_or_else(|| out_dir.join("source")));
    let out_tmp_dir = ensure_dir(&out_dir.join("out_tmp"));
    let apex_tmp_dir = ensure_dir(&out_dir.join("apex_tmp"));
    let artifact_path = out_dir.join(".artifact.json");
    let artifact = read_artifact(&artifact_path);
    let brand = sanitize_segment(oem.as_deref().or(artifact.as_ref().map(|a| a.oem.as_str())).unwrap_or("google"));
    let vendor = sanitize_segment(artifact.as_ref().map(|a| a.vendor.as_str()).unwrap_or("unknown"));
    let jar_path = framework_jar_path_for(&out_dir, &brand, &vendor);
    Ok(Layout {
        root_dir,
        source_dir,
        out_dir,
        out_tmp_dir,
        apex_tmp_dir,
        artifact_path,
        jar_path,
    })
}

fn remove_legacy_metadata(out_dir: &Path) {
    let _ = fs::remove_file(out_dir.join(".meta.json"));
}

// ── Collector ──────────────────────────────────────────────────────────────

fn build_find_command(paths: &[&str], file_types: &[&str]) -> String {
    let names = file_types
        .iter()
        .map(|t| format!("-name '*{t}'"))
        .collect::<Vec<_>>()
        .join(" -o ");
    paths
        .iter()
        .map(|p| format!("find {p} -type f \\( {names} \\)"))
        .collect::<Vec<_>>()
        .join(" ; ")
}

pub fn filter_scan_output(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| !l.contains("Permission denied") && !l.contains("No such file"))
        .map(str::to_string)
        .collect()
}

/// Module name of a remote path inside the /apex mount, `@version` stripped.
pub fn apex_mount_module(remote_path: &str) -> Option<String> {
    let segments: Vec<&str> = remote_path.split('/').filter(|s| !s.is_empty()).collect();
    let apex_index = segments.iter().position(|s| *s == "apex")?;
    let module = segments.get(apex_index + 1)?;
    let name = module.split('@').next().unwrap_or("");
    if name.is_empty() { None } else { Some(name.to_string()) }
}

/// Module name of a fallback-tier image file (`com.android.art.apex`).
pub fn apex_image_module(remote_path: &str) -> Option<String> {
    let name = Path::new(remote_path).file_name()?.to_str()?;
    let lowered = name.to_lowercase();
    let stem = lowered
        .strip_suffix(".capex")
        .or_else(|| lowered.strip_suffix(".apex"))?;
    let module = stem.split('@').next().unwrap_or("");
    if module.is_empty() { None } else { Some(module.to_string()) }
}

/// Modules already collected under `<source>/apex/<module>[@version]/` with content.
fn collected_apex_modules(source_dir: &Path) -> std::collections::BTreeSet<String> {
    let mut covered = std::collections::BTreeSet::new();
    let apex_dir = source_dir.join("apex");
    let Ok(entries) = fs::read_dir(&apex_dir) else { return covered };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // A directory left behind by a failed pull holds no content.
        if fs::read_dir(&path).map(|mut it| it.next().is_none()).unwrap_or(true) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let module = name.split('@').next().unwrap_or("").to_string();
        if !module.is_empty() {
            covered.insert(module);
        }
    }
    covered
}

pub struct CollectionResult {
    pub scanned: usize,
    pub pulled: usize,
    pub failed: usize,
    pub files: Vec<String>,
    pub failures: Vec<(String, String)>,
    pub skipped_covered_modules: usize,
}

pub fn collect_framework_files(
    adb: &mut AdbClient,
    oem: &str,
    source_dir: &Path,
) -> DecxResult<CollectionResult> {
    let mut files = Vec::new();
    let mut failures = Vec::new();
    let mut covered = collected_apex_modules(source_dir);

    // Pull one file; returns the /apex mount module it covers, if any.
    fn pull_one(
        adb: &mut AdbClient,
        remote: &str,
        source_dir: &Path,
        files: &mut Vec<String>,
        failures: &mut Vec<(String, String)>,
    ) -> Option<String> {
        let local = source_dir.join(remote.trim_start_matches('/'));
        if let Some(parent) = local.parent() {
            let _ = fs::create_dir_all(parent);
        }
        match adb.pull(remote, &local.display().to_string()) {
            Ok(()) => {
                files.push(local.display().to_string());
                apex_mount_module(remote)
            }
            Err(e) => {
                failures.push((remote.to_string(), e.to_string()));
                None
            }
        }
    }

    // Tier 1 — ready-made files: framework dirs plus the /apex mount content.
    let primary_files = filter_scan_output(&adb.shell(
        &build_find_command(&oem_search_paths(oem), &PRIMARY_FILE_TYPES),
        std::time::Duration::from_secs(120),
    )?);
    for remote in &primary_files {
        if let Some(module) = pull_one(adb, remote, source_dir, &mut files, &mut failures) {
            covered.insert(module);
        }
    }

    // Tier 2 — .apex/.capex images, only for modules the mount did not cover.
    let image_files = filter_scan_output(&adb.shell(
        &build_find_command(&APEX_IMAGE_FALLBACK_DIRS, &APEX_IMAGE_FILE_TYPES),
        std::time::Duration::from_secs(120),
    )?);
    let mut skipped_covered_modules = 0;
    for remote in &image_files {
        if let Some(module) = apex_image_module(remote) {
            if covered.contains(&module) {
                skipped_covered_modules += 1;
                continue;
            }
        }
        pull_one(adb, remote, source_dir, &mut files, &mut failures);
    }

    Ok(CollectionResult {
        scanned: primary_files.len() + image_files.len(),
        pulled: files.len(),
        failed: failures.len(),
        files,
        failures,
        skipped_covered_modules,
    })
}

// ── External image tools (debugfs / erofs-utils, PATH lookup) ─────────────

#[derive(Debug, Clone)]
pub struct FrameworkTool {
    pub argv: Vec<String>,
}

impl FrameworkTool {
    fn local(argv: &[&str]) -> Self {
        Self { argv: argv.iter().map(|s| s.to_string()).collect() }
    }
}

fn command_exists(command: &str) -> bool {
    let probe = if cfg!(windows) { "where" } else { "which" };
    Command::new(probe)
        .arg(command)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn resolve_debugfs() -> DecxResult<FrameworkTool> {
    if command_exists("debugfs") {
        return Ok(FrameworkTool::local(&["debugfs"]));
    }
    if cfg!(windows) {
        return Err(DecxError::file(
            "ext4 feature not supported by the native reader and debugfs has no native Windows binary. Run 'framework process' on Linux/macOS (e2fsprogs) to unpack this payload.",
            None,
        ));
    }
    Err(DecxError::file(
        "debugfs not found. Install e2fsprogs (e.g. 'apt install e2fsprogs') so debugfs is on PATH.",
        None,
    ))
}

fn resolve_erofs_extractor() -> DecxResult<FrameworkTool> {
    if command_exists("fsck.erofs") {
        return Ok(FrameworkTool::local(&["fsck.erofs"]));
    }
    if command_exists("extract.erofs") {
        return Ok(FrameworkTool::local(&["extract.erofs"]));
    }
    if cfg!(windows) {
        return Err(DecxError::file(
            "EROFS payload images need erofs-utils, which has no native Windows binary. Run 'framework process' on Linux/macOS (erofs-utils) to unpack this payload.",
            None,
        ));
    }
    Err(DecxError::file(
        "No EROFS extractor found. Install fsck.erofs/extract.erofs (erofs-utils).",
        None,
    ))
}

fn run_tool(tool: &FrameworkTool, args: &[String]) -> DecxResult<()> {
    let out = Command::new(&tool.argv[0])
        .args(&tool.argv[1..])
        .args(args)
        .output()
        .map_err(|e| DecxError::file(format!("failed to execute {}: {e}", tool.argv.join(" ")), None))?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let msg = if msg.is_empty() { String::from_utf8_lossy(&out.stdout).trim().to_string() } else { msg };
        return Err(DecxError::file(
            if msg.is_empty() { format!("{} failed", tool.argv.join(" ")) } else { msg },
            None,
        ));
    }
    Ok(())
}

// ── Processor ──────────────────────────────────────────────────────────────

const SUPPORTED_EXTENSIONS: [&str; 5] = [".apk", ".jar", ".apex", ".capex", ".dex"];
/// File types extracted from apex payload images (native ext4 + tool path).
const APEX_PAYLOAD_EXTENSIONS: [&str; 3] = [".jar", ".apk", ".dex"];

fn extension_of(path: &Path) -> String {
    path.extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default()
}

fn walk_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_files(&path, found);
        } else if SUPPORTED_EXTENSIONS.contains(&extension_of(&path).as_str()) {
            found.push(path);
        }
    }
}

fn detect_filesystem_type(path: &Path) -> DecxResult<&'static str> {
    // ext4/erfs magics sit at fixed offsets in the first 2 KiB.
    let mut buf = vec![0u8; 2048];
    let mut file = fs::File::open(path)
        .map_err(|e| DecxError::file(format!("failed to open '{}': {e}", path.display()), None))?;
    use std::io::Read;
    file.read_exact(&mut buf)
        .map_err(|e| DecxError::file(format!("failed to read '{}': {e}", path.display()), None))?;
    if buf[1024..1028] == [0xe2, 0xe1, 0xf5, 0xe0] {
        return Ok("erofs");
    }
    if buf[1080..1082] == [0x53, 0xef] {
        return Ok("ext4");
    }
    Ok("ext2")
}

fn extract_dex_from_zip(input: &Path, output_dir: &Path, prefix: &str) -> DecxResult<()> {
    let mut counter = 0usize;
    for entry in list_zip_entries(input)? {
        if !entry.to_lowercase().ends_with(".dex") {
            continue;
        }
        let base = Path::new(&entry)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| entry.clone());
        let target = output_dir.join(format!("{prefix}_{base}"));
        // Extract via a unique temp file then rename, so processing of inputs
        // producing the same target name cannot interleave writes.
        let tmp = output_dir.join(format!(
            "{}.{}.{}.tmp",
            target.display(),
            std::process::id(),
            counter
        ));
        counter += 1;
        if let Err(e) = extract_zip_entry(input, &entry, &tmp) {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
        fs::rename(&tmp, &target)
            .map_err(|e| DecxError::file(format!("failed to rename '{}': {e}", target.display()), None))?;
    }
    Ok(())
}

/// Pull `apex_payload.img` (recursing through `original_apex`) out of an .apex.
fn extract_apex_payload(apex_file: &Path, target_dir: &Path) -> DecxResult<PathBuf> {
    fs::create_dir_all(target_dir)
        .map_err(|e| DecxError::file(format!("failed to create '{}': {e}", target_dir.display()), None))?;
    let entries = list_zip_entries(apex_file)?;

    if entries.iter().any(|e| e == "original_apex") {
        let nested = target_dir.join("original.apex");
        extract_zip_entry(apex_file, "original_apex", &nested)?;
        return extract_apex_payload(&nested, target_dir);
    }
    if !entries.iter().any(|e| e == "apex_payload.img") {
        return Err(DecxError::file(
            format!("No apex_payload.img found in {}", apex_file.display()),
            Some(apex_file.display().to_string()),
        ));
    }
    let payload = target_dir.join("apex_payload.img");
    extract_zip_entry(apex_file, "apex_payload.img", &payload)?;
    Ok(payload)
}

/// Extract jar/apk/dex from an ext4/erofs payload image natively. Returns
/// false when the image is not natively parseable (unsupported ext4 feature,
/// EROFS with an unsupported compressor) so the caller can fall back to the
/// external-tool pipeline.
fn extract_payload_natively(payload: &Path, payload_dir: &Path) -> DecxResult<bool> {
    let wanted = |rel: &str| {
        APEX_PAYLOAD_EXTENSIONS.contains(&extension_of(Path::new(rel)).as_str())
    };
    match detect_filesystem_type(payload)? {
        "ext4" => {
            let image = match Ext4Image::open(&payload.display().to_string()) {
                Ok(image) => image,
                Err(Ext4Error::NotExt4Image | Ext4Error::UnsupportedFeature(_)) => return Ok(false),
                Err(e) => return Err(DecxError::file(format!("ext4 read failed: {e:?}"), None)),
            };
            match image.extract_to(&payload_dir.display().to_string(), &wanted) {
                Ok(()) => Ok(true),
                Err(Ext4Error::UnsupportedFeature(_)) => Ok(false), // retry with debugfs
                Err(e) => Err(DecxError::file(format!("ext4 extract failed: {e:?}"), None)),
            }
        }
        "erofs" => {
            let image = match ErofsImage::open(&payload.display().to_string()) {
                Ok(image) => image,
                // Unknown compressors etc. keep the external-tool fallback.
                Err(ErofsError::NotErofsImage | ErofsError::Unsupported(_)) => return Ok(false),
                Err(e) => return Err(DecxError::file(format!("erofs read failed: {e:?}"), None)),
            };
            match image.extract_to(&payload_dir.display().to_string(), &wanted) {
                Ok(()) => Ok(true),
                Err(ErofsError::Unsupported(_)) => Ok(false), // retry with erofs-utils
                Err(e) => Err(DecxError::file(format!("erofs extract failed: {e:?}"), None)),
            }
        }
        _ => Ok(false),
    }
}

fn extract_filesystem_image(image: &Path, extract_dir: &Path, debugfs: &FrameworkTool, erofs: &FrameworkTool) -> DecxResult<()> {
    fs::create_dir_all(extract_dir)
        .map_err(|e| DecxError::file(format!("failed to create '{}': {e}", extract_dir.display()), None))?;
    let fs_type = detect_filesystem_type(image)?;
    if fs_type == "erofs" {
        let bin = Path::new(&erofs.argv[erofs.argv.len() - 1]);
        let name = bin.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if name == "fsck.erofs" {
            return run_tool(
                erofs,
                &[format!("--extract={}", extract_dir.display()), "--overwrite".into(), image.display().to_string()],
            );
        }
        return run_tool(
            erofs,
            &["-i".into(), image.display().to_string(), "-x".into(), "-f".into(), "-o".into(), extract_dir.display().to_string()],
        );
    }
    run_tool(debugfs, &["-R".into(), format!("rdump ./ {}", extract_dir.display()), image.display().to_string()])
}

/// APEX module name for inputs pulled from the runtime APEX mount point
/// (`apex/<module>/...`, at least one deeper component required).
fn extracted_apex_module(input: &Path, source_dir: &Path) -> Option<String> {
    let rel = input.strip_prefix(source_dir).ok()?;
    let segments: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let apex_index = segments.iter().position(|s| s == "apex")?;
    // Require at least `apex/<module>/<file>` depth.
    if segments.len() < apex_index + 3 {
        return None;
    }
    let module = segments[apex_index + 1].split('@').next().unwrap_or("").to_string();
    if module.is_empty() { None } else { Some(module) }
}

fn process_apex(input: &Path, layout: &Layout, tools: &mut Option<(FrameworkTool, FrameworkTool)>) -> DecxResult<()> {
    let apex_name = input
        .file_stem()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let apex_tmp = layout.apex_tmp_dir.join(&apex_name);
    let payload_dir = apex_tmp.join("payload");
    let payload = extract_apex_payload(input, &apex_tmp)?;

    if !extract_payload_natively(&payload, &payload_dir)? {
        // EROFS payload or unsupported ext4 feature: external tools (PATH only;
        // no native Windows binaries — the error explains where to run it).
        if tools.is_none() {
            *tools = Some((resolve_debugfs()?, resolve_erofs_extractor()?));
        }
        let (debugfs, erofs) = tools.as_ref().unwrap();
        extract_filesystem_image(&payload, &payload_dir, debugfs, erofs)?;
    }

    let mut nested = Vec::new();
    walk_files(&payload_dir, &mut nested);
    for file in nested {
        let ext = extension_of(&file);
        let base = file.file_name().unwrap_or_default().to_string_lossy().to_string();
        if ext == ".jar" || ext == ".apk" {
            let stem = file.file_stem().unwrap_or_default().to_string_lossy().to_string();
            extract_dex_from_zip(&file, &layout.out_tmp_dir, &format!("{apex_name}_{stem}"))?;
        } else if ext == ".dex" {
            fs::copy(&file, layout.out_tmp_dir.join(format!("{apex_name}_{base}")))
                .map_err(|e| DecxError::file(format!("copy failed: {e}"), None))?;
        }
    }
    Ok(())
}

fn process_framework_input(
    input: &Path,
    layout: &Layout,
    tools: &mut Option<(FrameworkTool, FrameworkTool)>,
) -> DecxResult<()> {
    let ext = extension_of(input);
    if let Some(module) = extracted_apex_module(input, &layout.source_dir) {
        if ext == ".jar" || ext == ".apk" || ext == ".dex" {
            // Pulled from the runtime /apex mount: payload already extracted on
            // device; namespace dex outputs by module name.
            let base = input.file_name().unwrap_or_default().to_string_lossy().to_string();
            if ext == ".dex" {
                fs::copy(input, layout.out_tmp_dir.join(format!("{module}_{base}")))
                    .map_err(|e| DecxError::file(format!("copy failed: {e}"), None))?;
            } else {
                let stem = input.file_stem().unwrap_or_default().to_string_lossy().to_string();
                extract_dex_from_zip(input, &layout.out_tmp_dir, &format!("{module}_{stem}"))?;
            }
            return Ok(());
        }
    }
    match ext.as_str() {
        ".jar" | ".apk" => {
            let stem = input.file_stem().unwrap_or_default().to_string_lossy().to_string();
            extract_dex_from_zip(input, &layout.out_tmp_dir, &stem)
        }
        ".apex" | ".capex" => process_apex(input, layout, tools),
        ".dex" => {
            let base = input.file_name().unwrap_or_default().to_string_lossy().to_string();
            fs::copy(input, layout.out_tmp_dir.join(base))
                .map(|_| ())
                .map_err(|e| DecxError::file(format!("copy failed: {e}"), None))
        }
        _ => Ok(()),
    }
}

pub struct ProcessResult {
    pub processed: usize,
    pub failed: usize,
    pub outputs: Vec<String>,
    pub failures: Vec<(String, String)>,
}

/// True when the source tree contains .apex/.capex inputs needing tools.
pub fn has_apex_image_inputs(source_dir: &Path) -> bool {
    let mut files = Vec::new();
    walk_files(source_dir, &mut files);
    files
        .iter()
        .any(|f| matches!(extension_of(f).as_str(), ".apex" | ".capex"))
}

pub fn process_framework_files(layout: &Layout) -> DecxResult<ProcessResult> {
    fs::create_dir_all(&layout.out_tmp_dir)
        .map_err(|e| DecxError::file(format!("failed to create '{}': {e}", layout.out_tmp_dir.display()), None))?;
    fs::create_dir_all(&layout.apex_tmp_dir)
        .map_err(|e| DecxError::file(format!("failed to create '{}': {e}", layout.apex_tmp_dir.display()), None))?;

    let mut files = Vec::new();
    walk_files(&layout.source_dir, &mut files);
    let before: std::collections::BTreeSet<String> = walk_files_vec(&layout.out_tmp_dir)
        .into_iter()
        .map(|p| p.display().to_string())
        .collect();

    // Tools resolve lazily: a source pulled from /apex (plain jars/dex) never
    // resolves the external toolchain at all.
    let mut tools: Option<(FrameworkTool, FrameworkTool)> = None;
    let mut processed = 0usize;
    let mut failures = Vec::new();
    for input in &files {
        match process_framework_input(input, layout, &mut tools) {
            Ok(()) => processed += 1,
            Err(e) => failures.push((input.display().to_string(), e.to_string())),
        }
    }

    let outputs = walk_files_vec(&layout.out_tmp_dir)
        .into_iter()
        .map(|p| p.display().to_string())
        .filter(|p| !before.contains(p))
        .collect();
    Ok(ProcessResult {
        processed,
        failed: failures.len(),
        outputs,
        failures,
    })
}

fn walk_files_vec(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_all_files(dir, &mut out);
    out
}

fn walk_all_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_all_files(&path, found);
        } else {
            found.push(path);
        }
    }
}

// ── Packer ─────────────────────────────────────────────────────────────────

pub fn pack_framework_jar(layout: &Layout) -> DecxResult<(String, usize)> {
    let staged = walk_files_vec(&layout.out_tmp_dir);
    if staged.is_empty() {
        return Err(DecxError::file(
            format!("No processed files found in {}", layout.out_tmp_dir.display()),
            Some(layout.out_tmp_dir.display().to_string()),
        ));
    }
    let count = staged.len();
    let staging = layout
        .out_dir
        .join(format!(".pack_tmp_{}_{:x}", std::process::id(), now_millis()));
    let manifest_dir = staging.join("META-INF");
    fs::create_dir_all(&manifest_dir)
        .map_err(|e| DecxError::file(format!("failed to create '{}': {e}", manifest_dir.display()), None))?;
    fs::write(manifest_dir.join("MANIFEST.MF"), FRAMEWORK_MANIFEST)
        .map_err(|e| DecxError::file(format!("failed to write MANIFEST.MF: {e}"), None))?;
    let mut entries = vec!["META-INF".to_string()];
    for file in &staged {
        let base = file.file_name().unwrap_or_default().to_string_lossy().to_string();
        fs::copy(file, staging.join(&base))
            .map_err(|e| DecxError::file(format!("staging copy failed: {e}"), None))?;
        entries.push(base);
    }
    let result = create_zip_archive(&layout.jar_path, &entries, &staging);
    let _ = fs::remove_dir_all(&staging);
    result?;
    Ok((layout.jar_path.display().to_string(), count))
}

fn clean_framework_temp_dirs(layout: &Layout) {
    let _ = fs::remove_dir_all(&layout.out_tmp_dir);
    let _ = fs::remove_dir_all(&layout.apex_tmp_dir);
}

// ── Flows (command entry points) ───────────────────────────────────────────

fn opts_from_args(a: &Args) -> FrameworkOptions {
    FrameworkOptions {
        oem: a.opt_str("oem").map(str::to_string),
        out_dir: a.opt_str("out-dir").map(str::to_string),
        source_dir: a.opt_str("source-dir").map(str::to_string),
        adb_path: a.opt_str("adb-path").map(str::to_string),
        serial: a.opt_str("serial").map(str::to_string),
    }
}

fn adb_client(opts: &FrameworkOptions) -> AdbClient {
    AdbClient::new(opts.adb_path.clone(), opts.serial.clone())
}

/// Device model for the artifact vendor segment. Single-device auto-select;
/// several devices without --serial is an error; no device keeps "unknown"
/// (offline-safe).
fn detect_connected_vendor(opts: &FrameworkOptions) -> DecxResult<String> {
    let mut adb = adb_client(opts);
    adb.ensure_available()?;
    adb.ensure_device_connected()?;
    let model = adb.device_model()?;
    Ok(if model.is_empty() { "unknown".into() } else { model })
}

pub fn collect_flow(opts: &FrameworkOptions) -> DecxResult<Value> {
    let mut adb = adb_client(opts);
    adb.ensure_available()?;
    adb.ensure_device_connected()?;
    let oem = match &opts.oem {
        Some(raw) => normalize_oem(raw)?,
        None => adb.framework_oem()?,
    };
    let opts = FrameworkOptions { oem: Some(oem.clone()), ..opts.clone() };
    let layout = resolve_layout(&opts, true)?;
    let vendor = {
        let model = adb.device_model().unwrap_or_default();
        if model.is_empty() { "unknown".into() } else { model }
    };
    write_artifact(&layout.artifact_path, &build_artifact(&layout, &oem, &vendor));
    remove_legacy_metadata(&layout.out_dir);
    let layout = resolve_layout(&opts, true)?;
    let result = collect_framework_files(&mut adb, &oem, &layout.source_dir)?;
    Ok(json!({
        "oem": oem,
        "vendor": sanitize_segment(&vendor),
        "sourceDir": layout.source_dir.display().to_string(),
        "scanned": result.scanned,
        "pulled": result.pulled,
        "failed": result.failed,
        "skippedCoveredModules": result.skipped_covered_modules,
        "files": result.files,
        "failures": result.failures.iter().map(|(p, e)| json!({"path": p, "error": e})).collect::<Vec<_>>(),
    }))
}

/// Resolve the process OEM: explicit > artifact at this out-dir > device.
fn resolve_process_oem(opts: &FrameworkOptions) -> DecxResult<String> {
    if let Some(oem) = &opts.oem {
        return normalize_oem(oem);
    }
    let layout = resolve_layout(opts, false)?;
    if let Some(artifact) = read_artifact(&layout.artifact_path) {
        if let Ok(oem) = normalize_oem(&artifact.oem) {
            return Ok(oem);
        }
    }
    let mut adb = adb_client(opts);
    adb.ensure_available()?;
    adb.ensure_device_connected()?;
    adb.framework_oem()
}

pub fn process_flow(opts: &FrameworkOptions) -> DecxResult<Value> {
    // Vendor feeds the framework_<oem>_<vendor>.jar name; detect from the
    // device only when the artifact has not recorded it yet (offline-safe).
    let pre = resolve_layout(opts, false)?;
    let pre_artifact = read_artifact(&pre.artifact_path);
    if pre_artifact.as_ref().map(|a| a.vendor == "unknown").unwrap_or(true) {
        if let Ok(vendor) = detect_connected_vendor(opts) {
            if vendor != "unknown" {
                let artifact = pre_artifact.unwrap_or_else(|| build_artifact(&pre, "google", "unknown"));
                write_artifact(
                    &pre.artifact_path,
                    &build_artifact(&pre, &artifact.oem, &vendor),
                );
            }
        }
        // No adb / no device / read failure: keep the offline default.
    }
    let oem = resolve_process_oem(opts)?;
    let opts = FrameworkOptions { oem: Some(oem.clone()), ..opts.clone() };
    let layout = resolve_layout(&opts, false)?;
    let result = process_framework_files(&layout)?;
    Ok(json!({
        "oem": oem,
        "outDir": layout.out_dir.display().to_string(),
        "processed": result.processed,
        "failed": result.failed,
        "outputs": result.outputs,
        "failures": result.failures.iter().map(|(p, e)| json!({"path": p, "error": e})).collect::<Vec<_>>(),
    }))
}

pub fn pack_flow(opts: &FrameworkOptions) -> DecxResult<Value> {
    let layout = resolve_layout(opts, false)?;
    fs::create_dir_all(&layout.out_dir)
        .map_err(|e| DecxError::file(format!("failed to create '{}': {e}", layout.out_dir.display()), None))?;
    let (jar, count) = pack_framework_jar(&layout)?;
    remove_legacy_metadata(&layout.out_dir);
    clean_framework_temp_dirs(&layout);
    Ok(json!({ "ok": true, "jarPath": jar, "fileCount": count }))
}

/// process + pack with an up-to-date artifact record.
pub fn build_flow(opts: &FrameworkOptions) -> DecxResult<Value> {
    process_flow(opts)?;
    let layout = resolve_layout(opts, false)?;
    fs::create_dir_all(&layout.out_dir)
        .map_err(|e| DecxError::file(format!("failed to create '{}': {e}", layout.out_dir.display()), None))?;
    let (jar, count) = pack_framework_jar(&layout)?;
    if let Some(artifact) = read_artifact(&layout.artifact_path) {
        write_artifact(&layout.artifact_path, &build_artifact(&layout, &artifact.oem, &artifact.vendor));
    }
    remove_legacy_metadata(&layout.out_dir);
    clean_framework_temp_dirs(&layout);
    Ok(json!({ "ok": true, "jarPath": jar, "fileCount": count }))
}

/// collect + process + pack. Opening the produced jar is deliberately NOT
/// here — use `decx session open <framework.jar>`.
pub fn run_flow(opts: &FrameworkOptions) -> DecxResult<Value> {
    let collection = collect_flow(opts)?;
    let build = build_flow(opts)?;
    Ok(json!({
        "collection": collection,
        "build": build,
        "hint": "open with: decx session open <jarPath>",
    }))
}

// ── Local command handlers ─────────────────────────────────────────────────

pub fn framework_collect(_ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    collect_flow(&opts_from_args(a))
}

pub fn framework_process(_ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    process_flow(&opts_from_args(a))
}

pub fn framework_pack(_ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    pack_flow(&opts_from_args(a))
}

pub fn framework_run(_ctx: &ToolContext, a: &Args) -> DecxResult<Value> {
    run_flow(&opts_from_args(a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_segment_matches_ts() {
        assert_eq!(sanitize_segment("Pixel 8 Pro"), "pixel_8_pro");
        assert_eq!(sanitize_segment("  X--Y  "), "x--y");
        assert_eq!(sanitize_segment("Ď锬"), "unknown");
        assert_eq!(sanitize_segment("MI 14@U"), "mi_14_u");
    }

    #[test]
    fn oem_paths() {
        assert_eq!(oem_search_paths("google"), DEFAULT_FRAMEWORK_DIRS.to_vec());
        assert!(!oem_search_paths("oppo").contains(&"/vendor/framework"));
        assert!(normalize_oem("Samsung").is_ok());
        assert!(normalize_oem("nope").is_err());
    }

    #[test]
    fn apex_module_names() {
        assert_eq!(
            apex_mount_module("/apex/com.android.art@340999010/javalib/core-oj.jar"),
            Some("com.android.art".into())
        );
        assert_eq!(apex_mount_module("/system/framework/framework.jar"), None);
        assert_eq!(apex_image_module("/system/apex/com.android.art.apex"), Some("com.android.art".into()));
        assert_eq!(apex_image_module("/system/apex/com.android.art.capex"), Some("com.android.art".into()));
        assert_eq!(apex_image_module("/system/framework/framework.jar"), None);
    }

    #[test]
    fn scan_output_filtering() {
        let out = "/a/b.jar\r\n\n  /c.dex  \nfind: '/x': Permission denied\nfind: '/y': No such file or directory\n";
        assert_eq!(filter_scan_output(out), vec!["/a/b.jar", "/c.dex"]);
    }

    #[test]
    fn find_command_shape() {
        let cmd = build_find_command(&["/system/framework"], &[".jar", ".dex"]);
        assert!(cmd.starts_with("find /system/framework -type f \\( -name '*.jar' -o -name '*.dex' \\)"));
    }

    #[test]
    fn extracted_apex_module_depth() {
        let source = Path::new("/src");
        assert_eq!(
            extracted_apex_module(Path::new("/src/apex/com.android.art@1/javalib/x.jar"), source),
            Some("com.android.art".into())
        );
        assert_eq!(extracted_apex_module(Path::new("/src/apex/stray.jar"), source), None);
        assert_eq!(extracted_apex_module(Path::new("/src/system/framework.jar"), source), None);
    }

    #[test]
    fn artifact_roundtrip_and_layout() {
        let dir = std::env::temp_dir().join(format!("decx-fw-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let opts = FrameworkOptions {
            oem: Some("google".into()),
            out_dir: Some(dir.display().to_string()),
            source_dir: None,
            adb_path: None,
            serial: None,
        };
        let layout = resolve_layout(&opts, true).unwrap();
        assert!(layout.jar_path.ends_with("framework_google_unknown.jar"));

        write_artifact(&layout.artifact_path, &build_artifact(&layout, "google", "Pixel 8"));
        let again = resolve_layout(&opts, true).unwrap();
        assert!(again.jar_path.ends_with("framework_google_pixel_8.jar"));
        let artifact = read_artifact(&again.artifact_path).unwrap();
        assert_eq!(artifact.name, "framework_google_pixel_8");
        assert!(artifact.updated_at > 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn processes_erofs_payload_apex_natively() {
        let root = std::env::temp_dir().join(format!("decx-fw-erofs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let source = ensure_dir(&root.join("source"));
        let payload_src = ensure_dir(&root.join("payload-src"));
        // apex_payload.img = EROFS fixture image (contains javalib/module.jar,
        // 171000 bytes, among other files)
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("apex_payload_erofs.img"),
            payload_src.join("apex_payload.img"),
        )
        .unwrap();
        create_zip_archive(
            &source.join("com.android.art.apex"),
            &["apex_payload.img".to_string()],
            &payload_src,
        )
        .unwrap();

        let opts = FrameworkOptions {
            oem: Some("google".into()),
            out_dir: Some(root.display().to_string()),
            source_dir: Some(source.display().to_string()),
            adb_path: None,
            serial: None,
        };
        let layout = resolve_layout(&opts, true).unwrap();
        // On Windows there are no erofs-utils binaries at all: the only
        // expected failure is the fixture's fake module.jar (plain text, not
        // a real zip) being rejected by the dex extractor — proving the EROFS
        // payload itself was unpacked by the native reader (otherwise the
        // error would be the missing erofs extractor).
        let result = process_framework_files(&layout).unwrap();
        assert_eq!(result.failed, 1, "failures: {:#?}", result.failures);
        assert!(
            result.failures[0].1.contains("Unrecognized archive format"),
            "unexpected failure: {}",
            result.failures[0].1
        );
        let extracted = layout
            .apex_tmp_dir
            .join("com.android.art")
            .join("payload")
            .join("javalib")
            .join("module.jar");
        assert_eq!(fs::metadata(&extracted).unwrap().len(), 171000);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn packer_requires_processed_outputs() {
        let dir = std::env::temp_dir().join(format!("decx-fw-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let opts = FrameworkOptions {
            oem: Some("google".into()),
            out_dir: Some(dir.display().to_string()),
            source_dir: None,
            adb_path: None,
            serial: None,
        };
        let layout = resolve_layout(&opts, true).unwrap();
        assert!(pack_framework_jar(&layout).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pack_and_process_offline_jar() {
        // End-to-end offline: a fake source jar with one dex entry → process → pack.
        let dir = std::env::temp_dir().join(format!("decx-fw-e2e-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let source = dir.join("source");
        fs::create_dir_all(&source).unwrap();
        let inner = dir.join("fake.jar");
        create_zip_archive(&inner, &["classes.dex".to_string()], &make_dex_dir(&dir)).unwrap();

        // move the staged jar into source
        fs::copy(&inner, source.join("framework.jar")).unwrap();
        let opts = FrameworkOptions {
            oem: Some("google".into()),
            out_dir: Some(dir.display().to_string()),
            source_dir: Some(source.display().to_string()),
            adb_path: None,
            serial: None,
        };
        let layout = resolve_layout(&opts, true).unwrap();
        let result = process_framework_files(&layout).unwrap();
        assert_eq!(result.failed, 0);
        assert_eq!(result.outputs.len(), 1);
        assert!(result.outputs[0].replace('\\', "/").ends_with("framework_classes.dex"));

        let (jar, count) = pack_framework_jar(&layout).unwrap();
        assert_eq!(count, 1);
        assert!(Path::new(&jar).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    fn make_dex_dir(dir: &Path) -> PathBuf {
        let dex_dir = dir.join("dexstage");
        fs::create_dir_all(&dex_dir).unwrap();
        fs::write(dex_dir.join("classes.dex"), b"fake dex bytes").unwrap();
        dex_dir
    }
}
