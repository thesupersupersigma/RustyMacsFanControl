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

---------------------------------------------------------------------------------------------------
## 4. Startup and shutdown sequence

Entry: `main`-level function FUN_00460740 (called from the CRT start-up FUN_00623c50).
Application class `QMacFanControlApp` (ctor FUN_00431350) : single-instance base FUN_0059a8a0 :
QApplication wrapper FUN_00593720.

### 4.1 Before QApplication (FUN_00460740)

1. `QCoreApplication::setAttribute(13, true)`, `setAttribute(20, true)`, `setAttribute(4, true)`
   (raw values; 13/20 are the legacy high-DPI attributes AA_UseHighDpiPixmaps /
   AA_EnableHighDpiScaling, no-ops in Qt 6; 4 = AA_DontCreateNativeWidgetSiblings).
2. Three `qRegisterMetaType` typedef registrations.
3. `QGuiApplication::setHighDpiScaleFactorRoundingPolicy(1)` = **Round** (scale factors are
   rounded to integers: 125% -> 1, 150% -> 2, 175% -> 2).
4. Construct the app object.

### 4.2 App constructor (FUN_00431350)

1. `QApplication(argc, argv)`; `setApplicationName("Macs Fan Control")`; thread name "mainThread";
   `setApplicationVersion("%2.%3.%4")` filled from the EXE version resource (FUN_00593dd0).
2. **Single instance** (FUN_0059a8a0): a `QLocalSocket` tries `connectToServer("MACSFANCONTROL")`
   and `waitForConnected(500)`. On Windows a QLocalServer name is the named pipe
   `\\.\pipe\MACSFANCONTROL`.
   - connected -> flag `alreadyRunning` (this+0x18) = 1 (nothing is sent; the mere connection is
     the signal).
   - not connected -> create `QLocalServer`, `listen("MACSFANCONTROL")`, connect `newConnection()`
     to the app signal that ends in `slotAnotherInstanceStarted` -> FUN_004360f0: if the main
     window exists, call its `slotRestore` (show + activate).
   No mutex and no window-class lookup are used.
3. Sysinfo object (FUN_004b1d70), logger/license manager set-up (FUN_004329c0, FUN_00433360),
   SMC service object, QFanControl core (FUN_00442760).
4. Settings registration and defaults (FUN_00433790, section 5).
5. Language: default resource `:/build/languages/English.xml` (FUN_00593a60), then load
   `InterfaceLanguage` (FUN_00595b90, section 8); the resolved file name is written back to
   `InterfaceLanguage`.
6. License check (FUN_00432320) - sets app property `pro-version`. Leave out.

### 4.3 `init()` (FUN_00432f70) -> result code

1. Parse arguments (FUN_00435010), `QCoreApplication::arguments()`, exact-match, case-sensitive
   compare (`compare_helper(..., CaseSensitive)`):

| Switch | Effect | Where |
|---|---|---|
| `/minimized` | `app+0x30 = 1`: main window is created but not shown (starts in the tray). Used by autostart. | FUN_00435010 |
| `/log` | app property `log-file` = "yes" -> log file is written next to the EXE (4.6) | FUN_00435010 |
| `/smcdump` | app property `smc-dump` = "yes" (consumed by the fan-control core, FUN_0044b0d0: SMC key dump into the log) | FUN_00435010 |
| `/uninstall` | calls `platform->vtbl[0x14]()` (FUN_00437b00: remove driver/service `AppleSMC`, registry key `Software\CrystalIdea Software\Macs Fan Control`, autostart; run by the uninstaller) and returns result 1 -> process exits silently | FUN_00435010 |
| `/init` | installer-time initialisation: skips the trial dialog; if the OS check FUN_00437050(10) passes it runs a legacy `AppleSMC.exe /uninstall` from the app folder if that file exists, then installs the kernel driver (FUN_004380f0: `applesmc.sys` / `applesmc32.sys`, service name `AppleSMC`, display name "Apple SMC service"); returns result 1 (exit, no UI) | FUN_00437360 |
| `/skipcheck` | skips both the virtual-machine check and the "is this Apple hardware" check | FUN_00437360 |
| `/inpout32` | force SMC backend 0 (inpout32.dll port I/O) (FUN_004b1b20(0)) | FUN_00437360 |
| `/applesmc` | force SMC backend 1 (applesmc.sys driver) (FUN_004b1b20(1)) | FUN_00437360 |

2. First run: if `AppFirstRun != 0` -> write `AppFirstRun = 0` and set app property
   `first-start` (FUN_00436010).
3. `printSysInfo` (FUN_00435390) logs: `"Macs Fan Control 1.5.21"`, `"Process PID=%1"`, the OS
   description string, the machine model, `"%1 [%2 cores]"` (CPU name, core count), `"Qt 6.8.3"`.
4. If `alreadyRunning` -> return 6.
5. Otherwise return `platform->vtbl[4]()` = FUN_00437360:
   1. `FUN_00593fe0(app, true)`: enable dark-mode support + watcher (section 9).
   2. Unless `/init`: if `pro-version` is not set, the trial dialog is shown here (FUN_004c0c30)
      and property `trial-version` is set. **Trigger point only - leave out.**
   3. Unless `/skipcheck`: VM check FUN_00438c60 (reads
      `HKLM\SYSTEM\CurrentControlSet\Control\SystemInformation\SystemManufacturer`; contains
      "Parallels" or "VMware" (case-insensitive `indexOf`) -> VM) -> return 5. Then: machine model
      string empty (not Apple hardware) -> return 4.
   4. `/init` path as in the table -> return 1.
   5. Normal path: `CoInitializeEx(NULL, 0)`; crash-report set-up with
      "support@crystalidea.com" (FUN_00594890); backend overrides; then if app property
      `win-no-support` is not set and the SMC service init FUN_004ac360() returns 2 -> return 3,
      else return 0.

### 4.4 Result handling (switch in FUN_00460740)

| Code | Action |
|---|---|
| 0 | `platform->vtbl[8]()` (FUN_00433350 -> FUN_00432dc0: create the main window; start worker threads) then `QApplication::exec()` |
| 1 | exit silently (`/uninstall`, `/init`) |
| 2 | FUN_004330c0: `QProcess::startDetached(applicationFilePath(), [])` - relaunch itself, then exit (used by the license flow) |
| 3 | QMessageBox (parent 0): icon 2 (Warning), text `<b>Error initializing SMC interaction layer</b>`, informative text "Please make sure that your Mac system is supported and you have correctly installed the program.", window title = the SMC error string from the platform object, window icon = tray icon (FUN_0045d9f0). Buttons: "Copy log" (role value 6; copies the whole log text to the clipboard, FUN_00460610 -> FUN_00434cf0) and standard OK (0x400). Then exit. |
| 4 | native `MessageBoxW` with caption "Macs Fan Control 1.5.21" and text "Unsupported hardware detected. The program is able to run on Apple hardware only. \n\nIf you're sure you run the app on Apple computer, please use the /skipcheck command line switch." Then exit. |
| 5 | unless `NoVmWarning == 1`: QMessageBox, icon 2 (Warning), text `<b>Virtual machine detected</b>`, informative text "Unable to start Macs Fan Control in a virtual machine.", OK button, window icon = tray icon, check box "Don't display this warning again" (checked -> `NoVmWarning = 1`, FUN_00460530). Then exit (in both cases the app does not run in a VM). |
| 6 | another instance is running: exit immediately (the first instance has already restored its window) |

After the switch: app destructor (FUN_004319f0), return code.

### 4.5 Main window creation (FUN_00432dc0)

- `new QFanControlMainWindow` (section 1); the tray icon object is created with it.
- If not `/minimized`: if app property `first-start` is **not** set -> `show()` immediately
  (geometry restored from settings). If `first-start` is set the window is shown later from
  `slotInitFinished` auto-sized and centred (1.8).
- With `/minimized` the window stays hidden; only the tray icon appears.

### 4.6 Log file

- Logger: spdlog logger named "SimpleLogger", pattern
  `[%Y-%m-%d %H:%M:%S:%e] [thread %t] [%s#%#] %v` (FUN_005536e0) (millisecond timestamp, thread
  id, source file#line, message).
- Path (FUN_00434e10): without `/log`: `QDir::tempPath() + "\MacsFanControl.log"`
  (i.e. `%TEMP%\MacsFanControl.log`); with `/log` (property `log-file`): `platform->vtbl[0x18]()` =
  the EXE path with `.exe` replaced by `.exe.log` (FUN_00437e80), e.g.
  `C:\Program Files (x86)\MacsFanControl\MacsFanControl.exe.log`.
- INFERRED: the file sink is always created (FUN_00433360 calls the sink factory unconditionally
  with the path); there is no "enable log" registry switch on Windows - `EnableLog` is in the list
  of obsolete values that are deleted (section 5). UNKNOWN: truncate-vs-append and rotation
  parameters (a rotating sink exists in the binary; arguments not traced).

### 4.7 Shutdown (FUN_00463330, run once from closeEvent when quitting)

1. Save window geometry/state and splitter sizes (FUN_00465b50, section 5).
2. `hide()` the main window; hide the tray icon (FUN_004bf730(0)).
3. `QFanControl::stopAndWaitForFinished()` (FUN_0044db90) - stops the worker threads (restoring
   fan state is part of the fan-control core, not covered here).
4. If that fails: message box, title "Macs Fan Control 1.5.21", text "Unable to terminate the app
   in the right way, \nplease contact support for assistance and resolving the problem.", OK.
5. The window closes; `exec()` returns; app destructor.
Quit triggers: tray "Exit" (`slotExit`), WM_QUERYENDSESSION (1.8), fatal init crash.

---------------------------------------------------------------------------------------------------
## 5. Settings

Storage: `QSettings(organization = "CrystalIDEA Software", application = "Macs Fan Control")`
(FUN_00433790 -> FUN_00593b00 -> FUN_00596f80), native format = registry key
**`HKCU\Software\CrystalIDEA Software\Macs Fan Control`**. Qt stores ints as REG_DWORD, strings as
REG_SZ, string lists as REG_MULTI_SZ, QByteArray as REG_SZ `@ByteArray(...)` (or REG_BINARY when it
contains NULs). Every write is followed by `QSettings::sync()` (FUN_00599ab0 etc.).
Typed accessors: int read FUN_005987a0 / write FUN_005999b0; string read FUN_005988a0 / write
FUN_00599ab0; string-list read FUN_005989b0 / write FUN_00599bb0; QDateTime (stored as string)
read FUN_00598640 / write FUN_00599880; QByteArray read FUN_00598530 / write FUN_00599780.
Defaults are registered in FUN_00433790 (and FUN_00438f80 for the last two).

| Value | Type | Default | Read | Written |
|---|---|---|---|---|
| `Fahrenheit` | int 0/1 | 0 | sensors header/values FUN_004bbd60, FUN_004bd110, FUN_004bce30; fan dialog FUN_004bd040, FUN_004bd1e0, FUN_004bcd60; prefs FUN_0046d410 | prefs `OnTempScaleChanged` FUN_0046db40 |
| `PreciseTemperature` | int 0/1 | 0 | sensors model FUN_004bb660; tray FUN_004bf940; prefs FUN_0046d410 | prefs `OnPreciseChanged` FUN_0046da90 |
| `ShowDrives` | int 0/1 | **1** | sensor provider FUN_0047e120; prefs FUN_0046d410 | prefs FUN_0046dd80 |
| `ShowDrivesExternal` | int 0/1 | 0 | FUN_0047e120; prefs | prefs FUN_0046de70 |
| `ShowEGpuTemp` | int 0/1 | default taken from the GPU provider capability (`provider->vtbl[0xc]()` when the machine has Thunderbolt, FUN_004b2650); forced to 0 at start-up when eGPU is unsupported | prefs FUN_0046d410 | FUN_00433790 (forced 0), prefs FUN_0046df60 |
| `traySensor` | string (sensor identifier/name as stored in the combo item data) | "" (none) | tray FUN_004bf1b0; prefs FUN_0046bf30; FUN_00467c50; FUN_0044e460 | prefs `OnSensorTrayChanged` FUN_0046bdb0; reset to "" in FUN_00467c50 when the sensor vanished |
| `InterfaceLanguage` | string, language file name e.g. `English.xml` | "" (= follow system language) | FUN_00431350 | FUN_00431350 (resolved name written back at every start), prefs FUN_0046ab60 |
| `ActivePreset` | string | "" | profile list FUN_00453fa0 | FUN_00454f70 |
| `CustomPresets` | string list | empty | FUN_00454670 | FUN_00455210 (format belongs to the preset spec) |
| `License` | string | "" | FUN_00432320, FUN_004349d0 | license code - leave out |
| `AppFirstRun` | int | non-zero when absent (the first run writes 0); the registered default constant is not visible in the decompilation - INFERRED 1 | FUN_00432f70 | FUN_00432f70 (set to 0) |
| `RateOnMacupdateSkip` | int 0/1 | 0 | FUN_0046f6b0 | FUN_0046f550 (set 1) |
| `RateOnMacupdateNextDate` | QDateTime as string (Qt::TextDate, `QDateTime::fromString(s, format 0)`) | invalid | FUN_0046f490, FUN_0046f6b0 | FUN_0046f5e0 (now + 604800 s = 7 days) |
| `WindowSplitterPosition` | string list of 2 decimal ints (pane widths in logical px: left, right) | empty -> `[width-240, 240]` | FUN_004658a0 | FUN_00465b50 at shutdown |
| `WindowGeometry` | QByteArray = `QWidget::saveGeometry()` blob | empty | FUN_0059e500 | FUN_0059e6c0 at shutdown |
| `WindowState` | QByteArray = `QMainWindow::saveState()` blob (version 0) | empty | FUN_0059e500 | FUN_0059e6c0 at shutdown |
| `NoVmWarning` | int 0/1 | 0 (FUN_00438f80) | FUN_00460740 | FUN_00460530 |
| `WindowState2` | QByteArray, default empty (FUN_00438f80) | - | **Not window geometry.** It is read/written only by the trial-period code (FUN_004c2100, FUN_004c25a0, FUN_004c2a50); the name is a disguise. Leave out with the trial/licence feature. | |

`WindowGeometry` / `WindowState` are opaque Qt blobs (magic 0x1D9D0CB, version, frame/normal
geometry rects, screen number, maximized/fullscreen flags, screen width, ...). A native rewrite
cannot usefully reuse them: store its own placement (e.g. WINDOWPLACEMENT) under a new value name
and ignore the Qt values.

Obsolete values deleted at start-up if present (registered as removed, FUN_00433790 ->
FUN_00597f80): `Fans`, `Machine`, `CPU`, `TempSensors`, `ShowGpuTemp`, `EnableLog`,
`ShowSmartDrives`, `ShowNVMeDrives`, `Fan_0` ... `Fan_9`.

There are **no** registry values for tray icon style, tray fan or two-line display on Windows
(those controls are hidden, 6.3). The update checker keeps its own values (auto-check flag etc.,
FUN_005a39d0 / FUN_005a3bc0) - leave out with the update checker.

Other registry locations touched:
- `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme` (read, dark mode, FUN_0059eab0)
- `HKLM\SYSTEM\CurrentControlSet\Control\SystemInformation\SystemManufacturer` (read, VM check, FUN_004388c0)
- `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\MacsFanControl` (legacy autostart, 6.4)

---------------------------------------------------------------------------------------------------
## 6. Preferences window (`QPreferencesDialog`)

Created by `slotPreferences` (FUN_00467720) through the dialog helper (7.7); constructor
FUN_0046a1b0 on top of a generic "toolbar preferences" dialog FUN_005b9070.

### 6.1 Frame

- QDialog with widget attribute 0x4C cleared (`setAttribute(76, false)`); window icon = app icon
  (FUN_0059fb90).
- A **QToolBar** child (FUN_005bc470): not floatable, not movable,
  `toolButtonStyle = 3` (ToolButtonTextUnderIcon), `iconSize = 32x32`, one checkable action per
  page in an exclusive QActionGroup; toolbar is `adjustSize()`d after pages are added.
- Pages are plain child widgets of the dialog; only the current one is visible. Selecting a page
  (FUN_005b9ac0): hide all, show the page, page is moved to `(40, toolbarHeight + 0)`, and the
  dialog is resized to `(toolbarAreaWidth, pageHeight + toolbarHeight)` where the width comes from
  FUN_005bc8f0 (derived from the toolbar buttons' widths; exact formula not traced - UNKNOWN) -
  instantly on first show, otherwise with a `QPropertyAnimation` on `size`, duration 150 ms.
- Window title = `tr("PreferencesWindow/title") + ": " + pageTitle` e.g. "Preferences: General".
- No OK/Cancel buttons; every control applies immediately.

| # | Page | Title | Icon |
|---|---|---|---|
| 0 | `QPreferencesGeneral` (FUN_0046a6f0) | tr("PreferencesWindow/TabGeneral") "General" | `:/Resources/preferences_general.png` |
| 1 | `QPreferencesSensors` (FUN_0046d410) | `"T° %1".arg(tr("PreferencesWindow/tabSensors"))` = "T° Sensors" (FUN_0046dc10) | `:/Resources/preferences_sensors.png` |
| 2 | `QPreferencesMenubar` (FUN_0046bb40) | tr("PreferencesWindow/tabTrayIcon") "Tray icon" | `:/Resources/preferences_menubar.png` |

(icons FUN_0045d750). There is no Diagnostics tab on Windows (`tabDiagnostics`, `checkLog`,
`browseLog`, `preferences_diagnostics.png` are not referenced).

Signals: any change emits the dialog's "preferences changed" signal -> main window
`slotUpdatePreferences` (FUN_00468440: retranslate everything, refresh tables, rebuild tray);
sensor-list options additionally emit `sensorsReloadRequired` -> core reloads sensors.

### 6.2 General page (setupUi FUN_0046b1d0, 422 x 121; texts FUN_0046b970)

- `verticalLayout` QVBoxLayout
  - `checkAutostart` QCheckBox - tr("PreferencesWindow/checkSystemStart") "Autostart minimized when you log in (recommended)". Initial state = `platform->vtbl[0x10]()` (autostart exists, 6.4). Toggle -> `OnAutoStartChanged` (FUN_0046aad0): `platform->vtbl[0xc](checked)` then re-reads the real state back into the box.
  - `checkUpdates` QCheckBox - tr("PreferencesWindow/checkUpdatesOnStartup") "Check for updates when the app starts"; state from/to the update checker (FUN_005a39d0 / FUN_005a3bc0). Leave out or keep as a dead setting.
  - `checkDockIcon` (`QCheckBoxDockIconToggle`) - **hidden on Windows** (`setVisible(false)` in FUN_0046a6f0)
  - `horizontalLayout`: `labelLanguage` QLabel (alignment 0x82 = AlignRight|AlignVCenter, wordWrap) tr("PreferencesWindow/staticLanguage") "Language:"; `comboLanguage` (minimumSize 210x0, maximumSize 195x16777215 as coded, `maxVisibleItems = 30`); `labelTranslate` QLabel (alignment 0x82) with rich text `<a href="translate">Translate</a>` (not translated); spacer 40x20 (Expanding, Minimum)
- `adjustSize()` after texts are set.

Language combo (FUN_005a52f0): one item per `*.xml` file found in `<exe dir>\languages`
(`QDir::entryList(filter = Files)`, directory order = alphabetical), item data = file base name
(without `.xml`), item text = base name, or `"%1 (%2)"` with %2 = native language name from a
built-in table (FUN_00623e00) when the table has an entry. Current item = current language.
`OnLanguageChanged` (FUN_0046ab60): if the selected name differs from the current one: load
`<name>.xml` (FUN_00595b90), write `InterfaceLanguage = <name>.xml`, emit the changed signals ->
all windows retranslate live (no restart).
`labelTranslate` link `translate` -> opens
`https://crystalidea.com/macs-fan-control-translation?source=fancontrol_win` (FUN_0046ad20).

### 6.3 Tray icon page (setupUi FUN_0046c390, 421 x 161; texts FUN_0046ce90)

- `verticalLayout`
  - `gridLayout`:
    - (0,0) `labelIcon` "Icon:" / (0,1) `comboIcon` - **hidden on Windows**
    - (1,0) `labelFan` "Fan:" / (1,1) `comboFan` - **hidden on Windows**
    - (2,0) `labelSensor` QLabel max width 100, alignment 0x82 (right), wordWrap - tr("PreferencesWindow/staticSensor") "Sensor:"
    - (2,1) `comboSensor` QComboBox, `maxVisibleItems = 20`
    - (2,2) `staticCelsius` QLabel - tr("PreferencesWindow/staticCelsiusOnlyOnWindows") "(Celsius only)"
  - `checkTwoLines` - **hidden on Windows**
  - vertical spacer 17x1 (Minimum, Expanding)

(The five hidden widgets are hidden with plain `setVisible(false)` in FUN_0046bb40.)
`comboSensor` (FUN_0046bf30): item 0 = tr("PreferencesWindow/comboOptionNone") "None" with a
transparent 16x16 icon and null data; then one item per sensor (sensor icon, sensor name, data =
sensor identifier string). Selection = item whose data equals `traySensor`, else 0.
`OnSensorTrayChanged(index)` (FUN_0046bdb0): `traySensor = (index < 1) ? "" : itemData(index)`,
emit changed -> tray icon redrawn (2.1).

### 6.4 Sensors page (setupUi FUN_0046e370, 445 x 175; texts FUN_0046eae0)

- `verticalLayout`
  - `labelSensors` QLabel tr("PreferencesWindow/labelSensors") "Read temperatures of"
  - `checkDrives` tr("PreferencesWindow/checkDrives") "Connected NVMe/SATA drives" (`ShowDrives`)
  - `horizontalLayout`: `checkDummy` QCheckBox - **hidden but keeps its size**
    (`setRetainSizeWhenHidden` + `setVisible(false)`), acting as an indent; `checkDrivesExternal`
    tr("PreferencesWindow/checkDrivesExternal") + `" (USB)"` or `" (Thunderbolt/USB)"` when the
    machine has Thunderbolt (FUN_004b2650) -> "Including external drives (USB)"
    (`ShowDrivesExternal`); spacer 40x20 (Expanding, Minimum)
  - `checkGPUThunderbolt` tr("PreferencesWindow/checkExternalGPU") "eGPUs connected via Thunderbolt" (`ShowEGpuTemp`); disabled unless the machine has Thunderbolt and the GPU provider supports it
  - `line` QFrame, frameShape 4 (HLine), frameShadow 0x30 (Sunken)
  - `checkFahrenheit` tr("PreferencesWindow/checkFahrenheit") "Use Fahrenheit temperature scale"
  - `checkPrecise` tr("PreferencesWindow/checkPrecise") "Precise display of temperature when possible (e.g. 45.4)"

Behaviour: each box writes its value immediately (table in section 5). The three "read
temperatures of" boxes also emit `sensorsReloadRequired`. `slotDrivesChecked(bool)`
(FUN_0046eaa0): `checkDrivesExternal.setEnabled(checked)`; when unchecked it is also unchecked.

### 6.5 Autostart (FUN_00439400 set, FUN_004386d0 query) - `WindowsTasks::*` wrappers over Task Scheduler 2.0 COM

On Windows Vista or later (FUN_00437050(6,0,0)):

| Property | Value | Wrapper |
|---|---|---|
| Task name | `MacsFanControl` | FUN_00562cd0 |
| Folder | root folder `\` (the folder-name argument is the empty string at 0x00756204) | FUN_00562cd0 |
| RegistrationInfo.Author | `CrystalIDEA Software` | TaskDefinition::setAuthor FUN_00564190 |
| Principal.RunLevel | 1 = TASK_RUNLEVEL_HIGHEST (needed because the EXE is `requireAdministrator`) | setHighestPriviledges FUN_005643b0 |
| Principal.LogonType | 3 = TASK_LOGON_INTERACTIVE_TOKEN | setLogonType FUN_00564480 |
| Trigger | one trigger of type 9 = TASK_TRIGGER_LOGON (no user id / delay set -> any user logon) | TaskTriggers::createTrigger FUN_005660e0 |
| Settings.DisallowStartIfOnBatteries | false | FUN_00564740 |
| Settings.StopIfGoingOnBatteries | false | FUN_00564a20 |
| Action | type 0 = TASK_ACTION_EXEC; Path = full path of the running EXE; Arguments = `/minimized` | FUN_00565000, setExe FUN_00565a00, setArguments FUN_00565920 |
| Registration | `RegisterTaskDefinition(name, def, 6 = TASK_CREATE_OR_UPDATE, ...)` with empty user/password | FUN_00562cd0 |

Other settings (ExecutionTimeLimit, MultipleInstances, ...) are left at Task Scheduler defaults -
only the two battery flags are changed.
- Disable: delete task `MacsFanControl` (FUN_00561c30) and also delete the legacy value
  `HKCU\...\Run\MacsFanControl`.
- Query (check box state): task `MacsFanControl` exists (FUN_00563620).
- Pre-Vista fallback: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, value `MacsFanControl`
  (REG_SZ) = `"<exe path>" /minimized`.

---------------------------------------------------------------------------------------------------
## 7. Other dialogs

All app dialogs derive from `QDialogEx` (FUN_0059e850) which clears the `?` title-bar button
(`setWindowFlags(flags & ~0x00010000)` = no WindowContextHelpButtonHint, FUN_005a6570) and applies
the dark title bar (section 9).

### 7.1 About (`QInfoDialog`, ctor FUN_0045ded0, setupUi FUN_0045f300)

- Title tr("AboutDialog/title") "About Macs Fan Control"; window icon = tray icon; setupUi size
  350 x 205, then `adjustSize()` + `setFixedSize()`.
- Layout: `verticalLayout_2` [ `horizontalLayout` [ `m_AboutIcon` QLabel (alignment 0x85) showing
  the app icon (`:/Resources/icon_128.png`, FUN_0045d6d0) as a 128x128 pixmap ;
  `verticalLayout` (spacing 6) [ spacer 20x40 ; `labelAppName` ; `labelCopyright` ; `labelVersion` ;
  `labelLicense` ; `labelSupport` ; spacer 20x40 ] ] ; `labelDisclamer` ; `buttonBox` ]. All text
  labels are centred (0x84); `labelDisclamer` is wordWrap, alignment 1 (left), no frame.
- `labelAppName` = `"<b>" + "Macs Fan Control" + " " + "1.5.21" + " Build %1".arg(build) + "</b>"`
  where build = FUN_00593dc0() (4th component of the EXE file version).
- `labelCopyright` = `"© 2013-%1 CrystalIDEA Software".arg(current year)` (string at 0x007598a8).
- `labelLicense`: Pro -> tr("AboutDialog/staticProVersion").arg(n) "Pro version, %1 computer(s)
  license"; not Pro -> the label is toggled with `setVisible` and the trial/free text
  (tr("TrialDialog/trialVersion") / tr("AboutDialog/staticFreeVersion")) is prepared. UNKNOWN:
  exactly which of `labelVersion` / `labelLicense` receives which of these strings (label
  pointers are lost in the decompilation); license-related, so a rewrite can show name+version
  and omit the licence line.
- `labelSupport` = `<a href='help'>%1</a>` with tr("AboutDialog/staticOnlineHelp") "Online help";
  `linkActivated` -> `OnLinkActivated` (FUN_0045ecc0) opens
  `https://crystalidea.com/macs-fan-control/support`.
- `labelDisclamer` = tr("AboutDialog/staticWarning") (the upper-case disclaimer).
- Button: QPushButton tr("Common/buttonClose") "Close", default + autoDefault, role 1 (RejectRole).

### 7.2 Save / rename preset (`QSaveEditProfileDialog`, ctor FUN_0046f820, setupUi FUN_004705d0)

- setupUi size 300 x 100, then `adjustSize()` + `setFixedSize()`; window icon = app icon set
  (FUN_0045db90); attribute 0x4C cleared.
- Layout: QVBoxLayout [ `labelTitle` QLabel centred (text = the window title) ; QHBoxLayout
  [ `labelProfileName` tr("Presets/labelPresetName") "Preset name:" ; `comboName` editable
  QComboBox (`sizeAdjustPolicy 1`, `minimumContentsLength 14`) ; `editName` QLineEdit
  (`maxLength 30`) ] ; `buttonBox` (OK / Cancel added in code, FUN_00470140) ].
- Two modes (which of comboName / editName is visible is decided in the ctor; INFERRED from the
  two code paths):
  - **Save** (no current name): title tr("Presets/savePresetTitle") "Save custom preset"; the
    editable combo is used, filled with the existing custom preset names (choose one to
    overwrite, or type a new name).
  - **Rename** (current name given): title tr("Presets/editPresetTitle") "Edit preset name"; the
    line edit is used, pre-filled with the current name.
- `textChanged` (FUN_00470c30) validates on every edit and enables/disables OK (member +0x1c):
  the name must be non-empty; in rename mode it must differ from the original name
  (FUN_00448e70 string compare), and it is then compared lower-cased against each existing
  preset name (case-insensitive duplicate check). PARTLY UNKNOWN: the polarity of the last two
  comparisons could not be read reliably from the decompilation - verify against the running
  app whether duplicates are rejected in save mode (where picking an existing name means
  "overwrite"). `accept` returns the name to
  `letUserSelectProfileName` (FUN_00464eb0), which returns "" on cancel.

### 7.3 "Fans disabled by hardware" (`messageFanControlLimited/*`)

Not present in the Windows build: none of the `messageFanControlLimited` keys is referenced by
any function (macOS-only, Apple-silicon behaviour). Nothing to implement.

### 7.4 Tech-info zip (`slotCreateTechInfoZip` FUN_00466b40, `TechZip::createZip` FUN_004b4f50)

1. The main window is shown (`showNormal`, `activateWindow`, `raise`, `processEvents`) so it can be
   captured.
2. Four temp files are written in `QDir::tempPath()` (extensions `txt`, `log`, `smc`, `png`):
   system info text, the log, an SMC dump, and a screenshot of the main window (failure of the
   screenshot callback is only logged: "screenshotFunc failed").
3. Zip created at `QStandardPaths::writableLocation(DesktopLocation)` with file name
   `"%1_tech_info.zip"` (%1 = INFERRED machine model identifier; argument not traced). Entries:
   `app_screen.png`, `app_info.txt`, `app_log.txt`, `smc_dump.txt`. Temp files are removed.
4. Success: QMessageBox with text tr("TrayMenu/itemTechInfoZip") "Create tech zip file...",
   informative text tr("techInfoZipMessage/message").arg(zip file name), an extra button
   tr("techInfoZipMessage/browseButton") "Show zip file" (role 0; reveals the file, FUN_00464cb0)
   plus the default OK.
   Failure: message box title "Create technical info", text "Unable to create tech_info.zip file."

### 7.5 Christmas

Only the snowflake watermark in the two tables, 24-31 December (1.7, FUN_00469b40).
`christmas_santa.png` is not referenced; the tray icon is not changed.

### 7.6 Rate-me (`QRateMeDialog`, FUN_0046ee90)

- At main-window construction: if `RateOnMacupdateNextDate` is not a valid date it is set to
  now + 7 days (FUN_0046f490).
- Checked each time the window is restored from the tray (`slotRestore`, FUN_00467ad0), only when
  the window was hidden: show if FUN_0046f6b0() (`RateOnMacupdateSkip == 0` and now is past
  `RateOnMacupdateNextDate`; the next date is then pushed 7 days ahead) and FUN_00438b10() is
  false (licence-state predicate, not analysed).
- QMessageBox: text tr("RateMeDialog/message") "Rate the app on MacUpdate"; informative text
  tr("RateMeDialog/informativeText").arg("Macs Fan Control"); icon pixmap = app icon 48x48;
  buttons tr("RateMeDialog/buttonRate") "Rate on MacUpdate" (role 0 AcceptRole) and
  tr("RateMeDialog/buttonNoThanks") "No, thanks" (role 1 RejectRole); check box
  tr("RateMeDialog/checkDontShowAgain") "Do not show this message again".
- Result: check box checked -> `RateOnMacupdateSkip = 1`; Rate clicked -> `slotRateApp` (opens
  `https://www.macupdate.com/app/mac/47386/macs-fan-control`, sets Skip = 1).

### 7.7 Dialog helper (`MenubarAppDialogHelper`, FUN_00469940 / FUN_00469700)

Preferences, About and upgrade dialogs are shown through a helper owned by the main window: it
clears attribute 0x4C, sets attribute 0x37 (WA_DeleteOnClose); if no helper dialog is active
(`DAT_0089f1fc == 0`): `setModal(parent != null)`, `show()`, and when parentless also
`activateWindow()` + `raise()`; if one is active it is closed first and the new one is shown when
the old one is destroyed (FUN_00469850). So only one of these dialogs is open at a time and they
are shown with `show`, not `exec`. The parent is the main window if it is visible, otherwise
no parent (so they work while the app is hidden in the tray).

### 7.8 Licence / trial / upgrade / update checker - trigger points only

| Feature | Triggered from |
|---|---|
| Trial dialog (`TrialDialog/*`, FUN_004c0c30) | start-up, FUN_00437360 step 2, when `pro-version` is unset |
| Licence check | app ctor FUN_00432320; `License` value |
| Upgrade dialog (`UpgradeToProDialog/*`, FUN_004c4c40) | `slotUpgradeToPro` FUN_00468670 <- tray "Upgrade to Pro version", presets menu "Available in PRO version", "Save..." when not Pro |
| Enter licence (`EnterLicenseDialog/*`) | `slotEnterLicense` FUN_00466f10 (from the upgrade/trial dialogs) |
| Licence warning / deactivate | `showLicenseWarning` FUN_00466a40, `deactivateLicense` FUN_004643d0 |
| Update checker | created in the main window ctor FUN_00468790 (feed `https://crystalidea.com/update/fancontrol.xml.gz`, download page `https://crystalidea.com/macs-fan-control/download`, first check scheduled by FUN_005a2e40(0, 24)); manual: `slotCheckForUpdates` FUN_00466b20 <- tray "Check for updates..."; preference `checkUpdates` |
| Title bar text "Trial version" / "Pro" | FUN_00468cb0 |

---------------------------------------------------------------------------------------------------
## 8. Language XML

- Files: `<exe dir>\languages\<Name>.xml`; root `<LanguageStrings version=".." Author="..">`.
  A copy of English is embedded as resource `:/build/languages/English.xml`.
- Parsing (FUN_005961d0, recursive): every element without child elements becomes one entry whose
  key is the `/`-joined path of element names below the root, e.g. `MainWindow/buttonPreferences`,
  `core/UpdateChecker/ReleasedOn`. The value is the element text decoded as UTF-8 with every
  literal two-character sequence `\n` (backslash, n) replaced by a real newline. XML entities
  (`&amp;`, `&lt;br/&gt;`) are decoded by the XML parser, so `statusWarning` contains a literal
  `<br/>` that Qt rich text renders as a line break.
- Loading (FUN_00595b90, argument = stored `InterfaceLanguage`):
  1. clear the table, parse the embedded English resource into it (current name = `English.xml`);
  2. requested name empty -> `QLocale::languageToString(QLocale::system().language()) + ".xml"`
     (e.g. `German.xml`); a name without `.xml` gets it appended;
  3. if the name is not `English.xml`: open `<exe dir>\languages\<name>`; if it opens and parses,
     its entries are added over the English ones and it becomes the current language; otherwise
     English stays.
  Because the English table is loaded first, **a key missing from a translation falls back to
  the embedded English text**.
- Lookup (FUN_005964a0, wrapped per class e.g. FUN_00464fd0, FUN_004c6060): returns the entry; if
  there is no table or the key is unknown everywhere, returns **the key string itself**.
- Placeholders: plain `QString::arg` - `%1`, `%2`, `%3` replaced in order of the `.arg()` calls
  (lowest-numbered marker first).
- Markdown-ish conversion, applied only where noted (no-fans overlay; FUN_005a59c0):
  1. newline -> `%newline%`; 2. regex `\[(.+?)\]\((.+?)\)` -> `<a href="\2">\1</a>`;
  3. regex `\*\*(.+?)\*\*` -> `<b>\1</b>`; 4. `%newline%` -> `<br>`. The result is shown in a QLabel
  with `openExternalLinks(true)`.
- Language list for the combo: all `*.xml` in the folder (6.2). Switching is live.
- Keys present in English.xml but unused on Windows (verified unreferenced): `buttonMinimizeMac`,
  `messageHelperTool*`, `messageFanControlNotAvailable*`, `itemUninstall`, `itemExitMac`,
  `tabTrayIconMac`, `tabDiagnostics`, `checkLog`, `browseLog`, `messageFanControlLimited/*`,
  `UninstallDialog/*`, `core/LetsMoveMac/*`; `checkDockIcon`, `staticFan`, `staticIcon`,
  `comboIconOption*`, `checkTwoLines` are set on hidden widgets only.

---------------------------------------------------------------------------------------------------
## 9. Dark mode, style and DPI

### 9.1 Style

The app sets the widget style explicitly (FUN_00594370), it does not rely on Qt's default
(`styles/qmodernwindowsstyle.dll` provides both `windowsvista` and `windows11`; the app asks the
factory for **"WindowsVista"**):

| Mode | Style | Palette | App style sheet |
|---|---|---|---|
| Light | `QStyleFactory::create("WindowsVista")` | the palette captured at start-up (`QGuiApplication::palette()`) | the style sheet captured at start-up (empty) |
| Dark | `QStyleFactory::create("Fusion")` | custom, below | `QToolTip { color: #ffffff; background-color: #2a82da; border: 1px solid white; }` |

Dark palette (QPalette::ColorRole raw value -> colour):
Window(10) #2D2D2D; WindowText(0) white; Base(9) #121212; AlternateBase(16) #2D2D2D;
ToolTipBase(18) white; ToolTipText(19) white; Text(6) white; Disabled Text #7F7F7F;
Button(1) #2D2D2D; ButtonText(8) white; Disabled ButtonText #7F7F7F; BrightText(7) red;
Link(14) #2A82DA; Highlight(12) #2A82DA; HighlightedText(13) black; Disabled HighlightedText
#7F7F7F.

### 9.2 Dark-mode detection and switching

- Dark = `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme == 0`
  (FUN_0059eab0).
- Support is gated by the undocumented uxtheme dark-mode API being available (`WinDarkMode`
  singleton FUN_00627780 resolves `AllowDarkModeForApp`, `AllowDarkModeForWindow`,
  `ShouldAppsUseDarkMode`, `IsDarkModeAllowedForWindow`, `IsDarkModeAllowedForApp`,
  `ShouldSystemUseDarkMode`; predicate FUN_00627cc0). If unavailable nothing is changed.
- `FUN_00593fe0(app, watch = true)` at start-up: sets app properties `DarkModeSupport` and
  `DarkModeWatchChanges`, applies the mode once, and starts `System::WindowsDarkModeWatcher`
  (background task; emits `darkModeChanged` through `System::PowerSource`-style notifier
  FUN_0059ee40) -> FUN_005942f0 re-reads the registry value and calls FUN_00594370 again: the
  switch is **live**, no restart.
- Per window (`MacWindowWindows` mix-in FUN_005a6300 -> FUN_005a6320, used by the main window and
  every QDialogEx): on creation, and again on every dark-mode change (FUN_005a6500), calls the
  WinDarkMode helper with `(winId(), isDark)` (FUN_00627e10) - INFERRED: `AllowDarkModeForWindow`
  + DWM immersive-dark-mode title bar attribute; the exact DWM attribute id was not read.
- App-drawn exceptions that react to dark mode: only the "no fans" overlay label colour
  (#293956 light / #ffffff dark, evaluated when the label is rebuilt, FUN_004b6fd0). The orange
  #FF7E00 RPM highlight, the grey (128,128,128) stale-sensor colour and the dark-blue tray bitmap
  are the same in both modes.
- `System::WindowsPowerSourceWatcher` (`powerSourceChanged`) also exists (FUN_005a6b70); it feeds
  the fan-control core, there is no UI for it.

### 9.3 DPI

- Manifest: PerMonitorV2. Qt 6.8.3 does all scaling; the app only sets the rounding policy to
  Round (4.1) so every logical pixel value in this document is multiplied by an integer factor
  (1 at 100%/125%, 2 at 150%-250%, ...).
- App-side DPI code: (a) the tray bitmap is sized `16 * primaryScreen.devicePixelRatio` with font
  `8pt * dpr` (2.1); (b) icons are QIcons with `@2x` variants (16/32 px; `_big` 32/64 px) selected
  by Qt; (c) FUN_0059fb40(16, 0) scales the size of placeholder pixmaps. Nothing else is
  DPI-aware in app code; there is no WM_DPICHANGED handling of its own.

---------------------------------------------------------------------------------------------------
## 10. Summary of UNKNOWN / INFERRED items

| # | Item | What was tried |
|---|---|---|
| 1 | Worker polling period (UI refresh rate) | UI has no timer; signals come from FanControl.cpp threads (not in scope read) |
| 2 | Window-title %3 and zip-name %1 (assumed machine model id) | field `app+0x48 -> +4`, also logged by printSysInfo; origin in sysinfo_win.cpp not read |
| 3 | Order/content of the fan-dialog sensor combo | list supplied by a callback from FUN_00467da0; insertion loop read, source list not traced |
| 4 | Role of the OK button in the fan dialog; whether `labelTitle` is hidden | arguments not present in decompilation |
| 5 | Preferences dialog width formula | FUN_005bc8f0 sums toolbar button widths; constants lost in decompilation |
| 6 | About dialog: which label gets the licence/trial text | label pointers lost; licence-related |
| 7 | Save-preset name validation rule | FUN_00470c30 not read |
| 8 | Log sink mode (append/truncate/rotation) | sink factory FUN_005536e0 not traced into spdlog |
| 9 | `AppFirstRun` registered default constant | push not visible; behaviour "non-zero when absent" is certain |
| 10 | DWM attribute used for dark title bars | FUN_00627e10 calls through a function pointer table |
| 11 | FUN_00438b10 predicate gating the rate-me box | licence-state code, intentionally not analysed |
| 12 | Unreferenced auto-size function FUN_00462d90 | identical logic is inlined in FUN_00467390; standalone copy has no callers |
