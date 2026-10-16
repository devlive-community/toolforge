/**
 * 在线安装插件的界面构建：输出 dist/index.js（ES module，默认导出插件根组件）与 dist/index.css。
 *
 * React、react-i18next、@toolforge/ui 与插件 SDK 由宿主在 window.__TF_SHARED__ 中提供，
 * 构建时改写为从该对象取值（导出名在构建时列出），保证与宿主共用同一份 React 与组件库。
 * 样式按宿主的设计 Token 只生成插件用到的工具类，不包含 preflight。
 *
 * 用法：在插件的 ui 目录执行 `vite build --config <本文件>`。
 */
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { defineConfig, type Plugin } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import ts from 'typescript'
import { SHARED_MODULES } from './shared.ts'

const root = process.cwd()
const here = path.dirname(fileURLToPath(import.meta.url))
const work = path.join(root, '.tf-build')
const PREFIX = '\0tf-shared:'

/** 源码形式的工作区包：解析入口文件中的值导出（忽略 type 导出） */
const SOURCE_ENTRIES: Record<string, string> = {
  '@toolforge/ui': '../ui/src/index.ts',
  '@toolforge/plugin-ui-sdk': '../plugin-ui-sdk/src/index.ts',
}

function sourceExports(file: string): string[] {
  const text = readFileSync(file, 'utf8')
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.Latest)
  const names: string[] = []
  for (const statement of source.statements) {
    if (ts.isExportDeclaration(statement) && !statement.isTypeOnly && statement.exportClause && ts.isNamedExports(statement.exportClause)) {
      for (const element of statement.exportClause.elements) if (!element.isTypeOnly) names.push(element.name.text)
    }
    const modifiers = ts.canHaveModifiers(statement) ? ts.getModifiers(statement) : undefined
    if (!modifiers?.some((m) => m.kind === ts.SyntaxKind.ExportKeyword)) continue
    if ((ts.isFunctionDeclaration(statement) || ts.isClassDeclaration(statement)) && statement.name) names.push(statement.name.text)
    if (ts.isVariableStatement(statement)) {
      for (const declaration of statement.declarationList.declarations) if (ts.isIdentifier(declaration.name)) names.push(declaration.name.text)
    }
  }
  return names
}

async function exportsOf(name: string): Promise<string[]> {
  const entry = SOURCE_ENTRIES[name]
  if (entry) return sourceExports(path.join(here, entry))
  return Object.keys(await import(name)).filter((key) => key !== 'default' && /^[A-Za-z_$][\w$]*$/.test(key))
}

const sharedExports = Object.fromEntries(await Promise.all(SHARED_MODULES.map(async (name) => [name, await exportsOf(name)] as const)))

function shared(): Plugin {
  return {
    name: 'toolforge-shared-modules',
    enforce: 'pre',
    resolveId(id) {
      return (SHARED_MODULES as readonly string[]).includes(id) ? PREFIX + id : null
    },
    load(id) {
      if (!id.startsWith(PREFIX)) return null
      const name = id.slice(PREFIX.length)
      const key = JSON.stringify(name)
      const names = sharedExports[name] ?? []
      return [
        `const m = window.__TF_SHARED__[${key}];`,
        `if (!m) throw new Error('ToolForge host does not share ' + ${key});`,
        'export default m.default ?? m;',
        names.length ? `export const { ${names.join(', ')} } = m;` : '',
      ].join('\n')
    },
  }
}

// 入口：引入样式并重新导出插件根组件
mkdirSync(work, { recursive: true })
const src = path.join(root, 'src').replaceAll('\\', '/')
writeFileSync(
  path.join(work, 'entry.css'),
  `@import "tailwindcss/theme.css" layer(theme);\n@import "tailwindcss/utilities.css" layer(utilities);\n@import "@toolforge/ui/tokens.css";\n@source "${src}";\n`,
)
writeFileSync(path.join(work, 'entry.ts'), `import './entry.css'\nexport { default } from '../src/index'\n`)

export default defineConfig({
  root,
  plugins: [shared(), react(), tailwindcss()],
  define: { 'process.env.NODE_ENV': JSON.stringify('production') },
  build: {
    outDir: path.join(root, 'dist'),
    emptyOutDir: true,
    target: 'safari15',
    lib: {
      entry: path.join(work, 'entry.ts'),
      formats: ['es'],
      fileName: () => 'index.js',
      cssFileName: 'index',
    },
  },
})
