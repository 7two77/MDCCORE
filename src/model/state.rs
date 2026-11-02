use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::contract::dynamics::{
    CriterionOrder, DynamicsContext, StateContract, StateCriterionContract, StateCriterionKind,
    StateDefinition,
};
use crate::contract::encoding::{put_dynamics_context, put_object_ref, put_reference};
use crate::contract::evaluation::{aggregate_results, ConformanceResult, LifecycleStatus};
use crate::contract::registry::VersionedId;
use crate::contract::structure::TemporalScope;
use crate::identity::{CanonicalSemanticBasis, IdentityAdapter};
use crate::model::composition::AdmittedComposition;
use crate::provenance::{DerivationProvenance, ObjectRef};

/// Legacy raw state retained for compatibility. It is not an admitted object.
pub struct State {
    pub id: u64,
    pub source_composition: u64,
    pub criteria_met: bool,
    pub status: LifecycleStatus,
}

/// A source observation resolved from the immutable admitted graph by Pipeline.
pub(crate) struct ObservationFact {
    pub reference: ObjectRef,
    pub source: VersionedId,
    pub timestamp: u64,
}

/// A composition and its actual observation interval, resolved by Pipeline.
pub(crate) struct ResolvedComposition {
    pub composition: AdmittedComposition,
    pub interval: TemporalScope,
    pub relation_contracts: Vec<VersionedId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateCriterionResult {
    contract: StateCriterionContract,
    result: ConformanceResult,
    evidence: Vec<ObjectRef>,
}

impl StateCriterionResult {
    pub fn reference(&self) -> &VersionedId {
        &self.contract.reference
    }

    pub fn kind(&self) -> &StateCriterionKind {
        &self.contract.kind
    }

    pub fn result(&self) -> ConformanceResult {
        self.result
    }

    pub fn evidence(&self) -> &[ObjectRef] {
        &self.evidence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateError {
    CompositionContractMismatch,
    IncompatibleContext,
    OutsideTemporalScope,
    HorizonBeforeInput,
    HorizonOutsideScope,
    CriterionSetMismatch,
    FailedCriteria(Vec<StateCriterionResult>),
}

/// A contract-defined abstraction of an admitted composition.
/// A complete or unresolved derivation stays Candidate until separate conformance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedState {
    id: u64,
    contract: VersionedId,
    definition: StateDefinition,
    source_composition: ObjectRef,
    criterion_order: CriterionOrder,
    criteria: Vec<StateCriterionResult>,
    criteria_result: ConformanceResult,
    validity_interval: TemporalScope,
    horizon: Option<u64>,
    context: DynamicsContext,
    provenance: DerivationProvenance,
}

impl AdmittedState {
    pub(crate) fn derive(
        contract: &StateContract,
        input: &ResolvedComposition,
        horizon: Option<u64>,
        mut criteria: Vec<StateCriterionContract>,
        observations: &[ObservationFact],
    ) -> Result<Self, StateError> {
        if input.composition.contract() != &contract.composition {
            return Err(StateError::CompositionContractMismatch);
        }
        if input.composition.context() != contract.context.frame() {
            return Err(StateError::IncompatibleContext);
        }
        let scope = contract.context.frame().scope();
        if !scope.contains(input.interval.start(), input.interval.end()) {
            return Err(StateError::OutsideTemporalScope);
        }
        if let Some(horizon) = horizon {
            if horizon < input.interval.end() {
                return Err(StateError::HorizonBeforeInput);
            }
            if !scope.contains(horizon, horizon) {
                return Err(StateError::HorizonOutsideScope);
            }
        }

        if contract.criterion_order == CriterionOrder::Unordered {
            criteria.sort_by(|left, right| left.reference.cmp(&right.reference));
        }
        let mut seen = BTreeSet::new();
        if criteria.is_empty()
            || !criteria
                .iter()
                .map(|criterion| &criterion.reference)
                .eq(contract.criteria.iter())
            || criteria
                .iter()
                .any(|criterion| !seen.insert(criterion.reference.clone()))
        {
            return Err(StateError::CriterionSetMismatch);
        }

        // Resolution is a precondition, not an implicit Fail or default Pass.
        for criterion in &criteria {
            if let StateCriterionKind::ConstraintPassed { constraint } = &criterion.kind {
                if !input
                    .composition
                    .constraints()
                    .iter()
                    .any(|result| result.reference() == constraint)
                {
                    return Err(StateError::CriterionSetMismatch);
                }
            }
        }

        let evaluated: Vec<_> = criteria
            .into_iter()
            .map(|criterion| {
                let (result, evidence) =
                    evaluate_criterion(&criterion.kind, input, horizon, scope, observations);
                StateCriterionResult {
                    contract: criterion,
                    result,
                    evidence,
                }
            })
            .collect();
        let results: Vec<_> = evaluated.iter().map(|criterion| criterion.result).collect();
        let criteria_result =
            aggregate_results(&results).map_err(|_| StateError::CriterionSetMismatch)?;
        if criteria_result == ConformanceResult::Fail {
            return Err(StateError::FailedCriteria(evaluated));
        }

        let source_composition = ObjectRef::Composition(input.composition.id());
        let witnesses: BTreeSet<_> = evaluated
            .iter()
            .flat_map(|criterion| criterion.evidence.iter().cloned())
            .collect();
        let mut inputs = alloc::vec![source_composition.clone()];
        inputs.extend(witnesses);
        let provenance = DerivationProvenance::new(contract.reference.clone(), inputs);
        let mut state = Self {
            id: 0,
            contract: contract.reference.clone(),
            definition: contract.definition,
            source_composition,
            criterion_order: contract.criterion_order,
            criteria: evaluated,
            criteria_result,
            // Confirmation witnesses are later facts, not part of this interval.
            validity_interval: input.interval.clone(),
            horizon,
            context: contract.context.clone(),
            provenance,
        };
        state.id = IdentityAdapter::derive_identity_from_basis(&state.canonical_basis());
        Ok(state)
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn contract(&self) -> &VersionedId {
        &self.contract
    }

    pub fn definition(&self) -> StateDefinition {
        self.definition
    }

    pub fn source_composition(&self) -> &ObjectRef {
        &self.source_composition
    }

    pub fn criteria(&self) -> &[StateCriterionResult] {
        &self.criteria
    }

    pub fn criteria_result(&self) -> ConformanceResult {
        self.criteria_result
    }

    pub fn validity_interval(&self) -> &TemporalScope {
        &self.validity_interval
    }

    pub fn horizon(&self) -> Option<u64> {
        self.horizon
    }

    pub fn context(&self) -> &DynamicsContext {
        &self.context
    }

    pub fn provenance(&self) -> &DerivationProvenance {
        &self.provenance
    }

    pub fn status(&self) -> LifecycleStatus {
        LifecycleStatus::Candidate
    }
}

fn evaluate_criterion(
    kind: &StateCriterionKind,
    input: &ResolvedComposition,
    horizon: Option<u64>,
    scope: &TemporalScope,
    observations: &[ObservationFact],
) -> (ConformanceResult, Vec<ObjectRef>) {
    let passed = match kind {
        StateCriterionKind::Cardinality { min, max } => {
            let count = input.composition.members().len() as u128;
            u128::from(*min) <= count && count <= u128::from(*max)
        }
        StateCriterionKind::HasRelation { relation } => input.relation_contracts.contains(relation),
        StateCriterionKind::ConstraintPassed { constraint } => {
            input.composition.constraints().iter().any(|result| {
                result.reference() == constraint && result.result() == ConformanceResult::Pass
            })
        }
        StateCriterionKind::MaxTemporalSpan { ticks } => {
            input.interval.end() - input.interval.start() <= *ticks
        }
        StateCriterionKind::ObservationAfter { source } => {
            return observation_after(source, input.interval.end(), horizon, scope, observations);
        }
    };
    (
        if passed {
            ConformanceResult::Pass
        } else {
            ConformanceResult::Fail
        },
        Vec::new(),
    )
}

/// Resolve an open-world availability criterion from real admitted facts.
/// Missing knowledge stays Unresolved; absence never fabricates a negative fact.
pub(crate) fn observation_after(
    source: &VersionedId,
    after: u64,
    horizon: Option<u64>,
    scope: &TemporalScope,
    facts: &[ObservationFact],
) -> (ConformanceResult, Vec<ObjectRef>) {
    let Some(horizon) = horizon else {
        return (ConformanceResult::Unresolved, Vec::new());
    };
    let witnesses: BTreeSet<_> = facts
        .iter()
        .filter(|fact| {
            &fact.source == source
                && matches!(fact.reference, ObjectRef::Observation(_))
                && after < fact.timestamp
                && fact.timestamp <= horizon
                && scope.contains(fact.timestamp, fact.timestamp)
        })
        .map(|fact| fact.reference.clone())
        .collect();
    if witnesses.is_empty() {
        (ConformanceResult::Unresolved, Vec::new())
    } else {
        (ConformanceResult::Pass, witnesses.into_iter().collect())
    }
}

impl CanonicalSemanticBasis for AdmittedState {
    fn canonical_basis(&self) -> Vec<u8> {
        let mut bytes = b"MDC-SCALAR-001/state/v1\0".to_vec();
        put_reference(&mut bytes, &self.contract);
        bytes.push(match self.definition {
            StateDefinition::CompositionAbstraction => 0,
        });
        put_object_ref(&mut bytes, &self.source_composition);
        bytes.push(match self.criterion_order {
            CriterionOrder::Ordered => 0,
            CriterionOrder::Unordered => 1,
        });
        bytes.extend_from_slice(&(self.criteria.len() as u64).to_le_bytes());
        for criterion in &self.criteria {
            put_reference(&mut bytes, criterion.reference());
            put_criterion_kind(&mut bytes, criterion.kind());
            put_result(&mut bytes, criterion.result());
            put_references(&mut bytes, criterion.evidence());
        }
        put_result(&mut bytes, self.criteria_result);
        bytes.extend_from_slice(&self.validity_interval.start().to_le_bytes());
        bytes.extend_from_slice(&self.validity_interval.end().to_le_bytes());
        match self.horizon {
            Some(horizon) => {
                bytes.push(1);
                bytes.extend_from_slice(&horizon.to_le_bytes());
            }
            None => bytes.push(0),
        }
        put_dynamics_context(&mut bytes, &self.context);
        put_reference(&mut bytes, self.provenance.contract());
        put_references(&mut bytes, self.provenance.inputs());
        bytes.push(0); // Candidate; separate conformance is not executed here.
        bytes
    }
}

fn put_criterion_kind(bytes: &mut Vec<u8>, kind: &StateCriterionKind) {
    match kind {
        StateCriterionKind::Cardinality { min, max } => {
            bytes.push(0);
            bytes.extend_from_slice(&min.to_le_bytes());
            bytes.extend_from_slice(&max.to_le_bytes());
        }
        StateCriterionKind::HasRelation { relation } => {
            bytes.push(1);
            put_reference(bytes, relation);
        }
        StateCriterionKind::ConstraintPassed { constraint } => {
            bytes.push(2);
            put_reference(bytes, constraint);
        }
        StateCriterionKind::MaxTemporalSpan { ticks } => {
            bytes.push(3);
            bytes.extend_from_slice(&ticks.to_le_bytes());
        }
        StateCriterionKind::ObservationAfter { source } => {
            bytes.push(4);
            put_reference(bytes, source);
        }
    }
}

fn put_result(bytes: &mut Vec<u8>, result: ConformanceResult) {
    bytes.push(match result {
        ConformanceResult::Pass => 0,
        ConformanceResult::Fail => 1,
        ConformanceResult::Unresolved => 2,
    });
}

fn put_references(bytes: &mut Vec<u8>, references: &[ObjectRef]) {
    bytes.extend_from_slice(&(references.len() as u64).to_le_bytes());
    for reference in references {
        put_object_ref(bytes, reference);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(name: &str, version: u32) -> VersionedId {
        VersionedId::new(name, version).unwrap()
    }

    fn fact(id: u64, source: VersionedId, timestamp: u64) -> ObservationFact {
        ObservationFact {
            reference: ObjectRef::Observation(id),
            source,
            timestamp,
        }
    }

    #[test]
    fn missing_or_out_of_horizon_witnesses_remain_unresolved() {
        let source = reference("source", 1);
        let facts = [fact(1, source.clone(), 30)];
        let scope = TemporalScope::new(0, 100).unwrap();
        assert_eq!(
            observation_after(&source, 20, None, &scope, &facts),
            (ConformanceResult::Unresolved, Vec::new())
        );
        assert_eq!(
            observation_after(&source, 20, Some(29), &scope, &facts),
            (ConformanceResult::Unresolved, Vec::new())
        );
        assert_eq!(
            observation_after(&source, 20, Some(30), &scope, &facts),
            (
                ConformanceResult::Pass,
                alloc::vec![ObjectRef::Observation(1)]
            )
        );
    }

    #[test]
    fn witness_snapshot_is_source_version_scoped_sorted_and_deduplicated() {
        let source = reference("source", 1);
        let facts = [
            fact(3, source.clone(), 40),
            fact(2, source.clone(), 30),
            fact(2, source.clone(), 30),
            fact(7, reference("source", 2), 30),
            fact(8, source.clone(), 61),
            fact(9, source.clone(), 20),
        ];
        let scope = TemporalScope::new(0, 60).unwrap();
        assert_eq!(
            observation_after(&source, 20, Some(100), &scope, &facts),
            (
                ConformanceResult::Pass,
                alloc::vec![ObjectRef::Observation(2), ObjectRef::Observation(3)]
            )
        );
    }
}
