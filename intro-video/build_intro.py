"""Audiora opening film, built inside Blender (run through the Blender MCP or `blender -P`).

Roujan Digital Agency mark -> iris transition -> Audiora logo -> "a product by" end card.
24 fps, 1920x1080, 12 s. Everything is procedural except the two logo images.
"""
import bpy, math, os

D = r"C:\Users\rouja\Documents\GitHub\Audiora\intro-video"
FPS, END = 24, 288
C_OFF = (0.88, 0.88, 0.85, 1.0)
DIAL = (-0.174, 0.532)          # world position of the Audiora dial centre once it settles
scene = bpy.context.scene

# ---------------------------------------------------------------- clean slate
for o in list(bpy.data.objects):
    bpy.data.objects.remove(o)
for coll in (bpy.data.meshes, bpy.data.materials, bpy.data.curves, bpy.data.cameras, bpy.data.lights):
    for b in list(coll):
        coll.remove(b)
if scene.animation_data:
    scene.animation_data_clear()

# ---------------------------------------------------------------- render setup
try:
    scene.render.engine = 'BLENDER_EEVEE'
except TypeError as e:
    print("engine:", e)
scene.render.resolution_x, scene.render.resolution_y = 1920, 1080
scene.render.resolution_percentage = 100
scene.render.fps = FPS
scene.frame_start, scene.frame_end = 1, END
scene.view_settings.view_transform = 'Standard'
try:
    scene.view_settings.look = 'None'
except Exception:
    pass
try:
    scene.eevee.taa_render_samples = 64
except Exception:
    pass
if scene.world is None:
    scene.world = bpy.data.worlds.new("World")
scene.world.use_nodes = True
bg = next(n for n in scene.world.node_tree.nodes if n.type == 'BACKGROUND')
bg.inputs['Color'].default_value = (0, 0, 0, 1)
bg.inputs['Strength'].default_value = 0.0

# ---------------------------------------------------------------- helpers
STYLE = {
    'out':  ('EXPO', 'EASE_OUT'), 'in': ('EXPO', 'EASE_IN'), 'back': ('BACK', 'EASE_OUT'),
    'io':   ('CUBIC', 'EASE_IN_OUT'), 'lin': ('LINEAR', 'AUTO'), 'step': ('CONSTANT', 'AUTO'),
    'soft': ('SINE', 'EASE_OUT'), 'out4': ('QUART', 'EASE_OUT'), 'in4': ('QUART', 'EASE_IN'),
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
    """keys: [(frame, value, style)]; the style shapes the segment that starts at that key."""
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


def new_mat(name, blended=False):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    nt.nodes.clear()
    out = nt.nodes.new('ShaderNodeOutputMaterial')
    if blended:
        try:
            m.surface_render_method = 'BLENDED'
        except Exception as e:
            print("blend:", e)
    return m, nt, out


def Msock(nt, op, *args):
    n = nt.nodes.new('ShaderNodeMath')
    n.operation = op
    for i, a in enumerate(args):
        if isinstance(a, bpy.types.NodeSocket):
            nt.links.new(a, n.inputs[i])
        else:
            n.inputs[i].default_value = a
    return n.outputs[0]


def world_pos(nt):
    g = nt.nodes.new('ShaderNodeNewGeometry')
    return g.outputs['Position']


def dist_to(nt, x, y):
    v = nt.nodes.new('ShaderNodeVectorMath')
    v.operation = 'DISTANCE'
    nt.links.new(world_pos(nt), v.inputs[0])
    v.inputs[1].default_value = (x, y, 0.0)
    return v.outputs['Value']


def value_node(nt, name, v=0.0):
    n = nt.nodes.new('ShaderNodeValue')
    n.name = name
    n.label = name
    n.outputs[0].default_value = v
    return n


def emission_mat(name, color, strength=1.0):
    m, nt, out = new_mat(name)
    e = nt.nodes.new('ShaderNodeEmission')
    e.inputs['Color'].default_value = color
    e.inputs['Strength'].default_value = strength
    nt.links.new(e.outputs[0], out.inputs[0])
    return m, e


def sweep_mat(name, k=0.9, width=0.5, skew=0.35, base=C_OFF):
    """Flat off-white with a diagonal light band whose position is the Value node 'S'."""
    m, nt, out = new_mat(name)
    geo = nt.nodes.new('ShaderNodeNewGeometry')
    sep = nt.nodes.new('ShaderNodeSeparateXYZ')
    nt.links.new(geo.outputs['Position'], sep.inputs[0])
    S = value_node(nt, 'S', -9.0)
    d = Msock(nt, 'MULTIPLY_ADD', sep.outputs['Y'], skew, sep.outputs['X'])
    d = Msock(nt, 'SUBTRACT', d, S.outputs[0])
    d = Msock(nt, 'ABSOLUTE', d)
    d = Msock(nt, 'DIVIDE', d, width)
    d = Msock(nt, 'SUBTRACT', 1.0, d)
    d = Msock(nt, 'MAXIMUM', d, 0.0)
    d = Msock(nt, 'POWER', d, 2.0)
    mul = Msock(nt, 'MULTIPLY_ADD', d, k, 1.0)
    e = nt.nodes.new('ShaderNodeEmission')
    e.inputs['Color'].default_value = base
    nt.links.new(mul, e.inputs['Strength'])
    nt.links.new(e.outputs[0], out.inputs[0])
    return m, S


def mesh_obj(name, verts, faces, loc=(0, 0, 0), uv=None):
    me = bpy.data.meshes.new(name)
    me.from_pydata(verts, [], faces)
    me.update()
    if uv:
        layer = me.uv_layers.new(name="UVMap")
        for i, loop in enumerate(me.loops):
            layer.data[i].uv = uv[loop.vertex_index]
    ob = bpy.data.objects.new(name, me)
    scene.collection.objects.link(ob)
    ob.location = loc
    return ob


def quad(name, w, h, loc, bottom=False, uv=False):
    y0, y1 = (0, h) if bottom else (-h / 2, h / 2)
    return mesh_obj(name, [(-w / 2, y0, 0), (w / 2, y0, 0), (w / 2, y1, 0), (-w / 2, y1, 0)],
                    [(0, 1, 2, 3)], loc, uv=[(0, 0), (1, 0), (1, 1), (0, 1)] if uv else None)


def disc(name, r, loc, n=160):
    verts = [(0, 0, 0)] + [(r * math.cos(2 * math.pi * i / n), r * math.sin(2 * math.pi * i / n), 0) for i in range(n)]
    faces = [(0, 1 + i, 1 + (i + 1) % n) for i in range(n)]
    return mesh_obj(name, verts, faces, loc)


def text_obj(name, body, size, loc, mat, spacing=1.0, font="segoeuib.ttf"):
    cu = bpy.data.curves.new(name, 'FONT')
    cu.body = body
    cu.size = size
    cu.align_x = 'CENTER'
    cu.align_y = 'CENTER'
    cu.space_character = spacing
    cu.font = bpy.data.fonts.load(os.path.join(r"C:\Windows\Fonts", font))
    ob = bpy.data.objects.new(name, cu)
    scene.collection.objects.link(ob)
    ob.location = loc
    ob.data.materials.append(mat)
    return ob


# ---------------------------------------------------------------- camera
cam_data = bpy.data.cameras.new("Cam")
cam_data.lens = 50
cam = bpy.data.objects.new("Camera", cam_data)
scene.collection.objects.link(cam)
cam.location = (0, 0, 10)
scene.camera = cam
animate(cam, 'location', [(1, 10.0, 'soft'), (END, 9.2, 'lin')], index=2)

# ================================================================ ROUJAN MARK
OY = 0.20
SW, SH = 0.83, 2.24
m_r, S_r = sweep_mat("Roujan_Mark", k=0.9, width=0.55)
rect = quad("r_stem", SW, SH, (-0.504, -0.03 - SH / 2 + OY, 0), bottom=True)
rect.data.materials.append(m_r)
rect.location.y = -0.03 - SH / 2 + OY
dot = disc("r_dot", 0.405, (0.515, 0.741 + OY, 0))
dot.data.materials.append(m_r)

animate(rect, 'scale', [(1, 0, 'step'), (6, 0, 'out'), (28, 1, 'out')], index=1)
animate(dot, 'scale', [(1, 0, 'step'), (20, 0, 'back'), (36, 1, 'lin')], index=0)
animate(dot, 'scale', [(1, 0, 'step'), (20, 0, 'back'), (36, 1, 'lin')], index=1)
animate(dot, 'location', [(1, 3.2, 'step'), (18, 3.2, 'back'), (40, 0.741 + OY, 'lin')], index=1)
animate(S_r.outputs[0], 'default_value', [(42, -1.8, 'io'), (78, 2.0, 'lin')])

# exit: stem thins out, the dot flies to the centre and floods the frame
animate(rect, 'scale', [(94, 1, 'io'), (110, 0, 'step')], index=0)
animate(dot, 'location', [(96, 0.515, 'io'), (116, 0.0, 'lin')], index=0)
animate(dot, 'location', [(96, 0.741 + OY, 'io'), (116, 0.0, 'lin')], index=1)
for ax in (0, 1):
    animate(dot, 'scale', [(106, 1, 'in4'), (126, 12.5, 'step'), (127, 0, 'lin')], index=ax)
animate(rect, 'scale', [(94, 1, 'io'), (110, 0, 'step')], index=0)

m_t, e_t = emission_mat("Agency_Text", C_OFF, 0.0)
agency = text_obj("Agency", "ROUJAN DIGITAL AGENCY", 0.19, (0, -1.48, 0), m_t, spacing=2.6)
animate(e_t.inputs['Strength'], 'default_value', [(50, 0.0, 'out'), (70, 1.0, 'lin'), (94, 1.0, 'lin'), (104, 0.0, 'lin')])
animate(agency.data, 'space_character', [(50, 2.7, 'out'), (96, 1.55, 'lin')])
animate(agency, 'location', [(50, -1.56, 'out'), (72, -1.48, 'lin')], index=1)
animate(agency, 'scale', [(1, 1, 'step'), (104, 1, 'step'), (105, 0, 'lin')], index=0)
animate(agency, 'scale', [(1, 1, 'step'), (104, 1, 'step'), (105, 0, 'lin')], index=1)

# hairline under the wordmark
m_l, e_l = emission_mat("Hairline", C_OFF, 0.9)
line = quad("Hairline", 1.0, 0.012, (0, -1.74, 0))
line.data.materials.append(m_l)
animate(line, 'scale', [(1, 0, 'step'), (56, 0, 'out'), (84, 1.9, 'lin'), (96, 1.9, 'io'), (106, 0, 'step')], index=0)

# ================================================================ AUDIORA LOGO
PLANE = 4.2
img = bpy.data.images.load(os.path.join(D, "assets", "audiora-logo.png"), check_existing=True)
img.colorspace_settings.name = 'sRGB'
m_a, nt_a, out_a = new_mat("Audiora_Logo")
geo = nt_a.nodes.new('ShaderNodeNewGeometry')
sep = nt_a.nodes.new('ShaderNodeSeparateXYZ')
nt_a.links.new(geo.outputs['Position'], sep.inputs[0])
S_a = value_node(nt_a, 'S', -9.0)
d = Msock(nt_a, 'MULTIPLY_ADD', sep.outputs['Y'], 0.4, sep.outputs['X'])
d = Msock(nt_a, 'SUBTRACT', d, S_a.outputs[0])
d = Msock(nt_a, 'ABSOLUTE', d)
d = Msock(nt_a, 'DIVIDE', d, 0.6)
d = Msock(nt_a, 'SUBTRACT', 1.0, d)
d = Msock(nt_a, 'MAXIMUM', d, 0.0)
d = Msock(nt_a, 'POWER', d, 2.0)
mul = Msock(nt_a, 'MULTIPLY_ADD', d, 0.9, 1.0)
tex = nt_a.nodes.new('ShaderNodeTexImage')
tex.image = img
tex.interpolation = 'Cubic'
ea = nt_a.nodes.new('ShaderNodeEmission')
nt_a.links.new(tex.outputs['Color'], ea.inputs['Color'])
st = Msock(nt_a, 'MULTIPLY', mul, 0.88)
nt_a.links.new(st, ea.inputs['Strength'])
nt_a.links.new(ea.outputs[0], out_a.inputs[0])

logo = quad("Audiora", PLANE, PLANE, (0, 0.15, 0), uv=True)
logo.data.materials.append(m_a)
for ax in (0, 1, 2):
    animate(logo, 'scale', [(1, 0, 'step'), (125, 0, 'step'), (126, 1.2, 'out4'), (180, 1.0, 'lin')], index=ax)
animate(logo, 'rotation_euler', [(126, math.radians(-3.5), 'out4'), (190, 0.0, 'lin')], index=2)
animate(S_a.outputs[0], 'default_value', [(182, -2.4, 'io'), (222, 2.6, 'lin')])

# iris: a full-frame off-white cover with a hole that opens from the dial
def hole_cover(name):
    m, nt, out = new_mat(name, blended=True)
    R = value_node(nt, 'R', 0.0)
    dist = dist_to(nt, *DIAL)
    mask = Msock(nt, 'GREATER_THAN', dist, R.outputs[0])
    tr = nt.nodes.new('ShaderNodeBsdfTransparent')
    em = nt.nodes.new('ShaderNodeEmission')
    em.inputs['Color'].default_value = C_OFF
    mix = nt.nodes.new('ShaderNodeMixShader')
    nt.links.new(mask, mix.inputs[0])
    nt.links.new(tr.outputs[0], mix.inputs[1])
    nt.links.new(em.outputs[0], mix.inputs[2])
    nt.links.new(mix.outputs[0], out.inputs[0])
    return m, R


m_c, R_c = hole_cover("Iris_Cover")
cover = quad("Iris_Cover", 24, 14, (0, 0, 0.5))
cover.data.materials.append(m_c)
for ax in (0, 1):
    animate(cover, 'scale', [(1, 0, 'step'), (125, 0, 'step'), (126, 1, 'lin')], index=ax)
animate(R_c.outputs[0], 'default_value', [(126, 0.0, 'out4'), (170, 6.5, 'lin')])

# sound rings leaving the dial
def ring_mat(name):
    m, nt, out = new_mat(name, blended=True)
    R = value_node(nt, 'R', 0.0)
    F = value_node(nt, 'F', 0.0)
    dist = dist_to(nt, *DIAL)
    x = Msock(nt, 'DIVIDE', Msock(nt, 'SUBTRACT', dist, R.outputs[0]), 0.045)
    g = Msock(nt, 'EXPONENT', Msock(nt, 'MULTIPLY', Msock(nt, 'MULTIPLY', x, x), -1.0))
    a = Msock(nt, 'MULTIPLY', g, F.outputs[0])
    tr = nt.nodes.new('ShaderNodeBsdfTransparent')
    em = nt.nodes.new('ShaderNodeEmission')
    em.inputs['Color'].default_value = C_OFF
    mix = nt.nodes.new('ShaderNodeMixShader')
    nt.links.new(a, mix.inputs[0])
    nt.links.new(tr.outputs[0], mix.inputs[1])
    nt.links.new(em.outputs[0], mix.inputs[2])
    nt.links.new(mix.outputs[0], out.inputs[0])
    return m, R, F


for i, t0 in enumerate((150, 166, 182)):
    m, R, F = ring_mat(f"Ring{i}")
    rg = quad(f"Ring{i}", 14, 14, (0, 0, 0.3))
    rg.data.materials.append(m)
    animate(R.outputs[0], 'default_value', [(t0, 0.72, 'out'), (t0 + 70, 4.6, 'lin')])
    animate(F.outputs[0], 'default_value', [(t0, 0.0, 'lin'), (t0 + 4, 0.55, 'soft'), (t0 + 70, 0.0, 'lin')])

# end card: "a product by"
m_k, e_k = emission_mat("Credit_Text", C_OFF, 0.0)
credit = text_obj("Credit", "A PRODUCT BY ROUJAN DIGITAL AGENCY", 0.105, (0, -1.62, 0), m_k, spacing=2.0)
animate(e_k.inputs['Strength'], 'default_value', [(206, 0.0, 'out'), (230, 0.75, 'lin')])
animate(credit, 'location', [(206, -1.68, 'out'), (232, -1.62, 'lin')], index=1)
animate(credit.data, 'space_character', [(206, 2.6, 'out'), (260, 1.9, 'lin')])

# fade from / to black
m_f, nt_f, out_f = new_mat("Fade", blended=True)
F = value_node(nt_f, 'F', 1.0)
tr = nt_f.nodes.new('ShaderNodeBsdfTransparent')
em = nt_f.nodes.new('ShaderNodeEmission')
em.inputs['Color'].default_value = (0, 0, 0, 1)
mx = nt_f.nodes.new('ShaderNodeMixShader')
nt_f.links.new(F.outputs[0], mx.inputs[0])
nt_f.links.new(tr.outputs[0], mx.inputs[1])
nt_f.links.new(em.outputs[0], mx.inputs[2])
nt_f.links.new(mx.outputs[0], out_f.inputs[0])
fade = quad("Fade", 30, 18, (0, 0, 3))
fade.data.materials.append(m_f)
animate(F.outputs[0], 'default_value', [(1, 1.0, 'lin'), (12, 0.0, 'lin'), (274, 0.0, 'lin'), (END, 1.0, 'lin')])

# ---------------------------------------------------------------- output
r = scene.render
try:
    r.image_settings.file_format = 'FFMPEG'
    r.ffmpeg.format = 'MPEG4'
    r.ffmpeg.codec = 'H264'
    r.ffmpeg.constant_rate_factor = 'HIGH'
    r.ffmpeg.audio_codec = 'AAC'
    r.ffmpeg.audio_bitrate = 192
except Exception as e:
    print("ffmpeg:", e)
r.filepath = os.path.join(D, "Audiora_Intro.mp4")
scene.frame_set(1)
bpy.ops.wm.save_as_mainfile(filepath=os.path.join(D, "Audiora_Intro.blend"))
print("built ok")
