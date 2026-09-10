//! decx-taint end-to-end tests over synthetic `MethodBody`s (no real Dex
//! needed): direct paths, inter-procedural materialization, static-field
//! routes, propagator chains, sanitizers, return-value taint, and the
//! embedded rule set sanity.

use decx_taint::body::{MethodBody, Op};
use decx_taint::dispatch::DispatchIndex;
use decx_taint::intra::{analyze, Origin};
use decx_taint::model::{Entry, PropMode, Propagator, Rules};
use std::collections::{BTreeMap, BTreeSet};

const SRC: &str = "Landroid/telephony/TelephonyManager;->getDeviceId()Ljava/lang/String;";
const SINK: &str = "Ljava/net/HttpURLConnection;->setRequestProperty(Ljava/lang/String;Ljava/lang/String;)V";
const APPEND: &str = "Ljava/lang/StringBuilder;->append(Ljava/lang/String;)Ljava/lang/StringBuilder;";
const TO_STRING: &str = "Ljava/lang/StringBuilder;->toString()Ljava/lang/String;";
const PARSE_INT: &str = "Ljava/lang/Integer;->parseInt(Ljava/lang/String;)I";

fn rules() -> Rules {
    Rules {
        sources: vec![Entry {
            sig: "Landroid/telephony/TelephonyManager;->getDeviceId(".into(),
            label: "imei".into(),
            kind: "method",
            param: None,
        }],
        sinks: vec![Entry {
            sig: "Ljava/net/HttpURLConnection;->setRequestProperty(".into(),
            label: "http_header".into(),
            kind: "method",
            param: None,
        }],
        propagators: vec![
            Propagator { sig: "Ljava/lang/StringBuilder;->append(".into(), mode: PropMode::ArgToRecv },
            Propagator { sig: "Ljava/lang/StringBuilder;->toString(".into(), mode: PropMode::RecvToRet },
        ],
        sanitizers: vec![PARSE_INT.into()],
        excludes: vec!["Landroid/support/".into()],
    }
}

fn body(sig: &str, registers_size: u16, ins_size: u16, ops: Vec<Op>) -> MethodBody {
    MethodBody {
        sig: sig.into(),
        registers_size,
        ins_size,
        ops,
    }
}

fn src_invoke(pc: usize) -> Op {
    Op::Invoke { pc, callee: SRC.into(), args: vec![], kind: "static" }
}

fn sink_invoke(pc: usize, args: Vec<u8>) -> Op {
    Op::Invoke { pc, callee: SINK.into(), args, kind: "virtual" }
}

#[test]
fn direct_source_to_sink() {
    let r = rules();
    let b = body(
        "Ltest/Main;->run()V",
        2,
        0,
        vec![
            src_invoke(0),
            Op::MoveResult { pc: 1, dst: 0 },
            sink_invoke(2, vec![0, 1]),
        ],
    );
    let out = analyze(&b, &r, &BTreeMap::new(), &BTreeMap::new(), &DispatchIndex::empty());
    assert_eq!(out.findings.len(), 1);
    let f = out.findings.iter().next().unwrap();
    assert_eq!(f.src_pc, 0);
    assert_eq!(f.sink_pc, 2);
    assert_eq!(f.src_method, "Ltest/Main;->run()V");
    assert_eq!(f.sink_method, "Ltest/Main;->run()V");
    assert!(f.hops.is_empty());
}

#[test]
fn param_sink_summarized_then_materialized() {
    let r = rules();
    // callee: static exfil(String s) { urlconn.setRequestProperty("X", s) }
    let callee = body(
        "Ltest/Helper;->exfil(Ljava/lang/String;)V",
        2,
        1,
        vec![sink_invoke(4, vec![0, 1])], // v1 = param s
    );
    let c_out = analyze(&callee, &r, &BTreeMap::new(), &BTreeMap::new(), &DispatchIndex::empty());
    assert!(c_out.findings.is_empty(), "param taint alone must not be a finding");
    assert_eq!(c_out.summary.param_sinks.len(), 1);
    assert_eq!(c_out.summary.param_sinks[&0].len(), 1);

    // caller: String id = tm.getDeviceId(); Helper.exfil(id);
    let caller = body(
        "Ltest/Main;->run()V",
        1,
        0,
        vec![
            src_invoke(0),
            Op::MoveResult { pc: 1, dst: 0 },
            Op::Invoke {
                pc: 2,
                callee: "Ltest/Helper;->exfil(Ljava/lang/String;)V".into(),
                args: vec![0],
                kind: "static",
            },
        ],
    );
    let mut summaries = BTreeMap::new();
    summaries.insert(callee.sig.clone(), c_out.summary);
    let out = analyze(&caller, &r, &summaries, &BTreeMap::new(), &DispatchIndex::empty());
    assert_eq!(out.findings.len(), 1);
    let f = out.findings.iter().next().unwrap();
    assert_eq!(f.sink_method, "Ltest/Helper;->exfil(Ljava/lang/String;)V");
    assert_eq!(f.sink_pc, 4);
    assert_eq!(f.hops, vec![("Ltest/Main;->run()V".to_string(), 2)]);
}

#[test]
fn static_field_route() {
    let r = rules();
    // producer: Cfg.token = tm.getDeviceId()
    let producer = body(
        "Ltest/Main;->run()V",
        1,
        0,
        vec![
            src_invoke(0),
            Op::MoveResult { pc: 1, dst: 0 },
            Op::FieldPut { pc: 2, src: 0, field: "Ltest/Cfg;->token Ljava/lang/String;".into(), obj: None },
        ],
    );
    let p_out = analyze(&producer, &r, &BTreeMap::new(), &BTreeMap::new(), &DispatchIndex::empty());
    assert!(p_out.findings.is_empty());
    assert_eq!(p_out.static_publishes.len(), 1);

    // consumer: sink("x", Cfg.token)
    let consumer = body(
        "Ltest/Other;->go()V",
        2,
        0,
        vec![
            Op::FieldGet { pc: 0, dst: 1, field: "Ltest/Cfg;->token Ljava/lang/String;".into(), obj: None },
            sink_invoke(1, vec![0, 1]),
        ],
    );
    let mut statics: BTreeMap<String, BTreeSet<decx_taint::Token>> = BTreeMap::new();
    statics.extend(p_out.static_publishes.clone());
    let out = analyze(&consumer, &r, &BTreeMap::new(), &statics, &DispatchIndex::empty());
    assert_eq!(out.findings.len(), 1);
    let f = out.findings.iter().next().unwrap();
    assert_eq!(f.src_method, "Ltest/Main;->run()V");
    assert_eq!(f.sink_method, "Ltest/Other;->go()V");
}

#[test]
fn propagator_chain_append_tostring() {
    let r = rules();
    // v0 = sb; v1 = tainted; sb.append(v1); v2 = sb.toString(); sink("k", v2)
    let b = body(
        "Ltest/Main;->run()V",
        3,
        0,
        vec![
            Op::Const { pc: 0, dst: 0 },
            src_invoke(1),
            Op::MoveResult { pc: 2, dst: 1 },
            Op::Invoke { pc: 3, callee: APPEND.into(), args: vec![0, 1], kind: "virtual" },
            Op::Invoke { pc: 4, callee: TO_STRING.into(), args: vec![0], kind: "virtual" },
            Op::MoveResult { pc: 5, dst: 2 },
            sink_invoke(6, vec![2, 0]),
        ],
    );
    let out = analyze(&b, &r, &BTreeMap::new(), &BTreeMap::new(), &DispatchIndex::empty());
    assert_eq!(out.findings.len(), 1, "append/toString chain must carry taint to the sink");
}

#[test]
fn sanitizer_kills_taint() {
    let r = rules();
    // v0 = getDeviceId(); v2 = v0; v1 = Integer.parseInt(v2); v0 = "k"; sink(v0, v1)
    let b = body(
        "Ltest/Main;->run()V",
        3,
        0,
        vec![
            src_invoke(0),
            Op::MoveResult { pc: 1, dst: 0 },
            Op::Invoke { pc: 2, callee: PARSE_INT.into(), args: vec![0], kind: "static" },
            Op::MoveResult { pc: 3, dst: 1 },
            Op::Const { pc: 4, dst: 0 },
            sink_invoke(5, vec![0, 1]),
        ],
    );
    let out = analyze(&b, &r, &BTreeMap::new(), &BTreeMap::new(), &DispatchIndex::empty());
    assert!(out.findings.is_empty(), "sanitized value must not reach the sink");
}

#[test]
fn tainted_return_adopted_by_caller() {
    let r = rules();
    // callee: String id() { return tm.getDeviceId(); }
    let callee = body(
        "Ltest/Util;->id()Ljava/lang/String;",
        1,
        0,
        vec![src_invoke(0), Op::MoveResult { pc: 1, dst: 0 }, Op::Return { pc: 2, src: Some(0) }],
    );
    let c_out = analyze(&callee, &r, &BTreeMap::new(), &BTreeMap::new(), &DispatchIndex::empty());
    assert_eq!(c_out.summary.ret_tokens.len(), 1);
    assert!(matches!(
        c_out.summary.ret_tokens.iter().next().unwrap().origin,
        Origin::Src { .. }
    ));

    // caller: sink("k", Util.id())
    let caller = body(
        "Ltest/Main;->run()V",
        2,
        0,
        vec![
            Op::Invoke {
                pc: 0,
                callee: "Ltest/Util;->id()Ljava/lang/String;".into(),
                args: vec![],
                kind: "static",
            },
            Op::MoveResult { pc: 1, dst: 0 },
            sink_invoke(2, vec![0, 1]),
        ],
    );
    let mut summaries = BTreeMap::new();
    summaries.insert(callee.sig.clone(), c_out.summary);
    let out = analyze(&caller, &r, &summaries, &BTreeMap::new(), &DispatchIndex::empty());
    assert_eq!(out.findings.len(), 1);
    let f = out.findings.iter().next().unwrap();
    assert_eq!(f.src_method, "Ltest/Util;->id()Ljava/lang/String;");
    assert_eq!(f.sink_method, "Ltest/Main;->run()V");
    assert_eq!(f.hops.len(), 1);
}

#[test]
fn const_overwrites_taint() {
    let r = rules();
    let b = body(
        "Ltest/Main;->run()V",
        2,
        0,
        vec![
            src_invoke(0),
            Op::MoveResult { pc: 1, dst: 0 },
            Op::Const { pc: 2, dst: 0 },
            sink_invoke(3, vec![0, 1]),
        ],
    );
    let out = analyze(&b, &r, &BTreeMap::new(), &BTreeMap::new(), &DispatchIndex::empty());
    assert!(out.findings.is_empty(), "const-new overwrite must kill register taint");
}

#[test]
fn excluded_owner_skipped_by_extract_predicate() {
    let r = rules();
    assert!(r.owner_excluded("Landroid/support/v7/AppCompat;->onCreate()V"));
    assert!(!r.owner_excluded("Ltest/Main;->run()V"));
}

#[test]
fn embedded_ruleset_loads() {
    let r = Rules::default_rules();
    assert!(r.sources.len() >= 30);
    assert!(r.sinks.len() >= 40);
    assert!(r.propagators.len() >= 40);
    assert!(!r.sanitizers.is_empty());
    assert!(r.match_source("Landroid/telephony/TelephonyManager;->getDeviceId()Ljava/lang/String;").is_some());
    assert!(r.match_sink("Ljava/net/HttpURLConnection;->setRequestProperty(Ljava/lang/String;Ljava/lang/String;)V").is_some());
    assert!(r.match_field_source("Landroid/os/Build;->SERIAL Ljava/lang/String;").is_some());
}

// ---------- Phase 1: param-anchored callback sources ----------

#[test]
fn param_anchored_callback_source_fires() {
    // rule: LocationListener.onLocationChanged(param 0) is a source
    let r = Rules {
        sources: vec![Entry {
            sig: "Landroid/location/LocationListener;->onLocationChanged(".into(),
            label: "location".into(),
            kind: "method",
            param: Some(0),
        }],
        ..rules()
    };
    // the app's override (different owner, same key): sink(loc)
    // virtual void onLocationChanged(Location loc) — v1 = loc param
    let cb = body(
        "Lcom/app/L;->onLocationChanged(Landroid/location/Location;)V",
        2,
        1,
        vec![sink_invoke(3, vec![0, 1])],
    );
    let out = analyze(&cb, &r, &BTreeMap::new(), &BTreeMap::new(), &DispatchIndex::empty());
    assert_eq!(out.findings.len(), 1, "callback override param must be a seed");
    let f = out.findings.iter().next().unwrap();
    assert_eq!(f.src_pc, 0);
    assert_eq!(f.src_method, "Lcom/app/L;->onLocationChanged(Landroid/location/Location;)V");
    assert_eq!(f.src_api, "location");
}

#[test]
fn param_anchor_respects_declared_position() {
    // onReceive(Context ctx, Intent intent): only param 1 is a source
    let r = Rules {
        sources: vec![Entry {
            sig: "Landroid/content/BroadcastReceiver;->onReceive(".into(),
            label: "intent_in".into(),
            kind: "method",
            param: Some(1),
        }],
        ..rules()
    };
    // sink(recv, intent): v0 = local recv, v1 = ctx (slot 0), v2 = intent (slot 1)
    let cb = body(
        "Lcom/app/R;->onReceive(Landroid/content/Context;Landroid/content/Intent;)V",
        3,
        2,
        vec![sink_invoke(0, vec![0, 2])],
    );
    let out = analyze(&cb, &r, &BTreeMap::new(), &BTreeMap::new(), &DispatchIndex::empty());
    assert_eq!(out.findings.len(), 1, "intent (param 1) is tainted");
}

#[test]
fn rules_load_file_roundtrip_and_errors() {
    let dir = std::env::temp_dir().join("decx-taint-rules-test");
    std::fs::create_dir_all(&dir).unwrap();
    let ok = dir.join("ok.json");
    std::fs::write(&ok, r#"{"sources": [{"sig": "La;->src(", "label": "x", "param": 0}], "sinks": [{"sig": "Lb;->sink(", "label": "y"}]}"#).unwrap();
    let r = Rules::load_file(ok.to_str().unwrap()).unwrap();
    assert_eq!(r.sources.len(), 1);
    assert_eq!(r.sources[0].param, Some(0));
    assert_eq!(r.sinks.len(), 1);

    let bad_json = dir.join("bad.json");
    std::fs::write(&bad_json, "{not json").unwrap();
    let e = Rules::load_file(bad_json.to_str().unwrap()).unwrap_err();
    assert!(e.contains("invalid JSON"), "got: {e}");

    let empty = dir.join("empty.json");
    std::fs::write(&empty, "{}").unwrap();
    let e = Rules::load_file(empty.to_str().unwrap()).unwrap_err();
    assert!(e.contains("no sources and no sinks"), "got: {e}");

    let missing = dir.join("missing.json");
    let e = Rules::load_file(missing.to_str().unwrap()).unwrap_err();
    assert!(e.contains("cannot read"), "got: {e}");
}

// ---------- Phase 2: virtual dispatch fan-out ----------

#[test]
fn virtual_call_fans_out_to_override_sink() {
    let r = rules();
    // override holds the sink: Base.call() overridden by Sub.call() with a sink
    let sub = body(
        "Ltest/Sub;->call(Ljava/lang/String;)V",
        2,
        1,
        vec![sink_invoke(4, vec![0, 1])], // v1 = param
    );
    let sub_out = analyze(&sub, &r, &BTreeMap::new(), &BTreeMap::new(), &DispatchIndex::empty());
    assert!(sub_out.findings.is_empty());
    assert_eq!(sub_out.summary.param_sinks.len(), 1);

    // caller invokes the BASE signature virtually
    let caller = body(
        "Ltest/Main;->run()V",
        1,
        0,
        vec![
            src_invoke(0),
            Op::MoveResult { pc: 1, dst: 0 },
            Op::Invoke {
                pc: 2,
                callee: "Ltest/Base;->call(Ljava/lang/String;)V".into(),
                args: vec![0],
                kind: "virtual",
            },
        ],
    );
    let mut summaries = BTreeMap::new();
    summaries.insert(sub.sig.clone(), sub_out.summary);
    let mut dispatch = DispatchIndex::empty();
    dispatch.insert("Ltest/Base;->call(Ljava/lang/String;)V");
    dispatch.insert("Ltest/Sub;->call(Ljava/lang/String;)V");
    let out = analyze(&caller, &r, &summaries, &BTreeMap::new(), &dispatch);
    assert_eq!(out.findings.len(), 1, "override summary must apply at base-class call site");
    let f = out.findings.iter().next().unwrap();
    assert_eq!(f.sink_method, "Ltest/Sub;->call(Ljava/lang/String;)V");
}

#[test]
fn static_call_does_not_fan_out() {
    let r = rules();
    let sub = body(
        "Ltest/Sub;->call(Ljava/lang/String;)V",
        2,
        1,
        vec![sink_invoke(4, vec![0, 1])],
    );
    let sub_out = analyze(&sub, &r, &BTreeMap::new(), &BTreeMap::new(), &DispatchIndex::empty());
    let caller = body(
        "Ltest/Main;->run()V",
        1,
        0,
        vec![
            src_invoke(0),
            Op::MoveResult { pc: 1, dst: 0 },
            Op::Invoke {
                pc: 2,
                callee: "Ltest/Base;->call(Ljava/lang/String;)V".into(),
                args: vec![0],
                kind: "static",
            },
        ],
    );
    let mut summaries = BTreeMap::new();
    summaries.insert(sub.sig.clone(), sub_out.summary);
    let mut dispatch = DispatchIndex::empty();
    dispatch.insert("Ltest/Base;->call(Ljava/lang/String;)V");
    dispatch.insert("Ltest/Sub;->call(Ljava/lang/String;)V");
    let out = analyze(&caller, &r, &summaries, &BTreeMap::new(), &dispatch);
    assert!(out.findings.is_empty(), "static invoke must not dispatch to overrides");
}

#[test]
fn sink_rule_matches_override_at_base_call_site() {
    // the sink rule targets the OVERRIDE; code calls the base signature
    let r = Rules {
        sinks: vec![Entry {
            sig: "Ltest/Sub;->leak(".into(),
            label: "http_header".into(),
            kind: "method",
            param: None,
        }],
        ..rules()
    };
    let caller = body(
        "Ltest/Main;->run()V",
        1,
        0,
        vec![
            src_invoke(0),
            Op::MoveResult { pc: 1, dst: 0 },
            Op::Invoke {
                pc: 2,
                callee: "Ltest/Base;->leak(Ljava/lang/String;)V".into(),
                args: vec![0],
                kind: "virtual",
            },
        ],
    );
    let mut dispatch = DispatchIndex::empty();
    dispatch.insert("Ltest/Base;->leak(Ljava/lang/String;)V");
    dispatch.insert("Ltest/Sub;->leak(Ljava/lang/String;)V");
    let out = analyze(&caller, &r, &BTreeMap::new(), &BTreeMap::new(), &dispatch);
    assert_eq!(out.findings.len(), 1, "sink rule on the override must fire at the base call site");
    let f = out.findings.iter().next().unwrap();
    assert!(f.sink_api.starts_with("Ltest/Sub;->leak("));
}
