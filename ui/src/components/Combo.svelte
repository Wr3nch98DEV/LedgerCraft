<script lang="ts">
  // Searchable select. options: {value, label, sub?, group?}
  type Opt = { value: any; label: string; sub?: string; group?: string };
  let {
    value = $bindable(null), options = [], placeholder = '', onchange, allowCreate = false, oncreate, compact = false, disabled = false,
    clearable = true, id = undefined, invalid = false
  }: {
    value?: any; options?: Opt[]; placeholder?: string; onchange?: (v: any, o?: Opt) => void; allowCreate?: boolean; oncreate?: (text: string) => void;
    compact?: boolean; disabled?: boolean; clearable?: boolean; id?: string; invalid?: boolean;
  } = $props();

  let open = $state(false);
  let q = $state('');
  let hi = $state(0);
  let input: HTMLInputElement | undefined = $state();
  let list: HTMLDivElement | undefined = $state();

  const selected = $derived(options.find((o) => o.value === value));
  const filtered = $derived.by(() => {
    const t = q.trim().toLowerCase();
    if (!t) return options.slice(0, 300);
    return options.filter((o) => o.label.toLowerCase().includes(t) || (o.sub ?? '').toLowerCase().includes(t)).slice(0, 300);
  });

  function pick(o: Opt) { value = o.value; open = false; q = ''; onchange?.(o.value, o); }
  function focus() { if (disabled) return; open = true; q = ''; hi = Math.max(0, filtered.findIndex((o) => o.value === value)); }
  function blur() { setTimeout(() => { open = false; q = ''; }, 120); }
  function key(e: KeyboardEvent) {
    if (!open && (e.key === 'ArrowDown' || e.key === 'Enter')) { open = true; return; }
    if (e.key === 'ArrowDown') { hi = Math.min(hi + 1, filtered.length - (allowCreate && q ? 0 : 1)); scroll(); e.preventDefault(); }
    else if (e.key === 'ArrowUp') { hi = Math.max(hi - 1, 0); scroll(); e.preventDefault(); }
    else if (e.key === 'Enter') {
      e.preventDefault();
      if (hi < filtered.length) pick(filtered[hi]);
      else if (allowCreate && q.trim()) { oncreate?.(q.trim()); open = false; q = ''; }
    } else if (e.key === 'Escape') { open = false; q = ''; }
    else if (e.key === 'Tab' && open && q && filtered[hi]) { pick(filtered[hi]); }
  }
  function scroll() { requestAnimationFrame(() => list?.querySelector('[data-hi="1"]')?.scrollIntoView({ block: 'nearest' })); }
</script>

<div class="relative">
  <input
    bind:this={input} {id} {disabled}
    class={compact ? 'grid-in' : 'input' + (invalid ? ' invalid' : '')}
    value={open ? q : (selected?.label ?? '')}
    placeholder={selected ? selected.label : placeholder}
    oninput={(e) => { q = (e.target as HTMLInputElement).value; open = true; hi = 0; }}
    onfocus={focus} onblur={blur} onkeydown={key}
    autocomplete="off" role="combobox" aria-expanded={open} aria-controls="combo-list"
  />
  {#if clearable && value !== null && value !== undefined && !open && !disabled && !compact}
    <button class="absolute right-2 top-1/2 -translate-y-1/2 text-faint hover:text-text" tabindex="-1" aria-label="Clear" onmousedown={(e) => { e.preventDefault(); value = null; onchange?.(null); }}>×</button>
  {/if}
  {#if open}
    <div bind:this={list} id="combo-list" class="absolute left-0 z-40 mt-1 max-h-72 min-w-full w-max max-w-[480px] overflow-y-auto rounded-lg border border-line2 bg-panel py-1 shadow-[var(--shadow)]" role="listbox">
      {#each filtered as o, i (o.value)}
        {#if o.group && (i === 0 || filtered[i - 1].group !== o.group)}
          <div class="px-3 pb-1 pt-2 text-[11px] font-semibold uppercase tracking-wide text-faint">{o.group}</div>
        {/if}
        <button type="button" data-hi={i === hi ? '1' : '0'} class="flex w-full items-baseline gap-2 px-3 py-1.5 text-left {i === hi ? 'bg-hover' : ''} {o.value === value ? 'text-accent' : ''}"
          onmousedown={(e) => { e.preventDefault(); pick(o); }} onmouseenter={() => (hi = i)} role="option" aria-selected={o.value === value}>
          <span class="truncate">{o.label}</span>{#if o.sub}<span class="truncate text-xs text-faint">{o.sub}</span>{/if}
        </button>
      {/each}
      {#if allowCreate && q.trim()}
        <button type="button" data-hi={hi === filtered.length ? '1' : '0'} class="w-full px-3 py-1.5 text-left text-accent {hi === filtered.length ? 'bg-hover' : ''}"
          onmousedown={(e) => { e.preventDefault(); oncreate?.(q.trim()); open = false; q = ''; }}>+ Add "{q.trim()}"</button>
      {/if}
      {#if !filtered.length && !(allowCreate && q.trim())}<div class="px-3 py-2 text-faint">No matches</div>{/if}
    </div>
  {/if}
</div>
