//! The panel beside the scene that reads out what a pick on the view holds,
//! from rows the App derives at each change.

use crate::{
    i18n::tr,
    ui::{
        state::{InspectorRow, InspectorRows},
        widgets::{
            data_grid::{PropertyRows, PropertyTable, property_table_height},
            island::{Island, IslandResponse, Side},
        },
    },
};

pub(crate) const PANEL_ID: &str = "prismoid_inspector_panel";

/// The inspector down the right edge: the pick's properties, then what it
/// holds below them.
pub(crate) fn draw_prismoid_inspector(ui: &mut egui::Ui, rows: &InspectorRows) -> IslandResponse<()> {
    Island::new(PANEL_ID, Side::Right)
        .default_width(300.0)
        .min_width(200.0)
        .max_width(420.0)
        .flush()
        .show(ui, |ui, rect| {
            let split = (rect.top() + property_table_height(ui, rows.properties.len() + 1)).min(rect.bottom());
            let (properties, contents) = rect.split_top_bottom_at_y(split);
            PropertyTable::new("prismoid_inspector_properties", properties, &rows.title).show(ui, |table| draw_rows(table, &rows.properties));
            PropertyTable::new("prismoid_inspector_contents", contents, &tr!("solids-contents")).show(ui, |table| draw_rows(table, &rows.contents));
        })
}

fn draw_rows(table: &mut PropertyRows<'_>, rows: &[InspectorRow]) {
    table.header(&tr!("planning-property"), &tr!("planning-value"));
    for row in rows {
        table.readonly(&row.key, &row.value, row.unit.as_deref(), row.note.as_deref());
    }
}
