//! HiGHS's `HighsSymmetry`: the symmetries of a MIP, as permutations of its
//! columns and as orbitopes, used to branch and fix by orbits (detection is
//! being ported).

use std::{collections::BTreeSet, rc::Rc};

use super::{
    BoundType, DomainChange,
    clique_table::{CliqueTable, CliqueVar},
    domain::{Domain, DomainEnv, Reason},
};
use crate::{hash_table::HashTable, pdqsort::pdqsort};

const NONE: usize = usize::MAX;

/// HiGHS's `RowPackingStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum RowPacking {
    Undetermined = -1,
    NotPacking = 0,
    Packing = 1,
    PackingNegated = 2,
}

/// HiGHS's `HighsOrbitopeMatrix`: columns laid out as a matrix whose
/// columns any permutation may swap.
#[derive(Clone, Debug, Default)]
pub(crate) struct OrbitopeMatrix {
    pub row_length: usize,
    pub num_rows: usize,
    pub num_set_packing_rows: usize,
    pub column_to_row: HashTable<usize, usize>,
    pub row_is_set_packing: Vec<RowPacking>,
    pub matrix: Vec<usize>,
}

impl OrbitopeMatrix {
    pub fn entry(&self, i: usize, j: usize) -> usize {
        self.matrix[i + j * self.num_rows]
    }

    /// HiGHS's `getBranchingColumn`: in a set packing row, the first
    /// unfixed column before `col`.
    pub fn branching_column(&self, col_lower: &[f64], col_upper: &[f64], col: usize) -> usize {
        if let Some(&i) = self.column_to_row.find(&col)
            && self.row_is_set_packing[i] > RowPacking::NotPacking
        {
            for j in 0..self.row_length {
                let branch_col = self.entry(i, j);
                if branch_col == col {
                    break;
                }
                if col_lower[branch_col] != col_upper[branch_col] {
                    return branch_col;
                }
            }
        }
        col
    }
}

/// HiGHS's `HighsSymmetries`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Symmetries {
    pub permutation_columns: Vec<usize>,
    pub permutations: Vec<usize>,
    pub orbit_partition: Vec<usize>,
    pub orbit_size: Vec<usize>,
    pub column_position: Vec<Option<usize>>,
    pub link_compression_stack: Vec<usize>,
    pub orbitopes: Vec<OrbitopeMatrix>,
    pub column_to_orbitope: HashTable<usize, usize>,
    pub num_perms: usize,
    pub num_generators: usize,
}

impl Symmetries {
    /// HiGHS's `getBranchingColumn`.
    pub fn branching_column(&self, col_lower: &[f64], col_upper: &[f64], col: usize) -> usize {
        if self.column_to_orbitope.is_empty() {
            return col;
        }
        match self.column_to_orbitope.find(&col) {
            Some(&orbitope) if self.orbitopes[orbitope].num_set_packing_rows != 0 => self.orbitopes[orbitope].branching_column(col_lower, col_upper, col),
            _ => col,
        }
    }
}

/// HiGHS's `StabilizerOrbitWorkspace`.
#[derive(Clone, Debug, Default)]
pub(crate) struct StabilizerOrbitWorkspace {
    orbit_partition: Vec<usize>,
    orbit_size: Vec<usize>,
    link_compression_stack: Vec<usize>,
}

/// HiGHS's `StabilizerOrbits`: the orbits of the stabilizer of a node's
/// branchings, for orbital fixing there.
#[derive(Clone, Debug, Default)]
pub(crate) struct StabilizerOrbits {
    pub orbit_cols: Vec<usize>,
    pub orbit_starts: Vec<usize>,
    pub stabilized_cols: Vec<usize>,
}

fn change(domain: &mut Domain, env: &mut DomainEnv, col: usize, boundtype: BoundType, val: f64) {
    domain.change_bound(DomainChange::new(val, col, boundtype), Reason::unspecified(), env);
}

impl StabilizerOrbits {
    /// HiGHS's `orbitalFixing`.
    pub fn orbital_fixing(&self, symmetries: &Symmetries, domain: &mut Domain, env: &mut DomainEnv) -> usize {
        let mut num_fixed = symmetries.propagate_orbitopes(domain, env);
        if domain.infeasible() || self.orbit_cols.is_empty() {
            return num_fixed;
        }
        let num_orbits = self.orbit_starts.len() - 1;
        let mut i = 0;
        while i < num_orbits {
            let range = self.orbit_starts[i]..self.orbit_starts[i + 1];
            let fixcol = self.orbit_cols[range.clone()].iter().copied().find(|&col| domain.is_fixed(col));
            let mut restart = false;
            if let Some(fixcol) = fixcol {
                let old_num_fixed = num_fixed;
                let old_size = domain.domain_change_stack().len();
                if domain.col_lower[fixcol] == 1.0 {
                    for j in range.clone() {
                        if domain.col_lower[self.orbit_cols[j]] == 1.0 {
                            continue;
                        }
                        num_fixed += 1;
                        change(domain, env, self.orbit_cols[j], BoundType::Lower, 1.0);
                        if domain.infeasible() {
                            return num_fixed;
                        }
                    }
                } else {
                    for j in range.clone() {
                        if domain.col_upper[self.orbit_cols[j]] == 0.0 {
                            continue;
                        }
                        num_fixed += 1;
                        change(domain, env, self.orbit_cols[j], BoundType::Upper, 0.0);
                        if domain.infeasible() {
                            return num_fixed;
                        }
                    }
                }
                let new_fixed = num_fixed - old_num_fixed;
                if new_fixed != 0 {
                    domain.propagate(env);
                    if domain.infeasible() {
                        return num_fixed;
                    }
                    if domain.domain_change_stack().len() - old_size > new_fixed {
                        restart = true;
                    }
                }
            }
            i = if restart { 0 } else { i + 1 };
        }
        num_fixed
    }

    /// HiGHS's `isStabilized`.
    pub fn is_stabilized(&self, symmetries: &Symmetries, col: usize) -> bool {
        symmetries.column_position[col].is_none() || self.stabilized_cols.binary_search(&col).is_ok()
    }
}

impl OrbitopeMatrix {
    /// HiGHS's `determineOrbitopeType`: which rows are set packing (or
    /// negated set packing) rows by the clique table.
    pub fn determine_orbitope_type(&mut self, cliquetable: &mut CliqueTable) {
        for j in 0..self.row_length {
            for i in 0..self.num_rows {
                self.column_to_row.insert(self.entry(i, j), i);
            }
        }
        self.row_is_set_packing = vec![RowPacking::Undetermined; self.num_rows];
        self.num_set_packing_rows = 0;
        for (val, packing) in [(1usize, RowPacking::Packing), (0, RowPacking::PackingNegated)] {
            if val == 0 {
                for status in self.row_is_set_packing.iter_mut() {
                    if *status == RowPacking::NotPacking {
                        *status = RowPacking::Undetermined;
                    }
                }
            }
            'outer: for j in 1..self.row_length {
                for j0 in 0..j {
                    for i in 0..self.num_rows {
                        if self.row_is_set_packing[i] != RowPacking::Undetermined {
                            continue;
                        }
                        let xj0 = self.entry(i, j0);
                        let xj1 = self.entry(i, j);
                        let Some(clique) = cliquetable.find_common_clique(CliqueVar::new(xj0, val), CliqueVar::new(xj1, val)) else {
                            self.row_is_set_packing[i] = RowPacking::NotPacking;
                            continue;
                        };
                        let mut overlap = 0;
                        for v in clique {
                            if v.val() as usize != val {
                                continue;
                            }
                            if self.column_to_row.find(&v.col()) == Some(&i) {
                                overlap += 1;
                            }
                        }
                        if overlap == self.row_length {
                            self.row_is_set_packing[i] = packing;
                            self.num_set_packing_rows += 1;
                            if self.num_set_packing_rows == self.num_rows {
                                break;
                            }
                        }
                    }
                    if self.num_set_packing_rows == self.num_rows {
                        break;
                    }
                }
                if self.num_set_packing_rows == self.num_rows {
                    break 'outer;
                }
            }
        }
    }

    /// HiGHS's `orbitalFixingForPackingOrbitope`.
    fn orbital_fixing_for_packing_orbitope(&self, rows: &[usize], domain: &mut Domain, env: &mut DomainEnv) -> usize {
        let num_dynamic_rows = rows.len();
        let mut first_one_in_row: Vec<Option<usize>> = vec![None; num_dynamic_rows];
        for j in 0..self.row_length {
            for i in 0..num_dynamic_rows {
                if first_one_in_row[i].is_some() {
                    continue;
                }
                let r = rows[i];
                let colrj = self.entry(r, j);
                if self.row_is_set_packing[r] == RowPacking::Packing {
                    if domain.col_lower[colrj] > 0.5 {
                        first_one_in_row[i] = Some(j);
                    }
                } else if domain.col_upper[colrj] < 0.5 {
                    first_one_in_row[i] = Some(j);
                }
            }
        }
        let after = |first: Option<usize>, j: usize| first.is_some_and(|f| f > j);
        let mut j = 0;
        let mut num_fixed = 0;
        let fix_zero = |domain: &mut Domain, env: &mut DomainEnv, negate: bool, col: usize| -> bool {
            if negate {
                if domain.col_lower[col] > 0.5 {
                    return false;
                }
                change(domain, env, col, BoundType::Lower, 1.0);
            } else {
                if domain.col_upper[col] < 0.5 {
                    return false;
                }
                change(domain, env, col, BoundType::Upper, 0.0);
            }
            true
        };
        let mut finished_rows = false;
        for i in 0..num_dynamic_rows {
            if after(first_one_in_row[i], j) {
                domain.mark_infeasible();
                return num_fixed;
            }
            let col_ij = self.entry(rows[i], j);
            let negate_i = self.row_is_set_packing[rows[i]] == RowPacking::PackingNegated;
            let not_zero_fixed = if negate_i { domain.col_lower[col_ij] < 0.5 } else { domain.col_upper[col_ij] > 0.5 };
            if not_zero_fixed {
                let mut j0 = j;
                for k in i + 1..num_dynamic_rows {
                    if after(first_one_in_row[k], j0) {
                        if negate_i {
                            change(domain, env, col_ij, BoundType::Upper, 0.0);
                        } else {
                            change(domain, env, col_ij, BoundType::Lower, 1.0);
                        }
                        num_fixed += 1;
                        if domain.infeasible() {
                            return num_fixed;
                        }
                        break;
                    }
                    let negate_k = self.row_is_set_packing[rows[k]] == RowPacking::PackingNegated;
                    let not_zero_fixed = if negate_k {
                        domain.col_lower[self.entry(rows[k], j0)] < 0.5
                    } else {
                        domain.col_upper[self.entry(rows[k], j0)] > 0.5
                    };
                    if not_zero_fixed {
                        j0 += 1;
                        if j0 == self.row_length {
                            break;
                        }
                    }
                }
                j += 1;
                if j == self.row_length {
                    finished_rows = true;
                    break;
                }
                for k in 0..=i {
                    let col_kj = self.entry(rows[k], j);
                    let negate_k = self.row_is_set_packing[rows[k]] == RowPacking::PackingNegated;
                    if !fix_zero(domain, env, negate_k, col_kj) {
                        continue;
                    }
                    num_fixed += 1;
                    if domain.infeasible() {
                        return num_fixed;
                    }
                }
            }
        }
        let _ = finished_rows;
        loop {
            j += 1;
            if j >= self.row_length {
                break;
            }
            for k in 0..num_dynamic_rows {
                let col_kj = self.entry(rows[k], j);
                let negate_k = self.row_is_set_packing[rows[k]] == RowPacking::PackingNegated;
                if !fix_zero(domain, env, negate_k, col_kj) {
                    continue;
                }
                num_fixed += 1;
                if domain.infeasible() {
                    return num_fixed;
                }
            }
        }
        if !domain.infeasible() && num_fixed != 0 {
            domain.propagate(env);
        }
        num_fixed
    }

    /// HiGHS's `orbitalFixingForFullOrbitope`.
    fn orbital_fixing_for_full_orbitope(&self, rows: &[usize], domain: &mut Domain, env: &mut DomainEnv) -> usize {
        let n = rows.len();
        let len = self.row_length;
        let mut mminimal = vec![-1i8; n * len];
        for j in 0..len {
            for i in 0..n {
                let colij = self.matrix[rows[i] + j * self.num_rows];
                if domain.col_lower[colij] == 1.0 {
                    mminimal[i + j * n] = 1;
                } else if domain.col_upper[colij] == 0.0 {
                    mminimal[i + j * n] = 0;
                }
            }
        }
        let mut mmaximal = mminimal.clone();
        for k in 0..n {
            if mminimal[n * (len - 1) + k] == -1 {
                mminimal[n * (len - 1) + k] = 0;
            }
            if mmaximal[k] == -1 {
                mmaximal[k] = 1;
            }
        }
        let i_fixed = |colj0: &[i8], colj1: &[i8]| (0..n).find(|&i| colj0[i] != -1 && colj1[i] != -1 && colj0[i] != colj1[i]).unwrap_or(n);
        let i_discr = |colj0: &[i8], colj1: &[i8], i_f: usize| (0..=i_f.min(n.saturating_sub(1))).rev().find(|&i| i <= i_f && colj0[i] != 0 && colj1[i] != 1);
        let fill = |colj0: &mut [i8], colj1: &[i8], upto: usize| {
            for k in 0..upto {
                let is_free = i8::from(colj0[k] == -1);
                colj0[k] = colj0[k].wrapping_add((is_free & colj1[k]).wrapping_add(is_free));
            }
        };
        for j in (0..len.saturating_sub(1)).rev() {
            let (head, tail) = mminimal.split_at_mut((j + 1) * n);
            let colj0 = &mut head[j * n..];
            let colj1 = &tail[..n];
            let i_f = i_fixed(colj0, colj1);
            if i_f == n {
                fill(colj0, colj1, n);
            } else {
                let i_d = if i_f == n { None } else { i_discr(colj0, colj1, i_f) };
                let Some(i_d) = i_d else {
                    domain.mark_infeasible();
                    return 0;
                };
                fill(colj0, colj1, i_d);
                colj0[i_d] = 1;
                for k in i_d + 1..n {
                    colj0[k] += i8::from(colj0[k] == -1);
                }
            }
        }
        for j in 1..len {
            let (head, tail) = mmaximal.split_at_mut(j * n);
            let colj1 = &head[(j - 1) * n..];
            let colj0 = &mut tail[..n];
            let i_f = i_fixed(colj1, colj0);
            if i_f == n {
                fill(colj0, colj1, n);
            } else {
                let Some(i_d) = i_discr(colj1, colj0, i_f) else {
                    domain.mark_infeasible();
                    return 0;
                };
                fill(colj0, colj1, i_d);
                colj0[i_d] = 0;
                for k in i_d + 1..n {
                    colj0[k] += 2 * i8::from(colj0[k] == -1);
                }
            }
        }
        let mut num_fixed = 0;
        'columns: for j in 0..len {
            for i in 0..n {
                let (lo, hi) = (mminimal[j * n + i], mmaximal[j * n + i]);
                if lo != hi {
                    break;
                }
                let colrj = self.matrix[rows[i] + j * self.num_rows];
                if domain.is_fixed(colrj) {
                    continue;
                }
                num_fixed += 1;
                if lo == 1 {
                    change(domain, env, colrj, BoundType::Lower, 1.0);
                } else {
                    change(domain, env, colrj, BoundType::Upper, 0.0);
                }
                if domain.infeasible() {
                    break 'columns;
                }
            }
        }
        if !domain.infeasible() {
            domain.propagate(env);
        }
        num_fixed
    }

    /// HiGHS's `orbitalFixing`.
    pub fn orbital_fixing(&self, domain: &mut Domain, env: &mut DomainEnv) -> usize {
        let mut rows = Vec::with_capacity(self.num_rows);
        let mut row_used = vec![false; self.num_rows];
        let mut is_packing = true;
        for &pos in domain.branching_positions() {
            if let Some(&i) = self.column_to_row.find(&domain.domain_change_stack()[pos].column)
                && !row_used[i]
            {
                row_used[i] = true;
                is_packing = is_packing && self.row_is_set_packing[i] > RowPacking::NotPacking;
                rows.push(i);
            }
        }
        if rows.is_empty() {
            return 0;
        }
        if is_packing {
            return self.orbital_fixing_for_packing_orbitope(&rows, domain, env);
        }
        self.orbital_fixing_for_full_orbitope(&rows, domain, env)
    }
}

impl Symmetries {
    /// HiGHS's `clear`.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// HiGHS's `getOrbit` on a partition.
    fn orbit_in(&self, col: usize, orbit_partition: &mut [usize], link_compression_stack: &mut Vec<usize>) -> Option<usize> {
        let mut i = self.column_position[col]?;
        let mut orbit = orbit_partition[i];
        if orbit != orbit_partition[orbit] {
            loop {
                link_compression_stack.push(i);
                i = orbit;
                orbit = orbit_partition[orbit];
                if orbit == orbit_partition[orbit] {
                    break;
                }
            }
            while let Some(i) = link_compression_stack.pop() {
                orbit_partition[i] = orbit;
            }
        }
        Some(orbit)
    }

    /// HiGHS's `mergeOrbits` on a partition.
    fn merge_orbits_in(&self, v1: usize, v2: usize, orbit_partition: &mut [usize], orbit_size: &mut [usize], link_compression_stack: &mut Vec<usize>) {
        if v1 == v2 {
            return;
        }
        let orbit1 = self.orbit_in(v1, orbit_partition, link_compression_stack).expect("a permuted column");
        let orbit2 = self.orbit_in(v2, orbit_partition, link_compression_stack).expect("a permuted column");
        if orbit1 == orbit2 {
            return;
        }
        if orbit_size[orbit2] < orbit_size[orbit1] {
            orbit_partition[orbit2] = orbit1;
            orbit_size[orbit1] += orbit_size[orbit2];
        } else {
            orbit_partition[orbit1] = orbit2;
            orbit_size[orbit2] += orbit_size[orbit1];
        }
    }

    /// HiGHS's `propagateOrbitopes`.
    pub fn propagate_orbitopes(&self, domain: &mut Domain, env: &mut DomainEnv) -> usize {
        if self.column_to_orbitope.is_empty() || domain.branch_depth() == 0 {
            return 0;
        }
        let mut propagation_orbitopes = BTreeSet::new();
        for &pos in domain.branching_positions() {
            if let Some(&orbitope) = self.column_to_orbitope.find(&domain.domain_change_stack()[pos].column) {
                propagation_orbitopes.insert(orbitope);
            }
        }
        let mut num_fixed = 0;
        for i in propagation_orbitopes {
            num_fixed += self.orbitopes[i].orbital_fixing(domain, env);
            if domain.infeasible() {
                break;
            }
        }
        num_fixed
    }

    /// HiGHS's `computeStabilizerOrbits`.
    pub fn compute_stabilizer_orbits(&self, localdom: &Domain, model: &super::MipModel, workspace: &mut StabilizerOrbitWorkspace) -> Rc<StabilizerOrbits> {
        let domchg_stack = localdom.domain_change_stack();
        let is_global_binary = |col: usize| model.is_col_integral(col) && model.col_lower[col] == 0.0 && model.col_upper[col] == 1.0;
        let mut stabilizer_orbits = StabilizerOrbits::default();
        stabilizer_orbits.stabilized_cols.reserve(self.permutation_columns.len());
        for &i in localdom.branching_positions() {
            let col = domchg_stack[i].column;
            let Some(position) = self.column_position[col] else { continue };
            if domchg_stack[i].boundtype == BoundType::Lower || !is_global_binary(col) {
                stabilizer_orbits.stabilized_cols.push(position);
            }
        }
        let perm_length = self.permutation_columns.len();
        workspace.orbit_partition = (0..perm_length).collect();
        workspace.orbit_size = vec![1; perm_length];
        workspace.link_compression_stack.clear();
        for p in 0..self.num_perms {
            let perm = &self.permutations[p * perm_length..(p + 1) * perm_length];
            let respects = stabilizer_orbits.stabilized_cols.iter().all(|&i| self.permutation_columns[i] == perm[i]);
            if !respects {
                continue;
            }
            for j in 0..perm_length {
                self.merge_orbits_in(
                    self.permutation_columns[j],
                    perm[j],
                    &mut workspace.orbit_partition,
                    &mut workspace.orbit_size,
                    &mut workspace.link_compression_stack,
                );
            }
        }
        stabilizer_orbits.stabilized_cols.clear();
        stabilizer_orbits.orbit_cols.reserve(perm_length);
        for i in 0..perm_length {
            let col = self.permutation_columns[i];
            if model.is_col_continuous(col) {
                continue;
            }
            let orbit = self
                .orbit_in(col, &mut workspace.orbit_partition, &mut workspace.link_compression_stack)
                .expect("a permuted column");
            if workspace.orbit_size[orbit] == 1 {
                stabilizer_orbits.stabilized_cols.push(col);
            } else if is_global_binary(col) {
                stabilizer_orbits.orbit_cols.push(col);
            }
        }
        pdqsort(&mut stabilizer_orbits.stabilized_cols, |a, b| a < b);
        if !stabilizer_orbits.orbit_cols.is_empty() {
            let StabilizerOrbitWorkspace {
                orbit_partition,
                link_compression_stack,
                ..
            } = workspace;
            let mut orbit = |col: usize| self.orbit_in(col, orbit_partition, link_compression_stack).expect("a permuted column");
            pdqsort(&mut stabilizer_orbits.orbit_cols, |&a, &b| orbit(a) < orbit(b));
            let num_orbit_cols = stabilizer_orbits.orbit_cols.len();
            stabilizer_orbits.orbit_starts.reserve(num_orbit_cols + 1);
            stabilizer_orbits.orbit_starts.push(0);
            for i in 1..num_orbit_cols {
                if orbit(stabilizer_orbits.orbit_cols[i]) != orbit(stabilizer_orbits.orbit_cols[i - 1]) {
                    stabilizer_orbits.orbit_starts.push(i);
                }
            }
            stabilizer_orbits.orbit_starts.push(num_orbit_cols);
        }
        Rc::new(stabilizer_orbits)
    }
}
