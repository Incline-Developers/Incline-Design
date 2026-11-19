//! The pane under the explorer tree that reads out what a pick on the view
//! holds, from rows the App derives at each change.

use crate::{
    i18n::tr,
    ui::{
        state::{InspectorRow, InspectorRows},
        widgets::data_grid::{PropertyRows, PropertyTable, stack_slot, table_stack},
    },
};

/// The pick's properties, then what it holds below them, in one scroll; with
/// nothing picked, one row saying how to pick.
pub(crate) fn draw_prismoid_inspector(ui: &mut egui::Ui, rect: egui::Rect, rows: Option<&InspectorRows>) {
    let Some(rows) = rows else {
        PropertyTable::new("prismoid_inspector_properties", rect, &tr!("planning-properties")).show(ui, |table| {
            table.header(&tr!("planning-property"), &tr!("planning-value"));
            table.readonly(&tr!("solids-selection"), &tr!("prismoid-inspector-select-prismoids"), None, None);
        });
        return;
    };
    table_stack(ui, rect, "prismoid_inspector", |ui| {
        PropertyTable::new("prismoid_inspector_properties", stack_slot(ui), &rows.title).show(ui, |table| draw_rows(table, &rows.properties));
        PropertyTable::new("prismoid_inspector_contents", stack_slot(ui), &tr!("solids-contents")).show(ui, |table| draw_rows(table, &rows.contents));
    });
}

fn draw_rows(table: &mut PropertyRows<'_>, rows: &[InspectorRow]) {
    table.header(&tr!("planning-property"), &tr!("planning-value"));
    // A note says what a figure rests on or leaves out, never an error.
    for row in rows {
        table.readonly_warning(&row.key, &row.value, row.unit.as_deref(), row.note.as_deref());
    }
}
