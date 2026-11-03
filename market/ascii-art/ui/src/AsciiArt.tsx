import { useEffect, useRef, useState } from 'react'
import { Button, Empty, Input, NumberInput, Panel, SegmentedControl, Select, Spinner, Switch, Tabs, cn, toast } from '@toolforge/ui'
import { host, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Copy, FileDown, Image as ImageIcon, RefreshCw, Type } from 'lucide-react'
import {
  ASPECT_MAX,
  ASPECT_MIN,
  DEFAULT_PARAMS,
  IMAGE_EXTENSIONS,
  PRESET_IDS,
  WIDTH_MAX,
  WIDTH_MIN,
  type ArtParams,
  type ConvertResult,
  type FetchResult,
  type PrepareResult,
} from './types'

type Tab = 'art' | 'original'

/** 参数 → convert 调用参数；自定义符号非空时覆盖符号组 */
function toArgs(work: string, params: ArtParams) {
  const base: Record<string, unknown> = {
    data: work,
    width: params.width,
    brightness: params.brightness,
    contrast: params.contrast,
    invert: params.invert,
    color: params.color,
    aspect: params.aspect,
    alphaMode: params.alphaMode,
  }
  if (params.custom.trim()) base.chars = params.custom
  else base.preset = params.preset
  if (params.alphaMode === 'fill') base.bg = params.bg
  return base
}

export function AsciiArt() {
  const { t, call, errorMessage } = usePlugin()
  const [params, setParams] = usePluginState<ArtParams>('params', { ...DEFAULT_PARAMS })
  const [work, setWork] = useState<PrepareResult | null>(null)
  const [preview, setPreview] = useState<ConvertResult | null>(null)
  const [busy, setBusy] = useState(false)
  const [tab, setTab] = useState<Tab>('art')
  // 请求序号：丢弃过期响应，避免慢转换覆盖新参数的结果
  const seq = useRef(0)

  const patch = (value: Partial<ArtParams>) => setParams((prev) => ({ ...prev, ...value }))

  const run = async (action: () => Promise<void>) => {
    setBusy(true)
    try {
      await action()
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setBusy(false)
    }
  }

  const pick = () =>
    run(async () => {
      const path = await host.dialog.openFile([{ name: t('file.filter'), extensions: IMAGE_EXTENSIONS }])
      if (!path) return
      const out = await call<PrepareResult>('prepare', { path })
      setWork(out)
      setPreview(null)
      setTab('art')
    })

  // 参数或图片变化后防抖重算；依赖全部是稳定原始值
  useEffect(() => {
    const data = work?.workData
    if (!data) return
    const current = ++seq.current
    const timer = setTimeout(() => {
      call<ConvertResult>('convert', toArgs(data, params))
        .then((out) => {
          if (seq.current === current) setPreview(out)
        })
        .catch((error) => {
          if (seq.current === current) {
            setPreview(null)
            toast.error(errorMessage(error))
          }
        })
        .finally(() => {
          if (seq.current === current) setBusy(false)
        })
      // 参数变化即进入转换中状态，导出按钮同步禁用，不会误导出旧结果
      setBusy(true)
    }, 250)
    return () => clearTimeout(timer)
  }, [call, errorMessage, work?.workData, params])

  /** 取完整文本（fetch 结果缓存里的全文，而非有界预览） */
  const takeText = async (ansi: boolean) => {
    if (!preview) return null
    const out = await call<FetchResult>('fetch', { id: preview.id, ansi })
    return out.text
  }

  const copy = (ansi: boolean) =>
    run(async () => {
      const text = await takeText(ansi)
      if (text === null) return
      await host.clipboard.writeText(text)
      toast.success(t('copied'))
    })

  const save = (ansi: boolean) =>
    run(async () => {
      if (!preview) return
      const ext = ansi ? 'ans' : 'txt'
      const path = await host.dialog.saveFile(`ascii-art.${ext}`, [
        { name: ext === 'ans' ? 'ANSI text' : 'Plain text', extensions: [ext] },
      ])
      if (!path) return
      await call('save', { id: preview.id, ansi, path })
      toast.success(t('saved'))
    })

  const actionable = preview !== null && preview.complete && !busy

  return (
    <div className="grid h-full min-h-0 flex-1 grid-cols-1 gap-3 md:grid-cols-[288px_minmax(0,1fr)]">
      <Panel title={t('options.title')} icon={<Type />} className="min-h-0 overflow-y-auto" bodyClassName="space-y-4 p-3">
        <Button variant="primary" className="w-full" onClick={pick}>
          {work ? <RefreshCw /> : <ImageIcon />}
          {work ? t('actions.replace') : t('actions.pick')}
        </Button>
        <div className="space-y-1.5">
          <span className="text-xs font-medium text-fg-muted">{t('options.width')}</span>
          <NumberInput
            value={params.width}
            onValueChange={(value) => patch({ width: value })}
            min={WIDTH_MIN}
            max={WIDTH_MAX}
            step={8}
            aria-label={t('options.width')}
          />
        </div>
        <div className="space-y-1.5">
          <span className="text-xs font-medium text-fg-muted">{t('options.preset')}</span>
          <Select
            value={params.preset}
            onValueChange={(value) => patch({ preset: value })}
            aria-label={t('options.preset')}
            options={PRESET_IDS.map((id) => ({ value: id, label: t(`presets.${id}`) }))}
          />
        </div>
        <div className="space-y-1.5">
          <span className="text-xs font-medium text-fg-muted">{t('options.custom')}</span>
          <Input
            value={params.custom}
            onChange={(event) => patch({ custom: event.target.value })}
            placeholder=" .:-=+*#%@"
            aria-label={t('options.custom')}
            spellCheck={false}
          />
          {preview?.warnings.includes('duplicateChars') && (
            <p className="text-[11px] text-warning">{t('warnings.duplicateChars')}</p>
          )}
          {preview?.warnings.includes('skippedChars') && (
            <p className="text-[11px] text-warning">{t('warnings.skippedChars', { count: preview.warnings.length })}</p>
          )}
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div className="space-y-1.5">
            <span className="text-xs font-medium text-fg-muted">{t('options.brightness')}</span>
            <NumberInput
              value={params.brightness}
              onValueChange={(value) => patch({ brightness: value })}
              min={-1}
              max={1}
              step={0.05}
              aria-label={t('options.brightness')}
            />
          </div>
          <div className="space-y-1.5">
            <span className="text-xs font-medium text-fg-muted">{t('options.contrast')}</span>
            <NumberInput
              value={params.contrast}
              onValueChange={(value) => patch({ contrast: value })}
              min={0.2}
              max={3}
              step={0.05}
              aria-label={t('options.contrast')}
            />
          </div>
        </div>
        <div className="space-y-1.5">
          <span className="text-xs font-medium text-fg-muted">{t('options.aspect')}</span>
          <NumberInput
            value={params.aspect}
            onValueChange={(value) => patch({ aspect: value })}
            min={ASPECT_MIN}
            max={ASPECT_MAX}
            step={0.05}
            aria-label={t('options.aspect')}
          />
        </div>
        <div className="flex items-center justify-between gap-3">
          <span className="text-xs font-medium text-fg-muted">{t('options.invert')}</span>
          <Switch size="sm" checked={params.invert} onCheckedChange={(on) => patch({ invert: on })} aria-label={t('options.invert')} />
        </div>
        <div className="flex items-center justify-between gap-3">
          <span className="text-xs font-medium text-fg-muted">{t('options.color')}</span>
          <Switch size="sm" checked={params.color} onCheckedChange={(on) => patch({ color: on })} aria-label={t('options.color')} />
        </div>
        <div className="space-y-1.5">
          <span className="text-xs font-medium text-fg-muted">{t('options.alphaMode')}</span>
          <SegmentedControl<'keep' | 'fill'>
            value={params.alphaMode}
            onValueChange={(value) => patch({ alphaMode: value })}
            options={[
              { value: 'keep', label: t('options.keep') },
              { value: 'fill', label: t('options.fill') },
            ]}
          />
        </div>
        {params.alphaMode === 'fill' && (
          <div className="space-y-1.5">
            <span className="text-xs font-medium text-fg-muted">{t('options.bg')}</span>
            <Input
              value={params.bg}
              onChange={(event) => patch({ bg: event.target.value })}
              placeholder="1a1a1a"
              aria-label={t('options.bg')}
              spellCheck={false}
            />
          </div>
        )}
      </Panel>
      <Panel
        title={t('view.art')}
        className="min-h-0"
        bodyClassName="p-3"
        footer={
          <div className="flex flex-wrap items-center gap-2">
            <Button size="sm" variant="outline" disabled={!actionable} onClick={() => void copy(false)}>
              <Copy />
              {t('actions.copyText')}
            </Button>
            {params.color && (
              <Button size="sm" variant="outline" disabled={!actionable} onClick={() => void copy(true)}>
                <Copy />
                {t('actions.copyAnsi')}
              </Button>
            )}
            <Button size="sm" variant="outline" disabled={!actionable} onClick={() => void save(false)}>
              <FileDown />
              {t('actions.saveText')}
            </Button>
            {params.color && (
              <Button size="sm" variant="outline" disabled={!actionable} onClick={() => void save(true)}>
                <FileDown />
                {t('actions.saveAnsi')}
              </Button>
            )}
            <span className="ml-auto text-[11px] text-fg-subtle">
              {preview ? t('info.size', { cols: preview.cols, rows: preview.rows, chars: preview.chars }) : t('state.noImage')}
            </span>
          </div>
        }
      >
        {work ? (
          <div className="flex h-full min-h-0 flex-col gap-2">
            <Tabs
              value={tab}
              onValueChange={setTab}
              items={[
                { value: 'art', label: t('view.art') },
                { value: 'original', label: t('view.original') },
              ]}
            />
            {tab === 'art' ? (
              busy && !preview ? (
                <div className="flex flex-1 items-center justify-center text-fg-muted">
                  <Spinner />
                </div>
              ) : (
                <pre
                  className={cn('min-h-0 flex-1 overflow-auto rounded-control bg-surface-2 p-3 font-mono text-[10px] leading-[1.2] text-fg whitespace-pre', busy && 'opacity-60')}
                >
                  {preview?.text ?? ''}
                </pre>
              )
            ) : (
              <div className="flex min-h-0 flex-1 items-center justify-center overflow-hidden rounded-control bg-surface-2 p-3">
                <img src={work.workData} alt="" className="max-h-full max-w-full object-contain" />
              </div>
            )}
            <p className="text-[11px] text-fg-subtle">{t('hint.monospace')}</p>
          </div>
        ) : (
          <Empty
            className="py-24"
            icon={<ImageIcon />}
            title={t('state.noImage')}
            action={
              <Button variant="primary" onClick={pick}>
                {t('actions.pick')}
              </Button>
            }
          />
        )}
      </Panel>
    </div>
  )
}
