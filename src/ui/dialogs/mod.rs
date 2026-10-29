//! Floating dialogs and viewport-docked tool panels.
//!
//! Dialogs are grouped by workflow while their draw functions remain
//! re-exported here to keep existing call sites concise.

use crate::model::{Axis, ObjectId, PolyVertex};

pub(crate) mod about;
pub(crate) mod charging;
pub(crate) mod confirmations;
pub(crate) mod drill_hole;
pub(crate) mod drill_pattern;
pub(crate) mod editing;
pub(crate) mod files;
pub(crate) mod import_export;
pub(crate) mod object_edit;
pub(crate) mod optimization;
pub(crate) mod optimization_results;
pub(crate) mod plot;
pub(crate) mod point_cloud;
pub(crate) mod products;
pub(crate) mod reference_points;
pub(crate) mod reference_surface;
pub(crate) mod reserve_fields;
pub(crate) mod schedule;
pub(crate) mod sequence_editor;
pub(crate) mod solids;
pub(crate) mod survey;
pub(crate) mod thickness_points;
pub(crate) mod triangulation;
pub(crate) mod video_export;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MoveToAxisDialog {
    pub(crate) object_ids: Vec<ObjectId>,
    pub(crate) axis: Axis,
    pub(crate) value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InsertPointAtElevationDialog {
    pub(crate) object_ids: Vec<ObjectId>,
    pub(crate) elevation: f64,
    /// Lowest and highest vertex Z across `object_ids`; nothing can be
    /// inserted outside that band, so the entry box is bounded to it.
    pub(crate) min_elevation: f64,
    pub(crate) max_elevation: f64,
}

/// Thin Strings on the strings selected when it opened. The App fills the
/// preview when it opens and when the tolerance changes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ThinStringsDialog {
    pub(crate) object_ids: Vec<ObjectId>,
    pub(crate) tolerance: f64,
    /// Above this every string is as thin as it gets: the slider's top.
    pub(crate) slider_max: f64,
    /// Vertices across the strings now, and after Apply.
    pub(crate) before: usize,
    pub(crate) after: usize,
    /// Each string as Apply would leave it, with whether it is closed.
    pub(crate) preview: Vec<(Vec<PolyVertex>, bool)>,
    /// Advanced with every new preview, so the overlay redraws only then.
    pub(crate) preview_generation: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InsertPointAtSpacingDialog {
    pub(crate) object_ids: Vec<ObjectId>,
    pub(crate) spacing: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SmoothStringsDialog {
    pub(crate) object_ids: Vec<ObjectId>,
    pub(crate) tolerance: f64,
}
