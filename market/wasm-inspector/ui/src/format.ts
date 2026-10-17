export const hex = (n: number) => `0x${n.toString(16)}`

export const percent = (part: number, total: number) => (total > 0 ? `${((part / total) * 100).toFixed(1)}%` : '—')

/** 函数的显示名：name section → 导出名 → 索引 */
export const functionLabel = (f: { index: number; name: string | null; export: string | null }) =>
  f.name ?? f.export ?? `func[${f.index}]`
