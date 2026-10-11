//! Payroll: employees, pay calendars, pay items, timesheets, leave, pay runs and payslips.
//! Tax is calculated by a pluggable [`PayrollJurisdiction`]. The bundled tables are simplified
//! approximations of published rates and MUST be verified by a qualified professional.
//! Statutory filing (STP, RTI, etc.) requires government developer registration — LedgerCraft
//! only generates export files.

use crate::{banking::BankTxInput, documents::LineInput, error::invalid, journal::{JLine, NewJournal}, money, util, Error, Ledger, Result};
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};

pub struct EmpCalc { pub tax: i64, pub employee_retirement: i64, pub employer_retirement: i64, pub employer_tax: i64, pub notes: Vec<String> }

pub trait PayrollJurisdiction {
    fn code(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn retirement_label(&self) -> &'static str;
    /// Calculate withholding etc. for one pay period. `taxable` and `eligible` are cents for the period.
    fn calc(&self, taxable: i64, eligible: i64, ppy: i64, tax_code: &str, retirement_override_bp: Option<i64>) -> EmpCalc;
}

/// Progressive tax on an annual amount: brackets of (upper bound cents or i64::MAX, rate bp).
fn progressive(annual: i64, brackets: &[(i64, i64)]) -> i64 {
    let mut tax: i128 = 0; let mut lower = 0i64;
    for (upper, rate) in brackets {
        if annual <= lower { break; }
        let slice = annual.min(*upper) - lower;
        tax += slice as i128 * *rate as i128;
        lower = *upper;
    }
    money::div_round(tax, 10_000) as i64
}
fn per_period(annual: i64, ppy: i64) -> i64 { money::div_round(annual as i128, ppy as i128) as i64 }
const D: i64 = 100; // dollars → cents

pub struct Au;
impl PayrollJurisdiction for Au {
    fn code(&self) -> &'static str { "AU" }
    fn name(&self) -> &'static str { "Australia (PAYG withholding approx. 2025-26, SG 12%)" }
    fn retirement_label(&self) -> &'static str { "Superannuation" }
    fn calc(&self, taxable: i64, eligible: i64, ppy: i64, tax_code: &str, ovr: Option<i64>) -> EmpCalc {
        let annual = taxable * ppy;
        let tft = tax_code != "NOTFT"; // tax-free threshold claimed unless "NOTFT"
        let brackets: Vec<(i64, i64)> = if tft { vec![(18_200 * D, 0), (45_000 * D, 1_600), (135_000 * D, 3_000), (190_000 * D, 3_700), (i64::MAX, 4_500)] }
            else { vec![(45_000 * D, 1_600), (135_000 * D, 3_000), (190_000 * D, 3_700), (i64::MAX, 4_500)] };
        let medicare = if annual > 27_222 * D { money::div_round(annual as i128 * 200, 10_000) as i64 } else { 0 };
        let tax = ((per_period(progressive(annual, &brackets) + medicare, ppy)) / 100) * 100; // whole dollars
        EmpCalc { tax, employee_retirement: 0, employer_retirement: money::div_round(eligible as i128 * ovr.unwrap_or(1_200) as i128, 10_000) as i64, employer_tax: 0,
            notes: vec!["Approximation of ATO tax tables — verify before paying".into()] }
    }
}

pub struct Uk;
impl PayrollJurisdiction for Uk {
    fn code(&self) -> &'static str { "UK" }
    fn name(&self) -> &'static str { "United Kingdom (PAYE/NI approx. 2025-26, auto-enrolment)" }
    fn retirement_label(&self) -> &'static str { "Workplace pension" }
    fn calc(&self, taxable: i64, _eligible: i64, ppy: i64, tax_code: &str, ovr: Option<i64>) -> EmpCalc {
        let annual = taxable * ppy;
        let allowance = tax_code.trim_end_matches(|c: char| c.is_alphabetic()).parse::<i64>().map(|n| n * 10 * D + 9 * D).unwrap_or(12_570 * D);
        let taxable_annual = (annual - allowance).max(0);
        let income_tax = progressive(taxable_annual, &[(37_700 * D, 2_000), (125_140 * D - 12_570 * D, 4_000), (i64::MAX, 4_500)]);
        let ni = progressive(annual, &[(12_570 * D, 0), (50_270 * D, 800), (i64::MAX, 200)]);
        let er_ni = progressive(annual, &[(5_000 * D, 0), (i64::MAX, 1_500)]);
        let qualifying = (annual.min(50_270 * D) - 6_240 * D).max(0);
        let ee_pension = per_period(money::div_round(qualifying as i128 * 500, 10_000) as i64, ppy);
        let er_pension = per_period(money::div_round(qualifying as i128 * ovr.unwrap_or(300) as i128, 10_000) as i64, ppy);
        EmpCalc { tax: per_period(income_tax + ni, ppy), employee_retirement: ee_pension, employer_retirement: er_pension, employer_tax: per_period(er_ni, ppy),
            notes: vec!["Cumulative PAYE not applied (week1/month1 basis) — verify against HMRC".into()] }
    }
}

pub struct Nz;
impl PayrollJurisdiction for Nz {
    fn code(&self) -> &'static str { "NZ" }
    fn name(&self) -> &'static str { "New Zealand (PAYE incl. ACC levy approx., KiwiSaver 3%)" }
    fn retirement_label(&self) -> &'static str { "KiwiSaver" }
    fn calc(&self, taxable: i64, eligible: i64, ppy: i64, _tax_code: &str, ovr: Option<i64>) -> EmpCalc {
        let annual = taxable * ppy;
        let paye = progressive(annual, &[(15_600 * D, 1_050), (53_500 * D, 1_750), (78_100 * D, 3_000), (180_000 * D, 3_300), (i64::MAX, 3_900)]);
        let acc = money::div_round(annual.min(152_790 * D) as i128 * 167, 10_000) as i64;
        let ks = money::div_round(eligible as i128 * 300, 10_000) as i64;
        EmpCalc { tax: per_period(paye + acc, ppy), employee_retirement: ks, employer_retirement: money::div_round(eligible as i128 * ovr.unwrap_or(300) as i128, 10_000) as i64, employer_tax: 0,
            notes: vec!["ESCT on employer KiwiSaver not deducted — verify against IRD".into()] }
    }
}

pub struct Us;
impl PayrollJurisdiction for Us {
    fn code(&self) -> &'static str { "US" }
    fn name(&self) -> &'static str { "United States (federal withholding & FICA approx. 2025, single)" }
    fn retirement_label(&self) -> &'static str { "401(k) employer match" }
    fn calc(&self, taxable: i64, eligible: i64, ppy: i64, _tax_code: &str, ovr: Option<i64>) -> EmpCalc {
        let annual = (taxable * ppy - 15_000 * D).max(0);
        let fed = progressive(annual, &[(11_925 * D, 1_000), (48_475 * D, 1_200), (103_350 * D, 2_200), (197_300 * D, 2_400), (250_525 * D, 3_200), (626_350 * D, 3_500), (i64::MAX, 3_700)]);
        let ss = money::div_round(taxable as i128 * 620, 10_000) as i64;
        let med = money::div_round(taxable as i128 * 145, 10_000) as i64;
        EmpCalc { tax: per_period(fed, ppy) + ss + med, employee_retirement: 0, employer_retirement: money::div_round(eligible as i128 * ovr.unwrap_or(0) as i128, 10_000) as i64, employer_tax: ss + med,
            notes: vec!["State income tax not calculated — add a deduction pay item".into()] }
    }
}

/// Configurable bracket table stored in settings key `payroll_brackets` as JSON [[upper_dollars|null, rate_percent], ...].
pub struct GenericJurisdiction { brackets: Vec<(i64, i64)>, retirement_bp: i64 }
impl PayrollJurisdiction for GenericJurisdiction {
    fn code(&self) -> &'static str { "GENERIC" }
    fn name(&self) -> &'static str { "Generic bracket table (configure in Payroll settings)" }
    fn retirement_label(&self) -> &'static str { "Pension" }
    fn calc(&self, taxable: i64, eligible: i64, ppy: i64, _t: &str, ovr: Option<i64>) -> EmpCalc {
        EmpCalc { tax: per_period(progressive(taxable * ppy, &self.brackets), ppy), employee_retirement: 0,
            employer_retirement: money::div_round(eligible as i128 * ovr.unwrap_or(self.retirement_bp) as i128, 10_000) as i64, employer_tax: 0, notes: vec![] }
    }
}

pub fn periods_per_year(freq: &str) -> i64 { match freq { "WEEKLY" => 52, "FORTNIGHTLY" => 26, "FOURWEEKLY" => 13, "MONTHLY" => 12, "QUARTERLY" => 4, _ => 26 } }

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct EmployeeInput {
    pub first_name: String, pub last_name: String, pub email: String, pub address: String, pub start_date: String, pub termination_date: Option<String>,
    pub tax_id: String, pub tax_code: String, pub employment_type: String, pub pay_calendar_id: Option<i64>, pub pay_basis: String,
    pub annual_salary: Option<String>, pub hourly_rate: Option<String>, pub hours_per_week: Option<String>, pub super_rate: Option<String>, pub bank_account: String,
    pub template: Vec<Value>,
}

impl Ledger {
    pub fn jurisdiction(&self) -> Result<Box<dyn PayrollJurisdiction>> {
        Ok(match self.setting_or("payroll_jurisdiction", "GENERIC")?.as_str() {
            "AU" => Box::new(Au), "UK" | "GB" => Box::new(Uk), "NZ" => Box::new(Nz), "US" => Box::new(Us),
            _ => {
                let raw = self.setting_or("payroll_brackets", "[[20000, 0], [60000, 20], [null, 30]]")?;
                let v: Vec<(Option<i64>, f64)> = serde_json::from_str(&raw).unwrap_or_default();
                let brackets = v.into_iter().map(|(u, r)| (u.map(|x| x * D).unwrap_or(i64::MAX), (r * 100.0).round() as i64)).collect();
                let retirement_bp = self.setting_or("payroll_retirement_bp", "0")?.parse().unwrap_or(0);
                Box::new(GenericJurisdiction { brackets, retirement_bp })
            }
        })
    }

    fn sys_or_create(&self, key: &str, code: &str, name: &str, kind: &str) -> Result<i64> {
        if let Ok(id) = self.sys_account(key) { return Ok(id); }
        let class = crate::accounts::class_of(kind)?;
        let code = if self.account_by_code(code)?.is_some() { format!("{code}A") } else { code.to_string() };
        self.conn.execute("INSERT INTO accounts(code, name, type, class, system_account) VALUES (?1, ?2, ?3, ?4, ?5)", params![code, name, kind, class, key])?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Seed pay items, leave types and a pay calendar the first time payroll is used.
    pub fn ensure_payroll_setup(&self) -> Result<()> {
        let n: i64 = self.conn.query_row("SELECT COUNT(*) FROM pay_items", [], |r| r.get(0))?;
        if n > 0 { return Ok(()); }
        let wages = self.sys_or_create("WAGESEXPENSE", "477", "Wages & Salaries", "EXPENSE")?;
        let sup_exp = self.sys_or_create("SUPEREXPENSE", "478", "Superannuation / Pension", "EXPENSE")?;
        let sup_liab = self.sys_or_create("SUPERPAYABLE", "826", "Superannuation / Pension Payable", "CURRLIAB")?;
        let ded_liab = self.sys_or_create("PAYROLLDEDUCTIONS", "827", "Payroll Deductions Payable", "CURRLIAB")?;
        let travel = self.account_by_code("493")?.unwrap_or(wages);
        let j = self.jurisdiction()?;
        for (kind, name, acct, liab, rate_type, mult, taxable, eligible) in [
            ("EARNINGS", "Ordinary hours", Some(wages), None, "RATEPERUNIT", None, 1, 1),
            ("EARNINGS", "Overtime (time and a half)", Some(wages), None, "MULTIPLE", Some(15_000), 1, 0),
            ("EARNINGS", "Bonus", Some(wages), None, "FIXED", None, 1, 0),
            ("EARNINGS", "Allowance", Some(wages), None, "FIXED", None, 1, 0),
            ("SUPER", j.retirement_label(), Some(sup_exp), Some(sup_liab), "PERCENT", None, 0, 0),
            ("DEDUCTION", "Salary sacrifice / additional contribution", None, Some(sup_liab), "FIXED", None, 0, 0),
            ("DEDUCTION", "Union fees", None, Some(ded_liab), "FIXED", None, 0, 0),
            ("DEDUCTION", "Child support / garnishee", None, Some(ded_liab), "FIXED", None, 0, 0),
            ("REIMBURSEMENT", "Travel reimbursement", Some(travel), None, "FIXED", None, 0, 0),
        ] {
            self.conn.execute("INSERT INTO pay_items(kind, name, account_id, liability_account_id, rate_type, multiplier_bp, taxable, super_eligible, system) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,1)",
                params![kind, name, acct, liab, rate_type, mult, taxable, eligible])?;
        }
        let (annual, personal) = match j.code() { "AU" => (15_200, 7_600), "UK" => (21_280, 0), "NZ" => (16_000, 8_000), _ => (8_000, 4_000) };
        self.conn.execute("INSERT INTO leave_types(name, accrual_hours_per_year_e2) VALUES (?1, ?2)", params![if j.code() == "UK" { "Holiday" } else { "Annual leave" }, annual])?;
        if personal > 0 { self.conn.execute("INSERT INTO leave_types(name, accrual_hours_per_year_e2) VALUES (?1, ?2)", params![if j.code() == "NZ" { "Sick leave" } else { "Personal / carer's leave" }, personal])?; }
        let t = util::parse_date(&util::today())?;
        let monday = t - chrono::Duration::days(chrono::Datelike::weekday(&t).num_days_from_monday() as i64);
        self.conn.execute("INSERT INTO pay_calendars(name, frequency, start_date) VALUES ('Fortnightly', 'FORTNIGHTLY', ?1)", [util::fmt_date(monday - chrono::Duration::days(14))])?;
        self.conn.execute("INSERT INTO pay_calendars(name, frequency, start_date) VALUES ('Monthly', 'MONTHLY', ?1)", [util::fmt_date(chrono::NaiveDate::from_ymd_opt(chrono::Datelike::year(&t), chrono::Datelike::month(&t), 1).unwrap())])?;
        Ok(())
    }

    pub fn payroll_settings(&self) -> Result<Value> {
        self.ensure_payroll_setup()?;
        let j = self.jurisdiction()?;
        Ok(json!({
            "jurisdiction": j.code(), "jurisdiction_name": j.name(), "retirement_label": j.retirement_label(),
            "brackets": self.setting_or("payroll_brackets", "[[20000, 0], [60000, 20], [null, 30]]")?,
            "pay_items": util::query_json(&self.conn, "SELECT p.*, a.name AS account_name, b.name AS liability_name FROM pay_items p LEFT JOIN accounts a ON a.id = p.account_id LEFT JOIN accounts b ON b.id = p.liability_account_id ORDER BY kind, id", [])?,
            "leave_types": util::query_json(&self.conn, "SELECT * FROM leave_types ORDER BY id", [])?,
            "calendars": util::query_json(&self.conn, "SELECT * FROM pay_calendars ORDER BY id", [])?,
            "disclaimer": "Tax tables are simplified approximations. Verify every pay run with a qualified professional. Statutory filing (STP, RTI, etc.) requires government developer registration — LedgerCraft generates export files only.",
        }))
    }

    pub fn save_pay_item(&self, id: Option<i64>, p: &Value) -> Result<i64> {
        let name = p["name"].as_str().unwrap_or("").trim();
        if name.is_empty() { return invalid("pay item name is required"); }
        let kind = p["kind"].as_str().unwrap_or("EARNINGS");
        match id {
            None => { self.conn.execute("INSERT INTO pay_items(kind, name, account_id, liability_account_id, rate_type, multiplier_bp, taxable, super_eligible) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![kind, name, p["account_id"].as_i64(), p["liability_account_id"].as_i64(), p["rate_type"].as_str().unwrap_or("FIXED"), p["multiplier_bp"].as_i64(), p["taxable"].as_bool().unwrap_or(true) as i64, p["super_eligible"].as_bool().unwrap_or(false) as i64])?;
                Ok(self.conn.last_insert_rowid()) }
            Some(id) => { self.conn.execute("UPDATE pay_items SET name=?2, account_id=?3, liability_account_id=?4, rate_type=?5, multiplier_bp=?6, taxable=?7, super_eligible=?8 WHERE id=?1",
                params![id, name, p["account_id"].as_i64(), p["liability_account_id"].as_i64(), p["rate_type"].as_str().unwrap_or("FIXED"), p["multiplier_bp"].as_i64(), p["taxable"].as_bool().unwrap_or(true) as i64, p["super_eligible"].as_bool().unwrap_or(false) as i64])?;
                Ok(id) }
        }
    }

    pub fn save_pay_calendar(&self, id: Option<i64>, name: &str, frequency: &str, start: &str) -> Result<i64> {
        util::parse_date(start)?;
        if !["WEEKLY", "FORTNIGHTLY", "FOURWEEKLY", "MONTHLY", "QUARTERLY"].contains(&frequency) { return invalid("unknown frequency"); }
        match id {
            None => { self.conn.execute("INSERT INTO pay_calendars(name, frequency, start_date) VALUES (?1, ?2, ?3)", params![name, frequency, start])?; Ok(self.conn.last_insert_rowid()) }
            Some(id) => { self.conn.execute("UPDATE pay_calendars SET name = ?2, frequency = ?3, start_date = ?4 WHERE id = ?1", params![id, name, frequency, start])?; Ok(id) }
        }
    }

    pub fn list_employees(&self, status: &str) -> Result<Vec<Value>> {
        self.ensure_payroll_setup()?;
        util::query_json(&self.conn, "SELECT e.*, c.name AS calendar_name FROM employees e LEFT JOIN pay_calendars c ON c.id = e.pay_calendar_id WHERE (?1 = 'ALL' OR e.status = ?1) ORDER BY e.last_name, e.first_name", [status])
    }

    pub fn get_employee(&self, id: i64) -> Result<Value> {
        let mut e = util::query_one_json(&self.conn, "SELECT e.*, c.name AS calendar_name FROM employees e LEFT JOIN pay_calendars c ON c.id = e.pay_calendar_id WHERE e.id = ?1", [id])?;
        e["template"] = serde_json::from_str(e["template"].as_str().unwrap_or("[]")).unwrap_or(json!([]));
        e["leave"] = json!(util::query_json(&self.conn, "SELECT t.id AS leave_type_id, t.name, COALESCE(b.hours_e2, 0) AS hours_e2 FROM leave_types t LEFT JOIN leave_balances b ON b.leave_type_id = t.id AND b.employee_id = ?1", [id])?);
        e["payslips"] = json!(util::query_json(&self.conn, "SELECT s.id, s.gross, s.tax, s.net, r.period_end, r.payment_date, r.status, r.id AS pay_run_id FROM payslips s JOIN pay_runs r ON r.id = s.pay_run_id WHERE s.employee_id = ?1 ORDER BY r.payment_date DESC", [id])?);
        e["ytd"] = util::query_one_json(&self.conn, "SELECT COALESCE(SUM(s.gross),0) AS gross, COALESCE(SUM(s.tax),0) AS tax, COALESCE(SUM(s.super),0) AS super, COALESCE(SUM(s.net),0) AS net FROM payslips s JOIN pay_runs r ON r.id = s.pay_run_id WHERE s.employee_id = ?1 AND r.status = 'POSTED' AND r.payment_date >= ?2",
            params![id, self.fy_start(&util::today())?])?;
        Ok(e)
    }

    pub fn save_employee(&self, id: Option<i64>, e: &EmployeeInput) -> Result<i64> {
        if e.first_name.trim().is_empty() || e.last_name.trim().is_empty() { return invalid("first and last name are required"); }
        util::parse_date(&e.start_date)?;
        let salary = e.annual_salary.as_deref().and_then(|s| money::parse_fixed(s, 2)).unwrap_or(0);
        let hourly = e.hourly_rate.as_deref().and_then(|s| money::parse_fixed(s, 4)).unwrap_or(0);
        let hours = e.hours_per_week.as_deref().and_then(|s| money::parse_fixed(s, 2)).unwrap_or(3800);
        let sup = e.super_rate.as_deref().filter(|s| !s.trim().is_empty()).and_then(|s| money::parse_fixed(s, 2));
        let basis = if e.pay_basis == "HOURLY" { "HOURLY" } else { "SALARY" };
        let full = format!("{} {}", e.first_name.trim(), e.last_name.trim());
        self.atomic(|| {
            let eid = match id {
                None => {
                    let cid = match self.conn.query_row("SELECT id FROM contacts WHERE name = ?1", [&full], |r| r.get::<_, i64>(0)).optional()? {
                        Some(c) => c, None => self.create_contact(&crate::contacts::ContactInput { name: full.clone(), first_name: e.first_name.clone(), last_name: e.last_name.clone(), email: e.email.clone(), is_employee: Some(true), ..Default::default() })?,
                    };
                    self.conn.execute("UPDATE contacts SET is_employee = 1 WHERE id = ?1", [cid])?;
                    self.conn.execute("INSERT INTO employees(contact_id, first_name, last_name, start_date) VALUES (?1, ?2, ?3, ?4)", params![cid, e.first_name.trim(), e.last_name.trim(), e.start_date])?;
                    self.conn.last_insert_rowid()
                }
                Some(id) => id,
            };
            self.conn.execute("UPDATE employees SET first_name=?2, last_name=?3, email=?4, address=?5, start_date=?6, termination_date=?7, tax_id=?8, tax_code=?9, employment_type=?10,
                pay_calendar_id=?11, pay_basis=?12, annual_salary=?13, hourly_rate_e2=?14, hours_per_week_e2=?15, super_rate_bp=?16, bank_account=?17, template=?18,
                status = CASE WHEN ?7 IS NOT NULL AND ?7 <= date('now','localtime') THEN 'TERMINATED' ELSE 'ACTIVE' END WHERE id=?1",
                params![eid, e.first_name.trim(), e.last_name.trim(), e.email, e.address, e.start_date, e.termination_date.clone().filter(|s| !s.is_empty()), e.tax_id, e.tax_code,
                    if e.employment_type.is_empty() { "FULLTIME" } else { &e.employment_type }, e.pay_calendar_id, basis, salary, hourly, hours, sup, e.bank_account, json!(e.template).to_string()])?;
            self.audit(if id.is_some() { "UPDATE" } else { "CREATE" }, "employee", Some(eid), None, Some(json!({"name": full})))?;
            Ok(eid)
        })
    }

    // ---------------- Timesheets & leave ----------------
    pub fn list_timesheets(&self) -> Result<Vec<Value>> {
        let mut v = util::query_json(&self.conn, "SELECT t.*, e.first_name || ' ' || e.last_name AS employee FROM timesheets t JOIN employees e ON e.id = t.employee_id ORDER BY t.period_start DESC", [])?;
        for t in v.iter_mut() { t["lines"] = serde_json::from_str(t["lines"].as_str().unwrap_or("[]")).unwrap_or(json!([])); }
        Ok(v)
    }
    pub fn save_timesheet(&self, id: Option<i64>, p: &Value) -> Result<i64> {
        let lines = p["lines"].clone();
        let total: i64 = lines.as_array().map(|a| a.iter().map(|l| l["hours"].as_str().and_then(|h| money::parse_fixed(h, 2)).or(l["hours_e2"].as_i64()).unwrap_or(0)).sum()).unwrap_or(0);
        match id {
            None => { self.conn.execute("INSERT INTO timesheets(employee_id, period_start, period_end, lines, total_hours_e2) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![p["employee_id"].as_i64(), p["period_start"].as_str(), p["period_end"].as_str(), lines.to_string(), total])?; Ok(self.conn.last_insert_rowid()) }
            Some(id) => { self.conn.execute("UPDATE timesheets SET lines = ?2, total_hours_e2 = ?3, period_start = ?4, period_end = ?5 WHERE id = ?1 AND status = 'DRAFT'",
                params![id, lines.to_string(), total, p["period_start"].as_str(), p["period_end"].as_str()])?; Ok(id) }
        }
    }
    pub fn set_timesheet_status(&self, id: i64, status: &str) -> Result<()> { self.conn.execute("UPDATE timesheets SET status = ?2 WHERE id = ?1", params![id, status])?; Ok(()) }

    pub fn list_leave(&self) -> Result<Value> {
        Ok(json!({
            "requests": util::query_json(&self.conn, "SELECT r.*, e.first_name || ' ' || e.last_name AS employee, t.name AS leave_type FROM leave_requests r JOIN employees e ON e.id = r.employee_id JOIN leave_types t ON t.id = r.leave_type_id ORDER BY r.start_date DESC", [])?,
            "balances": util::query_json(&self.conn, "SELECT e.id AS employee_id, e.first_name || ' ' || e.last_name AS employee, t.id AS leave_type_id, t.name AS leave_type, COALESCE(b.hours_e2, 0) AS hours_e2
                FROM employees e CROSS JOIN leave_types t LEFT JOIN leave_balances b ON b.employee_id = e.id AND b.leave_type_id = t.id WHERE e.status = 'ACTIVE' ORDER BY employee, t.id", [])?,
        }))
    }
    pub fn save_leave_request(&self, p: &Value) -> Result<i64> {
        let hours = p["hours"].as_str().and_then(|h| money::parse_fixed(h, 2)).ok_or_else(|| Error::Validation("enter the number of hours".into()))?;
        self.conn.execute("INSERT INTO leave_requests(employee_id, leave_type_id, start_date, end_date, hours_e2, description) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![p["employee_id"].as_i64(), p["leave_type_id"].as_i64(), p["start_date"].as_str(), p["end_date"].as_str(), hours, p["description"].as_str().unwrap_or("")])?;
        Ok(self.conn.last_insert_rowid())
    }
    pub fn set_leave_status(&self, id: i64, status: &str) -> Result<()> { self.conn.execute("UPDATE leave_requests SET status = ?2 WHERE id = ?1 AND status <> 'PROCESSED'", params![id, status])?; Ok(()) }

    // ---------------- Pay runs ----------------
    pub fn list_pay_runs(&self) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT r.*, c.name AS calendar_name, (SELECT COUNT(*) FROM payslips s WHERE s.pay_run_id = r.id) AS employees FROM pay_runs r LEFT JOIN pay_calendars c ON c.id = r.pay_calendar_id ORDER BY r.payment_date DESC", [])
    }

    /// Next period for a calendar after its last pay run.
    pub fn next_pay_period(&self, calendar_id: i64) -> Result<(String, String)> {
        let (freq, start): (String, String) = self.conn.query_row("SELECT frequency, start_date FROM pay_calendars WHERE id = ?1", [calendar_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let last: Option<String> = self.conn.query_row("SELECT MAX(period_end) FROM pay_runs WHERE pay_calendar_id = ?1", [calendar_id], |r| r.get(0))?;
        let s = match last { Some(l) => util::parse_date(&util::add_days(&l, 1)?)?, None => util::parse_date(&start)? };
        let e = match freq.as_str() {
            "WEEKLY" => s + chrono::Duration::days(6), "FORTNIGHTLY" => s + chrono::Duration::days(13), "FOURWEEKLY" => s + chrono::Duration::days(27),
            "QUARTERLY" => util::add_months(s, 3) - chrono::Duration::days(1), _ => util::add_months(s, 1) - chrono::Duration::days(1),
        };
        Ok((util::fmt_date(s), util::fmt_date(e)))
    }

    pub fn create_pay_run(&self, calendar_id: i64, payment_date: Option<&str>) -> Result<i64> {
        self.ensure_payroll_setup()?;
        let (from, to) = self.next_pay_period(calendar_id)?;
        let pay_date = payment_date.map(String::from).unwrap_or(to.clone());
        let emps: Vec<i64> = { let mut st = self.conn.prepare("SELECT id FROM employees WHERE pay_calendar_id = ?1 AND status = 'ACTIVE' AND start_date <= ?2 AND (termination_date IS NULL OR termination_date >= ?3)")?;
            st.query_map(params![calendar_id, to, from], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()? };
        if emps.is_empty() { return invalid("no active employees are on this pay calendar"); }
        self.atomic(|| {
            let number = self.next_number("PAYRUN")?;
            self.conn.execute("INSERT INTO pay_runs(number, pay_calendar_id, period_start, period_end, payment_date) VALUES (?1, ?2, ?3, ?4, ?5)", params![number, calendar_id, from, to, pay_date])?;
            let rid = self.conn.last_insert_rowid();
            for e in emps { let lines = self.default_payslip_lines(e, calendar_id, &from, &to)?; self.conn.execute("INSERT INTO payslips(pay_run_id, employee_id, lines) VALUES (?1, ?2, ?3)", params![rid, e, json!(lines).to_string()])?; }
            self.recalc_pay_run(rid)?;
            self.audit("CREATE", "pay_run", Some(rid), None, Some(json!({"period": format!("{from} to {to}")})))?;
            Ok(rid)
        })
    }

    fn default_payslip_lines(&self, emp: i64, calendar_id: i64, from: &str, to: &str) -> Result<Vec<Value>> {
        let freq: String = self.conn.query_row("SELECT frequency FROM pay_calendars WHERE id = ?1", [calendar_id], |r| r.get(0))?;
        let ppy = periods_per_year(&freq);
        let (basis, salary, hourly, hpw, template): (String, i64, i64, i64, String) = self.conn.query_row("SELECT pay_basis, annual_salary, hourly_rate_e2, hours_per_week_e2, template FROM employees WHERE id = ?1", [emp], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
        let ord: (i64, String) = self.conn.query_row("SELECT id, name FROM pay_items WHERE kind = 'EARNINGS' ORDER BY id LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let mut lines = Vec::new();
        let ts_hours: Option<i64> = self.conn.query_row("SELECT SUM(total_hours_e2) FROM timesheets WHERE employee_id = ?1 AND status = 'APPROVED' AND period_start >= ?2 AND period_end <= ?3", params![emp, from, to], |r| r.get(0))?;
        let weeks_e2 = 5200 / ppy; // weeks per period ×100 (52.00 weeks a year)
        if basis == "HOURLY" {
            let units = ts_hours.unwrap_or(hpw * weeks_e2 / 100);
            lines.push(json!({"pay_item_id": ord.0, "name": ord.1, "kind": "EARNINGS", "units_e2": units, "rate_e2": hourly, "amount": money::div_round(units as i128 * hourly as i128, 10_000) as i64}));
        } else {
            let hours = hpw * weeks_e2 / 100;
            lines.push(json!({"pay_item_id": ord.0, "name": ord.1, "kind": "EARNINGS", "units_e2": hours, "rate_e2": null, "amount": per_period(salary, ppy)}));
        }
        let tmpl: Vec<Value> = serde_json::from_str(&template).unwrap_or_default();
        for t in tmpl {
            let Some(pid) = t["pay_item_id"].as_i64() else { continue };
            let (kind, name, rate_type, mult): (String, String, String, Option<i64>) = self.conn.query_row("SELECT kind, name, rate_type, multiplier_bp FROM pay_items WHERE id = ?1", [pid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
            let units = t["units"].as_str().and_then(|u| money::parse_fixed(u, 2)).unwrap_or(0);
            let amount = match rate_type.as_str() {
                "MULTIPLE" => money::div_round(units as i128 * hourly as i128 * mult.unwrap_or(10_000) as i128, 100 * 100 * 10_000) as i64,
                "RATEPERUNIT" => money::div_round(units as i128 * t["rate"].as_str().and_then(|r| money::parse_fixed(r, 4)).unwrap_or(hourly) as i128, 10_000) as i64,
                _ => t["amount"].as_str().and_then(|a| money::parse_fixed(a, 2)).unwrap_or(0),
            };
            lines.push(json!({"pay_item_id": pid, "name": name, "kind": kind, "units_e2": units, "amount": amount}));
        }
        Ok(lines)
    }

    /// Recalculate tax, retirement and net pay for every payslip of a draft pay run.
    pub fn recalc_pay_run(&self, rid: i64) -> Result<()> {
        let (cal, status, from, to): (i64, String, String, String) = self.conn.query_row("SELECT pay_calendar_id, status, period_start, period_end FROM pay_runs WHERE id = ?1", [rid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        if status != "DRAFT" { return invalid("only draft pay runs can be changed"); }
        let freq: String = self.conn.query_row("SELECT frequency FROM pay_calendars WHERE id = ?1", [cal], |r| r.get(0))?;
        let ppy = periods_per_year(&freq);
        let j = self.jurisdiction()?;
        let slips: Vec<(i64, i64, String)> = { let mut st = self.conn.prepare("SELECT id, employee_id, lines FROM payslips WHERE pay_run_id = ?1")?; st.query_map([rid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()? };
        let (mut tw, mut tt, mut ts, mut td, mut tr, mut tn, mut te) = (0, 0, 0, 0, 0, 0, 0);
        for (sid, emp, lines) in slips {
            let lines: Vec<Value> = serde_json::from_str(&lines)?;
            let (tax_code, sup_ovr, etype, hpw): (String, Option<i64>, String, i64) = self.conn.query_row("SELECT tax_code, super_rate_bp, employment_type, hours_per_week_e2 FROM employees WHERE id = ?1", [emp], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
            let (mut gross, mut taxable, mut eligible, mut ded, mut reimb) = (0i64, 0i64, 0i64, 0i64, 0i64);
            let mut kept: Vec<Value> = Vec::new();
            for l in lines {
                if l["kind"] == "TAX" || l["kind"] == "SUPER" || l["auto"].as_bool() == Some(true) { continue; }
                let a = l["amount"].as_i64().unwrap_or(0);
                let (taxable_flag, elig): (i64, i64) = l["pay_item_id"].as_i64().and_then(|p| self.conn.query_row("SELECT taxable, super_eligible FROM pay_items WHERE id = ?1", [p], |r| Ok((r.get(0)?, r.get(1)?))).ok()).unwrap_or((1, 1));
                match l["kind"].as_str().unwrap_or("") {
                    "EARNINGS" | "LEAVE" => { gross += a; if taxable_flag != 0 { taxable += a; } if elig != 0 { eligible += a; } }
                    "DEDUCTION" => ded += a,
                    "REIMBURSEMENT" => reimb += a,
                    _ => {}
                }
                kept.push(l);
            }
            let c = j.calc(taxable, eligible, ppy, &tax_code, sup_ovr);
            if c.employee_retirement != 0 {
                kept.push(json!({"name": format!("{} (employee)", j.retirement_label()), "kind": "DEDUCTION", "amount": c.employee_retirement, "auto": true, "retirement": true}));
                ded += c.employee_retirement;
            }
            kept.push(json!({"name": "Tax withheld", "kind": "TAX", "amount": c.tax, "auto": true}));
            kept.push(json!({"name": format!("{} (employer)", j.retirement_label()), "kind": "SUPER", "amount": c.employer_retirement, "auto": true}));
            if c.employer_tax != 0 { kept.push(json!({"name": "Employer taxes", "kind": "EMPLOYERTAX", "amount": c.employer_tax, "auto": true})); }
            let net = gross - c.tax - ded + reimb;
            // leave accrual for the period
            let mut leave = Vec::new();
            if etype != "CASUAL" && etype != "CONTRACTOR" {
                let mut st = self.conn.prepare("SELECT id, name, accrual_hours_per_year_e2 FROM leave_types WHERE accrual_hours_per_year_e2 > 0")?;
                for r in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)))? {
                    let (lt, name, per_year) = r?;
                    let accrual = money::div_round(per_year as i128 * hpw as i128, 3800 * ppy as i128) as i64;
                    let taken: i64 = self.conn.query_row("SELECT COALESCE(SUM(hours_e2),0) FROM leave_requests WHERE employee_id = ?1 AND leave_type_id = ?2 AND status = 'APPROVED' AND start_date <= ?3 AND end_date >= ?4", params![emp, lt, to, from], |r| r.get(0))?;
                    leave.push(json!({"leave_type_id": lt, "name": name, "accrued_e2": accrual, "taken_e2": taken}));
                }
            }
            self.conn.execute("UPDATE payslips SET gross=?2, tax=?3, super=?4, deductions=?5, reimbursements=?6, net=?7, lines=?8, leave=?9 WHERE id=?1",
                params![sid, gross, c.tax, c.employer_retirement, ded, reimb, net, json!(kept).to_string(), json!(leave).to_string()])?;
            tw += gross; tt += c.tax; ts += c.employer_retirement; td += ded; tr += reimb; tn += net; te += c.employer_tax;
        }
        self.conn.execute("UPDATE pay_runs SET wages=?2, tax=?3, super=?4, deductions=?5, reimbursements=?6, net_pay=?7, employer_tax=?8 WHERE id=?1", params![rid, tw, tt, ts, td, tr, tn, te])?;
        Ok(())
    }

    pub fn get_pay_run(&self, rid: i64) -> Result<Value> {
        let mut r = util::query_one_json(&self.conn, "SELECT r.*, c.name AS calendar_name, c.frequency FROM pay_runs r LEFT JOIN pay_calendars c ON c.id = r.pay_calendar_id WHERE r.id = ?1", [rid])?;
        let mut slips = util::query_json(&self.conn, "SELECT s.*, e.first_name || ' ' || e.last_name AS employee, e.email FROM payslips s JOIN employees e ON e.id = s.employee_id WHERE s.pay_run_id = ?1 ORDER BY employee", [rid])?;
        for s in slips.iter_mut() { for k in ["lines", "leave"] { s[k] = serde_json::from_str(s[k].as_str().unwrap_or("[]")).unwrap_or(json!([])); } }
        r["payslips"] = json!(slips);
        r["disclaimer"] = json!("Verify all calculations before paying employees. Statutory filing requires government developer registration — export file only.");
        Ok(r)
    }

    /// Replace the editable (non-automatic) lines of a draft payslip and recalculate.
    pub fn update_payslip(&self, sid: i64, lines: &[Value]) -> Result<()> {
        let rid: i64 = self.conn.query_row("SELECT pay_run_id FROM payslips WHERE id = ?1", [sid], |r| r.get(0))?;
        let clean: Vec<Value> = lines.iter().filter(|l| l["auto"].as_bool() != Some(true) && !matches!(l["kind"].as_str(), Some("TAX") | Some("SUPER") | Some("EMPLOYERTAX")))
            .map(|l| { let mut l = l.clone(); if let Some(a) = l["amount"].as_str().and_then(|s| money::parse_fixed(s, 2)) { l["amount"] = json!(a); } l }).collect();
        self.conn.execute("UPDATE payslips SET lines = ?2 WHERE id = ?1", params![sid, json!(clean).to_string()])?;
        self.recalc_pay_run(rid)
    }

    pub fn delete_pay_run(&self, rid: i64) -> Result<()> {
        let st: String = self.conn.query_row("SELECT status FROM pay_runs WHERE id = ?1", [rid], |r| r.get(0))?;
        if st != "DRAFT" { return invalid("posted pay runs can't be deleted"); }
        self.conn.execute("DELETE FROM payslips WHERE pay_run_id = ?1", [rid])?;
        self.conn.execute("DELETE FROM pay_runs WHERE id = ?1", [rid])?;
        Ok(())
    }

    /// Post the pay run to the ledger, update leave balances, and optionally pay from a bank account.
    pub fn post_pay_run(&self, rid: i64, bank_account_id: Option<i64>) -> Result<i64> {
        self.recalc_pay_run(rid)?;
        let r = self.get_pay_run(rid)?;
        let date = r["payment_date"].as_str().unwrap().to_string();
        let wages_payable = self.sys_or_create("WAGESPAYABLE", "804", "Wages Payable", "CURRLIAB")?;
        let paye = self.sys_or_create("PAYGWITHHOLDING", "825", "Payroll Tax Withholdings Payable", "CURRLIAB")?;
        let super_liab = self.sys_or_create("SUPERPAYABLE", "826", "Superannuation / Pension Payable", "CURRLIAB")?;
        let super_exp = self.sys_or_create("SUPEREXPENSE", "478", "Superannuation / Pension", "EXPENSE")?;
        let wages_exp = self.sys_or_create("WAGESEXPENSE", "477", "Wages & Salaries", "EXPENSE")?;
        let ded_liab = self.sys_or_create("PAYROLLDEDUCTIONS", "827", "Payroll Deductions Payable", "CURRLIAB")?;
        let mut jl = Vec::new();
        for s in r["payslips"].as_array().unwrap() {
            let emp = s["employee"].as_str().unwrap_or("");
            let contact: Option<i64> = self.conn.query_row("SELECT contact_id FROM employees WHERE id = ?1", [s["employee_id"].as_i64().unwrap_or(0)], |r| r.get(0))?;
            for l in s["lines"].as_array().unwrap() {
                let a = l["amount"].as_i64().unwrap_or(0);
                if a == 0 { continue; }
                let item: Option<(Option<i64>, Option<i64>)> = l["pay_item_id"].as_i64().and_then(|p| self.conn.query_row("SELECT account_id, liability_account_id FROM pay_items WHERE id = ?1", [p], |r| Ok((r.get(0)?, r.get(1)?))).ok());
                let desc = format!("{} — {}", l["name"].as_str().unwrap_or(""), emp);
                match l["kind"].as_str().unwrap_or("") {
                    "EARNINGS" | "LEAVE" => jl.push(JLine::new(item.and_then(|x| x.0).unwrap_or(wages_exp), a).desc(desc).contact(contact)),
                    "REIMBURSEMENT" => jl.push(JLine::new(item.and_then(|x| x.0).unwrap_or(wages_exp), a).desc(desc).contact(contact)),
                    "DEDUCTION" => jl.push(JLine::new(if l["retirement"].as_bool() == Some(true) { super_liab } else { item.and_then(|x| x.1).unwrap_or(ded_liab) }, -a).desc(desc).contact(contact)),
                    "TAX" => jl.push(JLine::new(paye, -a).desc(desc).contact(contact)),
                    "SUPER" => { jl.push(JLine::new(super_exp, a).desc(desc.clone()).contact(contact)); jl.push(JLine::new(super_liab, -a).desc(desc).contact(contact)); }
                    "EMPLOYERTAX" => { jl.push(JLine::new(wages_exp, a).desc(desc.clone()).contact(contact)); jl.push(JLine::new(paye, -a).desc(desc).contact(contact)); }
                    _ => {}
                }
            }
            jl.push(JLine::new(wages_payable, -s["net"].as_i64().unwrap_or(0)).desc(format!("Net pay — {emp}")).contact(contact));
        }
        self.atomic(|| {
            let jid = self.post_journal(NewJournal { date: date.clone(), source_type: "PAYRUN".into(), source_id: Some(rid), reference: r["number"].as_str().unwrap_or("").into(),
                narration: format!("Pay run {} to {}", r["period_start"].as_str().unwrap_or(""), r["period_end"].as_str().unwrap_or("")), lines: jl, cash_basis: true, reverses: None })?;
            self.conn.execute("UPDATE pay_runs SET status = 'POSTED', journal_id = ?2 WHERE id = ?1", params![rid, jid])?;
            for s in r["payslips"].as_array().unwrap() {
                for lv in s["leave"].as_array().unwrap() {
                    let delta = lv["accrued_e2"].as_i64().unwrap_or(0) - lv["taken_e2"].as_i64().unwrap_or(0);
                    self.conn.execute("INSERT INTO leave_balances(employee_id, leave_type_id, hours_e2) VALUES (?1, ?2, ?3) ON CONFLICT(employee_id, leave_type_id) DO UPDATE SET hours_e2 = hours_e2 + excluded.hours_e2",
                        params![s["employee_id"].as_i64(), lv["leave_type_id"].as_i64(), delta])?;
                }
                self.conn.execute("UPDATE leave_requests SET status = 'PROCESSED', payslip_id = ?2 WHERE employee_id = ?1 AND status = 'APPROVED' AND start_date <= ?3 AND end_date >= ?4",
                    params![s["employee_id"].as_i64(), s["id"].as_i64(), r["period_end"].as_str(), r["period_start"].as_str()])?;
            }
            self.conn.execute("UPDATE timesheets SET status = 'PROCESSED' WHERE status = 'APPROVED' AND period_start >= ?1 AND period_end <= ?2", params![r["period_start"].as_str(), r["period_end"].as_str()])?;
            if let Some(bank) = bank_account_id {
                let net = r["net_pay"].as_i64().unwrap_or(0);
                if net > 0 {
                    self.save_bank_transaction(None, &BankTxInput { kind: "SPEND".into(), account_id: bank, date: date.clone(), reference: format!("Net wages {}", r["number"].as_str().unwrap_or("")),
                        contact_name: Some("Employees (net wages)".into()), line_amount_type: Some("NOTAX".into()),
                        lines: vec![LineInput { description: "Net wages".into(), amount: Some(money::fmt_money(net)), account_id: Some(wages_payable), ..Default::default() }], ..Default::default() })?;
                }
            }
            self.audit("POST", "pay_run", Some(rid), None, Some(json!({"net": r["net_pay"]})))?;
            Ok(jid)
        })
    }

    /// Statutory reporting payload (STP / RTI / IR / W-2 style). Export file only.
    pub fn payroll_filing_payload(&self, rid: i64) -> Result<Value> {
        let r = self.get_pay_run(rid)?;
        let j = self.jurisdiction()?;
        let fy = self.fy_start(r["payment_date"].as_str().unwrap_or(&util::today()))?;
        let mut emps = Vec::new();
        for s in r["payslips"].as_array().unwrap() {
            let eid = s["employee_id"].as_i64().unwrap_or(0);
            let ytd = util::query_one_json(&self.conn, "SELECT COALESCE(SUM(s.gross),0) AS gross, COALESCE(SUM(s.tax),0) AS tax, COALESCE(SUM(s.super),0) AS super FROM payslips s JOIN pay_runs r ON r.id = s.pay_run_id
                WHERE s.employee_id = ?1 AND (r.status = 'POSTED' OR r.id = ?2) AND r.payment_date >= ?3 AND r.payment_date <= ?4", params![eid, rid, fy, r["payment_date"].as_str()])?;
            let tax_id: String = self.conn.query_row("SELECT tax_id FROM employees WHERE id = ?1", [eid], |r| r.get(0))?;
            emps.push(json!({"employee": s["employee"], "tax_id": tax_id, "period": {"gross": s["gross"], "tax": s["tax"], "retirement": s["super"]}, "ytd": ytd}));
        }
        Ok(json!({"format": match j.code() { "AU" => "STP Phase 2 (draft structure)", "UK" => "RTI FPS (draft structure)", "NZ" => "Employment information (draft structure)", "US" => "Payroll register", _ => "Payroll register" },
            "employer": {"name": self.setting_or("legal_name", "")?, "tax_number": self.setting_or("tax_number", "")?}, "pay_run": r["number"], "payment_date": r["payment_date"], "employees": emps,
            "notice": "Requires government developer registration — export file only. Not a certified submission."}))
    }

    pub fn payslip_html(&self, sid: i64) -> Result<String> {
        let s = util::query_one_json(&self.conn, "SELECT s.*, e.first_name || ' ' || e.last_name AS employee, e.address, r.period_start, r.period_end, r.payment_date FROM payslips s JOIN employees e ON e.id = s.employee_id JOIN pay_runs r ON r.id = s.pay_run_id WHERE s.id = ?1", [sid])?;
        let lines: Vec<Value> = serde_json::from_str(s["lines"].as_str().unwrap_or("[]"))?;
        let leave: Vec<Value> = serde_json::from_str(s["leave"].as_str().unwrap_or("[]"))?;
        let esc = |x: &str| x.replace('&', "&amp;").replace('<', "&lt;");
        let mut rows = String::new();
        for l in &lines {
            let k = l["kind"].as_str().unwrap_or("");
            let sign = if matches!(k, "TAX" | "DEDUCTION") { "-" } else { "" };
            if k == "SUPER" || k == "EMPLOYERTAX" { continue; }
            rows += &format!("<tr><td>{}</td><td>{}</td><td style='text-align:right'>{sign}{}</td></tr>", esc(l["name"].as_str().unwrap_or("")), k.to_lowercase(), money::fmt_money(l["amount"].as_i64().unwrap_or(0)));
        }
        let lv: String = leave.iter().map(|l| format!("<div>{}: +{} hrs accrued{}</div>", esc(l["name"].as_str().unwrap_or("")), money::fmt_fixed(l["accrued_e2"].as_i64().unwrap_or(0), 2),
            if l["taken_e2"].as_i64().unwrap_or(0) > 0 { format!(", {} hrs taken", money::fmt_fixed(l["taken_e2"].as_i64().unwrap_or(0), 2)) } else { String::new() })).collect();
        Ok(format!(r#"<!doctype html><html><head><meta charset="utf-8"><title>Payslip</title><style>body{{font-family:Segoe UI,system-ui,sans-serif;background:#f3f4f6;margin:0;color:#111}}.p{{max-width:720px;margin:24px auto;background:#fff;padding:40px}}table{{width:100%;border-collapse:collapse;margin-top:16px}}td,th{{padding:8px;border-bottom:1px solid #e5e7eb;text-align:left}}.t{{font-weight:700;font-size:18px}}button{{float:right}}@media print{{button{{display:none}}.p{{margin:0}}}}</style></head><body><div class="p"><button onclick="print()">Print / PDF</button>
<div class="t">PAYSLIP</div><div>{org}</div><hr><div><b>{emp}</b><br>{addr}</div><div>Pay period {ps} to {pe} · paid {pd}</div>
<table><tr><th>Item</th><th>Type</th><th style="text-align:right">Amount</th></tr>{rows}<tr><td><b>Net pay</b></td><td></td><td style="text-align:right"><b>{net}</b></td></tr></table>
<p>Employer {ret}: {sup}</p>{lv}<p style="font-size:11px;color:#666">Generated by LedgerCraft. Verify calculations with a qualified professional.</p></div></body></html>"#,
            org = esc(&self.setting_or("org_name", "")?), emp = esc(s["employee"].as_str().unwrap_or("")), addr = esc(s["address"].as_str().unwrap_or("")).replace('\n', "<br>"),
            ps = s["period_start"].as_str().unwrap_or(""), pe = s["period_end"].as_str().unwrap_or(""), pd = s["payment_date"].as_str().unwrap_or(""),
            net = money::fmt_money(s["net"].as_i64().unwrap_or(0)), ret = self.jurisdiction()?.retirement_label().to_lowercase(), sup = money::fmt_money(s["super"].as_i64().unwrap_or(0))))
    }

    pub fn payroll_activity_report(&self, from: &str, to: &str) -> Result<Value> {
        let rows = util::query_json(&self.conn, "SELECT e.first_name || ' ' || e.last_name AS label, SUM(s.gross) AS gross, SUM(s.tax) AS tax, SUM(s.deductions) AS ded, SUM(s.super) AS sup, SUM(s.reimbursements) AS reimb, SUM(s.net) AS net
            FROM payslips s JOIN pay_runs r ON r.id = s.pay_run_id JOIN employees e ON e.id = s.employee_id WHERE r.status = 'POSTED' AND r.payment_date >= ?1 AND r.payment_date <= ?2 GROUP BY e.id ORDER BY label", params![from, to])?;
        let mut t = [0i64; 6];
        let out: Vec<Value> = rows.iter().map(|r| { let v = [r["gross"].as_i64().unwrap_or(0), r["tax"].as_i64().unwrap_or(0), r["ded"].as_i64().unwrap_or(0), r["sup"].as_i64().unwrap_or(0), r["reimb"].as_i64().unwrap_or(0), r["net"].as_i64().unwrap_or(0)];
            for i in 0..6 { t[i] += v[i]; } json!({"label": r["label"], "values": v}) }).collect();
        Ok(json!({"title": "Payroll Activity Summary", "org": self.setting_or("org_name", "")?, "subtitle": format!("{} to {}", crate::reports::fmt_long(from), crate::reports::fmt_long(to)),
            "columns": ["Gross", "Tax", "Deductions", self.jurisdiction()?.retirement_label(), "Reimbursements", "Net pay"], "sections": [{"title": "", "rows": out, "total": {"label": "Total", "values": t}}]}))
    }

    pub fn leave_balances_report(&self) -> Result<Value> {
        let l = self.list_leave()?;
        let rows: Vec<Value> = l["balances"].as_array().unwrap().iter().map(|b| json!({"label": format!("{} — {}", b["employee"].as_str().unwrap_or(""), b["leave_type"].as_str().unwrap_or("")), "values": [b["hours_e2"]], "format": "hours"})).collect();
        Ok(json!({"title": "Leave Balances", "org": self.setting_or("org_name", "")?, "subtitle": format!("As at {}", crate::reports::fmt_long(&util::today())), "columns": ["Hours"], "qty_cols": [], "hours": true,
            "sections": [{"title": "", "rows": rows}]}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn brackets() {
        assert_eq!(progressive(10_000 * D, &[(18_200 * D, 0), (45_000 * D, 1_600)]), 0);
        assert_eq!(progressive(45_000 * D, &[(18_200 * D, 0), (45_000 * D, 1_600)]), 4_288 * D);
        // AU: $78,000 p.a. fortnightly (3000/fortnight) — tax ≈ (4288 + 0.30*33000 + 2% medicare) / 26
        let c = Au.calc(300_000, 300_000, 26, "", None);
        assert!(c.tax > 50_000 && c.tax < 65_000, "{}", c.tax);
        assert_eq!(c.employer_retirement, 36_000); // 12% SG
        let u = Uk.calc(300_000, 300_000, 12, "1257L", None);
        assert!(u.tax > 0 && u.employee_retirement > 0 && u.employer_tax > 0);
    }
}
