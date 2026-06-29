<script lang="ts">
  import { metrics } from '$lib/metrics.svelte'
  import { metricMeta } from '$lib/metric-meta'
  import { formatBytes, formatBps } from '$lib/format'
  import Ring from '$lib/components/Ring.svelte'
  import HistoryChart from '$lib/components/HistoryChart.svelte'
  import type { MetricKey } from '$lib/types'

  let { metric, onBack }: { metric: MetricKey; onBack: () => void } = $props()

  const meta = $derived(metricMeta[metric])
  const snap = $derived(metrics.latest)

  // CPU
  const cpu = $derived(snap?.cpuTotal ?? null)

  // Memory
  const memPct = $derived(
    snap && snap.memTotal > 0 ? (snap.memUsed / snap.memTotal) * 100 : null,
  )

  // GPU
  const gpuUtil = $derived(snap?.gpu.utilization ?? null)
  const gpuMem = $derived(snap?.gpu.memUsed ?? null)
  const vram = $derived(snap?.gpu.vramTotal ?? null)
  const gpuNa = $derived(gpuUtil === null && gpuMem === null)

  // Disk
  const diskPct = $derived(
    snap && snap.diskTotal > 0 ? (snap.diskUsed / snap.diskTotal) * 100 : null,
  )

  // Network
  const netTotal = $derived(snap ? snap.netRxBps + snap.netTxBps : null)
</script>

<section class="detail">
  <header class="head">
    <button class="back" type="button" onclick={onBack}>‹ Back</button>
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
      {:else if vram !== null}
        <Ring value={gpuUtil} color="var(--gpu)" />
      {:else}
        <span class="big" class:muted={gpuMem === null}
          >{formatBytes(gpuMem)}</span
        >
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
</style>
