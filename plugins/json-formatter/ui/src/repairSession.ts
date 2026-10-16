import type { AppError } from '@toolforge/plugin-ui-sdk'
import type { Indent } from './types'

export interface RepairSnapshot {
  input: string
  indent: Indent
  sortKeys: boolean
}

export type RepairState<T> =
  | { status: 'idle' }
  | { status: 'pending'; sequence: number; snapshot: RepairSnapshot }
  | { status: 'success'; sequence: number; snapshot: RepairSnapshot; result: T }
  | { status: 'error'; sequence: number; snapshot: RepairSnapshot; error: AppError }

export function matchesRepairSnapshot(left: RepairSnapshot, right: RepairSnapshot) {
  return left.input === right.input && left.indent === right.indent && left.sortKeys === right.sortKeys
}

/** Only explicit submissions can publish a repair preview. Edits and navigation revoke it. */
export function createRepairSession<T>(publish: (state: RepairState<T>) => void) {
  let sequence = 0
  let alive = true
  const isCurrent = (candidate: number) => alive && sequence === candidate

  return {
    activate() { alive = true },
    close() { alive = false; sequence++ },
    invalidate() { sequence++; if (alive) publish({ status: 'idle' }) },
    isCurrent,
    async start(snapshot: RepairSnapshot, send: (snapshot: RepairSnapshot) => Promise<T>, errorOf: (error: unknown) => AppError) {
      if (!alive) return
      const submitted = { ...snapshot }
      const current = ++sequence
      publish({ status: 'pending', sequence: current, snapshot: submitted })
      try {
        const result = await send(submitted)
        if (isCurrent(current)) publish({ status: 'success', sequence: current, snapshot: submitted, result })
      } catch (error) {
        if (isCurrent(current)) publish({ status: 'error', sequence: current, snapshot: submitted, error: errorOf(error) })
      }
    },
  }
}
