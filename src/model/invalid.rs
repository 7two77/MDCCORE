//! Typed partial semantic objects retained after failed construction.
//!
//! These objects record what was actually known at failure time. A missing
//! computed value remains `None`; retention never manufactures a usable input.

use alloc::string::String;
use alloc::vec::Vec;

use crate::contract::dynamics::{StateContract, TransitionContract};
use crate::contract::encoding::{put_bytes, put_object_ref, put_reference};
use crate::contract::evaluation::LifecycleStatus;
use crate::contract::invalid_encoding::{put_invalid_body, put_pipeline_error};
use crate::contract::registry::{
    DomainContext, OperatorContract, SchemaContract, SourceContract, VersionedId,
};
use crate::contract::structure::{CompositionContract, RelationContract, TemporalScope};
use crate::identity::CanonicalSemanticBasis;
use crate::model::composition::ConstraintResult;
use crate::model::relation::RelationValue;
use crate::model::state::StateCriterionResult;
use crate::model::transition::TransitionConditionResult;
use crate::pipeline::PipelineError;
use crate::provenance::{InvalidId, ObjectRef};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum InvalidKind {
    Observation,
    Measurement,
    Relation,
    Composition,
    State,
    Transition,
}

/// Available semantic fields and definitions, rather than an audit message.
/// Missing definitions and unevaluated results are represented explicitly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvalidBody {
    Observation {
        source: VersionedId,
        definition: Option<SourceContract>,
        schema: Option<SchemaContract>,
        timestamp: Option<u64>,
        payload: Vec<u8>,
        acquisition_context: String,
    },
    Measurement {
        operator: VersionedId,
        definition: Option<OperatorContract>,
        inputs: Vec<ObjectRef>,
        value: Option<i128>,
        unit: Option<String>,
        context: Option<DomainContext>,
    },
    Relation {
        contract: VersionedId,
        definition: Option<RelationContract>,
        left: ObjectRef,
        right: ObjectRef,
        value: Option<RelationValue>,
    },
    Composition {
        contract: VersionedId,
        definition: Option<CompositionContract>,
        members: Vec<ObjectRef>,
        relations: Vec<ObjectRef>,
        constraints: Option<Vec<ConstraintResult>>,
    },
    State {
        contract: VersionedId,
        definition: Option<StateContract>,
        source_composition: ObjectRef,
        observation_horizon: Option<u64>,
        interval: Option<TemporalScope>,
        criteria: Option<Vec<StateCriterionResult>>,
    },
    Transition {
        contract: VersionedId,
        definition: Option<TransitionContract>,
        source_state: ObjectRef,
        target_state: ObjectRef,
        observation_horizon: Option<u64>,
        conditions: Option<Vec<TransitionConditionResult>>,
    },
}

impl InvalidBody {
    pub fn kind(&self) -> InvalidKind {
        match self {
            Self::Observation { .. } => InvalidKind::Observation,
            Self::Measurement { .. } => InvalidKind::Measurement,
            Self::Relation { .. } => InvalidKind::Relation,
            Self::Composition { .. } => InvalidKind::Composition,
            Self::State { .. } => InvalidKind::State,
            Self::Transition { .. } => InvalidKind::Transition,
        }
    }

    /// The requested source, operator, or construction contract reference.
    /// Its presence here does not claim that its definition resolved.
    pub fn contract(&self) -> &VersionedId {
        match self {
            Self::Observation { source, .. } => source,
            Self::Measurement { operator, .. } => operator,
            Self::Relation { contract, .. }
            | Self::Composition { contract, .. }
            | Self::State { contract, .. }
            | Self::Transition { contract, .. } => contract,
        }
    }
}

/// A resolved reference and its immutable creation-time semantic snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedInput {
    reference: ObjectRef,
    basis: Vec<u8>,
}

impl CapturedInput {
    pub(crate) fn new(reference: ObjectRef, basis: Vec<u8>) -> Self {
        Self { reference, basis }
    }

    pub fn reference(&self) -> &ObjectRef {
        &self.reference
    }

    pub fn basis(&self) -> &[u8] {
        &self.basis
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DefinitionKind {
    Source,
    Schema,
    Operator,
    Relation,
    Composition,
    State,
    Transition,
    Constraint,
    StateCriterion,
    TransitionCondition,
    Conformance,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct UnresolvedDefinition {
    pub kind: DefinitionKind,
    pub reference: VersionedId,
}

pub(crate) struct InvalidArtifactData {
    pub body: InvalidBody,
    pub error: PipelineError,
    pub captured_inputs: Vec<CapturedInput>,
    pub unresolved: Vec<ObjectRef>,
    pub unresolved_definitions: Vec<UnresolvedDefinition>,
}

/// An immutable invalid semantic result with a collision-disambiguated identity.
/// It is retained outside the admitted graph and cannot be used as a valid input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidSemanticObject {
    id: InvalidId,
    body: InvalidBody,
    error: PipelineError,
    captured_inputs: Vec<CapturedInput>,
    unresolved: Vec<ObjectRef>,
    unresolved_definitions: Vec<UnresolvedDefinition>,
}

impl InvalidSemanticObject {
    pub(crate) fn new(mut data: InvalidArtifactData) -> Self {
        // Invalid objects follow the resolved rule's semantics too. The audit
        // receipt separately retains the exact submitted request. Sorting must
        // preserve duplicates: their multiplicity can be the failure itself.
        match &mut data.body {
            InvalidBody::Composition {
                definition: Some(definition),
                members,
                relations,
                ..
            } => {
                if definition.member_order == crate::contract::structure::MemberOrder::Unordered {
                    members.sort();
                }
                relations.sort();
            }
            InvalidBody::Relation {
                definition: Some(definition),
                left,
                right,
                ..
            } => {
                if definition.kind.is_symmetric() && right < left {
                    core::mem::swap(left, right);
                }
            }
            _ => {}
        }
        data.captured_inputs.sort_by(|left, right| {
            left.reference
                .cmp(&right.reference)
                .then_with(|| left.basis.cmp(&right.basis))
        });
        // Preserve conflicting snapshots if supplied: only identical captures
        // may collapse. Silently selecting one would lose failure evidence.
        data.captured_inputs.dedup();
        data.unresolved.sort();
        data.unresolved.dedup();
        data.unresolved_definitions.sort();
        data.unresolved_definitions.dedup();
        let basis = encode_fields(
            &data.body,
            &data.error,
            &data.captured_inputs,
            &data.unresolved,
            &data.unresolved_definitions,
        );
        Self {
            id: InvalidId::new(basis),
            body: data.body,
            error: data.error,
            captured_inputs: data.captured_inputs,
            unresolved: data.unresolved,
            unresolved_definitions: data.unresolved_definitions,
        }
    }

    pub fn id(&self) -> &InvalidId {
        &self.id
    }

    pub fn reference(&self) -> ObjectRef {
        match self.kind() {
            InvalidKind::Observation => ObjectRef::InvalidObservation(self.id.clone()),
            InvalidKind::Measurement => ObjectRef::InvalidMeasurement(self.id.clone()),
            InvalidKind::Relation => ObjectRef::InvalidRelation(self.id.clone()),
            InvalidKind::Composition => ObjectRef::InvalidComposition(self.id.clone()),
            InvalidKind::State => ObjectRef::InvalidState(self.id.clone()),
            InvalidKind::Transition => ObjectRef::InvalidTransition(self.id.clone()),
        }
    }

    pub fn body(&self) -> &InvalidBody {
        &self.body
    }

    pub fn kind(&self) -> InvalidKind {
        self.body.kind()
    }

    pub fn contract(&self) -> &VersionedId {
        self.body.contract()
    }

    pub fn error(&self) -> &PipelineError {
        &self.error
    }

    pub fn captured_inputs(&self) -> &[CapturedInput] {
        &self.captured_inputs
    }

    pub fn unresolved_references(&self) -> &[ObjectRef] {
        &self.unresolved
    }

    pub fn unresolved_definitions(&self) -> &[UnresolvedDefinition] {
        &self.unresolved_definitions
    }

    pub fn status(&self) -> LifecycleStatus {
        LifecycleStatus::Invalid
    }
}

impl CanonicalSemanticBasis for InvalidSemanticObject {
    fn canonical_basis(&self) -> Vec<u8> {
        encode_fields(
            &self.body,
            &self.error,
            &self.captured_inputs,
            &self.unresolved,
            &self.unresolved_definitions,
        )
    }
}

fn encode_fields(
    body: &InvalidBody,
    error: &PipelineError,
    captured_inputs: &[CapturedInput],
    unresolved: &[ObjectRef],
    unresolved_definitions: &[UnresolvedDefinition],
) -> Vec<u8> {
    let mut bytes = b"MDC-SCALAR-001/invalid/v1\0".to_vec();
    put_invalid_body(&mut bytes, body);
    put_pipeline_error(&mut bytes, error);
    bytes.extend_from_slice(&(captured_inputs.len() as u64).to_le_bytes());
    for input in captured_inputs {
        put_object_ref(&mut bytes, input.reference());
        put_bytes(&mut bytes, input.basis());
    }
    bytes.extend_from_slice(&(unresolved.len() as u64).to_le_bytes());
    for reference in unresolved {
        put_object_ref(&mut bytes, reference);
    }
    bytes.extend_from_slice(&(unresolved_definitions.len() as u64).to_le_bytes());
    for definition in unresolved_definitions {
        bytes.push(match definition.kind {
            DefinitionKind::Source => 0,
            DefinitionKind::Schema => 1,
            DefinitionKind::Operator => 2,
            DefinitionKind::Relation => 3,
            DefinitionKind::Composition => 4,
            DefinitionKind::State => 5,
            DefinitionKind::Transition => 6,
            DefinitionKind::Constraint => 7,
            DefinitionKind::StateCriterion => 8,
            DefinitionKind::TransitionCondition => 9,
            DefinitionKind::Conformance => 10,
        });
        put_reference(&mut bytes, &definition.reference);
    }
    bytes.push(2); // Invalid, never a usable or conformant result.
    bytes
}

/// Reference/definition closure is separate from admissibility. For an invalid
/// object the report describes its captured creation-time resolution snapshot.
/// It never repairs that snapshot after later admission of a missing input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageReport {
    pub resolved: Vec<ObjectRef>,
    pub unresolved: Vec<ObjectRef>,
    pub unresolved_definitions: Vec<UnresolvedDefinition>,
}

impl LineageReport {
    pub fn is_complete(&self) -> bool {
        self.unresolved.is_empty() && self.unresolved_definitions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::structure::{MemberOrder, MemberType, RelationContext, RelationKind};

    fn reference(name: &str) -> VersionedId {
        VersionedId::new(name, 1).unwrap()
    }

    fn data(reverse: bool, value: Option<i128>) -> InvalidArtifactData {
        let mut captured_inputs = alloc::vec![
            CapturedInput::new(ObjectRef::Observation(1), alloc::vec![11]),
            CapturedInput::new(ObjectRef::Observation(2), alloc::vec![22]),
            CapturedInput::new(ObjectRef::Observation(1), alloc::vec![11]),
        ];
        let mut unresolved = alloc::vec![ObjectRef::Observation(9), ObjectRef::Observation(8)];
        if reverse {
            captured_inputs.reverse();
            unresolved.reverse();
        }
        InvalidArtifactData {
            body: InvalidBody::Measurement {
                operator: reference("unknown-operator"),
                definition: None,
                inputs: alloc::vec![
                    ObjectRef::Observation(1),
                    ObjectRef::Observation(2),
                    ObjectRef::Observation(8),
                    ObjectRef::Observation(9),
                ],
                value,
                unit: None,
                context: None,
            },
            error: PipelineError::OperatorUnresolved(reference("unknown-operator")),
            captured_inputs,
            unresolved,
            unresolved_definitions: alloc::vec![UnresolvedDefinition {
                kind: DefinitionKind::Operator,
                reference: reference("unknown-operator"),
            }],
        }
    }

    #[test]
    fn capture_order_does_not_change_invalid_identity_or_drop_semantic_inputs() {
        let first = InvalidSemanticObject::new(data(false, None));
        let reverse = InvalidSemanticObject::new(data(true, None));
        assert_eq!(first.reference(), reverse.reference());
        assert_eq!(first.canonical_basis(), reverse.canonical_basis());
        assert_eq!(first.id().canonical_key(), first.canonical_basis());
        assert_eq!(first.captured_inputs().len(), 2);
        assert_eq!(first.status(), LifecycleStatus::Invalid);
        assert!(matches!(
            first.body(),
            InvalidBody::Measurement {
                definition: None,
                value: None,
                ..
            }
        ));
    }

    #[test]
    fn absent_value_and_conflicting_snapshots_remain_distinguishable() {
        let absent = InvalidSemanticObject::new(data(false, None));
        let zero = InvalidSemanticObject::new(data(false, Some(0)));
        assert_ne!(absent.reference(), zero.reference());
        let mut conflict = data(false, None);
        conflict.captured_inputs.push(CapturedInput::new(
            ObjectRef::Observation(1),
            alloc::vec![99],
        ));
        let conflict = InvalidSemanticObject::new(conflict);
        assert_eq!(conflict.captured_inputs().len(), 3);
        assert_ne!(absent.canonical_basis(), conflict.canonical_basis());
    }

    fn context() -> RelationContext {
        RelationContext::new(
            DomainContext::new("fixture", reference("ticks")).unwrap(),
            TemporalScope::new(0, 100).unwrap(),
            reference("frame"),
            reference("noncausal"),
        )
    }

    #[test]
    fn known_unordered_composition_normalizes_sets_without_losing_duplicates() {
        fn failure(reverse: bool, order: MemberOrder, resolved: bool) -> InvalidSemanticObject {
            let mut members = alloc::vec![
                ObjectRef::Measurement(1),
                ObjectRef::Measurement(2),
                ObjectRef::Measurement(1),
            ];
            let mut relations = alloc::vec![
                ObjectRef::Relation(9),
                ObjectRef::Relation(8),
                ObjectRef::Relation(9),
            ];
            if reverse {
                members.rotate_left(1);
                relations.reverse();
            }
            let mut data = data(false, None);
            data.body = InvalidBody::Composition {
                contract: reference("composition"),
                definition: resolved.then(|| CompositionContract {
                    reference: reference("composition"),
                    member_order: order,
                    member_type: MemberType::Measurements,
                    constraints: alloc::vec![reference("cardinality")],
                    context: context(),
                }),
                members,
                relations,
                constraints: None,
            };
            data.error = PipelineError::Composition(
                crate::model::composition::CompositionError::DuplicateMember(
                    ObjectRef::Measurement(1),
                ),
            );
            InvalidSemanticObject::new(data)
        }

        let first = failure(false, MemberOrder::Unordered, true);
        let reverse = failure(true, MemberOrder::Unordered, true);
        assert_eq!(first.reference(), reverse.reference());
        if let InvalidBody::Composition {
            members, relations, ..
        } = first.body()
        {
            assert_eq!(members.len(), 3);
            assert_eq!(relations.len(), 3);
            assert_eq!(
                members
                    .iter()
                    .filter(|reference| **reference == ObjectRef::Measurement(1))
                    .count(),
                2,
            );
        } else {
            panic!("expected retained Composition fields");
        }
        assert_ne!(
            failure(false, MemberOrder::Ordered, true).reference(),
            failure(true, MemberOrder::Ordered, true).reference(),
        );
        assert_ne!(
            failure(false, MemberOrder::Unordered, false).reference(),
            failure(true, MemberOrder::Unordered, false).reference(),
        );
    }

    #[test]
    fn known_symmetric_invalid_relation_normalizes_only_symmetric_operands() {
        fn failure(reverse: bool, kind: RelationKind) -> InvalidSemanticObject {
            let mut data = data(false, None);
            let (left, right) = (ObjectRef::Observation(1), ObjectRef::Observation(2));
            let (left, right) = if reverse {
                (right, left)
            } else {
                (left, right)
            };
            data.body = InvalidBody::Relation {
                contract: reference("relation"),
                definition: Some(RelationContract {
                    reference: reference("relation"),
                    kind,
                    context: context(),
                }),
                left,
                right,
                value: None,
            };
            data.error =
                PipelineError::Relation(crate::model::relation::RelationError::NotAdjacent);
            InvalidSemanticObject::new(data)
        }
        let symmetric = RelationKind::Adjacent {
            step: 10,
            symmetric: true,
        };
        assert_eq!(
            failure(false, symmetric.clone()).reference(),
            failure(true, symmetric).reference(),
        );
        assert_ne!(
            failure(false, RelationKind::Proportion).reference(),
            failure(true, RelationKind::Proportion).reference(),
        );
    }
}
