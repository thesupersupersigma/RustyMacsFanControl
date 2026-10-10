//! Fan presets: one control mode per fan, the two built-in presets, custom presets,
//! and their persistence in the settings store.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

use super::mode::Mode;
use crate::settings::Store;

/// Name given to a profile that has none.
const UNSAVED_NAME: &str = "UNSAVED";
/// Internal name of the profile built from pre-preset `Fan_N` settings.
const LEGACY_NAME: &str = "old_guy";

#[derive(Clone, Debug)]
pub struct Profile {
    pub name: String,
    pub modes: Vec<Mode>,
}

/// Profiles are the same preset when their modes agree; names do not matter.
impl PartialEq for Profile {
    fn eq(&self, other: &Self) -> bool {
        self.modes == other.modes
    }
}

impl Profile {
    pub fn new(name: &str, modes: Vec<Mode>) -> Profile {
        let name = if name.is_empty() { UNSAVED_NAME.to_string() } else { name.replace('|', "-") };
        Profile { name, modes }
    }

    pub fn is_valid(&self, fan_count: usize) -> bool {
        self.modes.len() == fan_count
            && !self.name.is_empty()
            && !self.modes.contains(&Mode::Invalid)
    }

    /// Base64 of `name|mode|mode...`.
    pub fn to_setting(&self) -> String {
        let mut plain = self.name.clone();
        for mode in &self.modes {
            plain.push('|');
            plain.push_str(&mode.to_setting());
        }
        BASE64.encode(plain)
    }

    pub fn from_setting(
        text: &str,
        fan_count: usize,
        sensor_exists: &dyn Fn(&str) -> bool,
    ) -> Option<Profile> {
        let plain = BASE64.decode(text).ok().and_then(|bytes| String::from_utf8(bytes).ok());
        let parts: Vec<&str> = plain
            .as_deref()
            .unwrap_or("")
            .split('|')
            .filter(|part| !part.is_empty())
            .collect();
        if parts.len() != fan_count + 1 {
            crate::warn!("Unexpected profile string: {text}");
            return None;
        }
        let mut modes = Vec::with_capacity(fan_count);
        for part in &parts[1..] {
            let mode = Mode::from_setting(part, sensor_exists);
            if mode == Mode::Invalid {
                crate::warn!("Unexpected profile mode: {part}");
                return None;
            }
            modes.push(mode);
        }
        Some(Profile::new(parts[0], modes))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Predefined {
    Automatic = 0,
    FullBlast = 1,
}

/// Which preset is active.
#[derive(Clone, Debug, PartialEq)]
pub enum Current {
    Predefined(Predefined),
    Custom(usize),
    /// Modified modes that match no stored preset (shown as "Custom*").
    Unsaved(Profile),
}

pub struct ProfileList {
    fan_count: usize,
    automatic: Profile,
    full_blast: Profile,
    pub custom: Vec<Profile>,
    current: Current,
    /// Set whenever the active preset changes; the control loop clears it.
    pub changed: bool,
}

impl ProfileList {
    /// `max_rpms` holds each fan's maximum RPM as read at startup.
    pub fn new(max_rpms: &[u16]) -> ProfileList {
        ProfileList {
            fan_count: max_rpms.len(),
            automatic: Profile::new("Automatic", vec![Mode::Automatic; max_rpms.len()]),
            full_blast: Profile::new(
                "Full blast",
                max_rpms.iter().map(|&rpm| Mode::Constant(rpm as i32)).collect(),
            ),
            custom: Vec::new(),
            current: Current::Predefined(Predefined::Automatic),
            changed: false,
        }
    }

    pub fn predefined(&self, which: Predefined) -> &Profile {
        match which {
            Predefined::Automatic => &self.automatic,
            Predefined::FullBlast => &self.full_blast,
        }
    }

    pub fn current(&self) -> &Current {
        &self.current
    }

    pub fn current_profile(&self) -> Profile {
        match &self.current {
            Current::Predefined(which) => self.predefined(*which).clone(),
            Current::Custom(index) => self.custom[*index].clone(),
            Current::Unsaved(profile) => profile.clone(),
        }
    }

    pub fn is_automatic(&self) -> bool {
        self.current == Current::Predefined(Predefined::Automatic)
    }

    /// Loads custom presets and the active preset from the store.
    pub fn load(&mut self, store: &dyn Store, sensor_exists: &dyn Fn(&str) -> bool) {
        let saved = store.get_strings("CustomPresets");
        crate::info!("loadCustomProfiles = {}", saved.len());
        for text in &saved {
            match Profile::from_setting(text, self.fan_count, sensor_exists) {
                Some(profile) if profile.is_valid(self.fan_count) => {
                    crate::info!("Profile {} is loaded", profile.name);
                    self.custom.push(profile);
                }
                _ => crate::warn!("Failed to read profile {text}"),
            }
        }

        let mut candidate = self.automatic.clone();
        let active = store.get_string("ActivePreset").unwrap_or_default();
        crate::info!("Saved active preset: '{active}'");
        if active.is_empty() {
            if let Some(legacy) = self.legacy_profile(store, sensor_exists) {
                candidate = legacy;
            }
        } else if let Some((kind, value)) = active.split_once(':') {
            match kind {
                "Predefined" => match value.parse::<u32>() {
                    Ok(1) => candidate = self.full_blast.clone(),
                    _ => {}
                },
                "Custom" => {
                    if let Some(profile) =
                        value.parse::<usize>().ok().and_then(|index| self.custom.get(index))
                    {
                        candidate = profile.clone();
                    }
                }
                "Unsaved" => {
                    if let Some(profile) =
                        Profile::from_setting(value, self.fan_count, sensor_exists)
                            .filter(|profile| profile.is_valid(self.fan_count))
                    {
                        candidate = profile;
                    }
                }
                _ => {}
            }
        }
        self.set_current(candidate, store);
    }

    /// Per-fan `Fan_N` values written by versions that predate presets.
    fn legacy_profile(
        &self,
        store: &dyn Store,
        sensor_exists: &dyn Fn(&str) -> bool,
    ) -> Option<Profile> {
        let modes: Vec<Mode> = (0..self.fan_count)
            .map(|fan| {
                store
                    .get_string(&format!("Fan_{fan}"))
                    .filter(|text| !text.is_empty())
                    .map_or(Mode::Invalid, |text| Mode::from_setting(&text, sensor_exists))
            })
            .collect();
        let profile = Profile::new(LEGACY_NAME, modes);
        profile.is_valid(self.fan_count).then_some(profile)
    }

    /// Makes `profile` active. It becomes a stored preset when its modes equal one
    /// (built-in presets first), otherwise the unsaved profile.
    pub fn set_current(&mut self, profile: Profile, store: &dyn Store) {
        self.current = if profile == self.full_blast {
            Current::Predefined(Predefined::FullBlast)
        } else if profile == self.automatic {
            Current::Predefined(Predefined::Automatic)
        } else if let Some(index) = self.custom.iter().rposition(|custom| *custom == profile) {
            Current::Custom(index)
        } else {
            Current::Unsaved(Profile::new(&profile.name, profile.modes))
        };
        self.changed = true;
        self.save(store);
    }

    fn save(&self, store: &dyn Store) {
        let custom: Vec<String> = self.custom.iter().map(Profile::to_setting).collect();
        store.set_strings("CustomPresets", &custom);
        let active = match &self.current {
            Current::Predefined(which) => format!("Predefined:{}", *which as u32),
            Current::Custom(index) => format!("Custom:{index}"),
            Current::Unsaved(profile) => format!("Unsaved:{}", profile.to_setting()),
        };
        store.set_string("ActivePreset", &active);
    }

    /// Stores the active modes as the custom preset `name`, replacing one of that name.
    pub fn save_current_as(&mut self, name: &str, store: &dyn Store) {
        let modes = self.current_profile().modes;
        let index = match self.custom.iter().position(|custom| custom.name == name) {
            Some(index) => {
                self.custom[index].modes = modes;
                index
            }
            None => {
                self.custom.push(Profile { name: name.to_string(), modes });
                self.custom.len() - 1
            }
        };
        self.current = Current::Custom(index);
        self.save(store);
    }

    /// Renames the first custom preset whose modes equal `profile`.
    pub fn rename(&mut self, profile: &Profile, new_name: &str, store: &dyn Store) {
        if let Some(custom) = self.custom.iter_mut().find(|custom| *custom == profile) {
            custom.name = new_name.to_string();
        }
        self.save(store);
    }

    /// Deletes the first custom preset whose modes equal `profile`; when it was the
    /// active one, Automatic takes over.
    pub fn delete(&mut self, profile: &Profile, store: &dyn Store) {
        let Some(index) = self.custom.iter().position(|custom| custom == profile) else {
            return;
        };
        let active = self.current_profile();
        self.custom.remove(index);
        if active == *profile {
            self.set_current(self.automatic.clone(), store);
        } else {
            // Re-resolve so a stored custom index stays pointed at the same preset.
            self.set_current(active, store);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::MemoryStore;

    fn exists(key: &str) -> bool {
        key == "TC0P"
    }

    fn quiet() -> Profile {
        Profile::new(
            "Quiet",
            vec![Mode::Constant(1800), Mode::Sensor { key: "TC0P".into(), start: 50, max: 80 }],
        )
    }

    #[test]
    fn profile_setting_matches_the_documented_encoding() {
        assert_eq!(quiet().to_setting(), "UXVpZXR8MSwxODAwfDIsVEMwUCw1MCw4MA==");
        let parsed = Profile::from_setting(&quiet().to_setting(), 2, &exists).unwrap();
        assert_eq!(parsed.name, "Quiet");
        assert_eq!(parsed, quiet());
        assert_eq!(
            Profile::new("", vec![Mode::Automatic, Mode::Constant(2400)]).to_setting(),
            "VU5TQVZFRHwwfDEsMjQwMA=="
        );
    }

    #[test]
    fn rejects_wrong_fan_count_missing_sensor_and_garbage() {
        let text = quiet().to_setting();
        assert!(Profile::from_setting(&text, 3, &exists).is_none());
        assert!(Profile::from_setting(&text, 2, &|_| false).is_none());
        assert!(Profile::from_setting("not base64!", 2, &exists).is_none());
        assert_eq!(Profile::new("a|b", vec![]).name, "a-b");
    }

    #[test]
    fn active_preset_is_classified_and_persisted() {
        let store = MemoryStore::default();
        let mut list = ProfileList::new(&[5616, 5200]);
        list.load(&store, &exists);
        assert_eq!(store.get_string("ActivePreset").as_deref(), Some("Predefined:0"));

        let full = list.predefined(Predefined::FullBlast).clone();
        assert_eq!(full.modes, vec![Mode::Constant(5616), Mode::Constant(5200)]);
        list.set_current(full, &store);
        assert_eq!(store.get_string("ActivePreset").as_deref(), Some("Predefined:1"));

        list.set_current(quiet(), &store);
        assert!(matches!(list.current(), Current::Unsaved(_)));
        assert_eq!(
            store.get_string("ActivePreset").unwrap(),
            format!("Unsaved:{}", quiet().to_setting())
        );

        list.save_current_as("Quiet", &store);
        assert_eq!(*list.current(), Current::Custom(0));
        assert_eq!(store.get_string("ActivePreset").as_deref(), Some("Custom:0"));
        assert_eq!(store.get_strings("CustomPresets"), vec![quiet().to_setting()]);

        // a fresh list restores both the preset and the selection
        let mut reloaded = ProfileList::new(&[5616, 5200]);
        reloaded.load(&store, &exists);
        assert_eq!(*reloaded.current(), Current::Custom(0));
        assert_eq!(reloaded.current_profile().name, "Quiet");
    }

    #[test]
    fn editing_back_to_saved_modes_snaps_to_the_preset_and_delete_falls_back() {
        let store = MemoryStore::default();
        let mut list = ProfileList::new(&[5616, 5200]);
        list.set_current(quiet(), &store);
        list.save_current_as("Quiet", &store);

        let mut edited = quiet();
        edited.modes[0] = Mode::Constant(2000);
        list.set_current(edited, &store);
        assert!(matches!(list.current(), Current::Unsaved(p) if p.name == "Quiet"));
        list.set_current(quiet(), &store);
        assert_eq!(*list.current(), Current::Custom(0));

        list.rename(&quiet(), "Silent", &store);
        assert_eq!(list.custom[0].name, "Silent");
        list.delete(&quiet(), &store);
        assert!(list.custom.is_empty());
        assert!(list.is_automatic());
        assert!(store.get_strings("CustomPresets").is_empty());
    }

    #[test]
    fn legacy_per_fan_settings_become_an_unsaved_profile() {
        let store = MemoryStore::default();
        store.set_string("Fan_0", "1,2400");
        store.set_string("Fan_1", "2,TC0P,50,80");
        let mut list = ProfileList::new(&[5616, 5200]);
        list.load(&store, &exists);
        assert!(matches!(list.current(), Current::Unsaved(p) if p.name == "old_guy"));
        assert!(store.get_string("ActivePreset").unwrap().starts_with("Unsaved:"));
    }
}
