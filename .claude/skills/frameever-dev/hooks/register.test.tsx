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

test('the pane draws every button on every surface', async ($, on) => {
  on('command.register', () => ({ value: { command: 'fe-dev' } }))
  on('ui.open', () => ({ value: { isPlaced: true } }))
  on('clock.now', () => ({ value: 1000 }))
  on('session.start', (_, e) => ({ cwd: e.cwd }))

  await $.session.start({ cwd: 'D:\\repo', surface: 'desktop', isInteractive: true })

  for (const surface of ['terminal', 'desktop'] as const) {
    const ui = await $.ui.mount({ ...PANE, surface })
    for (const key of ['run-dev', 'run-check', 'run-test-all', 'run-export-av1', 'run-package', 'open-dist']) {
      expect(await ui.find({ type: 'Button', key })).toBeTruthy()
    }
    expect(await ui.find({ type: 'Text', text: /pnpm fe release/ })).toBeTruthy()
    await ui.unmount()
  }
})
