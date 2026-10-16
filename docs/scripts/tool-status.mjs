// Compare complete plugin trees so code, UI, assets and deleted files all count,
// even when the plugin's manifest version was not bumped for an app release.
function pluginTreesAt(ref, git) {
  const trees = new Map()
  for (const entry of git(['ls-tree', '-d', '-z', ref, 'plugins/']).split('\0')) {
    const match = entry.match(/^040000 tree ([0-9a-f]+)\tplugins\/([^/]+)$/)
    if (match) trees.set(match[2], match[1])
  }
  return trees
}

export function collectToolReleaseStatus({ previousRef, latestRef, currentRef = 'HEAD', git }) {
  const previous = pluginTreesAt(previousRef, git)
  const latest = pluginTreesAt(latestRef, git)
  const current = pluginTreesAt(currentRef, git)
  const updatedSet = new Set()
  const pendingSet = new Set()
  for (const [slug, tree] of latest) {
    if (previous.has(slug) && previous.get(slug) !== tree) updatedSet.add(slug)
    if (current.has(slug) && current.get(slug) !== tree) pendingSet.add(slug)
  }
  return {
    prevTag: previousRef,
    latestTag: latestRef,
    prevSet: new Set(previous.keys()),
    latestSet: new Set(latest.keys()),
    updatedSet,
    pendingSet,
  }
}

export function toolReleaseStatus(slug, sets) {
  if (!sets) return { status: 'released' }
  const updateOnly = sets.latestSet.has(slug) && sets.pendingSet.has(slug)
  const status = updateOnly || !sets.latestSet.has(slug)
    ? 'upcoming'
    : !sets.prevSet.has(slug)
      ? 'new'
      : sets.updatedSet.has(slug)
        ? 'updated'
        : 'released'
  return { status, ...(updateOnly ? { updateOnly: true } : {}) }
}
