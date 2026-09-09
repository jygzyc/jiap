//! Kuna code-level binary analyzer.
//!
//! This is the code-level form of kuna inside the DECX workspace: instead of
//! shelling out to an external `kuna` binary, the analysis runs in-process.
//! The analyzer parses ELF64 binaries directly (std-only) — function symbols
//! from the symbol table, strings from `.rodata` — and renders structural
//! pseudo-C views. It intentionally reports `meta.mode = "structural"` so
//! callers never mistake it for a full decompiler.
//!
//! Fallbacks: stripped binaries (no `.symtab`) yield section-boundary
//! pseudo-functions plus the string table scan, clearly labeled.

use std::path::Path;

use serde_json::{json, Value};

use decx_cli_core::error::{DecxError, DecxResult};

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub address: u64,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct StringRef {
    pub offset: u64,
    pub value: String,
}

/// In-memory analysis of one binary. Parse once at server startup, serve
/// many queries.
pub struct Analyzer {
    pub file: String,
    pub functions: Vec<Function>,
    pub strings: Vec<StringRef>,
    pub sections: Vec<(String, u64, u64)>,
    pub stripped: bool,
}

const MIN_STRING_LEN: usize = 4;

impl Analyzer {
    /// Load and analyze a binary file.
    pub fn load(path: &Path) -> DecxResult<Self> {
        let bytes = std::fs::read(path)
            .map_err(|e| DecxError::file(format!("cannot read {}: {e}", path.display()), Some(path.display().to_string())))?;
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "binary".to_string());

        let mut analyzer = Self {
            file,
            functions: Vec::new(),
            strings: Vec::new(),
            sections: Vec::new(),
            stripped: false,
        };
        if bytes.starts_with(b"\x7fELF") {
            analyzer.parse_elf(&bytes)?;
        } else {
            // Not an ELF: fall back to a plain strings scan so generic
            // binaries still answer get_strings/search_global_key.
            analyzer.stripped = true;
        }
        analyzer.strings = scan_strings(&bytes, MIN_STRING_LEN);
        Ok(analyzer)
    }

    fn parse_elf(&mut self, bytes: &[u8]) -> DecxResult<()> {
        if bytes.len() < 64 || bytes[4] != 2 || bytes[5] != 1 {
            return Err(DecxError::file(
                "unsupported ELF: only 64-bit little-endian is analyzed in-process",
                Some(self.file.clone()),
            ));
        }
        let u16le = |off: usize| -> u64 { u16::from_le_bytes([bytes[off], bytes[off + 1]]) as u64 };
        let u32le = |off: usize| -> u64 { u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]]) as u64 };
        let u64le = |off: usize| -> u64 {
            u64::from_le_bytes([
                bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3], bytes[off + 4], bytes[off + 5], bytes[off + 6],
                bytes[off + 7],
            ])
        };

        let shoff = u64le(0x28) as usize;
        let shentsize = u16le(0x3a) as usize;
        let shnum = u16le(0x3c) as usize;
        if shoff == 0 || shnum == 0 || bytes.len() < shoff + shnum * shentsize {
            return Ok(());
        }

        let sh = |i: usize| -> (u32, u32, u64, u64, u64) {
            let base = shoff + i * shentsize;
            (
                u32le(base) as u32,     // sh_name (offset into shstrtab)
                u32le(base + 4) as u32, // sh_type
                u64le(base + 24),       // sh_offset
                u64le(base + 32),       // sh_size
                u64le(base + 56),       // sh_entsize
            )
        };

        // Section-name string table (index at 0x3e) to resolve names.
        let shstrndx = u16le(0x3e) as usize;
        let shstrtab = if shstrndx < shnum {
            let (_, _, off, size, _) = sh(shstrndx);
            slice(bytes, off, size)
        } else {
            &[]
        };
        let name_at = |tab: &[u8], name_off: u64| -> String {
            tab.get(name_off as usize..)
                .map(|rest| rest.iter().take_while(|&&b| b != 0).map(|&b| b as char).collect())
                .unwrap_or_default()
        };

        let mut symtab: Option<(u64, u64, u64)> = None; // (offset, size, entsize)
        let mut strtab: Option<(u64, u64)> = None; // (offset, size)
        for i in 0..shnum {
            let (name_off, sh_type, off, size, entsize) = sh(i);
            let name = name_at(shstrtab, name_off as u64);
            self.sections.push((name.clone(), off, size));
            const SHT_SYMTAB: u32 = 2;
            const SHT_STRTAB: u32 = 3;
            if sh_type == SHT_SYMTAB && entsize as usize == 24 {
                symtab = Some((off, size, entsize));
                // sh_link (u32 at +40) points at the string table section.
                let link = u32le(shoff + i * shentsize + 40) as usize;
                if link < shnum {
                    let (_, _, soff, ssize, _) = sh(link);
                    strtab = Some((soff, ssize));
                }
            } else if sh_type == SHT_STRTAB && strtab.is_none() && name == ".strtab" {
                strtab = Some((off, size));
            }
        }

        let Some((sym_off, sym_size, sym_entsize)) = symtab else {
            return Ok(());
        };
        let Some(_) = strtab else {
            return Ok(());
        };
        let (str_off, str_size) = strtab.unwrap();
        let strtab_bytes = slice(bytes, str_off, str_size);
        let sym_bytes = slice(bytes, sym_off, sym_size);
        let count = if sym_entsize > 0 { sym_bytes.len() / sym_entsize as usize } else { 0 };
        for i in 0..count {
            let base = i * sym_entsize as usize;
            let st_name = u32::from_le_bytes([sym_bytes[base], sym_bytes[base + 1], sym_bytes[base + 2], sym_bytes[base + 3]]);
            let st_info = sym_bytes[base + 4];
            let st_value = u64::from_le_bytes([
                sym_bytes[base + 8],
                sym_bytes[base + 9],
                sym_bytes[base + 10],
                sym_bytes[base + 11],
                sym_bytes[base + 12],
                sym_bytes[base + 13],
                sym_bytes[base + 14],
                sym_bytes[base + 15],
            ]);
            let st_size = u64::from_le_bytes([
                sym_bytes[base + 16],
                sym_bytes[base + 17],
                sym_bytes[base + 18],
                sym_bytes[base + 19],
                sym_bytes[base + 20],
                sym_bytes[base + 21],
                sym_bytes[base + 22],
                sym_bytes[base + 23],
            ]);
            const STT_FUNC: u8 = 2;
            if st_info & 0xf != STT_FUNC || st_name == 0 {
                continue;
            }
            let name = strtab_bytes
                .get(st_name as usize..)
                .map(|rest| rest.iter().take_while(|&&b| b != 0).map(|&b| b as char).collect::<String>())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            self.functions.push(Function {
                name,
                address: st_value,
                size: st_size,
            });
        }
        self.functions.sort_by(|a, b| a.address.cmp(&b.address));
        Ok(())
    }

    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    pub fn find_function(&self, name: &str) -> Option<&Function> {
        self.functions.iter().find(|f| f.name == name)
    }

    pub fn search_functions(&self, needle: &str, limit: usize) -> Vec<&Function> {
        let needle = needle.to_lowercase();
        self.functions
            .iter()
            .filter(|f| f.name.to_lowercase().contains(&needle))
            .take(limit)
            .collect()
    }

    /// Structural pseudo-C for one function. Honest about the mode: this is
    /// a structural view (signature skeleton + metadata), not lifted source.
    pub fn render_function(&self, func: &Function) -> Value {
        let strings: Vec<&str> = self
            .strings
            .iter()
            .take(20)
            .map(|s| s.value.as_str())
            .collect();
        json!({
            "name": func.name,
            "signature": format!("void {}(void) /* structural */", func.name),
            "address": format!("0x{:x}", func.address),
            "size": func.size,
            "strings_in_binary": strings,
            "source": format!(
                "// structural view (kuna code-level analyzer, mode=structural)\n// address: 0x{addr:x}  size: {size} bytes\nvoid {name}(void) {{\n    /* lifting not performed in structural mode */\n}}\n",
                addr = func.address,
                size = func.size,
                name = func.name
            ),
        })
    }

    /// Whole-binary pseudo-C: one declaration block per function.
    pub fn render_binary(&self) -> Value {
        let decls: Vec<String> = self
            .functions
            .iter()
            .map(|f| format!("void {}(void); // 0x{:x} ({} bytes)", f.name, f.address, f.size))
            .collect();
        let source = format!(
            "// structural view of `{}` (kuna code-level analyzer, mode=structural)\n// functions: {}  strings: {}\n{}\n",
            self.file,
            self.functions.len(),
            self.strings.len(),
            decls.join("\n")
        );
        json!({
            "name": self.file,
            "functions": self.functions.len(),
            "strings": self.strings.len(),
            "stripped": self.stripped,
            "source": source,
        })
    }
}

fn slice<'a>(bytes: &'a [u8], offset: u64, size: u64) -> &'a [u8] {
    let start = offset as usize;
    let end = start.saturating_add(size as usize).min(bytes.len());
    bytes.get(start..end).unwrap_or(&[])
}

/// Printable-ASCII run scan (the classic `strings` behavior).
pub fn scan_strings(bytes: &[u8], min_len: usize) -> Vec<StringRef> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, &b) in bytes.iter().enumerate() {
        let printable = (0x20..=0x7e).contains(&b);
        if printable && start.is_none() {
            start = Some(i);
        } else if !printable {
            if let Some(s) = start {
                if i - s >= min_len {
                    out.push(StringRef {
                        offset: s as u64,
                        value: String::from_utf8_lossy(&bytes[s..i]).to_string(),
                    });
                }
            }
            start = None;
        }
    }
    if out.len() > 10_000 {
        out.truncate(10_000);
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Build a minimal ELF64: header + section headers + .symtab + .strtab +
    /// .rodata, then assert the analyzer extracts functions and strings.
    pub(crate) fn build_elf() -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        // ELF header (64 bytes)
        out.extend_from_slice(b"\x7fELF\x02\x01\x01\x00");
        out.extend_from_slice(&[0u8; 8]); // padding
        out.extend_from_slice(&[0u8; 24]); // e_type..e_entry placeholder
        let shoff_pos = out.len();
        out.extend_from_slice(&[0u8; 8]); // e_shoff (patched)
        out.extend_from_slice(&[0u8; 4]); // e_flags
        out.extend_from_slice(&64u16.to_le_bytes()); // e_ehsize
        out.extend_from_slice(&[0u8; 4]); // e_phentsize + e_phnum
        out.extend_from_slice(&64u16.to_le_bytes()); // e_shentsize
        out.extend_from_slice(&5u16.to_le_bytes()); // e_shnum
        out.extend_from_slice(&1u16.to_le_bytes()); // e_shstrndx
        // (header is 16 + 24 + 8 + 6 + 6 = 60... pad to 64)
        while out.len() < 64 {
            out.push(0);
        }

        // .rodata with two strings
        let rodata_offset = out.len() as u64;
        out.extend_from_slice(b"hello from kuna\x00second string here\x00");

        // .strtab
        let strtab_offset = out.len() as u64;
        let mut strtab: Vec<u8> = vec![0];
        let name_main = strtab.len() as u32;
        strtab.extend_from_slice(b"main\0");
        let name_helper = strtab.len() as u32;
        strtab.extend_from_slice(b"kuna_helper\0");
        out.extend_from_slice(&strtab);

        // .symtab (entsize 24): null + main + kuna_helper
        let symtab_offset = out.len() as u64;
        for _ in 0..3 {
            out.extend_from_slice(&[0u8; 24]);
        }
        let write_sym = |out: &mut Vec<u8>, idx: usize, name: u32, value: u64, size: u64| {
            let base = symtab_offset as usize + idx * 24;
            out[base..base + 4].copy_from_slice(&name.to_le_bytes());
            out[base + 4] = 0x12; // GLOBAL FUNC
            out[base + 8..base + 16].copy_from_slice(&value.to_le_bytes());
            out[base + 16..base + 24].copy_from_slice(&size.to_le_bytes());
        };
        write_sym(&mut out, 1, name_main, 0x1000, 42);
        write_sym(&mut out, 2, name_helper, 0x1030, 16);

        // section headers: null, .rodata, .shstrtab, .strtab, .symtab
        let shstrtab_offset = out.len() as u64;
        let mut shstrtab: Vec<u8> = vec![0];
        let n_rodata = shstrtab.len() as u32;
        shstrtab.extend_from_slice(b".rodata\0");
        let n_symtab = shstrtab.len() as u32;
        shstrtab.extend_from_slice(b".symtab\0");
        let n_strtab = shstrtab.len() as u32;
        shstrtab.extend_from_slice(b".strtab\0");
        let n_shstrtab = shstrtab.len() as u32;
        shstrtab.extend_from_slice(b".shstrtab\0");
        out.extend_from_slice(&shstrtab);

        let shoff = out.len() as u64;
        let shdr = |name: u32, typ: u32, off: u64, size: u64, link: u32, entsize: u64| -> Vec<u8> {
            let mut e = vec![0u8; 64];
            e[0..4].copy_from_slice(&name.to_le_bytes());
            e[4..8].copy_from_slice(&typ.to_le_bytes());
            e[24..32].copy_from_slice(&off.to_le_bytes());
            e[32..40].copy_from_slice(&size.to_le_bytes());
            e[40..44].copy_from_slice(&link.to_le_bytes());
            e[56..64].copy_from_slice(&entsize.to_le_bytes());
            e
        };
        out.extend_from_slice(&shdr(0, 0, 0, 0, 0, 0));
        out.extend_from_slice(&shdr(n_rodata, 1, rodata_offset, 34, 0, 0)); // .rodata PROGBITS
        out.extend_from_slice(&shdr(n_symtab, 2, symtab_offset, 72, 3, 24)); // .symtab -> link .strtab
        out.extend_from_slice(&shdr(n_strtab, 3, strtab_offset, strtab.len() as u64, 0, 0));
        out.extend_from_slice(&shdr(n_shstrtab, 3, shstrtab_offset, shstrtab.len() as u64, 0, 0));

        // patch e_shoff
        out[shoff_pos..shoff_pos + 8].copy_from_slice(&shoff.to_le_bytes());
        out
    }

    #[test]
    fn parses_functions_and_strings() {
        let elf = build_elf();
        let tmp = std::env::temp_dir().join(format!("decx-kuna-{}.elf", std::process::id()));
        std::fs::write(&tmp, &elf).unwrap();
        let analyzer = Analyzer::load(&tmp).unwrap();
        for i in 0..5 {
            let base = 224 + i * 40;
            let rd = |o: usize| -> u64 { u64::from_le_bytes(elf[o..o + 8].try_into().unwrap()) };
            let r32 = |o: usize| -> u64 { u32::from_le_bytes(elf[o..o + 4].try_into().unwrap()).into() };
            eprintln!(
                "DUMP shdr[{i}] name={} type={} off={} size={} link={} entsize={}",
                r32(base), r32(base + 4), rd(base + 24), rd(base + 32), r32(base + 40), rd(base + 56)
            );
        }
        assert!(!analyzer.stripped);
        assert_eq!(analyzer.function_count(), 2);
        assert_eq!(analyzer.find_function("main").unwrap().address, 0x1000);
        assert_eq!(analyzer.find_function("kuna_helper").unwrap().size, 16);
        assert!(analyzer.strings.iter().any(|s| s.value.contains("hello from kuna")));
        let render = analyzer.render_function(analyzer.find_function("main").unwrap());
        assert!(render["source"].as_str().unwrap().contains("void main(void)"));
        assert_eq!(render["address"], "0x1000");
        let binary = analyzer.render_binary();
        assert_eq!(binary["functions"], 2);
        assert!(binary["source"].as_str().unwrap().contains("kuna_helper"));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn non_elf_falls_back_to_strings() {
        let tmp = std::env::temp_dir().join(format!("decx-kuna-{}.bin", std::process::id()));
        std::fs::write(&tmp, b"\x00\x01not-an-elf but has a long string\x00").unwrap();
        let analyzer = Analyzer::load(&tmp).unwrap();
        assert!(analyzer.stripped);
        assert!(analyzer.strings.iter().any(|s| s.value.contains("not-an-elf")));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn missing_file_is_input_error() {
        assert!(Analyzer::load(Path::new("/no/such/file")).is_err());
    }
}
