import { atom, read, update } from 'claude-code'
import type { EngineInterface, Register } from 'claude-code'

import type { JobState } from '../types'

const PANE = 'frameever-dev'
const TITLE = 'FrameEver 开发'
const MAX_LINES = 400

const jobs = atom({ plugin: 'frameever-dev', key: 'jobs' } as const, {})
const selected = atom({ plugin: 'frameever-dev', key: 'selected' } as const, null)

type Job = { id: string; label: string; args: string[]; launchesApp?: boolean }

const JOBS: Job[] = [
  { id: 'dev', label: '运行应用', args: ['dev'], launchesApp: true },
  { id: 'dev-zh', label: '运行（中文）', args: ['dev', '--lang', 'zh-CN'], launchesApp: true },
  { id: 'dev-watch', label: '热重载开发', args: ['dev', '--watch'], launchesApp: true },
  { id: 'logs', label: '查看日志', args: ['logs', '--lines', '300'] },
  { id: 'doctor', label: '环境检查', args: ['doctor'] },
  { id: 'check', label: '编译检查', args: ['check'] },
  { id: 'test', label: '测试', args: ['test'] },
  { id: 'test-all', label: '全部测试', args: ['test', '--all'] },
  { id: 'lint', label: '代码检查', args: ['lint'] },
  { id: 'fmt', label: '格式化', args: ['fmt'] },
  { id: 'export-h264', label: '导出 H.264', args: ['export', 'h264'] },
  { id: 'export-hevc', label: '导出 HEVC', args: ['export', 'hevc'] },
  { id: 'export-av1', label: '导出 AV1', args: ['export', 'av1'] },
  { id: 'package', label: '打包安装程序', args: ['package'] },
]

const FOLDERS = [
  { key: 'open-testdata', label: '测试数据', path: 'apps\\desktop-gpui\\target\\testdata' },
  { key: 'open-exports', label: '导出视频', path: 'apps\\desktop-gpui\\target\\testdata\\exports' },
  { key: 'open-logs', label: '日志', path: 'apps\\desktop-gpui\\target\\testdata\\logs' },
  { key: 'open-dist', label: '安装包', path: 'dist' },
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
    return
  }
  const startedAt = await $.clock.now()
  await update($, jobs, all => ({
    ...all,
    [job.id]: { status: 'running' as const, startedAt, endedAt: null, lines: [] },
  }))
  await update($, selected, () => job.id)

  const handle = { stop: () => {}, isStopped: false }
  running.set(job.id, handle)
  let status: JobState['status'] = 'passed'
  try {
    const stream = $.process.spawn({
      argv: ['node', 'scripts/fe.mjs', ...job.args],
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
    const job2 = all[job.id]
    return job2 ? { ...all, [job.id]: { ...job2, status, endedAt } } : all
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
      description: 'Open the FrameEver dev pane (pnpm fe: run, check, test, export, package)',
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
    const now = await $.clock.now()
    const describe = (job: JobState) =>
      `${STATUS_LABEL[job.status]} · ${Math.round(((job.endedAt ?? now) - job.startedAt) / 1000)}s`
    const color = (job: JobState) =>
      job.status === 'passed' ? 'success' : job.status === 'failed' ? 'error' : 'subtle'
    const output = shown ? all[shown] : undefined
    const shownLabel = JOBS.find(job => job.id === shown)?.label ?? ''
    const room = Math.max(6, (e.viewport?.rows ?? 40) - JOBS.length - 10)

    return (
      <Box flexDirection="column" gap={1}>
        <Box flexDirection="column">
          {JOBS.map(job => {
            const state = all[job.id]
            return (
              <Box key={`row-${job.id}`} flexDirection="row" gap={1}>
                <Button
                  key={`run-${job.id}`}
                  label={job.label}
                  variant={job.id === 'dev' ? 'primary' : undefined}
                  onPress={() => {
                    void runJob($, job)
                  }}
                />
                {state?.status === 'running' && (
                  <Button
                    key={`stop-${job.id}`}
                    label="停止"
                    dimColor
                    onPress={() => {
                      running.get(job.id)?.stop()
                    }}
                  />
                )}
                {state && (
                  <Button
                    key={`show-${job.id}`}
                    label={describe(state)}
                    plain
                    dimColor={shown !== job.id}
                    onPress={() => update($, selected, () => job.id)}
                  />
                )}
              </Box>
            )
          })}
        </Box>
        <Box flexDirection="row" gap={1}>
          <Text dimColor>打开：</Text>
          {FOLDERS.map(folder => (
            <Button
              key={folder.key}
              label={folder.label}
              dimColor
              onPress={async () => {
                await $.process.run(['explorer', `${root}\\${folder.path}`]).catch(() => undefined)
              }}
            />
          ))}
        </Box>
        <Text dimColor>发版请在终端运行：pnpm fe release 3.0.0（会先跑全部测试并请你确认）</Text>
        {output ? (
          <Box flexDirection="column">
            <Text>
              {shownLabel} · <Text color={color(output)}>{describe(output)}</Text>
            </Text>
            {output.lines.slice(-room).map((line, index) => (
              <Text key={`line-${index}`} dimColor>
                {line || ' '}
              </Text>
            ))}
          </Box>
        ) : (
          <Text dimColor>点按钮开始。所有按钮都调用 pnpm fe，“运行应用”使用独立的测试数据。</Text>
        )}
      </Box>
    )
  })
}
