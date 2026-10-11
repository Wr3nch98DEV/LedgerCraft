# Extending LedgerCraft

LedgerCraft keeps third-party services out of the core. Integrations plug in at well-defined seams.

## Tax returns — `taxreturns::TaxReturnForm`
```rust
pub trait TaxReturnForm {
    fn code(&self) -> &'static str;
    fn title(&self) -> &'static str;
    fn boxes(&self, t: &TaxTotals) -> Vec<(&'static str, &'static str, i64, bool)>; // (box, label, cents, is_total)
}
```
`TaxTotals` gives net and tax per tax-rate *report type* (OUTPUT, INPUT, CAPEXINPUT, ZERORATED, EXEMPTOUTPUT, REVERSECHARGE…) plus payroll wages/withholding. Register a new form in `form_for()`.

## Payroll — `payroll::PayrollJurisdiction`
```rust
pub trait PayrollJurisdiction {
    fn code(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn retirement_label(&self) -> &'static str;
    fn calc(&self, taxable: i64, eligible: i64, periods_per_year: i64, tax_code: &str, retirement_override_bp: Option<i64>) -> EmpCalc;
}
```
Bundled: AU, UK, NZ, US (simplified approximations — verify) and a generic bracket table configured in Payroll settings. Statutory filing payloads are produced by `payroll_filing_payload`; submitting them requires government developer registration.

## Bank feeds
Statement lines enter through `Ledger::import_lines(account_id, source, format, &[StmtLine])`, which de-duplicates on the provider's transaction id (`fitid`). A feed adapter (GoCardless Bank Account Data, Plaid, TrueLayer, Basiq, Akahu…) only has to:
1. Fetch transactions with its own credentials (store tokens outside the org file).
2. Map each to `StmtLine { date, amount (cents, + money in), payee, description, reference, fitid }`.
3. Call `import_lines` (or `POST /api/rpc/statements.import` with a CSV/OFX body).

## Payment gateways
The online invoice page is `/online/{token}`. A gateway plugin renders a "Pay now" button that redirects to the provider's hosted checkout and, on the provider's webhook, calls `payments.add` with the clearing bank account. Card data never touches LedgerCraft.

## OCR for the files inbox
PDF text is extracted natively. For photos set **`ocr_command`** (Settings API) to an executable that accepts `<image> stdout` and prints text — e.g. the path to `tesseract.exe`.

## Exchange rates
Rates are stored in `exchange_rates(currency, date, rate_e8, source)`. A rate-feed plugin writes there via `rates.set` (e.g. daily ECB reference rates).

## Webhooks
Events (`contact.created`, `document.created`, `payment.created`, `banktransaction.created`) are queued in `webhook_deliveries` for registered webhooks. A delivery worker POSTs them with an HMAC of the payload using the webhook secret.
