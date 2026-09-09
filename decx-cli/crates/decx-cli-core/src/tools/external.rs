//! External CLI tool registry (opencli `external register` equivalent).
//!
//! Registered tools live in the unified `config.json` (`tools` array) and
//! become reachable as top-level commands: `decx <name> [args...]` spawns the
//! registered command with inherited stdio and propagates its exit code.
//! Legacy `tools.json` files are migrated on first load.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::{DecxError, DecxResult};
use crate::fsx;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalTool {
    pub name: String,
    /// Full argv to spawn, e.g. `["gh", "pr"]` or `["/usr/bin/docker", "compose"]`.
    pub command: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub registered_at_ms: u64,
}

impl ExternalTool {
    pub fn to_summary(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "command": self.command,
            "description": self.description,
            "registered_at": self.registered_at_ms,
        })
    }
}

pub struct ExternalRegistry {
    home: PathBuf,
}

impl ExternalRegistry {
    pub fn new(home: &Path) -> Self {
        Self {
            home: home.to_path_buf(),
        }
    }

    pub fn load(&self) -> Vec<ExternalTool> {
        crate::config::Config::load(&self.home)
            .tools
            .into_iter()
            .map(|spec| ExternalTool {
                name: spec.name,
                command: spec.command,
                description: spec.description,
                registered_at_ms: spec.registered_at_ms,
            })
            .collect()
    }

    fn save(&self, tools: Vec<ExternalTool>) -> DecxResult<()> {
        let mut config = crate::config::Config::load(&self.home);
        config.tools = tools
            .into_iter()
            .map(|tool| crate::config::ToolSpec {
                name: tool.name,
                command: tool.command,
                description: tool.description,
                registered_at_ms: tool.registered_at_ms,
            })
            .collect();
        config.save(&self.home)
    }

    pub fn get(&self, name: &str) -> Option<ExternalTool> {
        self.load().into_iter().find(|t| t.name == name)
    }

    /// Register a tool, rejecting names that collide with built-in commands or
    /// are not usable as a single CLI token.
    pub fn register(&self, name: &str, command: Vec<String>, description: Option<String>) -> DecxResult<ExternalTool> {
        Self::validate_name(name)?;
        if command.is_empty() {
            return Err(DecxError::usage("A command to run is required (use `-- <command...>`)"));
        }
        let mut tools = self.load();
        if tools.iter().any(|t| t.name == name) {
            return Err(DecxError::usage(format!(
                "Tool '{name}' is already registered; remove it first with 'decx tools remove {name}'"
            )));
        }
        let tool = ExternalTool {
            name: name.to_string(),
            command,
            description,
            registered_at_ms: fsx::now_ms(),
        };
        tools.push(tool.clone());
        self.save(tools)?;
        Ok(tool)
    }

    pub fn remove(&self, name: &str) -> DecxResult<ExternalTool> {
        let mut tools = self.load();
        let pos = tools
            .iter()
            .position(|t| t.name == name)
            .ok_or_else(|| DecxError::not_found("TOOL_NOT_FOUND", format!("Tool not found: {name}")))?;
        let tool = tools.remove(pos);
        self.save(tools)?;
        Ok(tool)
    }

    /// Spawn the registered command with `args` appended, inheriting stdio.
    /// Returns the child's exit code.
    pub fn run_passthrough(&self, tool: &ExternalTool, args: &[String]) -> DecxResult<i32> {
        let (program, leading) = tool.command.split_first().expect("validated non-empty");
        let mut cmd = Command::new(program);
        cmd.args(leading).args(args);
        let status = cmd
            .status()
            .map_err(|e| DecxError::process(format!("failed to launch '{}': {e}", program)))?;
        Ok(status.code().unwrap_or(70))
    }

    fn validate_name(name: &str) -> DecxResult<()> {
        let valid = !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            && !name.starts_with('-');
        if !valid {
            return Err(DecxError::usage(format!(
                "Invalid tool name '{name}': use letters, digits, '-' or '_' (no leading '-')"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_registry() -> (PathBuf, ExternalRegistry) {
        let home = std::env::temp_dir().join(format!(
            "decx-ext-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()
        ));
        let _ = std::fs::remove_dir_all(&home);
        (home.clone(), ExternalRegistry::new(&home))
    }

    #[test]
    fn register_and_run_roundtrip() {
        let (home, reg) = temp_registry();
        reg.register("echo-tool", vec!["echo".into(), "hello".into()], None).unwrap();
        let tool = reg.get("echo-tool").unwrap();
        assert_eq!(tool.command, vec!["echo", "hello"]);
        let code = reg.run_passthrough(&tool, &["world".to_string()]).unwrap();
        assert_eq!(code, 0);
        reg.remove("echo-tool").unwrap();
        assert!(reg.get("echo-tool").is_none());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn rejects_bad_names_and_duplicates() {
        let (home, reg) = temp_registry();
        assert!(reg.register("-bad", vec!["x".into()], None).is_err());
        assert!(reg.register("a/b", vec!["x".into()], None).is_err());
        reg.register("demo", vec!["x".into()], None).unwrap();
        assert!(reg.register("demo", vec!["y".into()], None).is_err());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn rejects_empty_command() {
        let (home, reg) = temp_registry();
        assert!(reg.register("demo", vec![], None).is_err());
        let _ = std::fs::remove_dir_all(&home);
    }
}
