<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { metrics } from '$lib/metrics.svelte'
  import { chartSeries, alignSeries } from '$lib/chart'
  import { formatPercent, formatBytes, formatBps } from '$lib/format'
  import type uPlot from 'uplot'
  import type { History, MetricKey, MetricsSnapshot } from '$lib/types'

  let { metric }: { metric: MetricKey } = $props()

  const series = $derived(chartSeries(metric))

  // Y-axis / legend value formatter, by the unit of this metric's series.
  const valueFmt = $derived(
    metric === 'cpu' || metric === 'gpu'
      ? formatPercent
      : metric === 'mem'
        ? formatBytes
        : formatBps,
  )

  // The live snapshot value(s) for this metric, one per series.
  function liveValues(snap: MetricsSnapshot): (number | null)[] {
    switch (metric) {
      case 'cpu':
        return [snap.cpuTotal]
      case 'mem':
        return [snap.memUsed]
      case 'gpu':
        return [snap.gpu.utilization]
      case 'disk':
        return [snap.diskReadBps, snap.diskWriteBps]
      case 'net':
        return [snap.netRxBps, snap.netTxBps]
    }
  }

  function cssVar(name: string): string {
    return getComputedStyle(document.documentElement)
      .getPropertyValue(name)
      .trim()
  }

  const HEIGHT = 160
  const WINDOW = 60

  let el: HTMLDivElement
  let u: uPlot | null = null
  let ro: ResizeObserver | null = null
  let destroyed = false
  // uPlot data matrix [xs, ...ys]; ys parallel to `series`.
  let data: number[][] = []

  onMount(() => {
    void init()
  })

  async function init(): Promise<void> {
    const UPlot = (await import('uplot')).default
    await import('uplot/dist/uPlot.min.css')
    if (destroyed) return

    // Backfill each series from the backend ring buffer.
    const histories = await Promise.all(
      series.map((s) => invoke<History>('get_history', { metric: s.series })),
    )
    if (destroyed) return

    const xs = histories[0].t.map((ms) => ms / 1000)
    const ys = histories.map((h) => h.v)
    data = alignSeries([xs, ...ys])

    const accent = cssVar(`--${metric}`)
    const muted = cssVar('--muted')
    const hair = cssVar('--hair')

    const opts: uPlot.Options = {
      width: el.clientWidth,
      height: HEIGHT,
      scales: { x: { time: true } },
      series: [
        {},
        ...series.map((s, i) => ({
          label: s.label,
          stroke: accent,
          width: 1.5,
          dash: i > 0 ? [4, 4] : undefined,
          value: (_u: uPlot, raw: number | null) => valueFmt(raw),
        })),
      ],
      axes: [
        {
          stroke: muted,
          grid: { stroke: hair, width: 1 },
          ticks: { stroke: hair, width: 1 },
        },
        {
          stroke: muted,
          grid: { stroke: hair, width: 1 },
          ticks: { stroke: hair, width: 1 },
          values: (_u: uPlot, splits: number[]) =>
            splits.map((v) => valueFmt(v)),
        },
      ],
    }

    u = new UPlot(opts, data as uPlot.AlignedData, el)

    ro = new ResizeObserver(() => {
      u?.setSize({ width: el.clientWidth, height: HEIGHT })
    })
    ro.observe(el)
  }

  // Live update: append the newest snapshot value(s) on each 1 Hz tick, cap the
  // window, then hand the data back to uPlot. No raf/animation loop.
  $effect(() => {
    const snap = metrics.latest
    if (!snap || !u) return
    const vals = liveValues(snap)
    if (vals.some((v) => v === null)) return
    data[0].push(snap.tsMs / 1000)
    vals.forEach((v, i) => data[i + 1].push(v as number))
    while (data[0].length > WINDOW) {
      data.forEach((col) => col.shift())
    }
    u.setData(data as uPlot.AlignedData)
  })

  onDestroy(() => {
    destroyed = true
    ro?.disconnect()
    u?.destroy()
  })
</script>

<div class="chart" bind:this={el}></div>

<style>
  .chart {
    width: 100%;
  }
</style>
