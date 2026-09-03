//! PendingIntent vulnerability detection (PITracker-aligned).
//!
//! Ports the core checks from [PITracker](https://github.com/Sp1keeeee/PItracker)
//! (WiSec'22 — Zhang et al.): Intent-flow analysis on `PendingIntent.get*` sites.
//!
//! Checks:
//! - Base Intent: are action / package / data / clipData / component set?
//! - Implicit empty base Intent (attacker can fill fields)
//! - Mutable flags (`FLAG_MUTABLE` without `FLAG_IMMUTABLE`)
//! - Destination: PI handed to AICC sinks (Notification, AlarmManager, RemoteViews, …)
//!
//! Paper: https://diaowenrui.github.io/paper/wisec22-zhang.pdf

use crate::decompile::cfg::MethodCfg;
use crate::decompile::value_flow::{ValueFlowAnalysisOwned, ValueFlowResult};
use std::collections::{HashMap, HashSet};

const PENDING_INTENT_GET_METHODS: &[&str] = &[
    "PendingIntent.getActivity",
    "PendingIntent.getBroadcast",
    "PendingIntent.getService",
    "PendingIntent.getForegroundService",
];

/// Fields whose absence leaves the base Intent attacker-fillable (PITracker).
const INTENT_FILL_SETTERS: &[&str] = &[
    "Intent.setAction",
    "setAction",
    "Intent.setPackage",
    "setPackage",
    "Intent.setData",
    "setData",
    "Intent.setDataAndType",
    "setDataAndType",
    "Intent.setClipData",
    "setClipData",
    "Intent.setComponent",
    "setComponent",
    "Intent.setClass",
    "setClass",
    "Intent.setClassName",
    "setClassName",
    "Intent.setSelector",
    "setSelector",
];

/// Subset: explicit-target setters (component/class/package).
const INTENT_EXPLICIT_SETTERS: &[&str] =
    &["setComponent", "setClass", "setClassName", "setPackage"];

/// AICC / “where PI goes” sinks distilled from PITracker `aicc_new.txt`.
const DANGEROUS_SINKS: &[&str] = &[
    // Notification
    "setContentIntent",
    "setDeleteIntent",
    "setFullScreenIntent",
    "addAction",
    "setLatestEventInfo",
    "BubbleMetadata",
    "setReadPendingIntent",
    "setReplyAction",
    "setDisplayIntent",
    "setCancelButtonIntent",
    // Alarm / Location / Connectivity
    "AlarmManager.set",
    "setAlarmClock",
    "setAndAllowWhileIdle",
    "setExactAndAllowWhileIdle",
    "setExact",
    "setInexactRepeating",
    "setRepeating",
    "setWindow",
    "addProximityAlert",
    "requestLocationUpdates",
    "requestSingleUpdate",
    "registerNetworkCallback",
    "requestNetwork",
    "startScan",
    // RemoteViews / media / NFC / VPN
    "setOnClickPendingIntent",
    "setPendingIntentTemplate",
    "setMediaButtonReceiver",
    "setSessionActivity",
    "registerMediaButtonEventReceiver",
    "enableForegroundDispatch",
    "setConfigureIntent",
    "setInfoIntent",
    // SMS / telephony / slices / shortcuts
    "sendTextMessage",
    "sendDataMessage",
    "sendMultimediaMessage",
    "downloadMultimediaMessage",
    "injectSmsPdu",
    "createAppSpecificSmsToken",
    "Slice$Builder.addAction",
    "requestPinShortcut",
    "requestUsageTimeReport",
    // IntentSender / send / package installer
    "PendingIntent.send",
    "IntentSender.sendIntent",
    "startIntentSender",
    "startIntentSenderForResult",
    "PackageInstaller",
    "commit",
    "uninstall",
    // GMS location
    "requestActivityUpdates",
    "addGeofences",
    "FusedLocationProvider",
];

const FLAG_MUTABLE: i64 = 1 << 25; // 0x0200_0000
const FLAG_IMMUTABLE: i64 = 1 << 26; // 0x0400_0000

/// One PendingIntent creation site with PITracker-style risk info.
#[derive(Debug, Clone)]
pub struct PendingIntentFinding {
    pub class_name: String,
    pub method_name: String,
    pub invoke_offset: u32,
    pub creation_kind: String,
    /// True when no fill setters found on the base Intent (PITracker empty/implicit).
    pub base_intent_empty: bool,
    pub base_intent_explicit: bool,
    pub action_set: bool,
    pub data_set: bool,
    pub clipdata_set: bool,
    pub package_or_component_set: bool,
    pub mutable_flag: bool,
    pub immutable_flag: bool,
    pub dangerous_destination: bool,
    pub destination_kind: String,
    /// PITracker forward: PI stuffed into another Intent via putExtra (wrap).
    pub wrapped_in_intent: bool,
}

impl PendingIntentFinding {
    /// High-threat per PITracker: empty/implicit base + obtainable destination (or mutable/wrap).
    pub fn is_high_threat(&self) -> bool {
        let weak_base = self.base_intent_empty || !self.base_intent_explicit;
        let obtainable = self.dangerous_destination
            || self.wrapped_in_intent
            || (self.mutable_flag && !self.immutable_flag);
        weak_base && obtainable
    }
}

fn prev_offset_in_block(cfg: &MethodCfg) -> HashMap<u32, u32> {
    let mut prev = HashMap::new();
    for block in &cfg.blocks {
        let offsets = &block.instruction_offsets;
        for i in 1..offsets.len() {
            prev.insert(offsets[i], offsets[i - 1]);
        }
    }
    prev
}

fn is_pending_intent_creation(method_ref: &str) -> bool {
    PENDING_INTENT_GET_METHODS
        .iter()
        .any(|m| method_ref.contains(m))
}

fn creation_kind(method_ref: &str) -> String {
    if method_ref.contains("getActivity") {
        "getActivity".into()
    } else if method_ref.contains("getBroadcast") {
        "getBroadcast".into()
    } else if method_ref.contains("getForegroundService") {
        "getForegroundService".into()
    } else if method_ref.contains("getService") {
        "getService".into()
    } else {
        "get*".into()
    }
}

fn method_matches_any(method_ref: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| method_ref.contains(p))
}

fn is_dangerous_sink(method_ref: &str) -> bool {
    method_matches_any(method_ref, DANGEROUS_SINKS)
}

fn transitive_defs(
    owned: &ValueFlowAnalysisOwned,
    use_offset: u32,
    use_reg: u32,
) -> HashSet<(u32, u32)> {
    let analysis = owned.analysis();
    let mut defs = HashSet::new();
    let mut worklist = vec![(use_offset, use_reg)];
    let mut visited = HashSet::new();
    while let Some((off, reg)) = worklist.pop() {
        if !visited.insert((off, reg)) {
            continue;
        }
        for (d_off, d_reg) in analysis.use_def(off, reg) {
            defs.insert((d_off, d_reg));
            if let Some((reads, _)) = owned.rw_map.get(&d_off) {
                if reads.len() == 1 {
                    worklist.push((d_off, reads[0]));
                }
            }
        }
    }
    defs
}

#[derive(Default)]
struct BaseIntentShape {
    any_fill: bool,
    explicit: bool,
    action: bool,
    data: bool,
    clipdata: bool,
    package_or_component: bool,
}

fn analyze_base_intent(
    owned: &ValueFlowAnalysisOwned,
    invoke_offset: u32,
    intent_reg: u32,
) -> BaseIntentShape {
    let prev = prev_offset_in_block(&owned.cfg);
    let defs = transitive_defs(owned, invoke_offset, intent_reg);
    let mut shape = BaseIntentShape::default();

    let mut consider = |method_ref: &str| {
        if !method_matches_any(method_ref, INTENT_FILL_SETTERS) {
            return;
        }
        shape.any_fill = true;
        if method_ref.contains("setAction") {
            shape.action = true;
        }
        if method_ref.contains("setData") {
            shape.data = true;
        }
        if method_ref.contains("setClipData") {
            shape.clipdata = true;
        }
        if method_matches_any(method_ref, INTENT_EXPLICIT_SETTERS) {
            shape.explicit = true;
            shape.package_or_component = true;
        }
    };

    for (d_off, d_reg) in &defs {
        if let Some(method_ref) = owned.invoke_method_map.get(d_off) {
            if let Some((reads, _)) = owned.rw_map.get(d_off) {
                if reads.first() == Some(d_reg) || reads.contains(d_reg) {
                    consider(method_ref);
                }
            }
        }
        if let Some((reads, writes)) = owned.rw_map.get(d_off) {
            if reads.is_empty() && writes.len() == 1 && writes[0] == *d_reg {
                if let Some(&prev_off) = prev.get(d_off) {
                    if let Some(method_ref) = owned.invoke_method_map.get(&prev_off) {
                        consider(method_ref);
                    }
                }
            }
        }
    }
    shape
}

fn classify_destination(owned: &ValueFlowAnalysisOwned, flow: &ValueFlowResult) -> (bool, String) {
    for (off, _reg) in &flow.reads {
        if let Some(method_ref) = owned.invoke_method_map.get(off) {
            if is_dangerous_sink(method_ref) {
                let kind = if method_ref.contains("Notification")
                    || method_ref.contains("setContentIntent")
                    || method_ref.contains("setDeleteIntent")
                    || method_ref.contains("setFullScreenIntent")
                    || method_ref.contains("addAction")
                {
                    "Notification/AICC"
                } else if method_ref.contains("AlarmManager")
                    || method_ref.contains("setExact")
                    || method_ref.contains("setRepeating")
                    || method_ref.contains("setAlarmClock")
                {
                    "AlarmManager"
                } else if method_ref.contains("RemoteViews")
                    || method_ref.contains("setOnClickPendingIntent")
                    || method_ref.contains("setPendingIntentTemplate")
                {
                    "RemoteViews"
                } else if method_ref.contains("Location")
                    || method_ref.contains("Geofenc")
                    || method_ref.contains("Proximity")
                {
                    "Location"
                } else if method_ref.contains("send") || method_ref.contains("IntentSender") {
                    "PendingIntent.send/IntentSender"
                } else {
                    "AICC"
                };
                return (true, kind.to_string());
            }
        }
    }
    (false, "other".to_string())
}

fn pi_wrapped_in_intent(owned: &ValueFlowAnalysisOwned, flow: &ValueFlowResult) -> bool {
    const WRAP: &[&str] = &[
        "putExtra",
        "putParcelableExtra",
        "Intent.putExtra",
        "Bundle.putParcelable",
        "putParcelable",
    ];
    for (off, _reg) in &flow.reads {
        if let Some(method_ref) = owned.invoke_method_map.get(off) {
            if method_matches_any(method_ref, WRAP) {
                return true;
            }
        }
    }
    false
}

fn parse_hex_or_dec(tok: &str) -> Option<i64> {
    let t = tok.trim().trim_end_matches(',');
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()
    } else {
        t.parse::<i64>().ok()
    }
}

fn flags_from_insn_text(text: &str) -> (bool, bool) {
    let u = text.to_uppercase();
    let mut mutable =
        u.contains("FLAG_MUTABLE") || u.contains("0x2000000") || u.contains("0x02000000");
    let mut immutable =
        u.contains("FLAG_IMMUTABLE") || u.contains("0x4000000") || u.contains("0x04000000");
    for tok in text.split(|c: char| c.is_whitespace() || c == '{' || c == '}') {
        if let Some(v) = parse_hex_or_dec(tok) {
            if v & FLAG_MUTABLE != 0 {
                mutable = true;
            }
            if v & FLAG_IMMUTABLE != 0 {
                immutable = true;
            }
        }
    }
    (mutable, immutable)
}

fn analyze_flags(
    owned: &ValueFlowAnalysisOwned,
    invoke_offset: u32,
    flag_reg: Option<u32>,
) -> (bool, bool) {
    let mut mutable = false;
    let mut immutable = false;
    if let Some(text) = owned.insn_at.get(&invoke_offset) {
        let (m, i) = flags_from_insn_text(text);
        mutable |= m;
        immutable |= i;
    }
    if let Some(reg) = flag_reg {
        let defs = transitive_defs(owned, invoke_offset, reg);
        for (d_off, _) in defs {
            if let Some(text) = owned.insn_at.get(&d_off) {
                let (m, i) = flags_from_insn_text(text);
                mutable |= m;
                immutable |= i;
            }
        }
    }
    // Whole-method fallback for FLAG_* stringified in nearby consts.
    for text in owned.insn_at.values() {
        let (m, i) = flags_from_insn_text(text);
        if m || i {
            mutable |= m;
            immutable |= i;
            break;
        }
    }
    (mutable, immutable)
}

/// Scan one method for PendingIntent creation sites and assess risk (PITracker-style).
pub fn scan_pending_intents(
    owned: &ValueFlowAnalysisOwned,
    class_name: &str,
    method_name: &str,
) -> Vec<PendingIntentFinding> {
    let prev = prev_offset_in_block(&owned.cfg);
    let mut findings = Vec::new();

    for ((move_result_offset, pi_reg), method_ref) in &owned.api_return_sources {
        if !is_pending_intent_creation(method_ref) {
            continue;
        }
        let Some(invoke_offset) = prev.get(move_result_offset) else {
            continue;
        };
        let empty = (vec![], vec![]);
        let (arg_reads, _) = owned.rw_map.get(invoke_offset).unwrap_or(&empty);
        // static get*(Context, int, Intent, int) → intent at index 2, flags at 3
        if arg_reads.len() < 3 {
            continue;
        }
        let intent_reg = arg_reads[2];
        let flag_reg = arg_reads.get(3).copied();

        let shape = analyze_base_intent(owned, *invoke_offset, intent_reg);
        let (mutable_flag, immutable_flag) = analyze_flags(owned, *invoke_offset, flag_reg);
        let flow: ValueFlowResult = owned
            .analysis()
            .value_flow_from_seed(*move_result_offset, *pi_reg);
        let (dangerous_destination, mut destination_kind) = classify_destination(owned, &flow);
        let wrapped_in_intent = pi_wrapped_in_intent(owned, &flow);
        if wrapped_in_intent && destination_kind == "other" {
            destination_kind = "Intent.putExtra wrap".into();
        }

        findings.push(PendingIntentFinding {
            class_name: class_name.to_string(),
            method_name: method_name.to_string(),
            invoke_offset: *invoke_offset,
            creation_kind: creation_kind(method_ref),
            base_intent_empty: !shape.any_fill,
            base_intent_explicit: shape.explicit,
            action_set: shape.action,
            data_set: shape.data,
            clipdata_set: shape.clipdata,
            package_or_component_set: shape.package_or_component,
            mutable_flag,
            immutable_flag,
            dangerous_destination,
            destination_kind,
            wrapped_in_intent,
        });
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decompile::cfg::{BlockEnd, CfgBlock};
    use std::collections::HashMap;

    fn make_cfg(instruction_offsets: Vec<u32>) -> MethodCfg {
        let block = CfgBlock {
            start_offset: *instruction_offsets.first().unwrap_or(&0),
            end_offset: instruction_offsets.last().copied().unwrap_or(0) + 2,
            end: BlockEnd::Exit,
            instruction_offsets: instruction_offsets.clone(),
        };
        let mut block_by_start = HashMap::new();
        block_by_start.insert(block.start_offset, 0);
        MethodCfg {
            blocks: vec![block],
            block_by_start,
            loop_headers: HashSet::new(),
            entry: 0,
            folded_const_offsets: HashSet::new(),
        }
    }

    fn synthetic_owned(intent_has_setter: bool, sink_is_dangerous: bool) -> ValueFlowAnalysisOwned {
        let offsets = vec![0u32, 2, 4, 6, 8];
        let cfg = make_cfg(offsets.clone());
        let mut rw_map: HashMap<u32, (Vec<u32>, Vec<u32>)> = HashMap::new();
        if intent_has_setter {
            rw_map.insert(0, (vec![0, 1], vec![]));
            rw_map.insert(2, (vec![], vec![2]));
        }
        rw_map.insert(4, (vec![0, 1, 2, 3], vec![]));
        rw_map.insert(6, (vec![], vec![4]));
        rw_map.insert(8, (vec![4], vec![]));
        let api_return_sources =
            vec![((6, 4), "android.app.PendingIntent.getActivity".to_string())];
        let mut invoke_method_map: HashMap<u32, String> = HashMap::new();
        if intent_has_setter {
            invoke_method_map.insert(0, "android.content.Intent.setAction".to_string());
        }
        invoke_method_map.insert(4, "android.app.PendingIntent.getActivity".to_string());
        invoke_method_map.insert(
            8,
            if sink_is_dangerous {
                "android.app.Notification.Builder.setContentIntent".to_string()
            } else {
                "android.app.AlarmManager.cancel".to_string()
            },
        );
        ValueFlowAnalysisOwned {
            cfg,
            rw_map,
            exceptional_edges: vec![],
            api_return_sources,
            invoke_method_map,
            insn_at: HashMap::new(),
            registers_size: 0,
            ins_size: 0,
        }
    }

    #[test]
    fn scan_finds_vulnerable_site_when_intent_empty_and_dangerous_sink() {
        let owned = synthetic_owned(false, true);
        let findings = scan_pending_intents(&owned, "com.example.Foo", "bar");
        assert_eq!(findings.len(), 1);
        let f = &findings[0];
        assert!(f.base_intent_empty);
        assert!(f.dangerous_destination);
        assert!(f.is_high_threat());
        assert_eq!(f.invoke_offset, 4);
    }

    #[test]
    fn scan_base_intent_not_empty_when_setter_in_chain() {
        let owned = synthetic_owned(true, true);
        let findings = scan_pending_intents(&owned, "com.example.Foo", "bar");
        assert_eq!(findings.len(), 1);
        assert!(!findings[0].base_intent_empty);
    }

    #[test]
    fn scan_destination_other_when_sink_not_dangerous() {
        let owned = synthetic_owned(false, false);
        let findings = scan_pending_intents(&owned, "com.example.Foo", "bar");
        assert_eq!(findings.len(), 1);
        assert!(!findings[0].dangerous_destination);
        assert_eq!(findings[0].destination_kind, "other");
    }

    #[test]
    fn flags_parse_mutable() {
        let (m, i) = flags_from_insn_text("const/high16 v3, 0x2000000");
        assert!(m);
        assert!(!i);
    }
}
