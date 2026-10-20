import assert from 'node:assert/strict'
import { mock, test } from 'node:test'
import { createInstance } from 'i18next'
import zhCN from '../src/locales/zh-CN.json' with { type: 'json' }
import enUS from '../src/locales/en-US.json' with { type: 'json' }
import { formatErrorMessage } from '../../../packages/plugin-ui-sdk/src/errorMessage.ts'

let taskPage = { records: [], total: 0 }
mock.module('@toolforge/plugin-ui-sdk', {
  namedExports: { listTasks: async () => taskPage },
})
const { useTasks } = await import('../src/stores/tasks.ts')

async function language(lng, interpolation = {}) {
  const instance = createInstance()
  await instance.init({
    lng,
    fallbackLng: 'en-US',
    defaultNS: 'app',
    fallbackNS: 'common',
    ns: ['app', 'common'],
    resources: { 'zh-CN': zhCN, 'en-US': enUS },
    interpolation: { escapeValue: false, ...interpolation },
    returnNull: false,
  })
  return instance
}

for (const lng of ['zh-CN', 'en-US']) {
  test(`${lng}: renders saved network parameters without altering their values`, async () => {
    const i18n = await language(lng)
    const params = Object.freeze({ host: 'github.com', detail: 'timeout: <network> & {{host}} / $&' })
    const rendered = formatErrorMessage({ code: 'resource.network', params }, i18n.t, i18n)
    assert.equal(rendered, lng === 'zh-CN'
      ? `连接 github.com 时网络错误：${params.detail}`
      : `Network error while contacting github.com: ${params.detail}`)
    assert.equal(params.detail, 'timeout: <network> & {{host}} / $&')
  })

  test(`${lng}: old or incomplete task parameters produce a readable explanation`, async () => {
    const i18n = await language(lng)
    for (const params of [undefined, {}, { host: 'github.com' }, { host: undefined, detail: 'timeout' }]) {
      const rendered = formatErrorMessage({ code: 'resource.network', params }, i18n.t, i18n)
      assert.equal(rendered, lng === 'zh-CN'
        ? '无法取得完整错误详情（resource.network），请查看日志'
        : 'Full error details are unavailable (resource.network). See the logs.')
      assert.ok(!rendered.includes('{{'))
    }
  })

  test(`${lng}: parameterless and unknown errors retain their normal messages`, async () => {
    const i18n = await language(lng)
    assert.equal(formatErrorMessage({ code: 'fs.not_found' }, i18n.t, i18n), lng === 'zh-CN' ? '文件不存在' : 'File not found')
    assert.equal(formatErrorMessage({ code: 'fixture.unknown' }, i18n.t, i18n), 'fixture.unknown')
    const rendered = formatErrorMessage({ code: 'resource.size_mismatch', params: { actual: 0, expected: 10 } }, i18n.t, i18n)
    assert.equal(rendered, lng === 'zh-CN'
      ? '下载中断（已下载 0 / 10 字节），重试即可继续'
      : 'The download stopped early (0 of 10 bytes); try again to resume')
  })

  test(`${lng}: plugin messages take priority and common errors remain available`, async () => {
    const i18n = await language(lng)
    const ns = 'fixture-plugin'
    i18n.addResourceBundle(lng, ns, { errors: { 'resource.network': 'Plugin: {{detail}}' } })
    assert.equal(formatErrorMessage({ code: 'resource.network', params: { detail: '{{literal}}' } }, i18n.t, i18n, ns), 'Plugin: {{literal}}')
    assert.equal(formatErrorMessage({ code: 'fs.not_found' }, i18n.t, i18n, ns), lng === 'zh-CN' ? '文件不存在' : 'File not found')
    assert.equal(formatErrorMessage({ code: 'resource.network', params: { host: 'fixture' } }, i18n.t, i18n, ns),
      i18n.t('errors.task.details_missing', { ns: 'common', code: 'resource.network' }))
  })

  test(`${lng}: resource recovery messages have complete interpolations`, async () => {
    const i18n = await language(lng)
    const rendered = formatErrorMessage({ code: 'resource.range_mismatch', params: { host: 'github.com' } }, i18n.t, i18n)
    assert.ok(rendered.includes('github.com'))
    assert.ok(!rendered.includes('{{'))
    assert.equal(i18n.t('logs.resource.retry', { ns: 'common', host: 'github.com', attempt: 2, total: 3 }), lng === 'zh-CN'
      ? '下载暂时中断，正在重试 2/3（github.com）'
      : 'Download interrupted. Retrying 2/3 (github.com)')
  })
}

test('nested, formatted, default and empty values are not mistaken for missing parameters', async () => {
  const i18n = await language('en-US', { defaultVariables: { product: 'ToolForge' } })
  i18n.addResourceBundle('en-US', 'fixture-plugin', {
    errors: { values: '{{product}} {{payload.amount, number}} {{- detail}} [{{empty}}] {{flag}}' },
  })
  const params = Object.freeze({ payload: Object.freeze({ amount: 0 }), detail: '<{{literal}}>', empty: '', flag: false })
  assert.equal(formatErrorMessage({ code: 'values', params }, i18n.t, i18n, 'fixture-plugin'), 'ToolForge 0 <{{literal}}> [] false')
  assert.equal(formatErrorMessage({ code: 'values', params: { ...params, product: undefined } }, i18n.t, i18n, 'fixture-plugin'), 'ToolForge 0 <{{literal}}> [] false')
})

function resetTasks() {
  useTasks.setState({ records: [], total: 0, running: 0, live: {}, open: false })
}

const started = { type: 'started', taskId: 'download-1', pluginId: 'org.devlive.toolforge.background-remover', function: 'download_resource', startedAt: 100 }

test('the real task store keeps finished error parameters and clears stale ones on success', () => {
  resetTasks()
  const params = Object.freeze({ host: 'github.com', detail: 'fixture timeout', attempt: 0, retry: false })
  useTasks.getState().apply(started)
  useTasks.getState().apply({ type: 'finished', taskId: started.taskId, status: 'failed', elapsedMs: 223, result: null, error: { code: 'resource.network', params } })
  const record = useTasks.getState().records[0]
  assert.equal(record.status, 'failed')
  assert.equal(record.errorCode, 'resource.network')
  assert.equal(record.errorParams, params)
  assert.equal(record.errorParams.attempt, 0)
  assert.equal(record.errorParams.retry, false)

  useTasks.getState().apply({ type: 'finished', taskId: started.taskId, status: 'succeeded', elapsedMs: 400, result: {}, error: null })
  assert.equal(useTasks.getState().records[0].errorCode, null)
  assert.equal(useTasks.getState().records[0].errorParams, undefined)
})

test('paging restored tasks preserves full errors while old records still get the missing-details message', async () => {
  resetTasks()
  const i18n = await language('en-US')
  taskPage = {
    total: 2,
    records: [
      { id: 'saved', errorCode: 'resource.network', errorParams: { host: 'github.com', detail: 'fixture reset' } },
      { id: 'old', errorCode: 'resource.network' },
    ],
  }
  await useTasks.getState().loadPage(1)
  const [saved, old] = useTasks.getState().records
  assert.equal(formatErrorMessage({ code: saved.errorCode, params: saved.errorParams }, i18n.t, i18n), 'Network error while contacting github.com: fixture reset')
  assert.equal(formatErrorMessage({ code: old.errorCode, params: old.errorParams }, i18n.t, i18n), 'Full error details are unavailable (resource.network). See the logs.')
})
