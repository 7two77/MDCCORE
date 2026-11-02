use crate::contract::encoding::{put_object_ref, put_reference, put_relation_context};
use crate::contract::evaluation::{aggregate_results, ConformanceResult, LifecycleStatus};
use crate::contract::registry::VersionedId;
use crate::contract::structure::{
    CompositionContract, ConstraintContract, ConstraintKind, MemberOrder, MemberType,
    RelationContext,
};
use crate::identity::{CanonicalSemanticBasis, IdentityAdapter};
use crate::model::relation::{AdmittedRelation, ResolvedRelationInput};
use crate::provenance::{DerivationProvenance, ObjectRef};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

/// Legacy raw composition retained for compatibility. It is not an admitted object.
pub struct Composition {
    pub id: u64,
    pub members: Vec<u64>,
    pub relations: Vec<u64>,
    pub constraints_satisfied: bool,
    pub status: LifecycleStatus,
}

/// An actual result of evaluating a registered local composition constraint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstraintResult {
    reference: VersionedId,
    kind: ConstraintKind,
    result: ConformanceResult,
}

impl ConstraintResult {
    pub fn reference(&self) -> &VersionedId {
        &self.reference
    }

    pub fn kind(&self) -> &ConstraintKind {
        &self.kind
    }

    pub fn result(&self) -> ConformanceResult {
        self.result
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompositionError {
    EmptyMembers,
    InvalidMemberType(ObjectRef),
    DuplicateMember(ObjectRef),
    DuplicateRelation(ObjectRef),
    IncompatibleContext,
    OutsideTemporalScope,
    RelationContextMismatch(ObjectRef),
    ExternalEndpoint(ObjectRef),
    ConstraintSetMismatch,
    FailedConstraints(Vec<ConstraintResult>),
}

/// A composition constructed from admitted scalar members and relations.
/// Local constraint satisfaction does not promote Candidate to Conformant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedComposition {
    id: u64,
    contract: VersionedId,
    member_order: MemberOrder,
    member_type: MemberType,
    members: Vec<ObjectRef>,
    relations: Vec<ObjectRef>,
    constraints: Vec<ConstraintResult>,
    constraints_result: ConformanceResult,
    context: RelationContext,
    provenance: DerivationProvenance,
}

impl AdmittedComposition {
    pub(crate) fn derive(
        contract: &CompositionContract,
        mut members: Vec<ResolvedRelationInput>,
        mut relations: Vec<AdmittedRelation>,
        mut constraints: Vec<ConstraintContract>,
    ) -> Result<Self, CompositionError> {
        if members.is_empty() {
            return Err(CompositionError::EmptyMembers);
        }
        let mut declared = BTreeSet::new();
        for member in &members {
            let allowed = matches!(
                (&contract.member_type, &member.reference),
                (MemberType::Observations, ObjectRef::Observation(_))
                    | (MemberType::Measurements, ObjectRef::Measurement(_))
                    | (MemberType::ScalarInputs, ObjectRef::Observation(_))
                    | (MemberType::ScalarInputs, ObjectRef::Measurement(_))
            );
            if !allowed {
                return Err(CompositionError::InvalidMemberType(
                    member.reference.clone(),
                ));
            }
            if !declared.insert(member.reference.clone()) {
                return Err(CompositionError::DuplicateMember(member.reference.clone()));
            }
            if member.context != *contract.context.domain() {
                return Err(CompositionError::IncompatibleContext);
            }
            if !contract
                .context
                .scope()
                .contains(member.temporal_bounds.0, member.temporal_bounds.1)
            {
                return Err(CompositionError::OutsideTemporalScope);
            }
        }
        let mut relation_refs = BTreeSet::new();
        for relation in &relations {
            let reference = ObjectRef::Relation(relation.id());
            if !relation_refs.insert(reference.clone()) {
                return Err(CompositionError::DuplicateRelation(reference));
            }
            if relation.context() != &contract.context {
                return Err(CompositionError::RelationContextMismatch(reference));
            }
            for endpoint in [relation.left_ref(), relation.right_ref()] {
                if !declared.contains(endpoint) {
                    return Err(CompositionError::ExternalEndpoint(endpoint.clone()));
                }
            }
        }

        if contract.member_order == MemberOrder::Unordered {
            members.sort_by(|left, right| left.reference.cmp(&right.reference));
        }
        relations.sort_by_key(AdmittedRelation::id);
        constraints.sort_by(|left, right| left.reference.cmp(&right.reference));
        if constraints.is_empty()
            || constraints
                .windows(2)
                .any(|pair| pair[0].reference == pair[1].reference)
            || !constraints
                .iter()
                .map(|constraint| &constraint.reference)
                .eq(contract.constraints.iter())
        {
            return Err(CompositionError::ConstraintSetMismatch);
        }

        let evaluated: Vec<_> = constraints
            .into_iter()
            .map(|constraint| {
                let passed = evaluate_constraint(&constraint.kind, &members, &relations);
                ConstraintResult {
                    reference: constraint.reference,
                    kind: constraint.kind,
                    result: if passed {
                        ConformanceResult::Pass
                    } else {
                        ConformanceResult::Fail
                    },
                }
            })
            .collect();
        let results: Vec<_> = evaluated
            .iter()
            .map(|constraint| constraint.result)
            .collect();
        let constraints_result =
            aggregate_results(&results).map_err(|_| CompositionError::ConstraintSetMismatch)?;
        if constraints_result == ConformanceResult::Fail {
            return Err(CompositionError::FailedConstraints(evaluated));
        }

        let members: Vec<_> = members.into_iter().map(|member| member.reference).collect();
        let relations: Vec<_> = relations
            .into_iter()
            .map(|relation| ObjectRef::Relation(relation.id()))
            .collect();
        let mut inputs = members.clone();
        inputs.extend(relations.iter().cloned());
        let provenance = DerivationProvenance::new(contract.reference.clone(), inputs);
        let mut composition = Self {
            id: 0,
            contract: contract.reference.clone(),
            member_order: contract.member_order,
            member_type: contract.member_type,
            members,
            relations,
            constraints: evaluated,
            constraints_result,
            context: contract.context.clone(),
            provenance,
        };
        composition.id =
            IdentityAdapter::derive_identity_from_basis(&composition.canonical_basis());
        Ok(composition)
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn contract(&self) -> &VersionedId {
        &self.contract
    }

    pub fn member_order(&self) -> MemberOrder {
        self.member_order
    }

    pub fn member_type(&self) -> MemberType {
        self.member_type
    }

    pub fn members(&self) -> &[ObjectRef] {
        &self.members
    }

    pub fn relations(&self) -> &[ObjectRef] {
        &self.relations
    }

    pub fn constraints(&self) -> &[ConstraintResult] {
        &self.constraints
    }

    pub fn constraints_result(&self) -> ConformanceResult {
        self.constraints_result
    }

    pub fn context(&self) -> &RelationContext {
        &self.context
    }

    pub fn provenance(&self) -> &DerivationProvenance {
        &self.provenance
    }

    pub fn status(&self) -> LifecycleStatus {
        LifecycleStatus::Candidate
    }
}

fn evaluate_constraint(
    kind: &ConstraintKind,
    members: &[ResolvedRelationInput],
    relations: &[AdmittedRelation],
) -> bool {
    match kind {
        ConstraintKind::Cardinality { min, max } => {
            let count = members.len() as u128;
            u128::from(*min) <= count && count <= u128::from(*max)
        }
        ConstraintKind::SameUnits => members.iter().all(|member| member.unit == members[0].unit),
        ConstraintKind::Connected => connected(members, relations),
        ConstraintKind::Acyclic => acyclic(members, relations),
        ConstraintKind::OrderedChain { relation } => members.windows(2).all(|pair| {
            relations.iter().any(|edge| {
                edge.contract() == relation
                    && edge.left_ref() == &pair[0].reference
                    && edge.right_ref() == &pair[1].reference
            })
        }),
        ConstraintKind::RequiresRelation { relation } => {
            relations.iter().any(|edge| edge.contract() == relation)
        }
    }
}

fn connected(members: &[ResolvedRelationInput], relations: &[AdmittedRelation]) -> bool {
    let mut visited = BTreeSet::new();
    visited.insert(members[0].reference.clone());
    loop {
        let previous_len = visited.len();
        for relation in relations {
            if visited.contains(relation.left_ref()) || visited.contains(relation.right_ref()) {
                visited.insert(relation.left_ref().clone());
                visited.insert(relation.right_ref().clone());
            }
        }
        if visited.len() == previous_len {
            return visited.len() == members.len();
        }
    }
}

fn acyclic(members: &[ResolvedRelationInput], relations: &[AdmittedRelation]) -> bool {
    let mut indegree: BTreeMap<_, usize> = members
        .iter()
        .map(|member| (member.reference.clone(), 0))
        .collect();
    let mut adjacency: BTreeMap<ObjectRef, BTreeSet<ObjectRef>> = BTreeMap::new();
    for relation in relations.iter().filter(|relation| relation.is_directed()) {
        if adjacency
            .entry(relation.left_ref().clone())
            .or_default()
            .insert(relation.right_ref().clone())
        {
            // Referential closure was checked before any constraint is evaluated.
            if let Some(degree) = indegree.get_mut(relation.right_ref()) {
                *degree += 1;
            }
        }
    }
    let mut ready: BTreeSet<_> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(reference, _)| reference.clone())
        .collect();
    let mut removed = 0;
    while let Some(reference) = ready.pop_first() {
        removed += 1;
        if let Some(targets) = adjacency.get(&reference) {
            for target in targets {
                if let Some(degree) = indegree.get_mut(target) {
                    *degree -= 1;
                    if *degree == 0 {
                        ready.insert(target.clone());
                    }
                }
            }
        }
    }
    removed == members.len()
}

impl CanonicalSemanticBasis for AdmittedComposition {
    fn canonical_basis(&self) -> Vec<u8> {
        let mut bytes = b"MDC-SCALAR-001/composition/v1\0".to_vec();
        put_reference(&mut bytes, &self.contract);
        bytes.push(match self.member_order {
            MemberOrder::Ordered => 0,
            MemberOrder::Unordered => 1,
        });
        bytes.push(match self.member_type {
            MemberType::Observations => 0,
            MemberType::Measurements => 1,
            MemberType::ScalarInputs => 2,
        });
        put_references(&mut bytes, &self.members);
        put_references(&mut bytes, &self.relations);
        bytes.extend_from_slice(&(self.constraints.len() as u64).to_le_bytes());
        for constraint in &self.constraints {
            put_reference(&mut bytes, &constraint.reference);
            put_constraint_kind(&mut bytes, &constraint.kind);
            bytes.push(match constraint.result {
                ConformanceResult::Pass => 0,
                ConformanceResult::Fail => 1,
                ConformanceResult::Unresolved => 2,
            });
        }
        bytes.push(match self.constraints_result {
            ConformanceResult::Pass => 0,
            ConformanceResult::Fail => 1,
            ConformanceResult::Unresolved => 2,
        });
        put_relation_context(&mut bytes, &self.context);
        put_reference(&mut bytes, self.provenance.contract());
        put_references(&mut bytes, self.provenance.inputs());
        bytes.push(0); // Candidate; local checks do not execute CB conformance.
        bytes
    }
}

fn put_references(bytes: &mut Vec<u8>, references: &[ObjectRef]) {
    bytes.extend_from_slice(&(references.len() as u64).to_le_bytes());
    for reference in references {
        put_object_ref(bytes, reference);
    }
}

fn put_constraint_kind(bytes: &mut Vec<u8>, kind: &ConstraintKind) {
    match kind {
        ConstraintKind::Cardinality { min, max } => {
            bytes.push(0);
            bytes.extend_from_slice(&min.to_le_bytes());
            bytes.extend_from_slice(&max.to_le_bytes());
        }
        ConstraintKind::SameUnits => bytes.push(1),
        ConstraintKind::Connected => bytes.push(2),
        ConstraintKind::Acyclic => bytes.push(3),
        ConstraintKind::OrderedChain { relation } => {
            bytes.push(4);
            put_reference(bytes, relation);
        }
        ConstraintKind::RequiresRelation { relation } => {
            bytes.push(5);
            put_reference(bytes, relation);
        }
    }
}
