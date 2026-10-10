# Macs Fan Control 1.5.20 (Windows) — temperature sensor subsystem spec

Source: Ghidra decompilation (`re/out/MacsFanControl.exe/decomp.c`) plus raw byte inspection of
`MacsFanControl/MacsFanControl.exe` where the decompiler dropped call arguments. Every claim carries the
function address (image base 0x00400000). "UNKNOWN" = could not be determined; "ASSUMPTION" = not read
from the binary.

Conventions: `CI` = case-insensitive, `CS` = case-sensitive. Offsets are for the 32-bit build.

---------------------------------------------------------------------------------------------------

## 1. Object model

### 1.1 `TemperatureSensorAbstract` (ctor 00473be0, vtable 0075bd94)

`ctor(this, QString key, QString name, int type)` (argument order verified from the call site bytes at
0047fe2a: `push [rec+0x44] ; push &rec.name ; push smcKey`).

| Off | Type | Field | Notes |
|----|------|-------|-------|
| 0x00 | vptr | | |
| 0x08 | int | `index` | global creation counter `DAT_0089f5d8++` (00473be0); starts at 0. Not persisted. |
| 0x10 | double | `value` | degrees Celsius, initial 0.0 |
| 0x18 | QString | `name` | display name |
| 0x24 | QString | `key` | **the unique id** (what fan presets / `traySensor` store) |
| 0x30 | QString | `groupKey` | from XML `<group key=…>`; empty = not in a group |
| 0x3c | QString | `groupName` | from XML `<group name=…>` |
| 0x48 | bool | `groupHideSensors` | from XML `<group hideSensors=…>` |
| 0x4c | int | `type` | category enum, see 1.3 |
| 0x50 | bool | `hidden` | initial false; hidden sensors are excluded from every list returned to the UI |

Virtual methods (same slot layout for all subclasses):

| Slot | Abstract | Meaning |
|------|----------|---------|
| 0 | 00473ce0 | deleting dtor |
| 1 | 00473dc0 | `suggestedMinTemp()` = `max(20, round(value) - 5)` (default "start increasing at" for sensor-based fan control). Disk override 00474000: constant **42**. |
| 2 | 00473d40 | `suggestedMaxTemp()` = `type == 1 (cpu) ? 90 : min(100, round(value) + 5)`. Disk override 00473fe0: `type ∈ {3,4} ? 70 : 52`. |
| 3 | pure | `UpdateValue()` — refreshes `value` |
| 4 | 00473e70 | `matchesKey(QString k)` — exact CS equality with `key`. Overrides: SMC 0047f960, GPU 00479b30 (see 1.2). |

### 1.2 Subclasses and their id (`key`) strings

| Class | size | vtable | `key` (id) format | `matchesKey` | UpdateValue |
|-------|------|--------|-------------------|--------------|-------------|
| `TemperatureSensorSMC` | 0x60 | 0075c900 | the 4-char SMC key **exactly as enumerated from the SMC** (e.g. `TC0P`, `TG0p`), not the spelling in the XML | 0047f960 → `smcKeyMatch(key, k)` (see below) | 0047f720 |
| `TemperatureSensorAverage` (derives `TemperatureSensorVirtual`) | 0x68 | 0075c5fc | the XML group `key` attribute, e.g. **`cpucoreavg`**; if the group has no `key`, `name.replace(" ", "_").toLower()` (0047a580) | exact CS | 00480ac0 |
| `TempSensorDiskWin` (derives `TempSensorDisk`) | see §4 | 0075bdfc | see §4.7 | exact CS | 00475180 |
| `TemperatureSensorGPU` | 0x68 | 0075c3dc | (a) key of the covering SMC sensor (`TG0D`/`TGDD`/`TCGC` in the SMC's real case) when one exists, else (b) `"GPU <name> (<i>)"` — see §5.1 | 00479b30: if `len(key)==len(k)==4` → `smcKeyMatch(key,k)`, else exact CS | 00479920 |
| `TemperatureSensorVirtual` | 0x68 | 0075ca0c | base of Average/Copy; ctor 00480960; holds `std::vector<Sensor*> members` at +0x58/+0x5c/+0x60 | exact CS | pure |
| `TemperatureSensorCopy` | 0x68 | 0075c91c | **never instantiated** in this build: only a vtable and dtor (0047f650) exist; no constructor call site stores this vtable anywhere in decomp.c. Its UpdateValue slot is the same function as Average (00480ac0). | | |

`smcKeyMatch(a, b)` = FUN_0047f850: `a.length() == 4 && a.compare(b, CI) == 0 && a.left(3) == b.left(3)`
(left length 3 verified: `6a 03` at 0047f89e / 0047f8ae). I.e. first three characters exact, the fourth
character case-insensitive. This is used for XML lookup, preset-id matching and GPU↔SMC matching.

Consequence for presets: a stored id `TC0c` resolves to an SMC sensor whose real key is `TC0C`, and
vice-versa.

### 1.3 `type` enum (category)

Derived from XML `type` attribute (004720c0, string table 0075bd70…) and from the disk/GPU loaders.

| Value | Meaning | Source |
|------:|---------|--------|
| 0 | optical drive | XML `type="odd"` |
| 1 | CPU | XML `type="cpu"` |
| 2 | disk: HDD (rotational) | disk loader (§4.6) |
| 3 | disk: SSD | XML `type="ssd"`; disk loader |
| 4 | disk: NVMe | disk loader |
| 5 | disk: external | disk loader |
| 6 | Airport / Wi-Fi | XML `type="airport"` |
| 7 | power supply | XML `type="psu"` |
| 8 | GPU | XML `type="gpu"`; all `TemperatureSensorGPU` (push 8 at 00479f87 / 0047a27f) |
| 9 | generic / other | XML: attribute absent or any other value |

Types 2–5 are treated as "disk" everywhere (`type ∈ {2,3,4,5}` tests at 0047e120, 0047c6c0).

### 1.4 `TemperatureSensorProvider` (QObject; ctor 0047baf0, vtable 0075c5c0)

| Off | Field |
|----|-------|
| 0x08..0x10 | `std::vector<Sensor*> smcSensors` (SMC sensors + their Average sensors) |
| 0x14..0x1c | `std::vector<Sensor*> diskSensors` |
| 0x20..0x28 | `std::vector<Sensor*> gpuSensors` |
| 0x2c..0x34 | fourth `std::vector<Sensor*>` — never filled on Windows (only cleared in 0047c9e0, post-processed in 0047e120). ASSUMPTION: the macOS IOKit sensor list. |

#### `load(smcService, gpuProvider)` — 0047e120

1. `smcSensors = TemperatureSensorSMC::loadAvailableSensors(smcService)` (0047f980, §2).
2. `showDrives = Settings["ShowDrives"]` (global 0089f698), `showExternal = Settings["ShowDrivesExternal"] == 1`
   (global 0089f6a4); both read as int through FUN_005987a0.
3. Find the SMC sensors used for GPU matching by scanning `smcSensors` with `matchesKey` and keeping the
   **last** match: `integrated = match("TCGC")`; `discrete = match("TG0D")`, and only if that is null
   `discrete = match("TGDD")`. (Same lookups are also available as 0047de70 / 0047dfd0.)
4. `gpuSensors = TemperatureSensorGPU::loadAvailableSensors(gpuProvider, discrete, integrated)` (00479c60, §5.1).
5. Once per process (`InitOnce` 0089f6f0): `QObject::connect(gpuProvider, <signal>, this, <slot>)` — the GPU
   provider's `gpuListReloadRequired` signal (string 007a534e) to a provider slot that rebuilds the list.
   UNKNOWN: exact slot body (pointer-to-member thunks 004082fb / 0040a5fb / 00407a59 not followed).
6. If `showDrives == 1`: `diskSensors = TempSensorDisk::loadAvailableSensors(showExternal)` (004756e0, §4).
7. If `diskSensors` is non-empty: every sensor in `smcSensors` (and the 4th list) with `type ∈ {2,3,4,5}`
   gets `hidden = true`. Effect: SMC `type="ssd"` sensors (`TH0A`, `TH1A`, `TH1B` "Apple SSD") disappear
   when at least one S.M.A.R.T./NVMe drive sensor was found.
8. `buildAverages(smcSensors)`; `buildAverages(4th list)` (0047bf50, §2.5).
9. Logs: `InitialLoadThread: number of supported SMC sensors: {}` (count **includes** hidden and average
   sensors), `… DISK sensors: {}`, `… GPU sensors: {}`.

#### `visibleSensors()` — 0047d750 (the list every UI consumer uses)

```
out = [s for s in smcSensors  if !s.hidden]
    + [s for s in diskSensors if !s.hidden]
    + [s for s in gpuSensors  if !s.hidden]
sort(out, sensorLess)            // comparator thunk 0040e863 → 0047c6c0
```

`sensorLess(a, b)` — 0047c6c0:

```
na = a.name; nb = b.name
if a.type ∈ {2,3,4,5}: na = "zzz" + na          // disks sort after everything else
if b.type ∈ {2,3,4,5}: nb = "zzz" + nb
for (s, n) in ((a, na), (b, nb)):
    if s.type == 1 && n.contains("Performance", CS) && n.right(2).toInt() >= 10:
        n.insert(n.size() - 2, "9")             // "…Core 10" sorts after "…Core 9"
return QString::compare(na, nb, CS) < 0         // UTF-16 code-unit order
```

So the final order is: **all non-disk sensors (SMC, averages, GPU) sorted by display name, case-sensitive
(uppercase before lowercase), then all disk sensors sorted by display name.** The sort is `std::sort`
(0047adb0), not stable.

The "Disk Drives:" separator is not a sensor; see §8.

#### `getSensorDisplayNameByKey(key)` — 0047dc10

First sensor in `visibleSensors()` with `matchesKey(key) && !hidden` → its `name`. Otherwise logs
(level 4) `Cannot locate sensor by key {}` and returns an empty QString.

---------------------------------------------------------------------------------------------------

## 2. SMC sensors and `smcSensors.xml`

### 2.1 Files

* `:/Resources/xml/smcSensors.xml.gz` is the only sensor table loaded (static init 00422980 builds the
  definitions object at 0089f6f8 with that path and the comparator `smcKeyMatch` 0047f850).
* **`IOKitSensors.xml(.gz)` is not used on Windows**: its name appears only inside the Qt resource name
  table (string 007961d3, no code reference); there is no `:/Resources/xml/IOKitSensors…` path string.

Definitions object (0089f6f8): `+0x00 QString resourcePath`, `+0x10..0x34 std::function<bool(const
QString&, const QString&)> keyEq` (= `smcKeyMatch`), `+0x38 QList<SensorDef> defs`.

`SensorDef` (0x48 bytes, built in 004720c0):

| Off | Field |
|----|-------|
| 0x00 | QString `key` (XML `name` attribute, verbatim) |
| 0x0c | QString `displayName` |
| 0x18 | bool `addNoFail` (attribute `add_no_fail` present) |
| 0x1c | QList<QString> `extraNames` (see 2.3) |
| 0x28 | QString `groupKey` |
| 0x34 | QString `groupName` |
| 0x40 | bool `groupHideSensors` |
| 0x44 | int `type` |

### 2.2 Loading — 00471f10

Runs once (skipped when `defs` is already non-empty). Reads the resource (0059a0a0), gunzips it
(005bcde0), parses it with pugixml (00528640). Then:

```
root = doc.first_child()                       // <smcSensors>
for block in root.children("modelMatch"):      // document order
    models = block.attribute("models")
    if models && modelMatches(models):         // 004b1ee0
        parseBlock(block, group = {key:"", name:"", hide:false})   // 004720c0
```

### 2.3 `modelMatches(models)` — 004b1ee0 (method of the machine-info object)

```
if models == "all": return true                       // exact strcmp
for item in models.split(';'):                        // no trimming, empty parts kept
    if !item.contains('+'):
        if item == machine.modelString: return true   // exact, CASE-SENSITIVE (QtPrivate::equalStrings)
    else:
        family = 11; number = 0
        if parseModel(item, &family, &number)         // 004b2900
           && machine.family == family
           && machine.number >= number:               // unsigned 16-bit compare (jae at 004b201c)
            return true
return false
```

`parseModel(s, &family, &number)` — 004b2900. Family by `startsWith` (CS), tested **in this order**:

| Prefix | family |
|--------|-------:|
| `Macmini` | 6 |
| `MacPro` | 8 |
| `MacBookPro` | 3 |
| `MacBookNeo` | 2 |
| `MacBookAir` | 1 |
| `MacBook` | 0 |
| `iMacPro` | 5 |
| `iMac` | 4 |
| `Xserve` | 9 |
| `ADP` | 10 |
| `MacStudio` | 7 |
| (none) | stays 11 = unknown |

`number = s.replace(QRegularExpression("[^0-9]"), "").toUInt()` truncated to 16 bits — i.e. all digits
concatenated: `MacBookPro13,2+` → 132, `MacBookAir6,1+` → 61, `MacBook8,1+` → 81. Returns false if family
is 11 or number is 0. The machine's own `family` (byte at +0) / `number` (u16 at +2) / model string
(`*(this+0x10)+4`) come from the machine-info object (other spec); they are presumably produced by the
same parser (ASSUMPTION).

Notes / quirks to reproduce:
* `X,Y+` means "same family and concatenated-digits number ≥": `MacBookPro13,2+` (132) matches
  MacBookPro14,1 (141), 15,x, 16,x; it does not match MacBookPro9,1 (91). Because digits are concatenated,
  `MacBookAir6,1+` (61) would not match a hypothetical `MacBookAir10,1` → 101 ≥ 61 does match; but
  `MacBook10,1` (101) vs `MacBook8,1+` (81) matches as expected. Just implement the integer compare.
* Exact items are case-sensitive, so the XML entry `MacMini3,1` (capital M, in the `TH0P "HDD Proximity"`
  block) never matches a real `Macmini3,1`.
* There is no generic prefix rule: an item without `+` must equal the whole model string.

### 2.4 `parseBlock(node, group)` — 004720c0 (recursive)

For each child element of `node`, in document order (`first_child` / `next_sibling`, any element name):

```
nameAttr = child.attribute("name")                 // required (used unchecked)
if child.name() == "group":
    g.name = nameAttr
    g.key  = child.attribute("key") or ""          // if empty and name non-empty:
                                                   //   key = name.replace(" ", "_").toLower()   (0047a580)
    h = child.attribute("hideSensors")
    g.hide = (h absent) || h == "true" || h == "1" // CS compare; anything else (e.g. "0") = false
    parseBlock(child, g)                           // nested elements inherit this group
else:                                              // <smc> (element name is not checked)
    text    = child.child_value()                  // element text
    typeStr = child.attribute("type")
    excl    = child.attribute("exclude")
    noFail  = child.attribute("add_no_fail") != null
    defs.removeAll(d => smcKeyMatch(d.key, nameAttr))      // 00473760 — ALWAYS, before anything else
    if excl attribute is ABSENT:                           // presence alone excludes; value is ignored
        type = 9
        if typeStr: "cpu"→1, "gpu"→8, "odd"→0, "psu"→7, "airport"→6, "ssd"→3, else 9   (CS)
        parts = QString(text).split(';')
        def.displayName = parts.isEmpty() ? "Unknown" : parts[0]
        def.extraNames  = parts.size() > 1 ? parts.mid(1) : []        // 00473260
        def.key = nameAttr; def.addNoFail = noFail; def.type = type
        def.group{Key,Name,HideSensors} = group
        defs.append(def)                                             // 00470db0
```

Resulting semantics:

* **Precedence**: blocks are applied in document order; a later matching block's `<smc name=K>` *replaces*
  any earlier definition whose key matches under `smcKeyMatch` (4th char case-insensitive!) and the new
  definition is appended at the end of the list. So `TC1C` in the "Macmini8,1;MacBook8,1+;…" block
  replaces the `all` block's `TC1c` ("CPU Core 2") with "CPU Core 1", etc. The same holds inside one
  block (`TM9P` is defined twice in the MacPro1,1 block → the second, "FBDIMM Riser B Regulator Area",
  wins).
* **`exclude`**: removes any earlier definition for that key and adds nothing → an SMC key with no
  definition is not shown (2.6). The attribute value is not inspected.
* **`group`**: members get the group's key/name/hide flag; an Average sensor is created per group key
  (2.5). A redefinition outside a group removes group membership (the def is replaced whole).
* **`type`**: sets the category (icon, `suggestedMaxTemp`, sorting quirk, hide-when-disks rule for `ssd`).
* **`add_no_fail`**: parsed and stored (`SensorDef+0x18`) but **never read** by the Windows SMC loader
  (0047f980 only reads def+0x0c, +0x28, +0x34, +0x40, +0x44). No element in the shipped XML uses it.
* `extraNames` (text after the first `;`) is stored but not read by the Windows loader either; unused by
  the shipped XML.
* Element text of an excluded element is ignored.

### 2.5 Group → `TemperatureSensorAverage` — 0047bf50

```
QHash<QString groupKey, std::vector<Sensor*>> groups
for s in list (vector order):
    if !s.groupKey.isEmpty():
        groups[s.groupKey].push_back(s)
        if s.groupHideSensors: s.hidden = true
for (gk, members) in groups:                       // QHash iteration order
    avg = new TemperatureSensorAverage(key = gk,
                                       name = members[0].groupName,
                                       type = members[0].type,     // 00480960
                                       members)
    list.push_back(avg)
```

* An Average exists only if at least one member SMC key is actually present (and readable) on the machine.
* `hideSensors="0"` (as shipped) keeps the individual cores visible next to "CPU Core (average)".
  When the attribute is absent the default is **hide** (members hidden, only the average shown).
* Average sensors have empty group fields and `hidden=false`.

### 2.6 `TemperatureSensorSMC::loadAvailableSensors(smc)` — 0047f980

```
defs.load()                                               // 00471f10
keys = smc.getTemperatureKeys()  → QStringList            // 004802a0 wraps 004af070; [] if it returns false
excluded = []
for a in keys:                                            // de-duplicate case variants
    for b in keys:
        if a != b && smcKeyMatch(a, b) && a ∉ excluded && b ∉ excluded:
            if preferFirst(a, b) == 1: excluded += b      // 004806b0
            else:                      excluded += a
            break                                         // next a
for k in keys (enumeration order):
    if k ∈ excluded: continue
    def = defs.first(d => smcKeyMatch(d.key, k))          // 004739e0
    if def == null: continue                              // *** keys absent from the XML are dropped silently ***
    double v
    if !smc.getTemperature(k.toStdString(), &v):          // 004aeb90 — probe read
        log(level 2) "FilterSupportedTempSensors - skipping sensor {}" ; continue
    s = new TemperatureSensorSMC(key = k, name = def.displayName, type = def.type)
    s.smc = smc                                           // +0x58
    if !def.groupKey.isEmpty(): s.group{Key,Name,HideSensors} = def.group…
    result.push_back(s)
```

`preferFirst(a, b)` — 004806b0 (returns 1 = keep a, 2 = keep b; indices verified `6a 03` = char index 3):

```
special = a CI-equals one of "TH0A","TH1A","TH1B","TG0D","TG1D"
if special:
    if a[3].isUpper(): return 1
    if b[3].isUpper(): return 2
if a[3].isLower(): return 1
if b[3].isLower(): return 2
return 0
```

i.e. when the SMC exposes both `TxxY` and `Txxy`, the **lowercase** variant is kept, except for
`TH0A/TH1A/TH1B/TG0D/TG1D` where the **uppercase** variant is kept. The dropped variant gets no sensor.

Sensors are created in SMC enumeration order; the probe value is discarded (sensor `value` starts at 0.0
until the first `UpdateValue`).

`TemperatureSensorSMC::UpdateValue` — 0047f720: `if smc.getTemperature(key, &v, false) value = v;` on
failure the previous value is kept. No range filtering in the sensor class itself (any filtering is inside
`getTemperature`, other spec).

