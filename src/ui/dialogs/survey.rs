//! The Survey workspace's Coordinates menu: the systems a site works in, and
//! moving data between them.
//!
//! Definitions is built like Preferences - a list down the left, the selected
//! entry's fields on the right - because it is the same shape of thing: a
//! small, standing set of named settings that outlives any project.
//!
//! Every row is a coordinate system and there is no anonymous frame behind
//! them. A site's own numbers are a system in the list like any other, usually
//! the one marked as the mine coordinate system; a grid is defined against a
//! *named* parent rather than against something invisible. That is what mine
//! grids actually are on paper, and it is what stops a site having to invent a
//! do-nothing definition just to give the project's own coordinates a name.
//!
//! Nothing in the list is a button: a system is edited by clicking its row,
//! and the set itself is changed from the right-click menus. Edits write
//! straight through to the config the way Preferences do, once a field's edit
//! lands rather than on every frame of a drag.
use crate::{
    i18n::tr,
    model::{
        SceneEntityId,
        crs::{self, SurveyTransform},
        survey::{MineGridTransform, SystemDefinition, SystemKind, resolve_system},
    },
    ui::{
        EditorState, UiCommand, unthemed_icon,
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup, context_menu_separator},
            explorer::{ExplorerEntry, explorer_note, row_height, stripe_bands},
            menu::{self, DragableMenu, MenuButton, MenuFieldCombo, MenuFieldF64, MenuFieldText, committed},
            tree_row_colors,
        },
    },
};

/// Which sort of system the detail pane is editing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DraftKind {
    #[default]
    Registry,
    Grid,
}

impl DraftKind {
    fn label(self) -> String {
        match self {
            Self::Registry => tr!("survey-kind-registry-short"),
            Self::Grid => tr!("survey-kind-grid-short"),
        }
    }
}

#[derive(Default)]
pub(crate) struct SurveyState {
    pub(crate) definitions: Vec<SystemDefinition>,
    /// The system the project's numbers are in.
    pub(crate) local_system: Option<String>,
    pub(crate) definitions_open: bool,
    pub(crate) transform_open: bool,
    /// Which row the Definitions list stands on.
    pub(crate) editing_name: Option<String>,
    pub(crate) name: String,
    pub(crate) kind: DraftKind,
    /// The registry search box, and the code it settled on.
    pub(crate) registry_query: String,
    pub(crate) registry_code: Option<u16>,
    pub(crate) parent: Option<String>,
    pub(crate) transform: MineGridTransform,
    /// The three axis names as typed. Blank means "use X, Y and Z".
    pub(crate) axis_names: [String; 3],
    pub(crate) source_system: Option<String>,
    pub(crate) target_system: Option<String>,
    pub(crate) definition_message: Option<String>,
}

impl SurveyState {
    /// Point the Definitions list at `name`, seeding the draft from the saved
    /// definition behind it.
    pub(crate) fn edit_definition(&mut self, name: Option<String>) {
        let definition = name.as_ref().and_then(|name| self.definitions.iter().find(|d| &d.name == name)).cloned();
        self.name = definition.as_ref().map_or_else(String::new, |d| d.name.clone());
        self.kind = match definition.as_ref().map(|d| &d.kind) {
            Some(SystemKind::Grid { .. }) => DraftKind::Grid,
            _ => DraftKind::Registry,
        };
        self.registry_code = match definition.as_ref().map(|d| &d.kind) {
            Some(SystemKind::Registry { code }) => Some(*code),
            _ => None,
        };
        self.registry_query = String::new();
        (self.parent, self.transform) = match definition.as_ref().map(|d| &d.kind) {
            Some(SystemKind::Grid { parent, transform }) => (Some(parent.clone()), *transform),
            _ => (None, MineGridTransform::default()),
        };
        self.axis_names = definition.as_ref().and_then(|d| d.axis_names.clone()).unwrap_or_default();
        self.editing_name = name;
        self.definition_message = None;
    }

    pub(crate) fn open_transform(&mut self) {
        self.definitions_open = false;
        self.transform_open = true;
        self.source_system = self.local_system.clone();
        self.target_system = None;
    }

    /// The draft as a definition, or why it is not one yet.
    fn draft(&self) -> anyhow::Result<SystemDefinition> {
        let name = self.name.trim().to_owned();
        anyhow::ensure!(!name.is_empty(), "{}", tr!("survey-name-required"));
        let kind = match self.kind {
            DraftKind::Registry => SystemKind::Registry {
                code: self.registry_code.ok_or_else(|| anyhow::anyhow!("{}", tr!("survey-pick-registry")))?,
            },
            DraftKind::Grid => {
                self.transform.validate()?;
                SystemKind::Grid {
                    parent: self.parent.clone().ok_or_else(|| anyhow::anyhow!("{}", tr!("survey-pick-parent")))?,
                    transform: self.transform,
                }
            }
        };
        // All three or none: two named axes and one still called Z would read
        // as a mistake rather than a choice.
        let axis_names = self
            .axis_names
            .iter()
            .all(|name| !name.trim().is_empty())
            .then(|| self.axis_names.clone().map(|name| name.trim().to_owned()));
        Ok(SystemDefinition { name, kind, axis_names })
    }

    pub(crate) fn resolve(&self, name: &Option<String>) -> anyhow::Result<crs::CoordinateSystem> {
        let name = name.as_deref().ok_or_else(|| anyhow::anyhow!("{}", tr!("survey-pick-systems")))?;
        resolve_system(name, &self.definitions)
    }

    /// The selected conversion, planned but not yet run.
    pub(crate) fn selected_survey_transform(&self) -> anyhow::Result<SurveyTransform> {
        anyhow::ensure!(self.source_system != self.target_system, "{}", tr!("survey-same-system"));
        let from = self.resolve(&self.source_system)?;
        let to = self.resolve(&self.target_system)?;
        Ok(crs::Conversion::plan(&from, &to)?.into_transform())
    }

    /// A system's name, or the prompt when none is chosen yet.
    ///
    /// The mine coordinate system is not marked here. It carries its own icon
    /// in the definitions list, and a suffix on one entry of a two-entry combo
    /// says nothing about the conversion being set up.
    pub(crate) fn system_label(&self, system: &Option<String>) -> String {
        system.clone().unwrap_or_else(|| tr!("survey-pick-system"))
    }
}

/// Fill one point's three coordinate fields, reporting whether any edit landed.
fn origin_fields(ui: &mut egui::Ui, origin: &mut glam::DVec3) -> bool {
    let mut landed = false;
    for (label, value) in crate::model::survey::axis_names().into_iter().zip([&mut origin.x, &mut origin.y, &mut origin.z]) {
        landed |= committed(&MenuFieldF64::new(label, value, -f64::MAX..=f64::MAX).speed(0.1).max_decimals(8).show(ui));
    }
    landed
}

/// Add a system under a name nothing else has taken.
///
/// It is saved immediately rather than held as a pending draft, so the list
/// always shows exactly what the config holds and a half-finished entry cannot
/// be stranded by closing the dialog. A site's own grid is the default, because
/// it is what a project already has before anyone says anything about it.
fn add_definition(state: &SurveyState, commands: &mut Vec<UiCommand>) {
    commands.push(UiCommand::SaveSurveyDefinition {
        target: None,
        definition: SystemDefinition {
            name: crate::model::project::unique_item_name(tr!("survey-new-system-name"), state.definitions.iter().map(|d| d.name.as_str())),
            // WGS84: a real system, so a new row is never unsaveable, and the
            // one everybody recognises well enough to know to change it.
            kind: SystemKind::Registry { code: 4326 },
            axis_names: None,
        },
    });
}

/// The systems themselves, one to a row.
///
/// A clicked row is returned rather than applied, because the page beside the
/// list still has to be given the frame to commit whatever edit was in it:
/// switching rows is how most renames end, and swapping the draft out from
/// under the fields first would drop it.
fn draw_definition_list(ui: &mut egui::Ui, state: &SurveyState, commands: &mut Vec<UiCommand>) -> Option<Option<String>> {
    let mut select = None;
    let mut mark = None;
    let mut delete = None;
    let mut add = false;

    for definition in &state.definitions {
        let name = definition.name.clone();
        let local = state.local_system.as_ref() == Some(&name);
        let mut entry = ExplorerEntry::new(ui.id().with(("survey_system", &name)), name.clone()).selected(state.editing_name.as_ref() == Some(&name));
        // The mine coordinate system is where the project's numbers already
        // are, so it is worth seeing without reading any of the fields.
        if local {
            entry = entry.leading_icon(unthemed_icon!("local_system.svg"), ui.visuals().strong_text_color());
        }
        let response = entry.show(ui).response.on_hover_text(definition.summary());
        if response.clicked() {
            select = Some(Some(name.clone()));
        }
        context_menu_popup(&response, name.clone(), |ui| {
            if ContextMenuAction::new(tr!("survey-set-local")).enabled(!local).show(ui).clicked() {
                mark = Some(name.clone());
                ui.close();
            }
            if ContextMenuAction::new(tr!("survey-delete-system")).show(ui).clicked() {
                delete = Some(name.clone());
                ui.close();
            }
            context_menu_separator(ui);
            if ContextMenuAction::new(tr!("survey-new-system")).show(ui).clicked() {
                add = true;
                ui.close();
            }
        });
    }
    if state.definitions.is_empty() {
        explorer_note(ui, tr!("survey-systems-empty"));
    }

    // Everything below the last row answers to the same menu, so a site with
    // no systems yet still has somewhere to right-click.
    let empty = ui
        .allocate_exact_size(egui::vec2(ui.available_width(), ui.available_height().max(0.0)), egui::Sense::click())
        .1;
    context_menu_popup(&empty, tr!("survey-definitions-title"), |ui| {
        if ContextMenuAction::new(tr!("survey-new-system")).show(ui).clicked() {
            add = true;
            ui.close();
        }
    });

    if let Some(name) = mark {
        commands.push(UiCommand::SetSurveyLocalSystem(Some(name)));
    }
    if let Some(name) = delete {
        commands.push(UiCommand::DeleteSurveyDefinition(name));
    }
    if add {
        add_definition(state, commands);
    }
    select
}

/// The searchable registry picker.
fn registry_fields(ui: &mut egui::Ui, state: &mut SurveyState) -> bool {
    let mut landed = false;
    if let Some(code) = state.registry_code {
        menu::menu_note(ui, crs::registry_name(code).map_or_else(|| format!("EPSG:{code}"), |name| format!("{name}\nEPSG:{code}")));
    }
    MenuFieldText::new(tr!("survey-registry-search"), &mut state.registry_query)
        .hint_text(tr!("survey-registry-hint"))
        .show(ui);
    let query = state.registry_query.trim();
    if query.is_empty() {
        return landed;
    }
    // A short list, because a picker that returns four hundred rows is a list
    // nobody reads. Narrowing is done by typing another word.
    let hits = crs::search_registry(query, 12);
    if hits.is_empty() {
        menu::menu_note(ui, tr!("survey-registry-none"));
    }
    for code in hits {
        let label = crs::registry_name(code).map_or_else(|| format!("EPSG:{code}"), |name| format!("{name}  ·  EPSG:{code}"));
        if ui.add(MenuButton::new(label).enabled(state.registry_code != Some(code))).clicked() {
            state.registry_code = Some(code);
            landed = true;
        }
    }
    landed
}

/// The selected system's fields.
fn draw_definition_page(ui: &mut egui::Ui, state: &mut SurveyState, commands: &mut Vec<UiCommand>) {
    let Some(saved) = state.editing_name.as_ref().and_then(|name| state.definitions.iter().find(|d| &d.name == name)).cloned() else {
        menu::menu_note(ui, tr!("survey-no-selection"));
        return;
    };

    let name_response = MenuFieldText::new(tr!("survey-system-name"), &mut state.name)
        .hint_text(tr!("survey-name-required"))
        .show(ui);
    // An emptied name is a slip rather than an edit, so the saved one goes
    // back instead of the save failing behind the user.
    if name_response.lost_focus() && state.name.trim().is_empty() {
        state.name = saved.name.clone();
    }
    // A name commits when it is left, not per keystroke: saving each one
    // would re-sort the list under the cursor as it is typed.
    let mut landed = name_response.lost_focus();

    let kinds = [DraftKind::Registry, DraftKind::Grid];
    let kind_text = state.kind.label();
    landed |= MenuFieldCombo::new("survey-kind", tr!("survey-kind"), &mut state.kind, kind_text, kinds.map(|kind| (kind, kind.label().into())))
        .show(ui)
        .changed();

    match state.kind {
        DraftKind::Registry => landed |= registry_fields(ui, state),
        DraftKind::Grid => {
            // A grid can sit over any other saved system except itself: that
            // is the loop `resolve_system` would otherwise have to catch.
            let options: Vec<_> = state
                .definitions
                .iter()
                .filter(|definition| Some(&definition.name) != state.editing_name.as_ref())
                .map(|definition| (Some(definition.name.clone()), definition.name.clone().into()))
                .collect();
            let parent_text = state.parent.clone().unwrap_or_else(|| tr!("survey-pick-parent"));
            landed |= MenuFieldCombo::new("survey-parent", tr!("survey-parent"), &mut state.parent, parent_text, options)
                .show(ui)
                .changed();
            menu::menu_section(ui, tr!("survey-parent-origin"));
            landed |= origin_fields(ui, &mut state.transform.source_origin);
            menu::menu_section(ui, tr!("survey-system-origin"));
            landed |= origin_fields(ui, &mut state.transform.target_origin);
            landed |= committed(
                &MenuFieldF64::new(tr!("survey-angle"), &mut state.transform.angle_degrees, -f64::MAX..=f64::MAX)
                    .suffix("\u{b0}")
                    .speed(0.1)
                    .max_decimals(8)
                    .help_text(tr!("survey-angle-help"))
                    .show(ui),
            );
            landed |= committed(
                &MenuFieldF64::new(tr!("survey-scale"), &mut state.transform.scale, 0.0..=f64::MAX)
                    .speed(0.000001)
                    .max_decimals(10)
                    .help_text(tr!("survey-scale-help"))
                    .show(ui),
            );
        }
    }

    // Axis names belong to the system rather than to one kind of system, so
    // they sit below whatever the kind above needed.
    menu::menu_section(ui, tr!("survey-axis-names"));
    for (index, default) in ["X", "Y", "Z"].into_iter().enumerate() {
        landed |= MenuFieldText::new(default, &mut state.axis_names[index]).hint_text(default).show(ui).lost_focus();
    }

    let draft = state.draft();
    if let Err(error) = &draft {
        menu::menu_note(ui, error.to_string());
    }
    if let Some(message) = &state.definition_message {
        menu::menu_note(ui, message);
    }
    if let Ok(draft) = draft
        && landed
        && draft != saved
    {
        commands.push(UiCommand::SaveSurveyDefinition {
            target: Some(saved.name),
            definition: draft,
        });
    }
}

pub(crate) fn draw_definitions_dialog(ui: &mut egui::Ui, editor: &mut EditorState, commands: &mut Vec<UiCommand>) {
    let state = &mut editor.survey;
    if !state.definitions_open {
        return;
    }
    let mut open = true;
    // Sized and scaled exactly as Preferences is, so the two dialogs are the
    // same object at whatever window size they are opened at.
    let available = ui.ctx().content_rect().size() - egui::vec2(24.0, 24.0);
    let scale = (available.x / 620.0).min(available.y / 460.0).clamp(0.1, 1.0);
    let size = egui::vec2(620.0, 460.0) * scale;
    DragableMenu::new("survey_definitions", tr!("survey-definitions-title"))
        .open(&mut open)
        .fixed_size(size)
        .inner_margin(egui::Margin::ZERO)
        .show(ui.ctx(), |ui| {
            let mut select = None;
            let body_height = (size.y - menu::TITLE_BAR_HEIGHT).max(0.0);
            let (body, _) = ui.allocate_exact_size(egui::vec2(size.x, body_height), egui::Sense::hover());
            let navigation = egui::Rect::from_min_max(body.min, egui::pos2(body.left() + 180.0 * scale, body.bottom()));
            let page = egui::Rect::from_min_max(egui::pos2(navigation.right() + 1.0, body.top()), body.max);
            let (surface, stripe) = tree_row_colors(ui);
            ui.painter().rect_filled(navigation, 0.0, surface);
            ui.painter()
                .add(stripe_bands(navigation.x_range(), navigation.top(), navigation.bottom(), row_height(ui), stripe));
            ui.painter()
                .line_segment([navigation.right_top(), navigation.right_bottom()], ui.visuals().widgets.noninteractive.bg_stroke);
            ui.scope_builder(egui::UiBuilder::new().id_salt("survey_systems").max_rect(navigation), |ui| {
                ui.set_clip_rect(ui.clip_rect().intersect(navigation));
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.spacing_mut().interact_size.y = row_height(ui);
                ui.spacing_mut().button_padding.y = 0.0;
                select = egui::ScrollArea::vertical()
                    .id_salt("survey_systems_scroll")
                    .auto_shrink([false; 2])
                    .show(ui, |ui| draw_definition_list(ui, state, commands))
                    .inner;
            });
            ui.scope_builder(
                egui::UiBuilder::new().id_salt("survey_definition_page").max_rect(page.shrink2(egui::vec2(12.0, 8.0))),
                |ui| {
                    ui.set_clip_rect(ui.clip_rect().intersect(page));
                    egui::ScrollArea::both()
                        .id_salt("survey_definition_page_scroll")
                        .auto_shrink([false; 2])
                        .show(ui, |ui| draw_definition_page(ui, state, commands));
                },
            );
            if let Some(row) = select {
                state.edit_definition(row);
            }
            if menu::dialog_cancel_pressed(ui.ctx()) {
                state.definitions_open = false;
            }
        });
    state.definitions_open &= open;
}

pub(crate) fn draw_transform_dialog(ui: &mut egui::Ui, editor: &mut EditorState, has_project: bool, commands: &mut Vec<UiCommand>) {
    if !editor.survey.transform_open {
        return;
    }
    let mut counts = [0usize; 6];
    for handle in &editor.selected_handles {
        counts[match handle {
            SceneEntityId::Object(_) => 0,
            SceneEntityId::Triangulation(_) => 1,
            SceneEntityId::BlockModel(_) => 2,
            SceneEntityId::PointCloud(_) => 3,
            SceneEntityId::DrillHole(_) => 4,
            SceneEntityId::Raster(_) => 5,
        }] += 1;
    }
    let state = &mut editor.survey;
    let mut open = true;
    DragableMenu::new("survey_transform", tr!("survey-transform-title"))
        .open(&mut open)
        .min_width(390.0)
        .max_width(540.0)
        .show(ui.ctx(), |ui| {
            // Only saved systems: there is no frame behind them to offer as
            // a third option. From starts on the mine coordinate system,
            // because that is where the project's numbers already are.
            let options: Vec<_> = state
                .definitions
                .iter()
                .map(|definition| {
                    let id = Some(definition.name.clone());
                    let label = state.system_label(&id);
                    (id, label.into())
                })
                .collect();
            let source_text = state.system_label(&state.source_system);
            let target_text = state.system_label(&state.target_system);
            MenuFieldCombo::new("survey-from", tr!("survey-from"), &mut state.source_system, source_text, options.clone()).show(ui);
            MenuFieldCombo::new("survey-to", tr!("survey-to"), &mut state.target_system, target_text, options).show(ui);
            let count = counts.iter().sum::<usize>();
            if count == 0 {
                menu::menu_note(ui, tr!("survey-empty-selection"));
            } else {
                menu::menu_note(ui, crate::model::survey::describe_counts(counts));
            }
            // What the conversion will do is stated before it is run, not
            // reported after: which published operations it goes through, and
            // how accurate they claim to be. A conversion nobody can vouch for
            // is refused here rather than silently returning its input.
            let plan = state.selected_survey_transform();
            match &plan {
                Err(error) => menu::menu_note(ui, error.to_string()),
                Ok(transform) => {
                    for step in transform.steps() {
                        menu::menu_note(ui, step.clone());
                    }
                    match transform.accuracy_m() {
                        Some(accuracy) => menu::menu_note(ui, tr!("survey-conversion-accuracy", accuracy = format!("{accuracy}"))),
                        None => menu::menu_note(ui, tr!("survey-conversion-exact")),
                    }
                }
            }
            // Only the surfaces in the selection lose anything the conversion
            // cannot carry, so the warning appears only when one is there.
            if counts[1] > 0 {
                menu::menu_note(ui, tr!("survey-drape-note"));
            }
            menu::menu_actions(ui, |ui| {
                let enabled = has_project && count > 0 && plan.is_ok();
                let submitted = menu::dialog_confirm_pressed(ui.ctx());
                if (ui.add(MenuButton::new(tr!("survey-transform-button")).primary().enabled(enabled)).clicked() || submitted) && enabled {
                    commands.push(UiCommand::TransformSurveySelection);
                    // The run is the end of the dialog's job: progress, the
                    // result and any failure are reported in the activity
                    // console, so there is nothing left here to read.
                    state.transform_open = false;
                }
                if ui.add(MenuButton::new(tr!("survey-swap"))).clicked() {
                    std::mem::swap(&mut state.source_system, &mut state.target_system);
                }
                if ui.add(MenuButton::new(tr!("survey-close"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    state.transform_open = false;
                }
            });
        });
    state.transform_open &= open;
}
