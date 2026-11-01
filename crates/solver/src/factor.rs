//! HiGHS's `HFactor`: the basis matrix factored as `PBQ = LU` by Markowitz
//! pivoting after singletons, solves with it (FTRAN and BTRAN, sparse or
//! hyper-sparse), and Forrest-Tomlin updates as the basis changes.
//!
//! Only the Forrest-Tomlin update of HiGHS's four is ported: it is the one
//! the simplex solver uses.

use crate::hvector::{HVector, TINY, ZERO};

pub(crate) const MIN_PIVOT_THRESHOLD: f64 = 8e-4;
pub(crate) const DEFAULT_PIVOT_THRESHOLD: f64 = 0.1;
pub(crate) const PIVOT_THRESHOLD_CHANGE_FACTOR: f64 = 5.0;
pub(crate) const MAX_PIVOT_THRESHOLD: f64 = 0.5;
pub(crate) const DEFAULT_PIVOT_TOLERANCE: f64 = 1e-10;

const HYPER_FTRAN_L: f64 = 0.15;
const HYPER_FTRAN_U: f64 = 0.10;
const HYPER_BTRAN_L: f64 = 0.10;
const HYPER_BTRAN_U: f64 = 0.15;
const HYPER_CANCEL: f64 = 0.05;

/// A column-wise sparse matrix, borrowed: column `j`'s rows are
/// `index[start[j]..start[j + 1]]`.
#[derive(Clone, Copy)]
pub(crate) struct ColMatrix<'a> {
    pub start: &'a [usize],
    pub index: &'a [usize],
    pub value: &'a [f64],
}

/// An index that is not one.
const NONE: usize = usize::MAX;

#[derive(Clone, Default)]
pub(crate) struct Factor {
    pub num_row: usize,
    pub num_col: usize,
    /// The number of basic variables, which only differs from `num_row`
    /// when finding the rank of a set of columns.
    pub num_basic: usize,
    inv_num_row: f64,
    pivot_threshold: f64,
    pivot_tolerance: f64,

    /// Synthetic clock of the last build.
    pub build_synthetic_tick: f64,
    /// Seconds a build may take, as HiGHS's `setTimeLimit` (only presolve's
    /// search for dependent equations sets one).
    pub time_limit: Option<f64>,
    /// The last build gave up, expecting to pass `time_limit`
    /// (HiGHS's `kBuildKernelReturnTimeout`).
    pub timed_out: bool,
    pub rank_deficiency: usize,
    pub row_with_no_pivot: Vec<usize>,
    pub col_with_no_pivot: Vec<usize>,
    pub var_with_no_pivot: Vec<usize>,
    pub basis_matrix_num_el: usize,
    pub invert_num_el: usize,
    pub kernel_dim: usize,
    pub kernel_num_el: usize,

    nwork: isize,
    iwork: Vec<isize>,

    b_var: Vec<usize>,
    b_start: Vec<usize>,
    b_index: Vec<usize>,
    b_value: Vec<f64>,

    permute: Vec<isize>,

    mc_var: Vec<usize>,
    mc_start: Vec<u32>,
    mc_count_a: Vec<u32>,
    mc_count_n: Vec<u32>,
    mc_space: Vec<u32>,
    mc_index: Vec<u32>,
    mc_value: Vec<f64>,
    mc_min_pivot: Vec<f64>,

    mr_start: Vec<u32>,
    mr_count: Vec<u32>,
    mr_space: Vec<u32>,
    mr_count_before: Vec<isize>,
    mr_index: Vec<u32>,

    mwz_column_index: Vec<usize>,
    mwz_column_mark: Vec<u8>,
    mwz_column_array: Vec<f64>,

    col_link_first: Vec<isize>,
    col_link_next: Vec<isize>,
    col_link_last: Vec<isize>,
    row_link_first: Vec<isize>,
    row_link_next: Vec<isize>,
    row_link_last: Vec<isize>,

    l_pivot_lookup: Vec<usize>,
    l_pivot_index: Vec<usize>,
    l_start: Vec<usize>,
    l_index: Vec<usize>,
    l_value: Vec<f64>,
    lr_start: Vec<usize>,
    lr_index: Vec<usize>,
    lr_value: Vec<f64>,

    u_pivot_lookup: Vec<usize>,
    /// [`NONE`] marks a pivot replaced by an update.
    u_pivot_index: Vec<usize>,
    u_pivot_value: Vec<f64>,
    u_merit_x: usize,
    u_total_x: usize,
    u_start: Vec<usize>,
    u_last_p: Vec<usize>,
    u_index: Vec<usize>,
    u_value: Vec<f64>,
    ur_start: Vec<usize>,
    ur_lastp: Vec<usize>,
    ur_space: Vec<usize>,
    ur_index: Vec<usize>,
    ur_value: Vec<f64>,

    pf_pivot_index: Vec<usize>,
    pf_start: Vec<usize>,
    pf_index: Vec<usize>,
    pf_value: Vec<f64>,
}

/// HiGHS's `InvertibleRepresentation`: what `getInvert` saves of a
/// factor and `setInvert` puts back (nothing else of it).
#[derive(Clone, Debug, Default)]
pub(crate) struct Invert {
    l_pivot_index: Vec<usize>,
    l_pivot_lookup: Vec<usize>,
    l_start: Vec<usize>,
    l_index: Vec<usize>,
    l_value: Vec<f64>,
    lr_start: Vec<usize>,
    lr_index: Vec<usize>,
    lr_value: Vec<f64>,
    u_pivot_lookup: Vec<usize>,
    u_pivot_index: Vec<usize>,
    u_pivot_value: Vec<f64>,
    u_start: Vec<usize>,
    u_last_p: Vec<usize>,
    u_index: Vec<usize>,
    u_value: Vec<f64>,
    ur_start: Vec<usize>,
    ur_lastp: Vec<usize>,
    ur_space: Vec<usize>,
    ur_index: Vec<usize>,
    ur_value: Vec<f64>,
    pf_start: Vec<usize>,
    pf_index: Vec<usize>,
    pf_value: Vec<f64>,
    pf_pivot_index: Vec<usize>,
}

impl Factor {
    /// HiGHS's `getInvert`.
    pub fn get_invert(&self) -> Invert {
        Invert {
            l_pivot_index: self.l_pivot_index.clone(),
            l_pivot_lookup: self.l_pivot_lookup.clone(),
            l_start: self.l_start.clone(),
            l_index: self.l_index.clone(),
            l_value: self.l_value.clone(),
            lr_start: self.lr_start.clone(),
            lr_index: self.lr_index.clone(),
            lr_value: self.lr_value.clone(),
            u_pivot_lookup: self.u_pivot_lookup.clone(),
            u_pivot_index: self.u_pivot_index.clone(),
            u_pivot_value: self.u_pivot_value.clone(),
            u_start: self.u_start.clone(),
            u_last_p: self.u_last_p.clone(),
            u_index: self.u_index.clone(),
            u_value: self.u_value.clone(),
            ur_start: self.ur_start.clone(),
            ur_lastp: self.ur_lastp.clone(),
            ur_space: self.ur_space.clone(),
            ur_index: self.ur_index.clone(),
            ur_value: self.ur_value.clone(),
            pf_start: self.pf_start.clone(),
            pf_index: self.pf_index.clone(),
            pf_value: self.pf_value.clone(),
            pf_pivot_index: self.pf_pivot_index.clone(),
        }
    }

    /// HiGHS's `setInvert`.
    pub fn set_invert(&mut self, invert: &Invert) {
        self.l_pivot_index.clone_from(&invert.l_pivot_index);
        self.l_pivot_lookup.clone_from(&invert.l_pivot_lookup);
        self.l_start.clone_from(&invert.l_start);
        self.l_index.clone_from(&invert.l_index);
        self.l_value.clone_from(&invert.l_value);
        self.lr_start.clone_from(&invert.lr_start);
        self.lr_index.clone_from(&invert.lr_index);
        self.lr_value.clone_from(&invert.lr_value);
        self.u_pivot_lookup.clone_from(&invert.u_pivot_lookup);
        self.u_pivot_index.clone_from(&invert.u_pivot_index);
        self.u_pivot_value.clone_from(&invert.u_pivot_value);
        self.u_start.clone_from(&invert.u_start);
        self.u_last_p.clone_from(&invert.u_last_p);
        self.u_index.clone_from(&invert.u_index);
        self.u_value.clone_from(&invert.u_value);
        self.ur_start.clone_from(&invert.ur_start);
        self.ur_lastp.clone_from(&invert.ur_lastp);
        self.ur_space.clone_from(&invert.ur_space);
        self.ur_index.clone_from(&invert.ur_index);
        self.ur_value.clone_from(&invert.ur_value);
        self.pf_start.clone_from(&invert.pf_start);
        self.pf_index.clone_from(&invert.pf_index);
        self.pf_value.clone_from(&invert.pf_value);
        self.pf_pivot_index.clone_from(&invert.pf_pivot_index);
    }

    /// Size the factor for `num_row` rows and `num_col` structural columns,
    /// logical `i` being variable `num_col + i`.
    pub fn setup(&mut self, num_col: usize, num_row: usize, pivot_threshold: f64, pivot_tolerance: f64) {
        self.setup_general(num_col, num_row, num_row, pivot_threshold, pivot_tolerance);
    }

    /// [`Factor::setup`] for `num_basic` basic variables, as HiGHS's
    /// `setupGeneral`: with fewer than `num_row` the build only finds
    /// which are dependent.
    pub fn setup_general(&mut self, num_col: usize, num_row: usize, num_basic: usize, pivot_threshold: f64, pivot_tolerance: f64) {
        self.num_row = num_row;
        self.num_col = num_col;
        self.num_basic = num_basic;
        self.inv_num_row = 1.0 / num_row as f64;
        self.pivot_threshold = pivot_threshold.clamp(MIN_PIVOT_THRESHOLD, MAX_PIVOT_THRESHOLD);
        self.pivot_tolerance = pivot_tolerance.clamp(0.0, 1.0);

        let max_dim = num_row.max(num_basic);
        self.b_var = vec![0; max_dim];
        self.b_start = vec![0; max_dim + 1];
        self.permute = vec![-1; max_dim];
        self.mc_var = vec![0; num_basic];
        self.mc_start = vec![0; num_basic];
        self.mc_count_a = vec![0; num_basic];
        self.mc_count_n = vec![0; num_basic];
        self.mc_space = vec![0; num_basic];
        self.mc_min_pivot = vec![0.0; num_basic];
        self.mr_start = vec![0; num_row];
        self.mr_count = vec![0; num_row];
        self.mr_space = vec![0; num_row];
        self.mr_count_before = vec![0; num_row];
        self.mwz_column_mark = vec![0; num_row];
        self.mwz_column_index = vec![0; num_row];
        self.mwz_column_array = vec![0.0; num_row];
        self.col_link_first = vec![-1; num_row + 1];
        self.col_link_next = vec![0; num_basic];
        self.col_link_last = vec![0; num_basic];
        self.row_link_first = vec![-1; num_basic + 1];
        self.row_link_next = vec![0; num_row];
        self.row_link_last = vec![0; num_row];
        self.l_pivot_lookup = vec![0; num_row];
        self.u_pivot_lookup = vec![0; num_row];
    }

    pub fn set_pivot_threshold(&mut self, threshold: f64) -> bool {
        if !(MIN_PIVOT_THRESHOLD..=MAX_PIVOT_THRESHOLD).contains(&threshold) {
            return false;
        }
        self.pivot_threshold = threshold;
        true
    }

    /// Factor the basis `basic_index` of `a`, permuting `basic_index` to
    /// the pivot order. Returns the rank deficiency: when it is not zero,
    /// the variables with no pivot have been replaced in `basic_index` by
    /// logicals, and are listed in `var_with_no_pivot`.
    ///
    /// With fewer basic variables than rows, the build stops once it
    /// knows which basic variables are dependent: it returns how many
    /// are, and `var_with_no_pivot` starts with them, followed by
    /// [`NONE`]s for the rows no basic variable could have pivoted on.
    pub fn build(&mut self, a: ColMatrix, basic_index: &mut [usize]) -> usize {
        self.build_synthetic_tick = 0.0;
        self.timed_out = false;
        let started = web_time::Instant::now();
        self.build_simple(a, basic_index);
        let Some(rank_deficiency) = self.build_kernel(basic_index, started) else {
            self.timed_out = true;
            return 0;
        };
        self.rank_deficiency = rank_deficiency;
        let incomplete_basis = self.num_basic < self.num_row;
        if self.rank_deficiency > 0 || incomplete_basis {
            self.build_handle_rank_deficiency(basic_index);
            self.build_mark_sing_c(basic_index);
        }
        if incomplete_basis {
            return self.rank_deficiency - (self.num_row - self.num_basic);
        }
        self.build_finish(basic_index);
        self.invert_num_el = self.l_start[self.num_row] + self.u_last_p[self.num_row - 1] + self.num_row;
        self.kernel_dim -= self.rank_deficiency;
        self.rank_deficiency
    }

    fn lu_clear(&mut self) {
        self.l_start.clear();
        self.l_start.push(0);
        self.l_index.clear();
        self.l_value.clear();
        self.u_pivot_index.clear();
        self.u_pivot_value.clear();
        self.u_start.clear();
        self.u_start.push(0);
        self.u_index.clear();
        self.u_value.clear();
    }

    fn build_simple(&mut self, a: ColMatrix, basic_index: &[usize]) {
        self.lu_clear();
        let num_row = self.num_row;
        let num_col = self.num_col;
        let num_basic = self.num_basic;
        self.permute.clear();
        self.permute.resize(num_basic, -1);

        // 1. The basis matrix, with unit columns pivoted on at once.
        self.mr_count_before.fill(0);
        self.nwork = 0;
        self.iwork.clear();
        self.iwork.resize(num_basic + 1, 0);
        self.b_index.clear();
        self.b_value.clear();
        for i_col in 0..num_basic {
            let i_mat = basic_index[i_col];
            let mut i_row = NONE;
            if i_mat >= num_col {
                let lc_row = i_mat - num_col;
                if self.mr_count_before[lc_row] >= 0 {
                    i_row = lc_row;
                } else {
                    self.mr_count_before[lc_row] += 1;
                    self.b_index.push(lc_row);
                    self.b_value.push(1.0);
                    self.iwork[self.nwork as usize] = i_col as isize;
                    self.nwork += 1;
                }
            } else {
                let start = a.start[i_mat];
                let count = a.start[i_mat + 1] - start;
                let ok_unit_col = count == 1 && a.value[start] == 1.0 && self.mr_count_before[a.index[start]] >= 0;
                if ok_unit_col {
                    i_row = a.index[start];
                } else {
                    for k in start..start + count {
                        self.mr_count_before[a.index[k]] += 1;
                        self.b_index.push(a.index[k]);
                        self.b_value.push(a.value[k]);
                    }
                    self.iwork[self.nwork as usize] = i_col as isize;
                    self.nwork += 1;
                }
            }
            if i_row != NONE {
                self.permute[i_col] = i_row as isize;
                self.l_start.push(self.l_index.len());
                self.u_pivot_index.push(i_row);
                self.u_pivot_value.push(1.0);
                self.u_start.push(self.u_index.len());
                self.mr_count_before[i_row] = -(num_basic as isize);
            }
            self.b_start[i_col + 1] = self.b_index.len();
            self.b_var[i_col] = i_mat;
        }
        let b_count = self.b_index.len();
        // Signed: with more basic variables than rows, nwork can pass num_row.
        let unit_count = num_row as isize - self.nwork;
        self.basis_matrix_num_el = (unit_count + b_count as isize) as usize;
        self.build_synthetic_tick += (b_count as isize * 60 + unit_count * 80) as f64;

        // 2. Singletons, row and column, until none are left.
        let mut t2_search = 0.0;
        let t2_store_l0 = self.l_index.len() as f64;
        let t2_store_u0 = self.u_index.len() as f64;
        let t2_store_p0 = self.nwork as f64;
        while self.nwork > 0 {
            let nwork_last = self.nwork as usize;
            self.nwork = 0;
            for i in 0..nwork_last {
                let i_col = self.iwork[i] as usize;
                let start = self.b_start[i_col];
                let end = self.b_start[i_col + 1];
                let mut pivot_k = NONE;
                let mut found_row_singleton = false;
                let mut count = 0;
                t2_search += (end - start) as f64;
                for k in start..end {
                    let i_row = self.b_index[k];
                    if self.mr_count_before[i_row] == 1 {
                        pivot_k = k;
                        found_row_singleton = true;
                        break;
                    }
                    if self.mr_count_before[i_row] > 1 {
                        pivot_k = k;
                        count += 1;
                    }
                }
                if found_row_singleton {
                    let pivot_multiplier = 1.0 / self.b_value[pivot_k];
                    for (p0, p1) in [(start, pivot_k), (pivot_k + 1, end)] {
                        for k in p0..p1 {
                            let i_row = self.b_index[k];
                            if self.mr_count_before[i_row] > 0 {
                                self.l_index.push(i_row);
                                self.l_value.push(self.b_value[k] * pivot_multiplier);
                            } else {
                                self.u_index.push(i_row);
                                self.u_value.push(self.b_value[k]);
                            }
                            self.mr_count_before[i_row] -= 1;
                        }
                    }
                    let i_row = self.b_index[pivot_k];
                    self.mr_count_before[i_row] = 0;
                    self.permute[i_col] = i_row as isize;
                    self.l_start.push(self.l_index.len());
                    self.u_pivot_index.push(i_row);
                    self.u_pivot_value.push(self.b_value[pivot_k]);
                    self.u_start.push(self.u_index.len());
                } else if count == 1 {
                    for k in (start..pivot_k).chain(pivot_k + 1..end) {
                        self.u_index.push(self.b_index[k]);
                        self.u_value.push(self.b_value[k]);
                    }
                    let i_row = self.b_index[pivot_k];
                    self.mr_count_before[i_row] = 0;
                    self.permute[i_col] = i_row as isize;
                    self.l_start.push(self.l_index.len());
                    self.u_pivot_index.push(i_row);
                    self.u_pivot_value.push(self.b_value[pivot_k]);
                    self.u_start.push(self.u_index.len());
                } else {
                    self.iwork[self.nwork as usize] = i_col as isize;
                    self.nwork += 1;
                }
            }
            if nwork_last == self.nwork as usize {
                break;
            }
        }
        let t2_store_l = self.l_index.len() as f64 - t2_store_l0;
        let t2_store_u = self.u_index.len() as f64 - t2_store_u0;
        let t2_store_p = t2_store_p0 - self.nwork as f64;
        self.build_synthetic_tick += t2_search * 20.0 + (t2_store_p + t2_store_l + t2_store_u) * 80.0;

        // 3. The kernel, row-wise and column-wise, with count links.
        self.row_link_first.clear();
        self.row_link_first.resize(num_basic + 1, -1);
        self.mr_count.fill(0);
        let mut mr_count_x = 0;
        self.kernel_num_el = 0;
        for i_row in 0..num_row {
            let count = self.mr_count_before[i_row];
            if count > 0 {
                let count = count as usize;
                self.mr_start[i_row] = (mr_count_x) as u32;
                self.mr_space[i_row] = (count * 2) as u32;
                mr_count_x += count * 2;
                self.rlink_add(i_row, count);
                self.kernel_num_el += count + 1;
            }
        }
        self.mr_index.clear();
        self.mr_index.resize(mr_count_x, 0);

        self.col_link_first.clear();
        self.col_link_first.resize(num_row + 1, -1);
        self.mc_index.clear();
        self.mc_value.clear();
        self.mc_count_a.fill(0);
        self.mc_count_n.fill(0);
        let mut mc_count_x = 0;
        for i in 0..self.nwork as usize {
            let i_col = self.iwork[i] as usize;
            self.mc_var[i_col] = self.b_var[i_col];
            self.mc_start[i_col] = (mc_count_x) as u32;
            self.mc_space[i_col] = ((self.b_start[i_col + 1] - self.b_start[i_col]) * 2) as u32;
            mc_count_x += self.mc_space[i_col] as usize;
            self.mc_index.resize(mc_count_x, 0);
            self.mc_value.resize(mc_count_x, 0.0);
            for k in self.b_start[i_col]..self.b_start[i_col + 1] {
                let i_row = self.b_index[k];
                let value = self.b_value[k];
                if self.mr_count_before[i_row] > 0 {
                    self.col_insert(i_col, i_row, value);
                    self.row_insert(i_col, i_row);
                } else {
                    self.col_store_n(i_col, i_row, value);
                }
            }
            self.col_fix_max(i_col);
            self.clink_add(i_col, self.mc_count_a[i_col] as usize);
        }
        self.build_synthetic_tick += ((num_row + self.nwork as usize + mc_count_x) * 40 + mr_count_x * 20) as f64;
        self.kernel_dim = self.nwork as usize;
    }

    /// Markowitz pivoting on the kernel. Returns its rank deficiency, or
    /// `None` once the time it expects to take passes `time_limit`.
    fn build_kernel(&mut self, basic_index: &[usize], started: web_time::Instant) -> Option<usize> {
        let _ = basic_index;
        let num_row = self.num_row;
        let num_basic = self.num_basic;
        let mut fake_search = 0.0;
        let mut fake_fill = 0.0;
        let mut fake_eliminate = 0.0;
        let mut timer_frequency = 100;
        let mut previous_iteration_time = 0.0;
        let mut average_iteration_time = 0.0;
        let mut search_k = 0usize;

        loop {
            if self.nwork <= 0 {
                self.nwork -= 1;
                break;
            }
            self.nwork -= 1;
            if let Some(time_limit) = self.time_limit
                && search_k.is_multiple_of(timer_frequency)
            {
                let current_time = started.elapsed().as_secs_f64();
                let time_difference = current_time - previous_iteration_time;
                previous_iteration_time = current_time;
                let iteration_time = time_difference / timer_frequency as f64;
                average_iteration_time = 0.9 * average_iteration_time + 0.1 * iteration_time;
                if time_difference > time_limit / 1e3 {
                    timer_frequency = (timer_frequency / 10).max(1);
                }
                let iterations_left = (self.kernel_dim as f64) - search_k as f64 + 1.0;
                let total_time_bound = current_time + average_iteration_time * iterations_left;
                if current_time > time_limit || total_time_bound > time_limit {
                    return None;
                }
            }
            search_k += 1;
            let nwork = self.nwork as usize;

            // 1. Search for the pivot.
            let mut j_col_pivot = NONE;
            let mut i_row_pivot = NONE;
            let search_limit = nwork.min(8);
            let mut search_count = 0;
            let merit_limit = num_basic as f64 * num_row as f64;
            let mut merit_pivot = merit_limit;

            let mut found_pivot = false;
            if self.col_link_first[1] != -1 {
                j_col_pivot = self.col_link_first[1] as usize;
                i_row_pivot = self.mc_index[self.mc_start[j_col_pivot] as usize] as usize;
                found_pivot = true;
            }
            if !found_pivot && self.row_link_first[1] != -1 {
                i_row_pivot = self.row_link_first[1] as usize;
                j_col_pivot = self.mr_index[self.mr_start[i_row_pivot] as usize] as usize;
                found_pivot = true;
            }
            let max_count = num_row.max(num_basic);
            let mut count = 2;
            while !found_pivot && count <= max_count {
                if count <= num_row {
                    let mut j = self.col_link_first[count];
                    while j != -1 {
                        let ju = j as usize;
                        let min_pivot = self.mc_min_pivot[ju];
                        let start = self.mc_start[ju] as usize;
                        let end = start + (self.mc_count_a[ju] as usize);
                        for k in start..end {
                            if self.mc_value[k].abs() >= min_pivot {
                                let i = self.mc_index[k] as usize;
                                let row_count = self.mr_count[i] as usize;
                                let merit_local = (count - 1) as f64 * (row_count as f64 - 1.0);
                                if merit_pivot > merit_local {
                                    merit_pivot = merit_local;
                                    j_col_pivot = ju;
                                    i_row_pivot = i;
                                    found_pivot = found_pivot || row_count < count;
                                }
                            }
                        }
                        let searched = search_count;
                        search_count += 1;
                        if searched >= search_limit && merit_pivot < merit_limit {
                            found_pivot = true;
                        }
                        if found_pivot {
                            break;
                        }
                        fake_search += count as f64;
                        j = self.col_link_next[ju];
                    }
                }
                if count <= num_basic {
                    let mut i = self.row_link_first[count];
                    while i != -1 {
                        let iu = i as usize;
                        let start = self.mr_start[iu] as usize;
                        let end = start + (self.mr_count[iu] as usize);
                        for k in start..end {
                            let j = self.mr_index[k] as usize;
                            let column_count = self.mc_count_a[j] as usize;
                            let merit_local = (count - 1) as f64 * (column_count as f64 - 1.0);
                            if merit_local < merit_pivot {
                                let ifind = find_entry(&self.mc_index, self.mc_start[j] as usize, iu);
                                if self.mc_value[ifind].abs() >= self.mc_min_pivot[j] {
                                    merit_pivot = merit_local;
                                    j_col_pivot = j;
                                    i_row_pivot = iu;
                                    found_pivot = found_pivot || column_count <= count;
                                }
                            }
                        }
                        let searched = search_count;
                        search_count += 1;
                        if searched >= search_limit && merit_pivot < merit_limit {
                            found_pivot = true;
                        }
                        if found_pivot {
                            break;
                        }
                        i = self.row_link_next[iu];
                    }
                    fake_search += count as f64;
                }
                count += 1;
            }
            // 1.4. Nothing left to pivot on: singular.
            if i_row_pivot == NONE {
                return Some(nwork + 1);
            }

            // 2. Eliminate with the pivot.
            let pivot_multiplier = self.col_delete(j_col_pivot, i_row_pivot);
            self.row_delete(j_col_pivot, i_row_pivot);
            self.clink_del(j_col_pivot);
            self.rlink_del(i_row_pivot);
            if pivot_multiplier.abs() < self.pivot_tolerance {
                // Singular, but other valid pivots may remain: defer.
                if (self.mr_count[i_row_pivot] as usize) == 0 {
                    self.clink_add(j_col_pivot, self.mc_count_a[j_col_pivot] as usize);
                } else {
                    self.zero_col(j_col_pivot);
                    self.rlink_add(i_row_pivot, self.mr_count[i_row_pivot] as usize);
                }
                self.nwork += 1;
                continue;
            }
            self.permute[j_col_pivot] = i_row_pivot as isize;

            // 2.2. The active part of the pivot column goes to L.
            let start_a = self.mc_start[j_col_pivot] as usize;
            let end_a = start_a + (self.mc_count_a[j_col_pivot] as usize);
            let mut mwz_column_count = 0;
            for k in start_a..end_a {
                let i_row = self.mc_index[k] as usize;
                let value = self.mc_value[k] / pivot_multiplier;
                self.mwz_column_index[mwz_column_count] = i_row;
                mwz_column_count += 1;
                self.mwz_column_array[i_row] = value;
                self.mwz_column_mark[i_row] = 1;
                self.l_index.push(i_row);
                self.l_value.push(value);
                self.mr_count_before[i_row] = (self.mr_count[i_row] as usize) as isize;
                self.row_delete(j_col_pivot, i_row);
            }
            self.l_start.push(self.l_index.len());
            fake_fill += 2.0 * (self.mc_count_a[j_col_pivot] as usize) as f64;

            // 2.3. Its inactive part goes to U.
            let end_n = start_a + (self.mc_space[j_col_pivot] as usize);
            let start_n = end_n - (self.mc_count_n[j_col_pivot] as usize);
            for i in start_n..end_n {
                self.u_index.push(self.mc_index[i] as usize);
                self.u_value.push(self.mc_value[i]);
            }
            self.u_pivot_index.push(i_row_pivot);
            self.u_pivot_value.push(pivot_multiplier);
            self.u_start.push(self.u_index.len());
            fake_fill += (end_n - start_n) as f64;

            // 2.4. Eliminate along the pivot row.
            let row_start = self.mr_start[i_row_pivot] as usize;
            let row_end = row_start + (self.mr_count[i_row_pivot] as usize);
            for row_k in row_start..row_end {
                let i_col = self.mr_index[row_k] as usize;
                let my_count = self.mc_count_a[i_col] as usize;
                let my_start = self.mc_start[i_col] as usize;
                let my_end = my_start + my_count - 1;
                let my_pivot = self.col_delete(i_col, i_row_pivot);
                self.col_store_n(i_col, i_row_pivot, my_pivot);

                let mut n_fillin = mwz_column_count as isize;
                let mut n_cancel = 0;
                for my_k in my_start..my_end {
                    let i_row = self.mc_index[my_k] as usize;
                    let mut value = self.mc_value[my_k];
                    if self.mwz_column_mark[i_row] != 0 {
                        self.mwz_column_mark[i_row] = 0;
                        n_fillin -= 1;
                        value -= my_pivot * self.mwz_column_array[i_row];
                        if value.abs() < TINY {
                            value = 0.0;
                            n_cancel += 1;
                        }
                        self.mc_value[my_k] = value;
                    }
                }
                fake_eliminate += mwz_column_count as f64;
                fake_eliminate += n_fillin as f64 * 2.0;

                if n_cancel > 0 {
                    let mut new_end = my_start;
                    for my_k in my_start..my_end {
                        if self.mc_value[my_k] != 0.0 {
                            self.mc_index[new_end] = self.mc_index[my_k];
                            self.mc_value[new_end] = self.mc_value[my_k];
                            new_end += 1;
                        } else {
                            let row = self.mc_index[my_k] as usize;
                            self.row_delete(i_col, row);
                        }
                    }
                    self.mc_count_a[i_col] = (new_end - my_start) as u32;
                }

                if n_fillin > 0 {
                    let n_fillin = n_fillin as usize;
                    if (self.mc_count_a[i_col] as usize) + (self.mc_count_n[i_col] as usize) + n_fillin > (self.mc_space[i_col] as usize) {
                        let p1 = self.mc_start[i_col] as usize;
                        let p2 = p1 + (self.mc_count_a[i_col] as usize);
                        let p3 = p1 + (self.mc_space[i_col] as usize) - (self.mc_count_n[i_col] as usize);
                        let p4 = p1 + (self.mc_space[i_col] as usize);
                        self.mc_space[i_col] += (self.mc_space[i_col] as usize).max(n_fillin) as u32;
                        let p5 = self.mc_index.len();
                        self.mc_start[i_col] = (p5) as u32;
                        let p7 = p5 + (self.mc_space[i_col] as usize) - (self.mc_count_n[i_col] as usize);
                        self.mc_index.resize(p5 + (self.mc_space[i_col] as usize), 0);
                        self.mc_value.resize(p5 + (self.mc_space[i_col] as usize), 0.0);
                        self.mc_index.copy_within(p1..p2, p5);
                        self.mc_value.copy_within(p1..p2, p5);
                        self.mc_index.copy_within(p3..p4, p7);
                        self.mc_value.copy_within(p3..p4, p7);
                    }
                    for i in 0..mwz_column_count {
                        let i_row = self.mwz_column_index[i];
                        if self.mwz_column_mark[i_row] != 0 {
                            self.col_insert(i_col, i_row, -my_pivot * self.mwz_column_array[i_row]);
                        }
                    }
                    for i in 0..mwz_column_count {
                        let i_row = self.mwz_column_index[i];
                        if self.mwz_column_mark[i_row] != 0 {
                            if (self.mr_count[i_row] as usize) == (self.mr_space[i_row] as usize) {
                                let p1 = self.mr_start[i_row] as usize;
                                let p2 = p1 + (self.mr_count[i_row] as usize);
                                let p3 = self.mr_index.len();
                                self.mr_start[i_row] = (p3) as u32;
                                self.mr_space[i_row] *= 2_u32;
                                self.mr_index.resize(p3 + (self.mr_space[i_row] as usize), 0);
                                self.mr_index.copy_within(p1..p2, p3);
                            }
                            self.row_insert(i_col, i_row);
                        }
                    }
                }

                for i in 0..mwz_column_count {
                    self.mwz_column_mark[self.mwz_column_index[i]] = 1;
                }
                self.col_fix_max(i_col);
                if my_count != (self.mc_count_a[i_col] as usize) {
                    self.clink_del(i_col);
                    self.clink_add(i_col, self.mc_count_a[i_col] as usize);
                }
            }

            for i in 0..mwz_column_count {
                self.mwz_column_mark[self.mwz_column_index[i]] = 0;
            }
            // 2.6. Row links for what remains active.
            for i in start_a..end_a {
                let i_row = self.mc_index[i] as usize;
                if self.mr_count_before[i_row] != (self.mr_count[i_row] as usize) as isize {
                    self.rlink_del(i_row);
                    self.rlink_add(i_row, self.mr_count[i_row] as usize);
                }
            }
        }
        self.build_synthetic_tick += fake_search * 20.0 + fake_fill * 160.0 + fake_eliminate * 80.0;
        Some(0)
    }

    fn build_handle_rank_deficiency(&mut self, basic_index: &[usize]) {
        let num_row = self.num_row;
        let num_basic = self.num_basic;
        if num_basic < num_row {
            self.rank_deficiency += num_row - num_basic;
        }
        let rank_deficiency = self.rank_deficiency;
        self.row_with_no_pivot = vec![0; rank_deficiency];
        self.col_with_no_pivot = vec![0; rank_deficiency];
        self.iwork.resize(num_row.max(num_basic), 0);
        self.iwork[..num_row].fill(-1);
        let mut lc = 0;
        for i in 0..num_basic {
            let perm = self.permute[i];
            if perm >= 0 {
                self.iwork[perm as usize] = basic_index[i] as isize;
            } else {
                self.col_with_no_pivot[lc] = i;
                lc += 1;
            }
        }
        if num_basic < num_row {
            self.permute.resize(num_row, -1);
            for i in num_basic..num_row {
                self.col_with_no_pivot[lc] = i;
                lc += 1;
                self.permute[i] = -1;
            }
        }
        debug_assert_eq!(lc, rank_deficiency);
        lc = 0;
        for i in 0..num_row {
            if self.iwork[i] < 0 {
                self.row_with_no_pivot[lc] = i;
                self.iwork[i] = -(lc as isize + 1);
                lc += 1;
            }
        }
        // Fictitious rows for basic variables beyond the rows.
        for i in num_row..num_basic {
            self.row_with_no_pivot[lc] = i;
            self.iwork[i] = -(lc as isize + 1);
            lc += 1;
        }
        debug_assert_eq!(lc, rank_deficiency);
        let row_rank_deficiency = rank_deficiency - num_basic.saturating_sub(num_row);
        for k in 0..rank_deficiency {
            let i_row = self.row_with_no_pivot[k];
            let i_col = self.col_with_no_pivot[k];
            debug_assert_eq!(self.permute[i_col], -1);
            self.permute[i_col] = i_row as isize;
            if k < row_rank_deficiency {
                self.l_start.push(self.l_index.len());
                self.u_pivot_index.push(i_row);
                self.u_pivot_value.push(1.0);
                self.u_start.push(self.u_index.len());
            }
        }
    }

    fn build_mark_sing_c(&mut self, basic_index: &mut [usize]) {
        self.var_with_no_pivot = vec![NONE; self.rank_deficiency];
        for k in 0..self.rank_deficiency {
            let asm_row = self.row_with_no_pivot[k];
            let asm_col = self.col_with_no_pivot[k];
            self.iwork[asm_row] = -(asm_col as isize + 1);
            if asm_col < self.num_basic {
                self.var_with_no_pivot[k] = basic_index[asm_col];
                basic_index[asm_col] = self.num_col + asm_row;
            }
        }
    }

    fn build_finish(&mut self, basic_index: &mut [usize]) {
        let num_row = self.num_row;
        for i in 0..num_row {
            self.u_pivot_lookup[self.u_pivot_index[i]] = i;
        }
        self.l_pivot_index.clone_from(&self.u_pivot_index);
        self.l_pivot_lookup.clone_from(&self.u_pivot_lookup);

        // L row-wise.
        let l_count_x = self.l_index.len();
        self.lr_index.clear();
        self.lr_index.resize(l_count_x, 0);
        self.lr_value.clear();
        self.lr_value.resize(l_count_x, 0.0);
        let mut counts = vec![0usize; num_row];
        for k in 0..l_count_x {
            counts[self.l_pivot_lookup[self.l_index[k]]] += 1;
        }
        self.lr_start.clear();
        self.lr_start.resize(num_row + 1, 0);
        for i in 1..=num_row {
            self.lr_start[i] = self.lr_start[i - 1] + counts[i - 1];
        }
        let mut put: Vec<usize> = self.lr_start[..num_row].to_vec();
        for i in 0..num_row {
            let index = self.l_pivot_index[i];
            for k in self.l_start[i]..self.l_start[i + 1] {
                let i_row = self.l_pivot_lookup[self.l_index[k]];
                let i_put = put[i_row];
                put[i_row] += 1;
                self.lr_index[i_put] = index;
                self.lr_value[i_put] = self.l_value[k];
            }
        }

        // U's ends.
        self.u_last_p.clear();
        self.u_last_p.extend_from_slice(&self.u_start[1..=num_row]);
        self.u_start.truncate(num_row);

        // U row-wise, with room for five update entries a row.
        let u_count_x = self.u_index.len();
        let ur_stuff_size = 5;
        let ur_count_size = u_count_x + ur_stuff_size * num_row;
        self.ur_index.clear();
        self.ur_index.resize(ur_count_size, 0);
        self.ur_value.clear();
        self.ur_value.resize(ur_count_size, 0.0);
        self.ur_start.clear();
        self.ur_start.resize(num_row + 1, 0);
        self.ur_lastp.clear();
        self.ur_lastp.resize(num_row, 0);
        self.ur_space.clear();
        self.ur_space.resize(num_row, ur_stuff_size);
        for k in 0..u_count_x {
            self.ur_lastp[self.u_pivot_lookup[self.u_index[k]]] += 1;
        }
        for i in 1..=num_row {
            self.ur_start[i] = self.ur_start[i - 1] + self.ur_lastp[i - 1] + ur_stuff_size;
        }
        self.ur_start.truncate(num_row);
        self.ur_lastp.clone_from(&self.ur_start);
        for i in 0..num_row {
            let index = self.u_pivot_index[i];
            for k in self.u_start[i]..self.u_last_p[i] {
                let i_row = self.u_pivot_lookup[self.u_index[k]];
                let i_put = self.ur_lastp[i_row];
                self.ur_lastp[i_row] += 1;
                self.ur_index[i_put] = index;
                self.ur_value[i_put] = self.u_value[k];
            }
        }

        self.u_merit_x = num_row + ((l_count_x + u_count_x) as f64 * 1.5) as usize;
        self.u_total_x = u_count_x;

        self.pf_pivot_index.clear();
        self.pf_start.clear();
        self.pf_start.push(0);
        self.pf_index.clear();
        self.pf_value.clear();

        // Permute the basic variables to the pivot order.
        let num_basic = self.num_basic;
        let original: Vec<usize> = basic_index[..num_basic].to_vec();
        for i in 0..num_basic {
            basic_index[self.permute[i] as usize] = original[i];
        }
        self.build_synthetic_tick += (num_row * 80 + (l_count_x + u_count_x) * 60) as f64;
    }

    fn zero_col(&mut self, j_col: usize) {
        let a_start = self.mc_start[j_col] as usize;
        let a_end = a_start + (self.mc_count_a[j_col] as usize);
        for i_el in a_start..a_end {
            let i_row = self.mc_index[i_el] as usize;
            self.row_delete(j_col, i_row);
            self.rlink_del(i_row);
            self.rlink_add(i_row, self.mr_count[i_row] as usize);
        }
        self.clink_del(j_col);
        self.mc_count_a[j_col] = 0_u32;
        self.mc_count_n[j_col] = 0_u32;
    }

    /// Solve `B x = b` in place.
    pub fn ftran(&self, rhs: &mut HVector, expected_density: f64) {
        let use_indices = rhs.count >= 0;
        self.ftran_l(rhs, expected_density);
        self.ftran_u(rhs, expected_density);
        if use_indices {
            rhs.re_index();
        }
    }

    /// Solve `Bᵀ x = b` in place.
    pub fn btran(&self, rhs: &mut HVector, expected_density: f64) {
        let use_indices = rhs.count >= 0;
        self.btran_u(rhs, expected_density);
        self.btran_l(rhs, expected_density);
        if use_indices {
            rhs.re_index();
        }
    }

    fn ftran_l(&self, rhs: &mut HVector, expected_density: f64) {
        let current_density = rhs.count as f64 * self.inv_num_row;
        let sparse_solve = rhs.count < 0 || current_density > HYPER_CANCEL || expected_density > HYPER_FTRAN_L;
        if sparse_solve {
            let mut rhs_count = 0;
            for i in 0..self.num_row {
                let pivot_row = self.l_pivot_index[i];
                let pivot_multiplier = rhs.array[pivot_row];
                if pivot_multiplier.abs() > TINY {
                    rhs.index[rhs_count] = pivot_row;
                    rhs_count += 1;
                    let range = self.l_start[i]..self.l_start[i + 1];
                    for (&row, &value) in self.l_index[range.clone()].iter().zip(&self.l_value[range]) {
                        rhs.array[row] -= pivot_multiplier * value;
                    }
                } else {
                    rhs.array[pivot_row] = 0.0;
                }
            }
            rhs.count = rhs_count as isize;
        } else {
            solve_hyper(
                self.num_row,
                &self.l_pivot_lookup,
                &self.l_pivot_index,
                None,
                &self.l_start,
                &self.l_start[1..],
                &self.l_index,
                &self.l_value,
                rhs,
            );
        }
    }

    fn btran_l(&self, rhs: &mut HVector, expected_density: f64) {
        let current_density = rhs.count as f64 * self.inv_num_row;
        let sparse_solve = rhs.count < 0 || current_density > HYPER_CANCEL || expected_density > HYPER_BTRAN_L;
        if sparse_solve {
            let mut rhs_count = 0;
            for i in (0..self.num_row).rev() {
                let pivot_row = self.l_pivot_index[i];
                let pivot_multiplier = rhs.array[pivot_row];
                if pivot_multiplier.abs() > TINY {
                    rhs.index[rhs_count] = pivot_row;
                    rhs_count += 1;
                    let range = self.lr_start[i]..self.lr_start[i + 1];
                    for (&row, &value) in self.lr_index[range.clone()].iter().zip(&self.lr_value[range]) {
                        rhs.array[row] -= pivot_multiplier * value;
                    }
                } else {
                    rhs.array[pivot_row] = 0.0;
                }
            }
            rhs.count = rhs_count as isize;
        } else {
            solve_hyper(
                self.num_row,
                &self.l_pivot_lookup,
                &self.l_pivot_index,
                None,
                &self.lr_start,
                &self.lr_start[1..],
                &self.lr_index,
                &self.lr_value,
                rhs,
            );
        }
    }

    fn ftran_u(&self, rhs: &mut HVector, expected_density: f64) {
        // The update part.
        self.ftran_ft(rhs);
        rhs.tight();
        rhs.pack();

        let current_density = rhs.count as f64 * self.inv_num_row;
        let sparse_solve = rhs.count < 0 || current_density > HYPER_CANCEL || expected_density > HYPER_FTRAN_U;
        if sparse_solve {
            let mut rhs_synthetic_tick = 0.0;
            let mut rhs_count = 0;
            let u_pivot_count = self.u_pivot_index.len();
            for i_logic in (0..u_pivot_count).rev() {
                let pivot_row = self.u_pivot_index[i_logic];
                if pivot_row == NONE {
                    continue;
                }
                let mut pivot_multiplier = rhs.array[pivot_row];
                if pivot_multiplier.abs() > TINY {
                    pivot_multiplier /= self.u_pivot_value[i_logic];
                    rhs.index[rhs_count] = pivot_row;
                    rhs_count += 1;
                    rhs.array[pivot_row] = pivot_multiplier;
                    let start = self.u_start[i_logic];
                    let end = self.u_last_p[i_logic];
                    if i_logic >= self.num_row {
                        rhs_synthetic_tick += (end - start) as f64;
                    }
                    for (&row, &value) in self.u_index[start..end].iter().zip(&self.u_value[start..end]) {
                        rhs.array[row] -= pivot_multiplier * value;
                    }
                } else {
                    rhs.array[pivot_row] = 0.0;
                }
            }
            rhs.count = rhs_count as isize;
            rhs.synthetic_tick += rhs_synthetic_tick * 15.0 + (u_pivot_count - self.num_row) as f64 * 10.0;
        } else {
            solve_hyper(
                self.num_row,
                &self.u_pivot_lookup,
                &self.u_pivot_index,
                Some(&self.u_pivot_value),
                &self.u_start,
                &self.u_last_p,
                &self.u_index,
                &self.u_value,
                rhs,
            );
        }
    }

    fn btran_u(&self, rhs: &mut HVector, expected_density: f64) {
        let current_density = rhs.count as f64 * self.inv_num_row;
        let sparse_solve = rhs.count < 0 || current_density > HYPER_CANCEL || expected_density > HYPER_BTRAN_U;
        if sparse_solve {
            let mut rhs_synthetic_tick = 0.0;
            let mut rhs_count = 0;
            let u_pivot_count = self.u_pivot_index.len();
            for i_logic in 0..u_pivot_count {
                let pivot_row = self.u_pivot_index[i_logic];
                if pivot_row == NONE {
                    continue;
                }
                let mut pivot_multiplier = rhs.array[pivot_row];
                if pivot_multiplier.abs() > TINY {
                    pivot_multiplier /= self.u_pivot_value[i_logic];
                    rhs.index[rhs_count] = pivot_row;
                    rhs_count += 1;
                    rhs.array[pivot_row] = pivot_multiplier;
                    let start = self.ur_start[i_logic];
                    let end = self.ur_lastp[i_logic];
                    if i_logic >= self.num_row {
                        rhs_synthetic_tick += (end - start) as f64;
                    }
                    for (&row, &value) in self.ur_index[start..end].iter().zip(&self.ur_value[start..end]) {
                        rhs.array[row] -= pivot_multiplier * value;
                    }
                } else {
                    rhs.array[pivot_row] = 0.0;
                }
            }
            rhs.count = rhs_count as isize;
            rhs.synthetic_tick += rhs_synthetic_tick * 15.0 + (u_pivot_count - self.num_row) as f64 * 10.0;
        } else {
            solve_hyper(
                self.num_row,
                &self.u_pivot_lookup,
                &self.u_pivot_index,
                Some(&self.u_pivot_value),
                &self.ur_start,
                &self.ur_lastp,
                &self.ur_index,
                &self.ur_value,
                rhs,
            );
        }
        // The update part.
        rhs.tight();
        rhs.pack();
        self.btran_ft(rhs);
        rhs.tight();
    }

    fn ftran_ft(&self, rhs: &mut HVector) {
        let mut rhs_count = rhs.count as usize;
        let pf_pivot_count = self.pf_pivot_index.len();
        for i in 0..pf_pivot_count {
            let i_row = self.pf_pivot_index[i];
            let value0 = rhs.array[i_row];
            let mut value1 = value0;
            let range = self.pf_start[i]..self.pf_start[i + 1];
            for (&row, &value) in self.pf_index[range.clone()].iter().zip(&self.pf_value[range]) {
                value1 -= rhs.array[row] * value;
            }
            if value0 != 0.0 || value1 != 0.0 {
                if value0 == 0.0 {
                    rhs.index[rhs_count] = i_row;
                    rhs_count += 1;
                }
                rhs.array[i_row] = if value1.abs() < TINY { ZERO } else { value1 };
            }
        }
        rhs.count = rhs_count as isize;
        let pf_entries = self.pf_start[pf_pivot_count];
        rhs.synthetic_tick += (pf_pivot_count * 20 + pf_entries * 5) as f64;
        if pf_entries / (pf_pivot_count + 1) < 5 {
            rhs.synthetic_tick += (pf_entries * 5) as f64;
        }
    }

    fn btran_ft(&self, rhs: &mut HVector) {
        let mut rhs_count = rhs.count as usize;
        let pf_pivot_count = self.pf_pivot_index.len();
        let mut rhs_synthetic_tick = 0.0;
        for i in (0..pf_pivot_count).rev() {
            let pivot_row = self.pf_pivot_index[i];
            let pivot_multiplier = rhs.array[pivot_row];
            if pivot_multiplier != 0.0 {
                let start = self.pf_start[i];
                let end = self.pf_start[i + 1];
                rhs_synthetic_tick += (end - start) as f64;
                for (&i_row, &value) in self.pf_index[start..end].iter().zip(&self.pf_value[start..end]) {
                    let value0 = rhs.array[i_row];
                    let value1 = value0 - pivot_multiplier * value;
                    if value0 == 0.0 {
                        rhs.index[rhs_count] = i_row;
                        rhs_count += 1;
                    }
                    rhs.array[i_row] = if value1.abs() < TINY { ZERO } else { value1 };
                }
            }
        }
        rhs.synthetic_tick += rhs_synthetic_tick * 15.0 + pf_pivot_count as f64 * 10.0;
        rhs.count = rhs_count as isize;
    }

    /// Replace the basic variable pivoting on `i_row` by the column whose
    /// FTRAN is `aq`, given the BTRAN `ep` of `e_{i_row}`. Both must have been
    /// solved with their pack flag set.
    pub fn update(&mut self, aq: &HVector, ep: &HVector, i_row: usize) {
        let p_logic = self.u_pivot_lookup[i_row];
        let pivot = self.u_pivot_value[p_logic];
        let alpha = aq.array[i_row];
        self.u_pivot_index[p_logic] = NONE;

        // Delete the pivotal row from U.
        for k in self.ur_start[p_logic]..self.ur_lastp[p_logic] {
            let i_logic = self.u_pivot_lookup[self.ur_index[k]];
            let mut i_find = self.u_start[i_logic];
            self.u_last_p[i_logic] -= 1;
            let i_last = self.u_last_p[i_logic];
            while i_find <= i_last {
                if self.u_index[i_find] == i_row {
                    break;
                }
                i_find += 1;
            }
            self.u_index[i_find] = self.u_index[i_last];
            self.u_value[i_find] = self.u_value[i_last];
        }

        // Delete the pivotal column from UR.
        for k in self.u_start[p_logic]..self.u_last_p[p_logic] {
            let i_logic = self.u_pivot_lookup[self.u_index[k]];
            let mut i_find = self.ur_start[i_logic];
            self.ur_lastp[i_logic] -= 1;
            let i_last = self.ur_lastp[i_logic];
            while i_find <= i_last {
                if self.ur_index[i_find] == i_row {
                    break;
                }
                i_find += 1;
            }
            self.ur_space[i_logic] += 1;
            self.ur_index[i_find] = self.ur_index[i_last];
            self.ur_value[i_find] = self.ur_value[i_last];
        }

        // Store the column to U.
        self.u_start.push(self.u_index.len());
        for i in 0..aq.pack_count {
            if aq.pack_index[i] != i_row {
                self.u_index.push(aq.pack_index[i]);
                self.u_value.push(aq.pack_value[i]);
            }
        }
        self.u_last_p.push(self.u_index.len());
        let u_start_x = *self.u_start.last().unwrap();
        let u_end_x = *self.u_last_p.last().unwrap();
        self.u_total_x += u_end_x - u_start_x + 1;

        // And as UR elements.
        for k in u_start_x..u_end_x {
            let i_logic = self.u_pivot_lookup[self.u_index[k]];
            if self.ur_space[i_logic] == 0 {
                let row_start = self.ur_start[i_logic];
                let row_count = self.ur_lastp[i_logic] - row_start;
                let new_start = self.ur_index.len();
                let new_space = (row_count as f64 * 1.1) as usize + 5;
                self.ur_index.resize(new_start + new_space, 0);
                self.ur_value.resize(new_start + new_space, 0.0);
                self.ur_index.copy_within(row_start..row_start + row_count, new_start);
                self.ur_value.copy_within(row_start..row_start + row_count, new_start);
                self.ur_start[i_logic] = new_start;
                self.ur_lastp[i_logic] = new_start + row_count;
                self.ur_space[i_logic] = new_space - row_count;
            }
            self.ur_space[i_logic] -= 1;
            let i_put = self.ur_lastp[i_logic];
            self.ur_lastp[i_logic] += 1;
            self.ur_index[i_put] = i_row;
            self.ur_value[i_put] = self.u_value[k];
        }

        // UR pointers for the new pivot.
        self.ur_start.push(self.ur_start[p_logic]);
        self.ur_lastp.push(self.ur_start[p_logic]);
        self.ur_space.push(self.ur_space[p_logic] + self.ur_lastp[p_logic] - self.ur_start[p_logic]);

        self.u_pivot_lookup[i_row] = self.u_pivot_index.len();
        self.u_pivot_index.push(i_row);
        self.u_pivot_value.push(pivot * alpha);

        // The row eta.
        let pf_start_back = *self.pf_start.last().unwrap();
        for i in 0..ep.pack_count {
            if ep.pack_index[i] != i_row {
                self.pf_index.push(ep.pack_index[i]);
                self.pf_value.push(-ep.pack_value[i] * pivot);
            }
        }
        self.u_total_x += self.pf_index.len() - pf_start_back;
        self.pf_pivot_index.push(i_row);
        self.pf_start.push(self.pf_index.len());

        self.u_total_x -= self.u_last_p[p_logic] - self.u_start[p_logic];
        self.u_total_x -= self.ur_lastp[p_logic] - self.ur_start[p_logic];
    }

    fn col_insert(&mut self, i_col: usize, i_row: usize, value: f64) {
        let iput = (self.mc_start[i_col] as usize) + (self.mc_count_a[i_col] as usize);
        self.mc_count_a[i_col] += 1_u32;
        self.mc_index[iput] = i_row as u32;
        self.mc_value[iput] = value;
    }

    fn col_store_n(&mut self, i_col: usize, i_row: usize, value: f64) {
        self.mc_count_n[i_col] += 1_u32;
        let iput = (self.mc_start[i_col] as usize) + (self.mc_space[i_col] as usize) - (self.mc_count_n[i_col] as usize);
        self.mc_index[iput] = i_row as u32;
        self.mc_value[iput] = value;
    }

    fn col_fix_max(&mut self, i_col: usize) {
        let start = self.mc_start[i_col] as usize;
        let max_value = self.mc_value[start..start + (self.mc_count_a[i_col] as usize)]
            .iter()
            .fold(0.0f64, |max, value| max.max(value.abs()));
        // Never below the pivot tolerance, or a pivot under it can be chosen
        // over larger entries that meet it, and the column is then taken
        // as singular (HiGHS #3354).
        self.mc_min_pivot[i_col] = (max_value * self.pivot_threshold).max(self.pivot_tolerance);
    }

    fn col_delete(&mut self, i_col: usize, i_row: usize) -> f64 {
        let start = self.mc_start[i_col] as usize;
        self.mc_count_a[i_col] -= 1_u32;
        let imov = start + (self.mc_count_a[i_col] as usize);
        let idel = find_entry(&self.mc_index, start, i_row);
        let pivot_multiplier = self.mc_value[idel];
        self.mc_index[idel] = self.mc_index[imov];
        self.mc_value[idel] = self.mc_value[imov];
        pivot_multiplier
    }

    fn row_insert(&mut self, i_col: usize, i_row: usize) {
        let iput = (self.mr_start[i_row] as usize) + (self.mr_count[i_row] as usize);
        self.mr_count[i_row] += 1_u32;
        self.mr_index[iput] = i_col as u32;
    }

    fn row_delete(&mut self, i_col: usize, i_row: usize) {
        let start = self.mr_start[i_row] as usize;
        self.mr_count[i_row] -= 1_u32;
        let imov = start + (self.mr_count[i_row] as usize);
        let idel = find_entry(&self.mr_index, start, i_col);
        self.mr_index[idel] = self.mr_index[imov];
    }

    fn clink_add(&mut self, index: usize, count: usize) {
        let mover = self.col_link_first[count];
        self.col_link_last[index] = -2 - count as isize;
        self.col_link_next[index] = mover;
        self.col_link_first[count] = index as isize;
        if mover >= 0 {
            self.col_link_last[mover as usize] = index as isize;
        }
    }

    fn clink_del(&mut self, index: usize) {
        let xlast = self.col_link_last[index];
        let xnext = self.col_link_next[index];
        if xlast >= 0 {
            self.col_link_next[xlast as usize] = xnext;
        } else {
            self.col_link_first[(-xlast - 2) as usize] = xnext;
        }
        if xnext >= 0 {
            self.col_link_last[xnext as usize] = xlast;
        }
    }

    fn rlink_add(&mut self, index: usize, count: usize) {
        let mover = self.row_link_first[count];
        self.row_link_last[index] = -2 - count as isize;
        self.row_link_next[index] = mover;
        self.row_link_first[count] = index as isize;
        if mover >= 0 {
            self.row_link_last[mover as usize] = index as isize;
        }
    }

    fn rlink_del(&mut self, index: usize) {
        let xlast = self.row_link_last[index];
        let xnext = self.row_link_next[index];
        if xlast >= 0 {
            self.row_link_next[xlast as usize] = xnext;
        } else {
            self.row_link_first[(-xlast - 2) as usize] = xnext;
        }
        if xnext >= 0 {
            self.row_link_last[xnext as usize] = xlast;
        }
    }
}

/// HiGHS's `while (index[k] != value) k++` from `start`: the position of
/// `value`, which the caller knows is there.
#[inline]
fn find_entry(index: &[u32], start: usize, value: usize) -> usize {
    start + index[start..].iter().position(|&entry| entry as usize == value).expect("the entry is in its column or row")
}

/// Solve with a triangular factor touching only the entries reachable from
/// the right-hand side's nonzeros: a depth-first search orders them, then
/// they are eliminated in that order.
#[allow(clippy::too_many_arguments)]
fn solve_hyper(
    h_size: usize,
    h_lookup: &[usize],
    h_pivot_index: &[usize],
    h_pivot_value: Option<&[f64]>,
    h_start: &[usize],
    h_end: &[usize],
    h_index: &[usize],
    h_value: &[f64],
    rhs: &mut HVector,
) {
    // Updated factors can have more pivots than rows. Marks are indexed by
    // pivot; the traversal and its stack contain at most one per row.
    if rhs.cwork.len() < h_pivot_index.len() {
        rhs.cwork.resize(h_pivot_index.len(), 0);
    }
    if rhs.iwork.len() < h_size * 4 {
        rhs.iwork.resize(h_size * 4, 0);
    }
    let rhs_count = rhs.count as usize;
    let HVector {
        index: rhs_index,
        array: rhs_array,
        cwork: list_mark,
        iwork,
        synthetic_tick,
        ..
    } = rhs;
    // Each row has one current pivot, so at most `h_size` are listed, and
    // the stack holds at most three entries for each.
    let (list_index, list_stack) = iwork.split_at_mut(h_size);
    let mut list_count = 0;
    let mut count_pivot = 0usize;
    let mut count_entry = 0usize;

    // HiGHS's depth-first search, with each node's end kept beside its
    // position on the stack instead of read again at every step.
    for i in 0..rhs_count {
        let i_trans = h_lookup[rhs_index[i]];
        if list_mark[i_trans] != 0 {
            continue;
        }
        let mut hi = i_trans;
        let mut hk = h_start[hi];
        let mut he = h_end[hi];
        let mut n_stack = 0usize;
        list_mark[hi] = 1;
        loop {
            if hk < he {
                let hi_sub = h_lookup[h_index[hk]];
                hk += 1;
                if list_mark[hi_sub] == 0 {
                    list_mark[hi_sub] = 1;
                    list_stack[n_stack] = hi;
                    list_stack[n_stack + 1] = hk;
                    list_stack[n_stack + 2] = he;
                    n_stack += 3;
                    hi = hi_sub;
                    hk = h_start[hi];
                    he = h_end[hi];
                    if hi >= h_size {
                        count_pivot += 1;
                        count_entry += he - hk;
                    }
                }
            } else {
                list_index[list_count] = hi;
                list_count += 1;
                if n_stack == 0 {
                    break;
                }
                he = list_stack[n_stack - 1];
                hk = list_stack[n_stack - 2];
                hi = list_stack[n_stack - 3];
                n_stack -= 3;
            }
        }
    }
    *synthetic_tick += (count_pivot * 20 + count_entry * 10) as f64;

    let mut count = 0;
    for i_list in (0..list_count).rev() {
        let i = list_index[i_list];
        list_mark[i] = 0;
        let pivot_row = h_pivot_index[i];
        let mut pivot_multiplier = rhs_array[pivot_row];
        if pivot_multiplier.abs() > TINY {
            if let Some(pivot_value) = h_pivot_value {
                pivot_multiplier /= pivot_value[i];
                rhs_array[pivot_row] = pivot_multiplier;
            }
            rhs_index[count] = pivot_row;
            count += 1;
            let (start, end) = (h_start[i], h_end[i]);
            for (&row, &value) in h_index[start..end].iter().zip(&h_value[start..end]) {
                rhs_array[row] -= pivot_multiplier * value;
            }
        } else {
            rhs_array[pivot_row] = 0.0;
        }
    }
    rhs.count = count as isize;
}
