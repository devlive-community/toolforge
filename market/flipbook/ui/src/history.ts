/**
 * 作品级撤销/重做。帧内容不可变（每次修改都生成新帧编号），
 * 所以一条历史只需要记下当时的帧顺序；切换帧之后仍能撤销刚才的删除或排序。
 */
export interface History {
  past: string[][]
  future: string[][]
}

/** 撤销栈深度上限：超出时丢弃最早的一步 */
export const MAX_HISTORY = 50

export const emptyHistory = (): History => ({ past: [], future: [] })

/** 记录一次修改前的帧顺序；新的修改会清空重做栈 */
export function record(history: History, before: string[]): History {
  const past = [...history.past, before]
  return { past: past.length > MAX_HISTORY ? past.slice(past.length - MAX_HISTORY) : past, future: [] }
}

/** 撤销：返回要恢复的帧顺序与新的历史；没有可撤销时返回 null */
export function undo(history: History, current: string[]): { frames: string[]; history: History } | null {
  const frames = history.past.at(-1)
  if (!frames) return null
  return { frames, history: { past: history.past.slice(0, -1), future: [current, ...history.future] } }
}

export function redo(history: History, current: string[]): { frames: string[]; history: History } | null {
  const frames = history.future[0]
  if (!frames) return null
  return { frames, history: { past: [...history.past, current], future: history.future.slice(1) } }
}

/** 历史中仍被引用的帧编号（会话中清理存储时保留，撤销后还能找回） */
export function referenced(history: History): string[] {
  return [...new Set([...history.past.flat(), ...history.future.flat()])]
}

/** 存储中多出这么多不在当前顺序里的帧时，清理一次不再被撤销历史引用的帧 */
export const COLLECT_SLACK = 60

/** 把 from 处的元素移到 to（移动后的位置），用于拖动排序的落点换算 */
export function dropIndex(from: number, target: number, side: 'before' | 'after'): number {
  let to = side === 'after' ? target + 1 : target
  if (from < to) to -= 1
  return to
}

/**
 * 串行队列：存储写入与后端调用按顺序执行，
 * 后一次修改总是基于前一次已经保存成功的作品。
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
