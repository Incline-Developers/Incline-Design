//! What a project's coordinate numbers actually mean, and how to move data
//! between systems without lying about it.
//!
//! A triple like `334000, 6251000, 45` locates nothing until you know which
//! grid it is counted from, and two independent choices define that grid: the
//! *projection* that flattened the curved earth onto a plane, and the
//! *reference frame* that projection was measured against. Conflating the two
//! is how a conversion silently returns its own input - the projection maths
//! for GDA94 and GDA2020 is identical, and the two frames are 1.8 m apart.
//!
//! So the two are kept apart here. [`proj4rs`] does projection only: every
//! definition has its `+towgs84` and `+nadgrids` stripped before it is handed
//! over, because those would apply a second, invisible frame change on top of
//! the one this module makes. Frame changes go through [`DATUM_SHIFTS`], a
//! table of published EPSG transformations, chained through intermediate
//! frames when no direct entry exists. **A pair of frames with no path through
//! that table is an error, never an identity**: refusing is recoverable, and a
//! silent 1.8 m is not.
//!
//! Heights pass through untouched. A mine's RL is a height above a vertical
//! datum this module knows nothing about, so a frame change consumes the
//! height to place the point in space but does not rewrite it - which is also
//! what PROJ does when it transforms between two-dimensional systems.

use anyhow::{Result, anyhow, bail, ensure};
use glam::{DMat3, DVec3};

use crate::{i18n::tr, model::survey::MineGridTransform};

/// The earth-shape a reference frame measures against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Ellipsoid {
    /// Semi-major axis, metres.
    a: f64,
    /// Inverse flattening, `1/f`.
    inv_f: f64,
}

impl Ellipsoid {
    fn e2(self) -> f64 {
        let f = 1.0 / self.inv_f;
        2.0 * f - f * f
    }

    /// Geodetic (radians, radians, metres) to earth-centred cartesian.
    fn to_geocentric(self, lon: f64, lat: f64, height: f64) -> DVec3 {
        let e2 = self.e2();
        let prime_vertical = self.a / (1.0 - e2 * lat.sin() * lat.sin()).sqrt();
        DVec3::new(
            (prime_vertical + height) * lat.cos() * lon.cos(),
            (prime_vertical + height) * lat.cos() * lon.sin(),
            (prime_vertical * (1.0 - e2) + height) * lat.sin(),
        )
    }

    /// Earth-centred cartesian back to geodetic, by Bowring's closed form.
    ///
    /// Closed form rather than the usual iteration so the result cannot depend
    /// on a convergence threshold: at terrestrial heights Bowring is good to
    /// well under a millimetre in one pass.
    fn to_geodetic(self, point: DVec3) -> (f64, f64, f64) {
        let e2 = self.e2();
        let b = self.a * (1.0 - 1.0 / self.inv_f);
        let e2_prime = (self.a * self.a - b * b) / (b * b);
        let p = (point.x * point.x + point.y * point.y).sqrt();
        let theta = (point.z * self.a).atan2(p * b);
        let lat = (point.z + e2_prime * b * theta.sin().powi(3)).atan2(p - e2 * self.a * theta.cos().powi(3));
        let lon = point.y.atan2(point.x);
        let prime_vertical = self.a / (1.0 - e2 * lat.sin() * lat.sin()).sqrt();
        (lon, lat, p / lat.cos() - prime_vertical)
    }

    /// The ellipsoid a proj string names, by `+ellps=`, or by an explicit
    /// `+a=` paired with `+rf=` or `+b=`.
    fn from_proj_string(proj: &str) -> Result<Self> {
        /// Every ellipsoid the embedded EPSG definitions actually reference.
        const NAMED: &[(&str, f64, f64)] = &[
            ("GRS80", 6_378_137.0, 298.257_222_101),
            ("WGS84", 6_378_137.0, 298.257_223_563),
            ("krass", 6_378_245.0, 298.3),
            ("intl", 6_378_388.0, 297.0),
            ("WGS72", 6_378_135.0, 298.26),
            ("bessel", 6_377_397.155, 299.152_812_8),
            ("clrk80", 6_378_249.145, 293.465),
            ("clrk66", 6_378_206.4, 294.978_698_213_898_2),
            ("aust_SA", 6_378_160.0, 298.25),
            ("GRS67", 6_378_160.0, 298.247_167_427),
            ("helmert", 6_378_200.0, 298.3),
            ("bess_nam", 6_377_483.865, 299.152_812_8),
            ("evrstSS", 6_377_298.556, 300.801_7),
            ("airy", 6_377_563.396, 299.324_964_6),
            ("WGS66", 6_378_145.0, 298.25),
            ("mod_airy", 6_377_340.189, 299.324_964_6),
            ("NWL9D", 6_378_145.0, 298.25),
            ("GSK2011", 6_378_136.5, 298.256_415_1),
        ];

        let param = |key: &str| proj.split_whitespace().find_map(|token| token.strip_prefix(key)).map(str::to_owned);
        if let Some(name) = param("+ellps=") {
            let (_, a, inv_f) = NAMED
                .iter()
                .find(|(named, ..)| *named == name)
                .ok_or_else(|| anyhow!("{}", tr!("crs-unknown-ellipsoid", name = name.clone())))?;
            return Ok(Self { a: *a, inv_f: *inv_f });
        }
        if let Some(datum) = param("+datum=") {
            // Some definitions name a frame instead of its earth model. The
            // model is still what this function wants, so map across.
            let ellipsoid = match datum.as_str() {
                "WGS84" => "WGS84",
                "NAD83" | "GGRS87" => "GRS80",
                "NAD27" => "clrk66",
                "nzgd49" => "intl",
                "hermannskogel" | "potsdam" => "bessel",
                "carthage" => "clrk80",
                "ire65" => "mod_airy",
                "OSGB36" => "airy",
                _ => bail!("{}", tr!("crs-unknown-ellipsoid", name = datum)),
            };
            let (_, a, inv_f) = NAMED.iter().find(|(named, ..)| *named == ellipsoid).expect("mapped names are in the table");
            return Ok(Self { a: *a, inv_f: *inv_f });
        }
        let a = param("+a=").and_then(|value| value.parse::<f64>().ok());
        let Some(a) = a else {
            bail!("{}", tr!("crs-no-ellipsoid"));
        };
        let inv_f = match (param("+rf="), param("+b=")) {
            (Some(rf), _) => rf.parse::<f64>().ok(),
            (None, Some(b)) => b.parse::<f64>().ok().map(|b| a / (a - b)),
            (None, None) => Some(f64::INFINITY), // a sphere
        };
        let inv_f = inv_f.ok_or_else(|| anyhow!("{}", tr!("crs-no-ellipsoid")))?;
        ensure!(a.is_finite() && a > 0.0, "{}", tr!("crs-no-ellipsoid"));
        Ok(Self { a, inv_f })
    }
}

/// A seven-parameter frame transformation in the EPSG "coordinate frame
/// rotation" convention.
///
/// Translations are metres, rotations arc-seconds, scale parts per million -
/// the units published transformation parameters are quoted in, so a table
/// entry can be read straight off an EPSG datasheet without conversion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Helmert {
    translation: [f64; 3],
    rotation_arcsec: [f64; 3],
    scale_ppm: f64,
}

impl Helmert {
    fn matrix(self) -> DMat3 {
        let [rx, ry, rz] = self.rotation_arcsec.map(|value| (value / 3600.0).to_radians());
        // Coordinate frame convention: the axes rotate, not the point. The
        // position vector convention is this matrix transposed, and getting
        // the two the wrong way round doubles the rotation error rather than
        // cancelling it, so only one convention is admitted here.
        let rows = DMat3::from_cols_array(&[1.0, -rz, ry, rz, 1.0, -rx, -ry, rx, 1.0]);
        rows * (1.0 + self.scale_ppm * 1e-6)
    }

    fn forward(self, point: DVec3) -> DVec3 {
        DVec3::from_array(self.translation) + self.matrix() * point
    }

    fn inverse(self, point: DVec3) -> Result<DVec3> {
        let matrix = self.matrix();
        ensure!(matrix.determinant().abs() > f64::EPSILON, "{}", tr!("crs-transform-failed"));
        Ok(matrix.inverse() * (point - DVec3::from_array(self.translation)))
    }
}

/// One published transformation between two reference frames.
///
/// Frames are identified by their EPSG datum code, which is read out of the
/// definition's WKT rather than guessed from its name.
pub(crate) struct DatumShift {
    from: u16,
    to: u16,
    /// EPSG operation code and name, so a dialog can say which published
    /// transformation it is about to apply rather than just "converting".
    pub(crate) name: &'static str,
    /// The accuracy EPSG states for this operation, in metres.
    pub(crate) accuracy_m: f64,
    /// `None` where the published operation is a null transformation - the two
    /// frames agree to within the stated accuracy. Still a real entry, because
    /// "they agree to within 3 m" is a fact worth showing, and is not the same
    /// claim as "no transformation exists".
    helmert: Option<Helmert>,
}

/// The transformations this build knows.
///
/// Deliberately short and explicit. Anything not reachable through it fails
/// loudly; the alternative - falling back on the `+towgs84` parameters carried
/// by some definitions and not others - is what produces a confident answer
/// that is metres wrong. (Those parameters are also quoted in the opposite
/// rotation convention to the one used here, so taking them at face value
/// doubles the rotation error rather than cancelling it.)
///
/// One entry per pair of frames, which is a simplification EPSG does not make:
/// the older Australian frames have a dozen published operations each, chosen
/// by which state the point is in, and they disagree with one another by a few
/// tenths of a metre. The entries here are the Australia-wide ones, so a
/// result is never worse than its stated accuracy, but a site wanting its
/// state's own operation cannot yet ask for it.
pub(crate) static DATUM_SHIFTS: &[DatumShift] = &[
    DatumShift {
        from: 6283, // GDA94
        to: 1168,   // GDA2020
        name: "EPSG:8048 GDA94 to GDA2020 (1)",
        accuracy_m: 0.01,
        helmert: Some(Helmert {
            translation: [0.06155, -0.01087, -0.04019],
            rotation_arcsec: [-0.0394924, -0.0327221, -0.0328979],
            scale_ppm: -0.009994,
        }),
    },
    DatumShift {
        from: 6203, // AGD84
        to: 6283,   // GDA94
        name: "EPSG:1280 AGD84 to GDA94 (2)",
        accuracy_m: 1.0,
        helmert: Some(Helmert {
            translation: [-117.763, -51.510, 139.061],
            rotation_arcsec: [-0.292, -0.443, -0.277],
            scale_ppm: -0.191,
        }),
    },
    DatumShift {
        from: 6202, // AGD66
        to: 6283,   // GDA94
        name: "EPSG:15979 AGD66 to GDA94 (12)",
        accuracy_m: 3.0,
        helmert: Some(Helmert {
            translation: [-117.808, -51.536, 137.784],
            rotation_arcsec: [-0.303, -0.446, -0.234],
            scale_ppm: -0.29,
        }),
    },
    DatumShift {
        from: 6326, // WGS84
        to: 6283,   // GDA94
        name: "EPSG:1150 GDA94 to WGS 84 (1)",
        accuracy_m: 3.0,
        helmert: None,
    },
];

/// Breadth-first search for a chain of published transformations between two
/// frames, shortest chain first.
///
/// Chaining is how AGD66 reaches GDA2020 without a table entry of its own: the
/// published route is through GDA94, exactly as PROJ would compose it.
fn datum_path(from: u16, to: u16) -> Option<Vec<(&'static DatumShift, bool)>> {
    if from == to {
        return Some(Vec::new());
    }
    let mut frontier = vec![(from, Vec::new())];
    let mut seen = vec![from];
    while !frontier.is_empty() {
        let mut next = Vec::new();
        for (frame, path) in frontier {
            for shift in DATUM_SHIFTS {
                let step = if shift.from == frame {
                    (shift.to, true)
                } else if shift.to == frame {
                    (shift.from, false)
                } else {
                    continue;
                };
                if seen.contains(&step.0) {
                    continue;
                }
                let mut path = path.clone();
                path.push((shift, step.1));
                if step.0 == to {
                    return Some(path);
                }
                seen.push(step.0);
                next.push((step.0, path));
            }
        }
        frontier = next;
    }
    None
}

/// A coordinate system a project's numbers can be counted from.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum CoordinateSystem {
    /// An entry in the embedded EPSG registry, by code.
    Epsg(u16),
    /// A projection definition supplied by the user.
    Custom { name: String, proj: String },
    /// A site grid: its own numbering laid over a real system, which is what a
    /// mine grid is. The parent is what gives it a place on the earth, so a
    /// mine grid can be converted to anything its parent can reach.
    MineGrid {
        name: String,
        parent: Box<CoordinateSystem>,
        transform: MineGridTransform,
    },
}

impl CoordinateSystem {
    /// The system a mine grid is laid over, or the system itself.
    fn base(&self) -> &Self {
        match self {
            Self::MineGrid { parent, .. } => parent.base(),
            other => other,
        }
    }

    /// The grids stacked between this system and its root, outermost first.
    ///
    /// A grid on a grid is ordinary - a pit grid laid over a site grid laid
    /// over a national system - so the chain is walked rather than assuming
    /// one level.
    fn grid_chain(&self) -> Vec<MineGridTransform> {
        let mut chain = Vec::new();
        let mut system = self;
        while let Self::MineGrid { parent, transform, .. } = system {
            chain.push(*transform);
            system = parent;
        }
        chain
    }

    /// Collapse the whole chain into the single transform from the root.
    fn grid_from_base(&self) -> Result<Option<MineGridTransform>> {
        let mut combined: Option<MineGridTransform> = None;
        // Root-ward first, so each step is applied to the one below it.
        for transform in self.grid_chain().into_iter().rev() {
            combined = Some(match combined {
                None => transform,
                Some(previous) => previous.then(transform)?,
            });
        }
        Ok(combined)
    }

    /// The proj definition of the underlying projection, with the frame
    /// parameters stripped: see this module's header.
    fn projection(&self) -> Result<String> {
        let proj = match self.base() {
            Self::Epsg(code) => crs_definitions::from_code(*code)
                .ok_or_else(|| anyhow!("{}", tr!("crs-unknown-code", code = i64::from(*code))))?
                .proj4
                .to_owned(),
            Self::Custom { proj, .. } => proj.clone(),
            Self::MineGrid { .. } => unreachable!("base() resolves mine grids"),
        };
        Ok(proj
            .split_whitespace()
            .filter(|token| !token.starts_with("+towgs84=") && !token.starts_with("+nadgrids=") && !token.starts_with("+geoidgrids="))
            .collect::<Vec<_>>()
            .join(" "))
    }

    /// The EPSG datum code of the frame this system is measured against.
    ///
    /// Read out of the definition's WKT: the datum node carries its own
    /// authority code after the spheroid's. A system whose frame cannot be
    /// identified converts within its own frame and nowhere else.
    fn datum_code(&self) -> Option<u16> {
        let Self::Epsg(code) = self.base() else {
            return None;
        };
        let wkt = crs_definitions::from_code(*code)?.wkt;
        let start = wkt.find("DATUM[")?;
        let mut depth = 0usize;
        let mut end = start;
        for (offset, character) in wkt[start..].char_indices() {
            match character {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + offset;
                        break;
                    }
                }
                _ => {}
            }
        }
        wkt[start..end]
            .rsplit_once("AUTHORITY[\"EPSG\",\"")
            .and_then(|(_, rest)| rest.split('"').next())
            .and_then(|code| code.parse().ok())
    }

    pub(crate) fn label(&self) -> String {
        match self {
            Self::Epsg(code) => registry_name(*code).map_or_else(|| format!("EPSG:{code}"), |name| format!("{name} (EPSG:{code})")),
            Self::Custom { name, .. } | Self::MineGrid { name, .. } => name.clone(),
        }
    }
}

impl CoordinateSystem {
    /// How a system is written into a project file or the config.
    ///
    /// An EPSG code keeps the spelling everyone already quotes, so a project
    /// written here is legible to other software. Anything without a registry
    /// code has no such spelling, so it goes in as JSON behind a marker - a
    /// format we own, rather than a parameter list invented for the occasion.
    pub(crate) fn to_stored(&self) -> String {
        match self {
            Self::Epsg(code) => format!("EPSG:{code}"),
            other => format!("INCLINE:{}", serde_json::to_string(other).unwrap_or_default()),
        }
    }

    /// Read one back, including the free text that projects already carry.
    ///
    /// OMF files in the wild name their system however their author felt like
    /// ("GDA94 / MGA zone 56", "mine grid", nothing at all). Those are not
    /// understood and must not be guessed at: they come back as
    /// [`StoredSystem::Unrecognised`] so the project can show what it was told
    /// without offering to convert anything on the strength of it.
    pub(crate) fn parse_stored(stored: &str) -> StoredSystem {
        let stored = stored.trim();
        if stored.is_empty() {
            return StoredSystem::Unset;
        }
        if let Some(code) = stored.strip_prefix("EPSG:").or_else(|| stored.strip_prefix("epsg:"))
            && let Ok(code) = code.trim().parse::<u16>()
            && crs_definitions::from_code(code).is_some()
        {
            return StoredSystem::Known(Self::Epsg(code));
        }
        if let Some(json) = stored.strip_prefix("INCLINE:")
            && let Ok(system) = serde_json::from_str::<Self>(json)
        {
            return StoredSystem::Known(system);
        }
        if stored.starts_with('+') {
            return StoredSystem::Known(Self::Custom {
                name: stored.to_owned(),
                proj: stored.to_owned(),
            });
        }
        StoredSystem::Unrecognised(stored.to_owned())
    }
}

/// What a project's recorded coordinate system turned out to be.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum StoredSystem {
    /// The project never said.
    Unset,
    Known(CoordinateSystem),
    /// The project said something this build cannot act on - free text from
    /// another package, most often. Worth showing and never worth converting
    /// with.
    Unrecognised(String),
}

/// Every EPSG code in the embedded registry, in order.
///
/// Built once by walking the code range, because the registry exposes a
/// lookup and no listing. The range is the one EPSG allocates coordinate
/// systems from.
pub(crate) fn registry_codes() -> &'static [u16] {
    static CODES: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();
    CODES.get_or_init(|| (2000..=32766u16).filter(|code| crs_definitions::from_code(*code).is_some()).collect())
}

/// Registry entries whose name or code matches every whitespace-separated term
/// in `query`, best (shortest name) first.
///
/// All terms have to match, so "mga 56" narrows to the one zone rather than
/// listing every system mentioning either word, which is how a surveyor
/// actually types a system they already know the name of.
pub(crate) fn search_registry(query: &str, limit: usize) -> Vec<u16> {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut hits: Vec<(usize, u16)> = registry_codes()
        .iter()
        .filter_map(|code| {
            let name = registry_name(*code)?;
            let haystack = format!("{name} epsg:{code}").to_lowercase();
            terms.iter().all(|term| haystack.contains(term)).then_some((name.len(), *code))
        })
        .collect();
    hits.sort_unstable();
    hits.truncate(limit);
    hits.into_iter().map(|(_, code)| code).collect()
}

/// The display name an EPSG definition carries, which is the first quoted
/// string of its WKT.
pub(crate) fn registry_name(code: u16) -> Option<&'static str> {
    let wkt = crs_definitions::from_code(code)?.wkt;
    let (_, rest) = wkt.split_once('"')?;
    rest.split('"').next()
}

/// The projection machinery for a conversion whose ends are actually located
/// somewhere on the earth.
struct Projections {
    source: proj4rs::Proj,
    target: proj4rs::Proj,
    /// The curved-earth system each side's projection unfolds onto. Built once
    /// rather than per point: a conversion runs over whole block models.
    source_geographic: proj4rs::Proj,
    target_geographic: proj4rs::Proj,
    source_ellipsoid: Ellipsoid,
    target_ellipsoid: Ellipsoid,
    source_is_geographic: bool,
    target_is_geographic: bool,
}

/// The curved-earth system belonging to one ellipsoid.
fn geographic(ellipsoid: Ellipsoid) -> Result<proj4rs::Proj> {
    let shape = if ellipsoid.inv_f.is_finite() {
        format!("+a={} +rf={}", ellipsoid.a, ellipsoid.inv_f)
    } else {
        format!("+a={a} +b={a}", a = ellipsoid.a)
    };
    proj4rs::Proj::from_proj_string(&format!("+proj=longlat {shape} +no_defs")).map_err(|error| anyhow!("{error}"))
}

/// A resolved plan for moving coordinates from one system to another.
///
/// Built before any point is touched so a dialog can state what it is about to
/// do - which published transformation, to what accuracy - rather than
/// reporting it afterwards.
pub(crate) struct Conversion {
    /// `None` when nothing curved is involved: two grids on the same
    /// unlocated site grid need no projection, and demanding one would make
    /// the ordinary case - moving between a site's own grids - impossible
    /// until somebody georeferences the site.
    projections: Option<Projections>,
    identical_projection: bool,
    source_grid: Option<MineGridTransform>,
    target_grid: Option<MineGridTransform>,
    frame: Vec<(&'static DatumShift, bool)>,
    /// Worst stated accuracy along the chain, metres. `None` when no frame
    /// change is involved and the conversion is exact.
    pub(crate) accuracy_m: Option<f64>,
    /// The published operations this will apply, in order.
    pub(crate) steps: Vec<String>,
}

impl Conversion {
    pub(crate) fn plan(from: &CoordinateSystem, to: &CoordinateSystem) -> Result<Self> {
        let source_grid = from.grid_from_base()?;
        let target_grid = to.grid_from_base()?;

        let source_proj = from.projection()?;
        let target_proj = to.projection()?;
        let source_ellipsoid = Ellipsoid::from_proj_string(&source_proj)?;
        let target_ellipsoid = Ellipsoid::from_proj_string(&target_proj)?;

        let frame = match (from.datum_code(), to.datum_code()) {
            (Some(a), Some(b)) => datum_path(a, b).ok_or_else(|| {
                anyhow!(
                    "{}",
                    tr!("crs-no-datum-path", from = from.label(), to = to.label(), source = i64::from(a), target = i64::from(b))
                )
            })?,
            // One or both frames unidentified. Same ellipsoid is not proof of
            // the same frame, but it is the only evidence available, and
            // refusing every custom definition would make them useless.
            (a, b) => {
                ensure!(
                    a == b || source_ellipsoid == target_ellipsoid,
                    "{}",
                    tr!("crs-unknown-datum", from = from.label(), to = to.label())
                );
                Vec::new()
            }
        };
        let accuracy_m = frame
            .iter()
            .map(|(shift, _)| shift.accuracy_m)
            .fold(None, |worst: Option<f64>, accuracy| Some(worst.map_or(accuracy, |worst: f64| worst.max(accuracy))));
        let steps = frame.iter().map(|(shift, _)| shift.name.to_owned()).collect();

        Ok(Self {
            projections: Some(Projections {
                source: proj4rs::Proj::from_proj_string(&source_proj).map_err(|error| anyhow!("{error}"))?,
                target: proj4rs::Proj::from_proj_string(&target_proj).map_err(|error| anyhow!("{error}"))?,
                source_geographic: geographic(source_ellipsoid)?,
                target_geographic: geographic(target_ellipsoid)?,
                source_ellipsoid,
                target_ellipsoid,
                source_is_geographic: source_proj.contains("+proj=longlat"),
                target_is_geographic: target_proj.contains("+proj=longlat"),
            }),
            identical_projection: source_proj == target_proj,
            source_grid,
            target_grid,
            frame,
            accuracy_m,
            steps,
        })
    }

    /// Whether this conversion would leave every coordinate untouched, so a
    /// caller can skip the work rather than rewriting data to the same values.
    pub(crate) fn is_identity(&self) -> bool {
        self.identical_projection && self.source_grid.is_none() && self.target_grid.is_none() && self.frame.iter().all(|(shift, _)| shift.helmert.is_none())
    }

    /// Convert one point. Geographic systems are spoken to in degrees here and
    /// converted to the radians proj4rs wants on the way in and out, so a
    /// caller never has to know which unit a given system works in.
    pub(crate) fn point(&self, point: DVec3) -> Result<DVec3> {
        ensure!(point.is_finite(), "{}", tr!("crs-transform-failed"));
        // Returning the input is not an optimisation here, it is the correct
        // answer: a round trip out to geodetic and back moves a coordinate by
        // a fraction of a nanometre, and data that is already in the target
        // system must come back bit-for-bit unchanged.
        if self.is_identity() {
            return Ok(point);
        }

        // Off the source's site grid onto whatever it is laid over.
        let mut current = match self.source_grid {
            Some(grid) => grid.inverse_point(point)?,
            None => point,
        };

        if let Some(projections) = &self.projections {
            if projections.source_is_geographic {
                current = DVec3::new(current.x.to_radians(), current.y.to_radians(), current.z);
            }

            // Onto the curved earth. proj4rs speaks radians either side.
            let mut geodetic = (current.x, current.y, current.z);
            proj4rs::transform::transform(&projections.source, &projections.source_geographic, &mut geodetic).map_err(|error| anyhow!("{error}"))?;

            // The frame change itself, in earth-centred cartesian space. The
            // height goes in, because a frame change is a rotation about the
            // earth's centre and the answer depends on where the point sits,
            // but the original height comes back out: see this module's header.
            //
            // Intermediate frames in a chain need no ellipsoid of their own:
            // cartesian space has no ellipsoid, so only the two ends convert.
            if self.frame.iter().any(|(shift, _)| shift.helmert.is_some()) {
                let height = geodetic.2;
                let mut geocentric = projections.source_ellipsoid.to_geocentric(geodetic.0, geodetic.1, geodetic.2);
                for (shift, forward) in &self.frame {
                    if let Some(helmert) = shift.helmert {
                        geocentric = if *forward { helmert.forward(geocentric) } else { helmert.inverse(geocentric)? };
                    }
                }
                let (lon, lat, _) = projections.target_ellipsoid.to_geodetic(geocentric);
                geodetic = (lon, lat, height);
            }

            // Back down onto the target's plane.
            proj4rs::transform::transform(&projections.target_geographic, &projections.target, &mut geodetic).map_err(|error| anyhow!("{error}"))?;
            current = DVec3::new(geodetic.0, geodetic.1, geodetic.2);

            if projections.target_is_geographic {
                current = DVec3::new(current.x.to_degrees(), current.y.to_degrees(), current.z);
            }
        }

        // And onto the destination's site grid, if it has one.
        let out = match self.target_grid {
            Some(grid) => grid.point(current)?,
            None => current,
        };
        ensure!(out.is_finite(), "{}", tr!("crs-transform-failed"));
        Ok(out)
    }
}

/// How a survey conversion moves coordinates, and what that costs.
///
/// The distinction is not presentational. A flat grid change is affine:
/// straight lines stay straight, parallel lines stay parallel, and every
/// regular structure in a project survives because its *parameters* can be
/// rewritten - a block model stays a regular grid in a turned frame, a raster
/// stays an affine drape. A change of projection or reference frame is not
/// affine. A regular grid in one projection is not a regular grid in another,
/// and a raster's affine world-to-texture map has no counterpart on the far
/// side. Anything made of independent points converts exactly; anything whose
/// shape is implied by a handful of parameters does not, and this type is what
/// stops the second kind being quietly converted as though it were the first.
pub(crate) enum SurveyTransform {
    Grid(MineGridTransform),
    Geodetic(Box<Conversion>),
}

impl SurveyTransform {
    pub(crate) fn point(&self, point: DVec3) -> Result<DVec3> {
        match self {
            Self::Grid(transform) => transform.point(point),
            Self::Geodetic(conversion) => conversion.point(point),
        }
    }

    /// The affine parameters, when there are any.
    ///
    /// `None` is what makes a caller stop and say so rather than reach for an
    /// approximation: there is no rotation and no scale factor that describes
    /// a reprojection, only a different answer at every point.
    pub(crate) fn grid(&self) -> Option<MineGridTransform> {
        match self {
            Self::Grid(transform) => Some(*transform),
            Self::Geodetic(_) => None,
        }
    }

    /// Whether lengths measured along an object survive unchanged.
    ///
    /// A hole drilled 30 m deep is 30 m deep in any coordinate system, so a
    /// reprojection leaves depths and diameters exactly as drilled. A mine
    /// grid carrying a scale factor is the case where they genuinely do
    /// change, because the grid is restating what a metre means.
    pub(crate) fn length_scale(&self) -> f64 {
        self.grid().map_or(1.0, |transform| transform.scale)
    }

    pub(crate) fn accuracy_m(&self) -> Option<f64> {
        match self {
            Self::Grid(_) => None,
            Self::Geodetic(conversion) => conversion.accuracy_m,
        }
    }

    /// The published operations this will apply, for the dialog to show before
    /// anything is touched.
    pub(crate) fn steps(&self) -> &[String] {
        match self {
            Self::Grid(_) => &[],
            Self::Geodetic(conversion) => &conversion.steps,
        }
    }
}

impl Conversion {
    /// Reduce to flat grid parameters where the conversion really is one.
    ///
    /// True when nothing curved is involved: the two systems share a
    /// projection and a reference frame, and only their site grids differ.
    /// That is the common case inside one mine, and it is worth detecting,
    /// because it is the case where block models and rasters can come too.
    pub(crate) fn as_grid(&self) -> Option<MineGridTransform> {
        if !self.identical_projection || self.frame.iter().any(|(shift, _)| shift.helmert.is_some()) {
            return None;
        }
        let source = self.source_grid.unwrap_or_default();
        let target = self.target_grid.unwrap_or_default();
        MineGridTransform::between(source, target).ok()
    }

    /// The conversion expressed the cheapest correct way.
    ///
    /// A conversion that really is a grid change is handed over as one, so
    /// block models and rasters can come along; anything else stays general.
    pub(crate) fn into_transform(self) -> SurveyTransform {
        match self.as_grid() {
            Some(grid) => SurveyTransform::Grid(grid),
            None => SurveyTransform::Geodetic(Box::new(self)),
        }
    }
}
