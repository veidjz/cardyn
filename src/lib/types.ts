export interface GpuSample {
  utilization: number | null
  memUsed: number | null
  vramTotal: number | null
}

export interface MetricsSnapshot {
  cpuTotal: number
  cpuPerCore: number[]
  cpuFreqMhz: number | null
  memUsed: number
  memTotal: number
  memAvailable: number
  memFree: number
  swapUsed: number
  swapTotal: number
  gpu: GpuSample
  diskUsed: number
  diskTotal: number
  diskReadBps: number
  diskWriteBps: number
  netRxBps: number
  netTxBps: number
  tsMs: number
}

export type HistoryMetric =
  | 'cpu'
  | 'mem'
  | 'gpuUtil'
  | 'gpuMem'
  | 'diskRead'
  | 'diskWrite'
  | 'netRx'
  | 'netTx'

export interface History {
  t: number[]
  v: number[]
}
