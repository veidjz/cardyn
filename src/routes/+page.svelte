<script lang="ts">
  import { onMount } from 'svelte'
  import { metrics, startMetrics } from '$lib/metrics.svelte'
  import {
    formatPercent,
    formatFreq,
    formatBytes,
    formatBps,
  } from '$lib/format'
  import { sparklineMax } from '$lib/chart'
  import Ring from '$lib/components/Ring.svelte'
  import Sparkline from '$lib/components/Sparkline.svelte'
  import Detail from '$lib/components/Detail.svelte'
  import type { MetricKey } from '$lib/types'

  let route = $state<'main' | { detail: MetricKey }>('main')

  const snap = $derived(metrics.latest)

  // CPU
  const cpu = $derived(snap?.cpuTotal ?? null)
  const cores = $derived(snap?.cpuPerCore.length ?? null)
  const freq = $derived(snap?.cpuFreqMhz ?? null)

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

  onMount(() => {
    let unlisten: (() => void) | undefined
    startMetrics().then((stop) => {
      unlisten = stop
    })
    return () => unlisten?.()
  })
</script>

<main class="app">
  {#if route === 'main'}
    <div class="grid">
      <!-- CPU -->
      <button
        class="card"
        type="button"
        onclick={() => (route = { detail: 'cpu' })}
      >
        <header class="head">
          <span class="dot" style="background: var(--cpu);"></span>
          <span class="title">CPU</span>
        </header>
        <div class="primary">
          <Ring value={cpu} color="var(--cpu)" />
        </div>
        <p class="sub">
          {cores === null ? '--' : cores} cores · {formatFreq(freq)}
        </p>
        <Sparkline
          data={metrics.history.cpu}
          color="var(--cpu)"
          max={100}
          height={34}
        />
      </button>

      <!-- Memory -->
      <button
        class="card"
        type="button"
        onclick={() => (route = { detail: 'mem' })}
      >
        <header class="head">
          <span class="dot" style="background: var(--mem);"></span>
          <span class="title">Memory</span>
        </header>
        <div class="primary">
          <Ring value={memPct} color="var(--mem)" />
        </div>
        <p class="sub">
          {formatBytes(snap?.memUsed ?? null)} / {formatBytes(
            snap?.memTotal ?? null,
          )}
        </p>
        <Sparkline
          data={metrics.history.mem}
          color="var(--mem)"
          max={100}
          height={34}
        />
      </button>

      <!-- GPU -->
      <button
        class="card"
        type="button"
        onclick={() => (route = { detail: 'gpu' })}
      >
        <header class="head">
          <span class="dot" style="background: var(--gpu);"></span>
          <span class="title">GPU</span>
        </header>
        {#if gpuNa}
          <div class="primary">
            <span class="big muted">N/A</span>
          </div>
          <p class="sub">--</p>
        {:else if vram === null}
          <div class="primary">
            <span class="big" class:muted={gpuMem === null}
              >{formatBytes(gpuMem)}</span
            >
          </div>
          <p class="sub">
            {gpuUtil !== null ? formatPercent(gpuUtil) + ' util' : '--'}
          </p>
          <Sparkline
            data={metrics.history.gpu}
            color="var(--gpu)"
            max={100}
            height={34}
          />
        {:else}
          <div class="primary">
            <Ring value={gpuUtil} color="var(--gpu)" />
          </div>
          <p class="sub">
            {formatBytes(gpuMem)} / {formatBytes(vram)}
          </p>
          <Sparkline
            data={metrics.history.gpu}
            color="var(--gpu)"
            max={100}
            height={34}
          />
        {/if}
      </button>

      <!-- Disk -->
      <button
        class="card"
        type="button"
        onclick={() => (route = { detail: 'disk' })}
      >
        <header class="head">
          <span class="dot" style="background: var(--disk);"></span>
          <span class="title">Disk</span>
        </header>
        <div class="primary">
          <Ring value={diskPct} color="var(--disk)" />
        </div>
        <p class="sub">
          {snap && snap.diskTotal > 0
            ? formatBytes(snap.diskUsed) + ' / ' + formatBytes(snap.diskTotal)
            : '--'}
        </p>
        <Sparkline
          data={metrics.history.disk}
          color="var(--disk)"
          max={sparklineMax(metrics.history.disk, 1)}
          height={34}
        />
      </button>

      <!-- Network -->
      <button
        class="card"
        type="button"
        onclick={() => (route = { detail: 'net' })}
      >
        <header class="head">
          <span class="dot" style="background: var(--net);"></span>
          <span class="title">Network</span>
        </header>
        <div class="primary">
          <span class="big" class:muted={netTotal === null}
            >{formatBps(netTotal)}</span
          >
        </div>
        <p class="sub">
          ↓ {formatBps(snap?.netRxBps ?? null)} &nbsp;&nbsp; ↑ {formatBps(
            snap?.netTxBps ?? null,
          )}
        </p>
        <Sparkline
          data={metrics.history.net}
          color="var(--net)"
          max={sparklineMax(metrics.history.net, 1)}
          height={34}
        />
      </button>
    </div>
  {:else}
    <Detail metric={route.detail} onBack={() => (route = 'main')} />
  {/if}
</main>

<style>
  .app {
    height: 100vh;
    box-sizing: border-box;
    padding: 18px;
    display: grid;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    grid-auto-rows: 1fr;
    gap: 16px;
    height: 100%;
  }

  .card {
    appearance: none;
    box-sizing: border-box;
    width: 100%;
    font: inherit;
    color: inherit;
    text-align: inherit;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 16px;
    background: var(--panel);
    border: 1px solid var(--hair);
    border-radius: 16px;
    transition: transform 0.15s ease;
  }

  .card:hover {
    transform: translateY(-2px);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    align-self: flex-start;
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }

  .title {
    font-weight: 600;
    letter-spacing: 0.02em;
  }

  .primary {
    display: grid;
    place-items: center;
    min-height: 96px;
  }

  .big {
    font-size: 1.4rem;
    font-weight: 600;
    color: var(--text);
  }

  .big.muted {
    color: var(--muted);
  }

  .sub {
    margin: 0;
    color: var(--muted);
    font-size: 0.8rem;
  }
</style>
