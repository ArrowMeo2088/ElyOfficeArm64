// Myers' diff over two lists.

/** Equal pairs between `a` and `b`, in order. */
export function common(a, b, same) {
  const n = a.length
  const m = b.length
  const max = n + m
  const reach = new Map([[1, 0]])
  const trace = []
  for (let d = 0; d <= max; d++) {
    trace.push(new Map(reach))
    for (let k = -d; k <= d; k += 2) {
      const down = k === -d || (k !== d && reach.get(k - 1) < reach.get(k + 1))
      let x = down ? reach.get(k + 1) : reach.get(k - 1) + 1
      let y = x - k
      while (x < n && y < m && same(a[x], b[y])) {
        x += 1
        y += 1
      }
      reach.set(k, x)
      if (x >= n && y >= m) return walk(trace, n, m)
    }
  }
  throw new Error('diff did not end')
}

/** Back through the trace: each equal pair, in order. */
function walk(trace, n, m) {
  const pairs = []
  let x = n
  let y = m
  for (let d = trace.length - 1; d > 0; d--) {
    const reach = trace[d]
    const k = x - y
    const down = k === -d || (k !== d && reach.get(k - 1) < reach.get(k + 1))
    const from = down ? k + 1 : k - 1
    const fromX = reach.get(from)
    const fromY = fromX - from
    while (x > fromX + (down ? 0 : 1) && y > fromY + (down ? 1 : 0)) {
      x -= 1
      y -= 1
      pairs.push([x, y])
    }
    x = fromX
    y = fromY
  }
  while (x > 0 && y > 0) {
    x -= 1
    y -= 1
    pairs.push([x, y])
  }
  return pairs.reverse()
}
