# usage: fn.py show <addr>... [--max N] | str <regex> | callers <addr> | callees <addr>
import sys, re, os, pickle
D = os.path.join(os.path.dirname(__file__), '..', 'out', 'MacsFanControl.exe')
def index():
    p = os.path.join(D, 'index.pkl')
    if os.path.exists(p): return pickle.load(open(p, 'rb'))
    idx = {}; data = open(os.path.join(D, 'decomp.c'), 'rb').read()
    ms = list(re.finditer(rb'//==== FUNC ([0-9a-f]+) (\S+)', data))
    for i, m in enumerate(ms):
        idx[m.group(1).decode()] = (m.start(), ms[i + 1].start() if i + 1 < len(ms) else len(data))
    pickle.dump(idx, open(p, 'wb')); return idx
def funcs():
    return [l.rstrip('\n').split('\t') for l in open(os.path.join(D, 'functions.tsv'), encoding='utf-8')]
cmd, args = sys.argv[1], sys.argv[2:]
sys.stdout.reconfigure(encoding='utf-8', errors='replace')
if cmd == 'show':
    mx = 400
    if '--max' in args: i = args.index('--max'); mx = int(args[i + 1]); del args[i:i + 2]
    idx = index(); f = open(os.path.join(D, 'decomp.c'), 'rb')
    for a in args:
        a = a.lower().replace('0x', '').zfill(8)
        if a not in idx: print('// no function', a); continue
        s, e = idx[a]; f.seek(s); lines = f.read(e - s).decode('utf-8', 'replace').splitlines()
        lines = [l for l in lines if l.strip()]
        print('\n'.join(lines[:mx])); 
        if len(lines) > mx: print('// ... %d more lines' % (len(lines) - mx))
elif cmd == 'str':
    rx = re.compile(args[0], re.I)
    for l in open(os.path.join(D, 'strings.tsv'), encoding='utf-8'):
        a, refs, v = l.rstrip('\n').split('\t', 2)
        if rx.search(v): print(a, refs or '-', v[:150])
elif cmd == 'callers':
    t = args[0].lower().zfill(8)
    print(' '.join(r[0] + ':' + r[1] for r in funcs() if len(r) > 4 and t in r[4].split(',')))
elif cmd == 'callees':
    t = args[0].lower().zfill(8)
    for r in funcs():
        if r[0] == t: print(r[4].replace(',', ' '))
