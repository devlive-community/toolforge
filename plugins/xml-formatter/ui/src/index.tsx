import { useState } from 'react'
import { Select } from '@toolforge/ui'
import { FormatterTool, usePlugin, type FormatterResult } from '@toolforge/plugin-ui-sdk'
import { Minimize, ShieldCheck, Sparkles } from 'lucide-react'

type Indent = '2' | '4' | 'tab'

interface XmlResult extends FormatterResult {
  stats: { lines: number; chars: number; elements: number; attributes: number; comments: number; maxDepth: number }
}

const SAMPLE = `<?xml version="1.0" encoding="UTF-8"?><!-- ToolForge 插件清单 --><toolforge version="0.1.0"><plugin id="org.devlive.toolforge.json-formatter" category="dev"><name lang="zh-CN">JSON 格式化</name><name lang="en-US">JSON Formatter</name><functions><function name="format"/><function name="minify"/></functions></plugin><plugin id="org.devlive.toolforge.hash" category="dev"><name lang="zh-CN">哈希计算</name><description><![CDATA[MD5 & SHA <fast>]]></description></plugin></toolforge>`

function XmlFormatter() {
  const { t } = usePlugin()
  const [indent, setIndent] = useState<Indent>('2')
  return (
    <FormatterTool<XmlResult>
      language="xml"
      inputTitle={t('input')}
      placeholder={t('placeholder')}
      sample={SAMPLE}
      fileFilters={[{ name: 'XML', extensions: ['xml', 'svg', 'xhtml', 'xsd', 'xsl', 'plist', 'txt'] }]}
      downloadName="formatted.xml"
      args={{ indent }}
      modes={[
        { fn: 'format', label: t('modes.format'), icon: <Sparkles />, outputTitle: t('output.format') },
        { fn: 'minify', label: t('modes.minify'), icon: <Minimize />, outputTitle: t('output.minify') },
        { fn: 'validate', label: t('modes.validate'), icon: <ShieldCheck />, outputTitle: t('output.validate') },
      ]}
      summary={(result) => (
        <span>{t('summary', { elements: result.stats.elements, attributes: result.stats.attributes, depth: result.stats.maxDepth })}</span>
      )}
      options={
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
      }
    />
  )
}

export default XmlFormatter
