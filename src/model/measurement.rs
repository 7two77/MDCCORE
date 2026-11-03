use alloc::string::String;
use alloc::vec::Vec;

use crate::contract::encoding::{put_context, put_object_ref, put_reference, put_str};
use crate::contract::evaluation::LifecycleStatus;
use crate::contract::registry::{DomainContext, OperatorContract, OperatorKind, VersionedId};
use crate::identity::{CanonicalSemanticBasis, IdentityAdapter};
use crate::provenance::{DerivationProvenance, ObjectRef};

/// A successfully constructed scalar measurement. Construction does not imply
/// conformance to any later State, Transition, or Evaluation contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measurement {
    id: u64,
    operator: VersionedId,
    inputs: Vec<ObjectRef>,
    value: i128,
    unit: String,
    context: DomainContext,
    provenance: DerivationProvenance,
}

/// Failures of the registered scalar operator contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeasurementError {
    WrongInputCount { expected: usize, actual: usize },
    IncompatibleContext,
    IncompatibleUnits,
    ArithmeticOverflow,
}

/// Values resolved by the pipeline from already admitted Observation or
/// Measurement objects. Source contracts are not directly measurable inputs.
pub(crate) struct ResolvedInput {
    pub reference: ObjectRef,
    pub value: i128,
    pub unit: String,
    pub context: DomainContext,
}

impl Measurement {
    pub(crate) fn derive(
        operator: &OperatorContract,
        inputs: &[ResolvedInput],
    ) -> Result<Self, MeasurementError> {
        let expected = match &operator.kind {
            OperatorKind::Scalar => 1,
            OperatorKind::Difference => 2,
        };
        if inputs.len() != expected {
            return Err(MeasurementError::WrongInputCount {
                expected,
                actual: inputs.len(),
            });
        }

        let first = &inputs[0];
        if inputs.iter().any(|input| input.context != first.context) {
            return Err(MeasurementError::IncompatibleContext);
        }
        if inputs.iter().any(|input| input.unit != first.unit) {
            return Err(MeasurementError::IncompatibleUnits);
        }

        let value = match &operator.kind {
            OperatorKind::Scalar => first.value,
            OperatorKind::Difference => first
                .value
                .checked_sub(inputs[1].value)
                .ok_or(MeasurementError::ArithmeticOverflow)?,
        };
        let references: Vec<_> = inputs.iter().map(|input| input.reference.clone()).collect();
        let provenance = DerivationProvenance::new(operator.reference.clone(), references.clone());
        let mut measurement = Self {
            id: 0,
            operator: operator.reference.clone(),
            inputs: references,
            value,
            unit: first.unit.clone(),
            context: first.context.clone(),
            provenance,
        };
        measurement.id =
            IdentityAdapter::derive_identity_from_basis(&measurement.canonical_basis());
        Ok(measurement)
    }

    pub(crate) fn seal_identity(&mut self, protocol: crate::identity::IdentityProtocol) {
        self.id = protocol.derive_from_basis(&self.canonical_basis());
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn operator(&self) -> &VersionedId {
        &self.operator
    }

    pub fn inputs(&self) -> &[ObjectRef] {
        &self.inputs
    }

    pub fn value(&self) -> i128 {
        self.value
    }

    pub fn unit(&self) -> &str {
        &self.unit
    }

    pub fn context(&self) -> &DomainContext {
        &self.context
    }

    pub fn provenance(&self) -> &DerivationProvenance {
        &self.provenance
    }

    pub fn status(&self) -> LifecycleStatus {
        LifecycleStatus::Candidate
    }
}

impl CanonicalSemanticBasis for Measurement {
    fn canonical_basis(&self) -> Vec<u8> {
        let mut bytes = b"MDC-SCALAR-001/measurement/v1\0".to_vec();
        put_reference(&mut bytes, &self.operator);
        put_references(&mut bytes, &self.inputs);
        bytes.extend_from_slice(&self.value.to_le_bytes());
        put_str(&mut bytes, &self.unit);
        put_context(&mut bytes, &self.context);
        put_reference(&mut bytes, self.provenance.contract());
        put_references(&mut bytes, self.provenance.inputs());
        bytes
    }
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
    use alloc::string::ToString;

    fn contract() -> OperatorContract {
        OperatorContract {
            reference: VersionedId::new("difference", 1).unwrap(),
            kind: OperatorKind::Difference,
        }
    }

    fn input(id: u64, value: i128) -> ResolvedInput {
        ResolvedInput {
            reference: ObjectRef::Observation(id),
            value,
            unit: "count".to_string(),
            context: DomainContext::new("fixture", VersionedId::new("tick", 1).unwrap()).unwrap(),
        }
    }

    #[test]
    fn difference_preserves_order_in_value_identity_and_lineage() {
        let forward = Measurement::derive(&contract(), &[input(1, 9), input(2, 4)]).unwrap();
        let reverse = Measurement::derive(&contract(), &[input(2, 4), input(1, 9)]).unwrap();

        assert_eq!(forward.value(), 5);
        assert_eq!(reverse.value(), -5);
        assert_ne!(forward.canonical_basis(), reverse.canonical_basis());
        assert_ne!(forward.id(), reverse.id());
        assert_eq!(
            forward.inputs(),
            &[ObjectRef::Observation(1), ObjectRef::Observation(2)]
        );
        assert_eq!(forward.provenance().inputs(), forward.inputs());
        assert_eq!(forward.status(), LifecycleStatus::Candidate);
    }

    #[test]
    fn difference_rejects_overflow_instead_of_wrapping() {
        let result = Measurement::derive(&contract(), &[input(1, i128::MAX), input(2, -1)]);

        assert_eq!(result, Err(MeasurementError::ArithmeticOverflow));
    }
}
