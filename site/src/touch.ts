// Buildings lift on hover or focus and sink when pressed.
import { UNIT } from "./iso.ts";

/** Links each app's blocks to its region, in reading order. */
export function touch(host: HTMLElement, parts: Map<string, SVGGElement[]>): void {
  const caption = document.createElement("p");
  caption.className = "caption";
  caption.setAttribute("aria-hidden", "true");
  host.append(caption);
  host.style.setProperty("--step", `${UNIT / 2}px`);
  const nav = document.createElement("nav");
  nav.className = "doors";
  host.append(nav);

  for (const [name, blocks] of parts) {
    const target = `#${name.toLowerCase()}`;
    const inside = (node: EventTarget | null) =>
      node instanceof Element && node.closest<SVGGElement>(".block")?.dataset.part === name;
    const lift = (up: boolean) => {
      for (const b of blocks) b.classList.toggle("lifted", up);
      caption.classList.toggle("shown", up);
      if (up) place(caption, host, blocks, name);
    };
    const press = (down: boolean) => {
      for (const b of blocks) b.classList.toggle("pressed", down);
    };
    const open = () => {
      press(false);
      location.hash = target;
    };

    for (const b of blocks) {
      b.dataset.part = name;
      b.addEventListener("pointerenter", () => lift(true));
      b.addEventListener("pointerleave", (e) => inside(e.relatedTarget) || (press(false), lift(false)));
      b.addEventListener("pointerdown", () => press(true));
      b.addEventListener("pointerup", open);
    }
    const door = document.createElement("a");
    door.href = target;
    door.textContent = name;
    door.addEventListener("focus", () => lift(true));
    door.addEventListener("blur", () => (press(false), lift(false)));
    door.addEventListener("keydown", (e) => e.key === "Enter" && press(true));
    door.addEventListener("click", () => press(false));
    nav.append(door);
  }
}

/** Puts the caption above the building's top. */
function place(caption: HTMLElement, host: HTMLElement, blocks: SVGGElement[], name: string) {
  const origin = host.getBoundingClientRect();
  const rects = blocks.map((b) => b.getBoundingClientRect());
  const left = Math.min(...rects.map((r) => r.left));
  const right = Math.max(...rects.map((r) => r.right));
  const top = Math.min(...rects.map((r) => r.top));
  caption.textContent = name;
  caption.style.left = `${(left + right) / 2 - origin.left}px`;
  caption.style.top = `${top - origin.top}px`;
}
