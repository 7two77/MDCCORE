use alloc::string::String;
use alloc::vec::Vec;

use crate::contract::encoding::{put_object_ref, put_reference, put_relation_context};
use crate::contract::evaluation::LifecycleStatus;
use crate::contract::registry::{DomainContext, VersionedId};
use crate::contract::structure::{RelationContext, RelationContract, RelationKind};
use crate::identity::{CanonicalSemanticBasis, IdentityAdapter};
use crate::provenance::{DerivationProvenance, ObjectRef};

/// Legacy raw relation retained for compatibility. It is not an admitted edge.
pub struct Relation {
    pub id: u64,
    pub kind: &'static str,
    pub left_ref: u64,
    pub right_ref: u64,
}

/// A deterministic attribute computed by the registered relation contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelationValue {
    Established,
    TemporalDistance(i128),
    /// An exact signed ratio, without reduction or lossy floating conversion.
    Proportion {
        numerator: i128,
        denominator: i128,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelationError {
    InvalidInputType(ObjectRef),
    IncompatibleContext,
    IncompatibleUnits,
    OutsideTemporalScope,
    TemporalOrder,
    NotAdjacent,
    ZeroDenominator,
}

/// Data resolved by Pipeline from an already admitted scalar object.
pub(crate) struct ResolvedRelationInput {
    pub reference: ObjectRef,
    pub context: DomainContext,
    pub unit: String,
    pub timestamp: Option<u64>,
    pub value: Option<i128>,
    pub temporal_bounds: (u64, u64),
}

/// An immutable relation constructed under a registered kind and context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedRelation {
    id: u64,
    contract: VersionedId,
    kind: RelationKind,
    left_ref: ObjectRef,
    right_ref: ObjectRef,
    context: RelationContext,
    value: RelationValue,
    provenance: DerivationProvenance,
}

impl AdmittedRelation {
    pub(crate) fn derive(
        contract: &RelationContract,
        left: ResolvedRelationInput,
        right: ResolvedRelationInput,
    ) -> Result<Self, RelationError> {
        if left.context != *contract.context.domain() || right.context != *contract.context.domain()
        {
            return Err(RelationError::IncompatibleContext);
        }
        for input in [&left, &right] {
            if !contract
                .context
                .scope()
                .contains(input.temporal_bounds.0, input.temporal_bounds.1)
            {
                return Err(RelationError::OutsideTemporalScope);
            }
        }

        // Normalization applies only when the registered semantics are symmetric.
        // Lineage is built after this normalization, so it has the same ordering.
        let (left, right) = if contract.kind.is_symmetric() && right.reference < left.reference {
            (right, left)
        } else {
            (left, right)
        };
        let value = match &contract.kind {
            RelationKind::Precedes => {
                let (left_time, right_time) = (observation_time(&left)?, observation_time(&right)?);
                if left_time >= right_time {
                    return Err(RelationError::TemporalOrder);
                }
                RelationValue::Established
            }
            RelationKind::TemporalDistance => {
                let (left_time, right_time) = (observation_time(&left)?, observation_time(&right)?);
                RelationValue::TemporalDistance(i128::from(right_time) - i128::from(left_time))
            }
            RelationKind::Adjacent { step, symmetric } => {
                let (left_time, right_time) = (observation_time(&left)?, observation_time(&right)?);
                let adjacent = if *symmetric {
                    left_time.abs_diff(right_time) == *step
                } else {
                    right_time.checked_sub(left_time) == Some(*step)
                };
                if *step == 0 || !adjacent {
                    return Err(RelationError::NotAdjacent);
                }
                RelationValue::Established
            }
            RelationKind::Proportion => {
                let numerator = measurement_value(&left)?;
                let denominator = measurement_value(&right)?;
                if left.unit != right.unit {
                    return Err(RelationError::IncompatibleUnits);
                }
                if denominator == 0 {
                    return Err(RelationError::ZeroDenominator);
                }
                RelationValue::Proportion {
                    numerator,
                    denominator,
                }
            }
        };
        let left_ref = left.reference;
        let right_ref = right.reference;
        let provenance = DerivationProvenance::new(
            contract.reference.clone(),
            alloc::vec![left_ref.clone(), right_ref.clone()],
        );
        let mut relation = Self {
            id: 0,
            contract: contract.reference.clone(),
            kind: contract.kind.clone(),
            left_ref,
            right_ref,
            context: contract.context.clone(),
            value,
            provenance,
        };
        relation.id = IdentityAdapter::derive_identity_from_basis(&relation.canonical_basis());
        Ok(relation)
    }

    pub(crate) fn seal_identity(&mut self, protocol: crate::identity::IdentityProtocol) {
        self.id = protocol.derive_from_basis(&self.canonical_basis());
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn contract(&self) -> &VersionedId {
        &self.contract
    }

    pub fn kind(&self) -> &RelationKind {
        &self.kind
    }

    pub fn left_ref(&self) -> &ObjectRef {
        &self.left_ref
    }

    pub fn right_ref(&self) -> &ObjectRef {
        &self.right_ref
    }

    pub fn context(&self) -> &RelationContext {
        &self.context
    }

    pub fn value(&self) -> &RelationValue {
        &self.value
    }

    pub fn provenance(&self) -> &DerivationProvenance {
        &self.provenance
    }

    pub fn status(&self) -> LifecycleStatus {
        LifecycleStatus::Candidate
    }

    /// Directed graph topology is distinct from numeric operand ordering.
    /// In particular, Proportion preserves operands but does not create an arc.
    pub fn is_directed(&self) -> bool {
        self.kind.is_directed()
    }
}

impl CanonicalSemanticBasis for AdmittedRelation {
    fn canonical_basis(&self) -> Vec<u8> {
        let mut bytes = b"MDC-SCALAR-001/relation/v1\0".to_vec();
        put_reference(&mut bytes, &self.contract);
        match &self.kind {
            RelationKind::Precedes => bytes.push(0),
            RelationKind::TemporalDistance => bytes.push(1),
            RelationKind::Adjacent { step, symmetric } => {
                bytes.push(2);
                bytes.extend_from_slice(&step.to_le_bytes());
                bytes.push(u8::from(*symmetric));
            }
            RelationKind::Proportion => bytes.push(3),
        }
        put_object_ref(&mut bytes, &self.left_ref);
        put_object_ref(&mut bytes, &self.right_ref);
        put_relation_context(&mut bytes, &self.context);
        match &self.value {
            RelationValue::Established => bytes.push(0),
            RelationValue::TemporalDistance(value) => {
                bytes.push(1);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            RelationValue::Proportion {
                numerator,
                denominator,
            } => {
                bytes.push(2);
                bytes.extend_from_slice(&numerator.to_le_bytes());
                bytes.extend_from_slice(&denominator.to_le_bytes());
            }
        }
        put_reference(&mut bytes, self.provenance.contract());
        bytes.extend_from_slice(&(self.provenance.inputs().len() as u64).to_le_bytes());
        for reference in self.provenance.inputs() {
            put_object_ref(&mut bytes, reference);
        }
        bytes.push(0); // Candidate; construction does not run conformance.
        bytes
    }
}

fn observation_time(input: &ResolvedRelationInput) -> Result<u64, RelationError> {
    if !matches!(input.reference, ObjectRef::Observation(_)) {
        return Err(RelationError::InvalidInputType(input.reference.clone()));
    }
    input
        .timestamp
        .ok_or_else(|| RelationError::InvalidInputType(input.reference.clone()))
}

fn measurement_value(input: &ResolvedRelationInput) -> Result<i128, RelationError> {
    if !matches!(input.reference, ObjectRef::Measurement(_)) {
        return Err(RelationError::InvalidInputType(input.reference.clone()));
    }
    input
        .value
        .ok_or_else(|| RelationError::InvalidInputType(input.reference.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::structure::TemporalScope;

    fn reference(name: &str) -> VersionedId {
        VersionedId::new(name, 1).unwrap()
    }

    fn domain() -> DomainContext {
        DomainContext::new("fixture", reference("tick")).unwrap()
    }

    fn contract(kind: RelationKind) -> RelationContract {
        RelationContract {
            reference: reference("fixture/relation"),
            kind,
            context: RelationContext::new(
                domain(),
                TemporalScope::new(0, u64::MAX).unwrap(),
                reference("fixture/frame"),
                reference("fixture/noncausal"),
            ),
        }
    }

    fn observation(id: u64, timestamp: u64) -> ResolvedRelationInput {
        ResolvedRelationInput {
            reference: ObjectRef::Observation(id),
            context: domain(),
            unit: "count".into(),
            timestamp: Some(timestamp),
            value: None,
            temporal_bounds: (timestamp, timestamp),
        }
    }

    fn measurement(id: u64, value: i128) -> ResolvedRelationInput {
        ResolvedRelationInput {
            reference: ObjectRef::Measurement(id),
            context: domain(),
            unit: "count".into(),
            timestamp: None,
            value: Some(value),
            temporal_bounds: (10, 20),
        }
    }

    #[test]
    fn precedence_checks_real_time_order_and_preserves_references() {
        let contract = contract(RelationKind::Precedes);
        let relation =
            AdmittedRelation::derive(&contract, observation(8, 10), observation(3, 20)).unwrap();
        assert_eq!(relation.left_ref(), &ObjectRef::Observation(8));
        assert_eq!(relation.right_ref(), &ObjectRef::Observation(3));
        assert!(relation.is_directed());
        assert_eq!(
            AdmittedRelation::derive(&contract, observation(3, 20), observation(8, 10)),
            Err(RelationError::TemporalOrder)
        );
        assert_eq!(
            AdmittedRelation::derive(&contract, observation(3, 10), observation(8, 10)),
            Err(RelationError::TemporalOrder)
        );
    }

    #[test]
    fn symmetric_adjacency_normalizes_both_endpoints_and_lineage() {
        let contract = contract(RelationKind::Adjacent {
            step: 10,
            symmetric: true,
        });
        let forward =
            AdmittedRelation::derive(&contract, observation(8, 10), observation(3, 20)).unwrap();
        let reverse =
            AdmittedRelation::derive(&contract, observation(3, 20), observation(8, 10)).unwrap();
        assert_eq!(forward, reverse);
        assert_eq!(forward.left_ref(), &ObjectRef::Observation(3));
        assert_eq!(
            forward.provenance().inputs(),
            &[ObjectRef::Observation(3), ObjectRef::Observation(8)]
        );
        assert!(!forward.is_directed());
    }

    #[test]
    fn proportion_preserves_numeric_order_without_creating_directed_topology() {
        let contract = contract(RelationKind::Proportion);
        let forward =
            AdmittedRelation::derive(&contract, measurement(8, 7), measurement(3, 2)).unwrap();
        let reverse =
            AdmittedRelation::derive(&contract, measurement(3, 2), measurement(8, 7)).unwrap();
        assert_eq!(
            forward.value(),
            &RelationValue::Proportion {
                numerator: 7,
                denominator: 2
            }
        );
        assert_ne!(forward.canonical_basis(), reverse.canonical_basis());
        assert!(!forward.is_directed());
        assert_eq!(
            AdmittedRelation::derive(&contract, measurement(8, 7), measurement(3, 0)),
            Err(RelationError::ZeroDenominator)
        );
    }

    #[test]
    fn exact_signed_ratio_and_temporal_distance_avoid_lossy_or_overflowing_math() {
        let ratio = AdmittedRelation::derive(
            &contract(RelationKind::Proportion),
            measurement(1, i128::MIN),
            measurement(2, -1),
        )
        .unwrap();
        assert_eq!(
            ratio.value(),
            &RelationValue::Proportion {
                numerator: i128::MIN,
                denominator: -1
            }
        );
        let distance = AdmittedRelation::derive(
            &contract(RelationKind::TemporalDistance),
            observation(1, u64::MAX),
            observation(2, 0),
        )
        .unwrap();
        assert_eq!(
            distance.value(),
            &RelationValue::TemporalDistance(-i128::from(u64::MAX))
        );
    }
}
