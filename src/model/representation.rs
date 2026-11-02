//! Detached external representation, outside the calculus ObjectRef graph.
use crate::contract::dynamics::DynamicsContext;
use crate::contract::encoding::{
    put_bytes, put_dynamics_context, put_object_ref, put_reference, put_str,
};
use crate::contract::projection::ProjectionProfile;
use crate::contract::registry::VersionedId;
use crate::provenance::ObjectRef;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Representation {
    source: ObjectRef,
    evaluation: ObjectRef,
    profile: VersionedId,
    schema: VersionedId,
    conformance: VersionedId,
    context: DynamicsContext,
    source_basis: Vec<u8>,
    evaluation_basis: Vec<u8>,
    /// External presentation text. Editing it never changes source semantics.
    pub label: String,
}

impl Representation {
    pub(crate) fn new(
        profile: &ProjectionProfile,
        source: ObjectRef,
        evaluation: ObjectRef,
        source_basis: Vec<u8>,
        evaluation_basis: Vec<u8>,
    ) -> Self {
        Self {
            source,
            evaluation,
            profile: profile.reference.clone(),
            schema: profile.schema.clone(),
            conformance: profile.conformance.clone(),
            context: profile.context.clone(),
            source_basis,
            evaluation_basis,
            label: "external-description".into(),
        }
    }
    pub fn source(&self) -> &ObjectRef {
        &self.source
    }
    pub fn evaluation(&self) -> &ObjectRef {
        &self.evaluation
    }
    pub fn profile(&self) -> &VersionedId {
        &self.profile
    }
    pub fn schema(&self) -> &VersionedId {
        &self.schema
    }
    pub fn conformance(&self) -> &VersionedId {
        &self.conformance
    }
    pub fn context(&self) -> &DynamicsContext {
        &self.context
    }
    pub fn source_basis(&self) -> &[u8] {
        &self.source_basis
    }
    pub fn evaluation_basis(&self) -> &[u8] {
        &self.evaluation_basis
    }
    /// External representation bytes, deliberately not a calculus identity API.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = b"MDC-TEST-REP-01/representation/v1\0".to_vec();
        put_reference(&mut bytes, &self.profile);
        put_reference(&mut bytes, &self.schema);
        put_reference(&mut bytes, &self.conformance);
        put_dynamics_context(&mut bytes, &self.context);
        put_object_ref(&mut bytes, &self.source);
        put_object_ref(&mut bytes, &self.evaluation);
        put_bytes(&mut bytes, &self.source_basis);
        put_bytes(&mut bytes, &self.evaluation_basis);
        put_str(&mut bytes, &self.label);
        bytes
    }
}
