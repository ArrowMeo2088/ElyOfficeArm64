// Code block icons, drawn in the text color.
const icon = (path) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${path}</svg>`

export const icons = {
  expandIcon: icon('<path d="m6 9 6 6 6-6"/>'),
  searchIcon: icon('<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>'),
  clearSearchIcon: icon('<path d="M18 6 6 18M6 6l12 12"/>'),
  copyIcon: icon(
    '<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/>',
  ),
}
