import { useEffect, useRef, useState, type KeyboardEvent, type MouseEvent } from 'react'
import { Badge, Button, DropdownMenu, Empty, NumberInput, Panel, SegmentedControl, cn, toast } from '@toolforge/ui'
import { CopyButton, host, useCopy, useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Crosshair, Download, FolderOpen, ImageIcon, Palette, Pipette } from 'lucide-react'

interface Formats {
  hex: string
  rgb: string
  hsl: string
  oklch: string
  contrastWhite: number
  contrastBlack: number
  text: 'light' | 'dark'
}
interface Swatch extends Formats {
  channels: [number, number, number]
  share: number
}
interface Picked extends Formats {
  channels: [number, number, number]
  x: number
  y: number
  alpha: number
  loupe: (string | null)[]
}
interface Opened {
  path: string
  name: string
  width: number
  height: number
  preview: string
}

const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp', 'tif', 'tiff', 'ico']
const EXPORTS = ['css', 'scss', 'tailwind', 'json', 'gpl', 'hex'] as const
const RADII = [0, 1, 2, 4]
const LOUPE = 11
/** 取色标记与放大镜中心要在任何颜色上都看得清，固定用黑白描边 */
const MARKER = 'border-2 border-white shadow-[0_0_0_1px_rgb(0_0_0/0.6)]' // tf-allow: 取色标记需在任意颜色上可见
const LOUPE_CENTER = 'outline outline-1 outline-offset-[-1px] outline-white' // tf-allow: 放大镜中心框需在任意颜色上可见
const TEXT_ON = { light: '#ffffff', dark: '#000000' } // tf-allow: 色块上的示例文字随明暗切换黑白

function FormatRows({ color }: { color: Formats }) {
  const { t } = usePlugin()
  return (
    <div className="divide-y divide-border">
      {(['hex', 'rgb', 'hsl', 'oklch'] as const).map((key) => (
        <div key={key} className="group flex items-center gap-2 px-1 py-1.5">
          <span className="w-12 text-[11px] text-fg-subtle uppercase">{key}</span>
          <code className="min-w-0 flex-1 truncate font-mono text-[12.5px] text-fg" data-selectable>
            {color[key]}
          </code>
          <CopyButton text={color[key]} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" />
        </div>
      ))}
      <p className="px-1 pt-1.5 text-[11px] text-fg-subtle">
        {t('picked.contrast', { white: color.contrastWhite, black: color.contrastBlack })} · {t(`picked.text.${color.text}`)}
      </p>
    </div>
  )
}

function ImagePalette() {
  const { t, call, errorMessage } = usePlugin()
  const { copy } = useCopy()
  const [count, setCount] = usePluginState('count', 8)
  const [radius, setRadius] = usePluginState('radius', 0)
  const [image, setImage] = useState<Opened | null>(null)
  const [picked, setPicked] = useState<Picked | null>(null)
  const [loading, setLoading] = useState(false)
  const [hovering, setHovering] = useState(false)
  const imgRef = useRef<HTMLImageElement>(null)

  const open = async (path: string) => {
    setLoading(true)
    try {
      const opened = await call<Opened>('open', { path })
      setImage(opened)
      setPicked(null)
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setLoading(false)
    }
  }
  const openRef = useRef(open)
  useEffect(() => {
    openRef.current = open
  })
  useEffect(() => {
    const off = host.onFileDrop({ drop: (paths) => paths[0] && void openRef.current(paths[0]), over: setHovering })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])

  const paletteArgs = image ? { path: image.path, count } : null
  const palette = useDebouncedCall<Swatch[]>(paletteArgs ? 'palette' : null, paletteArgs, [image?.path, count])

  const pickAt = async (x: number, y: number) => {
    if (!image) return
    const cx = Math.max(0, Math.min(image.width - 1, Math.round(x)))
    const cy = Math.max(0, Math.min(image.height - 1, Math.round(y)))
    try {
      setPicked(await call<Picked>('pick', { path: image.path, x: cx, y: cy, radius }))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  // 预览图可能缩小过，按显示尺寸换算回原图坐标
  const onClick = (e: MouseEvent<HTMLImageElement>) => {
    if (!image) return
    const rect = e.currentTarget.getBoundingClientRect()
    void pickAt(((e.clientX - rect.left) / rect.width) * image.width - 0.5, ((e.clientY - rect.top) / rect.height) * image.height - 0.5)
  }
  const onKeyDown = (e: KeyboardEvent) => {
    if (!picked) return
    const step = e.shiftKey ? 10 : 1
    const delta: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }
    const d = delta[e.key]
    if (!d) return
    e.preventDefault()
    void pickAt(picked.x + d[0], picked.y + d[1])
  }
  const [seenRadius, setSeenRadius] = useState(radius)
  if (radius !== seenRadius) {
    setSeenRadius(radius)
    if (picked) void pickAt(picked.x, picked.y)
  }

  const pick = async () => {
    try {
      const path = await host.dialog.openFile([{ name: t('open.filter'), extensions: IMAGE_EXTENSIONS }])
      if (path) await open(path)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const exportAs = async (format: (typeof EXPORTS)[number]) => {
    if (!palette.result?.length) return
    try {
      const name = image?.name.replace(/\.[^.]+$/, '') ?? 'palette'
      const { text } = await call<{ text: string }>('export', { colors: palette.result.map((s) => s.hex), format, name })
      await copy(text)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_340px] gap-3">
      <Panel
        icon={<ImageIcon />}
        title={image ? image.name : t('image.title')}
        extra={image && <Badge>{t('image.size', { width: image.width, height: image.height })}</Badge>}
        actions={
          <Button size="sm" onClick={pick}>
            <FolderOpen />
            {t('image.open')}
          </Button>
        }
        footer={image && <span>{t('image.hint')}</span>}
        bodyClassName="p-3"
      >
        {!image ? (
          <button
            type="button"
            onClick={pick}
            disabled={loading}
            className={cn(
              'flex h-full min-h-40 w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
              'focus-visible:ring-2 focus-visible:ring-ring',
              hovering ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
            )}
          >
            <Pipette className="size-6" />
            <span className="text-[13px] font-medium">{t('image.drop')}</span>
            <span className="text-xs text-fg-subtle">{t('image.dropHint')}</span>
          </button>
        ) : (
          <div
            tabIndex={0}
            onKeyDown={onKeyDown}
            className={cn(
              'flex h-full min-h-0 items-center justify-center rounded-control outline-none focus-visible:ring-2 focus-visible:ring-ring',
              'bg-surface-2 bg-[conic-gradient(var(--color-border)_25%,transparent_0_50%,var(--color-border)_0_75%,transparent_0)] bg-[length:16px_16px]',
              hovering && 'ring-2 ring-primary',
            )}
          >
            <div className="relative max-h-full max-w-full">
              <img
                ref={imgRef}
                src={image.preview}
                alt=""
                draggable={false}
                onClick={onClick}
                className="block max-h-[calc(100vh-260px)] max-w-full cursor-crosshair object-contain"
              />
              {picked && (
                <span
                  className={cn('pointer-events-none absolute size-4 -translate-x-1/2 -translate-y-1/2 rounded-full', MARKER)}
                  style={{ left: `${((picked.x + 0.5) / image.width) * 100}%`, top: `${((picked.y + 0.5) / image.height) * 100}%` }}
                />
              )}
            </div>
          </div>
        )}
      </Panel>

      <div className="flex min-h-0 flex-col gap-3 overflow-auto">
        <Panel
          icon={<Crosshair />}
          title={t('picked.title')}
          actions={
            <SegmentedControl<string>
              size="sm"
              value={String(radius)}
              onValueChange={(value) => setRadius(Number(value))}
              aria-label={t('picked.area')}
              options={RADII.map((r) => ({ value: String(r), label: r === 0 ? '1 px' : `${r * 2 + 1}×${r * 2 + 1}` }))}
            />
          }
          className="shrink-0"
          bodyClassName="p-3"
        >
          {!picked ? (
            <p className="text-xs text-fg-subtle">{t(image ? 'picked.empty' : 'picked.noImage')}</p>
          ) : (
            <div className="space-y-3">
              <div className="flex gap-3">
                <div className="grid shrink-0 overflow-hidden rounded-control border border-border" style={{ gridTemplateColumns: `repeat(${LOUPE}, 8px)` }}>
                  {picked.loupe.map((hex, i) => (
                    <span
                      key={i}
                      className={cn('size-2', i === (LOUPE * LOUPE - 1) / 2 && LOUPE_CENTER)}
                      style={{ backgroundColor: hex ?? 'transparent' }}
                    />
                  ))}
                </div>
                <div className="flex min-w-0 flex-1 flex-col gap-1">
                  <div className="h-14 rounded-control border border-border" style={{ backgroundColor: picked.hex }} />
                  <p className="text-[11px] text-fg-subtle tabular-nums">
                    {t('picked.position', { x: picked.x, y: picked.y })}
                    {picked.alpha < 255 && ` · ${t('picked.alpha', { alpha: Math.round((picked.alpha / 255) * 100) })}`}
                  </p>
                </div>
              </div>
              <FormatRows color={picked} />
            </div>
          )}
        </Panel>

        <Panel
          icon={<Palette />}
          title={t('palette.title')}
          actions={
            <>
              <NumberInput size="sm" value={count} onValueChange={setCount} min={2} max={16} className="w-20" aria-label={t('palette.count')} />
              <DropdownMenu
                trigger={
                  <Button size="sm" disabled={!palette.result?.length}>
                    <Download />
                    {t('palette.export')}
                  </Button>
                }
                items={EXPORTS.map((format) => ({ key: format, label: t(`exports.${format}`), onSelect: () => void exportAs(format) }))}
              />
            </>
          }
          className="min-h-0 flex-1"
          bodyClassName={cn('overflow-auto p-2', palette.pending && 'opacity-60')}
        >
          {!image ? (
            <Empty icon={<Palette />} title={t('palette.empty')} />
          ) : palette.error ? (
            <p className="p-2 text-xs text-danger">{errorMessage(palette.error)}</p>
          ) : (
            <div className="space-y-1">
              <div className="flex h-8 overflow-hidden rounded-control">
                {palette.result?.map((s) => (
                  <span key={s.hex} style={{ backgroundColor: s.hex, flexGrow: s.share }} title={s.hex} />
                ))}
              </div>
              {palette.result?.map((s) => (
                <div key={s.hex} className="group flex items-center gap-2 rounded-control px-1.5 py-1 hover:bg-hover">
                  <span
                    className="grid size-8 shrink-0 place-items-center rounded-control border border-border text-[10px] font-medium"
                    style={{ backgroundColor: s.hex, color: TEXT_ON[s.text] }}
                  >
                    {t('palette.sample')}
                  </span>
                  <code className="flex-1 font-mono text-[12.5px] text-fg" data-selectable>
                    {s.hex}
                  </code>
                  <span className="text-[11px] text-fg-subtle tabular-nums">{t('palette.share', { share: (s.share * 100).toFixed(1) })}</span>
                  <CopyButton text={s.hex} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" />
                </div>
              ))}
            </div>
          )}
        </Panel>
      </div>
    </div>
  )
}

export default ImagePalette
