//! The Schedule workspace's destination pages: Stockpiles, Dumps, Crushers and
//! the ordered routing rules.
//!
//! Three lists and a rule table. The lists are *derived*: every stockpile and
//! dump solid the project holds appears on its page automatically, named by
//! the solid, so drawing one in Solids is all it takes to deliver to it and
//! there is no second name to keep in step. What these pages own is the
//! receiving capacity, and - for standalone destinations with no geometry -
//! the name and kind as well.
//!
//! Nothing here computes and nothing here writes on open: a page that created
//! a document entry merely by being looked at would put an undo step on the
//! stack nobody asked for.

use crate::{
    i18n::tr,
    model::{
        Document, ReserveAggregation,
        schedule::{
            Bound, ConditionTest, DestinationId, DestinationKind, FieldCondition, LoaderAgentId, LoaderSelection, MovementSourceScope, MovementSourceSelection, SchedulePlan,
            SourceScope, StandaloneDestinationId, destinations,
        },
    },
    ui::{
        EditorState,
        state::{ConditionOwner, ScheduleConditionDraft, ScheduleDestinationDraft, ScheduleEdit, ScheduleRuleDraft, SourceScopeView, UiCommand},
        widgets::{
            context_menu::{ChecklistRow, ContextMenuAction, Tick, checklist_popup, context_menu_popup, context_menu_separator},
            data_grid::{
                CELL_WARNING_WIDTH, DataGrid, GridRow, PropertyTable, grid_add_action_row, grid_cell_fixed, grid_cell_warning, grid_columns_row, grid_named_row, grid_row,
                grid_select_row, grid_separator_row, property_table_height,
            },
            explorer::explorer_note,
            menu::{self, DragableMenu, MenuButton, MenuFieldCombo, MenuFieldText},
        },
    },
};

/// Blank is unlimited, and is not the same answer as zero - so the field holds
/// text and an empty one is a deliberate choice rather than a failed parse.
fn capacity_text(capacity: Option<f64>) -> String {
    capacity.map(|value| value.to_string()).unwrap_or_default()
}

/// Parse a typed capacity. An empty field is unlimited; anything else must be a
/// number of tonnes that is not negative.
fn parse_capacity(text: &str) -> Result<Option<f64>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let Ok(value) = trimmed.parse::<f64>() else {
        return Err(tr!("schedule-rate-not-a-number"));
    };
    if !value.is_finite() || value < 0.0 {
        return Err(crate::model::schedule::ScheduleError::InvalidCapacity.message());
    }
    Ok(Some(value))
}

/// A rest in hours: blank is none.
fn parse_rest(text: &str) -> Result<f64, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(0.0);
    }
    text.parse::<f64>()
        .ok()
        .and_then(|hours| crate::model::schedule::stockpile_operation::checked_rest(hours).ok())
        .ok_or_else(|| crate::model::schedule::ScheduleError::InvalidExperimentSetting.message())
}

fn name_problem(name: &str, taken: impl Iterator<Item = String>) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Some(crate::model::schedule::ScheduleError::EmptyName.message());
    }
    taken
        .into_iter()
        .any(|other| other.trim().eq_ignore_ascii_case(trimmed))
        .then(|| crate::model::schedule::ScheduleError::DuplicateName(trimmed.to_owned()).message())
}

/// The destinations of one kind, solid-backed first and then standalone.
///
/// Rebuilt from the document every frame on purpose: a stockpile drawn while
/// this page was open should be in the list, and nothing has to notice it was
/// added for that to happen.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_destination_list(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    kind: DestinationKind,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let available = destinations::available(document.solids(), plan.routing());
    let entries: Vec<_> = available.into_iter().filter(|entry| entry.kind == kind).collect();
    let mut selected = editor.schedule_selected_destination;
    let mut open_dialog = false;
    let title = kind_page_title(kind);
    // A crusher's figure is a daily budget; a pile's or a dump's, what it holds.
    let figure = if kind == DestinationKind::Crusher {
        tr!("destination-limit-column")
    } else {
        tr!("destination-capacity-column")
    };
    let columns = [(tr!("planning-name"), LIST_FRACTIONS[0]), (figure, LIST_FRACTIONS[1])];
    DataGrid::new(list_id(kind), rect, &title).columns(&columns).show(ui, |ui| {
        for entry in &entries {
            let figure = match entry.kind {
                DestinationKind::Crusher => plan
                    .routing()
                    .crusher(standalone_of(entry.id))
                    .and_then(|calendar| calendar.default_tpd)
                    .map(tonnes_per_day),
                _ => entry.capacity_t.map(tonnes),
            }
            .unwrap_or_else(|| tr!("destination-unlimited"));
            let (response, _) = grid_columns_row(ui, &LIST_FRACTIONS, &[&entry.name, &figure], selected == Some(entry.id));
            if response.clicked() {
                selected = Some(entry.id);
            }
            let linked = entry.is_linked();
            context_menu_popup(&response, &entry.name, |ui| {
                // A linked destination is the solid: it is deleted in Solids,
                // and offering deletion here would imply the schedule owned it.
                if ContextMenuAction::new(tr!("destination-delete")).enabled(!linked).show(ui).clicked() {
                    if let DestinationId::Standalone(id) = entry.id {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DeleteDestination(id)));
                    }
                    ui.close();
                }
                if linked {
                    ContextMenuAction::new(tr!("destination-edit-in-solids")).enabled(false).show(ui);
                }
            });
        }
        if grid_add_action_row(ui, &new_label(kind)) {
            open_dialog = true;
        }
    });
    if editor.schedule_selected_destination != selected {
        editor.schedule_destination_draft = None;
    }
    editor.schedule_selected_destination = selected;
    if open_dialog && !editor.new_destination_open {
        editor.new_destination_name = suggested_destination_name(plan, document, kind);
        editor.new_destination_kind = kind;
        editor.new_destination_open = true;
    }
}

/// Seed the selected destination's draft from the plan, unless it already
/// holds the same source and may hold half-typed text.
fn destination_draft<'e>(editor: &'e mut EditorState, plan: &SchedulePlan, entry: &destinations::DestinationView) -> &'e mut ScheduleDestinationDraft {
    let crusher_default = (entry.kind == DestinationKind::Crusher)
        .then(|| plan.routing().crusher(standalone_of(entry.id)).and_then(|calendar| calendar.default_tpd))
        .flatten();
    let rest_h = plan.stockpile_operation(entry.id).rest_h;
    let chunk_t = plan.experiment().chunk_t(entry.id);
    let dump_time = plan.routing().dump_time_s(entry.id);
    let source = (
        entry.name.clone(),
        entry.capacity_t,
        crusher_default,
        rest_h.to_bits(),
        dump_time.map(f64::to_bits),
        chunk_t.map(f64::to_bits),
    );
    if editor
        .schedule_destination_draft
        .as_ref()
        .is_none_or(|draft| draft.id != entry.id || draft.source != source)
    {
        editor.schedule_destination_draft = Some(ScheduleDestinationDraft {
            id: entry.id,
            name: entry.name.clone(),
            capacity: capacity_text(entry.capacity_t),
            crusher_default: capacity_text(crusher_default),
            dump_time: dump_time.map(|v| v.to_string()).unwrap_or_default(),
            rest: super::schedule_trucking::number(rest_h),
            chunk: capacity_text(chunk_t),
            source,
        });
    }
    editor.schedule_destination_draft.as_mut().expect("just ensured")
}

/// The selected destination of `kind`, if it still resolves.
fn selected_destination(editor: &EditorState, plan: &SchedulePlan, document: &Document, kind: DestinationKind) -> Option<destinations::DestinationView> {
    editor
        .schedule_selected_destination
        .and_then(|id| destinations::resolve(id, document.solids(), plan.routing()).ok())
        .filter(|entry| entry.kind == kind)
}

/// The Name rows: typed for a standalone destination, read-only with where
/// it is edited for one that is a solid.
fn name_rows(
    rows: &mut crate::ui::widgets::data_grid::PropertyRows<'_>,
    entry: &destinations::DestinationView,
    draft: &mut ScheduleDestinationDraft,
    plan: &SchedulePlan,
    session: u32,
    edits: &mut Vec<UiCommand>,
) {
    if entry.is_linked() {
        rows.readonly(&tr!("planning-name"), &entry.name, None, None);
        rows.readonly(&tr!("destination-linked-solid"), &tr!("destination-linked-note"), None, None);
        return;
    }
    let taken = plan
        .routing()
        .standalone
        .iter()
        .filter(|other| DestinationId::Standalone(other.id) != entry.id)
        .map(|other| other.name.clone());
    let name_error = name_problem(&draft.name, taken);
    let response = rows.field(&tr!("planning-name"), &mut draft.name, name_error.as_deref());
    if response.lost_focus()
        && name_error.is_none()
        && draft.name.trim() != entry.name
        && let DestinationId::Standalone(id) = entry.id
    {
        edits.push(UiCommand::schedule(
            session,
            ScheduleEdit::RenameDestination {
                destination: id,
                name: draft.name.trim().to_owned(),
            },
        ));
    }
}

/// The Maximum tonnes row, blank for unlimited. `problem` is a reason the
/// typed figure, though a number, will not do.
fn capacity_row(
    rows: &mut crate::ui::widgets::data_grid::PropertyRows<'_>,
    entry: &destinations::DestinationView,
    draft: &mut ScheduleDestinationDraft,
    problem: Option<String>,
    session: u32,
    edits: &mut Vec<UiCommand>,
) {
    let error = parse_capacity(&draft.capacity).err().or(problem);
    let response = rows.field_with_unit_and_hint(&tr!("destination-capacity"), &mut draft.capacity, "t", &tr!("pile-capacity-unlimited"), error.as_deref());
    if response.lost_focus()
        && let Ok(capacity) = parse_capacity(&draft.capacity)
        && capacity != entry.capacity_t
    {
        edits.push(UiCommand::schedule(
            session,
            ScheduleEdit::SetDestinationCapacity {
                destination: entry.id,
                capacity_t: capacity,
            },
        ));
    }
}

/// How trucks meet one of a destination's points, in the Haulage step's words,
/// with a warning when they cannot.
fn haul_point_row(rows: &mut crate::ui::widgets::data_grid::PropertyRows<'_>, key: &str, link: Option<crate::app::commands::haulage::HaulLink>, problem: fn(String) -> String) {
    let value = link.map_or_else(|| tr!("pile-not-checked"), |link| link.label());
    let warning = link.filter(|link| link.is_problem()).map(|link| problem(link.label()));
    rows.readonly_warning(key, &value, None, warning.as_deref());
}

fn dump_problem(status: String) -> String {
    tr!("haul-link-dump-problem", status = status)
}

fn reclaim_problem(status: String) -> String {
    tr!("haul-link-reclaim-problem", status = status)
}

/// The selected dump's or crusher's cells.
///
/// A linked destination's name and type are read-only and say where they are
/// edited; its capacity is the schedule's own and is editable here. A crusher
/// has no storage, so it carries a daily budget instead of a capacity.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_destination_properties(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    kind: DestinationKind,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let Some(entry) = selected_destination(editor, plan, document, kind) else {
        let table_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), property_table_height(ui, 2).min(rect.height())));
        PropertyTable::new("schedule_destination_properties", table_rect, &kind_page_title(kind)).show(ui, |rows| {
            rows.header(&tr!("planning-property"), &tr!("planning-value"));
            rows.readonly(&tr!("planning-name"), &tr!("destination-select"), None, None);
        });
        return;
    };
    let link = editor
        .schedule_haul_connections
        .as_ref()
        .and_then(|connections| connections.destinations.iter().find(|links| links.id == entry.id))
        .map(|links| links.dump);
    let crusher = kind == DestinationKind::Crusher;
    let draft = destination_draft(editor, plan, &entry);
    let rows_used = 6 + usize::from(entry.is_linked()) + usize::from(crusher && !entry.is_linked());
    let table_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), property_table_height(ui, rows_used).min(rect.height())));
    let mut edits = Vec::new();
    PropertyTable::new("schedule_destination_properties", table_rect, &entry.name).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));
        name_rows(rows, &entry, draft, plan, session, &mut edits);
        rows.readonly(&tr!("destination-type"), &entry.kind.label(), None, None);
        if crusher {
            let current = plan.routing().crusher(standalone_of(entry.id)).and_then(|calendar| calendar.default_tpd);
            let error = parse_capacity(&draft.crusher_default).err();
            let response = rows.field_with_unit_and_hint(
                &tr!("destination-daily-limit"),
                &mut draft.crusher_default,
                "t",
                &tr!("pile-capacity-unlimited"),
                error.as_deref(),
            );
            if response.lost_focus()
                && let Ok(limit) = parse_capacity(&draft.crusher_default)
                && limit != current
                && let DestinationId::Standalone(id) = entry.id
            {
                edits.push(UiCommand::schedule(
                    session,
                    ScheduleEdit::SetCrusherCells {
                        edits: vec![crate::model::schedule::CrusherCellEdit {
                            destination: id,
                            cell: crate::model::schedule::CrusherCell::Default,
                            value: limit.map(crate::model::schedule::CrusherOverride::Limit),
                        }],
                    },
                ));
            }
            if let DestinationId::Standalone(destination) = entry.id {
                let response = rows.field(&tr!("haul-dump-override"), &mut draft.dump_time, None);
                if response.lost_focus() {
                    let seconds = if draft.dump_time.trim().is_empty() {
                        Some(None)
                    } else {
                        draft.dump_time.parse::<f64>().ok().map(Some)
                    };
                    if let Some(seconds) = seconds
                        && seconds != plan.routing().dump_time_s(entry.id)
                    {
                        edits.push(UiCommand::schedule(session, ScheduleEdit::SetDestinationDumpTime { destination, seconds }));
                    }
                }
            }
        } else {
            capacity_row(rows, &entry, draft, None, session, &mut edits);
        }
        haul_point_row(rows, &tr!("haul-link-dump-column"), link, dump_problem);
        if rows.action_row(&tr!("haul-open-layout")).clicked() {
            edits.push(UiCommand::EditHaulProperties);
        }
    });
    commands.append(&mut edits);
}

/// A chunk size in tonnes: blank for none, otherwise above zero.
fn parse_chunk(text: &str) -> Result<Option<f64>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(Some)
        .ok_or_else(|| crate::model::schedule::ScheduleError::InvalidExperimentSetting.message())
}

/// The selected stockpile as one table read in sections - what it is, how it
/// is worked, how the optimiser sees it, what it opens holding and how trucks
/// reach it - and, for a chunked pile, its opening chunks and the one selected
/// among them beside it.
pub(crate) fn draw_stockpile(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, document: &Document, session: u32, commands: &mut Vec<UiCommand>) {
    use crate::model::schedule::{
        experiment::StockpileRepresentation,
        inventory::{OpeningBlend, OpeningValue},
    };

    let Some(entry) = selected_destination(editor, plan, document, DestinationKind::Stockpile) else {
        let table_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), property_table_height(ui, 2).min(rect.height())));
        PropertyTable::new("schedule_destination_properties", table_rect, &kind_page_title(DestinationKind::Stockpile)).show(ui, |rows| {
            rows.header(&tr!("planning-property"), &tr!("planning-value"));
            rows.readonly(&tr!("planning-name"), &tr!("destination-select"), None, None);
        });
        return;
    };
    let links = editor
        .schedule_haul_connections
        .as_ref()
        .and_then(|connections| connections.destinations.iter().find(|links| links.id == entry.id))
        .cloned();
    let categories = editor.schedule_category_values.clone();
    let operation = plan.stockpile_operation(entry.id).into_owned();
    let experiment = plan.experiment();
    let representation = experiment.representation(entry.id);
    let chunked = representation == StockpileRepresentation::Chunks;
    let chunk_t = experiment.chunk_t(entry.id);

    // The opening stock as one blend, and whether the chunks still are
    // exactly that blend split - so it can be edited as one.
    let extensive: Vec<_> = document
        .reserve_fields()
        .iter()
        .filter(|field| field.aggregation == ReserveAggregation::Sum)
        .map(|field| field.id)
        .collect();
    let inventory = plan.routing().inventory(entry.id).cloned().unwrap_or_default();
    let blend = inventory.combined(&extensive);
    let arranged = inventory.lots.is_empty() || inventory.is_arranged(&extensive, crate::app::commands::schedule::opening_split(plan, entry.id));
    let tonnage_field = plan.tonnage_field();
    let fields: Vec<_> = document.reserve_fields().iter().filter(|field| Some(field.id) != tonnage_field).collect();
    let tracked: Vec<_> = experiment.grades.iter().map(|(field, _)| *field).collect();

    let draft = destination_draft(editor, plan, &entry);
    let mut edits = Vec::new();
    let mut open_calendar = false;

    // What a typed figure, though a number, will not do for.
    let capacity_problem = entry
        .capacity_t
        .is_some_and(|capacity| entry.opening_t > capacity)
        .then(|| crate::model::schedule::ScheduleError::OpeningOverCapacity.message());
    let chunk_error = parse_chunk(&draft.chunk)
        .err()
        .or_else(|| (chunked && chunk_t.is_none()).then(|| tr!("pile-chunk-size-missing")));

    let gap = ui.spacing().item_spacing.y * 2.0;
    // A chunked pile shares the page with its chunks, the settings keeping
    // their full height; a blend has the page.
    let table_rect = if chunked {
        egui::Rect::from_min_size(rect.min, egui::vec2(((rect.width() - gap) * 0.5).max(0.0), rect.height()))
    } else {
        rect
    };
    let number = super::schedule_trucking::number;
    let mut new_blend: Option<OpeningBlend> = None;
    PropertyTable::new("schedule_pile_settings", table_rect, &entry.name).show(ui, |rows| {
        rows.header(&tr!("planning-property"), &tr!("planning-value"));

        rows.section(&tr!("pile-general"));
        name_rows(rows, &entry, draft, plan, session, &mut edits);
        rows.readonly(&tr!("destination-type"), &entry.kind.label(), None, None);
        capacity_row(rows, &entry, draft, capacity_problem.clone(), session, &mut edits);

        rows.section(&tr!("pile-operation"));
        let mut simultaneous = operation.simultaneous;
        rows.checkbox(&tr!("pile-simultaneous"), &mut simultaneous).on_hover_text(tr!("pile-simultaneous-help"));
        if simultaneous != operation.simultaneous {
            edits.push(UiCommand::schedule(
                session,
                ScheduleEdit::SetStockpileOperating {
                    destination: entry.id,
                    simultaneous,
                    rest_h: operation.rest_h,
                },
            ));
        }
        let rest_error = parse_rest(&draft.rest).err();
        let response = rows.field_with_unit(&tr!("pile-rest"), &mut draft.rest, "h", rest_error.as_deref());
        let committed = response.lost_focus();
        response.on_hover_text(tr!("pile-rest-help"));
        if committed
            && let Ok(rest_h) = parse_rest(&draft.rest)
            && rest_h != operation.rest_h
        {
            edits.push(UiCommand::schedule(
                session,
                ScheduleEdit::SetStockpileOperating {
                    destination: entry.id,
                    simultaneous: operation.simultaneous,
                    rest_h,
                },
            ));
        }
        // The day-by-day mode is the Calendar's; this says what it is and
        // takes you there.
        let differing = operation.periods.values().filter(|mode| **mode != operation.default_mode).count();
        let mode = if differing == 0 {
            operation.default_mode.label()
        } else {
            tr!("pile-daily-mode-overrides", mode = operation.default_mode.label(), days = differing.to_string())
        };
        rows.readonly(&tr!("pile-daily-mode"), &mode, None, None);
        open_calendar = rows.action_row(&tr!("pile-open-calendar")).clicked();

        rows.section(&tr!("pile-optimiser"));
        let mut chosen = representation;
        let response = rows.combo(
            ("pile_representation", format!("{:?}", entry.id)),
            &tr!("experiment-representation"),
            &mut chosen,
            &representation.label(),
            [StockpileRepresentation::Blended, StockpileRepresentation::Chunks].map(|option| (option, option.label())),
        );
        response.on_hover_text(if chunked { tr!("experiment-chunks-help") } else { tr!("experiment-blended-help") });
        if chosen != representation {
            edits.push(UiCommand::schedule(
                session,
                ScheduleEdit::SetStockpileRepresentation {
                    destination: entry.id,
                    representation: chosen,
                },
            ));
        }
        // Order and chunks mean something only to a chunked pile: a blend
        // has one composition and no oldest end.
        if chunked {
            let mut order = entry.reclaim_order;
            rows.combo(
                ("reclaim_order", format!("{:?}", entry.id)),
                &tr!("inventory-reclaim-order"),
                &mut order,
                &entry.reclaim_order.label(),
                [crate::model::schedule::ReclaimOrder::Fifo, crate::model::schedule::ReclaimOrder::Lifo].map(|option| (option, option.label())),
            );
            if order != entry.reclaim_order {
                edits.push(UiCommand::schedule(session, ScheduleEdit::SetReclaimOrder { destination: entry.id, order }));
            }
            let response = rows.field_with_unit(&tr!("experiment-chunk-size"), &mut draft.chunk, "t", chunk_error.as_deref());
            let committed = response.lost_focus();
            response.on_hover_text(tr!("experiment-chunks-help"));
            if committed
                && let Ok(chunk) = parse_chunk(&draft.chunk)
                && chunk != chunk_t
            {
                edits.push(UiCommand::schedule(
                    session,
                    ScheduleEdit::SetStockpileChunkSize {
                        destination: entry.id,
                        chunk_t: chunk,
                    },
                ));
            }
            // An unlimited pile takes as many as what its rules can send it
            // needs; a limited one its maximum tonnes' worth.
            let receiving = match (entry.capacity_t, chunk_t) {
                (_, None) => "—".to_owned(),
                (None, Some(chunk)) => tr!("experiment-receiving-unlimited", size = tonnes(chunk)),
                (Some(capacity), Some(chunk)) => {
                    let chunks = crate::model::schedule::experiment::receiving_chunks(capacity, chunk);
                    match chunks.split_last() {
                        Some((last, whole)) if (*last - chunk).abs() > 1e-6 => tr!(
                            "experiment-receiving-remainder",
                            count = whole.len().to_string(),
                            size = tonnes(chunk),
                            last = tonnes(*last)
                        ),
                        _ => tr!("experiment-receiving-whole", count = chunks.len().to_string(), size = tonnes(chunk)),
                    }
                }
            };
            rows.readonly(&tr!("experiment-receiving-chunks"), &receiving, None, None);
        }

        // One tonnage and one set of values. A chunked pile is split into
        // chunks of its chunk size from these; once a chunk is edited on its
        // own they are read back combined, and can be split again.
        rows.section(&tr!("pile-opening-stock"));
        let missing = |field: crate::model::ReserveFieldId| tracked.contains(&field) && blend.value(field).is_none() && blend.tonnes_t > 0.0;
        if arranged {
            let current = if blend.tonnes_t > 0.0 { number(blend.tonnes_t) } else { String::new() };
            let (_, typed) = rows.committed_entry_with_hint(
                ("pile_opening_tonnes", format!("{:?}", entry.id)),
                &tr!("inventory-lot-tonnes"),
                &current,
                Some("t"),
                &tr!("pile-opening-empty"),
            );
            if let Some(text) = typed {
                let text = text.trim();
                let tonnes_t = if text.is_empty() {
                    Some(0.0)
                } else {
                    text.parse::<f64>().ok().filter(|t| t.is_finite() && *t >= 0.0)
                };
                if let Some(tonnes_t) = tonnes_t {
                    new_blend = Some(OpeningBlend { tonnes_t, ..blend.clone() });
                }
            }
            if blend.tonnes_t > 0.0 {
                for field in &fields {
                    let held = blend.value(field.id);
                    if field.aggregation == ReserveAggregation::Category {
                        let mut chosen = match held {
                            Some(OpeningValue::Category(label)) => Some(label.clone()),
                            _ => None,
                        };
                        let mut options: Vec<Option<String>> = vec![None];
                        options.extend(categories.get(&field.id).into_iter().flatten().cloned().map(Some));
                        if let Some(label) = &chosen
                            && !options.contains(&Some(label.clone()))
                        {
                            options.push(Some(label.clone()));
                        }
                        let label = |value: &Option<String>| value.clone().unwrap_or_else(|| tr!("inventory-portion-missing"));
                        let before = chosen.clone();
                        rows.combo(
                            ("pile_opening_category", format!("{:?}", entry.id), field.id.0),
                            &field.name,
                            &mut chosen,
                            &label(&before),
                            options.iter().map(|option| (option.clone(), label(option))).collect::<Vec<_>>(),
                        );
                        if chosen != before {
                            let mut values = blend.values.clone();
                            values.retain(|(id, _)| *id != field.id);
                            if let Some(label) = chosen {
                                values.push((field.id, OpeningValue::Category(label)));
                            }
                            new_blend = Some(OpeningBlend { values, ..blend.clone() });
                        }
                        continue;
                    }
                    let current = match held {
                        Some(OpeningValue::Number(value)) => value.to_string(),
                        _ => String::new(),
                    };
                    let warning = missing(field.id).then(|| tr!("pile-grade-needed"));
                    let (_, typed) = rows.committed_entry_warned(
                        ("pile_opening_value", format!("{:?}", entry.id), field.id.0),
                        &field.name,
                        &current,
                        None,
                        " ",
                        warning.as_deref(),
                    );
                    if let Some(text) = typed {
                        let text = text.trim();
                        let value = if text.is_empty() {
                            Some(None)
                        } else {
                            text.parse::<f64>().ok().filter(|v| v.is_finite()).map(Some)
                        };
                        if let Some(value) = value {
                            let mut values = blend.values.clone();
                            values.retain(|(id, _)| *id != field.id);
                            if let Some(value) = value {
                                values.push((field.id, OpeningValue::Number(value)));
                            }
                            new_blend = Some(OpeningBlend { values, ..blend.clone() });
                        }
                    }
                }
            }
        } else {
            rows.note(&tr!("pile-opening-differs"));
            rows.readonly(&tr!("inventory-lot-tonnes"), &number(blend.tonnes_t), Some("t"), None);
            for field in &fields {
                let value = match blend.value(field.id) {
                    Some(OpeningValue::Number(value)) => value.to_string(),
                    Some(OpeningValue::Category(label)) => label.clone(),
                    None => tr!("pile-opening-not-held"),
                };
                let warning = missing(field.id).then(|| tr!("pile-grade-needed"));
                rows.readonly_warning(&field.name, &value, None, warning.as_deref());
            }
            let reset = if chunked { tr!("pile-opening-split-again") } else { tr!("pile-opening-combine") };
            if rows.action_row(&reset).clicked() {
                new_blend = Some(blend.clone());
            }
        }

        rows.section(&tr!("pile-haulage"));
        haul_point_row(rows, &tr!("haul-link-dump-column"), links.as_ref().map(|links| links.dump), dump_problem);
        haul_point_row(rows, &tr!("haul-link-reclaim-column"), links.as_ref().and_then(|links| links.reclaim), reclaim_problem);
        if rows.action_row(&tr!("haul-open-layout")).clicked() {
            edits.push(UiCommand::EditHaulProperties);
        }
    });
    if let Some(blend) = new_blend {
        edits.push(UiCommand::schedule(session, ScheduleEdit::SetOpeningBlend { destination: entry.id, blend }));
    }
    commands.append(&mut edits);
    if open_calendar {
        editor.open_calendar_cell(crate::ui::state::CalendarCellAddress {
            owner: crate::ui::state::CalendarOwner::Destination(entry.id),
            row: crate::ui::state::CalendarRow::PileMode,
            cell: crate::model::schedule::CalendarCell::Default,
        });
    }

    // A chunked pile's opening chunks beside: the list, oldest first, over
    // the one selected, for a chunk that differs from the rest.
    let beside = egui::Rect::from_min_max(egui::pos2(table_rect.right() + gap, rect.top()), rect.max);
    if chunked && beside.is_positive() {
        let list = egui::Rect::from_min_size(beside.min, egui::vec2(beside.width(), ((beside.height() - gap) * 0.4).max(0.0)));
        let chunk = egui::Rect::from_min_max(egui::pos2(beside.left(), list.bottom() + gap), beside.max);
        draw_opening_lots(ui, list, editor, plan, document, &entry, session, commands);
        draw_lot_editor(ui, chunk, editor, plan, document, &entry, session, commands);
    }
}

/// The rule table: every rule in priority order, with what it matches.
///
/// Compact and ordered, not a graphical builder: the order *is* the resolution,
/// so the one thing the table has to make obvious is which rule comes first.
pub(crate) fn draw_rule_list(ui: &mut egui::Ui, rect: egui::Rect, editor: &mut EditorState, plan: &SchedulePlan, document: &Document, session: u32, commands: &mut Vec<UiCommand>) {
    let routing = plan.routing();
    let mut selected = editor.schedule_selected_rule;
    let mut new_rule = false;
    DataGrid::new("schedule_rule_list", rect, &tr!("destination-destinations"))
        .column_header(&tr!("destination-rule-order"))
        .show(ui, |ui| {
            if routing.rules.is_empty() {
                explorer_note(ui, tr!("destination-no-rules"));
            }
            for (position, rule) in routing.rules.iter().enumerate() {
                // Every destination the rule allows, in the order it tries them.
                let target = rule
                    .destinations
                    .iter()
                    .map(|destination| {
                        destinations::resolve(*destination, document.solids(), routing)
                            .map(|entry| entry.name)
                            .unwrap_or_else(|_| tr!("destination-unresolved"))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let label = format!("{}. {} → {}", (position + 1), rule.name, target);
                let summary = rule.summary(
                    |agent| plan.agent(agent).map(|agent| agent.name.clone()).unwrap_or_else(|| tr!("schedule-error-unknown-agent")),
                    |field| {
                        document
                            .reserve_fields()
                            .iter()
                            .find(|entry| entry.id == field)
                            .map(|entry| entry.name.clone())
                            .unwrap_or_else(|| tr!("destination-stage-field-missing"))
                    },
                );
                // A disabled rule is dimmed rather than hidden: it is part of
                // the order the user is reading, and hiding it would make the
                // numbering jump.
                let disabled = (!rule.enabled).then(|| tr!("destination-rule-disabled"));
                let row = GridRow::new(&label).error(disabled.as_deref());
                let response = grid_row(ui, row.selected(selected == Some(rule.id))).on_hover_text(format!("{}\n{}", label, summary));
                if response.clicked() {
                    selected = Some(rule.id);
                }
                context_menu_popup(&response, &rule.name, |ui| {
                    if ContextMenuAction::new(tr!("destination-move-rule-up")).enabled(position > 0).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::MoveRule { rule: rule.id, later: false }));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("destination-move-rule-down"))
                        .enabled(position + 1 < routing.rules.len())
                        .show(ui)
                        .clicked()
                    {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::MoveRule { rule: rule.id, later: true }));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("destination-duplicate-rule")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DuplicateRule(rule.id)));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("destination-delete-rule")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(session, ScheduleEdit::DeleteRule(rule.id)));
                        ui.close();
                    }
                });
            }
            let body = ui.available_rect_before_wrap();
            if body.is_positive() {
                let response = ui.interact(body, ui.id().with("new_rule_space"), egui::Sense::click());
                context_menu_popup(&response, tr!("destination-destinations"), |ui| {
                    let available = !destinations::available(document.solids(), routing).is_empty();
                    if ContextMenuAction::new(tr!("destination-new-rule")).enabled(available).show(ui).clicked() {
                        new_rule = true;
                        ui.close();
                    }
                });
            }
        });
    if editor.schedule_selected_rule != selected {
        editor.schedule_rule_draft = None;
        editor.schedule_condition_draft = None;
    }
    editor.schedule_selected_rule = selected;
    if new_rule && let Some(first) = destinations::available(document.solids(), routing).first() {
        let name = crate::model::schedule::suggested_name(&tr!("destination-rule-default"), routing.rules.iter().map(|rule| rule.name.clone()));
        commands.push(UiCommand::schedule(
            session,
            ScheduleEdit::AddRule {
                name,
                destinations: vec![first.id],
            },
        ));
    }
}

/// The selected rule's editor: what it delivers to, and the three filter
/// groups that decide what reaches it.
pub(crate) fn draw_rule_editor(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let routing = plan.routing();
    let Some(rule) = editor.schedule_selected_rule.and_then(|id| routing.rule(id)).cloned() else {
        PropertyTable::new("schedule_rule_editor", rect, &tr!("destination-rule")).show(ui, |rows| {
            rows.header(&tr!("planning-property"), &tr!("planning-value"));
            rows.readonly(&tr!("planning-name"), &tr!("destination-select-rule"), None, None);
        });
        return;
    };
    if editor.schedule_rule_draft.as_ref().is_none_or(|draft| draft.id != rule.id || draft.source != rule.name) {
        editor.schedule_rule_draft = Some(ScheduleRuleDraft {
            id: rule.id,
            source: rule.name.clone(),
            name: rule.name.clone(),
        });
    }
    let available = destinations::available(document.solids(), routing);
    let stockpiles: Vec<_> = available.iter().filter(|entry| entry.kind == DestinationKind::Stockpile).cloned().collect();
    let source_views = editor.schedule_routing_sources.clone();
    let categories = editor.schedule_category_values.clone();
    let mut edits = Vec::new();
    let taken: Vec<String> = routing.rules.iter().filter(|other| other.id != rule.id).map(|other| other.name.clone()).collect();
    let draft = editor.schedule_rule_draft.as_mut().expect("just ensured");
    let name_error = name_problem(&draft.name, taken.iter().cloned());
    let mut condition_action = None;
    // Cloned for the frame: the grid holds the draft mutably while it draws, and
    // the folded set is small enough that a clone is cheaper than splitting the
    // editor borrow.
    let mut collapsed = editor.schedule_source_collapsed.clone();
    DataGrid::new("schedule_rule_editor", rect, &rule.name)
        .column_header(&tr!("destination-rule"))
        .show(ui, |ui| {
            grid_separator_row(ui, &tr!("destination-rule"), 0);
            {
                let (cell, _) = crate::ui::widgets::data_grid::grid_named_row(ui, &tr!("planning-name"), 0);
                if cell.is_positive() {
                    let response = ui.put(cell, egui::TextEdit::singleline(&mut draft.name).desired_width(cell.width()));
                    if let Some(message) = &name_error {
                        response.clone().on_hover_text(message);
                    }
                    if response.lost_focus() && name_error.is_none() && draft.name.trim() != rule.name {
                        edits.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::RenameRule {
                                rule: rule.id,
                                name: draft.name.trim().to_owned(),
                            },
                        ));
                    }
                }
            }
            let mut enabled = rule.enabled;
            if crate::ui::widgets::data_grid::grid_checkbox_row(ui, &tr!("destination-rule-enabled"), &mut enabled, 0) {
                edits.push(UiCommand::schedule(session, ScheduleEdit::SetRuleEnabled { rule: rule.id, enabled }));
            }
            // Destinations, tried in the order they are listed. A rule may allow
            // several: "this material may go to ROM A or ROM B" is one decision,
            // and writing it as two identical rules would make it two.
            {
                let names: Vec<String> = rule
                    .destinations
                    .iter()
                    .map(|id| {
                        available
                            .iter()
                            .find(|entry| entry.id == *id)
                            .map(|entry| entry.name.clone())
                            .unwrap_or_else(|| tr!("destination-unresolved"))
                    })
                    .collect();
                let summary = if names.is_empty() { tr!("destination-rule-none") } else { names.join(", ") };
                let response = grid_select_row(ui, ("rule_destinations", rule.id.0), &tr!("destination-rule-target"), &summary, 0);
                let mut chosen = rule.destinations.clone();
                checklist_popup(&response, tr!("destination-rule-target"), 260.0, |ui| {
                    if available.is_empty() {
                        menu::menu_note(ui, tr!("destination-no-destinations"));
                        return;
                    }
                    let all = available.iter().all(|entry| chosen.contains(&entry.id));
                    let any = available.iter().any(|entry| chosen.contains(&entry.id));
                    if ChecklistRow::new(&tr!("destination-select-all"), Tick::of(all, any)).show(ui).toggled && !all {
                        chosen = available.iter().map(|entry| entry.id).collect();
                    }
                    context_menu_separator(ui);
                    for entry in &available {
                        let position = chosen.iter().position(|held| *held == entry.id);
                        // Numbered while selected, because the order they are
                        // listed in is the order they are tried.
                        let label = match position {
                            Some(position) => format!("{}. {} · {}", (position + 1), entry.name, entry.kind.label()),
                            None => format!("{} · {}", entry.name, entry.kind.label()),
                        };
                        if ChecklistRow::new(&label, Tick::of(position.is_some(), false)).depth(1).show(ui).toggled {
                            match position {
                                Some(position) => {
                                    chosen.remove(position);
                                }
                                None => chosen.push(entry.id),
                            }
                        }
                    }
                });
                // An empty list is not a state a rule can hold - it would match
                // material and have nowhere to put it - so the last tick stays
                // until another is put in its place.
                if !chosen.is_empty() && chosen != rule.destinations {
                    edits.push(UiCommand::schedule(
                        session,
                        ScheduleEdit::SetRuleDestinations {
                            rule: rule.id,
                            destinations: chosen,
                        },
                    ));
                }
            }

            // Loaders. "All" is not the same as "every loader currently in the
            // fleet": a machine added tomorrow is covered by All and is not added
            // to a list of names. Turning All off writes out the list it stands
            // for today, which is where editing from it starts.
            grid_separator_row(ui, &tr!("destination-rule-loaders"), 0);
            {
                let all = matches!(rule.loaders, LoaderSelection::All);
                let held: Vec<LoaderAgentId> = match &rule.loaders {
                    LoaderSelection::All => Vec::new(),
                    LoaderSelection::Only(agents) => agents.clone(),
                };
                let summary = if all {
                    tr!("destination-rule-all-loaders")
                } else {
                    held.iter()
                        .map(|agent| plan.agent(*agent).map(|agent| agent.name.clone()).unwrap_or_else(|| tr!("schedule-error-unknown-agent")))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let response = grid_select_row(ui, ("rule_loaders", rule.id.0), &tr!("destination-rule-loaders"), &summary, 0);
                let mut next: Option<LoaderSelection> = None;
                checklist_popup(&response, tr!("destination-rule-loaders"), 240.0, |ui| {
                    if ChecklistRow::new(&tr!("destination-rule-all-loaders"), Tick::of(all, false)).show(ui).toggled {
                        next = Some(if all {
                            LoaderSelection::Only(plan.agents().iter().map(|agent| agent.id).collect())
                        } else {
                            LoaderSelection::All
                        });
                    }
                    context_menu_separator(ui);
                    if plan.agents().is_empty() {
                        menu::menu_note(ui, tr!("destination-rule-no-loaders"));
                    }
                    for agent in plan.agents() {
                        let picked = all || held.contains(&agent.id);
                        if ChecklistRow::new(&agent.name, Tick::of(picked, false)).depth(1).show(ui).toggled {
                            let mut list: Vec<LoaderAgentId> = if all { plan.agents().iter().map(|agent| agent.id).collect() } else { held.clone() };
                            if picked {
                                list.retain(|entry| *entry != agent.id);
                            } else {
                                list.push(agent.id);
                            }
                            // The last tick stays until All is chosen. An empty
                            // list is not "everything": it is a restriction that
                            // matches nothing, and reading it as All would widen
                            // the rule at the moment the user narrowed it most.
                            next = (!list.is_empty()).then_some(LoaderSelection::Only(list));
                        }
                    }
                });
                if let Some(loaders) = next
                    && loaders != rule.loaders
                    && !matches!(&loaders, LoaderSelection::Only(agents) if agents.is_empty())
                {
                    edits.push(UiCommand::schedule(session, ScheduleEdit::SetRuleLoaders { rule: rule.id, loaders }));
                }
            }

            // Sources: the bands the last completed Solids run produced, nested
            // pit → bench → flitch so a whole area can be taken in one tick or
            // folded away, and the stockpiles a reclaim would load from. One
            // widget, shared with the trucking and cashflow rules, so what the
            // three pages call a source cannot drift apart.
            grid_separator_row(ui, &tr!("destination-rule-sources"), 0);
            if let Some(sources) = movement_sources_row(
                ui,
                ("rule_sources", rule.id.0),
                &tr!("destination-rule-sources"),
                &rule.sources,
                &source_views,
                &stockpiles,
                &mut collapsed,
            ) && sources != rule.sources
            {
                edits.push(UiCommand::schedule(session, ScheduleEdit::SetRuleSources { rule: rule.id, sources }));
            }

            // Conditions, ANDed. A field carries at most one, because one interval
            // or value set already expresses anything two on the same field could.
            grid_separator_row(ui, &tr!("destination-rule-conditions"), 0);
            // The note stands where the first condition would, so it offers
            // the same menu as the space below it.
            let note = rule.conditions.is_empty().then(|| explorer_note(ui, tr!("destination-no-conditions")));
            // A category condition the rule's current sources cannot evaluate.
            // Named rather than removed: the planner chose it, and the repair -
            // narrowing the sources, or splitting the rule - is theirs to make.
            category_conflict_note(ui, document, &rule.sources, &rule.conditions);
            for condition in &rule.conditions {
                let field_name = document
                    .reserve_fields()
                    .iter()
                    .find(|field| field.id == condition.field)
                    .map(|field| field.name.clone())
                    .unwrap_or_else(|| tr!("destination-stage-field-missing"));
                let label = condition.summary(&field_name);
                let response = grid_row(ui, GridRow::new(&label)).on_hover_text(condition_note(document, condition));
                context_menu_popup(&response, &label, |ui| {
                    if ContextMenuAction::new(tr!("destination-edit-condition")).show(ui).clicked() {
                        condition_action = Some(ConditionAction::Edit(condition.field));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("destination-delete-condition")).show(ui).clicked() {
                        condition_action = Some(ConditionAction::Delete(condition.field));
                        ui.close();
                    }
                });
            }
            let body = ui.available_rect_before_wrap();
            let body = match &note {
                Some(note) => body.union(note.rect),
                None => body,
            };
            if body.is_positive() {
                let response = ui.interact(body, ui.id().with(("new_condition_space", rule.id.0)), egui::Sense::click());
                context_menu_popup(&response, tr!("destination-rule-conditions"), |ui| {
                    let spare = document
                        .reserve_fields()
                        .iter()
                        .any(|field| !rule.conditions.iter().any(|condition| condition.field == field.id));
                    if ContextMenuAction::new(tr!("destination-add-condition")).enabled(spare).show(ui).clicked() {
                        condition_action = Some(ConditionAction::Add);
                        ui.close();
                    }
                });
            }
        });
    editor.schedule_source_collapsed = collapsed;
    match condition_action {
        None => {}
        Some(ConditionAction::Add) => {
            editor.schedule_condition_draft = Some(ScheduleConditionDraft {
                rule: ConditionOwner::Routing(rule.id),
                replacing: None,
                field: document
                    .reserve_fields()
                    .iter()
                    .find(|field| !rule.conditions.iter().any(|condition| condition.field == field.id))
                    .map(|field| field.id),
                values: Vec::new(),
                lower: String::new(),
                lower_inclusive: false,
                upper: String::new(),
                upper_inclusive: false,
            });
        }
        Some(ConditionAction::Edit(field)) => {
            let condition = rule.conditions.iter().find(|condition| condition.field == field);
            let (values, lower, lower_inclusive, upper, upper_inclusive) = match condition.map(|condition| &condition.test) {
                Some(ConditionTest::Category { values }) => (values.clone(), String::new(), false, String::new(), false),
                Some(ConditionTest::Range { lower, upper }) => (
                    Vec::new(),
                    lower.map(|bound| bound.value.to_string()).unwrap_or_default(),
                    lower.is_some_and(|bound| bound.inclusive),
                    upper.map(|bound| bound.value.to_string()).unwrap_or_default(),
                    upper.is_some_and(|bound| bound.inclusive),
                ),
                None => (Vec::new(), String::new(), false, String::new(), false),
            };
            editor.schedule_condition_draft = Some(ScheduleConditionDraft {
                rule: ConditionOwner::Routing(rule.id),
                replacing: Some(field),
                field: Some(field),
                values,
                lower,
                lower_inclusive,
                upper,
                upper_inclusive,
            });
        }
        Some(ConditionAction::Delete(field)) => {
            let conditions = rule.conditions.iter().filter(|condition| condition.field != field).cloned().collect();
            edits.push(UiCommand::schedule(session, ScheduleEdit::SetRuleConditions { rule: rule.id, conditions }));
        }
    }
    commands.append(&mut edits);
    draw_condition_dialog(ui, editor, plan, document, ConditionOwner::Routing(rule.id), &categories, session, commands);
}

pub(crate) enum ConditionAction {
    Add,
    Edit(crate::model::ReserveFieldId),
    Delete(crate::model::ReserveFieldId),
}

/// Seed the condition dialog for one rule, whichever kind it is.
///
/// Shared so the two rule editors open the same dialog on the same draft
/// rather than each building one.
pub(crate) fn open_condition_draft(editor: &mut EditorState, document: &Document, owner: ConditionOwner, conditions: &[FieldCondition], action: ConditionAction) {
    match action {
        ConditionAction::Delete(_) => {}
        ConditionAction::Add => {
            editor.schedule_condition_draft = Some(ScheduleConditionDraft {
                rule: owner,
                replacing: None,
                field: document
                    .reserve_fields()
                    .iter()
                    .find(|field| !conditions.iter().any(|condition| condition.field == field.id))
                    .map(|field| field.id),
                values: Vec::new(),
                lower: String::new(),
                lower_inclusive: false,
                upper: String::new(),
                upper_inclusive: false,
            });
        }
        ConditionAction::Edit(field) => {
            let condition = conditions.iter().find(|condition| condition.field == field);
            let (values, lower, lower_inclusive, upper, upper_inclusive) = match condition.map(|condition| &condition.test) {
                Some(ConditionTest::Category { values }) => (values.clone(), String::new(), false, String::new(), false),
                Some(ConditionTest::Range { lower, upper }) => (
                    Vec::new(),
                    lower.map(|bound| bound.value.to_string()).unwrap_or_default(),
                    lower.is_some_and(|bound| bound.inclusive),
                    upper.map(|bound| bound.value.to_string()).unwrap_or_default(),
                    upper.is_some_and(|bound| bound.inclusive),
                ),
                None => (Vec::new(), String::new(), false, String::new(), false),
            };
            editor.schedule_condition_draft = Some(ScheduleConditionDraft {
                rule: owner,
                replacing: Some(field),
                field: Some(field),
                values,
                lower,
                lower_inclusive,
                upper,
                upper_inclusive,
            });
        }
    }
}

/// What a condition is read against, stated on demand rather than on the page.
///
/// A summed field's condition compares the *contributing row's own* mapped
/// value, not the tonnes that row contributed - which is the one thing about
/// this that is not obvious from the expression.
pub(crate) fn condition_note(document: &Document, condition: &FieldCondition) -> String {
    let aggregation = document.reserve_fields().iter().find(|field| field.id == condition.field).map(|field| field.aggregation);
    match aggregation {
        Some(ReserveAggregation::Sum) => tr!("destination-condition-note-sum"),
        Some(ReserveAggregation::WeightedAverage { .. } | ReserveAggregation::VolumeAverage) => tr!("destination-condition-note-average"),
        Some(ReserveAggregation::Category) => tr!("destination-condition-note-category"),
        None => tr!("destination-stage-field-missing"),
    }
}

/// The condition editor: one field, and either the values it may hold or the
/// interval it must fall in.
#[allow(
    clippy::too_many_arguments,
    reason = "the dialog's whole input: the page state, the plan and document it edits, the rule on screen, the category values, the session and the command sink"
)]
pub(crate) fn draw_condition_dialog(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    shown: ConditionOwner,
    categories: &std::collections::BTreeMap<crate::model::ReserveFieldId, Vec<String>>,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    // A condition belongs to the rule it was opened from. Once another rule
    // is on screen - picked from the list, or a new one just added - the
    // dialog would otherwise go on editing a rule the page no longer shows.
    if editor.schedule_condition_draft.as_ref().is_some_and(|draft| draft.rule != shown) {
        editor.schedule_condition_draft = None;
    }
    let Some(draft) = editor.schedule_condition_draft.as_mut() else { return };
    let owner = draft.rule;
    // Whichever kind of rule it belongs to, a condition is the same statement
    // about the same field variable, so one dialog writes both.
    let held: Option<Vec<FieldCondition>> = match owner {
        ConditionOwner::Routing(id) => plan.routing().rule(id).map(|rule| rule.conditions.clone()),
        ConditionOwner::Cashflow(id) => plan.cashflow().rule(id).map(|rule| rule.conditions.clone()),
    };
    let Some(existing) = held else {
        editor.schedule_condition_draft = None;
        return;
    };
    // The sources this rule names decide whether a category can be tested at
    // all - the blended pile keeps tonnes and grade and discards categories.
    // The field is offered and the values are shown, so the planner can see
    // what the rule would have said; only committing it is refused, with the
    // reason beside the button rather than after the edit.
    let categories_available = match owner {
        ConditionOwner::Routing(id) => plan.routing().rule(id).is_some_and(|rule| destinations::category_conditions_available(&rule.sources)),
        ConditionOwner::Cashflow(id) => plan.cashflow().rule(id).is_some_and(|rule| destinations::category_conditions_available(&rule.sources)),
    };
    let mut open = true;
    let mut close = false;
    let mut apply = false;
    let field = draft.field.and_then(|id| document.reserve_fields().iter().find(|field| field.id == id).cloned());
    let categorical = field.as_ref().is_some_and(|field| field.aggregation == ReserveAggregation::Category);
    DragableMenu::new("destination_condition_dialog", tr!("destination-condition"))
        .open(&mut open)
        .min_width(360.0)
        .show(ui.ctx(), |ui| {
            let options: Vec<(Option<crate::model::ReserveFieldId>, egui::WidgetText)> = document
                .reserve_fields()
                .iter()
                // A field that already carries a condition on this rule is not
                // offered: two would be ANDed, and one condition can already
                // say whatever both would.
                .filter(|entry| !existing.iter().any(|condition| condition.field == entry.id) || draft.replacing == Some(entry.id))
                .map(|entry| (Some(entry.id), entry.name.clone().into()))
                .collect();
            let selected = field.as_ref().map(|field| field.name.clone()).unwrap_or_else(|| tr!("destination-condition-pick-field"));
            MenuFieldCombo::new("destination_condition_field", tr!("destination-condition-field"), &mut draft.field, selected, options).show(ui);
            if categorical {
                menu::panel_section(ui, tr!("destination-condition-values"));
                let known = draft.field.and_then(|id| categories.get(&id));
                match known {
                    None => menu::menu_note(ui, tr!("destination-condition-no-values")),
                    Some(values) => {
                        for value in values {
                            let mut picked = draft.values.iter().any(|held| held == value);
                            if ui.checkbox(&mut picked, value).changed() {
                                if picked {
                                    draft.values.push(value.clone());
                                } else {
                                    draft.values.retain(|held| held != value);
                                }
                            }
                        }
                    }
                }
                // Selections the models no longer hold, still listed so they can
                // be seen and removed rather than silently narrowing the rule.
                let retained: Vec<String> = draft
                    .values
                    .iter()
                    .filter(|value| !known.is_some_and(|values| values.iter().any(|entry| entry == *value)))
                    .cloned()
                    .collect();
                for value in retained {
                    let mut picked = true;
                    let label = format!("{} ({})", value, tr!("destination-condition-absent"));
                    if ui.checkbox(&mut picked, label).changed() {
                        draft.values.retain(|held| *held != value);
                    }
                }
            } else {
                // Both ends, each with its own inclusivity, so `60 < Fe < 70` is
                // expressible exactly as it was written.
                menu::panel_section(ui, tr!("destination-condition-range"));
                MenuFieldText::new(tr!("destination-condition-lower"), &mut draft.lower)
                    .hint_text(tr!("destination-condition-open"))
                    .show(ui);
                ui.checkbox(&mut draft.lower_inclusive, tr!("destination-condition-lower-inclusive"));
                MenuFieldText::new(tr!("destination-condition-upper"), &mut draft.upper)
                    .hint_text(tr!("destination-condition-open"))
                    .show(ui);
                ui.checkbox(&mut draft.upper_inclusive, tr!("destination-condition-upper-inclusive"));
            }
            let built = build_condition(draft, categorical).and_then(|condition| {
                if categorical && !categories_available {
                    Err(tr!("destination-condition-category-blocked"))
                } else {
                    Ok(condition)
                }
            });
            if let Err(message) = &built {
                ui.label(egui::RichText::new(message).color(ui.visuals().error_fg_color));
            }
            menu::menu_actions(ui, |ui| {
                let submitted = menu::dialog_confirm_pressed(ui.ctx());
                if (submitted || ui.add(MenuButton::new(tr!("common-apply")).primary().enabled(built.is_ok())).clicked()) && built.is_ok() {
                    apply = true;
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    if apply
        && let Some(draft) = editor.schedule_condition_draft.as_ref()
        && let Ok(condition) = build_condition(draft, categorical)
        && (!categorical || categories_available)
    {
        let replacing = draft.replacing;
        let mut conditions = existing.clone();
        match replacing.and_then(|field| conditions.iter().position(|held| held.field == field)) {
            Some(position) => conditions[position] = condition,
            None => conditions.push(condition),
        }
        commands.push(UiCommand::schedule(
            session,
            match owner {
                ConditionOwner::Routing(rule) => ScheduleEdit::SetRuleConditions { rule, conditions },
                ConditionOwner::Cashflow(rule) => ScheduleEdit::SetCashflowRuleConditions { rule, conditions },
            },
        ));
    }
    if close || !open {
        editor.schedule_condition_draft = None;
    }
}

/// Turn the draft into a condition, or say what is wrong with it.
///
/// The same rules the domain applies, checked here so the dialog can refuse
/// before anything is committed rather than reporting after.
pub(crate) fn build_condition(draft: &ScheduleConditionDraft, categorical: bool) -> Result<FieldCondition, String> {
    let Some(field) = draft.field else {
        return Err(tr!("destination-condition-pick-field"));
    };
    let test = if categorical {
        if draft.values.is_empty() {
            return Err(crate::model::schedule::ScheduleError::EmptyCondition.message());
        }
        ConditionTest::Category { values: draft.values.clone() }
    } else {
        let bound = |text: &str, inclusive: bool| -> Result<Option<Bound>, String> {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            let Ok(value) = trimmed.parse::<f64>() else {
                return Err(tr!("schedule-rate-not-a-number"));
            };
            if !value.is_finite() {
                return Err(crate::model::schedule::ScheduleError::InvalidBound.message());
            }
            Ok(Some(Bound { value, inclusive }))
        };
        let lower = bound(&draft.lower, draft.lower_inclusive)?;
        let upper = bound(&draft.upper, draft.upper_inclusive)?;
        if lower.is_none() && upper.is_none() {
            return Err(crate::model::schedule::ScheduleError::EmptyCondition.message());
        }
        if let (Some(lower), Some(upper)) = (lower, upper) {
            let ordered = if lower.inclusive && upper.inclusive {
                lower.value <= upper.value
            } else {
                lower.value < upper.value
            };
            if !ordered {
                return Err(crate::model::schedule::ScheduleError::EmptyInterval.message());
            }
        }
        ConditionTest::Range { lower, upper }
    };
    Ok(FieldCondition { field, test })
}

/// The New Destination dialog. The kind comes from the page it was opened on,
/// so there is nothing to choose but a name.
pub(crate) fn draw_new_destination_dialog(ui: &mut egui::Ui, editor: &mut EditorState, plan: &SchedulePlan, session: u32, commands: &mut Vec<UiCommand>) {
    if !editor.new_destination_open {
        return;
    }
    let kind = editor.new_destination_kind;
    let taken: Vec<String> = plan.routing().standalone.iter().map(|entry| entry.name.clone()).collect();
    let error = name_problem(&editor.new_destination_name, taken.into_iter());
    let mut open = true;
    let mut close = false;
    let mut create = false;
    DragableMenu::new("new_destination_dialog", new_label(kind))
        .open(&mut open)
        .min_width(320.0)
        .show(ui.ctx(), |ui| {
            MenuFieldText::new(tr!("planning-name"), &mut editor.new_destination_name)
                .hint_text(tr!("dialog-rename-field-hint"))
                .show(ui);
            menu::menu_note(ui, tr!("destination-type-fixed", kind = kind.label()));
            if let Some(message) = &error {
                ui.label(egui::RichText::new(message).color(ui.visuals().error_fg_color));
            }
            menu::menu_actions(ui, |ui| {
                let submitted = menu::dialog_confirm_pressed(ui.ctx());
                if (submitted || ui.add(MenuButton::new(tr!("common-create")).primary().enabled(error.is_none())).clicked()) && error.is_none() {
                    create = true;
                    close = true;
                }
                if ui.add(MenuButton::new(tr!("common-cancel"))).clicked() || menu::dialog_cancel_pressed(ui.ctx()) {
                    close = true;
                }
            });
        });
    if create {
        commands.push(UiCommand::schedule(
            session,
            ScheduleEdit::AddDestination {
                name: editor.new_destination_name.trim().to_owned(),
                kind,
            },
        ));
    }
    if close || !open {
        editor.new_destination_open = false;
        editor.new_destination_name.clear();
    }
}

fn suggested_destination_name(plan: &SchedulePlan, document: &Document, kind: DestinationKind) -> String {
    let taken = destinations::available(document.solids(), plan.routing()).into_iter().map(|entry| entry.name);
    crate::model::schedule::suggested_name(&default_name(kind), taken)
}

fn default_name(kind: DestinationKind) -> String {
    match kind {
        DestinationKind::Stockpile => tr!("destination-default-stockpile"),
        DestinationKind::Dump => tr!("destination-default-dump"),
        DestinationKind::Crusher => tr!("destination-default-crusher"),
    }
}

fn new_label(kind: DestinationKind) -> String {
    match kind {
        DestinationKind::Stockpile => tr!("planning-new-stockpile"),
        DestinationKind::Dump => tr!("planning-new-dump"),
        DestinationKind::Crusher => tr!("destination-new-crusher"),
    }
}

/// The destination lists' Name and Capacity (or Daily limit) shares.
const LIST_FRACTIONS: [f32; 2] = [0.6, 0.4];

fn kind_page_title(kind: DestinationKind) -> String {
    match kind {
        DestinationKind::Stockpile => tr!("planning-stockpiles"),
        DestinationKind::Dump => tr!("planning-dumps"),
        DestinationKind::Crusher => tr!("destination-crushers"),
    }
}

fn list_id(kind: DestinationKind) -> &'static str {
    match kind {
        DestinationKind::Stockpile => "schedule_stockpile_list",
        DestinationKind::Dump => "schedule_dump_list",
        DestinationKind::Crusher => "schedule_crusher_list",
    }
}

/// The standalone half of a destination id, for the crusher lookups that only
/// apply to one. A solid-backed destination has no crusher calendar, and the
/// sentinel it produces matches nothing.
fn standalone_of(id: DestinationId) -> StandaloneDestinationId {
    match id {
        DestinationId::Standalone(id) => id,
        DestinationId::Solid(_) => StandaloneDestinationId(u64::MAX),
    }
}

fn tonnes(value: f64) -> String {
    format!("{} t", super::schedule_calendar::format_tonnes(value))
}

fn tonnes_per_day(value: f64) -> String {
    tr!("schedule-tonnes-per-day", value = (super::schedule_calendar::format_tonnes(value)).to_string())
}

/// Whether a source row is shown: every group above it is open.
///
/// Ancestry is read off the depths, which is how the flat list the run produces
/// describes its nesting - the nearest row above with a smaller depth is the
/// parent.
/// The one movement-source selector, shared by the destination, trucking and
/// cashflow rules.
///
/// Ground nested pit → bench → flitch, then the stockpiles a reclaim would load
/// from, then whatever the rule holds that the current run cannot place - kept
/// and named rather than dropped, because a rule that quietly stopped
/// restricting would move material somewhere nobody chose.
///
/// `All` is explicit throughout. Turning it off writes out the list it stands
/// for today, which is where editing from it starts; unticking the last entry
/// does nothing, because an empty list matches nothing and reading it as All
/// would widen the rule at the moment the user narrowed it most. Returns the
/// new selection, or `None` when nothing was chosen this frame.
pub(crate) fn movement_sources_row(
    ui: &mut egui::Ui,
    id: (&'static str, u64),
    label: &str,
    held: &MovementSourceSelection,
    sources: &[SourceScopeView],
    stockpiles: &[destinations::DestinationView],
    collapsed: &mut Vec<(u64, u64, u64)>,
) -> Option<MovementSourceSelection> {
    let all = matches!(held, MovementSourceSelection::All);
    let chosen: Vec<MovementSourceScope> = match held {
        MovementSourceSelection::All => Vec::new(),
        MovementSourceSelection::Only(scopes) => scopes.clone(),
    };
    let summary = if all {
        tr!("destination-rule-all-sources")
    } else {
        chosen
            .iter()
            .map(|scope| match scope {
                MovementSourceScope::Ground(ground) => sources
                    .iter()
                    .find(|view| view.scope == *ground)
                    .map(|view| view.label.clone())
                    .unwrap_or_else(|| tr!("destination-source-unplaced")),
                MovementSourceScope::Stockpile(stockpile) => stockpiles
                    .iter()
                    .find(|entry| entry.id == *stockpile)
                    .map(|entry| entry.name.clone())
                    .unwrap_or_else(|| tr!("destination-unresolved")),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    // The pits and the piles, not every band of them: a whole pit is what All
    // stood for, said as scopes.
    let everything = || -> Vec<MovementSourceScope> {
        sources
            .iter()
            .filter(|view| view.depth == 0)
            .map(|view| MovementSourceScope::Ground(view.scope))
            .chain(stockpiles.iter().map(|entry| MovementSourceScope::Stockpile(entry.id)))
            .collect()
    };
    let toggle = |scope: MovementSourceScope, picked: bool| -> Option<MovementSourceSelection> {
        let mut list = if all { everything() } else { chosen.clone() };
        if picked {
            list.retain(|entry| *entry != scope);
        } else {
            list.push(scope);
        }
        (!list.is_empty()).then_some(MovementSourceSelection::Only(list))
    };
    let response = grid_select_row(ui, id, label, &summary, 0);
    let mut next: Option<MovementSourceSelection> = None;
    checklist_popup(&response, label.to_owned(), 260.0, |ui| {
        if ChecklistRow::new(&tr!("destination-rule-all-sources"), Tick::of(all, false)).show(ui).toggled {
            next = Some(if all {
                match everything() {
                    scopes if scopes.is_empty() => MovementSourceSelection::All,
                    scopes => MovementSourceSelection::Only(scopes),
                }
            } else {
                MovementSourceSelection::All
            });
        }
        context_menu_separator(ui);
        menu::panel_section(ui, tr!("truck-rule-ground"));
        if sources.is_empty() {
            menu::menu_note(ui, tr!("destination-no-sources"));
        }
        let ground: Vec<SourceScope> = chosen.iter().filter_map(|scope| scope.ground()).collect();
        for (index, view) in sources.iter().enumerate() {
            if !visible(sources, index, collapsed) {
                continue;
            }
            let scope = MovementSourceScope::Ground(view.scope);
            let children = sources.get(index + 1).is_some_and(|below| below.depth > view.depth);
            let picked = all || chosen.contains(&scope);
            let descendant = !picked && descendant_selected(sources, index, &ground);
            let acted = ChecklistRow::new(&view.label, Tick::of(picked, descendant))
                .depth(view.depth)
                .disclosure(children.then(|| !collapsed.contains(&scope_key(view.scope))))
                .show(ui);
            if acted.expanded {
                let key = scope_key(view.scope);
                match collapsed.iter().position(|folded| *folded == key) {
                    Some(position) => {
                        collapsed.remove(position);
                    }
                    None => collapsed.push(key),
                }
            }
            if acted.toggled {
                next = toggle(scope, picked);
            }
        }
        context_menu_separator(ui);
        menu::panel_section(ui, tr!("truck-rule-stockpiles"));
        if stockpiles.is_empty() {
            menu::menu_note(ui, tr!("destination-no-stockpiles"));
        }
        for entry in stockpiles {
            let scope = MovementSourceScope::Stockpile(entry.id);
            let picked = all || chosen.contains(&scope);
            if ChecklistRow::new(&entry.name, Tick::of(picked, false)).depth(1).show(ui).toggled {
                next = toggle(scope, picked);
            }
        }
        // What this rule holds that the run cannot place, or that names a
        // destination the project no longer has.
        let unresolved: Vec<MovementSourceScope> = chosen
            .iter()
            .copied()
            .filter(|scope| match scope {
                MovementSourceScope::Ground(ground) => !sources.iter().any(|view| view.scope == *ground),
                MovementSourceScope::Stockpile(id) => !stockpiles.iter().any(|entry| entry.id == *id),
            })
            .collect();
        if !unresolved.is_empty() {
            context_menu_separator(ui);
        }
        for scope in unresolved {
            let label = match scope {
                MovementSourceScope::Ground(_) => tr!("destination-source-unplaced"),
                MovementSourceScope::Stockpile(_) => tr!("destination-unresolved"),
            };
            if ChecklistRow::new(&label, Tick::On).depth(1).show(ui).toggled {
                next = toggle(scope, true);
            }
        }
    });
    next
}

pub(crate) fn visible(sources: &[SourceScopeView], index: usize, collapsed: &[(u64, u64, u64)]) -> bool {
    let mut depth = sources[index].depth;
    for above in sources[..index].iter().rev() {
        if above.depth < depth {
            if collapsed.contains(&scope_key(above.scope)) {
                return false;
            }
            depth = above.depth;
            if depth == 0 {
                break;
            }
        }
    }
    true
}

/// Whether anything nested under this row is selected, which is what puts a
/// folded group's box in its mixed state rather than leaving it empty.
pub(crate) fn descendant_selected(sources: &[SourceScopeView], index: usize, held: &[SourceScope]) -> bool {
    let depth = sources[index].depth;
    sources[index + 1..].iter().take_while(|below| below.depth > depth).any(|below| held.contains(&below.scope))
}

/// A folded group's key. A pit carries no band, and a band's base is always
/// below its top, so the two can never collide.
pub(crate) fn scope_key(scope: SourceScope) -> (u64, u64, u64) {
    let bits = |value: f64| if value == 0.0 { 0.0_f64.to_bits() } else { value.to_bits() };
    match scope {
        SourceScope::Pit(solid) => (solid.0, 0, 0),
        SourceScope::Bench { solid, base, top } | SourceScope::Flitch { solid, base, top } => (solid.0, bits(base), bits(top)),
    }
}

/// The tracked grades an opening chunk has no value for, by name.
fn missing_grades(plan: &SchedulePlan, document: &Document, lot: &crate::model::schedule::inventory::OpeningLot) -> Vec<String> {
    plan.experiment()
        .grades
        .iter()
        .filter_map(|(field, _)| document.reserve_fields().iter().find(|known| known.id == *field))
        .filter(|field| {
            lot.portions
                .iter()
                .any(|portion| !matches!(portion.value(field.id), Some(crate::model::schedule::OpeningValue::Number(value)) if value.is_finite()))
        })
        .map(|field| field.name.clone())
        .collect()
}

/// What the stockpile opens holding: its chunks, oldest first, each with
/// its tonnes and a mark where it lacks a grade the schedule tracks.
#[allow(clippy::too_many_arguments)]
fn draw_opening_lots(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    destination: &destinations::DestinationView,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let inventory = plan.routing().inventory(destination.id).cloned().unwrap_or_default();
    let mut selected = editor.schedule_selected_lot;
    let columns = [(tr!("inventory-chunk-column"), LIST_FRACTIONS[0]), (tr!("inventory-lot-tonnes"), LIST_FRACTIONS[1])];
    let total = tonnes(destination.opening_t);
    DataGrid::new("schedule_opening_lots", rect, &tr!("inventory-opening"))
        .title_detail(&total)
        .columns(&columns)
        .show(ui, |ui| {
            if inventory.lots.is_empty() {
                explorer_note(ui, tr!("inventory-no-lots"));
            }
            let last = inventory.lots.len().saturating_sub(1);
            for (position, lot) in inventory.lots.iter().enumerate() {
                let (response, cells) = grid_columns_row(ui, &LIST_FRACTIONS, &[&lot.name, &tonnes(lot.tonnes())], selected == Some(lot.id));
                let missing = missing_grades(plan, document, lot);
                if let (false, Some(cell)) = (missing.is_empty(), cells.last()) {
                    let message = missing
                        .iter()
                        .map(|grade| tr!("pile-chunk-grade-missing", chunk = lot.name.clone(), grade = grade.clone()))
                        .collect::<Vec<_>>()
                        .join("\n");
                    grid_cell_warning(ui, ("opening_chunk_warning", lot.id.0), *cell, &message);
                }
                if response.clicked() {
                    selected = Some(lot.id);
                }
                context_menu_popup(&response, &lot.name, |ui| {
                    if ContextMenuAction::new(tr!("inventory-duplicate-lot")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::DuplicateOpeningLot {
                                destination: destination.id,
                                lot: lot.id,
                            },
                        ));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("inventory-delete-lot")).show(ui).clicked() {
                        commands.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::DeleteOpeningLot {
                                destination: destination.id,
                                lot: lot.id,
                            },
                        ));
                        ui.close();
                    }
                    context_menu_separator(ui);
                    if ContextMenuAction::new(tr!("inventory-move-lot-older")).enabled(position > 0).show(ui).clicked() {
                        commands.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::MoveOpeningLot {
                                destination: destination.id,
                                lot: lot.id,
                                newer: false,
                            },
                        ));
                        ui.close();
                    }
                    if ContextMenuAction::new(tr!("inventory-move-lot-newer")).enabled(position < last).show(ui).clicked() {
                        commands.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::MoveOpeningLot {
                                destination: destination.id,
                                lot: lot.id,
                                newer: true,
                            },
                        ));
                        ui.close();
                    }
                });
            }
            if grid_add_action_row(ui, &tr!("inventory-new-lot")) {
                let name = crate::model::schedule::suggested_name(&tr!("inventory-default-lot-name"), inventory.lots.iter().map(|lot| lot.name.clone()));
                commands.push(UiCommand::schedule(
                    session,
                    ScheduleEdit::AddOpeningLot {
                        destination: destination.id,
                        name,
                        tonnes_t: DEFAULT_LOT_TONNES,
                    },
                ));
            }
        });
    if editor.schedule_selected_lot != selected {
        editor.schedule_lot_draft = None;
    }
    editor.schedule_selected_lot = selected;
}

/// What a new lot is created holding, so it is stock rather than an empty row
/// the user has to fill in before it means anything.
const DEFAULT_LOT_TONNES: f64 = 1000.0;

/// The selected opening chunk: its name, and each portion's tonnes and
/// property values. A chunk of one portion shows that portion's rows
/// directly; one of several shows its total, then each portion in turn.
///
/// A portion's tonnes are asked once, here. The nominated tonnage field is not
/// offered as a property - it would be the same number entered twice, free to
/// disagree with itself.
#[allow(clippy::too_many_arguments)]
fn draw_lot_editor(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    editor: &mut EditorState,
    plan: &SchedulePlan,
    document: &Document,
    destination: &destinations::DestinationView,
    session: u32,
    commands: &mut Vec<UiCommand>,
) {
    let tracked: Vec<_> = plan.experiment().grades.iter().map(|(field, _)| *field).collect();
    let inventory = plan.routing().inventory(destination.id).cloned().unwrap_or_default();
    let Some(lot) = editor.schedule_selected_lot.and_then(|id| inventory.lot(id)).cloned() else {
        DataGrid::new("schedule_lot_editor", rect, &tr!("inventory-lot")).show(ui, |ui| {
            explorer_note(ui, tr!("inventory-select-lot"));
        });
        return;
    };
    // The tonnage field is supplied by the portion's own tonnes, so it is not
    // offered as a property to enter a second time.
    let tonnage_field = plan.tonnage_field();
    let mut fields: Vec<_> = document
        .reserve_fields()
        .iter()
        .filter(|field| Some(field.id) != tonnage_field)
        .map(|field| (field.id, field.name.clone(), field.aggregation == ReserveAggregation::Category))
        .collect();
    for (field, value) in lot.portions.iter().flat_map(|portion| &portion.values) {
        if Some(*field) != tonnage_field && !fields.iter().any(|(id, _, _)| id == field) {
            fields.push((
                *field,
                tr!("inventory-field-missing", id = field.0.to_string()),
                matches!(value, crate::model::schedule::OpeningValue::Category(_)),
            ));
        }
    }
    let categories = editor.schedule_category_values.clone();
    let source = format!("{lot:?}");
    if editor
        .schedule_lot_draft
        .as_ref()
        .is_none_or(|draft| draft.destination != destination.id || draft.lot != lot.id || draft.source != source)
    {
        editor.schedule_lot_draft = Some(crate::ui::state::ScheduleLotDraft {
            destination: destination.id,
            lot: lot.id,
            source,
            name: lot.name.clone(),
            portions: lot.portions.iter().map(|portion| (portion.id, portion.tonnes_t.to_string())).collect(),
            values: lot
                .portions
                .iter()
                .flat_map(|portion| {
                    portion.values.iter().filter_map(move |(field, value)| match value {
                        crate::model::schedule::OpeningValue::Number(number) => Some((portion.id, *field, number.to_string())),
                        crate::model::schedule::OpeningValue::Category(_) => None,
                    })
                })
                .collect(),
        });
    }
    let taken: Vec<String> = inventory.lots.iter().filter(|other| other.id != lot.id).map(|other| other.name.clone()).collect();
    let draft = editor.schedule_lot_draft.as_mut().expect("just ensured");
    let name_error = name_problem(&draft.name, taken.iter().cloned());
    let mut edits = Vec::new();
    let mut portion_action = None;
    let several = lot.portions.len() > 1;
    let depth = usize::from(several);
    DataGrid::new("schedule_lot_editor", rect, &tr!("inventory-lot")).title_detail(&lot.name).show(ui, |ui| {
        {
            let (cell, _) = grid_named_row(ui, &tr!("inventory-lot-name"), 0);
            if cell.is_positive() {
                let response = ui.put(cell, egui::TextEdit::singleline(&mut draft.name).desired_width(cell.width()));
                if let Some(message) = &name_error {
                    response.clone().on_hover_text(message);
                }
                if response.lost_focus() && name_error.is_none() && draft.name.trim() != lot.name {
                    edits.push(UiCommand::schedule(
                        session,
                        ScheduleEdit::RenameOpeningLot {
                            destination: destination.id,
                            lot: lot.id,
                            name: draft.name.trim().to_owned(),
                        },
                    ));
                }
            }
        }
        if several {
            let (cell, _) = grid_named_row(ui, &tr!("inventory-lot-tonnes"), 0);
            if cell.is_positive() {
                grid_cell_fixed(ui, cell, &tonnes(lot.tonnes()));
            }
        }

        for (index, portion) in lot.portions.iter().enumerate() {
            let title = format!("{} {}", tr!("inventory-portion"), (index + 1));
            if several {
                grid_separator_row(ui, &title, 0);
            }
            {
                let (cell, response) = grid_named_row(ui, &tr!("inventory-lot-tonnes"), depth);
                context_menu_popup(&response, &title, |ui| {
                    if ContextMenuAction::new(tr!("inventory-new-portion")).show(ui).clicked() {
                        portion_action = Some(PortionAction::Add);
                        ui.close();
                    }
                    // The last portion is refused rather than hidden: a lot
                    // with nothing in it is not stock, and deleting the lot
                    // is the edit that says so.
                    if ContextMenuAction::new(tr!("inventory-delete-portion")).enabled(lot.portions.len() > 1).show(ui).clicked() {
                        portion_action = Some(PortionAction::Delete(portion.id));
                        ui.close();
                    }
                });
                if cell.is_positive() {
                    let text = draft
                        .portions
                        .iter_mut()
                        .find(|(id, _)| *id == portion.id)
                        .map(|(_, text)| text)
                        .expect("the draft holds every portion");
                    let error = parse_lot_tonnes(text).err();
                    let response = ui.put(cell, egui::TextEdit::singleline(text).desired_width(cell.width()));
                    if let Some(message) = &error {
                        response.clone().on_hover_text(message);
                    }
                    if response.lost_focus()
                        && let Ok(tonnes_t) = parse_lot_tonnes(text)
                        && tonnes_t != portion.tonnes_t
                    {
                        edits.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::SetOpeningPortionTonnes {
                                destination: destination.id,
                                lot: lot.id,
                                portion: portion.id,
                                tonnes_t,
                            },
                        ));
                    }
                }
            }
            for (field, name, categorical) in &fields {
                let held = portion.value(*field);
                let compatible = held.is_none_or(|value| matches!(value, crate::model::schedule::OpeningValue::Category(_)) == *categorical);
                let display_name = if compatible {
                    name.clone()
                } else {
                    tr!("inventory-field-incompatible", field = name.clone())
                };
                if *categorical {
                    // The values the models were measured holding, plus
                    // whatever this portion already says: a label that no
                    // longer occurs is kept rather than quietly cleared.
                    let mut options: Vec<Option<String>> = vec![None];
                    options.extend(categories.get(field).into_iter().flatten().cloned().map(Some));
                    if let Some(crate::model::schedule::OpeningValue::Category(label)) = &held
                        && !options.contains(&Some(label.clone()))
                    {
                        options.push(Some(label.clone()));
                    }
                    let mut chosen = match &held {
                        Some(crate::model::schedule::OpeningValue::Category(label)) => Some(label.clone()),
                        _ => None,
                    };
                    let before = chosen.clone();
                    let label = |value: &Option<String>| value.clone().unwrap_or_else(|| tr!("inventory-portion-missing"));
                    let (cell, response) = grid_named_row(ui, &display_name, depth);
                    if !compatible {
                        response.on_hover_text(tr!("inventory-field-incompatible-note"));
                    }
                    if cell.is_positive() {
                        let id = egui::Id::new(("lot_category", portion.id.0, field.0));
                        let selected = label(&chosen);
                        ui.scope_builder(egui::UiBuilder::new().id_salt(id).max_rect(cell), |ui| {
                            ui.set_clip_rect(ui.clip_rect().intersect(cell));
                            ui.spacing_mut().interact_size.y = cell.height();
                            egui::ComboBox::from_id_salt(id.with("combo"))
                                .selected_text(selected)
                                .width(cell.width())
                                .truncate()
                                .show_ui(ui, |ui| {
                                    for option in &options {
                                        ui.selectable_value(&mut chosen, option.clone(), label(option));
                                    }
                                });
                        });
                    }
                    if chosen != before {
                        edits.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::SetOpeningPortionValue {
                                destination: destination.id,
                                lot: lot.id,
                                portion: portion.id,
                                field: *field,
                                value: chosen.map(crate::model::schedule::OpeningValue::Category),
                            },
                        ));
                    }
                    continue;
                }
                let (cell, response) = grid_named_row(ui, &display_name, depth);
                if !compatible {
                    response.on_hover_text(tr!("inventory-field-incompatible-note"));
                }
                if !cell.is_positive() {
                    continue;
                }
                let position = draft.values.iter().position(|(id, held, _)| *id == portion.id && held == field);
                let position = match position {
                    Some(position) => position,
                    None => {
                        draft.values.push((portion.id, *field, String::new()));
                        draft.values.len() - 1
                    }
                };
                let text = &mut draft.values[position].2;
                let error = parse_lot_value(text).err();
                // A grade the schedule tracks cannot be left blank: marked
                // here, where it is typed, as well as on the step.
                let mut cell = cell;
                if tracked.contains(field) && held.is_none() {
                    grid_cell_warning(ui, ("opening_grade_needed", portion.id.0, field.0), cell, &tr!("pile-grade-needed"));
                    cell.max.x -= CELL_WARNING_WIDTH;
                }
                let response = ui.put(cell, egui::TextEdit::singleline(text).desired_width(cell.width()));
                if let Some(message) = &error {
                    response.clone().on_hover_text(message);
                }
                if response.lost_focus()
                    && let Ok(value) = parse_lot_value(text)
                {
                    let current = match &held {
                        Some(crate::model::schedule::OpeningValue::Number(number)) => Some(*number),
                        _ => None,
                    };
                    if value != current {
                        edits.push(UiCommand::schedule(
                            session,
                            ScheduleEdit::SetOpeningPortionValue {
                                destination: destination.id,
                                lot: lot.id,
                                portion: portion.id,
                                field: *field,
                                value: value.map(crate::model::schedule::OpeningValue::Number),
                            },
                        ));
                    }
                }
            }
        }
        let body = ui.available_rect_before_wrap();
        if body.is_positive() {
            let response = ui.interact(body, ui.id().with(("new_portion_space", lot.id.0)), egui::Sense::click());
            context_menu_popup(&response, tr!("inventory-portions"), |ui| {
                if ContextMenuAction::new(tr!("inventory-new-portion")).show(ui).clicked() {
                    portion_action = Some(PortionAction::Add);
                    ui.close();
                }
            });
        }
    });
    match portion_action {
        Some(PortionAction::Add) => edits.push(UiCommand::schedule(
            session,
            ScheduleEdit::AddOpeningPortion {
                destination: destination.id,
                lot: lot.id,
                tonnes_t: DEFAULT_LOT_TONNES,
            },
        )),
        Some(PortionAction::Delete(portion)) => edits.push(UiCommand::schedule(
            session,
            ScheduleEdit::DeleteOpeningPortion {
                destination: destination.id,
                lot: lot.id,
                portion,
            },
        )),
        None => {}
    }
    commands.append(&mut edits);
}

enum PortionAction {
    Add,
    Delete(crate::model::schedule::OpeningPortionId),
}

/// A portion's tonnes: finite and above zero. Blank is not an answer here -
/// a portion of nothing is not a portion.
fn parse_lot_tonnes(text: &str) -> Result<f64, String> {
    let trimmed = text.trim();
    let Ok(value) = trimmed.parse::<f64>() else {
        return Err(tr!("schedule-calendar-invalid-number"));
    };
    if !value.is_finite() || value <= 0.0 {
        return Err(crate::model::schedule::ScheduleError::InvalidLotTonnes.message());
    }
    Ok(value)
}

/// One numerical property value, or `None` for a field this portion was never
/// measured against - which fails every condition and is not zero.
fn parse_lot_value(text: &str) -> Result<Option<f64>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let Ok(value) = trimmed.parse::<f64>() else {
        return Err(tr!("schedule-calendar-invalid-number"));
    };
    if !value.is_finite() {
        return Err(crate::model::schedule::ScheduleError::InvalidLotValue.message());
    }
    Ok(Some(value))
}

/// Report the category conditions a rule's sources cannot evaluate.
///
/// Draws nothing when the rule is consistent. When it is not, the rule is
/// marked invalid *here*, beside the conditions, with the offending fields
/// named - the one place a planner is already looking at both halves of the
/// contradiction. Shared by the destination and cashflow rule editors so the
/// two pages cannot describe the same policy differently.
pub(crate) fn category_conflict_note(ui: &mut egui::Ui, document: &Document, sources: &MovementSourceSelection, conditions: &[FieldCondition]) {
    let conflicts = destinations::conflicting_category_conditions(sources, conditions);
    if conflicts.is_empty() {
        return;
    }
    let fields = conflicts
        .iter()
        .map(|field| {
            document
                .reserve_fields()
                .iter()
                .find(|entry| entry.id == *field)
                .map(|entry| entry.name.clone())
                .unwrap_or_else(|| tr!("destination-stage-field-missing"))
        })
        .collect::<Vec<_>>()
        .join(", ");
    explorer_note(ui, tr!("destination-rule-category-conflict", fields = fields));
}
