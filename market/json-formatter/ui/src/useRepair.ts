import { useEffect, useState } from 'react'
import { toAppError, usePlugin } from '@toolforge/plugin-ui-sdk'
import { createRepairSession, matchesRepairSnapshot, type RepairSnapshot, type RepairState } from './repairSession'
import type { RepairResult } from './types'

export function useRepair(active: boolean, snapshot: RepairSnapshot) {
  const { call } = usePlugin()
  const [state, setState] = useState<RepairState<RepairResult>>({ status: 'idle' })
  const [session] = useState(() => createRepairSession<RepairResult>(setState))
  useEffect(() => {
    session.activate()
    return () => session.close()
  }, [session])

  const current = active && state.status !== 'idle' && matchesRepairSnapshot(state.snapshot, snapshot) ? state : null
  return {
    pending: current?.status === 'pending',
    result: current?.status === 'success' ? current.result : null,
    error: current?.status === 'error' ? current.error : null,
    invalidate: session.invalidate,
    isCurrent: () => current !== null && session.isCurrent(current.sequence),
    start: () => session.start(snapshot, (args) => call<RepairResult>('repair', args), toAppError),
  }
}
