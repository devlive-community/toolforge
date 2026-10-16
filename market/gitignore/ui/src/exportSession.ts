/** Edits and unmounts revoke pending exports, including an open native save dialog. */
export function createExportSession() {
  let revision = 0
  let alive = true
  return {
    activate() { alive = true },
    close() { alive = false; revision++ },
    invalidate() { revision++ },
    capture() {
      const captured = revision
      return () => alive && captured === revision
    },
  }
}

export async function saveCurrentOutput(
  output: string,
  isCurrent: () => boolean,
  choosePath: () => Promise<string | null>,
  write: (path: string, output: string) => Promise<void>,
): Promise<'saved' | 'cancelled' | 'changed'> {
  if (!isCurrent()) return 'changed'
  const path = await choosePath()
  if (!path) return 'cancelled'
  if (!isCurrent()) return 'changed'
  await write(path, output)
  return 'saved'
}
