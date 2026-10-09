// Mermaid previews; each editor owns its records.
import { failure } from './failure.js'

// Beside this script; the page's base is the document's folder.
const here = document.currentScript.src

/** The Mermaid bundle, added once as a classic script. */
let loading
export function bundle() {
  loading ??= new Promise((resolve, reject) => {
    const script = document.createElement('script')
    script.src = new URL('diagram.js', here).href
    script.onload = () => resolve(window.elyDiagram)
    script.onerror = () => reject(new Error('diagram.js did not load'))
    document.head.append(script)
  })
  return loading
}

let queue = Promise.resolve()
let count = 0
const owners = new Set()

// New tokens mean a new theme; every owner redraws.
new MutationObserver(() => {
  for (const owner of owners) owner.redraw()
}).observe(document.documentElement, { attributes: true, attributeFilter: ['style'] })

/** A block's diagram, by its id on the page. */
const box = (id, inner) => `<div class="ely-diagram" data-diagram="${id}">${inner}</div>`
const placed = (id) => document.querySelector(`[data-diagram="${id}"]`) !== null
// After the code block's next render.
const settle = () => new Promise((done) => setTimeout(done))

/** One editor's diagrams; a result lands only on its placeholder. */
export function diagrams() {
  const records = new Map()

  /** Drops diagrams whose placeholder has left the page. */
  const sweep = () => {
    for (const [id, record] of records) {
      if (record.settled && !placed(id)) records.delete(id)
    }
  }

  /** Each draw has a turn; a later draw voids earlier. */
  const draw = (id, record) => {
    const turn = ++record.turn
    const live = () => records.get(id) === record && record.turn === turn && placed(id)
    queue = queue.then(async () => {
      await settle()
      if (!live()) return
      try {
        const render = await bundle()
        if (!live()) return
        const svg = await render(record.source)
        await settle()
        if (live()) record.apply(box(id, svg))
      } catch (error) {
        await settle()
        if (!live()) return
        records.delete(id)
        record.apply(failure(record.source, error.message))
      }
    })
  }

  const owner = {
    /** The placeholder for `source`; its diagram follows. */
    show(source, apply) {
      sweep()
      const id = `${++count}`
      const record = { source, apply, settled: false, turn: 0 }
      records.set(id, record)
      settle().then(() => (record.settled = true))
      draw(id, record)
      return box(id, '')
    },
    redraw() {
      sweep()
      for (const [id, record] of records) draw(id, record)
    },
    release() {
      records.clear()
      owners.delete(owner)
    },
  }
  owners.add(owner)
  return owner
}
