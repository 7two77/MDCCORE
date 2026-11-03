//! Explicit, versioned contracts for the initial scalar profile.
use super::conformance::{
    ConformanceContract, ConformanceCriterionContract, ConformanceCriterionKind, TargetType,
};
use super::lifecycle::LifecycleContract;
use super::projection::ProjectionProfile;

use super::dynamics::{
    CriterionOrder, StateContract, StateCriterionContract, StateCriterionKind,
    TransitionConditionContract, TransitionConditionKind, TransitionContract,
};
use super::structure::{
    CompositionContract, ConstraintContract, ConstraintKind, MemberOrder, RelationContract,
    RelationKind,
};
use alloc::collections::BTreeSet;
use alloc::collections::{btree_map::Entry, BTreeMap};
use alloc::string::String;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct VersionedId {
    id: String,
    version: u32,
}

impl VersionedId {
    pub fn new(id: impl Into<String>, version: u32) -> Result<Self, RegistryError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(RegistryError::EmptyIdentifier);
        }
        if version == 0 {
            return Err(RegistryError::InvalidVersion);
        }
        Ok(Self { id, version })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn version(&self) -> u32 {
        self.version
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainContext {
    domain: String,
    time_basis: VersionedId,
}

impl DomainContext {
    pub fn new(domain: impl Into<String>, time_basis: VersionedId) -> Result<Self, RegistryError> {
        let domain = domain.into();
        if domain.trim().is_empty() {
            return Err(RegistryError::EmptyDomain);
        }
        Ok(Self { domain, time_basis })
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub fn time_basis(&self) -> &VersionedId {
        &self.time_basis
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarSchema {
    SignedI64Le,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaContract {
    pub reference: VersionedId,
    pub format: ScalarSchema,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceContract {
    pub reference: VersionedId,
    pub schema: VersionedId,
    pub context: DomainContext,
    pub unit: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatorKind {
    Scalar,
    Difference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatorContract {
    pub reference: VersionedId,
    pub kind: OperatorKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryError {
    EmptyIdentifier,
    InvalidVersion,
    EmptyDomain,
    EmptyUnit,
    DuplicateReference(VersionedId),
    SchemaUnresolved(VersionedId),
    InvalidTemporalScope,
    InvalidAdjacencyStep,
    InvalidCardinality,
    EmptyConstraintSet,
    DuplicateConstraint(VersionedId),
    ConstraintUnresolved(VersionedId),
    RelationContractUnresolved(VersionedId),
    IncompatibleConstraintOrdering,
    SourceContractUnresolved(VersionedId),
    CompositionContractUnresolved(VersionedId),
    StateCriterionUnresolved(VersionedId),
    StateContractUnresolved(VersionedId),
    TransitionConditionUnresolved(VersionedId),
    EmptyStateCriteria,
    DuplicateStateCriterion(VersionedId),
    EmptyTransitionConditions,
    DuplicateTransitionCondition(VersionedId),
    DynamicsContextMismatch,
    InvalidStateCriterionScope,
    MissingTransitionRequirement,
    EmptyConformanceCriteria,
    ConformanceCriterionUnresolved(VersionedId),
    DuplicateConformanceCriterion(VersionedId),
    InvalidConformanceDefinition,
    ProjectionConformanceUnresolved(VersionedId),
    ProjectionContractMismatch,
    InvalidLifecycleReasons,
}

/// A registry never replaces a previously registered contract version.
#[derive(Default)]
pub struct Registry {
    lifecycle: BTreeMap<VersionedId, LifecycleContract>,
    projections: BTreeMap<VersionedId, ProjectionProfile>,
    conformance_criteria: BTreeMap<VersionedId, ConformanceCriterionContract>,
    conformance: BTreeMap<VersionedId, ConformanceContract>,
    schemas: BTreeMap<VersionedId, SchemaContract>,
    sources: BTreeMap<VersionedId, SourceContract>,
    operators: BTreeMap<VersionedId, OperatorContract>,
    relations: BTreeMap<VersionedId, RelationContract>,
    constraints: BTreeMap<VersionedId, ConstraintContract>,
    compositions: BTreeMap<VersionedId, CompositionContract>,
    state_criteria: BTreeMap<VersionedId, StateCriterionContract>,
    states: BTreeMap<VersionedId, StateContract>,
    transition_conditions: BTreeMap<VersionedId, TransitionConditionContract>,
    transitions: BTreeMap<VersionedId, TransitionContract>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_lifecycle(
        &mut self,
        mut contract: LifecycleContract,
    ) -> Result<(), RegistryError> {
        if self.lifecycle.contains_key(&contract.reference) {
            return Err(RegistryError::DuplicateReference(contract.reference));
        }
        contract.allowed_reasons.sort();
        if contract.allowed_reasons.is_empty()
            || contract.allowed_reasons.windows(2).any(|p| p[0] == p[1])
        {
            return Err(RegistryError::InvalidLifecycleReasons);
        }
        insert(&mut self.lifecycle, contract.reference.clone(), contract)
    }
    pub fn lifecycle(&self, reference: &VersionedId) -> Option<&LifecycleContract> {
        self.lifecycle.get(reference)
    }

    pub fn register_schema(&mut self, contract: SchemaContract) -> Result<(), RegistryError> {
        insert(&mut self.schemas, contract.reference.clone(), contract)
    }

    pub fn register_source(&mut self, contract: SourceContract) -> Result<(), RegistryError> {
        if self.sources.contains_key(&contract.reference) {
            return Err(RegistryError::DuplicateReference(contract.reference));
        }
        if !self.schemas.contains_key(&contract.schema) {
            return Err(RegistryError::SchemaUnresolved(contract.schema));
        }
        if contract.unit.trim().is_empty() {
            return Err(RegistryError::EmptyUnit);
        }
        insert(&mut self.sources, contract.reference.clone(), contract)
    }

    pub fn register_operator(&mut self, contract: OperatorContract) -> Result<(), RegistryError> {
        insert(&mut self.operators, contract.reference.clone(), contract)
    }

    pub fn register_relation(&mut self, contract: RelationContract) -> Result<(), RegistryError> {
        if self.relations.contains_key(&contract.reference) {
            return Err(RegistryError::DuplicateReference(contract.reference));
        }
        if matches!(contract.kind, RelationKind::Adjacent { step: 0, .. }) {
            return Err(RegistryError::InvalidAdjacencyStep);
        }
        insert(&mut self.relations, contract.reference.clone(), contract)
    }

    pub fn register_constraint(
        &mut self,
        contract: ConstraintContract,
    ) -> Result<(), RegistryError> {
        if self.constraints.contains_key(&contract.reference) {
            return Err(RegistryError::DuplicateReference(contract.reference));
        }
        match &contract.kind {
            ConstraintKind::Cardinality { min, max } if *min == 0 || min > max => {
                return Err(RegistryError::InvalidCardinality);
            }
            ConstraintKind::OrderedChain { relation }
            | ConstraintKind::RequiresRelation { relation } => {
                let target = self
                    .relations
                    .get(relation)
                    .ok_or_else(|| RegistryError::RelationContractUnresolved(relation.clone()))?;
                if matches!(contract.kind, ConstraintKind::OrderedChain { .. })
                    && !target.kind.is_directed()
                {
                    return Err(RegistryError::IncompatibleConstraintOrdering);
                }
            }
            _ => {}
        }
        insert(&mut self.constraints, contract.reference.clone(), contract)
    }

    pub fn register_composition(
        &mut self,
        mut contract: CompositionContract,
    ) -> Result<(), RegistryError> {
        if self.compositions.contains_key(&contract.reference) {
            return Err(RegistryError::DuplicateReference(contract.reference));
        }
        if contract.constraints.is_empty() {
            return Err(RegistryError::EmptyConstraintSet);
        }
        let mut seen = BTreeSet::new();
        for reference in &contract.constraints {
            if !seen.insert(reference) {
                return Err(RegistryError::DuplicateConstraint(reference.clone()));
            }
            let constraint = self
                .constraints
                .get(reference)
                .ok_or_else(|| RegistryError::ConstraintUnresolved(reference.clone()))?;
            if matches!(constraint.kind, ConstraintKind::OrderedChain { .. })
                && contract.member_order != MemberOrder::Ordered
            {
                return Err(RegistryError::IncompatibleConstraintOrdering);
            }
        }
        contract.constraints.sort();
        insert(&mut self.compositions, contract.reference.clone(), contract)
    }

    pub fn register_state_criterion(
        &mut self,
        contract: StateCriterionContract,
    ) -> Result<(), RegistryError> {
        if self.state_criteria.contains_key(&contract.reference) {
            return Err(RegistryError::DuplicateReference(contract.reference));
        }
        match &contract.kind {
            StateCriterionKind::Cardinality { min, max } if *min == 0 || min > max => {
                return Err(RegistryError::InvalidCardinality);
            }
            StateCriterionKind::HasRelation { relation }
                if !self.relations.contains_key(relation) =>
            {
                return Err(RegistryError::RelationContractUnresolved(relation.clone()));
            }
            StateCriterionKind::ConstraintPassed { constraint }
                if !self.constraints.contains_key(constraint) =>
            {
                return Err(RegistryError::ConstraintUnresolved(constraint.clone()));
            }
            StateCriterionKind::ObservationAfter { source }
                if !self.sources.contains_key(source) =>
            {
                return Err(RegistryError::SourceContractUnresolved(source.clone()));
            }
            _ => {}
        }
        insert(
            &mut self.state_criteria,
            contract.reference.clone(),
            contract,
        )
    }

    pub fn register_state(&mut self, mut contract: StateContract) -> Result<(), RegistryError> {
        if self.states.contains_key(&contract.reference) {
            return Err(RegistryError::DuplicateReference(contract.reference));
        }
        let composition = self
            .compositions
            .get(&contract.composition)
            .ok_or_else(|| {
                RegistryError::CompositionContractUnresolved(contract.composition.clone())
            })?;
        if contract.context.frame() != &composition.context {
            return Err(RegistryError::DynamicsContextMismatch);
        }
        if contract.criteria.is_empty() {
            return Err(RegistryError::EmptyStateCriteria);
        }
        let mut seen = BTreeSet::new();
        for reference in &contract.criteria {
            if !seen.insert(reference) {
                return Err(RegistryError::DuplicateStateCriterion(reference.clone()));
            }
            let criterion = self
                .state_criteria
                .get(reference)
                .ok_or_else(|| RegistryError::StateCriterionUnresolved(reference.clone()))?;
            match &criterion.kind {
                StateCriterionKind::ConstraintPassed { constraint } => {
                    if !composition.constraints.contains(constraint) {
                        return Err(RegistryError::InvalidStateCriterionScope);
                    }
                }
                StateCriterionKind::HasRelation { relation } => {
                    let relation = self.relations.get(relation).ok_or_else(|| {
                        RegistryError::RelationContractUnresolved(relation.clone())
                    })?;
                    if &relation.context != contract.context.frame() {
                        return Err(RegistryError::DynamicsContextMismatch);
                    }
                }
                StateCriterionKind::ObservationAfter { source } => {
                    let source = self
                        .sources
                        .get(source)
                        .ok_or_else(|| RegistryError::SourceContractUnresolved(source.clone()))?;
                    if &source.context != contract.context.frame().domain() {
                        return Err(RegistryError::DynamicsContextMismatch);
                    }
                }
                _ => {}
            }
        }
        if contract.criterion_order == CriterionOrder::Unordered {
            contract.criteria.sort();
        }
        insert(&mut self.states, contract.reference.clone(), contract)
    }

    pub fn register_transition_condition(
        &mut self,
        contract: TransitionConditionContract,
    ) -> Result<(), RegistryError> {
        if self.transition_conditions.contains_key(&contract.reference) {
            return Err(RegistryError::DuplicateReference(contract.reference));
        }
        if let TransitionConditionKind::ObservationAfterTarget { source } = &contract.kind {
            if !self.sources.contains_key(source) {
                return Err(RegistryError::SourceContractUnresolved(source.clone()));
            }
        }
        insert(
            &mut self.transition_conditions,
            contract.reference.clone(),
            contract,
        )
    }

    pub fn register_transition(
        &mut self,
        mut contract: TransitionContract,
    ) -> Result<(), RegistryError> {
        if self.transitions.contains_key(&contract.reference) {
            return Err(RegistryError::DuplicateReference(contract.reference));
        }
        let source = self
            .states
            .get(&contract.source_state)
            .ok_or_else(|| RegistryError::StateContractUnresolved(contract.source_state.clone()))?;
        let target = self
            .states
            .get(&contract.target_state)
            .ok_or_else(|| RegistryError::StateContractUnresolved(contract.target_state.clone()))?;
        if source.context != contract.context || target.context != contract.context {
            return Err(RegistryError::DynamicsContextMismatch);
        }
        if contract.conditions.is_empty() {
            return Err(RegistryError::EmptyTransitionConditions);
        }
        let mut seen = BTreeSet::new();
        let mut temporal = false;
        let mut source_criteria = false;
        let mut target_criteria = false;
        for reference in &contract.conditions {
            if !seen.insert(reference) {
                return Err(RegistryError::DuplicateTransitionCondition(
                    reference.clone(),
                ));
            }
            let condition = self
                .transition_conditions
                .get(reference)
                .ok_or_else(|| RegistryError::TransitionConditionUnresolved(reference.clone()))?;
            match &condition.kind {
                TransitionConditionKind::TemporalOrder { .. } => temporal = true,
                TransitionConditionKind::SourceCriteriaPass => source_criteria = true,
                TransitionConditionKind::TargetCriteriaPass => target_criteria = true,
                TransitionConditionKind::ObservationAfterTarget { source } => {
                    let source = self
                        .sources
                        .get(source)
                        .ok_or_else(|| RegistryError::SourceContractUnresolved(source.clone()))?;
                    if &source.context != contract.context.frame().domain() {
                        return Err(RegistryError::DynamicsContextMismatch);
                    }
                }
            }
        }
        if !(temporal && source_criteria && target_criteria) {
            return Err(RegistryError::MissingTransitionRequirement);
        }
        if contract.condition_order == CriterionOrder::Unordered {
            contract.conditions.sort();
        }
        insert(&mut self.transitions, contract.reference.clone(), contract)
    }

    pub fn state_criterion(&self, reference: &VersionedId) -> Option<&StateCriterionContract> {
        self.state_criteria.get(reference)
    }

    pub fn state(&self, reference: &VersionedId) -> Option<&StateContract> {
        self.states.get(reference)
    }

    pub fn transition_condition(
        &self,
        reference: &VersionedId,
    ) -> Option<&TransitionConditionContract> {
        self.transition_conditions.get(reference)
    }

    pub fn transition(&self, reference: &VersionedId) -> Option<&TransitionContract> {
        self.transitions.get(reference)
    }

    pub fn schema(&self, reference: &VersionedId) -> Option<&SchemaContract> {
        self.schemas.get(reference)
    }

    pub fn source(&self, reference: &VersionedId) -> Option<&SourceContract> {
        self.sources.get(reference)
    }

    pub fn operator(&self, reference: &VersionedId) -> Option<&OperatorContract> {
        self.operators.get(reference)
    }

    pub fn relation(&self, reference: &VersionedId) -> Option<&RelationContract> {
        self.relations.get(reference)
    }

    pub fn constraint(&self, reference: &VersionedId) -> Option<&ConstraintContract> {
        self.constraints.get(reference)
    }

    pub fn composition(&self, reference: &VersionedId) -> Option<&CompositionContract> {
        self.compositions.get(reference)
    }
}

fn insert<T>(
    map: &mut BTreeMap<VersionedId, T>,
    reference: VersionedId,
    value: T,
) -> Result<(), RegistryError> {
    match map.entry(reference) {
        Entry::Vacant(entry) => {
            entry.insert(value);
            Ok(())
        }
        Entry::Occupied(entry) => Err(RegistryError::DuplicateReference(entry.key().clone())),
    }
}

#[cfg(test)]
mod dynamics_tests {
    use super::*;
    use crate::contract::dynamics::{DynamicsContext, StateDefinition};
    use crate::contract::structure::{MemberType, RelationContext, TemporalScope};
    use alloc::vec;

    fn id(name: &str) -> VersionedId {
        VersionedId::new(name, 1).unwrap()
    }

    fn frame() -> RelationContext {
        RelationContext::new(
            DomainContext::new("fixture", id("ticks")).unwrap(),
            TemporalScope::new(0, 100).unwrap(),
            id("frame"),
            id("noncausal"),
        )
    }

    fn context() -> DynamicsContext {
        DynamicsContext::new(frame(), id("calculus"), id("domain"))
    }

    fn registry() -> Registry {
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
                context: frame().domain().clone(),
                unit: "count".into(),
            })
            .unwrap();
        registry
            .register_relation(RelationContract {
                reference: id("before"),
                kind: RelationKind::Precedes,
                context: frame(),
            })
            .unwrap();
        for name in ["cardinality", "outside-constraint"] {
            registry
                .register_constraint(ConstraintContract {
                    reference: id(name),
                    kind: ConstraintKind::Cardinality { min: 1, max: 3 },
                })
                .unwrap();
        }
        registry
            .register_composition(CompositionContract {
                reference: id("composition"),
                member_order: MemberOrder::Unordered,
                member_type: MemberType::Observations,
                constraints: vec![id("cardinality")],
                context: frame(),
            })
            .unwrap();
        for (name, kind) in [
            (
                "z-count",
                StateCriterionKind::Cardinality { min: 1, max: 3 },
            ),
            ("a-span", StateCriterionKind::MaxTemporalSpan { ticks: 20 }),
        ] {
            registry
                .register_state_criterion(StateCriterionContract {
                    reference: id(name),
                    kind,
                })
                .unwrap();
        }
        for (name, kind) in [
            (
                "z-time",
                TransitionConditionKind::TemporalOrder { strict: true },
            ),
            ("a-source", TransitionConditionKind::SourceCriteriaPass),
            ("b-target", TransitionConditionKind::TargetCriteriaPass),
        ] {
            registry
                .register_transition_condition(TransitionConditionContract {
                    reference: id(name),
                    kind,
                })
                .unwrap();
        }
        registry
    }

    fn state(name: &str, order: CriterionOrder) -> StateContract {
        StateContract {
            reference: id(name),
            composition: id("composition"),
            definition: StateDefinition::CompositionAbstraction,
            criterion_order: order,
            criteria: vec![id("z-count"), id("a-span")],
            context: context(),
        }
    }

    fn transition() -> TransitionContract {
        TransitionContract {
            reference: id("transition"),
            source_state: id("state"),
            target_state: id("state"),
            condition_order: CriterionOrder::Unordered,
            conditions: vec![id("z-time"), id("a-source"), id("b-target")],
            context: context(),
        }
    }

    #[test]
    fn state_criterion_dependencies_resolve_exact_versions_without_fallback() {
        let mut registry = registry();
        for (kind, expected) in [
            (
                StateCriterionKind::HasRelation {
                    relation: VersionedId::new("before", 2).unwrap(),
                },
                RegistryError::RelationContractUnresolved(VersionedId::new("before", 2).unwrap()),
            ),
            (
                StateCriterionKind::ConstraintPassed {
                    constraint: VersionedId::new("cardinality", 2).unwrap(),
                },
                RegistryError::ConstraintUnresolved(VersionedId::new("cardinality", 2).unwrap()),
            ),
            (
                StateCriterionKind::ObservationAfter {
                    source: VersionedId::new("source", 2).unwrap(),
                },
                RegistryError::SourceContractUnresolved(VersionedId::new("source", 2).unwrap()),
            ),
        ] {
            assert_eq!(
                registry.register_state_criterion(StateCriterionContract {
                    reference: id("new"),
                    kind
                }),
                Err(expected)
            );
            assert!(registry.state_criterion(&id("new")).is_none());
        }
        assert_eq!(
            registry.register_state_criterion(StateCriterionContract {
                reference: id("new"),
                kind: StateCriterionKind::Cardinality { min: 4, max: 2 },
            }),
            Err(RegistryError::InvalidCardinality)
        );
    }

    #[test]
    fn state_registration_requires_nonempty_unique_resolved_criteria_and_composition() {
        let mut registry = registry();
        let mut contract = state("state", CriterionOrder::Ordered);
        contract.composition = VersionedId::new("composition", 2).unwrap();
        assert_eq!(
            registry.register_state(contract.clone()),
            Err(RegistryError::CompositionContractUnresolved(
                contract.composition
            ))
        );
        let mut contract = state("state", CriterionOrder::Ordered);
        contract.criteria.clear();
        assert_eq!(
            registry.register_state(contract),
            Err(RegistryError::EmptyStateCriteria)
        );
        let mut contract = state("state", CriterionOrder::Ordered);
        contract.criteria = vec![id("z-count"), id("z-count")];
        assert_eq!(
            registry.register_state(contract),
            Err(RegistryError::DuplicateStateCriterion(id("z-count")))
        );
        let mut contract = state("state", CriterionOrder::Ordered);
        contract.criteria = vec![VersionedId::new("z-count", 2).unwrap()];
        assert_eq!(
            registry.register_state(contract.clone()),
            Err(RegistryError::StateCriterionUnresolved(
                contract.criteria[0].clone()
            ))
        );
        assert!(registry.state(&id("state")).is_none());
    }

    #[test]
    fn state_registration_checks_frame_and_declared_constraint_membership() {
        let mut registry = registry();
        registry
            .register_state_criterion(StateCriterionContract {
                reference: id("outside"),
                kind: StateCriterionKind::ConstraintPassed {
                    constraint: id("outside-constraint"),
                },
            })
            .unwrap();
        let mut contract = state("state", CriterionOrder::Unordered);
        contract.criteria = vec![id("outside")];
        assert_eq!(
            registry.register_state(contract),
            Err(RegistryError::InvalidStateCriterionScope)
        );
        let mut contract = state("state", CriterionOrder::Unordered);
        contract.context = DynamicsContext::new(
            RelationContext::new(
                frame().domain().clone(),
                TemporalScope::new(0, 100).unwrap(),
                id("other-frame"),
                id("noncausal"),
            ),
            id("calculus"),
            id("domain"),
        );
        assert_eq!(
            registry.register_state(contract),
            Err(RegistryError::DynamicsContextMismatch)
        );
    }

    #[test]
    fn criteria_order_follows_contract_and_existing_versions_are_never_replaced() {
        let mut registry = registry();
        let ordered = state("ordered", CriterionOrder::Ordered);
        registry.register_state(ordered.clone()).unwrap();
        assert_eq!(
            registry.state(&id("ordered")).unwrap().criteria,
            ordered.criteria
        );
        let unordered = state("unordered", CriterionOrder::Unordered);
        registry.register_state(unordered.clone()).unwrap();
        assert_eq!(
            registry.state(&id("unordered")).unwrap().criteria,
            vec![id("a-span"), id("z-count")]
        );
        let mut replacement = ordered.clone();
        replacement.criteria = vec![id("a-span")];
        assert_eq!(
            registry.register_state(replacement),
            Err(RegistryError::DuplicateReference(id("ordered")))
        );
        assert_eq!(registry.state(&id("ordered")), Some(&ordered));
        let original = registry.state_criterion(&id("z-count")).unwrap().clone();
        assert_eq!(
            registry.register_state_criterion(StateCriterionContract {
                reference: id("z-count"),
                kind: StateCriterionKind::MaxTemporalSpan { ticks: 0 },
            }),
            Err(RegistryError::DuplicateReference(id("z-count")))
        );
        assert_eq!(registry.state_criterion(&id("z-count")), Some(&original));
    }

    #[test]
    fn transition_requires_existing_state_versions_and_matching_calculus_versions() {
        let mut registry = registry();
        registry
            .register_state(state("state", CriterionOrder::Unordered))
            .unwrap();
        let mut contract = transition();
        contract.target_state = VersionedId::new("state", 2).unwrap();
        assert_eq!(
            registry.register_transition(contract.clone()),
            Err(RegistryError::StateContractUnresolved(
                contract.target_state
            ))
        );
        let mut contract = transition();
        contract.context = DynamicsContext::new(
            frame(),
            VersionedId::new("calculus", 2).unwrap(),
            id("domain"),
        );
        assert_eq!(
            registry.register_transition(contract),
            Err(RegistryError::DynamicsContextMismatch)
        );
        assert!(registry.transition(&id("transition")).is_none());
    }

    #[test]
    fn transition_condition_set_must_resolve_and_cover_all_required_semantics() {
        let mut registry = registry();
        registry
            .register_state(state("state", CriterionOrder::Unordered))
            .unwrap();
        let mut contract = transition();
        contract.conditions.clear();
        assert_eq!(
            registry.register_transition(contract),
            Err(RegistryError::EmptyTransitionConditions)
        );
        let mut contract = transition();
        contract.conditions = vec![id("z-time"), id("z-time")];
        assert_eq!(
            registry.register_transition(contract),
            Err(RegistryError::DuplicateTransitionCondition(id("z-time")))
        );
        let mut contract = transition();
        contract.conditions = vec![VersionedId::new("z-time", 2).unwrap()];
        assert_eq!(
            registry.register_transition(contract.clone()),
            Err(RegistryError::TransitionConditionUnresolved(
                contract.conditions[0].clone()
            ))
        );
        for omitted in ["z-time", "a-source", "b-target"] {
            let mut contract = transition();
            contract
                .conditions
                .retain(|reference| reference != &id(omitted));
            assert_eq!(
                registry.register_transition(contract),
                Err(RegistryError::MissingTransitionRequirement)
            );
        }
    }

    #[test]
    fn transition_order_is_explicit_and_registered_behavior_cannot_be_replaced() {
        let mut registry = registry();
        registry
            .register_state(state("state", CriterionOrder::Unordered))
            .unwrap();
        let mut ordered = transition();
        ordered.condition_order = CriterionOrder::Ordered;
        registry.register_transition(ordered.clone()).unwrap();
        assert_eq!(
            registry.transition(&id("transition")).unwrap().conditions,
            ordered.conditions
        );
        let mut unordered = transition();
        unordered.reference = id("unordered-transition");
        registry.register_transition(unordered).unwrap();
        assert_eq!(
            registry
                .transition(&id("unordered-transition"))
                .unwrap()
                .conditions,
            vec![id("a-source"), id("b-target"), id("z-time")]
        );
        let mut replacement = ordered.clone();
        replacement.conditions.reverse();
        assert_eq!(
            registry.register_transition(replacement),
            Err(RegistryError::DuplicateReference(id("transition")))
        );
        assert_eq!(registry.transition(&id("transition")), Some(&ordered));
        assert_eq!(
            registry.register_transition_condition(TransitionConditionContract {
                reference: id("a-source"),
                kind: TransitionConditionKind::TargetCriteriaPass,
            }),
            Err(RegistryError::DuplicateReference(id("a-source")))
        );
        assert_eq!(
            registry.transition_condition(&id("a-source")).unwrap().kind,
            TransitionConditionKind::SourceCriteriaPass
        );
    }

    #[test]
    fn evidence_sources_must_exist_and_match_the_state_or_transition_domain() {
        let mut registry = registry();
        assert_eq!(
            registry.register_transition_condition(TransitionConditionContract {
                reference: id("future"),
                kind: TransitionConditionKind::ObservationAfterTarget {
                    source: VersionedId::new("source", 2).unwrap()
                },
            }),
            Err(RegistryError::SourceContractUnresolved(
                VersionedId::new("source", 2).unwrap()
            ))
        );
        registry
            .register_source(SourceContract {
                reference: id("other-source"),
                schema: id("schema"),
                context: DomainContext::new("other-domain", id("ticks")).unwrap(),
                unit: "count".into(),
            })
            .unwrap();
        registry
            .register_state_criterion(StateCriterionContract {
                reference: id("other-future"),
                kind: StateCriterionKind::ObservationAfter {
                    source: id("other-source"),
                },
            })
            .unwrap();
        let mut invalid = state("invalid-state", CriterionOrder::Unordered);
        invalid.criteria = vec![id("other-future")];
        assert_eq!(
            registry.register_state(invalid),
            Err(RegistryError::DynamicsContextMismatch)
        );
        registry
            .register_state(state("state", CriterionOrder::Unordered))
            .unwrap();
        registry
            .register_transition_condition(TransitionConditionContract {
                reference: id("other-future"),
                kind: TransitionConditionKind::ObservationAfterTarget {
                    source: id("other-source"),
                },
            })
            .unwrap();
        let mut invalid = transition();
        invalid.conditions.push(id("other-future"));
        assert_eq!(
            registry.register_transition(invalid),
            Err(RegistryError::DynamicsContextMismatch)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::structure::{MemberType, RelationContext, TemporalScope};
    use alloc::vec;

    fn reference(id: &str, version: u32) -> VersionedId {
        VersionedId::new(id, version).unwrap()
    }

    fn source(schema: VersionedId, unit: &str) -> SourceContract {
        SourceContract {
            reference: reference("source", 1),
            schema,
            context: DomainContext::new("test-domain", reference("ticks", 1)).unwrap(),
            unit: unit.into(),
        }
    }

    fn relation_context() -> RelationContext {
        RelationContext::new(
            DomainContext::new("test-domain", reference("ticks", 1)).unwrap(),
            TemporalScope::new(0, 100).unwrap(),
            reference("frame", 1),
            reference("noncausal", 1),
        )
    }

    fn relation(kind: RelationKind) -> RelationContract {
        RelationContract {
            reference: reference("relation", 1),
            kind,
            context: relation_context(),
        }
    }

    fn composition(constraints: alloc::vec::Vec<VersionedId>) -> CompositionContract {
        CompositionContract {
            reference: reference("composition", 1),
            member_order: MemberOrder::Ordered,
            member_type: MemberType::Observations,
            constraints,
            context: relation_context(),
        }
    }

    #[test]
    fn references_and_contexts_require_explicit_nonempty_values() {
        assert_eq!(
            VersionedId::new(" \n", 1),
            Err(RegistryError::EmptyIdentifier)
        );
        assert_eq!(
            VersionedId::new("source", 0),
            Err(RegistryError::InvalidVersion)
        );
        assert_eq!(
            DomainContext::new(" ", reference("ticks", 1)),
            Err(RegistryError::EmptyDomain)
        );
    }

    #[test]
    fn unresolved_versions_do_not_fall_back_to_another_registered_version() {
        let mut registry = Registry::new();
        let schema = reference("scalar-i64-le", 1);
        registry
            .register_schema(SchemaContract {
                reference: schema.clone(),
                format: ScalarSchema::SignedI64Le,
            })
            .unwrap();
        let missing_version = reference("scalar-i64-le", 2);
        assert_eq!(
            registry.register_source(source(missing_version.clone(), "count")),
            Err(RegistryError::SchemaUnresolved(missing_version.clone()))
        );
        assert!(registry.schema(&missing_version).is_none());
        assert!(registry.source(&reference("source", 1)).is_none());
        registry.register_source(source(schema, "count")).unwrap();
    }

    #[test]
    fn source_registration_checks_units_and_preserves_the_registered_contract() {
        let mut registry = Registry::new();
        let schema = reference("scalar-i64-le", 1);
        registry
            .register_schema(SchemaContract {
                reference: schema.clone(),
                format: ScalarSchema::SignedI64Le,
            })
            .unwrap();
        assert_eq!(
            registry.register_source(source(schema.clone(), "\t")),
            Err(RegistryError::EmptyUnit)
        );
        let original = source(schema.clone(), "count");
        registry.register_source(original.clone()).unwrap();
        assert_eq!(
            registry.register_source(source(schema, "price")),
            Err(RegistryError::DuplicateReference(reference("source", 1)))
        );
        assert_eq!(registry.source(&reference("source", 1)), Some(&original));
    }

    #[test]
    fn operator_registration_does_not_replace_existing_behavior() {
        let mut registry = Registry::new();
        let original = OperatorContract {
            reference: reference("operator", 1),
            kind: OperatorKind::Scalar,
        };
        registry.register_operator(original.clone()).unwrap();
        assert_eq!(
            registry.register_operator(OperatorContract {
                reference: original.reference.clone(),
                kind: OperatorKind::Difference,
            }),
            Err(RegistryError::DuplicateReference(
                original.reference.clone()
            ))
        );
        assert_eq!(registry.operator(&original.reference), Some(&original));
    }

    #[test]
    fn temporal_scope_checks_boundaries_and_rejects_inverted_intervals() {
        assert_eq!(
            TemporalScope::new(2, 1),
            Err(RegistryError::InvalidTemporalScope)
        );
        let scope = TemporalScope::new(10, 20).unwrap();
        assert!(scope.contains(10, 20));
        assert!(scope.contains(15, 15));
        assert!(!scope.contains(9, 20));
        assert!(!scope.contains(10, 21));
        assert!(!scope.contains(18, 12));
    }

    #[test]
    fn relation_registration_preserves_versions_and_validates_adjacency() {
        let mut registry = Registry::new();
        assert_eq!(
            registry.register_relation(relation(RelationKind::Adjacent {
                step: 0,
                symmetric: true
            })),
            Err(RegistryError::InvalidAdjacencyStep)
        );
        assert!(registry.relation(&reference("relation", 1)).is_none());
        let original = relation(RelationKind::Precedes);
        registry.register_relation(original.clone()).unwrap();
        assert_eq!(
            registry.register_relation(relation(RelationKind::TemporalDistance)),
            Err(RegistryError::DuplicateReference(reference("relation", 1)))
        );
        assert_eq!(registry.relation(&original.reference), Some(&original));
        assert!(registry.relation(&reference("relation", 2)).is_none());
    }

    #[test]
    fn constraint_registration_resolves_exact_relation_versions_and_direction() {
        let mut registry = Registry::new();
        registry
            .register_relation(relation(RelationKind::Precedes))
            .unwrap();
        assert_eq!(
            registry.register_constraint(ConstraintContract {
                reference: reference("chain", 1),
                kind: ConstraintKind::OrderedChain {
                    relation: reference("relation", 2)
                },
            }),
            Err(RegistryError::RelationContractUnresolved(reference(
                "relation", 2
            )))
        );
        for (name, kind) in [
            (
                "symmetric",
                RelationKind::Adjacent {
                    step: 1,
                    symmetric: true,
                },
            ),
            ("ratio", RelationKind::Proportion),
        ] {
            let relation_ref = reference(name, 1);
            registry
                .register_relation(RelationContract {
                    reference: relation_ref.clone(),
                    kind,
                    context: relation_context(),
                })
                .unwrap();
            assert_eq!(
                registry.register_constraint(ConstraintContract {
                    reference: reference(name, 1),
                    kind: ConstraintKind::OrderedChain {
                        relation: relation_ref.clone()
                    },
                }),
                Err(RegistryError::IncompatibleConstraintOrdering)
            );
            registry
                .register_constraint(ConstraintContract {
                    reference: reference(name, 1),
                    kind: ConstraintKind::RequiresRelation {
                        relation: relation_ref,
                    },
                })
                .unwrap();
        }
    }

    #[test]
    fn invalid_cardinality_and_constraint_replacement_are_rejected() {
        let mut registry = Registry::new();
        for (min, max) in [(0, 2), (3, 2)] {
            assert_eq!(
                registry.register_constraint(ConstraintContract {
                    reference: reference("size", 1),
                    kind: ConstraintKind::Cardinality { min, max },
                }),
                Err(RegistryError::InvalidCardinality)
            );
        }
        let original = ConstraintContract {
            reference: reference("size", 1),
            kind: ConstraintKind::Cardinality { min: 1, max: 3 },
        };
        registry.register_constraint(original.clone()).unwrap();
        assert_eq!(
            registry.register_constraint(ConstraintContract {
                reference: original.reference.clone(),
                kind: ConstraintKind::Connected,
            }),
            Err(RegistryError::DuplicateReference(
                original.reference.clone()
            ))
        );
        assert_eq!(registry.constraint(&original.reference), Some(&original));
    }

    #[test]
    fn composition_registration_requires_resolved_unique_constraints() {
        let mut registry = Registry::new();
        assert_eq!(
            registry.register_composition(composition(vec![])),
            Err(RegistryError::EmptyConstraintSet)
        );
        let size = reference("size", 1);
        let units = reference("units", 1);
        registry
            .register_constraint(ConstraintContract {
                reference: size.clone(),
                kind: ConstraintKind::Cardinality { min: 1, max: 3 },
            })
            .unwrap();
        registry
            .register_constraint(ConstraintContract {
                reference: units.clone(),
                kind: ConstraintKind::SameUnits,
            })
            .unwrap();
        assert_eq!(
            registry.register_composition(composition(vec![size.clone(), size.clone()])),
            Err(RegistryError::DuplicateConstraint(size.clone()))
        );
        assert_eq!(
            registry.register_composition(composition(vec![reference("size", 2)])),
            Err(RegistryError::ConstraintUnresolved(reference("size", 2)))
        );
        registry
            .register_composition(composition(vec![units.clone(), size.clone()]))
            .unwrap();
        let original = registry
            .composition(&reference("composition", 1))
            .unwrap()
            .clone();
        assert_eq!(original.constraints, vec![size, units]);
        assert_eq!(
            registry.register_composition(composition(vec![])),
            Err(RegistryError::DuplicateReference(
                original.reference.clone()
            ))
        );
        assert_eq!(registry.composition(&original.reference), Some(&original));
    }

    #[test]
    fn ordered_chain_cannot_be_assigned_to_unordered_composition_members() {
        let mut registry = Registry::new();
        registry
            .register_relation(relation(RelationKind::Precedes))
            .unwrap();
        let chain = reference("chain", 1);
        registry
            .register_constraint(ConstraintContract {
                reference: chain.clone(),
                kind: ConstraintKind::OrderedChain {
                    relation: reference("relation", 1),
                },
            })
            .unwrap();
        let mut contract = composition(vec![chain]);
        contract.member_order = MemberOrder::Unordered;
        assert_eq!(
            registry.register_composition(contract),
            Err(RegistryError::IncompatibleConstraintOrdering)
        );
    }
}

impl Registry {
    pub fn register_conformance_criterion(
        &mut self,
        criterion: ConformanceCriterionContract,
    ) -> Result<(), RegistryError> {
        if let ConformanceCriterionKind::MemberCount { min, max } = criterion.kind {
            if min > max {
                return Err(RegistryError::InvalidCardinality);
            }
        }
        insert(
            &mut self.conformance_criteria,
            criterion.reference.clone(),
            criterion,
        )
    }
    pub fn register_conformance(
        &mut self,
        mut contract: ConformanceContract,
    ) -> Result<(), RegistryError> {
        if self.conformance.contains_key(&contract.reference) {
            return Err(RegistryError::DuplicateReference(contract.reference));
        }
        if contract.criteria.is_empty() {
            return Err(RegistryError::EmptyConformanceCriteria);
        }
        let mut seen = BTreeSet::new();
        let mut target = false;
        let mut lineage = false;
        for reference in &contract.criteria {
            if !seen.insert(reference) {
                return Err(RegistryError::DuplicateConformanceCriterion(
                    reference.clone(),
                ));
            }
            let criterion = self
                .conformance_criteria
                .get(reference)
                .ok_or_else(|| RegistryError::ConformanceCriterionUnresolved(reference.clone()))?;
            match criterion.kind {
                ConformanceCriterionKind::TargetExists => target = true,
                ConformanceCriterionKind::LineageComplete => lineage = true,
                ConformanceCriterionKind::MemberCount { .. }
                    if contract.target_type != TargetType::Composition =>
                {
                    return Err(RegistryError::InvalidConformanceDefinition)
                }
                _ => (),
            }
        }
        if !(target && lineage) {
            return Err(RegistryError::InvalidConformanceDefinition);
        }
        if contract.criterion_order == CriterionOrder::Unordered {
            contract.criteria.sort();
        }
        insert(&mut self.conformance, contract.reference.clone(), contract)
    }
    pub fn conformance(&self, reference: &VersionedId) -> Option<&ConformanceContract> {
        self.conformance.get(reference)
    }
    pub fn conformance_criterion(
        &self,
        reference: &VersionedId,
    ) -> Option<&ConformanceCriterionContract> {
        self.conformance_criteria.get(reference)
    }
}

impl Registry {
    pub fn register_projection(&mut self, profile: ProjectionProfile) -> Result<(), RegistryError> {
        if self.projections.contains_key(&profile.reference) {
            return Err(RegistryError::DuplicateReference(profile.reference));
        }
        if profile.schema.id() != "TEST-REP-01" || profile.schema.version() != 1 {
            return Err(RegistryError::ProjectionContractMismatch);
        }
        let contract = self.conformance(&profile.conformance).ok_or_else(|| {
            RegistryError::ProjectionConformanceUnresolved(profile.conformance.clone())
        })?;
        if contract.target_type != profile.target_type || contract.context != profile.context {
            return Err(RegistryError::ProjectionContractMismatch);
        }
        insert(&mut self.projections, profile.reference.clone(), profile)
    }
    pub fn projection(&self, reference: &VersionedId) -> Option<&ProjectionProfile> {
        self.projections.get(reference)
    }
}
