// Film additions: desk tiles, screens, files, props.
import type { Block } from "../../src/iso.ts";
import { ACCENT, model } from "../../src/model.ts";

export type Box = Required<Block>;

const desk = model[0].blocks[0];
export const DESK = desk.tone;
export const PAPER = model[1].blocks[0].tone;
/** The blank face a recording is later pinned onto. */
export const SCREEN = "#3a3a3c";
const TOP = desk.h;

const on = (x: number, y: number, w: number, d: number, h: number, z = 0, tone = PAPER): Box => ({
  x, y, z: TOP + z, w, d, h, tone,
});

/** The desk as one-cell tiles, for growing it. */
export const tiles: Box[] = Array.from({ length: desk.w * desk.d }, (_, i) => ({
  x: i % desk.w, y: Math.floor(i / desk.w), z: 0, w: 1, d: 1, h: desk.h, tone: DESK,
}));

/** Upright 16:9 screens on each app; +y face is pinned. */
const screen = (x: number, y: number, w: number, z: number) => on(x, y, w, 0.08, (w * 9) / 16, z, SCREEN);
export const screens: Record<string, Box> = {
  ElyDoc: screen(1.24, 2.2, 2.6, 1),
  ElyTable: screen(5, 1.4, 2.9, 2.4),
  ElyShow: on(9.4, 1.25, 3.2, 0.05, 1.8, 0.4, SCREEN),
  ElyPDF: screen(13, 4.3, 2.4, 1),
  ElyTree: screen(6.5, 7.9, 2.9, 2.8),
  ElyMark: screen(1, 7.4, 3, 2),
};

/** A file travelling a road, one hop per beat. */
export interface Trip {
  path: [number, number][];
  lands: number;
  hops: number[];
}

const FILE = 0.48;
const ROAD = TOP + 0.08;

/** Mark to Tree first, then four exports queue at PDF. */
export const trips: Trip[] = [
  { path: [[4.26, 7.9], [5.2, 7.9]], lands: 129, hops: [130, 131] },
  { path: [[11, 3.1], [11, 4.9], [12.74, 4.9]], lands: 130, hops: [132, 133] },
  { path: [[6.3, 4.16], [6.3, 4.9], [12.14, 4.9]], lands: 130.25, hops: [132, 133, 134, 135] },
  { path: [[2.3, 4.26], [2.3, 4.9], [11.54, 4.9]], lands: 130.5, hops: [132, 133, 134, 135, 136, 137] },
  { path: [[2.3, 6.74], [2.3, 4.9], [10.94, 4.9]], lands: 130.75, hops: [132, 133, 134, 135, 136, 137] },
];

/** The point `s` along a polyline. */
function along(path: [number, number][], s: number): [number, number] {
  for (let i = 1; i < path.length; i++) {
    const [[ax, ay], [bx, by]] = [path[i - 1], path[i]];
    const length = Math.hypot(bx - ax, by - ay);
    if (s <= length || i === path.length - 1) {
      const u = Math.min(s / length, 1);
      return [ax + (bx - ax) * u, ay + (by - ay) * u];
    }
    s -= length;
  }
  throw new Error("a path needs two points");
}

/** Total length of a polyline. */
export function length(path: [number, number][]): number {
  return path.slice(1).reduce((sum, [x, y], i) => sum + Math.hypot(x - path[i][0], y - path[i][1]), 0);
}

/** The file box with its centre `s` along the trip. */
export function file(trip: Trip, s: number): Box {
  const [x, y] = along(trip.path, s);
  return { x: x - FILE / 2, y: y - FILE / 2, z: ROAD, w: FILE, d: FILE, h: FILE, tone: ACCENT };
}

/** Three props, each landing block by block from its bar. */
export const props: { bar: number; blocks: Box[] }[] = [
  {
    bar: 37,
    blocks: [
      on(10, 7.2, 1.2, 0.6, 1.3),
      on(10.2, 8.5, 0.8, 0.8, 0.6),
      on(10.38, 8.15, 0.12, 0.35, 0.12, 0.25),
      on(10.7, 8.15, 0.12, 0.35, 0.12, 0.25),
      on(10.45, 9.3, 0.3, 1.2, 0.2),
      on(10.75, 10.2, 1.4, 0.3, 0.2),
    ],
  },
  {
    bar: 38,
    blocks: [
      on(12.4, 7.2, 1.9, 1.9, 0.1),
      on(12.4, 7.2, 1.9, 0.12, 1.1, 0.1),
      on(12.4, 7.32, 0.12, 1.78, 1.1, 0.1),
      on(14.18, 7.32, 0.12, 1.66, 1.1, 0.1),
      on(12.52, 8.98, 1.78, 0.12, 1.1, 0.1),
      on(11.5, 7.32, 0.9, 1.66, 0.06),
      on(12.52, 9.1, 1.78, 0.9, 0.06),
    ],
  },
  {
    bar: 39,
    blocks: [
      on(14.6, 8.2, 1.3, 0.12, 1.5),
      on(14.6, 8.2, 0.55, 0.12, 0.2, 1.5),
      on(14.75, 8.32, 1, 0.08, 1.4, 0, ACCENT),
      on(14.6, 8.4, 1.3, 0.12, 1.15),
    ],
  },
];
