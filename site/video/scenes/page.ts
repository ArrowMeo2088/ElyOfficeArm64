// Draws one film frame with the site's own iso renderer.
import "../../src/iso.css";
import { UNIT, block, order, project } from "../../src/iso.ts";
import { ACCENT, model } from "../../src/model.ts";
import { type Box, DESK, PAPER, SCREEN, props, screens, tiles } from "./parts.ts";
import { BAR, BEAT, glide, toward } from "./time.ts";
import { type Moment, moment } from "./timeline.ts";

export const WIDTH = 1920;
export const HEIGHT = 1080;
export const FPS = 30;

/** Bars [from, to); plates push into one app. */
export interface Scene {
  name: string;
  from: number;
  to: number;
  app?: string;
}

const PLATES = ["ElyDoc", "ElyTable", "ElyShow", "ElyPDF", "ElyTree", "ElyMark"];
export const scenes: Scene[] = [
  { name: "opening", from: 0, to: 4 },
  { name: "rise", from: 4, to: 8 },
  ...PLATES.map((app, i) => ({ name: `plate-${app.slice(3).toLowerCase()}`, from: 8 + i * 4, to: 9 + i * 4, app })),
  { name: "files", from: 32, to: 36 },
  { name: "props", from: 36, to: 40 },
  { name: "end", from: 40, to: 42 },
];

/** Global frames [first, end) whose time falls in the scene. */
export function frames(scene: Scene): [number, number] {
  const first = (bar: number) => Math.ceil((bar * BAR * FPS) - 1e-9);
  return [first(scene.from), first(scene.to)];
}

/** Light tone to dark tone; three faces derive from it. */
const DARK: Record<string, string> = {
  [DESK]: "#3a3a3a",
  [PAPER]: "#5c5c5a",
  [SCREEN]: "#3a3a3c",
  [ACCENT]: ACCENT,
};
const PAGE = ["#fbfbfa", "#252525"];
const INK = ["#1d1d1f", "#f5f5f7"];
const SOFT = ["#6e6e73", "#a1a1a6"];

const mix = (a: string, b: string, u: number) =>
  u <= 0 ? a : u >= 1 ? b : `color-mix(in oklab, ${a}, ${b} ${(u * 100).toFixed(2)}%)`;

function tone(light: string, dark: number): string {
  const night = DARK[light];
  if (!night) throw new Error(`no dark tone for ${light}`);
  return mix(light, night, dark);
}

/** World to frame pixels: scale, then the origin's pixel. */
interface Camera {
  s: number;
  x: number;
  y: number;
}

const corners = (b: Box): [number, number][] =>
  [0, 1].flatMap((i) => [0, 1].flatMap((j) => [0, 1].map((k) => project(b.x + i * b.w, b.y + j * b.d, b.z + k * b.h))));

/** Fits the whole film's model below the title. */
const base: Camera = (() => {
  const all = [...model.flatMap((p) => p.blocks), ...props.flatMap((p) => p.blocks)].flatMap(corners);
  const xs = all.map(([x]) => x);
  const ys = all.map(([, y]) => y);
  const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  const s = Math.min(1500 / (x1 - x0), 620 / (y1 - y0));
  return { s, x: WIDTH / 2 - (s * (x0 + x1)) / 2, y: 690 - (s * (y0 + y1)) / 2 };
})();

/** Pinned face corners: TL, TR, BL, BR of +y side. */
function face(b: Box): [number, number][] {
  const y = b.y + b.d;
  return [project(b.x, y, b.z + b.h), project(b.x + b.w, y, b.z + b.h), project(b.x, y, b.z), project(b.x + b.w, y, b.z)];
}

/** Pushes from the base view until the screen fills frame. */
function push(app: string, t: number, beat: number): Camera {
  const pts = face(screens[app]);
  const xs = pts.map(([x]) => x);
  const ys = pts.map(([, y]) => y);
  const [cx, cy] = [(Math.min(...xs) + Math.max(...xs)) / 2, (Math.min(...ys) + Math.max(...ys)) / 2];
  const end = Math.min((0.78 * HEIGHT) / (Math.max(...ys) - Math.min(...ys)), (0.8 * WIDTH) / (Math.max(...xs) - Math.min(...xs)));
  const u = toward(t, beat + 2, 2 * BEAT, glide);
  const s = base.s * (end / base.s) ** u;
  const [px, py] = [base.x + base.s * cx, base.y + base.s * cy];
  const [qx, qy] = [px + (WIDTH / 2 - px) * u, py + (HEIGHT / 2 - py) * u];
  return { s, x: qx - s * cx, y: qy - s * cy };
}

const svg = document.querySelector<SVGSVGElement>("#frame");
const world = document.querySelector<SVGGElement>("#world");
if (!svg || !world) throw new Error("index.html lacks #frame or #world");

function draw(m: Moment, cam: Camera) {
  const page = mix(mix("#000000", PAGE[0], m.light), PAGE[1], m.dark);
  document.body.style.background = page;
  svg!.style.background = page;
  world!.setAttribute("transform", `matrix(${cam.s} 0 0 ${cam.s} ${cam.x} ${cam.y})`);
  const groups = m.pieces.map(({ box, land }) => {
    const g = block({ ...box, tone: tone(box.tone, m.dark) });
    if (tiles.includes(box)) g.classList.add("tile");
    if (land < 1) {
      g.setAttribute("transform", `translate(0 ${(-UNIT * (1 - land)).toFixed(3)})`);
      g.setAttribute("opacity", land.toFixed(4));
    }
    return g;
  });
  world!.replaceChildren(...order(m.pieces.map((p) => p.box)).map((i) => groups[i]));
  for (const el of document.querySelectorAll<HTMLElement>("[data-text]")) {
    const shown = m.texts[el.dataset.text!] ?? 0;
    el.style.opacity = shown.toFixed(4);
    const [day, dark] = el.dataset.soft === undefined ? INK : SOFT;
    el.style.color = mix(day, dark, m.dark);
  }
}

/** Screen-face corners in frame pixels, with the landing's state. */
export interface Face {
  opacity: number;
  corners: [number, number][];
}

/** Draws global frame `f` of `scene`; plates return the face. */
function render(name: string, f: number): Face | null {
  const scene = scenes.find((s) => s.name === name);
  if (!scene) throw new Error(`no scene ${name}`);
  const [first, end] = frames(scene);
  if (f < first || f >= end) throw new Error(`frame ${f} is outside ${name}`);
  const t = f / FPS;
  const app = scene.app;
  const beat = scene.from * 4 + 1;
  const m = moment(t, app ? { app, beat } : undefined);
  const cam = app ? push(app, t, beat) : base;
  draw(m, cam);
  if (!app) return null;
  const piece = m.pieces.find((p) => p.box === screens[app]);
  if (!piece) return { opacity: 0, corners: [] };
  const lift = -UNIT * (1 - piece.land);
  return {
    opacity: piece.land,
    corners: face(piece.box).map(([x, y]) => [cam.x + cam.s * x, cam.y + cam.s * (y + lift)]),
  };
}

declare global {
  interface Window {
    ely: { scenes: Scene[]; frames: typeof frames; render: typeof render };
  }
}
window.ely = { scenes, frames, render };
