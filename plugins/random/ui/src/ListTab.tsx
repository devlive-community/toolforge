import { Button, CodeEditor, Empty, NumberInput, Panel, SegmentedControl, Switch } from '@toolforge/ui'
import { CopyButton, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { ListOrdered, Shuffle, Trophy, Users } from 'lucide-react'
import { Field, MetaBadge, SeedField, ToggleRow, useRun, type Meta, type SeedProps } from './shared'

type Mode = 'draw' | 'shuffle' | 'groups'
type SplitBy = 'groups' | 'size'

interface ListResult extends Meta {
  total: number
  winners?: string[]
  items?: string[]
  groups?: string[][]
}

const lineCount = (text: string) => text.split('\n').filter((l) => l.trim()).length

export function ListTab({ seed, setSeed }: SeedProps) {
  const { t, errorMessage } = usePlugin()
  const [items, setItems] = usePluginState('list.items', '')
  const [mode, setMode] = usePluginState<Mode>('list.mode', 'draw')
  const [count, setCount] = usePluginState('list.count', 1)
  const [weighted, setWeighted] = usePluginState('list.weighted', false)
  const [splitBy, setSplitBy] = usePluginState<SplitBy>('list.splitBy', 'groups')
  const [split, setSplit] = usePluginState('list.split', 2)
  const { result, error, pending, round, run } = useRun<ListResult>()

  const start = () =>
    run(mode, {
      items,
      count,
      weighted,
      seed,
      ...(splitBy === 'groups' ? { groups: split } : { size: split }),
    })
  const shown = result && (result.winners ?? result.items)
  const copyText = result?.groups ? result.groups.map((g, i) => `${t('list.group', { n: i + 1 })}: ${g.join(', ')}`).join('\n') : (shown ?? []).join('\n')
  const lines = lineCount(items)

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(280px,340px)_minmax(0,1fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel
          icon={<Users />}
          title={t('list.items')}
          extra={<span className="text-xs text-fg-subtle tabular-nums">{t('list.lines', { count: lines })}</span>}
          className="min-h-40 flex-1"
        >
          <CodeEditor
            value={items}
            onChange={setItems}
            language="text"
            placeholder={t(weighted && mode === 'draw' ? 'list.weightedPlaceholder' : 'list.placeholder')}
            aria-label={t('list.items')}
          />
        </Panel>
        <Panel className="shrink-0" bodyClassName="space-y-4 p-4">
          <SegmentedControl<Mode>
            value={mode}
            onValueChange={setMode}
            className="w-full"
            aria-label={t('list.mode')}
            options={(['draw', 'shuffle', 'groups'] as const).map((m) => ({
              value: m,
              label: t(`list.modes.${m}`),
            }))}
          />
          {mode === 'draw' && (
            <>
              <Field label={t('list.count')}>
                <NumberInput value={count} onValueChange={setCount} min={1} max={10000} className="w-full" aria-label={t('list.count')} />
              </Field>
              <div className="rounded-control border border-border px-3 py-1.5">
                <ToggleRow label={t('list.weighted')}>
                  <Switch size="sm" checked={weighted} onCheckedChange={setWeighted} aria-label={t('list.weighted')} />
                </ToggleRow>
              </div>
            </>
          )}
          {mode === 'groups' && (
            <Field
              label={
                <SegmentedControl<SplitBy>
                  size="sm"
                  value={splitBy}
                  onValueChange={setSplitBy}
                  aria-label={t('list.splitBy')}
                  options={[
                    { value: 'groups', label: t('list.byGroups') },
                    { value: 'size', label: t('list.bySize') },
                  ]}
                />
              }
            >
              <NumberInput
                value={split}
                onValueChange={setSplit}
                min={1}
                max={10000}
                className="w-full"
                aria-label={t(splitBy === 'groups' ? 'list.byGroups' : 'list.bySize')}
              />
            </Field>
          )}
          <SeedField seed={seed} setSeed={setSeed} />
          <Button variant="primary" block onClick={start} disabled={pending || lines === 0}>
            {mode === 'draw' ? <Trophy /> : mode === 'shuffle' ? <Shuffle /> : <Users />}
            {t(`list.start.${mode}`)}
          </Button>
        </Panel>
      </div>

      <Panel
        icon={<ListOrdered />}
        title={t('result.title')}
        extra={!error && <MetaBadge meta={result} seed={seed} />}
        actions={result && !error && <CopyButton text={copyText} label={t('result.copy')} variant="outline" />}
        footer={result && !error && <span>{t('list.total', { count: result.total })}</span>}
        bodyClassName="overflow-auto p-3"
      >
        {error ? (
          <p className="p-1 text-[13px] text-danger">{errorMessage(error)}</p>
        ) : !result ? (
          <Empty icon={<ListOrdered />} title={t('result.empty')} description={t('list.emptyHint')} />
        ) : result.groups ? (
          <div key={round} className="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-2">
            {result.groups.map((group, i) => (
              <div key={i} className="rounded-control border border-border">
                <p className="flex items-center justify-between border-b border-border bg-surface-2 px-3 py-1.5 text-xs font-medium text-fg">
                  {t('list.group', { n: i + 1 })}
                  <span className="text-fg-subtle tabular-nums">{group.length}</span>
                </p>
                <ul className="space-y-0.5 p-2 text-[13px] text-fg" data-selectable>
                  {group.map((name, j) => (
                    <li key={j} className="truncate px-1">
                      {name}
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        ) : (
          <ol key={round} className="divide-y divide-border" data-selectable>
            {shown?.map((name, i) => (
              <li key={i} className="flex items-baseline gap-3 px-2 py-1.5">
                <span className="w-8 shrink-0 text-right font-mono text-xs text-fg-subtle tabular-nums">{i + 1}</span>
                <span className={result.winners && shown.length <= 3 ? 'text-lg font-semibold text-fg' : 'text-[13px] text-fg'}>{name}</span>
              </li>
            ))}
          </ol>
        )}
      </Panel>
    </div>
  )
}
