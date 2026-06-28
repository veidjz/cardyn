<script lang="ts">
  import { ringFraction } from '$lib/chart'
  import { formatPercent } from '$lib/format'

  let {
    value,
    max = 100,
    color = 'var(--cpu)',
    size = 132,
    stroke = 12,
  }: {
    value: number | null
    max?: number
    color?: string
    size?: number
    stroke?: number
  } = $props()

  const center = $derived(size / 2)
  const radius = $derived((size - stroke) / 2)
  const circumference = $derived(2 * Math.PI * radius)
  const arc = $derived(ringFraction(value, max) * circumference)
</script>

<div class="ring" style="width: {size}px; height: {size}px;">
  <svg width={size} height={size} viewBox="0 0 {size} {size}">
    <circle
      cx={center}
      cy={center}
      r={radius}
      fill="none"
      stroke="var(--track)"
      stroke-width={stroke}
    />
    <circle
      class="arc"
      cx={center}
      cy={center}
      r={radius}
      fill="none"
      stroke={color}
      stroke-width={stroke}
      stroke-linecap="round"
      stroke-dasharray="{arc} {circumference}"
      transform="rotate(-90 {center} {center})"
    />
  </svg>
  <div class="label" class:muted={value === null}>{formatPercent(value)}</div>
</div>

<style>
  .ring {
    position: relative;
    display: grid;
    place-items: center;
  }

  svg {
    display: block;
  }

  .arc {
    transition: stroke-dasharray 0.3s ease;
  }

  .label {
    position: absolute;
    font-size: 1.6rem;
    font-weight: 600;
    color: var(--text);
  }

  .label.muted {
    color: var(--muted);
  }
</style>
