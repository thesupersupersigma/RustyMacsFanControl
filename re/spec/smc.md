# SMC access layer — behavioural spec (Macs Fan Control 1.5.20, Windows, 32-bit)

Source: Ghidra decompilation (`re/out/MacsFanControl.exe/decomp.c`) cross-checked against a
capstone disassembly of `MacsFanControl/MacsFanControl.exe` where the decompiler dropped
arguments. Every claim carries the address of the function it was read from.
Items that could not be determined are marked **UNKNOWN**; inferences are marked **ASSUMPTION**.

Source-file names recovered from log strings:
`app\SMC\Platform_win.cpp`, `Platform_win_applesmc.cpp`, `Platform_win_bootcamp.cpp`,
`Platform_win_inpout32.cpp`, `common\core_win\Service.cpp` (class `WindowsService`).

---------------------------------------------------------------------------------------------

## 1. Provider selection — `SMC::PlatformWin::open` (004838e0)

### 1.1 Objects

| Class | vtable | size | fields |
|---|---|---|---|
| `SMC::PlatformWin` (QObject) | 0075ccdc | — | `+8` = `PlatformWinProvider*` (current provider, owned) |
| `PlatformWinProviderAppleSmc` (QObject) | 0075cd94 | 0x0c | `+8` = `HANDLE` of `\\.\APPLESMC` |
| `PlatformWinProviderBootCamp` (QObject) | 0075cd38 | 0x0c | `+8` = `HANDLE` |
| `PlatformWinProviderInpOut32` (QObject) | 0075cdf0 | 0x0c | `+8` = `HMODULE` of inpout32.dll |

Virtual interface, identical for `PlatformWin` and every provider (slot offsets after the 11 QObject slots).
`PlatformWin`'s slots 0x30..0x44 are pure forwarders to `this->provider` (004834c0, 004834d0, 004834f0,
00483cc0, 00484770, 00483510); they return without doing anything if provider is null.

| slot | meaning | AppleSmc | BootCamp | InpOut32 |
|---|---|---|---|---|
| 0x2c | `int open()` — 0 = OK, 2 = failed | 004980f0 | 0049aa50 | 004a7560 |
| 0x30 | `void close()` | 00496270 | 0049a280 | 004a6f70 |
| 0x34 | `bool getKeyByIndex(u32 index, std::string* outKey)` | 004976b0 | 0049a2a0 | 004a7050 |
| 0x38 | `bool getKeyInfo(const u32* key, KeyInfo6* out)` | 00497970 | 0049a6a0 | 004a7310 |
| 0x3c | `bool readKey(const u32* key, void* out, u8 len)` | 004989b0 | 0049b2d0 | 004a7b20 |
| 0x40 | `bool writeKey(const u32* key, const void* data, u8 len)` | 004993d0 | 0049b680 | 004a7b50 |
| 0x44 | `bool isOpen()` | 00483530 | 00483550 | 004a7550 |

### 1.2 Inputs to the decision

| Name | How obtained | Address |
|---|---|---|
| `isT2` | `isT2Model(&QCoreApplication::instance()->model)` — model struct lives at `app+0x38`: `u8 family` at +0, `u16 number` at +2 | 004b27e0 |
| `optInpOut32` | command-line switch `/inpout32` → option id 0 present in global option map (`std::map<int,…>` at 0089f7d4) | 00437360 (parse), 004b1a00 (lookup) |
| `optAppleSmc` | command-line switch `/applesmc` → option id 1 | same |
| `isWin11` | `RtlGetVersion`: `dwMajorVersion == 10 && dwBuildNumber > 21999` | 0058dd40 |

Model parsing (004b2900): the SMBIOS product name is matched with `startsWith` in this order and the
first match wins; `number` = the product string with every non-digit removed (`[^0-9]` → ""), parsed
as unsigned decimal (so `MacBookPro15,1` → 151, `iMac20,1` → 201, `MacPro7,1` → 71).

| family id | prefix | T2 when (004b27e0) |
|---|---|---|
| 6 | `Macmini` | number > 80 |
| 8 | `MacPro` | number >= 71 |
| 3 | `MacBookPro` | number > 150 |
| 2 | `MacBookNeo` | never |
| 1 | `MacBookAir` | number > 80 |
| 0 | `MacBook` | never |
| 5 | `iMacPro` | always |
| 4 | `iMac` | number >= 201 |
| 9 | `Xserve` | never |
| 10 | `ADP` | never |
| 7 | `MacStudio` | never |

### 1.3 Algorithm (004838e0) — returns 0 on success, 2 on failure

```
isT2 = isT2Model(app.model); optIO = option(0); optASMC = option(1)

if optIO && !isT2:
    log(info) "Force trying InpOut32 first"
    provider = new InpOut32            # replaces/deletes any previous provider
    if provider.open() == 0: return 0
    # on failure fall through to the normal chain (the failed InpOut32 object stays as provider
    # until replaced)

if isWin11 || isT2 || optASMC:
    p = tryAppleSMC()                  # 00483d10
    if p: provider = p; return 0
else:
    if bootCampVersionOk():            # 00483570, may show a dialog
        p = tryBootCamp()              # 00484160
        if !p: p = tryAppleSMC()
        if p: provider = p; return 0

# nothing worked
if isT2: return 2                      # T2 Macs never use InpOut32
log(info) "Last resort: trying InpOut32"
provider = new InpOut32
return provider.open() != 0 ? 2 : 0
```

Notes
* When `/inpout32` was given and InpOut32 failed first, the "last resort" step creates and opens a
  second InpOut32 instance again (no flag remembers the earlier failure).
* `/applesmc` simply forces the AppleSMC-only branch (skips Boot Camp). It does not suppress the
  final InpOut32 fallback on non-T2 machines.

### 1.4 Boot Camp version check — 00483570 (returns bool "may use Boot Camp")

1. Read `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion` value `ReleaseId` (string). If missing/empty → return true.
2. `_wtol(ReleaseId) > 1902` (i.e. Windows 10 1903 or later) else return true.
3. Path = `C:\Windows\Sysnative\drivers\MacHALDriver.sys` (stored reversed in the binary as
   `sys.revirDLAHcaM\srevird\evitansyS\swodniW\:C`, 0049a210). If the file does not exist (005871c0) → true.
4. Authenticode-verify the file with `WinVerifyTrust` (Wintrust.dll, action GUID
   {00AAC56B-CD44-11D0-8CC2-00C04FC295EE}) and fetch the signer's signing time (005819d0).
   The "old Boot Camp" condition is: verification result == 0 (trusted) **and** signing-time
   `SYSTEMTIME.wYear < 2013` **and** file size (GetFileAttributesExW, 005851c0) `!= 0x5958` (22872 bytes).
5. If old: `QMessageBox`-style dialog (0059d9e0) with title `"Macs Fan Control" + " " + "1.5.21"`
   (sic — the binary's version string literal is 1.5.21), buttons `0x140000` = Abort(0x40000) | Ignore(0x100000), text:

   > It seems that you're running Boot Camp version older than 6.0, it's required to upgrade to 6.0 or later. Older versions can cause BSOD on Windows 10 1903+. Press Abort to use alternative way to start the app. `<br><br><a href="https://crystalidea.com/blog/bootcamp-and-windows-10-1903-may-update">Read more</a>`

   Ignore → return true (Boot Camp HAL is used anyway). Abort → return false (skips both Boot Camp and
   AppleSMC and goes straight to the InpOut32 last resort). Any other result → true.
6. Otherwise → true.

There is no numeric "Boot Camp version" read anywhere: "older than 6.0" is inferred purely from the
MacHALDriver.sys signature year / file size as above.

### 1.5 `tryAppleSMC` (00483d10)

1. `svc = WindowsService::open(L"AppleSMC")` (0058a870: `OpenSCManagerW(NULL, L"ServicesActive", SC_MANAGER_ALL_ACCESS 0xF003F)`
   then `OpenServiceW(name, SERVICE_ALL_ACCESS 0xF01FF)`). Null → log(error) `AppleSMC service cannot be opened`, return null.
2. `svc.isEnabled()` (0058a640): `QueryServiceConfigW`; "enabled" ⇔ `dwStartType` is neither 2 (AUTO_START) nor
   4 (DISABLED); a failed query counts as 2 → not enabled. If not enabled: log(warn) `AppleSMC service is not enabled`
   and `ChangeServiceConfigW(h, SERVICE_NO_CHANGE, dwStartType = 3 /*DEMAND_START*/, SERVICE_NO_CHANGE, NULLs…)` (0058b0b0).
   Execution continues regardless of the result.
3. log(info) `Trying PlatformWinProviderAppleSmc`; `p = new PlatformWinProviderAppleSmc; rc = p->open()`.
4. `rc == 0` → release `svc` wrapper, return `p`. Otherwise delete `p`, release `svc`, return null.

### 1.6 `tryBootCamp` (00484160)

Service name `MacHALDriver` (stored reversed `revirDLAHcaM`, 0049b610).

1. `svc = WindowsService::open(L"MacHALDriver")`; null → log(error) `HAL service cannot be opened`, return null.
2. `!svc.isEnabled()` (same predicate as above) → log(error) `HAL service is not enabled`, return null. (No attempt to enable.)
3. `!svc.isStarted()` (0058a730: `QueryServiceStatus`, `dwCurrentState == 4 /*RUNNING*/`) →
   log(info) `HAL service is not started, trying to fix this`; `svc.start(500)` (0058b210, see 1.8).
4. `!svc.isStarted()` again → log(error) `HAL service is not started`, return null.
5. log(info) `Using HAL`; `p = new PlatformWinProviderBootCamp; rc = p->open()`; 0 → return p, else delete and return null.

### 1.7 Driver installation (`/init`) — 00437360 → 004380f0

Runs only when the process is started with the `/init` argument (done by the installer) **and**
`VerifyVersionInfoW` says OS >= 10.0 (00437050(10,0,0)):

1. If `<appdir>\AppleSMC.exe` exists: run `"<appdir>\AppleSMC.exe" /uninstall` and wait for it (005812a0, wait = INFINITE)
   — removes the legacy user-mode service of older releases.
2. `installDriver()` (004380f0):
   * file name = `applesmc.sys`; if `IsWow64Process(GetCurrentProcess())` is false (true 32-bit OS) → `applesmc32.sys`.
   * full path = application directory + file name. If it does not exist: message box
     (title `Macs Fan Control 1.5.21`) `"%1 driver doesn't seem to exist in the app folder"` with %1 = file name; stop.
   * `WindowsService::open(L"AppleSMC")`:
     * exists → `ChangeServiceConfigW(h, NO_CHANGE, NO_CHANGE, NO_CHANGE, lpBinaryPathName = fullPath, …)` (0058ab60, `setBinaryPath`).
     * missing → `WindowsService::create` (0058a050): `OpenSCManagerW(NULL, NULL, SC_MANAGER_CREATE_SERVICE 2)`,
       `CreateServiceW(scm, L"AppleSMC", L"Apple SMC service", SERVICE_ALL_ACCESS 0xF01FF, dwServiceType = 1 (KERNEL_DRIVER),
       dwStartType = 3 (DEMAND_START), dwErrorControl = 1 (NORMAL), lpBinaryPathName = fullPath, NULL…)`.
   * The service is **not** started here.
   The path is passed as a plain Win32 path; the SCM stores it as `\??\C:\…\applesmc.sys`.

Uninstall (00437b00, vtable-registered handler): `RegDeleteKeyW(HKCU, L"Software\CrystalIdea Software\Macs Fan Control")`;
on T2 models additionally runs `AppleSMC.exe /uninstall` (with ` /alt` appended for iMacPro number 11 and MacPro number 71)
and waits; finally `WindowsService::remove(L"AppleSMC")` → `DeleteService` (0058aa50).

### 1.8 `WindowsService` primitives (common\core_win\Service.cpp)

| Method | Addr | Behaviour |
|---|---|---|
| `open(name)` | 0058a870 | `OpenSCManagerW(NULL, L"ServicesActive", 0xF003F)`; `OpenServiceW(.., 0xF01FF)`; logs `OpenSCManager failed with error {}` / `OpenService ({}) failed with error {}`; object: `+4` logger, `+0xc` SC_HANDLE, `+0x10` name |
| `create(name, display, path, type, start)` | 0058a050 | see 1.7 |
| `isEnabled()` | 0058a640 | start type ∉ {2, 4} |
| `isStarted()` | 0058a730 | `dwCurrentState == SERVICE_RUNNING (4)` |
| `setServiceStartupType(t)` | 0058b0b0 | `ChangeServiceConfigW(h, -1, t, -1, 0…)` |
| `setBinaryPath(p)` | 0058ab60 | `ChangeServiceConfigW(h, -1, -1, -1, p, 0…)`; always returns true |
| `start(timeoutMs)` | 0058b210 | `QueryServiceStatus`; state 3 (STOP_PENDING) → return 1; state 4 (RUNNING) → return 4; else `StartServiceW(h,0,NULL)`; failure → log `StartService failed with error {}`, return 0; if timeoutMs == 0 → return 0; else `waitForState(4, timeoutMs)` → return 3 if reached, 2 if timed out |
| `waitForState(state, ms)` | 0058b770 | poll `QueryServiceStatus`; loop: equal → true; `ms < 0` → false; `Sleep(100)`; `ms -= 100` |
| `stop(timeoutMs)` | 0058b3d0 | logs `Trying to stop service {}`; `QueryServiceStatus`; state 3 → return 2; state 1 (STOPPED) → return 4; else `ControlService(h, SERVICE_CONTROL_STOP 1, &st)` … |
| `remove(name)` | 0058aa50 | open + `DeleteService` |

---------------------------------------------------------------------------------------------

## 2. Providers

### 2.0 Key packing (all providers)

The Service layer passes the key as `const char*` pointing at the ASCII key text (std::string data);
every provider copies the first 4 bytes **in string order, no byte swap** (004989b0 `local_4c = *param_2`,
0049b680 byte-wise copy, 004a84e0 sends `key[0..3]` in order).

Worked example, key `"TC0P"`: buffer bytes `54 43 30 50` (`'T','C','0','P'`); read as a little-endian
u32 that is `0x50304354`. Keys returned by key-by-index come back the same way (4 raw chars, then NUL).

`KeyInfo6` (output of getKeyInfo, 6 bytes, same for all providers): `[0]` data size (u8), `[1..5]` 4-char
type code in string order (e.g. `66 6c 74 20` = `"flt "`), `[5]` attributes (u8).

Index for key-by-index: host value `u32`; byte order on the wire differs per provider (below).

### 2.1 AppleSmc — `\\.\APPLESMC` (Platform_win_applesmc.cpp)

**open (004980f0)** — returns 0 OK / 2 fail
1. log(info) `Connecting using our driver...`
2. `svc = WindowsService::open(L"AppleSMC")`; null → return 2.
3. `r = svc.start(500)`; `r < 3` (neither "started within 500 ms" nor "already running") →
   log(error) `Failed to start service {}, error {}` (name, GetLastError), `svc.stop(0)`, return 2.
4. `CreateFileW(L"\\.\APPLESMC", GENERIC_READ|GENERIC_WRITE 0xC0000000, FILE_SHARE_READ|WRITE 3, NULL, OPEN_EXISTING 3, FILE_ATTRIBUTE_NORMAL 0x80, NULL)`;
   handle stored at `this+8`. `INVALID_HANDLE_VALUE` → log(error) `Create file failed to open {}, error {}`, `svc.stop(0)`, return 2.
5. `DeviceIoControl(h, 0x220020, NULL, 0, &proto /*u8, preset 0xFF*/, 1, &ret, NULL)`.
   Failure → log(warn) `GetProtocol failed, error {}` — **not fatal**. Success → log(info) `Connected, protocol is {}`.
   The protocol byte is only logged, never stored or used.
6. return 0.

**close (00496270)**: `CloseHandle(h)` if non-null; then `WindowsService::open(L"AppleSMC")` and `stop(0)` (driver is stopped on exit).
**isOpen (00483530)**: handle != 0 && handle != -1.
All four I/O calls first check `this+8 != 0`, else log(error) `The device is not opened` and return false. No retries anywhere.

| Op | Addr | IOCTL | Input buffer | nIn | Output buffer | nOut | Success condition |
|---|---|---|---|---|---|---|---|
| readKey(key,out,len) | 004989b0 | 0x220000 | `[0..4]` key chars, `[4]` len (u8) | 5 | 32-byte zeroed local | 0x20 | `DeviceIoControl != 0`; then `memcpy(out, buf, bytesReturned)` (bytesReturned is not compared to len) |
| writeKey(key,data,len) | 004993d0 | 0x220004 | `[0..4]` key, `[4]` len, `[5..5+len]` data (local buffer is 37 bytes ⇒ len ≤ 32) | len+5 | 1 byte | 1 | `DeviceIoControl != 0` **and** `bytesReturned == 1` |
| getKeyByIndex(idx,out) | 004976b0 | 0x220008 | u32 index, **little-endian** (native) | 4 | 4 bytes (+ NUL appended locally) | 4 | ioctl ok and `bytesReturned == 4`; key = C-string of the 4 bytes |
| getKeyInfo(key,out) | 00497970 | 0x22000C | `[0..4]` key | 4 | `KeyInfo6` | 6 | ioctl ok and `bytesReturned == 6`; otherwise log(error) `IOCTL_APPLESMC_GET_SMC_KEY_INFO returned {} bytes` |
| protocol | 004980f0 | 0x220020 | — | 0 | u8 | 1 | see open |

Error log on ioctl failure: `Error in DeviceIoControl {}` (GetLastError), level error.

### 2.2 BootCamp — `\\.\MacHALDriver` (Platform_win_bootcamp.cpp)

**open (0049aa50)** — returns 0 / 2
1. If global option id 2 has a non-empty string value: log(info) `Extra delay of {}` and `Sleep(stoi(value))`.
   Option 2 is set to the literal `"2000"` in 00437360 (at 00437752) when a flag byte at `+0x30` of an object
   created by 00594890 is non-zero. **UNKNOWN**: what that flag means (not traced; it is outside the SMC layer).
2. `CreateFileW(L"\\.\MacHALDriver" /*stored reversed*/, GENERIC_READ 0x80000000, share 0, NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL)` → `this+8`.
   Failure → log(error) `Failed to open standard port X (error {}), will try inpout32`, return 2.
3. Diagnostics only (when logging enabled, 00553e20): log `Ex version {} ({})` for `C:\Program Files\Boot Camp\Bootcamp.exe`
   and `Sy version {} ({})` for `C:\Windows\Sysnative\drivers\MacHALDriver.sys` (file version string + file size).
4. return 0.

**close (0049a280)**: `CloseHandle`. **isOpen**: 00483550 (same test as AppleSmc — ASSUMPTION, body not read).
Unopened handle → log(error) `Port X not opened`, return false. No retries.

| Op | Addr | IOCTL | Input buffer | nIn | Output buffer | nOut | Success |
|---|---|---|---|---|---|---|---|
| readKey | 0049b2d0 | 0x9C402454 | `[0..4]` key chars, `[4]` = 0 | 5 | caller's buffer directly | len | `DeviceIoControl != 0` (else log `readKey: ioctl failed, GetLastError() = {}`) |
| writeKey | 0049b680 | 0x9C402458 | heap buffer: `[0..4]` key, `[4]` = 0, `[5..]` data | len+5 | none (NULL) | 0 | `DeviceIoControl != 0` (else `writeKey: ioctl failed, …`) |
| getKeyByIndex | 0049a2a0 | 0x9C40245C | u32 index, **little-endian** (stored byte by byte, low byte first) | 4 | 5 bytes zeroed (4 chars + NUL) | 5 | `DeviceIoControl != 0`; key = C-string of buffer (else `getKeyByIndex: ioctl failed, …`) |
| getKeyInfo | 0049a6a0 | 0x9C402460 | `[0..4]` key, `[4]` = 0 | 5 | 12 bytes: `[0]` size (only low byte used; `[1..4]` ignored), `[4..8]` type chars, `[8]` attributes, `[9..12]` ignored | 0xC | ioctl ok **and** `out[0] != 0`; mapped to KeyInfo6 = {out[0], out[4..8], out[8]} (ioctl failure logs `GetKeyInfo: ioctl failed, …`) |

Note the length byte is *not* sent to the Boot Camp driver for reads (byte 4 is always 0); the requested
length is conveyed only through `nOutBufferSize`.

### 2.3 InpOut32 — direct port I/O (Platform_win_inpout32.cpp)

**open (004a7560)** — returns 0 / 2
1. `LoadLibraryW(L"inpout32.dll")` → `this+8`. Null → log(error) `inpout32 loading failed with {}` (GetLastError), return 2.
2. `GetProcAddress` for exactly three exports, stored in process globals:
   `IsInpOutDriverOpen` (0089f754), `Out32` (0089f758), `Inp32` (0089f75c).
   `IsInpOutDriverOpen` missing → same error log, return 2.
3. `ok = IsInpOutDriverOpen() != 0`; if false, call it **once more** and use that result (single immediate retry, no delay).
4. ok → log(info) `Using inpout32 module`, set global `isOpen` flag (0089f760) = 1, return 0.
   Not ok → log(error) `Initialization of inpout32 transport failed`, return 2.

**close (004a6f70)**: zero the three function pointers, `FreeLibrary`. (The isOpen flag 0089f760 is not cleared.)
**isOpen (004a7550)**: returns global 0089f760.

Calls used: `short Inp32(short port)` (low byte used), `void Out32(short port, short value)`.
If a function pointer is null the read yields 0 and the write is skipped.

Protocol — a direct port of Linux `drivers/hwmon/applesmc.c` (function names from its log strings `read_smc`/`write_smc`):

| Constant | Value |
|---|---|
| data port | 0x300 |
| command/status port | 0x304 |
| status bit 0 | 0x01 data awaiting read |
| status bit 1 | 0x02 input buffer closed |
| status bit 2 | 0x04 busy |
| READ_CMD | 0x10 |
| WRITE_CMD | 0x11 |
| GET_KEY_BY_INDEX_CMD | 0x12 |
| GET_KEY_TYPE_CMD | 0x13 |

```
udelay(us)            004a85b0  NtDelayExecution(FALSE, &(-10*us)) (relative, 100-ns units).
                                On the very first call: ZwSetTimerResolution(1, TRUE, &actual)
                                (one-shot flag byte 008901b1, initial value 1).
wait_status(val,mask) 004a8710  us = 8
                                for i in 0..23:
                                    if (Inp32(0x304) & mask) == val: return 0
                                    udelay(us*2)
                                    if i >= 10: us *= 2
                                return -5            # 24 polls, ~0.26 s worst case, no wall-clock limit
send_byte(b, port)    004a8560  wait_status(0, 0x02) || wait_status(0x04, 0x04) → on failure return -5
                                Out32(port, b); return 0
send_argument(key)    004a84e0  for i in 0..3: send_byte(key[i], 0x300)  (inlined; any failure → -5)
send_command(cmd)               wait_status(0, 0x02) else fail; Out32(0x304, cmd)        (inlined)
smc_sane()                      if wait_status(0, 0x04) == 0: ok
                                else: send_command(0x10) else fail; wait_status(0, 0x04) else fail   (inlined)

read_smc(cmd,key,buf,len)  004a7c20
    smc_sane() else return -5
    send_command(cmd) && send_argument(key)   else log(warn) "%.4s: read arg fail",  return -5
    send_byte(len, 0x300)                     else log(warn) "%.4s: read len fail",  return -5
    for i in 0..len-1:
        wait_status(0x05, 0x05)               else log(warn) "%.4s: read data[%d] fail", return -5
        buf[i] = Inp32(0x300)
    # flush
    for i in 0..15:
        udelay(8)
        if (Inp32(0x304) & 0x01) == 0: break
        data = Inp32(0x300)
    if i != 0: log(warn) "flushed %d bytes, last value is: %d"
    return wait_status(0, 0x04)               # 0 = success, -5 = failure

write_smc(cmd,key,buf,len) 004a8790
    smc_sane() else return -5
    send_command(cmd) && send_argument(key)   else log(warn) "%s: write arg fail",   return -5
    send_byte(len, 0x300)                     else log(warn) "%.4s: write len fail", return -5
    for i in 0..len-1: send_byte(buf[i], 0x300) else log(warn) "%s: write data fail", return -5
    return wait_status(0, 0x04)
```

| Op | Addr | Call | Result |
|---|---|---|---|
| readKey | 004a7b20 | `read_smc(0x10, key, out, len)` | `rc == 0` |
| writeKey | 004a7b50 | `write_smc(0x11, key, data, len)` | **`rc != 0`** — see bug note |
| getKeyInfo | 004a7310 | `read_smc(0x13, key, buf6, 6)` | `rc == 0 && buf6[0] != 0` → KeyInfo6 = buf6 verbatim; on rc != 0 log(error) `getKeyInfo: read_smc failed` |
| getKeyByIndex | 004a7050 | `read_smc(0x12, be32(index), buf4, 4)` — index sent **big-endian** as the 4 "key" bytes | `rc == 0` → key = C-string of buf4; else log(error) `getKeyByIndex: read_smc failed` |

**Bug in the original (verified in disassembly, 004a7b66 `test eax,eax; setne al`)**: InpOut32 `writeKey`
returns *true when `write_smc` failed* and *false when it succeeded* — the sense is inverted relative to
`readKey` (`sete`). Consequently, on the InpOut32 path a physically successful write is reported upward as
failure (Service logs `Unable to set fan … RPM …` / `Unable to set fan control modes` and returns false).
A reimplementation should decide deliberately whether to reproduce this.

There is no retry loop and no lock inside the InpOut32 provider; serialisation comes only from the
Service mutex (section 5). No named (cross-process) mutex exists for any provider — the only kernel mutex
strings in the binary (`Local\SM0:%lu:%lu:%hs`, 00577c20) belong to an unrelated library.

---------------------------------------------------------------------------------------------

## 3. `SMC::Service` (app\SMC\Service.cpp)

### 3.1 Objects

`SMC::Service` (QObject): `+8` = `ServicePrivate* d`.

| `ServicePrivate` offset | Field | Set by |
|---|---|---|
| +0x00 | `Platform* platform` (virtual interface of 1.1) | ctor |
| +0x04 | `std::mutex` (MSVC `_Mtx`, 0x30 bytes) | — |
| +0x34 | `atomic<u8> fanCount` cache, initial 0xFF = "not read yet" | getFanCount |
| +0x38..+0x40 | `std::vector<std::string> temperatureKeys` cache (begin/end/cap) | getTemperatureKeys |
| +0x44 | `u8 smcFanKeySize` (2 or 4) | doInit |
| +0x45 | `u8 smcFanModeKey` (1 = `FS! ` bitmask, 2 = per-fan `F{}Md`) | doInit |
| +0x46 | `u8 temperatureKeySize` (0 unset, 2 = sp78, 4 = flt) | getTemperatureKeys |
| +0x47 | `bool dualFan` = model family is iMacPro (5) or MacStudio (7) | doInit |
| +0x48 | `bool isXserve` = family 9 | doInit |
| +0x49 | `bool` = MacBookPro with number > 201 (004b2dd0) — consumer not found in this layer | doInit |
| +0x4a | `bool` = (MacStudio number > 30) or (MacBookPro number > 220) (004b2da0) — consumer not found in this layer | doInit |

### 3.2 Key name table

Key strings are XOR-obfuscated in the binary and decoded by static initialisers into globals
(decoder 0042e170: `byte[i] ^= key64 >> (8*(i%8))`). `{}` is a `fmt` placeholder filled with the
**0-based decimal fan index** (so `F0Ac`, `F1Ac`, …; indexes ≥ 10 would produce 5-char strings of which
only the first 4 bytes reach the SMC).

| Global | Key | Used by | Purpose |
|---|---|---|---|
| 0089f774 | `#KEY` | getSMCKeysCount 004ae7d0 | key count, 4 bytes, big-endian u32 |
| 0089f778 | `FNum` | getFanCount 004ad0e0 | fan count, 1 byte |
| 0089f77c | `FS! ` (trailing space) | doInit, get/setFansControlModes | forced-mode bitmask, 2 bytes |
| 0089f780 | `F{}ID` | getFanName 004ad610 | fan descriptor, 16 bytes |
| 0089f784 | `F{}Mn` | doInit (index 0, size probe); fanKey type 1 | minimum RPM |
| 0089f788 | `F{}Mx` | fanKey type 2 | maximum RPM |
| 0089f78c | `F{}Ac` | fanKey type 0 | actual RPM |
| 0089f790 | `F{}Tg` | fanKey type 3 | target RPM |
| 0089f794 | `F{}Sf` | fanKey type 4 | "safe" RPM |
| 0089f798 | `F{}Md` | get/setFansControlModes | per-fan mode byte (T2) |
| 0089f79c | `F{}md` | **no code reference** (only its initialiser 00422ec0) | unused |
| 0089f7a0 | `Ftst` | **no code reference** (only its initialiser 00422fd0) | unused — there is no unlock sequence |
| 0089f7a4 | `LKSB` | 004ae6f0 (read), 004b1450 (write) | keyboard backlight, 2 bytes |
| 0089f7a8 | `sp78` | getTemperatureKeys | type code compared |
| 0089f7ac | `flt ` | getTemperatureKeys | type code compared |
| 0075e85c / 0075e860 | `T` / `U` | getTemperatureKeys | lower-bound search keys |
| 0075e924.. | `TCAC`,`TCBC`,`TC0C`,`TC1C`,`TC2C`,`TC3C` | getTemperature | MacPro inversion quirk |

`fanKey(type, fanIndex)` = 004afb10: type 0→`F{}Ac`, 1→`F{}Mn`, 2→`F{}Mx`, 3→`F{}Tg`, 4→`F{}Sf`, other → empty string.

### 3.3 Initialisation — `ServicePrivate::doInit` (004ac480)

1. `getKeyInfo("F0Mn")`: success → `smcFanKeySize = info.size` (logs `detectSmcFanKeySize KeyType {}` / `KeyLen {}`);
   failure → log(error) `Unexpected failure at getKeyInfo, using 2, no fans?`, `smcFanKeySize = 2`.
   The type code itself is not stored — only the size decides fpe2 (2) vs flt (4).
2. `getKeyInfo("FS! ")`: success → `smcFanModeKey = 1`; failure → log(error)
   `Unexpected failure at getKeyInfo for SMC_KEY_FANS_CONTROL_MODE, using 2`, `smcFanModeKey = 2`.
   (This probe, not the T2 model test, decides old-style vs per-fan mode.)
3. Set the model flags +0x47..+0x4a as in 3.1.

doInit does not take the mutex.

### 3.4 Data type decoding / encoding actually implemented

| Type | Size | Decode | Encode | Where |
|---|---|---|---|---|
| `fpe2` (chosen when fan key size == 2) | 2 | `rpm = (b[0] << 6) \| (b[1] >> 2)` (big-endian 14.2 fixed point, fraction dropped) | `b[0] = rpm >> 6; b[1] = (rpm << 2) & 0xFF` | 004adb00 / 004afda0 |
| `flt ` (fan key size == 4) | 4 | 4 bytes as **little-endian IEEE-754 float**, truncated to int → u16 | `(float)rpm` little-endian | 004adb00 / 004afda0 |
| `sp78` (temperature size == 2) | 2 | `t = b[0] + b[1] * 0.00390625` with **both bytes unsigned** (no sign handling; negative values appear as ≥128 and are rejected) | — | 004aeb90 |
| `flt ` (temperature size == 4) | 4 | little-endian float → double | — | 004aeb90 |
| `ui8` | 1 | raw byte | raw byte | `FNum`, `F{}Md` |
| `ui16` (`FS! `) | 2 | only `b[1]` (low byte of the big-endian word) is examined | written as `{0x00, mask}` | 004adeb0 / 004b05f0 |
| `ui32` (`#KEY`) | 4 | big-endian u32 | — | 004ae7d0 |
| `LKSB` | 2 | first byte returned | caller-supplied byte pair | 004ae6f0 / 004b1450 |

No generic type-driven decoder exists in this layer (no `ui16`/`ui32`/`sp78` dispatch on the type string
apart from the temperature filter); sizes cached at init select the decoding.

### 3.5 Key count and enumeration

`getSMCKeysCount` (004ae7d0, locks mutex): `readKey("#KEY", &v, 4)`; `n = bswap32(v)`; logs
`getSMCKeysCount: {} {}` (raw, swapped). If `n > 0xFFFF` → log(error) `getSMCKeysCount failed`, return 0.
Read failure → returns 0.

Entry cache (stack-local to getTemperatureKeys, `calloc(count * 13)`), 13-byte record identical to Linux `applesmc_entry`:
`[0..5]` key (4 chars + NUL), `[5]` valid, `[6]` len, `[7..12]` type (4 chars + NUL), `[12]` flags.

`applesmc_get_entry_by_index(cache, i)` (004abb50; inlined copy inside 004abed0) — locks mutex per call:
if `valid` return true; `platform.getKeyByIndex(i, &key)` then `platform.getKeyInfo(key, &info)`; either failing →
log(error) `applesmc_get_entry_by_index failed`, false. Else fill record (`len = info[0]`, `type = info[1..5]`, `flags = info[5]`, valid = 1).

`applesmc_get_lower_bound(cache, count, &lo, key)` (004abed0): binary search `begin=0, end=count`;
`mid = begin + (end-begin)/2`; entry error → `*lo = 0`, return; `strcmp(entry.key, key) < 0 ? begin = mid+1 : end = mid`; `*lo = begin`.
(Relies on the SMC returning keys in sorted order.)

### 3.6 Fans

**getFanCount (004ad0e0)** — cached; when cache == 0xFF: lock, `readKey("FNum", &b, 1)`:
* failure → cache = 0, log(error) `Unable to read number of fans` (0 stays cached; never retried).
* success → cache = b; then
  * `dualFan && b == 2` → log(info) `Dual fan: the mac has a dual-fan cooler with two syncronized fans.`, cache = 1;
  * else if `isXserve` → log(info) `XServe 10 fans limit`, cache = 10 (unconditional).
`getFansControlModes` and `setFansControlModes` each additionally latch `getFanCount()` in their own
function-static variable on first call.

**getFanName(idx) (004ad610)** — lock; `readKey(fmt("F{}ID", idx), buf16, 16)`.
Success: name = C-string starting at `buf16[4]` (max 12 chars; a NUL is forced after byte 15), trimmed of leading and
trailing `isspace` characters (00559090). Failure: log(error) `Unable to read fan {:d} name. smc key {}`.
If the resulting name is empty (failure or blank) → `fmt("Fan {}", idx)` with the same **0-based** index.

**getFanRPM(key, u16* out) (004adb00)** — lock; `u32 buf = 0; readKey(key, &buf, smcFanKeySize)`; failure →
log(error) `Unable to read fan rpm, smc key {}`, false. Decode per 3.4. Wrapper 004addc0(idx, type, out) = `getFanRPM(fanKey(type, idx), out)`.
No range/sanity check on RPM values.

**setFanRPM(key, u16 rpm) (004afda0)** — lock; encode per 3.4 (`smcFanKeySize` not 2/4 → log(error) `Unknown m_smcFanKeySize`, false);
`writeKey(key, buf, smcFanKeySize)`; failure → log(error) `Unable to set fan {} RPM {}`, false; success → log(info) `setFanRPM {} for {}`, true.
No read-back verification, no delay, no retry.

**setFanRPM(idx, type, rpm) (004b02e0)** — `r = setFanRPM(fanKey(type, idx), rpm)`. If `dualFan`:
idx == 0 → additionally `r = setFanRPM(fanKey(type, 1), rpm)` (second result replaces the first);
idx != 0 → log(error) `Dual fan: unexpected error, fan index is supposed to be 0`.
Which `type` values callers pass (e.g. `Tg` vs `Mn`) is decided outside this layer — **UNKNOWN here** (callers of 004b02e0 not traced).

**getFansControlModes(std::vector<bool>* out) (004adeb0)** — lock; values are appended with push_back
(the function does not clear `out` — ASSUMPTION from the absence of a clear call):
* `smcFanModeKey == 1`: `readKey("FS! ", b, 2)`; failure → log(error) `Unable to read fans control mode, key {}`, false.
  For i in 0..fanCount-1: push `(b[1] & (1 << i)) != 0`. Return true (also when fanCount == 0).
* `smcFanModeKey == 2`: for i in 0..fanCount-1: `readKey(fmt("F{}Md", i), &m, 1)`; failure → log(error)
  `Unable to query fan mode {}` and stop; else push `m != 0` and set result = true. (Result stays true if an
  earlier fan succeeded and a later one failed; false when fanCount == 0.)
* otherwise log(error) `Unknown m_smcFanModeKey`, false.

**setFansControlModes(const std::vector<bool>& modes) (004b05f0)** — `true` = forced/manual, `false` = automatic.
* `modes.size() != fanCount` → log(error) `setFansControlModes: fanCount != vModes.size()`, false (before locking).
* lock.
* `smcFanModeKey == 1` (pre-T2): `mask = Σ (modes[i] ? 1 << i : 0)` (u8); `writeKey("FS! ", {0x00, mask}, 2)`.
  The whole word is overwritten (no read-modify-write; bits of fans ≥ fanCount are cleared).
  Success → log(info) `Set fans mode {}` (mask), true. Failure → log(error) `Unable to set fan control modes`, false.
  Clearing manual mode = writing the mask with that fan's bit 0 (all auto = `00 00`).
* `smcFanModeKey == 2` (T2): work on a copy of `modes`. If `dualFan`: size == 1 → append a second element equal to
  `modes[0]`; otherwise log(error) `Dual fan: unexpected error, must be only one fan` and continue.
  For i in 0..size-1: `writeKey(fmt("F{}Md", i), {modes[i] ? 1 : 0}, 1)`; failure → log(error)
  `Unable to set fan {} control mode`, result = false, stop; success → log(info) `Set fan {} mode {}`, result = true.
* otherwise log(error) `Unknown m_smcFanModeKey`, false.

There is **no `Ftst` unlock write, no verification read-back and no sleep** anywhere in these paths; if the
shipped app does any of that it is not in the SMC layer functions listed here.

### 3.7 Temperatures

**getTemperatureKeys(std::vector<std::string>* out) (004af070)** → bool
1. If `d->temperatureKeys` is non-empty → copy to `out`, return true (discovery runs once).
2. `count = getSMCKeysCount()`; 0 → return false.
3. Allocate the 13-byte-per-key cache (bad_alloc is caught → log(error) `std::bad_alloc for keysCount {}`).
4. `lo = lower_bound("T")`, `hi = lower_bound("U")`; log(info) `Locating temperature SMC keys`.
5. If `lo == 0 || hi == 0 || hi <= lo || count <= hi` → log(error)
   `one of applesmc_get_lower_bound failed {}, {}, {}` (lo, hi, count), result false.
6. Else for `i` in `lo .. hi-1`: `get_entry_by_index(i)`; skip on failure. Keep the key when
   `(type == "sp78" || type == "flt ") && key[0] == 'T'`. Append the 4-char key to `d->temperatureKeys`.
   On the first kept key only: `temperatureKeySize = (type == "flt ") ? 4 : 2` — **one global size for all
   temperature keys**, decided by the first match. Result true (even if nothing matched).
7. log(info) `{} keys found`; free cache; copy `d->temperatureKeys` to `out`.

No further filtering (no blacklist, no probing read, no dedup) happens here.

**getTemperature(key, double* value, double* raw /*nullable*/) (004aeb90)** → bool
1. lock; `u32 buf = 0; readKey(key, &buf, temperatureKeySize)`; failure → false.
2. Decode per 3.4 (size 2 → sp78, size 4 → float; any other size leaves `*value` untouched).
3. If `raw != NULL`: `*raw = *value`.
4. Reject when `*value <= 0.0 || *value >= 127.0` → log(warn)
   `getTemperature - failed reading sensor {}, its value is {}`, return false. Accepted range is the open interval (0, 127).
5. Quirk: if model family == MacPro (8; latched in a function-static on first call) and key is one of
   `TCAC, TCBC, TC0C, TC1C, TC2C, TC3C` → `*value = 100.0 - *value` (raw is left unmodified).
6. return true.

### 3.8 Keyboard backlight helpers

`004ae6f0(u8* out)`: lock; `readKey("LKSB", b2, 2)`; success → `*out = b2[0]`. `004b1450(...)`: lock; `writeKey("LKSB", buf2, 2)`
with buffer `{value, 0x00}` (value = the u8 argument).
Both are reached only through thunks 0040cb9e / 00405a8d (no direct callers found by call-site scan).

---------------------------------------------------------------------------------------------

## 4. SMC dump

### 4.1 Where it lives

* Text generator: `SMC::Service` method 004acc10 → `QString`. Its only caller is 0044a420, called from
  `TechZip::createZip` (004b4f50), which stores the text as `smc_dump.txt` inside `%1_tech_info.zip`
  (next to `app_info.txt`, `app_log.txt`, `app_screen.png`).
* Command-line switch `/smcdump` (00435010) only sets the QCoreApplication dynamic property `"smc-dump"` = `"yes"`.
  The property is read in `QFanControl::initialLoadThread` (0044b0d0 at 0044b25b). **UNKNOWN**: what that
  branch does with it (UI layer, not traced) — in particular whether it writes a file or opens the tech-info flow.

### 4.2 Generation loop (004acc10)

```
n = getSMCKeysCount()                         # 0 → empty string
for i in 0..n-1:
    if !platform.getKeyByIndex(i, &key): continue
    if !platform.getKeyInfo(key, &info): continue
    data = []
    if info.size <= 32 and platform.readKey(key, buf, info.size): data = buf[0..info.size]
    out += formatLine(key, type = info.type as 4-char string, info.size, data)     # 004818e0
```
The loop calls the platform directly and does **not** hold the Service mutex (only getSMCKeysCount locks).
Keys whose read fails (or size > 32) are still listed, with no value and no hex bytes.

### 4.3 Line format (004818e0, std::ostringstream)

```
<KEY> [<TYPE>]            always; TYPE printed verbatim incl. trailing spaces, e.g. "[ui8 ]"
 <value>                  only if a decoded value string is non-empty AND type is neither "flag" nor "hex_"
 (<size> bytes            <size> = info.size in decimal
:                         only if data is non-empty
 xx xx …                  one " " + 2-digit lowercase hex (setw 2, fill '0') per byte, min(size, data.len) bytes
)\n
```
Illustrative examples constructed from the format above (not captured output): `TC0P [sp78] 45.125 (2 bytes: 2d 20)` · `FNum [ui8 ] 2 (1 bytes: 02)` · `XXXX [ch8*] (16 bytes)` (read failed).

Value decoding in the dump (requires at least the stated number of data bytes; `u16` = big-endian
`(b[0]<<8)|b[1]`; reals printed with `std::fixed << std::setprecision(p)`):

| Type | Min bytes | Value | Precision p |
|---|---|---|---|
| `flag`, `hex_` | — | none | — |
| `fp1f` | 2 | u16 / 32768 | 5 |
| `fp4c` | 2 | u16 / 4096 | 5 |
| `fp5b` | 2 | u16 / 2048 | 5 |
| `fp6a` | 2 | u16 / 1024 | 4 |
| `fp79` | 2 | u16 / 512 | 4 |
| `fp88` | 2 | u16 / 256 | 3 |
| `fpa6` | 2 | u16 / 64 | 2 |
| `fpc4` | 2 | u16 / 16 | 2 |
| `fpe2` | 2 | u16 / 4 | 2 |
| `ui8 ` | 1 | b[0] unsigned decimal | int |
| `ui16` | 2 | big-endian u16 | int |
| `ui32` | 4 | big-endian u32 | int |
| `ui64` | 8 | big-endian u64 | int |
| `si8 ` | 1 | b[0] signed | int |
| `si16` | 2 | big-endian 16-bit (signedness of the print not confirmed — ASSUMPTION signed) | int |
| `sp1e` | 2 | 16-bit / 16384 | 5 |
| `sp3c` | 2 | 16-bit / 4096 | 5 |
| `sp4b` | 2 | 16-bit / 2048 | 4 |
| `sp5a` | 2 | 16-bit / 1024 | 4 |
| `sp69` | 2 | 16-bit / 512 | 3 |
| `sp78` | 2 | 16-bit / 256 | 3 |
| `sp87` | 2 | 16-bit / 128 | 3 |
| `sp96` | 2 | 16-bit / 64 | 2 |
| `spb4` | 2 | 16-bit / 16 | 2 |
| `spf0` | 2 | 16-bit as a real, no scaling | default stream precision (not confirmed) |
| `flt ` | 4 | little-endian float | default stream precision (not confirmed) |
| `{pwm` | 2 | u16 * 100 / 65536, followed by `%` | p not confirmed |
| anything else | — | none | — |

**UNKNOWN / low confidence in 4.3**: whether the `sp**` family sign-extends the 16-bit word before scaling
(the decompiler shows `(double)(int)CONCAT11(b0,b1)`, which does not reveal a `movsx`; not checked in
disassembly), and the exact precision/format flags of the `spf0`, `flt ` and `{pwm` branches. The precision
column for the fp/sp rows was read from the shared tail labels of the decompilation and should be spot-checked
against a real `smc_dump.txt`.

---------------------------------------------------------------------------------------------

## 5. Thread-safety

| Lock | Location | Guards |
|---|---|---|
| `ServicePrivate::mutex` (`std::mutex`, non-recursive; `_Mtx_lock`/`_Mtx_unlock`) | `d+4` | every single platform transaction issued by: getSMCKeysCount 004ae7d0, getFanCount 004ad0e0 (only while the cache is unset), getFanName 004ad610, getFanRPM 004adb00, setFanRPM 004afda0, getFansControlModes 004adeb0 (whole loop), setFansControlModes 004b05f0 (whole loop), getTemperature 004aeb90, LKSB read/write 004ae6f0/004b1450, applesmc_get_entry_by_index 004abb50 and each probe inside applesmc_get_lower_bound 004abed0 (locked per entry) |
| MSVC thread-safe static init (`_Init_thread_header`) | function statics | latched fanCount copies in get/setFansControlModes, `isMacPro` in getTemperature, the decoded key strings |
| `atomic<u8>` (`xchg`) | `d+0x34` | fanCount cache |

Not locked:
* `doInit` 004ac480 (runs once before other use), the dump loop 004acc10, `PlatformWin::open`.
* `getTemperatureKeys` 004af070 as a whole — it locks only inside its helpers, so the
  `temperatureKeys` vector and `temperatureKeySize` are written without the mutex held.
* The dual-fan `setFanRPM(idx,type,rpm)` wrapper 004b02e0 takes the lock separately for each of its two writes.
* Providers hold no lock of their own; InpOut32's function pointers and `isOpen` flag are plain process globals,
  so two InpOut32 provider instances share state.
* No cross-process (named) mutex is used for SMC access by any provider. With the AppleSmc and BootCamp
  providers, serialisation against other SMC clients is left to the respective kernel driver; with InpOut32
  there is none.

`getFansControlModes`/`setFansControlModes` call `getFanCount()` (which locks) *before* taking the lock
themselves, so the non-recursive mutex is never re-entered.

---------------------------------------------------------------------------------------------

## 6. Consolidated UNKNOWN list

| # | Item | What was tried |
|---|---|---|
| 1 | Which fan-key `type` (0 Ac / 1 Mn / 2 Mx / 3 Tg / 4 Sf) the UI passes to `setFanRPM` (004b02e0; call sites 00445a40 in 00445980 and 004487fb in 004485b0) and to `getFanRPM` (004addc0; call sites in 00445b00 ×3, 0044e460) | call sites located, argument values not decoded |
| 2 | Consumers of model flags `d+0x49` (MacBookPro > 201) and `d+0x4a` (MacStudio > 30 / MacBookPro > 220) | none of the Service functions read here use them |
| 3 | Meaning of the flag that enables the Boot Camp `"2000"` ms extra delay (option id 2, 00437752) | producer object from 00594890 not traced |
| 4 | Effect of the `/smcdump` → `"smc-dump"` property in 0044b0d0 | property read located, branch not traced |
| 5 | Sign-extension and precision details of some dump value branches (4.3) | decompilation only |
| 6 | Whether `getFansControlModes` clears its output vector first | no clear call seen; not proven in disassembly |
| 7 | `WindowsService::stop` behaviour after `ControlService` (wait/timeout handling of its argument) | body read only up to the `ControlService` call |
| 8 | Where the SMBIOS product name that feeds the model struct (`app+0x38`) is read | outside this layer |
| 9 | How the AppleSmc service-object wrapper is released on the success path of 004980f0 (it is not stopped) | decompilation ambiguous; behaviourally irrelevant |

Things explicitly looked for and **not present** in the SMC layer: any `Ftst` write (the string exists but is
unreferenced), any `F{}md` use, retry loops around key I/O, read-back verification after writes, sleeps
between writes, RPM clamping to Mn/Mx, temperature-key blacklists.

