//! Chunky toy meshes built in code: people, cars, engines, vans, the air
//! tanker, refuge signs. One vertex-coloured mesh each, a few hundred
//! triangles, neutral tones so the live status tint on the material still
//! reads (the same contract as the Blender bake it replaces).
//!
//! Parts are convex, and every triangle is wound by checking it against the
//! part's own centre, so a part cannot come out back-facing (finding 11: a
//! back-facing mesh draws nothing while the logs say it exists).

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

#[derive(Default)]
pub struct Toy {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

type V3 = Vec3;

impl Toy {
    fn tri(&mut self, a: (V3, V3), b: (V3, V3), c: (V3, V3), centre: V3, color: [f32; 3]) {
        let g = (b.0 - a.0).cross(c.0 - a.0);
        let mid = (a.0 + b.0 + c.0) / 3.0;
        let flip = g.dot(mid - centre) < 0.0;
        let (b, c) = if flip { (c, b) } else { (b, c) };
        let s = self.pos.len() as u32;
        for v in [a, b, c] {
            self.pos.push(v.0.into());
            self.nrm.push(v.1.into());
            self.col.push([color[0], color[1], color[2], 1.0]);
        }
        self.idx.extend_from_slice(&[s, s + 1, s + 2]);
    }

    /// Axis-aligned box, flat shaded. `c` centre, `h` half extents.
    pub fn cuboid(&mut self, c: V3, h: V3, color: [f32; 3]) -> &mut Self {
        for axis in 0..3 {
            for sign in [-1.0f32, 1.0] {
                let mut n = V3::ZERO;
                n[axis] = sign;
                let t1 = {
                    let mut t = V3::ZERO;
                    t[(axis + 1) % 3] = 1.0;
                    t
                };
                let t2 = {
                    let mut t = V3::ZERO;
                    t[(axis + 2) % 3] = 1.0;
                    t
                };
                let (h0, h1, h2) = (h[axis], h[(axis + 1) % 3], h[(axis + 2) % 3]);
                let o = c + n * h0;
                let p = |a: f32, b: f32| o + t1 * h1 * a + t2 * h2 * b;
                let shade = 0.78 + 0.22 * (n.y * 0.5 + 0.5) + if n.y > 0.5 { 0.0 } else { 0.0 };
                let col = [color[0] * shade, color[1] * shade, color[2] * shade];
                let q = [p(-1.0, -1.0), p(1.0, -1.0), p(1.0, 1.0), p(-1.0, 1.0)];
                self.tri((q[0], n), (q[1], n), (q[2], n), c, col);
                self.tri((q[0], n), (q[2], n), (q[3], n), c, col);
            }
        }
        self
    }

    /// Smooth ellipsoid, darker belly.
    pub fn ellipsoid(&mut self, c: V3, r: V3, color: [f32; 3]) -> &mut Self {
        const SEG: usize = 10;
        const RINGS: usize = 6;
        let vert = |i: usize, j: usize| -> (V3, V3, f32) {
            let phi = i as f32 / RINGS as f32 * std::f32::consts::PI;
            let a = j as f32 / SEG as f32 * std::f32::consts::TAU;
            let u = V3::new(phi.sin() * a.cos(), phi.cos(), phi.sin() * a.sin());
            (c + u * r, (u / r).normalize(), phi.cos())
        };
        for i in 0..RINGS {
            for j in 0..SEG {
                let (p00, p01) = (vert(i, j), vert(i, j + 1));
                let (p10, p11) = (vert(i + 1, j), vert(i + 1, j + 1));
                let shade = |y: f32| 0.70 + 0.30 * (y * 0.5 + 0.5);
                let k = shade((p00.2 + p11.2) * 0.5);
                let col = [color[0] * k, color[1] * k, color[2] * k];
                if i > 0 {
                    self.tri((p00.0, p00.1), (p01.0, p01.1), (p10.0, p10.1), c, col);
                }
                if i < RINGS - 1 {
                    self.tri((p01.0, p01.1), (p11.0, p11.1), (p10.0, p10.1), c, col);
                }
            }
        }
        self
    }

    /// Upright cylinder, smooth sides, flat caps.
    pub fn cylinder(&mut self, base: V3, radius: f32, height: f32, color: [f32; 3]) -> &mut Self {
        const SEG: usize = 8;
        let c = base + V3::Y * height * 0.5;
        let at = |j: usize, y: f32| -> (V3, V3) {
            let a = j as f32 / SEG as f32 * std::f32::consts::TAU;
            let d = V3::new(a.cos(), 0.0, a.sin());
            (base + d * radius + V3::Y * y, d)
        };
        for j in 0..SEG {
            let (b0, b1, t0, t1) = (at(j, 0.0), at(j + 1, 0.0), at(j, height), at(j + 1, height));
            self.tri(b0, b1, t1, c, color);
            self.tri(b0, t1, t0, c, color);
            let top = base + V3::Y * height;
            self.tri((top, V3::Y), (t0.0, V3::Y), (t1.0, V3::Y), c, [color[0] * 1.1, color[1] * 1.1, color[2] * 1.1]);
        }
        self
    }

    pub fn finish(self) -> Mesh {
        let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        let n = self.pos.len();
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.pos);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.col);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; n]);
        m.insert_indices(Indices::U32(self.idx));
        m
    }
}

const WHITE: [f32; 3] = [1.0, 1.0, 1.0];
const GLASS: [f32; 3] = [0.16, 0.22, 0.30];
const TYRE: [f32; 3] = [0.08, 0.08, 0.09];
const SKIN: [f32; 3] = [0.98, 0.84, 0.72];

/// Round, oversized bean figure about 1.8 m tall: body white (takes the status
/// tint), pale head. Origin at the feet, facing +Z.
pub fn person() -> Mesh {
    let mut t = Toy::default();
    t.ellipsoid(V3::new(0.0, 0.62, 0.0), V3::new(0.46, 0.62, 0.40), WHITE);
    t.ellipsoid(V3::new(0.0, 1.50, 0.02), V3::new(0.36, 0.36, 0.34), SKIN);
    t.cuboid(V3::new(0.0, 0.1, 0.05), V3::new(0.30, 0.1, 0.26), [0.25, 0.22, 0.2]);
    t.finish()
}

/// Three of them close together: a hand crew. Yellow once tinted.
pub fn crew() -> Mesh {
    let mut t = Toy::default();
    for (x, z, s) in [(-0.9, 0.5, 0.95), (0.9, 0.4, 1.0), (0.0, -0.7, 1.05)] {
        let k = s;
        t.ellipsoid(V3::new(x, 0.62 * k, z), V3::new(0.46, 0.62, 0.40) * k, WHITE);
        t.ellipsoid(V3::new(x, 1.50 * k, z), V3::new(0.36, 0.36, 0.34) * k, [0.95, 0.95, 0.95]);
        // hard hat
        t.ellipsoid(V3::new(x, 1.72 * k, z), V3::new(0.40, 0.2, 0.38) * k, [0.9, 0.9, 0.9]);
    }
    // a tool bundle, so it reads as work and not as a gathering
    t.cuboid(V3::new(0.0, 0.5, 0.9), V3::new(0.9, 0.06, 0.06), [0.4, 0.3, 0.2]);
    t.finish()
}

/// Toy car, 4.4 m long, nose along +Z. Body takes the tint; glass, tyres dark.
pub fn car() -> Mesh {
    let mut t = Toy::default();
    t.cuboid(V3::new(0.0, 0.62, 0.0), V3::new(0.95, 0.38, 2.15), WHITE);
    t.cuboid(V3::new(0.0, 1.12, -0.2), V3::new(0.80, 0.34, 1.05), GLASS);
    t.cuboid(V3::new(0.0, 1.30, -0.2), V3::new(0.78, 0.18, 0.95), WHITE);
    for (x, z) in [(-0.95, 1.3), (0.95, 1.3), (-0.95, -1.3), (0.95, -1.3)] {
        t.cuboid(V3::new(x, 0.32, z), V3::new(0.14, 0.32, 0.38), TYRE);
    }
    // headlights, pale
    t.cuboid(V3::new(-0.6, 0.7, 2.17), V3::new(0.22, 0.12, 0.05), [1.0, 0.97, 0.7]);
    t.cuboid(V3::new(0.6, 0.7, 2.17), V3::new(0.22, 0.12, 0.05), [1.0, 0.97, 0.7]);
    t.finish()
}

/// Fire engine: tall cab, long tank, ladder rack, blue lights. 7 m, nose +Z.
pub fn engine() -> Mesh {
    let mut t = Toy::default();
    t.cuboid(V3::new(0.0, 1.45, 2.2), V3::new(1.3, 1.1, 1.1), WHITE); // cab
    t.cuboid(V3::new(0.0, 1.9, 2.95), V3::new(1.15, 0.45, 0.06), GLASS); // windscreen
    t.cuboid(V3::new(0.0, 1.55, -0.9), V3::new(1.3, 1.2, 2.9), [0.92, 0.92, 0.92]); // body
    t.cuboid(V3::new(0.0, 2.95, -0.5), V3::new(0.45, 0.12, 2.6), [0.7, 0.72, 0.75]); // ladder
    t.cuboid(V3::new(0.0, 2.6, 2.2), V3::new(0.7, 0.14, 0.2), [0.2, 0.4, 1.0]); // lightbar
    for z in [2.2, -1.6] {
        for x in [-1.3f32, 1.3] {
            t.cuboid(V3::new(x, 0.5, z), V3::new(0.16, 0.5, 0.55), TYRE);
        }
    }
    t.finish()
}

/// Hand-crew van, mid-sized, nose +Z. Pale body, roof rack.
pub fn van() -> Mesh {
    let mut t = Toy::default();
    t.cuboid(V3::new(0.0, 1.25, -0.3), V3::new(1.0, 0.95, 2.1), WHITE);
    t.cuboid(V3::new(0.0, 1.0, 2.05), V3::new(1.0, 0.7, 0.55), WHITE);
    t.cuboid(V3::new(0.0, 1.45, 2.3), V3::new(0.9, 0.3, 0.1), GLASS);
    t.cuboid(V3::new(0.0, 2.35, -0.3), V3::new(0.8, 0.08, 1.6), [0.45, 0.4, 0.35]); // rack with tools
    for (x, z) in [(-1.0, 1.7), (1.0, 1.7), (-1.0, -1.6), (1.0, -1.6)] {
        t.cuboid(V3::new(x, 0.38, z), V3::new(0.14, 0.38, 0.42), TYRE);
    }
    t.finish()
}

/// Air tanker: fat fuselage, high straight wing, twin nacelles, T-tail. Nose
/// along -Z? No: nose along +X, matching the old slab mesh's `-FRAC_PI_2`
/// heading offset in `units::update_units` (long axis +Z there; kept +Z).
pub fn tanker() -> Mesh {
    let mut t = Toy::default();
    t.ellipsoid(V3::new(0.0, 0.0, 0.0), V3::new(1.7, 1.6, 7.5), WHITE); // fuselage
    t.cuboid(V3::new(0.0, 1.0, 0.8), V3::new(10.5, 0.22, 1.7), [0.95, 0.95, 0.95]); // wing
    t.ellipsoid(V3::new(-4.2, 0.5, 1.6), V3::new(0.9, 0.9, 1.8), [0.8, 0.8, 0.8]);
    t.ellipsoid(V3::new(4.2, 0.5, 1.6), V3::new(0.9, 0.9, 1.8), [0.8, 0.8, 0.8]);
    t.cuboid(V3::new(0.0, 2.7, -6.4), V3::new(0.22, 2.0, 1.0), [0.9, 0.55, 0.3]); // fin
    t.cuboid(V3::new(0.0, 4.5, -6.5), V3::new(3.8, 0.16, 0.9), [0.9, 0.9, 0.9]); // tailplane
    t.cuboid(V3::new(0.0, 0.3, 5.6), V3::new(1.0, 0.5, 1.5), GLASS); // canopy
    t.finish()
}

/// A refuge: a flat piazza disc with a blue "P"-style sign post and flag.
pub fn refuge_sign() -> Mesh {
    let mut t = Toy::default();
    t.cylinder(V3::ZERO, 22.0, 0.6, [0.80, 0.78, 0.72]);
    t.cylinder(V3::new(0.0, 0.6, 0.0), 14.0, 0.5, [0.55, 0.78, 0.92]);
    t.cylinder(V3::new(0.0, 0.6, 0.0), 1.0, 22.0, [0.9, 0.9, 0.9]);
    t.cuboid(V3::new(0.0, 25.0, 0.0), V3::new(7.0, 5.0, 0.8), [0.1, 0.5, 0.95]);
    t.cuboid(V3::new(0.0, 25.0, 0.9), V3::new(4.5, 3.2, 0.2), [1.0, 1.0, 1.0]);
    t.finish()
}
