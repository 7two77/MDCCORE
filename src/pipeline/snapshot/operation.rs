use super::{Reader, SnapshotError, SnapshotLimits, Writer};
use crate::contract::lifecycle::InvalidationReason;
use crate::model::lifecycle::{InvalidationRequest, LifecycleError, LifecycleEventId};
use crate::pipeline::*;
use crate::provenance::ObjectRef;
use alloc::vec::Vec;

pub(super) enum Operation {
    Admit(ObservationRequest),
    Measure(MeasurementRequest),
    Relate(RelationRequest),
    Compose(CompositionRequest),
    State(StateRequest),
    Transition(TransitionRequest),
    Evaluate(EvaluationRequest),
    EvaluateKeyed(EvaluationRequest),
    Invalidate(InvalidationRequest),
}
#[derive(Clone)]
pub(super) enum Outcome {
    Object(Result<ObjectRef, PipelineError>),
    Lifecycle(Result<LifecycleEventId, LifecycleError>),
}

fn refs(values: &[ObjectRef], w: &mut Writer) -> Result<(), SnapshotError> {
    w.len(values.len())?;
    for value in values {
        w.object_ref(value)?;
    }
    Ok(())
}
fn read_refs(r: &mut Reader<'_>) -> Result<Vec<ObjectRef>, SnapshotError> {
    let count = r.len()?;
    (0..count).map(|_| r.object_ref()).collect()
}
fn optional(value: Option<u64>, w: &mut Writer) -> Result<(), SnapshotError> {
    w.boolean(value.is_some())?;
    if let Some(value) = value {
        w.u64(value)?;
    }
    Ok(())
}
fn read_optional(r: &mut Reader<'_>) -> Result<Option<u64>, SnapshotError> {
    if r.boolean()? {
        Ok(Some(r.u64()?))
    } else {
        Ok(None)
    }
}

pub(super) fn encode_operation(op: &Operation, w: &mut Writer) -> Result<(), SnapshotError> {
    match op {
        Operation::Admit(r) => {
            w.u8(0)?;
            w.reference(&r.source)?;
            optional(r.timestamp, w)?;
            w.bytes(&r.payload)?;
            w.string(&r.acquisition_context)?;
        }
        Operation::Measure(r) => {
            w.u8(1)?;
            w.reference(&r.operator)?;
            refs(&r.inputs, w)?;
        }
        Operation::Relate(r) => {
            w.u8(2)?;
            w.reference(&r.contract)?;
            w.object_ref(&r.left)?;
            w.object_ref(&r.right)?;
        }
        Operation::Compose(r) => {
            w.u8(3)?;
            w.reference(&r.contract)?;
            refs(&r.members, w)?;
            refs(&r.relations, w)?;
        }
        Operation::State(r) => {
            w.u8(4)?;
            w.reference(&r.contract)?;
            w.object_ref(&r.composition)?;
            optional(r.observation_horizon, w)?;
        }
        Operation::Transition(r) => {
            w.u8(5)?;
            w.reference(&r.contract)?;
            w.object_ref(&r.source)?;
            w.object_ref(&r.target)?;
            optional(r.observation_horizon, w)?;
        }
        Operation::Evaluate(r) | Operation::EvaluateKeyed(r) => {
            w.u8(if matches!(op, Operation::Evaluate(_)) {
                6
            } else {
                7
            })?;
            w.reference(&r.contract)?;
            w.object_ref(&r.target)?;
            w.dynamics(&r.context)?;
        }
        Operation::Invalidate(r) => {
            w.u8(8)?;
            encode_invalidation(r, w)?;
        }
    }
    Ok(())
}
pub(super) fn decode_operation(r: &mut Reader<'_>) -> Result<Operation, SnapshotError> {
    Ok(match r.u8()? {
        0 => Operation::Admit(ObservationRequest {
            source: r.reference()?,
            timestamp: read_optional(r)?,
            payload: r.bytes()?,
            acquisition_context: r.string()?,
        }),
        1 => Operation::Measure(MeasurementRequest {
            operator: r.reference()?,
            inputs: read_refs(r)?,
        }),
        2 => Operation::Relate(RelationRequest {
            contract: r.reference()?,
            left: r.object_ref()?,
            right: r.object_ref()?,
        }),
        3 => Operation::Compose(CompositionRequest {
            contract: r.reference()?,
            members: read_refs(r)?,
            relations: read_refs(r)?,
        }),
        4 => Operation::State(StateRequest {
            contract: r.reference()?,
            composition: r.object_ref()?,
            observation_horizon: read_optional(r)?,
        }),
        5 => Operation::Transition(TransitionRequest {
            contract: r.reference()?,
            source: r.object_ref()?,
            target: r.object_ref()?,
            observation_horizon: read_optional(r)?,
        }),
        tag @ (6 | 7) => {
            let request = EvaluationRequest {
                contract: r.reference()?,
                target: r.object_ref()?,
                context: r.dynamics()?,
            };
            if tag == 6 {
                Operation::Evaluate(request)
            } else {
                Operation::EvaluateKeyed(request)
            }
        }
        8 => Operation::Invalidate(read_invalidation(r)?),
        tag => {
            return Err(SnapshotError::InvalidTag {
                kind: "operation",
                tag,
            })
        }
    })
}
pub(super) fn encode_rejected_request(
    r: &RejectedRequest,
    w: &mut Writer,
) -> Result<(), SnapshotError> {
    let op = match r {
        RejectedRequest::Observation(r) => Operation::Admit(r.clone()),
        RejectedRequest::Measurement(r) => Operation::Measure(r.clone()),
        RejectedRequest::Relation(r) => Operation::Relate(r.clone()),
        RejectedRequest::Composition(r) => Operation::Compose(r.clone()),
        RejectedRequest::State(r) => Operation::State(r.clone()),
        RejectedRequest::Transition(r) => Operation::Transition(r.clone()),
    };
    encode_operation(&op, w)
}
pub(super) fn encode_invalidation(
    r: &InvalidationRequest,
    w: &mut Writer,
) -> Result<(), SnapshotError> {
    w.string(&r.request_id)?;
    w.reference(&r.contract)?;
    w.object_ref(&r.target)?;
    w.u8(r.reason as u8)?;
    refs(&r.evidence, w)?;
    w.u64(r.expected_revision)
}
fn read_invalidation(r: &mut Reader<'_>) -> Result<InvalidationRequest, SnapshotError> {
    let request_id = r.string()?;
    let contract = r.reference()?;
    let target = r.object_ref()?;
    let reason = match r.u8()? {
        0 => InvalidationReason::SourceContractViolation,
        1 => InvalidationReason::ContradictoryTemporalIdentity,
        2 => InvalidationReason::DerivationContractViolation,
        3 => InvalidationReason::EvidenceWithdrawn,
        tag => {
            return Err(SnapshotError::InvalidTag {
                kind: "invalidation reason",
                tag,
            })
        }
    };
    Ok(InvalidationRequest {
        request_id,
        contract,
        target,
        reason,
        evidence: read_refs(r)?,
        expected_revision: r.u64()?,
    })
}
pub(super) fn encode_pipeline_error(
    error: &PipelineError,
    w: &mut Writer,
) -> Result<(), SnapshotError> {
    let mut bytes = Vec::new();
    crate::contract::invalid_encoding::put_pipeline_error(&mut bytes, error);
    w.bytes(&bytes)?;
    // Existing semantic error encoding stores event keys; exact runtime outcomes
    // additionally retain each digest rather than deriving it from the key.
    if let PipelineError::InputInvalidated { events, .. } = error {
        w.len(events.len())?;
        for event in events {
            w.u64(event.digest())?;
            w.bytes(event.canonical_key())?;
        }
    }
    Ok(())
}
pub(super) fn encode_lifecycle_error(
    error: &LifecycleError,
    w: &mut Writer,
) -> Result<(), SnapshotError> {
    match error {
        LifecycleError::ContractUnresolved(v) => {
            w.u8(0)?;
            w.reference(v)?;
        }
        LifecycleError::TargetUnresolved(v) => {
            w.u8(1)?;
            w.object_ref(v)?;
        }
        LifecycleError::UnsupportedTarget(v) => {
            w.u8(2)?;
            w.object_ref(v)?;
        }
        LifecycleError::ContextMismatch => w.u8(3)?,
        LifecycleError::ReasonNotAllowed => w.u8(4)?,
        LifecycleError::EmptyRequestId => w.u8(5)?,
        LifecycleError::EmptyEvidence => w.u8(6)?,
        LifecycleError::DuplicateEvidence => w.u8(7)?,
        LifecycleError::EvidenceInvalid(v) => {
            w.u8(8)?;
            w.object_ref(v)?;
        }
        LifecycleError::RevisionConflict { expected, actual } => {
            w.u8(9)?;
            w.u64(*expected)?;
            w.u64(*actual)?;
        }
        LifecycleError::RequestIdConflict => w.u8(10)?,
        LifecycleError::FutureRevision => w.u8(11)?,
        LifecycleError::NotYetAdmitted(v) => {
            w.u8(12)?;
            w.object_ref(v)?;
        }
    }
    Ok(())
}
pub(super) fn outcome_bytes(
    outcome: &Outcome,
    limits: SnapshotLimits,
) -> Result<Vec<u8>, SnapshotError> {
    let mut w = Writer::new(limits);
    match outcome {
        Outcome::Object(Ok(value)) => {
            w.u8(0)?;
            w.object_ref(value)?;
        }
        Outcome::Object(Err(error)) => {
            w.u8(1)?;
            encode_pipeline_error(error, &mut w)?;
        }
        Outcome::Lifecycle(Ok(id)) => {
            w.u8(2)?;
            w.u64(id.digest())?;
            w.bytes(id.canonical_key())?;
        }
        Outcome::Lifecycle(Err(error)) => {
            w.u8(3)?;
            encode_lifecycle_error(error, &mut w)?;
        }
    }
    Ok(w.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_operation_tag_is_not_reinterpreted() {
        let mut reader = Reader::new(&[255], SnapshotLimits::default()).unwrap();
        assert!(matches!(
            decode_operation(&mut reader),
            Err(SnapshotError::InvalidTag {
                kind: "operation",
                tag: 255
            })
        ));
    }
}
