//! A random forest trained offline, evaluated here.
//!
//! The model is a flat little-endian binary written by
//! `tools/point_forest/train.py`:
//!
//! ```text
//! "IRF1"  u8 feature count  u8 class count  u16 tree count
//! per tree: u16 node count  u16 leaf count
//!           nodes: u8 feature (255 = leaf)  u16 left  u16 right  f32 threshold
//!           leaves: class-count u8 class probabilities, scaled to 255
//! ```
//!
//! A split sends `feature <= threshold` left. A leaf node's `left` indexes its
//! row of leaves. Thresholds are rounded down to f32 at export, so the split
//! falls exactly where training put it for f32 features.

use anyhow::{Result, bail, ensure};

const LEAF: u8 = u8::MAX;
/// Most classes a model may have, which sizes the vote tally.
const MAX_CLASSES: usize = 8;

#[derive(Clone, Copy)]
struct Node {
    feature: u8,
    left: u16,
    right: u16,
    threshold: f32,
}

struct Tree {
    nodes: Vec<Node>,
    leaves: Vec<u8>,
}

pub(crate) struct Forest {
    feature_count: usize,
    class_count: usize,
    trees: Vec<Tree>,
}

impl Forest {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self> {
        let mut cursor = bytes;
        let mut take = |count: usize| -> Result<&[u8]> {
            ensure!(cursor.len() >= count, "The classifier model is truncated");
            let (head, tail) = cursor.split_at(count);
            cursor = tail;
            Ok(head)
        };
        if take(4)? != b"IRF1" {
            bail!("The classifier model has an unknown format");
        }
        let header = take(4)?;
        let (feature_count, class_count) = (usize::from(header[0]), usize::from(header[1]));
        let tree_count = u16::from_le_bytes([header[2], header[3]]);
        ensure!((1..=MAX_CLASSES).contains(&class_count), "The classifier model has {class_count} classes");
        let mut trees = Vec::with_capacity(usize::from(tree_count));
        for _ in 0..tree_count {
            let counts = take(4)?;
            let node_count = usize::from(u16::from_le_bytes([counts[0], counts[1]]));
            let leaf_count = usize::from(u16::from_le_bytes([counts[2], counts[3]]));
            let nodes: Vec<Node> = take(node_count * 9)?
                .as_chunks::<9>()
                .0
                .iter()
                .map(|node| Node {
                    feature: node[0],
                    left: u16::from_le_bytes([node[1], node[2]]),
                    right: u16::from_le_bytes([node[3], node[4]]),
                    threshold: f32::from_le_bytes([node[5], node[6], node[7], node[8]]),
                })
                .collect();
            let leaves = take(leaf_count * class_count)?.to_vec();
            for node in &nodes {
                if node.feature == LEAF {
                    ensure!(usize::from(node.left) < leaf_count, "The classifier model has a leaf out of range");
                } else {
                    ensure!(usize::from(node.feature) < feature_count, "The classifier model splits on a feature it does not have");
                    ensure!(
                        usize::from(node.left) < node_count && usize::from(node.right) < node_count,
                        "The classifier model has a branch out of range"
                    );
                }
            }
            ensure!(!nodes.is_empty(), "The classifier model has an empty tree");
            trees.push(Tree { nodes, leaves });
        }
        ensure!(cursor.is_empty(), "The classifier model has trailing data");
        Ok(Self {
            feature_count,
            class_count,
            trees,
        })
    }

    pub(crate) fn feature_count(&self) -> usize {
        self.feature_count
    }

    /// The class most trees' leaves vote for, weighting each by its
    /// probability.
    pub(crate) fn predict(&self, features: &[f32]) -> usize {
        debug_assert_eq!(features.len(), self.feature_count);
        let mut votes = [0u32; MAX_CLASSES];
        let votes = &mut votes[..self.class_count];
        for tree in &self.trees {
            let mut node = tree.nodes[0];
            // A malformed tree could cycle; a valid one is never deeper than it
            // has nodes.
            for _ in 0..tree.nodes.len() {
                if node.feature == LEAF {
                    break;
                }
                let next = if features[usize::from(node.feature)] <= node.threshold { node.left } else { node.right };
                node = tree.nodes[usize::from(next)];
            }
            if node.feature == LEAF {
                let start = usize::from(node.left) * self.class_count;
                for (vote, &probability) in votes.iter_mut().zip(&tree.leaves[start..start + self.class_count]) {
                    *vote += u32::from(probability);
                }
            }
        }
        votes
            .iter()
            .enumerate()
            .max_by_key(|(class, vote)| (**vote, std::cmp::Reverse(*class)))
            .map_or(0, |(class, _)| class)
    }
}
