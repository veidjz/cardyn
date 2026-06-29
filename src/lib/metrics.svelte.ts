import { listen } from '@tauri-apps/api/event'
import type { MetricsSnapshot } from './types'

export const metrics = $state<{
  latest: MetricsSnapshot | null
  history: {
    cpu: number[]
    mem: number[]
    gpu: number[]
    disk: number[]
    net: number[]
  }
}>({ latest: null, history: { cpu: [], mem: [], gpu: [], disk: [], net: [] } })

let unlisten: (() => void) | null = null

function push(arr: number[], v: number): void {
  arr.push(v)
  if (arr.length > 60) arr.shift()
}

export async function startMetrics(): Promise<() => void> {
  if (unlisten) return unlisten

  const stop = await listen<MetricsSnapshot>('metrics', (event) => {
    const p = event.payload
    metrics.latest = p

    push(metrics.history.cpu, p.cpuTotal)
    if (p.memTotal > 0)
      push(metrics.history.mem, (p.memUsed / p.memTotal) * 100)
    if (p.gpu.utilization !== null) push(metrics.history.gpu, p.gpu.utilization)
    push(metrics.history.disk, p.diskReadBps + p.diskWriteBps)
    push(metrics.history.net, p.netRxBps + p.netTxBps)
  })

  unlisten = () => {
    stop()
    unlisten = null
  }

  return unlisten
}
