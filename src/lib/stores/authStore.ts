import { writable } from 'svelte/store';
import type { Session, User } from '@supabase/supabase-js';
import { supabase } from '../supabaseClient';

export const session = writable<Session | null>(null);
export const user = writable<User | null>(null);

// Resolves once the initial session (if any) has been restored from
// storage - App.svelte's boot sequence awaits authReadyPromise before
// deciding whether a persisted cloud-notebook selection can actually be
// resumed, rather than racing an unresolved session on first paint. authReady
// (the writable) exists alongside it for any reactive UI that wants to bind
// to the same fact.
export const authReady = writable(false);
let resolveAuthReady: () => void = () => {};
export const authReadyPromise = new Promise<void>((resolve) => {
  resolveAuthReady = resolve;
});

if (supabase) {
  supabase.auth.getSession().then(({ data }) => {
    session.set(data.session);
    user.set(data.session?.user ?? null);
    authReady.set(true);
    resolveAuthReady();
  });

  supabase.auth.onAuthStateChange((_event, newSession) => {
    session.set(newSession);
    user.set(newSession?.user ?? null);
  });
} else {
  // No Supabase configuration - resolve immediately so callers awaiting
  // authReadyPromise don't hang forever.
  authReady.set(true);
  resolveAuthReady();
}

export async function signUp(email: string, password: string): Promise<void> {
  if (!supabase) throw new Error('Cloud notebooks are not configured');
  const { error } = await supabase.auth.signUp({ email, password });
  if (error) throw error;
}

export async function signInWithPassword(email: string, password: string): Promise<void> {
  if (!supabase) throw new Error('Cloud notebooks are not configured');
  const { error } = await supabase.auth.signInWithPassword({ email, password });
  if (error) throw error;
}

export async function signOut(): Promise<void> {
  if (!supabase) return;
  const { error } = await supabase.auth.signOut();
  if (error) throw error;
}
