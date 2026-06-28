export interface MetricsSnapshot {
  cpuTotal: number
  cpuPerCore: number[]
  cpuFreqMhz: number | null
  tsMs: number
}
