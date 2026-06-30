<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { metrics } from '$lib/metrics.svelte'
  import { metricMeta } from '$lib/metric-meta'
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
  const meta = $derived(metricMeta[metric])

  // Y-axis tick + tooltip value formatter, by the unit of this metric's series.
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
  // Feeds the centered focus card.
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
    const wasPinned = pinnedIdx !== null
    pinnedIdx = idx
    if (!wasPinned) {
      window.addEventListener('keydown', onKeydown)
      document.addEventListener('click', onOutside)
    }
    u.redraw()
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') closeInspect()
  }

  // Close on a click anywhere outside the chart container. Clicks on the plot
  // re-pin via onChartClick (the focus card sits outside, so a click there closes).
  function onOutside(e: MouseEvent): void {
    if (!el.contains(e.target as Node)) closeInspect()
  }

  // Exit inspect mode: drop the highlight/card and resume live appends.
  function closeInspect(): void {
    pinnedIdx = null
    window.removeEventListener('keydown', onKeydown)
    document.removeEventListener('click', onOutside)
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
      legend: { show: false },
      scales: {
        x: { time: true },
        y: isPercentMetric(metric) ? { range: [0, 100] } : {},
      },
      series: [
        {},
        ...series.map((s, i) => ({
          label: s.label,
          stroke: accent,
          width: 1.5,
          dash: i > 0 ? [4, 4] : undefined,
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
        // Pinpoint-inspect highlight: one simple filled accent dot per series at
        // the pinned x, with a thin --bg outline ring for legibility over the
        // line. valToPos(_, true) returns DEVICE px (the ctx is unscaled), so
        // cx/cy are already device px; only radii / line widths are scaled by pr.
        draw: [
          (chart: uPlot) => {
            const idx = pinnedIdx
            if (idx === null || idx < 0 || idx >= chart.data[0].length) return
            const { ctx } = chart
            const pr = UPlot.pxRatio
            const cx = Math.round(chart.valToPos(chart.data[0][idx], 'x', true))
            for (let s = 1; s < chart.data.length; s++) {
              const y = chart.data[s][idx]
              if (y == null) continue
              const cy = Math.round(chart.valToPos(y, 'y', true))
              ctx.save()
              ctx.beginPath()
              ctx.strokeStyle = bg
              ctx.lineWidth = 2 * pr
              ctx.arc(cx, cy, 4 * pr, 0, Math.PI * 2)
              ctx.stroke()
              ctx.beginPath()
              ctx.fillStyle = accent
              ctx.arc(cx, cy, 4 * pr, 0, Math.PI * 2)
              ctx.fill()
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
    window.removeEventListener('keydown', onKeydown)
    document.removeEventListener('click', onOutside)
    u?.destroy()
  })
</script>

<div class="chart" bind:this={el}></div>

{#if detail}
  <div class="focus" style="--accent: var(--{metric})">
    <div class="focus-card">
      <div class="focus-head">
        <span class="focus-id">
          <span class="focus-dot"></span>
          <span class="focus-name">{meta.label}</span>
        </span>
        <button
          class="close"
          type="button"
          aria-label="Close"
          onclick={closeInspect}>✕</button
        >
      </div>
      {#if detail.rows.length === 1}
        <div class="focus-body single">
          <span class="hero">{detail.rows[0].value}</span>
          <span class="caption">{detail.time}</span>
        </div>
      {:else}
        <div class="focus-body multi">
          <div class="cols">
            {#each detail.rows as row, i (row.label)}
              <div class="col">
                <span class="swatch" class:dashed={i > 0}></span>
                <span class="col-label">{row.label}</span>
                <span class="col-value">{row.value}</span>
              </div>
            {/each}
          </div>
          <span class="caption">{detail.time}</span>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  @keyframes inspect-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .chart {
    width: 100%;
    position: relative;
    overflow: visible;
  }

  /* Centered focus card ----------------------------------------------- */
  .focus {
    display: flex;
    justify-content: center;
    margin-top: 8px;
  }

  .focus-card {
    position: relative;
    min-width: 180px;
    max-width: 300px;
    padding: 12px 16px 11px;
    background: var(--panel);
    border: 1px solid var(--hair);
    border-top: 2px solid var(--accent);
    border-radius: 10px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
    animation: inspect-in 120ms ease-out;
  }

  .focus-card::before {
    content: '';
    position: absolute;
    top: -8px;
    left: 50%;
    transform: translateX(-50%);
    width: 0;
    height: 0;
    border-left: 8px solid transparent;
    border-right: 8px solid transparent;
    border-bottom: 8px solid var(--accent);
  }

  .focus-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }

  .focus-id {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .focus-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
  }

  .focus-name {
    color: var(--text);
    font-size: 0.75rem;
    letter-spacing: 0.02em;
  }

  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    margin: -6px -8px -6px 0;
    appearance: none;
    background: transparent;
    border: none;
    color: var(--muted);
    font: inherit;
    line-height: 1;
    cursor: pointer;
  }

  .close:hover {
    color: var(--text);
  }

  .focus-body {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }

  .hero {
    font-size: 1.5rem;
    font-weight: 650;
    color: var(--accent);
    font-variant-numeric: tabular-nums;
  }

  .caption {
    font-size: 0.72rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .cols {
    display: flex;
    justify-content: center;
    gap: 24px;
    margin-bottom: 6px;
  }

  .col {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
  }

  .swatch {
    width: 10px;
    height: 0;
    border-top: 2px solid var(--accent);
  }

  .swatch.dashed {
    border-top-style: dashed;
  }

  .col-label {
    color: var(--muted);
    font-size: 0.7rem;
  }

  .col-value {
    font-size: 1.05rem;
    font-weight: 600;
    color: var(--accent);
    font-variant-numeric: tabular-nums;
  }
</style>
