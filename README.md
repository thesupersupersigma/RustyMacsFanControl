# Rusty Macs Fan Control

A Rust reimplementation of [Macs Fan Control](https://crystalidea.com/macs-fan-control) for
Windows on Apple hardware (Boot Camp). It monitors fan speeds and temperature sensors and
lets you set each fan to a constant speed or have it follow a temperature sensor.

It is a single native executable of about 1.3 MB with no Qt runtime. The behaviour was
worked out by reverse engineering version 1.5.21 of the original and reimplementing it;
no code from the original is included.

> **Use at your own risk.** This program overrides your Mac's own fan control. Setting fans
> too low can let the machine overheat. The original app's warning applies here too: it is
> for advanced users who know how to use it without harming their Mac.

This project is not affiliated with or endorsed by CrystalIDEA. "Macs Fan Control" is their
product.

## Requirements

- An Intel Mac running Windows 10 or 11 (64-bit) through Boot Camp.
- The original Macs Fan Control installed. This app does not ship a kernel driver; it uses
  the signed `applesmc.sys` driver that the original's installer registers as the
  `AppleSMC` service. Language files are also read from the original's install folder.
- Administrator rights. Starting the driver needs them, so Windows asks for elevation each
  time the app starts.

Do not run this app and the original at the same time. They share settings and would both
try to control the fans.

## Install

Download the zip from the [Releases](../../releases) page, unpack it anywhere and run
`RustyMacsFanControl.exe`.

The executable is not code signed, so the elevation prompt shows an unknown publisher and
SmartScreen may warn the first time.

## Troubleshooting

**"Error initializing SMC interaction layer"** means the app could not reach the SMC. The
message now says why; the usual causes are:

- *The AppleSMC driver is not installed.* Install the original Macs Fan Control first. Its
  installer provides the signed `applesmc.sys` driver. If the driver file is present but its
  service is missing, this app registers the service itself.
- *Windows refused to load the driver* (error 577 or 1275). Memory Integrity blocks it; it
  is under Windows Security > Device security > Core isolation.
- *Not a Mac,* or Windows running in a virtual machine.

The log named in the message (`%TEMP%\RustyMacsFanControl.log`) has the details; please
attach it when opening an issue.

## Using it

- **Main window.** Fans are on the left with their minimum, current and maximum RPM.
  Temperature sensors are on the right.
- **Auto / Custom...** Each fan has two buttons. *Auto* hands the fan back to the system.
  *Custom...* opens a dialog where you choose a constant RPM or a sensor-based value with a
  start temperature and a maximum temperature.
- **Presets.** *Automatic* and *Full blast* are built in. After changing a fan the preset
  shows as *Custom\**; save it under a name from the button next to the preset name.
- **Tray icon.** Closing or minimizing the window keeps the app running in the notification
  area. Double-click the icon to bring the window back, right-click for the menu. *Exit* in
  that menu quits the app and returns every fan to automatic control.
- **Preferences.** Autostart at logon, language, Fahrenheit, one-decimal temperatures, and a
  sensor whose temperature is drawn in the tray icon.

The active preset is remembered. If you quit with a fan on a custom setting, that setting
is applied again the next time the app starts.

### Command line

| Switch | Effect |
|---|---|
| `/minimized` | Start hidden in the notification area (used by autostart). |
| `/log` | Write the log next to the executable instead of `%TEMP%\RustyMacsFanControl.log`. |
| `/applesmc` | Use the `applesmc.sys` driver even on Macs where Boot Camp's driver would be tried first. |
| `/skipcheck` | Skip the Apple-hardware and virtual-machine checks. |
| `--probe` | Print the fans and temperatures the SMC reports, then exit. Read-only. |

## What is missing compared with the original

- Temperatures of disk drives (NVMe/SATA) and of NVIDIA/AMD graphics cards read through
  their own drivers. Only sensors reported by the SMC are shown.
- The "Create tech zip file" command and the `/smcdump` switch.
- Dark mode.
- The direct port I/O fallback (`inpout32.dll`) used on very old Macs. It is a 32-bit DLL
  and this is a 64-bit program.
- Licensing, the trial, the update checker and the upgrade dialogs are intentionally left
  out. Custom presets are always available.

Settings are stored in the same registry key as the original
(`HKCU\Software\CrystalIdea Software\Macs Fan Control`), so presets carry over in both
directions. Autostart uses its own task named `RustyMacsFanControl`.

## Building

You need Rust (MSVC toolchain), the Windows SDK and Python 3.

The icons, sensor name tables and English strings belong to CrystalIDEA and are not stored
in this repository. Extract them from your own installed copy first:

```
python -I re/scripts/extract_assets.py "C:\Program Files (x86)\Macs Fan Control" app/assets
cargo test --manifest-path app/Cargo.toml
cargo build --release --manifest-path app/Cargo.toml
```

The result is `app/target/release/rusty-macs-fan-control.exe`.

Debug builds do not request elevation, so they can only reach the SMC when started from an
elevated terminal. `--demo` runs a debug build against a simulated SMC instead, which is
enough for working on the interface.

### Releases

Pushing a tag such as `v0.2.0` runs `.github/workflows/release.yml`. It checks that the tag
matches the version in `app/Cargo.toml`, downloads a zip of the `app/assets` folder from the
address stored in the `ASSETS_URL` repository secret, runs the tests, builds the release
executable and attaches it to a GitHub release. The workflow pins the zip's SHA-256, so
update that value in the workflow whenever the zip changes.

## Repository layout

| Path | Contents |
|---|---|
| `app/` | The Rust application. |
| `re/spec/` | Behavioural notes on the original that the implementation follows: SMC access, fan control, sensors and user interface. |
| `re/scripts/` | Tools used for the analysis: Ghidra dump scripts and the resource extractors. |
