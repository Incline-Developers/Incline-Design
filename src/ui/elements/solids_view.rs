//! The Solids page's View subpage: everything the setup has generated.
//!
//! Where the Setup pages configure a solid, this one is for looking through
//! what came out - the solids grouped by what they are, each opening into its
//! benches and those into their flitches - and reading off what any selection
//! of them holds.

use crate::{
    i18n::tr,
    model::{Document, SolidKind},
    ui::{
        EditorState, UiProjectView,
        dialogs::solids::{block_model_label, kind_label},
        fonts::bold,
        state::{BenchSelection, BlastShapeRef, SolidsViewRow, UiCommand},
        unthemed_icon,
        widgets::{
            context_menu::{ContextMenuAction, context_menu_popup},
            data_grid::PropertyTable,
            explorer::{ExplorerEntry, ExplorerHeader, explorer_note, paint_fixed_stripes, reserve_fixed_stripes},
        },
    },
};

/// Draw the tree: kind, solid, bench, blast, then flitch.
///
/// Rows are selected the way a file list is - click to replace the selection,
/// ctrl-click to add to it - because the properties beside the tree are about
/// however many rows are picked, not just one. Clicking a parent picks
/// everything under it, so the arrows are what collapse a row rather than its
/// label; flitches start collapsed, since a pit has tens of them and the
/// benches are what the tree is read for.
pub(crate) fn draw_tree(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, commands: &mut Vec<UiCommand>) {
    draw_tree_to_depth(ui, editor, document, true, false, commands);
}

/// The same tree for the sequence editors, with an eye on every row that
/// hides its ground from their preview - see [`EditorState::sequence_hidden`].
pub(crate) fn draw_sequence_tree(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, commands: &mut Vec<UiCommand>) {
    draw_tree_to_depth(ui, editor, document, true, true, commands);
}

/// Which page a Solids Navigation tree drives. Each keeps its own hidden
/// set and selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NavigationTree {
    Animation,
    Haulage,
}

fn hidden(editor: &EditorState, tree: NavigationTree) -> &crate::ui::state::SolidsVisibility {
    match tree {
        NavigationTree::Animation => &editor.schedule_animation_hidden,
        NavigationTree::Haulage => &editor.haul_hidden,
    }
}

fn hidden_mut(editor: &mut EditorState, tree: NavigationTree) -> &mut crate::ui::state::SolidsVisibility {
    match tree {
        NavigationTree::Animation => &mut editor.schedule_animation_hidden,
        NavigationTree::Haulage => &mut editor.haul_hidden,
    }
}

fn navigation_selection(editor: &EditorState, tree: NavigationTree) -> &[SolidsViewRow] {
    match tree {
        NavigationTree::Animation => &editor.schedule_animation_selection,
        NavigationTree::Haulage => &editor.haul_navigation_selection,
    }
}

/// Animate's independent hierarchy. Every row has the same eye affordance as
/// the Objects navigator, while clicking a solid also frames it in the main
/// viewport.
pub(crate) fn draw_animation_tree(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, tree: NavigationTree, commands: &mut Vec<UiCommand>) {
    ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
    egui::ScrollArea::vertical().auto_shrink([false; 2]).min_scrolled_height(0.0).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        let (slot, top) = reserve_fixed_stripes(ui);
        if document.solids().is_empty() {
            explorer_note(ui, tr!("solids-no-solids-yet-add"));
        }
        for kind in SolidKind::ALL {
            let solids: Vec<_> = document.solids().iter().filter(|solid| solid.kind == kind).collect();
            if solids.is_empty() {
                continue;
            }
            let kind_visible = solids.iter().any(|solid| !hidden(editor, tree).solids.contains(&solid.id));
            let (_, visibility_clicked) = animation_parent_row(ui, egui::Id::new(("animation_kind", kind as u8)), kind_label(kind), false, kind_visible, |ui| {
                for solid in &solids {
                    draw_animation_solid(ui, editor, document, solid, tree, commands);
                }
            });
            if visibility_clicked {
                for solid in &solids {
                    set_animation_solid_visible(editor, tree, solid.id, !kind_visible);
                }
            }
        }
        paint_fixed_stripes(ui, slot, top, crate::ui::widgets::tree_row_colors(ui).1);
    });
}

/// How the Layout's dug blocks under a Haulage navigation row meet the roads:
/// green when every one reaches a road, yellow when any is out of reach,
/// and plain where nothing is dug.
fn reach_text(editor: &EditorState, tree: NavigationTree, label: String, under: impl Fn(&crate::ui::state::HaulBlock) -> bool) -> egui::WidgetText {
    if tree != NavigationTree::Haulage {
        return label.into();
    }
    let mut dug = editor.haul_blocks.iter().filter(|b| b.dug && under(b)).peekable();
    if dug.peek().is_none() {
        return label.into();
    }
    let color = if dug.all(|b| b.connected) { REACHED_COLOR } else { UNREACHED_COLOR };
    egui::RichText::new(label).color(color).into()
}

/// The green a navigation row takes when all its dug blocks reach a road.
const REACHED_COLOR: egui::Color32 = egui::Color32::from_rgb(72, 170, 110);
/// The yellow it takes when any is out of reach: the warning badge's.
const UNREACHED_COLOR: egui::Color32 = egui::Color32::from_rgb(0xE5, 0xB6, 0x2F);

fn draw_animation_solid(ui: &mut egui::Ui, editor: &mut EditorState, _document: &Document, solid: &crate::model::Solid, tree: NavigationTree, commands: &mut Vec<UiCommand>) {
    // Haulage lists the flitches its dig blocks are on: the occupied bands
    // are only gathered while a page that draws the solids is open.
    let occupied = match tree {
        NavigationTree::Haulage if editor.haul_blocks.iter().any(|b| b.solid == solid.id) => {
            let mut flitches: Vec<BenchSelection> = Vec::new();
            for block in editor.haul_blocks.iter().filter(|b| b.solid == solid.id) {
                if !flitches.contains(&block.flitch) {
                    flitches.push(block.flitch);
                }
            }
            Some(flitches)
        }
        _ => editor.solid_view_bands.get(&solid.id).cloned(),
    };
    let solid_rows = descendants(solid, occupied.as_ref(), true);
    let selected = group_selected(navigation_selection(editor, tree), &solid_rows);
    // Ground no longer mined is not dug, so it is not listed.
    let mined = editor.mined_ground.clone();
    let visible = !hidden(editor, tree).solids.contains(&solid.id);
    let benches: Vec<_> = solid
        .benching
        .benches()
        .into_iter()
        .rev()
        .filter(|bench| holds(occupied.as_ref(), bench.base, bench.top()))
        .filter(|bench| mined.as_ref().is_none_or(|mined| mined.bench(solid.id, bench.base)))
        .collect();
    let bench_rows: Vec<_> = benches
        .iter()
        .map(|bench| SolidsViewRow {
            solid: solid.id,
            band: Some(BenchSelection {
                base: bench.base,
                top: bench.top(),
                is_flitch: false,
            }),
        })
        .collect();
    let title = reach_text(editor, tree, solid.name.clone(), |b| b.solid == solid.id);
    let (clicked, visibility_clicked) = animation_parent_row(ui, egui::Id::new(("animation_solid", solid.id.0)), title, selected, visible, |ui| {
        if occupied.is_none() {
            explorer_note(ui, tr!("planning-solid-geometry-pending"));
        }
        for (bench, &bench_row) in benches.iter().zip(&bench_rows) {
            let flitch_rows: Vec<_> = bench
                .flitches
                .iter()
                .rev()
                .filter(|flitch| holds(occupied.as_ref(), flitch.base, flitch.top()))
                .map(|flitch| SolidsViewRow {
                    solid: solid.id,
                    band: Some(BenchSelection {
                        base: flitch.base,
                        top: flitch.top(),
                        is_flitch: true,
                    }),
                })
                .collect();
            let blasts = solid.blasting.bench(bench.base).map(|entry| entry.blasts.as_slice()).unwrap_or_default();
            let blast_refs: Vec<_> = blasts.iter().map(|blast| BlastShapeRef::new(solid.id, bench.base, blast.anchor)).collect();
            let bench_selected = group_selected(navigation_selection(editor, tree), std::slice::from_ref(&bench_row));
            let bench_visible = visible && !hidden(editor, tree).rows.contains(&bench_row);
            let bench_title = reach_text(editor, tree, format_rl(bench.base), |b| b.solid == solid.id && b.bench.base == bench.base);
            let (bench_clicked, bench_visibility_clicked) = animation_parent_row(
                ui,
                egui::Id::new(("animation_bench", solid.id.0, bench.base.to_bits())),
                bench_title,
                bench_selected,
                bench_visible,
                |ui| {
                    for (blast_index, (blast, &blast_ref)) in blasts.iter().zip(&blast_refs).enumerate() {
                        if mined.as_ref().is_some_and(|mined| !mined.blast(blast_ref)) {
                            continue;
                        }
                        let blast_visible = bench_visible && !hidden(editor, tree).blasts.contains(&blast_ref);
                        let blast_title = reach_text(editor, tree, blast.name.clone(), |b| b.blast == Some(blast_ref));
                        let (_, blast_visibility_clicked) = animation_parent_row(
                            ui,
                            egui::Id::new(("animation_blast", solid.id.0, bench.base.to_bits(), blast_index)),
                            blast_title,
                            false,
                            blast_visible,
                            |ui| {
                                for &flitch_row in &flitch_rows {
                                    let Some(flitch) = flitch_row.band else { continue };
                                    if mined.as_ref().is_some_and(|mined| !mined.flitch(blast_ref, flitch.base)) {
                                        continue;
                                    }
                                    let flitch_visible = blast_visible && !hidden(editor, tree).hides_flitch(blast_ref, flitch.base);
                                    let flitch_title = reach_text(editor, tree, format_rl(flitch.base), |b| b.blast == Some(blast_ref) && b.flitch.base == flitch.base);
                                    let row = animation_leaf_row(
                                        ui,
                                        egui::Id::new(("animation_flitch", solid.id.0, bench.base.to_bits(), blast_index, flitch.base.to_bits())),
                                        flitch_title,
                                        group_selected(navigation_selection(editor, tree), std::slice::from_ref(&flitch_row)),
                                        flitch_visible,
                                    );
                                    if row.0 {
                                        select_animation_rows(ui, editor, tree, vec![flitch_row]);
                                    }
                                    if row.1 {
                                        let family = Family {
                                            benches: &bench_rows,
                                            bench: bench_row,
                                            flitches: &flitch_rows,
                                            blasts: &blast_refs,
                                        };
                                        set_animation_flitch_visible(hidden_mut(editor, tree), &family, blast_ref, flitch.base, !flitch_visible);
                                    }
                                }
                            },
                        );
                        if blast_visibility_clicked {
                            let family = Family {
                                benches: &bench_rows,
                                bench: bench_row,
                                flitches: &flitch_rows,
                                blasts: &blast_refs,
                            };
                            set_animation_blast_visible(hidden_mut(editor, tree), &family, blast_ref, !blast_visible);
                        }
                    }
                },
            );
            if bench_clicked {
                select_animation_rows(ui, editor, tree, vec![bench_row]);
            }
            if bench_visibility_clicked {
                let family = Family {
                    benches: &bench_rows,
                    bench: bench_row,
                    flitches: &flitch_rows,
                    blasts: &blast_refs,
                };
                set_animation_bench_visible(hidden_mut(editor, tree), &family, !bench_visible);
            }
        }
    });
    if clicked {
        select_animation_rows(ui, editor, tree, solid_rows);
        match tree {
            NavigationTree::Animation => commands.push(UiCommand::FocusScheduleAnimationSolid(solid.id)),
            NavigationTree::Haulage => {
                let points: Vec<_> = editor
                    .haul_blocks
                    .iter()
                    .filter(|b| b.solid == solid.id)
                    .flat_map(|b| b.rings.iter().flatten().copied())
                    .collect();
                if let Some(min) = points.iter().copied().reduce(glam::DVec3::min)
                    && let Some(max) = points.iter().copied().reduce(glam::DVec3::max)
                {
                    commands.push(UiCommand::FrameHaul(min, max));
                }
            }
        }
    }
    if visibility_clicked {
        set_animation_solid_visible(editor, tree, solid.id, !visible);
    }
}

fn animation_parent_row(ui: &mut egui::Ui, id: egui::Id, label: impl Into<egui::WidgetText>, selected: bool, visible: bool, body: impl FnOnce(&mut egui::Ui)) -> (bool, bool) {
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
    let row = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);
            ExplorerEntry::new(id.with("row"), label).selected(selected).visibility_toggle(visible).show(ui)
        })
        .inner;
    state.show_body_indented(&row.response, ui, body);
    (row.response.clicked(), row.visibility_clicked)
}

fn animation_leaf_row(ui: &mut egui::Ui, id: egui::Id, label: impl Into<egui::WidgetText>, selected: bool, visible: bool) -> (bool, bool) {
    let row = ExplorerEntry::new(id, label)
        .reserve_toggle_gutter(true)
        .selected(selected)
        .visibility_toggle(visible)
        .show(ui);
    (row.response.clicked(), row.visibility_clicked)
}

fn select_animation_rows(ui: &egui::Ui, editor: &mut EditorState, tree: NavigationTree, rows: Vec<SolidsViewRow>) {
    let extend = ui.input(|input| input.modifiers.command || input.modifiers.shift);
    let selection = match tree {
        NavigationTree::Animation => &mut editor.schedule_animation_selection,
        NavigationTree::Haulage => &mut editor.haul_navigation_selection,
    };
    apply_selection(selection, &rows, extend);
    ui.ctx().request_repaint();
}

fn set_animation_solid_visible(editor: &mut EditorState, tree: NavigationTree, solid: crate::model::SolidId, visible: bool) {
    set_solid_visible(hidden_mut(editor, tree), solid, visible);
}

/// Show or hide a whole solid. Either way what was hidden under it goes:
/// a parent's eye sets everything below it, so showing it again brings
/// back every bench, blast and flitch rather than only those not hidden
/// on their own before.
fn set_solid_visible(hidden: &mut crate::ui::state::SolidsVisibility, solid: crate::model::SolidId, visible: bool) {
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
    fn open_solid(&self, hidden: &mut crate::ui::state::SolidsVisibility) {
        if hidden.solids.remove(&self.bench.solid) {
            for bench in self.benches.iter().filter(|row| **row != self.bench) {
                if !hidden.rows.contains(bench) {
                    hidden.rows.push(*bench);
                }
            }
        }
    }

    /// Open the solid and the bench above `blast`, a bench hidden whole
    /// becoming its other blasts, hidden one by one.
    fn open_bench(&self, hidden: &mut crate::ui::state::SolidsVisibility, blast: BlastShapeRef) {
        self.open_solid(hidden);
        if hidden.rows.contains(&self.bench) {
            hidden.rows.retain(|hidden| *hidden != self.bench);
            hidden.blasts.extend(self.blasts.iter().filter(|other| **other != blast));
        }
    }
}

/// A bench's eye sets its blasts and flitches with it, as a solid's does.
fn set_animation_bench_visible(hidden: &mut crate::ui::state::SolidsVisibility, family: &Family<'_>, visible: bool) {
    hidden.rows.retain(|hidden| *hidden != family.bench && !family.flitches.contains(hidden));
    hidden.blasts.retain(|blast| !family.blasts.contains(blast));
    hidden.flitches.retain(|(blast, _)| !family.blasts.contains(blast));
    if visible {
        family.open_solid(hidden);
    } else {
        hidden.rows.push(family.bench);
    }
}

fn set_animation_blast_visible(hidden: &mut crate::ui::state::SolidsVisibility, family: &Family<'_>, blast: BlastShapeRef, visible: bool) {
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
fn set_animation_flitch_visible(hidden: &mut crate::ui::state::SolidsVisibility, family: &Family<'_>, blast: BlastShapeRef, flitch_base: f64, visible: bool) {
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

/// The same tree stopping at benches, for the Blasting step: a blast divides
/// a bench, and flitches have nothing to say about it.
pub(crate) fn draw_bench_tree(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, commands: &mut Vec<UiCommand>) {
    draw_tree_to_depth(ui, editor, document, false, false, commands);
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
                    let occupied = editor.solid_view_bands.get(&solid.id);
                    occupied.is_none() || solid.benching.benches().iter().any(|bench| holds(occupied, bench.base, bench.top()))
                })
                .collect();
            if solids.is_empty() {
                continue;
            }
            let kind_id = egui::Id::new(("strip_kind", kind as u8));
            if reveal && solids.iter().any(|solid| editor.solids_view_selection.iter().any(|row| row.solid == solid.id)) {
                open_row(ui, kind_id);
            }
            ExplorerHeader::new(kind_id, kind_label(kind)).icon(unthemed_icon!("layer.svg")).show(ui, |ui| {
                for solid in solids {
                    let occupied = editor.solid_view_bands.get(&solid.id);
                    let solid_id = egui::Id::new(("strip_solid", solid.id.0));
                    let picked = editor.solids_view_selection.iter().find(|row| row.solid == solid.id).and_then(|row| row.band);
                    if reveal && picked.is_some() {
                        open_row(ui, solid_id);
                    }
                    ExplorerHeader::new(solid_id, solid.name.clone()).icon(unthemed_icon!("triangulation.svg")).show(ui, |ui| {
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
                            let bench_id = egui::Id::new(("strip_bench", solid.id.0, bench.base.to_bits()));
                            if reveal && picked.is_some_and(|band| bench.contains_rl(band.base)) {
                                open_row(ui, bench_id);
                            }
                            // Only a flitch is drawn on, so the bench row just
                            // opens and closes.
                            let response = collapsible_row(ui, bench_id, &format_rl(bench.base), false, |ui| {
                                for flitch in flitches.iter() {
                                    let row = SolidsViewRow {
                                        solid: solid.id,
                                        band: Some(BenchSelection {
                                            base: flitch.base,
                                            top: flitch.top(),
                                            is_flitch: true,
                                        }),
                                    };
                                    if leaf_row(
                                        ui,
                                        egui::Id::new(("strip_flitch", solid.id.0, flitch.base.to_bits())),
                                        &format_rl(flitch.base),
                                        editor.solids_view_selection == [row],
                                    ) {
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

fn draw_tree_to_depth(ui: &mut egui::Ui, editor: &mut EditorState, document: &Document, show_flitches: bool, toggles: bool, commands: &mut Vec<UiCommand>) {
    ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
    let reveal = std::mem::take(&mut editor.solids_tree_reveal);
    egui::ScrollArea::vertical().auto_shrink([false; 2]).min_scrolled_height(0.0).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        let (slot, top) = reserve_fixed_stripes(ui);

        if document.solids().is_empty() {
            explorer_note(ui, tr!("solids-no-solids-yet-add"));
        }
        // Where it picks ground to dig, the tree leaves out what is no
        // longer mined; the Solids pages list it, to put it back.
        let mined = editor.mined_ground.clone();
        let selection = &editor.solids_view_selection;
        // Edited on a copy, because the rows read the editor as they draw,
        // and written back once the tree is done.
        let mut hidden = toggles.then(|| editor.sequence_hidden.clone());
        let mut clicked: Option<Vec<SolidsViewRow>> = None;
        let mut clicked_blast = None;
        for kind in SolidKind::ALL {
            let solids: Vec<_> = document.solids().iter().filter(|solid| solid.kind == kind).collect();
            if solids.is_empty() {
                continue;
            }
            let kind_rows: Vec<_> = solids
                .iter()
                .flat_map(|solid| descendants(solid, editor.solid_view_bands.get(&solid.id), show_flitches))
                .collect();
            let kind_id = egui::Id::new(("solids_view_kind", kind as u8));
            if reveal && solids.iter().any(|solid| selection.iter().any(|row| row.solid == solid.id)) {
                open_row(ui, kind_id);
            }
            let kind_visible = hidden.as_ref().map(|hidden| solids.iter().any(|solid| !hidden.solids.contains(&solid.id)));
            let kind_solids: Vec<_> = solids.iter().map(|solid| solid.id).collect();
            let mut kind_header = ExplorerHeader::new(kind_id, kind_label(kind))
                .icon(unthemed_icon!("layer.svg"))
                .collapse_on_click(false)
                .selected(group_selected(selection, &kind_rows));
            if let Some(visible) = kind_visible {
                kind_header = kind_header.visibility_toggle(visible);
            }
            let (_, kind_heading, _, kind_visibility_clicked) = kind_header.show_with_visibility(ui, |ui| {
                for solid in solids {
                    let occupied = editor.solid_view_bands.get(&solid.id);
                    let solid_rows = descendants(solid, occupied, show_flitches);
                    // `.1` is the heading itself; its `.inner` is the clickable
                    // label, where `.response` is only the row's hover area.
                    let solid_id = egui::Id::new(("solids_view_solid", solid.id.0));
                    if reveal && selection.iter().any(|row| row.solid == solid.id) {
                        open_row(ui, solid_id);
                    }
                    let solid_visible = hidden.as_ref().map(|hidden| !hidden.solids.contains(&solid.id));
                    let benches = solid.benching.benches();
                    let listed: Vec<_> = benches
                        .iter()
                        .rev()
                        .filter(|bench| holds(occupied, bench.base, bench.top()))
                        .filter(|bench| mined.as_ref().is_none_or(|mined| mined.bench(solid.id, bench.base)))
                        .collect();
                    // Every bench row the tree lists, which showing one
                    // bench of a hidden solid hides one by one.
                    let listed_rows: Vec<_> = listed
                        .iter()
                        .map(|bench| SolidsViewRow {
                            solid: solid.id,
                            band: Some(BenchSelection {
                                base: bench.base,
                                top: bench.top(),
                                is_flitch: false,
                            }),
                        })
                        .collect();
                    let mut solid_header = ExplorerHeader::new(solid_id, solid.name.clone())
                        .icon(unthemed_icon!("triangulation.svg"))
                        .collapse_on_click(false)
                        .selected(group_selected(selection, &solid_rows));
                    if let Some(visible) = solid_visible {
                        solid_header = solid_header.visibility_toggle(visible);
                    }
                    let (_, heading, _, solid_visibility_clicked) = solid_header.show_with_visibility(ui, |ui| {
                        if occupied.is_none() {
                            explorer_note(ui, tr!("planning-solid-geometry-pending"));
                        }
                        for (bench, &bench_row) in listed.iter().zip(&listed_rows) {
                            let bench_band = BenchSelection {
                                base: bench.base,
                                top: bench.top(),
                                is_flitch: false,
                            };
                            let flitches: Vec<_> = if show_flitches {
                                bench.flitches.iter().rev().filter(|flitch| holds(occupied, flitch.base, flitch.top())).collect()
                            } else {
                                Vec::new()
                            };
                            let bench_rows: Vec<_> = std::iter::once(bench_band)
                                .chain(flitches.iter().map(|flitch| BenchSelection {
                                    base: flitch.base,
                                    top: flitch.top(),
                                    is_flitch: true,
                                }))
                                .map(|band| SolidsViewRow {
                                    solid: solid.id,
                                    band: Some(band),
                                })
                                .collect();
                            let bench_id = egui::Id::new(("solids_view_bench", solid.id.0, bench.base.to_bits()));
                            let bench_target = crate::model::ExclusionTarget::Bench(bench.base);
                            let bench_label = exclusion_label(&format_rl(bench.base), solid.exclusions.is_target_excluded(bench_target));
                            let bench_selected = group_selected(selection, &bench_rows);
                            let bench_visible = hidden.as_ref().zip(solid_visible).map(|(hidden, visible)| visible && !hidden.rows.contains(&bench_row));
                            let flitch_rows: Vec<_> = bench_rows.iter().filter(|row| row.band.is_some_and(|band| band.is_flitch)).copied().collect();
                            let blast_refs: Vec<_> = solid
                                .blasting
                                .bench(bench.base)
                                .map(|entry| entry.blasts.iter().map(|blast| BlastShapeRef::new(solid.id, bench.base, blast.anchor)).collect())
                                .unwrap_or_default();
                            if !show_flitches {
                                let response = leaf_row_response(ui, bench_id, &bench_label, bench_selected);
                                exclusion_menu(&response, &bench_label, solid, bench_target, commands);
                                if response.clicked() {
                                    clicked = Some(bench_rows);
                                }
                                continue;
                            }
                            // Flitches start closed: a pit carries tens of them,
                            // and the benches are what the tree is scanned for.
                            let (bench_response, bench_visibility_clicked) = collapsible_toggle_row(ui, bench_id, &bench_label, bench_selected, bench_visible, |ui| {
                                let blasts = solid.blasting.bench(bench.base).map(|entry| entry.blasts.as_slice()).unwrap_or_default();
                                if blasts.is_empty() {
                                    explorer_note(ui, tr!("planning-solid-geometry-pending"));
                                    return;
                                }
                                for (blast_index, blast) in blasts.iter().enumerate() {
                                    let shape = crate::ui::state::BlastShapeRef::new(solid.id, bench.base, blast.anchor);
                                    if mined.as_ref().is_some_and(|mined| !mined.blast(shape)) {
                                        continue;
                                    }
                                    let blast_ref = Some(shape);
                                    let blast_selected = editor.selected_blast == blast_ref || (editor.selected_blast.is_none() && bench_selected);
                                    let blast_target = crate::model::ExclusionTarget::Blast {
                                        bench: bench.base,
                                        anchor: blast.anchor,
                                    };
                                    let blast_label = exclusion_label(&blast.name, solid.exclusions.is_target_excluded(blast_target));
                                    let blast_visible = hidden.as_ref().zip(bench_visible).map(|(hidden, visible)| visible && !hidden.blasts.contains(&shape));
                                    let (blast_response, blast_visibility_clicked) =
                                        collapsible_toggle_row(ui, bench_id.with(("blast", blast_index)), &blast_label, blast_selected, blast_visible, |ui| {
                                            for flitch in flitches.iter().filter(|flitch| mined.as_ref().is_none_or(|mined| mined.flitch(shape, flitch.base))) {
                                                let flitch_row = SolidsViewRow {
                                                    solid: solid.id,
                                                    band: Some(BenchSelection {
                                                        base: flitch.base,
                                                        top: flitch.top(),
                                                        is_flitch: true,
                                                    }),
                                                };
                                                let flitch_visible = hidden
                                                    .as_ref()
                                                    .zip(blast_visible)
                                                    .map(|(hidden, visible)| visible && !hidden.hides_flitch(shape, flitch.base));
                                                let (response, visibility_clicked) = leaf_toggle_row(
                                                    ui,
                                                    egui::Id::new(("solids_view_flitch", solid.id.0, bench.base.to_bits(), blast_index, flitch.base.to_bits())),
                                                    &format_rl(flitch.base),
                                                    group_selected(selection, std::slice::from_ref(&flitch_row))
                                                        && (editor.selected_blast.is_none() || editor.selected_blast == blast_ref),
                                                    flitch_visible,
                                                );
                                                if response.clicked() {
                                                    clicked = Some(vec![flitch_row]);
                                                    clicked_blast = blast_ref;
                                                }
                                                if visibility_clicked && let Some(hidden) = hidden.as_mut() {
                                                    let family = Family {
                                                        benches: &listed_rows,
                                                        bench: bench_row,
                                                        flitches: &flitch_rows,
                                                        blasts: &blast_refs,
                                                    };
                                                    set_animation_flitch_visible(hidden, &family, shape, flitch.base, !flitch_visible.unwrap_or(true));
                                                }
                                            }
                                        });
                                    if blast_visibility_clicked && let Some(hidden) = hidden.as_mut() {
                                        let family = Family {
                                            benches: &listed_rows,
                                            bench: bench_row,
                                            flitches: &flitch_rows,
                                            blasts: &blast_refs,
                                        };
                                        set_animation_blast_visible(hidden, &family, shape, !blast_visible.unwrap_or(true));
                                    }
                                    exclusion_menu(&blast_response, &blast_label, solid, blast_target, commands);
                                    if blast_response.clicked() {
                                        clicked = Some(bench_rows.clone());
                                        clicked_blast = blast_ref;
                                    }
                                }
                            });
                            exclusion_menu(&bench_response, &bench_label, solid, bench_target, commands);
                            if bench_visibility_clicked && let Some(hidden) = hidden.as_mut() {
                                let family = Family {
                                    benches: &listed_rows,
                                    bench: bench_row,
                                    flitches: &flitch_rows,
                                    blasts: &blast_refs,
                                };
                                set_animation_bench_visible(hidden, &family, !bench_visible.unwrap_or(true));
                            }
                            if bench_response.clicked() {
                                clicked = Some(bench_rows);
                            }
                        }
                    });
                    if heading.inner.clicked() {
                        clicked = Some(solid_rows);
                    }
                    if solid_visibility_clicked && let Some(hidden) = hidden.as_mut() {
                        set_solid_visible(hidden, solid.id, !solid_visible.unwrap_or(true));
                    }
                }
            });
            if kind_heading.inner.clicked() {
                clicked = Some(kind_rows);
            }
            if kind_visibility_clicked && let Some(hidden) = hidden.as_mut() {
                for &solid in &kind_solids {
                    set_solid_visible(hidden, solid, !kind_visible.unwrap_or(true));
                }
            }
        }
        paint_fixed_stripes(ui, slot, top, crate::ui::widgets::tree_row_colors(ui).1);
        if let Some(hidden) = hidden
            && hidden != editor.sequence_hidden
        {
            editor.sequence_hidden = hidden;
            ui.ctx().request_repaint();
        }

        if let Some(rows) = clicked {
            let extend = ui.input(|input| input.modifiers.command || input.modifiers.shift);
            if clicked_blast.is_some() && clicked_blast != editor.selected_blast && !extend {
                editor.solids_view_selection = rows;
            } else {
                apply_selection(&mut editor.solids_view_selection, &rows, extend);
            }
            editor.selected_blast = clicked_blast.filter(|_| !editor.solids_view_selection.is_empty());
            editor.selected_dig_block = None;
            ui.ctx().request_repaint();
        }
    });
}

/// Whether a band of the solid was actually generated, so the tree lists the
/// RLs the solid occupies rather than every RL the plan could name.
pub(crate) fn holds(occupied: Option<&Vec<BenchSelection>>, base: f64, top: f64) -> bool {
    occupied.is_some_and(|bands| bands.iter().any(|band| band.top > base + 1e-6 && band.base < top - 1e-6))
}

/// Every row a click on a solid stands for: the solid itself, and each of the
/// benches and flitches it occupies. Picking a parent picks what is under it.
fn descendants(solid: &crate::model::Solid, occupied: Option<&Vec<BenchSelection>>, include_flitches: bool) -> Vec<SolidsViewRow> {
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

/// A selectable tree row that opens into children.
///
/// The arrow is the only thing that collapses it: clicking the row itself
/// selects it *and* its children, because the figures beside the tree are
/// read across whatever is picked.
fn collapsible_row(ui: &mut egui::Ui, id: egui::Id, label: &str, selected: bool, body: impl FnOnce(&mut egui::Ui)) -> egui::Response {
    collapsible_toggle_row(ui, id, label, selected, None, body).0
}

/// [`collapsible_row`] ending in an eye showing `visible`, when there is
/// one, and whether it was clicked.
fn collapsible_toggle_row(ui: &mut egui::Ui, id: egui::Id, label: &str, selected: bool, visible: Option<bool>, body: impl FnOnce(&mut egui::Ui)) -> (egui::Response, bool) {
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
    let header = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            state.show_toggle_button(ui, egui::collapsing_header::paint_default_icon);
            let entry = ExplorerEntry::new(id.with("row"), label.to_owned()).selected(selected);
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
/// an arrow so the column of labels stays straight.
fn leaf_row(ui: &mut egui::Ui, id: egui::Id, label: &str, selected: bool) -> bool {
    leaf_row_response(ui, id, label, selected).clicked()
}

fn leaf_row_response(ui: &mut egui::Ui, id: egui::Id, label: &str, selected: bool) -> egui::Response {
    leaf_toggle_row(ui, id, label, selected, None).0
}

/// [`leaf_row_response`] ending in an eye showing `visible`, when there is
/// one, and whether it was clicked.
fn leaf_toggle_row(ui: &mut egui::Ui, id: egui::Id, label: &str, selected: bool, visible: Option<bool>) -> (egui::Response, bool) {
    let entry = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let entry = ExplorerEntry::new(id, label.to_owned()).reserve_toggle_gutter(true).selected(selected);
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
                &block.volume.map_or_else(|| String::from("—"), |volume| format!("{volume:.1}")),
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
        let volume = match &editor.solid_preview_summary {
            crate::ui::state::SolidPreviewSummary::Ready { volume: Some(volume), .. } => format!("{volume:.1}"),
            _ => String::from("—"),
        };
        table.readonly(&tr!("tri-volume"), &volume, Some("m³"), None);
        // Geometric volume and block-model coverage are separate figures: a
        // solid the model only partly reaches must not read as fully measured.
        if let Some(coverage) = editor.solid_view_coverage {
            table.readonly(
                &tr!("planning-reserve-coverage"),
                &format!("{:.1}", coverage * 100.0),
                Some("%"),
                Some(&tr!("planning-reserve-coverage-note")),
            );
        }
        if let Some(status) = &editor.solid_view_reserve_status {
            table.readonly(&tr!("planning-reserve-status"), status, None, None);
        }
        let reserves = editor.solid_view_reserves.as_ref();
        if let Some(reserves) = reserves {
            table.readonly(
                &tr!("planning-reserve-blocks"),
                &format!("{:.3}", reserves.all.equivalent_blocks),
                None,
                Some(&tr!("planning-reserve-method")),
            );
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
                        table.readonly(&label, &format!("{:.3}", group.equivalent_blocks), Some(&tr!("planning-reserve-blocks")), None);
                        for value_field in document
                            .reserve_fields()
                            .iter()
                            .filter(|field| field.aggregation != crate::model::ReserveAggregation::Category)
                        {
                            let value = group.numeric.get(&value_field.id).and_then(|total| total.value(value_field.aggregation));
                            table.readonly(
                                &format!("{label} · {}", value_field.name),
                                &value.map_or_else(|| String::from("—"), |value| format!("{value:.3}")),
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
                let note = match (value, total) {
                    (None, _) => editor
                        .solid_view_reserve_issues
                        .get(&field.id)
                        .cloned()
                        .or_else(|| total.map(|total| tr!("planning-reserve-none-contributed", missing = total.missing.to_string()))),
                    (Some(_), Some(total)) if total.is_partial() => Some(tr!(
                        "planning-reserve-partial",
                        contributed = total.contributions.to_string(),
                        missing = (total.missing + total.unusable_weights).to_string()
                    )),
                    _ => None,
                };
                table.readonly(&field.name, &value.map_or_else(|| String::from("—"), |value| format!("{value:.3}")), None, note.as_deref());
            }
        }
    });
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

/// The View subpage's own layout: the tree is drawn into the explorer column,
/// and this fills the rest of the window with the properties beside it.
/// The View page: what the selection is and what it holds, down the right, and
/// the inspector taking the rest.
///
/// The two tables are a column of their own rather than a slice cut out of the
/// pane, so the seam between them and the seam beside them both belong to the
/// user - and the inspector gets the width the column is not using.
pub(crate) fn draw_details(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    project: &UiProjectView,
    document: &Document,
    commands: &mut Vec<UiCommand>,
) -> super::planning_setup::PlanningLayout {
    let mut layout = super::planning_setup::PlanningLayout::default();
    let rect = egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            super::planning_setup::stacked_column(
                ui,
                &mut layout,
                "solids_view_column",
                "solids_view_contents_island",
                |ui, rect| draw_properties(ui, rect, editor, project, document),
                |ui, rect| draw_contents(ui, rect, editor, document),
            );
            super::planning_setup::central_pane_of(ui, |ui, rect| {
                super::planning_setup::draw_solid_render(ui, rect, editor, project.active_session, commands);
            })
        })
        .inner;
    layout.rect = rect;
    layout
}

/// Heading shown above the tree, so the column says what it is listing.
pub(crate) fn tree_heading(ui: &mut egui::Ui) {
    heading(ui, &tr!("planning-solids"));
}

/// The Blasting step's heading over the same tree stopped at benches.
pub(crate) fn bench_tree_heading(ui: &mut egui::Ui) {
    heading(ui, &tr!("planning-benches"));
}

fn heading(ui: &mut egui::Ui, label: &str) {
    ui.horizontal(|ui| {
        ui.add_space(ui.spacing().indent);
        ui.label(bold(label));
    });
}
