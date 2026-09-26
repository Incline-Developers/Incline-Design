use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::{
    i18n::tr,
    model::{Document, FolderId, FolderRegistry, Layer, LayerId, MemberKind, SectionKind},
};

/// Persistence state shared by every project-owned dataset. The source name
/// is informational provenance only; it is never used to reload content.
///
/// Two counters, deliberately: `revision` only ever climbs and is what GPU and
/// view caches key on, while `epoch` names *which* content the item is holding.
/// Undoing an edit puts the old epoch back (see
/// [`Self::restore_epoch`]) while still advancing the revision, so a caches-are-
/// stale signal and a this-matches-what-was-saved signal never have to be the
/// same number. Without the split, undoing back to the last save would leave
/// the item starred forever, or reusing the revision would hand a stale GPU
/// buffer to different content.
#[derive(Clone, Debug)]
pub(crate) struct ProjectItemState {
    pub(crate) source_name: Option<String>,
    pub(crate) source_format: Option<String>,
    pub(crate) loaded: bool,
    pub(crate) deferred: Option<crate::model::formats::omf::DeferredAsset>,
    /// Where an unchanged payload's encoded arrays can be copied from on save.
    pub(crate) payload_source: Option<crate::model::formats::omf::PayloadSource>,
    pub(crate) summary: Option<crate::model::asset_residency::AssetSummary>,
    /// Explorer folder this item sits in, or `None` for its section root.
    ///
    /// One field here covers all five item kinds, each already carrying a
    /// `ProjectItemState`; a move goes through `touch` like any other edit,
    /// so undo can take it back. Not serialized - the id is meaningless
    /// outside the open project; a file records the folder's name instead.
    pub(crate) folder: Option<FolderId>,
    /// Explorer section this item is shown under.
    ///
    /// A tag on the item, not a consequence of which collection holds it,
    /// so a section may show items of more than one kind. Unlike `folder`,
    /// the value means the same thing in every project, so a file records it.
    pub(crate) section: SectionKind,
    revision: u64,
    epoch: u64,
    saved_epoch: u64,
}

impl ProjectItemState {
    pub(crate) fn dirty(kind: MemberKind, source_name: Option<String>) -> Self {
        Self::dirty_with_format(kind, source_name, None)
    }

    pub(crate) fn dirty_with_format(kind: MemberKind, source_name: Option<String>, source_format: Option<String>) -> Self {
        let source_name = provenance_filename(source_name);
        let source_format = provenance_format(source_format).or_else(|| {
            source_name
                .as_deref()
                .and_then(|name| Path::new(name).extension())
                .and_then(|extension| extension.to_str())
                .map(|extension| extension.to_ascii_lowercase())
        });
        Self {
            source_name,
            source_format,
            loaded: true,
            deferred: None,
            payload_source: None,
            summary: None,
            folder: None,
            section: SectionKind::natural_for(kind),
            revision: 1,
            epoch: 1,
            saved_epoch: 0,
        }
    }

    pub(crate) fn with_deferred(mut self, deferred: Option<(crate::model::formats::omf::DeferredAsset, crate::model::asset_residency::AssetSummary)>) -> Self {
        if let Some((locator, summary)) = deferred {
            self.deferred = Some(locator);
            self.summary = Some(summary);
        }
        self
    }

    pub(crate) fn with_loaded(mut self, loaded: bool) -> Self {
        self.loaded = loaded;
        self
    }

    pub(crate) fn with_folder(mut self, folder: Option<FolderId>) -> Self {
        self.folder = folder;
        self
    }

    /// Show the item under `section` rather than where its kind would put it.
    pub(crate) fn with_section(mut self, section: SectionKind) -> Self {
        self.section = section;
        self
    }

    /// Fold what the explorer tree draws of this item into a view key.
    pub(crate) fn hash_row(&self, hasher: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        self.loaded.hash(hasher);
        self.revision.hash(hasher);
        self.folder.hash(hasher);
        self.section.hash(hasher);
    }

    /// Record that the item's content changed, giving it an epoch no earlier
    /// state can collide with. Returns the epoch it was holding, which is what
    /// an undo step keeps so it can put the identity back.
    pub(crate) fn touch(&mut self) -> u64 {
        let previous = self.epoch;
        self.revision = self.revision.wrapping_add(1);
        self.epoch = self.revision;
        previous
    }

    pub(crate) fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Put back an epoch captured by an earlier [`Self::touch`], after undo or
    /// redo has restored the content that epoch named. The revision still
    /// advances: the bytes changed even though the identity is an old one.
    pub(crate) fn restore_epoch(&mut self, epoch: u64) {
        self.revision = self.revision.wrapping_add(1);
        self.epoch = epoch;
    }

    pub(crate) fn set_provenance(&mut self, source_name: Option<String>, source_format: Option<String>) {
        let source_name = provenance_filename(source_name);
        self.source_format = provenance_format(source_format).or_else(|| {
            source_name
                .as_deref()
                .and_then(|name| Path::new(name).extension())
                .and_then(|extension| extension.to_str())
                .map(|extension| extension.to_ascii_lowercase())
        });
        self.source_name = source_name;
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.epoch != self.saved_epoch
    }

    /// Monotonic mutation counter. Cache keys only - never compare it against
    /// a saved baseline, because undo deliberately advances it while putting
    /// older content back. Use [`Self::epoch`] for that.
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn mark_snapshot_saved(&mut self, epoch: u64) {
        self.saved_epoch = epoch;
    }

    pub(crate) fn mark_saved(&mut self) {
        self.saved_epoch = self.epoch;
    }
}

/// The same two-counter split as [`ProjectItemState`], for the project-level
/// content outside the design document: which items exist, and every
/// item-owned field an OMF save writes. `revision` keys the dirty cache;
/// `epoch` is folded into the project content hash, so putting an old epoch
/// back is what lets undo clear the title-bar dirty marker.
#[derive(Clone, Debug, Default)]
pub(crate) struct ProjectContentState {
    revision: u64,
    epoch: u64,
}

impl ProjectContentState {
    pub(crate) fn touch(&mut self) -> u64 {
        let previous = self.epoch;
        self.revision = self.revision.wrapping_add(1);
        self.epoch = self.revision;
        previous
    }

    pub(crate) fn epoch(&self) -> u64 {
        self.epoch
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn restore_epoch(&mut self, epoch: u64) {
        self.revision = self.revision.wrapping_add(1);
        self.epoch = epoch;
    }
}

fn provenance_filename(source_name: Option<String>) -> Option<String> {
    let source_name = source_name?;
    let filename = Path::new(&source_name).file_name().and_then(|name| name.to_str()).unwrap_or(&source_name).trim();
    (!filename.is_empty()).then(|| filename.to_owned())
}

fn provenance_format(source_format: Option<String>) -> Option<String> {
    let source_format = source_format?;
    let source_format = source_format.trim();
    (!source_format.is_empty()).then(|| source_format.to_owned())
}

/// Content epochs captured alongside an immutable whole-project OMF snapshot,
/// paired with the id each belongs to. Completion applies these exact epochs,
/// so edits made while encoding stay dirty instead of being accidentally
/// acknowledged by the older save - and an undo back to the snapshotted state
/// still reads as saved, because it restores the same epoch.
#[derive(Clone, Debug, Default)]
pub(crate) struct SaveToken {
    /// The folder names each section held when the snapshot was taken. A
    /// section's heading is dirty while its folders differ from this, which
    /// is how an empty folder - unmentioned by any item's epoch - still
    /// reads as unsaved work.
    ///
    /// Boxed to keep this variant no bigger than it has to be: it rides
    /// inside a pending-save enum next to much smaller fields.
    pub(crate) folders: Box<FolderRegistry>,
    pub(crate) triangulations: Vec<(u64, u64)>,
    pub(crate) block_models: Vec<(u64, u64)>,
    pub(crate) drill_holes: Vec<(u64, u64)>,
    pub(crate) point_clouds: Vec<(u64, u64)>,
    pub(crate) rasters: Vec<(u64, u64)>,
}

pub(crate) const PROJECT_FORMAT_VERSION: u32 = 2;

#[cfg(target_arch = "wasm32")]
pub(crate) type ProjectId = uuid::Uuid;

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProjectPersistence {
    Untitled,
    NativePath(PathBuf),
    ImportedFile { source_name: String },
    BrowserRecord(ProjectId),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectMetadata {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) coordinate_reference_system: String,
    #[serde(default)]
    pub(crate) units: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectFile {
    pub(crate) format_version: u32,
    pub(crate) document: Document,
    pub(crate) metadata: ProjectMetadata,
    /// Every explorer folder in the project, for every section.
    ///
    /// Beside the document: most sections hold items the `App`
    /// owns, not layers, so there's nowhere else to save their empty
    /// folders from.
    #[serde(default)]
    pub(crate) folders: FolderRegistry,
}

#[derive(Clone, Debug)]
pub(crate) struct OpenProject {
    #[cfg(target_arch = "wasm32")]
    pub(crate) id: ProjectId,
    /// Stable identity for this open instance. Namespace zero is reserved for
    /// persistent IDs and a runtime namespace prevents stale session handles.
    pub(crate) runtime_id: u32,
    pub(crate) path: Option<PathBuf>,
    #[cfg(target_arch = "wasm32")]
    pub(crate) persistence: ProjectPersistence,
    pub(crate) project: ProjectFile,
    /// Content reported by the OMF decoder that Incline Design cannot guarantee it
    /// will reproduce. Native Open retains these until a confirmed rewrite
    /// succeeds; Merge reports them without attaching them to the target.
    pub(crate) lossy_save_warnings: Vec<String>,
    pub(crate) lossy_save_confirmed: bool,
    /// Identity of project-owned content outside the design document. Every
    /// aggregate membership change advances it; each item's own state tracks
    /// its payload, style and loaded flag. Public so an [`crate::model::EditTarget`] can borrow it
    /// alongside the document, which lives one field over.
    pub(crate) content: ProjectContentState,
    /// Namespace-invariant content fingerprint of the complete project at its
    /// last successful load/save. `None` means the project has never been
    /// saved. Replaces whole-document JSON snapshots: an interactive drag
    /// used to re-clone and re-serialize the full project per pointer event.
    saved_content_hash: Option<u64>,
    /// Per-layer fingerprints (keyed by the layer id's 32-bit local half) at
    /// the last successful load/save. `None` means never saved: every layer
    /// counts as dirty.
    saved_layer_hashes: Option<HashMap<u64, u64>>,
    /// Changes whenever a successful load/save establishes a new dirty-state
    /// baseline. The document revision can stay unchanged while an async save
    /// completes, so UI caches cannot use that revision alone.
    savepoint_revision: u64,
    /// Per-object hash cache backing [`Document::content_hash`].
    content_hash_cache: RefCell<HashMap<crate::model::ObjectId, (u64, u64)>>,
    dirty_cache: RefCell<Option<ProjectDirtyCache>>,
    /// Dirty-layer set cached against the document revision it was computed at.
    layer_dirty_cache: RefCell<Option<(u64, HashSet<LayerId>)>>,
}

/// Cached whole-project dirty result. Document mutations bump `revision`; the
/// other serialized project fields are included directly in the cache key.
#[derive(Clone, Debug)]
struct ProjectDirtyCache {
    revision: u64,
    content_revision: u64,
    content_epoch: u64,
    format_version: u32,
    metadata: ProjectMetadata,
    dirty: bool,
}

impl OpenProject {
    pub(crate) fn has_unsaved_changes(&self) -> bool {
        let Some(saved_content_hash) = self.saved_content_hash else {
            return true;
        };
        let revision = self.project.document.revision();
        if let Some(cache) = self.dirty_cache.borrow().as_ref()
            && cache.revision == revision
            && cache.content_revision == self.content.revision()
            && cache.content_epoch == self.content.epoch()
            && cache.format_version == self.project.format_version
            && cache.metadata == self.project.metadata
        {
            return cache.dirty;
        }

        let dirty = self.content_hash() != saved_content_hash;
        *self.dirty_cache.borrow_mut() = Some(ProjectDirtyCache {
            revision,
            content_revision: self.content.revision(),
            content_epoch: self.content.epoch(),
            format_version: self.project.format_version,
            metadata: self.project.metadata.clone(),
            dirty,
        });
        dirty
    }

    /// Fingerprint of everything the project file serializes, computed from the
    /// runtime document with cached per-object hashes - only objects touched
    /// since the previous call re-hash.
    fn content_hash(&self) -> u64 {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        self.project.format_version.hash(&mut hasher);
        self.project.metadata.name.hash(&mut hasher);
        self.project.metadata.coordinate_reference_system.hash(&mut hasher);
        self.project.metadata.units.hash(&mut hasher);
        self.content.epoch().hash(&mut hasher);
        // Folder names, so an empty folder still counts as unsaved work.
        self.project.folders.hash_into(&mut hasher);
        self.project
            .document
            .content_hash(&mut self.content_hash_cache.borrow_mut(), &self.project.folders)
            .hash(&mut hasher);
        hasher.finish()
    }

    pub(crate) fn current_content_hash(&self) -> u64 {
        self.content_hash()
    }

    pub(crate) fn touch_content(&mut self) -> u64 {
        let previous = self.content.touch();
        *self.dirty_cache.borrow_mut() = None;
        previous
    }

    /// Per-layer fingerprints of the current document state. Captured next to
    /// [`Self::current_content_hash`] when an asynchronous saver snapshots the
    /// project.
    pub(crate) fn current_layer_hashes(&self) -> HashMap<u64, u64> {
        self.project.document.layer_content_hashes(&mut self.content_hash_cache.borrow_mut(), &self.project.folders)
    }

    /// Record the exact snapshot written by an asynchronous saver. If edits
    /// happened while it was writing, `has_unsaved_changes` compares the live
    /// content with this hash and correctly remains dirty.
    pub(crate) fn mark_snapshot_saved(&mut self, snapshot_hash: u64, snapshot_layer_hashes: HashMap<u64, u64>) {
        self.saved_content_hash = Some(snapshot_hash);
        self.saved_layer_hashes = Some(snapshot_layer_hashes);
        self.savepoint_revision = self.savepoint_revision.wrapping_add(1);
        self.dirty_cache = RefCell::new(None);
        self.layer_dirty_cache = RefCell::new(None);
    }

    pub(crate) fn mark_saved(&mut self) {
        self.saved_content_hash = Some(self.content_hash());
        self.saved_layer_hashes = Some(self.current_layer_hashes());
        self.savepoint_revision = self.savepoint_revision.wrapping_add(1);
        self.dirty_cache = RefCell::new(None);
        self.layer_dirty_cache = RefCell::new(None);
    }

    pub(crate) fn savepoint_revision(&self) -> u64 {
        self.savepoint_revision
    }

    /// Layers whose content differs from the last successful load/save.
    /// Recomputed only when the document revision changes.
    pub(crate) fn dirty_layer_ids(&self) -> HashSet<LayerId> {
        let revision = self.project.document.revision();
        if let Some((cached_revision, dirty)) = self.layer_dirty_cache.borrow().as_ref()
            && *cached_revision == revision
        {
            return dirty.clone();
        }
        let dirty: HashSet<LayerId> = match &self.saved_layer_hashes {
            None => self.project.document.layers().iter().map(|layer| layer.id).collect(),
            Some(saved) => {
                let current = self.current_layer_hashes();
                self.project
                    .document
                    .layers()
                    .iter()
                    .filter(|layer| {
                        let key = layer.id.0 & LOCAL_MASK;
                        saved.get(&key) != current.get(&key)
                    })
                    .map(|layer| layer.id)
                    .collect()
            }
        };
        *self.layer_dirty_cache.borrow_mut() = Some((revision, dirty.clone()));
        dirty
    }

    /// Whether the Designs collection itself differs from the save baseline.
    /// Comparing the complete layer-hash maps catches deleted layers after
    /// their explorer rows have disappeared.
    pub(crate) fn designs_dirty(&self, saved_folders: &FolderRegistry) -> bool {
        self.section_layers_dirty(SectionKind::Designs, saved_folders)
    }

    /// The same for any section that shows layers, reading only the layers
    /// tagged with it. The baseline records no section, so a moved layer is
    /// unsaved work where it lands, and a deleted one in every section a
    /// layer can sit in: nothing left says which held it.
    pub(crate) fn section_layers_dirty(&self, section: SectionKind, saved_folders: &FolderRegistry) -> bool {
        const LOCAL_MASK: u64 = u32::MAX as u64;
        if self.project.folders.names(section) != saved_folders.names(section) {
            return true;
        }
        let Some(saved) = &self.saved_layer_hashes else {
            return self.project.document.layers().iter().any(|layer| layer.section == section);
        };
        let current = self.current_layer_hashes();
        let tagged_changed = self.project.document.layers().iter().any(|layer| {
            let key = layer.id.0 & LOCAL_MASK;
            layer.section == section && saved.get(&key) != current.get(&key)
        });
        tagged_changed || saved.keys().any(|key| !current.contains_key(key))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ProjectStore {
    /// The single native project, represented as a one-element collection
    /// while older document helpers are progressively simplified.
    pub(crate) projects: Vec<OpenProject>,
    /// `Some(0)` while a project is open, otherwise `None`.
    pub(crate) active_index: Option<usize>,
    next_runtime_namespace: u32,
}

impl Default for ProjectStore {
    fn default() -> Self {
        Self {
            projects: Vec::new(),
            active_index: None,
            next_runtime_namespace: 1,
        }
    }
}

impl ProjectStore {
    pub(crate) fn active_project(&self) -> Option<&OpenProject> {
        self.active_index.and_then(|index| self.projects.get(index))
    }

    pub(crate) fn active_project_mut(&mut self) -> Option<&mut OpenProject> {
        self.active_index.and_then(|index| self.projects.get_mut(index))
    }

    pub(crate) fn active_document(&self) -> Option<&Document> {
        self.active_project().map(|p| &p.project.document)
    }

    pub(crate) fn active_document_mut(&mut self) -> Option<&mut Document> {
        self.active_project_mut().map(|p| &mut p.project.document)
    }

    pub(crate) fn has_active_project(&self) -> bool {
        self.active_index.is_some_and(|index| index < self.projects.len())
    }

    pub(crate) fn project_index_for_runtime_id(&self, runtime_id: u32) -> Option<usize> {
        self.projects.iter().position(|project| project.runtime_id == runtime_id)
    }

    pub(crate) fn project_index_for_object(&self, object_id: crate::model::ObjectId) -> Option<usize> {
        self.projects.iter().position(|project| project.project.document.get_object(object_id).is_some())
    }

    pub(crate) fn project_index_for_layer(&self, layer_id: LayerId) -> Option<usize> {
        self.projects.iter().position(|project| project.project.document.layer(layer_id).is_some())
    }

    /// Install the one native project and make it the editing target.
    pub(crate) fn add_and_activate(&mut self, mut project: OpenProject) -> usize {
        self.prepare_project(&mut project);
        self.projects.clear();
        self.projects.push(project);
        self.active_index = Some(0);
        0
    }

    fn prepare_project(&mut self, project: &mut OpenProject) {
        let namespace = self.next_runtime_namespace;
        self.next_runtime_namespace = self.next_runtime_namespace.saturating_add(1);
        project.runtime_id = namespace;
        project.project.document.apply_runtime_namespace(namespace);
        // `open_project` hashes disk-local ObjectIds to establish the saved
        // baseline. The hash values are namespace-invariant, but those cache
        // keys are not; retaining them would permanently duplicate every
        // entry after runtime ids are assigned.
        project.content_hash_cache.borrow_mut().clear();
    }

    /// Replace the project at `index` with a freshly parsed copy (revert to
    /// disk). The replacement gets a new runtime namespace, so the caller must
    /// drop undo history and selections that reference the old namespace.
    /// Loaded states come from the replacement project. Returns its new
    /// runtime id.
    #[cfg(any(not(target_arch = "wasm32"), test))]
    pub(crate) fn replace_project(&mut self, index: usize, mut project: OpenProject) -> Option<u32> {
        if index >= self.projects.len() {
            return None;
        }
        self.prepare_project(&mut project);
        let runtime_id = project.runtime_id;
        self.projects[index] = project;
        Some(runtime_id)
    }

    pub(crate) fn set_active_index(&mut self, index: usize) {
        if index < self.projects.len() {
            self.active_index = Some(index);
        }
    }

    /// Fingerprint of everything `scene_document()` reads: the active
    /// document's revision (bumped by logical edits, including load toggles).
    /// Equal keys guarantee an identical composite, letting callers skip
    /// the rebuild.
    pub(crate) fn composite_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for project in &self.projects {
            project.runtime_id.hash(&mut hasher);
            project.project.document.revision().hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Build the runtime document rendered and queried by the viewport from
    /// the loaded layers in the retained project document.
    pub(crate) fn scene_document(&self) -> Document {
        let mut scene = Document::new();
        for project in &self.projects {
            let document = &project.project.document;
            let mut per_layer: HashMap<LayerId, Vec<usize>> = HashMap::new();
            for (index, object) in document.objects().iter().enumerate() {
                if project.project.document.layer(object.layer()).is_some_and(|layer| layer.loaded) && !document.is_object_hidden(object.id()) {
                    per_layer.entry(object.layer()).or_default().push(index);
                }
            }
            for layer in document.layers() {
                if !layer.loaded {
                    continue;
                }
                let indices = per_layer.remove(&layer.id).unwrap_or_default();
                scene.append_layer_snapshot_unindexed(
                    layer,
                    indices.iter().map(|&index| {
                        let object = &document.objects()[index];
                        (object, document.object_revision(object.id()))
                    }),
                );
            }
        }
        scene.rebuild_object_index();
        scene
    }
}

pub(crate) fn new_empty(path: Option<PathBuf>) -> ProjectFile {
    let document = Document::new();

    ProjectFile {
        format_version: PROJECT_FORMAT_VERSION,
        document,
        metadata: ProjectMetadata {
            name: project_name(path.as_deref(), &tr!(literal = "Untitled")),
            ..Default::default()
        },
        folders: FolderRegistry::default(),
    }
}

impl ProjectFile {
    /// Drop design-layer memberships the registry cannot resolve.
    ///
    /// `EditTarget::heal_folders` heals item memberships, since it can see
    /// the `App`-owned collections; this half covers the document, wherever
    /// it arrives without its folder list.
    pub(crate) fn heal_folders(&mut self) -> bool {
        self.document.heal_layers(&self.folders)
    }
}

pub(crate) fn validate(project: &mut ProjectFile) -> Result<()> {
    if project.format_version != PROJECT_FORMAT_VERSION {
        bail!("Unsupported legacy design format version {} (expected {})", project.format_version, PROJECT_FORMAT_VERSION);
    }
    // Enforce model invariants before runtime namespacing: duplicate or
    // out-of-range ids would otherwise silently alias distinct records once
    // ids are masked into the 32-bit local namespace.
    project.document.validate().context("invalid project document")?;
    project.folders.validate().context("invalid project folders")?;
    // Serialized counters are advisory only; derive them from the actual ids.
    project.document.recompute_id_counters();
    project.folders.recompute_next_id();
    project.document.rebuild_object_index();
    // Healed, not rejected - a missing folder doesn't invalidate the layer.
    project.heal_folders();
    Ok(())
}

/// How an incoming folder list meets the one already in the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FolderMergeMode {
    /// A folder of the same name in the same section is the same folder.
    /// What native Open wants, so reopening a project doesn't double folders.
    Reuse,
    /// Every incoming folder becomes a new one, its name suffixed until it is
    /// free, matching the treatment its layers and items already get.
    Distinct,
}

/// Bring `imported`'s folders into `target`, for every section at once, and
/// report which target folder each imported id became. Sections are merged
/// independently, so a "Pit" under Triangulations can never absorb one under
/// Designs.
pub(crate) fn merge_folders(target: &mut FolderRegistry, imported: &FolderRegistry, mode: FolderMergeMode) -> HashMap<FolderId, FolderId> {
    let mut map = HashMap::new();
    for section in SectionKind::ALL {
        for folder in imported.folders(section) {
            let merged = match mode {
                FolderMergeMode::Reuse => target.ensure(section, &folder.name),
                FolderMergeMode::Distinct => {
                    let name = unique_item_name(folder.name.clone(), target.folders(section).iter().map(|folder| folder.name.as_str()));
                    target.add(section, name)
                }
            };
            if let Some(merged) = merged {
                map.insert(folder.id, merged);
            }
        }
    }
    map
}

/// Membership for one incoming member: the target folder its imported folder
/// became, or `None` for a member that was at the root or whose folder did not
/// survive the merge.
pub(crate) fn merged_folder(folders: &HashMap<FolderId, FolderId>, imported: Option<FolderId>) -> Option<FolderId> {
    imported.and_then(|folder| folders.get(&folder).copied())
}

/// Give a freshly merged layer the placement its imported twin had: the
/// section it is shown under, then the folder within that section.
///
/// The tag goes first because moving sections clears the folder, and a
/// folder tied to a section this layer no longer sits in is dropped too.
fn carry_layer_placement(target: &mut Document, id: LayerId, imported: &Layer, folders: &HashMap<FolderId, FolderId>) {
    let section = imported.section.healed_for(MemberKind::Layer);
    target.set_layer_section(id, section);
    let folder = (section == imported.section).then(|| merged_folder(folders, imported.folder)).flatten();
    target.set_layer_folder(id, folder);
}

/// `folders` is the map [`merge_folders`] returned for the same merge: the
/// registry is merged once for the whole file, and each document merge looks
/// its layers' membership up in it.
pub(crate) fn merge_document(target: &mut Document, imported: &Document, folders: &HashMap<FolderId, FolderId>) -> usize {
    let mut layer_map = HashMap::new();
    for layer in imported.layers() {
        let target_layer = match target.layer_id_by_name(&layer.name) {
            // A layer that is already here keeps the folder the user put it
            // in: an import that happens to share a name must not rearrange
            // the tree.
            Some(existing) => existing,
            None => {
                let id = target.add_layer(layer.name.clone(), layer.color_index, layer.color, layer.loaded, layer.elevation);
                // Set after insertion: `add_layer` always lands a layer at the
                // root of the section its kind puts it in.
                carry_layer_placement(target, id, layer, folders);
                id
            }
        };
        layer_map.insert(layer.id, target_layer);
    }

    copy_objects(target, imported, &layer_map, false)
}

/// Merge a foreign design while keeping every incoming layer distinct. Name
/// collisions receive conventional numbered suffixes and every object gets a
/// fresh target id, avoiding aliases with native OMF ids already in use.
pub(crate) fn merge_document_unique_layers(target: &mut Document, imported: &Document, folders: &HashMap<FolderId, FolderId>) -> usize {
    let mut layer_map = HashMap::new();
    for layer in imported.layers() {
        let name = unique_layer_name(target, &layer.name);
        let target_layer = target.add_layer(name, layer.color_index, layer.color, layer.loaded, layer.elevation);
        carry_layer_placement(target, target_layer, layer, folders);
        layer_map.insert(layer.id, target_layer);
    }

    copy_objects(target, imported, &layer_map, false)
}

/// Largest id a native OMF assigns locally; anything above came from elsewhere.
const LOCAL_MASK: u64 = u32::MAX as u64;

/// Merge documents belonging to one native OMF while preserving stable local
/// IDs whenever they are valid and unused. Legacy multi-composite files can
/// contain collisions, which are remapped into the target document.
pub(crate) fn merge_document_preserve_ids(target: &mut Document, imported: &Document, folders: &HashMap<FolderId, FolderId>) -> usize {
    let mut layer_map = HashMap::new();
    for layer in imported.layers() {
        let target_layer = if layer.id.0 <= LOCAL_MASK && target.layer(layer.id).is_none() {
            target.append_layer_snapshot(layer, std::iter::empty());
            layer.id
        } else {
            let mut remapped = layer.clone();
            remapped.id = target.allocate_layer_id();
            remapped.name = unique_layer_name(target, &remapped.name);
            let id = remapped.id;
            target.append_layer_snapshot(&remapped, std::iter::empty());
            id
        };
        carry_layer_placement(target, target_layer, layer, folders);
        layer_map.insert(layer.id, target_layer);
    }

    copy_objects(target, imported, &layer_map, true)
}

/// Copy `imported`'s objects, and its deferred layers, onto the target layers
/// `layer_map` gives them, returning how many objects were added. With
/// `preserve_ids`, an object keeps its id wherever that is a valid local id
/// the target has not used; otherwise every object gets a fresh one.
fn copy_objects(target: &mut Document, imported: &Document, layer_map: &HashMap<LayerId, LayerId>, preserve_ids: bool) -> usize {
    target.copy_deferred_layers(imported, layer_map, preserve_ids);
    let mut added = 0;
    for object in imported.objects() {
        let layer = layer_map.get(&object.layer()).copied().unwrap_or_else(|| target.ensure_default_layer());
        let source_id = object.id();
        let id = if preserve_ids && source_id.0 <= LOCAL_MASK && target.get_object(source_id).is_none() {
            source_id
        } else {
            target.allocate_object_id()
        };
        target.insert_object(object.with_id_and_layer(id, layer));
        if imported.is_object_hidden(source_id) {
            target.set_object_hidden(id, true);
        }
        added += 1;
    }
    added
}

fn unique_layer_name(document: &Document, requested: &str) -> String {
    let fallback;
    let base = if requested.trim().is_empty() {
        fallback = tr!(literal = "Layer");
        fallback.as_str()
    } else {
        requested.trim()
    };
    if document.layer_id_by_name(base).is_none() {
        return base.to_owned();
    }
    for suffix in 2u64.. {
        let candidate = format!("{base} ({suffix})");
        if document.layer_id_by_name(&candidate).is_none() {
            return candidate;
        }
    }
    unreachable!()
}

/// Resolve a project-item display name without using a source path as its
/// identity. Imports and generated data share this rule across all dataset
/// families.
pub(crate) fn unique_item_name<'a>(requested: String, existing: impl Iterator<Item = &'a str>) -> String {
    let base = if requested.trim().is_empty() {
        tr!(literal = "Item")
    } else {
        requested.trim().to_owned()
    };
    let existing = existing.collect::<HashSet<_>>();
    if !existing.contains(base.as_str()) {
        return base;
    }
    for suffix in 2u64.. {
        let candidate = format!("{base} ({suffix})");
        if !existing.contains(candidate.as_str()) {
            return candidate;
        }
    }
    unreachable!()
}

/// Derive the name of an imported project item from its source filename.
/// Provenance retains the complete filename; only the in-project display name
/// drops the final extension.
pub(crate) fn imported_item_name(path: &Path, fallback: &str) -> String {
    path.file_stem()
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| fallback.to_owned())
}

fn project_name(path: Option<&Path>, fallback: &str) -> String {
    path.and_then(Path::file_stem)
        .and_then(|name| name.to_str())
        .or_else(|| Path::new(fallback).file_stem().and_then(|name| name.to_str()))
        .unwrap_or(fallback)
        .to_owned()
}

pub(crate) fn open_project(path: Option<PathBuf>, mut project: ProjectFile) -> Result<OpenProject> {
    validate(&mut project)?;
    let mut project = OpenProject {
        #[cfg(target_arch = "wasm32")]
        id: uuid::Uuid::new_v4(),
        runtime_id: 0,
        #[cfg(target_arch = "wasm32")]
        persistence: path.clone().map(ProjectPersistence::NativePath).unwrap_or(ProjectPersistence::Untitled),
        path,
        project,
        lossy_save_warnings: Vec::new(),
        lossy_save_confirmed: false,
        content: ProjectContentState::default(),
        saved_content_hash: None,
        saved_layer_hashes: None,
        savepoint_revision: 0,
        content_hash_cache: RefCell::new(HashMap::new()),
        dirty_cache: RefCell::new(None),
        layer_dirty_cache: RefCell::new(None),
    };
    // A pathless project has never been saved and stays dirty; the hash is
    // namespace-invariant, so capturing it before runtime namespacing is fine.
    if project.path.is_some() {
        project.mark_saved();
    }
    Ok(project)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn open_imported_project(source_name: String, mut project: ProjectFile) -> Result<OpenProject> {
    validate(&mut project)?;
    let mut project = OpenProject {
        #[cfg(target_arch = "wasm32")]
        id: uuid::Uuid::new_v4(),
        runtime_id: 0,
        path: None,
        persistence: ProjectPersistence::ImportedFile { source_name },
        project,
        lossy_save_warnings: Vec::new(),
        lossy_save_confirmed: false,
        content: ProjectContentState::default(),
        saved_content_hash: None,
        saved_layer_hashes: None,
        savepoint_revision: 0,
        content_hash_cache: RefCell::new(HashMap::new()),
        dirty_cache: RefCell::new(None),
        layer_dirty_cache: RefCell::new(None),
    };
    project.mark_saved();
    Ok(project)
}
