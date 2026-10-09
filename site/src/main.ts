import "./style.css";
import "./iso.css";
import "./tour.css";
import { tour } from "./tour.ts";
import { props, reveal } from "./regions.ts";
import { night } from "./night.ts";
import { rhythm } from "./rhythm.ts";
import { playground } from "./play.ts";
import { film } from "./film.ts";
import { model } from "./model.ts";
import { said, speak, type Lang } from "./text.ts";

const REPO = "https://github.com/ZacharyZhang-NY/ElyOffice";
const RELEASE = `${REPO}/releases/download/v0.1.0`;
const APPS = model.slice(1, -1).map((p) => p.name);

const app = document.querySelector<HTMLElement>("#app");
if (!app) throw new Error("index.html has no #app");

const hero = document.createElement("header");
const title = document.createElement("h1");
title.textContent = "Ely Office";
const github = document.createElement("a");
github.href = REPO;
github.textContent = "GitHub";
hero.append(title, said("p", "tagline"), said("p", "platforms"), github);
const { section, host, svg } = tour(hero);
rhythm(host, svg);

const shelf = props();

const footer = document.createElement("footer");
footer.id = "download";
const version = document.createElement("h2");
version.textContent = "Ely Office 0.1.0";
const list = document.createElement("ul");
for (const name of APPS) {
  const a = document.createElement("a");
  a.href = `${RELEASE}/${name}-0.1.0-arm64.dmg`;
  a.textContent = name;
  const li = document.createElement("li");
  li.append(a);
  list.append(li);
}
for (const [key, file] of [
  ["source", "ElyOffice-0.1.0-source.tar.gz"],
  ["upstream", "ElyOffice-0.1.0-upstream-source.tar.gz"],
] as const) {
  const a = said("a", key);
  a.href = `${RELEASE}/${file}`;
  const li = document.createElement("li");
  li.append(a);
  list.append(li);
}
footer.append(version, said("p", "platforms"), said("p", "needs"), said("p", "unsigned"), list);

const toggle = said("button", "other");
toggle.className = "lang";
let lang: Lang = navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";
/** Writes the page in `next`; the toggle names the other. */
const say = (next: Lang) => {
  lang = next;
  speak(lang);
  toggle.lang = lang === "en" ? "zh-CN" : "en";
};
toggle.addEventListener("click", () => say(lang === "en" ? "zh" : "en"));

const main = document.createElement("main");
main.append(section, playground(), shelf, film());
app.append(night(), toggle, main, footer);
say(lang);
reveal([shelf]);
