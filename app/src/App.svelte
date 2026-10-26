<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { localTime } from "./lib/format";
  import Overview from "./lib/Overview.svelte";
  import ToolDetail from "./lib/ToolDetail.svelte";
  import type { ScanView } from "./lib/types";

  let view = $state<ScanView | null>(null);
  let loaded = $state(false);
  let busy = $state(false);
  let scanning = $state(false);
  let selected = $state<string | null>(null);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);

  const tool = $derived(view?.tools.find((tool) => tool.id === selected) ?? null);

  /** Runs one command at a time and shows its error, if any. */
  async function run<T>(work: () => Promise<T>): Promise<T | undefined> {
    busy = true;
    error = null;
    notice = null;
    try {
      return await work();
    } catch (problem) {
      error = String(problem);
      return undefined;
    } finally {
      busy = false;
    }
  }

  async function scan() {
    if (busy) return;
    scanning = true;
    const result = await run(() => invoke<ScanView>("scan"));
    scanning = false;
    if (result) view = result;
  }

  async function exportSnapshot() {
    if (!view || busy) return;
    const id = view.snapshot.id;
    const path = await run(() => invoke<string | null>("export", { id }));
    if (path) notice = `Saved snapshot ${id} to ${path}`;
  }

  onMount(() => {
    run(() => invoke<ScanView | null>("latest")).then((latest) => {
      view = latest ?? null;
      loaded = true;
    });
    const stop = listen("scan-requested", () => scan());
    return () => {
      stop.then((unlisten) => unlisten());
    };
  });
</script>

<header>
  <div class="brand">
    <svg viewBox="0 0 1024 1024" aria-hidden="true">
      <rect width="1024" height="1024" rx="224" fill="#16213e" />
      <path
        d="M136 512 C 272 296, 752 296, 888 512 C 752 728, 272 728, 136 512 Z"
        fill="none"
        stroke="#fff"
        stroke-width="60"
      />
      <circle cx="512" cy="512" r="136" fill="#3ddc97" />
      <circle cx="512" cy="512" r="56" fill="#16213e" />
    </svg>
    <span>AIEye</span>
  </div>
  {#if view}
    <span class="meta muted">Snapshot {view.snapshot.id} · {localTime(view.snapshot.taken_at)}</span>
  {/if}
  <div class="actions">
    <button onclick={exportSnapshot} disabled={busy || !view}>Export JSON</button>
    <button class="primary" onclick={scan} disabled={busy}>
      {scanning ? "Scanning…" : "Scan now"}
    </button>
  </div>
</header>

<main>
  {#if error}
    <p class="message error" role="alert">{error}</p>
  {/if}
  {#if notice}
    <p class="message">{notice}</p>
  {/if}

  {#if !loaded}
    <p class="muted">Loading…</p>
  {:else if !view}
    <section class="empty">
      <h1>No scans yet</h1>
      <p class="muted">
        Scan this device to see which AI tools are here, what they can reach, and later what
        changed. A scan only reads, runs as you and keeps no secret values.
      </p>
      <button class="primary" onclick={scan} disabled={busy}>
        {scanning ? "Scanning…" : "Scan now"}
      </button>
    </section>
  {:else if tool}
    <ToolDetail {tool} onback={() => (selected = null)} />
  {:else}
    <Overview {view} onselect={(id) => (selected = id)} />
  {/if}
</main>

<style>
  header {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 24px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 700;
    font-size: 1.1rem;
  }

  .brand svg {
    width: 28px;
    height: 28px;
  }

  .actions {
    margin-left: auto;
    display: flex;
    gap: 8px;
  }

  main {
    max-width: 1100px;
    margin: 0 auto;
    padding: 20px 24px 40px;
  }

  .message {
    padding: 10px 14px;
    border-radius: 8px;
    background: var(--panel);
    border: 1px solid var(--border);
  }

  .error {
    color: var(--alert);
    background: var(--alert-bg);
    border-color: transparent;
  }

  .empty {
    max-width: 520px;
    margin: 80px auto;
    text-align: center;
  }
</style>
