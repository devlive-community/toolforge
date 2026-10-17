import { useCallback, useEffect, useEffectEvent, useRef, useState } from 'react'
import { Button, Empty, Spinner } from '@toolforge/ui'
import { useLaunchInput, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Plus } from 'lucide-react'
import { CollectionsModal } from './CollectionsModal'
import { CurlImportModal } from './CurlImportModal'
import { EnvironmentsModal } from './EnvironmentsModal'
import { OAuthModal } from './OAuthModal'
import { AUTH_NONE } from './types'
import { useOAuth } from './useOAuth'
import { RequestEditor } from './RequestEditor'
import { WorkspaceToolbar } from './WorkspaceToolbar'
import { useWorkspace } from './useWorkspace'

export function HttpClient() {
  const { t } = usePlugin()
  const workspace = useWorkspace()
  const [running, setRunning] = useState(false)
  const [dialog, setDialog] = useState<'collections' | 'environments' | 'import' | 'oauth' | null>(null)
  const waitingLaunch = useRef<string | null>(null)
  useLaunchInput((text) => {
    if (running || workspace.busy) waitingLaunch.current = text
    else workspace.receiveLaunchInput(text)
  })
  const openWaitingLaunch = useEffectEvent(() => {
    if (waitingLaunch.current !== null) {
      const text = waitingLaunch.current
      waitingLaunch.current = null
      workspace.receiveLaunchInput(text)
    }
  })
  useEffect(() => { if (!running && !workspace.busy) openWaitingLaunch() }, [running, workspace.busy])
  const onRunningChange = useCallback((value: boolean) => setRunning(value), [])
  const environment = workspace.workspace.environments.find((item) => item.id === workspace.workspace.activeEnvironmentId)
  const configs = workspace.workspace.oauthConfigs ?? []
  const oauth = useOAuth(configs, environment?.id ?? null, environment?.variables ?? [])
  const selected = workspace.selected
  const collection = workspace.workspace.collections.find((item) => item.id === selected?.collectionId) ?? null
  // 请求自身的设置优先，未设置时沿用集合
  const effectiveId = selected?.auth === AUTH_NONE ? null : selected?.auth ?? collection?.auth ?? null
  const effective = configs.find((item) => item.id === effectiveId) ?? null
  const auth = {
    request: selected?.auth ?? null,
    collection,
    configs,
    effective,
    oauth,
    onRequestChange: workspace.setRequestAuth,
    onCollectionChange: workspace.setCollectionAuth,
    onManage: () => setDialog('oauth'),
  }
  return <div className="flex h-full min-h-0 flex-col gap-3">
    <WorkspaceToolbar controller={workspace} running={running} onCollections={() => setDialog('collections')} onEnvironments={() => setDialog('environments')} />
    {!workspace.loaded ? <Empty icon={<Spinner />} title={t('workspace.loading')} /> : workspace.selected ? <RequestEditor key={workspace.generation} request={workspace.selected.request} variables={environment?.variables ?? []} auth={auth} session={effective ? { configId: effective.id, environmentId: environment?.id ?? null } : null} onChange={workspace.editRequest} onRunningChange={onRunningChange} onImport={() => setDialog('import')} disabled={workspace.busy} /> : <Empty icon={<Plus />} title={t('workspace.empty')} action={<Button onClick={() => void workspace.createRequest()}><Plus />{t('workspace.new')}</Button>} />}
    <CollectionsModal open={dialog === 'collections'} onClose={() => setDialog(null)} controller={workspace} />
    <EnvironmentsModal open={dialog === 'environments'} onClose={() => setDialog(null)} controller={workspace} />
    <OAuthModal open={dialog === 'oauth'} onClose={() => setDialog(null)} controller={workspace} oauth={oauth} />
    <CurlImportModal open={dialog === 'import'} onClose={() => setDialog(null)} onImport={(request) => workspace.createRequest(request, t('import.requestName'))} />
  </div>
}
