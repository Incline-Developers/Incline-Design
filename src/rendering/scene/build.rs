//! Document/editor scene assembly entry points.

use std::collections::{HashMap, HashSet};

use glam::{DVec3, Mat4, Vec3};
use lyon::tessellation::VertexBuffers;

use crate::{
    model::{
        Document, FillStyle, Object, ObjectId, PolyVertex, SceneEntityId,
        geometry::{PolylineFillMesh, triangulate_polyline_fill},
    },
    rendering::{
        StrokeInstance, Vertex,
        geometry::{DrawContext, draw_line, draw_screen_cross, tessellate_polyline_stroke},
        graphics::{DOC_LINE_WIDTH, DOC_TEXT_FONT_SIZE, TEXT_EDIT_INDICATOR_COLOR, text_bounds_corners_with_layout_width},
        pick::{PickRecord, TextPickRecord, world_bounds_from_local_positions},
        scene::{
            document::{fill_polyline_hatch, fill_polyline_solid, polyline_hatch_spacing},
            document_style::{DocumentStyleSlots, STROKE_SELECTION_ONLY, STYLE_SLOT_NONE, TRANSLUCENT_ALPHA, is_highlighted},
        },
        text::{Text, TextBox, TextSystem},
    },
    ui::state::EditorState,
};

pub(crate) struct DocumentSceneBuildInput<'a> {
    pub(crate) editor: &'a EditorState,
    pub(crate) document: &'a Document,
    /// Objects owned by the static stroke chunk cache; skipped here.
    pub(crate) static_ids: &'a HashSet<ObjectId>,
    pub(crate) fill_cache: &'a mut PolylineFillCache,
    pub(crate) text_system: &'a mut TextSystem,
    pub(crate) slots: &'a mut DocumentStyleSlots,
    pub(crate) lyon_buffer: &'a mut VertexBuffers<Vertex, u32>,
    pub(crate) strokes: &'a mut Vec<StrokeInstance>,
    pub(crate) text_vertex_buf: &'a mut Vec<Vertex>,
    pub(crate) text_index_buf: &'a mut Vec<u32>,
    pub(crate) text_draw_batches: &'a mut Vec<TextDrawBatch>,
    pub(crate) pick_records: &'a mut Vec<PickRecord>,
    pub(crate) text_pick_records: &'a mut Vec<TextPickRecord>,
    pub(crate) object_ranges: &'a mut Vec<DocumentObjectRanges>,
    pub(crate) scene_origin: DVec3,
    pub(crate) scale_factor: f32,
}

/// Everything [`rebuild_document_scene`] bakes into its buffers. Selection,
/// hover and translucency are deliberately absent: the shaders apply them
/// and [`restyle_document_scene`] re-stages them, so changing them never
/// re-tessellates. `static_key` identifies the static chunks' claimed set.
pub(crate) fn document_scene_key(document: &Document, editor: &EditorState, static_key: u64, scene_origin: DVec3, scale_factor: f32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    // The viewport draws a composite rebuilt as a new document whenever a
    // project changes, so its allocation tells documents apart even when
    // their revision counters coincide.
    (document.objects().as_ptr() as usize).hash(&mut hasher);
    document.objects().len().hash(&mut hasher);
    document.revision().hash(&mut hasher);
    static_key.hash(&mut hasher);
    scene_origin.to_array().map(f64::to_bits).hash(&mut hasher);
    scale_factor.to_bits().hash(&mut hasher);
    let hidden = editor.hidden_handles.iter().fold(editor.hidden_handles.len() as u64, |acc, handle| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        handle.hash(&mut hasher);
        acc ^ hasher.finish()
    });
    hidden.hash(&mut hasher);
    editor.editing_labels_id.hash(&mut hasher);
    if editor.editing_labels_id.is_some() {
        editor.pending_text.hash(&mut hasher);
        editor.pending_text_height.to_bits().hash(&mut hasher);
        editor.pending_text_rotation_degrees.to_bits().hash(&mut hasher);
        editor.pending_text_color.map(f32::to_bits).hash(&mut hasher);
    }
    hasher.finish()
}

/// World-space fill meshes survive selection, style changes and scene rebasing.
/// Compare source geometry so a color-only object revision does not triangulate
/// again, while edits, undo and replacement documents still invalidate it.
#[derive(Default)]
pub(crate) struct PolylineFillCache {
    entries: HashMap<ObjectId, CachedPolylineFill>,
}

struct CachedPolylineFill {
    verts: Vec<PolyVertex>,
    mesh: Option<PolylineFillMesh>,
}

impl PolylineFillCache {
    pub(crate) fn release_layers(&mut self, document: &Document, layers: &[crate::model::LayerId]) {
        self.entries
            .retain(|id, _| document.get_object(*id).is_some_and(|object| !layers.contains(&object.layer())));
        self.entries.shrink_to_fit();
    }

    fn retain_document(&mut self, document: &Document) {
        // Anything that encloses an area and is actually hatched keeps its
        // cached mesh. Naming only the polyline variant here would evict every
        // filled circle on each rebuild and re-triangulate it from scratch.
        self.entries.retain(|id, _| {
            document
                .get_object(*id)
                .is_some_and(|object| object.encloses_area() && object.fill().is_some_and(|fill| fill != FillStyle::Clear))
        });
    }

    fn mesh(&mut self, id: ObjectId, verts: &[PolyVertex]) -> Option<&PolylineFillMesh> {
        let entry = self.entries.entry(id).or_insert_with(|| CachedPolylineFill {
            verts: verts.to_vec(),
            mesh: triangulate_polyline_fill(verts),
        });
        if entry.verts != verts {
            entry.verts = verts.to_vec();
            entry.mesh = triangulate_polyline_fill(verts);
        }
        entry.mesh.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DocumentPrimitive {
    Fill,
    Stroke,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DocumentRenderStage {
    Opaque,
    Translucent,
    Overlay,
    AlwaysVisible,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DocumentDrawBatch {
    pub(crate) primitive: DocumentPrimitive,
    /// Stroke instances, or fill indices.
    pub(crate) range: (u32, u32),
    pub(crate) center: DVec3,
    stage: DocumentRenderStage,
}

impl DocumentDrawBatch {
    pub(crate) fn stage(self, xray_enabled: bool) -> DocumentRenderStage {
        if xray_enabled { DocumentRenderStage::AlwaysVisible } else { self.stage }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TextDrawBatch {
    pub(crate) index_range: (u32, u32),
    stage: DocumentRenderStage,
}

impl TextDrawBatch {
    pub(crate) fn stage(self, xray_enabled: bool) -> DocumentRenderStage {
        if xray_enabled { DocumentRenderStage::AlwaysVisible } else { self.stage }
    }
}

/// One object's slice of the document streams, kept between rebuilds so
/// a style change can re-stage the object without re-tessellating it.
#[derive(Clone, Copy)]
pub(crate) struct DocumentObjectRanges {
    entity: SceneEntityId,
    /// Half-open range of stroke instances.
    stroke_range: (u32, u32),
    fill_index_range: (u32, u32),
    center: DVec3,
}

pub(crate) fn rebuild_document_scene(input: DocumentSceneBuildInput<'_>) {
    let DocumentSceneBuildInput {
        editor,
        document,
        static_ids,
        fill_cache,
        text_system,
        slots,
        lyon_buffer,
        strokes,
        text_vertex_buf,
        text_index_buf,
        text_draw_batches,
        pick_records,
        text_pick_records,
        object_ranges,
        scene_origin,
        scale_factor,
    } = input;

    fill_cache.retain_document(document);
    lyon_buffer.indices.clear();
    lyon_buffer.vertices.clear();
    strokes.clear();
    text_vertex_buf.clear();
    text_index_buf.clear();
    text_draw_batches.clear();
    pick_records.clear();
    text_pick_records.clear();
    object_ranges.clear();

    let mut draw_ctx = DrawContext::unstyled(strokes, &mut lyon_buffer.vertices, &mut lyon_buffer.indices, scene_origin, scale_factor);

    // The document is the sole source of geometry. Draw each object in world
    // space, honoring layer visibility and hiding, and record a pick range so
    // objects select and highlight; frozen ones are filtered at pick time.
    for object in document.objects() {
        if static_ids.contains(&object.id()) {
            continue;
        }
        let handle = SceneEntityId::Object(object.id());
        let layer_visible = document.layer(object.layer()).map(|layer| layer.loaded).unwrap_or(true);
        if !layer_visible || editor.hidden_handles.contains(&handle) {
            continue;
        }
        draw_ctx.style = slots.slot(object.id());
        let rgba = document.object_rgba(object);
        let stroke_start = draw_ctx.strokes.len() as u32;
        let fill_start = draw_ctx.fill_vertex_buf.len() as u32;
        let fill_index_start = draw_ctx.fill_index_buf.len() as u32;
        let mut fill_opaque = false;
        let mut pickable = true;
        match object {
            Object::Point { pos, .. } => {
                draw_screen_cross(&mut draw_ctx, *pos, 6.0, DOC_LINE_WIDTH, rgba);
            }
            // A circle draws exactly as the closed bulged string it is
            // equivalent to, so both variants share one path through
            // `string_geometry` rather than duplicating stroke and hatch.
            Object::Polyline { fill, line_weight, .. } | Object::Circle { fill, line_weight, .. } => {
                let line_rgba = rgba;
                let fill_rgba = document.object_fill_rgba(object);
                let geometry = object.string_geometry();
                let (verts, closed) = geometry.as_ref().map_or((&[][..], false), |(verts, closed)| (verts.as_ref(), *closed));
                tessellate_polyline_stroke(&mut draw_ctx, verts, closed, *line_weight, line_rgba);
                if closed
                    && verts.len() >= 2
                    && *fill != crate::model::FillStyle::Clear
                    && let Some(fill_mesh) = fill_cache.mesh(object.id(), verts)
                {
                    match fill {
                        crate::model::FillStyle::Solid => {
                            fill_opaque = fill_rgba[3] >= 1.0 - f32::EPSILON;
                            fill_polyline_solid(
                                draw_ctx.fill_vertex_buf,
                                draw_ctx.fill_index_buf,
                                fill_mesh,
                                fill_rgba,
                                draw_ctx.scene_origin,
                                draw_ctx.style,
                            );
                        }
                        crate::model::FillStyle::Slashes | crate::model::FillStyle::Crosses => {
                            let hatch_spacing = polyline_hatch_spacing(fill_mesh, 15.0);
                            fill_polyline_hatch(&mut draw_ctx, fill_mesh, fill_rgba, 45.0, hatch_spacing, *line_weight);
                            if *fill == crate::model::FillStyle::Crosses {
                                fill_polyline_hatch(&mut draw_ctx, fill_mesh, fill_rgba, 135.0, hatch_spacing, *line_weight);
                            }
                        }
                        crate::model::FillStyle::Clear => {}
                    }
                }
            }
            Object::Text {
                pos, content, height, rotation, ..
            } => {
                // Text is picked by its layout box, never by the selection
                // outline drawn around it.
                pickable = false;
                let is_editing = editor.editing_labels_id == Some(object.id());
                let (render_content, render_height, render_rotation, render_rgba) = if is_editing {
                    (
                        &editor.pending_text,
                        editor.pending_text_height,
                        editor.pending_text_rotation_degrees.to_radians(),
                        editor.pending_text_color,
                    )
                } else {
                    (content, *height, *rotation, rgba)
                };
                let mut textbox = TextBox::new(vec![Text::new(render_content.clone(), render_rgba)], 1.0);
                textbox.font_size = DOC_TEXT_FONT_SIZE;
                let scale = (render_height / textbox.font_size as f64).max(f64::EPSILON) as f32;
                let layout_bounds = (f32::MAX, f32::MAX);
                let layout_width = f64::from(textbox.layout_width(text_system, layout_bounds)) * f64::from(scale);
                let corners = text_bounds_corners_with_layout_width(*pos, render_content, render_height, render_rotation, layout_width);
                text_pick_records.push(TextPickRecord { entity: handle, corners });
                // The box is always tessellated and shown by the shader while
                // the text is selected; the editing box keeps its own colour.
                let (box_style, box_color) = if is_editing {
                    (STYLE_SLOT_NONE, TEXT_EDIT_INDICATOR_COLOR)
                } else {
                    (draw_ctx.style | STROKE_SELECTION_ONLY, crate::ui::SELECTION_COLOR_F32)
                };
                let slot = std::mem::replace(&mut draw_ctx.style, box_style);
                for edge in corners.iter().copied().zip(corners.iter().copied().cycle().skip(1)).take(4) {
                    draw_line(&mut draw_ctx, edge.0, edge.1, 2.0, box_color);
                }
                draw_ctx.style = slot;
                let local = *pos - scene_origin;
                let matrix = Mat4::from_translation(local.as_vec3()) * Mat4::from_rotation_z(render_rotation as f32) * Mat4::from_scale(Vec3::new(scale, -scale, scale));
                let text_index_start = text_index_buf.len() as u32;
                textbox.append_meshes(text_system, layout_bounds, matrix, text_vertex_buf, text_index_buf);
                let text_index_end = text_index_buf.len() as u32;
                if text_index_start < text_index_end {
                    push_text_batch(
                        text_draw_batches,
                        TextDrawBatch {
                            index_range: (text_index_start, text_index_end),
                            stage: text_render_stage(is_editing),
                        },
                    );
                }
            }
        }
        let stroke_end = draw_ctx.strokes.len() as u32;
        let fill_end = draw_ctx.fill_vertex_buf.len() as u32;
        let fill_index_end = draw_ctx.fill_index_buf.len() as u32;
        if stroke_end > stroke_start || fill_index_end > fill_index_start {
            object_ranges.push(DocumentObjectRanges {
                entity: handle,
                stroke_range: (stroke_start, stroke_end),
                fill_index_range: (fill_index_start, fill_index_end),
                center: object_center(object),
            });
        }
        if pickable
            && (stroke_end > stroke_start || fill_end > fill_start)
            && let Some(world_bounds) = world_bounds_from_local_positions(
                draw_ctx.strokes[stroke_start as usize..stroke_end as usize]
                    .iter()
                    .flat_map(|stroke| {
                        let (start, end) = stroke.world_ends();
                        [start, end]
                    })
                    .chain(draw_ctx.fill_vertex_buf[fill_start as usize..fill_end as usize].iter().map(|vertex| vertex.pos)),
                scene_origin,
            )
        {
            pick_records.push(PickRecord {
                entity: handle,
                world_bounds,
                stroke_range: (stroke_start, stroke_end),
                fill_range: (fill_start, fill_end),
                fill_index_range: (fill_index_start, fill_index_end),
                fill_opaque,
            });
        }
    }
}

/// Re-stage the document batches for the current selection, hover and
/// translucency. Cheap: it walks object ranges and alphas, never geometry.
pub(crate) fn restyle_document_scene(
    batches: &mut Vec<DocumentDrawBatch>,
    objects: &[DocumentObjectRanges],
    editor: &EditorState,
    strokes: &[StrokeInstance],
    fill_vertices: &[Vertex],
    fill_indices: &[u32],
) {
    batches.clear();
    for object in objects {
        let always_visible = editor.editing_labels_id.is_some_and(|id| object.entity == SceneEntityId::Object(id));
        let overlay = is_highlighted(editor, object.entity);
        // Frozen objects are never restyled, translucency included.
        let alpha_scale = if editor.translucent_handles.contains(&object.entity) && !editor.frozen_handles.contains(&object.entity) {
            TRANSLUCENT_ALPHA
        } else {
            1.0
        };
        let stage_for = |alpha: Option<f32>| {
            if always_visible {
                DocumentRenderStage::AlwaysVisible
            } else if overlay {
                DocumentRenderStage::Overlay
            } else if alpha.unwrap_or(1.0) * alpha_scale < 0.999 {
                DocumentRenderStage::Translucent
            } else {
                DocumentRenderStage::Opaque
            }
        };
        append_stage_runs(batches, DocumentPrimitive::Stroke, object.stroke_range, 1, object.center, |start| {
            stage_for(strokes.get(start).map(|stroke| stroke.color[3]))
        });
        append_stage_runs(batches, DocumentPrimitive::Fill, object.fill_index_range, 3, object.center, |start| {
            stage_for(fill_indices.get(start).and_then(|&index| fill_vertices.get(index as usize)).map(|vertex| vertex.color[3]))
        });
    }
}

fn object_center(object: &Object) -> DVec3 {
    match object {
        Object::Point { pos, .. } | Object::Text { pos, .. } => *pos,
        Object::Circle { center, .. } => *center,
        Object::Polyline { verts, .. } => average_positions(verts.iter().map(|vertex| vertex.pos)),
    }
}

fn average_positions(points: impl Iterator<Item = DVec3>) -> DVec3 {
    let (sum, count) = points.fold((DVec3::ZERO, 0usize), |(sum, count), point| (sum + point, count + 1));
    if count == 0 { DVec3::ZERO } else { sum / count as f64 }
}

/// Split `range` into runs of one stage. `step` is the primitive's length in
/// the stream (one stroke instance, or three fill indices); `stage_at`
/// judges the primitive starting at an offset.
fn append_stage_runs(
    batches: &mut Vec<DocumentDrawBatch>,
    primitive: DocumentPrimitive,
    range: (u32, u32),
    step: usize,
    center: DVec3,
    stage_at: impl Fn(usize) -> DocumentRenderStage,
) {
    let (start, end) = (range.0 as usize, range.1 as usize);
    let end = start + (end.saturating_sub(start) / step) * step;
    if start >= end {
        return;
    }
    let mut run_start = start;
    let mut run_stage = stage_at(start);
    for primitive_start in (start + step..end).step_by(step) {
        let stage = stage_at(primitive_start);
        if stage == run_stage {
            continue;
        }
        push_document_batch(
            batches,
            DocumentDrawBatch {
                primitive,
                range: (run_start as u32, primitive_start as u32),
                center,
                stage: run_stage,
            },
        );
        run_start = primitive_start;
        run_stage = stage;
    }
    push_document_batch(
        batches,
        DocumentDrawBatch {
            primitive,
            range: (run_start as u32, end as u32),
            center,
            stage: run_stage,
        },
    );
}

fn push_document_batch(batches: &mut Vec<DocumentDrawBatch>, batch: DocumentDrawBatch) {
    // Fill and stroke indices live in separate streams, so batches of one
    // primitive can remain contiguous even when the other primitive was
    // appended between them. Coalescing opaque/overlay runs keeps large point
    // and other documents at a handful of draw calls; translucent runs retain
    // per-object centers for back-to-front sorting.
    if let Some(previous) = batches.iter_mut().rev().find(|previous| previous.primitive == batch.primitive)
        && previous.stage == batch.stage
        && previous.range.1 == batch.range.0
        && (batch.stage != DocumentRenderStage::Translucent || previous.center == batch.center)
    {
        previous.range.1 = batch.range.1;
        return;
    }
    batches.push(batch);
}

fn push_text_batch(batches: &mut Vec<TextDrawBatch>, batch: TextDrawBatch) {
    if let Some(previous) = batches.last_mut()
        && previous.stage == batch.stage
        && previous.index_range.1 == batch.index_range.0
        && batch.stage != DocumentRenderStage::Translucent
    {
        previous.index_range.1 = batch.index_range.1;
    } else {
        batches.push(batch);
    }
}

fn text_render_stage(is_editing: bool) -> DocumentRenderStage {
    if is_editing {
        DocumentRenderStage::AlwaysVisible
    } else {
        // Match an ordinary selected-text box: draw late while retaining
        // scene depth.
        DocumentRenderStage::Overlay
    }
}

pub(crate) struct DynamicSceneBuildInput<'a> {
    pub(crate) editor: &'a EditorState,
    pub(crate) dynamic_strokes: &'a mut Vec<StrokeInstance>,
    pub(crate) scene_origin: DVec3,
    pub(crate) scale_factor: f32,
}

/// Per-frame geometry for the live drawing tools: the batter/berm preview.
/// Stroke-only and tiny, so rebuilding it every frame while a tool is active
/// is cheap - unlike the full document scene it replaces in that role.
pub(crate) fn rebuild_dynamic_scene(input: DynamicSceneBuildInput<'_>) {
    let DynamicSceneBuildInput {
        editor,
        dynamic_strokes,
        scene_origin,
        scale_factor,
    } = input;

    dynamic_strokes.clear();

    let mut unused_fill_vertices: Vec<Vertex> = Vec::new();
    let mut unused_fill_indices: Vec<u32> = Vec::new();
    let mut draw_ctx = DrawContext::unstyled(dynamic_strokes, &mut unused_fill_vertices, &mut unused_fill_indices, scene_origin, scale_factor);

    if editor.batter_berm_dialog_open {
        const BATTER_BERM_PREVIEW_COLOR: [f32; 4] = [1.0, 0.86, 0.0, 1.0];

        for ring in &editor.batter_berm_rings_world {
            for pair in ring.windows(2) {
                draw_line(&mut draw_ctx, pair[0], pair[1], 2.0, BATTER_BERM_PREVIEW_COLOR);
            }
            if editor.batter_berm_preview_closed
                && ring.len() >= 2
                && let (Some(&first), Some(&last)) = (ring.first(), ring.last())
            {
                draw_line(&mut draw_ctx, last, first, 2.0, BATTER_BERM_PREVIEW_COLOR);
            }
        }
    }
}
