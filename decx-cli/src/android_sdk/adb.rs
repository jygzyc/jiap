//! adb interaction and output parsers (port of android/adb.ts).

use std::process::Command;
use std::time::Duration;

use serde_json::{json, Value};

use crate::error::{DecxError, DecxResult};

pub const SUPPORTED_FRAMEWORK_OEMS: [&str; 6] = ["vivo", "oppo", "xiaomi", "honor", "google", "samsung"];

/// Parse `adb devices` output into serials in the `device` state.
pub fn parse_adb_devices_output(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("List of devices attached"))
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let serial = parts.next()?.to_string();
            let state = parts.next()?.to_string();
            (state == "device").then_some(serial)
        })
        .collect()
}

/// Resolve the target serial: explicit request wins, otherwise require exactly
/// one connected device.
pub fn resolve_preferred_serial(output: &str, requested: Option<&str>) -> DecxResult<String> {
    if let Some(serial) = requested {
        return Ok(serial.to_string());
    }
    let devices = parse_adb_devices_output(output);
    match devices.len() {
        0 => Err(DecxError::not_found(
            "ADB_DEVICE_MISSING",
            "No connected Android device detected via adb",
        )),
        1 => Ok(devices[0].clone()),
        _ => Err(DecxError::new(
            "ADB_DEVICE_AMBIGUOUS",
            format!(
                "Multiple adb devices detected ({}). Use --serial to select one.",
                devices.join(", ")
            ),
            crate::error::EX_USAGE,
        )),
    }
}

/// Detect whether a device brand is a supported framework-collection OEM.
pub fn detect_framework_oem_from_brand(brand: &str) -> DecxResult<String> {
    let normalized = brand.trim().to_lowercase();
    if SUPPORTED_FRAMEWORK_OEMS.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(DecxError::new(
            "ADB_UNSUPPORTED_OEM",
            format!("Unsupported device OEM '{brand}'. Supported: {}", SUPPORTED_FRAMEWORK_OEMS.join(", ")),
            crate::error::EX_UNAVAILABLE,
        ))
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemServiceInfo {
    pub index: u64,
    pub name: String,
    pub interfaces: Vec<String>,
}

/// Parse `adb shell service list` output
/// (`Found N services:` header + `index\tname: [iface, iface]` lines).
pub fn parse_system_services_output(output: &str) -> (u64, Vec<SystemServiceInfo>) {
    let lines: Vec<&str> = output.lines().map(|l| l.trim_end()).filter(|l| !l.trim().is_empty()).collect();
    let total_from_header = lines
        .first()
        .and_then(|l| {
            l.strip_prefix("Found ")
                .and_then(|rest| rest.strip_suffix(" services:"))
                .and_then(|num| num.trim().parse::<u64>().ok())
        })
        .is_some();
    let services: Vec<SystemServiceInfo> = lines
        .iter()
        .skip(if total_from_header { 1 } else { 0 })
        .filter_map(|line| {
            let (index, rest) = line.split_once('\t')?;
            let (name, ifaces) = rest.split_once(": ")?;
            let index = index.trim().parse().ok()?;
            let interfaces = ifaces
                .trim_matches(|c| c == '[' || c == ']')
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            Some(SystemServiceInfo {
                index,
                name: name.to_string(),
                interfaces,
            })
        })
        .collect();
    let total = if total_from_header {
        lines
            .first()
            .and_then(|l| {
                l.strip_prefix("Found ")
                    .and_then(|rest| rest.strip_suffix(" services:"))
                    .and_then(|num| num.trim().parse::<u64>().ok())
            })
            .unwrap_or(services.len() as u64)
    } else {
        services.len() as u64
    };
    (total, services)
}

/// Case-insensitive substring filter over service names and interfaces.
pub fn filter_system_services(total: u64, services: &[SystemServiceInfo], keyword: Option<&str>) -> Value {
    let Some(keyword) = keyword.map(str::trim).filter(|k| !k.is_empty()) else {
        return json!({ "total": total, "services": services });
    };
    let kw = keyword.to_lowercase();
    let filtered: Vec<&SystemServiceInfo> = services
        .iter()
        .filter(|s| s.name.to_lowercase().contains(&kw) || s.interfaces.iter().any(|i| i.to_lowercase().contains(&kw)))
        .collect();
    json!({ "total": filtered.len(), "services": filtered })
}

/// The device-side shell pipeline `pm list permissions -f | grep -A 5 -F -- '<perm>'`.
pub fn build_permission_info_command(permission: &str) -> String {
    let quoted = format!("'{}'", permission.replace('\'', "'\"'\"'"));
    format!("pm list permissions -f | grep -A 5 -F -- {quoted} || true")
}

/// Parse `pm list permissions -f` grep output into one permission record.
/// Returns `None` when the permission block is absent. Field lines look like
/// `+ group:...` / `label:...`; a leading `+` (or `+ `) is stripped so both
/// device output styles produce the same keys.
pub fn parse_permission_info_output(output: &str, permission: &str) -> Option<Value> {
    let normalized = permission.trim();
    let lines: Vec<&str> = output.lines().map(|l| l.trim_end()).filter(|l| !l.trim().is_empty()).collect();
    let header = format!("+ permission:{normalized}");
    let start = lines.iter().position(|l| {
        *l == header || *l == format!("permission:{normalized}")
    })?;
    let mut info = serde_json::Map::new();
    info.insert("permission".into(), json!(normalized));
    for line in lines.iter().skip(start + 1) {
        if line.starts_with("+ permission:") || line.starts_with("permission:") {
            break;
        }
        let field = line.trim_start().strip_prefix('+').map(str::trim_start).unwrap_or(line);
        let Some((key, value)) = field.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let mut value = value.trim().to_string();
        if value == "null" {
            value = String::new();
        }
        if !key.is_empty() {
            info.insert(key.to_string(), json!(value));
        }
    }
    Some(Value::Object(info))
}

/// Minimal adb client (sync spawns, like adb.ts).
pub struct AdbClient {
    pub adb_path: String,
    pub serial: Option<String>,
    selected: Option<String>,
}

impl AdbClient {
    pub fn new(adb_path: Option<String>, serial: Option<String>) -> Self {
        Self {
            adb_path: adb_path.filter(|p| !p.is_empty()).unwrap_or_else(|| "adb".to_string()),
            serial,
            selected: None,
        }
    }

    fn run(&self, args: &[&str], timeout: Duration) -> DecxResult<(String, String, Option<i32>)> {
        let mut cmd = Command::new(&self.adb_path);
        if let Some(serial) = self.selected.as_deref().or(self.serial.as_deref()) {
            cmd.args(["-s", serial]);
        }
        cmd.args(args);
        let output = wait_with_timeout(&mut cmd, timeout)?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Ok((stdout, stderr, output.status.code()))
    }

    pub fn ensure_available(&self) -> DecxResult<()> {
        let (stdout, stderr, status) = self.run(&["version"], Duration::from_secs(10))?;
        if status != Some(0) {
            return Err(DecxError::process(
                if stderr.trim().is_empty() { stdout.trim() } else { stderr.trim() }.to_string(),
            ));
        }
        Ok(())
    }

    fn select_device(&mut self) -> DecxResult<String> {
        let (stdout, _, _) = self.run(&["devices"], Duration::from_secs(10))?;
        let serial = resolve_preferred_serial(&stdout, self.serial.as_deref())?;
        self.selected = Some(serial.clone());
        Ok(serial)
    }

    pub fn ensure_device_connected(&mut self) -> DecxResult<String> {
        let serial = self.select_device()?;
        let (stdout, _, status) = self.run(&["get-state"], Duration::from_secs(10))?;
        if status != Some(0) || !stdout.contains("device") {
            return Err(DecxError::not_found(
                "ADB_DEVICE_MISSING",
                "No connected Android device detected via adb",
            ));
        }
        Ok(serial)
    }

    pub fn shell(&mut self, command: &str, timeout: Duration) -> DecxResult<String> {
        let (stdout, stderr, status) = self.run(&["shell", command], timeout)?;
        if status != Some(0) {
            return Err(DecxError::process(
                if stderr.trim().is_empty() { stdout.trim() } else { stderr.trim() }.to_string(),
            ));
        }
        Ok(stdout)
    }

    pub fn list_system_services(&mut self) -> DecxResult<(u64, Vec<SystemServiceInfo>)> {
        self.ensure_device_connected()?;
        let out = self.shell("service list", Duration::from_secs(10))?;
        Ok(parse_system_services_output(&out))
    }

    pub fn permission_info(&mut self, permission: &str) -> DecxResult<Value> {
        let normalized = permission.trim().to_string();
        if normalized.is_empty() {
            return Err(DecxError::new("ADB_PERMISSION_REQUIRED", "Permission name is required", crate::error::EX_USAGE));
        }
        self.ensure_device_connected()?;
        let out = self.shell(&build_permission_info_command(&normalized), Duration::from_secs(10))?;
        parse_permission_info_output(&out, &normalized).ok_or_else(|| {
            DecxError::not_found("ADB_PERMISSION_NOT_FOUND", format!("Permission '{normalized}' not found"))
        })
    }

    pub fn get_prop(&mut self, name: &str) -> DecxResult<String> {
        Ok(self.shell(&format!("getprop {name}"), Duration::from_secs(10))?.trim().to_string())
    }

    /// Pull one remote file to `local_path` (framework collection).
    pub fn pull(&mut self, remote: &str, local: &str) -> DecxResult<()> {
        let (stdout, stderr, status) = self.run(&["pull", remote, local], Duration::from_secs(600))?;
        if status != Some(0) {
            return Err(DecxError::process(
                if stderr.trim().is_empty() { stdout.trim() } else { stderr.trim() }.to_string(),
            ));
        }
        Ok(())
    }

    /// Framework OEM derived from `ro.product.brand` (see
    /// `detect_framework_oem_from_brand`).
    pub fn framework_oem(&mut self) -> DecxResult<String> {
        let brand = self.get_prop("ro.product.brand")?;
        if brand.is_empty() {
            return Err(DecxError::not_found(
                "ADB_BRAND_UNKNOWN",
                "Device did not report ro.product.brand; pass --oem explicitly",
            ));
        }
        detect_framework_oem_from_brand(&brand)
    }

    /// Device model string used as the artifact vendor segment.
    pub fn device_model(&mut self) -> DecxResult<String> {
        self.get_prop("ro.product.model")
    }
}

fn wait_with_timeout(cmd: &mut Command, timeout: Duration) -> DecxResult<std::process::Output> {
    // Synchronous spawn; adb commands here are short-lived, so we rely on the
    // OS rather than a watchdog thread. Framework pulls go through `pull`,
    // which is equally synchronous (adb does the transfer).
    let _ = timeout;
    cmd.output()
        .map_err(|e| DecxError::process(format!("failed to execute adb: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_parsing() {
        let out = "List of devices attached\nABC123\tdevice\nXYZ\toffline\nDEF\tdevice\n";
        assert_eq!(parse_adb_devices_output(out), vec!["ABC123", "DEF"]);
    }

    #[test]
    fn serial_resolution() {
        assert_eq!(resolve_preferred_serial("X\tdevice\n", Some("X")).unwrap(), "X");
        assert!(resolve_preferred_serial("", None).is_err());
        let two = resolve_preferred_serial("A\tdevice\nB\tdevice\n", None).unwrap_err();
        assert_eq!(two.code, "ADB_DEVICE_AMBIGUOUS");
    }

    #[test]
    fn oem_detection() {
        assert_eq!(detect_framework_oem_from_brand("vivo").unwrap(), "vivo");
        assert_eq!(detect_framework_oem_from_brand(" VIVO ").unwrap(), "vivo");
        assert!(detect_framework_oem_from_brand("acme").is_err());
    }

    #[test]
    fn service_list_parsing() {
        let out = "Found 2 services:\n1\twifi: [android.net.wifi.IWifi]\n2\tactivity: []\n";
        let (total, services) = parse_system_services_output(out);
        assert_eq!(total, 2);
        assert_eq!(services[0].name, "wifi");
        assert_eq!(services[0].interfaces, vec!["android.net.wifi.IWifi"]);
        assert!(services[1].interfaces.is_empty());

        // without header
        let (total, services) = parse_system_services_output("1\twifi: [a.B]\n");
        assert_eq!(total, 1);
        assert_eq!(services.len(), 1);
    }

    #[test]
    fn service_grep_filter() {
        let (_, services) = parse_system_services_output("1\twifi: [a.B]\n2\tbattery: []\n");
        let filtered = filter_system_services(2, &services, Some("WIFI"));
        assert_eq!(filtered["total"], 1);
        let none = filter_system_services(2, &services, Some("zzz"));
        assert_eq!(none["total"], 0);
        let all = filter_system_services(2, &services, None);
        assert_eq!(all["total"], 2);
    }

    #[test]
    fn permission_command_quoting() {
        assert_eq!(
            build_permission_info_command("android.permission.INTERNET"),
            "pm list permissions -f | grep -A 5 -F -- 'android.permission.INTERNET' || true"
        );
    }

    #[test]
    fn permission_block_parsing() {
        let out = "+ permission:android.permission.INTERNET\n+ group:null\n+ label:Network\nfoo without colon\n+ permission:OTHER\n";
        let info = parse_permission_info_output(out, "android.permission.INTERNET").unwrap();
        assert_eq!(info["permission"], "android.permission.INTERNET");
        assert_eq!(info["label"], "Network");
        assert!(info.get("group").and_then(Value::as_str).map(str::is_empty).unwrap_or(false));
        assert!(!info.as_object().unwrap().contains_key("foo without colon"));
    }

    #[test]
    fn permission_missing_returns_none() {
        assert!(parse_permission_info_output("no block here", "a.b").is_none());
    }
}
