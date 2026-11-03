//! Explicit version-one encoding for invalid semantic bodies and failures.
//! No debug/display output participates in these identity bytes.
use super::dynamics::*;
use super::encoding::{
    put_bytes, put_context, put_dynamics_context, put_object_ref, put_reference,
    put_relation_context, put_str,
};
use super::evaluation::ConformanceResult;
use super::registry::*;
use super::structure::*;
use crate::model::composition::{CompositionError, ConstraintResult};
use crate::model::invalid::InvalidBody;
use crate::model::measurement::MeasurementError;
use crate::model::observation::AdmissionError;
use crate::model::relation::{RelationError, RelationValue};
use crate::model::state::{StateCriterionResult, StateError};
use crate::model::transition::{TransitionConditionResult, TransitionError};
use crate::pipeline::PipelineError;
use alloc::vec::Vec;

fn sequence<T>(out: &mut Vec<u8>, values: &[T], mut encode: impl FnMut(&mut Vec<u8>, &T)) {
    out.extend_from_slice(&(values.len() as u64).to_le_bytes());
    for value in values {
        encode(out, value);
    }
}
fn option<T>(out: &mut Vec<u8>, value: &Option<T>, encode: impl FnOnce(&mut Vec<u8>, &T)) {
    match value {
        Some(value) => {
            out.push(1);
            encode(out, value);
        }
        None => out.push(0),
    }
}
fn integer(out: &mut Vec<u8>, value: &u64) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn signed(out: &mut Vec<u8>, value: &i128) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn count(out: &mut Vec<u8>, value: usize) {
    out.extend_from_slice(&(value as u64).to_le_bytes());
}
fn result(out: &mut Vec<u8>, value: ConformanceResult) {
    out.push(match value {
        ConformanceResult::Pass => 0,
        ConformanceResult::Fail => 1,
        ConformanceResult::Unresolved => 2,
    });
}
fn order(out: &mut Vec<u8>, value: CriterionOrder) {
    out.push(match value {
        CriterionOrder::Ordered => 0,
        CriterionOrder::Unordered => 1,
    });
}
fn scope(out: &mut Vec<u8>, value: &TemporalScope) {
    integer(out, &value.start());
    integer(out, &value.end());
}

pub(crate) fn put_source_contract(out: &mut Vec<u8>, value: &SourceContract) {
    put_reference(out, &value.reference);
    put_reference(out, &value.schema);
    put_context(out, &value.context);
    put_str(out, &value.unit);
}
fn schema(out: &mut Vec<u8>, value: &SchemaContract) {
    put_reference(out, &value.reference);
    match value.format {
        ScalarSchema::SignedI64Le => out.push(0),
    }
}
fn operator(out: &mut Vec<u8>, value: &OperatorContract) {
    put_reference(out, &value.reference);
    out.push(match value.kind {
        OperatorKind::Scalar => 0,
        OperatorKind::Difference => 1,
    });
}
fn relation_kind(out: &mut Vec<u8>, value: &RelationKind) {
    match value {
        RelationKind::Precedes => out.push(0),
        RelationKind::TemporalDistance => out.push(1),
        RelationKind::Adjacent { step, symmetric } => {
            out.push(2);
            integer(out, step);
            out.push(u8::from(*symmetric));
        }
        RelationKind::Proportion => out.push(3),
    }
}
fn relation(out: &mut Vec<u8>, value: &RelationContract) {
    put_reference(out, &value.reference);
    relation_kind(out, &value.kind);
    put_relation_context(out, &value.context);
}
fn relation_value(out: &mut Vec<u8>, value: &RelationValue) {
    match value {
        RelationValue::Established => out.push(0),
        RelationValue::TemporalDistance(value) => {
            out.push(1);
            signed(out, value);
        }
        RelationValue::Proportion {
            numerator,
            denominator,
        } => {
            out.push(2);
            signed(out, numerator);
            signed(out, denominator);
        }
    }
}
fn constraint_kind(out: &mut Vec<u8>, value: &ConstraintKind) {
    match value {
        ConstraintKind::Cardinality { min, max } => {
            out.push(0);
            out.extend_from_slice(&min.to_le_bytes());
            out.extend_from_slice(&max.to_le_bytes());
        }
        ConstraintKind::SameUnits => out.push(1),
        ConstraintKind::Connected => out.push(2),
        ConstraintKind::Acyclic => out.push(3),
        ConstraintKind::OrderedChain { relation } => {
            out.push(4);
            put_reference(out, relation);
        }
        ConstraintKind::RequiresRelation { relation } => {
            out.push(5);
            put_reference(out, relation);
        }
    }
}
fn composition(out: &mut Vec<u8>, value: &CompositionContract) {
    put_reference(out, &value.reference);
    out.push(match value.member_order {
        MemberOrder::Ordered => 0,
        MemberOrder::Unordered => 1,
    });
    out.push(match value.member_type {
        MemberType::Observations => 0,
        MemberType::Measurements => 1,
        MemberType::ScalarInputs => 2,
    });
    sequence(out, &value.constraints, put_reference);
    put_relation_context(out, &value.context);
}
fn state_kind(out: &mut Vec<u8>, value: &StateCriterionKind) {
    match value {
        StateCriterionKind::Cardinality { min, max } => {
            out.push(0);
            out.extend_from_slice(&min.to_le_bytes());
            out.extend_from_slice(&max.to_le_bytes());
        }
        StateCriterionKind::HasRelation { relation } => {
            out.push(1);
            put_reference(out, relation);
        }
        StateCriterionKind::ConstraintPassed { constraint } => {
            out.push(2);
            put_reference(out, constraint);
        }
        StateCriterionKind::MaxTemporalSpan { ticks } => {
            out.push(3);
            integer(out, ticks);
        }
        StateCriterionKind::ObservationAfter { source } => {
            out.push(4);
            put_reference(out, source);
        }
    }
}
fn state(out: &mut Vec<u8>, value: &StateContract) {
    put_reference(out, &value.reference);
    put_reference(out, &value.composition);
    match value.definition {
        StateDefinition::CompositionAbstraction => out.push(0),
    }
    order(out, value.criterion_order);
    sequence(out, &value.criteria, put_reference);
    put_dynamics_context(out, &value.context);
}
fn transition_kind(out: &mut Vec<u8>, value: &TransitionConditionKind) {
    match value {
        TransitionConditionKind::TemporalOrder { strict } => {
            out.push(0);
            out.push(u8::from(*strict));
        }
        TransitionConditionKind::SourceCriteriaPass => out.push(1),
        TransitionConditionKind::TargetCriteriaPass => out.push(2),
        TransitionConditionKind::ObservationAfterTarget { source } => {
            out.push(3);
            put_reference(out, source);
        }
    }
}
fn transition(out: &mut Vec<u8>, value: &TransitionContract) {
    put_reference(out, &value.reference);
    put_reference(out, &value.source_state);
    put_reference(out, &value.target_state);
    order(out, value.condition_order);
    sequence(out, &value.conditions, put_reference);
    put_dynamics_context(out, &value.context);
}
fn constraint_result(out: &mut Vec<u8>, value: &ConstraintResult) {
    put_reference(out, value.reference());
    constraint_kind(out, value.kind());
    result(out, value.result());
}
fn state_result(out: &mut Vec<u8>, value: &StateCriterionResult) {
    put_reference(out, value.reference());
    state_kind(out, value.kind());
    result(out, value.result());
    sequence(out, value.evidence(), put_object_ref);
}
fn transition_result(out: &mut Vec<u8>, value: &TransitionConditionResult) {
    put_reference(out, value.reference());
    transition_kind(out, value.kind());
    result(out, value.result());
    sequence(out, value.evidence(), put_object_ref);
}

pub(crate) fn put_invalid_body(out: &mut Vec<u8>, body: &InvalidBody) {
    match body {
        InvalidBody::Observation {
            source,
            definition,
            schema: source_schema,
            timestamp,
            payload,
            acquisition_context,
        } => {
            out.push(0);
            put_reference(out, source);
            option(out, definition, put_source_contract);
            option(out, source_schema, schema);
            option(out, timestamp, integer);
            put_bytes(out, payload);
            put_str(out, acquisition_context);
        }
        InvalidBody::Measurement {
            operator: operator_ref,
            definition,
            inputs,
            value,
            unit,
            context,
        } => {
            out.push(1);
            put_reference(out, operator_ref);
            option(out, definition, operator);
            sequence(out, inputs, put_object_ref);
            option(out, value, signed);
            option(out, unit, |out, value| put_str(out, value));
            option(out, context, put_context);
        }
        InvalidBody::Relation {
            contract,
            definition,
            left,
            right,
            value,
        } => {
            out.push(2);
            put_reference(out, contract);
            option(out, definition, relation);
            put_object_ref(out, left);
            put_object_ref(out, right);
            option(out, value, relation_value);
        }
        InvalidBody::Composition {
            contract,
            definition,
            members,
            relations,
            constraints,
        } => {
            out.push(3);
            put_reference(out, contract);
            option(out, definition, composition);
            sequence(out, members, put_object_ref);
            sequence(out, relations, put_object_ref);
            option(out, constraints, |out, values| {
                sequence(out, values, constraint_result)
            });
        }
        InvalidBody::State {
            contract,
            definition,
            source_composition,
            observation_horizon,
            interval,
            criteria,
        } => {
            out.push(4);
            put_reference(out, contract);
            option(out, definition, state);
            put_object_ref(out, source_composition);
            option(out, observation_horizon, integer);
            option(out, interval, scope);
            option(out, criteria, |out, values| {
                sequence(out, values, state_result)
            });
        }
        InvalidBody::Transition {
            contract,
            definition,
            source_state,
            target_state,
            observation_horizon,
            conditions,
        } => {
            out.push(5);
            put_reference(out, contract);
            option(out, definition, transition);
            put_object_ref(out, source_state);
            put_object_ref(out, target_state);
            option(out, observation_horizon, integer);
            option(out, conditions, |out, values| {
                sequence(out, values, transition_result)
            });
        }
    }
}

fn admission_error(out: &mut Vec<u8>, error: &AdmissionError) {
    match error {
        AdmissionError::MissingTimestamp => out.push(0),
        AdmissionError::EmptyAcquisitionContext => out.push(1),
        AdmissionError::InvalidPayload { expected, actual } => {
            out.push(2);
            count(out, *expected);
            count(out, *actual);
        }
    }
}
fn measurement_error(out: &mut Vec<u8>, error: &MeasurementError) {
    match error {
        MeasurementError::WrongInputCount { expected, actual } => {
            out.push(0);
            count(out, *expected);
            count(out, *actual);
        }
        MeasurementError::IncompatibleContext => out.push(1),
        MeasurementError::IncompatibleUnits => out.push(2),
        MeasurementError::ArithmeticOverflow => out.push(3),
    }
}
fn relation_error(out: &mut Vec<u8>, error: &RelationError) {
    match error {
        RelationError::InvalidInputType(reference) => {
            out.push(0);
            put_object_ref(out, reference);
        }
        RelationError::IncompatibleContext => out.push(1),
        RelationError::IncompatibleUnits => out.push(2),
        RelationError::OutsideTemporalScope => out.push(3),
        RelationError::TemporalOrder => out.push(4),
        RelationError::NotAdjacent => out.push(5),
        RelationError::ZeroDenominator => out.push(6),
    }
}
fn composition_error(out: &mut Vec<u8>, error: &CompositionError) {
    match error {
        CompositionError::EmptyMembers => out.push(0),
        CompositionError::InvalidMemberType(reference) => {
            out.push(1);
            put_object_ref(out, reference);
        }
        CompositionError::DuplicateMember(reference) => {
            out.push(2);
            put_object_ref(out, reference);
        }
        CompositionError::DuplicateRelation(reference) => {
            out.push(3);
            put_object_ref(out, reference);
        }
        CompositionError::IncompatibleContext => out.push(4),
        CompositionError::OutsideTemporalScope => out.push(5),
        CompositionError::RelationContextMismatch(reference) => {
            out.push(6);
            put_object_ref(out, reference);
        }
        CompositionError::ExternalEndpoint(reference) => {
            out.push(7);
            put_object_ref(out, reference);
        }
        CompositionError::ConstraintSetMismatch => out.push(8),
        CompositionError::FailedConstraints(values) => {
            out.push(9);
            sequence(out, values, constraint_result);
        }
    }
}
fn state_error(out: &mut Vec<u8>, error: &StateError) {
    match error {
        StateError::CompositionContractMismatch => out.push(0),
        StateError::IncompatibleContext => out.push(1),
        StateError::OutsideTemporalScope => out.push(2),
        StateError::HorizonBeforeInput => out.push(3),
        StateError::HorizonOutsideScope => out.push(4),
        StateError::CriterionSetMismatch => out.push(5),
        StateError::FailedCriteria(values) => {
            out.push(6);
            sequence(out, values, state_result);
        }
    }
}
fn transition_error(out: &mut Vec<u8>, error: &TransitionError) {
    match error {
        TransitionError::SameState => out.push(0),
        TransitionError::SourceContractMismatch => out.push(1),
        TransitionError::TargetContractMismatch => out.push(2),
        TransitionError::IncompatibleContext => out.push(3),
        TransitionError::OutsideTemporalScope => out.push(4),
        TransitionError::HorizonBeforeInput => out.push(5),
        TransitionError::HorizonOutsideScope => out.push(6),
        TransitionError::ConditionSetMismatch => out.push(7),
        TransitionError::FailedConditions(values) => {
            out.push(8);
            sequence(out, values, transition_result);
        }
    }
}
pub(crate) fn put_pipeline_error(out: &mut Vec<u8>, error: &PipelineError) {
    match error {
        PipelineError::SourceUnresolved(reference) => {
            out.push(0);
            put_reference(out, reference);
        }
        PipelineError::SchemaUnresolved(reference) => {
            out.push(1);
            put_reference(out, reference);
        }
        PipelineError::Admission(error) => {
            out.push(2);
            admission_error(out, error);
        }
        PipelineError::OperatorUnresolved(reference) => {
            out.push(3);
            put_reference(out, reference);
        }
        PipelineError::RelationContractUnresolved(reference) => {
            out.push(4);
            put_reference(out, reference);
        }
        PipelineError::CompositionContractUnresolved(reference) => {
            out.push(5);
            put_reference(out, reference);
        }
        PipelineError::ConstraintUnresolved(reference) => {
            out.push(6);
            put_reference(out, reference);
        }
        PipelineError::StateContractUnresolved(reference) => {
            out.push(7);
            put_reference(out, reference);
        }
        PipelineError::StateCriterionUnresolved(reference) => {
            out.push(8);
            put_reference(out, reference);
        }
        PipelineError::TransitionContractUnresolved(reference) => {
            out.push(9);
            put_reference(out, reference);
        }
        PipelineError::TransitionConditionUnresolved(reference) => {
            out.push(10);
            put_reference(out, reference);
        }
        PipelineError::InputUnresolved(reference) => {
            out.push(11);
            put_object_ref(out, reference);
        }
        PipelineError::InvalidInputType(reference) => {
            out.push(12);
            put_object_ref(out, reference);
        }
        PipelineError::Measurement(error) => {
            out.push(13);
            measurement_error(out, error);
        }
        PipelineError::Relation(error) => {
            out.push(14);
            relation_error(out, error);
        }
        PipelineError::Composition(error) => {
            out.push(15);
            composition_error(out, error);
        }
        PipelineError::State(error) => {
            out.push(16);
            state_error(out, error);
        }
        PipelineError::Transition(error) => {
            out.push(17);
            transition_error(out, error);
        }
        PipelineError::IdentityCollision(id) => {
            out.push(18);
            integer(out, id);
        }
        PipelineError::ProvenanceIncomplete(reference) => {
            out.push(19);
            put_object_ref(out, reference);
        }
        PipelineError::InputInvalidated {
            reference,
            revision,
            events,
        } => {
            out.push(20);
            put_object_ref(out, reference);
            integer(out, revision);
            out.extend_from_slice(&(events.len() as u64).to_le_bytes());
            for event in events {
                put_bytes(out, event.canonical_key());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provenance::ObjectRef;
    use alloc::vec;

    fn reference(name: &str) -> VersionedId {
        VersionedId::new(name, 1).unwrap()
    }

    #[test]
    fn unevaluated_measurement_value_is_distinct_from_an_actual_zero() {
        let absent = InvalidBody::Measurement {
            operator: reference("read"),
            definition: None,
            inputs: vec![ObjectRef::Observation(42)],
            value: None,
            unit: None,
            context: None,
        };
        let mut zero = absent.clone();
        if let InvalidBody::Measurement { value, .. } = &mut zero {
            *value = Some(0);
        }
        let mut absent_bytes = Vec::new();
        let mut zero_bytes = Vec::new();
        put_invalid_body(&mut absent_bytes, &absent);
        put_invalid_body(&mut zero_bytes, &zero);
        assert_ne!(absent_bytes, zero_bytes);
    }

    #[test]
    fn failure_reason_and_input_count_order_are_part_of_the_encoding() {
        let variants = [
            PipelineError::InputUnresolved(ObjectRef::Observation(42)),
            PipelineError::InvalidInputType(ObjectRef::Observation(42)),
            PipelineError::Measurement(MeasurementError::WrongInputCount {
                expected: 1,
                actual: 2,
            }),
            PipelineError::Measurement(MeasurementError::WrongInputCount {
                expected: 2,
                actual: 1,
            }),
        ];
        let encoded: Vec<_> = variants
            .iter()
            .map(|error| {
                let mut bytes = Vec::new();
                put_pipeline_error(&mut bytes, error);
                bytes
            })
            .collect();
        for left in 0..encoded.len() {
            for right in left + 1..encoded.len() {
                assert_ne!(encoded[left], encoded[right]);
            }
        }
    }
}
