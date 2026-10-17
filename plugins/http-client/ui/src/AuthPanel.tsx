import { Button, Select } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Info, Settings2, TriangleAlert } from 'lucide-react'
import { SessionCard } from './SessionCard'
import { AUTH_NONE, type Collection, type OAuthConfig, type Request } from './types'
import type { OAuthController } from './useOAuth'

const INHERIT = '__inherit'

export interface AuthBinding {
  /** 请求自身的设置：null 沿用集合，'none' 不使用，否则为配置 id */
  request: string | null
  collection: Collection | null
  configs: OAuthConfig[]
  /** 实际生效的配置 */
  effective: OAuthConfig | null
  oauth: OAuthController
  onRequestChange: (auth: string | null) => void
  onCollectionChange: (collectionId: string, auth: string | null) => void
  onManage: () => void
}

export function AuthPanel({ binding, request }: { binding: AuthBinding; request: Request }) {
  const { t } = usePlugin()
  const { configs, collection, effective, oauth } = binding
  const inheritedName = configs.find((c) => c.id === collection?.auth)?.name
  const conflict = effective !== null && request.headers.some((p) => p.enabled && p.key.trim().toLowerCase() === 'authorization')
  return (
    <div className="space-y-4">
      {configs.length === 0 ? (
        <div className="flex flex-wrap items-center gap-3 rounded-control border border-border bg-surface-2 p-3 text-xs text-fg-muted">
          <Info className="size-4 shrink-0 text-fg-subtle" />
          <span className="min-w-0 flex-1">{t('oauth.noConfigs')}</span>
          <Button size="sm" variant="outline" onClick={binding.onManage}><Settings2 />{t('oauth.manage')}</Button>
        </div>
      ) : (
        <>
          <label className="block space-y-1 text-xs text-fg-muted">
            <span>{t('oauth.requestBinding')}</span>
            <div className="flex gap-2">
              <Select
                value={binding.request ?? INHERIT}
                onValueChange={(value) => binding.onRequestChange(value === INHERIT ? null : value)}
                options={[
                  {
                    value: INHERIT,
                    label: collection
                      ? t('oauth.inherit', { name: inheritedName ?? t('oauth.noneShort') })
                      : t('oauth.inheritNone'),
                  },
                  { value: AUTH_NONE, label: t('oauth.none') },
                  ...configs.map((c) => ({ value: c.id, label: c.name })),
                ]}
                className="min-w-0 flex-1"
                aria-label={t('oauth.requestBinding')}
              />
              <Button size="icon-md" variant="ghost" onClick={binding.onManage} aria-label={t('oauth.manage')}><Settings2 /></Button>
            </div>
          </label>
          {collection && (
            <label className="block space-y-1 text-xs text-fg-muted">
              <span>{t('oauth.collectionBinding', { name: collection.name })}</span>
              <Select
                value={collection.auth ?? ''}
                onValueChange={(value) => binding.onCollectionChange(collection.id, value || null)}
                options={[{ value: '', label: t('oauth.none') }, ...configs.map((c) => ({ value: c.id, label: c.name }))]}
                className="w-full"
                aria-label={t('oauth.collectionBinding', { name: collection.name })}
              />
            </label>
          )}
          {effective ? (
            <div className="space-y-2">
              <p className="text-xs text-fg-muted">{t('oauth.effective', { name: effective.name })}</p>
              {conflict && (
                <p className="flex items-start gap-1.5 rounded-control bg-warning-soft p-2 text-xs text-fg">
                  <TriangleAlert className="mt-0.5 size-3.5 shrink-0 text-warning" />
                  {t('oauth.conflict')}
                </p>
              )}
              <SessionCard config={effective} oauth={oauth} />
            </div>
          ) : (
            <p className="text-xs text-fg-subtle">{t('oauth.unbound')}</p>
          )}
        </>
      )}
      <p className="text-xs text-fg-subtle">{t('oauth.memoryOnly')}</p>
    </div>
  )
}
