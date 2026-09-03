//! Minimal method IR (jadx-style foundation).
//!
//! This is intentionally small and permissive: unhandled cases can fall back to `Raw`.

use std::collections::HashMap;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VarId {
    pub reg: u32,
    /// SSA version. Version 0 is the "unversioned" base used by the current pipeline.
    pub ver: u32,
}

impl VarId {
    pub fn new(reg: u32, ver: u32) -> Self {
        Self { reg, ver }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Var(VarId), // vN or vN_k in SSA
    /// Java-style call expression: `target(args)`
    Call {
        target: String,
        args: String,
    },
    /// Placeholder for move-result; merged by InvokeChainPass with preceding Call.
    PendingResult,
    Raw(String),
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stmt {
    Assign {
        dst: VarId,
        rhs: Expr,
        comment: Option<String>,
    },
    Expr {
        expr: Expr,
        comment: Option<String>,
    },
    Return {
        value: Option<Expr>,
        comment: Option<String>,
    },
    /// SSA φ-node (not emitted as Java; stripped after renaming).
    Phi {
        dst: VarId,
        /// (predecessor block id, value from that predecessor)
        incomings: Vec<(usize, VarId)>,
    },
    Raw(String),
}

impl Expr {
    pub fn to_java(&self) -> String {
        self.to_java_impl(None)
    }

    /// Emit Java with optional variable name map (VarId -> display name).
    pub fn to_java_with_names(&self, names: Option<&HashMap<VarId, String>>) -> String {
        self.to_java_impl(names)
    }

    fn to_java_impl(&self, names: Option<&HashMap<VarId, String>>) -> String {
        match self {
            Expr::Var(v) => {
                if let Some(ns) = names {
                    if let Some(n) = ns.get(v) {
                        return n.clone();
                    }
                }
                if v.ver == 0 {
                    format!("v{}", v.reg)
                } else {
                    format!("v{}_{}", v.reg, v.ver)
                }
            }
            Expr::Call { target, args } => {
                let target_str = names
                    .map(|n| substitute_names_in_text(target, n))
                    .unwrap_or_else(|| target.clone());
                let args_str = names
                    .map(|n| substitute_names_in_text(args, n))
                    .unwrap_or_else(|| args.clone());
                format!("{}({})", target_str, args_str)
            }
            Expr::PendingResult => "<result>".to_string(),
            Expr::Raw(s) => names
                .map(|n| substitute_names_in_text(s, n))
                .unwrap_or_else(|| s.clone()),
        }
    }
}

impl Stmt {
    pub fn to_java_line(&self) -> String {
        self.to_java_line_impl(None)
    }

    /// Emit Java line with optional variable name map.
    pub fn to_java_line_with_names(&self, names: Option<&HashMap<VarId, String>>) -> String {
        self.to_java_line_impl(names)
    }

    fn to_java_line_impl(&self, names: Option<&HashMap<VarId, String>>) -> String {
        match self {
            Stmt::Assign { dst, rhs, comment } => {
                let dst_str = names
                    .and_then(|n| n.get(dst).cloned())
                    .unwrap_or_else(|| Expr::Var(*dst).to_java());
                let rhs_str = rhs.to_java_impl(names);
                let base = format!("{} = {};", dst_str, rhs_str);
                append_comment(base, comment.as_deref())
            }
            Stmt::Expr { expr, comment } => {
                let base = format!("{};", expr.to_java_impl(names));
                append_comment(base, comment.as_deref())
            }
            Stmt::Return { value, comment } => {
                let base = match value {
                    None => "return;".to_string(),
                    Some(v) => format!("return {};", v.to_java_impl(names)),
                };
                append_comment(base, comment.as_deref())
            }
            Stmt::Phi { .. } => String::new(),
            Stmt::Raw(s) => names
                .map(|n| substitute_names_in_text(s, n))
                .unwrap_or_else(|| s.clone()),
        }
    }
}

/// Replace variable references (vN or vN_k) in text with names from the map.
pub fn substitute_names_in_text_pub(s: &str, names: &HashMap<VarId, String>) -> String {
    substitute_names_in_text(s, names)
}

fn substitute_names_in_text(s: &str, names: &HashMap<VarId, String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'v' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            let start = i;
            i += 1;
            let mut reg: u32 = 0;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                reg = reg * 10 + (bytes[i] - b'0') as u32;
                i += 1;
            }
            let ver = if i + 1 < bytes.len() && bytes[i] == b'_' && bytes[i + 1].is_ascii_digit() {
                i += 1;
                let mut v: u32 = 0;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    v = v * 10 + (bytes[i] - b'0') as u32;
                    i += 1;
                }
                v
            } else {
                0
            };
            let vid = VarId::new(reg, ver);
            let resolved = names.get(&vid).or_else(|| {
                if ver != 0 {
                    return None;
                }
                names
                    .iter()
                    .filter(|(candidate, _)| candidate.reg == reg)
                    .max_by_key(|(candidate, _)| candidate.ver)
                    .map(|(_, name)| name)
            });
            if let Some(name) = resolved {
                out.push_str(name);
            } else {
                out.push_str(&s[start..i]);
            }
            continue;
        }
        let c = s[i..].chars().next().unwrap_or('\0');
        out.push(c);
        i += c.len_utf8();
    }
    out
}

fn append_comment(base: String, _comment: Option<&str>) -> String {
    base
}
