<script lang="ts">
  import Combo from './Combo.svelte';
  import { rpc } from '../lib/api';
  import { toastError } from '../lib/stores.svelte';
  let { value = $bindable(null), onchange, placeholder = 'Find or add a contact', kind = 'all', invalid = false, compact = false }:
    { value?: number | null; onchange?: (id: number | null, contact?: any) => void; placeholder?: string; kind?: string; invalid?: boolean; compact?: boolean } = $props();
  let contacts = $state<any[]>([]);
  async function load() { contacts = await rpc('contacts.list', { filter: 'all' }); }
  load();
  const options = $derived(contacts.map((c) => ({ value: c.id, label: c.name, sub: c.email || (c.is_customer && c.is_supplier ? 'Customer & supplier' : c.is_supplier ? 'Supplier' : c.is_customer ? 'Customer' : '') })));
  async function create(name: string) {
    try {
      const r = await rpc('contacts.create', { name, is_customer: kind === 'customer' ? true : null, is_supplier: kind === 'supplier' ? true : null });
      await load();
      value = r.id;
      onchange?.(r.id, contacts.find((c) => c.id === r.id));
    } catch (e) { toastError(e); }
  }
</script>

<Combo bind:value {options} {placeholder} {invalid} {compact} allowCreate oncreate={create} onchange={(v) => onchange?.(v, contacts.find((c) => c.id === v))} />
