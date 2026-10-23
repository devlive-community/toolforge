import { REPO_URL, type Lang } from '@/lib/catalog'
import type { PluginVersion } from '@/lib/plugin-history'

const TYPE_LABELS = {
  feat: { zh: '新增', en: 'New' },
  fix: { zh: '修复', en: 'Fix' },
  perf: { zh: '性能', en: 'Faster' },
} as const

/** 插件详情页的版本历史：每个版本的面向用户改动，保留提交原文 */
export function PluginHistory({ versions, lang }: { versions: PluginVersion[]; lang: Lang }) {
  const zh = lang === 'zh'
  if (versions.length === 0) {
    return (
      <div className="site-prose">
        <h2>{zh ? '版本历史' : 'Version history'}</h2>
        <p>{zh ? '暂时没有可显示的版本记录。' : 'No version history is available yet.'}</p>
      </div>
    )
  }
  const current = versions.find((v) => !v.pending)?.version
  return (
    <div className="site-prose">
      <h2>{zh ? '版本历史' : 'Version history'}</h2>
      <ol className="site-versions">
        {versions.map((item) => (
          <li key={item.version}>
            <div className="site-version-head">
              <strong>v{item.version}</strong>
              {item.pending ? (
                <span className="site-tag site-tag-warning">{zh ? '待发布' : 'Unreleased'}</span>
              ) : (
                item.version === current && <span className="site-tag">{zh ? '当前版本' : 'Current'}</span>
              )}
              {item.date && <time dateTime={item.date}>{item.date}</time>}
            </div>
            {item.changes.length > 0 ? (
              <ul>
                {item.changes.map((change) => (
                  <li key={change.hash + change.text}>
                    <span className={`site-change site-change-${change.type}`}>{TYPE_LABELS[change.type][lang]}</span>
                    <span>{change.text}</span>
                    <a href={`${REPO_URL}/commit/${change.hash}`} target="_blank" rel="noreferrer" className="site-change-hash">
                      {change.hash}
                    </a>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="site-version-empty">{zh ? '维护性更新，没有面向使用者的改动。' : 'Maintenance release with no user-facing changes.'}</p>
            )}
          </li>
        ))}
      </ol>
      <p className="site-version-note">{zh ? '改动说明保留提交原文。' : 'Change notes keep the original commit wording.'}</p>
    </div>
  )
}
