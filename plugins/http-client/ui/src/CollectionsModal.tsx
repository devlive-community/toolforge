import { useState } from 'react'
import { Badge, Button, Empty, Input, Modal } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Folder, Pencil, Plus, Trash2 } from 'lucide-react'
import type { WorkspaceController } from './useWorkspace'

export function CollectionsModal({ open, onClose, controller }: { open: boolean; onClose: () => void; controller: WorkspaceController }) {
  const { t } = usePlugin()
  const [editing, setEditing] = useState<string | null>(null)
  const [name, setName] = useState('')
  const save = async () => {
    if (!name.trim()) return
    if (await controller.saveCollection(editing, name.trim())) { setEditing(null); setName('') }
  }
  return <Modal open={open} onOpenChange={(value) => !value && onClose()} title={t('workspace.collections')} description={t('workspace.collectionHint')} closeLabel={t('common:close')}>
    <div className="space-y-3 px-5 pb-5">
      <form className="flex gap-2" onSubmit={(event) => { event.preventDefault(); void save() }}>
        <Input value={name} onChange={(event) => setName(event.target.value)} placeholder={t('workspace.collectionName')} aria-label={t('workspace.collectionName')} wrapperClassName="min-w-0 flex-1" />
        <Button type="submit" variant="primary" disabled={!name.trim() || controller.busy}>{editing ? <Pencil /> : <Plus />}{editing ? t('workspace.save') : t('workspace.addCollection')}</Button>
        {editing && <Button variant="ghost" onClick={() => { setEditing(null); setName('') }}>{t('common:cancel')}</Button>}
      </form>
      {!controller.workspace.collections.length ? <Empty icon={<Folder />} title={t('workspace.noCollections')} /> : controller.workspace.collections.map((item) => <div key={item.id} className="flex items-center gap-2 rounded-control border border-border p-2">
        <Folder className="size-4 shrink-0 text-fg-subtle" /><span className="min-w-0 flex-1 truncate text-[13px] text-fg">{item.name}</span><Badge>{controller.workspace.requests.filter((request) => request.collectionId === item.id).length}</Badge>
        <Button variant="ghost" size="icon-sm" onClick={() => { setEditing(item.id); setName(item.name) }} aria-label={t('workspace.renameCollection')}><Pencil /></Button>
        <Button variant="ghost" size="icon-sm" onClick={() => { controller.deleteCollection(item.id); if (editing === item.id) { setEditing(null); setName('') } }} aria-label={t('workspace.deleteCollection')}><Trash2 /></Button>
      </div>)}
    </div>
  </Modal>
}
