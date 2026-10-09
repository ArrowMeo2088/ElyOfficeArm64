import "./style.css";
import "./iso.css";
import { block, stage } from "./iso.ts";
import { model } from "./model.ts";
import { touch } from "./touch.ts";

/** 112 BPM, the film's tempo; blocks land on sixteenths. */
const BEAT = 60 / 112;
const STEP = BEAT / 4;

const app = document.querySelector<HTMLElement>("#app");
if (!app) throw new Error("index.html has no #app");

const apps = model.slice(1, -1);
const parts = new Map<string, SVGGElement[]>();
const blocks: SVGGElement[] = [];
let beat = 1;
for (const part of model) {
  const own = part.blocks.map((spec, i) => {
    const g = block(spec);
    g.style.setProperty("--land", `${(beat * BEAT + i * STEP).toFixed(3)}s`);
    return g;
  });
  if (apps.includes(part)) parts.set(part.name, own);
  blocks.push(...own);
  beat += Math.ceil(part.blocks.length / 4);
}

const title = document.createElement("h1");
title.textContent = "Ely Office";
const host = document.createElement("div");
host.className = "model";
const svg = stage(blocks);
svg.classList.add("grow");
svg.setAttribute("aria-hidden", "true");
host.append(svg);
app.append(title, host);
touch(host, parts);
