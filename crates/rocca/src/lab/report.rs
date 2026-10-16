//! Tables out of a batch: the raw CSVs and a Markdown reading of them.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use anyhow::Result;

use super::run::RunOut;

fn q(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn num(v: f64) -> String {
    if v.is_nan() {
        String::new()
    } else {
        format!("{v:.4}")
    }
}

/// `runs.csv` (one row per run), `series.csv` and `units.csv` (if kept).
pub fn write_csv(dir: &Path, runs: &[RunOut]) -> Result<()> {
    fs::create_dir_all(dir)?;
    let mut cols: Vec<String> = vec![];
    for r in runs {
        for (k, _) in &r.metrics.0 {
            if !cols.contains(k) {
                cols.push(k.clone());
            }
        }
    }
    let mut s = String::from("scenario,strategy,teams,seed,tune,aim,without");
    for c in &cols {
        s += ",";
        s += &q(c);
    }
    s += "\n";
    for r in runs {
        let j = &r.job;
        let _ = write!(s, "{},{},{},{},{},{:?},{}", q(&j.scenario), q(&j.strategy), j.teams, j.seed, q(&j.tune_tag), j.aim, j.without.map_or(String::new(), |k| k.to_string()));
        for c in &cols {
            s += ",";
            s += &r.metrics.get(c).map_or(String::new(), num);
        }
        s += "\n";
    }
    fs::write(dir.join("runs.csv"), s)?;

    if runs.iter().any(|r| !r.series.is_empty()) {
        let mut s = String::from("scenario,strategy,teams,seed,tune,t_min,burning,burnt_ha,reach_m,homes_hit,homes_threatened,cleared_cells,wet_cells,litres,wind_from,wind_kmh\n");
        let mut u = String::from("scenario,strategy,teams,seed,tune,t_min,unit,team,kind,state,x,y,fire_m,threat,heat_s,water_frac,water_used_l,line_cut_m,note\n");
        for r in runs {
            let j = &r.job;
            let id = format!("{},{},{},{},{}", q(&j.scenario), q(&j.strategy), j.teams, j.seed, q(&j.tune_tag));
            for p in &r.series {
                let _ = writeln!(s, "{id},{:.1},{},{:.2},{:.0},{},{},{},{},{:.0},{:.0},{:.0}", p.t_min, p.burning, p.burnt_ha, p.reach_m, p.homes_hit, p.homes_threatened, p.cleared_cells, p.wet_cells, p.litres, p.wind_from, p.wind_kmh);
            }
            for p in &r.unit_series {
                let _ = writeln!(u, "{id},{:.1},{},{},{},{},{:.0},{:.0},{:.0},{:.3},{:.1},{:.2},{:.0},{:.1},{}", p.t_min, p.unit, p.team, p.kind, p.state, p.pos.x, p.pos.y, p.fire_m, p.threat, p.heat_s, p.water_frac, p.water_used_l, p.line_cut_m, q(p.note));
            }
        }
        fs::write(dir.join("series.csv"), s)?;
        fs::write(dir.join("units.csv"), u)?;
    }
    Ok(())
}

fn finite(v: impl Iterator<Item = f64>) -> Vec<f64> {
    v.filter(|x| x.is_finite()).collect()
}

pub fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        f64::NAN
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

pub fn sd(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return f64::NAN;
    }
    let m = mean(v);
    (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
}

type Key = (String, String, usize, String, String);

struct Group<'a>(Vec<&'a RunOut>);

impl Group<'_> {
    fn m(&self, k: &str) -> f64 {
        mean(&finite(self.0.iter().filter_map(|r| r.metrics.get(k))))
    }
    fn sd(&self, k: &str) -> f64 {
        sd(&finite(self.0.iter().filter_map(|r| r.metrics.get(k))))
    }
}

fn f(v: f64, d: usize) -> String {
    if v.is_nan() {
        "–".into()
    } else {
        format!("{v:.d$}")
    }
}

fn pct(v: f64) -> String {
    if v.is_nan() {
        "–".into()
    } else {
        format!("{:.0}%", v * 100.0)
    }
}

fn groups(runs: &[RunOut]) -> BTreeMap<Key, Group<'_>> {
    let mut m: BTreeMap<Key, Group> = BTreeMap::new();
    for r in runs {
        m.entry(r.job.key()).or_insert_with(|| Group(vec![])).0.push(r);
    }
    m
}

/// The Markdown reading: results per scenario, scaling with the number of
/// teams, the effect of each dial, the value of each team, and what looks
/// wrong with the algorithms.
pub fn markdown(runs: &[RunOut]) -> String {
    let g = groups(runs);
    let mut s = String::new();
    let seeds: Vec<u64> = {
        let mut v: Vec<u64> = runs.iter().map(|r| r.job.seed).collect();
        v.sort();
        v.dedup();
        v
    };
    let _ = writeln!(s, "# Laboratorio di spegnimento\n\n{} corse, semi {:?}. «Risparmio» = ettari in meno rispetto allo stesso fuoco senza ordini (stesso seme); media sui semi, ± deviazione standard.\n", runs.len(), seeds);

    let scenarios: Vec<String> = {
        let mut v: Vec<String> = g.keys().map(|k| k.0.clone()).collect();
        v.dedup();
        v
    };
    for sc in &scenarios {
        let _ = writeln!(s, "## {sc}\n");
        let _ = writeln!(s, "| strategia | sq. | taratura | ettari | senza ordini | risparmio | case colpite (senza) | perse | L/ha salvato | autobotte al lavoro | squadre a piedi al lavoro | primo contatto min | acqua tenuta | linea tenuta | scarto autobotte al fronte m |");
        let _ = writeln!(s, "|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
        for (k, grp) in g.iter().filter(|(k, _)| &k.0 == sc && k.4.is_empty()) {
            if k.1 == "none" {
                continue;
            }
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} ±{} | {} | {}% ±{} | {} ({}) | {} | {} | {} | {} | {} | {} | {} | {} |",
                k.1,
                k.2,
                k.3,
                f(grp.m("ha"), 0),
                f(grp.sd("ha"), 0),
                f(grp.m("base_ha"), 0),
                f(grp.m("saved_pct"), 0),
                f(grp.sd("saved_pct"), 0),
                f(grp.m("homes_hit"), 1),
                f(grp.m("base_homes_hit"), 1),
                f(grp.m("units_lost"), 1),
                f(grp.m("litres_per_ha_saved"), 0),
                pct(grp.m("engine_working_share")),
                pct(grp.m("crew_working_share")),
                f(grp.m("first_contact_min"), 0),
                pct(grp.m("wet_held_share")),
                pct(grp.m("line_held_share")),
                f(grp.m("gap_at_arrival_m"), 0),
            );
        }
        let _ = writeln!(s);
    }

    // best strategy per scenario and team count
    let _ = writeln!(s, "## Migliore per scenario\n");
    let _ = writeln!(s, "| scenario | sq. | migliore | risparmio | seconda | risparmio |");
    let _ = writeln!(s, "|---|---:|---|---:|---|---:|");
    let mut best: BTreeMap<(String, usize), Vec<(f64, String)>> = BTreeMap::new();
    for (k, grp) in g.iter().filter(|(k, _)| k.4.is_empty() && k.3.is_empty() && k.1 != "none") {
        best.entry((k.0.clone(), k.2)).or_default().push((grp.m("saved_pct"), k.1.clone()));
    }
    for ((sc, n), mut v) in best {
        v.sort_by(|a, b| b.0.total_cmp(&a.0));
        let _ = writeln!(s, "| {sc} | {n} | {} | {}% | {} | {}% |", v[0].1, f(v[0].0, 0), v.get(1).map_or("–".into(), |x| x.1.clone()), v.get(1).map_or("–".into(), |x| f(x.0, 0)));
    }

    // scaling: saved ha against team count, per strategy over all scenarios
    let _ = writeln!(s, "\n## Quante squadre servono\n\nRisparmio medio (ettari) su tutti gli scenari, per strategia e numero di squadre; l'ultima colonna è il ritorno marginale dell'ultima squadra aggiunta.\n");
    let mut by: BTreeMap<String, BTreeMap<usize, Vec<f64>>> = BTreeMap::new();
    for (k, grp) in g.iter().filter(|(k, _)| k.4.is_empty() && k.3.is_empty() && k.1 != "none") {
        by.entry(k.1.clone()).or_default().entry(k.2).or_default().push(grp.m("saved_ha"));
    }
    let ns: Vec<usize> = {
        let mut v: Vec<usize> = by.values().flat_map(|m| m.keys().copied()).collect();
        v.sort();
        v.dedup();
        v
    };
    let _ = write!(s, "| strategia |");
    for n in &ns {
        let _ = write!(s, " {n} sq. |");
    }
    let _ = writeln!(s, " marginale |");
    let _ = write!(s, "|---|");
    for _ in &ns {
        let _ = write!(s, "---:|");
    }
    let _ = writeln!(s, "---:|");
    for (name, m) in &by {
        let _ = write!(s, "| {name} |");
        let mut vals = vec![];
        for n in &ns {
            let v = m.get(n).map_or(f64::NAN, |x| mean(x));
            vals.push(v);
            let _ = write!(s, " {} |", f(v, 1));
        }
        let marg = if vals.len() >= 2 { vals[vals.len() - 1] - vals[vals.len() - 2] } else { f64::NAN };
        let _ = writeln!(s, " {} |", f(marg, 1));
    }

    // the effect of each dial: paired against the default tuning
    let dials: Vec<&Key> = g.keys().filter(|k| !k.3.is_empty() && k.4.is_empty()).collect();
    if !dials.is_empty() {
        let _ = writeln!(s, "\n## Effetto delle tarature\n\nDifferenza di risparmio (punti percentuali) e di case colpite rispetto alla taratura predefinita, a parità di scenario, strategia e squadre; media sui semi e sugli scenari.\n");
        let _ = writeln!(s, "| taratura | Δ risparmio (pp) | Δ case colpite | Δ squadre perse | Δ L/ha | corse |");
        let _ = writeln!(s, "|---|---:|---:|---:|---:|---:|");
        let mut tags: Vec<String> = dials.iter().map(|k| k.3.clone()).collect();
        tags.sort();
        tags.dedup();
        for tag in tags {
            let (mut d_s, mut d_h, mut d_l, mut d_w, mut n) = (vec![], vec![], vec![], vec![], 0);
            for (k, grp) in g.iter().filter(|(k, _)| k.3 == tag && k.4.is_empty()) {
                let Some(base) = g.get(&(k.0.clone(), k.1.clone(), k.2, String::new(), String::new())) else { continue };
                d_s.push(grp.m("saved_pct") - base.m("saved_pct"));
                d_h.push(grp.m("homes_hit") - base.m("homes_hit"));
                d_l.push(grp.m("units_lost") - base.m("units_lost"));
                d_w.push(grp.m("litres_per_ha_saved") - base.m("litres_per_ha_saved"));
                n += grp.0.len();
            }
            let _ = writeln!(s, "| {tag} | {} | {} | {} | {} | {n} |", f(mean(&finite(d_s.into_iter())), 1), f(mean(&finite(d_h.into_iter())), 2), f(mean(&finite(d_l.into_iter())), 2), f(mean(&finite(d_w.into_iter())), 0));
        }
    }

    // the value of each team: leave one out
    let loo: Vec<&Key> = g.keys().filter(|k| !k.4.is_empty()).collect();
    if !loo.is_empty() {
        let _ = writeln!(s, "\n## Valore di ogni squadra (una in meno)\n\nEttari in più bruciati senza la squadra indicata, rispetto al roster completo (media sui semi). Una squadra con valore ≈ 0 non serve; negativo = la sua assenza migliora.\n");
        let _ = writeln!(s, "| scenario | strategia | sq. | taratura | tolta | ettari in più | case in più | perse in meno |");
        let _ = writeln!(s, "|---|---|---:|---|---|---:|---:|---:|");
        for (k, grp) in g.iter().filter(|(k, _)| !k.4.is_empty()) {
            let Some(full) = g.get(&(k.0.clone(), k.1.clone(), k.2, k.3.clone(), String::new())) else { continue };
            let _ = writeln!(s, "| {} | {} | {} | {} | {} | {} | {} | {} |", k.0, k.1, k.2, k.3, k.4, f(grp.m("ha") - full.m("ha"), 0), f(grp.m("homes_hit") - full.m("homes_hit"), 1), f(grp.m("units_lost") - full.m("units_lost"), 1));
        }
    }

    s += &findings(runs, &g);
    s
}

/// Mechanical reading of the numbers: where an algorithm wastes its people.
fn findings(runs: &[RunOut], g: &BTreeMap<Key, Group<'_>>) -> String {
    let mut s = String::from("\n## Segnali sugli algoritmi\n\n");
    let all = Group(runs.iter().filter(|r| r.job.strategy != "none" && r.job.without.is_none()).collect());
    if all.0.is_empty() {
        return s;
    }
    let _ = writeln!(s, "Su {} corse con ordini (tarature predefinite e non):\n", all.0.len());
    let _ = writeln!(s, "- Autobotte: {} del tempo al lavoro, {} in viaggio, {} in ricarica, {} ferma. A piedi: {} al lavoro, {} in viaggio, {} ferma.", pct(all.m("engine_working_share")), pct(all.m("engine_responding_share")), pct(all.m("engine_refilling_share")), pct(all.m("engine_staged_share")), pct(all.m("crew_working_share")), pct(all.m("crew_responding_share")), pct(all.m("crew_staged_share")));
    let _ = writeln!(s, "- Acqua: {} delle celle bagnate che il fuoco ha toccato ha poi bruciato (fallita); {} sono state tenute; {} non sono mai state messe alla prova.", pct(all.m("wet_failed_share")), pct(all.m("wet_held_share")), pct(all.m("wet_untested_share")));
    let _ = writeln!(s, "- Linee: tenuta in {} dei tratti messi alla prova dal fuoco.", pct(all.m("line_held_share")));
    let _ = writeln!(s, "- Dove lavorano le squadre rispetto al fuoco: davanti {}, di fianco {}, dietro {}.", pct(all.m("work_ahead_share")), pct(all.m("work_flank_share")), pct(all.m("work_behind_share")));
    let _ = writeln!(s, "- Sicurezza: {} squadre perse per corsa, {} ritirate, distanza minima dell'autobotte dal fuoco {} m, calore massimo {} s su {} s.", f(all.m("units_lost"), 2), f(all.m("withdrawals"), 1), f(all.m("engine_min_fire_m"), 0), f(all.m("max_heat_s"), 0), abm::suppression::BURNOVER_S);
    let _ = writeln!(s, "- L'autobotte cambia posto da sola {} volte per corsa; si arriva al fronte, in media, a {} m dalla fiamma più vicina.", f(all.m("engine_relocations"), 1), f(all.m("gap_at_arrival_m"), 0));

    // what keeps a unit from working: the notes, by minutes
    let mut notes: BTreeMap<String, f64> = BTreeMap::new();
    let mut n = 0.0;
    for r in &all.0 {
        n += 1.0;
        for (k, v) in &r.metrics.0 {
            if let Some(name) = k.strip_prefix("note[").and_then(|x| x.strip_suffix("]_min")) {
                *notes.entry(name.to_string()).or_default() += v;
            }
        }
    }
    let mut notes: Vec<_> = notes.into_iter().collect();
    notes.sort_by(|a, b| b.1.total_cmp(&a.1));
    if !notes.is_empty() {
        let _ = writeln!(s, "\nPerché un'unità non lavora o lavora male (minuti per corsa, somma delle unità):\n");
        for (k, v) in notes.iter().take(8) {
            let _ = writeln!(s, "- {k}: {:.0} min", v / n);
        }
    }

    // which strategies lose people
    let mut risky: Vec<(f64, &Key)> = g.iter().filter(|(k, _)| k.4.is_empty() && k.1 != "none").map(|(k, grp)| (grp.m("units_lost"), k)).filter(|x| x.0 > 0.0).collect();
    risky.sort_by(|a, b| b.0.total_cmp(&a.0));
    if !risky.is_empty() {
        let _ = writeln!(s, "\nStrategie che perdono squadre (media per corsa):\n");
        for (v, k) in risky.iter().take(8) {
            let _ = writeln!(s, "- {} su {} con {} squadre{}: {:.2}", k.1, k.0, k.2, if k.3.is_empty() { String::new() } else { format!(" ({})", k.3) }, v);
        }
    }
    s
}

/// Print the Markdown and save it.
pub fn write_markdown(dir: &Path, runs: &[RunOut]) -> Result<String> {
    let md = markdown(runs);
    fs::write(dir.join("report.md"), &md)?;
    Ok(md)
}
