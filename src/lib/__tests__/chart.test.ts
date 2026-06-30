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
  const wide = { left: 0, top: 0, right: 400, bottom: 200 }
  // Short container so both vertical sides can fail, forcing a horizontal pick.
  const short = { left: 0, top: 0, right: 400, bottom: 60 }

  it('places above and points the caret down at the marker when there is room', () => {
    expect(tipPlacement(200, 150, 100, 40, wide)).toEqual({
      side: 'above',
      left: 150,
      top: 100,
      caretLeft: 200,
      caretTop: 140,
    })
  })

  it('flips below the marker when there is no room above', () => {
    expect(tipPlacement(200, 20, 100, 40, wide)).toEqual({
      side: 'below',
      left: 150,
      top: 30,
      caretLeft: 200,
      caretTop: 30,
    })
  })

  it('places to the right when neither vertical side fits', () => {
    expect(tipPlacement(100, 30, 100, 40, short)).toEqual({
      side: 'right',
      left: 110,
      top: 10,
      caretLeft: 110,
      caretTop: 30,
    })
  })

  it('places to the left when right has no room either', () => {
    expect(tipPlacement(350, 30, 100, 40, short)).toEqual({
      side: 'left',
      left: 240,
      top: 10,
      caretLeft: 340,
      caretTop: 30,
    })
  })

  it('clamps the box to the left edge while the caret tracks the marker', () => {
    const p = tipPlacement(10, 150, 100, 40, wide)
    expect(p.side).toBe('above')
    expect(p.left).toBe(4)
    expect(p.caretLeft).toBe(10)
  })

  it('clamps the box to the right edge while the caret tracks the marker', () => {
    const p = tipPlacement(395, 150, 100, 40, wide)
    expect(p.side).toBe('above')
    expect(p.left).toBe(296)
    expect(p.caretLeft).toBe(395)
  })

  it('falls back to the side with the most space when none fit', () => {
    const tiny = { left: 0, top: 0, right: 50, bottom: 50 }
    expect(tipPlacement(25, 10, 100, 40, tiny).side).toBe('below')
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
