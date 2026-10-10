//! Preferences: General, T° Sensors and Tray icon pages behind an icon toolbar. Every
//! control applies immediately; there is no OK button.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::DeleteObject;
use windows_sys::Win32::UI::Controls::{
    ImageList_Add, ImageList_Create, CCS_NODIVIDER, ILC_COLOR32, NMHDR, NM_CLICK,
    NM_RETURN, TBBUTTON, TBSTATE_CHECKED, TBSTATE_ENABLED, TBSTYLE_FLAT, TB_ADDBUTTONSW,
    TB_AUTOSIZE, TB_BUTTONSTRUCTSIZE, TB_GETMAXSIZE, TB_SETIMAGELIST,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    MoveWindow, SendMessageW, ShowWindow, BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX,
    CBN_SELCHANGE, CBS_DROPDOWNLIST, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, IDCANCEL,
    SW_HIDE, SW_SHOW, WM_COMMAND, WM_NOTIFY, WS_TABSTOP,
    WS_VSCROLL,
};

use super::dialogs::{close, create, open_url, place, resize, run_modal};
use super::main_window::MainWindow;
use super::win::{
    self, control, px, wide, BTNS_CHECKGROUP, SS_CENTERIMAGE, SS_ETCHEDHORZ, SS_RIGHT,
};
use crate::autostart;
use crate::lang::Language;

const ID_PAGE_FIRST: i32 = 100;
const ID_AUTOSTART: i32 = 10;
const ID_LANGUAGE: i32 = 11;
const ID_TRANSLATE: i32 = 12;
const ID_DRIVES: i32 = 20;
const ID_DRIVES_EXTERNAL: i32 = 21;
const ID_EGPU: i32 = 22;
const ID_FAHRENHEIT: i32 = 23;
const ID_PRECISE: i32 = 24;
const ID_TRAY_SENSOR: i32 = 30;

const WIDTH: i32 = 445;
/// Client heights of the three pages.
const PAGE_HEIGHTS: [i32; 3] = [86, 168, 60];

fn send(hwnd: HWND, message: u32, wparam: usize, lparam: isize) -> isize {
    unsafe { SendMessageW(hwnd, message, wparam, lparam) }
}

fn is_checked(hwnd: HWND) -> bool {
    send(hwnd, BM_GETCHECK, 0, 0) == 1
}

fn set_checked(hwnd: HWND, checked: bool) {
    send(hwnd, BM_SETCHECK, checked as usize, 0);
}

/// Folders searched for language files: next to the exe, then the original app's.
pub fn language_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|p| p.join("languages"))) {
        dirs.push(dir);
    }
    dirs.push(PathBuf::from(r"C:\Program Files (x86)\Macs Fan Control\languages"));
    dirs
}

/// Language names (file names without `.xml`), alphabetical, with the folder they are in.
pub fn available_languages() -> (Option<PathBuf>, Vec<String>) {
    for dir in language_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.strip_suffix(".xml").map(str::to_string)
            })
            .collect();
        if !names.is_empty() {
            names.sort();
            return (Some(dir), names);
        }
    }
    (None, vec!["English".to_string()])
}

/// Loads the language a settings value such as `German.xml` names.
pub fn load_language(file_name: &str) -> Language {
    if file_name.is_empty() || file_name == "English.xml" {
        return Language::english();
    }
    language_dirs()
        .into_iter()
        .map(|dir| dir.join(file_name))
        .find(|path| path.is_file())
        .map_or_else(Language::english, |path| Language::load(&path))
}

pub fn show(window: &MainWindow) {
    let owner = window.hwnd.get();
    let store = window.engine.borrow().store_arc();
    let font = window.fonts.normal;
    let tr = |path: &str| window.lang.borrow().get(path).to_string();
    let page_titles = [
        tr("PreferencesWindow/TabGeneral"),
        format!("T\u{B0} {}", tr("PreferencesWindow/tabSensors")),
        tr("PreferencesWindow/tabTrayIcon"),
    ];
    let dialog = create(owner, &tr("PreferencesWindow/title"), WIDTH, 200);

    // Page selector: icon above text, one checked at a time.
    let toolbar = control(
        dialog,
        "ToolbarWindow32",
        "",
        TBSTYLE_FLAT | CCS_NODIVIDER as u32,
        0,
        0,
        font,
    );
    send(toolbar, TB_BUTTONSTRUCTSIZE, std::mem::size_of::<TBBUTTON>(), 0);
    let images = unsafe { ImageList_Create(px(32), px(32), ILC_COLOR32, 3, 0) };
    for name in ["preferences_general", "preferences_sensors", "preferences_menubar"] {
        if let Some(image) = win::load_image(name) {
            let bitmap = image.to_bitmap();
            unsafe {
                ImageList_Add(images, bitmap, std::ptr::null_mut());
                DeleteObject(bitmap);
            }
        }
    }
    send(toolbar, TB_SETIMAGELIST, 0, images as isize);
    let titles: Vec<Vec<u16>> = page_titles.iter().map(|title| wide(title)).collect();
    let buttons: Vec<TBBUTTON> = titles
        .iter()
        .enumerate()
        .map(|(index, title)| {
            let mut button: TBBUTTON = unsafe { std::mem::zeroed() };
            button.iBitmap = index as i32;
            button.idCommand = ID_PAGE_FIRST + index as i32;
            button.fsState = (TBSTATE_ENABLED | if index == 0 { TBSTATE_CHECKED } else { 0 }) as u8;
            button.fsStyle = BTNS_CHECKGROUP as u8;
            button.iString = title.as_ptr() as isize;
            button
        })
        .collect();
    send(toolbar, TB_ADDBUTTONSW, buttons.len(), buttons.as_ptr() as isize);
    send(toolbar, TB_AUTOSIZE, 0, 0);
    let mut toolbar_size = windows_sys::Win32::Foundation::SIZE { cx: 0, cy: 0 };
    send(toolbar, TB_GETMAXSIZE, 0, &mut toolbar_size as *mut _ as isize);
    let toolbar_height = toolbar_size.cy / win::scale() + 6;
    unsafe { MoveWindow(toolbar, 0, 0, px(WIDTH), px(toolbar_height), 1) };
    let top = toolbar_height + 8;
    let left = 40;

    // General.
    let autostart_box = control(dialog, "BUTTON", &tr("PreferencesWindow/checkSystemStart"), BS_AUTOCHECKBOX as u32 | WS_TABSTOP, 0, ID_AUTOSTART, font);
    place(autostart_box, left, top, WIDTH - left - 11, 20);
    set_checked(autostart_box, autostart::is_enabled());
    let language_label = control(dialog, "STATIC", &tr("PreferencesWindow/staticLanguage"), SS_RIGHT | SS_CENTERIMAGE, 0, 0, font);
    place(language_label, left, top + 30, 80, 21);
    let language_combo = control(dialog, "COMBOBOX", "", CBS_DROPDOWNLIST as u32 | WS_VSCROLL | WS_TABSTOP, 0, ID_LANGUAGE, font);
    place(language_combo, left + 86, top + 30, 210, 400);
    let (_, languages) = available_languages();
    let current_language = store
        .get_string("InterfaceLanguage")
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "English.xml".to_string());
    for (index, name) in languages.iter().enumerate() {
        send(language_combo, CB_ADDSTRING, 0, wide(&name.replace('_', " ")).as_ptr() as isize);
        if format!("{name}.xml") == current_language {
            send(language_combo, CB_SETCURSEL, index, 0);
        }
    }
    let translate = control(dialog, "SysLink", "<a href=\"translate\">Translate</a>", WS_TABSTOP, 0, ID_TRANSLATE, font);
    place(translate, left + 86 + 216, top + 33, 60, 16);
    let general = vec![autostart_box, language_label, language_combo, translate];

    // T° Sensors.
    let sensors_label = control(dialog, "STATIC", &tr("PreferencesWindow/labelSensors"), 0, 0, 0, font);
    place(sensors_label, left, top, WIDTH - left - 11, 16);
    let drives = control(dialog, "BUTTON", &tr("PreferencesWindow/checkDrives"), BS_AUTOCHECKBOX as u32 | WS_TABSTOP, 0, ID_DRIVES, font);
    place(drives, left, top + 22, WIDTH - left - 11, 20);
    let external_text = format!("{} (Thunderbolt/USB)", tr("PreferencesWindow/checkDrivesExternal"));
    let drives_external = control(dialog, "BUTTON", &external_text, BS_AUTOCHECKBOX as u32 | WS_TABSTOP, 0, ID_DRIVES_EXTERNAL, font);
    place(drives_external, left + 18, top + 44, WIDTH - left - 29, 20);
    let egpu = control(dialog, "BUTTON", &tr("PreferencesWindow/checkExternalGPU"), BS_AUTOCHECKBOX as u32 | WS_TABSTOP, 0, ID_EGPU, font);
    place(egpu, left, top + 66, WIDTH - left - 11, 20);
    let line = control(dialog, "STATIC", "", SS_ETCHEDHORZ, 0, 0, font);
    place(line, left, top + 94, WIDTH - left - 11, 2);
    let fahrenheit = control(dialog, "BUTTON", &tr("PreferencesWindow/checkFahrenheit"), BS_AUTOCHECKBOX as u32 | WS_TABSTOP, 0, ID_FAHRENHEIT, font);
    place(fahrenheit, left, top + 104, WIDTH - left - 11, 20);
    let precise = control(dialog, "BUTTON", &tr("PreferencesWindow/checkPrecise"), BS_AUTOCHECKBOX as u32 | WS_TABSTOP, 0, ID_PRECISE, font);
    place(precise, left, top + 126, WIDTH - left - 11, 20);
    let show_drives = store.get_bool("ShowDrives", true);
    set_checked(drives, show_drives);
    set_checked(drives_external, store.get_bool("ShowDrivesExternal", false));
    set_checked(egpu, store.get_bool("ShowEGpuTemp", false));
    set_checked(fahrenheit, store.get_bool("Fahrenheit", false));
    set_checked(precise, store.get_bool("PreciseTemperature", false));
    unsafe {
        EnableWindow(drives_external, show_drives as i32);
        EnableWindow(egpu, 0);
    }
    let sensors_page = vec![sensors_label, drives, drives_external, egpu, line, fahrenheit, precise];

    // Tray icon.
    let tray_label = control(dialog, "STATIC", &tr("PreferencesWindow/staticSensor"), SS_RIGHT | SS_CENTERIMAGE, 0, 0, font);
    place(tray_label, left, top, 60, 21);
    let tray_combo = control(dialog, "COMBOBOX", "", CBS_DROPDOWNLIST as u32 | WS_VSCROLL | WS_TABSTOP, 0, ID_TRAY_SENSOR, font);
    place(tray_combo, left + 66, top, 200, 400);
    let celsius_only = control(dialog, "STATIC", &tr("PreferencesWindow/staticCelsiusOnlyOnWindows"), SS_CENTERIMAGE, 0, 0, font);
    place(celsius_only, left + 272, top, 110, 21);
    send(tray_combo, CB_ADDSTRING, 0, wide(&tr("PreferencesWindow/comboOptionNone")).as_ptr() as isize);
    send(tray_combo, CB_SETCURSEL, 0, 0);
    let tray_setting = store.get_string("traySensor").unwrap_or_default();
    let tray_keys: Vec<String> = {
        let engine = window.engine.borrow();
        let core = engine.core();
        core.sensors
            .visible()
            .into_iter()
            .enumerate()
            .map(|(position, index)| {
                let sensor = &core.sensors.sensors()[index];
                send(tray_combo, CB_ADDSTRING, 0, wide(&sensor.name).as_ptr() as isize);
                if sensor.key == tray_setting {
                    send(tray_combo, CB_SETCURSEL, position + 1, 0);
                }
                sensor.key.clone()
            })
            .collect()
    };
    let tray_page = vec![tray_label, tray_combo, celsius_only];

    let pages = [general, sensors_page, tray_page];
    let current_page = Rc::new(Cell::new(0usize));
    let select_page = {
        let pages = pages.clone();
        let titles = page_titles.clone();
        let base_title = tr("PreferencesWindow/title");
        move |dialog: HWND, page: usize| {
            for (index, controls) in pages.iter().enumerate() {
                for &control in controls {
                    unsafe { ShowWindow(control, if index == page { SW_SHOW } else { SW_HIDE }) };
                }
            }
            win::set_text(dialog, &format!("{base_title}: {}", titles[page]));
            resize(dialog, WIDTH, top + PAGE_HEIGHTS[page] + 11);
        }
    };
    select_page(dialog, 0);

    let window_ptr: *const MainWindow = window;
    run_modal(
        owner,
        dialog,
        Box::new(move |hwnd, message, wparam, lparam| {
            // The main window outlives this modal loop.
            let window = unsafe { &*window_ptr };
            match message {
                WM_NOTIFY => {
                    let header = unsafe { &*(lparam as *const NMHDR) };
                    if header.idFrom as i32 == ID_TRANSLATE
                        && (header.code == NM_CLICK || header.code == NM_RETURN)
                    {
                        open_url(hwnd, "https://crystalidea.com/macs-fan-control-translation");
                        return Some(0);
                    }
                    None
                }
                WM_COMMAND => {
                    let (id, code) = (win::lo_word(wparam) as i32, win::hi_word(wparam));
                    match id {
                        IDCANCEL => close(owner, hwnd),
                        id if (ID_PAGE_FIRST..ID_PAGE_FIRST + 3).contains(&id) => {
                            let page = (id - ID_PAGE_FIRST) as usize;
                            current_page.set(page);
                            select_page(hwnd, page);
                        }
                        ID_AUTOSTART => {
                            autostart::set_enabled(is_checked(autostart_box));
                            // Show what actually happened.
                            set_checked(autostart_box, autostart::is_enabled());
                        }
                        ID_LANGUAGE if code == CBN_SELCHANGE => {
                            let index = send(language_combo, CB_GETCURSEL, 0, 0);
                            if let Some(name) = languages.get(index.max(0) as usize) {
                                let file = format!("{name}.xml");
                                store.set_string("InterfaceLanguage", &file);
                                *window.lang.borrow_mut() = load_language(&file);
                                window.retranslate();
                            }
                        }
                        ID_DRIVES => {
                            let on = is_checked(drives);
                            store.set_bool("ShowDrives", on);
                            if !on {
                                set_checked(drives_external, false);
                                store.set_bool("ShowDrivesExternal", false);
                            }
                            unsafe { EnableWindow(drives_external, on as i32) };
                        }
                        ID_DRIVES_EXTERNAL => {
                            store.set_bool("ShowDrivesExternal", is_checked(drives_external));
                        }
                        ID_EGPU => store.set_bool("ShowEGpuTemp", is_checked(egpu)),
                        ID_FAHRENHEIT => {
                            store.set_bool("Fahrenheit", is_checked(fahrenheit));
                            window.retranslate();
                        }
                        ID_PRECISE => {
                            store.set_bool("PreciseTemperature", is_checked(precise));
                            window.retranslate();
                        }
                        ID_TRAY_SENSOR if code == CBN_SELCHANGE => {
                            let index = send(tray_combo, CB_GETCURSEL, 0, 0);
                            let key = if index < 1 {
                                ""
                            } else {
                                tray_keys.get(index as usize - 1).map_or("", String::as_str)
                            };
                            store.set_string("traySensor", key);
                            window.retranslate();
                        }
                        _ => return None,
                    }
                    Some(0)
                }
                _ => None,
            }
        }),
    );
}
