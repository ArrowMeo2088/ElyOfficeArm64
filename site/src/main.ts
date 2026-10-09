import "./style.css";
import "./iso.css";
import { block, stage } from "./iso.ts";

const app = document.querySelector<HTMLElement>("#app");
if (!app) throw new Error("index.html has no #app");

app.append(
  stage([
    block({ x: 0, y: 0, w: 12, d: 12, h: 0.5, tone: "#e9e9e6" }),
    block({ x: 2, y: 2, z: 0.5, w: 3, d: 3, h: 4, tone: "#f4f4f2" }),
    block({ x: 7, y: 2, z: 0.5, w: 3, d: 3, h: 2, tone: "#f4f4f2" }),
    block({ x: 2, y: 7, z: 0.5, w: 3, d: 3, h: 1, tone: "#3d7fe5" }),
  ]),
);
