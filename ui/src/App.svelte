<script lang="ts">
  import Nav from './components/Nav.svelte';
  import Overlays from './components/Overlays.svelte';
  import Palette from './components/Palette.svelte';
  import Panels from './components/Panels.svelte';
  import { route, match } from './lib/router.svelte';
  import { session, loadSession, loadLookups, lookups } from './lib/stores.svelte';
  import { onAuthError } from './lib/api';
  import { routes } from './routes';
  import Setup from './pages/Setup.svelte';
  import Login from './pages/Login.svelte';
  import NotFound from './pages/NotFound.svelte';

  let ready = $state(false);
  async function boot() {
    const s = await loadSession();
    if (s?.user) { try { await loadLookups(); } catch (e) { console.error(e); } }
    ready = true;
  }
  boot();
  onAuthError(() => { session.data = { ...session.data, user: null, login_required: true }; });

  const current = $derived.by(() => {
    for (const [pattern, component] of routes) {
      const params = match(pattern, route.path);
      if (params) return { component, params };
    }
    return { component: NotFound, params: {} };
  });
  const key = $derived(route.path);
</script>

<Overlays />
{#if !ready || !session.loaded}
  <div class="grid h-screen place-items-center text-muted">Loading…</div>
{:else if !session.data?.org || route.path === '/setup'}
  <Setup />
{:else if session.data?.login_required || !session.data?.user}
  <Login />
{:else if lookups.loaded}
  <Nav />
  <Palette />
  <Panels />
  <main class="mx-auto max-w-[1440px] px-4 pb-24 pt-6 sm:px-6">
    {#key key}
      <current.component {...current.params} />
    {/key}
  </main>
  <footer class="no-print mx-auto max-w-[1440px] px-6 pb-6 text-[12px] text-faint">
    LedgerCraft {session.data?.version} · open-source accounting · not affiliated with Xero Limited
  </footer>
{/if}
