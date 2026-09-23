import { Button, Tooltip, toast } from '@toolforge/ui'
import { host, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Copy } from 'lucide-react'

export function DigestRow({ label, digest }: { label: string; digest: string }) {
  const { t, errorMessage } = usePlugin()
  const copy = async () => {
    try {
      await host.clipboard.writeText(digest)
      toast.success(t('common:copied'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  return (
    <div className="group flex items-center gap-3 rounded-control px-3 py-2 hover:bg-hover">
      <span className="w-20 shrink-0 text-xs font-semibold text-fg-muted">{label}</span>
      <code className="min-w-0 flex-1 font-mono text-[12.5px] break-all text-fg" data-selectable>
        {digest}
      </code>
      <Tooltip content={t('common:copy')}>
        <Button variant="ghost" size="icon-sm" aria-label={t('common:copy')} onClick={copy} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100">
          <Copy />
        </Button>
      </Tooltip>
    </div>
  )
}
