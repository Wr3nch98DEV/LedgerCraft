//! Bank statement upload: pick a file, recognise its format, guess CSV columns, import.

use serde_json::{Value, json};

/// Guess the CSV column mapping from the first rows (header names, or the data itself).
/// `us_dates` picks month-first dates when the file doesn't make it obvious.
pub fn guess_mapping(rows: &[Vec<String>], us_dates: bool) -> Option<Value> {
    let first = rows.first()?;
    let looks_like_date = |s: &str| { let s = s.trim(); s.len() >= 6 && s.chars().filter(|c| c.is_ascii_digit()).count() >= 4 && (s.contains('/') || s.contains('-') || s.contains('.')) };
    let is_number = |s: &str| { let s = s.trim().replace([',', '$', '£', '€', ' '], ""); !s.is_empty() && s.trim_start_matches(['-', '+', '(']).trim_end_matches(')').parse::<f64>().is_ok() };
    let has_header = !first.iter().any(|c| looks_like_date(c));
    let names: Vec<String> = first.iter().map(|h| h.to_lowercase()).collect();
    let find = |keys: &[&str]| if has_header { names.iter().position(|h| keys.iter().any(|k| h.contains(k))) } else { None };
    let data: Vec<&Vec<String>> = rows.iter().skip(if has_header { 1 } else { 0 }).collect();
    let sample = data.first()?;
    let date_col = find(&["date"]).or_else(|| sample.iter().position(|c| looks_like_date(c)))?;
    let debit_col = find(&["debit", "withdrawal", "money out", "paid out"]);
    let credit_col = find(&["credit", "deposit", "money in", "paid in"]);
    let mut amount_col = find(&["amount", "value"]);
    if amount_col.is_none() && (debit_col.is_none() || credit_col.is_none()) {
        amount_col = sample.iter().enumerate().find(|(n, c)| *n != date_col && is_number(c)).map(|(n, _)| n);
    }
    if amount_col.is_none() && (debit_col.is_none() || credit_col.is_none()) { return None; }
    let payee_col = find(&["payee", "name", "merchant", "counterparty"]);
    let description_col = find(&["description", "details", "memo", "narrative", "particulars", "transaction"])
        .filter(|c| Some(*c) != payee_col && Some(*c) != amount_col)
        .or_else(|| if has_header { None } else { sample.iter().enumerate().find(|(n, c)| *n != date_col && Some(*n) != amount_col && !is_number(c) && !c.trim().is_empty()).map(|(n, _)| n) });
    let reference_col = find(&["reference", "ref", "cheque", "check no"]);
    let balance_col = find(&["balance"]);
    // Day/month order: a first number above 12 settles it; otherwise follow the company's country.
    let mut order = if us_dates { "MDY" } else { "DMY" };
    for r in &data {
        let d = r.get(date_col).map(|s| s.trim()).unwrap_or("");
        if d.len() >= 4 && d[..4].chars().all(|c| c.is_ascii_digit()) { order = "YMD"; break; }
        let parts: Vec<&str> = d.split(['/', '-', '.']).collect();
        if parts.len() == 3 {
            if parts[0].parse::<u32>().map(|x| x > 12).unwrap_or(false) { order = "DMY"; break; }
            if parts[1].parse::<u32>().map(|x| x > 12).unwrap_or(false) { order = "MDY"; break; }
        }
    }
    let split = amount_col.is_none();
    Some(json!({"has_header": has_header, "date_col": date_col, "date_format": order, "amount_col": if split { None } else { amount_col },
        "debit_col": if split { debit_col } else { None }, "credit_col": if split { credit_col } else { None }, "payee_col": payee_col,
        "description_col": description_col, "reference_col": reference_col, "balance_col": balance_col, "negate": false}))
}

/// Import a statement file into a bank account. Returns a summary for the user.
pub fn import_file(be: &crate::backend::Backend, account: i64, name: &str, text: &str, us_dates: bool) -> Result<String, String> {
    let preview = be.call_now("statements.preview", json!({"filename": name, "text": text}))?;
    let format = preview["format"].as_str().unwrap_or("").to_string();
    let mut params = json!({"account_id": account, "filename": name, "text": text, "format": format});
    if format == "CSV" {
        let rows: Vec<Vec<String>> = preview["rows"].as_array().cloned().unwrap_or_default().iter()
            .map(|r| r.as_array().cloned().unwrap_or_default().iter().map(|c| c.as_str().unwrap_or("").to_string()).collect()).collect();
        let mapping = guess_mapping(&rows, us_dates).ok_or("Couldn't work out this CSV's columns (date and amount). Use the web version's import, which lets you map columns by hand.")?;
        params["date_order"] = mapping["date_format"].clone();
        params["mapping"] = mapping;
    }
    let r = be.call_now("statements.import", params)?;
    let added = r["added"].as_i64().unwrap_or(0);
    let dupes = r["duplicates"].as_i64().or(r["skipped"].as_i64()).unwrap_or(0);
    Ok(if dupes > 0 { format!("{added} transaction(s) imported, {dupes} already there and skipped ({format})") } else { format!("{added} transaction(s) imported ({format})") })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rows(s: &str) -> Vec<Vec<String>> { s.lines().map(|l| l.split(',').map(String::from).collect()).collect() }

    #[test]
    fn guesses_common_bank_csvs() {
        let m = guess_mapping(&rows("Date,Description,Amount,Balance\n13/10/2026,COFFEE,-4.50,100.00"), false).unwrap();
        assert_eq!((m["date_col"].as_u64(), m["amount_col"].as_u64(), m["description_col"].as_u64(), m["date_format"].as_str()), (Some(0), Some(2), Some(1), Some("DMY")));
        let m = guess_mapping(&rows("Posting Date,Payee,Debit,Credit\n10/31/2026,ACME,12.00,"), true).unwrap();
        assert_eq!((m["debit_col"].as_u64(), m["credit_col"].as_u64(), m["payee_col"].as_u64(), m["date_format"].as_str()), (Some(2), Some(3), Some(1), Some("MDY")));
        // No header: the date and amount are found from the data.
        let m = guess_mapping(&rows("2026-10-13,-4.50,COFFEE HUT"), false).unwrap();
        assert_eq!((m["has_header"].as_bool(), m["date_col"].as_u64(), m["amount_col"].as_u64(), m["description_col"].as_u64(), m["date_format"].as_str()), (Some(false), Some(0), Some(1), Some(2), Some("YMD")));
        assert!(guess_mapping(&rows("Name,Email\nA,b@c"), false).is_none());
    }

    #[test]
    fn imports_csv_and_skips_duplicates() {
        let dir = std::env::temp_dir().join(format!("lc-import-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let be = crate::backend::Backend::new(dir.clone());
        be.call_now("orgs.create", json!({"org_name": "T", "country": "US", "demo": false, "owner_name": "T"})).unwrap();
        let bank = be.call_now("accounts.bank", json!({})).unwrap()[0]["id"].as_i64().unwrap();
        let csv = "Date,Description,Amount\n10/13/2026,COFFEE HUT,-4.50\n10/14/2026,CLIENT PAYMENT,250.00\n";
        let msg = import_file(&be, bank, "oct.csv", csv, true).unwrap();
        assert!(msg.starts_with("2 transaction"), "{msg}");
        let lines = be.call_now("statements.lines", json!({"account_id": bank})).unwrap();
        assert_eq!(lines.as_array().unwrap().len(), 2);
        assert!(lines.as_array().unwrap().iter().any(|l| l["date"] == "2026-10-13" && l["amount"] == -450));
        let again = import_file(&be, bank, "oct.csv", csv, true).unwrap();
        assert!(again.starts_with("0 transaction"), "{again}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
