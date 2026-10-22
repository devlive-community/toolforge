import assert from 'node:assert/strict'
import { test } from 'node:test'
import { formatDownloadCount, loadDownloadStats } from '../src/lib/pluginStats.ts'

test('download counters preserve BIGINT precision and distinguish unknown from zero', () => {
  assert.equal(formatDownloadCount('18446744073709551615', 'en-US'), '18,446,744,073,709,551,615')
  assert.equal(formatDownloadCount('0', 'en-US'), '0')
  assert.equal(formatDownloadCount(undefined, 'en-US'), null)
  assert.equal(formatDownloadCount('NaN', 'en-US'), null)
})

test('missing, numeric and unrelated counters do not become displayed zeroes', async () => {
  const stats = await loadDownloadStats(['a', 'b'], async () => ({ plugins: [
    { pluginId: 'a', totalDownloads: '0', versions: [{ version: '1.0.0', downloads: '0' }] },
    { pluginId: 'b', totalDownloads: 12, versions: [] },
    { pluginId: 'other', totalDownloads: '99', versions: [] },
  ] }))
  assert.equal(stats.get('a').totalDownloads, '0')
  assert.equal(stats.has('b'), false)
  assert.equal(stats.has('other'), false)
})

test('batch failures leave only their entries unknown and requests are deduplicated', async () => {
  const ids = Array.from({ length: 101 }, (_, index) => `plugin-${index}`)
  const batches = []
  const stats = await loadDownloadStats([...ids, ids[0]], async (batch) => {
    batches.push(batch)
    if (batch.length === 1) throw new Error('offline')
    return { plugins: batch.map((pluginId) => ({ pluginId, totalDownloads: '1', versions: [] })) }
  })
  assert.deepEqual(batches.map((batch) => batch.length), [100, 1])
  assert.equal(stats.size, 100)
  assert.equal(stats.has('plugin-100'), false)
})
