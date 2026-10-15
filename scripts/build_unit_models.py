"""Run with: Blender --background --python scripts/build_unit_models.py

The people and vehicles drawn for the player's teams: a firefighter on foot,
the engine (autobotte) and the air tanker. Bakes `assets/models/units.json`
(positions, normals, colours, indices) and writes `units_assets.blend` and a
contact sheet `units_preview.png`.

Blender: metres, Z up, front -Y. Runtime: metres, Y up, front +Z; origins at
ground level (the tanker's origin is its centre). Colours are baked as neutral
greys: the game multiplies them by the live status tint of the unit, so paint
and clothing are bright and everything else is darker, never coloured.
"""
import json
import math
from pathlib import Path

import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets/models"

bpy.ops.object.select_all(action="SELECT")
bpy.ops.object.delete(use_global=False)

# (grey level baked into the mesh, display colour in the Blender preview)
TONES = {
    "paint": (1.00, (.85, .88, .92)),
    "cloth": (0.92, (.80, .82, .85)),
    "dark": (0.16, (.03, .035, .045)),
    "metal": (0.55, (.45, .50, .55)),
    "glass": (0.26, (.09, .16, .21)),
    "light": (0.98, (.96, .94, .80)),
    "skin": (0.78, (.70, .46, .30)),
    "beacon": (0.80, (.04, .28, .95)),
}
materials = {}
current = None


def mat(tone):
    if tone not in materials:
        grey, rgb = TONES[tone]
        m = bpy.data.materials.new(tone)
        m.diffuse_color = (*rgb, 1)
        m["bake_grey"] = grey
        materials[tone] = m
    return materials[tone]


def finish(obj, name, tone):
    obj.name = name
    for c in list(obj.users_collection):
        c.objects.unlink(obj)
    current.objects.link(obj)
    obj.data.materials.append(mat(tone))
    return obj


def box(name, pos, size, tone, bevel=0.0):
    bpy.ops.mesh.primitive_cube_add(size=1, location=pos)
    o = bpy.context.object
    o.scale = size
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    if bevel:
        mod = o.modifiers.new("bevel", "BEVEL")
        mod.width = min(bevel, min(size) * .4)
        mod.segments = 2
        bpy.ops.object.modifier_apply(modifier=mod.name)
    return finish(o, name, tone)


def ico(name, pos, size, tone, sub=2):
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=sub, radius=1, location=pos)
    o = bpy.context.object
    o.scale = size
    bpy.ops.object.shade_smooth()
    return finish(o, name, tone)


def rod(name, a, b, r1, tone, r2=None, sides=8):
    a, b = Vector(a), Vector(b)
    bpy.ops.mesh.primitive_cone_add(vertices=sides, radius1=r1, radius2=r1 if r2 is None else r2,
                                    depth=(b - a).length, location=(a + b) / 2)
    o = bpy.context.object
    o.rotation_euler = (b - a).to_track_quat("Z", "Y").to_euler()
    return finish(o, name, tone)


# --- firefighter on foot ------------------------------------------------------

def firefighter():
    # Slightly stylised: big head and helmet so it still reads from the
    # game camera, where a person is a few pixels tall.
    for s in (-1, 1):
        x = s * .13
        box("Boot", (x, -.05, .08), (.20, .34, .16), "dark", .04)
        rod("Trouser leg", (x, 0, .16), (x, s * .02, .86), .095, "metal", r2=.115)
        rod("Knee pad", (x, -.09, .52), (x, -.12, .52), .08, "dark", sides=8)
        # arm held a little forward, as if carrying the tool
        rod("Sleeve", (s * .26, 0, 1.32), (s * .31, -.17, 1.02), .085, "cloth", r2=.075)
        ico("Glove", (s * .31, -.19, .98), (.075, .075, .085), "dark", 1)
    box("Hip", (0, 0, .90), (.38, .24, .16), "metal", .05)
    box("Jacket", (0, 0, 1.14), (.48, .30, .56), "cloth", .09)
    box("Reflective band", (0, -.002, 1.02), (.495, .31, .06), "light")
    box("Reflective shoulder", (0, 0, 1.32), (.50, .31, .035), "light")
    rod("Neck", (0, 0, 1.40), (0, 0, 1.48), .07, "skin")
    ico("Head", (0, -.01, 1.60), (.14, .135, .17), "skin")
    ico("Helmet", (0, .0, 1.70), (.19, .19, .12), "paint")
    box("Helmet brim", (0, -.09, 1.63), (.30, .12, .025), "paint", .01)
    box("Visor", (0, -.17, 1.63), (.20, .03, .09), "glass", .015)
    box("Helmet crest", (0, .0, 1.83), (.04, .30, .035), "metal")
    box("Backpack pump", (0, .19, 1.18), (.30, .17, .40), "dark", .06)
    rod("Pump hose", (-.1, .19, .96), (-.1, .12, .80), .025, "dark")
    # the hand tool: a rake-hoe, planted in the ground like a staff
    rod("Tool shaft", (.31, -.20, 0.0), (.31, -.20, 1.45), .022, "metal")
    box("Tool head", (.31, -.27, 1.46), (.04, .22, .09), "metal", .01)


# --- fire engine (autobotte) --------------------------------------------------

def engine():
    w, l = 2.5, 6.8
    wr = .52
    for y in (-2.0, 2.0):
        for s in (-1, 1):
            rod("Tyre", (s * (w / 2 - .12), y, wr), (s * (w / 2 + .10), y, wr), wr, "dark", sides=14)
            rod("Hub", (s * (w / 2 + .105), y, wr), (s * (w / 2 + .125), y, wr), wr * .52, "metal", sides=10)
            box("Wheel arch", (s * (w / 2 + .02), y, wr + .30), (.10, 1.25, .12), "dark", .03)
    box("Chassis", (0, 0, .80), (w * .92, l, .30), "dark", .05)
    # cab
    box("Cab", (0, -2.05, 1.62), (w, 2.2, 1.75), "paint", .14)
    box("Windscreen", (0, -3.17, 2.00), (w * .86, .04, .72), "glass", .05)
    box("Cab roof visor", (0, -3.10, 2.52), (w * .9, .35, .06), "paint", .02)
    for s in (-1, 1):
        box("Cab side window", (s * (w / 2 + .01), -2.25, 2.04), (.03, 1.1, .62), "glass", .03)
        box("Cab door line", (s * (w / 2 + .01), -1.45, 1.5), (.03, .04, 1.2), "dark")
        box("Wing mirror", (s * (w / 2 + .20), -3.0, 2.05), (.14, .26, .34), "dark", .03)
    box("Grille", (0, -3.17, 1.05), (w * .6, .05, .38), "dark")
    box("Front bumper", (0, -3.28, .68), (w * .98, .22, .22), "metal", .05)
    for s in (-1, 1):
        box("Headlamp", (s * w * .38, -3.18, 1.03), (.30, .05, .22), "light", .03)
    # body: tank with compartment shutters on both sides
    box("Equipment body", (0, 1.15, 1.72), (w, 4.3, 1.95), "paint", .09)
    ico("Water tank dome", (0, 1.15, 2.78), (.80, 2.0, .26), "paint", 2)
    for s in (-1, 1):
        for y in (.0, 1.2, 2.4):
            box("Roller shutter", (s * (w / 2 + .012), y, 1.62), (.025, 1.0, 1.15), "metal", .02)
            for z in (1.25, 1.47, 1.69, 1.91):
                box("Shutter rib", (s * (w / 2 + .03), y, z), (.02, .94, .02), "dark")
        box("Reflective stripe", (s * (w / 2 + .02), .6, .98), (.025, 4.2, .14), "light")
    # roof: ladder, lights, siren
    for s in (-1, 1):
        rod("Ladder rail", (s * .34, -.4, 3.12), (s * .34, 3.1, 3.12), .04, "metal")
        box("Light bar end", (s * .62, -3.0, 2.64), (.28, .20, .16), "beacon", .03)
    for i in range(12):
        y = -.4 + i * .31
        rod("Ladder rung", (-.34, y, 3.12), (.34, y, 3.12), .025, "metal", sides=6)
    box("Light bar", (0, -3.0, 2.64), (.9, .20, .14), "beacon", .03)
    # rear: hose reel and step
    rod("Hose reel", (-.55, 3.38, 1.75), (.55, 3.38, 1.75), .32, "dark", sides=12)
    rod("Hose reel hub", (-.57, 3.38, 1.75), (.57, 3.38, 1.75), .12, "metal", sides=8)
    box("Rear step", (0, 3.35, .72), (w * .9, .3, .12), "metal", .02)
    for s in (-1, 1):
        box("Tail lamp", (s * w * .40, 3.41, 1.15), (.24, .04, .22), "beacon", .02)


# --- air tanker ---------------------------------------------------------------

def tanker():
    # Amphibious water bomber: boat hull, high wing, two engines. Nose -Y.
    ico("Fuselage", (0, 0, 0), (1.5, 7.0, 1.35), "paint", 3)
    box("Hull step", (0, 1.0, -1.2), (1.2, 3.8, .22), "metal", .05)
    ico("Canopy", (0, -4.9, .55), (.95, 1.55, .62), "glass", 2)
    ico("Nose", (0, -6.6, -.05), (.85, .9, .85), "paint", 2)
    box("Wing", (0, -.6, 1.55), (17.0, 2.4, .26), "paint", .08)
    for s in (-1, 1):
        box("Wing tip", (s * 8.7, -.6, 1.55), (.30, 2.0, .20), "beacon", .05)
        box("Wing stripe", (s * 5.0, -.6, 1.69), (2.5, 2.42, .03), "light")
        ico("Engine nacelle", (s * 3.6, -1.7, 1.95), (.62, 1.5, .62), "metal", 2)
        rod("Propeller hub", (s * 3.6, -3.25, 1.95), (s * 3.6, -3.45, 1.95), .22, "dark", sides=8)
        box("Propeller blade", (s * 3.6, -3.42, 1.95), (.14, .05, 3.3), "dark", .01)
        rod("Wing strut", (s * 1.0, -.6, .6), (s * 3.8, -.6, 1.5), .08, "metal", sides=5)
        ico("Float", (s * 7.6, -.6, .55), (.28, 1.1, .28), "metal", 1)
        rod("Float strut", (s * 7.6, -.6, .70), (s * 7.6, -.6, 1.5), .05, "metal", sides=5)
    box("Tail fin", (0, 6.0, 2.1), (.22, 1.7, 3.0), "paint", .06)
    box("Tailplane", (0, 6.4, 3.6), (5.4, 1.2, .18), "paint", .05)
    box("Fin stripe", (0, 6.0, 2.9), (.24, 1.6, .5), "beacon")


BUILDERS = {"firefighter": firefighter, "engine": engine, "tanker": tanker}
baked = {}
for name, build in BUILDERS.items():
    current = bpy.data.collections.new(name)
    bpy.context.scene.collection.children.link(current)
    build()
    bpy.context.view_layer.update()
    pos, nrm, col, idx = [], [], [], []
    for o in current.objects:
        mesh = o.to_mesh()
        mesh.calc_loop_triangles()
        grey = o.data.materials[0]["bake_grey"]
        nm = o.matrix_world.to_3x3().inverted().transposed()
        for tri in mesh.loop_triangles:
            for vi in tri.vertices:
                p = o.matrix_world @ mesh.vertices[vi].co
                n = (nm @ (mesh.vertices[vi].normal if tri.use_smooth else tri.normal)).normalized()
                pos.append([round(p.x, 5), round(p.z, 5), round(-p.y, 5)])
                nrm.append([round(n.x, 5), round(n.z, 5), round(-n.y, 5)])
                col.append([grey, grey, grey])
                idx.append(len(idx))
        o.to_mesh_clear()
    baked[name] = dict(positions=pos, normals=nrm, colors=col, indices=idx)
    print(f"{name}: {len(idx) // 3} triangles")

(OUT / "units.json").write_text(json.dumps(baked, separators=(",", ":")) + "\n")
bpy.context.preferences.filepaths.save_version = 0
bpy.ops.wm.save_as_mainfile(filepath=str(OUT / "units_assets.blend"))

# --- contact sheet (after saving the source) ---------------------------------
layout = {"firefighter": (-5, 0, 0), "engine": (0, 0, 0), "tanker": (0, 16, 2.4)}
for name, pos in layout.items():
    for o in bpy.data.collections[name].objects:
        o.location = Vector(pos) + o.location
for tone, m in materials.items():
    m.use_nodes = True
    sh = m.node_tree.nodes.get("Principled BSDF")
    sh.inputs["Base Color"].default_value = m.diffuse_color
    sh.inputs["Roughness"].default_value = .6
red = bpy.data.materials.new("red")
red.use_nodes = True
red.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (.72, .03, .02, 1)
for o in bpy.data.collections["engine"].objects:
    if o.data.materials[0].name == "paint":
        o.data.materials[0] = red
for o in bpy.data.collections["firefighter"].objects:
    if o.data.materials[0].name == "paint":
        o.data.materials[0] = red
current = bpy.data.collections.new("studio")
bpy.context.scene.collection.children.link(current)
box("Ground", (0, 8, -.2), (50, 50, .3), "dark")
bpy.ops.object.light_add(type="SUN", location=(0, 0, 20))
bpy.context.object.data.energy = 3.5
bpy.context.object.rotation_euler = (math.radians(40), 0, math.radians(35))
bpy.ops.object.camera_add(location=(14, -16, 14))
cam = bpy.context.object
cam.rotation_euler = (Vector((-1, 7, 1.5)) - cam.location).to_track_quat("-Z", "Y").to_euler()
cam.data.type = "ORTHO"
cam.data.ortho_scale = 30
sc = bpy.context.scene
sc.camera = cam
sc.render.engine = "BLENDER_EEVEE"
sc.world = bpy.data.worlds.new("w")
sc.world.color = (.45, .5, .55)
sc.render.resolution_x, sc.render.resolution_y = 1400, 1000
sc.render.filepath = str(OUT / "units_preview.png")
bpy.ops.render.render(write_still=True)
