//! Map kinds and the symbol factor table (ADR 0012).
//!
//! Printing at another scale than the map was drawn at scales geometry by the geometry factor and symbol
//! dimensions by the symbol factor, which the IOF standards fix per map kind.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::omap::MapDocument;

/// Kartenart: the kind of map, as defined by freemapper's glossary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MapKind {
    Sprint,
    Schoolyard,
    Forest,
}

impl MapKind {
    fn name(self) -> &'static str {
        match self {
            MapKind::Sprint => "sprint",
            MapKind::Schoolyard => "schoolyard",
            MapKind::Forest => "forest",
        }
    }

    fn allowed_scales(self) -> &'static [u32] {
        match self {
            MapKind::Sprint => &[3000, 4000, 5000],
            MapKind::Schoolyard => &[1000],
            MapKind::Forest => &[5000, 7500, 10000, 15000],
        }
    }
}

fn join_scales(scales: &[u32]) -> String {
    scales
        .iter()
        .map(|s| format!("1:{s}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Errors for scales outside the standard table of a map kind.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScaleError {
    #[error(
        "a {kind:?} map cannot be printed at 1:{print_scale}; allowed print scales: {}",
        join_scales(allowed)
    )]
    PrintScaleNotAllowed {
        kind: MapKind,
        print_scale: u32,
        allowed: Vec<u32>,
    },
    #[error(
        "a {kind:?} map drawn at 1:{map_scale} is not a standard {name} map; allowed map scales: {}",
        join_scales(allowed),
        name = kind.name()
    )]
    MapScaleNotAllowed {
        kind: MapKind,
        map_scale: u32,
        allowed: Vec<u32>,
    },
}

/// Symbol enlargement of `scale` relative to the base scale of the kind (Sprint 1:4000, Forest 1:15000,
/// Schoolyard 1:1000). `None` outside the standard table.
pub fn standard_enlargement(kind: MapKind, scale: u32) -> Option<f64> {
    match (kind, scale) {
        (MapKind::Sprint, 3000 | 4000 | 5000) => Some(1.0),
        (MapKind::Forest, 15000) => Some(1.0),
        (MapKind::Forest, 10000 | 7500 | 5000) => Some(1.5),
        (MapKind::Schoolyard, 1000) => Some(1.0),
        _ => None,
    }
}

/// Factor for symbol dimensions when a map drawn at `map_scale` is printed at `print_scale`.
pub fn symbol_factor(kind: MapKind, map_scale: u32, print_scale: u32) -> Result<f64, ScaleError> {
    let allowed = || kind.allowed_scales().to_vec();
    let from =
        standard_enlargement(kind, map_scale).ok_or_else(|| ScaleError::MapScaleNotAllowed {
            kind,
            map_scale,
            allowed: allowed(),
        })?;
    let to = standard_enlargement(kind, print_scale).ok_or_else(|| {
        ScaleError::PrintScaleNotAllowed {
            kind,
            print_scale,
            allowed: allowed(),
        }
    })?;
    Ok(to / from)
}

/// Factor for object geometry: map scale divided by print scale.
pub fn geometry_factor(map_scale: u32, print_scale: u32) -> f64 {
    f64::from(map_scale) / f64::from(print_scale)
}

#[derive(Deserialize)]
struct Sidecar {
    map_kind: Option<MapKind>,
}

fn read_sidecar(map_path: &Path) -> Option<MapKind> {
    let dir = map_path.parent()?;
    let text = std::fs::read_to_string(dir.join("freemapper.toml")).ok()?;
    toml::from_str::<Sidecar>(&text).ok()?.map_kind
}

/// Infers the map kind: `freemapper.toml` next to the map wins; otherwise the symbol set name and scale decide.
pub fn infer_kind(map: &MapDocument, map_path: &Path) -> Option<MapKind> {
    if let Some(kind) = read_sidecar(map_path) {
        return Some(kind);
    }
    let name = map.symbol_set.name.to_lowercase();
    let scale = map.georef.scale_denominator;
    let sprint_marker = name.contains("issprom") || name.contains("sprint");
    if sprint_marker {
        return match scale {
            3000..=5000 => Some(MapKind::Sprint),
            1000 => Some(MapKind::Schoolyard),
            _ => None,
        };
    }
    if name.contains("isom") {
        return (5000..=15000).contains(&scale).then_some(MapKind::Forest);
    }
    // No marker in the set name: only scales above 1:5000 are unambiguous. 1:5000 is both a sprint and a
    // forest scale, so it stays unknown and the caller asks for an explicit kind.
    (7500..=15000).contains(&scale).then_some(MapKind::Forest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::omap::read::read_map_file;

    const LUEBECK: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/maps/luebeck.omap"
    );
    const FOREST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/maps/forest-sample.omap"
    );

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn sprint_keeps_symbol_size() {
        assert_eq!(symbol_factor(MapKind::Sprint, 4000, 5000), Ok(1.0));
        assert_eq!(symbol_factor(MapKind::Sprint, 4000, 3000), Ok(1.0));
    }

    #[test]
    fn forest_enlarges_at_10000() {
        assert_eq!(symbol_factor(MapKind::Forest, 15000, 10000), Ok(1.5));
        assert!(close(
            symbol_factor(MapKind::Forest, 10000, 15000).unwrap(),
            1.0 / 1.5
        ));
        assert_eq!(symbol_factor(MapKind::Forest, 15000, 7500), Ok(1.5));
    }

    #[test]
    fn schoolyard_only_1000() {
        assert_eq!(symbol_factor(MapKind::Schoolyard, 1000, 1000), Ok(1.0));
        let err = symbol_factor(MapKind::Schoolyard, 1000, 2000).unwrap_err();
        match &err {
            ScaleError::PrintScaleNotAllowed { allowed, .. } => assert_eq!(allowed, &[1000]),
            other => panic!("unexpected {other:?}"),
        }
        assert!(err.to_string().contains("1:1000"));
    }

    #[test]
    fn sprint_refuses_10000() {
        let err = symbol_factor(MapKind::Sprint, 4000, 10000).unwrap_err();
        match &err {
            ScaleError::PrintScaleNotAllowed { allowed, .. } => {
                assert_eq!(allowed, &[3000, 4000, 5000])
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(err.to_string().contains("1:3000, 1:4000, 1:5000"));
        assert!(matches!(
            symbol_factor(MapKind::Sprint, 10000, 4000),
            Err(ScaleError::MapScaleNotAllowed { .. })
        ));
    }

    #[test]
    fn geometry_factor_4000_to_5000() {
        assert!(close(geometry_factor(4000, 5000), 0.8));
    }

    #[test]
    fn infer_from_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("freemapper.toml"),
            "title = \"x\"\nmap_kind = \"schoolyard\"\n",
        )
        .unwrap();
        let map = read_map_file(Path::new(LUEBECK)).unwrap();
        assert_eq!(
            infer_kind(&map, &dir.path().join("a.omap")),
            Some(MapKind::Schoolyard)
        );
    }

    #[test]
    fn infer_from_symbol_set() {
        let map = read_map_file(Path::new(LUEBECK)).unwrap();
        assert_eq!(infer_kind(&map, Path::new(LUEBECK)), Some(MapKind::Sprint));
        let map = read_map_file(Path::new(FOREST)).unwrap();
        assert_eq!(infer_kind(&map, Path::new(FOREST)), Some(MapKind::Forest));
    }

    #[test]
    fn infer_forest_by_scale_without_marker() {
        let mut map = read_map_file(Path::new(LUEBECK)).unwrap();
        map.symbol_set.name = String::new();
        map.georef.scale_denominator = 10000;
        assert_eq!(infer_kind(&map, Path::new(LUEBECK)), Some(MapKind::Forest));
        map.symbol_set.name = "Custom".into();
        assert_eq!(infer_kind(&map, Path::new(LUEBECK)), Some(MapKind::Forest));
    }

    #[test]
    fn infer_none_at_5000_without_marker() {
        let mut map = read_map_file(Path::new(LUEBECK)).unwrap();
        map.symbol_set.name = String::new();
        map.georef.scale_denominator = 5000;
        assert_eq!(infer_kind(&map, Path::new(LUEBECK)), None);
    }

    #[test]
    fn infer_unknown() {
        let mut map = read_map_file(Path::new(LUEBECK)).unwrap();
        map.georef.scale_denominator = 25000;
        assert_eq!(infer_kind(&map, Path::new(LUEBECK)), None);
    }
}
