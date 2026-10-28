//! IPX's `Model` (`model.cc`): the user's LP `min c'x, Ax (=,<,>) b,
//! lb <= x <= ub` turned into the solver's `min c'x, AI x = b, lb <= x <=
//! ub` with a slack column per row, after scaling and, when it has many
//! more rows than columns, dualization; and the way back for solutions
//! and bases.

use super::{
    IPX_BASIC, IPX_NONBASIC, IPX_NONBASIC_LB, IPX_NONBASIC_UB, IPX_SUPERBASIC, Info,
    sparse::{Int, SparseMatrix, dot, dot_column, infnorm, scale_column, scatter_column, transpose, u},
};

const INF: f64 = f64::INFINITY;

/// An interior point in the user's terms: `(x, xl, xu, slack, y, zl, zu)`.
pub(crate) type InteriorSolution = (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>);

#[derive(Clone, Debug, Default)]
pub(crate) struct Model {
    dualized: bool,
    num_rows: Int,
    num_cols: Int,
    num_dense_cols: Int,
    nz_dense: Int,
    ai: SparseMatrix,
    ait: SparseMatrix,
    b: Vec<f64>,
    c: Vec<f64>,
    lb: Vec<f64>,
    ub: Vec<f64>,
    norm_bounds: f64,
    norm_c: f64,
    num_constr: Int,
    num_eqconstr: Int,
    num_var: Int,
    num_free_var: Int,
    num_entries: Int,
    boxed_vars: Vec<Int>,
    constr_type: Vec<u8>,
    norm_obj: f64,
    norm_rhs: f64,
    offset: f64,
    scaled_obj: Vec<f64>,
    scaled_rhs: Vec<f64>,
    scaled_lbuser: Vec<f64>,
    scaled_ubuser: Vec<f64>,
    a: SparseMatrix,
    flipped_vars: Vec<Int>,
    colscale: Vec<f64>,
    rowscale: Vec<f64>,
}

impl Model {
    /// `Model::Load` with `dualize` -1 (dualize when there are more than
    /// twice as many constraints as variables) or 0, and `scale` >= 1 to
    /// equilibrate. Returns IPX's error flag.
    #[allow(clippy::too_many_arguments)]
    pub fn load(
        &mut self,
        dualize: Int,
        scale: Int,
        num_constr: Int,
        num_var: Int,
        ap: &[Int],
        ai: &[Int],
        ax: &[f64],
        rhs: &[f64],
        constr_type: &[u8],
        offset: f64,
        obj: &[f64],
        lbuser: &[f64],
        ubuser: &[f64],
    ) -> Int {
        *self = Self::default();
        let errflag = self.copy_input(num_constr, num_var, ap, ai, ax, rhs, constr_type, offset, obj, lbuser, ubuser);
        if errflag != 0 {
            return errflag;
        }
        self.scale_model(scale);
        let dualize = if dualize == -1 { num_constr > 2 * num_var } else { dualize != 0 };
        if dualize {
            self.load_dual();
        } else {
            self.load_primal();
        }
        self.a.clear();
        self.ait = transpose(&self.ai);
        self.find_dense_columns();
        self.norm_c = infnorm(&self.c);
        self.norm_bounds = infnorm(&self.b);
        for &x in self.lb.iter().chain(&self.ub) {
            if x.is_finite() && self.norm_bounds < x.abs() {
                self.norm_bounds = x.abs();
            }
        }
        0
    }

    pub fn get_info(&self, info: &mut Info) {
        info.num_var = self.num_var;
        info.num_constr = self.num_constr;
        info.num_entries = self.num_entries;
        info.num_rows_solver = self.num_rows;
        info.num_cols_solver = self.num_cols + self.num_rows;
        info.num_entries_solver = self.ai.entries();
        info.dualized = self.dualized;
        info.dense_cols = self.num_dense_cols;
    }

    pub fn empty(&self) -> bool {
        self.cols() == 0
    }
    pub fn rows(&self) -> Int {
        self.num_rows
    }
    pub fn cols(&self) -> Int {
        self.num_cols
    }
    pub fn num_dense_cols(&self) -> Int {
        self.num_dense_cols
    }
    pub fn is_dense_column(&self, j: Int) -> bool {
        self.ai.entries_of(j) >= self.nz_dense
    }
    pub fn dualized(&self) -> bool {
        self.dualized
    }
    pub fn ai(&self) -> &SparseMatrix {
        &self.ai
    }
    pub fn ait(&self) -> &SparseMatrix {
        &self.ait
    }
    pub fn offset(&self) -> f64 {
        self.offset
    }
    pub fn b(&self) -> &[f64] {
        &self.b
    }
    pub fn c(&self) -> &[f64] {
        &self.c
    }
    pub fn lb(&self) -> &[f64] {
        &self.lb
    }
    pub fn ub(&self) -> &[f64] {
        &self.ub
    }
    pub fn norm_bounds(&self) -> f64 {
        self.norm_bounds
    }
    pub fn norm_c(&self) -> f64 {
        self.norm_c
    }
    pub fn num_var(&self) -> Int {
        self.num_var
    }
    pub fn num_constr(&self) -> Int {
        self.num_constr
    }

    #[allow(clippy::too_many_arguments)]
    fn copy_input(
        &mut self,
        num_constr: Int,
        num_var: Int,
        ap: &[Int],
        ai: &[Int],
        ax: &[f64],
        rhs: &[f64],
        constr_type: &[u8],
        offset: f64,
        obj: &[f64],
        lbuser: &[f64],
        ubuser: &[f64],
    ) -> Int {
        if num_constr < 0 || num_var <= 0 {
            return super::IPX_ERROR_INVALID_DIMENSION;
        }
        if check_vectors(num_constr, num_var, rhs, constr_type, obj, lbuser, ubuser) != 0 {
            return super::IPX_ERROR_INVALID_VECTOR;
        }
        if check_matrix(num_constr, num_var, ap, ai, ax) != 0 {
            return super::IPX_ERROR_INVALID_MATRIX;
        }
        let n = u(num_var);
        self.num_constr = num_constr;
        self.num_eqconstr = constr_type[..u(num_constr)].iter().filter(|&&kind| kind == b'=').count() as Int;
        self.num_var = num_var;
        self.num_entries = ap[n];
        self.num_free_var = 0;
        self.boxed_vars.clear();
        for j in 0..n {
            if lbuser[j].is_infinite() && ubuser[j].is_infinite() {
                self.num_free_var += 1;
            }
            if lbuser[j].is_finite() && ubuser[j].is_finite() {
                self.boxed_vars.push(j as Int);
            }
        }
        self.constr_type = constr_type[..u(num_constr)].to_vec();
        self.offset = offset;
        self.scaled_obj = obj[..n].to_vec();
        self.scaled_rhs = rhs[..u(num_constr)].to_vec();
        self.scaled_lbuser = lbuser[..n].to_vec();
        self.scaled_ubuser = ubuser[..n].to_vec();
        self.a.load_from_arrays(num_constr, num_var, &ap[..n], &ap[1..=n], ai, ax);
        self.norm_obj = infnorm(&self.scaled_obj);
        self.norm_rhs = infnorm(&self.scaled_rhs);
        for &x in self.scaled_lbuser.iter().chain(&self.scaled_ubuser) {
            if x.is_finite() && self.norm_rhs < x.abs() {
                self.norm_rhs = x.abs();
            }
        }
        0
    }

    fn scale_model(&mut self, scale: Int) {
        self.flipped_vars.clear();
        for j in 0..u(self.num_var) {
            if self.scaled_ubuser[j].is_finite() && self.scaled_lbuser[j].is_infinite() {
                self.scaled_lbuser[j] = -self.scaled_ubuser[j];
                self.scaled_ubuser[j] = INF;
                scale_column(&mut self.a, j as Int, -1.0);
                self.scaled_obj[j] *= -1.0;
                self.flipped_vars.push(j as Int);
            }
        }
        self.colscale.clear();
        self.rowscale.clear();
        if scale >= 1 {
            self.equilibrate_matrix();
        }
        if !self.colscale.is_empty() {
            for j in 0..u(self.num_var) {
                self.scaled_obj[j] *= self.colscale[j];
                self.scaled_lbuser[j] /= self.colscale[j];
                self.scaled_ubuser[j] /= self.colscale[j];
            }
        }
        if !self.rowscale.is_empty() {
            for i in 0..u(self.num_constr) {
                self.scaled_rhs[i] *= self.rowscale[i];
            }
        }
    }

    fn load_primal(&mut self) {
        let (m, n) = (u(self.num_constr), u(self.num_var));
        self.num_rows = self.num_constr;
        self.num_cols = self.num_var;
        self.dualized = false;
        self.ai = self.a.clone();
        for i in 0..m {
            self.ai.push_back(i as Int, 1.0);
            self.ai.add_column();
        }
        self.b = self.scaled_rhs.clone();
        self.c = vec![0.0; n + m];
        self.c[..n].copy_from_slice(&self.scaled_obj);
        self.lb = vec![0.0; n + m];
        self.lb[..n].copy_from_slice(&self.scaled_lbuser);
        self.ub = vec![0.0; n + m];
        self.ub[..n].copy_from_slice(&self.scaled_ubuser);
        for i in 0..m {
            let (lower, upper) = match self.constr_type[i] {
                b'=' => (0.0, 0.0),
                b'<' => (0.0, INF),
                _ => (-INF, 0.0),
            };
            self.lb[n + i] = lower;
            self.ub[n + i] = upper;
        }
    }

    fn load_dual(&mut self) {
        let num_var = u(self.num_var);
        let num_constr = u(self.num_constr);
        self.num_rows = self.num_var;
        self.num_cols = self.num_constr + self.boxed_vars.len() as Int;
        self.dualized = true;
        let num_cols = u(self.num_cols);
        self.ai = transpose(&self.a);
        for j in 0..num_var {
            if self.scaled_ubuser[j].is_finite() {
                self.ai.push_back(j as Int, -1.0);
                self.ai.add_column();
            }
        }
        for i in 0..num_var {
            self.ai.push_back(i as Int, 1.0);
            self.ai.add_column();
        }
        self.b = self.scaled_obj.clone();
        self.c = vec![0.0; num_cols + num_var];
        let mut put = 0;
        for &x in &self.scaled_rhs {
            self.c[put] = -x;
            put += 1;
        }
        for &x in &self.scaled_ubuser {
            if x.is_finite() {
                self.c[put] = x;
                put += 1;
            }
        }
        for &x in &self.scaled_lbuser {
            self.c[put] = if x.is_finite() { -x } else { 0.0 };
            put += 1;
        }
        self.lb = vec![0.0; num_cols + num_var];
        self.ub = vec![0.0; num_cols + num_var];
        for i in 0..num_constr {
            let (lower, upper) = match self.constr_type[i] {
                b'=' => (-INF, INF),
                b'<' => (-INF, 0.0),
                _ => (0.0, INF),
            };
            self.lb[i] = lower;
            self.ub[i] = upper;
        }
        for j in num_constr..num_cols {
            self.lb[j] = 0.0;
            self.ub[j] = INF;
        }
        for j in 0..num_var {
            self.lb[num_cols + j] = 0.0;
            self.ub[num_cols + j] = if self.scaled_lbuser[j].is_finite() { INF } else { 0.0 };
        }
    }

    fn equilibrate_matrix(&mut self) {
        let m = u(self.a.rows());
        let n = u(self.a.cols());
        self.colscale.clear();
        self.rowscale.clear();
        const EXPMIN: i32 = 0;
        const EXPMAX: i32 = 3;
        const MAXROUND: usize = 10;
        let nnz = u(self.a.colptr[n]);
        if !self.a.values[..nnz].iter().any(|&x| {
            let exp = frexp_exp(x.abs());
            !(EXPMIN..=EXPMAX).contains(&exp)
        }) {
            return;
        }
        self.colscale = vec![1.0; n];
        self.rowscale = vec![1.0; m];
        let mut colmax = vec![0.0; n];
        let mut rowmax = vec![0.0; m];
        for _ in 0..MAXROUND {
            rowmax.fill(0.0);
            for j in 0..n {
                colmax[j] = 0.0;
                for p in u(self.a.colptr[j])..u(self.a.colptr[j + 1]) {
                    let i = u(self.a.rowidx[p]);
                    let xa = self.a.values[p].abs();
                    colmax[j] = std_max(colmax[j], xa);
                    rowmax[i] = std_max(rowmax[i], xa);
                }
            }
            let mut out_of_range = false;
            for i in 0..m {
                rowmax[i] = equilibration_factor(EXPMIN, EXPMAX, frexp_exp(rowmax[i]));
                if rowmax[i] != 1.0 {
                    out_of_range = true;
                    self.rowscale[i] *= rowmax[i];
                }
            }
            for j in 0..n {
                colmax[j] = equilibration_factor(EXPMIN, EXPMAX, frexp_exp(colmax[j]));
                if colmax[j] != 1.0 {
                    out_of_range = true;
                    self.colscale[j] *= colmax[j];
                }
            }
            if !out_of_range {
                break;
            }
            for j in 0..n {
                for p in u(self.a.colptr[j])..u(self.a.colptr[j + 1]) {
                    self.a.values[p] *= colmax[j];
                    self.a.values[p] *= rowmax[u(self.a.rowidx[p])];
                }
            }
        }
    }

    fn find_dense_columns(&mut self) {
        self.num_dense_cols = 0;
        self.nz_dense = self.rows() + 1;
        let num_cols = u(self.num_cols);
        let mut colcount: Vec<Int> = (0..num_cols).map(|j| self.ai.end(j as Int) - self.ai.begin(j as Int)).collect();
        colcount.sort_unstable();
        for j in 1..num_cols {
            if colcount[j] > 40.max(10 * colcount[j - 1]) {
                self.num_dense_cols = (num_cols - j) as Int;
                self.nz_dense = colcount[j];
                break;
            }
        }
        if self.num_dense_cols > 1000 {
            self.num_dense_cols = 0;
            self.nz_dense = self.rows() + 1;
        }
    }

    // ---- From the solver's space back to the user's ----------------------

    /// IPX's `EvaluateInteriorSolution`: residuals, objectives and gap of
    /// the interior point, in the user's (scaled) space.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_interior_solution(&self, x_solver: &[f64], xl_solver: &[f64], xu_solver: &[f64], y_solver: &[f64], zl_solver: &[f64], zu_solver: &[f64], info: &mut Info) {
        let (nv, nc) = (u(self.num_var), u(self.num_constr));
        let mut x = vec![0.0; nv];
        let mut xl = vec![0.0; nv];
        let mut xu = vec![0.0; nv];
        let mut slack = vec![0.0; nc];
        let mut y = vec![0.0; nc];
        let mut zl = vec![0.0; nv];
        let mut zu = vec![0.0; nv];
        self.dualize_back_interior_solution(
            x_solver, xl_solver, xu_solver, y_solver, zl_solver, zu_solver, &mut x, &mut xl, &mut xu, &mut slack, &mut y, &mut zl, &mut zu,
        );
        let mut rl = vec![0.0; nv];
        for j in 0..nv {
            if self.scaled_lbuser[j].is_finite() {
                rl[j] = self.scaled_lbuser[j] - x[j] + xl[j];
            }
        }
        let mut ru = vec![0.0; nv];
        for j in 0..nv {
            if self.scaled_ubuser[j].is_finite() {
                ru[j] = self.scaled_ubuser[j] - x[j] - xu[j];
            }
        }
        let mut rb = vec![0.0; nc];
        self.multiply_with_scaled_matrix(&x, -1.0, &mut rb, 'N');
        for i in 0..nc {
            rb[i] -= slack[i];
            rb[i] += self.scaled_rhs[i];
        }
        let mut rc = vec![0.0; nv];
        self.multiply_with_scaled_matrix(&y, -1.0, &mut rc, 'T');
        for j in 0..nv {
            rc[j] -= zl[j] - zu[j];
            rc[j] += self.scaled_obj[j];
        }
        self.scale_back_residuals(&mut rb, &mut rc, &mut rl, &mut ru);
        let mut presidual = infnorm(&rb);
        presidual = std_max(presidual, infnorm(&rl));
        presidual = std_max(presidual, infnorm(&ru));
        let dresidual = infnorm(&rc);
        let pobjective = self.offset + dot(&self.scaled_obj, &x);
        let mut dobjective = self.offset + dot(&self.scaled_rhs, &y);
        for j in 0..nv {
            if self.scaled_lbuser[j].is_finite() {
                dobjective += self.scaled_lbuser[j] * zl[j];
            }
            if self.scaled_ubuser[j].is_finite() {
                dobjective -= self.scaled_ubuser[j] * zu[j];
            }
        }
        let objective_gap = (pobjective - dobjective) / (1.0 + 0.5 * (pobjective + dobjective).abs());
        let mut complementarity = 0.0;
        for j in 0..nv {
            if self.scaled_lbuser[j].is_finite() {
                complementarity += xl[j] * zl[j];
            }
            if self.scaled_ubuser[j].is_finite() {
                complementarity += xu[j] * zu[j];
            }
        }
        for i in 0..nc {
            complementarity -= y[i] * slack[i];
        }
        self.scale_back_interior_solution(&mut x, &mut xl, &mut xu, &mut slack, &mut y, &mut zl, &mut zu);
        info.abs_presidual = presidual;
        info.abs_dresidual = dresidual;
        info.rel_presidual = presidual / (1.0 + self.norm_rhs);
        info.rel_dresidual = dresidual / (1.0 + self.norm_obj);
        info.pobjval = pobjective;
        info.dobjval = dobjective;
        info.rel_objgap = objective_gap;
        info.complementarity = complementarity;
        info.normx = infnorm(&x);
        info.normy = infnorm(&y);
        info.normz = std_max(infnorm(&zl), infnorm(&zu));
    }

    /// IPX's `PostsolveInteriorSolution`, into the user's space.
    #[allow(clippy::too_many_arguments)]
    pub fn postsolve_interior_solution(&self, x_solver: &[f64], xl_solver: &[f64], xu_solver: &[f64], y_solver: &[f64], zl_solver: &[f64], zu_solver: &[f64]) -> InteriorSolution {
        let (nv, nc) = (u(self.num_var), u(self.num_constr));
        let mut x = vec![0.0; nv];
        let mut xl = vec![0.0; nv];
        let mut xu = vec![0.0; nv];
        let mut slack = vec![0.0; nc];
        let mut y = vec![0.0; nc];
        let mut zl = vec![0.0; nv];
        let mut zu = vec![0.0; nv];
        self.dualize_back_interior_solution(
            x_solver, xl_solver, xu_solver, y_solver, zl_solver, zu_solver, &mut x, &mut xl, &mut xu, &mut slack, &mut y, &mut zl, &mut zu,
        );
        self.scale_back_interior_solution(&mut x, &mut xl, &mut xu, &mut slack, &mut y, &mut zl, &mut zu);
        (x, xl, xu, slack, y, zl, zu)
    }

    /// IPX's `PostsolveBasicSolution`: x, slack, y, z in the user's space.
    pub fn postsolve_basic_solution(&self, x_solver: &[f64], y_solver: &[f64], z_solver: &[f64], basic_status_solver: &[Int]) -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
        let (nv, nc) = (u(self.num_var), u(self.num_constr));
        let mut x = vec![0.0; nv];
        let mut slack = vec![0.0; nc];
        let mut y = vec![0.0; nc];
        let mut z = vec![0.0; nv];
        let mut cbasis = vec![0; nc];
        let mut vbasis = vec![0; nv];
        self.dualize_back_basic_solution(x_solver, y_solver, z_solver, &mut x, &mut slack, &mut y, &mut z);
        self.dualize_back_basis(basic_status_solver, &mut cbasis, &mut vbasis);
        self.correct_scaled_basic_solution(&mut x, &mut slack, &mut y, &mut z, &cbasis, &vbasis);
        self.scale_back_basic_solution(&mut x, &mut slack, &mut y, &mut z);
        (x, slack, y, z)
    }

    /// IPX's `EvaluateBasicSolution`: the basic solution's objective and
    /// largest primal and dual infeasibilities.
    pub fn evaluate_basic_solution(&self, x_solver: &[f64], y_solver: &[f64], z_solver: &[f64], basic_status_solver: &[Int], info: &mut Info) {
        let (nv, nc) = (u(self.num_var), u(self.num_constr));
        let mut x = vec![0.0; nv];
        let mut slack = vec![0.0; nc];
        let mut y = vec![0.0; nc];
        let mut z = vec![0.0; nv];
        let mut cbasis = vec![0; nc];
        let mut vbasis = vec![0; nv];
        self.dualize_back_basic_solution(x_solver, y_solver, z_solver, &mut x, &mut slack, &mut y, &mut z);
        self.dualize_back_basis(basic_status_solver, &mut cbasis, &mut vbasis);
        self.correct_scaled_basic_solution(&mut x, &mut slack, &mut y, &mut z, &cbasis, &vbasis);
        let pobj = dot(&self.scaled_obj, &x);
        let mut xinfeas = vec![0.0; nv];
        let mut sinfeas = vec![0.0; nc];
        let mut yinfeas = vec![0.0; nc];
        let mut zinfeas = vec![0.0; nv];
        for j in 0..nv {
            if x[j] < self.scaled_lbuser[j] {
                xinfeas[j] = x[j] - self.scaled_lbuser[j];
            }
            if x[j] > self.scaled_ubuser[j] {
                xinfeas[j] = x[j] - self.scaled_ubuser[j];
            }
            if vbasis[j] != IPX_NONBASIC_LB && z[j] > 0.0 {
                zinfeas[j] = z[j];
            }
            if vbasis[j] != IPX_NONBASIC_UB && z[j] < 0.0 {
                zinfeas[j] = z[j];
            }
        }
        for i in 0..nc {
            if self.constr_type[i] == b'<' {
                if slack[i] < 0.0 {
                    sinfeas[i] = slack[i];
                }
                if y[i] > 0.0 {
                    yinfeas[i] = y[i];
                }
            }
            if self.constr_type[i] == b'>' {
                if slack[i] > 0.0 {
                    sinfeas[i] = slack[i];
                }
                if y[i] < 0.0 {
                    yinfeas[i] = y[i];
                }
            }
        }
        self.scale_back_basic_solution(&mut x, &mut slack, &mut y, &mut z);
        self.scale_back_basic_solution(&mut xinfeas, &mut sinfeas, &mut yinfeas, &mut zinfeas);
        info.primal_infeas = std_max(infnorm(&xinfeas), infnorm(&sinfeas));
        info.dual_infeas = std_max(infnorm(&zinfeas), infnorm(&yinfeas));
        info.objval = pobj;
    }

    /// IPX's `PostsolveBasis`: the statuses of the user's constraints
    /// (`cbasis`) and variables (`vbasis`).
    pub fn postsolve_basis(&self, basic_status_solver: &[Int]) -> (Vec<Int>, Vec<Int>) {
        let mut cbasis = vec![0; u(self.num_constr)];
        let mut vbasis = vec![0; u(self.num_var)];
        self.dualize_back_basis(basic_status_solver, &mut cbasis, &mut vbasis);
        for &j in &self.flipped_vars {
            if vbasis[u(j)] == IPX_NONBASIC_LB {
                vbasis[u(j)] = IPX_NONBASIC_UB;
            }
        }
        (cbasis, vbasis)
    }

    #[allow(clippy::too_many_arguments)]
    fn scale_back_interior_solution(&self, x: &mut [f64], xl: &mut [f64], xu: &mut [f64], slack: &mut [f64], y: &mut [f64], zl: &mut [f64], zu: &mut [f64]) {
        if !self.colscale.is_empty() {
            for j in 0..u(self.num_var) {
                let s = self.colscale[j];
                x[j] *= s;
                xl[j] *= s;
                xu[j] *= s;
                zl[j] /= s;
                zu[j] /= s;
            }
        }
        if !self.rowscale.is_empty() {
            for i in 0..u(self.num_constr) {
                y[i] *= self.rowscale[i];
                slack[i] /= self.rowscale[i];
            }
        }
        for &j in &self.flipped_vars {
            let j = u(j);
            x[j] *= -1.0;
            xu[j] = xl[j];
            xl[j] = INF;
            zu[j] = zl[j];
            zl[j] = 0.0;
        }
    }

    fn scale_back_residuals(&self, rb: &mut [f64], rc: &mut [f64], rl: &mut [f64], ru: &mut [f64]) {
        if !self.colscale.is_empty() {
            for j in 0..u(self.num_var) {
                rc[j] /= self.colscale[j];
                rl[j] *= self.colscale[j];
                ru[j] *= self.colscale[j];
            }
        }
        if !self.rowscale.is_empty() {
            for i in 0..u(self.num_constr) {
                rb[i] /= self.rowscale[i];
            }
        }
        for &j in &self.flipped_vars {
            let j = u(j);
            rc[j] *= -1.0;
            ru[j] = -rl[j];
            rl[j] = 0.0;
        }
    }

    fn scale_back_basic_solution(&self, x: &mut [f64], slack: &mut [f64], y: &mut [f64], z: &mut [f64]) {
        if !self.colscale.is_empty() {
            for j in 0..u(self.num_var) {
                x[j] *= self.colscale[j];
                z[j] /= self.colscale[j];
            }
        }
        if !self.rowscale.is_empty() {
            for i in 0..u(self.num_constr) {
                y[i] *= self.rowscale[i];
                slack[i] /= self.rowscale[i];
            }
        }
        for &j in &self.flipped_vars {
            x[u(j)] *= -1.0;
            z[u(j)] *= -1.0;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn dualize_back_interior_solution(
        &self,
        x_solver: &[f64],
        xl_solver: &[f64],
        xu_solver: &[f64],
        y_solver: &[f64],
        zl_solver: &[f64],
        zu_solver: &[f64],
        x_user: &mut [f64],
        xl_user: &mut [f64],
        xu_user: &mut [f64],
        slack_user: &mut [f64],
        y_user: &mut [f64],
        zl_user: &mut [f64],
        zu_user: &mut [f64],
    ) {
        let m = u(self.rows());
        let n = u(self.cols());
        let (nv, nc) = (u(self.num_var), u(self.num_constr));
        if self.dualized {
            for i in 0..m {
                x_user[i] = -y_solver[i];
            }
            for i in 0..nc {
                y_user[i] = match self.constr_type[i] {
                    b'=' => x_solver[i],
                    b'<' => -xu_solver[i],
                    _ => xl_solver[i],
                };
            }
            zl_user[..nv].copy_from_slice(&xl_solver[n..n + nv]);
            for j in 0..nv {
                if !self.scaled_lbuser[j].is_finite() {
                    zl_user[j] = 0.0;
                }
            }
            zu_user.fill(0.0);
            for (&j, &xl) in self.boxed_vars.iter().zip(&xl_solver[nc..]) {
                zu_user[u(j)] = xl;
            }
            for i in 0..m {
                xl_user[i] = if self.scaled_lbuser[i].is_finite() { zl_solver[n + i] } else { INF };
            }
            xu_user.fill(INF);
            for (&j, &zl) in self.boxed_vars.iter().zip(&zl_solver[nc..]) {
                xu_user[u(j)] = zl;
            }
            for i in 0..nc {
                slack_user[i] = match self.constr_type[i] {
                    b'=' => 0.0,
                    b'<' => zu_solver[i],
                    _ => -zl_solver[i],
                };
            }
        } else {
            x_user[..nv].copy_from_slice(&x_solver[..nv]);
            for i in 0..m {
                y_user[i] = match self.constr_type[i] {
                    b'=' => y_solver[i],
                    b'<' => -zl_solver[n + i],
                    _ => zu_solver[n + i],
                };
            }
            zl_user[..nv].copy_from_slice(&zl_solver[..nv]);
            zu_user[..nv].copy_from_slice(&zu_solver[..nv]);
            xl_user[..nv].copy_from_slice(&xl_solver[..nv]);
            xu_user[..nv].copy_from_slice(&xu_solver[..nv]);
            for i in 0..m {
                slack_user[i] = match self.constr_type[i] {
                    b'=' => 0.0,
                    b'<' => xl_solver[n + i],
                    _ => -xu_solver[n + i],
                };
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn dualize_back_basic_solution(
        &self,
        x_solver: &[f64],
        y_solver: &[f64],
        z_solver: &[f64],
        x_user: &mut [f64],
        slack_user: &mut [f64],
        y_user: &mut [f64],
        z_user: &mut [f64],
    ) {
        let m = u(self.rows());
        let n = u(self.cols());
        let (nv, nc) = (u(self.num_var), u(self.num_constr));
        if self.dualized {
            for i in 0..m {
                x_user[i] = -y_solver[i];
            }
            for i in 0..nc {
                slack_user[i] = -z_solver[i];
            }
            y_user[..nc].copy_from_slice(&x_solver[..nc]);
            z_user[..nv].copy_from_slice(&x_solver[n..n + nv]);
            for (&j, &x) in self.boxed_vars.iter().zip(&x_solver[nc..]) {
                z_user[u(j)] -= x;
            }
        } else {
            x_user[..nv].copy_from_slice(&x_solver[..nv]);
            slack_user[..nc].copy_from_slice(&x_solver[n..n + nc]);
            y_user[..nc].copy_from_slice(&y_solver[..nc]);
            z_user[..nv].copy_from_slice(&z_solver[..nv]);
        }
    }

    fn dualize_back_basis(&self, basic_status_solver: &[Int], cbasis_user: &mut [Int], vbasis_user: &mut [Int]) {
        let n = u(self.cols());
        let (nv, nc) = (u(self.num_var), u(self.num_constr));
        if self.dualized {
            for i in 0..nc {
                cbasis_user[i] = if basic_status_solver[i] == IPX_BASIC { IPX_NONBASIC } else { IPX_BASIC };
            }
            for j in 0..nv {
                vbasis_user[j] = if basic_status_solver[n + j] == 0 {
                    if self.scaled_lbuser[j].is_finite() { IPX_NONBASIC_LB } else { IPX_SUPERBASIC }
                } else {
                    IPX_BASIC
                };
            }
            for (&j, &status) in self.boxed_vars.iter().zip(&basic_status_solver[nc..]) {
                if status == IPX_BASIC {
                    vbasis_user[u(j)] = IPX_NONBASIC_UB;
                }
            }
        } else {
            for i in 0..nc {
                cbasis_user[i] = if basic_status_solver[n + i] == IPX_BASIC { IPX_BASIC } else { IPX_NONBASIC };
            }
            vbasis_user[..nv].copy_from_slice(&basic_status_solver[..nv]);
        }
    }

    fn correct_scaled_basic_solution(&self, x: &mut [f64], slack: &mut [f64], y: &mut [f64], z: &mut [f64], cbasis: &[Int], vbasis: &[Int]) {
        for j in 0..u(self.num_var) {
            if vbasis[j] == IPX_NONBASIC_LB {
                x[j] = self.scaled_lbuser[j];
            }
            if vbasis[j] == IPX_NONBASIC_UB {
                x[j] = self.scaled_ubuser[j];
            }
            if vbasis[j] == IPX_BASIC {
                z[j] = 0.0;
            }
        }
        for i in 0..u(self.num_constr) {
            if cbasis[i] == IPX_NONBASIC {
                slack[i] = 0.0;
            }
            if cbasis[i] == IPX_BASIC {
                y[i] = 0.0;
            }
        }
    }

    fn multiply_with_scaled_matrix(&self, rhs: &[f64], alpha: f64, lhs: &mut [f64], trans: char) {
        let (nv, nc) = (self.num_var, self.num_constr);
        if trans == 't' || trans == 'T' {
            if self.dualized {
                for i in 0..nc {
                    scatter_column(&self.ai, i, alpha * rhs[u(i)], lhs);
                }
            } else {
                for j in 0..nv {
                    lhs[u(j)] += alpha * dot_column(&self.ai, j, rhs);
                }
            }
        } else if self.dualized {
            for i in 0..nc {
                lhs[u(i)] += alpha * dot_column(&self.ai, i, rhs);
            }
        } else {
            for j in 0..nv {
                scatter_column(&self.ai, j, alpha * rhs[u(j)], lhs);
            }
        }
    }
}

fn check_vectors(m: Int, n: Int, rhs: &[f64], constr_type: &[u8], obj: &[f64], lb: &[f64], ub: &[f64]) -> Int {
    for rhs_i in &rhs[..u(m)] {
        if !rhs_i.is_finite() {
            return -1;
        }
    }
    for obj_j in &obj[..u(n)] {
        if !obj_j.is_finite() {
            return -2;
        }
    }
    for j in 0..u(n) {
        if !lb[j].is_finite() && lb[j] != -INF {
            return -3;
        }
        if !ub[j].is_finite() && ub[j] != INF {
            return -3;
        }
        if lb[j] > ub[j] {
            return -3;
        }
    }
    for constr_type_i in &constr_type[..u(m)] {
        if !matches!(*constr_type_i, b'=' | b'<' | b'>') {
            return -4;
        }
    }
    0
}

fn check_matrix(m: Int, n: Int, ap: &[Int], ai: &[Int], ax: &[f64]) -> Int {
    if ap[0] != 0 {
        return -5;
    }
    for j in 0..u(n) {
        if ap[j] > ap[j + 1] {
            return -5;
        }
    }
    for ax_p in &ax[..u(ap[u(n)])] {
        if !ax_p.is_finite() {
            return -6;
        }
    }
    let mut marked = vec![-1 as Int; u(m)];
    for j in 0..u(n) {
        for ai_p in &ai[u(ap[j])..u(ap[j + 1])] {
            let i = *ai_p;
            if i < 0 || i >= m {
                return -7;
            }
            if marked[u(i)] == j as Int {
                return -8;
            }
            marked[u(i)] = j as Int;
        }
    }
    0
}

fn equilibration_factor(expmin: i32, expmax: i32, exp: i32) -> f64 {
    if exp < expmin {
        return ldexp(1.0, (expmin - exp + 1) / 2);
    }
    if exp > expmax {
        return ldexp(1.0, -((exp - expmax + 1) / 2));
    }
    1.0
}

fn ldexp(x: f64, exp: i32) -> f64 {
    x * 2f64.powi(exp)
}

/// The exponent `std::frexp` gives.
fn frexp_exp(x: f64) -> i32 {
    crate::util::frexp(x).1
}

/// `std::max(a, b)`: `a` unless `b` is greater.
fn std_max(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// IPX's `PrimalInfeasibility`: the largest bound violation of `x`.
pub(crate) fn primal_infeasibility(model: &Model, x: &[f64]) -> f64 {
    let (lb, ub) = (model.lb(), model.ub());
    let mut infeas = 0.0;
    for j in 0..x.len() {
        infeas = std_max(infeas, lb[j] - x[j]);
        infeas = std_max(infeas, x[j] - ub[j]);
    }
    infeas
}

/// IPX's `DualInfeasibility`: the largest sign violation of `z`.
pub(crate) fn dual_infeasibility(model: &Model, x: &[f64], z: &[f64]) -> f64 {
    let (lb, ub) = (model.lb(), model.ub());
    let mut infeas = 0.0;
    for j in 0..x.len() {
        if x[j] > lb[j] {
            infeas = std_max(infeas, z[j]);
        }
        if x[j] < ub[j] {
            infeas = std_max(infeas, -z[j]);
        }
    }
    infeas
}

pub(crate) fn primal_residual(model: &Model, x: &[f64]) -> f64 {
    let ait = model.ait();
    let b = model.b();
    let mut res = 0.0;
    for (i, b_i) in b.iter().enumerate() {
        let r = *b_i - dot_column(ait, i as Int, x);
        res = std_max(res, r.abs());
    }
    res
}

pub(crate) fn dual_residual(model: &Model, y: &[f64], z: &[f64]) -> f64 {
    let ai = model.ai();
    let c = model.c();
    let mut res = 0.0;
    for j in 0..c.len() {
        let r = c[j] - z[j] - dot_column(ai, j as Int, y);
        res = std_max(res, r.abs());
    }
    res
}
