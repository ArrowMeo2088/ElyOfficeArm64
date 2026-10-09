import "./style.css";
import "./iso.css";
import { block, stage } from "./iso.ts";
import { model } from "./model.ts";

/** 112 BPM, the film's tempo; blocks land on sixteenths. */
const BEAT = 60 / 112;
const STEP = BEAT / 4;

const app = document.querySelector<HTMLElement>("#app");
if (!app) throw new Error("index.html has no #app");

const blocks: SVGGElement[] = [];
let beat = 1;
for (const part of model) {
  part.blocks.forEach((spec, i) => {
    const g = block(spec);
    g.style.setProperty("--land", `${(beat * BEAT + i * STEP).toFixed(3)}s`);
    blocks.push(g);
  });
  beat += Math.ceil(part.blocks.length / 4);
}

const title = document.createElement("h1");
title.textContent = "Ely Office";
const model3d = stage(blocks);
model3d.classList.add("grow");
model3d.setAttribute("role", "img");
model3d.setAttribute("aria-label", model.slice(1, -1).map((p) => p.name).join(", "));
app.append(title, model3d);
