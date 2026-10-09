import re, sys
data = open(sys.argv[1], 'rb').read()
out = sys.argv[2]
with open(out + '_ascii.txt', 'w', encoding='utf-8') as f:
    for m in re.finditer(rb'[\x20-\x7e]{4,}', data):
        f.write('%08x\t%s\n' % (m.start(), m.group().decode('ascii')))
with open(out + '_utf16.txt', 'w', encoding='utf-8') as f:
    for m in re.finditer(rb'(?:[\x20-\x7e]\x00){4,}', data):
        f.write('%08x\t%s\n' % (m.start(), m.group().decode('utf-16le')))
