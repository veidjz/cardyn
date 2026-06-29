import { describe, it, expect } from 'vitest'
import { formatPercent, formatFreq, formatBytes, formatBps } from '../format'

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
