# Extracts Qt resource (rcc) trees embedded in a PE file.
import struct, sys, zlib, os, re
data = open(sys.argv[1], 'rb').read()
outdir = sys.argv[2]

def u16(o): return struct.unpack_from('>H', data, o)[0]
def u32(o): return struct.unpack_from('>I', data, o)[0]

def name_at(names, off):
    n = u16(names + off)
    return data[names + off + 6: names + off + 6 + n * 2].decode('utf-16-be')

def try_tree(tree, names, node_size):
    files = []
    def walk(idx, path, depth):
        o = tree + idx * node_size
        name_off, flags = u32(o), u16(o + 4)
        name = name_at(names, name_off) if idx else ''
        if flags & 2:
            count, first = u32(o + 6), u32(o + 10)
            if count > 5000 or depth > 12: raise ValueError('bad dir')
            for i in range(count):
                walk(first + i, path + [name] if idx else path, depth + 1)
        else:
            files.append(('/'.join(path + [name]), flags, u32(o + 10)))
    walk(0, [], 0)
    return files

roots = [m.start() for m in re.finditer(rb'\x00\x00\x00\x00\x00\x02\x00\x00\x00[\x01-\x20]\x00\x00\x00\x01', data)]
done = 0
for tree in roots:
    for node_size in (22, 14):
        # the names blob is somewhere nearby; try every plausible start within 4MB before/after
        o = tree + 6
        first_name_off = u32(tree + node_size)  # child 1's name offset
        for m in re.finditer(rb'\x00[\x01-\x40][\x00-\xff]{4}(?:\x00[\x20-\x7e]){2,}', data[max(0, tree - 0x400000): tree + 0x400000]):
            names = max(0, tree - 0x400000) + m.start() - first_name_off
            if names < 0: continue
            try:
                files = try_tree(tree, names, node_size)
            except Exception:
                continue
            if not files or not all(re.fullmatch(r'[\w@. /+-]+', f[0]) for f in files): continue
            globals().setdefault('found', []).append((tree, names, node_size, files))
            break
        else:
            continue
        break

# data blob: each file is u32 len + payload at data_base+offset; locate data_base via a PNG file
for tree, names, node_size, files in globals().get('found', []):
    base = None
    pngs = [f for f in files if f[0].endswith('.png') and not f[1] & 1]
    if not pngs: continue
    offs0 = pngs[0][2]
    for m in re.finditer(rb'\x89PNG\r\n\x1a\n', data):
        cand = m.start() - 4 - offs0
        if cand < 0: continue
        if all(data[cand + f[2] + 4: cand + f[2] + 8] == b'\x89PNG' for f in pngs[:6]):
            base = cand; break
    if base is None: continue
    for path, flags, off in files:
        n = u32(base + off)
        blob = data[base + off + 4: base + off + 4 + n]
        if flags & 4: path += '.zst'
        elif flags & 1: blob = zlib.decompress(blob[4:])
        p = os.path.join(outdir, path)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        open(p, 'wb').write(blob); done += 1
print('extracted', done)
