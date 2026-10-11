use crate::{error::invalid, money::{div_round, line_amount}, util, Ledger, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lat { Exclusive, Inclusive, NoTax }

impl Lat {
    pub fn parse(s: &str) -> Result<Lat> {
        match s {
            "EXCLUSIVE" | "Exclusive" | "" => Ok(Lat::Exclusive),
            "INCLUSIVE" | "Inclusive" => Ok(Lat::Inclusive),
            "NOTAX" | "NoTax" => Ok(Lat::NoTax),
            _ => invalid(format!("unknown line amount type {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component { pub name: String, pub rate_bp: i64, #[serde(default)] pub compound: bool }

/// Tax on a tax-exclusive amount given components (compound components apply to net + prior taxes).
pub fn tax_on_net(net: i64, comps: &[Component]) -> i64 {
    let simple: i64 = comps.iter().filter(|c| !c.compound).map(|c| div_round(net as i128 * c.rate_bp as i128, 10_000) as i64).sum();
    let mut total = simple;
    for c in comps.iter().filter(|c| c.compound) {
        total += div_round((net + simple) as i128 * c.rate_bp as i128, 10_000) as i64;
    }
    total
}

/// Effective rate in bp for a set of components.
pub fn effective_rate(comps: &[Component]) -> i64 {
    let simple: i64 = comps.iter().filter(|c| !c.compound).map(|c| c.rate_bp).sum();
    let mut eff = simple;
    for c in comps.iter().filter(|c| c.compound) {
        eff += div_round((10_000 + simple) as i128 * c.rate_bp as i128, 10_000) as i64;
    }
    eff
}

/// Returns (line_amount, net, tax) for one line. Tax is rounded per line.
pub fn compute_line(lat: Lat, qty_e4: i64, unit_e2: i64, discount_bp: i64, rate_bp: i64, comps: &[Component]) -> (i64, i64, i64) {
    let la = line_amount(qty_e4, unit_e2, discount_bp);
    split_amount(lat, la, rate_bp, comps)
}

/// Split an entered amount into (line_amount, net, tax).
pub fn split_amount(lat: Lat, la: i64, rate_bp: i64, comps: &[Component]) -> (i64, i64, i64) {
    match lat {
        Lat::NoTax => (la, la, 0),
        Lat::Exclusive => {
            let tax = if comps.is_empty() { div_round(la as i128 * rate_bp as i128, 10_000) as i64 } else { tax_on_net(la, comps) };
            (la, la, tax)
        }
        Lat::Inclusive => {
            let eff = if comps.is_empty() { rate_bp } else { effective_rate(comps) };
            let tax = div_round(la as i128 * eff as i128, 10_000 + eff as i128) as i64;
            (la, la - tax, tax)
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaxRateInput {
    pub name: String,
    pub report_type: String,
    #[serde(default)]
    pub components: Vec<Component>,
    #[serde(default)]
    pub rate_bp: Option<i64>,
}

impl Ledger {
    pub fn tax_rate(&self, id: i64) -> Result<(i64, Vec<Component>, String)> {
        let (rate, rt): (i64, String) = self.conn.query_row("SELECT rate_bp, report_type FROM tax_rates WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let mut stmt = self.conn.prepare_cached("SELECT name, rate_bp, compound FROM tax_components WHERE tax_rate_id = ?1 ORDER BY id")?;
        let comps = stmt.query_map([id], |r| Ok(Component { name: r.get(0)?, rate_bp: r.get(1)?, compound: r.get::<_, i64>(2)? != 0 }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok((rate, comps, rt))
    }

    pub fn list_tax_rates(&self) -> Result<Vec<Value>> {
        let mut rates = util::query_json(&self.conn, "SELECT * FROM tax_rates ORDER BY status, name", [])?;
        for r in rates.iter_mut() {
            let id = r["id"].as_i64().unwrap_or(0);
            r["components"] = Value::from(util::query_json(&self.conn, "SELECT name, rate_bp, compound FROM tax_components WHERE tax_rate_id = ?1", [id])?);
        }
        Ok(rates)
    }

    pub fn create_tax_rate(&self, input: &TaxRateInput) -> Result<i64> {
        if input.name.trim().is_empty() { return invalid("tax rate needs a name"); }
        let comps = if input.components.is_empty() {
            vec![Component { name: input.name.clone(), rate_bp: input.rate_bp.unwrap_or(0), compound: false }]
        } else { input.components.clone() };
        let eff = effective_rate(&comps);
        self.atomic(|| {
            self.conn.execute("INSERT INTO tax_rates(name, report_type, rate_bp) VALUES (?1, ?2, ?3)", params![input.name, input.report_type, eff])?;
            let id = self.conn.last_insert_rowid();
            for c in &comps {
                self.conn.execute("INSERT INTO tax_components(tax_rate_id, name, rate_bp, compound) VALUES (?1, ?2, ?3, ?4)", params![id, c.name, c.rate_bp, c.compound as i64])?;
            }
            self.audit("CREATE", "tax_rate", Some(id), None, Some(serde_json::json!({"name": input.name, "rate_bp": eff})))?;
            Ok(id)
        })
    }

    /// Changing a rate's components only affects future transactions (posted journals keep their amounts).
    pub fn update_tax_rate(&self, id: i64, input: &TaxRateInput) -> Result<()> {
        let comps = if input.components.is_empty() {
            vec![Component { name: input.name.clone(), rate_bp: input.rate_bp.unwrap_or(0), compound: false }]
        } else { input.components.clone() };
        let eff = effective_rate(&comps);
        self.atomic(|| {
            self.conn.execute("UPDATE tax_rates SET name = ?2, report_type = ?3, rate_bp = ?4 WHERE id = ?1", params![id, input.name, input.report_type, eff])?;
            self.conn.execute("DELETE FROM tax_components WHERE tax_rate_id = ?1", [id])?;
            for c in &comps {
                self.conn.execute("INSERT INTO tax_components(tax_rate_id, name, rate_bp, compound) VALUES (?1, ?2, ?3, ?4)", params![id, c.name, c.rate_bp, c.compound as i64])?;
            }
            self.audit("UPDATE", "tax_rate", Some(id), None, Some(serde_json::json!({"name": input.name, "rate_bp": eff})))?;
            Ok(())
        })
    }

    pub fn set_tax_rate_status(&self, id: i64, status: &str) -> Result<()> {
        self.conn.execute("UPDATE tax_rates SET status = ?2 WHERE id = ?1 AND system = 0", params![id, status])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusive_inclusive() {
        // $100 ex GST at 10% -> 10 tax
        assert_eq!(split_amount(Lat::Exclusive, 10000, 1000, &[]), (10000, 10000, 1000));
        // $110 inc GST -> 100 net, 10 tax
        assert_eq!(split_amount(Lat::Inclusive, 11000, 1000, &[]), (11000, 10000, 1000));
        // $99.99 inc 15% -> tax 13.04
        assert_eq!(split_amount(Lat::Inclusive, 9999, 1500, &[]), (9999, 8695, 1304));
        assert_eq!(split_amount(Lat::NoTax, 500, 1000, &[]), (500, 500, 0));
    }
    #[test]
    fn compound() {
        // Quebec style: GST 5% + QST 9.975% (non compound) on $100
        let c = vec![Component { name: "GST".into(), rate_bp: 500, compound: false }, Component { name: "QST".into(), rate_bp: 998, compound: false }];
        assert_eq!(tax_on_net(10000, &c), 1498);
        // compound: 5% then 10% on (net + 5%) = 5 + 10.5 = 15.50
        let c2 = vec![Component { name: "A".into(), rate_bp: 500, compound: false }, Component { name: "B".into(), rate_bp: 1000, compound: true }];
        assert_eq!(tax_on_net(10000, &c2), 1550);
        assert_eq!(effective_rate(&c2), 1550);
    }
}
