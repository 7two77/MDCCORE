use super::*;
use mdc_core::{
    contract::{dynamics::*, evaluation::ConformanceResult, registry::*, structure::*},
    identity::IdentityProtocol,
    pipeline::*,
    provenance::ObjectRef,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Open,
    High,
    Low,
    Close,
    Volume,
}
impl Field {
    pub fn name(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::High => "high",
            Self::Low => "low",
            Self::Close => "close",
            Self::Volume => "volume",
        }
    }
    pub fn index(self) -> usize {
        match self {
            Self::Open => 0,
            Self::High => 1,
            Self::Low => 2,
            Self::Close => 3,
            Self::Volume => 4,
        }
    }
}
const FIELDS: [Field; 5] = [
    Field::Open,
    Field::High,
    Field::Low,
    Field::Close,
    Field::Volume,
];
pub fn reference(name: &str) -> VersionedId {
    VersionedId::new(name, 1).expect("validated/static profile identifier")
}
fn source(profile: Profile, field: Field) -> VersionedId {
    reference(&format!(
        "binance/spot/{}/1m/{}",
        profile.symbol.text(),
        field.name()
    ))
}
impl MarketEvidence {
    pub fn identity(&self) -> String {
        format!("binance:spot:{}", self.profile.symbol.text())
    }
    pub fn scope(&self) -> &'static str {
        "timeframe:1m"
    }
    pub fn availability_quality(&self) -> &'static str {
        "modeled"
    }
    /// Mechanical scalar projection only. Temporal admission is enforced by
    /// Dataset::decision; manually admitting this request bypasses that profile.
    pub fn observation(&self, field: Field) -> ObservationRequest {
        ObservationRequest {
            source: source(self.profile, field),
            timestamp: Some(self.event_us),
            payload: self.payload.values[field.index()].to_le_bytes().to_vec(),
            acquisition_context: format!(
                "BINANCE-SPOT-1M-MODELED-001/{}/{}",
                self.id,
                field.name()
            ),
        }
    }
}
fn registry(profile: Profile, cutoff: u64) -> Result<Registry> {
    let mut registry = Registry::new();
    let domain = DomainContext::new(
        format!("binance/spot/{}/1m", profile.symbol.text()),
        reference("UTC/unix/microseconds"),
    )?;
    registry.register_schema(SchemaContract {
        reference: reference("market/fixed-e8"),
        format: ScalarSchema::SignedI64Le,
    })?;
    for field in FIELDS {
        registry.register_source(SourceContract {
            reference: source(profile, field),
            schema: reference("market/fixed-e8"),
            context: domain.clone(),
            unit: if field == Field::Volume {
                format!("{}:e8", profile.symbol.base())
            } else {
                format!("USDT/{}:e8", profile.symbol.base())
            },
        })?;
    }
    registry.register_operator(OperatorContract {
        reference: reference("market/read"),
        kind: OperatorKind::Scalar,
    })?;
    registry.register_operator(OperatorContract {
        reference: reference("market/close-delta"),
        kind: OperatorKind::Difference,
    })?;
    let mut p = Vec::new();
    profile.encode(&mut p);
    let frame = RelationContext::new(
        domain,
        TemporalScope::new(0, cutoff)?,
        reference("final-bars/UTC-us"),
        reference(&format!("modeled-availability/{}", digest(&p))),
    );
    registry.register_constraint(ConstraintContract {
        reference: reference("technical/one"),
        kind: ConstraintKind::Cardinality { min: 1, max: 1 },
    })?;
    registry.register_composition(CompositionContract {
        reference: reference("technical/first-close"),
        member_order: MemberOrder::Ordered,
        member_type: MemberType::Measurements,
        constraints: vec![reference("technical/one")],
        context: frame.clone(),
    })?;
    registry.register_state_criterion(StateCriterionContract {
        reference: reference("technical/later-visible-close"),
        kind: StateCriterionKind::ObservationAfter {
            source: source(profile, Field::Close),
        },
    })?;
    registry.register_state(StateContract {
        reference: reference("technical/confirmation"),
        composition: reference("technical/first-close"),
        definition: StateDefinition::CompositionAbstraction,
        criterion_order: CriterionOrder::Ordered,
        criteria: vec![reference("technical/later-visible-close")],
        context: DynamicsContext::new(
            frame,
            reference("MDC-SCALAR-001"),
            reference("BINANCE-SPOT-1M-MODELED-001"),
        ),
    })?;
    Ok(registry)
}
#[derive(Clone, Debug)]
pub struct AdmittedBar {
    evidence_id: String,
    observations: [ObjectRef; 5],
    measurements: [ObjectRef; 5],
}
impl AdmittedBar {
    pub fn evidence_id(&self) -> &str {
        &self.evidence_id
    }
    pub fn observation(&self, field: Field) -> &ObjectRef {
        &self.observations[field.index()]
    }
    pub fn measurement(&self, field: Field) -> &ObjectRef {
        &self.measurements[field.index()]
    }
}
pub struct DecisionView {
    profile: Profile,
    cutoff: u64,
    raw_hash: String,
    duplicates: usize,
    evidence: Vec<MarketEvidence>,
    bars: Vec<AdmittedBar>,
    deltas: Vec<ObjectRef>,
    technical_state: Option<ObjectRef>,
    session: SnapshotSession,
    id: String,
}
impl DecisionView {
    pub fn decision_at(&self) -> u64 {
        self.cutoff
    }
    pub fn decision_id(&self) -> &str {
        &self.id
    }
    pub fn evidence(&self) -> &[MarketEvidence] {
        &self.evidence
    }
    pub fn admitted(&self) -> &[AdmittedBar] {
        &self.bars
    }
    pub fn close_deltas(&self) -> &[ObjectRef] {
        &self.deltas
    }
    pub fn session(&self) -> &SnapshotSession {
        &self.session
    }
    pub fn technical_state(&self) -> Option<&ObjectRef> {
        self.technical_state.as_ref()
    }
    pub fn technical_result(&self) -> Option<ConformanceResult> {
        self.technical_state
            .as_ref()
            .and_then(|r| self.session.pipeline().resolve_state(r))
            .map(|s| s.criteria_result())
    }
    pub fn snapshot_bytes(&self) -> Result<Vec<u8>> {
        export_session(&self.session)
    }
    pub fn snapshot_sha256(&self) -> Result<String> {
        Ok(digest(&self.snapshot_bytes()?))
    }
    /// Fields are fixed enums, numeric values or hex digests; no unescaped caller
    /// string is interpolated into this machine report.
    pub fn report_json(&self) -> Result<String> {
        Ok(self.report_with_digest(&self.snapshot_sha256()?))
    }
    // Only callers in this module supply the hash of their own captured bytes.
    fn report_with_digest(&self, snapshot_digest: &str) -> String {
        use std::fmt::Write;
        let result = match self.technical_result() {
            Some(ConformanceResult::Pass) => "PASS",
            Some(ConformanceResult::Fail) => "FAIL",
            Some(ConformanceResult::Unresolved) => "UNRESOLVED",
            None => "NO_INPUT",
        };
        let mut out=format!("{{\"profile\":\"BINANCE-SPOT-1M-MODELED-001\",\"symbol\":\"{}\",\"timestamp_unit\":\"{}\",\"canonical_time_unit\":\"us\",\"availability_quality\":\"modeled\",\"origin_attestation\":\"provided_csv\",\"lag_us\":{},\"lag_anchor\":\"{}\",\"decision_us\":{},\"decision_id\":\"{}\",\"raw_dataset_sha256\":\"{}\",\"duplicate_rows\":{},\"eligible\":[",self.profile.symbol.text(),self.profile.unit.text(),self.profile.lag_us,if self.profile.anchor==LagAnchor::ReportedClose{"reported_close"}else{"minute_end"},self.cutoff,self.id,self.raw_hash,self.duplicates);
        for (i, e) in self.evidence.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            write!(&mut out,"{{\"evidence_id\":\"{}\",\"event_us\":{},\"available_us\":{},\"evidence_basis_hex\":\"{}\",\"ohlcv_e8\":[{},{},{},{},{}]}}",e.id,e.event_us,e.available_us,hex(&e.canonical),e.payload.open(),e.payload.high(),e.payload.low(),e.payload.close(),e.payload.volume()).expect("String formatting");
        }
        write!(&mut out,"],\"core_operations\":{},\"technical_confirmation\":\"{}\",\"core_snapshot_sha256\":\"{}\"}}",self.session.operation_count(),result,snapshot_digest).expect("String formatting");
        out
    }
}
pub(crate) fn build(dataset: &Dataset, cutoff: u64) -> Result<DecisionView> {
    Ok(capture(dataset, cutoff)?.view)
}
fn export_session(session: &SnapshotSession) -> Result<Vec<u8>> {
    #[cfg(test)]
    crate::work_counts::bump(|c| c.exports += 1);
    Ok(session.export(SnapshotLimits::default())?)
}
// No caller can assemble an unrelated view/snapshot pair or inject a digest.
pub(crate) struct CapturedDecision {
    view: DecisionView,
    snapshot: Vec<u8>,
}
impl CapturedDecision {
    pub(crate) fn into_parts(self) -> (String, String, Vec<u8>) {
        let report = self.view.report_with_digest(&digest(&self.snapshot));
        (self.view.id, report, self.snapshot)
    }
}
pub(crate) fn capture(dataset: &Dataset, cutoff: u64) -> Result<CapturedDecision> {
    let visible = dataset.eligible(cutoff)?;
    let operations = if visible.is_empty() {
        0
    } else {
        visible
            .len()
            .checked_mul(11)
            .and_then(|n| n.checked_add(1))
            .ok_or(Error::Limit("core operations"))?
    };
    if operations > SnapshotLimits::default().max_operations {
        return Err(Error::Limit("core operations"));
    }
    let mut session = SnapshotSession::new(
        registry(dataset.profile, cutoff)?,
        IdentityProtocol::PortableV1,
    );
    let mut bars: Vec<AdmittedBar> = Vec::new();
    let mut deltas = Vec::new();
    let mut key = b"BINANCE-SPOT-1M-MODELED-001/decision/v1\0".to_vec();
    dataset.profile.encode(&mut key);
    key.extend_from_slice(&cutoff.to_le_bytes());
    key.extend_from_slice(&(visible.len() as u64).to_le_bytes());
    for e in &visible {
        key.extend_from_slice(&(e.canonical.len() as u64).to_le_bytes());
        key.extend_from_slice(&e.canonical);
        let mut obs = Vec::new();
        let mut measurements = Vec::new();
        for field in FIELDS {
            let o = session.admit(e.observation(field))?;
            let m = session.measure(MeasurementRequest {
                operator: reference("market/read"),
                inputs: vec![o.clone()],
            })?;
            obs.push(o);
            measurements.push(m);
        }
        let bar = AdmittedBar {
            evidence_id: e.id.clone(),
            observations: obs.try_into().expect("five fixed fields"),
            measurements: measurements.try_into().expect("five fixed fields"),
        };
        if let Some(prev) = bars.last() {
            deltas.push(session.measure(MeasurementRequest {
                operator: reference("market/close-delta"),
                inputs: vec![
                    bar.measurement(Field::Close).clone(),
                    prev.measurement(Field::Close).clone(),
                ],
            })?);
        }
        bars.push(bar);
    }
    let technical_state = if let Some(first) = bars.first() {
        let c = session.compose(CompositionRequest {
            contract: reference("technical/first-close"),
            members: vec![first.measurement(Field::Close).clone()],
            relations: vec![],
        })?;
        Some(session.derive_state(StateRequest {
            contract: reference("technical/confirmation"),
            composition: c,
            observation_horizon: Some(cutoff),
        })?)
    } else {
        None
    };
    let snapshot = export_session(&session)?;
    let view = DecisionView {
        profile: dataset.profile,
        cutoff,
        raw_hash: dataset.raw_hash.clone(),
        duplicates: dataset.duplicates,
        evidence: visible.into_iter().cloned().collect(),
        bars,
        deltas,
        technical_state,
        session,
        id: digest(&key),
    };
    Ok(CapturedDecision { view, snapshot })
}
