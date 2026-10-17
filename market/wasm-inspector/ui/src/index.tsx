import { useEffect, useRef, useState } from 'react'
import { Badge, Button, DropdownMenu, Empty, Progress, Tabs, toast } from '@toolforge/ui'
import { callPlugin, formatBytes, host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { ArrowDownToLine, Binary, Boxes, Copy, FileCode2, FolderOpen, LayoutDashboard, LogIn, LogOut, SquareFunction, Upload } from 'lucide-react'
import { Exports, Functions, Imports } from './Lists'
import { OverviewView } from './OverviewView'
import type { Exported, FunctionRow, Overview, WatInfo } from './types'
import { WatView } from './WatView'

type Tab = 'overview' | 'imports' | 'exports' | 'functions' | 'wat'

/** 小于这个体积的模块打开 WAT 页时自动生成 */
const AUTO_WAT = 2 * 1024 * 1024

export default function WasmInspector() {
  const { t, manifest, errorMessage } = usePlugin()
  const loading = useTask<Overview>()
  const wat = useTask<WatInfo>()
  const [tab, setTab] = useState<Tab>('overview')
  const [focus, setFocus] = useState<number | null>(null)
  const [dropping, setDropping] = useState(false)
  const overview = loading.status === 'succeeded' ? loading.result : null

  const load = (path: string) => {
    wat.reset()
    setFocus(null)
    setTab('overview')
    void loading.start('load', { path })
  }

  const loadRef = useRef(load)
  useEffect(() => {
    loadRef.current = load
  })
  useEffect(() => {
    const off = host.onFileDrop({ drop: (paths) => paths[0] && loadRef.current(paths[0]), over: setDropping })
    return () => {
      void off.then((unlisten) => unlisten())
    }
  }, [])

  const openFile = async () => {
    const path = await host.dialog.openFile([{ name: t('open.filter'), extensions: ['wasm'] }])
    if (path) load(path)
  }

  const startWat = () => {
    if (overview) void wat.start('wat', { token: overview.token })
  }

  const openFunction = (f: FunctionRow) => {
    setFocus(f.index)
    setTab('wat')
  }

  const exportAs = async (format: 'json' | 'wat', copy: boolean) => {
    if (!overview) return
    try {
      if (format === 'wat' && wat.status !== 'succeeded') {
        toast.error(t('export.watFirst'))
        return
      }
      const out = await callPlugin<Exported>(manifest.id, 'export', { token: overview.token, format })
      if (copy) {
        await host.clipboard.writeText(out.content)
        toast.success(t('export.copied'))
        return
      }
      const base = overview.name.replace(/\.wasm$/i, '') || 'module'
      const path = await host.dialog.saveFile(`${base}.${out.ext}`, [{ name: out.ext.toUpperCase(), extensions: [out.ext] }])
      if (!path) return
      await host.fs.writeText(path, out.content)
      toast.success(t('export.saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  if (!overview) {
    return (
      <Empty
        icon={<Binary />}
        title={t('open.emptyTitle')}
        description={t('open.emptyHint')}
        action={
          <div className="flex flex-col items-center gap-3">
            <Button variant="primary" onClick={() => void openFile()} loading={loading.running}>
              <Upload />
              {t('open.openFile')}
            </Button>
            {loading.running && (
              <div className="flex w-80 items-center gap-3">
                <Progress
                  value={loading.progress ? (loading.progress.done / Math.max(loading.progress.total, 1)) * 100 : null}
                  aria-label={t('open.loading')}
                />
                <span className="shrink-0 text-xs text-fg-muted">
                  {loading.stage ? t(`stages.${loading.stage}`) : t('open.loading')}
                </span>
                <Button size="sm" onClick={() => void loading.cancel()}>
                  {t('open.cancel')}
                </Button>
              </div>
            )}
            {dropping && <p className="text-xs text-fg-muted">{t('open.drop')}</p>}
            {loading.status === 'failed' && loading.error && (
              <p className="max-w-lg text-center text-xs text-danger">{errorMessage(loading.error)}</p>
            )}
          </div>
        }
      />
    )
  }

  const v = overview.validation
  const statusVariant = v.status === 'valid' ? 'success' : v.status === 'needs_features' ? 'warning' : 'danger'

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <div className="flex flex-wrap items-center gap-3">
        <div className="flex min-w-0 items-center gap-2">
          <Binary className="size-4 shrink-0 text-fg-muted" />
          <span className="truncate text-[13px] font-semibold text-fg" data-selectable>
            {overview.name}
          </span>
          <span className="shrink-0 text-xs text-fg-muted">{formatBytes(overview.sizes.total)}</span>
          <Badge variant={statusVariant}>{t(`validation.${v.status}`)}</Badge>
        </div>
        <div className="ml-auto flex items-center gap-2">
          <Button size="sm" onClick={() => void openFile()} loading={loading.running}>
            <FolderOpen />
            {t('open.another')}
          </Button>
          <DropdownMenu
            trigger={
              <Button size="sm">
                <ArrowDownToLine />
                {t('export.title')}
              </Button>
            }
            items={[
              { key: 'json', label: t('export.json'), icon: <ArrowDownToLine />, onSelect: () => void exportAs('json', false) },
              { key: 'json-copy', label: t('export.copyJson'), icon: <Copy />, onSelect: () => void exportAs('json', true) },
              {
                key: 'wat',
                label: t('export.wat'),
                icon: <FileCode2 />,
                disabled: wat.status !== 'succeeded' || !!wat.result?.truncated,
                onSelect: () => void exportAs('wat', false),
              },
            ]}
          />
        </div>
      </div>
      <Tabs
        value={tab}
        onValueChange={setTab}
        items={[
          { value: 'overview', label: t('tabs.overview'), icon: <LayoutDashboard /> },
          { value: 'imports', label: t('tabs.imports', { count: overview.imports }), icon: <LogIn /> },
          { value: 'exports', label: t('tabs.exports', { count: overview.exports }), icon: <LogOut /> },
          { value: 'functions', label: t('tabs.functions', { count: overview.counts.functions }), icon: <SquareFunction /> },
          { value: 'wat', label: t('tabs.wat'), icon: <Boxes /> },
        ]}
        aria-label={t('name')}
      />
      <div className="min-h-0 flex-1 overflow-auto">
        {tab === 'overview' ? (
          <OverviewView overview={overview} onOpenFunction={openFunction} />
        ) : tab === 'imports' ? (
          <Imports token={overview.token} />
        ) : tab === 'exports' ? (
          <Exports token={overview.token} />
        ) : tab === 'functions' ? (
          <Functions token={overview.token} codeSize={overview.sizes.code} onOpen={openFunction} />
        ) : (
          <WatView
            token={overview.token}
            task={wat}
            autoStart={overview.sizes.total <= AUTO_WAT && !overview.parseError}
            focus={focus}
            onFocus={setFocus}
            onStart={startWat}
          />
        )}
      </div>
    </div>
  )
}
