//! Editor styling of document geometry (selection, hover, translucency),
//! applied by the document shaders rather than baked into vertices.
//!
//! Every document object owns a stable style slot. Its stroke instances and
//! fill vertices carry the slot, and a storage buffer holds one flag word per
//! slot. Selecting or hovering an object rewrites that small buffer instead
//! of re-tessellating the scene - and objects in the static stroke chunks keep
//! their chunk no matter how they are styled.

use std::collections::{HashMap, HashSet};

use crate::{
    model::{Document, ObjectId, SceneEntityId},
    ui::state::EditorState,
};

/// Low bits of a style word: the slot. `STYLE_SLOT_NONE` is never styled.
pub(crate) const STYLE_SLOT_MASK: u32 = 0x0fff_ffff;
pub(crate) const STYLE_SLOT_NONE: u32 = STYLE_SLOT_MASK;
/// A disc of radius `half_width_px` at `start`.
pub(crate) const STROKE_ROUND: u32 = 1 << 28;
/// A bar through `start` spanning `±end.xy` pixels on screen.
pub(crate) const STROKE_SCREEN_AXIS: u32 = 1 << 29;
/// Drawn only while the owning slot is selected.
pub(crate) const STROKE_SELECTION_ONLY: u32 = 1 << 30;

/// Flag bits of one slot's word in the style buffer.
pub(crate) const STYLE_SELECTED: u32 = 1;
pub(crate) const STYLE_HOVER: u32 = 2;
pub(crate) const STYLE_TRANSLUCENT: u32 = 4;

/// Alpha multiplier of a translucent object, as `make_translucent` applies it.
pub(crate) const TRANSLUCENT_ALPHA: f32 = 0.3;

/// Stable object-to-slot assignment shared by the stream builder and the
/// static stroke chunks, so a slot baked into a chunk stays valid while the
/// stream around it rebuilds.
#[derive(Default)]
pub(crate) struct DocumentStyleSlots {
    by_object: HashMap<ObjectId, u32>,
    free: Vec<u32>,
    next: u32,
}

impl DocumentStyleSlots {
    pub(crate) fn slot(&mut self, id: ObjectId) -> u32 {
        if let Some(&slot) = self.by_object.get(&id) {
            return slot;
        }
        let slot = self.free.pop().unwrap_or_else(|| {
            let slot = self.next;
            self.next = (self.next + 1).min(STYLE_SLOT_NONE);
            slot
        });
        self.by_object.insert(id, slot);
        slot
    }

    pub(crate) fn get(&self, id: ObjectId) -> Option<u32> {
        self.by_object.get(&id).copied()
    }

    /// Release the slots of objects no longer in `document`. Call only after
    /// every holder of baked slots (stream and chunks) has rebuilt against
    /// the same document, so no geometry still names a released slot.
    pub(crate) fn retain(&mut self, document: &Document) {
        let free = &mut self.free;
        self.by_object.retain(|id, slot| {
            let keep = document.get_object(*id).is_some();
            if !keep {
                free.push(*slot);
            }
            keep
        });
    }

    /// One flag word per slot for the current editor state. Frozen objects
    /// are never restyled, as they have never been.
    pub(crate) fn flags(&self, editor: &EditorState) -> Vec<u32> {
        let mut flags = vec![0_u32; (self.next as usize).max(1)];
        let mut mark = |handles: &mut dyn Iterator<Item = ObjectId>, bit: u32| {
            for id in handles {
                if editor.frozen_handles.contains(&SceneEntityId::Object(id)) {
                    continue;
                }
                if let Some(slot) = self.get(id) {
                    flags[slot as usize] |= bit;
                }
            }
        };
        mark(&mut objects(&editor.translucent_handles), STYLE_TRANSLUCENT);
        mark(&mut objects(&editor.tri_hover_handles), STYLE_HOVER);
        mark(&mut objects(&editor.selected_handles), STYLE_SELECTED);
        mark(&mut editor.tool_highlight_id.into_iter(), STYLE_SELECTED);
        flags
    }
}

fn objects(handles: &HashSet<SceneEntityId>) -> impl Iterator<Item = ObjectId> + '_ {
    handles.iter().filter_map(|handle| match handle {
        SceneEntityId::Object(id) => Some(*id),
        _ => None,
    })
}

/// Whether the object is drawn as the editor highlights it (selected,
/// hovered or tool-highlighted), which moves it to the overlay stage.
pub(crate) fn is_highlighted(editor: &EditorState, entity: SceneEntityId) -> bool {
    editor.selected_handles.contains(&entity) || editor.tri_hover_handles.contains(&entity) || editor.tool_highlight_id.is_some_and(|id| entity == SceneEntityId::Object(id))
}

/// WGSL for group 1 of the document pipelines, with the highlight colours
/// taken from the constants the rest of the editor uses.
pub(crate) fn shader_prelude() -> String {
    let color = |rgba: [f32; 4]| format!("vec4<f32>({:?}, {:?}, {:?}, {:?})", rgba[0], rgba[1], rgba[2], rgba[3]);
    format!(
        "const STYLE_SLOT_MASK: u32 = {STYLE_SLOT_MASK}u;\n\
         const STROKE_ROUND: u32 = {STROKE_ROUND}u;\n\
         const STROKE_SCREEN_AXIS: u32 = {STROKE_SCREEN_AXIS}u;\n\
         const STROKE_SELECTION_ONLY: u32 = {STROKE_SELECTION_ONLY}u;\n\
         const STYLE_SELECTED: u32 = {STYLE_SELECTED}u;\n\
         const STYLE_HOVER: u32 = {STYLE_HOVER}u;\n\
         const STYLE_TRANSLUCENT: u32 = {STYLE_TRANSLUCENT}u;\n\
         const TRANSLUCENT_ALPHA: f32 = {TRANSLUCENT_ALPHA:?};\n\
         const SELECTION_COLOR: vec4<f32> = {};\n\
         const HOVER_COLOR: vec4<f32> = {};\n\
         {}",
        color(crate::ui::SELECTION_COLOR_F32),
        color(crate::rendering::graphics::YELLOW_HIGHLIGHT_COLOR),
        include_str!("../shaders/document_style.wgsl"),
    )
}

/// The GPU half: the flag buffer and the two bind groups over it. The
/// ordinary group draws every member; the highlighted-only group lets the
/// static chunks redraw just their highlighted members in the overlay stage.
pub(crate) struct DocumentStyleGpu {
    pub(crate) layout: wgpu::BindGroupLayout,
    flags_buffer: wgpu::Buffer,
    flags_capacity: usize,
    all_pass: wgpu::Buffer,
    highlighted_pass: wgpu::Buffer,
    pub(crate) all_bind_group: wgpu::BindGroup,
    pub(crate) highlighted_bind_group: wgpu::BindGroup,
    /// Whether any static chunk member is highlighted, so the overlay stage
    /// needs the highlighted-only redraw of the chunks.
    pub(crate) static_highlighted: bool,
}

impl DocumentStyleGpu {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Document Style Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pass_buffer = |label, highlighted_only: u32| {
            wgpu::util::DeviceExt::create_buffer_init(
                device,
                &wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: bytemuck::cast_slice(&[highlighted_only, 0, 0, 0]),
                    usage: wgpu::BufferUsages::UNIFORM,
                },
            )
        };
        let all_pass = pass_buffer("Document Style Pass (All)", 0);
        let highlighted_pass = pass_buffer("Document Style Pass (Highlighted)", 1);
        let flags_buffer = Self::create_flags_buffer(device, 1);
        let (all_bind_group, highlighted_bind_group) = Self::bind_groups(device, &layout, &flags_buffer, &all_pass, &highlighted_pass);
        Self {
            layout,
            flags_buffer,
            flags_capacity: 1,
            all_pass,
            highlighted_pass,
            all_bind_group,
            highlighted_bind_group,
            static_highlighted: false,
        }
    }

    fn create_flags_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Document Style Flags"),
            size: (capacity * size_of::<u32>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn bind_groups(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        flags: &wgpu::Buffer,
        all: &wgpu::Buffer,
        highlighted: &wgpu::Buffer,
    ) -> (wgpu::BindGroup, wgpu::BindGroup) {
        let group = |label, pass: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: flags.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: pass.as_entire_binding(),
                    },
                ],
            })
        };
        (group("Document Style (All)", all), group("Document Style (Highlighted)", highlighted))
    }

    /// Upload one flag word per slot. The buffer only grows; slots past the
    /// written range read as unstyled because the shader bounds-checks
    /// against the whole buffer, so stale words are zeroed on growth.
    pub(crate) fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, flags: &[u32]) {
        if flags.len() > self.flags_capacity {
            let max_items = usize::try_from(device.limits().max_storage_buffer_binding_size).unwrap_or(usize::MAX) / size_of::<u32>();
            self.flags_capacity = flags.len().next_power_of_two().min(max_items).max(1);
            self.flags_buffer = Self::create_flags_buffer(device, self.flags_capacity);
            (self.all_bind_group, self.highlighted_bind_group) = Self::bind_groups(device, &self.layout, &self.flags_buffer, &self.all_pass, &self.highlighted_pass);
        }
        let written = flags.len().min(self.flags_capacity);
        let mut padded = Vec::with_capacity(self.flags_capacity);
        padded.extend_from_slice(&flags[..written]);
        padded.resize(self.flags_capacity, 0);
        queue.write_buffer(&self.flags_buffer, 0, bytemuck::cast_slice(&padded));
    }
}
