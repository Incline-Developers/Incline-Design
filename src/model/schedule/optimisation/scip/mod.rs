//! Experimental SCIP backend for *blended* stockpile formulations.
//!
//! This module is an investigation, not a second production backend. The
//! accepted scheduling engine remains [`super::highs`]; nothing here is
//! reachable from `Run Schedule`, and the whole module is behind the
//! `scip-experiment` (SCIP 10.0.2, bundled) or `scip-system` (whatever SCIP
//! `$SCIPOPTDIR` points at) cargo feature and a `not(target_arch = "wasm32")`
//! gate.
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
//! equality*, which no linear backend can express. HiGHS cannot take it;
//! `good_lp`'s interface cannot even describe it. SCIP can.
//!
//! # What is and is not claimed here
//!
//! The three experiments in [`experiments`] are deliberately separate, because
//! they answer different questions and their objectives are **not**
//! comparable:
//!
//! - [`experiments::parcel_via_mps`] gives SCIP the *existing* parcel MILP,
//!   exported through MPS. Same mathematics, different backend, so objectives
//!   are directly comparable and any difference is the solver.
//! - [`blend`] builds a one-pile blended model. Its stockpile semantics are
//!   different from the parcel model's, so its objective must never be
//!   compared against a parcel objective as though they measured the same
//!   thing.
//! - Chunking (§8 of the brief) extends [`blend`] with a small number of
//!   ordered blended chunks.
//!
//! Every extracted solution is replayed by [`super::blended::replay`], which reconstructs the
//! physical timeline from published rows alone and never consults SCIP's own
//! constraint-status report. That is not ceremony: on the exact constraint
//! class this module exists to investigate, SCIP was observed returning a
//! *wrong* answer labelled `Optimal` with a matching dual bound. See
//! [`adapter::SolveTuning`] for the reproducer and the mitigation.

pub(crate) mod adapter;
pub(crate) mod blend;
pub(crate) mod experiments;

#[cfg(test)]
pub(crate) mod scenarios;
