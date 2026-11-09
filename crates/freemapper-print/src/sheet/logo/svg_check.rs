//! The allow-list for SVG logos (ADR 0026, limits in ADR 0019).
//!
//! `usvg` copies whole subtrees while it resolves an SVG: `use`, markers, patterns, masks, clip
//! paths in bounding-box units, `feImage`. A few kilobytes can grow into gigabytes before any
//! limit on the drawn paths applies. Print logos need plain shapes only, so the XML is checked
//! before `usvg` sees it: only the elements and references below are allowed, every reference
//! is resolved the way `usvg` resolves it, and the copies that remain (`use`, clip paths) are
//! counted.

use std::collections::{HashMap, HashSet};

use usvg::roxmltree::{self, Node, NodeId};

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

/// Most work of an SVG logo once `usvg` has resolved it: one unit per element plus one per two
/// bytes of path data, counted again for every `use` and clip path copy, and gradient stops
/// counted once per shape.
const MAX_WORK: u64 = 1_000_000;
/// Deepest SVG nesting, `use` resolved.
const MAX_DEPTH: u64 = 256;
/// Longest chain of gradients that take their stops from another.
const MAX_GRADIENT_LINKS: usize = 16;
/// Most stops of a gradient, its own or those it takes from another. `usvg` sorts stops with
/// equal offsets in time that grows with the square of their number.
const MAX_STOPS: u64 = 256;
/// Most attributes of one element, and most of the squares of all elements' attribute counts:
/// the XML parser compares each attribute of an element with the others.
const MAX_ATTRIBUTES: u64 = 1000;
const MAX_ATTRIBUTE_PAIRS: u64 = 50_000_000;
/// Most namespace declarations (`xmlns`, `xmlns:*`) in one file: the XML parser gives every
/// element that declares one a copy of all namespaces in scope.
const MAX_NAMESPACES: u64 = 64;
/// Longest text of an entity of the internal subset.
const MAX_ENTITY_BYTES: usize = 1024;

/// The SVG elements a logo may use. `symbol` only takes effect through `use` and is counted with
/// it.
const ALLOWED: &[&str] = &[
    "svg",
    "g",
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "defs",
    "linearGradient",
    "radialGradient",
    "stop",
    "title",
    "desc",
    "metadata",
    "use",
    "clipPath",
    "symbol",
];

const SHAPES: &[&str] = &[
    "path", "rect", "circle", "ellipse", "line", "polyline", "polygon",
];

/// Properties that take effects the sheet does not draw. Only `none` is allowed.
const EFFECTS: &[&str] = &[
    "filter",
    "mask",
    "marker",
    "marker-start",
    "marker-mid",
    "marker-end",
];

fn unsupported(what: &str) -> String {
    format!(
        "{what} is not supported in logos; export the logo as a plain SVG (shapes, paths, gradients)"
    )
}

/// An element as `usvg` reads it: in the SVG namespace or in none. `usvg` skips other
/// elements (Inkscape's `sodipodi:namedview` and the like) with everything inside them.
fn is_svg(node: Node<'_, '_>) -> bool {
    node.is_element() && matches!(node.tag_name().namespace(), None | Some(SVG_NS))
}

fn name<'a>(node: Node<'a, '_>) -> &'a str {
    node.tag_name().name()
}

/// Whether `usvg` reads an attribute of this namespace (`parse_svg_element`).
fn read_namespace(ns: Option<&str>) -> bool {
    matches!(ns, None | Some(SVG_NS) | Some(XLINK_NS) | Some(XML_NS))
}

/// Namespaces of drawing programs whose attributes `usvg` passes over: Inkscape, Sodipodi and
/// Adobe Illustrator.
fn editor_namespace(ns: &str) -> bool {
    ns == "http://www.inkscape.org/namespaces/inkscape"
        || ns == "http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd"
        || ns.starts_with("http://ns.adobe.com/")
}

/// Attributes that `usvg` looks up by local name in every namespace (`node.attribute(..)` in its
/// parser), so they must not appear in a namespace it otherwise passes over.
const READ_IN_ANY_NAMESPACE: &[&str] = &["id", "style"];

/// An attribute as `usvg`'s `parse_svg_element` reads it: by local name, in the namespaces it
/// reads. [`check_attributes`] has made sure that each local name occurs once per element.
fn attr<'a>(node: Node<'a, '_>, local: &str) -> Option<&'a str> {
    node.attributes()
        .find(|a| a.name() == local && read_namespace(a.namespace()))
        .map(|a| a.value())
}

/// The id `usvg` gives an element for `use`: the first attribute with the local name `id`, in any
/// namespace, as `roxmltree::Node::attribute("id")` finds it.
fn use_id<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    node.attributes()
        .find(|a| a.name() == "id")
        .map(|a| a.value())
}

/// Local names of attributes that `usvg` or this check reads: geometry, links, styles and every
/// presentation attribute. Two attributes of such a name on one element are refused, in any
/// namespaces.
fn read_name(name: &str) -> bool {
    matches!(
        name,
        "id" | "d"
            | "points"
            | "x"
            | "y"
            | "width"
            | "height"
            | "r"
            | "rx"
            | "ry"
            | "cx"
            | "cy"
            | "fx"
            | "fy"
            | "fr"
            | "x1"
            | "y1"
            | "x2"
            | "y2"
            | "href"
            | "style"
            | "class"
            | "space"
            | "lang"
            | "viewBox"
            | "preserveAspectRatio"
            | "clipPathUnits"
            | "offset"
            | "gradientUnits"
            | "gradientTransform"
            | "spreadMethod"
            | "requiredFeatures"
            | "requiredExtensions"
            | "systemLanguage"
            // Presentation attributes.
            | "alignment-baseline"
            | "baseline-shift"
            | "clip"
            | "clip-path"
            | "clip-rule"
            | "color"
            | "color-interpolation"
            | "color-interpolation-filters"
            | "color-profile"
            | "color-rendering"
            | "cursor"
            | "direction"
            | "display"
            | "dominant-baseline"
            | "enable-background"
            | "fill"
            | "fill-opacity"
            | "fill-rule"
            | "filter"
            | "flood-color"
            | "flood-opacity"
            | "font"
            | "font-family"
            | "font-feature-settings"
            | "font-kerning"
            | "font-size"
            | "font-size-adjust"
            | "font-stretch"
            | "font-style"
            | "font-synthesis"
            | "font-variant"
            | "font-variant-caps"
            | "font-variant-east-asian"
            | "font-variant-ligatures"
            | "font-variant-numeric"
            | "font-variant-position"
            | "font-weight"
            | "glyph-orientation-horizontal"
            | "glyph-orientation-vertical"
            | "image-rendering"
            | "inline-size"
            | "isolation"
            | "kerning"
            | "letter-spacing"
            | "lighting-color"
            | "marker"
            | "marker-start"
            | "marker-mid"
            | "marker-end"
            | "mask"
            | "mask-border"
            | "mask-border-mode"
            | "mask-border-outset"
            | "mask-border-repeat"
            | "mask-border-slice"
            | "mask-border-source"
            | "mask-border-width"
            | "mask-clip"
            | "mask-composite"
            | "mask-image"
            | "mask-mode"
            | "mask-origin"
            | "mask-position"
            | "mask-repeat"
            | "mask-size"
            | "mask-type"
            | "mix-blend-mode"
            | "opacity"
            | "overflow"
            | "paint-order"
            | "pointer-events"
            | "shape-image-threshold"
            | "shape-inside"
            | "shape-margin"
            | "shape-padding"
            | "shape-rendering"
            | "shape-subtract"
            | "stop-color"
            | "stop-opacity"
            | "stroke"
            | "stroke-dasharray"
            | "stroke-dashoffset"
            | "stroke-linecap"
            | "stroke-linejoin"
            | "stroke-miterlimit"
            | "stroke-opacity"
            | "stroke-width"
            | "text-anchor"
            | "text-decoration"
            | "text-decoration-color"
            | "text-decoration-fill"
            | "text-decoration-line"
            | "text-decoration-stroke"
            | "text-decoration-style"
            | "text-orientation"
            | "text-overflow"
            | "text-rendering"
            | "transform"
            | "transform-box"
            | "transform-origin"
            | "unicode-bidi"
            | "vector-effect"
            | "visibility"
            | "white-space"
            | "word-spacing"
            | "writing-mode"
    )
}

/// Refuses attributes that `usvg` and this check could read differently: two attributes of one
/// local name on an element when [`read_name`] holds for it or both are in namespaces `usvg`
/// reads; attributes of foreign namespaces other than those of drawing programs; and `id` or
/// `style` in a drawing program's namespace, which `usvg` reads as if they had none. Names that
/// nothing reads may repeat in a drawing program's namespace: Inkscape writes `version` and
/// `inkscape:version` on the root.
fn check_attributes(node: Node<'_, '_>) -> Result<(), String> {
    let mut seen = HashSet::new();
    let mut seen_read = HashSet::new();
    for a in node.attributes() {
        let twice = !seen.insert(a.name());
        let twice_read = read_namespace(a.namespace()) && !seen_read.insert(a.name());
        if ((twice && read_name(a.name())) || twice_read)
            && !(a.name() == "href" && same_hrefs(node))
        {
            return Err(unsupported(&format!(
                "an element with two \"{}\" attributes",
                a.name()
            )));
        }
    }
    for a in node.attributes() {
        let Some(ns) = a.namespace() else {
            continue;
        };
        if read_namespace(Some(ns)) {
            continue;
        }
        if !editor_namespace(ns) || READ_IN_ANY_NAMESPACE.contains(&a.name()) {
            return Err(unsupported(&format!(
                "the attribute {} in the namespace {ns}",
                a.name()
            )));
        }
    }
    Ok(())
}

/// The link of `href` as `usvg` takes it: the unprefixed `href` first, then `xlink:href`,
/// parsed as an IRI (`#id`, spaces around it allowed).
fn href<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    let value = node
        .attributes()
        .find(|a| a.name() == "href" && a.namespace().is_none())
        .or_else(|| {
            node.attributes()
                .find(|a| a.name() == "href" && a.namespace() == Some(XLINK_NS))
        })?
        .value();
    svgtypes::IRI::from_str(value).ok().map(|iri| iri.0)
}

/// Whether the element has exactly an unprefixed `href` and an `xlink:href` that point to the same
/// id. `usvg` follows the unprefixed one, so with equal links nothing depends on which.
fn same_hrefs(node: Node<'_, '_>) -> bool {
    let hrefs: Vec<_> = node.attributes().filter(|a| a.name() == "href").collect();
    let link = |ns: Option<&str>| {
        hrefs
            .iter()
            .find(|a| a.namespace() == ns)
            .and_then(|a| svgtypes::IRI::from_str(a.value()).ok())
            .map(|iri| iri.0)
    };
    hrefs.len() == 2 && link(None).is_some() && link(None) == link(Some(XLINK_NS))
}

/// Every property of an element as `usvg` sees it: the attributes of the namespaces it reads and
/// the declarations of the `style` attribute, split by the same CSS tokenizer.
fn properties<'a>(node: Node<'a, '_>) -> Vec<(&'a str, &'a str)> {
    let mut all: Vec<(&str, &str)> = node
        .attributes()
        .filter(|a| read_namespace(a.namespace()) && a.name() != "style")
        .map(|a| (a.name(), a.value()))
        .collect();
    // `usvg` takes the first `style` in any namespace; there is at most one.
    if let Some(style) = node
        .attributes()
        .find(|a| a.name() == "style")
        .map(|a| a.value())
    {
        all.extend(simplecss::DeclarationTokenizer::from(style).map(|d| (d.name, d.value)));
    }
    all
}

/// The SVG with ids resolved: what each element refers to.
struct Links<'a, 'input> {
    ids: HashMap<&'a str, Node<'a, 'input>>,
}

impl<'a, 'input> Links<'a, 'input> {
    fn get(&self, id: &str) -> Option<Node<'a, 'input>> {
        self.ids.get(id).copied()
    }

    /// The clip path a `clip-path` value refers to, if any.
    fn clip_path(&self, value: &str) -> Option<Node<'a, 'input>> {
        let id = svgtypes::FuncIRI::from_str(value).ok()?.0;
        self.get(id)
    }

    /// The clip path of an element, from its attribute or its `style`. [`check_clip_property`]
    /// has made sure that an element sets `clip-path` at most once.
    fn clip_of(&self, node: Node<'a, 'input>) -> Option<Node<'a, 'input>> {
        properties(node)
            .into_iter()
            .filter(|(n, _)| *n == "clip-path")
            .find_map(|(_, v)| self.clip_path(v))
    }
}

/// An entity of the internal subset: the length of its text and how often the document uses it.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Entity {
    pub(super) len: u64,
    pub(super) uses: u64,
}

/// What [`pre_scan`] found: the entities of the internal subset by name, with their uses.
#[derive(Debug, Default)]
pub(super) struct Scan<'a> {
    pub(super) entities: HashMap<&'a [u8], Entity>,
}

impl Scan<'_> {
    /// Bytes of text the XML parser makes from entity references. An entity holds neither markup
    /// nor references, so each reference brings in exactly the entity's text.
    pub(super) fn entity_text(&self) -> u64 {
        self.entities.values().fold(0u64, |sum, e| {
            sum.saturating_add(e.len.saturating_mul(e.uses))
        })
    }
}

/// White space as XML defines it.
fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}

/// The bytes of an SVG, read the way the XML parser (`roxmltree`) reads them.
struct Scanner<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Scanner<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn starts_with(&self, pattern: &[u8]) -> bool {
        self.bytes[self.at..].starts_with(pattern)
    }

    fn malformed(&self) -> String {
        format!("cannot read the SVG (malformed XML at byte {})", self.at)
    }

    /// Skips white space; whether there was any.
    fn skip_space(&mut self) -> bool {
        let from = self.at;
        while self.peek().is_some_and(is_space) {
            self.at += 1;
        }
        self.at > from
    }

    fn require_space(&mut self) -> Result<(), String> {
        if self.skip_space() {
            Ok(())
        } else {
            Err(self.malformed())
        }
    }

    fn require(&mut self, b: u8) -> Result<(), String> {
        if self.peek() == Some(b) {
            self.at += 1;
            Ok(())
        } else {
            Err(self.malformed())
        }
    }

    /// Moves past the first `pattern` from here; `None` if there is none.
    fn skip_past(&mut self, pattern: &[u8]) -> Option<()> {
        let at = self.bytes[self.at..]
            .windows(pattern.len())
            .position(|w| w == pattern)?;
        self.at += at + pattern.len();
        Some(())
    }

    /// The bytes up to the first one for which `stop` holds, or to the end.
    fn take_until(&mut self, stop: impl Fn(u8) -> bool) -> &'a [u8] {
        let from = self.at;
        while self.peek().is_some_and(|b| !stop(b)) {
            self.at += 1;
        }
        &self.bytes[from..self.at]
    }

    /// A quoted literal; returns its text.
    fn literal(&mut self) -> Result<&'a [u8], String> {
        let quote = match self.peek() {
            Some(q @ (b'"' | b'\'')) => q,
            _ => return Err(self.malformed()),
        };
        self.at += 1;
        let text = self.take_until(|b| b == quote);
        self.require(quote)?;
        Ok(text)
    }

    /// An entity reference at `&`: counts a use of the entity it names. Character references
    /// (`&#…;`) and names the subset does not declare are passed over; the XML parser reads
    /// the latter as one of the five predefined entities or refuses the file.
    fn reference(&mut self, entities: &mut HashMap<&'a [u8], Entity>) {
        self.at += 1;
        if self.peek() == Some(b'#') {
            return;
        }
        let name = self
            .take_until(|b| is_space(b) || matches!(b, b';' | b'<' | b'&' | b'"' | b'\'' | b'>'));
        if self.peek() == Some(b';')
            && let Some(entity) = entities.get_mut(name)
        {
            entity.uses += 1;
        }
    }

    /// The XML declaration after `<?xml `, up to its `?>`.
    fn xml_declaration(&mut self) -> Result<(), String> {
        loop {
            match self.peek() {
                None => return Err(self.malformed()),
                Some(b'"' | b'\'') => {
                    let value = self.literal()?;
                    if value.contains(&b'<') {
                        return Err(self.malformed());
                    }
                }
                Some(b'?') if self.starts_with(b"?>") => {
                    self.at += 2;
                    return Ok(());
                }
                Some(_) => self.at += 1,
            }
        }
    }

    /// The document type declaration after `<!DOCTYPE`, by the strict grammar of ADR 0026: a
    /// name, an optional external ID (never loaded), and an internal subset of comments and
    /// entities with plain text only.
    fn doctype(&mut self, entities: &mut HashMap<&'a [u8], Entity>) -> Result<(), String> {
        self.require_space()?;
        let name = self.take_until(|b| is_space(b) || matches!(b, b'[' | b'>'));
        if name.is_empty() {
            return Err(self.malformed());
        }
        self.skip_space();
        if self.starts_with(b"SYSTEM") || self.starts_with(b"PUBLIC") {
            let public = self.starts_with(b"PUBLIC");
            self.at += 6;
            self.require_space()?;
            self.literal()?;
            if public {
                self.require_space()?;
                self.literal()?;
            }
            self.skip_space();
        }
        match self.peek() {
            Some(b'>') => {
                self.at += 1;
                return Ok(());
            }
            Some(b'[') => self.at += 1,
            _ => return Err(self.malformed()),
        }
        let other =
            || unsupported("a document type declaration with more than entities and comments");
        loop {
            self.skip_space();
            if self.peek().is_none() {
                return Err(unsupported("an unterminated document type declaration"));
            }
            if self.starts_with(b"]") {
                self.at += 1;
                self.skip_space();
                return if self.peek() == Some(b'>') {
                    self.at += 1;
                    Ok(())
                } else {
                    Err(other())
                };
            } else if self.starts_with(b"<!--") {
                self.at += 4;
                self.skip_past(b"-->")
                    .ok_or_else(|| unsupported("an unterminated document type declaration"))?;
            } else if self.starts_with(b"<!ENTITY") {
                self.at += 8;
                self.entity(entities)?;
            } else {
                // Processing instructions, ELEMENT, ATTLIST, NOTATION, parameter references,
                // anything else, and the end of the file before the subset ends.
                return Err(other());
            }
        }
    }

    /// An entity declaration after `<!ENTITY`: a name and a quoted text without markup,
    /// references or `%`, at most [`MAX_ENTITY_BYTES`] long.
    fn entity(&mut self, entities: &mut HashMap<&'a [u8], Entity>) -> Result<(), String> {
        self.require_space()?;
        if self.peek() == Some(b'%') {
            return Err(unsupported("a parameter entity"));
        }
        let name = self.take_until(|b| is_space(b) || matches!(b, b'"' | b'\'' | b'>'));
        if name.is_empty() {
            return Err(self.malformed());
        }
        self.require_space()?;
        if !matches!(self.peek(), Some(b'"' | b'\'')) {
            return Err(unsupported("an external entity"));
        }
        let text = self.literal()?;
        if text.iter().any(|b| matches!(b, b'<' | b'&' | b'%')) {
            return Err(unsupported("an entity holding markup or references"));
        }
        if text.len() > MAX_ENTITY_BYTES {
            return Err(unsupported("an entity longer than 1024 bytes"));
        }
        self.skip_space();
        self.require(b'>')?;
        let entity = entities.entry(name).or_default();
        // The XML parser takes the first of two declarations; the longer one is counted.
        entity.len = entity.len.max(text.len() as u64);
        Ok(())
    }
}

/// A linear pass over the bytes of an SVG before the XML parser sees them, reading the markup
/// the way `roxmltree` reads it. It refuses what would make parsing or checking slow: elements
/// nested deeper than [`MAX_DEPTH`], an element with more than [`MAX_ATTRIBUTES`] attributes or
/// too many attributes in all, more than [`MAX_NAMESPACES`] namespace declarations, and any
/// document type declaration beyond the strict grammar of [`Scanner::doctype`]. It returns the
/// entities and how often each is used.
///
/// Comments, CDATA sections and processing instructions are skipped where the XML parser skips
/// them; quoted attribute values may hold `>`. Markup the XML parser would refuse is refused here
/// too, so the two never read the bytes differently.
pub(super) fn pre_scan(bytes: &[u8]) -> Result<Scan<'_>, String> {
    let mut s = Scanner { bytes, at: 0 };
    let mut entities = HashMap::new();
    if s.starts_with(&[0xEF, 0xBB, 0xBF]) {
        s.at += 3;
    }
    if s.starts_with(b"<?xml ") {
        s.at += 6;
        s.xml_declaration()?;
    }
    let mut depth = 0u64;
    let mut pairs = 0u64;
    let mut namespaces = 0u64;
    let mut doctype = false;
    let mut root = false;
    while let Some(next) = s.bytes[s.at..].iter().position(|&b| b == b'<' || b == b'&') {
        s.at += next;
        if s.peek() == Some(b'&') {
            s.reference(&mut entities);
            continue;
        }
        if s.starts_with(b"<!--") {
            s.at += 4;
            s.skip_past(b"-->").ok_or_else(|| s.malformed())?;
        } else if s.starts_with(b"<![CDATA[") {
            s.at += 9;
            s.skip_past(b"]]>").ok_or_else(|| s.malformed())?;
        } else if s.starts_with(b"<?") {
            s.at += 2;
            s.skip_past(b"?>").ok_or_else(|| s.malformed())?;
        } else if s.starts_with(b"<!DOCTYPE") {
            if doctype {
                return Err(unsupported("a second document type declaration"));
            }
            if root {
                return Err(unsupported("a document type declaration inside the SVG"));
            }
            doctype = true;
            s.at += 9;
            s.doctype(&mut entities)?;
        } else if s.starts_with(b"<!") {
            return Err(s.malformed());
        } else if s.starts_with(b"</") {
            depth = depth.saturating_sub(1);
            s.at += 2;
            s.skip_past(b">").ok_or_else(|| s.malformed())?;
        } else {
            root = true;
            s.at += 1;
            let name = s.take_until(|b| is_space(b) || matches!(b, b'/' | b'>'));
            if name.is_empty() {
                return Err(s.malformed());
            }
            let mut attributes = 0u64;
            let open = loop {
                let spaced = s.skip_space();
                match s.peek() {
                    None => return Err(s.malformed()),
                    Some(b'>') => {
                        s.at += 1;
                        break true;
                    }
                    Some(b'/') => {
                        s.at += 1;
                        s.require(b'>')?;
                        break false;
                    }
                    Some(_) if !spaced => return Err(s.malformed()),
                    Some(_) => {}
                }
                let name = s.take_until(|b| {
                    is_space(b) || matches!(b, b'=' | b'/' | b'>' | b'"' | b'\'' | b'<')
                });
                if name.is_empty() {
                    return Err(s.malformed());
                }
                if name == b"xmlns" || name.starts_with(b"xmlns:") {
                    namespaces += 1;
                    if namespaces > MAX_NAMESPACES {
                        return Err(unsupported("more than 64 namespace declarations"));
                    }
                }
                attributes += 1;
                if attributes > MAX_ATTRIBUTES {
                    return Err(unsupported("an element with more than 1000 attributes"));
                }
                s.skip_space();
                s.require(b'=')?;
                s.skip_space();
                let quote = match s.peek() {
                    Some(q @ (b'"' | b'\'')) => q,
                    _ => return Err(s.malformed()),
                };
                s.at += 1;
                loop {
                    s.take_until(|b| b == quote || b == b'<' || b == b'&');
                    match s.peek() {
                        Some(b'&') => s.reference(&mut entities),
                        Some(q) if q == quote => {
                            s.at += 1;
                            break;
                        }
                        _ => return Err(s.malformed()),
                    }
                }
            };
            pairs += attributes * attributes;
            if pairs > MAX_ATTRIBUTE_PAIRS {
                return Err(unsupported("this many attributes in all"));
            }
            // Depth counts the elements on a path, the empty element at its end included, as
            // [`check_work`] does.
            if depth + 1 > MAX_DEPTH {
                return Err(unsupported("nesting more than 256 levels deep"));
            }
            if open {
                depth += 1;
            }
        }
    }
    Ok(Scan { entities })
}
/// Checks an SVG logo before `usvg` resolves it. The error is the reason the logo is left out.
pub(super) fn check_svg(doc: &roxmltree::Document<'_>) -> Result<(), String> {
    let links = ids(doc)?;
    // The elements inside a clip path, found in one pass in document order (parents first), so
    // no element walks its ancestors.
    let mut in_clip = HashSet::new();
    for node in doc.descendants().filter(|n| n.is_element()) {
        if node
            .parent()
            .is_some_and(|p| in_clip.contains(&p.id()) || (is_svg(p) && name(p) == "clipPath"))
        {
            in_clip.insert(node.id());
        }
    }
    for node in doc.descendants().filter(|n| n.is_element()) {
        // `usvg` reads every element named `style` as a style sheet, in any namespace.
        if name(node) == "style" {
            return Err(unsupported("the element <style>"));
        }
        if !is_svg(node) {
            continue;
        }
        check_attributes(node)?;
        check_clip_property(node)?;
        if !ALLOWED.contains(&name(node)) {
            return Err(unsupported(&format!("the element <{}>", name(node))));
        }
        // As `node.ancestors()`, which starts at the node itself.
        let clipping = in_clip.contains(&node.id()) || name(node) == "clipPath";
        for (property, value) in properties(node) {
            check_property(clipping, property, value, &links)?;
        }
        match name(node) {
            "linearGradient" | "radialGradient" => {
                check_gradient_links(node, &links)?;
                if gradient_stops(node, &links) > MAX_STOPS {
                    return Err(unsupported("a gradient with more than 256 stops"));
                }
            }
            "clipPath" => check_clip_path(node, &links)?,
            _ => {}
        }
    }
    check_work(doc, &links)
}

/// Every element by its id, as `usvg` maps them for `use`: the first attribute named `id` in any
/// namespace ([`use_id`]), on every element. `usvg` resolves other links among the SVG elements
/// it parsed, by `id` in the namespaces it reads. [`check_attributes`] leaves an SVG element at
/// most one `id`, in such a namespace, so both maps agree; an element with two ids or an id
/// used twice is refused.
fn ids<'a, 'input>(doc: &'a roxmltree::Document<'input>) -> Result<Links<'a, 'input>, String> {
    let mut ids = HashMap::new();
    for node in doc.descendants().filter(|n| n.is_element()) {
        if node.attributes().filter(|a| a.name() == "id").count() > 1 {
            return Err(unsupported("an element with two ids"));
        }
        let Some(id) = use_id(node) else {
            continue;
        };
        if ids.insert(id, node).is_some() {
            return Err(format!(
                "the id \"{id}\" is used twice; export the logo as a plain SVG"
            ));
        }
    }
    Ok(Links { ids })
}

/// `clip-path` at most once per element, attribute and `style` together, and never `inherit`,
/// which copies the parent's clip path.
fn check_clip_property(node: Node<'_, '_>) -> Result<(), String> {
    let values: Vec<&str> = properties(node)
        .into_iter()
        .filter(|(n, _)| *n == "clip-path")
        .map(|(_, v)| v)
        .collect();
    if values.len() > 1 {
        return Err(unsupported("clip-path set more than once on one element"));
    }
    if values.iter().any(|v| v.trim() == "inherit") {
        return Err(unsupported("clip-path: inherit"));
    }
    Ok(())
}

/// One property of an element; `in_clip` tells whether the element is a clip path or lies inside
/// one.
fn check_property(
    in_clip: bool,
    property: &str,
    value: &str,
    links: &Links<'_, '_>,
) -> Result<(), String> {
    let refers_to = |id: &str| links.get(id).filter(|n| is_svg(*n)).map(name);
    if EFFECTS.contains(&property) {
        if value.trim() != "none" {
            return Err(unsupported(&format!("the property {property}")));
        }
    } else if property == "clip-path" {
        if value.trim() == "none" {
            return Ok(());
        }
        if in_clip {
            return Err(unsupported("a clip path inside a clip path"));
        }
        if let Ok(iri) = svgtypes::FuncIRI::from_str(value)
            && let Some(target) = refers_to(iri.0)
            && target != "clipPath"
        {
            return Err(unsupported(&format!("clip-path pointing to <{target}>")));
        }
    } else if property == "fill" || property == "stroke" {
        if let Ok(svgtypes::Paint::FuncIRI(id, _)) = svgtypes::Paint::from_str(value)
            && let Some(target) = refers_to(id)
            && target != "linearGradient"
            && target != "radialGradient"
        {
            return Err(unsupported(&format!("{property} with a <{target}>")));
        }
    } else if property != "href" && value.to_ascii_lowercase().contains("url(") {
        return Err(unsupported(&format!("url() in the property {property}")));
    }
    Ok(())
}

/// A gradient may take its stops from another gradient, and that one from a third; the chain
/// must end within [`MAX_GRADIENT_LINKS`] and lead to gradients only.
fn check_gradient_links(node: Node<'_, '_>, links: &Links<'_, '_>) -> Result<(), String> {
    let mut current = node;
    for _ in 0..MAX_GRADIENT_LINKS {
        let Some(next) = href(current).and_then(|id| links.get(id)) else {
            return Ok(());
        };
        if !is_svg(next) || !matches!(name(next), "linearGradient" | "radialGradient") {
            return Err(unsupported(&format!(
                "a gradient linked to <{}>",
                name(next)
            )));
        }
        current = next;
    }
    Err(unsupported("a chain of gradient links this long"))
}

fn own_stops(gradient: Node<'_, '_>) -> u64 {
    gradient
        .children()
        .filter(|c| is_svg(*c) && name(*c) == "stop")
        .count() as u64
}

fn is_gradient(node: Node<'_, '_>) -> bool {
    is_svg(node) && matches!(name(node), "linearGradient" | "radialGradient")
}

/// The stops a gradient ends up with: its own, or, if it has none, those of the first gradient
/// along its links that has some, as `usvg` takes them. [`check_gradient_links`] has made sure
/// the links end.
fn gradient_stops(gradient: Node<'_, '_>, links: &Links<'_, '_>) -> u64 {
    let mut current = gradient;
    for _ in 0..=MAX_GRADIENT_LINKS {
        let own = own_stops(current);
        if own > 0 {
            return own;
        }
        match href(current).and_then(|id| links.get(id)) {
            Some(next) if is_gradient(next) => current = next,
            _ => return 0,
        }
    }
    0
}

/// A clip path in user space, of shapes, or of `use` pointing to shapes, none of them clipped
/// again.
fn check_clip_path(node: Node<'_, '_>, links: &Links<'_, '_>) -> Result<(), String> {
    if let Some(units) = attr(node, "clipPathUnits")
        && units.trim() != "userSpaceOnUse"
    {
        return Err(unsupported("a clip path in objectBoundingBox units"));
    }
    if links.clip_of(node).is_some() {
        return Err(unsupported("a clip path inside a clip path"));
    }
    for child in node.descendants().skip(1).filter(|n| is_svg(*n)) {
        let child_name = name(child);
        if matches!(child_name, "title" | "desc") || SHAPES.contains(&child_name) {
            continue;
        }
        if child_name != "use" {
            return Err(unsupported(&format!("<{child_name}> inside a clip path")));
        }
        if let Some(target) = href(child).and_then(|id| links.get(id)) {
            if !is_svg(target) || !SHAPES.contains(&name(target)) {
                return Err(unsupported(&format!(
                    "a clip path that uses <{}>",
                    name(target)
                )));
            }
            if links.clip_of(target).is_some() {
                return Err(unsupported("a clip path inside a clip path"));
            }
        }
    }
    Ok(())
}

/// Size of one element as `usvg` holds it: one plus half the length of its path data (`d`,
/// `points`), an upper bound of its points.
fn element_work(node: Node<'_, '_>) -> u64 {
    let data = attr(node, "d").map_or(0, str::len) + attr(node, "points").map_or(0, str::len);
    1 + data as u64 / 2
}

/// One subtree with every `use` and clip path replaced by what it refers to.
#[derive(Debug, Clone, Copy, Default)]
struct Expanded {
    /// Sum of [`element_work`].
    work: u64,
    /// Shapes, each of which may get its own copy of a gradient.
    shapes: u64,
    depth: u64,
}

/// The children of `node` plus what `usvg` copies into it: the target of a `use` and the clip
/// path.
fn parts<'a, 'input>(node: Node<'a, 'input>, links: &Links<'a, 'input>) -> Vec<Node<'a, 'input>> {
    let used = (name(node) == "use")
        .then(|| href(node).and_then(|id| links.get(id)))
        .flatten();
    node.children()
        .filter(|c| c.is_element())
        .chain(used)
        .chain(links.clip_of(node))
        .collect()
}

/// Counts the SVG as `usvg` will build it: every `use` replaced by what it refers to and every
/// clip path copied for each element it clips. A `use` that refers to itself, directly or
/// through others, is refused: `usvg` drops only some of these loops and expands the others until
/// its own limits stop it.
fn check_work<'a, 'input>(
    doc: &'a roxmltree::Document<'input>,
    links: &Links<'a, 'input>,
) -> Result<(), String> {
    let too_complex = || "too complex once `use` and clip paths are resolved".to_string();
    let looped = || unsupported("a `use` that refers to itself");
    let parts = |node| parts(node, links);
    // Each subtree worked out children first, without recursion.
    let mut done: HashMap<NodeId, Expanded> = HashMap::new();
    let mut open = HashSet::new();
    let mut stack = vec![(doc.root_element(), false)];
    while let Some((node, expanded)) = stack.pop() {
        if done.contains_key(&node.id()) {
            continue;
        }
        if !expanded {
            // A node met again while its own subtree is being worked out refers to itself.
            if !open.insert(node.id()) {
                return Err(looped());
            }
            stack.push((node, true));
            for part in parts(node) {
                if open.contains(&part.id()) {
                    return Err(looped());
                }
                if !done.contains_key(&part.id()) {
                    stack.push((part, false));
                }
            }
            continue;
        }
        let mut sum = Expanded {
            work: element_work(node),
            shapes: u64::from(is_svg(node) && SHAPES.contains(&name(node))),
            depth: 0,
        };
        for part in parts(node) {
            let p = done.get(&part.id()).copied().unwrap_or_default();
            sum.work = sum.work.saturating_add(p.work);
            sum.shapes = sum.shapes.saturating_add(p.shapes);
            sum.depth = sum.depth.max(p.depth);
        }
        sum.depth += 1;
        if sum.work > MAX_WORK {
            return Err(too_complex());
        }
        if sum.depth > MAX_DEPTH {
            return Err(format!("SVG nested deeper than {MAX_DEPTH} levels"));
        }
        open.remove(&node.id());
        done.insert(node.id(), sum);
    }
    let all = done
        .get(&doc.root_element().id())
        .copied()
        .unwrap_or_default();
    // `usvg` copies the stops into every gradient that takes them from another, and may give
    // every shape its own copy of a gradient in bounding-box units.
    let gradients: Vec<(u64, u64)> = doc
        .descendants()
        .filter(|n| is_gradient(*n))
        .map(|g| (own_stops(g), gradient_stops(g, links)))
        .collect();
    let taken: u64 = gradients
        .iter()
        .filter(|(own, _)| *own == 0)
        .map(|(_, stops)| stops)
        .sum();
    let most = gradients.iter().map(|(_, stops)| *stops).max().unwrap_or(0);
    if all.work.saturating_add(taken) > MAX_WORK || all.shapes.saturating_mul(most) > MAX_WORK {
        return Err(too_complex());
    }
    Ok(())
}
