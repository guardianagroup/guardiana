//! «Audita a GUARDIANA» (1.0.46): what this program keeps on disk, measured from the file
//! itself rather than told. Every table of the ledger, its columns, how many rows and how many
//! bytes, and, column by column, how many stored values are an internet address: the page
//! promises «no IPs», and this is the check of that promise, not a copy of it.
//!
//! A value counts as an address only when the whole value is one (`192.168.1.20`, `::1`). The
//! columns that hold one on purpose are the address of a device of the house on its own network
//! (`events.client_ip`, `devices.last_ip`): local addresses, said as such. A public address in
//! any column is reported as one, wherever it is.
//!
//! Also here: the connections another copy of GUARDIANA (GUARDIANA ZERO, in the person's own
//! folder) wrote down in its own file, read without changing that file.

use std::net::IpAddr;
use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::ledger::Ledger;
use crate::model::{Outbound, Purpose};

/// One column of one table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnaInventario {
    /// Its name.
    pub nombre: String,
    /// The type it was declared with (`TEXT`, `INTEGER`, `BLOB`).
    pub tipo: String,
    /// Stored values whose whole value is a local address (loopback, private network,
    /// link-local): a device of the house on its own network.
    pub direcciones_locales: u64,
    /// Stored values whose whole value is a public internet address.
    pub direcciones_publicas: u64,
}

/// One table of the ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TablaInventario {
    /// Its name.
    pub nombre: String,
    /// Rows it holds now.
    pub filas: u64,
    /// Bytes its pages take in the file, when SQLite can say (`dbstat`); `None` otherwise.
    pub bytes: Option<u64>,
    /// Its columns, in order.
    pub columnas: Vec<ColumnaInventario>,
}

/// What the ledger file holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Inventario {
    /// Every table, by name.
    pub tablas: Vec<TablaInventario>,
    /// Bytes of the whole database (pages × page size).
    pub bytes_base: u64,
    /// Values that are a public address, all columns together. The promise is that this is 0.
    pub direcciones_publicas: u64,
    /// Values that are a local address, all columns together.
    pub direcciones_locales: u64,
}

/// Whether an address is of a local network (or this computer), not of the internet.
#[must_use]
pub fn es_direccion_local(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            v.is_loopback()
                || v.is_private()
                || v.is_link_local()
                || v.is_unspecified()
                || v.is_broadcast()
                // Shared address space of carriers (RFC 6598), never reachable from the internet.
                || (o[0] == 100 && (64..128).contains(&o[1]))
        }
        IpAddr::V6(v) => {
            let s = v.segments();
            v.is_loopback()
                || v.is_unspecified()
                || (s[0] & 0xfe00) == 0xfc00
                || (s[0] & 0xffc0) == 0xfe80
                || v.to_ipv4_mapped()
                    .is_some_and(|m| es_direccion_local(&IpAddr::V4(m)))
        }
    }
}

/// A name SQLite will take between double quotes without surprises.
fn nombre_seguro(n: &str) -> bool {
    !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn cuenta_direcciones(conn: &Connection, tabla: &str, columna: &str) -> (u64, u64) {
    // Only values shaped like an address get to Rust; most columns have none at all.
    let sql = format!(
        "SELECT \"{columna}\", COUNT(*) FROM \"{tabla}\" WHERE typeof(\"{columna}\") = 'text' \
         AND length(\"{columna}\") BETWEEN 2 AND 45 \
         AND (\"{columna}\" GLOB '[0-9]*.[0-9]*.[0-9]*.[0-9]*' OR \"{columna}\" GLOB '*:*') \
         GROUP BY \"{columna}\" LIMIT 20000"
    );
    let Ok(mut st) = conn.prepare(&sql) else {
        return (0, 0);
    };
    let Ok(filas) = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))) else {
        return (0, 0);
    };
    let (mut locales, mut publicas) = (0u64, 0u64);
    for (valor, n) in filas.flatten() {
        let v = valor.trim().trim_start_matches('[').trim_end_matches(']');
        if let Ok(ip) = v.parse::<IpAddr>() {
            let n = u64::try_from(n).unwrap_or(0);
            if es_direccion_local(&ip) {
                locales += n;
            } else {
                publicas += n;
            }
        }
    }
    (locales, publicas)
}

/// The inventory of a database connection.
///
/// # Errors
/// What SQLite says when the list of tables cannot be read.
pub fn inventario_de(conn: &Connection) -> Result<Inventario> {
    let mut nombres: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<_, _>>()?;
    nombres.retain(|n| nombre_seguro(n));
    let mut out = Inventario::default();
    for nombre in nombres {
        let filas: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM \"{nombre}\""), [], |r| {
                r.get(0)
            })
            .unwrap_or(0);
        // `dbstat` is there when SQLite was built with it; the size is then measured, not guessed.
        let bytes: Option<i64> = conn
            .query_row(
                "SELECT SUM(pgsize) FROM dbstat WHERE name = ?1",
                [&nombre],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        let mut columnas = Vec::new();
        if let Ok(mut st) = conn.prepare(&format!("PRAGMA table_info(\"{nombre}\")")) {
            let cols: Vec<(String, String)> = st
                .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
                .map(|it| it.flatten().collect())
                .unwrap_or_default();
            for (c, tipo) in cols {
                let (locales, publicas) = if nombre_seguro(&c) {
                    cuenta_direcciones(conn, &nombre, &c)
                } else {
                    (0, 0)
                };
                out.direcciones_locales += locales;
                out.direcciones_publicas += publicas;
                columnas.push(ColumnaInventario {
                    nombre: c,
                    tipo,
                    direcciones_locales: locales,
                    direcciones_publicas: publicas,
                });
            }
        }
        out.tablas.push(TablaInventario {
            nombre,
            filas: u64::try_from(filas).unwrap_or(0),
            bytes: bytes.and_then(|b| u64::try_from(b).ok()),
            columnas,
        });
    }
    let paginas: i64 = conn
        .query_row("PRAGMA page_count", [], |r| r.get(0))
        .unwrap_or(0);
    let tam: i64 = conn
        .query_row("PRAGMA page_size", [], |r| r.get(0))
        .unwrap_or(0);
    out.bytes_base = u64::try_from(paginas.saturating_mul(tam)).unwrap_or(0);
    Ok(out)
}

impl Ledger {
    /// What this ledger keeps, table by table (see [`inventario_de`]).
    ///
    /// # Errors
    /// As [`inventario_de`].
    pub fn inventario(&self) -> Result<Inventario> {
        inventario_de(&self.conn)
    }
}

/// The connections written down in another GUARDIANA file (GUARDIANA ZERO's `licencia.db`),
/// newest first, opened read-only: nothing in that file is changed, created or migrated. A
/// purpose this version does not know is skipped, not invented.
///
/// # Errors
/// When the file cannot be opened or has no `outbound` table.
pub fn salidas_en_archivo(ruta: &Path) -> Result<Vec<Outbound>> {
    let conn = Connection::open_with_flags(
        ruta,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let mut st = conn.prepare(
        "SELECT id, ts, purpose, host, bytes, initiated_by_user FROM outbound ORDER BY id DESC LIMIT 5000",
    )?;
    let filas = st.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, i32>(5)?,
        ))
    })?;
    let mut out = Vec::new();
    for f in filas {
        let (id, ts, p, host, bytes, por) = f?;
        if let Ok(purpose) = p.parse::<Purpose>() {
            out.push(Outbound {
                id,
                ts,
                purpose,
                host,
                bytes,
                initiated_by_user: por != 0,
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{Hash, NewEvent};

    #[test]
    fn local_and_public_addresses_are_told_apart() {
        for l in [
            "127.0.0.1",
            "192.168.1.20",
            "10.0.0.5",
            "172.20.1.1",
            "::1",
            "fe80::1",
            "fd00::2",
            "100.64.3.4",
        ] {
            assert!(es_direccion_local(&l.parse().unwrap()), "{l}");
        }
        for p in ["8.8.8.8", "142.250.72.14", "2606:4700::1111", "172.32.0.1"] {
            assert!(!es_direccion_local(&p.parse().unwrap()), "{p}");
        }
    }

    #[test]
    fn the_inventory_lists_every_table_and_counts_addresses_by_kind() {
        let mut l = Ledger::open_in_memory(Hash::of(b"inventario")).unwrap();
        l.append(NewEvent::observed(
            1,
            "self",
            "127.0.0.1",
            "example.com",
            "A",
        ))
        .unwrap();
        l.append(NewEvent::observed(
            2,
            "tv",
            "192.168.1.30",
            "samsungacr.com",
            "A",
        ))
        .unwrap();
        let inv = l.inventario().unwrap();
        let ev = inv.tablas.iter().find(|t| t.nombre == "events").unwrap();
        assert_eq!(ev.filas, 2);
        let ip = ev
            .columnas
            .iter()
            .find(|c| c.nombre == "client_ip")
            .unwrap();
        assert_eq!((ip.direcciones_locales, ip.direcciones_publicas), (2, 0));
        // A domain name is not an address, nor is a version.
        let q = ev.columnas.iter().find(|c| c.nombre == "qname").unwrap();
        assert_eq!((q.direcciones_locales, q.direcciones_publicas), (0, 0));
        assert_eq!(inv.direcciones_publicas, 0);
        assert!(inv.tablas.iter().any(|t| t.nombre == "outbound"));
        assert!(inv.bytes_base > 0);

        // A public address anywhere is reported, never hidden.
        l.connection()
            .execute("UPDATE devices SET last_ip = '8.8.8.8'", [])
            .unwrap();
        l.connection()
            .execute(
                "INSERT INTO settings (key, value) VALUES ('x', '203.0.113.9')",
                [],
            )
            .unwrap();
        let inv = l.inventario().unwrap();
        assert!(inv.direcciones_publicas >= 1);
    }

    #[test]
    fn another_files_connections_are_read_without_touching_it() {
        let dir = std::env::temp_dir().join(format!("inv-salidas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("licencia.db");
        {
            let mut l = Ledger::open(&ruta, Hash::of(b"zero")).unwrap();
            l.record_outbound(10, Purpose::Rele, "rele.guardianagroup.com", 0, true)
                .unwrap();
            l.record_outbound(20, Purpose::Licencia, "live.dodopayments.com", 312, true)
                .unwrap();
        }
        let antes = std::fs::metadata(&ruta).unwrap().len();
        let s = salidas_en_archivo(&ruta).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].purpose, Purpose::Licencia);
        assert_eq!(s[1].host, "rele.guardianagroup.com");
        assert_eq!(std::fs::metadata(&ruta).unwrap().len(), antes);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
