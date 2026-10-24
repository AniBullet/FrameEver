export type JobStatus = 'running' | 'passed' | 'failed' | 'stopped'

export type JobState = {
  status: JobStatus
  startedAt: number
  endedAt: number | null
  lines: string[]
}

export type AppLanguage = 'zh-CN' | 'en'

declare module 'claude-code' {
  interface PluginState {
    'frameever-dev': {
      jobs: Record<string, JobState>
      selected: string | null
      language: AppLanguage
    }
  }
}
