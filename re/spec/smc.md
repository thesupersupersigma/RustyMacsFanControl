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

