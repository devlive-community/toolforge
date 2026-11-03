import { useState } from 'react'
import { Badge, Button, Input, Panel, SegmentedControl, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { MoonStar, Sparkles, Wand2 } from 'lucide-react'

interface StickOut {
  index: number
  level: string
  title: string
  textZh: string
  textEn: string
  seed: number
}
interface TarotCard {
  key: string
  nameZh: string
  nameEn: string
  upright: boolean
  meaningZh: string
  meaningEn: string
  position: string
}
interface TarotOut {
  spread: string
  cards: TarotCard[]
  seed: number
}
interface DailyOut {
  date: number
  card: TarotCard
  level: string
  seed: number
}

export default function FortuneTarot() {
  const { t } = usePlugin()

  const [tab, setTab] = useState<'stick' | 'tarot' | 'daily'>('stick')

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <SegmentedControl
        value={tab}
        onValueChange={setTab}
        aria-label={t('tab.label')}
        options={[
          { value: 'stick', label: t('tab.stick') },
          { value: 'tarot', label: t('tab.tarot') },
          { value: 'daily', label: t('tab.daily') },
        ]}
      />
      <p className="flex shrink-0 items-center gap-1.5 text-xs text-warning">
        <Sparkles className="size-3.5 shrink-0" />
        {t('disclaimer')}
      </p>
      {tab === 'stick' ? <StickTab /> : tab === 'tarot' ? <TarotTab /> : <DailyTab />}
    </div>
  )
}

function StickTab() {
  const { t, call } = usePlugin()
  const [drawn, setDrawn] = useState<StickOut | null>(null)
  const [pending, setPending] = useState(false)

  const drawStick = async (value: number) => {
    try {
      setDrawn(await call<StickOut>('stick', { seed: value }))
    } finally {
      setPending(false)
    }
  }

  const draw = () => {
    setPending(true)
    const next = Date.now() % 1_000_000
    setDrawn(null)
    // 延迟一拍制造「摇签」节奏，结果仍由种子决定
    setTimeout(() => {
      void drawStick(next)
    }, 300)
  }

  return (
    <Panel icon={<Wand2 />} title={t('tab.stick')} className="min-h-0 flex-1" bodyClassName="space-y-3 p-3">
      <Button onClick={draw} disabled={pending}>
        <Wand2 className="size-4" />
        {t('stick.draw')}
      </Button>
      {pending && <Spinner className="size-4" />}
      {drawn && (
        <div className="space-y-2 rounded-card border border-border bg-surface-2 p-4">
          <div className="flex items-center justify-between">
            <span className="text-sm font-semibold text-fg">{drawn.title}</span>
            <Badge variant={drawn.level === 'upper' ? 'success' : drawn.level === 'lower' ? 'danger' : 'neutral'}>
              {t(`levels.${drawn.level}`)}
            </Badge>
          </div>
          <p className="leading-relaxed text-fg">{drawn.textZh}</p>
          <p className="text-xs text-fg-subtle">{t('seedHint', { seed: drawn.seed })}</p>
        </div>
      )}
    </Panel>
  )
}

function TarotTab() {
  const { t, errorMessage } = usePlugin()
  const [spread, setSpread] = useState<'single' | 'three'>('three')
  const [seed, setSeed] = useState(() => Date.now() % 1_000_000)
  const args = { seed, spread }
  const { result, error, pending } = useDebouncedCall<TarotOut>('tarot', args, [seed, spread])

  return (
    <Panel icon={<MoonStar />} title={t('tarotTitle.' + spread)} className="min-h-0 flex-1" bodyClassName="min-h-0 overflow-auto space-y-3 p-3" extra={pending && <Spinner className="size-3.5" />}>
      <div className="flex items-center gap-2">
        <SegmentedControl
          size="sm"
          value={spread}
          onValueChange={setSpread}
          aria-label={t('api.tarot.spread')}
          options={[
            { value: 'single', label: t('positions.single') },
            { value: 'three', label: t('tarotTitle.three') },
          ]}
        />
        <Button size="sm" variant="secondary" onClick={() => setSeed(Date.now() % 1_000_000)}>
          {t('tarot.draw')}
        </Button>
      </div>
      {error ? (
        <p className="text-xs text-danger">{errorMessage(error)}</p>
      ) : (
        <div className={`grid gap-3 ${spread === 'three' ? 'grid-cols-3' : 'grid-cols-1'}`}>
          {(result?.cards ?? []).map((card) => (
            <div key={card.position} className="rounded-card border border-border bg-surface-2 p-3">
              <p className="text-[11px] text-fg-subtle">{t(`positions.${card.position}`)}</p>
              <p className="mt-1 font-semibold text-fg">
                {card.nameZh}
                <span className="ml-1.5 text-xs font-normal text-fg-subtle">{card.nameEn}</span>
              </p>
              <Badge variant={card.upright ? 'success' : 'neutral'} className="mt-1.5">
                {card.upright ? t('upright') : t('reversed')}
              </Badge>
              <p className="mt-2 text-xs leading-relaxed text-fg-muted">{card.meaningZh}</p>
            </div>
          ))}
        </div>
      )}
      {result && <p className="text-[11px] text-fg-subtle">{t('seedHint', { seed: result.seed })}</p>}
    </Panel>
  )
}

function DailyTab() {
  const { t, errorMessage } = usePlugin()
  const today = new Date()
  const [dateText, setDateText] = useState(`${today.getFullYear()}${String(today.getMonth() + 1).padStart(2, '0')}${String(today.getDate()).padStart(2, '0')}`) // tf-allow: 默认值
  const date = Number(dateText)
  const args = Number.isInteger(date) && date >= 19700101 && date <= 99991231 ? { date } : null
  const { result, error, pending } = useDebouncedCall<DailyOut>(args ? 'daily' : null, args, [dateText])

  return (
    <Panel icon={<MoonStar />} title={t('tab.daily')} className="min-h-0 flex-1" bodyClassName="space-y-3 p-3" extra={pending && <Spinner className="size-3.5" />}>
      <Input value={dateText} onChange={(event) => setDateText(event.target.value)} placeholder={t('api.daily.date')} aria-label={t('api.daily.date')} />
      {error ? (
        <p className="text-xs text-danger">{errorMessage(error)}</p>
      ) : (
        result && (
          <div className="space-y-2 rounded-card border border-border bg-surface-2 p-4">
            <div className="flex items-center justify-between">
              <span className="font-semibold text-fg">
                {result.card.nameZh}
                <span className="ml-1.5 text-xs font-normal text-fg-subtle">{result.card.nameEn}</span>
              </span>
              <Badge variant={result.level === 'upper' ? 'success' : result.level === 'lower' ? 'danger' : 'neutral'}>
                {t(`levels.${result.level}`)}
              </Badge>
            </div>
            <p className="text-sm text-fg-muted">{result.card.meaningZh}</p>
            <p className="text-[11px] text-fg-subtle">{t('seedHint', { seed: result.seed })}</p>
          </div>
        )
      )}
    </Panel>
  )
}
