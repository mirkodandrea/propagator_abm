//! The town kit on the map: landmarks and dressed open spaces.
//!
//! A building whose `kind` is a landmark (church, town hall, school, fire
//! station, utilities, farms, lighthouse) is drawn with its Blender model
//! (`scripts/build_town_models.py`) scaled to its footprint, instead of being
//! extruded. A footprint that is an open space -- piazza, pitch, car park,
//! assembly area, lido, campsite, harbour, cemetery -- is draped on the ground
//! and dressed with props. Everything lands in the building chunks' merged
//! mesh, so it costs no extra draw calls and inherits the chunk's culling.
//!
//! Open spaces are draped vertex by vertex on `Terrain::height_at`, never laid
//! flat (finding 12: a flat plane on a slope is half buried and half floating).

use scenario::{Building, Pos, Scenario};

use super::{hash01, Builder, TOY_HEIGHT, TOY_SCALE};
use crate::models::{kit, Kit};

/// Landmarks drawn from a kit model of the same name.
const MODELLED: [&str; 14] = [
    "church", "chapel", "townhall", "school", "fire_station", "fuel", "water_tower", "substation", "farm", "barn", "silo", "mill",
    "industrial", "lighthouse",
];

/// Paint for parked cars (their bodies are white in the bake).
const PAINT: [[f32; 3]; 7] = [
    [0.93, 0.93, 0.95],
    [0.55, 0.78, 0.95],
    [0.98, 0.85, 0.40],
    [0.60, 0.85, 0.62],
    [0.95, 0.55, 0.50],
    [0.75, 0.70, 0.92],
    [0.35, 0.40, 0.48],
];

/// Draw `b` if it is a landmark or an open space. `false`: not ours, extrude it.
pub(super) fn emit(scn: &Scenario, b: &Building, out: &mut Builder) -> bool {
    let Some(kind) = b.kind.as_deref() else { return false };
    let (min, max) = bbox(b);
    let c = Pos { x: (min.x + max.x) * 0.5, y: (min.y + max.y) * 0.5 };
    let (w, d) = (max.x - min.x, max.y - min.y);
    if MODELLED.contains(&kind) {
        let m = kit(kind);
        let [fw, fd] = m.footprint.unwrap_or([w, d]);
        let (sx, sz) = (w * TOY_SCALE / fw, d * TOY_SCALE / fd);
        let sy = (sx * sz).sqrt() * TOY_HEIGHT / TOY_SCALE;
        let ground = lowest(scn, c, w * TOY_SCALE, d * TOY_SCALE) - 0.4;
        // A plinth of paving so the model sits on its own lot, not on scrub.
        drape(scn, out, c, w * TOY_SCALE + 6.0, d * TOY_SCALE + 6.0, [0.80, 0.77, 0.70], 0.25);
        place(out, m, c, ground, [sx, sy, sz], 0.0, None);
        return true;
    }
    let h = |salt: u64| hash01(b.id as u64, salt);
    match kind {
        "plaza" => {
            drape(scn, out, c, w, d, [0.88, 0.84, 0.76], 0.3);
            drape(scn, out, c, w * 0.55, d * 0.55, [0.80, 0.74, 0.64], 0.35);
            put(scn, out, "fountain", c, 1.6, 0.0, None);
            for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let p = Pos { x: c.x + dx * w * 0.38, y: c.y + dy * d * 0.36 };
                put(scn, out, "street_tree", p, 1.6, h(7) * 6.0, None);
                put(scn, out, "bench", Pos { x: c.x + dx * w * 0.2, y: c.y + dy * d * 0.3 }, 1.8, if dy < 0.0 { 0.0 } else { std::f32::consts::PI }, None);
                put(scn, out, "streetlight", Pos { x: c.x + dx * w * 0.47, y: c.y + dy * d * 0.12 }, 1.4, 0.0, None);
            }
        }
        "pitch" => {
            drape(scn, out, c, w + 8.0, d + 8.0, [0.62, 0.58, 0.52], 0.3);
            drape(scn, out, c, w, d, [0.36, 0.67, 0.33], 0.35);
            let line = [0.97, 0.97, 0.95];
            drape(scn, out, Pos { x: c.x, y: c.y - d * 0.5 }, w, 1.2, line, 0.4);
            drape(scn, out, Pos { x: c.x, y: c.y + d * 0.5 }, w, 1.2, line, 0.4);
            drape(scn, out, Pos { x: c.x - w * 0.5, y: c.y }, 1.2, d, line, 0.4);
            drape(scn, out, Pos { x: c.x + w * 0.5, y: c.y }, 1.2, d, line, 0.4);
            drape(scn, out, c, 1.2, d, line, 0.4);
            drape(scn, out, c, d * 0.22, d * 0.22, line, 0.38);
            drape(scn, out, c, d * 0.22 - 2.4, d * 0.22 - 2.4, [0.36, 0.67, 0.33], 0.39);
            for s in [-1.0f32, 1.0] {
                put(scn, out, "goal", Pos { x: c.x + s * (w * 0.5 - 0.5), y: c.y }, 1.8, s * std::f32::consts::FRAC_PI_2, None);
            }
        }
        "parking" => {
            drape(scn, out, c, w, d, [0.36, 0.37, 0.39], 0.3);
            let bays = ((w / 7.0) as i32).max(2);
            for row in [-1.0f32, 1.0] {
                for i in 0..bays {
                    let x = c.x - w * 0.5 + (i as f32 + 0.5) * w / bays as f32;
                    let y = c.y + row * d * 0.25;
                    drape(scn, out, Pos { x: x - w / bays as f32 * 0.5, y }, 0.5, d * 0.45, [0.95, 0.95, 0.92], 0.34);
                    if hash01(b.id as u64 + i as u64 * 31 + (row > 0.0) as u64 * 7, 0x51) < 0.7 {
                        let car = ["car_hatch", "car_sedan", "car_suv", "car_van"][(hash01(b.id as u64 + i as u64, 0x52) * 4.0) as usize % 4];
                        let paint = PAINT[(hash01(b.id as u64 + i as u64 * 3 + row as u64, 0x53) * 7.0) as usize % 7];
                        put(scn, out, car, Pos { x, y }, 1.25, if row < 0.0 { 0.0 } else { std::f32::consts::PI }, Some(paint));
                    }
                }
            }
        }
        "assembly" => {
            // The Protezione Civile assembly area: a cleared ground with its
            // green sign and the blue tents, the thing the visitor should
            // recognise next time they see one.
            drape(scn, out, c, w, d, [0.70, 0.84, 0.62], 0.3);
            drape(scn, out, c, w - 6.0, d - 6.0, [0.80, 0.86, 0.74], 0.32);
            put(scn, out, "assembly_sign", Pos { x: c.x, y: c.y - d * 0.5 - 4.0 }, 3.2, 0.0, None);
            for i in 0..4 {
                let x = c.x - w * 0.3 + i as f32 * w * 0.2;
                put(scn, out, "pc_tent", Pos { x, y: c.y + d * 0.1 }, 2.2, 0.0, None);
            }
        }
        "lido" => {
            for i in 0..((w / 9.0) as i32) {
                for j in 0..3 {
                    let p = Pos { x: c.x - w * 0.5 + 4.5 + i as f32 * 9.0, y: c.y - d * 0.3 + j as f32 * d * 0.3 };
                    if ground_at(scn, p) < 0.2 {
                        continue;
                    }
                    put(scn, out, if (i + j) % 2 == 0 { "umbrella_a" } else { "umbrella_b" }, p, 1.8, 0.0, None);
                    put(scn, out, "lounger", Pos { x: p.x + 2.4, y: p.y }, 1.8, 0.0, None);
                }
            }
            put(scn, out, "beach_hut", Pos { x: c.x + w * 0.5, y: c.y + d * 0.4 }, 2.2, 0.0, None);
        }
        "campsite" => {
            drape(scn, out, c, w, d, [0.74, 0.70, 0.52], 0.3);
            for i in 0..34u64 {
                let p = Pos { x: c.x + (hash01(b.id as u64 + i, 0x61) - 0.5) * w * 0.9, y: c.y + (hash01(b.id as u64 + i, 0x62) - 0.5) * d * 0.9 };
                let r = hash01(b.id as u64 + i, 0x63);
                let what = if r < 0.35 { "caravan" } else if r < 0.6 { "camp_tent" } else if r < 0.85 { "camp_tent_b" } else { "car_camper" };
                let paint = (what == "car_camper").then_some([0.95, 0.95, 0.92]);
                put(scn, out, what, p, if what.starts_with("camp_tent") { 2.2 } else { 1.4 }, r * 6.3, paint);
            }
        }
        "harbour" => {
            // A pier out into the sea from the middle of the waterfront, and
            // boats moored either side of it.
            let mut y = c.y + d * 0.5;
            let mut k = 0;
            while k < 40 && ground_at(scn, Pos { x: c.x, y }) > -2.5 {
                y -= 4.0;
                k += 1;
            }
            let start = c.y + d * 0.5;
            let len = (start - y).max(20.0);
            let mut t = 0.0;
            while t < len {
                place(out, kit("pier"), Pos { x: c.x, y: start - t }, 0.2, [4.0, 1.0, 4.0], 0.0, None);
                t += 4.0;
            }
            for i in 0..6 {
                for s in [-1.0f32, 1.0] {
                    let p = Pos { x: c.x + s * 9.0, y: start - 10.0 - i as f32 * len / 6.5 };
                    if ground_at(scn, p) < -0.5 {
                        let m = if (i + (s > 0.0) as i32) % 3 == 0 { "sailboat" } else { "boat" };
                        place(out, kit(m), p, 0.0, [1.5, 1.5, 1.5], s * std::f32::consts::FRAC_PI_2, None);
                    }
                }
            }
        }
        "cemetery" => {
            drape(scn, out, c, w, d, [0.74, 0.76, 0.62], 0.3);
            let ring = [
                Pos { x: c.x - w * 0.5, y: c.y - d * 0.5 },
                Pos { x: c.x + w * 0.5, y: c.y - d * 0.5 },
                Pos { x: c.x + w * 0.5, y: c.y + d * 0.5 },
                Pos { x: c.x - w * 0.5, y: c.y + d * 0.5 },
            ];
            for i in 0..4 {
                let (a, e) = (ring[i], ring[(i + 1) % 4]);
                wall(scn, out, a, e, 2.6, 0.8, [0.90, 0.88, 0.82]);
            }
            for i in 0..5 {
                for s in [-1.0f32, 1.0] {
                    let p = Pos { x: c.x - w * 0.4 + i as f32 * w * 0.2, y: c.y + s * d * 0.32 };
                    put(scn, out, "cypress", p, 1.8, 0.0, None);
                }
            }
            put(scn, out, "chapel", Pos { x: c.x, y: c.y + d * 0.15 }, 0.75, 0.0, None);
        }
        _ => return false,
    }
    true
}

fn bbox(b: &Building) -> (Pos, Pos) {
    let (mut lo, mut hi) = (Pos { x: f32::MAX, y: f32::MAX }, Pos { x: f32::MIN, y: f32::MIN });
    for v in &b.ring {
        lo.x = lo.x.min(v[0]);
        lo.y = lo.y.min(v[1]);
        hi.x = hi.x.max(v[0]);
        hi.y = hi.y.max(v[1]);
    }
    (lo, hi)
}

fn ground_at(scn: &Scenario, p: Pos) -> f32 {
    scn.terrain.height_at(p)
}

fn lowest(scn: &Scenario, c: Pos, w: f32, d: f32) -> f32 {
    let mut lo = f32::MAX;
    for dx in [-0.5f32, 0.0, 0.5] {
        for dy in [-0.5f32, 0.0, 0.5] {
            lo = lo.min(ground_at(scn, Pos { x: c.x + dx * w, y: c.y + dy * d }));
        }
    }
    lo
}

/// A kit model at world position `c` (ground height `y`), scaled per axis and
/// turned `yaw` radians about the vertical. `paint` recolours its white parts.
fn place(out: &mut Builder, m: &Kit, c: Pos, y: f32, s: [f32; 3], yaw: f32, paint: Option<[f32; 3]>) {
    let (sn, cs) = yaw.sin_cos();
    let start = out.positions.len() as u32;
    for ((p, n), col) in m.positions.iter().zip(&m.normals).zip(&m.colors) {
        let (x, z) = (p[0] * s[0], p[2] * s[2]);
        let (rx, rz) = (x * cs + z * sn, -x * sn + z * cs);
        out.positions.push([c.x + rx, y + p[1] * s[1], -c.y + rz]);
        // Normals under non-uniform scale: divide by the scale, renormalise.
        let (nx, ny, nz) = (n[0] / s[0], n[1] / s[1], n[2] / s[2]);
        let (rnx, rnz) = (nx * cs + nz * sn, -nx * sn + nz * cs);
        let l = (rnx * rnx + ny * ny + rnz * rnz).sqrt().max(1e-6);
        out.normals.push([rnx / l, ny / l, rnz / l]);
        let white = col[0] > 0.99 && col[1] > 0.99 && col[2] > 0.99;
        let c3 = match paint {
            Some(pc) if white => pc,
            _ => *col,
        };
        out.colors.push([c3[0], c3[1], c3[2], 1.0]);
    }
    out.indices.extend(m.indices.iter().map(|i| start + i));
}

/// A kit prop at `p` on the ground, uniformly scaled.
fn put(scn: &Scenario, out: &mut Builder, name: &str, p: Pos, scale: f32, yaw: f32, paint: Option<[f32; 3]>) {
    let y = ground_at(scn, p).max(0.0);
    place(out, kit(name), p, y - 0.1, [scale; 3], yaw, paint);
}

/// A rectangle draped on the terrain, `lift` metres above it, ~8 m posting.
fn drape(scn: &Scenario, out: &mut Builder, c: Pos, w: f32, d: f32, color: [f32; 3], lift: f32) {
    let nx = ((w / 8.0).ceil() as usize).max(1);
    let ny = ((d / 8.0).ceil() as usize).max(1);
    let at = |i: usize, j: usize| {
        let p = Pos { x: c.x - w * 0.5 + w * i as f32 / nx as f32, y: c.y - d * 0.5 + d * j as f32 / ny as f32 };
        [p.x, ground_at(scn, p).max(0.0) + lift, -p.y]
    };
    for i in 0..nx {
        for j in 0..ny {
            // Counter-clockwise seen from above (Bevy -Z is north), so the face
            // points up (finding 11).
            out.quad(at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1), color);
        }
    }
}

/// A low wall from `a` to `b`, following the ground.
fn wall(scn: &Scenario, out: &mut Builder, a: Pos, b: Pos, h: f32, t: f32, color: [f32; 3]) {
    let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
    let n = ((len / 6.0).ceil() as usize).max(1);
    let (ux, uy) = ((b.x - a.x) / len, (b.y - a.y) / len);
    let (px, py) = (-uy * t * 0.5, ux * t * 0.5);
    for k in 0..n {
        let p0 = Pos { x: a.x + ux * len * k as f32 / n as f32, y: a.y + uy * len * k as f32 / n as f32 };
        let p1 = Pos { x: a.x + ux * len * (k + 1) as f32 / n as f32, y: a.y + uy * len * (k + 1) as f32 / n as f32 };
        let ring = [
            Pos { x: p0.x - px, y: p0.y - py },
            Pos { x: p1.x - px, y: p1.y - py },
            Pos { x: p1.x + px, y: p1.y + py },
            Pos { x: p0.x + px, y: p0.y + py },
        ];
        let g = ground_at(scn, p0).min(ground_at(scn, p1)).max(0.0);
        // Wind CCW for `prism`, whichever way the wall runs.
        let ring: Vec<Pos> = if super::signed_area(&ring) < 0.0 { ring.iter().rev().copied().collect() } else { ring.to_vec() };
        out.prism(&ring, g - 0.5, g + h, color, [color[0] * 0.9, color[1] * 0.9, color[2] * 0.9]);
    }
}

/// A garden tree behind about half the houses, and a hedge-side bush: the
/// greenery between houses that a town is drawn with.
pub(super) fn garden(scn: &Scenario, b: &Building, out: &mut Builder) {
    if !matches!(b.kind.as_deref(), Some("house" | "villa" | "terrace")) {
        return;
    }
    let (min, max) = bbox(b);
    let c = Pos { x: (min.x + max.x) * 0.5, y: (min.y + max.y) * 0.5 };
    let h = hash01(b.id as u64, 0xA1);
    if h < 0.55 {
        let side = if hash01(b.id as u64, 0xA2) < 0.5 { -1.0 } else { 1.0 };
        let p = Pos { x: c.x + side * (max.x - min.x) * 0.95, y: c.y + (hash01(b.id as u64, 0xA3) - 0.5) * 8.0 };
        put(scn, out, if h < 0.15 { "cypress" } else { "street_tree" }, p, 1.3 + h, h * 6.3, None);
    }
}
