// Code blocks in CodeMirror; LaTeX and Mermaid preview.
import { basicSetup, EditorView } from 'codemirror'
import { EditorState } from '@codemirror/state'
import { keymap } from '@codemirror/view'
import { indentWithTab } from '@codemirror/commands'
import { languages } from '@codemirror/language-data'
import { codeBlockConfig } from '@milkdown/kit/component/code-block'
import { theme } from './theme.js'
import { icons } from './icons.js'
import { render } from './math.js'

/** Previews for one editor's code blocks. */
function previews(owner) {
  return (language, source, apply) => {
    const kind = source.length === 0 ? '' : language.toLowerCase()
    switch (kind) {
      case 'latex':
        return render(source, true, document.createElement('div'))
      case 'mermaid':
        return owner.show(source, apply)
      default:
        return null
    }
  }
}

/** Milkdown's code block config; read-only in preview. */
export function code(texts, editable, owner) {
  const locked = editable ? [] : [EditorState.readOnly.of(true), EditorView.editable.of(false)]
  return (ctx) =>
    ctx.update(codeBlockConfig.key, (prev) => ({
      ...prev,
      ...icons,
      extensions: [basicSetup, keymap.of([indentWithTab]), theme, ...locked],
      languages,
      searchPlaceholder: texts.searchLanguage,
      noResultText: texts.noResult,
      copyText: texts.copy,
      previewLabel: texts.preview,
      previewLoading: texts.loading,
      previewToggleButton: (only) => (only ? texts.edit : texts.hide),
      renderPreview: previews(owner),
    }))
}
