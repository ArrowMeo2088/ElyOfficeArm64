// Find and replace in either editor (MARK-06).
import {
  SearchQuery as TextQuery,
  findNext as nextText,
  findPrevious as previousText,
  replaceAll as allText,
  replaceNext as oneText,
  setSearchQuery,
} from '@codemirror/search'
import {
  SearchQuery as DocQuery,
  findNext as nextDoc,
  findPrev as previousDoc,
  replaceAll as allDoc,
  replaceNext as oneDoc,
  setSearchState,
} from 'prosemirror-search'
import { TextSelection } from '@milkdown/kit/prose/state'

/** Why a pattern fails under the editors' `gmu` flags. */
function broken(spec) {
  if (!spec.regexp || !spec.search) return null
  try {
    new RegExp(spec.search, 'gmu')
    return null
  } catch (error) {
    return error.message
  }
}

/** Whether two specs find the same hits. */
export function sameQuery(a, b) {
  return ['search', 'caseSensitive', 'regexp', 'wholeWord'].every((key) => a[key] === b[key])
}

/** One character's width at `pos`: two for a surrogate pair. */
function width(doc, pos) {
  const next = doc.textBetween(pos, Math.min(pos + 2, doc.content.size), '\n', '\n')
  return next.codePointAt(0) > 0xffff ? 2 : 1
}

/** Where the selection stands among `ranges`. */
function standing(ranges, from, to) {
  const at = ranges.findIndex((range) => range.from === from && range.to === to)
  return { current: at < 0 ? null : at, total: ranges.length }
}

/** Find in Milkdown's view; `prosemirror-search` is installed there. */
export function docFinder(view) {
  let spec = { search: '' }
  const query = () => new DocQuery(spec)
  const ranges = () => {
    const found = []
    const asked = query()
    if (!asked.valid) return found
    const { doc } = view.state
    for (let at = 0; ;) {
      const hit = asked.findNext(view.state, at)
      // Mid-pair, a Unicode pattern answers before `at`.
      if (!hit || hit.from < at) return found
      found.push(hit)
      at = hit.to > hit.from ? hit.to : hit.to + width(doc, hit.to)
    }
  }
  const status = () => {
    const error = broken(spec)
    if (error) return { error }
    const { from, to } = view.state.selection
    return standing(ranges(), from, to)
  }
  const run = (command) => command(view.state, view.dispatch)
  return {
    seek(next) {
      spec = next
      const { from } = view.state.selection
      const tr = setSearchState(view.state.tr, query())
      // From the hit's start: the hit stays while typing.
      tr.setSelection(TextSelection.near(tr.doc.resolve(from)))
      view.dispatch(tr)
      if (query().valid) run(nextDoc)
      return status()
    },
    step(forward) {
      if (query().valid) run(forward ? nextDoc : previousDoc)
      return status()
    },
    replace(all) {
      if (query().valid) run(all ? allDoc : oneDoc)
      return status()
    },
    clear() {
      spec = { search: '' }
      view.dispatch(setSearchState(view.state.tr, query()))
    },
  }
}

/** Find in CodeMirror; its `search()` extension is installed there. */
export function textFinder(view) {
  let spec = { search: '' }
  const valid = () => new TextQuery(spec).valid
  const ranges = () => {
    const found = []
    const asked = new TextQuery(spec)
    if (!asked.valid) return found
    const cursor = asked.getCursor(view.state)
    for (let next = cursor.next(); !next.done; next = cursor.next()) found.push(next.value)
    return found
  }
  const status = () => {
    const error = broken(spec)
    if (error) return { error }
    const { from, to } = view.state.selection.main
    return standing(ranges(), from, to)
  }
  return {
    seek(next) {
      spec = next
      const selection = { anchor: view.state.selection.main.from }
      view.dispatch({ effects: setSearchQuery.of(new TextQuery(spec)), selection })
      // CodeMirror opens its own panel for invalid queries.
      if (valid()) nextText(view)
      return status()
    },
    step(forward) {
      if (valid()) (forward ? nextText : previousText)(view)
      return status()
    },
    replace(all) {
      if (valid()) (all ? allText : oneText)(view)
      return status()
    },
    clear() {
      spec = { search: '' }
      view.dispatch({ effects: setSearchQuery.of(new TextQuery(spec)) })
    },
  }
}
