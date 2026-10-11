# Local API

LedgerCraft listens on **127.0.0.1 only**. There are two APIs.

## 1. REST API for integrations — `/api/v1/…`
Create a token in **Settings → Connected apps & API** and send it as `Authorization: Bearer lc_…`.

| Method & path | Scope | Description |
|---|---|---|
| `GET /api/v1/contacts?q=` | contacts:read | List contacts |
| `GET /api/v1/contacts/{id}` | contacts:read | Contact with summary |
| `POST /api/v1/contacts` | contacts:write | Create (`{name, email, …}`) |
| `GET /api/v1/invoices?status=&q=` | sales:read | List invoices |
| `POST /api/v1/invoices` | sales:write | Create (`DocInput`; `status: "AUTHORISED"` to approve) |
| `GET /api/v1/bills` / `POST /api/v1/bills` | purchases:* | Bills |
| `GET /api/v1/documents/{id}` | sales:read | Any document |
| `POST /api/v1/payments` | banking:write | `{document_id, account_id, date, amount, reference}` |
| `POST /api/v1/bank-transactions` | banking:write | Spend/receive money |
| `GET /api/v1/accounts`, `/tax-rates`, `/items` | read | Reference data |
| `GET /api/v1/journals/{id}` | accounting:read | A posted journal |
| `GET /api/v1/reports/{name}?from=&to=&date=` | reports:read | Any report (`profit-and-loss`, `balance-sheet`, `trial-balance`, `aged-receivables`, …) |
| `GET /api/v1/openapi.json` | any | OpenAPI 3 description |

Example:
```bash
curl -H "Authorization: Bearer lc_xxx" -X POST http://127.0.0.1:4180/api/v1/invoices \
  -H "content-type: application/json" \
  -d '{"contact_name":"Acme","date":"2026-10-12","lines":[{"description":"Consulting","quantity":"2","unit_price":"150","account_id":3}],"status":"AUTHORISED"}'
```
Money in requests is a decimal string (`"150.00"`); money in responses is integer cents.

## 2. Application RPC — `POST /api/rpc/{method}`
Used by the UI. Body is a JSON object of parameters; session header `x-session` when passwords are enabled. Every method checks the caller's role permissions. Method names are grouped (`docs.save`, `payments.add`, `rec.match`, `reports.run`, `payroll.post`, …); see `crates/ledger-server/src/rpc.rs`, `crates/ledger-core/src/ext.rs` and `crates/ledger-core/src/modules.rs` for the full list.
