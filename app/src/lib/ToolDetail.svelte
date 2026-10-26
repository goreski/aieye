<script lang="ts">
  import Evidence from "./Evidence.svelte";
  import type { ToolView } from "./types";

  let { tool, onback }: { tool: ToolView; onback: () => void } = $props();
</script>

<button class="back" onclick={onback}>← All tools</button>

<h1>{tool.name} <span class="muted">{tool.vendor}</span></h1>
<p class="muted">
  {tool.verified.length > 0
    ? `Definition checked on a real install: ${tool.verified.join(", ")}.`
    : "This definition has not been checked on a real install yet."}
</p>

<h2>Running now</h2>
{#if tool.running.length > 0}
  <table>
    <thead><tr><th>PID</th><th>Program</th><th>Evidence</th></tr></thead>
    <tbody>
      {#each tool.running as record, index (index)}
        {#if record.fact.kind === "tool_running"}
          <tr>
            <td>{record.fact.pid}</td>
            <td class="path">{record.fact.exe}</td>
            <td><Evidence {record} /></td>
          </tr>
        {/if}
      {/each}
    </tbody>
  </table>
{:else}
  <p class="muted">Not running at scan time.</p>
{/if}

<h2>Install traces</h2>
{#if tool.present.length > 0}
  <table>
    <thead><tr><th>File or folder</th><th>Evidence</th></tr></thead>
    <tbody>
      {#each tool.present as record, index (index)}
        {#if record.fact.kind === "tool_present"}
          <tr>
            <td class="path">{record.fact.path}</td>
            <td><Evidence {record} /></td>
          </tr>
        {/if}
      {/each}
    </tbody>
  </table>
{:else}
  <p class="muted">No files or folders found.</p>
{/if}

<h2>Configured folders</h2>
{#if tool.folders.length > 0}
  <table>
    <thead>
      <tr><th>Folder (as the config writes it)</th><th>Trust</th><th>Boundary</th><th>Config file</th><th>Evidence</th></tr>
    </thead>
    <tbody>
      {#each tool.folders as record, index (index)}
        {#if record.fact.kind === "configured_folder"}
          <tr>
            <td class="path">{record.fact.path}</td>
            <td>{record.fact.trust ?? "—"}</td>
            <td>{record.fact.boundary ? "yes" : "no"}</td>
            <td class="path">{record.fact.file}</td>
            <td><Evidence {record} /></td>
          </tr>
        {/if}
      {/each}
    </tbody>
  </table>
  <p class="muted note">
    Boundary says whether the tool limits its file access to these folders. Trust settings usually
    do not.
  </p>
{:else}
  <p class="muted">The tool's config names no folders.</p>
{/if}

<h2>MCP servers</h2>
{#if tool.servers.length > 0}
  <table>
    <thead>
      <tr><th>Name</th><th>Runs</th><th>Environment variables</th><th>Scope</th><th>Evidence</th></tr>
    </thead>
    <tbody>
      {#each tool.servers as record, index (index)}
        {#if record.fact.kind === "mcp_server"}
          <tr>
            <td><strong>{record.fact.name}</strong></td>
            <td class="path">{record.fact.command ?? record.fact.origin ?? "—"}</td>
            <td class="path">{record.fact.env_names.join(", ") || "—"}</td>
            <td class="path">{record.fact.scope ?? "user"}</td>
            <td><Evidence {record} /></td>
          </tr>
        {/if}
      {/each}
    </tbody>
  </table>
  <p class="muted note">
    Only names are kept. AIEye never stores variable values, arguments, headers or tokens.
  </p>
{:else}
  <p class="muted">The tool's config lists no MCP servers.</p>
{/if}

<style>
  .back {
    border: none;
    background: none;
    padding: 0;
    margin-bottom: 14px;
    color: var(--accent);
  }

  h1 .muted {
    font-size: 1rem;
    font-weight: 400;
    margin-left: 6px;
  }

  .note {
    font-size: 0.85rem;
    margin-top: 6px;
  }
</style>
