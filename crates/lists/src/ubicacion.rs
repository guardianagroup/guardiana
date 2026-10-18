//! Where a server is, said only as precisely as something proves (1.0.45).
//!
//! The owner, 18 Oct 2026: «precisión máxima de todos los datos hacia dónde van». Maximum is not
//! invented: the precision of a place is the precision of its evidence, and the page says which.
//!
//! - **City**, only when the network's own operator declares it in a geofeed (RFC 8805:
//!   DigitalOcean, Akamai/Linode, Vultr, see `build/geo.py`).
//! - **Region**, when a cloud publishes the region of the address (Amazon, Google Cloud, Azure,
//!   Oracle): the region as the cloud names it (`Virginia del Norte (Ashburn)`).
//! - Otherwise the bundled base (DB-IP «IP to City Lite», CC BY 4.0): its region or its country,
//!   always marked approximate. The city the base guesses is kept apart as «cerca de», never as
//!   the place, because a geolocation base's city is a guess, not a measurement.
//! - An anycast address answers from many places at once: no place at all («repartido»).
//!
//! No error radius is ever given: none of these sources publishes one for an address, and a
//! number of kilometres would look like a measurement GUARDIANA never made.

use std::net::IpAddr;

use serde::Serialize;

use crate::red::{ciudad_desde_clave, lugar_de, lugar_por_clave, servidor_de, Lugar};
use crate::{ciudad_de, CIUDAD_IP_JSON, GEO_JSON};

/// How precise a place is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Precision {
    /// A city, declared by the network's operator.
    Ciudad,
    /// A region: a cloud's, or the base's state or department.
    Region,
    /// A country only.
    Pais,
    /// Anycast: the same address answers from many places.
    Repartido,
    /// A private or local address: not on the internet.
    Local,
    /// Nothing known.
    SinDato,
}

/// What the place rests on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidencia {
    /// The operator's geofeed (RFC 8805).
    GeofeedOperador,
    /// The region a cloud publishes for its address ranges.
    RegionOperador,
    /// The geolocation base bundled with the program.
    BaseGeoip,
    /// The operator says the address is anycast (`anycast.txt`).
    Anycast,
    /// Nothing.
    Ninguna,
}

/// A server's place, as precise as its evidence and no more.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ubicacion {
    /// How precise.
    pub precision: Precision,
    /// What it rests on.
    pub evidencia: Evidencia,
    /// Approximate: it comes from the geolocation base, not from the operator. The page writes
    /// «aprox.» next to it.
    pub aprox: bool,
    /// The city, only with `precision == Ciudad`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ciudad: Option<String>,
    /// The city the base guesses, when it gives one and nothing better exists: shown as «cerca
    /// de», approximate, never as the place itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cerca_de: Option<String>,
    /// The region: a cloud's (`Virginia del Norte (Ashburn)`) or the base's state or department.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// A cloud's own code for the region (`us-east-1`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codigo_region: Option<String>,
    /// Two-letter country.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pais: Option<String>,
    /// Where to draw it: the operator's city or region, or the base's point (then `aprox`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lat: Option<f64>,
    /// Likewise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lon: Option<f64>,
    /// Who published the place, when an operator did (`DigitalOcean`, `Amazon Web Services`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operador: Option<String>,
    /// The source in words (`geofeed RFC 8805 de DigitalOcean`, `DB-IP IP to City Lite`).
    pub fuente: String,
    /// Of when: the day the operators' data was taken, or the base's month.
    pub fecha: String,
}

impl Ubicacion {
    fn vacia(precision: Precision, evidencia: Evidencia) -> Self {
        Self {
            precision,
            evidencia,
            aprox: false,
            ciudad: None,
            cerca_de: None,
            region: None,
            codigo_region: None,
            pais: None,
            lat: None,
            lon: None,
            operador: None,
            fuente: String::new(),
            fecha: String::new(),
        }
    }
}

/// The name of the operator behind a `lugares.json` provider code.
#[must_use]
pub fn operador_de_proveedor(p: &str) -> &'static str {
    match p {
        "aws" => "Amazon Web Services",
        "gcp" => "Google Cloud",
        "azure" => "Microsoft Azure",
        "oracle" => "Oracle Cloud",
        "do" => "DigitalOcean",
        "linode" => "Akamai (Linode)",
        "vultr" => "Vultr",
        _ => "",
    }
}

fn fecha_geo() -> String {
    serde_json::from_str::<serde_json::Value>(GEO_JSON)
        .ok()
        .and_then(|v| v["hecho"].as_str().map(|s| s.chars().take(10).collect()))
        .unwrap_or_default()
}

fn fuente_ciudad() -> (String, String) {
    let v: serde_json::Value = serde_json::from_str(CIUDAD_IP_JSON).unwrap_or_default();
    (
        v["fuente"].as_str().unwrap_or("DB-IP").to_owned(),
        v["fecha"].as_str().unwrap_or("").to_owned(),
    )
}

fn de_lugar(l: &Lugar) -> Ubicacion {
    let geofeed = l.region.contains('|');
    let operador = operador_de_proveedor(&l.proveedor);
    let mut u = Ubicacion::vacia(
        if geofeed {
            Precision::Ciudad
        } else {
            Precision::Region
        },
        if geofeed {
            Evidencia::GeofeedOperador
        } else {
            Evidencia::RegionOperador
        },
    );
    if geofeed {
        u.ciudad = Some(l.ciudad.clone());
        u.fuente = format!("geofeed RFC 8805 · {operador}");
    } else {
        u.region = Some(l.ciudad.clone());
        u.codigo_region = Some(l.region.clone());
        u.fuente = format!("región publicada · {operador}");
    }
    u.pais = Some(l.pais.clone());
    u.lat = Some(l.lat);
    u.lon = Some(l.lon);
    u.operador = Some(operador.to_owned()).filter(|o| !o.is_empty());
    u.fecha = fecha_geo();
    u
}

fn de_ciudad_base(c: &crate::Ciudad) -> Ubicacion {
    let (fuente, fecha) = fuente_ciudad();
    let con_region = !c.region.is_empty();
    let mut u = Ubicacion::vacia(
        if con_region {
            Precision::Region
        } else {
            Precision::Pais
        },
        Evidencia::BaseGeoip,
    );
    u.aprox = true;
    u.region = Some(c.region.clone()).filter(|r| !r.is_empty());
    u.cerca_de = Some(c.ciudad.clone()).filter(|r| !r.is_empty());
    u.pais = Some(c.pais.clone());
    u.lat = Some(c.lat);
    u.lon = Some(c.lon);
    u.fuente = fuente;
    u.fecha = fecha;
    u
}

/// The place of an address, looked up now in the data inside the program. The address is not
/// kept in what comes back.
#[must_use]
pub fn ubicacion_de_ip(ip: IpAddr) -> Ubicacion {
    let s = servidor_de(ip);
    if s.local {
        return Ubicacion::vacia(Precision::Local, Evidencia::Ninguna);
    }
    if s.anycast {
        return repartido();
    }
    if let Some(l) = lugar_de(ip) {
        return de_lugar(l);
    }
    if let Some(c) = ciudad_de(ip) {
        return de_ciudad_base(c);
    }
    de_pais(s.pais)
}

fn repartido() -> Ubicacion {
    let mut u = Ubicacion::vacia(Precision::Repartido, Evidencia::Anycast);
    "anycast.txt".clone_into(&mut u.fuente);
    u
}

fn de_pais(pais: Option<&str>) -> Ubicacion {
    match pais {
        Some(p) if p.len() == 2 => {
            let f = crate::fuente_red();
            let mut u = Ubicacion::vacia(Precision::Pais, Evidencia::BaseGeoip);
            u.aprox = true;
            u.pais = Some(p.to_owned());
            u.fuente.clone_from(&f.fuente);
            u.fecha.clone_from(&f.fecha);
            u
        }
        _ => Ubicacion::vacia(Precision::SinDato, Evidencia::Ninguna),
    }
}

/// The place a measurement kept as its key (`aws:us-east-1`, `dbip:CO|Bolívar|…`), with the
/// country it kept beside it for when the key is empty or no longer known.
#[must_use]
pub fn ubicacion_de_clave(clave: &str, pais: &str) -> Ubicacion {
    if let Some(c) = ciudad_desde_clave(clave) {
        return de_ciudad_base(&c);
    }
    if let Some(l) = lugar_por_clave(clave) {
        return de_lugar(l);
    }
    de_pais(Some(pais).filter(|p| p.len() == 2))
}

/// The place of a query's server as the extract kept it (decision 203): its kind and country.
#[must_use]
pub fn ubicacion_de_servidor(s: &guardiana_core::ServerInfo) -> Ubicacion {
    match s.kind {
        guardiana_core::ServerKind::Local => Ubicacion::vacia(Precision::Local, Evidencia::Ninguna),
        guardiana_core::ServerKind::Anycast => repartido(),
        guardiana_core::ServerKind::Publico => de_pais(Some(s.country.as_str())),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn claves(u: &Ubicacion) -> Vec<String> {
        serde_json::to_value(u)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    }

    #[test]
    fn an_operators_region_is_a_region_not_a_city() {
        let u = ubicacion_de_clave("aws:us-east-1", "US");
        assert_eq!(u.precision, Precision::Region);
        assert_eq!(u.evidencia, Evidencia::RegionOperador);
        assert!(!u.aprox);
        assert!(u.ciudad.is_none());
        assert_eq!(u.codigo_region.as_deref(), Some("us-east-1"));
        assert_eq!(u.operador.as_deref(), Some("Amazon Web Services"));
        assert_eq!(u.fecha.len(), 10, "{u:?}");
    }

    #[test]
    fn a_geofeed_gives_a_city() {
        if let Some(l) = crate::red::lugares_para_pruebas()
            .iter()
            .find(|l| l.region.contains('|'))
        {
            let u = ubicacion_de_clave(&l.clave(), "");
            assert_eq!(u.precision, Precision::Ciudad);
            assert_eq!(u.evidencia, Evidencia::GeofeedOperador);
            assert!(!u.aprox);
            assert_eq!(u.ciudad.as_deref(), Some(l.ciudad.as_str()));
        }
    }

    #[test]
    fn the_bases_city_is_only_near_and_approximate() {
        let u = ubicacion_de_ip("181.49.0.1".parse().unwrap());
        assert_eq!(u.evidencia, Evidencia::BaseGeoip);
        assert!(u.aprox);
        assert_ne!(u.precision, Precision::Ciudad, "a base never makes a city");
        assert!(u.ciudad.is_none());
        assert_eq!(u.cerca_de.as_deref(), Some("Cartagena"));
        assert_eq!(u.pais.as_deref(), Some("CO"));
        // The same from the key the measurement kept.
        let k = crate::ciudad_de("181.49.0.1".parse().unwrap())
            .unwrap()
            .clave();
        assert_eq!(ubicacion_de_clave(&k, "CO"), u);
    }

    #[test]
    fn anycast_local_and_unknown_say_so() {
        let u = ubicacion_de_ip("8.8.8.8".parse().unwrap());
        assert_eq!(u.precision, Precision::Repartido);
        assert!(u.pais.is_none() && u.lat.is_none());
        let u = ubicacion_de_ip("192.168.1.1".parse().unwrap());
        assert_eq!(u.precision, Precision::Local);
        let u = ubicacion_de_clave("", "");
        assert_eq!(u.precision, Precision::SinDato);
        let u = ubicacion_de_clave("", "DE");
        assert_eq!(u.precision, Precision::Pais);
        assert!(u.aprox);
    }

    #[test]
    fn a_query_server_is_a_country_at_most() {
        let s = guardiana_core::ServerInfo {
            kind: guardiana_core::ServerKind::Publico,
            country: "IE".into(),
            asn: 32934,
            base: "dbip-2026-10".into(),
        };
        let u = ubicacion_de_servidor(&s);
        assert_eq!(u.precision, Precision::Pais);
        assert!(u.aprox);
        assert_eq!(u.pais.as_deref(), Some("IE"));
    }

    /// Honesty: no place ever carries an error radius, a distance or a confidence number.
    #[test]
    fn no_place_invents_a_radius() {
        let casos = [
            ubicacion_de_clave("aws:us-east-1", "US"),
            ubicacion_de_ip("181.49.0.1".parse().unwrap()),
            ubicacion_de_ip("8.8.8.8".parse().unwrap()),
            ubicacion_de_clave("", "DE"),
        ];
        for u in &casos {
            for k in claves(u) {
                for prohibida in ["radio", "radius", "km", "error", "precision_m", "confianza"] {
                    assert!(!k.contains(prohibida), "{k}");
                }
            }
            // A city only with the operator's word.
            if u.ciudad.is_some() {
                assert_eq!(u.evidencia, Evidencia::GeofeedOperador);
            }
            if u.evidencia == Evidencia::BaseGeoip {
                assert!(u.aprox);
            }
        }
    }
}
