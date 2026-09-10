//! decx-apk — APK/JAR container layer (std-only): zip + inflate + axml + manifest.

pub mod axml;
pub mod inflate;
pub mod manifest;
pub mod zip;

pub use manifest::{Component, Manifest};
pub use zip::{ZipArchive, ZipEntry};

/// A loaded APK: zip + decoded manifest + dex entry listing.
pub struct Apk {
    pub zip: ZipArchive,
    pub manifest: Option<Manifest>,
    pub manifest_xml: Option<String>,
}

impl Apk {
    pub fn open(data: Vec<u8>) -> Result<Apk, String> {
        let zip = ZipArchive::open(data)?;
        let (manifest_xml, manifest) = match zip.read_by_name("AndroidManifest.xml") {
            Some(Ok(raw)) => {
                let text = axml::decode(&raw)?;
                let model = manifest::parse(&text);
                (Some(text), Some(model))
            }
            _ => (None, None),
        };
        Ok(Apk {
            zip,
            manifest,
            manifest_xml,
        })
    }

    /// Entry names of all classes*.dex files, sorted naturally (classes, classes2, ...).
    pub fn dex_entries(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .zip
            .entries()
            .iter()
            .filter(|e| {
                e.name == "classes.dex"
                    || (e.name.starts_with("classes")
                        && e.name.ends_with(".dex"))
            })
            .map(|e| e.name.as_str())
            .collect();
        names.sort_by(|a, b| dex_sort_key(a).cmp(&dex_sort_key(b)));
        names
    }

    pub fn read_dex(&self, name: &str) -> Option<Result<Vec<u8>, String>> {
        self.zip.read_by_name(name)
    }
}

fn dex_sort_key(n: &str) -> (u32, String) {
    // classes.dex -> 0; classesN.dex -> N; else big
    let stem = n.trim_end_matches(".dex");
    if stem == "classes" {
        (0, String::new())
    } else if let Some(num) = stem.strip_prefix("classes") {
        (num.parse().unwrap_or(u32::MAX), String::new())
    } else {
        (u32::MAX, n.to_string())
    }
}
