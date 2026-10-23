import { useEffect, useEffectEvent, useMemo, useState, type ReactNode } from 'react'
import { Badge, Button, Empty, Input, Panel, Spinner, Tooltip, cn, toast } from '@toolforge/ui'
import { CopyButton, ValueRow, formatBytes, host, useCopy, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Aperture, Check, Eraser, FileImage, FolderSearch, ImagePlus, MapPin, Palette, Search, ShieldCheck, Tags, TriangleAlert, Upload } from 'lucide-react'
import type { Report, StripReport, Summary, Swatch } from './types'

const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif', 'tif', 'tiff', 'ico']
const SUMMARY_KEYS: (keyof Omit<Summary, 'gps' | 'orientation'>)[] = [
  'make',
  'model',
  'lens',
  'takenAt',
  'exposure',
  'aperture',
  'iso',
  'focalLength',
  'flash',
  'software',
]
const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path
const isImage = (path: string) => IMAGE_EXTENSIONS.includes(path.split('.').pop()?.toLowerCase() ?? '')
const mapUrl = (lat: number, lon: number) => `https://www.openstreetmap.org/?mlat=${lat}&mlon=${lon}#map=15/${lat}/${lon}` // tf-allow: 地图服务地址

/** 透明区域显示为棋盘格 */
const CHECKERBOARD =
  'bg-[conic-gradient(var(--color-border)_25%,var(--color-surface)_0_50%,var(--color-border)_0_75%,var(--color-surface)_0)] bg-[length:14px_14px]'

export function ImageInfo() {
  const { t, call, errorMessage } = usePlugin()
  const [report, setReport] = useState<Report | null>(null)
  const [loading, setLoading] = useState(false)
  const [hovering, setHovering] = useState(false)

  const open = async (path: string) => {
    setLoading(true)
    try {
      setReport(await call<Report>('inspect', { path }))
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setLoading(false)
    }
  }

  const onDrop = useEffectEvent((paths: string[]) => {
    const path = paths.find(isImage)
    if (path) open(path)
  })

  useEffect(() => {
    const off = host.onFileDrop({ drop: onDrop, over: setHovering })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])

  const pick = async () => {
    try {
      const path = await host.dialog.openFile([{ name: t('open.filter'), extensions: IMAGE_EXTENSIONS }])
      if (path) await open(path)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  if (!report) {
    return (
      <button
        type="button"
        onClick={pick}
        disabled={loading}
        className={cn(
          'flex h-full w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
          'focus-visible:ring-2 focus-visible:ring-ring',
          hovering ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
        )}
      >
        {loading ? <Spinner className="size-6" /> : <Upload className="size-6" />}
        <span className="text-[13px] font-medium">{loading ? t('open.loading') : t('open.drop')}</span>
        <span className="text-xs text-fg-subtle">{t('open.hint')}</span>
      </button>
    )
  }

  return (
    <div className={cn('grid h-full min-h-0 grid-cols-[minmax(280px,0.9fr)_minmax(0,1.1fr)] gap-3 rounded-card', hovering && 'ring-2 ring-primary')}>
      <div className="flex min-h-0 flex-col gap-3 overflow-auto">
        <Panel
          icon={<FileImage />}
          title={report.name}
          actions={
            <Button size="sm" onClick={pick} disabled={loading}>
              {loading ? <Spinner /> : <ImagePlus />}
              {t('open.another')}
            </Button>
          }
          className="shrink-0"
          bodyClassName="p-0"
        >
          <div className={cn('flex h-64 items-center justify-center p-3', report.hasAlpha ? CHECKERBOARD : 'bg-surface-2')}>
            {report.preview && <img src={report.preview} alt={report.name} className="max-h-full max-w-full object-contain" />}
          </div>
        </Panel>
        <PalettePanel palette={report.palette} average={report.average} />
        <FilePanel report={report} />
      </div>

      <div className="flex min-h-0 flex-col gap-3 overflow-auto">
        <CameraPanel summary={report.summary} />
        {report.summary?.gps && <GpsPanel summary={report.summary} />}
        <StripPanel report={report} />
        {report.exif.length > 0 && <TagsPanel report={report} />}
      </div>
    </div>
  )
}

function Row({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <div className="flex items-baseline gap-3 px-3 py-1.5">
      <span className="w-24 shrink-0 text-xs font-semibold text-fg-muted">{label}</span>
      <span className="min-w-0 flex-1 text-[13px] break-words text-fg tabular-nums" data-selectable>
        {children}
      </span>
    </div>
  )
}

function FilePanel({ report }: { report: Report }) {
  const { t, errorMessage } = usePlugin()
  const reveal = async () => {
    try {
      await host.revealPath(report.path)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const megapixels = ((report.width * report.height) / 1e6).toFixed(1)

  return (
    <Panel
      icon={<FileImage />}
      title={t('file.title')}
      actions={
        <Tooltip content={t('file.reveal')}>
          <Button size="icon-sm" variant="ghost" aria-label={t('file.reveal')} onClick={reveal}>
            <FolderSearch />
          </Button>
        </Tooltip>
      }
      className="shrink-0"
      bodyClassName="py-1.5"
    >
      <Row label={t('file.format')}>{report.format ?? '-'}</Row>
      <Row label={t('file.dimensions')}>{`${report.width} × ${report.height}`}</Row>
      <Row label={t('file.aspect')}>{report.aspect.join(':')}</Row>
      <Row label={t('file.megapixels')}>{t('file.megapixelsValue', { value: megapixels })}</Row>
      <Row label={t('file.size')}>{formatBytes(report.size)}</Row>
      <Row label={t('file.color')}>{report.colorType}</Row>
      <Row label={t('file.depth')}>{report.bitsPerPixel}</Row>
      <Row label={t('file.alpha')}>{report.hasAlpha ? t('file.yes') : t('file.no')}</Row>
      <ValueRow label={t('file.path')} value={report.path} labelWidth="w-24" />
    </Panel>
  )
}

function SwatchButton({ swatch, label }: { swatch: Swatch; label: ReactNode }) {
  const { t } = usePlugin()
  const { copy, copied } = useCopy()
  return (
    <Tooltip content={t('palette.copy', { hex: swatch.hex })}>
      <button
        type="button"
        onClick={() => copy(swatch.hex)}
        className="group flex min-w-0 flex-col overflow-hidden rounded-control border border-border text-left outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <span className="flex h-12 items-center justify-center" style={{ backgroundColor: swatch.hex }}>
          {copied && <Check className="size-4 rounded-full bg-surface p-0.5 text-success" />}
        </span>
        <span className="px-2 py-1 font-mono text-[11px] text-fg group-hover:bg-hover">{swatch.hex}</span>
        <span className="px-2 pb-1 text-[11px] text-fg-subtle tabular-nums group-hover:bg-hover">{label}</span>
      </button>
    </Tooltip>
  )
}

function PalettePanel({ palette, average }: { palette: Swatch[]; average: string | null }) {
  const { t } = usePlugin()
  return (
    <Panel icon={<Palette />} title={t('palette.title')} className="shrink-0" bodyClassName="p-3">
      {palette.length === 0 ? (
        <p className="text-xs text-fg-muted">{t('palette.empty')}</p>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(84px,1fr))] gap-2">
          {palette.map((swatch) => (
            <SwatchButton key={swatch.hex} swatch={swatch} label={`${swatch.percent}%`} />
          ))}
          {average && <SwatchButton swatch={{ hex: average, percent: 100 }} label={t('palette.average')} />}
        </div>
      )}
    </Panel>
  )
}

function CameraPanel({ summary }: { summary: Summary | null }) {
  const { t } = usePlugin()
  const rows = summary ? SUMMARY_KEYS.filter((key) => summary[key]) : []
  return (
    <Panel icon={<Aperture />} title={t('camera.title')} className="shrink-0" bodyClassName={rows.length ? 'py-1.5' : 'p-3'}>
      {!summary || (rows.length === 0 && summary.orientation == null) ? (
        <p className="text-xs text-fg-muted">{t('camera.none')}</p>
      ) : (
        <>
          {rows.map((key) => (
            <Row key={key} label={t(`camera.${key}`)}>
              {summary[key]}
            </Row>
          ))}
          {summary.orientation != null && (
            <Row label={t('camera.orientation')}>{t(`camera.orientations.${summary.orientation}`, { defaultValue: String(summary.orientation) })}</Row>
          )}
        </>
      )}
    </Panel>
  )
}

function GpsPanel({ summary }: { summary: Summary }) {
  const { t, errorMessage } = usePlugin()
  const gps = summary.gps!
  const coordinates = `${gps.latitude}, ${gps.longitude}`
  const openMap = async () => {
    try {
      await host.openUrl(mapUrl(gps.latitude, gps.longitude))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  return (
    <Panel
      icon={<MapPin />}
      title={t('gps.title')}
      actions={
        <>
          <CopyButton text={coordinates} />
          <Button size="sm" onClick={openMap}>
            <MapPin />
            {t('gps.map')}
          </Button>
        </>
      }
      className="shrink-0"
      bodyClassName="py-1.5"
    >
      <p className="mx-3 mb-1.5 flex items-start gap-1.5 rounded-control bg-warning-soft px-2.5 py-2 text-xs text-warning">
        <TriangleAlert className="mt-px size-3.5 shrink-0" />
        {t('gps.warning')}
      </p>
      <Row label={t('gps.latitude')}>{gps.latitude}</Row>
      <Row label={t('gps.longitude')}>{gps.longitude}</Row>
      {gps.altitude != null && <Row label={t('gps.altitude')}>{t('gps.altitudeValue', { value: gps.altitude })}</Row>}
    </Panel>
  )
}

function StripPanel({ report }: { report: Report }) {
  const { t, call, errorMessage } = usePlugin()
  const [busy, setBusy] = useState(false)
  const [result, setResult] = useState<StripReport | null>(null)

  const strip = async () => {
    setBusy(true)
    try {
      const next = await call<StripReport>('strip_metadata', { path: report.path })
      setResult(next)
      toast.success(t('strip.done', { name: baseName(next.output) }))
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setBusy(false)
    }
  }
  const reveal = async (path: string) => {
    try {
      await host.revealPath(path)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  return (
    <Panel icon={<ShieldCheck />} title={t('strip.title')} className="shrink-0" bodyClassName="space-y-2.5 p-3">
      {report.canStrip ? (
        <>
          <div className="flex items-center gap-3">
            <Button variant="primary" onClick={strip} disabled={busy}>
              {busy ? <Spinner /> : <Eraser />}
              {t('strip.action')}
            </Button>
            <p className="min-w-0 flex-1 text-xs text-fg-muted">{t('strip.hint')}</p>
          </div>
          {result && (
            <div className="space-y-1.5 rounded-control border border-border p-2.5">
              <div className="flex items-center gap-2">
                <Check className="size-4 shrink-0 text-success" />
                <p className="min-w-0 flex-1 truncate text-[13px] font-medium text-fg" title={result.output}>
                  {baseName(result.output)}
                </p>
                {result.savedBytes > 0 && <Badge variant="success">{t('strip.saved', { size: formatBytes(result.savedBytes) })}</Badge>}
                <Tooltip content={t('file.reveal')}>
                  <Button size="icon-sm" variant="ghost" aria-label={t('file.reveal')} onClick={() => reveal(result.output)}>
                    <FolderSearch />
                  </Button>
                </Tooltip>
              </div>
              <p className="text-xs text-fg-muted">
                {result.removed.length > 0 ? t('strip.removed', { list: result.removed.join(', ') }) : t('strip.nothing')}
              </p>
              {result.orientationLost && <p className="text-xs text-warning">{t('strip.orientation')}</p>}
            </div>
          )}
        </>
      ) : (
        <p className="text-xs text-fg-muted">{t('strip.unsupported')}</p>
      )}
    </Panel>
  )
}

function TagsPanel({ report }: { report: Report }) {
  const { t } = usePlugin()
  const [query, setQuery] = useState('')
  const entries = useMemo(() => {
    const needle = query.trim().toLowerCase()
    return needle ? report.exif.filter((e) => `${e.tag} ${e.value}`.toLowerCase().includes(needle)) : report.exif
  }, [report.exif, query])

  return (
    <Panel
      icon={<Tags />}
      title={t('exif.title')}
      extra={<Badge>{report.exif.length}</Badge>}
      actions={
        <Input
          size="sm"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t('exif.filter')}
          aria-label={t('exif.filter')}
          leading={<Search />}
          className="w-44"
        />
      }
      className="shrink-0"
      bodyClassName="py-1.5"
    >
      {entries.length === 0 ? (
        <Empty title={t('exif.noMatch')} className="p-4" />
      ) : (
        <table className="w-full text-left text-[12.5px]">
          <thead>
            <tr className="text-xs text-fg-muted">
              <th className="px-3 py-1.5 font-semibold">{t('exif.tag')}</th>
              <th className="px-3 py-1.5 font-semibold">{t('exif.value')}</th>
              <th className="px-3 py-1.5 font-semibold">{t('exif.ifd')}</th>
            </tr>
          </thead>
          <tbody>
            {entries.map((entry, index) => (
              <tr key={`${entry.ifd}-${entry.tag}-${index}`} className="border-t border-border hover:bg-hover">
                <td className="px-3 py-1.5 align-top font-medium text-fg">{entry.tag}</td>
                <td className="px-3 py-1.5 align-top font-mono break-all text-fg" data-selectable>
                  {entry.value}
                </td>
                <td className="px-3 py-1.5 align-top text-fg-subtle">{entry.ifd}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Panel>
  )
}
