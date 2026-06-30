import type { MetricKey } from './types'

// Drill-down order of the 5 main-screen cards (matches +page.svelte grid).
export const metricKeys: MetricKey[] = ['cpu', 'mem', 'gpu', 'disk', 'net']

// Per-metric display label + accent color CSS var (01-design-ui.md tokens).
export const metricMeta: Record<MetricKey, { label: string; color: string }> = {
  cpu: { label: 'CPU', color: 'var(--cpu)' },
  mem: { label: 'Memory', color: 'var(--mem)' },
  gpu: { label: 'GPU', color: 'var(--gpu)' },
  disk: { label: 'Disk', color: 'var(--disk)' },
  net: { label: 'Network', color: 'var(--net)' },
}
