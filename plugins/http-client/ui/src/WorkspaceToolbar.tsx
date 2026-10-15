import { useState } from 'react'
import { Button, Input, Modal, Select, Spinner } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Check, Copy, FolderCog, Pencil, Plus, RotateCw, Trash2, Variable } from 'lucide-react'
import type { WorkspaceController } from './useWorkspace'

export function WorkspaceToolbar({ controller, running, onCollections, onEnvironments }: { controller: WorkspaceController; running: boolean; onCollections: () => void; onEnvironments: () => void }) {
  const { t } = usePlugin()
  const [details, setDetails] = useState<{ id: string; name: string; collectionId: string } | null>(null)
  const [collectionFilter, setCollectionFilter] = useState('*')
  const { workspace, selected, busy, loaded } = controller
  const locked = !loaded || busy || running
  const filterExists = collectionFilter === '*' || collectionFilter === '' || workspace.collections.some((item) => item.id === collectionFilter)
  const filter = filterExists && (!selected || collectionFilter === '*' || (selected.collectionId ?? '') === collectionFilter) ? collectionFilter : '*'
  const requests = workspace.requests.filter((item) => filter === '*' || (item.collectionId ?? '') === filter)
  return (
    <>
      <div className="flex flex-wrap items-center gap-2 rounded-card border border-border bg-surface p-3">
        <Select value={filter} onValueChange={(value) => { setCollectionFilter(value); const first = workspace.requests.find((item) => value === '*' || (item.collectionId ?? '') === value); controller.selectRequest(first?.id ?? null) }} disabled={locked} options={[{ value: '*', label: t('workspace.allCollections') }, { value: '', label: t('workspace.ungrouped') }, ...workspace.collections.map((item) => ({ value: item.id, label: item.name }))]} className="w-36" aria-label={t('workspace.collection')} />
        <Select value={controller.selectedId} onValueChange={(id) => { setCollectionFilter(filter); controller.selectRequest(id) }} disabled={locked || !requests.length} placeholder={t('workspace.empty')} options={requests.map((item) => ({ value: item.id, label: `${item.request.method} · ${item.name}` }))} className="min-w-40 flex-1 sm:max-w-72" aria-label={t('workspace.requests')} />
        <Button size="icon-sm" variant="ghost" disabled={locked} onClick={() => void controller.createRequest(undefined, undefined, filter === '*' ? null : filter || null)} aria-label={t('workspace.new')}><Plus /></Button>
        <Button size="icon-sm" variant="ghost" disabled={locked || !selected} onClick={() => selected && setDetails({ id: selected.id, name: selected.name, collectionId: selected.collectionId ?? '' })} aria-label={t('workspace.details')}><Pencil /></Button>
        <Button size="icon-sm" variant="ghost" disabled={locked || !selected} onClick={() => void controller.duplicateRequest()} aria-label={t('workspace.duplicate')}><Copy /></Button>
        <Button size="icon-sm" variant="ghost" disabled={locked || !selected} onClick={controller.deleteRequest} aria-label={t('workspace.delete')}><Trash2 /></Button>
        <Button size="icon-sm" variant="ghost" disabled={locked} onClick={onCollections} aria-label={t('workspace.collections')}><FolderCog /></Button>
        <div className="ml-auto flex min-w-0 flex-wrap items-center gap-2">
          <Select value={workspace.activeEnvironmentId ?? ''} onValueChange={(id) => controller.selectEnvironment(id || null)} disabled={locked} options={[{ value: '', label: t('environment.none') }, ...workspace.environments.map((item) => ({ value: item.id, label: item.name }))]} className="w-40" aria-label={t('environment.active')} />
          <Button size="icon-sm" variant="ghost" disabled={locked} onClick={onEnvironments} aria-label={t('environment.manage')}><Variable /></Button>
          <span className="flex items-center gap-1 text-xs text-fg-subtle" role="status" title={controller.saveError ?? undefined}>
            {controller.saveStatus === 'saving' ? <Spinner className="size-3" /> : controller.saveStatus === 'saved' ? <Check className="size-3 text-success" /> : null}
            {t(`workspace.${controller.saveStatus}`)}
          </span>
          {controller.saveStatus === 'error' && <Button size="icon-sm" variant="ghost" onClick={() => void controller.flush()} aria-label={t('workspace.retry')}><RotateCw /></Button>}
        </div>
      </div>
      {controller.saveError && <p className="text-xs text-danger" role="alert">{controller.saveError}</p>}
      <Modal open={details !== null} onOpenChange={(open) => !open && setDetails(null)} title={t('workspace.details')} closeLabel={t('common:close')} footer={<Button variant="primary" disabled={!details?.name.trim()} onClick={() => { if (details) controller.editDetails(details.id, details.name.trim(), details.collectionId || null); setDetails(null) }}>{t('workspace.save')}</Button>}>
        {details && <div className="space-y-4 px-5 pb-5">
          <label className="block space-y-1 text-xs text-fg-muted"><span>{t('workspace.name')}</span><Input autoFocus value={details.name} onChange={(event) => setDetails({ ...details, name: event.target.value })} aria-label={t('workspace.name')} /></label>
          <label className="block space-y-1 text-xs text-fg-muted"><span>{t('workspace.collection')}</span><Select value={details.collectionId} onValueChange={(collectionId) => setDetails({ ...details, collectionId })} options={[{ value: '', label: t('workspace.ungrouped') }, ...workspace.collections.map((item) => ({ value: item.id, label: item.name }))]} className="w-full" aria-label={t('workspace.collection')} /></label>
        </div>}
      </Modal>
    </>
  )
}
