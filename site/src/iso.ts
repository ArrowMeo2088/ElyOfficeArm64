// Isometric blocks: one colour, three shaded faces, no perspective.

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

/** Builds a block: top, left and right faces of one colour. */
export function block(spec: Block, top?: Node): HTMLElement {
  const el = document.createElement("div");
  el.className = "block";
  el.style.setProperty("--x", String(spec.x));
  el.style.setProperty("--y", String(spec.y));
  el.style.setProperty("--z", String(spec.z ?? 0));
  el.style.setProperty("--w", String(spec.w));
  el.style.setProperty("--d", String(spec.d));
  el.style.setProperty("--h", String(spec.h));
  el.style.setProperty("--tone", spec.tone);
  for (const face of ["left", "right", "top"]) {
    const part = document.createElement("div");
    part.className = `face ${face}`;
    if (face === "top" && top) part.append(top);
    el.append(part);
  }
  return el;
}

/** The ground: a stage `w` by `d` cells that holds blocks. */
export function stage(w: number, d: number): HTMLElement {
  const el = document.createElement("div");
  el.className = "stage";
  el.style.setProperty("--w", String(w));
  el.style.setProperty("--d", String(d));
  return el;
}
