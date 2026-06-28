export function ringFraction(value: number | null, max: number): number {
  if (value === null) return 0
  const frac = value / max
  if (frac < 0) return 0
  if (frac > 1) return 1
  return frac
}

export function sparklinePoints(
  values: number[],
  width: number,
  height: number,
  max: number,
): string {
  const n = values.length
  if (n === 0) return ''

  const step = n > 1 ? width / (n - 1) : 0

  return values
    .map((v, i) => {
      const clamped = v < 0 ? 0 : v > max ? max : v
      const x = i * step
      const y = height - (clamped / max) * height
      return `${x},${y}`
    })
    .join(' ')
}
