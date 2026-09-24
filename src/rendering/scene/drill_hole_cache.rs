use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
};

use glam::DVec3;
use wgpu::util::DeviceExt;

use crate::{
    i18n::tr_format,
    model::drill_hole::{
        COLLAR_MARKER_FILL_COLOR, COLLAR_MARKER_MIN_PIXEL_DIAMETER, COLLAR_MARKER_OUTLINE_COLOR, COLLAR_MARKER_RADIUS_SCALE, DISC_MIN_PIXEL_LENGTH, DrillColorState,
        DrillFieldKind, DrillHoleId, DrillHoleStyle, DrillValue, HoleDisc, MIN_RENDER_PIXEL_DIAMETER, OpenDrillHoleDataset, TIE_RADIUS_SCALE, TieIn, hole_discs,
    },
};

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DrillSegmentInstance {
    pub(crate) start: [f32; 3],
    pub(crate) radius: f32,
    pub(crate) end: [f32; 3],
    pub(crate) pixel_diameter: f32,
    pub(crate) color: [f32; 3],
    /// This instance's slot in the dataset's selection bitset - see
    /// [`selection_bits_for`].
    pub(crate) selection_index: u32,
    /// String-and-discs only: least on-screen length along the axis, in
    /// physical pixels, that the shader stretches a disc to about its
    /// midpoint. Zero for a true-diameter cylinder or a string, neither of
    /// which is ever lengthened.
    pub(crate) pixel_length: f32,
    /// String-and-discs only: a disc's place among its hole's discs,
    /// thinnest highest, packed into (0, 1]. Breaks the tie when a
    /// lengthened disc's screen box grows into a neighbour - see
    /// [`bucket_instances`]'s within-cell ordering - by z-nudging and
    /// draw-ordering the thinner disc on top. Zero for anything that is not
    /// a disc.
    pub(crate) depth_rank: f32,
    /// String-and-discs only: scene-relative position of the whole disc's
    /// first point (`position_at_depth(disc.from)`), the same for every
    /// piece the disc was split into at a station. The shader lengthens a
    /// thin disc about the midpoint of this whole span, not of the one
    /// piece an instance happens to carry, so a disc split at a station
    /// still stretches as one interval. Zero for anything that is not a
    /// disc.
    pub(crate) disc_start: [f32; 3],
    /// The disc's whole span's other end - see [`Self::disc_start`]. Zero
    /// for anything that is not a disc.
    pub(crate) disc_end: [f32; 3],
}

/// One spatial bucket of a dataset's segment instances: a contiguous range
/// and the scene-relative box bounding every cylinder in it. Boxes overlap
/// where a merged run crosses a cell edge. A box is padded by world radius
/// alone, so a culled cell can lose up to half the set's screen floor: a
/// sliver under a pixel at the default, wider on a set that raised it. A
/// string-and-discs disc is the same kind of sliver risk from a different
/// cause: the shader may stretch it on screen past `pixel_length` about its
/// midpoint, wider than the world-space box computed here.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DrillCell {
    pub(crate) min: glam::Vec3,
    pub(crate) max: glam::Vec3,
    pub(crate) start: u32,
    pub(crate) end: u32,
}

/// The disc drawn at the top of a hole so its collar reads at a glance
/// instead of being the indistinguishable end of a cylinder.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DrillCollarInstance {
    pub(crate) center: [f32; 3],
    pub(crate) marker_radius: f32,
    pub(crate) outline: [f32; 3],
    pub(crate) pixel_diameter: f32,
    pub(crate) fill: [f32; 3],
    pub(crate) hole_radius: f32,
    pub(crate) selection_index: u32,
    /// The trace's own pixel floor, so the marker is lifted clear of the
    /// trace as it is drawn far away, not as it would be at a fixed floor.
    pub(crate) trace_pixel_diameter: f32,
}

/// How many `vec4<u32>` the selection uniform's bitset array holds. WebGPU
/// guarantees only 64 KiB for one uniform binding, and the header takes 32
/// bytes of it, so the array is 4094 elements and the whole binding is
/// exactly 65536 bytes. A uniform array needs a 16-byte stride, hence four
/// bitset words per element rather than a flat `array<u32>`.
const SELECTION_VECTORS: usize = 4094;
const SELECTION_WORDS: usize = SELECTION_VECTORS * 4;
const SELECTION_CAPACITY: usize = SELECTION_WORDS * 32;
const SELECTION_BUFFER_BYTES: usize = size_of::<SelectionHeader>() + SELECTION_VECTORS * 16;

const _: () = assert!(SELECTION_BUFFER_BYTES <= 64 * 1024);

/// The 32-byte header of a dataset's selection uniform buffer, matched field
/// for field by the `Selection` struct in `drill_hole.wgsl` and
/// `drill_collar.wgsl`. Followed in the buffer by the bitset words - see
/// [`selection_bits_for`].
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SelectionHeader {
    selection_color: [f32; 4],
    whole: u32,
    hole_count: u32,
    word_count: u32,
    _pad: u32,
}

/// A dataset's selection bitset, rebuilt only when the dataset key or
/// [`HoleSelection::key`] changed. Bit layout: holes occupy
/// `[0, hole_count)`; tie-ins occupy `[hole_count, hole_count + tie_count)`,
/// indexed by their position in `dataset.dataset.ties` (not the position
/// among ties that survive [`build_tie_instances`]'s `filter_map`, so a tie
/// with a dangling hole reference still owns a stable, if unused, bit).
struct SelectionBits {
    header: SelectionHeader,
    words: Vec<u32>,
    over_capacity: usize,
}

impl SelectionBits {
    /// Header plus the live words only: the shader never reads past
    /// `word_count`, so a write into a live buffer needs nothing more.
    fn live_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(size_of::<SelectionHeader>() + self.words.len() * 4);
        bytes.extend_from_slice(bytemuck::bytes_of(&self.header));
        bytes.extend_from_slice(bytemuck::cast_slice(&self.words));
        bytes
    }

    fn bytes(&self) -> Vec<u8> {
        let mut bytes = self.live_bytes();
        bytes.resize(SELECTION_BUFFER_BYTES, 0);
        bytes
    }
}

fn selection_bits_for(hole_count: usize, ties: &[TieIn], selection: &HoleSelection) -> SelectionBits {
    let needed = hole_count + ties.len();
    let [red, green, blue, alpha] = crate::ui::SELECTION_COLOR_F32;
    let mut header = SelectionHeader {
        selection_color: [red, green, blue, alpha],
        whole: u32::from(selection.whole),
        hole_count: hole_count as u32,
        word_count: 0,
        _pad: 0,
    };
    if needed > SELECTION_CAPACITY {
        return SelectionBits {
            header,
            words: Vec::new(),
            over_capacity: needed,
        };
    }
    let word_count = needed.max(1).div_ceil(32);
    let mut words = vec![0u32; word_count];
    if !selection.whole {
        for &hole in &selection.holes {
            if hole < hole_count {
                words[hole / 32] |= 1 << (hole % 32);
            }
        }
    }
    for (index, tie) in ties.iter().enumerate() {
        if selection.contains_tie(tie.from, tie.to) {
            let bit = hole_count + index;
            words[bit / 32] |= 1 << (bit % 32);
        }
    }
    header.word_count = word_count as u32;
    SelectionBits { header, words, over_capacity: 0 }
}

fn empty_selection_bytes() -> Vec<u8> {
    SelectionBits {
        header: SelectionHeader {
            selection_color: [0.0; 4],
            whole: 0,
            hole_count: 0,
            word_count: 0,
            _pad: 0,
        },
        words: Vec::new(),
        over_capacity: 0,
    }
    .bytes()
}

fn create_selection_buffer_and_bind_group(device: &wgpu::Device, queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout, bits: &SelectionBits) -> (wgpu::Buffer, wgpu::BindGroup) {
    // Full uniform size: the binding must match the shader's fixed array.
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Drillhole Selection Bitset"),
        size: SELECTION_BUFFER_BYTES as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, &bits.live_bytes());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Drillhole Selection Bind Group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });
    (buffer, bind_group)
}

pub(crate) struct CachedDrillHoles {
    pub(crate) buffer: Option<wgpu::Buffer>,
    pub(crate) count: u32,
    /// Cells over `buffer` for frustum culling. Empty exactly when there is
    /// no buffer to cull - no instances were built - since a real entry's
    /// cells, once built, always cover its buffer in full. The pattern
    /// preview draws its own buffer whole rather than cell by cell and
    /// leaves this empty for the same reason.
    pub(crate) cells: Vec<DrillCell>,
    /// Index into `cells` where disc cells begin: string cells (or, for a
    /// true-diameter set, the whole buffer) occupy `[0, disc_cells_from)`,
    /// discs occupy the rest. `cells.len()` for a true-diameter set or an
    /// empty buffer, so a split at that index draws everything in the first
    /// half and nothing in the second - see `passes::draw_drill_holes`,
    /// which draws every dataset's first half before any dataset's second
    /// so all strings win the x-ray draw order over all discs.
    pub(crate) disc_cells_from: usize,
    pub(crate) tie_buffer: Option<wgpu::Buffer>,
    pub(crate) tie_count: u32,
    pub(crate) collar_buffer: Option<wgpu::Buffer>,
    pub(crate) collar_count: u32,
    /// The dataset's persistent selection uniform buffer, `None` only for
    /// the pattern-preview entry. A selection change writes new bytes in
    /// place instead of reallocating, so a click never touches `buffer`,
    /// `tie_buffer` or `collar_buffer` above.
    pub(crate) selection_buffer: Option<wgpu::Buffer>,
    /// Never optional. A drill draw binds group 1 or it is not a legal draw,
    /// and an entry that could not offer one used to be skipped silently,
    /// which is how every drill hole disappeared from a canvas whose GPU had
    /// no complaint to make. An entry with no bitset of its own binds the
    /// shared all-clear buffer.
    pub(crate) selection_bind_group: wgpu::BindGroup,
    key: u64,
    selection_key: u64,
}

/// Every string-and-discs set's discs, hole by hole, as the last build drew
/// them, so a pick reads them rather than working them out on every click.
/// Held apart from the GPU entries so a pick needs no device, and keyed by
/// only what the discs depend on.
#[derive(Default)]
pub(crate) struct DiscSpans {
    sets: HashMap<DrillHoleId, (u64, Vec<Vec<HoleDisc>>)>,
}

impl DiscSpans {
    /// What a set's discs depend on: its geometry and intervals (both
    /// behind the revision), the colour field and the style.
    pub(crate) fn key(dataset: &OpenDrillHoleDataset) -> u64 {
        let mut hash = DefaultHasher::new();
        dataset.id.hash(&mut hash);
        dataset.state.loaded.hash(&mut hash);
        dataset.state.revision().hash(&mut hash);
        dataset.color.active_field.hash(&mut hash);
        dataset.color.hole_style.hash(&mut hash);
        hash.finish()
    }

    /// The discs of each of `dataset`'s holes, indexed like its holes, or
    /// `None` when the last build is stale or never happened, as between an
    /// edit and the next frame: the caller works them out itself then.
    pub(crate) fn get(&self, dataset: &OpenDrillHoleDataset) -> Option<&[Vec<HoleDisc>]> {
        self.sets.get(&dataset.id).filter(|(key, _)| *key == Self::key(dataset)).map(|(_, holes)| holes.as_slice())
    }

    pub(crate) fn insert(&mut self, dataset: &OpenDrillHoleDataset, holes: Vec<Vec<HoleDisc>>) {
        self.sets.insert(dataset.id, (Self::key(dataset), holes));
    }

    pub(crate) fn remove(&mut self, id: DrillHoleId) {
        self.sets.remove(&id);
    }

    pub(crate) fn retain(&mut self, keep: impl Fn(DrillHoleId) -> bool) {
        self.sets.retain(|id, _| keep(*id));
    }

    pub(crate) fn clear(&mut self) {
        self.sets.clear();
    }
}

pub(crate) struct DrillHoleGpuCache {
    entries: HashMap<DrillHoleId, CachedDrillHoles>,
    /// The discs the entries above were built from, for picking.
    disc_spans: DiscSpans,
    /// Transient pattern-menu geometry, kept outside the id-keyed project
    /// entries so it can use the normal drill shaders without pretending to
    /// be a dataset before Create is pressed.
    preview: Option<CachedDrillHoles>,
    /// Bumped whenever the instances above are rebuilt, dropped or replaced.
    ///
    /// A drill hole dataset is edited in place - a tie laid, a collar turned -
    /// with the item's revision as the only signal that anything changed, so
    /// there is nothing in the dataset itself for the cached scene image in
    /// `frame::main_scene_cache_key` to key on. It hashes this counter
    /// instead: every rebuild here re-renders the scene the instances are
    /// drawn into, rather than leaving the edit invisible until the camera
    /// next moves.
    content_key: u64,
    warned_over_capacity: std::collections::HashSet<DrillHoleId>,
    /// Sets whose merge report has been logged: once per set, not once per
    /// rebuild, since a colour stop drag rebuilds every frame.
    reported_build: std::collections::HashSet<DrillHoleId>,
    /// Group 1 layout for every drill pipeline. A uniform because a
    /// vertex-stage storage buffer is missing on some WebGPU canvases.
    selection_layout: wgpu::BindGroupLayout,
    /// Cloned by any entry with no bitset of its own - see
    /// `CachedDrillHoles::selection_bind_group` - rather than rebuilt.
    empty_selection_bind_group: wgpu::BindGroup,
}

#[derive(Clone, Copy, PartialEq)]
enum SyncAction {
    Keep,
    RebuildSelection,
    RebuildAll,
}

fn sync_action(existing: Option<(u64, u64)>, key: u64, selection_key: u64) -> SyncAction {
    match existing {
        Some((existing_key, existing_selection_key)) if existing_key == key => {
            if existing_selection_key == selection_key {
                SyncAction::Keep
            } else {
                SyncAction::RebuildSelection
            }
        }
        _ => SyncAction::RebuildAll,
    }
}

impl DrillHoleGpuCache {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let selection_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("drill_selection_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let empty_selection_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Drillhole Empty Selection Bitset"),
            contents: &empty_selection_bytes(),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let empty_selection_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Drillhole Empty Selection Bind Group"),
            layout: &selection_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: empty_selection_buffer.as_entire_binding(),
            }],
        });
        Self {
            entries: HashMap::new(),
            disc_spans: DiscSpans::default(),
            preview: None,
            content_key: 0,
            warned_over_capacity: std::collections::HashSet::new(),
            reported_build: std::collections::HashSet::new(),
            selection_layout,
            empty_selection_bind_group,
        }
    }

    /// Drops the entries, the preview and `disc_spans`, and bumps
    /// `content_key`; keeps the selection layout, the empty bind group, and
    /// the two once-per-set sets `warned_over_capacity` and
    /// `reported_build`.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.disc_spans.clear();
        self.preview = None;
        self.content_key = self.content_key.wrapping_add(1);
    }

    pub(crate) fn selection_layout(&self) -> &wgpu::BindGroupLayout {
        &self.selection_layout
    }

    /// The discs each set was last built with, for the pick.
    pub(crate) fn disc_spans(&self) -> &DiscSpans {
        &self.disc_spans
    }

    pub(crate) fn sync(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, scene_origin: DVec3, datasets: &[OpenDrillHoleDataset], editor: &crate::ui::state::EditorState) {
        let retained = self.entries.len();
        self.entries.retain(|id, _| datasets.iter().any(|dataset| dataset.id == *id && dataset.state.loaded));
        self.disc_spans.retain(|id| datasets.iter().any(|dataset| dataset.id == id && dataset.state.loaded));
        self.warned_over_capacity.retain(|id| datasets.iter().any(|dataset| dataset.id == *id));
        self.reported_build.retain(|id| datasets.iter().any(|dataset| dataset.id == *id));
        if self.entries.len() != retained {
            self.content_key = self.content_key.wrapping_add(1);
        }
        // Computed once, not per dataset: every string-and-discs set in the
        // scene shares the same background and therefore the same string
        // colour, and a true-diameter set never reads it (`dataset_key`
        // leaves it out of that set's hash, so a theme change alone never
        // rebuilds it).
        let string_color = string_color_for(editor.renderer_background_color);
        for dataset in datasets {
            if !dataset.state.loaded {
                continue;
            }
            let selection = HoleSelection::of(dataset, editor);
            let key = dataset_key(dataset, scene_origin, string_color);
            let selection_key = selection.key();
            let action = sync_action(self.entries.get(&dataset.id).map(|cached| (cached.key, cached.selection_key)), key, selection_key);
            if action == SyncAction::Keep {
                continue;
            }
            let base_changed = action == SyncAction::RebuildAll;

            // Built only now the keys above say the bits would differ.
            let bits = selection_bits_for(dataset.dataset.holes.len(), &dataset.dataset.ties, &selection);
            if bits.over_capacity > 0 && self.warned_over_capacity.insert(dataset.id) {
                crate::userspace_warn!(
                    "{}",
                    tr_format!(
                        literal = "Drill hole set %name% has %count% holes and tie-ins, past the %capacity% the selection highlight can carry: selecting the set as a whole still highlights it, selecting single holes will not",
                        name = dataset.name,
                        count = bits.over_capacity,
                        capacity = SELECTION_CAPACITY,
                    )
                );
            }

            let (buffer, count, cells, disc_cells_from, tie_buffer, tie_count, collar_buffer, collar_count, selection_buffer, selection_bind_group) =
                match self.entries.remove(&dataset.id) {
                    Some(cached) if !base_changed => {
                        // Only the preview, never inserted here, has no buffer.
                        let selection_buffer = cached.selection_buffer.unwrap();
                        queue.write_buffer(&selection_buffer, 0, &bits.live_bytes());
                        (
                            cached.buffer,
                            cached.count,
                            cached.cells,
                            cached.disc_cells_from,
                            cached.tie_buffer,
                            cached.tie_count,
                            cached.collar_buffer,
                            cached.collar_count,
                            selection_buffer,
                            cached.selection_bind_group,
                        )
                    }
                    _ => {
                        let built = build_segment_instances(dataset, scene_origin, string_color);
                        match dataset.color.hole_style {
                            DrillHoleStyle::StringAndDiscs => self.disc_spans.insert(dataset, built.disc_spans),
                            DrillHoleStyle::TrueDiameter => self.disc_spans.remove(dataset.id),
                        }
                        let buffer = (!built.instances.is_empty()).then(|| {
                            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Drillhole Segment Instances"),
                                contents: bytemuck::cast_slice(&built.instances),
                                usage: wgpu::BufferUsages::VERTEX,
                            })
                        });
                        let count = built.instances.len().min(u32::MAX as usize) as u32;
                        let cells = built.cells;
                        let disc_cells_from = built.disc_cells_from;
                        if self.reported_build.insert(dataset.id) {
                            crate::userspace_log!(
                                "{}",
                                tr_format!(
                                    literal = "Drill hole set %name%: %stations% stations, %before% segments merged to %after%, %cells% cells",
                                    name = dataset.name,
                                    stations = built.stations,
                                    before = built.before_merge,
                                    after = built.instances.len(),
                                    cells = cells.len(),
                                )
                            );
                        }

                        let ties = build_tie_instances(dataset, scene_origin);
                        let tie_buffer = (!ties.is_empty()).then(|| {
                            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Drillhole Tie-In Instances"),
                                contents: bytemuck::cast_slice(&ties),
                                usage: wgpu::BufferUsages::VERTEX,
                            })
                        });
                        let tie_count = ties.len().min(u32::MAX as usize) as u32;

                        let collars = build_collar_instances(dataset, scene_origin);
                        let collar_buffer = (!collars.is_empty()).then(|| {
                            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Drillhole Collar Instances"),
                                contents: bytemuck::cast_slice(&collars),
                                usage: wgpu::BufferUsages::VERTEX,
                            })
                        });
                        let collar_count = collars.len().min(u32::MAX as usize) as u32;

                        let (selection_buffer, selection_bind_group) = create_selection_buffer_and_bind_group(device, queue, &self.selection_layout, &bits);

                        (
                            buffer,
                            count,
                            cells,
                            disc_cells_from,
                            tie_buffer,
                            tie_count,
                            collar_buffer,
                            collar_count,
                            selection_buffer,
                            selection_bind_group,
                        )
                    }
                };

            self.content_key = self.content_key.wrapping_add(1);
            self.entries.insert(
                dataset.id,
                CachedDrillHoles {
                    buffer,
                    count,
                    cells,
                    disc_cells_from,
                    tie_buffer,
                    tie_count,
                    collar_buffer,
                    collar_count,
                    selection_buffer: Some(selection_buffer),
                    selection_bind_group,
                    key,
                    selection_key,
                },
            );
        }
        self.sync_pattern_preview(device, scene_origin, editor);
    }

    fn sync_pattern_preview(&mut self, device: &wgpu::Device, scene_origin: DVec3, editor: &crate::ui::state::EditorState) {
        if !editor.drill_pattern_open
            || editor.drill_pattern_preview_collars.is_empty()
            || !editor.drill_pattern_preview_depth.is_finite()
            || editor.drill_pattern_preview_depth <= 0.0
            || !editor.drill_pattern_preview_diameter.is_finite()
            || editor.drill_pattern_preview_diameter <= 0.0
        {
            self.content_key = self.content_key.wrapping_add(u64::from(self.preview.take().is_some()));
            return;
        }

        let mut hash = DefaultHasher::new();
        editor.drill_pattern_preview_depth.to_bits().hash(&mut hash);
        editor.drill_pattern_preview_diameter.to_bits().hash(&mut hash);
        for value in scene_origin.to_array() {
            value.to_bits().hash(&mut hash);
        }
        for collar in &editor.drill_pattern_preview_collars {
            for value in collar.to_array() {
                value.to_bits().hash(&mut hash);
            }
        }
        let key = hash.finish();
        if self.preview.as_ref().is_some_and(|cached| cached.key == key) {
            return;
        }

        let preview_radius = editor.drill_pattern_preview_diameter * 0.5;
        let instances: Vec<_> = editor
            .drill_pattern_preview_collars
            .iter()
            .map(|&collar| DrillSegmentInstance {
                start: (collar - scene_origin).as_vec3().to_array(),
                radius: preview_radius as f32,
                end: (collar - DVec3::Z * editor.drill_pattern_preview_depth - scene_origin).as_vec3().to_array(),
                pixel_diameter: MIN_RENDER_PIXEL_DIAMETER,
                color: [1.0; 3],
                selection_index: 0,
                pixel_length: 0.0,
                depth_rank: 0.0,
                disc_start: [0.0; 3],
                disc_end: [0.0; 3],
            })
            .collect();
        let collars: Vec<_> = editor
            .drill_pattern_preview_collars
            .iter()
            .map(|&collar| DrillCollarInstance {
                center: (collar - scene_origin).as_vec3().to_array(),
                marker_radius: (preview_radius * COLLAR_MARKER_RADIUS_SCALE) as f32,
                outline: COLLAR_MARKER_OUTLINE_COLOR,
                pixel_diameter: COLLAR_MARKER_MIN_PIXEL_DIAMETER,
                fill: COLLAR_MARKER_FILL_COLOR,
                hole_radius: preview_radius as f32,
                selection_index: 0,
                trace_pixel_diameter: MIN_RENDER_PIXEL_DIAMETER,
            })
            .collect();
        let buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Drill Pattern Preview Segment Instances"),
            contents: bytemuck::cast_slice(&instances),
            usage: wgpu::BufferUsages::VERTEX,
        }));
        let collar_buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Drill Pattern Preview Collar Instances"),
            contents: bytemuck::cast_slice(&collars),
            usage: wgpu::BufferUsages::VERTEX,
        }));

        let selection_bind_group = self.empty_selection_bind_group.clone();

        self.content_key = self.content_key.wrapping_add(1);
        self.preview = Some(CachedDrillHoles {
            buffer,
            count: instances.len().min(u32::MAX as usize) as u32,
            cells: Vec::new(),
            disc_cells_from: 0,
            tie_buffer: None,
            tie_count: 0,
            collar_buffer,
            collar_count: collars.len().min(u32::MAX as usize) as u32,
            selection_buffer: None,
            selection_bind_group,
            key,
            selection_key: 0,
        });
    }

    /// What the cached instances currently are, for a scene cache that has to
    /// know when they stop being what it drew. See [`Self::content_key`]'s
    /// field.
    pub(crate) fn content_key(&self) -> u64 {
        self.content_key
    }
    pub(crate) fn get(&self, id: DrillHoleId) -> Option<&CachedDrillHoles> {
        self.entries.get(&id)
    }
    pub(crate) fn preview(&self) -> Option<&CachedDrillHoles> {
        self.preview.as_ref()
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.preview.is_none() && self.entries.values().all(|entry| entry.count == 0 && entry.tie_count == 0 && entry.collar_count == 0)
    }
}

/// Which of a dataset's holes are drawn as selected.
///
/// The explorer selects a dataset whole; a canvas click selects holes one at
/// a time - see [`crate::ui::state::EditorState::selected_drill_holes`] - so
/// this carries both and [`selection_bits_for`] turns it into the bitset
/// the shaders read.
struct HoleSelection {
    /// The whole dataset is selected: every hole in it is.
    whole: bool,
    /// Indices of the individually selected holes, ascending, so the cache
    /// key below hashes the same set the same way every frame.
    holes: Vec<usize>,
    /// Canonical hole pairs of selected tie-ins in this dataset.
    ties: Vec<(usize, usize)>,
}

impl HoleSelection {
    fn of(dataset: &OpenDrillHoleDataset, editor: &crate::ui::state::EditorState) -> Self {
        let mut holes: Vec<usize> = editor.selected_drill_holes.iter().filter(|hole| hole.dataset == dataset.id).map(|hole| hole.hole).collect();
        holes.sort_unstable();
        let mut ties: Vec<_> = editor.selected_tie_ins.iter().filter(|tie| tie.dataset == dataset.id).map(|tie| (tie.a, tie.b)).collect();
        ties.sort_unstable();
        Self {
            whole: editor.selected_handles.contains(&dataset.entity_id()),
            holes,
            ties,
        }
    }

    fn contains_tie(&self, from: usize, to: usize) -> bool {
        let pair = if from <= to { (from, to) } else { (to, from) };
        self.ties.binary_search(&pair).is_ok()
    }

    /// Cheap stand-in for hashing the built bitset: everything else those
    /// bits depend on is constant or already inside `dataset_key`.
    fn key(&self) -> u64 {
        let mut hash = DefaultHasher::new();
        self.whole.hash(&mut hash);
        self.holes.hash(&mut hash);
        self.ties.hash(&mut hash);
        hash.finish()
    }
}

/// The neutral grey a string is drawn in, chosen for contrast against the
/// scene background the same way the viewport picks its overlay ink - see
/// `contrast_ink` in `ui/widgets/viewport.rs`, whose 0.45 threshold this
/// matches. The scene background is `EditorState::renderer_background_color`,
/// a preference independent of the UI's own dark/light toggle.
fn string_color_for(background: [f32; 4]) -> [f32; 3] {
    let rgba = if crate::rendering::color::relative_luminance(background) > 0.45 {
        crate::rendering::color::hex_to_linear_rgba(0x383838)
    } else {
        crate::rendering::color::hex_to_linear_rgba(0xd0d0d0)
    };
    [rgba[0], rgba[1], rgba[2]]
}

fn dataset_key(dataset: &OpenDrillHoleDataset, scene_origin: DVec3, string_color: [f32; 3]) -> u64 {
    let mut hash = DefaultHasher::new();
    dataset.id.hash(&mut hash);
    dataset.state.loaded.hash(&mut hash);
    // Hole positions are not hashed one by one: an edit to the geometry - the
    // Move Collar tool is the only one so far - bumps the item's revision, and
    // that is what tells the cache the instances it built are stale.
    dataset.state.revision().hash(&mut hash);
    for value in scene_origin.to_array() {
        value.to_bits().hash(&mut hash);
    }
    dataset.color.active_field.hash(&mut hash);
    (dataset.color.preset as u8).hash(&mut hash);
    dataset.color.smooth.hash(&mut hash);
    dataset.color.radius_scale.to_bits().hash(&mut hash);
    dataset.color.min_pixel_diameter.to_bits().hash(&mut hash);
    for stop in &dataset.color.stops {
        stop.t.to_bits().hash(&mut hash);
        for value in stop.color {
            value.to_bits().hash(&mut hash);
        }
    }
    dataset.color.categories.content_hash().hash(&mut hash);
    dataset.color.working_sections.hash(&mut hash);
    dataset.color.by_working_section.hash(&mut hash);
    dataset.color.hole_style.hash(&mut hash);
    // Diameter, string width and string colour only matter to a
    // string-and-discs build: hashing them for a true-diameter set would
    // rebuild it on every theme change for no visible difference.
    if dataset.color.hole_style == DrillHoleStyle::StringAndDiscs {
        dataset.color.disc_diameter.to_bits().hash(&mut hash);
        dataset.color.string_pixel_width.to_bits().hash(&mut hash);
        for value in string_color {
            value.to_bits().hash(&mut hash);
        }
    }
    hash.finish()
}

/// What `sync` logs once per full segment rebuild.
struct SegmentBuild {
    instances: Vec<DrillSegmentInstance>,
    cells: Vec<DrillCell>,
    /// Index into `cells` where disc cells begin - see
    /// [`CachedDrillHoles::disc_cells_from`]. `cells.len()` for a
    /// true-diameter build, which has no discs.
    disc_cells_from: usize,
    /// Per hole, indexed like `dataset.dataset.holes`: the discs
    /// [`build_string_and_disc_instances`] drew for it, for
    /// [`DiscSpans`]. Empty for a true-diameter build, which draws none.
    disc_spans: Vec<Vec<HoleDisc>>,
    stations: usize,
    before_merge: usize,
}

/// An emitted segment before merging, in f64 world positions so the merge
/// tests run before the f32 cast the instance takes.
#[derive(Clone, Copy)]
struct RawSegment {
    start: DVec3,
    end: DVec3,
    radius: f32,
    color: [f32; 3],
    selection_index: u32,
    /// Carried straight to [`DrillSegmentInstance::depth_rank`]; zero for a
    /// true-diameter segment or a string, set per disc in
    /// [`build_string_and_disc_instances`].
    depth_rank: f32,
}

/// Largest direction change, in degrees, between a run's chord and a
/// segment joining it: the cheap first gate that rejects a dogleg before
/// the offset arithmetic; [`MERGE_MAX_OFFSET`] is what bounds the drift.
const MERGE_MAX_DEVIATION_DEGREES: f64 = 1.0;

/// Largest perpendicular distance, in metres, a swallowed station may sit
/// from its run's chord: below the zoom at which a hole reads wider than a
/// pixel, above downhole survey scatter. An angle alone cannot say this:
/// one degree over three hundred metres is a metre off the trace, a wrong
/// line for a mine design tool, not a nearly straight one.
const MERGE_MAX_OFFSET: f64 = 0.1;

/// Distance from `station` to the line through `start` along unit `direction`.
fn offset_from_chord(station: DVec3, start: DVec3, direction: DVec3) -> f64 {
    (station - start).cross(direction).length()
}

/// Most stations one run may swallow; every extension rechecks them all, so
/// this cap keeps the merge one pass over the hole.
const MERGE_MAX_STATIONS: usize = 64;

/// Merges consecutive same-hole segments that share a colour, start where the
/// last ended, bend under [`MERGE_MAX_DEVIATION_DEGREES`] from the run's chord
/// and leave every swallowed station within [`MERGE_MAX_OFFSET`] of it; any
/// break starts a new run. `cos_threshold` is the angle's cosine, taken once.
/// The offset is rechecked exactly per extension: a carried bound must assume
/// the worst of each move and splits runs on survey scatter alone.
fn merge_raw_segments(segments: Vec<RawSegment>, cos_threshold: f64) -> Vec<RawSegment> {
    let mut merged: Vec<RawSegment> = Vec::with_capacity(segments.len());
    let mut swallowed: Vec<DVec3> = Vec::new();
    for segment in segments {
        if let Some(run) = merged.last_mut()
            && run.color == segment.color
            && run.end == segment.start
            && swallowed.len() < MERGE_MAX_STATIONS
        {
            let run_direction = run.end - run.start;
            let segment_direction = segment.end - run.end;
            let run_length = run_direction.length();
            let segment_length = segment_direction.length();
            let within_angle = run_length > 0.0 && segment_length > 0.0 && run_direction.dot(segment_direction) / (run_length * segment_length) >= cos_threshold;
            let chord = segment.end - run.start;
            let chord_length = chord.length();
            if within_angle && chord_length > 0.0 {
                let direction = chord / chord_length;
                // The newly swallowed station is the likeliest to fail.
                let stations_near = offset_from_chord(run.end, run.start, direction) <= MERGE_MAX_OFFSET
                    && swallowed.iter().all(|&station| offset_from_chord(station, run.start, direction) <= MERGE_MAX_OFFSET);
                if stations_near {
                    swallowed.push(run.end);
                    run.end = segment.end;
                    continue;
                }
            }
        }
        swallowed.clear();
        merged.push(segment);
    }
    merged
}

fn build_segment_instances(dataset: &OpenDrillHoleDataset, scene_origin: DVec3, string_color: [f32; 3]) -> SegmentBuild {
    if !dataset.state.loaded {
        return SegmentBuild {
            instances: Vec::new(),
            cells: Vec::new(),
            disc_cells_from: 0,
            disc_spans: Vec::new(),
            stations: 0,
            before_merge: 0,
        };
    }
    match dataset.color.hole_style {
        DrillHoleStyle::TrueDiameter => build_true_diameter_instances(dataset, scene_origin),
        DrillHoleStyle::StringAndDiscs => build_string_and_disc_instances(dataset, scene_origin, string_color),
    }
}

/// The true-diameter look: one cylinder per merged run at the hole's
/// drilled diameter, cut wherever the active field's value changes.
fn build_true_diameter_instances(dataset: &OpenDrillHoleDataset, scene_origin: DVec3) -> SegmentBuild {
    let mut instances = Vec::new();
    let mut stations = 0usize;
    let mut before_merge = 0usize;
    let cos_threshold = MERGE_MAX_DEVIATION_DEGREES.to_radians().cos();
    let field = dataset.color.active_field.as_deref().and_then(|key| dataset.dataset.field(key));
    let sections = dataset.color.section_lookup();
    for (index, hole) in dataset.dataset.holes.iter().enumerate() {
        stations += hole.trace.len();
        if hole.trace.len() < 2 {
            continue;
        }
        let min_depth = hole.trace.first().unwrap().depth;
        let max_depth = hole.trace.last().unwrap().depth;
        // Interval boundaries are extra cuts, not extra pieces of their own:
        // `trace_pieces` already cuts at every station and render-range end.
        let mut cuts = Vec::new();
        if let Some(field) = field {
            for interval in &hole.intervals {
                if interval.values.contains_key(&field.key) {
                    cuts.push(interval.from.clamp(min_depth, max_depth));
                    cuts.push(interval.to.clamp(min_depth, max_depth));
                }
            }
        }
        let pieces = hole.trace_pieces(min_depth, max_depth, &cuts);
        if pieces.is_empty() {
            continue;
        }

        // Only the coloured path reads this sweep.
        let interval_order = field.is_some().then(|| {
            let mut order: Vec<usize> = (0..hole.intervals.len()).collect();
            order.sort_by(|&a, &b| hole.intervals[a].from.total_cmp(&hole.intervals[b].from));
            order
        });
        let mut cursor = 0usize;
        let mut raw_segments: Vec<RawSegment> = Vec::with_capacity(pieces.len());

        for piece in &pieces {
            let midpoint = (piece.from + piece.to) * 0.5;
            let value = field.and_then(|field| {
                let order = interval_order.as_ref()?;
                while cursor < order.len() && hole.intervals[order[cursor]].to <= midpoint {
                    cursor += 1;
                }
                // The sweep is ordered by `from`, so picking the first match
                // in that order would let the sort decide the winner when two
                // intervals both cover this midpoint. Import order is the
                // answer, so take the smallest original index among the
                // candidates, not the smallest `from`; do not simplify this
                // back to `.find()`.
                order[cursor..]
                    .iter()
                    .take_while(|&&i| hole.intervals[i].from <= midpoint)
                    .filter(|&&i| midpoint < hole.intervals[i].to && hole.intervals[i].values.contains_key(&field.key))
                    .min_by_key(|&&i| i)
                    .and_then(|&i| hole.intervals[i].values.get(&field.key))
            });
            raw_segments.push(RawSegment {
                start: piece.start,
                end: piece.end,
                radius: hole.diameter.map_or(0.0, |diameter| (diameter * 0.5 * dataset.color.radius_scale) as f32),
                color: field
                    .and_then(|field| value.map(|value| evaluate_color_with(&field.kind, value, &dataset.color, &sections)))
                    .unwrap_or([1.0; 3]),
                selection_index: index as u32,
                depth_rank: 0.0,
            });
        }

        before_merge += raw_segments.len();
        instances.extend(merge_raw_segments(raw_segments, cos_threshold).into_iter().map(|segment| DrillSegmentInstance {
            start: (segment.start - scene_origin).as_vec3().to_array(),
            radius: segment.radius,
            end: (segment.end - scene_origin).as_vec3().to_array(),
            pixel_diameter: dataset.color.min_pixel_diameter,
            color: segment.color,
            selection_index: segment.selection_index,
            pixel_length: 0.0,
            depth_rank: 0.0,
            disc_start: [0.0; 3],
            disc_end: [0.0; 3],
        }));
    }
    let (instances, cells) = bucket_instances(instances);
    let disc_cells_from = cells.len();
    SegmentBuild {
        instances,
        cells,
        disc_cells_from,
        disc_spans: Vec::new(),
        stations,
        before_merge,
    }
}

/// The string-and-discs look: a thin constant-width string along the trace,
/// coloured by [`string_color_for`], and a disc on every interval carrying a
/// value in the active field - see [`hole_discs`] for the overlap rule that
/// keeps discs from ever fighting one another for the same depth.
///
/// Strings and discs are bucketed separately and concatenated string-first so
/// the draw (`passes::draw_drill_holes`) can emit every string before any
/// disc: with no depth test on the x-ray pipeline, draw order is the only
/// thing standing between a disc and the string underneath it.
fn build_string_and_disc_instances(dataset: &OpenDrillHoleDataset, scene_origin: DVec3, string_color: [f32; 3]) -> SegmentBuild {
    let mut strings = Vec::new();
    let mut discs = Vec::new();
    let mut stations = 0usize;
    let mut before_merge = 0usize;
    let cos_threshold = MERGE_MAX_DEVIATION_DEGREES.to_radians().cos();

    let field = dataset.color.active_field.as_deref().and_then(|key| dataset.dataset.field(key));
    let sections = dataset.color.section_lookup();
    let disc_radius = (dataset.color.disc_diameter * 0.5) as f32;
    let disc_pixel_diameter = dataset.color.disc_min_pixel_diameter();

    let mut disc_spans: Vec<Vec<HoleDisc>> = vec![Vec::new(); dataset.dataset.holes.len()];

    for (index, hole) in dataset.dataset.holes.iter().enumerate() {
        stations += hole.trace.len();

        if hole.trace.len() >= 2 {
            let min_depth = hole.trace.first().unwrap().depth;
            let max_depth = hole.trace.last().unwrap().depth;
            let pieces = hole.trace_pieces(min_depth, max_depth, &[]);
            let raw_segments: Vec<RawSegment> = pieces
                .iter()
                .map(|piece| RawSegment {
                    start: piece.start,
                    end: piece.end,
                    radius: 0.0,
                    color: string_color,
                    selection_index: index as u32,
                    depth_rank: 0.0,
                })
                .collect();
            before_merge += raw_segments.len();
            strings.extend(merge_raw_segments(raw_segments, cos_threshold).into_iter().map(|segment| DrillSegmentInstance {
                start: (segment.start - scene_origin).as_vec3().to_array(),
                radius: 0.0,
                end: (segment.end - scene_origin).as_vec3().to_array(),
                pixel_diameter: dataset.color.string_pixel_width,
                color: segment.color,
                selection_index: segment.selection_index,
                pixel_length: 0.0,
                depth_rank: 0.0,
                disc_start: [0.0; 3],
                disc_end: [0.0; 3],
            }));
        }

        let Some(field) = field else { continue };
        disc_spans[index] = hole_discs(hole, &field.key);
        let discs_for_hole = &disc_spans[index];
        let disc_count = discs_for_hole.len();
        if disc_count == 0 {
            continue;
        }
        // Longest first so the k-th (0-based) disc's rank is (k+1)/n: the
        // thinnest disc, last in this order, lands on 1.0.
        let mut rank_order: Vec<usize> = (0..disc_count).collect();
        rank_order.sort_by(|&a, &b| {
            let length_a = discs_for_hole[a].to - discs_for_hole[a].from;
            let length_b = discs_for_hole[b].to - discs_for_hole[b].from;
            length_b.total_cmp(&length_a).then(discs_for_hole[a].from.total_cmp(&discs_for_hole[b].from))
        });
        let mut depth_ranks = vec![0.0f32; disc_count];
        for (order, &disc_index) in rank_order.iter().enumerate() {
            depth_ranks[disc_index] = (order + 1) as f32 / disc_count as f32;
        }

        for (disc_index, disc) in discs_for_hole.iter().enumerate() {
            let Some(value) = hole.intervals[disc.interval].values.get(&field.key) else {
                continue;
            };
            let color = evaluate_color_with(&field.kind, value, &dataset.color, &sections);
            // The whole disc's span, shared by every piece it is split into
            // at a station: the shader lengthens a thin disc about this
            // span's midpoint, not one piece's.
            let (Some(disc_start_pos), Some(disc_end_pos)) = (hole.position_at_depth(disc.from), hole.position_at_depth(disc.to)) else {
                continue;
            };
            let disc_start = (disc_start_pos - scene_origin).as_vec3().to_array();
            let disc_end = (disc_end_pos - scene_origin).as_vec3().to_array();

            let pieces = hole.trace_pieces(disc.from, disc.to, &[]);
            let raw_segments: Vec<RawSegment> = pieces
                .iter()
                .map(|piece| RawSegment {
                    start: piece.start,
                    end: piece.end,
                    radius: disc_radius,
                    color,
                    selection_index: index as u32,
                    depth_rank: depth_ranks[disc_index],
                })
                .collect();
            before_merge += raw_segments.len();
            // Merged on its own, never joined with a neighbouring disc's
            // segments: each disc must keep its own extent and rank.
            discs.extend(merge_raw_segments(raw_segments, cos_threshold).into_iter().map(|segment| DrillSegmentInstance {
                start: (segment.start - scene_origin).as_vec3().to_array(),
                radius: segment.radius,
                end: (segment.end - scene_origin).as_vec3().to_array(),
                pixel_diameter: disc_pixel_diameter,
                color: segment.color,
                selection_index: segment.selection_index,
                pixel_length: DISC_MIN_PIXEL_LENGTH,
                depth_rank: segment.depth_rank,
                disc_start,
                disc_end,
            }));
        }
    }

    // Ascending so a spatial cell's members, which keep their relative order
    // through bucketing, end with the thinnest disc: the one x-ray draw order
    // should put on top.
    discs.sort_by(|a, b| a.depth_rank.total_cmp(&b.depth_rank));

    let (strings, string_cells) = bucket_instances(strings);
    let (discs, disc_cells) = bucket_instances(discs);
    let string_count = strings.len() as u32;
    let mut cells = string_cells;
    let disc_cells_from = cells.len();
    cells.extend(disc_cells.into_iter().map(|cell| DrillCell {
        min: cell.min,
        max: cell.max,
        start: cell.start + string_count,
        end: cell.end + string_count,
    }));
    let mut instances = strings;
    instances.extend(discs);

    SegmentBuild {
        instances,
        cells,
        disc_cells_from,
        disc_spans,
        stations,
        before_merge,
    }
}

/// Instances one cell should hold on average when sizing the grid.
const TARGET_INSTANCES_PER_CELL: usize = 512;
/// Cap on cell count so an odd aspect ratio cannot bloat the grid.
const MAX_CELLS: usize = 4096;

/// The scene-relative box an instance occupies, padded by its radius.
fn instance_box(instance: &DrillSegmentInstance) -> (glam::Vec3, glam::Vec3) {
    let start = glam::Vec3::from_array(instance.start);
    let end = glam::Vec3::from_array(instance.end);
    let radius = glam::Vec3::splat(instance.radius);
    (start.min(end) - radius, start.max(end) + radius)
}

/// Buckets merged instances into a uniform grid over their bounds and
/// reorders `instances` so each cell owns one contiguous range; cells come
/// out sorted by `start` and cover the whole Vec, a permutation of the input.
fn bucket_instances(instances: Vec<DrillSegmentInstance>) -> (Vec<DrillSegmentInstance>, Vec<DrillCell>) {
    if instances.is_empty() {
        return (instances, Vec::new());
    }

    let mut bounds_min = glam::Vec3::splat(f32::INFINITY);
    let mut bounds_max = glam::Vec3::splat(f32::NEG_INFINITY);
    for instance in &instances {
        let (min, max) = instance_box(instance);
        bounds_min = bounds_min.min(min);
        bounds_max = bounds_max.max(max);
    }
    // A single hole or a flat dataset has a zero extent on some axis.
    let extent = (bounds_max - bounds_min).max(glam::Vec3::splat(1.0e-6));

    let target = (instances.len() / TARGET_INSTANCES_PER_CELL).clamp(1, MAX_CELLS);
    let volume = extent.x as f64 * extent.y as f64 * extent.z as f64;
    let mut side = (volume / target as f64).cbrt().max(1.0e-6);
    let (divisions_x, divisions_y, divisions_z) = loop {
        let x = ((extent.x as f64 / side).ceil() as usize).max(1);
        let y = ((extent.y as f64 / side).ceil() as usize).max(1);
        let z = ((extent.z as f64 / side).ceil() as usize).max(1);
        if x.saturating_mul(y).saturating_mul(z) <= MAX_CELLS {
            break (x, y, z);
        }
        side *= 1.3;
    };

    let cell_of = |midpoint: glam::Vec3| -> (usize, usize, usize) {
        let relative = (midpoint - bounds_min) / extent;
        (
            ((relative.x * divisions_x as f32) as usize).min(divisions_x - 1),
            ((relative.y * divisions_y as f32) as usize).min(divisions_y - 1),
            ((relative.z * divisions_z as f32) as usize).min(divisions_z - 1),
        )
    };

    let mut buckets: HashMap<(usize, usize, usize), Vec<usize>> = HashMap::new();
    for (index, instance) in instances.iter().enumerate() {
        let start = glam::Vec3::from_array(instance.start);
        let end = glam::Vec3::from_array(instance.end);
        buckets.entry(cell_of((start + end) * 0.5)).or_default().push(index);
    }

    let mut keys: Vec<(usize, usize, usize)> = buckets.keys().copied().collect();
    keys.sort_unstable();

    let mut reordered = Vec::with_capacity(instances.len());
    let mut cells = Vec::with_capacity(keys.len());
    for key in keys {
        let start = reordered.len() as u32;
        let mut cell_min = glam::Vec3::splat(f32::INFINITY);
        let mut cell_max = glam::Vec3::splat(f32::NEG_INFINITY);
        for &member in &buckets[&key] {
            let (min, max) = instance_box(&instances[member]);
            cell_min = cell_min.min(min);
            cell_max = cell_max.max(max);
            reordered.push(instances[member]);
        }
        cells.push(DrillCell {
            min: cell_min,
            max: cell_max,
            start,
            end: reordered.len() as u32,
        });
    }
    (reordered, cells)
}

/// The surface connectors, drawn collar to collar through the same instanced
/// cylinder the traces use. A tie has no thickness of its own, so it takes a
/// world radius from the holes it joins and scales with them until reaching
/// the shared two-pixel screen floor. `selection_index` stamps the tie's own
/// slot in the dataset's selection bitset - `hole_count + i`, where `i` is
/// this tie's position in `dataset.dataset.ties` - matching
/// [`selection_bits_for`] bit for bit.
fn build_tie_instances(dataset: &OpenDrillHoleDataset, scene_origin: DVec3) -> Vec<DrillSegmentInstance> {
    let holes = &dataset.dataset.holes;
    let hole_count = holes.len() as u32;
    dataset
        .dataset
        .ties
        .iter()
        .enumerate()
        .filter_map(|(tie_index, tie)| {
            let from = holes.get(tie.from)?;
            let to = holes.get(tie.to)?;
            let start = from.collar_position();
            let end = to.collar_position();
            (start.distance_squared(end) > 1.0e-18).then_some(DrillSegmentInstance {
                start: (start - scene_origin).as_vec3().to_array(),
                // A run between holes of unequal diameter takes their mean, so
                // it reads the same whichever end it was tied from.
                radius: ((from.render_radius() + to.render_radius()) * 0.5 * TIE_RADIUS_SCALE) as f32,
                end: (end - scene_origin).as_vec3().to_array(),
                pixel_diameter: MIN_RENDER_PIXEL_DIAMETER,
                color: tie.color,
                selection_index: hole_count + tie_index as u32,
                pixel_length: 0.0,
                depth_rank: 0.0,
                disc_start: [0.0; 3],
                disc_end: [0.0; 3],
            })
        })
        .collect()
}

fn build_collar_instances(dataset: &OpenDrillHoleDataset, scene_origin: DVec3) -> Vec<DrillCollarInstance> {
    if !dataset.state.loaded {
        return Vec::new();
    }
    // The marker itself keeps the drilled radius regardless of style; only
    // the lift that holds it clear of what is drawn below it - `hole_radius`
    // and `trace_pixel_diameter` - follows the trace's own width, so it sits
    // proud of a disc the same way it sits proud of a true-diameter cylinder.
    // `SceneQuery::nearest_drill_hole` picks the collar at this same lift.
    // Constant across the dataset for string-and-discs, so it is worked out
    // once rather than matched per hole.
    let disc_lift = matches!(dataset.color.hole_style, DrillHoleStyle::StringAndDiscs).then(|| (dataset.color.disc_diameter * 0.5, dataset.color.disc_min_pixel_diameter()));
    dataset
        .dataset
        .holes
        .iter()
        .enumerate()
        .map(|(index, hole)| {
            let center = hole.collar_position();
            let hole_radius = hole.render_radius();
            let (radius, pixel_diameter) = disc_lift.unwrap_or((hole_radius * dataset.color.radius_scale, dataset.color.min_pixel_diameter));
            DrillCollarInstance {
                center: (center - scene_origin).as_vec3().to_array(),
                marker_radius: (hole_radius * COLLAR_MARKER_RADIUS_SCALE) as f32,
                outline: COLLAR_MARKER_OUTLINE_COLOR,
                pixel_diameter: COLLAR_MARKER_MIN_PIXEL_DIAMETER,
                fill: COLLAR_MARKER_FILL_COLOR,
                hole_radius: radius as f32,
                selection_index: index as u32,
                trace_pixel_diameter: pixel_diameter,
            }
        })
        .collect()
}

/// The colour the 3D view gives one interval value. Shared with the borehole
/// log so a hole reads the same in the panel as it does in the scene. The
/// kind is borrowed: a categorical kind owns its whole code list.
pub(crate) fn evaluate_color_for(kind: &DrillFieldKind, value: &DrillValue, state: &DrillColorState) -> [f32; 3] {
    match (kind, value) {
        (DrillFieldKind::Categorical { .. }, DrillValue::Category(value)) => state.category_color(state.display_code(value)).unwrap_or([1.0; 3]),
        _ => evaluate_ramp(kind, value, state),
    }
}

/// [`evaluate_color_for`] over a whole rebuild: the codes go through a
/// section lookup built once, not a walk of the section list per interval.
pub(crate) fn evaluate_color_with(kind: &DrillFieldKind, value: &DrillValue, state: &DrillColorState, sections: &crate::model::drill_hole::SectionLookup<'_>) -> [f32; 3] {
    match (kind, value) {
        (DrillFieldKind::Categorical { .. }, DrillValue::Category(value)) => state.category_color(sections.display_code(value)).unwrap_or([1.0; 3]),
        _ => evaluate_ramp(kind, value, state),
    }
}

/// The numeric half of the two above; anything not numeric is white.
fn evaluate_ramp(kind: &DrillFieldKind, value: &DrillValue, state: &DrillColorState) -> [f32; 3] {
    match (kind, value) {
        // A no-data sentinel falls through to white, the same as a missing
        // value: it is not the bottom of the ramp, it is nothing at all.
        (DrillFieldKind::Numeric { min, max }, DrillValue::Numeric(value)) if value.is_finite() && !crate::model::block_model::is_no_data_sentinel(*value) => {
            let t = if (max - min).abs() <= f64::EPSILON {
                0.5
            } else {
                ((*value - min) / (max - min)).clamp(0.0, 1.0) as f32
            };
            evaluate_stops(t, &state.stops, state.smooth)
        }
        _ => [1.0; 3],
    }
}

pub(crate) fn evaluate_stops(t: f32, stops: &[crate::model::drill_hole::DrillColorStop], smooth: bool) -> [f32; 3] {
    let Some(first) = stops.first() else {
        return [1.0; 3];
    };
    if !smooth {
        return stops.iter().rev().find(|stop| t >= stop.t).unwrap_or(first).color;
    }
    if t <= first.t {
        return first.color;
    }
    for pair in stops.windows(2) {
        if t <= pair[1].t {
            let span = (pair[1].t - pair[0].t).max(f32::EPSILON);
            let f = ((t - pair[0].t) / span).clamp(0.0, 1.0);
            return [
                pair[0].color[0] + (pair[1].color[0] - pair[0].color[0]) * f,
                pair[0].color[1] + (pair[1].color[1] - pair[0].color[1]) * f,
                pair[0].color[2] + (pair[1].color[2] - pair[0].color[2]) * f,
            ];
        }
    }
    stops.last().map_or([1.0; 3], |stop| stop.color)
}
