import { useState } from 'react'
import { Input, Panel, Select, Spinner } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Atom, Sigma } from 'lucide-react'

interface SolveOut {
  card: string
  target: string
  value: number
  unit: string
  formula: string
  substituted: string
}

interface Constant {
  key: string
  value: number
  unit: string
}

const CARDS = [
  'kinematics_v', 'kinematics_s', 'kinematics_v2', 'newton', 'friction', 'work', 'power',
  'potential', 'kinetic', 'ohm', 'electric_power', 'electric_power_r', 'series_r', 'parallel_r',
  'refraction', 'heat',
] as const

type Card = (typeof CARDS)[number]

/** 每张卡的变量与单位（SI） */
const VARS: Record<Card, Record<string, string>> = {
  kinematics_v: { v: 'm/s', v0: 'm/s', a: 'm/s2', t: 's' },
  kinematics_s: { s: 'm', v0: 'm/s', a: 'm/s2', t: 's' },
  kinematics_v2: { v: 'm/s', v0: 'm/s', a: 'm/s2', s: 'm' },
  newton: { f: 'N', m: 'kg', a: 'm/s2' },
  friction: { f: 'N', mu: '1', n: 'N' },
  work: { w: 'J', f: 'N', s: 'm' },
  power: { p: 'W', w: 'J', t: 's' },
  potential: { ep: 'J', m: 'kg', g: 'm/s2', h: 'm' },
  kinetic: { ek: 'J', m: 'kg', v: 'm/s' },
  ohm: { u: 'V', i: 'A', r: 'Ω' },
  electric_power: { p: 'W', u: 'V', i: 'A' },
  electric_power_r: { p: 'W', i: 'A', r: 'Ω' },
  series_r: { r: 'Ω', r1: 'Ω', r2: 'Ω' },
  parallel_r: { r: 'Ω', r1: 'Ω', r2: 'Ω' },
  refraction: { n: '1', c: 'm/s', v: 'm/s' },
  heat: { q: 'J', m: 'kg', cc: 'J/(kg·K)', dt: 'K' },
}

export default function PhysicsCalculator() {
  const { t, errorMessage } = usePlugin()
  const [card, setCard] = useState<Card>('kinematics_v')
  const [target, setTarget] = useState('v')
  const [values, setValues] = useState<Record<string, number | undefined>>({ v0: 2, a: 3, t: 4 }) // tf-allow: 默认值

  const varNames = Object.keys(VARS[card])
  const switchCard = (next: Card) => {
    setCard(next)
    const names = Object.keys(VARS[next])
    setTarget(names[0])
    const init: Record<string, number | undefined> = {}
    for (const name of names) {
      if (name !== names[0]) init[name] = undefined
    }
    setValues(init)
  }

  const args = (() => {
    const given: Record<string, number> = {}
    for (const name of varNames) {
      if (name === target) continue
      const value = values[name]
      if (value === undefined || !Number.isFinite(value)) return null
      given[name] = value
    }
    return { card, target, given }
  })()
  const { result, error, pending } = useDebouncedCall<SolveOut>(args ? 'solve' : null, args, [card, target, values])
  const constants = useDebouncedCall<Constant[]>('constants', {}, [])

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(300px,0.8fr)_minmax(0,1.2fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel icon={<Atom />} title={t('input.title')} bodyClassName="space-y-2.5 p-3">
          <Select
            value={card}
            onValueChange={(value) => switchCard(value as Card)}
            options={CARDS.map((key) => ({ value: key, label: t(`cards.${key}`) }))}
            aria-label={t('api.solve.card')}
          />
          <Select
            value={target}
            onValueChange={setTarget}
            options={varNames.map((name) => ({ value: name, label: t(`vars.${name}`) }))}
            aria-label={t('api.solve.target')}
          />
          {varNames
            .filter((name) => name !== target)
            .map((name) => (
              <Input
                key={name}
                type="number"
                value={values[name] === undefined ? '' : String(values[name])}
                onChange={(event) => setValues((prev) => ({ ...prev, [name]: event.target.value === '' ? undefined : Number(event.target.value) }))}
                placeholder={`${t(`vars.${name}`)}`}
                aria-label={t(`vars.${name}`)}
              />
            ))}
        </Panel>
        <Panel icon={<Sigma />} title={t('constants.title')} className="min-h-0 shrink-0" bodyClassName="py-1">
          {(constants.result ?? []).map((constant) => (
            <ValueRow key={constant.key} label={t(`vars.${constant.key}`)} value={`${constant.value} ${t(`units.${constant.unit}`)}`} labelWidth="w-28" />
          ))}
        </Panel>
      </div>
      <Panel
        icon={<Atom />}
        title={t('results.title')}
        className="min-h-0"
        bodyClassName="p-3"
        extra={pending && <Spinner className="size-3.5" />}
      >
        {error ? (
          <p className="text-xs text-danger">{errorMessage(error)}</p>
        ) : result ? (
          <div className="space-y-2">
            <ValueRow
              label={t(`vars.${result.target}`)}
              value={`${format(result.value)} ${t(`units.${result.unit}`)}`}
              labelWidth="w-32"
            />
            <p className="pl-[calc(8rem+0.5rem)] font-mono text-[11px] text-fg-subtle">
              {result.formula} {' → '}{result.substituted}
            </p>
          </div>
        ) : (
          <p className="py-6 text-center text-xs text-fg-subtle">{t('results.empty')}</p>
        )}
      </Panel>
    </div>
  )
}

function format(value: number): string {
  if (Number.isInteger(value)) return String(value)
  if (value !== 0 && (Math.abs(value) >= 1e9 || Math.abs(value) < 1e-6)) return value.toExponential(6)
  return String(Number(value.toPrecision(10)))
}
