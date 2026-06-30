import { describe, it, expect } from 'vitest'
import { metricKeys, metricMeta } from '../metric-meta'

describe('metricKeys', () => {
  it('lists the five metric keys in card order', () => {
    expect(metricKeys).toEqual(['cpu', 'mem', 'gpu', 'disk', 'net'])
  })
})

describe('metricMeta', () => {
  it('has an entry for every metric key', () => {
    for (const key of metricKeys) {
      expect(metricMeta[key]).toBeDefined()
    }
  })

  it('maps each key to its label and accent color', () => {
    expect(metricMeta).toEqual({
      cpu: { label: 'CPU', color: 'var(--cpu)' },
      mem: { label: 'Memory', color: 'var(--mem)' },
      gpu: { label: 'GPU', color: 'var(--gpu)' },
      disk: { label: 'Disk', color: 'var(--disk)' },
      net: { label: 'Network', color: 'var(--net)' },
    })
  })
})
