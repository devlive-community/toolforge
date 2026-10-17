import { Badge, Button, Empty, Panel, SegmentedControl, Spinner, cn } from '@toolforge/ui'
import { host, usePlugin } from '@toolforge/plugin-ui-sdk'
import { FileCode2, FolderPlus, Network, RefreshCw, Server, X } from 'lucide-react'
import type { Loaded, Method, Settings, SourceKind } from './types'

const baseName = (path: string) => path.split(/[\\/]/).pop() || path

function MethodBadge({ method }: { method: Method }) {
  const { t } = usePlugin()
  if (method.clientStreaming) {
    return <Badge variant="neutral">{method.serverStreaming ? t('kind.bidi') : t('kind.client')}</Badge>
  }
  return <Badge variant={method.serverStreaming ? 'info' : 'primary'}>{method.serverStreaming ? t('kind.server') : t('kind.unary')}</Badge>
}

/** 定义来源与「服务 → 方法」目录 */
export function SourcePanel({
  settings,
  onSettings,
  loaded,
  loading,
  error,
  onLoad,
  selected,
  onSelect,
}: {
  settings: Settings
  onSettings: (patch: Partial<Settings>) => void
  loaded: Loaded | null
  loading: boolean
  error: string | null
  onLoad: (kind: SourceKind) => void
  selected: string
  onSelect: (method: Method) => void
}) {
  const { t } = usePlugin()
  const source = settings.source
  const setSource = (patch: Partial<Settings['source']>) => onSettings({ source: { ...source, ...patch } })

  const pickProto = async () => {
    const path = await host.dialog.openFile([{ name: t('source.protoFilter'), extensions: ['proto'] }])
    if (path) setSource({ proto: path })
  }
  const pickInclude = async () => {
    const dir = await host.dialog.openDirectory()
    if (dir && !source.includes.includes(dir)) setSource({ includes: [...source.includes, dir] })
  }
  const pickDescriptor = async () => {
    const path = await host.dialog.openFile([{ name: t('source.descriptorFilter'), extensions: ['pb', 'protoset', 'desc', 'bin'] }])
    if (path) setSource({ descriptor: path })
  }
  const canLoad =
    source.kind === 'proto' ? !!source.proto : source.kind === 'descriptor' ? !!source.descriptor : !!settings.target.address.trim()

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Panel title={t('source.title')} icon={<FileCode2 />} bodyClassName="space-y-3 p-3">
        <SegmentedControl<SourceKind>
          size="sm"
          value={source.kind}
          onValueChange={(kind) => setSource({ kind })}
          options={[
            { value: 'proto', label: t('source.proto') },
            { value: 'descriptor', label: t('source.descriptor') },
            { value: 'reflection', label: t('source.reflection') },
          ]}
          aria-label={t('source.title')}
          className="w-full"
        />
        {source.kind === 'proto' && (
          <div className="space-y-2 text-xs">
            <Button size="sm" variant="outline" className="w-full justify-start" onClick={() => void pickProto()}>
              <FileCode2 />
              <span className="truncate">{source.proto ? baseName(source.proto) : t('source.pickProto')}</span>
            </Button>
            <div className="space-y-1">
              <div className="text-fg-muted">{t('source.includes')}</div>
              {source.includes.map((dir) => (
                <div key={dir} className="flex items-center gap-1 rounded-control bg-surface-2 px-2 py-1">
                  <span className="min-w-0 flex-1 truncate font-mono text-fg" data-selectable>{dir}</span>
                  <Button size="icon-sm" variant="ghost" aria-label={t('source.removeInclude')} onClick={() => setSource({ includes: source.includes.filter((d) => d !== dir) })}>
                    <X />
                  </Button>
                </div>
              ))}
              <Button size="sm" variant="ghost" onClick={() => void pickInclude()}>
                <FolderPlus />
                {t('source.addInclude')}
              </Button>
              <p className="text-fg-subtle">{t('source.includesHint')}</p>
            </div>
          </div>
        )}
        {source.kind === 'descriptor' && (
          <div className="space-y-2 text-xs">
            <Button size="sm" variant="outline" className="w-full justify-start" onClick={() => void pickDescriptor()}>
              <FileCode2 />
              <span className="truncate">{source.descriptor ? baseName(source.descriptor) : t('source.pickDescriptor')}</span>
            </Button>
            <p className="text-fg-subtle">{t('source.descriptorHint')}</p>
          </div>
        )}
        {source.kind === 'reflection' && (
          <p className="flex items-start gap-1.5 text-xs text-fg-muted">
            <Network className="mt-0.5 size-3.5 shrink-0" />
            {t('source.reflectionHint', { target: settings.target.address || '—' })}
          </p>
        )}
        <Button variant="primary" size="sm" className="w-full" loading={loading} disabled={!canLoad} onClick={() => onLoad(source.kind)}>
          <RefreshCw />
          {source.kind === 'reflection' ? t('source.reflect') : t('source.load')}
        </Button>
        {error && <p className="text-xs break-words text-danger" role="alert">{error}</p>}
      </Panel>

      <Panel
        title={t('catalog.title')}
        icon={<Server />}
        extra={loaded && <span className="text-xs text-fg-subtle">{t('catalog.stats', { files: loaded.files, messages: loaded.messages })}</span>}
        className="min-h-0 flex-1"
        bodyClassName="overflow-auto p-1.5"
      >
        {loading && !loaded ? (
          <div className="flex h-24 items-center justify-center">
            <Spinner />
          </div>
        ) : !loaded ? (
          <Empty title={t('catalog.empty')} description={t('catalog.emptyHint')} />
        ) : (
          loaded.services.map((service) => (
            <div key={service.name} className="mb-2">
              <div className="truncate px-2 py-1 font-mono text-[11px] text-fg-subtle" data-selectable>
                {service.name}
              </div>
              {service.methods.map((method) => (
                <button
                  key={method.path}
                  type="button"
                  onClick={() => onSelect(method)}
                  className={cn(
                    'flex w-full items-center gap-2 rounded-control px-2 py-1.5 text-left hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
                    selected === method.path && 'bg-active',
                    !method.supported && 'opacity-60',
                  )}
                >
                  <span className="min-w-0 flex-1 truncate font-mono text-xs text-fg">{method.name}</span>
                  <MethodBadge method={method} />
                </button>
              ))}
            </div>
          ))
        )}
      </Panel>
    </div>
  )
}
