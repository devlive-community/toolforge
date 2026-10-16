/** Shared by remounts. Every transaction reads the latest durable value inside the queue. */
const transactions = new Map<string, Promise<void>>()

function serialize<T>(key: string, action: () => Promise<T>): Promise<T> {
  const previous = transactions.get(key) ?? Promise.resolve()
  const next = previous.then(action)
  // A failed write must not poison subsequent retries or reads.
  const settled = next.then(() => undefined, () => undefined)
  transactions.set(key, settled)
  void settled.then(() => {
    if (transactions.get(key) === settled) transactions.delete(key)
  })
  return next
}

export function loadLibrary<T>(key: string, read: () => Promise<unknown>, validate: (saved: unknown) => Promise<T>): Promise<T> {
  return serialize(key, async () => validate(await read()))
}

export function mutateLibrary<T extends { library: unknown }>(
  key: string,
  read: () => Promise<unknown>,
  mutate: (saved: unknown) => Promise<T>,
  write: (library: T['library']) => Promise<void>,
): Promise<T> {
  return serialize(key, async () => {
    const result = await mutate(await read())
    await write(result.library)
    return result
  })
}
