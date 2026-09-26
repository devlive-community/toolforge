import { useEffect, useRef, useState } from 'react'
import { Spinner, cn } from '@toolforge/ui'
import { formatBytes, usePlugin } from '@toolforge/plugin-ui-sdk'
import { CATEGORY_CLASS, type Tile } from './types'

interface TreemapProps {
  scan: number
  node: number
  version: number
  selected: Set<number>
  onOpen: (id: number) => void
}

/** 树图：布局由 Rust 计算，这里只负责按容器大小请求并绘制 */
export function Treemap({ scan, node, version, selected, onOpen }: TreemapProps) {
  const { t, call } = usePlugin()
  const box = useRef<HTMLDivElement>(null)
  const [size, setSize] = useState<{ w: number; h: number } | null>(null)
  const [tiles, setTiles] = useState<{ key: string; tiles: Tile[] } | null>(null)

  useEffect(() => {
    const element = box.current
    if (!element) return
    const observer = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect
      setSize({ w: Math.floor(width), h: Math.floor(height) })
    })
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  const key = size ? `${scan}:${node}:${version}:${size.w}x${size.h}` : ''
  useEffect(() => {
    if (!size || size.w < 10 || size.h < 10) return
    let alive = true
    const timer = setTimeout(() => {
      call<Tile[]>('treemap', { id: scan, node, width: size.w, height: size.h })
        .then((result) => alive && setTiles({ key, tiles: result }))
        .catch(() => {})
    }, 80)
    return () => {
      alive = false
      clearTimeout(timer)
    }
  }, [key]) // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div ref={box} className="relative h-full w-full overflow-hidden rounded-control bg-surface-2">
      {!tiles && (
        <div className="flex h-full items-center justify-center">
          <Spinner />
        </div>
      )}
      {tiles?.tiles.map((tile, index) => {
        const label = tile.id === null ? t('treemap.merged', { count: tile.merged }) : tile.name
        const title = `${label} · ${formatBytes(tile.size)}`
        const style = { left: tile.x, top: tile.y, width: tile.w, height: tile.h }
        if (tile.dir) {
          return (
            <button
              key={`${tile.id}-${index}`}
              type="button"
              title={title}
              onClick={() => tile.id !== null && onOpen(tile.id)}
              className={cn(
                // 按钮默认垂直居中内容，文件夹名称需要固定在顶部标题栏
                'absolute flex flex-col items-stretch justify-start overflow-hidden rounded-[3px] border border-border text-left outline-none hover:border-primary focus-visible:ring-2 focus-visible:ring-ring',
                tile.depth % 2 === 0 ? 'bg-surface' : 'bg-surface-2',
                tile.id !== null && selected.has(tile.id) && 'ring-2 ring-danger',
              )}
              style={style}
            >
              {tile.h > 16 && tile.w > 30 && (
                <span className="block truncate px-1 text-[11px] font-medium text-fg leading-[16px]">{`${tile.name} · ${formatBytes(tile.size)}`}</span>
              )}
            </button>
          )
        }
        return (
          <div
            key={`${tile.id}-${index}`}
            title={title}
            className={cn(
              'absolute overflow-hidden rounded-[2px] border border-surface/60',
              tile.id === null ? 'bg-active' : CATEGORY_CLASS[tile.category],
              tile.id !== null && selected.has(tile.id) && 'ring-2 ring-danger ring-inset',
            )}
            style={style}
          >
            {tile.w > 48 && tile.h > 28 && (
              <span className={cn('block truncate px-1 pt-0.5 text-[10px] leading-tight', tile.id === null ? 'text-fg-muted' : 'text-on-tile')}>
                {label}
                <br />
                {formatBytes(tile.size)}
              </span>
            )}
          </div>
        )
      })}
    </div>
  )
}
