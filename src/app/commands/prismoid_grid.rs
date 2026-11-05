//! Prismoid grid explorer commands.

use crate::{
    app::App,
    model::{ItemRef, SceneEntityId, prismoid_grid::PrismoidGridId},
};

impl App<'_> {
    /// Remove a prismoid grid from the project as one undoable step.
    pub(crate) fn remove_prismoid_grid(&mut self, id: PrismoidGridId) {
        let entity = SceneEntityId::PrismoidGrid(id);
        self.editor.selected_handles.remove(&entity);
        self.editor.hidden_handles.remove(&entity);
        self.editor.explicitly_frozen.remove(&entity);
        self.editor.frozen_handles.remove(&entity);
        self.editor.translucent_handles.remove(&entity);
        self.delete_project_item(ItemRef::PrismoidGrid(id));
    }
}
