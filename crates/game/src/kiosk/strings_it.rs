//! Every string the player can read, in one place (spec §7.7).
//!
//! No literal in a widget: if it is visible it is here, which is what makes
//! "no English on screen" a thing a native speaker can review in one file.
//! NOTE: placeholder copy, still to be read by a native speaker.

pub const TITLE: &str = "Comandante dell'incidente";
pub const SUBTITLE: &str = "Settimana della Protezione Civile";
pub const START: &str = "Clicca per iniziare";

pub const GO: &str = "Comincia";
pub const SKIP: &str = "Salta";
pub const RETRY: &str = "Riprova";
pub const ANOTHER_TOWN: &str = "Un altro paese";
pub const SEE_COMPARE: &str = "Cosa sarebbe successo senza ordini?";
pub const BACK: &str = "Indietro";

// --- HUD -----------------------------------------------------------------
pub const SAFE: &str = "Al sicuro";
pub const MOVING: &str = "In fuga";
pub const DANGER: &str = "In pericolo";
pub const HOMES_LOST: &str = "Case colpite";
pub const FAMILIES: &str = "famiglie";
pub const WIND_FROM: &str = "Vento da";
pub const FIRE_GOES: &str = "il fuoco va verso";
pub const TIME_LEFT: &str = "Tempo rimasto";
pub const PAUSED: &str = "In pausa";

// --- Forecast (spec 6.1) ---------------------------------------------------
pub const FORECAST: &str = "Previsioni";
pub const FORECAST_UPDATED: &str = "Previsioni aggiornate";
pub const FORECAST_NEW: &str = "Nuove previsioni del vento: guardale!";
pub const SHIFT_CHANCE: &str = "Probabilità che il vento cambi";
pub const FORECAST_CAVEAT: &str = "Sono previsioni: possono sbagliare";

pub fn forecast_wind(dir: &str, kmh: f32, cone: f32) -> String {
    format!("Vento da {dir} · {kmh:.0} km/h (±{cone:.0}°)")
}

pub fn forecast_shift(to: &str, a: u32, b: u32) -> String {
    format!("poi da {to}, tra {a} e {b} min")
}

// --- Action bar ----------------------------------------------------------
pub const ACT_EVACUATE: &str = "Evacuazione";
pub const ACT_EVACUATE_SUB: &str = "avvisa tutte le famiglie";
pub const ACT_EVACUATE_DONE: &str = "ordine dato";
pub const ACT_CREW: &str = "Squadra";
pub const ACT_ENGINE: &str = "Autobotte";
pub const ACT_AIR: &str = "Canadair";
pub const ACT_PAUSE: &str = "Pausa";
pub const ACT_RESUME: &str = "Riprendi";
pub const ACT_PAUSE_SUB: &str = "ferma il tempo";
pub const ACT_NONE_LEFT: &str = "tutte impegnate";
pub const ACT_AIR_ASK: &str = "arrivo in 25 min";
pub const ACT_AIR_READY: &str = "sul posto";

pub fn free_units(n: usize) -> String {
    match n {
        1 => "1 libera".to_string(),
        n => format!("{n} libere"),
    }
}

pub fn air_inbound(min: u32, sec: u32) -> String {
    format!("in arrivo {min}:{sec:02}")
}

pub fn pending_order(action: &str) -> String {
    format!("{action}: clicca sulla mappa dove intervenire")
}
pub const PENDING_LINE: &str = "clicca sulla mappa per scegliere il punto";
pub const CANCEL: &str = "Annulla";

// --- Idle ----------------------------------------------------------------
pub const STILL_THERE: &str = "Sei ancora lì?";
pub const STILL_THERE_SUB: &str = "Tocca per continuare, altrimenti si ricomincia.";

// --- Outcome -------------------------------------------------------------
pub const OUTCOME_TITLE: &str = "Fine dell'incidente";
pub const COMPARE_TITLE: &str = "Con i tuoi ordini e senza";
pub const YOU: &str = "Tu";
pub const NO_ORDERS: &str = "Nessun ordine";
pub const FAMILIES_SAFE: &str = "famiglie al sicuro";
pub const STILL_IN_DANGER: &str = "ancora in pericolo";
pub const HOMES_LOST_LC: &str = "case colpite dal fuoco";
pub const HECTARES: &str = "ettari bruciati";
pub const CAUGHT: &str = "sorprese dal fuoco in casa";

pub fn safe_of(safe: usize, total: usize) -> String {
    format!("{safe}/{total}")
}

/// One takeaway, chosen from the difference between the run and its twin.
pub fn takeaway(you_caught: usize, cf_caught: usize, you_safe: usize, cf_safe: usize) -> String {
    if you_caught + 3 <= cf_caught {
        let n = cf_caught - you_caught;
        format!(
            "Le tue decisioni hanno cambiato le cose: {n} famiglie non sono state sorprese dal fuoco in casa. Avvisare presto è ciò che conta di più."
        )
    } else if you_safe > cf_safe + 2 {
        let n = you_safe - cf_safe;
        format!("Grazie ai tuoi ordini {n} famiglie in più sono arrivate al sicuro.")
    } else {
        "Questa volta i tuoi ordini non hanno cambiato molto. Prova ad avvisare prima: il fuoco va dove soffia il vento e il tempo è la cosa che manca.".to_string()
    }
}

// --- Towns ---------------------------------------------------------------
pub struct TownText {
    pub name: &'static str,
    pub place: &'static str,
    pub brief: &'static str,
    pub mission: &'static str,
}

pub fn town(id: &str) -> TownText {
    match id {
        "demo_borgo" => TownText {
            name: "Rocca Ventosa",
            place: "Un borgo sulla collina",
            brief: "Un incendio è partito sotto il paese e il vento lo spinge in salita. Le famiglie non si sono accorte di nulla. Quando vuoi dare l'allarme?",
            mission: "Porta tutte le famiglie al sicuro",
        },
        "demo_valle" => TownText {
            name: "Due Casali",
            place: "Una valle con due frazioni",
            brief: "Il fuoco corre verso la prima frazione. Ma il vento oggi non è stabile: tieni d'occhio la freccia, perché prima o poi cambia.",
            mission: "Proteggi entrambe le frazioni",
        },
        "demo_porto" => TownText {
            name: "Porto Pineta",
            place: "Un paese sul mare con una sola strada",
            brief: "Il fuoco è nella pineta dietro il paese. Una sola strada porta fuori, e passa proprio lì vicino. Il fuoco può saltare avanti con le scintille.",
            mission: "Fai uscire tutti prima che la strada si chiuda",
        },
        _ => TownText { name: "Paese", place: "", brief: "", mission: "" },
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
        s if s.contains("No suppressible fuel") => "Lì non c'è niente da difendere: scegli vegetazione non ancora bruciata.",
        s if s.contains("cannot cut line") => "Un'autobotte non può aprire una linea: manda una squadra.",
        s if s.contains("aircraft drop") => "Gli aerei lanciano acqua, non aprono linee.",
        s if s.contains("only aircraft") => "Solo gli aerei possono lanciare.",
        s if s.contains("Select") => "Scegli prima cosa fare.",
        _ => "Non si può fare lì.",
    }
}
