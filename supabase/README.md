# Supabase setup for FlashPad cloud notebooks

FlashPad's local SQLite databases work with zero setup, as always. Cloud
notebooks (sync across devices, sharing with friends) are optional and need
a Supabase project.

## 1. Create a project

Go to [supabase.com](https://supabase.com), create an account/organization,
then "New project". Pick any name/region/password (the database password
isn't used by FlashPad directly - Supabase manages that connection).

## 2. Run the schema

Open **SQL Editor** in the Supabase dashboard, paste the entire contents of
[`schema.sql`](./schema.sql) in this folder, and click **Run**. This creates
the `notebooks` / `notebook_members` / `notebook_invites` / `notes` tables,
row-level security policies, and the `redeem_invite` function used for
joining a shared notebook via an invite code.

Email/password sign-up is enabled by default in a new Supabase project
(**Authentication -> Providers -> Email**) - no change needed there.

## 3. Get your API keys

**Project Settings -> API**. Copy:
- **Project URL**
- **anon / public** key (NOT the `service_role` key - that one must never
  ship in a desktop app)

## 4. Configure FlashPad

Copy `.env.example` (repo root) to `.env` and fill in the two values:

```
VITE_SUPABASE_URL=https://your-project.supabase.co
VITE_SUPABASE_ANON_KEY=your-anon-key
```

`.env` is gitignored - never commit real keys. Restart `npm run dev` /
`npm run tauri dev` after creating or changing it. Without a `.env`, FlashPad
runs exactly as before with only local databases; the Cloud tab in Settings
will say cloud notebooks aren't configured.
