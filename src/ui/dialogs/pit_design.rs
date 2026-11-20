//! Pit Design: the tool's panel, docked in the viewport, that edits a pit
//! design object, and what it holds while one is open.
//!
//! The panel edits a copy of what the object is designed with
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
        ObjectId, PolyVertex, ReserveAggregation, ReserveField,
        block_model::BlockModelId,
        pit_design::{
            Design, Direction, End, Kind, Problem, RampBand, Zone,
            checks::{Check, Flag},
            evaluation::{Evaluation, Figures, ShellFigures, Waiting},
            object::PitObject,
            settings::{PitDesignSettings, RampSettings, StopMode},
            walls::{Sector, WallReport},
        },
        triangulation::{GeometryVersion, TriangulationId},
    },
    ui::{
        elements::properties::read_only_row,
        state::{EditorState, UiCommand, UiProjectView},
        widgets::{
            hint::Hint,
            menu::{self, MenuButton, MenuField, MenuFieldBool, MenuFieldCombo, MenuFieldF64, MenuFieldText, MenuFieldU32},
            viewport::ViewportDockPanel,
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
    pub(crate) ramp_starts: Vec<DVec3>,
}

/// What the panel shows of its object beyond what it edits.
#[derive(Clone, Debug, Default)]
pub(crate) struct DraftFacts {
    /// Whether the object's own design is what it comes to now.
    pub(crate) fresh: bool,
    /// Why its measure is not what it comes to now, if it is not.
    pub(crate) waiting: Option<Waiting>,
    /// The project's reserve fields, which its measure is in.
    pub(crate) fields: Vec<ReserveField>,
    /// Every pit design in the project, as last measured, to compare.
    pub(crate) designs: Vec<ComparedDesign>,
}

/// One pit design's measure, beside the others'.
#[derive(Clone, Debug)]
pub(crate) struct ComparedDesign {
    pub(crate) id: ObjectId,
    pub(crate) name: String,
    pub(crate) total: Option<Figures>,
    pub(crate) shell: Option<ShellFigures>,
    /// Whether its measure is what it comes to now.
    pub(crate) current: bool,
}

/// The steps the Pit Design tool takes a pit through once its string is
/// drawn: its benches first, then its ramp.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PitStage {
    #[default]
    Benches,
    Ramp,
}

/// What the pointer rests on in the panel's checks and bench table: a
/// bench, or one of the design's flags, for the viewport to pick out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PitHover {
    pub(crate) bench: Option<usize>,
    pub(crate) flag: Option<usize>,
}

/// The panel's state.
#[derive(Clone, Debug, Default)]
pub(crate) struct PitDesignDraft {
    /// The step the panel is on.
    pub(crate) stage: PitStage,
    /// Where on the string the ramp would start, under the cursor, while
    /// its start is being picked.
    pub(crate) ramp_hover: Option<DVec3>,
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
    pub(crate) picking_ramp: Option<usize>,
    /// The viewport is waiting for a click on a closed polyline, the outline
    /// of a new slope region.
    pub(crate) picking_region: bool,
    /// The last preview worked out, and what from.
    pub(crate) preview: Option<Preview>,
    pub(crate) built_for: Option<PreviewKey>,
    /// The preview being worked out, by its job's token.
    pub(crate) building: Option<(u64, PreviewKey)>,
    pub(crate) next_token: u64,
    /// What the pointer rests on in the panel.
    pub(crate) hover: PitHover,
    /// The flag under the pointer in the viewport, by its place in the
    /// preview's design.
    pub(crate) flag_under_cursor: Option<usize>,
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

fn direction_label(kind: Kind, direction: Direction) -> String {
    match (kind, direction) {
        (Kind::Pit, Direction::Up) => tr!("pit-design-up"),
        (Kind::Pit, Direction::Down) => tr!("pit-design-down"),
        (Kind::Dump, Direction::Up) => tr!("pit-design-dump-up"),
        (Kind::Dump, Direction::Down) => tr!("pit-design-dump-down"),
    }
}

fn kind_label(kind: Kind) -> String {
    match kind {
        Kind::Pit => tr!("pit-design-kind-pit"),
        Kind::Dump => tr!("pit-design-kind-dump"),
    }
}

fn stop_label(kind: Kind, stop: StopMode) -> String {
    match stop {
        StopMode::Benches => tr!("pit-design-stop-benches"),
        StopMode::Elevation => tr!("pit-design-stop-elevation"),
        StopMode::Daylight if kind == Kind::Dump => tr!("pit-design-stop-ground"),
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
        Problem::Zone => tr!("pit-design-problem-zone"),
        Problem::WallRadius => tr!("pit-design-problem-wall-radius"),
        Problem::RampBand => tr!("pit-design-problem-ramp-band"),
        Problem::Sector => tr!("pit-design-problem-sector"),
        Problem::Region => tr!("pit-design-problem-region"),
        Problem::RampWidth => tr!("pit-design-problem-ramp-width"),
        Problem::RampGradient => tr!("pit-design-problem-ramp-gradient"),
        Problem::Switchback => tr!("pit-design-problem-switchback"),
        Problem::Benches => tr!("pit-design-problem-benches"),
        Problem::DaylightDown => tr!("pit-design-problem-daylight-down"),
        Problem::DaylightDumpUp => tr!("pit-design-problem-daylight-dump-up"),
        Problem::StopElevation => tr!("pit-design-problem-stop-elevation"),
    }
}

/// Why a design ended where it did, in words.
pub(crate) fn end_text(design: &Design) -> String {
    let rl = format!("{:.1}", design.final_elevation());
    match design.end {
        End::Complete => tr!("pit-design-end-complete"),
        End::Daylight if design.dump => tr!("pit-design-end-ground", rl = rl),
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

/// How tall the panel's scrolling body may be: the panel, its heading, name
/// and actions all fit in the viewport.
fn body_height(viewport_rect: egui::Rect) -> f32 {
    (viewport_rect.height() - 150.0).max(160.0)
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

/// The Pit Design tool's panel, docked in the viewport while a pit design is
/// open in it: its name and settings, and what it
/// comes to.
pub(crate) fn draw_pit_design_panel(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, commands: &mut Vec<UiCommand>, viewport_rect: egui::Rect) {
    // Drawing the string: how its corners are made.
    if editor.pit_drawing() {
        ViewportDockPanel::new("pit_design_panel", tr!("pit-design-title"), viewport_rect)
            .min_width(300.0)
            .help_text(tr!("pit-design-draw-help"))
            .show(ui.ctx(), |ui| {
                MenuFieldBool::new(tr!("pit-design-round-corners"), &mut editor.pit_round_corners)
                    .help_text(tr!("pit-design-round-corners-help"))
                    .show(ui);
                if editor.pit_round_corners {
                    MenuFieldF64::new(tr!("pit-design-corner-radius"), &mut editor.pit_corner_radius, 0.1..=10000.0)
                        .speed(0.5)
                        .suffix(" m")
                        .show(ui);
                }
            });
        return;
    }
    let Some(draft) = editor.pit_design.as_mut() else {
        return;
    };
    let Some(id) = draft.object_id() else {
        return;
    };
    ViewportDockPanel::new("pit_design_panel", tr!("pit-design-title"), viewport_rect)
        .min_width(400.0)
        .max_width(420.0)
        .show(ui.ctx(), |ui| {
            if let Some(working) = draft.working.as_mut() {
                MenuFieldText::new(tr!("pit-design-name"), &mut working.name).width(FIELD_WIDTH).show(ui);
            }
            // The step's settings and what they come to scroll between the
            // name and the actions, which stay in view.
            egui::ScrollArea::vertical()
                .id_salt("pit_design_body")
                .max_height(body_height(viewport_rect))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let stage = draft.stage;
                    let picking = draft.picking_ramp;
                    let picking_region = draft.picking_region;
                    let Some(working) = draft.working.as_mut() else {
                        return;
                    };
                    match stage {
                        PitStage::Benches => {
                            let base_rl = draft.built_for.as_ref().and_then(|key| key.verts.first()).map_or(0.0, |vertex| vertex.pos.z);
                            draw_bench_settings(ui, &mut working.settings, project, (base_rl, picking_region), commands);
                        }
                        PitStage::Ramp => {
                            let base_rl = draft.built_for.as_ref().and_then(|key| key.verts.first()).map_or(0.0, |vertex| vertex.pos.z);
                            let settings = &mut working.settings;
                            let (sign, height) = (settings.direction.sign(), settings.bench_height);
                            if settings.ramps.is_empty() && picking.is_none() {
                                menu::menu_section(ui, tr!("pit-design-ramp-section"));
                                menu::menu_note(ui, tr!("pit-design-ramp-none"));
                            }
                            let several = settings.ramps.len() > 1;
                            let mut remove = None;
                            for (index, ramp) in settings.ramps.iter_mut().enumerate() {
                                ui.push_id(("pit_design_ramp", index), |ui| {
                                    let heading = if several {
                                        tr!("pit-design-ramp-numbered", number = (index + 1).to_string())
                                    } else {
                                        tr!("pit-design-ramp-section")
                                    };
                                    menu::menu_section(ui, heading);
                                    if picking == Some(index) {
                                        menu::menu_note(ui, tr!("pit-design-ramp-picking"));
                                        return;
                                    }
                                    draw_ramp_settings(ui, ramp, commands, index, (base_rl, sign, height));
                                    if button_row(ui, String::new(), tr!("pit-design-ramp-remove-help"), &[(tr!("pit-design-ramp-remove"), true)]).is_some() {
                                        remove = Some(index);
                                    }
                                });
                            }
                            if let Some(index) = remove {
                                settings.ramps.remove(index);
                            }
                            let count = settings.ramps.len();
                            if picking == Some(count) {
                                menu::menu_section(ui, tr!("pit-design-ramp-numbered", number = (count + 1).to_string()));
                                menu::menu_note(ui, tr!("pit-design-ramp-picking"));
                            } else if picking.is_none() {
                                let (label, help) = if count == 0 {
                                    (tr!("pit-design-ramp-place"), tr!("pit-design-ramp-place-help"))
                                } else {
                                    (tr!("pit-design-ramp-add"), tr!("pit-design-ramp-add-help"))
                                };
                                if button_row(ui, String::new(), help, &[(label, true)]).is_some() {
                                    commands.push(UiCommand::BeginPitRampStartPick(count));
                                }
                            }
                        }
                    }
                    menu::menu_section(ui, tr!("pit-design-result-section"));
                    let settings = working.settings();
                    let hover = draw_result(ui, draft, &settings, commands);
                    if hover != draft.hover {
                        commands.push(UiCommand::PitDesignHover(hover));
                    }
                    if let Some(working) = draft.working.as_mut() {
                        draw_evaluation(ui, working, &draft.facts, project);
                    }
                });

            ui.add_space(4.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match draft.stage {
                PitStage::Benches => {
                    if ui
                        .add(MenuButton::new(tr!("pit-design-continue")).primary())
                        .hint(tr!("pit-design-continue-help"))
                        .clicked()
                    {
                        commands.push(UiCommand::PitDesignStage(PitStage::Ramp));
                    }
                    if ui.add(MenuButton::new(tr!("pit-design-done"))).hint(tr!("pit-design-done-help")).clicked() {
                        commands.push(UiCommand::FinishPitDesign);
                    }
                }
                PitStage::Ramp => {
                    if ui.add(MenuButton::new(tr!("pit-design-done")).primary()).hint(tr!("pit-design-done-help")).clicked() {
                        commands.push(UiCommand::FinishPitDesign);
                    }
                    if ui.add(MenuButton::new(tr!("pit-design-back"))).clicked() {
                        commands.push(UiCommand::PitDesignStage(PitStage::Benches));
                    }
                    if ui.add(MenuButton::new(tr!("pit-design-explode"))).hint(tr!("pit-design-explode-help")).clicked() {
                        commands.push(UiCommand::ExplodePitDesign(id));
                    }
                }
            });
        });
    // The flag under the pointer in the viewport, said beside it.
    if let Some(flag) = draft
        .flag_under_cursor
        .and_then(|index| draft.preview.as_ref()?.design.as_ref().ok()?.flags.get(index))
        .filter(|_| ui.ctx().pointer_hover_pos().is_some_and(|pointer| viewport_rect.contains(pointer)) && !ui.ctx().egui_wants_pointer_input())
    {
        let settings = draft.working.as_ref().map(PitObject::settings).unwrap_or_default();
        crate::ui::widgets::hint::hint_at_pointer(ui.ctx(), egui::Id::unique("pit_design_flag_hint"), flag_text(flag, &settings));
    }
    // A field is mid-edit while the pointer is down on it or text is being
    // typed into it; until then the copy waits to be written.
    draft.settled = !ui.ctx().input(|input| input.pointer.any_down()) && ui.ctx().memory(|memory| memory.focused().is_none());
}

/// The pit's benches and where it stops. `base_rl` is the string's, where a
/// new zone starts.
fn draw_bench_settings(ui: &mut egui::Ui, settings: &mut PitDesignSettings, project: &UiProjectView, (base_rl, picking_region): (f64, bool), commands: &mut Vec<UiCommand>) {
    let chosen = kind_label(settings.kind);
    MenuFieldCombo::new(
        "pit_design_kind",
        tr!("pit-design-type"),
        &mut settings.kind,
        chosen,
        [Kind::Pit, Kind::Dump].map(|kind| (kind, kind_label(kind).into())),
    )
    .width(FIELD_WIDTH)
    .help_text(tr!("pit-design-type-help"))
    .show(ui);
    let kind = settings.kind;
    let chosen = direction_label(kind, settings.direction);
    MenuFieldCombo::new(
        "pit_design_direction",
        tr!("pit-design-direction"),
        &mut settings.direction,
        chosen,
        [Direction::Up, Direction::Down].map(|direction| (direction, direction_label(kind, direction).into())),
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
    if menu::menu_section_folding(ui, "pit_design_bench_rules", tr!("pit-design-rules-section")) {
        draw_bench_rules(ui, settings, base_rl);
    }
    if menu::menu_section_folding(ui, "pit_design_slope_rules", tr!("pit-design-slopes-section")) {
        draw_slope_rules(ui, settings, picking_region, commands);
    }

    menu::menu_section(ui, tr!("pit-design-stop-section"));
    // Only a pit built up, or a dump built down, meets the ground.
    let stops: Vec<StopMode> = if settings.can_daylight() {
        vec![StopMode::Benches, StopMode::Elevation, StopMode::Daylight]
    } else {
        vec![StopMode::Benches, StopMode::Elevation]
    };
    if !stops.contains(&settings.stop) {
        settings.stop = StopMode::Benches;
    }
    let chosen = stop_label(kind, settings.stop);
    MenuFieldCombo::new(
        "pit_design_stop",
        tr!("pit-design-stop"),
        &mut settings.stop,
        chosen,
        stops.into_iter().map(|stop| (stop, stop_label(kind, stop).into())),
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
}

/// How the benches vary: a cycle of heights, catch berms, and bands of RL
/// cut otherwise.
fn draw_bench_rules(ui: &mut egui::Ui, settings: &mut PitDesignSettings, base_rl: f64) {
    // The cycle is typed as a list. What is typed stays as it is while it
    // still reads as the cycle the settings hold, so a half-typed list is
    // not rewritten under the cursor.
    let typed_id = egui::Id::unique("pit_design_height_cycle");
    let cycle = settings.bench_rules.height_cycle.clone();
    let mut text = ui
        .data(|data| data.get_temp::<(String, Vec<f64>)>(typed_id))
        .filter(|(_, read)| *read == cycle)
        .map_or_else(|| cycle.iter().map(|height| format!("{height}")).collect::<Vec<_>>().join(", "), |(text, _)| text);
    let response = MenuFieldText::new(tr!("pit-design-height-cycle"), &mut text)
        .width(FIELD_WIDTH)
        .hint_text(tr!("pit-design-height-cycle-hint"))
        .help_text(tr!("pit-design-height-cycle-help"))
        .show(ui);
    if response.changed() {
        let read: Option<Vec<f64>> = text
            .split([',', ' ', ';'])
            .filter(|part| !part.trim().is_empty())
            .map(|part| part.trim().parse::<f64>().ok().filter(|height| height.is_finite() && *height > 0.0))
            .collect();
        if let Some(read) = read {
            settings.bench_rules.height_cycle.clone_from(&read);
            ui.data_mut(|data| data.insert_temp(typed_id, (text, read)));
        } else {
            ui.data_mut(|data| data.insert_temp(typed_id, (text, cycle)));
        }
    }
    let rules = &mut settings.bench_rules;
    MenuFieldU32::new(tr!("pit-design-catch-every"), &mut rules.catch_every, 0..=u32::MAX)
        .width(FIELD_WIDTH)
        .help_text(tr!("pit-design-catch-every-help"))
        .show(ui);
    if rules.catch_every > 0 {
        MenuFieldF64::new(tr!("pit-design-catch-width"), &mut rules.catch_width, 0.0..=1000.0)
            .width(FIELD_WIDTH)
            .speed(0.1)
            .suffix(" m")
            .show(ui);
    }

    let sign = settings.direction.sign();
    let (height, face_angle, berm_width, wall_radius) = (settings.bench_height, settings.face_angle, settings.berm_width, settings.wall_radius);
    let mut remove = None;
    for (index, zone) in settings.bench_rules.zones.iter_mut().enumerate() {
        ui.push_id(("pit_design_zone", index), |ui| {
            menu::menu_section(ui, tr!("pit-design-zone", number = (index + 1).to_string()));
            MenuFieldF64::new(tr!("pit-design-zone-from"), &mut zone.from, f64::MIN..=f64::MAX)
                .width(FIELD_WIDTH)
                .speed(0.5)
                .suffix(" m")
                .show(ui);
            MenuFieldF64::new(tr!("pit-design-zone-to"), &mut zone.to, f64::MIN..=f64::MAX)
                .width(FIELD_WIDTH)
                .speed(0.5)
                .suffix(" m")
                .show(ui);
            // A value the zone leaves to the design shows the design's, and
            // becomes the zone's own once changed.
            for (label, value, design, range, speed, suffix) in [
                (tr!("pit-design-bench-height"), &mut zone.height, height, 0.1..=1000.0, 0.1, " m"),
                (tr!("pit-design-face-angle"), &mut zone.face_angle, face_angle, 1.0..=89.0, 0.5, "°"),
                (tr!("pit-design-berm-width"), &mut zone.berm_width, berm_width, 0.0..=1000.0, 0.1, " m"),
                (tr!("pit-design-wall-radius"), &mut zone.wall_radius, wall_radius, 0.0..=10000.0, 0.5, " m"),
            ] {
                let mut shown = value.unwrap_or(design);
                if MenuFieldF64::new(label, &mut shown, range)
                    .width(FIELD_WIDTH)
                    .speed(speed)
                    .suffix(suffix)
                    .show(ui)
                    .changed()
                {
                    *value = Some(shown);
                }
            }
            if button_row(ui, String::new(), String::new(), &[(tr!("pit-design-zone-remove"), true)]).is_some() {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        settings.bench_rules.zones.remove(index);
    }
    if button_row(ui, String::new(), tr!("pit-design-zone-add-help"), &[(tr!("pit-design-zone-add"), true)]).is_some() {
        // A new zone takes the next two benches' worth of RL from the end of
        // the last, or from the string.
        let from = settings.bench_rules.zones.last().map_or(base_rl, |zone| zone.to);
        settings.bench_rules.zones.push(Zone {
            from,
            to: from + sign * 2.0 * height,
            ..Zone::default()
        });
    }
}

/// How the walls vary round the pit: by the way they face (sectors), by
/// where they stand (regions), and the inter-ramp angle they are checked
/// against.
fn draw_slope_rules(ui: &mut egui::Ui, settings: &mut PitDesignSettings, picking_region: bool, commands: &mut Vec<UiCommand>) {
    let (face_angle, berm_width) = (settings.face_angle, settings.berm_width);
    let rules = &mut settings.bench_rules;
    // An angle of 0 asks for nothing.
    let mut target = rules.inter_ramp.unwrap_or(0.0);
    if MenuFieldF64::new(tr!("pit-design-inter-ramp"), &mut target, 0.0..=89.0)
        .width(FIELD_WIDTH)
        .speed(0.5)
        .suffix("°")
        .help_text(tr!("pit-design-inter-ramp-help"))
        .show(ui)
        .changed()
    {
        rules.inter_ramp = (target > 0.0).then_some(target);
    }

    let mut remove = None;
    for (index, sector) in rules.sectors.iter_mut().enumerate() {
        ui.push_id(("pit_design_sector", index), |ui| {
            menu::menu_section(ui, tr!("pit-design-sector", number = (index + 1).to_string()));
            MenuFieldF64::new(tr!("pit-design-sector-bearing"), &mut sector.bearing, 0.0..=360.0)
                .width(FIELD_WIDTH)
                .speed(1.0)
                .suffix("°")
                .help_text(tr!("pit-design-sector-bearing-help"))
                .show(ui);
            MenuFieldF64::new(tr!("pit-design-face-angle"), &mut sector.face_angle, 1.0..=89.0)
                .width(FIELD_WIDTH)
                .speed(0.5)
                .suffix("°")
                .show(ui);
            MenuFieldF64::new(tr!("pit-design-berm-width"), &mut sector.berm_width, 0.0..=1000.0)
                .width(FIELD_WIDTH)
                .speed(0.1)
                .suffix(" m")
                .show(ui);
            let mut target = sector.inter_ramp.unwrap_or(0.0);
            if MenuFieldF64::new(tr!("pit-design-inter-ramp"), &mut target, 0.0..=89.0)
                .width(FIELD_WIDTH)
                .speed(0.5)
                .suffix("°")
                .show(ui)
                .changed()
            {
                sector.inter_ramp = (target > 0.0).then_some(target);
            }
            if button_row(ui, String::new(), String::new(), &[(tr!("pit-design-sector-remove"), true)]).is_some() {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        rules.sectors.remove(index);
    }
    if button_row(ui, String::new(), tr!("pit-design-sector-add-help"), &[(tr!("pit-design-sector-add"), true)]).is_some() {
        // A quarter turn on from the last, cut as the design is.
        let bearing = rules.sectors.last().map_or(0.0, |sector| (sector.bearing + 90.0).rem_euclid(360.0));
        rules.sectors.push(Sector {
            bearing,
            face_angle,
            berm_width,
            inter_ramp: None,
        });
    }

    let mut remove = None;
    for (index, region) in rules.regions.iter_mut().enumerate() {
        ui.push_id(("pit_design_region", index), |ui| {
            menu::menu_section(ui, tr!("pit-design-region", number = (index + 1).to_string()));
            // A value the region leaves alone shows the design's, and becomes
            // the region's own once changed.
            for (label, value, design, range, speed, suffix) in [
                (tr!("pit-design-face-angle"), &mut region.face_angle, face_angle, 1.0..=89.0, 0.5, "°"),
                (tr!("pit-design-berm-width"), &mut region.berm_width, berm_width, 0.0..=1000.0, 0.1, " m"),
            ] {
                let mut shown = value.unwrap_or(design);
                if MenuFieldF64::new(label, &mut shown, range)
                    .width(FIELD_WIDTH)
                    .speed(speed)
                    .suffix(suffix)
                    .show(ui)
                    .changed()
                {
                    *value = Some(shown);
                }
            }
            if button_row(ui, String::new(), String::new(), &[(tr!("pit-design-region-remove"), true)]).is_some() {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        rules.regions.remove(index);
    }
    if !rules.regions.is_empty() {
        MenuFieldF64::new(tr!("pit-design-blend"), &mut rules.blend, 0.0..=1000.0)
            .width(FIELD_WIDTH)
            .speed(0.5)
            .suffix(" m")
            .help_text(tr!("pit-design-blend-help"))
            .show(ui);
    }
    if picking_region {
        menu::menu_note(ui, tr!("pit-design-region-picking"));
    } else if button_row(ui, String::new(), tr!("pit-design-region-add-help"), &[(tr!("pit-design-region-add"), true)]).is_some() {
        commands.push(UiCommand::BeginPitRegionPick);
    }
}

/// A list of numbers typed into one field, such as RLs: what is typed stays
/// as it is while it still reads as the list held, so a half-typed list is
/// not rewritten under the cursor. Values `accept` refuses leave the list
/// as it was.
fn number_list_field(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    (label, hint, help): (String, String, String),
    values: &mut Vec<f64>,
    accept: impl Fn(f64) -> bool,
) {
    let typed_id = egui::Id::unique(id);
    let held = values.clone();
    let mut text = ui
        .data(|data| data.get_temp::<(String, Vec<f64>)>(typed_id))
        .filter(|(_, read)| *read == held)
        .map_or_else(|| held.iter().map(|value| format!("{value}")).collect::<Vec<_>>().join(", "), |(text, _)| text);
    let response = MenuFieldText::new(label, &mut text).width(FIELD_WIDTH).hint_text(hint).help_text(help).show(ui);
    if response.changed() {
        let read: Option<Vec<f64>> = text
            .split([',', ' ', ';'])
            .filter(|part| !part.trim().is_empty())
            .map(|part| part.trim().parse::<f64>().ok().filter(|value| accept(*value)))
            .collect();
        match read {
            Some(read) => {
                values.clone_from(&read);
                ui.data_mut(|data| data.insert_temp(typed_id, (text, read)));
            }
            None => {
                ui.data_mut(|data| data.insert_temp(typed_id, (text, held)));
            }
        }
    }
}

/// The ramp's settings, for a pit that has one: where it starts, its
/// width and gradient, which way it climbs, and how it turns.
fn draw_ramp_settings(ui: &mut egui::Ui, ramp: &mut RampSettings, commands: &mut Vec<UiCommand>, index: usize, (base_rl, sign, height): (f64, f64, f64)) {
    MenuFieldF64::new(tr!("pit-design-ramp-width"), &mut ramp.width, 0.1..=1000.0)
        .width(FIELD_WIDTH)
        .speed(0.1)
        .suffix(" m")
        .show(ui);
    MenuFieldF64::new(tr!("pit-design-ramp-gradient"), &mut ramp.gradient, 0.1..=100.0)
        .width(FIELD_WIDTH)
        .speed(0.1)
        .suffix(" %")
        .help_text(tr!("pit-design-ramp-gradient-help"))
        .show(ui);
    let start_text = ramp.start.map_or_else(
        || tr!("pit-design-ramp-start-default"),
        |start: DVec2| tr!("pit-design-ramp-start-picked", x = format!("{:.1}", start.x), y = format!("{:.1}", start.y)),
    );
    let mut pick = false;
    MenuField::new(tr!("pit-design-ramp-start"))
        .help_text(tr!("pit-design-ramp-start-help"))
        .show(ui, |ui, row_height, _| {
            ui.allocate_ui_with_layout(egui::vec2(FIELD_WIDTH, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let pick_label = tr!("pit-design-ramp-move");
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
        commands.push(UiCommand::BeginPitRampStartPick(index));
    }
    let turn_label = |clockwise: bool| if clockwise { tr!("pit-design-clockwise") } else { tr!("pit-design-anticlockwise") };
    let chosen = turn_label(ramp.climbs_clockwise);
    MenuFieldCombo::new(
        "pit_design_climbs",
        tr!("pit-design-climbs"),
        &mut ramp.climbs_clockwise,
        chosen,
        [false, true].map(|clockwise| (clockwise, turn_label(clockwise).into())),
    )
    .width(FIELD_WIDTH)
    .help_text(tr!("pit-design-climbs-help"))
    .show(ui);
    // The tightest the road should turn: what a switchback turns round, and
    // what the whole road is checked against, either way.
    MenuFieldF64::new(tr!("pit-design-inside-radius"), &mut ramp.inside_radius, 0.0..=10000.0)
        .width(FIELD_WIDTH)
        .speed(0.5)
        .suffix(" m")
        .help_text(tr!("pit-design-inside-radius-help"))
        .show(ui);
    let layout_label = |switchback: bool| if switchback { tr!("pit-design-switchback") } else { tr!("pit-design-spiral") };
    let chosen = layout_label(ramp.switchback);
    MenuFieldCombo::new(
        "pit_design_layout",
        tr!("pit-design-layout"),
        &mut ramp.switchback,
        chosen,
        [false, true].map(|switchback| (switchback, layout_label(switchback).into())),
    )
    .width(FIELD_WIDTH)
    .help_text(tr!("pit-design-layout-help"))
    .show(ui);
    if ramp.switchback {
        MenuFieldU32::new(tr!("pit-design-switchback-every"), &mut ramp.switchback_every, 0..=u32::MAX)
            .width(FIELD_WIDTH)
            .help_text(tr!("pit-design-switchback-every-help"))
            .show(ui);
        number_list_field(
            ui,
            ("pit_design_turns_at", index),
            (tr!("pit-design-turns-at"), tr!("pit-design-turns-at-hint"), tr!("pit-design-turns-at-help")),
            &mut ramp.turns_at,
            f64::is_finite,
        );
        MenuFieldF64::new(tr!("pit-design-turn-widening"), &mut ramp.turn_widening, 0.0..=1000.0)
            .width(FIELD_WIDTH)
            .speed(0.1)
            .suffix(" m")
            .help_text(tr!("pit-design-turn-widening-help"))
            .show(ui);
        let steepest = ramp.gradient;
        MenuFieldF64::new(tr!("pit-design-turn-grade"), &mut ramp.turn_grade, 0.0..=steepest)
            .width(FIELD_WIDTH)
            .speed(0.1)
            .suffix(" %")
            .help_text(tr!("pit-design-turn-grade-help"))
            .show(ui);
        // Turning only at RLs, with none given, it never turns.
        if ramp.switchback_every == 0 && ramp.turns_at.is_empty() {
            menu::menu_note(ui, tr!("pit-design-turns-none"));
        }
    }
    if menu::menu_section_folding(ui, "pit_design_ramp_extent", tr!("pit-design-ramp-extent-section")) {
        draw_ramp_extent(ui, ramp, (base_rl, sign, height));
    }
}

/// Where the ramp runs, and how its stretches are laid bench by bench: the
/// RLs it begins and ends at, a flat where it leaves the pit, and bands of
/// RL laid narrower or with a flat.
fn draw_ramp_extent(ui: &mut egui::Ui, ramp: &mut RampSettings, (base_rl, sign, height): (f64, f64, f64)) {
    // Each end at the design's own, or at an RL of its own.
    for (id, label, help, value, default, whole) in [
        (
            "pit_design_ramp_from",
            tr!("pit-design-ramp-begins"),
            tr!("pit-design-ramp-begins-help"),
            &mut ramp.from,
            base_rl,
            tr!("pit-design-ramp-begins-string"),
        ),
        (
            "pit_design_ramp_to",
            tr!("pit-design-ramp-ends"),
            tr!("pit-design-ramp-ends-help"),
            &mut ramp.to,
            base_rl + sign * 4.0 * height,
            tr!("pit-design-ramp-ends-last"),
        ),
    ] {
        let mut at_rl = value.is_some();
        let chosen = if at_rl { tr!("pit-design-ramp-at-rl") } else { whole.clone() };
        MenuFieldCombo::new(id, label, &mut at_rl, chosen, [(false, whole.into()), (true, tr!("pit-design-ramp-at-rl").into())])
            .width(FIELD_WIDTH)
            .help_text(help)
            .show(ui);
        match (at_rl, value.as_mut()) {
            (false, _) => *value = None,
            (true, None) => *value = Some(default),
            (true, Some(rl)) => {
                ui.push_id(id, |ui| {
                    MenuFieldF64::new(tr!("pit-design-ramp-rl"), rl, f64::MIN..=f64::MAX)
                        .width(FIELD_WIDTH)
                        .speed(0.5)
                        .suffix(" m")
                        .show(ui);
                });
            }
        }
    }
    MenuFieldF64::new(tr!("pit-design-ramp-exit-flat"), &mut ramp.exit_flat, 0.0..=10000.0)
        .width(FIELD_WIDTH)
        .speed(0.5)
        .suffix(" m")
        .help_text(tr!("pit-design-ramp-exit-flat-help"))
        .show(ui);
    MenuFieldF64::new(tr!("pit-design-ramp-junction-flat"), &mut ramp.junction_flat, 0.0..=10000.0)
        .width(FIELD_WIDTH)
        .speed(0.5)
        .suffix(" m")
        .help_text(tr!("pit-design-ramp-junction-flat-help"))
        .show(ui);

    let width = ramp.width;
    let mut remove = None;
    for (index, band) in ramp.bands.iter_mut().enumerate() {
        ui.push_id(("pit_design_ramp_band", index), |ui| {
            menu::menu_section(ui, tr!("pit-design-ramp-band", number = (index + 1).to_string()));
            MenuFieldF64::new(tr!("pit-design-zone-from"), &mut band.from, f64::MIN..=f64::MAX)
                .width(FIELD_WIDTH)
                .speed(0.5)
                .suffix(" m")
                .show(ui);
            MenuFieldF64::new(tr!("pit-design-zone-to"), &mut band.to, f64::MIN..=f64::MAX)
                .width(FIELD_WIDTH)
                .speed(0.5)
                .suffix(" m")
                .show(ui);
            // A value the band leaves to the ramp shows the ramp's, and
            // becomes the band's own once changed.
            for (label, value, ramp, range) in [
                (tr!("pit-design-ramp-width"), &mut band.width, width, 0.1..=1000.0),
                (tr!("pit-design-ramp-band-flat"), &mut band.flat, 0.0, 0.0..=10000.0),
            ] {
                let mut shown = value.unwrap_or(ramp);
                if MenuFieldF64::new(label, &mut shown, range).width(FIELD_WIDTH).speed(0.1).suffix(" m").show(ui).changed() {
                    *value = Some(shown);
                }
            }
            if button_row(ui, String::new(), String::new(), &[(tr!("pit-design-ramp-band-remove"), true)]).is_some() {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        ramp.bands.remove(index);
    }
    if button_row(ui, String::new(), tr!("pit-design-ramp-band-add-help"), &[(tr!("pit-design-ramp-band-add"), true)]).is_some() {
        // A new band takes the next two benches' worth of RL from the end of
        // the last, or from the string.
        let from = ramp.bands.last().map_or(base_rl, |band| band.to);
        ramp.bands.push(RampBand {
            from,
            to: from + sign * 2.0 * height,
            ..RampBand::default()
        });
    }
}

/// What the settings come to: the design's figures, or why there is none;
/// and what the pointer rests on among them.
fn draw_result(ui: &mut egui::Ui, draft: &PitDesignDraft, settings: &PitDesignSettings, commands: &mut Vec<UiCommand>) -> PitHover {
    let ramp = !settings.ramps.is_empty();
    let warning = ui.visuals().warn_fg_color;
    let mut hover = PitHover::default();
    let Some(preview) = draft.preview.as_ref() else {
        menu::menu_note(ui, tr!("pit-design-designing"));
        return hover;
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
            // Turns put off from where they were asked, for want of room.
            for bench in &design.report {
                for road in bench.ramps.iter().filter(|road| road.switchback) {
                    if let Some(asked) = road.turn_moved_from {
                        ui.colored_label(
                            warning,
                            tr!("pit-design-turn-moved", asked = format!("{asked:.1}"), made = format!("{:.1}", bench.elevation)),
                        );
                    }
                }
            }
            if let Some(blocked) = design.blocked_ramp.filter(|_| settings.ramps.len() > 1) {
                ui.colored_label(warning, tr!("pit-design-ramp-blocked", number = (blocked + 1).to_string()));
            }
            if design.ramp_start_moved > 0.0 {
                ui.colored_label(warning, tr!("pit-design-start-moved", distance = format!("{:.0}", design.ramp_start_moved)));
            }
            if ramp {
                let flats: Vec<f64> = design
                    .report
                    .iter()
                    .flat_map(|bench| bench.ramps.iter())
                    .map(|road| road.flat)
                    .filter(|flat| *flat > 0.0)
                    .collect();
                if !flats.is_empty() {
                    menu::menu_note(ui, tr!("pit-design-flats", count = flats.len(), length = format!("{:.0}", flats.iter().sum::<f64>())));
                }
                let flatter = design.report.iter().filter(|bench| bench.ramps.iter().any(|road| road.lengthened > 0.0)).count();
                if flatter > 0 {
                    ui.colored_label(warning, tr!("pit-design-flatter", count = flatter));
                }
                let steep = design.walls.iter().filter(|wall| wall.too_steep()).count();
                if steep > 0 {
                    ui.colored_label(warning, tr!("pit-design-walls-too-steep", count = steep));
                }
            }
            if design.flags.is_empty() {
                menu::menu_note(ui, tr!("pit-design-checks-pass"));
            } else {
                ui.colored_label(warning, tr!("pit-design-checks-count", count = design.flags.len()));
                if menu::menu_section_folding(ui, "pit_design_checks", tr!("pit-design-checks")) {
                    draw_checks(ui, design, settings, draft.hover, &mut hover, commands);
                }
            }
            if ramp {
                if menu::menu_section_folding(ui, "pit_design_bench_table", tr!("pit-design-bench-table")) {
                    draw_bench_table(ui, design, settings, &mut hover);
                }
                if !design.walls.is_empty() && menu::menu_section_folding(ui, "pit_design_walls_table", tr!("pit-design-walls-table")) {
                    draw_walls_table(ui, &design.walls);
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
    hover
}

/// A quantity as the reserves show one: whole and grouped in thousands
/// from a thousand, to two places below.
fn quantity(value: Option<f64>) -> String {
    use thousands::Separable;
    match value {
        None => "-".to_owned(),
        Some(value) if value.abs() >= 1000.0 => format!("{value:.0}").separate_with_commas(),
        Some(value) => format!("{value:.2}"),
    }
}

/// What the design takes out of the ground, measured against the
/// topography and a block model the section chooses: in all, by material
/// and bench by bench.
fn draw_evaluation(ui: &mut egui::Ui, working: &mut PitObject, facts: &DraftFacts, project: &UiProjectView) {
    if !menu::menu_section_folding(ui, "pit_design_evaluation", tr!("pit-design-evaluation")) {
        return;
    }
    // Built up short of the ground it could daylight against, its ground
    // above its last ring is measured as though its walls stood straight up.
    let short = working.design().is_some_and(|design| working.settings.can_daylight() && design.end != End::Daylight);
    let settings = &mut working.settings;
    // Daylighting, the topography is chosen with where the design stops.
    if !settings.daylights() {
        let surfaces: Vec<(TriangulationId, &str)> = project.triangulations.iter().map(|entry| (entry.id, entry.name.as_str())).collect();
        let chosen = settings
            .topography
            .and_then(|id| surfaces.iter().find(|(surface, _)| *surface == id).map(|(_, name)| (*name).to_owned()))
            .unwrap_or_else(|| tr!("pit-design-topography-none"));
        MenuFieldCombo::new(
            "pit_design_evaluation_topography",
            tr!("pit-design-topography"),
            &mut settings.topography,
            chosen,
            std::iter::once((None, tr!("pit-design-topography-none").into())).chain(surfaces.iter().map(|(id, name)| (Some(*id), (*name).into()))),
        )
        .width(FIELD_WIDTH)
        .help_text(tr!("pit-design-evaluation-topography-help"))
        .show(ui);
    }
    let models: Vec<(BlockModelId, &str)> = project.block_models.iter().map(|entry| (entry.id, entry.name.as_str())).collect();
    let chosen = settings
        .evaluation
        .block_model
        .and_then(|id| models.iter().find(|(model, _)| *model == id).map(|(_, name)| (*name).to_owned()))
        .unwrap_or_else(|| tr!("pit-design-block-model-none"));
    MenuFieldCombo::new(
        "pit_design_block_model",
        tr!("pit-design-block-model"),
        &mut settings.evaluation.block_model,
        chosen,
        std::iter::once((None, tr!("pit-design-block-model-none").into())).chain(models.iter().map(|(id, name)| (Some(*id), (*name).into()))),
    )
    .width(FIELD_WIDTH)
    .help_text(tr!("pit-design-block-model-help"))
    .show(ui);
    // An optimisation shell to set a pit against.
    if settings.kind == Kind::Pit {
        let surfaces: Vec<(TriangulationId, &str)> = project.triangulations.iter().map(|entry| (entry.id, entry.name.as_str())).collect();
        let chosen = settings
            .evaluation
            .shell
            .and_then(|id| surfaces.iter().find(|(surface, _)| *surface == id).map(|(_, name)| (*name).to_owned()))
            .unwrap_or_else(|| tr!("pit-design-block-model-none"));
        MenuFieldCombo::new(
            "pit_design_shell",
            tr!("pit-design-shell"),
            &mut settings.evaluation.shell,
            chosen,
            std::iter::once((None, tr!("pit-design-block-model-none").into())).chain(surfaces.iter().map(|(id, name)| (Some(*id), (*name).into()))),
        )
        .width(FIELD_WIDTH)
        .help_text(tr!("pit-design-shell-help"))
        .show(ui);
    }
    let warning = ui.visuals().warn_fg_color;
    match &facts.waiting {
        Some(Waiting::Design) => menu::menu_note(ui, tr!("pit-design-designing")),
        Some(Waiting::NoTopography) => menu::menu_note(ui, tr!("pit-design-evaluation-no-topography")),
        Some(Waiting::NotLoaded(name)) => {
            ui.colored_label(warning, tr!("pit-design-evaluation-not-loaded", name = name.clone()));
        }
        Some(Waiting::Measuring) => menu::menu_note(ui, tr!("pit-design-measuring")),
        None => {}
    }
    let Some(evaluation) = working.evaluation.as_ref() else {
        return;
    };
    if let Some(problem) = &evaluation.problem {
        ui.colored_label(warning, tr!("pit-design-evaluation-failed", reason = problem.clone()));
        return;
    }
    if short {
        ui.colored_label(warning, tr!("pit-design-evaluation-short"));
    }
    draw_figures(ui, evaluation, &facts.fields);
    if let Some(shell) = &evaluation.shell {
        draw_against_shell(ui, shell, &facts.fields);
    }
    draw_comparison(ui, facts, working.design().is_some());
}

/// The numeric reserve fields: those a figure is given for.
fn numeric_fields(fields: &[ReserveField]) -> Vec<&ReserveField> {
    fields.iter().filter(|field| field.aggregation != ReserveAggregation::Category).collect()
}

/// A small heading over a table's column.
fn column_heading(ui: &mut egui::Ui, text: String) {
    let weak = ui.visuals().weak_text_color();
    ui.label(egui::RichText::new(text).color(weak).small());
}

/// What the design takes beyond the shell and leaves inside it, and the
/// difference, where a difference means something: of the volume and the
/// summed fields, not of an average.
fn draw_against_shell(ui: &mut egui::Ui, shell: &ShellFigures, fields: &[ReserveField]) {
    menu::menu_note(ui, tr!("pit-design-shell-against"));
    if let Some(problem) = &shell.problem {
        ui.colored_label(ui.visuals().warn_fg_color, tr!("pit-design-shell-failed", reason = problem.clone()));
        return;
    }
    let numeric = numeric_fields(fields);
    let value = |figures: &Figures, field: &ReserveField| figures.measured.as_ref().and_then(|measured| measured.value(field.id));
    egui::ScrollArea::horizontal().id_salt("pit_design_against_shell").show(ui, |ui| {
        egui::Grid::new("pit_design_against_shell_grid").striped(true).spacing([12.0, 2.0]).show(ui, |ui| {
            column_heading(ui, String::new());
            column_heading(ui, tr!("pit-design-evaluation-volume"));
            for field in &numeric {
                column_heading(ui, field.name.clone());
            }
            ui.end_row();
            for (label, figures) in [(tr!("pit-design-shell-gained"), &shell.gained), (tr!("pit-design-shell-left"), &shell.left)] {
                ui.label(label);
                ui.label(quantity(Some(figures.volume)));
                for field in &numeric {
                    ui.label(quantity(value(figures, field)));
                }
                ui.end_row();
            }
            ui.label(tr!("pit-design-shell-net"));
            ui.label(quantity(Some(shell.gained.volume - shell.left.volume)));
            for field in &numeric {
                let net = match field.aggregation {
                    ReserveAggregation::Sum => value(&shell.gained, field).zip(value(&shell.left, field)).map(|(gained, left)| gained - left),
                    _ => None,
                };
                ui.label(net.map_or_else(String::new, |net| quantity(Some(net))));
            }
            ui.end_row();
        });
    });
}

/// Every pit design in the project measured, side by side: this one picked
/// out, any not measured as it stands now dimmed.
fn draw_comparison(ui: &mut egui::Ui, facts: &DraftFacts, designed: bool) {
    let measured: Vec<&ComparedDesign> = facts.designs.iter().filter(|design| design.total.is_some()).collect();
    if measured.len() < 2 || !designed {
        return;
    }
    menu::menu_note(ui, tr!("pit-design-compare"));
    let numeric = numeric_fields(&facts.fields);
    let shells = measured.iter().any(|design| design.shell.as_ref().is_some_and(|shell| shell.problem.is_none()));
    let weak = ui.visuals().weak_text_color();
    egui::ScrollArea::horizontal().id_salt("pit_design_compare").show(ui, |ui| {
        egui::Grid::new("pit_design_compare_grid").striped(true).spacing([12.0, 2.0]).show(ui, |ui| {
            column_heading(ui, tr!("pit-design-name"));
            column_heading(ui, tr!("pit-design-evaluation-volume"));
            for field in &numeric {
                column_heading(ui, field.name.clone());
            }
            if shells {
                column_heading(ui, tr!("pit-design-shell-gained"));
                column_heading(ui, tr!("pit-design-shell-left"));
            }
            ui.end_row();
            for design in measured {
                let this = facts.designs.first().is_some_and(|first| first.id == design.id);
                let text = |text: String| {
                    let text = egui::RichText::new(text);
                    match (this, design.current) {
                        (true, _) => text.strong(),
                        (false, false) => text.color(weak),
                        (false, true) => text,
                    }
                };
                let name = ui.label(text(design.name.clone()));
                if !design.current {
                    name.hint(tr!("pit-design-compare-stale"));
                }
                let total = design.total.as_ref();
                ui.label(text(quantity(total.map(|total| total.volume))));
                for field in &numeric {
                    ui.label(text(quantity(
                        total.and_then(|total| total.measured.as_ref()).and_then(|measured| measured.value(field.id)),
                    )));
                }
                if shells {
                    let shell = design.shell.as_ref().filter(|shell| shell.problem.is_none());
                    ui.label(text(shell.map_or_else(String::new, |shell| quantity(Some(shell.gained.volume)))));
                    ui.label(text(shell.map_or_else(String::new, |shell| quantity(Some(shell.left.volume)))));
                }
                ui.end_row();
            }
        });
    });
}

/// A measure's figures: the whole design's, its materials' and its
/// benches'.
fn draw_figures(ui: &mut egui::Ui, evaluation: &Evaluation, fields: &[ReserveField]) {
    let weak = ui.visuals().weak_text_color();
    let total = &evaluation.total;
    read_only_row(ui, &tr!("pit-design-evaluation-volume"), &format!("{} m³", quantity(Some(total.volume))));
    let numeric: Vec<&ReserveField> = fields.iter().filter(|field| field.aggregation != ReserveAggregation::Category).collect();
    let Some(measured) = total.measured.as_ref() else {
        return;
    };
    read_only_row(ui, &tr!("pit-design-evaluation-covered"), &format!("{} m³", quantity(Some(measured.covered))));
    for field in &numeric {
        read_only_row(ui, &field.name, &quantity(measured.value(field.id)));
    }
    let heading = |ui: &mut egui::Ui, text: String| {
        ui.label(egui::RichText::new(text).color(weak).small());
    };
    // By material: each category field's categories.
    for category in &measured.categories {
        let Some(field) = fields.iter().find(|field| field.id == category.field) else {
            continue;
        };
        if category.groups.is_empty() {
            continue;
        }
        menu::menu_note(ui, tr!("pit-design-evaluation-by", field = field.name.clone()));
        egui::ScrollArea::horizontal().id_salt(("pit_design_materials", field.id.0)).show(ui, |ui| {
            egui::Grid::new(("pit_design_materials_grid", field.id.0))
                .striped(true)
                .spacing([12.0, 2.0])
                .show(ui, |ui| {
                    heading(ui, field.name.clone());
                    heading(ui, tr!("pit-design-evaluation-volume"));
                    for field in &numeric {
                        heading(ui, field.name.clone());
                    }
                    ui.end_row();
                    for group in &category.groups {
                        ui.label(&group.name);
                        ui.label(quantity(Some(group.covered)));
                        for field in &numeric {
                            ui.label(quantity(group.value(field.id)));
                        }
                        ui.end_row();
                    }
                });
        });
    }
    // Bench by bench.
    if evaluation.benches.is_empty() {
        return;
    }
    menu::menu_note(ui, tr!("pit-design-evaluation-benches"));
    egui::ScrollArea::horizontal().id_salt("pit_design_evaluation_benches").show(ui, |ui| {
        egui::Grid::new("pit_design_evaluation_benches_grid").striped(true).spacing([12.0, 2.0]).show(ui, |ui| {
            heading(ui, tr!("pit-design-table-rl"));
            heading(ui, tr!("pit-design-evaluation-volume"));
            for field in &numeric {
                heading(ui, field.name.clone());
            }
            ui.end_row();
            for bench in evaluation.benches.iter().rev() {
                let Figures { volume, measured } = &bench.figures;
                ui.label(format!("{:.1}–{:.1}", bench.base, bench.top));
                ui.label(quantity(Some(*volume)));
                for field in &numeric {
                    ui.label(quantity(measured.as_ref().and_then(|measured| measured.value(field.id))));
                }
                ui.end_row();
            }
        });
    });
}

/// What a check is called.
pub(crate) fn check_label(check: Check) -> String {
    match check {
        Check::Steep => tr!("pit-design-check-steep"),
        Check::Flatter => tr!("pit-design-check-flatter"),
        Check::Narrow => tr!("pit-design-check-narrow"),
        Check::Tight => tr!("pit-design-check-tight"),
        Check::Berm => tr!("pit-design-check-berm"),
        Check::Face => tr!("pit-design-check-face"),
        Check::InterRamp => tr!("pit-design-check-inter-ramp"),
    }
}

/// A flag's figure, or what was asked, as the check measures it.
pub(crate) fn flag_figure(check: Check, value: f64) -> String {
    match check {
        Check::Steep | Check::Flatter => format!("{:.1} %", value * 100.0),
        Check::Narrow | Check::Tight | Check::Berm => format!("{value:.1} m"),
        Check::Face | Check::InterRamp => format!("{value:.1}°"),
    }
}

/// A flag said whole: what falls short, by how much, and where.
pub(crate) fn flag_text(flag: &Flag, settings: &PitDesignSettings) -> String {
    let mut text = tr!(
        "pit-design-flag",
        check = check_label(flag.check),
        value = flag_figure(flag.check, flag.value),
        asked = flag_figure(flag.check, flag.asked),
        rl = format!("{:.1}", flag.at.z)
    );
    if let Some(ramp) = flag.ramp.filter(|_| settings.ramps.len() > 1) {
        text = format!("{text}\n{}", tr!("pit-design-ramp-numbered", number = (ramp + 1).to_string()));
    }
    if flag.length > 0.5 {
        text = format!("{text}\n{}", tr!("pit-design-flag-along", length = format!("{:.0}", flag.length)));
    }
    text
}

/// Each place the design falls short, by bench: hovered, it is picked out
/// in the viewport; clicked, the view goes to it.
fn draw_checks(ui: &mut egui::Ui, design: &Design, settings: &PitDesignSettings, shown: PitHover, hover: &mut PitHover, commands: &mut Vec<UiCommand>) {
    let weak = ui.visuals().weak_text_color();
    let mut order: Vec<usize> = (0..design.flags.len()).collect();
    order.sort_by(|a, b| design.flags[*a].bench.cmp(&design.flags[*b].bench).then(a.cmp(b)));
    egui::Grid::new("pit_design_checks_grid").striped(true).num_columns(4).spacing([12.0, 2.0]).show(ui, |ui| {
        for heading in [
            tr!("pit-design-table-rl"),
            tr!("pit-design-checks-what"),
            tr!("pit-design-checks-is"),
            tr!("pit-design-checks-asked"),
        ] {
            ui.label(egui::RichText::new(heading).color(weak).small());
        }
        ui.end_row();
        for index in order {
            let flag = &design.flags[index];
            let top = ui.cursor().min.y;
            ui.label(format!("{:.1}", flag.at.z));
            let lit = shown.flag == Some(index) || shown.bench == Some(flag.bench);
            let name = ui.selectable_label(lit, check_label(flag.check)).hint(flag_text(flag, settings));
            if name.clicked() {
                commands.push(UiCommand::FramePitFlag(index));
            }
            ui.colored_label(ui.visuals().warn_fg_color, flag_figure(flag.check, flag.value));
            ui.label(flag_figure(flag.check, flag.asked));
            ui.end_row();
            let row = egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.cursor().min.y);
            if ui.rect_contains_pointer(row) {
                hover.flag = Some(index);
            }
        }
    });
    menu::menu_note(ui, tr!("pit-design-checks-help"));
}

/// Each bench as built: its RL, its ramp's grade, flat and narrowest width,
/// and whether it turned back; what falls short of the settings in the
/// warning colour.
/// What the walls come to between ramps, facing each way: the angle their
/// benches were cut to make, what they make on average and at the steepest,
/// and the target, those too steep picked out.
fn draw_walls_table(ui: &mut egui::Ui, walls: &[WallReport]) {
    let warning = ui.visuals().warn_fg_color;
    let weak = ui.visuals().weak_text_color();
    egui::Grid::new("pit_design_walls").striped(true).num_columns(5).spacing([12.0, 2.0]).show(ui, |ui| {
        for heading in [
            tr!("pit-design-walls-facing"),
            tr!("pit-design-walls-designed"),
            tr!("pit-design-walls-achieved"),
            tr!("pit-design-walls-steepest"),
            tr!("pit-design-walls-target"),
        ] {
            ui.label(egui::RichText::new(heading).color(weak).small());
        }
        ui.end_row();
        for wall in walls {
            ui.label(format!("{:.0}°", wall.bearing));
            ui.label(format!("{:.1}°", wall.designed));
            let achieved = format!("{:.1}°", wall.achieved);
            if wall.too_steep() {
                ui.colored_label(warning, achieved).hint(tr!("pit-design-walls-too-steep-hint"));
            } else {
                ui.label(achieved);
            }
            ui.label(format!("{:.1}°", wall.steepest));
            ui.label(wall.target.map_or_else(String::new, |target| format!("{target:.1}°")));
            ui.end_row();
        }
    });
}

/// A value that may vary round the pit, to `decimals` places: one number
/// where it does not, its least and greatest where it does.
fn span([low, high]: [f64; 2], decimals: usize, unit: &str) -> String {
    let (low, high) = (format!("{low:.decimals$}"), format!("{high:.decimals$}"));
    if low == high { format!("{low}{unit}") } else { format!("{low}–{high}{unit}") }
}

fn draw_bench_table(ui: &mut egui::Ui, design: &Design, settings: &PitDesignSettings, hover: &mut PitHover) {
    let warning = ui.visuals().warn_fg_color;
    let weak = ui.visuals().weak_text_color();
    let sign = settings.direction.sign();
    let several = settings.ramps.len() > 1;
    // The face and berm get columns of their own only where the benches
    // differ in them.
    // (The first bench has no berm before it, so berms are compared from the
    // second on.)
    let faces_vary = design.report.windows(2).any(|pair| pair[0].face_angle != pair[1].face_angle) || design.report.iter().any(|bench| bench.face_angle[0] != bench.face_angle[1]);
    let berms_vary = design
        .report
        .get(1..)
        .is_some_and(|after| after.windows(2).any(|pair| pair[0].berm != pair[1].berm) || after.iter().any(|bench| bench.berm[0] != bench.berm[1]));
    let varies = faces_vary || berms_vary;
    let flagged = !design.flags.is_empty();
    let columns = 5 + if varies { 2 } else { 0 } + usize::from(several) + usize::from(flagged);
    egui::Grid::new("pit_design_benches")
        .striped(true)
        .num_columns(columns)
        .spacing([12.0, 2.0])
        .show(ui, |ui| {
            let mut headings = vec![tr!("pit-design-table-rl")];
            if varies {
                headings.extend([tr!("pit-design-table-face"), tr!("pit-design-table-berm")]);
            }
            if several {
                headings.push(tr!("pit-design-table-ramp"));
            }
            headings.extend([
                tr!("pit-design-table-grade"),
                tr!("pit-design-table-flat"),
                tr!("pit-design-table-width"),
                tr!("pit-design-table-turn"),
            ]);
            if flagged {
                headings.push(tr!("pit-design-checks"));
            }
            for heading in headings {
                ui.label(egui::RichText::new(heading).color(weak).small());
            }
            ui.end_row();
            for (number, bench) in design.report.iter().enumerate() {
                let top = bench.elevation + sign * bench.height;
                // A row for each ramp's stretch on the bench, or one for a
                // bench with none; the bench's own columns on the first.
                let roads: Vec<Option<&crate::model::pit_design::RampReport>> = if bench.ramps.is_empty() { vec![None] } else { bench.ramps.iter().map(Some).collect() };
                let row_top = ui.cursor().min.y;
                // The bench's flags, said in its last column on its first row.
                let flags: Vec<&Flag> = design.flags.iter().filter(|flag| flag.bench == number).collect();
                let checks_cell = |ui: &mut egui::Ui, row: usize| {
                    if !flagged {
                        return;
                    }
                    if row > 0 || flags.is_empty() {
                        ui.label("");
                        return;
                    }
                    let text = flags.iter().map(|flag| flag_text(flag, settings)).collect::<Vec<_>>().join("\n\n");
                    ui.colored_label(warning, flags.len().to_string()).hint(text);
                };
                for (row, road) in roads.into_iter().enumerate() {
                    if row == 0 {
                        ui.label(format!("{:.1}–{top:.1}", bench.elevation));
                        if varies {
                            ui.label(span(bench.face_angle, 0, "°"));
                            ui.label(span(bench.berm, 1, " m"));
                        }
                    } else {
                        for _ in 0..if varies { 3 } else { 1 } {
                            ui.label("");
                        }
                    }
                    if several {
                        ui.label(road.map_or_else(String::new, |road| (road.ramp + 1).to_string()));
                    }
                    let Some((road, ramp)) = road.and_then(|road| Some((road, settings.ramps.get(road.ramp)?))) else {
                        for _ in 0..4 {
                            ui.label("");
                        }
                        checks_cell(ui, row);
                        ui.end_row();
                        continue;
                    };
                    let gradient = ramp.gradient / 100.0;
                    let grade = format!("{:.1} %", road.steepest * 100.0);
                    let tolerance = crate::model::pit_design::checks::GRADE_TOLERANCE;
                    if road.steepest > gradient + tolerance {
                        ui.colored_label(warning, grade)
                            .hint(tr!("pit-design-table-steeper-hint", grade = format!("{:.1}", ramp.gradient)));
                    } else if road.lengthened > 0.0 || road.steepest < gradient - tolerance {
                        ui.colored_label(warning, grade)
                            .hint(tr!("pit-design-table-flatter-hint", length = format!("{:.0}", road.lengthened)));
                    } else {
                        ui.label(grade);
                    }
                    ui.label(if road.flat > 0.0 { format!("{:.0} m", road.flat) } else { String::new() });
                    // As wide as its band asks, or narrowing from the last
                    // bench's width as it tapers to this one's.
                    let wanted = crate::model::pit_design::checks::wanted_width(&ramp.spec(DVec2::ZERO), design, number, sign);
                    let width = format!("{:.1} m", road.narrowest);
                    if road.narrowest < wanted * (1.0 - crate::model::pit_design::checks::WIDTH_TOLERANCE) {
                        ui.colored_label(warning, width).hint(tr!("pit-design-table-narrow-hint", width = format!("{wanted:.1}")));
                    } else {
                        ui.label(width);
                    }
                    match (road.switchback, road.turn_moved_from) {
                        (true, Some(asked)) => {
                            ui.colored_label(warning, tr!("pit-design-table-turned")).hint(tr!(
                                "pit-design-turn-moved",
                                asked = format!("{asked:.1}"),
                                made = format!("{:.1}", bench.elevation)
                            ));
                        }
                        (true, None) => {
                            ui.label(tr!("pit-design-table-turned"));
                        }
                        _ => {
                            ui.label("");
                        }
                    }
                    checks_cell(ui, row);
                    ui.end_row();
                }
                let rows = egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), row_top..=ui.cursor().min.y);
                if ui.rect_contains_pointer(rows) {
                    hover.bench = Some(number);
                }
            }
        });
}
