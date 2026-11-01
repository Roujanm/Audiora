"""Audiora opening film v2 - cinematic 3D (run inside Blender).

Roujan Digital Agency lockup floats over a glossy black floor, the camera dives into the sphere,
a flash cut lands on the Audiora logo, whose traced pieces fly in and lock together.
24 fps, 1920x1080, 15 s.
"""
import bpy, bmesh, math, os, json, random
from mathutils import Vector

D = r"C:\Users\rouja\Documents\GitHub\Audiora\intro-video"
FPS, END = 24, 360
XB = 40.0                         # world x of the Audiora set
random.seed(11)
scene = bpy.context.scene
WHITE = (0.92, 0.92, 0.90, 1.0)

# ------------------------------------------------------------------ clean slate
for o in list(bpy.data.objects):
    bpy.data.objects.remove(o)
for coll in (bpy.data.meshes, bpy.data.materials, bpy.data.curves, bpy.data.cameras, bpy.data.lights,
             bpy.data.node_groups, bpy.data.actions):
    for b in list(coll):
        coll.remove(b)
if scene.sequence_editor:
    scene.sequence_editor_clear()
if scene.animation_data:
    scene.animation_data_clear()

# ------------------------------------------------------------------ render setup
try:
    scene.render.engine = 'BLENDER_EEVEE'
except TypeError as e:
    print("engine:", e)
r = scene.render
r.resolution_x, r.resolution_y, r.resolution_percentage, r.fps = 1920, 1080, 100, FPS
scene.frame_start, scene.frame_end = 1, END
scene.view_settings.view_transform = 'AgX' if 'AgX' in [i.identifier for i in bpy.types.ColorManagedViewSettings.bl_rna.properties['view_transform'].enum_items] else 'Standard'
try:
    scene.view_settings.look = 'None'
except Exception:
    pass
for attr, val in (("taa_render_samples", 96), ("use_raytracing", True), ("ray_tracing_method", 'SCREEN'),
                  ("use_shadows", True), ("use_volumetric_shadows", False)):
    try:
        setattr(scene.eevee, attr, val)
    except Exception as e:
        print("eevee", attr, e)
if scene.world is None:
    scene.world = bpy.data.worlds.new("World")
scene.world.use_nodes = True
bgn = next(n for n in scene.world.node_tree.nodes if n.type == 'BACKGROUND')
bgn.inputs['Color'].default_value = (0.0, 0.0, 0.0, 1)
bgn.inputs['Strength'].default_value = 0.0

# ------------------------------------------------------------------ helpers
STYLE = {
    'out': ('EXPO', 'EASE_OUT'), 'in': ('EXPO', 'EASE_IN'), 'back': ('BACK', 'EASE_OUT'),
    'io': ('CUBIC', 'EASE_IN_OUT'), 'lin': ('LINEAR', 'AUTO'), 'step': ('CONSTANT', 'AUTO'),
    'soft': ('SINE', 'EASE_OUT'), 'out4': ('QUART', 'EASE_OUT'), 'in4': ('QUART', 'EASE_IN'),
    'sio': ('SINE', 'EASE_IN_OUT'),
}


def fcurves_of(idb):
    ad = idb.animation_data
    if not ad or not ad.action:
        return []
    act = ad.action
    if hasattr(act, 'fcurves'):
        return list(act.fcurves)
    out = []
    for layer in act.layers:
        for strip in layer.strips:
            for cb in strip.channelbags:
                out.extend(cb.fcurves)
    return out


def animate(target, prop, keys, index=-1):
    for f, v, _ in keys:
        if index >= 0:
            getattr(target, prop)[index] = v
            target.keyframe_insert(prop, index=index, frame=f)
        else:
            setattr(target, prop, v)
            target.keyframe_insert(prop, frame=f)
    path = target.path_from_id(prop)
    for fc in fcurves_of(target.id_data):
        if fc.data_path != path or (index >= 0 and fc.array_index != index):
            continue
        for kp in fc.keyframe_points:
            for f, _, st in keys:
                if abs(kp.co[0] - f) < 0.01:
                    kp.interpolation, kp.easing = STYLE[st]


def link(ob):
    scene.collection.objects.link(ob)
    return ob


def empty(name, loc):
    e = bpy.data.objects.new(name, None)
    e.location = loc
    return link(e)


def principled(name, color, metallic=0.0, rough=0.3, coat=0.0, emission=None):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    p = next(n for n in nt.nodes if n.type == 'BSDF_PRINCIPLED')
    p.inputs['Base Color'].default_value = color
    p.inputs['Metallic'].default_value = metallic
    p.inputs['Roughness'].default_value = rough
    for k, v in (('Coat Weight', coat), ('Coat Roughness', 0.08)):
        if k in p.inputs:
            p.inputs[k].default_value = v
    if emission:
        p.inputs['Emission Color'].default_value = color
        p.inputs['Emission Strength'].default_value = emission
    return m, p


def blended_mat(name, color, emissive=True):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    nt.nodes.clear()
    try:
        m.surface_render_method = 'BLENDED'
    except Exception:
        pass
    out = nt.nodes.new('ShaderNodeOutputMaterial')
    F = nt.nodes.new('ShaderNodeValue')
    F.name = 'F'
    tr = nt.nodes.new('ShaderNodeBsdfTransparent')
    em = nt.nodes.new('ShaderNodeEmission')
    em.inputs['Color'].default_value = color
    em.inputs['Strength'].default_value = 1.0
    mx = nt.nodes.new('ShaderNodeMixShader')
    nt.links.new(F.outputs[0], mx.inputs[0])
    nt.links.new(tr.outputs[0], mx.inputs[1])
    nt.links.new(em.outputs[0], mx.inputs[2])
    nt.links.new(mx.outputs[0], out.inputs[0])
    return m, F


def smooth(me):
    me.polygons.foreach_set('use_smooth', [True] * len(me.polygons))


def box(name, w, d, h, loc, bevel=0.012, mat=None, bottom_origin=False):
    z0, z1 = (0, h) if bottom_origin else (-h / 2, h / 2)
    v = [(-w / 2, -d / 2, z0), (w / 2, -d / 2, z0), (w / 2, d / 2, z0), (-w / 2, d / 2, z0),
         (-w / 2, -d / 2, z1), (w / 2, -d / 2, z1), (w / 2, d / 2, z1), (-w / 2, d / 2, z1)]
    f = [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)]
    me = bpy.data.meshes.new(name)
    me.from_pydata(v, [], f)
    me.update()
    ob = link(bpy.data.objects.new(name, me))
    ob.location = loc
    md = ob.modifiers.new("Bevel", 'BEVEL')
    md.width, md.segments, md.limit_method = bevel, 5, 'ANGLE'
    if mat:
        me.materials.append(mat)
    return ob


def sphere(name, radius, loc, mat, seg=64):
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=seg, v_segments=seg // 2, radius=radius)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    smooth(me)
    me.materials.append(mat)
    ob = link(bpy.data.objects.new(name, me))
    ob.location = loc
    return ob


def text3d(name, body, size, loc, mat, spacing=1.0, extrude=0.012, font="segoeuib.ttf", rot=True):
    cu = bpy.data.curves.new(name, 'FONT')
    cu.body, cu.size, cu.align_x, cu.align_y = body, size, 'CENTER', 'CENTER'
    cu.space_character = spacing
    cu.extrude, cu.bevel_depth, cu.bevel_resolution = extrude, extrude * 0.18, 2
    cu.font = bpy.data.fonts.load(os.path.join(r"C:\Windows\Fonts", font))
    cu.materials.append(mat)
    ob = link(bpy.data.objects.new(name, cu))
    ob.location = loc
    if rot:
        ob.rotation_euler = (math.pi / 2, 0, 0)
    return ob


def area_light(name, loc, target, energy, size, size_y=None, color=(1, 1, 1)):
    ld = bpy.data.lights.new(name, 'AREA')
    ld.energy, ld.shape, ld.size, ld.color = energy, 'RECTANGLE', size, color
    ld.size_y = size_y or size
    ob = link(bpy.data.objects.new(name, ld))
    ob.location = loc
    con = ob.constraints.new('TRACK_TO')
    con.target, con.track_axis, con.up_axis = target, 'TRACK_NEGATIVE_Z', 'UP_Y'
    return ob


try:
    scene.eevee.ray_tracing_options.screen_trace_quality = 1.0
    scene.eevee.ray_tracing_options.trace_max_roughness = 0.7
except Exception as e:
    print('rt opts', e)
# ------------------------------------------------------------------ floor
floor_m, fp = principled("Floor", (0.010, 0.010, 0.012, 1), 0.0, 0.07)
fp.inputs['Specular IOR Level'].default_value = 0.7
bpy.ops.mesh.primitive_plane_add(size=240, location=(20, 0, 0)) if False else None
fme = bpy.data.meshes.new("Floor")
fme.from_pydata([(-120, -120, 0), (120, -120, 0), (120, 120, 0), (-120, 120, 0)], [], [(0, 1, 2, 3)])
fme.materials.append(floor_m)
floor = link(bpy.data.objects.new("Floor", fme))
floor.location = (20, 0, 0)

# ------------------------------------------------------------------ materials
white_m, wp = principled("Satin_White", WHITE, 0.15, 0.26, coat=0.35)
text_m, tp = principled("Text_White", (0.9, 0.9, 0.88, 1), 0.0, 0.4, emission=1.4)

# ------------------------------------------------------------------ lighting rig (one per set)
def rig(x0, tag):
    focus = empty(f"Focus_{tag}", (x0, 0, 1.5))
    area_light(f"Key_{tag}", (x0 - 3.2, -5.5, 5.0), focus, 420, 3.0, color=(1.0, 0.97, 0.93))
    area_light(f"Rim_{tag}", (x0 + 4.0, 4.0, 3.2), focus, 900, 5.0, 1.0, color=(0.75, 0.86, 1.0))
    area_light(f"Fill_{tag}", (x0 + 1.5, -9.0, 1.2), focus, 70, 4.0)
    return focus


rig(0.0, "A")
rig(XB, "B")

# travelling strip light for glints
sw_target = empty("SweepTarget", (-5, 0, 1.4))
sweep = area_light("Sweep", (-5, -4.2, 2.4), sw_target, 200, 0.8, 5.0, color=(1.0, 0.98, 0.95))
for tg in (sw_target, sweep):
    animate(tg, 'location', [(1, -6.0, 'step'), (34, -6.0, 'sio'), (92, 6.0, 'step'), (190, XB - 6.0, 'sio'), (262, XB + 6.0, 'step')], index=0)

# ------------------------------------------------------------------ camera
cam_data = bpy.data.cameras.new("Cam")
cam_data.lens = 50
cam_data.dof.use_dof = True
cam_data.dof.aperture_fstop = 2.2
cam = link(bpy.data.objects.new("Camera", cam_data))
scene.camera = cam
look = empty("Look", (0, 0, 1.3))
con = cam.constraints.new('TRACK_TO')
con.target, con.track_axis, con.up_axis = look, 'TRACK_NEGATIVE_Z', 'UP_Y'
cam_data.dof.focus_object = look

Z0 = 0.95                                       # base of the Roujan mark
DOT = (0.2575, 0.0, Z0 + 0.945)                 # centre of the sphere
CUT = 119                                       # first frame of the Audiora set
DIAL_W = (XB - 0.19, 0.0, 2.35 + 0.42)          # Audiora dial centre in world

# camera path: slow push, dive into the sphere, hard cut under the flash, long pull-back
animate(cam, 'location', [(1, 0.0, 'sio'), (88, 0.0, 'io'), (113, DOT[0], 'step'), (CUT, XB - 0.9, 'out4'), (260, XB + 0.0, 'lin')], index=0)
animate(cam, 'location', [(1, -6.0, 'sio'), (88, -4.3, 'in4'), (113, -0.52, 'step'), (CUT, -3.1, 'out4'), (260, -11.8, 'lin')], index=1)
animate(cam, 'location', [(1, 1.45, 'sio'), (88, 1.55, 'io'), (113, DOT[2], 'step'), (CUT, 2.70, 'out4'), (260, 2.1, 'lin')], index=2)
animate(look, 'location', [(1, 0.0, 'sio'), (84, 0.0, 'io'), (110, DOT[0], 'step'), (CUT, XB - 0.19, 'out4'), (260, XB, 'lin')], index=0)
animate(look, 'location', [(1, 1.3, 'sio'), (84, 1.3, 'io'), (110, DOT[2], 'step'), (CUT, 2.77, 'out4'), (260, 2.0, 'lin')], index=2)

# ------------------------------------------------------------------ SET A: Roujan Digital Agency
stem = box("r_stem", 0.415, 0.20, 1.12, (-0.252, 0.0, Z0), 0.016, white_m, bottom_origin=True)
dot = sphere("r_dot", 0.2, (DOT[0], 0.0, DOT[2]), white_m)
animate(stem, 'scale', [(1, 0.001, 'step'), (8, 0.001, 'out'), (36, 1.0, 'lin')], index=2)
animate(dot, 'location', [(1, DOT[2] + 3.0, 'step'), (22, DOT[2] + 3.0, 'back'), (50, DOT[2], 'lin')], index=2)
for ax in range(3):
    animate(dot, 'scale', [(1, 0.001, 'step'), (22, 0.001, 'out'), (34, 1.0, 'lin')], index=ax)
# turn the sphere very slowly so highlights drift across it
animate(dot, 'rotation_euler', [(1, 0.0, 'lin'), (113, 1.4, 'lin')], index=2)

name_m, ne = principled("Name", (0.9, 0.9, 0.88, 1), 0.0, 0.4, emission=0.0)
roujan = text3d("ROUJAN", "ROUJAN", 0.26, (0, 0, Z0 - 0.33), name_m, spacing=1.6)
digital = text3d("DIGITAL", "DIGITAL AGENCY", 0.088, (0, 0, Z0 - 0.62), name_m, spacing=3.4, font="seguisb.ttf", extrude=0.008)
for ob, t0 in ((roujan, 52), (digital, 64)):
    animate(ob.data, 'space_character', [(t0, ob.data.space_character * 1.9, 'out'), (t0 + 50, ob.data.space_character, 'lin')])
    animate(ob, 'location', [(t0, ob.location.z - 0.12, 'out'), (t0 + 22, ob.location.z, 'lin')], index=2)
    for ax in range(3):
        animate(ob, 'scale', [(1, 0.001, 'step'), (t0, 0.001, 'step'), (t0 + 1, 1.0, 'step'), (104, 1.0, 'step'), (105, 0.001, 'lin')], index=ax)
bsdf = next(n for n in name_m.node_tree.nodes if n.type == 'BSDF_PRINCIPLED')
animate(bsdf.inputs['Emission Strength'], 'default_value', [(52, 0.0, 'out'), (76, 1.3, 'lin'), (96, 1.3, 'lin'), (106, 0.0, 'lin')])
bsdf.inputs['Emission Color'].default_value = (0.9, 0.9, 0.88, 1)

# ------------------------------------------------------------------ SET B: Audiora (traced vectors, extruded)
tr = json.load(open(os.path.join(D, "assets", "audiora_trace.json")))
PX = 2.94 / 1278.0                               # metres per source pixel
CX = CY = 1000.0
DIAL_PX = (917.0, 1182.0)
root = empty("AudioraRoot", (XB, 0.0, 2.35))
root.rotation_euler = (math.pi / 2, 0, 0)
dial_m = ((DIAL_PX[0] - CX) * PX, (DIAL_PX[1] - CY) * PX)


def loop_nodes(lp, ox, oy):
    def P(x, y):
        return ((x - CX) * PX - ox, (y - CY) * PX - oy, 0.0)
    s0 = P(*lp["start"])
    nodes = [dict(co=s0, hl=s0, hr=s0)]
    for g in lp["segs"]:
        if g[0] == 'b':
            nodes[-1]['hr'] = P(g[1], g[2])
            e = P(g[5], g[6])
            nodes.append(dict(co=e, hl=P(g[3], g[4]), hr=e))
        else:
            c, e = P(g[1], g[2]), P(g[3], g[4])
            nodes[-1]['hr'] = nodes[-1]['co']
            nodes.append(dict(co=c, hl=c, hr=c))
            nodes.append(dict(co=e, hl=e, hr=e))
    last = nodes.pop()
    nodes[0]['hl'] = last['hl']
    return nodes


def centroid_px(lp):
    pts = [lp["start"]] + [(g[-2], g[-1]) for g in lp["segs"]]
    return (sum(p[0] for p in pts) / len(pts), sum(p[1] for p in pts) / len(pts))


def wipe_mat():
    """Satin white plate that is revealed by a glowing circular front travelling out of the dial."""
    m, p = principled("Audiora_Plate", WHITE, 0.15, 0.26, coat=0.35)
    nt = m.node_tree
    out = next(n for n in nt.nodes if n.type == 'OUTPUT_MATERIAL')
    R = nt.nodes.new('ShaderNodeValue')
    R.name = 'R'
    g = nt.nodes.new('ShaderNodeNewGeometry')
    v = nt.nodes.new('ShaderNodeVectorMath')
    v.operation = 'DISTANCE'
    nt.links.new(g.outputs['Position'], v.inputs[0])
    v.inputs[1].default_value = DIAL_W
    d = v.outputs['Value']

    def M(op, a, b):
        n = nt.nodes.new('ShaderNodeMath')
        n.operation = op
        for i_, x in enumerate((a, b)):
            if isinstance(x, bpy.types.NodeSocket):
                nt.links.new(x, n.inputs[i_])
            else:
                n.inputs[i_].default_value = x
        return n.outputs[0]

    vis = M('LESS_THAN', d, R.outputs[0])
    band = M('MULTIPLY', vis, M('GREATER_THAN', d, M('SUBTRACT', R.outputs[0], 0.06)))
    em = nt.nodes.new('ShaderNodeEmission')
    em.inputs['Color'].default_value = (1, 1, 1, 1)
    em.inputs['Strength'].default_value = 7.0
    mx1 = nt.nodes.new('ShaderNodeMixShader')
    nt.links.new(band, mx1.inputs[0])
    nt.links.new(p.outputs['BSDF'], mx1.inputs[1])
    nt.links.new(em.outputs[0], mx1.inputs[2])
    tr_ = nt.nodes.new('ShaderNodeBsdfTransparent')
    mx2 = nt.nodes.new('ShaderNodeMixShader')
    nt.links.new(vis, mx2.inputs[0])
    nt.links.new(tr_.outputs[0], mx2.inputs[1])
    nt.links.new(mx1.outputs[0], mx2.inputs[2])
    nt.links.new(mx2.outputs[0], out.inputs['Surface'])
    return m, R


plate_m, plate_R = wipe_mat()
animate(plate_R.outputs[0], 'default_value', [(1, 0.0, 'step'), (CUT + 3, 0.0, 'out4'), (CUT + 62, 3.4, 'lin')])

parts = []
for i, part in enumerate(tr["parts"]):
    cxp, cyp = centroid_px(part["outer"])
    ox, oy = (cxp - CX) * PX, (cyp - CY) * PX
    cu = bpy.data.curves.new(f"A_part{i}", 'CURVE')
    cu.dimensions, cu.fill_mode = '2D', 'BOTH'
    cu.extrude, cu.bevel_depth, cu.bevel_resolution = 0.05, 0.007, 3
    for lp in [part["outer"]] + part["holes"]:
        nodes = loop_nodes(lp, ox, oy)
        sp = cu.splines.new('BEZIER')
        sp.bezier_points.add(len(nodes) - 1)
        sp.use_cyclic_u = True
        for bp, nd in zip(sp.bezier_points, nodes):
            bp.handle_left_type = bp.handle_right_type = 'FREE'
            bp.co, bp.handle_left, bp.handle_right = nd['co'], nd['hl'], nd['hr']
    cu.materials.append(plate_m if len(part["holes"]) > 10 else white_m)
    ob = link(bpy.data.objects.new(f"A_part{i}", cu))
    ob.parent = root
    ob.location = (ox, oy, 0.0)
    parts.append((ob, ox, oy, cyp))

letters = sorted([p for p in parts if p[3] < 510], key=lambda p: p[1])
others = [p for p in parts if p[3] >= 510]
print("audiora parts:", len(parts), "letters:", len(letters))

for ob, ox, oy, _ in others:                       # the connected plate: slides in while the front passes over it
    animate(ob, 'location', [(1, -0.5, 'step'), (CUT + 3, -0.5, 'out4'), (CUT + 70, 0.0, 'lin')], index=2)
    animate(ob, 'rotation_euler', [(1, 0.05, 'step'), (CUT + 3, 0.05, 'out4'), (CUT + 90, 0.0, 'lin')], index=2)

for n, (ob, ox, oy, _) in enumerate(letters):
    t0 = CUT + 52 + n * 4
    t1 = t0 + 26
    animate(ob, 'location', [(1, oy - 0.7, 'step'), (t0, oy - 0.7, 'back'), (t1, oy, 'lin')], index=1)
    for ax in range(3):
        animate(ob, 'scale', [(1, 0.001, 'step'), (t0, 0.001, 'out'), (t1, 1.0, 'lin')], index=ax)

# caption on the Audiora set
by = text3d("ByLabel", "A PRODUCT BY", 0.06, (XB, 0.0, 0.64), text_m, spacing=3.6, font="seguisb.ttf", extrude=0.006)
agency = text3d("AgencyLabel", "ROUJAN DIGITAL AGENCY", 0.13, (XB, 0.0, 0.40), text_m, spacing=2.4)
tbsdf = next(n for n in text_m.node_tree.nodes if n.type == 'BSDF_PRINCIPLED')
animate(tbsdf.inputs['Emission Strength'], 'default_value', [(1, 0.0, 'lin'), (CUT + 118, 0.0, 'out'), (CUT + 146, 1.4, 'lin')])
for ob, t0 in ((by, CUT + 118), (agency, CUT + 124)):
    animate(ob.data, 'space_character', [(t0, ob.data.space_character * 1.5, 'out'), (t0 + 60, ob.data.space_character, 'lin')])
    animate(ob, 'location', [(t0, ob.location.z - 0.08, 'out'), (t0 + 24, ob.location.z, 'lin')], index=2)

# ------------------------------------------------------------------ flash & fades (planes riding on the camera)
def cam_plane(name, color, z):
    m, F = blended_mat(name, color)
    me = bpy.data.meshes.new(name)
    me.from_pydata([(-1.2, -0.7, 0), (1.2, -0.7, 0), (1.2, 0.7, 0), (-1.2, 0.7, 0)], [], [(0, 1, 2, 3)])
    me.materials.append(m)
    ob = link(bpy.data.objects.new(name, me))
    ob.parent = cam
    ob.location = (0, 0, z)
    return F


FL = cam_plane("Flash", (1, 1, 1, 1), -0.55)
animate(FL.outputs[0], 'default_value', [(1, 0.0, 'lin'), (106, 0.0, 'in'), (117, 1.0, 'step'), (CUT, 1.0, 'out4'), (CUT + 22, 0.0, 'lin')])
FD = cam_plane("Fade", (0, 0, 0, 1), -0.45)
animate(FD.outputs[0], 'default_value', [(1, 1.0, 'lin'), (16, 0.0, 'lin'), (338, 0.0, 'lin'), (END, 1.0, 'lin')])

# ------------------------------------------------------------------ dust motes (bokeh in the beams)
dust_m, dp = principled("Dust", (1, 1, 1, 1), 0, 1.0, emission=3.0)
dme = bpy.data.meshes.new("Dust")
bmd = bmesh.new()
bmesh.ops.create_uvsphere(bmd, u_segments=10, v_segments=6, radius=0.006)
bmd.to_mesh(dme)
bmd.free()
dme.materials.append(dust_m)
for x0 in (0.0, XB):
    for i in range(45):
        ob = link(bpy.data.objects.new("dust", dme))
        p0 = (x0 + random.uniform(-3.5, 3.5), random.uniform(-4.5, 3.0), random.uniform(0.2, 3.6))
        ob.location = p0
        ob.keyframe_insert('location', frame=1)
        ob.location = (p0[0] + random.uniform(-0.5, 0.5), p0[1] + random.uniform(-0.5, 0.5), p0[2] + random.uniform(0.2, 0.9))
        ob.keyframe_insert('location', frame=END)
        s = random.uniform(0.5, 1.6)
        ob.scale = (s, s, s)

# ------------------------------------------------------------------ compositor glow
try:
    ng = bpy.data.node_groups.new("Glow", 'CompositorNodeTree')
    ng.interface.new_socket("Image", in_out='OUTPUT', socket_type='NodeSocketColor')
    rl = ng.nodes.new('CompositorNodeRLayers')
    gl = ng.nodes.new('CompositorNodeGlare')
    go = ng.nodes.new('NodeGroupOutput')
    ng.links.new(rl.outputs['Image'], gl.inputs['Image'])
    ng.links.new(gl.outputs[0], go.inputs[0])
    gl.inputs['Type'].default_value = 'Bloom'
    gl.inputs['Threshold'].default_value = 0.9
    gl.inputs['Strength'].default_value = 0.4
    gl.inputs['Size'].default_value = 0.6
    scene.compositing_node_group = ng
except Exception as e:
    print("compositor:", repr(e))

# ------------------------------------------------------------------ audio + output
wav = os.path.join(D, "assets", "intro_audio_3d.wav")
if os.path.exists(wav):
    se = scene.sequence_editor_create()
    (se.strips if hasattr(se, 'strips') else se.sequences).new_sound("Intro Audio", wav, 1, 1)
s = r.image_settings
try:
    s.media_type = 'VIDEO'
    s.file_format = 'FFMPEG'
    r.ffmpeg.format = 'MPEG4'
    r.ffmpeg.codec = 'H264'
    r.ffmpeg.constant_rate_factor = 'HIGH'
    r.ffmpeg.audio_codec = 'AAC'
    r.ffmpeg.audio_bitrate = 192
except Exception as e:
    print("ffmpeg:", e)
r.filepath = os.path.join(D, "Audiora_Intro_3D.mp4")
scene.frame_set(1)
bpy.ops.wm.save_as_mainfile(filepath=os.path.join(D, "Audiora_Intro_3D.blend"))
print("built ok")
