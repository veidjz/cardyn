<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte'
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
  // Live points captured while inspecting (frozen). Flushed on close so the
  // chart catches up to the current window. Capped at WINDOW.
  let buffer: number[][] = []

  // Index into `data` of the inspected point, or null when live. While not
  // null the chart is frozen on that point: live updates are buffered, not
  // appended, so the view (and this index) stay fixed on the selected moment.
  let pinnedIdx = $state<number | null>(null)

  // Detail of the inspected point: time + one row per series (label + value).
  // Recomputed when the pinned index changes; `data` is frozen while inspecting.
  const detail = $derived.by(() => {
    const idx = pinnedIdx
    if (idx === null || !data[0] || idx >= data[0].length) return null
    return {
      time: formatClock(data[0][idx]),
      rows: series.map((s, i) => ({
        label: s.label,
        value: valueFmt(data[i + 1][idx]),
      })),
    }
  })

  function appendRow(row: number[]): void {
    row.forEach((v, i) => data[i].push(v))
    while (data[0].length > WINDOW) data.forEach((col) => col.shift())
  }

  // Flush buffered live points into the chart and resume the live window.
  function flushBuffer(): void {
    if (!u) return
    buffer.forEach((row) => appendRow(row))
    buffer = []
    u.setData(data as uPlot.AlignedData)
  }

  function onChartClick(): void {
    if (!u) return
    const idx = u.cursor.idx
    if (idx == null) return
    // Click a point to inspect/freeze; clicking another moves the lock.
    pinnedIdx = idx
    u.redraw()
  }

  // Exit inspect mode: drop the highlight/panel and resume live appends.
  function closeInspect(): void {
    pinnedIdx = null
    flushBuffer()
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
    const bg = cssVar('--bg')

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
        // Highlight the inspected point: vertical guide + a filled marker on
        // every series at the pinned x.
        draw: [
          (chart: uPlot) => {
            const idx = pinnedIdx
            if (idx === null || idx < 0 || idx >= chart.data[0].length) return
            const { ctx } = chart
            const cx = Math.round(chart.valToPos(chart.data[0][idx], 'x', true))

            ctx.save()
            ctx.beginPath()
            ctx.setLineDash([2, 2])
            ctx.lineWidth = 1
            ctx.strokeStyle = muted
            ctx.moveTo(cx, Math.round(chart.bbox.top))
            ctx.lineTo(cx, Math.round(chart.bbox.top + chart.bbox.height))
            ctx.stroke()
            ctx.restore()

            for (let s = 1; s < chart.data.length; s++) {
              const y = chart.data[s][idx]
              if (y == null) continue
              const cy = Math.round(chart.valToPos(y, 'y', true))
              ctx.save()
              ctx.beginPath()
              ctx.fillStyle = accent
              ctx.strokeStyle = bg
              ctx.lineWidth = 2
              ctx.arc(cx, cy, 4, 0, Math.PI * 2)
              ctx.fill()
              ctx.stroke()
              ctx.restore()
            }
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

  // Live update on each 1 Hz tick. While inspecting (pinnedIdx not null) the
  // chart is frozen: buffer the point instead of appending so the view holds
  // on the selected moment. `untrack` keeps this effect tied only to
  // `metrics.latest`, not to pinnedIdx. No raf/animation loop.
  $effect(() => {
    const snap = metrics.latest
    untrack(() => {
      if (!snap || !u) return
      const vals = liveValues(snap)
      if (vals.some((v) => v === null)) return
      const row = [snap.tsMs / 1000, ...(vals as number[])]
      if (pinnedIdx !== null) {
        buffer.push(row)
        if (buffer.length > WINDOW) buffer.shift()
        return
      }
      appendRow(row)
      u.setData(data as uPlot.AlignedData)
    })
  })

  onDestroy(() => {
    destroyed = true
    ro?.disconnect()
    el?.removeEventListener('click', onChartClick)
    u?.destroy()
  })
</script>

<div class="chart" bind:this={el}></div>

{#if detail}
  <div class="detail" style="--accent: var(--{metric})">
    <div class="detail-head">
      <span class="detail-time">{detail.time}</span>
      <button
        class="close"
        type="button"
        aria-label="Close"
        onclick={closeInspect}>✕</button
      >
    </div>
    <div class="detail-rows">
      {#each detail.rows as row (row.label)}
        <div class="detail-row">
          <span class="detail-label">{row.label}</span>
          <span class="detail-value">{row.value}</span>
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .chart {
    width: 100%;
  }

  .detail {
    margin-top: 8px;
    padding: 10px 12px;
    background: var(--panel);
    border: 1px solid var(--hair);
    border-left: 3px solid var(--accent);
    border-radius: 8px;
    font-size: 0.8rem;
  }

  .detail-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  .detail-time {
    color: var(--accent);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .detail-rows {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .detail-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }

  .detail-label {
    color: var(--muted);
  }

  .detail-value {
    color: var(--text);
    font-variant-numeric: tabular-nums;
  }

  .close {
    appearance: none;
    background: transparent;
    border: none;
    color: var(--muted);
    font: inherit;
    line-height: 1;
    padding: 2px 4px;
    cursor: pointer;
  }

  .close:hover {
    color: var(--text);
  }
</style>
