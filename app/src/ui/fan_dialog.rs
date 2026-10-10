//! "Change fan control" dialog: constant RPM or sensor-based control for one fan.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Controls::{
    EM_SETSEL, TBM_SETPOS, TBM_SETRANGEMAX, TBM_SETRANGEMIN, TBS_HORZ, TBS_NOTICKS,
    UDM_SETBUDDY, UDM_SETRANGE32, UDS_ALIGNRIGHT, UDS_ARROWKEYS, UDS_SETBUDDYINT,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetKeyState, SetFocus, VK_CONTROL, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SendMessageW, ShowWindow, BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_AUTORADIOBUTTON,
    CBN_SELCHANGE, CBS_DROPDOWNLIST, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL,     EN_CHANGE, EN_KILLFOCUS, ES_AUTOHSCROLL, ES_NUMBER, IDCANCEL, IDOK, SW_HIDE,
    SW_SHOW, WM_COMMAND, WM_HSCROLL, WS_EX_CLIENTEDGE, WS_GROUP, WS_TABSTOP, WS_VSCROLL,
};

use super::dialogs::{close, create, ok_cancel, place, run_modal};
use super::main_window::MainWindow;
use super::text;
use super::win::{self, control, wide, SS_CENTERIMAGE, TBM_GETPOS};
use crate::engine::NO_RPM;
use crate::fan::mode::Mode;
use crate::fan::profile::Current;

const ID_RADIO_CONSTANT: i32 = 10;
const ID_RADIO_SENSOR: i32 = 11;
const ID_SLIDER: i32 = 12;
const ID_SPIN_CONSTANT: i32 = 13;
const ID_COMBO: i32 = 14;
const ID_SPIN_INCREASE: i32 = 15;
const ID_SPIN_MAX: i32 = 16;
const ID_CHECK_SAVE: i32 = 17;

thread_local! {
    /// The mode last applied through this dialog for each fan, offered again when the
    /// fan is on automatic.
    static LAST_APPLIED: RefCell<HashMap<usize, Mode>> = RefCell::new(HashMap::new());
}

struct SensorChoice {
    key: String,
    name: String,
    suggested_min: i32,
    suggested_max: i32,
}

fn send(hwnd: HWND, message: u32, wparam: usize, lparam: isize) -> isize {
    unsafe { SendMessageW(hwnd, message, wparam, lparam) }
}

fn show(hwnd: HWND, visible: bool) {
    unsafe { ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE }) };
}

fn number(hwnd: HWND) -> Option<i32> {
    win::get_text(hwnd).trim().parse().ok()
}

fn checked(hwnd: HWND) -> bool {
    send(hwnd, BM_GETCHECK, 0, 0) == 1
}

/// A number box with up/down arrows.
fn spin_box(dialog: HWND, id: i32, font: windows_sys::Win32::Graphics::Gdi::HFONT) -> (HWND, HWND) {
    let edit = control(
        dialog,
        "EDIT",
        "",
        ES_NUMBER as u32 | ES_AUTOHSCROLL as u32 | WS_TABSTOP,
        WS_EX_CLIENTEDGE,
        id,
        font,
    );
    let arrows = control(
        dialog,
        "msctls_updown32",
        "",
        (UDS_ALIGNRIGHT | UDS_ARROWKEYS | UDS_SETBUDDYINT) as u32,
        0,
        0,
        font,
    );
    send(arrows, UDM_SETBUDDY, edit as usize, 0);
    (edit, arrows)
}

pub fn show_dialog(window: &MainWindow, fan: usize) {
    let owner = window.hwnd.get();
    let store = window.engine.borrow().store_arc();
    let lang = window.lang.borrow();
    let font = window.fonts.normal;

    // Everything the dialog needs, copied so the engine is not locked while it is open.
    let (name, fan_min, fan_max, fan_current, current_mode, preset_name, sensors) = {
        let engine = window.engine.borrow();
        let core = engine.core();
        let Some(state) = core.fans.get(fan) else {
            return;
        };
        let profile = core.profiles.current_profile();
        let preset_name = match core.profiles.current() {
            Current::Custom(index) => Some(core.profiles.custom[*index].name.clone()),
            _ => None,
        };
        let sensors: Vec<SensorChoice> = core
            .sensors
            .visible()
            .into_iter()
            .map(|index| {
                let sensor = &core.sensors.sensors()[index];
                SensorChoice {
                    key: sensor.key.clone(),
                    name: sensor.name.clone(),
                    suggested_min: sensor.suggested_min_temp(),
                    suggested_max: sensor.suggested_max_temp(),
                }
            })
            .collect();
        (
            state.name.clone(),
            state.min as i32,
            state.max as i32,
            state.current,
            profile.modes.get(fan).cloned().unwrap_or(Mode::Invalid),
            preset_name,
            sensors,
        )
    };
    let limits_equal = fan_min == fan_max;
    // Holding Ctrl+Shift while opening lets the constant speed go below the minimum.
    let unlocked = unsafe { GetKeyState(VK_CONTROL as i32) < 0 && GetKeyState(VK_SHIFT as i32) < 0 };
    let slider_min = if unlocked { 0 } else { fan_min };
    let start_mode = if current_mode == Mode::Automatic {
        LAST_APPLIED.with(|last| last.borrow().get(&fan).cloned()).unwrap_or(Mode::Automatic)
    } else {
        current_mode.clone()
    };
    // Maximum temperature must stay this far above the start temperature.
    let gap = if text::is_fahrenheit(&*store) { 14 } else { 8 };
    let display = |celsius: i32| text::to_display(&*store, celsius as f64) as i32;
    let (temp_low, temp_high) = (display(20), display(100));

    let width = 465;
    let footer_height = if limits_equal { 48 } else { 0 };
    let height = 196 + footer_height;
    let dialog = create(owner, &lang.format("FanControlDialog/title", &[&name]), width, height);

    let radio_constant = control(
        dialog,
        "BUTTON",
        lang.get("FanControlDialog/radioConstant"),
        BS_AUTORADIOBUTTON as u32 | WS_TABSTOP | WS_GROUP,
        0,
        ID_RADIO_CONSTANT,
        font,
    );
    place(radio_constant, 11, 13, 160, 20);
    let slider = control(dialog, "msctls_trackbar32", "", (TBS_HORZ | TBS_NOTICKS) as u32, 0, ID_SLIDER, font);
    place(slider, 172, 10, 222, 26);
    let spin_constant = control(
        dialog,
        "EDIT",
        "",
        ES_NUMBER as u32 | ES_AUTOHSCROLL as u32 | WS_TABSTOP,
        WS_EX_CLIENTEDGE,
        ID_SPIN_CONSTANT,
        font,
    );
    place(spin_constant, 400, 12, 54, 21);

    let radio_sensor = control(
        dialog,
        "BUTTON",
        lang.get("FanControlDialog/radioSensor"),
        BS_AUTORADIOBUTTON as u32 | WS_TABSTOP,
        0,
        ID_RADIO_SENSOR,
        font,
    );
    place(radio_sensor, 11, 44, 160, 20);
    let combo = control(dialog, "COMBOBOX", "", CBS_DROPDOWNLIST as u32 | WS_VSCROLL | WS_TABSTOP, 0, ID_COMBO, font);
    place(combo, 174, 43, 280, 300);
    for sensor in &sensors {
        send(combo, CB_ADDSTRING, 0, wide(&sensor.name).as_ptr() as isize);
    }

    let unit = text::unit(&*store);
    let label_increase = control(dialog, "STATIC", lang.get("FanControlDialog/staticIncrease"), SS_CENTERIMAGE, 0, 0, font);
    place(label_increase, 11, 76, 350, 21);
    let (spin_increase, arrows_increase) = spin_box(dialog, ID_SPIN_INCREASE, font);
    place(spin_increase, 368, 76, 58, 21);
    send(arrows_increase, UDM_SETBUDDY, spin_increase as usize, 0);
    let degree_increase = control(dialog, "STATIC", unit, SS_CENTERIMAGE, 0, 0, font);
    place(degree_increase, 432, 76, 24, 21);

    let label_max = control(dialog, "STATIC", lang.get("FanControlDialog/staticMax"), SS_CENTERIMAGE, 0, 0, font);
    place(label_max, 11, 104, 350, 21);
    let (spin_max, arrows_max) = spin_box(dialog, ID_SPIN_MAX, font);
    place(spin_max, 368, 104, 58, 21);
    send(arrows_max, UDM_SETBUDDY, spin_max as usize, 0);
    let degree_max = control(dialog, "STATIC", unit, SS_CENTERIMAGE, 0, 0, font);
    place(degree_max, 432, 104, 24, 21);

    let check_text = preset_name
        .as_ref()
        .map(|preset| lang.format("FanControlDialog/saveToCurrentPreset", &[preset]))
        .unwrap_or_default();
    let check_save = control(dialog, "BUTTON", &check_text, BS_AUTOCHECKBOX as u32 | WS_TABSTOP, 0, ID_CHECK_SAVE, font);
    place(check_save, 11, 134, width - 22, 20);
    show(check_save, preset_name.is_some());

    let ok = ok_cancel(window, dialog, width, 162);
    if limits_equal {
        let warning = lang
            .format("FanControlDialog/statusWarning", &[&fan_min.to_string()])
            .replace("<br/>", "\n");
        let footer = control(dialog, "STATIC", &warning, 0, 0, 0, font);
        place(footer, 11, 194, width - 22, 44);
    }
    drop(lang);

    // Ranges and starting values.
    send(slider, TBM_SETRANGEMIN, 0, slider_min as isize);
    send(slider, TBM_SETRANGEMAX, 1, fan_max as isize);
    let initial_rpm = match &start_mode {
        Mode::Constant(rpm) => *rpm,
        _ if fan_current != NO_RPM => fan_current as i32,
        _ => fan_min,
    }
    .clamp(slider_min, fan_max.max(slider_min));
    send(slider, TBM_SETPOS, 1, initial_rpm as isize);
    win::set_text(spin_constant, &initial_rpm.to_string());
    send(arrows_increase, UDM_SETRANGE32, temp_low as usize, temp_high as isize);
    send(arrows_max, UDM_SETRANGE32, temp_low as usize, temp_high as isize);

    let set_temps = move |start: i32, max: i32| {
        let start = start.clamp(temp_low, temp_high);
        win::set_text(spin_increase, &start.to_string());
        win::set_text(spin_max, &max.clamp(start + gap, temp_high.max(start + gap)).to_string());
        send(arrows_max, UDM_SETRANGE32, (start + gap) as usize, temp_high.max(start + gap) as isize);
    };
    let load_sensor_defaults = {
        let store = store.clone();
        let defaults: Vec<(i32, i32)> =
            sensors.iter().map(|s| (s.suggested_min, s.suggested_max)).collect();
        move |index: usize| {
            if let Some(&(min, max)) = defaults.get(index) {
                let display = |celsius: i32| text::to_display(&*store, celsius as f64) as i32;
                set_temps(display(min), display(max));
            }
        }
    };

    match &start_mode {
        Mode::Sensor { key, start, max } => {
            send(radio_sensor, BM_SETCHECK, 1, 0);
            match sensors.iter().position(|sensor| sensor.key == *key) {
                Some(index) => {
                    send(combo, CB_SETCURSEL, index, 0);
                    set_temps(display(*start), display(*max));
                }
                None => {
                    send(combo, CB_SETCURSEL, 0, 0);
                    load_sensor_defaults(0);
                }
            }
        }
        Mode::Constant(_) | Mode::Automatic => {
            send(radio_constant, BM_SETCHECK, 1, 0);
            send(combo, CB_SETCURSEL, 0, 0);
            load_sensor_defaults(0);
        }
        Mode::Invalid => {
            send(combo, CB_SETCURSEL, 0, 0);
            load_sensor_defaults(0);
        }
    }

    // Only the controls of the chosen kind of control are shown.
    let sensor_controls = [
        combo,
        label_increase,
        label_max,
        spin_increase,
        arrows_increase,
        spin_max,
        arrows_max,
        degree_increase,
        degree_max,
    ];
    let update_visibility = move || {
        let constant = checked(radio_constant);
        let sensor = checked(radio_sensor);
        show(slider, constant);
        show(spin_constant, constant);
        for control in sensor_controls {
            show(control, sensor);
        }
        unsafe {
            EnableWindow(ok, ((constant || sensor) && !limits_equal) as i32);
            if constant {
                SetFocus(spin_constant);
                send(spin_constant, EM_SETSEL, 0, -1);
            } else if sensor {
                SetFocus(combo);
            }
        }
    };
    update_visibility();

    let result: Rc<RefCell<Option<(Mode, bool)>>> = Rc::new(RefCell::new(None));
    let chosen = result.clone();
    let sensor_keys: Vec<String> = sensors.iter().map(|sensor| sensor.key.clone()).collect();
    let current_sensor_key = match &current_mode {
        Mode::Sensor { key, .. } => Some(key.clone()),
        _ => None,
    };
    let store_for_ok = store.clone();
    run_modal(
        owner,
        dialog,
        Box::new(move |hwnd, message, wparam, _| {
            if message == WM_HSCROLL {
                let rpm = send(slider, TBM_GETPOS, 0, 0);
                win::set_text(spin_constant, &rpm.to_string());
                unsafe { SetFocus(spin_constant) };
                send(spin_constant, EM_SETSEL, 0, -1);
                return Some(0);
            }
            if message != WM_COMMAND {
                return None;
            }
            let (id, code) = (win::lo_word(wparam) as i32, win::hi_word(wparam));
            match id {
                ID_RADIO_CONSTANT | ID_RADIO_SENSOR => update_visibility(),
                ID_SPIN_CONSTANT if code == EN_CHANGE => {
                    if let Some(rpm) = number(spin_constant) {
                        send(slider, TBM_SETPOS, 1, rpm.clamp(slider_min, fan_max) as isize);
                    }
                }
                ID_SPIN_CONSTANT if code == EN_KILLFOCUS => {
                    let rpm = number(spin_constant).unwrap_or(slider_min).clamp(slider_min, fan_max);
                    win::set_text(spin_constant, &rpm.to_string());
                }
                ID_COMBO if code == CBN_SELCHANGE => {
                    let index = send(combo, CB_GETCURSEL, 0, 0).max(0) as usize;
                    // Returning to the sensor already in use keeps the entered values.
                    if sensor_keys.get(index) != current_sensor_key.as_ref() {
                        load_sensor_defaults(index);
                    }
                }
                ID_SPIN_INCREASE if code == EN_KILLFOCUS => {
                    let start = number(spin_increase).unwrap_or(temp_low);
                    let max = number(spin_max).unwrap_or(temp_high);
                    set_temps(start, max);
                }
                ID_SPIN_INCREASE if code == EN_CHANGE => {
                    if let Some(start) = number(spin_increase) {
                        let low = start + gap;
                        send(arrows_max, UDM_SETRANGE32, low as usize, temp_high.max(low) as isize);
                        if number(spin_max).is_some_and(|max| max < low) && start >= temp_low {
                            win::set_text(spin_max, &low.to_string());
                        }
                    }
                }
                ID_SPIN_MAX if code == EN_KILLFOCUS => {
                    let start = number(spin_increase).unwrap_or(temp_low);
                    set_temps(start, number(spin_max).unwrap_or(temp_high));
                }
                IDOK => {
                    let mode = if checked(radio_constant) {
                        let rpm = number(spin_constant).unwrap_or(slider_min);
                        Some(Mode::Constant(rpm.clamp(slider_min, fan_max)))
                    } else if checked(radio_sensor) {
                        let index = send(combo, CB_GETCURSEL, 0, 0);
                        sensor_keys.get(index.max(0) as usize).map(|key| {
                            let start = number(spin_increase).unwrap_or(temp_low).clamp(temp_low, temp_high);
                            let max = number(spin_max).unwrap_or(temp_high).max(start + gap);
                            Mode::Sensor {
                                key: key.clone(),
                                start: text::from_display(&*store_for_ok, start),
                                max: text::from_display(&*store_for_ok, max),
                            }
                        })
                    } else {
                        None
                    };
                    if let Some(mode) = mode {
                        *chosen.borrow_mut() = Some((mode, checked(check_save)));
                        close(owner, hwnd);
                    }
                }
                IDCANCEL => close(owner, hwnd),
                _ => return None,
            }
            Some(0)
        }),
    );

    let Some((mode, save)) = result.borrow_mut().take() else {
        return;
    };
    LAST_APPLIED.with(|last| last.borrow_mut().insert(fan, mode.clone()));
    if mode != current_mode {
        let save_as = if save { preset_name.as_deref() } else { None };
        window.engine.borrow().core().set_fan_mode(fan, mode, save_as, &*store);
    }
}
