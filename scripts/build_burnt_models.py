"""Run with Blender --background --python scripts/build_burnt_models.py.
Burnt vegetation: three variants per species (pine, chestnut, bush, olive), in
the same model space as the live plants of build_models.py so the game scales
them with the same vector. Writes assets/models/burnt.json, burnt_assets.blend
and burnt_preview.png.
Blender: metres, Z up, front -Y. Runtime: metres, Y up, front +Z.
Colours are baked as sRGB (the game converts them) and are absolute: char,
ash and scorched foliage do not depend on the species tint.
"""
import bpy
import json
import math
from pathlib import Path
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'assets/models'
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)

CHAR = (.045, .038, .034)      # blackened wood
CHAR2 = (.11, .085, .07)       # the lighter, cracked side of a charred trunk
EMBER = (.20, .09, .05)        # a trunk still brown-red under the soot
ASH = (.45, .44, .41)          # pale ash on the ground
ASH_DARK = (.24, .23, .22)
SCORCH = (.31, .17, .07)       # needles and leaves browned by the heat
SCORCH2 = (.40, .25, .10)
SCORCH3 = (.22, .12, .06)
current = None
mats = {}

def paint(o, color):
    key = tuple(round(c, 3) for c in color)
    if key not in mats:
        m = bpy.data.materials.new('Burnt %s' % (key,))
        m.use_nodes = True
        # Blender's own colour is linear; the bake keeps the sRGB triple
        lin = [c ** 2.2 for c in color]
        m.diffuse_color = (*lin, 1)
        m.node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (*lin, 1)
        m.node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = .9
        m['srgb'] = list(color)
        mats[key] = m
    o.data.materials.append(mats[key])

def finish(o, name, color):
    o.name = name
    for c in list(o.users_collection):
        c.objects.unlink(o)
    current.objects.link(o)
    paint(o, color)
    return o

def ico(name, pos, size, color, squash=None, rot=0.0, sub=1):
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=sub, radius=1, location=pos)
    o = bpy.context.object
    o.scale = size
    o.rotation_euler = (0, 0, rot)
    return finish(o, name, color)

def rod(name, a, b, radius, color, sides=5, top=None):
    a, b = Vector(a), Vector(b)
    bpy.ops.mesh.primitive_cone_add(vertices=sides, radius1=radius, radius2=radius if top is None else top, depth=(b - a).length, location=(a + b) / 2)
    o = bpy.context.object
    o.rotation_euler = (b - a).to_track_quat('Z', 'Y').to_euler()
    return finish(o, name, color)

def jagged(name, pos, radius, height, color, sides=5):
    """A snapped trunk top: a short cone, so the break reads as splinters."""
    return rod(name, pos, (pos[0], pos[1], pos[2] + height), radius, color, sides, top=radius * .15)

def disc(name, pos, rx, ry, h, color, rot=0.0):
    bpy.ops.mesh.primitive_cylinder_add(vertices=7, radius=1, depth=h, location=(pos[0], pos[1], pos[2] + h / 2))
    o = bpy.context.object
    o.scale = (rx, ry, 1)
    o.rotation_euler = (0, 0, rot)
    return finish(o, name, color)

def twigs(prefix, root, n, reach, rise, thick, color, phase=0.0, jitter=0.0):
    for i in range(n):
        a = phase + i * math.tau / n
        j = 1 + jitter * math.sin(i * 2.7)
        end = (root[0] + math.cos(a) * reach * j, root[1] + math.sin(a) * reach * j, root[2] + rise * (1 + .25 * math.cos(i * 1.9)))
        rod(f'{prefix} twig', root, end, thick, color, 4, top=thick * .25)

# --- pine (live: trunk to .80, flat umbrella of pads at .80-.93) --------------
def pine_a():
    # standing snag: bare charred trunk, the branch stubs left on it
    rod('Snag trunk', (0, 0, 0), (.07, .02, .78), .040, CHAR, 6, top=.018)
    for z, a, ln in [(.50, 0.4, .13), (.60, 2.6, .17), (.68, 4.4, .12), (.74, 1.4, .09)]:
        b = (.05 + math.cos(a) * ln, .02 + math.sin(a) * ln, z + .08)
        rod('Branch stub', (.05, .015, z), b, .013, CHAR, 4, top=.005)

def pine_b():
    # scorched, not consumed: the umbrella stays up, needles gone rust-brown
    rod('Scorched trunk', (0, 0, 0), (.07, .02, .80), .038, CHAR, 6, top=.02)
    rod('Pine fork', (.05, .015, .62), (-.13, .09, .84), .016, CHAR, 4, top=.008)
    for pos, size, c in [((.08, .02, .84), (.30, .26, .06), SCORCH), ((.23, .10, .78), (.16, .14, .05), SCORCH2),
                         ((-.12, .10, .88), (.18, .15, .05), SCORCH3), ((.04, -.16, .90), (.13, .11, .04), SCORCH)]:
        ico('Browned pad', pos, size, c)

def pine_c():
    # broken: snapped halfway, the crown down on the ground beside it
    rod('Broken trunk', (0, 0, 0), (.03, .01, .40), .042, CHAR, 6, top=.032)
    jagged('Splinters', (.03, .01, .40), .032, .09, CHAR2, 5)
    rod('Fallen crown', (.10, -.02, .03), (.52, -.16, .05), .028, CHAR, 5, top=.012)
    rod('Fallen fork', (.30, -.09, .04), (.40, .08, .04), .012, CHAR, 4, top=.006)
    ico('Burnt litter', (.28, -.06, .02), (.20, .13, .03), ASH_DARK)

# --- chestnut (live: trunk .34, crown centred at .66, lobes to .98) -------------
def chestnut_a():
    rod('Charred trunk', (0, 0, 0), (0, 0, .36), .078, CHAR, 6, top=.052)
    for i in range(3):
        a = i * 2.1
        mid = (math.cos(a) * .12, math.sin(a) * .12, .52)
        rod('Charred bough', (0, 0, .30), mid, .032, CHAR, 4, top=.02)
        for k, da in enumerate((-.7, .7)):
            end = (mid[0] + math.cos(a + da) * .18, mid[1] + math.sin(a + da) * .18, .78 + .06 * k)
            rod('Charred limb', mid, end, .018, CHAR2 if k else CHAR, 4, top=.005)
    ico('Ash at the foot', (0, 0, .01), (.26, .24, .025), ASH_DARK)

def chestnut_b():
    # a thinned, browned crown over a blackened trunk
    rod('Scorched trunk', (0, 0, 0), (0, 0, .36), .075, CHAR, 6, top=.05)
    for i in range(3):
        a = i * 2.1
        rod('Bough', (0, 0, .28), (math.cos(a) * .22, math.sin(a) * .22, .52), .03, CHAR, 4, top=.012)
    ico('Browned crown', (0, 0, .64), (.36, .34, .22), SCORCH)
    for i, c in enumerate((SCORCH2, SCORCH3, SCORCH2)):
        a = i * 2.1 + .5
        ico('Browned lobe', (math.cos(a) * .30, math.sin(a) * .28, .58 + .09 * i), (.20, .19, .15), c)

def chestnut_c():
    # felled by the fire: a hollow stump, the limb lying across the ash
    rod('Hollow stump', (0, 0, 0), (0, 0, .22), .088, CHAR, 7, top=.07)
    jagged('Stump splinters', (0, 0, .22), .07, .08, CHAR2, 6)
    rod('Fallen limb', (.12, .05, .035), (.50, .26, .035), .040, CHAR, 5, top=.016)
    rod('Fallen twig', (.34, .17, .035), (.40, .04, .03), .014, CHAR2, 4, top=.005)
    disc('Ash ring', (0, 0, 0), .30, .27, .02, ASH)

# --- bush / macchia (live: low clump, .42 core, wider than tall) -----------------
def bush_a():
    # ash mound with the blackened stems still standing in it
    disc('Ash mound', (0, 0, 0), .62, .58, .06, ASH)
    ico('Ash heap', (0, 0, .11), (.46, .43, .13), ASH_DARK)
    twigs('Standing', (0, 0, .08), 7, .20, .30, .016, CHAR, phase=.3, jitter=.3)

def bush_b():
    # charred skeleton: a fan of bare, branched stems
    twigs('Stem', (0, 0, .03), 8, .30, .42, .024, CHAR, phase=.1, jitter=.35)
    twigs('Sprig', (0, 0, .20), 6, .24, .30, .012, CHAR2, phase=.6, jitter=.4)
    disc('Ash skirt', (0, 0, 0), .52, .48, .02, ASH_DARK)

def bush_c():
    # singed: a small shrunken clump, brown and open
    ico('Singed core', (0, 0, .24), (.44, .41, .24), SCORCH3)
    for i in range(3):
        a = i * 2.1 + .4
        ico('Singed lobe', (math.cos(a) * .34, math.sin(a) * .30, .17), (.24, .22, .17), SCORCH if i % 2 else SCORCH2)
    twigs('Bare', (0, 0, .15), 4, .38, .08, .015, CHAR, phase=.9)
    disc('Ash skirt', (0, 0, 0), .55, .5, .02, ASH_DARK)

# --- olive (live: trunk .43, four boughs, silvery crowns at .60-.68) -------------
def olive_a():
    rod('Gnarled charred trunk', (0, 0, 0), (.06, 0, .43), .068, CHAR, 5, top=.034)
    for i in range(4):
        a = i * 2.4
        end = (math.cos(a) * .26, math.sin(a) * .22, .60 + (i % 2) * .08)
        rod('Charred bough', (.06, 0, .27), end, .024, CHAR2 if i % 2 else CHAR, 4, top=.007)
    ico('Ash at the foot', (0, 0, .01), (.22, .2, .02), ASH_DARK)

def olive_b():
    rod('Scorched trunk', (0, 0, 0), (.06, 0, .43), .066, CHAR, 5, top=.034)
    for i in range(4):
        a = i * 2.4
        end = (math.cos(a) * .26, math.sin(a) * .22, .60 + (i % 2) * .08)
        rod('Bough', (.06, 0, .25), end, .022, CHAR, 4, top=.009)
        ico('Browned crown', end, (.22, .2, .15), SCORCH if i % 2 else SCORCH2)

def olive_c():
    rod('Olive stump', (0, 0, 0), (.04, 0, .26), .075, CHAR, 5, top=.05)
    jagged('Stump splinters', (.04, 0, .26), .05, .07, CHAR2, 5)
    rod('Sucker', (.0, .08, .02), (-.08, .22, .24), .014, EMBER, 4, top=.006)
    disc('Ash ring', (0, 0, 0), .24, .22, .02, ASH)

builders = {}
for species, fns in {'pine': (pine_a, pine_b, pine_c), 'chestnut': (chestnut_a, chestnut_b, chestnut_c),
                     'bush': (bush_a, bush_b, bush_c), 'olive': (olive_a, olive_b, olive_c)}.items():
    for k, fn in zip('abc', fns):
        builders[f'{species}_{k}'] = fn

# --- distant versions: a few dozen triangles each, same silhouette and tone ----
def cone(name, base, top, r1, r2, color, sides=5):
    return rod(name, base, top, r1, color, sides, top=r2)

def blob(name, pos, rx, rz, color, sides=5):
    """A squashed bipyramid: the crown of a distant tree, 10 triangles."""
    h = rz / 2
    cone(name + ' lower', (pos[0], pos[1], pos[2] - h * .6), (pos[0], pos[1], pos[2] + h * .1), .0001, rx, color, sides)
    cone(name + ' upper', (pos[0], pos[1], pos[2] + h * .1), (pos[0], pos[1], pos[2] + h * .9), rx, .0001, color, sides)

def low_pine_a(): rod('Snag', (0, 0, 0), (.07, .02, .78), .040, CHAR, 3, top=.018)
def low_pine_b():
    rod('Trunk', (0, 0, 0), (.07, .02, .82), .038, CHAR, 3, top=.02)
    blob('Browned crown', (.07, .02, .88), .30, .16, SCORCH)
def low_pine_c():
    rod('Broken trunk', (0, 0, 0), (.03, .01, .45), .042, CHAR, 3, top=.015)
    rod('Fallen crown', (.10, -.02, .03), (.52, -.16, .05), .028, CHAR, 3, top=.012)
def low_chestnut_a():
    rod('Trunk', (0, 0, 0), (0, 0, .36), .078, CHAR, 3, top=.05)
    cone('Bare crown', (0, 0, .30), (0, 0, .80), .03, .26, CHAR2, 5)
def low_chestnut_b():
    rod('Trunk', (0, 0, 0), (0, 0, .40), .075, CHAR, 3, top=.05)
    blob('Browned crown', (0, 0, .66), .36, .40, SCORCH)
def low_chestnut_c():
    rod('Stump', (0, 0, 0), (0, 0, .26), .088, CHAR, 4, top=.06)
    disc('Ash ring', (0, 0, 0), .30, .27, .02, ASH)
def low_bush_a():
    disc('Ash mound', (0, 0, 0), .62, .58, .08, ASH_DARK)
    cone('Standing stems', (0, 0, .05), (0, 0, .34), .12, .01, CHAR, 4)
def low_bush_b():
    cone('Stem fan', (0, 0, 0), (0, 0, .46), .04, .32, CHAR, 5)
    disc('Ash skirt', (0, 0, 0), .5, .46, .02, ASH_DARK)
def low_bush_c():
    blob('Singed clump', (0, 0, .2), .46, .34, SCORCH3)
    disc('Ash skirt', (0, 0, 0), .55, .5, .02, ASH_DARK)
def low_olive_a():
    rod('Trunk', (0, 0, 0), (.06, 0, .43), .068, CHAR, 3, top=.034)
    cone('Bare boughs', (.06, 0, .30), (.06, 0, .66), .02, .26, CHAR2, 4)
def low_olive_b():
    rod('Trunk', (0, 0, 0), (.06, 0, .43), .066, CHAR, 3, top=.034)
    blob('Browned crown', (.06, 0, .64), .32, .30, SCORCH2)
def low_olive_c():
    rod('Stump', (0, 0, 0), (.04, 0, .28), .075, CHAR, 4, top=.05)
    disc('Ash ring', (0, 0, 0), .24, .22, .02, ASH)

for name, fn in list(globals().items()):
    if name.startswith('low_') and callable(fn):
        builders[name[4:] + '_low'] = fn

baked = {}
for name, build in builders.items():
    current = bpy.data.collections.new(name)
    bpy.context.scene.collection.children.link(current)
    build()
    bpy.context.view_layer.update()
    lookup, ps, ns, cs, idx = {}, [], [], [], []
    for o in current.objects:
        mesh = o.to_mesh()
        mesh.calc_loop_triangles()
        color = tuple(o.data.materials[0]['srgb'])
        nm = o.matrix_world.to_3x3().inverted().transposed()
        for tri in mesh.loop_triangles:
            n = (nm @ tri.normal).normalized()
            for vi in tri.vertices:
                p = o.matrix_world @ mesh.vertices[vi].co
                # Blender Z-up/front -Y -> runtime Y-up/front +Z
                pp = [round(p.x, 5), round(p.z, 5), round(-p.y, 5)]
                nn = [round(n.x, 4), round(n.z, 4), round(-n.y, 4)]
                key = (*pp, *color)
                if key not in lookup:
                    lookup[key] = len(ps)
                    ps.append(pp); ns.append(nn); cs.append([round(c, 4) for c in color])
                idx.append(lookup[key])
        o.to_mesh_clear()
    baked[name] = dict(positions=ps, normals=ns, colors=cs, indices=idx)
    print(f'{name}: {len(ps)} vertices, {len(idx) // 3} triangles')
(OUT / 'burnt.json').write_text(json.dumps(baked, separators=(',', ':')) + '\n')

bpy.context.preferences.filepaths.save_version = 0
bpy.ops.wm.save_as_mainfile(filepath=str(OUT / 'burnt_assets.blend'))

# Contact sheet: a row per species, the three variants side by side.
rows = {'pine': -9, 'chestnut': -3, 'bush': 3, 'olive': 9}
for name in builders:
    species, k = name.split('_', 1)
    pos = Vector(((ord(k[0]) - 98) * 6.0 - 6.0 + (20.0 if k.endswith('low') else 0.0), rows[species], 0))
    scale = 4.5 if species != 'bush' else 3.5
    for o in bpy.data.collections[name].objects:
        o.location = pos + o.location * scale
        o.scale *= scale
mat_ground = bpy.data.materials.new('Studio'); mat_ground.diffuse_color = (.30, .27, .22, 1)
mat_ground.use_nodes = True; mat_ground.node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (.30, .27, .22, 1)
bpy.ops.mesh.primitive_plane_add(size=40, location=(0, 0, -.01))
bpy.context.object.data.materials.append(mat_ground)
bpy.ops.object.light_add(type='SUN', location=(0, 0, 20))
bpy.context.object.rotation_euler = (math.radians(50), 0, math.radians(35)); bpy.context.object.data.energy = 4
bpy.ops.object.camera_add(location=(9, -26, 26))
cam = bpy.context.object
cam.rotation_euler = (Vector((9, 0, 0)) - cam.location).to_track_quat('-Z', 'Y').to_euler()
cam.data.type = 'ORTHO'; cam.data.ortho_scale = 62
sc = bpy.context.scene; sc.camera = cam
sc.render.engine = 'CYCLES'; sc.cycles.samples = 24
sc.world = sc.world or bpy.data.worlds.new('W'); sc.world.color = (.35, .35, .35)
sc.render.resolution_x = 1500; sc.render.resolution_y = 1100
sc.render.filepath = str(OUT / 'burnt_preview.png')
bpy.ops.render.render(write_still=True)
