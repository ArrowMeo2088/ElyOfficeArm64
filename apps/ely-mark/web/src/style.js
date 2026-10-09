// The document's own markers, so blocks write back unchanged.
import { unified } from 'unified'
import remarkParse from 'remark-parse'
import { visit } from 'unist-util-visit'

/** remark-stringify options most of `text` already uses. */
export function style(text) {
  const votes = {}
  const vote = (key, value) => {
    votes[key] ??= new Map()
    votes[key].set(value, (votes[key].get(value) ?? 0) + 1)
  }
  visit(unified().use(remarkParse).parse(text), (node) => {
    const at = node.position?.start.offset
    if (at === undefined) return
    const head = text.slice(at, at + 12)
    switch (node.type) {
      case 'listItem': {
        const ordered = /^\d+([.)])/.exec(head)
        if (ordered) vote('bulletOrdered', ordered[1])
        else if (/^[*+-]/.test(head)) vote('bullet', head[0])
        break
      }
      case 'emphasis':
      case 'strong':
        vote(node.type, head[0])
        break
      case 'code':
        if (/^[`~]/.test(head)) vote('fence', head[0])
        break
      case 'thematicBreak':
        vote('rule', head[0])
        break
      case 'heading':
        if (node.depth <= 2) vote('setext', head[0] !== '#')
        break
    }
  })
  const most = (key) => [...votes[key].entries()].sort((a, b) => b[1] - a[1])[0][0]
  return Object.fromEntries(Object.keys(votes).map((key) => [key, most(key)]))
}
