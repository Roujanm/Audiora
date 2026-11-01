"""Cowboy duel map: one western town, two player spawns at opposite ends of the main street.
Run: D:\\blender\\blender.exe -b -P build_map.py
Units are metres. Main street runs along X. Spawn A at x=-34, Spawn B at x=+34.
"""
import bpy, math, random, os
from mathutils import Vector

OUT = os.path.dirname(os.path.abspath(__file__))
random.seed(7)

bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene
scene.render.engine = 'BLENDER_EEVEE'
scene.render.resolution_x, scene.render.resolution_y = 1600, 1000
scene.view_settings.view_transform = 'Standard'

col = bpy.data.collections.new("CowboyMap")
scene.collection.children.link(col)

_mats = {}
def mat(name, rgb, rough=0.9):
    if name not in _mats:
        m = bpy.data.materials.new(name)
        m.use_nodes = True
        b = m.node_tree.nodes["Principled BSDF"]
        b.inputs["Base Color"].default_value = (*rgb, 1)
        b.inputs["Roughness"].default_value = rough
        _mats[name] = m
    return _mats[name]

SAND = mat("Sand", (0.62, 0.45, 0.26))
DIRT = mat("Dirt", (0.42, 0.29, 0.17))
WOOD = mat("Wood", (0.36, 0.2, 0.09))
WOOD2 = mat("WoodLight", (0.55, 0.36, 0.18))
WOOD3 = mat("WoodRed", (0.5, 0.17, 0.1))
ROOF = mat("Roof", (0.22, 0.12, 0.07))
STONE = mat("Stone", (0.45, 0.33, 0.24))
CACT = mat("Cactus", (0.16, 0.34, 0.14))
METAL = mat("Metal", (0.12, 0.12, 0.13), 0.5)
WHITE = mat("White", (0.85, 0.82, 0.74))
GLASS = mat("Window", (0.05, 0.08, 0.1), 0.2)

def link(o):
    for c in o.users_collection:
        c.objects.unlink(o)
    col.objects.link(o)
    return o

def box(name, loc, size, m, rot=0.0):
    """Box centred in x/y, sitting on loc.z (loc is the bottom centre)."""
    bpy.ops.mesh.primitive_cube_add(size=1)
    o = bpy.context.object
    o.name = name
    o.scale = size
    o.location = (loc[0], loc[1], loc[2] + size[2] / 2)
    o.rotation_euler[2] = rot
    o.data.materials.append(m)
    return link(o)

def cyl(name, loc, r, h, m, verts=16):
    bpy.ops.mesh.primitive_cylinder_add(radius=r, depth=h, vertices=verts)
    o = bpy.context.object
    o.name = name
    o.location = (loc[0], loc[1], loc[2] + h / 2)
    o.data.materials.append(m)
    return link(o)

def cone(name, loc, r, h, m, verts=6):
    bpy.ops.mesh.primitive_cone_add(radius1=r, radius2=0, depth=h, vertices=verts)
    o = bpy.context.object
    o.name = name
    o.location = (loc[0], loc[1], loc[2] + h / 2)
    o.data.materials.append(m)
    return link(o)

# ---------------------------------------------------------------- ground
box("Ground", (0, 0, -1), (140, 140, 1), SAND)
box("MainStreet", (0, 0, 0), (76, 9, 0.05), DIRT)
box("CrossStreet", (0, 0, 0), (9, 44, 0.05), DIRT)

# ---------------------------------------------------------------- buildings
def building(name, x, y, w, d, h, wall, face, false_front=True):
    """face = +1 door faces +Y (north side buildings look at the street means face -1)."""
    box(f"{name}_Body", (x, y, 0), (w, d, h), wall)
    box(f"{name}_Roof", (x, y, h), (w + 0.6, d + 0.6, 0.25), ROOF)
    if false_front:
        box(f"{name}_FalseFront", (x, y - face * (d / 2 - 0.15), h), (w, 0.3, 2.2), wall)
    fy = y - face * (d / 2 + 0.01)
    box(f"{name}_Door", (x, fy - face * 0.05, 0), (1.3, 0.12, 2.4), WOOD)
    for dx in (-w / 3, w / 3):
        box(f"{name}_Window", (x + dx, fy - face * 0.05, 1.2), (1.2, 0.12, 1.2), GLASS)
    # porch
    box(f"{name}_Porch", (x, fy - face * 1.6, 0), (w, 3.2, 0.3), WOOD2)
    for dx in (-w / 2 + 0.3, w / 2 - 0.3):
        box(f"{name}_PorchPost", (x + dx, fy - face * 3.0, 0.3), (0.25, 0.25, 2.6), WOOD)
    box(f"{name}_PorchRoof", (x, fy - face * 1.7, 2.9), (w, 3.4, 0.2), ROOF)

# north side (y>0, doors face -Y toward street => face=+1 means front toward -Y)
building("Saloon",    -16, 10, 12, 9, 6.0, WOOD3, +1)
building("Bank",        0, 10, 8, 8, 5.5, STONE, +1)
building("Sheriff",    14, 10, 9, 8, 5.0, WOOD2, +1)
building("Hotel",      28, 11, 10, 10, 7.0, WOOD, +1)
# south side
building("GeneralStore", -26, -10, 9, 8, 5.0, WOOD2, -1)
building("Barber",       -12, -10, 7, 7, 4.8, WOOD, -1)
building("Undertaker",   12, -10, 7, 7, 4.8, WOOD3, -1)
building("Stable",       26, -11, 11, 9, 5.5, WOOD, -1)

# church on the cross street (north end)
box("Church_Body", (0, 26, 0), (9, 12, 6), WHITE)
box("Church_Roof", (0, 26, 6), (9.6, 12.6, 0.3), ROOF)
box("Church_Tower", (0, 31, 6), (3.2, 3.2, 5), WHITE)
cone("Church_Spire", (0, 31, 11), 2.4, 4.0, ROOF, 4)
box("Church_Door", (0, 19.9, 0), (1.8, 0.2, 3), WOOD)

# ---------------------------------------------------------------- town centre props
cyl("Well_Wall", (0, 0, 0), 1.6, 1.1, STONE, 20)
for sx in (-1.4, 1.4):
    box("Well_Post", (sx, 0, 1.1), (0.2, 0.2, 2.2), WOOD)
box("Well_Beam", (0, 0, 3.3), (3.4, 0.25, 0.25), WOOD)
box("Well_Roof", (0, 0, 3.5), (3.8, 2.4, 0.2), ROOF)

# water tower
for lx in (-1.6, 1.6):
    for ly in (-1.6, 1.6):
        box("Tower_Leg", (22 + lx, -24 + ly, 0), (0.35, 0.35, 8), WOOD)
cyl("Tower_Tank", (22, -24, 8), 2.6, 3.6, WOOD, 24)
cone("Tower_Cap", (22, -24, 11.6), 2.8, 1.6, ROOF, 24)

# windmill-ish tall sign for orientation: saloon sign
box("Saloon_Sign", (-16, 5.2, 6.2), (6, 0.2, 1.4), WOOD2)

# ---------------------------------------------------------------- cover (waist-high, for duelling)
def barrel(x, y):
    cyl("Barrel", (x, y, 0), 0.5, 1.0, WOOD)
def crate(x, y, s=1.1, r=0.0):
    box("Crate", (x, y, 0), (s, s, s), WOOD2, r)
def trough(x, y, r=0):
    box("Trough", (x, y, 0), (3.0, 0.8, 0.8), WOOD, r)
def wagon(x, y, r=0):
    box("Wagon_Bed", (x, y, 0.8), (4.0, 2.0, 0.5), WOOD2, r)
    for wx in (-1.4, 1.4):
        for wy in (-1.1, 1.1):
            bpy.ops.mesh.primitive_cylinder_add(radius=0.8, depth=0.2, vertices=16, rotation=(math.pi / 2, 0, r))
            w = bpy.context.object
            w.name = "Wagon_Wheel"
            c, s = math.cos(r), math.sin(r)
            w.location = (x + wx * c - wy * s, y + wx * s + wy * c, 0.8)
            w.data.materials.append(WOOD)
            link(w)

# mid-street cover so players aren't in a straight clear lane
wagon(-8, 1.5, 0.15)
wagon(9, -2.0, -0.2)
trough(-18, -3.5)
trough(17, 3.5)
for p in [(-5, -3), (-4.2, -3.8), (5, 3), (4.2, 3.9), (-24, 2.5), (24, -2.5)]:
    barrel(*p)
for p in [(-12, 3.3), (12.8, -3.3), (-29, -3.0), (30, 3.0), (-3, 4.2), (3, -4.2)]:
    crate(*p, r=random.uniform(0, 1))

# hitching posts along the street edges
for x in range(-30, 32, 6):
    for y in (-4.1, 4.1):
        box("HitchPost", (x, y, 0), (0.2, 0.2, 1.3), WOOD)
        box("HitchRail", (x + 1.2, y, 1.1), (2.6, 0.12, 0.12), WOOD)

# ---------------------------------------------------------------- spawns
def spawn(name, x, rgb):
    m = mat(name, rgb, 0.4)
    m.node_tree.nodes["Principled BSDF"].inputs["Emission Color"].default_value = (*rgb, 1)
    m.node_tree.nodes["Principled BSDF"].inputs["Emission Strength"].default_value = 1.5
    cyl(name + "_Pad", (x, 0, 0.05), 1.6, 0.1, m, 32)
    e = bpy.data.objects.new(name, None)
    e.empty_display_type = 'ARROWS'
    e.empty_display_size = 1.5
    e.location = (x, 0, 0.1)
    e.rotation_euler[2] = 0 if x < 0 else math.pi   # face the other player
    col.objects.link(e)

spawn("Spawn_Player1", -34, (0.1, 0.35, 1.0))
spawn("Spawn_Player2", 34, (1.0, 0.15, 0.1))

# fence + rock walls closing the street ends (so nobody leaves the map)
for x in (-39.5, 39.5):
    for y in range(-6, 7, 2):
        box("EndFence", (x, y, 0), (0.25, 1.9, 1.6), WOOD)
    box("EndRailTop", (x, 0, 1.2), (0.2, 13, 0.2), WOOD)

# ---------------------------------------------------------------- nature outside the town
def cactus(x, y):
    h = random.uniform(2.0, 3.5)
    cyl("Cactus", (x, y, 0), 0.3, h, CACT, 10)
    cyl("CactusArmL", (x - 0.55, y, h * 0.5), 0.18, 0.9, CACT, 8)
    cyl("CactusArmR", (x + 0.55, y, h * 0.65), 0.18, 0.9, CACT, 8)

def rock(x, y, s):
    bpy.ops.mesh.primitive_ico_sphere_add(radius=s, subdivisions=1)
    o = bpy.context.object
    o.name = "Rock"
    o.scale = (1, random.uniform(0.7, 1.2), random.uniform(0.6, 1.0))
    o.rotation_euler = (random.random(), random.random(), random.random())
    o.location = (x, y, s * 0.4)
    o.data.materials.append(STONE)
    link(o)

def free(x, y):
    # keep the town (street, buildings, cross street) clear
    return not (abs(x) < 43 and abs(y) < 16) and not (abs(x) < 8 and abs(y) < 36)

n = 0
while n < 40:
    x, y = random.uniform(-60, 60), random.uniform(-60, 60)
    if free(x, y):
        cactus(x, y); n += 1
n = 0
while n < 30:
    x, y = random.uniform(-62, 62), random.uniform(-62, 62)
    if free(x, y):
        rock(x, y, random.uniform(0.8, 2.2)); n += 1

# mesas ringing the map
for i in range(14):
    a = i / 14 * math.tau + random.uniform(-0.1, 0.1)
    r = random.uniform(66, 72)
    h = random.uniform(10, 22)
    box("Mesa", (math.cos(a) * r, math.sin(a) * r, 0), (random.uniform(10, 18), random.uniform(8, 14), h), STONE, a)

# ---------------------------------------------------------------- light + cameras
sun = bpy.data.lights.new("Sun", 'SUN')
sun.energy = 3.0
so = bpy.data.objects.new("Sun", sun)
so.rotation_euler = (math.radians(50), math.radians(10), math.radians(-35))
col.objects.link(so)
w = bpy.data.worlds.new("Sky"); scene.world = w
w.use_nodes = True
w.node_tree.nodes["Background"].inputs[0].default_value = (0.45, 0.62, 0.85, 1)
w.node_tree.nodes["Background"].inputs[1].default_value = 1.0

def cam(name, loc, target, lens=35, ortho=None):
    cd = bpy.data.cameras.new(name)
    cd.lens = lens
    if ortho:
        cd.type = 'ORTHO'; cd.ortho_scale = ortho
    co = bpy.data.objects.new(name, cd)
    co.location = loc
    col.objects.link(co)
    d = Vector(target) - Vector(loc)
    co.rotation_euler = d.to_track_quat('-Z', 'Y').to_euler()
    return co

top = cam("Cam_TopDown", (0, 0, 120), (0, 0, 0), ortho=96)
persp = cam("Cam_Overview", (-52, -62, 38), (0, 0, 2), lens=30)
street = cam("Cam_Player1View", (-34, 0, 1.7), (34, 0, 1.7), lens=28)

# ---------------------------------------------------------------- render previews + export
for c, fn in ((top, "map_topdown.png"), (persp, "map_overview.png"), (street, "map_street.png")):
    scene.camera = c
    scene.render.filepath = os.path.join(OUT, fn)
    bpy.ops.render.render(write_still=True)

scene.camera = persp
bpy.ops.wm.save_as_mainfile(filepath=os.path.join(OUT, "CowboyMap.blend"))
bpy.ops.export_scene.gltf(filepath=os.path.join(OUT, "CowboyMap.glb"), export_format='GLB')
print("DONE")
