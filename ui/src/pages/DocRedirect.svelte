<script lang="ts">
  import { rpc } from '../lib/api';
  import { navigate } from '../lib/router.svelte';
  import { docPath } from '../lib/format';
  let { id }: { id: string } = $props();
  rpc('docs.get', { id: Number(id) }).then((d) => navigate(docPath(d.type, d.id), { replace: true })).catch(() => navigate('/', { replace: true }));
</script>
<div class="text-muted">Opening…</div>
