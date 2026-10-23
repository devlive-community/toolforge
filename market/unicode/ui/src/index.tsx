import { useEffect, useMemo, useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Badge, Button, CodeEditor, Empty, Input, Panel, Select, Switch, Tabs, Tooltip, cn } from '@toolforge/ui'
import { CopyButton, useDebouncedCall, useLaunchInput, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Brush, ListFilter, ScanText, Search, ShieldAlert, TextSearch } from 'lucide-react'
import { CharDetail, glyph } from './CharDetail'
import { FLAG_TONE, type Analysis, type CharInfo, type Found } from './types'

type Tab = 'inspect' | 'lookup'
type Form = 'none' | 'nfc' | 'nfkc'

const ROW_HEIGHT = 36

interface CleanOptions {
  removeInvisible: boolean
  removeBidi: boolean
  normalizeSpaces: boolean
  replaceConfusables: boolean
  removeControls: boolean
  form: Form
}

const DEFAULT_CLEAN: CleanOptions = { removeInvisible: true, removeBidi: true, normalizeSpaces: true, replaceConfusables: false, removeControls: false, form: 'none' }

const TONE_TEXT = { danger: 'text-danger', warning: 'text-warning', info: 'text-info' } as const

function Stat({ label, value, tone }: { label: string; value: number; tone?: 'danger' | 'warning' | 'info' }) {
  return (
    <div className="rounded-control border border-border bg-surface-2 px-3 py-2">
      <p className="text-[11px] text-fg-muted">{label}</p>
      <p className={cn('text-lg font-semibold tabular-nums', value > 0 && tone ? TONE_TEXT[tone] : 'text-fg')}>{value.toLocaleString()}</p>
    </div>
  )
}

function Inspect({ initial }: { initial: string }) {
  const { t, call } = usePlugin()
  const [text, setText] = useState(initial)
  const [onlyFlagged, setOnlyFlagged] = useState(false)
  const [selected, setSelected] = useState<Found | null>(null)
  const [cleanOptions, setCleanOptions] = usePluginState<CleanOptions>('clean', DEFAULT_CLEAN)
  const analysis = useDebouncedCall<Analysis>(text ? 'analyze' : null, text ? { text } : null, [text]).result
  const cleaned = useDebouncedCall<{ text: string; removed: number; replaced: number }>(text ? 'clean' : null, text ? { text, ...cleanOptions } : null, [text, cleanOptions]).result
  const listRef = useRef<HTMLDivElement>(null)

  const rows = useMemo(() => (analysis?.chars ?? []).filter((c) => !onlyFlagged || c.flags.length > 0), [analysis, onlyFlagged])
  const marks = useMemo(
    () =>
      (analysis?.chars ?? [])
        .filter((c) => c.flags.some((f) => f !== 'combining' && f !== 'unassigned'))
        .map((c) => ({ from: c.offset, to: c.offset + c.char.length, tone: c.flags.includes('bidi') || c.flags.includes('invisible') ? ('warn' as const) : ('alt' as const) })),
    [analysis],
  )
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({ count: rows.length, getScrollElement: () => listRef.current, estimateSize: () => ROW_HEIGHT, overscan: 16 })

  const open = (info: CharInfo) => {
    call<Found[]>('lookup', { query: info.code })
      .then((found) => setSelected(found[0] ?? null))
      .catch(() => {})
  }
  const counts = analysis?.counts
  const issues = counts ? counts.invisible + counts.bidi + counts.confusable + counts.control : 0
  const setClean = (patch: Partial<CleanOptions>) => setCleanOptions({ ...cleanOptions, ...patch })

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel icon={<ScanText />} title={t('inspect.input')} className="min-h-0 flex-1" bodyClassName="p-0">
          <CodeEditor value={text} onChange={setText} language="text" lineWrapping marks={marks} placeholder={t('inspect.placeholder')} aria-label={t('inspect.input')} />
        </Panel>
        <Panel
          icon={<Brush />}
          title={t('clean.title')}
          extra={cleaned && <Badge variant={cleaned.removed + cleaned.replaced > 0 ? 'success' : 'neutral'}>{t('clean.summary', { removed: cleaned.removed, replaced: cleaned.replaced })}</Badge>}
          actions={
            cleaned && (
              <>
                <CopyButton text={cleaned.text} label={t('clean.copy')} variant="outline" />
                <Button size="sm" onClick={() => setText(cleaned.text)} disabled={cleaned.text === text}>
                  {t('clean.apply')}
                </Button>
              </>
            )
          }
          className="shrink-0"
          bodyClassName="flex flex-wrap items-center gap-x-4 gap-y-2 p-3"
        >
          {(['removeInvisible', 'removeBidi', 'normalizeSpaces', 'replaceConfusables', 'removeControls'] as const).map((key) => (
            <label key={key} className="flex items-center gap-1.5 text-xs text-fg">
              <Switch size="sm" checked={cleanOptions[key]} onCheckedChange={(value) => setClean({ [key]: value })} aria-label={t(`clean.${key}`)} />
              {t(`clean.${key}`)}
            </label>
          ))}
          <Select<Form>
            size="sm"
            value={cleanOptions.form}
            onValueChange={(form) => setClean({ form })}
            aria-label={t('clean.form')}
            options={(['none', 'nfc', 'nfkc'] as Form[]).map((value) => ({ value, label: t(`forms.${value}`) }))}
          />
        </Panel>
      </div>

      <div className={cn('grid min-h-0 gap-3', selected ? 'grid-rows-[auto_minmax(0,1fr)_minmax(0,1fr)]' : 'grid-rows-[auto_minmax(0,1fr)]')}>
        {counts ? (
          <section className="space-y-2">
            <div className="grid grid-cols-4 gap-2">
              <Stat label={t('stats.chars')} value={counts.chars} />
              <Stat label={t('stats.graphemes')} value={counts.graphemes} />
              <Stat label={t('stats.utf8')} value={counts.utf8Bytes} />
              <Stat label={t('stats.utf16')} value={counts.utf16Units} />
              <Stat label={t('stats.invisible')} value={counts.invisible} tone="danger" />
              <Stat label={t('stats.bidi')} value={counts.bidi} tone="danger" />
              <Stat label={t('stats.confusable')} value={counts.confusable} tone="warning" />
              <Stat label={t('stats.space')} value={counts.space} tone="info" />
            </div>
            {counts.bidi > 0 && (
              <p className="flex items-start gap-1.5 rounded-control bg-danger-soft px-2.5 py-2 text-xs text-danger">
                <ShieldAlert className="mt-px size-3.5 shrink-0" />
                {t('warnings.bidi')}
              </p>
            )}
            {analysis.mixedWords.length > 0 && (
              <p className="rounded-control bg-warning-soft px-2.5 py-2 text-xs text-warning">{t('warnings.mixed', { words: analysis.mixedWords.slice(0, 5).join('、') })}</p>
            )}
            <div className="flex flex-wrap items-center gap-1.5 text-[11px] text-fg-muted">
              {(['nfc', 'nfd', 'nfkc', 'nfkd'] as const).map((form) => (
                <Tooltip key={form} content={t(`normalization.${form}Hint`)}>
                  <Badge variant={analysis.normalization[form] ? 'success' : 'neutral'}>{`${form.toUpperCase()} ${analysis.normalization[form] ? '✓' : '✗'}`}</Badge>
                </Tooltip>
              ))}
            </div>
          </section>
        ) : (
          <div />
        )}
        <Panel
          icon={<ListFilter />}
          title={t('list.title')}
          extra={issues > 0 && <Badge variant="danger">{t('list.issues', { count: issues })}</Badge>}
          actions={
            <label className="flex items-center gap-1.5 text-xs text-fg-muted">
              <Switch size="sm" checked={onlyFlagged} onCheckedChange={setOnlyFlagged} aria-label={t('list.onlyFlagged')} />
              {t('list.onlyFlagged')}
            </label>
          }
          footer={analysis?.truncated && <span className="text-warning">{t('list.truncated', { count: analysis.chars.length })}</span>}
          bodyClassName="p-0"
        >
          {rows.length === 0 ? (
            <Empty icon={<TextSearch />} title={text ? t('list.noFlagged') : t('list.empty')} description={text ? undefined : t('list.emptyHint')} />
          ) : (
            <div ref={listRef} className="h-full overflow-auto">
              <div className="relative" style={{ height: virtualizer.getTotalSize() }}>
                {virtualizer.getVirtualItems().map((row) => {
                  const info = rows[row.index]
                  return (
                    <button
                      key={row.key}
                      type="button"
                      onClick={() => open(info)}
                      className={cn('absolute left-0 grid w-full grid-cols-[36px_76px_minmax(0,1fr)_auto] items-center gap-2 border-b border-border px-3 text-left outline-none hover:bg-hover focus-visible:bg-hover', selected?.code === info.code && 'bg-primary-soft')}
                      style={{ top: row.start, height: ROW_HEIGHT }}
                    >
                      <span className="text-center text-lg text-fg">{glyph(info.char, info.flags)}</span>
                      <span className="font-mono text-[11px] text-fg-muted">{info.code}</span>
                      <span className="truncate text-xs text-fg">
                        {info.name ?? t('list.unnamed')}
                        {info.lookalike && <span className="ml-1.5 text-warning">{t('list.lookalike', { ascii: info.lookalike })}</span>}
                      </span>
                      <span className="flex gap-1">
                        {info.flags.map((flag) => (
                          <Badge key={flag} variant={FLAG_TONE[flag]}>
                            {t(`flags.${flag}`)}
                          </Badge>
                        ))}
                      </span>
                    </button>
                  )
                })}
              </div>
            </div>
          )}
        </Panel>
        {selected && <CharDetail found={selected} onClose={() => setSelected(null)} />}
      </div>
    </div>
  )
}

function Lookup({ active }: { active: boolean }) {
  const { t } = usePlugin()
  const [query, setQuery] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)
  // 页签一直挂载，切换到查找页时再聚焦输入框
  useEffect(() => {
    if (active) inputRef.current?.focus()
  }, [active])
  const [selected, setSelected] = useState<Found | null>(null)
  const results = useDebouncedCall<Found[]>(query.trim() ? 'lookup' : null, query.trim() ? { query } : null, [query]).result ?? []
  const current = selected && results.some((r) => r.code === selected.code) ? selected : (results[0] ?? null)
  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_380px] gap-3">
      <Panel
        title={<Input ref={inputRef} value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t('lookup.placeholder')} leading={<Search />} wrapperClassName="w-96" aria-label={t('lookup.placeholder')} />}
        extra={results.length > 0 && <Badge>{results.length}</Badge>}
        bodyClassName="overflow-auto p-2"
      >
        {results.length === 0 ? (
          <Empty icon={<Search />} title={query.trim() ? t('lookup.none') : t('lookup.empty')} description={t('lookup.hint')} />
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(92px,1fr))] gap-1.5">
            {results.map((found) => (
              <Tooltip key={found.code} content={found.name ?? found.code}>
                <button
                  type="button"
                  onClick={() => setSelected(found)}
                  className={cn('flex flex-col items-center gap-1 rounded-control border px-1 py-2 outline-none hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring', current?.code === found.code ? 'border-primary bg-primary-soft' : 'border-border')}
                >
                  <span className="text-2xl text-fg">{glyph(found.char, found.flags)}</span>
                  <span className="font-mono text-[10px] text-fg-muted">{found.code}</span>
                </button>
              </Tooltip>
            ))}
          </div>
        )}
      </Panel>
      {current ? <CharDetail found={current} /> : <Panel><Empty title={t('lookup.select')} /></Panel>}
    </div>
  )
}

function UnicodeInspector() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<Tab>('inspect')
  const [launched, setLaunched] = useState<{ text: string; seq: number } | null>(null)
  useLaunchInput((text) => {
    setTab('inspect')
    setLaunched((prev) => ({ text, seq: (prev?.seq ?? 0) + 1 }))
  })
  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Tabs<Tab>
        value={tab}
        onValueChange={setTab}
        className="self-start"
        aria-label={t('name')}
        items={[
          { value: 'inspect', label: t('tabs.inspect'), icon: <ScanText /> },
          { value: 'lookup', label: t('tabs.lookup'), icon: <Search /> },
        ]}
      />
      <div className="min-h-0 flex-1">
        <div className="h-full" hidden={tab !== 'inspect'}>
          <Inspect key={launched?.seq ?? 0} initial={launched?.text ?? ''} />
        </div>
        <div className="h-full" hidden={tab !== 'lookup'}>
          <Lookup active={tab === 'lookup'} />
        </div>
      </div>
    </div>
  )
}

export default UnicodeInspector
