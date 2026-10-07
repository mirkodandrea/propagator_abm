//! `play`: the kiosk's game as text commands (playtest spec §1). One command
//! per kiosk gesture; the session lives in `partita.json` in the working
//! directory and is replayed through `demo::Session` on every command, so
//! there is no hidden server and the same seed + orders always give the
//! same game.

mod data;
mod screen;

use anyhow::{Context, Result};
use demo::{Session, TargetId, TargetKind, TokenId, TokenState, TURNS};
use serde::{Deserialize, Serialize};
use text::play as t;

const FILE: &str = "partita.json";

/// The saved game: seed, the orders of every played turn, and this turn's
/// orders not yet played. `semi` keeps every seed this directory has played
/// (the playtest harness reads it; `play` never prints a drawn seed).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Partita {
    seme: u64,
    turni: Vec<Vec<(String, u8)>>,
    ordini: Vec<(String, u8)>,
    #[serde(default)]
    semi: Vec<u64>,
}

/// An error the player sees: one Italian line.
struct Msg(String);

impl From<&str> for Msg {
    fn from(s: &str) -> Msg {
        Msg(s.to_string())
    }
}

impl From<String> for Msg {
    fn from(s: String) -> Msg {
        Msg(s)
    }
}

impl From<anyhow::Error> for Msg {
    fn from(_: anyhow::Error) -> Msg {
        Msg(t::ERR_INTERNAL.into())
    }
}

fn main() {
    // Never a stack trace, never English: a panic prints one Italian line.
    std::panic::set_hook(Box::new(|_| {
        eprintln!("{}", t::ERR_INTERNAL);
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match std::panic::catch_unwind(|| run(&args)) {
        Ok(Ok(out)) => {
            println!("{out}");
            0
        }
        Ok(Err(Msg(m))) => {
            println!("{m}");
            1
        }
        Err(_) => 1,
    };
    std::process::exit(code);
}

fn run(args: &[String]) -> Result<String, Msg> {
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("aiuto");
    match cmd {
        "nuova" => nuova(&args[1..]),
        "mostra" => mostra(),
        "scegli" => scegli(args.get(1)),
        "ordina" => ordina(args.get(1), args.get(2)),
        "annulla" => annulla(args.get(1)),
        "avanti" => avanti(),
        "riprova" => riprova(),
        "aiuto" | "help" | "--help" | "-h" => Ok(t::HELP.join("\n")),
        "stato" if args.get(1).map(|s| s.as_str()) == Some("--json") => stato(),
        _ => Err(t::ERR_UNKNOWN.into()),
    }
}

// --- the saved game ---------------------------------------------------------------

fn load() -> Result<Partita, Msg> {
    let bytes = std::fs::read(FILE).map_err(|_| Msg(t::ERR_NO_GAME.into()))?;
    serde_json::from_slice(&bytes).map_err(|_| Msg(t::ERR_NO_GAME.into()))
}

fn save(p: &Partita) -> Result<(), Msg> {
    let s = serde_json::to_string_pretty(p).map_err(|_| Msg(t::ERR_SAVE.into()))?;
    std::fs::write(FILE, s).map_err(|_| Msg(t::ERR_SAVE.into()))
}

fn token(code: &str) -> Option<TokenId> {
    text::token_from_code(code)
}

/// Rebuild the session from the saved game: every played turn's orders, then
/// *Avanti*; after the last turn the finale; then this turn's orders.
fn replay(p: &Partita) -> Result<Session> {
    let dir = data::dir()?;
    let mut s = Session::new(&dir, p.seme).context("session")?;
    for turn in &p.turni {
        give(&mut s, turn);
        s.end_turn()?;
    }
    if p.turni.len() >= TURNS as usize {
        s.finish()?;
    } else {
        give(&mut s, &p.ordini);
    }
    Ok(s)
}

fn give(s: &mut Session, orders: &[(String, u8)]) {
    for (code, id) in orders {
        if let Some(tk) = token(code) {
            let _ = s.assign(tk, TargetId(*id));
        }
    }
}

// --- commands ------------------------------------------------------------------------

fn draw_seed() -> u64 {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut z = (nanos as u64) ^ ((std::process::id() as u64) << 32) ^ 0x9E37_79B9_7F4A_7C15;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) % 1_000_000 + 1
}

fn nuova(args: &[String]) -> Result<String, Msg> {
    let seed = match args.iter().position(|a| a == "--seme") {
        Some(i) => args.get(i + 1).and_then(|s| s.parse::<u64>().ok()).ok_or(Msg(t::ERR_SEED.into()))?,
        None => draw_seed(),
    };
    let mut semi = load().map(|p| p.semi).unwrap_or_default();
    semi.push(seed);
    let p = Partita { seme: seed, turni: vec![], ordini: vec![], semi };
    let s = replay(&p)?;
    save(&p)?;
    let base = screen::Base::new(&s);
    Ok(format!("{}\n\n{}", text::BRIEFING, screen::screen(&s, &base, None)))
}

fn riprova() -> Result<String, Msg> {
    let old = load()?;
    let mut semi = old.semi.clone();
    semi.push(old.seme);
    let p = Partita { seme: old.seme, turni: vec![], ordini: vec![], semi };
    let s = replay(&p)?;
    save(&p)?;
    let base = screen::Base::new(&s);
    Ok(screen::screen(&s, &base, None))
}

fn mostra() -> Result<String, Msg> {
    let p = load()?;
    let mut s = replay(&p)?;
    let base = screen::Base::new(&s);
    if s.finished() {
        let v = s.verdict()?;
        return Ok(screen::verdict(&s, &v));
    }
    Ok(screen::screen(&s, &base, None))
}

fn parse_token(s: &Session, code: Option<&String>) -> Result<TokenId, Msg> {
    let code = code.ok_or(Msg(t::ERR_NEED_TOKEN.into()))?;
    let valid: Vec<&str> = s.tokens().iter().map(|k| text::token_code(k.id)).collect();
    match token(code) {
        Some(tk) if s.in_tray(tk) => Ok(tk),
        Some(tk) => Err(text::refusal(tk, demo::Refusal::NonDisponibile, s.time_s()).into()),
        None => Err(t::err_token(&valid).into()),
    }
}

fn scegli(code: Option<&String>) -> Result<String, Msg> {
    let p = load()?;
    let s = replay(&p)?;
    if s.finished() || s.turn().finale() {
        return Err(t::ERR_OVER.into());
    }
    let tk = parse_token(&s, code)?;
    let base = screen::Base::new(&s);
    Ok(screen::screen(&s, &base, Some(tk)))
}

fn ordina(code: Option<&String>, target: Option<&String>) -> Result<String, Msg> {
    let mut p = load()?;
    let mut s = replay(&p)?;
    if s.finished() || s.turn().finale() {
        return Err(t::ERR_OVER.into());
    }
    let tk = parse_token(&s, code)?;
    // The Canadair call needs no map target.
    let id = match target {
        Some(x) => TargetId(x.trim_matches(|c| c == '[' || c == ']').parse::<u8>().map_err(|_| Msg(t::ERR_BAD_NUMBER.into()))?),
        None => {
            let own = match tk {
                TokenId::K if s.token_state(TokenId::K) == TokenState::NonChiamato => s.target_of(TargetKind::Sky),
                _ => None,
            };
            own.map(|x| x.id).ok_or(Msg(t::ERR_NEED_TARGET.into()))?
        }
    };
    s.assign(tk, id).map_err(|r| Msg(text::refusal(tk, r, s.time_s())))?;
    let code = text::token_code(tk).to_string();
    p.ordini.retain(|(c, _)| *c != code);
    p.ordini.push((code, id.0));
    save(&p)?;
    let base = screen::Base::new(&s);
    Ok(screen::screen(&s, &base, None))
}

fn annulla(code: Option<&String>) -> Result<String, Msg> {
    let mut p = load()?;
    let mut s = replay(&p)?;
    if s.finished() || s.turn().finale() {
        return Err(t::ERR_OVER.into());
    }
    let tk = parse_token(&s, code)?;
    if !s.unassign(tk) {
        return Err(t::NOTHING_TO_CANCEL.into());
    }
    let code = text::token_code(tk).to_string();
    p.ordini.retain(|(c, _)| *c != code);
    save(&p)?;
    let base = screen::Base::new(&s);
    Ok(screen::screen(&s, &base, None))
}

fn avanti() -> Result<String, Msg> {
    let mut p = load()?;
    let mut s = replay(&p)?;
    if s.finished() || s.turn().finale() {
        return Err(t::ERR_OVER.into());
    }
    let base = screen::Base::new(&s);
    let turn = s.turn().index;
    let mid = s.time_s() + demo::TURN_S / 2;
    let mut out = vec![t::playing_title(turn), t::orders_sent(s.pending().len())];
    let mut frames = vec![];
    s.end_turn_observed(|x| {
        if x.time_s() == mid {
            frames.push(screen::frame(x, &base));
        }
    })?;
    frames.push(screen::frame(&s, &base));
    out.extend(frames);
    p.turni.push(std::mem::take(&mut p.ordini));
    if turn >= TURNS {
        out.push(String::new());
        out.push(t::finale_title());
        s.finish()?;
        out.push(screen::frame(&s, &base));
        let v = s.verdict()?;
        out.push(String::new());
        out.extend(screen::stamps(&s, &v));
        out.push(String::new());
        out.push(screen::verdict(&s, &v));
    } else {
        out.push(String::new());
        out.push(screen::screen(&s, &base, None));
    }
    save(&p)?;
    Ok(out.join("\n"))
}

// --- the builder's dump (not in `aiuto`, not allowed to the playtester) -----------------

fn stato() -> Result<String, Msg> {
    use serde_json::json;
    let p = load()?;
    let mut s = replay(&p)?;
    let names: Vec<String> = s.run.referee.districts.iter().map(|d| d.name.clone()).collect();
    let tokens: Vec<_> = s
        .tokens()
        .iter()
        .map(|k| {
            json!({
                "id": text::token_code(k.id), "state": format!("{:?}", k.state), "water": k.water,
                "at": [k.at.x, k.at.y], "order": k.order.map(|o| o.0), "doing": k.doing.map(|d| format!("{d:?}")),
                "orderable": k.orderable,
            })
        })
        .collect();
    let targets: Vec<_> = s.targets().iter().map(|g| json!({"id": g.id.0, "kind": format!("{:?}", g.kind), "pos": [g.pos.x, g.pos.y], "name": text::target_name(g, &names)})).collect();
    let districts: Vec<_> = s
        .districts()
        .iter()
        .map(|d| json!({"name": d.name, "households": d.households, "fire_m": if d.fire_m.is_finite() { Some(d.fire_m) } else { None }, "level": format!("{:?}", d.level), "warned": d.warned, "moving": d.moving, "safe": d.safe, "caught": d.caught}))
        .collect();
    let report: Vec<_> = s.report().lines.iter().map(|l| json!({"kind": format!("{:?}", l.kind), "district": l.district, "n": l.n, "text": text::report_line(l, &names)})).collect();
    let verdict = if s.finished() { Some(format!("{:?}", s.verdict()?)) } else { None };
    let turn = s.turn();
    let (from, kmh) = s.wind();
    let v = json!({
        "seed": p.seme, "turn": turn.index, "at_s": turn.at_s, "finished": s.finished(),
        "wind": {"from_deg": from, "kmh": kmh, "turned": s.wind_turned()},
        "forecast": format!("{:?}", s.forecast()),
        "shift": format!("{:?}", s.draw.spec.shift),
        "tokens": tokens, "targets": targets, "districts": districts, "report": report,
        "pending": p.ordini, "turns": p.turni, "log": s.log().iter().map(|(t, k, g)| json!([t, text::token_code(*k), format!("{g:?}")])).collect::<Vec<_>>(),
        "drops": s.drops(), "spots": s.spot_fires().len(), "verdict": verdict,
    });
    Ok(serde_json::to_string_pretty(&v).unwrap_or_default())
}
