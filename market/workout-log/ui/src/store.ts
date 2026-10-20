export interface Plan<T = unknown> { need: string[]; writes: Record<string, unknown>; data: T }
export interface Storage {
  read: (key: string) => Promise<unknown>
  write: (key: string, value: unknown) => Promise<void>
  call: <T>(fn: string, args: object) => Promise<T>
}
/** 仅持久化/恢复失败需要阻止新修改；原始错误留给界面本地化。 */
export class PersistenceError extends Error {
  constructor(cause: unknown) {
    super('Workout persistence failed', { cause })
    this.name = 'PersistenceError'
  }
}
export function errorCause(error: unknown): unknown {
  return error instanceof PersistenceError ? error.cause : error
}
async function persisted<T>(operation: () => Promise<T>): Promise<T> {
  try { return await operation() }
  catch (error) { throw error instanceof PersistenceError ? error : new PersistenceError(error) }
}
export function readState(storage: Storage, key: string): Promise<unknown> {
  return persisted(() => storage.read(key))
}
const queues = new Map<string, Promise<void>>()
export function serial<T>(key: string, work: () => Promise<T>): Promise<T> {
  const next = (queues.get(key) ?? Promise.resolve()).then(work)
  const settled = next.then(() => undefined, () => undefined)
  queues.set(key, settled)
  void settled.then(() => { if (queues.get(key) === settled) queues.delete(key) })
  return next
}
export async function recover(storage: Storage) {
  await persisted(async () => {
    const pending = await storage.read('pending')
    if (pending === null) return
    const checked = await storage.call<{ writes: Record<string, unknown> }>('recover', pending as object)
    for (const [key, value] of Object.entries(checked.writes)) await storage.write(key, value)
    await storage.write('pending', null)
  })
}
export async function apply(storage: Storage, plan: Plan) {
  if (Object.keys(plan.writes).length === 0) return
  await persisted(async () => {
    const checked = await storage.call<{ writes: Record<string, unknown> }>('recover', { writes: plan.writes })
    await storage.write('pending', checked)
    for (const [key, value] of Object.entries(checked.writes)) await storage.write(key, value)
    await storage.write('pending', null)
  })
}
export async function request<T>(storage: Storage, action: object, fn = 'plan', extra: object = {}): Promise<T> {
  const head = await readState(storage, 'head')
  const records: Record<string, unknown> = {}
  for (let pass = 0; pass < 8; pass++) {
    const plan = await storage.call<Plan<T>>(fn, { head, records, ...(fn === 'plan' ? { action } : extra) })
    if (plan.need.length === 0) { await apply(storage, plan); return plan.data }
    const keys = [...new Set(plan.need)]
    await Promise.all(keys.map(async (key) => { records[key] = await readState(storage, key) }))
  }
  throw { code: 'workout.readLimit', params: {} }
}
export async function transact<T>(key: string, storage: Storage, action: object): Promise<T> {
  return serial(key, async () => { await recover(storage); return request<T>(storage, action) })
}
