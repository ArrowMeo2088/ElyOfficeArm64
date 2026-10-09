// One region per app, then the three props.
import { block, screen, stage, type Block } from "./iso.ts";
import { grow } from "./beat.ts";
import { model, type Part } from "./model.ts";
import { said, type Key } from "./text.ts";

const RELEASE = "https://github.com/ZacharyZhang-NY/ElyOffice/releases/download/v0.1.0";

const PAPER = "#f4f4f2";
const DESK = "#e6e6e2";
const SCREEN = "#3a3a3c";
const SHOT = [1440, 810];

/** A part's blocks moved to corner (x, y). */
function moved(part: Part, x: number, y: number): Required<Block>[] {
  const left = Math.min(...part.blocks.map((b) => b.x));
  const back = Math.min(...part.blocks.map((b) => b.y));
  const floor = Math.min(...part.blocks.map((b) => b.z));
  const snap = (v: number) => Math.round(v * 1000) / 1000;
  return part.blocks.map((b) => ({ ...b, x: snap(b.x - left + x), y: snap(b.y - back + y), z: snap(b.z - floor + 0.4) }));
}

/** A plinth, the app's screen, and its building. */
function scene(part: Part): SVGSVGElement {
  const plinth = block({ x: 0, y: 0, w: 11.6, d: 5, h: 0.4, tone: DESK });
  const slab = block({ x: 0.4, y: 1.2, z: 0.4, w: 7.2, d: 0.25, h: 4.05, tone: SCREEN });
  screen(slab, `/shots/${part.name.slice(3).toLowerCase()}.jpg`, SHOT[0], SHOT[1]);
  const house = moved(part, 8.4, 1.2).map(block);
  grow([[plinth], [slab], house]);
  const svg = stage([plinth, slab, ...house]);
  svg.classList.add("grow");
  svg.setAttribute("role", "img");
  svg.dataset.shot = part.name;
  return svg;
}

const t = (x: number, y: number, z: number, w: number, d: number, h: number, tone = PAPER) =>
  block({ x, y, z, w, d, h, tone });

/** Offline: a plug pulled from its socket. */
const cable = () => [
  t(0, 0, 0, 7, 3, 0.3, DESK),
  t(0.4, 1.3, 0.3, 2.6, 0.4, 0.4),
  t(3, 1.15, 0.3, 0.9, 0.7, 0.7),
  t(5, 0.9, 0.3, 1.4, 1.2, 1.2),
];

/** Open source: a box with its lid laid open. */
const box = () => [
  t(0, 0, 0, 7, 4, 0.3, DESK),
  t(1, 0.6, 0.3, 3, 0.2, 1.6),
  t(1, 0.8, 0.3, 0.2, 2.2, 1.6),
  t(3.8, 0.8, 0.3, 0.2, 2.2, 1.6),
  t(1.2, 0.8, 0.3, 2.6, 2.2, 0.2),
  t(1, 3, 0.3, 3, 0.2, 1.6),
  t(4.3, 0.6, 0.3, 2.2, 2.6, 0.12),
];

/** Local files: a folder holding one sheet. */
const folder = () => [
  t(0, 0, 0, 7, 3, 0.3, DESK),
  t(1, 1, 0.3, 4.6, 0.2, 2.6),
  t(1, 1, 2.9, 1.6, 0.2, 0.4),
  t(1.4, 1.2, 0.3, 3.8, 0.1, 2.3, "#3d7fe5"),
  t(1, 1.3, 0.3, 4.6, 0.2, 2),
];

/** A prop with its line. */
function prop(blocks: SVGGElement[], line: Key): HTMLElement {
  grow(blocks.map((b) => [b]));
  const svg = stage(blocks);
  svg.classList.add("grow");
  svg.setAttribute("aria-hidden", "true");
  const p = said("p", line);
  const cell = document.createElement("div");
  cell.className = "prop";
  cell.append(svg, p);
  return cell;
}

/** The app regions and the props row. */
export function regions(): HTMLElement[] {
  const apps = model.slice(1, -1).map((part) => {
    const section = document.createElement("section");
    section.id = part.name.toLowerCase();
    section.className = "region";
    const h2 = document.createElement("h2");
    h2.textContent = part.name;
    const get = said("a", "download");
    get.className = "get";
    get.href = `${RELEASE}/${part.name}-0.1.0-arm64.dmg`;
    const text = document.createElement("div");
    text.append(h2, said("p", part.name as Key), get);
    section.append(scene(part), text);
    return section;
  });
  const props = document.createElement("section");
  props.className = "props";
  props.append(
    prop(cable(), "offline"),
    prop(box(), "open"),
    prop(folder(), "local"),
  );
  return [...apps, props];
}

/** Starts each model's growth when it scrolls near. */
export function reveal(sections: HTMLElement[]): void {
  const seen = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        if (!e.isIntersecting) continue;
        for (const svg of e.target.querySelectorAll(".stage")) svg.classList.add("seen");
        seen.unobserve(e.target);
      }
    },
    { rootMargin: "0px 0px -20% 0px" },
  );
  for (const s of sections) seen.observe(s);
}
