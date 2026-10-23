import Link from 'next/link'
import { ArrowRight, Clipboard, ClipboardCheck, FolderOpen, Globe, Shield } from 'lucide-react'
import type { Lang } from '@/lib/catalog'
import { agentReleased, type ToolDocumentation } from '@/lib/tool-docs'

export function ToolPermissions({ data, lang }: { data: ToolDocumentation; lang: Lang }) {
  const zh = lang === 'zh'
  const descriptions: Record<string, [string, string]> = {
    'fs:user-selected': [
      zh ? '用户选择的文件' : 'Selected files',
      zh ? '读取或写入你选定的文件。操作前请确认输入和输出位置。' : 'Read or write selected files. Check input and output locations before proceeding.',
    ],
    'clipboard:read': [zh ? '读取剪贴板' : 'Read clipboard', zh ? '在对应操作中读取待处理内容。' : 'Read content for the relevant operation.'],
    'clipboard:write': [zh ? '写入剪贴板' : 'Write clipboard', zh ? '在复制操作中写入处理结果。' : 'Write results when you use a copy action.'],
    network: [
      zh ? '网络访问' : 'Network access',
      zh
        ? '向目标服务发送请求，内容可能离开本机。请检查目标地址和所用凭据。'
        : 'Send requests to a target service. Content may leave your device; check the destination and credentials.',
    ],
  }
  return (
    <div className="site-prose">
      <h2>{zh ? '用到什么权限，一眼看清。' : 'Know what a tool can access.'}</h2>
      <p>
        {zh
          ? '以下来自工具清单，是能力声明。具体访问范围仍受宿主授权与当前操作约束。'
          : 'These capabilities come from the tool manifest. Host authorization and the current operation still determine actual access.'}
      </p>
      <ul className="site-permission-list">
        {data.permissions.map((permission) => {
          const Icon =
            permission === 'network'
              ? Globe
              : permission.startsWith('fs:')
                ? FolderOpen
                : permission === 'clipboard:read'
                  ? Clipboard
                  : permission === 'clipboard:write'
                    ? ClipboardCheck
                    : Shield
          const text = descriptions[permission] ?? [
            permission,
            zh ? '此工具声明的系统能力；请查看应用中的具体授权提示。' : 'A declared system capability. Check the authorization prompt in the app.',
          ]
          return (
            <li className="site-permission" key={permission}>
              <Icon aria-hidden="true" />
              <div>
                <strong>{text[0]}</strong>
                <p>{text[1]}</p>
              </div>
            </li>
          )
        })}
      </ul>
      {data.permissions.length === 0 && (
        <div className="site-note">{zh ? '当前清单未声明额外系统权限。' : 'No additional system permissions are declared.'}</div>
      )}
      {data.sensitive && (
        <p>
          {zh
            ? '该工具标记为敏感工具，任务参数不会保存为可重跑输入。请留意输出内容和导出文件。'
            : 'This tool is marked sensitive: task arguments are not saved for reruns. Handle results and exported files with care.'}
        </p>
      )}
      <h3>{zh ? '命令行与 AI Agent' : 'Command line & AI agents'}</h3>
      {data.functions.length > 0 ? (
        <>
          <p>
            {zh
              ? '只有下方明确声明的函数开放相应通道。最终可调用范围还取决于应用版本、工具配置与用户授权。'
              : 'Only the functions explicitly listed below are exposed to their declared channels. Availability also depends on the app version, tool settings and user authorization.'}
          </p>
          {!agentReleased && (
            <div className="site-note site-note-warn">
              {zh
                ? '当前官方稳定版尚未确认包含 CLI / MCP 接入能力。以下为新版本接口文档，请以应用设置与正式发布记录为准。'
                : 'CLI / MCP support has not been verified in the current stable app. The following documents the newer interfaces; check app settings and published releases.'}
            </div>
          )}
          <div className="site-api-functions">
            {data.functions.map((fn) => (
              <details key={fn.name}>
                <summary>
                  <code>{fn.name}</code> · {fn.summaries[lang] || fn.name}
                </summary>
                <p>
                  {fn.expose.map((channel) => (channel === 'cli' ? 'CLI' : 'AI Agent')).join(' / ')} ·{' '}
                  {{ read: zh ? '只读计算' : 'Read-only', write: zh ? '写入或修改' : 'Writes data', external: zh ? '外部操作' : 'External effects' }[fn.effect]}
                  {fn.task ? (zh ? ' · 支持任务进度与取消' : ' · Task progress and cancellation') : ''}
                  {fn.confirm ? (zh ? ' · 每次确认' : ' · Confirmation required') : ''}
                </p>
                {fn.uses.length > 0 && (
                  <p>
                    {zh ? '需要的能力：' : 'Capabilities: '}
                    {fn.uses.join(', ')}
                  </p>
                )}
                {fn.params.length > 0 && (
                  <div className="site-api-scroll">
                    <table className="site-param-table">
                      <thead>
                        <tr>
                          <th>{zh ? '参数' : 'Parameter'}</th>
                          <th>{zh ? '类型' : 'Type'}</th>
                          <th>{zh ? '说明' : 'Description'}</th>
                        </tr>
                      </thead>
                      <tbody>
                        {fn.params.map((param) => (
                          <tr key={param.name}>
                            <td>
                              <code>{param.name}</code>
                              {param.required ? ' *' : ''}
                            </td>
                            <td>{param.type}</td>
                            <td>
                              {param.descriptions[lang]}
                              {param.sensitive ? (zh ? '（敏感参数）' : ' (sensitive)') : ''}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
                {fn.examples.map((example) => (
                  <pre key={example} className="site-doc-code">
                    <code>{example}</code>
                  </pre>
                ))}
              </details>
            ))}
          </div>
        </>
      ) : (
        <p>
          {zh
            ? '当前工具未声明 CLI 或 AI Agent 接口；应用内可用不代表可以从外部直接调用。'
            : 'This tool currently declares no CLI or AI Agent interface. In-app availability does not imply external access.'}
        </p>
      )}
      <p>
        <Link href={`/${lang}/guide/agent/`} className="site-link">
          {zh ? '阅读 Agent 接入指南' : 'Read the Agent guide'}
          <ArrowRight aria-hidden="true" />
        </Link>
      </p>
    </div>
  )
}
