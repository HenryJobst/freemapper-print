//! Print rules as data profiles (ADR 0031): the symbol size table, north-line spacing, course symbol
//! sizes, the course colour and the other print values, each with its source.
//!
//! A rule profile is a TOML file in `rules/`, embedded with `include_str!`. A base profile (such as
//! `iof-print`) holds a full rule set; an overlay names its base and adds rules, replaces base rules
//! with the same `id`, or switches them off with `enabled = false`. [`load_rule_set`] merges one base
//! and its overlays into a [`RuleSet`] that remembers where each rule came from and checks that every
//! map kind gets each value exactly once. `rules/README.md` describes the file format.
//!
//! Each rule names a `key`; [`KNOWN_KEYS`] lists the keys and the values each reads. Value names carry
//! their unit ([`UNITS`]); value names without a unit are tables whose shape [`TABLE_VALUES`]
//! documents. Every value holds at the base scale of its map kind, so a `print_scales` filter is
//! read at the base scale (sprint 1:4000, forest 1:15000, schoolyard 1:1000).
//!
//! Provenance: the format, the loader and its validation are copied from freemapper-courseplan,
//! `crates/freemapper-courseplan/src/rules.rs` at commit 86f9a42399397883762ecaf362d76fc36e3a3170
//! (tag v0.2.0, GPL-3.0-or-later, same author), and adapted: print rule keys instead of check keys,
//! no `disciplines` filter, values checked against the key, and a set that resolves its values per
//! map kind when it is loaded.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use toml::{Table, Value};

use crate::placement::GapRange;
use crate::scale::MapKind;

/// The profile file format this program reads.
pub const FORMAT: i64 = 1;

/// The base profile a project uses unless it names another (`[print] rule_profile`).
pub const BASE_PROFILE: &str = "iof-print";

/// The embedded profiles: profile id and content.
const EMBEDDED: &[(&str, &str)] = &[("iof-print", include_str!("../rules/iof-print.toml"))];

/// The value names of `course_symbol` rules, one per field of [`CourseSymbolSizes`].
const COURSE_SYMBOL_VALUES: [&str; 18] = [
    "circle_diameter_mm",
    "line_width_mm",
    "start_side_mm",
    "finish_inner_mm",
    "finish_outer_mm",
    "number_height_mm",
    "marked_route_dash_mm",
    "marked_route_gap_mm",
    "out_of_bounds_line_mm",
    "crossing_point_mm",
    "out_of_bounds_hatch_angle_degrees",
    "out_of_bounds_hatch_second_angle_degrees",
    "out_of_bounds_hatch_spacing_mm",
    "out_of_bounds_hatch_line_mm",
    "first_aid_mm",
    "first_aid_bar_mm",
    "refreshment_mm",
    "refreshment_line_mm",
];

/// The rule keys this program reads, each with the value names a rule of that key may give.
pub const KNOWN_KEYS: &[(&str, &[&str])] = &[
    ("scale_table", &["rows"]),
    ("north_lines", &["spacing_metres"]),
    ("contour_interval", &["proposed_metres"]),
    ("course_symbol", &COURSE_SYMBOL_VALUES),
    ("description_box", &["cell_mm"]),
    (
        "course_colour",
        &[
            "cyan_percent",
            "magenta_percent",
            "yellow_percent",
            "black_percent",
        ],
    ),
    (
        "placement_weights",
        &[
            "black_factor",
            "brown_factor",
            "blue_factor",
            "other_factor",
        ],
    ),
    ("number_gap", &["min_gap_mm", "max_gap_mm"]),
];

/// Keys whose values hold for every map kind: their rules take no filter.
const KIND_INDEPENDENT_KEYS: &[&str] = &["course_colour", "placement_weights", "number_gap"];

/// Units a numeric value name ends in (`metres`, `spacing_metres`, `cyan_percent`).
pub const UNITS: &[&str] = &["metres", "mm", "degrees", "percent", "factor"];

/// Value names that hold a table or a list instead of a number with a unit: rule key, value name
/// and the shape of the value.
pub const TABLE_VALUES: &[(&str, &str, &str)] = &[(
    "scale_table",
    "rows",
    "[{ scale, size_factor, standard }, ...]: one row per scale denominator a map of the kind may be \
     drawn and printed at, ascending; size_factor is the symbol size relative to the base scale, \
     standard = false marks a reduction the standard does not foresee (default true)",
)];

/// The document key of values freemapper-print chose itself where no standard gives a number
/// (`[sources.fmp]` in a profile). Such a rule needs a note.
pub const PROPOSAL_DOC: &str = "fmp";

/// A text in several languages, keyed by language code. English is always present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizedText(pub BTreeMap<String, String>);

impl LocalizedText {
    /// The text in `lang` (a language code such as `de`), else the English text.
    pub fn get(&self, lang: &str) -> &str {
        self.0
            .get(lang)
            .or_else(|| self.0.get("en"))
            .map(String::as_str)
            .unwrap_or("")
    }
}

/// How serious a deviation from a rule is when the program reports one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Hint,
}

impl Severity {
    /// The name used in profile files.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Hint => "hint",
        }
    }

    fn parse(text: &str) -> Option<Severity> {
        [Severity::Error, Severity::Warning, Severity::Hint]
            .into_iter()
            .find(|s| s.as_str() == text)
    }
}

/// Whether a profile stands alone or extends a base profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileKind {
    Base,
    Overlay,
}

/// A document a profile cites, listed under `[sources.<doc>]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub title: String,
    pub url: Option<String>,
}

/// Where a rule comes from: the document key of the profile's `[sources]` and the section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRef {
    pub doc: String,
    pub section: String,
}

/// One print rule of a profile.
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    /// Stable dotted id, `<profile>.<family>.<name>`.
    pub id: String,
    /// What the rule sets, one of [`KNOWN_KEYS`].
    pub key: String,
    pub severity: Severity,
    /// `false` in an overlay switches the base rule with the same id off.
    pub enabled: bool,
    /// Map kinds the rule applies to; empty means all.
    pub map_kinds: Vec<MapKind>,
    /// Print scale denominators the rule applies to; empty means all.
    pub print_scales: Vec<u32>,
    /// The values; names carry their unit.
    pub values: Table,
    pub source: SourceRef,
    pub note: Option<LocalizedText>,
}

impl Rule {
    /// Whether the rule's filters select a map of `kind` printed at 1:`print_scale`.
    pub fn applies(&self, kind: MapKind, print_scale: u32) -> bool {
        (self.map_kinds.is_empty() || self.map_kinds.contains(&kind))
            && (self.print_scales.is_empty() || self.print_scales.contains(&print_scale))
    }
}

/// One parsed profile file.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleProfile {
    /// The profile id, such as `iof-print`.
    pub id: String,
    pub kind: ProfileKind,
    /// The base profile an overlay extends.
    pub base: Option<String>,
    pub name: LocalizedText,
    /// The date the rules apply from, as written in the file.
    pub valid_from: Option<String>,
    pub sources: BTreeMap<String, Source>,
    pub rules: Vec<Rule>,
}

/// A rule of a rule set with the profile it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct SetRule {
    pub rule: Rule,
    pub profile: String,
}

/// A rule that a profile switched off with `enabled = false`.
#[derive(Debug, Clone, PartialEq)]
pub struct SwitchedOff {
    /// The rule as it was before it was switched off.
    pub rule: Rule,
    /// The profile the switched-off rule came from.
    pub profile: String,
    /// The profile that switched it off.
    pub by: String,
}

/// Sizes of the course planning symbols (7xx) in mm at the base scale of a map kind. Circle
/// diameters run from line centre to line centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CourseSymbolSizes {
    /// Control circle (703).
    pub circle_diameter_mm: f64,
    /// Lines of circles, legs, start and finish (705).
    pub line_width_mm: f64,
    /// Side of the start triangle (701).
    pub start_side_mm: f64,
    /// Inner finish circle (706).
    pub finish_inner_mm: f64,
    /// Outer finish circle (706).
    pub finish_outer_mm: f64,
    /// Height of a control number (704).
    pub number_height_mm: f64,
    /// Dash of a marked route (707).
    pub marked_route_dash_mm: f64,
    /// Gap of a marked route (707).
    pub marked_route_gap_mm: f64,
    /// Width of an out-of-bounds boundary (708).
    pub out_of_bounds_line_mm: f64,
    /// Length of a crossing point (710).
    pub crossing_point_mm: f64,
    /// Direction of the hatch lines of an out-of-bounds area (709), in degrees counter-clockwise from
    /// east on paper.
    pub out_of_bounds_hatch_angle_degrees: f64,
    /// Direction of a second set of hatch lines, for a cross hatch; `None` for one set.
    pub out_of_bounds_hatch_second_angle_degrees: Option<f64>,
    /// Distance between the centres of two hatch lines.
    pub out_of_bounds_hatch_spacing_mm: f64,
    /// Width of a hatch line.
    pub out_of_bounds_hatch_line_mm: f64,
    /// Length of each bar of a first aid post (712).
    pub first_aid_mm: f64,
    /// Width of a bar of a first aid post.
    pub first_aid_bar_mm: f64,
    /// Outer width and height of a refreshment point (713).
    pub refreshment_mm: f64,
    /// Line width of a refreshment point.
    pub refreshment_line_mm: f64,
}

/// How much a mm² of map detail under a control number counts, by colour class.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacementWeights {
    pub black: f64,
    pub brown: f64,
    pub blue: f64,
    pub other: f64,
}

/// One row of the symbol size table (ADR 0018).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleEntry {
    /// Scale denominator.
    pub scale: u32,
    /// Symbol size relative to the base scale of the kind.
    pub size_factor: f64,
    /// The standard foresees this scale; `false` for a reduction below the base scale.
    pub standard: bool,
}

/// The values of one map kind, resolved when the set is loaded.
#[derive(Debug, Clone, PartialEq)]
struct KindValues {
    scales: Vec<ScaleEntry>,
    north_line_spacing_m: f64,
    contour_interval_m: Option<f64>,
    course: CourseSymbolSizes,
    description_cell_mm: f64,
}

/// A base profile merged with its overlays: the print rules a sheet uses.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleSet {
    pub base: String,
    pub overlays: Vec<String>,
    /// Active rules in file order: base rules first, then rules the overlays add.
    pub rules: Vec<SetRule>,
    pub switched_off: Vec<SwitchedOff>,
    /// The `[sources]` of every profile in the set, by profile id.
    pub sources: BTreeMap<String, BTreeMap<String, Source>>,
    /// Per map kind, in the order of [`MapKind::ALL`].
    kinds: Vec<KindValues>,
    purple_cmyk: [f64; 4],
    placement_weights: PlacementWeights,
    number_gap: Option<(f64, f64)>,
}

impl RuleSet {
    /// The active rule with this id.
    pub fn rule(&self, id: &str) -> Option<&SetRule> {
        self.rules.iter().find(|r| r.rule.id == id)
    }

    /// The document a rule cites, from the profile the rule came from.
    pub fn source(&self, rule: &SetRule) -> Option<&Source> {
        self.sources.get(&rule.profile)?.get(&rule.rule.source.doc)
    }

    fn kind(&self, kind: MapKind) -> &KindValues {
        let slot = match kind {
            MapKind::Sprint => 0,
            MapKind::Schoolyard => 1,
            MapKind::Forest => 2,
        };
        &self.kinds[slot]
    }

    /// Course symbol sizes in mm at the base scale of `kind`.
    pub fn course_symbol(&self, kind: MapKind) -> CourseSymbolSizes {
        self.kind(kind).course
    }

    /// Ground distance (m) between north lines (ADR 0027).
    pub fn north_line_spacing_m(&self, kind: MapKind) -> f64 {
        self.kind(kind).north_line_spacing_m
    }

    /// The contour interval (m) `init` proposes, if the set has one for `kind`.
    pub fn proposed_contour_interval_m(&self, kind: MapKind) -> Option<f64> {
        self.kind(kind).contour_interval_m
    }

    /// Side (mm) of a control description cell printed on the map.
    pub fn description_cell_mm(&self, kind: MapKind) -> f64 {
        self.kind(kind).description_cell_mm
    }

    /// The course colour as CMYK fractions from 0 to 1.
    pub fn purple_cmyk(&self) -> [f64; 4] {
        self.purple_cmyk
    }

    /// Weights of map detail under a control number.
    pub fn placement_weights(&self) -> PlacementWeights {
        self.placement_weights
    }

    /// The smallest and the largest gap (print mm at the base scale) between a control circle and its number,
    /// from the active `number_gap` rules (ruling R29). With several rules the strictest wins: the largest
    /// minimum and the smallest maximum, normalised as [`GapRange::from_limits`] does. `None` without such a
    /// rule; placement then uses a fixed gap.
    pub fn number_gap(&self) -> Option<(f64, f64)> {
        self.number_gap
    }

    /// The symbol size table of `kind`: scale denominator and symbol size relative to the base
    /// scale, ascending by scale.
    pub fn scale_table(&self, kind: MapKind) -> Vec<(u32, f64)> {
        self.kind(kind)
            .scales
            .iter()
            .map(|e| (e.scale, e.size_factor))
            .collect()
    }

    /// The rows of the symbol size table of `kind`, ascending by scale.
    pub fn scale_entries(&self, kind: MapKind) -> &[ScaleEntry] {
        &self.kind(kind).scales
    }

    /// The row of the symbol size table for `kind` at 1:`scale`.
    pub fn scale_entry(&self, kind: MapKind, scale: u32) -> Option<ScaleEntry> {
        self.kind(kind)
            .scales
            .iter()
            .find(|e| e.scale == scale)
            .copied()
    }

    /// Whether the standard foresees printing a map of `kind` at 1:`scale`.
    pub fn is_standard_scale(&self, kind: MapKind, scale: u32) -> bool {
        self.scale_entry(kind, scale).is_some_and(|e| e.standard)
    }
}

/// Why a rule profile or rule set cannot be loaded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    #[error("unknown rule profile {id}; available: {available}")]
    UnknownProfile { id: String, available: String },
    #[error("rule profile {0} is an overlay, not a base profile")]
    NotABase(String),
    #[error("rule profile {0} is a base profile, not an overlay")]
    NotAnOverlay(String),
    #[error("overlay {overlay} extends {extends}, not {base}")]
    OverlayForAnotherBase {
        overlay: String,
        extends: String,
        base: String,
    },
    #[error("rule profile {profile} is malformed: {message}")]
    Malformed { profile: String, message: String },
    #[error("rule {rule} in profile {profile} has the unknown key {key}")]
    UnknownKey {
        profile: String,
        rule: String,
        key: String,
    },
    #[error("overlay {overlay} switches off rule {rule}, which the base profile does not have")]
    UnknownRule { overlay: String, rule: String },
    #[error("rule set {set} cannot be used: {message}")]
    Incomplete { set: String, message: String },
}

/// The ids of the embedded profiles, base profiles and overlays.
pub fn available() -> Vec<&'static str> {
    EMBEDDED.iter().map(|(id, _)| *id).collect()
}

/// The embedded profile `id`.
pub fn profile(id: &str) -> Result<RuleProfile, RuleError> {
    profile_in(EMBEDDED, id)
}

/// The embedded base profile `iof-print` without overlays, loaded once.
pub fn base() -> &'static RuleSet {
    static BASE: OnceLock<RuleSet> = OnceLock::new();
    BASE.get_or_init(|| match load_rule_set(BASE_PROFILE, &[]) {
        Ok(set) => set,
        // The tests load every embedded profile, so this is a build mistake.
        Err(e) => panic!("the embedded rule profile {BASE_PROFILE} is invalid: {e}"),
    })
}

/// Merges the embedded base profile `base` with the overlays, in order.
pub fn load_rule_set(base: &str, overlays: &[String]) -> Result<RuleSet, RuleError> {
    load_from(EMBEDDED, base, overlays)
}

fn profile_in(files: &[(&str, &str)], id: &str) -> Result<RuleProfile, RuleError> {
    match files.iter().find(|(stem, _)| *stem == id) {
        Some((stem, text)) => parse_profile(stem, text),
        None => Err(RuleError::UnknownProfile {
            id: id.to_owned(),
            available: files
                .iter()
                .map(|(stem, _)| *stem)
                .collect::<Vec<_>>()
                .join(", "),
        }),
    }
}

/// [`load_from`] for tests elsewhere in the crate, with test-only profiles.
#[cfg(test)]
pub(crate) fn load_for_tests(
    files: &[(&str, &str)],
    base: &str,
    overlays: &[String],
) -> Result<RuleSet, RuleError> {
    load_from(files, base, overlays)
}

/// The text of the embedded profile `id`, for tests elsewhere in the crate.
#[cfg(test)]
pub(crate) fn embedded_text(id: &str) -> &'static str {
    EMBEDDED
        .iter()
        .find(|(stem, _)| *stem == id)
        .map_or("", |(_, text)| text)
}

fn load_from(
    files: &[(&str, &str)],
    base: &str,
    overlays: &[String],
) -> Result<RuleSet, RuleError> {
    let base_profile = profile_in(files, base)?;
    if base_profile.kind != ProfileKind::Base {
        return Err(RuleError::NotABase(base.to_owned()));
    }
    let mut merged = Merged::default();
    merge(&mut merged, base_profile)?;
    for overlay in overlays {
        let profile = profile_in(files, overlay)?;
        if profile.kind != ProfileKind::Overlay {
            return Err(RuleError::NotAnOverlay(overlay.clone()));
        }
        let extends = profile.base.clone().unwrap_or_default();
        if extends != base {
            return Err(RuleError::OverlayForAnotherBase {
                overlay: overlay.clone(),
                extends,
                base: base.to_owned(),
            });
        }
        merge(&mut merged, profile)?;
    }
    let set_name = std::iter::once(base)
        .chain(overlays.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" + ");
    let incomplete = |message: String| RuleError::Incomplete {
        set: set_name.clone(),
        message,
    };
    let mut kinds = Vec::new();
    for kind in MapKind::ALL {
        kinds.push(resolve_kind(&merged.rules, kind).map_err(incomplete)?);
    }
    let colour = |name| needed(&merged.rules, "course_colour", name, None);
    let mut purple_cmyk = [0.0; 4];
    for (slot, name) in purple_cmyk.iter_mut().zip([
        "cyan_percent",
        "magenta_percent",
        "yellow_percent",
        "black_percent",
    ]) {
        *slot = colour(name).map_err(incomplete)? / 100.0;
    }
    let weight = |name| needed(&merged.rules, "placement_weights", name, None);
    let placement_weights = PlacementWeights {
        black: weight("black_factor").map_err(incomplete)?,
        brown: weight("brown_factor").map_err(incomplete)?,
        blue: weight("blue_factor").map_err(incomplete)?,
        other: weight("other_factor").map_err(incomplete)?,
    };
    let number_gap = number_gap(&merged.rules);
    Ok(RuleSet {
        base: base.to_owned(),
        overlays: overlays.to_vec(),
        rules: merged.rules,
        switched_off: merged.switched_off,
        sources: merged.sources,
        kinds,
        purple_cmyk,
        placement_weights,
        number_gap,
    })
}

/// The strictest range of the active `number_gap` rules: the largest `min_gap_mm` and the smallest
/// `max_gap_mm`, normalised by [`GapRange::from_limits`] (ruling R30, courseplan ADR 0032). A rule with neither
/// value is skipped; `None` when no rule gives one. Values that are not finite or not above 0 never get here: the
/// profile loader refuses them.
fn number_gap(rules: &[SetRule]) -> Option<(f64, f64)> {
    let values = |name: &str| -> Vec<f64> {
        rules
            .iter()
            .filter(|r| r.rule.key == "number_gap")
            .filter_map(|r| r.rule.values.get(name).and_then(number))
            .collect()
    };
    let (mins, maxs) = (values("min_gap_mm"), values("max_gap_mm"));
    if mins.is_empty() && maxs.is_empty() {
        return None;
    }
    let range = GapRange::from_limits(
        mins.into_iter().reduce(f64::max),
        maxs.into_iter().reduce(f64::min),
    );
    Some((range.min, range.max))
}

#[derive(Default)]
struct Merged {
    rules: Vec<SetRule>,
    switched_off: Vec<SwitchedOff>,
    sources: BTreeMap<String, BTreeMap<String, Source>>,
}

/// Adds the rules of `profile` to `set`: a rule with a known id replaces the rule, a new id is
/// added, and `enabled = false` switches the rule off.
fn merge(set: &mut Merged, profile: RuleProfile) -> Result<(), RuleError> {
    let from = profile.id.clone();
    set.sources.insert(from.clone(), profile.sources);
    for rule in profile.rules {
        let active = set.rules.iter().position(|r| r.rule.id == rule.id);
        let off = set.switched_off.iter().position(|r| r.rule.id == rule.id);
        if rule.enabled {
            let entry = SetRule {
                rule,
                profile: from.clone(),
            };
            match active {
                Some(i) => set.rules[i] = entry,
                None => {
                    if let Some(i) = off {
                        set.switched_off.remove(i);
                    }
                    set.rules.push(entry);
                }
            }
        } else if let Some(i) = active {
            let old = set.rules.remove(i);
            set.switched_off.push(SwitchedOff {
                rule: old.rule,
                profile: old.profile,
                by: from.clone(),
            });
        } else if off.is_some() {
            // Already switched off by an earlier profile.
        } else if profile.kind == ProfileKind::Base {
            set.switched_off.push(SwitchedOff {
                rule,
                profile: from.clone(),
                by: from.clone(),
            });
        } else {
            return Err(RuleError::UnknownRule {
                overlay: from,
                rule: rule.id,
            });
        }
    }
    Ok(())
}

/// The one active rule with `key` that gives `name` for maps of `kind` at their base scale (all
/// rules of the key when `kind` is `None`), and the value. Two such rules are an error.
fn pick<'a>(
    rules: &'a [SetRule],
    key: &str,
    name: &str,
    kind: Option<MapKind>,
) -> Result<Option<&'a Value>, String> {
    let mut found = rules.iter().filter(|r| {
        r.rule.key == key
            && r.rule.values.contains_key(name)
            && kind.is_none_or(|k| r.rule.applies(k, k.base_scale()))
    });
    let first = found.next();
    if let Some(second) = found.next() {
        let first = first.map_or("", |r| r.rule.id.as_str());
        return Err(format!(
            "rules {first} and {} both give {key} {name}{}",
            second.rule.id,
            for_kind(kind)
        ));
    }
    Ok(first.and_then(|r| r.rule.values.get(name)))
}

fn for_kind(kind: Option<MapKind>) -> String {
    kind.map_or(String::new(), |k| format!(" for {} maps", k.name()))
}

/// The number `pick` finds, or `None` without a rule.
fn optional(
    rules: &[SetRule],
    key: &str,
    name: &str,
    kind: Option<MapKind>,
) -> Result<Option<f64>, String> {
    Ok(pick(rules, key, name, kind)?.and_then(number))
}

/// The number `pick` finds; an error without a rule.
fn needed(rules: &[SetRule], key: &str, name: &str, kind: Option<MapKind>) -> Result<f64, String> {
    optional(rules, key, name, kind)?
        .ok_or_else(|| format!("no rule gives {key} {name}{}", for_kind(kind)))
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Integer(n) => Some(*n as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    }
}

fn resolve_kind(rules: &[SetRule], kind: MapKind) -> Result<KindValues, String> {
    let k = Some(kind);
    let rows = pick(rules, "scale_table", "rows", k)?
        .ok_or_else(|| format!("no rule gives scale_table rows{}", for_kind(k)))?;
    // parse_rule checked the shape of each row.
    let scales = scale_rows(rows)?;
    if scales.windows(2).any(|w| w[0].scale >= w[1].scale) {
        return Err(format!(
            "the scale_table rows{} are not in ascending order of scale",
            for_kind(k)
        ));
    }
    // The base scale is where every other value holds, and `scale.rs` refers to it.
    let base = kind.base_scale();
    let at_base = scales
        .iter()
        .any(|e| e.scale == base && e.size_factor == 1.0 && e.standard);
    if !at_base {
        return Err(format!(
            "the scale_table rows{} have no row {{ scale = {base}, size_factor = 1.0 }} for the \
             base scale",
            for_kind(k)
        ));
    }
    let course = |name| needed(rules, "course_symbol", name, k);
    Ok(KindValues {
        scales,
        north_line_spacing_m: needed(rules, "north_lines", "spacing_metres", k)?,
        contour_interval_m: optional(rules, "contour_interval", "proposed_metres", k)?,
        course: CourseSymbolSizes {
            circle_diameter_mm: course("circle_diameter_mm")?,
            line_width_mm: course("line_width_mm")?,
            start_side_mm: course("start_side_mm")?,
            finish_inner_mm: course("finish_inner_mm")?,
            finish_outer_mm: course("finish_outer_mm")?,
            number_height_mm: course("number_height_mm")?,
            marked_route_dash_mm: course("marked_route_dash_mm")?,
            marked_route_gap_mm: course("marked_route_gap_mm")?,
            out_of_bounds_line_mm: course("out_of_bounds_line_mm")?,
            crossing_point_mm: course("crossing_point_mm")?,
            out_of_bounds_hatch_angle_degrees: course("out_of_bounds_hatch_angle_degrees")?,
            out_of_bounds_hatch_second_angle_degrees: optional(
                rules,
                "course_symbol",
                "out_of_bounds_hatch_second_angle_degrees",
                k,
            )?,
            out_of_bounds_hatch_spacing_mm: course("out_of_bounds_hatch_spacing_mm")?,
            out_of_bounds_hatch_line_mm: course("out_of_bounds_hatch_line_mm")?,
            first_aid_mm: course("first_aid_mm")?,
            first_aid_bar_mm: course("first_aid_bar_mm")?,
            refreshment_mm: course("refreshment_mm")?,
            refreshment_line_mm: course("refreshment_line_mm")?,
        },
        description_cell_mm: needed(rules, "description_box", "cell_mm", k)?,
    })
}

/// The rows of a `scale_table` value; an error names the first row that is not
/// `{ scale, size_factor, standard }`.
fn scale_rows(rows: &Value) -> Result<Vec<ScaleEntry>, String> {
    let Value::Array(rows) = rows else {
        return Err("rows is not a list of { scale, size_factor }".to_owned());
    };
    if rows.is_empty() {
        return Err("rows is empty".to_owned());
    }
    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            let place = format!("row {}", i + 1);
            let Value::Table(row) = row else {
                return Err(format!("{place} is not {{ scale, size_factor }}"));
            };
            only_fields(row, &["scale", "size_factor", "standard"], &place)?;
            let scale = match row.get("scale") {
                Some(Value::Integer(n)) if *n > 0 && *n <= i64::from(u32::MAX) => *n as u32,
                _ => return Err(format!("{place}: scale is no scale denominator")),
            };
            let size_factor = match row.get("size_factor").and_then(number) {
                Some(f) if f.is_finite() && f > 0.0 => f,
                _ => return Err(format!("{place}: size_factor is not a number above 0")),
            };
            let standard = match row.get("standard") {
                None => true,
                Some(Value::Boolean(b)) => *b,
                Some(_) => return Err(format!("{place}: standard is not true or false")),
            };
            Ok(ScaleEntry {
                scale,
                size_factor,
                standard,
            })
        })
        .collect()
}

/// Parses one profile file embedded as `id`.
pub fn parse_profile(id: &str, text: &str) -> Result<RuleProfile, RuleError> {
    let bad = |message: String| RuleError::Malformed {
        profile: id.to_owned(),
        message,
    };
    let doc: Table = text
        .parse()
        .map_err(|e: toml::de::Error| bad(e.to_string()))?;
    only_fields(&doc, &["format", "profile", "sources", "rules"], "the file").map_err(bad)?;
    match doc.get("format") {
        Some(Value::Integer(FORMAT)) => {}
        Some(Value::Integer(n)) if *n > FORMAT => {
            return Err(bad(format!(
                "format {n} is newer than this program reads ({FORMAT})"
            )));
        }
        Some(other) => return Err(bad(format!("format {other} is not {FORMAT}"))),
        None => return Err(bad(format!("format = {FORMAT} is missing"))),
    }

    let head = table(&doc, "profile", "the file").map_err(bad)?;
    only_fields(
        head,
        &["id", "kind", "base", "name", "valid_from"],
        "[profile]",
    )
    .map_err(bad)?;
    let file_id = string(head, "id", "[profile]").map_err(bad)?;
    if file_id != id {
        return Err(bad(format!(
            "[profile] id {file_id} differs from the profile id {id}"
        )));
    }
    let kind = match string(head, "kind", "[profile]").map_err(bad)?.as_str() {
        "base" => ProfileKind::Base,
        "overlay" => ProfileKind::Overlay,
        other => return Err(bad(format!("kind {other} is neither base nor overlay"))),
    };
    let base = optional_string(head, "base", "[profile]").map_err(bad)?;
    match (kind, &base) {
        (ProfileKind::Overlay, None) => return Err(bad("an overlay needs a base".to_owned())),
        (ProfileKind::Base, Some(_)) => {
            return Err(bad("a base profile has no base".to_owned()));
        }
        _ => {}
    }
    let name = localized(head.get("name"), "[profile] name").map_err(bad)?;
    let valid_from = match head.get("valid_from") {
        None => None,
        Some(Value::Datetime(date)) => Some(date.to_string()),
        Some(Value::String(date)) => Some(date.clone()),
        Some(_) => return Err(bad("[profile] valid_from is not a date".to_owned())),
    };

    let mut sources = BTreeMap::new();
    if let Some(value) = doc.get("sources") {
        let Value::Table(entries) = value else {
            return Err(bad("[sources] is not a table".to_owned()));
        };
        for (doc_key, entry) in entries {
            let place = format!("[sources.{doc_key}]");
            let Value::Table(entry) = entry else {
                return Err(bad(format!("{place} is not a table")));
            };
            only_fields(entry, &["title", "url"], &place).map_err(bad)?;
            sources.insert(
                doc_key.clone(),
                Source {
                    title: string(entry, "title", &place).map_err(bad)?,
                    url: optional_string(entry, "url", &place).map_err(bad)?,
                },
            );
        }
    }

    let mut rules: Vec<Rule> = Vec::new();
    let entries = match doc.get("rules") {
        None => Vec::new(),
        Some(Value::Array(entries)) => entries.clone(),
        Some(_) => return Err(bad("rules is not a list of [[rules]]".to_owned())),
    };
    for (index, entry) in entries.iter().enumerate() {
        let Value::Table(entry) = entry else {
            return Err(bad(format!("rule {} is not a table", index + 1)));
        };
        let rule = parse_rule(id, entry, index)?;
        if rules.iter().any(|r| r.id == rule.id) {
            return Err(bad(format!("rule {} is defined twice", rule.id)));
        }
        if !sources.contains_key(&rule.source.doc) {
            return Err(bad(format!(
                "rule {} cites source {}, which [sources] does not list",
                rule.id, rule.source.doc
            )));
        }
        rules.push(rule);
    }

    Ok(RuleProfile {
        id: id.to_owned(),
        kind,
        base,
        name,
        valid_from,
        sources,
        rules,
    })
}

fn parse_rule(profile: &str, entry: &Table, index: usize) -> Result<Rule, RuleError> {
    let bad = |message: String| RuleError::Malformed {
        profile: profile.to_owned(),
        message,
    };
    let place = format!("rule {}", index + 1);
    let id = string(entry, "id", &place).map_err(bad)?;
    let place = format!("rule {id}");
    only_fields(
        entry,
        &[
            "id",
            "key",
            "severity",
            "enabled",
            "map_kinds",
            "print_scales",
            "values",
            "source",
            "note",
        ],
        &place,
    )
    .map_err(bad)?;
    let well_formed = id.split('.').count() >= 3
        && id.split('.').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        });
    if !well_formed {
        return Err(bad(format!(
            "rule id {id} is not of the form <profile>.<family>.<name> in lower case"
        )));
    }
    let key = string(entry, "key", &place).map_err(bad)?;
    let Some((_, known_values)) = KNOWN_KEYS.iter().find(|(k, _)| *k == key) else {
        return Err(RuleError::UnknownKey {
            profile: profile.to_owned(),
            rule: id,
            key,
        });
    };
    let severity_text = string(entry, "severity", &place).map_err(bad)?;
    let severity = Severity::parse(&severity_text).ok_or_else(|| {
        bad(format!(
            "{place}: severity {severity_text} is not error, warning or hint"
        ))
    })?;
    let enabled = match entry.get("enabled") {
        None => true,
        Some(Value::Boolean(enabled)) => *enabled,
        Some(_) => return Err(bad(format!("{place}: enabled is not true or false"))),
    };
    let map_kinds = strings(entry, "map_kinds", &place)
        .map_err(bad)?
        .iter()
        .map(|kind| {
            MapKind::ALL
                .into_iter()
                .find(|k| k.name() == kind)
                .ok_or_else(|| bad(format!("{place}: unknown map kind {kind}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let print_scales = match entry.get("print_scales") {
        None => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| match item {
                Value::Integer(n) if *n > 0 && *n <= i64::from(u32::MAX) => Ok(*n as u32),
                other => Err(bad(format!("{place}: print scale {other} is no scale"))),
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => return Err(bad(format!("{place}: print_scales is not a list"))),
    };
    if KIND_INDEPENDENT_KEYS.contains(&key.as_str())
        && (!map_kinds.is_empty() || !print_scales.is_empty())
    {
        return Err(bad(format!(
            "{place}: {key} holds for every map kind and takes no filter"
        )));
    }
    let values = match entry.get("values") {
        None => Table::new(),
        Some(Value::Table(values)) => values.clone(),
        Some(_) => return Err(bad(format!("{place}: values is not a table"))),
    };
    let source = match entry.get("source") {
        Some(Value::Table(source)) => {
            let where_ = format!("{place} source");
            only_fields(source, &["doc", "section"], &where_).map_err(bad)?;
            SourceRef {
                doc: string(source, "doc", &where_).map_err(bad)?,
                section: string(source, "section", &where_).map_err(bad)?,
            }
        }
        _ => return Err(bad(format!("{place} has no source = {{ doc, section }}"))),
    };
    let note = match entry.get("note") {
        None => None,
        value => Some(localized(value, &format!("{place} note")).map_err(bad)?),
    };
    let rule = Rule {
        id,
        key,
        severity,
        enabled,
        map_kinds,
        print_scales,
        values,
        source,
        note,
    };
    let problems = value_name_problems(&rule);
    if !problems.is_empty() {
        return Err(bad(format!(
            "rule {}: value names without a unit: {}",
            rule.id,
            problems.join(", ")
        )));
    }
    let unread: Vec<&str> = rule
        .values
        .keys()
        .map(String::as_str)
        .filter(|name| !known_values.contains(name))
        .collect();
    if !unread.is_empty() {
        return Err(bad(format!(
            "rule {}: {} does not read {}; it reads {}",
            rule.id,
            rule.key,
            unread.join(", "),
            known_values.join(", ")
        )));
    }
    for (name, value) in &rule.values {
        value_problem(&rule.key, name, value)
            .map_err(|message| bad(format!("rule {}: {name}: {message}", rule.id)))?;
    }
    Ok(rule)
}

/// Checks one value of a rule: a documented table has its shape, a number its range by unit
/// (`mm` and `metres` above 0, `factor` 0 or more, `percent` from 0 to 100, `degrees` any finite number;
/// `description_box` `cell_mm` from 4 to 10, as the project file's `cell_mm`).
fn value_problem(key: &str, name: &str, value: &Value) -> Result<(), String> {
    if key == "scale_table" && name == "rows" {
        return scale_rows(value).map(|_| ());
    }
    let Some(n) = number(value).filter(|n| n.is_finite()) else {
        return Err("is not a number".to_owned());
    };
    // The description cell has the range the project file allows for `cell_mm`.
    if key == "description_box" && name == "cell_mm" {
        use crate::project::{MAX_CELL_MM, MIN_CELL_MM};
        return if (MIN_CELL_MM..=MAX_CELL_MM).contains(&n) {
            Ok(())
        } else {
            Err(format!(
                "{n} is out of range; a description cell is {MIN_CELL_MM} to {MAX_CELL_MM} mm"
            ))
        };
    }
    let unit = UNITS
        .iter()
        .find(|unit| name == **unit || name.ends_with(&format!("_{unit}")))
        .copied()
        .unwrap_or("");
    let fits = match unit {
        "percent" => (0.0..=100.0).contains(&n),
        "factor" => n >= 0.0,
        // An angle may point anywhere.
        "degrees" => true,
        _ => n > 0.0,
    };
    if fits {
        Ok(())
    } else {
        Err(format!("{n} is out of range for a value in {unit}"))
    }
}

/// Value names of `rule` that carry no unit and are no documented table.
///
/// Text, true/false and lists of texts need no unit. A number or a list of numbers needs a name
/// that is a unit or ends in `_<unit>`. Anything else must be listed in [`TABLE_VALUES`] for the
/// rule's key.
pub fn value_name_problems(rule: &Rule) -> Vec<String> {
    fn is_text(value: &Value) -> bool {
        match value {
            Value::String(_) | Value::Boolean(_) | Value::Datetime(_) => true,
            Value::Array(items) => items.iter().all(is_text),
            _ => false,
        }
    }
    fn is_number(value: &Value) -> bool {
        match value {
            Value::Integer(_) | Value::Float(_) => true,
            Value::Array(items) => items
                .iter()
                .all(|v| matches!(v, Value::Integer(_) | Value::Float(_))),
            _ => false,
        }
    }
    let has_unit = |name: &str| {
        UNITS
            .iter()
            .any(|unit| name == *unit || name.ends_with(&format!("_{unit}")))
    };
    let documented = |name: &str| {
        TABLE_VALUES
            .iter()
            .any(|(key, value, _)| *key == rule.key && *value == name)
    };
    rule.values
        .iter()
        .filter(|(name, value)| {
            !(is_text(value) || (is_number(value) && has_unit(name)) || documented(name))
        })
        .map(|(name, _)| name.clone())
        .collect()
}

fn only_fields(table: &Table, allowed: &[&str], place: &str) -> Result<(), String> {
    match table.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(unknown) => Err(format!("{place} has the unknown field {unknown}")),
        None => Ok(()),
    }
}

fn table<'a>(table: &'a Table, name: &str, place: &str) -> Result<&'a Table, String> {
    match table.get(name) {
        Some(Value::Table(inner)) => Ok(inner),
        Some(_) => Err(format!("{name} in {place} is not a table")),
        None => Err(format!("{place} has no [{name}]")),
    }
}

fn string(table: &Table, name: &str, place: &str) -> Result<String, String> {
    optional_string(table, name, place)?.ok_or_else(|| format!("{place} has no {name}"))
}

fn optional_string(table: &Table, name: &str, place: &str) -> Result<Option<String>, String> {
    match table.get(name) {
        None => Ok(None),
        Some(Value::String(text)) if !text.trim().is_empty() => Ok(Some(text.clone())),
        Some(_) => Err(format!("{name} in {place} is not a text")),
    }
}

fn strings(table: &Table, name: &str, place: &str) -> Result<Vec<String>, String> {
    match table.get(name) {
        None => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| match item {
                Value::String(text) => Ok(text.clone()),
                _ => Err(format!("{name} in {place} is not a list of texts")),
            })
            .collect(),
        Some(_) => Err(format!("{name} in {place} is not a list")),
    }
}

fn localized(value: Option<&Value>, place: &str) -> Result<LocalizedText, String> {
    let Some(Value::Table(texts)) = value else {
        return Err(format!("{place} is not {{ en = \"...\", de = \"...\" }}"));
    };
    let mut map = BTreeMap::new();
    for (lang, text) in texts {
        match text {
            Value::String(text) if !text.trim().is_empty() => {
                map.insert(lang.clone(), text.clone());
            }
            _ => return Err(format!("{place}: the {lang} text is empty or no text")),
        }
    }
    if !map.contains_key("en") {
        return Err(format!("{place} has no English text (en)"));
    }
    Ok(LocalizedText(map))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test-only overlay for the embedded base profile: replaces the sprint north-line rule and adds
    /// a rule of its own.
    const REPLACE: &str = r#"
    format = 1

    [profile]
    id = "test-replace"
    kind = "overlay"
    base = "iof-print"
    name = { de = "Test", en = "Test" }

    [sources.REG]
    title = "Regional print rules"

    [[rules]]
    id = "iofprint.north_lines.sprint"
    key = "north_lines"
    severity = "hint"
    map_kinds = ["sprint"]
    values = { spacing_metres = 100.0 }
    source = { doc = "REG", section = "§ 4" }
    "#;

    /// A test-only overlay that switches the sprint control circle rule off and gives its own.
    const SWITCH_OFF: &str = r#"
    format = 1

    [profile]
    id = "test-off"
    kind = "overlay"
    base = "iof-print"
    name = { en = "Switch off" }

    [sources.REG]
    title = "Regional print rules"

    [[rules]]
    id = "iofprint.course.control_sprint"
    key = "course_symbol"
    severity = "warning"
    enabled = false
    map_kinds = ["sprint"]
    values = { circle_diameter_mm = 6.0 }
    source = { doc = "REG", section = "§ 6" }

    [[rules]]
    id = "testoff.course.control_sprint"
    key = "course_symbol"
    severity = "warning"
    map_kinds = ["sprint"]
    values = { circle_diameter_mm = 5.5 }
    source = { doc = "REG", section = "§ 6" }
    "#;

    /// A test-only overlay that switches a rule off without a replacement: the set lacks a value.
    const LEAVES_A_GAP: &str = r#"
    format = 1

    [profile]
    id = "test-gap"
    kind = "overlay"
    base = "iof-print"
    name = { en = "Gap" }

    [sources.REG]
    title = "Regional print rules"

    [[rules]]
    id = "iofprint.course.line_forest"
    key = "course_symbol"
    severity = "warning"
    enabled = false
    map_kinds = ["forest"]
    values = { line_width_mm = 0.35 }
    source = { doc = "REG", section = "§ 7" }
    "#;

    fn files() -> Vec<(&'static str, &'static str)> {
        vec![
            ("iof-print", EMBEDDED[0].1),
            ("test-replace", REPLACE),
            ("test-off", SWITCH_OFF),
            ("test-gap", LEAVES_A_GAP),
        ]
    }

    fn s(text: &str) -> String {
        text.to_owned()
    }

    /// A minimal base profile with `rule` as its only rule.
    fn base_with_rule(rule: &str) -> String {
        format!(
            "format = 1\n[profile]\nid = \"base-x\"\nkind = \"base\"\nname = {{ en = \"Base\" }}\n\
             [sources.DOC]\ntitle = \"Some rules\"\n[[rules]]\n{rule}"
        )
    }

    #[test]
    fn base_profile_loads_and_every_value_has_a_unit_and_source() {
        assert_eq!(available(), vec!["iof-print"]);
        let profile = profile("iof-print").unwrap();
        assert_eq!(profile.kind, ProfileKind::Base);
        for lang in ["de", "en"] {
            assert!(profile.name.0.contains_key(lang), "name has no {lang}");
        }
        assert!(!profile.rules.is_empty());
        for rule in &profile.rules {
            assert!(rule.id.starts_with("iofprint."), "{}", rule.id);
            assert!(
                KNOWN_KEYS.iter().any(|(key, _)| *key == rule.key),
                "{}",
                rule.id
            );
            assert_eq!(
                value_name_problems(rule),
                Vec::<String>::new(),
                "{}",
                rule.id
            );
            assert!(!rule.values.is_empty(), "{} has no values", rule.id);
            assert!(
                profile.sources.contains_key(&rule.source.doc),
                "{}: source {} not listed",
                rule.id,
                rule.source.doc
            );
            assert!(!rule.source.section.trim().is_empty(), "{}", rule.id);
            if rule.source.doc == PROPOSAL_DOC {
                assert!(
                    rule.note.is_some(),
                    "{}: our own call needs a note",
                    rule.id
                );
            }
            if let Some(note) = &rule.note {
                for lang in ["de", "en"] {
                    assert!(note.0.contains_key(lang), "{}: note has no {lang}", rule.id);
                }
            }
        }
        let set = base();
        assert_eq!(set.base, "iof-print");
        assert!(set.overlays.is_empty() && set.switched_off.is_empty());
        assert!(set.rules.iter().all(|r| set.source(r).is_some()));
        // Every key the program reads has a rule.
        for (key, _) in KNOWN_KEYS {
            assert!(set.rules.iter().any(|r| r.rule.key == *key), "{key}");
        }
        assert_eq!(set.purple_cmyk(), [0.35, 0.85, 0.0, 0.0]);
        assert_eq!(
            set.placement_weights(),
            PlacementWeights {
                black: 10.0,
                brown: 4.0,
                blue: 4.0,
                other: 1.0
            }
        );
        for kind in MapKind::ALL {
            assert_eq!(set.description_cell_mm(kind), 6.0);
        }
        assert_eq!(set.north_line_spacing_m(MapKind::Sprint), 120.0);
        assert_eq!(set.north_line_spacing_m(MapKind::Forest), 300.0);
        assert_eq!(set.north_line_spacing_m(MapKind::Schoolyard), 37.5);
        assert_eq!(set.proposed_contour_interval_m(MapKind::Sprint), Some(2.0));
        assert_eq!(set.proposed_contour_interval_m(MapKind::Forest), Some(5.0));
        assert_eq!(set.proposed_contour_interval_m(MapKind::Schoolyard), None);
    }

    #[test]
    fn scale_table_matches_adr_0018() {
        let set = base();
        assert_eq!(
            set.scale_table(MapKind::Sprint),
            [(3000, 4.0 / 3.0), (4000, 1.0), (5000, 1.0)]
        );
        assert_eq!(
            set.scale_table(MapKind::Forest),
            [
                (5000, 3.0),
                (7500, 2.0),
                (10000, 1.5),
                (15000, 1.0),
                (20000, 1.0)
            ]
        );
        assert_eq!(set.scale_table(MapKind::Schoolyard), [(1000, 1.0)]);
        for (kind, scale, standard) in [
            (MapKind::Sprint, 3000, true),
            (MapKind::Sprint, 4000, true),
            (MapKind::Sprint, 5000, false),
            (MapKind::Schoolyard, 1000, true),
            (MapKind::Forest, 5000, true),
            (MapKind::Forest, 15000, true),
            (MapKind::Forest, 20000, false),
            (MapKind::Forest, 4000, false),
        ] {
            assert_eq!(
                set.is_standard_scale(kind, scale),
                standard,
                "{kind:?} {scale}"
            );
        }
        // The base scale of each kind is in its table at 100 %.
        for kind in MapKind::ALL {
            assert!(
                set.scale_table(kind).contains(&(kind.base_scale(), 1.0)),
                "{kind:?}"
            );
            assert!(set.is_standard_scale(kind, kind.base_scale()));
        }
    }

    #[test]
    fn degrees_take_any_finite_number() {
        let check = |name: &str, value: Value| value_problem("course_symbol", name, &value);
        let angle = "out_of_bounds_hatch_angle_degrees";
        assert!(check(angle, Value::Integer(0)).is_ok());
        assert!(check(angle, Value::Integer(-45)).is_ok());
        assert!(check(angle, Value::Float(400.0)).is_ok());
        assert!(check(angle, Value::Float(f64::NAN)).is_err());
        assert!(check(angle, Value::Float(f64::INFINITY)).is_err());
        // Lengths stay above 0.
        assert!(check("first_aid_mm", Value::Integer(0)).is_err());
    }

    #[test]
    fn description_cell_is_bounded() {
        let check = |value: Value| value_problem("description_box", "cell_mm", &value);
        assert!(check(Value::Integer(4)).is_ok());
        assert!(check(Value::Float(6.0)).is_ok());
        assert!(check(Value::Integer(10)).is_ok());
        assert!(check(Value::Float(3.9)).is_err());
        assert!(check(Value::Float(10.5)).is_err());
        assert!(check(Value::Float(f64::NAN)).is_err());
    }

    #[test]
    fn sprint_course_symbols() {
        // ISSprOM 2019-2 section 4.7, at 1:4000; Purple Pen's Spr2019 sizes agree.
        assert_eq!(
            base().course_symbol(MapKind::Sprint),
            CourseSymbolSizes {
                circle_diameter_mm: 6.0,
                line_width_mm: 0.35,
                start_side_mm: 7.0,
                finish_inner_mm: 5.0,
                finish_outer_mm: 7.0,
                number_height_mm: 4.0,
                marked_route_dash_mm: 2.0,
                marked_route_gap_mm: 0.5,
                out_of_bounds_line_mm: 1.0,
                crossing_point_mm: 3.0,
                out_of_bounds_hatch_angle_degrees: 45.0,
                out_of_bounds_hatch_second_angle_degrees: Some(135.0),
                out_of_bounds_hatch_spacing_mm: 1.2,
                out_of_bounds_hatch_line_mm: 0.2,
                first_aid_mm: 4.0,
                first_aid_bar_mm: 1.33,
                refreshment_mm: 3.5,
                refreshment_line_mm: 0.4,
            }
        );
        // Schoolyard maps take the sprint values (an assumption the rule's note states).
        assert_eq!(
            base().course_symbol(MapKind::Schoolyard),
            base().course_symbol(MapKind::Sprint)
        );
        // The assumption is our call, so the note check of PROPOSAL_DOC applies.
        let schoolyard = &base().rule("iofprint.course.schoolyard").unwrap().rule;
        assert_eq!(schoolyard.source.doc, PROPOSAL_DOC);
        assert!(schoolyard.note.is_some());
    }

    #[test]
    fn forest_course_symbols() {
        // ISOM 2017-2 section 3.7, at 1:15000; Purple Pen's ISOM 2017 sizes agree.
        assert_eq!(
            base().course_symbol(MapKind::Forest),
            CourseSymbolSizes {
                circle_diameter_mm: 5.0,
                line_width_mm: 0.35,
                start_side_mm: 6.0,
                finish_inner_mm: 4.0,
                finish_outer_mm: 6.0,
                number_height_mm: 4.0,
                marked_route_dash_mm: 2.0,
                marked_route_gap_mm: 0.5,
                out_of_bounds_line_mm: 0.7,
                crossing_point_mm: 3.0,
                out_of_bounds_hatch_angle_degrees: 45.0,
                out_of_bounds_hatch_second_angle_degrees: Some(135.0),
                out_of_bounds_hatch_spacing_mm: 1.2,
                out_of_bounds_hatch_line_mm: 0.2,
                first_aid_mm: 4.0,
                first_aid_bar_mm: 1.33,
                refreshment_mm: 3.5,
                refreshment_line_mm: 0.4,
            }
        );
    }

    /// An overlay with `rules` (TOML `[[rules]]` entries) for the embedded base.
    fn overlay_with(rules: &str) -> String {
        format!(
            "format = 1\n[profile]\nid = \"test-gaps\"\nkind = \"overlay\"\nbase = \"iof-print\"\n\
             name = {{ en = \"Gaps\" }}\n[sources.REG]\ntitle = \"Regional print rules\"\n{rules}"
        )
    }

    fn gap_rule(id: &str, values: &str) -> String {
        format!(
            "[[rules]]\nid = \"{id}\"\nkey = \"number_gap\"\nseverity = \"warning\"\nvalues = {{ {values} }}\n\
             source = {{ doc = \"REG\", section = \"§ 9\" }}\n"
        )
    }

    fn gap_set(rules: &str) -> Result<RuleSet, RuleError> {
        let overlay = overlay_with(rules);
        let files = [
            ("iof-print", EMBEDDED[0].1),
            ("test-gaps", overlay.as_str()),
        ];
        load_from(&files, "iof-print", &[s("test-gaps")])
    }

    #[test]
    fn number_gap_from_the_profile_strictest_wins() {
        // iof-print: 0.5 to 5 mm (ruling R29).
        assert_eq!(base().number_gap(), Some((0.5, 5.0)));
        let rule = base().rule("iofprint.number.gap").unwrap();
        assert_eq!(rule.rule.severity, Severity::Warning);
        // A second rule: the largest minimum and the smallest maximum.
        let set = gap_set(&gap_rule(
            "reg.number.gap",
            "min_gap_mm = 0.8, max_gap_mm = 8.0",
        ))
        .unwrap();
        assert_eq!(set.number_gap(), Some((0.8, 5.0)));
        let set = gap_set(&gap_rule("reg.number.gap", "max_gap_mm = 2.0")).unwrap();
        assert_eq!(set.number_gap(), Some((0.5, 2.0)));
        // Switched off: no rule, placement takes its fixed gap.
        let off = "[[rules]]\nid = \"iofprint.number.gap\"\nkey = \"number_gap\"\nseverity = \"warning\"\n\
                   enabled = false\nvalues = { min_gap_mm = 0.5 }\nsource = { doc = \"REG\", section = \"§ 9\" }\n";
        assert_eq!(gap_set(off).unwrap().number_gap(), None);
        // Ruling R30, courseplan ADR 0032: a maximum below the minimum is raised to it; a missing side stays
        // open (minimum 0.5, maximum 10); both are held to 0.01..10 and rounded inwards to 0.01 mm.
        let set = gap_set(&gap_rule("reg.number.gap", "min_gap_mm = 6.0")).unwrap();
        assert_eq!(set.number_gap(), Some((6.0, 6.0)));
        let only = |values: &str| {
            gap_set(&format!("{off}{}", gap_rule("reg.number.gap", values)))
                .unwrap()
                .number_gap()
        };
        assert_eq!(only("max_gap_mm = 12.0"), Some((0.5, 10.0)));
        assert_eq!(only("min_gap_mm = 0.8"), Some((0.8, 10.0)));
        assert_eq!(
            only("min_gap_mm = 0.123, max_gap_mm = 3.456"),
            Some((0.13, 3.45))
        );
        assert_eq!(only("min_gap_mm = 0.001"), Some((0.01, 10.0)));
        // A rule with neither value is skipped.
        assert_eq!(only(""), None);
        // Values that are not above 0 or not finite still stop the profile from loading.
        for bad in [
            "min_gap_mm = 0.0",
            "max_gap_mm = -1.0",
            "min_gap_mm = nan",
            "max_gap_mm = inf",
        ] {
            assert!(
                gap_set(&format!("{off}{}", gap_rule("reg.number.gap", bad))).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn overlay_replaces_rule_by_id() {
        let set = load_from(&files(), "iof-print", &[s("test-replace")]).unwrap();
        let rule = set.rule("iofprint.north_lines.sprint").unwrap();
        assert_eq!(rule.profile, "test-replace");
        assert_eq!(set.source(rule).unwrap().title, "Regional print rules");
        assert_eq!(set.north_line_spacing_m(MapKind::Sprint), 100.0);
        assert_eq!(set.north_line_spacing_m(MapKind::Forest), 300.0);
        // The replaced rule keeps its place in the set.
        let position = |set: &RuleSet| {
            set.rules
                .iter()
                .position(|r| r.rule.id == "iofprint.north_lines.sprint")
        };
        assert_eq!(position(&set), position(base()));
        assert_eq!(set.overlays, vec![s("test-replace")]);
        assert!(set.switched_off.is_empty());
    }

    #[test]
    fn overlay_disables_rule() {
        let set = load_from(&files(), "iof-print", &[s("test-off")]).unwrap();
        assert!(set.rule("iofprint.course.control_sprint").is_none());
        assert_eq!(set.switched_off.len(), 1);
        let off = &set.switched_off[0];
        assert_eq!(off.rule.id, "iofprint.course.control_sprint");
        assert_eq!(off.profile, "iof-print");
        assert_eq!(
            off.by, "test-off",
            "the set names the profile that switched it off"
        );
        assert_eq!(set.course_symbol(MapKind::Sprint).circle_diameter_mm, 5.5);
        assert_eq!(set.course_symbol(MapKind::Forest).circle_diameter_mm, 5.0);
        // Both overlays in order.
        let both = load_from(&files(), "iof-print", &[s("test-replace"), s("test-off")]).unwrap();
        assert_eq!(both.north_line_spacing_m(MapKind::Sprint), 100.0);
        assert_eq!(both.switched_off[0].by, "test-off");
        // Switching off a rule the base does not have is a mistake in the overlay.
        let wrong = SWITCH_OFF.replace(
            "id = \"iofprint.course.control_sprint\"",
            "id = \"iofprint.course.nope\"",
        );
        let files = [("iof-print", EMBEDDED[0].1), ("test-off", wrong.as_str())];
        assert_eq!(
            load_from(&files, "iof-print", &[s("test-off")]),
            Err(RuleError::UnknownRule {
                overlay: s("test-off"),
                rule: s("iofprint.course.nope"),
            })
        );
    }

    #[test]
    fn scale_table_keeps_the_base_scale() {
        // An overlay whose sprint table drops 1:4000 would make every base-scale project unusable.
        let drops = REPLACE
            .replace("key = \"north_lines\"", "key = \"scale_table\"")
            .replace("iofprint.north_lines.sprint", "iofprint.scale.sprint")
            .replace(
                "values = { spacing_metres = 100.0 }",
                "values = { rows = [{ scale = 3000, size_factor = 1.3333333333333333 }, \
                 { scale = 5000, size_factor = 1.0, standard = false }] }",
            );
        let files = [
            ("iof-print", EMBEDDED[0].1),
            ("test-replace", drops.as_str()),
        ];
        let err = load_from(&files, "iof-print", &[s("test-replace")]).unwrap_err();
        assert!(matches!(err, RuleError::Incomplete { .. }), "{err:?}");
        assert!(err.to_string().contains("scale = 4000"), "{err}");
        // A base-scale row that is not at 100 %, or not standard, is refused too.
        for row in [
            "{ scale = 4000, size_factor = 1.1 }",
            "{ scale = 4000, size_factor = 1.0, standard = false }",
        ] {
            let text = drops.replace("{ scale = 5000, size_factor = 1.0, standard = false }", row);
            let files = [
                ("iof-print", EMBEDDED[0].1),
                ("test-replace", text.as_str()),
            ];
            assert!(
                load_from(&files, "iof-print", &[s("test-replace")]).is_err(),
                "{row}"
            );
        }
        // With the base row the overlay loads.
        let keeps = drops.replace(
            "{ scale = 5000, size_factor = 1.0, standard = false }",
            "{ scale = 4000, size_factor = 1.0 }",
        );
        let files = [
            ("iof-print", EMBEDDED[0].1),
            ("test-replace", keeps.as_str()),
        ];
        let set = load_from(&files, "iof-print", &[s("test-replace")]).unwrap();
        assert_eq!(
            set.scale_table(MapKind::Sprint),
            [(3000, 4.0 / 3.0), (4000, 1.0)]
        );
    }

    #[test]
    fn a_set_without_a_value_is_refused() {
        let err = load_from(&files(), "iof-print", &[s("test-gap")]).unwrap_err();
        assert!(matches!(err, RuleError::Incomplete { .. }), "{err:?}");
        let text = err.to_string();
        assert!(
            text.contains("line_width_mm") && text.contains("forest"),
            "{text}"
        );
        // Two rules that give the same value for a kind are refused too.
        let twice = REPLACE.replace(
            "id = \"iofprint.north_lines.sprint\"",
            "id = \"testreplace.north_lines.sprint\"",
        );
        let files = [
            ("iof-print", EMBEDDED[0].1),
            ("test-replace", twice.as_str()),
        ];
        let err = load_from(&files, "iof-print", &[s("test-replace")]).unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains("iofprint.north_lines.sprint")
                && text.contains("testreplace.north_lines.sprint"),
            "{text}"
        );
    }

    #[test]
    fn unknown_value_key_is_an_error() {
        // A rule key the program does not implement.
        let unknown = base_with_rule(
            "id = \"basex.x.y\"\nkey = \"no_such_rule\"\nseverity = \"hint\"\nsource = { doc = \"DOC\", section = \"1\" }\n",
        );
        assert_eq!(
            parse_profile("base-x", &unknown),
            Err(RuleError::UnknownKey {
                profile: s("base-x"),
                rule: s("basex.x.y"),
                key: s("no_such_rule"),
            })
        );
        for (values, ok) in [
            ("{ spacing_metres = 120.0 }", true),
            // A number without its unit.
            ("{ spacing = 120.0 }", false),
            ("{ spacing_metresx = 120.0 }", false),
            // A value with a unit that the key does not read: a typo, not an extension.
            ("{ spacing_mm = 120.0 }", false),
            ("{ spacing_metres = 120.0, extra_metres = 1.0 }", false),
        ] {
            let text = base_with_rule(&format!(
                "id = \"basex.north.x\"\nkey = \"north_lines\"\nseverity = \"hint\"\nvalues = {values}\nsource = {{ doc = \"DOC\", section = \"1\" }}\n"
            ));
            let result = parse_profile("base-x", &text);
            assert_eq!(result.is_ok(), ok, "{values}: {result:?}");
            if !ok {
                assert!(
                    matches!(result, Err(RuleError::Malformed { .. })),
                    "{values}: {result:?}"
                );
            }
        }
        // A scale table row with an unknown field.
        let row = base_with_rule(
            "id = \"basex.scale.x\"\nkey = \"scale_table\"\nseverity = \"error\"\nvalues = { rows = [{ scale = 4000, size = 1.0 }] }\nsource = { doc = \"DOC\", section = \"1\" }\n",
        );
        assert!(matches!(
            parse_profile("base-x", &row),
            Err(RuleError::Malformed { .. })
        ));
    }

    #[test]
    fn profile_format_follows_courseplan() {
        let good = base_with_rule(
            "id = \"basex.north.x\"\nkey = \"north_lines\"\nseverity = \"hint\"\nvalues = { spacing_metres = 1.0 }\nsource = { doc = \"DOC\", section = \"1\" }\n",
        );
        assert!(parse_profile("base-x", &good).is_ok());
        for text in [
            good.replacen("format = 1", "", 1),
            good.replacen("format = 1", "format = 2", 1),
            good.replacen("format = 1", "format = \"1\"", 1),
            // A typo in a field name.
            good.replacen("severity", "severty", 1),
            good.replacen("key = ", "map_kind = [\"sprint\"]\nkey = ", 1),
            // Disciplines are courseplan's filter, not a print one.
            good.replacen("key = ", "disciplines = [\"sprint\"]\nkey = ", 1),
            // An unknown map kind.
            good.replacen("key = ", "map_kinds = [\"moon\"]\nkey = ", 1),
            // A source that [sources] does not list, and no source at all.
            good.replacen("doc = \"DOC\"", "doc = \"NOPE\"", 1),
            good.replacen("source = { doc = \"DOC\", section = \"1\" }", "", 1),
            // An id that is not <profile>.<family>.<name> in lower case.
            good.replacen("basex.north.x", "basex.North", 1),
            good.replacen("severity = \"hint\"", "severity = \"fatal\"", 1),
        ] {
            assert!(
                matches!(
                    parse_profile("base-x", &text),
                    Err(RuleError::Malformed { .. })
                ),
                "{text}"
            );
        }
        // The id in the file must be the profile id it is embedded under.
        assert!(matches!(
            parse_profile("base-y", &good),
            Err(RuleError::Malformed { .. })
        ));
    }

    #[test]
    fn unknown_profile_is_refused() {
        let expected = |id: &str| {
            Err(RuleError::UnknownProfile {
                id: s(id),
                available: s("iof-print"),
            })
        };
        assert_eq!(load_rule_set("iof-2030", &[]), expected("iof-2030"));
        assert_eq!(load_rule_set("iof-print", &[s("nope")]), expected("nope"));
        assert_eq!(
            load_from(&files(), "test-off", &[]),
            Err(RuleError::NotABase(s("test-off")))
        );
        assert_eq!(
            load_from(&files(), "iof-print", &[s("iof-print")]),
            Err(RuleError::NotAnOverlay(s("iof-print")))
        );
        let foreign = REPLACE.replace("base = \"iof-print\"", "base = \"other\"");
        let files = [
            ("iof-print", EMBEDDED[0].1),
            ("test-replace", foreign.as_str()),
        ];
        assert_eq!(
            load_from(&files, "iof-print", &[s("test-replace")]),
            Err(RuleError::OverlayForAnotherBase {
                overlay: s("test-replace"),
                extends: s("other"),
                base: s("iof-print"),
            })
        );
    }

    #[test]
    fn print_scale_filter_is_read_at_the_base_scale() {
        // Values hold at the base scale of the kind, so a print_scales filter selects by it.
        let overlay = REPLACE.replace(
            "map_kinds = [\"sprint\"]",
            "map_kinds = [\"sprint\"]\nprint_scales = [3000]",
        );
        let files = [
            ("iof-print", EMBEDDED[0].1),
            ("test-replace", overlay.as_str()),
        ];
        let err = load_from(&files, "iof-print", &[s("test-replace")]).unwrap_err();
        assert!(
            err.to_string().contains("spacing_metres"),
            "the replaced rule no longer applies at 1:4000: {err}"
        );
        let overlay = REPLACE.replace(
            "map_kinds = [\"sprint\"]",
            "map_kinds = [\"sprint\"]\nprint_scales = [4000]",
        );
        let files = [
            ("iof-print", EMBEDDED[0].1),
            ("test-replace", overlay.as_str()),
        ];
        let set = load_from(&files, "iof-print", &[s("test-replace")]).unwrap();
        assert_eq!(set.north_line_spacing_m(MapKind::Sprint), 100.0);
        let rule = &set.rule("iofprint.north_lines.sprint").unwrap().rule;
        assert!(rule.applies(MapKind::Sprint, 4000));
        assert!(!rule.applies(MapKind::Sprint, 3000));
        assert!(!rule.applies(MapKind::Forest, 4000));
    }
}
