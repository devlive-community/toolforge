import { cancelTask, startTask, toAppError, type TaskEvent } from '@toolforge/plugin-ui-sdk'

/** 以 Promise 形式运行插件任务；onStart 拿到任务 id 以便取消 */
export function runTask<T>(pluginId: string, fn: string, args: object, onStart?: (taskId: string) => void): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const onEvent = (event: TaskEvent) => {
      if (event.type !== 'finished') return
      if (event.status === 'succeeded') resolve(event.result as T)
      else reject(event.error ?? toAppError({ code: 'task.cancelled' }))
    }
    startTask(pluginId, fn, args, onEvent).then(
      (taskId) => onStart?.(taskId),
      (error) => reject(toAppError(error)),
    )
  })
}

export const cancel = (taskId: string | null) => (taskId ? cancelTask(taskId).catch(() => {}) : Promise.resolve())
