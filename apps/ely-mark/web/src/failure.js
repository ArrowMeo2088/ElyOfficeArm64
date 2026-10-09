/** The source as saved, and why it did not render. */
export function failure(source, message, at) {
  const box = document.createElement('div')
  box.className = 'ely-failure'
  const why = document.createElement('p')
  why.textContent = at === undefined ? message : `${message} (${at})`
  const code = document.createElement('pre')
  code.textContent = source
  box.append(why, code)
  return box
}
