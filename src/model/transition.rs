use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::contract::dynamics::{
    CriterionOrder, DynamicsContext, TransitionConditionContract, TransitionConditionKind,
    TransitionContract,
};
use crate::contract::encoding::{put_dynamics_context, put_object_ref, put_reference};
use crate::contract::evaluation::{aggregate_results, ConformanceResult, LifecycleStatus};
use crate::contract::registry::VersionedId;
use crate::contract::structure::TemporalScope;
use crate::identity::{CanonicalSemanticBasis, IdentityAdapter};
use crate::model::state::{observation_after, AdmittedState, ObservationFact};
use crate::provenance::{DerivationProvenance, ObjectRef};

/// Legacy raw transition retained for compatibility; it is not admitted.
pub struct Transition {
    pub id: u64,
    pub source_state: u64,
    pub target_state: u64,
    pub condition_met: bool,
    pub status: LifecycleStatus,
}

/// The actual result of a registered transition condition and its witnesses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionConditionResult {
    contract: TransitionConditionContract,
    result: ConformanceResult,
    evidence: Vec<ObjectRef>,
}

impl TransitionConditionResult {
    pub fn reference(&self) -> &VersionedId {
        &self.contract.reference
    }

    pub fn kind(&self) -> &TransitionConditionKind {
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
pub enum TransitionError {
    SameState,
    SourceContractMismatch,
    TargetContractMismatch,
    IncompatibleContext,
    OutsideTemporalScope,
    HorizonBeforeInput,
    HorizonOutsideScope,
    ConditionSetMismatch,
    FailedConditions(Vec<TransitionConditionResult>),
}

/// Pipeline resolves both States and the extent of all facts in their lineage.
/// Evidence bounds can extend beyond the State validity intervals.
pub(crate) struct ResolvedStates {
    pub source: AdmittedState,
    pub target: AdmittedState,
    pub source_bounds: TemporalScope,
    pub target_bounds: TemporalScope,
}

/// A directed construction with explicit conditions. Candidate construction
/// neither establishes causal truth nor executes a conformance lifecycle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedTransition {
    id: u64,
    contract: VersionedId,
    source_state: ObjectRef,
    target_state: ObjectRef,
    condition_order: CriterionOrder,
    conditions: Vec<TransitionConditionResult>,
    conditions_result: ConformanceResult,
    horizon: Option<u64>,
    context: DynamicsContext,
    provenance: DerivationProvenance,
}

impl AdmittedTransition {
    pub(crate) fn derive(
        contract: &TransitionContract,
        input: &ResolvedStates,
        horizon: Option<u64>,
        mut conditions: Vec<TransitionConditionContract>,
        facts: &[ObservationFact],
    ) -> Result<Self, TransitionError> {
        if input.source.id() == input.target.id() {
            return Err(TransitionError::SameState);
        }
        if input.source.contract() != &contract.source_state {
            return Err(TransitionError::SourceContractMismatch);
        }
        if input.target.contract() != &contract.target_state {
            return Err(TransitionError::TargetContractMismatch);
        }
        if input.source.context() != &contract.context
            || input.target.context() != &contract.context
        {
            return Err(TransitionError::IncompatibleContext);
        }
        let scope = contract.context.frame().scope();
        if !scope.contains(input.source_bounds.start(), input.source_bounds.end())
            || !scope.contains(input.target_bounds.start(), input.target_bounds.end())
        {
            return Err(TransitionError::OutsideTemporalScope);
        }
        if let Some(horizon) = horizon {
            if horizon < input.source_bounds.end().max(input.target_bounds.end()) {
                return Err(TransitionError::HorizonBeforeInput);
            }
            if !scope.contains(horizon, horizon) {
                return Err(TransitionError::HorizonOutsideScope);
            }
        }
        if contract.condition_order == CriterionOrder::Unordered {
            conditions.sort_by(|left, right| left.reference.cmp(&right.reference));
        }
        let unique: BTreeSet<_> = conditions
            .iter()
            .map(|condition| &condition.reference)
            .collect();
        if conditions.is_empty()
            || unique.len() != conditions.len()
            || !conditions
                .iter()
                .map(|condition| &condition.reference)
                .eq(contract.conditions.iter())
        {
            return Err(TransitionError::ConditionSetMismatch);
        }
        let evaluated: Vec<_> = conditions
            .into_iter()
            .map(|condition| {
                let (result, evidence) = match &condition.kind {
                    TransitionConditionKind::TemporalOrder { strict } => {
                        let source_end = input.source.validity_interval().end();
                        let target_start = input.target.validity_interval().start();
                        let ordered = if *strict {
                            source_end < target_start
                        } else {
                            source_end <= target_start
                        };
                        (
                            if ordered {
                                ConformanceResult::Pass
                            } else {
                                ConformanceResult::Fail
                            },
                            Vec::new(),
                        )
                    }
                    TransitionConditionKind::SourceCriteriaPass => {
                        (input.source.criteria_result(), Vec::new())
                    }
                    TransitionConditionKind::TargetCriteriaPass => {
                        (input.target.criteria_result(), Vec::new())
                    }
                    TransitionConditionKind::ObservationAfterTarget { source } => {
                        observation_after(
                            source,
                            input.target.validity_interval().end(),
                            horizon,
                            scope,
                            facts,
                        )
                    }
                };
                TransitionConditionResult {
                    contract: condition,
                    result,
                    evidence,
                }
            })
            .collect();
        let results: Vec<_> = evaluated.iter().map(|condition| condition.result).collect();
        let conditions_result =
            aggregate_results(&results).map_err(|_| TransitionError::ConditionSetMismatch)?;
        if conditions_result == ConformanceResult::Fail {
            return Err(TransitionError::FailedConditions(evaluated));
        }
        let source_state = ObjectRef::State(input.source.id());
        let target_state = ObjectRef::State(input.target.id());
        let mut inputs = alloc::vec![source_state.clone(), target_state.clone()];
        let witnesses: BTreeSet<_> = evaluated
            .iter()
            .flat_map(|condition| condition.evidence.iter().cloned())
            .collect();
        inputs.extend(witnesses);
        let provenance = DerivationProvenance::new(contract.reference.clone(), inputs);
        let mut transition = Self {
            id: 0,
            contract: contract.reference.clone(),
            source_state,
            target_state,
            condition_order: contract.condition_order,
            conditions: evaluated,
            conditions_result,
            horizon,
            context: contract.context.clone(),
            provenance,
        };
        transition.id = IdentityAdapter::derive_identity_from_basis(&transition.canonical_basis());
        Ok(transition)
    }

    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn contract(&self) -> &VersionedId {
        &self.contract
    }
    pub fn source_state(&self) -> &ObjectRef {
        &self.source_state
    }
    pub fn target_state(&self) -> &ObjectRef {
        &self.target_state
    }
    pub fn conditions(&self) -> &[TransitionConditionResult] {
        &self.conditions
    }
    pub fn conditions_result(&self) -> ConformanceResult {
        self.conditions_result
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

impl CanonicalSemanticBasis for AdmittedTransition {
    fn canonical_basis(&self) -> Vec<u8> {
        let mut bytes = b"MDC-SCALAR-001/transition/v1\0".to_vec();
        put_reference(&mut bytes, &self.contract);
        put_object_ref(&mut bytes, &self.source_state);
        put_object_ref(&mut bytes, &self.target_state);
        bytes.push(match self.condition_order {
            CriterionOrder::Ordered => 0,
            CriterionOrder::Unordered => 1,
        });
        bytes.extend_from_slice(&(self.conditions.len() as u64).to_le_bytes());
        for condition in &self.conditions {
            put_reference(&mut bytes, condition.reference());
            match condition.kind() {
                TransitionConditionKind::TemporalOrder { strict } => {
                    bytes.push(0);
                    bytes.push(u8::from(*strict));
                }
                TransitionConditionKind::SourceCriteriaPass => bytes.push(1),
                TransitionConditionKind::TargetCriteriaPass => bytes.push(2),
                TransitionConditionKind::ObservationAfterTarget { source } => {
                    bytes.push(3);
                    put_reference(&mut bytes, source);
                }
            }
            put_result(&mut bytes, condition.result());
            put_references(&mut bytes, condition.evidence());
        }
        put_result(&mut bytes, self.conditions_result);
        match self.horizon {
            None => bytes.push(0),
            Some(horizon) => {
                bytes.push(1);
                bytes.extend_from_slice(&horizon.to_le_bytes());
            }
        }
        put_dynamics_context(&mut bytes, &self.context);
        put_reference(&mut bytes, self.provenance.contract());
        put_references(&mut bytes, self.provenance.inputs());
        bytes.push(0); // Candidate; construction does not run CB conformance.
        bytes
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
