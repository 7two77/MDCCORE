use alloc::vec::Vec;
use crate::provenance::Provenance;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConformanceResult {
    Pass,
    Fail,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleStatus {
    Candidate,
    Conformant,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConformanceError {
    TargetUnresolved,
    ContractUnresolved,
    InvalidTargetType,
    ProvenanceIncomplete,
    FailedCriterion,
    UnresolvedCriterion,
}

pub struct Evaluation {
    pub target_id: u64,
    pub contract_id: &'static str,
    pub result: ConformanceResult,
    pub status: LifecycleStatus,
    pub criteria: Vec<CriterionResult>,
    pub error: Option<ConformanceError>,
    pub provenance: Provenance,
}

pub struct CriterionResult {
    pub id: &'static str,
    pub result: ConformanceResult,
}