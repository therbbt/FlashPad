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

## 5. Building/releasing with cloud notebooks enabled

`VITE_SUPABASE_URL`/`VITE_SUPABASE_ANON_KEY` are Vite **build-time** env
vars - they get baked into the bundle wherever `npm run tauri build` (or
`npm run build`) actually runs, not read at runtime from a file that ships
with the installer. A `.env` on your own machine only affects builds you run
locally.

The CI workflows (`.github/workflows/build-windows.yml`,
`build-linux.yml`, `release.yml`) build on GitHub's runners, which never see
your local `.env` (it's gitignored, not checked out). For those builds to
have cloud notebooks enabled too, add `VITE_SUPABASE_URL` and
`VITE_SUPABASE_ANON_KEY` as **repository secrets**
(Settings → Secrets and variables → Actions) with the same values as your
`.env` - the workflows already forward them if the secrets exist. Skipping
this is fine: cloud notebooks are fully optional, so a build without these
secrets just has the sign-in UI hidden, same as running locally with no
`.env`.
