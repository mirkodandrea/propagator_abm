//! Every word the player reads, in Italian, in one Bevy-free place
//! (`docs/demo-spec.md` §3). The `play` binary and the kiosk both format the
//! typed facts of `demo::` through here, so a blind playtest reads exactly the
//! kiosk's words.
//!
//! Every match on a `demo::` kind is exhaustive -- no `_` arm -- so a new
//! kind cannot ship without its string; `tests` below walks every variant.
//! NOTE: copy still to be read by a native speaker (milestone 4).

use demo::district::Level;
use demo::{
    Effect, Forecast, Headline, Note, Preview, Refusal, ReportKind, ReportLine, Side, Stamp, Target, TargetKind, TokenId,
    TokenKind, TokenState, LIKELY_P, POINTS_AT_P, TURNS,
};

pub const TOWN: &str = "Rocca Ventosa";
pub const TITLE: &str = "Rocca Ventosa brucia. Tocca a te.";

// --- small formatting helpers ---------------------------------------------------

/// `T+08`: minutes since the fire started.
pub fn clock(at_s: i64) -> String {
    format!("T+{:02}", at_s / 60)
}

/// `4′` (rounded up to the minute; under a minute is `1′`).
pub fn minutes(s: i64) -> String {
    format!("{}′", ((s.max(0) + 59) / 60).max(1))
}

/// Distance as a chip shows it: `700 m`, `1,4 km`.
pub fn fire_at_m(m: f32) -> String {
    if !m.is_finite() {
        "lontano".into()
    } else if m >= 1000.0 {
        format!("{:.1} km", m / 1000.0).replace('.', ",")
    } else {
        format!("{:.0} m", (m / 50.0).round().max(1.0) * 50.0)
    }
}

/// `Fuoco a 700 m` (kept for the kiosk's chips).
pub fn fire_at(m: f32) -> String {
    format!("Fuoco a {}", fire_at_m(m))
}

pub fn households(n: usize) -> String {
    match n {
        1 => "1 famiglia".into(),
        n => format!("{n} famiglie"),
    }
}

fn homes(n: u32) -> String {
    match n {
        1 => "1 casa".into(),
        n => format!("{n} case"),
    }
}

/// Eight-way compass word for a bearing in degrees (0 = north), capitalised.
pub fn compass(deg: f32) -> &'static str {
    const N: [&str; 8] = ["Nord", "Nord-Est", "Est", "Sud-Est", "Sud", "Sud-Ovest", "Ovest", "Nord-Ovest"];
    N[((deg.rem_euclid(360.0) + 22.5) / 45.0) as usize % 8]
}

/// The same word, lower case (`fianco ovest`).
pub fn compass_lower(deg: f32) -> String {
    compass(deg).to_lowercase()
}

/// The kiosk's lower-case bearing names (moved here from the kiosk).
pub fn bearing_name(deg: f32) -> &'static str {
    const N: [&str; 8] = ["nord", "nord-est", "est", "sud-est", "sud", "sud-ovest", "ovest", "nord-ovest"];
    N[((deg.rem_euclid(360.0) + 22.5) / 45.0) as usize % 8]
}

/// A preposition joined to a place name that carries its article, the
/// Italian way: `a` + `Il Borgo` → `al Borgo`, `di` + `Le Coste` → `delle
/// Coste`. Names without an article are left alone (`a Rocca Ventosa`).
pub fn con(prep: &str, name: &str) -> String {
    let (art, rest) = match name.split_once(' ') {
        Some((a, r)) if ["Il", "Lo", "La", "I", "Gli", "Le"].contains(&a) => (a, r),
        _ => return format!("{prep} {name}"),
    };
    let fused = match (prep, art) {
        ("verso", a) => return format!("verso {} {rest}", a.to_lowercase()),
        (p, "Il") => format!("{}l", stem(p)),
        (p, "Lo") => format!("{}llo", stem(p)),
        (p, "La") => format!("{}lla", stem(p)),
        (p, "I") => format!("{}i", stem(p)),
        (p, "Gli") => format!("{}gli", stem(p)),
        (p, "Le") => format!("{}lle", stem(p)),
        _ => return format!("{prep} {name}"),
    };
    format!("{fused} {rest}")
}

fn stem(prep: &str) -> &'static str {
    match prep {
        "a" => "a",
        "da" => "da",
        "di" => "de",
        "in" => "ne",
        "su" => "su",
        _ => "a",
    }
}

// --- the top bar ------------------------------------------------------------------

/// `ROCCA VENTOSA · Turno 2 di 5 · T+08` (the finale says so instead).
pub fn turn_title(turn: u8, at_s: i64) -> String {
    if turn > TURNS {
        format!("{} · Finale · {}", TOWN.to_uppercase(), clock(at_s))
    } else {
        format!("{} · Turno {turn} di {TURNS} · {}", TOWN.to_uppercase(), clock(at_s))
    }
}

/// `Vento: da Sud, 35 km/h — spinge il fuoco verso Nord`.
pub fn wind_line(from_deg: f32, kmh: f32) -> String {
    format!("Vento: da {}, {:.0} km/h — spinge il fuoco verso {}", compass(from_deg), kmh, compass(from_deg + 180.0))
}

/// How the forecast words a chance.
pub fn likelihood(p: f32) -> &'static str {
    if p >= LIKELY_P {
        "probabile"
    } else if p >= POINTS_AT_P {
        "possibile"
    } else {
        "poco probabile"
    }
}

/// `Meteo: il vento potrebbe girare e soffiare da Est (probabile, 60 %), tra T+12 e T+26`.
pub fn forecast_line(f: &Forecast, turned: bool, issue_new: bool) -> String {
    if turned {
        return format!("Meteo: il vento è girato e ora soffia da {}.", compass(f.shift_to_deg));
    }
    let new = if issue_new { "NUOVO BOLLETTINO — " } else { "" };
    format!(
        "Meteo: {new}il vento potrebbe girare e soffiare da {} ({}, {:.0} %), tra T+{:02} e T+{:02}",
        compass(f.shift_to_deg),
        likelihood(f.shift_p),
        f.shift_p * 100.0,
        f.shift_eta_min.0,
        f.shift_eta_min.1
    )
}

// --- tokens -----------------------------------------------------------------------

/// The short id printed in the tray and typed in `play`.
pub fn token_code(t: TokenId) -> &'static str {
    match t {
        TokenId::P => "P",
        TokenId::I => "I",
        TokenId::E1 => "E1",
        TokenId::E2 => "E2",
        TokenId::E3 => "E3",
        TokenId::S => "S",
        TokenId::K => "K",
    }
}

/// The token for a typed code, case-insensitive.
pub fn token_from_code(s: &str) -> Option<TokenId> {
    TokenId::ALL.iter().copied().find(|t| token_code(*t).eq_ignore_ascii_case(s.trim()))
}

pub fn token_name(t: TokenId) -> String {
    match t.kind() {
        TokenKind::Autobotte => format!("Autobotte {}", t.number().unwrap_or(1)),
        k => kind_name(k).into(),
    }
}

pub fn kind_name(k: TokenKind) -> &'static str {
    match k {
        TokenKind::Pattuglia => "Pattuglia",
        TokenKind::ItAlert => "IT-alert",
        TokenKind::Autobotte => "Autobotte",
        TokenKind::Squadra => "Squadra AIB",
        TokenKind::Canadair => "Canadair",
    }
}

/// What each resource is, in one line (the tray's tooltip, `play`'s legend).
pub fn kind_what(k: TokenKind) -> &'static str {
    match k {
        TokenKind::Pattuglia => "Polizia Locale: va in un quartiere e avvisa le famiglie quando arriva",
        TokenKind::ItAlert => "messaggio su tutti i telefoni: avvisa subito tutti i quartieri; si usa una volta",
        TokenKind::Autobotte => "Vigili del Fuoco: difende le case dalla strada; l'acqua dura circa 6 minuti, poi va a riempire",
        TokenKind::Squadra => "squadra antincendio a piedi: taglia la vegetazione per fermare il fuoco, ma è lenta",
        TokenKind::Canadair => "aereo: va chiamato, arriva dopo 25 minuti; poi un lancio d'acqua a turno",
    }
}

fn feminine(t: TokenId) -> bool {
    matches!(t.kind(), TokenKind::Pattuglia | TokenKind::Autobotte | TokenKind::Squadra)
}

/// The badge on a token: `libera`, `in viaggio 3′`, `in arrivo turno 4`, ...
pub fn token_state(t: TokenId, s: TokenState, now_s: i64) -> String {
    let a = if feminine(t) { "a" } else { "o" };
    match s {
        TokenState::Libero => format!("liber{a}"),
        TokenState::InViaggio { eta_s } => format!("in viaggio, arriva in {}", minutes(eta_s)),
        TokenState::AlLavoro => "al lavoro".into(),
        TokenState::Rifornimento { eta_s } => format!("a riempire l'acqua ({})", minutes(eta_s)),
        TokenState::Ritirato => format!("ritirat{a}: salta un turno"),
        TokenState::Perso => format!("pers{a}"),
        TokenState::Usato => "usato".into(),
        TokenState::InArrivo { eta_s } => {
            let turn = ((now_s + eta_s) / demo::TURN_S) as u8 + 1;
            if turn > TURNS {
                format!("in arrivo tra {} (dopo l'ultimo turno)", minutes(eta_s))
            } else {
                format!("in arrivo al turno {turn} (tra {})", minutes(eta_s))
            }
        }
        TokenState::NonChiamato => "non chiamato".into(),
    }
}

// --- targets ------------------------------------------------------------------------

/// A target's name on the map and in lists.
pub fn target_name(t: &Target, districts: &[String]) -> String {
    match t.kind {
        TargetKind::District(d) => districts.get(d).cloned().unwrap_or_else(|| "Quartiere".into()),
        TargetKind::Head => "Testa del fuoco".into(),
        TargetKind::Flank(side) => match t.facing_deg {
            Some(deg) => format!("Fianco {}", compass_lower(deg)),
            None => match side {
                Side::Left => "Fianco sinistro".into(),
                Side::Right => "Fianco destro".into(),
            },
        },
        TargetKind::SpotFire(n) => format!("Focolaio {}", n + 1),
        TargetKind::Sky => "Chiama il Canadair".into(),
        TargetKind::Town => "Tutto il paese".into(),
    }
}

/// What a target would do for a token, in words.
pub fn effect(token: TokenId, e: Effect) -> String {
    match e {
        Effect::Avvisa { families } => format!("avvisa {}", households(families as usize)),
        Effect::AvvisaTutti { families } => format!("avvisa tutti, anche chi non serve ({})", households(families as usize)),
        Effect::Difende { homes: n } => format!("difende {}", homes(n)),
        Effect::Linea => match token.kind() {
            TokenKind::Squadra => "taglia una linea nella vegetazione".into(),
            _ => "bagna il bordo del fuoco dalla strada".into(),
        },
        Effect::Spegne => "prova a spegnerlo".into(),
        Effect::Ritirata => "si ritirerà: lì il calore è troppo forte".into(),
        Effect::Chiamata { turn, .. } => {
            if turn > TURNS {
                "arriva dopo l'ultimo turno".into()
            } else {
                format!("arriva al turno {turn}")
            }
        }
        Effect::Bagna { homes: 0 } => "bagna la vegetazione davanti al fuoco".into(),
        Effect::Bagna { homes: n } => format!("bagna la vegetazione vicino a {}", homes(n)),
        Effect::RallentaPoco => "rallenta poco il fuoco".into(),
        Effect::Lontano => "strada troppo lontana: non ci arriva".into(),
        Effect::Inutile => "inutile adesso".into(),
    }
}

/// The preview tag: `arriva in 4′ · difende 60 case`.
pub fn preview(token: TokenId, p: &Preview) -> String {
    match p.effect {
        Effect::Chiamata { .. } | Effect::AvvisaTutti { .. } => effect(token, p.effect),
        _ => format!("arriva in {} · {}", minutes(p.eta_s), effect(token, p.effect)),
    }
}

// --- districts ------------------------------------------------------------------------

pub fn level(l: Level) -> &'static str {
    match l {
        Level::Calm => "tranquillo",
        Level::Watch => "attenzione",
        Level::Threatened => "MINACCIATO",
        Level::Reached => "RAGGIUNTO",
    }
}

/// `avvisato: 90 in viaggio, 40 al sicuro` / `non avvisato`.
pub fn warned_line(warned: bool, moving: u32, safe: u32) -> String {
    if warned {
        format!("avvisato: {moving} in viaggio, {safe} al sicuro")
    } else {
        "non avvisato".into()
    }
}

// --- refusals --------------------------------------------------------------------------

pub fn refusal(token: TokenId, r: Refusal, now_s: i64) -> String {
    let name = token_name(token);
    match r {
        Refusal::Finita => "L'incendio è finito: non si danno più ordini.".into(),
        Refusal::NonDisponibile => format!("{name} non c'è ancora: arriva al turno {}.", demo::REINFORCEMENT_TURN),
        Refusal::Occupato(s) => format!("{name} non può ricevere ordini adesso: è {}.", token_state(token, s, now_s)),
        Refusal::BersaglioSconosciuto => "Quel numero non è un bersaglio sulla mappa.".into(),
        Refusal::BersaglioNonValido => format!("{name} non può andare lì."),
    }
}

// --- the report --------------------------------------------------------------------------

fn district_name(d: Option<usize>, districts: &[String]) -> String {
    d.and_then(|d| districts.get(d).cloned()).unwrap_or_else(|| "un quartiere".into())
}

fn token_or(t: Option<TokenId>, fallback: &str) -> String {
    t.map(token_subject).unwrap_or_else(|| fallback.into())
}

/// A token as the subject of a sentence: `Il Canadair`, `La pattuglia`,
/// `Autobotte 1` (a call sign takes no article).
pub fn token_subject(t: TokenId) -> String {
    match t.kind() {
        TokenKind::Canadair => "Il Canadair".into(),
        TokenKind::Pattuglia => "La pattuglia".into(),
        TokenKind::Squadra => "La squadra AIB".into(),
        TokenKind::ItAlert => "L'IT-alert".into(),
        TokenKind::Autobotte => token_name(t),
    }
}

pub fn report_line(l: &ReportLine, districts: &[String]) -> String {
    let d = || district_name(l.district, districts);
    let n = l.n.unwrap_or(0);
    let tok = || token_or(l.token, "Un mezzo");
    let a = if l.token.is_some_and(|t| !feminine(t)) { "o" } else { "a" };
    match l.kind {
        ReportKind::QuartiereRaggiunto => format!("Il fuoco è arrivato alle case {}.", con("di", &d())),
        ReportKind::MezzoPerso => format!("{} è stat{a} pers{a} nel fuoco.", tok()),
        ReportKind::QuartiereMinacciato => format!("Il fuoco è a meno di 300 m {}.", con("da", &d())),
        ReportKind::VentoGirato => format!("Il vento è girato: ora soffia da {}.", compass(n as f32)),
        ReportKind::NuovoFocolaio if n > 1 => format!("{n} nuovi focolai: le scintille accendono il fuoco più avanti."),
        ReportKind::NuovoFocolaio => "Un nuovo focolaio: le scintille accendono il fuoco più avanti.".into(),
        ReportKind::Ritirata if l.token == Some(TokenId::K) => "Il Canadair ha interrotto il volo: sopra il fuoco il calore era troppo forte.".into(),
        ReportKind::Ritirata => format!("{} si è ritirat{a}: lì il calore era troppo forte.", tok()),
        ReportKind::PattugliaArrivata => format!("La pattuglia è arrivata {} e ha avvisato {}.", con("a", &d()), households(n as usize)),
        ReportKind::FamiglieInViaggio => format!("{}: {} in viaggio verso l'area di attesa.", d(), households(n as usize)),
        ReportKind::AutobotteASecco => format!("{} ha finito l'acqua: va a riempire.", tok()),
        ReportKind::FocolaioSpento => "Un focolaio si è spento.".into(),
        ReportKind::CanadairInZona => "Il Canadair è arrivato sopra il paese.".into(),
        ReportKind::Lancio => match l.district {
            Some(_) => format!("Il Canadair ha lanciato l'acqua vicino {}.", con("a", &d())),
            None => "Il Canadair ha lanciato l'acqua.".into(),
        },
        ReportKind::Arrivato => match l.district {
            Some(_) => format!("{} è al lavoro {}.", tok(), con("a", &d())),
            None => format!("{} è al lavoro.", tok()),
        },
        ReportKind::RipetitoreGiu => "Il fuoco ha abbattuto un ripetitore: alcuni telefoni non ricevono più gli avvisi.".into(),
        ReportKind::Rinforzo => "Arriva un'altra autobotte: sarà disponibile al prossimo turno.".into(),
        ReportKind::NuovoBollettino => "Al prossimo turno arriva un nuovo bollettino meteo.".into(),
    }
}

// --- the verdict ----------------------------------------------------------------------------

pub fn stamp(s: Stamp) -> &'static str {
    match s {
        Stamp::InTempo => "In tempo",
        Stamp::Tardi => "Tardi",
        Stamp::MaiAvvisati => "Mai avvisati",
        Stamp::GiustoNonAvvisare => "Giusto non avvisare",
        Stamp::Prudente => "Prudente",
        Stamp::AllarmeInutile => "Allarme inutile",
    }
}

/// One clause saying why a district got its stamp.
pub fn stamp_why(s: Stamp) -> &'static str {
    match s {
        Stamp::InTempo => "avvisati prima che arrivasse il fuoco",
        Stamp::Tardi => "avvisati quando il fuoco era già vicino",
        Stamp::MaiAvvisati => "il fuoco è arrivato e nessuno li aveva avvisati",
        Stamp::GiustoNonAvvisare => "il fuoco non è arrivato: giusto non allarmarli",
        Stamp::Prudente => "avvisati perché il vento o le previsioni li indicavano: prudente",
        Stamp::AllarmeInutile => "avvisati, ma niente indicava che il fuoco andasse lì",
    }
}

/// The mark next to a stamp (text stands in for the kiosk's ✅ / ⚠️ / ❌).
pub fn stamp_mark(s: Stamp) -> &'static str {
    match s {
        Stamp::InTempo | Stamp::GiustoNonAvvisare | Stamp::Prudente => "[OK]",
        Stamp::Tardi | Stamp::AllarmeInutile => "[!]",
        Stamp::MaiAvvisati => "[X]",
    }
}

pub fn headline(h: Headline) -> &'static str {
    match h {
        Headline::TuttiAlSicuro => "Tutti al sicuro!",
        Headline::OttimoLavoro => "Ottimo lavoro",
        Headline::HaiFattoLaDifferenza => "Hai fatto la differenza",
        Headline::FuocoPiuVeloce => "Il fuoco è stato più veloce",
    }
}

pub fn note(n: &Note, districts: &[String]) -> String {
    let d = |k: usize| districts.get(k).cloned().unwrap_or_default();
    match *n {
        Note::UnitaPersa { token } => format!("{} è rimast{} intrappolat{} nel fuoco: i mezzi vanno tenuti lontani dalla testa.", token_subject(token), gender(token), gender(token)),
        Note::TestaRitirata { token } => {
            format!("{}, mandat{} sulla testa del fuoco, si è ritirat{}: col vento la testa non si ferma.", token_subject(token), gender(token), gender(token))
        }
        Note::AutobotteASecco { token, district } => {
            format!("{} era senza acqua quando il fuoco è arrivato {}: l'acqua dura pochi minuti.", token_name(token), con("a", &d(district)))
        }
        Note::ItAlertSprecato => "L'IT-alert ha avvisato anche chi non era in pericolo.".into(),
        Note::DifesaInutile { district } => format!("Autobotti mandate {}, dove né il vento né le previsioni portavano il fuoco.", con("a", &d(district))),
        Note::CanadairTardi => "Canadair chiamato tardi: non ha fatto in tempo a lanciare.".into(),
        Note::CanadairMaiChiamato => "Canadair mai chiamato: ci mette 25 minuti, va chiamato prima che serva.".into(),
        Note::CanadairInTempo { drops } => match drops {
            1 => "Canadair chiamato in tempo: 1 lancio.".into(),
            n => format!("Canadair chiamato in tempo: {n} lanci."),
        },
        Note::FocolaioSpento => "Un focolaio attaccato subito si è spento.".into(),
        Note::CaseDifese { district, homes: n } => format!("Autobotti {} prima del fuoco: {} in meno colpite.", con("a", &d(district)), homes(n)),
    }
}

fn gender(t: TokenId) -> &'static str {
    if feminine(t) {
        "a"
    } else {
        "o"
    }
}

// --- labels of the verdict card and the screens -------------------------------------------------

pub const FAMILIES_SAFE: &str = "famiglie in salvo";
pub const HOMES_HIT: &str = "case colpite";
pub const WITHOUT_ORDERS: &str = "senza ordini";
pub const RETRY: &str = "Riprova";
pub const AVANTI: &str = "Avanti";
pub const BRIEFING: &str = "Sei il comandante. Ogni turno: scegli una risorsa, poi un punto sulla mappa. Poi Avanti.";

/// Why a token cannot take an order now (shown after `scegli`).
pub fn token_state_why(t: TokenId, s: TokenState, now_s: i64) -> String {
    format!("{} non può ricevere ordini in questo turno: è {}.", token_name(t), token_state(t, s, now_s))
}

/// The text screen of the `play` binary: the same words as the kiosk, laid
/// out as plain text (playtest spec §1.3).
pub mod play {
    use super::*;

    pub const SEZ_QUARTIERI: &str = "QUARTIERI";
    pub const SEZ_RISORSE: &str = "RISORSE";
    pub const SEZ_REGOLE: &str = "COSA HA FUNZIONATO, COSA NO";
    pub const NO_TARGETS: &str = "Nessun bersaglio possibile in questo turno.";
    pub const PROMPT: &str = "> ./play scegli <risorsa> · ./play ordina <risorsa> <bersaglio> · ./play avanti";
    pub const PROMPT_END: &str = "> ./play riprova (stesso incendio, dal turno 1) · ./play nuova (un altro incendio)";

    /// The letter a district's houses are drawn with: `Il Borgo` → `B`.
    pub fn district_letter(name: &str) -> char {
        name.split_whitespace().last().and_then(|w| w.chars().next()).map(|c| c.to_ascii_uppercase()).unwrap_or('H')
    }

    /// `     0 km           1  ...  4` over a map `cols` wide (4 km).
    pub fn ruler(cols: usize) -> String {
        let mut r: Vec<char> = vec![' '; cols + 8];
        let step = cols / 4;
        for k in 0..=4 {
            let label = if k == 0 { "0 km".to_string() } else { k.to_string() };
            let at = 6 + k * step;
            for (j, c) in label.chars().enumerate() {
                if at + j < r.len() {
                    r[at + j] = c;
                }
            }
        }
        r.into_iter().collect::<String>().trim_end().to_string()
    }

    pub fn legend(water: bool) -> Vec<String> {
        let w = if water { "  ~ acqua" } else { "" };
        vec![
            format!("Legenda: \" bosco/macchia  . campi  = strada{w}  * fuoco  x bruciato"),
            "         B Il Borgo  C Le Coste  M Il Mulino  A area di attesa".into(),
            "         [n] bersaglio  P pattuglia  E1 E2 E3 autobotti  S squadra  K canadair".into(),
        ]
    }

    #[allow(clippy::too_many_arguments)]
    pub fn district_row(id: u8, name: &str, n: u32, fire_m: f32, lvl: Level, warned: bool, moving: u32, safe: u32, units: &[&str]) -> String {
        let u = if units.is_empty() { String::new() } else { format!(" · qui: {}", units.join(" ")) };
        format!(
            "  [{id}] {name:<10} {:>12} · fuoco a {} · {} · {}{u}",
            households(n as usize),
            fire_at_m(fire_m),
            level(lvl),
            warned_line(warned, moving, safe)
        )
    }

    pub fn token_row(id: TokenId, s: TokenState, now_s: i64, doing: Option<&str>, water: Option<f32>, pending: Option<(&str, &str)>) -> String {
        let mut state = match (id, s) {
            (TokenId::I, TokenState::Libero) => "1 uso".to_string(),
            _ => token_state(id, s, now_s),
        };
        if let Some(d) = doing {
            match s {
                TokenState::AlLavoro => state = format!("al lavoro: {d}"),
                TokenState::InViaggio { .. } => state = format!("{state} · {}", con("verso", d)),
                _ => {}
            }
        }
        if let Some(w) = water {
            state = format!("{state} · acqua {:.0} %", w * 100.0);
        }
        let mut row = format!("  {:<3} {:<13} {state}", token_code(id), token_name(id));
        if let Some((to, what)) = pending {
            row = format!("{row}\n        → ORDINE: {to} ({what})");
        }
        row
    }

    pub fn targets_row(others: &[String]) -> String {
        if others.is_empty() {
            "BERSAGLI  (i quartieri hanno il loro numero qui sopra)".into()
        } else {
            format!("BERSAGLI  {}", others.join("  "))
        }
    }

    pub fn report_title(turn: u8) -> String {
        format!("RAPPORTO (turno {turn})")
    }

    pub fn choice_title(name: &str, what: &str) -> String {
        format!("HAI SCELTO: {name} — {what}.\nDove la mandi? Bersagli possibili:")
    }

    pub fn choice_no_target(code: &str, target: &str, line: &str) -> String {
        format!("  ./play ordina {code}  → {target}: {line}")
    }

    pub fn prompt_after_choice(code: &str) -> String {
        format!("> ./play ordina {code} <bersaglio> · ./play scegli <altra risorsa> · ./play avanti")
    }

    pub fn frame_title(at_s: i64) -> String {
        format!("--- {} · il fuoco avanza ---", clock(at_s))
    }

    pub fn playing_title(turn: u8) -> String {
        format!("=== Turno {turn}: gli ordini partono, il fuoco corre per 8 minuti ===")
    }

    pub fn finale_title() -> String {
        "=== Ultimo turno finito. Il fuoco corre fino a T+60: vediamo com'è andata. ===".into()
    }

    pub fn stamp_frame(name: &str, mark: &str, stamp: &str, why: &str) -> String {
        format!("  {name:<10} {mark} {stamp} — {why}")
    }

    pub fn verdict_title(at_s: i64) -> String {
        format!("FINE DELL'INCENDIO · {}", clock(at_s))
    }

    pub fn families_row(safe: u32, total: u32, none: u32) -> String {
        format!("  Famiglie in salvo:  {safe} su {total}   ({WITHOUT_ORDERS}: {none})")
    }

    pub fn homes_row(hit: u32, none: u32) -> String {
        format!("  Case colpite:       {hit}   ({WITHOUT_ORDERS}: {none})")
    }

    pub fn verdict_district_row(name: &str, mark: &str, stamp: &str, why: &str, homes: u32, homes_none: u32) -> String {
        format!("  {name:<10} {mark} {stamp} — {why}\n             case colpite {homes} ({WITHOUT_ORDERS}: {homes_none})")
    }

    pub fn orders_sent(n: usize) -> String {
        match n {
            0 => "Nessun ordine in questo turno.".into(),
            1 => "1 ordine parte adesso.".into(),
            n => format!("{n} ordini partono adesso."),
        }
    }

    pub const HELP: [&str; 5] = [
        "./play nuova                         comincia un incendio nuovo (./play riprova rigioca lo stesso)",
        "./play mostra                        rivedi lo schermo",
        "./play scegli <risorsa>              vedi dove puoi mandarla e cosa farebbe   (es. ./play scegli E1)",
        "./play ordina <risorsa> <bersaglio>  dai l'ordine (es. ./play ordina E1 1); ./play annulla <risorsa> lo toglie",
        "./play avanti                        fai partire gli ordini: il fuoco corre per 8 minuti",
    ];

    pub const ERR_NO_GAME: &str = "Nessuna partita in corso: comincia con ./play nuova";
    pub const ERR_UNKNOWN: &str = "Comando sconosciuto. Comandi: nuova, mostra, scegli, ordina, annulla, avanti, riprova, aiuto.";
    pub const ERR_OVER: &str = "L'incendio è finito: ./play riprova per rigiocarlo, ./play nuova per un altro.";
    pub const ERR_NEED_TOKEN: &str = "Manca la risorsa: per esempio ./play scegli E1";
    pub const ERR_NEED_TARGET: &str = "Manca il bersaglio: per esempio ./play ordina E1 1 (i numeri sono sulla mappa).";
    pub const ERR_BAD_NUMBER: &str = "Il bersaglio è un numero tra parentesi quadre sulla mappa, per esempio 1.";
    pub const ERR_SEED: &str = "Il seme è un numero, per esempio ./play nuova --seme 7";
    pub const ERR_INTERNAL: &str = "Qualcosa è andato storto. Riprova con ./play mostra.";
    pub const ERR_SAVE: &str = "Non riesco a salvare la partita in questa cartella.";
    pub const NOTHING_TO_CANCEL: &str = "Quella risorsa non aveva ordini in questo turno.";

    pub fn err_token(valid: &[&str]) -> String {
        format!("Risorsa sconosciuta: usa una di {}.", valid.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use demo::{Counterfactual, TurnReport};

    fn all_states() -> Vec<TokenState> {
        // Exhaustive by construction: adding a variant breaks this match.
        let v = vec![
            TokenState::Libero,
            TokenState::InViaggio { eta_s: 120 },
            TokenState::AlLavoro,
            TokenState::Rifornimento { eta_s: 90 },
            TokenState::Ritirato,
            TokenState::Perso,
            TokenState::Usato,
            TokenState::InArrivo { eta_s: 600 },
            TokenState::NonChiamato,
        ];
        for s in &v {
            match s {
                TokenState::Libero
                | TokenState::InViaggio { .. }
                | TokenState::AlLavoro
                | TokenState::Rifornimento { .. }
                | TokenState::Ritirato
                | TokenState::Perso
                | TokenState::Usato
                | TokenState::InArrivo { .. }
                | TokenState::NonChiamato => {}
            }
        }
        v
    }

    fn all_effects() -> Vec<Effect> {
        vec![
            Effect::Avvisa { families: 148 },
            Effect::AvvisaTutti { families: 250 },
            Effect::Difende { homes: 60 },
            Effect::Linea,
            Effect::Spegne,
            Effect::Ritirata,
            Effect::Chiamata { eta_s: 1500, turn: 4 },
            Effect::Chiamata { eta_s: 1500, turn: 6 },
            Effect::Bagna { homes: 12 },
            Effect::Bagna { homes: 0 },
            Effect::RallentaPoco,
            Effect::Lontano,
            Effect::Inutile,
        ]
    }

    fn all_report_kinds() -> Vec<ReportKind> {
        use ReportKind::*;
        vec![
            QuartiereRaggiunto,
            MezzoPerso,
            QuartiereMinacciato,
            VentoGirato,
            NuovoFocolaio,
            Ritirata,
            PattugliaArrivata,
            FamiglieInViaggio,
            AutobotteASecco,
            FocolaioSpento,
            CanadairInZona,
            Lancio,
            Arrivato,
            RipetitoreGiu,
            Rinforzo,
            NuovoBollettino,
        ]
    }

    fn all_notes() -> Vec<Note> {
        vec![
            Note::UnitaPersa { token: TokenId::E1 },
            Note::TestaRitirata { token: TokenId::S },
            Note::AutobotteASecco { token: TokenId::E2, district: 0 },
            Note::ItAlertSprecato,
            Note::DifesaInutile { district: 2 },
            Note::CanadairTardi,
            Note::CanadairMaiChiamato,
            Note::CanadairInTempo { drops: 3 },
            Note::FocolaioSpento,
            Note::CaseDifese { district: 0, homes: 30 },
        ]
    }

    fn names() -> Vec<String> {
        vec!["Il Borgo".into(), "Le Coste".into(), "Il Mulino".into()]
    }

    /// The words a playtester must never see.
    fn no_english(s: &str) {
        for w in [" the ", " and ", "unit", "engine", "crew", "withdraw", "Some(", "None", "{", "}"] {
            assert!(!s.contains(w), "English or debug text in {s:?}");
        }
        assert!(!s.trim().is_empty());
    }

    #[test]
    fn every_typed_kind_has_italian() {
        let ds = names();
        for t in TokenId::ALL {
            no_english(token_code(t));
            no_english(&token_name(t));
            no_english(kind_what(t.kind()));
            assert_eq!(token_from_code(&token_code(t).to_lowercase()), Some(t));
            for s in all_states() {
                no_english(&token_state(t, s, 0));
                for r in [Refusal::Finita, Refusal::NonDisponibile, Refusal::Occupato(s), Refusal::BersaglioSconosciuto, Refusal::BersaglioNonValido] {
                    no_english(&refusal(t, r, 0));
                }
            }
            for e in all_effects() {
                no_english(&effect(t, e));
                no_english(&preview(t, &Preview { eta_s: 240, effect: e }));
            }
        }
        let p = scenario::Pos { x: 0.0, y: 0.0 };
        for kind in [
            TargetKind::District(0),
            TargetKind::Head,
            TargetKind::Flank(Side::Left),
            TargetKind::Flank(Side::Right),
            TargetKind::SpotFire(0),
            TargetKind::Sky,
            TargetKind::Town,
        ] {
            for facing in [None, Some(270.0)] {
                let t = Target { id: demo::TargetId(1), kind, pos: p, label_pos: p, facing_deg: facing };
                no_english(&target_name(&t, &ds));
            }
        }
        for k in all_report_kinds() {
            for (d, tok) in [(None, None), (Some(1), Some(TokenId::E1)), (Some(0), Some(TokenId::K))] {
                let l = ReportLine { kind: k, district: d, pos: None, n: Some(3), token: tok };
                no_english(&report_line(&l, &ds));
            }
        }
        for s in Stamp::ALL {
            no_english(stamp(s));
            no_english(stamp_why(s));
            no_english(stamp_mark(s));
        }
        for n in all_notes() {
            no_english(&note(&n, &ds));
        }
        for h in [Headline::TuttiAlSicuro, Headline::OttimoLavoro, Headline::HaiFattoLaDifferenza, Headline::FuocoPiuVeloce] {
            no_english(headline(h));
        }
        for l in [Level::Calm, Level::Watch, Level::Threatened, Level::Reached] {
            no_english(level(l));
        }
        for h in play::HELP {
            no_english(h);
        }
        assert_eq!(play::district_letter("Il Borgo"), 'B');
        assert_eq!(play::district_letter("Le Coste"), 'C');
        assert_eq!(play::district_letter("Il Mulino"), 'M');
        let _ = (TurnReport::default(), Counterfactual::default());
    }

    #[test]
    fn the_wind_reads_the_way_it_blows() {
        assert_eq!(wind_line(180.0, 35.0), "Vento: da Sud, 35 km/h — spinge il fuoco verso Nord");
        assert_eq!(compass(90.0), "Est");
        assert_eq!(compass(359.0), "Nord");
        assert_eq!(fire_at_m(1400.0), "1,4 km");
        assert_eq!(fire_at_m(700.0), "700 m");
        assert_eq!(clock(8 * 60), "T+08");
        assert_eq!(minutes(1), "1′");
        assert_eq!(minutes(240), "4′");
        assert_eq!(con("a", "Il Borgo"), "al Borgo");
        assert_eq!(con("di", "Le Coste"), "delle Coste");
        assert_eq!(con("da", "Il Mulino"), "dal Mulino");
        assert_eq!(con("verso", "Le Coste"), "verso le Coste");
        assert_eq!(con("a", "Rocca Ventosa"), "a Rocca Ventosa");
    }
}
