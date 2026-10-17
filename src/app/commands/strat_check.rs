//! The strat column check: the order most holes give a categorical field's
//! codes, worked out when a set is imported and on demand, and the holes
//! that disagree.

use std::sync::Arc;

use crate::{
    app::App,
    i18n::tr,
    model::{
        drill_hole::{DrillFieldKind, DrillHoleDataset, DrillHoleId, WorkingSection, tidy_working_sections},
        strat_order::{HoleFlag, MajorityOrder, default_strat_field, flags_against, majority_order, seeded_sections},
    },
    ui::state::StratCheckReport,
    userspace_log, userspace_warn,
};

/// Sets with up to this many intervals are checked on the spot; larger ones
/// go to the job queue so the window keeps drawing.
const INLINE_CHECK_INTERVALS: usize = 20_000;

/// What a check found, worked out off the UI thread for a large set.
struct StratCheckOutcome {
    majority: Option<MajorityOrder>,
    /// The holes that disagree with the column as it stood; empty when the
    /// column was empty or already the majority order.
    column_flags: Vec<HoleFlag>,
}

/// The majority order of `field` and the holes that disagree with it and
/// with `column`. Logs nothing: it may run on a worker thread.
fn run_strat_check(dataset: &DrillHoleDataset, field: &str, column: &[String]) -> StratCheckOutcome {
    let majority = majority_order(dataset, field);
    let same = majority.as_ref().is_some_and(|found| found.order == column);
    let column_flags = if column.is_empty() || same { Vec::new() } else { flags_against(dataset, field, column) };
    StratCheckOutcome { majority, column_flags }
}

/// How many holes `flags` name; they come sorted by hole.
fn flagged_holes(flags: &[HoleFlag]) -> usize {
    let mut holes = flags.iter().map(|flag| flag.hole).collect::<Vec<_>>();
    holes.dedup();
    holes.len()
}

/// What an import works out for its new set: the field it orders, the order
/// found with the holes that disagree, one working section per group of the
/// order, already tidied, the field the strat column reads until one is
/// picked, and the check of the field the Column tab opens on when that is
/// another field.
pub(crate) struct ImportedStrat {
    field: String,
    found: MajorityOrder,
    sections: Vec<WorkingSection>,
    recorded: String,
    shown: Option<ShownCheck>,
}

/// The check of the field the Column tab opens on, against the column the
/// import gives it.
struct ShownCheck {
    field: String,
    column: Vec<String>,
    outcome: StratCheckOutcome,
}

impl ShownCheck {
    /// The holes that disagree with the column, as the Column tab counts.
    fn flags(&self) -> &[HoleFlag] {
        match &self.outcome.majority {
            Some(majority) if self.column.is_empty() || self.column == majority.order => &majority.flags,
            Some(_) => &self.outcome.column_flags,
            None => &[],
        }
    }
}

impl ImportedStrat {
    /// The field the import's count is for, the one the Column tab opens on,
    /// and how many holes disagree on it.
    fn counted(&self) -> (&str, usize) {
        match &self.shown {
            Some(shown) => (&shown.field, flagged_holes(shown.flags())),
            None => (&self.field, flagged_holes(&self.found.flags)),
        }
    }
}

/// The strat column a freshly imported `dataset` starts with, on the most
/// detailed field, and the check of the field the Column tab opens on, the
/// parent when one is found. Pure and silent, for the import's worker;
/// `None` when there is no field to order.
pub(crate) fn imported_strat(dataset: &DrillHoleDataset) -> Option<ImportedStrat> {
    let field = default_strat_field(&dataset.fields, &[], None)?.key.clone();
    let found = majority_order(dataset, &field)?;
    if found.order.is_empty() {
        return None;
    }
    let (sections, _) = tidy_working_sections(seeded_sections(&found, &field), &dataset.fields);
    let recorded = found.parent.as_ref().map_or_else(|| field.clone(), |parent| parent.field.clone());
    let shown = default_strat_field(&dataset.fields, &sections, Some(&recorded))
        .filter(|shown| shown.key != field)
        .map(|shown| {
            let column = if found.parent.as_ref().is_some_and(|parent| parent.field == shown.key) {
                parent_column(&found)
            } else {
                Vec::new()
            };
            let outcome = run_strat_check(dataset, &shown.key, &column);
            ShownCheck {
                field: shown.key.clone(),
                column,
                outcome,
            }
        });
    Some(ImportedStrat {
        field,
        found,
        sections,
        recorded,
        shown,
    })
}

/// The parent's column an import writes: its named values in group order.
fn parent_column(found: &MajorityOrder) -> Vec<String> {
    found.groups.iter().filter_map(|group| group.name.clone()).collect()
}

impl<'a> App<'a> {
    /// Write an import's strat column and seeded sections on the set it just
    /// added, `id`, and say so in one line. Only the import paths call this:
    /// an opened project keeps the column it was saved with, empty or not.
    /// No undo step: the import added the set as new and unsaved.
    pub(crate) fn fill_imported_strat(&mut self, id: DrillHoleId, strat: Option<ImportedStrat>) {
        let Some(strat) = strat else {
            return;
        };
        let (counted, flagged) = strat.counted();
        let counted = counted.to_owned();
        let ImportedStrat {
            field,
            found,
            sections,
            recorded,
            shown,
        } = strat;
        let Some(open) = self.drill_holes.iter_mut().find(|item| item.id == id) else {
            return;
        };
        if !open.color.strat_column(&field).is_empty() {
            return;
        }
        open.color.set_strat_column(&field, found.order.clone());
        if !open.color.working_sections.iter().any(|section| section.field == field) {
            open.color.working_sections.extend(sections);
        }
        // The parent gets the order of its named values as its own column,
        // and becomes the field the strat column reads until one is picked.
        if let Some(parent) = &found.parent
            && open.color.strat_column(&parent.field).is_empty()
        {
            open.color.set_strat_column(&parent.field, parent_column(&found));
        }
        open.color.strat_field = Some(recorded);
        let checked = Arc::downgrade(&open.dataset);
        // Counted on the field the Column tab opens on, so the number here is
        // the one the tab shows.
        let message = if found.groups.is_empty() {
            tr!(
                "cmd-strat-import-filled",
                field = field.clone(),
                names = found.order.len().to_string(),
                flagged = flagged.to_string(),
                checked = counted.to_string()
            )
        } else {
            tr!(
                "cmd-strat-import-filled-groups",
                field = field.clone(),
                names = found.order.len().to_string(),
                groups = found.groups.len().to_string(),
                flagged = flagged.to_string(),
                checked = counted.to_string()
            )
        };
        // Disagreeing holes are worth a look, so they stand out in the console.
        if flagged > 0 {
            userspace_warn!("{}", message);
        } else {
            userspace_log!("{}", message);
        }
        if let Some(ShownCheck { field, column, outcome }) = shown {
            let StratCheckOutcome { majority, column_flags } = outcome;
            let (order, order_flags, overruled, holes) = match majority {
                Some(majority) => (Some(majority.order), majority.flags, majority.overruled, majority.holes),
                None => (None, Vec::new(), 0, 0),
            };
            self.editor.keep_strat_check(StratCheckReport {
                dataset: id,
                field,
                checked: checked.clone(),
                order,
                order_flags,
                column,
                column_flags,
                overruled,
                holes,
                comparing: false,
            });
        }
        self.editor.keep_strat_check(StratCheckReport {
            dataset: id,
            field,
            checked,
            column: found.order.clone(),
            order: Some(found.order),
            order_flags: found.flags,
            column_flags: Vec::new(),
            overruled: found.overruled,
            holes: found.holes,
            comparing: false,
        });
    }

    /// Check `field` of set `id`: on the spot for a small set, through the
    /// job queue for a large one. An empty column is filled with the order
    /// found, one undo step; a different column is left as it is, and the
    /// differences wait on the geologist's yes.
    pub(crate) fn check_strat_column(&mut self, id: DrillHoleId, field: String) {
        let Some(open) = self.drill_holes.iter().find(|item| item.id == id && item.state.loaded) else {
            return;
        };
        if !open.dataset.field(&field).is_some_and(|found| matches!(found.kind, DrillFieldKind::Categorical { .. })) {
            return;
        }
        let dataset = Arc::clone(&open.dataset);
        let column = open.color.strat_column(&field).to_vec();
        let intervals: usize = dataset.holes.iter().map(|hole| hole.intervals.len()).sum();
        if intervals <= INLINE_CHECK_INTERVALS {
            let outcome = run_strat_check(&dataset, &field, &column);
            self.finish_strat_check(id, field, &dataset, column, outcome);
            return;
        }
        let label = tr!("cmd-strat-check-checking", name = open.name.clone());
        let worked = Arc::clone(&dataset);
        let read_field = field.clone();
        let read_column = column.clone();
        self.spawn_job(
            label,
            vec![crate::app::jobs::JobKey::DrillHole(id)],
            move |_cancel| Ok(run_strat_check(&worked, &read_field, &read_column)),
            move |app: &mut App<'a>, result: anyhow::Result<StratCheckOutcome>| match result {
                Ok(outcome) => app.finish_strat_check(id, field, &dataset, column, outcome),
                Err(error) => userspace_warn!("{}", tr!("cmd-strat-check-failed", error = format!("{error:#}"))),
            },
        );
    }

    /// Keep a finished check for the Column tab, fill an empty column with
    /// the order found, and say what was found in one line.
    fn finish_strat_check(&mut self, id: DrillHoleId, field: String, checked: &Arc<DrillHoleDataset>, column: Vec<String>, outcome: StratCheckOutcome) {
        let Some(open) = self.drill_holes.iter().find(|item| item.id == id) else {
            return;
        };
        let name = open.name.clone();
        let current = open.color.strat_column(&field).to_vec();
        let StratCheckOutcome { majority, column_flags } = outcome;
        let Some(majority) = majority else {
            userspace_warn!("{}", tr!("cmd-strat-check-too-many-codes", field = field.clone(), name = name));
            self.editor.keep_strat_check(StratCheckReport {
                dataset: id,
                field,
                checked: Arc::downgrade(checked),
                order: None,
                order_flags: Vec::new(),
                column,
                column_flags,
                overruled: 0,
                holes: 0,
                comparing: false,
            });
            return;
        };
        let filled = current.is_empty() && !majority.order.is_empty();
        let comparing = !filled && !current.is_empty() && current != majority.order;
        userspace_log!(
            "{}",
            tr!(
                "cmd-strat-check-summary",
                field = field.clone(),
                name = name,
                holes = majority.holes.to_string(),
                // Counted against the column as it stood, as the Column tab counts.
                flagged = flagged_holes(if column.is_empty() || column == majority.order {
                    &majority.flags
                } else {
                    &column_flags
                })
                .to_string()
            )
        );
        if filled {
            self.set_strat_column(id, field.clone(), majority.order.clone());
        }
        self.editor.keep_strat_check(StratCheckReport {
            dataset: id,
            field,
            checked: Arc::downgrade(checked),
            order: Some(majority.order),
            order_flags: majority.flags,
            column,
            column_flags,
            overruled: majority.overruled,
            holes: majority.holes,
            comparing,
        });
        self.redraw_requested = true;
    }
}
