import { useCallback, useEffect, useRef, useState } from 'react'
import { cancelTask, host, startTask, usePlugin, type TaskEvent } from '@toolforge/plugin-ui-sdk'
import { toast } from '@toolforge/ui'
import type { OAuthConfig, OAuthFlow, SessionView, Variable } from './types'

/** 会话按「配置 + 环境」隔离，与后端 session_key 相同 */
export const sessionKey = (configId: string, environmentId: string | null) => `${configId}@${environmentId ?? '-'}`

/** 刷新会话状态（是否过期）的间隔 */
const POLL = 15_000

/**
 * 授权会话的唯一持有者：令牌只在后端内存中，这里只有遮罩后的状态。
 * 获取 / 刷新只改变授权状态，不会发送接口请求。
 */
export function useOAuth(configs: OAuthConfig[], environmentId: string | null, variables: Variable[]) {
  const { manifest, call, errorMessage, t } = usePlugin()
  const [sessions, setSessions] = useState<Record<string, SessionView>>({})
  const [flow, setFlow] = useState<OAuthFlow | null>(null)
  const [refreshingKey, setRefreshingKey] = useState<string | null>(null)
  const waitTask = useRef<string | null>(null)
  const flowRef = useRef<OAuthFlow | null>(null)
  const keys = configs.map((config) => sessionKey(config.id, environmentId)).join('\n')

  const reload = useCallback(async () => {
    const list = keys ? keys.split('\n') : []
    try {
      const views = await call<SessionView[]>('oauth_sessions', { keys: list })
      setSessions(Object.fromEntries(views.map((view) => [view.key, view])))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }, [call, errorMessage, keys])

  useEffect(() => {
    const first = setTimeout(() => void reload(), 0)
    const timer = setInterval(() => void reload(), POLL)
    return () => {
      clearTimeout(first)
      clearInterval(timer)
    }
  }, [reload])

  // 关闭工具时结束未完成的授权，释放回调端口
  useEffect(() => () => {
    const pending = flowRef.current
    if (pending) void call('oauth_cancel', { flow: pending.flow }).catch(() => {})
  }, [call])

  const updateFlow = (next: OAuthFlow | null) => {
    flowRef.current = next
    setFlow(next)
  }

  const authorize = async (config: OAuthConfig) => {
    if (flowRef.current) return
    try {
      const begun = await call<Omit<OAuthFlow, 'configId'>>('oauth_begin', { config, variables, environmentId })
      updateFlow({ ...begun, configId: config.id })
      // 授权结束（成功、失败、取消）都结束这次会话
      const onEvent = (event: TaskEvent) => {
        if (event.type !== 'finished') return
        if (event.status === 'succeeded') {
          const view = event.result as SessionView
          setSessions((prev) => ({ ...prev, [view.key]: view }))
          toast.success(t('oauth.obtained'))
        } else if (event.status === 'failed' && event.error) {
          toast.error(errorMessage(event.error))
        }
        waitTask.current = null
        updateFlow(null)
      }
      waitTask.current = await startTask(manifest.id, 'oauth_wait', { flow: begun.flow }, onEvent)
      await host.openUrl(begun.authorizeUrl)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const reopen = async () => {
    if (!flow) return
    try { await host.openUrl(flow.authorizeUrl) } catch (error) { toast.error(errorMessage(error)) }
  }

  const cancel = async () => {
    const pending = flowRef.current
    if (waitTask.current) await cancelTask(waitTask.current).catch(() => {})
    if (pending) await call('oauth_cancel', { flow: pending.flow }).catch(() => {})
    updateFlow(null)
  }

  const refresh = async (config: OAuthConfig) => {
    if (refreshingKey) return
    setRefreshingKey(sessionKey(config.id, environmentId))
    const onEvent = (event: TaskEvent) => {
      if (event.type !== 'finished') return
      if (event.status === 'succeeded') toast.success(t('oauth.refreshed'))
      else if (event.status === 'failed' && event.error) toast.error(errorMessage(event.error))
      setRefreshingKey(null)
      void reload()
    }
    try {
      await startTask(manifest.id, 'oauth_refresh', { config, variables, environmentId }, onEvent)
    } catch (error) {
      setRefreshingKey(null)
      toast.error(errorMessage(error))
    }
  }

  const clear = async (configId: string) => {
    try {
      await call('oauth_clear', { configId, environmentId })
      const key = sessionKey(configId, environmentId)
      setSessions((prev) => Object.fromEntries(Object.entries(prev).filter(([k]) => k !== key)))
      toast.success(t('oauth.cleared'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const copyToken = async (configId: string) => {
    try {
      const token = await call<string>('oauth_reveal', { configId, environmentId })
      await host.clipboard.writeText(token)
      toast.success(t('oauth.copied'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const session = (configId: string) => sessions[sessionKey(configId, environmentId)] ?? null

  return {
    session,
    flow,
    authorizing: flow !== null,
    refreshingKey,
    environmentId,
    authorize,
    reopen,
    cancel,
    refresh,
    clear,
    copyToken,
  }
}

export type OAuthController = ReturnType<typeof useOAuth>
