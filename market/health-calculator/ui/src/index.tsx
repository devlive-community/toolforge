import { useState } from 'react'
import { Badge, Input, Panel, SegmentedControl, Select, Spinner } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { HeartPulse, TriangleAlert } from 'lucide-react'

interface BmiOut {
  bmi: number
  cnCategory: string
  whoCategory: string
  healthyMin: number
  healthyMax: number
  whr: number | null
  whrRisk: boolean | null
}
interface EnergyOut {
  bmr: number
  tdee: number
  cut: number
  bulk: number
}
interface Zone {
  label: string
  low: number
  high: number
}
interface HeartRateOut {
  max: number
  resting: number
  zones: Zone[]
}
interface BodyOut {
  bsa: number
  idealWeight: number
  idealMin: number
  idealMax: number
  waterLiters: number
  proteinGrams: number
  whr: number | null
  whrRisk: boolean | null
}

type Tool = 'bmi' | 'energy' | 'heart_rate' | 'body'
const TOOLS: Tool[] = ['bmi', 'energy', 'heart_rate', 'body']

/** 每个工具需要的输入字段 */
const FIELDS: Record<Tool, { key: string; default?: number; options?: string[] }[]> = {
  bmi: [
    { key: 'height', default: 170 }, // tf-allow: 默认值
    { key: 'weight', default: 70 }, // tf-allow: 默认值
    { key: 'waist' },
    { key: 'hip' },
  ],
  energy: [
    { key: 'height', default: 175 }, // tf-allow: 默认值
    { key: 'weight', default: 75 }, // tf-allow: 默认值
    { key: 'age', default: 30 }, // tf-allow: 默认值
    { key: 'sex', options: ['male', 'female'] },
    { key: 'activity', default: 1.55 }, // tf-allow: 默认值
  ],
  heart_rate: [
    { key: 'age', default: 40 }, // tf-allow: 默认值
    { key: 'resting', default: 60 }, // tf-allow: 默认值
  ],
  body: [
    { key: 'height', default: 175 }, // tf-allow: 默认值
    { key: 'weight', default: 72 }, // tf-allow: 默认值
    { key: 'sex', options: ['male', 'female'] },
    { key: 'waist' },
    { key: 'hip' },
  ],
}

export default function HealthCalculator() {
  const { t } = usePlugin()
  const [tool, setTool] = useState<Tool>('bmi')

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <SegmentedControl
        value={tool}
        onValueChange={(value) => setTool(value as Tool)}
        aria-label={t('tool.label')}
        options={TOOLS.map((key) => ({ value: key, label: t(`tool.${key}`) }))}
      />
      <ToolPanel tool={tool} />
      <p className="flex shrink-0 items-center gap-1.5 text-xs text-warning">
        <TriangleAlert className="size-3.5 shrink-0" />
        {t('disclaimer')}
      </p>
    </div>
  )
}

function ToolPanel({ tool }: { tool: Tool }) {
  const { t, errorMessage } = usePlugin()
  const [values, setValues] = useState<Record<string, number | string | undefined>>(() => {
    const init: Record<string, number | string | undefined> = {}
    for (const field of FIELDS[tool]) {
      init[field.key] = field.default
    }
    if (FIELDS[tool].some((f) => f.key === 'sex')) init.sex = 'male' // tf-allow: 默认值
    return init
  })
  // 切工具时补默认值
  const [lastTool, setLastTool] = useState(tool)
  if (tool !== lastTool) {
    setLastTool(tool)
    const next: Record<string, number | string | undefined> = {}
    for (const field of FIELDS[tool]) {
      next[field.key] = field.default
    }
    if (FIELDS[tool].some((f) => f.key === 'sex')) next.sex = 'male' // tf-allow: 默认值
    setValues(next)
  }

  const numeric = (key: string) => (typeof values[key] === 'number' ? (values[key] as number) : undefined)
  const args: Record<string, unknown> | null = (() => {
    const out: Record<string, unknown> = {}
    for (const field of FIELDS[tool]) {
      const value = values[field.key]
      if (field.options) {
        if (typeof value !== 'string') return null
        out[field.key] = value
      } else {
        const n = numeric(field.key)
        if (n === undefined) {
          // 可选字段缺省就不传；必填缺失时暂不调用
          if (field.key === 'waist' || field.key === 'hip') continue
          return null
        }
        if (n <= 0) return null
        out[field.key] = n
      }
    }
    return out
  })()

  const { result, error, pending } = useDebouncedCall<Record<string, unknown>>(
    args ? tool : null,
    args,
    [tool, values],
  )

  return (
    <div className="grid min-h-0 flex-1 grid-cols-[minmax(280px,0.7fr)_minmax(0,1.3fr)] gap-3">
      <Panel icon={<HeartPulse />} title={t(`tool.${tool}`)} bodyClassName="space-y-2.5 p-3">
        {FIELDS[tool].map((field) =>
          field.options ? (
            <Select
              key={field.key}
              value={String(values[field.key] ?? field.options[0])}
              onValueChange={(value) => setValues((prev) => ({ ...prev, [field.key]: value }))}
              options={field.options.map((option) => ({ value: option, label: t(`sex.${option}`) }))}
              aria-label={t(`field.${field.key}`)}
            />
          ) : (
            <Input
              key={field.key}
              type="number"
              value={values[field.key] === undefined ? '' : String(values[field.key])}
              onChange={(event) =>
                setValues((prev) => ({ ...prev, [field.key]: event.target.value === '' ? undefined : Number(event.target.value) }))
              }
              min={0}
              placeholder={t(`field.${field.key}`)}
              aria-label={t(`field.${field.key}`)}
            />
          ),
        )}
      </Panel>
      <ResultPanel tool={tool} result={result} error={error} pending={pending} errorMessage={errorMessage} />
    </div>
  )
}

type AppErrorLike = { code: string; params?: Record<string, unknown> }

function ResultPanel({
  tool,
  result,
  error,
  pending,
  errorMessage,
}: {
  tool: Tool
  result: Record<string, unknown> | null
  error: AppErrorLike | null
  pending: boolean
  errorMessage: (error: AppErrorLike) => string
}) {
  const { t } = usePlugin()
  const r = result as unknown as (BmiOut & EnergyOut & HeartRateOut & BodyOut) | null
  return (
    <Panel
      icon={<HeartPulse />}
      title={t('results.title')}
      className="min-h-0"
      bodyClassName="min-h-0 overflow-auto space-y-1 py-1"
      extra={pending && <Spinner className="size-3.5" />}
    >
      {error ? (
        <p className="p-3 text-xs text-danger">{errorMessage(error)}</p>
      ) : r ? (
        <div className="space-y-1 px-3 py-2">
          {tool === 'bmi' && r.cnCategory !== undefined && (
            <>
              <ValueRow label={t('bmi.bmi')} value={r.bmi.toFixed(1)} labelWidth="w-28" />
              <div className="flex items-center gap-2 pl-[calc(7rem+0.5rem)]">
                <Badge variant={r.cnCategory === 'normal' ? 'success' : 'neutral'}>
                  {t('bmi.cn')}: {t(`category.${r.cnCategory}`)}
                </Badge>
                <Badge variant={r.whoCategory === 'normal' ? 'success' : 'neutral'}>
                  {t('bmi.who')}: {t(`category.${r.whoCategory}`)}
                </Badge>
              </div>
              <ValueRow label={t('bmi.healthyRange')} value={`${r.healthyMin.toFixed(1)} – ${r.healthyMax.toFixed(1)} ${t('units.kg')}`} labelWidth="w-28" />
              {r.whr !== null && <ValueRow label={t('body.whr')} value={r.whr.toFixed(2)} labelWidth="w-28" />}
              {r.whrRisk !== null && (
                <div className="pl-[calc(7rem+0.5rem)]">
                  <Badge variant={r.whrRisk ? 'danger' : 'success'}>{t('body.central')}</Badge>
                </div>
              )}
            </>
          )}
          {tool === 'energy' && r.tdee !== undefined && (
            <>
              <ValueRow label={t('energy.bmr')} value={`${Math.round(r.bmr)} ${t('units.kcal')}`} labelWidth="w-28" />
              <ValueRow label={t('energy.tdee')} value={`${Math.round(r.tdee)} ${t('units.kcal')}`} labelWidth="w-28" />
              <ValueRow label={t('energy.cut')} value={`${Math.round(r.cut)} ${t('units.kcal')}`} labelWidth="w-28" />
              <ValueRow label={t('energy.bulk')} value={`${Math.round(r.bulk)} ${t('units.kcal')}`} labelWidth="w-28" />
            </>
          )}
          {tool === 'heart_rate' && r.zones !== undefined && (
            <>
              <ValueRow label={t('zones.max')} value={`${Math.round(r.max)} ${t('units.bpm')}`} labelWidth="w-28" />
              {r.zones.map((zone) => (
                <ValueRow
                  key={zone.label}
                  label={t(`zones.${zone.label}`)}
                  value={`${Math.round(zone.low)} – ${Math.round(zone.high)} ${t('units.bpm')}`}
                  labelWidth="w-28"
                />
              ))}
            </>
          )}
          {tool === 'body' && r.idealWeight !== undefined && (
            <>
              <ValueRow label={t('body.bsa')} value={`${r.bsa.toFixed(2)} ${t('units.m2')}`} labelWidth="w-28" />
              <ValueRow label={t('body.ideal')} value={`${r.idealWeight.toFixed(1)} ${t('units.kg')}`} labelWidth="w-28" />
              <ValueRow label={t('body.idealRange')} value={`${r.idealMin.toFixed(1)} – ${r.idealMax.toFixed(1)} ${t('units.kg')}`} labelWidth="w-28" />
              <ValueRow label={t('body.water')} value={`${r.waterLiters.toFixed(1)} ${t('units.l')}`} labelWidth="w-28" />
              <ValueRow label={t('body.protein')} value={`${Math.round(r.proteinGrams)} ${t('units.g')}`} labelWidth="w-28" />
              {r.whr !== null && (
                <>
                  <ValueRow label={t('body.whr')} value={r.whr.toFixed(2)} labelWidth="w-28" />
                  <div className="pl-[calc(7rem+0.5rem)]">
                    <Badge variant={r.whrRisk ? 'danger' : 'success'}>{t('body.central')}</Badge>
                  </div>
                </>
              )}
            </>
          )}
        </div>
      ) : (
        <p className="py-6 text-center text-xs text-fg-subtle">{t('results.empty')}</p>
      )}
    </Panel>
  )
}
