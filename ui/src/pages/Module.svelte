<script lang="ts">
  // Router for feature-module pages (fixed assets, expenses, projects, payroll, tax returns, files, workpapers).
  import Assets from './modules/Assets.svelte';
  import AssetEdit from './modules/AssetEdit.svelte';
  import TaxReturns from './modules/TaxReturns.svelte';
  import TaxReturn from './modules/TaxReturn.svelte';
  import Expenses from './modules/Expenses.svelte';
  import ExpenseEdit from './modules/ExpenseEdit.svelte';
  import Projects from './modules/Projects.svelte';
  import Project from './modules/Project.svelte';
  import Payroll from './modules/Payroll.svelte';
  import Employees from './modules/Employees.svelte';
  import Employee from './modules/Employee.svelte';
  import PayRuns from './modules/PayRuns.svelte';
  import PayRun from './modules/PayRun.svelte';
  import Timesheets from './modules/Timesheets.svelte';
  import Leave from './modules/Leave.svelte';
  import PayrollSettings from './modules/PayrollSettings.svelte';
  import Files from './modules/Files.svelte';
  import Workpapers from './modules/Workpapers.svelte';
  let props: any = $props();
  const map: Record<string, any> = {
    assets: Assets, asset: AssetEdit, 'tax-returns': TaxReturns, 'tax-return': TaxReturn, expenses: Expenses, expense: ExpenseEdit,
    projects: Projects, project: Project, payroll: Payroll, employees: Employees, employee: Employee, 'pay-runs': PayRuns, 'pay-run': PayRun,
    timesheets: Timesheets, leave: Leave, 'payroll-settings': PayrollSettings, files: Files, workpapers: Workpapers,
  };
  const C = $derived(map[props.name]);
</script>

<C {...props} />
