<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { metrics } from '$lib/metrics.svelte'
  import { chartSeries, alignSeries, isPercentMetric } from '$lib/chart'
  import {
    formatPercent,
    formatBytes,
    formatBps,
    formatClock,
  } from '$lib/format'
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

  // Index of the clicked/pinned data point (into `data`), or null when none.
  // Tracks its logical point as the window scrolls; cleared once it falls off.
  let pinned = $state<number | null>(null)
  // Readout derived from the pinned point's current data (time + per-series
  // value). Recomputed on click and on every live tick so it stays in sync.
  let readout = $state<{
    time: string
    rows: { label: string; value: string }[]
  } | null>(null)

  function computeReadout(): void {
    if (pinned === null || pinned < 0 || !data[0] || pinned >= data[0].length) {
      readout = null
      return
    }
    readout = {
      time: formatClock(data[0][pinned]),
      rows: series.map((s, i) => ({
        label: s.label,
        value: valueFmt(data[i + 1][pinned as number]),
      })),
    }
  }

  function onChartClick(): void {
    if (!u) return
    const idx = u.cursor.idx
    if (idx == null) return
    // Click the pinned point again to clear; otherwise re-pin.
    pinned = pinned === idx ? null : idx
    computeReadout()
    u.redraw()
  }

  function clearPin(): void {
    pinned = null
    readout = null
    u?.redraw()
  }

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
      scales: {
        x: { time: true },
        y: isPercentMetric(metric) ? { range: [0, 100] } : {},
      },
      series: [
        {
          value: (_u: uPlot, raw: number | null) =>
            raw == null ? '--' : formatClock(raw),
        },
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
          values: (_u: uPlot, splits: number[]) =>
            splits.map((v) => formatClock(v)),
        },
        {
          stroke: muted,
          grid: { stroke: hair, width: 1 },
          ticks: { stroke: hair, width: 1 },
          values: (_u: uPlot, splits: number[]) =>
            splits.map((v) => valueFmt(v)),
        },
      ],
      hooks: {
        // Subtle dashed vertical marker at the pinned point.
        draw: [
          (chart: uPlot) => {
            if (pinned === null || pinned < 0 || pinned >= chart.data[0].length)
              return
            const cx = Math.round(
              chart.valToPos(chart.data[0][pinned], 'x', true),
            )
            const { ctx } = chart
            ctx.save()
            ctx.beginPath()
            ctx.setLineDash([2, 2])
            ctx.lineWidth = 1
            ctx.strokeStyle = muted
            ctx.moveTo(cx, Math.round(chart.bbox.top))
            ctx.lineTo(cx, Math.round(chart.bbox.top + chart.bbox.height))
            ctx.stroke()
            ctx.restore()
          },
        ],
      },
    }

    u = new UPlot(opts, data as uPlot.AlignedData, el)
    el.addEventListener('click', onChartClick)

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
      // The pinned point shifts left with the data; clear it once it falls off.
      if (pinned !== null) {
        pinned -= 1
        if (pinned < 0) pinned = null
      }
    }
    u.setData(data as uPlot.AlignedData)
    computeReadout()
  })

  onDestroy(() => {
    destroyed = true
    ro?.disconnect()
    el?.removeEventListener('click', onChartClick)
    u?.destroy()
  })
</script>

<div class="chart" bind:this={el}></div>

{#if readout}
  <div class="readout">
    <span class="time">{readout.time}</span>
    <span class="vals">
      {#each readout.rows as row (row.label)}
        <span class="val"
          ><span class="label">{row.label}</span> {row.value}</span
        >
      {/each}
    </span>
    <button
      class="clear"
      type="button"
      aria-label="Clear pin"
      onclick={clearPin}>✕</button
    >
  </div>
{/if}

<style>
  .chart {
    width: 100%;
  }

  .readout {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 8px;
    padding: 6px 10px;
    background: var(--panel);
    border: 1px solid var(--hair);
    border-radius: 8px;
    font-size: 0.8rem;
  }

  .time {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .vals {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    color: var(--text);
  }

  .label {
    color: var(--muted);
    margin-right: 4px;
  }

  .clear {
    appearance: none;
    margin-left: auto;
    background: transparent;
    border: none;
    color: var(--muted);
    font: inherit;
    line-height: 1;
    padding: 2px 4px;
    cursor: pointer;
  }

  .clear:hover {
    color: var(--text);
  }
</style>
