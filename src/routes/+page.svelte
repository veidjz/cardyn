<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { metrics, startMetrics } from '$lib/metrics.svelte'
  import { formatFreq, formatBytes, formatBps } from '$lib/format'
  import { sparklineMax } from '$lib/chart'
  import Ring from '$lib/components/Ring.svelte'
  import Sparkline from '$lib/components/Sparkline.svelte'
  import Detail from '$lib/components/Detail.svelte'
  import type { MetricKey } from '$lib/types'

  let route = $state<'main' | { detail: MetricKey }>('main')

  // Card element refs (for return-focus) and the last-opened metric, so that
  // returning to the grid restores focus to the card that was opened.
  let cardEls: Partial<Record<MetricKey, HTMLButtonElement>> = {}
  let lastMetric: MetricKey | null = null

  function open(metric: MetricKey) {
    lastMetric = metric
    route = { detail: metric }
  }

  async function back() {
    route = 'main'
    await tick()
    if (lastMetric) cardEls[lastMetric]?.focus()
  }

  // Keyboard navigation among the cards. Left/Right move linearly through all
  // cards (first..last), wrapping at the ends. Up/Down move within the current
  // COLUMN using the live column count, wrapping within that column and never
  // going sideways. Tab-wrap keeps linear focus from falling off either end.
  const order: MetricKey[] = ['cpu', 'mem', 'gpu', 'disk', 'net']
  let gridEl = $state<HTMLDivElement>()

  function columnCount() {
    if (!gridEl) return 1
    const cols = getComputedStyle(gridEl)
      .gridTemplateColumns.split(' ')
      .filter((c) => c && c !== '0px')
    return Math.max(1, cols.length)
  }

  function focusAt(i: number) {
    cardEls[order[i]]?.focus()
  }
  // Cards fill the grid row-major, so card i sits at row floor(i/cols), col
  // i%cols. `colCells` lists the indices sharing its column (top to bottom).
  function colCells(i: number, cols: number) {
    const cells: number[] = []
    for (let j = i % cols; j < order.length; j += cols) cells.push(j)
    return cells
  }
  function step(cells: number[], from: number, delta: number) {
    const pos = cells.indexOf(from)
    return cells[(pos + delta + cells.length) % cells.length]
  }

  function onCardKey(e: KeyboardEvent, metric: MetricKey) {
    const i = order.indexOf(metric)
    const last = order.length - 1
    const cols = columnCount()
    if (e.key === 'ArrowRight') {
      e.preventDefault()
      focusAt((i + 1) % order.length)
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault()
      focusAt((i - 1 + order.length) % order.length)
    } else if (e.key === 'ArrowDown') {
      e.preventDefault()
      focusAt(step(colCells(i, cols), i, 1))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      focusAt(step(colCells(i, cols), i, -1))
    } else if (e.key === 'Home') {
      e.preventDefault()
      focusAt(0)
    } else if (e.key === 'End') {
      e.preventDefault()
      focusAt(last)
    } else if (e.key === 'Tab' && !e.shiftKey && i === last) {
      e.preventDefault()
      focusAt(0)
    } else if (e.key === 'Tab' && e.shiftKey && i === 0) {
      e.preventDefault()
      focusAt(last)
    }
  }

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
  const gpuNa = $derived(gpuUtil === null)

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
    <div class="grid" bind:this={gridEl}>
      <!-- CPU -->
      <button
        class="card"
        type="button"
        style="--focus: var(--cpu)"
        bind:this={cardEls.cpu}
        onclick={() => open('cpu')}
        onkeydown={(e) => onCardKey(e, 'cpu')}
      >
        <header class="head">
          <span class="dot" style="background: var(--cpu);"></span>
          <span class="title">CPU</span>
        </header>
        <div class="primary">
          <Ring value={cpu} color="var(--cpu)" fill />
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
        style="--focus: var(--mem)"
        bind:this={cardEls.mem}
        onclick={() => open('mem')}
        onkeydown={(e) => onCardKey(e, 'mem')}
      >
        <header class="head">
          <span class="dot" style="background: var(--mem);"></span>
          <span class="title">Memory</span>
        </header>
        <div class="primary">
          <Ring value={memPct} color="var(--mem)" fill />
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
        style="--focus: var(--gpu)"
        bind:this={cardEls.gpu}
        onclick={() => open('gpu')}
        onkeydown={(e) => onCardKey(e, 'gpu')}
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
        {:else}
          <div class="primary">
            <Ring value={gpuUtil} color="var(--gpu)" fill />
          </div>
          <p class="sub">
            {vram === null
              ? formatBytes(gpuMem)
              : formatBytes(gpuMem) + ' / ' + formatBytes(vram)}
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
        style="--focus: var(--disk)"
        bind:this={cardEls.disk}
        onclick={() => open('disk')}
        onkeydown={(e) => onCardKey(e, 'disk')}
      >
        <header class="head">
          <span class="dot" style="background: var(--disk);"></span>
          <span class="title">Disk</span>
        </header>
        <div class="primary">
          <Ring value={diskPct} color="var(--disk)" fill />
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
        style="--focus: var(--net)"
        bind:this={cardEls.net}
        onclick={() => open('net')}
        onkeydown={(e) => onCardKey(e, 'net')}
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
    <Detail metric={route.detail} onBack={back} />
  {/if}
</main>

<style>
  .app {
    height: 100vh;
    box-sizing: border-box;
    padding: 18px;
    overflow-y: auto;
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr;
    grid-auto-rows: minmax(min-content, 1fr);
    gap: 16px;
    width: 100%;
    height: 100%;
  }

  @media (min-width: 600px) {
    .grid {
      grid-template-columns: 1fr 1fr;
    }
  }

  @media (min-width: 720px) {
    .grid {
      grid-template-columns: 1fr 1fr 1fr;
    }
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

  .card:focus-visible {
    outline: 2px solid var(--focus, var(--text));
    outline-offset: 2px;
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
    flex: 1 1 auto;
    min-height: var(--rsz);
    display: grid;
    place-items: center;
    width: 100%;
    --rsz: clamp(120px, 22vmin, 240px);
  }

  .big {
    font-size: clamp(1.4rem, 5vmin, 2.6rem);
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
