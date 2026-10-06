//! Scripted commanders for A/B sweeps (`docs/demo-spec-gameplay.md` §0).
//!
//! A [`Policy`] is a name and a rule for what to do and when. It is the one
//! definition tests, sweeps and (later) the supervisor script share, so "evacuate
//! at T+0" means the same thing in every table. Units are sent to the *head of
//! the fire as it is when the order is given* ([`crate::Run::head`]), the way a
//! commander would, never to a point fixed in advance.

use abm::suppression::UnitKind;
use anyhow::Result;

use crate::run::{Order, Outcome, Run};
use crate::weather::{Draw, ISSUE_2_AT_S};

/// One thing a commander does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    Evacuate,
    /// This many free ground units of this kind, `ahead_m` metres downwind of the
    /// head of the fire (0 = on it; units refuse to work in lethal heat, finding
    /// 41's ember shadow, so *where* is part of the policy).
    Send(UnitKind, usize, f32),
    /// This many aircraft loads, `ahead_m` downwind of the head (requests the
    /// aircraft if need be).
    Drop(usize, f32),
    /// Protect the town rather than fight the front: each unit is sent to the
    /// house cluster nearest the head (spec §4 option B).
    Protect(UnitKind, usize),
    /// Aircraft loads on the house clusters nearest the head.
    ProtectDrop(usize),
    /// This many free ground units of this kind at the most recent spot fire
    /// (does nothing if there has not been one).
    AtSpot(UnitKind, usize),
    /// Warn one district by name.
    WarnDistrict(usize),
    /// Warn the district the wind is blowing the fire toward *now*.
    WarnDownwind,
    /// Warn the district the forecast shift would turn the fire onto.
    WarnShiftSide,
    /// This many engines to defend the district downwind now.
    DefendDownwind(usize),
    /// This many engines to defend this district.
    DefendDistrict(usize, usize),
}

/// How a policy reads the forecast, if it does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rule {
    /// Evacuate when the forecast says the town is at risk: at the briefing
    /// (issue 1) if it already says so, otherwise when issue 2 does. Never
    /// otherwise -- it trusts the forecast blindly, which is the point.
    FollowForecast,
    /// Districts: warn the downwind district at once; warn the district the
    /// forecast shift would threaten when a forecast issue puts the shift at
    /// 50 % or more.
    DistrictForecast,
    /// Districts: when the wind shifts, warn the district it now blows toward.
    ReactToShift,
    /// Districts: warn each district when fire comes within
    /// `district::THREATENED_M` of it (a commander who waits to see).
    WarnWhenThreatened,
}

#[derive(Debug, Clone)]
pub struct Policy {
    pub name: String,
    pub script: Vec<(i64, Act)>,
    pub rule: Option<Rule>,
    /// Further rules, all applied (the district rules combine).
    pub rules: Vec<Rule>,
    /// Acts held back until the head of the fire is within this many metres of
    /// the nearest home -- a commander who waits to see where it is going.
    pub when_near: Vec<(f32, Act)>,
    /// Acts taken each time a new spot fire is reported (spec 5.4).
    pub on_spot: Vec<Act>,
}

impl Policy {
    pub fn named(name: impl Into<String>) -> Policy {
        Policy { name: name.into(), script: vec![], rule: None, rules: vec![], when_near: vec![], on_spot: vec![] }
    }

    /// Everything on the roster at the head at `min`, but each spot fire gets one
    /// engine and one crew sent to it (spec 5.4's split).
    pub fn head_and_spots(min: i64, ahead_m: f32) -> Policy {
        let mut p = Policy::named(format!("head T+{min} + spots"));
        p = p.at(min * 60, Act::Send(UnitKind::Engine, 1, ahead_m)).at(min * 60, Act::Send(UnitKind::HandCrew, 1, ahead_m));
        p.on_spot = vec![Act::AtSpot(UnitKind::Engine, 1), Act::AtSpot(UnitKind::HandCrew, 1)];
        p
    }

    pub fn when_head_within(mut self, metres: f32, a: Act) -> Policy {
        self.when_near.push((metres, a));
        self
    }

    /// Engines to the clusters nearest the head, crews ahead, aircraft on the
    /// clusters -- all sent once the head is `metres` from the nearest home.
    pub fn protect_when_near(metres: f32) -> Policy {
        Policy::named(format!("protect when head <{metres:.0}m"))
            .when_head_within(metres, Act::Protect(UnitKind::Engine, 3))
            .when_head_within(metres, Act::Send(UnitKind::HandCrew, 3, 300.0))
            .when_head_within(metres, Act::ProtectDrop(2))
    }

    pub fn at(mut self, t_s: i64, a: Act) -> Policy {
        self.script.push((t_s, a));
        self.script.sort_by_key(|(t, _)| *t);
        self
    }

    pub fn follow_forecast(mut self) -> Policy {
        self.rule = Some(Rule::FollowForecast);
        self
    }

    pub fn none() -> Policy {
        Policy::named("none")
    }

    pub fn evacuate(min: i64) -> Policy {
        Policy::named(format!("evac T+{min}")).at(min * 60, Act::Evacuate)
    }

    /// Everything on the roster, at the head, `min` minutes in.
    pub fn units(min: i64) -> Policy {
        let mut p = Policy::named(format!("units T+{min}"));
        for a in all_units() {
            p = p.at(min * 60, a);
        }
        p
    }

    /// The roster `ahead_m` downwind of the head (spec §4: where units work).
    pub fn units_ahead(min: i64, ahead_m: f32) -> Policy {
        let mut p = Policy::named(format!("units T+{min} @+{ahead_m:.0}m"));
        for a in all_units_ahead(ahead_m) {
            p = p.at(min * 60, a);
        }
        p
    }

    /// Engines and crews hold the houses nearest the fire; aircraft go ahead.
    pub fn protect(min: i64) -> Policy {
        Policy::named(format!("protect T+{min}"))
            .at(min * 60, Act::Protect(UnitKind::Engine, 3))
            .at(min * 60, Act::Send(UnitKind::HandCrew, 3, 300.0))
            .at(min * 60, Act::ProtectDrop(2))
    }

    pub fn evacuate_and_units(evac_min: i64, units_min: i64) -> Policy {
        let mut p = Policy::named(format!("evac T+{evac_min} + units T+{units_min}")).at(evac_min * 60, Act::Evacuate);
        for a in all_units() {
            p = p.at(units_min * 60, a);
        }
        p
    }

    pub fn forecast_follower() -> Policy {
        Policy::named("follow-forecast").follow_forecast()
    }

    pub fn forecast_follower_and_units(units_min: i64) -> Policy {
        let mut p = Policy::named(format!("follow-forecast + units T+{units_min}")).follow_forecast();
        for a in all_units() {
            p = p.at(units_min * 60, a);
        }
        p
    }

    pub fn with(mut self, r: Rule) -> Policy {
        self.rules.push(r);
        self
    }

    /// Warn the districts the town was built to put at risk -- 0, the one the
    /// opening wind drives the fire at, and 1, the one a forecast shift would --
    /// at `min` minutes. District 2 is upwind in every forecast, by design.
    pub fn warn_at_risk(min: i64) -> Policy {
        Policy::named(format!("at-risk T+{min}")).at(min * 60, Act::WarnDistrict(0)).at(min * 60, Act::WarnDistrict(1))
    }

    /// The district commanders (spec §2): who to warn, not just when.
    pub fn district_set() -> Vec<Policy> {
        vec![
            Policy::none(),
            Policy::evacuate(0),
            Policy::named("downwind T+0").at(0, Act::WarnDownwind),
            Policy::named("downwind T+0 + react to shift").at(0, Act::WarnDownwind).with(Rule::ReactToShift),
            Policy::named("district forecast").with(Rule::DistrictForecast),
            Policy::named("both sides T+0").at(0, Act::WarnDownwind).at(0, Act::WarnShiftSide),
            Policy::named("warn when threatened").with(Rule::WarnWhenThreatened),
        ]
    }

    /// The fixed policies a good design must make lose on average (spec §0).
    pub fn fixed_set() -> Vec<Policy> {
        vec![
            Policy::none(),
            Policy::evacuate(0),
            Policy::evacuate(10),
            Policy::evacuate(20),
            Policy::evacuate(40),
            Policy::forecast_follower(),
        ]
    }

    fn act(run: &mut Run, a: Act) {
        match a {
            Act::Evacuate => run.order(Order::EvacuateAll),
            Act::Send(kind, n, ahead) => {
                for _ in 0..n {
                    let at = run.ahead(ahead);
                    run.order(Order::Attack { kind, at });
                }
            }
            Act::Drop(n, ahead) => {
                for _ in 0..n {
                    let at = run.ahead(ahead);
                    run.order(Order::Drop { at });
                }
            }
            Act::AtSpot(kind, n) => {
                if let Some(at) = run.latest_spot() {
                    for _ in 0..n {
                        run.order(Order::Attack { kind, at });
                    }
                }
            }
            Act::ProtectDrop(n) => {
                for at in run.clusters_near_head(n) {
                    run.order(Order::Drop { at });
                }
            }
            Act::Protect(kind, n) => {
                for at in run.clusters_near_head(n) {
                    run.order(Order::Attack { kind, at });
                }
            }
            Act::WarnDistrict(d) => run.order(Order::EvacuateDistrict(d)),
            Act::WarnDownwind => {
                let to = (run.fire.weather().wind_dir_deg as f32 + 180.0) % 360.0;
                if let Some(d) = run.district_toward(to) {
                    run.order(Order::EvacuateDistrict(d));
                }
            }
            Act::WarnShiftSide => {
                let to = (run.spec.climate.shift_to.wind_dir_deg as f32 + 180.0) % 360.0;
                if let Some(d) = run.district_toward(to) {
                    run.order(Order::EvacuateDistrict(d));
                }
            }
            Act::DefendDistrict(d, n) => {
                for _ in 0..n {
                    run.order(Order::Defend { kind: UnitKind::Engine, district: d });
                }
            }
            Act::DefendDownwind(n) => {
                let to = (run.fire.weather().wind_dir_deg as f32 + 180.0) % 360.0;
                if let Some(d) = run.district_toward(to) {
                    for _ in 0..n {
                        run.order(Order::Defend { kind: UnitKind::Engine, district: d });
                    }
                }
            }
        }
    }

    /// Play a whole mission under this policy. `draw` is the session's weather
    /// (the forecast the policy may read comes from it); the run must have been
    /// built from `draw.spec`.
    pub fn play(&self, run: &mut Run, draw: &Draw) -> Result<Outcome> {
        self.play_observed(run, draw, |_| {})
    }

    /// [`Policy::play`], calling `observe` after every step (for diagnostics).
    pub fn play_observed(&self, run: &mut Run, draw: &Draw, mut observe: impl FnMut(&Run)) -> Result<Outcome> {
        let mut next = 0;
        let mut ruled = false;
        let mut fired: Vec<usize> = vec![];
        let mut seen_events = 0usize;
        let mut judged_issue2 = false;
        let mut seen_shift = 0usize;
        let threatens = draw.spec.climate.shift_threatens;
        let says_risk = |p: f32| if threatens { p >= 0.5 } else { p < 0.5 };
        while run.time_s() < run.spec.duration_s {
            while next < self.script.len() && self.script[next].0 <= run.time_s() {
                Self::act(run, self.script[next].1);
                next += 1;
            }
            if !self.on_spot.is_empty() {
                while seen_events < run.referee.events.len() {
                    let e = run.referee.events[seen_events];
                    seen_events += 1;
                    if e.kind == crate::event::EventKind::SpotFire {
                        for a in &self.on_spot {
                            Self::act(run, *a);
                        }
                    }
                }
            }
            if !self.when_near.is_empty() {
                let d = run.head_to_town_m();
                for (k, (m, a)) in self.when_near.iter().enumerate() {
                    if !fired.contains(&k) && d <= *m {
                        fired.push(k);
                        Self::act(run, *a);
                    }
                }
            }
            if self.rule == Some(Rule::FollowForecast) && !ruled {
                if run.time_s() == 0 && says_risk(draw.forecast(1).shift_p) {
                    ruled = true;
                } else if run.time_s() >= ISSUE_2_AT_S && !judged_issue2 {
                    judged_issue2 = true;
                    ruled = says_risk(draw.forecast(2).shift_p);
                }
                if ruled {
                    Self::act(run, Act::Evacuate);
                }
            }
            for r in &self.rules {
                match r {
                    Rule::DistrictForecast => {
                        if run.time_s() == 0 {
                            Self::act(run, Act::WarnDownwind);
                            if draw.forecast(1).shift_p >= 0.5 {
                                Self::act(run, Act::WarnShiftSide);
                            }
                        } else if run.time_s() >= ISSUE_2_AT_S && !judged_issue2 {
                            judged_issue2 = true;
                            if draw.forecast(2).shift_p >= 0.5 {
                                Self::act(run, Act::WarnShiftSide);
                            }
                        }
                    }
                    Rule::ReactToShift => {
                        let shifted = run.referee.events[seen_shift..].iter().any(|e| matches!(e.kind, crate::event::EventKind::WindShifted { .. }));
                        seen_shift = run.referee.events.len();
                        if shifted {
                            Self::act(run, Act::WarnDownwind);
                        }
                    }
                    Rule::WarnWhenThreatened => {
                        let due: Vec<usize> = run
                            .referee
                            .reports
                            .iter()
                            .enumerate()
                            .filter(|(_, r)| r.warned_at_s.is_none() && r.level() >= crate::district::Level::Threatened)
                            .map(|(k, _)| k)
                            .collect();
                        for d in due {
                            Self::act(run, Act::WarnDistrict(d));
                        }
                    }
                    Rule::FollowForecast => {}
                }
            }
            run.step()?;
            observe(run);
        }
        Ok(run.outcome())
    }
}

/// The whole roster: three crews, three engines, two aircraft loads, all on the head.
pub fn all_units() -> Vec<Act> {
    all_units_ahead(0.0)
}

/// The whole roster, `ahead_m` downwind of the head.
pub fn all_units_ahead(ahead_m: f32) -> Vec<Act> {
    vec![Act::Send(UnitKind::HandCrew, 3, ahead_m), Act::Send(UnitKind::Engine, 3, ahead_m), Act::Drop(2, ahead_m)]
}
