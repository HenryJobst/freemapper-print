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

/// Refuses attributes that `usvg` and this check could read differently: two attributes of one
/// local name on an element, in any namespaces; attributes of foreign namespaces other than
/// those of drawing programs; and `id` or `style` in a drawing program's namespace, which
/// `usvg` reads as if they had none.
fn check_attributes(node: Node<'_, '_>) -> Result<(), String> {
    let all: Vec<_> = node.attributes().collect();
    for (i, a) in all.iter().enumerate() {
        if all[..i].iter().any(|b| b.name() == a.name()) {
            return Err(unsupported(&format!(
                "an element with two \"{}\" attributes",
                a.name()
            )));
        }
    }
    for a in &all {
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

/// Checks an SVG logo before `usvg` resolves it. The error is the reason the logo is left out.
pub(super) fn check_svg(doc: &roxmltree::Document<'_>) -> Result<(), String> {
    let links = ids(doc)?;
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
        for (property, value) in properties(node) {
            check_property(node, property, value, &links)?;
        }
        match name(node) {
            "linearGradient" | "radialGradient" => check_gradient_links(node, &links)?,
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

fn check_property(
    node: Node<'_, '_>,
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
        if node.ancestors().any(|a| is_svg(a) && name(a) == "clipPath") {
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
    // `usvg` may give every shape its own copy of a gradient in bounding-box units.
    let stops = doc
        .descendants()
        .filter(|n| is_svg(*n) && matches!(name(*n), "linearGradient" | "radialGradient"))
        .map(|g| g.children().filter(|c| c.is_element()).count() as u64)
        .max()
        .unwrap_or(0);
    if all.shapes.saturating_mul(stops) > MAX_WORK {
        return Err(too_complex());
    }
    Ok(())
}
