"""Cowboy duel map v2: a weathered western town in a rocky canyon, two spawns at opposite ends of the main street.
Run: D:\\blender\\blender.exe -b -P build_map.py
Units are metres. Main street runs along X. Spawn Player1 at x=-34, Player2 at x=+34, facing each other.
Everything is procedural (materials use world-space coordinates, so no UVs / image files are needed).
"""
import bpy, bmesh, math, os, random
from mathutils import Vector, Matrix, Euler, noise

OUT = os.path.dirname(os.path.abspath(__file__))
random.seed(11)

bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
col = bpy.data.collections.new("CowboyMap")
scene.collection.children.link(col)

# =============================================================== node helpers
def nd(t, typ, **kw):
    n = t.nodes.new(typ)
    for k, v in kw.items():
        setattr(n, k, v)
    return n

def L(t, a, b):
    t.links.new(a, b)

def put(t, sock, v):
    if isinstance(v, bpy.types.NodeSocket):
        L(t, v, sock)
    else:
        sock.default_value = v

def mth(t, op, a=None, b=None, clamp=False):
    n = nd(t, 'ShaderNodeMath', operation=op, use_clamp=clamp)
    if a is not None: put(t, n.inputs[0], a)
    if b is not None: put(t, n.inputs[1], b)
    return n.outputs[0]

def ramp(t, fac, stops):
    n = nd(t, 'ShaderNodeValToRGB')
    cr = n.color_ramp
    while len(cr.elements) < len(stops):
        cr.elements.new(1.0)
    for e, (p, c) in zip(cr.elements, stops):
        e.position = p
        e.color = c if len(c) == 4 else (*c, 1)
    put(t, n.inputs[0], fac)
    return n.outputs[0]

def mixc(t, fac, a, b):
    n = nd(t, 'ShaderNodeMix', data_type='RGBA')
    put(t, n.inputs[0], fac)
    put(t, n.inputs[6], a if isinstance(a, bpy.types.NodeSocket) else (*a, 1))
    put(t, n.inputs[7], b if isinstance(b, bpy.types.NodeSocket) else (*b, 1))
    return n.outputs[2]

def noi(t, vec, scale, detail=4.0, rough=0.55):
    n = nd(t, 'ShaderNodeTexNoise', noise_dimensions='3D')
    L(t, vec, n.inputs['Vector'])
    n.inputs['Scale'].default_value = scale
    n.inputs['Detail'].default_value = detail
    n.inputs['Roughness'].default_value = rough
    return n.outputs['Fac']

def mapped(t, vec, scale):
    n = nd(t, 'ShaderNodeMapping')
    L(t, vec, n.inputs['Vector'])
    n.inputs['Scale'].default_value = scale
    return n.outputs[0]

def new_mat(name, avg, rough=0.9):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    t = m.node_tree
    b = t.nodes['Principled BSDF']
    b.inputs['Base Color'].default_value = (*avg, 1)   # shown by exporters that can't read procedural nodes
    b.inputs['Roughness'].default_value = rough
    tc = nd(t, 'ShaderNodeTexCoord')
    return m, t, b, tc.outputs['Object']

def bump(t, b, height, strength, dist=0.02):
    n = nd(t, 'ShaderNodeBump')
    n.inputs['Strength'].default_value = strength
    n.inputs['Distance'].default_value = dist
    put(t, n.inputs['Height'], height)
    L(t, n.outputs['Normal'], b.inputs['Normal'])

# =============================================================== materials
def wood(name, c1, c2, axis='Z', plank=0.22, paint=None, chip=0.45, rough=0.88, bump_s=0.8):
    avg = tuple((a + b) / 2 for a, b in zip(paint or c1, paint or c2)) if paint else tuple((a + b) / 2 for a, b in zip(c1, c2))
    m, t, b, obj = new_mat(name, avg, rough)
    sep = nd(t, 'ShaderNodeSeparateXYZ'); L(t, obj, sep.inputs[0])
    c = sep.outputs[axis]
    div = mth(t, 'DIVIDE', c, plank)
    fl = mth(t, 'FLOOR', div)
    fr = mth(t, 'FRACT', div)
    wn = nd(t, 'ShaderNodeTexWhiteNoise', noise_dimensions='1D'); L(t, fl, wn.inputs['W'])
    rnd = wn.outputs['Value']
    seam = ramp(t, fr, [(0.0, (0, 0, 0)), (0.06, (1, 1, 1))])
    sc = {'Z': (2, 2, 50), 'Y': (2, 50, 2), 'X': (50, 2, 2)}[axis]
    grain = noi(t, mapped(t, obj, sc), 1.0, 5.0)
    a = mixc(t, rnd, c1, c2)
    d = mixc(t, rnd, tuple(x * 0.5 for x in c1), tuple(x * 0.5 for x in c2))
    colr = mixc(t, grain, d, a)
    if paint:
        pn = noi(t, obj, 4.0, 8.0)
        chp = ramp(t, pn, [(chip, (0, 0, 0)), (chip + 0.1, (1, 1, 1))])
        shade = mixc(t, noi(t, obj, 1.5, 3.0), tuple(x * 0.75 for x in paint), paint)
        colr = mixc(t, chp, colr, shade)
    dust = mth(t, 'MULTIPLY', ramp(t, sep.outputs['Z'], [(0.15, (1, 1, 1)), (1.5, (0, 0, 0))]),
               ramp(t, noi(t, obj, 2.5, 6.0), [(0.25, (0.35, 0.35, 0.35)), (0.75, (1, 1, 1))]))
    colr = mixc(t, mth(t, 'MULTIPLY', dust, 0.8), colr, (0.6, 0.43, 0.26))
    streak = ramp(t, noi(t, mapped(t, obj, (6, 6, 0.45)), 1.0, 3.0), [(0.4, (0, 0, 0)), (0.72, (1, 1, 1))])
    colr = mixc(t, mth(t, 'MULTIPLY', streak, 0.4), colr, (0.05, 0.035, 0.024))
    colr = mixc(t, seam, (0.015, 0.01, 0.007), colr)
    L(t, colr, b.inputs['Base Color'])
    L(t, ramp(t, noi(t, obj, 6.0, 4.0), [(0.0, (rough - 0.2,) * 3), (1.0, (min(1, rough + 0.1),) * 3)]), b.inputs['Roughness'])
    bump(t, b, mth(t, 'ADD', mth(t, 'MULTIPLY', seam, 0.6), mth(t, 'MULTIPLY', grain, 0.3)), bump_s)
    return m

def stone_brick(name, c1, c2, mortar=(0.5, 0.47, 0.42)):
    m, t, b, obj = new_mat(name, c1, 0.9)
    sep = nd(t, 'ShaderNodeSeparateXYZ'); L(t, obj, sep.inputs[0])
    u = mth(t, 'ADD', sep.outputs['X'], sep.outputs['Y'])
    cm = nd(t, 'ShaderNodeCombineXYZ'); L(t, u, cm.inputs['X']); L(t, sep.outputs['Z'], cm.inputs['Y'])
    br = nd(t, 'ShaderNodeTexBrick', offset=0.5, offset_frequency=2, squash=1.0)
    L(t, cm.outputs[0], br.inputs['Vector'])
    br.inputs['Color1'].default_value = (*c1, 1)
    br.inputs['Color2'].default_value = (*c2, 1)
    br.inputs['Mortar'].default_value = (*mortar, 1)
    br.inputs['Scale'].default_value = 1.0
    br.inputs['Mortar Size'].default_value = 0.025
    br.inputs['Brick Width'].default_value = 0.4
    br.inputs['Row Height'].default_value = 0.14
    dirt = noi(t, obj, 3.0, 6.0)
    L(t, mixc(t, mth(t, 'MULTIPLY', dirt, 0.55), br.outputs['Color'], (0.08, 0.06, 0.05)), b.inputs['Base Color'])
    bump(t, b, mth(t, 'SUBTRACT', 1.0, br.outputs['Fac']), 0.8, 0.02)
    return m

def shingles(name, c1, c2):
    m, t, b, obj = new_mat(name, c1, 0.92)
    br = nd(t, 'ShaderNodeTexBrick', offset=0.5, offset_frequency=2, squash=1.0)
    L(t, obj, br.inputs['Vector'])
    br.inputs['Color1'].default_value = (*c1, 1)
    br.inputs['Color2'].default_value = (*c2, 1)
    br.inputs['Mortar'].default_value = (0.02, 0.015, 0.01, 1)
    br.inputs['Mortar Size'].default_value = 0.03
    br.inputs['Brick Width'].default_value = 0.22
    br.inputs['Row Height'].default_value = 0.3
    L(t, mixc(t, mth(t, 'MULTIPLY', noi(t, obj, 2.5, 6.0), 0.5), br.outputs['Color'], (0.05, 0.04, 0.03)), b.inputs['Base Color'])
    bump(t, b, mth(t, 'SUBTRACT', 1.0, br.outputs['Fac']), 1.0, 0.03)
    return m

def rust_metal(name):
    m, t, b, obj = new_mat(name, (0.42, 0.2, 0.12), 0.7)
    w = nd(t, 'ShaderNodeTexWave', wave_type='BANDS', bands_direction='X', wave_profile='SIN')
    L(t, obj, w.inputs['Vector']); w.inputs['Scale'].default_value = 5.0
    cr = ramp(t, w.outputs['Fac'], [(0.0, (0.28, 0.1, 0.06)), (1.0, (0.5, 0.25, 0.14))])
    rustn = noi(t, obj, 1.2, 8.0)
    L(t, mixc(t, ramp(t, rustn, [(0.35, (0, 0, 0)), (0.7, (1, 1, 1))]), cr, (0.2, 0.08, 0.04)), b.inputs['Base Color'])
    b.inputs['Metallic'].default_value = 0.4
    bump(t, b, w.outputs['Fac'], 0.7, 0.03)
    return m

def plain(name, rgb, rough=0.9, metal=0.0, noise_s=0.0, emit=None):
    m, t, b, obj = new_mat(name, rgb, rough)
    if noise_s:
        L(t, mixc(t, noi(t, obj, 5.0, 6.0), tuple(x * 0.65 for x in rgb), rgb), b.inputs['Base Color'])
        bump(t, b, noi(t, obj, 12.0, 6.0), 0.3)
    else:
        b.inputs['Base Color'].default_value = (*rgb, 1)
    b.inputs['Metallic'].default_value = metal
    if emit:
        b.inputs['Emission Color'].default_value = (*emit, 1)
        b.inputs['Emission Strength'].default_value = 2.0
    return m

def terrain_mat():
    m, t, b, obj = new_mat('Terrain', (0.62, 0.46, 0.3), 0.95)
    sep = nd(t, 'ShaderNodeSeparateXYZ'); L(t, obj, sep.inputs[0])
    X, Y, Z = sep.outputs['X'], sep.outputs['Y'], sep.outputs['Z']
    wob = mth(t, 'MULTIPLY', mth(t, 'SUBTRACT', noi(t, obj, 0.35, 3.0), 0.5), 3.5)
    r1 = ramp(t, mth(t, 'ADD', mth(t, 'ABSOLUTE', Y), wob), [(4.2, (1, 1, 1)), (6.2, (0, 0, 0))])
    r2 = ramp(t, mth(t, 'ADD', mth(t, 'ABSOLUTE', X), wob), [(4.2, (1, 1, 1)), (6.2, (0, 0, 0))])
    lim = ramp(t, mth(t, 'ABSOLUTE', Y), [(26.0, (1, 1, 1)), (32.0, (0, 0, 0))])
    road = mth(t, 'MAXIMUM', r1, mth(t, 'MULTIPLY', r2, lim))
    # sand
    sa = mixc(t, noi(t, obj, 0.12, 3.0), (0.86, 0.62, 0.34), (0.7, 0.46, 0.22))
    sa = mixc(t, mth(t, 'MULTIPLY', noi(t, obj, 6.0, 5.0), 0.5), sa, (0.5, 0.36, 0.22))
    dirt = mixc(t, noi(t, obj, 0.5, 4.0), (0.52, 0.34, 0.18), (0.36, 0.22, 0.11))
    ground = mixc(t, road, sa, dirt)
    vor = nd(t, 'ShaderNodeTexVoronoi', voronoi_dimensions='3D', feature='DISTANCE_TO_EDGE')
    L(t, mapped(t, obj, (1, 1, 0.2)), vor.inputs['Vector'])
    vor.inputs['Scale'].default_value = 3.2
    crack = ramp(t, vor.outputs['Distance'], [(0.0, (0, 0, 0)), (0.025, (1, 1, 1))])
    ground = mixc(t, mth(t, 'MULTIPLY', mth(t, 'SUBTRACT', 1.0, crack), 0.5), ground, (0.16, 0.09, 0.045))
    ripple = noi(t, mapped(t, obj, (0.6, 5.0, 1.0)), 2.0, 5.0)
    # rock (horizontal strata)
    w = nd(t, 'ShaderNodeTexWave', wave_type='BANDS', bands_direction='Z')
    L(t, obj, w.inputs['Vector'])
    w.inputs['Scale'].default_value = 0.55; w.inputs['Distortion'].default_value = 4.0; w.inputs['Detail'].default_value = 3.0
    strata = ramp(t, w.outputs['Fac'], [(0.0, (0.36, 0.19, 0.1)), (0.5, (0.6, 0.36, 0.2)), (1.0, (0.78, 0.55, 0.34))])
    rock = mixc(t, ramp(t, noi(t, obj, 2.0, 10.0), [(0.3, (0, 0, 0)), (0.8, (1, 1, 1))]), strata, (0.2, 0.14, 0.1))
    geo = nd(t, 'ShaderNodeNewGeometry')
    sn = nd(t, 'ShaderNodeSeparateXYZ'); L(t, geo.outputs['Normal'], sn.inputs[0])
    slope = ramp(t, sn.outputs['Z'], [(0.55, (1, 1, 1)), (0.86, (0, 0, 0))])
    alt = ramp(t, Z, [(1.0, (0, 0, 0)), (4.0, (1, 1, 1))])
    rockf = mth(t, 'MAXIMUM', slope, alt)
    L(t, mixc(t, rockf, ground, rock), b.inputs['Base Color'])
    # bump
    hs = mth(t, 'ADD', mth(t, 'ADD', noi(t, obj, 10.0, 6.0), mth(t, 'MULTIPLY', ripple, 0.6)), mth(t, 'ADD', mth(t, 'MULTIPLY', noi(t, mapped(t, obj, (0.3, 6, 2)), 1.0, 3.0), road), mth(t, 'MULTIPLY', crack, 0.5)))
    hr = noi(t, obj, 3.0, 14.0)
    hmix = mth(t, 'ADD', mth(t, 'MULTIPLY', hs, mth(t, 'SUBTRACT', 1.0, rockf)), mth(t, 'MULTIPLY', hr, rockf))
    bump(t, b, hmix, 1.0, 0.35)
    return m

def grass_mat():
    m, t, b, obj = new_mat('DryGrass', (0.72, 0.58, 0.28), 0.9)
    L(t, mixc(t, noi(t, obj, 0.3, 2.0), (0.82, 0.68, 0.34), (0.55, 0.42, 0.2)), b.inputs['Base Color'])
    return m

# palette
W_WEATHER = wood('Wood_Weathered', (0.30, 0.22, 0.15), (0.43, 0.34, 0.25))
W_DARK = wood('Wood_DarkRed', (0.16, 0.08, 0.06), (0.27, 0.14, 0.1), paint=(0.28, 0.1, 0.07), chip=0.28)
W_TEAL = wood('Wood_PaintedTeal', (0.35, 0.3, 0.24), (0.42, 0.36, 0.28), paint=(0.3, 0.46, 0.42), chip=0.28)
W_WHITE = wood('Wood_PaintedWhite', (0.34, 0.28, 0.2), (0.4, 0.33, 0.25), paint=(0.78, 0.75, 0.67), chip=0.26)
W_GREEN = wood('Wood_PaintedGreen', (0.3, 0.25, 0.18), (0.38, 0.3, 0.22), paint=(0.2, 0.3, 0.24), chip=0.26)
W_SAND = wood('Wood_Tan', (0.45, 0.35, 0.22), (0.56, 0.44, 0.29))
TRIM = wood('Wood_Trim', (0.14, 0.09, 0.055), (0.24, 0.16, 0.09), plank=1.1)
DECK = wood('Wood_Deck', (0.34, 0.26, 0.18), (0.48, 0.38, 0.27), axis='Y', plank=0.17)
CRATE = wood('Wood_Crate', (0.4, 0.3, 0.19), (0.52, 0.4, 0.26), plank=0.14)
BRICK = stone_brick('Brick', (0.42, 0.2, 0.14), (0.5, 0.26, 0.18))
STONE = stone_brick('Stone', (0.4, 0.36, 0.3), (0.5, 0.45, 0.38), mortar=(0.3, 0.28, 0.24))
SHING = shingles('Shingles', (0.17, 0.11, 0.08), (0.26, 0.17, 0.11))
RUST = rust_metal('RustMetal')
GLASS = plain('Glass', (0.04, 0.06, 0.07), rough=0.15)
SIGNB = plain('SignBoard', (0.12, 0.08, 0.05), noise_s=1)
SIGNT = plain('SignText', (0.82, 0.76, 0.6), noise_s=1)
IRON = plain('Iron', (0.06, 0.06, 0.065), rough=0.55, metal=0.8)
ROPE = plain('Rope', (0.5, 0.4, 0.26), noise_s=1)
CANVAS = plain('Canvas', (0.7, 0.66, 0.54), noise_s=1)
HAY = plain('Hay', (0.7, 0.55, 0.22), noise_s=1)
BURLAP = plain('Burlap', (0.55, 0.45, 0.3), noise_s=1)
CACTUS = plain('Cactus', (0.2, 0.34, 0.16), rough=0.8, noise_s=1)
AGAVE = plain('Agave', (0.3, 0.42, 0.3), rough=0.7)
DEADWOOD = plain('DeadWood', (0.42, 0.36, 0.3), noise_s=1)
ROCK = terrain_mat()   # same strata look for boulders and spires
GRASS = grass_mat()
PEBBLE = plain('Pebble', (0.4, 0.3, 0.22), rough=0.95, noise_s=1)
BLUE_PAD = plain('SpawnBlue', (0.1, 0.4, 1.0), rough=0.4, emit=(0.1, 0.4, 1.0))
RED_PAD = plain('SpawnRed', (1.0, 0.12, 0.08), rough=0.4, emit=(1.0, 0.12, 0.08))
SMOOTH = {'Cactus', 'Agave', 'DeadWood', 'Terrain', 'Rope', 'Burlap', 'DryGrass', 'Hay'}

# =============================================================== geometry accumulators (one mesh per material)
ACC = {}
def acc(m):
    if m.name not in ACC:
        ACC[m.name] = (m, bmesh.new())
    return ACC[m.name][1]

T = Matrix.Translation
def box(M, cx, cy, z0, sx, sy, sz, m, rot=None):
    mat = M @ T((cx, cy, z0 + sz / 2)) @ (rot if rot else Matrix.Identity(4)) @ Matrix.Diagonal((sx, sy, sz, 1))
    bmesh.ops.create_cube(acc(m), size=1.0, matrix=mat)

def cyl(M, cx, cy, z0, r, h, m, seg=12, r2=None):
    bmesh.ops.create_cone(acc(m), cap_ends=True, cap_tris=False, segments=seg, radius1=r,
                          radius2=r if r2 is None else r2, depth=h, matrix=M @ T((cx, cy, z0 + h / 2)))

def tube(m, p0, p1, r0, r1=None, seg=6):
    p0, p1 = Vector(p0), Vector(p1)
    d = p1 - p0
    if d.length < 1e-4:
        return
    rot = d.to_track_quat('Z', 'Y').to_matrix().to_4x4()
    bmesh.ops.create_cone(acc(m), cap_ends=True, cap_tris=False, segments=seg, radius1=r0,
                          radius2=r0 if r1 is None else r1, depth=d.length, matrix=T((p0 + p1) / 2) @ rot)

def lathe(M, m, profile, seg=14, cx=0, cy=0, z0=0):
    bm = acc(m)
    rings = []
    for r, z in profile:
        rings.append([bm.verts.new(M @ Vector((cx + r * math.cos(i / seg * math.tau), cy + r * math.sin(i / seg * math.tau), z0 + z))) for i in range(seg)])
    for a, b in zip(rings[:-1], rings[1:]):
        for i in range(seg):
            j = (i + 1) % seg
            bm.faces.new((a[i], a[j], b[j], b[i]))
    for ring in (rings[0], rings[-1]):
        bm.faces.new(ring)

def sphere(M, m, r, sc=(1, 1, 1), loc=(0, 0, 0)):
    bmesh.ops.create_uvsphere(acc(m), u_segments=10, v_segments=7, radius=r,
                              matrix=M @ T(loc) @ Matrix.Diagonal((*sc, 1)))

def blob(m, center, r, sc, rough_amt, subdiv=2, seed=0):
    """displaced icosphere (boulders, spires)."""
    bm = acc(m)
    tmp = bmesh.new()
    bmesh.ops.create_icosphere(tmp, subdivisions=subdiv, radius=1.0)
    for v in tmp.verts:
        p = v.co.copy()
        n = noise.noise(p * 1.3 + Vector((seed, seed * 0.7, seed * 1.3)))
        v.co = Vector((p.x * sc[0], p.y * sc[1], p.z * sc[2])) * r * (1 + rough_amt * n)
        v.co += Vector(center)
    mp = {}
    for v in tmp.verts:
        mp[v] = bm.verts.new(v.co)
    for f in tmp.faces:
        bm.faces.new([mp[v] for v in f.verts])
    tmp.free()

def sstep(a, b, x):
    t = max(0.0, min(1.0, (x - a) / (b - a)))
    return t * t * (3 - 2 * t)

def hfun(x, y):
    d = math.hypot(x / 58.0, y / 46.0)
    w = sstep(0.92, 1.45, d)
    if w <= 0:
        return 0.0
    p = Vector((x * 0.032 + 11, y * 0.032 + 7, 0))
    n = noise.fractal(p, 0.9, 2.1, 6) * 0.5 + 0.5
    h = w * w * (7 + 38 * n)
    step = 4.5
    terr = math.floor(h / step) * step + step * sstep(0.55, 1.0, (h / step) % 1.0)
    h = h * 0.35 + terr * 0.65
    return h + w * 2.5 * noise.noise(Vector((x * 0.11, y * 0.11, 3.0)))

# =============================================================== terrain
def build_terrain():
    bm = bmesh.new()
    N, SIZE = 320, 190.0
    grid = []
    for j in range(N + 1):
        row = []
        for i in range(N + 1):
            x = (i / N - 0.5) * SIZE
            y = (j / N - 0.5) * SIZE
            row.append(bm.verts.new((x, y, hfun(x, y))))
        grid.append(row)
    for j in range(N):
        for i in range(N):
            bm.faces.new((grid[j][i], grid[j][i + 1], grid[j + 1][i + 1], grid[j + 1][i]))
    ACC['Terrain'] = (ROCK, bm)
build_terrain()

# =============================================================== text
def sign_text(txt, size, M, mat, ext=0.025):
    cu = bpy.data.curves.new('t', 'FONT')
    cu.body, cu.size, cu.extrude = txt, size, ext
    cu.align_x, cu.align_y = 'CENTER', 'CENTER'
    o = bpy.data.objects.new('t', cu)
    scene.collection.objects.link(o)
    o.matrix_world = M
    bpy.context.view_layer.update()
    me = bpy.data.meshes.new_from_object(o.evaluated_get(bpy.context.evaluated_depsgraph_get()))
    bm = acc(mat)
    mp = {}
    for v in me.vertices:
        mp[v.index] = bm.verts.new(M @ v.co)
    for p in me.polygons:
        try:
            bm.faces.new([mp[i] for i in p.vertices])
        except ValueError:
            pass
    bpy.data.objects.remove(o)
    bpy.data.meshes.remove(me)
    bpy.data.curves.remove(cu)

# =============================================================== roofs / buildings
FOOT = []   # (xmin, xmax, ymin, ymax) keep-outs for vegetation

def gable(M, m, w, d, z, rise, axis='X', over=0.45):
    """closed gable-roof prism. ridge runs along X (axis X) or Y."""
    bm = acc(m)
    hu = (w if axis == 'X' else d) / 2 + over
    hv = (d if axis == 'X' else w) / 2 + over
    def P(u, v, h):
        return M @ (Vector((u, v, z + h)) if axis == 'X' else Vector((v, u, z + h)))
    A, B, C, D = P(-hu, -hv, 0), P(hu, -hv, 0), P(hu, hv, 0), P(-hu, hv, 0)
    E, F = P(-hu, 0, rise), P(hu, 0, rise)
    vs = {k: bm.verts.new(v) for k, v in dict(A=A, B=B, C=C, D=D, E=E, F=F).items()}
    for f in ('ABFE', 'DCFE', 'AED', 'BCF', 'ABCD'):
        bm.faces.new([vs[c] for c in f])
    # thickness edge boards
    return

def window(M, x, y, z, wd, ht, shutters=False):
    box(M, x, y + 0.02, z, wd, 0.06, ht, GLASS)
    box(M, x, y - 0.03, z + ht - 0.02, wd + 0.22, 0.1, 0.12, TRIM)
    box(M, x, y - 0.03, z - 0.08, wd + 0.22, 0.1, 0.12, TRIM)
    for s in (-1, 1):
        box(M, x + s * (wd / 2 + 0.05), y - 0.03, z, 0.11, 0.1, ht, TRIM)
    box(M, x, y - 0.05, z, 0.05, 0.05, ht, TRIM)
    box(M, x, y - 0.05, z + ht / 2 - 0.02, wd, 0.05, 0.05, TRIM)
    box(M, x, y - 0.12, z - 0.16, wd + 0.35, 0.22, 0.08, TRIM)
    if shutters:
        for s in (-1, 1):
            box(M, x + s * (wd / 2 + 0.36), y - 0.05, z, 0.5, 0.06, ht, W_GREEN)

def door(M, x, y, w=1.3, h=2.35):
    box(M, x, y + 0.02, 0, w, 0.08, h, SIGNB)
    box(M, x, y - 0.03, 0, w - 0.1, 0.07, h - 0.1, W_WEATHER)
    box(M, x, y - 0.06, h - 0.25, w - 0.1, 0.04, 0.12, TRIM)
    box(M, x, y - 0.06, 0.5, w - 0.1, 0.04, 0.12, TRIM)
    box(M, x, y - 0.06, 0, 0.1, 0.04, h - 0.1, TRIM)
    for s in (-1, 1):
        box(M, x + s * (w / 2 + 0.07), y - 0.03, 0, 0.14, 0.1, h + 0.1, TRIM)
    box(M, x, y - 0.03, h + 0.04, w + 0.35, 0.1, 0.14, TRIM)

def swing_doors(M, x, y):
    box(M, x, y + 0.02, 0, 1.6, 0.06, 2.3, IRON)
    for s in (-1, 1):
        box(M, x + s * 0.4, y - 0.08, 0.85, 0.74, 0.06, 1.1, W_SAND)
        for k in range(4):
            box(M, x + s * 0.4, y - 0.115, 0.9 + k * 0.26, 0.74, 0.03, 0.05, TRIM)
    for s in (-1, 1):
        box(M, x + s * 0.88, y - 0.03, 0, 0.14, 0.1, 2.4, TRIM)
    box(M, x, y - 0.03, 2.35, 1.95, 0.1, 0.14, TRIM)

def porch(M, w, d, pd, H, h1, stories, dx, upper_roof=False, brace=True):
    fy = -d / 2
    box(M, 0, fy - pd / 2, 0, w + 0.5, pd, 0.35, DECK)
    box(M, 0, fy - pd - 0.2, 0, w + 0.5, 0.15, 0.3, TRIM)
    # steps
    box(M, dx, fy - pd - 0.32, 0, 2.0, 0.45, 0.17, DECK)
    box(M, dx, fy - pd - 0.75, 0, 2.0, 0.4, 0.07, DECK)
    n = int(w / 3.2) + 2
    top = h1 - 0.0 if stories > 1 else 3.1
    for i in range(n):
        px = -w / 2 - 0.1 + i * (w + 0.2) / (n - 1)
        box(M, px, fy - pd + 0.2, 0.35, 0.2, 0.2, top - 0.35, TRIM)
        if brace:
            for s in (-1, 1):
                box(M, px + s * 0.26, fy - pd + 0.3, top - 0.7, 0.07, 0.07, 0.55, TRIM,
                    Matrix.Rotation(s * math.radians(40), 4, 'Y'))
    if stories > 1:
        box(M, 0, fy - pd / 2, top, w + 0.5, pd, 0.22, DECK)
        # railing
        yr = fy - pd + 0.12
        box(M, 0, yr, top + 0.22 + 0.95, w + 0.45, 0.12, 0.1, TRIM)
        box(M, 0, yr, top + 0.22 + 0.08, w + 0.45, 0.12, 0.08, TRIM)
        k = int((w + 0.4) / 0.3)
        for i in range(k + 1):
            box(M, -w / 2 - 0.1 + i * (w + 0.2) / k, yr, top + 0.22, 0.05, 0.05, 0.95, TRIM)
        for s in (-1, 1):
            box(M, s * (w / 2 + 0.2), fy - pd / 2, top + 0.22 + 0.95, 0.1, pd, 0.1, TRIM)
            for j in range(1, int(pd / 0.3)):
                box(M, s * (w / 2 + 0.2), fy - j * 0.3, top + 0.22, 0.05, 0.05, 0.95, TRIM)
        if upper_roof:
            for i in range(n):
                px = -w / 2 - 0.1 + i * (w + 0.2) / (n - 1)
                box(M, px, fy - pd + 0.2, top + 0.22, 0.18, 0.18, H - top - 0.22 - 0.3, TRIM)
            box(M, 0, fy - pd / 2 - 0.1, H - 0.35, w + 0.7, pd + 0.5, 0.14, SHING,
                Matrix.Rotation(math.radians(-6), 4, 'X'))
    else:
        box(M, 0, fy - pd / 2 - 0.15, top, w + 0.8, pd + 0.6, 0.16, SHING, Matrix.Rotation(math.radians(-6), 4, 'X'))
        box(M, 0, fy - pd - 0.45, top - 0.12, w + 0.8, 0.1, 0.3, TRIM)

def building(name, x, y, w, d, h1, stories, side, wall, label, pd=3.0, kind='false', door_x=0.0,
             saloon=False, upper_roof=False, roof=SHING, sign=SIGNB, label_size=0.7, shutters=False):
    rot = 0.0 if side < 0 else math.pi
    M = T((x, y, 0)) @ Matrix.Rotation(rot, 4, 'Z')
    H = h1 * stories
    FOOT.append((x - w / 2 - 1.5, x + w / 2 + 1.5, y - d / 2 - pd - 1.5, y + d / 2 + 1.5))
    box(M, 0, 0, 0, w + 0.3, d + 0.3, 0.55, STONE)
    box(M, 0, 0, 0.4, w, d, H - 0.4, wall)
    for sx in (-1, 1):
        for sy in (-1, 1):
            box(M, sx * w / 2, sy * d / 2, 0.4, 0.22, 0.22, H - 0.4, TRIM)
    # side / back boards trim at top
    box(M, 0, 0, H - 0.18, w + 0.25, d + 0.25, 0.2, TRIM)
    if stories > 1:
        box(M, 0, -d / 2 - 0.04, h1 - 0.12, w + 0.12, 0.12, 0.26, TRIM)
    gable(M, roof, w, d, H, 1.5 if kind == 'false' else 2.2)
    fy = -d / 2
    if kind == 'false':
        FH = 2.3
        box(M, 0, fy - 0.1, H - 0.2, w + 0.12, 0.3, FH + 0.2, wall)
        box(M, 0, fy - 0.18, H + FH - 0.05, w + 0.5, 0.5, 0.22, TRIM)
        box(M, 0, fy - 0.1, H + FH, w * 0.5, 0.3, 0.9, wall)
        box(M, 0, fy - 0.18, H + FH + 0.9, w * 0.5 + 0.35, 0.5, 0.2, TRIM)
        for i in range(int(w // 1.2)):
            bx = -w / 2 + 0.6 + i * 1.2
            box(M, bx, fy - 0.3, H + FH - 0.5, 0.14, 0.2, 0.5, TRIM)
        box(M, 0, fy - 0.26, H + 0.15, w * 0.68, 0.08, 1.25, sign)
        box(M, 0, fy - 0.3, H + 0.1, w * 0.68 + 0.2, 0.06, 0.1, TRIM)
        box(M, 0, fy - 0.3, H + 1.4, w * 0.68 + 0.2, 0.06, 0.1, TRIM)
        sign_text(label, label_size, M @ T((0, fy - 0.33, H + 0.78)) @ Matrix.Rotation(math.pi / 2, 4, 'X'), SIGNT)
    # facade
    zw = 1.0
    if saloon:
        swing_doors(M, door_x, fy - 0.02)
    else:
        door(M, door_x, fy - 0.02)
    cells = [c for c in (-w * 0.3, w * 0.3) if abs(c - door_x) > 1.6] or [w * 0.35]
    for cx in (-w * 0.32, w * 0.32):
        if abs(cx - door_x) > 1.7:
            window(M, cx, fy - 0.02, zw, 1.15, 1.5, shutters)
    if stories > 1:
        for i in range(3):
            window(M, -w / 2 + (i + 0.5) * w / 3, fy - 0.02, h1 + 0.9, 1.0, 1.6, shutters)
    # side windows
    for sx in (-1, 1):
        for k in (-1, 1):
            box(M, sx * (w / 2 + 0.03), k * d * 0.22, 1.0, 0.06, 1.0, 1.5, GLASS)
    if pd > 0:
        porch(M, w, d, pd, H, h1, stories, door_x, upper_roof)
    return M

def barn(name, x, y, w, d, side, label):
    rot = 0.0 if side < 0 else math.pi
    M = T((x, y, 0)) @ Matrix.Rotation(rot, 4, 'Z')
    H = 5.2
    FOOT.append((x - w / 2 - 1.5, x + w / 2 + 1.5, y - d / 2 - 2, y + d / 2 + 1.5))
    box(M, 0, 0, 0, w, d, H, W_WEATHER)
    for sx in (-1, 1):
        for sy in (-1, 1):
            box(M, sx * w / 2, sy * d / 2, 0, 0.3, 0.3, H, TRIM)
    gable(M, RUST, w, d, H, 2.8, over=0.6)
    fy = -d / 2
    box(M, 0, fy - 0.05, 0, 4.6, 0.12, 4.0, SIGNB)
    for s in (-1, 1):
        box(M, s * 1.15, fy - 0.12, 0.1, 2.2, 0.08, 3.8, W_SAND)
        box(M, s * 1.15, fy - 0.18, 0.1, 0.12, 0.05, 3.8, TRIM)
        for rr in (0, 1):
            box(M, s * 1.15, fy - 0.19, 1.9, 2.2, 0.05, 0.12, TRIM,
                Matrix.Rotation(s * math.radians(40) * (1 if rr else -1), 4, 'Y'))
    box(M, 0, fy - 0.12, 4.05, 1.6, 0.1, 1.0, SIGNB)
    box(M, 0, fy - 0.15, 4.05, 1.5, 0.05, 0.9, HAY)
    sign_text(label, 0.8, M @ T((0, fy - 0.3, 4.9)) @ Matrix.Rotation(math.pi / 2, 4, 'X'), SIGNT)
    hay_beam = box(M, 0, fy - 0.9, 4.55, 0.18, 1.7, 0.18, TRIM)
    box(M, 0, fy - 1.7, 4.2, 0.18, 0.18, 0.5, TRIM)
    # lean-to shade roof + posts
    for sx in (-w / 2 + 0.3, w / 2 - 0.3, 0):
        box(M, sx, fy - 3.0, 0, 0.22, 0.22, 3.2, TRIM)
    box(M, 0, fy - 1.6, 3.2, w, 3.4, 0.16, RUST, Matrix.Rotation(math.radians(-5), 4, 'X'))
    return M

def church(x, y):
    M = T((x, y, 0))
    w, d, H = 9.0, 15.0, 5.6
    FOOT.append((x - 7, x + 7, y - 10, y + 10))
    box(M, 0, 0, 0, w + 0.4, d + 0.4, 0.8, STONE)
    box(M, 0, 0, 0.5, w, d, H - 0.5, W_WHITE)
    for sx in (-1, 1):
        for sy in (-1, 1):
            box(M, sx * w / 2, sy * d / 2, 0.5, 0.25, 0.25, H - 0.5, TRIM)
    gable(M, RUST, w, d, H, 3.4, axis='Y', over=0.5)
    fy = -d / 2
    # entrance porch + steps
    box(M, 0, fy - 1.0, 0, 5.0, 2.0, 0.5, DECK)
    for i in range(3):
        box(M, 0, fy - 2.4 - i * 0.4, 0, 3.0, 0.5, 0.45 - i * 0.14, DECK)
    for s in (-1, 1):
        box(M, s * 2.2, fy - 1.9, 0.5, 0.25, 0.25, 3.4, TRIM)
    gable(M @ T((0, fy - 1.0, 0)), RUST, 5.0, 2.0, 3.9, 1.5, axis='Y', over=0.3)
    for s in (-1, 1):
        box(M, s * 0.65, fy - 0.03, 0.5, 1.2, 0.1, 2.6, W_WEATHER)
    box(M, 0, fy - 0.03, 0.5, 0.1, 0.12, 2.6, TRIM)
    # tower
    ty = fy + 1.8
    box(M, 0, ty, H - 0.5, 3.4, 3.4, 6.0, W_WHITE)
    for sx in (-1, 1):
        for sy in (-1, 1):
            box(M, sx * 1.7, ty + sy * 1.7, H - 0.5, 0.2, 0.2, 6.0, TRIM)
    for s in (-1, 1):
        box(M, 0, ty + s * 1.71, H + 3.0, 1.2, 0.1, 2.3, GLASS)
        box(M, s * 1.71, ty, H + 3.0, 0.1, 1.2, 2.3, GLASS)
    box(M, 0, ty, H + 5.4, 3.8, 3.8, 0.25, TRIM)
    lathe(M, RUST, [(2.7, H + 5.6), (1.9, H + 7.0), (0.9, H + 8.6), (0.12, H + 10.0)], 4, 0, ty)
    box(M, 0, ty, H + 9.9, 0.1, 0.1, 1.6, TRIM)
    box(M, 0, ty, H + 10.6, 0.8, 0.1, 0.1, TRIM)
    # tall side windows
    for sx in (-1, 1):
        for k in range(4):
            box(M, sx * (w / 2 + 0.03), -d / 2 + 5.2 + k * 2.4, 1.2, 0.08, 1.0, 2.6, GLASS)
            box(M, sx * (w / 2 + 0.07), -d / 2 + 5.2 + k * 2.4, 1.2, 0.1, 1.2, 0.12, TRIM)
            box(M, sx * (w / 2 + 0.07), -d / 2 + 5.2 + k * 2.4, 3.7, 0.1, 1.2, 0.12, TRIM)
    # fence around yard
    for i in range(-8, 9):
        for ssx in (-6.5, 6.5):
            box(M, ssx, i * 1.3, 0, 0.12, 0.12, 1.1, TRIM)
    for ssx in (-6.5, 6.5):
        box(M, ssx, 0, 0.9, 0.08, 21, 0.08, TRIM)
        box(M, ssx, 0, 0.5, 0.08, 21, 0.08, TRIM)

# =============================================================== town layout
# north side (y>0): fronts face -Y
building('Hotel', -29, 14, 13, 10, 3.6, 2, -1, W_SAND, 'HOTEL', shutters=True, upper_roof=True, door_x=0)
building('Saloon', -14.5, 14, 12, 10, 3.7, 2, -1, W_WEATHER, 'SALOON', saloon=True, label_size=1.0)
building('Bank', 10, 14, 8, 9, 3.4, 2, -1, BRICK, 'BANK', sign=SIGNB, label_size=1.1)
building('Sheriff', 19.5, 14, 9, 8, 4.2, 1, -1, W_TEAL, 'SHERIFF', label_size=0.8)
building('Barber', 29, 14, 7, 8, 4.0, 1, -1, W_DARK, 'BARBER', label_size=0.8)
# south side (y<0): fronts face +Y
building('GeneralStore', -28, -14, 11, 10, 3.5, 2, +1, W_DARK, 'GENERAL STORE', label_size=0.6, upper_roof=True)
building('Undertaker', -14, -14, 8, 9, 4.2, 1, +1, W_WHITE, 'UNDERTAKER', label_size=0.6)
barn('Livery', 19, -14, 14, 11, +1, 'LIVERY')
building('Assayer', 31.5, -14, 6, 8, 4.0, 1, +1, W_GREEN, 'ASSAYER', label_size=0.7)
church(0, 33)

# ---------------------------------------------------------------- props
def barrel(x, y, z=0, s=1.0):
    M = T((x, y, z)) @ Matrix.Diagonal((s, s, s, 1))
    prof = [(0.36, 0.0), (0.44, 0.2), (0.5, 0.5), (0.44, 0.8), (0.36, 1.0)]
    lathe(M, W_WEATHER, prof, 14)
    for zz in (0.12, 0.4, 0.72, 0.9):
        r = 0.36 + (0.14 * (1 - abs(zz - 0.5) / 0.5)) * 0.97 + 0.02
        lathe(M, IRON, [(r, zz - 0.02), (r + 0.015, zz - 0.02), (r + 0.015, zz + 0.02)], 14)

def crate(x, y, z=0, s=1.0, rot=0.0):
    M = T((x, y, z)) @ Matrix.Rotation(rot, 4, 'Z')
    box(M, 0, 0, 0, s, s, s, CRATE)
    for sx in (-1, 1):
        for sy in (-1, 1):
            box(M, sx * s / 2, sy * s / 2, 0, 0.08, 0.08, s, TRIM)
    for zz in (0.04, s - 0.12):
        box(M, 0, 0, zz, s + 0.04, s + 0.04, 0.08, TRIM)

def sack(x, y, z=0, rot=0.0):
    sphere(T((x, y, z)) @ Matrix.Rotation(rot, 4, 'Z'), BURLAP, 0.36, (1.0, 0.8, 0.9), (0, 0, 0.3))

def hay_bale(x, y, z=0, rot=0.0):
    M = T((x, y, z)) @ Matrix.Rotation(rot, 4, 'Z')
    box(M, 0, 0, 0, 1.1, 0.7, 0.6, HAY)
    for ox in (-0.3, 0.3):
        box(M, ox, 0, 0, 0.03, 0.72, 0.62, ROPE)

def trough(x, y, rot=0.0):
    M = T((x, y, 0)) @ Matrix.Rotation(rot, 4, 'Z')
    box(M, 0, 0, 0.45, 2.6, 0.7, 0.12, TRIM)
    box(M, 0, 0.34, 0.5, 2.6, 0.08, 0.4, W_WEATHER)
    box(M, 0, -0.34, 0.5, 2.6, 0.08, 0.4, W_WEATHER)
    for s in (-1, 1):
        box(M, s * 1.3, 0, 0.5, 0.08, 0.7, 0.4, W_WEATHER)
        box(M, s * 1.0, 0, 0, 0.15, 0.6, 0.5, TRIM)

def wheel(M, y, rz, r=0.78):
    n = 16
    for i in range(n):
        th = i / n * math.tau
        box(M, r * math.cos(th), y, rz + r * math.sin(th) - 0.04, 2 * r * math.sin(math.pi / n) * 1.06, 0.1, 0.08, TRIM,
            Matrix.Rotation(th + math.pi / 2, 4, 'Y'))
    for i in range(12):
        th = i / 12 * math.tau
        box(M, r / 2 * math.cos(th), y, rz + r / 2 * math.sin(th) - 0.025, r, 0.06, 0.05, W_WEATHER,
            Matrix.Rotation(-th, 4, 'Y'))
    cyl(M @ Matrix.Rotation(math.pi / 2, 4, 'X'), 0, -rz, y - 0.09, 0.13, 0.18, IRON, 10)

def wagon(x, y, rot=0.0, covered=False, broken=False):
    M = T((x, y, 0)) @ Matrix.Rotation(rot, 4, 'Z')
    FOOT.append((x - 3, x + 3, y - 3, y + 3))
    box(M, 0, 0, 0.9, 3.6, 1.5, 0.14, DECK)
    for sy in (-1, 1):
        box(M, 0, sy * 0.75, 1.04, 3.6, 0.07, 0.5, W_WEATHER)
        for k in (-1.6, -0.8, 0, 0.8, 1.6):
            box(M, k, sy * 0.78, 1.04, 0.07, 0.05, 0.5, TRIM)
    for sx in (-1, 1):
        box(M, sx * 1.8, 0, 1.04, 0.07, 1.5, 0.5, W_WEATHER)
    box(M, 1.9, 0, 0.9, 1.2, 0.1, 0.1, TRIM)          # tongue
    for sx in (-1.1, 1.1):
        box(M, sx, 0, 0.78, 0.1, 1.9, 0.1, TRIM)       # axle
        wheel(M, 0.98, 0.78) if not (broken and sx > 0) else None
        wheel(M, -0.98, 0.78)
    if covered:
        N = 8
        bm = acc(CANVAS)
        for i in range(N):
            a0, a1 = math.pi * i / N, math.pi * (i + 1) / N
            pts = []
            for a in (a0, a1):
                for xx in (-1.8, 1.8):
                    pts.append(bm.verts.new(M @ Vector((xx, 0.78 * math.cos(a), 1.5 + 0.95 * math.sin(a)))))
            bm.faces.new((pts[0], pts[1], pts[3], pts[2]))
        for xx in (-1.6, 0.0, 1.6):
            for i in range(N):
                a0 = math.pi * i / N
                a1 = math.pi * (i + 1) / N
                tube(TRIM, M @ Vector((xx, 0.8 * math.cos(a0), 1.5 + 0.97 * math.sin(a0))),
                     M @ Vector((xx, 0.8 * math.cos(a1), 1.5 + 0.97 * math.sin(a1))), 0.03)

def pole(x, y, h=7.2):
    box(Matrix.Identity(4), x, y, 0, 0.26, 0.26, h, TRIM)
    box(Matrix.Identity(4), x, y, h - 0.6, 0.18, 2.2, 0.15, TRIM)
    box(Matrix.Identity(4), x, y, h - 1.5, 0.18, 1.8, 0.13, TRIM)
    for s in (-1, 1):
        box(Matrix.Identity(4), x, y + s * 0.5, h - 1.35, 0.1, 0.1, 0.6, TRIM, Matrix.Rotation(s * math.radians(40), 4, 'X'))

def wire(p0, p1, sag=0.7, r=0.014, seg=10):
    prev = None
    for i in range(seg + 1):
        t = i / seg
        p = Vector(p0).lerp(Vector(p1), t)
        p.z -= sag * 4 * t * (1 - t)
        if prev is not None:
            tube(IRON, prev, p, r, seg=4)
        prev = p

# telegraph line
poles = [-39 + i * 13 for i in range(7)]
for px in poles:
    pole(px, -5.2)
for a, b in zip(poles[:-1], poles[1:]):
    for dy, dz in ((-0.95, 5.7), (0.95, 5.7), (-0.8, 6.6), (0.8, 6.6)):
        wire((a, -5.2 + dy, dz + 0.9 * (1 if abs(dy) < 0.9 else 0)), (b, -5.2 + dy, dz + 0.9 * (1 if abs(dy) < 0.9 else 0)))

# (the town square is left empty)

# gallows on the cross-road, south
GX, GY = 0, -17
G = T((GX, GY, 0))
box(G, 0, 0, 0, 7, 3.6, 0.9, DECK)
for sx in (-3, 0, 3):
    for sy in (-1.5, 1.5):
        box(G, sx, sy, 0, 0.3, 0.3, 0.9, TRIM)
for k in range(4):
    for s in (-1, 1):
        box(G, -3 + k * 2, s * 1.5, 0.2, 0.12, 0.12, 0.8, TRIM,
            Matrix.Rotation(math.radians(35) * s, 4, 'X'))
for s in (-2.8, 2.8):
    box(G, s, 0, 0.9, 0.3, 0.3, 4.2, TRIM)
box(G, 0, 0, 5.0, 6.4, 0.3, 0.3, TRIM)
for s in (-1, 1):
    box(G, s * 2.6, 0.15, 4.2, 0.14, 0.14, 1.1, TRIM, Matrix.Rotation(s * math.radians(40), 4, 'Y'))
for nx in (-1.3, 1.3):
    tube(ROPE, G @ Vector((nx, 0, 4.9)), G @ Vector((nx, 0, 3.4)), 0.035)
    for i in range(10):
        a0, a1 = i / 10 * math.tau, (i + 1) / 10 * math.tau
        tube(ROPE, G @ Vector((nx + 0.22 * math.cos(a0), 0, 3.0 + 0.3 * math.sin(a0) + 0.1)),
             G @ Vector((nx + 0.22 * math.cos(a1), 0, 3.0 + 0.3 * math.sin(a1) + 0.1)), 0.035)
for i in range(3):
    box(G, -2.2 + i * 0.1, 2.5, 0, 0.9, 0.9, 0.4, DECK)
FOOT.append((-5, 5, -20, -14))

# water tower
WX, WY = 36, -26
for lx in (-2.0, 2.0):
    for ly in (-2.0, 2.0):
        box(Matrix.Identity(4), WX + lx, WY + ly, 0, 0.32, 0.32, 9.5, TRIM)
for z in (2.5, 5.0, 7.5):
    for s in (-2.0, 2.0):
        box(Matrix.Identity(4), WX, WY + s, z, 4.2, 0.16, 0.16, TRIM)
        box(Matrix.Identity(4), WX + s, WY, z, 0.16, 4.2, 0.16, TRIM)
for k in range(3):
    z = 0.4 + k * 2.5
    for s in (-1, 1):
        tube(TRIM, (WX - 2, WY + s * 2, z), (WX + 2, WY + s * 2, z + 2.5), 0.06)
        tube(TRIM, (WX + 2, WY + s * 2, z), (WX - 2, WY + s * 2, z + 2.5), 0.06)
lathe(Matrix.Identity(4), W_WEATHER, [(2.5, 9.5), (2.7, 10.5), (2.7, 12.8), (2.5, 13.2)], 24, WX, WY)
for z in (10.0, 11.5, 12.8):
    lathe(Matrix.Identity(4), IRON, [(2.74, z), (2.8, z), (2.8, z + 0.07)], 24, WX, WY)
lathe(Matrix.Identity(4), SHING, [(2.9, 13.1), (1.4, 14.0), (0.0, 14.8)], 24, WX, WY)
FOOT.append((WX - 4, WX + 4, WY - 4, WY + 4))

# porch clutter
for bx, by, sd in [(-30, 8.0, -1), (-14, 8.3, -1), (10, 8.3, -1), (-28, -8.2, 1), (-14, -8.5, 1), (30, 8.5, -1)]:
    barrel(bx + 1.8, by, 0.35)
    sack(bx - 1.6, by + 0.2, 0.35)
    sack(bx - 1.0, by - 0.4, 0.35, 0.8)
    crate(bx + 3.0, by, 0.35, 0.8, 0.3)
for hx, hy in [(14, -9.5), (15.2, -9.3), (14.6, -9.4)]:
    hay_bale(hx, hy, 0 if hx != 14.6 else 0.6, random.uniform(-0.2, 0.2))
# hitching posts
for xx in range(-30, 34, 8):
    for yy in (-5.6, 5.6):
        for k in (0, 2.2):
            box(Matrix.Identity(4), xx + k, yy, 0, 0.2, 0.2, 1.3, TRIM)
        box(Matrix.Identity(4), xx + 1.1, yy, 1.05, 2.4, 0.1, 0.1, TRIM)
        box(Matrix.Identity(4), xx + 1.1, yy, 0.75, 2.4, 0.1, 0.1, TRIM)

# =============================================================== spawn markers + end barricades
def spawn(name, x, mat):
    e = bpy.data.objects.new(name, None)
    e.empty_display_type = 'ARROWS'; e.empty_display_size = 1.5
    e.location = (x, 0, 0.1)
    e.rotation_euler[2] = 0 if x < 0 else math.pi
    col.objects.link(e)
spawn("Spawn_Player1", -34, BLUE_PAD)
spawn("Spawn_Player2", 34, RED_PAD)

for sx in (-1, 1):
    X0 = sx * 41
    for yy in range(-6, 7, 2):
        box(Matrix.Identity(4), X0, yy, 0, 0.18, 0.18, 1.5, TRIM)
    box(Matrix.Identity(4), X0, 0, 1.2, 0.1, 13, 0.1, TRIM)
    box(Matrix.Identity(4), X0, 0, 0.7, 0.1, 13, 0.1, TRIM)

# =============================================================== nature
def keep_out(x, y, pad=0.0):
    if abs(y) < 6.0 or (abs(x) < 6.0 and abs(y) < 30):
        return True
    for x0, x1, y0, y1 in FOOT:
        if x0 - pad < x < x1 + pad and y0 - pad < y < y1 + pad:
            return True
    return False

def saguaro(x, y):
    z0 = hfun(x, y)
    h = random.uniform(2.6, 5.0)
    r = random.uniform(0.28, 0.4)
    tube(CACTUS, (x, y, z0 - 0.1), (x, y, z0 + h), r, r * 0.85, 10)
    sphere(Matrix.Identity(4), CACTUS, r * 0.85, (1, 1, 1), (x, y, z0 + h))
    for k in range(random.choice((0, 1, 2, 2))):
        a = random.uniform(0, math.tau)
        zz = z0 + h * random.uniform(0.35, 0.6)
        ex, ey = x + math.cos(a) * 0.9, y + math.sin(a) * 0.9
        tube(CACTUS, (x, y, zz), (ex, ey, zz), r * 0.55, r * 0.5, 8)
        tube(CACTUS, (ex, ey, zz), (ex, ey, zz + random.uniform(0.9, 1.8)), r * 0.5, r * 0.42, 8)

def agave(x, y):
    z0 = hfun(x, y)
    for i in range(14):
        a = i / 14 * math.tau + random.uniform(-0.1, 0.1)
        L_ = random.uniform(0.7, 1.1)
        tube(AGAVE, (x, y, z0 + 0.1), (x + math.cos(a) * L_, y + math.sin(a) * L_, z0 + 0.1 + random.uniform(0.35, 0.8)), 0.07, 0.01, 4)

def dead_tree(x, y, h=4.5):
    z0 = hfun(x, y)
    def branch(p, d, ln, r, depth):
        p1 = p + d * ln
        tube(DEADWOOD, p, p1, r, r * 0.62, 6)
        if depth == 0:
            return
        for _ in range(random.choice((2, 2, 3))):
            nd_ = (d + Vector((random.uniform(-0.8, 0.8), random.uniform(-0.8, 0.8), random.uniform(0.0, 0.7)))).normalized()
            branch(p1, nd_, ln * random.uniform(0.62, 0.8), r * 0.62, depth - 1)
    branch(Vector((x, y, z0 - 0.15)), Vector((random.uniform(-0.1, 0.1), random.uniform(-0.1, 0.1), 1)).normalized(), h * 0.45, 0.3, 4)

def tuft(x, y):
    bm = acc(GRASS)
    z0 = hfun(x, y)
    for _ in range(7):
        a = random.uniform(0, math.tau)
        dx, dy = math.cos(a), math.sin(a)
        ox, oy = x + random.uniform(-0.12, 0.12), y + random.uniform(-0.12, 0.12)
        hgt = random.uniform(0.35, 0.8)
        lean = random.uniform(0.1, 0.45)
        wdt = 0.035
        v0 = bm.verts.new((ox - dy * wdt, oy + dx * wdt, z0))
        v1 = bm.verts.new((ox + dy * wdt, oy - dx * wdt, z0))
        v2 = bm.verts.new((ox + dx * lean, oy + dy * lean, z0 + hgt))
        bm.faces.new((v0, v1, v2))

# saguaros, agave, dead trees, boulders, grass
n = 0
while n < 34:
    x, y = random.uniform(-58, 58), random.uniform(-50, 50)
    if not keep_out(x, y, 2) and hfun(x, y) < 1.0:
        saguaro(x, y); n += 1
n = 0
while n < 45:
    x, y = random.uniform(-58, 58), random.uniform(-50, 50)
    if not keep_out(x, y, 1) and hfun(x, y) < 1.5:
        agave(x, y); n += 1
for p in [(-46, 20), (44, 24), (-44, -28), (42, -20), (-8, 44), (15, 40)]:
    dead_tree(*p)
dead_tree(-38, 8.5, 4.0)
n = 0
while n < 60:
    x, y = random.uniform(-70, 70), random.uniform(-60, 60)
    if not keep_out(x, y, 1) and hfun(x, y) < 14:
        r = random.uniform(0.6, 2.4)
        blob(PEBBLE, (x, y, hfun(x, y) + r * 0.2), r, (1, random.uniform(0.7, 1.2), random.uniform(0.55, 0.9)), 0.22, 2, random.random() * 50)
        n += 1
n = 0
while n < 2600:
    x, y = random.uniform(-58, 58), random.uniform(-50, 50)
    if not keep_out(x, y, 0.5) and hfun(x, y) < 1.2:
        tuft(x, y); n += 1
# a few tufts hugging the porch fronts and street edges
for _ in range(220):
    x = random.uniform(-38, 38)
    y = random.choice((1, -1)) * random.uniform(5.7, 8.5)
    if not keep_out(x, y, 0.0):
        tuft(x, y)
n = 0
while n < 900:
    x, y = random.uniform(-48, 48), random.uniform(-24, 24)
    inside = any(x0 < x < x1 and y0 < y < y1 for x0, x1, y0, y1 in FOOT)
    if not inside:
        r = random.uniform(0.04, 0.14)
        blob(PEBBLE, (x, y, hfun(x, y) + r * 0.2), r, (1, random.uniform(0.7, 1.2), random.uniform(0.5, 0.9)), 0.3, 1, random.random() * 90)
        n += 1
# rock spires on the canyon rim
for a, r_ in [(0.6, 1.0), (1.9, 1.05), (3.0, 1.0), (4.0, 1.0), (5.2, 1.05)]:
    x, y = math.cos(a) * 58 * 1.42, math.sin(a) * 46 * 1.42
    if abs(x) < 88 and abs(y) < 88:
        z = hfun(x, y)
        for k in range(5):
            blob(ROCK, (x + random.uniform(-2, 2), y + random.uniform(-2, 2), z + 4 + k * 5), 5.5 - k * 0.7, (1, 1, 1.5), 0.3, 2, random.random() * 80)

# =============================================================== finalize meshes
BEVEL = {'Wood_Weathered', 'Wood_DarkRed', 'Wood_PaintedTeal', 'Wood_PaintedWhite', 'Wood_PaintedGreen', 'Wood_Tan',
         'Wood_Trim', 'Wood_Deck', 'Wood_Crate', 'Brick', 'Stone', 'Shingles', 'RustMetal', 'SignBoard', 'Hay'}
for name, (m, bm) in ACC.items():
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    if m.name in BEVEL:
        edges = [e for e in bm.edges if len(e.link_faces) == 2 and e.calc_face_angle(0.0) > 0.7]
        bmesh.ops.bevel(bm, geom=edges, offset=0.018, segments=1, affect='EDGES', clamp_overlap=True)
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    me.materials.append(m)
    if m.name in SMOOTH or name == 'Terrain':
        for p in me.polygons:
            p.use_smooth = True
    o = bpy.data.objects.new(name, me)
    col.objects.link(o)

# =============================================================== sky, sun, camera
w = bpy.data.worlds.new("Sky"); scene.world = w; w.use_nodes = True
wt = w.node_tree
bg = wt.nodes['Background']
sky = nd(wt, 'ShaderNodeTexSky')
try:
    sky.sky_type = 'MULTIPLE_SCATTERING'
except Exception:
    pass
for k, v in dict(sun_elevation=math.radians(34), sun_rotation=math.radians(210), air_density=1.1, dust_density=1.6, sun_intensity=1.0).items():
    try:
        setattr(sky, k, v)
    except Exception:
        pass
tcw = nd(wt, 'ShaderNodeTexCoord')
dirv = nd(wt, 'ShaderNodeVectorMath', operation='NORMALIZE'); L(wt, tcw.outputs['Generated'], dirv.inputs[0])
sp = nd(wt, 'ShaderNodeSeparateXYZ'); L(wt, dirv.outputs[0], sp.inputs[0])
cm = mapped(wt, dirv.outputs[0], (3.0, 3.0, 7.0))
cl = ramp(wt, noi(wt, cm, 1.4, 7.0, 0.6), [(0.52, (0, 0, 0)), (0.74, (1, 1, 1))])
cl = mth(wt, 'MULTIPLY', cl, ramp(wt, sp.outputs['Z'], [(0.04, (0, 0, 0)), (0.35, (1, 1, 1))]))
L(wt, mixc(wt, mth(wt, 'MULTIPLY', cl, 0.92), sky.outputs[0], (1.4, 1.35, 1.25)), bg.inputs['Color'])
HAZE = float(os.environ.get('HAZE', '0'))
if HAZE > 0:
    vs = nd(wt, 'ShaderNodeVolumePrincipled')
    vs.inputs['Density'].default_value = HAZE
    vs.inputs['Anisotropy'].default_value = 0.55
    vs.inputs['Color'].default_value = (1.0, 0.9, 0.75, 1)
    L(wt, vs.outputs[0], wt.nodes['World Output'].inputs['Volume'])
bg.inputs['Strength'].default_value = 0.22

sun = bpy.data.lights.new("Sun", 'SUN')
sun.energy = 4.2
sun.angle = math.radians(0.9)
sun.color = (1.0, 0.8, 0.58)
so = bpy.data.objects.new("Sun", sun)
so.rotation_euler = (math.radians(56), math.radians(0), math.radians(-30))
col.objects.link(so)

def cam(name, loc, target, lens=28, ortho=None, dof=0):
    cd = bpy.data.cameras.new(name)
    if dof:
        cd.dof.use_dof = True
        cd.dof.focus_distance = (Vector(target) - Vector(loc)).length * dof
        cd.dof.aperture_fstop = 5.6
    cd.lens = lens
    cd.clip_end = 600
    if ortho:
        cd.type = 'ORTHO'; cd.ortho_scale = ortho
    co = bpy.data.objects.new(name, cd)
    co.location = loc
    col.objects.link(co)
    co.rotation_euler = (Vector(target) - Vector(loc)).to_track_quat('-Z', 'Y').to_euler()
    return co

top = cam("Cam_TopDown", (0, 0, 150), (0, 0, 0), ortho=105)
over = cam("Cam_Overview", (-30, -40, 22), (2, 6, 2), lens=24)
p1 = cam("Cam_Player1View", (-34, 0, 1.75), (34, 0, 2.2), lens=24, dof=0.22)
hero = cam("Cam_Hero", (-33, 5.0, 3.4), (12, 2, 3.0), lens=24, dof=0.55)
low = cam("Cam_SaloonCorner", (-3.5, -9.0, 1.7), (-13, 8, 3.6), lens=26)

# =============================================================== duel animation (24 fps, 720 frames = 30 s)
#  1-144   both gunmen walk in from the street ends and meet in the middle
#  144-192 they stand face to face, then turn back to back
#  216-456 ten slow paces apart
#  456-539 tension: close-ups of the gun hands, a tumbleweed rolls through the gap
#  540     "ting" bell -> both spin round, draw, fire (562/563) and fall
#  600-720 slow crane out, the tumbleweed rolls on
import numpy as np, wave

scene.render.fps = 24
scene.frame_start, scene.frame_end = 1, 720
F_MEET, F_T0, F_T1, F_W0, F_W1, F_TING = 144, 168, 192, 216, 456, 540
F_DRAW, F_SHOT = 549, {'P1': 562, 'P2': 563}
START_X, PACE_X = 14.0, 8.4          # walk-in distance from the middle, and ten paces apart

def interp(kind):
    try:
        bpy.context.preferences.edit.keyframe_new_interpolation_type = kind
    except Exception:
        pass

def key(o, prop, frame, value=None, kind='LINEAR', index=-1):
    interp(kind)
    if value is not None:
        setattr(o, prop, value)
    o.keyframe_insert(prop, frame=frame, index=index)

# ---- materials for the gunmen
SKIN = plain('Skin', (0.42, 0.25, 0.17), rough=0.5, noise_s=1)
LEATHER = plain('Leather', (0.1, 0.06, 0.035), rough=0.6, noise_s=1)
FLASH = plain('MuzzleFlash', (1, 0.7, 0.2), emit=(1.0, 0.65, 0.15))
FLASH.node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 40.0

def bx(bm, c, s):
    bmesh.ops.create_cube(bm, size=1.0, matrix=T(c) @ Matrix.Diagonal((*s, 1)))
def cy_(bm, c, r, h, seg=12, r2=None):
    bmesh.ops.create_cone(bm, cap_ends=True, cap_tris=False, segments=seg, radius1=r,
                          radius2=r if r2 is None else r2, depth=h, matrix=T(c))
def sp_(bm, c, r, sc=(1, 1, 1)):
    bmesh.ops.create_uvsphere(bm, u_segments=12, v_segments=8, radius=r, matrix=T(c) @ Matrix.Diagonal((*sc, 1)))

def empty(name, parent, loc):
    e = bpy.data.objects.new(name, None)
    e.empty_display_size = 0.2
    col.objects.link(e)
    e.parent = parent
    e.location = loc
    return e

def mesh_obj(name, mat, parent, loc, build, smooth=False, thick=0.0):
    bm = bmesh.new()
    build(bm)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me); bm.free()
    if smooth:
        for p in me.polygons:
            p.use_smooth = True
    o = bpy.data.objects.new(name, me)
    o.data.materials.append(mat)
    col.objects.link(o)
    if thick:
        so = o.modifiers.new('Solid', 'SOLIDIFY')
        so.thickness = thick
        so.offset = 0.0
    if smooth and 'Hat' not in name:
        sm = o.modifiers.new('Subsurf', 'SUBSURF')
        sm.levels = sm.render_levels = 2
    o.parent = parent
    o.location = loc
    return o

HAIR = plain('Hair', (0.05, 0.035, 0.025), rough=0.8, noise_s=1)
BRASS = plain('Brass', (0.6, 0.42, 0.1), rough=0.3, metal=0.95)
SILVER = plain('SilverTrim', (0.55, 0.55, 0.56), rough=0.3, metal=0.95)
EYE = plain('Eye', (0.04, 0.025, 0.02), rough=0.25)
SCLERA = plain('Sclera', (0.55, 0.52, 0.48), rough=0.3)
GLOVE = plain('Glove', (0.025, 0.025, 0.028), rough=0.55, noise_s=1)

def leather(name, rgb, rough=0.5):
    m, t, b, obj = new_mat(name, rgb, rough)
    wear = noi(t, obj, 18.0, 6.0)
    pat = noi(t, obj, 3.0, 4.0)
    light = tuple(min(1.0, c * 1.45) for c in rgb)
    colr = mixc(t, ramp(t, wear, [(0.45, (0, 0, 0)), (0.75, (1, 1, 1))]), rgb, light)
    colr = mixc(t, mth(t, 'MULTIPLY', pat, 0.5), colr, tuple(c * 0.45 for c in rgb))
    L(t, colr, b.inputs['Base Color'])
    L(t, ramp(t, wear, [(0.3, (max(0.1, rough - 0.15),) * 3), (0.8, (min(1.0, rough + 0.25),) * 3)]), b.inputs['Roughness'])
    bump(t, b, mth(t, 'ADD', wear, noi(t, mapped(t, obj, (40, 2, 40)), 1.0, 2.0)), 0.5, 0.004)
    return m

def sweater(name, c1, c2):
    m, t, b, obj = new_mat(name, c1, 0.95)
    sep = nd(t, 'ShaderNodeSeparateXYZ'); L(t, obj, sep.inputs[0])
    u = mth(t, 'ADD', sep.outputs['X'], sep.outputs['Y'])
    saw = mth(t, 'ABSOLUTE', mth(t, 'SUBTRACT', mth(t, 'FRACT', mth(t, 'MULTIPLY', u, 24.0)), 0.5))
    band = mth(t, 'FRACT', mth(t, 'ADD', mth(t, 'MULTIPLY', sep.outputs['Z'], 15.0), saw))
    pat = ramp(t, band, [(0.0, (0, 0, 0)), (0.5, (0, 0, 0)), (0.53, (1, 1, 1))])
    colr = mixc(t, pat, c1, c2)
    colr = mixc(t, mth(t, 'MULTIPLY', noi(t, obj, 60.0, 3.0), 0.5), colr, tuple(c * 0.6 for c in c1))
    L(t, colr, b.inputs['Base Color'])
    bump(t, b, mth(t, 'ADD', pat, noi(t, obj, 120.0, 2.0)), 0.6, 0.003)
    return m

def loft(bm, rings, n=16, caps=(True, True), arc=None):
    """rings: (z, cx, cy, rx, ry). arc=(a0,a1) makes an open shell instead of a closed tube (front is +Y = 90deg)."""
    rows = []
    if arc is None:
        for z, cx, cy, rx, ry in rings:
            rows.append([bm.verts.new((cx + rx * math.cos(i / n * math.tau), cy + ry * math.sin(i / n * math.tau), z)) for i in range(n)])
        wrap = n
    else:
        a0, a1 = arc
        for z, cx, cy, rx, ry in rings:
            rows.append([bm.verts.new((cx + rx * math.cos(a0 + (a1 - a0) * i / n), cy + ry * math.sin(a0 + (a1 - a0) * i / n), z)) for i in range(n + 1)])
        wrap = None
    for a, b in zip(rows[:-1], rows[1:]):
        for i in range(n):
            j = (i + 1) % n if wrap else i + 1
            bm.faces.new((a[i], a[j], b[j], b[i]))
    if arc is None:
        if caps[0]:
            bm.faces.new(rows[0][::-1])
        if caps[1]:
            bm.faces.new(rows[-1])

def brim(bm, r0=0.12, r1=0.33, n=36, rows=7):
    top, bot = [], []
    for k in range(rows):
        t = k / (rows - 1)
        r = r0 + (r1 - r0) * t
        tr, br_ = [], []
        for i in range(n):
            th = i / n * math.tau
            zc = 0.07 * (math.cos(th) ** 2) * t * t - 0.025 * (math.sin(th) ** 2) * t * t
            x, y = r * math.cos(th), r * 1.05 * math.sin(th)
            tr.append(bm.verts.new((x, y, zc + 0.012)))
            br_.append(bm.verts.new((x, y, zc - 0.004)))
        top.append(tr); bot.append(br_)
    for g in (top, bot):
        for a, b in zip(g[:-1], g[1:]):
            for i in range(n):
                j = (i + 1) % n
                bm.faces.new((a[i], a[j], b[j], b[i]))
    for i in range(n):
        j = (i + 1) % n
        bm.faces.new((top[-1][i], top[-1][j], bot[-1][j], bot[-1][i]))

def tri(bm, pts):
    bm.faces.new([bm.verts.new(p) for p in pts])

def spike(bm, pos, direction, r, h):
    d = Vector(direction).normalized()
    bmesh.ops.create_cone(bm, cap_ends=True, cap_tris=False, segments=6, radius1=r, radius2=0.0, depth=h,
                          matrix=T(Vector(pos) + d * h / 2) @ d.to_track_quat('Z', 'Y').to_matrix().to_4x4())

def cowboy(tag, skin, coat, sweat, pants, belt, mask, hat, emblem='skull', beard=False, trim=BRASS):
    root = empty(tag + '_Root', None, (0, 0, 0))
    O = (0, 0, 0)
    # --- body under the coat: sweater torso + turtleneck, leather trousers
    mesh_obj(tag + '_Sweater', sweat, root, O, lambda b: loft(b, [(0.86, 0, 0, .195, .115), (0.98, 0, 0, .18, .108), (1.12, 0, 0, .19, .115), (1.28, 0, 0, .225, .13),
                                                                    (1.42, 0, 0, .25, .125), (1.5, 0, 0, .17, .1), (1.56, 0, 0, .075, .07)]), True)
    mesh_obj(tag + '_Turtleneck', sweat, root, O, lambda b: loft(b, [(1.47, 0, 0, .085, .08), (1.56, 0, 0, .075, .075), (1.64, 0, 0, .068, .068)], 14), True)
    mesh_obj(tag + '_Pelvis', pants, root, O, lambda b: loft(b, [(0.72, 0, 0, .185, .115), (0.86, 0, 0, .195, .12), (0.97, 0, 0, .185, .112)], 16, (False, False)), True)
    # --- long open leather coat with collar, epaulettes
    A0, A1 = math.radians(112), math.radians(428)
    mesh_obj(tag + '_Coat', coat, root, O, lambda b: loft(b, [(1.5, 0, -.005, .27, .155), (1.38, 0, -.005, .275, .15), (1.2, 0, -.01, .262, .148), (1.0, 0, -.01, .235, .138),
                                                               (0.86, 0, -.01, .262, .158), (0.66, 0, -.015, .295, .185), (0.48, 0, -.02, .325, .205)], 24, arc=(A0, A1)), True, 0.014)
    mesh_obj(tag + '_Collar', coat, root, O, lambda b: loft(b, [(1.46, 0, -.012, .115, .108), (1.54, 0, -.016, .118, .11), (1.62, 0, -.022, .135, .125)], 16, arc=(math.radians(135), math.radians(405))), True, 0.012)
    def lapels(b):
        for sx in (-1, 1):
            tri(b, ((sx * .075, .152, 1.46), (sx * .15, .158, 1.38), (sx * .1, .158, 1.0)))
            tri(b, ((sx * .075, .152, 1.46), (sx * .1, .158, 1.0), (sx * .03, .152, 1.0)))
    mesh_obj(tag + '_Lapels', coat, root, O, lapels, False, 0.012)
    def epaulette(b):
        for sx in (-1, 1):
            bx(b, (sx * .21, 0, 1.495), (0.13, 0.085, 0.018))
            cy_(b, (sx * .24, 0, 1.51), 0.013, 0.012, 8)
    mesh_obj(tag + '_Epaulettes', coat, root, O, epaulette)
    # --- belt, buckle, thigh strap, pouch, holster
    mesh_obj(tag + '_Belt', belt, root, O, lambda b: loft(b, [(0.9, 0, 0, .206, .124), (0.99, 0, 0, .206, .124)], 20, (False, False)))
    mesh_obj(tag + '_Buckle', trim, root, O, lambda b: (bx(b, (0, 0.128, 0.945), (0.07, 0.014, 0.055)), bx(b, (0, 0.136, 0.945), (0.04, 0.008, 0.03))))
    mesh_obj(tag + '_Pouch', belt, root, O, lambda b: (bx(b, (-.19, 0.04, 0.88), (0.06, 0.1, 0.1)), bx(b, (-.19, 0.04, 0.935), (0.065, 0.105, 0.02)),
                                                       bx(b, (.0, -.13, 0.88), (0.12, 0.05, 0.1))))
    mesh_obj(tag + '_Holster', belt, root, (0.2, 0.0, 0.0), lambda b: (bx(b, (0, 0.01, 0.78), (0.06, 0.09, 0.24)), bx(b, (0, 0.0, 0.55), (0.07, 0.07, 0.02))))
    def hbutt_(b):
        bx(b, (0, -0.012, 0.94), (0.04, 0.05, 0.1)); bx(b, (0, 0.0, 0.9), (0.03, 0.035, 0.05))
    hbutt = mesh_obj(tag + '_HolsterGun', TRIM, root, (0.2, 0.0, 0.0), hbutt_)
    # --- neck + head
    mesh_obj(tag + '_Neck', skin, root, O, lambda b: loft(b, [(1.6, 0, 0, .055, .055), (1.66, 0, 0, .05, .05)], 12), True)
    def head(b):
        loft(b, [(1.59, 0, .012, .052, .06), (1.62, 0, .012, .076, .084), (1.66, 0, .004, .096, .103), (1.7, 0, 0, .104, .114), (1.75, 0, -.002, .106, .12),
                 (1.8, 0, -.004, .098, .114), (1.84, 0, -.006, .07, .085), (1.865, 0, -.006, .03, .04)], 18)
        bx(b, (0, .12, 1.7), (0.026, 0.045, 0.06))
        bx(b, (-.04, .105, 1.752), (0.05, 0.03, 0.022)); bx(b, (.04, .105, 1.752), (0.05, 0.03, 0.022))   # brow ridge
        sp_(b, (-.108, 0, 1.72), 0.022, (0.5, 1, 1.3)); sp_(b, (.108, 0, 1.72), 0.022, (0.5, 1, 1.3))
    mesh_obj(tag + '_Head', skin, root, O, head, True)
    def eyes(b):
        for sx in (-1, 1):
            sp_(b, (sx * .038, .103, 1.73), 0.013, (1.2, 0.8, 0.9))
    mesh_obj(tag + '_Sclera', SCLERA, root, O, eyes)
    def iris(b):
        for sx in (-1, 1):
            sp_(b, (sx * .038, .113, 1.73), 0.0075)
            bx(b, (sx * .038, .114, 1.755), (0.055, 0.012, 0.012))      # brows
    mesh_obj(tag + '_Iris', EYE, root, O, iris)
    mesh_obj(tag + '_Hair', HAIR, root, O, lambda b: loft(b, [(1.7, 0, -.004, .108, .12), (1.75, 0, -.006, .11, .126), (1.8, 0, -.008, .101, .12), (1.84, 0, -.01, .073, .09)], 18, arc=(math.radians(120), math.radians(420))), True, 0.01)
    # --- face mask (bandana) with knot
    mesh_obj(tag + '_Mask', mask, root, O, lambda b: loft(b, [(1.585, 0, .012, .062, .068), (1.625, 0, .012, .088, .094), (1.67, 0, .004, .108, .116), (1.712, 0, 0, .112, .124)],
                                                          16, arc=(math.radians(-20), math.radians(200))), True, 0.008)
    mesh_obj(tag + '_MaskBack', mask, root, O, lambda b: (loft(b, [(1.66, 0, 0, .109, .12), (1.69, 0, 0, .111, .122)], 12, arc=(math.radians(200), math.radians(340))),
                                                           sp_(b, (0, -.13, 1.67), 0.03), tri(b, ((0, -.14, 1.67), (-.04, -.17, 1.58), (.01, -.17, 1.56)))), True)
    mesh_obj(tag + '_MaskNose', mask, root, O, lambda b: bx(b, (0, .127, 1.69), (0.032, 0.05, 0.05)), True)
    # --- hat: tall crown, wide upturned brim, band, emblem and studs
    mesh_obj(tag + '_HatBrim', hat, root, (0, 0, 1.8), lambda b: brim(b), True)
    mesh_obj(tag + '_HatCrown', hat, root, (0, 0, 1.8), lambda b: loft(b, [(0.0, 0, 0, .155, .17), (0.05, 0, 0, .153, .168), (0.11, 0, 0, .15, .165), (0.16, 0, 0, .14, .155),
                                                                            (0.19, 0, 0, .12, .135), (0.178, 0, 0, .06, .07)], 22, (False, True)), True)
    mesh_obj(tag + '_HatBand', belt, root, (0, 0, 1.8), lambda b: loft(b, [(0.014, 0, 0, .159, .175), (0.065, 0, 0, .157, .173)], 22, (False, False)))
    def studs(b):
        for i in range(11):
            a = math.radians(-60 + i * 30 + 15)
            spike(b, (.162 * math.cos(a), .178 * math.sin(a), 0.1), (math.cos(a), math.sin(a), 0.4), 0.011, 0.04)
    mesh_obj(tag + '_HatStuds', trim, root, (0, 0, 1.8), studs)
    def skull(b):
        sp_(b, (0, .18, 0.04), 0.03, (1.0, 0.55, 1.1)); bx(b, (0, .185, 0.008), (0.03, 0.025, 0.02))
    if emblem == 'skull':
        mesh_obj(tag + '_Skull', trim, root, (0, 0, 1.8), skull)
        mesh_obj(tag + '_SkullEyes', EYE, root, (0, 0, 1.8), lambda b: (sp_(b, (-.011, .205, 0.047), 0.007), sp_(b, (.011, .205, 0.047), 0.007)))
    else:
        mesh_obj(tag + '_Concho', trim, root, (0, 0, 1.8), lambda b: cy_(b, (0, .18, 0.04), 0.03, 0.012, 14))
    P = {}
    for side, sx in (('L', -1), ('R', 1)):
        leg = empty(f'{tag}_{side}Leg', root, (sx * 0.1, 0, 0.9))
        mesh_obj(f'{tag}_{side}Pants', pants, leg, O, lambda b: loft(b, [(0.04, 0, 0, .095, .1), (-0.2, 0, 0, .085, .092), (-0.46, 0, .005, .066, .07), (-0.62, 0, .005, .062, .066)], 14, (False, False)), True)
        def boot(b):
            loft(b, [(-0.5, 0, .005, .068, .072), (-0.58, 0, .005, .072, .078), (-0.8, 0, .008, .066, .072), (-0.86, 0, .012, .064, .07)], 14)
            sp_(b, (0, 0.085, -0.86), 0.052, (1.05, 1.9, 0.9)); bx(b, (0, -0.06, -0.885), (0.1, 0.075, 0.035)); bx(b, (0, 0.0, -0.894), (0.115, 0.22, 0.02))
        mesh_obj(f'{tag}_{side}Boot', belt, leg, O, boot, True)
        mesh_obj(f'{tag}_{side}BootStrap', trim, leg, O, lambda b: (loft(b, [(-0.62, 0, .005, .074, .08), (-0.65, 0, .005, .074, .08)], 14, (False, False)), bx(b, (0, .08, -0.635), (0.03, 0.01, 0.035))))
        mesh_obj(f'{tag}_{side}Spur', trim, leg, O, lambda b: (bx(b, (0, -0.1, -0.84), (0.012, 0.04, 0.012)), cy_(b, (0, -0.125, -0.84), 0.022, 0.006, 10)))
        arm = empty(f'{tag}_{side}Arm', root, (sx * 0.272, 0, 1.43))
        mesh_obj(f'{tag}_{side}Shoulder', coat, arm, O, lambda b: sp_(b, (0, 0, 0.0), 0.08, (1.0, 0.95, 1.0)), True)
        mesh_obj(f'{tag}_{side}Sleeve', coat, arm, O, lambda b: loft(b, [(0.03, 0, 0, .068, .068), (-0.14, 0, 0, .072, .072), (-0.34, 0, 0, .062, .062), (-0.55, 0, 0, .05, .05), (-0.6, 0, 0, .046, .046)], 14, (True, False)), True)
        mesh_obj(f'{tag}_{side}Cuff', belt, arm, O, lambda b: loft(b, [(-0.52, 0, 0, .054, .054), (-0.6, 0, 0, .05, .05)], 14, (False, False)))
        def hand(b):
            sp_(b, (0, 0.008, -0.67), 0.047, (0.9, 0.75, 1.25)); sp_(b, (0.0, 0.03, -0.71), 0.028)
            for k in (-1.5, -0.5, 0.5, 1.5):
                bx(b, (k * 0.0105, 0.04, -0.745), (0.0105, 0.018, 0.04))
        mesh_obj(f'{tag}_{side}Glove', GLOVE, arm, O, hand, True)
        mesh_obj(f'{tag}_{side}Fingertips', skin, arm, O, lambda b: [bx(b, (k * 0.0105, 0.04, -0.775), (0.009, 0.015, 0.016)) for k in (-1.5, -0.5, 0.5, 1.5)])
        P[side + 'Leg'], P[side + 'Arm'] = leg, arm
    arm = P['RArm']
    def revolver(b):
        bx(b, (0, 0, -0.88), (0.022, 0.026, 0.24)); bx(b, (0, -0.002, -0.735), (0.05, 0.05, 0.08))
        bx(b, (0, 0.027, -0.715), (0.012, 0.016, 0.03)); bx(b, (0, -0.04, -0.775), (0.012, 0.045, 0.012))
    gun = mesh_obj(tag + '_Gun', IRON, arm, O, revolver)
    mesh_obj(tag + '_GunGrip', TRIM, gun, O, lambda b: bx(b, (0, -0.058, -0.69), (0.036, 0.04, 0.095)))
    flash = mesh_obj(tag + '_Flash', FLASH, arm, O, lambda b: cy_(b, (0, 0, -1.13), 0.0, 0.25, 8, 0.06))
    ld = bpy.data.lights.new(tag + '_FlashLight', 'POINT')
    ld.energy, ld.color, ld.shadow_soft_size = 0.0, (1.0, 0.65, 0.3), 0.05
    flight = bpy.data.objects.new(tag + '_FlashLight', ld)
    col.objects.link(flight); flight.parent = arm; flight.location = (0, 0.05, -1.2)
    chest = empty(tag + '_Chest', root, (0, 0, 1.4))
    P.update(root=root, gun=gun, flash=flash, light=ld, hbutt=hbutt, chest=chest)
    return P

P1 = cowboy('P1', plain('P1Skin', (0.26, 0.15, 0.1), rough=0.5, noise_s=1), leather('P1Coat', (0.09, 0.05, 0.034), 0.5),
            sweater('P1Sweater', (0.42, 0.35, 0.29), (0.14, 0.09, 0.065)), leather('P1Pants', (0.11, 0.065, 0.045), 0.42),
            leather('P1Belt', (0.07, 0.042, 0.028), 0.45), plain('P1Mask', (0.035, 0.035, 0.04), noise_s=1),
            plain('P1Hat', (0.5, 0.47, 0.4), rough=0.95, noise_s=1), emblem='skull', beard=False, trim=BRASS)
P2 = cowboy('P2', plain('P2Skin', (0.42, 0.26, 0.18), rough=0.5, noise_s=1), leather('P2Coat', (0.075, 0.07, 0.065), 0.55),
            sweater('P2Sweater', (0.36, 0.07, 0.05), (0.12, 0.05, 0.04)), leather('P2Pants', (0.1, 0.085, 0.07), 0.45),
            leather('P2Belt', (0.06, 0.045, 0.035), 0.45), plain('P2Mask', (0.34, 0.04, 0.03), noise_s=1),
            plain('P2Hat', (0.045, 0.04, 0.036), rough=0.95, noise_s=1), emblem='concho', beard=True, trim=SILVER)

def lerp(a, b, t):
    return a + (b - a) * t

def animate(P, sgn, yline):
    """sgn=-1: gunman from the west side (faces +X first); sgn=+1: from the east side (faces -X first)."""
    root = P['root']
    face_in = -math.pi / 2 * (-sgn)          # yaw that looks at the opponent (P1 faces +X = -90deg)
    yaw0 = face_in
    yaw_bb = face_in - math.pi * (-sgn)      # back to back, turning "clockwise" for P1, mirrored for P2
    yaw_end = yaw_bb - math.pi * (-sgn)      # turned round to face the opponent again
    xs = sgn * 0.45
    def pose(f, x, z, phase, la, aa, ar):
        key(root, 'location', f, (x, yline, z))
        s = math.sin(phase)
        key(P['LLeg'], 'rotation_euler', f, (la * s, 0, 0))
        key(P['RLeg'], 'rotation_euler', f, (-la * s, 0, 0))
        key(P['LArm'], 'rotation_euler', f, (-aa * s, 0, 0))
        key(P['RArm'], 'rotation_euler', f, (ar * s, 0, 0))
    # walk in
    n = 143
    cycles = 7
    for f in range(1, F_MEET + 1, 4):
        t = (f - 1) / n
        ph = math.tau * cycles * t
        end = f >= F_MEET
        pose(f, lerp(sgn * START_X, xs, t), 0.03 * abs(math.sin(ph)), ph, 0.55, 0.45, 0.15)
    pose(F_MEET, xs, 0, 0, 0, 0, 0)
    pose(F_W0, xs, 0, 0, 0, 0, 0)
    # ten slow paces
    for f in range(F_W0, F_W1 + 1, 4):
        t = (f - F_W0) / (F_W1 - F_W0)
        ph = math.tau * 5 * t
        pose(f, lerp(xs, sgn * (0.45 + PACE_X), t), 0.02 * abs(math.sin(ph)), ph, 0.42, 0.28, 0.1)
    pose(F_W1 + 6, sgn * (0.45 + PACE_X), 0, 0, 0, 0, 0)
    x_end = sgn * (0.45 + PACE_X)
    # turns
    key(root, 'rotation_euler', 1, (0, 0, yaw0), 'BEZIER')
    key(root, 'rotation_euler', F_T0, (0, 0, yaw0), 'BEZIER')
    key(root, 'rotation_euler', F_T1, (0, 0, yaw_bb), 'BEZIER')
    key(root, 'rotation_euler', F_TING + 6, (0, 0, yaw_bb), 'BEZIER')
    key(root, 'rotation_euler', F_TING + 17, (0, 0, yaw_end), 'BEZIER')
    # tension: the gun hand twitches
    for f, v in ((F_W1 + 6, 0.0), (F_W1 + 34, 0.07), (F_W1 + 52, -0.05), (F_W1 + 70, 0.05), (F_TING, 0.0)):
        key(P['RArm'], 'rotation_euler', f, (v, 0, 0), 'BEZIER')
    # draw + fire + fall
    fs = F_SHOT[P['tag']]
    key(P['RArm'], 'rotation_euler', F_DRAW, (0, 0, 0), 'BEZIER')
    key(P['RArm'], 'rotation_euler', F_DRAW + 9, (math.pi / 2, 0, 0), 'BEZIER')
    key(P['RArm'], 'rotation_euler', fs, (math.pi / 2, 0, 0), 'BEZIER')
    key(P['RArm'], 'rotation_euler', fs + 2, (1.95, 0, 0), 'BEZIER')
    key(P['RArm'], 'rotation_euler', fs + 6, (1.7, 0, 0), 'BEZIER')
    key(P['RArm'], 'rotation_euler', fs + 11, (1.4, 0, 0), 'BEZIER')
    key(P['RArm'], 'rotation_euler', fs + 30, (0.25, 0, 0), 'BEZIER')
    for f, v in ((F_TING + 6, 0.0), (F_TING + 16, -0.5), (fs + 4, -0.3), (fs + 22, -0.9), (fs + 32, -1.2)):
        key(P['LArm'], 'rotation_euler', f, (v, 0, 0), 'BEZIER')
    # holster gun / hand gun swap (stepped)
    key(P['gun'], 'scale', 1, (0, 0, 0), 'CONSTANT')
    key(P['gun'], 'scale', F_DRAW + 3, (1, 1, 1), 'CONSTANT')
    key(P['hbutt'], 'scale', 1, (1, 1, 1), 'CONSTANT')
    key(P['hbutt'], 'scale', F_DRAW + 3, (0, 0, 0), 'CONSTANT')
    # muzzle flash + light
    key(P['flash'], 'scale', 1, (0, 0, 0), 'CONSTANT')
    key(P['flash'], 'scale', fs, (1, 1, 1), 'CONSTANT')
    key(P['flash'], 'scale', fs + 2, (0, 0, 0), 'CONSTANT')
    interp('CONSTANT')
    for f, v in ((1, 0.0), (fs, 1800.0), (fs + 2, 0.0)):
        P['light'].energy = v
        P['light'].keyframe_insert('energy', frame=f)
    # hit -> stagger -> fall backwards (away from the opponent)
    back = -sgn * -1     # direction of falling, world x
    fall_dir = sgn       # P1 (west, sgn=-1) falls toward -X, P2 toward +X
    yaw = yaw_end
    key(root, 'location', fs + 3, (x_end, yline, 0), 'BEZIER')
    key(root, 'location', fs + 11, (x_end + fall_dir * 0.35, yline, 0.0), 'BEZIER')
    key(root, 'location', fs + 28, (x_end + fall_dir * 0.95, yline, 0.14), 'BEZIER')
    key(root, 'rotation_euler', fs + 3, (0, 0, yaw), 'BEZIER')
    key(root, 'rotation_euler', fs + 11, (0.45, 0, yaw), 'BEZIER')
    key(root, 'rotation_euler', fs + 28, (1.46, 0, yaw), 'BEZIER')
    for leg, v in (('LLeg', 0.2), ('RLeg', -0.15)):
        key(P[leg], 'rotation_euler', fs + 3, (0, 0, 0), 'BEZIER')
        key(P[leg], 'rotation_euler', fs + 28, (v, 0, 0), 'BEZIER')

P1['tag'], P2['tag'] = 'P1', 'P2'
animate(P1, -1, -0.12)
animate(P2, +1, +0.12)

def fcurves_of(idb):
    ad = idb.animation_data
    if not ad or not ad.action:
        return []
    a = ad.action
    if hasattr(a, 'fcurves'):
        return list(a.fcurves)
    out = []
    for layer in a.layers:
        for st in layer.strips:
            cb = st.channelbag(ad.action_slot) if ad.action_slot else None
            if cb:
                out += list(cb.fcurves)
    return out

def set_interp():
    for P in (P1, P2):
        for k in ('gun', 'hbutt', 'flash'):
            for fc in fcurves_of(P[k]):
                for kp in fc.keyframe_points:
                    kp.interpolation = 'CONSTANT'
        for fc in fcurves_of(P['light']):
            for kp in fc.keyframe_points:
                kp.interpolation = 'CONSTANT'
        for k in ('root', 'LLeg', 'RLeg', 'LArm', 'RArm'):
            for fc in fcurves_of(P[k]):
                for kp in fc.keyframe_points:
                    walking = kp.co[0] <= F_W1 + 6 or kp.co[0] < F_MEET + 1
                    smooth = fc.data_path == 'rotation_euler' and k == 'root'
                    kp.interpolation = 'BEZIER' if (smooth or not walking) else 'LINEAR'
                    if smooth or not walking:
                        kp.handle_left_type = kp.handle_right_type = 'AUTO_CLAMPED'

# ---- tumbleweed
def tube_bm(bm, p0, p1, r, seg=4):
    d = Vector(p1) - Vector(p0)
    if d.length < 1e-4:
        return
    bmesh.ops.create_cone(bm, cap_ends=True, cap_tris=False, segments=seg, radius1=r, radius2=r, depth=d.length,
                          matrix=T((Vector(p0) + Vector(p1)) / 2) @ d.to_track_quat('Z', 'Y').to_matrix().to_4x4())
TW = plain('Tumbleweed', (0.5, 0.37, 0.2), rough=0.95, noise_s=1)
def tumble(bm):
    for _ in range(26):
        ax = Vector((random.gauss(0, 1), random.gauss(0, 1), random.gauss(0, 1))).normalized()
        u = ax.cross(Vector((0.3, 0.8, 0.5))).normalized()
        v = ax.cross(u).normalized()
        rr = random.uniform(0.25, 0.5)
        off = Vector((random.uniform(-0.05, 0.05), random.uniform(-0.05, 0.05), random.uniform(-0.05, 0.05)))
        pts = [off + (u * math.cos(i / 14 * math.tau) + v * math.sin(i / 14 * math.tau)) * rr * (1 + 0.08 * math.sin(i * 2.3)) for i in range(15)]
        for a, b in zip(pts[:-1], pts[1:]):
            tube_bm(bm, a, b, 0.011)
    for _ in range(60):
        d = Vector((random.gauss(0, 1), random.gauss(0, 1), random.gauss(0, 1))).normalized()
        tube_bm(bm, d * random.uniform(0.0, 0.2), d * random.uniform(0.3, 0.52), 0.008, 3)
tw = mesh_obj('Tumbleweed', TW, None, (0, 0, 0), tumble, True)
T_X0, T_F0, T_F1, T_CROSS, T_SPEED = -34, 332, 688, 500, 0.19
for f in list(range(T_F0, T_F1 + 1, 4)):
    x = (f - T_CROSS) * T_SPEED
    y = -1.8 + 0.45 * math.sin((f - T_F0) / 55.0)
    z = 0.5 + 0.3 * abs(math.sin(math.pi * (f - T_F0) / 38.0))
    key(tw, 'location', f, (x, y, z))
    key(tw, 'rotation_euler', f, (0.3 * math.sin(f / 31.0), x / 0.5, 0.2 * f / 40.0))

for fc in fcurves_of(tw):
    for kp in fc.keyframe_points:
        kp.interpolation = 'LINEAR'
set_interp()

# ---- cameras (cuts via timeline markers)
tmid = empty('T_Mid', None, (0, 0, 1.25))
def track(c, target):
    k = c.constraints.new('TRACK_TO')
    k.target, k.track_axis, k.up_axis = target, 'TRACK_NEGATIVE_Z', 'UP_Y'
shots = [
    ('Shot1_Approach', 1, (0, -12.5, 1.3), tmid, 24),
    ('Shot2_BackToBack', F_MEET, (3.6, -3.7, 1.5), tmid, 50),
    ('Shot3_Paces', F_W0, (0, -16, 1.0), tmid, 28),
    ('Shot4_FollowP1', 340, (-5.5, -5.0, 1.3), P1['chest'], 40),
    ('Shot5_P1Hand', 456, (-8.85, 2.4, 0.9), None, 55),
    ('Shot6_P2Hand', 478, (8.85, -2.4, 0.9), None, 55),
    ('Shot7_Tension', 496, (0, -12.5, 1.2), tmid, 24),
    ('Shot8_Crane', 600, (0, -16, 1.0), tmid, 28),
]
hand_t = {'Shot5_P1Hand': empty('T_P1Hand', None, (-8.85, 0.29, 0.8)), 'Shot6_P2Hand': empty('T_P2Hand', None, (8.85, -0.29, 0.8))}
for name, f, loc, target, lens in shots:
    c = cam(name, loc, (0, 0, 1.2), lens=lens)
    track(c, target or hand_t[name])
    m = scene.timeline_markers.new(name, frame=f)
    m.camera = c
    if name == 'Shot2_BackToBack':
        key(c, 'location', F_MEET, (3.6, -3.7, 1.5), 'BEZIER')
        key(c, 'location', F_W0 - 1, (2.4, -2.5, 1.45), 'BEZIER')
    if name == 'Shot8_Crane':
        key(c, 'location', 600, (0, -16, 1.0), 'BEZIER')
        key(c, 'location', 720, (0, -25, 7.5), 'BEZIER')
scene.camera = bpy.data.objects['Shot1_Approach']

# ---- sound: wind bed, the "ting", two gunshots
AUD = os.path.join(OUT, 'audio')
os.makedirs(AUD, exist_ok=True)
SR = 44100
def write_wav(name, data):
    pcm = (np.clip(data, -1, 1) * 32767).astype('<i2')
    path = os.path.join(AUD, name)
    with wave.open(path, 'wb') as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(SR)
        w.writeframes(pcm.tobytes())
    return path
rng = np.random.default_rng(5)
tt = np.arange(int(SR * 3.2)) / SR
ting = sum(a * np.sin(math.tau * f * tt) * np.exp(-dk * tt) for f, a, dk in [(1318, 1.0, 1.7), (2637, 0.5, 2.6), (3951, 0.22, 4.0), (1760, 0.3, 1.9), (5300, 0.1, 6.0)]) * 0.35
ting[:90] *= np.linspace(0, 1, 90)
def shot_wav(seed):
    r = np.random.default_rng(seed)
    t = np.arange(int(SR * 2.4)) / SR
    n = r.normal(0, 1, t.size)
    n = np.convolve(n, np.ones(6) / 6, 'same')
    body = n * np.exp(-t * 22) + 0.9 * np.sin(math.tau * 62 * t) * np.exp(-t * 14)
    out = body.copy()
    for dly, g in ((0.22, 0.4), (0.47, 0.22), (0.8, 0.1)):
        k = int(dly * SR)
        out[k:] += g * np.convolve(body[:t.size - k], np.ones(300) / 300, 'same')
    return out / np.abs(out).max() * 0.9
wl = np.arange(SR * 30)
wind = np.convolve(rng.normal(0, 1, wl.size), np.ones(900) / 900, 'same') * (0.55 + 0.45 * np.sin(math.tau * wl / SR / 9.0))
wind = wind / np.abs(wind).max() * 0.5
paths = {k: write_wav(k + '.wav', v) for k, v in dict(wind=wind, ting=ting, shot1=shot_wav(1), shot2=shot_wav(2)).items()}
se = scene.sequence_editor_create()
strips = se.strips if hasattr(se, 'strips') else se.sequences
strips.new_sound('wind', paths['wind'], 1, 1)
strips.new_sound('ting', paths['ting'], 2, F_TING)
strips.new_sound('shot_P1', paths['shot1'], 3, F_SHOT['P1'])
strips.new_sound('shot_P2', paths['shot2'], 4, F_SHOT['P2'])

# ---- numeric sanity check of the choreography (no rendering)
def snap(f):
    scene.frame_set(f)
    out = {}
    for tag, P in (('P1', P1), ('P2', P2)):
        mw = P['root'].matrix_world
        fwd = mw.to_3x3() @ Vector((0, 1, 0))
        head = mw @ Vector((0, 0, 1.75))
        out[tag] = (round(mw.translation.x, 2), round(math.degrees(math.atan2(fwd.y, fwd.x))), round(head.z, 2))
    return out
for f in (1, 100, 144, 200, 300, 456, 530, 546, 556, 563, 580, 600, 720):
    print('ANIM f%d' % f, snap(f), 'tumble x=%.1f' % (bpy.data.objects['Tumbleweed'].matrix_world.translation.x if scene.frame_set(f) is None else 0))
scene.frame_set(1)
if os.environ.get('CHAR_TEST') == '1':
    scene.render.engine = 'BLENDER_EEVEE'
    scene.render.resolution_x, scene.render.resolution_y = 1280, 720
    scene.render.resolution_percentage = 100
    saved = [(m, m.camera) for m in scene.timeline_markers]
    for m, _ in saved:
        m.camera = None
    scene.frame_set(150)
    c1 = cam('Tmp_Both', (0, -3.6, 1.05), (0, 0, 1.0), lens=34)
    scene.camera = c1
    scene.render.filepath = os.path.join(OUT, '_char_both.png')
    bpy.ops.render.render(write_still=True)
    c2 = cam('Tmp_Close', (0.15, -1.7, 1.72), (-0.45, 0, 1.62), lens=70)
    scene.camera = c2
    scene.render.filepath = os.path.join(OUT, '_char_close.png')
    bpy.ops.render.render(write_still=True)
    for m, c in saved:
        m.camera = c
    scene.frame_set(1)
    scene.camera = bpy.data.objects['Shot1_Approach']
    bpy.data.objects.remove(c1); bpy.data.objects.remove(c2)


# =============================================================== render settings + previews
ENGINE = os.environ.get('ENGINE', 'BLENDER_EEVEE')
scene.render.engine = ENGINE
scene.render.resolution_x, scene.render.resolution_y = 1920, 1080
scene.render.resolution_percentage = int(os.environ.get('RES', '100'))
if ENGINE == 'CYCLES':
    cy = scene.cycles
    cy.device = 'CPU'
    cy.samples = int(os.environ.get('SAMPLES', '96'))
    cy.use_adaptive_sampling = True
    cy.use_denoising = True
    try:
        cy.denoiser = 'OPENIMAGEDENOISE'
    except Exception:
        pass
    cy.max_bounces = 6
    cy.volume_bounces = 1
    cy.volume_step_rate = 2.0
try:
    scene.view_settings.view_transform = 'Khronos PBR Neutral'
except Exception:
    scene.view_settings.view_transform = 'AgX'
scene.view_settings.exposure = -0.5
try:
    scene.view_settings.look = 'AgX - Medium High Contrast'
except Exception:
    pass
for k, v in dict(taa_render_samples=48, use_gtao=True, gtao_distance=1.2, use_shadows=True, shadow_ray_count=2,
                 use_raytracing=True).items():
    try:
        setattr(scene.eevee, k, v)
    except Exception:
        pass

def render(c, fn):
    scene.camera = c
    scene.render.filepath = os.path.join(OUT, fn)
    bpy.ops.render.render(write_still=True)

if os.environ.get("RENDER") == "1":
    only = os.environ.get("CAMS")
    for c, fn in ((top, "map_topdown.png"), (over, "map_overview.png"), (p1, "map_player1_view.png"),
                  (hero, "map_hero.png"), (low, "map_saloon_corner.png")):
        if not only or fn in only.split(","):
            render(c, fn)

scene.camera = over
bpy.ops.wm.save_as_mainfile(filepath=os.path.join(OUT, "CowboyMap.blend"))
bpy.ops.export_scene.gltf(filepath=os.path.join(OUT, "CowboyMap.glb"), export_format='GLB', export_apply=True)
print("DONE")
