import { Badge, Button, Spinner } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Copy, ExternalLink, KeyRound, LogIn, RefreshCw, ShieldCheck, Square, Trash2, TriangleAlert } from 'lucide-react'
import type { OAuthConfig } from './types'
import { sessionKey, type OAuthController } from './useOAuth'

const time = (ms: number) => new Date(ms).toLocaleString()

/** 一个授权配置在当前环境下的会话：状态、遮罩令牌与操作 */
export function SessionCard({ config, oauth }: { config: OAuthConfig; oauth: OAuthController }) {
  const { t, errorMessage } = usePlugin()
  const session = oauth.session(config.id)
  const flow = oauth.flow?.configId === config.id ? oauth.flow : null
  const key = sessionKey(config.id, oauth.environmentId)
  const refreshing = oauth.refreshingKey === key
  const busy = oauth.authorizing || refreshing

  if (flow) {
    return (
      <div className="space-y-2 rounded-control border border-border bg-surface-2 p-3 text-xs">
        <div className="flex items-center gap-2 text-[13px] font-medium text-fg">
          <Spinner className="size-3.5" />
          {t('oauth.waiting')}
        </div>
        <p className="text-fg-muted">{t('oauth.waitingHint', { minutes: Math.round(flow.timeoutSecs / 60) })}</p>
        <p className="text-fg-muted">
          {t('oauth.redirectUri')}
          <span className="ml-1 font-mono text-fg" data-selectable>{flow.redirectUri}</span>
        </p>
        <div className="flex gap-2">
          <Button size="sm" variant="outline" onClick={() => void oauth.reopen()}><ExternalLink />{t('oauth.reopen')}</Button>
          <Button size="sm" variant="ghost" onClick={() => void oauth.cancel()}><Square />{t('oauth.cancel')}</Button>
        </div>
      </div>
    )
  }

  if (!session) {
    return (
      <div className="flex flex-wrap items-center gap-3 rounded-control border border-border bg-surface-2 p-3 text-xs">
        <KeyRound className="size-4 text-fg-subtle" />
        <span className="min-w-0 flex-1 text-fg-muted">{t('oauth.noSession')}</span>
        <Button size="sm" variant="primary" disabled={busy} onClick={() => void oauth.authorize(config)}><LogIn />{t('oauth.authorize')}</Button>
      </div>
    )
  }

  return (
    <div className="space-y-2 rounded-control border border-border bg-surface-2 p-3 text-xs">
      <div className="flex flex-wrap items-center gap-2">
        {session.expired ? (
          <Badge variant="warning"><TriangleAlert className="size-3" />{t('oauth.expired')}</Badge>
        ) : (
          <Badge variant="success"><ShieldCheck className="size-3" />{t('oauth.valid')}</Badge>
        )}
        <span className="text-fg-muted">{session.tokenType}</span>
        {session.scope && <span className="font-mono text-fg-muted">{session.scope}</span>}
      </div>
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
        <dt className="text-fg-subtle">{t('oauth.token')}</dt>
        <dd className="flex items-center gap-1 font-mono text-fg">
          {session.masked}
          <Button size="icon-sm" variant="ghost" onClick={() => void oauth.copyToken(config.id)} aria-label={t('oauth.copy')}><Copy /></Button>
        </dd>
        <dt className="text-fg-subtle">{t('oauth.expiresAt')}</dt>
        <dd className="text-fg">{session.expiresAt ? time(session.expiresAt) : t('oauth.noExpiry')}</dd>
        <dt className="text-fg-subtle">{t('oauth.obtainedAt')}</dt>
        <dd className="text-fg">{time(session.refreshedAt ?? session.obtainedAt)}</dd>
        <dt className="text-fg-subtle">{t('oauth.refreshToken')}</dt>
        <dd className="text-fg">
          {session.hasRefreshToken ? t('oauth.hasRefresh') : t('oauth.noRefresh')}
          {session.rotated && <span className="ml-2 text-fg-subtle">{t('oauth.rotated')}</span>}
        </dd>
      </dl>
      {session.expired && <p className="text-warning">{session.hasRefreshToken ? t('oauth.expiredHint') : t('oauth.expiredNoRefresh')}</p>}
      {session.lastError && <p className="text-danger">{t('oauth.refreshFailed', { reason: errorMessage({ code: session.lastError }) })}</p>}
      <div className="flex flex-wrap gap-2">
        {session.hasRefreshToken && (
          <Button size="sm" variant={session.expired ? 'primary' : 'outline'} loading={refreshing} disabled={busy} onClick={() => void oauth.refresh(config)}><RefreshCw />{t('oauth.refresh')}</Button>
        )}
        <Button size="sm" variant="outline" disabled={busy} onClick={() => void oauth.authorize(config)}><LogIn />{t('oauth.reauthorize')}</Button>
        <Button size="sm" variant="ghost" disabled={busy} onClick={() => void oauth.clear(config.id)}><Trash2 />{t('oauth.clear')}</Button>
      </div>
    </div>
  )
}
