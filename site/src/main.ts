import "./style.css";
import "./iso.css";
import { block, stage } from "./iso.ts";
import { grow } from "./beat.ts";
import { model } from "./model.ts";
import { touch } from "./touch.ts";
import { regions, reveal } from "./regions.ts";
import { said, speak, type Lang } from "./text.ts";

const REPO = "https://github.com/ZacharyZhang-NY/ElyOffice";
const RELEASE = `${REPO}/releases/download/v0.1.0`;

const app = document.querySelector<HTMLElement>("#app");
if (!app) throw new Error("index.html has no #app");

const apps = model.slice(1, -1);
const built = model.map((part) => part.blocks.map(block));
grow(built);
const parts = new Map(apps.map((p) => [p.name, built[model.indexOf(p)]]));

const hero = document.createElement("header");
hero.className = "hero";
const title = document.createElement("h1");
title.textContent = "Ely Office";
const host = document.createElement("div");
host.className = "model";
const svg = stage(built.flat());
svg.classList.add("grow", "seen");
svg.setAttribute("aria-hidden", "true");
host.append(svg);
const github = document.createElement("a");
github.href = REPO;
github.textContent = "GitHub";
hero.append(title, said("p", "tagline"), said("p", "platforms"), github, host);
touch(host, svg, parts);

const sections = regions();

const footer = document.createElement("footer");
footer.id = "download";
const version = document.createElement("h2");
version.textContent = "Ely Office 0.1.0";
const list = document.createElement("ul");
for (const p of apps) {
  const a = document.createElement("a");
  a.href = `${RELEASE}/${p.name}-0.1.0-arm64.dmg`;
  a.textContent = p.name;
  const li = document.createElement("li");
  li.append(a);
  list.append(li);
}
const source = said("a", "source");
source.href = REPO;
const li = document.createElement("li");
li.append(source);
list.append(li);
footer.append(version, said("p", "platforms"), said("p", "needs"), said("p", "unsigned"), list);

const toggle = said("button", "other");
toggle.className = "lang";
let lang: Lang = navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
toggle.addEventListener("click", () => {
  lang = lang === "en" ? "zh" : "en";
  speak(lang);
});

const main = document.createElement("main");
main.append(...sections);
app.append(toggle, hero, main, footer);
speak(lang);
reveal(sections);
