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

export type TipSide = 'above' | 'below' | 'right' | 'left'

export interface TipBounds {
  left: number
  top: number
  right: number
  bottom: number
}

export interface TipPlacement {
  // Side of the marker the card sits on.
  side: TipSide
  // Top-left corner of the card (CSS px relative to the chart container), clamped
  // so the card never overflows `bounds`.
  left: number
  top: number
  // The caret anchor on the card edge nearest the marker; tracks the marker (cx
  // for above/below, cy for right/left), kept within the card span.
  caretLeft: number
  caretTop: number
}

// Gap between the marker and the nearest card edge (the caret lives here) and the
// minimum margin kept between the card and the container edge when clamping.
const TIP_GAP = 10
const TIP_MARGIN = 4

// Pure geometry for the at-point focus card. `(cx, cy)` is the marker in CSS px
// relative to the chart container; `tipW`/`tipH` is the measured card size;
// `bounds` is the container box. Picks the first side with room in the order
// above -> below -> right -> left (else the side with the most space), centers the
// card on the marker along the free axis, and clamps both axes so the card stays
// inside `bounds`. The caret keeps tracking the marker within the card span.
export function tipPlacement(
  cx: number,
  cy: number,
  tipW: number,
  tipH: number,
  bounds: TipBounds,
): TipPlacement {
  const spaceAbove = cy - bounds.top
  const spaceBelow = bounds.bottom - cy
  const spaceRight = bounds.right - cx
  const spaceLeft = cx - bounds.left

  const candidates: { side: TipSide; fits: boolean; space: number }[] = [
    { side: 'above', fits: spaceAbove >= tipH + TIP_GAP, space: spaceAbove },
    { side: 'below', fits: spaceBelow >= tipH + TIP_GAP, space: spaceBelow },
    { side: 'right', fits: spaceRight >= tipW + TIP_GAP, space: spaceRight },
    { side: 'left', fits: spaceLeft >= tipW + TIP_GAP, space: spaceLeft },
  ]
  const chosen =
    candidates.find((c) => c.fits) ??
    candidates.reduce((best, c) => (c.space > best.space ? c : best))
  const side = chosen.side

  const minLeft = bounds.left + TIP_MARGIN
  const maxLeft = bounds.right - TIP_MARGIN - tipW
  const clampLeft = (l: number) =>
    maxLeft < minLeft ? l : Math.min(Math.max(l, minLeft), maxLeft)
  const minTop = bounds.top + TIP_MARGIN
  const maxTop = bounds.bottom - TIP_MARGIN - tipH
  const clampTop = (t: number) =>
    maxTop < minTop ? t : Math.min(Math.max(t, minTop), maxTop)

  if (side === 'above' || side === 'below') {
    const left = clampLeft(cx - tipW / 2)
    const top = clampTop(side === 'above' ? cy - TIP_GAP - tipH : cy + TIP_GAP)
    const caretLeft = Math.min(Math.max(cx, left), left + tipW)
    const caretTop = side === 'above' ? top + tipH : top
    return { side, left, top, caretLeft, caretTop }
  }
  const top = clampTop(cy - tipH / 2)
  const left = clampLeft(side === 'right' ? cx + TIP_GAP : cx - TIP_GAP - tipW)
  const caretTop = Math.min(Math.max(cy, top), top + tipH)
  const caretLeft = side === 'right' ? left : left + tipW
  return { side, left, top, caretLeft, caretTop }
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
