import { useState } from 'react'
import { Button, Checkbox, Empty, Input, Modal, Select, Switch } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Eye, EyeOff, KeyRound, Plus, Trash2, Variable, X } from 'lucide-react'
import type { Environment, Variable as VariableValue } from './types'
import type { WorkspaceController } from './useWorkspace'

function VariablesEditor({ environment, onChange }: { environment: Environment; onChange: (environment: Environment) => void }) {
  const { t } = usePlugin()
  const [revealed, setRevealed] = useState<number | null>(null)
  const [newKey, setNewKey] = useState('')
  const edit = (index: number, patch: Partial<VariableValue>) => onChange({ ...environment, variables: environment.variables.map((item, i) => i === index ? { ...item, ...patch } : item) })
  return <div className="space-y-3">
    <p className="flex items-start gap-2 rounded-control bg-primary-soft/40 p-3 text-xs text-fg-muted"><KeyRound className="size-4 shrink-0 text-primary" />{t('environment.secretHint')}</p>
    <div className="space-y-2">
      {environment.variables.map((item, index) => <div key={index} className="space-y-2 rounded-control border border-border p-2">
        <div className="flex items-center gap-2">
          <Checkbox checked={item.enabled} onCheckedChange={(enabled) => edit(index, { enabled })} aria-label={t('request.enabled')} />
          <Input size="sm" value={item.key} onChange={(event) => edit(index, { key: event.target.value })} placeholder={t('environment.key')} wrapperClassName="min-w-0 flex-1" className="font-mono" aria-label={t('environment.key')} />
          <Button variant="ghost" size="icon-sm" onClick={() => { setRevealed(null); onChange({ ...environment, variables: environment.variables.filter((_, i) => i !== index) }) }} aria-label={t('request.remove')}><X /></Button>
        </div>
        <div className="flex flex-wrap items-center gap-2 pl-6">
          <Input size="sm" type={item.secret && revealed !== index ? 'password' : 'text'} value={item.value} onChange={(event) => edit(index, { value: event.target.value })} placeholder={item.secret ? t('environment.sessionValue') : t('request.value')} wrapperClassName="min-w-28 flex-1" className="font-mono" autoComplete="off" aria-label={t('request.value')} />
          {item.secret && <Button variant="ghost" size="icon-sm" onClick={() => setRevealed(revealed === index ? null : index)} aria-label={revealed === index ? t('environment.hide') : t('environment.reveal')}>{revealed === index ? <EyeOff /> : <Eye />}</Button>}
          <label className="flex items-center gap-1.5 text-xs text-fg-muted"><Switch checked={item.secret} onCheckedChange={(secret) => edit(index, { secret })} aria-label={t('environment.secret')} />{t('environment.secret')}</label>
        </div>
      </div>)}
    </div>
    <form className="flex gap-2" onSubmit={(event) => { event.preventDefault(); if (!newKey.trim()) return; onChange({ ...environment, variables: [...environment.variables, { key: newKey.trim(), value: '', enabled: true, secret: false }] }); setNewKey('') }}>
      <Input size="sm" value={newKey} onChange={(event) => setNewKey(event.target.value)} placeholder={t('environment.key')} wrapperClassName="min-w-0 flex-1" className="font-mono" aria-label={t('environment.newKey')} />
      <Button type="submit" size="sm" variant="outline" disabled={!newKey.trim()}><Plus />{t('environment.addVariable')}</Button>
    </form>
  </div>
}

export function EnvironmentsModal({ open, onClose, controller }: { open: boolean; onClose: () => void; controller: WorkspaceController }) {
  const { t } = usePlugin()
  const [name, setName] = useState('')
  const [renaming, setRenaming] = useState(false)
  const [renameValue, setRenameValue] = useState('')
  const active = controller.workspace.environments.find((item) => item.id === controller.workspace.activeEnvironmentId) ?? null
  return <Modal open={open} onOpenChange={(value) => !value && onClose()} title={t('environment.manage')} description={t('environment.hint')} size="lg" closeLabel={t('common:close')}>
    <div className="space-y-4 px-5 pb-5">
      <form className="flex gap-2" onSubmit={(event) => { event.preventDefault(); if (name.trim()) void controller.createEnvironment(name.trim()).then((created) => { if (created) setName('') }) }}>
        <Input value={name} onChange={(event) => setName(event.target.value)} placeholder={t('environment.name')} wrapperClassName="min-w-0 flex-1" aria-label={t('environment.name')} />
        <Button type="submit" variant="outline" disabled={!name.trim() || controller.busy}><Plus />{t('environment.add')}</Button>
      </form>
      {!controller.workspace.environments.length ? <Empty icon={<Variable />} title={t('environment.empty')} /> : <>
        <div className="flex flex-wrap gap-2">
          <Select value={controller.workspace.activeEnvironmentId ?? ''} onValueChange={(id) => { controller.selectEnvironment(id || null); setRenaming(false) }} options={[{ value: '', label: t('environment.none') }, ...controller.workspace.environments.map((item) => ({ value: item.id, label: item.name }))]} className="min-w-36 flex-1" aria-label={t('environment.active')} />
          {active && <><Button variant="ghost" onClick={() => { setRenameValue(active.name); setRenaming(true) }}>{t('environment.rename')}</Button><Button variant="ghost" size="icon-sm" onClick={() => { controller.deleteEnvironment(active.id); setRenaming(false) }} aria-label={t('environment.delete')}><Trash2 /></Button></>}
        </div>
        {active && renaming && <form className="flex gap-2" onSubmit={(event) => { event.preventDefault(); if (renameValue.trim()) { controller.editEnvironment({ ...active, name: renameValue.trim() }); setRenaming(false) } }}><Input value={renameValue} onChange={(event) => setRenameValue(event.target.value)} wrapperClassName="flex-1" aria-label={t('environment.name')} /><Button type="submit" disabled={!renameValue.trim()}>{t('workspace.save')}</Button><Button variant="ghost" onClick={() => setRenaming(false)}>{t('common:cancel')}</Button></form>}
        {active ? <VariablesEditor key={active.id} environment={active} onChange={controller.editEnvironment} /> : <p className="text-xs text-fg-subtle">{t('environment.choose')}</p>}
      </>}
    </div>
  </Modal>
}
