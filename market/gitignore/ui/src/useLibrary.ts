import { useEffect, useState } from 'react'
import { loadPluginState, savePluginState, toAppError, usePlugin, type AppError } from '@toolforge/plugin-ui-sdk'
import { createExportSession } from './exportSession'
import { loadLibrary, mutateLibrary } from './libraryStore'
import type { Library, LibraryMutation, LibraryResult } from './types'

const STATE_KEY = 'custom-templates'

function createMutationGate() {
  let busy = false
  return {
    enter() { if (busy) return false; busy = true; return true },
    leave() { busy = false },
  }
}

export function useLibrary() {
  const { manifest, call } = usePlugin()
  const [state, setState] = useState<{ library?: Library; loading: boolean; error: AppError | null }>({ loading: true, error: null })
  const [reload, setReload] = useState(0)
  const [pending, setPending] = useState(false)
  const [lifetime] = useState(createExportSession)
  const [gate] = useState(createMutationGate)
  const key = `${manifest.id}:${STATE_KEY}`

  useEffect(() => {
    lifetime.activate()
    return () => lifetime.close()
  }, [lifetime])

  useEffect(() => {
    let alive = true
    void loadLibrary(
      key,
      () => loadPluginState<unknown>(manifest.id, STATE_KEY),
      (saved) => call<LibraryResult>('library', { operation: 'validate', library: saved ?? undefined }),
    ).then(
      (result) => { if (alive) setState({ library: result.library, loading: false, error: null }) },
      (reason) => { if (alive) setState({ loading: false, error: toAppError(reason) }) },
    )
    return () => { alive = false }
  }, [call, key, manifest.id, reload])

  const retry = () => {
    setState({ loading: true, error: null })
    setReload((value) => value + 1)
  }

  const mutate = async (operation: LibraryMutation) => {
    if (!state.library || state.loading || state.error || !gate.enter()) return null
    const isMounted = lifetime.capture()
    setPending(true)
    try {
      const result = await mutateLibrary(
        key,
        () => loadPluginState<unknown>(manifest.id, STATE_KEY),
        (saved) => call<LibraryResult>('library', { ...operation, library: saved ?? undefined }),
        (library) => savePluginState(manifest.id, STATE_KEY, library),
      )
      if (!isMounted()) return null
      setState({ library: result.library, loading: false, error: null })
      return result
    } finally {
      gate.leave()
      if (isMounted()) setPending(false)
    }
  }

  return { ...state, pending, retry, mutate }
}
