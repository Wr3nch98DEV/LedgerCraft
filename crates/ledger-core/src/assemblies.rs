//! Inventory assemblies (QuickBooks Desktop): an assembly item lists the component items (and
//! quantities) it's made from. Building assemblies takes the components out of stock at their
//! average cost and puts the same cost into the assembly's stock.

use crate::{error::invalid, journal::{JLine, NewJournal}, money, util, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};

impl Ledger {
    /// The components of an assembly, with what's on hand.
    pub fn assembly_components(&self, id: i64) -> Result<Vec<Value>> {
        util::query_json(&self.conn,
            "SELECT c.component_id AS item_id, c.qty_e4, i.code, i.name, i.qty_on_hand_e4, i.total_cost,
               CASE WHEN i.qty_on_hand_e4 <> 0 THEN i.total_cost * 10000 / i.qty_on_hand_e4 ELSE 0 END AS avg_cost
             FROM item_components c JOIN items i ON i.id = c.component_id WHERE c.assembly_id = ?1 ORDER BY i.code", [id])
    }

    /// Replace an assembly's components: (component item id, quantity per assembly in e4).
    pub fn set_assembly_components(&self, id: i64, comps: &[(i64, i64)]) -> Result<()> {
        let (tracked, _, _) = self.item_tracking(id)?;
        if !tracked && !comps.is_empty() { return invalid("an assembly must be a tracked inventory item"); }
        let mut seen = std::collections::HashSet::new();
        for (c, q) in comps {
            if *c == id { return invalid("an assembly can't be one of its own components"); }
            if *q <= 0 { return invalid("each component needs a quantity"); }
            if !seen.insert(*c) { return invalid("each component can only be listed once"); }
            if !self.item_tracking(*c)?.0 { return invalid("components must be tracked inventory items"); }
            if self.assembly_uses(*c, id)? { return invalid("that component is itself built from this assembly"); }
        }
        self.atomic(|| {
            self.conn.execute("DELETE FROM item_components WHERE assembly_id = ?1", [id])?;
            for (c, q) in comps { self.conn.execute("INSERT INTO item_components(assembly_id, component_id, qty_e4) VALUES (?1, ?2, ?3)", params![id, c, q])?; }
            self.audit("UPDATE", "item_components", Some(id), None, Some(json!(comps)))?;
            Ok(())
        })
    }

    /// Whether `item` is built (directly or indirectly) from `target`.
    fn assembly_uses(&self, item: i64, target: i64) -> Result<bool> {
        let mut stack = vec![item];
        let mut seen = std::collections::HashSet::new();
        while let Some(i) = stack.pop() {
            if !seen.insert(i) { continue; }
            let mut st = self.conn.prepare("SELECT component_id FROM item_components WHERE assembly_id = ?1")?;
            for c in st.query_map([i], |r| r.get::<_, i64>(0))? { let c = c?; if c == target { return Ok(true); } stack.push(c); }
        }
        Ok(false)
    }

    /// How many assemblies can be built from what's on hand.
    pub fn max_buildable(&self, id: i64) -> Result<i64> {
        let comps = self.assembly_components(id)?;
        if comps.is_empty() { return Ok(0); }
        Ok(comps.iter().map(|c| {
            let per = c["qty_e4"].as_i64().unwrap_or(1).max(1) as i128;
            ((c["qty_on_hand_e4"].as_i64().unwrap_or(0).max(0) as i128 * 10_000) / per) as i64
        }).min().unwrap_or(0) / 10_000 * 10_000)
    }

    /// Build `qty_e4` assemblies on `date`. Returns the build id.
    pub fn build_assembly(&self, id: i64, qty_e4: i64, date: &str, memo: &str) -> Result<i64> {
        util::parse_date(date)?;
        if qty_e4 <= 0 { return invalid("enter how many to build"); }
        let (tracked, inv, _) = self.item_tracking(id)?;
        if !tracked { return invalid("only tracked inventory assemblies can be built"); }
        let comps = self.assembly_components(id)?;
        if comps.is_empty() { return invalid("add the components this assembly is built from first"); }
        let inv = inv.unwrap_or(self.sys_account("INVENTORY")?);
        self.atomic(|| {
            // Cost of each component taken out (errors if there isn't enough stock).
            let mut outs = vec![];
            for c in &comps {
                let cid = c["item_id"].as_i64().unwrap_or(0);
                let need = money::div_round(qty_e4 as i128 * c["qty_e4"].as_i64().unwrap_or(0) as i128, 10_000) as i64;
                let cost = self.inventory_cost_out(cid, need)?;
                let cinv = self.item_tracking(cid)?.1.unwrap_or(inv);
                outs.push((cid, need, cost, cinv));
            }
            let total: i64 = outs.iter().map(|o| o.2).sum();
            let reference = format!("Build {}", self.conn.query_row("SELECT code FROM items WHERE id = ?1", [id], |r| r.get::<_, String>(0))?);
            let mut lines = vec![JLine::new(inv, total).desc(&reference)];
            for o in &outs { lines.push(JLine::new(o.3, -o.2).desc(&reference)); }
            // Components and assembly usually share one inventory account, so the journal can net to nothing.
            let jid = if lines.iter().any(|l| l.amount != 0) && !(lines.iter().all(|l| l.account_id == inv)) {
                Some(self.post_journal(NewJournal { date: date.into(), source_type: "INVENTORY".into(), source_id: Some(id), reference: reference.clone(), narration: memo.into(), lines, cash_basis: true, reverses: None })?)
            } else { self.check_lock(date)?; None };
            self.conn.execute("INSERT INTO assembly_builds(assembly_id, date, qty_e4, cost, memo, journal_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![id, date, qty_e4, total, memo, jid])?;
            let bid = self.conn.last_insert_rowid();
            for (cid, need, cost, _) in &outs { self.record_movement(*cid, date, -need, -cost, "BUILD", Some(bid), jid)?; }
            self.record_movement(id, date, qty_e4, total, "BUILD", Some(bid), jid)?;
            self.audit("CREATE", "assembly_build", Some(bid), None, Some(json!({"item_id": id, "qty_e4": qty_e4, "cost": total})))?;
            Ok(bid)
        })
    }

    pub fn list_builds(&self, id: Option<i64>) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT b.*, i.code, i.name FROM assembly_builds b JOIN items i ON i.id = b.assembly_id WHERE ?1 IS NULL OR b.assembly_id = ?1 ORDER BY b.date DESC, b.id DESC LIMIT 200", [id])
    }
}
