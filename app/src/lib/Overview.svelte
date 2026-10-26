<script lang="ts">
  import ChangeList from "./ChangeList.svelte";
  import { count } from "./format";
  import type { ScanView, Status } from "./types";

  let { view, onselect }: { view: ScanView; onselect: (id: string) => void } = $props();

  const status: Record<Status, string> = {
    running: "Running now",
    installed: "Installed",
    config_only: "Config only",
  };
</script>

<section class="totals">
  <div><strong>{view.totals.tools}</strong><span>AI tools</span></div>
  <div><strong>{view.totals.running}</strong><span>AI processes running</span></div>
  <div><strong>{view.totals.folders}</strong><span>configured folders</span></div>
  <div><strong>{view.totals.servers}</strong><span>MCP servers</span></div>
</section>

<ChangeList changes={view.changes} />

<h2>AI tools on this device</h2>
<div class="cards">
  {#each view.tools as tool (tool.id)}
    <button class="card" onclick={() => onselect(tool.id)}>
      <span class="head">
        <span class="name">{tool.name}</span>
        <span class="status {tool.status}">{status[tool.status]}</span>
      </span>
      <span class="vendor muted">{tool.vendor}</span>
      <span class="counts">
        <span>{count(tool.running.length, "process", "processes")}</span>
        <span>{count(tool.present.length, "install trace")}</span>
        <span>{count(tool.folders.length, "folder")}</span>
        <span>{count(tool.servers.length, "MCP server")}</span>
      </span>
      <span class="checked muted">
        {tool.verified.length > 0
          ? `Definition checked: ${tool.verified.join(", ")}`
          : "Definition not yet checked on a real install"}
      </span>
    </button>
  {/each}
</div>

{#if view.not_found.length > 0}
  <p class="muted">Also known, not found here: {view.not_found.join(", ")}.</p>
{/if}

<style>
  .totals {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 12px;
  }

  .totals div {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
  }

  .totals strong {
    font-size: 1.7rem;
    line-height: 1.1;
  }

  .totals span {
    color: var(--muted);
  }

  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 12px;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    text-align: left;
    padding: 14px 16px;
    border-radius: 10px;
  }

  .card:hover {
    border-color: var(--accent);
  }

  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
  }

  .name {
    font-weight: 600;
    font-size: 1.05rem;
  }

  .status {
    font-size: 0.8rem;
    padding: 1px 8px;
    border-radius: 999px;
    color: var(--muted);
    border: 1px solid var(--border);
    white-space: nowrap;
  }

  .status.running {
    color: var(--running);
    background: var(--running-bg);
    border-color: transparent;
  }

  .counts {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 2px 12px;
  }

  .checked {
    font-size: 0.85rem;
  }
</style>
