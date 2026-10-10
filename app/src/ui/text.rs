//! Text the main window, tray and dialogs share.

use crate::fan::mode::Mode;
use crate::lang::Language;
use crate::settings::Store;

pub const APP_NAME: &str = "Rusty Macs Fan Control";
pub const PROJECT_URL: &str = "https://github.com/thesupersupersigma/RustyMacsFanControl";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn app_title() -> String {
    format!("{APP_NAME} {APP_VERSION}")
}

/// Describes a fan's control mode. `sensor_name` resolves a sensor key to its display
/// name; the short form is used in the tray menu.
pub fn mode_text(
    lang: &Language,
    mode: &Mode,
    short: bool,
    sensor_name: &dyn Fn(&str) -> String,
) -> String {
    match mode {
        Mode::Invalid => "n/a".to_string(),
        Mode::Automatic => lang.get("MainWindow/fanControlAutomatic").to_string(),
        Mode::Constant(rpm) => lang.format("MainWindow/fanControlConstant", &[&rpm.to_string()]),
        Mode::Sensor { .. } if short => lang.get("TrayMenu/itemSensorBased").to_string(),
        Mode::Sensor { key, .. } => lang.format("MainWindow/fanControlSensor", &[&sensor_name(key)]),
    }
}

pub fn is_fahrenheit(store: &dyn Store) -> bool {
    store.get_bool("Fahrenheit", false)
}

pub fn unit(store: &dyn Store) -> &'static str {
    if is_fahrenheit(store) {
        "°F"
    } else {
        "°C"
    }
}

/// Celsius to the display scale.
pub fn to_display(store: &dyn Store, celsius: f64) -> f64 {
    if is_fahrenheit(store) {
        celsius * 9.0 / 5.0 + 32.0
    } else {
        celsius
    }
}

/// Whole degrees of the display scale back to Celsius.
pub fn from_display(store: &dyn Store, value: i32) -> i32 {
    if is_fahrenheit(store) {
        ((value - 32) as f64 * 5.0 / 9.0).round() as i32
    } else {
        value
    }
}

/// A sensor reading as shown in the list: no unit, `-` when unread.
pub fn temperature_text(store: &dyn Store, celsius: f64) -> String {
    if celsius == 0.0 {
        "-".to_string()
    } else if celsius < 0.0 {
        String::new()
    } else if store.get_bool("PreciseTemperature", false) {
        format!("{:.1}", to_display(store, celsius))
    } else {
        format!("{}", to_display(store, celsius).round() as i64)
    }
}

/// Converts `[label](url)` to the `<a href>` form the link control understands.
pub fn markdown_links(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let Some(mid) = rest[open..].find("](").map(|i| open + i) else {
            break;
        };
        let Some(close) = rest[mid..].find(')').map(|i| mid + i) else {
            break;
        };
        out.push_str(&rest[..open]);
        out.push_str(&format!("<a href=\"{}\">{}</a>", &rest[mid + 2..close], &rest[open + 1..mid]));
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::MemoryStore;

    #[test]
    fn temperature_formatting_follows_the_settings() {
        let store = MemoryStore::default();
        assert_eq!(temperature_text(&store, 0.0), "-");
        assert_eq!(temperature_text(&store, -1.0), "");
        assert_eq!(temperature_text(&store, 45.5), "46");
        assert_eq!(temperature_text(&store, 45.4), "45");
        store.set_bool("PreciseTemperature", true);
        assert_eq!(temperature_text(&store, 45.44), "45.4");
        store.set_bool("Fahrenheit", true);
        assert_eq!(temperature_text(&store, 50.0), "122.0");
        assert_eq!(unit(&store), "°F");
        assert_eq!(from_display(&store, 122), 50);
    }

    #[test]
    fn mode_descriptions() {
        let lang = Language::english();
        let name = |key: &str| format!("<{key}>");
        let sensor = Mode::Sensor { key: "TC0P".into(), start: 50, max: 80 };
        assert_eq!(mode_text(&lang, &Mode::Automatic, false, &name), "Auto");
        assert_eq!(mode_text(&lang, &Mode::Constant(2400), true, &name), "Constant value of 2400");
        assert_eq!(mode_text(&lang, &sensor, false, &name), "Based on <TC0P>");
        assert_eq!(mode_text(&lang, &sensor, true, &name), "Sensor-based");
    }

    #[test]
    fn converts_markdown_links() {
        assert_eq!(
            markdown_links("try to [reset the SMC](https://example.com/x) now"),
            "try to <a href=\"https://example.com/x\">reset the SMC</a> now"
        );
        assert_eq!(markdown_links("no links"), "no links");
    }
}
