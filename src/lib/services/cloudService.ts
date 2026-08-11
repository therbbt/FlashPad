import { supabase } from '../supabaseClient';

export interface NotebookSummary {
  id: string;
  name: string;
  ownerId: string;
  role: 'owner' | 'collaborator';
  createdAt: string;
}

// Member emails aren't shown - auth.users isn't exposed over PostgREST (by
// design, it's a different schema), and adding a public "profiles" mirror
// table just to show a friend's email is more surface area than a v1 needs.
// Members are distinguished by role + "you" in the UI instead.
export interface NotebookMember {
  userId: string;
  role: 'owner' | 'collaborator';
  joinedAt: string;
}

export interface NotebookInvite {
  token: string;
  createdAt: string;
  revoked: boolean;
}

const client = () => {
  if (!supabase) throw new Error('Cloud notebooks are not configured');
  return supabase;
};

async function currentUserId(): Promise<string> {
  const { data, error } = await client().auth.getUser();
  if (error) throw new Error(error.message);
  if (!data.user) throw new Error('Not signed in');
  return data.user.id;
}

// Every notebook the signed-in user belongs to (owned or joined), with
// their role in each - explicitly filtered to their own membership rows
// rather than relying on RLS alone, since notebook_members' select policy
// permits seeing every OTHER member of a shared notebook too (needed for
// listMembers), not just the caller's own row.
export async function listMyNotebooks(): Promise<NotebookSummary[]> {
  const userId = await currentUserId();
  const { data, error } = await client()
    .from('notebook_members')
    .select('role, notebooks (id, name, owner_id, created_at)')
    .eq('user_id', userId);
  if (error) throw new Error(error.message);
  return (data as unknown as { role: 'owner' | 'collaborator'; notebooks: { id: string; name: string; owner_id: string; created_at: string } }[]).map(
    (row) => ({
      id: row.notebooks.id,
      name: row.notebooks.name,
      ownerId: row.notebooks.owner_id,
      role: row.role,
      createdAt: row.notebooks.created_at,
    }),
  );
}

// owner_id is deliberately NOT sent by the client - notebooks.owner_id
// defaults to auth.uid() at the database level (schema.sql), so it can
// never mismatch what notebooks_insert's own RLS check evaluates.
//
// Deliberately does NOT chain .select() onto this insert. Postgres RLS
// filters an INSERT's RETURNING output through the table's SELECT policy,
// not its INSERT policy - and notebooks_select (is_member(id)) depends on a
// notebook_members row that the notebooks_after_insert trigger only creates
// once this row already exists. Within the SAME statement, that membership
// row isn't visible yet when RETURNING's SELECT-policy check runs, so
// chaining .select() here would make every notebook creation spuriously
// fail RLS. The id is generated client-side so the caller still gets a
// usable NotebookSummary back without needing RETURNING; every real caller
// (CloudNotebooksSection) re-fetches the authoritative list via
// listMyNotebooks() right after anyway, which is a plain SELECT run in a
// separate request/transaction and has no such timing issue.
export async function createNotebook(name: string): Promise<NotebookSummary> {
  const id = crypto.randomUUID();
  const { error } = await client().from('notebooks').insert({ id, name });
  if (error) throw new Error(error.message);
  const ownerId = await currentUserId();
  return { id, name, ownerId, role: 'owner', createdAt: new Date().toISOString() };
}

export async function renameNotebook(id: string, name: string): Promise<void> {
  const { error } = await client().from('notebooks').update({ name }).eq('id', id);
  if (error) throw new Error(error.message);
}

// Cascades to notebook_members/notebook_invites/notes via ON DELETE CASCADE.
// RLS (notebooks_delete) already restricts this to the owner.
export async function deleteNotebook(id: string): Promise<void> {
  const { error } = await client().from('notebooks').delete().eq('id', id);
  if (error) throw new Error(error.message);
}

// Removes the caller's own membership. RLS (members_self_leave) rejects
// this for the owner - they must delete the notebook instead.
export async function leaveNotebook(id: string): Promise<void> {
  const userId = await currentUserId();
  const { error } = await client().from('notebook_members').delete().eq('notebook_id', id).eq('user_id', userId);
  if (error) throw new Error(error.message);
}

export async function listMembers(notebookId: string): Promise<NotebookMember[]> {
  const { data, error } = await client()
    .from('notebook_members')
    .select('user_id, role, joined_at')
    .eq('notebook_id', notebookId)
    .order('joined_at', { ascending: true });
  if (error) throw new Error(error.message);
  return (data as { user_id: string; role: 'owner' | 'collaborator'; joined_at: string }[]).map((row) => ({
    userId: row.user_id,
    role: row.role,
    joinedAt: row.joined_at,
  }));
}

// RLS (members_owner_removes) restricts this to the owner removing someone
// other than themselves.
export async function removeMember(notebookId: string, userId: string): Promise<void> {
  const { error } = await client().from('notebook_members').delete().eq('notebook_id', notebookId).eq('user_id', userId);
  if (error) throw new Error(error.message);
}

export async function listInvites(notebookId: string): Promise<NotebookInvite[]> {
  const { data, error } = await client()
    .from('notebook_invites')
    .select('token, created_at, revoked')
    .eq('notebook_id', notebookId)
    .order('created_at', { ascending: false });
  if (error) throw new Error(error.message);
  return (data as { token: string; created_at: string; revoked: boolean }[]).map((row) => ({
    token: row.token,
    createdAt: row.created_at,
    revoked: row.revoked,
  }));
}

// created_by is deliberately NOT sent by the client, same reasoning as
// createNotebook's owner_id above - it defaults to auth.uid() in the DB.
export async function createInvite(notebookId: string): Promise<string> {
  const { data, error } = await client().from('notebook_invites').insert({ notebook_id: notebookId }).select('token').single();
  if (error) throw new Error(error.message);
  return data.token as string;
}

export async function revokeInvite(token: string): Promise<void> {
  const { error } = await client().from('notebook_invites').update({ revoked: true }).eq('token', token);
  if (error) throw new Error(error.message);
}

// Calls the SECURITY DEFINER redeem_invite() RPC (schema.sql) - the caller
// isn't a member yet, so has no RLS visibility into notebook_invites and no
// write access to notebook_members to do this directly. Returns the joined
// notebook's id.
export async function redeemInvite(token: string): Promise<string> {
  const { data, error } = await client().rpc('redeem_invite', { p_token: token });
  if (error) throw new Error(error.message);
  return data as string;
}
