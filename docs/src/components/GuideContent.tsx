import Link from 'next/link'
import { ArrowRight, Database, FolderOpen, Globe } from 'lucide-react'
import { DownloadDialog } from './DownloadDialog'
import { agentReleased } from '@/lib/tool-docs'
import { RELEASES_URL, type Lang } from '@/lib/catalog'
import type { GuideTopic } from '@/lib/guides'

function Step({ number, title, children }: { number: number; title: string; children: React.ReactNode }) {
  return (
    <section className="site-step">
      <span className="site-step-number">{String(number).padStart(2, '0')}</span>
      <div>
        <h2>{title}</h2>
        {children}
      </div>
    </section>
  )
}
export function GuideContent({ topic, lang }: { topic: GuideTopic; lang: Lang }) {
  const zh = lang === 'zh'
  if (topic === 'start')
    return (
      <>
        <Step number={1} title={zh ? '安装桌面应用' : 'Install the desktop app'}>
          <p>
            {zh
              ? '选择系统与架构，下载对应的正式安装包。macOS 区分 Apple Silicon 和 Intel；Windows、Linux 请选择页面实际提供的包。'
              : 'Choose an official installer for your OS and architecture. On macOS, select Apple Silicon or Intel; choose one of the packages actually offered for Windows or Linux.'}
          </p>
          <DownloadDialog lang={lang} compact label={zh ? '选择安装包' : 'Choose an installer'} />
          <p>
            {zh
              ? 'macOS：打开 DMG 并将应用放入 Applications。Windows：运行安装程序。Linux：使用对应 deb 包，或给 AppImage 执行权限后打开。'
              : 'macOS: open the DMG and move the app to Applications. Windows: run the installer. Linux: install the deb package, or make the AppImage executable before opening it.'}
          </p>
        </Step>
        <Step number={2} title={zh ? '找到顺手的工具' : 'Find the right tool'}>
          <p>
            {zh
              ? '在侧栏按名称搜索，或按分类浏览。内置工具随应用提供；在线插件先在工具市场安装，再打开使用。'
              : 'Search by name in the sidebar or browse categories. Built-in tools ship with the app; install online plugins from the market before using them.'}
          </p>
          <Link className="site-link" href={`/${lang}/tools/`}>
            {zh ? '浏览工具库' : 'Explore the toolbox'}
            <ArrowRight aria-hidden="true" />
          </Link>
        </Step>
        <Step number={3} title={zh ? '试一次，再融入工作流' : 'Try it, then make it part of your workflow'}>
          <p>
            {zh
              ? '以 JSON 格式化为例：粘贴内容、选择缩进、执行格式化，检查后再复制或保存。文件操作请先确认输入、输出位置与覆盖范围。'
              : 'For JSON formatting, paste content, choose indentation, format it, then inspect and copy or save the result. For files, check input, output and overwrite behavior first.'}
          </p>
          <Link className="site-link" href={`/${lang}/tools/json-formatter/`}>
            {zh ? '阅读 JSON 格式化指南' : 'Read the JSON formatting guide'}
            <ArrowRight aria-hidden="true" />
          </Link>
        </Step>
      </>
    )
  if (topic === 'plugins')
    return (
      <>
        <Step number={1} title={zh ? '先了解工具' : 'Get to know the tool'}>
          <p>
            {zh
              ? '在工具库查看功能、使用方法、清单版本与权限。待发布的条目仅供了解，不能据此假定当前版本已经提供。'
              : 'Check features, usage, manifest version and permissions. Upcoming entries document planned changes, not guaranteed availability in your installed version.'}
          </p>
        </Step>
        <Step number={2} title={zh ? '选择安装方式' : 'Choose how to install'}>
          <p>
            {zh
              ? '内置工具在应用中直接查找。在线插件可从应用内市场安装，或在已发布工具的详情页点击一键安装，浏览器会请求唤起 ToolForge。'
              : 'Find built-in tools directly in the app. Install online plugins from the in-app market, or use one-click install on a released tool’s page to launch ToolForge from the browser.'}
          </p>
          <div className="site-note">
            {zh
              ? '网页一键安装需要 ToolForge 0.2.0 或更高版本。请先安装并打开对应版本的桌面应用。开发运行的裸二进制不一定已向系统注册链接协议。'
              : 'One-click installation needs ToolForge 0.2.0 or later. Install and open a compatible desktop app first. A development binary may not have registered the link protocol with the OS.'}
          </div>
        </Step>
        <Step number={3} title={zh ? '管理更新与调用方式' : 'Manage updates and access'}>
          <p>
            {zh
              ? '在线插件独立于应用更新；内置工具随应用更新。工具市场的配置入口可管理支持工具的命令行与 AI Agent 调用，具体配置能力取决于所装版本。'
              : 'Online plugins update independently; built-in tools update with the app. Tool-market settings manage CLI and AI Agent access for supported tools, depending on the installed version.'}
          </p>
          <Link className="site-link" href={`/${lang}/guide/agent/`}>
            {zh ? '了解 Agent 接入' : 'Read about Agent access'}
            <ArrowRight aria-hidden="true" />
          </Link>
        </Step>
      </>
    )
  if (topic === 'tasks')
    return (
      <>
        <h2>{zh ? '执行过程中' : 'While a task runs'}</h2>
        <p>
          {zh
            ? '耗时处理会显示阶段、进度和日志。在工具当前页面或任务中心查看状态，需要停止时使用取消。取消请求与操作实际停止之间可能存在短暂间隔，文件结果请以最终状态为准。'
            : 'Long operations show stages, progress and logs. Follow them in the tool or task center, and request cancellation when needed. Stopping may take a moment; use the final status to determine whether a file was produced.'}
        </p>
        <h2>{zh ? '回看记录' : 'Review previous work'}</h2>
        <p>
          {zh
            ? '历史和任务记录用于回看，列表通过分页展示。每次工具调用单独记录，不通过短时间合并来覆盖之前的调用。清理记录前请确认是否还需要相关内容。'
            : 'History and task records support review with paginated lists. Each tool invocation is recorded separately rather than merged with recent calls. Check whether you still need a record before clearing it.'}
        </p>
        <h2>{zh ? '重跑与敏感输入' : 'Reruns and sensitive inputs'}</h2>
        <p>
          {zh
            ? '只有保存了参数的任务才能重跑。敏感工具和超出参数预算的任务不会保存可重跑输入；任务仍可能有状态或日志，请留意其中的输出。'
            : 'Only tasks with saved arguments can be rerun. Sensitive tools and oversized arguments are not saved as rerunnable inputs. Task status or logs may still exist; handle their outputs appropriately.'}
        </p>
      </>
    )
  if (topic === 'privacy')
    return (
      <>
        <p>
          {zh
            ? '本地优先不等于所有功能永不联网。格式化、转换等本机处理，与 HTTP 请求、下载插件或模型等网络操作，应分别理解。'
            : 'Local first does not mean every feature is always offline. Local formatting and conversions differ from HTTP requests, plugin downloads or model downloads.'}
        </p>
        {[
          {
            icon: FolderOpen,
            title: zh ? '文件读写' : 'Files',
            text: zh
              ? '先确认选择了哪些文件、结果保存在哪里，以及是否覆盖已有内容。Agent 还需要被授予相应目录的只读或可写范围，提供路径不自动代表获准访问。'
              : 'Check selected files, output locations and overwrites. Agents additionally need authorized read-only or writable directory scopes; supplying a path does not itself grant access.',
          },
          {
            icon: Globe,
            title: zh ? '网络访问' : 'Network requests',
            text: zh
              ? '确认目标服务、发送的内容和凭据。不同工具的联网用途不同，不应将本地计算与发送请求混为一谈。'
              : 'Check the target service, transmitted content and credentials. Different tools use the network differently; local calculations and requests are separate operations.',
          },
          {
            icon: Database,
            title: zh ? '本地保存' : 'Saved data',
            text: zh
              ? '应用与插件持久数据复用 ToolForge SQLite。是否自动保存、是否支持备份恢复，应以各工具帮助为准。删除与导出是单独的操作。'
              : 'The app and plugins share ToolForge SQLite for persistent data. Autosave and backup/restore behavior depend on each tool. Deleting and exporting are separate operations.',
          },
        ].map(({ icon: Icon, title, text }) => (
          <section className="site-permission" key={title}>
            <Icon aria-hidden="true" />
            <div>
              <strong>{title}</strong>
              <p>{text}</p>
            </div>
          </section>
        ))}
        <div className="site-note">
          {zh
            ? '权限清单描述工具具备的能力，不代表外部 Agent 自动获得这些权限。请同时检查总开关、工具/函数配置与目录范围。'
            : 'Manifest capabilities do not automatically grant an external Agent access. Review the global switch, tool/function settings and directory scopes.'}
        </div>
      </>
    )
  if (topic === 'agent')
    return (
      <>
        {!agentReleased && (
          <div className="site-note site-note-warn">
            {zh
              ? '接入说明对应包含 CLI / MCP 的新版本代码。当前稳定安装包尚未确认包含此能力，请先核对正式发布说明及应用中的设置入口。'
              : 'This guide describes the newer CLI / MCP implementation. The current stable installer has not been verified to include it. Check published release notes and the settings available in your app.'}
            <p>
              <a href={RELEASES_URL} target="_blank" rel="noreferrer" className="site-link">
                {zh ? '查看发布记录' : 'Check releases'}
                <ArrowRight aria-hidden="true" />
              </a>
            </p>
          </div>
        )}
        <h2>{zh ? '命令行：先生成入口' : 'CLI: generate the commands first'}</h2>
        <p>
          {zh
            ? '在系统设置的命令行工具页生成命令，将界面展示的命令目录加入 PATH。通过 tfp 查看工具与帮助，支持的插件也可使用 tfp-工具名 的独立入口。'
            : 'Generate commands from the command-line tools page in Settings, then add the displayed directory to PATH. Use tfp to list tools and read help; supported plugins also get a tfp-tool-name command.'}
        </p>
        <pre className="site-doc-code">
          <code>{'tfp list\ntfp help json-formatter'}</code>
        </pre>
        <p>
          {zh
            ? '默认输出为人类可读文本，脚本需要原始 JSON 时使用 --json。命令行开关按工具和函数分别管理；未声明或被关闭的函数不会暴露。'
            : 'Output is human-readable by default; use --json for raw JSON. CLI access is controlled per tool and function. Undeclared or disabled functions are not exposed.'}
        </p>
        <h2>{zh ? 'MCP：从应用复制配置' : 'MCP: copy the app’s configuration'}</h2>
        <ol>
          <li>
            {zh
              ? '先在工具市场的工具配置中检查 AI Agent 页签，按需关闭不希望开放的工具或函数。'
              : 'Review the AI Agent tab in each tool’s market settings. Disable tools or functions you do not want exposed.'}
          </li>
          <li>
            {zh
              ? '在系统设置 → AI Agent 中开启总开关，配置可读/可写目录。总开关默认关闭。'
              : 'Enable the global switch in Settings → AI Agent and configure readable/writable directories. The global switch is off by default.'}
          </li>
          <li>
            {zh
              ? '复制该页面生成的 MCP 配置到支持 stdio MCP 的客户端。command 使用应用提供的 tfp 绝对路径，args 为 ["mcp"]。'
              : 'Copy the generated configuration into a client supporting stdio MCP. command uses the app-provided absolute tfp path; args is ["mcp"].'}
          </li>
        </ol>
        <pre className="site-doc-code">
          <code>tfp mcp</code>
        </pre>
        <p>
          {zh
            ? '这是标准输入输出连接，不需要自行开放网络端口。不同客户端的配置文件位置与格式可能不同，请按客户端要求放置复制的条目。'
            : 'The connection uses standard input/output, so you do not need to open a network port. Client configuration locations and formats vary; place the copied entry as required by that client.'}
        </p>
        <h2>{zh ? '调用前确认范围' : 'Review access before calling'}</h2>
        <p>
          {zh
            ? '工具是否出现，由接口声明、总开关和工具/函数配置共同决定。插件和函数默认未单独关闭时可被列出；只读函数默认允许，写入或外部操作默认需要确认。部分能力不能设置为始终允许。'
            : 'Visibility depends on interface declarations, the global switch and tool/function settings. Tools and functions are listed unless disabled individually. Read-only functions are allowed by default; writes and external effects require confirmation by default. Some capabilities cannot be set to always allow.'}
        </p>
        <p>
          {zh
            ? 'Agent 文件参数使用绝对路径，并且必须位于授权目录内。需要确认而客户端不支持确认交互时，调用会被拒绝；不要通过扩大权限绕过这个限制。'
            : 'Agent file arguments must be absolute paths inside authorized directories. Calls needing confirmation are rejected if the client cannot support the confirmation flow; do not expand permissions to bypass it.'}
        </p>
        <Link className="site-link" href={`/${lang}/guide/privacy/`}>
          {zh ? '阅读权限与隐私说明' : 'Read about permissions & privacy'}
          <ArrowRight aria-hidden="true" />
        </Link>
      </>
    )
  const faq = zh
    ? [
        ['必须联网才能使用吗？', '常用处理可以在本机完成。首次安装在线插件、下载模型或使用网络工具时需要网络，具体以工具说明为准。'],
        ['目录中的工具都已发布了吗？', '不一定。目录会标注待发布内容和未发布更新。在线安装按钮只在核对到正式插件版本时提供；信息不可用时可到应用内市场确认。'],
        ['这是网页工具箱吗？', '站点用于发现工具与阅读文档，实际工具在 ToolForge 桌面应用中运行。'],
        ['点击一键安装没有打开应用？', '请安装 ToolForge 0.2.0+，确认系统允许浏览器打开 toolforge 链接。也可以直接在应用内工具市场搜索安装。'],
        ['为什么某些工具没有 Agent 接口？', '适合外部调用的功能需要逐个声明和评估。应用里能打开的工具不一定能从 CLI 或 Agent 调用。'],
      ]
    : [
        [
          'Do I always need an internet connection?',
          'Everyday processing can stay local. Installing online plugins, downloading models and using network tools requires a connection; check each tool’s guide.',
        ],
        [
          'Has every tool in the catalog been released?',
          'No. Upcoming tools and pending updates are labeled. Online install actions are offered only after verifying a published plugin version; use the in-app market if details are unavailable.',
        ],
        ['Is this a web toolbox?', 'The site helps you discover tools and read guides. Tools run in the ToolForge desktop app.'],
        [
          'Why does one-click install not open the app?',
          'Install ToolForge 0.2.0+ and allow your browser to open toolforge links, or search for the plugin in the in-app market.',
        ],
        [
          'Why do some tools have no Agent interface?',
          'External interfaces need explicit declarations and review. A tool available in the app may not be callable through CLI or Agent.',
        ],
      ]
  return (
    <>
      {faq.map(([question, answer]) => (
        <details className="site-faq" key={question}>
          <summary>{question}</summary>
          <p>{answer}</p>
        </details>
      ))}
    </>
  )
}
