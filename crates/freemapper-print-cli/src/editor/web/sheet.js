// The sheet preview: the renderer's SVG in the page, scaled to fit. The SVG's user unit is 1 mm on paper, so a
// click converts to page mm through the SVG's own matrix.

const SVG_NS = "http://www.w3.org/2000/svg";

// The renderer writes `style="mix-blend-mode:multiply"` on the course overprint. The page's CSP blocks style
// attributes, so the declarations are moved to the CSSOM, which it allows.
function moveStyles(root) {
  for (const el of root.querySelectorAll("[style]")) {
    for (const declaration of el.getAttribute("style").split(";")) {
      const [name, value] = declaration.split(":").map((part) => part.trim());
      if (name && value) el.style.setProperty(name, value);
    }
    el.removeAttribute("style");
  }
}

// Puts the SVG text `svg` into `host`; returns the <svg> element, or null if the text is not an SVG.
export function show(host, svg) {
  const parsed = new DOMParser().parseFromString(svg, "image/svg+xml");
  const root = parsed.documentElement;
  if (root.localName !== "svg" || parsed.querySelector("parsererror")) return null;
  const node = document.importNode(root, true);
  // The size comes from the container; the viewBox keeps the page's proportions.
  node.removeAttribute("width");
  node.removeAttribute("height");
  moveStyles(node);
  host.replaceChildren(node);
  return node;
}

// The page position (mm, y down) of a mouse event over `svg`.
export function pageMm(svg, event) {
  const point = svg.createSVGPoint();
  point.x = event.clientX;
  point.y = event.clientY;
  const at = point.matrixTransform(svg.getScreenCTM().inverse());
  return { x: at.x, y: at.y };
}

// Outlines a rectangle [x0, y0, x1, y1] (page mm) over the sheet; null clears the outline.
export function outline(svg, rect) {
  for (const old of svg.querySelectorAll(".pick-rect")) old.remove();
  if (!rect) return;
  const box = document.createElementNS(SVG_NS, "rect");
  box.setAttribute("class", "pick-rect");
  box.setAttribute("x", rect[0]);
  box.setAttribute("y", rect[1]);
  box.setAttribute("width", rect[2] - rect[0]);
  box.setAttribute("height", rect[3] - rect[1]);
  svg.append(box);
}
