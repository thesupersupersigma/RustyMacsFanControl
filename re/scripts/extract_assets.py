# Builds app/assets from an installed copy of Macs Fan Control:
#   python -I re/scripts/extract_assets.py "<folder containing MacsFanControl.exe>" app/assets
# The icons, sensor tables and English strings belong to CrystalIDEA and are not kept
# in this repository; this pulls them out of the app's own executable.
import gzip, os, shutil, subprocess, sys, tempfile

here = os.path.dirname(os.path.abspath(__file__))
source, target = sys.argv[1], sys.argv[2]
exe = source if source.lower().endswith('.exe') else os.path.join(source, 'MacsFanControl.exe')
if not os.path.isfile(exe):
    sys.exit('MacsFanControl.exe not found at ' + exe)

def run(script, *args):
    subprocess.run([sys.executable, '-I', os.path.join(here, script), *args], check=True,
                   stdout=subprocess.DEVNULL)

with tempfile.TemporaryDirectory() as work:
    run('rcc_extract.py', exe, os.path.join(work, 'qrc'))
    run('ico_extract.py', exe, os.path.join(work, 'ico'))
    resources = os.path.join(work, 'qrc', 'Resources')
    icons, xml = os.path.join(target, 'icons'), os.path.join(target, 'xml')
    os.makedirs(icons, exist_ok=True)
    os.makedirs(xml, exist_ok=True)
    count = 0
    for name in sorted(os.listdir(resources)):
        if name.endswith('.png'):
            shutil.copyfile(os.path.join(resources, name), os.path.join(icons, name)); count += 1
    for name in sorted(os.listdir(os.path.join(resources, 'xml'))):
        if name.endswith('.xml.gz'):
            with gzip.open(os.path.join(resources, 'xml', name)) as packed:
                open(os.path.join(xml, name[:-3]), 'wb').write(packed.read())
            count += 1
    shutil.copyfile(os.path.join(work, 'qrc', 'build', 'languages', 'English.xml'),
                    os.path.join(xml, 'English.xml'))
    shutil.copyfile(os.path.join(work, 'ico', 'app_1.ico'), os.path.join(icons, 'app.ico'))
    print('extracted %d resources plus English.xml and app.ico into %s' % (count, target))
