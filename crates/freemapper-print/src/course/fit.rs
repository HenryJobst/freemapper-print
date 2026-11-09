//! Course fitting: puts the points of a course file into the map's coordinates (ADR 0032).
//!
//! A course file gives map positions in mm from the map's origin, but writers disagree on the y axis:
//! Purple Pen, Condes and OCAD write y up, the IOF standard and OOM y down. Both readings are tried, and one
//! fits when every control lies within the box of the map's objects grown by [`MARGIN_MM`]. When the map
//! is georeferenced with a UTM CRS and the file gives latitude and longitude, they decide between the two
//! readings and check the chosen one: a control more than [`GEOREF_TOLERANCE_M`] from where its latitude
//! and longitude put it is an error, never a silent shift. Controls with only latitude and longitude are
//! placed through the map's georeferencing.

use crate::omap::coord::MapCoord;
use crate::omap::{MapDocument, MapGeoref};

use super::model::{CourseEvent, FittedEvent, Offset, RawEvent, RawPoint};
use super::tm::{TransverseMercator, utm_from_crs};

/// How far beyond the box of the map's objects a control may lie, in map mm.
pub const MARGIN_MM: f64 = 10.0;
/// The largest distance in metres between a control's map position and its latitude and longitude.
pub const GEOREF_TOLERANCE_M: f64 = 20.0;

/// A fitted event and what the fit found out.
#[derive(Debug, Clone, PartialEq)]
pub struct Fit {
    pub event: FittedEvent,
    pub notes: Vec<FitNote>,
}

/// What the fit found out, for `check` to report.
#[derive(Debug, Clone, PartialEq)]
pub enum FitNote {
    /// The file writes y up; the fit flipped it.
    YUpDetected,
    /// `controls` controls have both a map position and latitude and longitude; the largest distance
    /// between the two is `metres`.
    GeorefChecked { controls: usize, metres: f64 },
    /// No check against latitude and longitude, and why.
    GeorefSkipped(SkipReason),
    /// These controls have only latitude and longitude; the map's georeferencing placed them.
    PlacedByGeoref(Vec<String>),
}

/// Why latitude and longitude could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// The map has no CRS or no projected reference point.
    NotGeoreferenced,
    /// The map's CRS is not a UTM projection; the CRS text.
    UnsupportedCrs(String),
    /// No control with a map position has latitude and longitude.
    NoLatLng,
}

impl std::fmt::Display for SkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotGeoreferenced => f.write_str("the map is not georeferenced"),
            Self::UnsupportedCrs(spec) => write!(
                f,
                "the map's coordinate system \"{spec}\" is not UTM, the only one supported"
            ),
            Self::NoLatLng => f.write_str("no control has both a map position and lat/lng"),
        }
    }
}

/// Why a course file does not fit the map.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FitError {
    #[error(
        "only {inside} of {total} controls lie on the map, with y up or y down; the course file belongs to another map or uses another origin"
    )]
    OutsideMap { inside: usize, total: usize },
    #[error(
        "control {control} lies {metres:.0} m from where its lat/lng put it (20 m allowed); the course file does not match the map's georeferencing"
    )]
    GeorefMismatch { control: String, metres: f64 },
    #[error("controls {} have only lat/lng and cannot be placed: {reason}", controls.join(", "))]
    NoPositionNoGeoref {
        controls: Vec<String>,
        reason: SkipReason,
    },
}

/// A control with a map position: code, file point and lat/lng if any.
type Positioned<'a> = (&'a str, RawPoint, Option<(f64, f64)>);

/// The two readings of a file's y axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    YUp,
    YDown,
}

impl Axis {
    /// The map position (µm, y down) of a file point, unrounded.
    fn place(self, p: RawPoint) -> (f64, f64) {
        let y = match self {
            Axis::YUp => -p.y_mm,
            Axis::YDown => p.y_mm,
        };
        (p.x_mm * 1000.0, y * 1000.0)
    }

    /// The linear part only: offsets are not moved.
    fn turn(self, o: Offset) -> Offset {
        match self {
            Axis::YUp => Offset {
                dx_mm: o.dx_mm,
                dy_mm: -o.dy_mm,
            },
            Axis::YDown => o,
        }
    }
}

/// OOM's map-to-grid transform (`Georeferencing::updateTransformation`): map mm less the map reference
/// point, scaled to metres with y flipped, turned by the grivation, plus the projected reference point.
struct Georef {
    tm: TransverseMercator,
    ref_grid: (f64, f64),
    map_ref_um: (f64, f64),
    metres_per_um: f64,
    grivation_rad: f64,
}

impl Georef {
    fn of(g: &MapGeoref) -> Result<Self, SkipReason> {
        let (Some(spec), Some(ref_grid)) = (&g.crs_spec, g.ref_projected) else {
            return Err(SkipReason::NotGeoreferenced);
        };
        if g.scale_denominator == 0 {
            return Err(SkipReason::NotGeoreferenced);
        }
        let tm = utm_from_crs(spec).ok_or_else(|| SkipReason::UnsupportedCrs(spec.clone()))?;
        Ok(Self {
            tm,
            ref_grid,
            map_ref_um: (g.map_ref_mm.0 * 1000.0, g.map_ref_mm.1 * 1000.0),
            metres_per_um: f64::from(g.scale_denominator) * g.combined_scale_factor / 1e6,
            grivation_rad: g.grivation_deg.to_radians(),
        })
    }

    /// Grid easting and northing of a map position (µm, y down).
    fn to_grid(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let qx = (x - self.map_ref_um.0) * self.metres_per_um;
        let qy = -(y - self.map_ref_um.1) * self.metres_per_um;
        let (s, c) = self.grivation_rad.sin_cos();
        (
            self.ref_grid.0 + qx * c + qy * s,
            self.ref_grid.1 - qx * s + qy * c,
        )
    }

    /// The map position (µm, y down) of a grid point: the inverse of [`Self::to_grid`].
    fn to_map(&self, (e, n): (f64, f64)) -> (f64, f64) {
        let (de, dn) = (e - self.ref_grid.0, n - self.ref_grid.1);
        let (s, c) = self.grivation_rad.sin_cos();
        let qx = de * c - dn * s;
        let qy = de * s + dn * c;
        (
            qx / self.metres_per_um + self.map_ref_um.0,
            -qy / self.metres_per_um + self.map_ref_um.1,
        )
    }

    fn grid_of(&self, (lat, lng): (f64, f64)) -> (f64, f64) {
        self.tm.forward(lat, lng)
    }

    /// Metres between a map position and the grid point of `geo`; infinite when either is not finite.
    fn distance(&self, map_um: (f64, f64), geo: (f64, f64)) -> f64 {
        let a = self.to_grid(map_um);
        let b = self.grid_of(geo);
        let d = (a.0 - b.0).hypot(a.1 - b.1);
        if d.is_finite() { d } else { f64::INFINITY }
    }
}

/// The box around every object coordinate of the map (µm), grown by [`MARGIN_MM`].
fn map_box(map: &MapDocument) -> Option<((f64, f64), (f64, f64))> {
    let mut coords = map
        .parts
        .iter()
        .flat_map(|p| &p.objects)
        .flat_map(|o| &o.coords);
    let first = coords.next()?;
    let (min, max) = coords.fold(((first.x, first.y), (first.x, first.y)), |(lo, hi), c| {
        (
            (lo.0.min(c.x), lo.1.min(c.y)),
            (hi.0.max(c.x), hi.1.max(c.y)),
        )
    });
    let m = MARGIN_MM * 1000.0;
    Some((
        (f64::from(min.0) - m, f64::from(min.1) - m),
        (f64::from(max.0) + m, f64::from(max.1) + m),
    ))
}

/// Rounds a map position known to lie on the map; `as` saturates for anything else.
fn coord((x, y): (f64, f64)) -> MapCoord {
    MapCoord::new(x.round() as i32, y.round() as i32)
}

/// Puts the event's points into map coordinates (µm, y down). See the module documentation.
pub fn fit(event: RawEvent, map: &MapDocument) -> Result<Fit, FitError> {
    let georef = Georef::of(&map.georef);
    let bounds = map_box(map);
    let inside = |(x, y): (f64, f64)| {
        bounds.is_some_and(|(lo, hi)| x >= lo.0 && x <= hi.0 && y >= lo.1 && y <= hi.1)
    };

    // Controls with only lat/lng, placed through the georeferencing.
    let geo_only: Vec<(usize, &str, (f64, f64))> = event
        .controls
        .iter()
        .enumerate()
        .filter(|(_, c)| c.position.is_none())
        .filter_map(|(i, c)| c.geo.map(|g| (i, c.code.as_str(), g)))
        .collect();
    let placed: Vec<(usize, (f64, f64))> = match &georef {
        Ok(g) => geo_only
            .iter()
            .map(|&(i, _, geo)| (i, g.to_map(g.grid_of(geo))))
            .collect(),
        Err(reason) if !geo_only.is_empty() => {
            return Err(FitError::NoPositionNoGeoref {
                controls: geo_only.iter().map(|&(_, c, _)| c.to_owned()).collect(),
                reason: reason.clone(),
            });
        }
        Err(_) => Vec::new(),
    };
    let placed_inside = placed.iter().filter(|(_, p)| inside(*p)).count();

    let positioned: Vec<Positioned> = event
        .controls
        .iter()
        .filter_map(|c| c.position.map(|p| (c.code.as_str(), p, c.geo)))
        .collect();
    let total = positioned.len() + placed.len();
    let count_inside = |axis: Axis| {
        placed_inside
            + positioned
                .iter()
                .filter(|(_, p, _)| inside(axis.place(*p)))
                .count()
    };
    // Y up first: rule 4 prefers it when both fit.
    let fitting: Vec<Axis> = [Axis::YUp, Axis::YDown]
        .into_iter()
        .filter(|&a| count_inside(a) == total)
        .collect();
    if fitting.is_empty() {
        return Err(FitError::OutsideMap {
            inside: count_inside(Axis::YUp).max(count_inside(Axis::YDown)),
            total,
        });
    }

    let mut notes = Vec::new();
    let checks: Vec<(&str, RawPoint, (f64, f64))> = positioned
        .iter()
        .filter_map(|&(code, p, geo)| geo.map(|g| (code, p, g)))
        .collect();
    let axis = match &georef {
        Ok(g) if !checks.is_empty() => {
            // The control farthest from its lat/lng, for each reading that fits the map.
            let worst = |axis: Axis| {
                checks
                    .iter()
                    .map(|&(code, p, geo)| (code, g.distance(axis.place(p), geo)))
                    .fold(("", 0.0_f64), |w, c| if c.1 > w.1 { c } else { w })
            };
            let results: Vec<(Axis, (&str, f64))> =
                fitting.iter().map(|&a| (a, worst(a))).collect();
            match results.iter().find(|(_, (_, m))| *m <= GEOREF_TOLERANCE_M) {
                Some(&(axis, (_, metres))) => {
                    notes.push(FitNote::GeorefChecked {
                        controls: checks.len(),
                        metres,
                    });
                    axis
                }
                None => {
                    let (_, (control, metres)) = results
                        .iter()
                        .min_by(|a, b| a.1.1.total_cmp(&b.1.1))
                        .copied()
                        .unwrap_or((Axis::YUp, ("", f64::INFINITY)));
                    return Err(FitError::GeorefMismatch {
                        control: control.to_owned(),
                        metres,
                    });
                }
            }
        }
        Ok(_) => {
            notes.push(FitNote::GeorefSkipped(SkipReason::NoLatLng));
            fitting[0]
        }
        Err(reason) => {
            notes.push(FitNote::GeorefSkipped(reason.clone()));
            fitting[0]
        }
    };
    if axis == Axis::YUp && !positioned.is_empty() {
        notes.insert(0, FitNote::YUpDetected);
    }
    if !geo_only.is_empty() {
        notes.push(FitNote::PlacedByGeoref(
            geo_only.iter().map(|&(_, c, _)| c.to_owned()).collect(),
        ));
    }

    let mut fitted: CourseEvent<MapCoord> =
        event.map_points(|p| coord(axis.place(p)), |o| axis.turn(o));
    for (i, p) in placed {
        fitted.controls[i].position = Some(coord(p));
    }
    Ok(Fit {
        event: fitted,
        notes,
    })
}
