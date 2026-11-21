//! The order panels are mined in.
//!
//! Pushback `p` (0-based) mines bench `j` in round `j + p * lead`; within a
//! round the outer pushback goes first. So the leading pushback is never more
//! than `lead` benches ahead of the one behind it, and the order is one line
//! (Micromine's constant lag): lead 3 gives PB1 b1, b2, b3, PB2 b1, PB1 b4,
//! PB2 b2, ... A lead of 0 mines the pit bench by bench (the worst case); with
//! no lead each pushback is finished before the next (the best case).

use super::panels::Panels;

/// One step of a sequence: pushback `pushback`'s panel number `panel` (an
/// index into its [`Panels`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Step {
    pub(crate) pushback: usize,
    pub(crate) panel: usize,
}

/// The order of `pushbacks`' panels, innermost pushback first, with
/// `lead` benches between them, or one after another for `None`.
pub(crate) fn order(pushbacks: &[&Panels], lead: Option<u32>) -> Vec<Step> {
    let mut steps: Vec<(u64, std::cmp::Reverse<usize>, u32, Step)> = Vec::new();
    for (pushback, panels) in pushbacks.iter().enumerate() {
        for (panel, (bench, _)) in panels.iter().enumerate() {
            let round = match lead {
                Some(lead) => u64::from(*bench) + pushback as u64 * u64::from(lead),
                // Pushback by pushback: the pushback outranks the bench.
                None => ((pushback as u64) << 32) | u64::from(*bench),
            };
            steps.push((round, std::cmp::Reverse(pushback), *bench, Step { pushback, panel }));
        }
    }
    steps.sort_by_key(|(round, outer, bench, _)| (*round, *outer, *bench));
    steps.into_iter().map(|(.., step)| step).collect()
}
