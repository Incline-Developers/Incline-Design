//! The water a rain simulation's tab draws over its ground.
//!
//! The mesh is the ground's own wet triangles, each vertex carrying the
//! water's depth and velocity at the two kept instants the timeline lies
//! between; `shaders/water.wgsl` blends the two, lifts the vertex to the
//! water's level and shades it: still pools glassy, running water faintly
//! streaked along its way, white water only in a deep torrent, the ground
//! showing through the pale blue shallows, the sky on the pools, rings while it
//! rains.
//!
//! A dry corner of a wet triangle is given the water's level too, below the
//! ground there, as a depth under zero: the surface carries on level and the
//! shore falls where it meets the ground, inside the triangle, rather than
//! along the triangle's sides with the surface drooping down to them. Built app-side when the instants change (`app/water.rs`), uploaded
//! here once per change, and drawn by the editor overlay pass. The ripples
//! move only while the run's timeline plays.

use bytemuck::{Pod, Zeroable};
use glam::DVec3;

use crate::model::water::Ground;

/// The shallowest water drawn, in metres: a film. `water.wgsl` fades the
/// surface in from here.
const FILM_DEPTH: f32 = 0.003;
/// How far the drawn surface stands off the ground under it, in metres, so a
/// film is not lost in it.
const LIFT: f32 = 0.02;
/// The most a chunk holds, to stay inside any device's buffer limit.
const CHUNK_VERTICES: usize = 1 << 21;
const CHUNK_FACES: usize = 1 << 22;
/// The ripple clock starts again after this long, to keep it exact in `f32`.
const CLOCK_WRAP_S: f64 = 3600.0;

/// One vertex of the wet ground, at two instants.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct WaterVertex {
    /// The ground, relative to the mesh's anchor.
    position: [f32; 3],
    /// The water's depth at the earlier instant and the later: under zero on
    /// a dry corner of a wet triangle, by how far the water's level there
    /// lies below the ground.
    depth: [f32; 2],
    /// Its velocity in plan at each, metres a second.
    flow_from: [f32; 2],
    flow_to: [f32; 2],
    /// How far from the earlier instant to the later the water came, 0 to 1:
    /// until then the vertex stays as it was at the earlier.
    arrival: f32,
    /// The water surface's normal at each instant: see [`surface_normals`].
    normal_from: [f32; 3],
    normal_to: [f32; 3],
}

pub(crate) const WATER_VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 7] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x2, 3 => Float32x2, 4 => Float32, 5 => Float32x3, 6 => Float32x3];

/// Each vertex's depth, with a dry corner of a wet triangle given the water's
/// level as a depth under zero: the lowest of the wet triangles' levels around
/// it (each the mean over its wet corners, as `flow_velocity` takes it), when
/// that is below the ground. A dry corner the water stands above stays as it
/// was: the run says it is dry.
fn shore_depths(ground: &Ground, depth: &[f32]) -> Vec<f32> {
    use rayon::prelude::*;
    let face_level: Vec<Option<f64>> = ground
        .faces
        .par_iter()
        .map(|face| {
            let wet: Vec<f64> = face
                .iter()
                .map(|&corner| corner as usize)
                .filter(|&vertex| depth[vertex] >= FILM_DEPTH)
                .map(|vertex| ground.vertices[vertex][2] + f64::from(depth[vertex]))
                .collect();
            (!wet.is_empty() && wet.len() < 3).then(|| wet.iter().sum::<f64>() / wet.len() as f64)
        })
        .collect();
    let mut level = vec![f64::INFINITY; ground.vertices.len()];
    for (face, face_level) in ground.faces.iter().zip(&face_level) {
        if let Some(face_level) = *face_level {
            for &corner in face {
                let lowest = &mut level[corner as usize];
                *lowest = lowest.min(face_level);
            }
        }
    }
    depth
        .iter()
        .zip(&level)
        .zip(&ground.vertices)
        .map(|((&deep, &level), vertex)| {
            if deep < FILM_DEPTH && level.is_finite() {
                deep.min((level - vertex[2]) as f32)
            } else {
                deep
            }
        })
        .collect()
}

/// The water surface's normal at each vertex: the area-weighted mean of the
/// wet triangles' around it, the surface standing at the ground plus `shore`.
/// Smooth across the triangles, so running water does not break into facets
/// where its slope turns. Straight up where there is no water.
fn surface_normals(ground: &Ground, shore: &[f32]) -> Vec<[f32; 3]> {
    let mut summed = vec![DVec3::ZERO; ground.vertices.len()];
    for face in &ground.faces {
        if !face.iter().any(|&corner| shore[corner as usize] >= FILM_DEPTH) {
            continue;
        }
        let [a, b, c] = face.map(|corner| {
            let vertex = ground.vertices[corner as usize];
            DVec3::new(vertex[0], vertex[1], vertex[2] + f64::from(shore[corner as usize]))
        });
        // Its length is twice the triangle's area, which is the weight.
        let mut normal = (b - a).cross(c - a);
        if normal.z < 0.0 {
            normal = -normal;
        }
        for &corner in face {
            summed[corner as usize] += normal;
        }
    }
    summed.into_iter().map(|normal| normal.try_normalize().unwrap_or(DVec3::Z).as_vec3().to_array()).collect()
}

struct WaterChunk {
    vertices: Vec<WaterVertex>,
    indices: Vec<u32>,
}

/// The water between two instants of a run, ready to upload.
pub(crate) struct WaterMesh {
    /// Unique to what the mesh was built from; what the GPU copy is kept by.
    stamp: u64,
    /// The world point the vertices are relative to.
    anchor: DVec3,
    chunks: Vec<WaterChunk>,
}

impl WaterMesh {
    /// The water over `ground` between the depths `from` and `to` at its
    /// vertices; no `from` is dry ground, and the same slice for both is one
    /// instant. `arrival` is the later instant's [`WaterFrame::arrival`], or
    /// empty.
    ///
    /// [`WaterFrame::arrival`]: crate::model::water::WaterFrame::arrival
    pub(crate) fn build(stamp: u64, ground: &Ground, from: Option<&[f32]>, to: &[f32], arrival: &[u8], roughness: f64) -> Self {
        let anchor = ground.vertices.first().map_or(DVec3::ZERO, |&vertex| DVec3::from_array(vertex));
        let flow_to = crate::model::water::flow_velocity(ground, to, roughness);
        let flow_from = match from {
            Some(from) if !std::ptr::eq(from, to) => Some(crate::model::water::flow_velocity(ground, from, roughness)),
            _ => None,
        };
        let depth_from = |vertex: usize| from.map_or(0.0, |from| from[vertex]);
        let one_instant = from.is_none_or(|from| std::ptr::eq(from, to));
        let shore_to = shore_depths(ground, to);
        let shore_from = match from {
            Some(from) if !one_instant => Some(shore_depths(ground, from)),
            _ => None,
        };
        let normal_to = surface_normals(ground, &shore_to);
        let normal_from = shore_from.as_ref().map(|shore| surface_normals(ground, shore));
        // A dry start is dry everywhere; one instant is the same at both ends.
        let drawn_from = |vertex: usize| match (&shore_from, from) {
            (Some(shore), _) => (shore[vertex], normal_from.as_ref().map_or([0.0, 0.0, 1.0], |normal| normal[vertex])),
            (None, Some(_)) => (shore_to[vertex], normal_to[vertex]),
            (None, None) => (shore_to[vertex].min(0.0), normal_to[vertex]),
        };
        let velocity_from = |vertex: usize| match (&flow_from, from) {
            (Some(flow), _) => flow[vertex],
            (None, Some(_)) => flow_to[vertex],
            (None, None) => [0.0; 2],
        };

        // Each wet triangle's corners, numbered within the chunk it lands
        // in: a vertex two chunks share is in both.
        const UNUSED: u32 = u32::MAX;
        let mut slot = vec![UNUSED; ground.vertices.len()];
        let mut placed: Vec<u32> = Vec::new();
        let mut chunks = Vec::new();
        let mut chunk = WaterChunk {
            vertices: Vec::new(),
            indices: Vec::new(),
        };
        for face in &ground.faces {
            if !face.iter().any(|&corner| depth_from(corner as usize).max(to[corner as usize]) >= FILM_DEPTH) {
                continue;
            }
            if chunk.vertices.len() + 3 > CHUNK_VERTICES || chunk.indices.len() >= CHUNK_FACES * 3 {
                for vertex in placed.drain(..) {
                    slot[vertex as usize] = UNUSED;
                }
                chunks.push(std::mem::replace(
                    &mut chunk,
                    WaterChunk {
                        vertices: Vec::new(),
                        indices: Vec::new(),
                    },
                ));
            }
            for &corner in face {
                let vertex = corner as usize;
                if slot[vertex] == UNUSED {
                    slot[vertex] = chunk.vertices.len() as u32;
                    placed.push(corner);
                    let (shore_from, normal_from) = drawn_from(vertex);
                    chunk.vertices.push(WaterVertex {
                        position: (DVec3::from_array(ground.vertices[vertex]) - anchor).as_vec3().to_array(),
                        depth: [shore_from, shore_to[vertex]],
                        flow_from: velocity_from(vertex),
                        flow_to: flow_to[vertex],
                        arrival: arrival.get(vertex).map_or(0.0, |part| f32::from(*part) / 255.0),
                        normal_from,
                        normal_to: normal_to[vertex],
                    });
                }
                chunk.indices.push(slot[vertex]);
            }
        }
        if !chunk.indices.is_empty() {
            chunks.push(chunk);
        }
        Self { stamp, anchor, chunks }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WaterUniform {
    /// xyz: the mesh's anchor relative to the scene origin; w: the ripple
    /// clock, seconds.
    anchor: [f32; 4],
    /// x: how far from the earlier instant to the later, 0 to 1; y: 1 while
    /// rain is falling; z: the lift off the ground, metres; w: unused.
    params: [f32; 4],
}

struct GpuChunk {
    vertices: wgpu::Buffer,
    vertex_capacity: usize,
    indices: wgpu::Buffer,
    index_capacity: usize,
    index_count: u32,
}

/// The shown mesh on the GPU.
pub(crate) struct WaterGpu {
    /// Group 1 of the water pipeline.
    pub(crate) layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// Kept as long as water is shown, so playback reuses their buffers.
    chunks: Vec<GpuChunk>,
    /// How many of `chunks` the shown mesh fills.
    drawn: usize,
    shown: Option<(u64, DVec3)>,
}

impl WaterGpu {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Water Uniform"),
            size: size_of::<WaterUniform>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Water Bind Group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        Self {
            layout,
            uniform,
            bind_group,
            chunks: Vec::new(),
            drawn: 0,
            shown: None,
        }
    }

    fn buffer(device: &wgpu::Device, label: &'static str, bytes: usize, usage: wgpu::BufferUsages) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: bytes as wgpu::BufferAddress,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Show `mesh`, or nothing: uploaded when it is not the one already
    /// here, and its buffers let go once there is nothing to show.
    pub(crate) fn show(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, mesh: Option<&WaterMesh>) {
        let Some(mesh) = mesh else {
            self.chunks = Vec::new();
            self.drawn = 0;
            self.shown = None;
            return;
        };
        if self.shown.is_some_and(|(stamp, _)| stamp == mesh.stamp) {
            return;
        }
        self.shown = Some((mesh.stamp, mesh.anchor));
        self.drawn = mesh.chunks.len();
        for (index, chunk) in mesh.chunks.iter().enumerate() {
            if index == self.chunks.len() {
                self.chunks.push(GpuChunk {
                    vertices: Self::buffer(device, "Water Vertices", size_of::<WaterVertex>(), wgpu::BufferUsages::VERTEX),
                    vertex_capacity: 1,
                    indices: Self::buffer(device, "Water Indices", size_of::<u32>(), wgpu::BufferUsages::INDEX),
                    index_capacity: 1,
                    index_count: 0,
                });
            }
            let gpu = &mut self.chunks[index];
            if chunk.vertices.len() > gpu.vertex_capacity {
                gpu.vertex_capacity = chunk.vertices.len().next_power_of_two().min(CHUNK_VERTICES);
                gpu.vertices = Self::buffer(device, "Water Vertices", gpu.vertex_capacity * size_of::<WaterVertex>(), wgpu::BufferUsages::VERTEX);
            }
            if chunk.indices.len() > gpu.index_capacity {
                gpu.index_capacity = chunk.indices.len().next_power_of_two().min(CHUNK_FACES * 3);
                gpu.indices = Self::buffer(device, "Water Indices", gpu.index_capacity * size_of::<u32>(), wgpu::BufferUsages::INDEX);
            }
            queue.write_buffer(&gpu.vertices, 0, bytemuck::cast_slice(&chunk.vertices));
            queue.write_buffer(&gpu.indices, 0, bytemuck::cast_slice(&chunk.indices));
            gpu.index_count = chunk.indices.len() as u32;
        }
    }

    /// Set what moves every frame: where the scene's origin is, the ripple
    /// clock, how far between the mesh's two instants the timeline is, and
    /// whether rain is falling.
    pub(crate) fn set_frame(&self, queue: &wgpu::Queue, scene_origin: DVec3, clock_s: f64, blend: f32, raining: bool) {
        let Some((_, anchor)) = self.shown else {
            return;
        };
        let offset = (anchor - scene_origin).as_vec3();
        let uniform = WaterUniform {
            anchor: [offset.x, offset.y, offset.z, (clock_s % CLOCK_WRAP_S) as f32],
            params: [blend.clamp(0.0, 1.0), f32::from(u8::from(raining)), LIFT, 0.0],
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.drawn == 0
    }

    /// Draws the shown water; the caller has set the pipeline and the camera
    /// bind group.
    pub(crate) fn draw(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        render_pass.set_bind_group(1, &self.bind_group, &[]);
        for chunk in &self.chunks[..self.drawn] {
            render_pass.set_vertex_buffer(0, chunk.vertices.slice(..));
            render_pass.set_index_buffer(chunk.indices.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..chunk.index_count, 0, 0..1);
        }
    }
}
