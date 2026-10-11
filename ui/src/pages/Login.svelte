<script lang="ts">
  import { rpc, setToken } from '../lib/api';
  import { session } from '../lib/stores.svelte';
  let email = $state(''), password = $state(''), err = $state(''), busy = $state(false);
  async function login(e: Event) {
    e.preventDefault(); busy = true; err = '';
    try { const r = await rpc('auth.login', { email, password }); setToken(r.token); location.reload(); }
    catch (x: any) { err = x.message; busy = false; }
  }
</script>

<div class="grid min-h-screen place-items-center bg-bg px-4">
  <form class="card w-full max-w-sm p-6" onsubmit={login}>
    <div class="mb-1 text-lg font-semibold">Sign in</div>
    <div class="mb-5 text-muted">{session.data?.org?.name}</div>
    <label class="label" for="em">Email</label><input id="em" class="input mb-3" type="email" bind:value={email} required />
    <label class="label" for="pw">Password</label><input id="pw" class="input mb-4" type="password" bind:value={password} required />
    {#if err}<div class="mb-3 text-[13px] text-bad">{err}</div>{/if}
    <button class="btn btn-primary w-full justify-center" disabled={busy}>{busy ? 'Signing in…' : 'Sign in'}</button>
    <a class="mt-4 block text-center text-[13px] text-muted hover:text-accent" href="/setup">Switch organisation</a>
  </form>
</div>
