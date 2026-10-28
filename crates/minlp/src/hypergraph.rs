//! Port of `hypergraph.c`: a hypergraph with vertices, edges and overlaps of
//! edge pairs.
//!
//! The vertex, edge and overlap data (C's byte blocks of fixed size) are
//! typed arrays. The overlap hash table only answers lookups, so it is a map
//! from vertex sets to overlaps. `SCIPhypergraphIsValid` is not ported.

use std::collections::HashMap;

/// `SCIP_HYPERGRAPH_VERTEX`.
pub type Vertex = i32;
/// `SCIP_HYPERGRAPH_EDGE`.
pub type Edge = i32;
/// `SCIP_HYPERGRAPH_OVERLAP`.
pub type Overlap = i32;

/// `SCIP_HYPERGRAPH`: a hypergraph with vertices, edges and overlaps of edge
/// pairs.
pub struct Hypergraph<V, E, O> {
    /// Number of vertices.
    pub nvertices: i32,
    /// Number of edges.
    pub nedges: i32,
    /// Number of overlaps.
    pub noverlaps: i32,

    /// Array with vertex data.
    pub verticesdata: Vec<V>,
    /// Array with edge data.
    pub edgesdata: Vec<E>,
    /// Array with indices of edges' incident vertices.
    pub edgesverticesbeg: Vec<i32>,
    /// Array with all edges' vertices.
    pub edgesvertices: Vec<Vertex>,

    /// Whether there is a mapping from vertices to incident edges.
    pub hasvertexedges: bool,
    /// Array with indices of vertices' incident edges.
    pub verticesedgesbeg: Vec<i32>,
    /// Array with all vertices' incident edges.
    pub verticesedges: Vec<Edge>,

    /// Whether overlap sets are known.
    pub hasoverlaps: bool,
    /// Hashtable for overlap sets (`NULL`: None).
    overlaphashtable: Option<HashMap<Vec<Vertex>, Overlap>>,
    /// Array with indices of overlaps' vertices.
    pub overlapsverticesbeg: Vec<i32>,
    /// Array with all overlaps' vertices.
    pub overlapsvertices: Vec<Vertex>,
    /// Array with overlaps' data.
    pub overlapsdata: Vec<O>,
    /// Array with indices of edges' incident overlaps.
    pub edgesoverlapsbeg: Vec<i32>,
    /// Array with edges' incident overlaps.
    pub edgesoverlaps: Vec<Overlap>,

    /// Whether overlaps' edges are known.
    pub hasoverlapsedges: bool,
    /// Array with indices of overlaps' incident edges.
    pub overlapsedgesbeg: Vec<i32>,
    /// Array with overlaps' incident edges.
    pub overlapsedges: Vec<Edge>,

    /// Whether vertices' overlaps are known.
    pub hasverticesoverlaps: bool,
    /// Array with indices of vertices' incident overlaps.
    pub verticesoverlapsbeg: Vec<i32>,
    /// Array with vertices' incident overlaps.
    pub verticesoverlaps: Vec<Overlap>,
}

/// Called once an overlap is found with `(hypergraph, overlap, first edge,
/// second edge, created)`.
pub type OverlapHandler<V, E, O> = dyn FnMut(&mut Hypergraph<V, E, O>, Overlap, Edge, Edge, bool);

impl<V: Default + Clone, E: Default + Clone, O: Default + Clone> Hypergraph<V, E, O> {
    /// `SCIPhypergraphCreate`: creates a hypergraph (the expected sizes only
    /// reserve memory).
    pub fn new(memvertices: i32, memedges: i32, memoverlaps: i32, memedgesvertices: i32) -> Self {
        let mut h = Self {
            nvertices: 0,
            nedges: 0,
            noverlaps: 0,
            verticesdata: Vec::with_capacity(memvertices.max(0) as usize),
            edgesdata: Vec::with_capacity(memedges.max(0) as usize),
            edgesverticesbeg: vec![0],
            edgesvertices: Vec::with_capacity((memedgesvertices.max(0) as usize).saturating_mul(memedges.max(0) as usize).min(1 << 20)),
            hasvertexedges: false,
            verticesedgesbeg: Vec::new(),
            verticesedges: Vec::new(),
            hasoverlaps: false,
            overlaphashtable: None,
            overlapsverticesbeg: vec![0; 1],
            overlapsvertices: Vec::new(),
            overlapsdata: Vec::with_capacity(memoverlaps.max(0) as usize),
            edgesoverlapsbeg: Vec::new(),
            edgesoverlaps: Vec::new(),
            hasoverlapsedges: false,
            overlapsedgesbeg: Vec::new(),
            overlapsedges: Vec::new(),
            hasverticesoverlaps: false,
            verticesoverlapsbeg: Vec::new(),
            verticesoverlaps: Vec::new(),
        };
        h.clear();
        h
    }

    /// `SCIPhypergraphClear`: clears a hypergraph, deleting all vertices and
    /// edges.
    pub fn clear(&mut self) {
        self.nvertices = 0;
        self.nedges = 0;
        self.noverlaps = 0;
        self.verticesdata.clear();
        self.edgesdata.clear();
        self.edgesverticesbeg.truncate(1);
        self.edgesverticesbeg[0] = 0;
        self.edgesvertices.clear();
        self.hasvertexedges = false;
        self.hasoverlaps = false;
        self.overlaphashtable = None;
        self.hasoverlapsedges = false;
        self.hasverticesoverlaps = false;
    }

    /// `SCIPhypergraphAddVertex`: adds a new vertex to the hypergraph; returns
    /// it, its data to be set through [`Self::vertex_data_mut`].
    pub fn add_vertex(&mut self) -> Vertex {
        let v = self.nvertices;
        self.verticesdata.push(V::default());
        self.nvertices += 1;
        self.hasvertexedges = false;
        v
    }

    /// `SCIPhypergraphAddEdge`: adds a new edge to the hypergraph; does not
    /// update the vertices' or overlaps' lists of incident edges.
    pub fn add_edge(&mut self, vertices: &[Vertex]) -> Edge {
        let first = self.edgesverticesbeg[self.nedges as usize] as usize;
        let beyond = first + vertices.len();
        self.edgesvertices.truncate(first);
        self.edgesvertices.extend_from_slice(vertices);
        self.edgesverticesbeg.push(beyond as i32);
        crate::sort::sort(&mut self.edgesvertices[first..beyond], (), crate::sort::cmp_int);

        let e = self.nedges;
        self.edgesdata.push(E::default());
        self.nedges += 1;
        self.hasvertexedges = false;
        e
    }

    /// `findOverlap`: finds an overlap specified by a sorted vertex set,
    /// potentially adding it; returns the overlap (-1 if not found and not
    /// added) and whether it was added.
    fn find_overlap(&mut self, vertices: &[Vertex], add: bool) -> (Overlap, bool) {
        if let Some(&o) = self.overlaphashtable.as_ref().unwrap().get(vertices) {
            (o, false)
        } else if add {
            // In order to find a set of vertices we need to add it to the
            // overlaps' vertex storage.
            let first = self.overlapsverticesbeg[self.noverlaps as usize] as usize;
            self.overlapsvertices.truncate(first);
            self.overlapsvertices.extend_from_slice(vertices);
            self.overlapsverticesbeg.truncate(self.noverlaps as usize + 1);
            self.overlapsverticesbeg.push((first + vertices.len()) as i32);
            self.overlapsdata.push(O::default());

            let o = self.noverlaps;
            self.overlaphashtable.as_mut().unwrap().insert(vertices.to_vec(), o);
            self.noverlaps += 1;
            (o, true)
        } else {
            (-1, false)
        }
    }

    /// `SCIPhypergraphComputeOverlaps`: computes all overlaps and stores
    /// overlaps' vertices and all edges' overlaps; requires the vertices'
    /// edges. `handler` is called once an overlap is found with `(hypergraph,
    /// overlap, first edge, second edge, created)`.
    pub fn compute_overlaps(&mut self, mut handler: Option<&mut OverlapHandler<V, E, O>>) -> crate::retcode::ScipResult {
        if !self.hasvertexedges {
            return Err(crate::retcode::Retcode::Error);
        }

        if self.hasoverlaps {
            return Ok(());
        }

        self.noverlaps = 0;
        self.overlapsverticesbeg.clear();
        self.overlapsverticesbeg.push(0);
        self.overlapsvertices.clear();
        self.overlapsdata.clear();

        self.hasoverlaps = true;
        self.overlaphashtable = Some(HashMap::new());

        // An array consisting of pairs (e,o) where o is an overlap incident to
        // edge e.
        let mut edgeoverlappairs: Vec<i64> = Vec::new();
        let mut commonvertices: Vec<Vertex> = Vec::new();

        // We iterate over all edges e.
        for e in 0..self.nedges {
            let ebeyond = self.edgesverticesbeg[e as usize + 1];
            let ebegin = self.edgesverticesbeg[e as usize];
            let esize = ebeyond - ebegin;

            // We iterate over all vertices v of e and then over all edges f
            // incident to v to avoid scanning all pairs of edges because most of
            // them will be disjoint.
            for i in 0..esize {
                let v = self.edgesvertices[(ebegin + i) as usize];
                let vfirst = self.verticesedgesbeg[v as usize];
                let vbeyond = self.verticesedgesbeg[v as usize + 1];
                for j in vfirst..vbeyond {
                    let f = self.verticesedges[j as usize];

                    // Avoid considering (e,f) and (f,e).
                    if f <= e {
                        continue;
                    }

                    // We now traverse the sorted lists of vertices of e and f
                    // simultaneously to find common vertices.
                    let fbeyond = self.edgesverticesbeg[f as usize + 1];
                    commonvertices.clear();
                    let mut ie = ebegin;
                    let mut je = self.edgesverticesbeg[f as usize];
                    while ie < ebeyond && je < fbeyond {
                        let u = self.edgesvertices[ie as usize];
                        let w = self.edgesvertices[je as usize];
                        if u < w {
                            ie += 1;
                        } else if u > w {
                            je += 1;
                        } else {
                            // Vertex u = w is part of e and of f.
                            if commonvertices.is_empty() && u != v {
                                // v is not the minimum vertex in e \cap f, so we
                                // skip f since we have considered it already for
                                // a smaller v.
                                break;
                            }
                            commonvertices.push(u);
                            ie += 1;
                            je += 1;
                        }
                    }

                    // We only consider overlap sets that are not just vertices.
                    let ncommon = commonvertices.len();
                    if ncommon >= 2 {
                        // Find out if that overlap (as a set) already exists.
                        let cv = std::mem::take(&mut commonvertices);
                        let (overlap, added) = self.find_overlap(&cv, true);
                        commonvertices = cv;
                        if let Some(h) = handler.as_deref_mut() {
                            h(self, overlap, e, f, added);
                        }

                        // For the pair (overlap,e) we store (e << 40) | (|overlap|
                        // << 24) | overlap and sort by it to find duplicates and
                        // the get overlaps incident to e in order of increasing
                        // cardinality.
                        let obits = overlap as u64;
                        let ocardinalitybits = (ncommon as u64) << 24;
                        let ebits = ((e as u64) << 40) | ocardinalitybits | obits;
                        let fbits = ((f as u64) << 40) | ocardinalitybits | obits;
                        edgeoverlappairs.push(ebits as i64);
                        edgeoverlappairs.push(fbits as i64);
                    }
                }
            }
        }

        crate::sort::sort(&mut edgeoverlappairs[..], (), |a: &i64, b: &i64| a.cmp(b) as i32);

        // Remove duplicates.
        edgeoverlappairs.dedup();
        let numedgeoverlappairs = edgeoverlappairs.len();

        // Compute edges' incident overlaps.
        self.edgesoverlapsbeg = vec![0; self.nedges as usize + 1];
        self.edgesoverlaps = vec![0; numedgeoverlappairs];

        let mut lastedge: i32 = -1;
        for (i, &pair) in edgeoverlappairs.iter().enumerate() {
            let bits = pair as u64;
            let edge = (bits >> 40) as i32;
            let overlap = (bits & ((1u64 << 24) - 1)) as i32;

            while lastedge < edge {
                lastedge += 1;
                self.edgesoverlapsbeg[lastedge as usize] = i as i32;
            }
            self.edgesoverlaps[i] = overlap;
        }
        while lastedge < self.nedges {
            lastedge += 1;
            self.edgesoverlapsbeg[lastedge as usize] = numedgeoverlappairs as i32;
        }

        Ok(())
    }

    /// `SCIPhypergraphComputeVerticesEdges`: computes each vertex' list of
    /// incident edges.
    pub fn compute_vertices_edges(&mut self) {
        if self.hasvertexedges {
            return;
        }

        let nv = self.nvertices as usize;

        // We know the total number of vertex-edge incidences from the edges.
        let nincidences = self.edgesverticesbeg[self.nedges as usize] as usize;
        self.verticesedges = vec![0; nincidences];

        // We initialize beg[v] = 0.
        self.verticesedgesbeg = vec![0; nv + 1];

        // We traverse through all incidences (using the edges' vertex arrays)
        // and count the degree deg(v) in beg[v].
        for i in 0..nincidences {
            self.verticesedgesbeg[self.edgesvertices[i] as usize] += 1;
        }

        // This loop sets beg[0] = 0, beg[1] = deg(0), beg[2] = deg(0) +
        // deg(1), beg[v] = deg(0) + ... + deg(v-1).
        let mut current = 0;
        for beg in &mut self.verticesedgesbeg[..nv] {
            let temp = current;
            current += *beg;
            *beg = temp;
        }

        // We now traverse through all edges e and their incident vertices v,
        // storing e at beg[v], which is incremented.
        for e in 0..self.nedges as usize {
            let first = self.edgesverticesbeg[e] as usize;
            let beyond = self.edgesverticesbeg[e + 1] as usize;
            for i in first..beyond {
                let v = self.edgesvertices[i] as usize;
                let pos = self.verticesedgesbeg[v] as usize;
                self.verticesedges[pos] = e as i32;
                self.verticesedgesbeg[v] += 1;
            }
        }

        // We now revert the incrementation of the beg[v]-values again.
        for v in (1..=nv).rev() {
            self.verticesedgesbeg[v] = self.verticesedgesbeg[v - 1];
        }
        self.verticesedgesbeg[0] = 0;
        self.hasvertexedges = true;
    }

    /// `SCIPhypergraphOverlapFind`: finds the overlap corresponding to vertex
    /// set `vertices`; returns -1 if it is not found.
    pub fn overlap_find(&mut self, vertices: &[Vertex]) -> Overlap {
        self.find_overlap(vertices, false).0
    }

    /// `SCIPhypergraphIntersectEdges`: finds the overlap or singleton vertex
    /// corresponding to the intersection of edges `first` and `second`;
    /// returns `(overlap, vertex)`, the vertex only set if no overlap is found.
    pub fn intersect_edges(&mut self, first: Edge, second: Edge) -> (Overlap, Vertex) {
        let mut commonvertices = Vec::new();
        let firstbeyond = self.edgesverticesbeg[first as usize + 1];
        let secondbeyond = self.edgesverticesbeg[second as usize + 1];
        let mut i = self.edgesverticesbeg[first as usize];
        let mut j = self.edgesverticesbeg[second as usize];
        while i < firstbeyond && j < secondbeyond {
            let u = self.edgesvertices[i as usize];
            let w = self.edgesvertices[j as usize];
            if u < w {
                i += 1;
            } else if u > w {
                j += 1;
            } else {
                commonvertices.push(u);
                i += 1;
                j += 1;
            }
        }

        let overlap = self.find_overlap(&commonvertices, false).0;
        let vertex = if overlap < 0 {
            if !commonvertices.is_empty() { commonvertices[0] } else { -1 }
        } else {
            -1
        };
        (overlap, vertex)
    }

    /// `SCIPhypergraphComputeOverlapsEdges`: computes all overlaps' lists of
    /// incident edges.
    pub fn compute_overlaps_edges(&mut self) {
        if self.hasoverlapsedges {
            return;
        }

        let no = self.noverlaps as usize;

        // We know the total number of overlap-edge incidences from the edges.
        let nincidences = self.edgesoverlapsbeg[self.nedges as usize] as usize;
        self.overlapsedges = vec![0; nincidences];

        // We initialize beg[v] = 0.
        self.overlapsedgesbeg = vec![0; no + 1];

        // We traverse through all incidences (using the edges' overlap arrays)
        // and count the degree deg(o) in beg[o], where deg(o) shall denote the
        // number of edges incident to overlap o.
        for i in 0..nincidences {
            self.overlapsedgesbeg[self.edgesoverlaps[i] as usize] += 1;
        }

        // This loop sets beg[0] = 0, beg[1] = deg(0), beg[2] = deg(0) +
        // deg(1), beg[o] = deg(0) + ... + deg(o-1).
        let mut current = 0;
        for beg in &mut self.overlapsedgesbeg[..no] {
            let temp = current;
            current += *beg;
            *beg = temp;
        }

        // We now traverse through all edges e and their incident overlaps o,
        // storing e at beg[o], which is incremented.
        for e in 0..self.nedges as usize {
            let first = self.edgesoverlapsbeg[e] as usize;
            let beyond = self.edgesoverlapsbeg[e + 1] as usize;
            for i in first..beyond {
                let o = self.edgesoverlaps[i] as usize;
                let pos = self.overlapsedgesbeg[o] as usize;
                self.overlapsedges[pos] = e as i32;
                self.overlapsedgesbeg[o] += 1;
            }
        }

        // We now revert the incrementation of the beg[o]-values again.
        for o in (1..=no).rev() {
            self.overlapsedgesbeg[o] = self.overlapsedgesbeg[o - 1];
        }
        self.overlapsedgesbeg[0] = 0;
        self.hasoverlapsedges = true;
    }

    /// `SCIPhypergraphComputeVerticesOverlaps`: computes all vertices' lists of
    /// incident overlaps.
    pub fn compute_vertices_overlaps(&mut self) {
        if self.hasverticesoverlaps {
            return;
        }

        let nv = self.nvertices as usize;

        // We know the total number of vertex-overlap incidences from the
        // overlaps.
        let nincidences = self.overlapsverticesbeg[self.noverlaps as usize] as usize;
        self.verticesoverlaps = vec![0; nincidences];

        // We initialize beg[v] = 0.
        self.verticesoverlapsbeg = vec![0; nv + 1];

        // We traverse through all incidences (using the overlaps' vertex
        // arrays) and count the degree deg(v) in beg[v], where deg(v) shall
        // denote the number of overlaps incident to vertex v.
        for i in 0..nincidences {
            self.verticesoverlapsbeg[self.overlapsvertices[i] as usize] += 1;
        }

        // This loop sets beg[0] = 0, beg[1] = deg(0), beg[2] = deg(0) +
        // deg(1), beg[v] = deg(0) + ... + deg(v-1).
        let mut current = 0;
        for beg in &mut self.verticesoverlapsbeg[..nv] {
            let temp = current;
            current += *beg;
            *beg = temp;
        }

        // We traverse through all overlaps o and their incident vertices v,
        // storing o at beg[v], which is incremented.
        for o in 0..self.noverlaps as usize {
            let first = self.overlapsverticesbeg[o] as usize;
            let beyond = self.overlapsverticesbeg[o + 1] as usize;
            for i in first..beyond {
                let v = self.overlapsvertices[i] as usize;
                let pos = self.verticesoverlapsbeg[v] as usize;
                self.verticesoverlaps[pos] = o as i32;
                self.verticesoverlapsbeg[v] += 1;
            }
        }

        // We now revert the incrementation of the beg[o]-values again.
        for v in (1..=nv).rev() {
            self.verticesoverlapsbeg[v] = self.verticesoverlapsbeg[v - 1];
        }
        self.verticesoverlapsbeg[0] = 0;
        self.hasverticesoverlaps = true;
    }

    /// `SCIPhypergraphOverlapsDisjoint`: returns whether overlaps `overlap1`
    /// and `overlap2` are disjoint.
    pub fn overlaps_disjoint(&self, overlap1: Overlap, overlap2: Overlap) -> bool {
        // We loop over the vertices in parallel because they are sorted.
        let vertices1 = self.overlap_vertices(overlap1);
        let vertices2 = self.overlap_vertices(overlap2);
        let (mut i, mut j) = (0, 0);
        while i < vertices1.len() && j < vertices2.len() {
            if vertices1[i] < vertices2[j] {
                i += 1;
            } else if vertices1[i] > vertices2[j] {
                j += 1;
            } else {
                return false;
            }
        }

        true
    }

    /* simple getters */

    /// `SCIPhypergraphGetNVertices`.
    pub fn get_n_vertices(&self) -> i32 {
        self.nvertices
    }

    /// `SCIPhypergraphGetNEdges`.
    pub fn get_n_edges(&self) -> i32 {
        self.nedges
    }

    /// `SCIPhypergraphGetNOverlaps`.
    pub fn get_n_overlaps(&self) -> i32 {
        self.noverlaps
    }

    /// `SCIPhypergraphGetNIncidences`.
    pub fn get_n_incidences(&self) -> i32 {
        self.edgesverticesbeg[self.nedges as usize]
    }

    /// `SCIPhypergraphVertexData`.
    pub fn vertex_data(&self, v: Vertex) -> &V {
        &self.verticesdata[v as usize]
    }

    /// `SCIPhypergraphVertexData`, mutably.
    pub fn vertex_data_mut(&mut self, v: Vertex) -> &mut V {
        &mut self.verticesdata[v as usize]
    }

    /// `SCIPhypergraphEdgeData`.
    pub fn edge_data(&self, e: Edge) -> &E {
        &self.edgesdata[e as usize]
    }

    /// `SCIPhypergraphEdgeData`, mutably.
    pub fn edge_data_mut(&mut self, e: Edge) -> &mut E {
        &mut self.edgesdata[e as usize]
    }

    /// `SCIPhypergraphOverlapData`.
    pub fn overlap_data(&self, o: Overlap) -> &O {
        &self.overlapsdata[o as usize]
    }

    /// `SCIPhypergraphOverlapData`, mutably.
    pub fn overlap_data_mut(&mut self, o: Overlap) -> &mut O {
        &mut self.overlapsdata[o as usize]
    }

    /// `SCIPhypergraphEdgeSize`.
    pub fn edge_size(&self, e: Edge) -> i32 {
        self.edgesverticesbeg[e as usize + 1] - self.edgesverticesbeg[e as usize]
    }

    /// `SCIPhypergraphEdgeVertices`.
    pub fn edge_vertices(&self, e: Edge) -> &[Vertex] {
        &self.edgesvertices[self.edgesverticesbeg[e as usize] as usize..self.edgesverticesbeg[e as usize + 1] as usize]
    }

    /// `SCIPhypergraphVertexEdgesFirst`.
    pub fn vertex_edges_first(&self, v: Vertex) -> i32 {
        self.verticesedgesbeg[v as usize]
    }

    /// `SCIPhypergraphVertexEdgesBeyond`.
    pub fn vertex_edges_beyond(&self, v: Vertex) -> i32 {
        self.verticesedgesbeg[v as usize + 1]
    }

    /// `SCIPhypergraphVertexEdgesGetAtIndex`.
    pub fn vertex_edges_get_at_index(&self, i: i32) -> Edge {
        self.verticesedges[i as usize]
    }

    /// `SCIPhypergraphOverlapSize`.
    pub fn overlap_size(&self, o: Overlap) -> i32 {
        self.overlapsverticesbeg[o as usize + 1] - self.overlapsverticesbeg[o as usize]
    }

    /// `SCIPhypergraphOverlapVertices`.
    pub fn overlap_vertices(&self, o: Overlap) -> &[Vertex] {
        &self.overlapsvertices[self.overlapsverticesbeg[o as usize] as usize..self.overlapsverticesbeg[o as usize + 1] as usize]
    }

    /// `SCIPhypergraphEdgesOverlapsFirst`.
    pub fn edges_overlaps_first(&self, e: Edge) -> i32 {
        self.edgesoverlapsbeg[e as usize]
    }

    /// `SCIPhypergraphEdgesOverlapsBeyond`.
    pub fn edges_overlaps_beyond(&self, e: Edge) -> i32 {
        self.edgesoverlapsbeg[e as usize + 1]
    }

    /// `SCIPhypergraphEdgesOverlapsGetAtIndex`.
    pub fn edges_overlaps_get_at_index(&self, i: i32) -> Overlap {
        self.edgesoverlaps[i as usize]
    }

    /// `SCIPhypergraphOverlapsEdgesFirst`.
    pub fn overlaps_edges_first(&self, o: Overlap) -> i32 {
        self.overlapsedgesbeg[o as usize]
    }

    /// `SCIPhypergraphOverlapsEdgesBeyond`.
    pub fn overlaps_edges_beyond(&self, o: Overlap) -> i32 {
        self.overlapsedgesbeg[o as usize + 1]
    }

    /// `SCIPhypergraphOverlapsEdgesGetAtIndex`.
    pub fn overlaps_edges_get_at_index(&self, i: i32) -> Edge {
        self.overlapsedges[i as usize]
    }

    /// `SCIPhypergraphVertexOverlapsFirst`.
    pub fn vertex_overlaps_first(&self, v: Vertex) -> i32 {
        self.verticesoverlapsbeg[v as usize]
    }

    /// `SCIPhypergraphVertexOverlapsBeyond`.
    pub fn vertex_overlaps_beyond(&self, v: Vertex) -> i32 {
        self.verticesoverlapsbeg[v as usize + 1]
    }

    /// `SCIPhypergraphVertexOverlapsGetAtIndex`.
    pub fn vertex_overlaps_get_at_index(&self, i: i32) -> Overlap {
        self.verticesoverlaps[i as usize]
    }
}

/// `SCIP_HYPERGRAPH_ITER`: data for iterating over adjacent edges.
#[derive(Default)]
pub struct HypergraphIter {
    /// Base edge for iteration.
    pub base: Edge,
    /// Index of incident vertex w.r.t. to base; initially -1; invalid if -2.
    pub vertexidx: i32,
    /// Incident vertex.
    pub minvertex: Vertex,
    /// Index of the adjacent edge w.r.t. to the vertex.
    pub edgeidx: i32,
    /// Adjacent edge.
    pub adjacent: Edge,
    /// Common vertices (C's `ncommonvertices` is -1 when the iterator
    /// vertex is not the minimum one).
    pub commonvertices: Vec<Vertex>,
    ncommonvertices: i32,
    /// Overlap set of the intersection, if available; -2 if disabled.
    pub overlap: Overlap,
    /// Minimum size of the overlap.
    pub minoverlapsize: u32,
    /// Whether to only consider edges greater than the base edge.
    pub onlylater: bool,
    /// Whether to compute the overlap sets.
    pub findoverlaps: bool,
}

impl HypergraphIter {
    /// `SCIPhypergraphIterInit`: initializes a hypergraph iterator's internal
    /// memory.
    pub fn new() -> Self {
        Self {
            base: -1,
            vertexidx: -1,
            minvertex: -1,
            edgeidx: -1,
            adjacent: -1,
            commonvertices: Vec::with_capacity(32),
            ncommonvertices: 0,
            overlap: 0,
            minoverlapsize: 2,
            onlylater: false,
            findoverlaps: false,
        }
    }

    /// `SCIPhypergraphIterStart`: initializes the iterator to the first
    /// adjacent edge of `base`.
    pub fn start<V: Default + Clone, E: Default + Clone, O: Default + Clone>(
        &mut self,
        hypergraph: &mut Hypergraph<V, E, O>,
        base: Edge,
        minoverlapsize: u32,
        onlylater: bool,
        findoverlaps: bool,
    ) {
        self.base = base;
        self.vertexidx = -1;
        self.edgeidx = -1;
        self.minvertex = -1;
        self.adjacent = -1;
        self.ncommonvertices = 0;
        self.commonvertices.clear();
        self.overlap = -1;
        self.minoverlapsize = minoverlapsize;
        self.onlylater = onlylater;
        self.findoverlaps = findoverlaps;

        self.next(hypergraph);
    }

    /// `SCIPhypergraphIterValid`: returns whether the iterator is valid.
    pub fn valid(&self) -> bool {
        self.vertexidx != -2
    }

    /// `SCIPhypergraphIterNext`: initializes the iterator to the first adjacent
    /// edge of `base`.
    pub fn next<V: Default + Clone, E: Default + Clone, O: Default + Clone>(&mut self, hypergraph: &mut Hypergraph<V, E, O>) {
        let base = self.base as usize;
        if self.vertexidx < 0 {
            // Freshly initialized iterator.
            self.vertexidx = hypergraph.edgesverticesbeg[base];
            if self.vertexidx == hypergraph.edgesverticesbeg[base + 1] {
                // No vertices in this edge.
                self.vertexidx = -2;
                return;
            }

            // Edge index is set to one before the first edge and incremented in
            // next loop.
            self.minvertex = hypergraph.edgesvertices[self.vertexidx as usize];
            self.edgeidx = hypergraph.verticesedgesbeg[self.minvertex as usize] - 1;
        }

        while self.vertexidx >= 0 {
            // Go to next edge of vertex.
            self.edgeidx += 1;
            if self.edgeidx < hypergraph.verticesedgesbeg[self.minvertex as usize + 1] {
                // There is a next edge.
                self.adjacent = hypergraph.verticesedges[self.edgeidx as usize];
            } else {
                // There is no next edge, so we consider the next vertex.
                self.vertexidx += 1;
                if self.vertexidx == hypergraph.edgesverticesbeg[base + 1] {
                    // There is no next vertex either.
                    self.vertexidx = -2;
                    return;
                }

                // Edge index is set to one before the first edge of the next
                // vertex and incremented in the next iteration.
                self.minvertex = hypergraph.edgesvertices[self.vertexidx as usize];
                self.edgeidx = hypergraph.verticesedgesbeg[self.minvertex as usize] - 1;
                continue;
            }

            // Skip if we consider the base edge.
            if self.adjacent == self.base {
                continue;
            }

            // Check for adjacent < base and discard if not desired.
            if self.onlylater && self.adjacent < self.base {
                continue;
            }

            // We traverse the common vertices.
            self.ncommonvertices = 0;
            self.commonvertices.clear();
            let mut i = hypergraph.edgesverticesbeg[base];
            let iend = hypergraph.edgesverticesbeg[base + 1];
            let mut j = hypergraph.edgesverticesbeg[self.adjacent as usize];
            let jend = hypergraph.edgesverticesbeg[self.adjacent as usize + 1];
            while i < iend && j < jend {
                let ivertex = hypergraph.edgesvertices[i as usize];
                let jvertex = hypergraph.edgesvertices[j as usize];

                if ivertex < jvertex {
                    i += 1;
                } else if ivertex > jvertex {
                    j += 1;
                } else {
                    // First common vertex must be the iterator vertex. Otherwise
                    // we abort.
                    if self.ncommonvertices == 0 && ivertex != self.minvertex {
                        self.ncommonvertices = -1;
                        break;
                    }
                    self.commonvertices.push(ivertex);
                    self.ncommonvertices += 1;
                    i += 1;
                    j += 1;
                }
            }

            if self.ncommonvertices < self.minoverlapsize as i32 {
                // Abort since either ncommonvertices = -1 (minvertex not minimum
                // vertex in intersection) or if it was smaller than requested.
                continue;
            }

            if self.findoverlaps {
                // If requested, find corresponding overlap set.
                self.overlap = if self.ncommonvertices >= 2 { hypergraph.overlap_find(&self.commonvertices) } else { -1 };
            }

            break;
        }
    }

    /// `SCIPhypergraphIterBase`: returns the base edge.
    pub fn base(&self) -> Edge {
        self.base
    }

    /// `SCIPhypergraphIterAdjacent`: returns the current adjacent edge.
    pub fn adjacent(&self) -> Edge {
        self.adjacent
    }

    /// `SCIPhypergraphIterMinVertex`: returns the minimum vertex in the
    /// intersection of the base and the current adjacent edge.
    pub fn min_vertex(&self) -> Vertex {
        self.minvertex
    }

    /// `SCIPhypergraphIterOverlap`: returns the overlap for the intersection of
    /// the base and the current adjacent edge.
    pub fn overlap(&self) -> Overlap {
        self.overlap
    }
}
