import { useEffect, useRef, useState } from 'react'
import { Badge, Button, Checkbox, Empty, Input, NumberInput, Panel, Switch, cn, toast } from '@toolforge/ui'
import { CopyButton, host, useDebouncedCall, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { FolderOpen, FolderSearch, ImagePlus, LayoutGrid, Sparkles, Upload, X } from 'lucide-react'

type Preset = 'web' | 'ios' | 'android' | 'macos' | 'windows'
const PRESETS: Preset[] = ['web', 'ios', 'android', 'macos', 'windows']
const EXTENSIONS = ['svg', 'png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp', 'tif', 'tiff']
const SWATCHES = ['#ffffff', '#000000', '#1e293b', '#2563eb', '#16a34a', '#f97316', '#e11d48', '#7c3aed'] // tf-allow: 可选的背景色
const HEX = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i

interface Info {
  path: string
  name: string
  width: number
  height: number
  svg: boolean
  square: boolean
  transparent: boolean
  small: boolean
}
interface Previews {
  rounded: string
  opaque: string
  circle: string
  mac: string
}
interface Generated {
  dir: string
  files: number
  snippet: string | null
}

const dirname = (path: string) => path.replace(/[\\/][^\\/]*$/, '')

function Preview({ src, label, className }: { src?: string; label: string; className?: string }) {
  return (
    <figure className="flex flex-col items-center gap-2">
      <div className="grid size-32 place-items-center rounded-card bg-surface-2 bg-[conic-gradient(var(--color-border)_25%,transparent_0_50%,var(--color-border)_0_75%,transparent_0)] bg-[length:16px_16px]">
        {src && <img src={src} alt="" className={cn('size-28', className)} />}
      </div>
      <figcaption className="text-xs text-fg-muted">{label}</figcaption>
    </figure>
  )
}

function IconGenerator() {
  const { t, call, errorMessage } = usePlugin()
  const [padding, setPadding] = usePluginState('padding', 8)
  const [radius, setRadius] = usePluginState('radius', 18)
  const [useBackground, setUseBackground] = usePluginState('useBackground', false)
  const [background, setBackground] = usePluginState('background', '#ffffff') // tf-allow: 默认背景色输入值
  const [presets, setPresets] = usePluginState<Preset[]>('presets', PRESETS)
  const [info, setInfo] = useState<Info | null>(null)
  const [appName, setAppName] = useState('')
  const [outputDir, setOutputDir] = useState<string | null>(null)
  const [hovering, setHovering] = useState(false)
  const task = useTask<Generated>()

  const load = async (path: string) => {
    try {
      const next = await call<Info>('inspect', { path })
      setInfo(next)
      setAppName(next.name)
      setOutputDir(dirname(next.path))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const loadRef = useRef(load)
  useEffect(() => {
    loadRef.current = load
  })
  useEffect(() => {
    const off = host.onFileDrop({ drop: (paths) => paths[0] && void loadRef.current(paths[0]), over: setHovering })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])
  useEffect(() => {
    if (task.status === 'succeeded' && task.result) toast.success(t('generate.done', { count: task.result.files }))
    if (task.status === 'failed' && task.error) toast.error(errorMessage(task.error))
  }, [task.status]) // eslint-disable-line react-hooks/exhaustive-deps

  const colorValid = HEX.test(background)
  const style = {
    padding: padding / 100,
    radius: radius / 100,
    background: useBackground && colorValid ? (background.startsWith('#') ? background : `#${background}`) : null,
  }
  const renderArgs = info ? { path: info.path, style } : null
  const previews = useDebouncedCall<Previews>(renderArgs ? 'render' : null, renderArgs, [info?.path, padding, radius, style.background])

  const pick = async () => {
    try {
      const path = await host.dialog.openFile([{ name: t('source.filter'), extensions: EXTENSIONS }])
      if (path) await load(path)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const pickOutput = async () => {
    try {
      const dir = await host.dialog.openDirectory()
      if (dir) setOutputDir(dir)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const toggle = (preset: Preset, on: boolean) =>
    setPresets(on ? PRESETS.filter((p) => p === preset || presets.includes(p)) : presets.filter((p) => p !== preset))
  const generate = () => task.start('generate', { path: info!.path, style, presets, outputDir, name: appName })

  return (
    <div className="grid h-full min-h-0 grid-cols-[320px_minmax(0,1fr)] gap-3">
      <Panel icon={<Sparkles />} title={t('options.title')} bodyClassName="space-y-4 overflow-auto p-3">
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('source.title')}</p>
          {info ? (
            <div className="flex items-center gap-2 rounded-control border border-border px-3 py-2">
              <div className="min-w-0 flex-1">
                <p className="truncate text-[13px] text-fg" title={info.path}>
                  {info.path.split(/[\\/]/).pop()}
                </p>
                <p className="text-[11px] text-fg-subtle">
                  {info.svg ? t('source.svg') : t('source.size', { width: info.width, height: info.height })}
                </p>
              </div>
              <Button size="icon-sm" variant="ghost" aria-label={t('source.change')} onClick={pick}>
                <ImagePlus />
              </Button>
            </div>
          ) : (
            <button
              type="button"
              onClick={pick}
              className={cn(
                'flex w-full flex-col items-center justify-center gap-1.5 rounded-card border-2 border-dashed px-3 py-6 text-center outline-none transition-colors',
                'focus-visible:ring-2 focus-visible:ring-ring',
                hovering ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
              )}
            >
              <Upload className="size-5" />
              <span className="text-[13px] font-medium">{t('source.drop')}</span>
              <span className="text-[11px] text-fg-subtle">{t('source.dropHint')}</span>
            </button>
          )}
          {info?.small && <p className="text-[11px] text-warning">{t('source.small', { size: Math.max(info.width, info.height) })}</p>}
          {info && !info.square && <p className="text-[11px] text-fg-subtle">{t('source.notSquare')}</p>}
        </div>

        <div className="grid grid-cols-2 gap-3">
          <label className="space-y-1.5">
            <span className="block text-xs font-medium text-fg-muted">{t('options.padding')}</span>
            <NumberInput size="sm" value={padding} onValueChange={setPadding} min={0} max={40} step={2} aria-label={t('options.padding')} />
          </label>
          <label className="space-y-1.5">
            <span className="block text-xs font-medium text-fg-muted">{t('options.radius')}</span>
            <NumberInput size="sm" value={radius} onValueChange={setRadius} min={0} max={50} step={2} aria-label={t('options.radius')} />
          </label>
        </div>

        <div className="space-y-2">
          <label className="flex items-center justify-between gap-2 text-[13px] text-fg">
            {t('options.background')}
            <Switch size="sm" checked={useBackground} onCheckedChange={setUseBackground} aria-label={t('options.background')} />
          </label>
          {useBackground && (
            <>
              <div className="flex items-center gap-2">
                <span className="size-7 shrink-0 rounded-control border border-border" style={{ backgroundColor: colorValid ? style.background ?? undefined : undefined }} />
                <Input size="sm" value={background} onChange={(e) => setBackground(e.target.value)} className="font-mono" aria-label={t('options.color')} />
              </div>
              <div className="flex flex-wrap gap-1.5">
                {SWATCHES.map((color) => (
                  <button
                    key={color}
                    type="button"
                    aria-label={color}
                    onClick={() => setBackground(color)}
                    className={cn('size-6 rounded-full border border-border outline-none focus-visible:ring-2 focus-visible:ring-ring', background.toLowerCase() === color && 'ring-2 ring-primary')}
                    style={{ backgroundColor: color }}
                  />
                ))}
              </div>
              {!colorValid && <p className="text-[11px] text-danger">{t('options.invalidColor')}</p>}
            </>
          )}
          <p className="text-[11px] leading-snug text-fg-subtle">{t('options.backgroundHint')}</p>
        </div>

        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('presets.title')}</p>
          {PRESETS.map((preset) => (
            <Checkbox key={preset} className="flex" checked={presets.includes(preset)} onCheckedChange={(on) => toggle(preset, on)}>
              <span className="text-[13px] text-fg">{t(`presets.${preset}.name`)}</span>
              <span className="ml-1.5 text-[11px] text-fg-subtle">{t(`presets.${preset}.files`)}</span>
            </Checkbox>
          ))}
        </div>

        <label className="block space-y-1.5">
          <span className="block text-xs font-medium text-fg-muted">{t('options.appName')}</span>
          <Input size="sm" value={appName} onChange={(e) => setAppName(e.target.value)} aria-label={t('options.appName')} />
        </label>

        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('options.output')}</p>
          <Button block size="sm" className="justify-start" onClick={pickOutput}>
            <FolderOpen />
            <span className="truncate">{outputDir ?? t('options.chooseFolder')}</span>
          </Button>
        </div>

        <Button
          block
          variant="primary"
          loading={task.running}
          disabled={!info || !outputDir || presets.length === 0 || (useBackground && !colorValid)}
          onClick={generate}
        >
          <Sparkles />
          {t('generate.run')}
        </Button>
      </Panel>

      <div className="flex min-h-0 flex-col gap-3">
        <Panel icon={<LayoutGrid />} title={t('preview.title')} className="shrink-0" bodyClassName="p-4">
          {!info ? (
            <Empty icon={<LayoutGrid />} title={t('preview.empty')} description={t('preview.emptyHint')} />
          ) : previews.error ? (
            <p className="text-xs text-danger">{errorMessage(previews.error)}</p>
          ) : (
            <div className={cn('space-y-5', previews.pending && 'opacity-60')}>
              <div className="flex flex-wrap justify-around gap-4">
                <Preview src={previews.result?.rounded} label={t('preview.rounded')} />
                {/* iOS 由系统裁成圆角，这里模拟显示效果 */}
                <Preview src={previews.result?.opaque} label={t('preview.ios')} className="rounded-[22.37%]" />
                <Preview src={previews.result?.circle} label={t('preview.round')} />
                <Preview src={previews.result?.mac} label={t('preview.mac')} />
              </div>
              <div className="flex items-end justify-center gap-5">
                {[16, 32, 48, 64].map((size) => (
                  <figure key={size} className="flex flex-col items-center gap-1.5">
                    {previews.result && <img src={previews.result.rounded} alt="" style={{ width: size, height: size }} />}
                    <figcaption className="text-[11px] text-fg-subtle tabular-nums">{size}</figcaption>
                  </figure>
                ))}
              </div>
            </div>
          )}
        </Panel>
        {task.result && task.status === 'succeeded' && (
          <Panel
            title={t('generate.result')}
            extra={<Badge variant="success">{t('generate.files', { count: task.result.files })}</Badge>}
            actions={
              <Button size="sm" onClick={() => host.revealPath(task.result!.dir).catch(() => {})}>
                <FolderSearch />
                {t('generate.show')}
              </Button>
            }
            className="min-h-0"
            bodyClassName="space-y-2 overflow-auto p-3"
          >
            <p className="truncate font-mono text-xs text-fg-muted" title={task.result.dir} data-selectable>
              {task.result.dir}
            </p>
            {task.result.snippet && (
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <p className="text-xs font-medium text-fg-muted">{t('generate.snippet')}</p>
                  <CopyButton text={task.result.snippet} label={t('generate.copy')} />
                </div>
                <pre className="overflow-auto rounded-control bg-surface-2 p-2.5 font-mono text-[11px] leading-relaxed text-fg" data-selectable>
                  {task.result.snippet}
                </pre>
              </div>
            )}
          </Panel>
        )}
        {info && !task.result && (
          <Button variant="ghost" size="sm" className="self-start" onClick={() => setInfo(null)}>
            <X />
            {t('source.clear')}
          </Button>
        )}
      </div>
    </div>
  )
}

export default IconGenerator
