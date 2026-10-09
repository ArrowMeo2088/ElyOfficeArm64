// The model at any moment, as pieces and texts.
import { model } from "../../src/model.ts";
import { type Box, file, length, props, screens, tiles, trips } from "./parts.ts";
import { BEAT, LAND, begun, glide, toward } from "./time.ts";

/** A box and its landing progress, 0 to 1. */
export interface Piece {
  box: Box;
  land: number;
}

/** Everything one frame shows, before the camera. */
export interface Moment {
  pieces: Piece[];
  /** 0 is black, 1 is the page. */
  light: number;
  /** 0 is the light palette, 1 the dark one. */
  dark: number;
  texts: Record<string, number>;
}

/** The desk grows in rings around this tile. */
const SEED = [8, 5];
const ring = (b: Box) => Math.abs(b.x - SEED[0]) + Math.abs(b.y - SEED[1]);
const angle = (b: Box) => Math.atan2(b.y - SEED[1], b.x - SEED[0]);

/** Ring k lands from beat k + 1, by quarters. */
const tileBeats = tiles.map((tile) => {
  const k = ring(tile);
  const mates = tiles.filter((t) => ring(t) === k).sort((a, b) => angle(a) - angle(b));
  const quarter = Math.floor((mates.indexOf(tile) * 4) / mates.length);
  return k + 1 + quarter / 4;
});
const DESK_DONE = 15;

/** Beat each app starts on; its blocks follow on sixteenths. */
const partBeats = [16, 18, 21, 23, 25, 27, 29];

/** Beats of each text's fade, ending on that beat. */
const TEXTS: Record<string, [number, number]> = {
  title: [31, 32],
  name: [161, 168],
  version: [161.5, 168],
  url: [162, 168],
};

const DARK = 136;

function landed(t: number, box: Box, beat: number, out: Piece[]) {
  if (begun(t, beat)) out.push({ box, land: toward(t, beat) });
}

/** A trip's file: dropped at its start, then hopping. */
function travel(t: number, trip: (typeof trips)[number]): Piece | null {
  if (!begun(t, trip.lands)) return null;
  const step = length(trip.path) / trip.hops.length;
  const s = trip.hops.reduce((sum, beat) => sum + step * toward(t, beat), 0);
  return { box: file(trip, s), land: toward(t, trip.lands) };
}

/** The model at `t`, plus a plate's screen. */
export function moment(t: number, screen?: { app: string; beat: number }): Moment {
  const pieces: Piece[] = [];
  if (t < DESK_DONE * BEAT) tiles.forEach((tile, i) => landed(t, tile, tileBeats[i], pieces));
  else pieces.push({ box: model[0].blocks[0], land: 1 });
  model.slice(1).forEach((part, p) => part.blocks.forEach((b, i) => landed(t, b, partBeats[p] + i / 4, pieces)));
  for (const trip of trips) {
    const piece = travel(t, trip);
    if (piece) pieces.push(piece);
  }
  for (const prop of props) prop.blocks.forEach((b, i) => landed(t, b, (prop.bar - 1) * 4 + i / 4, pieces));
  if (screen) {
    const box = screens[screen.app];
    if (!box) throw new Error(`no screen for ${screen.app}`);
    landed(t, box, screen.beat, pieces);
  }
  const texts: Record<string, number> = {};
  for (const [name, [from, to]] of Object.entries(TEXTS)) {
    if (!screen && t < to * BEAT) texts[name] = toward(t, from, LAND, glide);
  }
  return { pieces, light: toward(t, 0.5, LAND, glide), dark: toward(t, DARK, LAND, glide), texts };
}
