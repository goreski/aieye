<script lang="ts">
  import ChangeList from "./ChangeList.svelte";
  import Evidence from "./Evidence.svelte";
  import { count, programs } from "./format";
  import type { ScanView, Status } from "./types";

  let { view, onselect }: { view: ScanView; onselect: (id: string) => void } = $props();

  const status: Record<Status, string> = {
    running: "Running now",
    installed: "Installed",
    config_only: "Config only",
  };

  /** A tool's display name from its id. */
  function toolName(id: string): string {
    return view.tools.find((tool) => tool.id === id)?.name ?? id;
  }
</script>

<section class="totals">
  <div><strong>{view.totals.tools}</strong><span>AI tools</span></div>
  <div><strong>{view.totals.running}</strong><span>AI processes running</span></div>
  <div><strong>{view.totals.folders}</strong><span>configured folders</span></div>
  <div><strong>{view.totals.servers}</strong><span>MCP servers</span></div>
  <div><strong>{view.totals.schedules}</strong><span>AI schedules</span></div>
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
        <span>{count(tool.schedules.length, "schedule")}</span>
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

<h2>Scheduled tasks on this device</h2>
{#if view.tasks.length > 0}
  <table>
    <thead>
      <tr><th>Task</th><th>Runs</th><th>When</th><th>AI tool</th><th>State</th><th>Evidence</th></tr>
    </thead>
    <tbody>
      {#each view.tasks as record, index (index)}
        {#if record.fact.kind === "scheduled_task"}
          <tr class:off={!record.fact.enabled}>
            <td class="path">{record.fact.path}</td>
            <td class="path">{programs(record.fact.actions)}</td>
            <td>{record.fact.triggers.join("; ") || "no triggers"}</td>
            <td>{record.fact.tool ? toolName(record.fact.tool) : "—"}</td>
            <td>{record.fact.enabled ? "on" : "off"}</td>
            <td><Evidence {record} /></td>
          </tr>
        {/if}
      {/each}
    </tbody>
  </table>
  <p class="muted note">
    Every task outside Windows' own folders is listed, because an AI agent can create a task
    that runs an ordinary program. A task is linked to an AI tool when it runs one of the
    tool's programs or something in its folders. Arguments are never kept.
  </p>
{:else}
  <p class="muted">No scheduled tasks outside Windows' own.</p>
{/if}

<style>
  .totals {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 12px;
  }

  th {
    white-space: nowrap;
  }

  tr.off td {
    color: var(--muted);
  }

  .note {
    font-size: 0.85rem;
    margin-top: 6px;
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
