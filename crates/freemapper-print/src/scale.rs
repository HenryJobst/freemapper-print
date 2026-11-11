//! Map kinds and the symbol size table live in `freemapper-printrules` (ADR 0056); this module re-exports
//! them and adds what needs the map file: the `freemapper.toml` next to a map and the guess of the kind.

use std::path::Path;

pub use crate::printrules::scale::*;

use crate::omap::MapDocument;

pub use freemapper_contract::SidecarError;
use freemapper_contract::{Sidecar, SidecarKind};

/// The map kind from `freemapper.toml` next to `map_path`: `Ok(None)` without the file, without
/// `map_kind` in it or with a kind this version does not know (a newer freemapper may write one), an
/// error when the file exists but cannot be read or parsed. The reader is the contract's (core ADR 0027).
pub fn read_sidecar(map_path: &Path) -> Result<Option<MapKind>, SidecarError> {
    Ok(Sidecar::next_to(map_path)?.and_then(|s| match s.map_kind {
        SidecarKind::Known(kind) => Some(kind),
        SidecarKind::Absent | SidecarKind::Unknown(_) => None,
    }))
}

/// The map title from `freemapper.toml` next to `map_path`: `Ok(None)` without the file, without `title`
/// in it or with an empty one, an error when the file exists but cannot be read or parsed.
pub fn read_sidecar_title(map_path: &Path) -> Result<Option<String>, SidecarError> {
    Ok(Sidecar::next_to(map_path)?.and_then(|s| s.title().map(str::to_owned)))
}

/// Infers the map kind: `freemapper.toml` next to the map wins; otherwise the symbol set name and
/// scale decide. A `freemapper.toml` that cannot be used counts as missing; [`read_sidecar`] tells
/// why.
pub fn infer_kind(map: &MapDocument, map_path: &Path) -> Option<MapKind> {
    if let Ok(Some(kind)) = read_sidecar(map_path) {
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
    fn broken_sidecar_is_reported_and_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let map_path = dir.path().join("a.omap");
        // A key of the wrong type counts as absent in the contract's reader; only text that is no TOML fails.
        std::fs::write(dir.path().join("freemapper.toml"), "map_kind = 4000\n").unwrap();
        assert_eq!(read_sidecar(&map_path).unwrap(), None);
        std::fs::write(dir.path().join("freemapper.toml"), "map_kind = \n").unwrap();
        let err = read_sidecar(&map_path).unwrap_err();
        assert!(
            err.to_string().contains("freemapper.toml is not valid"),
            "{err}"
        );
        let map = read_map_file(Path::new(LUEBECK)).unwrap();
        assert_eq!(
            infer_kind(&map, &map_path),
            Some(MapKind::Sprint),
            "falls back to the file"
        );
        // Without map_kind, or without the file: no kind, no error.
        std::fs::write(dir.path().join("freemapper.toml"), "title = \"x\"\n").unwrap();
        assert!(read_sidecar(&map_path).unwrap().is_none());
        std::fs::remove_file(dir.path().join("freemapper.toml")).unwrap();
        assert!(read_sidecar(&map_path).unwrap().is_none());
    }

    #[test]
    fn an_unknown_kind_counts_as_absent_and_keeps_the_title() {
        let dir = tempfile::tempdir().unwrap();
        let map_path = dir.path().join("a.omap");
        std::fs::write(
            dir.path().join("freemapper.toml"),
            "map_kind = \"gym\"\ntitle = \"Halle\"\nfuture_key = 1\n",
        )
        .unwrap();
        assert_eq!(read_sidecar(&map_path).unwrap(), None);
        assert_eq!(
            read_sidecar_title(&map_path).unwrap().as_deref(),
            Some("Halle")
        );
        // The kind then comes from the map file.
        let map = read_map_file(Path::new(LUEBECK)).unwrap();
        assert_eq!(infer_kind(&map, &map_path), Some(MapKind::Sprint));
        // Names are matched as written.
        std::fs::write(
            dir.path().join("freemapper.toml"),
            "map_kind = \"Sprint\"\n",
        )
        .unwrap();
        assert_eq!(read_sidecar(&map_path).unwrap(), None);
    }

    #[test]
    fn sidecar_title_is_read_trimmed() {
        let dir = tempfile::tempdir().unwrap();
        let map_path = dir.path().join("x.omap");
        assert_eq!(read_sidecar_title(&map_path).unwrap(), None);
        let toml = dir.path().join("freemapper.toml");
        std::fs::write(&toml, "title = \"  Stadtsprint  \"\n").unwrap();
        assert_eq!(
            read_sidecar_title(&map_path).unwrap().as_deref(),
            Some("Stadtsprint")
        );
        std::fs::write(&toml, "title = \" \"\nmap_kind = \"sprint\"\n").unwrap();
        assert_eq!(read_sidecar_title(&map_path).unwrap(), None);
        assert_eq!(read_sidecar(&map_path).unwrap(), Some(MapKind::Sprint));
    }

    #[test]
    fn issprom_at_1000_is_a_schoolyard_map() {
        let mut map = read_map_file(Path::new(LUEBECK)).unwrap();
        map.georef.scale_denominator = 1000;
        assert_eq!(
            infer_kind(&map, Path::new(LUEBECK)),
            Some(MapKind::Schoolyard)
        );
    }

    #[test]
    fn forest_fallback_boundaries() {
        let mut map = read_map_file(Path::new(LUEBECK)).unwrap();
        map.symbol_set.name = String::new();
        for (scale, kind) in [
            (7499, None),
            (7500, Some(MapKind::Forest)),
            (15000, Some(MapKind::Forest)),
            (15001, None),
        ] {
            map.georef.scale_denominator = scale;
            assert_eq!(infer_kind(&map, Path::new(LUEBECK)), kind, "1:{scale}");
        }
        // A sprint marker at a forest scale is no forest map: the caller asks.
        map.symbol_set.name = "ISSprOM 2019".into();
        map.georef.scale_denominator = 10000;
        assert_eq!(infer_kind(&map, Path::new(LUEBECK)), None);
    }

    #[test]
    fn infer_unknown() {
        let mut map = read_map_file(Path::new(LUEBECK)).unwrap();
        map.georef.scale_denominator = 25000;
        assert_eq!(infer_kind(&map, Path::new(LUEBECK)), None);
    }
}
