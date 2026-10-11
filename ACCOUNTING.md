# Posting rules

Every transaction type produces one balanced journal. Debits are positive, credits negative. *Tax* is the tax control account (e.g. "GST Payable" / "VAT"). All amounts are converted to the base currency per line; the control account takes the balancing figure.

## Sales invoice (ACCREC), on approval
| Account | Dr | Cr |
|---|---|---|
| Accounts Receivable | total | |
| Revenue account (per line) | | net |
| Tax | | tax |
| *Tracked item:* Cost of goods sold | avg cost × qty | |
| *Tracked item:* Inventory | | avg cost × qty |

## Bill (ACCPAY), on approval
| Account | Dr | Cr |
|---|---|---|
| Expense / asset account (per line) — tracked items always go to Inventory | net | |
| Tax | tax | |
| Accounts Payable | | total |

Reverse-charge lines record the tax for returns but post no tax to the ledger (output and input cancel).

## Credit notes
Mirror images of the above (sales credit: Dr revenue, Dr tax, Cr AR; returned tracked stock goes back at current average cost).

## Payment of an invoice (bill is the mirror)
| Account | Dr | Cr |
|---|---|---|
| Bank | amount at the payment rate | |
| Accounts Receivable | | amount at the invoice rate (final payment clears the exact remainder) |
| Realised Currency Gains | loss | gain |

## Credit / overpayment / prepayment allocation
No journal in the same currency (AR to AR). When rates differ, the difference goes to Realised Currency Gains.

## Spend money / receive money
Spend: Dr expense lines + Dr tax, Cr bank. Receive: Dr bank, Cr income lines + Cr tax.
Overpayments and prepayments: Dr bank, Cr AR (or Dr AP, Cr bank) as an unallocated credit for the contact.

## Transfer
Dr destination bank, Cr source bank; currency differences to Realised Currency Gains.

## Manual journal
As entered. With tax: each line's tax portion posts to the tax account. Auto-reversing journals post the exact reversal on the reversal date.

## Conversion balances
As entered at the conversion date; any difference goes to Historical Adjustment.

## Inventory adjustment
Increase: Dr Inventory, Cr adjustment account. Decrease: Dr adjustment account, Cr Inventory at average cost.

## Depreciation run
Dr Depreciation expense, Cr Accumulated depreciation (per asset type).

## Asset disposal
Dr Accumulated depreciation, Dr Bank (proceeds), Cr Asset cost, gain/loss to the chosen account.

## Pay run
| Account | Dr | Cr |
|---|---|---|
| Wages & salaries (earnings pay items) | gross | |
| Reimbursement accounts | reimbursements | |
| Superannuation / pension expense | employer contribution | |
| Wages (employer taxes, e.g. NI / FICA) | employer tax | |
| Payroll tax withholdings payable | | employee tax + employer tax |
| Superannuation / pension payable | | employer + employee contributions |
| Deduction liabilities | | other deductions |
| Wages payable | | net pay |

Paying net wages: Dr Wages payable, Cr Bank (recorded as a spend-money transaction so it can be reconciled).

## Currency revaluation
At the chosen date, open foreign-currency invoices/bills and foreign bank accounts are restated at the latest rate: Dr/Cr AR/AP/bank, Cr/Dr Unrealised Currency Gains (or Bank Revaluations). The journal reverses automatically the next day.

## Cash-basis reporting
Document journals are excluded; each payment, credit allocation and prepayment allocation recognises the document's income/expense/tax lines in proportion (payment ÷ document total) on the payment date. Bank-direct transactions and journals marked "show on cash basis" are included as-is.

## Rounding
* Tax: per line, half away from zero (accepted by the ATO, IRD, HMRC and CRA for invoice-level tax).
* BAS / VAT100 boxes that must be whole units are truncated to whole dollars/pounds as the forms require.
* Banker's rounding is available in `money::div_round_bankers` for jurisdictions that require it.
