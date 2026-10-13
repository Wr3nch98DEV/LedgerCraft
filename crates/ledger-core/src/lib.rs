//! ledger-core: the LedgerCraft accounting engine. No UI dependencies.
//!
//! The central type is [`Ledger`], a handle on one organisation's SQLite file.
//! Every write that touches the general ledger goes through [`Ledger::post_journal`],
//! which (together with SQLite triggers in the schema) guarantees the ledger is
//! balanced, immutable once posted, and respects lock dates.

pub mod money;
pub mod error;
pub mod db;
pub mod seed;
pub mod journal;
pub mod settings;
pub mod users;
pub mod audit;
pub mod accounts;
pub mod tax;
pub mod contacts;
pub mod items;
pub mod locations;
pub mod lots;
pub mod revrec;
pub mod customfields;
pub mod landed;
pub mod consolidation;
pub mod deposits;
pub mod agencies;
pub mod pricelevels;
pub mod assemblies;
pub mod us1099;
pub mod delayed;
pub mod documents;
pub mod payments;
pub mod banking;
pub mod statements;
pub mod tickoff;
pub mod reports;
pub mod search;
pub mod util;
pub mod dashboard;
pub mod advanced;
pub mod ext;
pub mod modules;
pub mod email;
pub mod extra_reports;
pub mod demo;
pub mod taxreturns;
pub mod assets;
pub mod payroll;
pub mod expenses;
pub mod projects;
pub mod importexport;
pub mod adviser;
pub mod customreports;

pub use error::{Error, Result};
pub use money::Cents;
pub use rusqlite;

use rusqlite::Connection;
use std::cell::Cell;

pub struct Ledger {
    pub conn: Connection,
    pub user_id: Option<i64>,
    pub role: String,
    depth: Cell<u32>,
}

impl Ledger {
    /// Open (creating and migrating if needed) an organisation file.
    pub fn open(path: &std::path::Path) -> Result<Ledger> {
        let conn = Connection::open(path)?;
        Self::from_conn(conn)
    }

    pub fn open_in_memory() -> Result<Ledger> {
        Self::from_conn(Connection::open_in_memory()?)
    }

    fn from_conn(conn: Connection) -> Result<Ledger> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000;",
        )?;
        db::migrate(&conn)?;
        Ok(Ledger { conn, user_id: None, role: "ADVISER".into(), depth: Cell::new(0) })
    }

    /// Run `f` atomically. Nests safely using savepoints.
    pub fn atomic<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        let d = self.depth.get();
        let sp = format!("sp{d}");
        self.conn.execute_batch(&format!("SAVEPOINT {sp}"))?;
        self.depth.set(d + 1);
        let r = f();
        self.depth.set(d);
        match r {
            Ok(v) => {
                self.conn.execute_batch(&format!("RELEASE {sp}"))?;
                Ok(v)
            }
            Err(e) => {
                let _ = self.conn.execute_batch(&format!("ROLLBACK TO {sp}; RELEASE {sp}"));
                Err(e)
            }
        }
    }
}
