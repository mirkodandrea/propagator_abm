"""Run with:
    /Applications/Blender.app/Contents/MacOS/Blender --background --python scripts/build_island_models.py

The island's landmarks and boats: the rocca (castle) on the summit, the stone
tower on the headland, and the sailing boats. Authored at real size in metres
(the game scales them up for the diorama, `island.json` -> `landmarks`), baked
to `assets/models/island.json` the way `build_town_models.py` bakes `town.json`
(positions, normals, flat colours, indices), and a contact sheet is rendered to
`assets/models/island_preview.png`.

Blender: metres, Z up, front -Y. Runtime: metres, Y up, front +Z (so the bake
maps (x, y, z) to (x, z, -y)). Origin: the centre of the footprint, on the
ground. The rocca's gate faces +X, a boat's bow points +X.
"""
import bpy
import json
import math
from pathlib import Path
from mathutils import Vector, Matrix

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets/models"
OUT.mkdir(parents=True, exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True)

materials = {}


def lin(c):
    """The palette below is written as the sRGB the eye should see; Blender's
    material colour (and so the bake, and Bevy's vertex colour) is linear."""
    return ((c + .055) / 1.055) ** 2.4 if c > .04045 else c / 12.92


def mat(color):
    color = tuple(lin(c) for c in color)
    key = tuple(round(c, 4) for c in color)
    if key not in materials:
        m = bpy.data.materials.new(f"c{len(materials)}")
        m.diffuse_color = (*color, 1)
        m.use_nodes = True
        bsdf = m.node_tree.nodes.get("Principled BSDF")
        bsdf.inputs["Base Color"].default_value = (*color, 1)
        bsdf.inputs["Roughness"].default_value = .8
        materials[key] = m
    return materials[key]


# --- palette: limestone, terracotta, timber ------------------------------------------
STONE = (.80, .68, .50)
STONE_L = (.90, .79, .60)
STONE_M = (.68, .56, .41)
STONE_D = (.52, .43, .33)
TUFA = (.60, .56, .50)
IVY = (.24, .38, .17)
IVY_D = (.16, .28, .12)
MORTAR = (.60, .56, .50)
TERRACOTTA = (.76, .36, .22)
TERRA_D = (.62, .28, .18)
WOOD = (.42, .28, .16)
WOOD_D = (.28, .18, .10)
IRON = (.22, .22, .24)
DARK = (.07, .07, .09)
GLASS = (.16, .22, .30)
RED = (.78, .12, .10)
WHITE = (.96, .96, .93)
CREAM = (.94, .88, .72)
NAVY = (.14, .22, .42)
GREEN = (.12, .36, .30)
YELLOW = (.97, .78, .24)
BLUE = (.20, .38, .68)
ANTIFOUL = (.62, .14, .12)
DECK = (.78, .68, .50)
METAL = (.66, .68, .70)

current = None
ROCK = False      # while True, new objects are bedrock: baked apart as `<name>_roccia`, drawn with the terrain's rock shader


def link(o, color):
    for c in list(o.users_collection):
        c.objects.unlink(o)
    current.objects.link(o)
    o.data.materials.clear()
    o.data.materials.append(mat(color))
    o["rock"] = ROCK
    return o


def apply(o, loc=False, rot=True, scale=True):
    bpy.context.view_layer.objects.active = o
    bpy.ops.object.select_all(action="DESELECT")
    o.select_set(True)
    bpy.ops.object.transform_apply(location=loc, rotation=rot, scale=scale)


def box(pos, size, color, yaw=0.0):
    bpy.ops.mesh.primitive_cube_add(size=1, location=pos)
    o = bpy.context.object
    o.scale = size
    o.rotation_euler = (0, 0, yaw)
    apply(o, rot=True)
    return link(o, color)


def cyl(base, radius, height, color, sides=16, top=None):
    """Upright cylinder (or frustum, `top` = top radius) standing on `base`."""
    bpy.ops.mesh.primitive_cone_add(vertices=sides, radius1=radius, radius2=radius if top is None else top, depth=height,
                                    location=(base[0], base[1], base[2] + height / 2))
    return link(bpy.context.object, color)


def cone(base, radius, height, color, sides=16):
    return cyl(base, radius, height, color, sides, top=0.0)


def rod(a, b, radius, color, sides=6):
    a, b = Vector(a), Vector(b)
    bpy.ops.mesh.primitive_cylinder_add(vertices=sides, radius=radius, depth=(b - a).length, location=(a + b) / 2)
    o = bpy.context.object
    o.rotation_euler = (b - a).to_track_quat("Z", "Y").to_euler()
    apply(o, rot=True, scale=False)
    return link(o, color)


def surface(verts, faces, color, centre=None, both=False):
    """A mesh from raw data. `centre`: flip each face to point away from it (for
    hulls); `both`: add the reverse winding too (sails seen from either side)."""
    verts = [Vector(v) for v in verts]
    out = []
    for f in faces:
        f = list(f)
        if centre is not None:
            n = (verts[f[1]] - verts[f[0]]).cross(verts[f[2]] - verts[f[0]])
            mid = sum((verts[i] for i in f), Vector()) / len(f)
            if n.dot(mid - Vector(centre)) < 0:
                f.reverse()
        out.append(f)
        if both:
            out.append(list(reversed(f)))
    me = bpy.data.meshes.new("m")
    me.from_pydata([tuple(v) for v in verts], [], out)
    me.update()
    o = bpy.data.objects.new("o", me)
    bpy.context.scene.collection.objects.link(o)
    return link(o, color)


def merlons(a, b, z, step, size, color, offset=0.0):
    """Crenellations along a wall top from a to b (2D), `size` = (along, across, high)."""
    a, b = Vector((a[0], a[1], 0)), Vector((b[0], b[1], 0))
    d = (b - a)
    length = d.length
    d.normalize()
    n = Vector((-d.y, d.x, 0))
    count = max(1, int(length // step))
    gap = length / count
    yaw = math.atan2(d.y, d.x)
    for i in range(count):
        c = a + d * (gap * (i + .5)) + n * offset
        box((c.x, c.y, z + size[2] / 2), size, color, yaw)


def merlon_ring(centre, radius, z, count, size, color):
    for i in range(count):
        t = 2 * math.pi * (i + .5) / count
        box((centre[0] + math.cos(t) * radius, centre[1] + math.sin(t) * radius, z + size[2] / 2), size, color, t + math.pi / 2)


def slit(pos, yaw, w=.5, h=2.0, depth=.5):
    box(pos, (depth, w, h), DARK, yaw)


# =========================================================================================
#  the rocca: an abandoned hill fortress on a crag: broken towers, fallen walls, scrub
# =========================================================================================
import random
RNG = random.Random(11)
IVY_P = .22
CREN_P = 1.0
HALF = 31.0          # the curtain wall's half width: `LANDMARKS["rocca"]["wall_half_m"]` in island.py
WALL_H, WALL_T = 11.0, 3.0


def prism(verts, faces, color, centre):
    return surface(verts, faces, color, centre=centre)


def rough_block(cx, cy, l, t, heights, yaw, color, z0=0.0):
    """A block on the ground whose top is uneven: `heights` = z of its four
    top corners (-l/-t, +l/-t, +l/+t, -l/+t), the way a broken wall ends."""
    c, s_ = math.cos(yaw), math.sin(yaw)
    loc = [(-l / 2, -t / 2), (l / 2, -t / 2), (l / 2, t / 2), (-l / 2, t / 2)]
    pts = [(cx + x * c - y * s_, cy + x * s_ + y * c) for x, y in loc]
    verts = [(x, y, z0) for x, y in pts] + [(x, y, h) for (x, y), h in zip(pts, heights)]
    faces = [(0, 1, 2, 3), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)]
    return prism(verts, faces, color, (cx, cy, max(heights) / 2))


def frustum(cx, cy, z0, z1, bottom, top, yaw, color):
    """A box that narrows upward: the scarp (talus) at a wall's foot."""
    c, s_ = math.cos(yaw), math.sin(yaw)

    def ring(size, z):
        return [(cx + x * c - y * s_, cy + x * s_ + y * c, z) for x, y in
                ((-size[0] / 2, -size[1] / 2), (size[0] / 2, -size[1] / 2), (size[0] / 2, size[1] / 2), (-size[0] / 2, size[1] / 2))]
    verts = ring(bottom, z0) + ring(top, z1)
    faces = [(0, 1, 2, 3), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)]
    return prism(verts, faces, color, (cx, cy, (z0 + z1) / 2))


def ghibelline(cx, cy, z, yaw, w=2.0, t=1.1, h=2.4, color=None):
    color = color or STONE_L
    """A swallow-tail merlon: the Ghibelline battlement."""
    c, s_ = math.cos(yaw), math.sin(yaw)
    prof = [(-w / 2, 0), (w / 2, 0), (w / 2, h), (0, h * .58), (-w / 2, h)]
    verts = []
    for ty in (-t / 2, t / 2):
        for u, v in prof:
            verts.append((cx + u * c - ty * s_, cy + u * s_ + ty * c, z + v))
    n = len(prof)
    faces = [tuple(range(n)), tuple(range(2 * n - 1, n - 1, -1))]
    faces += [(i, (i + 1) % n, n + (i + 1) % n, n + i) for i in range(n)]
    return prism(verts, faces, color, (cx, cy, z + h / 2))


def rubble(cx, cy, radius, count, color=None, big=2.0):
    color = color or STONE_M
    for _ in range(count):
        a, r = RNG.uniform(0, 2 * math.pi), radius * math.sqrt(RNG.random())
        sz = RNG.uniform(.6, big)
        box((cx + math.cos(a) * r, cy + math.sin(a) * r, sz * .35), (sz * RNG.uniform(.8, 1.5), sz, sz * .7),
            RNG.choice((color, STONE, STONE_D, TUFA)), RNG.uniform(0, 3.14))


def ivy(cx, cy, z, r=1.4):
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=1, location=(cx, cy, z))
    o = bpy.context.object
    o.scale = (r * RNG.uniform(.8, 1.3), r * RNG.uniform(.8, 1.3), r * .6)
    apply(o, rot=False)
    link(o, RNG.choice((IVY, IVY_D)))


def arch_window(pos, yaw, w=1.5, h=2.6):
    """A round-headed opening: a dark box and its arch."""
    x, y, z = pos
    box((x, y, z), (.5 if abs(math.cos(yaw)) > .5 else w, w if abs(math.cos(yaw)) > .5 else .5, h), DARK)
    cx, cy = (x, y)
    bpy.ops.mesh.primitive_cylinder_add(vertices=8, radius=w / 2, depth=.5, location=(cx, cy, z + h / 2))
    o = bpy.context.object
    o.rotation_euler = (0, math.pi / 2, yaw) if abs(math.cos(yaw)) > .5 else (math.pi / 2, 0, 0)
    apply(o, rot=True, scale=False)
    link(o, DARK)


def ruined_wall(a, b, ctrl, T, color=None, scarp=True, crenel=True, step=5.0, window=False):
    color = color or STONE
    """A wall from a to b, standing to the heights in `ctrl` (interpolated along
    its length): broken, with a talus at the foot, swallow-tail merlons where it
    is whole, rubble where it has fallen, ivy on what is left."""
    a, b = Vector((a[0], a[1], 0)), Vector((b[0], b[1], 0))
    d = b - a
    L = d.length
    n = max(2, int(round(L / step)))
    yaw = math.atan2(d.y, d.x)
    nrm = Vector((-d.y, d.x, 0)).normalized()
    xs = [i / n for i in range(n + 1)]
    cs = [i / (len(ctrl) - 1) for i in range(len(ctrl))]
    hv = []
    for u in xs:
        k = min(range(len(cs) - 1), key=lambda j: abs(((cs[j] + cs[j + 1]) / 2) - u))
        k = max(0, min(len(cs) - 2, int(u * (len(cs) - 1))))
        f = u * (len(cs) - 1) - k
        hv.append(ctrl[k] * (1 - f) + ctrl[k + 1] * f)
    top_ref = max(ctrl)
    for i in range(n):
        p0, p1 = a + d * xs[i], a + d * xs[i + 1]
        mid = (p0 + p1) / 2
        l = (p1 - p0).length + .08
        h0, h1 = hv[i], hv[i + 1]
        jit = [RNG.uniform(-.5, .5) for _ in range(2)]
        heights = [max(.5, h0 + jit[0]), max(.5, h1 + jit[1]), max(.5, h1 - jit[1]), max(.5, h0 - jit[0])]
        rough_block(mid.x, mid.y, l, T, heights, yaw, color)
        low = min(h0, h1)
        if scarp and low > 3.0:
            frustum(mid.x, mid.y, 0, min(low * .4, 4.0), (l + .1, T + 2.6), (l + .1, T), yaw, STONE_D)
        if crenel and h0 >= .93 * top_ref and h1 >= .93 * top_ref and RNG.random() < CREN_P:
            for k in (.28, .72):
                q = p0 + (p1 - p0) * k + nrm * (T / 2 - .55)
                ghibelline(q.x, q.y, (h0 + h1) / 2 + .1, yaw)
        if low < .5 * top_ref:
            rubble(mid.x + nrm.x * 2.0, mid.y + nrm.y * 2.0, 3.2, 4)
            rubble(mid.x - nrm.x * 2.2, mid.y - nrm.y * 2.2, 2.6, 2)
        if RNG.random() < IVY_P and low > 3:
            ivy(mid.x + nrm.x * (T / 2), mid.y + nrm.y * (T / 2), low * RNG.uniform(.5, .95))
        if window and low > 7.0 and i % 2 == 1:
            arch_window((mid.x + nrm.x * (T / 2 + .1), mid.y + nrm.y * (T / 2 + .1), 4.4), yaw + math.pi / 2, 1.4, 2.8)


def broken_round_tower(c, r, ctrl, scarp=7.0):
    """A round tower of masonry blocks, standing to `ctrl[i]` round its circle."""
    x, y = c
    n = len(ctrl)
    cyl((x, y, 0), r * 1.2, scarp, STONE_D, 16, top=r * 1.02)
    for i in range(n):
        t0, t1 = 2 * math.pi * i / n, 2 * math.pi * (i + 1) / n
        tm = (t0 + t1) / 2
        l = 2 * r * math.sin(math.pi / n) * 1.04
        h = ctrl[i]
        heights = [h + RNG.uniform(-.6, .4) for _ in range(4)]
        rough_block(x + math.cos(tm) * r, y + math.sin(tm) * r, l, 2.4, heights, tm + math.pi / 2, STONE)
        if h > 14:
            ghibelline(x + math.cos(tm) * (r + .2), y + math.sin(tm) * (r + .2), h, tm + math.pi / 2, w=2.2, h=2.3)
        if RNG.random() < .25:
            ivy(x + math.cos(tm) * (r + 1.0), y + math.sin(tm) * (r + 1.0), h * RNG.uniform(.4, .8))
    cyl((x, y, 0), r * .92, min(ctrl) * .55, STONE_M, 16)               # the floor of the broken storey
    for k in (0.4, 2.7, 4.9):
        slit((x + math.cos(k) * r * 1.0, y + math.sin(k) * r * 1.0, 8.5), k, .6, 2.4, .6)


# weathered, grey-ochre limestone: the rocca is a ruin on a crag, nobody lives there
CRAG = (.50, .47, .42)
CRAG_D = (.34, .32, .29)
CRAG_L = (.62, .58, .50)
MOSS = (.30, .42, .20)
GRASS = (.42, .50, .24)
DEAD = (.30, .25, .20)


def crag_rock(cx, cy, z0, r, h, color, sides=7, strata=3, lean=0.0):
    """A faceted outcrop of limestone: stacked strata, each a little narrower and
    offset from the one below, with ragged corners, so it reads as a ledged cliff."""
    global ROCK
    was, ROCK = ROCK, True
    z = z0
    ox, oy = 0.0, 0.0
    rr = r
    for k in range(strata):
        sh = h / strata * RNG.uniform(.8, 1.2)
        ph = RNG.uniform(0, 6.28)
        low = [(cx + ox + math.cos(ph + 2 * math.pi * i / sides) * rr * RNG.uniform(.78, 1.12),
                cy + oy + math.sin(ph + 2 * math.pi * i / sides) * rr * RNG.uniform(.78, 1.12), z) for i in range(sides)]
        rr *= RNG.uniform(.68, .86)
        ox += lean * RNG.uniform(.3, 1.0) + RNG.uniform(-.4, .4) * r * .2
        oy += RNG.uniform(-.4, .4) * r * .2
        high = [(cx + ox + math.cos(ph + 2 * math.pi * i / sides) * rr * RNG.uniform(.8, 1.1),
                 cy + oy + math.sin(ph + 2 * math.pi * i / sides) * rr * RNG.uniform(.8, 1.1), z + sh * RNG.uniform(.85, 1.15)) for i in range(sides)]
        verts = low + high
        faces = [tuple(range(sides)), tuple(range(2 * sides - 1, sides - 1, -1))]
        faces += [(i, (i + 1) % sides, sides + (i + 1) % sides, sides + i) for i in range(sides)]
        surface(verts, faces, color if k % 2 == 0 else RNG.choice((CRAG_D, CRAG_L)), centre=(cx + ox, cy + oy, z + sh / 2))
        z += sh * .95
    ROCK = was
    return z


def scrub(x, y, z, r=1.3):
    """Wild scrub growing in the stonework: where it grows, nobody weeds."""
    for _ in range(3):
        bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=1, location=(x + RNG.uniform(-r, r), y + RNG.uniform(-r, r), z + r * .35))
        o = bpy.context.object
        o.scale = (r * RNG.uniform(.6, 1.0), r * RNG.uniform(.6, 1.0), r * RNG.uniform(.45, .8))
        apply(o, rot=False)
        link(o, RNG.choice((IVY, IVY_D, GRASS, MOSS)))


def dead_tree(x, y, z, h=7.0):
    """A bare, bent trunk with a few snapped limbs, in the courtyard."""
    top = (x + RNG.uniform(-.8, .8), y + RNG.uniform(-.8, .8), z + h)
    rod((x, y, z), top, .32, DEAD, 5)
    for k in range(4):
        t = k * 1.7 + RNG.uniform(0, .6)
        s = z + h * (.45 + .12 * k)
        base = (x + (top[0] - x) * (s - z) / h, y + (top[1] - y) * (s - z) / h, s)
        rod(base, (base[0] + math.cos(t) * 2.4, base[1] + math.sin(t) * 2.4, s + RNG.uniform(.8, 2.0)), .13, DEAD, 4)


def fallen_drum(x, y, r, length, yaw, tilt=.0):
    """A length of tower shaft that came down and lies on its side."""
    bpy.ops.mesh.primitive_cylinder_add(vertices=10, radius=r, depth=length, location=(x, y, r * .85))
    o = bpy.context.object
    o.rotation_euler = (math.pi / 2 + tilt, 0, yaw)
    apply(o, rot=True, scale=False)
    link(o, CRAG)
    bpy.ops.mesh.primitive_cylinder_add(vertices=10, radius=r * .62, depth=length * 1.02, location=(x, y, r * .85))
    o = bpy.context.object
    o.rotation_euler = (math.pi / 2 + tilt, 0, yaw)
    apply(o, rot=True, scale=False)
    link(o, DARK)


def torrione(c, r, ctrl, scarp=8.0, windows=(), moss=True):
    """A round tower broken high and low round its circle: where the wall has
    gone you see the hollow inside (dark inner face) and the stub of its stair."""
    x, y = c
    n = len(ctrl)
    cyl((x, y, 0), r * 1.25, scarp, STONE_D, 14, top=r * 1.03)
    cyl((x, y, 0), r * .9, min(ctrl) * .8, DARK, 14)                  # the hollow inside shows dark
    for i in range(n):
        tm = 2 * math.pi * (i + .5) / n
        l = 2 * r * math.sin(math.pi / n) * 1.06
        h = ctrl[i]
        heights = [max(1.5, h + RNG.uniform(-1.0, .8)) for _ in range(4)]
        rough_block(x + math.cos(tm) * r, y + math.sin(tm) * r, l, 2.5, heights, tm + math.pi / 2, STONE)
        if h < 6:
            rubble(x + math.cos(tm) * (r + 3.2), y + math.sin(tm) * (r + 3.2), 3.0, 3, big=2.2)
        if RNG.random() < .55 and h > 4:
            ivy(x + math.cos(tm) * (r + 1.1), y + math.sin(tm) * (r + 1.1), h * RNG.uniform(.35, .85), 1.7)
    for k in windows:                                                  # empty openings: you see the sky through them
        slit((x + math.cos(k) * r * 1.0, y + math.sin(k) * r * 1.0, 10.5), k, .9, 3.0, .8)
    if moss:
        scrub(x, y, min(ctrl) * .62, r * .5)


def rocca():
    global ROCK, _PEAK_EDGE, STONE, STONE_L, STONE_M, STONE_D, TUFA, IVY_P, CREN_P
    saved = (STONE, STONE_L, STONE_M, STONE_D, TUFA, IVY_P, CREN_P)
    _PEAK_EDGE = fractal_edge(RNG)
    STONE, STONE_L, STONE_M, STONE_D, TUFA = (.68, .60, .47), (.78, .70, .55), (.56, .49, .39), (.45, .39, .32), (.44, .42, .38)
    IVY_P, CREN_P = .5, .55
    T = WALL_T
    gate_w = 7.0
    # --- the curtain, mostly fallen; only a few reaches stand
    ruined_wall((-HALF, -HALF), (-HALF, HALF), [12, 11.6, 10.4, 8.0, 6.0, 8.5, 11.5, 12], T)                    # west: the best-preserved reach
    ruined_wall((-HALF, HALF), (HALF, HALF), [11, 9.0, 6.0, 3.6, 2.6, 3.4, 6.0, 9.0, 11, 11.5], T)                # north: a vast breach
    ruined_wall((-HALF, -HALF), (HALF, -HALF), [12, 11, 8.5, 5.0, 2.4, 3.0, 6.5, 10.0, 11.5, 11.5], T)             # south: collapsed
    ruined_wall((HALF, -HALF), (HALF, -gate_w / 2), [11, 9.5, 6.0, 8.0, 10.6], T)
    ruined_wall((HALF, gate_w / 2), (HALF, HALF), [11, 9.0, 5.0, 3.0, 3.6], T)
    # --- the gate: half the arch has fallen, the passage is choked with its stones
    R = gate_w / 2
    for k in range(11):
        if k > 6:                                                        # the arch's other half has fallen
            continue
        t = math.pi * (k + .5) / 11
        box((HALF + .1, -math.cos(t) * (R + .5), 4.6 + math.sin(t) * (R + .5)), (T + .4, 1.2, 1.7), CRAG_L if k % 2 else CRAG, -math.pi / 2 + t + math.pi / 2 * 0 + 0.0)
    box((HALF + .1, -(R + .5), 2.3), (T + .4, 1.2, 4.6), CRAG_L)
    box((HALF + .1, R + .5, 1.3), (T + .4, 1.2, 2.6), CRAG_L)          # the other jamb is a stump
    box((HALF - .2, 0, 3.2), (T * .8, gate_w - .4, 6.4), DARK)
    rubble(HALF + 1.5, 1.6, 3.4, 9, big=2.4)                             # the fallen voussoirs block the way
    rubble(HALF + 5.0, -1.0, 3.0, 6, big=1.6)
    scrub(HALF + 1.0, -1.0, 1.0, 1.4)
    # --- the torrioni: five towers, no two alike, all of them broken
    torrione((-HALF, -HALF), 7.2, [23, 22, 16, 8, 5, 7, 16, 22], 7.5, windows=(.6, 3.6))            # SW: a split drum, one side gone to the foundations
    torrione((-HALF, HALF), 6.8, [27, 26, 24, 19, 21, 25, 27, 27], 8.0, windows=(1.0, 2.6, 5.0))   # NW: the tallest, nearly whole, roofless
    torrione((HALF, HALF), 6.6, [4.0, 3.0, 2.0, 1.5, 2.4, 3.6, 4.4, 4.0], 4.0, moss=False)          # NE: reduced to a stump
    torrione((HALF + .4, -gate_w / 2 - 6.0), 5.6, [16, 12, 6, 4, 9, 15, 17, 17], 5.5, windows=(2.0,))  # the gate's flanking tower
    # the SE bastion: a pentagon without its crown, its east face sheared off
    cyl((HALF, -HALF, 0), 9.4, 5.5, CRAG_D, 5, top=7.2)
    cyl((HALF, -HALF, 5.4), 7.0, 3.0, CRAG, 5, top=6.8)
    for k in (0, 1, 4):
        t = 2 * math.pi * (k + .5) / 5 + math.pi / 10
        ghibelline(HALF + math.cos(t) * 6.2, -HALF + math.sin(t) * 6.2, 8.2, t + math.pi / 2, w=2.4, h=2.2)
    # a tower that fell: its shaft lies in pieces down the slope, outside the NE corner
    fallen_drum(HALF + 6, HALF + 6, 3.2, 7.5, .5)
    fallen_drum(HALF + 11, HALF + 1, 3.0, 6.0, 1.4, .1)
    fallen_drum(HALF - 4, HALF + 12, 2.6, 5.0, -.4)
    rubble(HALF + 5, HALF + 5, 9, 16, big=3.0)
    # --- the mastio: a square keep, broken stepwise, empty window holes, no floors left
    kx, ky, ks = -10.0, 3.0, 8.2
    ruined_wall((kx - ks, ky - ks), (kx + ks, ky - ks), [30, 30, 28, 23, 17, 9, 6], 2.6, scarp=False, window=True)
    ruined_wall((kx + ks, ky - ks), (kx + ks, ky + ks), [6, 5, 12, 20, 16, 9], 2.6, scarp=False, window=True)
    ruined_wall((kx + ks, ky + ks), (kx - ks, ky + ks), [9, 18, 27, 31, 30, 29, 27], 2.6, scarp=False, window=True)
    ruined_wall((kx - ks, ky + ks), (kx - ks, ky - ks), [27, 30, 31, 29, 30, 30], 2.6, scarp=False, window=True)
    frustum(kx, ky, 0, 4.2, (2 * ks + 7.0, 2 * ks + 7.0), (2 * ks + 1.4, 2 * ks + 1.4), 0.0, CRAG_D)
    rubble(kx + ks + 4, ky - 3, 8.5, 16, big=3.2)                      # what fell from its broken south-east corner
    scrub(kx, ky, 3.0, 3.5)                                             # the keep's floor is a thicket
    # --- the palazzo: one blind façade standing, windows empty, nothing behind it
    ruined_wall((-27.0, 22.0), (-7.0, 22.0), [10.5, 11.0, 9.0, 6.0, 3.0, 2.2], 2.2, scarp=False, crenel=False, window=True)
    # --- the arcade, two arches left and no beam above
    ax, ay = 14.0, -17.0
    for i in range(3):
        cyl((ax + i * 6.0, ay, 0), .95, 7.2 if i != 2 else 3.6, CRAG_L, 10)
    for i in range(1):
        for k in range(7):
            t = math.pi * (k + .5) / 7
            box((ax + i * 6.0 + 3.0 - math.cos(t) * 2.6, ay, 7.2 + math.sin(t) * 2.6), (1.0, 1.9, 1.3), CRAG if k % 2 else CRAG_L, -t + math.pi / 2)
    rubble(ax + 10, ay - 1, 3.5, 6, big=1.8)
    # --- the well, covered by scrub; column stumps; a dead tree; grass in the yard
    cyl((8.0, 10.0, 0), 1.9, 1.0, CRAG, 12)
    cyl((8.0, 10.0, 1.0), 1.3, .15, DARK, 12)
    scrub(8.0, 10.0, 1.0, 1.5)
    for (x, y, h) in ((-2.0, -14.0, 1.2), (3.0, -4.0, 2.0), (-4.0, 14.0, .8)):
        cyl((x, y, 0), .9, h, CRAG_L, 10)
    dead_tree(-18.0, -10.0, 0.0, 7.5)
    dead_tree(16.0, 12.0, 0.0, 5.0)
    rubble(0, 0, 20, 30, big=1.8)
    for _ in range(14):                                                # tufts of grass and scrub all over the floor
        a, rr = RNG.uniform(0, 6.28), RNG.uniform(3, 27)
        scrub(math.cos(a) * rr, math.sin(a) * rr, 0, RNG.uniform(.9, 1.8))
    # --- everything above stands on the crag: lift it onto a stepped mass of rock
    conform_to_peak()
    crag_mound()
    STONE, STONE_L, STONE_M, STONE_D, TUFA, IVY_P, CREN_P = saved


def fractal_edge(seed_rng, octaves=6, base_f=2, gain=.55):
    """A closed 1-D fractal profile r(theta) in [0, 1]: sines of doubling frequency
    and halving amplitude, so the outline wanders at every scale (bays, headlands,
    notches, teeth) instead of being a circle."""
    parts = [(base_f * 2 ** k + seed_rng.choice((0, 1)), gain ** k, seed_rng.uniform(0, 6.28)) for k in range(octaves)]
    total = sum(a for _, a, _ in parts)
    return lambda t: .5 + .5 * sum(a * math.sin(f * t + ph) for f, a, ph in parts) / total


PEAK = (-8.0, 0.0)        # the summit: under the keep, in the west half
ZP = 30.0                 # its height over the foot of the walls
_PEAK_EDGE = None


def peak_radius(t):
    """Where the rock meets the ground, by bearing from the summit: wide and gentle on the
    east (the way up), short and sheer on the west (the cliff side), fractal throughout."""
    west = .5 - .5 * math.cos(t)
    return (58.0 + 18.0 * _PEAK_EDGE(t)) * (1 - .2 * west)


def ground_h(x, y):
    """Height of the crag at (x, y): a summit with a convex shoulder, wandering
    ridges, stepped in beds of ~3.5 m (a bedded limestone peak), falling below
    ground level past its foot so the rim is always buried in the mountain."""
    dx, dy = x - PEAK[0], y - PEAK[1]
    s = math.hypot(dx, dy) / peak_radius(math.atan2(dy, dx))
    bump = 1.6 * math.sin(.11 * x + 1.0) * math.sin(.13 * y + 2.0) + 1.0 * math.sin(.27 * x + .6 * y) + .6 * math.sin(.6 * x - .5 * y + 1.0)
    if s >= 1.0:
        return max(-26.0, -(s - 1.0) * 150.0)
    h = ZP * (1 - s ** 2.2) + bump * (1 - s) * 2.2
    if h > 0:
        t = h / 3.5
        h = (math.floor(t) + (lambda f: f * f * (3 - 2 * f))(min(1, max(0, (t - math.floor(t) - .3) / .4)))) * 3.5
    return h


def conform_to_peak():
    """The masonry was authored on level ground at z = 0. Bake each object into world
    space and lift every vertex by the crag's height beneath it, so walls, towers and
    keep climb the slope; whatever stands on the ground (taller than 3 m) is also sunk
    3 m into the rock, so a wall's foot never hangs in the air over a step."""
    for o in list(current.objects):
        if o.get("rock"):
            continue
        me = o.data
        me.transform(o.matrix_world)
        o.matrix_world = Matrix.Identity(4)
        zs = [v.co.z for v in me.vertices]
        zmin, zmax = min(zs), max(zs)
        grounded = zmin < .05 and zmax - zmin > 3.0
        for v in me.vertices:
            v.co.z += ground_h(v.co.x, v.co.y) - (1.8 if grounded and v.co.z - zmin < .05 else 0.0) + (.15 if not grounded else 0.0)
        me.update()


def crag_mound():
    """The peak itself: a heightfield of ground_h (3 m cells), coloured by slope (pale on the
    bedded tops, grey on the faces, dark on the sheer), a rock stair that is just its
    east shoulder, ledge blocks and spires on the cliff side, and boulders in families (a
    big one, its children, scree) round the foot."""
    global ROCK
    ROCK = True
    step, ext = 3.0, 90.0
    n = int(2 * ext / step) + 1
    pts = {}

    def vid(i, j):
        if (i, j) not in pts:
            # a jittered grid: no two triangles alike, so the faces do not read as a lattice
            h1, h2 = math.sin(i * 12.9898 + j * 78.233) * 43758.5453, math.sin(i * 39.346 + j * 11.135) * 24634.6345
            x = -ext + i * step + ((h1 - math.floor(h1)) - .5) * step * .75
            y = -ext + j * step + ((h2 - math.floor(h2)) - .5) * step * .75
            pts[(i, j)] = (x, y, ground_h(x, y))
        return pts[(i, j)]
    tris = {0: [], 1: [], 2: []}
    order = []
    index = {}

    def vi(i, j):
        if (i, j) not in index:
            index[(i, j)] = len(order)
            order.append(vid(i, j))
        return index[(i, j)]
    for i in range(n - 1):
        for j in range(n - 1):
            quad = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)]
            zs = [vid(*q)[2] for q in quad]
            if max(zs) < -22.0:
                continue
            for tri in ((quad[0], quad[1], quad[2]), (quad[0], quad[2], quad[3])):
                a, b, c = (Vector(vid(*q)) for q in tri)
                nz = (b - a).cross(c - a).normalized().z
                cls = 2 if nz > .86 else (1 if nz > .55 else 0)
                tris[cls].append([vi(*q) for q in tri])
    for cls, colour in ((0, CRAG_D), (1, CRAG), (2, (.52, .49, .42))):
        if tris[cls]:
            surface(order, tris[cls], colour)

    def at(x, y):
        return ground_h(x, y)
    # ledge blocks and spires, thick on the cliff side (west), few on the gentle east
    for _ in range(90):
        t = RNG.uniform(0, 6.28)
        west = .5 - .5 * math.cos(t)
        if RNG.random() > .25 + .75 * west:
            continue
        sv = RNG.uniform(.55, 1.0)
        d = peak_radius(t) * sv
        x, y = PEAK[0] + math.cos(t) * d, PEAK[1] + math.sin(t) * d
        if abs(x) < 47 and abs(y) < 47 and sv < .8:                     # the castle's own ground: leave it
            continue
        crag_rock(x, y, at(x, y) - 2.2, RNG.uniform(2.6, 5.5), RNG.uniform(4, 9) * (.6 + west), RNG.choice((CRAG, CRAG_D, CRAG_L)), RNG.choice((6, 7, 8)), 2, lean=-math.cos(t) * .5)
    for i in range(8):
        t = math.pi * (.55 + 1.05 * i / 7) + RNG.uniform(-.12, .12)
        d = peak_radius(t) * RNG.uniform(.7, .88)
        x, y = PEAK[0] + math.cos(t) * d, PEAK[1] + math.sin(t) * d
        if abs(x) < 47 and abs(y) < 47:
            continue
        crag_rock(x, y, at(x, y) - 2.5, RNG.uniform(2.4, 3.8), RNG.uniform(8, 14), RNG.choice((CRAG, CRAG_L)), 6, 3, lean=-math.cos(t) * .8)

    def family(x, y, r, depth):
        crag_rock(x, y, at(x, y) - r * .6, r, r * RNG.uniform(.7, 1.6), RNG.choice((CRAG, CRAG_D, CRAG_L)), RNG.choice((6, 7, 8)), 2 if r > 2 else 1, lean=RNG.uniform(-.4, .4))
        if depth == 0 or r < 1.0:
            return
        for _ in range(RNG.choice((2, 3, 4))):
            a = RNG.uniform(0, 6.28)
            d = r * RNG.uniform(1.1, 2.2)
            family(x + math.cos(a) * d, y + math.sin(a) * d, r * RNG.uniform(.35, .55), depth - 1)
    for i in range(34):
        t = 2 * math.pi * (i + RNG.random()) / 34
        west = .5 - .5 * math.cos(t)
        if RNG.random() > .35 + .65 * west:
            continue
        d = peak_radius(t) * RNG.uniform(.88, 1.12)
        family(PEAK[0] + math.cos(t) * d, PEAK[1] + math.sin(t) * d, RNG.uniform(2.6, 5.5) * (.6 + .6 * west), 2)
    ROCK = False


# =========================================================================================
#  the coastal tower: a stone watchtower on the cliff, like the ones along the Ligurian coast
# =========================================================================================
def torre():
    cyl((0, 0, 0), 6.8, 6.6, STONE_D, 14, top=5.3)                       # the scarp
    cyl((0, 0, 6.5), 5.3, 11.5, STONE, 14, top=4.95)                      # the shaft
    for z, r in ((10.0, 5.25), (14.5, 5.05)):
        cyl((0, 0, z), r * 1.012, .4, STONE_L, 14)
    for k in range(14):                                                    # the corbels of the machicolation
        t = 2 * math.pi * k / 14
        box((math.cos(t) * 5.35, math.sin(t) * 5.35, 17.4), (1.0, .9, 1.5), STONE_M, t)
    cyl((0, 0, 17.9), 5.0, .9, STONE_M, 14, top=6.1)
    cyl((0, 0, 18.8), 6.1, 1.4, STONE, 14)                                 # the parapet
    cyl((0, 0, 20.1), 5.0, .2, STONE_D, 14)                                # the terrace
    for k in range(13):
        if k in (3, 4, 9):                                                  # the parapet is broken in places
            continue
        t = 2 * math.pi * (k + .5) / 13
        ghibelline(math.cos(t) * 5.7, math.sin(t) * 5.7, 20.2, t + math.pi / 2, w=2.2, t=1.1, h=2.3)
    rubble(4.0, 4.0, 3.0, 5, big=1.6)
    # a raised door with a stone stair, slits, a window
    box((5.0, 0, 8.2), (1.3, 2.0, 3.2), DARK)
    box((5.3, 0, 6.3), (1.5, 2.6, .4), STONE_L)
    for i in range(6):
        box((5.7 + i * .8, 0, 1.1 + (5 - i) * .25 + .1), (1.0, 2.2, .5 + (5 - i) * .5), STONE_M)
    for k, z in enumerate((11.0, 14.0, 16.5)):
        t = k * 2.1 + .9
        slit((math.cos(t) * 5.1, math.sin(t) * 5.1, z), t, .5, 2.0, .6)
    arch_window((0, 5.1, 12.5), math.pi / 2, 1.2, 2.2)
    ivy(-3.8, 3.0, 5.0, 2.0)
    ivy(-4.6, 1.0, 9.0, 1.4)
    ivy(-3.0, 4.2, 14.0, 1.2)
    for i in range(10):                                                    # the rock it stands on
        t = 2 * math.pi * i / 10 + .3
        r = 7.6 + RNG.uniform(-.5, 1.4)
        bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=1, location=(math.cos(t) * r, math.sin(t) * r, .5))
        o = bpy.context.object
        sz = RNG.uniform(1.8, 3.2)
        o.scale = (sz * 1.2, sz, sz * .6)
        o.rotation_euler = (0, 0, RNG.uniform(0, 3.14))
        apply(o, rot=True)
        link(o, RNG.choice((STONE_D, TUFA, STONE_M)))


# =========================================================================================
#  the sailing boats (a 9.6 m sloop: bow +X)
# =========================================================================================
STATIONS = [(-4.4, .95), (-2.6, 1.45), (.3, 1.6), (2.9, 1.15), (4.9, .35), (5.5, .02)]
DECK_Z, KEEL_Z, WL_Z = .75, -.6, .1


def hull(topsides, stripe=None):
    pts = []                                      # per station: left deck edge, left waterline, keel, right waterline, right deck edge
    for x, w in STATIONS:
        pts.append([(x, -w, DECK_Z), (x, -w * .93, WL_Z), (x, 0, KEEL_Z + (.12 if x > 3 else 0)), (x, w * .93, WL_Z), (x, w, DECK_Z)])
    verts = [v for row in pts for v in row]
    c = (.5, 0, .2)
    top, under, deck = [], [], []
    for i in range(len(pts) - 1):
        a, b = i * 5, (i + 1) * 5
        top += [(a, b, b + 1, a + 1), (a + 3, b + 3, b + 4, a + 4)]
        under += [(a + 1, b + 1, b + 2, a + 2), (a + 2, b + 2, b + 3, a + 3)]
        deck += [(a, a + 4, b + 4, b)]
    surface(verts, top, topsides, centre=c)
    surface(verts, under, stripe or ANTIFOUL, centre=c)
    surface(verts, deck, DECK)
    # the transom and the rail
    surface([(-4.4, -.95, DECK_Z), (-4.4, .95, DECK_Z), (-4.4, .88, WL_Z), (-4.4, -.88, WL_Z)], [(0, 1, 2, 3)], topsides, centre=(0, 0, .3))
    box((-4.35, 0, DECK_Z + .02), (.1, 1.9, .06), WOOD)
    # the rubbing strake
    for sg in (-1, 1):
        for (x0, w0), (x1, w1) in zip(STATIONS[:-1], STATIONS[1:]):
            rod((x0, sg * w0, DECK_Z - .08), (x1, sg * w1, DECK_Z - .08), .05, WOOD, 4)


def rig(sail=None, jib=None, furled=False):
    box((-.9, 0, DECK_Z + .45), (2.8, 1.25, .9), (.93, .91, .86))             # cabin
    box((-.9, 0, DECK_Z + .92), (3.0, 1.35, .12), WOOD)
    for sg in (-1, 1):
        box((-.9, sg * .66, DECK_Z + .52), (1.6, .05, .34), GLASS)
    box((-3.0, 0, DECK_Z + .15), (1.6, 1.1, .3), (.30, .22, .14))               # the cockpit well
    rod((.9, 0, DECK_Z), (.9, 0, 11.6), .09, METAL)                              # the mast
    rod((.9, 0, 6.3), (.9, 1.1, 6.0), .04, METAL)
    rod((.9, 0, 6.3), (.9, -1.1, 6.0), .04, METAL)                               # spreaders
    rod((.9, 0, 11.6), (5.2, 0, DECK_Z + .3), .03, METAL, 4)                     # forestay
    rod((.9, 0, 11.6), (-4.4, 0, DECK_Z + .3), .03, METAL, 4)                    # backstay
    rod((.8, 0, 1.55), (-3.9, 0, 1.75), .08, METAL)                              # the boom
    rod((5.2, 0, DECK_Z + .2), (5.8, 0, DECK_Z + .5), .06, WOOD)                 # the bowsprit
    rod((-4.6, 0, -.5), (-4.6, 0, .9), .06, WOOD_D, 4)                           # the rudder post
    if furled:
        rod((.7, 0, 1.75), (-3.8, 0, 1.85), .17, (.88, .86, .80), 8)             # sail rolled on the boom
        return
    # the mainsail: a triangle with belly (rows of the luff-foot grid, leech curved)
    luff_top, foot_back = (.78, 0, 10.9), (-3.9, 0, 1.75)
    luff_bot = (.78, 0, 1.75)
    rows, cols = 6, 5
    verts = []
    for i in range(rows + 1):
        t = i / rows
        top = Vector(luff_top).lerp(Vector(luff_bot), t)
        back = Vector(luff_top).lerp(Vector(foot_back), t)
        back.x -= math.sin(math.pi * t) * .35 * (1 - t)                           # the roach
        for j in range(cols + 1):
            u = j / cols
            p = top.lerp(back, u)
            p.y = .75 * math.sin(math.pi * u) * math.sin(math.pi * min(1.0, t * .9 + .1)) * (1 - t * .4)
            verts.append(tuple(p))
    faces = []
    for i in range(rows):
        for j in range(cols):
            a = i * (cols + 1) + j
            faces.append((a, a + 1, a + cols + 2, a + cols + 1))
    surface(verts, faces, sail, both=True)
    for k in range(1, 3):                                                         # battens
        z = 1.75 + k * 2.6
        rod((.8, .3, z + .1), (-.2 - 4.2 * (1 - (z - 1.75) / 9.2), .3, z), .035, WHITE, 4)
    # the jib
    top, tack, clew = (.95, 0, 9.8), (5.1, 0, DECK_Z + .45), (.9, 0, 1.3)
    verts, faces = [], []
    rows, cols = 5, 4
    for i in range(rows + 1):
        t = i / rows
        luff = Vector(top).lerp(Vector(tack), t)
        leech = Vector(top).lerp(Vector(clew), t)
        for j in range(cols + 1):
            u = j / cols
            p = luff.lerp(leech, u)
            p.y = .55 * math.sin(math.pi * u) * math.sin(math.pi * min(1.0, t * .9 + .1))
            verts.append(tuple(p))
    for i in range(rows):
        for j in range(cols):
            a = i * (cols + 1) + j
            faces.append((a, a + 1, a + cols + 2, a + cols + 1))
    surface(verts, faces, jib or (.97, .96, .93), both=True)


def sloop(hull_c, sail_c, jib_c=None, furled=False, stripe=None):
    hull(hull_c, stripe)
    rig(sail=sail_c, jib=jib_c, furled=furled)
    box((4.9, 0, DECK_Z + .08), (.5, .05, .1), RED if not furled else METAL)     # the navigation light


BUILDERS = {
    "rocca": (rocca, (62.0, 62.0)),
    "torre_pietra": (torre, (14.0, 14.0)),
    "veliero_bianco": (lambda: sloop(WHITE, WHITE), None),
    "veliero_crema": (lambda: sloop(CREAM, (.95, .86, .66), stripe=NAVY), None),
    "veliero_rosso": (lambda: sloop(NAVY, RED, (.96, .95, .92)), None),
    "veliero_blu": (lambda: sloop(WHITE, BLUE, (.96, .95, .92), stripe=NAVY), None),
    "veliero_giallo": (lambda: sloop(GREEN, YELLOW, (.96, .95, .92)), None),
    "ormeggiato_bianco": (lambda: sloop(WHITE, None, furled=True), None),
    "ormeggiato_blu": (lambda: sloop(NAVY, None, furled=True, stripe=(.55, .55, .56)), None),
    "ormeggiato_crema": (lambda: sloop(CREAM, None, furled=True, stripe=GREEN), None),
}

baked = {}
for name, (build, footprint) in BUILDERS.items():
    current = bpy.data.collections.new(name)
    bpy.context.scene.collection.children.link(current)
    build()
    bpy.context.view_layer.update()
    parts = {False: ([], [], [], []), True: ([], [], [], [])}
    deps = bpy.context.evaluated_depsgraph_get()
    for o in current.objects:
        positions, normals, colors, indices = parts[bool(o.get("rock"))]
        eo = o.evaluated_get(deps)
        mesh = eo.to_mesh()
        mesh.calc_loop_triangles()
        color = o.data.materials[0].diffuse_color[:]
        mw = o.matrix_world
        nm = mw.to_3x3().inverted().transposed()
        for tri in mesh.loop_triangles:
            if tri.area < 1e-7:
                continue
            n = (nm @ tri.normal).normalized()
            for vi in tri.vertices:
                p = mw @ mesh.vertices[vi].co
                positions.append([round(p.x, 4), round(p.z, 4), round(-p.y, 4)])
                normals.append([round(n.x, 4), round(n.z, 4), round(-n.y, 4)])
                colors.append([round(c, 4) for c in color[:3]])
                indices.append(len(indices))
        eo.to_mesh_clear()
    for is_rock, (positions, normals, colors, indices) in parts.items():
        if is_rock and not indices:
            continue
        key = name + "_roccia" if is_rock else name
        baked[key] = dict(positions=positions, normals=normals, colors=colors, indices=indices,
                          footprint=None if is_rock or not footprint else list(footprint))
        print(f"{key}: {len(indices) // 3} triangles")

(OUT / "island.json").write_text(json.dumps(baked, separators=(",", ":")) + "\n")

# --- contact sheet -------------------------------------------------------------------
layout = {"rocca": (40, 0, 1.0), "torre_pietra": (118, 8, 1.5)}
boats = [n for n in BUILDERS if n.startswith(("veliero", "ormeggiato"))]
for i, n in enumerate(boats):
    layout[n] = (150 + (i % 4) * 27, -26 + (i // 4) * 22, 2.2)
for name, (x, y, k) in layout.items():
    for o in bpy.data.collections[name].objects:
        o.matrix_world = Matrix.Translation((x, y, 0)) @ Matrix.Scale(k, 4) @ o.matrix_world
current = bpy.data.collections.new("studio")
bpy.context.scene.collection.children.link(current)
box((115, -5, -.4), (400, 150, .6), (.55, .72, .84))
bpy.ops.object.light_add(type="SUN", location=(0, 0, 80))
bpy.context.object.data.energy = 4.5
bpy.context.object.rotation_euler = (math.radians(45), math.radians(10), math.radians(-40))
bpy.ops.object.camera_add(location=(115, -230, 150))
cam = bpy.context.object
cam.rotation_euler = (Vector((115, -3, 14)) - cam.location).to_track_quat("-Z", "Y").to_euler()
cam.data.type = "ORTHO"
cam.data.ortho_scale = 255
scene = bpy.context.scene
scene.camera = cam
scene.world = bpy.data.worlds.new("w")
scene.world.color = (.62, .68, .74)
scene.render.engine = "BLENDER_EEVEE" if "BLENDER_EEVEE" in [e.identifier for e in bpy.types.RenderSettings.bl_rna.properties["engine"].enum_items] else "BLENDER_EEVEE_NEXT"
scene.render.resolution_x, scene.render.resolution_y = 2200, 1100
scene.render.filepath = str(OUT / "island_preview.png")
bpy.ops.render.render(write_still=True)

# a 3/4 close-up of the rocca alone
for name in layout:
    if name != "rocca":
        for o in bpy.data.collections[name].objects:
            o.hide_render = True
cam.data.type = "PERSP"
cam.data.lens = 35
scene.render.resolution_x, scene.render.resolution_y = 1600, 1000
for tag, (dx, dy, dz) in {"": (-120, -190, 110), "_gate": (140, -10, 50), "_west": (-125, 70, 38)}.items():
    cam.location = (40 + dx, dy, dz)
    cam.rotation_euler = (Vector((40, 0, 18)) - cam.location).to_track_quat("-Z", "Y").to_euler()
    scene.render.filepath = str(OUT / f"rocca_preview{tag}.png")
    bpy.ops.render.render(write_still=True)
bpy.ops.wm.save_as_mainfile(filepath=str(OUT / "island_assets.blend"))
