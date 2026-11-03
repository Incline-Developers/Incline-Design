//! The Planning workspace's Reserves view: everything the setup has generated.
//!
//! Where the Setup pages configure a solid, this one is for looking through
//! what came out - the solids grouped by what they are, each opening into its
//! benches and those into their flitches - and reading off what any selection
//! of them holds.

use std::borrow::Cow;

use crate::{
    i18n::tr,
    model::{Document, ExclusionTarget, SolidKind},
    ui::{
        EditorState, UiProjectView,
        dialogs::solids::{block_model_label, kind_label},
        state::{BenchSelection, BlastShapeRef, SolidsViewRow, SolidsVisibility, UiCommand},
        unthemed_icon,
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup},
            data_grid::PropertyTable,
            explorer::{ExplorerEntry, ExplorerHeader, explorer_note, paint_fixed_stripes, reserve_fixed_stripes},
        },
    },
};

/// Which page a Solids tree is drawn for. Every page lists the same rows -
/// kind, solid, bench, blast, then flitch - drawn the same way.
///
/// Each page keeps its own selection and, where its rows carry eyes, its own
/// hidden set: hiding ground to see a block on Animate, or to click the right
/// one on the Road Network, leaves the Reserves figures and the other pages
/// alone. The Solids pages share one selection, so a blast picked on one is
/// the blast the next one opens on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SolidsTree {
    /// The Reserves view, whose selection is what the properties beside it read.
    Reserves,
    /// The sequence editors: the Reserves tree with an eye on every row, which
    /// hides its ground from their preview - see [`EditorState::sequence_hidden`].
    Sequence,
    /// The Blasting step: the tree stopping at benches, since a blast divides
    /// a bench and flitches have nothing to say about it.
    Benches,
    /// Animate's Solids Navigation. Clicking a solid frames it.
    Animation,
    /// The Road Network's, each row coloured by whether its dug blocks reach
    /// a road. Clicking a solid frames its blocks.
    Haulage,
}

impl SolidsTree {
    /// Whether this is one of the Solids pages' trees: it shares their
    /// selection, a picked blast included, and offers the menu that takes
    /// ground out of mining.
    fn is_solids_page(self) -> bool {
        matches!(self, Self::Reserves | Self::Sequence | Self::Benches)
    }

    fn shows_flitches(self) -> bool {
        self != Self::Benches
    }

    /// The page's hidden set, where its rows carry eyes.
    fn hidden(self, editor: &EditorState) -> Option<&SolidsVisibility> {
        match self {
            Self::Reserves | Self::Benches => None,
            Self::Sequence => Some(&editor.sequence_hidden),
            Self::Animation => Some(&editor.schedule_animation_hidden),
            Self::Haulage => Some(&editor.haul_hidden),
        }
    }

    fn hidden_mut(self, editor: &mut EditorState) -> Option<&mut SolidsVisibility> {
        match self {
            Self::Reserves | Self::Benches => None,
            Self::Sequence => Some(&mut editor.sequence_hidden),
            Self::Animation => Some(&mut editor.schedule_animation_hidden),
            Self::Haulage => Some(&mut editor.haul_hidden),
        }
    }

    fn selection(self, editor: &EditorState) -> &[SolidsViewRow] {
        match self {
            Self::Reserves | Self::Sequence | Self::Benches => &editor.solids_view_selection,
            Self::Animation => &editor.schedule_animation_selection,
            Self::Haulage => &editor.haul_navigation_selection,
        }
    }

    fn selection_mut(self, editor: &mut EditorState) -> &mut Vec<SolidsViewRow> {
        match self {
            Self::Reserves | Self::Sequence | Self::Benches => &mut editor.solids_view_selection,
            Self::Animation => &mut editor.schedule_animation_selection,
            Self::Haulage => &mut editor.haul_navigation_selection,
        }
    }
}

/// The icon a kind of solid is listed under.
pub(crate) fn kind_icon(kind: SolidKind) -> egui::ImageSource<'static> {
    match kind {
        SolidKind::Pit => unthemed_icon!("solid_pit.svg"),
        SolidKind::Dump => unthemed_icon!("solid_dump.svg"),
        SolidKind::Stockpile => unthemed_icon!("solid_stockpile.svg"),
    }
}

/// A solid's icon: the triangulated body it is.
fn solid_icon() -> egui::ImageSource<'static> {
    unthemed_icon!("triangulation.svg")
}

fn bench_icon() -> egui::ImageSource<'static> {
    unthemed_icon!("bench.svg")
}

fn blast_icon() -> egui::ImageSource<'static> {
    unthemed_icon!("blast.svg")
}

fn flitch_icon() -> egui::ImageSource<'static> {
    unthemed_icon!("flitch.svg")
}

/// Draw `tree` in a scroll area of its own, filling its pane.
pub(crate) fn draw_tree(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, tree: SolidsTree, commands: &mut Vec<UiCommand>) {
    ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .min_scrolled_height(0.0)
        .show(ui, |ui| draw_rows(ui, editor, document, tree, commands));
}

/// The rows of `tree`, for a pane that scrolls them itself, such as the table
/// stack down the Road Network's column.
///
/// Rows are selected the way a file list is - click to replace the selection,
/// ctrl-click to add to it - because the figures beside the Reserves tree are
/// about however many rows are picked, not just one. Clicking a parent picks
/// everything under it, so the arrows are what collapse a row rather than its
/// label; benches start collapsed, since a pit has tens of flitches and the
/// benches are what the tree is read for.
pub(crate) fn draw_rows(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, tree: SolidsTree, commands: &mut Vec<UiCommand>) {
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
    let (slot, top) = reserve_fixed_stripes(ui);
    if document.solids().is_empty() {
        explorer_note(ui, tr!("solids-no-solids-yet-add"));
    }
    // A selection made from outside the tree opens the rows it names; only
    // the Solids pages make one.
    let reveal = tree.is_solids_page() && std::mem::take(&mut editor.solids_tree_reveal);
    let mut pass = TreePass {
        tree,
        editor,
        reveal,
        // Edited on a copy, because the rows read the editor as they draw,
        // and written back once the tree is done.
        hidden: tree.hidden(editor).cloned(),
        clicked: None,
        framed: None,
    };
    for kind in SolidKind::ALL {
        pass.kind(ui, document, kind, commands);
    }
    paint_fixed_stripes(ui, slot, top, crate::ui::widgets::tree_row_colors(ui).1);
    let TreePass { hidden, clicked, framed, .. } = pass;

    if let Some(hidden) = hidden
        && tree.hidden(editor) != Some(&hidden)
        && let Some(slot) = tree.hidden_mut(editor)
    {
        *slot = hidden;
        ui.ctx().request_repaint();
    }
    if let Some((rows, blast)) = clicked {
        let extend = ui.input(|input| input.modifiers.command || input.modifiers.shift);
        if tree.is_solids_page() {
            if blast.is_some() && blast != editor.selected_blast && !extend {
                editor.solids_view_selection = rows;
            } else {
                apply_selection(&mut editor.solids_view_selection, &rows, extend);
            }
            editor.selected_blast = blast.filter(|_| !editor.solids_view_selection.is_empty());
            editor.selected_dig_block = None;
        } else {
            apply_selection(tree.selection_mut(editor), &rows, extend);
        }
        ui.ctx().request_repaint();
    }
    match (tree, framed) {
        (SolidsTree::Animation, Some(solid)) => commands.push(UiCommand::FocusScheduleAnimationSolid(solid)),
        (SolidsTree::Haulage, Some(solid)) => {
            let points: Vec<_> = editor
                .haul_blocks
                .iter()
                .filter(|b| b.solid == solid)
                .flat_map(|b| b.rings.iter().flatten().copied())
                .collect();
            if let Some(min) = points.iter().copied().reduce(glam::DVec3::min)
                && let Some(max) = points.iter().copied().reduce(glam::DVec3::max)
            {
                commands.push(UiCommand::FrameHaul(min, max));
            }
        }
        _ => {}
    }
}

/// One drawing of a Solids tree: what it reads, and what its rows asked for,
/// applied once every row is drawn.
struct TreePass<'a> {
    tree: SolidsTree,
    editor: &'a EditorState,
    reveal: bool,
    /// The page's hidden set as the eyes leave it, where it has one.
    hidden: Option<SolidsVisibility>,
    /// The rows a click picked, and the blast it picked them through.
    clicked: Option<(Vec<SolidsViewRow>, Option<BlastShapeRef>)>,
    /// A solid whose row was clicked, for the pages that frame it.
    framed: Option<crate::model::SolidId>,
}

impl<'a> TreePass<'a> {
    /// A row's id, kept apart per page so each keeps its own open rows.
    fn id(&self, key: impl std::hash::Hash + std::fmt::Debug) -> egui::Id {
        egui::Id::unique(("solids_tree", self.tree, key))
    }

    /// The bands a solid's rows are listed from. The Road Network lists the
    /// flitches its dig blocks are on; every other page the bands the solid
    /// was generated with, gathered only while a page that draws them is open.
    fn occupied(&self, solid: &crate::model::Solid) -> Option<Cow<'a, [BenchSelection]>> {
        let editor = self.editor;
        if self.tree == SolidsTree::Haulage && editor.haul_blocks.iter().any(|b| b.solid == solid.id) {
            let mut flitches: Vec<BenchSelection> = Vec::new();
            for block in editor.haul_blocks.iter().filter(|b| b.solid == solid.id) {
                if !flitches.contains(&block.flitch) {
                    flitches.push(block.flitch);
                }
            }
            return Some(Cow::Owned(flitches));
        }
        editor.solid_view_bands.get(&solid.id).map(|bands| Cow::Borrowed(bands.as_slice()))
    }

    /// On the Road Network, the colour of a row whose dug blocks - those
    /// `under` it - all reach a road, or the warning's when any is out of
    /// reach. Nothing where nothing under the row is dug, or on other pages.
    fn reach(&self, under: impl Fn(&crate::ui::state::HaulBlock) -> bool) -> Option<egui::Color32> {
        if self.tree != SolidsTree::Haulage {
            return None;
        }
        let mut dug = self.editor.haul_blocks.iter().filter(|b| b.dug && under(b)).peekable();
        dug.peek()?;
        Some(if dug.all(|b| b.connected) { REACHED_COLOR } else { UNREACHED_COLOR })
    }

    fn reach_label(&self, text: String, under: impl Fn(&crate::ui::state::HaulBlock) -> bool) -> egui::WidgetText {
        match self.reach(under) {
            Some(color) => egui::RichText::new(text).color(color).into(),
            None => text.into(),
        }
    }

    /// A bench or blast row's name, marked when it is out of mining on the
    /// pages that put it there.
    fn exclusion_label(&self, name: &str, solid: &crate::model::Solid, target: ExclusionTarget) -> String {
        exclusion_label(name, self.tree.is_solids_page() && solid.exclusions.is_target_excluded(target))
    }

    fn kind(&mut self, ui: &mut egui::Ui, document: &Document, kind: SolidKind, commands: &mut Vec<UiCommand>) {
        let editor = self.editor;
        let solids: Vec<_> = document
            .solids()
            .iter()
            .filter(|solid| solid.kind == kind)
            .map(|solid| (solid, self.occupied(solid)))
            .collect();
        if solids.is_empty() {
            return;
        }
        let selection = self.tree.selection(editor);
        let rows: Vec<_> = solids
            .iter()
            .flat_map(|(solid, occupied)| descendants(solid, occupied.as_deref(), self.tree.shows_flitches()))
            .collect();
        let id = self.id(("kind", kind as u8));
        if self.reveal && solids.iter().any(|(solid, _)| selection.iter().any(|row| row.solid == solid.id)) {
            open_row(ui, id);
        }
        let visible = self.hidden.as_ref().map(|hidden| solids.iter().any(|(solid, _)| !hidden.solids.contains(&solid.id)));
        let mut header = ExplorerHeader::new(id, kind_label(kind))
            .icon(kind_icon(kind))
            .collapse_on_click(false)
            .selected(group_selected(selection, &rows));
        if let Some(visible) = visible {
            header = header.visibility_toggle(visible);
        }
        let (_, heading, _, visibility_clicked) = header.show_with_visibility(ui, |ui| {
            for (solid, occupied) in &solids {
                self.solid(ui, solid, occupied.as_deref(), commands);
            }
        });
        if heading.inner.clicked() {
            self.clicked = Some((rows, None));
        }
        if visibility_clicked && let Some(hidden) = self.hidden.as_mut() {
            for (solid, _) in &solids {
                set_solid_visible(hidden, solid.id, !visible.unwrap_or(true));
            }
        }
    }

    fn solid(&mut self, ui: &mut egui::Ui, solid: &crate::model::Solid, occupied: Option<&[BenchSelection]>, commands: &mut Vec<UiCommand>) {
        let editor = self.editor;
        let selection = self.tree.selection(editor);
        let rows = descendants(solid, occupied, self.tree.shows_flitches());
        let id = self.id(("solid", solid.id.0));
        if self.reveal && selection.iter().any(|row| row.solid == solid.id) {
            open_row(ui, id);
        }
        // Where it picks ground to dig, the tree leaves out what is no
        // longer mined; the Solids pages list it, to put it back.
        let mined = editor.mined_ground.as_deref();
        let benches: Vec<_> = solid
            .benching
            .benches()
            .into_iter()
            .rev()
            .filter(|bench| holds(occupied, bench.base, bench.top()))
            .filter(|bench| mined.is_none_or(|mined| mined.bench(solid.id, bench.base)))
            .collect();
        // Every bench row the tree lists, which showing one bench of a hidden
        // solid hides one by one.
        let bench_rows: Vec<_> = benches.iter().map(|bench| band_row(solid.id, bench.base, bench.top(), false)).collect();
        let visible = self.hidden.as_ref().map(|hidden| !hidden.solids.contains(&solid.id));
        let mut header = ExplorerHeader::new(id, solid.name.clone())
            .icon(solid_icon())
            .collapse_on_click(false)
            .selected(group_selected(selection, &rows));
        if let Some(color) = self.reach(|b| b.solid == solid.id) {
            header = header.color(color);
        }
        if let Some(visible) = visible {
            header = header.visibility_toggle(visible);
        }
        let (_, heading, _, visibility_clicked) = header.show_with_visibility(ui, |ui| {
            if occupied.is_none() {
                explorer_note(ui, tr!("planning-solid-geometry-pending"));
            }
            for (bench, &row) in benches.iter().zip(&bench_rows) {
                self.bench(ui, solid, occupied, bench, row, &bench_rows, visible, commands);
            }
        });
        if heading.inner.clicked() {
            self.clicked = Some((rows, None));
            self.framed = Some(solid.id);
        }
        if visibility_clicked && let Some(hidden) = self.hidden.as_mut() {
            set_solid_visible(hidden, solid.id, !visible.unwrap_or(true));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn bench(
        &mut self,
        ui: &mut egui::Ui,
        solid: &crate::model::Solid,
        occupied: Option<&[BenchSelection]>,
        bench: &crate::model::Bench,
        row: SolidsViewRow,
        benches: &[SolidsViewRow],
        solid_visible: Option<bool>,
        commands: &mut Vec<UiCommand>,
    ) {
        let editor = self.editor;
        let tree = self.tree;
        let mined = editor.mined_ground.as_deref();
        let flitch_rows: Vec<_> = if tree.shows_flitches() {
            bench
                .flitches
                .iter()
                .rev()
                .filter(|flitch| holds(occupied, flitch.base, flitch.top()))
                .map(|flitch| band_row(solid.id, flitch.base, flitch.top(), true))
                .collect()
        } else {
            Vec::new()
        };
        // What a click on the bench picks: the bench and its flitches.
        let group: Vec<_> = std::iter::once(row).chain(flitch_rows.iter().copied()).collect();
        let blasts = solid.blasting.bench(bench.base).map(|entry| entry.blasts.as_slice()).unwrap_or_default();
        let blast_refs: Vec<_> = blasts.iter().map(|blast| BlastShapeRef::new(solid.id, bench.base, blast.anchor)).collect();
        let family = Family {
            benches,
            bench: row,
            flitches: &flitch_rows,
            blasts: &blast_refs,
        };
        let target = ExclusionTarget::Bench(bench.base);
        let text = self.exclusion_label(&format_rl(bench.base), solid, target);
        let label = self.reach_label(text.clone(), |b| b.solid == solid.id && b.bench.base == bench.base);
        let selected = group_selected(tree.selection(editor), &group);
        let visible = self.hidden.as_ref().zip(solid_visible).map(|(hidden, visible)| visible && !hidden.rows.contains(&row));
        let id = self.id(("bench", solid.id.0, bench.base.to_bits()));
        let (response, visibility_clicked) = if tree.shows_flitches() {
            branch_row(ui, id, bench_icon(), label, selected, visible, |ui| {
                if blasts.is_empty() {
                    explorer_note(ui, tr!("planning-solid-geometry-pending"));
                    return;
                }
                for (index, (blast, &shape)) in blasts.iter().zip(&blast_refs).enumerate() {
                    if mined.is_some_and(|mined| !mined.blast(shape)) {
                        continue;
                    }
                    self.blast(ui, solid, bench.base, &family, &group, index, blast, shape, selected, visible, commands);
                }
            })
        } else {
            leaf_row(ui, id, bench_icon(), label, selected, visible)
        };
        if tree.is_solids_page() {
            exclusion_menu(&response, &text, solid, target, commands);
        }
        if response.clicked() {
            self.clicked = Some((group, None));
        }
        if visibility_clicked && let Some(hidden) = self.hidden.as_mut() {
            set_bench_visible(hidden, &family, !visible.unwrap_or(true));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn blast(
        &mut self,
        ui: &mut egui::Ui,
        solid: &crate::model::Solid,
        bench_base: f64,
        family: &Family<'_>,
        bench_group: &[SolidsViewRow],
        index: usize,
        blast: &crate::model::BlastShape,
        shape: BlastShapeRef,
        bench_selected: bool,
        bench_visible: Option<bool>,
        commands: &mut Vec<UiCommand>,
    ) {
        let editor = self.editor;
        let tree = self.tree;
        let selection = tree.selection(editor);
        let mined = editor.mined_ground.as_deref();
        // Only the Solids pages pick a blast: elsewhere a blast row stands
        // for its bench's selection.
        let picked = if tree.is_solids_page() { editor.selected_blast } else { None };
        let selected = picked == Some(shape) || (picked.is_none() && bench_selected);
        let target = ExclusionTarget::Blast {
            bench: bench_base,
            anchor: blast.anchor,
        };
        let text = self.exclusion_label(&blast.name, solid, target);
        let label = self.reach_label(text.clone(), |b| b.blast == Some(shape));
        let visible = self.hidden.as_ref().zip(bench_visible).map(|(hidden, visible)| visible && !hidden.blasts.contains(&shape));
        let id = self.id(("blast", solid.id.0, bench_base.to_bits(), index));
        let (response, visibility_clicked) = branch_row(ui, id, blast_icon(), label, selected, visible, |ui| {
            for &row in family.flitches {
                let Some(flitch) = row.band else { continue };
                if mined.is_some_and(|mined| !mined.flitch(shape, flitch.base)) {
                    continue;
                }
                let flitch_visible = self
                    .hidden
                    .as_ref()
                    .zip(visible)
                    .map(|(hidden, visible)| visible && !hidden.hides_flitch(shape, flitch.base));
                let flitch_selected = group_selected(selection, std::slice::from_ref(&row)) && (picked.is_none() || picked == Some(shape));
                let label = self.reach_label(format_rl(flitch.base), |b| b.blast == Some(shape) && b.flitch.base == flitch.base);
                let id = self.id(("flitch", solid.id.0, bench_base.to_bits(), index, flitch.base.to_bits()));
                let (response, visibility_clicked) = leaf_row(ui, id, flitch_icon(), label, flitch_selected, flitch_visible);
                if response.clicked() {
                    self.clicked = Some((vec![row], tree.is_solids_page().then_some(shape)));
                }
                if visibility_clicked && let Some(hidden) = self.hidden.as_mut() {
                    set_flitch_visible(hidden, family, shape, flitch.base, !flitch_visible.unwrap_or(true));
                }
            }
        });
        if tree.is_solids_page() {
            exclusion_menu(&response, &text, solid, target, commands);
            if response.clicked() {
                self.clicked = Some((bench_group.to_vec(), Some(shape)));
            }
        }
        if visibility_clicked && let Some(hidden) = self.hidden.as_mut() {
            set_blast_visible(hidden, family, shape, !visible.unwrap_or(true));
        }
    }
}

/// A bench or flitch row of a solid.
fn band_row(solid: crate::model::SolidId, base: f64, top: f64, is_flitch: bool) -> SolidsViewRow {
    SolidsViewRow {
        solid,
        band: Some(BenchSelection { base, top, is_flitch }),
    }
}

/// The green a Road Network row takes when all its dug blocks reach a road.
const REACHED_COLOR: egui::Color32 = egui::Color32::from_rgb(72, 170, 110);
/// The yellow it takes when any is out of reach: the warning badge's.
const UNREACHED_COLOR: egui::Color32 = egui::Color32::from_rgb(0xE5, 0xB6, 0x2F);

/// Show or hide a whole solid. Either way what was hidden under it goes:
/// a parent's eye sets everything below it, so showing it again brings
/// back every bench, blast and flitch rather than only those not hidden
/// on their own before.
fn set_solid_visible(hidden: &mut SolidsVisibility, solid: crate::model::SolidId, visible: bool) {
    hidden.rows.retain(|row| row.solid != solid);
    hidden.blasts.retain(|blast| blast.solid != solid);
    hidden.flitches.retain(|(blast, _)| blast.solid != solid);
    if visible {
        hidden.solids.remove(&solid);
    } else {
        hidden.solids.insert(solid);
    }
}

/// The rows around one bench of a solid: every bench the tree lists for the
/// solid, and the bench's own flitches and blasts.
///
/// Showing one row under a hidden parent shows only that row: the parent's
/// hiding is handed down to the rest of its children instead of dropped.
struct Family<'a> {
    benches: &'a [SolidsViewRow],
    bench: SolidsViewRow,
    flitches: &'a [SolidsViewRow],
    blasts: &'a [BlastShapeRef],
}

impl Family<'_> {
    /// A solid hidden whole becomes its other benches, hidden one by one.
    /// Whether it was hidden, so the caller can hand the hiding further down.
    fn open_solid(&self, hidden: &mut SolidsVisibility) -> bool {
        let was_hidden = hidden.solids.remove(&self.bench.solid);
        if was_hidden {
            for bench in self.benches.iter().filter(|row| **row != self.bench) {
                if !hidden.rows.contains(bench) {
                    hidden.rows.push(*bench);
                }
            }
        }
        was_hidden
    }

    /// Open the solid and the bench above `blast`. A bench hidden whole - on
    /// its own or with its solid - becomes its other blasts, hidden one by
    /// one, so only `blast` comes back rather than the whole RL.
    fn open_bench(&self, hidden: &mut SolidsVisibility, blast: BlastShapeRef) {
        let solid_was_hidden = self.open_solid(hidden);
        let bench_was_hidden = hidden.rows.contains(&self.bench);
        hidden.rows.retain(|hidden| *hidden != self.bench);
        if solid_was_hidden || bench_was_hidden {
            hidden.blasts.extend(self.blasts.iter().filter(|other| **other != blast));
        }
    }
}

/// A bench's eye sets its blasts and flitches with it, as a solid's does.
fn set_bench_visible(hidden: &mut SolidsVisibility, family: &Family<'_>, visible: bool) {
    hidden.rows.retain(|hidden| *hidden != family.bench && !family.flitches.contains(hidden));
    hidden.blasts.retain(|blast| !family.blasts.contains(blast));
    hidden.flitches.retain(|(blast, _)| !family.blasts.contains(blast));
    if visible {
        family.open_solid(hidden);
    } else {
        hidden.rows.push(family.bench);
    }
}

fn set_blast_visible(hidden: &mut SolidsVisibility, family: &Family<'_>, blast: BlastShapeRef, visible: bool) {
    // A blast's eye sets its flitches with it.
    hidden.flitches.retain(|(hidden, _)| *hidden != blast);
    if !visible {
        hidden.blasts.insert(blast);
        return;
    }
    family.open_bench(hidden, blast);
    hidden.blasts.remove(&blast);
}

/// One flitch under one blast. Showing it under a hidden blast, bench or
/// solid opens each of them, and hands their hiding down to everything
/// else they hold, so only this flitch comes back.
fn set_flitch_visible(hidden: &mut SolidsVisibility, family: &Family<'_>, blast: BlastShapeRef, flitch_base: f64, visible: bool) {
    let key = (blast, flitch_base.to_bits());
    if !visible {
        hidden.flitches.insert(key);
        return;
    }
    family.open_bench(hidden, blast);
    if hidden.blasts.remove(&blast) {
        for flitch in family.flitches.iter().filter_map(|row| row.band) {
            hidden.flitches.insert((blast, flitch.base.to_bits()));
        }
    }
    hidden.flitches.remove(&key);
}

/// Dig Strips chooses a whole flitch, independently of blast partitions.
///
/// Only the ground the solid actually occupies is offered: a bench or solid
/// the benching plan names but the body never reaches has no flitch to draw
/// strips on, so it is left out rather than opening onto nothing.
pub(crate) fn draw_flitch_tree(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document) {
    // Banded and full-height like every other tree: its island is a region of
    // its own, so a short list has to fill it rather than shrink it away.
    ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
    let reveal = std::mem::take(&mut editor.solids_tree_reveal);
    egui::ScrollArea::vertical().auto_shrink([false; 2]).min_scrolled_height(0.0).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        let (slot, top) = reserve_fixed_stripes(ui);
        if document.solids().is_empty() {
            explorer_note(ui, tr!("solids-no-solids-yet-add"));
        }
        let mut clicked = None;
        for kind in SolidKind::ALL {
            let solids: Vec<_> = document
                .solids()
                .iter()
                .filter(|solid| solid.kind == kind)
                .filter(|solid| {
                    let occupied = editor.solid_view_bands.get(&solid.id).map(Vec::as_slice);
                    occupied.is_none() || solid.benching.benches().iter().any(|bench| holds(occupied, bench.base, bench.top()))
                })
                .collect();
            if solids.is_empty() {
                continue;
            }
            let kind_id = egui::Id::unique(("strip_kind", kind as u8));
            if reveal && solids.iter().any(|solid| editor.solids_view_selection.iter().any(|row| row.solid == solid.id)) {
                open_row(ui, kind_id);
            }
            ExplorerHeader::new(kind_id, kind_label(kind)).icon(kind_icon(kind)).show(ui, |ui| {
                for solid in solids {
                    let occupied = editor.solid_view_bands.get(&solid.id).map(Vec::as_slice);
                    let solid_id = egui::Id::unique(("strip_solid", solid.id.0));
                    let picked = editor.solids_view_selection.iter().find(|row| row.solid == solid.id).and_then(|row| row.band);
                    if reveal && picked.is_some() {
                        open_row(ui, solid_id);
                    }
                    ExplorerHeader::new(solid_id, solid.name.clone()).icon(solid_icon()).show(ui, |ui| {
                        if occupied.is_none() {
                            explorer_note(ui, tr!("planning-solid-geometry-pending"));
                        }
                        for bench in solid.benching.benches().iter().rev().filter(|bench| holds(occupied, bench.base, bench.top())) {
                            // Flitches Blasting took wholly out of mining have
                            // nothing to draw strips on, and a bench left with
                            // none goes too.
                            let flitches: Vec<_> = bench
                                .flitches
                                .iter()
                                .rev()
                                .filter(|flitch| holds(occupied, flitch.base, flitch.top()))
                                .filter(|flitch| !editor.dig_excluded_flitches.contains(&(solid.id, flitch.base.to_bits())))
                                .collect();
                            if flitches.is_empty() {
                                continue;
                            }
                            let bench_id = egui::Id::unique(("strip_bench", solid.id.0, bench.base.to_bits()));
                            if reveal && picked.is_some_and(|band| bench.contains_rl(band.base)) {
                                open_row(ui, bench_id);
                            }
                            // Only a flitch is drawn on, so the bench row just
                            // opens and closes.
                            let (response, _) = branch_row(ui, bench_id, bench_icon(), format_rl(bench.base), false, None, |ui| {
                                for flitch in flitches.iter() {
                                    let row = band_row(solid.id, flitch.base, flitch.top(), true);
                                    let (response, _) = leaf_row(
                                        ui,
                                        egui::Id::unique(("strip_flitch", solid.id.0, flitch.base.to_bits())),
                                        flitch_icon(),
                                        format_rl(flitch.base),
                                        editor.solids_view_selection == [row],
                                        None,
                                    );
                                    if response.clicked() {
                                        clicked = Some(row);
                                    }
                                }
                            });
                            if response.clicked() {
                                let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), bench_id, false);
                                state.toggle(ui);
                                state.store(ui.ctx());
                            }
                        }
                    });
                }
            });
        }
        paint_fixed_stripes(ui, slot, top, crate::ui::widgets::tree_row_colors(ui).1);
        if let Some(row) = clicked {
            editor.solids_view_selection = vec![row];
            editor.selected_blast = None;
            ui.ctx().request_repaint();
        }
    });
}

/// Open a closed row, for a selection made from outside the tree.
fn open_row(ui: &egui::Ui, id: egui::Id) {
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
    state.set_open(true);
    state.store(ui.ctx());
}

/// Whether a band of the solid was actually generated, so the tree lists the
/// RLs the solid occupies rather than every RL the plan could name.
pub(crate) fn holds(occupied: Option<&[BenchSelection]>, base: f64, top: f64) -> bool {
    occupied.is_some_and(|bands| bands.iter().any(|band| band.top > base + 1e-6 && band.base < top - 1e-6))
}

/// Every row a click on a solid stands for: the solid itself, and each of the
/// benches and flitches it occupies. Picking a parent picks what is under it.
fn descendants(solid: &crate::model::Solid, occupied: Option<&[BenchSelection]>, include_flitches: bool) -> Vec<SolidsViewRow> {
    let mut rows = vec![SolidsViewRow { solid: solid.id, band: None }];
    for bench in solid.benching.benches() {
        let flitches = include_flitches
            .then(|| bench.flitches.iter().map(|flitch| (flitch.base, flitch.top(), true)))
            .into_iter()
            .flatten();
        for (base, top, is_flitch) in std::iter::once((bench.base, bench.top(), false)).chain(flitches) {
            if holds(occupied, base, top) {
                rows.push(SolidsViewRow {
                    solid: solid.id,
                    band: Some(BenchSelection { base, top, is_flitch }),
                });
            }
        }
    }
    rows
}

/// Whether a group reads as selected: every row of it is picked, or the bare
/// solid row that stands for the whole solid is. Picking a solid before its
/// geometry has finished still highlights the benches that then appear.
fn group_selected(selection: &[SolidsViewRow], rows: &[SolidsViewRow]) -> bool {
    !rows.is_empty()
        && rows
            .iter()
            .all(|row| selection.contains(row) || (row.band.is_some() && selection.contains(&SolidsViewRow { solid: row.solid, band: None })))
}

/// Ground named the one way the whole app names it: solid, bench RL, blast,
/// flitch RL and dig block, joined by slashes and cut off at whatever is being
/// named - `Pit A/392` is a bench, `Pit A/392/1` a blast, `Pit A/392/1/396/23`
/// a dig block. Elevations go bare: their place in the path says what they are.
pub(crate) fn ground_path<S: AsRef<str>>(parts: &[S]) -> String {
    parts.iter().map(AsRef::as_ref).collect::<Vec<_>>().join("/")
}

/// A bench by [`ground_path`].
pub(crate) fn bench_path(solid: &str, bench: f64) -> String {
    ground_path(&[solid, &format_rl(bench)])
}

/// A blast by [`ground_path`]: its name is only unique on its bench.
pub(crate) fn blast_path(solid: &str, bench: f64, blast: &str) -> String {
    ground_path(&[solid, &format_rl(bench), blast])
}

/// A flitch by [`ground_path`]. A flitch runs under every blast of its bench,
/// so the blast's place in the path is a wildcard: `Pit A/348/*/344`.
pub(crate) fn flitch_path(solid: &str, bench: f64, flitch: f64) -> String {
    ground_path(&[solid, &format_rl(bench), "*", &format_rl(flitch)])
}

/// A plan area to the square metre, thousands separated: `1,124 m²`.
pub(crate) fn format_area(square_metres: f64) -> String {
    use thousands::Separable;
    format!("{} m²", format!("{square_metres:.0}").separate_with_commas())
}

/// An RL with no more decimals than it needs: 348, 348.5, 348.25.
pub(crate) fn format_rl(value: f64) -> String {
    if !value.is_finite() {
        return String::from("—");
    }
    let text = format!("{value:.2}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed == "-0" { "0".to_owned() } else { trimmed.to_owned() }
}

/// A selectable tree row that opens into children, led by `icon` and ending
/// in an eye showing `visible` when there is one. Returns the row's response
/// and whether the eye was clicked.
///
/// The arrow is the only thing that collapses it: clicking the row itself
/// selects it *and* its children, because the figures beside the tree are
/// read across whatever is picked.
fn branch_row(
    ui: &mut egui::Ui,
    id: egui::Id,
    icon: egui::ImageSource<'static>,
    label: impl Into<egui::WidgetText>,
    selected: bool,
    visible: Option<bool>,
    body: impl FnOnce(&mut egui::Ui),
) -> (egui::Response, bool) {
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
    let header = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);
            let entry = ExplorerEntry::new(id.with("row"), label).leading_icon(icon, egui::Color32::WHITE).selected(selected);
            match visible {
                Some(visible) => entry.visibility_toggle(visible),
                None => entry,
            }
            .show(ui)
        })
        .inner;
    state.show_body_indented(&header.response, ui, body);
    (header.response, header.visibility_clicked)
}

/// A tree row with nothing under it, gutter-aligned with the rows that have
/// an arrow so the column of icons and labels stays straight.
fn leaf_row(
    ui: &mut egui::Ui,
    id: egui::Id,
    icon: egui::ImageSource<'static>,
    label: impl Into<egui::WidgetText>,
    selected: bool,
    visible: Option<bool>,
) -> (egui::Response, bool) {
    let entry = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let entry = ExplorerEntry::new(id, label)
                .reserve_toggle_gutter(true)
                .leading_icon(icon, egui::Color32::WHITE)
                .selected(selected);
            match visible {
                Some(visible) => entry.visibility_toggle(visible),
                None => entry,
            }
            .show(ui)
        })
        .inner;
    (entry.response, entry.visibility_clicked)
}

/// A bench or blast row's label, saying so when it is out of mining.
fn exclusion_label(label: &str, excluded: bool) -> String {
    if excluded {
        tr!("planning-excluded-label", name = label.to_owned())
    } else {
        label.to_owned()
    }
}

/// The row's menu: take this ground out of mining, or put it back.
fn exclusion_menu(response: &egui::Response, title: &str, solid: &crate::model::Solid, target: crate::model::ExclusionTarget, commands: &mut Vec<UiCommand>) {
    let excluded = solid.exclusions.is_target_excluded(target);
    context_menu_popup(response, title, |ui| {
        if ContextMenuAction::new(tr!("planning-exclude-from-mining")).checked(excluded).show(ui).clicked() {
            commands.push(UiCommand::UpdateSolid {
                solid: solid.id,
                edit: crate::model::SolidEdit::Exclusions(solid.exclusions.with(target, !excluded)),
            });
            ui.close();
        }
    });
}

/// Click replaces the selection with `rows`; ctrl- or shift-click toggles the
/// whole group in it. Clicking a row that is already the entire selection
/// clears it, so a parent is a toggle for everything beneath it.
fn apply_selection(selection: &mut Vec<SolidsViewRow>, rows: &[SolidsViewRow], extend: bool) {
    if !extend {
        let only = selection.len() == rows.len() && rows.iter().all(|row| selection.contains(row));
        selection.clear();
        if !only {
            selection.extend_from_slice(rows);
        }
        return;
    }
    if rows.iter().all(|row| selection.contains(row)) {
        selection.retain(|row| !rows.contains(row));
    } else {
        for row in rows {
            if !selection.contains(row) {
                selection.push(*row);
            }
        }
    }
}

/// What a property reads across the selection: one shared value, or several.
fn shared<T: PartialEq, I: IntoIterator<Item = T>>(values: I) -> Option<Option<T>> {
    let mut iter = values.into_iter();
    let first = iter.next()?;
    Some(if iter.all(|value| value == first) { Some(first) } else { None })
}

/// Render one property, saying "Multiple" when the selection disagrees.
fn read_across<T: PartialEq>(values: Vec<T>, describe: impl Fn(&T) -> String) -> String {
    match shared(values) {
        None => String::from("—"),
        Some(None) => tr!("solids-multiple"),
        Some(Some(value)) => describe(&value),
    }
}

/// The properties of whatever is selected, and what it holds.
/// What the selection names: its type, the solid it belongs to, the bench and
/// flitch it sits in.
///
/// Paired with [`draw_contents`] below it, in a pane of its own: the two are
/// read together but they grow at different rates - this one is a fixed set of
/// rows, and the figures below grow with the Field List - so the split between
/// them is a seam the user sets rather than a height computed here.
pub(crate) fn draw_properties(ui: &mut egui::Ui, rect: egui::Rect, editor: &EditorState, project: &UiProjectView, document: &Document) {
    let rows: Vec<_> = editor
        .solids_view_selection
        .iter()
        .filter_map(|row| document.solid(row.solid).map(|solid| (solid, row.band)))
        .collect();
    let title = tr!("planning-properties");
    if rows.is_empty() {
        PropertyTable::new("solids_view_properties", rect, &title).show(ui, |table| {
            table.header(&tr!("planning-property"), &tr!("planning-value"));
            table.readonly(&tr!("solids-selection"), &tr!("solids-select-solid-bench-flitch"), None, None);
        });
        return;
    }

    PropertyTable::new("solids_view_properties", rect, &title).show(ui, |table| {
        table.header(&tr!("planning-property"), &tr!("planning-value"));
        table.readonly(
            &tr!("destination-type"),
            &read_across(rows.iter().map(|(solid, _)| solid.kind).collect(), |kind| kind_label(*kind)),
            None,
            None,
        );
        table.readonly(
            &tr!("tri-type-solid-closed"),
            &read_across(rows.iter().map(|(solid, _)| solid.name.clone()).collect(), Clone::clone),
            None,
            None,
        );
        if let Some(selected) = editor.selected_blast {
            let blast = editor
                .blasting_outlines
                .iter()
                .find(|outline| crate::ui::state::BlastShapeRef::new(outline.solid, outline.bench_base, outline.anchor) == selected);
            table.readonly(&tr!("solids-blast"), &blast.map_or_else(|| String::from("—"), |blast| blast.name.clone()), None, None);
            table.readonly(
                &tr!("solids-plan-area"),
                &blast.map_or_else(|| String::from("—"), |blast| format!("{:.1}", blast.area)),
                Some("m²"),
                None,
            );
        }
        if let Some(block) = &editor.selected_dig_block_info {
            // The block names its own parents. A scheduler reads the same
            // record, so the panel and the export agree by construction.
            table.readonly(&tr!("solids-dig-block"), &block.name, None, None);
            // Lineage is shown, not inferred: a block that came out of a split
            // or a merge names what it replaced, so a schedule can be traced
            // across a redraw instead of reading as an unrelated new block.
            table.readonly(
                &tr!("solids-block-id"),
                &block.id.0.to_string(),
                None,
                (!block.replaces.is_empty())
                    .then(|| {
                        tr!(
                            "planning-block-replaces",
                            ids = block.replaces.iter().map(|id| id.0.to_string()).collect::<Vec<_>>().join(", ")
                        )
                    })
                    .as_deref(),
            );
            table.readonly(&tr!("solids-plan-area"), &format!("{:.1}", block.plan_area), Some("m²"), None);
            table.readonly(&tr!("solids-bench"), &format!("{:.2}", block.bench.base), Some("RL"), None);
            table.readonly(&tr!("solids-flitch"), &format!("{:.2}", block.flitch.base), Some("RL"), None);
            table.readonly(
                &tr!("solids-block-volume"),
                &block.volume.map_or_else(|| String::from("—"), reserve_number),
                Some("m³"),
                None,
            );
        }
        // A flitch names the bench it sits in, so both rows read for either.
        let benches: Vec<_> = rows.iter().map(|(solid, band)| band.map(|band| bench_base_for(solid, band)).unwrap_or(f64::NAN)).collect();
        table.readonly(&tr!("solids-bench-rl"), &read_rl(benches), None, None);
        let flitches: Vec<_> = rows.iter().map(|(_, band)| band.filter(|band| band.is_flitch).map_or(f64::NAN, |band| band.base)).collect();
        table.readonly(&tr!("solids-flitch-rl"), &read_rl(flitches), None, None);
        table.readonly(
            &tr!("ws-menubar-block-model"),
            &read_across(rows.iter().map(|(solid, _)| solid.block_model).collect(), |model| block_model_label(project, *model)),
            None,
            None,
        );
    });
}

/// The figures for whatever is selected: volume, block-model coverage, and one
/// row per reserve field.
pub(crate) fn draw_contents(ui: &mut egui::Ui, rect: egui::Rect, editor: &EditorState, document: &Document) {
    PropertyTable::new("solids_view_figures", rect, &tr!("solids-contents")).show(ui, |table| {
        table.header(&tr!("destination-condition-field"), &tr!("planning-value"));
        // Why there is nothing to measure yet, while there is not.
        use crate::ui::state::SolidPreviewSummary;
        let geometry = match &editor.solid_preview_summary {
            SolidPreviewSummary::Ready { volume: Some(_), .. } => None,
            SolidPreviewSummary::Ready { volume: None, .. } => Some(tr!("planning-volume-unavailable")),
            SolidPreviewSummary::NotRun => Some(tr!("planning-not-run")),
            SolidPreviewSummary::Empty | SolidPreviewSummary::Unloaded => Some(tr!("planning-set-surface-inspect-solid")),
            SolidPreviewSummary::Building { showing_previous: true } | SolidPreviewSummary::LoadingInputs { showing_previous: true } => Some(tr!("planning-rebuilding-solid")),
            SolidPreviewSummary::Building { .. } | SolidPreviewSummary::LoadingInputs { .. } => Some(tr!("planning-building-solid")),
            SolidPreviewSummary::Failed(message) => Some(message.clone()),
        };
        if let Some(geometry) = geometry {
            table.readonly(&tr!("solids-view-geometry"), &geometry, None, Some(&geometry));
        }
        let volume = match &editor.solid_preview_summary {
            crate::ui::state::SolidPreviewSummary::Ready { volume: Some(volume), .. } => reserve_number(*volume),
            _ => String::from("—"),
        };
        table.readonly(&tr!("tri-volume"), &volume, Some("m³"), None);
        // Geometric volume and block-model coverage are separate figures: a
        // solid the model only partly reaches must not read as fully measured.
        if let Some(coverage) = editor.solid_view_coverage {
            let label = tr!("planning-reserve-coverage");
            let value = format!("{:.1}", coverage * 100.0);
            // Below full coverage the figures beneath are for part of the
            // ground only, which is worth a mark; at full, the row is plain.
            if coverage < 0.9995 {
                table.readonly_warning(&label, &value, Some("%"), Some(&tr!("planning-reserve-coverage-partial")));
            } else {
                table.readonly(&label, &value, Some("%"), None).on_hover_text(tr!("planning-reserve-coverage-note"));
            }
        }
        if let Some(status) = &editor.solid_view_reserve_status {
            table.readonly(&tr!("planning-reserve-status"), status, None, None);
        }
        let reserves = editor.solid_view_reserves.as_ref();
        if let Some(reserves) = reserves {
            table
                .readonly(&tr!("planning-reserve-blocks"), &reserve_number(reserves.all.equivalent_blocks), None, None)
                .on_hover_text(tr!("planning-reserve-method"));
            table.readonly(&tr!("planning-reserve-scope"), &tr!("planning-reserve-method"), None, None);
        }
        for field in document.reserve_fields() {
            if field.aggregation == crate::model::ReserveAggregation::Category {
                let groups = reserves.and_then(|reserves| reserves.categories.get(&field.id));
                table.readonly(
                    &field.name,
                    &groups.map_or_else(|| String::from("—"), |groups| tr!("planning-reserve-categories", count = groups.len())),
                    None,
                    groups.is_none().then(|| editor.solid_view_reserve_issues.get(&field.id)).flatten().map(String::as_str),
                );
                if let Some(groups) = groups {
                    for (category, group) in groups {
                        let label = format!("{} · {}", field.name, category);
                        // Named in the label rather than as a unit: a unit
                        // that long leaves the figure no room.
                        table.readonly(
                            &format!("{label} · {}", tr!("planning-reserve-blocks")),
                            &reserve_number(group.equivalent_blocks),
                            None,
                            None,
                        );
                        for value_field in document
                            .reserve_fields()
                            .iter()
                            .filter(|field| field.aggregation != crate::model::ReserveAggregation::Category)
                        {
                            let value = group.numeric.get(&value_field.id).and_then(|total| total.value(value_field.aggregation));
                            table.readonly(
                                &format!("{label} · {}", value_field.name),
                                &value.map_or_else(|| String::from("—"), reserve_number),
                                None,
                                None,
                            );
                        }
                    }
                }
            } else {
                let total = reserves.and_then(|reserves| reserves.all.numeric.get(&field.id));
                let value = total.and_then(|total| total.value(field.aggregation));
                // Every dash says what is wrong with it, and a figure that
                // only some of the ground contributed to says so too. An
                // absent number is not a zero, and a partial one is not a
                // total; a reader should not have to guess which they have.
                let shown = value.map_or_else(|| String::from("—"), reserve_number);
                match (value, total) {
                    (None, _) => {
                        let reason = editor
                            .solid_view_reserve_issues
                            .get(&field.id)
                            .cloned()
                            .or_else(|| total.map(|total| tr!("planning-reserve-none-contributed", missing = total.missing.to_string())));
                        table.readonly(&field.name, &shown, None, reason.as_deref());
                    }
                    // A figure some blocks could not add to stands, marked.
                    (Some(_), Some(total)) if total.is_partial() => {
                        let partial = tr!(
                            "planning-reserve-partial",
                            contributed = total.contributions.to_string(),
                            missing = (total.missing + total.unusable_weights).to_string()
                        );
                        table.readonly_warning(&field.name, &shown, None, Some(&partial));
                    }
                    _ => {
                        table.readonly(&field.name, &shown, None, None);
                    }
                }
            }
        }
    });
}

/// A reserve figure as the table shows it: whole numbers with separators
/// from a thousand up, where decimals are noise beside the digits before
/// them; three decimals, trimmed, below that, where a grade needs them.
fn reserve_number(value: f64) -> String {
    use thousands::Separable;

    if value.abs() >= 1000.0 {
        let rounded = value.round();
        return format!("{:.0}", if rounded == 0.0 { 0.0 } else { rounded }).separate_with_commas();
    }
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { String::from("0") } else { text.to_owned() }
}

/// The RL column for a set of slices: blank where a row has none, and
/// "Multiple" where they disagree.
fn read_rl(values: Vec<f64>) -> String {
    if values.iter().all(|value| value.is_nan()) {
        return String::from("—");
    }
    let first = values[0];
    if values.iter().all(|value| (*value - first).abs() < 1e-6 || (value.is_nan() && first.is_nan())) {
        return format_rl(first);
    }
    tr!("solids-multiple")
}

/// Which bench a slice belongs to: a bench is its own, and a flitch names the
/// bench it divides.
fn bench_base_for(solid: &crate::model::Solid, band: BenchSelection) -> f64 {
    if !band.is_flitch {
        return band.base;
    }
    solid
        .benching
        .benches()
        .into_iter()
        .find(|bench| band.base >= bench.base - 1e-6 && band.top <= bench.top() + 1e-6)
        .map_or(f64::NAN, |bench| bench.base)
}

/// The Reserves view's column down the right edge: what the selection is
/// and what it holds, one table under the other in one island that scrolls
/// as a whole. The tree is in the explorer column, and the dig blocks are in
/// the viewport between them.
pub(crate) fn draw_panel(ui: &mut egui::Ui, editor: &mut EditorState, project: &UiProjectView, document: &Document) -> super::haulage::LayoutColumn {
    use crate::ui::widgets::{
        data_grid::{stack_slot, table_stack},
        island::{Island, Side},
    };
    let column = Island::new("solids_view_column", Side::Right)
        .default_width(360.0)
        .min_width(200.0)
        .flush()
        .show(ui, |ui, rect| {
            table_stack(ui, rect, "solids_view_column", |ui| {
                draw_properties(ui, stack_slot(ui), editor, project, document);
                draw_contents(ui, stack_slot(ui), editor, document);
            });
        });
    super::haulage::LayoutColumn {
        regions: column.regions,
        grips: vec![column.grip],
    }
}
