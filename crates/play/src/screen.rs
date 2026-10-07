//! The text screen (playtest spec §1.3): a 64 × 32 map of the 4 km window and
//! the kiosk's panels, in the kiosk's words (`crates/text`). Presentation
//! only: every fact comes from `demo::Session`.

use demo::{Session, TargetKind, TokenId, TokenKind, TokenState, Verdict};
use fire::CellFire;
use scenario::Pos;
use text::play as t;

pub const COLS: usize = 64;
pub const ROWS: usize = 32;

/// The parts of the map that do not change during a session.
pub struct Base {
    /// Per map character: the fire-grid cells whose centre falls in it.
    cells: Vec<Vec<usize>>,
    glyph: Vec<char>,
    has_water: bool,
    cw: f32,
    ch: f32,
    height: f32,
}

impl Base {
    pub fn new(s: &Session) -> Base {
        let scn = &s.run.scn;
        let w = &scn.world;
        let (cw, ch) = (w.width_m / COLS as f32, w.height_m / ROWS as f32);
        let mut cells = vec![vec![]; COLS * ROWS];
        for i in 0..w.fire_rows * w.fire_cols {
            let p = w.centre_of(scenario::Cell { row: i / w.fire_cols, col: i % w.fire_cols });
            if let Some(k) = char_of(p, cw, ch, w.height_m) {
                cells[k].push(i);
            }
        }
        let mut glyph = vec!['.'; COLS * ROWS];
        let mut has_water = false;
        for (k, cs) in cells.iter().enumerate() {
            let veg = cs.iter().filter(|&&i| matches!(scn.fuel[i], 1..=12)).count();
            let water = cs.iter().filter(|&&i| scn.fuel[i] == 13 || scn.fuel[i] == -2).count();
            if water * 2 > cs.len().max(1) {
                glyph[k] = '~';
                has_water = true;
            } else if veg * 2 > cs.len().max(1) {
                glyph[k] = '"';
            }
        }
        for r in &scn.vectors.roads {
            if !r.drivable {
                continue;
            }
            for seg in r.line.windows(2) {
                let (a, b) = (Pos { x: seg[0][0], y: seg[0][1] }, Pos { x: seg[1][0], y: seg[1][1] });
                let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
                let n = (len / 10.0).ceil().max(1.0) as usize;
                for j in 0..=n {
                    let f = j as f32 / n as f32;
                    if let Some(k) = char_of(Pos { x: a.x + (b.x - a.x) * f, y: a.y + (b.y - a.y) * f }, cw, ch, w.height_m) {
                        glyph[k] = '=';
                    }
                }
            }
        }
        for r in s.run.agents.refuges.iter().filter(|r| !r.is_exit) {
            if let Some(k) = char_of(r.pos, cw, ch, w.height_m) {
                glyph[k] = 'A';
            }
        }
        for d in &s.run.referee.districts {
            let letter = t::district_letter(&d.name);
            for &i in &d.households {
                if let Some(k) = char_of(s.run.agents.households[i].home, cw, ch, w.height_m) {
                    glyph[k] = letter;
                }
            }
        }
        Base { cells, glyph, has_water, cw, ch, height: w.height_m }
    }

    /// The map character for a position, clamped onto the map (the station
    /// sits exactly on the east edge).
    fn at(&self, p: Pos) -> Option<(usize, usize)> {
        let w = self.cw * COLS as f32;
        let q = Pos { x: p.x.clamp(0.0, w - 0.01), y: p.y.clamp(0.01, self.height) };
        char_of(q, self.cw, self.ch, self.height).map(|k| (k / COLS, k % COLS))
    }
}

fn char_of(p: Pos, cw: f32, ch: f32, height: f32) -> Option<usize> {
    let c = (p.x / cw).floor();
    let r = ((height - p.y) / ch).floor();
    if c < 0.0 || r < 0.0 || c >= COLS as f32 || r >= ROWS as f32 {
        return None;
    }
    Some(r as usize * COLS + c as usize)
}

/// Which targets to draw: all of them except the tray-only ones.
fn on_map(k: TargetKind) -> bool {
    !matches!(k, TargetKind::Sky | TargetKind::Town)
}

/// Whether a unit is on the map now.
fn unit_shown(s: &Session, id: TokenId) -> bool {
    if !s.in_tray(id) || id == TokenId::I {
        return false;
    }
    !matches!(s.token_state(id), TokenState::NonChiamato | TokenState::InArrivo { .. } | TokenState::Perso)
}

/// The map, as lines (ruler, frame, 32 rows, frame).
pub fn map(s: &Session, base: &Base) -> Vec<String> {
    let mut g = base.glyph.clone();
    let state = s.run.fire.state();
    let burning: std::collections::HashSet<usize> = {
        let cols = s.run.scn.world.fire_cols;
        s.burning_cells().iter().map(|c| c.row * cols + c.col).collect()
    };
    for (k, cs) in base.cells.iter().enumerate() {
        if cs.iter().any(|i| burning.contains(i)) {
            g[k] = '*';
        } else if cs.iter().filter(|&&i| state[i] != CellFire::Unburnt).count() >= 3 {
            g[k] = 'x';
        }
    }
    // Labels never overwrite one another: try the spot itself, then the rows
    // just below and above, then slide along the row. A one-character gap is
    // kept on either side so `E1` and `E2` never read as `E1E2`.
    let put = |g: &mut Vec<char>, taken: &mut Vec<bool>, p: Pos, label: &str| {
        let Some((r0, c)) = base.at(p) else { return };
        let n = label.chars().count();
        let start0 = c.saturating_sub(n / 2).min(COLS - n);
        let free = |r: usize, st: usize, taken: &Vec<bool>| {
            let lo = st.saturating_sub(1);
            let hi = (st + n + 1).min(COLS);
            (lo..hi).all(|x| !taken[r * COLS + x])
        };
        let mut spots: Vec<(usize, usize)> = vec![(r0, start0)];
        for dr in [1i64, -1, 2, -2] {
            let r = r0 as i64 + dr;
            if (0..ROWS as i64).contains(&r) {
                spots.push((r as usize, start0));
            }
        }
        for dc in 1..COLS as i64 {
            for sgn in [1i64, -1] {
                let st = start0 as i64 + sgn * dc;
                if st >= 0 && st as usize + n <= COLS {
                    spots.push((r0, st as usize));
                }
            }
        }
        let (r, start) = spots.iter().copied().find(|(r, st)| free(*r, *st, taken)).unwrap_or((r0, start0));
        for (j, ch) in label.chars().enumerate() {
            g[r * COLS + start + j] = ch;
            taken[r * COLS + start + j] = true;
        }
    };
    let mut taken = vec![false; COLS * ROWS];
    // Units first (they win: a unit is never hidden), then target numbers.
    for id in TokenId::ALL {
        if unit_shown(s, id) {
            put(&mut g, &mut taken, s.token(id).at, text::token_code(id));
        }
    }
    for tg in s.targets().iter().filter(|x| on_map(x.kind)) {
        put(&mut g, &mut taken, tg.pos, &format!("[{}]", tg.id.0));
    }
    let mut out = vec![t::ruler(COLS), format!("   N +{}+", "-".repeat(COLS))];
    for r in 0..ROWS {
        out.push(format!("     |{}|", g[r * COLS..(r + 1) * COLS].iter().collect::<String>()));
    }
    out.push(format!("     +{}+", "-".repeat(COLS)));
    out
}

fn names(s: &Session) -> Vec<String> {
    s.run.referee.districts.iter().map(|d| d.name.clone()).collect()
}

/// The top bar: town, turn, clock; wind; forecast.
pub fn header(s: &Session) -> Vec<String> {
    let turn = s.turn();
    let (from, kmh) = s.wind();
    let fresh = turn.index == 2;
    vec![text::turn_title(turn.index, turn.at_s), text::wind_line(from, kmh), text::forecast_line(&s.forecast(), s.wind_turned(), fresh)]
}

/// The whole turn screen. `chosen`: the token picked with `scegli`.
pub fn screen(s: &Session, base: &Base, chosen: Option<TokenId>) -> String {
    let mut o: Vec<String> = header(s);
    o.push(String::new());
    o.extend(map(s, base));
    o.extend(t::legend(base.has_water));
    o.push(String::new());
    let ds = names(s);
    o.push(t::SEZ_QUARTIERI.into());
    for d in s.districts() {
        let id = s.target_of(TargetKind::District(d.district)).map_or(0, |x| x.id.0);
        let units: Vec<&str> = d.units.iter().map(|u| text::token_code(*u)).collect();
        o.push(t::district_row(id, &d.name, d.households, d.fire_m, d.level, d.warned, d.moving, d.safe, &units));
    }
    o.push(String::new());
    o.push(t::SEZ_RISORSE.into());
    for tk in s.tokens() {
        let doing = tk.doing.and_then(|k| s.target_of(k).map(|x| text::target_name(x, &ds)).or_else(|| doing_name(k, &ds)));
        let pending = tk.order.and_then(|g| {
            let tg = s.target(g)?;
            let p = s.preview(tk.id, g)?;
            Some((text::target_name(tg, &ds), text::preview(tk.id, &p)))
        });
        o.push(t::token_row(tk.id, tk.state, s.time_s(), doing.as_deref(), tk.water, pending.as_ref().map(|(a, b)| (a.as_str(), b.as_str()))));
    }
    o.push(String::new());
    let others: Vec<String> = s
        .targets()
        .iter()
        .filter(|x| on_map(x.kind) && !matches!(x.kind, TargetKind::District(_)))
        .map(|x| format!("[{}] {}", x.id.0, text::target_name(x, &ds)))
        .collect();
    o.push(t::targets_row(&others));
    if s.report().turn > 0 && !s.report().lines.is_empty() {
        o.push(String::new());
        o.push(t::report_title(s.report().turn));
        for l in &s.report().lines {
            o.push(format!("  · {}", text::report_line(l, &ds)));
        }
    }
    if let Some(tok) = chosen {
        o.push(String::new());
        o.extend(choice(s, tok));
    }
    o.push(String::new());
    o.push(if chosen.is_some() { t::prompt_after_choice(text::token_code(chosen.expect("some"))) } else { t::PROMPT.into() });
    o.join("\n")
}

fn doing_name(k: TargetKind, ds: &[String]) -> Option<String> {
    let p = Pos { x: 0.0, y: 0.0 };
    Some(text::target_name(&demo::Target { id: demo::TargetId(0), kind: k, pos: p, label_pos: p, facing_deg: None }, ds))
}

/// After `scegli`: the lit targets with their previews.
pub fn choice(s: &Session, tok: TokenId) -> Vec<String> {
    let ds = names(s);
    let tk = s.token(tok);
    let mut o = vec![t::choice_title(&text::token_name(tok), text::kind_what(tok.kind()))];
    if !tk.orderable {
        o.push(format!("  {}", text::token_state_why(tok, tk.state, s.time_s())));
        return o;
    }
    let lit = s.valid_targets(tok);
    for tg in &lit {
        let Some(p) = s.preview(tok, tg.id) else { continue };
        let line = text::preview(tok, &p);
        match tg.kind {
            TargetKind::Sky | TargetKind::Town => o.push(t::choice_no_target(text::token_code(tok), &text::target_name(tg, &ds), &line)),
            _ => o.push(format!("  [{}] {} — {}", tg.id.0, text::target_name(tg, &ds), line)),
        }
    }
    if lit.is_empty() {
        o.push(format!("  {}", t::NO_TARGETS));
    }
    let _ = TokenKind::Pattuglia;
    o
}

/// The between-turn frames: the map with a one-line header.
pub fn frame(s: &Session, base: &Base) -> String {
    let mut o = vec![t::frame_title(s.time_s())];
    o.extend(map(s, base));
    o.join("\n")
}

/// The finale's stamp frames and the verdict card.
pub fn verdict(s: &Session, v: &Verdict) -> String {
    let ds = names(s);
    let mut o = vec![t::verdict_title(s.time_s()), String::new(), format!("  {}", text::headline(v.headline)), String::new()];
    o.push(t::families_row(v.families_safe, v.households, v.none.families_safe));
    o.push(t::homes_row(v.homes_hit, v.none.homes_hit));
    o.push(String::new());
    o.push(t::SEZ_QUARTIERI.into());
    for d in &v.districts {
        o.push(t::verdict_district_row(&ds[d.district], text::stamp_mark(d.people), text::stamp(d.people), text::stamp_why(d.people), d.homes_hit, d.homes_hit_none));
    }
    if !v.notes.is_empty() {
        o.push(String::new());
        o.push(t::SEZ_REGOLE.into());
        for n in &v.notes {
            o.push(format!("  · {}", text::note(n, &ds)));
        }
    }
    o.push(String::new());
    o.push(t::PROMPT_END.into());
    o.join("\n")
}

/// One finale frame per district stamp, in the order the fire reached them.
pub fn stamps(s: &Session, v: &Verdict) -> Vec<String> {
    let ds = names(s);
    let mut order: Vec<usize> = (0..v.districts.len()).collect();
    let r = &s.run.referee.reports;
    order.sort_by_key(|&d| r[d].reached_at_s.or(r[d].threatened_at_s).unwrap_or(i64::MAX));
    order
        .into_iter()
        .map(|d| {
            let x = &v.districts[d];
            t::stamp_frame(&ds[d], text::stamp_mark(x.people), text::stamp(x.people), text::stamp_why(x.people))
        })
        .collect()
}
