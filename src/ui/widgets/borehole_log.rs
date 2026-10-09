//! The borehole log: one hole drawn down the page against a depth scale,
//! with its strat column, compass and the trace columns of [`super::log_traces`].

use std::{fmt::Debug, hash::Hash};

use super::viewport::{Outline, outlined_galley, trim_decimal_zeros};
use crate::{
    i18n::tr,
    model::drill_hole::{OpenDrillHoleDataset, ShiftDirection, UNKNOWN_NAME},
    ui::widgets::{
        context_menu::{ContextMenuAction, context_menu_popup, context_menu_popup_with_fields, context_menu_separator},
        log_traces::{self, ColumnTraces, TraceColumn, WellLogStyle},
        menu,
    },
};

/// Ink for a strat run's code. A run is filled with whatever its palette and
/// hatching give, pastel or dark, in either theme: white over a thin dark
/// halo reads on all of them.
const STRAT_LABEL_INK: egui::Color32 = egui::Color32::WHITE;

/// The halo under a strat run's code.
const STRAT_LABEL_HALO: egui::Color32 = egui::Color32::from_black_alpha(210);

/// How far seam names grow past the small style as the log zooms in.
const STRAT_LABEL_GROWTH: f32 = 1.6;

/// Zoom, in points per metre of hole, below which seam names stay at the
/// small style (about a 200 m hole shown whole)...
const STRAT_LABEL_GROW_FROM: f32 = 3.0;

/// ...and at which they reach their full growth (about a 45 m window).
const STRAT_LABEL_GROW_TO: f32 = 12.0;

/// Width of the hole's ribbon in the log track; narrow, so readings can sit
/// either side of it.
const LOG_COLUMN_WIDTH: f32 = 14.0;
/// Room under the plot for the sideways axis: tick labels, then its caption.
const LOG_AXIS_HEIGHT: f32 = 30.0;
/// Share of the track's free half-width a fitted hole may lean into.
const LOG_FIT_SHARE: f64 = 0.9;
/// Ratios a fitted squeeze is rounded up to, per decade, so the label stays
/// readable and the picture steps rather than creeps as the window moves.
const LOG_SQUEEZE_SERIES: [f64; 5] = [1.0, 2.0, 2.5, 5.0, 10.0];
/// Fixed squeezes offered in the log's right-click menu.
const LOG_SQUEEZE_CHOICES: [f64; 5] = [1.0, 2.0, 5.0, 10.0, 20.0];
/// Points a sideways tick needs before its neighbour crowds it.
const LOG_AXIS_TICK_MIN_GAP: f32 = 36.0;
/// Points a ribbon edge may pass the track's side before it counts as cut.
const LOG_CUT_TOLERANCE: f32 = 0.5;
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
/// Gap between neighbouring columns: strat, density, the hole, gamma.
const LOG_TRACK_GAP: f32 = 10.0;
/// Narrowest a well-log trace column is drawn before it is dropped.
const LOG_TRACE_MIN_WIDTH: f32 = 56.0;
/// Width a trace column grows to before the hole takes the rest.
const LOG_TRACE_WIDTH: f32 = 160.0;
/// Narrowest the hole's track is kept while trace columns stand beside it.
const LOG_TRACK_KEEP_WIDTH: f32 = 40.0;
/// Width the hole's track grows to alongside the trace columns.
const LOG_TRACK_WIDTH: f32 = 96.0;
/// Margin down the right of the log and below it.
const LOG_EDGE_MARGIN: f32 = 8.0;
/// Room above the plot for the strat column's field name.
const LOG_PLOT_TOP_MARGIN: f32 = 14.0;
/// Log width below which the header's compass shrinks to its smaller size.
const LOG_NARROW_WIDTH: f32 = 240.0;
/// Shortest the log is drawn, even with almost no panel height left; the
/// sideways axis under the plot takes its room on top of the plot's own.
const LOG_MIN_HEIGHT: f32 = 120.0 + LOG_AXIS_HEIGHT;
/// Degrees of bearing per point of horizontal drag.
const LOG_SPIN_PER_POINT: f32 = 0.5;
/// Shallowest depth window the wheel will zoom to.
const LOG_MIN_DEPTH_SPAN: f64 = 0.25;
/// Wheel points to one e-fold of zoom.
const LOG_ZOOM_PER_POINT: f64 = 0.004;
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
    /// Read the strat column from this field rather than guessing by name.
    strat_field: Option<String>,
    /// The hole's well-log traces, if any were loaded for it.
    well_logs: Option<&'a crate::model::geophysics::HoleLogs>,
    /// Whether any hole of the dataset has traces: only then is a hole
    /// without one worth a note.
    logs_loaded: bool,
    /// The drawn curves a hole being read has, by `LogKind::index`.
    reading: Option<[bool; 3]>,
    /// The depths the index says the hole's drawn curves have readings at,
    /// while the hole is read.
    logged: Option<[f64; 2]>,
    well_log_style: WellLogStyle,
}

/// The depth window a log keeps from frame to frame and hole to hole.
#[derive(Clone, Copy, PartialEq, Debug)]
enum LogWindow {
    Whole,
    Zoomed((f64, f64)),
}

impl LogWindow {
    /// What to keep of `view` on `hole`: all of it is kept as the whole, so
    /// a hole grown by its readings, or the next hole, shows whole too.
    fn kept(view: (f64, f64), hole: (f64, f64)) -> Self {
        // Relative slack for the rounding a clamp to the whole leaves.
        if view.1 - view.0 >= (hole.1 - hole.0) * (1.0 - 1.0e-9) {
            Self::Whole
        } else {
            Self::Zoomed(view)
        }
    }

    /// The window to draw `hole` with.
    fn view(kept: Option<Self>, hole: (f64, f64)) -> (f64, f64) {
        match kept {
            None | Some(Self::Whole) => hole,
            Some(Self::Zoomed(view)) => clamp_depth_view(view, hole),
        }
    }
}

/// Where a log's parts go: its columns, the notes under it and the plot,
/// below the height kept for the trace headers.
#[derive(Clone, PartialEq, Debug)]
struct LogLayout {
    columns: LogColumns,
    notes: [Option<String>; 2],
    notes_height: f32,
    top_margin: f32,
    plot: egui::Rect,
}

impl<'a> BoreholeLog<'a> {
    pub(crate) fn new(id_source: impl Hash + Debug, hole: &'a crate::model::drill_hole::DrillHole, dataset: &'a crate::model::drill_hole::OpenDrillHoleDataset) -> Self {
        Self {
            id: egui::Id::new(id_source),
            hole,
            dataset,
            strat_field: None,
            well_logs: None,
            logs_loaded: false,
            reading: None,
            logged: None,
            well_log_style: WellLogStyle::default(),
        }
    }

    pub(crate) fn strat_field(mut self, key: Option<String>) -> Self {
        self.strat_field = key;
        self
    }

    /// The hole's traces, and whether its dataset has any at all.
    pub(crate) fn well_logs(mut self, hole: Option<&'a crate::model::geophysics::HoleLogs>, loaded: bool) -> Self {
        self.well_logs = hole;
        self.logs_loaded = loaded;
        self
    }

    /// Lay the trace columns out for a hole's curves while it is read, so
    /// nothing moves when its readings land.
    pub(crate) fn reading(mut self, kinds: Option<[bool; 3]>) -> Self {
        self.reading = kinds;
        self
    }

    /// Span the index's geophysics depths while the hole is read.
    pub(crate) fn logged_depths(mut self, depths: Option<[f64; 2]>) -> Self {
        self.logged = depths;
        self
    }

    pub(crate) fn well_log_style(mut self, style: WellLogStyle) -> Self {
        self.well_log_style = style;
        self
    }

    /// Draw the log into what the panel has left. Nothing here may report a
    /// width larger than the panel gave it, or the log would slide the scene.
    /// Returns what the reader asked for, for the caller to act on.
    pub(crate) fn show(self, ui: &mut egui::Ui) -> LogOutput {
        let Some(hole) = self.depth_range() else {
            ui.weak(tr!("viewport-hole-has-no-trace-draw"));
            return LogOutput::default();
        };

        // The bearing and depth window live in egui's own per-id memory.
        let azimuth_id = self.id.with("azimuth");
        let view_id = self.id.with("depth_view");
        let squeeze_id = self.id.with("squeeze");
        let menu_id = self.id.with("trace_menu");
        let seam_menu_id = self.id.with("seam_menu");
        let mut azimuth = ui.data(|data| data.get_temp::<f32>(azimuth_id)).unwrap_or(0.0);
        let mut squeeze = ui.data(|data| data.get_temp::<LogSqueeze>(squeeze_id)).unwrap_or_default();
        // Re-clamped rather than trusted: the panel keeps its id while the
        // inspection moves, so the window may be from a different hole.
        let mut view = LogWindow::view(ui.data(|data| data.get_temp::<LogWindow>(view_id)), hole);
        let [density, gamma] = self.trace_columns();

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
            .on_hover_text(tr!("viewport-drag-spin-view-around-hole"));
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
        let strat_field = self.lithology_field();
        let line_height = ui.text_style_height(&egui::TextStyle::Small);
        let LogLayout {
            columns,
            notes,
            notes_height,
            top_margin,
            plot,
        } = self.layout([density, gamma], body, strat_field.is_some(), line_height);
        let lanes = columns.lanes(plot);
        let trace_lanes = [(density, lanes.density), (gamma, lanes.gamma)];

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

        // A right-click over a seam in the strat column opens the seam's
        // menu, over a trace column that column's own, anywhere else the
        // squeeze menu. Which one is settled at the click and held while the
        // menu stays open, with the style being edited.
        if handle.secondary_clicked() {
            let pointer = handle.interact_pointer_pos();
            let seam = pointer
                .zip(lanes.strat.zip(strat_field))
                .filter(|(pointer, (strat, _))| strat.contains(*pointer))
                .and_then(|(pointer, (_, field))| {
                    self.seam_at(field, depth_at(plot, view, pointer.y)).map(|(interval, name)| SeamMenu {
                        field: field.key.clone(),
                        name,
                        interval,
                    })
                });
            let menu = pointer
                .filter(|_| seam.is_none())
                .and_then(|pointer| trace_column_at(pointer, plot, trace_lanes))
                .map(|traces| TraceMenu {
                    column: traces.column,
                    draft: self.well_log_style,
                    custom: log_traces::column_range(&self.well_log_style, &traces),
                });
            ui.data_mut(|data| {
                data.insert_temp(menu_id, menu);
                data.insert_temp(seam_menu_id, seam);
            });
        }
        let mut output = LogOutput::default();
        let stored = ui.data(|data| data.get_temp::<Option<TraceMenu>>(menu_id)).flatten();
        let open = stored.and_then(|menu| column_traces(menu.column, density, gamma).map(|traces| (menu, traces)));
        let seam = ui.data(|data| data.get_temp::<Option<SeamMenu>>(seam_menu_id)).flatten();
        let menu = if let Some(seam) = seam {
            context_menu_popup(&handle, seam.name.clone(), |ui| {
                for (label, every_hole) in [(tr!("viewport-rename-seam-in-this-hole"), false), (tr!("viewport-rename-seam-in-every-hole"), true)] {
                    if ContextMenuAction::new(label).show(ui).clicked() {
                        output.rename = Some(SeamRename {
                            field: seam.field.clone(),
                            name: seam.name.clone(),
                            interval: seam.interval,
                            every_hole,
                        });
                        ui.close();
                    }
                }
                // The hole's names slide one run, all of them or from the
                // clicked horizon; with no column there is nothing to slide
                // along, and from a name outside it there is no horizon.
                let column = self.dataset.color.strat_column(&seam.field);
                let in_column = seam.name != UNKNOWN_NAME && column.contains(&seam.name);
                context_menu_separator(ui);
                for (label, direction, from) in [
                    (tr!("viewport-move-up-from-here"), ShiftDirection::Up, Some(seam.interval)),
                    (tr!("viewport-move-all-up"), ShiftDirection::Up, None),
                    (tr!("viewport-move-all-down"), ShiftDirection::Down, None),
                    (tr!("viewport-move-down-from-here"), ShiftDirection::Down, Some(seam.interval)),
                ] {
                    let enabled = !column.is_empty() && (from.is_none() || in_column);
                    let response = ContextMenuAction::new(label).enabled(enabled).show(ui);
                    let response = match (enabled, column.is_empty()) {
                        (true, _) => response,
                        (false, true) => response.on_disabled_hover_text(tr!("viewport-field-has-no-strat-column")),
                        (false, false) => response.on_disabled_hover_text(tr!("viewport-name-not-in-strat-column")),
                    };
                    if response.clicked() {
                        output.shift = Some(NameShift {
                            field: seam.field.clone(),
                            direction,
                            from,
                        });
                        ui.close();
                    }
                }
                context_menu_separator(ui);
            });
            None
        } else if let Some((mut menu, traces)) = open {
            let auto = log_traces::auto_range(menu.column, &traces);
            let shown = context_menu_popup_with_fields(&handle, log_traces::menu_title(menu.column), |ui| {
                log_traces::menu(ui, menu.column, &mut menu.draft, &mut menu.custom, auto);
            });
            shown.map(|_| {
                // The one place an edit is saved: once it differs from what
                // is saved and the pointer is up. A custom range reaches the
                // draft only when its edit is finished, and a drag across the
                // colour wheel saves once, on release.
                menu.draft = menu.draft.sanitized();
                if menu.draft != self.well_log_style && !ui.input(|input| input.pointer.any_down()) {
                    output.saved = Some(menu.draft);
                }
                menu
            })
        } else if stored.is_some() {
            // A trace menu was open but its column no longer has traces (the
            // hole changed underneath it). Close the popup outright rather
            // than switching its contents to the squeeze menu.
            egui::Popup::close_id(ui.ctx(), egui::Popup::default_response_id(&handle));
            None
        } else {
            context_menu_popup(&handle, tr!("viewport-sideways-scale"), |ui| {
                let fit = ContextMenuAction::new(tr!("viewport-fit-hole-track"))
                    .checked(squeeze == LogSqueeze::Fit)
                    .show(ui)
                    .on_hover_text(tr!("viewport-squeeze-sideways-just-enough-keep"));
                if fit.clicked() {
                    squeeze = LogSqueeze::Fit;
                    ui.close();
                }
                for ratio in LOG_SQUEEZE_CHOICES {
                    let label = if ratio == 1.0 {
                        tr!("viewport-1-1-true-shape")
                    } else {
                        tr!("viewport-1-ratio", ratio = crate::model::plot::format_quantity(ratio, 0).to_string())
                    };
                    if ContextMenuAction::new(label).checked(squeeze == LogSqueeze::Fixed(ratio)).show(ui).clicked() {
                        squeeze = LogSqueeze::Fixed(ratio);
                        ui.close();
                    }
                }
            });
            None
        };
        // Drawn with the style being edited while its menu is open, so a
        // change shows before it is saved.
        let style = menu.map_or(self.well_log_style, |menu| menu.draft);
        // Held still over a trace column, the readings under the pointer.
        if menu.is_none()
            && !handle.dragged()
            && let Some(traces) = handle.hover_pos().and_then(|pointer| trace_column_at(pointer, plot, trace_lanes))
            && let Some(pointer) = handle.hover_pos()
        {
            let depth = depth_at(plot, view, pointer.y);
            handle.clone().on_hover_ui_at_pointer(|ui| log_traces::readout(ui, &traces, depth, &style));
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
            data.insert_temp(view_id, LogWindow::kept(view, hole));
            data.insert_temp(squeeze_id, squeeze);
            data.insert_temp(menu_id, menu);
        });

        let (top, bottom) = view;
        if !plot.is_positive() {
            draw_azimuth_compass(ui, &painter, compass, azimuth);
            return output;
        }
        self.draw_scale(ui, &painter, plot, columns, top, bottom);
        if let (Some(strat), Some(field)) = (lanes.strat, strat_field) {
            self.draw_strat(ui, &painter, strat, field, top, bottom);
        }
        for (traces, lane) in trace_lanes {
            if let (Some(traces), Some(lane)) = (traces, lane) {
                let range = log_traces::column_range(&style, &traces);
                let heading = egui::Rect::from_x_y_ranges(lane.x_range(), (plot.top() - top_margin + 2.0)..=(plot.top() - 2.0));
                log_traces::draw_header(ui, &painter, heading, &traces, range, &style);
                log_traces::draw_body(ui, &painter, lane, view, &traces, range, &style);
            }
        }
        // No lean at all when the track has no room to lean in: better an
        // empty track than a ratio the picture cannot keep.
        let track = lanes.track;
        let path = visible_path(self.hole, view);
        let collar = self.hole.trace.first().map_or(glam::DVec2::ZERO, |station| station.position.truncate());
        if let Some(stretch) = plan_stretch(&path, collar)
            && let Some(lean) = LogLean::new(track, view, azimuth, squeeze, stretch)
        {
            self.draw_column(ui, &painter, lean, &path);
            Self::draw_offset_axis(ui, &painter, lean);
        }
        if let Some(recorded) = self.recorded_depth(hole).filter(|depth| (top..=bottom).contains(depth)) {
            draw_recorded_depth(ui, &painter, plot, view, recorded);
        }
        let area = egui::Rect::from_min_max(
            egui::pos2(body.left() + 4.0, plot.bottom() + LOG_AXIS_HEIGHT),
            egui::pos2(body.right() - LOG_EDGE_MARGIN, plot.bottom() + LOG_AXIS_HEIGHT + notes_height),
        );
        draw_trace_notes(ui, &painter, area, notes, line_height);
        draw_azimuth_compass(ui, &painter, compass, azimuth);
        output
    }

    /// The density and gamma columns: as read, or laid out ahead while the
    /// hole is read.
    fn trace_columns(&self) -> [Option<ColumnTraces<'a>>; 2] {
        [TraceColumn::Density, TraceColumn::Gamma].map(|column| match self.reading {
            Some(kinds) => ColumnTraces::reading(column, kinds),
            None => ColumnTraces::of(column, self.well_logs),
        })
    }

    /// Where the log's parts go in `body` for its trace columns `traces`. It
    /// reads which columns the hole has, never their readings, so it is the
    /// same while they are read.
    fn layout(&self, traces: [Option<ColumnTraces<'a>>; 2], body: egui::Rect, strat: bool, line_height: f32) -> LogLayout {
        let [density, gamma] = traces.map(|traces| traces.is_some());
        let columns = log_columns((body.width() - LOG_EDGE_MARGIN).max(0.0), strat, density, gamma);
        // Each note gets a line of its own across the log, under the axis.
        let notes = self.trace_notes([density, gamma], columns);
        let notes_height = notes.iter().flatten().count() as f32 * line_height;
        // Tall enough for the trace headers when there are any.
        let top_margin = traces
            .into_iter()
            .zip([columns.density, columns.gamma])
            .filter_map(|(traces, width)| traces.filter(|_| width > 0.0))
            .map(|traces| log_traces::header_height(traces.column, line_height) + 4.0)
            .fold(LOG_PLOT_TOP_MARGIN, f32::max);
        let plot = egui::Rect::from_min_max(
            egui::pos2(body.left() + columns.scale, body.top() + top_margin),
            egui::pos2(body.right() - LOG_EDGE_MARGIN, body.bottom() - LOG_EDGE_MARGIN - LOG_AXIS_HEIGHT - notes_height),
        );
        LogLayout {
            columns,
            notes,
            notes_height,
            top_margin,
            plot,
        }
    }

    /// Quiet notes for under the log: a curve this hole has no log for,
    /// said only when the dataset has logs at all, and a column the panel is
    /// too narrow for. `held` is whether the hole has density, then gamma.
    fn trace_notes(&self, held: [bool; 2], columns: LogColumns) -> [Option<String>; 2] {
        let missing = match held {
            _ if !self.logs_loaded => None,
            [false, false] => Some(tr!("viewport-no-downhole-geophysics-hole")),
            [false, true] => Some(tr!("viewport-no-density-log-hole")),
            [true, false] => Some(tr!("viewport-no-gamma-log-hole")),
            [true, true] => None,
        };
        let narrow = match (held[0] && columns.density <= 0.0, held[1] && columns.gamma <= 0.0) {
            (true, true) => Some(tr!("viewport-widen-panel-show-density-gamma")),
            (true, false) => Some(tr!("viewport-widen-panel-show-density")),
            (false, true) => Some(tr!("viewport-widen-panel-show-gamma")),
            (false, false) => None,
        };
        [missing, narrow]
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
            .add(menu::MenuButton::new(tr!("common-reset-view")).enabled(zoomed))
            .on_hover_text(tr!("viewport-back-whole-log"))
            // Disabled exactly when the log already shows the whole hole.
            .on_disabled_hover_text(tr!("viewport-roll-wheel-over-log-zoom"))
            .clicked();

        // Painted, not laid out, and dropped when the width is not there.
        let font = egui::TextStyle::Small.resolve(ui.style());
        let ink = ui.visuals().weak_text_color();
        let mut right = compass.left() - 4.0;
        for label in [
            format!("{azimuth:.0}°"),
            if zoomed {
                tr!("viewport-from-m", from = format!("{:.1}", view.0), to = format!("{:.1}", view.1))
            } else {
                tr!("log-traces-depth-m", depth = format!("{:.1}", hole.1 - hole.0))
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

    /// The depths the log spans, widened to cover any interval or well log
    /// past the trace: the index's depths while the hole is read, the
    /// traces' once they are shown.
    fn depth_range(&self) -> Option<(f64, f64)> {
        let first = self.hole.trace.first()?.depth;
        let last = self.hole.trace.last()?.depth;
        let logs = || {
            self.well_logs
                .into_iter()
                .flat_map(|logs| crate::model::geophysics::LogKind::ALL.into_iter().filter_map(|kind| logs.trace(kind)))
        };
        let [shallowest, deepest] = self.logged.unwrap_or_else(|| {
            [
                logs().map(|trace| trace.start_depth()).fold(f64::INFINITY, f64::min),
                logs().map(|trace| trace.end_depth()).fold(f64::NEG_INFINITY, f64::max),
            ]
        });
        let first = first.min(shallowest);
        let deepest = self.hole.intervals.iter().map(|interval| interval.to).fold(last, f64::max).max(deepest);
        (deepest > first).then_some((first, deepest))
    }

    /// Where the hole's trace ends, when the log in `range` runs on below
    /// it. A collar alone has no length recorded to mark.
    fn recorded_depth(&self, range: (f64, f64)) -> Option<f64> {
        let [first, .., last] = self.hole.trace.as_slice() else { return None };
        (last.depth > first.depth && range.1 > last.depth).then_some(last.depth)
    }

    /// Where a depth sits down the page.
    fn y_at(plot: egui::Rect, top: f64, bottom: f64, depth: f64) -> f32 {
        let t = ((depth - top) / (bottom - top)).clamp(0.0, 1.0) as f32;
        plot.top() + t * plot.height()
    }

    /// The field the strat column reads: the one chosen in the panel, else an
    /// alias list, falling back to the dataset's first categorical field.
    ///
    /// A chosen field is taken only while it is one of this dataset's and
    /// still categorical: the panel holds one choice while the inspection
    /// moves, and a numeric field here would draw a block per number.
    fn lithology_field(&self) -> Option<&crate::model::drill_hole::DrillField> {
        strat_field_of(self.dataset, self.strat_field.as_deref())
    }

    /// The seam name the strat column shows at `depth`, exactly as the
    /// interval holds it: the first interval down the hole covering the
    /// depth, as [`Self::lithology_runs`] draws it.
    fn seam_at(&self, field: &crate::model::drill_hole::DrillField, depth: f64) -> Option<(usize, String)> {
        self.hole
            .intervals
            .iter()
            .enumerate()
            .filter(|(_, interval)| interval.from <= depth && depth < interval.to)
            .min_by(|a, b| a.1.from.total_cmp(&b.1.from))
            .and_then(|(index, interval)| match interval.values.get(&field.key) {
                Some(crate::model::drill_hole::DrillValue::Category(name)) if !name.trim().is_empty() => Some((index, name.clone())),
                _ => None,
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

    fn draw_strat(&self, ui: &egui::Ui, painter: &egui::Painter, strat: egui::Rect, field: &crate::model::drill_hole::DrillField, top: f64, bottom: f64) {
        let visuals = ui.visuals();
        let font = egui::TextStyle::Small.resolve(ui.style());
        let runs = self.lithology_runs(field, top, bottom);
        // Every seam name in view shares one size, set by the zoom rather
        // than by each block, so a thick and a thin seam read alike: the
        // small style over the whole hole, growing to a ceiling as the view
        // closes in, and never so big that the widest name overflows the
        // column. A name too long even at the small style gets an ellipsis.
        let room = strat.width() - 4.0;
        let widest = runs
            .iter()
            .filter_map(|(_, _, value)| value.as_ref())
            .map(|code| painter.layout_no_wrap(code.clone(), font.clone(), STRAT_LABEL_INK).size().x)
            .fold(0.0_f32, f32::max);
        let points_per_metre = strat.height() / ((bottom - top) as f32).max(f32::EPSILON);
        let zoom = ((points_per_metre - STRAT_LABEL_GROW_FROM) / (STRAT_LABEL_GROW_TO - STRAT_LABEL_GROW_FROM)).clamp(0.0, 1.0);
        let size = (font.size * (1.0 + (STRAT_LABEL_GROWTH - 1.0) * zoom))
            .min(font.size * room / widest.max(1.0))
            .max(font.size);
        let label_font = egui::FontId::new(size, font.family.clone());
        // The bottom of the last seam name drawn, so a thin seam's name that
        // would land on its neighbour's is left out rather than stacked.
        let mut named_to = f32::NEG_INFINITY;
        // Names go on after every block, so a name taller than its seam runs
        // over the neighbouring blocks instead of being cut by them.
        let mut names = Vec::new();
        for (from, to, value) in runs {
            let block = egui::Rect::from_x_y_ranges(strat.x_range(), Self::y_at(strat, top, bottom, from)..=Self::y_at(strat, top, bottom, to));
            if block.height() <= 0.0 {
                continue;
            }
            // A coded run is its palette colour under hatching, its code in
            // white over a halo; an unlogged one is plain, its note quiet.
            let (label, ink, outline) = match &value {
                Some(code) => {
                    let [red, green, blue] = strat_run_color(&self.dataset.color, field, code);
                    painter.rect_filled(block, 0.0, crate::rendering::color::rgba_to_color32([red, green, blue, 1.0]));
                    hatch_placeholder(painter, block, visuals.weak_text_color().gamma_multiply(0.30));
                    (code.clone(), STRAT_LABEL_INK, Some(Outline::Halo(STRAT_LABEL_HALO)))
                }
                None => {
                    painter.rect_filled(block, 0.0, visuals.extreme_bg_color);
                    (tr!("viewport-not-logged"), visuals.weak_text_color(), None)
                }
            };
            painter.rect_stroke(block, 0.0, egui::Stroke::new(1.0, visuals.weak_text_color().gamma_multiply(0.45)), egui::StrokeKind::Inside);
            // Centred even on a block thinner than the text, so a thin seam
            // is still named, but kept inside the column, clear of its heading.
            if let Some(outline) = outline {
                let mut galley = painter.layout_no_wrap(label.clone(), label_font.clone(), ink);
                if galley.size().x > room {
                    galley = elided(painter, label, label_font.clone(), ink, room);
                }
                let mut origin = block.center() - 0.5 * galley.size();
                origin.y = origin.y.min(strat.bottom() - galley.size().y).max(strat.top());
                if origin.y >= named_to + 1.0 {
                    named_to = origin.y + galley.size().y;
                    names.push((origin, galley, outline));
                }
            } else if block.height() >= font.size + 3.0 {
                // An unlogged run's note is quiet: shown only where it fits.
                let galley = painter.layout_no_wrap(label, font.clone(), ink);
                if galley.size().x <= block.width() - 2.0 {
                    painter.galley(block.center() - 0.5 * galley.size(), galley, egui::Color32::PLACEHOLDER);
                }
            }
        }
        for (origin, galley, outline) in names {
            outlined_galley(painter, origin, egui::Align2::LEFT_TOP, galley, STRAT_LABEL_INK, outline);
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
        let step = log_tick_step(bottom - top, plot.height(), LOG_TICK_MIN_GAP);
        let places = tick_places(step);
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

    /// The hole down its track, `path` being its visible stretch, with its
    /// sides traced. A fixed squeeze can carry it past the track's side; it
    /// is cut there rather than pinned, and the side it left by is marked.
    fn draw_column(&self, ui: &egui::Ui, painter: &egui::Painter, lean: LogLean, path: &[(f64, glam::DVec3)]) {
        let painter = painter.with_clip_rect(lean.plot);
        let visuals = ui.visuals();
        let field = self.dataset.color.active_field.as_deref().and_then(|key| self.dataset.dataset.field(key));
        let uncoloured = visuals.widgets.inactive.bg_fill;
        let (top, bottom) = lean.view;

        // The unlogged hole first, so logged intervals draw on top of it;
        // only as far as the survey reaches.
        let toe = self.hole.trace.last().map_or(f64::NEG_INFINITY, |station| station.depth);
        let runs = lean.runs(path);
        painter.extend(runs.iter().filter(|(from, _)| from.0 < toe).map(|&(from, to)| lean.slab(from, to, uncoloured)));

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
            let end = |depth: f64| (depth, self.hole.position_at_depth(depth).map_or(lean.plot.center().x, |position| lean.x(position)));
            painter.add(lean.slab(end(interval.from.max(top)), end(interval.to.min(bottom)), color));
        }

        // The ribbon's own sides, following the hole down the window.
        if let Some(&(start, _)) = runs.first() {
            let stroke = egui::Stroke::new(1.0, visuals.weak_text_color().gamma_multiply(0.5));
            for side in [-lean.half, lean.half] {
                let edge = std::iter::once(start)
                    .chain(runs.iter().map(|&(_, to)| to))
                    .map(|(depth, x)| egui::pos2(x + side, lean.y(depth)));
                painter.add(egui::Shape::line(edge.collect(), stroke));
            }
        }

        let warn = visuals.warn_fg_color;
        for (spans, edge, outward) in [(lean.cut(&runs, -1.0), lean.plot.left(), -1.0), (lean.cut(&runs, 1.0), lean.plot.right(), 1.0)] {
            for (y0, y1) in spans {
                // A bar down the depths the hole is gone for, and an arrow
                // at its middle pointing the way it went.
                let bar = edge - outward * 1.5;
                painter.line_segment([egui::pos2(bar, y0), egui::pos2(bar, y1)], egui::Stroke::new(3.0, warn));
                let (middle, back) = (0.5 * (y0 + y1), edge - outward * 7.0);
                painter.add(egui::Shape::convex_polygon(
                    vec![egui::pos2(edge, middle), egui::pos2(back, middle - 4.0), egui::pos2(back, middle + 4.0)],
                    warn,
                    egui::Stroke::NONE,
                ));
            }
        }
    }

    /// Plan metres from the collar across the foot of the track, the
    /// bearing screen-right looks toward, and the squeeze over the top of it.
    fn draw_offset_axis(ui: &egui::Ui, painter: &egui::Painter, lean: LogLean) {
        let track = lean.plot;
        let visuals = ui.visuals();
        let font = egui::TextStyle::Small.resolve(ui.style());
        let ink = visuals.weak_text_color();
        let foot = track.bottom();
        painter.line_segment(
            [egui::pos2(track.left(), foot), egui::pos2(track.right(), foot)],
            egui::Stroke::new(1.0, ink.gamma_multiply(0.6)),
        );

        let (step, ticks) = lean.ticks();
        let places = tick_places(step);
        let mut label_bottom = foot + 4.0;
        for (x, reading) in ticks {
            painter.line_segment([egui::pos2(x, foot), egui::pos2(x, foot + 4.0)], egui::Stroke::new(1.0, ink.gamma_multiply(0.6)));
            let galley = painter.layout_no_wrap(format!("{reading:.places$}"), font.clone(), ink);
            let half_width = galley.size().x * 0.5;
            // Labels spill into the track gap at most, never onto a neighbour.
            if x - half_width < track.left() - LOG_TRACK_GAP * 0.5 || x + half_width > track.right() + LOG_EDGE_MARGIN * 0.5 {
                continue;
            }
            label_bottom = label_bottom.max(foot + 5.0 + galley.size().y);
            painter.galley(egui::pos2(x - half_width, foot + 5.0), galley, egui::Color32::PLACEHOLDER);
        }

        // Screen right is the bearing turned a quarter turn clockwise.
        let bearing = format!("{:03}", ((lean.azimuth + 90.0).round() as i32).rem_euclid(360));
        let caption = elided(
            painter,
            tr!("viewport-m-from-collar-toward-bearing", bearing = bearing.to_string()),
            font.clone(),
            ink,
            track.width(),
        );
        painter.galley(
            egui::pos2(track.center().x - caption.size().x * 0.5, label_bottom + 1.0),
            caption,
            egui::Color32::PLACEHOLDER,
        );

        let ratio = elided(
            painter,
            // Written as the plot sheet writes its 1:N, bar a fitted 1:2.5.
            tr!(
                "viewport-h-1-ratio",
                ratio = trim_decimal_zeros(crate::model::plot::format_quantity(lean.ratio, 1)).to_string()
            ),
            font,
            ink,
            track.width(),
        );
        painter.galley(
            egui::pos2(track.right() - ratio.size().x, track.top() - 4.0 - ratio.size().y),
            ratio,
            egui::Color32::PLACEHOLDER,
        );
    }
}

/// How the log squeezes the hole's sideways wander against its depth.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
enum LogSqueeze {
    /// Just enough to keep the visible stretch of hole inside the track.
    #[default]
    Fit,
    /// Sideways metres drawn at one part in this many of the depth scale.
    Fixed(f64),
}

/// Where a stretch of hole lies in plan: its middle, the furthest any of it
/// lies from that middle, and the collar its axis is read from. Plan only,
/// so spinning the view moves none of it.
#[derive(Clone, Copy, PartialEq, Debug)]
struct LogStretch {
    middle: glam::DVec2,
    reach: f64,
    collar: glam::DVec2,
}

/// The hole down the depth window `view` as (depth, position) samples: the
/// window's two ends and every station between. The trace is straight
/// between stations and held at the toe past the last, as
/// [`crate::model::drill_hole::DrillHole::position_at_depth`] reads it.
fn visible_path(hole: &crate::model::drill_hole::DrillHole, view: (f64, f64)) -> Vec<(f64, glam::DVec3)> {
    let (top, bottom) = view;
    let at = |depth: f64| hole.position_at_depth(depth).map(|position| (depth, position));
    let inside = hole
        .trace
        .iter()
        .filter(|station| station.depth > top && station.depth < bottom)
        .map(|station| (station.depth, station.position));
    at(top).into_iter().chain(inside).chain(at(bottom)).collect()
}

/// The plan stretch `path` covers. Its middle is where it lies on average
/// down its depth; it runs straight between samples, so the samples bound
/// how far it strays from that. `None` for an empty or non-finite path.
fn plan_stretch(path: &[(f64, glam::DVec3)], collar: glam::DVec2) -> Option<LogStretch> {
    // Summed from the first sample, so eastings in the millions keep their
    // precision.
    let base = path.first()?.1.truncate();
    let (mut sum, mut weight) = (glam::DVec2::ZERO, 0.0);
    for pair in path.windows(2) {
        let length = pair[1].0 - pair[0].0;
        if length > 0.0 {
            sum += (pair[0].1.truncate() + pair[1].1.truncate() - 2.0 * base) * 0.5 * length;
            weight += length;
        }
    }
    let middle = if weight > 0.0 { base + sum / weight } else { base };
    let reach = path.iter().map(|(_, position)| position.truncate().distance(middle)).fold(0.0, f64::max);
    (middle.is_finite() && reach.is_finite()).then_some(LogStretch { middle, reach, collar })
}

/// A straight run of the hole down its track, as its (depth, x) ends.
type LogRun = ((f64, f32), (f64, f32));

/// Where the hole sits across its track: plan metres along the way screen
/// right faces, from the middle of the visible hole on the track's centre
/// line, at the depth axis's own scale divided by `ratio`. The ratio is
/// never below 1: the log may squeeze the lean but never stretch it.
#[derive(Clone, Copy, PartialEq, Debug)]
struct LogLean {
    /// The hole's track, and the depth window drawn down it.
    plot: egui::Rect,
    view: (f64, f64),
    /// Half the ribbon's width.
    half: f32,
    azimuth: f32,
    /// Unit plan vector, east and north, toward screen right.
    across: glam::DVec2,
    stretch: LogStretch,
    points_per_metre: f32,
    ratio: f64,
}

impl LogLean {
    /// The lean for `stretch` seen from `azimuth`, down `plot` over the depth
    /// window `view`. `None` when there is no room to lean in or no depth to
    /// scale by: nothing is drawn then, and no ratio is claimed.
    fn new(plot: egui::Rect, view: (f64, f64), azimuth: f32, squeeze: LogSqueeze, stretch: LogStretch) -> Option<Self> {
        let span = view.1 - view.0;
        if !(plot.is_positive() && span.is_finite() && span > 0.0) {
            return None;
        }
        let half = (LOG_COLUMN_WIDTH * 0.5).min(plot.width() * 0.5 - 1.0).max(1.0);
        let room = f64::from(plot.width() * 0.5 - half) * LOG_FIT_SHARE;
        let depth_points_per_metre = f64::from(plot.height()) / span;
        let ratio = match squeeze {
            LogSqueeze::Fit => fitted_squeeze(stretch.reach, depth_points_per_metre, room)?,
            LogSqueeze::Fixed(ratio) => (room > 0.0 && ratio.is_finite()).then_some(ratio.max(1.0))?,
        };
        let points_per_metre = (depth_points_per_metre / ratio) as f32;
        if !(points_per_metre.is_finite() && points_per_metre > 0.0) {
            return None;
        }
        // Screen right is the bearing turned a quarter turn clockwise.
        let radians = f64::from(azimuth).to_radians();
        Some(Self {
            plot,
            view,
            half,
            azimuth,
            across: glam::DVec2::new(radians.cos(), -radians.sin()),
            stretch,
            points_per_metre,
            ratio,
        })
    }

    /// Plan metres from the middle of the visible hole to `position`, along
    /// the way screen right faces.
    fn offset(self, position: glam::DVec3) -> f64 {
        (position.truncate() - self.stretch.middle).dot(self.across)
    }

    /// Where `position` is drawn across the track.
    fn x(self, position: glam::DVec3) -> f32 {
        self.x_at_offset(self.offset(position))
    }

    fn x_at_offset(self, offset: f64) -> f32 {
        self.plot.center().x + offset as f32 * self.points_per_metre
    }

    /// The offset drawn at `x`; the inverse of [`Self::x_at_offset`].
    fn offset_at_x(self, x: f32) -> f64 {
        f64::from((x - self.plot.center().x) / self.points_per_metre)
    }

    /// Plan metres from the collar to the track's centre line, along the
    /// way screen right faces: what an offset gains when read from the
    /// collar instead of the middle.
    fn collar_shift(self) -> f64 {
        (self.stretch.middle - self.stretch.collar).dot(self.across)
    }

    /// The sideways axis's ticks, with their spacing: where each is drawn,
    /// and its reading in plan metres from the collar, so a reading keeps
    /// its meaning wherever the window is.
    fn ticks(self) -> (f64, Vec<(f32, f64)>) {
        let track = self.plot;
        let step = log_tick_step(f64::from(track.width()) / f64::from(self.points_per_metre), track.width(), LOG_AXIS_TICK_MIN_GAP);
        let shift = self.collar_shift();
        let first = ((self.offset_at_x(track.left()) + shift) / step).ceil() * step;
        // Capped independently of step, as the depth scale is.
        let max_ticks = (track.width() / LOG_AXIS_TICK_MIN_GAP) as usize + 2;
        let ticks = (0..=max_ticks)
            .map(|index| first + index as f64 * step)
            .map(|reading| (self.x_at_offset(reading - shift), reading))
            .take_while(|(x, _)| x.is_finite() && *x <= track.right() + 0.5)
            // Snapped, or rounding writes the collar's own tick as "-0".
            .map(|(x, reading)| (x, if reading.abs() < step * 1.0e-6 { 0.0 } else { reading }))
            .collect();
        (step, ticks)
    }

    /// The straight runs of `path` down the track.
    fn runs(self, path: &[(f64, glam::DVec3)]) -> Vec<LogRun> {
        path.windows(2).map(|pair| ((pair[0].0, self.x(pair[0].1)), (pair[1].0, self.x(pair[1].1)))).collect()
    }

    /// Where `depth` sits down the track.
    fn y(self, depth: f64) -> f32 {
        BoreholeLog::y_at(self.plot, self.view.0, self.view.1, depth)
    }

    /// One depth slice of the ribbon, between its (depth, x) ends.
    fn slab(self, (from, x0): (f64, f32), (to, x1): (f64, f32), color: egui::Color32) -> egui::Shape {
        let (y0, y1, half) = (self.y(from), self.y(to), self.half);
        egui::Shape::convex_polygon(
            vec![egui::pos2(x0 - half, y0), egui::pos2(x0 + half, y0), egui::pos2(x1 + half, y1), egui::pos2(x1 - half, y1)],
            color,
            egui::Stroke::NONE,
        )
    }

    /// The (top, bottom) stretches down the page over which the ribbon of
    /// `runs` passes the track's left side (`side` -1) or right side (1).
    fn cut(self, runs: &[LogRun], side: f32) -> Vec<(f32, f32)> {
        let edge = if side < 0.0 { self.plot.left() } else { self.plot.right() };
        // Positive once the ribbon's outer edge is past the side.
        let over = |x: f32| side * (x - edge) + self.half - LOG_CUT_TOLERANCE;
        let mut spans: Vec<(f32, f32)> = Vec::new();
        for &((from, x0), (to, x1)) in runs {
            let Some(span) = outside_span((self.y(from), over(x0)), (self.y(to), over(x1))) else {
                continue;
            };
            match spans.last_mut() {
                Some(last) if span.0 <= last.1 + LOG_CUT_TOLERANCE => last.1 = last.1.max(span.1),
                _ => spans.push(span),
            }
        }
        spans
    }
}

/// The squeeze, depth metres per sideways metre on the page, that fits a
/// stretch lying within `reach` plan metres of its middle into `room` points
/// either side of the track's centre line, with depth drawn at
/// `points_per_metre`. 1 when it already fits, else rounded up to a
/// readable ratio; `None` when there is no room or nothing to scale by.
fn fitted_squeeze(reach: f64, points_per_metre: f64, room: f64) -> Option<f64> {
    let needed = reach * points_per_metre / room;
    if !(room > 0.0 && points_per_metre > 0.0 && reach >= 0.0 && needed.is_finite()) {
        return None;
    }
    Some(if needed > 1.0 {
        crate::model::plot::round_up_to_series(needed, &LOG_SQUEEZE_SERIES)
    } else {
        1.0
    })
}

/// The part of a straight run from `a` to `b`, each a (y, over) pair, over
/// which `over` is positive, as a (top, bottom) span of y.
fn outside_span((y0, over0): (f32, f32), (y1, over1): (f32, f32)) -> Option<(f32, f32)> {
    match (over0 > 0.0, over1 > 0.0) {
        (true, true) => Some((y0, y1)),
        (false, false) => None,
        (first, _) => {
            let crossing = y0 + (y1 - y0) * over0 / (over0 - over1);
            Some(if first { (y0, crossing) } else { (crossing, y1) })
        }
    }
}

/// Decimals enough to tell adjacent ticks `step` apart.
fn tick_places(step: f64) -> usize {
    if step.is_finite() && step > 0.0 && step < 1.0 {
        (-step.log10()).ceil().clamp(0.0, 3.0) as usize
    } else {
        0
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

/// How a log's width is shared between its columns, left to right after the
/// depth scale: strat, density, the hole, gamma. A width of zero means the
/// column is dropped, for want of room or of data.
#[derive(Clone, Copy, PartialEq, Debug)]
struct LogColumns {
    scale: f32,
    strat: f32,
    density: f32,
    track: f32,
    gamma: f32,
}

/// The rects a log's columns take across its plot; `None` for one dropped.
#[derive(Clone, Copy, PartialEq, Debug)]
struct LogLanes {
    strat: Option<egui::Rect>,
    density: Option<egui::Rect>,
    track: egui::Rect,
    gamma: Option<egui::Rect>,
}

impl LogColumns {
    /// Lay the columns out across `plot`, gapped where both neighbours stand.
    fn lanes(self, plot: egui::Rect) -> LogLanes {
        let lane = |left: f32, width: f32| (width > 0.0).then(|| egui::Rect::from_x_y_ranges(left..=left + width, plot.y_range()));
        let strat = lane(plot.left(), self.strat);
        let after = |lane: Option<egui::Rect>, from: f32| lane.map_or(from, |lane| lane.right() + LOG_TRACK_GAP);
        let density = lane(after(strat, plot.left()), self.density);
        let gamma = lane(plot.right() - self.gamma, self.gamma);
        let track_left = after(density, after(strat, plot.left()));
        let track_right = gamma.map_or(plot.right(), |gamma| gamma.left() - LOG_TRACK_GAP);
        LogLanes {
            strat,
            density,
            track: egui::Rect::from_x_y_ranges(track_left..=track_right.max(track_left), plot.y_range()),
            gamma,
        }
    }
}

/// The columns a narrow log keeps, first to last, as indices into its
/// (strat, density, gamma) columns: it gives up gamma first, then strat,
/// then density. The depth scale is squeezed but never dropped, and the
/// hole's own track never goes.
const LOG_KEEP_ORDER: [usize; 3] = [1, 0, 2];

/// Share `width` between the depth scale, the strat column when there is a
/// field to fill it, the trace columns the hole has data for, and the hole.
/// Columns are kept in [`LOG_KEEP_ORDER`] while each still fits at its
/// narrowest beside those already kept, so a strat column that lost its place
/// to density comes back once density is gone; then each grows toward its own
/// width and the hole takes whatever is left.
fn log_columns(width: f32, strat: bool, density: bool, gamma: bool) -> LogColumns {
    let width = width.max(0.0);
    let scale = LOG_SCALE_WIDTH.min(width * 0.35).max(LOG_SCALE_MIN_WIDTH.min(width));
    let rest = (width - scale).max(0.0);
    let dropped = LogColumns {
        scale,
        strat: 0.0,
        density: 0.0,
        track: rest,
        gamma: 0.0,
    };

    // Strat, density, gamma.
    let wants = [strat, density, gamma];
    let minimum = [LOG_STRAT_MIN_WIDTH, LOG_TRACE_MIN_WIDTH, LOG_TRACE_MIN_WIDTH];
    let track_minimum = |kept: &[bool; 3]| if kept[1] || kept[2] { LOG_TRACK_KEEP_WIDTH } else { LOG_TRACK_MIN_WIDTH };
    let need = |kept: &[bool; 3]| track_minimum(kept) + (0..3).filter(|&index| kept[index]).map(|index| minimum[index] + LOG_TRACK_GAP).sum::<f32>();
    let mut kept = [false; 3];
    for index in LOG_KEEP_ORDER {
        let mut trial = kept;
        trial[index] = wants[index];
        if need(&trial) <= rest {
            kept = trial;
        }
    }
    if kept == [false; 3] {
        return dropped;
    }
    if !kept[1] && !kept[2] {
        // The strat column and the hole alone share as they always have.
        let strat = LOG_STRAT_WIDTH.min(rest * 0.45).min(rest - LOG_TRACK_GAP - LOG_TRACK_MIN_WIDTH);
        return LogColumns {
            strat,
            track: rest - strat - LOG_TRACK_GAP,
            ..dropped
        };
    }

    // Past every minimum, the spare is shared in proportion to how far each
    // column is from its own width; the hole keeps anything beyond that.
    let wanted = [LOG_STRAT_WIDTH, LOG_TRACE_WIDTH, LOG_TRACE_WIDTH];
    let track_min = track_minimum(&kept);
    let spare = rest - need(&kept);
    let short: f32 = (0..3).filter(|&index| kept[index]).map(|index| wanted[index] - minimum[index]).sum::<f32>() + (LOG_TRACK_WIDTH - track_min);
    let share = if short > 0.0 { (spare / short).min(1.0) } else { 0.0 };
    let grown = |index: usize| if kept[index] { minimum[index] + (wanted[index] - minimum[index]) * share } else { 0.0 };
    let [strat, density, gamma] = [grown(0), grown(1), grown(2)];
    let gaps = LOG_TRACK_GAP * kept.iter().filter(|kept| **kept).count() as f32;
    LogColumns {
        scale,
        strat,
        density,
        track: (rest - strat - density - gamma - gaps).max(track_min),
        gamma,
    }
}

/// The field a hole's strat column reads in `dataset`: `chosen` while it is
/// one of the dataset's and still categorical, else
/// [`default_strat_field`]. The log and the strat column tab both read
/// through here, so the two never disagree.
pub(crate) fn strat_field_of<'a>(dataset: &'a OpenDrillHoleDataset, chosen: Option<&str>) -> Option<&'a crate::model::drill_hole::DrillField> {
    chosen
        .and_then(|key| dataset.dataset.field(key))
        .filter(|field| is_categorical(field))
        .or_else(|| default_strat_field(&dataset.dataset.fields, &dataset.color.working_sections, dataset.color.strat_field.as_deref()))
}

/// Parts of a key a stratigraphic field is usually written under, the most
/// detailed first.
const STRAT_KEY_HINTS: [&str; 3] = ["code", "seam", "ply"];

fn is_categorical(field: &crate::model::drill_hole::DrillField) -> bool {
    matches!(field.kind, crate::model::drill_hole::DrillFieldKind::Categorical { .. })
}

/// The strat field when none is picked: `recorded`, the field an import
/// noted (the parent, seam field when it found one); else the categorical
/// field holding working sections; else the first whose key holds a part of
/// [`STRAT_KEY_HINTS`], tried in order; else the first categorical field.
/// A lithology field is never taken unless it holds sections: it is read
/// only when picked by hand.
pub(crate) fn default_strat_field<'a>(
    fields: &'a [crate::model::drill_hole::DrillField],
    sections: &[crate::model::drill_hole::WorkingSection],
    recorded: Option<&str>,
) -> Option<&'a crate::model::drill_hole::DrillField> {
    let lithology = |field: &crate::model::drill_hole::DrillField| crate::model::strat_order::is_lithology_key(&field.key);
    let mut candidates = fields.iter().filter(|field| is_categorical(field));
    let noted = recorded.and_then(|key| fields.iter().find(|field| field.key == key && is_categorical(field) && !lithology(field)));
    let sectioned = || {
        fields
            .iter()
            .filter(|field| is_categorical(field))
            .find(|field| sections.iter().any(|section| section.field == field.key))
    };
    noted
        .or_else(sectioned)
        .or_else(|| {
            STRAT_KEY_HINTS.iter().find_map(|hint| {
                fields
                    .iter()
                    .filter(|field| is_categorical(field) && !lithology(field))
                    .find(|field| field.key.to_ascii_lowercase().contains(hint))
            })
        })
        .or_else(|| candidates.find(|field| !lithology(field)))
}

/// What the reader asked of the log this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct LogOutput {
    /// A trace style the reader finished editing, to save.
    pub(crate) saved: Option<WellLogStyle>,
    /// A seam the reader chose to rename.
    pub(crate) rename: Option<SeamRename>,
    /// The hole's names the reader chose to slide one run along the hole.
    pub(crate) shift: Option<NameShift>,
}

/// A shift picked from the strat column's menu: the field read, which way
/// its names move, and the interval clicked when only its run and those on
/// that side move.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NameShift {
    pub(crate) field: String,
    pub(crate) direction: ShiftDirection,
    pub(crate) from: Option<usize>,
}

/// A seam picked from the strat column's menu: the field and name it is
/// read from, the interval clicked, and whether every hole of the set is
/// meant or only the clicked horizon.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SeamRename {
    pub(crate) field: String,
    pub(crate) name: String,
    pub(crate) interval: usize,
    pub(crate) every_hole: bool,
}

/// The seam menu while it is open: the seam right-clicked in the strat column.
#[derive(Clone, PartialEq, Debug)]
struct SeamMenu {
    field: String,
    name: String,
    /// The interval clicked, by index in the hole.
    interval: usize,
}

/// A trace column's right-click menu while it is open: the column, the
/// style being edited, and the custom range being typed.
#[derive(Clone, Copy, PartialEq, Debug)]
struct TraceMenu {
    column: TraceColumn,
    draft: WellLogStyle,
    custom: [f32; 2],
}

/// The trace column under `pos`, if any: inside a column's width and down
/// the plot itself, so the headers and the axis below open no trace menu.
fn trace_column_at<'t>(pos: egui::Pos2, plot: egui::Rect, lanes: [(Option<ColumnTraces<'t>>, Option<egui::Rect>); 2]) -> Option<ColumnTraces<'t>> {
    if !plot.y_range().contains(pos.y) {
        return None;
    }
    lanes
        .into_iter()
        .find_map(|(traces, lane)| traces.filter(|_| lane.is_some_and(|lane| lane.x_range().contains(pos.x))))
}

/// A dashed line across the plot where the hole ends, its label on a plate so
/// the traces crossing it cannot hide it.
fn draw_recorded_depth(ui: &egui::Ui, painter: &egui::Painter, plot: egui::Rect, view: (f64, f64), depth: f64) {
    let visuals = ui.visuals();
    let ink = visuals.text_color();
    let y = BoreholeLog::y_at(plot, view.0, view.1, depth);
    painter.extend(egui::Shape::dashed_line(
        &[egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
        egui::Stroke::new(1.5, ink),
        6.0,
        4.0,
    ));
    let label = tr!("viewport-depth-m-hole-end", depth = format!("{depth:.1}"));
    let galley = painter.layout_no_wrap(label, egui::TextStyle::Small.resolve(ui.style()), ink);
    let above = y - galley.size().y - 3.0;
    let top = if above >= plot.top() { above } else { y + 3.0 };
    let at = egui::pos2(plot.right() - galley.size().x - 4.0, top);
    let plate = egui::Rect::from_min_size(at, galley.size()).expand(2.0);
    painter.rect_filled(plate, crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS, visuals.extreme_bg_color.gamma_multiply(0.8));
    painter.galley(at, galley, egui::Color32::PLACEHOLDER);
}

/// The notes from [`BoreholeLog::trace_notes`], one weak line each across
/// `area`, elided to its width.
fn draw_trace_notes(ui: &egui::Ui, painter: &egui::Painter, area: egui::Rect, notes: [Option<String>; 2], line_height: f32) {
    let font = egui::TextStyle::Small.resolve(ui.style());
    let ink = ui.visuals().weak_text_color();
    for (index, note) in notes.into_iter().flatten().enumerate() {
        let galley = elided(painter, note, font.clone(), ink, area.width());
        painter.galley(egui::pos2(area.left(), area.top() + index as f32 * line_height), galley, egui::Color32::PLACEHOLDER);
    }
}

/// The traces of whichever column `column` names.
fn column_traces<'a>(column: TraceColumn, density: Option<ColumnTraces<'a>>, gamma: Option<ColumnTraces<'a>>) -> Option<ColumnTraces<'a>> {
    match column {
        TraceColumn::Density => density,
        TraceColumn::Gamma => gamma,
    }
}

/// Colour for one strat run, matched to the ribbon's own set: the ribbon's
/// field reads through the same working-section mapping the ribbon does, so
/// a code coloured by its section shows that colour in both.
pub(crate) fn strat_run_color(color: &crate::model::drill_hole::DrillColorState, field: &crate::model::drill_hole::DrillField, code: &str) -> [f32; 3] {
    if color.active_field.as_deref() == Some(field.key.as_str()) {
        let value = crate::model::drill_hole::DrillValue::Category(code.to_owned());
        return crate::rendering::scene::drill_hole_cache::evaluate_color_for(&field.kind, &value, color);
    }
    match &field.kind {
        crate::model::drill_hole::DrillFieldKind::Categorical { categories } => categories
            .iter()
            .position(|category| category == code)
            .map_or([1.0, 1.0, 1.0], crate::model::drill_hole::generated_category_color),
        crate::model::drill_hole::DrillFieldKind::Numeric { .. } => [1.0, 1.0, 1.0],
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
    // Rounding can put the lowest top a hair above the hole's own.
    let top = view.0.clamp(hole.0, (hole.1 - span).max(hole.0));
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

/// The tick spacing to read a `span` of metres over `length` points, with
/// ticks no closer than `min_gap` points.
fn log_tick_step(span: f64, length: f32, min_gap: f32) -> f64 {
    if span <= 0.0 || length <= 0.0 {
        return 1.0;
    }
    crate::model::plot::round_up_to_series(span * f64::from(min_gap) / f64::from(length), &[1.0, 2.0, 5.0, 10.0])
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
        (tr!("viewport-n"), 0.0_f32, true),
        (tr!("viewport-e"), 90.0, false),
        (tr!("viewport-s"), 180.0, false),
        (tr!("viewport-w"), 270.0, false),
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
