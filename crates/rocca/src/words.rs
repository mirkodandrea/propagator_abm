//! Numbers and directions as the player reads them, in Italian: one place for
//! the coordinator's reasons, the crises, the log and the kiosk.

/// The hour of day the fire is reported: the game's clock starts here, so
/// the player reads "ore 14:26", a time that runs, rather than "T+0:26"
/// (user, 2026-10-09: it was not clear the game runs in real time).
pub const START_H: i64 = 14;

/// Seconds since the start as a time of day: `1560` as "14:26".
pub fn clock(s: i64) -> String {
    let m = START_H * 60 + s.max(0) / 60;
    format!("{}:{:02}", (m / 60) % 24, m % 60)
}

/// `1234.0` m as "1,2 km".
pub fn km(m: f32) -> String {
    format!("{:.1} km", m / 1000.0).replace('.', ",")
}

/// The wind's bearing (where it blows FROM) as a compass word: "sud-ovest".
pub fn compass(deg: f64) -> &'static str {
    const N: [&str; 8] = ["nord", "nord-est", "est", "sud-est", "sud", "sud-ovest", "ovest", "nord-ovest"];
    N[(((deg + 22.5).rem_euclid(360.0)) / 45.0) as usize % 8]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn italian_numbers_and_winds() {
        assert_eq!(km(1250.0), "1,2 km");
        assert_eq!(compass(94.0), "est");
        assert_eq!(compass(200.0), "sud");
        assert_eq!(compass(350.0), "nord");
    }
}
