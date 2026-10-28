/** 本地日历的今天（YYYY-MM-DD）。借还日期按用户所在地的日期判断，不用 UTC。 */
export function localDate(date: Date = new Date()): string {
  const y = date.getFullYear()
  const m = String(date.getMonth() + 1).padStart(2, '0')
  const d = String(date.getDate()).padStart(2, '0')
  return `${y}-${m}-${d}`
}

/** 新操作编号：重复提交同一编号只生效一次 */
export function newOpId(): string {
  return crypto.randomUUID()
}
