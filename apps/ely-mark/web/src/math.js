// KaTeX math, from Milkdown Crepe's latex feature.
import katex from 'katex'
import remarkMath from 'remark-math'
import { visit } from 'unist-util-visit'
import { $nodeSchema, $remark } from '@milkdown/kit/utils'
import { codeBlockSchema } from '@milkdown/kit/preset/commonmark'
import { failure } from './failure.js'

/** Saved source as HTML; a bad one shows where. */
export function render(source, display, dom) {
  try {
    katex.render(source, dom, { displayMode: display, throwOnError: true })
  } catch (error) {
    const at = error instanceof katex.ParseError ? error.position : undefined
    dom.replaceChildren(failure(source, error.message, at))
  }
  return dom
}

// remark-math, plus each inline formula's `$` count from the source.
const remark = $remark('remarkMath', () => function (options) {
  remarkMath.call(this, options)
  return (tree, file) => {
    const text = String(file.value)
    visit(tree, 'inlineMath', (node) => {
      let at = node.position?.start.offset ?? 0
      let fence = 0
      while (text[at++] === '$') fence++
      node.data = { ...node.data, fence: Math.max(fence, 1) }
    })
  }
})

// Block math becomes a code block in language LaTeX.
const blocks = $remark(
  'remarkMathBlock',
  () => () => (tree) =>
    visit(tree, 'math', (node, index, parent) => {
      parent.children.splice(index, 1, {
        type: 'code',
        lang: 'LaTeX',
        value: node.value,
        position: node.position,
      })
    }),
)

const block = codeBlockSchema.extendSchema((prev) => (ctx) => {
  const base = prev(ctx)
  return {
    ...base,
    toMarkdown: {
      match: base.toMarkdown.match,
      runner: (state, node) => {
        if ((node.attrs.language ?? '').toLowerCase() !== 'latex') {
          return base.toMarkdown.runner(state, node)
        }
        state.addNode('math', undefined, node.content.firstChild?.text || '')
      },
    },
  }
})

const inline = $nodeSchema('math_inline', () => ({
  group: 'inline',
  inline: true,
  atom: true,
  attrs: { value: { default: '' }, fence: { default: 1 } },
  parseDOM: [
    {
      tag: 'span[data-type="math_inline"]',
      getAttrs: (dom) => ({ value: dom.dataset.value ?? '', fence: Number(dom.dataset.fence ?? 1) }),
    },
  ],
  toDOM: (node) => {
    const dom = document.createElement('span')
    dom.dataset.type = 'math_inline'
    dom.dataset.value = node.attrs.value
    dom.dataset.fence = node.attrs.fence
    // `$$…$$` displays, as GitHub shows it.
    return render(node.attrs.value, node.attrs.fence > 1, dom)
  },
  parseMarkdown: {
    match: (node) => node.type === 'inlineMath',
    runner: (state, node, type) =>
      state.addNode(type, { value: node.value, fence: node.data?.fence ?? 1 }),
  },
  toMarkdown: {
    match: (node) => node.type.name === 'math_inline',
    runner: (state, node) => {
      const { value, fence } = node.attrs
      if (fence === 1) return state.addNode('inlineMath', undefined, value)
      // The stringifier knows one `$`; the source's own count stays.
      const dollars = '$'.repeat(fence)
      state.addNode('html', undefined, `${dollars}${value}${dollars}`)
    },
  },
}))

export const math = [remark, blocks, block, inline].flat()
