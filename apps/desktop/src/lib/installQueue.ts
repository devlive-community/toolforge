/**
 * 限制同时进行的插件安装数量。超过上限的任务排队等待，按提交顺序执行。
 * 上限在每次调度时读取，设置页修改后对之后的任务立即生效。
 */
export function createInstallQueue(getLimit: () => number) {
  let running = 0
  const waiting: Array<() => void> = []

  const pump = () => {
    while (running < Math.max(1, getLimit()) && waiting.length > 0) {
      running += 1
      waiting.shift()?.()
    }
  }

  return {
    /** 排队执行 task；返回 task 的结果，失败时同样返回 reject */
    run<T>(task: () => Promise<T>): Promise<T> {
      return new Promise<T>((resolve, reject) => {
        waiting.push(() => {
          task().then(resolve, reject).finally(() => {
            running -= 1
            pump()
          })
        })
        pump()
      })
    },
  }
}
