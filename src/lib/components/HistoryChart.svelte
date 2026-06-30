<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { metrics } from '$lib/metrics.svelte'
  import { metricMeta } from '$lib/metric-meta'
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
  const meta = $derived(metricMeta[metric])

  // Y-axis tick + readout value formatter, by the unit of this metric's series.
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

  // Cursor sample index under the pointer (null when the pointer leaves the
  // plot), driving the hover/idle readout. `rev` bumps on each live append so
  // the hover-derived recomputes as the (non-reactive) `data` window scrolls.
  let hoveredIdx = $state<number | null>(null)
  let rev = $state(0)

  // Screen geometry of the at-point tooltip while pinned, in CSS px relative to
  // `.chart`. Computed in the click handler and the ResizeObserver (never in a
  // draw hook). Estimated tooltip half-width for edge clamping; upward reach
  // (box + caret + gap) that triggers the below-flip near the top.
  const TIP_HALF = 70
  const TIP_UP_REACH = 56
  let tip = $state<TipPlacement | null>(null)
  // Assigned in init() so it closes over the live uPlot instance + pxRatio.
  let recomputeTip: (() => void) | null = null

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

  // Quiet, always-meaningful readout strip under the chart. Precedence:
  // pinned -> hover -> idle. PINNED defers value+time to the focus card (it
  // prints labels only, never twice); HOVER tracks the cursor sample (accent
  // value + that point's clock); IDLE shows the muted live value + "now".
  type ReadoutRow = { label: string; value?: string }
  type Readout =
    | { mode: 'pinned'; rows: ReadoutRow[] }
    | { mode: 'hover'; time: string; rows: ReadoutRow[] }
    | { mode: 'idle'; rows: ReadoutRow[] }

  const readout = $derived.by((): Readout => {
    // Touch `rev` so the hover value stays fresh as the live window scrolls
    // (`data` is a plain array, not reactive). Idle reads metrics.latest below.
    void rev
    if (pinnedIdx !== null) {
      return { mode: 'pinned', rows: series.map((s) => ({ label: s.label })) }
    }
    if (hoveredIdx !== null && data[0] && hoveredIdx < data[0].length) {
      const idx = hoveredIdx
      return {
        mode: 'hover',
        time: formatClock(data[0][idx]),
        rows: series.map((s, i) => ({
          label: s.label,
          value: valueFmt(data[i + 1][idx]),
        })),
      }
    }
    const snap = metrics.latest
    const liveVals = snap ? liveValues(snap) : series.map(() => null)
    return {
      mode: 'idle',
      rows: series.map((s, i) => ({
        label: s.label,
        value: valueFmt(liveVals[i]),
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
    recomputeTip?.()
    if (!wasPinned) window.addEventListener('keydown', onKeydown)
    u.redraw()
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') closeInspect()
  }

  // Exit inspect mode: drop the highlight/tooltip/card and resume live appends.
  function closeInspect(): void {
    pinnedIdx = null
    tip = null
    window.removeEventListener('keydown', onKeydown)
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
        // Event-driven hover (no rAF): fires on every cursor move and sets
        // idx=null on leave, which is the idle trigger for the readout strip.
        setCursor: [
          (chart: uPlot) => {
            hoveredIdx = chart.cursor.idx ?? null
          },
        ],
        // Pinpoint-inspect highlight: a lit accent column + a solid accent
        // guide + a per-series bullseye at the pinned x. valToPos(_, true)
        // returns DEVICE px (the ctx is unscaled), so cx/cy are already device
        // px; only radii / line widths / the column width are scaled by pr.
        draw: [
          (chart: uPlot) => {
            const idx = pinnedIdx
            if (idx === null || idx < 0 || idx >= chart.data[0].length) return
            const { ctx } = chart
            const pr = UPlot.pxRatio
            const cx = Math.round(chart.valToPos(chart.data[0][idx], 'x', true))
            const top = Math.round(chart.bbox.top)
            const bottom = Math.round(chart.bbox.top + chart.bbox.height)

            // 1. Lit column: a faint accent slice centered on the pinned x.
            const spacingCss =
              chart.bbox.width / pr / Math.max(chart.data[0].length - 1, 1)
            const colW = Math.min(Math.max(spacingCss, 8), 16) * pr
            ctx.save()
            ctx.globalAlpha = 0.08
            ctx.fillStyle = accent
            ctx.fillRect(cx - colW / 2, top, colW, bottom - top)
            ctx.restore()

            // 2. Solid guide: a crisp accent line carrying the eye down.
            ctx.save()
            ctx.globalAlpha = 0.45
            ctx.strokeStyle = accent
            ctx.lineWidth = 1 * pr
            ctx.beginPath()
            ctx.moveTo(cx, top)
            ctx.lineTo(cx, bottom)
            ctx.stroke()
            ctx.restore()

            // 3. Bullseye per series: dark gap ring, accent halo, then a core
            // (filled for series 0, hollow ring for the dashed series 1).
            for (let s = 1; s < chart.data.length; s++) {
              const y = chart.data[s][idx]
              if (y == null) continue
              const cy = Math.round(chart.valToPos(y, 'y', true))
              ctx.save()
              ctx.beginPath()
              ctx.strokeStyle = bg
              ctx.lineWidth = 3 * pr
              ctx.arc(cx, cy, 6.5 * pr, 0, Math.PI * 2)
              ctx.stroke()
              ctx.globalAlpha = 0.55
              ctx.beginPath()
              ctx.strokeStyle = accent
              ctx.lineWidth = 1.5 * pr
              ctx.arc(cx, cy, 8 * pr, 0, Math.PI * 2)
              ctx.stroke()
              ctx.globalAlpha = 1
              ctx.beginPath()
              if (s > 1) {
                ctx.strokeStyle = accent
                ctx.lineWidth = 1.5 * pr
                ctx.arc(cx, cy, 5 * pr, 0, Math.PI * 2)
                ctx.stroke()
              } else {
                ctx.fillStyle = accent
                ctx.arc(cx, cy, 5 * pr, 0, Math.PI * 2)
                ctx.fill()
              }
              ctx.restore()
            }
          },
        ],
      },
    }

    u = new UPlot(opts, data as uPlot.AlignedData, el)
    el.addEventListener('click', onChartClick)

    // Anchor the tooltip + caret at the pinned sample, in CSS px relative to
    // `.chart`. valToPos(_, false) is CSS px relative to the plot area; add the
    // plot-area offset (bbox is device px -> /pr) to reach `.chart` coords.
    recomputeTip = () => {
      const chart = u
      if (!chart || pinnedIdx === null) return
      const idx = pinnedIdx
      const pr = UPlot.pxRatio
      const offX = chart.bbox.left / pr
      const offY = chart.bbox.top / pr
      const cx = chart.valToPos(data[0][idx], 'x', false) + offX
      let topY = Infinity
      let bottomY = -Infinity
      for (let s = 1; s < data.length; s++) {
        const y = data[s][idx]
        if (y == null) continue
        const cy = chart.valToPos(y, 'y', false) + offY
        if (cy < topY) topY = cy
        if (cy > bottomY) bottomY = cy
      }
      if (topY === Infinity) {
        tip = null
        return
      }
      const plotLeft = chart.bbox.left / pr
      const plotRight = (chart.bbox.left + chart.bbox.width) / pr
      tip = tipPlacement(
        cx,
        topY,
        bottomY,
        plotLeft,
        plotRight,
        TIP_HALF,
        TIP_UP_REACH,
      )
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
      rev++
    })
  })

  onDestroy(() => {
    destroyed = true
    ro?.disconnect()
    el?.removeEventListener('click', onChartClick)
    window.removeEventListener('keydown', onKeydown)
    u?.destroy()
  })
</script>

<div class="chart" bind:this={el} style="--accent: var(--{metric})">
  {#if detail && tip}
    <div
      class="tip"
      class:flip={tip.flip}
      style="left: {tip.boxX}px; top: {tip.anchorY}px;"
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
      class="caret"
      class:flip={tip.flip}
      style="left: {tip.caretX}px; top: {tip.anchorY}px;"
    ></div>
  {/if}
</div>

<div
  class="readout"
  class:hover={readout.mode === 'hover'}
  style="--accent: var(--{metric})"
>
  <div class="r-series">
    {#each readout.rows as row, i (row.label)}
      <span class="r-item">
        <span class="swatch" class:dashed={i > 0}></span>
        <span class="r-label">{row.label}</span>
        {#if readout.mode !== 'pinned'}
          <span class="r-value">{row.value}</span>
        {/if}
      </span>
    {/each}
  </div>
  <span class="r-time">
    {#if readout.mode === 'idle'}now{:else if readout.mode === 'hover'}{readout.time}{/if}
  </span>
</div>

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

  /* At-point tooltip --------------------------------------------------- */
  .tip {
    position: absolute;
    z-index: 2;
    pointer-events: none;
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
    transform: translate(-50%, calc(-100% - 10px));
    animation: inspect-in 120ms ease-out;
  }

  .tip.flip {
    transform: translate(-50%, 10px);
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

  .caret {
    position: absolute;
    z-index: 2;
    pointer-events: none;
    width: 0;
    height: 0;
    border-left: 6px solid transparent;
    border-right: 6px solid transparent;
    border-top: 6px solid var(--panel);
    transform: translate(-50%, -10px);
    animation: inspect-in 120ms ease-out;
  }

  .caret.flip {
    border-top: none;
    border-bottom: 6px solid var(--panel);
    transform: translate(-50%, 4px);
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

  /* Quiet 3-state readout strip --------------------------------------- */
  .readout {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin-top: 6px;
    padding: 0 2px;
    min-height: 1.15rem;
    font-size: 0.72rem;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }

  .r-series {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .r-item {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .r-label {
    color: var(--muted);
  }

  .r-value {
    color: var(--muted);
    font-weight: 500;
    transition: color 120ms ease-out;
  }

  .readout.hover .r-value {
    color: var(--accent);
    font-weight: 600;
  }

  .r-time {
    min-width: 4.5rem;
    text-align: right;
    font-size: 0.66rem;
    color: var(--muted);
  }

  .readout.hover .r-time {
    color: var(--text);
  }
</style>
