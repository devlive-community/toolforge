/** 内置练习文本，均为原创短句，不引用任何第三方词库或文章。 */
export interface Passage {
  id: string
  lang: 'zh' | 'en'
  text: string
}

export const PASSAGES: Passage[] = [
  { id: 'zh-1', lang: 'zh', text: '清晨的风轻轻吹过窗台，书页被翻得沙沙作响。' },
  { id: 'zh-2', lang: 'zh', text: '慢一点也没关系，每一次准确的敲击都是进步。' },
  { id: 'zh-3', lang: 'zh', text: '桌上的茶还冒着热气，远处传来几声鸟鸣。' },
  { id: 'en-1', lang: 'en', text: 'Small steady practice beats occasional long sessions.' },
  { id: 'en-2', lang: 'en', text: 'Read each word first, then let your fingers follow the rhythm.' },
  { id: 'en-3', lang: 'en', text: 'A calm mind and relaxed hands make accuracy come naturally.' },
]

/** 关卡顺序：同一语言内的内置短文依次解锁 */
export function passagesFor(lang: 'zh' | 'en'): Passage[] {
  return PASSAGES.filter((p) => p.lang === lang)
}
