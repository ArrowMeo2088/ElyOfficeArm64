// CodeMirror 6: the Markdown source.
import { basicSetup, EditorView } from 'codemirror'
import { Prec } from '@codemirror/state'
import { indentWithTab, redo, redoDepth, undoDepth } from '@codemirror/commands'
import { keymap } from '@codemirror/view'
import { markdown as language } from '@codemirror/lang-markdown'
import { languages } from '@codemirror/language-data'
import { ensureSyntaxTree } from '@codemirror/language'
import { theme } from './theme.js'
import { search } from '@codemirror/search'
import { textFinder } from './find.js'

/** Alt text that cannot close its brackets. */
const escape = (alt) => alt.replace(/[\\[\]]/g, '\\$&')

export function source(root, markdown, changed, edge) {
  const view = new EditorView({
    doc: markdown,
    extensions: [
      basicSetup,
      language({ codeLanguages: languages }),
      EditorView.lineWrapping,
      theme,
      search(),
      // Tab indents; it never leaves the editor.
      keymap.of([indentWithTab]),
      // Past their history, undo and redo reach the page.
      Prec.low(
        keymap.of([
          { key: 'Mod-z', run: (view) => undoDepth(view.state) === 0 && edge('undo') },
          { key: 'Mod-Shift-z', run: (view) => redoDepth(view.state) === 0 && edge('redo') },
          { key: 'Mod-y', run: (view) => redoDepth(view.state) === 0 && edge('redo') },
        ]),
      ),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) changed(() => update.state.doc.toString())
      }),
    ],
    parent: root,
  })
  // ATX and setext headings from the parse, with their starts.
  const headings = () => {
    const found = []
    const tree = ensureSyntaxTree(view.state, view.state.doc.length, 200)
    tree?.iterate({
      enter: ({ name, from, to }) => {
        const level = /^(?:ATX|Setext)Heading(\d)$/.exec(name)?.[1]
        if (!level) return
        const head = view.state.doc.sliceString(from, to).split('\n')[0]
        const title = head.replace(/^#{1,6}[ \t]*/, '').replace(/[ \t]+#+[ \t]*$/, '')
        found.push({ level: Number(level), title: title.trim(), from })
        return false
      },
    })
    return found
  }
  const reveal = (from) => {
    view.dispatch({ selection: { anchor: from }, scrollIntoView: true })
    view.focus()
  }
  return {
    // Images at the caret, as Markdown.
    insert: (images) => {
      const text = images.map(({ src, alt }) => `![${escape(alt)}](${src})`).join('\n')
      view.dispatch(view.state.replaceSelection(text))
      view.focus()
      return true
    },
    find: textFinder(view),
    outline: () => headings().map(({ level, title }) => ({ level, title })),
    heading: (key) => {
      const found = headings()[key]
      if (found) reveal(found.from)
      return Boolean(found)
    },
    line: (n, query) => {
      if (n < 1 || n > view.state.doc.lines) return false
      const line = view.state.doc.line(n)
      const column = line.text.toLowerCase().indexOf(query.toLowerCase())
      reveal(line.from + Math.max(0, column))
      return true
    },
    close: () => view.destroy(),
    center: () => {
      const { head } = view.state.selection.main
      view.dispatch({ effects: EditorView.scrollIntoView(head, { y: 'center' }) })
    },
    where: () => {
      const { head } = view.state.selection.main
      const line = view.state.doc.lineAt(head)
      return { line: line.number, column: head - line.from }
    },
    at: (n, column) => {
      if (n < 1 || n > view.state.doc.lines) return false
      const line = view.state.doc.line(n)
      reveal(line.from + Math.min(column, line.length))
      return true
    },
    text: () => view.state.doc.toString(),
    // The text once every undone step is redone.
    redone: () => {
      let state = view.state
      if (!redoDepth(state)) return null
      while (redoDepth(state)) redo({ state, dispatch: (tr) => (state = tr.state) })
      return state.doc.toString()
    },
    focus: () => view.focus(),
  }
}
