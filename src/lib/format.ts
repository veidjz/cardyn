const percentFmt = new Intl.NumberFormat('en-US', { maximumFractionDigits: 1 })

const freqFmt = new Intl.NumberFormat('en-US', {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
})

const byteFmt = new Intl.NumberFormat('en-US', {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
})

// Base-1000 ladder (Activity Monitor style: 1 GB = 1000 MB), capped at TB.
const byteUnits = ['B', 'KB', 'MB', 'GB', 'TB']

// Shared scaler for byte-like values. Negative inputs clamp to 0 ('0 B').
// Below 1000 -> integer + ' B'; otherwise scale by 1000 to one decimal place.
function scaleBytes(bytes: number): string {
  const n = bytes < 0 ? 0 : bytes
  if (n < 1000) return `${n} B`
  let value = n
  let i = 0
  while (value >= 1000 && i < byteUnits.length - 1) {
    value /= 1000
    i++
  }
  return `${byteFmt.format(value)} ${byteUnits[i]}`
}

export function formatPercent(v: number | null): string {
  if (v === null) return '--'
  return `${percentFmt.format(v)}%`
}

export function formatFreq(mhz: number | null): string {
  if (mhz === null || mhz === 0) return '--'
  return `${freqFmt.format(mhz / 1000)} GHz`
}

export function formatBytes(bytes: number | null): string {
  if (bytes === null) return '--'
  return scaleBytes(bytes)
}

export function formatBps(bytesPerSec: number | null): string {
  if (bytesPerSec === null) return '--'
  return `${scaleBytes(bytesPerSec)}/s`
}
