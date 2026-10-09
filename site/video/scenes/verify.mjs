// Checks the leaving screens play their recordings at the cut.
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";
import recs from "../montage/recs.json" with { type: "json" };

const here = dirname(fileURLToPath(import.meta.url));
const server = await createServer({ root: resolve(here, "../.."), configFile: false, logLevel: "error", server: { port: 0 } });
await server.listen();
const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
  page.on("pageerror", (e) => {
    throw e;
  });
  // Any frame will do; only which frame is asked matters.
  await page.route("**/rec/**", (route) => route.fulfill({ path: resolve(here, "../../public/film/poster.jpg") }));
  await page.goto(`${server.resolvedUrls.local[0]}video/scenes/index.html`, { waitUntil: "networkidle" });
  // The first frame after the montage is the cut.
  const cut = recs.first + 129;
  for (const f of [cut, cut + 1, cut + 2]) {
    await page.evaluate((f) => window.ely.render("files", f), f);
    const hrefs = await page.$$eval("#world image", (els) => els.map((el) => el.getAttribute("href")));
    const want = String(f - recs.first + 1).padStart(5, "0");
    if (hrefs.length !== 6 || !hrefs.every((h) => h.endsWith(`/${want}.jpg`)) || new Set(hrefs).size !== 6) {
      throw new Error(`frame ${f}: ${hrefs.length} recordings ${JSON.stringify(hrefs)}`);
    }
  }
  console.log("verify: six screens play their own frames across the cut");
} finally {
  await browser.close();
  await server.close();
}
