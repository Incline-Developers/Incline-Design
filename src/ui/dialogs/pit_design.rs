//! Pit Design: what its dialog holds while it is open, and the dialog.
//!
//! The dialog edits [`PitDesignSettings`]; `App::update_pit_design_preview`
//! builds the design from them in the background whenever they change, and
//! the viewport draws it while the dialog is open.

use glam::DVec2;

use crate::{
    i18n::tr,
    model::{
        ObjectId,
        pit_design::{Design, Direction, End, Layout, Problem, RampSpec, Spec, Stop},
        triangulation::TriangulationId,
    },
    ui::{
        elements::properties::read_only_row,
        state::{EditorState, SelectionPickTool, UiCommand, UiProjectView},
        widgets::{
            hint::Hint,
            menu::{self, DragableMenu, MenuButton, MenuField, MenuFieldBool, MenuFieldCombo, MenuFieldF64, MenuFieldU32},
        },
    },
};

const FIELD_WIDTH: f32 = 170.0;

/// Where a design stops, as the dialog offers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StopMode {
    Benches,
    Elevation,
    Daylight,
}

/// Everything the dialog sets. Two designs from the same string and equal
/// settings are the same design.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PitDesignSettings {
    pub(crate) direction: Direction,
    pub(crate) bench_height: f64,
    /// Degrees from horizontal.
    pub(crate) face_angle: f64,
    pub(crate) berm_width: f64,
    pub(crate) wall_radius: f64,
    pub(crate) ramp: bool,
    pub(crate) ramp_width: f64,
    /// Rise over run, as a percentage.
    pub(crate) ramp_gradient: f64,
    /// Where the ramp leaves the string, in plan; its first point if none
    /// has been picked.
    pub(crate) ramp_start: Option<DVec2>,
    pub(crate) climbs_clockwise: bool,
    pub(crate) switchback: bool,
    /// Benches climbed between turns.
    pub(crate) switchback_every: u32,
    pub(crate) inside_radius: f64,
    pub(crate) stop: StopMode,
    pub(crate) benches: u32,
    pub(crate) stop_elevation: f64,
    pub(crate) topography: Option<TriangulationId>,
    pub(crate) surface: bool,
    pub(crate) trim_to_topography: bool,
}

impl Default for PitDesignSettings {
    fn default() -> Self {
        Self {
            direction: Direction::Up,
            bench_height: 15.0,
            face_angle: 70.0,
            berm_width: 8.0,
            wall_radius: 15.0,
            ramp: true,
            ramp_width: 25.0,
            ramp_gradient: 10.0,
            ramp_start: None,
            climbs_clockwise: false,
            switchback: false,
            switchback_every: 3,
            inside_radius: 15.0,
            stop: StopMode::Benches,
            benches: 8,
            stop_elevation: 0.0,
            topography: None,
            surface: true,
            trim_to_topography: true,
        }
    }
}

impl PitDesignSettings {
    /// The design engine's settings, the ramp starting at `default_start`
    /// unless one has been picked.
    pub(crate) fn spec(&self, default_start: DVec2) -> Spec {
        Spec {
            direction: self.direction,
            bench_height: self.bench_height,
            face_angle: self.face_angle,
            berm_width: self.berm_width,
            wall_radius: self.wall_radius,
            ramp: self.ramp.then(|| RampSpec {
                width: self.ramp_width,
                gradient: self.ramp_gradient / 100.0,
                start: self.ramp_start.unwrap_or(default_start),
                climbs_clockwise: self.climbs_clockwise,
                layout: if self.switchback {
                    Layout::Switchback {
                        every: self.switchback_every,
                        inside_radius: self.inside_radius,
                    }
                } else {
                    Layout::Spiral
                },
            }),
            stop: match self.stop {
                StopMode::Benches => Stop::Benches(self.benches),
                StopMode::Elevation => Stop::Elevation(self.stop_elevation),
                StopMode::Daylight => Stop::Daylight,
            },
        }
    }

    /// Whether the surface is trimmed to the topography: only a design that
    /// daylights against one is.
    pub(crate) fn trims(&self) -> bool {
        self.surface && self.trim_to_topography && self.stop == StopMode::Daylight && self.topography.is_some()
    }
}

/// What a preview is built from: the document as it was, the string and the
/// settings.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PreviewKey {
    pub(crate) revision: u64,
    pub(crate) base: Option<ObjectId>,
    pub(crate) settings: PitDesignSettings,
}

/// Why there is no design to show.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum NoDesign {
    /// No string picked yet.
    NoString,
    /// The picked string is not closed round some area.
    NotClosed,
    /// The settings cannot make a design.
    Settings(Problem),
    /// Daylighting with no topography chosen.
    NoTopography,
    /// The topography chosen is not loaded.
    TopographyNotLoaded(String),
}

/// A finished preview: the design, and the string it was built from.
#[derive(Clone, Debug)]
pub(crate) struct Preview {
    pub(crate) design: Result<Design, NoDesign>,
    /// The string's level and how far its points stray from it.
    pub(crate) base_elevation: f64,
    pub(crate) base_unevenness: f64,
    /// Where the ramp was asked to start, on the string, for the viewport's
    /// marker.
    pub(crate) ramp_start: Option<glam::DVec3>,
}

/// The dialog's state.
#[derive(Clone, Debug, Default)]
pub(crate) struct PitDesignDraft {
    pub(crate) base: Option<ObjectId>,
    pub(crate) settings: PitDesignSettings,
    /// The viewport is waiting for a click on the string where the ramp
    /// starts.
    pub(crate) picking_ramp_start: bool,
    /// The last preview built, and what it was built from.
    pub(crate) preview: Option<Preview>,
    pub(crate) built_for: Option<PreviewKey>,
    /// The preview being built, by its job's token.
    pub(crate) building: Option<(u64, PreviewKey)>,
    pub(crate) next_token: u64,
}

impl PitDesignDraft {
    /// The design shown, if there is one and it is current.
    pub(crate) fn current_design(&self) -> Option<&Design> {
        if self.building.is_some() {
            return None;
        }
        self.preview.as_ref().and_then(|preview| preview.design.as_ref().ok())
    }
}

fn direction_label(direction: Direction) -> String {
    match direction {
        Direction::Up => tr!("pit-design-up"),
        Direction::Down => tr!("pit-design-down"),
    }
}

fn stop_label(stop: StopMode) -> String {
    match stop {
        StopMode::Benches => tr!("pit-design-stop-benches"),
        StopMode::Elevation => tr!("pit-design-stop-elevation"),
        StopMode::Daylight => tr!("pit-design-stop-daylight"),
    }
}

pub(crate) fn no_design_text(reason: &NoDesign) -> String {
    match reason {
        NoDesign::NoString => tr!("pit-design-pick-string-first"),
        NoDesign::NotClosed => tr!("pit-design-problem-string"),
        NoDesign::NoTopography => tr!("pit-design-problem-no-topography"),
        NoDesign::TopographyNotLoaded(name) => tr!("pit-design-problem-topography-not-loaded", name = name.clone()),
        NoDesign::Settings(problem) => match problem {
            Problem::BenchHeight => tr!("pit-design-problem-bench-height"),
            Problem::FaceAngle => tr!("pit-design-problem-face-angle"),
            Problem::BermWidth => tr!("pit-design-problem-berm-width"),
            Problem::RampWidth => tr!("pit-design-problem-ramp-width"),
            Problem::RampGradient => tr!("pit-design-problem-ramp-gradient"),
            Problem::Switchback => tr!("pit-design-problem-switchback"),
            Problem::Benches => tr!("pit-design-problem-benches"),
            Problem::DaylightDown => tr!("pit-design-problem-daylight-down"),
            Problem::StopElevation => tr!("pit-design-problem-stop-elevation"),
        },
    }
}

/// Why a design ended where it did, in words.
pub(crate) fn end_text(design: &Design) -> String {
    let rl = format!("{:.1}", design.final_elevation());
    match design.end {
        End::Complete => tr!("pit-design-end-complete"),
        End::Daylight => tr!("pit-design-end-daylight", rl = rl),
        End::OutOfRoom => tr!("pit-design-end-out-of-room", rl = rl),
        End::RampTooLong => tr!("pit-design-end-ramp-too-long", rl = rl),
        End::LeftGround => tr!("pit-design-end-left-ground", rl = rl),
    }
}

/// Whether a design stopped short of what was asked of it.
fn stopped_short(design: &Design) -> bool {
    matches!(design.end, End::OutOfRoom | End::RampTooLong | End::LeftGround)
}

pub(crate) fn draw_pit_design_dialog(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
    // Out of the way while the viewport picks for it.
    if editor.selection_pick.is_some() {
        return;
    }
    let Some(draft) = editor.pit_design.as_mut() else {
        return;
    };
    if draft.picking_ramp_start {
        return;
    }
    let mut open = true;
    let (mut apply, mut close) = (false, false);
    DragableMenu::new("pit_design_dialog", tr!("pit-design-title"))
        .open(&mut open)
        .min_width(400.0)
        .max_width(420.0)
        .show(ui.ctx(), |ui| {
            let settings = &mut draft.settings;
            let string_text = if draft.base.is_some() {
                tr!("pit-design-string-chosen")
            } else {
                tr!("pit-design-string-none")
            };
            if menu::picked_selection_field(ui, tr!("pit-design-string"), string_text, tr!("pit-design-string-help"), FIELD_WIDTH) {
                commands.push(UiCommand::BeginSelectionPick(SelectionPickTool::PitDesignString));
            }
            let chosen = direction_label(settings.direction);
            MenuFieldCombo::new(
                "pit_design_direction",
                tr!("pit-design-direction"),
                &mut settings.direction,
                chosen,
                [Direction::Up, Direction::Down].map(|direction| (direction, direction_label(direction).into())),
            )
            .width(FIELD_WIDTH)
            .help_text(tr!("pit-design-direction-help"))
            .show(ui);

            menu::menu_section(ui, tr!("pit-design-benches-section"));
            MenuFieldF64::new(tr!("pit-design-bench-height"), &mut settings.bench_height, 0.1..=1000.0)
                .width(FIELD_WIDTH)
                .speed(0.1)
                .suffix(" m")
                .show(ui);
            MenuFieldF64::new(tr!("pit-design-face-angle"), &mut settings.face_angle, 1.0..=89.0)
                .width(FIELD_WIDTH)
                .speed(0.5)
                .suffix("°")
                .show(ui);
            MenuFieldF64::new(tr!("pit-design-berm-width"), &mut settings.berm_width, 0.0..=1000.0)
                .width(FIELD_WIDTH)
                .speed(0.1)
                .suffix(" m")
                .show(ui);
            MenuFieldF64::new(tr!("pit-design-wall-radius"), &mut settings.wall_radius, 0.0..=10000.0)
                .width(FIELD_WIDTH)
                .speed(0.5)
                .suffix(" m")
                .help_text(tr!("pit-design-wall-radius-help"))
                .show(ui);

            menu::menu_section(ui, tr!("pit-design-stop-section"));
            // Building down there is no ground to daylight against.
            let stops: Vec<StopMode> = match settings.direction {
                Direction::Up => vec![StopMode::Benches, StopMode::Elevation, StopMode::Daylight],
                Direction::Down => vec![StopMode::Benches, StopMode::Elevation],
            };
            if !stops.contains(&settings.stop) {
                settings.stop = StopMode::Benches;
            }
            let chosen = stop_label(settings.stop);
            MenuFieldCombo::new(
                "pit_design_stop",
                tr!("pit-design-stop"),
                &mut settings.stop,
                chosen,
                stops.into_iter().map(|stop| (stop, stop_label(stop).into())),
            )
            .width(FIELD_WIDTH)
            .help_text(tr!("pit-design-stop-help"))
            .show(ui);
            match settings.stop {
                StopMode::Benches => {
                    MenuFieldU32::new(tr!("pit-design-benches"), &mut settings.benches, 1..=u32::MAX)
                        .width(FIELD_WIDTH)
                        .show(ui);
                }
                StopMode::Elevation => {
                    MenuFieldF64::new(tr!("pit-design-stop-rl"), &mut settings.stop_elevation, f64::MIN..=f64::MAX)
                        .width(FIELD_WIDTH)
                        .speed(0.5)
                        .suffix(" m")
                        .show(ui);
                }
                StopMode::Daylight => {}
            }
            if settings.stop == StopMode::Daylight {
                let surfaces: Vec<(TriangulationId, &str)> = project.triangulations.iter().map(|entry| (entry.id, entry.name.as_str())).collect();
                let chosen = settings
                    .topography
                    .and_then(|id| surfaces.iter().find(|(surface, _)| *surface == id).map(|(_, name)| (*name).to_owned()))
                    .unwrap_or_else(|| tr!("pit-design-topography-none"));
                MenuFieldCombo::new(
                    "pit_design_topography",
                    tr!("pit-design-topography"),
                    &mut settings.topography,
                    chosen,
                    std::iter::once((None, tr!("pit-design-topography-none").into())).chain(surfaces.iter().map(|(id, name)| (Some(*id), (*name).into()))),
                )
                .width(FIELD_WIDTH)
                .help_text(tr!("pit-design-topography-help"))
                .show(ui);
            }

            menu::menu_section(ui, tr!("pit-design-ramp-section"));
            MenuFieldBool::new(tr!("pit-design-ramp"), &mut settings.ramp).show(ui);
            if settings.ramp {
                MenuFieldF64::new(tr!("pit-design-ramp-width"), &mut settings.ramp_width, 0.1..=1000.0)
                    .width(FIELD_WIDTH)
                    .speed(0.1)
                    .suffix(" m")
                    .show(ui);
                MenuFieldF64::new(tr!("pit-design-ramp-gradient"), &mut settings.ramp_gradient, 0.1..=100.0)
                    .width(FIELD_WIDTH)
                    .speed(0.1)
                    .suffix(" %")
                    .help_text(tr!("pit-design-ramp-gradient-help"))
                    .show(ui);
                let start_text = settings.ramp_start.map_or_else(
                    || tr!("pit-design-ramp-start-default"),
                    |start| tr!("pit-design-ramp-start-picked", x = format!("{:.1}", start.x), y = format!("{:.1}", start.y)),
                );
                let mut pick = false;
                MenuField::new(tr!("pit-design-ramp-start"))
                    .help_text(tr!("pit-design-ramp-start-help"))
                    .show(ui, |ui, row_height, _| {
                        ui.allocate_ui_with_layout(egui::vec2(FIELD_WIDTH, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            let pick_label = tr!("pit-design-pick");
                            let pick_width = menu::natural_button_width(ui, &pick_label, 52.0);
                            let text_width = FIELD_WIDTH - ui.spacing().item_spacing.x - pick_width;
                            ui.allocate_ui_with_layout(egui::vec2(text_width, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_min_width(text_width);
                                ui.add(egui::Label::new(start_text.clone()).truncate()).hint(start_text.clone());
                            });
                            pick = ui.add(MenuButton::new(pick_label).min_width(pick_width).enabled(draft.base.is_some())).clicked();
                        })
                        .response
                    });
                if pick {
                    commands.push(UiCommand::BeginPitRampStartPick);
                }
                let turn_label = |clockwise: bool| if clockwise { tr!("pit-design-clockwise") } else { tr!("pit-design-anticlockwise") };
                let chosen = turn_label(settings.climbs_clockwise);
                MenuFieldCombo::new(
                    "pit_design_climbs",
                    tr!("pit-design-climbs"),
                    &mut settings.climbs_clockwise,
                    chosen,
                    [false, true].map(|clockwise| (clockwise, turn_label(clockwise).into())),
                )
                .width(FIELD_WIDTH)
                .help_text(tr!("pit-design-climbs-help"))
                .show(ui);
                let layout_label = |switchback: bool| if switchback { tr!("pit-design-switchback") } else { tr!("pit-design-spiral") };
                let chosen = layout_label(settings.switchback);
                MenuFieldCombo::new(
                    "pit_design_layout",
                    tr!("pit-design-layout"),
                    &mut settings.switchback,
                    chosen,
                    [false, true].map(|switchback| (switchback, layout_label(switchback).into())),
                )
                .width(FIELD_WIDTH)
                .help_text(tr!("pit-design-layout-help"))
                .show(ui);
                if settings.switchback {
                    MenuFieldU32::new(tr!("pit-design-switchback-every"), &mut settings.switchback_every, 1..=u32::MAX)
                        .width(FIELD_WIDTH)
                        .help_text(tr!("pit-design-switchback-every-help"))
                        .show(ui);
                    MenuFieldF64::new(tr!("pit-design-inside-radius"), &mut settings.inside_radius, 0.0..=10000.0)
                        .width(FIELD_WIDTH)
                        .speed(0.5)
                        .suffix(" m")
                        .help_text(tr!("pit-design-inside-radius-help"))
                        .show(ui);
                }
            }

            menu::menu_section(ui, tr!("pit-design-output-section"));
            MenuFieldBool::new(tr!("pit-design-surface"), &mut settings.surface)
                .help_text(tr!("pit-design-surface-help"))
                .show(ui);
            if settings.surface && settings.stop == StopMode::Daylight {
                MenuFieldBool::new(tr!("pit-design-trim"), &mut settings.trim_to_topography)
                    .help_text(tr!("pit-design-trim-help"))
                    .show(ui);
            }

            menu::menu_section(ui, tr!("pit-design-result-section"));
            draw_result(ui, draft);

            ui.add_space(4.0);
            let can_apply = draft.current_design().is_some_and(|design| design.benches > 0);
            let cancelled = menu::dialog_cancelled(ui.ctx());
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("edit-apply")).primary().enabled(can_apply)).clicked() {
                    apply = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || cancelled {
                    close = true;
                }
            });
        });
    if apply {
        commands.push(UiCommand::CommitPitDesign);
    } else if close || !open {
        editor.pit_design = None;
        commands.push(UiCommand::ClosePitDesign);
    }
}

/// What the settings come to: the design's figures, or why there is none.
fn draw_result(ui: &mut egui::Ui, draft: &PitDesignDraft) {
    let warning = ui.visuals().warn_fg_color;
    if draft.building.is_some() && draft.preview.is_none() {
        menu::menu_note(ui, tr!("pit-design-designing"));
        return;
    }
    let Some(preview) = draft.preview.as_ref() else {
        menu::menu_note(ui, tr!("pit-design-pick-string-first"));
        return;
    };
    match &preview.design {
        Err(reason) => {
            ui.colored_label(warning, no_design_text(reason));
        }
        Ok(design) => {
            read_only_row(ui, &tr!("pit-design-benches-built"), &design.benches.to_string());
            read_only_row(ui, &tr!("pit-design-final-rl"), &format!("{:.1} m", design.final_elevation()));
            if draft.settings.ramp {
                read_only_row(ui, &tr!("pit-design-ramp-length"), &format!("{:.0} m", design.ramp_length));
            }
            read_only_row(ui, &tr!("pit-design-volume"), &format!("{:.0} m³", design.volume));
            let ended = end_text(design);
            if stopped_short(design) {
                ui.colored_label(warning, ended);
            } else {
                read_only_row(ui, &tr!("pit-design-ended"), &ended);
            }
            if design.ramp_start_moved > 0.0 {
                ui.colored_label(warning, tr!("pit-design-start-moved", distance = format!("{:.0}", design.ramp_start_moved)));
            }
            if preview.base_unevenness > 0.01 {
                ui.colored_label(
                    warning,
                    tr!(
                        "pit-design-uneven-string",
                        distance = format!("{:.2}", preview.base_unevenness),
                        rl = format!("{:.2}", preview.base_elevation)
                    ),
                );
            }
            if draft.building.is_some() {
                menu::menu_note(ui, tr!("pit-design-designing"));
            }
        }
    }
}
