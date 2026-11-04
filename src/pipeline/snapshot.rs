//! Opt-in, exact recorded-session replay snapshots. No mutable Pipeline escape.
use super::*;
use crate::contract::registry::RegistryError;
use crate::identity::IdentityProtocol;
use crate::model::lifecycle::{InvalidationRequest, LifecycleError, LifecycleEventId};
use alloc::vec::Vec;

mod operation;
mod state;
pub(crate) mod wire;
use operation::{Operation, Outcome};
pub use operation::{Operation as SnapshotRequest, Outcome as SnapshotOutcome};
use wire::{Reader, Writer};

const MAGIC: &[u8] = b"MDC-SNAPSHOT-001\0";
const FORMAT_VERSION: u16 = 1;
const ENGINE_VERSION: u16 = 1;

/// Logical codec/replay bounds, not a hard allocator or wall-clock guarantee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotLimits {
    pub max_snapshot_bytes: usize,
    pub max_field_bytes: usize,
    pub max_collection_items: usize,
    pub max_total_items: usize,
    pub max_definitions: usize,
    pub max_operations: usize,
    pub max_manifest_bytes: usize,
}
impl Default for SnapshotLimits {
    fn default() -> Self {
        Self {
            max_snapshot_bytes: 16 * 1024 * 1024,
            max_field_bytes: 2 * 1024 * 1024,
            max_collection_items: 16_384,
            max_total_items: 65_536,
            max_definitions: 4_096,
            max_operations: 4_096,
            max_manifest_bytes: 8 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotError {
    LimitExceeded(&'static str),
    Truncated,
    InvalidMagic,
    UnsupportedFormat(u16),
    UnsupportedEngine(u16),
    UnsupportedProtocol(u8),
    InvalidTag { kind: &'static str, tag: u8 },
    InvalidUtf8,
    InvalidDefinition(RegistryError),
    NonCanonicalRegistry,
    LegacyEnvironmentMismatch,
    OutcomeMismatch { operation: usize },
    StateMismatch,
    TrailingBytes,
    InvalidSessionState(&'static str),
}

struct Entry {
    operation: Operation,
    outcome: Outcome,
}

/// Capture begins with an empty Pipeline and frozen Registry. All mutations
/// pass through this type; the shared Pipeline view supports queries/projection.
/// A snapshot is history-dependent, not a semantic graph identity or durable log.
pub struct SnapshotSession {
    pipeline: Pipeline,
    entries: Vec<Entry>,
}
impl core::fmt::Debug for SnapshotSession {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SnapshotSession")
            .field("identity_protocol", &self.pipeline.identity_protocol())
            .field("operations", &self.entries.len())
            .field("objects", &self.inventory().len())
            .finish()
    }
}

macro_rules! object_operation {
    ($method:ident, $variant:ident, $request:ty) => {
        pub fn $method(&mut self, request: $request) -> Result<ObjectRef, PipelineError> {
            match self.apply(Operation::$variant(request)) {
                Outcome::Object(result) => result,
                Outcome::Lifecycle(_) => unreachable!("object operation outcome"),
            }
        }
    };
}

impl SnapshotSession {
    /// Exact 011 request encoding, shared with the durable host adapter.
    pub fn encode_request(
        request: &SnapshotRequest,
        limits: SnapshotLimits,
    ) -> Result<Vec<u8>, SnapshotError> {
        let mut writer = Writer::new(limits);
        operation::encode_operation(request, &mut writer)?;
        Ok(writer.finish())
    }

    pub fn decode_request(
        bytes: &[u8],
        limits: SnapshotLimits,
    ) -> Result<SnapshotRequest, SnapshotError> {
        let mut reader = Reader::new(bytes, limits)?;
        let request = operation::decode_operation(&mut reader)?;
        reader.finish()?;
        Ok(request)
    }

    /// Obtain a saved call without exposing mutable storage or private fields.
    pub fn recorded_call(
        &self,
        index: usize,
        limits: SnapshotLimits,
    ) -> Result<(Vec<u8>, Vec<u8>), SnapshotError> {
        let entry = self
            .entries
            .get(index)
            .ok_or(SnapshotError::InvalidSessionState("journal index"))?;
        Ok((
            Self::encode_request(&entry.operation, limits)?,
            operation::outcome_bytes(&entry.outcome, limits)?,
        ))
    }

    /// Isolate the call in a fresh candidate. Budget/replay failures preserve self.
    /// Semantic rejection remains a recorded outcome, not an import error.
    pub fn prepare_call(
        &self,
        request: &[u8],
        expected: Option<&[u8]>,
        limits: SnapshotLimits,
    ) -> Result<(Self, Vec<u8>, SnapshotOutcome), SnapshotError> {
        let operation = Self::decode_request(request, limits)?;
        if Self::encode_request(&operation, limits)? != request {
            return Err(SnapshotError::InvalidSessionState("noncanonical request"));
        }
        let mut candidate = Self::restore(&self.export(limits)?, limits)?;
        let outcome = candidate.apply(operation);
        let bytes = operation::outcome_bytes(&outcome, limits)?;
        if expected.is_some_and(|value| value != bytes) {
            return Err(SnapshotError::OutcomeMismatch {
                operation: self.entries.len(),
            });
        }
        candidate.export(limits)?;
        Ok((candidate, bytes, outcome))
    }

    pub fn new(registry: Registry, protocol: IdentityProtocol) -> Self {
        Self {
            pipeline: Pipeline::with_identity_protocol(registry, protocol),
            entries: Vec::new(),
        }
    }
    pub fn pipeline(&self) -> &Pipeline {
        &self.pipeline
    }
    pub fn operation_count(&self) -> usize {
        self.entries.len()
    }
    pub fn recorded_outcome(&self, index: usize) -> Option<SnapshotOutcome> {
        self.entries.get(index).map(|entry| entry.outcome.clone())
    }
    pub fn inventory(&self) -> Vec<ObjectRef> {
        state::inventory(&self.pipeline)
    }
    pub fn state_manifest(&self, limits: SnapshotLimits) -> Result<Vec<u8>, SnapshotError> {
        state::manifest(&self.pipeline, limits)
    }

    object_operation!(admit, Admit, ObservationRequest);
    object_operation!(measure, Measure, MeasurementRequest);
    object_operation!(relate, Relate, RelationRequest);
    object_operation!(compose, Compose, CompositionRequest);
    object_operation!(derive_state, State, StateRequest);
    object_operation!(transition, Transition, TransitionRequest);
    object_operation!(evaluate, Evaluate, EvaluationRequest);
    object_operation!(evaluate_keyed, EvaluateKeyed, EvaluationRequest);

    pub fn invalidate(
        &mut self,
        request: InvalidationRequest,
    ) -> Result<LifecycleEventId, LifecycleError> {
        match self.apply(Operation::Invalidate(request)) {
            Outcome::Lifecycle(result) => result,
            Outcome::Object(_) => unreachable!("lifecycle operation outcome"),
        }
    }

    #[cfg(test)]
    pub(super) fn force_evaluation_digest_for_test(&mut self, value: u64) {
        self.pipeline.evaluation_digest_override = Some(value);
    }
    #[cfg(test)]
    pub(super) fn restore_with_digest_for_test(
        bytes: &[u8],
        limits: SnapshotLimits,
        value: u64,
    ) -> Result<Self, SnapshotError> {
        Self::restore_internal(bytes, limits, Some(value))
    }

    fn apply(&mut self, operation: Operation) -> Outcome {
        let outcome = match &operation {
            Operation::Admit(r) => Outcome::Object(self.pipeline.admit(r.clone())),
            Operation::Measure(r) => Outcome::Object(self.pipeline.measure(r.clone())),
            Operation::Relate(r) => Outcome::Object(self.pipeline.relate(r.clone())),
            Operation::Compose(r) => Outcome::Object(self.pipeline.compose(r.clone())),
            Operation::State(r) => Outcome::Object(self.pipeline.derive_state(r.clone())),
            Operation::Transition(r) => Outcome::Object(self.pipeline.transition(r.clone())),
            Operation::Evaluate(r) => Outcome::Object(self.pipeline.evaluate(r.clone())),
            Operation::EvaluateKeyed(r) => Outcome::Object(self.pipeline.evaluate_keyed(r.clone())),
            Operation::Invalidate(r) => Outcome::Lifecycle(self.pipeline.invalidate(r.clone())),
        };
        self.entries.push(Entry {
            operation,
            outcome: outcome.clone(),
        });
        outcome
    }

    /// Export failure preserves the session. Large sessions may require larger
    /// explicit bounds; live recording has the core's in-memory allocation model.
    pub fn export(&self, limits: SnapshotLimits) -> Result<Vec<u8>, SnapshotError> {
        let mut w = Writer::new(limits);
        w.raw(MAGIC)?;
        w.u16(FORMAT_VERSION)?;
        w.u16(ENGINE_VERSION)?;
        match self.pipeline.identity_protocol() {
            IdentityProtocol::LegacyRustHashFnv64 => {
                w.u8(0)?;
                w.u8(usize::BITS as u8)?;
                w.u8(u8::from(cfg!(target_endian = "big")))?;
            }
            IdentityProtocol::PortableV1 => w.u8(1)?,
        }
        crate::contract::registry::snapshot::encode_registry(&self.pipeline.registry, &mut w)?;
        w.operations(self.entries.len())?;
        for entry in &self.entries {
            operation::encode_operation(&entry.operation, &mut w)?;
            let outcome = operation::outcome_bytes(&entry.outcome, limits)?;
            w.bytes(&outcome)?;
        }
        w.manifest(&self.state_manifest(limits)?)?;
        Ok(w.finish())
    }

    /// Decode completely before replay. Restore builds a fresh candidate and
    /// fails closed on any differing historical outcome or final state.
    pub fn restore(bytes: &[u8], limits: SnapshotLimits) -> Result<Self, SnapshotError> {
        Self::restore_internal(bytes, limits, None)
    }

    fn restore_internal(
        bytes: &[u8],
        limits: SnapshotLimits,
        digest_override: Option<u64>,
    ) -> Result<Self, SnapshotError> {
        let mut r = Reader::new(bytes, limits)?;
        if r.raw(MAGIC.len())? != MAGIC {
            return Err(SnapshotError::InvalidMagic);
        }
        let version = r.u16()?;
        if version != FORMAT_VERSION {
            return Err(SnapshotError::UnsupportedFormat(version));
        }
        let engine = r.u16()?;
        if engine != ENGINE_VERSION {
            return Err(SnapshotError::UnsupportedEngine(engine));
        }
        let protocol = match r.u8()? {
            0 => {
                if r.u8()? != usize::BITS as u8 || r.u8()? != u8::from(cfg!(target_endian = "big"))
                {
                    return Err(SnapshotError::LegacyEnvironmentMismatch);
                }
                IdentityProtocol::LegacyRustHashFnv64
            }
            1 => IdentityProtocol::PortableV1,
            value => return Err(SnapshotError::UnsupportedProtocol(value)),
        };
        let registry = crate::contract::registry::snapshot::decode_registry(&mut r)?;
        let count = r.operations()?;
        let mut operations = Vec::new();
        for _ in 0..count {
            operations.push((operation::decode_operation(&mut r)?, r.bytes()?));
        }
        let expected_state = r.manifest()?;
        r.finish()?;
        let mut session = Self::new(registry, protocol);
        #[cfg(test)]
        {
            session.pipeline.evaluation_digest_override = digest_override;
        }
        #[cfg(not(test))]
        {
            let _ = digest_override;
        }
        for (index, (operation, expected)) in operations.into_iter().enumerate() {
            let outcome = session.apply(operation);
            if operation::outcome_bytes(&outcome, limits)? != expected {
                return Err(SnapshotError::OutcomeMismatch { operation: index });
            }
            // Stop further replay when retained logical state exceeds bounds.
            session.state_manifest(limits)?;
        }
        if session.state_manifest(limits)? != expected_state {
            return Err(SnapshotError::StateMismatch);
        }
        Ok(session)
    }
}
