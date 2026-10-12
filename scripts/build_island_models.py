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


def link(o, color):
    for c in list(o.users_collection):
        c.objects.unlink(o)
    current.objects.link(o)
    o.data.materials.clear()
    o.data.materials.append(mat(color))
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
#  the rocca: an Italian hill fortress, ruined
# =========================================================================================
import random
RNG = random.Random(11)
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


def ghibelline(cx, cy, z, yaw, w=2.0, t=1.1, h=2.4, color=STONE_L):
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


def rubble(cx, cy, radius, count, color=STONE_M, big=2.0):
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


def ruined_wall(a, b, ctrl, T, color=STONE, scarp=True, crenel=True, step=5.0, window=False):
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
        if crenel and h0 >= .93 * top_ref and h1 >= .93 * top_ref:
            for k in (.28, .72):
                q = p0 + (p1 - p0) * k + nrm * (T / 2 - .55)
                ghibelline(q.x, q.y, (h0 + h1) / 2 + .1, yaw)
        if low < .5 * top_ref:
            rubble(mid.x + nrm.x * 2.0, mid.y + nrm.y * 2.0, 3.2, 4)
            rubble(mid.x - nrm.x * 2.2, mid.y - nrm.y * 2.2, 2.6, 2)
        if RNG.random() < .22 and low > 3:
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


def rocca():
    gate_w = 7.0
    T = WALL_T
    # --- the curtain: each side broken in its own way (east is the gate side)
    ruined_wall((-HALF, -HALF), (-HALF, HALF), [11, 11, 10.6, 9.5, 8.8, 10.4, 11, 11], T)          # west: nearly whole
    ruined_wall((-HALF, HALF), (HALF, HALF), [10.5, 9.2, 7.0, 3.6, 2.2, 2.6, 5.0, 8.4, 10.0, 11], T)  # north: a great breach
    ruined_wall((-HALF, -HALF), (HALF, -HALF), [11, 11, 10.2, 8.0, 5.0, 2.8, 2.0, 4.6, 9.0, 11], T)   # south: another
    ruined_wall((HALF, -HALF), (HALF, -gate_w / 2), [11, 10.0, 7.5, 9.5, 11], T)
    ruined_wall((HALF, gate_w / 2), (HALF, HALF), [11, 11, 6.0, 3.4, 4.2], T)
    # --- the round-arched gate: voussoirs round the opening, a dark passage, jambs
    R = gate_w / 2
    for k in range(11):
        t = math.pi * (k + .5) / 11
        box((HALF + .1, -math.cos(t) * (R + .5), 4.6 + math.sin(t) * (R + .5)), (T + .4, 1.2, 1.7), STONE_L if k % 2 else STONE, -math.pi / 2 + t + math.pi / 2 * 0 + 0.0)
    for sg in (-1, 1):
        box((HALF + .1, sg * (R + .5), 2.3), (T + .4, 1.2, 4.6), STONE_L)
    box((HALF - .2, 0, 3.2), (T * .8, gate_w - .4, 6.4), DARK)
    cyl((HALF - .3, 0, 0), 0.1, .1, DARK)
    # the piece of wall left over the arch, and one of the flanking towers
    rough_block(HALF, 0, T, gate_w + 3.4, [10.8, 10.0, 11.4, 10.6], 0.0, STONE, z0=7.6)
    broken_round_tower((HALF + .4, -gate_w / 2 - 5.5), 5.6, [17, 15, 11, 8, 12, 16, 18, 18], 5.0)
    # --- the corners: a half tower, a whole one, a stump, a pentagonal bastion on a scarp
    broken_round_tower((-HALF, -HALF), 6.8, [20, 19, 15, 9, 6, 8, 14, 19], 7.0)
    broken_round_tower((-HALF, HALF), 6.8, [21, 21, 20, 19, 19, 20, 21, 21], 7.0)
    broken_round_tower((HALF, HALF), 6.6, [5.5, 4.0, 3.0, 2.2, 3.4, 4.8, 6.0, 5.4], 4.0)
    rubble(HALF + 3, HALF + 3, 6, 10, big=2.8)
    cyl((HALF, -HALF, 0), 9.2, 5.5, STONE_D, 5, top=7.0)                                           # the bastion, on a scarp
    cyl((HALF, -HALF, 5.4), 7.0, 5.0, STONE, 5, top=6.6)
    for k in range(5):
        t = 2 * math.pi * (k + .5) / 5 + math.pi / 10
        ghibelline(HALF + math.cos(t) * 6.2, -HALF + math.sin(t) * 6.2, 10.4, t + math.pi / 2, w=2.4, h=2.4)
    # --- the mastio: a square keep, whole on two sides, stepped down on the others
    kx, ky, ks = -10.0, 3.0, 8.2
    ruined_wall((kx - ks, ky - ks), (kx + ks, ky - ks), [31, 31, 31, 26, 22, 18, 16], 2.6, scarp=False, window=True)
    ruined_wall((kx + ks, ky - ks), (kx + ks, ky + ks), [16, 13, 21, 27, 24, 17], 2.6, scarp=False, window=True)
    ruined_wall((kx + ks, ky + ks), (kx - ks, ky + ks), [17, 24, 29, 31, 31, 30, 29], 2.6, scarp=False, window=True)
    ruined_wall((kx - ks, ky + ks), (kx - ks, ky - ks), [29, 31, 31, 30, 31, 31], 2.6, scarp=False, window=True)
    frustum(kx, ky, 0, 4.2, (2 * ks + 7.0, 2 * ks + 7.0), (2 * ks + 1.4, 2 * ks + 1.4), 0.0, STONE_D)
    for z in (11.0, 19.0, 26.0):                                                                   # the stubs of the floor beams
        for k in range(4):
            box((kx - ks - .6, ky - 5 + k * 3.4, z), (1.7, .4, .45), WOOD_D)
    rubble(kx + ks + 4, ky + 2, 7.5, 12, big=3.0)
    # --- the palazzo's façade left standing against the north wall, three arched windows
    ruined_wall((-27.0, 22.0), (-7.0, 22.0), [10.5, 11.0, 10.5, 8.5, 5.0, 3.0], 2.2, scarp=False, crenel=False, window=True)
    # --- an arcade (loggia) of three arches, still standing
    ax, ay = 14.0, -17.0
    for i in range(4):
        cyl((ax + i * 6.0, ay, 0), .95, 7.2, STONE_L, 10)
        box((ax + i * 6.0, ay, 7.5), (2.4, 2.2, .7), STONE_L)
    for i in range(3):
        for k in range(7):
            t = math.pi * (k + .5) / 7
            box((ax + i * 6.0 + 3.0 - math.cos(t) * 2.6, ay, 7.2 + math.sin(t) * 2.6), (1.0, 1.9, 1.3), STONE if k % 2 else STONE_L, -t + math.pi / 2)
    rough_block(ax + 9.0, ay, 18.0, 1.8, [9.8, 10.4, 10.4, 9.6], 0.0, STONE, z0=9.8 - .001)
    # --- a well, column stumps, loose stones
    cyl((8.0, 10.0, 0), 1.9, 1.3, STONE_M, 12)
    cyl((8.0, 10.0, 1.3), 1.4, .2, DARK, 12)
    for (x, y, h) in ((-2.0, -14.0, 1.6), (3.0, -4.0, 2.4), (-4.0, 14.0, 1.0)):
        cyl((x, y, 0), .9, h, STONE_L, 10)
    rubble(0, 0, 18, 22, big=1.6)
    # --- a cap of rock round the outside, so the walls stand on a crag
    for k in range(22):
        t = 2 * math.pi * k / 22 + RNG.uniform(-.1, .1)
        r = HALF * 1.34 + RNG.uniform(-1.5, 3.0)
        sz = RNG.uniform(3.0, 6.0)
        bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=1, location=(math.cos(t) * r, math.sin(t) * r, sz * .15))
        o = bpy.context.object
        o.scale = (sz * 1.3, sz, sz * .6)
        o.rotation_euler = (0, 0, RNG.uniform(0, 3.14))
        apply(o, rot=True)
        link(o, RNG.choice((STONE_D, TUFA, STONE_M)))
    # the courtyard floor
    cyl((0, 0, 0), HALF * 1.38, .3, TUFA, 4)
    current.objects[-1].rotation_euler = (0, 0, math.pi / 4)
    apply(current.objects[-1])


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
    positions, normals, colors, indices = [], [], [], []
    deps = bpy.context.evaluated_depsgraph_get()
    for o in current.objects:
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
    baked[name] = dict(positions=positions, normals=normals, colors=colors, indices=indices,
                       footprint=list(footprint) if footprint else None)
    print(f"{name}: {len(indices) // 3} triangles")

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
bpy.ops.wm.save_as_mainfile(filepath=str(OUT / "island_assets.blend"))
