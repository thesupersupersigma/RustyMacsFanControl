# Macs Fan Control 1.5.20 (Windows) — temperature sensor subsystem spec

Source: Ghidra decompilation (`re/out/MacsFanControl.exe/decomp.c`) plus raw byte inspection of
`MacsFanControl/MacsFanControl.exe` where the decompiler dropped call arguments. Every claim carries the
function address (image base 0x00400000). "UNKNOWN" = could not be determined; "ASSUMPTION" = not read
from the binary.

Conventions: `CI` = case-insensitive, `CS` = case-sensitive. Offsets are for the 32-bit build.
Sections appear in the order they were written: 1, 2, 3, 4, 7, 8, 5, 6, 9, 10.

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
  MacBookPro14,1 (141), 15,x, 16,x; it does not match MacBookPro9,1 (91). The compare is on the concatenated-digits integer, not on (major, minor): e.g.
  a two-digit minor such as `X9,10` gives 910, which is greater than `X10,1` = 101. Implement exactly the
  integer compare.
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

---------------------------------------------------------------------------------------------------

## 3. Value handling, polling, conversion, display

### 3.1 Polling — update thread 0044e460 (thread name `updateThread`)

Per cycle (only the sensor-relevant part; fan handling is another spec):

```
sensors = provider.visibleSensors()                 // 0047d750; re-fetched when a reload was flagged
windowVisible = firstCycle ? true : mainWindow.isVisible()
trayKey = Settings.string("traySensor")             // global 0089ed64
needed  = keys of sensors referenced by fans whose profile mode == 4 (sensor-based)   // only built when !windowVisible
for s in sensors:
    if windowVisible || s.key == trayKey (exact, CS, 00448e70) || needed.contains(s.key, CS):
        s.UpdateValue()                             // vtable slot 3
        emit signal#2(s.index, s.value)             // 004c8bb0, QMetaObject::activate(..., 2, ...)
    // else: neither updated nor emitted
wait up to 1500 ms (0x5dc) on two event objects (this+0x38, this+0x160)   // 0055aca0; then next cycle
```

* **Poll interval: 1500 ms** (interruptible by the thread's events).
* While the main window is hidden only the tray sensor and sensors driving sensor-based fans are read.
* The `needed`/tray comparison is exact CS string equality on `key` (not `smcKeyMatch`).

### 3.2 Validity / filtering

There is no explicit validity flag on a sensor. Rules found:

| Where | Rule |
|-------|------|
| SMC load (0047f980) | sensor only created if the probe `getTemperature` returns true |
| SMC update (0047f720) | on `getTemperature` failure the old value is kept |
| SATA update (004745f0) | raw byte 0 → value unchanged |
| NVMe update (004741b0) | Kelvin field 0 → value unchanged |
| GPU update (00479920) | provider result exactly `0.0f` = failure (§5.2) |
| Average (00480ac0) | **no filtering**: `value = Σ member.value / member_count` after calling `UpdateValue()` on every member (members that failed keep their stale/0 value and are still counted). Hidden members are included. |
| Display (004bb660) | `value == 0.0` → `"-"`; `value < 0` → empty string; `value > 0` → number |
| Display staleness (004bb660, role 9) | value cell drawn grey `QColor(128,128,128)` when the row's last-update timestamp is valid and more than 5 s old |

Any zero/range filtering of raw SMC readings lives inside `getTemperature` (004aeb90, other spec).

### 3.3 Celsius → Fahrenheit (helpers at 004bcce0…004bd1e0)

| Function | Behaviour |
|----------|-----------|
| 004bd110 `toDisplay(double c)` | `Settings["Fahrenheit"] != 0 ? c * 9.0 / 5.0 + 32.0 : c` (constants 0075fd30=9.0, 0075fd20=5.0, 0075fd40=32.0) |
| 004bd040 `toDisplay(int c)` | same, result truncated toward zero (`(int)` cast) |
| 004bcd60 `fromDisplay(int v)` | `Fahrenheit ? (int)round((v - 32) * 5.0 / 9.0) : v` |
| 004bce30 `unitString()` | `"°C"` or `"°F"` (UTF-8 `C2 B0` at 0075fd14 + letter) |
| 004bd1e0 | `isFahrenheit()` |

### 3.4 Main-window sensor list (model `data()` 004bb660, header 004bbd60, rebuild 004bc470)

Row struct (0x20 bytes): `int id` (= sensor `index`, −1 for the separator), `QString name`, `double value`
(placeholder default = constant 0075fd08), `QIcon icon`, `QDateTime lastUpdate`.

| Role | Column 0 | Column 1 |
|------|----------|----------|
| Display | sensor name; separator row: `tr("MainWindow/listSeparatorDrives")` = **"Disk Drives:"** | value text (below); separator: nothing |
| Decoration | icon (§7); separator: 16×16 transparent pixmap (0045d140) | — |
| Font | separator only: the model's special font (model+0x18) | — |
| TextAlignment | 0x81 (Left\|VCenter) | 0x84 (HCenter\|VCenter) |
| Foreground | — | grey if stale > 5 s |

Value text (identical code at 004bb660 and its tail 004bb812):

```
text = (value == 0.0) ? "-" : ""
if value > 0.0:
    if Settings["PreciseTemperature"] == 0:
        text = QString::number(round(toDisplay(value)), 'g', 6)   // integer, e.g. "45"; C round() = half away from zero
    else:
        text = QString::number(toDisplay(value), 'f', 1)          // one decimal, e.g. "45.4"
```

**No degree sign or unit in the cell.** The unit is in the column header: column 0 header =
`tr("MainWindow/listColumnSensor")` ("Sensor"), column 1 header =
`tr("MainWindow/listColumnSensorValue") + " " + unitString()` → **"Value °C"** / **"Value °F"**.
Rounding is applied after conversion to Fahrenheit.

Settings defaults (registration 00433790): `Fahrenheit`=0, `PreciseTemperature`=0, `ShowDrives`=1,
`ShowDrivesExternal`=0, `traySensor`=""; `ShowEGpuTemp` see §5.6. The same function deletes the obsolete
values `ShowGpuTemp`, `ShowSmartDrives`, `ShowNVMeDrives`, `TempSensors`, `Fans`, `Machine`, `CPU`,
`EnableLog`, `Fan_0`…`Fan_9` (their values are not migrated).

UNKNOWN: text format of the temperature drawn in the tray icon / tooltip (functions 004bf1b0, 004bf940,
00467c50 read `traySensor`/`PreciseTemperature` but were not analysed here). The only other format string
seen is `"T° %1"` (0075b604, Preferences tray-sensor combo label).

### 3.5 Fan-control dialog use of sensors (0043b540 / 0043b040 / 0043b000)

Sensor combo entries are `{name, key (item data), icon, suggestedMinTemp(), suggestedMaxTemp()}` built from
`visibleSensors()`. Spin boxes range `toDisplay(20) … toDisplay(100)`; selecting a sensor presets them to
`toDisplay(suggestedMin)` / `toDisplay(suggestedMax)`; the max spin box minimum is `minSpin + 8` (°C) or
`minSpin + 14` (°F). The preset stores the sensor `key` string.

---------------------------------------------------------------------------------------------------

## 4. Disk drive sensors (`TempSensorDisk` / `TempSensorDiskWin`)

Object: size 0x68, vtable 0075bdfc. Extra fields: `+0x58 HANDLE hPhysicalDrive` (kept open for the life of
the sensor), `+0x5c u32 diskNumber`, `+0x60 bool isUsb`, `+0x61 bool isNvme`.

### 4.1 Enumeration — `TempSensorDisk::loadAvailableSensors(bool showExternal)` 004756e0

Only called when `ShowDrives == 1`. Logs: `Loading available disk drives`, `GetLogicalDrives returned {}`.

```
mask = GetLogicalDrives()
seen = []                                           // physical disk numbers already handled
for i in 0..31 where mask bit i set:
    letter = 'A' + i                                // log "Checking drive {}"
    if GetDriveTypeW(L"%c:\\") != DRIVE_FIXED (3):  // log "The drive is NOT fixed ({})"
        continue
    hVol = CreateFileW(L"\\\\.\\%c:", 0xC0000000 /*GENERIC_READ|GENERIC_WRITE*/, 3 /*share R|W*/, NULL,
                       OPEN_EXISTING (3), 4 /*dwFlagsAndAttributes*/, NULL)
    if hVol == INVALID: log "Failed to open mounted volume {}, GetLastError() = {}" ; continue
    DeviceIoControl(hVol, 0x00560000 /*IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS*/, NULL, 0, out, 0x20, &ret)
        fail → log "Failed to get IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS" ; close ; continue
    disk = *(u32*)(out + 8)                         // Extents[0].DiskNumber — only the first extent
    log "Phisical disk number: {}"
    if disk ∈ seen: close hVol ; continue
    seen += disk
    hDisk = CreateFileW(L"\\\\.\\PHYSICALDRIVE%d", 0xC0000000, 3, NULL, 3, 4, NULL)
    if INVALID: log "Failed to open device {}, GetLastError() = {}" ; close hVol ; continue
    bus = busType(hDisk)                            // 4.2 ; 0 on failure
    added = false
    if bus == 7 (USB) && !showExternal: log "Skipping USB drive" ; close both ; continue
    if bus == 0x11 (NVMe):
        id = NVMe identify (4.3)
        if !id.ok: log "NVMeReader::readIdentify failed, GetLastError() = {}"          // falls to SATA path
        else:
            info = nvmeInfo(id)                                                        // 4.7
            tb = isThunderboltDrive(disk)                                              // 4.5
            if !tb || showExternal:
                log "Adding NVMe drive {}"
                add sensor(key=info.key, name=info.name, type = tb ? 5 : 4, isUsb=false, isNvme=true); added = true
            else: log "Skipping Thunderbolt drive"                                     // falls to SATA path!
    if !added:                                      // "SATA path" (also USB, and NVMe fall-through)
        isUsb = (bus == 7)
        if isUsb:
            info = readSmartIdentify(usb=true)      // 4.4d ; fail → log "readSmartIdentify, GetLastError() = {}"
        else:
            if !isSmartAvailable(hDisk, (u8)disk):  // 4.4a ; log "SMART is not enabled, GetLastError() = {}"
                info.valid = false
            else:
                log "Adding SATA drive {}"
                info = readSmartIdentify(usb=false) // 4.4b ; fail → log "readSmartIdentify, GetLastError() = {}"
        if info.valid:
            log "Adding drive {}"
            type = isUsb ? 5 : (info.isSsd ? 3 : 2)
            add sensor(key=info.key, name=info.name, type, isUsb, isNvme=false); added = true
    close hVol
    if !added: close hDisk                          // otherwise hDisk is owned by the sensor
```

Filters, summarised: only volumes with a drive letter and `DRIVE_FIXED`; one sensor per physical disk
(first extent of the volume); USB bus (BusType 7) and Thunderbolt NVMe only when `ShowDrivesExternal == 1`.
Thunderbolt detection is applied **only on the NVMe path**. No temperature read is attempted at load
time (a drive without a usable temperature attribute still gets a sensor that stays at 0 → "-").
A skipped Thunderbolt NVMe drive falls through to the SATA path, where `isSmartAvailable` normally fails.

### 4.2 Bus type — inline in 004756e0 (same code as helpers 00475410 / 00475570)

`IOCTL_STORAGE_QUERY_PROPERTY` = **0x002D1400**, input `STORAGE_PROPERTY_QUERY` 12 bytes all zero
(`PropertyId = StorageDeviceProperty(0)`, `QueryType = PropertyStandardQuery(0)`):

1. output 8 bytes (`STORAGE_DESCRIPTOR_HEADER{Version, Size}`); require success and `0x28 ≤ Size ≤ 0x10000`.
2. output `Size` bytes; require success and `bytesReturned ≥ 0x28`; `BusType = *(u32*)(buf + 0x1C)`.

Failure → bus 0. Values acted on: `7` = BusTypeUsb, `0x11` = BusTypeNvme; everything else = SATA path.

### 4.3 NVMe (`NVMeReader`)

Both use `IOCTL_STORAGE_QUERY_PROPERTY` 0x002D1400 with the same 0x1030-byte buffer as input and output
(zero-filled): `STORAGE_PROPERTY_QUERY` header + `STORAGE_PROTOCOL_SPECIFIC_DATA` at +8, payload at +0x30.

| Off | Field | readIdentify 00477c10 | readSmartLog 00477d70 |
|----:|-------|----------------------|-----------------------|
| 0x00 | PropertyId | 0x31 (49, StorageAdapterProtocolSpecificProperty) | 0x32 (50, StorageDeviceProtocolSpecificProperty) |
| 0x04 | QueryType | 0 | 0 |
| 0x08 | ProtocolType | 3 (ProtocolTypeNvme) | 3 |
| 0x0C | DataType | 1 (NVMeDataTypeIdentify) | 2 (NVMeDataTypeLogPage) |
| 0x10 | ProtocolDataRequestValue | 1 (CNS: identify controller) | 2 (log page 02h SMART/Health) |
| 0x14 | ProtocolDataRequestSubValue | 0 | 0 |
| 0x18 | ProtocolDataOffset | 0x28 | 0x28 |
| 0x1C | ProtocolDataLength | 0x1000 | 0x1000 |
| result | | 4096 bytes from buf+0x30 | first 512 bytes from buf+0x30 |

Temperature (004741b0): `k = *(u16*)(log + 1)` (Composite Temperature, Kelvin, unaligned LE).
`if k != 0: value = (double)(int)(x ± 0.5)` where `x = k − 273.15` and the 0.5 takes the sign of `x`
(round half away from zero; constants 0075bdd0 = 273.15, 0075bdc0 = 0.5). `k == 0` → value unchanged.
The per-sensor temperature fields of the log are not used.

### 4.4 SATA / ATA (`WinSataReader`)

`SENDCMDINPARAMS` is always 0x21 (33) bytes: `cBufferSize(u32)`, `IDEREGS{Features, SectorCount,
SectorNumber, CylLow, CylHigh, DriveHead, Command, Reserved}`, `bDriveNumber`, zero padding.
`bDriveNumber` = low byte of the physical disk number. `DriveHead` is the constant 0xA0.

**(a) `isSmartAvailable(h, drive)` 00477eb0**

1. `SMART_GET_VERSION` = **0x00074080**, out `GETVERSIONINPARAMS` 0x18 bytes. Fail → log
   `Failed to get SMART_GET_VERSION`, false.
2. `fCapabilities` (u32 at +4) `& 4` (CAP_SMART_CMD) must be set, else log `CAP_SMART_CMD doesn't present ({})`, false.
3. Enable SMART: `SMART_SEND_DRIVE_COMMAND` = **0x0007C084**, in 0x21 bytes, out 0x11 bytes:
   `cBufferSize=0`, regs `D8 01 01 4F C2 A0 B0 00` (Features=0xD8 ENABLE_SMART, Command=0xB0). Returns the
   ioctl's success.

**(b) identify, non-USB — 00478220**: `SMART_RCV_DRIVE_DATA` = **0x0007C088**, in 0x21 bytes, out 0x211
bytes; `cBufferSize=0x200`, regs `00 01 01 00 00 A0 EC 00` (IDENTIFY DEVICE). Data = 512 bytes at out+0x10.

**(c) read attributes, non-USB — 00478350**: `SMART_RCV_DRIVE_DATA` 0x0007C088, in 0x21, out 0x210;
`cBufferSize=0x200`, regs `D0 01 01 4F C2 A0 B0 00` (READ ATTRIBUTES). Data = 512 bytes at out+0x10.

**(d) USB drives** — identify 004788a0, attributes 00478ae0. Three methods tried in order; a method counts
only if it succeeds **and** the 512-byte result is not all zero:

1. 00478d20: `IOCTL_STORAGE_QUERY_PROPERTY` 0x002D1400, 0x230-byte in/out buffer:
   `PropertyId=0x32`, `QueryType=0`, `ProtocolType=2 (ProtocolTypeAta)`,
   `DataType = 1 (AtaDataTypeIdentify)` for identify / `2 (AtaDataTypeLogPage)` for attributes,
   `ProtocolDataRequestValue = 0` for identify / `0xD0` for attributes, `ProtocolDataOffset=0x28`,
   `ProtocolDataLength=0x200`; data at buf+0x30.
2. `IOCTL_SCSI_PASS_THROUGH_DIRECT` = **0x0004D014**, in/out 0x4C bytes = `SCSI_PASS_THROUGH_DIRECT`
   (`Length=0x2C`, `CdbLength=16`, `SenseInfoLength=0x20`, `DataIn=1`, `DataTransferLength=0x200`,
   `TimeOutValue=10`, `DataBuffer=buf`, `SenseInfoOffset=0x2C`) + 32 sense bytes; success also requires
   `ScsiStatus == 0`. 16-byte CDB (ATA PASS-THROUGH(16)):
   * identify (004784d0): `85 08 0E 00 00 00 01 00 00 00 00 00 00 EC 00 00`
   * attributes (00478500): `85 08 0E 00 D0 00 01 00 00 4F C2 00 00 B0 00 00`
     (0x4F/0xC2 are written at CDB bytes 9 and 10 — as in the binary, `66 c7 40 09 4f c2`)
3. Same ioctl with `CdbLength=12` (004785e0), 12-byte CDB (ATA PASS-THROUGH(12)):
   * identify (00478460): `A1 08 0E 00 01 00 00 00 00 EC 00 00`
   * attributes (00478490): `A1 08 0E D0 01 00 4F C2 00 B0 00 00`

(00478540 / 00478800 / 004786f0 are equivalent stand-alone wrappers of the same steps.)

**Temperature from the attribute page — 004745f0.** Table = 30 entries × 12 bytes starting at data+2
(`id` at entry+0, raw value bytes at entry+5…+10):

```
e = first entry with id == 0xC2 (194, Temperature)                    // priority 1
if none: e = first entry with id == 0xBE (190, Airflow Temperature)   // priority 2
if none: return
t = raw[0]                       // ONLY the lowest raw byte (entry+5), unsigned
if t != 0: value = (double)t     // t == 0 → unchanged, and no fallback from 194 to 190
```

No other attribute IDs (e.g. 231) are used; min/max bytes in the raw value are ignored.

### 4.5 Thunderbolt detection — `ThunderboltDrive::isThunderboltDrive(diskNumber)` 00478e40

```
set = SetupDiGetClassDevsW(&GUID_DEVINTERFACE_DISK {53F56307-B6BF-11D0-94F2-00A0C91EFB8B}, NULL, NULL,
                           DIGCF_PRESENT | DIGCF_DEVICEINTERFACE (0x12))
for each interface (SetupDiEnumDeviceInterfaces / SetupDiGetDeviceInterfaceDetailW, detail.cbSize = 6):
    h = CreateFileW(detail.DevicePath, 0, 3, NULL, OPEN_EXISTING, 0, NULL)
    DeviceIoControl(h, 0x002D1080 /*IOCTL_STORAGE_GET_DEVICE_NUMBER*/, NULL, 0, &sdn, 12, &ret)
    if ok && ret >= 12 && sdn.DeviceNumber == diskNumber:
        dev = devinfo.DevInst
        while dev != 0:
            if CM_Get_DevNode_Registry_PropertyW(dev, CM_DRP_CAPABILITIES (0x10), NULL, &caps, &len=4, 0) == CR_SUCCESS
               && (caps & 0x84) != 0:        // CM_DEVCAP_REMOVABLE (0x04) | CM_DEVCAP_SURPRISEREMOVALOK (0x80)
                log "Found Thunderbolt drive" ; return true
            if CM_Get_Parent(&dev, dev, 0) != CR_SUCCESS: break
        return false
return false
```

So "Thunderbolt" really means "the NVMe disk or any ancestor devnode is removable / surprise-removable".
USB is detected purely by `BusType == 7`.

### 4.6 Type / icon choice

| Condition | type | icon |
|-----------|-----:|------|
| NVMe (bus 0x11), not Thunderbolt | 4 | drive_nvme.png |
| NVMe, Thunderbolt (and ShowDrivesExternal) | 5 | drive_external.png |
| USB (bus 7, and ShowDrivesExternal) | 5 | drive_external.png |
| other bus, IDENTIFY word 217 == 1 | 3 | drive_ssd.png |
| other bus, otherwise | 2 | drive.png |

SSD test: 005c01f0 reads `u16` at identify+0x1B2 (word 217, nominal media rotation rate); `== 1` → SSD.

### 4.7 Names and ids

SATA/USB — 00474250 (from the 512-byte IDENTIFY data; each ATA string is byte-pair-swapped and
whitespace-trimmed by 005c00a0, then `QString::fromLocal8Bit(...).trimmed()`):

```
model  = identify[0x36 .. +40]      // words 27–46
serial = identify[0x14 .. +20]      // words 10–19
fw     = identify[0x2E .. +8]       // words 23–26, read but unused
if model.isEmpty():
    if serial.isEmpty(): model = serial                                    // (stays empty)
    else:                model = QString("Drive %1").arg(g_driveCounter)   // global 0089f5dc
key  = model + QString(" (%1)").arg(serial)
name = model
```

(The inner condition is as compiled — `jne` at 004743e2 skips the assignment when the serial is non-empty.
`g_driveCounter` 0089f5dc has no writer anywhere in decomp.c, so it is 0 → `"Drive 0"`.)

NVMe — 00474010 (from Identify Controller):

```
model  = QString::fromLatin1(id + 0x18, 40).trimmed()   // MN
serial = QString::fromLatin1(id + 0x04, 20).trimmed()   // SN
key  = model + " (" + serial + ")"
name = model ;  isSsd = true
```

**Disk sensor id = `"<model> (<serial>)"`; display name = `"<model>"`.** No " (USB)" /
" (Thunderbolt/USB)" suffix is ever appended to a sensor name: `" (Thunderbolt/USB)"` (0075b728) is only
used by the Preferences window (0046eae0) for a checkbox caption, and `" (USB)"` does not occur in
strings.tsv. Two identical drives are distinguished only by the serial in the key; their display names
are equal.

### 4.8 `TempSensorDiskWin::UpdateValue` — 00475180

```
if hPhysicalDrive == 0: return
if isNvme:  log = readSmartLog(h) (4.3);  ok → parse (004741b0)
            else log "NVMeReader::readSmartLog failed, GetLastError() = {}"
else:
    page = isUsb ? usbReadAttributes(h) (4.4d) : sataReadAttributes(h, (u8)diskNumber) (4.4c)
    ok → parse (004745f0) else log "Failed to read SMART values, GetLastError() = {}"
```

`suggestedMinTemp` = 42; `suggestedMaxTemp` = 70 for types 3/4 (SSD/NVMe), 52 otherwise.

---------------------------------------------------------------------------------------------------

## 7. Icons (type → resource) — 0045d520

| type | resource |
|-----:|----------|
| 0 | `:/Resources/disc.png` |
| 1 | `:/Resources/processor.png` |
| 2 | `:/Resources/drive.png` |
| 3 | `:/Resources/drive_ssd.png` |
| 4 | `:/Resources/drive_nvme.png` |
| 5 | `:/Resources/drive_external.png` |
| 6 | `:/Resources/airport.png` |
| 7 | `:/Resources/power-supply.png` |
| 8 | `:/Resources/gpu.png` |
| 9 / other | no file: a 16×16 (DPI-scaled via 0059fb40) pixmap filled with `Qt::transparent` (QColor(19)) |

`QIcon(path)` picks up the `@2x` variants automatically. (0045d400 is the *fan* icon helper: fan.png /
disc.png / drive.png / processor.png with optional `_big`.)

## 8. Final list order and the "Disk Drives:" separator — 004bc470

```
rows = []
separatorAdded = false
for s in provider.visibleSensors():            // already sorted, §1.4: non-disk by name, then disks by name
    if s.type ∈ {2,3,4,5} && !separatorAdded:
        rows += Row(id = -1, name = "separator", icon = empty)      // rendered as "Disk Drives:"
        separatorAdded = true
    rows += Row(id = s.index, name = s.name, icon = iconForType(s.type))
emit layoutChanged()
```

The separator appears only if at least one visible disk-type sensor exists. Because an SMC sensor with
`type="ssd"` (type 3) also counts as disk-type, a machine with no S.M.A.R.T. sensors but an SMC
"Apple SSD" sensor still gets the separator with that SMC sensor under it.

---------------------------------------------------------------------------------------------------

## 5. GPU sensors

### 5.0 Provider object model

`GPU::Provider` (QObject, ctor 004587c0, vtable 00758a38): `+0x08..0x10 std::vector<GpuInfo*> gpus`.
One signal, `gpuListReloadRequired` (meta string 007a534e).

`GpuInfo` (0x18 bytes, ctor 004584a0, appended by `addGpu(name, handle, flag)` 00458910):

| Off | Field |
|----|-------|
| 0x00 | QString `name` |
| 0x0c | `Provider* owner` (the concrete AMD/NVIDIA/WMI provider) |
| 0x10 | `handle` — NVIDIA: `NvPhysicalGpuHandle`; AMD: `int* {adapterIndex, overdriveVersion}`; WMI: 0 |
| 0x14 | bool `flag` — passed as **0 by all three providers** (00459fa0, 0045bc10, 0045c570). When set it would stop the GPU being paired with the SMC discrete sensor (00479c60). ASSUMPTION: "external/eGPU" marker that is never set in this build. |

Virtual slots after the QObject ones (vtable offsets):

| Off | Meaning | Base | Composite `ProviderWindows` (00758a8c) | AMD (00758af8) | NVIDIA (00759154) | WMI (00759358) |
|----|---------|------|------|-----|--------|-----|
| +0x2c | `clear()` | 004589c0 (delete GpuInfos) | 00458c10 (clear all three children) | 00459e00 | 0045bc00 | base |
| +0x30 | `count()` | 00458ab0 | 00458d90 = nvidia + wmi + amd | base | base | base |
| +0x34 | `gpu(i)` | 00458a90 | 00458d10 | base | base | base |
| +0x38 | `discover()` → bool | pure | 00458c40 | 00459eb0 | 0045bc10 | 0045c570 |
| +0x3c | `temperature(GpuInfo*)` → float | pure | 00458c90 | 0045ab50→0045ab60 | 0045c0a0 | 0045c7d0 |

`ProviderWindows` (size 0x20, ctor 00458ae0, factory 00458a20): `+0x14 ProviderWindowsAMD*` (0x18 bytes),
`+0x18 ProviderWindowsNVIDIA*` (0x30 bytes), `+0x1c ProviderWindowsWMI*` (0x14 bytes).

```
ProviderWindows::discover():                       // 00458c40
    if count() == 0:
        amd.discover(); nvidia.discover()
        if count() == 0: wmi.discover()            // WMI only when neither vendor API found a GPU
    return count() != 0

ProviderWindows::gpu(i):                           // 00458d10
    if amd.count() == 0 && nvidia.count() == 0: return wmi.gpu(i)
    if i < amd.count(): return amd.gpu(i)
    return nvidia.gpu(i - amd.count())             // order: AMD GPUs first, then NVIDIA
```

A sensor reads through `GpuInfo.owner->temperature(info)` (004584f0), i.e. directly from the concrete
provider.

### 5.1 `TemperatureSensorGPU::loadAvailableSensors(provider, discreteSmc, integratedSmc)` — 00479c60

Object: 0x68 bytes; `+0x58 GpuInfo* gpu`, `+0x5c Sensor* smc` (covering SMC sensor or null),
`+0x60 bool apiFailed`, `+0x61 bool integrated`. Type is always 8.

```
if !provider.discover(): return []
for i in 0 .. provider.count()-1:
    g = provider.gpu(i)
    name = g.name ; name.remove(" (FireGL V)", CS) ; name.prepend("GPU ")
    ownKey = QString("%1 (%2)").arg(name).arg(i)          // i = decimal index in the provider list
    if g.name.contains("Intel", CS):                      // 00458510 → "integrated"
        log "Integrated GPU: {}, matching SMC - {yes|no}"
        if integratedSmc == null: log(warn) "Unable to add this GPU" ; continue
        s = new TemperatureSensorGPU(key = integratedSmc.key, name, type 8)
        s.gpu = null ; s.smc = integratedSmc ; s.apiFailed = false ; s.integrated = true
        integratedSmc.hidden = true
    else:
        match = g.flag ? null : discreteSmc               // flag is always false, see 5.0
        log "Discrete GPU: {}, matching SMC - {yes|no}"
        if match: match.hidden = true
        s = new TemperatureSensorGPU(key = match ? match.key : ownKey, name, type 8)
        s.gpu = g ; s.smc = match ; s.apiFailed = false ; s.integrated = false
    result.push_back(s)
```

* **Naming**: display name = `"GPU " + <adapter name without " (FireGL V)">`, e.g.
  `GPU AMD Radeon Pro 560X`, `GPU NVIDIA GeForce GT 750M`, `GPU Intel(R) Iris(TM) Graphics 6100`.
* **Id**: the SMC key of the covering sensor when there is one — `discreteSmc` = SMC sensor matching
  `TG0D` (else `TGDD`), `integratedSmc` = SMC sensor matching `TCGC` (§1.4 step 3) — otherwise
  `"GPU <name> (<i>)"`, e.g. `GPU NVIDIA GeForce GTX 1080 (0)`.
* **Internal GPUs already covered by SMC**: the SMC sensor ("GPU Diode", "GPU Diode 2" or "GPU PECI") is
  hidden and replaced by the GPU sensor, which takes over its key (so presets saved against `TG0D`
  keep working). Other SMC GPU sensors (`TG0P`, `TG0H`, `TG1P`, …) stay visible.
* With several discrete GPUs every one of them is paired with the same `discreteSmc` and gets the same
  key (as compiled; `flag` is never set).
* Integrated (Intel) GPUs are never read through a vendor API; without `TCGC` they are dropped.
* No GPU → no GPU sensors; the SMC GPU sensors stay as plain SMC sensors.

### 5.2 `TemperatureSensorGPU::UpdateValue` — 00479920

```
if gpu == null || apiFailed:
    if smc == null: return
    smc.UpdateValue() ; value = smc.value
else:
    t = gpu.owner->temperature(gpu)            // float
    if t == 0.0f:                              // sentinel 00758adc = 0.0f
        once per process (InitOnce 0089f604, lambda 004793a0): log "Reading of GPU temperature failed"
        if smc == null: return                 // value unchanged
        log(warn) "Trying SMC instead"
        apiFailed = true                       // permanent: all later updates use the SMC sensor
        return                                 // value not updated in this cycle
    value = (double)t
```

### 5.3 NVIDIA — `NVApi::load` 0045c1f0, `discoverGPUs` 0045bc10, temperature 0045c0a0

* DLL: `LoadLibraryW(L"nvapi.dll")` only (32-bit process; no `nvapi64.dll`). Export `nvapi_QueryInterface`.
  Missing → log `LoadLibrary (nvapi.dll) failed with {}`; success → log `nvapi.dll loaded`.
* Function IDs passed to `nvapi_QueryInterface` (stored in the provider at +0x14…):

| Slot | ID | NvAPI function |
|------|----|----------------|
| +0x18 | **0x0150E828** | NvAPI_Initialize |
| +0x1c | **0xE5AC921F** | NvAPI_EnumPhysicalGPUs |
| +0x20 | **0xCEEE8E9F** | NvAPI_GPU_GetFullName |
| +0x24 | **0xE3640A56** | NvAPI_GPU_GetThermalSettings |

  `+0x28 HMODULE`, `+0x2c bool loaded = (EnumPhysicalGPUs pointer != NULL)`. NvAPI_Unload is not resolved.
* discover: `load()` if not loaded; `NvAPI_Initialize()` once per process (static 0089eff0 holds the
  status; non-zero → log `NvAPI_Initialize failed`). Then `NvAPI_EnumPhysicalGPUs(handles[64], &count)`
  (failure → `NvAPI_EnumPhysicalGPUs failed`; success → log `NvAPI_EnumPhysicalGPUs {}`); for each handle
  `NvAPI_GPU_GetFullName(handle, char name[64])`; status 0 → `addGpu(QString(name), handle, 0)`.
* temperature: `NV_GPU_THERMAL_SETTINGS` **V1**: `version = 0x00010044` (struct size 0x44 = 68, version 1),
  `count = 3` pre-set, 3 × sensor `{controller, defaultMinTemp, defaultMaxTemp, currentTemp, target}`
  (5 × i32) zeroed. Call `NvAPI_GPU_GetThermalSettings(handle, 15 /*NVAPI_THERMAL_TARGET_ALL*/, &s)`;
  the status is ignored. Result = `(float) s.sensor[i].currentTemp` of the **first sensor with
  `target == 1` (NVAPI_THERMAL_TARGET_GPU)** among `i < s.count`; none → 0.0 (= failure). Unit °C.

### 5.4 AMD — `amdGPUAPI` ctor 0045b380, `discoverGPUs`/`initADL` 00459fa0, `getCoreTemperature` 0045ab60

* DLL: `LoadLibraryW(L"atiadlxx.dll")`, then `L"atiadlxy.dll"` as fallback. Neither → log
  `Failed to load atiadlxx.dll, error = {}`.
* Exports resolved (`GetProcAddress`, table order in the 0x3c-byte private object):

| Idx | Export |
|----:|--------|
| 0 | ADL_Main_Control_Create |
| 1 | ADL_Main_Control_Destroy |
| 2 | ADL_Adapter_NumberOfAdapters_Get |
| 3 | ADL_Adapter_AdapterInfo_Get |
| 4 | ADL_Overdrive_Caps |
| 5 | ADL_Overdrive5_Temperature_Get |
| 6 | ADL2_OverdriveN_Temperature_Get |
| 7 | ADL2_Main_Control_Create |
| 8 | ADL2_Main_Control_Destroy |
| 9 | ADL2_Adapter_Primary_Get |
| 10 | ADL2_New_QueryPMLogData_Get |
| 11 | (HMODULE) |

  then `+0x30 ADL_CONTEXT_HANDLE ctx`, `+0x34 std::map<int pciLocation, int adapterIdx>`.
  "API usable" (0045b790) = exports 0, 1, 2, 3 and 9 all non-null.
* discover:

```
if !usable || ADL_Main_Control_Create(mallocCb, 1) != 0:  log "ADL init failed" ; return
if ADL2_Main_Control_Create(mallocCb, 1, &ctx) != 0:      log "Failed to initialize ADL2 context" ; return
if ADL_Adapter_NumberOfAdapters_Get(&n) != 0: return
primary = -1 ; if ctx: ADL2_Adapter_Primary_Get(ctx, &primary)          // only logged
infos = malloc(n * 0x624)                                               // sizeof(AdapterInfo) = 0x624
if ADL_Adapter_AdapterInfo_Get(infos, n * 0x624) != 0: log "ADL_Adapter_AdapterInfo_Get failed" ; return
for i in 0 .. n-1:
    a = infos[i]
    if a.iVendorID (+0x114) != 0x3EA (1002): continue                   // AMD only
    loc = ((a.iBusNumber(+0x108) & 0xFF) << 5 | a.iDeviceNumber(+0x10c) & 0x1F) << 3 | a.iFunctionNumber(+0x110) & 7
    if loc already in map: continue                                     // one GpuInfo per PCI location
    map[loc] = i
    supported = enabled = version = -1
    if ADL_Overdrive_Caps(i, &supported, &enabled, &version) != 0: log(error) "Can't get Overdrive capabilities"
    addGpu(QString(a.strAdapterName /*+0x118*/), new int[2]{ i, version }, 0)
```

  (The adapter index used for every later call is the loop index `i`, not `a.iAdapterIndex`.)
* temperature decision tree (result initialised to 0.0f = failure):

| `overdriveVersion` | Call | Result |
|--------------------|------|--------|
| ≥ 8 | `ADLPMLogDataOutput d; d.size = 0x804; ADL2_New_QueryPMLogData_Get(ctx, adapter, &d)` | if `d.sensors[8].supported` (ADL_PMLOG_TEMPERATURE_EDGE, at +0x44) → `(float) d.sensors[8].value` (+0x48), **°C as-is**; else log `ADL_PMLOG_TEMPERATURE_EDGE not supported`; call failure → log `ADL2_New_QueryPMLogData_Get failed` |
| == 7 | `int t = -1; ADL2_OverdriveN_Temperature_Get(ctx, adapter, 1 /*iTemperatureType*/, &t)` | `t * 0.001f` (millidegrees); failure → log `ADL2_OverdriveN_Temperature_Get failed` |
| anything else (< 7, incl. −1 when caps failed) | `ADLTemperature t = {iSize = 8}; ADL_Overdrive5_Temperature_Get(adapter, 0 /*thermal controller*/, &t)` | `t.iTemperature * 0.001f`; failure → log `ADL_Overdrive5_Temperature_Get failed` |

  (Multiplier constant 00758ed4 = 0.001f; no fallback between the three methods.)

### 5.5 WMI provider — 0045c570

Not a temperature source. The GPU names come from the process-wide system-info object (0058b820), which
runs, via COM `IWbemServices::ExecQuery`, in namespace **`ROOT\CIMV2`** the query
**`SELECT Name FROM Win32_VideoController`** and reads property **`Name`** of each result into a
`std::vector<std::wstring>` (object +0x48). `discover()` adds one `GpuInfo(name, handle 0, flag 0)` per
entry. `temperature()` (0045c7d0) **always returns 0.0** → treated as failure, so such a GPU sensor shows
the covering SMC sensor's value (after one "Trying SMC instead"), or stays at 0 ("-") when there is none.
Its purpose is to give the SMC GPU diode sensor a real GPU name when no vendor API is available.

### 5.6 eGPU / `ShowEGpuTemp`

* Registration (00433790): default = `machine.hasThunderbolt() (004b2650) && <obj at app+0x48>->vslot3()`
  ? 1 : 0; when that condition is false the value is additionally **forced to 0** at every start-up.
  `hasThunderbolt()` (family/number as in §2.3): iMac ≥ 121; MacBookPro ≥ 81; MacBookAir > 70;
  Macmini ≥ 51; MacPro ≥ 61; iMacPro and MacStudio always; everything else false.
  UNKNOWN: identity of the object at app+0x48 and what its virtual slot 3 tests.
* The Preferences checkbox ("eGPUs connected via Thunderbolt", `checkGPUThunderbolt`) loads it (0046d410)
  and its slot (0046df60) stores it and emits the generic settings-changed signal (004cb680), like the
  ShowDrives slots (0046dd80 / 0046de70).
* **No sensor-side reader exists.** Each translation unit has its own static copy of the setting-name
  object (15 copies); a byte scan of `.text` for all 15 addresses finds code references only in
  00433790, 0046d410 and 0046df60. Neither the GPU providers nor `TemperatureSensorGPU` /
  `TemperatureSensorProvider` consult it, there is no Thunderbolt check in any GPU code path, and
  `GpuInfo.flag` is always 0. So on Windows 1.5.20 the setting has no observable effect on the sensor
  list: every GPU reported by ADL/NVAPI is listed regardless.
* The `gpuListReloadRequired` signal is connected (0047e120, once) to `TemperatureSensorProvider`'s own
  signal 0 (004cb850, signal-to-signal), but no emitter of the GPU-provider signal was found in decomp.c
  (its emit stub 004ca630 is not referenced). UNKNOWN whether hot-plug of an eGPU triggers anything.

---------------------------------------------------------------------------------------------------

## 6. "Virtual", "Average" and "Copy" sensors

* `TemperatureSensorVirtual` (ctor 00480960): abstract base holding `std::vector<Sensor*> members`
  (+0x58); `key`/`name` given by the caller, `type` = `members[0].type`. Never instantiated directly.
* `TemperatureSensorAverage`: the only concrete virtual sensor created on Windows — one per XML `<group>`
  key that has at least one present member (0047bf50, §2.5). `UpdateValue` (00480ac0): calls
  `UpdateValue()` on each member, then `value = sum(member.value) / members.size()` (plain arithmetic
  mean, no skipping of zero/failed members; members are therefore polled twice per cycle when they are
  also visible). The average is appended to `smcSensors` after all SMC sensors, is not hidden, and has
  no group fields of its own.
* `TemperatureSensorCopy`: vtable 0075c91c and deleting dtor 0047f650 exist, `UpdateValue` slot is the
  same 00480ac0, but no code constructs it (decomp.c contains no assignment of `TemperatureSensorCopy::vftable` at all;
  `TemperatureSensorVirtual::vftable` is only written by 00480960, whose single caller 0047bf50
  immediately overwrites it with the Average vtable). Treat as dead code on Windows.
  ASSUMPTION: on macOS it aliases one sensor under another key/name.

---------------------------------------------------------------------------------------------------

## 9. Diagnostics dump (for reference) — 0047cd90

Builds `"SMC sensors:\n%1\nGPUs:\n%2\nDisk drives:\n%3"`:

* %1: for every key from `getTemperatureKeys()` (including ones without a sensor):
  `"%1 (%2) - %3, %4%5\n"` = key, sensor name (empty if none, exact CS key match), value read now
  (`'g'` format), a second number returned by the same `getTemperature` call, and `" [H]"` if the sensor
  is hidden. The whole block is then passed through 005c03f0 together with a machine-info string
  (004b2190) before insertion (UNKNOWN transformation).
* %2: per GPU sensor `"%1 - %2\n"` = name with `"GPU "` removed, value.
* %3: per disk sensor `"%1 - %2\n"` = name, value.

---------------------------------------------------------------------------------------------------

## 10. UNKNOWN / not determined

| Item | What was tried |
|------|----------------|
| Slot connected to `gpuListReloadRequired`, and whether it is ever emitted | Followed thunks 004082fb→004cb850 (re-emit of provider signal 0); no caller of the emit stub 004ca630 in decomp.c. |
| Object at app+0x48 used for the `ShowEGpuTemp` default | Only its `vtable+0xc` call is visible in 00433790; constructor not located. |
| Any effect of `ShowEGpuTemp` on sensors | Byte-scanned `.text` for all 15 static copies of the setting name; no reader outside Preferences/registration. Conclusion "no effect" is from absence of evidence. |
| Tray icon / tooltip temperature text format | 004bf1b0, 004bf940, 00467c50 not analysed. |
| Internals/filters of `getTemperature` (004aeb90) and `getTemperatureKeys` (004af070) | Out of scope (other spec). The third argument passed by 0047f720 is 0. |
| Machine `family`/`number` derivation | Assumed to be 004b2900 applied to the machine model string; the caller that fills the machine-info object was not read. |
| QHash iteration order when several groups exist | Qt-internal; only one group key (`cpucoreavg`) can occur with the shipped XML, so it is moot. |
| `g_driveCounter` (0089f5dc) initial value | No writer in decomp.c; it lies in zero-initialised data, so taken as 0. |
| `SensorDef.extraNames` / `add_no_fail` consumers | Stored by 004720c0; no reader found in the Windows SMC loader. |
| `dwFlagsAndAttributes = 4` in the disk `CreateFileW` calls | Literal from the call sites (004756e0); meaning (FILE_ATTRIBUTE_SYSTEM) is as per the Windows headers. |
