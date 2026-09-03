//! DEX `debug_info_item` parser (parameter and local variable names).

use std::collections::HashMap;

use crate::error::{DexError, Result};
use crate::leb128::{read_sleb128, read_uleb128, read_uleb128p1};

/// DBG_* opcodes from the Android DEX format.
const DBG_END_SEQUENCE: u8 = 0x00;
const DBG_ADVANCE_PC: u8 = 0x01;
const DBG_ADVANCE_LINE: u8 = 0x02;
const DBG_START_LOCAL: u8 = 0x03;
const DBG_START_LOCAL_EXTENDED: u8 = 0x04;
const DBG_END_LOCAL: u8 = 0x05;
const DBG_RESTART_LOCAL: u8 = 0x06;
const DBG_SET_PROLOGUE_END: u8 = 0x07;
const DBG_SET_EPILOGUE_BEGIN: u8 = 0x08;
const DBG_SET_FILE: u8 = 0x09;

/// Parsed debug information for one method.
#[derive(Clone, Debug, Default)]
pub struct DebugInfo {
    pub line_start: u32,
    /// Parameter names in declaration order (`None` = unnamed).
    pub parameter_names: Vec<Option<String>>,
    /// Best known local name per register (from START_LOCAL / RESTART_LOCAL).
    pub register_names: HashMap<u32, String>,
}

impl DebugInfo {
    /// Name for register `reg`, if known.
    pub fn name_for_reg(&self, reg: u32) -> Option<&str> {
        self.register_names.get(&reg).map(|s| s.as_str())
    }
}

/// Parse `debug_info_item` at `debug_info_off` within `data`.
///
/// `get_string` resolves string_id indices to UTF-8 names.
/// Callers should skip invoking this when `debug_info_off == 0` (no debug item).
pub fn parse_debug_info(
    data: &[u8],
    debug_info_off: u32,
    get_string: &dyn Fn(u32) -> Result<String>,
) -> Result<DebugInfo> {
    let mut off = debug_info_off as usize;
    if off >= data.len() {
        return Err(DexError::Truncated("debug_info_off".into()));
    }

    let (line_start, n) =
        read_uleb128(data, off).ok_or(DexError::Truncated("debug line_start".into()))?;
    off += n;
    let (parameters_size, n) =
        read_uleb128(data, off).ok_or(DexError::Truncated("debug parameters_size".into()))?;
    off += n;

    let mut parameter_names = Vec::with_capacity(parameters_size as usize);
    for _ in 0..parameters_size {
        let (name_idx_p1, n) =
            read_uleb128p1(data, off).ok_or(DexError::Truncated("debug param name".into()))?;
        off += n;
        let name = if name_idx_p1 < 0 {
            None
        } else {
            get_string(name_idx_p1 as u32).ok()
        };
        parameter_names.push(name);
    }

    let mut address: u32 = 0;
    let mut register_names: HashMap<u32, String> = HashMap::new();
    // Remember last ended local so RESTART_LOCAL can revive it.
    let mut last_local: HashMap<u32, String> = HashMap::new();

    loop {
        if off >= data.len() {
            break;
        }
        let opcode = data[off];
        off += 1;
        match opcode {
            DBG_END_SEQUENCE => break,
            DBG_ADVANCE_PC => {
                let (diff, n) =
                    read_uleb128(data, off).ok_or(DexError::Truncated("DBG_ADVANCE_PC".into()))?;
                off += n;
                address = address.saturating_add(diff);
            }
            DBG_ADVANCE_LINE => {
                let (_diff, n) =
                    read_sleb128(data, off).ok_or(DexError::Truncated("DBG_ADVANCE_LINE".into()))?;
                off += n;
            }
            DBG_START_LOCAL | DBG_START_LOCAL_EXTENDED => {
                let (reg, n) =
                    read_uleb128(data, off).ok_or(DexError::Truncated("DBG_START_LOCAL reg".into()))?;
                off += n;
                let (name_idx_p1, n) = read_uleb128p1(data, off)
                    .ok_or(DexError::Truncated("DBG_START_LOCAL name".into()))?;
                off += n;
                let (_type_idx_p1, n) = read_uleb128p1(data, off)
                    .ok_or(DexError::Truncated("DBG_START_LOCAL type".into()))?;
                off += n;
                if opcode == DBG_START_LOCAL_EXTENDED {
                    let (_sig, n) = read_uleb128p1(data, off)
                        .ok_or(DexError::Truncated("DBG_START_LOCAL_EXTENDED sig".into()))?;
                    off += n;
                }
                if name_idx_p1 >= 0 {
                    if let Ok(name) = get_string(name_idx_p1 as u32) {
                        if !name.is_empty() {
                            last_local.insert(reg, name.clone());
                            register_names.insert(reg, name);
                        }
                    }
                }
                let _ = address;
            }
            DBG_END_LOCAL => {
                let (reg, n) =
                    read_uleb128(data, off).ok_or(DexError::Truncated("DBG_END_LOCAL".into()))?;
                off += n;
                // Keep name in register_names for naming; track for restart.
                if let Some(name) = register_names.get(&reg).cloned() {
                    last_local.insert(reg, name);
                }
            }
            DBG_RESTART_LOCAL => {
                let (reg, n) = read_uleb128(data, off)
                    .ok_or(DexError::Truncated("DBG_RESTART_LOCAL".into()))?;
                off += n;
                if let Some(name) = last_local.get(&reg).cloned() {
                    register_names.insert(reg, name);
                }
            }
            DBG_SET_PROLOGUE_END | DBG_SET_EPILOGUE_BEGIN => {}
            DBG_SET_FILE => {
                let (_name_idx_p1, n) =
                    read_uleb128p1(data, off).ok_or(DexError::Truncated("DBG_SET_FILE".into()))?;
                off += n;
            }
            _ if opcode >= 0x0a => {
                // Special opcodes adjust line + address; we only care about names.
                let adjusted = opcode - 0x0a;
                let addr_diff = (adjusted / 15) as u32;
                address = address.saturating_add(addr_diff);
            }
            _ => {
                // Unknown opcode — stop safely.
                break;
            }
        }
    }

    Ok(DebugInfo {
        line_start,
        parameter_names,
        register_names,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_end_sequence_only() {
        // line_start=1, parameters_size=0, END_SEQUENCE
        let dbg = parse_debug_info(&[0x01, 0x00, 0x00], 0, &|_| Ok(String::new())).unwrap();
        assert!(dbg.parameter_names.is_empty());
        assert!(dbg.register_names.is_empty());
    }

    #[test]
    fn parse_params_and_start_local() {
        // line_start=1, parameters_size=1, param name string_idx=0,
        // START_LOCAL reg=1 name=0 type=-1, END_SEQUENCE
        let mut data = Vec::new();
        data.push(0x01); // line_start
        data.push(0x01); // parameters_size
        data.push(0x01); // uleb128p1 → string 0
        data.push(DBG_START_LOCAL);
        data.push(0x01); // reg 1
        data.push(0x01); // name string 0
        data.push(0x00); // type no-index (uleb128p1 of 0 → -1)
        data.push(DBG_END_SEQUENCE);

        let get_string = |idx: u32| -> Result<String> {
            match idx {
                0 => Ok("count".into()),
                _ => Err(DexError::Parse(format!("string {idx}"))),
            }
        };
        let dbg = parse_debug_info(&data, 0, &get_string).unwrap();
        assert_eq!(dbg.parameter_names, vec![Some("count".into())]);
        assert_eq!(dbg.name_for_reg(1), Some("count"));
    }
}
