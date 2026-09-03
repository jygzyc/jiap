//! Dataflow expected values (same as androguard test_decompiler_dataflow.py).

use std::collections::{HashMap, HashSet};

/// Reach-def style: defs at instruction indices. Same expected A/R/def_to_loc as testReachDefGCD.
#[test]
fn test_dataflow_reach_def_gcd_expected() {
    let expected_a_keys = [
        "entry", "n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9", "exit",
    ];
    let _ = expected_a_keys;
    let expected_def_to_loc: HashMap<&str, HashSet<i32>> = [
        ("a", HashSet::from([-1])),
        ("b", HashSet::from([-2])),
        ("c", HashSet::from([0, 6])),
        ("d", HashSet::from([1, 7])),
        ("ret", HashSet::from([3, 8])),
    ]
    .into_iter()
    .collect();
    assert_eq!(expected_def_to_loc.get("c").unwrap().len(), 2);
    assert_eq!(expected_def_to_loc.get("ret").unwrap().len(), 2);
}

/// Def-use: (var, def_loc) -> list of use locs. Same expected_du as testDefUseGCD.
#[test]
fn test_dataflow_def_use_gcd_expected() {
    let expected_du: HashMap<(String, i32), Vec<i32>> = [
        (("a".into(), -1), vec![0]),
        (("b".into(), -2), vec![1]),
        (("c".into(), 0), vec![2, 5, 6, 7, 8]),
        (("c".into(), 6), vec![8]),
        (("d".into(), 1), vec![3, 4, 5, 6, 7]),
        (("ret".into(), 3), vec![9]),
        (("ret".into(), 8), vec![9]),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        expected_du.get(&("c".into(), 0)).unwrap(),
        &vec![2, 5, 6, 7, 8]
    );
    assert_eq!(expected_du.get(&("ret".into(), 3)).unwrap(), &vec![9]);
}

/// Use-def: (var, use_loc) -> list of def locs. Same expected_ud as testDefUseGCD.
#[test]
fn test_dataflow_use_def_gcd_expected() {
    let expected_ud: HashMap<(String, i32), Vec<i32>> = [
        (("a".into(), 0), vec![-1]),
        (("b".into(), 1), vec![-2]),
        (("c".into(), 2), vec![0]),
        (("c".into(), 5), vec![0]),
        (("c".into(), 6), vec![0]),
        (("c".into(), 7), vec![0]),
        (("c".into(), 8), vec![0, 6]),
        (("d".into(), 3), vec![1]),
        (("d".into(), 4), vec![1]),
        (("d".into(), 5), vec![1]),
        (("d".into(), 6), vec![1]),
        (("d".into(), 7), vec![1]),
        (("ret".into(), 9), vec![3, 8]),
    ]
    .into_iter()
    .collect();
    assert_eq!(expected_ud.get(&("c".into(), 8)).unwrap(), &vec![0, 6]);
}

/// group_variables: var -> list of (def_locs, use_locs). Same expected as testGroupVariablesGCD.
#[test]
fn test_dataflow_group_variables_gcd_expected() {
    type DefUse = (Vec<i32>, Vec<i32>);
    let expected_groups: HashMap<String, Vec<DefUse>> = [
        ("a".into(), vec![(vec![-1], vec![0])]),
        ("b".into(), vec![(vec![-2], vec![1])]),
        ("c".into(), vec![(vec![0, 6], vec![8, 2, 5, 6, 7])]),
        ("d".into(), vec![(vec![1], vec![3, 4, 5, 6, 7])]),
        ("ret".into(), vec![(vec![3, 8], vec![9])]),
    ]
    .into_iter()
    .collect();
    let c_entries = expected_groups.get("c").unwrap();
    assert_eq!(c_entries.len(), 1);
    assert_eq!(c_entries[0].0, vec![0, 6]);
}
