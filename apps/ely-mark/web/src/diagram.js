// Mermaid, loaded on first use; only saved source renders.
import mermaid from 'mermaid'

let count = 0

/** The theme from the page's Ely tokens. */
function variables() {
  const style = getComputedStyle(document.documentElement)
  const token = (name) => style.getPropertyValue(`--ely-${name}`).trim()
  return {
    background: token('bg'),
    primaryColor: token('surface'),
    primaryTextColor: token('fg'),
    primaryBorderColor: token('border'),
    lineColor: token('fg-muted'),
    textColor: token('fg'),
    fontFamily: token('font'),
  }
}

/** `source` as SVG; `paper` draws Mermaid's light theme for export. */
window.elyDiagram = async (source, look) => {
  const paper = look === 'paper'
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: 'strict',
    suppressErrorRendering: true,
    theme: paper ? 'default' : 'base',
    themeVariables: paper ? {} : variables(),
  })
  count += 1
  const { svg } = await mermaid.render(`ely-diagram-${count}`, source)
  return svg
}
