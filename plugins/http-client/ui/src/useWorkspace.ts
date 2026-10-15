import { useCallback, useEffect, useEffectEvent, useRef, useState } from 'react'
import { loadPluginState, savePluginState, usePlugin } from '@toolforge/plugin-ui-sdk'
import { toast } from '@toolforge/ui'
import { newRequest, type Environment, type Request, type Workspace } from './types'
import { createPersistenceCoordinator, createWorkspacePersistence, type SaveStatus } from './workspacePersistence'

const STATE_KEY = 'workspace-v1'
// A remounted tool waits for the previous instance's final write before loading.
const persistence = createPersistenceCoordinator()
const emptyWorkspace = (): Workspace => ({ version: 1, collections: [], requests: [], environments: [], activeEnvironmentId: null })
interface Model { workspace: Workspace; selectedId: string | null; generation: number; loaded: boolean; busy: boolean; saveStatus: SaveStatus; saveError: string | null }

/** One owner for request data and session secrets. Only Rust's sanitized copy reaches SQLite. */
export function useWorkspace() {
  const { manifest, call, errorMessage, t } = usePlugin()
  const [model, setModel] = useState<Model>({ workspace: emptyWorkspace(), selectedId: null, generation: 0, loaded: false, busy: false, saveStatus: 'saved', saveError: null })
  const current = useRef(model)
  const mounted = useRef(true)
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const launchInput = useRef<string | null>(null)

  const update = useCallback((patch: Partial<Model>) => {
    current.current = { ...current.current, ...patch }
    if (mounted.current) setModel(current.current)
  }, [])

  const [queue] = useState(() => createWorkspacePersistence<Workspace, Workspace>(persistence, manifest.id, {
    validate: (workspace) => call<Workspace>('workspace_validate', { workspace }),
    persist: (sanitized) => savePluginState(manifest.id, STATE_KEY, sanitized),
    // Attach the state subscription in the effect, outside the render initializer.
    onStatus: () => {},
  }))

  useEffect(() => {
    queue.configure({
      validate: (workspace) => call<Workspace>('workspace_validate', { workspace }),
      persist: (sanitized) => savePluginState(manifest.id, STATE_KEY, sanitized),
      onStatus: (saveStatus, error) => update({ saveStatus, saveError: error ? errorMessage(error) : null }),
    })
  }, [queue, call, manifest.id, errorMessage, update])

  const flush = useCallback(() => {
    clearTimeout(timer.current)
    return queue.flush()
  }, [queue])

  const commit = useCallback((workspace: Workspace, patch: Partial<Model> = {}) => {
    queue.stage(workspace)
    update({ workspace, saveStatus: 'pending', saveError: null, ...patch })
    clearTimeout(timer.current)
    timer.current = setTimeout(() => void flush(), 400)
  }, [flush, queue, update])

  const loadWorkspace = useEffectEvent(async (alive: () => boolean) => {
      try {
        await persistence.wait(manifest.id)
        if (!alive()) return
        const saved = await loadPluginState<Workspace>(manifest.id, STATE_KEY)
        let workspace = saved ? await call<Workspace>('workspace_validate', { workspace: saved }) : emptyWorkspace()
        if (!alive()) return
        let selectedId = workspace.requests[0]?.id ?? null
        if (!selectedId || launchInput.current !== null) {
          const id = await call<string>('new_id')
          if (!alive()) return
          const request = newRequest()
          if (launchInput.current !== null) request.url = launchInput.current
          launchInput.current = null
          workspace = { ...workspace, requests: [...workspace.requests, { id, name: t('workspace.untitled'), collectionId: null, request }] }
          selectedId = id
          commit(workspace, { selectedId, loaded: true })
        } else update({ workspace, selectedId, loaded: true })
      } catch (error) {
        if (alive()) update({ loaded: true, saveStatus: 'error', saveError: errorMessage(error) })
      }
  })

  useEffect(() => {
    let alive = true
    void loadWorkspace(() => alive)
    return () => { alive = false }
  }, [manifest.id])

  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
      clearTimeout(timer.current)
      void queue.close()
    }
  }, [queue])

  const operate = async (action: () => Promise<void>) => {
    if (current.current.busy || !current.current.loaded) return false
    update({ busy: true })
    const running = queue.run(action)
    try { await running; return true } catch (error) { toast.error(errorMessage(error)); return false }
    finally { update({ busy: false }) }
  }

  const createRequest = (request = newRequest(), name = t('workspace.untitled'), collectionId: string | null = null) => operate(async () => {
    const id = await call<string>('new_id')
    const state = current.current
    commit({ ...state.workspace, requests: [...state.workspace.requests, { id, name, collectionId, request }] }, { selectedId: id, generation: state.generation + 1 })
  })

  const receiveLaunchInput = (url: string) => {
    if (!current.current.loaded) launchInput.current = url
    else void createRequest({ ...newRequest(), url })
  }

  const selected = model.workspace.requests.find((item) => item.id === model.selectedId) ?? null
  const editRequest = (request: Request) => {
    const state = current.current
    commit({ ...state.workspace, requests: state.workspace.requests.map((item) => item.id === state.selectedId ? { ...item, request } : item) })
  }
  const selectRequest = (selectedId: string | null) => update({ selectedId, generation: current.current.generation + 1 })
  const editDetails = (id: string, name: string, collectionId: string | null) => {
    const workspace = current.current.workspace
    commit({ ...workspace, requests: workspace.requests.map((item) => item.id === id ? { ...item, name, collectionId } : item) })
  }
  const duplicateRequest = () => operate(async () => {
    const source = current.current.workspace.requests.find((item) => item.id === current.current.selectedId)
    if (!source) return
    const id = await call<string>('new_id')
    const state = current.current
    commit({ ...state.workspace, requests: [...state.workspace.requests, { ...source, id, name: t('workspace.copyName', { name: source.name }) }] }, { selectedId: id, generation: state.generation + 1 })
  })
  const deleteRequest = () => {
    const state = current.current
    const requests = state.workspace.requests.filter((item) => item.id !== state.selectedId)
    commit({ ...state.workspace, requests }, { selectedId: requests[0]?.id ?? null, generation: state.generation + 1 })
  }
  const saveCollection = (id: string | null, name: string) => operate(async () => {
    const key = id ?? await call<string>('new_id')
    const workspace = current.current.workspace
    commit({ ...workspace, collections: id ? workspace.collections.map((item) => item.id === id ? { ...item, name } : item) : [...workspace.collections, { id: key, name }] })
  })
  const deleteCollection = (id: string) => {
    const workspace = current.current.workspace
    commit({ ...workspace, collections: workspace.collections.filter((item) => item.id !== id), requests: workspace.requests.map((item) => item.collectionId === id ? { ...item, collectionId: null } : item) })
  }
  const createEnvironment = (name: string) => operate(async () => {
    const id = await call<string>('new_id')
    const workspace = current.current.workspace
    commit({ ...workspace, environments: [...workspace.environments, { id, name, variables: [] }], activeEnvironmentId: id })
  })
  const editEnvironment = (environment: Environment) => {
    const workspace = current.current.workspace
    commit({ ...workspace, environments: workspace.environments.map((item) => item.id === environment.id ? environment : item) })
  }
  const deleteEnvironment = (id: string) => {
    const workspace = current.current.workspace
    commit({ ...workspace, environments: workspace.environments.filter((item) => item.id !== id), activeEnvironmentId: workspace.activeEnvironmentId === id ? null : workspace.activeEnvironmentId })
  }
  const selectEnvironment = (id: string | null) => commit({ ...current.current.workspace, activeEnvironmentId: id })

  return { ...model, selected, editRequest, createRequest, selectRequest, editDetails, duplicateRequest, deleteRequest, saveCollection, deleteCollection, createEnvironment, editEnvironment, deleteEnvironment, selectEnvironment, receiveLaunchInput, flush }
}

export type WorkspaceController = ReturnType<typeof useWorkspace>
