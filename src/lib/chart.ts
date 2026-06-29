import type { MetricKey, HistoryMetric } from './types'

export interface ChartSeries {
  series: HistoryMetric
  label: string
}

// The history series that back each main-screen metric card. Disk/Network map
// to two series (read/write, rx/tx); GPU uses utilization (gpuUtil), matching
// the existing card sparkline.
const seriesByMetric: Record<MetricKey, ChartSeries[]> = {
  cpu: [{ series: 'cpu', label: 'CPU' }],
  mem: [{ series: 'mem', label: 'Used' }],
  gpu: [{ series: 'gpuUtil', label: 'Utilization' }],
  disk: [
    { series: 'diskRead', label: 'Read' },
    { series: 'diskWrite', label: 'Write' },
  ],
  net: [
    { series: 'netRx', label: 'Down' },
    { series: 'netTx', label: 'Up' },
  ],
}

export function chartSeries(metric: MetricKey): ChartSeries[] {
  return seriesByMetric[metric]
}

// Truncate every column of a uPlot data matrix [xs, ...ys] to the shortest
// column length, keeping the most recent points (the tail). Defensive guard for
// series whose backfilled lengths may differ. Empty input -> [].
export function alignSeries(columns: number[][]): number[][] {
  if (columns.length === 0) return []
  const len = Math.min(...columns.map((c) => c.length))
  return columns.map((c) => c.slice(c.length - len))
}

export function ringFraction(value: number | null, max: number): number {
  if (value === null) return 0
  const frac = value / max
  if (frac < 0) return 0
  if (frac > 1) return 1
  return frac
}

/// Max for an auto-scaling sparkline (throughput): the window max, but never
/// below `floor` (avoids a flat/zero-division axis when idle). Empty -> floor.
export function sparklineMax(values: number[], floor: number): number {
  if (values.length === 0) return floor
  return Math.max(floor, ...values)
}

export function sparklinePoints(
  values: number[],
  width: number,
  height: number,
  max: number,
): string {
  const n = values.length
  if (n === 0) return ''

  const step = n > 1 ? width / (n - 1) : 0

  return values
    .map((v, i) => {
      const clamped = v < 0 ? 0 : v > max ? max : v
      const x = i * step
      const y = height - (clamped / max) * height
      return `${x},${y}`
    })
    .join(' ')
}
