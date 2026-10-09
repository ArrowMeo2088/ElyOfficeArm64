// True isometric in SVG: edges at 30°, no perspective.

const SVG = "http://www.w3.org/2000/svg";
const COS = Math.cos(Math.PI / 6);
const SIN = Math.sin(Math.PI / 6);

/** Pixels per grid cell; heights use the same unit. */
export const UNIT = 28;

/** A block on the grid, in grid units. */
export interface Block {
  x: number;
  y: number;
  z?: number;
  w: number;
  d: number;
  h: number;
  tone: string;
}

/** Grid point to screen pixels. */
export function project(x: number, y: number, z: number): [number, number] {
  return [(x - y) * COS * UNIT, ((x + y) * SIN - z) * UNIT];
}

function polygon(points: [number, number][], className: string): SVGPolygonElement {
  const el = document.createElementNS(SVG, "polygon");
  el.setAttribute("points", points.map(([x, y]) => `${x.toFixed(2)},${y.toFixed(2)}`).join(" "));
  el.setAttribute("class", className);
  return el;
}

/** A block: outline, then left, right and top faces. */
export function block(spec: Block): SVGGElement {
  const { x, y, w, d, h } = spec;
  const z = spec.z ?? 0;
  const p = (dx: number, dy: number, dz: number) => project(x + dx, y + dy, z + dz);
  const g = document.createElementNS(SVG, "g");
  g.setAttribute("class", "block");
  g.style.setProperty("--tone", spec.tone);
  // The outline hides seams between faces.
  g.append(
    polygon([p(0, d, 0), p(w, d, 0), p(w, 0, 0), p(w, 0, h), p(0, 0, h), p(0, d, h)], "face outline"),
    polygon([p(0, d, 0), p(w, d, 0), p(w, d, h), p(0, d, h)], "face left"),
    polygon([p(w, d, 0), p(w, 0, 0), p(w, 0, h), p(w, d, h)], "face right"),
    polygon([p(0, 0, h), p(w, 0, h), p(w, d, h), p(0, d, h)], "face top"),
  );
  boxes.set(g, { ...spec, z });
  return g;
}

const boxes = new WeakMap<SVGGElement, Block & { z: number }>();

/** Whether `a` draws before `b`: the first separating axis decides. */
function behind(a: Block & { z: number }, b: Block & { z: number }): boolean {
  if (a.x + a.w <= b.x) return true;
  if (b.x + b.w <= a.x) return false;
  if (a.y + a.d <= b.y) return true;
  if (b.y + b.d <= a.y) return false;
  if (a.z + a.h <= b.z) return true;
  if (b.z + b.h <= a.z) return false;
  throw new Error("blocks intersect");
}

/** Blocks in drawing order, back to front. */
function ordered(blocks: SVGGElement[]): SVGGElement[] {
  const box = (g: SVGGElement) => {
    const found = boxes.get(g);
    if (!found) throw new Error("not a block");
    return found;
  };
  const left = [...blocks];
  const out: SVGGElement[] = [];
  while (left.length) {
    const next = left.findIndex((g) => left.every((o) => o === g || !behind(box(o), box(g))));
    if (next < 0) throw new Error("blocks overlap; no drawing order");
    out.push(...left.splice(next, 1));
  }
  return out;
}

/** An SVG for `blocks`, drawn back to front. */
export function stage(blocks: SVGGElement[]): SVGSVGElement {
  const svg = document.createElementNS(SVG, "svg");
  svg.append(...ordered(blocks));
  document.body.append(svg);
  const box = svg.getBBox();
  svg.remove();
  svg.setAttribute("viewBox", `${box.x} ${box.y} ${box.width} ${box.height}`);
  svg.setAttribute("width", String(Math.ceil(box.width)));
  svg.setAttribute("height", String(Math.ceil(box.height)));
  svg.setAttribute("class", "stage");
  return svg;
}
