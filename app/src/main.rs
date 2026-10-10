mod lang;
mod log;
mod model;
mod registry;
mod smc;
mod win_service;

use smc::service::{FanKey, Service};

/// Read-only listing of what the SMC reports: `--probe`.
fn probe() {
    let model = model::Model::detect();
    println!("Model: {} (family {:?}, number {}, T2 {})", model.name, model.family, model.number, model.is_t2());
    let Some(provider) = smc::platform::open(&model, smc::platform::Options::default()) else {
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
            Some(value) => println!("{}  {value:.1}", smc::platform::key_text(&key)),
            None => println!("{}  -", smc::platform::key_text(&key)),
        }
    }
}

fn main() {
    if std::env::args().any(|arg| arg == "--probe") {
        probe();
        return;
    }
    let lang = lang::Language::english();
    println!("{}", lang.get("AboutDialog/title"));
}
