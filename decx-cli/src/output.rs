//! Unified output formatting: JSON (default, AI-facing) or table (human-facing).
//!
//! Every tool returns `serde_json::Value`; the formatter renders it to stdout.
//! Errors and progress notices always go to stderr so stdout stays
//! machine-parseable.

use serde_json::Value;

use crate::error::DecxError;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Json,
    Table,
}

impl OutputFormat {
    pub fn parse(value: &str) -> Result<Self, DecxError> {
        match value {
            "json" => Ok(Self::Json),
            "table" => Ok(Self::Table),
            other => Err(DecxError::usage(format!(
                "Invalid --format '{other}' (expected json or table)"
            ))),
        }
    }
}

pub struct Formatter {
    pub format: OutputFormat,
}

impl Default for Formatter {
    fn default() -> Self {
        Self {
            format: OutputFormat::Json,
        }
    }
}

impl Formatter {
    pub fn new(format: OutputFormat) -> Self {
        Self { format }
    }

    /// Print a successful result to stdout.
    pub fn output(&self, data: &Value) {
        match self.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(data).unwrap_or_default());
            }
            OutputFormat::Table => {
                println!("{}", render_table(data));
            }
        }
    }

    /// Print an error notice to stderr (stdout stays clean).
    pub fn error(&self, err: &DecxError) {
        eprintln!("  [ERR] {} ({})", err.message, err.code);
        if std::env::var("DECX_DEBUG").ok().as_deref() == Some("1") {
            eprintln!("  [DEBUG] {}", err.to_json());
        }
    }

    /// Print a progress / heartbeat notice to stderr.
    pub fn notice(&self, msg: &str) {
        eprintln!("  {msg}");
    }
}

/// Render a JSON value as a plain-text table.
///
/// - Array of objects: one column per key (union, first-seen order), one row per item.
/// - Flat object: `key | value` rows.
/// - Scalars: printed as-is.
pub fn render_table(data: &Value) -> String {
    match data {
        Value::Array(items) if !items.is_empty() => {
            let rows: Vec<&Value> = items.iter().collect();
            if rows.iter().all(|r| matches!(r, Value::Object(_))) {
                let mut cols: Vec<String> = Vec::new();
                for row in &rows {
                    if let Value::Object(map) = row {
                        for key in map.keys() {
                            if !cols.iter().any(|c| c == key) {
                                cols.push(key.clone());
                            }
                        }
                    }
                }
                let cells: Vec<Vec<String>> = rows
                    .iter()
                    .map(|row| {
                        cols.iter()
                            .map(|c| cell_string(row.get(c.as_str()).unwrap_or(&Value::Null)))
                            .collect()
                    })
                    .collect();
                let header: Vec<String> = cols.clone();
                return grid(&header, &cells);
            }
            items
                .iter()
                .map(|v| cell_string(v))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Value::Object(map) if !map.is_empty() => {
            let rows: Vec<(String, String)> = map
                .iter()
                .map(|(k, v)| (k.clone(), cell_string(v)))
                .collect();
            let header = vec!["key".to_string(), "value".to_string()];
            let cells: Vec<Vec<String>> = rows
                .into_iter()
                .map(|(k, v)| vec![k, v])
                .collect();
            grid(&header, &cells)
        }
        other => cell_string(other),
    }
}

fn cell_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Array(items) if items.len() <= 5 && items.iter().all(|i| i.is_string()) => {
            items
                .iter()
                .map(cell_string)
                .collect::<Vec<_>>()
                .join(", ")
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn grid(header: &[String], rows: &[Vec<String>]) -> String {
    let ncols = header.len();
    let mut widths: Vec<usize> = header.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate().take(ncols) {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let line = |l: &str, m: &str, r: &str| {
        let mut out = String::from(l);
        for (i, w) in widths.iter().enumerate() {
            if i > 0 {
                out.push_str(m);
            }
            out.push_str(&"-".repeat(w + 2));
        }
        out.push_str(r);
        out
    };
    let row_str = |row: &[String]| {
        let mut out = String::from("|");
        for (i, w) in widths.iter().enumerate() {
            let cell = row.get(i).map(String::as_str).unwrap_or("");
            out.push(' ');
            out.push_str(cell);
            out.push_str(&" ".repeat(w - cell.chars().count() + 1));
            out.push('|');
        }
        out
    };
    let mut out = Vec::new();
    out.push(line("+", "+", "+"));
    out.push(row_str(&header.to_vec()));
    out.push(line("+", "+", "+"));
    for row in rows {
        out.push(row_str(row));
    }
    out.push(line("+", "+", "+"));
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn format_parsing() {
        assert_eq!(OutputFormat::parse("json").unwrap(), OutputFormat::Json);
        assert_eq!(OutputFormat::parse("table").unwrap(), OutputFormat::Table);
        assert!(OutputFormat::parse("xml").is_err());
    }

    #[test]
    fn table_renders_array_of_objects() {
        let data = json!([
            { "name": "alpha", "port": 30001 },
            { "name": "beta", "port": 30002 }
        ]);
        let table = render_table(&data);
        assert!(table.contains("name"));
        assert!(table.contains("alpha"));
        assert!(table.contains("30002"));
        // every line has the same length
        let widths: std::collections::HashSet<usize> =
            table.lines().map(|l| l.chars().count()).collect();
        assert_eq!(widths.len(), 1, "grid lines must be aligned: {table}");
    }

    #[test]
    fn table_renders_flat_object_as_key_value_rows() {
        let data = json!({ "ok": true, "port": 25419 });
        let table = render_table(&data);
        assert!(table.contains("key"));
        assert!(table.contains("25419"));
    }
}
