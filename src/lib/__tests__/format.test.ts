import { describe, it, expect } from 'vitest'
import {
  formatPercent,
  formatFreq,
  formatBytes,
  formatBps,
  formatClock,
} from '../format'

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

describe('formatBytes', () => {
  it('returns -- for null', () => {
    expect(formatBytes(null)).toBe('--')
  })

  it('formats 0 as 0 B', () => {
    expect(formatBytes(0)).toBe('0 B')
  })

  it('clamps negative to 0 B', () => {
    expect(formatBytes(-1)).toBe('0 B')
  })

  it('formats sub-1000 bytes as integer B', () => {
    expect(formatBytes(512)).toBe('512 B')
  })

  it('scales to KB with one decimal', () => {
    expect(formatBytes(524_288)).toBe('524.3 KB')
  })

  it('scales to MB with one decimal', () => {
    expect(formatBytes(2_097_152)).toBe('2.1 MB')
  })

  it('scales to GB with one decimal', () => {
    expect(formatBytes(16_000_000_000)).toBe('16.0 GB')
    expect(formatBytes(1_800_000_000)).toBe('1.8 GB')
    expect(formatBytes(8_200_000_000)).toBe('8.2 GB')
  })
})

describe('formatBps', () => {
  it('returns -- for null', () => {
    expect(formatBps(null)).toBe('--')
  })

  it('formats 0 as 0 B/s', () => {
    expect(formatBps(0)).toBe('0 B/s')
  })

  it('appends /s to KB', () => {
    expect(formatBps(131_072)).toBe('131.1 KB/s')
  })

  it('appends /s to MB', () => {
    expect(formatBps(2_097_152)).toBe('2.1 MB/s')
  })
})

describe('formatClock', () => {
  // Build the unix-seconds input from a local Date so the expectation holds in
  // any timezone (local components in, local components out).
  it('formats morning time as 12-hour with am and no hour pad', () => {
    const d = new Date(2026, 0, 2, 3, 4, 5)
    expect(formatClock(d.getTime() / 1000)).toBe('3:04:05 am')
  })

  it('formats afternoon time as 12-hour with pm', () => {
    const d = new Date(2026, 0, 1, 13, 25, 59)
    expect(formatClock(d.getTime() / 1000)).toBe('1:25:59 pm')
  })

  it('renders midnight as 12:00:00 am', () => {
    const d = new Date(2026, 0, 1, 0, 0, 0)
    expect(formatClock(d.getTime() / 1000)).toBe('12:00:00 am')
  })

  it('renders noon as 12:00:00 pm', () => {
    const d = new Date(2026, 0, 1, 12, 0, 0)
    expect(formatClock(d.getTime() / 1000)).toBe('12:00:00 pm')
  })

  it('keeps minutes and seconds zero-padded', () => {
    const d = new Date(2026, 0, 1, 9, 3, 7)
    expect(formatClock(d.getTime() / 1000)).toBe('9:03:07 am')
  })
})
