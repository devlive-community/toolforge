import { useEffect, useState } from 'react'
import { Button, Checkbox, NumberInput, Panel, Spinner, cn, toast } from '@toolforge/ui'
import { host, useDebouncedCall, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { Download, FileImage, FolderInput, FolderOpen, LayoutTemplate, RotateCcw } from 'lucide-react'
import { SourceCard, dirname, type Info } from './source'

type Group = 'folder' | 'app' | 'drive' | 'document' | 'media' | 'photo' | 'retro'
type Format = 'png' | 'icns' | 'ico'
const GROUPS: Group[] = ['folder', 'app', 'drive', 'document', 'media', 'photo', 'retro']
const FORMATS: Format[] = ['png', 'icns', 'ico']

interface TemplateItem {
  id: string
  group: Group
  image: string
}
interface Exported {
  files: string[]
}
interface Applied {
  applied: number
  total: number
}

const CHECKER = 'bg-surface-2 bg-[conic-gradient(var(--color-border)_25%,transparent_0_50%,var(--color-border)_0_75%,transparent_0)] bg-[length:16px_16px]'

interface TemplatesProps {
  info: Info | null
  hovering: boolean
  onPick: () => void
}

/** 外层用 key={info.path} 挂载：换图片时输出目录随之重置 */
export function Templates({ info, hovering, onPick }: TemplatesProps) {
  const { t, call, errorMessage } = usePlugin()
  const [selected, setSelected] = usePluginState('template', 'folder')
  const [zoom, setZoom] = usePluginState('templateZoom', 100)
  const [formats, setFormats] = usePluginState<Format[]>('templateFormats', ['png', 'icns', 'ico'])
  const [outputDir, setOutputDir] = useState<string | null>(info ? dirname(info.path) : null)
  const [applyFiles, setApplyFiles] = useState(false)
  const exportTask = useTask<Exported>()
  const applyTask = useTask<Applied>()

  const path = info?.path ?? null
  const list = useDebouncedCall<TemplateItem[]>('templates', { path, zoom: zoom / 100 }, [path, zoom])
  const preview = useDebouncedCall<string>('renderTemplate', { path, template: selected, zoom: zoom / 100, size: 512 }, [path, selected, zoom])
  const args = { path, template: selected, zoom: zoom / 100 }

  useEffect(() => {
    call<{ files: boolean }>('applySupport', {}).then(
      (s) => setApplyFiles(s.files),
      () => {},
    )
  }, [call])

  useEffect(() => {
    if (exportTask.status === 'succeeded' && exportTask.result) toast.success(t('templates.exported', { count: exportTask.result.files.length }))
    if (exportTask.status === 'failed' && exportTask.error) toast.error(errorMessage(exportTask.error))
  }, [exportTask.status]) // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    const r = applyTask.result
    if (applyTask.status === 'succeeded' && r) {
      if (r.applied === r.total) toast.success(t('templates.applied', { count: r.applied }))
      else toast.error(t('templates.applyPartial', { applied: r.applied, total: r.total }))
    }
    if (applyTask.status === 'failed' && applyTask.error) toast.error(errorMessage(applyTask.error))
  }, [applyTask.status]) // eslint-disable-line react-hooks/exhaustive-deps

  const pickOutput = async () => {
    try {
      const dir = await host.dialog.openDirectory()
      if (dir) setOutputDir(dir)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const pickTargets = async (files: boolean) => {
    if (files) return host.dialog.openFiles()
    const dir = await host.dialog.openDirectory()
    return dir ? [dir] : []
  }
  const apply = async (files: boolean) => {
    try {
      const targets = await pickTargets(files)
      if (targets.length) applyTask.start('applyIcon', { ...args, targets })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const restore = async () => {
    try {
      const targets = await pickTargets(false)
      if (!targets.length) return
      await call('clearIcon', { targets })
      toast.success(t('templates.restored'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const toggle = (format: Format, on: boolean) =>
    setFormats(on ? FORMATS.filter((f) => f === format || formats.includes(f)) : formats.filter((f) => f !== format))
  const failures = applyTask.logs.filter((line) => line.level === 'error')
  // 预览还没跟上当前的模板与参数时，不让导出或设置，避免拿到的不是看到的
  const busy = preview.pending || (!preview.result && !preview.error)
  const templateName = t(`templates.names.${selected}`)
  const formatList = FORMATS.filter((f) => formats.includes(f))
    .map((f) => t(`templates.formats.${f}`))
    .join(t('templates.separator'))

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_340px] gap-3">
      <Panel
        icon={<LayoutTemplate />}
        title={t('templates.gallery')}
        extra={list.pending && <Spinner className="size-3.5 text-fg-subtle" />}
        bodyClassName="space-y-4 overflow-auto p-3"
      >
        {!info && <p className="rounded-control bg-surface-2 px-3 py-2 text-xs text-fg-muted">{t('templates.sampleHint')}</p>}
        {list.error ? (
          <p className="text-xs text-danger">{errorMessage(list.error)}</p>
        ) : (
          GROUPS.map((group) => {
            const items = list.result?.filter((item) => item.group === group) ?? []
            if (!items.length) return null
            return (
              <section key={group} className="space-y-2">
                <h3 className="text-xs font-medium text-fg-muted">{t(`templates.groups.${group}`)}</h3>
                <div className="grid grid-cols-[repeat(auto-fill,minmax(112px,1fr))] gap-2">
                  {items.map((item) => (
                    <button
                      key={item.id}
                      type="button"
                      onClick={() => setSelected(item.id)}
                      aria-pressed={item.id === selected}
                      className={cn(
                        'flex flex-col items-center gap-1 rounded-card border p-2 outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
                        item.id === selected ? 'border-primary bg-primary-soft' : 'border-transparent hover:bg-hover',
                      )}
                    >
                      <img src={item.image} alt="" className="size-20" />
                      <span className={cn('text-[11px]', item.id === selected ? 'text-primary-fg' : 'text-fg-muted')}>{t(`templates.names.${item.id}`)}</span>
                    </button>
                  ))}
                </div>
              </section>
            )
          })
        )}
      </Panel>

      <Panel icon={<FileImage />} title={t(`templates.names.${selected}`)} bodyClassName="space-y-4 overflow-auto p-3">
        <SourceCard info={info} hovering={hovering} onPick={onPick} compact />

        <div className={cn('relative mx-auto grid aspect-square w-full max-w-64 place-items-center overflow-hidden rounded-card', CHECKER)} aria-busy={busy}>
          {preview.result && <img src={preview.result} alt="" className={cn('size-full p-2 transition-opacity', busy && 'opacity-25')} />}
          {busy && (
            <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 bg-surface/60">
              <Spinner className="size-5 text-primary" />
              <span className="text-xs font-medium text-fg-muted">{t('templates.rendering')}</span>
            </div>
          )}
        </div>
        {preview.error && <p className="text-xs text-danger">{errorMessage(preview.error)}</p>}

        <label className="block space-y-1.5">
          <span className="block text-xs font-medium text-fg-muted">{t('templates.zoom')}</span>
          <NumberInput size="sm" value={zoom} onValueChange={setZoom} min={50} max={150} step={5} aria-label={t('templates.zoom')} />
        </label>

        <div className="space-y-2 rounded-control border border-border p-3">
          <p className="text-xs font-medium text-fg-muted">{t('templates.exportTitle', { name: templateName })}</p>
          <div className="flex gap-4">
            {FORMATS.map((format) => (
              <Checkbox key={format} checked={formats.includes(format)} onCheckedChange={(on) => toggle(format, on)}>
                <span className="text-[13px] text-fg">{t(`templates.formats.${format}`)}</span>
              </Checkbox>
            ))}
          </div>
          <Button block size="sm" className="justify-start" onClick={pickOutput}>
            <FolderOpen />
            <span className="truncate">{outputDir ?? t('options.chooseFolder')}</span>
          </Button>
          <Button
            block
            size="sm"
            variant="primary"
            loading={exportTask.running}
            disabled={!info || busy || !outputDir || formats.length === 0}
            onClick={() => exportTask.start('exportTemplate', { ...args, formats, outputDir })}
          >
            <Download />
            {formats.length ? t('templates.exportRun', { formats: formatList }) : t('templates.noFormat')}
          </Button>
          {!info && <p className="text-[11px] text-fg-subtle">{t('templates.needImage')}</p>}
          {exportTask.status === 'succeeded' && exportTask.result && (
            <Button block size="sm" variant="ghost" onClick={() => host.revealPath(exportTask.result!.files[0]).catch(() => {})}>
              {t('generate.show')}
            </Button>
          )}
        </div>

        <div className="space-y-2 rounded-control border border-border p-3">
          <p className="text-xs font-medium text-fg-muted">{t('templates.applyTitle', { name: templateName })}</p>
          <p className="text-[11px] leading-snug text-fg-subtle">{t(applyFiles ? 'templates.applyHintMac' : 'templates.applyHint')}</p>
          <div className="flex flex-wrap gap-2">
            <Button size="sm" loading={applyTask.running} disabled={!info || busy} onClick={() => apply(false)}>
              <FolderInput />
              {t('templates.applyFolder')}
            </Button>
            {applyFiles && (
              <Button size="sm" disabled={!info || busy || applyTask.running} onClick={() => apply(true)}>
                <FileImage />
                {t('templates.applyFile')}
              </Button>
            )}
            <Button size="sm" variant="ghost" onClick={restore}>
              <RotateCcw />
              {t('templates.restore')}
            </Button>
          </div>
          {failures.length > 0 && (
            <ul className="space-y-0.5 text-[11px] text-danger" data-selectable>
              {failures.map((line) => (
                <li key={line.id} className="break-all">
                  {line.text}
                </li>
              ))}
            </ul>
          )}
        </div>
      </Panel>
    </div>
  )
}
