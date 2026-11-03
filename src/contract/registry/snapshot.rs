//! Frozen Registry section for the recorded snapshot v1 format.
//!
//! Fourteen count-prefixed groups follow dependency order. Definitions in each
//! group follow VersionedId order; registration may normalize semantic sets, so
//! decoding verifies an exact re-encoding of the consumed section.
use super::*;
use crate::contract::conformance::{
    ConformanceContract, ConformanceCriterionContract, MissingTargetPolicy,
};
use crate::contract::dynamics::{StateDefinition, TransitionConditionContract};
use crate::contract::lifecycle::{InvalidationPropagation, InvalidationReason};
use crate::contract::structure::MemberType;
use crate::pipeline::snapshot::wire::{Reader, Writer};
use crate::pipeline::snapshot::SnapshotError;
use alloc::vec::Vec;

fn group<T>(
    writer: &mut Writer,
    values: &BTreeMap<VersionedId, T>,
    mut encode: impl FnMut(&mut Writer, &T) -> Result<(), SnapshotError>,
) -> Result<(), SnapshotError> {
    writer.definitions(values.len())?;
    for value in values.values() {
        encode(writer, value)?;
    }
    Ok(())
}

fn references(writer: &mut Writer, values: &[VersionedId]) -> Result<(), SnapshotError> {
    writer.len(values.len())?;
    for value in values {
        writer.reference(value)?;
    }
    Ok(())
}

fn read_references(reader: &mut Reader<'_>) -> Result<Vec<VersionedId>, SnapshotError> {
    let count = reader.len()?;
    let mut values = Vec::new();
    for _ in 0..count {
        values.push(reader.reference()?);
    }
    Ok(values)
}

fn order(writer: &mut Writer, value: CriterionOrder) -> Result<(), SnapshotError> {
    writer.u8(match value {
        CriterionOrder::Ordered => 0,
        CriterionOrder::Unordered => 1,
    })
}

fn read_order(reader: &mut Reader<'_>) -> Result<CriterionOrder, SnapshotError> {
    match reader.u8()? {
        0 => Ok(CriterionOrder::Ordered),
        1 => Ok(CriterionOrder::Unordered),
        tag => Err(SnapshotError::InvalidTag {
            kind: "criterion order",
            tag,
        }),
    }
}

fn target(writer: &mut Writer, value: TargetType) -> Result<(), SnapshotError> {
    writer.u8(match value {
        TargetType::Observation => 0,
        TargetType::Measurement => 1,
        TargetType::Relation => 2,
        TargetType::Composition => 3,
        TargetType::State => 4,
        TargetType::Transition => 5,
    })
}

fn read_target(reader: &mut Reader<'_>) -> Result<TargetType, SnapshotError> {
    match reader.u8()? {
        0 => Ok(TargetType::Observation),
        1 => Ok(TargetType::Measurement),
        2 => Ok(TargetType::Relation),
        3 => Ok(TargetType::Composition),
        4 => Ok(TargetType::State),
        5 => Ok(TargetType::Transition),
        tag => Err(SnapshotError::InvalidTag {
            kind: "conformance target",
            tag,
        }),
    }
}

fn schema(writer: &mut Writer, value: &SchemaContract) -> Result<(), SnapshotError> {
    writer.reference(&value.reference)?;
    writer.u8(match value.format {
        ScalarSchema::SignedI64Le => 0,
    })
}

fn read_schema(reader: &mut Reader<'_>) -> Result<SchemaContract, SnapshotError> {
    let reference = reader.reference()?;
    let format = match reader.u8()? {
        0 => ScalarSchema::SignedI64Le,
        tag => {
            return Err(SnapshotError::InvalidTag {
                kind: "schema",
                tag,
            })
        }
    };
    Ok(SchemaContract { reference, format })
}

fn source(writer: &mut Writer, value: &SourceContract) -> Result<(), SnapshotError> {
    writer.reference(&value.reference)?;
    writer.reference(&value.schema)?;
    writer.domain(&value.context)?;
    writer.string(&value.unit)
}

fn read_source(reader: &mut Reader<'_>) -> Result<SourceContract, SnapshotError> {
    Ok(SourceContract {
        reference: reader.reference()?,
        schema: reader.reference()?,
        context: reader.domain()?,
        unit: reader.string()?,
    })
}

fn operator(writer: &mut Writer, value: &OperatorContract) -> Result<(), SnapshotError> {
    writer.reference(&value.reference)?;
    writer.u8(match value.kind {
        OperatorKind::Scalar => 0,
        OperatorKind::Difference => 1,
    })
}

fn read_operator(reader: &mut Reader<'_>) -> Result<OperatorContract, SnapshotError> {
    let reference = reader.reference()?;
    let kind = match reader.u8()? {
        0 => OperatorKind::Scalar,
        1 => OperatorKind::Difference,
        tag => {
            return Err(SnapshotError::InvalidTag {
                kind: "operator",
                tag,
            })
        }
    };
    Ok(OperatorContract { reference, kind })
}

fn relation(writer: &mut Writer, value: &RelationContract) -> Result<(), SnapshotError> {
    writer.reference(&value.reference)?;
    match value.kind {
        RelationKind::Precedes => writer.u8(0)?,
        RelationKind::TemporalDistance => writer.u8(1)?,
        RelationKind::Adjacent { step, symmetric } => {
            writer.u8(2)?;
            writer.u64(step)?;
            writer.boolean(symmetric)?;
        }
        RelationKind::Proportion => writer.u8(3)?,
    }
    writer.relation_context(&value.context)
}

fn read_relation(reader: &mut Reader<'_>) -> Result<RelationContract, SnapshotError> {
    let reference = reader.reference()?;
    let kind = match reader.u8()? {
        0 => RelationKind::Precedes,
        1 => RelationKind::TemporalDistance,
        2 => RelationKind::Adjacent {
            step: reader.u64()?,
            symmetric: reader.boolean()?,
        },
        3 => RelationKind::Proportion,
        tag => {
            return Err(SnapshotError::InvalidTag {
                kind: "relation",
                tag,
            })
        }
    };
    Ok(RelationContract {
        reference,
        kind,
        context: reader.relation_context()?,
    })
}

fn constraint(writer: &mut Writer, value: &ConstraintContract) -> Result<(), SnapshotError> {
    writer.reference(&value.reference)?;
    match &value.kind {
        ConstraintKind::Cardinality { min, max } => {
            writer.u8(0)?;
            writer.u32(*min)?;
            writer.u32(*max)?;
        }
        ConstraintKind::SameUnits => writer.u8(1)?,
        ConstraintKind::Connected => writer.u8(2)?,
        ConstraintKind::Acyclic => writer.u8(3)?,
        ConstraintKind::OrderedChain { relation } => {
            writer.u8(4)?;
            writer.reference(relation)?;
        }
        ConstraintKind::RequiresRelation { relation } => {
            writer.u8(5)?;
            writer.reference(relation)?;
        }
    }
    Ok(())
}

fn read_constraint(reader: &mut Reader<'_>) -> Result<ConstraintContract, SnapshotError> {
    let reference = reader.reference()?;
    let kind = match reader.u8()? {
        0 => ConstraintKind::Cardinality {
            min: reader.u32()?,
            max: reader.u32()?,
        },
        1 => ConstraintKind::SameUnits,
        2 => ConstraintKind::Connected,
        3 => ConstraintKind::Acyclic,
        4 => ConstraintKind::OrderedChain {
            relation: reader.reference()?,
        },
        5 => ConstraintKind::RequiresRelation {
            relation: reader.reference()?,
        },
        tag => {
            return Err(SnapshotError::InvalidTag {
                kind: "constraint",
                tag,
            })
        }
    };
    Ok(ConstraintContract { reference, kind })
}

fn composition(writer: &mut Writer, value: &CompositionContract) -> Result<(), SnapshotError> {
    writer.reference(&value.reference)?;
    writer.u8(match value.member_order {
        MemberOrder::Ordered => 0,
        MemberOrder::Unordered => 1,
    })?;
    writer.u8(match value.member_type {
        MemberType::Observations => 0,
        MemberType::Measurements => 1,
        MemberType::ScalarInputs => 2,
    })?;
    references(writer, &value.constraints)?;
    writer.relation_context(&value.context)
}

fn read_composition(reader: &mut Reader<'_>) -> Result<CompositionContract, SnapshotError> {
    let reference = reader.reference()?;
    let member_order = match reader.u8()? {
        0 => MemberOrder::Ordered,
        1 => MemberOrder::Unordered,
        tag => {
            return Err(SnapshotError::InvalidTag {
                kind: "member order",
                tag,
            })
        }
    };
    let member_type = match reader.u8()? {
        0 => MemberType::Observations,
        1 => MemberType::Measurements,
        2 => MemberType::ScalarInputs,
        tag => {
            return Err(SnapshotError::InvalidTag {
                kind: "member type",
                tag,
            })
        }
    };
    Ok(CompositionContract {
        reference,
        member_order,
        member_type,
        constraints: read_references(reader)?,
        context: reader.relation_context()?,
    })
}

fn state_criterion(
    writer: &mut Writer,
    value: &StateCriterionContract,
) -> Result<(), SnapshotError> {
    writer.reference(&value.reference)?;
    match &value.kind {
        StateCriterionKind::Cardinality { min, max } => {
            writer.u8(0)?;
            writer.u32(*min)?;
            writer.u32(*max)?;
        }
        StateCriterionKind::HasRelation { relation } => {
            writer.u8(1)?;
            writer.reference(relation)?;
        }
        StateCriterionKind::ConstraintPassed { constraint } => {
            writer.u8(2)?;
            writer.reference(constraint)?;
        }
        StateCriterionKind::MaxTemporalSpan { ticks } => {
            writer.u8(3)?;
            writer.u64(*ticks)?;
        }
        StateCriterionKind::ObservationAfter { source } => {
            writer.u8(4)?;
            writer.reference(source)?;
        }
    }
    Ok(())
}

fn read_state_criterion(reader: &mut Reader<'_>) -> Result<StateCriterionContract, SnapshotError> {
    let reference = reader.reference()?;
    let kind = match reader.u8()? {
        0 => StateCriterionKind::Cardinality {
            min: reader.u32()?,
            max: reader.u32()?,
        },
        1 => StateCriterionKind::HasRelation {
            relation: reader.reference()?,
        },
        2 => StateCriterionKind::ConstraintPassed {
            constraint: reader.reference()?,
        },
        3 => StateCriterionKind::MaxTemporalSpan {
            ticks: reader.u64()?,
        },
        4 => StateCriterionKind::ObservationAfter {
            source: reader.reference()?,
        },
        tag => {
            return Err(SnapshotError::InvalidTag {
                kind: "state criterion",
                tag,
            })
        }
    };
    Ok(StateCriterionContract { reference, kind })
}

fn state(writer: &mut Writer, value: &StateContract) -> Result<(), SnapshotError> {
    writer.reference(&value.reference)?;
    writer.reference(&value.composition)?;
    writer.u8(match value.definition {
        StateDefinition::CompositionAbstraction => 0,
    })?;
    order(writer, value.criterion_order)?;
    references(writer, &value.criteria)?;
    writer.dynamics(&value.context)
}

fn read_state(reader: &mut Reader<'_>) -> Result<StateContract, SnapshotError> {
    let reference = reader.reference()?;
    let composition = reader.reference()?;
    let definition = match reader.u8()? {
        0 => StateDefinition::CompositionAbstraction,
        tag => {
            return Err(SnapshotError::InvalidTag {
                kind: "state definition",
                tag,
            })
        }
    };
    Ok(StateContract {
        reference,
        composition,
        definition,
        criterion_order: read_order(reader)?,
        criteria: read_references(reader)?,
        context: reader.dynamics()?,
    })
}

fn transition_condition(
    writer: &mut Writer,
    value: &TransitionConditionContract,
) -> Result<(), SnapshotError> {
    writer.reference(&value.reference)?;
    match &value.kind {
        TransitionConditionKind::TemporalOrder { strict } => {
            writer.u8(0)?;
            writer.boolean(*strict)?;
        }
        TransitionConditionKind::SourceCriteriaPass => writer.u8(1)?,
        TransitionConditionKind::TargetCriteriaPass => writer.u8(2)?,
        TransitionConditionKind::ObservationAfterTarget { source } => {
            writer.u8(3)?;
            writer.reference(source)?;
        }
    }
    Ok(())
}

fn read_transition_condition(
    reader: &mut Reader<'_>,
) -> Result<TransitionConditionContract, SnapshotError> {
    let reference = reader.reference()?;
    let kind = match reader.u8()? {
        0 => TransitionConditionKind::TemporalOrder {
            strict: reader.boolean()?,
        },
        1 => TransitionConditionKind::SourceCriteriaPass,
        2 => TransitionConditionKind::TargetCriteriaPass,
        3 => TransitionConditionKind::ObservationAfterTarget {
            source: reader.reference()?,
        },
        tag => {
            return Err(SnapshotError::InvalidTag {
                kind: "transition condition",
                tag,
            })
        }
    };
    Ok(TransitionConditionContract { reference, kind })
}

fn transition(writer: &mut Writer, value: &TransitionContract) -> Result<(), SnapshotError> {
    writer.reference(&value.reference)?;
    writer.reference(&value.source_state)?;
    writer.reference(&value.target_state)?;
    order(writer, value.condition_order)?;
    references(writer, &value.conditions)?;
    writer.dynamics(&value.context)
}

fn read_transition(reader: &mut Reader<'_>) -> Result<TransitionContract, SnapshotError> {
    Ok(TransitionContract {
        reference: reader.reference()?,
        source_state: reader.reference()?,
        target_state: reader.reference()?,
        condition_order: read_order(reader)?,
        conditions: read_references(reader)?,
        context: reader.dynamics()?,
    })
}

fn conformance_criterion(
    writer: &mut Writer,
    value: &ConformanceCriterionContract,
) -> Result<(), SnapshotError> {
    writer.reference(&value.reference)?;
    match value.kind {
        ConformanceCriterionKind::TargetExists => writer.u8(0)?,
        ConformanceCriterionKind::LineageComplete => writer.u8(1)?,
        ConformanceCriterionKind::IdentityCoherent => writer.u8(2)?,
        ConformanceCriterionKind::ConstructionPass => writer.u8(3)?,
        ConformanceCriterionKind::MemberCount { min, max } => {
            writer.u8(4)?;
            writer.u32(min)?;
            writer.u32(max)?;
        }
    }
    Ok(())
}

fn read_conformance_criterion(
    reader: &mut Reader<'_>,
) -> Result<ConformanceCriterionContract, SnapshotError> {
    let reference = reader.reference()?;
    let kind = match reader.u8()? {
        0 => ConformanceCriterionKind::TargetExists,
        1 => ConformanceCriterionKind::LineageComplete,
        2 => ConformanceCriterionKind::IdentityCoherent,
        3 => ConformanceCriterionKind::ConstructionPass,
        4 => ConformanceCriterionKind::MemberCount {
            min: reader.u32()?,
            max: reader.u32()?,
        },
        tag => {
            return Err(SnapshotError::InvalidTag {
                kind: "conformance criterion",
                tag,
            })
        }
    };
    Ok(ConformanceCriterionContract { reference, kind })
}

fn conformance(writer: &mut Writer, value: &ConformanceContract) -> Result<(), SnapshotError> {
    writer.reference(&value.reference)?;
    target(writer, value.target_type)?;
    order(writer, value.criterion_order)?;
    references(writer, &value.criteria)?;
    writer.dynamics(&value.context)?;
    writer.u8(match value.missing_target {
        MissingTargetPolicy::Fail => 0,
        MissingTargetPolicy::Unresolved => 1,
    })
}

fn read_conformance(reader: &mut Reader<'_>) -> Result<ConformanceContract, SnapshotError> {
    let reference = reader.reference()?;
    let target_type = read_target(reader)?;
    let criterion_order = read_order(reader)?;
    let criteria = read_references(reader)?;
    let context = reader.dynamics()?;
    let missing_target = match reader.u8()? {
        0 => MissingTargetPolicy::Fail,
        1 => MissingTargetPolicy::Unresolved,
        tag => {
            return Err(SnapshotError::InvalidTag {
                kind: "missing target policy",
                tag,
            })
        }
    };
    Ok(ConformanceContract {
        reference,
        target_type,
        criterion_order,
        criteria,
        context,
        missing_target,
    })
}

fn projection(writer: &mut Writer, value: &ProjectionProfile) -> Result<(), SnapshotError> {
    writer.reference(&value.reference)?;
    writer.reference(&value.schema)?;
    target(writer, value.target_type)?;
    writer.reference(&value.conformance)?;
    writer.dynamics(&value.context)
}

fn read_projection(reader: &mut Reader<'_>) -> Result<ProjectionProfile, SnapshotError> {
    Ok(ProjectionProfile {
        reference: reader.reference()?,
        schema: reader.reference()?,
        target_type: read_target(reader)?,
        conformance: reader.reference()?,
        context: reader.dynamics()?,
    })
}

fn lifecycle(writer: &mut Writer, value: &LifecycleContract) -> Result<(), SnapshotError> {
    writer.reference(&value.reference)?;
    target(writer, value.target_type)?;
    writer.dynamics(&value.context)?;
    writer.len(value.allowed_reasons.len())?;
    for reason in &value.allowed_reasons {
        writer.u8(match reason {
            InvalidationReason::SourceContractViolation => 0,
            InvalidationReason::ContradictoryTemporalIdentity => 1,
            InvalidationReason::DerivationContractViolation => 2,
            InvalidationReason::EvidenceWithdrawn => 3,
        })?;
    }
    writer.u8(match value.propagation {
        InvalidationPropagation::RequiredDeclaredInputs => 0,
    })
}

fn read_lifecycle(reader: &mut Reader<'_>) -> Result<LifecycleContract, SnapshotError> {
    let reference = reader.reference()?;
    let target_type = read_target(reader)?;
    let context = reader.dynamics()?;
    let count = reader.len()?;
    let mut allowed_reasons = Vec::new();
    for _ in 0..count {
        allowed_reasons.push(match reader.u8()? {
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
        });
    }
    let propagation = match reader.u8()? {
        0 => InvalidationPropagation::RequiredDeclaredInputs,
        tag => {
            return Err(SnapshotError::InvalidTag {
                kind: "invalidation propagation",
                tag,
            })
        }
    };
    Ok(LifecycleContract {
        reference,
        target_type,
        context,
        allowed_reasons,
        propagation,
    })
}

pub(crate) fn encode_registry(value: &Registry, writer: &mut Writer) -> Result<(), SnapshotError> {
    group(writer, &value.schemas, schema)?;
    group(writer, &value.sources, source)?;
    group(writer, &value.operators, operator)?;
    group(writer, &value.relations, relation)?;
    group(writer, &value.constraints, constraint)?;
    group(writer, &value.compositions, composition)?;
    group(writer, &value.state_criteria, state_criterion)?;
    group(writer, &value.states, state)?;
    group(writer, &value.transition_conditions, transition_condition)?;
    group(writer, &value.transitions, transition)?;
    group(writer, &value.conformance_criteria, conformance_criterion)?;
    group(writer, &value.conformance, conformance)?;
    group(writer, &value.projections, projection)?;
    group(writer, &value.lifecycle, lifecycle)
}

pub(crate) fn decode_registry(reader: &mut Reader<'_>) -> Result<Registry, SnapshotError> {
    let start = reader.position();
    let mut registry = Registry::new();
    macro_rules! decode_group {
        ($read:ident, $register:ident) => {
            for _ in 0..reader.definitions()? {
                registry
                    .$register($read(reader)?)
                    .map_err(SnapshotError::InvalidDefinition)?;
            }
        };
    }
    decode_group!(read_schema, register_schema);
    decode_group!(read_source, register_source);
    decode_group!(read_operator, register_operator);
    decode_group!(read_relation, register_relation);
    decode_group!(read_constraint, register_constraint);
    decode_group!(read_composition, register_composition);
    decode_group!(read_state_criterion, register_state_criterion);
    decode_group!(read_state, register_state);
    decode_group!(read_transition_condition, register_transition_condition);
    decode_group!(read_transition, register_transition);
    decode_group!(read_conformance_criterion, register_conformance_criterion);
    decode_group!(read_conformance, register_conformance);
    decode_group!(read_projection, register_projection);
    decode_group!(read_lifecycle, register_lifecycle);
    let end = reader.position();
    let mut writer = Writer::new(reader.limits());
    encode_registry(&registry, &mut writer)?;
    if writer.finish() != reader.slice(start, end) {
        return Err(SnapshotError::NonCanonicalRegistry);
    }
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::dynamics::DynamicsContext;
    use crate::contract::structure::{RelationContext, TemporalScope};
    use crate::pipeline::snapshot::SnapshotLimits;
    use alloc::vec;

    fn limits() -> SnapshotLimits {
        SnapshotLimits {
            max_snapshot_bytes: 1024 * 1024,
            max_field_bytes: 8192,
            max_collection_items: 128,
            max_total_items: 4096,
            max_definitions: 256,
            max_operations: 16,
            max_manifest_bytes: 64 * 1024,
        }
    }

    fn id(name: &str) -> VersionedId {
        VersionedId::new(name, 1).unwrap()
    }

    fn frame() -> RelationContext {
        RelationContext::new(
            DomainContext::new("registry/snapshot", id("ticks")).unwrap(),
            TemporalScope::new(0, 100).unwrap(),
            id("frame"),
            id("causal"),
        )
    }

    fn context() -> DynamicsContext {
        DynamicsContext::new(frame(), id("calculus"), id("domain"))
    }

    fn fixture() -> Registry {
        let mut registry = Registry::new();
        for version in [2, 1] {
            registry
                .register_schema(SchemaContract {
                    reference: VersionedId::new("schema", version).unwrap(),
                    format: ScalarSchema::SignedI64Le,
                })
                .unwrap();
        }
        registry
            .register_source(SourceContract {
                reference: id("source"),
                schema: id("schema"),
                context: frame().domain().clone(),
                unit: "count".into(),
            })
            .unwrap();
        for kind in [OperatorKind::Difference, OperatorKind::Scalar] {
            registry
                .register_operator(OperatorContract {
                    reference: VersionedId::new(
                        "unused/operator",
                        if kind == OperatorKind::Scalar { 1 } else { 2 },
                    )
                    .unwrap(),
                    kind,
                })
                .unwrap();
        }
        registry
            .register_relation(RelationContract {
                reference: id("relation"),
                kind: RelationKind::Precedes,
                context: frame(),
            })
            .unwrap();
        for (name, kind) in [
            (
                "constraint/count",
                ConstraintKind::Cardinality { min: 1, max: 8 },
            ),
            ("constraint/units", ConstraintKind::SameUnits),
        ] {
            registry
                .register_constraint(ConstraintContract {
                    reference: id(name),
                    kind,
                })
                .unwrap();
        }
        registry
            .register_composition(CompositionContract {
                reference: id("composition"),
                member_order: MemberOrder::Unordered,
                member_type: MemberType::ScalarInputs,
                constraints: vec![id("constraint/units"), id("constraint/count")],
                context: frame(),
            })
            .unwrap();
        for (name, kind) in [
            (
                "criterion/count",
                StateCriterionKind::Cardinality { min: 1, max: 8 },
            ),
            (
                "criterion/span",
                StateCriterionKind::MaxTemporalSpan { ticks: 20 },
            ),
        ] {
            registry
                .register_state_criterion(StateCriterionContract {
                    reference: id(name),
                    kind,
                })
                .unwrap();
        }
        registry
            .register_state(StateContract {
                reference: id("state"),
                composition: id("composition"),
                definition: StateDefinition::CompositionAbstraction,
                criterion_order: CriterionOrder::Unordered,
                criteria: vec![id("criterion/span"), id("criterion/count")],
                context: context(),
            })
            .unwrap();
        for (name, kind) in [
            (
                "condition/time",
                TransitionConditionKind::TemporalOrder { strict: true },
            ),
            (
                "condition/source",
                TransitionConditionKind::SourceCriteriaPass,
            ),
            (
                "condition/target",
                TransitionConditionKind::TargetCriteriaPass,
            ),
        ] {
            registry
                .register_transition_condition(TransitionConditionContract {
                    reference: id(name),
                    kind,
                })
                .unwrap();
        }
        registry
            .register_transition(TransitionContract {
                reference: id("transition"),
                source_state: id("state"),
                target_state: id("state"),
                condition_order: CriterionOrder::Unordered,
                conditions: vec![
                    id("condition/time"),
                    id("condition/target"),
                    id("condition/source"),
                ],
                context: context(),
            })
            .unwrap();
        for (name, kind) in [
            ("check/target", ConformanceCriterionKind::TargetExists),
            ("check/lineage", ConformanceCriterionKind::LineageComplete),
            (
                "check/count",
                ConformanceCriterionKind::MemberCount { min: 0, max: 8 },
            ),
        ] {
            registry
                .register_conformance_criterion(ConformanceCriterionContract {
                    reference: id(name),
                    kind,
                })
                .unwrap();
        }
        registry
            .register_conformance(ConformanceContract {
                reference: id("conformance"),
                target_type: TargetType::Composition,
                criteria: vec![id("check/target"), id("check/lineage"), id("check/count")],
                criterion_order: CriterionOrder::Unordered,
                context: context(),
                missing_target: MissingTargetPolicy::Unresolved,
            })
            .unwrap();
        registry
            .register_projection(ProjectionProfile {
                reference: id("projection"),
                schema: id("TEST-REP-01"),
                target_type: TargetType::Composition,
                conformance: id("conformance"),
                context: context(),
            })
            .unwrap();
        registry
            .register_lifecycle(LifecycleContract {
                reference: id("lifecycle"),
                target_type: TargetType::Composition,
                context: context(),
                allowed_reasons: vec![
                    InvalidationReason::EvidenceWithdrawn,
                    InvalidationReason::SourceContractViolation,
                ],
                propagation: InvalidationPropagation::RequiredDeclaredInputs,
            })
            .unwrap();
        registry
    }

    fn encoded(registry: &Registry) -> Vec<u8> {
        let mut writer = Writer::new(limits());
        encode_registry(registry, &mut writer).unwrap();
        writer.finish()
    }

    #[test]
    fn all_registry_groups_and_unused_exact_versions_roundtrip() {
        for original in [Registry::new(), fixture()] {
            let bytes = encoded(&original);
            let mut reader = Reader::new(&bytes, limits()).unwrap();
            let restored = decode_registry(&mut reader).unwrap();
            reader.finish().unwrap();
            assert_eq!(encoded(&restored), bytes);
            assert_eq!(restored.schemas.len(), original.schemas.len());
            assert_eq!(restored.sources.len(), original.sources.len());
            assert_eq!(restored.operators.len(), original.operators.len());
            assert_eq!(restored.relations.len(), original.relations.len());
            assert_eq!(restored.constraints.len(), original.constraints.len());
            assert_eq!(restored.compositions.len(), original.compositions.len());
            assert_eq!(restored.state_criteria.len(), original.state_criteria.len());
            assert_eq!(restored.states.len(), original.states.len());
            assert_eq!(
                restored.transition_conditions.len(),
                original.transition_conditions.len()
            );
            assert_eq!(restored.transitions.len(), original.transitions.len());
            assert_eq!(
                restored.conformance_criteria.len(),
                original.conformance_criteria.len()
            );
            assert_eq!(restored.conformance.len(), original.conformance.len());
            assert_eq!(restored.projections.len(), original.projections.len());
            assert_eq!(restored.lifecycle.len(), original.lifecycle.len());
        }
        let bytes = encoded(&fixture());
        let restored = decode_registry(&mut Reader::new(&bytes, limits()).unwrap()).unwrap();
        assert!(restored
            .operator(&VersionedId::new("unused/operator", 2).unwrap())
            .is_some());
        assert!(restored
            .schema(&VersionedId::new("schema", 2).unwrap())
            .is_some());
        assert!(restored
            .schema(&VersionedId::new("schema", 3).unwrap())
            .is_none());
    }

    #[test]
    fn noncanonical_group_order_and_duplicate_definitions_are_rejected() {
        for duplicate in [false, true] {
            let mut writer = Writer::new(limits());
            writer.definitions(2).unwrap();
            for version in [2, if duplicate { 2 } else { 1 }] {
                schema(
                    &mut writer,
                    &SchemaContract {
                        reference: VersionedId::new("schema", version).unwrap(),
                        format: ScalarSchema::SignedI64Le,
                    },
                )
                .unwrap();
            }
            for _ in 0..13 {
                writer.definitions(0).unwrap();
            }
            let bytes = writer.finish();
            let error = decode_registry(&mut Reader::new(&bytes, limits()).unwrap())
                .err()
                .unwrap();
            if duplicate {
                assert!(matches!(
                    error,
                    SnapshotError::InvalidDefinition(RegistryError::DuplicateReference(_))
                ));
            } else {
                assert_eq!(error, SnapshotError::NonCanonicalRegistry);
            }
        }
    }

    #[test]
    fn unordered_definition_lists_must_already_have_canonical_order() {
        for selected in 0..5 {
            let mut registry = fixture();
            match selected {
                0 => registry
                    .compositions
                    .get_mut(&id("composition"))
                    .unwrap()
                    .constraints
                    .reverse(),
                1 => registry
                    .states
                    .get_mut(&id("state"))
                    .unwrap()
                    .criteria
                    .reverse(),
                2 => registry
                    .transitions
                    .get_mut(&id("transition"))
                    .unwrap()
                    .conditions
                    .reverse(),
                3 => registry
                    .conformance
                    .get_mut(&id("conformance"))
                    .unwrap()
                    .criteria
                    .reverse(),
                4 => registry
                    .lifecycle
                    .get_mut(&id("lifecycle"))
                    .unwrap()
                    .allowed_reasons
                    .reverse(),
                _ => unreachable!(),
            }
            let bytes = encoded(&registry);
            assert!(matches!(
                decode_registry(&mut Reader::new(&bytes, limits()).unwrap()),
                Err(SnapshotError::NonCanonicalRegistry)
            ));
        }
    }

    #[test]
    fn unknown_tags_missing_dependencies_and_invalid_definitions_are_rejected() {
        let mut writer = Writer::new(limits());
        writer.definitions(1).unwrap();
        writer.reference(&id("schema")).unwrap();
        writer.u8(255).unwrap();
        let bytes = writer.finish();
        assert!(matches!(
            decode_registry(&mut Reader::new(&bytes, limits()).unwrap()),
            Err(SnapshotError::InvalidTag {
                kind: "schema",
                tag: 255
            })
        ));

        let mut writer = Writer::new(limits());
        writer.definitions(0).unwrap();
        writer.definitions(1).unwrap();
        source(
            &mut writer,
            &SourceContract {
                reference: id("source"),
                schema: id("missing"),
                context: frame().domain().clone(),
                unit: "count".into(),
            },
        )
        .unwrap();
        let bytes = writer.finish();
        assert!(matches!(
            decode_registry(&mut Reader::new(&bytes, limits()).unwrap()),
            Err(SnapshotError::InvalidDefinition(
                RegistryError::SchemaUnresolved(_)
            ))
        ));

        let mut writer = Writer::new(limits());
        for _ in 0..3 {
            writer.definitions(0).unwrap();
        }
        writer.definitions(1).unwrap();
        relation(
            &mut writer,
            &RelationContract {
                reference: id("bad"),
                kind: RelationKind::Adjacent {
                    step: 0,
                    symmetric: false,
                },
                context: frame(),
            },
        )
        .unwrap();
        let bytes = writer.finish();
        assert!(matches!(
            decode_registry(&mut Reader::new(&bytes, limits()).unwrap()),
            Err(SnapshotError::InvalidDefinition(
                RegistryError::InvalidAdjacencyStep
            ))
        ));
    }

    #[test]
    fn registry_lengths_and_aggregate_definition_budget_are_bounded() {
        let bytes = u64::MAX.to_le_bytes();
        assert!(matches!(
            decode_registry(&mut Reader::new(&bytes, limits()).unwrap()),
            Err(SnapshotError::LimitExceeded(_))
        ));
        let mut restricted = limits();
        restricted.max_definitions = 1;
        let bytes = encoded(&fixture());
        assert!(matches!(
            decode_registry(&mut Reader::new(&bytes, restricted).unwrap()),
            Err(SnapshotError::LimitExceeded("max_definitions"))
        ));
        let mut writer = Writer::new(restricted);
        assert!(matches!(
            encode_registry(&fixture(), &mut writer),
            Err(SnapshotError::LimitExceeded("max_definitions"))
        ));
    }
}
