<script lang="ts">
  import { ringFraction } from '$lib/chart'
  import { formatPercent } from '$lib/format'

  let {
    value,
    max = 100,
    color = 'var(--cpu)',
    size = 108,
    stroke = 9,
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
  <svg
    width={size}
    height={size}
    viewBox="0 0 {size} {size}"
    aria-hidden="true"
  >
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
      stroke-linecap={arc > 0 ? 'round' : 'butt'}
      stroke-dasharray="{arc} {circumference}"
      transform="rotate(-90 {center} {center})"
    />
  </svg>
  <div
    class="label"
    class:muted={value === null}
    style="font-size: {size * 0.2}px"
  >
    {formatPercent(value)}
  </div>
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
    font-weight: 600;
    color: var(--text);
  }

  .label.muted {
    color: var(--muted);
  }
</style>
