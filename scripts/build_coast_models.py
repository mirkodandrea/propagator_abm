"""Run with:
    /Applications/Blender.app/Contents/MacOS/Blender --background --python scripts/build_coast_models.py

The coast's own pieces: boulders for the foot of the cliffs, scree clusters,
sea stacks (faraglioni) and awash reefs. Authored in metres, origin at the
centre of the base, flat-bottomed so they can be sunk a little into the ground.
Baked to `assets/models/coast.json` in the same layout as `island.json`
(positions, normals, flat colours, indices; Blender Z-up -> runtime Y-up), and
placed along the shore by `crates/game/src/coast.rs`.
"""
import bpy
import bmesh
import json
import random
from pathlib import Path
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets/models"
bpy.ops.wm.read_factory_settings(use_empty=True)

materials = {}


def lin(c):
    return ((c + .055) / 1.055) ** 2.4 if c > .04045 else c / 12.92


def mat(color):
    color = tuple(lin(c) for c in color)
    key = tuple(round(c, 4) for c in color)
    if key not in materials:
        m = bpy.data.materials.new(f"c{len(materials)}")
        m.diffuse_color = (*color, 1)
        materials[key] = m
    return materials[key]


# limestone of the Ligurian coast, as in `terrain_mesh::coast_rock`
PALE = (.80, .76, .66)
OCHRE = (.70, .57, .40)
GREY = (.55, .54, .51)
IRON = (.36, .30, .26)
WET = (.27, .26, .25)
SALT = (.88, .86, .80)

current = None


def link(o, color):
    for c in list(o.users_collection):
        c.objects.unlink(o)
    current.objects.link(o)
    o.data.materials.clear()
    o.data.materials.append(mat(color))
    return o


def blob(centre, radii, color, seed, rough=.28, flat_base=True, yaw=0.0):
    """A faceted rock: a coarse icosphere, each vertex pushed in or out, the
    underside flattened so it sits on the ground."""
    rng = random.Random(seed)
    bm = bmesh.new()
    bmesh.ops.create_icosphere(bm, subdivisions=1, radius=1.0)
    for v in bm.verts:
        k = 1.0 + (rng.random() - .5) * 2 * rough
        v.co = Vector((v.co.x * radii[0] * k, v.co.y * radii[1] * k, v.co.z * radii[2] * k))
        if flat_base and v.co.z < 0:
            v.co.z = 0.0
    me = bpy.data.meshes.new("rock")
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new("rock", me)
    o.location = centre
    o.rotation_euler = (0, 0, yaw)
    bpy.context.scene.collection.objects.link(o)
    bpy.context.view_layer.objects.active = o
    o.select_set(True)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    for p in me.polygons:
        p.use_smooth = False
    return link(o, color)


def rock_a():
    blob((0, 0, 0), (1.7, 1.3, 1.1), GREY, 1)
    blob((.4, -.2, .8), (.8, .7, .5), PALE, 2)


def rock_b():
    blob((0, 0, 0), (1.1, 1.0, 1.8), OCHRE, 3, rough=.22)
    blob((-.2, .1, 1.2), (.7, .6, .7), PALE, 4)


def rock_c():
    blob((0, 0, 0), (2.4, 1.6, .7), PALE, 5, rough=.2)
    blob((-.6, .3, .3), (1.0, .8, .55), GREY, 6)


def scree():
    """A heap of fallen blocks at the foot of a face, dark where the spray reaches."""
    blob((0, 0, -.1), (3.4, 2.8, .6), WET, 7, rough=.15)
    rng = random.Random(8)
    for i in range(6):
        r = .7 + rng.random() * 1.1
        blob((rng.uniform(-2, 2), rng.uniform(-1.6, 1.6), 0), (r, r * .85, r * (.6 + rng.random() * .5)),
             [GREY, PALE, OCHRE, GREY, IRON, PALE][i], 20 + i, yaw=rng.random() * 6.28)


def column(base, height, radius, lean, color, seed, rings=16, segs=14, crown=.55):
    """A tapering rock column: rings of noisy radius, leaning, with a ragged
    flat-ish crown. Z up from `base` (metres), flat-bottomed at z=base."""
    import math
    rng = random.Random(seed)
    bm = bmesh.new()
    grid = []
    phase = [rng.random() * 6.28 for _ in range(3)]
    for i in range(rings + 1):
        t = i / rings
        # fat at the foot, waisted, then a blunt crown
        prof = (1 - t) ** .55 * (1 + .18 * math.sin(t * 7 + phase[0])) * (1 - .35 * t * t)
        r = radius * max(prof, crown * .12)
        z = base + height * t
        cx, cy = lean[0] * t ** 1.4 * height, lean[1] * t ** 1.4 * height
        ring = []
        for k in range(segs):
            a = k / segs * 6.2832
            n = 1 + .20 * (rng.random() - .5) + .10 * math.sin(a * 3 + phase[1] + t * 5) + .06 * math.sin(a * 5 + phase[2])
            rz = z + (rng.random() - .5) * height * .02 * (i == rings)
            ring.append(bm.verts.new((cx + math.cos(a) * r * n * 1.0, cy + math.sin(a) * r * n * .86, rz)))
        grid.append(ring)
    for i in range(rings):
        for k in range(segs):
            k2 = (k + 1) % segs
            bm.faces.new((grid[i][k], grid[i][k2], grid[i + 1][k2], grid[i + 1][k]))
    top = bm.verts.new((grid[-1][0].co.x * 0 + lean[0] * height, lean[1] * height, base + height * 1.02))
    for k in range(segs):
        bm.faces.new((grid[-1][k], grid[-1][(k + 1) % segs], top))
    bm.faces.new(list(reversed(grid[0])))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new("col")
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new("col", me)
    bpy.context.scene.collection.objects.link(o)
    link(o, color)
    return o


def sea_stack():
    """A faraglione: one tapering column of pale limestone with a lesser spire
    against it and a wet foot. Strata are tinted at placement by height."""
    blob((0, 0, -.5), (4.0, 3.4, 1.2), WET, 30, rough=.12)
    column(-.6, 12.5, 3.0, (.05, .02), PALE, 31)
    column(-.6, 6.5, 1.6, (-.06, .03), PALE, 32, rings=10, segs=10).location = (2.6, 1.0, 0)


def reef():
    """Rock just awash: a dark flat shelf with a salt-white crown."""
    blob((0, 0, -.3), (3.4, 2.4, .75), WET, 40, rough=.2)
    blob((.5, .2, .1), (1.5, 1.1, .5), SALT, 41, rough=.22)


BUILDERS = {"rock_a": rock_a, "rock_b": rock_b, "rock_c": rock_c, "scree": scree, "sea_stack": sea_stack, "reef": reef}

baked = {}
for name, build in BUILDERS.items():
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
    baked[name] = dict(positions=positions, normals=normals, colors=colors, indices=indices, footprint=None)
    print(f"{name}: {len(indices) // 3} triangles")

(OUT / "coast.json").write_text(json.dumps(baked, separators=(",", ":")) + "\n")
