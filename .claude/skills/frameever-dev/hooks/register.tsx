import { atom, read, update } from 'claude-code'
import type { EngineInterface, Register, RenderChildren } from 'claude-code'

import type { AppLanguage, JobState } from '../types'

const PANE = 'frameever-dev'
const TITLE = 'FrameEver 开发'
const MAX_LINES = 400

const jobs = atom({ plugin: 'frameever-dev', key: 'jobs' } as const, {})
const selected = atom({ plugin: 'frameever-dev', key: 'selected' } as const, null)
const language = atom({ plugin: 'frameever-dev', key: 'language' } as const, 'zh-CN')

type Job = {
  id: string
  label: string
  args: (lang: AppLanguage, version: string) => string[]
  needsVersion?: boolean
  launchesApp?: boolean
  primary?: boolean
}

type Group = { title: string; jobs: Job[] }

const fixed = (...args: string[]) => () => args

async function nextVersion($: EngineInterface) {
  const changelog = await $.fs.read(`${root}\\CHANGELOG.md`).catch(() => '')
  return changelog.match(/^## \[(\d+\.\d+\.\d+[^\]]*)\]/m)?.[1] ?? '0.0.0'
}

const GROUPS: Group[] = [
  {
    title: '运行',
    jobs: [
      { id: 'dev', label: '运行应用', args: lang => ['dev', '--lang', lang], launchesApp: true, primary: true },
      { id: 'dev-watch', label: '热重载开发', args: lang => ['dev', '--watch', '--lang', lang], launchesApp: true },
      { id: 'logs', label: '查看日志', args: fixed('logs', '--lines', '300') },
    ],
  },
  {
    title: '检查',
    jobs: [
      { id: 'check', label: '编译检查', args: fixed('check') },
      { id: 'test', label: '测试', args: fixed('test') },
      { id: 'test-all', label: '全部测试', args: fixed('test', '--all') },
      { id: 'lint', label: '代码检查', args: fixed('lint') },
      { id: 'fmt', label: '格式化', args: fixed('fmt') },
      { id: 'doctor', label: '环境检查', args: fixed('doctor') },
    ],
  },
  {
    title: '导出测试录像',
    jobs: [
      { id: 'export-h264', label: 'H.264', args: fixed('export', 'h264') },
      { id: 'export-hevc', label: 'HEVC', args: fixed('export', 'hevc') },
      { id: 'export-av1', label: 'AV1', args: fixed('export', 'av1') },
    ],
  },
  {
    title: '依赖与发布',
    jobs: [
      { id: 'deps', label: '依赖检查', args: fixed('deps') },
      { id: 'deps-apply', label: '升级依赖', args: fixed('deps', '--apply') },
      { id: 'notes', label: '预览更新记录', args: (_, version) => ['notes', version], needsVersion: true },
      { id: 'package', label: '打包安装程序', args: fixed('package') },
      { id: 'setup', label: '准备环境', args: fixed('setup') },
    ],
  },
]

const ALL_JOBS = GROUPS.flatMap(group => group.jobs)

const FOLDERS = [
  { key: 'open-testdata', label: '测试数据', path: 'apps\\desktop-gpui\\target\\testdata' },
  { key: 'open-exports', label: '导出视频', path: 'apps\\desktop-gpui\\target\\testdata\\exports' },
  { key: 'open-logs', label: '日志', path: 'apps\\desktop-gpui\\target\\testdata\\logs' },
  { key: 'open-dist', label: '安装包', path: 'dist' },
]

const LANGUAGES: { value: AppLanguage; label: string }[] = [
  { value: 'zh-CN', label: '中文' },
  { value: 'en', label: 'English' },
]

const DEV_PID_FILE = 'apps\\desktop-gpui\\target\\testdata\\fe-dev.pid'

const ANSI = new RegExp(`${String.fromCharCode(27)}\\[[0-9;]*[A-Za-z]`, 'g')

const STATUS_LABEL = { running: '运行中', passed: '通过', failed: '失败', stopped: '已停止' }

let root = '.'
const running = new Map<string, { stop: () => void; isStopped: boolean }>()

async function append($: EngineInterface, id: string, lines: string[]) {
  if (lines.length === 0) {
    return
  }
  await update($, jobs, all => {
    const job = all[id]
    if (!job) {
      return all
    }
    return { ...all, [id]: { ...job, lines: [...job.lines, ...lines].slice(-MAX_LINES) } }
  })
}

async function stopApp($: EngineInterface) {
  const pid = await $.fs.read(`${root}\\${DEV_PID_FILE}`).catch(() => '')
  if (/^\d+$/.test(pid.trim())) {
    await $.process.run(['taskkill', '/F', '/T', '/PID', pid.trim()]).catch(() => undefined)
  }
}

async function runJob($: EngineInterface, job: Job) {
  if (running.has(job.id)) {
    await update($, selected, () => job.id)
    return
  }
  const version = job.needsVersion ? await nextVersion($) : ''
  const args = job.args(await read($, language), version)
  const startedAt = await $.clock.now()
  await update($, jobs, all => ({
    ...all,
    [job.id]: {
      status: 'running' as const,
      startedAt,
      endedAt: null,
      lines: [`$ pnpm fe ${args.join(' ')}`],
    },
  }))
  await update($, selected, () => job.id)

  const handle = { stop: () => {}, isStopped: false }
  running.set(job.id, handle)
  let status: JobState['status'] = 'passed'
  try {
    const stream = $.process.spawn({
      argv: ['node', 'scripts/fe.mjs', ...args],
      cwd: root,
      env: { FORCE_COLOR: '0' },
    })
    handle.stop = () => {
      handle.isStopped = true
      void stream.return(undefined as never)
    }
    let partial = ''
    let next = await stream.next()
    while (!next.done) {
      const text = (partial + next.value.text).replace(ANSI, '')
      const pieces = text.split(/\r?\n/)
      partial = pieces.pop() ?? ''
      await append($, job.id, pieces)
      next = await stream.next()
    }
    await append($, job.id, partial ? [partial] : [])
    if (handle.isStopped) {
      status = 'stopped'
    } else if (next.value?.code !== 0) {
      status = 'failed'
    }
  } catch (error) {
    status = handle.isStopped ? 'stopped' : 'failed'
    if (!handle.isStopped) {
      await append($, job.id, [String(error instanceof Error ? error.message : error)])
    }
  }
  if (job.launchesApp) {
    await stopApp($)
  }
  running.delete(job.id)
  const endedAt = await $.clock.now()
  await update($, jobs, all => {
    const state = all[job.id]
    return state ? { ...all, [job.id]: { ...state, status, endedAt } } : all
  })
  if (status !== 'stopped') {
    $.ui.toast(`${job.label}：${status === 'passed' ? '完成' : '失败'}`)
  }
}

export const register: Register = on => {
  on('session.start', async ($, e, next) => {
    const started = await next(e)
    root = e.cwd || '.'
    await $.command.register({
      name: 'fe-dev',
      description: 'Open the FrameEver dev pane (pnpm fe: run, check, test, export, deps, package)',
    })
    void $.ui.open({ id: PANE, title: TITLE })
    return started
  })

  on('command.run', { command: 'fe-dev' }, async $ => {
    await $.ui.open({ id: PANE, title: TITLE })
    return { text: 'FrameEver 开发面板已打开。' }
  })

  on('ui.render', { component: 'Pane', requestId: PANE }, async ($, e) => {
    const { Box, Button, Text } = $.ui.resolve(e)
    const all = await read($, jobs)
    const shown = await read($, selected)
    const lang = await read($, language)
    const now = await $.clock.now()
    const seconds = (job: JobState) => Math.round(((job.endedAt ?? now) - job.startedAt) / 1000)
    const color = (job: JobState) =>
      job.status === 'passed' ? 'success' : job.status === 'failed' ? 'error' : 'subtle'
    const output = shown ? all[shown] : undefined
    const shownJob = ALL_JOBS.find(job => job.id === shown)
    const started = ALL_JOBS.filter(job => all[job.id])
    const room = Math.max(8, (e.viewport?.rows ?? 40) - 22)

    const row = (title: string, children: RenderChildren) => (
      <Box key={`group-${title}`} flexDirection="column">
        <Text bold>{title}</Text>
        <Box flexDirection="row" flexWrap="wrap" columnGap={1}>
          {children}
        </Box>
      </Box>
    )

    return (
      <Box flexDirection="column" gap={1}>
        {GROUPS.map(group =>
          row(group.title, [
            ...group.jobs.map(job => (
              <Button
                key={`run-${job.id}`}
                label={all[job.id]?.status === 'running' ? `${job.label} …` : job.label}
                variant={job.primary ? 'primary' : undefined}
                onPress={() => {
                  void runJob($, job)
                }}
              />
            )),
            ...(group.title === '运行'
              ? LANGUAGES.map(option => (
                  <Button
                    key={`lang-${option.value}`}
                    label={option.label}
                    plain
                    dimColor={lang !== option.value}
                    onPress={() => update($, language, () => option.value)}
                  />
                ))
              : []),
          ]),
        )}
        {row(
          '打开文件夹',
          FOLDERS.map(folder => (
            <Button
              key={folder.key}
              label={folder.label}
              dimColor
              onPress={async () => {
                await $.process.run(['explorer', `${root}\\${folder.path}`]).catch(() => undefined)
              }}
            />
          )),
        )}
        {started.length > 0 &&
          row(
            '任务',
            started.map(job => {
              const state = all[job.id]!
              return (
                <Button
                  key={`show-${job.id}`}
                  label={`${job.label} ${STATUS_LABEL[state.status]} ${seconds(state)}s`}
                  plain
                  dimColor={shown !== job.id}
                  onPress={() => update($, selected, () => job.id)}
                />
              )
            }),
          )}
        {output && shownJob ? (
          <Box flexDirection="column">
            <Box flexDirection="row" columnGap={1}>
              <Text>
                {shownJob.label} · <Text color={color(output)}>{STATUS_LABEL[output.status]}</Text> ·{' '}
                {seconds(output)}s
              </Text>
              {output.status === 'running' && (
                <Button
                  key="stop-shown"
                  label="停止"
                  dimColor
                  onPress={() => {
                    running.get(shownJob.id)?.stop()
                  }}
                />
              )}
            </Box>
            {output.lines.slice(-room).map((line, index) => (
              <Text key={`line-${index}`} dimColor>
                {line || ' '}
              </Text>
            ))}
          </Box>
        ) : (
          <Text dimColor>
            点按钮开始，所有按钮都调用 pnpm fe。“运行应用”用独立的测试数据。发版在终端运行 pnpm fe release
            &lt;版本&gt;，会先跑全部测试并请你确认。
          </Text>
        )}
      </Box>
    )
  })
}
