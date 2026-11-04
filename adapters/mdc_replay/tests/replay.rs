use mdc_core::{
    contract::evaluation::{ConformanceResult as R, LifecycleStatus},
    pipeline::{SnapshotLimits, SnapshotSession, StateRequest},
};
use mdc_replay::*;
const MS: &str = include_str!("fixtures/BTCUSDT-1m-2024-01-01-first3.csv");
const US: &str = include_str!("fixtures/BTCUSDT-1m-2025-01-01-first3.csv");
fn p(unit: TimestampUnit) -> Profile {
    Profile::new(Symbol::BtcUsdt, unit, 100, LagAnchor::ReportedClose).unwrap()
}
fn d() -> Dataset {
    Dataset::from_csv(
        MS.as_bytes(),
        p(TimestampUnit::Milliseconds),
        Limits::default(),
    )
    .unwrap()
}
fn row_replace(input: &str, column: usize, value: &str) -> String {
    let mut cols: Vec<_> = input
        .lines()
        .next()
        .unwrap()
        .split(',')
        .map(String::from)
        .collect();
    cols[column] = value.into();
    cols.join(",") + "\n"
}
#[test]
fn exact_decimals_preserve_small_values_and_integer_boundary() {
    for (s, v) in [
        ("0", 0),
        ("0.00000001", 1),
        ("1.2300000000", 123000000),
        ("00.10000000", 10000000),
        ("92233720368.54775807", i64::MAX),
    ] {
        assert_eq!(decimal_e8(s), Ok(v));
    }
    assert_eq!(decimal_e8("92233720368.54775808"), Err(RowIssue::Overflow));
    assert_eq!(decimal_e8("0.123456789"), Err(RowIssue::Precision));
}
#[test]
fn no_float_nan_infinity_rounding_or_zero_fallback() {
    for bad in [
        "NaN", "inf", "1e3", "-1", "+1", " 1", "1 ", ".1", "1.", "1..0", "",
    ] {
        assert!(decimal_e8(bad).is_err(), "{bad}");
        let input = row_replace(MS, 4, bad);
        assert!(Dataset::from_csv(
            input.as_bytes(),
            p(TimestampUnit::Milliseconds),
            Limits::default()
        )
        .is_err());
    }
}
#[test]
fn official_ms_and_us_fixtures_have_declared_units_and_modeled_lag() {
    for (input, unit, event, available) in [
        (
            MS,
            TimestampUnit::Milliseconds,
            1704067259999000,
            1704067260099000,
        ),
        (
            US,
            TimestampUnit::Microseconds,
            1735689659999999,
            1735689660099999,
        ),
    ] {
        let ds = Dataset::from_csv(input.as_bytes(), p(unit), Limits::default()).unwrap();
        let e = &ds.evidence()[0];
        assert_eq!(e.event_timestamp(), event);
        assert_eq!(e.available_at(), available);
        assert_eq!(e.availability_quality(), "modeled");
        assert_eq!(e.scope(), "timeframe:1m");
        assert_eq!(e.identity(), "binance:spot:BTCUSDT");
    }
    assert_eq!(d().evidence()[0].payload().close(), 4_229_861_000_000);
}
#[test]
fn wrong_or_mixed_timestamp_units_do_not_autoguess() {
    for (input, unit) in [
        (MS, TimestampUnit::Microseconds),
        (US, TimestampUnit::Milliseconds),
    ] {
        assert!(Dataset::from_csv(input.as_bytes(), p(unit), Limits::default()).is_err());
    }
    let mixed = format!(
        "{}\n{}\n",
        MS.lines().next().unwrap(),
        US.lines().next().unwrap()
    );
    assert!(Dataset::from_csv(
        mixed.as_bytes(),
        p(TimestampUnit::Milliseconds),
        Limits::default()
    )
    .is_err());
}
#[test]
fn reported_close_zero_lag_is_explicitly_rejected_and_minute_end_zero_is_safe() {
    for unit in [TimestampUnit::Milliseconds, TimestampUnit::Microseconds] {
        assert!(Profile::new(Symbol::BtcUsdt, unit, 0, LagAnchor::ReportedClose).is_err());
    }
    let profile = Profile::new(
        Symbol::BtcUsdt,
        TimestampUnit::Microseconds,
        0,
        LagAnchor::MinuteEnd,
    )
    .unwrap();
    let ds = Dataset::from_csv(US.as_bytes(), profile, Limits::default()).unwrap();
    let e = &ds.evidence()[0];
    assert_eq!(e.event_timestamp(), 1735689659999999);
    assert_eq!(e.available_at(), 1735689660000000);
    assert!(ds
        .decision(e.available_at() - 1)
        .unwrap()
        .evidence()
        .is_empty());
    assert_eq!(ds.decision(e.available_at()).unwrap().evidence().len(), 1);
}
#[test]
fn malformed_schema_header_quotes_and_empty_input_fail_closed() {
    assert!(matches!(
        Dataset::from_csv(b"", p(TimestampUnit::Milliseconds), Limits::default()),
        Err(Error::EmptyInput)
    ));
    for input in [
        "open_time,open,high,low,close,volume,close_time",
        "1,2,3",
        "\n",
    ] {
        assert!(Dataset::from_csv(
            input.as_bytes(),
            p(TimestampUnit::Milliseconds),
            Limits::default()
        )
        .is_err());
    }
    assert!(Dataset::from_csv(
        row_replace(MS, 1, "\"42283.58\"").as_bytes(),
        p(TimestampUnit::Milliseconds),
        Limits::default()
    )
    .is_err());
    assert!(matches!(
        Dataset::from_csv(&[255], p(TimestampUnit::Milliseconds), Limits::default()),
        Err(Error::Utf8)
    ));
}
#[test]
fn bad_bar_geometry_volumes_precision_and_timestamps_are_diagnosed() {
    for (column, value) in [
        (1, "0"),
        (2, "1"),
        (3, "999999"),
        (5, "-1"),
        (9, "999999"),
        (10, "9999999999"),
        (4, "1.000000001"),
        (8, "-1"),
        (0, "1704067200001"),
        (6, "1704067259998"),
    ] {
        assert!(
            Dataset::from_csv(
                row_replace(MS, column, value).as_bytes(),
                p(TimestampUnit::Milliseconds),
                Limits::default()
            )
            .is_err(),
            "column {column}"
        );
    }
}
#[test]
fn arithmetic_overflow_is_checked_for_timestamps_and_lag() {
    assert!(Profile::new(
        Symbol::BtcUsdt,
        TimestampUnit::Milliseconds,
        u64::MAX,
        LagAnchor::MinuteEnd
    )
    .is_err());
    let lag = u64::MAX / 1000;
    let profile = Profile::new(
        Symbol::BtcUsdt,
        TimestampUnit::Milliseconds,
        lag,
        LagAnchor::ReportedClose,
    )
    .unwrap();
    assert!(Dataset::from_csv(MS.as_bytes(), profile, Limits::default()).is_err());
    assert!(Dataset::from_csv(
        row_replace(MS, 0, &u64::MAX.to_string()).as_bytes(),
        p(TimestampUnit::Milliseconds),
        Limits::default()
    )
    .is_err());
}
#[test]
fn before_at_and_after_cutoff_enforce_receipt_before_decision_equality_rule() {
    let ds = d();
    let at = ds.evidence()[0].available_at();
    let before = ds.decision(at - 1).unwrap();
    assert_eq!(before.evidence().len(), 0);
    assert_eq!(before.technical_result(), None);
    assert_eq!(before.session().operation_count(), 0);
    let view = ds.decision(at).unwrap();
    assert_eq!(view.evidence().len(), 1);
    assert_eq!(view.technical_result(), Some(R::Unresolved));
    assert_eq!(view.session().operation_count(), 12);
    assert_eq!(ds.decision(at + 1).unwrap().evidence().len(), 1);
}
#[test]
fn data_and_operators_use_real_core_scalars_and_ordered_difference() {
    let ds = d();
    let view = ds.decision(ds.evidence()[2].available_at()).unwrap();
    assert_eq!(view.session().operation_count(), 34);
    for (e, b) in view.evidence().iter().zip(view.admitted()) {
        for field in [
            Field::Open,
            Field::High,
            Field::Low,
            Field::Close,
            Field::Volume,
        ] {
            let o = view
                .session()
                .pipeline()
                .resolve_observation(b.observation(field))
                .unwrap();
            let m = view
                .session()
                .pipeline()
                .resolve_measurement(b.measurement(field))
                .unwrap();
            assert_eq!(o.timestamp(), e.event_timestamp());
            assert_eq!(
                o.payload(),
                e.payload().values()[field.index()].to_le_bytes()
            );
            assert_eq!(m.value(), i128::from(e.payload().values()[field.index()]));
            assert!(o
                .provenance()
                .acquisition_context()
                .contains(e.evidence_id()));
        }
    }
    let first = view
        .session()
        .pipeline()
        .resolve_measurement(&view.close_deltas()[0])
        .unwrap();
    assert_eq!(first.value(), 2_139_000_000);
    assert_eq!(view.technical_result(), Some(R::Pass));
    assert_eq!(
        view.session()
            .pipeline()
            .resolve_state(view.technical_state().unwrap())
            .unwrap()
            .status(),
        LifecycleStatus::Candidate
    );
}
#[test]
fn a_stored_event_in_horizon_but_not_yet_available_cannot_be_an_implicit_witness() {
    let ds = d();
    let cutoff = ds.evidence()[1].event_timestamp() + 50_000;
    assert!(cutoff < ds.evidence()[1].available_at());
    let view = ds.decision(cutoff).unwrap();
    assert_eq!(view.technical_result(), Some(R::Unresolved));
    assert_eq!(view.evidence().len(), 1);
    // Counterfactual: the unfiltered current core would find this later event.
    let mut contaminated =
        SnapshotSession::restore(&view.snapshot_bytes().unwrap(), SnapshotLimits::default())
            .unwrap();
    contaminated
        .admit(ds.evidence()[1].observation(Field::Close))
        .unwrap();
    let state = view
        .session()
        .pipeline()
        .resolve_state(view.technical_state().unwrap())
        .unwrap();
    let extra = contaminated
        .derive_state(StateRequest {
            contract: state.contract().clone(),
            composition: state.source_composition().clone(),
            observation_horizon: Some(cutoff),
        })
        .unwrap();
    assert_eq!(
        contaminated
            .pipeline()
            .resolve_state(&extra)
            .unwrap()
            .criteria_result(),
        R::Pass
    );
    assert_eq!(view.technical_result(), Some(R::Unresolved));
}
#[test]
fn adding_a_late_suffix_does_not_rebind_early_core_bytes_or_decision_id() {
    let all = d();
    let prefix = Dataset::from_csv(
        (MS.lines().next().unwrap().to_string() + "\n").as_bytes(),
        p(TimestampUnit::Milliseconds),
        Limits::default(),
    )
    .unwrap();
    let cutoff = all.evidence()[0].available_at();
    let a = all.decision(cutoff).unwrap();
    let b = prefix.decision(cutoff).unwrap();
    assert_ne!(all.raw_sha256(), prefix.raw_sha256());
    assert_eq!(a.decision_id(), b.decision_id());
    assert_eq!(a.snapshot_bytes().unwrap(), b.snapshot_bytes().unwrap());
}
#[test]
fn input_permutation_duplicates_and_equivalent_decimal_text_are_canonical() {
    let ds = d();
    let cutoff = ds.evidence()[2].available_at();
    let reversed = MS.lines().rev().collect::<Vec<_>>().join("\n") + "\n";
    let dup = MS.to_string() + MS.lines().next().unwrap() + "\n";
    for (input, count) in [
        (reversed, 0),
        (dup, 1),
        (MS.replace("42298.61000000", "42298.6100000000"), 0),
    ] {
        let other = Dataset::from_csv(
            input.as_bytes(),
            p(TimestampUnit::Milliseconds),
            Limits::default(),
        )
        .unwrap();
        assert_eq!(other.duplicate_rows(), count);
        assert_eq!(
            ds.decision(cutoff).unwrap().snapshot_bytes().unwrap(),
            other.decision(cutoff).unwrap().snapshot_bytes().unwrap()
        );
    }
}
#[test]
fn conflicting_same_bar_is_not_silently_treated_as_a_late_correction() {
    let conflict = MS.to_string() + &row_replace(MS, 4, "42283.58000000");
    assert!(matches!(
        Dataset::from_csv(
            conflict.as_bytes(),
            p(TimestampUnit::Milliseconds),
            Limits::default()
        ),
        Err(Error::ConflictingBar { .. })
    ));
}
#[test]
fn model_symbol_and_anchor_are_explicit_identity_context() {
    let base = d();
    let eth = Dataset::from_csv(
        MS.as_bytes(),
        Profile::new(
            Symbol::EthUsdt,
            TimestampUnit::Milliseconds,
            100,
            LagAnchor::ReportedClose,
        )
        .unwrap(),
        Limits::default(),
    )
    .unwrap();
    let lag = Dataset::from_csv(
        MS.as_bytes(),
        Profile::new(
            Symbol::BtcUsdt,
            TimestampUnit::Milliseconds,
            200,
            LagAnchor::ReportedClose,
        )
        .unwrap(),
        Limits::default(),
    )
    .unwrap();
    let end = Dataset::from_csv(
        MS.as_bytes(),
        Profile::new(
            Symbol::BtcUsdt,
            TimestampUnit::Milliseconds,
            100,
            LagAnchor::MinuteEnd,
        )
        .unwrap(),
        Limits::default(),
    )
    .unwrap();
    for other in [&eth, &lag, &end] {
        assert_ne!(
            base.evidence()[0].evidence_id(),
            other.evidence()[0].evidence_id()
        );
    }
    assert_eq!(
        end.evidence()[0].available_at() - base.evidence()[0].available_at(),
        1000
    );
}
#[test]
fn bounds_reject_instead_of_silently_truncating_input_or_visible_history() {
    for limits in [
        Limits {
            max_input_bytes: 1,
            ..Limits::default()
        },
        Limits {
            max_rows: 2,
            ..Limits::default()
        },
        Limits {
            max_line_bytes: 10,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            Dataset::from_csv(MS.as_bytes(), p(TimestampUnit::Milliseconds), limits),
            Err(Error::Limit(_))
        ));
    }
    let ds = Dataset::from_csv(
        MS.as_bytes(),
        p(TimestampUnit::Milliseconds),
        Limits {
            max_visible_bars: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    assert!(matches!(
        ds.decision(ds.evidence()[2].available_at()),
        Err(Error::Limit("visible bars"))
    ));
}
#[test]
fn raising_visible_limit_does_not_bypass_the_core_call_budget() {
    let original: Vec<_> = MS
        .lines()
        .next()
        .unwrap()
        .split(',')
        .map(String::from)
        .collect();
    let mut csv = String::new();
    for i in 0..400u64 {
        let mut row = original.clone();
        let open = 1704067200000u64 + i * 60000;
        row[0] = open.to_string();
        row[6] = (open + 59999).to_string();
        csv.push_str(&(row.join(",") + "\n"));
    }
    let ds = Dataset::from_csv(
        csv.as_bytes(),
        p(TimestampUnit::Milliseconds),
        Limits {
            max_visible_bars: 4096,
            ..Limits::default()
        },
    )
    .unwrap();
    assert!(matches!(
        ds.decision(ds.evidence().last().unwrap().available_at()),
        Err(Error::Limit("core operations"))
    ));
}
