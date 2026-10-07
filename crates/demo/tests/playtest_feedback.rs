//! Regressions from the 2026-10-07 blind follow-up, using its recorded orders.
use demo::*;
fn new(seed: u64) -> Session {
    Session::new(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        seed,
    )
    .unwrap()
}
fn turn(s: &mut Session, orders: &[(TokenId, u8)]) {
    for &(t, g) in orders {
        s.assign(t, TargetId(g)).unwrap();
    }
    s.end_turn().unwrap();
}
#[test]
fn recorded_session_three_plane_breaks_off_after_timely_arrival() {
    use TokenId::*;
    let mut s = new(261793);
    turn(&mut s, &[(P, 1), (E1, 4), (E2, 1), (S, 2)]);
    assert_eq!(s.ongoing_work(E1), Some(OngoingWork::RoadsideAttack));
    assert!(s.token(E1).water.unwrap() < 0.6);
    assert!(s.preparing(0) > 0);
    turn(&mut s, &[(P, 2), (E1, 2), (K, 200)]);
    assert!(s.reinforcement_arrived());
    turn(&mut s, &[(E3, 1)]);
    assert_eq!(s.forecast_status(), ForecastStatus::Observed);
    turn(&mut s, &[(E3, 2)]);
    assert_eq!(
        s.preview_reason(K, TargetId(1)),
        Some(PreviewReason::OutsideDirection {
            level: Level::Threatened
        })
    );
    turn(&mut s, &[(K, 2)]);
    s.finish().unwrap();
    let h = s.aircraft_history();
    println!("aircraft history: {h:?}, outcome {:?}", s.aircraft_outcome());
    assert_eq!(h.called_at_s, Some(480));
    assert_eq!(h.arrived_at_s, Some(1980));
    assert_eq!(h.broke_off_at_s, Some(2016));
    assert_eq!(h.drops, 0);
    assert_eq!(s.aircraft_outcome(), AircraftOutcome::UnsafeBreakOff);
    assert!(h.broke_off_at_s.is_some());
    let v = s.verdict().unwrap();
    assert_eq!(
        (v.families_caught, v.none.families_caught, v.homes_hit, v.none.homes_hit),
        (14, 17, 39, 78)
    );
    assert!(!v.notes.contains(&Note::CanadairTardi));
    assert_eq!(v.districts[1].warning.reason, WarningReason::InsufficientLead);
    assert!(v.districts[1].caught < v.districts[1].caught_none);
}
#[test]
fn elapsed_forecast_stops_advising_shift_path_without_rewriting_history() {
    let mut s = new(656166);
    assert_eq!(s.forecast_status(), ForecastStatus::Pending);
    assert_eq!(s.risk_direction(2), RiskDirection::Outside);
    assert!(s.at_risk(1));
    turn(&mut s, &[(TokenId::P, 2)]);
    for _ in 0..3 {
        s.end_turn().unwrap();
    }
    assert_eq!(s.forecast_status(), ForecastStatus::Elapsed);
    assert_eq!(s.risk_direction(1), RiskDirection::Outside);
    assert_eq!(s.stamp(1), Stamp::Prudente);
}

#[test]
fn fixed_orders_preserve_both_hold_wind_baselines_and_latched_refill_coverage() {
    use TokenId::*;
    for retry in [false, true] {
        let mut s = new(656166);
        turn(
            &mut s,
            &[
                (P, if retry { 2 } else { 1 }),
                (E1, 1),
                (E2, if retry { 1 } else { 2 }),
                (S, 1),
                (K, 200),
            ],
        );
        turn(&mut s, &[(P, if retry { 1 } else { 2 })]);
        assert!(s.reinforcement_arrived());
        assert!(s
            .decisions()
            .iter()
            .any(|d| matches!(d, Decision::AircraftArrival { can_target: false, .. })));
        turn(&mut s, &if retry { vec![(E3, 1)] } else { vec![(P, 3), (E3, 3)] });
        let mut refilled_post_checked = false;
        s.assign(K, TargetId(1)).unwrap();
        s.end_turn_observed(|s| {
            for id in [E1, E2, E3] {
                if matches!(s.token_state(id), TokenState::Rifornimento { .. }) {
                    if let Some(post) = s.run.referee.tally.defence_post(id.unit().unwrap()) {
                        if matches!(s.token(id).doing, Some(TargetKind::District(_))) {
                            let homes = s
                                .run
                                .agents
                                .households
                                .iter()
                                .filter(|h| (h.home.x - post.x).hypot(h.home.y - post.y) <= demo::run::DEFEND_REACH_M)
                                .count() as u32;
                            assert_eq!(s.ongoing_work(id), Some(OngoingWork::HomeCoverage { homes }));
                            refilled_post_checked = true;
                        }
                    }
                }
            }
        })
        .unwrap();
        assert!(
            refilled_post_checked,
            "regression must exercise automatic refill with a latched post"
        );
        assert!(s.drops().0 > 0);
        assert_eq!(
            s.ongoing_work(K),
            Some(OngoingWork::AircraftDrops { drops: s.drops().0 })
        );
        s.finish().unwrap();
        let v = s.verdict().unwrap();
        assert_eq!(
            (
                v.families_caught,
                v.none.families_caught,
                v.homes_hit,
                v.none.homes_hit,
                s.drops().0
            ),
            if retry {
                (18, 18, 18, 76, 5)
            } else {
                (10, 18, 36, 76, 5)
            }
        );
    }
}
#[test]
fn preview_reasons_distinguish_warning_and_direction_and_account_for_overlap() {
    use TokenId::*;
    let mut s = new(656166);
    assert_eq!(
        s.preview_reason(E1, TargetId(3)),
        Some(PreviewReason::OutsideDirection { level: Level::Calm })
    );
    assert_eq!(s.preview_reason(E1, TargetId(4)), Some(PreviewReason::RoadOutOfReach));
    s.assign(E1, TargetId(1)).unwrap();
    let c = s.coverage_explanation(E2, TargetId(1)).unwrap();
    assert_eq!(c.homes, c.additional + c.already_covered);
    turn(&mut s, &[(P, 1)]);
    assert_eq!(s.preview_reason(P, TargetId(1)), Some(PreviewReason::AlreadyWarned));
    assert_eq!(
        s.preparing(0),
        s.run.referee.districts[0]
            .households
            .iter()
            .filter(|&&i| s.run.agents.households[i].status == scenario::population::Status::Preparing)
            .count() as u32
    );
}
