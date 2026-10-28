/** 结算输入：至少包含是否粘贴；其余成绩字段原样透传给界面。 */
export interface Outcome {
  pasted: boolean
}

/** 结算结果：原始成绩加上 `record`，为 false 时本局只作练习，不写入正式纪录。 */
export type Settlement<T extends Outcome> = T & { record: boolean }

/**
 * 粘贴标记是粘性的：本局只要出现过一次粘贴，之后即使继续手打也仍是练习局。
 */
export function pasteFlag(current: boolean, incoming: boolean): boolean {
  return current || incoming
}

/**
 * 结算器：一局只结算一次。异步提交可能让同一个完成状态被报告多次，
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
