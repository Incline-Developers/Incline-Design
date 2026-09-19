//! UI zoom supplements native OS/browser density scaling; screen resolution
//! and window size do not determine the size of text and controls.

pub(super) fn zoom_factor(size_percent: f64) -> f32 {
    // egui-winit supplies native_pixels_per_point separately. Leave that intact
    // so high-density displays render more pixels per logical UI point.
    (size_percent / 100.0) as f32
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
