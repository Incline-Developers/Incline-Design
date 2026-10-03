//! Explorer folders: one registry of named groups per explorer section.
//!
//! Every branch of the explorer tree groups its own items the same way, and
//! nothing crosses between branches - a folder belongs to exactly one
//! section, carried by [`SectionKind`].
//!
//! The registry lives on [`crate::model::project::ProjectFile`], beside the
//! design document, since most sections hold items the `App`
//! owns rather than the document.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::model::{ItemRef, LayerId};

/// Identity of a folder for as long as the project is open.
///
/// Minted on load and never written to a project file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct FolderId(pub(crate) u64);

/// A named group of items under one explorer section heading.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Folder {
    pub(crate) id: FolderId,
    pub(crate) name: String,
}

/// One branch of the explorer tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum SectionKind {
    Designs,
    Triangulations,
    Rasters,
    PointClouds,
    BlockModels,
    DrillHoles,
    /// What the project derives from its data, such as reference points and the
    /// surfaces built from them, and whatever is kept beside them.
    /// An item is shown here because it is tagged here, never because of what
    /// it holds - see [`Self::natural_for`].
    Modelling,
}

impl SectionKind {
    pub(crate) const ALL: [Self; 7] = [
        Self::Designs,
        Self::Triangulations,
        Self::Rasters,
        Self::PointClouds,
        Self::BlockModels,
        Self::DrillHoles,
        Self::Modelling,
    ];

    /// Stable key this section's folder list is written under. Never
    /// translated and never renamed: it is file content.
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Designs => "designs",
            Self::Triangulations => "triangulations",
            Self::Rasters => "rasters",
            Self::PointClouds => "point_clouds",
            Self::BlockModels => "block_models",
            Self::DrillHoles => "drill_holes",
            Self::Modelling => "modelling",
        }
    }

    pub(crate) fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|section| section.key() == key)
    }

    fn index(self) -> usize {
        match self {
            Self::Designs => 0,
            Self::Triangulations => 1,
            Self::Rasters => 2,
            Self::PointClouds => 3,
            Self::BlockModels => 4,
            Self::DrillHoles => 5,
            Self::Modelling => 6,
        }
    }

    /// Section an item of `kind` shows under until something tags it otherwise.
    ///
    /// Modelling is never natural for anything: a plain new layer is a design
    /// layer, and a modelling layer is one tagged Modelling, when it was made
    /// or by being moved there.
    pub(crate) const fn natural_for(kind: MemberKind) -> Self {
        match kind {
            MemberKind::Layer => Self::Designs,
            MemberKind::Triangulation => Self::Triangulations,
            MemberKind::Raster => Self::Rasters,
            MemberKind::PointCloud => Self::PointClouds,
            MemberKind::BlockModel => Self::BlockModels,
            MemberKind::DrillHole => Self::DrillHoles,
        }
    }

    /// The kinds of item this section has a row for.
    ///
    /// A section shows exactly what it admits, so nothing can be tagged into a
    /// section that would not draw it. This is also the whole rule for where
    /// an item may be kept: a kind never changes, and any section admitting
    /// that kind may hold it.
    pub(crate) fn admitted(self) -> &'static [MemberKind] {
        match self {
            Self::Designs => &[MemberKind::Layer],
            Self::Triangulations => &[MemberKind::Triangulation],
            Self::Rasters => &[MemberKind::Raster],
            Self::PointClouds => &[MemberKind::PointCloud],
            Self::BlockModels => &[MemberKind::BlockModel],
            Self::DrillHoles => &[MemberKind::DrillHole],
            // Points and the surfaces built from them sit side by side here.
            Self::Modelling => &[MemberKind::Layer, MemberKind::Triangulation],
        }
    }

    pub(crate) fn admits(self, kind: MemberKind) -> bool {
        self.admitted().contains(&kind)
    }

    /// Where an item of `kind` tagged `self` is actually shown: `self` when
    /// this section admits that kind, the natural section when it does not.
    ///
    /// A tag this build cannot honour comes from a file written with more
    /// sections, or more kinds in a section, than this build has; healing it
    /// stops the item being drawn nowhere at all.
    pub(crate) fn healed_for(self, kind: MemberKind) -> Self {
        if self.admits(kind) { self } else { Self::natural_for(kind) }
    }

    /// Where an item of `kind` built from things kept in `sources` is born:
    /// beside its sources when they agree on a section that can show it, at
    /// its natural section otherwise.
    ///
    /// A derived item belongs next to what it was derived from, so a surface
    /// built from points kept under Modelling appears there rather than away
    /// from them. Sources that disagree name no one place to be beside, and
    /// nor does a section with no row for the kind, so both fall back to where
    /// the kind puts it. Stated once here so the next tool that builds
    /// something from something else does not restate it.
    pub(crate) fn derived_for(kind: MemberKind, sources: impl IntoIterator<Item = Self>) -> Self {
        let mut sources = sources.into_iter();
        let Some(first) = sources.next() else {
            return Self::natural_for(kind);
        };
        if sources.any(|section| section != first) {
            return Self::natural_for(kind);
        }
        first.healed_for(kind)
    }

    /// `serde` hooks for [`crate::model::Layer::section`], the one tag a file
    /// records inside a blob rather than beside it: a layer left where its kind
    /// puts it serializes no tag at all.
    pub(crate) fn natural_layer() -> Self {
        Self::natural_for(MemberKind::Layer)
    }

    pub(crate) fn is_natural_layer(section: &Self) -> bool {
        *section == Self::natural_layer()
    }
}

/// What sort of thing a folder member is, with no id to hand.
///
/// Separate from [`FolderMember`] because an item's tag must be settled
/// before the item exists to name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum MemberKind {
    Layer,
    Triangulation,
    Raster,
    PointCloud,
    BlockModel,
    DrillHole,
}

/// What a folder holds: a design layer, from the document, or a project
/// item the `App` owns.
///
/// One type for both halves so a single move command, rename target and
/// drop handler cover every section.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum MemberTarget {
    Layer(LayerId),
    Item(ItemRef),
}

impl MemberTarget {
    pub(crate) fn kind(self) -> MemberKind {
        match self {
            Self::Layer(_) => MemberKind::Layer,
            Self::Item(item) => item.kind(),
        }
    }
}

/// Where a member is kept: the section showing it, and the folder within that
/// section, or `None` for the section root.
///
/// The two travel together because a folder id belongs to the section that
/// minted it, so settling one settles the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Placement {
    pub(crate) section: SectionKind,
    pub(crate) folder: Option<FolderId>,
}

impl Placement {
    pub(crate) fn new(section: SectionKind, folder: Option<FolderId>) -> Self {
        Self { section, folder }
    }

    /// The placement actually reachable for an item of `kind`: the tagged
    /// section when it admits that kind, its natural section otherwise, and a
    /// folder only when the settled section still has it.
    ///
    /// The one guard between a recorded placement and the state it names,
    /// which may have been changed by something outside the undo history.
    pub(crate) fn placeable(self, kind: MemberKind, known: &FolderRegistry) -> Self {
        let section = self.section.healed_for(kind);
        Self {
            section,
            folder: self.folder.filter(|folder| known.contains(section, *folder)),
        }
    }
}

/// A member of one section's folders: what is held, and the section holding
/// it.
///
/// The section is the tag read off the item when the member was made, not
/// derived from the target's type, so an item shown somewhere other than
/// its kind's natural section still resolves its folders there. Carried
/// rather than looked up on demand because most item kinds live on the
/// `App`, which `model` cannot reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FolderMember {
    section: SectionKind,
    target: MemberTarget,
}

impl FolderMember {
    pub(crate) fn new(section: SectionKind, target: MemberTarget) -> Self {
        Self { section, target }
    }

    pub(crate) fn layer(section: SectionKind, id: LayerId) -> Self {
        Self::new(section, MemberTarget::Layer(id))
    }

    pub(crate) fn item(section: SectionKind, item: ItemRef) -> Self {
        Self::new(section, MemberTarget::Item(item))
    }

    pub(crate) fn section(self) -> SectionKind {
        self.section
    }

    pub(crate) fn target(self) -> MemberTarget {
        self.target
    }

    pub(crate) fn kind(self) -> MemberKind {
        self.target.kind()
    }
}

/// Every explorer folder in the project, grouped by the section that owns it.
///
/// Ids are unique across the whole registry, not merely within a section,
/// and `contains` is section-scoped on top of that.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FolderRegistry {
    #[serde(default)]
    sections: [Vec<Folder>; 7],
    #[serde(default)]
    next_id: u64,
}

impl FolderRegistry {
    pub(crate) fn folders(&self, section: SectionKind) -> &[Folder] {
        &self.sections[section.index()]
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.sections.iter().all(Vec::is_empty)
    }

    pub(crate) fn index_of(&self, section: SectionKind, id: FolderId) -> Option<usize> {
        self.folders(section).iter().position(|folder| folder.id == id)
    }

    pub(crate) fn contains(&self, section: SectionKind, id: FolderId) -> bool {
        self.index_of(section, id).is_some()
    }

    pub(crate) fn name(&self, section: SectionKind, id: FolderId) -> Option<&str> {
        self.folders(section).iter().find(|folder| folder.id == id).map(|folder| folder.name.as_str())
    }

    pub(crate) fn by_name(&self, section: SectionKind, name: &str) -> Option<FolderId> {
        self.folders(section).iter().find(|folder| folder.name == name).map(|folder| folder.id)
    }

    pub(crate) fn has_name(&self, section: SectionKind, name: &str) -> bool {
        self.by_name(section, name).is_some()
    }

    /// Hand out an id without creating anything, for a command that will carry
    /// the whole folder into the undo history before it is applied.
    pub(crate) fn allocate_id(&mut self) -> FolderId {
        let id = FolderId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Add a folder called `name` to `section`. Blank names, and a name the
    /// section already has, are refused.
    pub(crate) fn add(&mut self, section: SectionKind, name: String) -> Option<FolderId> {
        if name.trim().is_empty() || self.has_name(section, &name) {
            return None;
        }
        let id = self.allocate_id();
        self.sections[section.index()].push(Folder { id, name });
        Some(id)
    }

    /// The id of `section`'s folder called `name`, adding it when there is
    /// none. `None` only for a blank name.
    pub(crate) fn ensure(&mut self, section: SectionKind, name: &str) -> Option<FolderId> {
        self.by_name(section, name).or_else(|| self.add(section, name.to_owned()))
    }

    /// Put `folder` back at `index`, or at the end when `index` is past it,
    /// keeping its id so everything already pointing at it still does.
    pub(crate) fn insert(&mut self, section: SectionKind, index: usize, folder: Folder) -> bool {
        if folder.name.trim().is_empty() || self.has_name(section, &folder.name) || self.index_of(section, folder.id).is_some() {
            return false;
        }
        self.next_id = self.next_id.max(folder.id.0.saturating_add(1));
        let list = &mut self.sections[section.index()];
        list.insert(index.min(list.len()), folder);
        true
    }

    /// Remove a folder. Its members are not touched here - returning them to
    /// the root is the caller's job, since where they live differs by section.
    pub(crate) fn remove(&mut self, section: SectionKind, id: FolderId) -> bool {
        let Some(index) = self.index_of(section, id) else {
            return false;
        };
        self.sections[section.index()].remove(index);
        true
    }

    /// Rename a folder. Membership follows the id, so nothing else moves.
    pub(crate) fn rename(&mut self, section: SectionKind, id: FolderId, name: String) -> bool {
        if name.trim().is_empty() || self.has_name(section, &name) {
            return false;
        }
        let Some(index) = self.index_of(section, id) else {
            return false;
        };
        self.sections[section.index()][index].name = name;
        true
    }

    /// Derive the id counter from the ids actually present.
    pub(crate) fn recompute_next_id(&mut self) {
        let highest = self.sections.iter().flatten().map(|folder| folder.id.0).max();
        self.next_id = highest.map_or(0, |id| id.saturating_add(1));
    }

    /// Blank or duplicated names inside a section, or an id used twice
    /// anywhere, would make a folder unnameable by what a file records for it.
    pub(crate) fn validate(&self) -> Result<()> {
        let mut ids = std::collections::HashSet::new();
        for section in SectionKind::ALL {
            let mut names = std::collections::HashSet::new();
            for folder in self.folders(section) {
                if folder.name.trim().is_empty() {
                    bail!("{} has a folder with a blank name", section.key());
                }
                if !names.insert(folder.name.as_str()) {
                    bail!("duplicate {} folder '{}'", section.key(), folder.name);
                }
                if !ids.insert(folder.id) {
                    bail!("duplicate folder id {} ('{}')", folder.id.0, folder.name);
                }
            }
        }
        Ok(())
    }

    /// Fold every folder name, in section and creation order, into a content
    /// fingerprint. Ids are left out: minted on load, they mean nothing to a
    /// file, so a reopened project is not dirty for having fresh ids.
    pub(crate) fn hash_into(&self, hasher: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        for section in SectionKind::ALL {
            section.hash(hasher);
            for folder in self.folders(section) {
                folder.name.hash(hasher);
            }
        }
    }

    /// The names in one section, in order - the shape a file records.
    pub(crate) fn names(&self, section: SectionKind) -> Vec<&str> {
        self.folders(section).iter().map(|folder| folder.name.as_str()).collect()
    }
}
