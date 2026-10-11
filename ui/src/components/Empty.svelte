<script lang="ts">
  import Icon from './Icon.svelte';
  let { icon = 'doc', title, text = '', children }: { icon?: string; title: string; text?: string; children?: any } = $props();
</script>

<div class="flex flex-col items-center justify-center px-6 py-14 text-center">
  <div class="mb-3 rounded-full bg-accent-soft p-3 text-accent"><Icon name={icon} size={22} /></div>
  <div class="text-[15px] font-semibold">{title}</div>
  {#if text}<div class="mt-1 max-w-md text-muted">{text}</div>{/if}
  {#if children}<div class="mt-4 flex gap-2">{@render children()}</div>{/if}
</div>
