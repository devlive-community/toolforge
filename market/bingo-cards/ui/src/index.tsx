import { useState } from 'react'
import { Badge, Button, Input, Panel, Select, cn } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { ClipboardList, Grid3x3, Undo2 } from 'lucide-react'

interface Card {
  number: number
  cells: string[]
  freeIndex: number | null
}
interface GenerateOut {
  size: number
  cards: Card[]
  itemsPerCard: number
}
interface Line {
  kind: string
  index: number
  cells: number[]
}

const SAMPLE = '苹果, 香蕉, 橙子, 西瓜, 葡萄, 草莓, 蓝莓, 桃子, 柠檬, 菠萝, 芒果, 樱桃' // tf-allow: 示例输入

export default function BingoCards() {
  const { t, call } = usePlugin()
  const [pool, setPool] = useState(SAMPLE) // tf-allow: 默认值
  const [size, setSize] = useState<'3' | '5'>('5')
  const [cardCount, setCardCount] = useState(4) // tf-allow: 默认值
  const [freeCenter, setFreeCenter] = useState(true)
  const [generated, setGenerated] = useState<GenerateOut | null>(null)
  const [busy, setBusy] = useState(false)
  const [called, setCalled] = useState<string[]>([])
  const [history, setHistory] = useState<string[]>([])
  const [lines, setLines] = useState<Line[]>([])
  const [marked, setMarked] = useState<Set<string>>(new Set())

  const items = pool.split(/[,，\n]/).map((item) => item.trim()).filter(Boolean)
  const generate = async () => {
    if (items.length === 0) return
    setBusy(true)
    try {
      const out = await call<GenerateOut>('generate', {
        items, size: Number(size), cards: cardCount, freeCenter: size === '5' && freeCenter, seed: Date.now() % 1_000_000,
      })
      setGenerated(out)
      setCalled([])
      setHistory([])
      setLines([])
      setMarked(new Set())
    } finally {
      setBusy(false)
    }
  }

  const callNext = async () => {
    const remaining = items.filter((item) => !called.includes(item))
    try {
      const out = await call<{ item: string; remaining: string[] }>('draw', { remaining, seed: Date.now() % 1_000_000 })
      setHistory((prev) => [...prev, out.item])
      setCalled((prev) => [...prev, out.item])
    } catch {
      // pool exhausted
    }
  }

  const undoCall = () => {
    setHistory((prev) => {
      const last = prev[prev.length - 1]
      setCalled((current) => current.filter((item) => item !== last))
      return prev.slice(0, -1)
    })
  }

  const checkLines = async () => {
    if (!generated) return
    const out = await call<{ hasLine: boolean; lines: Line[] }>('check', {
      size: generated.size, cells: generated.cards[0].cells, freeCenter: generated.cards[0].freeIndex !== null, called,
    })
    setLines(out.lines)
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(280px,0.9fr)_minmax(0,1.1fr)] gap-3">
      <Panel icon={<ClipboardList />} title={t('api.generate.summary')} bodyClassName="space-y-2.5 p-3">
        <Input value={pool} onChange={(event) => setPool(event.target.value)} placeholder={t('ui.pool')} aria-label={t('ui.pool')} />
        <div className="grid grid-cols-3 items-end gap-2">
          <label className="flex flex-col gap-1 text-xs text-fg-muted">
            {t('ui.size')}
            <Select value={size} onValueChange={setSize} options={[{ value: '3', label: '3×3' }, { value: '5', label: '5×5' }]} aria-label={t('ui.size')} />
          </label>
          <label className="flex flex-col gap-1 text-xs text-fg-muted">
            {t('ui.count')}
            <Input type="number" value={String(cardCount)} min={1} max={50} onChange={(event) => setCardCount(Number(event.target.value))} aria-label={t('ui.count')} />
          </label>
          {size === '5' && (
            <label className="flex items-center gap-1.5 pb-1.5 text-xs text-fg-muted">
              <input type="checkbox" checked={freeCenter} onChange={(event) => setFreeCenter(event.target.checked)} className="size-3.5" />
              {t('ui.free')}
            </label>
          )}
        </div>
        <Button onClick={() => void generate()} disabled={busy || items.length === 0}>
          {t('ui.gen')}
        </Button>
      </Panel>
      <Panel icon={<Grid3x3 />} title={t('ui.called')} className="min-h-0" bodyClassName="min-h-0 overflow-auto space-y-3 p-3">
        {generated ? (
          <>
            <div className="flex flex-wrap items-center gap-2">
              <Button size="sm" onClick={() => void callNext()} disabled={called.length >= items.length}>
                {t('ui.call')}
              </Button>
              <Button size="sm" variant="ghost" onClick={undoCall} disabled={history.length === 0} aria-label={t('ui.undo')}>
                <Undo2 className="size-3.5" />
                {t('ui.undo')}
              </Button>
              <Button size="sm" variant="secondary" onClick={() => void checkLines()}>
                {t('ui.checkLine')}
              </Button>
              <span className="text-xs text-fg-muted">{t('ui.called')}: {called.length}</span>
            </div>
            <div className="flex flex-wrap gap-1.5">
              {history.map((item) => (
                <Badge key={item} variant="primary">{item}</Badge>
              ))}
            </div>
            {lines.length > 0 ? (
              <div className="space-y-1 rounded-control bg-success-soft p-2.5 text-xs text-success">
                {lines.map((line) => (
                  <p key={`${line.kind}-${line.index}`}>{t(`line.${line.kind}`, { index: line.index + 1 })}</p>
                ))}
              </div>
            ) : (
              <p className="text-xs text-fg-subtle">{t('ui.noLine')}</p>
            )}
            <div className="grid grid-cols-2 gap-3">
              {generated.cards.map((card) => (
                <div key={card.number} className="rounded-card border border-border p-2">
                  <p className="mb-1 text-center text-[11px] text-fg-subtle">{t('ui.card')} #{card.number}</p>
                  <div className={cn('grid gap-0.5', generated.size === 3 ? 'grid-cols-3' : 'grid-cols-5')}>
                    {card.cells.map((cell, index) => {
                      const isFree = card.freeIndex === index
                      const hit = isFree || called.includes(cell) || marked.has(cell)
                      return (
                        <button
                          key={index}
                          type="button"
                          onClick={() =>
                            setMarked((prev) => {
                              const next = new Set(prev)
                              if (next.has(cell)) next.delete(cell)
                              else next.add(cell)
                              return next
                            })
                          }
                          className={cn(
                            'flex aspect-square items-center justify-center rounded-control border border-border p-0.5 text-[10px] outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
                            isFree && 'bg-primary-soft font-semibold text-primary-fg',
                            hit && !isFree && 'bg-success-soft text-success',
                          )}
                        >
                          {isFree ? '★' : cell}
                        </button>
                      )
                    })}
                  </div>
                </div>
              ))}
            </div>
          </>
        ) : (
          <p className="py-6 text-center text-xs text-fg-subtle">{t('ui.gen')}</p>
        )}
      </Panel>
    </div>
  )
}
