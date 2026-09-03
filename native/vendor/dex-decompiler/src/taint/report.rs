//! JSON issue report (SAPP-friendly subset).

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::issue::Issue;
use crate::error::{DexDecompilerError, Result};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IssueReport {
    pub tool: String,
    pub version: String,
    pub issues: Vec<Issue>,
    pub stats: ReportStats,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReportStats {
    pub methods_analyzed: usize,
    pub call_edges: usize,
    pub issues: usize,
    pub iterations: usize,
}

pub fn write_issues_json(report: &IssueReport, path: &Path) -> Result<()> {
    let text = serde_json::to_string_pretty(report)
        .map_err(|e| DexDecompilerError::Decompilation(format!("serialize issues: {e}")))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| DexDecompilerError::Decompilation(format!("mkdir: {e}")))?;
    }
    std::fs::write(path, text)
        .map_err(|e| DexDecompilerError::Decompilation(format!("write issues: {e}")))?;
    Ok(())
}
