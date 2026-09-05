"""Run with Blender --background --python scripts/build_models.py.
Build editable low-poly assets, bake renderer meshes, and render a contact sheet.
Blender: metres, Z up, front -Y. Runtime: metres, Y up, front +Z.
"""
import bpy
import json
import math
from pathlib import Path
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'assets/models'
OUT.mkdir(parents=True, exist_ok=True)
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
materials = {}

def mat(name, color):
    if name not in materials:
        m = bpy.data.materials.new(name)
        m.diffuse_color = (*color, 1)
        m.use_nodes = True
        shader = m.node_tree.nodes.get('Principled BSDF')
        shader.inputs['Base Color'].default_value = (*color, 1)
        shader.inputs['Roughness'].default_value = .72
        materials[name] = m
    return materials[name]

white = mat('Tintable paint or clothing', (.85, .88, .92))
dark = mat('Rubber and boots', (.025, .032, .042))
glass = mat('Smoky windows', (.09, .16, .21))
metal = mat('Brushed equipment', (.42, .48, .53))
light = mat('Reflectors', (.96, .94, .8))
skin = mat('Skin', (.66, .40, .24))
leaf = mat('Foliage', (.19, .34, .095))
leaf2 = mat('Foliage tips', (.29, .43, .15))
wood = mat('Bark', (.20, .11, .045))
blue = mat('Blue beacons', (.04, .28, .95))
red = mat('Tail lights', (.8, .035, .02))
current = None

def finish(obj, name, material):
    obj.name = name
    for c in list(obj.users_collection):
        c.objects.unlink(obj)
    current.objects.link(obj)
    obj.data.materials.append(material)
    return obj

def box(name, pos, size, material, bevel=0):
    bpy.ops.mesh.primitive_cube_add(size=1, location=pos)
    o = bpy.context.object
    o.scale = size
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    if bevel:
        mod = o.modifiers.new('Soft manufactured edges', 'BEVEL')
        mod.width = min(bevel, min(size) * .4)
        mod.segments = 1
        bpy.ops.object.modifier_apply(modifier=mod.name)
    return finish(o, name, material)

def ico(name, pos, size, material):
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=1, location=pos)
    o = bpy.context.object
    o.scale = size
    return finish(o, name, material)

def rod(name, a, b, radius, material, sides=6, top=None):
    a, b = Vector(a), Vector(b)
    bpy.ops.mesh.primitive_cone_add(vertices=sides, radius1=radius, radius2=radius if top is None else top, depth=(b-a).length, location=(a+b)/2)
    o = bpy.context.object
    o.rotation_euler = (b-a).to_track_quat('Z', 'Y').to_euler()
    return finish(o, name, material)

def person(firefighter=False):
    for side in (-1, 1):
        x = side*.14
        box('Boot', (x, -.06, .09), (.20,.36,.18), dark, .035)
        rod('Trouser leg', (x,0,.18), (x,side*.035,.88), .10, metal, top=.12)
        rod('Sleeve', (side*.25,0,1.34), (side*.34,-side*.08,.96), .09, white)
        ico('Hand', (side*.34,-side*.08,.91), (.08,.075,.10), skin)
    box('Jacket' if firefighter else 'Shirt', (0,0,1.13), (.47,.29,.57), white, .07)
    rod('Neck',(0,0,1.39),(0,0,1.49),.075,skin)
    ico('Head',(0,-.01,1.62),(.15,.14,.20),skin)
    ico('Helmet' if firefighter else 'Hair',(0,.015,1.75),(.19 if firefighter else .155,.17,.105),light if firefighter else dark)
    if firefighter:
        box('Reflective belt',(0,-.155,1.0),(.44,.015,.07),light)
        box('Helmet brim',(0,-.05,1.72),(.40,.40,.035),light,.02)
        rod('Air cylinder',(0,.23,.99),(0,.23,1.36),.11,metal)
    else:
        box('Backpack',(0,.20,1.16),(.31,.19,.37),dark,.055)

def wheels(length, width, radius):
    for y in (-length*.31,length*.31):
        for side in (-1,1):
            rod('Tire',(side*(width/2-.13),y,radius),(side*(width/2+.08),y,radius),radius,dark,10)
            rod('Hub',(side*(width/2+.081),y,radius),(side*(width/2+.091),y,radius),radius*.52,metal,8)

def vehicle(truck=False):
    w,l = (2.4,6.2) if truck else (1.8,4.3)
    wheels(l,w,.49 if truck else .33)
    box('Chassis',(0,0,.65 if truck else .48),(w*.9,l,.25),dark,.05)
    if truck:
        box('Cab',(0,-1.95,1.49),(w,2.1,1.72),white,.12)
        box('Windshield',(0,-3.011,1.85),(2.06,.02,.69),glass,.055)
        box('Equipment body',(0,1.02,1.50),(w,3.75,1.9),white,.06)
        for side in (-1,1):
            box('Cab side window',(side*1.205,-1.94,1.91),(.025,1.52,.61),glass,.03)
            for y in (-.15,1.0,2.15):
                box('Roller shutter',(side*1.208,y,1.54),(.025,1.03,1.15),metal,.025)
                for z in (1.15,1.38,1.61,1.84):
                    box('Shutter rib',(side*1.226,y,z),(.025,.98,.023),light)
            box('Reflective stripe',(side*1.23,0.05,.88),(.025,5.6,.13),light)
            box('Wing mirror',(side*1.36,-2.62,1.94),(.16,.24,.28),dark,.03)
            rod('Ladder rail',(side*.34,-.65,2.57),(side*.34,2.70,2.57),.045,metal)
            ico('Beacon',(side*.77,-1.91,2.48),(.19,.20,.15),blue)
        for i in range(11):
            y=-.60+i*.32
            rod('Ladder rung',(-.34,y,2.57),(.34,y,2.57),.03,metal)
        rod('Hose reel',(-.50,2.98,1.65),(.50,2.98,1.65),.30,dark,10)
    else:
        box('Body',(0,0,.76),(w,l,.67),white,.14)
        box('Window cabin',(0,.13,1.20),(1.52,2.15,.68),glass,.19)
        box('Roof',(0,.17,1.54),(1.36,1.65,.10),white,.045)
        for side in (-1,1):
            box('Window pillar',(side*.77,.1,1.27),(.045,.12,.52),white)
            box('Mirror',(side*.96,-.70,1.12),(.20,.25,.14),white,.025)
    front=-l/2-.025
    box('Front bumper',(0,front,.60),(w*.95,.15,.18),metal,.025)
    box('Grille',(0,front-.01,1.04 if truck else .77),(w*.46,.02,.24),dark)
    for side in (-1,1):
        box('Headlamp',(side*w*.35,front-.025,.99 if truck else .82),(w*.20,.04,.20),light,.025)
        box('Tail lamp',(side*w*.37,l/2+.015,.91 if truck else .75),(.20,.035,.22),red)

def plant(kind):
    if kind == 'bush':
        for i in range(5):
            a=i*2.4
            ico('Macchia crown',(math.cos(a)*.48,math.sin(a)*.40,.52+(i%2)*.13),(.66,.58,.52),leaf if i%2 else leaf2)
        return
    rod('Tapered trunk',(0,0,0),(.025,0,.69),.036 if kind=='pine' else .055,wood,5,top=.019)
    for i in range(3):
        a=i*2.4
        end=(math.cos(a)*.23,math.sin(a)*.23,.73)
        rod('Branch',(0,0,.38),end,.017,wood,4,top=.007)
    if kind=='pine':
        for i in range(5):
            a=i*2.4
            ico('Umbrella pine crown',(math.cos(a)*.19,math.sin(a)*.19,.79+(i%2)*.10),(.30,.29,.17),leaf if i%2 else leaf2)
    else:
        for i in range(5):
            a=i*2.4
            ico('Oak crown',(math.cos(a)*.21,math.sin(a)*.21,.64+(i%2)*.16),(.32,.30,.28),leaf if i%2 else leaf2)

builders = {'pedestrian':lambda:person(), 'firefighter':lambda:person(True), 'car':lambda:vehicle(), 'fire_engine':lambda:vehicle(True), 'pine':lambda:plant('pine'), 'oak':lambda:plant('oak'), 'bush':lambda:plant('bush')}
baked={}
for name, build in builders.items():
    current=bpy.data.collections.new(name)
    bpy.context.scene.collection.children.link(current)
    build()
    bpy.context.view_layer.update()
    positions=[]; normals=[]; colors=[]; indices=[]; wood_flags=[]
    for o in current.objects:
        mesh=o.to_mesh()
        mesh.calc_loop_triangles()
        material=o.data.materials[0]
        color=material.diffuse_color[:]
        # Operational status multiplies these neutral values in the game.
        if name in ('pedestrian','firefighter','fire_engine'):
            v=max(color[:3]); color=(v,v,v,1)
        normal_matrix=o.matrix_world.to_3x3().inverted().transposed()
        for tri in mesh.loop_triangles:
            for vi in tri.vertices:
                p=o.matrix_world @ mesh.vertices[vi].co
                n=(normal_matrix @ tri.normal).normalized()
                positions.append([round(p.x,6),round(p.z,6),round(-p.y,6)])
                normals.append([round(n.x,6),round(n.z,6),round(-n.y,6)])
                colors.append([round(v,5) for v in color])
                wood_flags.append(material==wood)
                indices.append(len(indices))
        o.to_mesh_clear()
    if name in ('pine', 'oak', 'bush'):
        # Weld the vegetation bake to keep large forests compact. The chunk
        # builder computes area-weighted normals after placement.
        lookup = {}; remap = []; ps = []; ns = []; cs = []; ws = []
        for p, n, c, w in zip(positions, normals, colors, wood_flags):
            key = (*p, *c, w)
            if key not in lookup:
                lookup[key] = len(ps)
                ps.append(p); ns.append(n); cs.append(c); ws.append(w)
            remap.append(lookup[key])
        indices = [remap[i] for i in indices]
        positions, normals, colors, wood_flags = ps, ns, cs, ws
    baked[name]=dict(positions=positions,normals=normals,colors=colors,indices=indices,wood=wood_flags)
    print(f'{name}: {len(indices)//3} triangles')
(OUT/'meshes.json').write_text(json.dumps(baked,separators=(',',':'))+'\n')
# Retain originals at the origin in separate collections for easy editing/export.
bpy.context.preferences.filepaths.save_version = 0
bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'emergency_assets.blend'))
# Contact sheet staged only after saving the reusable source.
layout={'pedestrian':(-6,-3,0),'firefighter':(-4.5,-3,0),'car':(-.8,-1.5,0),'fire_engine':(4.1,0,0),'pine':(-5,5,0),'oak':(-.7,5.6,0),'bush':(3.4,5.2,0)}
for name,pos in layout.items():
    scale=4 if name in ('pine','oak') else 1.5 if name=='bush' else 1
    for o in bpy.data.collections[name].objects:
        o.location=Vector(pos)+o.location*scale
        o.scale*=scale
        if name=='fire_engine' and o.data.materials[0]==white:
            o.data.materials.clear(); o.data.materials.append(mat('Fire engine red',(.72,.025,.018)))
current=bpy.data.collections.new('Preview studio'); bpy.context.scene.collection.children.link(current)
box('Ground',(0,1,-.16),(22,20,.25),mat('Studio',(.105,.135,.17)))
bpy.ops.object.light_add(type='AREA', location=(1,-4,13))
bpy.context.object.data.energy=2400; bpy.context.object.data.shape='DISK'; bpy.context.object.data.size=9
bpy.ops.object.camera_add(location=(15,-21,19))
camera=bpy.context.object; camera.rotation_euler=(Vector((0,1,1))-camera.location).to_track_quat('-Z','Y').to_euler(); camera.data.type='ORTHO'; camera.data.ortho_scale=19
scene=bpy.context.scene; scene.camera=camera; scene.render.engine='CYCLES'; scene.cycles.samples=32
scene.world.color=(.3,.3,.3); scene.render.resolution_x=1500; scene.render.resolution_y=1200; scene.render.resolution_percentage=100
scene.render.filepath=str(OUT/'preview.png')
bpy.ops.render.render(write_still=True)
