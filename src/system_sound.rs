//! The operating system's alert sound, played when a warning or an error
//! dialog opens.
//!
//! Each platform makes its own: Windows and macOS play the sound the user
//! chose for alerts, Linux asks the freedesktop sound theme through
//! `canberra-gtk-play` where it is installed, and the browser synthesises a
//! short tone. A sound that cannot be played is skipped without a message:
//! the dialog itself already says what went wrong.

use crate::logging::ConsoleSeverity;

/// Play the alert sound for `severity`. Info and success make no sound.
pub(crate) fn play_alert(severity: ConsoleSeverity) {
    match severity {
        ConsoleSeverity::Warn | ConsoleSeverity::Error => play(severity),
        ConsoleSeverity::Info | ConsoleSeverity::Success => {}
    }
}

#[cfg(target_os = "windows")]
fn play(severity: ConsoleSeverity) {
    use windows::Win32::{
        System::Diagnostics::Debug::MessageBeep,
        UI::WindowsAndMessaging::{MB_ICONERROR, MB_ICONWARNING},
    };

    let style = if severity == ConsoleSeverity::Error { MB_ICONERROR } else { MB_ICONWARNING };
    // SAFETY: `MessageBeep` takes a plain style value and queues the sound
    // asynchronously; it touches no memory of ours.
    let _ = unsafe { MessageBeep(style) };
}

#[cfg(target_os = "macos")]
fn play(_severity: ConsoleSeverity) {
    // AppKit has one alert sound, the one chosen in System Settings.
    objc2_app_kit::NSBeep();
}

#[cfg(target_os = "linux")]
fn play(severity: ConsoleSeverity) {
    use std::process::{Command, Stdio};

    let id = if severity == ConsoleSeverity::Error { "dialog-error" } else { "dialog-warning" };
    let Ok(mut child) = Command::new("canberra-gtk-play")
        .args(["--id", id])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    // Reap it once the sound has finished, so it leaves no zombie behind.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
}

#[cfg(target_arch = "wasm32")]
fn play(severity: ConsoleSeverity) {
    thread_local! {
        // Browsers cap how many audio contexts a page may open, so keep one.
        static CONTEXT: Option<web_sys::AudioContext> = web_sys::AudioContext::new().ok();
    }
    CONTEXT.with(|context| {
        if let Some(context) = context {
            let _ = play_tone(context, severity);
        }
    });
}

/// A short sine tone, lower for an error than for a warning, faded in and out
/// so it does not click.
#[cfg(target_arch = "wasm32")]
fn play_tone(context: &web_sys::AudioContext, severity: ConsoleSeverity) -> Result<(), wasm_bindgen::JsValue> {
    // A context made before the page saw a click or key starts suspended.
    let _ = context.resume();
    let frequency = if severity == ConsoleSeverity::Error { 330.0 } else { 660.0 };
    let start = context.current_time();
    let oscillator = context.create_oscillator()?;
    oscillator.set_type(web_sys::OscillatorType::Sine);
    oscillator.frequency().set_value(frequency);
    let gain = context.create_gain()?;
    gain.gain().set_value_at_time(0.0, start)?;
    gain.gain().linear_ramp_to_value_at_time(0.2, start + 0.01)?;
    gain.gain().exponential_ramp_to_value_at_time(0.0001, start + 0.25)?;
    oscillator.connect_with_audio_node(&gain)?;
    gain.connect_with_audio_node(&context.destination())?;
    oscillator.start_with_when(start)?;
    oscillator.stop_with_when(start + 0.3)?;
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux", target_arch = "wasm32")))]
fn play(_severity: ConsoleSeverity) {}
