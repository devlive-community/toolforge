import { dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { FlatCompat } from '@eslint/eslintrc'

// docs 是独立的 Next.js 站点，必须自带 ESLint 配置，
// 否则构建时会向上查找到仓库根的 Tauri 插件规则（如禁用 localStorage 的 no-restricted-globals）
const baseDirectory = dirname(fileURLToPath(import.meta.url))

const compat = new FlatCompat({ baseDirectory })

const eslintConfig = [
  {
    ignores: ['.next/**', 'out/**', 'node_modules/**', 'src/data/**', 'scripts/**'],
  },
  ...compat.extends('next/core-web-vitals', 'next/typescript'),
  {
    // 静态导出 + unoptimized images：next/image 的优化管线不适用，
    // 站点图标均为小体积 SVG，直接用原生 <img>
    rules: { '@next/next/no-img-element': 'off' },
  },
]

export default eslintConfig
