//! The blended-stockpile experiment: one scenario contract, one independent
//! replay, and two experimental solvers for it.
//!
//! This module holds everything about *blended* stockpiles that does not
//! depend on which solver is used:
//!
//! | file | what it is |
//! |---|---|
//! | [`input`] | the scenario contract and the stockpile semantics it encodes |
//! | [`grade`] | which reserve fields may be used as a grade, and why most may not |
//! | [`replay`] | an independent physical replay of a published schedule |
//! | [`iterative`] | the iterative fixed-grade HiGHS method |
//!
//! The nonlinear SCIP formulation lives next door in
//! [`super::scip::blend`], because it is the one piece that genuinely needs
//! that backend.
//!
//! # Why the contract is separate from `OptimisationInput`
//!
//! [`input::BlendInput`] is deliberately **not** an extension of the accepted
//! backend's `OptimisationInput`. The two describe different stockpile
//! physics - ordered parcels of fixed composition against a single blended
//! average - so sharing a type would invite comparing objectives that do not
//! measure the same thing. The accepted parcel model remains a separate
//! baseline and is not equivalent to either blended method.
//!
//! # What "valid" means here
//!
//! A schedule from either solver is only usable when [`replay`] passes on it.
//! Replay reconstructs the physical timeline from the published movement rows
//! alone and recomputes every quantity from first principles, including the
//! blended grade by division - the operation the SCIP formulation has to
//! avoid.
//!
//! Note carefully what that does and does not establish. Replay checks
//! **feasibility**: conservation, capacity, release timing, resource limits,
//! authored order and grade eligibility. It cannot establish **optimality**,
//! because a suboptimal schedule is perfectly feasible. The two claims are
//! kept apart everywhere in this module and in the benchmark document.

pub(crate) mod formulation;
pub(crate) mod grade;
pub(crate) mod input;
pub(crate) mod iterative;
pub(crate) mod replay;
