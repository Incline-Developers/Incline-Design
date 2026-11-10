//! IPX, the interior point solver HiGHS runs for `solver = "ipm"`
//! (Lukas Schork's, in `ipm/ipx`), with BASICLU for its LU factors.

#![allow(dead_code, reason = "ported in stages; parts are wired up as the port proceeds")]

pub(crate) mod basiclu;
pub(crate) mod basis;
pub(crate) mod control;
pub(crate) mod crossover;
mod highs;
pub(crate) mod ipm;
pub(crate) mod iterate;
pub(crate) mod kkt;
pub(crate) mod lp_solver;
pub(crate) mod lu;
pub(crate) mod maxvolume;
pub(crate) mod model;
pub(crate) mod sparse;

pub(crate) use highs::{analytic_centre, solve_lp, solve_reduced_lp};
use sparse::Int;

pub(crate) const IPX_STATUS_NOT_RUN: Int = 0;
pub(crate) const IPX_STATUS_SOLVED: Int = 1000;
pub(crate) const IPX_STATUS_STOPPED: Int = 1005;
pub(crate) const IPX_STATUS_NO_MODEL: Int = 1006;
pub(crate) const IPX_STATUS_OUT_OF_MEMORY: Int = 1003;
pub(crate) const IPX_STATUS_INTERNAL_ERROR: Int = 1004;
pub(crate) const IPX_STATUS_OPTIMAL: Int = 1;
pub(crate) const IPX_STATUS_IMPRECISE: Int = 2;
pub(crate) const IPX_STATUS_PRIMAL_INFEAS: Int = 3;
pub(crate) const IPX_STATUS_DUAL_INFEAS: Int = 4;
pub(crate) const IPX_STATUS_USER_INTERRUPT: Int = 5;
pub(crate) const IPX_STATUS_TIME_LIMIT: Int = 6;
pub(crate) const IPX_STATUS_ITER_LIMIT: Int = 7;
pub(crate) const IPX_STATUS_NO_PROGRESS: Int = 8;
pub(crate) const IPX_STATUS_FAILED: Int = 9;
pub(crate) const IPX_STATUS_DEBUG: Int = 10;

pub(crate) const IPX_ERROR_ARGUMENT_NULL: Int = 102;
pub(crate) const IPX_ERROR_INVALID_DIMENSION: Int = 103;
pub(crate) const IPX_ERROR_INVALID_MATRIX: Int = 104;
pub(crate) const IPX_ERROR_INVALID_VECTOR: Int = 105;
pub(crate) const IPX_ERROR_INVALID_BASIS: Int = 107;
pub(crate) const IPX_ERROR_CR_ITER_LIMIT: Int = 201;
pub(crate) const IPX_ERROR_CR_MATRIX_NOT_POSDEF: Int = 202;
pub(crate) const IPX_ERROR_CR_PRECOND_NOT_POSDEF: Int = 203;
pub(crate) const IPX_ERROR_CR_NO_PROGRESS: Int = 204;
pub(crate) const IPX_ERROR_CR_INF_OR_NAN: Int = 205;
pub(crate) const IPX_ERROR_BASIS_SINGULAR: Int = 301;
pub(crate) const IPX_ERROR_BASIS_ALMOST_SINGULAR: Int = 302;
pub(crate) const IPX_ERROR_BASIS_UPDATE_SINGULAR: Int = 303;
pub(crate) const IPX_ERROR_BASIS_REPAIR_OVERFLOW: Int = 304;
pub(crate) const IPX_ERROR_BASIS_REPAIR_SEARCH: Int = 305;
pub(crate) const IPX_ERROR_BASIS_TOO_ILL_CONDITIONED: Int = 306;
pub(crate) const IPX_ERROR_NOT_IMPLEMENTED: Int = 901;
pub(crate) const IPX_ERROR_USER_INTERRUPT: Int = 998;
pub(crate) const IPX_ERROR_TIME_INTERRUPT: Int = 999;

pub(crate) const IPX_BASIC: Int = 0;
pub(crate) const IPX_NONBASIC: Int = -1;
pub(crate) const IPX_NONBASIC_LB: Int = -1;
pub(crate) const IPX_NONBASIC_UB: Int = -2;
pub(crate) const IPX_SUPERBASIC: Int = -3;

/// IPX's `ipx_info`: what a solve did and found.
#[derive(Clone, Debug, Default)]
pub(crate) struct Info {
    pub status: Int,
    pub status_ipm: Int,
    pub status_crossover: Int,
    pub errflag: Int,
    pub num_var: Int,
    pub num_constr: Int,
    pub num_entries: Int,
    pub num_rows_solver: Int,
    pub num_cols_solver: Int,
    pub num_entries_solver: Int,
    pub dualized: bool,
    pub dense_cols: Int,
    pub centring_tried: bool,
    pub centring_success: bool,
    pub dependent_rows: Int,
    pub dependent_cols: Int,
    pub rows_inconsistent: bool,
    pub cols_inconsistent: bool,
    pub primal_dropped: Int,
    pub dual_dropped: Int,
    pub abs_presidual: f64,
    pub abs_dresidual: f64,
    pub rel_presidual: f64,
    pub rel_dresidual: f64,
    pub pobjval: f64,
    pub dobjval: f64,
    pub rel_objgap: f64,
    pub complementarity: f64,
    pub normx: f64,
    pub normy: f64,
    pub normz: f64,
    pub objval: f64,
    pub primal_infeas: f64,
    pub dual_infeas: f64,
    pub iter: Int,
    pub kktiter1: Int,
    pub kktiter2: Int,
    pub basis_repairs: Int,
    pub updates_start: Int,
    pub updates_ipm: Int,
    pub updates_crossover: Int,
    pub maxvol_updates: Int,
    pub maxvol_skipped: Int,
    pub maxvol_passes: Int,
    pub tbl_nnz: Int,
    pub tbl_max: f64,
    pub frobnorm_squared: f64,
    pub lambdamax: f64,
    pub volume_increase: f64,
}
