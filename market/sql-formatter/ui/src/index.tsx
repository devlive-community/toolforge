import { useState } from 'react'
import { Select, Switch } from '@toolforge/ui'
import { FormatterTool, usePlugin, type FormatterResult } from '@toolforge/plugin-ui-sdk'
import { Minimize, Sparkles } from 'lucide-react'

type Indent = '2' | '4' | 'tab'
type KeywordCase = 'upper' | 'lower' | 'preserve'
type Dialect = 'generic' | 'postgresql' | 'sqlserver'

interface SqlResult extends FormatterResult {
  stats: { lines: number; chars: number; statements: number }
}

const SAMPLE = `-- 最近 30 天活跃用户及其订单
select u.id, u.name, count(o.id) as orders, sum(o.amount) as total from users u left join orders o on o.user_id = u.id and o.created_at >= now() - interval '30 days' where u.status = 'active' and u.name <> '' group by u.id, u.name having count(o.id) > 0 order by total desc limit 20;
update users set last_seen = now() where id in (select user_id from sessions where expires_at > now());`

function SqlFormatter() {
  const { t } = usePlugin()
  const [indent, setIndent] = useState<Indent>('2')
  const [keywordCase, setKeywordCase] = useState<KeywordCase>('upper')
  const [dialect, setDialect] = useState<Dialect>('generic')
  const [compact, setCompact] = useState(false)

  return (
    <FormatterTool<SqlResult>
      language="sql"
      inputTitle={t('input')}
      placeholder={t('placeholder')}
      sample={SAMPLE}
      fileFilters={[{ name: 'SQL', extensions: ['sql', 'txt'] }]}
      downloadName="formatted.sql"
      args={{ indent, keywordCase, dialect, compact }}
      modes={[
        { fn: 'format', label: t('modes.format'), icon: <Sparkles />, outputTitle: t('output.format') },
        { fn: 'minify', label: t('modes.minify'), icon: <Minimize />, outputTitle: t('output.minify') },
      ]}
      summary={(result) => <span>{t('statements', { count: result.stats.statements })}</span>}
      options={
        <>
          <label className="flex items-center gap-1.5">
            {t('compact')}
            <Switch size="sm" checked={compact} onCheckedChange={setCompact} aria-label={t('compact')} />
          </label>
          <Select<Dialect>
            size="sm"
            variant="ghost"
            value={dialect}
            onValueChange={setDialect}
            aria-label={t('dialect')}
            options={[
              { value: 'generic', label: t('dialectGeneric') },
              { value: 'postgresql', label: t('dialectPostgresql') },
              { value: 'sqlserver', label: t('dialectSqlserver') },
            ]}
          />
          <Select<KeywordCase>
            size="sm"
            variant="ghost"
            value={keywordCase}
            onValueChange={setKeywordCase}
            aria-label={t('case')}
            options={[
              { value: 'upper', label: t('caseUpper') },
              { value: 'lower', label: t('caseLower') },
              { value: 'preserve', label: t('casePreserve') },
            ]}
          />
          <Select<Indent>
            size="sm"
            variant="ghost"
            value={indent}
            onValueChange={setIndent}
            aria-label={t('indent')}
            options={[
              { value: '2', label: t('indent2') },
              { value: '4', label: t('indent4') },
              { value: 'tab', label: t('indentTab') },
            ]}
          />
        </>
      }
    />
  )
}

export default SqlFormatter
