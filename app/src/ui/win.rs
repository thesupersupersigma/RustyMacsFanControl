//! Small Win32 helpers shared by the windows and dialogs.

use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateDIBSection, CreateFontW, DeleteObject, GetDC, GetTextExtentPoint32W, ReleaseDC,
    SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CLEARTYPE_QUALITY, DEFAULT_CHARSET,
    DIB_RGB_COLORS, FW_BOLD, FW_NORMAL, HBITMAP, HDC, HFONT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, CreateWindowExW, GetClientRect, GetWindowTextLengthW, GetWindowTextW,
    SendMessageW, SetWindowTextW, HICON, HMENU, ICONINFO, WM_SETFONT, WS_CHILD, WS_VISIBLE,
};

pub use crate::registry::wide;

// Static control styles and a few messages the bindings file under other modules.
pub const SS_CENTER: u32 = 0x1;
pub const SS_RIGHT: u32 = 0x2;
pub const SS_BITMAP: u32 = 0xE;
pub const SS_ETCHEDHORZ: u32 = 0x10;
pub const SS_CENTERIMAGE: u32 = 0x200;
pub const TBM_GETPOS: u32 = 0x400;
/// A toolbar button that stays pressed and releases its siblings.
pub const BTNS_CHECKGROUP: u32 = 0x6;

pub fn instance() -> HINSTANCE {
    unsafe { GetModuleHandleW(null()) }
}

/// Whole-number UI scale: 125% draws at 1x, 150% and 175% at 2x.
pub fn scale() -> i32 {
    let dpi = unsafe { GetDpiForSystem() } as f64;
    ((dpi / 96.0).round() as i32).max(1)
}

/// Logical pixels to device pixels.
pub fn px(value: i32) -> i32 {
    value * scale()
}

pub fn lo_word(value: usize) -> u32 {
    (value & 0xFFFF) as u32
}

pub fn hi_word(value: usize) -> u32 {
    ((value >> 16) & 0xFFFF) as u32
}

pub fn x_of(lparam: LPARAM) -> i32 {
    (lparam & 0xFFFF) as i16 as i32
}

pub fn y_of(lparam: LPARAM) -> i32 {
    ((lparam >> 16) & 0xFFFF) as i16 as i32
}

pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | (g as u32) << 8 | (b as u32) << 16
}

pub struct Fonts {
    pub normal: HFONT,
    pub bold: HFONT,
    /// One step larger and bold: the current RPM.
    pub big_bold: HFONT,
    /// One point smaller: the drives separator row.
    pub small: HFONT,
}

fn font(point_size: i32, weight: i32) -> HFONT {
    // Points to pixels at 96 dpi, times the UI scale.
    let height = -(point_size * 96 * scale() + 36) / 72;
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("Segoe UI").as_ptr(),
        )
    }
}

impl Fonts {
    pub fn new() -> Fonts {
        Fonts {
            normal: font(9, FW_NORMAL as i32),
            bold: font(9, FW_BOLD as i32),
            big_bold: font(11, FW_BOLD as i32),
            small: font(8, FW_NORMAL as i32),
        }
    }
}

pub fn text_width(hwnd: HWND, font: HFONT, text: &str) -> i32 {
    unsafe {
        let dc = GetDC(hwnd);
        let width = text_width_dc(dc, font, text);
        ReleaseDC(hwnd, dc);
        width
    }
}

pub fn text_width_dc(dc: HDC, font: HFONT, text: &str) -> i32 {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut size = SIZE { cx: 0, cy: 0 };
    unsafe {
        let old = SelectObject(dc, font);
        GetTextExtentPoint32W(dc, units.as_ptr(), units.len() as i32, &mut size);
        SelectObject(dc, old);
    }
    size.cx
}

pub fn client_rect(hwnd: HWND) -> RECT {
    let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { GetClientRect(hwnd, &mut rect) };
    rect
}

pub fn set_text(hwnd: HWND, text: &str) {
    unsafe { SetWindowTextW(hwnd, wide(text).as_ptr()) };
}

pub fn get_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        let mut buf = vec![0u16; len as usize + 1];
        let read = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        String::from_utf16_lossy(&buf[..read as usize])
    }
}

/// Creates a visible child control with the given font.
pub fn control(
    parent: HWND,
    class: &str,
    text: &str,
    style: u32,
    ex_style: u32,
    id: i32,
    font: HFONT,
) -> HWND {
    unsafe {
        let hwnd = CreateWindowExW(
            ex_style,
            wide(class).as_ptr(),
            wide(text).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            0,
            0,
            parent,
            id as isize as HMENU,
            instance(),
            null(),
        );
        SendMessageW(hwnd, WM_SETFONT, font as WPARAM, 1);
        hwnd
    }
}

/// A decoded PNG as premultiplied BGRA, top-down.
pub struct Image {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u8>,
}

impl Image {
    pub fn from_png(data: &[u8]) -> Option<Image> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
        decoder.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
        let mut reader = decoder.read_info().ok()?;
        let mut buf = vec![0u8; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut buf).ok()?;
        let channels = info.color_type.samples();
        let mut pixels = Vec::with_capacity(info.width as usize * info.height as usize * 4);
        for chunk in buf[..info.buffer_size()].chunks_exact(channels) {
            let (r, g, b, a) = match channels {
                4 => (chunk[0], chunk[1], chunk[2], chunk[3]),
                2 => (chunk[0], chunk[0], chunk[0], chunk[1]),
                _ => return None,
            };
            let mul = |c: u8| (c as u32 * a as u32 / 255) as u8;
            pixels.extend_from_slice(&[mul(b), mul(g), mul(r), a]);
        }
        Some(Image { width: info.width as i32, height: info.height as i32, pixels })
    }

    /// 32-bit bitmap with alpha, suitable for image lists and AlphaBlend.
    pub fn to_bitmap(&self) -> HBITMAP {
        bitmap_from_bgra(self.width, self.height, &self.pixels)
    }
}

pub fn bitmap_from_bgra(width: i32, height: i32, pixels: &[u8]) -> HBITMAP {
    let mut info: BITMAPINFO = unsafe { std::mem::zeroed() };
    info.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        biHeight: -height,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB as u32,
        ..unsafe { std::mem::zeroed() }
    };
    let mut bits: *mut core::ffi::c_void = null_mut();
    unsafe {
        let bitmap = CreateDIBSection(null_mut(), &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if !bitmap.is_null() && !bits.is_null() {
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u8>(), pixels.len());
        }
        bitmap
    }
}

/// Builds an icon from a 32-bit bitmap with alpha.
pub fn icon_from_bitmap(color: HBITMAP, width: i32, height: i32) -> HICON {
    let mask = bitmap_from_bgra(width, height, &vec![0u8; (width * height * 4) as usize]);
    let info = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
    unsafe {
        let icon = CreateIconIndirect(&info);
        DeleteObject(mask);
        icon
    }
}

macro_rules! icon_assets {
    ($($name:literal),* $(,)?) => {
        /// PNG bytes of an embedded icon; `big` selects the 2x variant.
        pub fn icon_png(name: &str, big: bool) -> Option<&'static [u8]> {
            match (name, big) {
                $(
                    ($name, false) => Some(include_bytes!(concat!("../../assets/icons/", $name, ".png"))),
                    ($name, true) => Some(include_bytes!(concat!("../../assets/icons/", $name, "@2x.png"))),
                )*
                _ => None,
            }
        }
    };
}

icon_assets!(
    "airport", "christmas_snowflake", "disc", "disc_big", "drive", "drive_big", "drive_external",
    "drive_nvme", "drive_ssd", "fan", "fan_big", "gpu", "icon_128", "power-supply",
    "preferences_general", "preferences_menubar", "preferences_sensors", "processor",
    "processor_big", "profiles_action", "tray", "warning",
);

/// Loads an embedded icon at the current UI scale.
pub fn load_image(name: &str) -> Option<Image> {
    Image::from_png(icon_png(name, scale() >= 2)?)
}
