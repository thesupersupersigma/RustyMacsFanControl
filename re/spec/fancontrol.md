# Macs Fan Control 1.5.20 (Win32) — fan control, presets, model detection: behavioural spec

Source: Ghidra decompilation (`re/out/MacsFanControl.exe/decomp.c`) cross-checked against raw x86
disassembly of `MacsFanControl/MacsFanControl.exe` wherever the decompiler dropped call arguments
(timeouts, `this` pointers). Addresses are function entry VAs (image base 0x00400000). "disasm @x"
means the fact was read from the instruction stream at that address, not from the decompiler.

Conventions: `UNKNOWN` = could not be determined, with what was tried. All integers are 32-bit
signed unless stated. C-style integer division truncates toward zero.

---------------------------------------------------------------------------------------------------

## 1. Data model

### 1.1 Fan (element of `QFanControl::fans`, std::vector, element size 0x50)

Constructor `FUN_004503c0(name)`; vector lives at `QFanControl+0x3b4/+0x3b8/+0x3bc` (begin/end/cap).

| off  | type      | meaning |
|------|-----------|---------|
| 0x00 | QString   | display name (from fanInfo.xml or SMC, see §6) |
| 0x0c | int       | kind: `0` if name == "ODD", `1` if name == "HDD", `2` if name == "CPU", else `3` (case-sensitive whole-string compare, `FUN_00450580`) |
| 0x10 | u16       | current RPM (last value read from SMC) |
| 0x12 | u16       | min RPM |
| 0x14 | u16       | max RPM |
| 0x18 | struct    | `SensorBasedControl_Simple` state, see §4.1 (0x38 bytes, ends with a QString copy of the fan name used only for logging) |

* The three RPM fields are initialised to `0xFFFF` ("no value", `DAT_0089004c`, `FUN_00458430`).
* `FUN_00458450(cur,min,max)`: always stores `cur`; stores `min`/`max` only when the new value
  != 0xFFFF (so a failed min/max read keeps the previous value).
* A static `QString DAT_0089eeb0` keeps the longest fan name seen (ctor updates it if
  `name.size() > longest.size()`); it is a UI column-width helper only.
* Fan count is held as a `u8` everywhere (loops use byte counters).

### 1.2 FanControlMode (0x20 bytes, value type)

| off  | type    | meaning |
|------|---------|---------|
| 0x00 | int     | unused/always 0 in every constructor seen |
| 0x04 | int     | type (enum below) |
| 0x08 | int     | constant RPM (type 3 only) |
| 0x0c | int     | sensor-based: start temperature, °C ("temperature that fan speed will start to increase from") |
| 0x10 | int     | sensor-based: max temperature, °C |
| 0x14 | QString | sensor-based: sensor *key* |

Type enum (from `toString` switch `FUN_0044fdc0`, ctors `FUN_0044f550/0044f740/0044f7a0`):

| value | meaning | built by |
|-------|---------|----------|
| 0 | Invalid (default ctor `FUN_0044f550`, all fields zero) | default, parse failure |
| 1 | NotAssigned | never constructed in the code read; only handled in `toString` |
| 2 | Automatic (SMC controls the fan) | `FUN_0044f740` |
| 3 | Constant RPM | `FUN_0044f7a0(rpm)` |
| 4 | Sensor-based | `fromString` / the fan dialog |

Equality `FUN_0044f5a0(a,b)`: types must be equal; for type 0/1/2 that is enough; type 3 also needs
equal `rpm`; type 4 needs equal sensor key (exact QString equality), equal start and equal max.

#### toString — `FUN_0044fdc0`

| type | output |
|------|--------|
| 0 | `INVALID` |
| 1 | empty string, plus `qDebug() << "FanControlMode::toString() - FanControlNotAssigned!"` |
| 2 | `0` |
| 3 | `1,%1` with %1 = rpm in decimal → e.g. `1,2400` |
| 4 | `2,%1,%2,%3` with %1 = sensor key, %2 = start temp (decimal int), %3 = max temp (decimal int) → e.g. `2,TC0P,50,80` |
| other | empty string |

#### fromString — `FUN_0044f800(str)` (FanControlMode.cpp)

Starts from the all-zero value (type 0 = Invalid), then:

1. `if str.size() == 1` → type = 2 (Automatic). **Any** one-character string is Automatic; the
   character is not inspected.
2. `else if str.at(0) == '1'` → type = 3; `rpm = str.split(',')[1].toInt()` (base 10; `toInt`
   failure yields 0). There is **no size check** on the split list: a string such as `"12"` indexes
   element 1 of a 1-element list (out-of-bounds read in the original; a reimplementation should
   treat it as rpm 0 or invalid — original behaviour is undefined).
3. `else if str.at(0) == '2'` → type = 4; `lst = str.split(',')`.
   * `lst.size() != 4` → log (level 3) `lst.size() is not 4`, type = 0 (Invalid).
   * else `sensorKey = lst[1]`; call the global resolver `std::function<QString(const QString&)>`
     `DAT_0089eea4(sensorKey)`. The resolver is bound in the QFanControl ctor (`FUN_00442760`) to
     `TemperatureSensorProvider::getSensorDisplayNameByKey` (`FUN_0047dc10`), which returns the
     sensor display name, or an empty string (and logs `Cannot locate sensor by key {}`) when no
     sensor matches the key or the matching sensor has its byte flag at sensor+0x50 set.
     If the function object is empty → `std::bad_function_call`.
     * result empty → log (level 3) `Failed to find {} sensor` (arg = lst[1]), type = 0.
     * else `start = lst[2].toInt()`, `max = lst[3].toInt()` (no range validation at all here).
4. otherwise (first char not '1'/'2') → stays Invalid (type 0), nothing logged.

An empty string reaches `str.at(0)` on an empty QString (undefined in the original).
The returned display name is used only as an existence test; the mode keeps the *key*.

Consequence: a saved sensor-based mode whose sensor is not present at parse time becomes Invalid,
which makes the whole profile invalid (§2.1 `isValid`).

---------------------------------------------------------------------------------------------------

## 4. SensorBasedControl_Simple::update — `FUN_00457030` (SensorControl.cpp)

Called through the Fan wrapper `FUN_00450700(fan, double temp, const FanControlMode* mode)` which
passes `this = &fan.state (fan+0x18)`, `rpmInfo = &fan.curRpm (fan+0x10)` (disasm @00450700).
Caller: the control thread, once per fan per loop iteration (§3.3), i.e. nominally every 1500 ms.

### 4.1 State (`int s[]`, at fan+0x18)

| index / off | name here | init (`FUN_00456e00`) | reset (`FUN_00456ff0`) |
|-------------|-----------|------|------|
| s[0]  +0x00 | `lastTemp` (int °C)           | -1 | -1 |
| s[1]  +0x04 | `rpm` (current target)        | -1 | -1 |
| s[2]  +0x08 | `lastSetRpm` (last returned)  | -1 | -1 |
| s[4..5] +0x10 | `lastSetTime` (int64, secs since epoch) | uninitialised by ctor (UNKNOWN garbage; in practice overwritten before first compare because `lastSetRpm != rpm` on first use) | -1 |
| s[6]  +0x18 | `tendency`: 0 Unknown, 1 Increase, 2 Decrease, 3 DoNothing (`FUN_00456f30` names: TendencyUnknown/TendencyIncrease/TendencyDecrease/TendencyDoNothing) | 0 | not touched |
| +0x1c (u8)  | `dbl` ("DOUBLE INCREASE")     | 0 | not touched |
| +0x1d (u8)  | `half` ("HALF INCREASE/DECREASE") | 0 | not touched |
| s[8]  +0x20 | `tDoNothing` QDateTime        | now | not touched |
| s[9]  +0x24 | `tIncrease` QDateTime         | now | not touched |
| s[10] +0x28 | `tDecrease` QDateTime         | now | not touched |
| s[11] +0x2c | `name` QString (fan name, log prefix `{}:`) | fan name | not touched |

Reset (`FUN_004506f0` → `FUN_00456ff0`) is called by the control thread for a fan whose sensor
cannot be found, and for every fan when entering sleep handling (§3.3).

### 4.2 Inputs

* `cur = rpmInfo[0]`, `minRpm = rpmInfo[1]`, `maxRpm = rpmInfo[2]` (u16 zero-extended to int).
* `T = (int)temp` — C truncation of the double (cvttsd2si), not rounding.
* `start = mode.startTemp`, `maxT = mode.maxTemp`.

### 4.3 Constants

| value | meaning |
|-------|---------|
| 60 s  | time in DoNothing with unchanged temperature before nudging again |
| 11 s  | hysteresis: a temperature reversal is ignored if the current tendency was entered `< 11` s ago |
| 31 s  | re-send period: the same RPM is returned again when `now - lastSetTime >= 31` s |
| +2 / -2 RPM | normal step per call |
| +1 / -1 RPM | step per call while `half` is set |
| +4 RPM | step per call while `dbl` is set (increase only) |
| `step/3` | "extra" jump when far from the band |
| `2*step` / `step` | "far from band" thresholds for increase / decrease |
| -1 | return value meaning "do not write anything" |

All time differences use `QDateTime::secsTo` (whole seconds, truncated) on wall-clock
`QDateTime::currentDateTime()`; the re-send check uses `toSecsSinceEpoch()`.

### 4.4 Pseudocode (exact order of operations)

```
fn update(s, cur, minRpm, maxRpm, temp: f64, start, maxT) -> int:
    T = trunc(temp)
    if T == 0: return -1                       # no state change at all

    range = maxT - start
    step  = trunc( (maxRpm - minRpm) as f64 / range as f64 )   # RPM per °C
            # range == 0 -> x86 cvttsd2si of inf/NaN = INT_MIN (0x80000000); UI is expected to
            # prevent it (not verified, see UNKNOWN list)
    process = true                             # "accept this temperature sample"

    # band(t): RPM band belonging to integer temperature t
    #   lo(t) = min( max( (t - start - 1) * step + minRpm, minRpm ), maxRpm - step )
    #   hi(t) = min( lo(t) + step, maxRpm )

    if T < start:                              # ---- below start temperature
        if s.rpm != minRpm:
            s.lastTemp = -1; s.lastSetRpm = -1; s.lastSetTime = -1
            s.rpm = minRpm
        goto FINAL                             # tendency/half/dbl/timestamps untouched

    if s.lastTemp == -1:                       # ---- first sample at/above start
        lo = lo(T); hi = hi(T)
        if lo < cur and cur < hi:              # actual fan RPM already strictly inside band
            s.rpm = cur
            s.tendency = DoNothing
        else:
            s.rpm = lo
            s.tendency = Increase
            s.half = 1                         # log "{}: HALF INCREASE ON"
        s.tIncrease = now                      # in BOTH branches; tDoNothing is NOT refreshed
        log "{}: Tendency is set to {}, Temperature {}, RPM={}"  (name, tendency, T, cur)

    elif s.lastTemp == T:                      # ---- temperature unchanged
        if s.tendency == DoNothing and secsTo(s.tDoNothing, now) >= 60:
            s.half = 1
            if range / 2 + start < T:          # integer division; strictly greater
                s.tendency = Increase          # log "{}: HALF INCREASE ON"
            else:
                s.tendency = Decrease          # log "{}: HALF DECREASE ON"
            # NOTE: no timestamp is updated here

    else:                                      # ---- temperature changed
        prevHalf = s.half
        s.dbl = 0; s.half = 0                  # one 16-bit store clears both
        if T < s.lastTemp:                     # log "{}: Temperature DECREASED to {}"
            match s.tendency:
              Unknown:   log(level 3) "TendencyUnknown is unexpected! (1)"
              Increase:  if secsTo(s.tIncrease, now) < 11:
                             process = false   # log "{}: Ignoring temperature change"
                         else:
                             s.tendency = DoNothing; s.tDoNothing = now
              Decrease:  s.tDecrease = now
              DoNothing: s.tDecrease = now; s.tendency = Decrease
            log "{}: Tendency is set to {}"
        else:                                  # log "{}: Temperature INCREASED to {}"
            match s.tendency:
              Unknown:   log(level 3) "TendencyUnknown is unexpected! (2)"
              Increase:  if prevHalf == 0: s.dbl = 1      # log "{}: DOUBLE INCREASE ON"
                         else:             (nothing)      # log "{}: HALF INCREASE OFF"
              Decrease:  if secsTo(s.tDecrease, now) < 11:
                             process = false   # log "{}: Ignoring temperature change"
                         else:
                             s.tIncrease = now; s.tendency = Increase
              DoNothing: if secsTo(s.tDoNothing, now) < 11:
                             process = false   # log "{}: Ignoring temperature change"
                         else:
                             s.tIncrease = now; s.tendency = Increase
            log "{}: Tendency is set to {}"
        # NOTE: when a change is ignored, half/dbl have still been cleared.

    # ---- COMPUTE (reached from the three "at/above start" branches)
    if maxT < T:
        s.rpm = maxRpm                          # immediate full speed above max temperature
    else:
        t  = process ? T : s.lastTemp
        lo = lo(t); hi = hi(t)
        if s.tendency == Increase:
            if 2*step < lo - s.rpm:             # far below the band
                s.rpm += step / 3               # log "{}: Adding extra {} from current rpm {}"
                                                #   (extra, rpm AFTER the addition)
            next = s.rpm + 2
            if s.dbl:        next = s.rpm + 4
            elif s.half:     next = s.rpm + 1
            s.rpm = min(next, hi)
            if s.rpm == hi:
                s.tendency = DoNothing; s.tDoNothing = now
                log "{}: PEEK MAX RPM '{}' for current temperature, do nothing now" (hi)
        elif s.tendency == Decrease:
            if step < s.rpm - hi:               # far above the band (compared with hi, not lo)
                s.rpm -= step / 3               # log "{}: Subtracting extra {} from current rpm {}"
            next = s.rpm - 2
            if s.half:       next = s.rpm - 1   # dbl is ignored when decreasing
            s.rpm = max(next, lo)
            if s.rpm == lo:
                s.tendency = DoNothing; s.tDoNothing = now
                log "{}: PEEK MIN RPM '{}' for current temperature, do nothing now" (lo)
        # tendency DoNothing / Unknown: rpm unchanged
    if process: s.lastTemp = T                  # also executed in the maxT < T case

FINAL:
    if s.lastSetRpm == s.rpm:
        if (low 32 bits of s.lastSetTime) == -1: return -1
        nowS = secsSinceEpoch(now)
        if nowS - s.lastSetTime < 31: return -1     # signed 64-bit compare; also -1 if clock went back
        s.lastSetTime = nowS                        # periodic re-send
    else:
        s.lastSetRpm = s.rpm                        # log "{}: Setting fan speed to {}"
        s.lastSetTime = secsSinceEpoch(now)
    return s.rpm
```

Notes that matter for bit-exact reproduction:

* The mapping is **not** a direct temperature→RPM function. Temperature only selects a band
  `[lo(t), hi(t)]` one `step` wide; the target RPM creeps toward `hi` (when increasing) or `lo`
  (when decreasing) by 1, 2 or 4 RPM **per call**, plus `step/3` jumps while far away. With the
  1500 ms loop this is ~0.67–2.67 RPM/s plus jumps.
* Band for `T == start` is `[minRpm, minRpm+step]` because of the `-1` and the lower clamp; for
  `T == start+1` it is the same band; from then on it moves up one `step` per °C; the top band is
  `[maxRpm-step, maxRpm]`.
* Above `maxT` the output is exactly `maxRpm` in one step; `tendency` is left as it was.
* Below `start` the output is exactly `minRpm`; the first time this branch is hit `lastTemp` is
  reset to -1 so that the next sample ≥ start re-runs the "first sample" branch. If the state
  already had `rpm == minRpm` (e.g. it crept down to min while still ≥ start) nothing is reset and
  `lastTemp`/`tendency` persist.
* In the "first sample" branch with `cur` inside the band, `tDoNothing` keeps its previous value
  (construction time or an older DoNothing entry), so the 60 s and 11 s checks run against that
  old timestamp.
* The value returned is the target `rpm`, only when it changed since the last return or 31 s have
  elapsed; the caller writes it to SMC (§3.3). The value is never rounded to a multiple of
  anything.
* The 0-temperature guard exists twice: the caller skips `update` when the sensor's double is
  exactly `0.0` (`ucomisd` against `DAT_00758230` = 0.0, disasm @00449167), and `update` returns -1
  when `trunc(temp) == 0` (so 0 < temp < 1 is also ignored).

---------------------------------------------------------------------------------------------------

## 6. Model detection and fan names

### 6.1 Model descriptor

The application object (`QCoreApplication::instance()`) holds a model descriptor at `app+0x38`
(passed as `this` to the matcher, disasm @00450859):

| off  | type | meaning |
|------|------|---------|
| 0x00 | u8   | family enum (below) |
| 0x02 | u16  | numeric id = all decimal digits of the model identifier concatenated and parsed as one unsigned number: `MacBookPro18,1` → 181, `Macmini8,1` → 81, `iMac21,1` → 211 |
| 0x10 | ptr  | → object whose QString at +4 is the full model identifier string (e.g. `MacBookPro11,3`) |

Family enum and parser `FUN_004b2900(str, &family, &num)` — first matching **prefix**
(`QString::startsWith`, case-sensitive), tested in this order:

`Macmini`→6, `MacPro`→8, `MacBookPro`→3, `MacBookNeo`→2, `MacBookAir`→1, `MacBook`→0,
`iMacPro`→5, `iMac`→4, `Xserve`→9, `ADP`→10, `MacStudio`→7; no match leaves the caller's initial
value (11 = unknown). `num = str.replace(QRegularExpression("[^0-9]"), "").toUInt()` truncated to
u16. Returns false if the string is empty, the family is 11, or num == 0.

How the model identifier string itself is obtained on Windows: see §6.4.

### 6.2 fanInfo.xml matching — `FUN_00450730` (loader) + `FUN_004b1ee0` (matcher)

* Resource `:/Resources/xml/fanInfo.xml.gz` is read (`FUN_0059a0a0`), gunzipped (`FUN_005bcde0`)
  and parsed (tinyxml2-style API: `FUN_00528640` Parse, `FUN_00526c10` FirstChildElement,
  `FUN_005283e0` NextSiblingElement, `FUN_00525970` Attribute, `FUN_005272b0` GetText).
* Iterate root's `<modelMatch>` children in document order. For each, read attribute `models`
  (skip element if absent) and call `matcher(models)`. **The first element that matches wins.**
* Matcher `FUN_004b1ee0(const char* models)` (this = model descriptor):
  1. `strcmp(models, "all") == 0` → match.
  2. Split on `';'` (QString::split, empty parts kept). For each entry:
     * if the entry contains `+` anywhere (`indexOf("+") != -1`): parse the entry with
       `FUN_004b2900` (the `+` and `,` are removed by the digit filter). Match iff parse succeeded
       **and** `descriptor.family == parsedFamily` **and** `descriptor.num >= parsedNum` (unsigned
       16-bit compare). So `MacBookPro18,1+` = any MacBookPro whose digit-concatenated id ≥ 181.
       Note the comparison is on the concatenated number, not on (major, minor): `Macmini10,1`
       (101) matches `Macmini8,1+` (81), but a hypothetical `X9,10` (910) would rank above
       `X10,1` (101).
     * else: match iff the entry equals the full model identifier string exactly
       (case-sensitive, same length).
  3. No entry matched → no match.
* On a match: for `i` in 0..=3 look up child element named `fan%1` (i decimal: `fan0`..`fan3`);
  each one that exists appends its text to the name list (so at most 4 names; a missing index is
  skipped, which shifts later names down). No match / parse failure / empty resource → empty list.

### 6.3 Fan naming and the fallback — `QFanControl::loadFansFromSMC` `FUN_0044be90`

For fan index `i` (0..count-1): `name = list[i]` if `i < list.size()` (`FUN_00450ca0`), else empty.
If empty → `name = QString::fromStdString(Service::getFanName(i))` (`FUN_004ad610`, SMC layer; its
own fallback text is specified with the SMC service). Log `Fan {}: {}` (i, name).

Fan-less models: `FUN_004b28c0(descriptor)` returns true for family 1 (MacBookAir) with num ≥ 101
(MacBookAir10,1 and later), any family 2 (MacBookNeo), and family 0 (MacBook) with num ∈
{81, 91, 101} (MacBook8,1 / 9,1 / 10,1). When true, `loadFansFromSMC` does nothing at all (no log,
no SMC access, zero fans).

