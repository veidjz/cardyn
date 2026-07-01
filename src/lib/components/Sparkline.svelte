<script lang="ts">
  import { sparklinePoints } from '$lib/chart'

  let {
    data,
    color = 'var(--cpu)',
    max = 100,
    width = 240,
    height = 40,
  }: {
    data: number[]
    color?: string
    max?: number
    width?: number
    height?: number
  } = $props()

  const points = $derived(sparklinePoints(data, width, height, max))
</script>

<svg
  class="sparkline"
  {width}
  {height}
  viewBox="0 0 {width} {height}"
  preserveAspectRatio="none"
  aria-hidden="true"
>
  <polyline
    {points}
    fill="none"
    stroke={color}
    stroke-width="2"
    stroke-linejoin="round"
    stroke-linecap="round"
    opacity="0.85"
  />
</svg>

<style>
  .sparkline {
    display: block;
    width: 100%;
    height: auto;
  }
</style>
