import { describe, it, expect } from 'vitest'
import {
  ringFraction,
  sparklineMax,
  sparklinePoints,
  chartSeries,
  alignSeries,
  isPercentMetric,
  tipPlacement,
} from '../chart'

describe('chartSeries', () => {
  it('maps single-line metrics to one series', () => {
    expect(chartSeries('cpu')).toEqual([{ series: 'cpu', label: 'CPU' }])
    expect(chartSeries('mem')).toEqual([{ series: 'mem', label: 'Used' }])
    expect(chartSeries('gpu')).toEqual([
      { series: 'gpuUtil', label: 'Utilization' },
    ])
  })

  it('maps disk and network to two series each', () => {
    expect(chartSeries('disk')).toEqual([
      { series: 'diskRead', label: 'Read' },
      { series: 'diskWrite', label: 'Write' },
    ])
    expect(chartSeries('net')).toEqual([
      { series: 'netRx', label: 'Down' },
      { series: 'netTx', label: 'Up' },
    ])
  })
})

describe('isPercentMetric', () => {
  it('is true for percentage metrics', () => {
    expect(isPercentMetric('cpu')).toBe(true)
    expect(isPercentMetric('gpu')).toBe(true)
  })

  it('is false for byte/throughput metrics', () => {
    expect(isPercentMetric('mem')).toBe(false)
    expect(isPercentMetric('disk')).toBe(false)
    expect(isPercentMetric('net')).toBe(false)
  })
})

describe('alignSeries', () => {
  it('returns [] for empty input', () => {
    expect(alignSeries([])).toEqual([])
  })

  it('leaves equal-length columns unchanged', () => {
    expect(
      alignSeries([
        [1, 2, 3],
        [4, 5, 6],
      ]),
    ).toEqual([
      [1, 2, 3],
      [4, 5, 6],
    ])
  })

  it('truncates to the shortest, keeping the most recent points', () => {
    expect(
      alignSeries([
        [1, 2, 3, 4],
        [5, 6],
      ]),
    ).toEqual([
      [3, 4],
      [5, 6],
    ])
  })

  it('collapses to empty columns when one column is empty', () => {
    expect(alignSeries([[1, 2], []])).toEqual([[], []])
  })
})

describe('ringFraction', () => {
  it('returns 0 for null', () => {
    expect(ringFraction(null, 100)).toBe(0)
  })

  it('returns the fraction within range', () => {
    expect(ringFraction(50, 100)).toBe(0.5)
  })

  it('clamps above max to 1', () => {
    expect(ringFraction(200, 100)).toBe(1)
  })

  it('clamps below 0 to 0', () => {
    expect(ringFraction(-5, 100)).toBe(0)
  })
})

describe('sparklineMax', () => {
  it('returns floor for an empty window', () => {
    expect(sparklineMax([], 1)).toBe(1)
  })

  it('returns floor when all values are below it', () => {
    expect(sparklineMax([0, 0.2, 0.5], 1)).toBe(1)
  })

  it('returns the window max when a value exceeds floor', () => {
    expect(sparklineMax([0, 5, 2], 1)).toBe(5)
  })

  it('ignores negatives and stays at floor', () => {
    expect(sparklineMax([-10, -3], 1)).toBe(1)
  })
})

describe('tipPlacement', () => {
  it('centers on the marker and anchors to the top when there is room', () => {
    expect(tipPlacement(100, 80, 80, 0, 200, 30, 56)).toEqual({
      boxX: 100,
      caretX: 100,
      anchorY: 80,
      flip: false,
    })
  })

  it('clamps the box left while the caret keeps tracking the marker', () => {
    const p = tipPlacement(10, 60, 60, 0, 200, 30, 56)
    expect(p.boxX).toBe(30)
    expect(p.caretX).toBe(10)
  })

  it('clamps the box right while the caret keeps tracking the marker', () => {
    const p = tipPlacement(190, 60, 60, 0, 200, 30, 56)
    expect(p.boxX).toBe(170)
    expect(p.caretX).toBe(190)
  })

  it('flips below and anchors to the lowest marker near the ceiling', () => {
    expect(tipPlacement(100, 20, 90, 0, 200, 30, 56)).toEqual({
      boxX: 100,
      caretX: 100,
      anchorY: 90,
      flip: true,
    })
  })

  it('falls back to the marker x when the plot is too narrow to clamp', () => {
    const p = tipPlacement(40, 60, 60, 30, 70, 30, 56)
    expect(p.boxX).toBe(40)
  })
})

describe('sparklinePoints', () => {
  it('returns empty string for no data', () => {
    expect(sparklinePoints([], 240, 40, 100)).toBe('')
  })

  it('maps oldest->newest with inverted y', () => {
    const points = sparklinePoints([0, 100], 240, 40, 100)
    const pairs = points.split(' ').map((p) => p.split(',').map(Number))

    expect(pairs).toHaveLength(2)
    // oldest value (0) sits at the bottom (y = height)
    expect(pairs[0][0]).toBe(0)
    expect(pairs[0][1]).toBe(40)
    // newest value (100) sits at the top (y = 0)
    expect(pairs[1][0]).toBe(240)
    expect(pairs[1][1]).toBe(0)
  })
})
