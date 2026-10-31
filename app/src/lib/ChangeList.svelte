<script lang="ts">
  import { count, localTime } from "./format";
  import type { ChangesView } from "./types";

  let { changes }: { changes: ChangesView | null } = $props();
</script>

<section>
  {#if !changes}
    <h2>Changes</h2>
    <p class="muted">This is the first scan, so there is nothing to compare yet.</p>
  {:else if changes.lines.length === 0}
    <h2>Changes</h2>
    <p class="muted">
      No changes since snapshot {changes.from.id} ({localTime(changes.from.taken_at)}).
    </p>
  {:else}
    <h2>
      Changes since snapshot {changes.from.id}
      <span class="muted">({localTime(changes.from.taken_at)})</span>
      {#if changes.alerts > 0}
        <span class="alerts">{count(changes.alerts, "alert")}</span>
      {/if}
    </h2>
    <ul>
      {#each changes.lines as line, index (index)}
        <li class={line.severity}>
          <span class="level">{line.severity === "alert" ? "Alert" : "Info"}</span>
          <span class="tool">{line.tool ?? "—"}</span>
          <span class="what">{line.what}</span>
          <span class="detail path">{line.detail}</span>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .alerts {
    margin-left: 8px;
    padding: 2px 10px;
    border-radius: 999px;
    font-size: 0.8rem;
    color: var(--alert);
    background: var(--alert-bg);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 8px;
  }

  li {
    display: grid;
    grid-template-columns: 56px 130px 150px 1fr;
    gap: 10px;
    padding: 7px 12px;
    border-bottom: 1px solid var(--border);
  }

  li:last-child {
    border-bottom: none;
  }

  .level {
    font-weight: 600;
    color: var(--muted);
  }

  .alert .level {
    color: var(--alert);
  }

  .tool {
    font-weight: 600;
  }
</style>
