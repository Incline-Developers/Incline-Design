//! Authored haul roads and directional, theoretical truck cycles. Pure model
//! code shared by the desktop scheduler and browser route inspection.
pub(crate) mod network;
pub(crate) mod routing;

pub(crate) use network::{HaulNetwork, HaulRoad, NodeId, NodeRole, RoadId};
