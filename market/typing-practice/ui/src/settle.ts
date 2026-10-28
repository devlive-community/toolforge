/** 结算输入：至少包含是否粘贴；其余成绩字段原样透传给界面。 */
export interface Outcome {
  pasted: boolean
}

/** 结算结果：原始成绩加上 `record`，为 false 时本局只作练习，不写入正式纪录。 */
export type Settlement<T extends Outcome> = T & { record: boolean }

/** 易错字符统计最多保留的字符种类（超出时丢弃次数最少的） */
export const MAX_MISTAKES = 200

/**
 * 粘贴标记是粘性的：本局只要出现过一次粘贴，之后即使继续手打也仍是练习局。
 */
export function pasteFlag(current: boolean, incoming: boolean): boolean {
  return current || incoming
}

/**
 * 结算器：一局只结算一次。异步提交、挑战到点与完成全文可能同时报告结束，
 * 这里保证只写入一次结果；reset 之后才允许下一局结算。
 *
 * 处理函数由每次 complete 调用时传入，结算器本身不持有任何外部引用，
 * 因此可以安全地在组件外创建，也不会捕获过期的界面状态。
 */
export function createSettler() {
  let settled = false
  return {
    complete<T extends Outcome>(outcome: T, onSettle: (result: Settlement<T>) => void): void {
      if (settled) return
      settled = true
      onSettle({ ...outcome, record: !outcome.pasted })
    },
    reset(): void {
      settled = false
    },
  }
}

/** 把本次打错的目标字符计入易错统计；种类数受上限约束，避免存档无限增长。 */
export function mergeMistakes(
  current: Record<string, number>,
  missed: string[],
  max = MAX_MISTAKES,
): Record<string, number> {
  if (missed.length === 0) return current
  const next = { ...current }
  for (const ch of missed) next[ch] = (next[ch] ?? 0) + 1
  const entries = Object.entries(next)
  if (entries.length <= max) return next
  return Object.fromEntries(entries.sort((a, b) => b[1] - a[1]).slice(0, max))
}

/**
 * 串行提交队列：后一次提交必须等前一次的累计状态回来再算，
 * 否则连续快速输入会让两次请求读到同一个旧状态而漏计尝试。
 */
export function createQueue() {
  let tail: Promise<void> = Promise.resolve()
  return {
    push(task: () => Promise<void>): Promise<void> {
      tail = tail.then(task, task)
      return tail
    },
  }
}
