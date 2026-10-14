import { execFileSync } from 'node:child_process'
import { copyFileSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { createRequire } from 'node:module'
import { dirname, join, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const icons = join(root, 'apps/desktop/src-tauri/icons')
const source = join(icons, 'logo.png')
const desktop = join(root, 'apps/desktop')
const require = createRequire(join(desktop, 'package.json'))
const cli = require.resolve('@tauri-apps/cli/tauri.js')

const output = mkdtempSync(join(tmpdir(), 'toolforge-icons-'))
const generate = (input, destination, sizes = []) => {
  execFileSync(process.execPath, [cli, 'icon', input,
    '--output', destination, ...sizes.flatMap((size) => ['--png', String(size)]),
  ], { cwd: desktop, stdio: 'inherit' })
}

try {
  generate(source, output)
  // macOS recolors a template icon from its alpha channel, without a colored tile.
  const trayImage = readFileSync(join(output, 'Square44x44Logo.png')).toString('base64')
  const traySource = join(output, 'tray.svg')
  const trayOutput = join(output, 'tray')
  writeFileSync(traySource,
    `<svg xmlns="http://www.w3.org/2000/svg" width="44" height="44" viewBox="0 0 44 44"><defs><mask id="alpha" mask-type="alpha" maskUnits="userSpaceOnUse" x="0" y="0" width="44" height="44"><image href="data:image/png;base64,${trayImage}" width="44" height="44"/></mask></defs><rect width="44" height="44" fill="black" mask="url(#alpha)"/></svg>\n`)
  generate(traySource, trayOutput, [44])

  // Copy desktop resources only after all exports succeed.
  for (const name of [
    '32x32.png', '64x64.png', '128x128.png', '128x128@2x.png', 'icon.png',
    'icon.icns', 'icon.ico', 'StoreLogo.png',
    ...[30, 44, 71, 89, 107, 142, 150, 284, 310].map((size) => `Square${size}x${size}Logo.png`),
  ]) {
    copyFileSync(join(output, name), join(icons, name))
  }
  copyFileSync(join(trayOutput, '44x44.png'), join(icons, 'tray-template.png'))
} finally {
  rmSync(output, { recursive: true, force: true })
}
