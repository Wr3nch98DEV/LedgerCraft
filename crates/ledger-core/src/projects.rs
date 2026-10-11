//! Projects: estimates, tasks, time tracking (timer + manual), costs, invoicing and profitability.

use crate::{documents::{DocInput, LineInput}, error::invalid, money, util, Error, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};

impl Ledger {
    pub fn list_projects(&self, status: &str) -> Result<Vec<Value>> {
        let mut v = util::query_json(&self.conn, "SELECT p.*, c.name AS contact_name FROM projects p LEFT JOIN contacts c ON c.id = p.contact_id WHERE (?1 = 'ALL' OR p.status = ?1) ORDER BY p.status, p.name", [status])?;
        for p in v.iter_mut() { let s = self.project_summary(p["id"].as_i64().unwrap())?; p["summary"] = s; }
        Ok(v)
    }

    pub fn project_summary(&self, id: i64) -> Result<Value> {
        let minutes: i64 = self.conn.query_row("SELECT COALESCE(SUM(minutes),0) FROM time_entries WHERE project_id = ?1", [id], |r| r.get(0))?;
        let unbilled_time: i64 = self.conn.query_row("SELECT COALESCE(SUM(t.minutes * k.rate_e2 / 60 / 100),0) FROM time_entries t JOIN project_tasks k ON k.id = t.task_id WHERE t.project_id = ?1 AND t.invoiced_document_id IS NULL AND k.charge_type = 'TIME'", [id], |r| r.get(0))?;
        let fixed_unbilled: i64 = self.conn.query_row("SELECT COALESCE(SUM(rate_e2 / 100),0) FROM project_tasks k WHERE k.project_id = ?1 AND k.charge_type = 'FIXED' AND k.status = 'ACTIVE'", [id], |r| r.get(0))?;
        let costs: i64 = self.conn.query_row("SELECT COALESCE(SUM(amount),0) FROM project_costs WHERE project_id = ?1", [id], |r| r.get(0))?;
        let unbilled_costs: i64 = self.conn.query_row("SELECT COALESCE(SUM(amount + amount * markup_bp / 10000),0) FROM project_costs WHERE project_id = ?1 AND chargeable = 1 AND invoiced_document_id IS NULL", [id], |r| r.get(0))?;
        let invoiced: i64 = self.conn.query_row("SELECT COALESCE(SUM(base_total * subtotal / MAX(total,1)),0) FROM documents WHERE project_id = ?1 AND type = 'ACCREC' AND status IN ('AUTHORISED','PAID')", [id], |r| r.get(0))?;
        let estimate: i64 = self.conn.query_row("SELECT estimate FROM projects WHERE id = ?1", [id], |r| r.get(0))?;
        Ok(json!({"minutes": minutes, "unbilled_time": unbilled_time, "unbilled_fixed": fixed_unbilled, "costs": costs, "unbilled_costs": unbilled_costs, "invoiced": invoiced,
            "profit": invoiced - costs, "estimate": estimate, "to_invoice": unbilled_time + fixed_unbilled + unbilled_costs}))
    }

    pub fn get_project(&self, id: i64) -> Result<Value> {
        let mut p = util::query_one_json(&self.conn, "SELECT p.*, c.name AS contact_name FROM projects p LEFT JOIN contacts c ON c.id = p.contact_id WHERE p.id = ?1", [id])?;
        p["tasks"] = json!(util::query_json(&self.conn, "SELECT k.*, COALESCE((SELECT SUM(minutes) FROM time_entries t WHERE t.task_id = k.id),0) AS minutes FROM project_tasks k WHERE k.project_id = ?1 ORDER BY k.id", [id])?);
        p["time"] = json!(util::query_json(&self.conn, "SELECT t.*, k.name AS task, u.name AS user FROM time_entries t JOIN project_tasks k ON k.id = t.task_id LEFT JOIN users u ON u.id = t.user_id WHERE t.project_id = ?1 ORDER BY t.date DESC, t.id DESC", [id])?);
        p["costs"] = json!(util::query_json(&self.conn, "SELECT * FROM project_costs WHERE project_id = ?1 ORDER BY date DESC", [id])?);
        p["invoices"] = json!(util::query_json(&self.conn, "SELECT id, number, date, total, status FROM documents WHERE project_id = ?1 ORDER BY date DESC", [id])?);
        p["summary"] = self.project_summary(id)?;
        Ok(p)
    }

    pub fn save_project(&self, id: Option<i64>, p: &Value) -> Result<i64> {
        let name = p["name"].as_str().unwrap_or("").trim();
        if name.is_empty() { return invalid("project name is required"); }
        let estimate = p["estimate"].as_str().and_then(|s| money::parse_fixed(s, 2)).unwrap_or(0);
        match id {
            None => { self.conn.execute("INSERT INTO projects(name, contact_id, deadline, estimate) VALUES (?1, ?2, ?3, ?4)", params![name, p["contact_id"].as_i64(), p["deadline"].as_str().filter(|s| !s.is_empty()), estimate])?;
                let pid = self.conn.last_insert_rowid();
                self.index_search("project", pid, name, "")?;
                Ok(pid) }
            Some(id) => { self.conn.execute("UPDATE projects SET name = ?2, contact_id = ?3, deadline = ?4, estimate = ?5, status = COALESCE(?6, status) WHERE id = ?1",
                params![id, name, p["contact_id"].as_i64(), p["deadline"].as_str().filter(|s| !s.is_empty()), estimate, p["status"].as_str()])?; Ok(id) }
        }
    }

    pub fn save_task(&self, id: Option<i64>, project: i64, p: &Value) -> Result<i64> {
        let name = p["name"].as_str().unwrap_or("").trim();
        if name.is_empty() { return invalid("task name is required"); }
        let rate = p["rate"].as_str().and_then(|s| money::parse_fixed(s, 4)).unwrap_or(0);
        let ct = p["charge_type"].as_str().unwrap_or("TIME");
        match id {
            None => { self.conn.execute("INSERT INTO project_tasks(project_id, name, charge_type, rate_e2, estimate_minutes) VALUES (?1, ?2, ?3, ?4, ?5)", params![project, name, ct, rate, p["estimate_hours"].as_str().and_then(|h| money::parse_fixed(h, 2)).map(|h| h * 60 / 100)])?; Ok(self.conn.last_insert_rowid()) }
            Some(id) => { self.conn.execute("UPDATE project_tasks SET name = ?2, charge_type = ?3, rate_e2 = ?4 WHERE id = ?1", params![id, name, ct, rate])?; Ok(id) }
        }
    }

    pub fn add_time(&self, project: i64, task: i64, date: &str, minutes: i64, description: &str) -> Result<i64> {
        util::parse_date(date)?;
        if minutes <= 0 { return invalid("enter the time spent"); }
        self.conn.execute("INSERT INTO time_entries(project_id, task_id, user_id, date, minutes, description) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![project, task, self.user_id, date, minutes, description])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn start_timer(&self, project: i64, task: i64, description: &str) -> Result<i64> {
        let running: i64 = self.conn.query_row("SELECT COUNT(*) FROM time_entries WHERE running_since IS NOT NULL AND user_id IS ?1", [self.user_id], |r| r.get(0))?;
        if running > 0 { return invalid("a timer is already running; stop it first"); }
        self.conn.execute("INSERT INTO time_entries(project_id, task_id, user_id, date, minutes, description, running_since) VALUES (?1, ?2, ?3, date('now','localtime'), 0, ?4, datetime('now'))", params![project, task, self.user_id, description])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn stop_timer(&self) -> Result<Value> {
        let row: (i64, i64) = self.conn.query_row("SELECT id, CAST((julianday('now') - julianday(running_since)) * 1440 AS INTEGER) FROM time_entries WHERE running_since IS NOT NULL AND user_id IS ?1", [self.user_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|_| Error::Validation("no timer is running".into()))?;
        let mins = row.1.max(1);
        self.conn.execute("UPDATE time_entries SET minutes = ?2, running_since = NULL WHERE id = ?1", params![row.0, mins])?;
        Ok(json!({"id": row.0, "minutes": mins}))
    }

    pub fn running_timer(&self) -> Result<Value> {
        Ok(util::query_json(&self.conn, "SELECT t.*, p.name AS project, k.name AS task FROM time_entries t JOIN projects p ON p.id = t.project_id JOIN project_tasks k ON k.id = t.task_id WHERE t.running_since IS NOT NULL AND t.user_id IS ?1", [self.user_id])?
            .into_iter().next().unwrap_or(Value::Null))
    }

    pub fn delete_time(&self, id: i64) -> Result<()> {
        let inv: Option<i64> = self.conn.query_row("SELECT invoiced_document_id FROM time_entries WHERE id = ?1", [id], |r| r.get(0))?;
        if inv.is_some() { return invalid("this time has been invoiced"); }
        self.conn.execute("DELETE FROM time_entries WHERE id = ?1", [id])?; Ok(())
    }

    pub fn add_project_cost(&self, project: i64, p: &Value) -> Result<i64> {
        let amount = p["amount"].as_str().and_then(|s| money::parse_fixed(s, 2)).ok_or_else(|| Error::Validation("enter an amount".into()))?;
        self.conn.execute("INSERT INTO project_costs(project_id, date, description, amount, chargeable, markup_bp, source_kind) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'MANUAL')",
            params![project, p["date"].as_str().unwrap_or(&util::today()), p["description"].as_str().unwrap_or(""), amount, p["chargeable"].as_bool().unwrap_or(true) as i64,
                p["markup"].as_str().and_then(|s| money::parse_fixed(s, 2)).unwrap_or(0)])?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Create a draft invoice from unbilled time, fixed-price tasks and chargeable costs.
    pub fn invoice_project(&self, id: i64) -> Result<i64> {
        let p = self.get_project(id)?;
        let contact = p["contact_id"].as_i64().ok_or_else(|| Error::Validation("set a contact on the project first".into()))?;
        let sales_acct = self.account_by_code("200")?;
        let sales_tax: Option<i64> = self.conn.query_row("SELECT id FROM tax_rates WHERE report_type = 'OUTPUT' AND status = 'ACTIVE' ORDER BY id LIMIT 1", [], |r| r.get(0)).ok();
        let mut lines = Vec::new();
        let mut time_ids = Vec::new();
        for t in p["tasks"].as_array().unwrap() {
            let tid = t["id"].as_i64().unwrap();
            match t["charge_type"].as_str().unwrap_or("") {
                "TIME" => {
                    let (mins, ids): (i64, String) = self.conn.query_row("SELECT COALESCE(SUM(minutes),0), COALESCE(GROUP_CONCAT(id),'') FROM time_entries WHERE task_id = ?1 AND invoiced_document_id IS NULL AND running_since IS NULL", [tid], |r| Ok((r.get(0)?, r.get(1)?)))?;
                    if mins == 0 { continue; }
                    time_ids.extend(ids.split(',').filter_map(|x| x.parse::<i64>().ok()));
                    lines.push(LineInput { description: format!("{} — {}", p["name"].as_str().unwrap_or(""), t["name"].as_str().unwrap_or("")), quantity: Some(money::fmt_fixed(mins * 10_000 / 60, 4)),
                        unit_price: Some(money::fmt_fixed(t["rate_e2"].as_i64().unwrap_or(0), 4)), account_id: sales_acct, tax_rate_id: sales_tax, ..Default::default() });
                }
                "FIXED" => {
                    if t["status"] != "ACTIVE" { continue; }
                    lines.push(LineInput { description: format!("{} — {} (fixed price)", p["name"].as_str().unwrap_or(""), t["name"].as_str().unwrap_or("")), quantity: Some("1".into()),
                        unit_price: Some(money::fmt_fixed(t["rate_e2"].as_i64().unwrap_or(0), 4)), account_id: sales_acct, tax_rate_id: sales_tax, ..Default::default() });
                    self.conn.execute("UPDATE project_tasks SET status = 'INVOICED' WHERE id = ?1", [tid])?;
                }
                _ => {}
            }
        }
        let costs = util::query_json(&self.conn, "SELECT * FROM project_costs WHERE project_id = ?1 AND chargeable = 1 AND invoiced_document_id IS NULL", [id])?;
        for c in &costs {
            let amt = c["amount"].as_i64().unwrap_or(0);
            let total = amt + money::div_round(amt as i128 * c["markup_bp"].as_i64().unwrap_or(0) as i128, 10_000) as i64;
            lines.push(LineInput { description: c["description"].as_str().unwrap_or("Expense").into(), quantity: Some("1".into()), unit_price: Some(money::fmt_money(total)), account_id: sales_acct, tax_rate_id: sales_tax, ..Default::default() });
        }
        if lines.is_empty() { return invalid("there's nothing unbilled on this project"); }
        self.atomic(|| {
            let did = self.save_document(None, &DocInput { kind: "ACCREC".into(), contact_id: Some(contact), date: util::today(), reference: p["name"].as_str().unwrap_or("").into(), lines: lines.clone(), ..Default::default() })?;
            self.conn.execute("UPDATE documents SET project_id = ?2 WHERE id = ?1", params![did, id])?;
            for t in &time_ids { self.conn.execute("UPDATE time_entries SET invoiced_document_id = ?2 WHERE id = ?1", params![t, did])?; }
            for c in &costs { self.conn.execute("UPDATE project_costs SET invoiced_document_id = ?2 WHERE id = ?1", params![c["id"].as_i64(), did])?; }
            Ok(did)
        })
    }

    pub fn project_profitability_report(&self) -> Result<Value> {
        let rows: Vec<Value> = self.list_projects("ALL")?.into_iter().map(|p| { let s = &p["summary"];
            json!({"label": p["name"], "values": [s["estimate"], s["invoiced"], s["costs"], s["profit"], s["to_invoice"]]}) }).collect();
        Ok(json!({"title": "Project Profitability", "org": self.setting_or("org_name", "")?, "subtitle": "All projects", "columns": ["Estimate", "Invoiced", "Costs", "Profit", "To invoice"], "sections": [{"title": "", "rows": rows}]}))
    }
}
