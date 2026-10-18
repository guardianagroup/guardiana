//! Who is behind a destination, by layers, and how sure that is (1.0.45).
//!
//! The owner, 18 Oct 2026: «precisión máxima de todos los datos hacia dónde van, vienen desde
//! cualquier país del mundo». Until 1.0.44 a name either was in `empresas.txt` or read «sin
//! identificar», and a server rented on Amazon Web Services could read as Amazon. This module puts
//! every clue GUARDIANA has, all of it inside the program (nothing is asked of anybody per name),
//! in a fixed order, and says which one decided:
//!
//! 1. **The name itself** in our curated base (`empresas.txt`, `empresas-mundo.txt`), in the
//!    maker's own documentation (`antivirus.txt`), in the public register of data brokers
//!    (`corredores.txt`) or in the AI list (`ia.txt`). Any of them: «Identificada».
//! 2. **The aliases our own resolver saw** (CNAME). GUARDIANA is the resolver, so when
//!    `metricas.tienda.com` answers as `tienda.com.ssl.d1.sc.omtrdc.net`, the answer itself says
//!    whose service it is (Adobe's). The alias's owner from the base: «Identificada», with the
//!    alias and the day it was seen as the proof. An alias into a delivery network or a cloud says
//!    whose road it is, never whose name.
//! 3. **The network** that answered (ASN, from DB-IP «IP to ASN Lite» and `redes.txt`). Only a
//!    network that carries nothing but its owner's own services (Meta's, Apple's, Netflix's),
//!    and only while the base's own name for it agrees with `redes.txt`: «Probable». A cloud, a
//!    delivery network or an access provider never names the company: it is the infrastructure,
//!    shown apart (hosted on AWS is not Amazon).
//! 4. Otherwise «Sin identificar», and the network operator is said instead, so nothing reads as
//!    a blank.
//!
//! Every clue carries where it comes from and of when (`Prueba`). Open lists with a
//! non-commercial licence (DuckDuckGo Tracker Radar, Disconnect, CC BY-NC-SA) are not used: this
//! program is sold (docs/LISTS.md).

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Serialize;

use crate::{
    ai_service_of, antivirus, company_match, data_broker_of, delivery_of, dominio, fuente_red,
    hosting_of, is_local_name, nombre_de_red, OrigenEmpresa, ALOJAMIENTO, CORREDORES, EMPRESAS,
    EMPRESAS_MUNDO, ENTREGA, IA, REDES,
};

/// The edition of GUARDIANA's own lists that carry no date of their own: the day the lists of this
/// version were closed. Each list that writes an `# edicion:` line says its own.
pub const EDICION_LISTAS: &str = "2026-10-18";

/// The copy of the data brokers' register the list was made from (`corredores.txt`: «Copia del
/// 8 de octubre de 2026»).
pub const FECHA_CORREDORES: &str = "2026-10-08";

/// The day `antivirus.txt` was written from the makers' pages («13 oct 2026» in its header).
pub const FECHA_ANTIVIRUS: &str = "2026-10-13";

/// How sure GUARDIANA is of who is behind a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Nivel {
    /// A direct source names the owner of this name, or of the alias it answered as.
    Identificada,
    /// No source names it, but the server answered from a network only that company uses for its
    /// own services, and two independent sources agree on whose network it is.
    Probable,
    /// Nothing says who. The network operator is shown instead.
    SinIdentificar,
}

/// Where a clue comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Fuente {
    /// `empresas.txt` / `empresas-mundo.txt`: GUARDIANA's curated base.
    BaseGuardiana,
    /// `antivirus.txt`: the maker documents this name as its own.
    Fabricante,
    /// `corredores.txt`: the California register of data brokers.
    RegistroCorredores,
    /// `ia.txt`: GUARDIANA's list of AI services.
    ListaIa,
    /// An alias (CNAME) GUARDIANA's own resolver saw in the answer.
    AliasDns,
    /// `redes.txt`: what kind of network an ASN is.
    ListaRedes,
    /// The network base bundled with the program (DB-IP «IP to ASN Lite»).
    BaseRed,
    /// `alojamiento.txt`: a cloud where anyone rents a server under the cloud's name.
    ListaAlojamiento,
    /// `entrega.txt`: a delivery network.
    ListaEntrega,
}

/// One clue: where from, what exactly, of when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Prueba {
    /// Which source.
    pub fuente: Fuente,
    /// What it said, as data (the entry that matched `facebook.com`, the alias `omtrdc.net`,
    /// `AS32934`, the maker's page). Never an address.
    pub detalle: String,
    /// Of when: `AAAA-MM-DD` for a list's edition or the day an alias was seen, `AAAA-MM` for the
    /// network base's month.
    pub fecha: String,
}

/// What kind of network served the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoRed {
    /// Only its owner's own services.
    Propia,
    /// Its owner's services and those of customers who rent.
    Mixta,
    /// A cloud: servers rented to anyone.
    Nube,
    /// A delivery network carrying other companies' content.
    Reparto,
    /// A filter or proxy its customers' traffic goes through.
    Seguridad,
    /// Web hosting for others.
    Alojamiento,
    /// An internet access provider.
    Acceso,
    /// A network `redes.txt` does not classify: only its name is known.
    Desconocido,
}

impl TipoRed {
    fn de_texto(s: &str) -> Option<Self> {
        Some(match s {
            "propia" => Self::Propia,
            "mixta" => Self::Mixta,
            "nube" => Self::Nube,
            "reparto" => Self::Reparto,
            "seguridad" => Self::Seguridad,
            "alojamiento" => Self::Alojamiento,
            "acceso" => Self::Acceso,
            _ => return None,
        })
    }
}

/// The infrastructure a name was served from: the road, never the owner of the name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Infraestructura {
    /// Who runs it (`Amazon Web Services`, `Akamai`, `Claro`), or the base's own name for the
    /// network when no list classifies it.
    pub operador: String,
    /// Its autonomous system, when the clue was the network.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asn: Option<u32>,
    /// What kind of network.
    pub tipo: TipoRed,
    /// The company of the lists that owns the operator, when there is one (`Amazon` for AWS).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empresa_operador: Option<&'static str>,
    /// The operator is the very company behind the name (Meta's name on Meta's network).
    pub misma_empresa: bool,
    /// Where this comes from.
    pub prueba: Prueba,
}

/// Who is behind one name, with every clue that says it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Atribucion {
    /// How sure.
    pub nivel: Nivel,
    /// The company behind the service, when identified or probable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empresa: Option<&'static str>,
    /// Its group, when the lists write one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grupo: Option<&'static str>,
    /// Two-letter country of the company's seat (not of the server).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pais_empresa: Option<&'static str>,
    /// Its seat's city, when written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ciudad_empresa: Option<&'static str>,
    /// When the company came from an alias: the site whose name it answered under
    /// (`tienda.com`): «Adobe, bajo un nombre de tienda.com».
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bajo_nombre_de: Option<String>,
    /// Another company whose service answers under this name, seen in its aliases, when the name
    /// has an owner of its own (`login.banco.com` → Okta).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servicio_de: Option<&'static str>,
    /// The name is a server rented on a cloud (`x.herokuapp.com` → Heroku): it belongs to whoever
    /// rents it, which the name does not say.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alojado_en: Option<&'static str>,
    /// The name belongs to a delivery network: the road for somebody else's content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reparto: Option<&'static str>,
    /// The machine's own network talking (reverse lookups, mDNS): not a destination at all.
    pub local: bool,
    /// Every clue, the deciding one first.
    pub pruebas: Vec<Prueba>,
    /// The network or service it was served from, apart from who owns the name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub infraestructura: Option<Infraestructura>,
    /// What to show when nobody is identified: the network operator, or the cloud, or the
    /// delivery network. `None` when not even that is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operador_red: Option<String>,
}

/// What is known of one destination when it is attributed.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pistas<'a> {
    /// The name asked for.
    pub qname: &'a str,
    /// The sites of the aliases the answer went through, in order (`dominio::sitio_de_alias`).
    pub alias: &'a [String],
    /// The day those aliases were last seen, `AAAA-MM-DD`.
    pub alias_visto: Option<&'a str>,
    /// The autonomous system of the server that answered, 0 when unknown.
    pub asn: u32,
}

// ----- the lists' own dates -------------------------------------------------------------------

/// The `# edicion: AAAA-MM-DD` line of a list, when it has one.
#[must_use]
pub fn edicion_de(texto: &str) -> Option<&str> {
    texto.lines().find_map(|l| {
        let f = l
            .trim()
            .strip_prefix('#')?
            .trim()
            .strip_prefix("edicion:")?
            .trim();
        let ok = f.len() == 10 && f.as_bytes()[4] == b'-' && f.as_bytes()[7] == b'-';
        ok.then_some(f)
    })
}

fn fecha_lista(texto: &'static str) -> String {
    edicion_de(texto).unwrap_or(EDICION_LISTAS).to_owned()
}

// ----- redes.txt ------------------------------------------------------------------------------

/// What `redes.txt` says of one autonomous system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RedInfo {
    /// The ASN.
    pub asn: u32,
    /// Who runs it, as the list names it.
    pub operador: &'static str,
    /// What kind of network.
    pub tipo: TipoRed,
    /// Words the base's own name for it must contain.
    pub senas: Vec<&'static str>,
    /// The company of the lists that owns it, when written.
    pub empresa: Option<&'static str>,
}

fn redes() -> &'static HashMap<u32, RedInfo> {
    static R: OnceLock<HashMap<u32, RedInfo>> = OnceLock::new();
    R.get_or_init(|| {
        let mut out = HashMap::new();
        let mut actual: Option<(
            &'static str,
            TipoRed,
            Vec<&'static str>,
            Option<&'static str>,
        )> = None;
        for linea in REDES.lines() {
            let linea = linea.split('#').next().unwrap_or("").trim();
            if let Some(h) = linea.strip_prefix('@') {
                let mut f = h.split('|').map(str::trim);
                let operador = f.next().unwrap_or("");
                let tipo = f.next().and_then(TipoRed::de_texto);
                let senas = f
                    .next()
                    .unwrap_or("")
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect();
                let empresa = f.next().filter(|e| !e.is_empty());
                actual = tipo.map(|t| (operador, t, senas, empresa));
            } else if let (Some(n), Some((op, tipo, senas, empresa))) = (
                linea.strip_prefix("AS").and_then(|n| n.parse::<u32>().ok()),
                actual.as_ref(),
            ) {
                out.entry(n).or_insert_with(|| RedInfo {
                    asn: n,
                    operador: op,
                    tipo: *tipo,
                    senas: senas.clone(),
                    empresa: *empresa,
                });
            }
        }
        out
    })
}

/// What `redes.txt` says of a network, or `None` when it does not classify it.
#[must_use]
pub fn red_de_asn(asn: u32) -> Option<&'static RedInfo> {
    redes().get(&asn)
}

/// Every network of `redes.txt`, for tests and tools.
#[must_use]
pub fn todas_las_redes() -> Vec<&'static RedInfo> {
    let mut v: Vec<&RedInfo> = redes().values().collect();
    v.sort_by_key(|r| r.asn);
    v
}

/// Whether the network base's own name for `asn` contains one of the list's words for it. With no
/// name in the base there is nothing to agree with, and the answer is no.
#[must_use]
pub fn base_coincide(red: &RedInfo) -> bool {
    nombre_de_red(red.asn).is_some_and(|n| {
        let n = n.to_lowercase();
        red.senas.iter().any(|s| n.contains(&s.to_lowercase()))
    })
}

// ----- the company of a name ------------------------------------------------------------------

/// What the header of a company says, split.
struct Cabecera {
    nombre: &'static str,
    pais: Option<&'static str>,
    ciudad: Option<&'static str>,
    grupo: Option<&'static str>,
}

fn cabecera(h: &'static str) -> Cabecera {
    let mut f = h.split('|').map(str::trim);
    Cabecera {
        nombre: f.next().unwrap_or(""),
        pais: f.next().filter(|p| p.len() == 2),
        ciudad: f.next().filter(|c| !c.is_empty()),
        grupo: f.next().filter(|g| !g.is_empty()),
    }
}

/// The owner a name has by itself, from the direct sources, with their proofs.
struct DuenoDirecto {
    empresa: &'static str,
    grupo: Option<&'static str>,
    pais: Option<&'static str>,
    ciudad: Option<&'static str>,
    pruebas: Vec<Prueba>,
}

fn dueno_directo(qname: &str) -> Option<DuenoDirecto> {
    let mut out: Option<DuenoDirecto> = None;
    if let Some((h, sufijo, origen)) = company_match(qname) {
        let c = cabecera(h);
        let lista = match origen {
            OrigenEmpresa::Empresas => EMPRESAS,
            OrigenEmpresa::Mundo => EMPRESAS_MUNDO,
        };
        out = Some(DuenoDirecto {
            empresa: c.nombre,
            grupo: c.grupo,
            pais: c.pais,
            ciudad: c.ciudad,
            pruebas: vec![Prueba {
                fuente: Fuente::BaseGuardiana,
                detalle: sufijo.to_owned(),
                fecha: fecha_lista(lista),
            }],
        });
    }
    // The maker's own documentation: a second proof for a company already known, or the owner.
    for av in antivirus::antivirus() {
        if let Some(d) = antivirus::uso_documentado(av, qname) {
            let prueba = Prueba {
                fuente: Fuente::Fabricante,
                detalle: d.fuente.to_owned(),
                fecha: FECHA_ANTIVIRUS.to_owned(),
            };
            match out.as_mut() {
                Some(o) => o.pruebas.push(prueba),
                None => {
                    out = Some(DuenoDirecto {
                        empresa: av.empresa,
                        grupo: None,
                        pais: Some(av.pais).filter(|p| p.len() == 2),
                        ciudad: None,
                        pruebas: vec![prueba],
                    });
                }
            }
            break;
        }
    }
    if let Some(b) = data_broker_of(qname) {
        let prueba = Prueba {
            fuente: Fuente::RegistroCorredores,
            detalle: b.name.to_owned(),
            fecha: fecha_lista_o(CORREDORES, FECHA_CORREDORES),
        };
        match out.as_mut() {
            Some(o) => o.pruebas.push(prueba),
            None => {
                out = Some(DuenoDirecto {
                    empresa: b.name,
                    grupo: None,
                    pais: b.country,
                    ciudad: None,
                    pruebas: vec![prueba],
                });
            }
        }
    }
    if let Some(ia) = ai_service_of(qname) {
        let prueba = Prueba {
            fuente: Fuente::ListaIa,
            detalle: ia.to_owned(),
            fecha: fecha_lista(IA),
        };
        match out.as_mut() {
            Some(o) => o.pruebas.push(prueba),
            None => {
                out = Some(DuenoDirecto {
                    empresa: ia,
                    grupo: None,
                    pais: None,
                    ciudad: None,
                    pruebas: vec![prueba],
                });
            }
        }
    }
    out
}

fn fecha_lista_o(texto: &'static str, otra: &str) -> String {
    edicion_de(texto).unwrap_or(otra).to_owned()
}

/// The group a company adds up to: its group when written, else itself.
fn grupo_o_empresa(empresa: &str, grupo: Option<&str>) -> String {
    grupo.unwrap_or(empresa).to_owned()
}

fn misma(a: (&str, Option<&str>), b: (&str, Option<&str>)) -> bool {
    a.0 == b.0 || grupo_o_empresa(a.0, a.1) == grupo_o_empresa(b.0, b.1)
}

// ----- the attribution ------------------------------------------------------------------------

/// Who is behind one destination, by the layers of the module's documentation. Pure: the same
/// clues always give the same answer, and nothing leaves the program.
#[must_use]
pub fn atribuir(p: &Pistas<'_>) -> Atribucion {
    let qname = p.qname.trim_end_matches('.').to_ascii_lowercase();
    let mut a = Atribucion {
        nivel: Nivel::SinIdentificar,
        empresa: None,
        grupo: None,
        pais_empresa: None,
        ciudad_empresa: None,
        bajo_nombre_de: None,
        servicio_de: None,
        alojado_en: None,
        reparto: None,
        local: is_local_name(&qname),
        pruebas: Vec::new(),
        infraestructura: None,
        operador_red: None,
    };
    if a.local {
        return a;
    }
    let sitio_pedido = dominio::sitio(&qname);
    let fecha_alias = p.alias_visto.unwrap_or(EDICION_LISTAS).to_owned();

    // 1. The name by itself. A rented server or a delivery name has no owner of its own here.
    a.alojado_en = hosting_of(&qname);
    a.reparto = if a.alojado_en.is_none() {
        delivery_of(&qname)
    } else {
        None
    };
    if a.alojado_en.is_none() && a.reparto.is_none() {
        if let Some(d) = dueno_directo(&qname) {
            a.nivel = Nivel::Identificada;
            a.empresa = Some(d.empresa);
            a.grupo = d.grupo;
            a.pais_empresa = d.pais;
            a.ciudad_empresa = d.ciudad;
            a.pruebas = d.pruebas;
        }
    }

    // 2. The aliases the answer went through: the first that belongs to a company (not a road).
    let mut infra_alias: Option<Infraestructura> = None;
    for alias in p.alias {
        let alias = alias.trim_end_matches('.').to_ascii_lowercase();
        if alias.is_empty() || alias.parse::<std::net::IpAddr>().is_ok() {
            continue;
        }
        if let Some(nube) = hosting_of(&alias) {
            infra_alias.get_or_insert_with(|| Infraestructura {
                operador: nube.to_owned(),
                asn: None,
                tipo: TipoRed::Nube,
                empresa_operador: None,
                misma_empresa: false,
                prueba: Prueba {
                    fuente: Fuente::AliasDns,
                    detalle: alias.clone(),
                    fecha: fecha_alias.clone(),
                },
            });
            continue;
        }
        if let Some(red) = delivery_of(&alias) {
            infra_alias.get_or_insert_with(|| Infraestructura {
                operador: red.to_owned(),
                asn: None,
                tipo: TipoRed::Reparto,
                empresa_operador: None,
                misma_empresa: false,
                prueba: Prueba {
                    fuente: Fuente::AliasDns,
                    detalle: alias.clone(),
                    fecha: fecha_alias.clone(),
                },
            });
            continue;
        }
        if dominio::sitio(&alias) == sitio_pedido {
            continue;
        }
        let Some(d) = dueno_directo(&alias) else {
            continue;
        };
        let alias_prueba = Prueba {
            fuente: Fuente::AliasDns,
            detalle: alias.clone(),
            fecha: fecha_alias.clone(),
        };
        match a.empresa {
            None if a.alojado_en.is_none() && a.reparto.is_none() => {
                a.nivel = Nivel::Identificada;
                a.empresa = Some(d.empresa);
                a.grupo = d.grupo;
                a.pais_empresa = d.pais;
                a.ciudad_empresa = d.ciudad;
                a.bajo_nombre_de = Some(sitio_pedido.clone());
                a.pruebas.push(alias_prueba);
                a.pruebas.extend(d.pruebas);
            }
            Some(e) if !misma((e, a.grupo), (d.empresa, d.grupo)) && a.servicio_de.is_none() => {
                a.servicio_de = Some(d.empresa);
                a.pruebas.push(alias_prueba);
                a.pruebas.extend(d.pruebas);
            }
            _ => {}
        }
        break;
    }

    // 3. The network that answered.
    let infra_red = infra_de_asn(p.asn);
    if a.empresa.is_none() && a.alojado_en.is_none() && a.reparto.is_none() {
        if let Some(r) = red_de_asn(p.asn) {
            if r.tipo == TipoRed::Propia && base_coincide(r) {
                if let Some(e) = r.empresa {
                    let c = crate::company_record(e);
                    a.nivel = Nivel::Probable;
                    a.empresa = Some(e);
                    a.grupo = c.as_ref().and_then(|c| c.group);
                    a.pais_empresa = c.as_ref().and_then(|c| c.country);
                    a.ciudad_empresa = c.as_ref().and_then(|c| c.city);
                    a.pruebas.push(Prueba {
                        fuente: Fuente::ListaRedes,
                        detalle: format!("AS{}", r.asn),
                        fecha: fecha_lista(REDES),
                    });
                    a.pruebas.push(Prueba {
                        fuente: Fuente::BaseRed,
                        detalle: nombre_de_red(r.asn).unwrap_or("").trim().to_owned(),
                        fecha: fuente_red().fecha.clone(),
                    });
                }
            }
        }
    }

    // The infrastructure: the network when known, else what the aliases said, else the name's
    // own cloud or delivery network.
    let mut infra = infra_red.or(infra_alias).or_else(|| {
        a.alojado_en
            .map(|n| (n, TipoRed::Nube, Fuente::ListaAlojamiento, ALOJAMIENTO))
            .or_else(|| {
                a.reparto
                    .map(|n| (n, TipoRed::Reparto, Fuente::ListaEntrega, ENTREGA))
            })
            .map(|(n, tipo, fuente, lista)| Infraestructura {
                operador: n.to_owned(),
                asn: None,
                tipo,
                empresa_operador: None,
                misma_empresa: false,
                prueba: Prueba {
                    fuente,
                    detalle: qname.clone(),
                    fecha: fecha_lista(lista),
                },
            })
    });
    if let (Some(i), Some(e)) = (infra.as_mut(), a.empresa) {
        if let Some(op) = i.empresa_operador {
            let c = crate::company_record(op);
            i.misma_empresa = misma((e, a.grupo), (op, c.as_ref().and_then(|c| c.group)));
        }
    }
    if a.nivel == Nivel::SinIdentificar {
        a.operador_red = infra.as_ref().map(|i| i.operador.clone());
    }
    a.infraestructura = infra;
    a
}

fn infra_de_asn(asn: u32) -> Option<Infraestructura> {
    if asn == 0 {
        return None;
    }
    if let Some(r) = red_de_asn(asn) {
        return Some(Infraestructura {
            operador: r.operador.to_owned(),
            asn: Some(asn),
            tipo: r.tipo,
            empresa_operador: r.empresa,
            misma_empresa: false,
            prueba: Prueba {
                fuente: Fuente::ListaRedes,
                detalle: format!("AS{asn}"),
                fecha: fecha_lista(REDES),
            },
        });
    }
    if let Some(e) = crate::company_of_asn(asn) {
        return Some(Infraestructura {
            operador: e.to_owned(),
            asn: Some(asn),
            tipo: TipoRed::Desconocido,
            empresa_operador: Some(e),
            misma_empresa: false,
            prueba: Prueba {
                fuente: Fuente::BaseGuardiana,
                detalle: format!("AS{asn}"),
                fecha: fecha_lista(EMPRESAS),
            },
        });
    }
    let nombre = nombre_de_red(asn)?.trim();
    Some(Infraestructura {
        operador: nombre.to_owned(),
        asn: Some(asn),
        tipo: TipoRed::Desconocido,
        empresa_operador: None,
        misma_empresa: false,
        prueba: Prueba {
            fuente: Fuente::BaseRed,
            detalle: format!("AS{asn}"),
            fecha: fuente_red().fecha.clone(),
        },
    })
}

// ----- how much is identified -----------------------------------------------------------------

/// How many queries of a window each level covers, and as whole percents of the queries that
/// left the house (local names do not count).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub struct Cobertura {
    /// Queries counted.
    pub consultas: i64,
    /// Of them, identified.
    pub identificada: i64,
    /// Probable.
    pub probable: i64,
    /// Not identified.
    pub sin_identificar: i64,
    /// Whole percent identified: the «% identificado».
    pub pct_identificada: i64,
    /// Whole percent probable.
    pub pct_probable: i64,
    /// Whole percent not identified.
    pub pct_sin_identificar: i64,
}

/// Whole percent, rounded half up; 0 of 0 is 0.
fn pct(parte: i64, todo: i64) -> i64 {
    if todo <= 0 {
        0
    } else {
        (parte * 200 + todo) / (2 * todo)
    }
}

/// The coverage of a set of attributions with how many queries each one stands for.
pub fn cobertura<'a>(filas: impl IntoIterator<Item = (&'a Atribucion, i64)>) -> Cobertura {
    let mut c = Cobertura::default();
    for (a, n) in filas {
        if a.local || n <= 0 {
            continue;
        }
        c.consultas += n;
        match a.nivel {
            Nivel::Identificada => c.identificada += n,
            Nivel::Probable => c.probable += n,
            Nivel::SinIdentificar => c.sin_identificar += n,
        }
    }
    c.pct_identificada = pct(c.identificada, c.consultas);
    c.pct_probable = pct(c.probable, c.consultas);
    c.pct_sin_identificar = pct(c.sin_identificar, c.consultas);
    c
}

/// The alias sites an answer's chain leaves for the record (1.0.45): each alias's
/// [`dominio::sitio_de_alias`], in order, without the asked name's own site, without repeats, at
/// most `max`. Names only, never an address.
#[must_use]
pub fn sitios_de_cadena(qname: &str, chain: &[String], max: usize) -> Vec<String> {
    let propio = dominio::sitio(qname);
    let mut out: Vec<String> = Vec::new();
    for alias in chain {
        if alias.parse::<std::net::IpAddr>().is_ok() {
            continue;
        }
        let s = dominio::sitio_de_alias(alias);
        if s.is_empty() || !s.contains('.') || s == propio || out.contains(&s) {
            continue;
        }
        out.push(s);
        if out.len() >= max {
            break;
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn de(qname: &str) -> Atribucion {
        atribuir(&Pistas {
            qname,
            ..Pistas::default()
        })
    }

    fn con(qname: &str, alias: &[&str], asn: u32) -> Atribucion {
        let alias: Vec<String> = alias.iter().map(|s| (*s).to_owned()).collect();
        atribuir(&Pistas {
            qname,
            alias: &alias,
            alias_visto: Some("2026-10-18"),
            asn,
        })
    }

    #[test]
    fn a_name_in_the_base_is_identified_with_its_source_and_date() {
        let a = de("graph.facebook.com");
        assert_eq!(a.nivel, Nivel::Identificada);
        assert_eq!(a.empresa, Some("Meta"));
        assert_eq!(a.pais_empresa, Some("US"));
        let p = &a.pruebas[0];
        assert_eq!(p.fuente, Fuente::BaseGuardiana);
        assert_eq!(p.detalle, "facebook.com");
        assert_eq!(p.fecha.len(), 10, "{p:?}");
        // From the world list, with its own edition date and the company's header inherited.
        let a = de("login.microsoftonline.com");
        assert_eq!(a.empresa, Some("Microsoft"));
        assert_eq!(a.pais_empresa, Some("US"));
        assert_eq!(a.pruebas[0].fecha, edicion_de(EMPRESAS_MUNDO).unwrap());
        let a = de("www.taobao.com");
        assert_eq!(a.empresa, Some("Alibaba Group"));
        assert_eq!(a.pais_empresa, Some("CN"));
    }

    #[test]
    fn the_makers_documentation_adds_a_proof() {
        let a = de("wdcp.microsoft.com");
        assert_eq!(a.empresa, Some("Microsoft"));
        assert!(a.pruebas.iter().any(|p| p.fuente == Fuente::Fabricante
            && p.detalle.starts_with("https://")
            && p.fecha == FECHA_ANTIVIRUS));
    }

    #[test]
    fn an_alias_reveals_the_service_behind_a_first_party_name() {
        // The classic: a sub-name of the shop that answers as Adobe's measurement.
        let a = con(
            "metricas.tienda-ejemplo.com",
            &["tienda-ejemplo.com.ssl.d1.sc.omtrdc.net"],
            0,
        );
        assert_eq!(a.nivel, Nivel::Identificada);
        assert_eq!(a.empresa, Some("Adobe"));
        assert_eq!(a.bajo_nombre_de.as_deref(), Some("tienda-ejemplo.com"));
        assert_eq!(a.pruebas[0].fuente, Fuente::AliasDns);
        assert_eq!(a.pruebas[0].fecha, "2026-10-18");
        // A name with its own owner keeps it; the alias names the service it uses.
        let a = con("login.mercadolibre.com", &["mercadolibre.okta.com"], 0);
        assert_eq!(a.empresa, Some("Mercado Libre"));
        assert_eq!(a.servicio_de, Some("Okta"));
        // The same company under another of its names is not somebody else's service.
        let a = con("www.youtube.com", &["youtube-ui.l.google.com"], 0);
        assert_eq!(a.empresa, Some("Google"));
        assert_eq!(a.servicio_de, None);
    }

    #[test]
    fn a_road_never_becomes_the_owner() {
        // Through a delivery network: the name stays unidentified, the road is the infrastructure.
        let a = con("www.tienda-ejemplo.com", &["e123.a.akamaiedge.net"], 0);
        assert_eq!(a.nivel, Nivel::SinIdentificar);
        assert_eq!(a.empresa, None);
        let i = a.infraestructura.as_ref().unwrap();
        assert_eq!(i.tipo, TipoRed::Reparto);
        assert_eq!(i.operador, "Akamai");
        assert_eq!(a.operador_red.as_deref(), Some("Akamai"));
        // A rented server: whoever rents it, not the cloud.
        let a = de("mi-app.herokuapp.com");
        assert_eq!(a.empresa, None);
        assert_eq!(a.alojado_en, Some("Heroku"));
        assert_eq!(a.nivel, Nivel::SinIdentificar);
        // A company that documents its own rented server keeps it (the longer entry wins).
        assert_eq!(crate::hosting_of("triplewhale-pixel.web.app"), None);
        assert_eq!(crate::hosting_of("otra-cosa.web.app"), Some("Google Cloud"));
    }

    #[test]
    fn hosted_on_a_cloud_is_not_the_cloud_company() {
        // An unknown name answered from AWS: not Amazon. The operator is said, apart.
        let a = con("api.app-desconocida.example", &[], 16509);
        assert_eq!(a.nivel, Nivel::SinIdentificar);
        assert_eq!(a.empresa, None);
        let i = a.infraestructura.as_ref().unwrap();
        assert_eq!(i.operador, "Amazon Web Services");
        assert_eq!(i.tipo, TipoRed::Nube);
        assert_eq!(i.empresa_operador, Some("Amazon"));
        assert!(!i.misma_empresa);
        // A known name on a cloud keeps its owner and says where it is hosted.
        let a = con("www.netflix.com", &[], 16509);
        assert_eq!(a.empresa, Some("Netflix"));
        assert_eq!(a.infraestructura.unwrap().tipo, TipoRed::Nube);
        // Meta's name on Meta's own network: the same company.
        let a = con("graph.facebook.com", &[], 32934);
        assert!(a.infraestructura.unwrap().misma_empresa);
    }

    #[test]
    fn probable_needs_an_own_network_and_the_base_to_agree() {
        let r = red_de_asn(32934).unwrap();
        assert_eq!(r.tipo, TipoRed::Propia);
        let a = con("x9-unknown.example", &[], 32934);
        if base_coincide(r) {
            assert_eq!(a.nivel, Nivel::Probable);
            assert_eq!(a.empresa, Some("Meta"));
            assert!(a.pruebas.iter().any(|p| p.fuente == Fuente::ListaRedes));
            assert!(a.pruebas.iter().any(|p| p.fuente == Fuente::BaseRed));
        } else {
            // A base that does not name the network as Meta's leaves it unidentified.
            assert_eq!(a.nivel, Nivel::SinIdentificar);
        }
        // A mixed network (Google's main one also hosts customers) never makes a name probable.
        assert_eq!(red_de_asn(15169).unwrap().tipo, TipoRed::Mixta);
        let a = con("x9-unknown.example", &[], 15169);
        assert_eq!(a.nivel, Nivel::SinIdentificar);
        assert_eq!(a.empresa, None);
        assert_eq!(a.operador_red.as_deref(), Some("Google"));
        // Nor does an access provider, nor a cloud.
        for asn in [10620, 16509, 13335] {
            let a = con("x9-unknown.example", &[], asn);
            assert_eq!(a.nivel, Nivel::SinIdentificar, "AS{asn}");
            assert_eq!(a.empresa, None, "AS{asn}");
            assert!(a.operador_red.is_some(), "AS{asn}");
        }
    }

    #[test]
    fn unidentified_shows_the_network_operator_or_nothing_invented() {
        let a = con("x9-unknown.example", &[], 0);
        assert_eq!(a.nivel, Nivel::SinIdentificar);
        assert_eq!(a.operador_red, None);
        assert!(a.pruebas.is_empty());
        assert!(a.infraestructura.is_none());
        // A network no list classifies: the base's own name, said as unknown kind.
        if let Some(n) = nombre_de_red(262_186) {
            let a = con("x9-unknown.example", &[], 262_186);
            let i = a.infraestructura.unwrap();
            assert_eq!(i.tipo, TipoRed::Desconocido);
            assert_eq!(i.operador, n.trim());
            assert_eq!(i.prueba.fuente, Fuente::BaseRed);
            assert_eq!(i.prueba.fecha, fuente_red().fecha);
        }
        // Local names are not destinations.
        let a = de("15.1.168.192.in-addr.arpa");
        assert!(a.local);
        assert_eq!(a.nivel, Nivel::SinIdentificar);
    }

    #[test]
    fn every_identified_or_probable_answer_has_a_dated_proof() {
        let casos = [
            con("graph.facebook.com", &[], 32934),
            con("metricas.tienda-ejemplo.com", &["x.sc.omtrdc.net"], 0),
            con("x9-unknown.example", &[], 32934),
            de("trc.taboola.com"),
            de("api.anthropic.com"),
        ];
        for a in &casos {
            if a.nivel != Nivel::SinIdentificar {
                assert!(a.empresa.is_some(), "{a:?}");
                assert!(!a.pruebas.is_empty(), "{a:?}");
                for p in &a.pruebas {
                    assert!(p.fecha.len() == 10 || p.fecha.len() == 7, "{p:?}");
                    assert!(p.detalle.parse::<std::net::IpAddr>().is_err(), "{p:?}");
                }
            }
        }
    }

    #[test]
    fn coverage_counts_queries_and_leaves_local_names_out() {
        let id = de("graph.facebook.com");
        let sin = con("x9-unknown.example", &[], 0);
        let local = de("_dns-sd._udp.local");
        let c = cobertura([(&id, 3), (&sin, 1), (&local, 50)]);
        assert_eq!(c.consultas, 4);
        assert_eq!(c.identificada, 3);
        assert_eq!(c.sin_identificar, 1);
        assert_eq!(c.pct_identificada, 75);
        assert_eq!(c.pct_sin_identificar, 25);
        assert_eq!(cobertura([]).pct_identificada, 0);
    }

    #[test]
    fn the_chain_keeps_sites_only() {
        let chain: Vec<String> = [
            "www.tienda.com.edgekey.net",
            "e12.a.akamaiedge.net",
            "d1abc.cloudfront.net",
            "93.184.216.34",
            "cdn.tienda.com",
            "e13.a.akamaiedge.net",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let s = sitios_de_cadena("www.tienda.com", &chain, 6);
        assert_eq!(s, vec!["edgekey.net", "akamaiedge.net", "cloudfront.net"]);
        assert_eq!(sitios_de_cadena("www.tienda.com", &chain, 1).len(), 1);
    }

    /// The measured «% identificado» on a sample of everyday names from everywhere, written before
    /// the world list grew (crates/lists/tests/data/muestra-mundo.txt). It was 66 % with 1.0.44's
    /// base; it must not fall back.
    #[test]
    fn the_world_sample_is_mostly_identified() {
        let muestra = include_str!("../tests/data/muestra-mundo.txt");
        let nombres: Vec<&str> = muestra
            .lines()
            .map(|l| l.split('#').next().unwrap_or("").trim())
            .filter(|l| !l.is_empty())
            .collect();
        let atribs: Vec<Atribucion> = nombres.iter().map(|n| de(n)).collect();
        let c = cobertura(atribs.iter().map(|a| (a, 1)));
        assert!(c.consultas > 500, "{c:?}");
        assert!(c.pct_identificada >= 95, "{c:?}");
    }

    #[test]
    fn the_world_list_is_well_formed() {
        let mut vistos = std::collections::HashSet::new();
        for (_, d) in crate::parse_guardiana(EMPRESAS) {
            vistos.insert(d);
        }
        let lineas: Vec<&str> = EMPRESAS_MUNDO.lines().collect();
        let mut seccion: Option<&str> = None;
        let mut con_dominios = true;
        for (i, l) in lineas.iter().enumerate() {
            let l = l.split('#').next().unwrap_or("").trim();
            if l.is_empty() {
                continue;
            }
            if let Some(h) = l.strip_prefix('@') {
                assert!(con_dominios, "{seccion:?}: sección vacía");
                con_dominios = false;
                seccion = Some(h);
                if let Some(n) = h.strip_prefix('=') {
                    assert!(
                        crate::company_record(n).is_some(),
                        "@={n}: no es una empresa de las listas"
                    );
                } else {
                    let f: Vec<&str> = h.split('|').map(str::trim).collect();
                    assert!(
                        crate::parse_guardiana(EMPRESAS).all(|(s, _)| s.map(|s| s
                            .split('|')
                            .next()
                            .unwrap_or(s)
                            .trim())
                            != Some(f[0])),
                        "{}: ya está en empresas.txt, va como @=",
                        f[0]
                    );
                    if let Some(p) = f.get(1).filter(|p| !p.is_empty()) {
                        assert!(
                            p.len() == 2 && p.chars().all(|c| c.is_ascii_uppercase()),
                            "{h}"
                        );
                    }
                    if f.get(3).is_some_and(|g| !g.is_empty()) {
                        let fuente = i
                            .checked_sub(1)
                            .and_then(|j| lineas.get(j))
                            .is_some_and(|p| p.starts_with("# Grupo: ") && p.contains("http"));
                        assert!(fuente, "{h}: un grupo lleva su fuente encima");
                    }
                }
                continue;
            }
            con_dominios = true;
            assert!(crate::parse::looks_like_domain(l), "{l}");
            assert!(vistos.insert(l), "{l}: dos veces entre los dos archivos");
        }
        assert!(con_dominios, "la última sección está vacía");
        assert!(edicion_de(EMPRESAS_MUNDO).is_some());
        assert!(edicion_de(REDES).is_some());
    }

    #[test]
    fn the_networks_list_is_well_formed_and_agrees_with_the_bundled_base() {
        let todas = todas_las_redes();
        assert!(todas.len() > 300, "{}", todas.len());
        let mut nombres_ok = 0;
        for r in &todas {
            assert!(!r.senas.is_empty(), "AS{}", r.asn);
            if let Some(e) = r.empresa {
                assert!(crate::company_record(e).is_some(), "AS{}: {e}", r.asn);
            }
            // With the real base, every network the base names agrees with the list's words: the
            // list can not say of a network what the base does not.
            if !fuente_red().muestra && nombre_de_red(r.asn).is_some() {
                assert!(
                    base_coincide(r),
                    "AS{} {}: {:?}",
                    r.asn,
                    r.operador,
                    nombre_de_red(r.asn)
                );
                nombres_ok += 1;
            }
        }
        if !fuente_red().muestra {
            assert!(nombres_ok > 300, "{nombres_ok}");
        }
        // A network written twice would mean two answers for one number.
        let mut vistos = std::collections::HashSet::new();
        for l in REDES.lines() {
            let l = l.split('#').next().unwrap_or("").trim();
            if let Some(n) = l.strip_prefix("AS") {
                assert!(vistos.insert(n.to_owned()), "AS{n} dos veces");
            }
        }
        // The two dates the attribution cites are the ones the lists write.
        assert!(CORREDORES.contains("8 de octubre de 2026"));
        assert!(crate::antivirus::ANTIVIRUS.contains("13 oct 2026"));
    }
}
