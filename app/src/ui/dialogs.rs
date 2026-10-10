//! Modal dialog plumbing, plus the small dialogs: preset name and About.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ptr::{null, null_mut};
use std::rc::Rc;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{DeleteObject, GetSysColorBrush, COLOR_BTNFACE};
use windows_sys::Win32::UI::Controls::{EM_SETLIMITTEXT, NMHDR, NMLINK, NM_CLICK, NM_RETURN};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetMessageW, GetWindowRect, IsDialogMessageW, IsWindow, LoadCursorW, LoadIconW, MoveWindow,
    PostQuitMessage, RegisterClassExW, SendMessageW, SetForegroundWindow, ShowWindow,
    TranslateMessage, BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CBN_EDITCHANGE, CBN_SELCHANGE,
    CBS_AUTOHSCROLL, CBS_DROPDOWN, CB_ADDSTRING, CB_GETCURSEL, CB_GETLBTEXT, CB_GETLBTEXTLEN,
    EN_CHANGE, ES_AUTOHSCROLL, IDCANCEL, IDC_ARROW, IDOK,
    IMAGE_BITMAP, MSG, STM_SETIMAGE, SW_SHOWNORMAL,
    WM_CLOSE, WM_COMMAND, WM_NOTIFY, WM_QUIT, WNDCLASSEXW, WS_CAPTION, WS_EX_CLIENTEDGE,
    WS_EX_DLGMODALFRAME, WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VSCROLL,
};

use super::main_window::MainWindow;
use super::text;
use super::win::{self, control, px, wide, SS_BITMAP, SS_CENTER, SS_CENTERIMAGE};

const DIALOG_CLASS: &str = "RustyMacsFanControlDialog";

/// Handles a dialog's messages; returning `None` leaves the message to the default.
pub type Handler = Box<dyn FnMut(HWND, u32, WPARAM, LPARAM) -> Option<LRESULT>>;

thread_local! {
    static HANDLERS: RefCell<HashMap<isize, Rc<RefCell<Handler>>>> = RefCell::new(HashMap::new());
    static REGISTERED: RefCell<bool> = const { RefCell::new(false) };
}

unsafe extern "system" fn dialog_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let handler = HANDLERS.with(|h| h.borrow().get(&(hwnd as isize)).cloned());
    if let Some(handler) = handler {
        // A handler that is already running (it sent a message itself) is not re-entered.
        if let Ok(mut handler) = handler.try_borrow_mut() {
            if let Some(result) = handler(hwnd, message, wparam, lparam) {
                return result;
            }
        }
    }
    if message == WM_CLOSE {
        DestroyWindow(hwnd);
        return 0;
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

/// Creates a fixed-size dialog window centred on its owner. The client area is given in
/// logical pixels.
pub fn create(owner: HWND, title: &str, client_width: i32, client_height: i32) -> HWND {
    unsafe {
        let class_name = wide(DIALOG_CLASS);
        REGISTERED.with(|registered| {
            if !registered.replace(true) {
                let class = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    lpfnWndProc: Some(dialog_proc),
                    hInstance: win::instance(),
                    hIcon: LoadIconW(win::instance(), 1 as _),
                    hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                    hbrBackground: GetSysColorBrush(COLOR_BTNFACE),
                    lpszClassName: class_name.as_ptr(),
                    ..std::mem::zeroed()
                };
                RegisterClassExW(&class);
            }
        });
        let style = WS_POPUP | WS_CAPTION | WS_SYSMENU;
        let mut rect = RECT { left: 0, top: 0, right: px(client_width), bottom: px(client_height) };
        AdjustWindowRectEx(&mut rect, style, 0, WS_EX_DLGMODALFRAME);
        let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
        let mut owner_rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetWindowRect(owner, &mut owner_rect);
        let x = owner_rect.left + ((owner_rect.right - owner_rect.left) - width) / 2;
        let y = owner_rect.top + ((owner_rect.bottom - owner_rect.top) - height) / 2;
        CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            class_name.as_ptr(),
            wide(title).as_ptr(),
            style,
            x.max(0),
            y.max(0),
            width,
            height,
            owner,
            null_mut(),
            win::instance(),
            null(),
        )
    }
}

/// Changes a dialog's client size (logical pixels), keeping its position.
pub fn resize(dialog: HWND, client_width: i32, client_height: i32) {
    unsafe {
        let mut rect = RECT { left: 0, top: 0, right: px(client_width), bottom: px(client_height) };
        AdjustWindowRectEx(&mut rect, WS_POPUP | WS_CAPTION | WS_SYSMENU, 0, WS_EX_DLGMODALFRAME);
        let mut current = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        GetWindowRect(dialog, &mut current);
        MoveWindow(dialog, current.left, current.top, rect.right - rect.left, rect.bottom - rect.top, 1);
    }
}

/// Shows `dialog` and blocks, with the owner disabled, until it is destroyed.
pub fn run_modal(owner: HWND, dialog: HWND, handler: Handler) {
    HANDLERS.with(|h| h.borrow_mut().insert(dialog as isize, Rc::new(RefCell::new(handler))));
    unsafe {
        EnableWindow(owner, 0);
        ShowWindow(dialog, SW_SHOWNORMAL);
        let mut message: MSG = std::mem::zeroed();
        while IsWindow(dialog) != 0 {
            let got = GetMessageW(&mut message, null_mut(), 0, 0);
            if got <= 0 {
                // Pass the quit request on to the main loop.
                if message.message == WM_QUIT {
                    PostQuitMessage(message.wParam as i32);
                }
                break;
            }
            if IsDialogMessageW(dialog, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        EnableWindow(owner, 1);
        SetForegroundWindow(owner);
    }
    HANDLERS.with(|h| h.borrow_mut().remove(&(dialog as isize)));
}

/// Closes a dialog, re-enabling its owner first so focus returns to it.
pub fn close(owner: HWND, dialog: HWND) {
    unsafe {
        EnableWindow(owner, 1);
        DestroyWindow(dialog);
    }
}

pub fn place(hwnd: HWND, x: i32, y: i32, width: i32, height: i32) {
    unsafe { MoveWindow(hwnd, px(x), px(y), px(width), px(height), 1) };
}

pub fn open_url(owner: HWND, url: &str) {
    unsafe {
        ShellExecuteW(owner, wide("open").as_ptr(), wide(url).as_ptr(), null(), null(), SW_SHOWNORMAL);
    }
}

/// OK and Cancel buttons, right aligned on a row at `y`.
pub fn ok_cancel(window: &MainWindow, dialog: HWND, client_width: i32, y: i32) -> HWND {
    let lang = window.lang.borrow();
    let font = window.fonts.normal;
    let ok = control(dialog, "BUTTON", lang.get("Common/buttonOK"), BS_DEFPUSHBUTTON as u32 | WS_TABSTOP, 0, IDOK, font);
    let cancel = control(dialog, "BUTTON", lang.get("Common/buttonCancel"), BS_PUSHBUTTON as u32 | WS_TABSTOP, 0, IDCANCEL, font);
    place(ok, client_width - 11 - 75 - 6 - 75, y, 75, 23);
    place(cancel, client_width - 11 - 75, y, 75, 23);
    ok
}

fn combo_item(combo: HWND, index: usize) -> String {
    unsafe {
        let len = SendMessageW(combo, CB_GETLBTEXTLEN, index, 0);
        if len < 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        SendMessageW(combo, CB_GETLBTEXT, index, buf.as_mut_ptr() as isize);
        String::from_utf16_lossy(&buf[..len as usize])
    }
}

/// Asks for a preset name. Saving offers the existing custom presets to overwrite;
/// renaming requires a name that differs from the current one and from every other.
pub fn preset_name(window: &MainWindow, initial: &str, renaming: bool) -> Option<String> {
    let owner = window.hwnd.get();
    let title = window.lang.borrow().get(if renaming {
        "Presets/editPresetTitle"
    } else {
        "Presets/savePresetTitle"
    })
    .to_string();
    let existing: Vec<String> =
        window.engine.borrow().core().profiles.custom.iter().map(|p| p.name.clone()).collect();
    let font = window.fonts.normal;
    let (width, height) = (300, 100);
    let dialog = create(owner, &title, width, height);

    let heading = control(dialog, "STATIC", &title, SS_CENTER, 0, 0, font);
    place(heading, 11, 9, width - 22, 16);
    let label_text = window.lang.borrow().get("Presets/labelPresetName").to_string();
    let label_width = win::text_width(dialog, font, &label_text) / win::scale() + 4;
    let label = control(dialog, "STATIC", &label_text, SS_CENTERIMAGE, 0, 0, font);
    place(label, 11, 33, label_width, 21);
    let input_x = 11 + label_width + 6;
    let input = if renaming {
        let edit = control(dialog, "EDIT", initial, ES_AUTOHSCROLL as u32 | WS_TABSTOP, WS_EX_CLIENTEDGE, 10, font);
        unsafe { SendMessageW(edit, EM_SETLIMITTEXT, 30, 0) };
        place(edit, input_x, 33, width - 11 - input_x, 21);
        edit
    } else {
        let combo = control(
            dialog,
            "COMBOBOX",
            "",
            CBS_DROPDOWN as u32 | CBS_AUTOHSCROLL as u32 | WS_VSCROLL | WS_TABSTOP,
            0,
            10,
            font,
        );
        for name in &existing {
            unsafe { SendMessageW(combo, CB_ADDSTRING, 0, wide(name).as_ptr() as isize) };
        }
        win::set_text(combo, initial);
        place(combo, input_x, 33, width - 11 - input_x, 200);
        combo
    };
    let ok = ok_cancel(window, dialog, width, height - 11 - 23);

    let original = initial.to_string();
    let validate = {
        let (existing, original) = (existing.clone(), original.clone());
        move |name: &str| {
            if name.is_empty() {
                return false;
            }
            if renaming {
                let taken = existing.iter().any(|other| other.to_lowercase() == name.to_lowercase());
                return name != original && !taken;
            }
            true
        }
    };
    unsafe {
        EnableWindow(ok, validate(initial) as i32);
        SetFocus(input);
    }

    let result = Rc::new(RefCell::new(None));
    let chosen = result.clone();
    run_modal(
        owner,
        dialog,
        Box::new(move |hwnd, message, wparam, _| {
            if message != WM_COMMAND {
                return None;
            }
            let (id, code) = (win::lo_word(wparam) as i32, win::hi_word(wparam));
            match id {
                10 if code == EN_CHANGE || code == CBN_EDITCHANGE => {
                    unsafe { EnableWindow(ok, validate(&win::get_text(input)) as i32) };
                }
                10 if code == CBN_SELCHANGE => {
                    let selected = unsafe { SendMessageW(input, CB_GETCURSEL, 0, 0) };
                    if selected >= 0 {
                        let name = combo_item(input, selected as usize);
                        unsafe { EnableWindow(ok, validate(&name) as i32) };
                    }
                }
                IDOK => {
                    let name = win::get_text(input);
                    if validate(&name) {
                        *chosen.borrow_mut() = Some(name);
                        close(owner, hwnd);
                    }
                }
                IDCANCEL => close(owner, hwnd),
                _ => return None,
            }
            Some(0)
        }),
    );
    let name = result.borrow_mut().take();
    name
}

/// The About box.
pub fn about(window: &MainWindow) {
    let owner = window.hwnd.get();
    let lang = window.lang.borrow();
    let font = window.fonts.normal;
    let (width, height) = (430, 300);
    let dialog = create(owner, lang.get("AboutDialog/title"), width, height);

    let picture = control(dialog, "STATIC", "", SS_BITMAP | SS_CENTERIMAGE, 0, 0, font);
    let bitmap = win::load_image("icon_128").map(|image| image.to_bitmap());
    if let Some(bitmap) = bitmap {
        unsafe { SendMessageW(picture, STM_SETIMAGE, IMAGE_BITMAP as usize, bitmap as isize) };
    }
    place(picture, 11, 11, 128, 128);

    let column_x = 11 + 128 + 6;
    let column_width = width - column_x - 11;
    let name = control(dialog, "STATIC", &text::app_title(), SS_CENTER, 0, 0, window.fonts.bold);
    place(name, column_x, 36, column_width, 16);
    let note = control(
        dialog,
        "STATIC",
        "Rust reimplementation of Macs Fan Control",
        SS_CENTER,
        0,
        0,
        font,
    );
    place(note, column_x, 58, column_width, 16);
    let credit = control(dialog, "STATIC", "Original app © CrystalIDEA Software", SS_CENTER, 0, 0, font);
    place(credit, column_x, 80, column_width, 16);
    let link_text = format!("<a href=\"help\">{}</a>", lang.get("AboutDialog/staticOnlineHelp"));
    let link_width = win::text_width(dialog, font, lang.get("AboutDialog/staticOnlineHelp")) / win::scale() + 6;
    let link = control(dialog, "SysLink", &link_text, WS_TABSTOP, 0, 20, font);
    place(link, column_x + (column_width - link_width) / 2, 102, link_width, 16);

    let disclaimer = control(dialog, "STATIC", lang.get("AboutDialog/staticWarning"), 0, 0, 0, font);
    place(disclaimer, 11, 150, width - 22, 104);

    let close_button = control(
        dialog,
        "BUTTON",
        lang.get("Common/buttonClose"),
        BS_DEFPUSHBUTTON as u32 | WS_TABSTOP,
        0,
        IDCANCEL,
        font,
    );
    place(close_button, width - 11 - 75, height - 11 - 23, 75, 23);
    unsafe { SetFocus(close_button) };
    drop(lang);

    run_modal(
        owner,
        dialog,
        Box::new(move |hwnd, message, wparam, lparam| match message {
            WM_COMMAND if matches!(win::lo_word(wparam) as i32, IDOK | IDCANCEL) => {
                close(owner, hwnd);
                Some(0)
            }
            WM_NOTIFY => {
                let header = unsafe { &*(lparam as *const NMHDR) };
                if header.idFrom == 20 && (header.code == NM_CLICK || header.code == NM_RETURN) {
                    let _ = unsafe { &*(lparam as *const NMLINK) };
                    open_url(hwnd, "https://crystalidea.com/macs-fan-control/support");
                    return Some(0);
                }
                None
            }
            _ => None,
        }),
    );
    if let Some(bitmap) = bitmap {
        unsafe { DeleteObject(bitmap) };
    }
}
