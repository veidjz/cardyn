import { listen } from '@tauri-apps/api/event'
import type { MetricsSnapshot } from './types'

export const metrics = $state<{
  latest: MetricsSnapshot | null
  cpuHistory: number[]
}>({
  latest: null,
  cpuHistory: [],
})

let unlisten: (() => void) | null = null

export async function startMetrics(): Promise<() => void> {
  if (unlisten) return unlisten

  const stop = await listen<MetricsSnapshot>('metrics', (event) => {
    metrics.latest = event.payload
    metrics.cpuHistory.push(event.payload.cpuTotal)
    if (metrics.cpuHistory.length > 60) metrics.cpuHistory.shift()
  })

  unlisten = () => {
    stop()
    unlisten = null
  }

  return unlisten
}
