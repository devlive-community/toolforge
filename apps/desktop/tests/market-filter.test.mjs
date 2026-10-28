import assert from 'node:assert/strict'
import test from 'node:test'
import { RECENT_DAYS, compareItems, countFilters, isNew, isRecentlyUpdated, matchesFilter, matchesQuery } from '../src/lib/marketFilter.ts'

const NOW = Date.parse('2026-10-28T12:00:00Z')
const daysAgo = (n) => new Date(NOW - n * 86_400_000).toISOString().replace(/\.\d{3}Z$/, 'Z')
const item = (over = {}) => ({
  id: 'org.x.a',
  name: 'Alpha',
  description: 'Draw things',
  category: 'image',
  publishedAt: daysAgo(100),
  updatedAt: daysAgo(100),
  installed: false,
  updatable: false,
  downloads: null,
  ...over,
})

test('new means first released within the recent window', () => {
  assert.equal(isNew(item({ publishedAt: daysAgo(3), updatedAt: daysAgo(3) }), NOW), true)
  assert.equal(isNew(item({ publishedAt: daysAgo(RECENT_DAYS + 1) }), NOW), false)
  assert.equal(isNew(item({ publishedAt: null }), NOW), false, '旧索引没有时间时不算新')
})

test('recently updated needs an older first release and a recent new version', () => {
  assert.equal(isRecentlyUpdated(item({ updatedAt: daysAgo(2) }), NOW), true)
  assert.equal(isRecentlyUpdated(item({ publishedAt: daysAgo(2), updatedAt: daysAgo(1) }), NOW), false, '新上架不重复算更新')
  assert.equal(isRecentlyUpdated(item(), NOW), false)
})

test('status filters follow installation state', () => {
  assert.equal(matchesFilter(item({ installed: true, updatable: true }), 'updatable', NOW), true)
  assert.equal(matchesFilter(item({ installed: true }), 'notInstalled', NOW), false)
  assert.equal(matchesFilter(item(), 'all', NOW), true)
})

test('query words must all match name, description or id', () => {
  assert.equal(matchesQuery(item(), 'alpha draw'), true)
  assert.equal(matchesQuery(item(), 'org.x'), true)
  assert.equal(matchesQuery(item(), 'alpha video'), false)
  assert.equal(matchesQuery(item(), '   '), true)
})

test('sorting by update time puts undated entries last and ties by name', () => {
  const list = [
    item({ id: 'b', name: 'Beta', updatedAt: daysAgo(1) }),
    item({ id: 'c', name: 'Gamma', publishedAt: null, updatedAt: null }),
    item({ id: 'a', name: 'Alpha', updatedAt: daysAgo(5) }),
    item({ id: 'd', name: 'Delta', updatedAt: daysAgo(1) }),
  ].sort(compareItems('updated', 'en'))
  assert.deepEqual(list.map((i) => i.id), ['b', 'd', 'a', 'c'])
})

test('sorting by downloads uses big integers and keeps unknown counts last', () => {
  const list = [item({ id: 'x', downloads: null }), item({ id: 'y', downloads: 5n }), item({ id: 'z', downloads: 12n })].sort(compareItems('downloads', 'en'))
  assert.deepEqual(list.map((i) => i.id), ['z', 'y', 'x'])
})

test('filter counts cover every option', () => {
  const counts = countFilters([item({ installed: true }), item({ publishedAt: daysAgo(1), updatedAt: daysAgo(1) })], NOW)
  assert.deepEqual(counts, { all: 2, new: 1, updated: 0, updatable: 0, installed: 1, notInstalled: 1 })
})
