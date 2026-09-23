import type { Manifest } from '@toolforge/plugin-ui-sdk'
import { ToolTile } from '../plugins/ToolIcon'
import { usePluginText } from '../plugins/usePluginText'
import { useApp } from '../stores/app'

export function ToolCard({ manifest }: { manifest: Manifest }) {
  const text = usePluginText()
  const openTool = useApp((s) => s.openTool)
  return (
    <button
      type="button"
      onClick={() => openTool(manifest.id)}
      className="group flex items-start gap-3 rounded-card border border-border bg-surface p-4 text-left shadow-card outline-none transition-all duration-150 hover:-translate-y-0.5 hover:border-border-strong hover:shadow-popover focus-visible:ring-2 focus-visible:ring-ring"
    >
      <ToolTile manifest={manifest} />
      <div className="min-w-0">
        <p className="truncate text-sm font-semibold text-fg">{text(manifest, manifest.name)}</p>
        <p className="mt-1 line-clamp-2 text-xs leading-relaxed text-fg-muted">
          {text(manifest, manifest.description)}
        </p>
      </div>
    </button>
  )
}

export function ToolGrid({ plugins }: { plugins: Manifest[] }) {
  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(260px,1fr))] gap-3">
      {plugins.map((manifest) => (
        <ToolCard key={manifest.id} manifest={manifest} />
      ))}
    </div>
  )
}
