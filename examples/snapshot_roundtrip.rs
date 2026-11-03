//! Producer and consumer are separate invocations; consumer imports no Registry.
#[allow(dead_code)]
#[path = "support/snapshot_fixture.rs"]
mod fixture;
use mdc_core::identity::IdentityProtocol;
use mdc_core::pipeline::{SnapshotLimits, SnapshotSession};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "produce|consume archive-path");
    let limits = SnapshotLimits::default();
    let session = match args[1].as_str() {
        "produce" => {
            let mut graph = fixture::graph(IdentityProtocol::PortableV1);
            fixture::enrich(&mut graph);
            std::fs::write(&args[2], graph.session.export(limits).unwrap()).unwrap();
            std::fs::write(
                format!("{}.state", args[2]),
                graph.session.state_manifest(limits).unwrap(),
            )
            .unwrap();
            graph.session
        }
        "consume" => {
            let bytes = std::fs::read(&args[2]).unwrap();
            let restored = SnapshotSession::restore(&bytes, limits).unwrap();
            assert_eq!(restored.export(limits).unwrap(), bytes);
            assert_eq!(
                restored.state_manifest(limits).unwrap(),
                std::fs::read(format!("{}.state", args[2])).unwrap()
            );
            restored
        }
        _ => panic!("unknown mode"),
    };
    println!("MDC_SNAPSHOT_VERIFY=PASS operations={} objects={} revision={} rejections={} lifecycle_rejections={}", session.operation_count(), session.inventory().len(), session.pipeline().lifecycle_revision(), session.pipeline().rejections().len(), session.pipeline().lifecycle_rejections().len());
}
