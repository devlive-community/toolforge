import { cn } from '@toolforge/ui'
import { Wrench } from 'lucide-react'
import type { Manifest } from '@toolforge/plugin-ui-sdk'
import { pluginIcon } from './modules'

const tiles = {
  violet: 'tile-violet',
  blue: 'tile-blue',
  green: 'tile-green',
  orange: 'tile-orange',
  pink: 'tile-pink',
  cyan: 'tile-cyan',
} as const

/** 插件图标：内置插件图标为随包分发的 SVG（使用 currentColor） */
export function ToolGlyph({ manifest, className }: { manifest: Manifest; className?: string }) {
  const svg = pluginIcon(manifest.id)
  if (!svg) return <Wrench className={className} />
  return (
    <span
      aria-hidden
      className={cn('inline-flex [&>svg]:size-full', className)}
      dangerouslySetInnerHTML={{ __html: svg }}
    />
  )
}

export function ToolTile({ manifest, size = 'md' }: { manifest: Manifest; size?: 'sm' | 'md' | 'lg' }) {
  const tile = tiles[manifest.accent ?? 'green'] ?? tiles.green
  return (
    <span
      className={cn(
        'inline-flex shrink-0 items-center justify-center text-on-tile shadow-tile',
        tile,
        size === 'lg' && 'size-16 rounded-tile',
        size === 'md' && 'size-10 rounded-[11px]',
        size === 'sm' && 'size-8 rounded-[9px]',
      )}
    >
      <ToolGlyph
        manifest={manifest}
        className={cn(size === 'lg' ? 'size-8' : size === 'md' ? 'size-5' : 'size-4')}
      />
    </span>
  )
}
