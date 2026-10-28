//! HiGHS's `HighsCliqueTable`: sets of binary literals of which at most one
//! may be true, with an inverted index from each literal to its cliques,
//! used to propagate fixings, find cliques to separate, partition the
//! objective's binaries, and substitute literals found to be equivalent.

use std::collections::BTreeSet;

use super::{
    BoundType, DomainChange, MipModel,
    domain::{Domain, DomainEnv, MipCtx, Reason},
    implications::Implications,
    node_queue::{Key, NodeQueue},
};
use crate::{
    hash_table::{HashKey, HashTable, hash_words},
    hash_tree::HashTree,
    pdqsort::{pdqsort, pdqsort_branchless},
    util::{CDouble, Random},
};

const INF: f64 = f64::INFINITY;
/// HiGHS's `kHighsIInf` as an origin: a clique not from a model row.
pub(crate) const NO_ORIGIN: i32 = i32::MAX;
/// HiGHS's -1 as an origin: a clique that was a model row's, now gone.
pub(crate) const ORIGIN_DELETED: i32 = -1;
/// A deleted clique's range.
const NONE: usize = usize::MAX;

/// HiGHS's `CliqueVar`: a column and the value (0 or 1) whose literal it
/// is; its bits are `col | val << 31`, as HiGHS's bit field lays them out.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CliqueVar(u32);

impl CliqueVar {
    pub fn new(col: usize, val: usize) -> Self {
        Self::from_parts(col as u32, val as u32)
    }

    pub fn from_parts(col: u32, val: u32) -> Self {
        Self((col & 0x7fff_ffff) | (val << 31))
    }

    /// The column, as HiGHS's 31-bit field holds it.
    pub fn col32(self) -> u32 {
        self.0 & 0x7fff_ffff
    }

    pub fn val(self) -> u32 {
        self.0 >> 31
    }

    pub fn index(self) -> usize {
        2 * self.col32() as usize + self.val() as usize
    }

    pub fn col(self) -> usize {
        self.col32() as usize
    }

    pub fn weight(self, sol: &[f64]) -> f64 {
        if self.val() != 0 { sol[self.col()] } else { 1.0 - sol[self.col()] }
    }

    pub fn complement(self) -> Self {
        Self(self.0 ^ (1 << 31))
    }

    fn bits(self) -> u32 {
        self.0
    }
}

impl PartialEq for CliqueVar {
    fn eq(&self, other: &Self) -> bool {
        self.index() == other.index()
    }
}

/// `std::pair<CliqueVar, CliqueVar>`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Edge(CliqueVar, CliqueVar);

impl HashKey for Edge {
    fn highs_hash(&self) -> u64 {
        hash_words(&[self.0.bits(), self.1.bits()])
    }
}

fn sorted_edge(v1: CliqueVar, v2: CliqueVar) -> Edge {
    if v1.col32() > v2.col32() { Edge(v2, v1) } else { Edge(v1, v2) }
}

/// HiGHS's `Clique`, in its 20 bytes: entry range, origin row, zero-fixed
/// count and whether it is an equation. [`NONE`] is stored as `u32::MAX`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Clique {
    start32: u32,
    end32: u32,
    pub origin: i32,
    num_zero_fixed32: u32,
    pub equality: bool,
}

const _: () = assert!(std::mem::size_of::<Clique>() == 20 && std::mem::size_of::<CliqueVar>() == 4);

/// A position or count as a clique holds it.
fn narrow(value: usize) -> u32 {
    if value == NONE {
        u32::MAX
    } else {
        u32::try_from(value).expect("a clique table index fits HiGHS's 32 bits")
    }
}

fn widen(value: u32) -> usize {
    if value == u32::MAX { NONE } else { value as usize }
}

impl Default for Clique {
    fn default() -> Self {
        Self {
            start32: u32::MAX,
            end32: u32::MAX,
            origin: NO_ORIGIN,
            num_zero_fixed32: 0,
            equality: false,
        }
    }
}

impl Clique {
    pub fn start(&self) -> usize {
        widen(self.start32)
    }

    pub fn end(&self) -> usize {
        widen(self.end32)
    }

    pub fn num_zero_fixed(&self) -> usize {
        self.num_zero_fixed32 as usize
    }

    pub fn set_start(&mut self, start: usize) {
        self.start32 = narrow(start);
    }

    pub fn set_end(&mut self, end: usize) {
        self.end32 = narrow(end);
    }

    pub fn set_num_zero_fixed(&mut self, count: usize) {
        self.num_zero_fixed32 = narrow(count);
    }
}

impl Clique {
    pub fn num_active(&self) -> usize {
        self.end() - self.start() - self.num_zero_fixed()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Substitution {
    pub substcol: usize,
    pub replace: CliqueVar,
}

/// HiGHS's `BronKerboschData`.
struct BronKerboschData<'a> {
    sol: &'a [f64],
    p: Vec<CliqueVar>,
    r: Vec<CliqueVar>,
    z: Vec<CliqueVar>,
    cliques: Vec<Vec<CliqueVar>>,
    neighbourhood_inds: Vec<usize>,
    w_r: f64,
    min_w: f64,
    feastol: f64,
    ncalls: i64,
    maxcalls: i64,
    maxcliques: i64,
    max_neighbourhood_queries: i64,
    pub num_neighbourhood_queries: i64,
}

impl<'a> BronKerboschData<'a> {
    fn new(sol: &'a [f64]) -> Self {
        Self {
            sol,
            p: Vec::new(),
            r: Vec::new(),
            z: Vec::new(),
            cliques: Vec::new(),
            neighbourhood_inds: Vec::new(),
            w_r: 0.0,
            min_w: 1.05,
            feastol: 1e-6,
            ncalls: 0,
            maxcalls: 10000,
            maxcliques: 100,
            max_neighbourhood_queries: i64::MAX,
            num_neighbourhood_queries: 0,
        }
    }

    fn stop(&self) -> bool {
        self.maxcalls == self.ncalls || self.cliques.len() as i64 == self.maxcliques || self.num_neighbourhood_queries > self.max_neighbourhood_queries
    }
}

/// What a clique table changes besides itself: the global domain (through
/// `cx`, whose node queue loses the nodes a new clique rules out), and the
/// implications.
pub(crate) struct CliqueCtx<'e, 'a> {
    pub cx: &'e mut MipCtx<'a>,
    pub implications: &'e mut Implications,
}

#[derive(Clone, Debug)]
pub(crate) struct CliqueTable {
    cliqueentries: Vec<CliqueVar>,
    inverted_hash_list: Vec<HashTree<usize, usize>>,
    inverted_hash_list_size_two: Vec<HashTree<usize, ()>>,
    size_two_cliques: HashTable<Edge, usize>,
    /// Free entry storage as `(length, start)`.
    freespaces: BTreeSet<(usize, usize)>,
    freeslots: Vec<usize>,
    cliques: Vec<Clique>,
    numcliquesvar: Vec<usize>,
    infeasvertexstack: Vec<CliqueVar>,
    /// One past the index of a column's substitution, or 0.
    colsubstituted: Vec<usize>,
    substitutions: Vec<Substitution>,
    deletedrows: Vec<i32>,
    cliqueextensions: Vec<(i32, CliqueVar)>,
    iscandidate: Vec<bool>,
    col_deleted: Vec<bool>,
    cliquehits: Vec<u32>,
    cliquehitinds: Vec<usize>,
    randgen: Random,
    nfixings: usize,
    num_entries: usize,
    max_entries: usize,
    in_presolve: bool,
    pub num_neighbourhood_queries: i64,
}

impl Default for CliqueTable {
    fn default() -> Self {
        Self::new(0)
    }
}

impl CliqueTable {
    pub fn new(ncols: usize) -> Self {
        Self {
            cliqueentries: Vec::new(),
            inverted_hash_list: vec![HashTree::new(); 2 * ncols],
            inverted_hash_list_size_two: vec![HashTree::new(); 2 * ncols],
            size_two_cliques: HashTable::new(),
            freespaces: BTreeSet::new(),
            freeslots: Vec::new(),
            cliques: Vec::new(),
            numcliquesvar: vec![0; 2 * ncols],
            infeasvertexstack: Vec::new(),
            colsubstituted: vec![0; ncols],
            substitutions: Vec::new(),
            deletedrows: Vec::new(),
            cliqueextensions: Vec::new(),
            iscandidate: Vec::new(),
            col_deleted: vec![false; ncols],
            cliquehits: Vec::new(),
            cliquehitinds: Vec::new(),
            randgen: Random::new(0),
            nfixings: 0,
            num_entries: 0,
            max_entries: i32::MAX as usize,
            in_presolve: false,
            num_neighbourhood_queries: 0,
        }
    }

    pub fn set_presolve_flag(&mut self, in_presolve: bool) {
        self.in_presolve = in_presolve;
    }

    pub fn presolve_flag(&self) -> bool {
        self.in_presolve
    }

    pub fn num_entries(&self) -> usize {
        self.num_entries
    }

    pub fn randgen(&mut self) -> &mut Random {
        &mut self.randgen
    }

    pub fn deleted_rows(&mut self) -> &mut Vec<i32> {
        &mut self.deletedrows
    }

    pub fn clear_substitutions(&mut self) {
        self.substitutions.clear();
    }

    pub fn substitutions(&self) -> &[Substitution] {
        &self.substitutions
    }

    pub fn substitution(&self, col: usize) -> Option<&Substitution> {
        (self.colsubstituted[col] != 0).then(|| &self.substitutions[self.colsubstituted[col] - 1])
    }

    pub fn clique_extensions(&mut self) -> &mut Vec<(i32, CliqueVar)> {
        &mut self.cliqueextensions
    }

    pub fn set_max_entries(&mut self, num_nz: usize) {
        self.max_entries = 2_000_000 + 10 * num_nz;
    }

    pub fn is_full(&self) -> bool {
        self.num_entries >= self.max_entries
    }

    pub fn num_fixings(&self) -> usize {
        self.nfixings
    }

    pub fn num_cliques(&self) -> usize {
        self.cliques.len() - self.freeslots.len()
    }

    pub fn num_cliques_of(&self, v: CliqueVar) -> usize {
        self.numcliquesvar[v.index()]
    }

    pub fn num_cliques_col(&self, col: usize, val: bool) -> usize {
        self.numcliquesvar[CliqueVar::new(col, usize::from(val)).index()]
    }

    fn unlink(&mut self, pos: usize, cliqueid: usize) {
        let index = self.cliqueentries[pos].index();
        self.numcliquesvar[index] -= 1;
        let cliquelen = self.cliques[cliqueid].end() - self.cliques[cliqueid].start();
        if cliquelen == 2 {
            self.inverted_hash_list_size_two[index].erase(&cliqueid);
        } else {
            self.inverted_hash_list[index].erase(&cliqueid);
        }
    }

    fn link(&mut self, pos: usize, cliqueid: usize) {
        let index = self.cliqueentries[pos].index();
        self.numcliquesvar[index] += 1;
        let cliquelen = self.cliques[cliqueid].end() - self.cliques[cliqueid].start();
        if cliquelen == 2 {
            self.inverted_hash_list_size_two[index].insert(cliqueid, ());
        } else {
            self.inverted_hash_list[index].insert(cliqueid, pos);
        }
    }

    /// HiGHS's `findCommonCliqueId`.
    fn find_common_clique_id(&self, num_queries: &mut i64, v1: CliqueVar, v2: CliqueVar) -> Option<usize> {
        *num_queries += 1;
        if !self.inverted_hash_list_size_two[v1.index()].is_empty()
            && !self.inverted_hash_list_size_two[v2.index()].is_empty()
            && let Some(&id) = self.size_two_cliques.find(&sorted_edge(v1, v2))
        {
            return Some(id);
        }
        self.inverted_hash_list[v1.index()].find_common(&self.inverted_hash_list[v2.index()]).map(|entry| entry.0)
    }

    pub fn have_common_clique(&self, num_queries: &mut i64, v1: CliqueVar, v2: CliqueVar) -> bool {
        if v1.col32() == v2.col32() {
            return false;
        }
        self.find_common_clique_id(num_queries, v1, v2).is_some()
    }

    pub fn have_common_clique_counted(&mut self, v1: CliqueVar, v2: CliqueVar) -> bool {
        let mut queries = self.num_neighbourhood_queries;
        let common = self.have_common_clique(&mut queries, v1, v2);
        self.num_neighbourhood_queries = queries;
        common
    }

    /// HiGHS's `findCommonClique`: a clique holding both, if any.
    pub fn find_common_clique(&mut self, v1: CliqueVar, v2: CliqueVar) -> Option<&[CliqueVar]> {
        if v1 == v2 {
            return None;
        }
        let mut queries = self.num_neighbourhood_queries;
        let clq = self.find_common_clique_id(&mut queries, v1, v2);
        self.num_neighbourhood_queries = queries;
        let clq = clq?;
        Some(&self.cliqueentries[self.cliques[clq].start()..self.cliques[clq].end()])
    }

    pub fn resolve_substitution(&self, v: &mut CliqueVar) {
        while self.colsubstituted[v.col()] != 0 {
            let subst = self.substitutions[self.colsubstituted[v.col()] - 1];
            *v = if v.val() == 1 { subst.replace } else { subst.replace.complement() };
        }
    }

    /// HiGHS's `resolveSubstitution(col, val, offset)` for a row entry.
    pub fn resolve_substitution_entry(&self, col: &mut usize, val: &mut f64, offset: &mut f64) {
        while self.colsubstituted[*col] != 0 {
            let subst = self.substitutions[self.colsubstituted[*col] - 1];
            if subst.replace.val() == 0 {
                *offset += *val;
                *val = -*val;
            }
            *col = subst.replace.col();
        }
    }

    /// HiGHS's `queryNeighbourhood` (serial): the positions in `q` of the
    /// literals sharing a clique with `v`.
    fn query_neighbourhood(&self, neighbourhood_inds: &mut Vec<usize>, num_queries: &mut i64, v: CliqueVar, q: &[CliqueVar]) {
        neighbourhood_inds.clear();
        if self.num_cliques_of(v) == 0 {
            return;
        }
        for (i, &qi) in q.iter().enumerate() {
            if self.have_common_clique(num_queries, v, qi) {
                neighbourhood_inds.push(i);
            }
        }
    }

    /// HiGHS's `partitionNeighbourhood`: `v`'s neighbours in `q` swapped to
    /// its front; returns how many.
    pub fn partition_neighbourhood(&self, neighbourhood_inds: &mut Vec<usize>, num_queries: &mut i64, v: CliqueVar, q: &mut [CliqueVar]) -> usize {
        self.query_neighbourhood(neighbourhood_inds, num_queries, v, q);
        for (i, &at) in neighbourhood_inds.iter().enumerate() {
            q.swap(i, at);
        }
        neighbourhood_inds.len()
    }

    /// HiGHS's `shrinkToNeighbourhood`: `q` cut down to `v`'s neighbours,
    /// in order; returns how many.
    pub fn shrink_to_neighbourhood(&self, neighbourhood_inds: &mut Vec<usize>, num_queries: &mut i64, v: CliqueVar, q: &mut [CliqueVar]) -> usize {
        self.query_neighbourhood(neighbourhood_inds, num_queries, v, q);
        for (i, &at) in neighbourhood_inds.iter().enumerate() {
            q[i] = q[at];
        }
        neighbourhood_inds.len()
    }

    // ---- changing the global domain -------------------------------------

    /// Run `f` on the global domain with this table and the implications
    /// lent to it.
    fn with_env<R>(&self, globaldom: &mut Domain, cc: &mut CliqueCtx, f: impl FnOnce(&mut Domain, &mut DomainEnv) -> R) -> R {
        let mut env = DomainEnv {
            cx: &mut *cc.cx,
            cliquetable: self,
            implications: &*cc.implications,
        };
        f(globaldom, &mut env)
    }

    /// HiGHS's `fixCol`: fix `v` to false; returns false if that made the
    /// domain infeasible.
    pub fn fix_col(&mut self, globaldom: &mut Domain, v: CliqueVar, do_process_infeasible_vertices: bool, cc: &mut CliqueCtx) -> bool {
        let wasfixed = globaldom.is_fixed(v.col());
        self.with_env(globaldom, cc, |dom, env| dom.fix_col(v.col(), (1 - v.val()) as f64, Reason::unspecified(), env));
        if globaldom.infeasible() {
            return false;
        }
        if !wasfixed {
            self.nfixings += 1;
            self.infeasvertexstack.push(v);
            if do_process_infeasible_vertices {
                self.process_infeasible_vertices(globaldom, cc);
            }
        }
        true
    }

    /// HiGHS's `processNewEdge`: what a new edge between `v1` and `v2`
    /// implies. Returns whether it closes a cover.
    pub fn process_new_edge(&mut self, globaldom: &mut Domain, mut v1: CliqueVar, mut v2: CliqueVar, cc: &mut CliqueCtx) -> bool {
        if v1.col32() == v2.col32() {
            if v1.val() == v2.val() {
                self.fix_col(globaldom, v1, true, cc);
                return false;
            }
            return true;
        }
        if self.have_common_clique_counted(v1.complement(), v2) {
            self.fix_col(globaldom, v2, true, cc);
            return false;
        }
        if self.have_common_clique_counted(v2.complement(), v1) {
            self.fix_col(globaldom, v1, true, cc);
            return false;
        }
        if !self.found_cover(globaldom, v1.complement(), v2.complement(), cc) {
            return false;
        }
        if globaldom.infeasible() {
            return true;
        }
        self.found_cover(globaldom, v1, v2, cc);
        if globaldom.is_fixed(v1.col()) || globaldom.is_fixed(v2.col()) || globaldom.infeasible() {
            return true;
        }
        let substitution = if v2.col32() < v1.col32() {
            if v1.val() == 1 {
                v2 = v2.complement();
            }
            Substitution { substcol: v1.col(), replace: v2 }
        } else {
            if v2.val() == 1 {
                v1 = v1.complement();
            }
            Substitution { substcol: v2.col(), replace: v1 }
        };
        self.substitutions.push(substitution);
        self.colsubstituted[substitution.substcol] = self.substitutions.len();
        self.replace_literal(CliqueVar::new(substitution.substcol, 1), substitution.replace);
        self.replace_literal(CliqueVar::new(substitution.substcol, 0), substitution.replace.complement());
        true
    }

    /// `processNewEdge`'s `replace`: move `substituted`'s cliques to
    /// `replacement`.
    fn replace_literal(&mut self, substituted: CliqueVar, replacement: CliqueVar) {
        self.numcliquesvar[replacement.index()] += self.numcliquesvar[substituted.index()];
        self.numcliquesvar[substituted.index()] = 0;
        let subst_list = std::mem::take(&mut self.inverted_hash_list[substituted.index()]);
        subst_list.for_each(|&cliqueid, &location| {
            self.inverted_hash_list[replacement.index()].insert(cliqueid, location);
            self.cliqueentries[location] = replacement;
        });
        let subst_list_size_two = std::mem::take(&mut self.inverted_hash_list_size_two[substituted.index()]);
        subst_list_size_two.for_each(|&cliqueid, _| {
            let mut pos = self.cliques[cliqueid].start();
            let mut other_pos = pos + 1;
            if self.cliqueentries[other_pos] == substituted {
                std::mem::swap(&mut pos, &mut other_pos);
            }
            self.inverted_hash_list_size_two[replacement.index()].insert(cliqueid, ());
            self.cliqueentries[pos] = replacement;
            let other = self.cliqueentries[other_pos];
            self.size_two_cliques.erase(&sorted_edge(substituted, other));
            self.size_two_cliques.insert(sorted_edge(replacement, other), cliqueid);
        });
    }

    /// HiGHS's `doAddClique`: store a clique, dropping literals that make
    /// it trivial and queueing those it shows infeasible.
    pub fn do_add_clique(&mut self, cliquevars: &[CliqueVar], equality: bool, origin: i32) {
        let numcliquevars = cliquevars.len();
        let cliqueid = match self.freeslots.pop() {
            None => {
                self.cliques.push(Clique::default());
                self.cliques.len() - 1
            }
            Some(id) => id,
        };
        self.cliques[cliqueid].equality = equality;
        self.cliques[cliqueid].origin = origin;
        let max_end;
        match self.freespaces.range((numcliquevars, 0)..).next().copied() {
            None => {
                self.cliques[cliqueid].set_start(self.cliqueentries.len());
                let value = self.cliques[cliqueid].start() + numcliquevars;
                self.cliques[cliqueid].set_end(value);
                max_end = self.cliques[cliqueid].end();
                self.cliqueentries.resize(max_end, CliqueVar::default());
            }
            Some(freespace) => {
                self.freespaces.remove(&freespace);
                self.cliques[cliqueid].set_start(freespace.1);
                self.cliques[cliqueid].set_end(freespace.1 + numcliquevars);
                max_end = freespace.1 + freespace.0;
            }
        }
        self.cliques[cliqueid].set_num_zero_fixed(0);
        let mut fixtozero = false;
        let mut k = self.cliques[cliqueid].start();
        for &var in cliquevars {
            let mut v = var;
            self.resolve_substitution(&mut v);
            if fixtozero {
                self.infeasvertexstack.push(v);
                continue;
            }
            let clq_has_v_compl = if numcliquevars == 2 {
                self.inverted_hash_list_size_two[v.complement().index()].contains(&cliqueid)
            } else {
                self.inverted_hash_list[v.complement().index()].contains(&cliqueid)
            };
            if clq_has_v_compl {
                fixtozero = true;
                for j in self.cliques[cliqueid].start()..k {
                    if self.cliqueentries[j].col32() != v.col32() {
                        self.infeasvertexstack.push(self.cliqueentries[j]);
                    }
                    self.unlink(j, cliqueid);
                }
                k = self.cliques[cliqueid].start();
                continue;
            }
            let inserted = if numcliquevars == 2 {
                self.inverted_hash_list_size_two[v.index()].insert(cliqueid, ())
            } else {
                self.inverted_hash_list[v.index()].insert(cliqueid, k)
            };
            if !inserted {
                self.infeasvertexstack.push(v);
                continue;
            }
            self.cliqueentries[k] = v;
            self.numcliquesvar[v.index()] += 1;
            k += 1;
        }
        if max_end > k {
            if self.cliqueentries.len() == max_end {
                self.cliqueentries.truncate(k);
            } else {
                self.freespaces.insert((max_end - k, k));
            }
            if self.cliques[cliqueid].end() > k {
                match k - self.cliques[cliqueid].start() {
                    0 => {
                        self.cliques[cliqueid].set_start(NONE);
                        self.cliques[cliqueid].set_end(NONE);
                        self.freeslots.push(cliqueid);
                        return;
                    }
                    1 => {
                        let start = self.cliques[cliqueid].start();
                        self.unlink(start, cliqueid);
                        self.cliques[cliqueid].set_start(NONE);
                        self.cliques[cliqueid].set_end(NONE);
                        self.freeslots.push(cliqueid);
                        return;
                    }
                    2 => {
                        let start = self.cliques[cliqueid].start();
                        self.unlink(start, cliqueid);
                        self.unlink(start + 1, cliqueid);
                        self.cliques[cliqueid].set_end(k);
                        self.link(start, cliqueid);
                        self.link(start + 1, cliqueid);
                    }
                    _ => self.cliques[cliqueid].set_end(k),
                }
            }
        }
        let clique_len = self.cliques[cliqueid].end() - self.cliques[cliqueid].start();
        self.num_entries += clique_len;
        if clique_len == 2 {
            let start = self.cliques[cliqueid].start();
            self.size_two_cliques
                .insert(sorted_edge(self.cliqueentries[start], self.cliqueentries[start + 1]), cliqueid);
        }
    }

    /// HiGHS's `addClique`: add the clique `cliquevars` (reordered), unless
    /// what it implies fixes everything it says.
    pub fn add_clique(&mut self, model: &MipModel, globaldom: &mut Domain, cliquevars: &mut [CliqueVar], equality: bool, origin: i32, cc: &mut CliqueCtx) {
        for v in cliquevars.iter_mut() {
            let mut resolved = *v;
            self.resolve_substitution(&mut resolved);
            *v = resolved;
        }
        let mut has_new_edge = false;
        if self.check_clique(model, globaldom, cliquevars, &mut has_new_edge, false, cc) {
            return;
        }
        if globaldom.infeasible() {
            return;
        }
        if cliquevars.len() == 2 && equality {
            if self.check_clique(model, globaldom, cliquevars, &mut has_new_edge, true, cc) {
                return;
            }
            if globaldom.infeasible() {
                return;
            }
        }
        if !has_new_edge && origin == NO_ORIGIN {
            return;
        }
        let mut unfixed: Vec<CliqueVar> = cliquevars.to_vec();
        unfixed.retain(|v| !globaldom.is_fixed(v.col()));
        if unfixed.len() < 2 {
            return;
        }
        self.do_add_clique(&unfixed, equality, origin);
        self.process_infeasible_vertices(globaldom, cc);
    }

    /// `addClique`'s `checkClique`, complementing the clique around
    /// `fixAllVarsInClique` when asked.
    fn check_clique(&mut self, model: &MipModel, globaldom: &mut Domain, cliquevars: &mut [CliqueVar], has_new_edge: &mut bool, complement: bool, cc: &mut CliqueCtx) -> bool {
        if complement {
            cliquevars.iter_mut().for_each(|v| *v = v.complement());
        }
        let done = self.fix_all_vars_in_clique(model, globaldom, cliquevars, has_new_edge, cc);
        if complement {
            cliquevars.iter_mut().for_each(|v| *v = v.complement());
        }
        done
    }

    /// `addClique`'s `fixAllVarsInClique`.
    fn fix_all_vars_in_clique(&mut self, _model: &MipModel, globaldom: &mut Domain, cliquevars: &[CliqueVar], has_new_edge: &mut bool, cc: &mut CliqueCtx) -> bool {
        const MAX_NUM_CLIQUE_VARS: usize = 100;
        let numcliquevars = cliquevars.len();
        for i in 0..numcliquevars {
            if !globaldom.is_fixed(cliquevars[i].col()) || cliquevars[i].val() as f64 != globaldom.col_lower[cliquevars[i].col()] {
                continue;
            }
            for (k, cliquevar) in cliquevars[..numcliquevars].iter().enumerate() {
                if k == i {
                    continue;
                }
                if !self.fix_col(globaldom, *cliquevar, false, cc) {
                    return false;
                }
            }
            self.process_infeasible_vertices(globaldom, cc);
            return true;
        }
        if numcliquevars > MAX_NUM_CLIQUE_VARS {
            return false;
        }
        for i in 0..numcliquevars.saturating_sub(1) {
            if globaldom.is_fixed(cliquevars[i].col()) {
                continue;
            }
            if self.num_cliques_of(cliquevars[i]) == 0 && self.num_cliques_of(cliquevars[i].complement()) == 0 {
                *has_new_edge = true;
                continue;
            }
            for j in i + 1..numcliquevars {
                if globaldom.is_fixed(cliquevars[j].col()) {
                    continue;
                }
                if self.have_common_clique_counted(cliquevars[i], cliquevars[j]) {
                    continue;
                }
                *has_new_edge = true;
                let iscover = self.process_new_edge(globaldom, cliquevars[i], cliquevars[j], cc);
                if globaldom.infeasible() {
                    return false;
                }
                self.prune_nodes_with_both(cliquevars[i], cliquevars[j], cc);
                if iscover {
                    for (k, cliquevar) in cliquevars[..numcliquevars].iter().enumerate() {
                        if k == i || k == j {
                            continue;
                        }
                        if !self.fix_col(globaldom, *cliquevar, false, cc) {
                            return false;
                        }
                    }
                    self.process_infeasible_vertices(globaldom, cc);
                    return true;
                }
            }
        }
        false
    }

    /// Prune the open nodes that set both literals true, which the new edge
    /// rules out.
    fn prune_nodes_with_both(&mut self, v1: CliqueVar, v2: CliqueVar, cc: &mut CliqueCtx) {
        let Some(nodequeue) = cc.cx.nodequeue else { return };
        let mut nodequeue = nodequeue.borrow_mut();
        if nodequeue.is_empty() {
            return;
        }
        let nodes_of = |queue: &NodeQueue, v: CliqueVar| -> Vec<usize> {
            let set = if v.val() == 1 { queue.up_nodes(v.col()) } else { queue.down_nodes(v.col()) };
            // HiGHS's `lower_bound((val, IInf))` to `upper_bound((val,
            // IInf))`: only a node of value `val` and index `IInf` would
            // fall inside, so the range is empty.
            let key = Key(v.val() as f64);
            set.range((key, i32::MAX as usize)..=(key, i32::MAX as usize)).map(|&(_, node)| node).collect()
        };
        let v1_nodes = nodes_of(&nodequeue, v1);
        let v2_nodes = nodes_of(&nodequeue, v2);
        if v1_nodes.is_empty() || v2_nodes.is_empty() {
            return;
        }
        if !(v1_nodes[0] <= *v2_nodes.last().expect("nodes") || v2_nodes[0] <= *v1_nodes.last().expect("nodes")) {
            return;
        }
        let (mut i1, mut i2) = (0, 0);
        while i1 < v1_nodes.len() && i2 < v2_nodes.len() {
            if v1_nodes[i1] < v2_nodes[i2] {
                i1 += 1;
            } else if v2_nodes[i2] < v1_nodes[i1] {
                i2 += 1;
            } else {
                let pruned = v2_nodes[i2];
                i1 += 1;
                i2 += 1;
                let weight = nodequeue.prune_node(pruned);
                if let Some(treeweight) = cc.cx.pruned_treeweight {
                    treeweight.set(treeweight.get() + weight);
                }
            }
        }
    }

    pub fn remove_clique(&mut self, cliqueid: usize) {
        let origin = self.cliques[cliqueid].origin;
        if origin != NO_ORIGIN && origin != ORIGIN_DELETED {
            self.deletedrows.push(origin);
        }
        let start = self.cliques[cliqueid].start();
        let end = self.cliques[cliqueid].end();
        let len = end - start;
        if len == 2 {
            self.size_two_cliques.erase(&sorted_edge(self.cliqueentries[start], self.cliqueentries[start + 1]));
        }
        for i in start..end {
            self.unlink(i, cliqueid);
        }
        self.freeslots.push(cliqueid);
        self.freespaces.insert((len, start));
        self.cliques[cliqueid].set_start(NONE);
        self.cliques[cliqueid].set_end(NONE);
        self.num_entries -= len;
    }

    /// HiGHS's `foundCover`: `v1` and `v2` share cliques; fix the rest of
    /// each such clique and drop it. Returns whether they shared one.
    pub fn found_cover(&mut self, globaldom: &mut Domain, v1: CliqueVar, v2: CliqueVar, cc: &mut CliqueCtx) -> bool {
        let mut queries = self.num_neighbourhood_queries;
        let mut commonclique = self.find_common_clique_id(&mut queries, v1, v2);
        self.num_neighbourhood_queries = queries;
        if commonclique.is_none() {
            return false;
        }
        while let Some(clq) = commonclique {
            let (start, end) = (self.cliques[clq].start(), self.cliques[clq].end());
            for i in start..end {
                let entry = self.cliqueentries[i];
                if entry == v1 || entry == v2 {
                    continue;
                }
                if !self.fix_col(globaldom, entry, false, cc) {
                    return true;
                }
            }
            self.remove_clique(clq);
            let mut queries = self.num_neighbourhood_queries;
            commonclique = self.find_common_clique_id(&mut queries, v1, v2);
            self.num_neighbourhood_queries = queries;
        }
        self.process_infeasible_vertices(globaldom, cc);
        true
    }

    /// HiGHS's `processInfeasibleVertices`: fix the literals shown false
    /// and everything their cliques then imply.
    pub fn process_infeasible_vertices(&mut self, globaldom: &mut Domain, cc: &mut CliqueCtx) {
        while let Some(&top) = self.infeasvertexstack.last() {
            if globaldom.infeasible() {
                break;
            }
            self.infeasvertexstack.pop();
            let mut v = top.complement();
            self.resolve_substitution(&mut v);
            let wasfixed = globaldom.is_fixed(v.col());
            self.with_env(globaldom, cc, |dom, env| dom.fix_col(v.col(), v.val() as f64, Reason::unspecified(), env));
            if globaldom.infeasible() {
                return;
            }
            if !wasfixed {
                self.nfixings += 1;
            }
            if self.col_deleted[v.col()] {
                continue;
            }
            self.col_deleted[v.col()] = true;
            let v_hash_lists = std::mem::take(&mut self.inverted_hash_list[v.index()]);
            let v_hash_lists_size_two = std::mem::take(&mut self.inverted_hash_list_size_two[v.index()]);
            let infeas = v_hash_lists
                .for_each_until(|&cliqueid, _| {
                    if !self.fix_clique_except(globaldom, cliqueid, v.col32(), cc) {
                        return Some(());
                    }
                    self.remove_clique(cliqueid);
                    None
                })
                .is_some();
            if infeas {
                return;
            }
            let infeas = v_hash_lists_size_two
                .for_each_until(|&cliqueid, _| {
                    if !self.fix_clique_except(globaldom, cliqueid, v.col32(), cc) {
                        return Some(());
                    }
                    self.remove_clique(cliqueid);
                    None
                })
                .is_some();
            if infeas {
                return;
            }
            let v_hash_lists = std::mem::take(&mut self.inverted_hash_list[v.complement().index()]);
            let v_hash_lists_size_two = std::mem::take(&mut self.inverted_hash_list_size_two[v.complement().index()]);
            if self.in_presolve {
                v_hash_lists.for_each(|&cliqueid, _| {
                    let value = self.cliques[cliqueid].num_zero_fixed() + 1;
                    self.cliques[cliqueid].set_num_zero_fixed(value);
                    if self.cliques[cliqueid].num_active() <= 1 {
                        self.fix_last_active_and_remove(globaldom, cliqueid, cc);
                    }
                });
                continue;
            }
            v_hash_lists_size_two.for_each(|&cliqueid, _| self.remove_clique(cliqueid));
            v_hash_lists.for_each(|&cliqueid, _| {
                let value = self.cliques[cliqueid].num_zero_fixed() + 1;
                self.cliques[cliqueid].set_num_zero_fixed(value);
                let clique = self.cliques[cliqueid];
                if clique.num_active() <= 1 {
                    self.fix_last_active_and_remove(globaldom, cliqueid, cc);
                } else if clique.num_zero_fixed() >= 10usize.max((clique.end() - clique.start()) >> 1) {
                    let mut clq: Vec<CliqueVar> = self.cliqueentries[clique.start()..clique.end()].to_vec();
                    self.remove_clique(cliqueid);
                    clq.retain(|x| !(globaldom.is_fixed(x.col()) && globaldom.col_lower[x.col()] == (1 - x.val()) as f64));
                    if clq.len() > 1 {
                        self.do_add_clique(&clq, false, NO_ORIGIN);
                    }
                }
            });
        }
        self.propagate_and_cleanup(globaldom, cc);
    }

    /// HiGHS's `fixLastActiveAndRemove` (#3220): an equality clique with one
    /// literal left forces it to one before the clique goes.
    fn fix_last_active_and_remove(&mut self, globaldom: &mut Domain, cliqueid: usize, cc: &mut CliqueCtx) {
        let clique = self.cliques[cliqueid];
        if clique.equality && clique.num_active() == 1 {
            for i in clique.start()..clique.end() {
                let entry = self.cliqueentries[i];
                if !globaldom.is_fixed(entry.col()) {
                    self.fix_col(globaldom, entry.complement(), false, cc);
                    break;
                }
            }
        }
        self.remove_clique(cliqueid);
    }

    /// Fix every literal of clique `cliqueid` but column `col`'s; returns
    /// false if that made the domain infeasible.
    fn fix_clique_except(&mut self, globaldom: &mut Domain, cliqueid: usize, col: u32, cc: &mut CliqueCtx) -> bool {
        let (start, end) = (self.cliques[cliqueid].start(), self.cliques[cliqueid].end());
        for i in start..end {
            let entry = self.cliqueentries[i];
            if entry.col32() == col {
                continue;
            }
            if !self.fix_col(globaldom, entry, false, cc) {
                return false;
            }
        }
        true
    }

    /// HiGHS's `propagateAndCleanup`.
    pub fn propagate_and_cleanup(&mut self, globaldom: &mut Domain, cc: &mut CliqueCtx) {
        let mut start = globaldom.domain_change_stack().len();
        self.with_env(globaldom, cc, |dom, env| dom.propagate(env));
        let mut end = globaldom.domain_change_stack().len();
        while !globaldom.infeasible() && start != end {
            for k in start..end {
                let col = globaldom.domain_change_stack()[k].column;
                if !globaldom.is_fixed(col) {
                    continue;
                }
                if globaldom.col_lower[col] != 1.0 && globaldom.col_lower[col] != 0.0 {
                    continue;
                }
                let fixval = globaldom.col_lower[col] as usize;
                let v = CliqueVar::new(col, 1 - fixval);
                if self.num_cliques_of(v) != 0 {
                    self.vertex_infeasible(globaldom, col, 1 - fixval, cc);
                    if globaldom.infeasible() {
                        return;
                    }
                }
            }
            start = globaldom.domain_change_stack().len();
            self.with_env(globaldom, cc, |dom, env| dom.propagate(env));
            end = globaldom.domain_change_stack().len();
        }
    }

    /// HiGHS's `vertexInfeasible`: literal `(col, val)` cannot be true.
    pub fn vertex_infeasible(&mut self, globaldom: &mut Domain, col: usize, val: usize, cc: &mut CliqueCtx) {
        let wasfixed = globaldom.is_fixed(col);
        self.with_env(globaldom, cc, |dom, env| dom.fix_col(col, (1 - val) as f64, Reason::unspecified(), env));
        if globaldom.infeasible() {
            return;
        }
        if !wasfixed {
            self.nfixings += 1;
        }
        self.infeasvertexstack.push(CliqueVar::new(col, val));
        self.process_infeasible_vertices(globaldom, cc);
    }

    /// HiGHS's `cleanupFixed`.
    pub fn cleanup_fixed(&mut self, globaldom: &mut Domain, cc: &mut CliqueCtx) {
        let numcol = globaldom.col_upper.len();
        let oldnfixings = self.nfixings;
        for i in 0..numcol {
            if self.col_deleted[i] || !globaldom.is_fixed(i) {
                continue;
            }
            if globaldom.col_lower[i] != 1.0 && globaldom.col_lower[i] != 0.0 {
                continue;
            }
            let fixval = globaldom.col_lower[i] as usize;
            self.vertex_infeasible(globaldom, i, 1 - fixval, cc);
            if globaldom.infeasible() {
                return;
            }
        }
        if self.nfixings != oldnfixings {
            self.propagate_and_cleanup(globaldom, cc);
        }
    }

    /// HiGHS's `addImplications`: once binary `col` is fixed to `val`, the
    /// literals sharing a clique with it are false.
    pub fn add_implications(&self, domain: &mut Domain, col: usize, val: usize, env: &mut DomainEnv) {
        let mut v = CliqueVar::new(col, val);
        while self.colsubstituted[v.col()] != 0 {
            let subst = self.substitutions[self.colsubstituted[v.col()] - 1];
            v = if v.val() == 1 { subst.replace } else { subst.replace.complement() };
            if v.val() == 1 {
                if domain.col_lower[v.col()] == 1.0 {
                    continue;
                }
                domain.change_bound(DomainChange::new(1.0, v.col(), BoundType::Lower), Reason::clique_table(col, val), env);
                if domain.infeasible() {
                    return;
                }
            } else {
                if domain.col_upper[v.col()] == 0.0 {
                    continue;
                }
                domain.change_bound(DomainChange::new(0.0, v.col(), BoundType::Upper), Reason::clique_table(col, val), env);
                if domain.infeasible() {
                    return;
                }
            }
        }
        let do_fixings = |cliqueid: usize, domain: &mut Domain, env: &mut DomainEnv| -> bool {
            let (start, end) = (self.cliques[cliqueid].start(), self.cliques[cliqueid].end());
            for cliqueentrie in &self.cliqueentries[start..end] {
                let entry = *cliqueentrie;
                if entry.col32() == v.col32() {
                    continue;
                }
                if entry.val() == 1 {
                    if domain.col_upper[entry.col()] == 0.0 {
                        continue;
                    }
                    domain.change_bound(DomainChange::new(0.0, entry.col(), BoundType::Upper), Reason::clique_table(col, val), env);
                    if domain.infeasible() {
                        return true;
                    }
                } else {
                    if domain.col_lower[entry.col()] == 1.0 {
                        continue;
                    }
                    domain.change_bound(DomainChange::new(1.0, entry.col(), BoundType::Lower), Reason::clique_table(col, val), env);
                    if domain.infeasible() {
                        return true;
                    }
                }
            }
            false
        };
        let infeas = self.inverted_hash_list[v.index()]
            .for_each_until(|&cliqueid, _| do_fixings(cliqueid, domain, env).then_some(()))
            .is_some();
        if infeas {
            return;
        }
        self.inverted_hash_list_size_two[v.index()].for_each_until(|&cliqueid, _| do_fixings(cliqueid, domain, env).then_some(()));
    }

    pub fn num_implications(&self, col: usize) -> usize {
        let i0 = CliqueVar::new(col, 0).index();
        let i1 = CliqueVar::new(col, 1).index();
        let mut numimplics = (self.numcliquesvar[i0] + self.numcliquesvar[i1]) as isize;
        let mut count = |cliqueid: &usize, _: &usize| {
            let clique = &self.cliques[*cliqueid];
            let nimplics = (clique.end() - clique.start() - 1) as isize * (1 + isize::from(clique.equality));
            numimplics += nimplics - 1;
        };
        self.inverted_hash_list[i0].for_each(&mut count);
        self.inverted_hash_list[i1].for_each(&mut count);
        numimplics as usize
    }

    pub fn num_implications_val(&self, col: usize, val: bool) -> usize {
        let i_val = CliqueVar::new(col, usize::from(val)).index();
        let mut numimplics = self.numcliquesvar[i_val] as isize;
        self.inverted_hash_list[i_val].for_each(|cliqueid, _| {
            let clique = &self.cliques[*cliqueid];
            let nimplics = (clique.end() - clique.start() - 1) as isize * (1 + isize::from(clique.equality));
            numimplics += nimplics - 1;
        });
        numimplics as usize
    }

    // ---- partitions and Bron-Kerbosch ------------------------------------

    /// HiGHS's `cliquePartition(clqVars, partitionStart)`: shuffle, then
    /// grow cliques greedily.
    pub fn clique_partition(&mut self, clq_vars: &mut [CliqueVar], partition_start: &mut Vec<usize>) {
        self.randgen.shuffle(clq_vars);
        let mut neighbourhood_inds = Vec::with_capacity(clq_vars.len());
        let num_clq_vars = clq_vars.len();
        partition_start.clear();
        partition_start.reserve(num_clq_vars);
        let mut extension_end = num_clq_vars;
        partition_start.push(0);
        let mut queries = self.num_neighbourhood_queries;
        for i in 0..num_clq_vars {
            if i == extension_end {
                partition_start.push(i);
                extension_end = num_clq_vars;
            }
            let v = clq_vars[i];
            let extension_start = i + 1;
            extension_end = self.partition_neighbourhood(&mut neighbourhood_inds, &mut queries, v, &mut clq_vars[extension_start..extension_end]) + extension_start;
        }
        self.num_neighbourhood_queries = queries;
        partition_start.push(num_clq_vars);
    }

    /// HiGHS's `cliquePartition(objective, clqVars, partitionStart)`: the
    /// same, greatest objective contribution first.
    pub fn clique_partition_objective(&mut self, objective: &[f64], clq_vars: &mut [CliqueVar], partition_start: &mut Vec<usize>) {
        self.randgen.shuffle(clq_vars);
        let key = |v: &CliqueVar| (2.0 * v.val() as f64 - 1.0) * objective[v.col()];
        pdqsort_branchless(clq_vars, |v1, v2| key(v1) > key(v2));
        let mut neighbourhood_inds = Vec::with_capacity(clq_vars.len());
        let num_clq_vars = clq_vars.len();
        partition_start.clear();
        partition_start.reserve(num_clq_vars);
        let mut extension_end = num_clq_vars;
        partition_start.push(0);
        let mut last_swapped_index = 0;
        let mut queries = self.num_neighbourhood_queries;
        for i in 0..num_clq_vars {
            if i == extension_end {
                partition_start.push(i);
                extension_end = num_clq_vars;
                if last_swapped_index >= i {
                    pdqsort_branchless(&mut clq_vars[i..last_swapped_index + 1], |v1, v2| key(v1) > key(v2));
                }
                last_swapped_index = 0;
            }
            let v = clq_vars[i];
            let extension_start = i + 1;
            extension_end = self.partition_neighbourhood(&mut neighbourhood_inds, &mut queries, v, &mut clq_vars[extension_start..extension_end]) + extension_start;
            if let Some(&last) = neighbourhood_inds.last() {
                last_swapped_index = (last + extension_start).max(last_swapped_index);
            }
        }
        self.num_neighbourhood_queries = queries;
        partition_start.push(num_clq_vars);
    }

    /// HiGHS's `bronKerboschRecurse`: maximal cliques heavier than the
    /// lightest kept, among `data.p[..plen]`, excluding `x`.
    fn bron_kerbosch_recurse(&self, data: &mut BronKerboschData, mut plen: usize, x: &[CliqueVar]) {
        let mut w = data.w_r;
        for i in 0..plen {
            w += data.p[i].weight(data.sol);
        }
        if w < data.min_w - data.feastol {
            return;
        }
        if plen == 0 && x.is_empty() {
            let clique = data.r.clone();
            if data.min_w < w - data.feastol {
                data.maxcliques -= data.cliques.len() as i64;
                data.cliques.clear();
                data.min_w = w;
            }
            data.cliques.push(clique);
            return;
        }
        data.ncalls += 1;
        if data.stop() {
            return;
        }
        let mut pivweight = -1.0;
        let mut pivot = CliqueVar::new(0, 0);
        for &xi in x {
            if xi.weight(data.sol) > pivweight {
                pivweight = xi.weight(data.sol);
                pivot = xi;
                if pivweight >= 1.0 - data.feastol {
                    break;
                }
            }
        }
        if pivweight < 1.0 - data.feastol {
            for i in 0..plen {
                if data.p[i].weight(data.sol) > pivweight {
                    pivweight = data.p[i].weight(data.sol);
                    pivot = data.p[i];
                    if pivweight >= 1.0 - data.feastol {
                        break;
                    }
                }
            }
        }
        let mut p_minus_nu: Vec<CliqueVar> = Vec::with_capacity(plen);
        let mut neighbourhood_inds = std::mem::take(&mut data.neighbourhood_inds);
        self.query_neighbourhood(&mut neighbourhood_inds, &mut data.num_neighbourhood_queries, pivot, &data.p[..plen]);
        neighbourhood_inds.push(plen);
        let mut k = 0;
        for &i in &neighbourhood_inds {
            while k < i {
                p_minus_nu.push(data.p[k]);
                k += 1;
            }
            k += 1;
        }
        let sol = data.sol;
        pdqsort(&mut p_minus_nu, |a, b| (a.weight(sol), a.index()) > (b.weight(sol), b.index()));
        let mut local_x: Vec<CliqueVar> = x.to_vec();
        for v in p_minus_nu {
            let new_plen = self.partition_neighbourhood(&mut neighbourhood_inds, &mut data.num_neighbourhood_queries, v, &mut data.p[..plen]);
            let new_xlen = self.partition_neighbourhood(&mut neighbourhood_inds, &mut data.num_neighbourhood_queries, v, &mut local_x);
            data.neighbourhood_inds = std::mem::take(&mut neighbourhood_inds);
            data.r.push(v);
            let wv = v.weight(data.sol);
            data.w_r += wv;
            let x_for_recursion = local_x[..new_xlen].to_vec();
            self.bron_kerbosch_recurse(data, new_plen, &x_for_recursion);
            neighbourhood_inds = std::mem::take(&mut data.neighbourhood_inds);
            if data.stop() {
                data.neighbourhood_inds = neighbourhood_inds;
                return;
            }
            data.r.pop();
            data.w_r -= wv;
            w -= wv;
            if w < data.min_w {
                data.neighbourhood_inds = neighbourhood_inds;
                return;
            }
            let vpos = (new_plen..plen).find(|&i| data.p[i] == v).expect("v among the candidates");
            plen -= 1;
            data.p.swap(vpos, plen);
            local_x.push(v);
        }
        data.neighbourhood_inds = neighbourhood_inds;
    }

    // ---- subsumption ------------------------------------------------------

    fn collect_cliques(&mut self, clique: &[CliqueVar]) {
        if self.cliquehits.len() < self.cliques.len() {
            self.cliquehits.resize(self.cliques.len(), 0);
        }
        for &v in clique {
            let cliquehits = &mut self.cliquehits;
            let cliquehitinds = &mut self.cliquehitinds;
            self.inverted_hash_list[v.index()].for_each(|&cliqueid, _| {
                if cliquehits[cliqueid] == 0 {
                    cliquehitinds.push(cliqueid);
                }
                cliquehits[cliqueid] += 1;
            });
            self.inverted_hash_list_size_two[v.index()].for_each(|&cliqueid, _| {
                if cliquehits[cliqueid] == 0 {
                    cliquehitinds.push(cliqueid);
                }
                cliquehits[cliqueid] += 1;
            });
        }
    }

    /// HiGHS's `cliqueSubsumption`: is `clique` redundant (and the origin of
    /// a clique that dominates it), removing the cliques it contains (by
    /// `remove`).
    fn clique_subsumption(&mut self, clique: &[CliqueVar], remove: &mut dyn FnMut(&mut CliqueTable, usize)) -> (bool, i32) {
        self.collect_cliques(clique);
        let mut redundant = false;
        let mut dominating_origin = NO_ORIGIN;
        let hit_inds = std::mem::take(&mut self.cliquehitinds);
        for &cliqueid in &hit_inds {
            let hits = self.cliquehits[cliqueid] as usize;
            self.cliquehits[cliqueid] = 0;
            if hits == clique.len() {
                redundant = true;
                let origin = self.cliques[cliqueid].origin;
                if origin != NO_ORIGIN && origin != ORIGIN_DELETED {
                    dominating_origin = origin;
                }
            } else if self.cliques[cliqueid].num_active() == hits {
                if self.cliques[cliqueid].equality {
                    let size_two = self.cliques[cliqueid].end() - self.cliques[cliqueid].start() == 2;
                    for &v in clique {
                        let v_has_clq = if size_two {
                            self.inverted_hash_list_size_two[v.index()].contains(&cliqueid)
                        } else {
                            self.inverted_hash_list[v.index()].contains(&cliqueid)
                        };
                        if !v_has_clq {
                            self.infeasvertexstack.push(v);
                        }
                    }
                } else {
                    remove(self, cliqueid);
                }
            }
        }
        self.cliquehitinds = hit_inds;
        self.cliquehitinds.clear();
        (redundant, dominating_origin)
    }

    /// HiGHS's `runCliqueSubsumption`; returns how many cliques it removed.
    fn run_clique_subsumption(&mut self, globaldom: &Domain, clique: &mut Vec<CliqueVar>) -> usize {
        let col_deleted = &self.col_deleted;
        clique.retain(|v| !col_deleted[v.col()]);
        if clique.len() <= 2 {
            return 0;
        }
        let mut nremoved = 0;
        let (redundant, _) = self.clique_subsumption(clique, &mut |table, cliqueid| {
            nremoved += 1;
            table.cliques[cliqueid].origin = NO_ORIGIN;
            table.remove_clique(cliqueid);
        });
        if redundant {
            clique.clear();
        }
        if !self.infeasvertexstack.is_empty() {
            clique.retain(|v| !globaldom.is_fixed(v.col()));
        }
        nremoved
    }

    // ---- extraction from rows --------------------------------------------

    /// HiGHS's `extractCliques(mipsolver, inds, vals, ...)` for a row
    /// `vals . x <= rhs` over complemented columns: variable bounds on the
    /// non-binaries, then cliques among the binaries.
    #[allow(clippy::too_many_arguments)]
    fn extract_cliques_from_row(
        &mut self,
        model: &MipModel,
        globaldom: &mut Domain,
        inds: &[usize],
        vals: &[f64],
        complementation: &[i8],
        rhs: f64,
        feastol: f64,
        cc: &mut CliqueCtx,
    ) {
        let mut perm: Vec<usize> = (0..inds.len()).collect();
        let nbin = super::cpp_partition(&mut perm, |&pos| globaldom.is_binary(model, inds[pos]));
        let ntotal = perm.len();
        if nbin < ntotal {
            for i in 0..nbin {
                let bincol = inds[perm[i]];
                let impliedub = CDouble::new(rhs) - vals[perm[i]];
                if cc.implications.too_many_var_bounds() {
                    break;
                }
                for j in nbin..ntotal {
                    let col = inds[perm[j]];
                    if globaldom.is_fixed(col) {
                        continue;
                    }
                    let colub = CDouble::new(globaldom.col_upper[col]) - globaldom.col_lower[col];
                    let mut implcolub = impliedub / vals[perm[j]];
                    if model.is_col_integral(col) {
                        implcolub = CDouble::new((implcolub.value() + cc.cx.feastol).floor());
                    }
                    if implcolub < (colub - feastol).value() {
                        let (coef, mut constant) = if complementation[perm[i]] == -1 {
                            (colub - implcolub, implcolub)
                        } else {
                            (implcolub - colub, colub)
                        };
                        if complementation[perm[j]] == -1 {
                            constant -= globaldom.col_upper[col];
                            cc.implications.add_vlb(col, bincol, -coef.value(), -constant.value(), globaldom, model, cc.cx.feastol);
                        } else {
                            constant += globaldom.col_lower[col];
                            cc.implications.add_vub(col, bincol, coef.value(), constant.value(), globaldom, model, cc.cx.feastol);
                        }
                    }
                }
            }
        }
        if nbin <= 1 {
            return;
        }
        pdqsort(&mut perm[..nbin], |&p1, &p2| (vals[p1], p1) > (vals[p2], p2));
        if vals[perm[0]] + vals[perm[1]] <= rhs + feastol {
            return;
        }
        let literal = |pos: usize| {
            if complementation[pos] == -1 {
                CliqueVar::new(inds[pos], 0)
            } else {
                CliqueVar::new(inds[pos], 1)
            }
        };
        if (vals[perm[0]] - vals[perm[nbin - 1]]).abs() <= feastol && rhs < 2.0 * vals[perm[nbin - 1]] - feastol {
            let mut clique: Vec<CliqueVar> = perm[..nbin].iter().map(|&pos| literal(pos)).collect();
            self.add_clique(model, globaldom, &mut clique, false, NO_ORIGIN, cc);
            return;
        }
        for k in (1..nbin).rev() {
            let mincliqueval = rhs - vals[perm[k]] + feastol;
            let cliqueend = perm[..k].partition_point(|&p| vals[p] > mincliqueval);
            if cliqueend == 0 {
                continue;
            }
            let mut clique: Vec<CliqueVar> = perm[..cliqueend].iter().map(|&pos| literal(pos)).collect();
            clique.push(literal(perm[k]));
            if clique.len() >= 2 {
                self.add_clique(model, globaldom, &mut clique, false, NO_ORIGIN, cc);
                if globaldom.infeasible() {
                    return;
                }
            }
            if cliqueend == k {
                return;
            }
        }
    }

    /// HiGHS's `extractCliquesFromCut`: tighten bounds from a cut
    /// `vals . x <= rhs` of the global pool, record its variable bounds,
    /// and add the cliques it implies.
    #[allow(clippy::too_many_arguments)]
    pub fn extract_cliques_from_cut(
        &mut self,
        model: &MipModel,
        globaldom: &mut Domain,
        inds: &[usize],
        vals: &[f64],
        rhs: f64,
        num_clique_entries_after_presolve: usize,
        cc: &mut CliqueCtx,
    ) {
        if self.is_full() {
            return;
        }
        let feastol = cc.cx.feastol;
        let len = inds.len();
        let mut minact = CDouble::new(0.0);
        let mut nbin = 0;
        for i in 0..len {
            if globaldom.is_binary(model, inds[i]) {
                nbin += 1;
            }
            if vals[i] > 0.0 {
                if globaldom.col_lower[inds[i]] == -INF {
                    return;
                }
                minact += vals[i] * CDouble::new(globaldom.col_lower[inds[i]]);
            } else {
                if globaldom.col_upper[inds[i]] == INF {
                    return;
                }
                minact += vals[i] * CDouble::new(globaldom.col_upper[inds[i]]);
            }
        }
        if (CDouble::new(rhs) - minact) < 0.0 {
            minact = CDouble::new(rhs);
        }
        for i in 0..len {
            if model.is_col_continuous(inds[i]) {
                continue;
            }
            // In double-double, with a tolerance that also covers epsilon
            // over the coefficient (HiGHS #3223).
            let implied_bound = (CDouble::new(rhs) - minact) / vals[i];
            let bound_tol = feastol.max(cc.cx.epsilon / vals[i].abs());
            if vals[i] > 0.0 {
                let bound_val = (implied_bound + globaldom.col_lower[inds[i]] + bound_tol).value().floor();
                self.with_env(globaldom, cc, |dom, env| {
                    dom.change_bound(DomainChange::new(bound_val, inds[i], BoundType::Upper), Reason::unspecified(), env)
                });
            } else {
                let bound_val = (implied_bound + globaldom.col_upper[inds[i]] - bound_tol).value().ceil();
                self.with_env(globaldom, cc, |dom, env| {
                    dom.change_bound(DomainChange::new(bound_val, inds[i], BoundType::Lower), Reason::unspecified(), env)
                });
            }
            if globaldom.infeasible() {
                return;
            }
        }
        if nbin <= 1 {
            return;
        }
        let mut perm: Vec<usize> = (0..len).collect();
        let nbin = super::cpp_partition(&mut perm, |&pos| globaldom.is_binary(model, inds[pos]));
        if nbin < len {
            for i in 0..nbin {
                let bincol = inds[perm[i]];
                let implied_activity = CDouble::new(rhs) - minact - vals[perm[i]].abs();
                for j in nbin..len {
                    let col = inds[perm[j]];
                    if globaldom.is_fixed(col) {
                        continue;
                    }
                    if vals[perm[j]] > 0.0 {
                        let mut implcolub = ((implied_activity + vals[perm[j]] * CDouble::new(globaldom.col_lower[col])) / vals[perm[j]]).value();
                        if model.is_col_integral(col) {
                            implcolub = (implcolub + feastol).floor();
                        }
                        if implcolub < globaldom.col_upper[col] - feastol {
                            let (coef, constant) = if vals[perm[i]] < 0.0 {
                                (globaldom.col_upper[col] - implcolub, implcolub)
                            } else {
                                if globaldom.col_upper[col] == INF {
                                    continue;
                                }
                                (implcolub - globaldom.col_upper[col], globaldom.col_upper[col])
                            };
                            cc.implications.add_vub(col, bincol, coef, constant, globaldom, model, feastol);
                        }
                    } else {
                        let mut implcollb = ((implied_activity + vals[perm[j]] * CDouble::new(globaldom.col_upper[col])) / vals[perm[j]]).value();
                        if model.is_col_integral(col) {
                            implcollb = (implcollb - feastol).ceil();
                        }
                        if implcollb > globaldom.col_lower[col] + feastol {
                            let (coef, constant) = if vals[perm[i]] < 0.0 {
                                (globaldom.col_lower[col] - implcollb, implcollb)
                            } else {
                                if globaldom.col_lower[col] == -INF {
                                    continue;
                                }
                                (implcollb - globaldom.col_lower[col], globaldom.col_lower[col])
                            };
                            cc.implications.add_vlb(col, bincol, coef, constant, globaldom, model, feastol);
                        }
                    }
                }
            }
        }
        if nbin <= 1 {
            return;
        }
        pdqsort(&mut perm[..nbin], |&p1, &p2| (vals[p1].abs(), p1) > (vals[p2].abs(), p2));
        if vals[perm[0]].abs() + vals[perm[1]].abs() <= (CDouble::new(rhs) - minact + feastol).value() {
            return;
        }
        let max_new_entries = (num_clique_entries_after_presolve + 100_000 + 4 * model.num_nonzero()).min(self.num_entries + 10 * nbin);
        let literal = |pos: usize| if vals[pos] < 0.0 { CliqueVar::new(inds[pos], 0) } else { CliqueVar::new(inds[pos], 1) };
        let mut k = nbin - 1;
        while k != 0 && self.num_entries < max_new_entries {
            let mincliqueval = (CDouble::new(rhs) - minact - vals[perm[k]].abs() + feastol).value();
            let cliqueend = perm[..k].partition_point(|&p| vals[p].abs() > mincliqueval);
            if cliqueend != 0 {
                let mut clique: Vec<CliqueVar> = perm[..cliqueend].iter().map(|&pos| literal(pos)).collect();
                clique.push(literal(perm[k]));
                if clique.len() >= 2 {
                    self.add_clique(model, globaldom, &mut clique, false, NO_ORIGIN, cc);
                    if globaldom.infeasible() || self.num_entries >= max_new_entries {
                        return;
                    }
                }
                if cliqueend == k {
                    return;
                }
            }
            k -= 1;
        }
    }

    /// HiGHS's `extractCliques(mipsolver, transformRows)`: set packing rows
    /// as cliques, and (when `transform_rows`) the cliques other rows
    /// imply. Rows from `num_orig_rows` on (by `orig_row_index`) were not
    /// the original model's.
    pub fn extract_cliques(
        &mut self,
        model: &MipModel,
        globaldom: &mut Domain,
        transform_rows: bool,
        orig_row_index: &dyn Fn(usize) -> usize,
        num_orig_rows: usize,
        cc: &mut CliqueCtx,
    ) {
        let mut entries: HashTable<usize, f64> = HashTable::new();
        for i in 0..model.num_row {
            let (start, end) = (model.ar_start[i], model.ar_start[i + 1]);
            if orig_row_index(i) >= num_orig_rows {
                break;
            }
            if model.row_upper[i] == 1.0 {
                let mut issetppc = true;
                let mut clique = Vec::new();
                for j in start..end {
                    let col = model.ar_index[j];
                    if globaldom.col_upper[col] == 0.0 && globaldom.col_lower[col] == 0.0 {
                        continue;
                    }
                    issetppc = globaldom.is_binary(model, col) && model.ar_value[j] == 1.0;
                    if !issetppc {
                        break;
                    }
                    clique.push(CliqueVar::new(col, 1));
                }
                if issetppc {
                    self.add_clique(model, globaldom, &mut clique, model.row_lower[i] == 1.0, i as i32, cc);
                    if globaldom.infeasible() {
                        return;
                    }
                    continue;
                }
            }
            if !transform_rows || self.is_full() {
                continue;
            }
            let mut offset = 0.0;
            for j in start..end {
                let mut col = model.ar_index[j];
                let mut val = model.ar_value[j];
                self.resolve_substitution_entry(&mut col, &mut val, &mut offset);
                *entries.entry(col) += val;
            }
            for (row_rhs, direction) in [(model.row_upper[i], 1.0), (model.row_lower[i], -1.0)] {
                if direction * row_rhs == INF {
                    continue;
                }
                let mut rhs = direction * (row_rhs - offset);
                let mut inds = Vec::new();
                let mut vals = Vec::new();
                let mut complementation = Vec::new();
                let mut freevar = false;
                let mut nbin = 0;
                for (&col, &value) in entries.iter() {
                    let val = direction * value;
                    if val.abs() < cc.cx.epsilon {
                        continue;
                    }
                    if globaldom.is_binary(model, col) {
                        nbin += 1;
                    }
                    if val < 0.0 {
                        freevar = globaldom.col_upper[col] == INF;
                        if freevar {
                            break;
                        }
                        vals.push(-val);
                        inds.push(col);
                        complementation.push(-1i8);
                        rhs -= val * globaldom.col_upper[col];
                    } else {
                        freevar = globaldom.col_lower[col] == -INF;
                        if freevar {
                            break;
                        }
                        vals.push(val);
                        inds.push(col);
                        complementation.push(1i8);
                        rhs -= val * globaldom.col_lower[col];
                    }
                }
                if !freevar && nbin != 0 {
                    let feastol = cc.cx.feastol;
                    self.extract_cliques_from_row(model, globaldom, &inds, &vals, &complementation, rhs, feastol, cc);
                    if globaldom.infeasible() {
                        return;
                    }
                }
            }
            entries.clear();
        }
    }

    /// HiGHS's `extractObjCliques`: cliques the objective cutoff implies.
    pub fn extract_obj_cliques(&mut self, model: &MipModel, globaldom: &mut Domain, cc: &mut CliqueCtx) {
        let nbin = cc.cx.objective.num_binaries_in_objective();
        if nbin <= 1 || globaldom.objective_lower_bound() == -INF {
            return;
        }
        let (inds, vals, rhs) = self.with_env(globaldom, cc, |dom, env| dom.cutoff_constraint(env));
        let mut perm: Vec<usize> = (0..nbin).collect();
        let nbin = super::cpp_partition(&mut perm, |&pos| vals[pos] != 0.0 && !globaldom.is_fixed(inds[pos]));
        if nbin <= 1 {
            return;
        }
        pdqsort(&mut perm[..nbin], |&p1, &p2| (vals[p1].abs(), p1) > (vals[p2].abs(), p2));
        let (_, minact) = globaldom.compute_min_activity(&inds, &vals);
        let feastol = cc.cx.feastol;
        if vals[perm[0]].abs() + vals[perm[1]].abs() <= (CDouble::new(rhs) - minact + feastol).value() {
            return;
        }
        let literal = |pos: usize| if vals[pos] < 0.0 { CliqueVar::new(inds[pos], 0) } else { CliqueVar::new(inds[pos], 1) };
        for k in (1..nbin).rev() {
            let mincliqueval = (CDouble::new(rhs) - minact - vals[perm[k]].abs() + feastol).value();
            let cliqueend = perm[..k].partition_point(|&p| vals[p].abs() > mincliqueval);
            if cliqueend == 0 {
                continue;
            }
            let mut clique: Vec<CliqueVar> = perm[..cliqueend].iter().map(|&pos| literal(pos)).collect();
            clique.push(literal(perm[k]));
            if clique.len() >= 2 {
                self.add_clique(model, globaldom, &mut clique, false, NO_ORIGIN, cc);
                if globaldom.infeasible() {
                    return;
                }
            }
            if cliqueend == k {
                return;
            }
        }
    }

    // ---- separation and merging ------------------------------------------

    /// HiGHS's `separateCliques`: maximal cliques violated by `sol`, as
    /// cuts `(inds, vals, rhs)`; the caller adds them to the pool. With
    /// `subsume` (the table's own generator), cliques they cover are merged.
    #[allow(clippy::too_many_arguments)]
    pub fn separate_cliques(
        &mut self,
        sol: &[f64],
        integral_cols: &[usize],
        globaldom: &Domain,
        feastol: f64,
        num_nonzero: usize,
        total_lp_iterations: i64,
        randgen: Option<&mut Random>,
        local_num_neighbourhood_queries: Option<&mut i64>,
    ) -> Vec<(Vec<usize>, Vec<f64>, f64)> {
        let mut data = BronKerboschData::new(sol);
        data.feastol = feastol;
        data.max_neighbourhood_queries = 1_000_000 + 100 * num_nonzero as i64 + total_lp_iterations * 1000;
        if self.num_neighbourhood_queries > data.max_neighbourhood_queries {
            return Vec::new();
        }
        data.max_neighbourhood_queries -= self.num_neighbourhood_queries;
        for &i in integral_cols {
            if self.colsubstituted[i] != 0 || self.col_deleted[i] {
                continue;
            }
            for val in [0, 1] {
                if self.num_cliques_col(i, val == 1) != 0 {
                    let v = CliqueVar::new(i, val);
                    if v.weight(sol) > feastol {
                        data.p.push(v);
                    } else {
                        data.z.push(v);
                    }
                }
            }
        }
        let plen = data.p.len();
        self.bron_kerbosch_recurse(&mut data, plen, &[]);
        let own_randgen = randgen.is_none();
        let mut fallback = Random::new(0);
        let randgen: &mut Random = match randgen {
            Some(randgen) => randgen,
            None => {
                fallback = self.randgen.clone();
                &mut fallback
            }
        };
        let mut runcliquesubsumption = false;
        let mut cuts = Vec::new();
        let mut cliques = std::mem::take(&mut data.cliques);
        for clique in &mut cliques {
            let mut extensionend = data.z.len();
            for &v in clique.iter() {
                let mut inds = std::mem::take(&mut data.neighbourhood_inds);
                extensionend = self.partition_neighbourhood(&mut inds, &mut data.num_neighbourhood_queries, v, &mut data.z[..extensionend]);
                data.neighbourhood_inds = inds;
                if extensionend == 0 {
                    break;
                }
            }
            if extensionend != 0 {
                randgen.shuffle(&mut data.z[..extensionend]);
                let mut i = 0;
                while i < extensionend {
                    let k = i + 1;
                    let zi = data.z[i];
                    let mut inds = std::mem::take(&mut data.neighbourhood_inds);
                    extensionend = k + self.partition_neighbourhood(&mut inds, &mut data.num_neighbourhood_queries, zi, &mut data.z[k..extensionend]);
                    data.neighbourhood_inds = inds;
                    i += 1;
                }
                clique.extend_from_slice(&data.z[..extensionend]);
            }
            let mut rhs = 1.0;
            runcliquesubsumption = self.cliques.len() > 2;
            let mut inds = Vec::with_capacity(clique.len());
            let mut vals = Vec::with_capacity(clique.len());
            for &v in clique.iter() {
                inds.push(v.col());
                if v.val() == 0 {
                    vals.push(-1.0);
                    rhs -= 1.0;
                } else {
                    vals.push(1.0);
                }
            }
            rhs = (rhs + 0.5f64).floor();
            cuts.push((inds, vals, rhs));
        }
        if own_randgen {
            self.randgen = fallback;
        }
        match local_num_neighbourhood_queries {
            Some(queries) => *queries += data.num_neighbourhood_queries,
            None => self.num_neighbourhood_queries += data.num_neighbourhood_queries,
        }
        if runcliquesubsumption && own_randgen {
            for clique in &mut cliques {
                let nremoved = self.run_clique_subsumption(globaldom, clique);
                if clique.is_empty() {
                    continue;
                }
                if nremoved != 0 {
                    let clique = clique.clone();
                    self.do_add_clique(&clique, false, ORIGIN_DELETED);
                }
            }
        }
        cuts
    }

    /// HiGHS's `computeMaximalCliques`.
    pub fn compute_maximal_cliques(&self, vars: &[CliqueVar], feastol: f64) -> Vec<Vec<CliqueVar>> {
        if vars.is_empty() {
            return Vec::new();
        }
        let maxcolindex = vars.iter().map(|var| var.col()).max().unwrap_or(0);
        let mut sol = vec![0.0; maxcolindex + 1];
        for var in vars {
            sol[var.col()] = var.val() as f64;
        }
        let mut data = BronKerboschData::new(&sol);
        data.feastol = feastol;
        for &var in vars {
            if self.colsubstituted[var.col()] != 0 || self.col_deleted[var.col()] {
                continue;
            }
            if self.num_cliques_of(var) != 0 {
                data.p.push(var);
            }
        }
        let plen = data.p.len();
        self.bron_kerbosch_recurse(&mut data, plen, &[]);
        data.cliques
    }

    /// Add `extension`'s candidate literals from the cliques of
    /// `extensionstart` (`runCliqueMerging`'s `addCands`).
    fn add_merge_candidates(&mut self, globaldom: &Domain, extensionstart: CliqueVar, out: &mut Vec<CliqueVar>) {
        let cliques = &self.cliques;
        let cliqueentries = &self.cliqueentries;
        let iscandidate = &mut self.iscandidate;
        let mut add = |cliqueid: usize| {
            let (start, end) = (cliques[cliqueid].start(), cliques[cliqueid].end());
            for cliqueentrie in &cliqueentries[start..end] {
                let entry = *cliqueentrie;
                if iscandidate[entry.index()] || globaldom.is_fixed(entry.col()) {
                    continue;
                }
                iscandidate[entry.index()] = true;
                out.push(entry);
            }
        };
        self.inverted_hash_list[extensionstart.index()].for_each(|&cliqueid, _| add(cliqueid));
        self.inverted_hash_list_size_two[extensionstart.index()].for_each(|&cliqueid, _| add(cliqueid));
    }

    /// HiGHS's `runCliqueMerging(globaldomain, clique, equation)`: extend
    /// `clique` as far as the table allows.
    pub fn run_clique_merging_clique(&mut self, globaldom: &mut Domain, clique: &mut Vec<CliqueVar>, equation: bool, cc: &mut CliqueCtx) {
        let mut extensionstart = CliqueVar::default();
        let mut numcliques = usize::MAX;
        self.iscandidate.resize(self.inverted_hash_list.len(), false);
        let mut neighbourhood_inds = Vec::with_capacity(self.inverted_hash_list.len());
        let initial_clique_size = clique.len();
        for &v in clique.iter() {
            if globaldom.is_fixed(v.col()) {
                continue;
            }
            let this_num_clqs = self.num_cliques_of(v);
            if this_num_clqs < numcliques {
                numcliques = this_num_clqs;
                extensionstart = v;
            }
        }
        if numcliques == usize::MAX {
            clique.clear();
            return;
        }
        for v in clique.iter() {
            self.iscandidate[v.index()] = true;
        }
        let mut candidates = Vec::new();
        self.add_merge_candidates(globaldom, extensionstart, &mut candidates);
        clique.extend(candidates);
        for v in clique.iter() {
            self.iscandidate[v.index()] = false;
        }
        let mut queries = self.num_neighbourhood_queries;
        let mut i = 0;
        while i != initial_clique_size && initial_clique_size < clique.len() {
            if clique[i] != extensionstart {
                let ci = clique[i];
                let new_size = initial_clique_size + self.shrink_to_neighbourhood(&mut neighbourhood_inds, &mut queries, ci, &mut clique[initial_clique_size..]);
                clique.truncate(new_size);
            }
            i += 1;
        }
        if initial_clique_size < clique.len() {
            self.randgen.shuffle(&mut clique[initial_clique_size..]);
            let mut i = initial_clique_size;
            while i < clique.len() {
                let extvar = clique[i];
                i += 1;
                let new_size = i + self.shrink_to_neighbourhood(&mut neighbourhood_inds, &mut queries, extvar, &mut clique[i..]);
                clique.truncate(new_size);
            }
        }
        self.num_neighbourhood_queries = queries;
        if equation {
            for clique_i in &clique[initial_clique_size..] {
                let v = *clique_i;
                self.vertex_infeasible(globaldom, v.col(), v.val() as usize, cc);
            }
        } else {
            self.run_clique_subsumption(globaldom, clique);
            if !clique.is_empty() {
                clique.retain(|v| !(globaldom.is_fixed(v.col()) && globaldom.col_lower[v.col()] as i32 == 1 - v.val() as i32));
            }
        }
        self.process_infeasible_vertices(globaldom, cc);
    }

    /// HiGHS's `runCliqueMerging(globaldomain)`: extend the model's cliques.
    pub fn run_clique_merging(&mut self, globaldom: &mut Domain, cc: &mut CliqueCtx) {
        let mut extensionvars: Vec<CliqueVar> = Vec::new();
        self.iscandidate.resize(self.inverted_hash_list.len(), false);
        let mut neighbourhood_inds = Vec::with_capacity(self.inverted_hash_list.len());
        let numcliqueslots = self.cliques.len();
        let max_new_entries = self.num_entries + cc.cx.model.num_nonzero();
        let mut have_non_model_cliques_to_merge = false;
        for k in 0..numcliqueslots {
            if self.cliques[k].start() == NONE {
                continue;
            }
            if !self.cliques[k].equality && self.cliques[k].origin == NO_ORIGIN {
                continue;
            }
            if self.cliques[k].origin == ORIGIN_DELETED {
                have_non_model_cliques_to_merge = true;
                continue;
            }
            let (start, end) = (self.cliques[k].start(), self.cliques[k].end());
            if end == start {
                continue;
            }
            let clqvars: Vec<CliqueVar> = self.cliqueentries[start..end].to_vec();
            let mut extensionstart = clqvars[0];
            let mut numcliques = self.num_cliques_of(clqvars[0]);
            for &v in &clqvars[1..] {
                let this_num_clqs = self.num_cliques_of(v);
                if this_num_clqs < numcliques {
                    numcliques = this_num_clqs;
                    extensionstart = v;
                }
            }
            for v in &clqvars {
                self.iscandidate[v.index()] = true;
            }
            self.add_merge_candidates(globaldom, extensionstart, &mut extensionvars);
            for v in &clqvars {
                self.iscandidate[v.index()] = false;
            }
            for v in &extensionvars {
                self.iscandidate[v.index()] = false;
            }
            let mut queries = self.num_neighbourhood_queries;
            let mut i = 0;
            while i != clqvars.len() && !extensionvars.is_empty() {
                if clqvars[i] != extensionstart {
                    let new_size = self.shrink_to_neighbourhood(&mut neighbourhood_inds, &mut queries, clqvars[i], &mut extensionvars);
                    extensionvars.truncate(new_size);
                }
                i += 1;
            }
            if !extensionvars.is_empty() {
                self.randgen.shuffle(&mut extensionvars);
                let mut i = 0;
                while i < extensionvars.len() {
                    let extvar = extensionvars[i];
                    i += 1;
                    let new_size = i + self.shrink_to_neighbourhood(&mut neighbourhood_inds, &mut queries, extvar, &mut extensionvars[i..]);
                    extensionvars.truncate(new_size);
                }
            }
            self.num_neighbourhood_queries = queries;
            if self.cliques[k].equality {
                for v in extensionvars.clone() {
                    self.vertex_infeasible(globaldom, v.col(), v.val() as usize, cc);
                }
            } else {
                let originrow = self.cliques[k].origin;
                self.cliques[k].origin = NO_ORIGIN;
                let num_extensions = extensionvars.len();
                let (start, end) = (self.cliques[k].start(), self.cliques[k].end());
                let members: Vec<CliqueVar> = self.cliqueentries[start..end].to_vec();
                extensionvars.extend(members.into_iter().filter(|v| !self.col_deleted[v.col()]));
                self.remove_clique(k);
                let (redundant, dominating_origin) = self.clique_subsumption(&extensionvars, &mut |table, cliqueid| table.remove_clique(cliqueid));
                if !redundant {
                    for &v in &extensionvars[..num_extensions] {
                        self.cliqueextensions.push((originrow, v));
                    }
                    extensionvars.retain(|v| !(globaldom.is_fixed(v.col()) && globaldom.col_lower[v.col()] as i32 == 1 - v.val() as i32));
                    if extensionvars.len() > 1 {
                        let vars = extensionvars.clone();
                        self.do_add_clique(&vars, false, originrow);
                    }
                } else if dominating_origin != NO_ORIGIN {
                    self.deletedrows.push(originrow);
                } else {
                    for &v in &extensionvars[..num_extensions] {
                        self.cliqueextensions.push((originrow, v));
                    }
                }
            }
            extensionvars.clear();
            self.process_infeasible_vertices(globaldom, cc);
            if self.num_entries >= max_new_entries {
                break;
            }
        }
        if have_non_model_cliques_to_merge {
            for k in 0..numcliqueslots {
                if self.cliques[k].start() == NONE || self.cliques[k].origin != ORIGIN_DELETED {
                    continue;
                }
                let (start, end) = (self.cliques[k].start(), self.cliques[k].end());
                extensionvars = self.cliqueentries[start..end].to_vec();
                self.remove_clique(k);
                self.run_clique_merging_clique(globaldom, &mut extensionvars, false, cc);
                if extensionvars.len() > 1 {
                    let vars = extensionvars.clone();
                    self.do_add_clique(&vars, false, NO_ORIGIN);
                }
            }
        }
    }

    /// HiGHS's `rebuild`: the table over the presolved columns
    /// (`orig2reducedcol`), keeping binaries that stay linearly
    /// transformable.
    pub fn rebuild(&mut self, ncols: usize, orig2reducedcol: &[Option<usize>], is_col_linearly_transformable: &dyn Fn(usize) -> bool, globaldom: &Domain, model: &MipModel) {
        let mut new_table = CliqueTable::new(ncols);
        new_table.set_presolve_flag(self.in_presolve);
        for i in 0..self.cliques.len() {
            if self.cliques[i].start() == NONE {
                continue;
            }
            let (start, end) = (self.cliques[i].start(), self.cliques[i].end());
            let oldnumvars = end - start;
            let mut kept = Vec::with_capacity(oldnumvars);
            for k in start..end {
                if let Some(col) = orig2reducedcol[self.cliqueentries[k].col()]
                    && globaldom.is_binary(model, col)
                    && is_col_linearly_transformable(col)
                {
                    kept.push(CliqueVar::from_parts(col as u32, self.cliqueentries[k].val()));
                }
            }
            let numvars = kept.len();
            if numvars <= 1 {
                continue;
            }
            let origin = if self.cliques[i].origin != NO_ORIGIN { ORIGIN_DELETED } else { NO_ORIGIN };
            new_table.do_add_clique(&kept, if numvars != oldnumvars { false } else { self.cliques[i].equality }, origin);
        }
        *self = new_table;
    }

    /// HiGHS's `buildFrom`: the table `init` holds, over the binaries of
    /// the original model (`col_lower`/`col_upper`).
    pub fn build_from(&mut self, orig_col_lower: &[f64], orig_col_upper: &[f64], init: &CliqueTable) {
        let ncols = init.colsubstituted.len();
        let mut new_table = CliqueTable::new(ncols);
        new_table.set_presolve_flag(self.in_presolve);
        for clique in &init.cliques {
            if clique.start() == NONE || clique.num_active() <= 1 {
                continue;
            }
            let buffer: Vec<CliqueVar> = init.cliqueentries[clique.start()..clique.end()]
                .iter()
                .copied()
                .filter(|v| orig_col_lower[v.col()] == 0.0 && orig_col_upper[v.col()] == 1.0)
                .collect();
            if buffer.len() <= 1 {
                continue;
            }
            let origin = if clique.origin != NO_ORIGIN { ORIGIN_DELETED } else { NO_ORIGIN };
            new_table.do_add_clique(&buffer, false, origin);
        }
        new_table.colsubstituted = init.colsubstituted.clone();
        new_table.substitutions = init.substitutions.clone();
        *self = new_table;
    }

    /// The clique `cliqueid`'s literals.
    pub fn clique(&self, cliqueid: usize) -> &[CliqueVar] {
        let clique = &self.cliques[cliqueid];
        &self.cliqueentries[clique.start()..clique.end()]
    }

    pub fn cliques(&self) -> &[Clique] {
        &self.cliques
    }

    /// The cliques of literal `v`: `(clique, position of v)` for the long
    /// ones, then the pairs.
    pub fn cliques_of(&self, v: CliqueVar, mut f: impl FnMut(usize)) {
        self.inverted_hash_list[v.index()].for_each(|&cliqueid, _| f(cliqueid));
        self.inverted_hash_list_size_two[v.index()].for_each(|&cliqueid, _| f(cliqueid));
    }

    pub fn col_deleted(&self, col: usize) -> bool {
        self.col_deleted[col]
    }

    pub fn col_substituted(&self, col: usize) -> bool {
        self.colsubstituted[col] != 0
    }
}
