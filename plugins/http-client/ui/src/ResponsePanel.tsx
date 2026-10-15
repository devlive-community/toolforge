import { useState } from 'react'
import { Badge, Button, CodeEditor, Empty, Panel, Spinner, Tabs, toast } from '@toolforge/ui'
import { CopyButton, formatBytes, host, usePlugin, type TaskState } from '@toolforge/plugin-ui-sdk'
import { Download, FileWarning, Info, Upload } from 'lucide-react'
import type { Reply } from './types'

type ResponseTab = 'body' | 'headers'
function statusTone(status: number) {
  if (status < 300) return 'success' as const
  if (status < 400) return 'warning' as const
  return 'danger' as const
}
export function ResponsePanel({ task }: { task: TaskState<Reply> & { running: boolean } }) {
  const { t, call, errorMessage } = usePlugin()
  const [responseTab, setResponseTab] = useState<ResponseTab>('body')
  const reply = task.result
  const saveBody = async () => {
    if (!reply) return
    try {
      const name = reply.url.split('?')[0].split('/').filter(Boolean).pop() || 'response'
      const path = await host.dialog.saveFile(reply.kind === 'json' && !name.includes('.') ? `${name}.json` : name)
      if (!path) return
      await call('save_body', { id: reply.id, path })
      toast.success(t('common:saved'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const language = reply?.kind === 'json' ? 'json' : reply?.contentType && /xml|html/i.test(reply.contentType) ? 'xml' : 'text'

  return (
        <Panel
          title={
            <Tabs<ResponseTab>
              variant="underline"
              value={responseTab}
              onValueChange={setResponseTab}
              aria-label={t('response.title')}
              items={[
                { value: 'body', label: t('response.body') },
                { value: 'headers', label: t('response.headers', { count: reply?.headers.length ?? 0 }) },
              ]}
            />
          }
          extra={task.running && <Spinner className="size-3.5 text-fg-subtle" />}
          actions={
            reply && (
              <>
                <Badge variant={statusTone(reply.status)}>{`${reply.status} ${reply.statusText}`}</Badge>
                <span className="text-xs text-fg-muted tabular-nums">{t('response.meta', { ms: reply.elapsedMs, size: formatBytes(reply.size) })}</span>
                {reply.kind !== 'binary' && <CopyButton text={reply.body} />}
                <Button size="icon-sm" variant="ghost" aria-label={t('response.save')} onClick={saveBody}>
                  <Download />
                </Button>
              </>
            )
          }
          footer={
            reply && (
              <>
                <span className="truncate" title={reply.url}>
                  {reply.url}
                </span>
                {reply.redirects.length > 0 && <span className="shrink-0">{t('response.redirects', { count: reply.redirects.length })}</span>}
                <span className="ml-auto shrink-0">{reply.version}</span>
              </>
            )
          }
          bodyClassName="flex min-h-0 flex-col"
        >
          {task.status === 'failed' && task.error ? (
            <div className="m-auto max-w-md space-y-1 p-6 text-center">
              <FileWarning className="mx-auto size-6 text-danger" />
              <p className="text-[13px] text-danger">{errorMessage(task.error)}</p>
            </div>
          ) : !reply ? (
            <Empty icon={task.running ? <Spinner /> : <Upload />} title={task.running ? t('response.waiting') : t('response.empty')} description={task.running ? undefined : t('response.emptyHint')} />
          ) : responseTab === 'headers' ? (
            <table className="w-full text-left text-xs">
              <tbody className="font-mono">
                {reply.headers.map(([name, value], index) => (
                  <tr key={`${name}-${index}`} className="border-b border-border align-top">
                    <td className="w-1/3 px-3 py-2 text-fg-muted" data-selectable>
                      {name}
                    </td>
                    <td className="px-3 py-2 break-all text-fg" data-selectable>
                      {value}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : reply.kind === 'binary' ? (
            <Empty
              icon={<Download />}
              title={t('response.binary', { size: formatBytes(reply.size) })}
              description={reply.contentType ?? undefined}
              action={
                <Button onClick={saveBody}>
                  <Download />
                  {t('response.save')}
                </Button>
              }
            />
          ) : (
            <>
              {reply.truncated && (
                <p className="flex shrink-0 items-center gap-2 border-b border-border bg-warning-soft/60 px-3 py-2 text-xs text-warning">
                  <Info className="size-3.5" />
                  {t('response.truncated', { size: formatBytes(reply.size) })}
                </p>
              )}
              <div className="min-h-0 flex-1">
                <CodeEditor value={reply.body} readOnly language={language} lineWrapping aria-label={t('response.body')} />
              </div>
            </>
          )}
        </Panel>
  )
}
