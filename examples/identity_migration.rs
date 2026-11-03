#[allow(dead_code)]
#[path = "lifecycle.rs"]
mod scenario;
use mdc_core::identity::IdentityProtocol;

fn main() {
    let old = scenario::graph();
    let preserved = scenario::capture(&old.p, &old.refs);
    let new =
        scenario::graph_with_identity_protocol(scenario::registry(), IdentityProtocol::PortableV1);
    // Semantic role is declared by this fixture. This is replay, not import of
    // legacy IDs into the portable graph or rewriting historical snapshots.
    for ((role, legacy), portable) in ["O", "M", "R", "C", "S", "T"]
        .iter()
        .zip(&old.roots)
        .zip(&new.roots)
    {
        assert_ne!(legacy, portable);
        println!("{role} legacy={legacy:?} portable={portable:?}");
    }
    assert_eq!(preserved, scenario::capture(&old.p, &old.refs));
    println!(
        "MDC_IDENTITY_MIGRATION_VERIFY=PASS legacy_snapshots_preserved={}",
        preserved.len()
    );
}
