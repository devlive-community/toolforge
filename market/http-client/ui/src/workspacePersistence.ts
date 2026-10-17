export type SaveStatus = 'saved' | 'pending' | 'saving' | 'error'

/** Serializes storage access across successive mounts of the same tool. */
export function createPersistenceCoordinator() {
  const tails = new Map<string, Promise<void>>()
  const enqueue = (key: string, action: () => Promise<void>) => {
    const queued = (tails.get(key) ?? Promise.resolve()).catch(() => undefined).then(action)
    tails.set(key, queued)
    const clear = () => { if (tails.get(key) === queued) tails.delete(key) }
    void queued.then(clear, clear)
    return queued
  }
  const wait = async (key: string) => {
    // A cleanup barrier can extend an already active write before it finishes.
    while (tails.has(key)) await tails.get(key)
  }
  return { enqueue, wait }
}

type Coordinator = ReturnType<typeof createPersistenceCoordinator>
interface Callbacks<Value, Sanitized> {
  validate: (value: Value) => Promise<Sanitized>
  persist: (value: Sanitized) => Promise<void>
  onStatus: (status: SaveStatus, error?: unknown) => void
}

/**
 * Owns pending snapshots and asynchronous mutations for one mounted workspace.
 * Mutations stage data, never await a flush: closing waits for those mutations.
 */
export function createWorkspacePersistence<Value, Sanitized>(coordinator: Coordinator, key: string, initialCallbacks: Callbacks<Value, Sanitized>) {
  let callbacks = initialCallbacks
  let revision = 0
  let pending: { value: Value; revision: number } | null = null
  let writer: Promise<void> | null = null
  let closing: Promise<void> | null = null
  const operations = new Set<Promise<unknown>>()

  const stage = (value: Value) => {
    pending = { value, revision: ++revision }
    callbacks.onStatus('pending')
  }

  const drain = async (): Promise<boolean> => {
    while (pending) {
      const snapshot = pending
      pending = null
      callbacks.onStatus('saving')
      try {
        const sanitized = await callbacks.validate(snapshot.value)
        if (snapshot.revision !== revision) continue
        await callbacks.persist(sanitized)
        if (snapshot.revision === revision) callbacks.onStatus('saved')
      } catch (error) {
        if (snapshot.revision === revision) {
          pending = snapshot
          callbacks.onStatus('error', error)
          return false
        }
      }
    }
    return true
  }

  const flush = (): Promise<void> => {
    if (closing) return closing
    if (writer) return writer
    const queued = coordinator.enqueue(key, async () => { await drain() })
    writer = queued
    const clear = () => { if (writer === queued) writer = null }
    void queued.then(clear, clear)
    return queued
  }

  const run = <Result,>(action: () => Promise<Result>): Promise<Result> => {
    // Register before invoking the action, including when new_id is still pending.
    const operation = Promise.resolve().then(action)
    operations.add(operation)
    const clear = () => { operations.delete(operation) }
    void operation.then(clear, clear)
    return operation
  }

  const close = (): Promise<void> => {
    if (closing) return closing
    // Register synchronously, even if a writer already exists. New mounts must
    // await both that writer and every mutation/pending snapshot left behind.
    const barrier = coordinator.enqueue(key, async () => {
      let drained: boolean
      do {
        await Promise.allSettled([...operations])
        // Call the private drain directly: flush() would await this very barrier.
        drained = await drain()
      } while (operations.size > 0 || (drained && pending !== null))
    })
    closing = barrier
    const clear = () => { if (closing === barrier) closing = null }
    void barrier.then(clear, clear)
    return barrier
  }

  return { stage, flush, run, close, configure: (next: Callbacks<Value, Sanitized>) => { callbacks = next } }
}
