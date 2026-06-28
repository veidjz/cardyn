import { describe, it, expect } from 'vitest'
import { ringFraction, sparklinePoints } from '../chart'

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
