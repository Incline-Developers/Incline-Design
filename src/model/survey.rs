//! Local mine-grid similarity transformation, independent of QGIS.
use glam::{DMat3, DVec3};

use crate::model::Object;

/// Target = target_origin + scale * Rz(angle) * (source - source_origin).
/// A positive angle is counterclockwise when viewed from above (+Z).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct MineGridTransform {
    pub(crate) source_origin: DVec3,
    pub(crate) target_origin: DVec3,
    pub(crate) angle_degrees: f64,
    /// Uniform XYZ scale; positive so handedness, arcs and winding survive.
    pub(crate) scale: f64,
}

impl Default for MineGridTransform {
    fn default() -> Self {
        Self {
            source_origin: DVec3::ZERO,
            target_origin: DVec3::ZERO,
            angle_degrees: 0.0,
            scale: 1.0,
        }
    }
}

impl MineGridTransform {
    pub(crate) fn validate(self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.source_origin.is_finite() && self.target_origin.is_finite() && self.angle_degrees.is_finite(),
            "{}",
            crate::i18n::tr!("survey-invalid-transform")
        );
        anyhow::ensure!(
            self.scale.is_finite() && self.scale > 0.0 && (1.0 / self.scale).is_finite(),
            "{}",
            crate::i18n::tr!("survey-invalid-scale")
        );
        Ok(())
    }

    pub(crate) fn rotation(self) -> DMat3 {
        DMat3::from_rotation_z((self.angle_degrees % 360.0).to_radians())
    }

    pub(crate) fn point(self, point: DVec3) -> anyhow::Result<DVec3> {
        let out = self.target_origin + self.rotation() * ((point - self.source_origin) * self.scale);
        anyhow::ensure!(point.is_finite() && out.is_finite(), "{}", crate::i18n::tr!("survey-invalid-transform"));
        Ok(out)
    }

    /// Back off this system onto the frame it is defined against.
    ///
    /// The forward direction is a rotation and scale about the system's
    /// origin, so the way back is the same one undone rather than a second
    /// definition: a site grid and its parent must round-trip exactly, or
    /// converting there and back would walk data off its own coordinates.
    pub(crate) fn inverse_point(self, point: DVec3) -> anyhow::Result<DVec3> {
        self.validate()?;
        let out = self.source_origin + self.rotation().transpose() * (point - self.target_origin) / self.scale;
        anyhow::ensure!(point.is_finite() && out.is_finite(), "{}", crate::i18n::tr!("survey-invalid-transform"));
        Ok(out)
    }

    /// This transform followed by `next`, as one transform.
    ///
    /// Grids stack: a pit grid laid over a site grid laid over a national
    /// system is three frames and two transforms, and collapsing them here
    /// means the conversion itself only ever applies one.
    pub(crate) fn then(self, next: Self) -> anyhow::Result<Self> {
        self.validate()?;
        next.validate()?;
        let combined = Self {
            source_origin: self.source_origin,
            target_origin: next.point(self.target_origin)?,
            angle_degrees: self.angle_degrees % 360.0 + next.angle_degrees % 360.0,
            scale: self.scale * next.scale,
        };
        combined.validate()?;
        Ok(combined)
    }

    /// Compose source-system -> reference -> destination-system. Both saved
    /// definitions map the same reference frame into their respective systems.
    pub(crate) fn between(source: Self, destination: Self) -> anyhow::Result<Self> {
        source.validate()?;
        destination.validate()?;
        let result = Self {
            source_origin: source.target_origin,
            target_origin: destination.point(source.source_origin)?,
            angle_degrees: destination.angle_degrees % 360.0 - source.angle_degrees % 360.0,
            scale: destination.scale / source.scale,
        };
        result.validate()?;
        Ok(result)
    }
}

/// Move one design object through a survey conversion.
///
/// Text is the only object carrying more than positions. Under a grid change
/// its rotation and height are restated along with everything else; under a
/// reprojection they are left alone, because there is no single angle a
/// reprojection turns things by - the amount varies across the sheet - and
/// leaving a label the size and attitude its author chose is the lesser error.
pub(crate) fn transform_object(transform: &crate::model::crs::SurveyTransform, object: &mut Object, cancel: &crate::app::jobs::CancelFlag) -> anyhow::Result<()> {
    match object {
        Object::Point { pos, .. } => *pos = transform.point(*pos)?,
        // A reprojection does not preserve circles - the radius is direction
        // dependent once the sheet distorts - but restating the centre and
        // keeping the radius is the same compromise text rotation makes, and
        // far better than silently shredding the shape into a polygon.
        Object::Circle { center, radius, .. } => {
            *center = transform.point(*center)?;
            if let Some(grid) = transform.grid() {
                *radius *= grid.scale;
            }
        }
        Object::Polyline { verts, .. } => {
            for (index, vertex) in verts.iter_mut().enumerate() {
                if index % 4096 == 0 {
                    anyhow::ensure!(!cancel.is_cancelled(), "Cancelled");
                }
                vertex.pos = transform.point(vertex.pos)?;
            }
        }
        Object::Text { pos, rotation, height, .. } => {
            *pos = transform.point(*pos)?;
            if let Some(grid) = transform.grid() {
                *rotation += (grid.angle_degrees % 360.0).to_radians();
                *height *= grid.scale;
            }
        }
    }
    object.validate_geometry().map_err(anyhow::Error::msg)
}

/// What the chosen mine coordinate system calls its three axes.
///
/// A global, the way the interface language is: every coordinate readout in
/// the app wants these, almost nothing changes them, and threading three
/// strings through every widget signature to serve a site that writes "RL"
/// instead of "Z" would be noise in each one. Set whenever the mine
/// coordinate system changes - see [`set_axis_names`].
static AXIS_NAMES: std::sync::RwLock<Option<[String; 3]>> = std::sync::RwLock::new(None);

/// Install the axis names, or clear them back to X/Y/Z with `None`.
pub(crate) fn set_axis_names(names: Option<[String; 3]>) {
    // A blank name is not a name: a half-filled set falls back per axis rather
    // than leaving a readout labelled with nothing.
    if let Ok(mut current) = AXIS_NAMES.write() {
        *current = names;
    }
}

/// The name of axis `index` (0 = X), or its letter when the mine coordinate
/// system has not renamed it.
pub(crate) fn axis_name(index: usize) -> String {
    let fallback = || ["X", "Y", "Z"].get(index).copied().unwrap_or_default().to_owned();
    let Ok(names) = AXIS_NAMES.read() else {
        return fallback();
    };
    names
        .as_ref()
        .and_then(|names| names.get(index))
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(fallback)
}

/// The three names together, for a readout that shows all of them.
pub(crate) fn axis_names() -> [String; 3] {
    [axis_name(0), axis_name(1), axis_name(2)]
}

/// The name of axis `index` cut to fit somewhere that has room for two
/// characters - the orientation gizmo's arms, the Z field's prefix.
///
/// Cut rather than scaled down: a gizmo arm is a 16-pixel disc, and a name
/// shrunk to fit inside one is unreadable where the first two letters of it
/// are not. "UPWARDS" reads as "UP", which is what the site would have
/// abbreviated it to anyway.
pub(crate) fn axis_abbreviation(index: usize) -> String {
    axis_name(index).chars().take(2).collect()
}

/// The kinds a selection holds, as "30 designs, 1 triangulation".
///
/// Counted per kind rather than as one total, because "12 designs, 0
/// triangulations" says immediately that a selection did not carry what its
/// owner thought it did, where "12 items" does not. Kinds nobody selected are
/// left out entirely: the one number a reader is checking is harder to find
/// with five zeroes sat beside it.
pub(crate) fn describe_counts(counts: SelectionCounts) -> String {
    use crate::i18n::tr;
    let mut parts: Vec<String> = Vec::new();
    for (index, count) in counts.into_iter().enumerate() {
        if count == 0 {
            continue;
        }
        let count = count as i64;
        parts.push(match index {
            0 => tr!("survey-count-designs", count = count),
            1 => tr!("survey-count-meshes", count = count),
            2 => tr!("survey-count-models", count = count),
            3 => tr!("survey-count-clouds", count = count),
            4 => tr!("survey-count-holes", count = count),
            _ => tr!("survey-count-rasters", count = count),
        });
    }
    parts.join(", ")
}

/// Designs, triangulations, block models, point clouds, drillhole datasets and
/// rasters, in that order.
pub(crate) type SelectionCounts = [usize; 6];

/// One saved coordinate system, as the config holds it.
///
/// A grid names its parent rather than embedding it, so a list of these is
/// flat and a rename only has to be written in one place. Resolving a name to
/// a [`CoordinateSystem`] is [`resolve_system`], which is also where a missing
/// or circular parent is caught.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct SystemDefinition {
    pub(crate) name: String,
    pub(crate) kind: SystemKind,
    /// What this system calls its axes, if not X, Y and Z. A mine writing
    /// easting, northing and reduced level puts "E", "N", "RL" here and every
    /// coordinate readout in the app follows - but only while this system is
    /// the chosen one, because they are this system's names and not the
    /// application's.
    #[serde(default)]
    pub(crate) axis_names: Option<[String; 3]>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum SystemKind {
    /// An entry in the coordinate system registry.
    Registry { code: u16 },
    /// A grid defined by its relationship to another saved system.
    Grid { parent: String, transform: MineGridTransform },
}

impl SystemDefinition {
    /// A one-line description of what the system is, for a list row.
    pub(crate) fn summary(&self) -> String {
        match &self.kind {
            SystemKind::Registry { code } => crate::model::crs::registry_name(*code).map_or_else(|| format!("EPSG:{code}"), |name| format!("{name} (EPSG:{code})")),
            SystemKind::Grid { parent, .. } => crate::i18n::tr!("survey-kind-grid", parent = parent.clone()),
        }
    }
}

/// Build the coordinate system `name` refers to, following its parents down.
///
/// Depth is bounded by the number of definitions, which is what catches a
/// grid made its own ancestor: a cycle would otherwise resolve for ever, and
/// the dialog lets a parent be chosen freely enough that one is easy to make.
pub(crate) fn resolve_system(name: &str, definitions: &[SystemDefinition]) -> anyhow::Result<crate::model::crs::CoordinateSystem> {
    fn resolve(name: &str, definitions: &[SystemDefinition], depth: usize) -> anyhow::Result<crate::model::crs::CoordinateSystem> {
        use crate::model::crs::CoordinateSystem;
        anyhow::ensure!(depth <= definitions.len(), "{}", crate::i18n::tr!("survey-system-cycle", name = name.to_owned()));
        let definition = definitions
            .iter()
            .find(|definition| definition.name == name)
            .ok_or_else(|| anyhow::anyhow!("{}", crate::i18n::tr!("survey-system-missing")))?;
        Ok(match &definition.kind {
            SystemKind::Registry { code } => CoordinateSystem::Epsg(*code),
            SystemKind::Grid { parent, transform } => CoordinateSystem::MineGrid {
                name: definition.name.clone(),
                parent: Box::new(resolve(parent, definitions, depth + 1)?),
                transform: *transform,
            },
        })
    }
    resolve(name, definitions, 0)
}
