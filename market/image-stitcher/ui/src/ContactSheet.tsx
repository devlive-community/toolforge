import { useState } from 'react'
import { Badge, Button, Empty, Input, LogViewer, NumberInput, Panel, Progress, SegmentedControl, Tooltip, cn, toast } from '@toolforge/ui'
import { host, useDebouncedCall, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { ArrowDown, ArrowUp, FileDown, FolderSearch, Play, ScrollText, Settings2, Square, Type, X } from 'lucide-react'
import { formatBytes } from './format'
import { DEFAULT_SHEET_SETTINGS, type InspectEntry, type Orientation, type Paper, type SheetPlan, type SheetReport, type SheetSettings } from './types'

const FONT_EXTENSIONS = ['ttf', 'otf', 'ttc']
/** 行距（与后端 PDF 布局一致） */
const LINE_SPACING = 1.35

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="space-y-1.5">
      <p className="text-xs font-medium text-fg-muted">{label}</p>
      {children}
    </div>
  )
}

interface Props {
  entries: InspectEntry[]
  onEntriesChange: (update: (prev: InspectEntry[]) => InspectEntry[]) => void
  onMove: (index: number, offset: number) => void
  onPick: () => void
  adding: boolean
  hovering: boolean
}

/** 联系印样：共享图片清单 + 纸张设置 + 分页预览 + 多页 PDF 导出 */
export function ContactSheet({ entries, onEntriesChange, onMove, onPick, adding, hovering }: Props) {
  const { t, errorMessage } = usePlugin()
  const task = useTask<SheetReport>()
  const [settings, setSettings] = usePluginState<SheetSettings>('sheet-settings', DEFAULT_SHEET_SETTINGS)
  const [notes, setNotes] = useState<Record<string, string>>({})
  const [fontPath, setFontPath] = useState<string | null>(null)
  const [pageIndex, setPageIndex] = useState(0)

  const usable = entries.filter((entry) => !entry.error)
  const items = usable.map((entry) => ({
    width: entry.width,
    height: entry.height,
    name: entry.name,
    note: notes[entry.path]?.trim() ? notes[entry.path] : null,
  }))
  const ready = usable.length > 0
  const plan = useDebouncedCall<SheetPlan>(
    ready ? 'plan_contact_sheet' : null,
    ready ? { items, settings } : null,
    [entries, notes, settings],
  )
  const pageCount = plan.result?.pages.length ?? 0
  const safePage = Math.min(pageIndex, Math.max(pageCount - 1, 0))
  const page = plan.result?.pages[safePage]
  const [pageW, pageH] = plan.result?.pageSize ?? [210, 297]
  /** pt 字号换算为页面渲染宽度的百分比（container query 单位） */
  const cqw = (sizePt: number) => ((sizePt * 25.4) / 72 / pageW) * 100

  const patch = (value: Partial<SheetSettings>) => setSettings((prev) => ({ ...prev, ...value }))
  const run = async (action: () => Promise<void>) => {
    try {
      await action()
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const pickFont = () =>
    run(async () => {
      const path = await host.dialog.openFile([{ name: t('sheet.fontFilter'), extensions: FONT_EXTENSIONS }])
      setFontPath(path)
    })

  const doExport = () =>
    run(async () => {
      const path = await host.dialog.saveFile('contact-sheet.pdf', [{ name: 'PDF', extensions: ['pdf'] }])
      if (!path) return
      task.start('export_contact_sheet', {
        items: usable.map((entry) => ({
          path: entry.path,
          name: entry.name,
          note: notes[entry.path]?.trim() ? notes[entry.path] : null,
        })),
        settings,
        fontPath,
        outputPath: path,
      })
    })

  const report = task.result
  const progress = task.progress && task.progress.total > 0 ? (task.progress.done / task.progress.total) * 100 : null
  const thumbFor = (number: number) => usable[number - 1]?.thumbnail

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(320px,0.9fr)_minmax(0,1.5fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel
          icon={<FileDown />}
          title={t('sheet.items')}
          extra={entries.length > 0 && <Badge>{entries.length}</Badge>}
          actions={
            <Button size="sm" onClick={onPick} disabled={task.running || adding}>
              {t('sheet.add')}
            </Button>
          }
          className="flex-1"
          bodyClassName="overflow-auto p-2"
        >
          {entries.length === 0 ? (
            <button
              type="button"
              onClick={onPick}
              className={cn(
                'flex h-full min-h-32 w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
                'focus-visible:ring-2 focus-visible:ring-ring',
                hovering ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
              )}
            >
              <span className="text-[13px] font-medium">{t('sheet.addHint')}</span>
              <span className="text-xs text-fg-subtle">{t('sheet.addDesc')}</span>
            </button>
          ) : (
            <ul className="space-y-0.5">
              {usable.map((entry, index) => (
                <li key={entry.path} className="flex items-start gap-2 rounded-control p-1.5 hover:bg-hover">
                  <span className="w-6 shrink-0 pt-1 text-center text-[11px] text-fg-subtle tabular-nums">{index + 1}</span>
                  <div className="flex size-10 shrink-0 items-center justify-center overflow-hidden rounded-control border border-border bg-surface-2">
                    {entry.thumbnail && <img src={entry.thumbnail} alt="" className="max-h-full max-w-full object-contain" />}
                  </div>
                  <div className="min-w-0 flex-1 space-y-1">
                    <p className="truncate text-[12.5px] text-fg" title={entry.path}>
                      {entry.name}
                    </p>
                    <Input
                      value={notes[entry.path] ?? ''}
                      onChange={(event) => setNotes((prev) => ({ ...prev, [entry.path]: event.target.value }))}
                      placeholder={t('sheet.notePlaceholder')}
                      aria-label={t('sheet.note', { name: entry.name })}
                      className="h-7 text-xs"
                    />
                  </div>
                  <div className="flex shrink-0 flex-col items-center gap-0.5">
                    <Tooltip content={t('sheet.moveUp')}>
                      <Button variant="ghost" size="icon-sm" aria-label={t('sheet.moveUp')} disabled={task.running || index === 0} onClick={() => onMove(index, -1)}>
                        <ArrowUp />
                      </Button>
                    </Tooltip>
                    <Tooltip content={t('sheet.moveDown')}>
                      <Button variant="ghost" size="icon-sm" aria-label={t('sheet.moveDown')} disabled={task.running || index === usable.length - 1} onClick={() => onMove(index, 1)}>
                        <ArrowDown />
                      </Button>
                    </Tooltip>
                    <Tooltip content={t('sheet.remove')}>
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        aria-label={t('sheet.remove', { name: entry.name })}
                        disabled={task.running}
                        onClick={() => onEntriesChange((prev) => prev.filter((e) => e.path !== entry.path))}
                      >
                        <X />
                      </Button>
                    </Tooltip>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </Panel>

        <Panel icon={<Settings2 />} title={t('sheet.options')} className="shrink-0" bodyClassName="space-y-3.5 p-3">
          <div className="grid grid-cols-2 gap-3">
            <Field label={t('sheet.paper')}>
              <SegmentedControl<Paper>
                size="sm"
                value={settings.paper}
                onValueChange={(paper) => patch({ paper })}
                aria-label={t('sheet.paper')}
                options={[
                  { value: 'a4', label: 'A4' },
                  { value: 'letter', label: 'Letter' },
                ]}
              />
            </Field>
            <Field label={t('sheet.orientation')}>
              <SegmentedControl<Orientation>
                size="sm"
                value={settings.orientation}
                onValueChange={(orientation) => patch({ orientation })}
                aria-label={t('sheet.orientation')}
                options={[
                  { value: 'portrait', label: t('sheet.portrait') },
                  { value: 'landscape', label: t('sheet.landscape') },
                ]}
              />
            </Field>
          </div>
          <div className="grid grid-cols-4 gap-2">
            <Field label={t('sheet.columns')}>
              <NumberInput value={settings.columns} onValueChange={(columns) => patch({ columns })} min={1} max={20} aria-label={t('sheet.columns')} />
            </Field>
            <Field label={t('sheet.rows')}>
              <NumberInput value={settings.rows} onValueChange={(rows) => patch({ rows })} min={1} max={30} aria-label={t('sheet.rows')} />
            </Field>
            <Field label={t('sheet.margin')}>
              <NumberInput value={settings.margin} onValueChange={(margin) => patch({ margin })} min={0} max={50} aria-label={t('sheet.margin')} />
            </Field>
            <Field label={t('sheet.gap')}>
              <NumberInput value={settings.gap} onValueChange={(gap) => patch({ gap })} min={0} max={50} aria-label={t('sheet.gap')} />
            </Field>
          </div>
          <p className="-mt-1.5 text-[11px] text-fg-subtle">{t('sheet.unitHint')}</p>
          <div className="grid grid-cols-2 gap-3">
            <Field label={t('sheet.labelHeight')}>
              <NumberInput value={settings.labelHeight} onValueChange={(labelHeight) => patch({ labelHeight })} min={0} max={30} aria-label={t('sheet.labelHeight')} />
            </Field>
            <Field label={t('sheet.fontSize')}>
              <NumberInput value={settings.fontSize} onValueChange={(fontSize) => patch({ fontSize })} min={4} max={48} aria-label={t('sheet.fontSize')} />
            </Field>
          </div>
          <Field label={t('sheet.title')}>
            <Input
              value={settings.title}
              onChange={(event) => patch({ title: event.target.value })}
              placeholder={t('sheet.titlePlaceholder')}
              aria-label={t('sheet.title')}
            />
          </Field>
          <div className="flex flex-wrap items-center gap-2">
            {(
              [
                ['showNumber', t('sheet.showNumber')],
                ['showName', t('sheet.showName')],
                ['showPageNumber', t('sheet.showPageNumber')],
              ] as const
            ).map(([key, label]) => (
              <label key={key} className="flex cursor-pointer items-center gap-1.5 text-[12.5px] text-fg">
                <input
                  type="checkbox"
                  checked={settings[key]}
                  onChange={(event) => patch({ [key]: event.target.checked })}
                  className="size-3.5 accent-[var(--color-primary)]"
                  aria-label={label}
                />
                {label}
              </label>
            ))}
          </div>

          <Field label={t('sheet.font')}>
            <div className="flex items-center gap-1.5">
              <Button className="min-w-0 flex-1 justify-start" onClick={pickFont} disabled={task.running}>
                <Type />
                <span className="truncate">{fontPath ? fontPath.split(/[\\/]/).pop() : t('sheet.fontHint')}</span>
              </Button>
              {fontPath && (
                <Tooltip content={t('sheet.fontReset')}>
                  <Button size="icon-md" variant="ghost" aria-label={t('sheet.fontReset')} onClick={() => setFontPath(null)}>
                    <X />
                  </Button>
                </Tooltip>
              )}
            </div>
          </Field>
          <p className="-mt-1.5 text-[11px] text-fg-subtle">{t('sheet.fontDesc')}</p>

          {task.running && (
            <div className="space-y-1.5">
              <Progress value={progress} />
              <p className="text-xs text-fg-muted tabular-nums">{t(`sheet.stages.${task.stage ?? 'sheet.decode'}`)}</p>
            </div>
          )}
          {task.running ? (
            <Button block onClick={task.cancel}>
              <Square />
              {t('sheet.cancel')}
            </Button>
          ) : (
            <Button block variant="primary" disabled={!ready || adding || !plan.result} onClick={doExport}>
              <Play />
              {t('sheet.export')}
            </Button>
          )}
          {plan.error && <p className="text-xs text-danger">{errorMessage(plan.error)}</p>}
        </Panel>
      </div>

      <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_minmax(140px,0.4fr)] gap-3">
        <Panel
          icon={<FileDown />}
          title={t('sheet.preview')}
          extra={
            report ? (
              <Badge variant="success">{t('sheet.report', { pages: report.pages, images: report.images })}</Badge>
            ) : pageCount > 0 ? (
              <Badge>{t('sheet.pageOf', { page: safePage + 1, count: pageCount })}</Badge>
            ) : null
          }
          actions={
            pageCount > 0 && (
              <div className="flex items-center gap-1">
                <Button size="icon-sm" variant="ghost" aria-label={t('sheet.prevPage')} disabled={safePage === 0} onClick={() => setPageIndex(safePage - 1)}>
                  <ArrowUp />
                </Button>
                <Button size="icon-sm" variant="ghost" aria-label={t('sheet.nextPage')} disabled={safePage >= pageCount - 1} onClick={() => setPageIndex(safePage + 1)}>
                  <ArrowDown />
                </Button>
              </div>
            )
          }
          footer={
            report && (
              <>
                <span className="tabular-nums">{`${formatBytes(report.bytes)} · ${t('sheet.elapsed', { ms: report.elapsedMs })}`}</span>
                {report.missingGlyphs.length > 0 && (
                  <span className="text-warning">{t('sheet.missingGlyphs', { chars: report.missingGlyphs.join(' ') })}</span>
                )}
                <Button size="sm" variant="outline" onClick={() => run(() => host.revealPath(report.output))}>
                  <FolderSearch />
                  {t('sheet.reveal')}
                </Button>
              </>
            )
          }
          bodyClassName="flex items-center justify-center overflow-auto bg-surface-2 p-3"
        >
          {task.status === 'failed' && task.error ? (
            <p className="p-4 text-[13px] text-danger">{errorMessage(task.error)}</p>
          ) : page ? (
            <div
              className="relative shrink-0 bg-white shadow-card"
              style={{
                aspectRatio: `${pageW} / ${pageH}`,
                maxHeight: '100%',
                maxWidth: '100%',
                containerType: 'inline-size',
                ...(pageH > pageW ? { height: '100%', width: 'auto' } : { width: '100%', height: 'auto' }),
              }}
              data-selectable
            >
              {plan.result?.title && (
                <p
                  className="absolute left-0 w-full truncate text-center text-fg"
                  style={{ top: `${(4 / pageH) * 100}%`, fontSize: `${cqw(plan.result.titleFontSize ?? 10)}cqw`, lineHeight: 1.2 }}
                >
                  {plan.result.title}
                </p>
              )}
              {page.cells.map((cell) => {
                const thumb = thumbFor(cell.number)
                return (
                  <div
                    key={cell.number}
                    className="absolute"
                    style={{
                      left: `${(cell.x / pageW) * 100}%`,
                      top: `${(cell.y / pageH) * 100}%`,
                      width: `${(cell.w / pageW) * 100}%`,
                      height: `${(cell.h / pageH) * 100}%`,
                    }}
                  >
                    {thumb && (
                      <img
                        src={thumb}
                        alt={cell.lines.join(' ')}
                        className="absolute object-contain"
                        style={{
                          left: `${((cell.imageX - cell.x) / cell.w) * 100}%`,
                          top: `${((cell.imageY - cell.y) / cell.h) * 100}%`,
                          width: `${(cell.imageW / cell.w) * 100}%`,
                          height: `${(cell.imageH / cell.h) * 100}%`,
                        }}
                      />
                    )}
                    {settings.labelHeight > 0 && (
                      <div
                        className="absolute inset-x-0 bottom-0 overflow-hidden px-[5%] text-fg"
                        style={{ height: `${(settings.labelHeight / cell.h) * 100}%`, fontSize: `${cqw(plan.result?.labelFontSize ?? 7)}cqw`, lineHeight: LINE_SPACING }}
                      >
                        {cell.lines.map((line) => (
                          <p key={line} className="truncate">
                            {line}
                          </p>
                        ))}
                      </div>
                    )}
                  </div>
                )
              })}
              {plan.result?.pageFontSize && (
                <p
                  className="absolute left-0 w-full text-center text-fg"
                  style={{ bottom: `${(1 / pageH) * 100}%`, fontSize: `${cqw(plan.result.pageFontSize)}cqw` }}
                >
                  {safePage + 1}/{pageCount}
                </p>
              )}
            </div>
          ) : task.running ? (
            <Empty icon={<FileDown />} title={t('sheet.running')} />
          ) : (
            <Empty icon={<FileDown />} title={t('sheet.empty')} description={t('sheet.emptyHint')} />
          )}
        </Panel>
        <Panel icon={<ScrollText />} title={t('sheet.logs')} extra={task.logs.length > 0 && <Badge>{task.logs.length}</Badge>}>
          <LogViewer entries={task.logs} emptyText={t('sheet.logsEmpty')} jumpLabel={t('sheet.jumpLatest')} />
        </Panel>
      </div>
    </div>
  )
}
