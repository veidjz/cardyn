<script lang="ts">
  import { onMount, onDestroy, untrack, tick } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { metrics } from '$lib/metrics.svelte'
  import {
    chartSeries,
    alignSeries,
    isPercentMetric,
    tipPlacement,
    type TipPlacement,
  } from '$lib/chart'
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

  // The floating tooltip: its placement (computed in the click handler and the
  // ResizeObserver, never in a draw hook) and its measured element. `tipW`/`tipH`
  // start as a sound estimate and are corrected from the real box on the next
  // tick so the helper can place + clamp it precisely.
  let tip = $state<TipPlacement | null>(null)
  let tipEl = $state<HTMLDivElement | undefined>(undefined)
  let tipW = 120
  let tipH = 50
  // Assigned in init() so it closes over the live uPlot instance + pxRatio.
  let recomputeTip: (() => void) | null = null

  // Detail of the inspected point: time + one row per series (label + value).
  // Recomputed when the pinned index changes; `data` is frozen while inspecting.
  // Feeds the floating tooltip.
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

  function onChartClick(e: MouseEvent): void {
    if (!u) return
    // Clicks within the tooltip (incl. the close control) never re-pin.
    if (tipEl?.contains(e.target as Node)) return
    const idx = u.cursor.idx
    if (idx == null) return
    // Click a point to inspect/freeze; clicking another moves the lock.
    const wasPinned = pinnedIdx !== null
    pinnedIdx = idx
    recomputeTip?.()
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
  // re-pin via onChartClick; clicks on the tooltip are guarded there.
  function onOutside(e: MouseEvent): void {
    if (!el.contains(e.target as Node)) closeInspect()
  }

  function onCloseClick(e: MouseEvent): void {
    // Don't bubble to onChartClick (re-pin) or onOutside (close).
    e.stopPropagation()
    closeInspect()
  }

  // Exit inspect mode: drop the highlight/tooltip and resume live appends.
  function closeInspect(): void {
    pinnedIdx = null
    tip = null
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

    // Anchor the tooltip at the pinned sample, in CSS px relative to `.chart`.
    // valToPos(_, false) is CSS px relative to the plot area; add the plot-area
    // offset (bbox is device px -> /pr) to reach `.chart` coords. Place with the
    // current size estimate, then correct from the measured box on the next tick.
    recomputeTip = async () => {
      const chart = u
      if (!chart || pinnedIdx === null) return
      const idx = pinnedIdx
      const pr = UPlot.pxRatio
      const offX = chart.bbox.left / pr
      const offY = chart.bbox.top / pr
      const cx = chart.valToPos(data[0][idx], 'x', false) + offX
      // Anchor at the topmost marker so an "above" tooltip clears all series.
      let cy = Infinity
      for (let s = 1; s < data.length; s++) {
        const y = data[s][idx]
        if (y == null) continue
        const py = chart.valToPos(y, 'y', false) + offY
        if (py < cy) cy = py
      }
      if (cy === Infinity) {
        tip = null
        return
      }
      const bounds = { left: 0, top: 0, right: el.clientWidth, bottom: HEIGHT }
      tip = tipPlacement(cx, cy, tipW, tipH, bounds)
      await tick()
      if (pinnedIdx !== idx || !tipEl) return
      const r = tipEl.getBoundingClientRect()
      tipW = r.width
      tipH = r.height
      tip = tipPlacement(cx, cy, tipW, tipH, bounds)
    }

    ro = new ResizeObserver(() => {
      u?.setSize({ width: el.clientWidth, height: HEIGHT })
      recomputeTip?.()
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

<div class="chart" bind:this={el} style="--accent: var(--{metric})">
  {#if detail && tip}
    <div
      bind:this={tipEl}
      class="tip {tip.side}"
      style="left: {tip.left}px; top: {tip.top}px;"
    >
      <button
        class="tip-close"
        type="button"
        aria-label="Close"
        onclick={onCloseClick}>✕</button
      >
      {#if detail.rows.length === 1}
        <span class="tip-value">{detail.rows[0].value}</span>
        <span class="tip-time">{detail.time}</span>
      {:else}
        <span class="tip-time tip-head">{detail.time}</span>
        {#each detail.rows as row, i (row.label)}
          <span class="tip-row">
            <span class="glyph" class:hollow={i > 0}></span>
            <span class="tip-rowval">{row.value}</span>
          </span>
        {/each}
      {/if}
    </div>
    <div
      class="caret {tip.side}"
      style="left: {tip.caretLeft}px; top: {tip.caretTop}px;"
    ></div>
  {/if}
</div>

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

  /* At-point tooltip --------------------------------------------------- */
  .tip {
    position: absolute;
    z-index: 2;
    pointer-events: auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1px;
    padding: 6px 8px;
    background: var(--panel);
    border: 1px solid var(--hair);
    border-radius: 6px;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.45);
    font-size: 0.72rem;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    animation: inspect-in 120ms ease-out;
  }

  .tip-close {
    align-self: flex-end;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    margin: -2px -4px 0 10px;
    padding: 0;
    appearance: none;
    background: transparent;
    border: none;
    color: var(--muted);
    font: inherit;
    font-size: 0.7rem;
    line-height: 1;
    cursor: pointer;
  }

  .tip-close:hover {
    color: var(--text);
  }

  .tip-value {
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--accent);
  }

  .tip-time {
    font-size: 0.66rem;
    color: var(--muted);
  }

  .tip-head {
    margin-bottom: 3px;
  }

  .tip-row {
    display: flex;
    align-items: center;
    gap: 6px;
    align-self: stretch;
  }

  .glyph {
    width: 6px;
    height: 6px;
    flex: none;
    background: var(--accent);
  }

  .glyph.hollow {
    background: transparent;
    border: 1px solid var(--accent);
  }

  .tip-rowval {
    font-size: 0.72rem;
    color: var(--text);
  }

  /* Caret: a small triangle on the box edge pointing back at the marker. ---- */
  .caret {
    position: absolute;
    z-index: 2;
    pointer-events: none;
    width: 0;
    height: 0;
    animation: inspect-in 120ms ease-out;
  }

  .caret.above {
    border-left: 6px solid transparent;
    border-right: 6px solid transparent;
    border-top: 6px solid var(--panel);
    transform: translate(-50%, 0);
  }

  .caret.below {
    border-left: 6px solid transparent;
    border-right: 6px solid transparent;
    border-bottom: 6px solid var(--panel);
    transform: translate(-50%, -100%);
  }

  .caret.right {
    border-top: 6px solid transparent;
    border-bottom: 6px solid transparent;
    border-right: 6px solid var(--panel);
    transform: translate(-100%, -50%);
  }

  .caret.left {
    border-top: 6px solid transparent;
    border-bottom: 6px solid transparent;
    border-left: 6px solid var(--panel);
    transform: translate(0, -50%);
  }
</style>
