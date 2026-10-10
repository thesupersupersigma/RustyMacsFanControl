# Macs Fan Control 1.5.20 (Windows, 32-bit, Qt6 Widgets) - UI and application shell spec

Source: Ghidra decompilation in `re/out/MacsFanControl.exe/decomp.c`. Every claim carries the
address of the function it was read from (`FUN_xxxxxxxx`; a `thunk_FUN_x` call is cited by the
thunk target address). Anything not established from the decompilation is marked **UNKNOWN**
or **INFERRED** (with the reasoning). Geometry values are Qt logical pixels (device-independent;
Qt multiplies by the monitor scale factor).

Conventions used below:
- `tr("Section/key")` = lookup in the language XML (see section 8). English text is given in quotes.
- Qt enum values are given both raw (as found) and decoded.

> NOTE on version: the shipped folder is labelled 1.5.20 but every version literal in the binary
> is `"1.5.21"` (FUN_00468cb0, FUN_00468790, FUN_00463330). Use whatever the rewrite wants to
> report; the *format* of the strings is what matters.

Class / method names below come from the moc string tables embedded in the binary
(`QFanControlMainWindow`, `QTableViewFans`, `QWidgetProfiles`, `QTrayIcon`, ...), so they are the
original names, not guesses.

---------------------------------------------------------------------------------------------------
## 1. Main window (`QFanControlMainWindow` : `QMainWindowEx` : `QMainWindow`)

Constructor FUN_00461d00, setupUi FUN_00465eb0 (objectName `MacFanControlClass`), base ctor
FUN_005a1040, retranslate FUN_00468440 (= slot `slotUpdatePreferences`).

### 1.1 Title, size, state

| Item | Value | Where |
|---|---|---|
| Initial size (setupUi `resize`) | 821 x 375 (0x335 x 0x177) | FUN_00465eb0 |
| `setAnimated(true)`, `setDocumentMode(false)` | | FUN_00465eb0 |
| Designer title (immediately overwritten) | `MacFanControl` | FUN_00465eb0 |
| Real title | `"%1 %2 (%3)"` with %1 = `"Macs Fan Control" + " " + "1.5.21"`, %2 = `"Pro"` if app property `pro-version` is set, else `tr("TrialDialog/trialVersion")` = "Trial version"; %3 = QString at `app+0x48 -> +4` (INFERRED: Mac model identifier from sysinfo, e.g. `MacBookPro11,1`; not traced further) | FUN_00468cb0 |
| Minimum size | No explicit window minimum. Effective minimum comes from children: tableFans `setMinimumWidth(500)` + `setMinimumHeight(horizontalHeader height)` (FUN_004b67f0); right frame `setMinimumSize(200,0)` (FUN_00465eb0); right frame `setMinimumHeight(height of left frame after adjustSize)` (FUN_00461d00: `frame->setMinimumHeight(frame_2->size().height())`); plus layout margins. | |
| After construction | `adjustSize()` then restore geometry (1.8) | FUN_00461d00 |
| Accessible descriptions | tableFans "Fans table", tableSensors "Sensors table" | FUN_00461d00 |
| Tab order | tableFans -> tableSensors | FUN_00465eb0 |

No menu bar, no status bar, no toolbar are created.

### 1.2 Layout tree (FUN_00465eb0)

- `QMainWindow` MacFanControlClass
  - `centralWidget` QWidget, sizePolicy (Preferred, Preferred)
    - `verticalLayout_3` QVBoxLayout, spacing 6, margins 11,11,11,11
      - `splitter` QSplitter, orientation = 1 (Horizontal), `childrenCollapsible = false`
        - `frame_2` QFrame, frameShape 0 (NoFrame), frameShadow 0x20 (Raised)  [left pane, member +0x38]
          - `verticalLayout` QVBoxLayout, spacing 6, margins 0,0,0,0 (set to 11 then overridden to 0)
            - `profilesWidget` (`QWidgetProfiles`, FUN_004c5610) sizePolicy (Preferred,Preferred), min 0x0, max 16777215, stretch 0
            - `tableFans` (`QTableViewFans`, FUN_004b6010) sizePolicy (Expanding, Expanding) (7,7), stretch 0
        - `frame` QFrame, NoFrame/Raised, `minimumSize(200,0)`   [right pane, member +0x48]
          - `verticalLayout_2` QVBoxLayout, spacing 6, margins 0,0,0,0
            - `labelSensorsHeader` QLabel, alignment 0x84 (AlignHCenter|AlignVCenter), text tr("MainWindow/staticSensorsListHeader") = "Temperature sensors:"
            - `tableSensors` (`QTableViewSensors`, FUN_004ba230), sizePolicy (Expanding, Expanding), max 16777215
      - `buttonBox` QDialogButtonBox, `setStandardButtons(0)` (none); buttons added in code (1.6)

Splitter sizes: read `WindowSplitterPosition` (QStringList of two decimal ints = the two pane
widths). If the stored list does not have exactly 2 entries the default is
`[windowWidth - 240, 240]` (FUN_004658a0). Saved on quit (1.8 / 4.7).

### 1.3 Presets bar - `profilesWidget` (`QWidgetProfiles`, FUN_004c5610)

- QHBoxLayout, margins 0,0,0,0 (default spacing), contents:
  1. stretch (factor 0)
  2. QLabel tr("Presets/activePresetLabel") = "Active preset:"                      (member +0x48)
  3. "preset button" (member +0x4c): wrapper widget FUN_005b8300 = QWidget with a zero-margin
     QVBoxLayout holding one **QPushButton** (a QToolButton is used only if the parent casts to
     some other class - not the case here). Initially disabled. Text = active preset name, has a
     drop-down menu (the *presets menu*, member +0x54). Its width is fixed to
     `max(horizontalAdvance(text of every enabled non-separator action of the presets menu)) + 30`
     using the button font (FUN_004c6190 / FUN_004c7250).
  4. "action button" (member +0x50): same wrapper, QPushButton with style sheet
     `QPushButton { text-align: left; }` (FUN_005b87d0(1)), no text, icon
     `:/Resources/profiles_action.png` (FUN_0045d970 -> FUN_005b8790), drop-down menu = *action menu*
     (member +0x58). Initially disabled.
  5. stretch (factor 0)
- `adjustSize()` at the end.

So on Windows the "combo box" is really a push button with a popup menu, followed by a small icon
button with a second popup menu. The whole group is horizontally centred (stretch on both sides).

Global enable: if the fan-control "ready" flag `DAT_0089eec0` is 0, both buttons are disabled and
no menus are built (FUN_004c6190). The whole left pane (`frame_2`) is additionally disabled when
FUN_004b28c0() is true (computer has no fans at all - same predicate that selects
`messageNoFansAvailable`) (FUN_00461d00) or when the SMC backend reports not-ready
(FUN_00467390: `backend->vtbl[0x20]()` false or `vtbl[0x24]()` true).

**Presets menu** (rebuilt by FUN_004c74d0 on every `profilesChanged`, FUN_004648f0):

| # | Item | State / action |
|---|---|---|
| 1 | tr("Presets/presetMenuPredefined") "Predefined presets:" | disabled header |
| 2.. | one action per predefined preset (names: tr("Presets/presetAutomatic") "Automatic", tr("Presets/presetFullBlast") "Full blast"; FUN_004524d0) | triggered -> signal `setActiveProfile(profile)` (FUN_004c9350). The active one is `setCheckable(true); setChecked(true)` |
| | separator | |
| | tr("Presets/presetMenuCustom") "Custom presets:" | disabled header |
| A (app property `pro-version` set) | one **submenu** per custom preset, title = preset name. If it is the active preset the submenu's menuAction is checkable+checked and it has no "Apply"; otherwise the submenu starts with tr("Presets/presetMenuApply") "Apply" (-> `setActiveProfile`) + separator. Then tr("Presets/presetMenuRename") "Rename" (-> FUN_004c6330: asks the name via `letUserSelectProfileName`, emits `renameProfile(profile,newName)`), tr("Presets/presetMenuDelete") "Delete" (-> `deleteProfile(profile)`, FUN_004c9130) | |
| A | tr("Presets/presetCreateNew") "New..." - only when the active preset is one of the two predefined ones (FUN_00453da0 / FUN_00453e90) | emits `showCreateNewPresetMessage` (FUN_004c9390) -> main window `slotShowNewPresetTip` (FUN_004680b0) |
| B (`pro-version` not set) | tr("Presets/presetMenuProVersion") "Available in PRO version" - only when the current state is not "unsaved" | emits `showUpgradeDialog` (FUN_004c93b0) |
| last (only when the current fan state is an unsaved custom state, FUN_00453f10) | the *action menu* added as a submenu titled tr("Presets/presetCurrentUnsaved") "Custom*", checkable + checked | |

After building: button text := active preset name (or "Custom*" when unsaved), button enabled,
and the menu's own title := tr("Presets/presetMenuCurrent").arg(name) = "Preset - %1".

**Action menu** (FUN_004c6af0). The action button is enabled only if the active preset is a custom
one or the state is unsaved; otherwise disabled with an empty menu.

| State | Items |
|---|---|
| unsaved custom state | `tr("Presets/presetMenuSave") + "\t" + "Ctrl+S"` ("Save..." with shortcut Ctrl+S) -> `slotSave` (FUN_004c66d0: not Pro -> `showUpgradeDialog`; Pro -> name dialog (7.2) -> `saveUnsavedProfile(name)`); separator; tr("Presets/presetMenuDiscard") "Discard (reset all fans to auto)" -> `slotDiscard` (FUN_004c6110: `setActiveProfile(predefined #0)`) |
| active preset is custom | "Rename" (same as above), "Delete" |
| predefined preset active | (menu empty, button disabled) |

`slotShowNewPresetTip` (FUN_004680b0): QMessageBox, icon 1 (Information), text
tr("Presets/presetCreateNewTitle") "Create a new preset"; informative text
tr("Presets/presetCreateNewMessage") or, if the active preset is "Full blast",
tr("Presets/presetCreateNewMessage2").arg(tr("Presets/presetFullBlast")). Buttons: standard OK
(0x400) and, when `DAT_0089eec0 == 1`, an extra button
tr("Presets/presetCreateNewButton").arg(name of fan #0) "Custom control for the '%1' fan" added with
role value 6, whose click opens the fan dialog for fan 0 (FUN_00464e70).

### 1.4 Fans table - `tableFans` (`QTableViewFans` : `QTableViewAltColor` : QTableView)

Constructor FUN_004b6010, base FUN_005a5bd0, model `QTableViewFansModel` (FUN_004b8660).

| Property | Value | Where |
|---|---|---|
| `alternatingRowColors` | true (base class) | FUN_005a5bd0 |
| Extra painting | when the vertical scroll position is 0 the base class continues the alternating stripes below the last row down to the bottom of the viewport, using palette role 0x10 (AlternateBase) and the default row height | FUN_005a5c90 |
| `showGrid` | false | FUN_00465eb0 |
| `wordWrap` | false | FUN_00465eb0 |
| corner button | disabled | FUN_00465eb0 |
| vertical header | hidden; resize mode 2 (Fixed); default section size (row height) **36** (0x24) | FUN_004b6010 |
| horizontal header | not clickable; section 2 resize mode 1 (Stretch); section 0 resize mode 3 (ResizeToContents); section 1 width `max(100, horizontalAdvance(tr("MainWindow/listColumnRPM")) + 30)` | FUN_004b6010, FUN_004b6360 |
| icon size | 32 x 32 | FUN_004b6010 |
| selection behaviour / mode | 1 (SelectRows) / 1 (SingleSelection) | FUN_004b6010 |
| item delegate | `Widgets::NoDrawFocusDelegate<QItemDelegate>` (no focus rectangle) | FUN_004b6010 |
| vertical scroll bar policy | 1 (ScrollBarAlwaysOff) | FUN_004b6010 |
| horizontal scroll bar policy | 0 (ScrollBarAsNeeded) | FUN_004b6010 |
| minimum width / height | 500 / height of the horizontal header | FUN_004b67f0 |
| Tab key | swallowed (keyPressEvent returns for key 0x01000001) | FUN_004b66d0 |
| context menu / double click | **none** - the class overrides only keyPressEvent, paintEvent, resizeEvent, showEvent (vtable 0x0075f094) | |

Columns (model `headerData`, FUN_004b9080; 3 columns):

| # | Header | Cell content |
|---|---|---|
| 0 | tr("MainWindow/listColumnFan") "Fan" | DisplayRole = fan name, DecorationRole = fan icon (32px `_big` variant: `:/Resources/fan_big.png`, `processor_big.png`, `drive_big.png`, `disc_big.png` chosen by fan type, FUN_0045d400), alignment 0x81 (AlignLeft|AlignVCenter) (FUN_004b8c30) |
| 1 | tr("MainWindow/listColumnRPM") "Min/Current/Max RPM" | index widget `QTableFansRPMWidget` (a QLabel, alignment 0x84 centred, rich text; FUN_004b9d80) - see below. ToolTipRole when min == max: tr("FanControlDialog/statusWarning").arg(max) |
| 2 | tr("MainWindow/listColumnControl") "Control" | index widget `QTableFansControlWidget` - see below |

Alignment role for columns 1 and 2 is 0x84 (centre).

**RPM cell text** (FUN_004b9e50, arguments min, current, max). The label text is the HTML

```
<div align='center'><table><tr><td style='vertical-align: middle'>%1 — </td><td style='vertical-align: middle'>%2<td><td style='vertical-align: middle'>— %3</td></tr></table></div>
```
(the dash is U+2014 EM DASH; the unclosed `<td>` after %2 is in the original, string at 0x0075f8d8)

- %1 = min as decimal; if `min == max` it is wrapped as `<font color='#FF7E00'>min</font>` (orange).
- %2 = `<b><big>current</big></b>`; if `current == 0` it becomes `<b><big><font color='#FF7E00'>0</font></big></b>`.
- %3 = max as decimal; if `min == max` the same orange string as %1 is used.

Visually: `1200 — **1850** — 6200` with the current value bold and one step larger.
Updated per fan from `slotFanUpdate` (FUN_00467040 -> FUN_004b6df0).

**Control cell** (`QTableFansControlWidget`, FUN_004b7430): QWidget with a zero-margin QHBoxLayout
containing one "segmented button" widget (moc name `QCocoaFansSegmentedButton`, generic
implementation FUN_005ba290): a QWidget with QHBoxLayout margins 0 / spacing 0 holding N
`QToolButton`s with `autoRaise(true)`, focus policy 1 (TabFocus), checkable + autoExclusive
(selection mode 0, FUN_005bbf90). Here N = 2 (FUN_005bb1a0(2)):

| Segment | Text | Tooltip | Click |
|---|---|---|---|
| 0 | tr("MainWindow/fanControlAutomatic") "Auto" | tr("MainWindow/fanControlAutomaticInfo") "Automatic (controlled by system)" | emits `fanAutoModeClicked(row)` -> main window `slotResetFanMode` |
| 1 | fan in automatic mode: tr("MainWindow/buttonFanControlCustom") "Custom..." with tooltip tr("MainWindow/buttonFanControlCustomInfo") "Specify a custom controlling mode". Fan in a custom mode: the long mode description (table below), and the tooltip is the same description | | emits `fanManualModeClicked(row)` -> main window `slotShowFanDialog` |

- The checked segment reflects the mode: automatic -> segment 0 checked; custom -> segment 1 checked (FUN_004b7b40, FUN_005bb090).
- The segmented widget is created hidden and made visible on the first mode update (FUN_004b7b40).
- Segment widths, recomputed in resizeEvent (FUN_004b7830, FUN_004b7940) with `w = cellWidth - 40`:
  segment 0 = `min(w / 2, 100)`, segment 1 = `min(w - segment0, 300)` (both `setFixedWidth`).
  Segment text is elided to the button width (FUN_005bb990, `QFontMetrics::elidedText`).

Mode description strings (FUN_00450050, used here, in the tray menu and tooltips):

| Mode value | Text |
|---|---|
| 0 | `n/a` (literal) |
| 1 | tr("MainWindow/fanControlUnknown") "Unknown!" |
| 2 (automatic) | tr("MainWindow/fanControlAutomatic") "Auto" |
| 3 (constant) | tr("MainWindow/fanControlConstant").arg(rpm) "Constant value of %1" |
| 4 (sensor based) | short form: tr("TrayMenu/itemSensorBased") "Sensor-based"; long form: tr("MainWindow/fanControlSensor").arg(sensor name) "Based on %1" |

`slotResetFanMode(row)` (FUN_00467a00): if the fan's mode is not automatic (2) and the access check
FUN_00463200 passes, sets that fan to automatic (FUN_0044c7f0); then refreshes the row.
`slotShowFanDialog(row)` (FUN_00467da0): if FUN_00463200 passes, opens ChangeControlModeDialog
(section 3) for that fan; afterwards refreshes the row.
FUN_00463200 is the "fan control available" gate: returns false immediately if
`backend->vtbl[0x24]()`; if `backend->vtbl[0x20]()` is false it tries `backend->vtbl[0x28](&err)`
and on failure with a non-empty message shows it (FUN_004360e0); on success it enables the left
pane and restarts the worker threads (FUN_0044d7a0).

**"No fans" overlay** (FUN_004b6fd0): a QLabel child of the table (`openExternalLinks(true)`),
visible only when the model has 0 rows.
- Text: tr("MainWindow/messageNoFansDetected"), or tr("MainWindow/messageNoFansAvailable") when
  FUN_004b28c0() is true; passed through the markdown converter (section 8) so the
  `[reset the SMC](https://support.apple.com/en-us/HT201295)` part becomes an `<a href>` link.
- alignment 0x84 (centre), fixed height 40, fixed width = table width, moved so its centre is the
  table centre (also on every table resize, FUN_004b6b80), widget attribute 0x78
  (WA_TranslucentBackground), font weight 700 (bold).
- Style sheet: `QLabel { color : #293956; }` in light mode, `QLabel { color : #ffffff; }` when
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme == 0`
  (FUN_0059eab0).

### 1.5 Sensors table - `tableSensors` (`QTableViewSensors` : `QTableViewAltColor`)

Constructor FUN_004ba230, model `QTableViewSensorsModel` (FUN_004bb030), 2 columns.

| Property | Value | Where |
|---|---|---|
| alternating rows + stripe extension | as for the fans table | FUN_005a5bd0 / FUN_005a5c90 |
| showGrid / wordWrap / corner button | false / false / disabled | FUN_00465eb0 |
| vertical header | hidden, Fixed, default section size (row height) **18** (0x12) | FUN_004ba230 |
| horizontal header | not clickable; section 1 Stretch; column 0 width set explicitly on reload / language change to `tableWidth - (horizontalAdvance(header text of column 1) + 20) - (vertical scrollbar width if visible)` | FUN_004ba230, FUN_004ba6b0 |
| selection | SelectRows, SingleSelection | FUN_004ba230 |
| horizontal scroll bar | 1 (AlwaysOff); vertical: default (AsNeeded) | FUN_004ba230 |
| delegate | NoDrawFocusDelegate<QItemDelegate> | FUN_004ba230 |
| icon size | Qt default - the icons are the 16px resources | |
| Tab key | swallowed (FUN_004b66a0) | |
| context menu / double click | none | vtable 0x0075f9bc |

Headers (FUN_004bbd60): column 0 tr("MainWindow/listColumnSensor") "Sensor"; column 1
`tr("MainWindow/listColumnSensorValue") + " " + "°C"` or `"°F"` when setting `Fahrenheit` != 0
(FUN_004bce30) -> "Value °C".

Rows (FUN_004bc470 builds them from the sensor provider list): one row per sensor, with icon by
sensor type (FUN_0045d520): 0 `disc.png`, 1 `processor.png`, 2 `drive.png`, 3 `drive_ssd.png`,
4 `drive_nvme.png`, 5 `drive_external.png`, 6 `airport.png`, 7 `power-supply.png`, 8 `gpu.png`
(all `:/Resources/`); null icons are replaced by a 16x16 transparent pixmap (FUN_004bb220).
Immediately before the **first** sensor whose type is 2, 3, 4 or 5 (a drive) a separator row with
id -1 is inserted.

Cell data (FUN_004bb660):

| Role | Column 0 | Column 1 |
|---|---|---|
| Display, normal row | sensor name | `""` by default; `"-"` if the value is exactly 0.0; if value > 0: value converted to °F when `Fahrenheit` (`v*9/5+32`, FUN_004bd110) then, if `PreciseTemperature == 0`: `QString::number(round(v), 'g', 6)` (integer), else `QString::number(v, 'f', 1)` (one decimal). No unit suffix in the cell. Initial value is -1.0 (shows ""). |
| Display, separator row | tr("MainWindow/listSeparatorDrives") "Disk Drives:" | (empty) |
| Decoration | sensor icon; separator row: transparent 16x16 pixmap (FUN_0045d140, size scaled by FUN_0059fb40) | - |
| Font (role 6), separator row only | the table font with `pointSize - 1` (FUN_004ba480) | |
| Alignment | 0x81 Left|VCenter | 0x84 centre |
| Foreground (role 9) | - | RGB(128,128,128) if the sensor's last-update timestamp is valid and more than 5 seconds old (stale reading) |

Values arrive through `slotSensorUpdate(sensorId, value)` (FUN_00467bf0 -> FUN_004ba680) which
also forwards to the tray icon (FUN_004bf700). During a sensor list reload the table's signals are
blocked (`slotSensorsReloadBegin` FUN_00467c30, `slotSensorsReloadEnd` FUN_00467c50); after reload,
if the stored `traySensor` name no longer exists (FUN_0047ef20) it is reset to "" and the tray is
rebuilt.

### 1.6 Bottom buttons (FUN_004634e0)

Three buttons added to `buttonBox` with role 1 (RejectRole) in this insertion order (Qt's Windows
button-box layout puts them right-aligned, in insertion order):

| Order | Text | Action |
|---|---|---|
| 1 | tr("MainWindow/buttonMinimize") "Minimize" | virtual `vtbl+0xC0` = `QWidget::hide()` - hides the window to the tray |
| 2 | tr("MainWindow/buttonPreferences") "Preferences..." | `slotPreferences` (FUN_00467720) |
| 3 | tr("MainWindow/buttonAbout") "About..." (wrapper widget FUN_005b8300 around a QPushButton) | `slotInfoDialog` (FUN_004672f0) |

After (re)translation each button is passed to FUN_0059e130: if `button.width - textWidth < 20` the
text is replaced by `" %1 "` (one space each side) and the button `adjustSize()`d.

### 1.7 Decorations

Christmas snowflake (FUN_00469b40, called from both tables' paintEvent FUN_004b67a0 / FUN_004ba430):
when the current local date has `month == 12 && 24 <= day <= 31`, the image
`:/Resources/christmas_snowflake.png` is drawn centred in the table viewport at its natural size
with painter opacity 0.05.

### 1.8 Show / close / minimise / geometry

- `closeEvent` (FUN_005a1290, QMainWindowEx): if the virtual "hide instead of close" predicate
  (`vtbl+0xB8`, returns 1 for QFanControlMainWindow, FUN_004634c0) is true **and** the quit flag
  (`this+0x15`) is 0 -> call `vtbl+0xC0` (= `hide()`) and `event->ignore()`. Otherwise (quit
  requested) run the one-time shutdown handler `vtbl+0xBC` (FUN_00463330, section 4.7) and let the
  window close. So the title-bar X **hides to tray**; it never exits.
- The "Minimize" button also hides to tray. The title-bar minimise button is plain Qt behaviour
  (minimises to the taskbar); no changeEvent override was found that hides on minimise.
- `slotRestore` (FUN_00467ad0; tray "Show" and tray icon activation): if not visible ->
  `showNormal()`, `activateWindow()`, FUN_00593d20(this) (force foreground); then the Rate-me check
  (7.6).
- `slotExit` (FUN_00467020): calls virtual `vtbl+0x2C` with argument 1, then FUN_005a14e0 (one-time
  `QWidget::close()`). In the dumped vtable slot +0x2C resolves to the `QWidget::setVisible`
  import thunk, i.e. Exit makes the window visible and then quits. FUN_005a14e0 ("quit once",
  guarded by an INIT_ONCE) sets the quit flag `this+0x15 = 1` and calls `QWidget::close()`, so
  closeEvent takes the real-exit path.
- Session end: QMainWindowEx installs itself as a native event filter (FUN_005a1040); for event
  type `windows_generic_MSG` with `msg.message == 0x11` (WM_QUERYENDSESSION) it calls
  FUN_005a14e0 (quit) (FUN_005a13a0).
- First show after init (`slotInitFinished(false)`, FUN_00467390): only when app property
  `first-start` is set: `setWindowOpacity(0)`, `show()`, compute
  `h = headerHeight(sensors) + 10 + (windowHeight - sensorsTableHeight) + rowHeight(0) * sensorRowCount`
  and, if `h > windowHeight`, `resize(width, h)` (grow so every sensor row is visible without
  scrolling); centre on the screen's available geometry (FUN_0059df90); `setWindowOpacity(1.0)`,
  `show()`.
- Geometry restore (FUN_004658a0 -> FUN_0059e500): `restoreGeometry(value("WindowGeometry"))` and
  `restoreState(value("WindowState"), 0)` using the settings object at `app+0xc`. If either fails
  the window is centred on its screen (FUN_0059df90). See section 5 for the registry mapping.

### 1.9 Refresh

The main window has no timer of its own. It is driven by signals of the `QFanControl` core object
(`app+0x2c`, member +0x54) connected in FUN_00461d00: `slotInitFinished(bool crashed)`,
`slotFanUpdate(index)`, `slotSensorUpdate(id,value)`, `cpuUsageUpdate` (empty slot on Windows,
FUN_004634d0), `slotSensorsReloadBegin/End`, and the profile list's `profilesChanged`.
UNKNOWN: the polling period of the worker threads (lives in FanControl.cpp, outside the UI code
read for this spec).

If the init thread reports a crash (`slotInitFinished(true)`): message box titled "Crash" with text
"Initial thread crashed, please contact support for assistance. " (button 0x400 = OK), then the
tech-zip is created (FUN_00466b40) and the app exits.

Main-window slot table (moc dispatcher FUN_004cd490): 0 slotUpdatePreferences FUN_00468440,
1 slotSensorsReloadBegin FUN_00467c30, 2 slotSensorsReloadEnd FUN_00467c50, 3 slotInitFinished
FUN_00467390, 4 slotFanUpdate FUN_00467040, 5 slotSensorUpdate FUN_00467bf0, 6 cpuUsageUpdate
FUN_004634d0, 7 slotShowFanDialog FUN_00467da0, 8 slotResetFanMode FUN_00467a00, 9 slotPreferences
FUN_00467720, 10 slotInfoDialog FUN_004672f0, 11 slotRestore FUN_00467ad0, 12 slotExit
FUN_00467020, 13 slotHomePage FUN_00467200, 14 slotCheckForUpdates FUN_00466b20, 15 slotRateApp
FUN_00467950, 16 slotFollowTwitter FUN_00467160, 17 slotCreateTechInfoZip FUN_00466b40,
18 slotUpgradeToPro FUN_00468670, 19 slotShowNewPresetTip FUN_004680b0, 20 slotEnterLicense
FUN_00466f10, 21 uninstallApp FUN_00468ca0, 22/23 letUserSelectProfileName FUN_00464eb0,
24 deactivateLicense FUN_004643d0, 25 showLicenseWarning FUN_00466a40.


---------------------------------------------------------------------------------------------------
## 2. Tray icon (`QTrayIcon` -> `QTrayIconPlatform` -> `QTrayIconWin`)

`QTrayIcon` (FUN_004bda80) owns a QMenu (member +0xc) and a platform object `QTrayIconWin`
(FUN_004c0640, member +0x18) which wraps a plain **QSystemTrayIcon** (member +8). There is no
direct Shell_NotifyIcon code in the app; everything goes through Qt.

### 2.1 Icon image (FUN_004c08a0 = QTrayIconWin::update)

Windows has only **two** looks; the macOS options (colored / monochrome / none, fan RPM text,
two-line layout) do not exist in the Windows code path (see 6.3: the Windows "Tray icon" tab only
has the sensor combo).

| Case | Image |
|---|---|
| No tray sensor selected (`traySensor` empty or naming a sensor that no longer exists, FUN_004bf1b0) | QIcon built from `:/Resources/tray.png` at 16x16 and `:/Resources/tray@2x.png` at 32x32 (FUN_0045d9f0) |
| Tray sensor selected (flag bit 4 set by FUN_004bf940) | generated bitmap, below |

Generated bitmap:
- `dpr = QGuiApplication::primaryScreen()->devicePixelRatio()`
- `QImage(size = int(dpr * 16.0) square, Format 5 = Format_ARGB32)` (constants 16.0 at 0x00760210)
- painter font = painter default font with `setPointSize(int(dpr * 8.0))` (constant 8.0 at 0x00760200)
- `setRenderHint(1 = TextAntialiasing... raw value 1, true)`
- pen: solid brush of Qt::GlobalColor 3 (**white**), width 1.0
- `fillRect(whole image, Qt::GlobalColor 0xF)` = **Qt::darkBlue (#000080)** - opaque background
- text = `QString::number((int)round(value))` where value is the sensor temperature **in Celsius**
  (no Fahrenheit conversion here; the preferences tab shows "(Celsius only)"), drawn with
  `drawText(rect, 0x84 = AlignHCenter|AlignVCenter, text)`. No degree sign, no unit.
- `QIcon(QPixmap::fromImage(image))` -> `QSystemTrayIcon::setIcon`.

No dark-mode variant is chosen for the tray on Windows: `tray_white` / `tray_grey` resources are
not referenced by the Windows icon functions (only `tray.png`/`tray@2x.png`, FUN_0045d9f0 and
FUN_0045db90). The icon is refreshed whenever the selected sensor's value changes
(FUN_004bf700 -> FUN_004bf940) and when preferences change (FUN_004bf920).

### 2.2 Tooltip (FUN_004c04a0)

For each fan one line `"%1 - %2 RPM\n"` (%1 fan name, %2 current RPM, decimal), concatenated and
`trimmed()`; set with `QSystemTrayIcon::setToolTip`. Rebuilt on every `slotFanUpdate`
(FUN_00467040 -> FUN_004bef90) . Empty if the fan list is not ready.

### 2.3 Clicks

- `QSystemTrayIcon::activated(reason)` is handled by FUN_004c0810: only `reason == 2`
  (**DoubleClick**) does something: emits `showMainWindow` (FUN_004ca510) -> `slotRestore`.
  Single left click (Trigger) does nothing.
- Right click: Qt shows the context menu set via `setContextMenu(menu)` (FUN_004c0be0).
- The menu's default (bold) action is "Show Macs Fan Control" (`setDefaultAction`).

### 2.4 Tray menu (FUN_004bfa70, rebuilt on profile/fan/preference changes)

`notPro` = app property `pro-version` unset. `ready` = QTrayIcon member +0x14 (fan list
initialised). `backendOk` = `backend->vtbl[0x20]()` (FUN_004348f0).

| # | Item | Notes |
|---|---|---|
| 1 | tr("TrayMenu/itemShow") "Show Macs Fan Control" | default action; -> `showMainWindow` |
| 2 | separator | |
| 3 | **Fans block** and **Presets block** - only if `ready`. Order: Pro: presets block (if `backendOk`) then fans block. Not Pro: fans block then presets block (if `backendOk`). | |
| 4 | tr("MainWindow/buttonPreferences") "Preferences..." | -> `showPreferences` (FUN_004ca530) |
| 5 | separator | |
| 6 | submenu tr("TrayMenu/itemMoreLinks") "More" | contents below |
| 7 | separator | |
| 8 | tr("TrayMenu/itemExit") "Exit" | -> signal FUN_004ca140 -> `slotExit` |

Fans block (FUN_004be090):

| Item | Notes |
|---|---|
| tr("TrayMenu/itemAvailableFans") "Available fans:" | disabled header |
| per fan: submenu titled `"%1 - %2"` = fan name, **short** mode description (FUN_00450050 with short=1: "Auto", "Constant value of N", "Sensor-based", ...) | actions are in a QActionGroup; action data strings `"<index>:control"` / `"<index>:reset"`, dispatched by `slotProcessFanAction` (FUN_004bf750) to `showFanDialog(index)` / `resetFanMode(index)` |
| - tr("MainWindow/popupMenuItemControl") "Change control..." | always |
| - separator + tr("MainWindow/popupMenuItemReset") "Reset to automatic control" | only if the fan's mode != 2 (automatic) |
| separator | after the last fan |

Presets block (FUN_004be7c0):

| Item | Notes |
|---|---|
| tr("TrayMenu/itemFanPresets") "Fan presets:" | disabled header |
| one action per predefined preset ("Automatic", "Full blast") | active one checkable+checked; -> `setActiveProfile` |
| one action per custom preset | only if Pro and at least one exists; active one checked; -> `setActiveProfile` |
| submenu tr("Presets/presetCurrentUnsaved") "Custom*" containing tr("Presets/presetMenuSave") "Save...", separator, tr("Presets/presetMenuDiscard") "Discard (reset all fans to auto)" | only when the state is unsaved (FUN_00453f10); -> `unsavedSave` / `unsavedDiscard` |
| separator | |

"More" submenu:

| Item | Signal -> main window slot |
|---|---|
| tr("TrayMenu/itemUpgradeToPro") "Upgrade to Pro version" + separator | only if `notPro`; FUN_004ca080 -> `slotUpgradeToPro` (license UI - leave out) |
| tr("MainWindow/buttonAbout") "About..." | FUN_004ca4b0 -> `slotInfoDialog` |
| tr("TrayMenu/itemCheckForUpdates") "Check for updates..." | FUN_004ca0a0 -> `slotCheckForUpdates` (update checker - leave out) |
| tr("TrayMenu/itemTechInfoZip") "Create tech zip file..." | FUN_004ca100 -> `slotCreateTechInfoZip` |
| separator | |
| tr("TrayMenu/itemRateApp") "Rate on MacUpdate" | FUN_004ca410 -> `slotRateApp` (opens `https://www.macupdate.com/app/mac/47386/macs-fan-control`, sets `RateOnMacupdateSkip=1`, FUN_00467950) |
| tr("TrayMenu/itemFollowTwitter") "Follow @crystalidea" | FUN_004ca160 -> `slotFollowTwitter` (`https://twitter.com/crystalidea`, FUN_00467160) |
| separator | |
| tr("TrayMenu/itemWebsite") "Website" | FUN_004ca180 -> `slotHomePage` (`https://crystalidea.com/?source=fancontrol_win`, FUN_00467200) |

"Uninstall..." (`TrayMenu/itemUninstall`) is **not** added on Windows (string not referenced by
FUN_004bfa70). No item has an icon. The tray icon is hidden at shutdown
(FUN_00463330 -> FUN_004bf730(0) -> `QSystemTrayIcon::setVisible(false)`).

---------------------------------------------------------------------------------------------------
## 3. ChangeControlModeDialog (`QChangeControlModeDialog`)

Constructor FUN_00439f90 (base `QDialogEx` FUN_0059e850), setupUi FUN_0043d6b0, designer texts
FUN_0043cdc0, runtime texts FUN_0043c6e0, populate FUN_0043b540, connections FUN_0043bc90, buttons
FUN_0043b9b0. Opened modally from `slotShowFanDialog` (FUN_00467da0) for one fan.

### 3.1 Window

- Title: tr("FanControlDialog/title").arg(fan name) = "Change fan control for the '%1' fan".
- Window icon: that fan's icon (FUN_00439f90).
- setupUi `resize(465, 259)`; after populating: `adjustSize()` then `setFixedSize(size())` - the
  dialog is **not resizable** and its size is whatever the layout needs.
- Every optional widget gets `sizePolicy.setRetainSizeWhenHidden(true)` (FUN_0059e7f0), so hiding
  the sensor controls does not change the dialog size.

### 3.2 Layout tree (FUN_0043d6b0). Margins/spacing are not set explicitly (Qt style defaults).

- `verticalLayout` QVBoxLayout
  - `labelTitle` QLabel, textFormat raw value 2, alignment 0x84 (centre). Designer text `%title%`.
    The constructor calls `setVisible(...)` on it (virtual +0x2C on member +0x90, FUN_00439f90);
    the argument is not visible in the decompilation. INFERRED hidden on Windows, because no code
    ever replaces the `%title%` placeholder text and the title is put in the window caption.
  - `gridLayout` QGridLayout (rows x columns):
    - (row 0, col 0) `radioConstant` QRadioButton - tr("FanControlDialog/radioConstant") "Constant RPM value"
    - (row 0, col 1, colspan 2) `horizontalLayout_2`: `sliderRPM` QSlider (Horizontal), then
      `spinConstant` QSpinBox: minimumSize 40x0, maximumSize 60x16777215, wrapping false, frame
      true, buttonSymbols 2 (NoButtons), no suffix, no special value text
    - (row 1, col 0) `radioSensor` QRadioButton - tr("FanControlDialog/radioSensor") "Sensor-based value"
    - (row 1, col 1, colspan 2) `comboSensor` QComboBox, `maxVisibleItems = 15`
    - (row 2, col 0, colspan 2) `labelIncrease` QLabel, wordWrap true -
      tr("FanControlDialog/staticIncrease") "Temperature that the fan speed will start to increase from:"
    - (row 2, col 2) `horizontalLayout_3`: spacer 40x20 (Expanding, Minimum), `spinIncrease` QSpinBox,
      `labelDegree1` QLabel = "°C" / "°F" (FUN_004bce30)
    - (row 3, col 0, colspan 2) `labelMax` QLabel - tr("FanControlDialog/staticMax") "Maximum temperature:"
    - (row 3, col 2) `horizontalLayout_4`: spacer 40x20 (Expanding, Minimum), `spinMax` QSpinBox,
      `labelDegree2` QLabel = "°C" / "°F"
  - `checkSaveProfile` QCheckBox
  - `buttonBox` QDialogButtonBox, no standard buttons; code adds (FUN_0043b9b0):
    OK = QPushButton(tr("Common/buttonOK")) [member +0x1c] -> `OnApply` (FUN_0043ab90);
    Cancel = QPushButton(tr("Common/buttonCancel")) -> `OnCancel` (FUN_0043ae30 = `done(0)`).
    One `setAutoDefault(true)` call is made on the member +0x1c button and one
    `addButton(button, 1 = RejectRole)` call is visible. UNKNOWN: the role constant used for the
    other button (second addButton not visible in the decompilation; AcceptRole for OK assumed).
  - `labelFooter` QLabel, wordWrap true, hidden unless the min==max warning applies (3.6)
- Tab order: radioConstant -> spinConstant -> radioSensor -> comboSensor -> spinIncrease -> spinMax.

### 3.3 Ranges and initial values (FUN_0043b540)

Fan record used: `min` (int16 @+0x1e), `max` (@+0x20), a third value @+0x1c.

| Widget | Min | Max | Initial value |
|---|---|---|---|
| sliderRPM | fan min - or **0 if Ctrl+Shift were both held when the dialog was opened** (`QGuiApplication::keyboardModifiers()` has both 0x04000000 and 0x02000000; flag at this+0x26) | fan max | if the starting mode is Constant: its RPM; otherwise the fan record value @+0x1c (INFERRED: current RPM) |
| spinConstant | same as slider min | fan max | same as slider |
| spinIncrease | `toDisplay(20)` | `toDisplay(100)` | see below |
| spinMax | `toDisplay(20)` | `toDisplay(100)` | see below |

`toDisplay(c)` = c when `Fahrenheit == 0`, else `c*9/5+32` truncated to int (FUN_004bd040); i.e.
20..100 °C or 68..212 °F. Single step is Qt's default (1) for all spin boxes and the slider; no
tick marks, no page-step override.

Starting mode: the fan's current mode. If the fan is currently Automatic, the dialog starts from
the mode last applied through this dialog for that fan during this run (static per-fan array
`DAT_0089ed28`, FUN_00439f90 / FUN_0043ab90).

- starting mode Constant (3) or Automatic (2): `radioConstant` checked.
- starting mode Sensor (4): `radioSensor` checked; if its sensor id is found in the combo
  (`findData`), select it and set spinIncrease = toDisplay(start temp), spinMax = toDisplay(max temp).
- any other starting mode: no radio checked -> OK disabled.

### 3.4 Sensor combo

One item per sensor supplied by the caller: `insertItem(count, icon, name, userData = sensor id
string)` in the order of the supplied list. INFERRED: the list is the same sensor list/order as
the sensors table (without the separator); the callback that fills it comes from FUN_00467da0
and was not traced item by item. Each sensor record also carries a default start temperature
(@+0x1c) and default max temperature (@+0x20), in Celsius.

### 3.5 Enabling logic and validation

`OnFanControlChanged` (FUN_0043ae40, on either radio `toggled`):
- First hide (`setVisible(false)`) comboSensor, spinConstant, sliderRPM, labelIncrease, labelMax,
  spinIncrease, spinMax, labelDegree1, labelDegree2. OK enabled = `!warningFlag` (this+0x24).
- radioConstant checked: show spinConstant + sliderRPM; focus spinConstant and `selectAll()`.
- radioSensor checked: show comboSensor, labelIncrease, labelMax, spinIncrease, spinMax, both degree
  labels; focus comboSensor; run `OnSensorChanged`.
- neither: OK disabled.
(Controls are shown/hidden, not enabled/disabled; their space is retained.)

Slider <-> spin sync: `sliderRPM.valueChanged -> spinConstant.setValue`; slider `valueChanged`
also focuses spinConstant and selects all (FUN_0043c4e0); `spinConstant.valueChanged` sets the
slider value with the slider's signals blocked (FUN_0043c540).

`OnSensorChanged` (FUN_0043b040, on `comboSensor.currentIndexChanged`):
- if the selected sensor is the one of the fan's current Sensor mode: keep the spin values and set
  `spinMax.minimum = spinIncrease.value + D`;
- otherwise load that sensor's defaults: spinIncrease = toDisplay(default start),
  spinMax = toDisplay(default max).

`OnIncreaseValueChanged(v)` (FUN_0043b000, on `spinIncrease.valueChanged`):
`spinMax.setMinimum(v + D)`.

`D` = **8** when Celsius, **14** when Fahrenheit: the maximum temperature must always be at least
8 °C (14 °F) above the start temperature; Qt clamps spinMax's value up when its minimum rises.
There is no other validation and no error message.

### 3.6 min == max warning (FUN_0043d140, FUN_0043d4c0)

If the fan's `min == max`: OK is disabled permanently (flag this+0x24 = 1) and `labelFooter` is
shown with rich text
`<table><tr><td></td><td><img src=":/Resources/warning.png"/></td><td valign="middle">&nbsp;TEXT</td></tr></table>`
where TEXT = tr("FanControlDialog/statusWarning").arg(min) ("Something is wrong - min/max fan
values are equal (%1).<br/> Please make sure to uninstall any other fan-controlling software and
to reset the SMC."). The same text is the RPM-cell tooltip in the main table (1.4).

### 3.7 "Save to the current preset" checkbox (FUN_0043c6e0)

Shown only when the caller supplies a non-empty preset name, which FUN_00467da0 does only when the
active preset is a **custom** preset (FUN_00453e20). Text
tr("FanControlDialog/saveToCurrentPreset").arg(name) = "Save to the current preset ('%1')".
Otherwise hidden. Unchecked by default.

### 3.8 OK (`OnApply`, FUN_0043ab90)

- radioConstant: result mode = Constant(spinConstant.value) (FUN_0044f7a0).
- radioSensor with a non-empty selected sensor id: result mode = Sensor(id,
  start = toCelsius(spinIncrease.value), max = toCelsius(spinMax.value)) (FUN_0044fd40;
  FUN_004bcd60 converts display units back to Celsius).
- `saveToPreset` (this+0x25) = checkbox visible && checked.
- The result is stored in the dialog, remembered in the per-fan static array, and the dialog is
  finished with result 1.
- Caller (FUN_00467da0 / FUN_00464d10): if the result mode differs from the fan's current mode
  (FUN_0044f5a0 compare) it is applied with FUN_0044c7f0 (set fan mode, with the save-to-preset
  flag); the fan row is refreshed. If not saved to a preset the profile state becomes "Custom*".

Cancel / closing the dialog: no change.
