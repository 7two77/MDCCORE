use crate::provenance::Provenance;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConformanceResult {
    Pass,
    Fail,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregationError {
    EmptyCriteria,
}

/// Aggregates explicit criterion results with FAIL > UNRESOLVED > PASS.
///
/// An empty criterion set is rejected rather than silently producing PASS.
/// This function does not resolve contracts or infer missing criterion results.
pub fn aggregate_results(
    results: &[ConformanceResult],
) -> Result<ConformanceResult, AggregationError> {
    if results.is_empty() {
        return Err(AggregationError::EmptyCriteria);
    }
    let mut result = ConformanceResult::Pass;
    for criterion in results {
        match criterion {
            ConformanceResult::Fail => return Ok(ConformanceResult::Fail),
            ConformanceResult::Unresolved => result = ConformanceResult::Unresolved,
            ConformanceResult::Pass => {}
        }
    }
    Ok(result)
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

#[cfg(test)]
mod tests {
    use super::{aggregate_results, AggregationError, ConformanceResult::*};

    #[test]
    fn aggregation_covers_all_pairs_and_is_order_independent() {
        let values = [Pass, Unresolved, Fail];
        for (left_rank, left) in values.iter().enumerate() {
            for (right_rank, right) in values.iter().enumerate() {
                let expected = values[left_rank.max(right_rank)];
                assert_eq!(aggregate_results(&[*left, *right]), Ok(expected));
                assert_eq!(aggregate_results(&[*right, *left]), Ok(expected));
            }
        }
        assert_eq!(aggregate_results(&[Pass, Unresolved, Fail]), Ok(Fail));
        assert_eq!(aggregate_results(&[Fail, Unresolved, Pass]), Ok(Fail));
    }

    #[test]
    fn aggregation_does_not_treat_an_empty_set_as_pass() {
        assert_eq!(aggregate_results(&[]), Err(AggregationError::EmptyCriteria));
    }
}
