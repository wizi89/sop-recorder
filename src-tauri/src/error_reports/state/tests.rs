use super::*;
use crate::error_reports::global_state_guard;

/// `Phase::ALL` is the one list the atomic and the parser read; a phase
/// missing from it would be stored and come back as `Unknown`.
#[test]
fn every_phase_survives_the_atomic_and_its_name() {
    let _guard = global_state_guard();
    assert_eq!(Phase::ALL.len(), Phase::Unknown as usize + 1);
    for (i, phase) in Phase::ALL.into_iter().enumerate() {
        assert_eq!(phase as usize, i);
        assert_eq!(Phase::from_str_or_unknown(phase.as_str()), phase);
        let wire = serde_json::to_string(&phase).unwrap();
        assert_eq!(wire, format!("\"{}\"", phase.as_str()));
        set_phase(phase);
        assert_eq!(super::phase(), phase);
    }
    assert_eq!(Phase::from_str_or_unknown("hochladen"), Phase::Unknown);
    set_phase(Phase::Startup);
}

#[test]
fn every_mode_survives_the_atomic_and_its_name() {
    let _guard = global_state_guard();
    for mode in ReportMode::ALL {
        assert_eq!(resolve_mode(Some(mode.as_str()), false), mode);
        set_mode(mode);
        assert_eq!(super::mode(), mode);
    }
    set_mode(ReportMode::Ask);
}
