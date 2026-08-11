-- FlashPad cloud notebooks schema.
--
-- Run this once in the Supabase SQL editor (SQL Editor -> New query -> paste
-- -> Run) against a fresh project. Safe to re-run individual "create or
-- replace function" statements, but the "create table"/"create policy"
-- statements are not idempotent - drop the tables first if you need to
-- re-apply this from scratch.
--
-- Mirrors the local SQLite notes schema (src-tauri/src/db.rs /
-- src-tauri/src/notes.rs) scoped under a notebook_id instead of a single
-- implicit local database, with row-level security enforcing membership.

-- ---------- tables ----------

-- owner_id/created_by below default to auth.uid() rather than trusting the
-- client to supply a matching value - the client never sends them, so
-- there's no way for the value RLS checks against (auth.uid()) to ever
-- diverge from what actually gets written.
create table public.notebooks (
  id uuid primary key default gen_random_uuid(),
  name text not null default 'Shared Notebook',
  owner_id uuid not null default auth.uid() references auth.users(id) on delete cascade,
  created_at timestamptz not null default now()
);

create table public.notebook_members (
  notebook_id uuid not null references public.notebooks(id) on delete cascade,
  user_id uuid not null references auth.users(id) on delete cascade,
  role text not null check (role in ('owner', 'collaborator')),
  joined_at timestamptz not null default now(),
  primary key (notebook_id, user_id)
);
create index on public.notebook_members (user_id);

create table public.notebook_invites (
  token uuid primary key default gen_random_uuid(),
  notebook_id uuid not null references public.notebooks(id) on delete cascade,
  created_by uuid not null default auth.uid() references auth.users(id),
  created_at timestamptz not null default now(),
  revoked boolean not null default false
);

-- int4 identity (not bigint): PostgREST/supabase-js reliably returns this as
-- a JS `number`, matching every id comparison already written against the
-- local SQLite `number` ids throughout the frontend (notesStore.ts etc.).
create table public.notes (
  id integer generated always as identity primary key,
  notebook_id uuid not null references public.notebooks(id) on delete cascade,
  title text not null default 'Untitled',
  content text not null default '',
  parent_id integer references public.notes(id) on delete cascade,
  is_markdown boolean not null default false,
  is_locked boolean not null default false,
  sort_order integer not null default 0,
  show_line_numbers boolean not null default false,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);
create index on public.notes (notebook_id);
create index on public.notes (notebook_id, parent_id);

-- ---------- membership helpers ----------
--
-- SECURITY DEFINER so these can be called from inside RLS policies (which
-- run as the querying user) without themselves being blocked by RLS on
-- notebook_members, and to avoid every policy below repeating the same
-- membership subquery. `set search_path` pins name resolution to `public`
-- so a SECURITY DEFINER function can't be hijacked by a role-local search
-- path pointing at a shadow "notebook_members" table.

create or replace function public.is_member(p_notebook_id uuid)
returns boolean
language sql security definer set search_path = public, pg_temp stable as $$
  select exists (
    select 1 from public.notebook_members
    where notebook_id = p_notebook_id and user_id = auth.uid()
  );
$$;

create or replace function public.is_owner(p_notebook_id uuid)
returns boolean
language sql security definer set search_path = public, pg_temp stable as $$
  select exists (
    select 1 from public.notebook_members
    where notebook_id = p_notebook_id and user_id = auth.uid() and role = 'owner'
  );
$$;

-- ---------- row level security ----------

alter table public.notebooks enable row level security;
alter table public.notebook_members enable row level security;
alter table public.notebook_invites enable row level security;
alter table public.notes enable row level security;

create policy notebooks_select on public.notebooks
  for select using (is_member(id));
create policy notebooks_insert on public.notebooks
  for insert with check (owner_id = auth.uid());
create policy notebooks_update on public.notebooks
  for update using (is_owner(id));
create policy notebooks_delete on public.notebooks
  for delete using (is_owner(id));

-- Deliberately NO insert policy on notebook_members: the only two ways a row
-- is ever created are the owner-bootstrap trigger below and redeem_invite(),
-- both SECURITY DEFINER (bypasses RLS for that one insert). Deliberately NO
-- update policy either: blocks any client from promoting themselves from
-- collaborator to owner.
create policy members_select on public.notebook_members
  for select using (is_member(notebook_id));
create policy members_self_leave on public.notebook_members
  for delete using (user_id = auth.uid() and role <> 'owner');
create policy members_owner_removes on public.notebook_members
  for delete using (is_owner(notebook_id) and user_id <> auth.uid());

-- Owner-only for every operation, including select: a collaborator doesn't
-- need to see outstanding invite tokens to use the notebook, and redemption
-- goes through the redeem_invite() RPC below (SECURITY DEFINER), not a
-- direct select/insert against this table.
create policy invites_owner_all on public.notebook_invites
  for all using (is_owner(notebook_id)) with check (is_owner(notebook_id));

create policy notes_select on public.notes
  for select using (is_member(notebook_id));
create policy notes_insert on public.notes
  for insert with check (is_member(notebook_id));
create policy notes_update on public.notes
  for update using (is_member(notebook_id));
create policy notes_delete on public.notes
  for delete using (is_member(notebook_id));

-- ---------- triggers ----------

-- Auto-add the creator as owner. Must be SECURITY DEFINER: notebook_members
-- has no permissive insert policy, so a plain (non-definer) trigger function
-- would fail RLS on its own insert, breaking every notebook creation.
--
-- IMPORTANT client-side consequence: because this membership row is what
-- notebooks_select's is_member(id) check depends on, and it's only created
-- HERE (after the notebooks row already exists), a client that does
-- `INSERT INTO notebooks ... RETURNING ...` (e.g. supabase-js's
-- .insert().select()) will spuriously fail RLS - Postgres filters an
-- INSERT's RETURNING output through the SELECT policy, and this trigger
-- hasn't run yet from that RETURNING check's point of view within the same
-- statement. See createNotebook() in src/lib/services/cloudService.ts,
-- which deliberately does NOT chain .select() onto this insert for exactly
-- this reason - don't reintroduce it.
create or replace function public.notebooks_add_owner()
returns trigger
language plpgsql security definer set search_path = public, pg_temp as $$
begin
  insert into public.notebook_members (notebook_id, user_id, role)
  values (new.id, new.owner_id, 'owner');
  return new;
end;
$$;
create trigger notebooks_after_insert
  after insert on public.notebooks
  for each row execute function public.notebooks_add_owner();

-- Rejects a parent_id pointing at a note in a DIFFERENT notebook. RLS alone
-- can't express this - it only checks that the row being written belongs to
-- a notebook the caller is a member of, not that parent_id's own
-- notebook_id matches. This is the Postgres-side equivalent of guarding
-- against cross-vault linkage; cycle prevention (a note becoming its own
-- ancestor) still has to be ported into the frontend's CloudNotesService,
-- same as check_no_cycle in src-tauri/src/notes.rs - a plain constraint
-- can't walk an arbitrary-depth parent chain.
create or replace function public.notes_validate_parent()
returns trigger
language plpgsql as $$
begin
  if new.parent_id is not null and not exists (
    select 1 from public.notes where id = new.parent_id and notebook_id = new.notebook_id
  ) then
    raise exception 'parent_id must belong to the same notebook';
  end if;
  return new;
end;
$$;
create trigger notes_before_write
  before insert or update of parent_id on public.notes
  for each row execute function public.notes_validate_parent();

-- Bumps updated_at - scoped with "UPDATE OF <columns>" so it only FIRES for
-- an UPDATE statement whose SET clause actually names one of these columns,
-- mirroring notes.rs's per-command SQL exactly rather than approximating it
-- with a value-diff check: update_note's statement always SETs
-- title/content/is_markdown/is_locked/show_line_numbers (so this always
-- fires, unconditionally bumping updated_at, matching Rust's own
-- unconditional `now_iso()` on every save); move_note's and reorder_note's
-- dragged-note statement SETs parent_id (fires); reorder_note's sibling
-- statements SET sort_order only, naming none of these columns (does NOT
-- fire) - siblings being renumbered during a drag must never get their
-- updated_at touched, matching Rust exactly. sort_order is deliberately
-- excluded from both the column list and the function body for this reason.
create or replace function public.notes_touch_updated_at()
returns trigger
language plpgsql as $$
begin
  new.updated_at := now();
  return new;
end;
$$;
create trigger notes_before_update
  before update of title, content, is_markdown, is_locked, show_line_numbers, parent_id on public.notes
  for each row execute function public.notes_touch_updated_at();

-- ---------- invite redemption ----------
--
-- SECURITY DEFINER: the redeeming user isn't a notebook member yet, so by
-- design has no RLS visibility into notebook_invites and no write access to
-- notebook_members. The client already holds the token (from the
-- copy/pasted invite code) and passes it straight in - it never needs to
-- SELECT notebook_invites to redeem, so that table can stay owner-only.
create or replace function public.redeem_invite(p_token uuid)
returns uuid
language plpgsql security definer set search_path = public, pg_temp as $$
declare
  v_notebook_id uuid;
begin
  select notebook_id into v_notebook_id
  from public.notebook_invites
  where token = p_token and not revoked;

  if v_notebook_id is null then
    raise exception 'Invite is invalid or has been revoked';
  end if;

  insert into public.notebook_members (notebook_id, user_id, role)
  values (v_notebook_id, auth.uid(), 'collaborator')
  on conflict (notebook_id, user_id) do nothing;

  return v_notebook_id;
end;
$$;

revoke all on function public.redeem_invite(uuid) from public;
grant execute on function public.redeem_invite(uuid) to authenticated;
