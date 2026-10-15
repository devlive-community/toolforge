import path from 'node:path'
import type { NextConfig } from 'next'

// 静态导出：产物在 out/，由 GitHub Pages 托管（自定义域名 toolforge.devlive.org，
// 域名根路径部署，因此不需要 basePath 与 assetPrefix）。
// outputFileTracingRoot 锁定到 docs 目录，避免上层仓库根的锁文件干扰 workspace 根推断。
const nextConfig: NextConfig = {
  output: 'export',
  trailingSlash: true,
  images: { unoptimized: true },
  outputFileTracingRoot: path.join(import.meta.dirname),
}

export default nextConfig
