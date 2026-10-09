// Untitled images: remark gives null, the schema wants text.
import { imageSchema } from '@milkdown/kit/preset/commonmark'

export const image = imageSchema.extendSchema((prev) => (ctx) => {
  const base = prev(ctx)
  return {
    ...base,
    parseMarkdown: {
      match: base.parseMarkdown.match,
      runner: (state, node, type) =>
        state.addNode(type, { src: node.url, alt: node.alt ?? '', title: node.title ?? '' }),
    },
  }
})
