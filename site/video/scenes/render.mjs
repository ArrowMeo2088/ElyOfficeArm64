// Renders iso scenes to PNG frames, faces and MP4s.
import { spawnSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";

const here = dirname(fileURLToPath(import.meta.url));
const site = resolve(here, "../..");
const out = resolve(here, "../out/scenes");
const PAGES = 6;

const server = await createServer({ root: site, configFile: false, logLevel: "error", server: { port: 0 } });
await server.listen();
const url = `${server.resolvedUrls.local[0]}video/scenes/index.html`;
const browser = await chromium.launch();

/** A film-sized page; any page error stops the run. */
async function open() {
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  page.on("pageerror", (e) => {
    throw e;
  });
  page.on("console", (m) => {
    if (m.type() === "error") throw new Error(m.text());
  });
  await page.goto(url, { waitUntil: "networkidle" });
  await page.evaluate(() => document.fonts.ready);
  return page;
}

const pages = await Promise.all(Array.from({ length: PAGES }, open));
const all = await pages[0].evaluate(() => window.ely.scenes.map((s) => ({ ...s, range: window.ely.frames(s) })));
const wanted = process.argv.slice(2);
const unknown = wanted.filter((name) => !all.some((s) => s.name === name));
if (unknown.length) throw new Error(`unknown scenes: ${unknown.join(", ")}`);
const scenes = wanted.length ? all.filter((s) => wanted.includes(s.name)) : all;

/** Renders one scene's frames on `page`, then encodes it. */
async function scene(page, s) {
  const dir = join(out, s.name);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const [first, end] = s.range;
  const faces = [];
  for (let f = first; f < end; f++) {
    const face = await page.evaluate(([name, f]) => window.ely.render(name, f), [s.name, f]);
    await page.screenshot({ path: join(dir, `${String(f - first).padStart(5, "0")}.png`) });
    if (face) faces.push({ frame: f, local: f - first, t: f / 30, ...face });
  }
  if (s.app) {
    const json = { scene: s.name, app: s.app, fps: 30, width: 1920, height: 1080, first, frames: faces };
    writeFileSync(join(out, `${s.name}.json`), JSON.stringify(json, null, 1));
  }
  const mp4 = join(out, `${s.name}.mp4`);
  const args = ["-y", "-v", "error", "-framerate", "30", "-i", join(dir, "%05d.png")];
  args.push("-c:v", "libx264", "-preset", "slow", "-crf", "14", "-pix_fmt", "yuv420p", "-r", "30", "-movflags", "+faststart", mp4);
  const run = spawnSync("ffmpeg", args, { stdio: "inherit" });
  if (run.status !== 0) throw new Error(`ffmpeg failed for ${s.name}`);
  console.log(`${s.name}: frames ${first}..${end - 1} (${end - first}), ${((end - first) / 30).toFixed(3)} s`);
  return { name: s.name, app: s.app ?? null, bars: [s.from, s.to], first, frames: end - first };
}

const queue = [...scenes];
const done = [];
await Promise.all(
  pages.map(async (page) => {
    for (let s = queue.shift(); s; s = queue.shift()) done.push(await scene(page, s));
  }),
);
if (!wanted.length) {
  done.sort((a, b) => a.first - b.first);
  writeFileSync(join(out, "scenes.json"), JSON.stringify({ fps: 30, bpm: 112, scenes: done }, null, 1));
}
await browser.close();
await server.close();
