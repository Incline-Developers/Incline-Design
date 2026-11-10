//! HiGHS's `HighsSymmetryDetection` (`presolve/HighsSymmetry.cpp`): the
//! automorphisms of the model's coloured column/row graph by partition
//! refinement and search (with bliss's certificate prefix pruning), the
//! orbits they leave, and the components that are full orbitopes.
//!
//! Vertices, cells and positions are HiGHS's `HighsInt`s (`i32`, -1 for
//! none). The hash tables here are only looked up, never walked, so any
//! table gives HiGHS's results.

use std::collections::{HashMap, HashSet};

use super::{
    MipModel, VarType, cpp_heap_pop, cpp_heap_push, cpp_partition,
    symmetry::{OrbitopeMatrix, Symmetries},
};
use crate::{
    pdqsort::{pdqsort, pdqsort_branchless},
    util::hash::{C, pair_hash},
};

const INF: f64 = f64::INFINITY;
const M31: u64 = 0x7fffffff;

fn multiply_mod_m31(a: u32, b: u32) -> u32 {
    let mut result = a as u64 * b as u64;
    result = (result >> 31) + (result & M31);
    if result >= M31 {
        result -= M31;
    }
    result as u32
}

fn modexp_m31(a: u32, mut e: u64) -> u32 {
    let mut result = a;
    while e != 1 {
        result = multiply_mod_m31(result, result);
        if e & 1 != 0 {
            result = multiply_mod_m31(result, a);
        }
        e >>= 1;
    }
    result
}

/// HiGHS's `HighsHashHelpers::sparse_combine32`.
fn sparse_combine32(hash: &mut u32, index: i32, value: u64) {
    let value = (pair_hash(0, value as u32, (value >> 32) as u32) >> 33) | 1;
    let a = (C[(index & 63) as usize] & M31) as u32;
    let degree = (index as u64 >> 6) + 1;
    let mut result = *hash as u64;
    result += multiply_mod_m31(value as u32, modexp_m31(a, degree)) as u64;
    result = (result >> 31) + (result & M31);
    if result >= M31 {
        result -= M31;
    }
    *hash = result as u32;
}

/// HiGHS's `HighsMatrixColoring`: a colour per value, values within the
/// tolerance of one already coloured sharing its colour.
struct MatrixColoring {
    /// `std::map<double, u32>`, ordered by value.
    color_map: Vec<(f64, u32)>,
    tolerance: f64,
}

impl MatrixColoring {
    fn new(tolerance: f64) -> Self {
        Self {
            color_map: vec![(-INF, 3), (0.0, 1), (1.0, 2), (INF, 4)],
            tolerance,
        }
    }

    fn color(&mut self, value: f64) -> u32 {
        let at = self.color_map.partition_point(|&(key, _)| key < value - self.tolerance);
        if at == self.color_map.len() || self.color_map[at].0 > value + self.tolerance {
            let color = self.color_map.len() as u32 + 1;
            self.color_map.insert(at, (value, color));
            return color;
        }
        self.color_map[at].1
    }
}

/// HiGHS's `HighsDisjointSets` (without minimal representatives).
#[derive(Default)]
struct DisjointSets {
    sizes: Vec<i32>,
    sets: Vec<i32>,
    link_compression_stack: Vec<i32>,
}

impl DisjointSets {
    fn reset(&mut self, num_items: usize) {
        self.sizes = vec![1; num_items];
        self.sets = (0..num_items as i32).collect();
    }

    fn get_set(&mut self, mut item: i32) -> i32 {
        let mut repr = self.sets[item as usize];
        if repr != self.sets[repr as usize] {
            loop {
                self.link_compression_stack.push(item);
                item = repr;
                repr = self.sets[repr as usize];
                if repr == self.sets[repr as usize] {
                    break;
                }
            }
            while let Some(i) = self.link_compression_stack.pop() {
                self.sets[i as usize] = repr;
            }
            self.sets[item as usize] = repr;
        }
        repr
    }

    fn get_set_size(&self, set: i32) -> i32 {
        self.sizes[set as usize]
    }

    fn merge(&mut self, item1: i32, item2: i32) {
        let repr1 = self.get_set(item1);
        let repr2 = self.get_set(item2);
        if repr1 == repr2 {
            return;
        }
        if self.sizes[repr1 as usize] > self.sizes[repr2 as usize] {
            self.sets[repr2 as usize] = repr1;
            self.sizes[repr1 as usize] += self.sizes[repr2 as usize];
        } else {
            self.sets[repr1 as usize] = repr2;
            self.sizes[repr2 as usize] += self.sizes[repr1 as usize];
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Node {
    stack_start: i32,
    certificate_end: i32,
    target_cell: i32,
    last_distinguished: i32,
}

/// HiGHS's `ComponentData`.
#[derive(Default)]
struct ComponentData {
    components: DisjointSets,
    component_starts: Vec<i32>,
    component_sets: Vec<i32>,
    component_num_orbits: Vec<i32>,
    component_number: Vec<i32>,
    perm_component_starts: Vec<i32>,
    perm_components: Vec<i32>,
    first_unfixed: Vec<i32>,
    num_unfixed: Vec<i32>,
}

impl ComponentData {
    fn num_components(&self) -> i32 {
        self.component_starts.len() as i32 - 1
    }

    fn component_size(&self, component: i32) -> i32 {
        self.component_starts[component as usize + 1] - self.component_starts[component as usize]
    }
}

type Graph = HashSet<(i32, i32, u32)>;

#[derive(Default)]
pub(crate) struct SymmetryDetection {
    col_lower: Vec<f64>,
    col_upper: Vec<f64>,
    integrality: Vec<VarType>,

    g_start: Vec<i32>,
    g_end: Vec<i32>,
    g_edge: Vec<(i32, u32)>,
    current_partition: Vec<i32>,
    current_partition_links: Vec<i32>,
    vertex_to_cell: Vec<i32>,
    vertex_position: Vec<i32>,
    vertex_ground_set: Vec<i32>,
    orbit_partition: Vec<i32>,
    orbit_size: Vec<i32>,
    cell_creation_stack: Vec<i32>,
    cell_in_refinement_queue: Vec<bool>,
    refinement_queue: Vec<i32>,
    /// Positions in `current_partition` (HiGHS keeps pointers into it).
    distinguish_cands: Vec<usize>,
    automorphisms: Vec<i32>,
    link_compression_stack: Vec<i32>,
    curr_node_certificate: Vec<u32>,
    first_leave_certificate: Vec<u32>,
    best_leave_certificate: Vec<u32>,
    first_leave_partition: Vec<i32>,
    best_leave_partition: Vec<i32>,
    vertex_hash: HashMap<i32, u32>,
    first_leave_graph: Graph,
    best_leave_graph: Graph,
    first_leave_prefix_len: i32,
    best_leave_prefix_len: i32,
    first_path_depth: i32,
    best_path_depth: i32,
    num_automorphisms: i32,
    num_col: i32,
    num_row: i32,
    num_vertices: i32,
    num_active_cols: i32,
    node_stack: Vec<Node>,
}

impl SymmetryDetection {
    fn cell_size(&self, cell: i32) -> i32 {
        self.current_partition_links[cell as usize] - cell
    }

    /// HiGHS's `removeFixPoints`: drop the vertices in singleton cells.
    fn remove_fix_points(&mut self) {
        self.g_end.resize(self.num_vertices as usize, 0);
        for i in 0..self.num_vertices as usize {
            let (start, end) = (self.g_start[i] as usize, self.g_start[i + 1] as usize);
            let (links, cells) = (&self.current_partition_links, &self.vertex_to_cell);
            let kept = cpp_partition(&mut self.g_edge[start..end], |edge| {
                let cell = cells[edge.0 as usize];
                links[cell as usize] - cell > 1
            });
            self.g_end[i] = (start + kept) as i32;
        }

        let mut unit_cell_index = self.num_vertices;
        let (links, cells) = (&self.current_partition_links, &mut self.vertex_to_cell);
        self.current_partition.retain(|&vertex| {
            let cell = cells[vertex as usize];
            if links[cell as usize] - cell == 1 {
                unit_cell_index -= 1;
                cells[vertex as usize] = unit_cell_index;
                return false;
            }
            true
        });

        for i in 0..self.num_vertices as usize {
            if self.g_end[i] == self.g_start[i + 1] {
                continue;
            }
            for j in self.g_end[i] as usize..self.g_start[i + 1] as usize {
                self.g_edge[j].0 = self.vertex_to_cell[self.g_edge[j].0 as usize];
            }
        }

        if (self.current_partition.len() as i32) < self.num_vertices {
            self.num_vertices = self.current_partition.len() as i32;
            if self.num_vertices == 0 {
                self.num_active_cols = 0;
                return;
            }
            let n = self.num_vertices as usize;
            self.current_partition_links.resize(n, 0);
            self.cell_in_refinement_queue = vec![false; n];
            self.refinement_queue.clear();
            let mut cell_start = 0;
            let mut cell_number = 0;
            for i in 0..n {
                let vertex = self.current_partition[i];
                if cell_number != self.vertex_to_cell[vertex as usize] {
                    cell_number = self.vertex_to_cell[vertex as usize];
                    self.current_partition_links[cell_start] = i as i32;
                    cell_start = i;
                }
                self.update_cell_membership(i as i32, cell_start as i32, false);
            }
            self.current_partition_links[cell_start] = self.num_vertices;
            let num_col = self.num_col;
            self.num_active_cols = self.current_partition.partition_point(|&v| v < num_col) as i32;
        } else {
            self.num_active_cols = self.num_col;
        }
    }

    /// HiGHS's `initializeGroundSet`.
    fn initialize_ground_set(&mut self) {
        self.vertex_ground_set = self.current_partition.clone();
        pdqsort_branchless(&mut self.vertex_ground_set, |a, b| a < b);
        self.vertex_position.resize(self.vertex_to_cell.len(), -1);
        for i in 0..self.num_vertices as usize {
            self.vertex_position[self.vertex_ground_set[i] as usize] = i as i32;
        }
        self.orbit_partition = (0..self.num_vertices).collect();
        self.orbit_size = vec![1; self.num_vertices as usize];
        self.automorphisms.resize(self.num_vertices as usize * 64, 0);
        self.num_automorphisms = 0;
    }

    fn merge_orbits(&mut self, v1: i32, v2: i32) -> bool {
        if v1 == v2 {
            return false;
        }
        let orbit1 = self.get_orbit(v1);
        let orbit2 = self.get_orbit(v2);
        if orbit1 == orbit2 {
            return false;
        }
        if orbit1 < orbit2 {
            self.orbit_partition[orbit2 as usize] = orbit1;
            self.orbit_size[orbit1 as usize] += self.orbit_size[orbit2 as usize];
        } else {
            self.orbit_partition[orbit1 as usize] = orbit2;
            self.orbit_size[orbit2 as usize] += self.orbit_size[orbit1 as usize];
        }
        true
    }

    fn get_orbit(&mut self, vertex: i32) -> i32 {
        let mut i = self.vertex_position[vertex as usize];
        let mut orbit = self.orbit_partition[i as usize];
        if orbit != self.orbit_partition[orbit as usize] {
            loop {
                self.link_compression_stack.push(i);
                i = orbit;
                orbit = self.orbit_partition[orbit as usize];
                if orbit == self.orbit_partition[orbit as usize] {
                    break;
                }
            }
            while let Some(i) = self.link_compression_stack.pop() {
                self.orbit_partition[i as usize] = orbit;
            }
        }
        orbit
    }

    fn initialize_hash_values(&mut self) {
        for i in 0..self.num_vertices as usize {
            let cell = self.vertex_to_cell[i];
            for j in self.g_start[i] as usize..self.g_end[i] as usize {
                let (dest, color) = self.g_edge[j];
                sparse_combine32(self.vertex_hash.entry(dest).or_insert(0), cell, color as u64);
            }
            self.mark_cell_for_refinement(cell);
        }
    }

    fn update_cell_membership(&mut self, i: i32, cell: i32, mark_for_refinement: bool) -> bool {
        let vertex = self.current_partition[i as usize];
        if self.vertex_to_cell[vertex as usize] != cell {
            self.vertex_to_cell[vertex as usize] = cell;
            if i != cell {
                self.current_partition_links[i as usize] = cell;
            }
            if mark_for_refinement {
                for j in self.g_start[vertex as usize] as usize..self.g_end[vertex as usize] as usize {
                    let (dest, color) = self.g_edge[j];
                    let edge_destination_cell = self.vertex_to_cell[dest as usize];
                    if self.cell_size(edge_destination_cell) == 1 {
                        continue;
                    }
                    sparse_combine32(self.vertex_hash.entry(dest).or_insert(0), cell, color as u64);
                    self.mark_cell_for_refinement(edge_destination_cell);
                }
            }
            return true;
        }
        false
    }

    fn get_vertex_hash(&self, v: i32) -> u32 {
        self.vertex_hash.get(&v).copied().unwrap_or(0)
    }

    fn split_cell(&mut self, cell: i32, split_point: i32) -> bool {
        let h_split = self.get_vertex_hash(self.current_partition[split_point as usize]);
        let h_cell = self.get_vertex_hash(self.current_partition[cell as usize]);
        let certificate_val = (pair_hash(0, h_split, h_cell)
            .wrapping_add(pair_hash(1, cell as u32, (self.current_partition_links[cell as usize] - split_point) as u32))
            .wrapping_add(pair_hash(2, split_point as u32, (split_point - cell) as u32))
            >> 32) as u32;
        if !self.first_leave_certificate.is_empty() {
            let len = self.curr_node_certificate.len() as i32;
            self.first_leave_prefix_len += ((self.first_leave_prefix_len == len) as i32) * ((certificate_val == self.first_leave_certificate[len as usize]) as i32);
            self.best_leave_prefix_len += ((self.best_leave_prefix_len == len) as i32) * ((certificate_val == self.best_leave_certificate[len as usize]) as i32);
            if self.first_leave_prefix_len <= len && self.best_leave_prefix_len <= len {
                let diff_val = if self.best_leave_prefix_len == len {
                    certificate_val
                } else {
                    self.curr_node_certificate[self.best_leave_prefix_len as usize]
                };
                if diff_val > self.best_leave_certificate[self.best_leave_prefix_len as usize] {
                    return false;
                }
            }
        }
        self.current_partition_links[split_point as usize] = self.current_partition_links[cell as usize];
        self.current_partition_links[cell as usize] = split_point;
        self.cell_creation_stack.push(split_point);
        self.curr_node_certificate.push(certificate_val);
        true
    }

    fn mark_cell_for_refinement(&mut self, cell: i32) {
        if self.cell_size(cell) == 1 || self.cell_in_refinement_queue[cell as usize] {
            return;
        }
        self.cell_in_refinement_queue[cell as usize] = true;
        cpp_heap_push(&mut self.refinement_queue, cell, |a, b| a > b);
    }

    /// The node can be pruned: leave the queue and hashes clear.
    fn abandon_refinement(&mut self) {
        for &c in &self.refinement_queue {
            self.cell_in_refinement_queue[c as usize] = false;
        }
        self.refinement_queue.clear();
        self.vertex_hash.clear();
    }

    /// HiGHS's `partitionRefinement`: split cells by the vertices'
    /// neighbourhood hashes until stable. False if the node can be pruned.
    fn partition_refinement(&mut self) -> bool {
        while !self.refinement_queue.is_empty() {
            let mut cell_start = cpp_heap_pop(&mut self.refinement_queue, |a, b| a > b).unwrap();
            let first_cell_start = cell_start;
            self.cell_in_refinement_queue[cell_start as usize] = false;
            if self.cell_size(cell_start) == 1 {
                continue;
            }
            let cell_end = self.current_partition_links[cell_start as usize];
            // What a pruned refinement must roll back (HiGHS #3182).
            let start_creation_stack_size = self.cell_creation_stack.len();
            let start_certificate_size = self.curr_node_certificate.len();
            let refine_start = {
                let hashes = &self.vertex_hash;
                cell_start + cpp_partition(&mut self.current_partition[cell_start as usize..cell_end as usize], |v| !hashes.contains_key(v)) as i32
            };
            if refine_start == cell_end {
                continue;
            }
            {
                let hashes = &self.vertex_hash;
                pdqsort(&mut self.current_partition[refine_start as usize..cell_end as usize], |v1, v2| hashes[v1] < hashes[v2]);
            }
            if refine_start != cell_start {
                if !self.split_cell(cell_start, refine_start) {
                    self.abandon_refinement();
                    return false;
                }
                cell_start = refine_start;
                self.update_cell_membership(cell_start, cell_start, true);
            }
            let mut prune = false;
            let mut last_hash = self.vertex_hash[&self.current_partition[cell_start as usize]] as u64;
            let mut i = cell_start + 1;
            while i < cell_end {
                let vertex = self.current_partition[i as usize];
                let hash = self.vertex_hash[&vertex] as u64;
                if hash != last_hash {
                    if !self.split_cell(cell_start, i) {
                        prune = true;
                        break;
                    }
                    cell_start = i;
                    last_hash = hash;
                }
                self.update_cell_membership(i, cell_start, true);
                i += 1;
            }
            if prune {
                self.abandon_refinement();
                self.current_partition_links[first_cell_start as usize] = cell_end;
                i -= 1;
                while i >= refine_start {
                    self.update_cell_membership(i, first_cell_start, false);
                    i -= 1;
                }
                self.cell_creation_stack.truncate(start_creation_stack_size);
                self.curr_node_certificate.truncate(start_certificate_size);
                if !self.first_leave_certificate.is_empty() {
                    self.first_leave_prefix_len = self.first_leave_prefix_len.min(start_certificate_size as i32);
                    self.best_leave_prefix_len = self.best_leave_prefix_len.min(start_certificate_size as i32);
                }
                return false;
            }
        }
        self.vertex_hash.clear();
        true
    }

    fn select_target_cell(&self) -> i32 {
        let mut i = 0;
        if self.node_stack.len() > 1 {
            i = self.node_stack[self.node_stack.len() - 2].target_cell;
        }
        while i < self.num_vertices {
            if self.cell_size(i) > 1 {
                return i;
            }
            i += 1;
        }
        -1
    }

    fn check_stored_automorphism(&self, vertex: i32) -> bool {
        let num_check = self.num_automorphisms.min(64);
        let n = self.num_vertices as usize;
        for i in 0..num_check as usize {
            let automorphism = &self.automorphisms[i * n..(i + 1) * n];
            let mut automorphism_useful = true;
            let mut j = self.node_stack.len() as i32 - 2;
            while j >= self.first_path_depth {
                let fix_pos = self.vertex_position[self.node_stack[j as usize].last_distinguished as usize] as usize;
                if automorphism[fix_pos] != self.vertex_ground_set[fix_pos] {
                    automorphism_useful = false;
                    break;
                }
                j -= 1;
            }
            if !automorphism_useful {
                continue;
            }
            if automorphism[self.vertex_position[vertex as usize] as usize] < vertex {
                return false;
            }
        }
        true
    }

    fn determine_next_to_distinguish(&mut self) -> bool {
        let curr_node = *self.node_stack.last().unwrap();
        self.distinguish_cands.clear();
        let cell_start = curr_node.target_cell as usize;
        let cell_end = self.current_partition_links[curr_node.target_cell as usize] as usize;
        // `std::min_element`: the first smallest.
        let first_min = |positions: &mut dyn Iterator<Item = usize>, partition: &[i32]| {
            let mut best: Option<usize> = None;
            for pos in positions {
                if best.is_none_or(|b| partition[pos] < partition[b]) {
                    best = Some(pos);
                }
            }
            best
        };
        if curr_node.last_distinguished == -1 {
            let pos = first_min(&mut (cell_start..cell_end), &self.current_partition).unwrap();
            self.distinguish_cands.push(pos);
        } else if self.node_stack.len() as i32 > self.first_path_depth {
            for i in cell_start..cell_end {
                let v = self.current_partition[i];
                if v > curr_node.last_distinguished && self.check_stored_automorphism(v) {
                    self.distinguish_cands.push(i);
                }
            }
            if self.distinguish_cands.is_empty() {
                return false;
            }
            let pos = first_min(&mut self.distinguish_cands.iter().copied(), &self.current_partition).unwrap();
            self.distinguish_cands.clear();
            self.distinguish_cands.push(pos);
        } else {
            for i in cell_start..cell_end {
                let v = self.current_partition[i];
                if v > curr_node.last_distinguished {
                    let orbit = self.get_orbit(v);
                    if self.vertex_ground_set[orbit as usize] == v {
                        self.distinguish_cands.push(i);
                    }
                }
            }
            if self.distinguish_cands.is_empty() {
                return false;
            }
            let pos = first_min(&mut self.distinguish_cands.iter().copied(), &self.current_partition).unwrap();
            self.distinguish_cands.clear();
            self.distinguish_cands.push(pos);
        }
        true
    }

    fn distinguish_vertex(&mut self, target_cell: i32) -> bool {
        let target_cell_end = self.current_partition_links[target_cell as usize];
        let new_cell = target_cell_end - 1;
        self.current_partition.swap(self.distinguish_cands[0], new_cell as usize);
        self.node_stack.last_mut().unwrap().last_distinguished = self.current_partition[new_cell as usize];
        if !self.split_cell(target_cell, new_cell) {
            return false;
        }
        self.update_cell_membership(new_cell, new_cell, true);
        true
    }

    fn backtrack(&mut self, backtrack_stack_new_end: i32, backtrack_stack_end: i32) {
        let mut stack_pos = backtrack_stack_end - 1;
        while stack_pos >= backtrack_stack_new_end {
            let cell = self.cell_creation_stack[stack_pos as usize];
            let new_start = self.get_cell_start(cell - 1);
            let curr_end = self.current_partition_links[cell as usize];
            self.current_partition_links[cell as usize] = new_start;
            self.current_partition_links[new_start as usize] = curr_end;
            stack_pos -= 1;
        }
    }

    fn cleanup_backtrack(&mut self, cell_creation_stack_pos: i32) {
        let mut stack_pos = self.cell_creation_stack.len() as i32 - 1;
        while stack_pos >= cell_creation_stack_pos {
            let cell = self.cell_creation_stack[stack_pos as usize];
            let cell_start = self.get_cell_start(cell);
            let cell_end = self.current_partition_links[cell_start as usize];
            let mut v = cell;
            while v < cell_end && self.vertex_to_cell[self.current_partition[v as usize] as usize] == cell {
                self.update_cell_membership(v, cell_start, false);
                v += 1;
            }
            stack_pos -= 1;
        }
        self.cell_creation_stack.truncate(cell_creation_stack_pos as usize);
    }

    fn get_cell_start(&mut self, mut pos: i32) -> i32 {
        let mut start_pos = self.current_partition_links[pos as usize];
        if start_pos > pos {
            return pos;
        }
        if self.current_partition_links[start_pos as usize] < start_pos {
            loop {
                self.link_compression_stack.push(pos);
                pos = start_pos;
                start_pos = self.current_partition_links[start_pos as usize];
                if self.current_partition_links[start_pos as usize] >= start_pos {
                    break;
                }
            }
            while let Some(p) = self.link_compression_stack.pop() {
                self.current_partition_links[p as usize] = start_pos;
            }
        }
        start_pos
    }

    fn create_node(&mut self) {
        self.node_stack.push(Node {
            stack_start: self.cell_creation_stack.len() as i32,
            certificate_end: self.curr_node_certificate.len() as i32,
            target_cell: -1,
            last_distinguished: -1,
        });
    }

    /// HiGHS's `loadModelAsGraph`: the bipartite column/row graph with
    /// coloured edges, and the initial partition by column and row
    /// colours.
    pub fn load_model_as_graph(model: &MipModel, epsilon: f64) -> Self {
        let mut d = Self {
            col_lower: model.col_lower.clone(),
            col_upper: model.col_upper.clone(),
            integrality: model.integrality.clone(),
            ..Self::default()
        };
        let num_col = model.num_col as i32;
        let num_row = model.num_row as i32;
        d.num_col = num_col;
        d.num_row = num_row;
        d.num_vertices = num_row + num_col;
        let nv = d.num_vertices as usize;
        d.cell_in_refinement_queue = vec![false; nv];
        d.vertex_to_cell = vec![0; nv];
        let mut column_set: HashMap<[u32; 5], i32> = HashMap::new();
        let mut row_set: HashMap<[u32; 3], i32> = HashMap::new();
        let mut coloring = MatrixColoring::new(epsilon);
        let num_nz = model.a_index.len();
        d.g_edge = model
            .a_index
            .iter()
            .map(|&row| (row as i32 + num_col, 0))
            .chain(std::iter::repeat_n((0, 0), num_nz))
            .collect();
        d.g_start = vec![0; nv + 1];
        for (i, &s) in model.a_start.iter().enumerate() {
            d.g_start[i] = s as i32;
        }
        let mut row_sizes = vec![0i32; num_row as usize];
        for i in 0..num_col as usize {
            for j in d.g_start[i] as usize..d.g_start[i + 1] as usize {
                d.g_edge[j].1 = coloring.color(model.a_value[j]);
                row_sizes[model.a_index[j]] += 1;
            }
        }
        let mut offset = num_nz as i32;
        for i in 0..num_row as usize {
            d.g_start[num_col as usize + i] = offset;
            offset += row_sizes[i];
        }
        d.g_start[nv] = offset;
        d.g_end = d.g_start[1..].to_vec();
        for i in 0..num_col as usize {
            for j in d.g_start[i] as usize..d.g_start[i + 1] as usize {
                let row = model.a_index[j];
                let ar_pos = (d.g_start[num_col as usize + row + 1] - row_sizes[row]) as usize;
                row_sizes[row] -= 1;
                d.g_edge[ar_pos].0 = i as i32;
                d.g_edge[ar_pos].1 = d.g_edge[j].1;
            }
        }

        let index_offset = num_col + 1;
        for i in 0..num_col as usize {
            let matrix_col = [
                coloring.color(model.col_cost[i]),
                coloring.color(model.col_lower[i]),
                coloring.color(model.col_upper[i]),
                model.integrality[i] as u32,
                (d.g_start[i + 1] - d.g_start[i]) as u32,
            ];
            let size_before = column_set.len();
            let column_cell = column_set.entry(matrix_col).or_insert(0);
            if *column_cell == 0 {
                *column_cell = size_before as i32 + 1;
                if model.col_lower[i] != 0.0 || model.col_upper[i] != 1.0 || model.integrality[i] == VarType::Continuous {
                    *column_cell += index_offset;
                }
            }
            d.vertex_to_cell[i] = *column_cell;
        }
        let index_offset = 2 * num_col + 1;
        for i in 0..num_row as usize {
            let matrix_row = [
                coloring.color(model.row_lower[i]),
                coloring.color(model.row_upper[i]),
                (d.g_start[num_col as usize + i + 1] - d.g_start[num_col as usize + i]) as u32,
            ];
            let size_before = row_set.len();
            let row_cell = row_set.entry(matrix_row).or_insert(0);
            if *row_cell == 0 {
                *row_cell = size_before as i32 + 1;
            }
            d.vertex_to_cell[num_col as usize + i] = index_offset + *row_cell;
        }

        d.current_partition = (0..d.num_vertices).collect();
        {
            let cells = &d.vertex_to_cell;
            pdqsort(&mut d.current_partition, |&v1, &v2| cells[v1 as usize] < cells[v2 as usize]);
        }
        d.current_partition_links = vec![0; nv];
        let mut cell_start = 0;
        let mut cell_number = 0;
        for i in 0..nv {
            let vertex = d.current_partition[i] as usize;
            if cell_number != d.vertex_to_cell[vertex] {
                cell_number = d.vertex_to_cell[vertex];
                d.current_partition_links[cell_start] = i as i32;
                cell_start = i;
            }
            d.vertex_to_cell[vertex] = cell_start as i32;
            d.current_partition_links[i] = cell_start as i32;
        }
        d.current_partition_links[cell_start] = d.num_vertices;
        d
    }

    fn dump_current_graph(&self) -> Graph {
        let mut graph_triplets = Graph::new();
        for i in 0..self.num_col as usize {
            let col_cell = self.vertex_to_cell[i];
            for j in self.g_start[i] as usize..self.g_end[i] as usize {
                graph_triplets.insert((self.vertex_to_cell[self.g_edge[j].0 as usize], col_cell, self.g_edge[j].1));
            }
            for g_edge_j in &self.g_edge[self.g_end[i] as usize..self.g_start[i + 1] as usize] {
                graph_triplets.insert((g_edge_j.0, col_cell, g_edge_j.1));
            }
        }
        graph_triplets
    }

    fn switch_to_next_node(&mut self, backtrack_depth: i32) {
        let mut stack_end = self.cell_creation_stack.len() as i32;
        self.node_stack.truncate(backtrack_depth as usize);
        if backtrack_depth == 0 {
            return;
        }
        loop {
            let curr_node = *self.node_stack.last().unwrap();
            self.backtrack(curr_node.stack_start, stack_end);
            stack_end = curr_node.stack_start;
            let depth = self.node_stack.len() as i32;
            self.first_path_depth = depth.min(self.first_path_depth);
            self.best_path_depth = depth.min(self.best_path_depth);
            self.first_leave_prefix_len = curr_node.certificate_end.min(self.first_leave_prefix_len);
            self.best_leave_prefix_len = curr_node.certificate_end.min(self.best_leave_prefix_len);
            self.curr_node_certificate.truncate(curr_node.certificate_end as usize);
            if !self.determine_next_to_distinguish() {
                self.node_stack.pop();
                if self.node_stack.is_empty() {
                    break;
                }
                continue;
            }
            self.cleanup_backtrack(stack_end);
            let target_cell = curr_node.target_cell;
            if !self.distinguish_vertex(target_cell) {
                self.node_stack.pop();
                if self.node_stack.is_empty() {
                    break;
                }
                continue;
            }
            if !self.partition_refinement() {
                stack_end = self.cell_creation_stack.len() as i32;
                continue;
            }
            self.create_node();
            break;
        }
    }

    fn compare_current_graph(&self, other_graph: &Graph, wrong_cell: &mut i32) -> bool {
        for i in 0..self.num_col as usize {
            let col_cell = self.vertex_to_cell[i];
            for j in self.g_start[i] as usize..self.g_end[i] as usize {
                if !other_graph.contains(&(self.vertex_to_cell[self.g_edge[j].0 as usize], col_cell, self.g_edge[j].1)) {
                    *wrong_cell = col_cell;
                    return false;
                }
            }
            for g_edge_j in &self.g_edge[self.g_end[i] as usize..self.g_start[i + 1] as usize] {
                if !other_graph.contains(&(g_edge_j.0, col_cell, g_edge_j.1)) {
                    *wrong_cell = col_cell;
                    return false;
                }
            }
        }
        true
    }

    fn is_from_binary_column(&self, pos: i32) -> bool {
        if pos >= self.num_active_cols {
            return false;
        }
        let col = self.current_partition[pos as usize] as usize;
        !(self.col_lower[col] != 0.0 || self.col_upper[col] != 1.0 || self.integrality[col] == VarType::Continuous)
    }

    fn compute_component_data(&mut self, symmetries: &Symmetries) -> ComponentData {
        let mut cd = ComponentData::default();
        let nac = self.num_active_cols as usize;
        cd.components.reset(nac);
        cd.first_unfixed = vec![-1; symmetries.num_perms];
        cd.num_unfixed = vec![0; symmetries.num_perms];
        for i in 0..symmetries.num_perms {
            let perm = &symmetries.permutations[i * nac..(i + 1) * nac];
            for j in 0..nac {
                if perm[j] as i32 != self.vertex_ground_set[j] {
                    let pos = self.vertex_position[perm[j]];
                    cd.num_unfixed[i] += 1;
                    if cd.first_unfixed[i] != -1 {
                        cd.components.merge(cd.first_unfixed[i], pos);
                    } else {
                        cd.first_unfixed[i] = pos;
                    }
                }
            }
        }
        cd.component_sets = self.vertex_ground_set[..nac].to_vec();
        {
            let (components, positions) = (&mut cd.components, &self.vertex_position);
            let mut sets = std::mem::take(&mut cd.component_sets);
            pdqsort(&mut sets, |&u, &v| {
                let u_comp = components.get_set(positions[u as usize]);
                let v_comp = components.get_set(positions[v as usize]);
                (components.get_set_size(u_comp) == 1, u_comp) < (components.get_set_size(v_comp) == 1, v_comp)
            });
            cd.component_sets = sets;
        }
        let mut current_component = -1;
        let mut curr_component_orbits: HashSet<i32> = HashSet::new();
        for i in 0..nac {
            let comp = cd.components.get_set(self.vertex_position[cd.component_sets[i] as usize]);
            if cd.components.get_set_size(comp) == 1 {
                break;
            }
            if comp != current_component {
                current_component = comp;
                cd.component_starts.push(i as i32);
                cd.component_number.push(current_component);
                cd.component_num_orbits.push(0);
                curr_component_orbits.clear();
            }
            let orbit = self.get_orbit(cd.component_sets[i]);
            if curr_component_orbits.insert(orbit) {
                *cd.component_num_orbits.last_mut().unwrap() += 1;
            }
        }
        for i in 0..symmetries.num_perms as i32 {
            if cd.first_unfixed[i as usize] == -1 {
                continue;
            }
            cd.perm_components.push(i);
        }
        {
            let (components, first_unfixed, num_unfixed) = (&mut cd.components, &cd.first_unfixed, &cd.num_unfixed);
            let mut perms = std::mem::take(&mut cd.perm_components);
            pdqsort(&mut perms, |&i, &j| {
                let seti = components.get_set(first_unfixed[i as usize]);
                let setj = components.get_set(first_unfixed[j as usize]);
                (seti, num_unfixed[i as usize]) < (setj, num_unfixed[j as usize])
            });
            cd.perm_components = perms;
        }
        let mut current_component = -1;
        let num_used_perms = cd.perm_components.len();
        for i in 0..num_used_perms {
            let p = cd.perm_components[i];
            let comp = cd.components.get_set(cd.first_unfixed[p as usize]);
            if comp != current_component {
                current_component = comp;
                cd.perm_component_starts.push(i as i32);
            }
        }
        cd.perm_component_starts.push(num_used_perms as i32);
        cd.component_starts.push(self.num_active_cols);
        cd
    }

    fn is_full_orbitope(&self, cd: &ComponentData, component: i32, symmetries: &mut Symmetries) -> bool {
        let c = component as usize;
        let component_size = cd.component_starts[c + 1] - cd.component_starts[c];
        if component_size == 1 {
            return false;
        }
        for i in cd.component_starts[c]..cd.component_starts[c + 1] {
            let col = cd.component_sets[i as usize] as usize;
            if self.integrality[col] == VarType::Continuous || self.col_lower[col] != 0.0 || self.col_upper[col] != 1.0 {
                return false;
            }
        }
        let p0 = cd.perm_components[cd.perm_component_starts[c] as usize] as usize;
        if cd.num_unfixed[p0] & 1 != 0 {
            return false;
        }
        for k in cd.perm_component_starts[c] + 1..cd.perm_component_starts[c + 1] {
            let p = cd.perm_components[k as usize] as usize;
            if cd.num_unfixed[p] != cd.num_unfixed[p0] {
                return false;
            }
        }
        let orbitope_num_rows = cd.num_unfixed[p0] >> 1;
        let orbitope_orbit_size = component_size / orbitope_num_rows;
        if orbitope_orbit_size * orbitope_num_rows != component_size {
            return false;
        }
        let component_num_perms = cd.perm_component_starts[c + 1] - cd.perm_component_starts[c];
        if component_num_perms != orbitope_orbit_size - 1 {
            return false;
        }
        let num_rows = orbitope_num_rows as usize;
        let row_length = orbitope_orbit_size as usize;
        let mut matrix: Vec<i32> = vec![-1; component_size as usize];
        let nac = self.num_active_cols as usize;
        let perm_of = |p: usize| &symmetries.permutations[p * nac..(p + 1) * nac];
        let mut perm = perm_of(p0);
        let mut col_set: HashSet<i32> = HashSet::new();
        let mut m = 0;
        for j in 0..nac {
            let j_image_pos = self.vertex_position[perm[j]] as usize;
            if j_image_pos <= j {
                continue;
            }
            if m == num_rows {
                return false;
            }
            if perm[j_image_pos] as i32 != self.vertex_ground_set[j] {
                return false;
            }
            matrix[m] = self.vertex_ground_set[j];
            matrix[num_rows + m] = perm[j] as i32;
            if !col_set.insert(self.vertex_ground_set[j]) {
                return false;
            }
            if !col_set.insert(perm[j] as i32) {
                return false;
            }
            m += 1;
        }
        let mut num_cols_added = 2;
        let mut tried_left_extension = false;
        while num_cols_added < row_length {
            if col_set.len() != num_cols_added * num_rows {
                return false;
            }
            let this_col = num_cols_added * num_rows;
            let mut prev_col = (num_cols_added - 1) * num_rows;
            let mut found_cand = false;
            loop {
                let move_pos = self.vertex_position[matrix[prev_col] as usize] as usize;
                for k in cd.perm_component_starts[c] + 1..cd.perm_component_starts[c + 1] {
                    let p = cd.perm_components[k as usize] as usize;
                    perm = perm_of(p);
                    if perm[move_pos] as i32 != self.vertex_ground_set[move_pos] && !col_set.contains(&(perm[move_pos] as i32)) {
                        found_cand = true;
                        break;
                    }
                }
                if !found_cand && !tried_left_extension {
                    prev_col = 0;
                    tried_left_extension = true;
                    continue;
                }
                break;
            }
            if !found_cand {
                return false;
            }
            for j in 0..num_rows {
                let next_vertex = perm[self.vertex_position[matrix[prev_col + j] as usize] as usize] as i32;
                matrix[this_col + j] = next_vertex;
                if perm[self.vertex_position[next_vertex as usize] as usize] as i32 != matrix[prev_col + j] {
                    return false;
                }
                if !col_set.insert(next_vertex) {
                    return false;
                }
            }
            num_cols_added += 1;
        }
        if col_set.len() != component_size as usize {
            return false;
        }
        let index = symmetries.orbitopes.len();
        for &col in &matrix {
            symmetries.column_to_orbitope.insert(col as usize, index);
        }
        symmetries.orbitopes.push(OrbitopeMatrix {
            row_length,
            num_rows,
            matrix: matrix.into_iter().map(|col| col as usize).collect(),
            ..OrbitopeMatrix::default()
        });
        true
    }

    /// HiGHS's `initializeDetection`: whether any column is in a cell of
    /// more than one after the first refinement.
    pub fn initialize_detection(&mut self) -> bool {
        self.initialize_hash_values();
        self.partition_refinement();
        self.remove_fix_points();
        self.num_active_cols != 0
    }

    /// HiGHS's `run`: the generators of the automorphism group, their
    /// orbits and the full orbitopes, into `symmetries`.
    pub fn run(&mut self, symmetries: &mut Symmetries) {
        self.initialize_ground_set();
        self.curr_node_certificate.clear();
        self.cell_creation_stack.clear();
        self.create_node();
        let max_perms = 64000000 / self.num_active_cols as usize;
        let nv = self.num_vertices as usize;
        let nac = self.num_active_cols as usize;
        while !self.node_stack.is_empty() {
            let target_cell = self.select_target_cell();
            if target_cell == -1 {
                if self.first_leave_partition.is_empty() {
                    self.first_leave_partition = self.current_partition.clone();
                    self.first_leave_certificate = self.curr_node_certificate.clone();
                    self.best_leave_certificate = self.curr_node_certificate.clone();
                    self.first_leave_graph = self.dump_current_graph();
                    self.first_path_depth = self.node_stack.len() as i32;
                    self.best_path_depth = self.node_stack.len() as i32;
                    self.first_leave_prefix_len = self.curr_node_certificate.len() as i32;
                    self.best_leave_prefix_len = self.curr_node_certificate.len() as i32;
                    let mut backtrack_depth = self.first_path_depth - 1;
                    while backtrack_depth > 0 && !self.is_from_binary_column(self.node_stack[backtrack_depth as usize - 1].target_cell) {
                        backtrack_depth -= 1;
                    }
                    self.switch_to_next_node(backtrack_depth);
                } else {
                    let mut wrong_cell = -1;
                    let mut backtrack_depth = self.node_stack.len() as i32 - 1;
                    let len = self.curr_node_certificate.len() as i32;
                    if self.first_leave_prefix_len == len || self.best_leave_prefix_len == len {
                        let leave = if self.first_leave_prefix_len == len && self.compare_current_graph(&self.first_leave_graph, &mut wrong_cell) {
                            Some(true)
                        } else if !self.best_leave_partition.is_empty() && self.best_leave_prefix_len == len && self.compare_current_graph(&self.best_leave_graph, &mut wrong_cell)
                        {
                            Some(false)
                        } else {
                            None
                        };
                        match leave {
                            Some(first) => {
                                let k = (self.num_automorphisms & 63) as usize;
                                self.num_automorphisms += 1;
                                for i in 0..nv {
                                    let leave_col = if first { self.first_leave_partition[i] } else { self.best_leave_partition[i] };
                                    let at = self.vertex_position[self.current_partition[i] as usize] as usize;
                                    self.automorphisms[k * nv + at] = leave_col;
                                }
                                let mut report = false;
                                for i in 0..nv {
                                    let image = self.automorphisms[k * nv + i];
                                    if self.merge_orbits(image, self.vertex_ground_set[i]) && i < nac {
                                        report = true;
                                    }
                                }
                                if report {
                                    symmetries.permutations.extend(self.automorphisms[k * nv..k * nv + nac].iter().map(|&v| v as usize));
                                    symmetries.num_perms += 1;
                                    if symmetries.num_perms == max_perms {
                                        break;
                                    }
                                }
                                let depth = if first { self.first_path_depth } else { self.best_path_depth };
                                backtrack_depth = backtrack_depth.min(depth);
                            }
                            None => {
                                let blen = self.best_leave_prefix_len as usize;
                                if (blen as i32) < len && self.curr_node_certificate[blen] > self.best_leave_certificate[blen] {
                                    let mut possible_backtrack_depth = self.first_path_depth - 1;
                                    while self.node_stack[possible_backtrack_depth as usize].certificate_end <= self.best_leave_prefix_len {
                                        possible_backtrack_depth += 1;
                                    }
                                    backtrack_depth = possible_backtrack_depth.min(backtrack_depth);
                                } else {
                                    // A hash collision the graph comparison
                                    // caught: back to where the wrong cell was
                                    // last targeted.
                                    let mut possible_backtrack_depth = backtrack_depth;
                                    while possible_backtrack_depth >= 0 {
                                        if self.node_stack[possible_backtrack_depth as usize].target_cell == wrong_cell {
                                            backtrack_depth = possible_backtrack_depth;
                                            break;
                                        }
                                        possible_backtrack_depth -= 1;
                                    }
                                }
                            }
                        }
                    } else {
                        self.best_leave_certificate = self.curr_node_certificate.clone();
                        self.best_leave_graph = self.dump_current_graph();
                        self.best_leave_partition = self.current_partition.clone();
                        self.best_path_depth = self.node_stack.len() as i32;
                        self.best_leave_prefix_len = self.curr_node_certificate.len() as i32;
                    }
                    self.switch_to_next_node(backtrack_depth);
                }
            } else {
                self.node_stack.last_mut().unwrap().target_cell = target_cell;
                self.determine_next_to_distinguish();
                if !self.distinguish_vertex(target_cell) {
                    let depth = self.node_stack.len() as i32 - 1;
                    self.switch_to_next_node(depth);
                    continue;
                }
                if !self.partition_refinement() {
                    let depth = self.node_stack.len() as i32;
                    self.switch_to_next_node(depth);
                    continue;
                }
                self.create_node();
            }
        }

        symmetries.num_generators = symmetries.num_perms;
        if symmetries.num_perms > 0 {
            self.vertex_position.resize(self.num_col as usize, -1);
            let cd = self.compute_component_data(symmetries);
            for i in 0..cd.num_components() {
                if cd.component_size(i) == 1 {
                    continue;
                }
                self.is_full_orbitope(&cd, i, symmetries);
            }
            let mut deleted_perms: HashSet<usize> = HashSet::new();
            for p in 0..symmetries.num_perms {
                let perm = &symmetries.permutations[p * nac..(p + 1) * nac];
                for i in 0..nac {
                    if perm[i] as i32 != self.vertex_ground_set[i] && symmetries.column_to_orbitope.find(&perm[i]).is_some() {
                        deleted_perms.insert(p);
                        break;
                    }
                }
            }
            let mut num_fixed = 0;
            for i in 0..nac {
                let v = self.vertex_ground_set[i];
                let orbit = self.get_orbit(v);
                if self.orbit_size[orbit as usize] == 1 || symmetries.column_to_orbitope.find(&(v as usize)).is_some() {
                    self.vertex_position[v as usize] = -1;
                    self.vertex_ground_set[i] = -1;
                    num_fixed += 1;
                }
            }
            let mut nac = nac;
            if num_fixed != 0 {
                let old = std::mem::take(&mut symmetries.permutations);
                let mut kept_perms = 0;
                for p in 0..symmetries.num_perms {
                    if deleted_perms.contains(&p) {
                        continue;
                    }
                    kept_perms += 1;
                    for i in 0..nac {
                        if self.vertex_ground_set[i] == -1 {
                            continue;
                        }
                        symmetries.permutations.push(old[p * nac + i]);
                    }
                }
                symmetries.num_perms = kept_perms;
                let mut out_pos = 0;
                for i in 0..nac {
                    if self.vertex_ground_set[i] == -1 {
                        continue;
                    }
                    self.vertex_ground_set[out_pos] = self.vertex_ground_set[i];
                    self.vertex_position[self.vertex_ground_set[out_pos] as usize] = out_pos as i32;
                    out_pos += 1;
                }
                nac -= num_fixed;
                self.num_active_cols = nac as i32;
            }
            self.vertex_ground_set.truncate(nac);
            symmetries.permutation_columns = self.vertex_ground_set.iter().map(|&v| v as usize).collect();
            symmetries.column_position = self.vertex_position.iter().map(|&p| (p >= 0).then_some(p as usize)).collect();
            symmetries.permutations.truncate(symmetries.num_perms * nac);
        }
    }
}
