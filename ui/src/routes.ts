import Dashboard from './pages/Dashboard.svelte';
import DocList from './pages/DocList.svelte';
import DocEdit from './pages/DocEdit.svelte';
import DocRedirect from './pages/DocRedirect.svelte';
import Overview from './pages/Overview.svelte';
import Contacts from './pages/Contacts.svelte';
import ContactView from './pages/ContactView.svelte';
import ContactEdit from './pages/ContactEdit.svelte';
import ContactGroups from './pages/ContactGroups.svelte';
import Products from './pages/Products.svelte';
import ProductEdit from './pages/ProductEdit.svelte';
import Banking from './pages/Banking.svelte';
import BankAccount from './pages/BankAccount.svelte';
import BankTxForm from './pages/BankTxForm.svelte';
import TransferForm from './pages/TransferForm.svelte';
import BankRules from './pages/BankRules.svelte';
import ImportStatement from './pages/ImportStatement.svelte';
import Reports from './pages/Reports.svelte';
import ReportView from './pages/ReportView.svelte';
import Chart from './pages/Chart.svelte';
import TaxRates from './pages/TaxRates.svelte';
import Journals from './pages/Journals.svelte';
import JournalEdit from './pages/JournalEdit.svelte';
import Settings from './pages/Settings.svelte';
import Recode from './pages/Recode.svelte';
import Budgets from './pages/Budgets.svelte';
import Assurance from './pages/Assurance.svelte';
import Currencies from './pages/Currencies.svelte';
import Tracking from './pages/Tracking.svelte';
import Conversion from './pages/Conversion.svelte';
import Repeating from './pages/Repeating.svelte';
import Batch from './pages/Batch.svelte';
import Cashflow from './pages/Cashflow.svelte';
import Snapshot from './pages/Snapshot.svelte';
import Module from './pages/Module.svelte';
import JournalView from './pages/JournalView.svelte';

const wrap = (c: any, extra: Record<string, any>) => (anchor: any, props: any) => c(anchor, { ...props, ...extra });

export const routes: [string, any][] = [
  ['/', Dashboard],
  ['/doc/:id', DocRedirect],
  ['/journal/:id', JournalView],
  ['/sales', wrap(Overview, { side: 'sales' })],
  ['/purchases', wrap(Overview, { side: 'purchases' })],
  ['/sales/invoices', wrap(DocList, { type: 'ACCREC' })],
  ['/sales/invoices/new', wrap(DocEdit, { type: 'ACCREC' })],
  ['/sales/invoices/:id', wrap(DocEdit, { type: 'ACCREC' })],
  ['/sales/quotes', wrap(DocList, { type: 'QUOTE' })],
  ['/sales/quotes/new', wrap(DocEdit, { type: 'QUOTE' })],
  ['/sales/quotes/:id', wrap(DocEdit, { type: 'QUOTE' })],
  ['/sales/credit-notes', wrap(DocList, { type: 'ACCRECCREDIT' })],
  ['/sales/credit-notes/new', wrap(DocEdit, { type: 'ACCRECCREDIT' })],
  ['/sales/credit-notes/:id', wrap(DocEdit, { type: 'ACCRECCREDIT' })],
  ['/sales/repeating', wrap(Repeating, { kind: 'ACCREC' })],
  ['/purchases/bills', wrap(DocList, { type: 'ACCPAY' })],
  ['/purchases/bills/new', wrap(DocEdit, { type: 'ACCPAY' })],
  ['/purchases/bills/:id', wrap(DocEdit, { type: 'ACCPAY' })],
  ['/purchases/orders', wrap(DocList, { type: 'PO' })],
  ['/purchases/orders/new', wrap(DocEdit, { type: 'PO' })],
  ['/purchases/orders/:id', wrap(DocEdit, { type: 'PO' })],
  ['/purchases/credit-notes', wrap(DocList, { type: 'ACCPAYCREDIT' })],
  ['/purchases/credit-notes/new', wrap(DocEdit, { type: 'ACCPAYCREDIT' })],
  ['/purchases/credit-notes/:id', wrap(DocEdit, { type: 'ACCPAYCREDIT' })],
  ['/purchases/repeating', wrap(Repeating, { kind: 'ACCPAY' })],
  ['/purchases/batch', Batch],
  ['/contacts', Contacts],
  ['/contacts/groups', ContactGroups],
  ['/contacts/new', ContactEdit],
  ['/contacts/:id/edit', ContactEdit],
  ['/contacts/:id', ContactView],
  ['/products', Products],
  ['/products/new', ProductEdit],
  ['/products/:id', ProductEdit],
  ['/banking', Banking],
  ['/banking/rules', BankRules],
  ['/banking/spend', wrap(BankTxForm, { kind: 'SPEND' })],
  ['/banking/receive', wrap(BankTxForm, { kind: 'RECEIVE' })],
  ['/banking/transaction/:id', BankTxForm],
  ['/banking/transfer', TransferForm],
  ['/banking/:id/import', ImportStatement],
  ['/banking/:id', BankAccount],
  ['/reports', Reports],
  ['/reports/:name', ReportView],
  ['/accounting/chart', Chart],
  ['/accounting/tax-rates', TaxRates],
  ['/accounting/journals', Journals],
  ['/accounting/repeating-journals', wrap(Repeating, { kind: 'MJ' })],
  ['/accounting/journals/new', JournalEdit],
  ['/accounting/journals/:id', JournalEdit],
  ['/accounting/recode', Recode],
  ['/accounting/budgets', Budgets],
  ['/accounting/assurance', Assurance],
  ['/accounting/currencies', Currencies],
  ['/accounting/tracking', Tracking],
  ['/accounting/conversion', Conversion],
  ['/settings', Settings],
  ['/settings/:section', Settings],
  ['/cashflow', Cashflow],
  ['/snapshot', Snapshot],
  // Feature modules (fixed assets, expenses, projects, payroll, tax returns, files, workpapers)
  ['/accounting/assets', wrap(Module, { name: 'assets' })],
  ['/accounting/assets/:id', wrap(Module, { name: 'asset' })],
  ['/accounting/tax-returns', wrap(Module, { name: 'tax-returns' })],
  ['/accounting/tax-returns/:id', wrap(Module, { name: 'tax-return' })],
  ['/accounting/workpapers', wrap(Module, { name: 'workpapers' })],
  ['/expenses', wrap(Module, { name: 'expenses' })],
  ['/expenses/new', wrap(Module, { name: 'expense' })],
  ['/expenses/:id', wrap(Module, { name: 'expense' })],
  ['/projects', wrap(Module, { name: 'projects' })],
  ['/projects/:id', wrap(Module, { name: 'project' })],
  ['/payroll', wrap(Module, { name: 'payroll' })],
  ['/payroll/employees', wrap(Module, { name: 'employees' })],
  ['/payroll/employees/:id', wrap(Module, { name: 'employee' })],
  ['/payroll/pay-runs', wrap(Module, { name: 'pay-runs' })],
  ['/payroll/pay-runs/:id', wrap(Module, { name: 'pay-run' })],
  ['/payroll/timesheets', wrap(Module, { name: 'timesheets' })],
  ['/payroll/leave', wrap(Module, { name: 'leave' })],
  ['/payroll/settings', wrap(Module, { name: 'payroll-settings' })],
  ['/files', wrap(Module, { name: 'files' })],
];
