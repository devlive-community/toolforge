import { useState } from 'react'
import { Input, Panel, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Search, Star } from 'lucide-react'

interface StarPosition {
  key: string
  nameZh: string
  nameEn: string
  constellation: string
  magnitude: number
  azimuth: number
  altitude: number
}
interface SkyOut {
  latitude: number
  longitude: number
  localSiderealTimeHours: number
  stars: StarPosition[]
  lines: [string, string][]
}

export default function StargazingGuide() {
  const { t, errorMessage } = usePlugin()
  const [lat, setLat] = useState(40) // tf-allow: 默认值
  const [lon, setLon] = useState(116.4) // tf-allow: 默认值
  const [search, setSearch] = useState('')
  const [selected, setSelected] = useState<string | null>(null)
  const [timestamp] = useState(() => Date.now())
  const args = { latitude: lat, longitude: lon, timestampMs: timestamp }
  const sky = useDebouncedCall<SkyOut>('sky', args, [lat, lon])

  // 星图投影：alt/az → 简单等距圆盘（天顶为圆心）
  const project = (az: number, alt: number): [number, number] => {
    const radius = (90 - Math.max(alt, 0)) / 90 * 48
    const angle = ((az - 90) * Math.PI) / 180
    return [50 + radius * Math.cos(angle), 50 + radius * Math.sin(angle)]
  }

  const needle = search.trim().toLowerCase()
  const stars = (sky.result?.stars ?? []).filter(
    (star) => !needle || star.nameZh.includes(search.trim()) || star.nameEn.toLowerCase().includes(needle),
  )
  const selectedStar = sky.result?.stars.find((star) => star.key === selected)

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1.2fr)_minmax(280px,0.8fr)] gap-3">
      <Panel
        icon={<Star />}
        title={t('ui.chart')}
        className="min-h-0"
        bodyClassName="flex items-center justify-center p-3"
        extra={sky.pending && <Spinner className="size-3.5" />}
      >
        <svg viewBox="0 0 100 100" className="max-h-full max-w-full rounded-full bg-slate-950" role="img" aria-label={t('ui.chart')}>
          {/* 地平圈与方位 */}
          <circle cx="50" cy="50" r="48" fill="none" stroke="#334155" strokeWidth="0.5" />
          <text x="50" y="5" textAnchor="middle" fill="#94a3b8" fontSize="3">{t('ui.north')}</text>
          <text x="96" y="51" textAnchor="middle" fill="#94a3b8" fontSize="3">{t('ui.east')}</text>
          <text x="50" y="98" textAnchor="middle" fill="#94a3b8" fontSize="3">{t('ui.south')}</text>
          <text x="4" y="51" textAnchor="middle" fill="#94a3b8" fontSize="3">{t('ui.west')}</text>
          {/* 高度圈 */}
          <circle cx="50" cy="50" r="32" fill="none" stroke="#1e293b" strokeWidth="0.3" strokeDasharray="1 1" />
          <circle cx="50" cy="50" r="16" fill="none" stroke="#1e293b" strokeWidth="0.3" strokeDasharray="1 1" />
          {/* 连线 */}
          {(sky.result?.lines ?? []).map(([a, b]) => {
            const starA = sky.result?.stars.find((star) => star.key === a)
            const starB = sky.result?.stars.find((star) => star.key === b)
            if (!starA || !starB) return null
            const [x1, y1] = project(starA.azimuth, starA.altitude)
            const [x2, y2] = project(starB.azimuth, starB.altitude)
            return <line key={`${a}-${b}`} x1={x1} y1={y1} x2={x2} y2={y2} stroke="#475569" strokeWidth="0.3" strokeDasharray="0.8 0.8" />
          })}
          {/* 亮星：半径按星等 */}
          {stars.map((star) => {
            const [x, y] = project(star.azimuth, star.altitude)
            const size = Math.max(0.6, 2.2 - star.magnitude * 0.6)
            return (
              <g key={star.key} onClick={() => setSelected(star.key)} className="cursor-pointer">
                <circle cx={x} cy={y} r={size} fill={star.key === selected ? '#fbbf24' : '#e2e8f0'} />
                {star.magnitude < 1.0 && (
                  <text x={x + 1.2} y={y + 1} fill="#cbd5e1" fontSize="2.2">
                    {star.nameZh}
                  </text>
                )}
              </g>
            )
          })}
        </svg>
      </Panel>
      <div className="flex min-h-0 flex-col gap-3">
        <Panel icon={<Search />} title={t('ui.search')} bodyClassName="space-y-2 p-3">
          <div className="grid grid-cols-2 gap-2">
            <Input type="number" value={String(lat)} onChange={(event) => setLat(Number(event.target.value))} placeholder={t('ui.lat')} aria-label={t('ui.lat')} />
            <Input type="number" value={String(lon)} onChange={(event) => setLon(Number(event.target.value))} placeholder={t('ui.lon')} aria-label={t('ui.lon')} />
          </div>
          <Input value={search} onChange={(event) => setSearch(event.target.value)} leading={<Search />} placeholder={t('ui.search')} aria-label={t('ui.search')} />
          {sky.error ? (
            <p className="text-xs text-danger">{errorMessage(sky.error)}</p>
          ) : (
            sky.result && <p className="text-xs text-fg-muted">{t('ui.above', { count: sky.result.stars.length })}</p>
          )}
          <p className="text-[11px] text-fg-subtle">{t('ui.lineNote')}</p>
        </Panel>
        <Panel icon={<Star />} title={selectedStar ? selectedStar.nameZh : t('ui.chart')} className="min-h-0 flex-1" bodyClassName="min-h-0 overflow-auto space-y-1 p-3">
          {selectedStar ? (
            <>
              <p className="text-sm font-semibold text-fg">
                {selectedStar.nameZh}
                <span className="ml-1.5 text-xs font-normal text-fg-subtle">{selectedStar.nameEn}</span>
              </p>
              <p className="text-xs text-fg-muted">{t('ui.mag')}: {selectedStar.magnitude}</p>
              <p className="text-xs text-fg-muted">{t('ui.az')}{' '}{selectedStar.azimuth}{'° · '}{t('ui.alt')}{' '}{selectedStar.altitude}{'°'}</p>
            </>
          ) : (
            stars.map((star) => (
              <button
                key={star.key}
                type="button"
                onClick={() => setSelected(star.key)}
                className="flex w-full items-center justify-between border-t border-border px-1 py-1.5 text-left text-xs outline-none transition-colors first:border-t-0 hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring"
              >
                <span className="font-medium text-fg">{star.nameZh}</span>
                <span className="text-fg-subtle">{t('ui.alt')}{' '}{star.altitude}{'° · '}{t('ui.az')}{' '}{star.azimuth}{'°'}</span>
              </button>
            ))
          )}
        </Panel>
      </div>
    </div>
  )
}
