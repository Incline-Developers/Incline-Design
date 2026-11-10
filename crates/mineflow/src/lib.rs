//! Pure Rust port of [MineFlow](https://github.com/MineFlowCSM/MineFlow).
//!
//! Maximizes the sum of economic block values subject to mining precedence.
//! An edge `(a, b)` means that mining block `a` requires mining block `b`.
//! Uses MineFlow's highest-label pseudoflow algorithm, with lazy precedence
//! generation for regular grids and exact i128 intermediate arithmetic.
//!
//! A [`Pattern`] turns a [`BlockModel`] and [`Slope`] into implicit
//! [`Precedence`] for a [`Solver`]. Economic values are inputs: costs, prices
//! and cutoffs belong to the caller. [`ValueScale`] explicitly quantizes
//! floating point values. Block indices increase in x, then y, then z; z
//! increases upward. Slope azimuths are clockwise from north.
//!
//! # Differences from upstream
//!
//! - Input values are i64; excess, flow and `SolveInfo::pit_value` are i128,
//!   which covers sums of any i64 block values exactly without GMP.
//! - The flow forest keeps 32 bit node links, so a solver addresses at most
//!   4,294,967,294 blocks; `Solver::solve` reports a larger model as invalid.
//! - Slope azimuths are normalized before lookup, duplicates are rejected,
//!   slopes lie in (0, 90] degrees, and cubic interpolation that overshoots
//!   that range is an error.
//! - Largest-cut selection uses residual reachability, contracting only tree
//!   arcs with positive flow. Upstream groups by whole flow-tree branch, which
//!   drops break-even blocks joined by zero-flow arcs: for values `[0, 0, -2]`
//!   and edges `[(0, 1), (0, 2)]` the largest optimal pit is
//!   `[false, true, false]`, where upstream returns all false.
//! - Accuracy field names keep upstream's convention: `false_positive` counts
//!   cone blocks the pattern misses, `false_negative` counts extras.
//!
//! The 32 bit links, cached parent index, precomputed offset deltas,
//! branch-free interior path and backwards walk in `Forest::largest` exist for
//! speed: 20-34% faster than the C++ in 29-41% less memory on the upstream
//! datasets. Smallest cuts and objective values match upstream exactly on 500
//! random graphs, 240 generated patterns and the five upstream datasets; the
//! largest-cut difference was checked by brute force.
//!
//! Ported from [MineFlowCSM/MineFlow](https://github.com/MineFlowCSM/MineFlow)
//! at `6f5dd7091070b1bd85457b3ce9fd95c4e345c983` and
//! [Incline-Developers/MineFlow-rs](https://github.com/Incline-Developers/MineFlow-rs)
//! at `3a6a3b541aa55a50eebd5efadea4943c82353a08`. Deutsch, M., Dağdelen, K. &
//! Johnson, T. (2022), [An Open-Source Program for Efficiently Computing
//! Ultimate Pit Limits: MineFlow](https://doi.org/10.1007/s11053-022-10035-w).
#![forbid(unsafe_code)]

mod error;
mod geometry;
mod pattern;
mod precedence;
mod pseudoflow;
mod slope;
mod solver;
mod source;
mod values;

pub use error::{Error, Result};
pub use geometry::{Angle, BlockModel, degrees, radians};
pub use pattern::{Pattern, PatternAccuracy};
pub use precedence::{Precedence, SearchBuffer};
pub use slope::Slope;
pub use solver::{Pit, SolveInfo, Solver};
pub use source::PrecedenceSource;
pub use values::ValueScale;
