use std::{fmt::Debug, hash::Hash};

use crate::{
    i18n::{tr, tr_format},
    model::block_model::{BlockModelSlice, Boundary, ColorTransferFunction, MAX_GRADIENT_ENTRIES, OpenBlockModel, color_variable_default, render_value_range},
    ui::{
        state::{EditorState, SectionGridAxis, SectionGridLineKind, UiCommand},
        widgets::menu,
    },
};

/// A compact tool panel pinned inside the 3D viewport.
///
/// Apears in the bottom left. Used for tool configuration.
pub(crate) struct ViewportDockPanel {
    id: egui::Id,
    title: egui::WidgetText,
    viewport_rect: egui::Rect,
    min_width: f32,
    max_width: f32,
    margin: egui::Vec2,
}

impl ViewportDockPanel {
    pub(crate) fn new(id_source: impl Hash + Debug, title: impl Into<egui::WidgetText>, viewport_rect: egui::Rect) -> Self {
        Self {
            id: egui::Id::new(id_source),
            title: title.into().fallback_text_style(egui::TextStyle::Button),
            viewport_rect,
            min_width: 0.0,
            max_width: 320.0,
            margin: egui::vec2(10.0, 10.0),
        }
    }

    pub(crate) fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = min_width;
        self
    }

    pub(crate) fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = max_width;
        self
    }

    pub(crate) fn show<R>(self, ctx: &egui::Context, add_contents: impl FnOnce(&mut egui::Ui) -> R) {
        let pos = egui::pos2(self.viewport_rect.left() + self.margin.x, self.viewport_rect.bottom() - self.margin.y);
        egui::Area::new(self.id)
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::LEFT_BOTTOM)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                // The same card as a floating menu, minus the drag bar's close
                // button: a docked tool panel is one of the menu family, and
                // the fields inside it are the menu fields.
                let surface = menu::menu_surface(ui.visuals());
                egui::Frame::new()
                    .fill(surface)
                    .stroke(menu::menu_border(ui.visuals()))
                    .corner_radius(egui::CornerRadius::same(menu::MENU_CORNER_RADIUS))
                    .show(ui, |ui| {
                        menu::apply_menu_style(ui, surface);
                        let inner_margin = egui::Margin::symmetric(10, 8);
                        let measured_width = menu::intrinsic_content_width(ui) + inner_margin.sum().x;
                        let available_width = (self.viewport_rect.width() - self.margin.x * 2.0).max(1.0);
                        let adaptive_min_width = self.min_width.max(measured_width).min(available_width);
                        ui.set_min_width(adaptive_min_width);
                        ui.set_max_width(self.max_width.max(adaptive_min_width));
                        let title_rect = ui.allocate_exact_size(egui::vec2(adaptive_min_width, menu::TITLE_BAR_HEIGHT), egui::Sense::hover()).0;
                        let inner = egui::Frame::NONE.inner_margin(inner_margin).show(ui, add_contents).inner;
                        let mut title_rect = title_rect;
                        title_rect.max.x = ui.min_rect().right();
                        menu::draw_menu_heading(ui, &self.title, title_rect, surface);
                        inner
                    })
                    .inner
            });
    }
}

/// The small "Reset" button in the Slice and Colour-mapping section headers.
///
/// Sized to its own label with tight padding and `Extend` wrap - the properties
/// panel sets a global `Truncate` that would otherwise clip it to "Re…".
fn reset_section_button(ui: &mut egui::Ui, tooltip: impl Into<String>) -> bool {
    let tooltip = tooltip.into();
    ui.scope(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.spacing_mut().button_padding = egui::vec2(6.0, 2.0);
        ui.add(egui::Button::new(egui::RichText::new(tr!(literal = "Reset")).small()))
            .on_hover_text(tooltip)
            .clicked()
    })
    .inner
}

/// A colormap as the ramp widget manipulates it.
///
/// The widget is a bar, so dragging along it is naturally a `0..1` position,
/// while a [`Boundary`] stores an absolute data value. Both are carried, and
/// `value` is only recomputed when a handle actually moves - so recolouring a
/// band whose boundary sits outside the current render range (an imported
/// colormap authored wider than its data) does not quietly clamp it into range.
///
/// OMF's `gradient[0]` - the band under the first boundary - is not editable
/// here: the widget always writes [`BELOW_FIRST_BOUNDARY`], so the first
/// boundary acts as a grade cutoff that hides everything beneath it.
struct UiRamp {
    stops: Vec<UiStop>,
    interpolate: bool,
}

#[derive(Clone, Copy)]
struct UiStop {
    id: u64,
    /// Position along the bar, `0..1`, clamped from `value`.
    t: f32,
    /// Absolute value in the variable's data units.
    value: f64,
    /// See [`Boundary::inclusive`].
    inclusive: bool,
    /// The colour of the band at or above this boundary - `gradient[i + 1]`.
    color: [f32; 4],
}

/// The colour of everything below the first boundary. Always transparent, so
/// blocks under the cutoff are not drawn.
const BELOW_FIRST_BOUNDARY: [f32; 4] = [0.0; 4];

impl UiRamp {
    /// Project a colormap onto the bar. A continuous colormap's evenly spaced
    /// gradient becomes evenly spaced handles, so both kinds edit identically;
    /// `to_transfer` puts each back into its own shape.
    fn from_transfer(transfer: &ColorTransferFunction, min: f64, max: f64) -> Self {
        match transfer {
            // Categories are drawn by `draw_category_legend`, never here.
            ColorTransferFunction::Category { .. } => Self {
                stops: Vec::new(),
                interpolate: false,
            },
            ColorTransferFunction::Continuous { range, gradient } => {
                let (low, high) = *range;
                let last = gradient.len().saturating_sub(1).max(1);
                Self {
                    stops: gradient
                        .iter()
                        .enumerate()
                        .map(|(index, &color)| {
                            let value = low + (high - low) * index as f64 / last as f64;
                            UiStop {
                                id: index as u64 + 1,
                                t: value_to_normalized(value, min, max),
                                value,
                                inclusive: false,
                                color,
                            }
                        })
                        .collect(),
                    interpolate: true,
                }
            }
            ColorTransferFunction::Discrete { boundaries, gradient } => Self {
                stops: boundaries
                    .iter()
                    .enumerate()
                    .map(|(index, boundary)| UiStop {
                        id: boundary.id,
                        t: value_to_normalized(boundary.value, min, max),
                        value: boundary.value,
                        inclusive: boundary.inclusive,
                        color: gradient.get(index + 1).copied().unwrap_or([0.0; 4]),
                    })
                    .collect(),
                interpolate: false,
            },
        }
    }

    fn to_transfer(&self) -> ColorTransferFunction {
        if self.interpolate {
            // A continuous gradient is evenly spaced by definition, so the
            // handles only get to set the span; their colours are resampled
            // onto an even grid across it.
            let (low, high) = (self.stops.first().map_or(0.0, |stop| stop.value), self.stops.last().map_or(1.0, |stop| stop.value));
            let samples = self.stops.len().max(2);
            let span = high - low;
            return ColorTransferFunction::Continuous {
                range: (low, high),
                gradient: (0..samples).map(|index| self.sample(low + span * index as f64 / (samples - 1) as f64)).collect(),
            };
        }
        ColorTransferFunction::Discrete {
            boundaries: self
                .stops
                .iter()
                .map(|stop| Boundary {
                    id: stop.id,
                    value: stop.value,
                    inclusive: stop.inclusive,
                })
                .collect(),
            gradient: std::iter::once(BELOW_FIRST_BOUNDARY).chain(self.stops.iter().map(|stop| stop.color)).collect(),
        }
    }

    /// Linear interpolation across the handles, clamped at both ends. Only used
    /// when resampling a continuous gradient.
    fn sample(&self, value: f64) -> [f32; 4] {
        match self.stops.iter().position(|stop| stop.value > value) {
            None => self.stops.last().map_or(BELOW_FIRST_BOUNDARY, |stop| stop.color),
            Some(0) => self.stops[0].color,
            Some(upper) => {
                let (low, high) = (self.stops[upper - 1], self.stops[upper]);
                let span = high.value - low.value;
                let f = if span.abs() <= f64::EPSILON { 0.0 } else { (value - low.value) / span };
                crate::model::block_model::lerp_rgba(low.color, high.color, f as f32)
            }
        }
    }

    /// The colour the bar shows at `t`, mirroring `ramp_rgba`.
    fn color_at_t(&self, t: f32) -> egui::Color32 {
        let Some(first) = self.stops.first() else {
            return color32_from_straight(BELOW_FIRST_BOUNDARY);
        };
        if t < first.t {
            return color32_from_straight(BELOW_FIRST_BOUNDARY);
        }
        let last = self.stops[self.stops.len() - 1];
        if t >= last.t {
            return color32_from_straight(last.color);
        }
        for pair in self.stops.windows(2) {
            if t < pair[1].t {
                if !self.interpolate {
                    return color32_from_straight(pair[0].color);
                }
                let span = pair[1].t - pair[0].t;
                let f = if span <= f32::EPSILON { 0.0 } else { (t - pair[0].t) / span };
                return color32_from_straight(crate::model::block_model::lerp_rgba(pair[0].color, pair[1].color, f));
            }
        }
        color32_from_straight(last.color)
    }
}

impl UiStop {
    fn set_t(&mut self, t: f32, min: f64, max: f64) {
        self.t = t;
        self.value = normalized_to_value(t, min, max);
    }
}

/// Minimum gap (in normalized `t`) enforced between adjacent colour-transfer
/// stops when dragging, so segments never collapse to zero width.
const STOP_EPSILON: f32 = 0.01;
/// Side of a boundary handle's square hit target.
const COLOR_STOP_HANDLE_SIZE: f32 = 18.0;
const COLOR_PICKER_BUTTON_WIDTH: f32 = 40.0;
const COLOR_PICKER_BUTTON_HEIGHT: f32 = 18.0;
/// Height reserved for the horizontal ramp, labels and colour picker.
const LEGEND_BAR_HEIGHT: f32 = 112.0;
const LEGEND_BAR_THICKNESS: f32 = 16.0;
/// Column drawn left of each boundary handle: the boundary's value in the
/// variable's own units (an editable number box once clicked), then the `≤`
/// marker when the boundary is inclusive.
const LEGEND_STOP_VALUE_WIDTH: f32 = 46.0;
const LEGEND_LABEL_FRACTIONS: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
/// Width of the per-category share column drawn left of each legend swatch.
const LEGEND_CATEGORY_PERCENT_WIDTH: f32 = 34.0;
const SCALE_BAR_TARGET_WIDTH: f64 = 320.0;
const SCALE_BAR_VIEWPORT_MARGIN: f32 = 10.0;
const SCALE_BAR_LABEL_OVERHANG: f32 = 18.0;
const SCALE_BAR_SEGMENT_FRACTIONS: [f64; 6] = [0.0, 0.05, 0.10, 0.25, 0.50, 1.0];
/// Height of the scale bar's block: the bar itself and the labels under it.
const SCALE_BAR_HEIGHT: f32 = 21.0;
/// Gap between the embedded slice preview and the viewport's edges.
const SLICE_PREVIEW_MARGIN: f32 = 10.0;
/// Drop from the top of the viewport to the embedded slice preview when the
/// orientation gizmo it normally hangs below is switched off.
const SLICE_PREVIEW_TOP: f32 = 10.0;
/// Bounds on the embedded slice preview's side. It tracks the viewport's
/// shorter side between them, so a small viewport still gets a usable preview
/// and a large one is not handed most of the scene as a minimap.
const SLICE_PREVIEW_MIN_SIZE: f32 = 160.0;
const SLICE_PREVIEW_MAX_SIZE: f32 = 320.0;
/// Clearance between a section-grid label and the end of the line it names.
const SECTION_GRID_LABEL_GAP: f32 = 4.0;

/// Clamps `raw_t` to `0..1` and, if that lands within `STOP_EPSILON` of an
/// existing stop, nudges it just outside that stop's epsilon band.
///
/// Without this, a double-click meant to insert a new stop near (or, after
/// edge-clamping, exactly on top of) an existing one produces a stop whose
/// `t` is within the later dedup pass's `1e-4` tolerance of the existing
/// stop - so the new stop is silently collapsed back out and insertion
/// appears to do nothing. This is most visible at the bar's edges: clamping
/// an overshot click to exactly `0.0`/`1.0` collides with a stop already
/// sitting at that exact edge (the common case for default colour ramps).
fn nudge_away_from_existing(stops: &[UiStop], raw_t: f32) -> f32 {
    let mut t = raw_t.clamp(0.0, 1.0);
    for _ in 0..=stops.len() {
        let Some(collision) = stops.iter().find(|stop| (stop.t - t).abs() < STOP_EPSILON) else {
            return t;
        };
        t = if collision.t >= t {
            (collision.t - STOP_EPSILON).max(0.0)
        } else {
            (collision.t + STOP_EPSILON).min(1.0)
        };
    }
    t
}

/// Inserts a new stop at `t` into an already-sorted `stops` vec, keeping it
/// sorted, and returns the index it landed at.
///
/// Inserting in sorted order (rather than appending and re-sorting later) is
/// what keeps positional-neighbour logic correct on the very frame of
/// insertion: the value popup and drag clamps derive a stop's allowed range
/// from `stops[i-1]`/`stops[i+1]`, so a freshly-appended stop parked at the
/// end of the vec would be treated as the right-most one and clamped to the
/// old last stop's position.
fn insert_stop_sorted(ramp: &mut UiRamp, id: u64, t: f32, min: f64, max: f64) -> usize {
    let index = ramp.stops.partition_point(|stop| stop.t < t);
    // A new boundary splits the band it lands in, so both halves start out the
    // colour that band already had.
    let color = color32_to_straight(ramp.color_at_t(t));
    ramp.stops.insert(
        index,
        UiStop {
            id,
            t,
            value: normalized_to_value(t, min, max),
            inclusive: false,
            color,
        },
    );
    index
}

fn color32_from_straight(c: [f32; 4]) -> egui::Color32 {
    let [r, g, b, a] = straight_to_unmultiplied_srgba(c);
    egui::Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn color32_to_straight(c: egui::Color32) -> [f32; 4] {
    unmultiplied_srgba_to_straight(c.to_srgba_unmultiplied())
}

/// Colour-stop RGB is linear: the scene renders into an sRGB surface view (see
/// `rendering/graphics/init.rs`), so the shader emits linear and the GPU
/// encodes it. egui works in sRGB bytes, so the swatch and the picker have to
/// go through the transfer function rather than scaling by 255 - otherwise a
/// legend swatch is visibly darker than the blocks it describes, and a colour
/// picked in the legend comes back brighter than the one chosen. Alpha is
/// linear in both, so it only scales.
fn straight_to_unmultiplied_srgba(c: [f32; 4]) -> [u8; 4] {
    let [r, g, b] = [c[0], c[1], c[2]].map(crate::rendering::color::linear_to_srgb_byte);
    [r, g, b, (c[3].clamp(0.0, 1.0) * 255.0).round() as u8]
}

fn unmultiplied_srgba_to_straight(c: [u8; 4]) -> [f32; 4] {
    let mut straight = crate::rendering::color::rgb_bytes_to_linear_rgba([c[0], c[1], c[2]]);
    straight[3] = f32::from(c[3]) / 255.0;
    straight
}

fn trim_decimal_zeros(mut value: String) -> String {
    if let Some(dot) = value.find('.') {
        while value.ends_with('0') {
            value.pop();
        }
        if value.len() == dot + 1 {
            value.pop();
        }
    }
    if value == "-0" { "0".to_owned() } else { value }
}

/// Formats a grade value with a decimal precision that scales down as the
/// magnitude grows, so labels stay short without losing meaningful digits.
fn format_grade(value: f64) -> String {
    let decimals = if value.abs() >= 1000.0 {
        0
    } else if value.abs() >= 10.0 {
        1
    } else {
        3
    };
    trim_decimal_zeros(format!("{value:.decimals$}"))
}

fn inferred_decimal_places(value: f64) -> usize {
    for decimals in 0..=4 {
        let scale = 10_f64.powi(decimals as i32);
        let rounded = (value * scale).round() / scale;
        let tolerance = 1e-8 * value.abs().max(1.0);
        if (rounded - value).abs() <= tolerance {
            return decimals;
        }
    }
    4
}

fn format_grade_range(min: f64, max: f64) -> String {
    let decimals = inferred_decimal_places(min).max(inferred_decimal_places(max));
    format!("({min:.decimals$} - {max:.decimals$})")
}

/// The interactive colour-scale and slice editor for one block model, drawn
/// in the horizontal viewport filter panel.
pub(crate) struct BlockModelProperties<'a> {
    id: egui::Id,
    model: &'a OpenBlockModel,
}

impl<'a> BlockModelProperties<'a> {
    pub(crate) fn new(id_source: impl Hash + Debug, model: &'a OpenBlockModel) -> Self {
        Self {
            id: egui::Id::new(id_source),
            model,
        }
    }

    pub(crate) fn show(self, ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
        let model = self.model;
        let content_width = 260.0;
        let range = active_variable_range(editor, model);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(content_width);
                self.draw_slice_controls(ui, content_width, model, commands);
            });
            if !model_has_selectable_variable(model) {
                return;
            }
            ui.separator();
            ui.vertical(|ui| {
                let content_width = 460.0;
                ui.set_width(content_width);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(tr!(literal = "Colour mapping")).strong().color(ui.visuals().weak_text_color()));
                    if reset_section_button(ui, tr!(literal = "Rebuild this variable's colours from its data")) {
                        commands.push(UiCommand::ResetBlockModelColorTransfer { id: model.id });
                    }
                });
                self.draw_variable_dropdown(ui, content_width, model, editor, commands);
                if model.active_variable_is_categorical() {
                    self.draw_category_legend(ui, content_width, model, commands);
                } else if let Some((min, max)) = range {
                    self.draw_bar(ui, content_width, min, max, model, editor, commands);
                } else {
                    self.draw_no_data(ui, content_width);
                }
            });
        });
    }

    /// One row per axis, so the six bounds fit a side panel's width.
    fn draw_slice_controls(&self, ui: &mut egui::Ui, content_width: f32, model: &OpenBlockModel, commands: &mut Vec<UiCommand>) {
        let (lower, upper) = model.local_bounds();
        let full = BlockModelSlice { min: lower, max: upper };
        let mut slice = model.slice.unwrap_or(full).clamped_to(lower, upper);
        let mut changed = false;

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(tr!(literal = "Slice")).strong().color(ui.visuals().weak_text_color()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if reset_section_button(ui, tr!(literal = "Restore the full model range")) {
                    commands.push(UiCommand::SetBlockModelSlice { id: model.id, slice: None });
                }
            });
        });

        let axis_label_width = 14.0;
        let gap = ui.spacing().item_spacing.x;
        let value_width = ((content_width - axis_label_width - gap * 2.0) * 0.5).max(48.0);
        for (axis, label) in crate::model::survey::axis_names().into_iter().enumerate() {
            let extent = (upper[axis] - lower[axis]).abs();
            let speed = (extent / 500.0).max(0.001);
            ui.horizontal(|ui| {
                ui.add_sized(egui::vec2(axis_label_width, 20.0), egui::Label::new(egui::RichText::new(label.clone()).strong()));
                changed |= ui
                    .add_sized(
                        egui::vec2(value_width, 20.0),
                        egui::DragValue::new(&mut slice.min[axis]).range(lower[axis]..=slice.max[axis]).speed(speed).max_decimals(4),
                    )
                    .on_hover_text(tr_format!(literal = "%axis% minimum", axis = label))
                    .changed();
                changed |= ui
                    .add_sized(
                        egui::vec2(value_width, 20.0),
                        egui::DragValue::new(&mut slice.max[axis]).range(slice.min[axis]..=upper[axis]).speed(speed).max_decimals(4),
                    )
                    .on_hover_text(tr_format!(literal = "%axis% maximum", axis = label))
                    .changed();
            });
        }

        if changed {
            commands.push(UiCommand::SetBlockModelSlice { id: model.id, slice: Some(slice) });
        }
    }

    fn draw_variable_dropdown(&self, ui: &mut egui::Ui, content_width: f32, model: &OpenBlockModel, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
        let current = model.active_color_variable.as_deref().unwrap_or("");
        let selected_text = model
            .active_color_variable
            .as_deref()
            .map(|name| {
                if let Some(variable) = model.model.variable(name)
                    && is_categorical_variable(variable)
                {
                    format!("{name} ({})", format_category_count(variable))
                } else if let Some((min, max)) = cached_variable_range(editor, model, name) {
                    format!("{name} {}", format_grade_range(min, max))
                } else {
                    tr_format!(literal = "%name% (no range)", name = name)
                }
            })
            .unwrap_or_else(|| tr!(literal = "Choose a variable"));
        let filter_id = self.id.with(("variable_filter", model.id));
        let mut filter = ui.data_mut(|data| data.get_persisted::<String>(filter_id)).unwrap_or_default();

        let popup_id = self.id.with(("variable_popup", model.id));
        let open = egui::Popup::is_id_open(ui.ctx(), popup_id);
        let button_response = ui
            .add_sized(egui::vec2(content_width, 22.0), egui::Button::selectable(open, egui::RichText::new(selected_text).strong()))
            .on_hover_text(tr!(literal = "Choose the active block model variable"));

        let _ = egui::Popup::menu(&button_response)
            .id(popup_id)
            .width(content_width)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut filter)
                        .hint_text(tr!(literal = "Filter variables"))
                        .desired_width(content_width - 12.0),
                );
                if response.changed() {
                    ui.data_mut(|data| data.insert_persisted(filter_id, filter.clone()));
                }
                ui.add_space(4.0);

                let needle = filter.trim().to_ascii_lowercase();
                let mut any = false;
                egui::ScrollArea::vertical().max_height(220.0).auto_shrink([false, true]).show(ui, |ui| {
                    for variable in model.model.color_variables().into_iter().filter(|variable| !variable.special) {
                        let name = variable.name.as_str();
                        if !needle.is_empty() && !name.to_ascii_lowercase().contains(&needle) {
                            continue;
                        }
                        any = true;
                        let range_text = if is_categorical_variable(variable) {
                            format_category_count(variable)
                        } else {
                            cached_variable_range(editor, model, name)
                                .map(|(min, max)| format_grade_range(min, max))
                                .unwrap_or_else(|| tr!(literal = "(no usable range)"))
                        };
                        let selected = name == current;
                        let row = ui
                            .horizontal(|ui| {
                                let response = ui.selectable_label(selected, egui::RichText::new(name).strong());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(egui::RichText::new(range_text).color(ui.visuals().weak_text_color()));
                                });
                                response
                            })
                            .inner;
                        if row.clicked() {
                            commands.push(UiCommand::SetBlockModelColorVariable {
                                id: model.id,
                                variable: name.to_owned(),
                            });
                            egui::Popup::close_id(ui.ctx(), popup_id);
                        }
                    }
                });
                if !any {
                    ui.label(egui::RichText::new(tr!(literal = "No matches")).color(ui.visuals().weak_text_color()));
                }
            });
    }

    fn draw_category_legend(&self, ui: &mut egui::Ui, content_width: f32, model: &OpenBlockModel, commands: &mut Vec<UiCommand>) {
        let Some(variable) = model.active_color_variable.as_deref().and_then(|name| model.model.variable(name)) else {
            return;
        };
        // A category attribute's colours are a plain per-name list in OMF, so
        // the legend edits that list directly - there are no boundaries to
        // place and no cutoff band, and every category the file names can be
        // coloured.
        let ColorTransferFunction::Category { gradient } = model.color_transfer() else {
            return;
        };
        let default = color_variable_default(variable);
        let mut gradient = gradient.clone();
        let mut changed = false;
        ui.allocate_ui_with_layout(egui::vec2(content_width, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            for (&code, label) in &variable.strings {
                let is_default = default == Some(code as f64);
                if model.active_category_code_present(code) == Some(false) {
                    continue;
                }
                let color = gradient.get(&code).copied().unwrap_or([0.72, 0.72, 0.75, 1.0]);
                let percent_text = model.active_category_code_fraction(code).map(format_category_percent).unwrap_or_default();
                ui.push_id(("category_color", code), |ui| {
                    ui.horizontal(|ui| {
                        // Share of the renderable blocks in this category, in a
                        // fixed column so the names still line up beneath it.
                        ui.allocate_ui_with_layout(
                            egui::vec2(LEGEND_CATEGORY_PERCENT_WIDTH, ui.spacing().interact_size.y),
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.label(egui::RichText::new(&percent_text).color(ui.visuals().weak_text_color()));
                            },
                        );
                        if !is_default || !model.hide_empty_color_values {
                            let mut srgba = straight_to_unmultiplied_srgba(color);
                            if super::color::edit_srgba_unmultiplied(ui, &mut srgba)
                                .on_hover_text(if is_default {
                                    tr!(literal = "Edit the colour used for empty values")
                                } else {
                                    tr!(literal = "Edit this category colour")
                                })
                                .changed()
                            {
                                gradient.insert(code, unmultiplied_srgba_to_straight(srgba));
                                changed = true;
                            }
                        }
                        let display_label = if label.trim().is_empty() { tr!(literal = "(blank)") } else { label.clone() };
                        let suffix = if is_default && model.hide_empty_color_values {
                            tr!(literal = " (empty · hidden)")
                        } else if is_default {
                            tr!(literal = " (empty)")
                        } else {
                            String::new()
                        };
                        ui.label(format!("{display_label}{suffix}"));
                    });
                });
            }
        });
        if variable.strings.len() >= MAX_GRADIENT_ENTRIES {
            ui.label(
                egui::RichText::new(tr_format!(
                    literal = "All %total% categories keep their colour; only the first %shown% are drawn distinctly",
                    total = variable.strings.len(),
                    shown = MAX_GRADIENT_ENTRIES - 1
                ))
                .color(ui.visuals().weak_text_color()),
            );
        }
        if changed {
            commands.push(UiCommand::SetBlockModelColorTransfer {
                id: model.id,
                transfer: ColorTransferFunction::Category { gradient },
            });
        }
    }

    fn draw_no_data(&self, ui: &mut egui::Ui, content_width: f32) {
        // Must match `draw_bar`'s allocation so switching to or from a
        // variable with no usable range doesn't resize the legend.
        let (rect, _response) = ui.allocate_exact_size(egui::vec2(content_width, LEGEND_BAR_HEIGHT), egui::Sense::hover());
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            tr!(literal = "No data for this variable"),
            egui::FontId::proportional(11.0),
            ui.visuals().weak_text_color(),
        );
    }

    /// The ramp runs from minimum on the left to maximum on the right.
    #[allow(clippy::too_many_arguments)]
    fn draw_bar(&self, ui: &mut egui::Ui, content_width: f32, min: f64, max: f64, model: &OpenBlockModel, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
        let text_color = ui.visuals().text_color();
        // Lay out scale labels before painting the horizontal ramp.
        let label_font = egui::FontId::proportional(11.0);
        let labels: Vec<_> = LEGEND_LABEL_FRACTIONS
            .iter()
            .map(|&t| {
                (
                    t,
                    ui.painter().layout_no_wrap(format_grade(normalized_to_value(t, min, max)), label_font.clone(), text_color),
                )
            })
            .collect();
        let (rect, _response) = ui.allocate_exact_size(egui::vec2(content_width, LEGEND_BAR_HEIGHT), egui::Sense::hover());
        let bar_rect = egui::Rect::from_min_max(rect.min + egui::vec2(28.0, 36.0), egui::pos2(rect.right() - 28.0, rect.top() + 36.0 + LEGEND_BAR_THICKNESS));
        let handle_center_y = bar_rect.top() - COLOR_STOP_HANDLE_SIZE * 0.5;
        let x_at = |t: f32| bar_rect.left() + bar_rect.width() * t;
        let t_at = |x: f32| (x - bar_rect.left()) / bar_rect.width().max(1.0);

        let mut ramp = UiRamp::from_transfer(model.color_transfer(), min, max);
        let mut changed = false;
        let mut remove_index = None;
        let selected_id = self.id.with(("selected_color_stop", model.id));
        let mut selected = ui
            .data_mut(|data| data.get_persisted::<usize>(selected_id))
            .filter(|index| *index < ramp.stops.len())
            .unwrap_or(0);
        let value_popup_id = self.id.with(("stop_value_popup_open", model.id));
        let mut value_popup_stop = ui
            .data_mut(|data| data.get_persisted::<Option<usize>>(value_popup_id))
            .flatten()
            .filter(|index| *index < ramp.stops.len());
        // Set whenever a stop interaction this frame opens/keeps the value
        // popup, so the click-outside close below doesn't immediately undo it
        // (clicking a handle is "outside" the popup's own rect).
        let mut popup_kept_open = false;

        {
            let painter = ui.painter();
            super::color::paint_alpha_checker(painter, bar_rect);
            const STRIPS: usize = 96;
            let strip_width = bar_rect.width() / STRIPS as f32;
            for i in 0..STRIPS {
                let t = i as f32 / (STRIPS - 1) as f32;
                let strip_rect = egui::Rect::from_min_size(
                    egui::pos2(bar_rect.left() + i as f32 * strip_width, bar_rect.top()),
                    egui::vec2(strip_width + 0.5, bar_rect.height()),
                );
                painter.rect_filled(strip_rect, 0.0, ramp.color_at_t(t));
            }
            painter.rect_stroke(bar_rect, 0.0, egui::Stroke::new(1.0, egui::Color32::from_gray(40)), egui::StrokeKind::Outside);
        }

        let bar_response = ui
            .interact(bar_rect, self.id.with("color_stop_bar"), egui::Sense::click())
            .on_hover_text(tr!(literal = "Double-click to add a boundary here"));
        // A double-click on either the bar or a handle requests an insert.
        // We record the target `t` and apply it *after* the handle loop so the
        // insertion never shifts indices mid-iteration, and so it lands in
        // sorted position (see `insert_stop_sorted`).
        let mut pending_insert: Option<f32> = None;
        if bar_response.double_clicked()
            && ramp.stops.len() < MAX_GRADIENT_ENTRIES - 1
            && let Some(pos) = bar_response.interact_pointer_pos()
        {
            pending_insert = Some(nudge_away_from_existing(&ramp.stops, t_at(pos.x)));
        }

        for i in 0..ramp.stops.len() {
            let x = x_at(ramp.stops[i].t);
            let handle_rect = egui::Rect::from_center_size(egui::pos2(x, handle_center_y), egui::vec2(COLOR_STOP_HANDLE_SIZE, COLOR_STOP_HANDLE_SIZE));
            let handle_id = self.id.with(("color_stop_handle", ramp.stops[i].id));
            let response = ui.interact(handle_rect, handle_id, egui::Sense::click_and_drag()).on_hover_text(if ramp.stops.len() > 1 {
                tr!(literal = "Drag to move · Right-click to remove · Middle-click toggles ≤")
            } else {
                tr!(literal = "Drag to move · Middle-click toggles ≤")
            });
            // Handles sit directly beside the gradient strip, so a
            // double-click aimed at the bar near an existing stop (most
            // often the last one, at the visually prominent top edge)
            // lands on the handle instead. Without this, that click is
            // swallowed as an ordinary single click that just reselects the
            // handle, and no new stop is ever added.
            if response.double_clicked()
                && ramp.stops.len() < MAX_GRADIENT_ENTRIES - 1
                && let Some(pos) = response.interact_pointer_pos()
            {
                pending_insert = Some(nudge_away_from_existing(&ramp.stops, t_at(pos.x)));
            } else {
                if response.secondary_clicked() && ramp.stops.len() > 1 {
                    remove_index = Some(i);
                }
                // Inclusiveness is an OMF boundary property with no natural
                // drag gesture; middle-click flips it in place.
                if response.middle_clicked() && !ramp.interpolate {
                    ramp.stops[i].inclusive = !ramp.stops[i].inclusive;
                    changed = true;
                }
                if response.clicked() {
                    selected = i;
                    value_popup_stop = Some(i);
                    popup_kept_open = true;
                }
                if response.dragged() {
                    let lower = if i == 0 { 0.0 } else { ramp.stops[i - 1].t + STOP_EPSILON };
                    let upper = if i + 1 == ramp.stops.len() { 1.0 } else { ramp.stops[i + 1].t - STOP_EPSILON };
                    let (lo, hi) = (lower.min(upper), lower.max(upper));
                    if let Some(pos) = response.interact_pointer_pos() {
                        let t = t_at(pos.x).clamp(lo, hi);
                        if (ramp.stops[i].t - t).abs() > f32::EPSILON {
                            ramp.stops[i].set_t(t, min, max);
                            changed = true;
                        }
                    }
                    selected = i;
                    if value_popup_stop.is_some() {
                        value_popup_stop = Some(i);
                        popup_kept_open = true;
                    }
                }
            }
            // The active boundary value sits above its handle and can be typed.
            let value_rect = egui::Rect::from_center_size(
                egui::pos2(
                    x.clamp(rect.left() + LEGEND_STOP_VALUE_WIDTH * 0.5, rect.right() - LEGEND_STOP_VALUE_WIDTH * 0.5),
                    rect.top() + 10.0,
                ),
                egui::vec2(LEGEND_STOP_VALUE_WIDTH, ui.spacing().interact_size.y.min(18.0)),
            );
            let editing_value = value_popup_stop == Some(i);
            let value_field_hovered;
            if editing_value {
                let gap_t = 1.0e-4_f32;
                let lower_t = if i == 0 { 0.0 } else { ramp.stops[i - 1].t + gap_t };
                let upper_t = if i + 1 == ramp.stops.len() { 1.0 } else { ramp.stops[i + 1].t - gap_t };
                let (lo_t, hi_t) = (lower_t.min(upper_t), lower_t.max(upper_t));
                let (lo_v, hi_v) = (normalized_to_value(lo_t, min, max), normalized_to_value(hi_t, min, max));
                let mut value = ramp.stops[i].value;
                let field = ui
                    .scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(value_rect)
                            .layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)),
                        |ui| {
                            // The column is narrow; trim the number box's padding so
                            // a few significant digits fit without clipping.
                            ui.spacing_mut().button_padding = egui::vec2(4.0, 2.0);
                            ui.add(
                                egui::DragValue::new(&mut value)
                                    .range(lo_v.min(hi_v)..=lo_v.max(hi_v))
                                    .speed(((max - min).abs() / 250.0).max(0.0001))
                                    .max_decimals(6),
                            )
                        },
                    )
                    .inner;
                if field.changed() {
                    ramp.stops[i].set_t(value_to_normalized(value, min, max).clamp(lo_t, hi_t), min, max);
                    changed = true;
                }
                value_field_hovered = field.hovered();
                if !popup_kept_open && (field.lost_focus() || field.clicked_elsewhere()) {
                    value_popup_stop = None;
                } else {
                    selected = i;
                }
            } else {
                let label = ui
                    .interact(value_rect, self.id.with(("color_stop_value_label", ramp.stops[i].id)), egui::Sense::click())
                    .on_hover_cursor(egui::CursorIcon::Text)
                    .on_hover_text(tr!(literal = "Click to type this boundary's value"));
                value_field_hovered = label.hovered();
                if label.clicked() {
                    selected = i;
                    value_popup_stop = Some(i);
                    popup_kept_open = true;
                }
            }

            let painter = ui.painter();
            let marker_color = color32_from_straight(ramp.stops[i].color);
            let center = egui::pos2(x, handle_center_y);
            let active = selected == i || response.dragged() || response.hovered() || value_field_hovered;
            let radius = if active { 5.5 } else { 4.5 };
            painter.line_segment(
                [egui::pos2(x, center.y + radius), egui::pos2(x, bar_rect.top())],
                egui::Stroke::new(if active { 1.5 } else { 1.0 }, ui.visuals().widgets.noninteractive.bg_stroke.color),
            );
            painter.circle_filled(center, radius, marker_color);
            painter.circle_stroke(
                center,
                radius,
                egui::Stroke::new(if active { 1.5 } else { 1.0 }, if active { egui::Color32::BLACK } else { egui::Color32::from_gray(40) }),
            );
            // Each boundary's value in the variable's own units, so every stop
            // shows what it is without having to be clicked open. Skipped while
            // the in-place field for this stop is showing.
            if !editing_value && active {
                painter.text(
                    value_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format_grade(ramp.stops[i].value),
                    egui::FontId::proportional(10.0),
                    if active { text_color } else { ui.visuals().weak_text_color() },
                );
            }
            // An inclusive boundary owns its own value, which changes which
            // band a block exactly on it lands in - worth showing.
            if ramp.stops[i].inclusive {
                painter.text(
                    egui::pos2(handle_rect.left() - 3.0, handle_center_y),
                    egui::Align2::RIGHT_CENTER,
                    "≤",
                    egui::FontId::proportional(9.0),
                    text_color,
                );
            }
        }

        if let Some(t) = pending_insert
            && ramp.stops.len() < MAX_GRADIENT_ENTRIES - 1
        {
            let new_index = insert_stop_sorted(&mut ramp, editor.allocate_color_stop_id(), t, min, max);
            selected = new_index;
            value_popup_stop = Some(new_index);
            changed = true;
        }

        if let Some(index) = remove_index {
            ramp.stops.remove(index);
            value_popup_stop = match value_popup_stop {
                Some(open_index) if open_index == index => None,
                Some(open_index) if open_index > index => Some(open_index - 1),
                other => other,
            };
            selected = selected.min(ramp.stops.len().saturating_sub(1));
            changed = true;
        }

        if let Some(color) = ramp.stops.get(selected).map(|stop| stop.color) {
            let swatch_rect = egui::Rect::from_min_size(
                egui::pos2(rect.center().x - COLOR_PICKER_BUTTON_WIDTH * 0.5, rect.bottom() - COLOR_PICKER_BUTTON_HEIGHT),
                egui::vec2(COLOR_PICKER_BUTTON_WIDTH, COLOR_PICKER_BUTTON_HEIGHT),
            );
            let mut srgba = straight_to_unmultiplied_srgba(color);
            let response = ui
                .scope_builder(egui::UiBuilder::new().id_salt("color_stop_color_picker").max_rect(swatch_rect), |ui| {
                    ui.spacing_mut().interact_size = egui::vec2(COLOR_PICKER_BUTTON_WIDTH, COLOR_PICKER_BUTTON_HEIGHT);
                    super::color::edit_srgba_unmultiplied(ui, &mut srgba)
                })
                .inner
                .on_hover_text(tr!(literal = "Click to edit color; right-click to remove"));
            let picker_remove_clicked =
                (response.secondary_clicked() || (ui.rect_contains_pointer(swatch_rect) && ui.input(|input| input.pointer.secondary_clicked()))) && ramp.stops.len() > 1;
            if picker_remove_clicked {
                value_popup_stop = None;
                ramp.stops.remove(selected);
                selected = selected.min(ramp.stops.len().saturating_sub(1));
                changed = true;
            }
            if response.changed() {
                if let Some(stop) = ramp.stops.get_mut(selected) {
                    stop.color = unmultiplied_srgba_to_straight(srgba);
                }
                changed = true;
            }
        }

        if changed {
            let selected_value = ramp.stops.get(selected).map(|stop| stop.value);
            let popup_value = value_popup_stop.and_then(|index| ramp.stops.get(index).map(|stop| stop.value));
            let transfer = ramp.to_transfer();
            // The command applies `sanitise`, which may sort and merge; track
            // the selection by value so it follows its stop through that.
            let sanitised = {
                let mut sanitised = transfer.clone();
                sanitised.sanitise(Some((min, max)));
                UiRamp::from_transfer(&sanitised, min, max)
            };
            let nearest = |value: f64| {
                sanitised
                    .stops
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| (a.value - value).abs().total_cmp(&(b.value - value).abs()))
                    .map(|(index, _)| index)
            };
            selected = selected_value.and_then(nearest).unwrap_or(selected);
            value_popup_stop = popup_value.and_then(nearest);
            commands.push(UiCommand::SetBlockModelColorTransfer { id: model.id, transfer });
        }
        ui.data_mut(|data| data.insert_persisted(selected_id, selected));
        ui.data_mut(|data| data.insert_persisted(value_popup_id, value_popup_stop));

        let painter = ui.painter();
        for (t, galley) in labels {
            let x = x_at(t);
            painter.galley(
                egui::pos2((x - galley.size().x * 0.5).clamp(rect.left(), rect.right() - galley.size().x), bar_rect.bottom() + 4.0),
                galley,
                text_color,
            );
        }
    }
}

/// Read-only summary of one drill hole: collar, trace extent, orientation,
/// provenance, and the interval table as a grid.
pub(crate) struct DrillHoleProperties<'a> {
    id: egui::Id,
    hole: &'a crate::model::drill_hole::DrillHole,
    /// The dataset's fields, in order, become the columns after From and
    /// To, so every hole gets the same columns even if it never recorded one.
    fields: &'a [crate::model::drill_hole::DrillField],
    list_height: f32,
}

/// Default height the interval table scrolls within.
const DRILL_INTERVAL_LIST_HEIGHT: f32 = 220.0;

/// Breathing room either side of a cell's text.
const DRILL_TABLE_CELL_PADDING: f32 = 5.0;

/// Added to the body text height to get a row's height.
const DRILL_TABLE_ROW_PADDING: f32 = 3.0;

/// Narrowest a column is drawn, even one full of blanks.
const DRILL_TABLE_MIN_COLUMN_WIDTH: f32 = 40.0;

/// Widest a column is drawn; text past this truncates on screen but still
/// travels whole in a copy.
const DRILL_TABLE_MAX_COLUMN_WIDTH: f32 = 180.0;

/// Intervals measured when sizing the columns, before the first row is
/// drawn.
const DRILL_TABLE_WIDTH_SAMPLE: usize = 512;

/// A rectangular block of cells marked for copying: the cell the reader
/// started from and the cell they last extended to.
#[derive(Clone, Copy, PartialEq)]
struct DrillTableSelection {
    anchor: (usize, usize),
    focus: (usize, usize),
}

impl DrillTableSelection {
    fn rows(self) -> std::ops::RangeInclusive<usize> {
        self.anchor.0.min(self.focus.0)..=self.anchor.0.max(self.focus.0)
    }

    fn columns(self) -> std::ops::RangeInclusive<usize> {
        self.anchor.1.min(self.focus.1)..=self.anchor.1.max(self.focus.1)
    }
}

/// Column widths measured once and cached in egui's frame store, keyed by
/// a fingerprint so a different hole under the same panel re-measures.
#[derive(Clone)]
struct DrillTableColumns {
    fingerprint: u64,
    widths: Vec<f32>,
}

/// One column as the table draws it: its width and which edge its cells
/// sit against.
#[derive(Clone, Copy)]
struct DrillTableColumn {
    width: f32,
    align: egui::Align,
}

impl<'a> DrillHoleProperties<'a> {
    pub(crate) fn new(id_source: impl Hash + Debug, hole: &'a crate::model::drill_hole::DrillHole, fields: &'a [crate::model::drill_hole::DrillField]) -> Self {
        Self {
            id: egui::Id::new(id_source),
            hole,
            fields,
            list_height: DRILL_INTERVAL_LIST_HEIGHT,
        }
    }

    /// Cap the interval table at `height` instead of the default, so a panel
    /// with little left below the summary still ends above its own edge.
    pub(crate) fn max_list_height(mut self, height: f32) -> Self {
        self.list_height = height;
        self
    }

    pub(crate) fn show(self, ui: &mut egui::Ui) {
        let hole = self.hole;
        let collar = hole.collar_position();

        egui::Grid::new(self.id.with("summary")).num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
            ui.label(tr!(literal = "Hole ID"));
            ui.add(egui::Label::new(&hole.dhid).truncate());
            ui.end_row();

            ui.label(tr!(literal = "Easting"));
            ui.label(format!("{:.2}", collar.x));
            ui.end_row();

            ui.label(tr!(literal = "Northing"));
            ui.label(format!("{:.2}", collar.y));
            ui.end_row();

            ui.label(tr!(literal = "Elevation"));
            ui.label(format!("{:.2}", collar.z));
            ui.end_row();

            ui.label(tr!(literal = "Trace extent"));
            // Truncated with an ellipsis rather than clipped mid-glyph.
            ui.add(
                egui::Label::new(match (hole.trace.first(), hole.trace.last()) {
                    (Some(first), Some(last)) => {
                        tr_format!(literal = "%from% to %to%", from = format!("{:.2}", first.depth), to = format!("{:.2}", last.depth))
                    }
                    _ => tr!(literal = "No trace"),
                })
                .truncate(),
            );
            ui.end_row();

            ui.label(tr!(literal = "Orientation"));
            ui.add(
                egui::Label::new(match hole.orientation() {
                    Some(orientation) => tr_format!(
                        literal = "Azimuth %azimuth%, dip %dip%",
                        azimuth = format!("{:.1}", orientation.azimuth),
                        dip = format!("{:.1}", orientation.dip)
                    ),
                    // Not "none": nobody recorded one, the same as its source.
                    None => tr!(literal = "Unknown"),
                })
                .truncate(),
            );
            ui.end_row();

            ui.label(tr!(literal = "Orientation source"));
            ui.add(egui::Label::new(hole.orientation_source.label()).truncate());
            ui.end_row();

            ui.label(tr!(literal = "Intervals"));
            ui.label(hole.intervals.len().to_string());
            ui.end_row();
        });

        if hole.intervals.is_empty() {
            return;
        }

        ui.separator();
        self.show_interval_table(ui);
    }

    /// The interval spreadsheet: copy actions, a header row, and the rows.
    /// Never reports a width wider than the panel gave it.
    fn show_interval_table(&self, ui: &mut egui::Ui) {
        let hole = self.hole;
        let header = drill_table_header(self.fields);
        let columns = self.columns(ui, &header);
        let total_width: f32 = columns.iter().map(|column| column.width).sum();
        let row_height = ui.text_style_height(&egui::TextStyle::Body) + DRILL_TABLE_ROW_PADDING;
        let row_height_with_spacing = row_height + ui.spacing().item_spacing.y;

        let selection_id = self.id.with("table_selection");
        let drag_id = self.id.with("table_drag");
        let mut selection: Option<DrillTableSelection> = ui.data(|data| data.get_temp(selection_id));
        let mut drag_anchor: Option<(usize, usize)> = ui.data(|data| data.get_temp(drag_id));
        let (copy_table, copy_selection) = self.show_table_heading(ui, selection.is_some());

        // Painted after the scroll area reports its offset, so it tracks.
        let available = ui.available_width();
        let (header_rect, _) = ui.allocate_exact_size(egui::vec2(available, row_height), egui::Sense::hover());

        let mut pending: Option<DrillTableSelection> = None;
        // Captured from the first row drawn, so a live drag can map the
        // pointer back to a row even if that row scrolls out of view.
        let mut table_left: Option<f32> = None;
        let mut first_row_top: Option<f32> = None;
        let list_height = self.list_height.min(ui.available_height()).max(row_height * 3.0);
        let output = egui::ScrollArea::both()
            .id_salt(self.id.with("intervals_scroll"))
            .max_width(available)
            .max_height(list_height)
            .auto_shrink([false, true])
            // Turns off drag-to-scroll, which would otherwise fight the
            // rows' own press-drag for the same pointer motion.
            .scroll_source(egui::scroll_area::ScrollSource::SCROLL_BAR | egui::scroll_area::ScrollSource::MOUSE_WHEEL)
            .show_rows(ui, row_height, hole.intervals.len(), |ui, rows| {
                let clip = ui.clip_rect();
                for index in rows {
                    let Some(interval) = hole.intervals.get(index) else {
                        continue;
                    };
                    let (_, row_rect) = ui.allocate_space(egui::vec2(total_width, row_height));
                    if table_left.is_none() {
                        table_left = Some(row_rect.left());
                        first_row_top = Some(row_rect.top() - index as f32 * row_height_with_spacing);
                    }
                    // An explicit id, keyed by row index, keeps the row's
                    // identity as the virtualised window slides.
                    let response = ui.interact(row_rect, self.id.with(("interval_row", index)), egui::Sense::click_and_drag());
                    if index % 2 == 1 {
                        ui.painter().rect_filled(row_rect, 0.0, ui.visuals().faint_bg_color);
                    }
                    if let Some(current) = selection.filter(|current| current.rows().contains(&index)) {
                        ui.painter().rect_filled(
                            drill_table_span(&columns, row_rect, current.columns()),
                            crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS,
                            ui.visuals().selection.bg_fill.gamma_multiply(0.4),
                        );
                    }
                    draw_drill_table_row(
                        ui,
                        row_rect,
                        clip,
                        &columns,
                        self.id.with(("interval_cells", index)),
                        &drill_table_row(interval, self.fields, false),
                        false,
                    );
                    if response.clicked()
                        && let Some(position) = response.interact_pointer_pos()
                    {
                        let column = drill_table_column_at(&columns, row_rect.left(), position.x);
                        let extend = ui.input(|input| input.modifiers.shift);
                        pending = Some(match selection.filter(|_| extend) {
                            Some(current) => DrillTableSelection {
                                anchor: current.anchor,
                                focus: (index, column),
                            },
                            None => DrillTableSelection {
                                anchor: (index, column),
                                focus: (index, column),
                            },
                        });
                    }
                    // Reads `press_origin`, not the pointer: it may already be
                    // over another row by the time the drag is confirmed.
                    if response.drag_started()
                        && let Some(origin) = ui.input(|input| input.pointer.press_origin())
                    {
                        drag_anchor = Some((index, drill_table_column_at(&columns, row_rect.left(), origin.x)));
                    }
                }
            });

        // Follows the pointer from here rather than from the origin row's
        // response, since that row can scroll out of view mid-drag.
        if let Some(anchor) = drag_anchor {
            if let (Some(left), Some(top), Some(position)) = (table_left, first_row_top, ui.input(|input| input.pointer.interact_pos())) {
                let row = drill_table_row_at(top, row_height_with_spacing, hole.intervals.len(), position.y);
                let column = drill_table_column_at(&columns, left, position.x);
                pending = Some(DrillTableSelection { anchor, focus: (row, column) });
            }
            if !ui.input(|input| input.pointer.primary_down()) {
                drag_anchor = None;
            }
        }

        if let Some(next) = pending {
            selection = Some(next);
            ui.data_mut(|data| data.insert_temp(selection_id, next));
        }
        // Stored as the anchor itself: egui's store is keyed by id and
        // type, so writing `Option<T>` and reading `T` back would fail.
        ui.data_mut(|data| match drag_anchor {
            Some(anchor) => {
                data.insert_temp(drag_id, anchor);
            }
            None => data.remove::<(usize, usize)>(drag_id),
        });

        // Tracks the columns horizontally but ignores them vertically, so
        // it still names the right one however far the reader has scrolled.
        let header_row = egui::Rect::from_min_size(
            egui::pos2(header_rect.left() - output.state.offset.x, header_rect.top()),
            egui::vec2(total_width, row_height),
        );
        let header_clip = header_rect.intersect(ui.clip_rect());
        ui.painter().rect_filled(header_rect, 0.0, ui.visuals().faint_bg_color);
        draw_drill_table_row(ui, header_row, header_clip, &columns, self.id.with("header_cells"), &header, true);

        // Ctrl+C over the table, scoped to the pointer being over it.
        let wants_shortcut = ui.rect_contains_pointer(header_rect.union(output.inner_rect)) && ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::C));
        match selection.filter(|_| copy_selection || (wants_shortcut && !copy_table)) {
            Some(current) => ui.ctx().copy_text(drill_selection_text(hole, self.fields, current)),
            None if copy_table || wants_shortcut => ui.ctx().copy_text(drill_table_text(hole, self.fields)),
            None => {}
        }
    }

    /// The strip above the table: its name on the left, its copy actions
    /// on the right, both drawn into one exactly sized rect.
    fn show_table_heading(&self, ui: &mut egui::Ui, has_selection: bool) -> (bool, bool) {
        let strip_height = ui.spacing().interact_size.y;
        let (strip, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), strip_height), egui::Sense::hover());

        let mut actions = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(self.id.with("table_actions"))
                .max_rect(strip)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        let copy_table = actions.small_button(tr!(literal = "Copy table")).clicked();
        let copy_selection = actions.add_enabled(has_selection, egui::Button::new(tr!(literal = "Copy selection")).small()).clicked();

        let mut title = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(self.id.with("table_title"))
                .max_rect(strip.with_max_x((actions.min_rect().left() - 6.0).max(strip.left())))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        title.add(egui::Label::new(egui::RichText::new(tr!(literal = "Interval data")).strong().color(ui.visuals().weak_text_color())).truncate());

        (copy_table, copy_selection)
    }

    /// The table's columns: each measured width paired with its alignment.
    fn columns(&self, ui: &egui::Ui, header: &[String]) -> Vec<DrillTableColumn> {
        self.column_widths(ui, header)
            .into_iter()
            .zip(drill_table_aligns(self.fields))
            .map(|(width, align)| DrillTableColumn { width, align })
            .collect()
    }

    /// Measure each column once, keyed on what it was measured from.
    fn column_widths(&self, ui: &egui::Ui, header: &[String]) -> Vec<f32> {
        let id = self.id.with("table_columns");
        let font = egui::TextStyle::Body.resolve(ui.style());
        let fingerprint = self.column_fingerprint(&font);
        if let Some(cached) = ui.data(|data| data.get_temp::<DrillTableColumns>(id)).filter(|cached| cached.fingerprint == fingerprint) {
            return cached.widths;
        }

        let measure = |text: &str| ui.painter().layout_no_wrap(text.to_owned(), font.clone(), egui::Color32::PLACEHOLDER).size().x;
        let mut widths: Vec<f32> = header.iter().map(|label| measure(label)).collect();
        for interval in self.hole.intervals.iter().take(DRILL_TABLE_WIDTH_SAMPLE) {
            for (width, cell) in widths.iter_mut().zip(drill_table_row(interval, self.fields, false)) {
                *width = width.max(measure(&cell));
            }
        }
        for width in &mut widths {
            *width = (*width + DRILL_TABLE_CELL_PADDING * 2.0).clamp(DRILL_TABLE_MIN_COLUMN_WIDTH, DRILL_TABLE_MAX_COLUMN_WIDTH);
        }

        ui.data_mut(|data| {
            data.insert_temp(
                id,
                DrillTableColumns {
                    fingerprint,
                    widths: widths.clone(),
                },
            )
        });
        widths
    }

    /// What the cached widths were measured from; a change invalidates them.
    fn column_fingerprint(&self, font: &egui::FontId) -> u64 {
        use std::hash::Hasher;

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hole.intervals.len().hash(&mut hasher);
        self.hole
            .intervals
            .last()
            .map(|interval| (interval.from.to_bits(), interval.to.to_bits()))
            .hash(&mut hasher);
        for field in self.fields {
            field.label.hash(&mut hasher);
        }
        font.size.to_bits().hash(&mut hasher);
        hasher.finish()
    }
}

/// The table's header row: From, To, then one column per dataset field.
fn drill_table_header(fields: &[crate::model::drill_hole::DrillField]) -> Vec<String> {
    let mut header = Vec::with_capacity(fields.len() + 2);
    header.push(tr!(literal = "From"));
    header.push(tr!(literal = "To"));
    header.extend(fields.iter().map(|field| field.label.clone()));
    header
}

/// One interval as text, one cell per header column, `raw` for full
/// precision instead of display rounding. An unrecorded field is an empty cell.
///
/// A corrected cell shows the interpreted value with the logged one in
/// brackets; raw text, which a copy carries, is the interpreted value alone.
fn drill_table_row(interval: &crate::model::drill_hole::DrillInterval, fields: &[crate::model::drill_hole::DrillField], raw: bool) -> Vec<String> {
    let (logged_from, logged_to, logged_values) = interval.logged();
    let corrected = !raw && interval.is_corrected();
    let beside = |shown: String, logged: String| if corrected && shown != logged { format!("{shown} ({logged})") } else { shown };
    let depth = |depth: f64| if raw { depth.to_string() } else { format!("{depth:.2}") };
    let value = |value: Option<&crate::model::drill_hole::DrillValue>| match value {
        Some(crate::model::drill_hole::DrillValue::Numeric(number)) => {
            if raw {
                number.to_string()
            } else {
                format_grade(*number)
            }
        }
        Some(crate::model::drill_hole::DrillValue::Category(category)) => category.clone(),
        None => String::new(),
    };
    let mut row = Vec::with_capacity(fields.len() + 2);
    row.push(beside(depth(interval.from), depth(logged_from)));
    row.push(beside(depth(interval.to), depth(logged_to)));
    row.extend(
        fields
            .iter()
            .map(|field| beside(value(interval.values.get(&field.key)), value(logged_values.get(&field.key)))),
    );
    row
}

/// Which edge a column's cells sit against: numbers right, categories left.
fn drill_table_aligns(fields: &[crate::model::drill_hole::DrillField]) -> Vec<egui::Align> {
    let mut aligns = vec![egui::Align::Max, egui::Align::Max];
    aligns.extend(fields.iter().map(|field| match field.kind {
        crate::model::drill_hole::DrillFieldKind::Numeric { .. } => egui::Align::Max,
        crate::model::drill_hole::DrillFieldKind::Categorical { .. } => egui::Align::Min,
    }));
    aligns
}

/// The whole table as tab separated text, header included, ready to paste.
fn drill_table_text(hole: &crate::model::drill_hole::DrillHole, fields: &[crate::model::drill_hole::DrillField]) -> String {
    let mut rows = Vec::with_capacity(hole.intervals.len() + 1);
    rows.push(drill_table_header(fields));
    rows.extend(hole.intervals.iter().map(|interval| drill_table_row(interval, fields, true)));
    drill_table_tsv(&rows)
}

/// The marked block as tab separated text, with no header row.
fn drill_selection_text(hole: &crate::model::drill_hole::DrillHole, fields: &[crate::model::drill_hole::DrillField], selection: DrillTableSelection) -> String {
    let columns = selection.columns();
    let rows: Vec<Vec<String>> = selection
        .rows()
        .filter_map(|index| hole.intervals.get(index))
        .map(|interval| {
            let row = drill_table_row(interval, fields, true);
            columns.clone().filter_map(|column| row.get(column).cloned()).collect()
        })
        .collect();
    drill_table_tsv(&rows)
}

/// Serialise rows as tab separated lines, one cell per tab.
fn drill_table_tsv(rows: &[Vec<String>]) -> String {
    let mut text = String::new();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            if index > 0 {
                text.push('\t');
            }
            // A tab or newline inside a value travels as a space instead.
            text.extend(cell.chars().map(|character| if matches!(character, '\t' | '\n' | '\r') { ' ' } else { character }));
        }
        text.push('\n');
    }
    text
}

/// The rect a run of columns covers within `row`, for painting behind them.
fn drill_table_span(columns: &[DrillTableColumn], row: egui::Rect, span: std::ops::RangeInclusive<usize>) -> egui::Rect {
    let start: f32 = columns.iter().take(*span.start()).map(|column| column.width).sum();
    let end: f32 = columns.iter().take(span.end().saturating_add(1)).map(|column| column.width).sum();
    egui::Rect::from_x_y_ranges(row.left() + start..=row.left() + end, row.y_range())
}

/// Which column the pointer is over, clamped to the table's own edges.
fn drill_table_column_at(columns: &[DrillTableColumn], left: f32, x: f32) -> usize {
    let mut edge = left;
    for (index, column) in columns.iter().enumerate() {
        edge += column.width;
        if x < edge {
            return index;
        }
    }
    columns.len().saturating_sub(1)
}

/// Which row the pointer is over during a drag, clamped to the row count.
fn drill_table_row_at(first_row_top: f32, row_height: f32, total_rows: usize, y: f32) -> usize {
    if total_rows == 0 || row_height <= 0.0 {
        return 0;
    }
    let offset = ((y - first_row_top) / row_height).floor();
    if offset <= 0.0 { 0 } else { (offset as usize).min(total_rows - 1) }
}

/// Lay one row of cells across `row`, each in its column's width and
/// against its column's edge, clipped to `clip`.
fn draw_drill_table_row(ui: &mut egui::Ui, row: egui::Rect, clip: egui::Rect, columns: &[DrillTableColumn], salt: egui::Id, cells: &[String], strong: bool) {
    let mut x = row.left();
    for (index, cell) in cells.iter().enumerate() {
        let Some(&column) = columns.get(index) else {
            break;
        };
        let rect = egui::Rect::from_min_size(egui::pos2(x, row.top()), egui::vec2(column.width, row.height()));
        x += column.width;
        let visible = rect.intersect(clip);
        // Columns scrolled off either side are not laid out at all.
        if cell.is_empty() || !visible.is_positive() {
            continue;
        }
        let layout = if column.align == egui::Align::Max {
            egui::Layout::right_to_left(egui::Align::Center)
        } else {
            egui::Layout::left_to_right(egui::Align::Center)
        };
        let mut cell_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(salt.with(index))
                .max_rect(rect.shrink2(egui::vec2(DRILL_TABLE_CELL_PADDING, 0.0)))
                .layout(layout),
        );
        cell_ui.set_clip_rect(visible);
        let text = egui::RichText::new(cell);
        cell_ui.add(egui::Label::new(if strong { text.strong() } else { text }).truncate().selectable(false));
    }
}

/// Ink/outline pair for overlay text: black-on-white over a light background,
/// white-on-black over a dark one, split at 0.45 Rec. 709 luminance.
fn contrast_ink(background: [f32; 4]) -> (egui::Color32, egui::Color32) {
    let luminance = crate::rendering::color::relative_luminance(background);
    if luminance > 0.45 {
        (egui::Color32::BLACK, egui::Color32::WHITE)
    } else {
        (egui::Color32::WHITE, egui::Color32::BLACK)
    }
}

/// Font shared by the scale bar and section-grid overlay labels.
fn overlay_label_font() -> egui::FontId {
    egui::FontId::new(11.0, egui::FontFamily::Name("noto_sans_bold".into()))
}

/// Draws `text` with a 1px outline pass behind the ink pass, readable with no solid backdrop.
fn outlined_label(painter: &egui::Painter, pos: egui::Pos2, align: egui::Align2, text: &str, font: &egui::FontId, ink: egui::Color32, outline: egui::Color32) {
    outlined_galley(painter, pos, align, painter.layout_no_wrap(text.to_owned(), font.clone(), ink), ink, outline);
}

/// [`outlined_label`] for a caller holding an already-laid-out galley.
fn outlined_galley(painter: &egui::Painter, pos: egui::Pos2, align: egui::Align2, galley: std::sync::Arc<egui::Galley>, ink: egui::Color32, outline: egui::Color32) {
    let rect = align.anchor_size(pos, galley.size());
    painter.galley_with_override_text_color(rect.min + egui::vec2(1.0, 1.0), galley.clone(), outline);
    painter.galley_with_override_text_color(rect.min, galley, ink);
}

/// A cartographic scale bar pinned to the viewport's bottom-right corner.
///
/// `world_per_point` is measured in metres per egui logical point. The scene
/// is orthographic outside fly mode, so this remains valid while orbiting as
/// well as in plan view.
pub(crate) struct ViewportScaleBar {
    id: egui::Id,
    viewport_rect: egui::Rect,
}

impl ViewportScaleBar {
    pub(crate) fn new(id_source: impl Hash + Debug, viewport_rect: egui::Rect) -> Self {
        Self {
            id: egui::Id::new(id_source),
            viewport_rect,
        }
    }

    pub(crate) fn show(self, ctx: &egui::Context, world_per_point: Option<f64>, viewport_background: [f32; 4]) {
        let Some(world_per_point) = world_per_point.filter(|value| value.is_finite() && *value > 0.0) else {
            return;
        };
        let distance = nice_scale_distance(world_per_point * SCALE_BAR_TARGET_WIDTH);
        let bar_width = (distance / world_per_point) as f32;
        let bar_size = egui::vec2(bar_width + SCALE_BAR_LABEL_OVERHANG * 2.0, SCALE_BAR_HEIGHT);
        // Nothing floats over the viewport's right edge any more, so the bar
        // hangs off its bottom-right corner with nothing to dodge.
        let anchor = egui::pos2(
            self.viewport_rect.right() - SCALE_BAR_VIEWPORT_MARGIN,
            self.viewport_rect.bottom() - SCALE_BAR_VIEWPORT_MARGIN,
        );
        let (ink, outline) = contrast_ink(viewport_background);

        egui::Area::new(self.id)
            .order(egui::Order::Background)
            // Read-only, like the tool label: kept out of `layer_id_at` so the
            // drawn cursor survives crossing it.
            .interactable(false)
            .pivot(egui::Align2::RIGHT_BOTTOM)
            .fixed_pos(anchor)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(bar_size, egui::Sense::hover());
                let painter = ui.painter();

                let bar_rect = egui::Rect::from_min_size(rect.min + egui::vec2(SCALE_BAR_LABEL_OVERHANG, 1.0), egui::vec2(bar_width, 5.0));
                for (index, fractions) in SCALE_BAR_SEGMENT_FRACTIONS.windows(2).enumerate() {
                    let segment = egui::Rect::from_min_max(
                        egui::pos2(bar_rect.left() + bar_width * fractions[0] as f32, bar_rect.top()),
                        egui::pos2(bar_rect.left() + bar_width * fractions[1] as f32, bar_rect.bottom()),
                    );
                    if index % 2 == 0 {
                        painter.rect_filled(segment, 0.0, ink);
                    } else {
                        painter.rect_stroke(segment, 0.0, egui::Stroke::new(1.0, ink), egui::StrokeKind::Inside);
                    }
                }

                let labels = scale_bar_labels(distance);
                let font = overlay_label_font();
                let label_y = bar_rect.bottom() + 2.0;
                for (index, fraction) in SCALE_BAR_SEGMENT_FRACTIONS.iter().copied().enumerate() {
                    let x = bar_rect.left() + bar_width * fraction as f32;
                    let label = &labels[index];
                    let position = egui::pos2(x, label_y);
                    outlined_label(painter, position, egui::Align2::CENTER_TOP, label, &font, ink, outline);
                }
            });
    }
}

/// Clips a line segment to a rectangle with the Liang-Barsky algorithm,
/// narrowing `t0..=t1` (0 at `from`, 1 at `to`) to the portion inside all
/// four edges. Returns `None` when that range is empty.
fn clip_segment_to_rect(from: egui::Pos2, to: egui::Pos2, rect: egui::Rect) -> Option<(egui::Pos2, egui::Pos2)> {
    let delta = to - from;
    let mut t0 = 0.0_f32;
    let mut t1 = 1.0_f32;
    // (p, q) per edge: p is the rate of approach (negative = entering), q is how far `from` already sits inside it.
    let edges = [
        (-delta.x, from.x - rect.left()),
        (delta.x, rect.right() - from.x),
        (-delta.y, from.y - rect.top()),
        (delta.y, rect.bottom() - from.y),
    ];
    for (p, q) in edges {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            if t > t1 {
                return None;
            }
            t0 = t0.max(t);
        } else {
            if t < t0 {
                return None;
            }
            t1 = t1.min(t);
        }
    }
    Some((from + delta * t0, from + delta * t1))
}

/// Paints the vertical slice view's grid: levels of constant elevation and the eastings/northings
/// the cut crosses, projected to window pixels once per frame by the renderer and published on `EditorState`.
pub(crate) fn draw_section_grid(ui: &egui::Ui, editor: &EditorState, canvas_rect: egui::Rect) {
    if editor.section_grid_px.is_empty() {
        return;
    }
    // Panels paint over the scene rather than clipping it, so the grid must clip itself to the viewport.
    let painter = ui.painter().with_clip_rect(canvas_rect);
    let ppp = ui.ctx().pixels_per_point();
    let to_pos = |px: (f32, f32)| egui::pos2(px.0 / ppp, px.1 / ppp);

    let (ink, outline) = contrast_ink(editor.renderer_background_color);
    let font = overlay_label_font();

    // A label under a floating panel is dropped rather than drawn unreadable; panel areas are
    // collected as a set rather than by name, since the dock alone is a dozen panels.
    let mut obscured_by: Vec<egui::Rect> = ui.ctx().memory(|memory| {
        let mut rects: Vec<egui::Rect> = memory
            .areas()
            .visible_layer_ids()
            .into_iter()
            .filter(|layer| layer.order != egui::Order::Background)
            .filter_map(|layer| memory.area_rect(layer.id))
            .collect();
        if editor.show_scale_bar {
            rects.extend(memory.area_rect(egui::Id::new("viewport_scale_bar")));
        }
        rects
    });
    if editor.show_world_axis_gizmo {
        obscured_by.push(crate::ui::elements::cursors::orientation_gizmo_rect(canvas_rect));
    }

    let mut placed: Vec<egui::Rect> = Vec::new();
    for line in &editor.section_grid_px {
        let from = to_pos(line.from_px);
        let to = to_pos(line.to_px);
        // The line itself is drawn on the plane by the renderer, under the
        // geometry; only its label is placed here, at the on-screen end.
        let Some((from, to)) = clip_segment_to_rect(from, to, canvas_rect) else {
            continue;
        };

        // A level's number stands alone as an RL; eastings and northings need their axis prefixed to tell them apart.
        // A negative zero from the index arithmetic would print as "-0".
        let value = if line.value == 0.0 { 0.0 } else { line.value };
        let number = format!("{value:.0}");
        let text = match line.kind {
            SectionGridLineKind::Level => number,
            SectionGridLineKind::Upright(SectionGridAxis::Easting) => format!("{}{number}", tr!(literal = "E ")),
            SectionGridLineKind::Upright(SectionGridAxis::Northing) => format!("{}{number}", tr!(literal = "N ")),
        };
        // RLs read down the right edge, not the left, to clear the slice view's bottom-left dock panel.
        let (endpoint, align, offset) = match line.kind {
            SectionGridLineKind::Level => {
                let endpoint = if from.x >= to.x { from } else { to };
                (endpoint, egui::Align2::RIGHT_CENTER, egui::vec2(-SECTION_GRID_LABEL_GAP, 0.0))
            }
            SectionGridLineKind::Upright(_) => {
                // Screen y grows downward, so the larger-y end is the lower one; eastings and northings read along the bottom.
                let endpoint = if from.y >= to.y { from } else { to };
                (endpoint, egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -SECTION_GRID_LABEL_GAP))
            }
        };
        let position = endpoint + offset;

        let galley = painter.layout_no_wrap(text, font.clone(), ink);
        let label_rect = align.anchor_size(position, galley.size());
        // A label landing on one already drawn is dropped; the line stays.
        if obscured_by.iter().chain(&placed).any(|taken| taken.intersects(label_rect)) {
            continue;
        }
        placed.push(label_rect);
        outlined_galley(&painter, position, align, galley, ink, outline);
    }
}

fn nice_scale_distance(target: f64) -> f64 {
    let exponent = target.log10().floor();
    let magnitude = 10.0_f64.powf(exponent);
    let normalized = target / magnitude;
    let multiplier = if normalized < 1.5 {
        1.0
    } else if normalized < 3.5 {
        2.0
    } else if normalized < 7.5 {
        5.0
    } else {
        10.0
    };
    multiplier * magnitude
}

fn format_scale_number(value: f64) -> String {
    let formatted = trim_decimal_zeros(format!("{value:.3}"));
    formatted.strip_prefix("0.").map_or(formatted.clone(), |fraction| format!(".{fraction}"))
}

fn scale_display_unit(metres: f64) -> (f64, &'static str) {
    [(0.001, "km"), (1.0, "m"), (100.0, "cm"), (1000.0, "mm")]
        .into_iter()
        .filter(|(scale, _)| metres * scale >= 0.1)
        .min_by_key(|(scale, _)| {
            SCALE_BAR_SEGMENT_FRACTIONS[1..SCALE_BAR_SEGMENT_FRACTIONS.len() - 1]
                .iter()
                .map(|fraction| format_scale_number(metres * fraction * scale).len())
                .sum::<usize>()
        })
        .unwrap_or((1000.0, "mm"))
}

fn scale_bar_labels(distance: f64) -> [String; SCALE_BAR_SEGMENT_FRACTIONS.len()] {
    let (unit_scale, unit) = scale_display_unit(distance);
    std::array::from_fn(|index| {
        let mut label = format_scale_number(distance * SCALE_BAR_SEGMENT_FRACTIONS[index] * unit_scale);
        if index + 1 == SCALE_BAR_SEGMENT_FRACTIONS.len() {
            label.push_str(unit);
        }
        label
    })
}

fn active_variable_range(editor: &mut EditorState, model: &OpenBlockModel) -> Option<(f64, f64)> {
    let name = model.active_color_variable.as_deref()?;
    cached_variable_range(editor, model, name)
}

/// Whether the legend can show this model: it already has an active variable,
/// or it has at least one supported, non-special colour variable the user can pick.
fn model_has_selectable_variable(model: &OpenBlockModel) -> bool {
    model.active_color_variable.is_some() || model.model.color_variables().into_iter().any(|variable| !variable.special)
}

fn cached_variable_range(editor: &mut EditorState, model: &OpenBlockModel, name: &str) -> Option<(f64, f64)> {
    let key = (model.id, name.to_owned());
    if let Some(range) = editor.block_model_variable_ranges.get(&key) {
        return *range;
    }
    let range = if model.active_color_variable.as_deref() == Some(name) {
        model.active_value_range()
    } else {
        model.model.variable(name).and_then(|variable| {
            let default = color_variable_default(variable);
            model
                .model
                .color_values(name)
                .ok()
                .and_then(|values| render_value_range(&values, &model.renderable_block_indices, default))
        })
    };
    editor.block_model_variable_ranges.insert(key, range);
    range
}

fn is_categorical_variable(variable: &crate::model::formats::block_model_data::BlockVariable) -> bool {
    matches!(variable.physical_type.as_str(), "namedbyte" | "namedshort")
}

fn category_count(variable: &crate::model::formats::block_model_data::BlockVariable) -> usize {
    variable.strings.len()
}

fn format_category_count(variable: &crate::model::formats::block_model_data::BlockVariable) -> String {
    let count = category_count(variable);
    if count == 1 {
        tr_format!(literal = "%count% category", count = count)
    } else {
        tr_format!(literal = "%count% categories", count = count)
    }
}

/// Compact share label for a legend category: `0%`, `<1%`, or a whole percent.
fn format_category_percent(fraction: f32) -> String {
    let percent = fraction * 100.0;
    if fraction <= 0.0 {
        "0%".to_owned()
    } else if percent < 1.0 {
        "<1%".to_owned()
    } else {
        format!("{percent:.0}%")
    }
}

fn normalized_to_value(t: f32, min: f64, max: f64) -> f64 {
    min + (max - min) * t.clamp(0.0, 1.0) as f64
}

fn value_to_normalized(value: f64, min: f64, max: f64) -> f32 {
    if (max - min).abs() <= f64::EPSILON {
        0.0
    } else {
        ((value - min) / (max - min)).clamp(0.0, 1.0) as f32
    }
}

/// One prompt for the viewport banner: the instruction, and optionally the
/// detail that qualifies it.
///
/// The two halves are given separately rather than punctuated into one string
/// so the banner can set them apart itself - the instruction at full strength,
/// the detail dimmed and held off at a gap, no punctuation between them - and
/// so a translator can move either half on its own.
#[derive(Clone)]
pub(crate) struct ViewportMessage {
    text: String,
    minor: Option<String>,
}

impl ViewportMessage {
    /// The instruction: what the tool is waiting for, in as few words as it
    /// takes to say it.
    pub(crate) fn text(text: impl Into<String>) -> Self {
        Self { text: text.into(), minor: None }
    }

    /// Detail that qualifies the instruction - the keys it answers to, the way
    /// out of it, why it is being asked. Written as its own phrase: the banner
    /// parts it from the instruction with weight and space, not punctuation.
    pub(crate) fn minor(mut self, minor: impl Into<String>) -> Self {
        self.minor = Some(minor.into());
        self
    }
}

/// A compact prompt pinned to the top of the 3D viewport.
///
/// Painted as a member of the floating-menu family - same surface, hairline
/// and corner radius - so it reads as part of the window rather than as a
/// sticky note dropped on top of it. An accent pip marks it as a live prompt,
/// and a [`ViewportMessage`]'s two halves are set apart by weight and spacing
/// so the thing to do is read before the keys that qualify it.
pub(crate) struct ViewportLabel {
    id: egui::Id,
    message: ViewportMessage,
    viewport_rect: egui::Rect,
    margin: f32,
}

/// Gap set between a prompt's instruction and its detail: the only thing
/// parting them, so it is wider than ordinary word spacing.
const PROMPT_DETAIL_GAP: f32 = 12.0;
/// Diameter of the pip that marks the card as a prompt.
const PROMPT_PIP_DIAMETER: f32 = 5.0;

/// Colour of the prompt pip: the amber the viewport already draws its guides
/// and previews in, stepped down in the light theme where full-strength amber
/// on a pale card reads as washed out.
fn prompt_pip_color(visuals: &egui::Visuals) -> egui::Color32 {
    if visuals.dark_mode {
        egui::Color32::from_rgb(255, 200, 60)
    } else {
        egui::Color32::from_rgb(198, 140, 12)
    }
}

impl ViewportLabel {
    pub(crate) fn new(id_source: impl Hash + Debug, message: ViewportMessage, viewport_rect: egui::Rect) -> Self {
        Self {
            id: egui::Id::new(id_source),
            message,
            viewport_rect,
            margin: 12.0,
        }
    }

    pub(crate) fn show(self, ctx: &egui::Context) {
        let pos = egui::pos2(self.viewport_rect.center().x, self.viewport_rect.top() + self.margin);
        egui::Area::new(self.id)
            .order(egui::Order::Foreground)
            // A banner, not a control: staying out of `layer_id_at` keeps the
            // viewport's drawn cursor from flicking back to the pointer every
            // time it passes underneath.
            .interactable(false)
            .pivot(egui::Align2::CENTER_TOP)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                let visuals = ui.visuals().clone();
                egui::Frame::new()
                    .fill(menu::menu_surface(&visuals))
                    .stroke(menu::menu_border(&visuals))
                    .corner_radius(egui::CornerRadius::same(crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS))
                    .inner_margin(egui::Margin { left: 9, right: 11, top: 5, bottom: 5 })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 7.0;
                            let (pip, _) = ui.allocate_exact_size(egui::Vec2::splat(PROMPT_PIP_DIAMETER), egui::Sense::hover());
                            ui.painter().circle_filled(pip.center(), PROMPT_PIP_DIAMETER / 2.0, prompt_pip_color(&visuals));

                            let label = |ui: &mut egui::Ui, text: String, color: egui::Color32| {
                                ui.add(egui::Label::new(egui::RichText::new(text).color(color)).wrap_mode(egui::TextWrapMode::Extend));
                            };
                            label(ui, self.message.text, visuals.strong_text_color());
                            if let Some(minor) = self.message.minor {
                                ui.spacing_mut().item_spacing.x = PROMPT_DETAIL_GAP;
                                label(ui, minor, visuals.weak_text_color());
                            }
                        });
                    });
            });
    }
}

/// Fixed, titleless shaded plan preview shown while slice mode is active.
pub(crate) struct ViewportMiniMap {
    id: egui::Id,
    viewport_rect: egui::Rect,
    gizmo_block: egui::Rect,
}

impl ViewportMiniMap {
    pub(crate) fn new(id_source: impl Hash + Debug, viewport_rect: egui::Rect) -> Self {
        Self {
            id: egui::Id::new(id_source),
            viewport_rect,
            gizmo_block: egui::Rect::NOTHING,
        }
    }

    /// Hang the preview below the orientation gizmo, which shares the
    /// viewport's top-right corner with it.
    ///
    /// The preview is painted in the foreground order, above the gizmo, so it
    /// has to keep clear of it itself rather than let paint order hide it.
    /// Pass [`egui::Rect::NOTHING`] while the gizmo is switched off, and the
    /// preview takes the corner.
    pub(crate) fn below_gizmo(mut self, block: egui::Rect) -> Self {
        self.gizmo_block = block;
        self
    }

    pub(crate) fn show(self, ctx: &egui::Context, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
        #[cfg(target_arch = "wasm32")]
        let _ = commands;

        if editor.slice_preview_detached {
            return;
        }

        // The preview is square - a slice is looked at from straight above, so
        // neither axis deserves more room than the other - and it tracks the
        // viewport's shorter side within fixed bounds. It is deliberately not
        // user-resizable: resizing an egui window captures pointer interaction
        // and makes the following middle-drag feel as if the 3D canvas needs
        // to be focused again.
        let top = if self.gizmo_block.is_positive() {
            self.gizmo_block.bottom() + SLICE_PREVIEW_MARGIN
        } else {
            self.viewport_rect.top() + SLICE_PREVIEW_TOP
        };
        let side = (self.viewport_rect.width().min(self.viewport_rect.height()) * 0.24)
            .clamp(SLICE_PREVIEW_MIN_SIZE, SLICE_PREVIEW_MAX_SIZE)
            // Never hang off the bottom of the viewport, however short it is.
            .min(self.viewport_rect.bottom() - SLICE_PREVIEW_MARGIN - top)
            .max(1.0);
        let preview_size = egui::vec2(side, side);
        let preview_pos = egui::pos2(self.viewport_rect.right() - preview_size.x - SLICE_PREVIEW_MARGIN, top);
        let frame = egui::Frame::window(&ctx.global_style()).inner_margin(egui::Margin::ZERO);
        egui::Window::new("")
            .id(self.id)
            .order(egui::Order::Foreground)
            .fixed_pos(preview_pos)
            .fixed_size(preview_size)
            .movable(false)
            .resizable(false)
            .collapsible(false)
            .title_bar(false)
            .frame(frame)
            .show(ctx, |ui| {
                let size = ui.available_size().max(egui::vec2(120.0, 120.0));
                let response = if let Some(texture_id) = editor.slice_preview_texture {
                    ui.add(
                        egui::Image::new(egui::load::SizedTexture::new(texture_id, size))
                            .fit_to_exact_size(size)
                            .sense(egui::Sense::click_and_drag()),
                    )
                } else {
                    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
                    ui.painter().rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
                    response
                };
                let pixels_per_point = ctx.pixels_per_point();
                editor.slice_preview_size_px = [
                    (response.rect.width() * pixels_per_point).round().max(1.0) as u32,
                    (response.rect.height() * pixels_per_point).round().max(1.0) as u32,
                ];

                let fitted_zoom = crate::ui::state::fitted_slice_preview_zoom(editor.slice_half_length, editor.slice_preview_size_px[1], f64::from(pixels_per_point));
                let current_zoom = fitted_zoom * editor.slice_preview_navigation.zoom_multiplier();
                let mut navigation_changed = false;
                if response.dragged_by(egui::PointerButton::Middle) {
                    let delta = response.drag_delta() * pixels_per_point;
                    navigation_changed |=
                        editor
                            .slice_preview_navigation
                            .pan_by_pixels([f64::from(delta.x), f64::from(delta.y)], current_zoom, f64::from(editor.slice_preview_size_px[1]));
                }

                if response.hovered() {
                    let scroll = ui.input(|input| {
                        input
                            .events
                            .iter()
                            .filter_map(|event| match event {
                                egui::Event::MouseWheel { unit, delta, .. } => Some(match unit {
                                    egui::MouseWheelUnit::Point => f64::from(delta.y * pixels_per_point),
                                    // Match the native camera's convention that one wheel
                                    // line is approximately one hundred physical pixels.
                                    egui::MouseWheelUnit::Line => f64::from(delta.y) * 100.0,
                                    egui::MouseWheelUnit::Page => f64::from(delta.y) * f64::from(editor.slice_preview_size_px[1]),
                                }),
                                _ => None,
                            })
                            .sum::<f64>()
                    });
                    if scroll != 0.0 {
                        let pointer = response.hover_pos().unwrap_or(response.rect.center());
                        let cursor_px = [
                            f64::from((pointer.x - response.rect.left()) * pixels_per_point),
                            f64::from((pointer.y - response.rect.top()) * pixels_per_point),
                        ];
                        navigation_changed |= editor.slice_preview_navigation.zoom_at_pixel(
                            scroll,
                            cursor_px,
                            [f64::from(editor.slice_preview_size_px[0]), f64::from(editor.slice_preview_size_px[1])],
                            fitted_zoom,
                        );
                    }
                }

                if navigation_changed {
                    ctx.request_repaint();
                }

                #[cfg(not(target_arch = "wasm32"))]
                if response.clicked() {
                    commands.push(UiCommand::SetSlicePreviewDetached(true));
                }
                #[cfg(not(target_arch = "wasm32"))]
                response.on_hover_text(tr!(literal = "Middle-drag to pan · Scroll to zoom · Click to detach"));
                #[cfg(target_arch = "wasm32")]
                response.on_hover_text(tr!(literal = "Middle-drag to pan · Scroll to zoom"));
            });
    }
}

/// Width of the hole's ribbon in the log track.
const LOG_COLUMN_WIDTH: f32 = 46.0;
/// Room reserved down the left of the log for depth ticks and their labels.
const LOG_SCALE_WIDTH: f32 = 52.0;
/// Narrowest the depth scale is drawn; never dropped entirely, only squeezed.
const LOG_SCALE_MIN_WIDTH: f32 = 34.0;
/// Width of the stratigraphic column drawn between the scale and the hole.
const LOG_STRAT_WIDTH: f32 = 64.0;
/// Narrowest a strat column is still worth drawing before it is dropped.
const LOG_STRAT_MIN_WIDTH: f32 = 22.0;
/// Narrowest the hole track is drawn before the strat column is dropped.
const LOG_TRACK_MIN_WIDTH: f32 = 24.0;
/// Gap between the stratigraphic column and the hole beside it.
const LOG_TRACK_GAP: f32 = 10.0;
/// Margin down the right of the log and below it.
const LOG_EDGE_MARGIN: f32 = 8.0;
/// Room above the plot for the strat column's field name.
const LOG_PLOT_TOP_MARGIN: f32 = 14.0;
/// Log width below which the header's compass shrinks to its smaller size.
const LOG_NARROW_WIDTH: f32 = 240.0;
/// Shortest the log is drawn, even with almost no panel height left.
const LOG_MIN_HEIGHT: f32 = 120.0;
/// Degrees of bearing per point of horizontal drag.
const LOG_SPIN_PER_POINT: f32 = 0.5;
/// Shallowest depth window the wheel will zoom to.
const LOG_MIN_DEPTH_SPAN: f64 = 0.25;
/// Wheel points to one e-fold of zoom.
const LOG_ZOOM_PER_POINT: f64 = 0.004;
/// Column keys a lithology is usually written under, tried in order.
const LOG_LITHOLOGY_KEYS: [&str; 6] = ["lith", "litho", "lithology", "lithtype", "lith_1", "rock"];
/// Size of the azimuth compass in the log's header.
const LOG_COMPASS_SIZE: f32 = 54.0;
/// Size of the compass once the panel is too narrow to spare the full dial.
const LOG_COMPASS_MIN_SIZE: f32 = 36.0;
/// Pixels a depth tick needs before its neighbour crowds it.
const LOG_TICK_MIN_GAP: f32 = 28.0;

/// One hole drawn down the page against a depth scale, coloured as the 3D
/// view colours it; spins in plan, never tilts, the bearing sets the lean.
pub(crate) struct BoreholeLog<'a> {
    id: egui::Id,
    hole: &'a crate::model::drill_hole::DrillHole,
    dataset: &'a crate::model::drill_hole::OpenDrillHoleDataset,
}

impl<'a> BoreholeLog<'a> {
    pub(crate) fn new(id_source: impl Hash + Debug, hole: &'a crate::model::drill_hole::DrillHole, dataset: &'a crate::model::drill_hole::OpenDrillHoleDataset) -> Self {
        Self {
            id: egui::Id::new(id_source),
            hole,
            dataset,
        }
    }

    /// Draw the log into what the panel has left. Nothing here may report a
    /// width larger than the panel gave it, or the log would slide the scene.
    pub(crate) fn show(self, ui: &mut egui::Ui) {
        let Some(hole) = self.depth_range() else {
            ui.weak(tr!(literal = "This hole has no trace to draw."));
            return;
        };

        // The bearing and depth window live in egui's own per-id memory.
        let azimuth_id = self.id.with("azimuth");
        let view_id = self.id.with("depth_view");
        let mut azimuth = ui.data(|data| data.get_temp::<f32>(azimuth_id)).unwrap_or(0.0);
        // Re-clamped rather than trusted: the panel keeps its id while the
        // inspection moves, so the window may be from a different hole.
        let mut view = clamp_depth_view(ui.data(|data| data.get_temp::<(f64, f64)>(view_id)).unwrap_or(hole), hole);

        let width = ui.available_width();
        let height = ui.available_height().max(LOG_MIN_HEIGHT);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
        let painter = ui.painter_at(rect);

        // A strip of its own, or a floating compass at minimum width
        // would cover the hole it is aiming.
        let compass_size = if width < LOG_NARROW_WIDTH { LOG_COMPASS_MIN_SIZE } else { LOG_COMPASS_SIZE };
        let header = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), compass_size.min(rect.height())));
        let compass = egui::Rect::from_min_size(egui::pos2(header.right() - compass_size - 6.0, header.top() + 2.0), egui::Vec2::splat(compass_size - 4.0));

        // The compass has its own drag zone, separate from the plot's.
        let spin = ui
            .interact(compass, self.id.with("compass"), egui::Sense::click_and_drag())
            .on_hover_text(tr!(literal = "Drag to spin the view around the hole. Double-click to face north."));
        if spin.dragged() {
            azimuth = spun(azimuth, spin.drag_delta().x);
        }
        if spin.double_clicked() {
            azimuth = 0.0;
        }
        if spin.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }

        let body = egui::Rect::from_min_max(egui::pos2(rect.left(), header.bottom()), rect.max);
        let columns = log_columns((body.width() - LOG_EDGE_MARGIN).max(0.0));
        let plot = egui::Rect::from_min_max(
            egui::pos2(body.left() + columns.scale, body.top() + LOG_PLOT_TOP_MARGIN),
            egui::pos2(body.right() - LOG_EDGE_MARGIN, body.bottom() - LOG_EDGE_MARGIN),
        );

        // Over the plot: drag sideways spins, drag vertically walks the depth
        // window, wheel zooms around the depth under the cursor.
        let handle = ui.interact(body, self.id.with("body"), egui::Sense::click_and_drag());
        if handle.dragged() {
            let drag = handle.drag_delta();
            azimuth = spun(azimuth, drag.x);
            if plot.height() > 0.0 {
                let metres = f64::from(drag.y) * (view.1 - view.0) / f64::from(plot.height());
                view = clamp_depth_view((view.0 - metres, view.1 - metres), hole);
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if handle.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if handle.double_clicked() {
            view = hole;
        }
        if handle.hovered() && plot.height() > 0.0 {
            // Read and then spent, so a scroll aimed at the log zooms it.
            let wheel = ui.input_mut(|input| {
                let wheel = input.smooth_scroll_delta.y;
                if wheel != 0.0 {
                    // egui's own ScrollArea marks a spent wheel this way.
                    input.smooth_scroll_delta = egui::Vec2::ZERO;
                }
                wheel
            });
            if wheel != 0.0 {
                let anchor = match ui.input(|input| input.pointer.hover_pos()) {
                    Some(pointer) => depth_at(plot, view, pointer.y),
                    None => 0.5 * (view.0 + view.1),
                };
                view = zoomed_depth_view(hole, view, anchor, (-f64::from(wheel) * LOG_ZOOM_PER_POINT).exp());
            }
        }

        if self.draw_header(ui, &painter, header, compass, LogReadout { view, hole, azimuth }) {
            view = hole;
        }
        ui.data_mut(|data| {
            data.insert_temp(azimuth_id, azimuth);
            data.insert_temp(view_id, view);
        });

        let (top, bottom) = view;
        if !plot.is_positive() {
            draw_azimuth_compass(ui, &painter, compass, azimuth);
            return;
        }
        self.draw_scale(ui, &painter, plot, columns, top, bottom);
        let strat = egui::Rect::from_x_y_ranges(plot.left()..=plot.left() + columns.strat, plot.y_range());
        let track = egui::Rect::from_x_y_ranges(plot.right() - columns.track..=plot.right(), plot.y_range());
        if columns.strat > 0.0 {
            self.draw_strat(ui, &painter, strat, top, bottom);
        }
        if track.is_positive() {
            self.draw_column(ui, &painter, track, top, bottom, azimuth);
        }
        draw_azimuth_compass(ui, &painter, compass, azimuth);
    }

    /// The strip above the plot: reset button, depth/bearing readout, compass.
    /// Returns whether the reader asked for the whole hole back.
    fn draw_header(&self, ui: &mut egui::Ui, painter: &egui::Painter, header: egui::Rect, compass: egui::Rect, readout: LogReadout) -> bool {
        let LogReadout { view, hole, azimuth } = readout;
        let zoomed = view != hole;
        let room = egui::Rect::from_min_max(header.min, egui::pos2((compass.left() - 6.0).max(header.left()), header.bottom()));
        let mut buttons = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(self.id.with("log_header"))
                .max_rect(room)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        buttons.set_clip_rect(room.intersect(ui.clip_rect()));
        let reset = buttons
            .add_enabled(
                zoomed,
                egui::Button::new(tr!(literal = "Whole hole"))
                    .small()
                    .corner_radius(crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS),
            )
            .on_hover_text(tr!(literal = "Back to the whole hole."))
            // Disabled exactly when the log already shows the whole hole.
            .on_disabled_hover_text(tr!(
                literal = "Roll the wheel over the log to zoom in on a seam. Drag the log to spin the hole and to walk down it."
            ))
            .clicked();

        // Painted, not laid out, and dropped when the width is not there.
        let font = egui::TextStyle::Small.resolve(ui.style());
        let ink = ui.visuals().weak_text_color();
        let mut right = compass.left() - 4.0;
        for label in [
            format!("{azimuth:.0}°"),
            if zoomed {
                tr_format!(literal = "%from% to %to% m", from = format!("{:.1}", view.0), to = format!("{:.1}", view.1))
            } else {
                tr_format!(literal = "%depth% m", depth = format!("{:.1}", hole.1 - hole.0))
            },
        ] {
            let galley = painter.layout_no_wrap(label, font.clone(), ink);
            if galley.size().x > right - 6.0 - buttons.min_rect().right() {
                break;
            }
            right -= galley.size().x;
            painter.galley(egui::pos2(right, header.center().y - galley.size().y * 0.5), galley, egui::Color32::PLACEHOLDER);
            right -= 8.0;
        }
        reset
    }

    /// The depths the log spans, widened to cover any interval past the trace.
    fn depth_range(&self) -> Option<(f64, f64)> {
        let first = self.hole.trace.first()?.depth;
        let last = self.hole.trace.last()?.depth;
        let deepest = self.hole.intervals.iter().map(|interval| interval.to).fold(last, f64::max);
        (deepest > first).then_some((first, deepest))
    }

    /// Where a depth sits down the page.
    fn y_at(plot: egui::Rect, top: f64, bottom: f64, depth: f64) -> f32 {
        let t = ((depth - top) / (bottom - top)).clamp(0.0, 1.0) as f32;
        plot.top() + t * plot.height()
    }

    /// How far the hole has wandered sideways at `depth`, seen from `azimuth`.
    fn offset_at(&self, depth: f64, azimuth: f32) -> f32 {
        match self.hole.position_at_depth(depth) {
            Some(position) => self.sideways_offset(position, azimuth),
            None => 0.0,
        }
    }

    /// Sideways offset of `position` from the collar, seen from `azimuth`.
    fn sideways_offset(&self, position: glam::DVec3, azimuth: f32) -> f32 {
        let Some(collar) = self.hole.trace.first().map(|station| station.position) else {
            return 0.0;
        };
        let (east, north) = (position.x - collar.x, position.y - collar.y);
        let radians = f64::from(azimuth).to_radians();
        // Screen right is the bearing turned a quarter turn clockwise.
        (east * radians.cos() - north * radians.sin()) as f32
    }

    /// The field the strat column reads: an alias list, falling back to
    /// the dataset's first categorical field.
    fn lithology_field(&self) -> Option<&crate::model::drill_hole::DrillField> {
        let fields = &self.dataset.dataset.fields;
        fields
            .iter()
            .find(|field| LOG_LITHOLOGY_KEYS.contains(&field.key.to_ascii_lowercase().as_str()))
            .or_else(|| {
                fields
                    .iter()
                    .find(|field| matches!(field.kind, crate::model::drill_hole::DrillFieldKind::Categorical { .. }))
            })
    }

    /// Consecutive intervals reading the same lithology, merged into one
    /// run, so forty abutting records of the same rock draw as one bed.
    fn lithology_runs(&self, field: &crate::model::drill_hole::DrillField, top: f64, bottom: f64) -> Vec<(f64, f64, Option<String>)> {
        let mut runs: Vec<(f64, f64, Option<String>)> = Vec::new();
        let mut cursor = top;
        let mut sorted = self
            .hole
            .intervals
            .iter()
            .filter(|interval| interval.to > top && interval.from < bottom)
            .collect::<Vec<_>>();
        sorted.sort_by(|a, b| a.from.total_cmp(&b.from));

        for interval in sorted {
            let (from, to) = (interval.from.max(top), interval.to.min(bottom));
            if to <= from {
                continue;
            }
            let value = match interval.values.get(&field.key) {
                Some(crate::model::drill_hole::DrillValue::Category(value)) if !value.trim().is_empty() => Some(value.trim().to_owned()),
                Some(crate::model::drill_hole::DrillValue::Numeric(number)) => Some(format!("{number}")),
                _ => None,
            };
            if from > cursor + 1.0e-9 {
                push_run(&mut runs, cursor, from, None);
            }
            push_run(&mut runs, from.max(cursor), to, value);
            cursor = cursor.max(to);
        }
        if cursor < bottom - 1.0e-9 {
            push_run(&mut runs, cursor, bottom, None);
        }
        runs
    }

    /// Colour for one strat run, matched to the ribbon's own set.
    fn strat_run_color(&self, field: &crate::model::drill_hole::DrillField, code: &str) -> [f32; 3] {
        if self.dataset.color.active_field.as_deref() == Some(field.key.as_str()) {
            return self.dataset.color.category_color(code).unwrap_or([1.0, 1.0, 1.0]);
        }
        match &field.kind {
            crate::model::drill_hole::DrillFieldKind::Categorical { categories } => categories
                .iter()
                .position(|category| category == code)
                .map_or([1.0, 1.0, 1.0], crate::model::drill_hole::generated_category_color),
            crate::model::drill_hole::DrillFieldKind::Numeric { .. } => [1.0, 1.0, 1.0],
        }
    }

    fn draw_strat(&self, ui: &egui::Ui, painter: &egui::Painter, strat: egui::Rect, top: f64, bottom: f64) {
        let visuals = ui.visuals();
        let font = egui::TextStyle::Small.resolve(ui.style());
        let Some(field) = self.lithology_field() else {
            painter.text(strat.center_top(), egui::Align2::CENTER_TOP, tr!(literal = "No lithology"), font, visuals.weak_text_color());
            return;
        };
        for (from, to, value) in self.lithology_runs(field, top, bottom) {
            let block = egui::Rect::from_x_y_ranges(strat.x_range(), Self::y_at(strat, top, bottom, from)..=Self::y_at(strat, top, bottom, to));
            if block.height() <= 0.0 {
                continue;
            }
            let (fill, label) = match &value {
                Some(code) => {
                    let [red, green, blue] = self.strat_run_color(field, code);
                    (crate::rendering::color::rgba_to_color32([red, green, blue, 1.0]), code.clone())
                }
                None => (visuals.extreme_bg_color, tr!(literal = "Not logged")),
            };
            painter.rect_filled(block, 0.0, fill);
            if value.is_some() {
                hatch_placeholder(painter, block, visuals.weak_text_color().gamma_multiply(0.30));
            }
            painter.rect_stroke(block, 0.0, egui::Stroke::new(1.0, visuals.weak_text_color().gamma_multiply(0.45)), egui::StrokeKind::Inside);
            // Only drawn once its galley is measured to fit; a squeezed
            // strat column goes quiet rather than spilling onto the hole.
            if block.height() >= font.size + 3.0 {
                let ink = if value.is_some() { visuals.strong_text_color() } else { visuals.weak_text_color() };
                let galley = painter.layout_no_wrap(label, font.clone(), ink);
                if galley.size().x <= block.width() - 2.0 {
                    painter.galley(block.center() - 0.5 * galley.size(), galley, egui::Color32::PLACEHOLDER);
                }
            }
        }
        // Elided to the column's own width, or a field named at length
        // would write itself across the top of the hole beside it.
        let name = elided(painter, field.label.clone(), font, visuals.weak_text_color(), strat.width());
        painter.galley(
            egui::pos2(strat.center().x - name.size().x * 0.5, strat.top() - 4.0 - name.size().y),
            name,
            egui::Color32::PLACEHOLDER,
        );
    }

    /// The depth ticks down the left of the plot, spaced for the window
    /// that is actually showing rather than for the whole hole.
    fn draw_scale(&self, ui: &egui::Ui, painter: &egui::Painter, plot: egui::Rect, columns: LogColumns, top: f64, bottom: f64) {
        let visuals = ui.visuals();
        let step = log_tick_step(bottom - top, plot.height());
        // Decimals enough to tell adjacent ticks apart at this step.
        let places = if step.is_finite() && step > 0.0 && step < 1.0 {
            (-step.log10()).ceil().clamp(0.0, 3.0) as usize
        } else {
            0
        };
        let overhang = (columns.scale * 0.12).min(6.0);
        let gap = (columns.scale * 0.2).min(10.0);

        let first = (top / step).ceil() * step;
        // Capped independently of step: a bad interval must not hang the UI.
        let max_ticks = (plot.height() / LOG_TICK_MIN_GAP) as usize + 2;
        for index in 0..=max_ticks {
            let depth = first + index as f64 * step;
            if depth > bottom {
                break;
            }
            let y = Self::y_at(plot, top, bottom, depth);
            painter.line_segment(
                [egui::pos2(plot.left() - overhang, y), egui::pos2(plot.right(), y)],
                egui::Stroke::new(1.0, visuals.weak_text_color().gamma_multiply(0.35)),
            );
            painter.text(
                egui::pos2(plot.left() - gap, y),
                egui::Align2::RIGHT_CENTER,
                format!("{depth:.places$}"),
                egui::TextStyle::Small.resolve(ui.style()),
                visuals.weak_text_color(),
            );
        }
    }

    fn draw_column(&self, ui: &egui::Ui, painter: &egui::Painter, plot: egui::Rect, top: f64, bottom: f64, azimuth: f32) {
        let field = self.dataset.color.active_field.as_deref().and_then(|key| self.dataset.dataset.field(key));
        let centre = plot.center().x;
        let half = (LOG_COLUMN_WIDTH * 0.5).min(plot.width() * 0.5 - 1.0).max(1.0);
        let uncoloured = ui.visuals().widgets.inactive.bg_fill;

        let lean = LogLean::new(plot, top, bottom, half);

        // The unlogged hole first, so logged intervals draw on top of it.
        let mut ribbon = Vec::new();
        for pair in self.hole.trace.windows(2) {
            let [a, b] = [pair[0], pair[1]];
            if b.depth <= top || a.depth >= bottom {
                continue;
            }
            let (from, to) = (a.depth.max(top), b.depth.min(bottom));
            let span = b.depth - a.depth;
            let station_at = |depth: f64| a.position.lerp(b.position, if span > 0.0 { ((depth - a.depth) / span).clamp(0.0, 1.0) } else { 0.0 });
            let x0 = lean.x(self.sideways_offset(station_at(from), azimuth));
            let x1 = lean.x(self.sideways_offset(station_at(to), azimuth));
            ribbon.push(Self::quad_between(plot, top, bottom, from, to, x0, x1, half, uncoloured));
        }
        painter.extend(ribbon);

        for interval in &self.hole.intervals {
            if interval.to <= top || interval.from >= bottom {
                continue;
            }
            let color = field
                .and_then(|field| {
                    interval.values.get(&field.key).map(|value| {
                        let [red, green, blue] = crate::rendering::scene::drill_hole_cache::evaluate_color_for(&field.kind, value, &self.dataset.color);
                        crate::rendering::color::rgba_to_color32([red, green, blue, 1.0])
                    })
                })
                .unwrap_or(uncoloured);
            painter.add(self.quad(plot, top, bottom, interval.from, interval.to, azimuth, lean, color));
        }

        painter.rect_stroke(
            egui::Rect::from_x_y_ranges(centre - half..=centre + half, plot.y_range()),
            0.0,
            egui::Stroke::new(1.0, ui.visuals().weak_text_color().gamma_multiply(0.5)),
            egui::StrokeKind::Inside,
        );
    }

    /// One depth slice of the ribbon, leaning by however far the hole has
    /// deviated at each end of it.
    #[allow(clippy::too_many_arguments)]
    fn quad(&self, plot: egui::Rect, top: f64, bottom: f64, from: f64, to: f64, azimuth: f32, lean: LogLean, color: egui::Color32) -> egui::Shape {
        let (from, to) = (from.max(top), to.min(bottom));
        let (x0, x1) = (lean.x(self.offset_at(from, azimuth)), lean.x(self.offset_at(to, azimuth)));
        Self::quad_between(plot, top, bottom, from, to, x0, x1, lean.half, color)
    }

    #[allow(clippy::too_many_arguments)]
    fn quad_between(plot: egui::Rect, top: f64, bottom: f64, from: f64, to: f64, x0: f32, x1: f32, half: f32, color: egui::Color32) -> egui::Shape {
        let (y0, y1) = (Self::y_at(plot, top, bottom, from), Self::y_at(plot, top, bottom, to));
        egui::Shape::convex_polygon(
            vec![egui::pos2(x0 - half, y0), egui::pos2(x0 + half, y0), egui::pos2(x1 + half, y1), egui::pos2(x1 - half, y1)],
            color,
            egui::Stroke::NONE,
        )
    }
}

/// Metres of deviation as points across the track, at the scale the depth
/// axis is drawn to, and never further out than the track's own edge.
#[derive(Clone, Copy)]
struct LogLean {
    centre: f32,
    half: f32,
    points_per_metre: f32,
    limit: f32,
}

impl LogLean {
    fn new(plot: egui::Rect, top: f64, bottom: f64, half: f32) -> Self {
        let span = bottom - top;
        let points_per_metre = if span.is_finite() && span > 0.0 {
            (f64::from(plot.height()) / span) as f32
        } else {
            0.0
        };
        Self {
            centre: plot.center().x,
            half,
            points_per_metre,
            limit: (plot.width() * 0.5 - half).max(0.0),
        }
    }

    fn x(self, offset: f32) -> f32 {
        let lean = offset * self.points_per_metre;
        self.centre + if lean.is_finite() { lean.clamp(-self.limit, self.limit) } else { 0.0 }
    }
}

/// What the log's header has to report: the depth window, the hole it is
/// a window on, and the bearing it is seen from.
#[derive(Clone, Copy)]
struct LogReadout {
    view: (f64, f64),
    hole: (f64, f64),
    azimuth: f32,
}

/// How a log's width is shared between its three columns; a strat column
/// of zero means there was no room for one.
#[derive(Clone, Copy, PartialEq, Debug)]
struct LogColumns {
    scale: f32,
    strat: f32,
    track: f32,
}

/// Share `width` between the depth scale, the strat column and the hole.
/// The scale is squeezed but never dropped; the strat column goes first.
fn log_columns(width: f32) -> LogColumns {
    let width = width.max(0.0);
    let scale = LOG_SCALE_WIDTH.min(width * 0.35).max(LOG_SCALE_MIN_WIDTH.min(width));
    let rest = (width - scale).max(0.0);
    if rest < LOG_STRAT_MIN_WIDTH + LOG_TRACK_GAP + LOG_TRACK_MIN_WIDTH {
        return LogColumns { scale, strat: 0.0, track: rest };
    }
    let strat = LOG_STRAT_WIDTH.min(rest * 0.45).min(rest - LOG_TRACK_GAP - LOG_TRACK_MIN_WIDTH);
    LogColumns {
        scale,
        strat,
        track: rest - strat - LOG_TRACK_GAP,
    }
}

/// The depth a point down the page stands for, unclamped and the inverse
/// of [`BoreholeLog::y_at`].
fn depth_at(plot: egui::Rect, view: (f64, f64), y: f32) -> f64 {
    if plot.height() <= 0.0 {
        return view.0;
    }
    let t = f64::from((y - plot.top()) / plot.height());
    view.0 + t * (view.1 - view.0)
}

/// Slide a depth window back inside the hole it belongs to, keeping its
/// span, so a 5 m window read off a 300 m hole still lands sensibly on a
/// 40 m hole that replaces it.
fn clamp_depth_view(view: (f64, f64), hole: (f64, f64)) -> (f64, f64) {
    let whole = (hole.1 - hole.0).max(0.0);
    let span = (view.1 - view.0).clamp(LOG_MIN_DEPTH_SPAN.min(whole), whole);
    let top = view.0.clamp(hole.0, hole.1 - span);
    (top, top + span)
}

/// The depth window after one wheel step of `factor`, holding `anchor` still.
fn zoomed_depth_view(hole: (f64, f64), view: (f64, f64), anchor: f64, factor: f64) -> (f64, f64) {
    let whole = (hole.1 - hole.0).max(0.0);
    let span = view.1 - view.0;
    if span <= 0.0 || whole <= 0.0 {
        return hole;
    }
    let next = (span * factor).clamp(LOG_MIN_DEPTH_SPAN.min(whole), whole);
    let held = ((anchor - view.0) / span).clamp(0.0, 1.0);
    let top = anchor - held * next;
    clamp_depth_view((top, top + next), hole)
}

/// One line of text cut to `width` with an ellipsis.
fn elided(painter: &egui::Painter, text: String, font: egui::FontId, ink: egui::Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text, font, ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(0.0));
    painter.layout_job(job)
}

/// The tick spacing to read a `span` of depth over `height` points.
fn log_tick_step(span: f64, height: f32) -> f64 {
    if span <= 0.0 || height <= 0.0 {
        return 1.0;
    }
    crate::model::plot::round_up_to_series(span * f64::from(LOG_TICK_MIN_GAP) / f64::from(height), &[1.0, 2.0, 5.0, 10.0])
}

/// A bearing turned by a drag across the page, wrapped back into 0..360.
fn spun(azimuth: f32, drag_x: f32) -> f32 {
    (azimuth + drag_x * LOG_SPIN_PER_POINT).rem_euclid(360.0)
}

/// The dial showing which way the log is looked at; flat, since this view
/// has no tilt to report.
fn draw_azimuth_compass(ui: &egui::Ui, painter: &egui::Painter, rect: egui::Rect, azimuth: f32) {
    let visuals = ui.visuals();
    let centre = rect.center();
    let radius = rect.width() * 0.5 - 2.0;
    painter.circle_filled(centre, radius, visuals.extreme_bg_color.gamma_multiply(0.8));
    painter.circle_stroke(centre, radius, egui::Stroke::new(1.0, visuals.weak_text_color().gamma_multiply(0.6)));

    let font = egui::TextStyle::Small.resolve(ui.style());
    for (label, bearing, strong) in [
        (tr!(literal = "N"), 0.0_f32, true),
        (tr!(literal = "E"), 90.0, false),
        (tr!(literal = "S"), 180.0, false),
        (tr!(literal = "W"), 270.0, false),
    ] {
        // Screen angle, with the current bearing rotated to the top.
        let screen = (bearing - azimuth - 90.0).to_radians();
        let at = centre + egui::vec2(screen.cos(), screen.sin()) * (radius - 9.0);
        painter.text(
            at,
            egui::Align2::CENTER_CENTER,
            label,
            font.clone(),
            if strong { visuals.strong_text_color() } else { visuals.weak_text_color() },
        );
    }
    painter.line_segment([centre, centre + egui::vec2(0.0, -radius + 4.0)], egui::Stroke::new(2.0, visuals.strong_text_color()));
}

/// Extend the last run rather than start a new one when values and depths abut.
fn push_run(runs: &mut Vec<(f64, f64, Option<String>)>, from: f64, to: f64, value: Option<String>) {
    if to <= from + 1.0e-9 {
        return;
    }
    if let Some(last) = runs.last_mut()
        && last.2 == value
        && (from - last.1).abs() <= 1.0e-9
    {
        last.1 = to;
        return;
    }
    runs.push((from, to, value));
}

/// Diagonal hatching over a lithology block, marking it as standing in
/// for the pattern that belongs there.
fn hatch_placeholder(painter: &egui::Painter, block: egui::Rect, ink: egui::Color32) {
    const SPACING: f32 = 7.0;
    let stroke = egui::Stroke::new(1.0, ink);
    let clipped = painter.with_clip_rect(block);
    let mut offset = block.left() - block.height();
    while offset < block.right() {
        let a = egui::pos2(offset, block.bottom());
        let b = egui::pos2(offset + block.height(), block.top());
        if a.x < block.right() && b.x > block.left() {
            clipped.line_segment([a, b], stroke);
        }
        offset += SPACING;
    }
}
