// The sheet preview: the renderer's SVG in the page, scaled to fit. The SVG's user unit is 1 mm on paper, so a
// click converts to page mm through the SVG's own matrix.

const SVG_NS = "http://www.w3.org/2000/svg";

// Puts the SVG text `svg` into `host`; returns the <svg> element, or null if the text is not an SVG.
export function show(host, svg) {
  // The renderer writes `style="mix-blend-mode:multiply"` on the course overprint. The page's CSP blocks style
  // attributes, so the text is changed before it is parsed: a class carries the blend (app.css).
  const text = svg.replaceAll(' style="mix-blend-mode:multiply"', ' class="blend-multiply"');
  const parsed = new DOMParser().parseFromString(text, "image/svg+xml");
  const root = parsed.documentElement;
  if (root.localName !== "svg" || parsed.querySelector("parsererror")) return null;
  const node = document.importNode(root, true);
  // The size comes from the container; the viewBox keeps the page's proportions.
  node.removeAttribute("width");
  node.removeAttribute("height");
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

// The size of the sheet on screen: `zoom` 1 fits the whole page into `host`, 2 is twice that. The viewBox is
// the page in mm, so the width and height follow its proportions.
export function setZoom(host, svg, zoom) {
  const box = svg.viewBox.baseVal;
  if (!box || !box.width || !box.height) return;
  const room = 16; // the host's padding
  const fit = Math.min((host.clientWidth - room) / box.width, (host.clientHeight - room) / box.height);
  if (!(fit > 0)) return;
  svg.style.width = `${box.width * fit * zoom}px`;
  svg.style.height = `${box.height * fit * zoom}px`;
}

// The marks of a drawing mode over the sheet (page mm): a line, round marks at points, and with `ring` open
// circles instead of dots. Called again it replaces the earlier marks; with nothing it clears them.
export function draw(svg, { line = [], marks = [], ring = false }) {
  for (const old of svg.querySelectorAll(".draw")) old.remove();
  const add = (name, attributes) => {
    const node = document.createElementNS(SVG_NS, name);
    node.setAttribute("class", `draw ${ring && name === "circle" ? "draw-ring" : "draw-mark"}`);
    for (const [key, value] of Object.entries(attributes)) node.setAttribute(key, value);
    svg.append(node);
    return node;
  };
  if (line.length >= 2) {
    const path = add("polyline", { points: line.map(([x, y]) => `${x},${y}`).join(" ") });
    path.setAttribute("class", "draw draw-line");
  }
  for (const [x, y] of marks) add("circle", { cx: x, cy: y, r: ring ? 3 : 1 });
}
