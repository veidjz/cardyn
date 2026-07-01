<script lang="ts">
  import { ringFraction } from '$lib/chart'
  import { formatPercent } from '$lib/format'

  let {
    value,
    max = 100,
    color = 'var(--cpu)',
    size = 108,
    stroke = 9,
    fill = false,
  }: {
    value: number | null
    max?: number
    color?: string
    size?: number
    stroke?: number
    fill?: boolean
  } = $props()

  const center = $derived(size / 2)
  const radius = $derived((size - stroke) / 2)
  const circumference = $derived(2 * Math.PI * radius)
  const arc = $derived(ringFraction(value, max) * circumference)
</script>

<div class="ring" class:fill style="--rs: {size}px;">
  <svg
    width="100%"
    height="100%"
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
  <div class="label" class:muted={value === null}>
    {formatPercent(value)}
  </div>
</div>

<style>
  .ring {
    position: relative;
    display: grid;
    place-items: center;
    width: var(--rs);
    height: var(--rs);
  }

  .ring.fill {
    width: var(--rsz, var(--rs));
    height: var(--rsz, var(--rs));
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
    font-size: calc(var(--rs) * 0.2);
  }

  .ring.fill .label {
    font-size: calc(var(--rsz, var(--rs)) * 0.2);
  }

  .label.muted {
    color: var(--muted);
  }
</style>
