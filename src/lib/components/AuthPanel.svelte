<script lang="ts">
  import { signInWithPassword, signUp } from '../stores/authStore';

  let mode: 'signIn' | 'signUp' = 'signIn';
  let email = '';
  let password = '';
  let busy = false;
  let error = '';
  let notice = '';

  const submit = async () => {
    if (!email.trim() || !password) return;
    busy = true;
    error = '';
    notice = '';
    try {
      if (mode === 'signIn') {
        await signInWithPassword(email.trim(), password);
      } else {
        await signUp(email.trim(), password);
        notice = 'Account created. Check your email to confirm, then sign in.';
        mode = 'signIn';
      }
      password = '';
    } catch (err) {
      error = err instanceof Error ? err.message : 'Something went wrong';
    } finally {
      busy = false;
    }
  };
</script>

<div class="auth-panel">
  <div class="tabs">
    <button class="tab" class:active={mode === 'signIn'} on:click={() => (mode = 'signIn')}>Sign in</button>
    <button class="tab" class:active={mode === 'signUp'} on:click={() => (mode = 'signUp')}>Create account</button>
  </div>

  <form class="form" on:submit|preventDefault={submit}>
    <input class="input" type="email" placeholder="Email" bind:value={email} autocomplete="email" required />
    <input
      class="input"
      type="password"
      placeholder="Password"
      bind:value={password}
      autocomplete={mode === 'signIn' ? 'current-password' : 'new-password'}
      minlength="6"
      required
    />
    <button class="btn primary" type="submit" disabled={busy || !email.trim() || !password}>
      {#if busy}
        Working…
      {:else if mode === 'signIn'}
        Sign in
      {:else}
        Create account
      {/if}
    </button>
  </form>

  {#if notice}<p class="notice">{notice}</p>{/if}
  {#if error}<p class="error">{error}</p>{/if}
</div>

<style>
  .auth-panel {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .tabs {
    display: flex;
    gap: 0.3rem;
  }

  .tab {
    flex: 1;
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--panel-2);
    color: var(--muted);
    font-size: 0.75rem;
    padding: 0.3rem 0.5rem;
    cursor: pointer;
  }

  .tab.active {
    color: var(--text);
    background: var(--accent-soft, var(--panel-2));
    font-weight: 600;
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .input {
    font-size: 0.8rem;
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--panel-2);
    color: var(--text);
    padding: 0.35rem 0.5rem;
  }

  .btn {
    border: 1px solid var(--border);
    border-radius: 0.35rem;
    background: var(--panel-2);
    color: var(--text);
    font-size: 0.75rem;
    padding: 0.35rem 0.6rem;
    cursor: pointer;
  }

  .btn.primary {
    background: var(--accent-soft, var(--panel-2));
    font-weight: 600;
  }

  .btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .notice {
    margin: 0;
    font-size: 0.75rem;
    color: var(--muted);
  }

  .error {
    margin: 0;
    font-size: 0.75rem;
    color: #ef4444;
  }
</style>
