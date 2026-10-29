//! IPX's `Crossover` (`crossover.cc`): from a complementary primal-dual
//! point to a basic solution, by dual then primal pushes.

use super::{
    IPX_ERROR_TIME_INTERRUPT, IPX_ERROR_USER_INTERRUPT, IPX_STATUS_FAILED, IPX_STATUS_OPTIMAL, IPX_STATUS_TIME_LIMIT, IPX_STATUS_USER_INTERRUPT, Info,
    basis::{Basis, copy_basic},
    control::Control,
    model::{Model, dual_infeasibility, dual_residual, primal_infeasibility, primal_residual},
    sparse::{IndexedVector, Int, sortperm, u},
};

const PIVOT_ZERO_TOL: f64 = 1e-5;

pub(crate) struct Crossover<'c> {
    control: &'c Control,
    pub primal_pushes: Int,
    pub dual_pushes: Int,
    pub primal_pivots: Int,
    pub dual_pivots: Int,
}

impl<'c> Crossover<'c> {
    pub fn new(control: &'c Control) -> Self {
        Self {
            control,
            primal_pushes: 0,
            dual_pushes: 0,
            primal_pivots: 0,
            dual_pivots: 0,
        }
    }

    /// Push every dual superbasic (basic with nonzero z) and then every
    /// primal superbasic (nonbasic between its bounds), in the order of
    /// `weights`.
    pub fn push_all(&mut self, basis: &mut Basis<'_>, x: &mut [f64], y: &mut [f64], z: &mut [f64], weights: Option<&[f64]>, info: &mut Info) {
        let model = basis.model();
        let m = u(model.rows());
        let n = u(model.cols());
        let (lb, ub) = (model.lb(), model.ub());
        let perm = sortperm((n + m) as Int, weights, false);
        // IPX logs these residuals, so it computes them.
        let _ = (primal_residual(model, x), dual_residual(model, y, z));
        let dual_superbasics: Vec<Int> = perm.iter().copied().filter(|&j| basis.is_basic(j) && z[u(j)] != 0.0).collect();
        self.push_dual_from_x(basis, y, z, &dual_superbasics, x, info);
        debug_assert_eq!(dual_infeasibility(model, x, z), 0.0);
        if info.status_crossover != IPX_STATUS_OPTIMAL {
            return;
        }
        let primal_superbasics: Vec<Int> = perm
            .iter()
            .rev()
            .copied()
            .filter(|&j| {
                let ju = u(j);
                basis.is_nonbasic(j) && x[ju] != lb[ju] && x[ju] != ub[ju] && !(lb[ju].is_infinite() && ub[ju].is_infinite() && x[ju] == 0.0)
            })
            .collect();
        self.push_primal(basis, x, &primal_superbasics, None, info);
        debug_assert_eq!(primal_infeasibility(model, x), 0.0);
        if info.status_crossover != IPX_STATUS_OPTIMAL {
            return;
        }
        let _ = (primal_residual(model, x), dual_residual(model, y, z));
        info.status_crossover = IPX_STATUS_OPTIMAL;
    }

    pub fn push_primal(&mut self, basis: &mut Basis<'_>, x: &mut [f64], variables: &[Int], fixed_at_bound: Option<&[bool]>, info: &mut Info) {
        let model: &Model = basis.model();
        let m = u(model.rows());
        let (lb, ub) = (model.lb(), model.ub());
        let mut ftran = IndexedVector::new(m);
        let feastol = if model.dualized() {
            self.control.params.dfeasibility_tol
        } else {
            self.control.params.pfeasibility_tol
        };
        self.primal_pushes = 0;
        self.primal_pivots = 0;
        let mut xbasic = copy_basic(x, basis);
        let mut lbbasic = copy_basic(lb, basis);
        let mut ubbasic = copy_basic(ub, basis);
        if let Some(fixed_at_bound) = fixed_at_bound {
            for p in 0..m {
                let j = u(basis.at(p));
                if fixed_at_bound[j] {
                    lbbasic[p] = x[j];
                    ubbasic[p] = x[j];
                }
            }
        }
        let mut next = 0;
        while next < variables.len() {
            info.errflag = self.control.interrupt_check();
            if info.errflag != 0 {
                break;
            }
            let jn = variables[next];
            let jnu = u(jn);
            if x[jnu] == lb[jnu] || x[jnu] == ub[jnu] || (x[jnu] == 0.0 && lb[jnu].is_infinite() && ub[jnu].is_infinite()) {
                next += 1;
                continue;
            }
            let move_to = if lb[jnu].is_finite() && ub[jnu].is_finite() {
                if x[jnu] - lb[jnu] <= ub[jnu] - x[jnu] { lb[jnu] } else { ub[jnu] }
            } else if lb[jnu].is_finite() {
                lb[jnu]
            } else if ub[jnu].is_finite() {
                ub[jnu]
            } else {
                0.0
            };
            let mut step = x[jnu] - move_to;
            basis.solve_for_update_into(jn, &mut ftran);
            let mut block_at_lb = true;
            let pblock = primal_ratio_test(&xbasic, &ftran, &lbbasic, &ubbasic, step, feastol, &mut block_at_lb);
            let jb = if pblock >= 0 { basis.at(u(pblock)) } else { -1 };
            if pblock >= 0 {
                let pivot = ftran.elements[u(pblock)];
                let mut exchanged = false;
                info.errflag = basis.exchange_if_stable(jb, jn, pivot, -1, &mut exchanged);
                if info.errflag != 0 {
                    // IPX logs this, so it is computed.
                    let _ = basis.min_singular_value();
                    break;
                }
                if !exchanged {
                    continue;
                }
                self.primal_pivots += 1;
                let pb = u(pblock);
                step = if block_at_lb {
                    (lbbasic[pb] - xbasic[pb]) / ftran.elements[pb]
                } else {
                    (ubbasic[pb] - xbasic[pb]) / ftran.elements[pb]
                };
            }
            if step != 0.0 {
                ftran.for_each_nonzero(|p, pivot| {
                    xbasic[p] += step * pivot;
                    xbasic[p] = std_max(xbasic[p], lbbasic[p]);
                    xbasic[p] = std_min(xbasic[p], ubbasic[p]);
                });
                x[jnu] -= step;
            }
            if pblock >= 0 {
                let pb = u(pblock);
                x[u(jb)] = if block_at_lb { lbbasic[pb] } else { ubbasic[pb] };
                xbasic[pb] = x[jnu];
                lbbasic[pb] = lb[jnu];
                ubbasic[pb] = ub[jnu];
            } else {
                x[jnu] = move_to;
            }
            self.primal_pushes += 1;
            next += 1;
        }
        for p in 0..m {
            x[u(basis.at(p))] = xbasic[p];
        }
        finish_status(info);
    }

    fn push_dual_from_x(&mut self, basis: &mut Basis<'_>, y: &mut [f64], z: &mut [f64], variables: &[Int], x: &[f64], info: &mut Info) {
        let model = basis.model();
        let (lb, ub) = (model.lb(), model.ub());
        let sign_restrict: Vec<i32> = (0..x.len()).map(|j| i32::from(x[j] != ub[j]) | (i32::from(x[j] != lb[j]) << 1)).collect();
        self.push_dual(basis, y, z, variables, &sign_restrict, info);
    }

    pub fn push_dual(&mut self, basis: &mut Basis<'_>, y: &mut [f64], z: &mut [f64], variables: &[Int], sign_restrict: &[i32], info: &mut Info) {
        let model = basis.model();
        let m = u(model.rows());
        let n = u(model.cols());
        let mut btran = IndexedVector::new(m);
        let mut row = IndexedVector::new(n + m);
        let feastol = if model.dualized() {
            self.control.params.pfeasibility_tol
        } else {
            self.control.params.dfeasibility_tol
        };
        self.dual_pushes = 0;
        self.dual_pivots = 0;
        let mut next = 0;
        while next < variables.len() {
            info.errflag = self.control.interrupt_check();
            if info.errflag != 0 {
                break;
            }
            let jb = variables[next];
            if z[u(jb)] == 0.0 {
                next += 1;
                continue;
            }
            basis.tableau_row(jb, &mut btran, &mut row, false);
            let mut step = z[u(jb)];
            let jn = dual_ratio_test(z, &row, sign_restrict, step, feastol);
            if jn >= 0 {
                let pivot = row.elements[u(jn)];
                let mut exchanged = false;
                info.errflag = basis.exchange_if_stable(jb, jn, pivot, 1, &mut exchanged);
                if info.errflag != 0 {
                    let _ = basis.min_singular_value();
                    break;
                }
                if !exchanged {
                    continue;
                }
                self.dual_pivots += 1;
                step = z[u(jn)] / row.elements[u(jn)];
            }
            if step != 0.0 {
                btran.for_each_nonzero(|i, x| y[i] += step * x);
                row.for_each_nonzero(|j, pivot| {
                    z[j] -= step * pivot;
                    if sign_restrict[j] & 1 != 0 {
                        z[j] = std_max(z[j], 0.0);
                    }
                    if sign_restrict[j] & 2 != 0 {
                        z[j] = std_min(z[j], 0.0);
                    }
                });
                z[u(jb)] -= step;
            }
            if jn >= 0 {
                z[u(jn)] = 0.0;
            }
            self.dual_pushes += 1;
            next += 1;
        }
        finish_status(info);
    }
}

fn finish_status(info: &mut Info) {
    if info.errflag == IPX_ERROR_USER_INTERRUPT {
        info.errflag = 0;
        info.status_ipm = IPX_STATUS_USER_INTERRUPT;
    } else if info.errflag == IPX_ERROR_TIME_INTERRUPT {
        info.errflag = 0;
        info.status_crossover = IPX_STATUS_TIME_LIMIT;
    } else if info.errflag != 0 {
        info.status_crossover = IPX_STATUS_FAILED;
    } else {
        info.status_crossover = IPX_STATUS_OPTIMAL;
    }
}

fn primal_ratio_test(xbasic: &[f64], ftran: &IndexedVector, lbbasic: &[f64], ubbasic: &[f64], mut step: f64, feastol: f64, block_at_lb: &mut bool) -> Int {
    let mut pblock: Int = -1;
    *block_at_lb = true;
    ftran.for_each_nonzero(|p, pivot| {
        if pivot.abs() > PIVOT_ZERO_TOL {
            if xbasic[p] + step * pivot < lbbasic[p] - feastol {
                step = (lbbasic[p] - xbasic[p] - feastol) / pivot;
                pblock = p as Int;
                *block_at_lb = true;
            }
            if xbasic[p] + step * pivot > ubbasic[p] + feastol {
                step = (ubbasic[p] - xbasic[p] + feastol) / pivot;
                pblock = p as Int;
                *block_at_lb = false;
            }
        }
    });
    if pblock < 0 {
        return pblock;
    }
    pblock = -1;
    let mut max_pivot = PIVOT_ZERO_TOL;
    ftran.for_each_nonzero(|p, pivot| {
        if pivot.abs() > max_pivot {
            if step * pivot < 0.0 {
                let step_p = (lbbasic[p] - xbasic[p]) / pivot;
                if step_p.abs() <= step.abs() {
                    pblock = p as Int;
                    *block_at_lb = true;
                    max_pivot = pivot.abs();
                }
            }
            if step * pivot > 0.0 {
                let step_p = (ubbasic[p] - xbasic[p]) / pivot;
                if step_p.abs() <= step.abs() {
                    pblock = p as Int;
                    *block_at_lb = false;
                    max_pivot = pivot.abs();
                }
            }
        }
    });
    pblock
}

fn dual_ratio_test(z: &[f64], row: &IndexedVector, sign_restrict: &[i32], mut step: f64, feastol: f64) -> Int {
    let mut jblock: Int = -1;
    row.for_each_nonzero(|j, pivot| {
        if pivot.abs() > PIVOT_ZERO_TOL {
            if sign_restrict[j] & 1 != 0 && z[j] - step * pivot < -feastol {
                step = (z[j] + feastol) / pivot;
                jblock = j as Int;
            }
            if sign_restrict[j] & 2 != 0 && z[j] - step * pivot > feastol {
                step = (z[j] - feastol) / pivot;
                jblock = j as Int;
            }
        }
    });
    if jblock < 0 {
        return jblock;
    }
    jblock = -1;
    let mut max_pivot = PIVOT_ZERO_TOL;
    row.for_each_nonzero(|j, pivot| {
        if pivot.abs() > max_pivot && (z[j] / pivot).abs() <= step.abs() {
            if sign_restrict[j] & 1 != 0 && step * pivot > 0.0 {
                jblock = j as Int;
                max_pivot = pivot.abs();
            }
            if sign_restrict[j] & 2 != 0 && step * pivot < 0.0 {
                jblock = j as Int;
                max_pivot = pivot.abs();
            }
        }
    });
    jblock
}

fn std_max(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

fn std_min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}
