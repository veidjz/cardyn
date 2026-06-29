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
