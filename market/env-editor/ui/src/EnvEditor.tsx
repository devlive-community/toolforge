import { useEffect, useMemo, useRef, useState } from 'react'
import { Badge, Button, Input, Panel, Spinner, Switch, Tooltip, cn, toast } from '@toolforge/ui'
import { host, usePlugin } from '@toolforge/plugin-ui-sdk'
import { FilePlus2, FileText, FileUp, FolderOpen, Save, Trash2, Upload } from 'lucide-react'
import { ENV_EXTENSIONS, KEY_PATTERN, baseName, type Line, type OpenResult } from './types'

const isEnvFile = (path: string) => baseName(path).toLowerCase().startsWith('.env')
const lineKey = (line: Line) => (line.kind === 'entry' ? line.key : '')

const rowGrid = 'grid grid-cols-[auto_minmax(140px,220px)_minmax(0,1fr)_minmax(120px,180px)_auto] items-center gap-2'

export function EnvEditor({ active }: { active: boolean }) {
  const { t, call, errorMessage } = usePlugin()
  const [path, setPath] = useState<string | null>(null)
  const [lines, setLines] = useState<Line[]>([])
  const [saved, setSaved] = useState('')
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)
  const [dropping, setDropping] = useState(false)

  const dirty = useMemo(() => path !== null && JSON.stringify(lines) !== saved, [path, lines, saved])
  const entries = lines.filter((line): line is Extract<Line, { kind: 'entry' }> => line.kind === 'entry')
  const duplicates = useMemo(() => {
    const seen = new Set<string>()
    const repeated = new Set<string>()
    for (const key of entries.map(lineKey)) {
      if (seen.has(key)) repeated.add(key)
      else seen.add(key)
    }
    return repeated
  }, [entries])
  const hasInvalidKey = entries.some((entry) => !KEY_PATTERN.test(entry.key))
  const canSave = dirty && !loading && !saving && !hasInvalidKey && duplicates.size === 0

  const open = async (target: string) => {
    setLoading(true)
    try {
      const result = await call<OpenResult>('open', { path: target })
      setPath(result.path)
      setLines(result.lines)
      setSaved(JSON.stringify(result.lines))
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setLoading(false)
    }
  }

  const pick = async () => {
    try {
      const files = await host.dialog.openFiles([{ name: t('dialog.filter'), extensions: ENV_EXTENSIONS }])
      if (files[0]) await open(files[0])
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  // 仅在当前页可见时注册 onFileDrop，回调经由 ref 转发到最新版本
  const openRef = useRef(open)
  useEffect(() => {
    openRef.current = open
  })
  useEffect(() => {
    if (!active) return
    const off = host.onFileDrop({ drop: (paths) => void openRef.current(paths.find(isEnvFile) ?? paths[0]), over: setDropping })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [active])

  const save = async () => {
    if (!path) return
    setSaving(true)
    try {
      await call('save', { path, lines })
      setSaved(JSON.stringify(lines))
      toast.success(t('toast.saved'))
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setSaving(false)
    }
  }

  const exportExample = async () => {
    if (!path) return
    setSaving(true)
    try {
      const result = await call<{ path: string }>('export_example', { path, lines })
      toast.success(t('toast.example', { path: result.path }))
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setSaving(false)
    }
  }

  const update = (index: number, next: Line) => setLines((prev) => prev.map((line, i) => (i === index ? next : line)))
  const remove = (index: number) => setLines((prev) => prev.filter((_, i) => i !== index))

  const addEntry = () =>
    setLines((prev) => {
      const next = [...prev]
      if (next.length > 0 && next[next.length - 1].kind !== 'blank') next.push({ kind: 'blank' })
      next.push({ kind: 'entry', export: false, key: '', value: '', quote: 'none', inlineComment: null, multiline: false })
      return next
    })
  const addComment = () =>
    setLines((prev) => {
      const next = [...prev]
      if (next.length > 0 && next[next.length - 1].kind !== 'blank') next.push({ kind: 'blank' })
      next.push({ kind: 'comment', text: '# ' })
      return next
    })

  if (!path) {
    return (
      <Panel className="h-full" bodyClassName="flex h-full items-center justify-center">
        {loading ? (
          <div className="flex flex-col items-center gap-2 text-fg-muted">
            <Spinner className="size-6" />
            <p className="text-xs">{t('state.loading')}</p>
          </div>
        ) : (
          <button
            type="button"
            onClick={pick}
            className={cn(
              'flex h-full min-h-64 w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
              'focus-visible:ring-2 focus-visible:ring-ring',
              dropping ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
            )}
          >
            <Upload className="size-6" />
            <span className="text-[13px] font-medium">{t('state.drop')}</span>
            <span className="text-xs text-fg-subtle">{t('state.dropHint')}</span>
          </button>
        )}
      </Panel>
    )
  }

  return (
    <Panel
      icon={<FileText />}
      title={baseName(path)}
      extra={
        dirty ? (
          <Badge>{t('state.dirty')}</Badge>
        ) : (
          <Badge variant="success">{t('state.clean')}</Badge>
        )
      }
      actions={
        <>
          <Tooltip content={t('actions.example')}>
            <Button size="sm" onClick={exportExample} disabled={saving || loading || hasInvalidKey || duplicates.size > 0}>
              <FileUp />
              {t('actions.exportExample')}
            </Button>
          </Tooltip>
          <Button size="sm" onClick={pick} disabled={loading || saving}>
            <FolderOpen />
            {t('actions.openOther')}
          </Button>
          <Button size="sm" variant="primary" onClick={save} disabled={!canSave}>
            <Save />
            {t('actions.save')}
          </Button>
        </>
      }
      footer={
        <div className="flex items-center justify-between gap-3">
          <span className="min-w-0 flex-1 truncate text-xs text-fg-subtle" data-selectable>
            {path}
          </span>
          <span className="shrink-0 text-xs text-fg-subtle tabular-nums">{t('state.count', { count: entries.length })}</span>
          <div className="flex shrink-0 items-center gap-1.5">
            <Button size="sm" onClick={addComment}>
              <FileText />
              {t('actions.addComment')}
            </Button>
            <Button size="sm" variant="outline" onClick={addEntry}>
              <FilePlus2 />
              {t('actions.addEntry')}
            </Button>
          </div>
        </div>
      }
      className="h-full"
      bodyClassName={cn('overflow-auto p-3', dropping && 'ring-2 ring-primary')}
    >
      {hasInvalidKey && <p className="mb-2 text-xs text-danger">{t('warn.invalidKey')}</p>}
      {duplicates.size > 0 && <p className="mb-2 text-xs text-warning">{t('warn.duplicate', { keys: [...duplicates].join(', ') })}</p>}

      <div className="space-y-0.5">
        {lines.map((line, index) => {
          if (line.kind === 'blank') return <div key={index} className="h-3" aria-hidden />
          if (line.kind === 'comment') {
            return (
              <div key={index} className={rowGrid}>
                <div />
                <Input
                  className="col-span-2 font-mono text-[13px] text-fg-muted italic"
                  value={line.text}
                  aria-label={t('row.comment')}
                  onChange={(event) => update(index, { kind: 'comment', text: event.target.value })}
                />
                <div />
                <DeleteButton index={index} onClick={remove} label={t('row.delete')} />
              </div>
            )
          }
          if (line.kind === 'passthrough') {
            return (
              <div key={index} className={rowGrid}>
                <div />
                <p className="col-span-3 truncate font-mono text-xs text-fg-subtle" title={line.text} data-selectable>
                  {line.text}
                </p>
                <div />
                <DeleteButton index={index} onClick={remove} label={t('row.delete')} />
              </div>
            )
          }
          const invalidKey = !KEY_PATTERN.test(line.key)
          const duplicate = line.key !== '' && duplicates.has(line.key)
          return (
            <div key={index} className={rowGrid}>
              <Tooltip content={t('row.export')}>
                <Switch checked={line.export} onCheckedChange={(checked) => update(index, { ...line, export: checked })} aria-label={t('row.export')} />
              </Tooltip>
              <div className="min-w-0">
                <Input
                  className={cn('font-mono text-[13px]', invalidKey && 'border-danger focus-visible:ring-danger')}
                  value={line.key}
                  placeholder="KEY"
                  aria-label={t('row.key')}
                  aria-invalid={invalidKey}
                  onChange={(event) => update(index, { ...line, key: event.target.value })}
                />
              </div>
              <div className="min-w-0">
                <Input
                  className="w-full font-mono text-[13px]"
                  value={line.value}
                  placeholder={t('row.valuePlaceholder')}
                  aria-label={t('row.value')}
                  onChange={(event) => update(index, { ...line, value: event.target.value })}
                />
              </div>
              <div className="flex min-w-0 items-center gap-1.5">
                <Input
                  className="min-w-0 font-mono text-xs text-fg-muted"
                  value={line.inlineComment ?? ''}
                  placeholder={t('row.commentPlaceholder')}
                  aria-label={t('row.inlineComment')}
                  onChange={(event) => update(index, { ...line, inlineComment: event.target.value === '' ? null : event.target.value })}
                />
                {line.multiline && (
                  <Tooltip content={t('row.multiline')}>
                    <Badge aria-hidden>{'\u21B5'}</Badge>
                  </Tooltip>
                )}
                {duplicate && (
                  <Tooltip content={t('row.duplicate')}>
                    <Badge variant="warning">{t('row.duplicateShort')}</Badge>
                  </Tooltip>
                )}
              </div>
              <DeleteButton index={index} onClick={remove} label={t('row.delete')} />
            </div>
          )
        })}
      </div>
    </Panel>
  )
}

function DeleteButton({ index, onClick, label }: { index: number; onClick: (index: number) => void; label: string }) {
  return (
    <Tooltip content={label}>
      <Button size="icon-sm" variant="ghost" aria-label={label} onClick={() => onClick(index)}>
        <Trash2 />
      </Button>
    </Tooltip>
  )
}
