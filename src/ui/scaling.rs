//! Monitor-relative UI sizing. Resizing the window does not resize controls.

use winit::window::Window;

pub(super) fn zoom_for_window(window: &Window, size_percent: f64) -> f32 {
    zoom_factor(monitor_size(window), window.scale_factor() as f32, size_percent)
}

#[cfg(not(target_arch = "wasm32"))]
fn monitor_size(window: &Window) -> Option<[u32; 2]> {
    window.current_monitor().or_else(|| window.primary_monitor()).map(|monitor| monitor.size().into())
}

#[cfg(target_arch = "wasm32")]
fn monitor_size(window: &Window) -> Option<[u32; 2]> {
    // Browser screen dimensions are CSS pixels, while the reference is physical
    // pixels. Use the same device scale as the canvas/winit integration.
    let screen = web_sys::window()?.screen().ok()?;
    let scale = window.scale_factor();
    let width = screen.width().ok()?;
    let height = screen.height().ok()?;
    (width > 0 && height > 0).then(|| [(f64::from(width) * scale).round() as u32, (f64::from(height) * scale).round() as u32])
}

fn zoom_factor(monitor_size: Option<[u32; 2]>, native_scale: f32, size_percent: f64) -> f32 {
    let zoom = monitor_size
        .filter(|size| !size.contains(&0))
        .map(|size| {
            let fit = (size[0] as f32 / 2560.0).min(size[1] as f32 / 1440.0);
            // egui already multiplies zoom by native DPI. Avoid applying it twice
            // to a physical-pixel reference. Without monitor information, retain
            // ordinary OS scaling rather than falling back to window dimensions.
            fit / native_scale
        })
        .unwrap_or(1.0);
    zoom * (size_percent / 100.0) as f32
}

/// Events queued by egui-winit used the previous zoom. Keep their physical
/// positions when changing zoom before consuming the frame's input.
pub(super) fn rescale_events(events: &mut [egui::Event], ratio: f32) {
    for event in events {
        match event {
            egui::Event::PointerMoved(pos) | egui::Event::PointerButton { pos, .. } | egui::Event::Touch { pos, .. } => *pos *= ratio,
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta,
                ..
            } => *delta *= ratio,
            _ => {}
        }
    }
}
