import { useCallback, useEffect, useRef, useState } from 'react'
import { Button, Panel, SegmentedControl } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Pause, Play } from 'lucide-react'

interface PresetOut {
  key: string
  count: number
  speed: number
  wind: number
  sizeMin: number
  sizeMax: number
  color: string
  shape: string
  background: string
}
interface NoiseOut {
  method: string
  filterType: string
  filterHz: number
  gain: number
  crackleMs: [number, number] | null
}

const KEYS = ['snow', 'rain', 'fireflies', 'bubbles'] as const
type Key = (typeof KEYS)[number]

interface Particle {
  x: number
  y: number
  size: number
  vy: number
  vx: number
  phase: number
}

export default function AmbientEffects() {
  const { t } = usePlugin()
  const [key, setKey] = useState<Key>('snow')
  const [density, setDensity] = useState(50) // tf-allow: 默认值
  const [wind, setWind] = useState(0) // tf-allow: 默认值
  const [playing, setPlaying] = useState(true)
  const [noiseOn, setNoiseOn] = useState(false)
  const [volume, setVolume] = useState(40) // tf-allow: 默认值
  const preset = useDebouncedCall<PresetOut>('preset', { key, density, lowPower: false }, [key, density])
  const noise = useDebouncedCall<NoiseOut>('noise', { kind: key === 'rain' ? 'rain' : 'campfire', volume }, [key, volume])
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const particlesRef = useRef<Particle[]>([])
  const audioRef = useRef<{ ctx: AudioContext; oscs: AudioScheduledSourceNode[] } | null>(null)

  // 白噪音合成
  useEffect(() => {
    if (!noiseOn || !noise.result) {
      if (audioRef.current) {
        for (const source of audioRef.current.oscs) {
          try { source.stop() } catch { /* stopped */ }
        }
        audioRef.current = null
      }
      return
    }
    const ctx = new AudioContext()
    void ctx.resume()
    const seconds = 2
    const buffer = ctx.createBuffer(1, ctx.sampleRate * seconds, ctx.sampleRate)
    const data = buffer.getChannelData(0)
    for (let index = 0; index < data.length; index++) {
      data[index] = Math.random() * 2 - 1
    }
    const source = ctx.createBufferSource()
    source.buffer = buffer
    source.loop = true
    const filter = ctx.createBiquadFilter()
    filter.type = noise.result.filterType as BiquadFilterType
    filter.frequency.value = noise.result.filterHz
    const gain = ctx.createGain()
    gain.gain.value = noise.result.gain
    source.connect(filter).connect(gain).connect(ctx.destination)
    source.start()
    audioRef.current = { ctx, oscs: [source] }
    return () => {
      source.stop()
      void ctx.close()
    }
  }, [noiseOn, noise.result])

  const spawn = useCallback((count: number, width: number, height: number, data: PresetOut): Particle[] => {
    const range = data.sizeMax - data.sizeMin
    return Array.from({ length: count }, () => ({
      x: Math.random() * width,
      y: Math.random() * height,
      size: data.sizeMin + Math.random() * range,
      vy: (0.4 + Math.random() * 0.8) * data.speed * (data.shape === 'streak' ? 3 : 1),
      vx: (Math.random() - 0.5) * 0.6 * data.speed,
      phase: Math.random() * Math.PI * 2,
    }))
  }, [])

  useEffect(() => {
    const canvas = canvasRef.current
    const data = preset.result
    if (!canvas || !data || !playing) return
    const context = canvas.getContext('2d')
    if (!context) return
    const resize = () => {
      canvas.width = canvas.clientWidth
      canvas.height = canvas.clientHeight
    }
    resize()
    window.addEventListener('resize', resize)
    particlesRef.current = spawn(data.count, canvas.width, canvas.height, data)
    let frame = 0
    const render = () => {
      frame = requestAnimationFrame(render)
      context.clearRect(0, 0, canvas.width, canvas.height)
      for (const particle of particlesRef.current) {
        particle.y += particle.vy
        particle.x += particle.vx + wind * 0.5 * data.speed
        if (data.shape === 'glow' || data.shape === 'bubble') {
          particle.phase += 0.05
          particle.y -= data.shape === 'bubble' ? particle.vy : Math.sin(particle.phase) * 0.3
        }
        // 出界回绕
        if (particle.y > canvas.height + 10) { particle.y = -10; particle.x = Math.random() * canvas.width }
        if (particle.y < -10) { particle.y = canvas.height + 10; particle.x = Math.random() * canvas.width }
        if (particle.x > canvas.width + 10) particle.x = -10
        if (particle.x < -10) particle.x = canvas.width + 10
        context.globalAlpha = data.shape === 'glow' ? 0.5 + Math.sin(particle.phase) * 0.3 : 0.8
        context.fillStyle = data.color
        if (data.shape === 'streak') {
          context.fillRect(particle.x, particle.y, particle.size * 0.4, particle.size * 6)
        } else if (data.shape === 'flake') {
          context.beginPath()
          context.arc(particle.x, particle.y, particle.size / 2, 0, Math.PI * 2)
          context.fill()
        } else if (data.shape === 'glow') {
          context.beginPath()
          context.arc(particle.x, particle.y, particle.size, 0, Math.PI * 2)
          context.fill()
        } else {
          context.beginPath()
          context.arc(particle.x, particle.y, particle.size / 2, 0, Math.PI * 2)
          context.stroke()
        }
      }
      context.globalAlpha = 1
    }
    render()
    return () => {
      cancelAnimationFrame(frame)
      window.removeEventListener('resize', resize)
    }
  }, [preset.result, playing, spawn, wind])

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Panel title={t('presets.snow')} bodyClassName="space-y-3 p-3">
        <div className="flex flex-wrap items-center gap-2">
          <SegmentedControl
            size="sm"
            value={key}
            onValueChange={(value) => setKey(value as Key)}
            aria-label={t('api.preset.key')}
            options={KEYS.map((item) => ({ value: item, label: t(`presets.${item}`) }))}
          />
          <Button size="sm" variant="secondary" onClick={() => setPlaying((prev) => !prev)}>
            {playing ? <Pause className="size-3.5" /> : <Play className="size-3.5" />}
          </Button>
          <label className="flex items-center gap-1.5 text-xs text-fg-muted">
            {t('ui.sound')}
            <input type="checkbox" checked={noiseOn} onChange={(event) => setNoiseOn(event.target.checked)} className="size-3.5" />
            {t(`noise.${key === 'rain' ? 'rain' : 'campfire'}`)}
          </label>
        </div>
        <label className="flex items-center gap-2 text-xs text-fg-muted">
          {t('ui.density')}
          <input type="range" min={0} max={100} value={density} onChange={(event) => setDensity(Number(event.target.value))} aria-label={t('ui.density')} className="min-w-0 flex-1 accent-primary" />
        </label>
        <label className="flex items-center gap-2 text-xs text-fg-muted">
          {t('ui.wind')}
          <input type="range" min={-100} max={100} value={wind} onChange={(event) => setWind(Number(event.target.value) / 100)} aria-label={t('ui.wind')} className="min-w-0 flex-1 accent-primary" />
        </label>
        {noiseOn && (
          <label className="flex items-center gap-2 text-xs text-fg-muted">
            {t('ui.volume')}
            <input type="range" min={0} max={100} value={volume} onChange={(event) => setVolume(Number(event.target.value))} aria-label={t('ui.volume')} className="min-w-0 flex-1 accent-primary" />
          </label>
        )}
      </Panel>
      <div className="relative min-h-0 flex-1 overflow-hidden rounded-card border border-border" style={{ background: preset.result?.background ?? '#0f172a' }}>
        <canvas ref={canvasRef} className="h-full w-full" aria-label={t('presets.snow')} />
      </div>
    </div>
  )
}
