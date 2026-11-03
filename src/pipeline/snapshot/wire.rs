//! Bounded fixed-width wire primitives for replay snapshot format v1.
//!
//! Raw reference keys are historical data. Decoding never repairs their digest
//! or treats an unresolved request reference as an admitted semantic object.

use super::{SnapshotError, SnapshotLimits};
use crate::contract::dynamics::DynamicsContext;
use crate::contract::registry::{DomainContext, VersionedId};
use crate::contract::structure::{RelationContext, TemporalScope};
use crate::provenance::{EvaluationId, InvalidId, ObjectRef};
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone, Copy)]
enum CountKind {
    Collection,
    Definitions,
    Operations,
}

#[derive(Default)]
struct Counts {
    items: usize,
    definitions: usize,
    operations: usize,
}

impl Counts {
    fn checked(
        &self,
        n: usize,
        kind: CountKind,
        limits: SnapshotLimits,
    ) -> Result<Self, SnapshotError> {
        if n > limits.max_collection_items {
            return Err(SnapshotError::LimitExceeded("max_collection_items"));
        }
        let items = self
            .items
            .checked_add(n)
            .filter(|n| *n <= limits.max_total_items)
            .ok_or(SnapshotError::LimitExceeded("max_total_items"))?;
        let definitions = if matches!(kind, CountKind::Definitions) {
            self.definitions
                .checked_add(n)
                .filter(|n| *n <= limits.max_definitions)
                .ok_or(SnapshotError::LimitExceeded("max_definitions"))?
        } else {
            self.definitions
        };
        let operations = if matches!(kind, CountKind::Operations) {
            self.operations
                .checked_add(n)
                .filter(|n| *n <= limits.max_operations)
                .ok_or(SnapshotError::LimitExceeded("max_operations"))?
        } else {
            self.operations
        };
        Ok(Self {
            items,
            definitions,
            operations,
        })
    }
}

pub(crate) struct Writer {
    output: Vec<u8>,
    limits: SnapshotLimits,
    counts: Counts,
}

impl Writer {
    pub(crate) fn new(limits: SnapshotLimits) -> Self {
        Self {
            output: Vec::new(),
            limits,
            counts: Counts::default(),
        }
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        self.output
    }

    fn room(&self, n: usize) -> Result<(), SnapshotError> {
        self.output
            .len()
            .checked_add(n)
            .filter(|n| *n <= self.limits.max_snapshot_bytes)
            .map(|_| ())
            .ok_or(SnapshotError::LimitExceeded("max_snapshot_bytes"))
    }

    pub(crate) fn raw(&mut self, data: &[u8]) -> Result<(), SnapshotError> {
        self.room(data.len())?;
        self.output.extend_from_slice(data);
        Ok(())
    }

    pub(crate) fn u8(&mut self, value: u8) -> Result<(), SnapshotError> {
        self.raw(&[value])
    }

    pub(crate) fn u16(&mut self, value: u16) -> Result<(), SnapshotError> {
        self.raw(&value.to_le_bytes())
    }

    pub(crate) fn u32(&mut self, value: u32) -> Result<(), SnapshotError> {
        self.raw(&value.to_le_bytes())
    }

    pub(crate) fn u64(&mut self, value: u64) -> Result<(), SnapshotError> {
        self.raw(&value.to_le_bytes())
    }

    pub(crate) fn signed(&mut self, value: i128) -> Result<(), SnapshotError> {
        self.raw(&value.to_le_bytes())
    }

    pub(crate) fn boolean(&mut self, value: bool) -> Result<(), SnapshotError> {
        self.u8(u8::from(value))
    }

    fn field(&mut self, data: &[u8], max: usize, label: &'static str) -> Result<(), SnapshotError> {
        if data.len() > max {
            return Err(SnapshotError::LimitExceeded(label));
        }
        let length = u64::try_from(data.len()).map_err(|_| SnapshotError::LimitExceeded(label))?;
        let encoded = 8_usize
            .checked_add(data.len())
            .ok_or(SnapshotError::LimitExceeded("max_snapshot_bytes"))?;
        self.room(encoded)?;
        self.output.extend_from_slice(&length.to_le_bytes());
        self.output.extend_from_slice(data);
        Ok(())
    }

    pub(crate) fn bytes(&mut self, value: &[u8]) -> Result<(), SnapshotError> {
        self.field(value, self.limits.max_field_bytes, "max_field_bytes")
    }

    pub(crate) fn manifest(&mut self, value: &[u8]) -> Result<(), SnapshotError> {
        self.field(value, self.limits.max_manifest_bytes, "max_manifest_bytes")
    }

    pub(crate) fn string(&mut self, value: &str) -> Result<(), SnapshotError> {
        self.bytes(value.as_bytes())
    }

    fn count(&mut self, n: usize, kind: CountKind) -> Result<(), SnapshotError> {
        let next = self.counts.checked(n, kind, self.limits)?;
        let encoded =
            u64::try_from(n).map_err(|_| SnapshotError::LimitExceeded("max_collection_items"))?;
        self.u64(encoded)?;
        self.counts = next;
        Ok(())
    }

    pub(crate) fn len(&mut self, value: usize) -> Result<(), SnapshotError> {
        self.count(value, CountKind::Collection)
    }

    pub(crate) fn definitions(&mut self, value: usize) -> Result<(), SnapshotError> {
        self.count(value, CountKind::Definitions)
    }

    pub(crate) fn operations(&mut self, value: usize) -> Result<(), SnapshotError> {
        self.count(value, CountKind::Operations)
    }

    pub(crate) fn reference(&mut self, value: &VersionedId) -> Result<(), SnapshotError> {
        self.string(value.id())?;
        self.u32(value.version())
    }

    pub(crate) fn domain(&mut self, value: &DomainContext) -> Result<(), SnapshotError> {
        self.string(value.domain())?;
        self.reference(value.time_basis())
    }

    pub(crate) fn relation_context(
        &mut self,
        value: &RelationContext,
    ) -> Result<(), SnapshotError> {
        self.domain(value.domain())?;
        self.u64(value.scope().start())?;
        self.u64(value.scope().end())?;
        self.reference(value.reference_frame())?;
        self.reference(value.causal_context())
    }

    pub(crate) fn dynamics(&mut self, value: &DynamicsContext) -> Result<(), SnapshotError> {
        self.relation_context(value.frame())?;
        self.reference(value.calculus_version())?;
        self.reference(value.domain_version())
    }

    pub(crate) fn object_ref(&mut self, value: &ObjectRef) -> Result<(), SnapshotError> {
        match value {
            ObjectRef::Source(reference) => {
                self.u8(0)?;
                self.reference(reference)
            }
            ObjectRef::Observation(id) => {
                self.u8(1)?;
                self.u64(*id)
            }
            ObjectRef::Measurement(id) => {
                self.u8(2)?;
                self.u64(*id)
            }
            ObjectRef::Relation(id) => {
                self.u8(3)?;
                self.u64(*id)
            }
            ObjectRef::Composition(id) => {
                self.u8(4)?;
                self.u64(*id)
            }
            ObjectRef::State(id) => {
                self.u8(5)?;
                self.u64(*id)
            }
            ObjectRef::Transition(id) => {
                self.u8(6)?;
                self.u64(*id)
            }
            ObjectRef::Evaluation(id) => {
                self.u8(7)?;
                self.u64(*id)
            }
            ObjectRef::InvalidObservation(id) => self.invalid(8, id),
            ObjectRef::InvalidMeasurement(id) => self.invalid(9, id),
            ObjectRef::InvalidRelation(id) => self.invalid(10, id),
            ObjectRef::InvalidComposition(id) => self.invalid(11, id),
            ObjectRef::InvalidState(id) => self.invalid(12, id),
            ObjectRef::InvalidTransition(id) => self.invalid(13, id),
            ObjectRef::EvaluationKey(id) => {
                self.u8(14)?;
                self.u64(id.digest())?;
                self.bytes(id.canonical_key())
            }
        }
    }

    fn invalid(&mut self, tag: u8, id: &InvalidId) -> Result<(), SnapshotError> {
        self.u8(tag)?;
        self.u64(id.digest())?;
        self.bytes(id.canonical_key())
    }
}

pub(crate) struct Reader<'a> {
    input: &'a [u8],
    position: usize,
    limits: SnapshotLimits,
    counts: Counts,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(input: &'a [u8], limits: SnapshotLimits) -> Result<Self, SnapshotError> {
        if input.len() > limits.max_snapshot_bytes {
            return Err(SnapshotError::LimitExceeded("max_snapshot_bytes"));
        }
        Ok(Self {
            input,
            position: 0,
            limits,
            counts: Counts::default(),
        })
    }

    pub(crate) fn position(&self) -> usize {
        self.position
    }
    pub(crate) fn limits(&self) -> SnapshotLimits {
        self.limits
    }

    /// Positions are supplied by the enclosing codec, never by wire data.
    pub(crate) fn slice(&self, start: usize, end: usize) -> &'a [u8] {
        &self.input[start..end]
    }

    pub(crate) fn finish(self) -> Result<(), SnapshotError> {
        if self.position == self.input.len() {
            Ok(())
        } else {
            Err(SnapshotError::TrailingBytes)
        }
    }

    fn peek(&self, n: usize) -> Result<&'a [u8], SnapshotError> {
        let end = self
            .position
            .checked_add(n)
            .ok_or(SnapshotError::Truncated)?;
        self.input
            .get(self.position..end)
            .ok_or(SnapshotError::Truncated)
    }

    pub(crate) fn raw(&mut self, n: usize) -> Result<&'a [u8], SnapshotError> {
        let bytes = self.peek(n)?;
        self.position += n;
        Ok(bytes)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, SnapshotError> {
        Ok(self.raw(1)?[0])
    }
    pub(crate) fn u16(&mut self) -> Result<u16, SnapshotError> {
        Ok(u16::from_le_bytes(
            self.raw(2)?.try_into().expect("fixed size slice"),
        ))
    }
    pub(crate) fn u32(&mut self) -> Result<u32, SnapshotError> {
        Ok(u32::from_le_bytes(
            self.raw(4)?.try_into().expect("fixed size slice"),
        ))
    }
    pub(crate) fn u64(&mut self) -> Result<u64, SnapshotError> {
        Ok(u64::from_le_bytes(
            self.raw(8)?.try_into().expect("fixed size slice"),
        ))
    }
    #[cfg(test)]
    pub(crate) fn signed(&mut self) -> Result<i128, SnapshotError> {
        Ok(i128::from_le_bytes(
            self.raw(16)?.try_into().expect("fixed size slice"),
        ))
    }

    pub(crate) fn boolean(&mut self) -> Result<bool, SnapshotError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(SnapshotError::InvalidTag {
                kind: "boolean",
                tag,
            }),
        }
    }

    fn field(&self, max: usize, label: &'static str) -> Result<(&'a [u8], usize), SnapshotError> {
        let raw: [u8; 8] = self.peek(8)?.try_into().expect("fixed size slice");
        let length = u64::from_le_bytes(raw);
        let length = usize::try_from(length).map_err(|_| SnapshotError::LimitExceeded(label))?;
        if length > max {
            return Err(SnapshotError::LimitExceeded(label));
        }
        let start = self
            .position
            .checked_add(8)
            .ok_or(SnapshotError::Truncated)?;
        let end = start.checked_add(length).ok_or(SnapshotError::Truncated)?;
        Ok((
            self.input.get(start..end).ok_or(SnapshotError::Truncated)?,
            end,
        ))
    }

    pub(crate) fn bytes(&mut self) -> Result<Vec<u8>, SnapshotError> {
        let (value, end) = self.field(self.limits.max_field_bytes, "max_field_bytes")?;
        let value = value.to_vec();
        self.position = end;
        Ok(value)
    }

    pub(crate) fn manifest(&mut self) -> Result<Vec<u8>, SnapshotError> {
        let (value, end) = self.field(self.limits.max_manifest_bytes, "max_manifest_bytes")?;
        let value = value.to_vec();
        self.position = end;
        Ok(value)
    }

    pub(crate) fn string(&mut self) -> Result<String, SnapshotError> {
        let (value, end) = self.field(self.limits.max_field_bytes, "max_field_bytes")?;
        let text = core::str::from_utf8(value).map_err(|_| SnapshotError::InvalidUtf8)?;
        let text = String::from(text);
        self.position = end;
        Ok(text)
    }

    fn count(&mut self, kind: CountKind) -> Result<usize, SnapshotError> {
        let raw: [u8; 8] = self.peek(8)?.try_into().expect("fixed size slice");
        let count = usize::try_from(u64::from_le_bytes(raw))
            .map_err(|_| SnapshotError::LimitExceeded("max_collection_items"))?;
        let next = self.counts.checked(count, kind, self.limits)?;
        self.position += 8;
        self.counts = next;
        Ok(count)
    }

    pub(crate) fn len(&mut self) -> Result<usize, SnapshotError> {
        self.count(CountKind::Collection)
    }
    pub(crate) fn definitions(&mut self) -> Result<usize, SnapshotError> {
        self.count(CountKind::Definitions)
    }
    pub(crate) fn operations(&mut self) -> Result<usize, SnapshotError> {
        self.count(CountKind::Operations)
    }

    pub(crate) fn reference(&mut self) -> Result<VersionedId, SnapshotError> {
        let id = self.string()?;
        let version = self.u32()?;
        VersionedId::new(id, version).map_err(SnapshotError::InvalidDefinition)
    }

    pub(crate) fn domain(&mut self) -> Result<DomainContext, SnapshotError> {
        let domain = self.string()?;
        let time_basis = self.reference()?;
        DomainContext::new(domain, time_basis).map_err(SnapshotError::InvalidDefinition)
    }

    pub(crate) fn relation_context(&mut self) -> Result<RelationContext, SnapshotError> {
        let domain = self.domain()?;
        let start = self.u64()?;
        let end = self.u64()?;
        let scope = TemporalScope::new(start, end).map_err(SnapshotError::InvalidDefinition)?;
        let frame = self.reference()?;
        let causal = self.reference()?;
        Ok(RelationContext::new(domain, scope, frame, causal))
    }

    pub(crate) fn dynamics(&mut self) -> Result<DynamicsContext, SnapshotError> {
        let frame = self.relation_context()?;
        let calculus = self.reference()?;
        let domain = self.reference()?;
        Ok(DynamicsContext::new(frame, calculus, domain))
    }

    pub(crate) fn object_ref(&mut self) -> Result<ObjectRef, SnapshotError> {
        let tag = self.u8()?;
        match tag {
            0 => Ok(ObjectRef::Source(self.reference()?)),
            1 => Ok(ObjectRef::Observation(self.u64()?)),
            2 => Ok(ObjectRef::Measurement(self.u64()?)),
            3 => Ok(ObjectRef::Relation(self.u64()?)),
            4 => Ok(ObjectRef::Composition(self.u64()?)),
            5 => Ok(ObjectRef::State(self.u64()?)),
            6 => Ok(ObjectRef::Transition(self.u64()?)),
            7 => Ok(ObjectRef::Evaluation(self.u64()?)),
            8..=13 => {
                let digest = self.u64()?;
                let key = self.bytes()?;
                let id = InvalidId::from_snapshot_parts(digest, key);
                Ok(match tag {
                    8 => ObjectRef::InvalidObservation(id),
                    9 => ObjectRef::InvalidMeasurement(id),
                    10 => ObjectRef::InvalidRelation(id),
                    11 => ObjectRef::InvalidComposition(id),
                    12 => ObjectRef::InvalidState(id),
                    13 => ObjectRef::InvalidTransition(id),
                    _ => unreachable!("bounded tag arm"),
                })
            }
            14 => {
                let digest = self.u64()?;
                let key = self.bytes()?;
                Ok(ObjectRef::EvaluationKey(EvaluationId::for_artifact(
                    digest, key,
                )))
            }
            tag => Err(SnapshotError::InvalidTag {
                kind: "object reference",
                tag,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> SnapshotLimits {
        SnapshotLimits {
            max_snapshot_bytes: 8192,
            max_field_bytes: 1024,
            max_collection_items: 32,
            max_total_items: 128,
            max_definitions: 16,
            max_operations: 16,
            max_manifest_bytes: 4096,
        }
    }

    #[test]
    fn integers_have_fixed_little_endian_bytes_and_roundtrip_without_native_width() {
        let mut w = Writer::new(limits());
        w.u8(0xab).unwrap();
        w.u16(0x1234).unwrap();
        w.u32(0x01020304).unwrap();
        w.u64(0x0102030405060708).unwrap();
        w.signed(-2).unwrap();
        w.boolean(true).unwrap();
        let out = w.finish();
        assert_eq!(
            &out[..15],
            &[0xab, 0x34, 0x12, 4, 3, 2, 1, 8, 7, 6, 5, 4, 3, 2, 1]
        );
        assert_eq!(out[15], 0xfe);
        assert!(out[16..31].iter().all(|b| *b == 0xff));
        let mut r = Reader::new(&out, limits()).unwrap();
        assert_eq!(r.u8().unwrap(), 0xab);
        assert_eq!(r.u16().unwrap(), 0x1234);
        assert_eq!(r.u32().unwrap(), 0x01020304);
        assert_eq!(r.u64().unwrap(), 0x0102030405060708);
        assert_eq!(r.signed().unwrap(), -2);
        assert!(r.boolean().unwrap());
        r.finish().unwrap();
    }

    #[test]
    fn unknown_boolean_reference_tags_truncation_and_trailing_bytes_are_rejected() {
        let mut r = Reader::new(&[2], limits()).unwrap();
        assert!(matches!(
            r.boolean(),
            Err(SnapshotError::InvalidTag {
                kind: "boolean",
                tag: 2
            })
        ));
        let mut r = Reader::new(&[255], limits()).unwrap();
        assert!(matches!(
            r.object_ref(),
            Err(SnapshotError::InvalidTag {
                kind: "object reference",
                tag: 255
            })
        ));
        for n in 0..16 {
            let bytes = alloc::vec![0; n];
            let mut r = Reader::new(&bytes, limits()).unwrap();
            assert!(matches!(r.signed(), Err(SnapshotError::Truncated)));
            assert_eq!(r.position(), 0);
        }
        assert!(matches!(
            Reader::new(&[0], limits()).unwrap().finish(),
            Err(SnapshotError::TrailingBytes)
        ));
    }

    #[test]
    fn length_bombs_and_invalid_utf8_fail_before_copying_or_advancing_fields() {
        let huge = u64::MAX.to_le_bytes();
        let mut r = Reader::new(&huge, limits()).unwrap();
        assert!(matches!(
            r.bytes(),
            Err(SnapshotError::LimitExceeded("max_field_bytes"))
        ));
        assert_eq!(r.position(), 0);
        let mut bad = 1_u64.to_le_bytes().to_vec();
        bad.push(0xff);
        let mut r = Reader::new(&bad, limits()).unwrap();
        assert!(matches!(r.string(), Err(SnapshotError::InvalidUtf8)));
        assert_eq!(r.position(), 0);
        let mut short = 2_u64.to_le_bytes().to_vec();
        short.push(0);
        assert!(matches!(
            Reader::new(&short, limits()).unwrap().bytes(),
            Err(SnapshotError::Truncated)
        ));
    }

    #[test]
    fn counts_enforce_cumulative_definitions_operations_and_total_items() {
        let mut caps = limits();
        caps.max_definitions = 3;
        let mut w = Writer::new(caps);
        w.definitions(2).unwrap();
        assert!(matches!(
            w.definitions(2),
            Err(SnapshotError::LimitExceeded("max_definitions"))
        ));
        let mut counts = 2_u64.to_le_bytes().to_vec();
        counts.extend_from_slice(&2_u64.to_le_bytes());
        let mut r = Reader::new(&counts, caps).unwrap();
        assert_eq!(r.definitions().unwrap(), 2);
        assert!(matches!(
            r.definitions(),
            Err(SnapshotError::LimitExceeded("max_definitions"))
        ));
        assert_eq!(r.position(), 8);
        caps.max_total_items = 3;
        let mut r = Reader::new(&counts, caps).unwrap();
        r.len().unwrap();
        assert!(matches!(
            r.len(),
            Err(SnapshotError::LimitExceeded("max_total_items"))
        ));
        caps.max_operations = 1;
        let mut w = Writer::new(caps);
        w.operations(1).unwrap();
        assert!(matches!(
            w.operations(1),
            Err(SnapshotError::LimitExceeded("max_operations"))
        ));
    }

    #[test]
    fn manifest_has_an_independent_cap_and_output_failure_does_not_append_a_prefix() {
        let mut caps = limits();
        caps.max_field_bytes = 1;
        caps.max_manifest_bytes = 3;
        let mut w = Writer::new(caps);
        assert!(matches!(
            w.bytes(&[1, 2]),
            Err(SnapshotError::LimitExceeded("max_field_bytes"))
        ));
        w.manifest(&[1, 2]).unwrap();
        let out = w.finish();
        assert_eq!(Reader::new(&out, caps).unwrap().manifest().unwrap(), [1, 2]);
        caps.max_snapshot_bytes = 9;
        let mut w = Writer::new(caps);
        assert!(matches!(
            w.manifest(&[1, 2]),
            Err(SnapshotError::LimitExceeded("max_snapshot_bytes"))
        ));
        assert!(w.finish().is_empty());
        assert!(matches!(
            Reader::new(&out, caps),
            Err(SnapshotError::LimitExceeded("max_snapshot_bytes"))
        ));
    }

    #[test]
    fn raw_full_key_references_and_all_namespaces_survive_without_digest_repair() {
        let invalid = InvalidId::from_snapshot_parts(7, alloc::vec![1, 2, 3]);
        let eval = EvaluationId::for_artifact(7, alloc::vec![9, 8, 7]);
        let refs = alloc::vec![
            ObjectRef::Source(VersionedId::new("источник", 2).unwrap()),
            ObjectRef::Observation(7),
            ObjectRef::Measurement(7),
            ObjectRef::Relation(7),
            ObjectRef::Composition(7),
            ObjectRef::State(7),
            ObjectRef::Transition(7),
            ObjectRef::Evaluation(7),
            ObjectRef::InvalidObservation(invalid.clone()),
            ObjectRef::InvalidMeasurement(invalid.clone()),
            ObjectRef::InvalidRelation(invalid.clone()),
            ObjectRef::InvalidComposition(invalid.clone()),
            ObjectRef::InvalidState(invalid.clone()),
            ObjectRef::InvalidTransition(invalid),
            ObjectRef::EvaluationKey(eval)
        ];
        let mut w = Writer::new(limits());
        for item in &refs {
            w.object_ref(item).unwrap();
        }
        let bytes = w.finish();
        let mut r = Reader::new(&bytes, limits()).unwrap();
        for expected in refs {
            assert_eq!(r.object_ref().unwrap(), expected);
        }
        r.finish().unwrap();
    }

    #[test]
    fn context_decoding_validates_identifiers_versions_and_scope_invariants() {
        let mut w = Writer::new(limits());
        w.string("").unwrap();
        w.u32(1).unwrap();
        let bytes = w.finish();
        assert!(matches!(
            Reader::new(&bytes, limits()).unwrap().reference(),
            Err(SnapshotError::InvalidDefinition(_))
        ));
        let domain = DomainContext::new("d", VersionedId::new("ticks", 1).unwrap()).unwrap();
        let mut w = Writer::new(limits());
        w.domain(&domain).unwrap();
        w.u64(10).unwrap();
        w.u64(9).unwrap();
        let bytes = w.finish();
        assert!(matches!(
            Reader::new(&bytes, limits()).unwrap().relation_context(),
            Err(SnapshotError::InvalidDefinition(_))
        ));
    }
}
