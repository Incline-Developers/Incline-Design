//! A row of tab buttons for a dialog that splits its settings into groups.

use crate::ui::widgets::menu::MenuButton;

/// Draw one button per tab, the open one lit; returns whether the user picked
/// a different tab.
///
/// The dialogs are egui throughout, so the tabs look the same on every
/// platform; macOS gets its native chrome from the menu bar, not from here.
pub(crate) fn tab_bar<T: Copy + PartialEq>(ui: &mut egui::Ui, current: &mut T, tabs: impl IntoIterator<Item = (T, String)>) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for (tab, label) in tabs {
            if ui.add(MenuButton::new(label).selected(*current == tab)).clicked() && *current != tab {
                *current = tab;
                changed = true;
            }
        }
    });
    changed
}
