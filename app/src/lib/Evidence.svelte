<script lang="ts">
  import { localTime } from "./format";
  import type { FactRecord } from "./types";

  let { record }: { record: FactRecord } = $props();
</script>

<span
  class="evidence {record.evidence}"
  title="Source: {record.source} · observed {localTime(record.observed_at)}"
>
  {record.evidence}
</span>

<style>
  .evidence {
    display: inline-block;
    padding: 1px 8px;
    border-radius: 999px;
    font-size: 0.8rem;
    font-weight: 600;
    white-space: nowrap;
  }

  .verified {
    color: var(--verified);
    background: var(--verified-bg);
  }

  .declared {
    color: var(--declared);
    background: var(--declared-bg);
  }

  .inferred {
    color: var(--inferred);
    background: var(--inferred-bg);
  }
</style>
