import { useCallback, useEffect, useRef, useState } from 'react'
import { Badge, Button, Panel, SegmentedControl, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Music, Star, VolumeX } from 'lucide-react'

interface Key {
  midi: number
  hz: number
  name: string
  solfege: string
  black: boolean
}
interface KeyboardOut {
  keys: Key[]
  startMidi: number
}
interface ChordNote {
  midi: number
  hz: number
  name: string
  role: string
  atMs: number
}
interface ChordOut {
  kind: string
  name: string
  notes: ChordNote[]
}

type Kind = 'major' | 'minor' | 'dom7'
const KINDS: Kind[] = ['major', 'minor', 'dom7']

export default function MiniInstrument() {
  const { t } = usePlugin()
  const [octave, setOctave] = useState(0)
  const startMidi = 60 + octave * 12
  const keyboard = useDebouncedCall<KeyboardOut>('keyboard', { startMidi }, [startMidi])
  const [kind, setKind] = useState<Kind>('major')
  const [arpeggio, setArpeggio] = useState(false)
  const [sustain, setSustain] = useState(true)
  const [volume, setVolume] = useState(50) // tf-allow: 默认值
  const [showSolfege, setShowSolfege] = useState(false)
  const [active, setActive] = useState<number[]>([])
  const [favorites, saveFavorites] = usePluginState<{ midi: number; kind: Kind }[]>('favorites', [])
  const audioRef = useRef<AudioContext | null>(null)
  const oscillators = useRef<Map<number, { osc: OscillatorNode; gain: GainNode }>>(new Map())

  const ctx = useCallback(() => {
    if (!audioRef.current) audioRef.current = new AudioContext()
    void audioRef.current.resume()
    return audioRef.current
  }, [])

  const tone = useCallback((hz: number, atSec: number, hold: boolean) => {
    const context = ctx()
    const osc = context.createOscillator()
    const gain = context.createGain()
    osc.type = 'triangle'
    osc.frequency.value = hz
    const peak = (volume / 100) * 0.35
    gain.gain.setValueAtTime(0.0001, atSec)
    gain.gain.exponentialRampToValueAtTime(peak, atSec + 0.02)
    if (!hold) {
      gain.gain.exponentialRampToValueAtTime(0.0001, atSec + 0.6)
      osc.start(atSec)
      osc.stop(atSec + 0.65)
    } else {
      osc.start(atSec)
    }
    osc.connect(gain).connect(context.destination)
    return { osc, gain }
  }, [ctx, volume])

  const stopAll = useCallback(() => {
    for (const [, voice] of oscillators.current) {
      try {
        voice.gain.gain.exponentialRampToValueAtTime(0.0001, audioRef.current!.currentTime + 0.05)
        voice.osc.stop(audioRef.current!.currentTime + 0.08)
      } catch { /* already stopped */ }
    }
    oscillators.current.clear()
    setActive([])
  }, [])

  const press = (key: Key) => {
    if (oscillators.current.has(key.midi)) return
    const voice = tone(key.hz, ctx().currentTime, sustain)
    oscillators.current.set(key.midi, voice)
    setActive((prev) => [...prev, key.midi])
  }

  const release = (midi: number) => {
    const voice = oscillators.current.get(midi)
    if (voice && !sustain) {
      const now = ctx().currentTime
      voice.gain.gain.exponentialRampToValueAtTime(0.0001, now + 0.05)
      voice.osc.stop(now + 0.08)
      oscillators.current.delete(midi)
    }
    setActive((prev) => prev.filter((m) => m !== midi))
  }

  const playChord = (chord: ChordOut) => {
    const now = ctx().currentTime + 0.02
    for (const note of chord.notes) {
      tone(note.hz, now + note.atMs / 1000, false)
    }
  }

  const chord = useDebouncedCall<ChordOut>('chord', { rootMidi: startMidi, kind, arpeggioMs: arpeggio ? 150 : 0 }, [startMidi, kind, arpeggio])

  // 窗口失焦停止全部持续音
  useEffect(() => {
    const onBlur = () => stopAll()
    window.addEventListener('blur', onBlur)
    return () => {
      window.removeEventListener('blur', onBlur)
      stopAll()
    }
  }, [stopAll])

  const keys = keyboard.result?.keys ?? []
  const toggleFavorite = (midi: number, chordKind: Kind) => {
    const next = [...(favorites ?? []), { midi, kind: chordKind }]
    saveFavorites(next.slice(-6))
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 overflow-auto">
      <Panel icon={<Music />} title={t('ui.octave')} bodyClassName="space-y-3 p-3">
        <div className="flex flex-wrap items-center gap-2">
          <SegmentedControl
            size="sm"
            value={kind}
            onValueChange={(value) => setKind(value as Kind)}
            aria-label={t('api.chord.kind')}
            options={KINDS.map((key) => ({ value: key, label: t(`chords.${key}`) }))}
          />
          <Button size="sm" variant="secondary" onClick={() => setOctave((prev) => Math.max(-2, prev - 1))} disabled={startMidi <= 36}>
            {t('ui.down')}
          </Button>
          <Button size="sm" variant="secondary" onClick={() => setOctave((prev) => Math.min(2, prev + 1))} disabled={startMidi >= 84}>
            {t('ui.up')}
          </Button>
          <Button size="sm" variant={arpeggio ? 'primary' : 'secondary'} onClick={() => setArpeggio((prev) => !prev)}>
            {t('ui.arpeggio')}
          </Button>
          <Button size="sm" variant="ghost" onClick={stopAll} aria-label={t('ui.stopAll')}>
            <VolumeX className="size-3.5" />
            {t('ui.stopAll')}
          </Button>
        </div>
        {chord.result && (
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="primary">{chord.result.name}</Badge>
            {chord.result.notes.map((note) => (
              <Badge key={note.midi} variant="neutral">
                {note.name} <span className="text-fg-subtle">{t(`roles.${note.role}`)}</span>
              </Badge>
            ))}
            <Button size="sm" variant="secondary" onClick={() => chord.result && playChord(chord.result)}>
              {t('api.chord.summary')}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => toggleFavorite(startMidi, kind)} aria-label={t('ui.favorites')}>
              <Star className="size-3.5" />
            </Button>
          </div>
        )}
        {(favorites ?? []).length > 0 && (
          <div className="flex flex-wrap gap-1.5">
            {(favorites ?? []).map((favorite, index) => (
              <button
                key={`${favorite.midi}-${favorite.kind}-${index}`}
                type="button"
                onClick={() => { setKind(favorite.kind); setOctave(0) }}
                className="rounded-control border border-border px-2 py-0.5 font-mono text-[11px] text-fg-muted outline-none transition-colors hover:bg-hover hover:text-fg focus-visible:ring-2 focus-visible:ring-ring"
              >
                {favorite.kind} @{favorite.midi}
              </button>
            ))}
          </div>
        )}
        <div className="flex items-center gap-3 text-xs text-fg-muted">
          <label className="flex items-center gap-1.5">
            {t('ui.volume')}
            <input type="range" min={0} max={100} value={volume} onChange={(event) => setVolume(Number(event.target.value))} aria-label={t('ui.volume')} className="w-28 accent-primary" />
          </label>
          <label className="flex items-center gap-1.5">
            <input type="checkbox" checked={sustain} onChange={(event) => setSustain(event.target.checked)} className="size-3.5" />
            {t('ui.sustain')}
          </label>
          <label className="flex items-center gap-1.5">
            <input type="checkbox" checked={showSolfege} onChange={(event) => setShowSolfege(event.target.checked)} className="size-3.5" />
            {t('ui.solfege')}
          </label>
        </div>
      </Panel>
      <Panel icon={<Music />} title={t('ui.name')} className="min-h-0" bodyClassName="p-3" extra={keyboard.pending && <Spinner className="size-3.5" />}>
        <div className="relative h-40 select-none" role="list" aria-label={t('ui.name')}>
          <div className="flex h-full gap-0.5">
            {keys.filter((key) => !key.black).map((key) => (
              <button
                key={key.midi}
                type="button"
                role="listitem"
                onMouseDown={() => press(key)}
                onMouseUp={() => release(key.midi)}
                onMouseLeave={() => release(key.midi)}
                onTouchStart={() => press(key)}
                onTouchEnd={() => release(key.midi)}
                aria-label={key.name}
                className={`flex h-full flex-1 items-end justify-center rounded-b-control border border-border bg-surface pb-1.5 outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring ${
                  active.includes(key.midi) ? 'bg-primary-soft' : 'hover:bg-hover'
                }`}
              >
                <span className="text-[10px] text-fg-subtle">{showSolfege ? key.solfege : key.name}</span>
              </button>
            ))}
          </div>
          {/* 黑键覆盖层 */}
          <div className="pointer-events-none absolute inset-0">
            <div className="relative h-full">
              {keys.filter((key) => key.black).map((key) => {
                // 黑键按其前一个白键的位置定位
                const whiteBefore = keys.filter((k) => !k.black && k.midi < key.midi).length
                const width = 100 / 14
                return (
                  <button
                    key={key.midi}
                    type="button"
                    onMouseDown={() => press(key)}
                    onMouseUp={() => release(key.midi)}
                    onMouseLeave={() => release(key.midi)}
                    aria-label={key.name}
                    className={`pointer-events-auto absolute top-0 h-[62%] rounded-b-control border border-border bg-slate-900 outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring ${
                      active.includes(key.midi) ? 'bg-primary' : ''
                    }`}
                    style={{ left: `calc(${(whiteBefore * (100 / 14)) - width / 2.4}%)`, width: `${width}%` }}
                  />
                )
              })}
            </div>
          </div>
        </div>
        <p className="mt-2 text-[11px] text-fg-subtle">{t('ui.hint')}</p>
      </Panel>
    </div>
  )
}
