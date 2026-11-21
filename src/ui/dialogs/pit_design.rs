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
            evaluation::{Evaluation, Figures, RehabFigures, RehabSettings, ShellFigures, Waiting},
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

/// A ramp's gizmo in the viewport, in physical window pixels, at its foot:
/// a ring standing on the road's line, which turns its grade (seen from
/// above it lies flat, off the road's side), and an arrow out across the
/// road, which widens it.
#[derive(Clone, Debug)]
pub(crate) struct RampGizmo {
    /// Which ramp, by its place in the list.
    pub(crate) ramp: usize,
    /// How much of its ring shows: none edge on, where it cannot be taken;
    /// and of its width arrow, none pointing at the camera.
    pub(crate) ring_fade: f32,
    pub(crate) width_fade: f32,
    pub(crate) centre: (f32, f32),
    /// The ring, a whole turn from the way the road goes, round over the
    /// top: a point every [`RING_STEPS`]th of a turn.
    pub(crate) ring: Vec<(f32, f32)>,
    /// The ring from the way the road goes up to its grade.
    pub(crate) grade_arc: Vec<(f32, f32)>,
    /// Where the ring stands at the road's grade, and the same the other way
    /// round, where the road would climb back.
    pub(crate) grade_handle: (f32, f32),
    pub(crate) turn_handle: (f32, f32),
    /// The width arrow, from the ring out across the road, its way a unit
    /// on screen, with the pixels a metre across takes.
    pub(crate) width_from: (f32, f32),
    pub(crate) width_tip: (f32, f32),
    pub(crate) across: (f32, f32),
    pub(crate) pixels_per_metre: f32,
}

/// How much steeper than it is a ramp's grade is drawn on its ring, so a
/// few percent reads and turns as a sizeable angle: 10 % at 22 degrees, 25 %
/// at 45.
pub(crate) const GRADE_EXAGGERATION: f64 = 4.0;

/// How many points a ramp gizmo's ring is sampled with round a whole turn.
pub(crate) const RING_STEPS: usize = 72;

/// The walls' gizmo: a section of the first bench standing on the pit's
/// string, where it is furthest from the ramps, drawn to a fixed size on
/// screen (seen from above, it lies flat along the wall). In physical window
/// pixels.
#[derive(Clone, Debug)]
pub(crate) struct WallGizmo {
    /// How much of its face's ring shows, edge on none, and of its height
    /// arrow, seen down it none: neither can then be taken.
    pub(crate) ring_fade: f32,
    pub(crate) height_fade: f32,
    /// How much of its berm arrow shows: none pointing at the camera.
    pub(crate) berm_fade: f32,
    /// The bench's toe, on the string.
    pub(crate) toe: (f32, f32),
    /// Up the section and out across it, units on screen, and the pixels a
    /// metre takes on the section.
    pub(crate) up: (f32, f32),
    pub(crate) out: (f32, f32),
    pub(crate) pixels_per_metre_up: f32,
    pub(crate) pixels_per_metre_out: f32,
    /// The face's ring about the toe, a quarter turn from out across to up:
    /// a point every [`RING_STEPS`]th of a turn; and its handle, on the face.
    pub(crate) angle_ring: Vec<(f32, f32)>,
    pub(crate) angle_handle: (f32, f32),
    /// The height arrow up from the toe, and the berm arrow out across from
    /// it: each from the toe to its tip.
    pub(crate) height_from: (f32, f32),
    pub(crate) height_tip: (f32, f32),
    pub(crate) berm_from: (f32, f32),
    pub(crate) berm_tip: (f32, f32),
}

/// A part of a gizmo: a ramp's, by the ramp's place in the list, or the
/// walls'.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GizmoPart {
    Width(usize),
    Grade(usize),
    Turn(usize),
    BenchHeight,
    FaceAngle,
    Berm,
    /// A place on a ramp it can switch back, by its place in the spots.
    Switch(usize),
}

/// A place on a ramp it can switch back: where its stretch on a bench
/// begins, or where it turns back, if it does there. Clicked, it turns
/// there or stops turning there. In physical window pixels.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SwitchSpot {
    pub(crate) ramp: usize,
    /// The RL the turn is asked at: the bench's, or where it was asked if
    /// it was made later.
    pub(crate) rl: f64,
    pub(crate) at: (f32, f32),
    /// Whether it turns back here.
    pub(crate) turn: bool,
}

/// A part of the walls' gizmo being dragged: which, where the press was
/// (and where round the face's ring), its value then, and its arm's way on
/// screen and pixels a metre then, held for the drag: pulled back through
/// the toe, the walls turn to go the other way, and the arm with them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WallDrag {
    pub(crate) part: GizmoPart,
    pub(crate) from: (f32, f32),
    pub(crate) value: f64,
    pub(crate) pressed: f64,
    pub(crate) way: (f32, f32),
    pub(crate) per_metre: f32,
    /// What the design was and the way it was built when taken up.
    pub(crate) kind: Kind,
    pub(crate) direction: Direction,
}

/// The panel's folding sections, folded each time a design is opened.
const PANEL_FOLDS: [&str; 6] = [
    "pit_design_bench_rules",
    "pit_design_slope_rules",
    "pit_design_ramp_extent",
    "pit_design_bench_table",
    "pit_design_walls_table",
    "pit_design_evaluation",
];

/// The panel's state.
#[derive(Clone, Debug, Default)]
pub(crate) struct PitDesignDraft {
    /// Whether the panel's sections have been folded for this design yet.
    pub(crate) folds_reset: bool,
    /// Where each ramp can switch back, as the viewport draws them.
    pub(crate) switch_spots: Vec<SwitchSpot>,
    /// Each ramp's handles, as the viewport draws them.
    pub(crate) gizmos: Vec<RampGizmo>,
    /// The handle under the pointer.
    pub(crate) gizmo_hover: Option<GizmoPart>,
    /// A ramp being widened by its handle: which, where the press was and
    /// the width then.
    pub(crate) widening: Option<(usize, (f32, f32), f64)>,
    /// The part of the viewport the panel leaves clear, in physical window
    /// pixels (left, top, right, bottom): where the walls' gizmo is put.
    pub(crate) clear_view: Option<[f32; 4]>,
    /// The settings as they were when a gizmo's handle was taken up: what
    /// Escape puts back.
    pub(crate) drag_from: Option<PitDesignSettings>,
    /// Where on the pit's string the walls' gizmo stands, kept while it is in
    /// view so it stays where it was left.
    pub(crate) wall_place: Option<DVec2>,
    /// The walls' gizmo, as the viewport draws it.
    pub(crate) wall_gizmo: Option<WallGizmo>,
    /// A part of the walls' gizmo being dragged.
    pub(crate) wall_drag: Option<WallDrag>,
    /// A ramp whose grade is being turned on its ring: which, where round
    /// the ring the press was, and the grade's angle then.
    pub(crate) grading: Option<(usize, f64, f64)>,
    /// Where each of the preview's flags stands on screen.
    pub(crate) flag_px: Vec<Option<(f32, f32)>>,
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
    /// The ramp's start is being dragged along the string, from where the
    /// press was: let go, it is placed (or, not moved, it waits for a click).
    pub(crate) dragging_ramp: Option<(f32, f32)>,
    /// The ramp start marker under the pointer, which a press drags.
    pub(crate) ramp_marker_under_cursor: Option<usize>,
    /// The viewport draws the preview in place of the object's own design.
    pub(crate) covering: bool,
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
    /// copy differs from it, its own design is not yet current, or a ramp
    /// is shown where a click would start it.
    pub(crate) fn shows_preview(&self) -> bool {
        self.object.is_some() && (self.working != self.synced || !self.facts.fresh || self.ramp_previewed().is_some())
    }

    /// The ramp, and where on the string, the preview starts it while its
    /// start is picked: where a click would put it.
    pub(crate) fn ramp_previewed(&self) -> Option<(usize, DVec3)> {
        self.picking_ramp.zip(self.ramp_hover)
    }

    /// Whether the viewport draws the preview in place of the object's own
    /// design: it shows one, and has a design to show.
    pub(crate) fn covers_object(&self) -> bool {
        self.shows_preview() && self.preview.as_ref().is_some_and(|preview| preview.design.is_ok())
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

/// The ground under the pointer while a pit's string is drawn, beside the
/// pointer: what the RL is set by, and how far below or above the string
/// would sit. The RL itself is the toolbar's Z.
pub(crate) fn draw_pit_ground_tag(ui: &egui::Ui, editor: &EditorState, canvas_rect: egui::Rect) {
    if !editor.pit_drawing() {
        return;
    }
    let Some(reading) = editor.pit_ground_under_cursor else {
        return;
    };
    let ctx = ui.ctx();
    let Some(pointer) = ctx.pointer_hover_pos().filter(|pos| canvas_rect.contains(*pos)) else {
        return;
    };
    // Only over the scene itself, not the panel or a menu over it.
    if ctx.layer_id_at(pointer).is_some_and(|layer| layer != egui::LayerId::background()) {
        return;
    }
    let text = reading.map_or_else(
        || "-".to_owned(),
        |ground| {
            let (rl, depth) = (format!("{ground:.1}"), ground - editor.z_level);
            if depth >= 0.0 {
                tr!("pit-design-draw-ground-above", rl = rl, by = format!("{depth:.1}"))
            } else {
                tr!("pit-design-draw-ground-below", rl = rl, by = format!("{:.1}", -depth))
            }
        },
    );
    let visuals = ui.visuals();
    let painter = ctx
        .layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("pit_ground_tag")))
        .with_clip_rect(canvas_rect);
    let font = egui::FontId::proportional(12.0);
    let label = painter.layout_no_wrap(tr!("pit-design-draw-ground"), font.clone(), visuals.weak_text_color());
    let value = painter.layout_no_wrap(text, font, visuals.text_color());
    let gap = 6.0;
    let size = egui::vec2(label.size().x + gap + value.size().x, label.size().y.max(value.size().y)) + egui::vec2(12.0, 8.0);
    // Below and right of the pointer, turned back in at the view's edges.
    let mut min = pointer + egui::vec2(16.0, 20.0);
    if min.x + size.x > canvas_rect.right() {
        min.x = pointer.x - 16.0 - size.x;
    }
    if min.y + size.y > canvas_rect.bottom() {
        min.y = pointer.y - 20.0 - size.y;
    }
    let rect = egui::Rect::from_min_size(min, size);
    painter.rect_filled(rect, crate::ui::widgets::toolbar::GROUP_CORNER_RADIUS, visuals.window_fill());
    let label_width = label.size().x;
    painter.galley(rect.min + egui::vec2(6.0, 4.0), label, visuals.weak_text_color());
    painter.galley(rect.min + egui::vec2(6.0 + label_width + gap, 4.0), value, visuals.text_color());
}

/// The Pit Design tool's panel, docked in the viewport while a pit design is
/// open in it: its name and settings, and what it comes to.
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
            // Each time a design is opened, its sections start folded: they
            // open only when asked.
            if !draft.folds_reset {
                draft.folds_reset = true;
                ui.data_mut(|data| {
                    for id in PANEL_FOLDS {
                        data.remove::<bool>(egui::Id::unique(id));
                    }
                });
            }
            // Each step scrolls on its own, from its top.
            let stage_salt = match draft.stage {
                PitStage::Benches => "benches",
                PitStage::Ramp => "ramp",
            };
            // The step's settings and what they come to scroll between the
            // name and the actions, which stay in view.
            egui::ScrollArea::vertical()
                .id_salt(("pit_design_body", stage_salt))
                .max_height(body_height(viewport_rect))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let stage = draft.stage;
                    let picking = draft.picking_ramp;
                    let picking_region = draft.picking_region;
                    // With the walls' gizmo up, what it sets is set there.
                    let on_gizmo = draft.wall_gizmo.is_some();
                    // The ramps whose gizmos stand in what the panel leaves of
                    // the view.
                    let in_view: Vec<usize> = draft
                        .gizmos
                        .iter()
                        .filter(|gizmo| {
                            draft
                                .clear_view
                                .is_some_and(|[left, top, right, bottom]| gizmo.centre.0 > left && gizmo.centre.0 < right && gizmo.centre.1 > top && gizmo.centre.1 < bottom)
                        })
                        .map(|gizmo| gizmo.ramp)
                        .collect();
                    let Some(working) = draft.working.as_mut() else {
                        return;
                    };
                    match stage {
                        PitStage::Benches => {
                            let base_rl = draft.built_for.as_ref().and_then(|key| key.verts.first()).map_or(0.0, |vertex| vertex.pos.z);
                            draw_bench_settings(ui, &mut working.settings, project, (base_rl, picking_region, on_gizmo), commands);
                        }
                        PitStage::Ramp => {
                            let verts = draft.built_for.as_ref().map_or(&[][..], |key| key.verts.as_slice());
                            let base_rl = verts.first().map_or(0.0, |vertex| vertex.pos.z);
                            draw_ramps(ui, &mut working.settings, (picking, &in_view), (base_rl, commands));
                        }
                    }
                    // While a design is worked out, a spinner at the heading's
                    // end: a line coming and going would jog the panel about.
                    let top = ui.cursor().top();
                    menu::menu_section(ui, tr!("pit-design-result-section"));
                    if draft.building.is_some() {
                        heading_spinner(ui, top);
                    }
                    // What the design shown was worked out with: the copy's
                    // settings, or with a ramp where a click would start it.
                    let settings = draft.built_for.as_ref().map_or_else(|| working.settings(), |key| key.settings.clone());
                    let hover = draw_result(ui, draft, &settings);
                    if hover != draft.hover {
                        commands.push(UiCommand::PitDesignHover(hover));
                    }
                    if let Some(working) = draft.working.as_mut() {
                        draw_evaluation(ui, working, &draft.facts, project);
                    }
                });

            let fresh = draft.facts.fresh;
            let picking = draft.picking_ramp.is_some();
            menu::menu_actions(ui, |ui| match draft.stage {
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
                    // On to the rest of the project, at the row's far end.
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        if let Some(working) = draft.working.as_ref() {
                            draw_send_button(ui, working, id, fresh && !picking, commands);
                        }
                    });
                }
            });
        });
    // What the panel leaves of the viewport, for the walls' gizmo to stand in.
    let per_point = ui.ctx().pixels_per_point();
    let left = ui
        .ctx()
        .memory(|memory| memory.area_rect(egui::Id::unique("pit_design_panel")))
        .map_or(viewport_rect.left(), |panel| panel.right().max(viewport_rect.left()));
    draft.clear_view = Some([
        left * per_point,
        viewport_rect.top() * per_point,
        viewport_rect.right() * per_point,
        viewport_rect.bottom() * per_point,
    ]);
    draw_viewport_handles(ui, draft, commands, viewport_rect);
    let over_viewport = ui.ctx().pointer_hover_pos().is_some_and(|pointer| viewport_rect.contains(pointer)) && !ui.ctx().egui_wants_pointer_input();
    // A gizmo's handles say what they do by their shape: nothing is said
    // over them.
    let over_viewport = over_viewport && draft.gizmo_hover.is_none();
    if draft.ramp_marker_under_cursor.is_some() && over_viewport {
        crate::ui::widgets::hint::hint_at_pointer(ui.ctx(), egui::Id::unique("pit_design_ramp_drag_hint"), tr!("pit-design-ramp-drag-hint"));
    } else if let Some(flag) = draft
        .flag_under_cursor
        .and_then(|index| draft.preview.as_ref()?.design.as_ref().ok()?.flags.get(index))
        .filter(|_| over_viewport)
    {
        // The flag under the pointer in the viewport, said beside it.
        let settings = draft.working.as_ref().map(PitObject::settings).unwrap_or_default();
        crate::ui::widgets::hint::hint_at_pointer(ui.ctx(), egui::Id::unique("pit_design_flag_hint"), flag_text(flag, &settings));
    }
    // A field is mid-edit while the pointer is down on it or text is being
    // typed into it; until then the copy waits to be written.
    draft.settled = !ui.ctx().input(|input| input.pointer.any_down()) && ui.ctx().memory(|memory| memory.focused().is_none());
}

/// In the viewport, over the design, each ramp's gizmo at its foot, in the
/// manner of the Move and Rotate gizmos: its grade ring, a half turn
/// standing on the road's line with the grade swept out of it and a handle
/// at the grade, the same handle the other way round faint (a click climbs
/// that way), and its width arrow out across the road; each with its value
/// beside it. And beside each place the design falls short, what falls
/// short, which a click goes to.
fn draw_viewport_handles(ui: &mut egui::Ui, draft: &mut PitDesignDraft, commands: &mut Vec<UiCommand>, viewport_rect: egui::Rect) {
    use crate::ui::{GIZMO_HEAD_HALF_WIDTH, GIZMO_HEAD_LENGTH, GIZMO_IDLE_ALPHA, ROTATE_RING_COLORS, gizmo_color};
    let per_point = ui.ctx().pixels_per_point();
    let at = |px: (f32, f32)| egui::pos2(px.0 / per_point, px.1 / per_point);
    let painter = ui.painter().with_clip_rect(viewport_rect);
    let grade_colour = ROTATE_RING_COLORS[1];
    let width_colour = {
        let [r, g, b, _] = crate::model::pit_design::StringKind::RampEdge.colour();
        egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
    };
    // Each handle's value beside it, to drag or type: gathered as the
    // gizmos are drawn, shown once they are.
    let mut fields: Vec<GizmoField> = Vec::new();
    let mut readout = |anchor: egui::Pos2, way: egui::Vec2, value: GizmoValue, lit: bool| fields.push(GizmoField { anchor, way, value, lit });
    // Where each ramp can switch back: a ring where it could, filled where
    // it does; a click turns it there or stops it.
    for (index, spot) in draft.switch_spots.iter().enumerate() {
        let centre = at(spot.at);
        if !viewport_rect.contains(centre) {
            continue;
        }
        let lit = draft.gizmo_hover == Some(GizmoPart::Switch(index));
        // In the ramp's own colour: amber is what falls short.
        let colour = if lit { egui::Color32::WHITE } else { width_colour };
        if spot.turn {
            painter.circle_filled(centre, if lit { 7.0 } else { 6.0 }, gizmo_color(colour, if lit { 1.0 } else { 0.9 }));
            painter.circle_stroke(centre, if lit { 7.0 } else { 6.0 }, egui::Stroke::new(1.5, gizmo_color(egui::Color32::BLACK, 0.5)));
        } else {
            painter.circle_filled(centre, 5.0, gizmo_color(egui::Color32::BLACK, if lit { 0.45 } else { 0.25 }));
            painter.circle_stroke(centre, 5.0, egui::Stroke::new(1.5, gizmo_color(colour, if lit { 1.0 } else { GIZMO_IDLE_ALPHA })));
        }
    }
    for gizmo in &draft.gizmos {
        let centre = at(gizmo.centre);
        if !viewport_rect.contains(centre) {
            continue;
        }
        if draft.working.as_ref().and_then(|working| working.settings.ramps.get(gizmo.ramp)).is_none() {
            continue;
        }
        let active = |part: GizmoPart| {
            draft.gizmo_hover == Some(part)
                || match part {
                    GizmoPart::Width(index) => draft.widening.is_some_and(|(dragged, ..)| dragged == index),
                    GizmoPart::Grade(index) => draft.grading.is_some_and(|(dragged, ..)| dragged == index),
                    GizmoPart::Turn(_) | GizmoPart::BenchHeight | GizmoPart::FaceAngle | GizmoPart::Berm | GizmoPart::Switch(_) => false,
                }
        };
        // The grade ring: its half above the road's line, the grade swept
        // out of it, and its handle.
        // Faded out edge on, as a Blender rotate ring: its value stays, to
        // drag or type.
        let lit = active(GizmoPart::Grade(gizmo.ramp));
        let fade = gizmo.ring_fade;
        let (colour, alpha) = if lit { (egui::Color32::WHITE, fade) } else { (grade_colour, GIZMO_IDLE_ALPHA * fade) };
        let handle = at(gizmo.grade_handle);
        if fade > 0.0 {
            let half: Vec<egui::Pos2> = gizmo.ring[..=RING_STEPS / 2].iter().copied().map(at).collect();
            painter.line_segment(
                [half[0], half[half.len() - 1]],
                egui::Stroke::new(1.0, gizmo_color(grade_colour, GIZMO_IDLE_ALPHA * 0.5 * fade)),
            );
            let mut sector = vec![centre];
            sector.extend(gizmo.grade_arc.iter().copied().map(at));
            if sector.len() >= 3 {
                painter.add(egui::Shape::convex_polygon(
                    sector,
                    gizmo_color(grade_colour, if lit { 0.45 } else { 0.25 } * fade),
                    egui::Stroke::NONE,
                ));
            }
            painter.add(egui::Shape::line(half, egui::Stroke::new(if lit { 3.0 } else { 2.0 }, gizmo_color(colour, alpha))));
            painter.line_segment([centre, handle], egui::Stroke::new(2.0, gizmo_color(colour, alpha)));
            painter.circle_filled(handle, if lit { 6.0 } else { 5.0 }, gizmo_color(colour, alpha.max(0.85 * fade)));
            // The other way round, faint: a click climbs that way.
            let turn_lit = active(GizmoPart::Turn(gizmo.ramp));
            let (colour, alpha) = if turn_lit {
                (egui::Color32::WHITE, fade)
            } else {
                (grade_colour, GIZMO_IDLE_ALPHA * 0.6 * fade)
            };
            painter.circle_stroke(at(gizmo.turn_handle), 4.5, egui::Stroke::new(1.5, gizmo_color(colour, alpha)));
        }
        // Its value beyond the handle, or seen from above, beside the foot.
        if fade > 0.0 {
            let way = (handle - centre).normalized();
            readout(handle + way * 10.0, way, GizmoValue::Gradient(gizmo.ramp), lit);
        } else {
            readout(centre + egui::vec2(12.0, -12.0), egui::vec2(1.0, -1.0).normalized(), GizmoValue::Gradient(gizmo.ramp), lit);
        }
        // The width arrow: faded out pointing at the camera, as a Blender
        // axis arrow, its value staying beside the foot.
        let lit = active(GizmoPart::Width(gizmo.ramp));
        let fade = gizmo.width_fade;
        let (colour, alpha) = if lit { (egui::Color32::WHITE, fade) } else { (width_colour, GIZMO_IDLE_ALPHA * fade) };
        let (from, tip) = (at(gizmo.width_from), at(gizmo.width_tip));
        if fade > 0.0 && (tip - from).length() > GIZMO_HEAD_LENGTH {
            let way = (tip - from).normalized();
            let head = tip - way * GIZMO_HEAD_LENGTH;
            let side = egui::vec2(-way.y, way.x) * GIZMO_HEAD_HALF_WIDTH * if lit { 1.15 } else { 1.0 };
            painter.line_segment([from, head], egui::Stroke::new(if lit { 3.0 } else { 2.0 }, gizmo_color(colour, alpha)));
            painter.add(egui::Shape::convex_polygon(
                vec![tip, head + side, head - side],
                gizmo_color(colour, alpha),
                egui::Stroke::NONE,
            ));
            readout(tip + way * 8.0, way, GizmoValue::Width(gizmo.ramp), lit);
        } else {
            readout(centre + egui::vec2(12.0, 12.0), egui::vec2(1.0, 1.0).normalized(), GizmoValue::Width(gizmo.ramp), lit);
        }
        // Where it turns about.
        painter.circle_filled(centre, 3.0, gizmo_color(egui::Color32::WHITE, GIZMO_IDLE_ALPHA));
    }
    draw_wall_gizmo(ui, draft, viewport_rect, &mut readout);
    gizmo_fields(ui, draft, &fields, viewport_rect);
    // What falls short, beside each place it does: every one while there are
    // few, the one under the pointer while there are many.
    let Some(design) = draft.preview.as_ref().and_then(|preview| preview.design.as_ref().ok()) else {
        return;
    };
    let settings = draft.working.as_ref().map(PitObject::settings).unwrap_or_default();
    let few = design.flags.len() <= FLAG_LABELS;
    let amber = ui.visuals().warn_fg_color;
    for (index, flag) in design.flags.iter().enumerate() {
        let Some(Some(px)) = draft.flag_px.get(index) else {
            continue;
        };
        let point = at(*px);
        if !viewport_rect.contains(point) || !(few || draft.flag_under_cursor == Some(index)) {
            continue;
        }
        let text = format!("{}  {}", check_label(flag.check), flag_figure(flag.check, flag.value));
        egui::Area::new(egui::Id::unique(("pit_design_flag_label", index)))
            .order(egui::Order::Middle)
            .fixed_pos(point + egui::vec2(10.0, -22.0))
            .show(ui.ctx(), |ui| {
                let response = egui::Frame::new()
                    .fill(egui::Color32::from_black_alpha(190))
                    .corner_radius(egui::CornerRadius::same(3))
                    .inner_margin(egui::Margin::symmetric(5, 1))
                    .show(ui, |ui| ui.add(egui::Label::new(egui::RichText::new(text).small().color(amber)).extend()))
                    .response
                    .interact(egui::Sense::click());
                let response = response.on_hover_cursor(egui::CursorIcon::PointingHand).hint(flag_text(flag, &settings));
                if response.clicked() {
                    commands.push(UiCommand::FramePitFlag(index));
                }
            });
    }
}

/// The walls' gizmo: the first bench's section standing on the string, its
/// face's ring about the toe with a handle on the face, an arrow up from the
/// crest for the bench's height and one out along the berm for its width,
/// each with its value; and above it, how far the design goes, to type or
/// drag.
fn draw_wall_gizmo(ui: &mut egui::Ui, draft: &mut PitDesignDraft, viewport_rect: egui::Rect, readout: &mut dyn FnMut(egui::Pos2, egui::Vec2, GizmoValue, bool)) {
    use crate::ui::{GIZMO_AXIS_COLORS, GIZMO_HEAD_HALF_WIDTH, GIZMO_HEAD_LENGTH, GIZMO_IDLE_ALPHA, ROTATE_RING_COLORS, gizmo_color};
    let Some(gizmo) = draft.wall_gizmo.clone() else {
        return;
    };
    let per_point = ui.ctx().pixels_per_point();
    let at = |px: (f32, f32)| egui::pos2(px.0 / per_point, px.1 / per_point);
    let toe = at(gizmo.toe);
    if !viewport_rect.contains(toe) {
        return;
    }
    let painter = ui.painter().with_clip_rect(viewport_rect);
    let active = |part: GizmoPart| draft.gizmo_hover == Some(part) || draft.wall_drag.is_some_and(|drag| drag.part == part);
    let Some(settings) = draft.working.as_ref().map(|working| working.settings.clone()) else {
        return;
    };
    let out = egui::vec2(gizmo.out.0, gizmo.out.1);
    // The face's ring, a quarter turn from out across to up, the face's
    // angle swept out of it: faded out edge on, as a Blender rotate ring,
    // its value staying beside the toe to drag or type.
    let lit = active(GizmoPart::FaceAngle);
    let fade = gizmo.ring_fade;
    let (colour, alpha) = if lit {
        (egui::Color32::WHITE, fade)
    } else {
        (ROTATE_RING_COLORS[1], GIZMO_IDLE_ALPHA * fade)
    };
    // The ring's steps the angle passes, short of it: the sweep ends at the
    // handle itself, never past it.
    let steps = ((settings.face_angle / 360.0 * RING_STEPS as f64).floor() as usize).min(RING_STEPS / 4);
    if fade > 0.0 {
        let handle = at(gizmo.angle_handle);
        let mut swept: Vec<egui::Pos2> = gizmo.angle_ring[..=steps].iter().copied().map(at).collect();
        swept.push(handle);
        let mut sector = vec![toe];
        sector.extend(swept.iter().copied());
        painter.add(egui::Shape::convex_polygon(
            sector,
            gizmo_color(ROTATE_RING_COLORS[1], if lit { 0.4 } else { 0.22 } * fade),
            egui::Stroke::NONE,
        ));
        // The rest of the quarter, up to standing upright, is only the track
        // the handle runs on.
        let mut track = vec![handle];
        track.extend(gizmo.angle_ring[steps + 1..=RING_STEPS / 4].iter().copied().map(at));
        if track.len() >= 2 {
            painter.add(egui::Shape::line(track, egui::Stroke::new(1.0, gizmo_color(colour, alpha * 0.35))));
        }
        painter.add(egui::Shape::line(swept, egui::Stroke::new(if lit { 3.0 } else { 2.0 }, gizmo_color(colour, alpha))));
        painter.circle_filled(handle, if lit { 6.0 } else { 5.0 }, gizmo_color(colour, alpha.max(0.85 * fade)));
    }
    // Its value outside the ring, halfway between the arms, where neither
    // can run into it whatever the angle; or seen from above, behind the toe.
    if fade > 0.0 {
        let corner = at(gizmo.angle_ring[RING_STEPS / 8]);
        let way = (corner - toe).normalized();
        readout(corner + way * 8.0, way, GizmoValue::FaceAngle, lit);
    } else {
        readout(toe - out * 12.0, -out, GizmoValue::FaceAngle, lit);
    }
    // An arrow, Move gizmo style.
    let arrow = |from: egui::Pos2, tip: egui::Pos2, base: egui::Color32, lit: bool, fade: f32| {
        if fade <= 0.0 || (tip - from).length() < GIZMO_HEAD_LENGTH {
            return;
        }
        let (colour, alpha) = if lit { (egui::Color32::WHITE, fade) } else { (base, GIZMO_IDLE_ALPHA * fade) };
        let way = (tip - from).normalized();
        let head = tip - way * GIZMO_HEAD_LENGTH;
        let side = egui::vec2(-way.y, way.x) * GIZMO_HEAD_HALF_WIDTH * if lit { 1.15 } else { 1.0 };
        painter.line_segment([from, head], egui::Stroke::new(if lit { 3.0 } else { 2.0 }, gizmo_color(colour, alpha)));
        painter.add(egui::Shape::convex_polygon(
            vec![tip, head + side, head - side],
            gizmo_color(colour, alpha),
            egui::Stroke::NONE,
        ));
    };
    let lit = active(GizmoPart::BenchHeight);
    let height_tip = at(gizmo.height_tip);
    // Seen down it, faded out as a Blender axis arrow; its value stays
    // beside the crest.
    arrow(at(gizmo.height_from), height_tip, GIZMO_AXIS_COLORS[2], lit, gizmo.height_fade);
    if gizmo.height_fade > 0.0 {
        let way = (height_tip - toe).normalized();
        readout(height_tip + way * 8.0, way, GizmoValue::BenchHeight, lit);
    } else {
        let side = egui::vec2(-out.y, out.x);
        readout(toe + side * 14.0, side, GizmoValue::BenchHeight, lit);
    }
    let lit = active(GizmoPart::Berm);
    let berm_tip = at(gizmo.berm_tip);
    arrow(at(gizmo.berm_from), berm_tip, GIZMO_AXIS_COLORS[1], lit, gizmo.berm_fade);
    let way = (berm_tip - toe).normalized();
    readout(berm_tip + way * 8.0, way, GizmoValue::Berm, lit);
    // How far the design goes, off the toe away from both arms, to type or
    // drag.
    let away = -(egui::vec2(gizmo.up.0, gizmo.up.1) + out).normalized();
    let Some(settings) = draft.working.as_mut().map(|working| &mut working.settings) else {
        return;
    };
    egui::Area::new(egui::Id::unique("pit_design_wall_extent"))
        .order(egui::Order::Middle)
        .pivot(pivot_toward(away))
        .fixed_pos(toe + away * 14.0)
        .constrain_to(viewport_rect)
        .show(ui.ctx(), |ui| {
            crate::ui::viewport_input_frame(ui, crate::ui::ViewportInputState::Idle).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    match settings.stop {
                        StopMode::Benches => {
                            ui.label(tr!("pit-design-benches"));
                            ui.add_sized([48.0, 22.0], egui::DragValue::new(&mut settings.benches).range(1..=u32::MAX).speed(0.05));
                        }
                        StopMode::Elevation => {
                            ui.label(tr!("pit-design-stop-rl"));
                            ui.add_sized([72.0, 22.0], egui::DragValue::new(&mut settings.stop_elevation).speed(0.5).fixed_decimals(1));
                            ui.label(tr!("common-m"));
                        }
                        StopMode::Daylight => {
                            let kind = settings.kind;
                            ui.label(stop_label(kind, StopMode::Daylight));
                        }
                    }
                });
            });
        });
}

/// Which corner or edge of a box to hold at a point for the box to lie off
/// it the way `way` points: off to the right, its left edge; above, its
/// bottom; and so on.
fn pivot_toward(way: egui::Vec2) -> egui::Align2 {
    let along = |component: f32| {
        if component > 0.38 {
            egui::Align::Min
        } else if component < -0.38 {
            egui::Align::Max
        } else {
            egui::Align::Center
        }
    };
    egui::Align2([along(way.x), along(way.y)])
}

/// A value a gizmo's handle sets: a ramp's, by its place in the list, or
/// the walls'.
#[derive(Clone, Copy)]
enum GizmoValue {
    Gradient(usize),
    Width(usize),
    BenchHeight,
    FaceAngle,
    Berm,
}

/// A handle's value: the point it stands off, the way it lies from there,
/// and whether the handle is lit.
struct GizmoField {
    anchor: egui::Pos2,
    way: egui::Vec2,
    value: GizmoValue,
    lit: bool,
}

/// Each handle's value beside it, a small field to drag or click and type
/// into, as the panel's are: what the gizmo sets, set exactly.
fn gizmo_fields(ui: &mut egui::Ui, draft: &mut PitDesignDraft, fields: &[GizmoField], viewport_rect: egui::Rect) {
    let Some(settings) = draft.working.as_mut().map(|working| &mut working.settings) else {
        return;
    };
    for (index, field) in fields.iter().enumerate() {
        let (value, unit, range, speed): (&mut f64, String, std::ops::RangeInclusive<f64>, f64) = match field.value {
            GizmoValue::Gradient(ramp) => match settings.ramps.get_mut(ramp) {
                Some(ramp) => (&mut ramp.gradient, "%".to_owned(), 0.1..=100.0, 0.1),
                None => continue,
            },
            GizmoValue::Width(ramp) => match settings.ramps.get_mut(ramp) {
                Some(ramp) => (&mut ramp.width, tr!("common-m"), 0.1..=1000.0, 0.1),
                None => continue,
            },
            GizmoValue::BenchHeight => (&mut settings.bench_height, tr!("common-m"), 0.1..=1000.0, 0.1),
            GizmoValue::FaceAngle => (&mut settings.face_angle, "°".to_owned(), 1.0..=89.0, 0.5),
            GizmoValue::Berm => (&mut settings.berm_width, tr!("common-m"), 0.0..=1000.0, 0.1),
        };
        let state = if field.lit {
            crate::ui::ViewportInputState::Lit
        } else {
            crate::ui::ViewportInputState::Idle
        };
        egui::Area::new(egui::Id::unique(("pit_design_gizmo_field", index)))
            .order(egui::Order::Middle)
            .pivot(pivot_toward(field.way))
            .fixed_pos(field.anchor)
            .constrain_to(viewport_rect)
            .show(ui.ctx(), |ui| {
                crate::ui::viewport_input_frame(ui, state).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.add_sized([52.0, 22.0], egui::DragValue::new(value).range(range).speed(speed).fixed_decimals(1));
                        ui.label(unit);
                    });
                });
            });
    }
}

/// The most flags labelled in the viewport at once: past this, only the one
/// under the pointer is.
const FLAG_LABELS: usize = 12;

/// The pit's ramps, each under a heading that removes it, and a button to
/// add one; while a new ramp's start is being picked, the banner over the
/// viewport says what to do, so nothing here repeats it.
/// `in_view` are the ramps whose gizmos stand in the view, which set what
/// they set there; the others', here.
fn draw_ramps(ui: &mut egui::Ui, settings: &mut PitDesignSettings, (picking, in_view): (Option<usize>, &[usize]), (base_rl, commands): (f64, &mut Vec<UiCommand>)) {
    let (sign, height) = (settings.direction.sign(), settings.bench_height);
    let several = settings.ramps.len() > 1;
    let mut remove = None;
    for (index, ramp) in settings.ramps.iter_mut().enumerate() {
        ui.push_id(("pit_design_ramp", index), |ui| {
            let heading = if several {
                tr!("pit-design-ramp-numbered", number = (index + 1).to_string())
            } else {
                tr!("pit-design-ramp-section")
            };
            if menu::menu_section_with_action(ui, heading, tr!("pit-design-ramp-remove-short"), tr!("pit-design-ramp-remove-help")) {
                remove = Some(index);
            }
            draw_ramp_settings(ui, ramp, (index, in_view.contains(&index)), (base_rl, sign, height));
        });
    }
    if let Some(index) = remove {
        settings.ramps.remove(index);
    }
    let count = settings.ramps.len();
    if picking.is_some() {
        return;
    }
    if count == 0 {
        menu::menu_section(ui, tr!("pit-design-ramp-section"));
        menu::menu_note(ui, tr!("pit-design-ramp-none"));
    }
    let (label, help) = if count == 0 {
        (tr!("pit-design-ramp-place"), tr!("pit-design-ramp-place-help"))
    } else {
        (tr!("pit-design-ramp-add"), tr!("pit-design-ramp-add-help"))
    };
    ui.add_space(2.0);
    let width = menu::natural_button_width(ui, &label, 80.0);
    if ui.add(MenuButton::new(label).min_width(width)).hint(help).clicked() {
        commands.push(UiCommand::BeginPitRampStartPick(count));
    }
}

/// Send the pit to Planning, or bring it up to date there: its ramps need
/// no sending, being always in the haul network.
fn draw_send_button(ui: &mut egui::Ui, working: &PitObject, id: ObjectId, ready: bool, commands: &mut Vec<UiCommand>) {
    let label = if working.sent.solid.is_none() {
        tr!("pit-design-send-planning")
    } else {
        tr!("pit-design-update-planning")
    };
    let topography = working.settings.topography.is_some();
    let width = menu::natural_button_width(ui, &label, 60.0);
    let response = ui.add(MenuButton::new(label).min_width(width).enabled(ready && topography));
    let response = if !ready {
        response.disabled_hint(tr!("pit-design-send-not-designed"))
    } else if !topography {
        response.disabled_hint(tr!("pit-design-send-no-topography"))
    } else {
        response.hint(tr!("pit-design-send-planning-help"))
    };
    if response.clicked() {
        commands.push(UiCommand::SendPitToPlanning(id));
    }
}

/// The pit's benches and where it stops. `base_rl` is the string's, where a
/// new zone starts.
/// What the walls' gizmo sets (`on_gizmo`, while it is in the view) is left
/// to it; without it, the fields are here.
fn draw_bench_settings(
    ui: &mut egui::Ui,
    settings: &mut PitDesignSettings,
    project: &UiProjectView,
    (base_rl, picking_region, on_gizmo): (f64, bool, bool),
    commands: &mut Vec<UiCommand>,
) {
    // What it is and the way it is built are pulled on the walls' gizmo: its
    // arms through the toe turn the walls the other way.
    let kind = settings.kind;
    // How far it goes, with what it is and which way it is built.
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
    // How many benches, or the RL, is set beside the walls' gizmo in the
    // view; the ground it daylights against, here.
    match settings.stop {
        StopMode::Benches | StopMode::Elevation if on_gizmo => {}
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

    // The bench's height, face angle and berm are set on the gizmo in the
    // view, which stands on the pit's wall.
    menu::menu_section(ui, tr!("pit-design-benches-section"));
    if !on_gizmo {
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
    }
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

/// The ramp's settings, for a pit that has one: where it starts, its
/// width and gradient, which way it climbs, and how it turns.
fn draw_ramp_settings(ui: &mut egui::Ui, ramp: &mut RampSettings, (index, on_gizmo): (usize, bool), (base_rl, sign, height): (f64, f64, f64)) {
    // Its width, grade and the way it climbs are set on its gizmo in the
    // view, at its foot; with its foot out of view, here.
    if !on_gizmo {
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
        let turn_label = |clockwise: bool| if clockwise { tr!("pit-design-clockwise") } else { tr!("pit-design-anticlockwise") };
        let chosen = turn_label(ramp.climbs_clockwise);
        MenuFieldCombo::new(
            ("pit_design_climbs", index),
            tr!("pit-design-climbs"),
            &mut ramp.climbs_clockwise,
            chosen,
            [false, true].map(|clockwise| (clockwise, turn_label(clockwise).into())),
        )
        .width(FIELD_WIDTH)
        .help_text(tr!("pit-design-climbs-help"))
        .show(ui);
    }
    // The tightest the road should turn: what a switchback turns round, and
    // what the whole road is checked against, either way.
    MenuFieldF64::new(tr!("pit-design-inside-radius"), &mut ramp.inside_radius, 0.0..=10000.0)
        .width(FIELD_WIDTH)
        .speed(0.5)
        .suffix(" m")
        .help_text(tr!("pit-design-inside-radius-help"))
        .show(ui);
    // Where it switches back is clicked on the ramp in the view; how its
    // turns are laid, here, once it has one.
    if ramp.switchback {
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
fn draw_result(ui: &mut egui::Ui, draft: &PitDesignDraft, settings: &PitDesignSettings) -> PitHover {
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
            // How it ended says the RL it ended at, but for every bench built.
            if matches!(design.end, End::Complete) {
                read_only_row(ui, &tr!("pit-design-final-rl"), &format!("{:.1} m", design.final_elevation()));
            }
            if ramp {
                read_only_row(ui, &tr!("pit-design-ramp-length"), &format!("{} m", whole(design.ramp_length)));
            }
            // One volume, always in view: as measured against the ground,
            // the last measure standing until the next comes (the rings
            // enclose the air above the ground too, so would jump about);
            // with nothing measured, what the rings enclose.
            let measured = draft
                .working
                .as_ref()
                .and_then(|working| working.evaluation.as_ref())
                .filter(|evaluation| evaluation.problem.is_none())
                .map(|evaluation| evaluation.total.volume);
            read_only_row(ui, &tr!("pit-design-volume"), &format!("{} m³", whole(measured.unwrap_or(design.volume))));
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
                let flatter = design.report.iter().filter(|bench| bench.ramps.iter().any(|road| road.lengthened > 0.0)).count();
                if flatter > 0 {
                    ui.colored_label(warning, tr!("pit-design-flatter", count = flatter));
                }
                let steep = design.walls.iter().filter(|wall| wall.too_steep()).count();
                if steep > 0 {
                    ui.colored_label(warning, tr!("pit-design-walls-too-steep", count = steep));
                }
            }
            // The checks: a quiet line where all pass, and where some do not,
            // a section that opens itself and says how many.
            if design.flags.is_empty() {
                ui.label(egui::RichText::new(tr!("pit-design-checks-pass-short")).color(ui.visuals().weak_text_color()))
                    .hint(tr!("pit-design-checks-pass"));
            } else {
                // Each is marked and said in the view, where it is.
                ui.colored_label(warning, tr!("pit-design-checks-in-view", count = design.flags.len()));
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
    hover
}

/// A spinner at the end of the section heading just drawn, which began at
/// `top`: work under way, said without taking room.
fn heading_spinner(ui: &mut egui::Ui, top: f32) {
    let bottom = ui.cursor().top();
    let size = 11.0;
    let rect = egui::Rect::from_center_size(egui::pos2(ui.max_rect().right() - size / 2.0, (top + bottom) / 2.0), egui::vec2(size, size));
    egui::Spinner::new().size(size).paint_at(ui, rect);
}

/// A whole quantity, grouped in thousands: a volume, a length.
fn whole(value: f64) -> String {
    use thousands::Separable;
    format!("{value:.0}").separate_with_commas()
}

/// What the design takes out of the ground, measured against the
/// topography and a block model the section chooses: in all, by material
/// and bench by bench.
fn draw_evaluation(ui: &mut egui::Ui, working: &mut PitObject, facts: &DraftFacts, project: &UiProjectView) {
    // Worked out again, the last figures stay with a spinner at the
    // heading's end, so nothing comes and goes to jog the panel about.
    let busy = matches!(facts.waiting, Some(Waiting::Design | Waiting::Measuring));
    let top = ui.cursor().top();
    let open = menu::menu_section_folding(ui, "pit_design_evaluation", tr!("pit-design-evaluation"));
    if busy {
        heading_spinner(ui, top);
    }
    if !open {
        return;
    }
    // Built up short of the ground it could daylight against, its ground
    // above its last ring is measured as though its walls stood straight up.
    let short = working.design().is_some_and(|design| working.settings.can_daylight() && design.end != End::Daylight);
    // A dump's top, where its rehabilitation's limit starts from.
    let design_top = working.design().map(|design| design.levels.iter().copied().fold(f64::NEG_INFINITY, f64::max));
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
    // A dump reshaped for its rehabilitation.
    if settings.kind == Kind::Dump {
        let mut on = settings.rehab.is_some();
        MenuFieldBool::new(tr!("pit-design-rehab"), &mut on).help_text(tr!("pit-design-rehab-help")).show(ui);
        if on != settings.rehab.is_some() {
            settings.rehab = on.then(RehabSettings::default);
        }
        if let Some(rehab) = settings.rehab.as_mut() {
            MenuFieldF64::new(tr!("pit-design-rehab-slope"), &mut rehab.slope, 1.0..=60.0)
                .width(FIELD_WIDTH)
                .speed(0.1)
                .suffix("°")
                .help_text(tr!("pit-design-rehab-slope-help"))
                .show(ui);
            let mut limited = rehab.top.is_some();
            MenuFieldBool::new(tr!("pit-design-rehab-limit"), &mut limited)
                .help_text(tr!("pit-design-rehab-limit-help"))
                .show(ui);
            if limited != rehab.top.is_some() {
                rehab.top = limited.then(|| design_top.filter(|top| top.is_finite()).unwrap_or_default());
            }
            if let Some(top) = rehab.top.as_mut() {
                MenuFieldF64::new(tr!("pit-design-rehab-top"), top, f64::MIN..=f64::MAX)
                    .width(FIELD_WIDTH)
                    .speed(0.5)
                    .suffix(" m")
                    .show(ui);
            }
            MenuFieldF64::new(tr!("pit-design-rehab-contour"), &mut rehab.contour, 0.5..=100.0)
                .width(FIELD_WIDTH)
                .speed(0.1)
                .suffix(" m")
                .help_text(tr!("pit-design-rehab-contour-help"))
                .show(ui);
        }
    }
    let warning = ui.visuals().warn_fg_color;
    let first = working.evaluation.is_none();
    match &facts.waiting {
        Some(Waiting::Design | Waiting::Measuring) if !first => {}
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
    if let Some(rehab) = &evaluation.rehab {
        draw_rehab(ui, rehab);
    }
    draw_comparison(ui, facts, working.design().is_some());
}

/// A dump reshaped for its rehabilitation: where its top stands, what it
/// holds, the earthworks it takes, and how far it spreads.
fn draw_rehab(ui: &mut egui::Ui, rehab: &RehabFigures) {
    menu::menu_section(ui, tr!("pit-design-rehab-figures"));
    if let Some(problem) = &rehab.problem {
        ui.colored_label(ui.visuals().warn_fg_color, tr!("pit-design-rehab-failed", reason = problem.clone()));
        return;
    }
    read_only_row(ui, &tr!("pit-design-rehab-top"), &format!("{:.1} m", rehab.top));
    read_only_row(ui, &tr!("pit-design-evaluation-volume"), &format!("{} m³", whole(rehab.volume)));
    read_only_row(ui, &tr!("pit-design-rehab-cut"), &format!("{} m³", whole(rehab.cut)));
    read_only_row(ui, &tr!("pit-design-rehab-fill"), &format!("{} m³", whole(rehab.fill)));
    let [before, after] = rehab.footprint;
    read_only_row(ui, &tr!("pit-design-rehab-footprint"), &format!("{} → {} m²", whole(before), whole(after)));
}

/// The numeric reserve fields: those a figure is given for.
fn numeric_fields(fields: &[ReserveField]) -> Vec<&ReserveField> {
    fields.iter().filter(|field| field.aggregation != ReserveAggregation::Category).collect()
}

/// A reserve field's figure as it reads best: a sum whole and grouped in
/// thousands, an average to two places.
fn field_figure(field: &ReserveField, value: Option<f64>) -> String {
    match (field.aggregation, value) {
        (_, None) => "-".to_owned(),
        (ReserveAggregation::Sum, Some(value)) => whole(value),
        (_, Some(value)) => format!("{value:.2}"),
    }
}

/// The heading over a column of volumes, with its unit.
fn volume_heading() -> String {
    format!("{} (m³)", tr!("pit-design-evaluation-volume"))
}

/// How a row of a figures table stands out from the rest.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RowStyle {
    Plain,
    /// The one the panel is open on.
    Strong,
    /// Out of date.
    Dim,
}

/// One row of a figures table: its name, and its figure under each column.
struct FigureRow {
    name: String,
    figures: Vec<String>,
    style: RowStyle,
    hint: Option<String>,
}

impl FigureRow {
    fn new(name: impl Into<String>, figures: Vec<String>) -> Self {
        Self {
            name: name.into(),
            figures,
            style: RowStyle::Plain,
            hint: None,
        }
    }
}

/// A table of figures: names down the left, each column of figures set right
/// under its heading, as wide as its widest entry, every other row shaded.
/// Scrolls sideways where it is wider than the panel.
fn figures_table(ui: &mut egui::Ui, id: impl std::hash::Hash + std::fmt::Debug, name_heading: String, headings: &[String], rows: &[FigureRow]) {
    const GAP: f32 = 14.0;
    const ROW: f32 = 20.0;
    let visuals = ui.visuals().clone();
    let (weak, text, strong) = (visuals.weak_text_color(), visuals.text_color(), visuals.strong_text_color());
    let small = egui::TextStyle::Small.resolve(ui.style());
    // Measured as the labels lay them out, in the panel's own text style, so
    // each column is exactly as wide as what is in it.
    let width = |text: egui::RichText| {
        egui::WidgetText::from(text)
            .into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Body)
            .size()
            .x
    };
    let heading_width = |text: &str| width(egui::RichText::new(text).font(small.clone()));
    let body_width = |text: &str| width(egui::RichText::new(text).strong());
    let name_width = rows.iter().map(|row| body_width(&row.name)).fold(heading_width(&name_heading), f32::max);
    let widths: Vec<f32> = (0..headings.len())
        .map(|column| {
            rows.iter()
                .filter_map(|row| row.figures.get(column))
                .map(|figure| body_width(figure))
                .fold(heading_width(&headings[column]), f32::max)
        })
        .collect();
    let total = name_width + widths.iter().map(|width| width + GAP).sum::<f32>() + 8.0;
    egui::ScrollArea::horizontal().id_salt(("pit_design_table", id)).show(ui, |ui| {
        let cell = |ui: &mut egui::Ui, width: f32, right: bool, text: egui::RichText| {
            let layout = if right {
                egui::Layout::right_to_left(egui::Align::Center)
            } else {
                egui::Layout::left_to_right(egui::Align::Center)
            };
            ui.allocate_ui_with_layout(egui::vec2(width, ROW), layout, |ui| {
                // As wide as its column, whatever is in it.
                ui.set_min_width(width);
                ui.label(text)
            })
            .inner
        };
        // Headings, small and weak, over their columns.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            cell(ui, name_width + 4.0, false, egui::RichText::new(name_heading.clone()).font(small.clone()).color(weak));
            for (heading, width) in headings.iter().zip(&widths) {
                cell(ui, *width, true, egui::RichText::new(heading.clone()).font(small.clone()).color(weak));
            }
        });
        for (index, row) in rows.iter().enumerate() {
            let top = ui.cursor().min.y;
            if index % 2 == 1 {
                let band = egui::Rect::from_min_size(egui::pos2(ui.cursor().min.x, top), egui::vec2(total, ROW));
                ui.painter().rect_filled(band, 2.0, visuals.faint_bg_color);
            }
            let color = match row.style {
                RowStyle::Plain => text,
                RowStyle::Strong => strong,
                RowStyle::Dim => weak,
            };
            let styled = |text: &str| {
                let text = egui::RichText::new(text).color(color);
                if row.style == RowStyle::Strong { text.strong() } else { text }
            };
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GAP;
                let name = cell(ui, name_width + 4.0, false, styled(&row.name));
                if let Some(hint) = &row.hint {
                    name.hint(hint.clone());
                }
                for (figure, width) in row.figures.iter().zip(&widths) {
                    cell(ui, *width, true, styled(figure));
                }
            });
        }
    });
}

/// What the design takes beyond the shell and leaves inside it, and the
/// difference, where a difference means something: of the volume and the
/// summed fields, not of an average.
fn draw_against_shell(ui: &mut egui::Ui, shell: &ShellFigures, fields: &[ReserveField]) {
    menu::menu_section(ui, tr!("pit-design-shell-against"));
    if let Some(problem) = &shell.problem {
        ui.colored_label(ui.visuals().warn_fg_color, tr!("pit-design-shell-failed", reason = problem.clone()));
        return;
    }
    let numeric = numeric_fields(fields);
    // Where there is no ground at all, a summed figure is none of it.
    let value = |figures: &Figures, field: &ReserveField| {
        figures
            .measured
            .as_ref()
            .and_then(|measured| measured.value(field.id))
            .or_else(|| (figures.volume == 0.0 && field.aggregation == ReserveAggregation::Sum).then_some(0.0))
    };
    let mut headings = vec![volume_heading()];
    headings.extend(numeric.iter().map(|field| field.name.clone()));
    let row = |label: String, figures: &Figures| {
        let mut cells = vec![whole(figures.volume)];
        cells.extend(numeric.iter().map(|field| field_figure(field, value(figures, field))));
        FigureRow::new(label, cells)
    };
    let mut net = vec![whole(shell.gained.volume - shell.left.volume)];
    net.extend(numeric.iter().map(|field| {
        match field.aggregation {
            ReserveAggregation::Sum => value(&shell.gained, field)
                .zip(value(&shell.left, field))
                .map_or_else(String::new, |(gained, left)| whole(gained - left)),
            _ => String::new(),
        }
    }));
    let rows = [
        row(tr!("pit-design-shell-gained"), &shell.gained),
        row(tr!("pit-design-shell-left"), &shell.left),
        FigureRow::new(tr!("pit-design-shell-net"), net),
    ];
    figures_table(ui, "against_shell", String::new(), &headings, &rows);
}

/// Every pit design in the project measured, side by side: this one picked
/// out, any not measured as it stands now dimmed.
fn draw_comparison(ui: &mut egui::Ui, facts: &DraftFacts, designed: bool) {
    let measured: Vec<&ComparedDesign> = facts.designs.iter().filter(|design| design.total.is_some()).collect();
    if measured.len() < 2 || !designed {
        return;
    }
    menu::menu_section(ui, tr!("pit-design-compare"));
    let numeric = numeric_fields(&facts.fields);
    let shells = measured.iter().any(|design| design.shell.as_ref().is_some_and(|shell| shell.problem.is_none()));
    let mut headings = vec![volume_heading()];
    headings.extend(numeric.iter().map(|field| field.name.clone()));
    if shells {
        headings.extend([tr!("pit-design-shell-gained"), tr!("pit-design-shell-left")]);
    }
    let rows: Vec<FigureRow> = measured
        .iter()
        .map(|design| {
            let total = design.total.as_ref();
            let mut cells = vec![total.map_or_else(|| "-".to_owned(), |total| whole(total.volume))];
            cells.extend(
                numeric
                    .iter()
                    .map(|field| field_figure(field, total.and_then(|total| total.measured.as_ref()).and_then(|measured| measured.value(field.id)))),
            );
            if shells {
                let shell = design.shell.as_ref().filter(|shell| shell.problem.is_none());
                cells.push(shell.map_or_else(String::new, |shell| whole(shell.gained.volume)));
                cells.push(shell.map_or_else(String::new, |shell| whole(shell.left.volume)));
            }
            let this = facts.designs.first().is_some_and(|first| first.id == design.id);
            FigureRow {
                name: design.name.clone(),
                figures: cells,
                style: match (this, design.current) {
                    (true, _) => RowStyle::Strong,
                    (false, false) => RowStyle::Dim,
                    (false, true) => RowStyle::Plain,
                },
                hint: (!design.current).then(|| tr!("pit-design-compare-stale")),
            }
        })
        .collect();
    figures_table(ui, "compare", tr!("pit-design-name"), &headings, &rows);
}

/// A measure's figures: the whole design's, its materials' and its
/// benches'.
fn draw_figures(ui: &mut egui::Ui, evaluation: &Evaluation, fields: &[ReserveField]) {
    // Its volume is said with the design's figures, above.
    let total = &evaluation.total;
    let numeric = numeric_fields(fields);
    let Some(measured) = total.measured.as_ref() else {
        return;
    };
    // The block model short of the ground is worth saying only where it is.
    if measured.covered < total.volume * 0.999 {
        read_only_row(ui, &tr!("pit-design-evaluation-covered"), &format!("{} m³", whole(measured.covered)));
    }
    for field in &numeric {
        read_only_row(ui, &field.name, &field_figure(field, measured.value(field.id)));
    }
    let mut headings = vec![volume_heading()];
    headings.extend(numeric.iter().map(|field| field.name.clone()));
    // By material: each category field's categories.
    for category in &measured.categories {
        let Some(field) = fields.iter().find(|field| field.id == category.field) else {
            continue;
        };
        if category.groups.is_empty() {
            continue;
        }
        menu::menu_section(ui, tr!("pit-design-evaluation-by", field = field.name.clone()));
        let rows: Vec<FigureRow> = category
            .groups
            .iter()
            .map(|group| {
                let mut cells = vec![whole(group.covered)];
                cells.extend(numeric.iter().map(|numeric| field_figure(numeric, group.value(numeric.id))));
                FigureRow::new(group.name.clone(), cells)
            })
            .collect();
        figures_table(ui, ("materials", field.id.0), field.name.clone(), &headings, &rows);
    }
    // Bench by bench, top down, as a pit is mined.
    if evaluation.benches.is_empty() {
        return;
    }
    menu::menu_section(ui, tr!("pit-design-evaluation-benches"));
    let rows: Vec<FigureRow> = evaluation
        .benches
        .iter()
        .rev()
        .map(|bench| {
            let Figures { volume, measured } = &bench.figures;
            let mut cells = vec![whole(*volume)];
            cells.extend(
                numeric
                    .iter()
                    .map(|field| field_figure(field, measured.as_ref().and_then(|measured| measured.value(field.id)))),
            );
            FigureRow::new(format!("{:.1}–{:.1}", bench.base, bench.top), cells)
        })
        .collect();
    figures_table(ui, "benches", tr!("pit-design-table-rl"), &headings, &rows);
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
