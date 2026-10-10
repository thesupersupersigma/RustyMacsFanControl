//! The main window: presets bar, fans table, sensors table and the bottom buttons.

use std::cell::{Cell, RefCell};
use std::ptr::{null, null_mut};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateSolidBrush, DeleteObject, DrawTextW, FillRect, GetSysColorBrush, InvalidateRect,
    SelectObject, SetBkMode, SetTextColor, COLOR_BTNFACE, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX,
    DT_SINGLELINE, DT_VCENTER, HDC, HFONT, TRANSPARENT,
};
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows_sys::Win32::UI::Controls::{
    CloseThemeData, DrawThemeBackground, ImageList_Add, ImageList_Create, ImageList_Draw,
    InitCommonControlsEx, OpenThemeData, SetWindowTheme, CDDS_ITEMPREPAINT, CDDS_PREPAINT,
    CDDS_SUBITEM, CDRF_DODEFAULT, CDRF_NEWFONT, CDRF_NOTIFYITEMDRAW, CDRF_NOTIFYSUBITEMDRAW,
    DRAWITEMSTRUCT, HIMAGELIST, ICC_LINK_CLASS, ICC_LISTVIEW_CLASSES, ICC_STANDARD_CLASSES,
    ICC_UPDOWN_CLASS, ILC_COLOR32, ILD_TRANSPARENT, INITCOMMONCONTROLSEX, LVCFMT_CENTER,
    LVCFMT_LEFT, LVCF_FMT, LVCF_TEXT, LVCF_WIDTH, LVCOLUMNW, LVIF_IMAGE, LVIF_TEXT, LVIR_BOUNDS,
    LVITEMW, LVM_DELETEALLITEMS, LVM_GETITEMCOUNT, LVM_GETSUBITEMRECT,
    LVM_INSERTCOLUMNW, LVM_INSERTITEMW, LVM_SETCOLUMNW, LVM_SETCOLUMNWIDTH,
    LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETIMAGELIST, LVM_SETITEMTEXTW, LVSIL_SMALL, LVS_EX_DOUBLEBUFFER, LVS_EX_FULLROWSELECT, LVS_NOSORTHEADER,
    LVS_OWNERDRAWFIXED, LVS_REPORT, LVS_SHOWSELALWAYS, LVS_SINGLESEL, MEASUREITEMSTRUCT,
    NMHDR, NMITEMACTIVATE, NMLINK, NMLVCUSTOMDRAW, NM_CLICK, NM_CUSTOMDRAW,
    NM_RETURN, ODS_SELECTED, ODT_LISTVIEW, TP_BUTTON, TS_CHECKED,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, ReleaseCapture, SetCapture};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMessageW, GetSystemMetrics, GetWindowRect,
    IsDialogMessageW, IsWindowVisible, LoadCursorW, LoadIconW, MessageBoxW, MoveWindow,
    PostMessageW, PostQuitMessage, RegisterClassExW, SendMessageW, SetCursor,
    SetForegroundWindow, SetMenuDefaultItem, SetWindowPos, ShowWindow, TrackPopupMenu,
    TranslateMessage, BM_SETIMAGE, BS_ICON, BS_PUSHBUTTON, CW_USEDEFAULT,
    ENDSESSION_LOGOFF, HMENU, HTCLIENT, IDC_ARROW, IDC_SIZEWE, IMAGE_ICON, MB_ICONINFORMATION,
    MB_OK, MF_CHECKED, MF_DISABLED, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING, MINMAXINFO,
    MSG, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND, SM_CXSCREEN,
    SM_CYSCREEN, SWP_NOMOVE, SWP_NOZORDER, SW_HIDE,
    SW_SHOWNORMAL, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP, WM_CLOSE, WM_COMMAND, WM_DESTROY,
    WM_DRAWITEM, WM_ENDSESSION, WM_GETMINMAXINFO, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MEASUREITEM, WM_MOUSEMOVE, WM_NOTIFY, WM_POWERBROADCAST, WM_QUERYENDSESSION,
    WM_SETCURSOR, WM_SIZE, WNDCLASSEXW, WS_CLIPCHILDREN, WS_EX_CLIENTEDGE,
    WS_OVERLAPPEDWINDOW, WS_TABSTOP,
};

use super::text::{self, mode_text, temperature_text};
use super::tray::Tray;
use super::win::{self, client_rect, control, px, rgb, wide, Fonts, SS_CENTER, SS_CENTERIMAGE};
use crate::engine::{Engine, Host, NO_RPM};
use crate::fan::mode::Mode;
use crate::fan::profile::{Current, Predefined, Profile};
use crate::lang::Language;
use crate::sensors::defs::Kind;
use crate::settings::Store;

const CLASS_NAME: &str = "RustyMacsFanControlMainWindow";
/// Posted by the refresh thread after each pass.
pub const WM_REFRESHED: u32 = WM_APP + 1;
pub const WM_TRAY: u32 = WM_APP + 2;
/// Posted when a second instance starts: bring the window back.
pub const WM_RESTORE: u32 = WM_APP + 3;

const ID_PRESET_BUTTON: i32 = 101;
const ID_ACTION_BUTTON: i32 = 102;
const ID_FANS: i32 = 103;
const ID_SENSORS: i32 = 104;
const ID_MINIMIZE: i32 = 105;
const ID_PREFERENCES: i32 = 106;
const ID_ABOUT: i32 = 107;
const ID_NO_FANS: i32 = 108;
const MENU_BASE: usize = 1000;

const ROW_ALTERNATE: u32 = rgb(245, 245, 245);
const ROW_BASE: u32 = rgb(255, 255, 255);
const ROW_SELECTED: u32 = rgb(204, 232, 255);
const ORANGE: u32 = rgb(0xFF, 0x7E, 0x00);
const STALE_GREY: u32 = rgb(128, 128, 128);
const STALE_AFTER: Duration = Duration::from_secs(5);

const FAN_ROW_HEIGHT: i32 = 36;
const MARGIN: i32 = 11;
const SPACING: i32 = 6;
const BUTTON_HEIGHT: i32 = 23;
const DEFAULT_RIGHT_PANE: i32 = 240;
const MIN_RIGHT_PANE: i32 = 200;
const MIN_FANS_WIDTH: i32 = 500;

/// What a menu item does once chosen.
#[derive(Clone, Debug)]
pub enum Action {
    Show,
    SetPredefined(Predefined),
    SetCustom(usize),
    RenameCustom(usize),
    DeleteCustom(usize),
    NewPreset,
    SaveUnsaved,
    DiscardUnsaved,
    FanControl(usize),
    FanReset(usize),
    Preferences,
    About,
    TechZip,
    RateApp,
    Twitter,
    Website,
    Exit,
}

#[derive(Clone)]
struct FanRow {
    name: String,
    current: u16,
    min: u16,
    max: u16,
    mode: Mode,
    mode_text: String,
}

#[derive(Clone)]
enum SensorRow {
    Separator,
    Sensor { name: String, kind: Kind, value: f64, last_update: Option<Instant> },
}

pub struct MainWindow {
    pub hwnd: Cell<HWND>,
    pub engine: RefCell<Engine>,
    pub lang: RefCell<Language>,
    pub fonts: Fonts,
    pub tray: RefCell<Option<Tray>>,
    preset_label: Cell<HWND>,
    preset_button: Cell<HWND>,
    action_button: Cell<HWND>,
    fans: Cell<HWND>,
    no_fans: Cell<HWND>,
    sensors_label: Cell<HWND>,
    sensors: Cell<HWND>,
    buttons: [Cell<HWND>; 3],
    fan_icons: Cell<HIMAGELIST>,
    fan_rows: RefCell<Vec<FanRow>>,
    sensor_rows: RefCell<Vec<SensorRow>>,
    /// Width of the right pane in device pixels.
    right_width: Cell<i32>,
    dragging_splitter: Cell<bool>,
    quitting: Cell<bool>,
}

thread_local! {
    static WINDOW: RefCell<Option<Rc<MainWindow>>> = const { RefCell::new(None) };
}

fn window() -> Option<Rc<MainWindow>> {
    WINDOW.with(|w| w.borrow().clone())
}

/// Lets the refresh thread ask about the window and wake the UI.
struct UiHost(isize);

impl Host for UiHost {
    fn window_visible(&self) -> bool {
        unsafe { IsWindowVisible(self.0 as HWND) != 0 }
    }

    fn on_battery(&self) -> bool {
        let mut status: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
        unsafe { GetSystemPowerStatus(&mut status) != 0 && status.ACLineStatus == 0 }
    }

    fn refreshed(&self) {
        unsafe { PostMessageW(self.0 as HWND, WM_REFRESHED, 0, 0) };
    }
}

fn sensor_icon_name(kind: Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::Optical => "disc",
        Kind::Cpu => "processor",
        Kind::Hdd => "drive",
        Kind::Ssd => "drive_ssd",
        Kind::Nvme => "drive_nvme",
        Kind::External => "drive_external",
        Kind::Airport => "airport",
        Kind::Psu => "power-supply",
        Kind::Gpu => "gpu",
        Kind::Other => return None,
    })
}

const SENSOR_KINDS: [Kind; 9] = [
    Kind::Optical,
    Kind::Cpu,
    Kind::Hdd,
    Kind::Ssd,
    Kind::Nvme,
    Kind::External,
    Kind::Airport,
    Kind::Psu,
    Kind::Gpu,
];

fn image_list(size: i32, names: &[&str]) -> HIMAGELIST {
    unsafe {
        let list = ImageList_Create(size, size, ILC_COLOR32, names.len() as i32, 0);
        for name in names {
            if let Some(image) = win::load_image(name) {
                let bitmap = image.to_bitmap();
                ImageList_Add(list, bitmap, null_mut());
                DeleteObject(bitmap);
            }
        }
        list
    }
}

fn send(hwnd: HWND, message: u32, wparam: usize, lparam: isize) -> isize {
    unsafe { SendMessageW(hwnd, message, wparam, lparam) }
}

fn add_column(list: HWND, index: i32, text: &str, format: i32) {
    let mut text = wide(text);
    let mut column: LVCOLUMNW = unsafe { std::mem::zeroed() };
    column.mask = LVCF_TEXT | LVCF_FMT | LVCF_WIDTH;
    column.fmt = format;
    column.cx = px(100);
    column.pszText = text.as_mut_ptr();
    send(list, LVM_INSERTCOLUMNW, index as usize, &column as *const _ as isize);
}

fn set_column_text(list: HWND, index: i32, text: &str) {
    let mut text = wide(text);
    let mut column: LVCOLUMNW = unsafe { std::mem::zeroed() };
    column.mask = LVCF_TEXT;
    column.pszText = text.as_mut_ptr();
    send(list, LVM_SETCOLUMNW, index as usize, &column as *const _ as isize);
}

fn set_item_text(list: HWND, row: i32, column: i32, text: &str) {
    let mut text = wide(text);
    let mut item: LVITEMW = unsafe { std::mem::zeroed() };
    item.iSubItem = column;
    item.pszText = text.as_mut_ptr();
    send(list, LVM_SETITEMTEXTW, row as usize, &item as *const _ as isize);
}

fn subitem_rect(list: HWND, row: i32, column: i32) -> RECT {
    // The message reads the column from `top` and the bounds kind from `left`.
    let mut rect = RECT { left: LVIR_BOUNDS as i32, top: column, right: 0, bottom: 0 };
    send(list, LVM_GETSUBITEMRECT, row as usize, &mut rect as *mut _ as isize);
    rect
}

fn draw_text(dc: HDC, font: HFONT, text: &str, rect: &mut RECT, color: u32, format: u32) {
    let units: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let old = SelectObject(dc, font);
        SetTextColor(dc, color);
        DrawTextW(dc, units.as_ptr(), units.len() as i32, rect, format | DT_NOPREFIX);
        SelectObject(dc, old);
    }
}

/// The Auto / Custom segments inside a control cell.
fn segment_rects(cell: &RECT) -> (RECT, RECT) {
    let available = (cell.right - cell.left) - px(40);
    let first = (available / 2).min(px(100)).max(0);
    let second = (available - first).min(px(300)).max(0);
    let left = cell.left + ((cell.right - cell.left) - first - second) / 2;
    let height = px(24);
    let top = cell.top + ((cell.bottom - cell.top) - height) / 2;
    (
        RECT { left, top, right: left + first, bottom: top + height },
        RECT { left: left + first, top, right: left + first + second, bottom: top + height },
    )
}

fn contains(rect: &RECT, point: POINT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

impl MainWindow {
    fn store(&self) -> Arc<dyn Store + Send + Sync> {
        self.engine.borrow().store_arc()
    }

    fn tr(&self, path: &str) -> String {
        self.lang.borrow().get(path).to_string()
    }

    fn create_children(&self, hwnd: HWND) {
        let font = self.fonts.normal;
        self.preset_label.set(control(hwnd, "STATIC", "", SS_CENTERIMAGE, 0, 0, font));
        self.preset_button.set(control(
            hwnd,
            "BUTTON",
            "",
            BS_PUSHBUTTON as u32 | WS_TABSTOP,
            0,
            ID_PRESET_BUTTON,
            font,
        ));
        let action = control(
            hwnd,
            "BUTTON",
            "",
            BS_PUSHBUTTON as u32 | BS_ICON as u32 | WS_TABSTOP,
            0,
            ID_ACTION_BUTTON,
            font,
        );
        if let Some(image) = win::load_image("profiles_action") {
            let bitmap = image.to_bitmap();
            let icon = win::icon_from_bitmap(bitmap, image.width, image.height);
            unsafe { DeleteObject(bitmap) };
            send(action, BM_SETIMAGE, IMAGE_ICON as usize, icon as isize);
        }
        self.action_button.set(action);

        let fans = control(
            hwnd,
            "SysListView32",
            "",
            LVS_REPORT | LVS_SINGLESEL | LVS_NOSORTHEADER | LVS_OWNERDRAWFIXED
                | LVS_SHOWSELALWAYS
                | WS_TABSTOP,
            WS_EX_CLIENTEDGE,
            ID_FANS,
            font,
        );
        send(
            fans,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            0,
            (LVS_EX_DOUBLEBUFFER | LVS_EX_FULLROWSELECT) as isize,
        );
        for (index, format) in [LVCFMT_LEFT, LVCFMT_CENTER, LVCFMT_CENTER].into_iter().enumerate() {
            add_column(fans, index as i32, "", format);
        }
        self.fans.set(fans);
        self.fan_icons.set(image_list(
            px(32),
            &["disc_big", "drive_big", "processor_big", "fan_big"],
        ));

        self.no_fans.set(control(hwnd, "SysLink", "", SS_CENTER, 0, ID_NO_FANS, self.fonts.bold));
        unsafe { ShowWindow(self.no_fans.get(), SW_HIDE) };

        self.sensors_label
            .set(control(hwnd, "STATIC", "", SS_CENTER | SS_CENTERIMAGE, 0, 0, font));
        let sensors = control(
            hwnd,
            "SysListView32",
            "",
            LVS_REPORT | LVS_SINGLESEL | LVS_NOSORTHEADER | LVS_SHOWSELALWAYS | WS_TABSTOP,
            WS_EX_CLIENTEDGE,
            ID_SENSORS,
            font,
        );
        send(
            sensors,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            0,
            (LVS_EX_DOUBLEBUFFER | LVS_EX_FULLROWSELECT) as isize,
        );
        unsafe { SetWindowTheme(sensors, wide("Explorer").as_ptr(), null()) };
        add_column(sensors, 0, "", LVCFMT_LEFT);
        add_column(sensors, 1, "", LVCFMT_CENTER);
        let names: Vec<&str> = SENSOR_KINDS.iter().filter_map(|&k| sensor_icon_name(k)).collect();
        send(sensors, LVM_SETIMAGELIST, LVSIL_SMALL as usize, image_list(px(16), &names) as isize);
        self.sensors.set(sensors);

        for (slot, id) in self.buttons.iter().zip([ID_MINIMIZE, ID_PREFERENCES, ID_ABOUT]) {
            slot.set(control(hwnd, "BUTTON", "", BS_PUSHBUTTON as u32 | WS_TABSTOP, 0, id, font));
        }
    }

    /// Applies the current language and unit to every static text.
    pub fn retranslate(&self) {
        let store = self.store();
        win::set_text(self.preset_label.get(), &self.tr("Presets/activePresetLabel"));
        win::set_text(self.sensors_label.get(), &self.tr("MainWindow/staticSensorsListHeader"));
        let fans = self.fans.get();
        set_column_text(fans, 0, &self.tr("MainWindow/listColumnFan"));
        set_column_text(fans, 1, &self.tr("MainWindow/listColumnRPM"));
        set_column_text(fans, 2, &self.tr("MainWindow/listColumnControl"));
        let sensors = self.sensors.get();
        set_column_text(sensors, 0, &self.tr("MainWindow/listColumnSensor"));
        set_column_text(
            sensors,
            1,
            &format!("{} {}", self.tr("MainWindow/listColumnSensorValue"), text::unit(&*store)),
        );
        for (slot, path) in self.buttons.iter().zip([
            "MainWindow/buttonMinimize",
            "MainWindow/buttonPreferences",
            "MainWindow/buttonAbout",
        ]) {
            win::set_text(slot.get(), &self.tr(path));
        }
        let fanless = self.engine.borrow().core().model.is_fanless();
        let message = if fanless {
            "MainWindow/messageNoFansAvailable"
        } else {
            "MainWindow/messageNoFansDetected"
        };
        win::set_text(self.no_fans.get(), &text::markdown_links(&self.tr(message)));
        self.refresh_presets_bar();
        self.layout();
        self.reload();
    }

    fn splitter_rect(&self) -> RECT {
        let client = client_rect(self.hwnd.get());
        let right = client.right - px(MARGIN) - self.right_width.get();
        RECT {
            left: right - px(SPACING),
            top: px(MARGIN),
            right,
            bottom: client.bottom - px(MARGIN) - px(BUTTON_HEIGHT) - px(SPACING),
        }
    }

    fn layout(&self) {
        let hwnd = self.hwnd.get();
        let client = client_rect(hwnd);
        if client.right == 0 {
            return;
        }
        let place = |child: HWND, x: i32, y: i32, w: i32, h: i32| unsafe {
            MoveWindow(child, x, y, w.max(0), h.max(0), 1);
        };
        let (margin, spacing, button_height) = (px(MARGIN), px(SPACING), px(BUTTON_HEIGHT));
        let font = self.fonts.normal;

        // Bottom buttons, right aligned in order.
        let mut x = client.right - margin;
        let button_y = client.bottom - margin - button_height;
        for slot in self.buttons.iter().rev() {
            let width = (win::text_width(hwnd, font, &win::get_text(slot.get())) + px(20)).max(px(75));
            x -= width;
            place(slot.get(), x, button_y, width, button_height);
            x -= spacing;
        }

        let panes_bottom = button_y - spacing;
        let total = client.right - 2 * margin;
        let max_right = (total - spacing - px(MIN_FANS_WIDTH)).max(px(MIN_RIGHT_PANE));
        let right_width = self.right_width.get().clamp(px(MIN_RIGHT_PANE), max_right);
        self.right_width.set(right_width);
        let left_width = total - spacing - right_width;
        let right_x = margin + left_width + spacing;

        // Presets bar: label, preset button and action button centred as a group.
        let label_width = win::text_width(hwnd, font, &win::get_text(self.preset_label.get()));
        let preset_width = self.preset_button_width();
        let action_width = px(32);
        let group = label_width + spacing + preset_width + spacing + action_width;
        let mut bar_x = margin + (left_width - group) / 2;
        place(self.preset_label.get(), bar_x, margin, label_width, button_height);
        bar_x += label_width + spacing;
        place(self.preset_button.get(), bar_x, margin, preset_width, button_height);
        bar_x += preset_width + spacing;
        place(self.action_button.get(), bar_x, margin, action_width, button_height);

        let fans_y = margin + button_height + spacing;
        let fans_height = panes_bottom - fans_y;
        place(self.fans.get(), margin, fans_y, left_width, fans_height);
        place(self.no_fans.get(), margin + px(4), fans_y + fans_height / 2 - px(20), left_width - px(8), px(40));

        let label_height = px(16);
        place(self.sensors_label.get(), right_x, margin, right_width, label_height);
        let sensors_y = margin + label_height + spacing;
        place(self.sensors.get(), right_x, sensors_y, right_width, panes_bottom - sensors_y);
        self.size_columns();
    }

    fn preset_button_width(&self) -> i32 {
        let hwnd = self.hwnd.get();
        let engine = self.engine.borrow();
        let core = engine.core();
        let mut names = vec![
            self.tr("Presets/presetAutomatic"),
            self.tr("Presets/presetFullBlast"),
            self.tr("Presets/presetCurrentUnsaved"),
            self.tr("Presets/presetCreateNew"),
        ];
        names.extend(core.profiles.custom.iter().map(|profile| profile.name.clone()));
        names
            .iter()
            .map(|name| win::text_width(hwnd, self.fonts.normal, name))
            .max()
            .unwrap_or(0)
            + px(30)
    }

    fn size_columns(&self) {
        let hwnd = self.hwnd.get();
        let fans = self.fans.get();
        let width = client_rect(fans).right;
        let rows = self.fan_rows.borrow();
        let longest = rows
            .iter()
            .map(|row| win::text_width(hwnd, self.fonts.normal, &row.name))
            .max()
            .unwrap_or(0)
            .max(win::text_width(hwnd, self.fonts.normal, &self.tr("MainWindow/listColumnFan")));
        let name_width = px(32) + px(18) + longest;
        let rpm_width =
            (win::text_width(hwnd, self.fonts.normal, &self.tr("MainWindow/listColumnRPM")) + px(30))
                .max(px(100));
        send(fans, LVM_SETCOLUMNWIDTH, 0, name_width as isize);
        send(fans, LVM_SETCOLUMNWIDTH, 1, rpm_width as isize);
        send(fans, LVM_SETCOLUMNWIDTH, 2, (width - name_width - rpm_width).max(px(60)) as isize);

        let sensors = self.sensors.get();
        let header = format!(
            "{} {}",
            self.tr("MainWindow/listColumnSensorValue"),
            text::unit(&*self.store())
        );
        let value_width = win::text_width(hwnd, self.fonts.normal, &header) + px(20);
        let width = client_rect(sensors).right;
        send(sensors, LVM_SETCOLUMNWIDTH, 0, (width - value_width).max(px(40)) as isize);
        send(sensors, LVM_SETCOLUMNWIDTH, 1, value_width as isize);
    }

    /// Rebuilds both tables from the engine state.
    pub fn reload(&self) {
        self.snapshot();
        let fans = self.fans.get();
        let count = self.fan_rows.borrow().len();
        if send(fans, LVM_GETITEMCOUNT, 0, 0) as usize != count {
            send(fans, LVM_DELETEALLITEMS, 0, 0);
            for row in 0..count {
                let mut item: LVITEMW = unsafe { std::mem::zeroed() };
                item.iItem = row as i32;
                send(fans, LVM_INSERTITEMW, 0, &item as *const _ as isize);
            }
        }
        unsafe {
            ShowWindow(self.no_fans.get(), if count == 0 { SW_SHOWNORMAL } else { SW_HIDE });
            InvalidateRect(fans, null(), 0);
        }

        let sensors = self.sensors.get();
        send(sensors, LVM_DELETEALLITEMS, 0, 0);
        let separator = self.tr("MainWindow/listSeparatorDrives");
        for (row, sensor) in self.sensor_rows.borrow().iter().enumerate() {
            let (name, image) = match sensor {
                SensorRow::Separator => (separator.clone(), -2),
                SensorRow::Sensor { name, kind, .. } => (
                    name.clone(),
                    sensor_icon_name(*kind)
                        .and_then(|_| SENSOR_KINDS.iter().position(|k| k == kind))
                        .map_or(-2, |i| i as i32),
                ),
            };
            let mut name = wide(&name);
            let mut item: LVITEMW = unsafe { std::mem::zeroed() };
            item.mask = LVIF_TEXT | LVIF_IMAGE;
            item.iItem = row as i32;
            item.pszText = name.as_mut_ptr();
            item.iImage = image;
            send(sensors, LVM_INSERTITEMW, 0, &item as *const _ as isize);
        }
        self.size_columns();
        self.update_values();
    }

    /// Copies what the tables show out of the engine, so painting never takes its lock.
    fn snapshot(&self) {
        let engine = self.engine.borrow();
        let core = engine.core();
        let lang = self.lang.borrow();
        let profile = core.profiles.current_profile();
        let sensor_name = |key: &str| {
            core.sensors
                .find(key)
                .map_or(String::new(), |index| core.sensors.sensors()[index].name.clone())
        };
        *self.fan_rows.borrow_mut() = core
            .fans
            .iter()
            .enumerate()
            .map(|(index, fan)| {
                let mode = profile.modes.get(index).cloned().unwrap_or(Mode::Invalid);
                FanRow {
                    name: fan.name.clone(),
                    current: fan.current,
                    min: fan.min,
                    max: fan.max,
                    mode_text: mode_text(&lang, &mode, false, &sensor_name),
                    mode,
                }
            })
            .collect();

        let mut rows = Vec::new();
        let mut separated = false;
        for index in core.sensors.visible() {
            let sensor = &core.sensors.sensors()[index];
            if sensor.kind.is_disk() && !separated {
                rows.push(SensorRow::Separator);
                separated = true;
            }
            rows.push(SensorRow::Sensor {
                name: sensor.name.clone(),
                kind: sensor.kind,
                value: sensor.value,
                last_update: sensor.last_update,
            });
        }
        *self.sensor_rows.borrow_mut() = rows;
    }

    /// Refreshes the numbers after a refresh pass without rebuilding the rows.
    fn update_values(&self) {
        let rows_before = self.sensor_rows.borrow().len();
        self.snapshot();
        if self.sensor_rows.borrow().len() != rows_before {
            self.reload();
            return;
        }
        let store = self.store();
        let sensors = self.sensors.get();
        for (row, sensor) in self.sensor_rows.borrow().iter().enumerate() {
            if let SensorRow::Sensor { value, .. } = sensor {
                set_item_text(sensors, row as i32, 1, &temperature_text(&*store, *value));
            }
        }
        unsafe { InvalidateRect(self.fans.get(), null(), 0) };
        if let Some(tray) = self.tray.borrow_mut().as_mut() {
            tray.update(self);
        }
    }

    pub fn fan_tooltips(&self) -> Vec<(String, u16)> {
        self.fan_rows.borrow().iter().map(|row| (row.name.clone(), row.current)).collect()
    }

    fn refresh_presets_bar(&self) {
        let (name, custom_or_unsaved, ready) = {
            let engine = self.engine.borrow();
            let core = engine.core();
            let name = match core.profiles.current() {
                Current::Predefined(Predefined::Automatic) => self.tr("Presets/presetAutomatic"),
                Current::Predefined(Predefined::FullBlast) => self.tr("Presets/presetFullBlast"),
                Current::Custom(index) => core.profiles.custom[*index].name.clone(),
                Current::Unsaved(_) => self.tr("Presets/presetCurrentUnsaved"),
            };
            let editable = !matches!(core.profiles.current(), Current::Predefined(_));
            (name, editable, !core.fans.is_empty())
        };
        win::set_text(self.preset_button.get(), &format!("{name}  \u{25BE}"));
        unsafe {
            EnableWindow(self.preset_button.get(), ready as i32);
            EnableWindow(self.action_button.get(), (ready && custom_or_unsaved) as i32);
        }
    }

    /// Called whenever the active preset or a fan's mode changed.
    pub fn profiles_changed(&self) {
        self.refresh_presets_bar();
        self.layout();
        self.update_values();
    }

    fn draw_fan_row(&self, draw: &DRAWITEMSTRUCT) {
        let rows = self.fan_rows.borrow();
        let Some(row) = rows.get(draw.itemID as usize) else {
            return;
        };
        let dc = draw.hDC;
        let list = self.fans.get();
        let selected = draw.itemState & ODS_SELECTED != 0;
        let background = if selected {
            ROW_SELECTED
        } else if draw.itemID % 2 == 1 {
            ROW_ALTERNATE
        } else {
            ROW_BASE
        };
        unsafe {
            let brush = CreateSolidBrush(background);
            FillRect(dc, &draw.rcItem, brush);
            DeleteObject(brush);
            SetBkMode(dc, TRANSPARENT as i32);
        }
        let black = rgb(0, 0, 0);
        let centre = DT_SINGLELINE | DT_VCENTER;

        // Fan icon and name.
        let name_cell = subitem_rect(list, draw.itemID as i32, 0);
        let rpm_cell = subitem_rect(list, draw.itemID as i32, 1);
        let icon = match row.name.as_str() {
            "ODD" => 0,
            "HDD" => 1,
            "CPU" => 2,
            _ => 3,
        };
        let icon_size = px(32);
        unsafe {
            ImageList_Draw(
                self.fan_icons.get(),
                icon,
                dc,
                draw.rcItem.left + px(4),
                draw.rcItem.top + (px(FAN_ROW_HEIGHT) - icon_size) / 2,
                ILD_TRANSPARENT,
            );
        }
        let mut name_rect = RECT {
            left: draw.rcItem.left + px(4) + icon_size + px(6),
            right: rpm_cell.left,
            ..name_cell
        };
        draw_text(dc, self.fonts.normal, &row.name, &mut name_rect, black, centre | DT_LEFT | DT_END_ELLIPSIS);

        // "min — current — max", the current speed bold and larger.
        let show = |rpm: u16| if rpm == NO_RPM { "-".to_string() } else { rpm.to_string() };
        let limits_colour = if row.min == row.max { ORANGE } else { black };
        let parts = [
            (format!("{} \u{2014} ", show(row.min)), self.fonts.normal, limits_colour),
            (show(row.current), self.fonts.big_bold, if row.current == 0 { ORANGE } else { black }),
            (format!(" \u{2014} {}", show(row.max)), self.fonts.normal, limits_colour),
        ];
        let widths: Vec<i32> =
            parts.iter().map(|(text, font, _)| win::text_width_dc(dc, *font, text)).collect();
        let mut x = rpm_cell.left + ((rpm_cell.right - rpm_cell.left) - widths.iter().sum::<i32>()) / 2;
        for ((text, font, colour), width) in parts.iter().zip(&widths) {
            let mut rect = RECT { left: x, right: x + width, ..rpm_cell };
            draw_text(dc, *font, text, &mut rect, *colour, centre | DT_LEFT);
            x += width;
        }

        // Auto / Custom segments; the pressed one shows the active mode.
        let control_cell = subitem_rect(list, draw.itemID as i32, 2);
        let (auto_rect, custom_rect) = segment_rects(&control_cell);
        let automatic = row.mode == Mode::Automatic;
        let custom_label = if automatic {
            self.tr("MainWindow/buttonFanControlCustom")
        } else {
            row.mode_text.clone()
        };
        let theme = unsafe { OpenThemeData(list, wide("TOOLBAR").as_ptr()) };
        let segments = [
            (auto_rect, self.tr("MainWindow/fanControlAutomatic"), automatic),
            (custom_rect, custom_label, !automatic),
        ];
        for (rect, label, checked) in segments {
            if checked && theme != 0 {
                unsafe { DrawThemeBackground(theme, dc, TP_BUTTON, TS_CHECKED, &rect, null()) };
            }
            let mut text_rect = RECT { left: rect.left + px(4), right: rect.right - px(4), ..rect };
            draw_text(dc, self.fonts.normal, &label, &mut text_rect, black, centre | DT_CENTER | DT_END_ELLIPSIS);
        }
        if theme != 0 {
            unsafe { CloseThemeData(theme) };
        }
    }

    fn fan_cell_clicked(&self, click: &NMITEMACTIVATE) {
        if click.iItem < 0 {
            return;
        }
        let cell = subitem_rect(self.fans.get(), click.iItem, 2);
        let (auto_rect, custom_rect) = segment_rects(&cell);
        if contains(&auto_rect, click.ptAction) {
            self.perform(Action::FanReset(click.iItem as usize));
        } else if contains(&custom_rect, click.ptAction) {
            self.perform(Action::FanControl(click.iItem as usize));
        }
    }

    fn sensors_custom_draw(&self, draw: &mut NMLVCUSTOMDRAW) -> LRESULT {
        match draw.nmcd.dwDrawStage {
            CDDS_PREPAINT => CDRF_NOTIFYITEMDRAW as LRESULT,
            CDDS_ITEMPREPAINT => CDRF_NOTIFYSUBITEMDRAW as LRESULT,
            stage if stage == CDDS_ITEMPREPAINT | CDDS_SUBITEM => {
                let row = draw.nmcd.dwItemSpec;
                let rows = self.sensor_rows.borrow();
                draw.clrTextBk = if row % 2 == 1 { ROW_ALTERNATE } else { ROW_BASE };
                draw.clrText = rgb(0, 0, 0);
                let mut font = self.fonts.normal;
                match rows.get(row) {
                    Some(SensorRow::Separator) => font = self.fonts.small,
                    Some(SensorRow::Sensor { last_update: Some(at), .. })
                        if draw.iSubItem == 1 && at.elapsed() > STALE_AFTER =>
                    {
                        draw.clrText = STALE_GREY;
                    }
                    _ => {}
                }
                unsafe { SelectObject(draw.nmcd.hdc, font) };
                CDRF_NEWFONT as LRESULT
            }
            _ => CDRF_DODEFAULT as LRESULT,
        }
    }

    fn popup(&self, anchor: HWND, build: impl FnOnce(&mut MenuBuilder)) {
        let mut builder = MenuBuilder::new();
        build(&mut builder);
        let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        unsafe { GetWindowRect(anchor, &mut rect) };
        if let Some(action) = builder.track(self.hwnd.get(), rect.left, rect.bottom) {
            self.perform(action);
        }
    }

    fn show_presets_menu(&self) {
        let lang = self.lang.borrow();
        let (current, custom) = {
            let engine = self.engine.borrow();
            let core = engine.core();
            let names: Vec<String> = core.profiles.custom.iter().map(|p| p.name.clone()).collect();
            (core.profiles.current().clone(), names)
        };
        self.popup(self.preset_button.get(), |menu| {
            menu.header(lang.get("Presets/presetMenuPredefined"));
            for (which, path) in [
                (Predefined::Automatic, "Presets/presetAutomatic"),
                (Predefined::FullBlast, "Presets/presetFullBlast"),
            ] {
                let checked = current == Current::Predefined(which);
                menu.item(lang.get(path), Action::SetPredefined(which), checked);
            }
            menu.separator();
            menu.header(lang.get("Presets/presetMenuCustom"));
            for (index, name) in custom.iter().enumerate() {
                let active = current == Current::Custom(index);
                let mut sub = MenuBuilder::child(menu);
                if !active {
                    sub.item(lang.get("Presets/presetMenuApply"), Action::SetCustom(index), false);
                    sub.separator();
                }
                sub.item(lang.get("Presets/presetMenuRename"), Action::RenameCustom(index), false);
                sub.item(lang.get("Presets/presetMenuDelete"), Action::DeleteCustom(index), false);
                menu.submenu(name, sub, active);
            }
            match current {
                Current::Predefined(_) => {
                    menu.item(lang.get("Presets/presetCreateNew"), Action::NewPreset, false)
                }
                Current::Unsaved(_) => {
                    let mut sub = MenuBuilder::child(menu);
                    unsaved_items(&mut sub, &lang);
                    menu.submenu(lang.get("Presets/presetCurrentUnsaved"), sub, true);
                }
                Current::Custom(_) => {}
            }
        });
    }

    fn show_action_menu(&self) {
        let lang = self.lang.borrow();
        let current = self.engine.borrow().core().profiles.current().clone();
        self.popup(self.action_button.get(), |menu| match current {
            Current::Unsaved(_) => unsaved_items(menu, &lang),
            Current::Custom(index) => {
                menu.item(lang.get("Presets/presetMenuRename"), Action::RenameCustom(index), false);
                menu.item(lang.get("Presets/presetMenuDelete"), Action::DeleteCustom(index), false);
            }
            Current::Predefined(_) => {}
        });
    }

    pub fn restore(&self) {
        let hwnd = self.hwnd.get();
        unsafe {
            if IsWindowVisible(hwnd) == 0 {
                ShowWindow(hwnd, SW_SHOWNORMAL);
            }
            SetForegroundWindow(hwnd);
        }
        self.update_values();
    }

    fn open_url(&self, url: &str) {
        unsafe {
            ShellExecuteW(
                self.hwnd.get(),
                wide("open").as_ptr(),
                wide(url).as_ptr(),
                null(),
                null(),
                SW_SHOWNORMAL,
            );
        }
    }

    fn info(&self, title: &str, message: &str) {
        unsafe {
            MessageBoxW(
                self.hwnd.get(),
                wide(message).as_ptr(),
                wide(title).as_ptr(),
                MB_OK | MB_ICONINFORMATION,
            );
        }
    }

    /// Asks for a preset name; `None` when cancelled.
    fn ask_preset_name(&self, initial: &str, renaming: bool) -> Option<String> {
        super::dialogs::preset_name(self, initial, renaming)
    }

    pub fn perform(&self, action: Action) {
        let store = self.store();
        let custom_profile = |index: usize| -> Option<Profile> {
            self.engine.borrow().core().profiles.custom.get(index).cloned()
        };
        match action {
            Action::Show => self.restore(),
            Action::SetPredefined(which) => {
                let engine = self.engine.borrow();
                let mut core = engine.core();
                let profile = core.profiles.predefined(which).clone();
                core.set_profile(profile, &*store);
            }
            Action::SetCustom(index) => {
                if let Some(profile) = custom_profile(index) {
                    self.engine.borrow().core().set_profile(profile, &*store);
                }
            }
            Action::RenameCustom(index) => {
                if let Some(profile) = custom_profile(index) {
                    if let Some(name) = self.ask_preset_name(&profile.name, true) {
                        self.engine.borrow().core().profiles.rename(&profile, &name, &*store);
                    }
                }
            }
            Action::DeleteCustom(index) => {
                if let Some(profile) = custom_profile(index) {
                    self.engine.borrow().core().delete_profile(&profile, &*store);
                }
            }
            Action::NewPreset => {
                let full_blast = self.engine.borrow().core().profiles.current()
                    == &Current::Predefined(Predefined::FullBlast);
                let message = if full_blast {
                    self.lang.borrow().format(
                        "Presets/presetCreateNewMessage2",
                        &[&self.tr("Presets/presetFullBlast")],
                    )
                } else {
                    self.tr("Presets/presetCreateNewMessage")
                };
                self.info(&self.tr("Presets/presetCreateNewTitle"), &message);
            }
            Action::SaveUnsaved => {
                let initial = self.engine.borrow().core().profiles.current_profile().name;
                let initial = if initial == "UNSAVED" { String::new() } else { initial };
                if let Some(name) = self.ask_preset_name(&initial, false) {
                    self.engine.borrow().core().profiles.save_current_as(&name, &*store);
                }
            }
            Action::DiscardUnsaved => {
                self.perform(Action::SetPredefined(Predefined::Automatic));
                return;
            }
            Action::FanControl(fan) => super::fan_dialog::show_dialog(self, fan),
            Action::FanReset(fan) => {
                let engine = self.engine.borrow();
                let mut core = engine.core();
                let automatic = core.profiles.current_profile().modes.get(fan) == Some(&Mode::Automatic);
                if !automatic {
                    core.set_fan_mode(fan, Mode::Automatic, None, &*store);
                }
            }
            Action::Preferences => super::preferences::show(self),
            Action::About => super::dialogs::about(self),
            Action::TechZip => self.info(&text::app_title(), "Technical info export is not available yet."),
            Action::RateApp => {
                store.set_bool("RateOnMacupdateSkip", true);
                self.open_url("https://www.macupdate.com/app/mac/47386/macs-fan-control");
            }
            Action::Twitter => self.open_url("https://twitter.com/crystalidea"),
            Action::Website => self.open_url("https://crystalidea.com/macs-fan-control"),
            Action::Exit => {
                self.quit();
                return;
            }
        }
        self.profiles_changed();
    }

    fn save_placement(&self) {
        let store = self.store();
        let hwnd = self.hwnd.get();
        let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        unsafe { GetWindowRect(hwnd, &mut rect) };
        store.set_string(
            "RustyWindowPlacement",
            &format!("{},{},{},{}", rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top),
        );
        let total = client_rect(hwnd).right - 2 * px(MARGIN) - px(SPACING);
        let right = self.right_width.get();
        let logical = |device: i32| (device / win::scale()).to_string();
        store.set_strings("WindowSplitterPosition", &[logical(total - right), logical(right)]);
    }

    /// Saves the layout, hides everything, hands the fans back to the system and exits.
    pub fn quit(&self) {
        if self.quitting.replace(true) {
            return;
        }
        self.save_placement();
        unsafe { ShowWindow(self.hwnd.get(), SW_HIDE) };
        self.tray.borrow_mut().take();
        self.engine.borrow_mut().stop();
        unsafe { DestroyWindow(self.hwnd.get()) };
    }

    fn splitter_drag(&self, x: i32) {
        let client = client_rect(self.hwnd.get());
        self.right_width.set(client.right - px(MARGIN) - x - px(SPACING) / 2);
        self.layout();
    }
}

fn unsaved_items(menu: &mut MenuBuilder, lang: &Language) {
    menu.item(&format!("{}\tCtrl+S", lang.get("Presets/presetMenuSave")), Action::SaveUnsaved, false);
    menu.separator();
    menu.item(lang.get("Presets/presetMenuDiscard"), Action::DiscardUnsaved, false);
}

/// Builds a popup menu whose items carry [`Action`]s.
pub struct MenuBuilder {
    pub handle: HMENU,
    actions: Vec<Action>,
    /// First action index this (sub)menu may use, so ids stay unique across submenus.
    base: usize,
    default: Option<usize>,
}

impl MenuBuilder {
    pub fn new() -> MenuBuilder {
        MenuBuilder { handle: unsafe { CreatePopupMenu() }, actions: Vec::new(), base: 0, default: None }
    }

    /// A submenu that numbers its items after everything added to `parent` so far.
    pub fn child(parent: &MenuBuilder) -> MenuBuilder {
        MenuBuilder {
            handle: unsafe { CreatePopupMenu() },
            actions: Vec::new(),
            base: parent.base + parent.actions.len(),
            default: None,
        }
    }

    pub fn item(&mut self, text: &str, action: Action, checked: bool) {
        let id = MENU_BASE + self.base + self.actions.len();
        self.actions.push(action);
        let flags = MF_STRING | if checked { MF_CHECKED } else { 0 };
        unsafe { AppendMenuW(self.handle, flags, id, wide(text).as_ptr()) };
    }

    pub fn default_item(&mut self, text: &str, action: Action) {
        self.default = Some(MENU_BASE + self.base + self.actions.len());
        self.item(text, action, false);
    }

    pub fn header(&mut self, text: &str) {
        unsafe { AppendMenuW(self.handle, MF_STRING | MF_DISABLED | MF_GRAYED, 0, wide(text).as_ptr()) };
    }

    pub fn separator(&mut self) {
        unsafe { AppendMenuW(self.handle, MF_SEPARATOR, 0, null()) };
    }

    pub fn submenu(&mut self, text: &str, sub: MenuBuilder, checked: bool) {
        let flags = MF_POPUP | if checked { MF_CHECKED } else { 0 };
        unsafe { AppendMenuW(self.handle, flags, sub.handle as usize, wide(text).as_ptr()) };
        self.actions.extend(sub.actions);
    }

    /// Shows the menu at a screen position and returns the chosen action.
    pub fn track(self, owner: HWND, x: i32, y: i32) -> Option<Action> {
        unsafe {
            if let Some(id) = self.default {
                SetMenuDefaultItem(self.handle, id as u32, 0);
            }
            SetForegroundWindow(owner);
            let chosen =
                TrackPopupMenu(self.handle, TPM_RETURNCMD | TPM_RIGHTBUTTON, x, y, 0, owner, null());
            DestroyMenu(self.handle);
            (chosen as usize)
                .checked_sub(MENU_BASE + self.base)
                .and_then(|index| self.actions.get(index).cloned())
        }
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let Some(this) = window() else {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    };
    match message {
        WM_SIZE => {
            this.layout();
            0
        }
        WM_GETMINMAXINFO => {
            let info = &mut *(lparam as *mut MINMAXINFO);
            info.ptMinTrackSize.x = px(MIN_FANS_WIDTH + MIN_RIGHT_PANE + 2 * MARGIN + SPACING + 16);
            info.ptMinTrackSize.y = px(240);
            0
        }
        WM_MEASUREITEM => {
            let measure = &mut *(lparam as *mut MEASUREITEMSTRUCT);
            if measure.CtlType == ODT_LISTVIEW {
                measure.itemHeight = px(FAN_ROW_HEIGHT) as u32;
            }
            1
        }
        WM_DRAWITEM => {
            let draw = &*(lparam as *const DRAWITEMSTRUCT);
            if draw.CtlID == ID_FANS as u32 {
                this.draw_fan_row(draw);
            }
            1
        }
        WM_NOTIFY => {
            let header = &*(lparam as *const NMHDR);
            match (header.idFrom as i32, header.code) {
                (ID_FANS, NM_CLICK) => {
                    this.fan_cell_clicked(&*(lparam as *const NMITEMACTIVATE));
                    0
                }
                (ID_SENSORS, NM_CUSTOMDRAW) => {
                    this.sensors_custom_draw(&mut *(lparam as *mut NMLVCUSTOMDRAW))
                }
                (ID_NO_FANS, NM_CLICK) | (ID_NO_FANS, NM_RETURN) => {
                    let link = &*(lparam as *const NMLINK);
                    let url = &link.item.szUrl;
                    let end = url.iter().position(|&c| c == 0).unwrap_or(url.len());
                    this.open_url(&String::from_utf16_lossy(&url[..end]));
                    0
                }
                _ => DefWindowProcW(hwnd, message, wparam, lparam),
            }
        }
        WM_COMMAND => {
            match win::lo_word(wparam) as i32 {
                ID_PRESET_BUTTON => this.show_presets_menu(),
                ID_ACTION_BUTTON => this.show_action_menu(),
                ID_MINIMIZE => {
                    ShowWindow(hwnd, SW_HIDE);
                }
                ID_PREFERENCES => this.perform(Action::Preferences),
                ID_ABOUT => this.perform(Action::About),
                _ => {}
            }
            0
        }
        WM_SETCURSOR if win::lo_word(lparam as usize) == HTCLIENT => {
            let mut point = POINT { x: 0, y: 0 };
            GetCursorPos(&mut point);
            windows_sys::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut point);
            if contains(&this.splitter_rect(), point) || this.dragging_splitter.get() {
                SetCursor(LoadCursorW(null_mut(), IDC_SIZEWE));
                return 1;
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        WM_LBUTTONDOWN => {
            let point = POINT { x: win::x_of(lparam), y: win::y_of(lparam) };
            if contains(&this.splitter_rect(), point) {
                this.dragging_splitter.set(true);
                SetCapture(hwnd);
            }
            0
        }
        WM_MOUSEMOVE => {
            if this.dragging_splitter.get() {
                this.splitter_drag(win::x_of(lparam));
            }
            0
        }
        WM_LBUTTONUP => {
            if this.dragging_splitter.replace(false) {
                ReleaseCapture();
            }
            0
        }
        WM_REFRESHED => {
            this.update_values();
            0
        }
        WM_RESTORE => {
            this.restore();
            0
        }
        WM_TRAY => {
            let event = win::lo_word(lparam as usize);
            if let Some(action) = super::tray::handle_event(&this, event) {
                this.perform(action);
            }
            0
        }
        WM_POWERBROADCAST => {
            match wparam as u32 {
                PBT_APMSUSPEND => this.engine.borrow().entering_sleep(),
                PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => this.engine.borrow().woke_up(),
                _ => {}
            }
            1
        }
        WM_QUERYENDSESSION => {
            if lparam as u32 & ENDSESSION_LOGOFF != 0 {
                this.engine.borrow().user_logged_out();
            }
            this.quit();
            1
        }
        WM_ENDSESSION => 0,
        WM_CLOSE => {
            // The close box hides to the tray; only Exit really quits.
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

fn restore_placement(hwnd: HWND, store: &dyn Store) -> bool {
    let Some(saved) = store.get_string("RustyWindowPlacement") else {
        return false;
    };
    let numbers: Vec<i32> = saved.split(',').filter_map(|part| part.parse().ok()).collect();
    let [x, y, width, height] = numbers[..] else {
        return false;
    };
    let (screen_w, screen_h) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
    if width < px(200) || height < px(150) || x > screen_w - px(50) || y > screen_h - px(50) || x + width < px(50) || y < 0 {
        return false;
    }
    unsafe { MoveWindow(hwnd, x, y, width, height, 0) };
    true
}

/// Creates the main window and runs the message loop until the app exits.
pub fn run(mut engine: Engine, lang: Language, start_hidden: bool, initial: Option<Action>) {
    unsafe {
        let controls = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES | ICC_STANDARD_CLASSES | ICC_LINK_CLASS | ICC_UPDOWN_CLASS,
        };
        InitCommonControlsEx(&controls);

        let class_name = wide(CLASS_NAME);
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wnd_proc),
            hInstance: win::instance(),
            hIcon: LoadIconW(win::instance(), 1 as _),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: GetSysColorBrush(COLOR_BTNFACE),
            lpszClassName: class_name.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassExW(&class);

        let model_name = engine.core().model.name.clone();
        let title = format!("{} ({model_name})", text::app_title());
        let (width, height) = (px(821), px(375));
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            wide(&title).as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            width,
            height,
            null_mut(),
            null_mut(),
            win::instance(),
            null(),
        );

        let store = engine.store_arc();
        let saved_split: Vec<i32> = store
            .get_strings("WindowSplitterPosition")
            .iter()
            .filter_map(|part| part.parse().ok())
            .collect();
        let right_width = match saved_split[..] {
            [_, right] => px(right),
            _ => px(DEFAULT_RIGHT_PANE),
        };

        engine.start(Arc::new(UiHost(hwnd as isize)));
        let this = Rc::new(MainWindow {
            hwnd: Cell::new(hwnd),
            engine: RefCell::new(engine),
            lang: RefCell::new(lang),
            fonts: Fonts::new(),
            tray: RefCell::new(None),
            preset_label: Cell::default(),
            preset_button: Cell::default(),
            action_button: Cell::default(),
            fans: Cell::default(),
            no_fans: Cell::default(),
            sensors_label: Cell::default(),
            sensors: Cell::default(),
            buttons: Default::default(),
            fan_icons: Cell::new(0),
            fan_rows: RefCell::default(),
            sensor_rows: RefCell::default(),
            right_width: Cell::new(right_width),
            dragging_splitter: Cell::new(false),
            quitting: Cell::new(false),
        });
        this.create_children(hwnd);
        WINDOW.with(|w| *w.borrow_mut() = Some(this.clone()));
        this.retranslate();
        *this.tray.borrow_mut() = Some(Tray::new(&this));

        if !restore_placement(hwnd, &*store) {
            // Tall enough to list every sensor, centred on the screen.
            let rows = this.sensor_rows.borrow().len() as i32;
            let wanted = (px(140) + rows * px(19)).max(height);
            let (screen_w, screen_h) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
            let height = wanted.min(screen_h - px(80));
            SetWindowPos(hwnd, null_mut(), 0, 0, width, height, SWP_NOMOVE | SWP_NOZORDER);
            MoveWindow(hwnd, (screen_w - width) / 2, (screen_h - height) / 2, width, height, 0);
        }
        this.layout();
        if !start_hidden {
            ShowWindow(hwnd, SW_SHOWNORMAL);
        }
        if let Some(action) = initial {
            this.perform(action);
        }

        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            if IsDialogMessageW(hwnd, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        WINDOW.with(|w| w.borrow_mut().take());
    }
}
