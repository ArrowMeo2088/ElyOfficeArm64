// CodeMirror colors from the page's Ely tokens.
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language'
import { EditorView } from '@codemirror/view'
import { tags } from '@lezer/highlight'

const token = (name) => `var(--ely-${name})`

const colors = HighlightStyle.define([
  { tag: [tags.keyword, tags.operatorKeyword, tags.modifier], color: token('keyword') },
  { tag: [tags.string, tags.regexp, tags.special(tags.string)], color: token('string') },
  { tag: [tags.number, tags.bool, tags.null, tags.atom], color: token('number') },
  { tag: [tags.comment, tags.meta], color: token('comment') },
  {
    tag: [tags.function(tags.variableName), tags.function(tags.propertyName)],
    color: token('function'),
  },
  { tag: [tags.typeName, tags.className, tags.namespace], color: token('type') },
  { tag: [tags.tagName, tags.heading], color: token('tag'), fontWeight: 'bold' },
  { tag: [tags.attributeName, tags.propertyName], color: token('attribute') },
  {
    tag: [tags.punctuation, tags.bracket, tags.processingInstruction],
    color: token('punctuation'),
  },
  { tag: [tags.link, tags.url], color: token('link') },
  { tag: tags.emphasis, fontStyle: 'italic' },
  { tag: tags.strong, fontWeight: 'bold' },
  { tag: tags.strikethrough, textDecoration: 'line-through' },
])

const frame = EditorView.theme({
  '&': { color: token('fg'), backgroundColor: 'transparent' },
  '.cm-content': { caretColor: token('fg'), fontFamily: 'var(--ely-mono), monospace' },
  '.cm-cursor': { borderLeftColor: token('fg') },
  '.cm-gutters': { backgroundColor: 'transparent', color: token('fg-subtle'), border: 'none' },
  '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: token('hover') },
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground': {
    backgroundColor: token('selection'),
  },
  '&.cm-focused': { outline: 'none' },
})

export const theme = [frame, syntaxHighlighting(colors)]
