<script lang="ts">
  import { onMount } from 'svelte'
  import { metrics, startMetrics } from '$lib/metrics.svelte'
  import { formatFreq } from '$lib/format'
  import Ring from '$lib/components/Ring.svelte'
  import Sparkline from '$lib/components/Sparkline.svelte'

  const cpu = $derived(metrics.latest?.cpuTotal ?? null)
  const cores = $derived(metrics.latest?.cpuPerCore.length ?? null)
  const freq = $derived(metrics.latest?.cpuFreqMhz ?? null)

  onMount(() => {
    let unlisten: (() => void) | undefined
    startMetrics().then((stop) => {
      unlisten = stop
    })
    return () => unlisten?.()
  })
</script>

<main class="app">
  <section class="card">
    <header class="head">
      <span class="dot" style="background: var(--cpu);"></span>
      <span class="title">CPU</span>
    </header>

    <Ring value={cpu} color="var(--cpu)" />

    <p class="sub">
      {cores === null ? '--' : cores} cores · {formatFreq(freq)}
    </p>

    <Sparkline data={metrics.cpuHistory} color="var(--cpu)" />
  </section>
</main>

<style>
  .app {
    height: 100vh;
    display: grid;
    place-items: center;
    padding: 24px;
  }

  .card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    width: 280px;
    padding: 24px;
    background: var(--panel);
    border: 1px solid var(--hair);
    border-radius: 16px;
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

  .sub {
    margin: 0;
    color: var(--muted);
    font-size: 0.85rem;
  }
</style>
