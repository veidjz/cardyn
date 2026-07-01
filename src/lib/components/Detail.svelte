<script lang="ts">
  import { metrics } from '$lib/metrics.svelte'
  import { metricMeta } from '$lib/metric-meta'
  import {
    formatBytes,
    formatBps,
    formatPercent,
    formatFreq,
  } from '$lib/format'
  import { ringFraction, memSegments } from '$lib/chart'
  import Ring from '$lib/components/Ring.svelte'
  import HistoryChart from '$lib/components/HistoryChart.svelte'
  import type { MetricKey } from '$lib/types'

  let { metric, onBack }: { metric: MetricKey; onBack: () => void } = $props()

  const meta = $derived(metricMeta[metric])
  const snap = $derived(metrics.latest)

  // CPU
  const cpu = $derived(snap?.cpuTotal ?? null)
  const cores = $derived(snap?.cpuPerCore ?? null)
  const freq = $derived(snap?.cpuFreqMhz ?? null)

  // Memory
  const memPct = $derived(
    snap && snap.memTotal > 0 ? (snap.memUsed / snap.memTotal) * 100 : null,
  )
  const seg = $derived(
    snap
      ? memSegments(
          snap.memUsed,
          snap.memAvailable,
          snap.memFree,
          snap.memTotal,
        )
      : null,
  )
  const swapPct = $derived(
    snap && snap.swapTotal > 0 ? (snap.swapUsed / snap.swapTotal) * 100 : null,
  )

  // GPU
  const gpuUtil = $derived(snap?.gpu.utilization ?? null)
  const gpuMem = $derived(snap?.gpu.memUsed ?? null)
  const vram = $derived(snap?.gpu.vramTotal ?? null)
  const gpuNa = $derived(gpuUtil === null)
  const vramPct = $derived(
    snap &&
      snap.gpu.vramTotal !== null &&
      snap.gpu.vramTotal > 0 &&
      snap.gpu.memUsed !== null
      ? (snap.gpu.memUsed / snap.gpu.vramTotal) * 100
      : null,
  )

  // Disk
  const diskPct = $derived(
    snap && snap.diskTotal > 0 ? (snap.diskUsed / snap.diskTotal) * 100 : null,
  )

  // Network
  const netTotal = $derived(snap ? snap.netRxBps + snap.netTxBps : null)

  // Move keyboard focus to the Back button on mount so keyboard/SR users land
  // inside the detail instead of at <body>.
  let backEl = $state<HTMLButtonElement | undefined>(undefined)
  $effect(() => {
    backEl?.focus()
  })
</script>

<section class="detail">
  <header class="head">
    <button class="back" type="button" bind:this={backEl} onclick={onBack}
      >‹ Back</button
    >
    <span class="dot" style="background: {meta.color};"></span>
    <h1 class="title">{meta.label}</h1>
  </header>

  <div class="primary">
    {#if metric === 'cpu'}
      <Ring value={cpu} color="var(--cpu)" />
    {:else if metric === 'mem'}
      <Ring value={memPct} color="var(--mem)" />
    {:else if metric === 'gpu'}
      {#if gpuNa}
        <span class="big muted">N/A</span>
      {:else}
        <div class="gpu-primary">
          <Ring value={gpuUtil} color="var(--gpu)" />
          <p class="sub">
            {vram === null
              ? formatBytes(gpuMem)
              : formatBytes(gpuMem) + ' / ' + formatBytes(vram)}
          </p>
        </div>
      {/if}
    {:else if metric === 'disk'}
      <Ring value={diskPct} color="var(--disk)" />
    {:else}
      <span class="big" class:muted={netTotal === null}
        >{formatBps(netTotal)}</span
      >
    {/if}
  </div>

  <HistoryChart {metric} />

  <div class="breakdown" style="--accent: var(--{metric})">
    {#if metric === 'cpu'}
      {#if cores && cores.length > 0}
        <div class="cores" aria-label="Per-core usage">
          {#each cores as pct, i (i)}
            <div class="core" title="Core {i}: {formatPercent(pct)}">
              <div
                class="core-fill"
                style="height: {ringFraction(pct, 100) * 100}%"
              ></div>
            </div>
          {/each}
        </div>
      {:else}
        <span class="value muted">--</span>
      {/if}
      <div class="row">
        <span class="label">Frequency</span>
        <span class="value">{formatFreq(freq)}</span>
      </div>
    {:else if metric === 'mem'}
      <div class="seg-bar" aria-hidden="true">
        {#if seg}
          <div class="seg used" style="width: {seg.used * 100}%"></div>
          <div class="seg avail" style="width: {seg.available * 100}%"></div>
          <div class="seg free" style="width: {seg.free * 100}%"></div>
        {/if}
      </div>
      <div class="legend">
        <div class="row">
          <span class="label"><span class="key used"></span>Used</span>
          <span class="value">{formatBytes(snap?.memUsed ?? null)}</span>
        </div>
        <div class="row">
          <span class="label"><span class="key avail"></span>Available</span>
          <span class="value">{formatBytes(snap?.memAvailable ?? null)}</span>
        </div>
        <div class="row">
          <span class="label"><span class="key free"></span>Free</span>
          <span class="value">{formatBytes(snap?.memFree ?? null)}</span>
        </div>
      </div>
      <div class="hr"></div>
      <div class="row">
        <span class="label">Swap</span>
        <span class="value"
          >{formatBytes(snap?.swapUsed ?? null)} / {formatBytes(
            snap?.swapTotal ?? null,
          )}</span
        >
      </div>
      <div class="bar">
        <div
          class="fill"
          style="width: {ringFraction(swapPct, 100) * 100}%"
        ></div>
      </div>
    {:else if metric === 'gpu'}
      {#if gpuMem === null}
        <div class="row">
          <span class="label">VRAM</span>
          <span class="value muted">N/A</span>
        </div>
      {:else if vram === null}
        <div class="row">
          <span class="label">VRAM</span>
          <span class="value">{formatBytes(gpuMem)}</span>
        </div>
      {:else}
        <div class="row">
          <span class="label">VRAM</span>
          <span class="value">{formatBytes(gpuMem)} / {formatBytes(vram)}</span>
        </div>
        <div class="bar">
          <div
            class="fill"
            style="width: {ringFraction(vramPct, 100) * 100}%"
          ></div>
        </div>
      {/if}
    {:else if metric === 'disk'}
      <div class="row">
        <span class="label">Read</span>
        <span class="value">{formatBps(snap?.diskReadBps ?? null)}</span>
      </div>
      <div class="row">
        <span class="label">Write</span>
        <span class="value">{formatBps(snap?.diskWriteBps ?? null)}</span>
      </div>
      <div class="hr"></div>
      <div class="row">
        <span class="label">Space</span>
        <span class="value"
          >{snap && snap.diskTotal > 0
            ? formatBytes(snap.diskUsed) + ' / ' + formatBytes(snap.diskTotal)
            : '--'}</span
        >
      </div>
      <div class="bar">
        <div
          class="fill"
          style="width: {ringFraction(diskPct, 100) * 100}%"
        ></div>
      </div>
    {:else}
      <div class="row">
        <span class="label">Download</span>
        <span class="value">↓ {formatBps(snap?.netRxBps ?? null)}</span>
      </div>
      <div class="row">
        <span class="label">Upload</span>
        <span class="value">↑ {formatBps(snap?.netTxBps ?? null)}</span>
      </div>
    {/if}
  </div>

  {#if metric === 'cpu' || metric === 'mem'}
    {@const procs =
      metric === 'cpu' ? (snap?.topByCpu ?? []) : (snap?.topByMem ?? [])}
    <div class="procs">
      <h2 class="proc-title">Top Processes</h2>
      {#if procs.length > 0}
        <table class="proc-table">
          <thead>
            <tr>
              <th class="name">Name</th>
              <th class="num cpu">CPU%</th>
              <th class="num mem">Memory</th>
            </tr>
          </thead>
          <tbody>
            {#each procs as p (p.pid)}
              <tr>
                <td class="name">{p.name}</td>
                <td class="num cpu" class:lead={metric === 'cpu'}
                  >{formatPercent(p.cpuPct)}</td
                >
                <td class="num mem" class:lead={metric === 'mem'}
                  >{formatBytes(p.memBytes)}</td
                >
              </tr>
            {/each}
          </tbody>
        </table>
      {:else}
        <span class="value muted">--</span>
      {/if}
    </div>
  {/if}
</section>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .back {
    appearance: none;
    background: var(--panel);
    border: 1px solid var(--hair);
    border-radius: 10px;
    color: var(--text);
    font: inherit;
    padding: 6px 12px;
    cursor: pointer;
  }

  .back:hover {
    border-color: var(--muted);
  }

  .back:focus-visible {
    outline: 2px solid var(--text);
    outline-offset: 2px;
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }

  .title {
    margin: 0;
    font-size: 1.1rem;
    font-weight: 600;
    letter-spacing: 0.02em;
  }

  .primary {
    display: grid;
    place-items: center;
    min-height: 140px;
  }

  .big {
    font-size: 2rem;
    font-weight: 600;
    color: var(--text);
  }

  .big.muted {
    color: var(--muted);
  }

  .gpu-primary {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  .sub {
    margin: 0;
    color: var(--muted);
    font-size: 0.8rem;
  }

  /* Per-metric breakdown ------------------------------------------------ */
  .breakdown {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: 16px;
    border-top: 1px solid var(--hair);
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .label {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    color: var(--muted);
    font-size: 0.8rem;
  }

  .value {
    color: var(--text);
    font-size: 0.85rem;
    font-variant-numeric: tabular-nums;
  }

  .value.muted {
    color: var(--muted);
  }

  .bar {
    height: 6px;
    background: var(--track);
    border-radius: 3px;
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 0.3s ease;
  }

  .hr {
    height: 1px;
    background: var(--hair);
  }

  /* Memory segmented bar + legend */
  .seg-bar {
    display: flex;
    height: 8px;
    background: var(--track);
    border-radius: 4px;
    overflow: hidden;
  }

  .seg {
    height: 100%;
  }

  .seg.used {
    background: var(--accent);
  }

  .seg.avail {
    background: var(--accent);
    opacity: 0.35;
  }

  .seg.free {
    background: transparent;
  }

  .legend {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .key {
    flex: none;
    width: 9px;
    height: 9px;
    border-radius: 2px;
  }

  .key.used {
    background: var(--accent);
  }

  .key.avail {
    background: var(--accent);
    opacity: 0.35;
  }

  .key.free {
    background: var(--track);
    border: 1px solid var(--hair);
  }

  /* CPU per-core equalizer */
  .cores {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
  }

  .core {
    flex: 1 1 5px;
    min-width: 3px;
    max-width: 18px;
    height: 44px;
    background: var(--track);
    border-radius: 2px;
    overflow: hidden;
    display: flex;
    align-items: flex-end;
  }

  .core-fill {
    width: 100%;
    background: var(--accent);
    transition: height 0.3s ease;
  }

  /* Top process table (cpu + mem only) */
  .procs {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 16px;
    border-top: 1px solid var(--hair);
  }

  .proc-title {
    margin: 0;
    font-size: 0.8rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--muted);
  }

  .proc-table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }

  .proc-table th {
    padding: 0 0 6px;
    font-size: 0.72rem;
    font-weight: 500;
    color: var(--muted);
    text-align: right;
    border-bottom: 1px solid var(--hair);
  }

  .proc-table td {
    padding: 5px 0;
    font-size: 0.85rem;
    color: var(--text);
  }

  .proc-table .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .proc-table td.num {
    color: var(--muted);
  }

  .proc-table td.num.lead {
    color: var(--text);
    font-weight: 500;
  }

  .proc-table .cpu {
    width: 58px;
  }

  .proc-table .mem {
    width: 82px;
  }

  .proc-table .name {
    padding-right: 12px;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
