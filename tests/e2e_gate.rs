#[path = "../examples/support/mdc_e2e.rs"]
mod harness;
#[test]
fn complete_declared_gate_executes_every_mandatory_case() {
    let report = harness::execute();
    assert!(report.accepted(), "{}", report.json());
    assert_eq!(report.cases.len(), harness::IDS.len());
    for id in harness::IDS {
        let case = report.cases.iter().find(|case| case.id == id).unwrap();
        assert!(case.executed && case.passed && !case.evidence.is_empty());
    }
}
#[test]
fn acceptance_refuses_missing_duplicate_unexecuted_or_failed_cases() {
    let mut report = harness::execute();
    assert!(report.accepted());
    let last = report.cases.pop().unwrap();
    assert!(!report.accepted());
    report.cases.push(last);
    report.cases[0].executed = false;
    assert!(!report.accepted());
    report.cases[0].executed = true;
    report.cases[0].passed = false;
    assert!(!report.accepted());
    report.cases[0].passed = true;
    let evidence = std::mem::take(&mut report.cases[0].evidence);
    assert!(!report.accepted());
    report.cases[0].evidence = evidence;
    let semantic_case = report
        .cases
        .iter()
        .position(|case| case.id == "E2E-A05")
        .unwrap();
    let actual = report.cases[semantic_case].actual_semantic;
    report.cases[semantic_case].actual_semantic =
        Some(mdc_core::contract::evaluation::ConformanceResult::Fail);
    assert!(!report.accepted());
    report.cases[semantic_case].expected_semantic =
        Some(mdc_core::contract::evaluation::ConformanceResult::Fail);
    assert!(!report.accepted());
    report.cases[semantic_case].expected_semantic = actual;
    report.cases[semantic_case].actual_semantic = actual;
    report.cases[0].id = report.cases[1].id.clone();
    assert!(!report.accepted());
}
