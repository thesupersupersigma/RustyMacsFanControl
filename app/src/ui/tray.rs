//! The notification-area icon, its tooltip and its menu.

use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW,
    FillRect, SelectObject, SetBkMode, SetTextColor, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, DT_CENTER, DT_SINGLELINE, DT_VCENTER, TRANSPARENT,
};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, GetCursorPos, HICON, WM_CONTEXTMENU, WM_LBUTTONDBLCLK, WM_RBUTTONUP,
};

use super::main_window::{Action, MainWindow, MenuBuilder, WM_TRAY};
use super::text::mode_text;
use super::win::{self, px, rgb, wide};
use crate::fan::mode::Mode;
use crate::fan::profile::{Current, Predefined};

const TRAY_ID: u32 = 1;
/// Background of the temperature icon.
const DARK_BLUE: u32 = rgb(0, 0, 0x80);

pub struct Tray {
    hwnd: HWND,
    app_icon: HICON,
    /// Generated icon showing the tray sensor's temperature, with the text it shows.
    sensor_icon: Option<(HICON, String)>,
    tooltip: String,
}

fn notify_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = TRAY_ID;
    data
}

/// A square icon with white text on dark blue.
fn text_icon(text: &str, font: windows_sys::Win32::Graphics::Gdi::HFONT) -> HICON {
    let size = px(16);
    let mut info: BITMAPINFO = unsafe { std::mem::zeroed() };
    info.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: size,
        biHeight: -size,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB as u32,
        ..unsafe { std::mem::zeroed() }
    };
    let mut bits: *mut core::ffi::c_void = null_mut();
    unsafe {
        let bitmap = CreateDIBSection(null_mut(), &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        let dc = CreateCompatibleDC(null_mut());
        let old_bitmap = SelectObject(dc, bitmap);
        let mut rect = RECT { left: 0, top: 0, right: size, bottom: size };
        let brush = CreateSolidBrush(DARK_BLUE);
        FillRect(dc, &rect, brush);
        DeleteObject(brush);
        let old_font = SelectObject(dc, font);
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, rgb(255, 255, 255));
        let units: Vec<u16> = text.encode_utf16().collect();
        DrawTextW(dc, units.as_ptr(), units.len() as i32, &mut rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
        SelectObject(dc, old_font);
        SelectObject(dc, old_bitmap);
        DeleteDC(dc);
        // GDI leaves the alpha channel at zero; the icon is fully opaque.
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u8>(), (size * size * 4) as usize);
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
        let icon = win::icon_from_bitmap(bitmap, size, size);
        DeleteObject(bitmap);
        icon
    }
}

impl Tray {
    pub fn new(window: &MainWindow) -> Tray {
        let app_icon = win::load_image("tray")
            .map(|image| {
                let bitmap = image.to_bitmap();
                let icon = win::icon_from_bitmap(bitmap, image.width, image.height);
                unsafe { DeleteObject(bitmap) };
                icon
            })
            .unwrap_or(null_mut());
        let hwnd = window.hwnd.get();
        let mut data = notify_data(hwnd);
        data.uFlags = NIF_ICON | NIF_MESSAGE;
        data.uCallbackMessage = WM_TRAY;
        data.hIcon = app_icon;
        unsafe { Shell_NotifyIconW(NIM_ADD, &data) };
        let mut tray = Tray { hwnd, app_icon, sensor_icon: None, tooltip: String::new() };
        tray.update(window);
        tray
    }

    /// Refreshes the tooltip and, when a tray sensor is chosen, the temperature icon.
    pub fn update(&mut self, window: &MainWindow) {
        let tooltip = window
            .fan_tooltips()
            .iter()
            .map(|(name, rpm)| format!("{name} - {rpm} RPM"))
            .collect::<Vec<_>>()
            .join("\n");

        // The tray shows whole degrees Celsius whatever the display scale is.
        let wanted = {
            let engine = window.engine.borrow();
            let key = engine.store().get_string("traySensor").unwrap_or_default();
            let core = engine.core();
            (!key.is_empty())
                .then(|| core.sensors.find(&key))
                .flatten()
                .map(|index| (core.sensors.sensors()[index].value.round() as i64).to_string())
        };
        let icon_changed = match (&self.sensor_icon, &wanted) {
            (Some((_, shown)), Some(text)) => shown != text,
            (None, None) => false,
            _ => true,
        };
        if !icon_changed && tooltip == self.tooltip {
            return;
        }
        if icon_changed {
            if let Some((old, _)) = self.sensor_icon.take() {
                unsafe { DestroyIcon(old) };
            }
            self.sensor_icon = wanted.map(|text| (text_icon(&text, window.fonts.small), text));
        }
        self.tooltip = tooltip;

        let mut data = notify_data(self.hwnd);
        data.uFlags = NIF_ICON | NIF_TIP;
        data.hIcon = self.sensor_icon.as_ref().map_or(self.app_icon, |(icon, _)| *icon);
        let tip = wide(&self.tooltip);
        let len = tip.len().min(data.szTip.len() - 1);
        data.szTip[..len].copy_from_slice(&tip[..len]);
        data.szTip[len] = 0;
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &notify_data(self.hwnd));
            DestroyIcon(self.app_icon);
            if let Some((icon, _)) = self.sensor_icon.take() {
                DestroyIcon(icon);
            }
        }
    }
}

/// Handles a notification from the tray icon: double-click restores the window, right
/// click opens the menu. Returns what the user chose, if anything.
pub fn handle_event(window: &MainWindow, event: u32) -> Option<Action> {
    match event {
        WM_LBUTTONDBLCLK => Some(Action::Show),
        WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(window),
        _ => None,
    }
}

fn show_menu(window: &MainWindow) -> Option<Action> {
    let lang = window.lang.borrow();
    // Copy what the menu shows so the engine is not locked while the menu is open.
    let (fans, current, custom) = {
        let engine = window.engine.borrow();
        let core = engine.core();
        let profile = core.profiles.current_profile();
        let fans: Vec<(String, Mode)> = core
            .fans
            .iter()
            .enumerate()
            .map(|(index, fan)| {
                (fan.name.clone(), profile.modes.get(index).cloned().unwrap_or(Mode::Invalid))
            })
            .collect();
        let custom: Vec<String> = core.profiles.custom.iter().map(|p| p.name.clone()).collect();
        (fans, core.profiles.current().clone(), custom)
    };

    let mut menu = MenuBuilder::new();
    menu.default_item(lang.get("TrayMenu/itemShow"), Action::Show);
    menu.separator();
    if !fans.is_empty() {
        menu.header(lang.get("TrayMenu/itemFanPresets"));
        for (which, path) in [
            (Predefined::Automatic, "Presets/presetAutomatic"),
            (Predefined::FullBlast, "Presets/presetFullBlast"),
        ] {
            menu.item(lang.get(path), Action::SetPredefined(which), current == Current::Predefined(which));
        }
        for (index, name) in custom.iter().enumerate() {
            menu.item(name, Action::SetCustom(index), current == Current::Custom(index));
        }
        if matches!(current, Current::Unsaved(_)) {
            let mut sub = MenuBuilder::child(&menu);
            sub.item(lang.get("Presets/presetMenuSave"), Action::SaveUnsaved, false);
            sub.separator();
            sub.item(lang.get("Presets/presetMenuDiscard"), Action::DiscardUnsaved, false);
            menu.submenu(lang.get("Presets/presetCurrentUnsaved"), sub, true);
        }
        menu.separator();

        menu.header(lang.get("TrayMenu/itemAvailableFans"));
        for (index, (name, mode)) in fans.iter().enumerate() {
            let mut sub = MenuBuilder::child(&menu);
            sub.item(lang.get("MainWindow/popupMenuItemControl"), Action::FanControl(index), false);
            if *mode != Mode::Automatic {
                sub.separator();
                sub.item(lang.get("MainWindow/popupMenuItemReset"), Action::FanReset(index), false);
            }
            let description = mode_text(&lang, mode, true, &|_| String::new());
            menu.submenu(&format!("{name} - {description}"), sub, false);
        }
        menu.separator();
    }
    menu.item(lang.get("MainWindow/buttonPreferences"), Action::Preferences, false);
    menu.separator();
    let mut more = MenuBuilder::child(&menu);
    more.item(lang.get("MainWindow/buttonAbout"), Action::About, false);
    more.item(lang.get("TrayMenu/itemTechInfoZip"), Action::TechZip, false);
    more.separator();
    more.item(lang.get("TrayMenu/itemWebsite"), Action::Website, false);
    menu.submenu(lang.get("TrayMenu/itemMoreLinks"), more, false);
    menu.separator();
    menu.item(lang.get("TrayMenu/itemExit"), Action::Exit, false);

    let mut point = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut point) };
    menu.track(window.hwnd.get(), point.x, point.y)
}
