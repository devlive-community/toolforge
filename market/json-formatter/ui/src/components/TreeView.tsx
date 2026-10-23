import { useCallback, useEffect, useState } from 'react'
import { Button, Empty, Spinner, cn } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { ChevronRight, ListTree } from 'lucide-react'
import type { NodeKind, Segment, TreeNode, TreePage } from '../types'

const PAGE_SIZE = 200

const valueColor: Record<NodeKind, string> = {
  object: 'text-fg-subtle',
  array: 'text-fg-subtle',
  string: 'text-syntax-string',
  number: 'text-syntax-number',
  boolean: 'text-syntax-bool',
  null: 'text-syntax-null',
}

const isContainer = (kind: NodeKind) => kind === 'object' || kind === 'array'
const sizeLabel = (kind: NodeKind, size: number) => (kind === 'array' ? `[${size}]` : `{${size}}`)

interface Loaded {
  kind: NodeKind
  nodes: TreeNode[]
  total: number
}

/** 按需从 Rust 读取某一层的子节点，支持分页加载 */
function useChildren(docId: number, path: Segment[]) {
  const { call } = usePlugin()
  const [state, setState] = useState<Loaded | null>(null)
  const [loading, setLoading] = useState(false)

  const load = useCallback(
    async (offset: number) => {
      setLoading(true)
      try {
        const page = await call<TreePage>('tree_children', { docId, path, offset, limit: PAGE_SIZE })
        setState((prev) => ({
          kind: page.kind,
          nodes: offset === 0 ? page.nodes : [...(prev?.nodes ?? []), ...page.nodes],
          total: page.total,
        }))
      } finally {
        setLoading(false)
      }
    },
    // path 在同一节点生命周期内不变
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [call, docId],
  )

  return { state, loading, load }
}

function Children({ docId, path, depth }: { docId: number; path: Segment[]; depth: number }) {
  const children = useChildren(docId, path)
  const { load } = children

  useEffect(() => {
    load(0).catch(() => {})
  }, [load])

  return <NodeList {...children} docId={docId} path={path} depth={depth} />
}

function NodeList({
  state,
  loading,
  load,
  docId,
  path,
  depth,
}: ReturnType<typeof useChildren> & { docId: number; path: Segment[]; depth: number }) {
  const { t } = usePlugin()
  if (!state) {
    return (
      <div className="py-1" style={{ paddingLeft: depth * 16 + 24 }}>
        <Spinner className="size-3.5 text-fg-subtle" />
      </div>
    )
  }
  return (
    <>
      {state.nodes.map((node) => (
        <Row key={String(node.key)} docId={docId} node={node} path={[...path, node.key]} depth={depth} />
      ))}
      {state.nodes.length < state.total && (
        <div style={{ paddingLeft: depth * 16 + 20 }} className="py-0.5">
          <Button variant="ghost" size="sm" loading={loading} onClick={() => load(state.nodes.length)}>
            {t('tree.more', { count: state.total - state.nodes.length })}
          </Button>
        </div>
      )}
    </>
  )
}

function Row({ docId, node, path, depth }: { docId: number; node: TreeNode; path: Segment[]; depth: number }) {
  const [open, setOpen] = useState(depth < 1)
  const container = isContainer(node.kind)

  return (
    <>
      <div
        role="treeitem"
        aria-expanded={container ? open : undefined}
        tabIndex={-1}
        onClick={() => container && setOpen((v) => !v)}
        className={cn(
          'flex h-[22px] items-center gap-1.5 rounded-[5px] pr-2 font-mono text-[12.5px] whitespace-nowrap',
          container && 'cursor-pointer hover:bg-hover',
        )}
        style={{ paddingLeft: depth * 16 + 4 }}
      >
        <ChevronRight
          className={cn(
            'size-3.5 shrink-0 text-fg-subtle transition-transform duration-150',
            open && 'rotate-90',
            !container && 'invisible',
          )}
        />
        <span className="shrink-0 text-fg">{String(node.key)}</span>
        {container ? (
          <span className="text-fg-subtle">{sizeLabel(node.kind, node.size)}</span>
        ) : (
          <span className={cn('truncate', valueColor[node.kind])} data-selectable>
            {node.preview}
          </span>
        )}
      </div>
      {container && open && <Children docId={docId} path={path} depth={depth + 1} />}
    </>
  )
}

function Root({ docId }: { docId: number }) {
  const root = useChildren(docId, [])
  const { state, load } = root
  const [open, setOpen] = useState(true)

  useEffect(() => {
    load(0).catch(() => {})
  }, [load])

  if (!state) return <Spinner className="m-2 size-3.5 text-fg-subtle" />
  if (!isContainer(state.kind)) {
    return <p className="px-2 font-mono text-[12.5px] text-fg-muted">{state.kind}</p>
  }
  return (
    <>
      <div
        onClick={() => setOpen((v) => !v)}
        className="flex h-[22px] cursor-pointer items-center gap-1.5 rounded-[5px] pl-1 font-mono text-[12.5px] hover:bg-hover"
      >
        <ChevronRight className={cn('size-3.5 text-fg-subtle transition-transform duration-150', open && 'rotate-90')} />
        <span className="text-fg">{state.kind}</span>
        <span className="text-fg-subtle">{sizeLabel(state.kind, state.total)}</span>
      </div>
      {open && <NodeList {...root} docId={docId} path={[]} depth={1} />}
    </>
  )
}

export function TreeView({ docId }: { docId: number | null }) {
  const { t } = usePlugin()
  if (docId === null) {
    return <Empty icon={<ListTree />} title={t('tree.empty')} description={t('tree.emptyHint')} />
  }
  return (
    <div role="tree" className="h-full overflow-auto p-2">
      <Root key={docId} docId={docId} />
    </div>
  )
}
