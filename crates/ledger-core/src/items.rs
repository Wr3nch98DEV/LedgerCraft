//! Products & services, including tracked inventory (average cost).

use crate::{error::invalid, journal::{JLine, NewJournal}, money, util, Ledger, Result};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ItemInput {
    pub code: String,
    pub name: String,
    pub sales_description: String,
    pub purchase_description: String,
    pub is_sold: Option<bool>,
    pub is_purchased: Option<bool>,
    pub sale_price: Option<String>,
    pub sale_account_id: Option<i64>,
    pub sale_tax_rate_id: Option<i64>,
    pub purchase_price: Option<String>,
    pub purchase_account_id: Option<i64>,
    pub purchase_tax_rate_id: Option<i64>,
    pub is_tracked: bool,
    pub inventory_account_id: Option<i64>,
    pub reorder_level: Option<String>,
}

impl Ledger {
    pub fn list_items(&self, q: &str) -> Result<Vec<Value>> {
        let like = format!("%{}%", q.trim());
        util::query_json(&self.conn,
            "SELECT i.*, CASE WHEN i.qty_on_hand_e4 <> 0 THEN i.total_cost * 10000 / i.qty_on_hand_e4 ELSE 0 END AS avg_cost,
               (i.is_tracked = 1 AND i.reorder_level_e4 IS NOT NULL AND i.qty_on_hand_e4 <= i.reorder_level_e4) AS low_stock
             FROM items i WHERE (?1 = '%%' OR i.code LIKE ?1 OR i.name LIKE ?1) ORDER BY i.status, i.code", [like])
    }

    pub fn get_item(&self, id: i64) -> Result<Value> {
        let mut it = util::query_one_json(&self.conn,
            "SELECT i.*, CASE WHEN i.qty_on_hand_e4 <> 0 THEN i.total_cost * 10000 / i.qty_on_hand_e4 ELSE 0 END AS avg_cost FROM items i WHERE id = ?1", [id])?;
        it["movements"] = json!(util::query_json(&self.conn,
            "SELECT m.date, m.qty_e4, m.cost, m.source_type, m.source_id, m.journal_id FROM inventory_movements m WHERE m.item_id = ?1 ORDER BY m.date DESC, m.id DESC LIMIT 200", [id])
            .unwrap_or_default());
        Ok(it)
    }

    pub fn create_item(&self, i: &ItemInput) -> Result<i64> {
        if i.code.trim().is_empty() { return invalid("item code is required"); }
        if i.is_tracked && i.inventory_account_id.is_none() && self.sys_account("INVENTORY").is_err() { return invalid("tracked items need an inventory asset account"); }
        self.conn.execute("INSERT INTO items(code, name) VALUES (?1, ?2)", params![i.code.trim(), if i.name.trim().is_empty() { i.code.trim() } else { i.name.trim() }])
            .map_err(|_| crate::Error::Validation(format!("item code {} is already used", i.code.trim())))?;
        let id = self.conn.last_insert_rowid();
        self.write_item(id, i, true)?;
        self.audit("CREATE", "item", Some(id), None, Some(json!({"code": i.code})))?;
        Ok(id)
    }

    pub fn update_item(&self, id: i64, i: &ItemInput) -> Result<()> {
        let was_tracked: i64 = self.conn.query_row("SELECT is_tracked FROM items WHERE id = ?1", [id], |r| r.get(0))?;
        let moves: i64 = self.conn.query_row("SELECT COUNT(*) FROM inventory_movements WHERE item_id = ?1", [id], |r| r.get(0))?;
        if (was_tracked != 0) != i.is_tracked && moves > 0 { return invalid("cannot change tracking for an item with stock movements"); }
        self.write_item(id, i, false)?;
        self.audit("UPDATE", "item", Some(id), None, Some(json!({"code": i.code})))?;
        Ok(())
    }

    fn write_item(&self, id: i64, i: &ItemInput, _new: bool) -> Result<()> {
        let sp = i.sale_price.as_deref().filter(|s| !s.trim().is_empty()).map(|s| money::parse_fixed(s, 4)).flatten();
        let pp = i.purchase_price.as_deref().filter(|s| !s.trim().is_empty()).map(|s| money::parse_fixed(s, 4)).flatten();
        let rl = i.reorder_level.as_deref().filter(|s| !s.trim().is_empty()).map(|s| money::parse_fixed(s, 4)).flatten();
        let inv = if i.is_tracked { Some(i.inventory_account_id.map(Ok).unwrap_or_else(|| self.sys_account("INVENTORY"))?) } else { None };
        self.conn.execute(
            "UPDATE items SET code=?2, name=?3, sales_description=?4, purchase_description=?5, is_sold=?6, is_purchased=?7, sale_unit_e2=?8,
               sale_account_id=?9, sale_tax_rate_id=?10, purchase_unit_e2=?11, purchase_account_id=?12, purchase_tax_rate_id=?13,
               is_tracked=?14, inventory_account_id=?15, reorder_level_e4=?16 WHERE id=?1",
            params![id, i.code.trim(), if i.name.trim().is_empty() { i.code.trim() } else { i.name.trim() }, i.sales_description, i.purchase_description,
                i.is_sold.unwrap_or(true) as i64, i.is_purchased.unwrap_or(true) as i64, sp, i.sale_account_id, i.sale_tax_rate_id, pp,
                i.purchase_account_id, i.purchase_tax_rate_id, i.is_tracked as i64, inv, rl],
        )?;
        self.index_search("item", id, &format!("{} {}", i.code, i.name), &i.sales_description)?;
        Ok(())
    }

    pub fn set_item_status(&self, id: i64, status: &str) -> Result<()> {
        self.conn.execute("UPDATE items SET status = ?2 WHERE id = ?1", params![id, status])?;
        Ok(())
    }

    pub fn delete_item(&self, id: i64) -> Result<()> {
        let used: i64 = self.conn.query_row("SELECT (SELECT COUNT(*) FROM line_items WHERE item_id = ?1) + (SELECT COUNT(*) FROM inventory_movements WHERE item_id = ?1)", [id], |r| r.get(0))?;
        if used > 0 { return invalid("this item has been used; archive it instead"); }
        self.conn.execute("DELETE FROM items WHERE id = ?1", [id])?;
        self.unindex_search("item", id)?;
        Ok(())
    }

    /// (is_tracked, inventory_account, cogs_account)
    pub fn item_tracking(&self, id: i64) -> Result<(bool, Option<i64>, Option<i64>)> {
        let (t, inv, cogs): (i64, Option<i64>, Option<i64>) = self.conn.query_row(
            "SELECT is_tracked, inventory_account_id, purchase_account_id FROM items WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok((t != 0, inv, cogs))
    }

    /// Cost (base) of removing `qty_e4` at current average cost. Errors if insufficient stock.
    pub fn inventory_cost_out(&self, item_id: i64, qty_e4: i64) -> Result<i64> {
        let (qoh, cost, code): (i64, i64, String) = self.conn.query_row("SELECT qty_on_hand_e4, total_cost, code FROM items WHERE id = ?1", [item_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        if qty_e4 > qoh { return invalid(format!("not enough stock of {code}: {} on hand", money::fmt_fixed(qoh, 4).trim_end_matches('0').trim_end_matches('.'))); }
        if qty_e4 == qoh { return Ok(cost); }
        if self.setting_or("inventory_costing", "AVERAGE")? == "FIFO" { return self.fifo_cost_out(item_id, qty_e4); }
        Ok(money::div_round(cost as i128 * qty_e4 as i128, qoh as i128) as i64)
    }

    /// First in, first out: the cost of the next `qty_e4` units after what has already gone out,
    /// taken from the stock layers in the order they came in. (Moves between locations don't
    /// change the order.)
    fn fifo_cost_out(&self, item_id: i64, qty_e4: i64) -> Result<i64> {
        let moves: Vec<(i64, i64)> = {
            let mut st = self.conn.prepare("SELECT qty_e4, cost FROM inventory_movements WHERE item_id = ?1 AND source_type <> 'TRANSFER' ORDER BY date, id")?;
            st.query_map([item_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let consumed: i64 = moves.iter().filter(|m| m.0 < 0).map(|m| -m.0).sum();
        let (mut skip, mut need, mut cost) = (consumed, qty_e4, 0i64);
        for (q, c) in moves.into_iter().filter(|m| m.0 > 0) {
            let at = |n: i64| money::div_round(c as i128 * n as i128, q as i128) as i64;
            let used = skip.min(q);
            skip -= used;
            let avail = q - used;
            if avail == 0 { continue; }
            let take = need.min(avail);
            // Cost of units [used, used+take) of this layer, without rounding drift.
            cost += at(used + take) - at(used);
            need -= take;
            if need == 0 { break; }
        }
        if need > 0 {
            // Shouldn't happen (quantity on hand was checked); fall back to the average for the rest.
            let (qoh, total): (i64, i64) = self.conn.query_row("SELECT qty_on_hand_e4, total_cost FROM items WHERE id = ?1", [item_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
            if qoh > 0 { cost += money::div_round(total as i128 * need as i128, qoh as i128) as i64; }
        }
        Ok(cost)
    }

    pub fn record_movement(&self, item_id: i64, date: &str, qty_e4: i64, cost: i64, source_type: &str, source_id: Option<i64>, journal_id: Option<i64>) -> Result<()> {
        self.record_movement_at(item_id, date, qty_e4, cost, source_type, source_id, journal_id, None)
    }

    /// A stock movement at a location (None = the main location).
    #[allow(clippy::too_many_arguments)]
    pub fn record_movement_at(&self, item_id: i64, date: &str, qty_e4: i64, cost: i64, source_type: &str, source_id: Option<i64>, journal_id: Option<i64>, location: Option<i64>) -> Result<()> {
        self.conn.execute("INSERT INTO inventory_movements(item_id, date, qty_e4, cost, source_type, source_id, journal_id, location_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![item_id, date, qty_e4, cost, source_type, source_id, journal_id, location])?;
        self.conn.execute("UPDATE items SET qty_on_hand_e4 = qty_on_hand_e4 + ?2, total_cost = total_cost + ?3 WHERE id = ?1", params![item_id, qty_e4, cost])?;
        Ok(())
    }

    /// Inventory adjustment (stocktake / write-off). Positive qty adds stock at `unit_cost`.
    pub fn adjust_inventory(&self, item_id: i64, date: &str, qty_e4: i64, unit_cost_e2: Option<i64>, adjustment_account_id: i64, reason: &str) -> Result<i64> {
        let (tracked, inv, _) = self.item_tracking(item_id)?;
        if !tracked { return invalid("only tracked items can be adjusted"); }
        let inv = inv.unwrap_or(self.sys_account("INVENTORY")?);
        let cost = if qty_e4 < 0 { -self.inventory_cost_out(item_id, -qty_e4)? } else {
            let u = unit_cost_e2.ok_or_else(|| crate::Error::Validation("unit cost required when adding stock".into()))?;
            money::div_round(qty_e4 as i128 * u as i128, 1_000_000) as i64
        };
        self.atomic(|| {
            let jid = self.post_journal(NewJournal {
                date: date.into(), source_type: "INVENTORY".into(), source_id: Some(item_id), reference: "Inventory adjustment".into(), narration: reason.into(),
                lines: vec![JLine::new(inv, cost).desc(reason), JLine::new(adjustment_account_id, -cost).desc(reason)], cash_basis: true, reverses: None,
            })?;
            self.record_movement(item_id, date, qty_e4, cost, "ADJUSTMENT", None, Some(jid))?;
            Ok(jid)
        })
    }

    /// Stock valuation per item vs. the general ledger inventory accounts.
    pub fn inventory_valuation(&self) -> Result<Value> {
        let items = util::query_json(&self.conn,
            "SELECT id, code, name, qty_on_hand_e4, total_cost, CASE WHEN qty_on_hand_e4 <> 0 THEN total_cost * 10000 / qty_on_hand_e4 ELSE 0 END AS avg_cost, reorder_level_e4,
              (reorder_level_e4 IS NOT NULL AND qty_on_hand_e4 <= reorder_level_e4) AS low_stock
             FROM items WHERE is_tracked = 1 ORDER BY code", [])?;
        let total: i64 = items.iter().map(|i| i["total_cost"].as_i64().unwrap_or(0)).sum();
        let gl: i64 = self.conn.query_row("SELECT COALESCE(SUM(jl.amount),0) FROM journal_lines jl JOIN journals j ON j.id=jl.journal_id JOIN accounts a ON a.id = jl.account_id
            WHERE j.posted = 1 AND a.id IN (SELECT DISTINCT inventory_account_id FROM items WHERE is_tracked = 1)", [], |r| r.get(0))?;
        Ok(json!({"items": items, "total": total, "gl_balance": gl}))
    }
}
