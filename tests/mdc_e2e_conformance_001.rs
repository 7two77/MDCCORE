use mdc_core::canonical::snapshot::snapshot;
use mdc_core::identity::IdentityAdapter;
use mdc_core::model::observation::{Observation, Source};

struct TestSource {
    id: String,
}

impl Source for TestSource {
    fn id(&self) -> &str {
        &self.id
    }
}

fn run_scenario() -> u64 {
    let obs = Observation::new(
        TestSource {
            id: "BTC".to_string(),
        },
        100,
        vec![10, 20, 30],
    )
    .unwrap();
    let snap = snapshot(&obs);
    snap.identity
}

#[test]
fn test_e2e_no_backflow() {
    let obs = Observation::new(
        TestSource {
            id: "BTC".to_string(),
        },
        100,
        vec![10, 20, 30],
    )
    .unwrap();

    let snap_before = snapshot(&obs);
    let obs_id = IdentityAdapter::derive_identity(&obs);

    let _dummy = 1;

    let snap_after = snapshot(&obs);

    assert_eq!(snap_before.identity, snap_after.identity);
    assert_eq!(snap_before.identity, obs_id);
}

#[test]
fn test_e2e_deterministic_double_run() {
    let snap1 = run_scenario();
    let snap2 = run_scenario();

    assert_eq!(snap1, snap2);
}
