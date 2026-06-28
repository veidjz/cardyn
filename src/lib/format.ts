const percentFmt = new Intl.NumberFormat('en-US', { maximumFractionDigits: 1 })

const freqFmt = new Intl.NumberFormat('en-US', {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
})

export function formatPercent(v: number | null): string {
  if (v === null) return '--'
  return `${percentFmt.format(v)}%`
}

export function formatFreq(mhz: number | null): string {
  if (mhz === null || mhz === 0) return '--'
  return `${freqFmt.format(mhz / 1000)} GHz`
}
