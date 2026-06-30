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

// Metrics whose series are percentages (0-100) and so warrant a fixed y-domain,
// matching their gauges. Bytes/throughput metrics stay auto-scaled.
export function isPercentMetric(metric: MetricKey): boolean {
  return metric === 'cpu' || metric === 'gpu'
}

// Truncate every column of a uPlot data matrix [xs, ...ys] to the shortest
// column length, keeping the most recent points (the tail). Defensive guard for
// series whose backfilled lengths may differ. Empty input -> [].
export function alignSeries(columns: number[][]): number[][] {
  if (columns.length === 0) return []
  const len = Math.min(...columns.map((c) => c.length))
  return columns.map((c) => c.slice(c.length - len))
}

export interface TipPlacement {
  // Box center x (CSS px, relative to the chart container) after horizontal
  // clamping; the caret keeps tracking the true marker x.
  boxX: number
  caretX: number
  // Anchor y the box hangs from: the topmost marker normally, the lowest when
  // flipped below.
  anchorY: number
  // True when the box is placed BELOW the markers (caret points up) because it
  // would otherwise clip past the container top.
  flip: boolean
}

// Pure geometry for the at-point inspect tooltip. All inputs are CSS px relative
// to the chart container. `cx` is the marker x; `topY`/`bottomY` are the highest
// and lowest markers' y. The box centers on `cx` but is clamped horizontally so
// it stays within [plotLeft, plotRight] by its estimated `halfWidth` (caret
// still tracks `cx`). When the topmost marker sits within `upReach` of the
// container top the box would clip above, so it flips to hang below `bottomY`.
export function tipPlacement(
  cx: number,
  topY: number,
  bottomY: number,
  plotLeft: number,
  plotRight: number,
  halfWidth: number,
  upReach: number,
): TipPlacement {
  const flip = topY < upReach
  const lo = plotLeft + halfWidth
  const hi = plotRight - halfWidth
  const boxX = hi < lo ? cx : Math.min(Math.max(cx, lo), hi)
  return { boxX, caretX: cx, anchorY: flip ? bottomY : topY, flip }
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
