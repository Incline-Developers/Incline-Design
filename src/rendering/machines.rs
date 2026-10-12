//! The machines Animate stands where they work: low-poly models read from
//! embedded glTF binaries, drawn instanced over the scene.
//!
//! Only what the models use is read: triangle-list primitives with positions,
//! normals and indices, materials' base colour factors, and the node tree's
//! translation, rotation and scale. Anything else in a file is ignored.

use anyhow::{Context, bail, ensure};
use glam::{DVec2, DVec3, Mat4, Quat, Vec3};
use serde::Deserialize;

/// Which model a machine is drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum MachineModel {
    Excavator,
    WheelLoader,
    Dozer,
    DrillRig,
    Mpu,
}

impl MachineModel {
    pub(crate) const ALL: [Self; 5] = [Self::Excavator, Self::WheelLoader, Self::Dozer, Self::DrillRig, Self::Mpu];

    fn glb(self) -> &'static [u8] {
        match self {
            Self::Excavator => include_bytes!("../../res/machines/excavator.glb"),
            Self::WheelLoader => include_bytes!("../../res/machines/wheel_loader.glb"),
            Self::Dozer => include_bytes!("../../res/machines/dozer.glb"),
            Self::DrillRig => include_bytes!("../../res/machines/drill_rig.glb"),
            Self::Mpu => include_bytes!("../../res/machines/mpu.glb"),
        }
    }

    /// Roughly how long the model is, in metres: how far apart two of them
    /// working side by side have to stand.
    pub(crate) fn length(self) -> f64 {
        match self {
            Self::Excavator => 10.0,
            Self::WheelLoader => 7.5,
            Self::Dozer => 8.0,
            Self::DrillRig => 7.0,
            Self::Mpu => 9.0,
        }
    }

    /// How far behind the work a machine's origin stands, in metres along
    /// its heading: an excavator's bucket reaches the face from its tracks,
    /// a dozer's blade is ahead of its middle.
    pub(crate) fn reach(self) -> f64 {
        match self {
            Self::Excavator => 6.0,
            Self::WheelLoader => 4.0,
            Self::Dozer => 4.0,
            Self::DrillRig | Self::Mpu => 0.0,
        }
    }
}

/// One machine at the instant Animate shows: where it stands, in world
/// coordinates on the ground, and which way it faces in plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MachinePlacement {
    pub(crate) model: MachineModel,
    pub(crate) position: DVec3,
    /// Unit vector in plan the model's front points along.
    pub(crate) heading: DVec2,
    /// Where its icon stands, zoomed out: the middle of the block or blast it
    /// works, which holds still while the machine moves across it.
    pub(crate) anchor: DVec3,
    /// Whether it is at work at the instant shown, rather than left standing
    /// where its last job was.
    pub(crate) working: bool,
}

/// A machine drawn as an icon, zoomed out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MachineIcon {
    pub(crate) model: MachineModel,
    /// In window pixels.
    pub(crate) at: (f32, f32),
    pub(crate) working: bool,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub(crate) struct MachineVertex {
    /// Model space in metres, Z up and the machine's front along +X.
    pub(crate) pos: [f32; 3],
    pub(crate) normal: [f32; 3],
    pub(crate) color: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub(crate) struct MachineInstance {
    /// Relative to the scene origin.
    pub(crate) position: [f32; 3],
    /// The model's longest plan extent in metres, which the shader holds at
    /// a least size on screen.
    pub(crate) length: f32,
    pub(crate) heading: [f32; 2],
    pub(crate) _pad: [f32; 2],
}

/// A model flattened to one indexed triangle list.
pub(crate) struct MachineMesh {
    pub(crate) vertices: Vec<MachineVertex>,
    pub(crate) indices: Vec<u32>,
    /// Longest plan extent, in metres.
    pub(crate) length: f32,
}

impl MachineMesh {
    pub(crate) fn load(model: MachineModel) -> anyhow::Result<Self> {
        read_glb(model.glb()).with_context(|| format!("reading the {model:?} model"))
    }
}

/// The models on the GPU, and the instances placed at the instant shown.
pub(crate) struct MachineGpu {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    /// Each model's index range and length, by `MachineModel::ALL` position.
    models: Vec<(std::ops::Range<u32>, f32)>,
    instances: wgpu::Buffer,
    capacity: usize,
    /// Each model's instances, by `MachineModel::ALL` position.
    draws: Vec<std::ops::Range<u32>>,
    /// What the instances were built from.
    placed: (Vec<MachinePlacement>, DVec3),
}

impl MachineGpu {
    pub(crate) fn new(device: &wgpu::Device) -> anyhow::Result<Self> {
        use wgpu::util::DeviceExt;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut models = Vec::new();
        for model in MachineModel::ALL {
            let mesh = MachineMesh::load(model)?;
            let base = vertices.len() as u32;
            let first = indices.len() as u32;
            vertices.extend(mesh.vertices);
            indices.extend(mesh.indices.into_iter().map(|index| base + index));
            models.push((first..indices.len() as u32, mesh.length));
        }
        Ok(Self {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Machine Vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Machine Indices"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            models,
            instances: Self::instance_buffer(device, 1),
            capacity: 1,
            draws: Vec::new(),
            placed: (Vec::new(), DVec3::ZERO),
        })
    }

    fn instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Machine Instances"),
            size: (capacity * size_of::<MachineInstance>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Place the machines, relative to `origin`. Uploads only when they or
    /// the origin moved.
    pub(crate) fn place(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, placements: &[MachinePlacement], origin: DVec3) {
        if self.placed.0 == placements && self.placed.1 == origin {
            return;
        }
        self.placed = (placements.to_vec(), origin);
        let mut instances = Vec::with_capacity(placements.len());
        self.draws.clear();
        for (position, model) in MachineModel::ALL.into_iter().enumerate() {
            let first = instances.len() as u32;
            let length = self.models[position].1;
            instances.extend(placements.iter().filter(|placement| placement.model == model).map(|placement| MachineInstance {
                position: (placement.position - origin).as_vec3().to_array(),
                length,
                heading: placement.heading.as_vec2().to_array(),
                _pad: [0.0; 2],
            }));
            self.draws.push(first..instances.len() as u32);
        }
        if instances.is_empty() {
            return;
        }
        if instances.len() > self.capacity {
            self.capacity = instances.len().next_power_of_two();
            self.instances = Self::instance_buffer(device, self.capacity);
        }
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&instances));
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.placed.0.is_empty()
    }

    /// Draws every placed machine; the caller has set the pipeline and the
    /// camera bind group.
    pub(crate) fn draw(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        if self.is_empty() {
            return;
        }
        render_pass.set_vertex_buffer(0, self.vertices.slice(..));
        render_pass.set_vertex_buffer(1, self.instances.slice(..));
        render_pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        for ((indices, _), instances) in self.models.iter().zip(&self.draws) {
            if !instances.is_empty() {
                render_pass.draw_indexed(indices.clone(), 0, instances.clone());
            }
        }
    }
}

#[derive(Deserialize)]
struct Gltf {
    #[serde(default)]
    scene: Option<usize>,
    #[serde(default)]
    scenes: Vec<Scene>,
    #[serde(default)]
    nodes: Vec<Node>,
    #[serde(default)]
    meshes: Vec<Mesh>,
    #[serde(default)]
    materials: Vec<Material>,
    #[serde(default)]
    accessors: Vec<Accessor>,
    #[serde(default, rename = "bufferViews")]
    buffer_views: Vec<BufferView>,
}

#[derive(Deserialize)]
struct Scene {
    #[serde(default)]
    nodes: Vec<usize>,
}

#[derive(Deserialize)]
struct Node {
    #[serde(default)]
    children: Vec<usize>,
    mesh: Option<usize>,
    matrix: Option<[f32; 16]>,
    translation: Option<[f32; 3]>,
    rotation: Option<[f32; 4]>,
    scale: Option<[f32; 3]>,
}

#[derive(Deserialize)]
struct Mesh {
    primitives: Vec<Primitive>,
}

#[derive(Deserialize)]
struct Primitive {
    attributes: std::collections::HashMap<String, usize>,
    indices: Option<usize>,
    material: Option<usize>,
    #[serde(default = "triangles")]
    mode: u32,
}

fn triangles() -> u32 {
    4
}

#[derive(Deserialize)]
struct Material {
    #[serde(default, rename = "pbrMetallicRoughness")]
    pbr: Option<Pbr>,
}

#[derive(Deserialize)]
struct Pbr {
    #[serde(rename = "baseColorFactor")]
    base_color: Option<[f32; 4]>,
}

#[derive(Deserialize)]
struct Accessor {
    #[serde(rename = "bufferView")]
    buffer_view: Option<usize>,
    #[serde(default, rename = "byteOffset")]
    byte_offset: usize,
    #[serde(rename = "componentType")]
    component_type: u32,
    count: usize,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct BufferView {
    #[serde(default)]
    buffer: usize,
    #[serde(default, rename = "byteOffset")]
    byte_offset: usize,
    #[serde(rename = "byteLength")]
    byte_length: usize,
    #[serde(rename = "byteStride")]
    byte_stride: Option<usize>,
}

const FLOAT: u32 = 5126;
const UNSIGNED_BYTE: u32 = 5121;
const UNSIGNED_SHORT: u32 = 5123;
const UNSIGNED_INT: u32 = 5125;

/// Reads a binary glTF into one triangle list in Z-up model space.
fn read_glb(bytes: &[u8]) -> anyhow::Result<MachineMesh> {
    let word = |at: usize| -> anyhow::Result<u32> {
        let slice = bytes.get(at..at + 4).context("truncated file")?;
        Ok(u32::from_le_bytes(slice.try_into()?))
    };
    ensure!(word(0)? == 0x4654_6C67, "not a binary glTF");
    ensure!(word(4)? == 2, "glTF version {} is not 2", word(4)?);
    let mut json = None;
    let mut binary: &[u8] = &[];
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let length = word(at)? as usize;
        let kind = word(at + 4)?;
        let chunk = bytes.get(at + 8..at + 8 + length).context("truncated chunk")?;
        match kind {
            0x4E4F_534A => json = Some(chunk),
            0x004E_4942 => binary = chunk,
            _ => {}
        }
        at += 8 + length;
    }
    let gltf: Gltf = serde_json::from_slice(json.context("no JSON chunk")?)?;

    let view = |accessor: &Accessor, components: usize| -> anyhow::Result<(&[u8], usize, usize)> {
        let index = accessor.buffer_view.context("accessor without a buffer view")?;
        let view = gltf.buffer_views.get(index).context("missing buffer view")?;
        ensure!(view.buffer == 0, "only the embedded buffer is read");
        let size = match accessor.component_type {
            FLOAT | UNSIGNED_INT => 4,
            UNSIGNED_SHORT => 2,
            UNSIGNED_BYTE => 1,
            other => bail!("component type {other} is not read"),
        };
        let data = binary
            .get(view.byte_offset..view.byte_offset + view.byte_length)
            .context("buffer view outside the buffer")?;
        let stride = view.byte_stride.unwrap_or(size * components);
        let start = accessor.byte_offset;
        if accessor.count > 0 {
            ensure!(start + stride * (accessor.count - 1) + size * components <= data.len(), "accessor outside its view");
        }
        Ok((&data[start..], stride, size))
    };
    let floats = |index: usize| -> anyhow::Result<Vec<Vec3>> {
        let accessor = gltf.accessors.get(index).context("missing accessor")?;
        ensure!(accessor.kind == "VEC3" && accessor.component_type == FLOAT, "expected float vectors");
        let (data, stride, _) = view(accessor, 3)?;
        Ok((0..accessor.count)
            .map(|i| {
                let at = i * stride;
                let f = |k: usize| f32::from_le_bytes(data[at + 4 * k..at + 4 * k + 4].try_into().expect("four bytes"));
                Vec3::new(f(0), f(1), f(2))
            })
            .collect())
    };
    let indices = |index: usize| -> anyhow::Result<Vec<u32>> {
        let accessor = gltf.accessors.get(index).context("missing accessor")?;
        ensure!(accessor.kind == "SCALAR", "expected scalar indices");
        let (data, stride, size) = view(accessor, 1)?;
        Ok((0..accessor.count)
            .map(|i| {
                let at = i * stride;
                match size {
                    4 => u32::from_le_bytes(data[at..at + 4].try_into().expect("four bytes")),
                    2 => u32::from(u16::from_le_bytes(data[at..at + 2].try_into().expect("two bytes"))),
                    _ => u32::from(data[at]),
                }
            })
            .collect())
    };

    // glTF is Y up with +Z toward the viewer; the scene is Z up. Blender's
    // exporter maps its (x, y, z) to (x, z, -y), so this maps it back.
    let to_z_up = Mat4::from_cols_array(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]);
    let mut mesh = MachineMesh {
        vertices: Vec::new(),
        indices: Vec::new(),
        length: 0.0,
    };
    let roots = gltf.scenes.get(gltf.scene.unwrap_or(0)).map(|scene| scene.nodes.clone()).unwrap_or_default();
    let mut stack: Vec<(usize, Mat4, usize)> = roots.into_iter().map(|node| (node, to_z_up, 0)).collect();
    while let Some((index, parent, depth)) = stack.pop() {
        ensure!(depth < 64, "node tree too deep");
        let node = gltf.nodes.get(index).context("missing node")?;
        let local = match node.matrix {
            Some(matrix) => Mat4::from_cols_array(&matrix),
            None => Mat4::from_scale_rotation_translation(
                node.scale.map_or(Vec3::ONE, Vec3::from),
                node.rotation.map_or(Quat::IDENTITY, Quat::from_array),
                node.translation.map_or(Vec3::ZERO, Vec3::from),
            ),
        };
        let world = parent * local;
        let normal_matrix = world.inverse().transpose();
        if let Some(mesh_index) = node.mesh {
            for primitive in &gltf.meshes.get(mesh_index).context("missing mesh")?.primitives {
                if primitive.mode != 4 {
                    continue;
                }
                let positions = floats(*primitive.attributes.get("POSITION").context("primitive without positions")?)?;
                let normals = match primitive.attributes.get("NORMAL") {
                    Some(&index) => Some(floats(index)?),
                    None => None,
                };
                let color = primitive
                    .material
                    .and_then(|material| gltf.materials.get(material))
                    .and_then(|material| material.pbr.as_ref())
                    .and_then(|pbr| pbr.base_color)
                    .map_or([0.6, 0.6, 0.6], |color| [color[0], color[1], color[2]]);
                let base = mesh.vertices.len() as u32;
                let local_indices = match primitive.indices {
                    Some(index) => indices(index)?,
                    None => (0..positions.len() as u32).collect(),
                };
                ensure!(local_indices.iter().all(|&index| (index as usize) < positions.len()), "index past the vertices");
                for (i, position) in positions.iter().enumerate() {
                    let normal = normals.as_ref().and_then(|normals| normals.get(i)).copied().unwrap_or(Vec3::Z);
                    mesh.vertices.push(MachineVertex {
                        pos: world.transform_point3(*position).to_array(),
                        normal: normal_matrix.transform_vector3(normal).normalize_or_zero().to_array(),
                        color,
                    });
                }
                mesh.indices.extend(local_indices.as_chunks::<3>().0.iter().flatten().map(|index| base + index));
            }
        }
        stack.extend(node.children.iter().map(|&child| (child, world, depth + 1)));
    }
    ensure!(!mesh.indices.is_empty(), "no triangles");
    let (min, max) = mesh
        .vertices
        .iter()
        .fold((Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)), |(min, max), vertex| {
            (min.min(Vec3::from(vertex.pos)), max.max(Vec3::from(vertex.pos)))
        });
    mesh.length = (max.x - min.x).max(max.y - min.y);
    Ok(mesh)
}
