# Extracts every RT_GROUP_ICON from a PE file as a .ico: ico_extract.py <pe> <outdir>
import struct, sys, os
data = open(sys.argv[1], 'rb').read()
out = sys.argv[2]; os.makedirs(out, exist_ok=True)
pe = struct.unpack_from('<I', data, 0x3c)[0]
nsec, optsz = struct.unpack_from('<H', data, pe + 6)[0], struct.unpack_from('<H', data, pe + 20)[0]
magic = struct.unpack_from('<H', data, pe + 24)[0]
ddir = pe + 24 + (96 if magic == 0x10b else 112)
rsrc_rva = struct.unpack_from('<I', data, ddir + 2 * 8)[0]
secs = [struct.unpack_from('<IIII', data, pe + 24 + optsz + i * 40 + 8) for i in range(nsec)]
def off(rva):
    for vsize, va, rsize, raw in secs:
        if va <= rva < va + max(vsize, rsize): return raw + rva - va
    raise ValueError(hex(rva))
base = off(rsrc_rva)
def entries(o):
    named, ids = struct.unpack_from('<HH', data, o + 12)
    for i in range(named + ids):
        name, ptr = struct.unpack_from('<II', data, o + 16 + i * 8)
        yield name, ptr
def leaves(o):
    for name, ptr in entries(o):
        if ptr & 0x80000000: yield from ((name, *rest) for rest in leaves(base + (ptr & 0x7fffffff)))
        else:
            rva, size = struct.unpack_from('<II', data, base + ptr)
            yield (name, data[off(rva): off(rva) + size])
types = {name: base + (ptr & 0x7fffffff) for name, ptr in entries(base)}
icons = {t[0]: t[-1] for t in leaves(types[3])}          # RT_ICON by id
for t in leaves(types[14]):                               # RT_GROUP_ICON
    gid, grp = t[0], t[-1]
    count = struct.unpack_from('<H', grp, 4)[0]
    hdr = grp[:6]; body = b''; pos = 6 + 16 * count
    for i in range(count):
        e = grp[6 + i * 14: 6 + i * 14 + 14]
        iid = struct.unpack_from('<H', e, 12)[0]; img = icons[iid]
        hdr += e[:8] + struct.pack('<II', len(img), pos); body += img; pos += len(img)
    name = os.path.join(out, 'app_%s.ico' % (gid & 0xffff))
    open(name, 'wb').write(hdr + body)
    print(name, count, 'images', [ (grp[6+i*14] or 256) for i in range(count)])
