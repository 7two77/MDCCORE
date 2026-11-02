use crate::contract::encoding::{put_bytes, put_context, put_reference, put_str};
use crate::contract::evaluation::LifecycleStatus;
use crate::contract::registry::{
    DomainContext, ScalarSchema, SchemaContract, SourceContract, VersionedId,
};
use crate::identity::{CanonicalSemanticBasis, IdentityAdapter};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub trait Source {
    fn id(&self) -> &str;
}

pub struct Observation<S: Source> {
    pub source: S,
    pub timestamp: u64,
    pub payload: Vec<u8>,
}

impl<S: Source> CanonicalSemanticBasis for Observation<S> {
    fn canonical_basis(&self) -> Vec<u8> {
        // Preserve the existing representation while giving the caller ownership.
        // A versioned, portable canonical format is a separate contract decision.
        format!(
            "{:?}|{:?}|{:?}",
            self.source.id(),
            self.timestamp,
            self.payload
        )
        .into_bytes()
    }
}

impl<S: Source> Observation<S> {
    pub fn new(source: S, timestamp: u64, payload: Vec<u8>) -> Result<Self, String> {
        if source.id().is_empty() {
            return Err("Source ID cannot be empty".to_string());
        }
        Ok(Observation {
            source,
            timestamp,
            payload,
        })
    }
}

/// Validation failures of the initial signed-i64 scalar admission profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdmissionError {
    MissingTimestamp,
    EmptyAcquisitionContext,
    InvalidPayload { expected: usize, actual: usize },
}

/// Source acquisition metadata supplied under the registered source contract.
/// This is declared lineage, not authentication of an external data feed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionProvenance {
    source: VersionedId,
    schema: VersionedId,
    acquisition_context: String,
}

impl AdmissionProvenance {
    pub fn source(&self) -> &VersionedId {
        &self.source
    }

    pub fn schema(&self) -> &VersionedId {
        &self.schema
    }

    pub fn acquisition_context(&self) -> &str {
        &self.acquisition_context
    }

    pub fn identity_profile(&self) -> &'static str {
        "legacy-rust-hash-fnv64"
    }
}

/// An immutable observation admitted by `Pipeline` under a registered schema.
/// Legacy `Observation::new()` alone does not produce an admitted object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedObservation {
    id: u64,
    timestamp: u64,
    payload: Vec<u8>,
    value: i64,
    context: DomainContext,
    unit: String,
    provenance: AdmissionProvenance,
}

impl AdmittedObservation {
    pub(crate) fn admit(
        source: &SourceContract,
        schema: &SchemaContract,
        timestamp: Option<u64>,
        payload: Vec<u8>,
        acquisition_context: String,
    ) -> Result<Self, AdmissionError> {
        let timestamp = timestamp.ok_or(AdmissionError::MissingTimestamp)?;
        if acquisition_context.trim().is_empty() {
            return Err(AdmissionError::EmptyAcquisitionContext);
        }
        let value =
            match schema.format {
                ScalarSchema::SignedI64Le => {
                    let bytes: [u8; 8] = payload.as_slice().try_into().map_err(|_| {
                        AdmissionError::InvalidPayload {
                            expected: 8,
                            actual: payload.len(),
                        }
                    })?;
                    i64::from_le_bytes(bytes)
                }
            };
        let mut observation = Self {
            id: 0,
            timestamp,
            payload,
            value,
            context: source.context.clone(),
            unit: source.unit.clone(),
            provenance: AdmissionProvenance {
                source: source.reference.clone(),
                schema: source.schema.clone(),
                acquisition_context,
            },
        };
        observation.id =
            IdentityAdapter::derive_identity_from_basis(&observation.canonical_basis());
        Ok(observation)
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn source_ref(&self) -> &VersionedId {
        self.provenance.source()
    }

    pub fn schema(&self) -> &VersionedId {
        self.provenance.schema()
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn context(&self) -> &DomainContext {
        &self.context
    }

    pub fn unit(&self) -> &str {
        &self.unit
    }

    pub fn provenance(&self) -> &AdmissionProvenance {
        &self.provenance
    }

    pub fn status(&self) -> LifecycleStatus {
        LifecycleStatus::Candidate
    }

    pub(crate) fn scalar_value(&self) -> i128 {
        i128::from(self.value)
    }
}

impl CanonicalSemanticBasis for AdmittedObservation {
    fn canonical_basis(&self) -> Vec<u8> {
        let mut bytes = b"MDC-SCALAR-001/observation/v1\0".to_vec();
        put_reference(&mut bytes, self.source_ref());
        put_reference(&mut bytes, self.schema());
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        put_context(&mut bytes, &self.context);
        put_str(&mut bytes, &self.unit);
        put_bytes(&mut bytes, &self.payload);
        // The supplied acquisition context is a deterministic part of this
        // profile's declared source context, never a generated clock/counter.
        put_str(&mut bytes, self.provenance.acquisition_context());
        bytes
    }
}
