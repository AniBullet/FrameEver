import { expect, test } from 'claude-code/testing'

const PANE = {
  plugin: 'frameever-dev',
  component: 'Pane',
  requestId: 'frameever-dev',
  props: {
    title: 'FrameEver 开发',
    isFocused: false,
    bodyColumns: 60,
    placement: 'dock',
    scroll: { offset: 0, bodyRows: 30 },
    view: {},
  },
} as const

test('a job runs in the repository even after the folder the pane started in moved', async ($, on) => {
  const cwds: (string | undefined)[] = []
  const written: { path: string; text: string }[] = []
  const copied: string[] = []
  on('fs.write', (_, e) => {
    written.push({ path: e.path, text: String(e.text) })
    return { value: undefined }
  })
  on('ui.copy', (_, e) => {
    copied.push(e.text)
    return { value: { isCopied: true } }
  })
  on('command.register', () => ({ value: { command: 'fe-dev' } }))
  on('ui.open', () => ({ value: { isPlaced: true } }))
  on('clock.now', () => ({ value: 1000 }))
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('ui.toast', () => ({ value: undefined }))
  on('session.root', () => ({ value: 'H:\\repos\\frameever' }))
  on('session.cwd', () => ({ value: 'H:\\repos\\frameever\\apps' }))
  on('fs.read', (_, e) =>
    e.path === 'H:\\repos\\frameever\\scripts\\fe.mjs' ? { value: '' } : { deny: 'ENOENT' },
  )
  on('process.spawn', async function* (_, e) {
    cwds.push(e.cwd)
    return { value: { code: 0, signal: null } }
  })

  await $.session.start({ cwd: 'D:\\old\\frameever', surface: 'desktop', isInteractive: true })
  const ui = await $.ui.mount({ ...PANE, surface: 'desktop' })
  await ui.press({ key: 'run-check' })
  expect(cwds).toEqual(['H:\\repos\\frameever'])
  expect(written[0]?.path).toBe('H:\\repos\\frameever\\apps\\desktop-gpui\\target\\testdata\\fe-pane-last.log')
  expect(written[0]?.text).toContain('$ pnpm fe check')
  await ui.press({ key: 'copy-shown' })
  expect(copied[0]).toContain('$ pnpm fe check')
  await ui.unmount()
})

test('a job says which folders it searched when no repository is found', async ($, on) => {
  let spawned = false
  on('command.register', () => ({ value: { command: 'fe-dev' } }))
  on('ui.open', () => ({ value: { isPlaced: true } }))
  on('clock.now', () => ({ value: 1000 }))
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('ui.toast', () => ({ value: undefined }))
  on('session.root', () => ({ value: 'D:\\old\\frameever' }))
  on('session.cwd', () => ({ value: 'D:\\old\\frameever' }))
  on('fs.read', () => ({ deny: 'ENOENT' }))
  on('process.spawn', async function* () {
    spawned = true
    return { value: { code: 0, signal: null } }
  })

  await $.session.start({ cwd: 'D:\\old\\frameever', surface: 'desktop', isInteractive: true })
  const ui = await $.ui.mount({ ...PANE, surface: 'desktop' })
  await ui.press({ key: 'run-check' })
  expect(spawned).toBe(false)
  expect(await ui.find({ type: 'Text', text: /找不到 FrameEver 仓库/ })).toBeTruthy()
  await ui.unmount()
})

test('when an app job ends, the recorded PID is stopped only as the test app', async ($, on) => {
  const commands: string[][] = []
  on('command.register', () => ({ value: { command: 'fe-dev' } }))
  on('ui.open', () => ({ value: { isPlaced: true } }))
  on('clock.now', () => ({ value: 1000 }))
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('ui.toast', () => ({ value: undefined }))
  on('fs.write', () => ({ value: undefined }))
  on('session.root', () => ({ value: 'H:\\repos\\frameever' }))
  on('session.cwd', () => ({ value: 'H:\\repos\\frameever' }))
  on('fs.read', (_, e) => ({ value: e.path.endsWith('fe-dev.pid') ? '4242\n' : '' }))
  on('process.spawn', async function* () {
    return { value: { code: 0, signal: null } }
  })
  on('process.run', (_, e) => {
    commands.push([...e.argv])
    return { value: { exitCode: 0, stdout: '', stderr: '' } }
  })

  await $.session.start({ cwd: 'H:\\repos\\frameever', surface: 'desktop', isInteractive: true })
  const ui = await $.ui.mount({ ...PANE, surface: 'desktop' })
  await ui.press({ key: 'run-dev' })
  expect(commands).toEqual([
    ['taskkill', '/F', '/T', '/FI', 'IMAGENAME eq frameever-gpui.fe-run.exe', '/PID', '4242'],
  ])
  await ui.unmount()
})

test('the pane draws every group of buttons on every surface', async ($, on) => {
  on('command.register', () => ({ value: { command: 'fe-dev' } }))
  on('ui.open', () => ({ value: { isPlaced: true } }))
  on('clock.now', () => ({ value: 1000 }))
  on('session.start', (_, e) => ({ cwd: e.cwd }))

  await $.session.start({ cwd: 'D:\\repo', surface: 'desktop', isInteractive: true })

  for (const surface of ['terminal', 'desktop'] as const) {
    const ui = await $.ui.mount({ ...PANE, surface })
    for (const key of [
      'run-dev',
      'run-check',
      'run-test-all',
      'run-export-av1',
      'run-deps',
      'run-deps-apply',
      'run-notes',
      'run-package',
      'lang-zh-CN',
      'lang-en',
      'open-dist',
      'design-index',
      'design-tray',
      'design-installer',
      'design-converter',
      'design-onboarding',
    ]) {
      expect(await ui.find({ type: 'Button', key })).toBeTruthy()
    }
    expect(await ui.find({ type: 'Button', key: 'run-dev-zh' })).toBeFalsy()
    expect(await ui.find({ type: 'Text', text: /pnpm fe release/ })).toBeTruthy()
    await ui.unmount()
  }
})
