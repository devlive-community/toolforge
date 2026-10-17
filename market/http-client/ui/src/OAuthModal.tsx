import { useState, type ReactNode } from 'react'
import { Button, Empty, Input, Modal, NumberInput, Switch, cn } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { KeyRound, Plus, Trash2 } from 'lucide-react'
import { SessionCard } from './SessionCard'
import type { OAuthConfig } from './types'
import type { OAuthController } from './useOAuth'
import type { WorkspaceController } from './useWorkspace'

const blank = (): OAuthConfig => ({
  id: '',
  name: '',
  authorizationUrl: '',
  tokenUrl: '',
  clientId: '',
  scope: '',
  redirectPort: null,
  redirectPath: '/callback',
})

function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="block space-y-1 text-xs text-fg-muted">
      <span>{label}</span>
      {children}
      {hint && <span className="block text-fg-subtle">{hint}</span>}
    </label>
  )
}

function ConfigForm({ initial, controller, oauth, onSaved, onDeleted }: { initial: OAuthConfig; controller: WorkspaceController; oauth: OAuthController; onSaved: (id: string) => void; onDeleted: () => void }) {
  const { t } = usePlugin()
  const [draft, setDraft] = useState(initial)
  const edit = (patch: Partial<OAuthConfig>) => setDraft((prev) => ({ ...prev, ...patch }))
  const dirty = JSON.stringify(draft) !== JSON.stringify(initial)
  const complete = [draft.name, draft.authorizationUrl, draft.tokenUrl, draft.clientId].every((v) => v.trim())
  const redirect = `http://127.0.0.1:${draft.redirectPort ?? t('oauth.randomPort')}${draft.redirectPath}`
  const save = async () => {
    const id = await controller.saveOAuthConfig({ ...draft, name: draft.name.trim(), redirectPath: draft.redirectPath.trim() || '/callback' })
    if (id) onSaved(id)
  }
  return (
    <div className="space-y-3">
      <Field label={t('oauth.name')}>
        <Input value={draft.name} onChange={(e) => edit({ name: e.target.value })} aria-label={t('oauth.name')} />
      </Field>
      <Field label={t('oauth.authorizationUrl')}>
        <Input value={draft.authorizationUrl} onChange={(e) => edit({ authorizationUrl: e.target.value })} placeholder="https://auth.example.com/authorize" className="font-mono" aria-label={t('oauth.authorizationUrl')} />
      </Field>
      <Field label={t('oauth.tokenUrl')}>
        <Input value={draft.tokenUrl} onChange={(e) => edit({ tokenUrl: e.target.value })} placeholder="https://auth.example.com/token" className="font-mono" aria-label={t('oauth.tokenUrl')} />
      </Field>
      <div className="grid gap-3 sm:grid-cols-2">
        <Field label={t('oauth.clientId')}>
          <Input value={draft.clientId} onChange={(e) => edit({ clientId: e.target.value })} className="font-mono" aria-label={t('oauth.clientId')} />
        </Field>
        <Field label={t('oauth.scope')}>
          <Input value={draft.scope} onChange={(e) => edit({ scope: e.target.value })} placeholder="openid profile" className="font-mono" aria-label={t('oauth.scope')} />
        </Field>
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        <Field label={t('oauth.redirectPath')}>
          <Input value={draft.redirectPath} onChange={(e) => edit({ redirectPath: e.target.value })} className="font-mono" aria-label={t('oauth.redirectPath')} />
        </Field>
        <Field label={t('oauth.fixedPort')}>
          <div className="flex items-center gap-2">
            <Switch checked={draft.redirectPort !== null} onCheckedChange={(on) => edit({ redirectPort: on ? 8765 : null })} aria-label={t('oauth.fixedPort')} />
            {draft.redirectPort !== null && (
              <NumberInput size="sm" value={draft.redirectPort} onValueChange={(port) => edit({ redirectPort: port })} min={1024} max={65535} className="w-28" aria-label={t('oauth.port')} />
            )}
          </div>
        </Field>
      </div>
      <p className="rounded-control bg-surface-2 p-2 text-xs text-fg-muted">
        {t('oauth.redirectHint')}
        <span className="mt-1 block font-mono text-fg" data-selectable>{redirect}</span>
      </p>
      <p className="text-xs text-fg-subtle">{t('oauth.variablesHint')}</p>
      <div className="flex flex-wrap items-center gap-2">
        <Button variant="primary" disabled={!complete || !dirty || controller.busy} onClick={() => void save()}>{t('workspace.save')}</Button>
        {initial.id && (
          <Button variant="ghost" onClick={() => { controller.deleteOAuthConfig(initial.id); void oauth.clear(initial.id); onDeleted() }}>
            <Trash2 />{t('oauth.delete')}
          </Button>
        )}
      </div>
      {initial.id && !dirty && (
        <div className="space-y-2 border-t border-border pt-3">
          <p className="text-xs font-medium text-fg">{t('oauth.session')}</p>
          <SessionCard config={initial} oauth={oauth} />
        </div>
      )}
      {initial.id && dirty && <p className="text-xs text-fg-subtle">{t('oauth.saveFirst')}</p>}
    </div>
  )
}

export function OAuthModal({ open, onClose, controller, oauth }: { open: boolean; onClose: () => void; controller: WorkspaceController; oauth: OAuthController }) {
  const { t } = usePlugin()
  const configs = controller.workspace.oauthConfigs ?? []
  const [selected, setSelected] = useState<string | null>(null)
  const current = selected === '' ? blank() : configs.find((c) => c.id === selected) ?? configs[0] ?? null
  return (
    <Modal open={open} onOpenChange={(value) => !value && onClose()} title={t('oauth.manage')} description={t('oauth.manageHint')} size="lg" closeLabel={t('common:close')}>
      <div className="grid gap-4 px-5 pb-5 sm:grid-cols-[180px_1fr]">
        <div className="space-y-1">
          {configs.map((c) => (
            <button
              key={c.id}
              type="button"
              onClick={() => setSelected(c.id)}
              className={cn('flex w-full items-center gap-2 rounded-control px-2 py-1.5 text-left text-[13px] hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none', current?.id === c.id && selected !== '' && 'bg-active')}
            >
              <KeyRound className="size-3.5 shrink-0 text-fg-subtle" />
              <span className="truncate text-fg">{c.name}</span>
            </button>
          ))}
          <Button size="sm" variant="ghost" className="w-full justify-start" onClick={() => setSelected('')}><Plus />{t('oauth.add')}</Button>
        </div>
        {current ? (
          <ConfigForm key={current.id || 'new'} initial={current} controller={controller} oauth={oauth} onSaved={setSelected} onDeleted={() => setSelected(null)} />
        ) : (
          <Empty icon={<KeyRound />} title={t('oauth.empty')} description={t('oauth.emptyHint')} action={<Button variant="primary" onClick={() => setSelected('')}><Plus />{t('oauth.add')}</Button>} />
        )}
      </div>
    </Modal>
  )
}
