import { Badge, Button, DropdownMenu, Empty, Spinner, cn, toast } from '@toolforge/ui'
import { host, usePlugin, type AppError } from '@toolforge/plugin-ui-sdk'
import { ChevronDown, CircleCheck, FileDown, GitCompare } from 'lucide-react'
import type { DiffReport, Mode, PatchResult } from '../types'
import { ErrorCard } from './ErrorCard'

const badge = { added: 'success', removed: 'danger', changed: 'warning' } as const

interface Sources {
  left: string
  right: string
  mode: Mode
}

function ExportMenu({ sources }: { sources: Sources }) {
  const { t, call, errorMessage } = usePlugin()
  const exportPatch = async (kind: 'patch' | 'merge', target: 'copy' | 'save') => {
    try {
      const result = await call<PatchResult>('diff_patch', { ...sources, kind })
      if (target === 'copy') {
        await host.clipboard.writeText(result.output)
        toast.success(t('common:copied'))
      } else {
        const path = await host.dialog.saveFile(kind === 'patch' ? 'patch.json' : 'merge-patch.json', [{ name: 'JSON', extensions: ['json'] }])
        if (!path) return
        await host.fs.writeText(path, result.output)
        toast.success(t('common:saved'))
      }
      if (result.nullPaths.length > 0) toast.info(t('diff.export.nullWarning', { paths: result.nullPaths.join(', ') }))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }
  return (
    <DropdownMenu
      placement="bottom-end"
      trigger={
        <Button size="sm" variant="outline" className="ml-auto">
          <FileDown />
          {t('diff.export.title')}
          <ChevronDown />
        </Button>
      }
      items={[
        { key: 'copy-patch', label: t('diff.export.copyPatch'), onSelect: () => void exportPatch('patch', 'copy') },
        { key: 'save-patch', label: t('diff.export.savePatch'), onSelect: () => void exportPatch('patch', 'save') },
        { key: 'copy-merge', label: t('diff.export.copyMerge'), onSelect: () => void exportPatch('merge', 'copy') },
        { key: 'save-merge', label: t('diff.export.saveMerge'), onSelect: () => void exportPatch('merge', 'save') },
      ]}
    />
  )
}

export function DiffResult({ report, error, pending, sources }: { report: DiffReport | null; error: AppError | null; pending: boolean; sources: Sources }) {
  const { t } = usePlugin()
  if (error) return <ErrorCard error={error} />
  if (!report) {
    return pending ? (
      <div className="flex h-full items-center justify-center text-fg-muted">
        <Spinner />
      </div>
    ) : (
      <Empty icon={<GitCompare />} title={t('diff.empty')} description={t('diff.emptyHint')} />
    )
  }
  if (report.items.length === 0) {
    return <Empty icon={<CircleCheck className="text-success" />} title={t('diff.same')} />
  }
  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-border px-3 py-2">
        <Badge variant="success">{t('diff.added', { count: report.added })}</Badge>
        <Badge variant="danger">{t('diff.removed', { count: report.removed })}</Badge>
        <Badge variant="warning">{t('diff.changed', { count: report.changed })}</Badge>
        <ExportMenu sources={sources} />
      </div>
      <ul className="min-h-0 flex-1 divide-y divide-border overflow-auto">
        {report.items.map((item, index) => (
          <li key={`${item.path}-${index}`} className="px-3 py-2">
            <div className="flex items-center gap-2">
              <Badge variant={badge[item.change]}>{t(`diff.kind.${item.change}`)}</Badge>
              <code className="truncate font-mono text-xs text-fg" data-selectable>
                {item.path}
              </code>
            </div>
            <div className="mt-1.5 space-y-0.5 font-mono text-xs" data-selectable>
              {item.left !== null && (
                <p className={cn('truncate', item.change === 'changed' ? 'text-fg-subtle line-through' : 'text-danger')}>
                  − {item.left}
                </p>
              )}
              {item.right !== null && <p className="truncate text-success">+ {item.right}</p>}
            </div>
          </li>
        ))}
        {report.truncated && <li className="px-3 py-2 text-xs text-fg-subtle">{t('diff.truncated')}</li>}
      </ul>
    </div>
  )
}
