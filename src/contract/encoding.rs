//! Length-prefixed encoding used by admitted scalar-profile objects.

use super::dynamics::DynamicsContext;
use super::registry::{DomainContext, VersionedId};
use super::structure::RelationContext;
use crate::provenance::ObjectRef;
use alloc::vec::Vec;

pub(crate) fn put_bytes(out: &mut Vec<u8>, value: &[u8]) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value);
}

pub(crate) fn put_str(out: &mut Vec<u8>, value: &str) {
    put_bytes(out, value.as_bytes());
}

pub(crate) fn put_reference(out: &mut Vec<u8>, value: &VersionedId) {
    put_str(out, value.id());
    out.extend_from_slice(&value.version().to_le_bytes());
}

pub(crate) fn put_context(out: &mut Vec<u8>, value: &DomainContext) {
    put_str(out, value.domain());
    put_reference(out, value.time_basis());
}

pub(crate) fn put_relation_context(out: &mut Vec<u8>, value: &RelationContext) {
    put_context(out, value.domain());
    out.extend_from_slice(&value.scope().start().to_le_bytes());
    out.extend_from_slice(&value.scope().end().to_le_bytes());
    put_reference(out, value.reference_frame());
    put_reference(out, value.causal_context());
}

pub(crate) fn put_object_ref(out: &mut Vec<u8>, value: &ObjectRef) {
    match value {
        ObjectRef::InvalidObservation(id) => {
            out.push(8);
            out.extend_from_slice(&id.digest().to_le_bytes());
            put_bytes(out, id.canonical_key());
        }
        ObjectRef::InvalidMeasurement(id) => {
            out.push(9);
            out.extend_from_slice(&id.digest().to_le_bytes());
            put_bytes(out, id.canonical_key());
        }
        ObjectRef::InvalidRelation(id) => {
            out.push(10);
            out.extend_from_slice(&id.digest().to_le_bytes());
            put_bytes(out, id.canonical_key());
        }
        ObjectRef::InvalidComposition(id) => {
            out.push(11);
            out.extend_from_slice(&id.digest().to_le_bytes());
            put_bytes(out, id.canonical_key());
        }
        ObjectRef::InvalidState(id) => {
            out.push(12);
            out.extend_from_slice(&id.digest().to_le_bytes());
            put_bytes(out, id.canonical_key());
        }
        ObjectRef::InvalidTransition(id) => {
            out.push(13);
            out.extend_from_slice(&id.digest().to_le_bytes());
            put_bytes(out, id.canonical_key());
        }
        ObjectRef::Source(reference) => {
            out.push(0);
            put_reference(out, reference);
        }
        ObjectRef::Observation(id) => {
            out.push(1);
            out.extend_from_slice(&id.to_le_bytes());
        }
        ObjectRef::Measurement(id) => {
            out.push(2);
            out.extend_from_slice(&id.to_le_bytes());
        }
        ObjectRef::Relation(id) => {
            out.push(3);
            out.extend_from_slice(&id.to_le_bytes());
        }
        ObjectRef::Composition(id) => {
            out.push(4);
            out.extend_from_slice(&id.to_le_bytes());
        }
        ObjectRef::State(id) => {
            out.push(5);
            out.extend_from_slice(&id.to_le_bytes());
        }
        ObjectRef::Evaluation(id) => {
            out.push(7);
            out.extend_from_slice(&id.to_le_bytes());
        }
        ObjectRef::Transition(id) => {
            out.push(6);
            out.extend_from_slice(&id.to_le_bytes());
        }
    }
}

pub(crate) fn put_dynamics_context(out: &mut Vec<u8>, value: &DynamicsContext) {
    put_relation_context(out, value.frame());
    put_reference(out, value.calculus_version());
    put_reference(out, value.domain_version());
}

#[cfg(test)]
mod tests {
    use super::{put_object_ref, put_str};
    use crate::provenance::ObjectRef;
    use alloc::vec::Vec;

    #[test]
    fn adjacent_variable_fields_preserve_their_boundaries() {
        let mut first = Vec::new();
        put_str(&mut first, "ab");
        put_str(&mut first, "c");
        let mut second = Vec::new();
        put_str(&mut second, "a");
        put_str(&mut second, "bc");
        assert_ne!(first, second);
    }

    #[test]
    fn reference_types_are_part_of_the_encoding() {
        let mut observation = Vec::new();
        put_object_ref(&mut observation, &ObjectRef::Observation(42));
        let mut measurement = Vec::new();
        put_object_ref(&mut measurement, &ObjectRef::Measurement(42));
        assert_ne!(observation, measurement);
        let mut relation = Vec::new();
        put_object_ref(&mut relation, &ObjectRef::Relation(42));
        let mut composition = Vec::new();
        put_object_ref(&mut composition, &ObjectRef::Composition(42));
        assert_ne!(measurement, relation);
        assert_ne!(relation, composition);
        assert_eq!(&observation[1..], &relation[1..]);
        assert_eq!(&measurement[1..], &composition[1..]);
        let mut state = Vec::new();
        put_object_ref(&mut state, &ObjectRef::State(42));
        let mut transition = Vec::new();
        put_object_ref(&mut transition, &ObjectRef::Transition(42));
        assert_eq!(state[0], 5);
        assert_eq!(transition[0], 6);
        assert_eq!(&state[1..], &observation[1..]);
        assert_eq!(&transition[1..], &observation[1..]);
    }
}
