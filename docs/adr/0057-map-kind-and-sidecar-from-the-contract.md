# 0057. The map kind and the sidecar reader come from the contract

Date: 2026-11-11
Status: Accepted, amends [0056](0056-print-rules-come-from-freemapper-core.md) and [0052](0052-an-unknown-map-kind-counts-as-absent.md)

## Context
Core v0.18.0 put `MapKind` and the reader of `freemapper.toml` (`Sidecar`) into `freemapper-contract`, and core
v0.19.0 made the print rules use that `MapKind` (core ADR 0028). print had a copy of both.

## Decision
- `scale::MapKind` is the contract's type, via `freemapper-printrules`.
- `read_sidecar` and `read_sidecar_title` call `Sidecar::next_to`. An unknown `map_kind` is `SidecarKind::Unknown`
  and counts as absent here, as ADR 0052 says; the title is read as before. One difference: a `map_kind` of the wrong type (`map_kind = 4000`) is no longer an error but counts as absent, as in the shared reader. Only text that is no TOML fails. `SidecarError` is the contract's.
- `allowed_scales` and `proposed_contour_interval_m` are the functions `scale::allowed_scales(kind)` and
  `scale::proposed_contour_interval_m(kind)`.
- `freemapper-contract` is a normal dependency without its default features; the dev-dependency keeps the
  feature `fixtures`.
