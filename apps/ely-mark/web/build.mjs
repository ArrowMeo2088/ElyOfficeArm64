// Builds the page into the folder given; no network.
import { build } from 'esbuild'
import { copyFile, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

const out = process.argv[2]
if (!out) throw new Error('usage: node build.mjs <output folder>')
await rm(out, { recursive: true, force: true })
await mkdir(out, { recursive: true })

const built = await build({
  entryPoints: ['src/main.js', 'src/diagram.js', 'src/main.css'],
  outdir: out,
  bundle: true,
  minify: true,
  format: 'iife',
  target: 'chrome140',
  loader: { '.woff2': 'file', '.woff': 'file', '.ttf': 'file' },
  assetNames: 'fonts/[name]-[hash]',
  legalComments: 'none',
  metafile: true,
  logLevel: 'warning',
})
// Exports carry one stylesheet: fonts inline, woff2 alone.
const exported = await build({
  entryPoints: ['src/export.css'],
  outdir: out,
  bundle: true,
  minify: true,
  loader: { '.woff2': 'dataurl', '.woff': 'empty', '.ttf': 'empty' },
  legalComments: 'none',
  metafile: true,
  logLevel: 'warning',
})
const sheet = join(out, 'export.css')
// The dropped woff and ttf left empty sources.
await writeFile(sheet, (await readFile(sheet, 'utf8')).replace(/,url\(\) format\("(?:woff|truetype)"\)/g, ''))
await copyFile('index.html', join(out, 'index.html'))

/** Each bundled package's folder under node_modules. */
function packages(inputs) {
  const found = new Set()
  for (const input of inputs) {
    const at = input.lastIndexOf('node_modules/')
    if (at < 0) continue
    const parts = input.slice(at + 13).split('/')
    const name = parts[0].startsWith('@') ? `${parts[0]}/${parts[1]}` : parts[0]
    found.add(input.slice(0, at + 13) + name)
  }
  return [...found].sort()
}

/** Name, version and license; the file, else the manifest's. */
async function notice(folder) {
  const manifest = JSON.parse(await readFile(join(folder, 'package.json'), 'utf8'))
  const head = `${manifest.name} ${manifest.version} (${manifest.license})`
  const file = (await readdir(folder)).find((name) => /^(licen[cs]e|copying)/i.test(name))
  if (file) return `${head}\n\n${(await readFile(join(folder, file), 'utf8')).trim()}\n`
  const author = manifest.author?.name ?? manifest.author
  if (manifest.license !== 'MIT' || !author) throw new Error(`${manifest.name}: no license file`)
  console.log(`${manifest.name} ships no license file; MIT from its manifest`)
  return `${head}\n\nCopyright (c) ${author}\n\n${MIT}\n`
}

const MIT = `Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.`

const folders = packages([
  ...Object.keys(built.metafile.inputs),
  ...Object.keys(exported.metafile.inputs),
])
const notices = await Promise.all(folders.map(notice))
await writeFile(join(out, 'THIRD-PARTY-NOTICES.txt'), notices.join('\n' + '-'.repeat(72) + '\n\n'))
console.log(`built ${folders.length} packages into ${out}`)
