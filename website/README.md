# Brutus website

Product website and early-access waitlist for the Brutus native agent workspace. The Rust application stays in the repository root; the website is a separate Vinext/React application in this directory.

## Develop

Requires Node 22.13+.

```sh
npm ci
npm run db:generate # only after changing db/schema.ts
npm run build
node --import ./scripts/sites-env.mjs ./node_modules/wrangler/bin/wrangler.js d1 execute DB --local --config dist/server/wrangler.json --persist-to .wrangler/state --file drizzle/0000_tidy_blue_marvel.sql
npm run dev
```

Apply the initial migration once per local database. After that, apply only newly generated migration files. Development defaults to port 5173. `npm start -- --port 5178` serves the production build locally and shares the same local D1 state.

## Verify

```sh
npx tsc --noEmit
npm run build
npm start -- --port 5178
# In another terminal:
node scripts/check-waitlist.mjs
```

The integration check verifies real API persistence, validation, duplicate handling, removal protection, and rejoining. It removes its test signups. Local and hosted databases are separate.

## Waitlist

`POST /api/waitlist` accepts an email, optional platform (`macos`, `windows`, `linux`, `multiple`), and empty `website` honeypot. Email is trimmed, lowercased, and stored once. Entries live in Cloudflare D1 via the `DB` binding declared in `.openai/hosting.json`. There is no public endpoint exposing signup records.

New signups receive a random removal token. Only its SHA-256 digest is saved in the database; the token stays in browser storage. `DELETE /api/waitlist` requires that token. Duplicate requests never disclose or rotate it. A honeypot, same-origin check, request-size validation, and a daily per-IP attempt limit reduce automated submissions. IP addresses are not stored directly; rate-limit digests expire after a day and are cleaned up on later signup requests.

The form collects registrations. It does **not** send email confirmations or newsletters; connect a chosen email provider when launch emails are needed. No provider credentials are required to collect signups.

The owner can inspect entries through the Sites database tools: read the database overview first, then the `DB` binding's `waitlist` table. Export only `email`, `platform`, and `created_at` for an email provider; the timestamp is Unix seconds. The removal-token digest is internal and should not be exported. There is no unauthenticated admin page.

## Hosting

The site is registered with Sites; `.openai/hosting.json` holds its existing project ID. Reuse it for all future publications. Sites provisions D1 and applies the checked-in Drizzle migration when deploying. New Sites start owner-private. Public access or a custom domain can be configured when ready to launch.

Use the installed Sites skill to publish. Prepare an isolated publication checkout outside this parent Rust repository, copying authored files and the matching `dist` output while excluding `node_modules`, local state, and Git metadata. The Sites source workflow manages its own Git source repository, preserving this project's GitHub remote. Never copy credentials into files or source control.

Product copy and the illustrative interactive demo are in `app/page.tsx`; waitlist UI is in `app/waitlist.tsx`, API in `app/api/waitlist/route.ts`, and visual styles in `app/globals.css`.
