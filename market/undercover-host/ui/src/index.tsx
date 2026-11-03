import { useState } from 'react'
import { Badge, Button, Input, Panel, SegmentedControl, cn } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Eye, EyeOff, Users, Vote } from 'lucide-react'

interface PlayerWord {
  seat: number
  name: string
  role: string
  word: string
}
interface DealOut {
  players: PlayerWord[]
  category: string
  pair: [string, string]
}
interface Count {
  seat: number
  votes: number
}
interface TallyOut {
  status: string
  eliminated: number | null
  runoffCandidates: number[]
  counts: Count[]
  alive: number[]
}

type Phase = 'setup' | 'peek' | 'talk' | 'vote' | 'end'

const CATEGORIES = ['random', 'food', 'life', 'place', 'work', 'tech', 'sport']

export default function UndercoverHost() {
  const { t, call } = usePlugin()
  const [names, setNames] = useState('小明, 小红, 小刚, 小丽') // tf-allow: 默认值
  const [category, setCategory] = useState('random')
  const [deal, setDeal] = useState<DealOut | null>(null)
  const [phase, setPhase] = useState<Phase>('setup')
  const [round, setRound] = useState(1)
  const [peekSeat, setPeekSeat] = useState(0)
  const [revealed, setRevealed] = useState(false)
  const [votes, setVotes] = useState<(number | null)[]>([])
  const [voteSeat, setVoteSeat] = useState(0)
  const [alive, setAlive] = useState<number[]>([])
  const [runoff, setRunoff] = useState(false)
  const [runoffCandidates, setRunoffCandidates] = useState<number[]>([])
  const [tally, setTally] = useState<TallyOut | null>(null)
  const [winner, setWinner] = useState<string | null>(null)

  const nameList = names.split(/[,，]/).map((name) => name.trim()).filter(Boolean)

  const startGame = async () => {
    if (nameList.length < 4) return
    try {
      const out = await call<DealOut>('deal', { players: nameList, category: category === 'random' ? undefined : category, seed: Date.now() % 1_000_000 })
      setDeal(out)
      setAlive(out.players.map((p) => p.seat))
      setPhase('peek')
      setPeekSeat(0)
      setRevealed(false)
      setRound(1)
      setVotes([])
      setVoteSeat(0)
      setTally(null)
      setWinner(null)
      setRunoff(false)
      setRunoffCandidates([])
    } catch {
      // errors surfaced by errorMessage below
    }
  }

  const nextPeek = () => {
    setRevealed(false)
    if (deal && peekSeat < deal.players.length - 1) {
      setPeekSeat((prev) => prev + 1)
    } else {
      setPhase('talk')
    }
  }

  const doTally = async (isRunoff: boolean, candidates: number[]) => {
    if (!deal) return
    try {
      const out = await call<TallyOut>('tally', { alive, votes, runoff: isRunoff, runoffCandidates: candidates, undercover: deal.players.find((p) => p.role === 'undercover')!.seat })
      setTally(out)
      if (out.status === 'eliminated') {
        setAlive(out.alive)
        setRound((prev) => prev + 1)
        setVotes([])
        setVoteSeat(0)
        setPhase('talk')
      } else if (out.status === 'tie') {
        setRunoff(true)
        setRunoffCandidates(out.runoffCandidates)
        setVotes([])
        setVoteSeat(0)
        setPhase('vote')
      } else if (out.status === 'runoff_stalemate') {
        setRunoff(false)
        setRound((prev) => prev + 1)
        setVotes([])
        setVoteSeat(0)
        setPhase('talk')
      } else {
        setWinner(out.status)
        setPhase('end')
      }
    } catch {
      // invalid votes surfaced below
    }
  }

  const castVote = (target: number | null) => {
    setVotes((prev) => {
      const next = [...prev]
      next[voteSeat] = target
      return next
    })
    setVoteSeat((prev) => {
      const nextSeat = prev + 1
      // 最后一位投票后自动计票
      if (nextSeat >= alive.length) {
        setTimeout(() => void doTally(runoff, runoffCandidates), 50)
      }
      return nextSeat
    })
  }

  const seatName = (seat: number) => deal?.players[seat]?.name ?? `#${seat}`

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 overflow-auto">
      <Panel icon={<Users />} title={t('ui.players')} bodyClassName="space-y-2.5 p-3">
        <Input value={names} onChange={(event) => setNames(event.target.value)} placeholder={t('ui.players')} aria-label={t('ui.players')} />
        <SegmentedControl
          size="sm"
          value={category}
          onValueChange={setCategory}
          aria-label={t('ui.category')}
          options={CATEGORIES.map((key) => ({ value: key, label: t(`ui.categories.${key}`) }))}
        />
        <Button onClick={() => void startGame()} disabled={phase !== 'setup' && phase !== 'end' ? true : nameList.length < 4}>
          {t('ui.deal')}
        </Button>
        </Panel>

      {deal && phase === 'peek' && (
        <Panel icon={<Eye />} title={t('ui.show')} bodyClassName="space-y-3 p-3">
          <p className="text-sm text-fg">
            {deal.players[peekSeat].name}
            <span className="ml-2 text-xs text-fg-subtle">{t('ui.viewed')}: {peekSeat}/{deal.players.length}</span>
          </p>
          <button
            type="button"
            onMouseDown={() => setRevealed(true)}
            onMouseUp={() => setRevealed(false)}
            onTouchStart={() => setRevealed(true)}
            onTouchEnd={() => setRevealed(false)}
            className={cn(
              'flex h-24 w-full items-center justify-center rounded-card border-2 border-dashed text-lg outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
              revealed ? 'border-primary bg-primary-soft font-semibold text-fg' : 'border-border bg-surface text-fg-subtle',
            )}
          >
            {revealed ? (
              deal.players[peekSeat].word
            ) : (
              <>
                <Eye className="mr-2 size-4" />
                {t('ui.show')}
              </>
            )}
          </button>
          <Button variant="secondary" onClick={nextPeek}>
            {t('ui.next')}
          </Button>
        </Panel>
      )}

      {deal && (phase === 'talk' || phase === 'vote') && (
        <Panel icon={<Vote />} title={`${t('ui.round', { round })} · ${phase === 'vote' ? t('ui.vote') : t('ui.allViewed')}`} bodyClassName="space-y-2.5 p-3">
          <div className="flex flex-wrap gap-1.5">
            {alive.map((seat) => (
              <Badge key={seat} variant={phase === 'talk' && seat === alive[0] ? 'primary' : 'neutral'}>
                {seatName(seat)}
              </Badge>
            ))}
          </div>
          {runoff && <p className="text-xs text-warning">{t('status.tie')}</p>}
          {phase === 'vote' ? (
            <>
              {voteSeat < alive.length ? (
                <>
                  <p className="text-sm font-medium text-fg">{seatName(alive[voteSeat])}{' →'}</p>
                  <div className="flex flex-wrap gap-1.5">
                    {alive
                      .filter((seat) => !runoff || runoffCandidates.includes(seat))
                      .map((seat) => (
                        <Button key={seat} size="sm" variant="secondary" onClick={() => castVote(seat)} disabled={!runoff && seat === alive[voteSeat]}>
                          {seatName(seat)}
                        </Button>
                      ))}
                    <Button size="sm" variant="ghost" onClick={() => castVote(null)}>
                      {t('ui.abstain')}
                    </Button>
                  </div>
                </>
              ) : (
                <p className="flex items-center gap-2 text-sm text-fg-muted">
                  <Vote className="size-4" />
                  {t('ui.vote')}{'…'}
                </p>
              )}
            </>
          ) : (
            <Button size="sm" onClick={() => { setVotes([]); setVoteSeat(0); setPhase('vote') }}>
              <Vote className="size-3.5" />
              {t('ui.vote')}
            </Button>
          )}
        </Panel>
      )}

      {tally && phase !== 'end' && (
        <Panel icon={<Vote />} title={t(`status.${tally.status}`)} bodyClassName="space-y-1.5 p-3">
          {tally.counts.map((count) => (
            <p key={count.seat} className="text-xs text-fg-muted">
              {seatName(count.seat)}: {count.votes}
            </p>
          ))}
        </Panel>
      )}

      {phase === 'end' && deal && (
        <Panel icon={<EyeOff />} title={t('ui.reveal')} bodyClassName="space-y-2.5 p-3">
          <Badge variant={winner === 'victory_civilian' ? 'success' : 'danger'}>{t(`status.${winner}`)}</Badge>
          <p className="text-sm text-fg">
            {t('roles.undercover')}{' '}{deal.players.find((p) => p.role === 'undercover')?.name}{' · '}{deal.pair[1]}
          </p>
          <p className="text-xs text-fg-muted">{t('roles.civilian')}: {deal.pair[0]}</p>
          <Button size="sm" variant="secondary" onClick={() => { setPhase('setup'); setDeal(null); setTally(null) }}>
            {t('ui.newGame')}
          </Button>
        </Panel>
      )}
    </div>
  )
}
