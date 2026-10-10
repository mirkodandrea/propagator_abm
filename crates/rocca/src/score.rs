//! The score and the leaderboard (user, 2026-10-09).
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
/// Places on the leaderboard.
pub const TOP: usize = 10;

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Three letters, arcade style: no personal data at a school event.
    pub initials: String,
    pub score: i64,
    /// The fire played, for the operator; one board for all of them.
    pub case: String,
}

/// The best [`TOP`] games, highest first; ties keep the earlier game above.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub entries: Vec<Entry>,
}

impl Board {
    /// Whether a score would enter the board.
    pub fn qualifies(&self, score: i64) -> bool {
        score > 0 && (self.entries.len() < TOP || self.entries.last().is_some_and(|e| score > e.score))
    }

    /// Add a game; returns its place (0 = first) if it stays on the board.
    pub fn insert(&mut self, e: Entry) -> Option<usize> {
        if !self.qualifies(e.score) {
            return None;
        }
        let at = self.entries.iter().position(|x| e.score > x.score).unwrap_or(self.entries.len());
        self.entries.insert(at, e);
        self.entries.truncate(TOP);
        Some(at)
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

/// Three letters from what was typed: upper case A-Z, padded with «A».
pub fn initials(letters: [u8; 3]) -> String {
    letters.iter().map(|&l| (b'A' + l % 26) as char).collect()
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
    fn the_board_keeps_the_best_ten() {
        let mut b = Board::default();
        for k in 1..=12 {
            b.insert(Entry { initials: "AAA".into(), score: k * 100, case: "c".into() });
        }
        assert_eq!(b.entries.len(), TOP);
        assert_eq!(b.entries[0].score, 1200);
        assert!(!b.qualifies(300));
        assert_eq!(b.insert(Entry { initials: "BBB".into(), score: 1200, case: "c".into() }), Some(1), "a tie goes below the earlier game");
        assert_eq!(Board::from_json(&b.to_json()), b);
        assert_eq!(Board::from_json("not json"), Board::default());
    }

    #[test]
    fn initials_are_three_letters() {
        assert_eq!(initials([0, 1, 25]), "ABZ");
    }
}
