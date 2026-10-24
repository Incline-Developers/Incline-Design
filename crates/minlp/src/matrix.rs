//! Port of `matrix.c`: the constraint matrix data structure the presolvers
//! work on, in column and row major format. The exact-arithmetic parts are
//! not ported.

use crate::{
    api,
    retcode::{Retcode, ScipResult},
    scip::Scip,
    types::{ConsId, LockType, VarId},
    var::{var_get_n_locks_down_type, var_get_n_locks_up_type},
};

/// `SCIP_MATRIX`: constraint matrix data structure in column and row major
/// format.
#[derive(Debug, Default)]
pub struct Matrix {
    /// Coefficients in column major format.
    pub colmatval: Vec<f64>,
    /// Row indexes in column major format.
    pub colmatind: Vec<i32>,
    /// Column storage offset.
    pub colmatbeg: Vec<i32>,
    /// Number of row entries per column.
    pub colmatcnt: Vec<i32>,
    /// Complete number of columns.
    pub ncols: i32,
    /// Lower bound per variable.
    pub lb: Vec<f64>,
    /// Upper bound per variable.
    pub ub: Vec<f64>,
    /// Number of up locks per variable.
    pub nuplocks: Vec<i32>,
    /// Number of down locks per variable.
    pub ndownlocks: Vec<i32>,
    /// Variables pointer.
    pub vars: Vec<VarId>,
    /// Coefficients in row major format.
    pub rowmatval: Vec<f64>,
    /// Column indexed in row major format.
    pub rowmatind: Vec<i32>,
    /// Row storage offset.
    pub rowmatbeg: Vec<i32>,
    /// Number of column entries per row.
    pub rowmatcnt: Vec<i32>,
    /// Complete number of rows.
    pub nrows: i32,
    /// Left hand side per row.
    pub lhs: Vec<f64>,
    /// Right hand side per row.
    pub rhs: Vec<f64>,
    /// Constraints pointer.
    pub cons: Vec<ConsId>,
    /// Is right hand side infinity.
    pub isrhsinfinite: Vec<bool>,
    /// Sparsity counter.
    pub nnonzs: i32,
    /// Min activity per row.
    pub minactivity: Vec<f64>,
    /// Max activity per row.
    pub maxactivity: Vec<f64>,
    /// Min activity negative infinity counter.
    pub minactivityneginf: Vec<i32>,
    /// Min activity positive infinity counter.
    pub minactivityposinf: Vec<i32>,
    /// Max activity negative infinity counter.
    pub maxactivityneginf: Vec<i32>,
    /// Max activity positive infinity counter.
    pub maxactivityposinf: Vec<i32>,
}

/*
 * private functions
 */

/// `addRow`: add one row to the constraint matrix; returns whether the
/// constraint was added to the matrix.
fn add_row(scip: &Scip, matrix: &mut Matrix, vars: &[VarId], vals: &[f64], lhs: f64, rhs: f64) -> bool {
    let rowidx = matrix.nrows as usize;
    let mut rangedorequality = false;
    let factor;

    if scip.set.is_infinity(-lhs) {
        factor = -1.0;
        matrix.lhs[rowidx] = -rhs;
        matrix.rhs[rowidx] = scip.set.infinity();
        matrix.isrhsinfinite[rowidx] = true;
    } else {
        factor = 1.0;
        matrix.lhs[rowidx] = lhs;
        matrix.rhs[rowidx] = rhs;
        matrix.isrhsinfinite[rowidx] = scip.set.is_infinity(matrix.rhs[rowidx]);
        if !scip.set.is_infinity(rhs) {
            rangedorequality = true;
        }
    }

    if scip.set.is_infinity(-matrix.lhs[rowidx]) {
        // Ignore redundant constraint.
        return false;
    }

    matrix.rowmatbeg[rowidx] = matrix.nnonzs;

    // = or ranged.
    if rangedorequality {
        for (j, &var) in vars.iter().enumerate() {
            // Ignore variables with very small coefficients.
            if scip.set.is_zero(vals[j]) {
                continue;
            }

            matrix.rowmatval.push(factor * vals[j]);
            let probindex = scip.vars[var].probindex as usize;

            matrix.nuplocks[probindex] += 1;
            matrix.ndownlocks[probindex] += 1;

            matrix.rowmatind.push(probindex as i32);

            matrix.nnonzs += 1;
        }
    }
    // >= or <=.
    else {
        for (j, &var) in vars.iter().enumerate() {
            // Ignore variables with very small coefficients.
            if scip.set.is_zero(vals[j]) {
                continue;
            }

            // Due to the factor, <= constraints will be transfered to >=.
            let val = factor * vals[j];
            matrix.rowmatval.push(val);
            let probindex = scip.vars[var].probindex as usize;

            if val > 0.0 {
                matrix.ndownlocks[probindex] += 1;
            } else {
                matrix.nuplocks[probindex] += 1;
            }

            matrix.rowmatind.push(probindex as i32);

            matrix.nnonzs += 1;
        }
    }

    matrix.rowmatcnt[rowidx] = matrix.nnonzs - matrix.rowmatbeg[rowidx];

    matrix.nrows += 1;

    true
}

/// `addConstraint`: add one constraint to matrix (`vals` `None` for all
/// coefficients one); returns whether the row was added to the matrix.
fn add_constraint(scip: &mut Scip, matrix: &mut Matrix, vars: &[VarId], vals: Option<&[f64]>, mut lhs: f64, mut rhs: f64) -> ScipResult<bool> {
    // Constraint is redundant.
    if scip.set.is_infinity(-lhs) && scip.set.is_infinity(rhs) {
        return Ok(false);
    }

    // We do not add empty constraints to the matrix.
    if vars.is_empty() {
        return Ok(false);
    }

    let mut nactivevars = vars.len() as i32;
    let mut activeconstant = 0.0;

    // Duplicate variable and value array.
    let mut activevars = crate::cvec::CVec(vars.to_vec());
    let mut activevals = crate::cvec::CVec(match vals {
        Some(v) => v.to_vec(),
        None => vec![1.0; vars.len()],
    });

    // Retransform given variables to active variables (`getActiveVariables`).
    let mut requiredsize = 0;
    let varssize = nactivevars;
    crate::var::var_get_active_representatives(scip, &mut activevars, &mut activevals, &mut nactivevars, varssize, &mut activeconstant, &mut requiredsize)?;
    if requiredsize > nactivevars {
        activevars.0.resize(requiredsize as usize, crate::var::NONE);
        activevals.0.resize(requiredsize as usize, 0.0);

        // Call function a second time with enough memory.
        crate::var::var_get_active_representatives(
            scip,
            &mut activevars,
            &mut activevals,
            &mut nactivevars,
            requiredsize,
            &mut activeconstant,
            &mut requiredsize,
        )?;
    }

    // Adapt left and right hand side.
    if !scip.set.is_infinity(-lhs) {
        lhs -= activeconstant;
    }
    if !scip.set.is_infinity(rhs) {
        rhs -= activeconstant;
    }

    // Add single row to matrix.
    if nactivevars > 0 {
        let n = nactivevars as usize;
        return Ok(add_row(scip, matrix, &activevars.0[..n], &activevals.0[..n], lhs, rhs));
    }

    Ok(false)
}

/// `setColumnMajorFormat`: transform row major format into column major
/// format.
fn set_column_major_format(matrix: &mut Matrix) {
    let ncols = matrix.ncols as usize;
    let mut fillidx = vec![0i32; ncols];
    matrix.colmatcnt.iter_mut().for_each(|c| *c = 0);

    for i in 0..matrix.nrows as usize {
        let beg = matrix.rowmatbeg[i] as usize;
        for k in beg..beg + matrix.rowmatcnt[i] as usize {
            let colidx = matrix.rowmatind[k] as usize;
            matrix.colmatcnt[colidx] += 1;
        }
    }

    matrix.colmatbeg[0] = 0;
    for i in 0..ncols.saturating_sub(1) {
        matrix.colmatbeg[i + 1] = matrix.colmatbeg[i] + matrix.colmatcnt[i];
    }

    for i in 0..matrix.nrows as usize {
        let beg = matrix.rowmatbeg[i] as usize;
        for k in beg..beg + matrix.rowmatcnt[i] as usize {
            let colidx = matrix.rowmatind[k] as usize;
            let pos = (matrix.colmatbeg[colidx] + fillidx[colidx]) as usize;
            matrix.colmatval[pos] = matrix.rowmatval[k];
            matrix.colmatind[pos] = i as i32;
            fillidx[colidx] += 1;
        }
    }
}

/// `calcActivityBounds`: calculate min/max activity per row.
fn calc_activity_bounds(scip: &Scip, matrix: &mut Matrix) {
    let inf = scip.set.infinity();
    for row in 0..matrix.nrows as usize {
        matrix.minactivity[row] = 0.0;
        matrix.maxactivity[row] = 0.0;
        matrix.minactivityneginf[row] = 0;
        matrix.minactivityposinf[row] = 0;
        matrix.maxactivityneginf[row] = 0;
        matrix.maxactivityposinf[row] = 0;

        let beg = matrix.rowmatbeg[row] as usize;
        for k in beg..beg + matrix.rowmatcnt[row] as usize {
            // Get column index.
            let col = matrix.rowmatind[k] as usize;

            // Get variable coefficient.
            let val = matrix.rowmatval[k];

            // Positive coefficient.
            if val > 0.0 {
                if scip.set.is_infinity(matrix.ub[col]) {
                    matrix.maxactivityposinf[row] += 1;
                } else {
                    matrix.maxactivity[row] += val * matrix.ub[col];
                }

                if scip.set.is_infinity(-matrix.lb[col]) {
                    matrix.minactivityneginf[row] += 1;
                } else {
                    matrix.minactivity[row] += val * matrix.lb[col];
                }
            }
            // Negative coefficient.
            else {
                if scip.set.is_infinity(-matrix.lb[col]) {
                    matrix.maxactivityneginf[row] += 1;
                } else {
                    matrix.maxactivity[row] += val * matrix.lb[col];
                }

                if scip.set.is_infinity(matrix.ub[col]) {
                    matrix.minactivityposinf[row] += 1;
                } else {
                    matrix.minactivity[row] += val * matrix.ub[col];
                }
            }
        }

        // Consider infinite bound contributions for the activities.
        if matrix.maxactivityneginf[row] + matrix.maxactivityposinf[row] > 0 {
            matrix.maxactivity[row] = inf;
        }

        if matrix.minactivityneginf[row] + matrix.minactivityposinf[row] > 0 {
            matrix.minactivity[row] = -inf;
        }
    }
}

/*
 * public functions
 */

/// The result of [`matrix_create`].
pub struct MatrixCreate {
    /// The matrix, if the initialization was successful.
    pub matrix: Option<Matrix>,
    /// Are all constraint represented within the matrix?
    pub complete: bool,
    /// Was the problem detected to be infeasible during matrix creation?
    pub infeasible: bool,
}

/// `SCIPmatrixCreate`: initialize matrix by copying all check constraints;
/// completeness is checked by testing whether all check constraints are from
/// a list of linear constraint handlers that can be represented.
#[allow(clippy::too_many_arguments)]
pub fn matrix_create(
    scip: &mut Scip,
    onlyifcomplete: bool,
    naddconss: &mut i32,
    ndelconss: &mut i32,
    nchgcoefs: &mut i32,
    nchgbds: &mut i32,
    nfixedvars: &mut i32,
) -> ScipResult<MatrixCreate> {
    use crate::cons_setppc::SetppcType;

    let mut out = MatrixCreate {
        matrix: None,
        complete: false,
        infeasible: false,
    };
    let mut nnonzstmp = 0;

    // Return if no variables or constraints are present.
    if api::get_n_vars(scip) == 0 || api::get_n_conss(scip) == 0 {
        return Ok(out);
    }

    // Return if pricers are present and the matrix should only be built when
    // complete.
    if onlyifcomplete && api::get_n_active_pricers(scip) != 0 {
        return Ok(out);
    }

    // Loop over all constraint handlers and collect the number of checked
    // constraints.
    let conshdlrs: Vec<usize> = scip.set.conshdlrs.0[..scip.set.nconshdlrs as usize].to_vec();
    let mut nconss = 0;
    let mut nconssall = 0;

    for &h in &conshdlrs {
        let nconshdlrconss = scip.conshdlrs[h].ncheckconss;

        if nconshdlrconss > 0 {
            let name = scip.conshdlrs[h].name.as_str();

            if matches!(name, "linear" | "knapsack" | "setppc" | "logicor" | "varbound" | "exactlinear") {
                // Increment number of supported constraints.
                nconss += nconshdlrconss;
            }

            // Increment number of supported and unsupported constraints.
            nconssall += nconshdlrconss;
        }
    }

    // We only abort the matrix creation process if requested, because it makes
    // sometimes sense to work on an incomplete matrix as long as the number of
    // interesting variable uplocks or downlocks of the matrix and scip are the
    // same.
    if nconss < nconssall {
        if onlyifcomplete {
            return Ok(out);
        }
    } else {
        // All constraints represented within the matrix.
        out.complete = true;
    }

    // Do nothing if we have no checked constraints.
    if nconss == 0 {
        return Ok(out);
    }

    let mut stopped = false;

    // First, clean up aggregations and fixings in varbound costraints, since
    // this can lead to boundchanges and the varbound constraint can get
    // downgraded to a linear constraint.
    out.infeasible = crate::cons_varbound::cleanup_conss_varbound(scip, true, naddconss, ndelconss, nchgbds)?;
    if out.infeasible {
        return Ok(out);
    }

    // Next, clean up aggregations and fixings in setppc costraints, since this
    // can lead to fixings and the setppc constraint can get downgraded to a
    // linear constraint.
    out.infeasible = crate::cons_setppc::cleanup_conss_setppc(scip, true, naddconss, ndelconss, nchgcoefs, nfixedvars)?;
    if out.infeasible {
        return Ok(out);
    }

    // Next, clean up aggregations and fixings in logicor costraints, since this
    // cannot lead to further fixings but the logicor constraint can also get
    // downgraded to a linear constraint.
    crate::cons_logicor::cleanup_conss_logicor(scip, true, naddconss, ndelconss, nchgcoefs)?;

    // Finally, clean up aggregations and fixings in knapsack and linear
    // constraints since now no new linaer constraints can come up due to
    // downgrading and the remaining cleanup methods cannot fix any more
    // variables.
    out.infeasible = crate::cons_knapsack::cleanup_conss_knapsack(scip, true, ndelconss)?;
    if out.infeasible {
        return Ok(out);
    }

    out.infeasible = crate::cons_linear::cleanup_conss_linear(scip, true, ndelconss)?;
    if out.infeasible {
        return Ok(out);
    }

    let vars = api::get_vars(scip).to_vec();
    let nvars = vars.len();

    // Approximate number of nonzeros by taking for each variable the number of
    // up- and downlocks; this counts nonzeros in equalities twice, but can be
    // at most two times as high as the exact number.
    for &var in vars.iter().rev() {
        nnonzstmp += var_get_n_locks_down_type(scip, var, LockType::Model);
        nnonzstmp += var_get_n_locks_up_type(scip, var, LockType::Model);
    }

    // Do nothing if we have no entries.
    if nnonzstmp == 0 {
        return Ok(out);
    }

    // Build the matrix structure.
    let nc = nconss as usize;
    let nnz = nnonzstmp as usize;
    let mut matrix = Matrix {
        colmatval: vec![0.0; nnz],
        colmatind: vec![0; nnz],
        colmatbeg: vec![0; nvars],
        colmatcnt: vec![0; nvars],
        ncols: nvars as i32,
        lb: vec![0.0; nvars],
        ub: vec![0.0; nvars],
        nuplocks: vec![0; nvars],
        ndownlocks: vec![0; nvars],
        vars,
        rowmatval: Vec::with_capacity(nnz),
        rowmatind: Vec::with_capacity(nnz),
        rowmatbeg: vec![0; nc],
        rowmatcnt: vec![0; nc],
        nrows: 0,
        lhs: vec![0.0; nc],
        rhs: vec![0.0; nc],
        cons: vec![crate::var::NONE; nc],
        isrhsinfinite: vec![false; nc],
        nnonzs: 0,
        minactivity: vec![0.0; nc],
        maxactivity: vec![0.0; nc],
        minactivityneginf: vec![0; nc],
        minactivityposinf: vec![0; nc],
        maxactivityneginf: vec![0; nc],
        maxactivityposinf: vec![0; nc],
    };

    // Init bounds.
    for v in 0..nvars {
        let var = matrix.vars[v];
        matrix.lb[v] = scip.vars[var].glbdom.lb;
        matrix.ub[v] = scip.vars[var].glbdom.ub;
    }

    let mut cnt = 0;

    // Loop a second time over constraints handlers and add supported
    // constraints to the matrix.
    for &h in &conshdlrs {
        if crate::solve::solve_is_stopped(scip, false) || (onlyifcomplete && !out.complete) {
            stopped = true;
            break;
        }

        let name = scip.conshdlrs[h].name.clone();
        let ncheck = scip.conshdlrs[h].ncheckconss as usize;
        let conshdlrconss: Vec<ConsId> = scip.conshdlrs[h].checkconss.0[..ncheck].to_vec();

        // The loop guard of each kind: `c % 1000 != 0 || !SCIPisStopped(scip)`.
        let go_on = |scip: &mut Scip, c: usize| !c.is_multiple_of(1000) || !crate::solve::solve_is_stopped(scip, false);

        match name.as_str() {
            "linear" | "knapsack" | "setppc" | "logicor" | "varbound" => {
                let mut c = 0;
                while c < ncheck && go_on(scip, c) {
                    let cons = conshdlrconss[c];
                    c += 1;

                    // Do not include constraints that can be altered due to column
                    // generation.
                    if scip.conss[cons].modifiable {
                        out.complete = false;

                        if onlyifcomplete {
                            break;
                        }

                        continue;
                    }

                    let inf = scip.set.infinity();
                    let rowadded = match name.as_str() {
                        "linear" => {
                            let vars = crate::cons_linear::get_vars_linear(scip, cons).to_vec();
                            let vals = crate::cons_linear::get_vals_linear(scip, cons).to_vec();
                            let (lhs, rhs) = (crate::cons_linear::get_lhs_linear(scip, cons), crate::cons_linear::get_rhs_linear(scip, cons));
                            add_constraint(scip, &mut matrix, &vars, Some(&vals), lhs, rhs)?
                        }
                        "knapsack" => {
                            let vars = crate::cons_knapsack::get_vars_knapsack(scip, cons).to_vec();
                            let consvals: Vec<f64> = crate::cons_knapsack::get_weights_knapsack(scip, cons).iter().map(|&w| w as f64).collect();
                            let capacity = crate::cons_knapsack::get_capacity_knapsack(scip, cons) as f64;
                            add_constraint(scip, &mut matrix, &vars, Some(&consvals), -inf, capacity)?
                        }
                        "setppc" => {
                            let (lhs, rhs) = match crate::cons_setppc::get_type_setppc(scip, cons) {
                                SetppcType::Partitioning => (1.0, 1.0),
                                SetppcType::Packing => (-inf, 1.0),
                                SetppcType::Covering => (1.0, inf),
                            };
                            let vars = crate::cons_setppc::get_vars_setppc(scip, cons).to_vec();
                            add_constraint(scip, &mut matrix, &vars, None, lhs, rhs)?
                        }
                        "logicor" => {
                            let vars = crate::cons_logicor::get_vars_logicor(scip, cons).to_vec();
                            add_constraint(scip, &mut matrix, &vars, None, 1.0, inf)?
                        }
                        _ => {
                            let consvars = [crate::cons_varbound::get_var_varbound(scip, cons), crate::cons_varbound::get_vbdvar_varbound(scip, cons)];
                            let consvals = [1.0, crate::cons_varbound::get_vbdcoef_varbound(scip, cons)];
                            let (lhs, rhs) = (crate::cons_varbound::get_lhs_varbound(scip, cons), crate::cons_varbound::get_rhs_varbound(scip, cons));
                            add_constraint(scip, &mut matrix, &consvars, Some(&consvals), lhs, rhs)?
                        }
                    };

                    if rowadded {
                        matrix.cons[cnt] = cons;
                        cnt += 1;
                    }
                }
            }
            "exactlinear" => return Err(Retcode::InvalidCall),
            _ => {}
        }
    }

    if out.complete {
        let mut lockmismatch = false;

        for i in 0..matrix.ncols as usize {
            if matrix_uplock_conflict(scip, &matrix, i) || matrix_downlock_conflict(scip, &matrix, i) {
                lockmismatch = true;
                break;
            }
        }

        if lockmismatch {
            out.complete = false;
            if onlyifcomplete {
                stopped = true;
            }
        }
    }

    if !stopped {
        // Calculate row activity bounds.
        calc_activity_bounds(scip, &mut matrix);

        // Transform row major format into column major format.
        set_column_major_format(&mut matrix);

        out.matrix = Some(matrix);
    }

    Ok(out)
}

/// `SCIPmatrixRemoveColumnBounds`: removes the bounds of a column and
/// updates the activities accordingly.
pub fn matrix_remove_column_bounds(scip: &Scip, matrix: &mut Matrix, col: usize) {
    let inf = scip.set.infinity();
    let beg = matrix.colmatbeg[col] as usize;
    let colmatend = beg + matrix.colmatcnt[col] as usize;

    for i in beg..colmatend {
        let row = matrix.colmatind[i] as usize;
        let val = matrix.colmatval[i];

        // Set lower bound to -infinity if necessary.
        if !scip.set.is_infinity(-matrix.lb[col]) {
            if val > 0.0 {
                matrix.minactivityneginf[row] += 1;
            } else {
                matrix.maxactivityneginf[row] += 1;
            }
        }

        // Set upper bound to infinity if necessary.
        if !scip.set.is_infinity(matrix.ub[col]) {
            if val > 0.0 {
                matrix.maxactivityposinf[row] += 1;
            } else {
                matrix.minactivityposinf[row] += 1;
            }
        }

        // Mark the activities of the rows to be infinite.
        matrix.maxactivity[row] = inf;
        matrix.minactivity[row] = -inf;
    }

    matrix.lb[col] = -inf;
    matrix.ub[col] = inf;
}

/// The class refinement both `SCIPmatrixGetParallelRows` and
/// `SCIPmatrixGetParallelCols` run over the lines (columns, or rows) of the
/// other direction: `lines` yields, per line, its (index, coefficient)
/// entries, or `None` to skip it; `n` is the number of classified items.
pub(crate) fn refine_parallel_classes(scip: &Scip, n: usize, lines: &mut dyn Iterator<Item = Option<Vec<(i32, f64)>>>, scale: &mut [f64], pclass: &mut [i32]) {
    let mut classsizes = vec![0i32; n];
    let mut pcset = vec![0i32; n];
    let mut values = vec![0.0f64; n];
    let mut indices = vec![0i32; n];
    let mut pcs = vec![0i32; n];

    // Init.
    scale.iter_mut().for_each(|s| *s = 0.0);
    pclass.iter_mut().for_each(|p| *p = 0);
    if n > 0 {
        classsizes[0] = n as i32;
    }
    let mut pcsetfill = 0usize;
    for t in 1..n {
        pcset[pcsetfill] = t as i32;
        pcsetfill += 1;
    }

    for entries in lines {
        let Some(entries) = entries else { continue };
        let cnt = entries.len();

        let mut i = 0usize;
        for &(idx, aij) in &entries {
            let idx = idx as usize;
            if scale[idx] == 0.0 {
                scale[idx] = aij;
            }

            indices[i] = idx as i32;
            values[i] = aij / scale[idx];
            let pc = pclass[idx] as usize;

            // Update class sizes and pclass set.
            classsizes[pc] -= 1;
            if classsizes[pc] == 0 {
                pcset[pcsetfill] = pc as i32;
                pcsetfill += 1;
            }
            pcs[i] = pc as i32;

            i += 1;
        }

        // Sort on the pclass values.
        if i > 1 {
            crate::sort::sort(&mut pcs[..i], (&mut indices[..i], &mut values[..i]), crate::sort::cmp_int);
        }

        let mut k = 0usize;
        loop {
            let startpc = pcs[k];
            let startk = k;

            // Find pclass-sets.
            while k < i && pcs[k] == startpc {
                k += 1;
            }

            // Sort on the A values which have equal pclass values.
            if k - startk > 1 {
                crate::sort::sort(&mut values[startk..k], (&mut indices[startk..k],), crate::sort::cmp_real);
            }

            let mut t = 0usize;
            loop {
                let startval = values[startk + t];
                let startt = t;

                // Find A-sets.
                while t < k - startk && scip.set.is_eq(startval, values[startk + t]) {
                    t += 1;
                }

                // Get new pclass.
                let newpclass = pcset[0];
                pcsetfill -= 1;
                pcset[0] = pcset[pcsetfill];

                // Renumbering.
                for m in startk + startt..startk + t {
                    pclass[indices[m] as usize] = newpclass;
                    classsizes[newpclass as usize] += 1;
                }

                if t == k - startk {
                    break;
                }
            }

            if k == cnt {
                break;
            }
        }
    }
}

/// `SCIPmatrixGetParallelRows`: detect parallel rows of matrix; rhs/lhs are
/// ignored.
pub fn matrix_get_parallel_rows(scip: &Scip, matrix: &Matrix, scale: &mut [f64], pclass: &mut [i32]) {
    // Loop over all columns.
    let mut lines = (0..matrix.ncols as usize).map(|c| {
        if matrix.colmatcnt[c] == 0 {
            return None;
        }
        let beg = matrix.colmatbeg[c] as usize;
        Some((beg..beg + matrix.colmatcnt[c] as usize).map(|k| (matrix.colmatind[k], matrix.colmatval[k])).collect())
    });
    refine_parallel_classes(scip, matrix.nrows as usize, &mut lines, scale, pclass);
}

/// `SCIPmatrixGetParallelCols`: detect parallel columns of matrix; obj
/// coefficients are ignored. `varineq` is set for variables within an
/// equation or ranged row.
pub fn matrix_get_parallel_cols(scip: &Scip, matrix: &Matrix, scale: &mut [f64], pclass: &mut [i32], varineq: &mut [bool]) {
    // Loop over all rows; we consider only equations or ranged rows.
    let mut lines = (0..matrix.nrows as usize).map(|r| {
        if matrix.isrhsinfinite[r] {
            return None;
        }
        let beg = matrix.rowmatbeg[r] as usize;
        let entries: Vec<(i32, f64)> = (beg..beg + matrix.rowmatcnt[r] as usize).map(|k| (matrix.rowmatind[k], matrix.rowmatval[k])).collect();
        Some(entries)
    });
    // Remember the variables that are part of an equation or ranged row (C
    // sets them inside the same loop; nothing else reads `varineq` there).
    for r in 0..matrix.nrows as usize {
        if !matrix.isrhsinfinite[r] {
            let beg = matrix.rowmatbeg[r] as usize;
            for k in beg..beg + matrix.rowmatcnt[r] as usize {
                varineq[matrix.rowmatind[k] as usize] = true;
            }
        }
    }
    refine_parallel_classes(scip, matrix.ncols as usize, &mut lines, scale, pclass);
}

/// `SCIPmatrixUplockConflict`: get if conflicting uplocks of a specific
/// variable present.
pub fn matrix_uplock_conflict(scip: &Scip, matrix: &Matrix, col: usize) -> bool {
    var_get_n_locks_up_type(scip, matrix.vars[col], LockType::Model) != matrix.nuplocks[col]
}

/// `SCIPmatrixDownlockConflict`: get if conflicting downlocks of a specific
/// variable present.
pub fn matrix_downlock_conflict(scip: &Scip, matrix: &Matrix, col: usize) -> bool {
    var_get_n_locks_down_type(scip, matrix.vars[col], LockType::Model) != matrix.ndownlocks[col]
}

impl Matrix {
    /// `SCIPmatrixGetColNNonzs`.
    #[inline]
    pub fn col_nnonzs(&self, col: usize) -> i32 {
        self.colmatcnt[col]
    }

    /// The column's (row index, coefficient) range (`SCIPmatrixGetColIdxPtr`
    /// and `SCIPmatrixGetColValPtr` with `SCIPmatrixGetColNNonzs`).
    #[inline]
    pub fn col_range(&self, col: usize) -> std::ops::Range<usize> {
        let beg = self.colmatbeg[col] as usize;
        beg..beg + self.colmatcnt[col] as usize
    }

    /// `SCIPmatrixGetRowNNonzs`.
    #[inline]
    pub fn row_nnonzs(&self, row: usize) -> i32 {
        self.rowmatcnt[row]
    }

    /// The row's (column index, coefficient) range (`SCIPmatrixGetRowIdxPtr`
    /// and `SCIPmatrixGetRowValPtr` with `SCIPmatrixGetRowNNonzs`).
    #[inline]
    pub fn row_range(&self, row: usize) -> std::ops::Range<usize> {
        let beg = self.rowmatbeg[row] as usize;
        beg..beg + self.rowmatcnt[row] as usize
    }
}
