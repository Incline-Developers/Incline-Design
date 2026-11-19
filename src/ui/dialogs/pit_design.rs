//! Pit Design: the window that edits a pit design object, and what it holds
//! while it is open.
//!
//! The window edits a copy of what the object is designed with
//! ([`PitDesignDraft::working`]). Once no field is mid-edit,
//! `App::sync_pit_design_draft` writes the copy to the object as one undo
//! step, and takes the object's version back when undo or redo changes it.
//! `App::update_pit_design_preview` works the copy out in the background
//! whenever it changes, and the viewport draws the result over the object
//! until the object's own design catches up.

use glam::{DVec2, DVec3};

use crate::{
    i18n::tr,
    model::{
        ObjectId, PolyVertex,
        pit_design::{
            Design, Direction, End, Problem,
            object::{PitObject, ScenarioId, SurfaceStanding},
            settings::{PitDesignSettings, StopMode},
        },
        triangulation::{GeometryVersion, TriangulationId},
    },
    ui::{
        elements::properties::read_only_row,
        state::{EditorState, SelectionPickTool, UiCommand, UiProjectView},
        widgets::{
            hint::Hint,
            menu::{self, DragableMenu, MenuButton, MenuField, MenuFieldBool, MenuFieldCombo, MenuFieldF64, MenuFieldText, MenuFieldU32},
        },
    },
};

const FIELD_WIDTH: f32 = 190.0;

/// What a preview is worked out from. Two previews from equal keys are the
/// same.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PreviewKey {
    pub(crate) verts: Vec<PolyVertex>,
    pub(crate) settings: PitDesignSettings,
    /// The version of the ground it daylights against, when it does.
    pub(crate) ground: Option<GeometryVersion>,
}

/// Why there is no design to show.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum NoDesign {
    /// The string is not closed round some area.
    NotClosed,
    /// The settings cannot make a design.
    Settings(Problem),
    /// Daylighting with no topography chosen.
    NoTopography,
    /// The topography chosen is not loaded.
    TopographyNotLoaded(String),
}

/// A finished preview: the design, and the string it was worked out from.
#[derive(Clone, Debug)]
pub(crate) struct Preview {
    pub(crate) design: Result<Design, NoDesign>,
    /// The string's level and how far its points stray from it.
    pub(crate) base_elevation: f64,
    pub(crate) base_unevenness: f64,
    /// Where the ramp was asked to start, on the string, for the viewport's
    /// marker.
    pub(crate) ramp_start: Option<DVec3>,
}

/// What the window shows of its object beyond what it edits.
#[derive(Clone, Debug, Default)]
pub(crate) struct DraftFacts {
    /// The surface it built, while that exists.
    pub(crate) surface: Option<(TriangulationId, String)>,
    pub(crate) surface_standing: SurfaceStanding,
    /// Whether the object's own design is what it comes to now.
    pub(crate) fresh: bool,
}

/// The window's state.
#[derive(Clone, Debug, Default)]
pub(crate) struct PitDesignDraft {
    /// The pit design object open, if there is one.
    object: Option<ObjectId>,
    /// What the object was designed with as last seen.
    pub(crate) synced: Option<PitObject>,
    /// The copy the window edits.
    pub(crate) working: Option<PitObject>,
    pub(crate) facts: DraftFacts,
    /// No field is mid-edit: no pointer held down and no text field typed
    /// in, so the copy can be written to the object.
    pub(crate) settled: bool,
    /// The viewport is waiting for a click on the string where the ramp
    /// starts.
    pub(crate) picking_ramp_start: bool,
    /// The last preview worked out, and what from.
    pub(crate) preview: Option<Preview>,
    pub(crate) built_for: Option<PreviewKey>,
    /// The preview being worked out, by its job's token.
    pub(crate) building: Option<(u64, PreviewKey)>,
    pub(crate) next_token: u64,
}

impl PitDesignDraft {
    /// The window open on pit design object `id`, designed with `pit`.
    pub(crate) fn on(id: ObjectId, pit: PitObject) -> Self {
        Self {
            object: Some(id),
            synced: Some(pit.clone()),
            working: Some(pit),
            settled: true,
            ..Self::default()
        }
    }

    pub(crate) fn object_id(&self) -> Option<ObjectId> {
        self.object
    }

    /// Whether the viewport draws the preview over the object: while the
    /// copy differs from it, or its own design is not yet current.
    pub(crate) fn shows_preview(&self) -> bool {
        self.object.is_some() && (self.working != self.synced || !self.facts.fresh)
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
        NoDesign::NotClosed => tr!("pit-design-problem-string"),
        NoDesign::NoTopography => tr!("pit-design-problem-no-topography"),
        NoDesign::TopographyNotLoaded(name) => tr!("pit-design-problem-topography-not-loaded", name = name.clone()),
        NoDesign::Settings(problem) => problem_text(*problem),
    }
}

pub(crate) fn problem_text(problem: Problem) -> String {
    match problem {
        Problem::BenchHeight => tr!("pit-design-problem-bench-height"),
        Problem::FaceAngle => tr!("pit-design-problem-face-angle"),
        Problem::BermWidth => tr!("pit-design-problem-berm-width"),
        Problem::RampWidth => tr!("pit-design-problem-ramp-width"),
        Problem::RampGradient => tr!("pit-design-problem-ramp-gradient"),
        Problem::Switchback => tr!("pit-design-problem-switchback"),
        Problem::Benches => tr!("pit-design-problem-benches"),
        Problem::DaylightDown => tr!("pit-design-problem-daylight-down"),
        Problem::StopElevation => tr!("pit-design-problem-stop-elevation"),
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

/// How tall the window's scrolling body may be: the window, its heading and
/// its actions all fit on screen.
fn body_height(ctx: &egui::Context) -> f32 {
    (ctx.content_rect().height() - 220.0).max(200.0)
}

/// A row of buttons under a label: which was clicked.
fn button_row(ui: &mut egui::Ui, label: String, help: String, buttons: &[(String, bool)]) -> Option<usize> {
    let mut clicked = None;
    MenuField::new(label).help_text(help).show(ui, |ui, row_height, _| {
        ui.allocate_ui_with_layout(egui::vec2(FIELD_WIDTH, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            for (index, (text, enabled)) in buttons.iter().enumerate() {
                let width = menu::natural_button_width(ui, text, 40.0);
                if ui.add(MenuButton::new(text.clone()).min_width(width).enabled(*enabled)).clicked() {
                    clicked = Some(index);
                }
            }
        })
        .response
    });
    clicked
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
    let mut close = false;
    DragableMenu::new("pit_design_dialog", tr!("pit-design-title"))
        .open(&mut open)
        .min_width(420.0)
        .max_width(440.0)
        .show(ui.ctx(), |ui| {
            let Some(id) = draft.object_id() else {
                menu::menu_note(ui, tr!("pit-design-none-yet"));
                if button_row(ui, String::new(), tr!("pit-design-string-help"), &[(tr!("pit-design-pick"), true)]).is_some() {
                    commands.push(UiCommand::BeginSelectionPick(SelectionPickTool::PitDesignString));
                }
                ui.add_space(4.0);
                menu::menu_actions(ui, |ui| {
                    if ui.add(MenuButton::new(tr!("common-close"))).clicked() || menu::dialog_cancelled(ui.ctx()) {
                        close = true;
                    }
                });
                return;
            };
            // The design's settings and what they come to scroll between its
            // name and the actions, which stay in view.
            if let Some(working) = draft.working.as_mut() {
                MenuFieldText::new(tr!("pit-design-name"), &mut working.name).width(FIELD_WIDTH).show(ui);
            }
            egui::ScrollArea::vertical()
                .id_salt("pit_design_body")
                .max_height(body_height(ui.ctx()))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let Some(working) = draft.working.as_mut() else {
                        return;
                    };
                    draw_scenarios(ui, working);
                    let Some(scenario) = working.scenario_mut() else {
                        return;
                    };
                    let settings = &mut scenario.settings;
                    draw_settings(ui, settings, project, commands);

                    menu::menu_section(ui, tr!("pit-design-surface-section"));
                    if settings.daylights() {
                        MenuFieldBool::new(tr!("pit-design-trim"), &mut settings.trim_to_topography)
                            .help_text(tr!("pit-design-trim-help"))
                            .show(ui);
                    }
                    draw_surface(ui, &draft.facts, id, commands);

                    menu::menu_section(ui, tr!("pit-design-result-section"));
                    let settings = working.settings();
                    draw_result(ui, draft, &settings);
                });

            ui.add_space(4.0);
            let cancelled = menu::dialog_cancelled(ui.ctx());
            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("pit-design-explode"))).hint(tr!("pit-design-explode-help")).clicked() {
                    commands.push(UiCommand::ExplodePitDesign(id));
                }
                if ui.add(MenuButton::new(tr!("common-close")).primary()).clicked() || cancelled {
                    close = true;
                }
            });
        });
    // A field is mid-edit while the pointer is down on it or text is being
    // typed into it; until then the copy waits to be written.
    draft.settled = !ui.ctx().input(|input| input.pointer.any_down()) && ui.ctx().memory(|memory| memory.focused().is_none());
    if close || !open {
        commands.push(UiCommand::ClosePitDesign);
    }
}

/// The design's scenarios: which is active, its name, and copy and remove.
fn draw_scenarios(ui: &mut egui::Ui, design: &mut PitObject) {
    let mut active: ScenarioId = design.scenario().map_or(design.active, |scenario| scenario.id);
    let chosen = design.scenario().map(|scenario| scenario.name.clone()).unwrap_or_default();
    MenuFieldCombo::new(
        "pit_design_scenario",
        tr!("pit-design-scenario"),
        &mut active,
        chosen,
        design.scenarios.iter().map(|scenario| (scenario.id, scenario.name.clone().into())),
    )
    .width(FIELD_WIDTH)
    .help_text(tr!("pit-design-scenario-help"))
    .show(ui);
    design.active = active;
    if let Some(scenario) = design.scenario_mut() {
        MenuFieldText::new(tr!("pit-design-scenario-name"), &mut scenario.name).width(FIELD_WIDTH).show(ui);
    }
    let buttons = [(tr!("pit-design-scenario-copy"), true), (tr!("pit-design-scenario-remove"), design.scenarios.len() > 1)];
    match button_row(ui, String::new(), tr!("pit-design-scenario-copy-help"), &buttons) {
        Some(0) => {
            let name = design
                .scenario()
                .map_or_else(|| tr!("pit-design-scenario-default"), |scenario| tr!("pit-design-scenario-copy-name", name = scenario.name.clone()));
            design.add_scenario(name);
        }
        Some(1) => {
            design.remove_scenario();
        }
        _ => {}
    }
}

/// The scenario's benches, stop and ramp.
fn draw_settings(ui: &mut egui::Ui, settings: &mut PitDesignSettings, project: &UiProjectView, commands: &mut Vec<UiCommand>) {
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
        StopMode::Daylight => {
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
    }

    menu::menu_section(ui, tr!("pit-design-ramp-section"));
    MenuFieldBool::new(tr!("pit-design-ramp"), &mut settings.ramp).show(ui);
    if !settings.ramp {
        return;
    }
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
        |start: DVec2| tr!("pit-design-ramp-start-picked", x = format!("{:.1}", start.x), y = format!("{:.1}", start.y)),
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
                pick = ui.add(MenuButton::new(pick_label).min_width(pick_width)).clicked();
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


/// The surface the design built, how it stands, and Build.
fn draw_surface(ui: &mut egui::Ui, facts: &DraftFacts, id: ObjectId, commands: &mut Vec<UiCommand>) {
    let warning = ui.visuals().warn_fg_color;
    let name = facts.surface.as_ref().map_or_else(|| tr!("pit-design-not-built"), |(_, name)| name.clone());
    read_only_row(ui, &tr!("pit-design-output-surface"), &name);
    match facts.surface_standing {
        SurfaceStanding::NotBuilt => {}
        SurfaceStanding::Current => read_only_row(ui, &tr!("pit-design-standing"), &tr!("pit-design-current")),
        SurfaceStanding::OutOfDate => {
            ui.colored_label(warning, tr!("pit-design-out-of-date"));
        }
        SurfaceStanding::Edited => {
            ui.colored_label(warning, tr!("pit-design-surface-edited"));
        }
    }
    let label = if facts.surface.is_some() { tr!("pit-design-rebuild") } else { tr!("pit-design-build") };
    if button_row(ui, String::new(), tr!("pit-design-surface-help"), &[(label, true)]).is_some() {
        commands.push(UiCommand::BuildPitSurface(id));
    }
}

/// What the settings come to: the design's figures, or why there is none.
fn draw_result(ui: &mut egui::Ui, draft: &PitDesignDraft, settings: &PitDesignSettings) {
    let ramp = settings.ramp;
    let warning = ui.visuals().warn_fg_color;
    let Some(preview) = draft.preview.as_ref() else {
        menu::menu_note(ui, tr!("pit-design-designing"));
        return;
    };
    match &preview.design {
        Err(reason) => {
            ui.colored_label(warning, no_design_text(reason));
        }
        Ok(design) => {
            read_only_row(ui, &tr!("pit-design-benches-built"), &design.benches.to_string());
            read_only_row(ui, &tr!("pit-design-final-rl"), &format!("{:.1} m", design.final_elevation()));
            if ramp {
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
            if ramp {
                let flats: Vec<f64> = design.report.iter().filter_map(|bench| bench.ramp).map(|road| road.flat).filter(|flat| *flat > 0.0).collect();
                if !flats.is_empty() {
                    menu::menu_note(
                        ui,
                        tr!(
                            "pit-design-flats",
                            count = flats.len(),
                            length = format!("{:.0}", flats.iter().sum::<f64>())
                        ),
                    );
                }
                let flatter = design.report.iter().filter(|bench| bench.ramp.is_some_and(|road| road.lengthened > 0.0)).count();
                if flatter > 0 {
                    ui.colored_label(warning, tr!("pit-design-flatter", count = flatter));
                }
                if menu::menu_section_folding(ui, "pit_design_bench_table", tr!("pit-design-bench-table")) {
                    draw_bench_table(ui, design, settings);
                }
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
        }
    }
    if draft.building.is_some() {
        menu::menu_note(ui, tr!("pit-design-designing"));
    }
}

/// Each bench as built: its RL, its ramp's grade, flat and narrowest width,
/// and whether it turned back; what falls short of the settings in the
/// warning colour.
fn draw_bench_table(ui: &mut egui::Ui, design: &Design, settings: &PitDesignSettings) {
    let warning = ui.visuals().warn_fg_color;
    let weak = ui.visuals().weak_text_color();
    let gradient = settings.ramp_gradient / 100.0;
    egui::Grid::new("pit_design_benches").striped(true).num_columns(5).spacing([12.0, 2.0]).show(ui, |ui| {
        for heading in [
            tr!("pit-design-table-rl"),
            tr!("pit-design-table-grade"),
            tr!("pit-design-table-flat"),
            tr!("pit-design-table-width"),
            tr!("pit-design-table-turn"),
        ] {
            ui.label(egui::RichText::new(heading).color(weak).small());
        }
        ui.end_row();
        for bench in &design.report {
            let top = bench.elevation + settings.direction.sign() * bench.height;
            ui.label(format!("{:.1}–{top:.1}", bench.elevation));
            match bench.ramp {
                Some(road) => {
                    let grade = format!("{:.1} %", road.steepest * 100.0);
                    if road.lengthened > 0.0 || road.steepest < gradient - 0.005 {
                        ui.colored_label(warning, grade).hint(tr!("pit-design-table-flatter-hint", length = format!("{:.0}", road.lengthened)));
                    } else {
                        ui.label(grade);
                    }
                    ui.label(if road.flat > 0.0 { format!("{:.0} m", road.flat) } else { String::new() });
                    let width = format!("{:.1} m", road.narrowest);
                    if road.narrowest < settings.ramp_width * 0.99 {
                        ui.colored_label(warning, width).hint(tr!("pit-design-table-narrow-hint", width = format!("{:.1}", settings.ramp_width)));
                    } else {
                        ui.label(width);
                    }
                    ui.label(if road.switchback { tr!("pit-design-table-turned") } else { String::new() });
                }
                None => {
                    for _ in 0..4 {
                        ui.label("");
                    }
                }
            }
            ui.end_row();
        }
    });
}
