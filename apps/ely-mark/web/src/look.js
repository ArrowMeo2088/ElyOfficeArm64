// The writing view: focus, typewriter, width, font, size (MARK-09).

let typewriter = false

/** Applies `look`; the Markdown never changes. */
export function apply(look) {
  const root = document.documentElement
  root.dataset.focus = look.focus
  root.dataset.width = look.width
  root.dataset.font = look.font
  root.dataset.typewriter = look.typewriter
  typewriter = look.typewriter
  const sheet =
    document.getElementById('ely-look') ??
    document.head.appendChild(Object.assign(document.createElement('style'), { id: 'ely-look' }))
  sheet.textContent = `:root { --ely-text-scale: ${look.size / 100}; }`
}

/** Typewriter scrolling: the caret's line stays mid-view. */
export function follow(editor) {
  if (typewriter) editor?.center?.()
}
