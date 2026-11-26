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

/// What the pointer rests on in the panel's checks and bench table: a
/// bench, or one of the design's flags, for the viewport to pick out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PitHover {
    pub(crate) bench: Option<usize>,
    pub(crate) flag: Option<usize>,
}

/// A ramp's gizmo in the viewport, in physical window pixels, at its foot:
/// a ring standing on the road's line, which turns its grade (seen from
/// above it fades out), an arrow out across the road,
/// which widens it, and arrows along the string the way the road leaves its
/// foot and the other way, which a click turns it to.
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
    /// Where the ring stands at the road's grade.
    pub(crate) grade_handle: (f32, f32),
    /// An arrowhead along the string the way the road leaves its foot, and
    /// one the other way: each its back corner, tip and other back corner.
    pub(crate) climbs: [(f32, f32); 3],
    pub(crate) climbs_back: [(f32, f32); 3],
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

/// The walls' gizmo: the first two benches' section standing upright on the
/// pit's string at their true size, its corners on the design's rings: its
/// toe on the string, its face up to its crest, its berm out to where the
/// next face starts and that face up to the next crest, the arc of the
/// face's angle at the toe, and beside the face, the mark of its height. In
/// physical window pixels.
#[derive(Clone, Debug)]
pub(crate) struct WallGizmo {
    /// How much of the height's mark shows (none seen down it), of the
    /// angle's arc (none edge on), and of the handles that move out across
    /// the bench (none looking along it): none, and they cannot be taken.
    pub(crate) height_fade: f32,
    pub(crate) arc_fade: f32,
    pub(crate) out_fade: f32,
    /// The section's corners: the toe, on the string; the crest; the berm's
    /// end, the next toe; the next crest; and the floor behind the toe.
    pub(crate) toe: (f32, f32),
    pub(crate) crest: (f32, f32),
    pub(crate) berm_end: (f32, f32),
    pub(crate) next_top: (f32, f32),
    pub(crate) floor_back: (f32, f32),
    /// The height's mark beside the face, from the toe's level to the
    /// crest's, its top the handle.
    pub(crate) height_from: (f32, f32),
    pub(crate) height_tip: (f32, f32),
    /// The face's angle, an arc about the toe from out across to the face.
    pub(crate) angle_arc: Vec<(f32, f32)>,
    /// Up the section, out across it and along the string, units on screen,
    /// and the pixels a drawn metre takes up and out.
    pub(crate) up: (f32, f32),
    pub(crate) out: (f32, f32),
    pub(crate) along: (f32, f32),
    pub(crate) pixels_per_metre_up: f32,
    pub(crate) pixels_per_metre_out: f32,
}

impl WallGizmo {
    /// The room its corners have on screen, between the nearest two, in
    /// physical pixels: seen from far off, the true-size section is too
    /// small to take its handles apart.
    pub(crate) fn room(&self) -> f32 {
        let apart = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).hypot(a.1 - b.1);
        apart(self.toe, self.crest).min(apart(self.crest, self.berm_end))
    }

    /// Whether its handles on its corners can be told apart to take: with
    /// less room, only its lines and foot are drawn, and its card sets what
    /// it sets.
    pub(crate) fn handles_apart(&self, pixels_per_point: f32) -> bool {
        self.room() >= HANDLE_ROOM_POINTS * pixels_per_point
    }
}

/// The least room on screen between the walls' gizmo's corners for its
/// handles to be drawn and taken, in logical points.
const HANDLE_ROOM_POINTS: f32 = 9.0;

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
    /// The walls' gizmo's foot, which moves it along the string.
    WallFoot,
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
    /// Whether a turn was asked here that no bench from here on has room
    /// for: kept asked, drawn amber, said so under the pointer.
    pub(crate) refused: bool,
}

/// A ramp being added: where on the string it was pressed, its foot; where
/// on screen; and the way it climbs, as the pointer has been dragged along
/// the string since (as the last ramp does, or the usual way, until it is).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AddingRamp {
    pub(crate) foot: DVec3,
    pub(crate) pressed: (f32, f32),
    pub(crate) clockwise: bool,
    /// The way it leaves its foot along the string, in plan, a unit.
    pub(crate) way: glam::DVec2,
    /// Whether the pointer has been dragged far enough to say the way.
    pub(crate) dragged: bool,
}

/// A part of the walls' gizmo being dragged: which, where the press was,
/// its value then (and the bench's height, which a face turns about), and
/// the way it moves on screen and pixels a metre then, held for the drag:
/// pulled back through the toe, the walls turn to go the other way.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WallDrag {
    pub(crate) part: GizmoPart,
    pub(crate) from: (f32, f32),
    pub(crate) value: f64,
    pub(crate) height: f64,
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
    /// The walls' gizmo's foot is being dragged along the string.
    pub(crate) moving_wall: bool,
    /// The pointer is held down over the panel or the view: a design under
    /// way is let finish, so what is dragged shows as it goes.
    pub(crate) pointer_held: bool,
    /// Where each of the preview's flags stands on screen.
    pub(crate) flag_px: Vec<Option<(f32, f32)>>,
    /// Where on the string the ramp would start, under the cursor, while
    /// its start is being dragged.
    pub(crate) ramp_hover: Option<DVec3>,
    /// The place on the pit's string under the pointer, with nothing else
    /// there: where a press adds a ramp.
    pub(crate) string_hover: Option<DVec3>,
    /// The same on screen, and while a ramp is added, its arrow from the
    /// foot the way it climbs (the foot, the tip and the head's two back
    /// corners), in physical window pixels.
    pub(crate) string_hover_px: Option<(f32, f32)>,
    pub(crate) adding_arrow: Option<[(f32, f32); 4]>,
    /// A value box beside a handle under the pointer, which lights the
    /// handle it sets.
    pub(crate) field_hover: Option<GizmoValue>,
    /// A ramp being added: pressed on the string, it is dragged the way it
    /// climbs, and let go, made.
    pub(crate) adding_ramp: Option<AddingRamp>,
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
    /// The ramp whose start is being dragged along the string.
    pub(crate) picking_ramp: Option<usize>,
    /// Where the press that took a ramp's start up was: let go, the start
    /// is placed where the pointer is, unless it was never moved.
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

    /// The ramp, where on the string the preview starts it and, for one
    /// being added, the way it climbs: while a start is dragged, where the
    /// pointer would put it; while a ramp is added, where it was pressed,
    /// one past the last.
    pub(crate) fn ramp_previewed(&self) -> Option<(usize, DVec3, Option<bool>)> {
        if let Some(adding) = self.adding_ramp {
            let next = self.working.as_ref().map_or(0, |working| working.settings.ramps.len());
            return Some((next, adding.foot, Some(adding.clockwise)));
        }
        self.picking_ramp.zip(self.ramp_hover).map(|(index, at)| (index, at, None))
    }

    /// Whether a ramp's start is being moved or a ramp added: the handles
    /// stand down until it is let go.
    pub(crate) fn placing_ramp(&self) -> bool {
        self.picking_ramp.is_some() || self.adding_ramp.is_some()
    }

    /// Whether a handle is picked out: under the pointer, its field in a
    /// card under the pointer, or dragged.
    pub(crate) fn lights(&self, part: GizmoPart) -> bool {
        self.gizmo_hover == Some(part)
            || self.field_hover.map(GizmoValue::part) == Some(part)
            || match part {
                GizmoPart::Width(index) => self.widening.is_some_and(|(dragged, ..)| dragged == index),
                GizmoPart::Grade(index) => self.grading.is_some_and(|(dragged, ..)| dragged == index),
                GizmoPart::WallFoot => self.moving_wall,
                GizmoPart::BenchHeight | GizmoPart::FaceAngle | GizmoPart::Berm => self.wall_drag.is_some_and(|drag| drag.part == part),
                GizmoPart::Turn(_) | GizmoPart::Switch(_) => false,
            }
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
    if draft.object_id().is_none() {
        return;
    }
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
            // The settings and what they come to scroll between the heading
            // and the actions, which stay in view.
            egui::ScrollArea::vertical()
                .id_salt("pit_design_body")
                .max_height(body_height(viewport_rect))
                .auto_shrink([false, true])
                .show(ui, |ui| {
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
                    // The walls, then the ramps: each set where it stands in
                    // the view, the rest here.
                    let base_rl = draft.built_for.as_ref().and_then(|key| key.verts.first()).map_or(0.0, |vertex| vertex.pos.z);
                    let built = draft.preview.as_ref().and_then(|preview| preview.design.as_ref().ok()).map(|design| design.benches);
                    draw_bench_settings(ui, &mut working.settings, project, (base_rl, picking_region, on_gizmo, built), commands);
                    draw_ramps(ui, &mut working.settings, &in_view, base_rl);
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

            menu::menu_actions(ui, |ui| {
                if ui.add(MenuButton::new(tr!("pit-design-done")).primary()).hint(tr!("pit-design-done-help")).clicked() {
                    commands.push(UiCommand::FinishPitDesign);
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
    // A turn asked where none fits is the one handle with something to say.
    let refused = match draft.gizmo_hover {
        Some(GizmoPart::Switch(index)) => draft.switch_spots.get(index).is_some_and(|spot| spot.refused),
        _ => false,
    };
    // A gizmo's handles say what they do by their shape: nothing else is
    // said over them.
    let over_viewport = over_viewport && (draft.gizmo_hover.is_none() || refused);
    if refused && over_viewport {
        crate::ui::widgets::hint::hint_at_pointer(ui.ctx(), egui::Id::unique("pit_design_switch_refused_hint"), tr!("pit-design-switch-refused"));
    } else if draft.ramp_marker_under_cursor.is_some() && over_viewport {
        crate::ui::widgets::hint::hint_at_pointer(ui.ctx(), egui::Id::unique("pit_design_ramp_drag_hint"), tr!("pit-design-ramp-drag-hint"));
    } else if draft.string_hover.is_some() && draft.adding_ramp.is_none() && over_viewport {
        crate::ui::widgets::hint::hint_at_pointer(ui.ctx(), egui::Id::unique("pit_design_ramp_add_hint"), tr!("pit-design-ramp-add-hint"));
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
    draft.pointer_held = ui.ctx().input(|input| input.pointer.any_down());
    draft.settled = !draft.pointer_held && ui.ctx().memory(|memory| memory.focused().is_none());
}

/// In the viewport, over the design: each ramp's gizmo at its foot, in the
/// manner of the Move and Rotate gizmos (its grade ring standing on the
/// road's line with the grade swept out of it and a handle at the grade,
/// arrows along the string the way it leaves its foot and, hollow, the other
/// way, which a click turns it to, and its width arrow out over the road);
/// the walls' gizmo, the first bench's section standing on the string; and
/// beside each, a card of what it sets, to drag or type. Where a press on the
/// string would add a ramp, and the way one being added goes. And beside each
/// place the design falls short, what falls short, which a click goes to.
fn draw_viewport_handles(ui: &mut egui::Ui, draft: &mut PitDesignDraft, commands: &mut Vec<UiCommand>, viewport_rect: egui::Rect) {
    use crate::ui::{GIZMO_HEAD_HALF_WIDTH, GIZMO_HEAD_LENGTH, GIZMO_IDLE_ALPHA, gizmo_color};
    let per_point = ui.ctx().pixels_per_point();
    let at = |px: (f32, f32)| egui::pos2(px.0 / per_point, px.1 / per_point);
    let painter = ui.painter().with_clip_rect(viewport_rect);
    let grade_colour = GizmoValue::Gradient(0).colour();
    let ramp_colour = GizmoValue::Width(0).colour();
    let mut cards: Vec<GizmoCard> = Vec::new();
    // Where each ramp can switch back: a ring where it could, filled where
    // it does; a click turns it there or stops it. Under the pointer, the
    // turn it would make shows in it.
    for (index, spot) in draft.switch_spots.iter().enumerate() {
        let centre = at(spot.at);
        if !viewport_rect.contains(centre) {
            continue;
        }
        let lit = draft.gizmo_hover == Some(GizmoPart::Switch(index));
        // In the ramp's own colour: amber is what falls short.
        let colour = if lit { egui::Color32::WHITE } else { ramp_colour };
        let radius = if lit { 7.5 } else { 5.5 };
        if spot.refused {
            let amber = ui.visuals().warn_fg_color;
            painter.circle_filled(centre, radius, gizmo_color(egui::Color32::BLACK, if lit { 0.5 } else { 0.3 }));
            painter.circle_stroke(centre, radius, egui::Stroke::new(2.0, gizmo_color(amber, if lit { 1.0 } else { 0.9 })));
        } else if spot.turn {
            painter.circle_filled(centre, radius, gizmo_color(colour, if lit { 1.0 } else { 0.9 }));
            painter.circle_stroke(centre, radius, egui::Stroke::new(1.5, gizmo_color(egui::Color32::BLACK, 0.5)));
        } else {
            painter.circle_filled(centre, radius, gizmo_color(egui::Color32::BLACK, if lit { 0.5 } else { 0.3 }));
            painter.circle_stroke(centre, radius, egui::Stroke::new(1.5, gizmo_color(colour, if lit { 1.0 } else { GIZMO_IDLE_ALPHA })));
        }
        if lit {
            // A hairpin: what a click makes here, or takes away.
            let ink = if spot.turn { egui::Color32::BLACK } else { egui::Color32::WHITE };
            hairpin(&painter, centre, radius * 0.55, gizmo_color(ink, 0.9));
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
        let active = |part: GizmoPart| draft.lights(part);
        // The grade ring: its half above the road's line, the grade swept
        // out of it, and its handle; faded out edge on, as a Blender rotate
        // ring.
        let lit = active(GizmoPart::Grade(gizmo.ramp));
        let fade = gizmo.ring_fade;
        let (colour, alpha) = if lit { (egui::Color32::WHITE, fade) } else { (grade_colour, GIZMO_IDLE_ALPHA * fade) };
        if fade > 0.0 {
            let handle = at(gizmo.grade_handle);
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
            painter.add(egui::Shape::line(half, egui::Stroke::new(if lit { 2.5 } else { 1.5 }, gizmo_color(colour, alpha * 0.8))));
            painter.line_segment([centre, handle], egui::Stroke::new(2.0, gizmo_color(colour, alpha)));
            handle_ball(&painter, handle, if lit { egui::Color32::WHITE } else { grade_colour }, lit, fade);
        }
        // The way it leaves its foot, an arrow out of it, and the other way,
        // a hollow head a click turns it to.
        let head = |corners: [(f32, f32); 3]| corners.map(at).to_vec();
        let climbs = head(gizmo.climbs);
        let neck = egui::pos2((climbs[0].x + climbs[2].x) * 0.5, (climbs[0].y + climbs[2].y) * 0.5);
        let start = neck + (centre - neck) * 0.55;
        painter.line_segment([start, neck], egui::Stroke::new(3.5, gizmo_color(egui::Color32::BLACK, 0.4)));
        painter.line_segment([start, neck], egui::Stroke::new(2.0, gizmo_color(ramp_colour, 0.95)));
        painter.add(egui::Shape::convex_polygon(
            climbs,
            gizmo_color(ramp_colour, 0.95),
            egui::Stroke::new(1.0, gizmo_color(egui::Color32::BLACK, 0.45)),
        ));
        let back = head(gizmo.climbs_back);
        if active(GizmoPart::Turn(gizmo.ramp)) {
            painter.add(egui::Shape::convex_polygon(back, gizmo_color(egui::Color32::WHITE, 0.9), egui::Stroke::NONE));
        } else {
            painter.add(egui::Shape::convex_polygon(
                back,
                gizmo_color(egui::Color32::BLACK, 0.2),
                egui::Stroke::new(1.5, gizmo_color(ramp_colour, GIZMO_IDLE_ALPHA)),
            ));
        }
        // The width arrow: faded out pointing at the camera, as a Blender
        // axis arrow.
        let lit = active(GizmoPart::Width(gizmo.ramp));
        let fade = gizmo.width_fade;
        let (colour, alpha) = if lit { (egui::Color32::WHITE, fade) } else { (ramp_colour, GIZMO_IDLE_ALPHA * fade) };
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
        }
        // Where it turns about.
        painter.circle_filled(centre, 3.0, gizmo_color(egui::Color32::WHITE, GIZMO_IDLE_ALPHA));
        // Its card beyond its width arrow, out over the road and clear of
        // the string a ramp is added on; below the foot as the road's way
        // turns to point at the camera.
        let over = egui::vec2(gizmo.across.0, gizmo.across.1) * fade;
        let away = (egui::vec2(0.0, 1.0) * (1.0 - fade) + over).normalized();
        cards.push(GizmoCard {
            kind: CardKind::Ramp(gizmo.ramp),
            foot: centre,
            anchor: centre + (tip - centre) * fade,
            away,
        });
    }
    if let Some(card) = draw_wall_gizmo(ui, draft, viewport_rect) {
        cards.push(card);
    }
    // Where a press on the string adds a ramp: a plus on the string.
    let over_view = ui.ctx().pointer_hover_pos().is_some_and(|pointer| viewport_rect.contains(pointer)) && !ui.ctx().egui_wants_pointer_input();
    if let Some(px) = draft.string_hover_px.filter(|_| draft.adding_ramp.is_none() && over_view) {
        let centre = at(px);
        painter.circle_filled(centre, 8.0, gizmo_color(egui::Color32::BLACK, 0.55));
        painter.circle_stroke(centre, 8.0, egui::Stroke::new(1.5, gizmo_color(ramp_colour, 1.0)));
        let ink = egui::Stroke::new(2.0, egui::Color32::WHITE);
        painter.line_segment([centre - egui::vec2(4.0, 0.0), centre + egui::vec2(4.0, 0.0)], ink);
        painter.line_segment([centre - egui::vec2(0.0, 4.0), centre + egui::vec2(0.0, 4.0)], ink);
    }
    // A ramp being added: from its foot, the way it goes.
    if let Some([foot, tip, left, right]) = draft.adding_arrow.map(|arrow| arrow.map(at)) {
        let neck = (left + right.to_vec2()) * 0.5;
        painter.line_segment([foot, neck], egui::Stroke::new(4.0, gizmo_color(egui::Color32::BLACK, 0.5)));
        painter.line_segment([foot, neck], egui::Stroke::new(2.5, gizmo_color(ramp_colour, 1.0)));
        painter.add(egui::Shape::convex_polygon(
            vec![tip, left, right],
            gizmo_color(ramp_colour, 1.0),
            egui::Stroke::new(1.0, gizmo_color(egui::Color32::BLACK, 0.5)),
        ));
    }
    gizmo_cards(ui, draft, &cards, viewport_rect);
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

/// A handle a press takes up: a ball with a dark rim, larger lit.
fn handle_ball(painter: &egui::Painter, centre: egui::Pos2, colour: egui::Color32, lit: bool, fade: f32) {
    sized_ball(painter, centre, colour, (lit, fade), 5.5);
}

/// A handle ball `radius` across at rest, a little larger lit.
fn sized_ball(painter: &egui::Painter, centre: egui::Pos2, colour: egui::Color32, (lit, fade): (bool, f32), radius: f32) {
    use crate::ui::gizmo_color;
    let radius = if lit { radius * 1.27 } else { radius };
    painter.circle_filled(centre, radius + 1.5, gizmo_color(egui::Color32::BLACK, 0.45 * fade));
    painter.circle_filled(centre, radius, gizmo_color(colour, fade));
}

/// A hairpin drawn in a switchback spot: a road up, round and back down,
/// `size` from the middle to its legs.
fn hairpin(painter: &egui::Painter, centre: egui::Pos2, size: f32, colour: egui::Color32) {
    let steps = 10;
    let mut points = vec![centre + egui::vec2(-size, size)];
    for step in 0..=steps {
        let turn = std::f32::consts::PI * step as f32 / steps as f32;
        points.push(centre + egui::vec2(-size * turn.cos(), -size * 0.2 - size * 0.8 * turn.sin()));
    }
    points.push(centre + egui::vec2(size, size));
    painter.add(egui::Shape::line(points, egui::Stroke::new(1.6, colour)));
}

/// The walls' gizmo: the first bench's section standing on the string, its
/// lines only drawn, its handles balls on its corners (the crest, pushed in
/// or out across the bench, turns the face; the berm's end, pulled, widens
/// the berm), an arrow on the mark of its height beside the face, and a
/// diamond at its foot that moves it along the string. Its card, of what it
/// sets, to go on the foot's other side from the bench.
fn draw_wall_gizmo(ui: &mut egui::Ui, draft: &PitDesignDraft, viewport_rect: egui::Rect) -> Option<GizmoCard> {
    use crate::ui::{GIZMO_HEAD_HALF_WIDTH, GIZMO_HEAD_LENGTH, GIZMO_IDLE_ALPHA, gizmo_color};
    let gizmo = draft.wall_gizmo.as_ref()?;
    let per_point = ui.ctx().pixels_per_point();
    let at = |px: (f32, f32)| egui::pos2(px.0 / per_point, px.1 / per_point);
    let toe = at(gizmo.toe);
    if !viewport_rect.contains(toe) {
        return None;
    }
    let painter = ui.painter().with_clip_rect(viewport_rect);
    let (crest, berm_end) = (at(gizmo.crest), at(gizmo.berm_end));
    let outline = |points: Vec<egui::Pos2>, width: f32, colour: egui::Color32| {
        painter.add(egui::Shape::line(points.clone(), egui::Stroke::new(width + 2.0, gizmo_color(egui::Color32::BLACK, 0.35))));
        painter.add(egui::Shape::line(points, egui::Stroke::new(width, colour)));
    };
    // The face's angle: a sector about the toe, from out across to the face.
    let lit = draft.lights(GizmoPart::FaceAngle);
    let angle_colour = GizmoValue::FaceAngle.colour();
    let fade = gizmo.arc_fade;
    if fade > 0.0 {
        let arc: Vec<egui::Pos2> = gizmo.angle_arc.iter().copied().map(at).collect();
        let mut sector = vec![toe];
        sector.extend(arc.iter().copied());
        painter.add(egui::Shape::convex_polygon(
            sector,
            gizmo_color(angle_colour, if lit { 0.4 } else { 0.22 } * fade),
            egui::Stroke::NONE,
        ));
        painter.add(egui::Shape::line(arc, egui::Stroke::new(1.5, gizmo_color(angle_colour, fade))));
    }
    // The section itself, over the design's own rings: the floor behind the
    // toe, faint; the faces up to the next crest; and the berm in its colour.
    let floor = [at(gizmo.floor_back), toe];
    painter.line_segment(floor, egui::Stroke::new(3.0, gizmo_color(egui::Color32::BLACK, 0.18)));
    painter.line_segment(floor, egui::Stroke::new(1.5, gizmo_color(egui::Color32::WHITE, 0.55)));
    outline(vec![berm_end, at(gizmo.next_top)], 2.0, gizmo_color(egui::Color32::WHITE, 0.7));
    outline(vec![toe, crest], 2.5, gizmo_color(egui::Color32::WHITE, 0.9));
    let berm_lit = draft.lights(GizmoPart::Berm);
    let berm_colour = GizmoValue::Berm.colour();
    outline(
        vec![crest, berm_end],
        if berm_lit { 3.0 } else { 2.5 },
        if berm_lit { egui::Color32::WHITE } else { gizmo_color(berm_colour, 0.95) },
    );
    // The height's mark beside the face: a tick at the toe's level, an arrow
    // at the crest's, which a press takes up.
    let height_lit = draft.lights(GizmoPart::BenchHeight);
    let fade = gizmo.height_fade;
    let (from, tip) = (at(gizmo.height_from), at(gizmo.height_tip));
    if fade > 0.0 && (tip - from).length() > GIZMO_HEAD_LENGTH {
        let colour = if height_lit {
            egui::Color32::WHITE
        } else {
            gizmo_color(GizmoValue::BenchHeight.colour(), (GIZMO_IDLE_ALPHA + 0.3) * fade)
        };
        let way = (tip - from).normalized();
        let side = egui::vec2(-way.y, way.x);
        painter.line_segment([from - side * 4.0, from + side * 4.0], egui::Stroke::new(1.5, colour));
        let head = tip - way * GIZMO_HEAD_LENGTH;
        painter.line_segment([from, head], egui::Stroke::new(if height_lit { 2.5 } else { 1.5 }, colour));
        let wing = side * GIZMO_HEAD_HALF_WIDTH * if height_lit { 1.2 } else { 1.0 };
        painter.add(egui::Shape::convex_polygon(vec![tip, head + wing, head - wing], colour, egui::Stroke::NONE));
        // Faint rules from the mark across to the toe and the crest.
        let rule = egui::Stroke::new(1.0, gizmo_color(GizmoValue::BenchHeight.colour(), 0.35 * fade));
        painter.line_segment([from, toe], rule);
        painter.line_segment([tip, crest], rule);
    }
    // The handles on the corners, which move out across the bench: as
    // large as the room between them allows, and none where there is too
    // little to tell them apart.
    let fade = gizmo.out_fade;
    if fade > 0.0 && gizmo.handles_apart(per_point) {
        let radius = (gizmo.room() / per_point * 0.35).clamp(3.0, 5.5);
        sized_ball(&painter, crest, if lit { egui::Color32::WHITE } else { angle_colour }, (lit, fade), radius);
        sized_ball(&painter, berm_end, if berm_lit { egui::Color32::WHITE } else { berm_colour }, (berm_lit, fade), radius);
    }
    // Its foot, a diamond on the string, dragged along it.
    let foot_lit = draft.lights(GizmoPart::WallFoot);
    let along = egui::vec2(gizmo.along.0, gizmo.along.1);
    let across = egui::vec2(-along.y, along.x);
    let size = if foot_lit { 7.0 } else { 5.5 };
    let diamond = vec![toe + along * size, toe + across * size, toe - along * size, toe - across * size];
    painter.add(egui::Shape::convex_polygon(
        diamond,
        if foot_lit {
            egui::Color32::WHITE
        } else {
            gizmo_color(egui::Color32::WHITE, GIZMO_IDLE_ALPHA + 0.2)
        },
        egui::Stroke::new(1.5, gizmo_color(egui::Color32::BLACK, 0.55)),
    ));
    if foot_lit || draft.moving_wall {
        // Which ways it goes: along the string either way.
        for way in [along, -along] {
            let tip = toe + way * (size + 9.0);
            let back = toe + way * (size + 4.0);
            painter.add(egui::Shape::convex_polygon(
                vec![tip, back + across * 3.5, back - across * 3.5],
                gizmo_color(egui::Color32::WHITE, 0.9),
                egui::Stroke::NONE,
            ));
        }
    }
    // Its card beyond the section, out the way the bench goes: clear of the
    // string inside, where ramps are added.
    let body = (crest - toe) + (berm_end - toe);
    let away = if body.length() > 1.0 { body.normalized() } else { egui::vec2(0.0, 1.0) };
    let reach = [crest, berm_end, at(gizmo.next_top)].into_iter().map(|corner| (corner - toe).dot(away)).fold(0.0, f32::max);
    Some(GizmoCard {
        kind: CardKind::Walls,
        foot: toe,
        anchor: toe + away * reach,
        away,
    })
}

/// How far the design goes, in a row: its benches, built until it meets
/// the ground where it can (the count it came to, which cut short, it stops
/// at), and where cut short with ground to meet, a button back to meeting
/// it. One stopped at an RL, the RL. `labelled` names the count, beside the
/// toe; in the panel the row does.
fn extent_controls(ui: &mut egui::Ui, settings: &mut PitDesignSettings, built: Option<u32>, labelled: bool) {
    if settings.stop == StopMode::Elevation {
        if labelled {
            ui.label(tr!("pit-design-stop-rl"));
        }
        ui.add_sized([72.0, 22.0], egui::DragValue::new(&mut settings.stop_elevation).speed(0.5).fixed_decimals(1));
        ui.label(tr!("common-m"));
        return;
    }
    if labelled {
        ui.label(tr!("pit-design-benches"));
    }
    let daylighting = settings.daylights();
    let mut count = if daylighting { built.unwrap_or(settings.benches) } else { settings.benches };
    if ui.add_sized([48.0, 22.0], egui::DragValue::new(&mut count).range(1..=u32::MAX).speed(0.05)).changed() {
        settings.stop = StopMode::Benches;
        settings.benches = count;
    }
    let meets = stop_label(settings.kind, StopMode::Daylight);
    if daylighting {
        ui.label(egui::RichText::new(meets).color(ui.visuals().weak_text_color()));
    } else if settings.can_daylight() && settings.topography.is_some() && ui.add(MenuButton::new(meets)).clicked() {
        settings.stop = StopMode::Daylight;
    }
}

/// A value a gizmo's handle sets: a ramp's, by its place in the list, or
/// the walls'.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum GizmoValue {
    Gradient(usize),
    Width(usize),
    BenchHeight,
    FaceAngle,
    Berm,
}

impl GizmoValue {
    /// The handle that sets it, which its field lights under the pointer.
    fn part(self) -> GizmoPart {
        match self {
            Self::Gradient(ramp) => GizmoPart::Grade(ramp),
            Self::Width(ramp) => GizmoPart::Width(ramp),
            Self::BenchHeight => GizmoPart::BenchHeight,
            Self::FaceAngle => GizmoPart::FaceAngle,
            Self::Berm => GizmoPart::Berm,
        }
    }

    /// What it is called in its card.
    fn name(self) -> String {
        match self {
            Self::Gradient(_) => tr!("pit-design-table-grade"),
            Self::Width(_) => tr!("pit-design-table-width"),
            Self::BenchHeight => tr!("pit-design-card-height"),
            Self::FaceAngle => tr!("pit-design-table-face"),
            Self::Berm => tr!("pit-design-table-berm"),
        }
    }

    /// The colour of its handle, which its field's mark shares.
    pub(crate) fn colour(self) -> egui::Color32 {
        use crate::ui::{GIZMO_AXIS_COLORS, ROTATE_RING_COLORS};
        match self {
            Self::Gradient(_) | Self::FaceAngle => ROTATE_RING_COLORS[1],
            Self::Width(_) => {
                let [r, g, b, _] = crate::model::pit_design::StringKind::RampEdge.colour();
                egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
            }
            Self::BenchHeight => GIZMO_AXIS_COLORS[2],
            Self::Berm => GIZMO_AXIS_COLORS[1],
        }
    }
}

/// Whose a card beside a gizmo is: the walls', or a ramp's by its place in
/// the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum CardKind {
    Walls,
    Ramp(usize),
}

/// A card of what a gizmo sets: whose, the gizmo's foot (which a faint line
/// ties it back to), the point beyond the gizmo it stands off, and the way
/// it lies off it, a unit on screen.
struct GizmoCard {
    kind: CardKind,
    foot: egui::Pos2,
    anchor: egui::Pos2,
    away: egui::Vec2,
}

/// How far off its gizmo's foot a card stands, and how far apart cards are
/// kept, in points.
const CARD_GAP: f32 = 26.0;
const CARD_SPACING: f32 = 6.0;

/// Each gizmo's card beside its foot: what it sets, each a coloured mark
/// like its handle's, a name, and a field to drag or click and type into.
/// A card slides round its foot as the view turns, never jumping, and is
/// pushed on out its way clear of those before it and inside what the panel
/// leaves of the view. A field under the pointer lights its handle, and a
/// handle under the pointer, its field.
fn gizmo_cards(ui: &mut egui::Ui, draft: &mut PitDesignDraft, cards: &[GizmoCard], viewport_rect: egui::Rect) {
    let built = draft.preview.as_ref().and_then(|preview| preview.design.as_ref().ok()).map(|design| design.benches);
    let ctx = ui.ctx().clone();
    let id = |kind: CardKind| egui::Id::unique(("pit_design_gizmo_card", kind));
    let per_point = ctx.pixels_per_point();
    let clear = draft.clear_view.map_or(viewport_rect, |[left, top, right, bottom]| {
        egui::Rect::from_min_max(egui::pos2(left / per_point, top / per_point), egui::pos2(right / per_point, bottom / per_point)).intersect(viewport_rect)
    });
    let mut placed: Vec<egui::Rect> = Vec::with_capacity(cards.len());
    for card in cards {
        let size = ctx.memory(|memory| memory.area_rect(id(card.kind))).map_or(egui::vec2(220.0, 32.0), |rect| rect.size());
        // Its middle out its way from the foot by the gap and as far as its
        // own half reaches that way: it moves smoothly as the way turns.
        let reach = card.away.x.abs() * size.x * 0.5 + card.away.y.abs() * size.y * 0.5;
        let mut rect = egui::Rect::from_center_size(card.anchor + card.away * (CARD_GAP + reach), size);
        for _ in 0..4 {
            let Some(push) = placed.iter().filter_map(|other| clearing(rect, other.expand(CARD_SPACING), card.away)).reduce(f32::max) else {
                break;
            };
            rect = rect.translate(card.away * push);
        }
        rect = rect.translate(egui::vec2(
            (clear.left() - rect.left()).max(0.0) - (rect.right() - clear.right()).max(0.0),
            (clear.top() - rect.top()).max(0.0) - (rect.bottom() - clear.bottom()).max(0.0),
        ));
        placed.push(rect);
    }
    let lit: Vec<bool> = cards
        .iter()
        .map(|card| match card.kind {
            CardKind::Walls => [GizmoPart::BenchHeight, GizmoPart::FaceAngle, GizmoPart::Berm].into_iter().any(|part| draft.lights(part)),
            CardKind::Ramp(ramp) => [GizmoPart::Grade(ramp), GizmoPart::Width(ramp)].into_iter().any(|part| draft.lights(part)),
        })
        .collect();
    let lights: Vec<GizmoValue> = [GizmoValue::BenchHeight, GizmoValue::FaceAngle, GizmoValue::Berm]
        .into_iter()
        .chain(
            cards
                .iter()
                .filter_map(|card| match card.kind {
                    CardKind::Ramp(ramp) => Some([GizmoValue::Gradient(ramp), GizmoValue::Width(ramp)]),
                    CardKind::Walls => None,
                })
                .flatten(),
        )
        .filter(|value| draft.lights(value.part()))
        .collect();
    // Each tied back to its gizmo's foot by a faint line, under it.
    let painter = ui.painter().with_clip_rect(viewport_rect);
    for (card, rect) in cards.iter().zip(&placed) {
        let near = rect.clamp(card.foot);
        if near.distance(card.foot) > CARD_GAP * 0.5 {
            let line = [card.foot, near];
            painter.line_segment(line, egui::Stroke::new(2.5, crate::ui::gizmo_color(egui::Color32::BLACK, 0.15)));
            painter.line_segment(line, egui::Stroke::new(1.0, crate::ui::gizmo_color(egui::Color32::WHITE, 0.5)));
        }
    }
    let Some(settings) = draft.working.as_mut().map(|working| &mut working.settings) else {
        return;
    };
    let mut hovered = None;
    for ((card, rect), card_lit) in cards.iter().zip(&placed).zip(&lit) {
        egui::Area::new(id(card.kind))
            .order(egui::Order::Middle)
            .fixed_pos(rect.min)
            .constrain_to(clear)
            .show(&ctx, |ui| {
                let state = if *card_lit {
                    crate::ui::ViewportInputState::Lit
                } else {
                    crate::ui::ViewportInputState::Idle
                };
                crate::ui::viewport_input_frame(ui, state).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(10.0, 4.0);
                    let values: &[GizmoValue] = match card.kind {
                        CardKind::Walls => &[GizmoValue::BenchHeight, GizmoValue::FaceAngle, GizmoValue::Berm],
                        CardKind::Ramp(ramp) => &[GizmoValue::Gradient(ramp), GizmoValue::Width(ramp)],
                    };
                    ui.horizontal(|ui| {
                        for &value in values {
                            if card_field(ui, settings, value, lights.contains(&value)) {
                                hovered = Some(value);
                            }
                        }
                    });
                    if card.kind == CardKind::Walls {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            extent_controls(ui, settings, built, true);
                        });
                    }
                });
            });
    }
    draft.field_hover = hovered;
}

/// One field of a card: the mark of its handle's colour, its name (strong
/// while its handle is lit), and its value to drag or type. Whether the
/// pointer is on it.
fn card_field(ui: &mut egui::Ui, settings: &mut PitDesignSettings, value: GizmoValue, lit: bool) -> bool {
    let (number, unit, range, speed): (&mut f64, String, std::ops::RangeInclusive<f64>, f64) = match value {
        GizmoValue::Gradient(ramp) => match settings.ramps.get_mut(ramp) {
            Some(ramp) => (&mut ramp.gradient, "%".to_owned(), 0.1..=100.0, 0.1),
            None => return false,
        },
        GizmoValue::Width(ramp) => match settings.ramps.get_mut(ramp) {
            Some(ramp) => (&mut ramp.width, tr!("common-m"), 0.1..=1000.0, 0.1),
            None => return false,
        },
        GizmoValue::BenchHeight => (&mut settings.bench_height, tr!("common-m"), 0.1..=1000.0, 0.1),
        GizmoValue::FaceAngle => (&mut settings.face_angle, "°".to_owned(), 1.0..=89.0, 0.5),
        GizmoValue::Berm => (&mut settings.berm_width, tr!("common-m"), 0.0..=1000.0, 0.1),
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let (mark, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
        ui.painter()
            .circle_filled(mark.center(), if lit { 4.0 } else { 3.5 }, if lit { egui::Color32::WHITE } else { value.colour() });
        let name = egui::RichText::new(value.name());
        ui.label(if lit { name.strong() } else { name.color(ui.visuals().weak_text_color()) });
        ui.add_sized([50.0, 22.0], egui::DragValue::new(number).range(range).speed(speed).fixed_decimals(1));
        ui.label(unit);
    })
    .response
    .contains_pointer()
}

/// How far `rect` must go along `way` (a unit) to clear `other`, if they
/// overlap.
fn clearing(rect: egui::Rect, other: egui::Rect, way: egui::Vec2) -> Option<f32> {
    if !rect.intersects(other) {
        return None;
    }
    let along = |low: f32, high: f32, other_low: f32, other_high: f32, way: f32| {
        if way > 1.0e-3 {
            Some((other_high - low) / way)
        } else if way < -1.0e-3 {
            Some((other_low - high) / way)
        } else {
            None
        }
    };
    let x = along(rect.min.x, rect.max.x, other.min.x, other.max.x, way.x);
    let y = along(rect.min.y, rect.max.y, other.min.y, other.max.y, way.y);
    match (x, y) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (Some(t), None) | (None, Some(t)) => Some(t),
        (None, None) => None,
    }
    .map(|push| push.max(0.0))
}

/// The most flags labelled in the viewport at once: past this, only the one
/// under the pointer is.
const FLAG_LABELS: usize = 12;

/// The pit's ramps, each under a heading that removes it; with none, how to
/// add one, which is done in the view: a click on the string.
/// `in_view` are the ramps whose gizmos stand in the view, which set what
/// they set there; the others', here.
fn draw_ramps(ui: &mut egui::Ui, settings: &mut PitDesignSettings, in_view: &[usize], base_rl: f64) {
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
    if settings.ramps.is_empty() {
        menu::menu_section(ui, tr!("pit-design-ramp-section"));
        menu::menu_note(ui, tr!("pit-design-ramp-none"));
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
    (base_rl, picking_region, on_gizmo, built): (f64, bool, bool, Option<u32>),
    commands: &mut Vec<UiCommand>,
) {
    // What it is and the way it is built are pulled on the walls' gizmo: its
    // arms through the toe turn the walls the other way. It is built until
    // it meets the ground where it can, unless cut short beside the toe.
    // The ground it meets and is measured against, here.
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
    // How far it goes: beside the walls' gizmo in the view, or here without.
    if !on_gizmo {
        MenuField::new(tr!("pit-design-benches")).show(ui, |ui, row_height, _| {
            ui.allocate_ui_with_layout(egui::vec2(FIELD_WIDTH, row_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                extent_controls(ui, settings, built, false);
            })
            .response
        });
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
            // The count stands in the view beside the toe while the walls'
            // gizmo is up; here only when it is not.
            if draft.wall_gizmo.is_none() {
                read_only_row(ui, &tr!("pit-design-benches-built"), &design.benches.to_string());
            }
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
    // The ground it is measured against is the one it meets, chosen with
    // the benches.
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
