//! Every string the player can read, in one place (spec §7.7).
//!
//! No literal in a widget: if it is visible it is here, which is what makes
//! "no English on screen" a thing a native speaker can review in one file.
//! District names are place names and come from the town data.
//! NOTE: placeholder copy, still to be read by a native speaker.

use demo::EventKind;

use super::Speaker;

pub const TITLE: &str = "Comandante dell'incidente";
pub const SUBTITLE: &str = "Settimana della Protezione Civile";
pub const TAGLINE: &str = "Un incendio, tre quartieri, il vento che decide. Tocca a te.";
pub const START: &str = "Clicca per iniziare";

pub const GO: &str = "Via!";
pub const RETRY: &str = "Riprova";
pub const ANOTHER_TOWN: &str = "Un altro paese";

// --- Briefing (the clock is stopped) ----------------------------------------
pub const BRIEF_HOW: &str = "Il tempo è fermo. Guarda il vento e le previsioni, poi avvisa i quartieri in pericolo e manda le autobotti. Quando sei pronto, premi Via!";
pub const BRIEF_TIP: &str = "Avvisare presto salva le famiglie. Avvisare chi non è in pericolo è un falso allarme: a ogni falso allarme la gente ci crede un po' meno.";

// --- HUD -----------------------------------------------------------------
pub const SAFE: &str = "al sicuro";
pub const MOVING: &str = "in viaggio";
pub const DANGER: &str = "in pericolo";
pub const HOMES_LOST: &str = "case colpite";
pub const WIND_FROM: &str = "Vento da";
pub const TIME_LEFT: &str = "Tempo";
pub const PAUSED: &str = "In pausa";
pub const SPENT: &str = "Spesa";

// --- Forecast (spec 6.1) ---------------------------------------------------
pub const FORECAST: &str = "Previsioni";
pub const FORECAST_UPDATED: &str = "Nuove previsioni";
pub const SHIFT_CHANCE: &str = "Probabilità che il vento giri";
pub const FORECAST_CAVEAT: &str = "Sono previsioni: possono sbagliare";

pub fn forecast_wind(dir: &str, kmh: f32, cone: f32) -> String {
    format!("Vento da {dir} · {kmh:.0} km/h (±{cone:.0}°)")
}

pub fn forecast_shift(to: &str, a: u32, b: u32) -> String {
    format!("Se gira: da {to}, tra {a} e {b} min")
}

// --- District chips -------------------------------------------------------
pub const WARN: &str = "Avvisa";
pub const DEFEND: &str = "Difendi";
pub const WARNED: &str = "Avvisati";
pub const NO_ENGINES: &str = "nessuna libera";
pub const FIRE_HERE: &str = "Il fuoco è qui!";
pub const CALM: &str = "Tranquillo";
pub const FIRE_TAG: &str = "Incendio";

/// Moved to `crates/text`, shared with the `play` binary.
pub use text::{fire_at, households};

pub fn leaving(out: usize, total: usize) -> String {
    format!("{out}/{total} via")
}

pub fn engines_posted(n: usize) -> String {
    match n {
        1 => "1 autobotte".into(),
        n => format!("{n} autobotti"),
    }
}


// --- Action bar ----------------------------------------------------------
pub const ACT_EVACUATE: &str = "Allerta generale";
pub const ACT_EVACUATE_SUB: &str = "avvisa tutti i quartieri";
pub const ACT_EVACUATE_DONE: &str = "tutti avvisati";
pub const ACT_ENGINE: &str = "Autobotti";
pub const ACT_AIR: &str = "Canadair";
pub const ACT_PAUSE: &str = "Pausa";
pub const ACT_RESUME: &str = "Riprendi";
pub const ACT_PAUSE_SUB: &str = "ferma il tempo";
pub const ACT_FAST: &str = "Veloce";
pub const ACT_FAST_ON: &str = "x3";
pub const ACT_FAST_SUB: &str = "avanti veloce";
pub const ACT_AIR_ASK: &str = "arrivo in 25 min";
pub const ACT_AIR_READY: &str = "clicca un quartiere";
pub const PENDING_ENGINE: &str = "Autobotte: clicca vicino a una strada o usa «Difendi» sui quartieri";

pub fn free_engines(n: usize) -> String {
    match n {
        0 => NO_ENGINES.into(),
        1 => "1 libera".into(),
        n => format!("{n} libere"),
    }
}

pub fn air_inbound(min: u32, sec: u32) -> String {
    format!("in arrivo {min}:{sec:02}")
}

pub const PENDING_DROP: &str = "Canadair: clicca sulla mappa dove lanciare";

// --- Advisors ----------------------------------------------------------------
pub fn speaker(s: Speaker) -> &'static str {
    match s {
        Speaker::Fire => "Capo squadra VVF",
        Speaker::Mayor => "Sindaco",
        Speaker::Weather => "Meteo",
    }
}

pub const ADVISOR_START: &str = "Il fuoco corre col vento: guarda la freccia. Dove punta, lì arriverà.";

pub fn forecast_update(p: f32, to: &str) -> String {
    format!("Nuove previsioni: probabilità che il vento giri da {to} al {:.0}%.", p * 100.0)
}

/// What an advisor says about a model event, if anything. `warned[k]`: whether
/// district `k` has been warned.
pub fn advisor(kind: &EventKind, names: &[String], warned: &[bool]) -> Option<(Speaker, String)> {
    let name = |k: usize| names.get(k).cloned().unwrap_or_default();
    Some(match *kind {
        EventKind::SpotFire => (Speaker::Fire, "Un nuovo focolaio! Le scintille hanno fatto saltare il fuoco più avanti.".into()),
        EventKind::WindShifted { to_deg, .. } => {
            (Speaker::Weather, format!("Il vento è girato: ora soffia da {}. Quale quartiere ha davanti?", crate::kiosk::ui::bearing_name(to_deg)))
        }
        EventKind::DistrictThreatened { district } if warned.get(district) == Some(&true) => {
            (Speaker::Mayor, format!("Il fuoco è vicino a {}. Le famiglie sono state avvisate.", name(district)))
        }
        EventKind::DistrictThreatened { district } => {
            (Speaker::Mayor, format!("Il fuoco è vicino a {} e nessuno li ha avvisati!", name(district)))
        }
        EventKind::DistrictReached { district } => (Speaker::Fire, format!("Il fuoco è arrivato a {}.", name(district))),
        EventKind::FalseAlarm => (Speaker::Mayor, "La gente si lamenta: un avviso era un falso allarme. Al prossimo ci crederanno meno.".into()),
        EventKind::UnitWithdrew { .. } => (Speaker::Fire, "Una squadra si ritira: il calore è troppo forte. Tornerà appena passa il fronte.".into()),
        EventKind::UnitLost { .. } => (Speaker::Fire, "Abbiamo perso un mezzo nel fuoco.".into()),
        EventKind::MastDown => (Speaker::Mayor, "Il fuoco ha abbattuto un ripetitore: alcune famiglie non ricevono più gli avvisi sul telefono.".into()),
        EventKind::FireNearTown => return None,
    })
}

// --- Idle ----------------------------------------------------------------
pub const STILL_THERE: &str = "Sei ancora lì?";
pub const STILL_THERE_SUB: &str = "Tocca per continuare, altrimenti si ricomincia.";

// --- Outcome -------------------------------------------------------------
pub const OUTCOME_TITLE: &str = "Fine dell'incidente";
pub const FAMILIES_SAFE: &str = "famiglie al sicuro";
pub const CAUGHT: &str = "sorprese in casa dal fuoco";
pub const WITHOUT_ORDERS: &str = "senza ordini";
pub const TWIN_WAIT: &str = "Calcolo: cosa sarebbe successo senza ordini…";
pub const TWIN_NA: &str = "Confronto non disponibile";
pub const BADGES: &str = "Medaglie";
pub const BADGE_IN_TIME: &str = "In tempo";
pub const BADGE_IN_TIME_HINT: &str = "avvisa almeno 10 minuti prima del fuoco";
pub const BADGE_NO_FALSE: &str = "Nessun falso allarme";
pub const BADGE_NO_FALSE_HINT: &str = "avvisa solo chi è in pericolo";
pub const BADGE_DEFENDED: &str = "Case difese";
pub const BADGE_DEFENDED_HINT: &str = "manda le autobotti prima del fuoco";
pub const EVAC_SAVES_PEOPLE: &str = "L'evacuazione salva le persone; le case le difendono le autobotti.";

pub fn saved_vs_none(saved: i64) -> String {
    match saved {
        s if s > 1 => format!("Hai salvato {s} famiglie rispetto a nessun ordine"),
        1 => "Hai salvato 1 famiglia rispetto a nessun ordine".into(),
        0 => "Stesso risultato che senza ordini".into(),
        s => format!("{} famiglie in più sorprese che senza ordini", -s),
    }
}

pub fn headline(caught: usize, saved: Option<i64>) -> &'static str {
    match (caught, saved) {
        (0, _) => "Tutti al sicuro!",
        (c, Some(s)) if s > 0 && c <= 3 => "Ottimo lavoro!",
        (_, Some(s)) if s > 0 => "Hai fatto la differenza",
        (_, Some(_)) => "Il fuoco è stato più veloce",
        _ => "Fine dell'incidente",
    }
}

/// One line per district for the end card.
pub fn district_story(r: &demo::district::Report) -> (String, Mood) {
    let t = |s: i64| format!("T+{}", s / 60);
    match (r.warned_at_s, r.reached_at_s) {
        (Some(w), Some(a)) if (a - w) / 60 >= demo::district::IN_TIME_MIN => {
            (format!("Avvisato a {} · fuoco a {}: {} min di anticipo", t(w), t(a), (a - w) / 60), Mood::Good)
        }
        (Some(w), Some(a)) if a > w => (format!("Avvisato a {}, ma il fuoco è arrivato a {}: poco tempo", t(w), t(a)), Mood::Meh),
        (Some(w), Some(a)) => (format!("Avvisato a {}, dopo il fuoco ({})", t(w), t(a)), Mood::Bad),
        (None, Some(a)) => (format!("Mai avvisato · fuoco arrivato a {}", t(a)), Mood::Bad),
        (Some(w), None) if r.needless() => (format!("Avvisato a {} · il fuoco non è arrivato: falso allarme", t(w)), Mood::Meh),
        (Some(w), None) => (format!("Avvisato a {} · il fuoco si è fermato vicino", t(w)), Mood::Good),
        (None, None) => ("Il fuoco non è arrivato: giusto non allarmarli".into(), Mood::Good),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mood {
    Good,
    Meh,
    Bad,
}

pub fn spent_eur(eur: f32) -> String {
    let k = (eur / 1000.0).round() as i64;
    if k == 0 {
        "0 €".into()
    } else {
        format!("{k}.000 €")
    }
}

pub fn caught_line(n: usize) -> String {
    match n {
        1 => "1 famiglia sorpresa in casa".into(),
        n => format!("{n} famiglie sorprese in casa"),
    }
}

/// The lesson of the session, from the badges and the districts.
pub fn lesson(b: demo::district::Badges, any_reached: bool, any_needless: bool) -> &'static str {
    if !any_reached {
        "Questa volta il fuoco non ha raggiunto nessun quartiere. Capire dove va il vento evita allarmi inutili."
    } else if !b.in_time {
        "Il fuoco va dove soffia il vento, e veloce. Le famiglie hanno bisogno di tempo: avvisa prima."
    } else if any_needless {
        "In tempo! Ma un avviso era inutile: ogni falso allarme fa perdere fiducia."
    } else if !b.homes_defended {
        "Famiglie avvisate in tempo. Le autobotti, mandate presto dove va il vento, salvano anche le case."
    } else {
        "Hai letto il vento, avvisato in tempo e difeso le case. Così lavora la Protezione Civile."
    }
}

// --- Towns ---------------------------------------------------------------
pub struct TownText {
    pub name: &'static str,
    pub place: &'static str,
    pub brief: &'static str,
}

pub fn town(id: &str) -> TownText {
    match id {
        "demo_borgo" => TownText {
            name: "Rocca Ventosa",
            place: "Un borgo sulla collina e due frazioni",
            brief: "Un incendio è partito nella pineta sotto il paese. Il vento lo spinge in salita, ma oggi potrebbe girare.",
        },
        "demo_valle" => TownText {
            name: "Due Casali",
            place: "Una valle, due casali e una frazione",
            brief: "Un'auto ha preso fuoco sulla provinciale, tra i due casali. Il vento spinge le fiamme verso ovest. Se gira, tocca all'altro casale.",
        },
        "demo_porto" => TownText {
            name: "Porto Pineta",
            place: "Un paese di mare con una sola strada",
            brief: "Il fuoco è nella pineta dietro il paese. L'unica strada passa proprio di lì: il lungomare è l'area di attesa sicura.",
        },
        _ => TownText { name: "Paese", place: "", brief: "" },
    }
}

// --- Refusals ------------------------------------------------------------
/// The model's refusals are English sentences; the player reads these.
pub fn refusal(english: &str) -> &'static str {
    match english {
        s if s.contains("outside the scenario") => "Quel punto è fuori mappa.",
        s if s.contains("No connected road") => "Non c'è una strada che arrivi fin lì.",
        s if s.contains("Outside hose reach") => "È troppo lontano dalla strada per la manichetta.",
        s if s.contains("Approach blocked") => "Il fuoco ha tagliato la strada per arrivarci.",
        s if s.contains("No suppressible fuel") => "Lì non c'è niente da bagnare: scegli vegetazione non ancora bruciata.",
        s if s.contains("aircraft drop") => "Gli aerei lanciano acqua, non aprono linee.",
        s if s.contains("only aircraft") => "Solo gli aerei possono lanciare.",
        s if s.contains("Select") => "Scegli prima cosa fare.",
        _ => "Non si può fare lì.",
    }
}
