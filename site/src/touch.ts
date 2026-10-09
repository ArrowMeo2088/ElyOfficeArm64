// Buildings lift on hover or focus and sink when pressed.
import { UNIT } from "./iso.ts";

const SVG = "http://www.w3.org/2000/svg";

/** Links each app's blocks to its region, in reading order. */
export function touch(host: HTMLElement, svg: SVGSVGElement, parts: Map<string, SVGGElement[]>): void {
  host.style.setProperty("--step", `${UNIT}px`);
  const nav = document.createElement("nav");
  nav.className = "doors";
  host.append(nav);

  for (const [name, blocks] of parts) {
    const target = `#${name.toLowerCase()}`;
    const state = { hover: false, focus: false, press: false };
    for (const b of blocks) b.dataset.part = name;
    const hit = area(blocks, name);
    svg.append(hit);
    const caption = document.createElement("p");
    caption.className = "caption";
    caption.textContent = name;
    caption.setAttribute("aria-hidden", "true");
    host.append(caption);

    const show = () => {
      const up = state.hover || state.focus;
      for (const b of blocks) {
        b.classList.toggle("lifted", up);
        b.classList.toggle("pressed", up && state.press);
      }
      caption.classList.toggle("shown", up);
      if (up) place(caption, host, hit);
    };
    const set = (change: Partial<typeof state>) => {
      Object.assign(state, change);
      show();
    };

    hit.addEventListener("pointerenter", () => set({ hover: true }));
    hit.addEventListener("pointerleave", () => set({ hover: false, press: false }));
    hit.addEventListener("pointerdown", (e) => e.button === 0 && set({ press: true }));
    hit.addEventListener("pointerup", () => set({ press: false }));
    hit.addEventListener("pointercancel", () => set({ press: false }));
    hit.addEventListener("click", () => (location.hash = target));

    const door = document.createElement("a");
    door.href = target;
    door.textContent = name;
    door.addEventListener("focus", () => set({ focus: true }));
    door.addEventListener("blur", () => set({ focus: false, press: false }));
    door.addEventListener("keydown", (e) => e.key === "Enter" && !e.repeat && set({ press: true }));
    door.addEventListener("keyup", (e) => e.key === "Enter" && set({ press: false }));
    nav.append(door);
  }
}

/** A still hit area over the building, resting and lifted. */
function area(blocks: SVGGElement[], name: string): SVGGElement {
  const hit = document.createElementNS(SVG, "g");
  hit.setAttribute("class", "hit");
  hit.dataset.part = name;
  for (const b of blocks) {
    const outline = b.querySelector(".outline");
    if (!outline) throw new Error("block has no outline");
    for (const dy of [0, -UNIT]) {
      const face = outline.cloneNode() as SVGPolygonElement;
      face.setAttribute("class", "hit-face");
      face.setAttribute("transform", `translate(0 ${dy})`);
      hit.append(face);
    }
  }
  return hit;
}

/** Puts the caption above the lifted building. */
function place(caption: HTMLElement, host: HTMLElement, hit: SVGGElement) {
  const origin = host.getBoundingClientRect();
  const r = hit.getBoundingClientRect();
  caption.style.left = `${r.left + r.width / 2 - origin.left}px`;
  caption.style.top = `${r.top - origin.top}px`;
}
