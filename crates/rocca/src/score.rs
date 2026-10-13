//! The score, the stars and the day's board. Since 2026-10-13 the kiosk
//! shows stars ([`Stars`]) and the day's best per level ([`Board`]); the 0-1000 score stays for the headless comparisons.
//!
//! The score is made of the debrief's own numbers, so the screen and the
//! points never disagree: of the families the same fire with no orders
//! would catch at home ([`Game::without_orders`]), the share the player
//! spared; of the homes it would hit, the share left standing. Shares, not
//! counts, so one board serves every fire: a small fire played well beats a
//! large one played badly (playtest 5: 2 / 12 scored below 3 / 71). Nothing
//! here reads the simulation: it only weighs two [`Outcome`]s.
//!
//! [`Game::without_orders`]: crate::Game::without_orders

use serde::{Deserialize, Serialize};

use crate::Outcome;

/// The best score: everything the fire would have taken, spared.
pub const MAX: i64 = 1000;
/// Weight of the families' share; the homes take the rest. People first.
pub const FAMILY_WEIGHT: f64 = 0.7;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Score {
    /// Families the fire would catch at home with no orders, and how many
    /// of them the player spared (never below 0).
    pub families_at_risk: usize,
    pub families: usize,
    /// The same for homes.
    pub homes_at_risk: usize,
    pub homes: usize,
    pub total: i64,
}

/// The player's game against the same fire with no orders.
pub fn score(mine: &Outcome, without: &Outcome) -> Score {
    let spared = |risk: usize, lost: usize| risk.saturating_sub(lost);
    let (fr, hr) = (without.caught(), without.homes_hit());
    let (f, h) = (spared(fr, mine.caught()), spared(hr, mine.homes_hit()));
    // a part with nothing at risk drops out and the other takes its weight
    let parts: Vec<(f64, f64)> = [(fr, f, FAMILY_WEIGHT), (hr, h, 1.0 - FAMILY_WEIGHT)]
        .into_iter()
        .filter(|(risk, _, _)| *risk > 0)
        .map(|(risk, kept, w)| (kept as f64 / risk as f64, w))
        .collect();
    let weight: f64 = parts.iter().map(|(_, w)| w).sum();
    let share = if weight > 0.0 { parts.iter().map(|(s, w)| s * w).sum::<f64>() / weight } else { 0.0 };
    Score { families_at_risk: fr, families: f, homes_at_risk: hr, homes: h, total: (share * MAX as f64).round() as i64 }
}

/// The game's summary in stars (docs/01-GIOCO.md, 2026-10-13): Persone,
/// Case, Tempismo, Misura, 1 to 5 each; `None` where the fire put nothing at
/// stake (no family or home at risk, no district reached).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stars {
    pub persone: Option<u8>,
    pub case: Option<u8>,
    pub tempismo: Option<u8>,
    pub misura: u8,
}

/// The share each further star needs, per category: from the headless
/// strategies over 82 fires, seeds 1-3 (`examples/sindaca.rs`,
/// `docs/fase6/sindaca`). No orders earns 1 star in Persone, a good plan
/// about 4; evacuating everyone at once, 1 in Misura.
pub const PERSONE: [f64; 4] = [0.2, 0.45, 0.65, 0.85];
pub const CASE: [f64; 4] = [0.1, 0.25, 0.45, 0.7];
pub const TEMPISMO: [f64; 4] = [0.25, 0.5, 0.75, 0.9];
pub const MISURA: [f64; 4] = [0.4, 0.6, 0.8, 0.95];

/// 1 to 5 stars for a share of 0-1: one more for each threshold it reaches.
pub fn stars_of(share: f64, thresholds: [f64; 4]) -> u8 {
    1 + thresholds.iter().filter(|&&t| share >= t).count() as u8
}

/// The stars from the score against the same fire with no orders and the
/// game's own [`crate::Game::timing`] and [`crate::Game::measure`].
pub fn stars(s: &Score, timing: Option<f64>, measure: f64) -> Stars {
    let share = |kept: usize, risk: usize, t| (risk > 0).then(|| stars_of(kept as f64 / risk as f64, t));
    Stars {
        persone: share(s.families, s.families_at_risk, PERSONE),
        case: share(s.homes, s.homes_at_risk, CASE),
        tempismo: timing.map(|x| stars_of(x, TEMPISMO)),
        misura: stars_of(measure, MISURA),
    }
}

/// One finished game on the day's board (docs/01-GIOCO.md, 2026-10-13): the
/// name, the level (stars are not comparable between levels), the stars and
/// when it was played.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Played {
    pub name: String,
    pub level: String,
    /// Persone, Case, Tempismo, Misura; `None` where nothing was at stake.
    pub stars: [Option<u8>; 4],
    /// The local date, `YYYY-MM-DD`: the board shows only today's games.
    #[serde(default)]
    pub day: String,
    /// Seconds since the Unix epoch: the earlier game wins a tie.
    #[serde(default)]
    pub at: u64,
}

impl Stars {
    /// Persone, Case, Tempismo, Misura.
    pub fn all(&self) -> [Option<u8>; 4] {
        [self.persone, self.case, self.tempismo, Some(self.misura)]
    }
}

impl Played {
    /// The sum of the four categories' stars (a category with nothing at
    /// stake counts nothing: every player of a level faces the same fire).
    pub fn total(&self) -> u32 {
        self.stars.iter().flatten().map(|&s| s as u32).sum()
    }

    /// Persone stars, the first tie-break.
    pub fn persone(&self) -> u32 {
        self.stars[0].map_or(0, |s| s as u32)
    }
}

/// Games kept per level and day (a kiosk day never gets near it).
pub const BOARD_KEEP: usize = 50;
/// Rows the screens show per level.
pub const BOARD_SHOWN: usize = 8;

/// The best of the day, per level. Ranking: total stars, then Persone
/// stars, then the earlier game.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub entries: Vec<Played>,
}

impl Board {
    /// Add a game. Games of other days are dropped, the board being «del
    /// giorno»; each level keeps its best [`BOARD_KEEP`].
    pub fn push(&mut self, p: Played) {
        let day = p.day.clone();
        self.entries.retain(|e| e.day == day);
        self.entries.push(p);
        let mut levels: Vec<String> = self.entries.iter().map(|e| e.level.clone()).collect();
        levels.sort();
        levels.dedup();
        let mut kept = Vec::new();
        for l in levels {
            kept.extend(self.ranking(&l, &day).into_iter().take(BOARD_KEEP).cloned());
        }
        self.entries = kept;
    }

    /// Today's games at `level`, best first.
    pub fn ranking(&self, level: &str, day: &str) -> Vec<&Played> {
        let mut v: Vec<&Played> = self.entries.iter().filter(|e| e.level == level && e.day == day).collect();
        v.sort_by(|a, b| b.total().cmp(&a.total()).then(b.persone().cmp(&a.persone())).then(a.at.cmp(&b.at)));
        v
    }

    /// Today's games at `level`.
    pub fn count(&self, level: &str, day: &str) -> usize {
        self.entries.iter().filter(|e| e.level == level && e.day == day).count()
    }

    /// The 0-based place of the game played at `at` by `name` in today's
    /// ranking of its level, if it is there.
    pub fn place(&self, level: &str, day: &str, name: &str, at: u64) -> Option<usize> {
        self.ranking(level, day).iter().position(|e| e.at == at && e.name == name)
    }

    /// Read a saved board; anything unreadable is an empty board, so a
    /// damaged file never stops the kiosk.
    pub fn from_json(s: &str) -> Board {
        serde_json::from_str(s).unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// Longest name kept, in characters.
pub const NAME_MAX: usize = 16;

/// The name as typed, tidied: no spaces around it, at most [`NAME_MAX`]
/// characters, first letter in capital. Empty if nothing was typed.
pub fn name(typed: &str) -> String {
    let t: String = typed.trim().chars().take(NAME_MAX).collect();
    let mut c = t.trim_end().chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DistrictOutcome;

    fn outcome(caught: usize, homes_hit: usize) -> Outcome {
        let d = DistrictOutcome { caught, homes_hit, ..Default::default() };
        Outcome { at_s: 0, districts: vec![d], hectares: 0.0, units_lost: 0 }
    }

    #[test]
    fn the_score_is_the_share_spared() {
        // playtest 5: game 2 (3 / 71 against 16 / 73) and game 3 (2 / 12
        // against 8 / 24) -- the better game must score higher
        let p2 = score(&outcome(3, 71), &outcome(16, 73));
        let p3 = score(&outcome(2, 12), &outcome(8, 24));
        assert_eq!((p2.families, p2.homes), (13, 2));
        assert_eq!(p2.total, ((0.7 * 13.0 / 16.0 + 0.3 * 2.0 / 73.0) * 1000.0_f64).round() as i64);
        assert!(p3.total > p2.total, "{} vs {}", p3.total, p2.total);
    }

    #[test]
    fn doing_nothing_does_not_score() {
        assert_eq!(score(&outcome(16, 73), &outcome(16, 73)).total, 0);
        // worse than no orders: no negative points either
        assert_eq!(score(&outcome(20, 80), &outcome(16, 73)).total, 0);
        // everything spared
        assert_eq!(score(&outcome(0, 0), &outcome(16, 73)).total, MAX);
        // no family at risk: the homes alone
        assert_eq!(score(&outcome(0, 5), &outcome(0, 10)).total, 500);
    }

    #[test]
    fn stars_run_from_one_to_five() {
        assert_eq!((stars_of(0.0, PERSONE), stars_of(0.5, PERSONE), stars_of(0.85, PERSONE), stars_of(1.7, PERSONE)), (1, 3, 5, 5));
        let s = score(&outcome(0, 10), &outcome(0, 10));
        let st = stars(&s, None, 1.0);
        assert_eq!((st.persone, st.case, st.tempismo, st.misura), (None, Some(1), None, 5));
    }

    fn game(name: &str, level: &str, stars: [u8; 4], day: &str, at: u64) -> Played {
        Played { name: name.into(), level: level.into(), stars: stars.map(Some), day: day.into(), at }
    }

    #[test]
    fn the_board_ranks_per_level_and_only_today() {
        let mut b = Board::default();
        b.push(game("Ieri", "Facile", [5, 5, 5, 5], "2026-10-12", 1));
        b.push(game("Anna", "Facile", [3, 3, 3, 3], "2026-10-13", 100));
        b.push(game("Bruno", "Facile", [4, 2, 3, 3], "2026-10-13", 200)); // 12, persone 4
        b.push(game("Carla", "Facile", [2, 4, 3, 3], "2026-10-13", 300)); // 12, persone 2
        b.push(game("Dario", "Facile", [3, 3, 3, 3], "2026-10-13", 50)); // 12, persone 3, earlier than Anna
        b.push(game("Elsa", "Esperto", [1, 1, 1, 1], "2026-10-13", 400));
        let names = |l: &str| b.ranking(l, "2026-10-13").iter().map(|e| e.name.clone()).collect::<Vec<_>>();
        // total first, then Persone, then the earlier game
        assert_eq!(names("Facile"), ["Bruno", "Dario", "Anna", "Carla"]);
        assert_eq!(names("Esperto"), ["Elsa"]);
        assert!(names("Medio").is_empty());
        // yesterday's game was dropped when today's came in
        assert!(b.entries.iter().all(|e| e.day == "2026-10-13"));
        assert_eq!(b.place("Facile", "2026-10-13", "Anna", 100), Some(2));
        // tomorrow shows nothing
        assert!(b.ranking("Facile", "2026-10-14").is_empty());
        assert_eq!(Board::from_json(&b.to_json()), b);
        assert_eq!(Board::from_json("not json"), Board::default());
    }

    #[test]
    fn a_name_is_tidied() {
        assert_eq!(name("  giulia "), "Giulia");
        assert_eq!(name("   "), "");
        assert_eq!(name("élodie maria rossi bianchi"), "Élodie maria ros");
    }
}
