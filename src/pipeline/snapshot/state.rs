//! Exact retained-state oracle for recorded-session replay.
//!
//! This is a versioned storage manifest, not a replacement semantic identity.
//! Storage keys, canonical bodies and hidden lifecycle/alias/audit state are
//! encoded separately so replay cannot hide a changed mapping or cached value.

use super::operation::{
    encode_invalidation, encode_lifecycle_error, encode_pipeline_error, encode_rejected_request,
};
use super::wire::Writer;
use super::{SnapshotError, SnapshotLimits};
use crate::contract::evaluation::LifecycleStatus;
use crate::contract::registry::snapshot::encode_registry;
use crate::identity::CanonicalSemanticBasis;
use crate::pipeline::Pipeline;
use crate::provenance::{DerivationProvenance, ObjectRef};
use alloc::vec::Vec;

const MANIFEST_MAGIC: &[u8] = b"MDC-SNAPSHOT-001/state/v1\0";

fn status(writer: &mut Writer, value: LifecycleStatus) -> Result<(), SnapshotError> {
    writer.u8(match value {
        LifecycleStatus::Candidate => 0,
        LifecycleStatus::Conformant => 1,
        LifecycleStatus::Invalid => 2,
    })
}

fn provenance(writer: &mut Writer, value: &DerivationProvenance) -> Result<(), SnapshotError> {
    writer.reference(value.contract())?;
    writer.len(value.inputs().len())?;
    for reference in value.inputs() {
        writer.object_ref(reference)?;
    }
    Ok(())
}

/// Include every retained artifact exactly once under its primary handle.
/// Registry Sources and the separate lifecycle-event namespace are excluded.
pub(super) fn inventory(pipeline: &Pipeline) -> Vec<ObjectRef> {
    let mut refs = Vec::new();
    refs.extend(
        pipeline
            .observations
            .keys()
            .copied()
            .map(ObjectRef::Observation),
    );
    refs.extend(
        pipeline
            .measurements
            .keys()
            .copied()
            .map(ObjectRef::Measurement),
    );
    refs.extend(pipeline.relations.keys().copied().map(ObjectRef::Relation));
    refs.extend(
        pipeline
            .compositions
            .keys()
            .copied()
            .map(ObjectRef::Composition),
    );
    refs.extend(pipeline.states.keys().copied().map(ObjectRef::State));
    refs.extend(
        pipeline
            .transitions
            .keys()
            .copied()
            .map(ObjectRef::Transition),
    );
    refs.extend(pipeline.evaluation_references());
    refs.extend(pipeline.invalid_objects.keys().cloned());
    refs.retain(|reference| !matches!(reference, ObjectRef::Source(_)));
    refs.sort();
    refs.dedup();
    refs
}

pub(super) fn manifest(
    pipeline: &Pipeline,
    limits: SnapshotLimits,
) -> Result<Vec<u8>, SnapshotError> {
    if pipeline.failed_candidate.is_some() {
        return Err(SnapshotError::InvalidSessionState(
            "computed candidate pending",
        ));
    }
    let mut writer = Writer::new(limits);
    writer.raw(MANIFEST_MAGIC)?;
    writer.string(pipeline.identity_protocol.name())?;
    writer.u64(pipeline.lifecycle_revision())?;

    writer.u8(0)?;
    encode_registry(&pipeline.registry, &mut writer)?;

    writer.u8(1)?;
    writer.len(pipeline.observations.len())?;
    for (stored_id, object) in &pipeline.observations {
        writer.u64(*stored_id)?;
        writer.u64(object.id())?;
        writer.bytes(&object.canonical_basis())?;
        writer.signed(object.scalar_value())?;
        writer.string(object.provenance().identity_profile())?;
        status(&mut writer, object.status())?;
    }

    writer.u8(2)?;
    writer.len(pipeline.measurements.len())?;
    for (stored_id, object) in &pipeline.measurements {
        writer.u64(*stored_id)?;
        writer.u64(object.id())?;
        writer.bytes(&object.canonical_basis())?;
        status(&mut writer, object.status())?;
        provenance(&mut writer, object.provenance())?;
    }

    writer.u8(3)?;
    writer.len(pipeline.relations.len())?;
    for (stored_id, object) in &pipeline.relations {
        writer.u64(*stored_id)?;
        writer.u64(object.id())?;
        writer.bytes(&object.canonical_basis())?;
        status(&mut writer, object.status())?;
        provenance(&mut writer, object.provenance())?;
    }

    writer.u8(4)?;
    writer.len(pipeline.compositions.len())?;
    for (stored_id, object) in &pipeline.compositions {
        writer.u64(*stored_id)?;
        writer.u64(object.id())?;
        writer.bytes(&object.canonical_basis())?;
        status(&mut writer, object.status())?;
        provenance(&mut writer, object.provenance())?;
    }

    writer.u8(5)?;
    writer.len(pipeline.states.len())?;
    for (stored_id, object) in &pipeline.states {
        writer.u64(*stored_id)?;
        writer.u64(object.id())?;
        writer.bytes(&object.canonical_basis())?;
        status(&mut writer, object.status())?;
        provenance(&mut writer, object.provenance())?;
    }

    writer.u8(6)?;
    writer.len(pipeline.transitions.len())?;
    for (stored_id, object) in &pipeline.transitions {
        writer.u64(*stored_id)?;
        writer.u64(object.id())?;
        writer.bytes(&object.canonical_basis())?;
        status(&mut writer, object.status())?;
        provenance(&mut writer, object.provenance())?;
    }

    writer.u8(7)?;
    writer.len(pipeline.evaluations.len())?;
    for (stored_key, object) in &pipeline.evaluations {
        writer.u64(stored_key.digest())?;
        writer.bytes(stored_key.canonical_key())?;
        writer.u64(object.id())?;
        writer.bytes(&object.canonical_basis())?;
        writer.object_ref(&pipeline.preferred_evaluation_reference(stored_key))?;
        status(&mut writer, object.status())?;
        provenance(&mut writer, object.provenance())?;
    }

    writer.u8(8)?;
    writer.len(pipeline.evaluation_aliases.len())?;
    for (alias, key) in &pipeline.evaluation_aliases {
        writer.u64(*alias)?;
        writer.u64(key.digest())?;
        writer.bytes(key.canonical_key())?;
    }

    writer.u8(9)?;
    writer.len(pipeline.invalid_objects.len())?;
    for (stored_reference, object) in &pipeline.invalid_objects {
        writer.object_ref(stored_reference)?;
        writer.object_ref(&object.reference())?;
        writer.u64(object.id().digest())?;
        writer.bytes(object.id().canonical_key())?;
        writer.bytes(&object.canonical_basis())?;
        status(&mut writer, object.status())?;
    }

    writer.u8(10)?;
    writer.len(pipeline.lifecycle_birth.len())?;
    for (reference, revision) in &pipeline.lifecycle_birth {
        writer.object_ref(reference)?;
        writer.u64(*revision)?;
    }

    writer.u8(11)?;
    writer.len(pipeline.lifecycle_invalid.len())?;
    for reference in &pipeline.lifecycle_invalid {
        writer.object_ref(reference)?;
    }

    writer.u8(12)?;
    writer.len(pipeline.lifecycle_events.len())?;
    for event in &pipeline.lifecycle_events {
        writer.u64(event.id().digest())?;
        writer.bytes(event.id().canonical_key())?;
        writer.bytes(&event.canonical_basis())?;
    }

    writer.u8(13)?;
    writer.len(pipeline.rejections.len())?;
    for rejection in &pipeline.rejections {
        encode_rejected_request(rejection.request(), &mut writer)?;
        encode_pipeline_error(rejection.error(), &mut writer)?;
        writer.object_ref(rejection.invalid_reference())?;
    }

    writer.u8(14)?;
    writer.len(pipeline.lifecycle_rejections.len())?;
    for rejection in &pipeline.lifecycle_rejections {
        encode_invalidation(&rejection.request, &mut writer)?;
        encode_lifecycle_error(&rejection.error, &mut writer)?;
        writer.u64(rejection.observed_revision)?;
    }

    let bytes = writer.finish();
    if bytes.len() > limits.max_manifest_bytes {
        return Err(SnapshotError::LimitExceeded("max_manifest_bytes"));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::dynamics::DynamicsContext;
    use crate::contract::registry::{
        DomainContext, OperatorContract, OperatorKind, Registry, ScalarSchema, SchemaContract,
        SourceContract, VersionedId,
    };
    use crate::contract::structure::{RelationContext, TemporalScope};
    use crate::pipeline::{
        EvaluationRequest, MeasurementRequest, ObservationRequest, RejectedRequest,
    };
    use alloc::boxed::Box;
    use alloc::vec;

    fn id(name: &str) -> VersionedId {
        VersionedId::new(name, 1).unwrap()
    }

    fn fixture() -> (Pipeline, ObjectRef, ObjectRef, ObjectRef) {
        let domain = DomainContext::new("manifest", id("ticks")).unwrap();
        let mut registry = Registry::new();
        registry
            .register_schema(SchemaContract {
                reference: id("schema"),
                format: ScalarSchema::SignedI64Le,
            })
            .unwrap();
        registry
            .register_source(SourceContract {
                reference: id("source"),
                schema: id("schema"),
                context: domain.clone(),
                unit: "count".into(),
            })
            .unwrap();
        registry
            .register_operator(OperatorContract {
                reference: id("read"),
                kind: OperatorKind::Scalar,
            })
            .unwrap();
        let mut pipeline = Pipeline::new(registry);
        let observation = pipeline
            .admit(ObservationRequest {
                source: id("source"),
                timestamp: Some(10),
                payload: 4_i64.to_le_bytes().to_vec(),
                acquisition_context: "record/10".into(),
            })
            .unwrap();
        let measurement = pipeline
            .measure(MeasurementRequest {
                operator: id("read"),
                inputs: vec![observation.clone()],
            })
            .unwrap();
        let evaluation = pipeline
            .evaluate(EvaluationRequest {
                contract: id("missing/check"),
                target: observation.clone(),
                context: DynamicsContext::new(
                    RelationContext::new(
                        domain,
                        TemporalScope::new(0, 100).unwrap(),
                        id("frame"),
                        id("causal"),
                    ),
                    id("calculus"),
                    id("scalar"),
                ),
            })
            .unwrap();
        (pipeline, observation, measurement, evaluation)
    }

    #[test]
    fn hidden_alias_and_lifecycle_maps_are_part_of_the_exact_state() {
        let (mut pipeline, observation, _, evaluation) = fixture();
        let original = manifest(&pipeline, SnapshotLimits::default()).unwrap();
        let key = pipeline.evaluation_key(&evaluation).unwrap().clone();
        let alias = pipeline.evaluation_aliases.remove(&key.digest()).unwrap();
        assert_ne!(
            manifest(&pipeline, SnapshotLimits::default()).unwrap(),
            original
        );
        pipeline.evaluation_aliases.insert(key.digest(), alias);
        assert_eq!(
            manifest(&pipeline, SnapshotLimits::default()).unwrap(),
            original
        );

        pipeline.lifecycle_birth.insert(observation.clone(), 1);
        assert_ne!(
            manifest(&pipeline, SnapshotLimits::default()).unwrap(),
            original
        );
        pipeline.lifecycle_birth.insert(observation.clone(), 0);
        pipeline.lifecycle_invalid.insert(observation.clone());
        assert_ne!(
            manifest(&pipeline, SnapshotLimits::default()).unwrap(),
            original
        );
        pipeline.lifecycle_invalid.remove(&observation);
        assert_eq!(
            manifest(&pipeline, SnapshotLimits::default()).unwrap(),
            original
        );
    }

    #[test]
    fn original_rejection_receipt_is_checked_separately_from_its_invalid_artifact() {
        let (mut pipeline, _, _, _) = fixture();
        assert!(pipeline
            .admit(ObservationRequest {
                source: id("source"),
                timestamp: None,
                payload: 4_i64.to_le_bytes().to_vec(),
                acquisition_context: "rejected/first".into(),
            })
            .is_err());
        let before = manifest(&pipeline, SnapshotLimits::default()).unwrap();
        let reference = pipeline.rejections[0].invalid_reference().clone();
        let invalid_body = pipeline
            .resolve_invalid(&reference)
            .unwrap()
            .canonical_basis();
        let RejectedRequest::Observation(request) = &mut pipeline.rejections[0].request else {
            panic!("observation rejection expected")
        };
        request.acquisition_context = "rejected/changed".into();
        assert_eq!(
            pipeline
                .resolve_invalid(&reference)
                .unwrap()
                .canonical_basis(),
            invalid_body
        );
        assert_ne!(
            manifest(&pipeline, SnapshotLimits::default()).unwrap(),
            before
        );
    }

    #[test]
    fn inventory_exposes_one_primary_evaluation_and_retained_invalid_objects() {
        let (mut pipeline, observation, measurement, evaluation) = fixture();
        let initial = inventory(&pipeline);
        assert_eq!(initial.len(), 3);
        for reference in [&observation, &measurement, &evaluation] {
            assert!(initial.contains(reference));
        }
        let full_key = pipeline
            .canonical_evaluation_reference(&evaluation)
            .unwrap();
        assert!(!initial.contains(&full_key));
        assert!(pipeline
            .measure(MeasurementRequest {
                operator: id("missing/operator"),
                inputs: vec![observation],
            })
            .is_err());
        let refs = inventory(&pipeline);
        assert_eq!(refs.len(), 4);
        assert!(refs.contains(pipeline.rejections[0].invalid_reference()));
        assert!(!refs
            .iter()
            .any(|reference| matches!(reference, ObjectRef::Source(_))));
        assert!(refs.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn pending_candidates_and_manifest_budget_cannot_be_exported() {
        let (mut pipeline, _, measurement, _) = fixture();
        let limits = SnapshotLimits {
            max_manifest_bytes: 1,
            ..SnapshotLimits::default()
        };
        assert_eq!(
            manifest(&pipeline, limits),
            Err(SnapshotError::LimitExceeded("max_manifest_bytes"))
        );
        pipeline.failed_candidate = Some(Box::new(
            crate::pipeline::invalid::ComputedCandidate::Measurement(
                pipeline.resolve_measurement(&measurement).unwrap().clone(),
            ),
        ));
        assert_eq!(
            manifest(&pipeline, SnapshotLimits::default()),
            Err(SnapshotError::InvalidSessionState(
                "computed candidate pending"
            ))
        );
    }
}
