export interface Diag {
  code: string
  params?: Record<string, unknown>
}

export interface PlanNode {
  id: number
  depth: number
  label: string
  nodeType: string
  relation: string | null
  indexName: string | null
  joinType: string | null
  filter: string | null
  indexCond: string | null
  hashCond: string | null
  joinFilter: string | null
  sortKey: string[]
  parallel: boolean
  startupCost: number | null
  totalCost: number | null
  planRows: number | null
  planWidth: number | null
  actualStartup: number | null
  actualTotal: number | null
  actualRows: number | null
  loops: number | null
  inclusiveMs: number | null
  exclusiveMs: number | null
  rowsRemovedByFilter: number | null
  tempWritten: number | null
  sharedHit: number | null
  sharedRead: number | null
  heapFetches: number | null
  notes: Diag[]
  children: PlanNode[]
}

export interface Statement {
  index: number
  planningTime: number | null
  executionTime: number | null
  statement: string | null
  hasActual: boolean
  root: PlanNode | null
  nodeCount: number
}

export interface Hotspot {
  statement: number
  nodeId: number
  label: string
  kind: 'time' | 'spill' | 'deviation'
  value: number
}

export interface Report {
  handle: number
  statements: Statement[]
  hotspots: Hotspot[]
  diagnostics: Diag[]
}

export interface Exported {
  content: string
  ext: string
}
