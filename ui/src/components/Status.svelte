<script lang="ts">
  import { STATUS_STYLE } from './Bits.svelte';
  import { STATUS_LABEL } from '../lib/format';
  let { status, overdue = false, label = undefined }: { status: string; overdue?: boolean; label?: string } = $props();
  const key = $derived(overdue && status === 'AUTHORISED' ? 'OVERDUE' : status);
</script>

<span class="chip {STATUS_STYLE[key] ?? 'bg-[var(--hover)] text-muted'}">{label ?? (key === 'OVERDUE' ? 'Overdue' : STATUS_LABEL[status] ?? status)}</span>
