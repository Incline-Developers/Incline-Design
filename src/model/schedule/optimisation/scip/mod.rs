//! The SCIP backend for the *blended* stockpile model: the schedule
//! optimiser behind Improve on native builds.
//!
//! Built only with the `scip` feature, and never for wasm: SCIP 10.0.2 built
//! from source (`scip-source`), or whatever SCIP `$SCIPOPTDIR` points at
//! (`scip-system`).
//!
//! # Why a different solver at all
//!
//! The accepted model treats stockpile material as *ordered parcels of fixed
//! composition*: a reclaim draws a known material share off a known position,
//! so every relationship in the model stays linear. A *blended* pile has no
//! such positions. Its reclaimed composition is the pile's own average at the
//! moment of the draw, which is a ratio of two decision variables:
//!
//! ```text
//! reclaimed contained / reclaimed tonnes = opening contained / opening tonnes
//! ```
//!
//! Cleared of the division (see [`blend`]) that is a *nonconvex bilinear
//! equality*, which no linear backend can express. SCIP can.
//!
//! Every extracted solution is replayed by [`super::blended::replay`], which
//! reconstructs the physical timeline from published rows alone and never
//! consults SCIP's own constraint-status report. On this constraint class
//! SCIP was observed returning a *wrong* answer labelled `Optimal` with a
//! matching dual bound; `offer_seed` in `app/scip_blend.rs` has the
//! reproducer and the mitigation.

pub(crate) mod adapter;
pub(crate) mod blend;
pub(crate) mod experiments;
pub(crate) mod plan;
