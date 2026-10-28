/** 与后端 score / prepare / settle 函数返回结构一致（camelCase） */
export interface Session {
  attempts: number
  wrongAttempts: number
  pasted: boolean
  activeMs: number
}

export interface Score {
  correct: number
  wrongNow: number
  total: number
  progress: number
  accuracy: number | null
  cpm: number | null
  wpm: number | null
  completed: boolean
  /** 完成且准确率达到过关门槛 */
  passed: boolean
  pasted: boolean
  /** 每个已输入位置是否正确（按可见字符） */
  marks: boolean[]
  /** 本次新提交中打错的目标字符 */
  missed: string[]
}

export interface ScoreOut {
  session: Session
  score: Score
}

/** prepare 的输出：可见字符切分、内容指纹与限制 */
export interface Prepared {
  fingerprint: string
  total: number
  graphemes: string[]
  passAccuracy: number
  maxText: number
}

export interface SettleOut {
  bests: Record<string, Best>
  recorded: boolean
  newBest: boolean
  cleared: boolean
}

export type Lang = 'zh' | 'en'
export type Mode = 'free' | 'challenge'

/** 个人纪录按 语言 + 模式 + 内容版本 分桶，自定义内容与内置内容分开比较 */
export interface Best {
  bucket: string
  cpm: number | null
  wpm: number | null
  accuracy: number | null
  activeMs: number
  at: string
}

export interface TypingState {
  /** 内置短文的完成情况（关卡解锁依据），键为 lang:index */
  cleared: Record<string, boolean>
  /** 个人纪录，键为 bucket */
  bests: Record<string, Best>
  /** 易错字符统计：字符 -> 错误次数 */
  mistakes: Record<string, number>
  /** 用户自定义练习文本（只保存在本设备） */
  custom: string
}
