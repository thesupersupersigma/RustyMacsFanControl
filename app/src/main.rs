// Release builds are GUI programs; debug builds keep a console for log output.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod engine;
mod fan;
mod lang;
mod log;
mod model;
mod registry;
mod sensors;
mod settings;
mod smc;
mod ui;
mod win_service;

use std::ptr::null_mut;
use std::sync::Arc;

use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, MessageBoxW, PostMessageW, MB_ICONWARNING, MB_OK,
};

use engine::{Core, Engine};
use model::Model;
use smc::platform::{self, Options, Provider};
use smc::service::{FanKey, Service};
use ui::win::wide;

/// Read-only listing of what the SMC reports: `--probe`.
fn probe() {
    let model = Model::detect();
    println!(
        "Model: {} (family {:?}, number {}, T2 {})",
        model.name,
        model.family,
        model.number,
        model.is_t2()
    );
    let Some(provider) = platform::open(&model, Options::default()) else {
        println!("Could not open the SMC (run elevated; the AppleSMC driver service must exist).");
        return;
    };
    let service = Service::new(provider, &model);
    println!("Provider: {}, {} SMC keys", service.provider_name(), service.key_count());
    let modes = service.fan_control_modes();
    for fan in 0..service.fan_count() {
        let rpm = |kind| service.fan_rpm(fan, kind).map_or("?".to_string(), |v| v.to_string());
        println!(
            "Fan {fan} '{}': min {} / current {} / max {} / target {} / forced {:?}",
            service.fan_name(fan),
            rpm(FanKey::Minimum),
            rpm(FanKey::Actual),
            rpm(FanKey::Maximum),
            rpm(FanKey::Target),
            modes.as_ref().and_then(|m| m.get(fan)),
        );
    }
    for key in service.temperature_keys() {
        match service.temperature(&key) {
            Some(value) => println!("{}  {value:.1}", platform::key_text(&key)),
            None => println!("{}  -", platform::key_text(&key)),
        }
    }
}

fn message_box(text: &str) {
    unsafe {
        MessageBoxW(
            null_mut(),
            wide(text).as_ptr(),
            wide(&ui::text::app_title()).as_ptr(),
            MB_OK | MB_ICONWARNING,
        );
    }
}

fn is_virtual_machine() -> bool {
    registry::read_machine_string(
        r"SYSTEM\CurrentControlSet\Control\SystemInformation",
        "SystemManufacturer",
    )
    .map(|maker| maker.to_lowercase())
    .is_some_and(|maker| maker.contains("parallels") || maker.contains("vmware"))
}

/// True when another instance is running; that instance is asked to show its window.
fn already_running() -> bool {
    unsafe {
        CreateMutexW(std::ptr::null(), 0, wide(r"Local\RustyMacsFanControl").as_ptr());
        if GetLastError() != ERROR_ALREADY_EXISTS {
            return false;
        }
        let existing = FindWindowW(wide("RustyMacsFanControlMainWindow").as_ptr(), std::ptr::null());
        if !existing.is_null() {
            PostMessageW(existing, ui::main_window::WM_RESTORE, 0, 0);
        }
        true
    }
}

fn open_provider(model: &Model, options: Options, demo: bool) -> Option<Box<dyn Provider>> {
    #[cfg(debug_assertions)]
    if demo {
        return Some(Box::new(smc::fake::demo()));
    }
    let _ = demo;
    platform::open(model, options)
}

/// Debug builds can open a dialog straight away (`--show=fan`), for checking layouts.
fn initial_action(args: &[String]) -> Option<ui::main_window::Action> {
    use ui::main_window::Action;
    if !cfg!(debug_assertions) {
        return None;
    }
    match args.iter().find_map(|arg| arg.strip_prefix("--show="))? {
        "fan" => Some(Action::FanControl(0)),
        "preferences" => Some(Action::Preferences),
        "about" => Some(Action::About),
        "save" => Some(Action::SaveUnsaved),
        _ => None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |switch: &str| args.iter().any(|arg| arg == switch);
    if has("--probe") {
        probe();
        return;
    }
    if has("/log") {
        if let Ok(exe) = std::env::current_exe() {
            let path = format!("{}.log", exe.to_string_lossy());
            if let Ok(file) = std::fs::File::create(path) {
                log::set_file(file);
            }
        }
    } else if let Ok(file) = std::fs::File::create(std::env::temp_dir().join("RustyMacsFanControl.log")) {
        log::set_file(file);
    }
    info!("{}", ui::text::app_title());

    if already_running() {
        return;
    }
    let demo = has("--demo");
    let Some(store) = settings::open() else {
        message_box("Unable to open the settings registry key.");
        return;
    };
    let mut model = Model::detect();
    if demo && model.family == model::Family::Unknown {
        model = Model::parse("MacBookPro16,1");
    }
    info!("Model: {}", model.name);
    if !has("/skipcheck") && !demo {
        if is_virtual_machine() {
            if !settings::Store::get_bool(&store, "NoVmWarning", false) {
                message_box("Virtual machine detected\n\nUnable to start Macs Fan Control in a virtual machine.");
            }
            return;
        }
        if model.family == model::Family::Unknown {
            message_box(
                "Unsupported hardware detected. The program is able to run on Apple hardware only. \n\nIf you're sure you run the app on Apple computer, please use the /skipcheck command line switch.",
            );
            return;
        }
    }

    let options = Options { force_apple_smc: has("/applesmc") };
    let Some(provider) = open_provider(&model, options, demo) else {
        message_box(
            "Error initializing SMC interaction layer\n\nPlease make sure that your Mac system is supported and you have correctly installed the program.",
        );
        return;
    };
    let service = Arc::new(Service::new(provider, &model));
    let language_file = settings::Store::get_string(&store, "InterfaceLanguage").unwrap_or_default();
    let lang = ui::preferences::load_language(&language_file);
    let core = Core::load(service, model, &store);
    let engine = Engine::new(core, Arc::new(store));
    ui::main_window::run(engine, lang, has("/minimized"), initial_action(&args));
}
