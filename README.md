# Orbifon — Gwalior Pilot Backend (Rust)

Text-based social network for the ABV-IIITM Gwalior + MITS Gwalior pilot.
Stack: **Axum + SQLx (MySQL) + Argon2id + JWT**. All dependencies are
MIT/Apache-2.0 licensed — no AGPL, per the closed-source commercial
constraint.

## What's implemented (Steps 1-8 of the roadmap)

- **Step 1** — Full MySQL schema + Gwalior pilot seed data (`migrations/0001_init.sql`)
- **Step 2** — Config loader, DB pool, centralized error handling (`config.rs`, `db.rs`, `error.rs`)
- **Step 3** — Domain-locked registration (`@iiitm.ac.in` / `@mitsgwalior.in` only), Argon2id password hashing, JWT auth middleware (`auth.rs`)
- **Step 4/5** — Post creation, image upload (magic-byte validated, no video), hot-score-ranked feed (`feed.rs`)
- **Step 6** — Voting (toggle/switch-safe), comments, reposts (`feed.rs`)
- **Step 7/8** — College-locked Hot Town servers/channels, long-poll-ready messages, `#confessions` identity masking (`hot_town.rs`)

## Running locally

1. Install Rust (stable) and a local MySQL 8 instance.
2. `cp .env.example .env` and fill in a real `DATABASE_URL` + a random 32+ char `JWT_SECRET`.
3. Create the database: `mysql -u root -p -e "CREATE DATABASE orbifon CHARACTER SET utf8mb4"`
4. `cargo run` — migrations run automatically on startup (via `sqlx::migrate!`).
5. Server listens on `http://0.0.0.0:8080` by default. Check `GET /api/health`.

## API surface

| Method | Path | Auth | Notes |
|---|---|---|---|
| POST | `/api/auth/register` | — | Rejects any non-pilot email domain |
| POST | `/api/auth/login` | — | Returns JWT |
| GET | `/api/feed?sort=hot\|new&limit=&offset=` | — | Hot-score or newest sort |
| POST | `/api/posts` | ✔ | `{ body?, image_path? }` |
| POST | `/api/uploads/image` | ✔ | multipart, field name `image` |
| POST | `/api/posts/:id/vote` | ✔ | `{ vote_type: "up"\|"down" }` — toggles/switches |
| GET/POST | `/api/posts/:id/comments` | GET open, POST ✔ | |
| POST | `/api/posts/:id/repost` | ✔ | one repost per user per post |
| GET | `/api/hot-town/my-server` | ✔ | your college's server + channels |
| GET/POST | `/api/hot-town/channels/:id/messages` | ✔ | `since_id` cursor for polling; `#confessions` responses are auto-masked |

Send the JWT as `Authorization: Bearer <token>` on any ✔ route.

## Design notes worth knowing before you extend this

- **Hot score** is stored and recalculated on every vote (see `feed::compute_hot_score`), not computed at read time — keeps the feed query a cheap indexed sort as the table grows.
- **Confession anonymity is enforced server-side**, at response-serialization time (`models::MessageResponse::from`), never by omitting `user_id` from the database. The real author is always recoverable for moderation; the public API response simply never contains it for anonymous channels.
- **Hot Town college-siloing** goes through one choke point: `hot_town::assert_channel_access`. Every channel route calls it before touching `messages`.
- **`client_nonce`** on messages makes the send-message endpoint safe to retry on flaky mobile connections without creating duplicates.
- Long-polling is implemented as a `since_id` cursor GET — swap this for a WebSocket push later without changing the response shape the frontend already expects.

## Not yet built (later steps)

- Real email delivery for verification tokens (currently logged, not sent)
- Rate limiting, CSRF, upload directory hardening (Step 10)
- Frontend (Step 9)

## A note on this code

This was generated in one large batch per your request. I'd strongly
recommend running `cargo check` locally and testing each endpoint with
a tool like `curl`/Postman before treating any of it as production-
ready — a single-pass generation this size is far more likely to have
a subtle bug than the same code built and reviewed incrementally.
