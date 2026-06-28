import { describe, it, expect } from 'vitest'
import { formatPercent, formatFreq } from '../format'

describe('formatPercent', () => {
  it('returns -- for null', () => {
    expect(formatPercent(null)).toBe('--')
  })

  it('rounds to at most one decimal and appends %', () => {
    expect(formatPercent(12.45)).toMatch(/^\d+(\.\d)?%$/)
  })

  it('formats 0 as 0%', () => {
    expect(formatPercent(0)).toBe('0%')
  })
})

describe('formatFreq', () => {
  it('returns -- for null', () => {
    expect(formatFreq(null)).toBe('--')
  })

  it('returns -- for 0', () => {
    expect(formatFreq(0)).toBe('--')
  })

  it('formats 4464 MHz as 4.5 GHz', () => {
    expect(formatFreq(4464)).toBe('4.5 GHz')
  })
})
