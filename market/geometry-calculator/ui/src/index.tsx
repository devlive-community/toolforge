import { useState } from 'react'
import { Input, Panel, SegmentedControl, Select, Spinner } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Ruler } from 'lucide-react'

interface Row {
  label: string
  value: number
  formula: string
  substituted: string
}

interface SolveOutput {
  shape: string
  kind: string
  rows: Row[]
}

interface MaterialOutput {
  kind: string
  rows: Row[]
  buy: number
  unit: string
}

type Shape = 'circle' | 'triangle' | 'rectangle' | 'trapezoid' | 'sphere' | 'cylinder' | 'cone' | 'cuboid'

const SHAPE_FIELDS: Record<Shape, (keyof SolveArgs)[]> = {
  circle: ['r'],
  triangle: ['a', 'b', 'c'],
  rectangle: ['w', 'l'],
  trapezoid: ['a', 'b', 'h'],
  sphere: ['r'],
  cylinder: ['r', 'h'],
  cone: ['r', 'h'],
  cuboid: ['w', 'l', 'h'],
}

interface SolveArgs {
  shape: Shape
  r?: number
  a?: number
  b?: number
  c?: number
  w?: number
  l?: number
  h?: number
}

interface MaterialArgs {
  kind: 'paint' | 'tiles'
  length: number
  width: number
  height?: number
  openings?: number
  coverage?: number
  coats?: number
  tileLength?: number
  tileWidth?: number
  waste?: number
}

const SHAPES: Shape[] = ['circle', 'triangle', 'rectangle', 'trapezoid', 'sphere', 'cylinder', 'cone', 'cuboid']

export default function GeometryCalculator() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<'solve' | 'materials'>('solve')

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <SegmentedControl
        value={tab}
        onValueChange={setTab}
        aria-label={t('tab.label')}
        options={[
          { value: 'solve', label: t('tab.solve') },
          { value: 'materials', label: t('tab.materials') },
        ]}
      />
      {tab === 'solve' ? <SolveTab /> : <MaterialsTab />}
      <p className="shrink-0 text-xs text-fg-subtle">{t('help')}</p>
    </div>
  )
}

// —— 以下两个子组件各自持有状态；错误通过 errorMessage 展示在结果面板里 ——

function SolveTab() {
  const { t, errorMessage } = usePlugin()
  const [shape, setShape] = useState<Shape>('circle')
  const [values, setValues] = useState<Record<string, number | undefined>>({})

  const args: SolveArgs | null = (() => {
    const missing = SHAPE_FIELDS[shape].some((key) => !values[key] || (values[key] as number) <= 0)
    if (missing) return null
    return { shape, ...values } as SolveArgs
  })()
  const { result, error, pending } = useDebouncedCall<SolveOutput>(args ? 'solve' : null, args, [shape, values])

  return (
    <div className="grid min-h-0 flex-1 grid-cols-[minmax(280px,0.8fr)_minmax(0,1.2fr)] gap-3">
      <Panel icon={<Ruler />} title={t('shape.label')} bodyClassName="space-y-2.5 p-3">
        <Select
          value={shape}
          onValueChange={(value) => setShape(value as Shape)}
          options={SHAPES.map((key) => ({ value: key, label: t(`shape.${key}`) }))}
          aria-label={t('shape.label')}
        />
        {SHAPE_FIELDS[shape].map((key) => (
          <Input
            key={key}
            type="number"
            value={values[key] === undefined ? '' : String(values[key])}
            onChange={(event) => setValues((prev) => ({ ...prev, [key]: event.target.value === '' ? undefined : Number(event.target.value) }))}
            min={0}
            placeholder={t(`field.${key}`)}
            aria-label={t(`field.${key}`)}
          />
        ))}
      </Panel>
      <ResultPanel rows={result?.rows} error={error} pending={pending} errorMessage={errorMessage} emptyTitle={t('results.empty')} />
    </div>
  )
}

function MaterialsTab() {
  const { t, errorMessage } = usePlugin()
  const [kind, setKind] = useState<'paint' | 'tiles'>('paint')
  const [values, setValues] = useState<Record<string, number | undefined>>({ length: 5, width: 4, height: 2.8, coverage: 10, coats: 2, waste: 10 }) // tf-allow: 默认值
  const set = (key: string, value: number | undefined) => setValues((prev) => ({ ...prev, [key]: value }))

  const common = values.length && values.length > 0 && values.width && values.width > 0
  const paintReady = common && kind === 'paint' && values.height && values.height > 0
  const tilesReady = common && kind === 'tiles' && values.tileLength && values.tileLength > 0 && values.tileWidth && values.tileWidth > 0
  const args: MaterialArgs | null =
    kind === 'paint'
      ? paintReady
        ? { kind, length: values.length!, width: values.width!, height: values.height, openings: values.openings, coverage: values.coverage, coats: values.coats }
        : null
      : tilesReady
        ? { kind, length: values.length!, width: values.width!, tileLength: values.tileLength, tileWidth: values.tileWidth, waste: values.waste }
        : null
  const { result, error, pending } = useDebouncedCall<MaterialOutput>(args ? 'materials' : null, args, [kind, values])

  return (
    <div className="grid min-h-0 flex-1 grid-cols-[minmax(280px,0.8fr)_minmax(0,1.2fr)] gap-3">
      <Panel icon={<Ruler />} title={t('tab.materials')} bodyClassName="space-y-2.5 p-3">
        <SegmentedControl
          value={kind}
          onValueChange={setKind}
          aria-label={t('api.materials.kind')}
          options={[
            { value: 'paint', label: t('material.paint') },
            { value: 'tiles', label: t('material.tiles') },
          ]}
        />
        {(['length', 'width'] as const).map((key) => (
          <NumberField key={key} label={t(`field.${key}`)} value={values[key]} onChange={(v) => set(key, v)} />
        ))}
        {kind === 'paint'
          ? (['height', 'openings', 'coverage', 'coats'] as const).map((key) => (
              <NumberField key={key} label={t(`field.${key}`)} value={values[key]} onChange={(v) => set(key, v)} />
            ))
          : (['tileLength', 'tileWidth', 'waste'] as const).map((key) => (
              <NumberField key={key} label={t(`field.${key}`)} value={values[key]} onChange={(v) => set(key, v)} />
            ))}
      </Panel>
      <ResultPanel
        rows={result?.rows}
        error={error}
        pending={pending}
        errorMessage={errorMessage}
        emptyTitle={t('results.empty')}
        buy={result ? { value: result.buy, unit: t(`unit.${result.unit}`) } : undefined}
      />
    </div>
  )
}

function NumberField({ label, value, onChange }: { label: string; value: number | undefined; onChange: (value: number | undefined) => void }) {
  return (
    <Input
      type="number"
      value={value === undefined ? '' : String(value)}
      onChange={(event) => onChange(event.target.value === '' ? undefined : Number(event.target.value))}
      min={0}
      placeholder={label}
      aria-label={label}
    />
  )
}

function ResultPanel({
  rows,
  error,
  pending,
  errorMessage,
  emptyTitle,
  buy,
}: {
  rows?: Row[]
  error: { code: string; params?: Record<string, unknown> } | null
  pending: boolean
  errorMessage: (error: { code: string; params?: Record<string, unknown> }) => string
  emptyTitle: string
  buy?: { value: number; unit: string }
}) {
  const { t } = usePlugin()
  return (
    <Panel
      icon={<Ruler />}
      title={t('results.title')}
      className="min-h-0"
      bodyClassName="min-h-0 overflow-auto space-y-1 py-1"
      extra={pending && <Spinner className="size-3.5" />}
    >
      {error ? (
        <p className="p-3 text-xs text-danger">{errorMessage(error)}</p>
      ) : rows && rows.length > 0 ? (
        <>
          {rows.map((row) => (
            <div key={row.label} className="space-y-0.5 px-3 py-1.5">
              <ValueRow label={t(`rows.${row.label}`)} value={format(row.value)} labelWidth="w-24" />
              <p className="pl-[calc(6rem+0.5rem)] font-mono text-[11px] text-fg-subtle">
                {row.formula}{' → '}{row.substituted}
              </p>
            </div>
          ))}
          {buy && (
            <div className="mx-3 mt-2 rounded-control bg-primary-soft px-3 py-2 text-xs font-medium text-primary-fg">
              {t('results.buy', { value: buy.value, unit: buy.unit })}
            </div>
          )}
        </>
      ) : (
        <p className="py-6 text-center text-xs text-fg-subtle">{emptyTitle}</p>
      )}
    </Panel>
  )
}

function format(value: number): string {
  if (Number.isInteger(value)) return String(value)
  return String(Number(value.toPrecision(8)))
}
