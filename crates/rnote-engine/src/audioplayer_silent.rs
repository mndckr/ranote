//! Silent `AudioPlayer` for builds without the `audio` feature.
//!
//! rodio has no Android backend. The Android build enables `ui` but not
//! `audio`, so this module keeps the engine's pen-sound call sites compiling
//! while producing no audio. `Capabilities` hides the toggle on Android.

/// No-op stand-in for the real pen-sound player.
#[derive(Debug, Default)]
pub struct AudioPlayer;

impl AudioPlayer {
    pub fn new_init(_pkg_data_dir: std::path::PathBuf) -> Result<Self, anyhow::Error> {
        Ok(Self)
    }

    pub fn play_random_marker_sound(&self) {}

    pub fn trigger_random_brush_sound(&mut self) {}

    pub fn play_typewriter_key_sound(
        &self,
        _keyboard_key: Option<rnote_compose::penevent::KeyboardKey>,
    ) {
    }
}
