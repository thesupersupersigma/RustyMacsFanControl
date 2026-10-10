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

Otherwise: log `QFanControl::loadFansFromSMC`; `count = Service::getFanCount()` (`FUN_004ad0e0`,
u8); log `Fans count - {}`; if 0 → log (level 4) `No fans detected!` and return. Else load the
name list (§6.2), and per fan in index order: resolve name, construct Fan, push to the vector, then
`FUN_00445b00(i, false)` = three reads through `FUN_004addc0(i, kind, &u16)` (which builds the key
with `FUN_004afb10(kind, i)` and calls `Service::getFanRPM` `FUN_004adb00`): kind 0 = current,
kind 1 = min, kind 2 = max (kind 3 = target is the one written by setFanRPM). After all fans:
build the predefined presets (`FUN_0044ad60`, §2.3) and run `checkFansControlModeFromSMC` (§3.6).

### 6.4 Obtaining the model identifier on Windows — `platform_getMachineName` `FUN_004b35c0`

(sysinfo_win.cpp; logs under the names `platform_getMachineName`, `readFromBIOS`,
`readFromRegistry`.) Produces `(manufacturer, productName)` by the first source that works:

1. `readFromBIOS` `FUN_004b39c0`: a function-static SMBIOS reader (`FUN_0058e850` →
   `FUN_0058e0d0`, the only user of `GetSystemFirmwareTable`) exposes two `std::string`s
   (manufacturer @0x0089f848, product name @0x0089f860; SMBIOS type-1 System Information fields —
   the parsing itself was not read). Logs `Manufacturer: {}` and `Product Name: {}` when the
   manufacturer is non-empty. Success iff both strings are non-empty.
2. else WMI `FUN_0058b820` (`SELECT * FROM Win32_ComputerSystem`, properties `Manufacturer`,
   `Model`); used if both come back non-empty.
3. else `readFromRegistry` `FUN_004b3e20`: `HKLM\SYSTEM\CurrentControlSet\Control\SystemInformation`
   opened with `KEY_READ` (0x20019); values `SystemManufacturer` and `SystemProductName`
   (REG_SZ, UTF-16); logs both; failure to open logs (level 3) `Unable to open registry key`.

Then: if `manufacturer.indexOf("Apple")` (case-sensitive) != -1 → the model identifier is
`productName` (e.g. `MacBookPro11,3`); otherwise log (level 3)
`Manufacturer string doesn't contain Apple ('{}')` and the model identifier is the empty string.
The descriptor of §6.1 is then built from that string (family/num via `FUN_004b2900`).
UNKNOWN: the exact place where `platform_getMachineName`'s result is stored into `app+0x38`
(the app constructor was not traced); the field meanings were recovered from their consumers.

---------------------------------------------------------------------------------------------------

## 3. QFanControl: object, threads, control loop

### 3.1 Object layout (ctor `FUN_00442760`)

| off | member |
|-----|--------|
| 0x08 | `QFanProfileList*` (0x58 bytes, §2) |
| 0x0c | SMC service pointer (copied from `app+0x28`) |
| 0x10 | `TemperatureSensorProvider*` |
| 0x14 | object from `FUN_00458a20` (passed to the sensor provider's load/clear; not analysed) |
| 0x18 / 0x1c | `m_ThreadInit` (std::future state ptr / valid flag) |
| 0x20 / 0x24 | `m_ThreadUpdate` |
| 0x28 / 0x2c | `m_ThreadControl` |
| 0x30 / 0x34 | `m_ThreadCpuUsage` |
| 0x38 | event `stop` |
| 0xcc | event `initDone` |
| 0x160 | event `pause` |
| 0x1f4 | event `resume` |
| 0x288 | event `pauseAck` |
| 0x31c | event `wakeup` |
| 0x3b0 | atomic u8 `pausedCount` |
| 0x3b1 | u8 `sleepMode` |
| 0x3b2 | u8 `userLoggedOut` |
| 0x3b3 | u8 `cpuUsage`, initial 0xFF |
| 0x3b4.. | `std::vector<Fan>` |

Events are a home-made manual-reset event (mutex + condvar; `set` = `FUN_0055a610`,
`reset` = `FUN_0055a600`, `isSet` = `FUN_0055a2e0`, `wait(ms)` = `FUN_0055ae40` where
`(0xFFFF, 0)` means infinite, `waitAny(vector<Event*>, waitAll=0, ms)` = `FUN_0055aca0` →
`FUN_0055a810`, returning the index of a signalled event; the callers only test "== 0", i.e.
"the stop event", everything else incl. timeout means "keep going"). All six are created with
`FUN_00559f80(0)` (manual reset).

Connections made in the ctor: power-notification object (`FUN_0059ee40`) signals →
`slotEnteringSleepMode` `FUN_0044d100`, `slotWakeUp` `FUN_0044d5d0`, `slotUserLoggedOut`
`FUN_0044d420`; sensor provider "sensors changed" signal → `FUN_0044d2b0` (§3.7). The global
sensor-name resolver used by `FanControlMode::fromString` is installed here.

### 3.2 Start / stop

* `Init` `FUN_004457e0` (called from main-window setup `FUN_00461d00`):
  `m_ThreadInit = std::async(launch::async, initialLoadThread)`.
* `initialLoadThread` `FUN_0044b0d0` (thread name `initialLoadThread`), runs once:
  1. `initDone.reset()`.
  2. `loadFansFromSMC()` (§6.3), then `sensorProvider->load(smc, this+0x14)` (`FUN_0047e120`).
  3. If the fan vector is empty → log (level 3) `initialLoadThread - no fans`. Else log
     `loadFansProfiles`; and if the model is not fan-less (`FUN_004b28c0`) **and** the dynamic app
     property `win-no-support` is unset (equals an invalid QVariant): `profileList->load()`
     (`FUN_00453b50`, §2.5) and `applyFanProfileToSMC(currentProfile, false)` (§3.5).
  4. If `stop` is not set → emit signal 0 `initFinished(uint8_t)` with argument 0 (`FUN_004c8880`).
     `initDone.set()`. Log `initialLoadThread finished`.
  (An app property `smc-dump` is also read at the top; its result is not used in this function.)
* `startThreads` `FUN_0044d7a0` (called from the main window, `FUN_00463200` and `FUN_00467390`):
  1. if `m_ThreadUpdate` is not running → `std::async(launch::async, updateThread)`.
  2. if `m_ThreadControl` is not running: if `app->FUN_004348f0()` is false → log (level 3)
     `Helper tool is not installed, thread cannot start`; else
     `std::async(launch::async, controlFanSpeedThread)`.
  "Running" = future valid and `wait_for(0)` not ready (`FUN_0043fe00`).
  `m_ThreadCpuUsage` is never assigned anywhere (§7).
* `stopAndWaitForFinished` `FUN_0044db90` → bool:
  `stop.set()`; budget = 8000 ms; loop: if none of the four futures is still running → log
  `stopAndWaitForFinished: Quit OK`, return true. Otherwise log
  `stopAndWaitForFinished: m_ThreadUpdate = {}, m_ThreadInit = {}, m_ThreadControl = {}, m_ThreadCpuUsage = {}`
  (booleans "still running"), `sleep_for(200 ms)`, budget -= 200, repeat while budget > 0.
  On exhaustion: log (level 4) `stopAndWaitForFinished ERROR: ...` (same fields), return false.
  The app-exit path `FUN_00463330` shows a message box
  (`Unable to terminate the app in the right way, ...`) when it returns false.
* Restart path `FUN_0044ca80` (no static caller found; presumably virtual/slot):
  `blockSignals(true)`, `stopAndWaitForFinished()`, clear fan vector, `sensorProvider->clear()`
  (`FUN_0047c9e0`), `profileList->clear()` (`FUN_00453570`), `stop.reset()`,
  `blockSignals(false)`, then `initialLoadThread()` synchronously on the calling thread.

Thread exit ordering: there is no join order; all threads observe the same `stop` event. Only the
control thread performs an action on exit (`restoreDefaultControl`, §3.4).

### 3.3 controlFanSpeedThread — `FUN_00448f40` (thread name `controlFanSpeedThread`)

`N` = fan count captured once in a function-static u8 on first run.

```
loop:
  if pause.isSet():                                  # cooperative pause, see §3.7
      n = (m_ThreadControl running ? 1 : 0)          # FUN_00440e00(&m_ThreadControl)
      if ++pausedCount == n + 1: pauseAck.set()
      resume.wait(INFINITE)
      --pausedCount

  for i in 0..N:                                     # u8 counter
      if sleepMode: break
      profile = profileList->currentProfile()        # copy under the list mutex, FUN_00453c60
      mode = profile.isValid() ? profile.mode(i) : Invalid      # FUN_004525d0 / FUN_00452470
      match mode.type:
        3 (Constant):
            target  = mode.rpm
            changed = profileList->changedFlag       # atomic u8 at list+0x24
            if abs(fan[i].curRpm - target) > 30 or changed:
                setFanSpeed(i, target)               # §3.5
                if changed: profileList->changedFlag = 0     # atomic exchange
        4 (Sensor-based):
            sensor = sensorProvider->findByKey(mode.sensorKey)   # FUN_0047ef20: first sensor whose
                                                     # virtual matchesKey() is true and whose
                                                     # byte flag at sensor+0x50 is 0
            if sensor == null:
                fan[i].state.reset()                 # FUN_004506f0
                log "ControlFanSpeedThread: reset fan mode for {}"   (fan name)
                p = profileList->currentProfile(); p.setMode(i, Automatic)   # FUN_00452760
                profileList->setCurrentProfile(p)    # FUN_004559b0 (sets changedFlag, persists, §2.6)
                applyFanProfileToSMC(profileList->currentProfile(), false)
            else:
                t = sensor.temperature               # double at sensor+0x10
                if t != 0.0:
                    r = fan[i].update(t, &mode)      # §4
                    if r != -1: setFanSpeed(i, r)
        other (Automatic/Invalid): nothing
      if stop.isSet(): goto EXIT

  if sleepMode:
      log "ControlFanSpeedThread: sleep mode"
      for every fan: state.reset()
      cur        = profileList->currentProfile()
      sleepProf  = getProfileToApplyForSleepMode()   # §3.8
      changed    = !(cur == sleepProf)               # FanProfile::operator==, §2.1
      log "ControlFanSpeedThread: changed = {}"
      if changed: applyFanProfileToSMC(sleepProf, true)
      wakeup.wait(INFINITE)                          # NOTE: `stop` is not observed here
      log "_wakeupEvent triggered"
      wakeup.reset()
      if changed:
          log "Restore previous profile"
          applyFanProfileToSMC(cur, false)

  if waitAny([stop, pause], 1500 ms) == 0: goto EXIT    # 0x5dc, disasm @00449bfb
  goto loop

EXIT:
  restoreDefaultControl()                            # §3.4
```

The loop period is therefore 1500 ms plus the time spent in SMC I/O. There is no SMC read in this
thread: `fan[i].curRpm` and the sensor temperature are whatever `updateThread` last stored.

Constant-RPM consequence: a constant target is written once when the profile changes
(`changedFlag`) and re-written on every 1.5 s pass for as long as the measured RPM differs from
the target by more than 30 RPM. `changedFlag` is one flag for the whole list, cleared by the first
constant fan processed; later constant fans in the same pass are only written if their own
deviation exceeds 30.

### 3.4 restoreDefaultControl — `FUN_0044cc60`

Log `restore default control?`. If `userLoggedOut` **and** `FUN_004b2760(model)` → log `no` and
do nothing (fans keep their forced state). Otherwise log `yes` and
`applyFanProfileToSMC(profileList->predefined(0) /*Automatic*/, false)`, i.e. one
`setFansControlModes` call with every bit clear.

`FUN_004b2760` = family iMac (4) with num in 101..=122 (iMac10,1 … iMac12,2) or num ∈
{132, 142, 151, 171, 183, 192}.

`slotUserLoggedOut` `FUN_0044d420` only logs `slotUserLoggedOut` and sets `userLoggedOut = 1`
(never cleared).

### 3.5 SMC writes

* `applyFanProfileToSMC(profile, bool writeConstants)` `FUN_004485b0`:
  log `Apply profile: ` + profile name. Build `std::vector<bool> forced(fanCount)`; for each fan
  `forced[i] = (profile.mode(i).type != 2)` — Automatic clears the bit, **every other type
  including Invalid sets it**. Call `Service::setFansControlModes(forced)` (`FUN_004b05f0`) once.
  If `writeConstants` and at least one mode is Constant: for each fan with type 3, in index
  order, `Service::setFanRPM(i, 3 /*target*/, mode.rpm)` (`FUN_004b02e0`).
  `writeConstants` is true only for the sleep profile (§3.3).
* `setFanSpeed(i, rpm)` `FUN_00445980`: function-static `lastModesApply` (initialised to
  now − 3001 ms). If `msecsTo(lastModesApply, now) > 3000` →
  `applyFanProfileToSMC(currentProfile, false)` and `lastModesApply = now`. Then always
  `Service::setFanRPM(i, 3, rpm)`. So the forced-mode mask is re-asserted at most every 3 s while
  RPM writes are happening. (`FUN_0044e310` is the same throttled re-apply as a standalone
  function sharing the same static; no static caller was found.)
* Public mutators that write immediately (all end with
  `applyFanProfileToSMC(currentProfile, false)`): `FUN_004488f0` (set current profile),
  `FUN_0044c7f0` (set one fan's mode, optionally saving to the named preset, §2.6),
  `FUN_0044a290` (delete a custom preset, §2.6; re-applies only if the current profile changed), `FUN_0044c510` (reload profiles).

### 3.6 "Fan mode set by another program" — `checkFansControlModeFromSMC` `FUN_00448980`

Called exactly once, at the end of `loadFansFromSMC` (startup / restart). Log
`checkFansControlModeFromSMC`; `ok = Service::getFansControlModes(&vector<bool>)`
(`FUN_004adeb0`). If `!ok` → log (level 4) `Unable to query fans control mode!`. Else for each
index whose bit is set → log (level 3) `Fan {} mode is set by another program`. If any bit was set
→ `applyFanProfileToSMC(predefined(0) /*Automatic*/, false)` (reset all fans to automatic).
There is no periodic detection while running; the running loop simply re-asserts its own mask
(§3.5).

### 3.7 updateThread — `FUN_0044e460` (thread name `updateThread`)

```
sensors = sensorProvider->sensors()                 # FUN_0047d750
first = true
loop:
  if pause.isSet(): (same pause protocol as §3.3); sensors = sensorProvider->sensors()
  visible   = first ? true : mainWindow->isVisible()
  traySensor = settings.value("traySensor").toString()     # read every pass (FUN_005988a0)
  needed = []                                       # sensor keys required by the profile
  for i in 0..N:
      cur = getFanRPM(i, kind 0)                    # FUN_004addc0(i, 0, &u16)
      fan[i].set(cur, 0xFFFF, 0xFFFF)               # min/max untouched
      if !visible:
          m = profileList->currentProfile().mode(i)
          if m.type == 4: needed.append(m.sensorKey)
      if stop.isSet(): return
      if pause.isSet(): break
      emit fanUpdate()  (signal 1, FUN_004c8840)    # no arguments seen
  if pause.isSet(): continue loop
  if needsWakeWorkaround and profileList->isCurrentPredefinedAutomatic()   # FUN_00453da0
        and powerSource() == 1 /*on battery*/:      # FUN_0059ea40: GetSystemPowerStatus,
        wakeSMCOnMacBookWorkaround()                #   ACLineStatus 0→1, 1→0, else 2
  for s in sensors:
      doUpdate = true
      if !visible:
          doUpdate = (s.key == traySensor) or needed.contains(s.key)   # key QString at sensor+0x24
      if doUpdate: s->update()                      # virtual slot 3 (vtable+0xc)
      if stop.isSet(): return
      if pause.isSet(): break
      if doUpdate: emit sensor_update(s.id /*u32 at sensor+8*/, s.temperature /*double +0x10*/)  # signal 2, FUN_004c8bb0
  if pause.isSet(): continue loop
  r = waitAny([stop, pause], 1500 ms)               # 0x5dc
  if r == 0: return
  first = false
```

Period 1500 ms. When the main window is hidden only the fans' current RPM, the tray sensor and
the sensors referenced by sensor-based modes are refreshed.

Wake workaround: `needsWakeWorkaround` (function-static, computed once) = MacBookPro with num 121
(MacBookPro12,1) or MacBookAir with num ∈ {62, 71, 72} (MacBookAir6,2 / 7,1 / 7,2).
`wakeSMCOnMacBookWorkaround` `FUN_0044f1d0`: function-static `lastWake` initialised to
now − 10001 ms; if `msecsTo(lastWake, now) > 10000`: log `Waking SMC system`;
`setFansControlModes(all bits = 1)`; `sleep_for(500 ms)`; `setFansControlModes(all bits = 0)`;
`lastWake = now` (the timestamp taken on entry). It runs only while the active preset is the
predefined Automatic one and the machine is on battery, i.e. at most once per ~10 s.

### 3.8 Sleep / wake

* `slotEnteringSleepMode` `FUN_0044d100`: `sleepMode = 1`; log `slotEnteringSleepMode`. Nothing
  else — the control thread notices the flag on its next pass (≤ 1.5 s later).
* `slotWakeUp` `FUN_0044d5d0`: `sleepMode = 0`; log `slotWakeUp`; `wakeup.set()`.
* `getProfileToApplyForSleepMode` `FUN_0044a5c0`: result = copy of predefined Automatic.
  Two function-statics: `noSleepProfile = FUN_004b26c0(model)`, `forceMin = FUN_004b2760(model)`
  (the iMac list of §3.4). If `!noSleepProfile`: for each fan i, with
  `m = currentProfile valid ? currentProfile.mode(i) : Invalid`: if `m.type == 4` **or**
  `forceMin` → log `getProfileToApplyForSleepMode: set min rpm for the {} fan` and
  `result.mode[i] = Constant(fan[i].minRpm)`.
  `FUN_004b26c0` = MacBookAir with num 81, or MacBookPro with num ∈
  {121, 131, 132, 141, 142, 152, 162, 163} or num ≥ 171.
  Net effect on sleep: sensor-controlled fans (all fans on the listed iMacs) are pinned to their
  minimum RPM, constant-RPM fans are released to Automatic, and the user's profile is re-applied
  on wake (mode mask only; constants are then rewritten by the normal loop because the measured
  RPM differs by > 30 — `changedFlag` is not set by this path).

### 3.9 Sensor reload pause — `FUN_0044d2b0`

While `pausedCount != 0`: `sleep_for(10 ms)` and `qDebug() << "wait!"`. Then `resume.reset()`,
`pause.set()`, `pauseAck.wait(INFINITE)`; emit signal 4 `sensorsReloadBegin` (`FUN_004c8c10`),
`sensorProvider->clear()`, `sensorProvider->load()`, emit signal 5 (the one after `sensorsReloadBegin`, `FUN_004c8c30`); finally
`pause.reset()`, `pauseAck.reset()`, `resume.set()`. Both worker threads park in
`resume.wait(INFINITE)` meanwhile; `pauseAck` is set when the number of parked threads equals
1 + (control thread running ? 1 : 0).

---------------------------------------------------------------------------------------------------

## 7. CPU-usage thread — `FUN_0044a0e0` (thread name `cpuUsageThread`)

Body: `SetThreadPriority(GetCurrentThread(), -2)` (`FUN_0059fb20`); then loop:
`v = provider->vfunc1()` (u8; provider pointer is `*(app+0x48)`); if `v != cpuUsage`
(member +0x3b3, initial 0xFF) → store it and emit signal 3 `cpuUsageUpdated(uint8_t)`
(`FUN_004c8800`); `stop.wait(1000 ms)`; exit when `stop` is set. Period 1000 ms.

In this Windows build the thread is dead code:

* no call, jump, thunk or function-pointer immediate references `FUN_0044a0e0` (whole `.text`
  scanned for E8/E9 rel32 and for the absolute address), and `m_ThreadCpuUsage` (+0x30) is only
  ever read (in `stopAndWaitForFinished`);
* the only consumer of `cpuUsageUpdated` is the main-window slot `cpuUsageUpdate`
  `FUN_004634d0`, whose body is an empty `return`;
* the import table has no `GetSystemTimes` / PDH / `NtQuerySystemInformation`.

UNKNOWN: what the provider at `app+0x48` would measure (its vtable was not located). A
reimplementation can omit the thread; the log line in `stopAndWaitForFinished` always reports
`m_ThreadCpuUsage = false`.

---------------------------------------------------------------------------------------------------

## 2. FanProfile and QFanProfileList

### 2.1 FanProfile (0x20 bytes) / FanProfilePredefined (0x24 bytes)

| off  | type | meaning |
|------|------|---------|
| 0x00 | vptr | `FanProfile::vftable` / `FanProfilePredefined::vftable`; slot 0 = deleting dtor, slot 1 = `getName()` |
| 0x04 | ptr  | cached translator pointer (predefined names only) |
| 0x08 | QString | name |
| 0x14 | QList<FanControlMode> | one mode per fan (d / ptr / size at 0x14 / 0x18 / 0x1c) |
| 0x20 | u8 | *Predefined only*: id, `0` = Automatic, `1` = Full blast (ctor default 0xFF) |

* Default ctor `FUN_00451430`: empty name, no modes.
* Copy ctor `FUN_00451100` and assignment `FUN_004516d0`/`FUN_004517f0`: copy modes and name;
  **if the resulting name is empty it becomes the literal `UNSAVED`**.
* Named ctor `FUN_00451270(modes, name)`: name with every `|` replaced by `-`.
* `setMode(i, mode)` `FUN_00452760`; `mode(i)` `FUN_00452470` (no bounds check).
* `operator==` `FUN_00451730`: same number of modes and every pair equal per §1.2. **Names are
  not compared.**
* `isValid()` `FUN_004525d0`: `modes.size() == fanCount` (static u8 `DAT_0089eec0`, set when
  the predefined presets are built) **and** `getName()` non-empty **and** no mode has type 0.
* Predefined `getName()` `FUN_004524d0`: id 0 → translation of `Presets/presetAutomatic`
  (English `Automatic`), id 1 → `Presets/presetFullBlast` (English `Full blast`), else `Invalid`.
  The base-class `getName()` returning the stored name was not read (inferred from its use in
  `toString`).

### 2.2 Serialised profile string

Separator `SEP` = `|` (static QString `DAT_0089eec4`, initialiser @004210c0).

`toString()` `FUN_00452820`:
```
plain = getName() + "|" + mode[0].toString() + "|" + ... + mode[n-1].toString()
        (built with a trailing "|" that is then cut off)
result = Base64( UTF-8(plain) )      # standard alphabet A–Z a–z 0–9 + /, '=' padding (FUN_0055f100)
```
`fromString(str)` `FUN_00451850` (FanProfile.cpp):
1. `plain = fromUtf8( Base64Decode( str.toUtf8() ) )` (`FUN_00556130`; behaviour on malformed
   Base64 not analysed).
2. `parts = plain.split("|", Qt::SkipEmptyParts, Qt::CaseSensitive)` (flags read from disasm
   @004519a0).
3. Expected count = `fanCount + 1` (function-static captured on first call). If
   `parts.size() != expected` → log (level 3) `Unexpected profile string: {}` (the encoded input)
   → return an empty profile (which is not valid).
4. `name = parts[0]`; for each further part `m = FanControlMode::fromString(part)`; if
   `m.type == 0` → log (level 3) `Unexpected profile mode: {}` (the part) → return an empty
   profile. Otherwise append.
5. Return `FanProfile(modes, name)` (named ctor, then copied: empty name → `UNSAVED`).

Because empty parts are skipped, an empty name or an empty mode string changes the count and the
profile is rejected. Rename (§2.6) stores the new name without replacing `|`, so a preset renamed
to contain `|` will fail to load next time (count mismatch).

Example, 2 fans, preset "Quiet": fan 0 constant 1800 RPM, fan 1 driven by sensor key `TC0P`
from 50 °C to 80 °C (the key is illustrative; real keys come from the sensor provider):
```
plain  : Quiet|1,1800|2,TC0P,50,80
stored : UXVpZXR8MSwxODAwfDIsVEMwUCw1MCw4MA==
```

### 2.3 Predefined presets — `FUN_0044ad60` (called from `loadFansFromSMC`)

Built once per fan load, appended to the predefined list by `FUN_004536a0(modes, id)` (which also
sets the static `fanCount = modes.size()`):

* id 0 **Automatic**: every fan `Automatic` (type 2).
* id 1 **Full blast**: every fan `Constant(fan.maxRpm)` (u16 at fan+0x14, as read from SMC at load).

### 2.4 QFanProfileList (QObject, 0x58 bytes, ctor `FUN_00453120`)

| off  | member |
|------|--------|
| 0x08 | `FanProfile* current` — points into one of the two lists, or to a heap object owned by the list (the "unsaved" profile) |
| 0x0c | `QList<FanProfilePredefined*> predefined` |
| 0x18 | `QList<FanProfile*> custom` |
| 0x24 | atomic u8 `changed` (consumed by the control thread, §3.3) |
| 0x28 | std::mutex guarding `current` |

Classification of `current` (`FUN_00452aa0` = pointer search): in `predefined` → predefined
(`FUN_00453e20`; `FUN_00453da0` = "is Automatic", `FUN_00453e90` = "is Full blast"); else in
`custom` → custom; else → **unsaved** (`FUN_00453f10`). The UI shows the unsaved state with the
translation `Presets/presetCurrentUnsaved` = `Custom*` (menu builder `FUN_004c74d0`).

`currentProfile()` `FUN_00453c60` returns a copy (empty profile if `current` is null);
`predefined(id)` `FUN_00453d40` returns a copy of the predefined entry with that id.

### 2.5 Loading — `load()` `FUN_00453b50`

1. If the app property `pro-version` is set (not an invalid QVariant) →
   `loadCustomProfiles()` `FUN_00454670`: `list = settings.value("CustomPresets")` as
   QStringList (`FUN_005989b0`); log `loadCustomProfiles = {}` (count); for each string in order:
   `p = FanProfile::fromString(s)`; if `p.isValid()` → log `Profile {} is loaded` and append a
   heap copy to `custom`; else log `Failed to read profile {}` and skip it (so indices of later
   presets shift). Without `pro-version` the custom list stays empty.
2. Restore the active preset `FUN_00453fa0`: `candidate` = copy of predefined Automatic;
   `s = settings.value("ActivePreset").toString()`.
   * `s` empty → legacy migration `FUN_00454350`: modes = `fanCount` × Invalid; for each fan `i`
     read `settings.value("Fan_%1")` (i decimal, `Fan_0`, `Fan_1`, …; the pre-preset per-fan
     format, value = a FanControlMode string of §1.2); if non-empty and it parses to type != 0,
     store it. Build `FanProfile(modes, "old_guy")`. If that profile `isValid()` (all fans had a
     parseable entry) → `candidate` = it. **`old_guy` is therefore the internal name of the
     profile synthesised from legacy `Fan_N` values**; since it normally matches no list entry it
     becomes the unsaved profile and is written back as `Unsaved:…` by step 3.
   * else if `s` contains `:` → `kind = s.left(idx)`, `value = s.mid(idx+1)`
     (first `:` only; exact case-sensitive compares):
     * `Predefined` → the predefined entry whose `id == value.toUInt()`; none → keep Automatic.
     * `Custom` → `n = value.toUInt()`; if `custom.size() != 0 && n < custom.size()` →
       `custom[n]`; else keep Automatic.
     * `Unsaved` → `p = FanProfile::fromString(value)`; if `p.isValid()` → `p`.
     * anything else, or no `:` → keep Automatic.
3. `setCurrentProfile(candidate)` (§2.6) — which also writes the settings back.

### 2.6 Mutations

`setCurrentProfile(p)` `FUN_004559b0` (also the Qt slot, moc index 1):
1. If `current` is the unsaved (owned) object → delete it. `current = null`.
2. Scan `predefined`, then (only if nothing matched) `custom`, for entries with `entry == p`
   (**modes only**); the loops do not break, so the *last* equal entry of the first list that
   has a match wins.
3. No match → `current = new FanProfile(p)` (unsaved; keeps `p`'s name, `UNSAVED` if empty).
4. `changed = 1`.
5. Persist: if `pro-version` is set → `saveCustom()`; always `saveActive()`. Emit
   the list's change signal (signal 0, `FUN_004cea70`; its real name was not recovered — called
   `currentChanged` below; the presets widget consumes it in `slotProfilesChanged`).

Consequence: editing one fan of preset "Quiet" (via `FUN_0044c7f0`) yields an unsaved profile
still named `Quiet`; editing it back to the original values snaps back to the saved preset.

`saveActive()` `FUN_00454f70`: skipped entirely when the app property `win-no-support` is set or
`current` is null. Otherwise `settings.setValue("ActivePreset", "%1:%2")` with
* predefined → `Predefined:<id>` (`Predefined:0`, `Predefined:1`)
* custom → `Custom:<index in the custom list>` (decimal)
* unsaved → `Unsaved:<toString() of §2.2>`

`saveCustom()` `FUN_00455210`: `settings.setValue("CustomPresets", QStringList)` with
`toString()` of every custom preset in list order (`FUN_00599bb0`).

Save unsaved as preset `FUN_00455640(name)` (slot, moc index 2; also called from
`FUN_0044c7f0` when the dialog's "save to current preset" box is ticked): under the mutex, look
for a custom preset whose `getName()` equals `name` exactly. Found → replace its modes with the
current profile's modes, delete the old `current` object, `current` = that preset. Not found →
set `current.name = name` (raw) and append `current` itself to `custom`. Then persist (custom if
`pro-version`, active always) and emit `currentChanged`.

Rename `FUN_00454e90(profile, newName)` (slot, moc index 3): first custom preset whose modes
equal `profile` gets `name = newName` (no `|` filtering); persist (`FUN_00455560`); emit.

Delete `FUN_004537e0(profile)` (via `QFanControl` `FUN_0044a290`): index = first custom preset
whose modes equal `profile` (no "not found" handling); if the current profile equals `profile` →
with signals blocked `setCurrentProfile(predefined Automatic)`; delete and remove the entry;
persist; emit. Custom indices after it shift down, and `ActivePreset` is rewritten in the same
call, so a stored `Custom:<n>` stays consistent.

Clear `FUN_00453570`: persist first, then delete the unsaved object and every list entry.

All identity lookups for rename/delete are by mode equality, so two custom presets with identical
modes are indistinguishable (the first one is affected).

### 2.7 Registry picture

QSettings native format, `HKCU\Software\CrystalIdea Software\Macs Fan Control`
(the settings initialiser `FUN_00433790` passes organisation `CrystalIDEA Software`, application
`Macs Fan Control`):

```
ActivePreset   REG_SZ        Custom:0
CustomPresets  REG_MULTI_SZ  UXVpZXR8MSwxODAwfDIsVEMwUCw1MCw4MA==
                             (one string per preset, in list order)
```
Other legal `ActivePreset` values: `Predefined:0`, `Predefined:1`,
`Unsaved:VU5TQVZFRHwwfDEsMjQwMA==` (= `UNSAVED|0|1,2400`).
Legacy (read-only, only when `ActivePreset` is missing/empty): `Fan_0` = `1,2400`,
`Fan_1` = `2,TC0P,50,80`, …

UNKNOWN: the REG_MULTI_SZ type was not verified in the QSettings wrapper (`FUN_00599bb0`), it is
what Qt produces for a QStringList; whether the `Fan_%1` read happens inside a QSettings group —
the read call (`FUN_005988a0`) is given the bare key, but `FUN_00433790` also touches the names
`Machine`, `TempSensors` and `Fan_%1` through `FUN_00597f80`, which was not analysed. No
reference to a group literally named `Fans` was seen in the functions read.

---------------------------------------------------------------------------------------------------

## 5. Constant-RPM validation and min == max

No clamping or validation of a constant RPM exists in the fan-control layer:

* `FanControlMode::fromString` stores `toInt()` unchanged (§1.2);
* `applyFanProfileToSMC` / `setFanSpeed` pass `mode.rpm` (or the sensor algorithm's output)
  straight to `Service::setFanRPM(i, 3, rpm)` (§3.5). Anything `setFanRPM` does with the value
  belongs to the SMC-service spec.

The only range enforcement is the Qt widgets of the fan dialog (`FUN_0043b540`, dialog ctor
`FUN_00439f90`):

* constant slider (dialog+0xc0) and spin box (+0xc4): minimum = `fan.minRpm`, maximum =
  `fan.maxRpm`. If **Ctrl+Shift** are both held when the dialog is constructed (dialog+0x26,
  `QGuiApplication::keyboardModifiers() & 0x06000000`), the minimum is `0` instead.
* initial value: `mode.rpm` if the fan's mode is Constant, else the fan's current RPM (Qt clamps
  it into the range).
* sensor-based spin boxes: start temperature (+0xd0) and max temperature (+0xa8) both ranged
  `conv(20)`..`conv(100)` where `conv` = `FUN_004bd040` (display-unit conversion); the max box's
  minimum is then raised to `start + 8` (or `start + 14` when `FUN_004bd1e0()` is true —
  presumably Fahrenheit display) by `FUN_0043b000` / `FUN_0043b040`, so `maxT > startT` and the
  division in §4 is never by zero for values created by the UI.

min == max: **no special-case code was found** (searched: Fan struct helpers 0x458430–0x458450,
`loadFansFromSMC`, the control thread, the fan dialog, strings containing min/max, and a pattern
search for min/max field comparisons in the decompilation). The observable behaviour simply falls
out of the generic code: Full blast = `Constant(max)` = `Constant(min)`; the dialog slider has an
empty range; in §4 `step = 0`, every band is `[min, min]`, the first sample sets `rpm = min`
(tendency Increase, half), the next compute step clamps to `hi = min` and enters DoNothing, so the
output is `min` once and then every 31 s. UNKNOWN whether the main-window table hides or disables
the control button for such fans (UI code not read).

---------------------------------------------------------------------------------------------------

## UNKNOWN / not determined (consolidated)

1. Where the model identifier is stored into the app descriptor (`app+0x38`) and the SMBIOS
   parsing inside `FUN_0058e0d0` (§6.4) — consumers and sources were read, the glue was not.
2. `Service::setFanRPM` / `setFansControlModes` / `getFanName` internals incl. any clamping and
   the SMC-side fallback fan name (out of scope; only call sites are specified).
3. CPU-usage provider (`app+0x48`) — thread is unreferenced dead code in this build (§7).
4. `waitAny` result when both `stop` and `pause` are signalled (`FUN_0055a810`); callers only
   distinguish 0 from non-0 and re-check `stop` inside their loops.
5. Behaviour of `FanControlMode::fromString` on an empty string or on `1…` without a comma, and
   of `FanProfile::fromString` on invalid Base64 (undefined / not analysed).
6. `step` when `maxT == startT` (float→int conversion of ±inf); UI prevents it (§5).
7. Initial content of `lastSetTime` in the sensor state before the first reset (ctor leaves it
   uninitialised).
8. Registry value type of `CustomPresets` and group context of the legacy `Fan_%1` keys (§2.7).
9. `FUN_004348f0` ("helper tool installed", gate for the control thread) and the app properties
   `pro-version`, `win-no-support`, `smc-dump` — where they are set was not traced.
10. Callers of the restart path `FUN_0044ca80` and of the standalone throttled re-apply
    `FUN_0044e310` (no static references found).
11. The binary's own version literal near the exit path is `1.5.21` (string @00755ec4), not
    1.5.20; noted only as a discrepancy with the stated version.

