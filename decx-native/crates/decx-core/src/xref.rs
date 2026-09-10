//! Global cross-DEX index: single pass over all code items, keyed by full names.

use crate::code::walk;
use crate::dex::Dex;
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct CallSite {
    pub caller: String,   // full signature of caller method
    pub pc: usize,
    pub kind: &'static str,
}

#[derive(Clone)]
pub struct FieldSite {
    pub accessor: String,
    pub pc: usize,
    pub kind: &'static str, // read | write
}

#[derive(Clone)]
pub struct StrSite {
    pub method: String,
    pub pc: usize,
}

#[derive(Clone)]
pub struct TypeSite {
    pub method: String,
    pub pc: usize,
    pub kind: &'static str, // new | cast | instance-of | array | class-const
}

#[derive(Default)]
pub struct XrefIndex {
    /// callee full sig -> call sites
    pub method_callers: BTreeMap<String, Vec<CallSite>>,
    /// field "Lcls;->name:T" -> access sites
    pub field_accessors: BTreeMap<String, Vec<FieldSite>>,
    /// dex string pool string -> methods using it via const-string
    pub string_refs: BTreeMap<String, Vec<StrSite>>,
    /// class descriptor -> usage sites
    pub type_refs: BTreeMap<String, Vec<TypeSite>>,
    /// caller full sig -> callees (reverse of method_callers)
    pub callees_of: BTreeMap<String, Vec<String>>,
}

impl XrefIndex {
    /// One pass over every method body of every dex.
    pub fn build(dexes: &[Dex]) -> XrefIndex {
        let mut ix = XrefIndex::default();
        for dex in dexes {
            for def in &dex.class_defs {
                let cd = dex.class_data(def);
                let owner = dex.type_descriptor(def.class_idx);
                for section in [&cd.direct_methods, &cd.virtual_methods] {
                    for m in section {
                        let caller = dex.method_full(m.method_idx);
                        let Some(code) = dex.code_item(m.code_off) else {
                            continue;
                        };
                        for insn in walk(&code.insns) {
                            if let Some(mid) = insn.method_idx {
                                let callee = dex.method_full(mid);
                                ix.method_callers.entry(callee.clone()).or_default().push(CallSite {
                                    caller: caller.clone(),
                                    pc: insn.pc,
                                    kind: insn.invoke_kind.unwrap_or("invoke"),
                                });
                                let v = ix.callees_of.entry(caller.clone()).or_default();
                                if !v.contains(&callee) {
                                    v.push(callee);
                                }
                            }
                            if let Some(fid) = insn.field_idx {
                                let field = dex.field_full(fid);
                                let kind = match insn.name {
                                    n if n.starts_with("sget") || n.starts_with("iget") => "read",
                                    _ => "write",
                                };
                                ix.field_accessors.entry(field).or_default().push(FieldSite {
                                    accessor: caller.clone(),
                                    pc: insn.pc,
                                    kind,
                                });
                            }
                            if let Some(sid) = insn.str_idx {
                                let s = dex.str(sid);
                                ix.string_refs.entry(s.to_string()).or_default().push(StrSite {
                                    method: caller.clone(),
                                    pc: insn.pc,
                                });
                            }
                            if let Some(tid) = insn.type_idx {
                                let t = dex.type_descriptor(tid);
                                let kind = match insn.name {
                                    "new-instance" => "new",
                                    "check-cast" => "cast",
                                    "instance-of" => "instance-of",
                                    "new-array" => "array",
                                    "const-class" => "class-const",
                                    _ => "type",
                                };
                                ix.type_refs.entry(t.to_string()).or_default().push(TypeSite {
                                    method: caller.clone(),
                                    pc: insn.pc,
                                    kind,
                                });
                            }
                        }
                        let _ = owner;
                    }
                }
            }
        }
        ix
    }

    pub fn callers_of(&self, method: &str) -> &[CallSite] {
        self.method_callers.get(method).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn callees_of(&self, method: &str) -> &[String] {
        self.callees_of.get(method).map(|v| v.as_slice()).unwrap_or(&[])
    }
}
