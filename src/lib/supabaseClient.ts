import { createClient, type SupabaseClient } from '@supabase/supabase-js';

const url = import.meta.env.VITE_SUPABASE_URL;
const anonKey = import.meta.env.VITE_SUPABASE_ANON_KEY;

// Cloud notebooks are entirely optional - FlashPad works fully local-only
// with no .env at all (see supabase/README.md). Every call site that reads
// this must handle `null` rather than assume Supabase is configured.
export const supabase: SupabaseClient | null = url && anonKey ? createClient(url, anonKey) : null;

export const isCloudConfigured = (): boolean => supabase !== null;
