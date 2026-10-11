# Contributing

## Setup
* Rust (stable) and Node.js LTS.
* `cd ui && npm install`
* Dev loop: run the server `cargo run -p ledger-server -- --port 4180` and the UI with hot reload `cd ui && npm run dev` (Vite proxies `/api` to 4180) → http://localhost:5173

## Rules
1. **All ledger writes go through `Ledger::post_journal` / `reverse_journal`.** Never insert into `journals` or `journal_lines` directly.
2. Money is integer cents. Never use floats for money.
3. Migrations are append-only: add `schema_vN.sql` and register it in `db.rs`. Never edit a shipped migration.
4. Every feature that touches the ledger needs a test in `crates/ledger-core/tests/accounting.rs` that calls `assert_invariants`.
5. Parameterise all SQL. String formatting is for display only.
6. UI: dark theme first; use the CSS tokens in `ui/src/app.css` (never hard-code colours); every control needs a label.

## Tests
```bash
cargo test -p ledger-core     # unit + integration + property tests
```
`start.bat test` runs the same on Windows.

## Releasing
`start.bat` builds a release binary at `%LOCALAPPDATA%\LedgerCraft\build\target\release\ledger-server.exe` with the UI embedded — that single file is the whole app.
