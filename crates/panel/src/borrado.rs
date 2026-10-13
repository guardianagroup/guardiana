//! «Haz que te borren» (decision 204): help the person ask a data broker to delete them, with
//! the law of their country, what the house's own DNS extract saw of that company, and a clock
//! for the legal deadline.
//!
//! What it does and, as importantly, what it does not:
//! - The letter is put together here from the texts in `i18n/*.json` and the tables below. The
//!   person's name and e-mail never reach this module: the letter carries the markers
//!   [`MARCA_NOMBRE`] and [`MARCA_CORREO`] and the page fills them in, in the browser. GUARDIANA
//!   sends nothing: the person copies the text, opens their own mail client or saves a `.txt`.
//! - The evidence is what the extract says the house asked for: «your devices asked N times for
//!   names of X». Never «X has your data»; only X can answer that, which is what the letter asks.
//!   A device whose owner did not share its detail is left out entirely (`Ledger::events`).
//! - The clock keeps only which company, which law and the day the person says they sent it, in
//!   the settings table. No name, no address, no letter.
//!
//! The jurisdictions are data with their sources (see `docs/BORRADO.md`); every sentence that
//! describes them lives in the three language files.

// Handlers return a full HTTP `Response` as their error, as axum intends (same as `api.rs`).
#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, OnceLock};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use guardiana_core::i18n::{self, Texts};
use guardiana_core::time::now_ms;
use guardiana_core::{NameSpan, SELF_DEVICE_ID};
use serde::{Deserialize, Serialize};

use crate::api::{with_ledger, ApiResult};
use crate::auth::{Lang, Session, Zona};
use crate::AppState;

/// Placeholder the page replaces with the person's name, in the browser.
pub(crate) const MARCA_NOMBRE: &str = "[[NOMBRE]]";
/// Placeholder the page replaces with the person's e-mail, in the browser.
pub(crate) const MARCA_CORREO: &str = "[[CORREO]]";
/// The setting that keeps the clock: a JSON list of [`Envio`].
pub(crate) const SETTING_ENVIOS: &str = "borrado_envios";

// ----- jurisdictions ------------------------------------------------------------------------

/// How a deadline is counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Unidad {
    /// Calendar days.
    Dias,
    /// Working days, Monday to Friday. Public holidays are not known here, so the real deadline
    /// may be a few days later, and the page says so.
    DiasHabiles,
    /// Calendar months (the GDPR's «one month»).
    Meses,
}

/// A deadline: `n` units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Plazo {
    pub(crate) n: u32,
    pub(crate) unidad: Unidad,
}

const fn dias(n: u32) -> Option<Plazo> {
    Some(Plazo {
        n,
        unidad: Unidad::Dias,
    })
}

const fn habiles(n: u32) -> Option<Plazo> {
    Some(Plazo {
        n,
        unidad: Unidad::DiasHabiles,
    })
}

const fn meses(n: u32) -> Option<Plazo> {
    Some(Plazo {
        n,
        unidad: Unidad::Meses,
    })
}

/// What a letter asks for: that the company delete the person's data, or that it say what data
/// it holds (the right of access, which every law here gives in some form).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Tipo {
    #[default]
    Borrado,
    Acceso,
}

impl Tipo {
    pub(crate) fn clave(self) -> &'static str {
        match self {
            Tipo::Borrado => "borrado",
            Tipo::Acceso => "acceso",
        }
    }
}

/// What the law of a place gives the person.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Derecho {
    /// A right to have personal data deleted (with the law's own conditions).
    Borrado,
    /// No general right to erasure: access, correction, withdrawing consent, or deletion only
    /// in narrow cases. The letter asks for those, and the page says it plainly.
    Limitado,
    /// No privacy law that applies to private companies in general: the letter is a request,
    /// not a legal demand.
    Ninguno,
}

/// One jurisdiction. The sentences about it are in the language files, under
/// `borrado_jur_<código>` (its name), `borrado_ley_`, `borrado_articulo_`, `borrado_plazo_`,
/// `borrado_nota_` and `borrado_queja_`.
#[derive(Debug)]
pub(crate) struct Jurisdiccion {
    /// Panel code, lower case (`es`, `us_ca`).
    pub(crate) codigo: &'static str,
    /// The language a letter there is written in by default (`es`, `en`, `pt`). Quebec gets
    /// English: the panel has no French, and the page says so.
    pub(crate) idioma: &'static str,
    /// Browser regions (`navigator.language`, `es-CO` → `CO`) that default to it.
    pub(crate) regiones: &'static [&'static str],
    pub(crate) derecho: Derecho,
    /// The legal deadline to answer, when there is one.
    pub(crate) plazo: Option<Plazo>,
    /// The extension the law allows, when it does (added after the deadline).
    pub(crate) prorroga: Option<Plazo>,
    /// The deadline to answer a request of access (what data it holds), when the law fixes one
    /// and it was checked; `None` when it is not known, and the clock says so. Written for every
    /// place, even where it is the same as for deletion, so the table is explicit.
    pub(crate) plazo_acceso: Option<Plazo>,
    pub(crate) prorroga_acceso: Option<Plazo>,
    /// The deadline is a reference, not fixed by the law (Brazil, Australia, other US states):
    /// the clock still counts it, and says so.
    pub(crate) referencia: bool,
    /// Who to complain to, as that authority names itself.
    pub(crate) autoridad: Option<&'static str>,
    pub(crate) autoridad_url: Option<&'static str>,
    /// Where each fact of this row was checked (docs/BORRADO.md has the full table).
    pub(crate) fuentes: &'static [&'static str],
}

const GDPR: &str = "https://eur-lex.europa.eu/eli/reg/2016/679/oj";
const GDPR_PLAZO: &str = "https://www.legislation.gov.uk/eur/2016/679/article/12/adopted/data.htm";

/// Every jurisdiction the page knows, in the order the selector shows them.
pub(crate) static JURISDICCIONES: &[Jurisdiccion] = &[
    Jurisdiccion {
        codigo: "es",
        idioma: "es",
        regiones: &["ES"],
        derecho: Derecho::Borrado,
        plazo: meses(1),
        prorroga: meses(2),
        plazo_acceso: meses(1),
        prorroga_acceso: meses(2),
        referencia: false,
        autoridad: Some("Agencia Española de Protección de Datos (AEPD)"),
        autoridad_url: Some(
            "https://sedeagpd.gob.es/sede-electronica-web/vistas/infoSede/tramitesCiudadano.jsf",
        ),
        fuentes: &[
            GDPR,
            GDPR_PLAZO,
            "https://web.guardiacivil.es/export/sites/guardiaCivil/documentos/pdfs/registro_actividades_tratamiento/formularios/EJERCICIO-DERECHO-DE-SUPRESION_.pdf",
            "https://caib.es/sites/protecciodedadespersonals/es/camo_puedo_reclamar/",
        ],
    },
    Jurisdiccion {
        codigo: "pt",
        idioma: "pt",
        regiones: &["PT"],
        derecho: Derecho::Borrado,
        plazo: meses(1),
        prorroga: meses(2),
        plazo_acceso: meses(1),
        prorroga_acceso: meses(2),
        referencia: false,
        autoridad: Some("Comissão Nacional de Proteção de Dados (CNPD)"),
        autoridad_url: Some("https://www.cnpd.pt"),
        fuentes: &[
            GDPR,
            GDPR_PLAZO,
            "https://www.legiscope.com/blog/cnpd-guia-autoridade.html",
        ],
    },
    Jurisdiccion {
        codigo: "ie",
        idioma: "en",
        regiones: &["IE"],
        derecho: Derecho::Borrado,
        plazo: meses(1),
        prorroga: meses(2),
        plazo_acceso: meses(1),
        prorroga_acceso: meses(2),
        referencia: false,
        autoridad: Some("Data Protection Commission (DPC)"),
        autoridad_url: Some("https://forms.dataprotection.ie/contact"),
        fuentes: &[
            GDPR,
            GDPR_PLAZO,
            "https://www.dataprotection.ie/en/faqs/initial-contact-dpc/making-complaint-dpc",
        ],
    },
    Jurisdiccion {
        codigo: "ue",
        idioma: "en",
        regiones: &[
            "AT", "BE", "BG", "HR", "CY", "CZ", "DK", "EE", "FI", "FR", "DE", "GR", "HU", "IT",
            "LV", "LT", "LU", "MT", "NL", "PL", "RO", "SK", "SI", "SE", "IS", "LI", "NO",
        ],
        derecho: Derecho::Borrado,
        plazo: meses(1),
        prorroga: meses(2),
        plazo_acceso: meses(1),
        prorroga_acceso: meses(2),
        referencia: false,
        autoridad: None,
        autoridad_url: Some("https://www.edpb.europa.eu/about-edpb/about-edpb/members_en"),
        fuentes: &[GDPR, GDPR_PLAZO],
    },
    Jurisdiccion {
        codigo: "gb",
        idioma: "en",
        regiones: &["GB", "UK"],
        derecho: Derecho::Borrado,
        plazo: meses(1),
        prorroga: meses(2),
        plazo_acceso: meses(1),
        prorroga_acceso: meses(2),
        referencia: false,
        autoridad: Some("Information Commissioner's Office (ICO)"),
        autoridad_url: Some("https://ico.org.uk/make-a-complaint/"),
        fuentes: &[
            "https://ico.org.uk/for-the-public/your-right-to-get-your-data-deleted",
            GDPR_PLAZO,
        ],
    },
    Jurisdiccion {
        codigo: "us_ca",
        idioma: "en",
        regiones: &[],
        derecho: Derecho::Borrado,
        plazo: dias(45),
        prorroga: dias(45),
        plazo_acceso: dias(45),
        prorroga_acceso: dias(45),
        referencia: false,
        autoridad: Some("California Privacy Protection Agency (CalPrivacy)"),
        autoridad_url: Some("https://privacy.ca.gov/submit-a-complaint/"),
        fuentes: &[
            "https://privacy.gtlaw.com/consumers-right-to-delete-personal-information/",
            "https://california.public.law/codes/civil_code_section_1798.130",
            "https://privacy.ca.gov/drop/",
            "https://privacy.ca.gov/data-brokers",
        ],
    },
    Jurisdiccion {
        codigo: "us_va",
        idioma: "en",
        regiones: &[],
        derecho: Derecho::Borrado,
        plazo: dias(45),
        prorroga: dias(45),
        plazo_acceso: dias(45),
        prorroga_acceso: dias(45),
        referencia: false,
        autoridad: Some("Virginia Attorney General"),
        autoridad_url: Some("https://www.oag.state.va.us"),
        fuentes: &["https://law.lis.virginia.gov/vacode/59.1-577"],
    },
    Jurisdiccion {
        codigo: "us_co",
        idioma: "en",
        regiones: &[],
        derecho: Derecho::Borrado,
        plazo: dias(45),
        prorroga: dias(45),
        plazo_acceso: dias(45),
        prorroga_acceso: dias(45),
        referencia: false,
        autoridad: Some("Colorado Attorney General"),
        autoridad_url: Some("https://complaints.coag.gov/s/data-privacy"),
        fuentes: &[
            "https://privacy.gtlaw.com/colorado/",
            "https://coag.gov/resources/data-privacy/",
        ],
    },
    Jurisdiccion {
        codigo: "us_ct",
        idioma: "en",
        regiones: &[],
        derecho: Derecho::Borrado,
        plazo: dias(45),
        prorroga: dias(45),
        plazo_acceso: dias(45),
        prorroga_acceso: dias(45),
        referencia: false,
        autoridad: Some("Connecticut Attorney General"),
        autoridad_url: Some("https://portal.ct.gov/ag/common/complaint-form-landing-page"),
        fuentes: &["https://www.recordinglaw.com/us-laws/data-privacy-laws/connecticut-data-privacy-laws/ctdpa-consumer-rights/"],
    },
    Jurisdiccion {
        codigo: "us_ut",
        idioma: "en",
        regiones: &[],
        derecho: Derecho::Borrado,
        plazo: dias(45),
        prorroga: dias(45),
        plazo_acceso: dias(45),
        prorroga_acceso: dias(45),
        referencia: false,
        autoridad: Some("Utah Division of Consumer Protection"),
        autoridad_url: Some("https://dcp.utah.gov"),
        fuentes: &[
            "https://le.utah.gov/xcode/Title13/Chapter61/C13-61-S201_2026070120250507.html",
            "https://privacy.gtlaw.com/?p=1399",
        ],
    },
    Jurisdiccion {
        codigo: "us_tx",
        idioma: "en",
        regiones: &[],
        derecho: Derecho::Borrado,
        plazo: dias(45),
        prorroga: dias(45),
        plazo_acceso: dias(45),
        prorroga_acceso: dias(45),
        referencia: false,
        autoridad: Some("Texas Attorney General"),
        autoridad_url: Some(
            "https://www.texasattorneygeneral.gov/consumer-protection/file-consumer-complaint",
        ),
        fuentes: &[
            "https://texas.public.law/statutes/tex._bus._&_com._code_section_541.051",
            "https://texas.public.law/statutes/tex._bus._&_com._code_section_541.052",
        ],
    },
    Jurisdiccion {
        codigo: "us_or",
        idioma: "en",
        regiones: &[],
        derecho: Derecho::Borrado,
        plazo: dias(45),
        prorroga: dias(45),
        plazo_acceso: dias(45),
        prorroga_acceso: dias(45),
        referencia: false,
        autoridad: Some("Oregon Department of Justice"),
        autoridad_url: Some("https://doj.state.or.us/consumer-protection/id-theft-data-breaches"),
        fuentes: &[
            "https://oregon.public.law/statutes/ors_646a.574",
            "https://oregon.public.law/statutes/ors_646a.576",
        ],
    },
    Jurisdiccion {
        codigo: "us_otros",
        idioma: "en",
        regiones: &[],
        derecho: Derecho::Borrado,
        plazo: dias(45),
        prorroga: dias(45),
        plazo_acceso: dias(45),
        prorroga_acceso: dias(45),
        referencia: true,
        autoridad: None,
        autoridad_url: None,
        fuentes: &["https://www.clym.io/blog/us-privacy-law-comparison-map"],
    },
    Jurisdiccion {
        codigo: "us",
        idioma: "en",
        regiones: &["US"],
        derecho: Derecho::Ninguno,
        plazo: None,
        prorroga: None,
        plazo_acceso: None,
        prorroga_acceso: None,
        referencia: false,
        autoridad: None,
        autoridad_url: None,
        fuentes: &["https://www.clym.io/blog/us-privacy-law-comparison-map"],
    },
    Jurisdiccion {
        codigo: "ca",
        idioma: "en",
        regiones: &["CA"],
        derecho: Derecho::Limitado,
        plazo: dias(30),
        prorroga: dias(30),
        plazo_acceso: dias(30),
        prorroga_acceso: dias(30),
        referencia: false,
        autoridad: Some("Office of the Privacy Commissioner of Canada (OPC)"),
        autoridad_url: Some("https://www.priv.gc.ca/en/report-a-concern/file-a-formal-privacy-complaint/file-a-complaint-about-a-business"),
        fuentes: &[
            "https://laws-lois.justice.gc.ca/eng/acts/P-8.6/section-8.html",
            "https://priv.gc.ca/en/opc-actions-and-decisions/investigations/investigations-into-businesses/2017/pipeda-2017-005",
            "https://www.priv.gc.ca/en/opc-actions-and-decisions/investigations/investigations-into-businesses/2026/pipeda-2026-001/",
        ],
    },
    Jurisdiccion {
        codigo: "ca_qc",
        idioma: "en",
        regiones: &[],
        derecho: Derecho::Limitado,
        plazo: dias(30),
        prorroga: None,
        plazo_acceso: dias(30),
        prorroga_acceso: None,
        referencia: false,
        autoridad: Some("Commission d'accès à l'information du Québec (CAI)"),
        autoridad_url: Some("https://www.cai.gouv.qc.ca"),
        fuentes: &[
            "https://www.legisquebec.gouv.qc.ca/en/document/cs/p-39.1",
            "https://www.blg.com/en/insights/2021/05/the-right-to-erasure-of-personal-information",
        ],
    },
    Jurisdiccion {
        codigo: "au",
        idioma: "en",
        regiones: &["AU"],
        derecho: Derecho::Limitado,
        plazo: dias(30),
        prorroga: None,
        plazo_acceso: dias(30),
        prorroga_acceso: None,
        referencia: true,
        autoridad: Some("Office of the Australian Information Commissioner (OAIC)"),
        autoridad_url: Some("https://www.oaic.gov.au"),
        fuentes: &[
            "https://oaic.gov.au/privacy/your-privacy-rights/your-personal-information/correct-your-personal-information",
            "https://ia.acs.org.au/article/2026/australians-to-gain--right-to-erasure--under-privacy-reforms.html",
        ],
    },
    Jurisdiccion {
        codigo: "nz",
        idioma: "en",
        regiones: &["NZ"],
        derecho: Derecho::Limitado,
        plazo: habiles(20),
        prorroga: None,
        plazo_acceso: habiles(20),
        prorroga_acceso: None,
        referencia: false,
        autoridad: Some("Office of the Privacy Commissioner (OPC)"),
        autoridad_url: Some("https://www.privacy.org.nz"),
        fuentes: &["https://www.privacy.org.nz/resources-and-learning/knowledge-base/view/235/"],
    },
    Jurisdiccion {
        codigo: "br",
        idioma: "pt",
        regiones: &["BR"],
        derecho: Derecho::Borrado,
        plazo: dias(15),
        prorroga: None,
        plazo_acceso: dias(15),
        prorroga_acceso: None,
        referencia: true,
        autoridad: Some("Autoridade Nacional de Proteção de Dados (ANPD)"),
        autoridad_url: Some("https://www.gov.br/pt-br/servicos/abrir-requerimento-relacionado-a-lgpd"),
        fuentes: &["https://planalto.gov.br/ccivil_03/_ato2015-2018/2018/lei/L13709.htm"],
    },
    Jurisdiccion {
        codigo: "mx",
        idioma: "es",
        regiones: &["MX"],
        derecho: Derecho::Borrado,
        plazo: habiles(20),
        prorroga: habiles(20),
        plazo_acceso: habiles(20),
        prorroga_acceso: habiles(20),
        referencia: false,
        autoridad: Some("Secretaría Anticorrupción y Buen Gobierno"),
        autoridad_url: Some("https://www.anticorrupcionybg.gob.mx"),
        fuentes: &["https://www.diputados.gob.mx/LeyesBiblio/pdf/LFPDPPP.pdf"],
    },
    Jurisdiccion {
        codigo: "co",
        idioma: "es",
        regiones: &["CO"],
        derecho: Derecho::Borrado,
        plazo: habiles(15),
        prorroga: habiles(8),
        plazo_acceso: habiles(10),
        prorroga_acceso: habiles(5),
        referencia: false,
        autoridad: Some("Superintendencia de Industria y Comercio (SIC)"),
        autoridad_url: Some("https://www.sic.gov.co"),
        fuentes: &["https://barranquilla.gov.co/documento/ley-1581-del-2012"],
    },
    Jurisdiccion {
        codigo: "ar",
        idioma: "es",
        regiones: &["AR"],
        derecho: Derecho::Borrado,
        plazo: habiles(5),
        prorroga: None,
        plazo_acceso: dias(10),
        prorroga_acceso: None,
        referencia: false,
        autoridad: Some("Agencia de Acceso a la Información Pública (AAIP)"),
        autoridad_url: Some("https://www.argentina.gob.ar/aaip"),
        fuentes: &["https://www.argentina.gob.ar/normativa/nacional/64790/texto"],
    },
    Jurisdiccion {
        codigo: "cl",
        idioma: "es",
        regiones: &["CL"],
        derecho: Derecho::Borrado,
        plazo: habiles(2),
        prorroga: None,
        plazo_acceso: habiles(2),
        prorroga_acceso: None,
        referencia: false,
        autoridad: None,
        autoridad_url: None,
        fuentes: &[
            "https://www.registrosocial.gob.cl/docs/LEY-19628_28-AGO-1999.pdf",
            "https://www.diarioconstitucional.cl/2026/06/12/la-ley-21-719-entra-en-vigor-el-1-de-diciembre-y-expone-vacios-en-regulacion-de-pequenas-empresas/",
        ],
    },
    Jurisdiccion {
        codigo: "pe",
        idioma: "es",
        regiones: &["PE"],
        derecho: Derecho::Borrado,
        plazo: habiles(10),
        prorroga: habiles(10),
        plazo_acceso: None,
        prorroga_acceso: None,
        referencia: false,
        autoridad: Some("Autoridad Nacional de Protección de Datos Personales (ANPD)"),
        autoridad_url: Some("https://www.gob.pe/anpd"),
        fuentes: &[
            "https://www.congreso.gob.pe/Docs/DGP/DIDP/files/ds_016-2024-jus.pdf",
            "https://cdn.www.gob.pe/uploads/document/file/3512953/EXP.%20014-2022%20-%20RD%202255-2022-DPDP.pdf.pdf",
        ],
    },
    Jurisdiccion {
        codigo: "tr",
        idioma: "en",
        regiones: &["TR"],
        derecho: Derecho::Borrado,
        plazo: dias(30),
        prorroga: None,
        plazo_acceso: dias(30),
        prorroga_acceso: None,
        referencia: false,
        autoridad: Some("Personal Data Protection Authority (KVKK)"),
        autoridad_url: Some("https://www.kvkk.gov.tr/"),
        fuentes: &[
            "https://www.kvkk.gov.tr/Icerik/6649/Personal-Data-Protection-Law",
        ],
    },
    Jurisdiccion {
        codigo: "jp",
        idioma: "en",
        regiones: &["JP"],
        derecho: Derecho::Limitado,
        plazo: dias(14),
        prorroga: None,
        plazo_acceso: dias(14),
        prorroga_acceso: None,
        referencia: true,
        autoridad: Some("Personal Information Protection Commission (PPC)"),
        autoridad_url: Some("https://www.ppc.go.jp/en/"),
        fuentes: &[
            "https://www.japaneselawtranslation.go.jp/en/laws/view/4241",
        ],
    },
    Jurisdiccion {
        codigo: "kr",
        idioma: "en",
        regiones: &["KR"],
        derecho: Derecho::Borrado,
        plazo: dias(10),
        prorroga: None,
        plazo_acceso: dias(10),
        prorroga_acceso: None,
        referencia: false,
        autoridad: Some("Personal Information Protection Commission (PIPC)"),
        autoridad_url: Some("https://www.pipc.go.kr/eng"),
        fuentes: &[
            "https://elaw.klri.re.kr/eng_service/lawView.do?hseq=62389&lang=ENG",
            "https://elaw.klri.re.kr/eng_service/lawView.do?hseq=63755&lang=ENG",
        ],
    },
    Jurisdiccion {
        codigo: "cn",
        idioma: "en",
        regiones: &["CN"],
        derecho: Derecho::Borrado,
        plazo: None,
        prorroga: None,
        plazo_acceso: None,
        prorroga_acceso: None,
        referencia: false,
        autoridad: Some("Cyberspace Administration of China (CAC)"),
        autoridad_url: Some("https://www.cac.gov.cn"),
        fuentes: &[
            "http://www.npc.gov.cn/c2/c30834/202108/t20210820_313088.html",
        ],
    },
    Jurisdiccion {
        codigo: "hk",
        idioma: "en",
        regiones: &["HK"],
        derecho: Derecho::Limitado,
        plazo: dias(40),
        prorroga: None,
        plazo_acceso: dias(40),
        prorroga_acceso: None,
        referencia: false,
        autoridad: Some("Office of the Privacy Commissioner for Personal Data (PCPD)"),
        autoridad_url: Some("https://www.pcpd.org.hk"),
        fuentes: &[
            "https://www.elegislation.gov.hk/hk/cap486!en",
        ],
    },
    Jurisdiccion {
        codigo: "tw",
        idioma: "en",
        regiones: &["TW"],
        derecho: Derecho::Borrado,
        plazo: dias(30),
        prorroga: dias(30),
        plazo_acceso: dias(15),
        prorroga_acceso: dias(15),
        referencia: false,
        autoridad: None,
        autoridad_url: None,
        fuentes: &[
            "https://law.moj.gov.tw/ENG/LawClass/LawAll.aspx?pcode=I0050021",
        ],
    },
    Jurisdiccion {
        codigo: "sg",
        idioma: "en",
        regiones: &["SG"],
        derecho: Derecho::Limitado,
        plazo: dias(30),
        prorroga: None,
        plazo_acceso: dias(30),
        prorroga_acceso: None,
        referencia: true,
        autoridad: Some("Personal Data Protection Commission (PDPC)"),
        autoridad_url: Some("https://www.pdpc.gov.sg"),
        fuentes: &[
            "https://sso.agc.gov.sg/Act/PDPA2012",
            "https://sso.agc.gov.sg/SL/PDPA2012-S63-2021?WholeDoc=1",
        ],
    },
    Jurisdiccion {
        codigo: "th",
        idioma: "en",
        regiones: &["TH"],
        derecho: Derecho::Borrado,
        plazo: None,
        prorroga: None,
        plazo_acceso: dias(30),
        prorroga_acceso: None,
        referencia: false,
        autoridad: Some("Office of the Personal Data Protection Committee (PDPC)"),
        autoridad_url: Some("https://www.pdpc.or.th"),
        fuentes: &[
            "https://www.mdes.go.th/uploads/tinymce/source/%E0%B8%AA%E0%B8%84%E0%B8%AA/Personal%20Data%20Protection%20Act%202019.pdf",
        ],
    },
    Jurisdiccion {
        codigo: "my",
        idioma: "en",
        regiones: &["MY"],
        derecho: Derecho::Limitado,
        plazo: dias(21),
        prorroga: None,
        plazo_acceso: dias(21),
        prorroga_acceso: dias(14),
        referencia: false,
        autoridad: Some("Personal Data Protection Commissioner (JPDP)"),
        autoridad_url: Some("https://www.pdp.gov.my"),
        fuentes: &[
            "https://lom.agc.gov.my/ilims/upload/portal/akta/outputaktap/3301061_BI/Act%20709%20(Reprint%202026).pdf",
        ],
    },
    Jurisdiccion {
        codigo: "ph",
        idioma: "en",
        regiones: &["PH"],
        derecho: Derecho::Borrado,
        plazo: dias(15),
        prorroga: None,
        plazo_acceso: dias(15),
        prorroga_acceso: None,
        referencia: true,
        autoridad: Some("National Privacy Commission (NPC)"),
        autoridad_url: Some("https://privacy.gov.ph"),
        fuentes: &[
            "https://privacy.gov.ph/data-privacy-act/",
            "https://privacy.gov.ph/wp-content/uploads/2024/03/2021-Rules-of-Procedure-of-the-NPC-As-Amended.pdf",
        ],
    },
    Jurisdiccion {
        codigo: "id",
        idioma: "en",
        regiones: &["ID"],
        derecho: Derecho::Borrado,
        plazo: None,
        prorroga: None,
        plazo_acceso: dias(3),
        prorroga_acceso: None,
        referencia: false,
        autoridad: None,
        autoridad_url: None,
        fuentes: &[
            "https://peraturan.bpk.go.id/Details/229798/uu-no-27-tahun-2022",
        ],
    },
    Jurisdiccion {
        codigo: "sa",
        idioma: "en",
        regiones: &["SA"],
        derecho: Derecho::Borrado,
        plazo: dias(30),
        prorroga: dias(30),
        plazo_acceso: dias(30),
        prorroga_acceso: dias(30),
        referencia: false,
        autoridad: Some("Saudi Data & AI Authority (SDAIA)"),
        autoridad_url: Some("https://sdaia.gov.sa"),
        fuentes: &[
            "https://sdaia.gov.sa/en/SDAIA/about/Documents/PersonalDataProtectionLaw.pdf",
            "https://sdaia.gov.sa/en/SDAIA/about/Documents/ImplementingRegulationPersonalDataProtectionLaw.pdf",
        ],
    },
];

/// A jurisdiction by its code.
pub(crate) fn jurisdiccion(codigo: &str) -> Option<&'static Jurisdiccion> {
    JURISDICCIONES.iter().find(|j| j.codigo == codigo)
}

/// A panel text of a jurisdiction, `borrado_<campo>_<código>`, owned.
pub(crate) fn texto_jur(t: &Texts, campo: &str, j: &Jurisdiccion) -> String {
    let k = format!("borrado_{campo}_{}", j.codigo);
    t.panel(&k).to_owned()
}

/// The text of a field for a request type: `borrado_<campo>_acceso_<código>` for access when
/// it is written, else the deletion one (the same deadline, worded once).
pub(crate) fn texto_jur_tipo(t: &Texts, campo: &str, j: &Jurisdiccion, tipo: Tipo) -> String {
    if tipo == Tipo::Acceso {
        let k = format!("borrado_{campo}_acceso_{}", j.codigo);
        if t.panel_has(&k) {
            return t.panel(&k).to_owned();
        }
    }
    texto_jur(t, campo, j)
}

impl Jurisdiccion {
    /// Whether the deadline counted for a request type is only a reference. Brazil's LGPD has no
    /// deadline in days for deletion, but fixes 15 days for the full statement of access
    /// (article 19, II; checked on 12 Oct 2026).
    pub(crate) fn es_referencia(&self, tipo: Tipo) -> bool {
        self.referencia && !(tipo == Tipo::Acceso && self.codigo == "br")
    }

    /// The deadline and the extension for a request type.
    pub(crate) fn plazos(&self, tipo: Tipo) -> (Option<Plazo>, Option<Plazo>) {
        match tipo {
            Tipo::Borrado => (self.plazo, self.prorroga),
            Tipo::Acceso => (self.plazo_acceso, self.prorroga_acceso),
        }
    }
}

// ----- the companies ------------------------------------------------------------------------

const DATOS_CORREDORES: &str = include_str!("../data/borrado-corredores.txt");

/// A company of `data/borrado-corredores.txt`.
#[derive(Debug, Clone, Default)]
pub(crate) struct Corredor {
    pub(crate) id: String,
    pub(crate) nombre: String,
    /// Two-letter country of the company that answers the letter, when checked.
    pub(crate) pais: Option<String>,
    /// `personas`, `datos`, `publicidad` or `credito`.
    pub(crate) tipo: String,
    /// The official page or form to ask for deletion.
    pub(crate) url: Option<String>,
    /// (address, jurisdictions it is given for; `*` for all).
    pub(crate) correos: Vec<(String, Vec<String>)>,
    /// `cerrado` or `incautado`, when the service is gone.
    pub(crate) estado: Option<String>,
    /// (URL, day checked).
    pub(crate) fuentes: Vec<(String, String)>,
    pub(crate) dominios: Vec<String>,
    /// Sections of `empresas.txt` whose names all count as this company's.
    pub(crate) empresas: Vec<String>,
}

impl Corredor {
    /// Whether a queried name belongs to this company.
    pub(crate) fn coincide(&self, qname: &str) -> bool {
        let q = qname.trim_end_matches('.').to_ascii_lowercase();
        if self
            .dominios
            .iter()
            .any(|d| q == *d || q.ends_with(&format!(".{d}")))
        {
            return true;
        }
        !self.empresas.is_empty()
            && guardiana_lists::company_of(&q).is_some_and(|e| self.empresas.iter().any(|x| x == e))
    }

    /// The address the company gives for this jurisdiction, if any.
    pub(crate) fn correo_para(&self, jur: &str) -> Option<&str> {
        self.correos
            .iter()
            .find(|(_, ambitos)| ambitos.iter().any(|a| a == "*" || a == jur))
            .map(|(c, _)| c.as_str())
    }
}

/// Reads the data file. Lines it does not understand are a test failure, not a silent skip.
pub(crate) fn parse_corredores(texto: &str) -> Result<Vec<Corredor>, String> {
    let mut out: Vec<Corredor> = Vec::new();
    for (i, linea) in texto.lines().enumerate() {
        let l = linea.split(" #").next().unwrap_or("").trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let err = |m: &str| format!("línea {}: {m}: {linea}", i + 1);
        if let Some(cab) = l.strip_prefix('@') {
            let partes: Vec<&str> = cab.split('|').collect();
            let [id, nombre, pais, tipo] = partes.as_slice() else {
                return Err(err("cabecera con cuatro campos"));
            };
            if !matches!(*tipo, "personas" | "datos" | "publicidad" | "credito") {
                return Err(err("tipo desconocido"));
            }
            if out.iter().any(|c| c.id == *id) {
                return Err(err("id repetido"));
            }
            out.push(Corredor {
                id: (*id).to_owned(),
                nombre: (*nombre).to_owned(),
                pais: (!pais.is_empty()).then(|| (*pais).to_owned()),
                tipo: (*tipo).to_owned(),
                ..Corredor::default()
            });
            continue;
        }
        let Some(c) = out.last_mut() else {
            return Err(err("línea antes de la primera cabecera"));
        };
        if let Some(u) = l.strip_prefix("url ") {
            if !u.starts_with("https://") || c.url.is_some() {
                return Err(err("url"));
            }
            c.url = Some(u.trim().to_owned());
        } else if let Some(resto) = l.strip_prefix("correo ") {
            let mut p = resto.split_whitespace();
            let (Some(correo), Some(ambitos)) = (p.next(), p.next()) else {
                return Err(err("correo sin ámbito"));
            };
            if !correo.contains('@') {
                return Err(err("correo"));
            }
            let ambitos: Vec<String> = ambitos.split(',').map(str::to_owned).collect();
            if !ambitos
                .iter()
                .all(|a| a == "*" || jurisdiccion(a).is_some())
            {
                return Err(err("ámbito desconocido"));
            }
            c.correos.push((correo.to_owned(), ambitos));
        } else if let Some(e) = l.strip_prefix("estado ") {
            if !matches!(e, "cerrado" | "incautado") {
                return Err(err("estado"));
            }
            c.estado = Some(e.to_owned());
        } else if let Some(resto) = l.strip_prefix("fuente ") {
            let mut p = resto.split_whitespace();
            let (Some(u), Some(f)) = (p.next(), p.next()) else {
                return Err(err("fuente sin fecha"));
            };
            if !u.starts_with("https://") || parse_fecha(f).is_none() {
                return Err(err("fuente"));
            }
            c.fuentes.push((u.to_owned(), f.to_owned()));
        } else if let Some(e) = l.strip_prefix('=') {
            c.empresas.push(e.trim().to_owned());
        } else if l.contains('.')
            && !l.contains(' ')
            && l.chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '.' || ch == '-')
        {
            c.dominios.push(l.to_owned());
        } else {
            return Err(err("no se entiende"));
        }
    }
    Ok(out)
}

/// The companies, read once.
pub(crate) fn corredores() -> &'static [Corredor] {
    static C: OnceLock<Vec<Corredor>> = OnceLock::new();
    C.get_or_init(|| parse_corredores(DATOS_CORREDORES).unwrap_or_default())
}

/// A company by id.
pub(crate) fn corredor(id: &str) -> Option<&'static Corredor> {
    corredores().iter().find(|c| c.id == id)
}

// ----- what the house saw -------------------------------------------------------------------

/// One name of a company the house asked for.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct NombreVisto {
    pub(crate) nombre: String,
    pub(crate) consultas: i64,
    pub(crate) cortadas: i64,
    pub(crate) primera: i64,
    pub(crate) ultima: i64,
}

/// Everything the household may read about one company, from the extract.
#[derive(Serialize, Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Evidencia {
    pub(crate) consultas: i64,
    pub(crate) cortadas: i64,
    pub(crate) primera: Option<i64>,
    pub(crate) ultima: Option<i64>,
    /// The names, most asked first.
    pub(crate) nombres: Vec<NombreVisto>,
    /// The devices that asked, as ids (this computer and devices that share their detail only).
    pub(crate) aparatos: Vec<String>,
}

/// Groups the (name, device) spans by company. A span whose name belongs to no company is
/// ignored; one that belongs to two (it should not happen) counts for the first in the file.
pub(crate) fn evidencias(spans: &[NameSpan], lista: &[Corredor]) -> HashMap<String, Evidencia> {
    let mut por: HashMap<String, (Evidencia, BTreeMap<String, NombreVisto>)> = HashMap::new();
    for s in spans {
        let Some(c) = lista.iter().find(|c| c.coincide(&s.qname)) else {
            continue;
        };
        let (ev, nombres) = por.entry(c.id.clone()).or_default();
        ev.consultas += s.queries;
        ev.cortadas += s.cut;
        ev.primera = Some(ev.primera.map_or(s.first_ts, |p| p.min(s.first_ts)));
        ev.ultima = Some(ev.ultima.map_or(s.last_ts, |u| u.max(s.last_ts)));
        if !ev.aparatos.contains(&s.device_id) {
            ev.aparatos.push(s.device_id.clone());
        }
        let n = nombres
            .entry(s.qname.clone())
            .or_insert_with(|| NombreVisto {
                nombre: s.qname.clone(),
                consultas: 0,
                cortadas: 0,
                primera: s.first_ts,
                ultima: s.last_ts,
            });
        n.consultas += s.queries;
        n.cortadas += s.cut;
        n.primera = n.primera.min(s.first_ts);
        n.ultima = n.ultima.max(s.last_ts);
    }
    por.into_iter()
        .map(|(id, (mut ev, nombres))| {
            ev.nombres = nombres.into_values().collect();
            ev.nombres
                .sort_by(|a, b| b.consultas.cmp(&a.consultas).then(a.nombre.cmp(&b.nombre)));
            // This computer first, then the others in a stable order.
            ev.aparatos
                .sort_by_key(|d| (d != SELF_DEVICE_ID, d.clone()));
            (id, ev)
        })
        .collect()
}

// ----- days ---------------------------------------------------------------------------------

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
fn dias_desde_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of a day number.
fn civil_desde_dias(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (
        y,
        u32::try_from(m).unwrap_or(1),
        u32::try_from(d).unwrap_or(1),
    )
}

fn dias_del_mes(y: i64, m: u32) -> u32 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// `AAAA-MM-DD` as a day number, `None` when it is not a real date.
pub(crate) fn parse_fecha(s: &str) -> Option<i64> {
    let mut p = s.split('-');
    let (Some(y), Some(m), Some(d), None) = (p.next(), p.next(), p.next(), p.next()) else {
        return None;
    };
    if y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let (y, m, d) = (
        y.parse::<i64>().ok()?,
        m.parse::<u32>().ok()?,
        d.parse::<u32>().ok()?,
    );
    if !(1..=12).contains(&m) || d == 0 || d > dias_del_mes(y, m) {
        return None;
    }
    Some(dias_desde_civil(y, m, d))
}

/// A day number as `AAAA-MM-DD`.
pub(crate) fn fecha_iso(z: i64) -> String {
    let (y, m, d) = civil_desde_dias(z);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Monday is 0.
fn dia_semana(z: i64) -> i64 {
    (z + 3).rem_euclid(7)
}

/// The day a deadline ends, counted from the day the letter was sent. Working days start on the
/// next day and skip Saturdays and Sundays; months keep the day of the month, or the last day of
/// a shorter month.
pub(crate) fn sumar(desde: i64, p: Plazo) -> i64 {
    match p.unidad {
        Unidad::Dias => desde + i64::from(p.n),
        Unidad::DiasHabiles => {
            let mut z = desde;
            let mut contados = 0;
            while contados < p.n {
                z += 1;
                if dia_semana(z) < 5 {
                    contados += 1;
                }
            }
            z
        }
        Unidad::Meses => {
            let (y, m, d) = civil_desde_dias(desde);
            let total = i64::from(m) - 1 + i64::from(p.n);
            let ny = y + total.div_euclid(12);
            let nm = u32::try_from(total.rem_euclid(12) + 1).unwrap_or(1);
            dias_desde_civil(ny, nm, d.min(dias_del_mes(ny, nm)))
        }
    }
}

/// A day number as the panel writes dates in this language.
pub(crate) fn fecha_texto(t: &Texts, z: i64) -> String {
    let (y, m, d) = civil_desde_dias(z);
    let mes = t
        .panel("fecha_meses")
        .split_whitespace()
        .nth(m as usize - 1)
        .map(str::to_owned);
    let dia = t
        .panel("fecha_dias")
        .split_whitespace()
        .nth(d as usize - 1)
        .map(str::to_owned);
    match (mes, dia) {
        (Some(nm), Some(nd)) => t
            .panel("fecha_formato")
            .replace("{dia}", &nd)
            .replace("{mes}", &nm)
            .replace("{ano}", &y.to_string()),
        _ => fecha_iso(z),
    }
}

/// The person's day, as a day number, from a time and their offset.
pub(crate) fn dia_de(ms: i64, zona_min: i64) -> i64 {
    (ms + zona_min * 60_000).div_euclid(guardiana_core::time::DAY_MS)
}

// ----- the letter ---------------------------------------------------------------------------

/// A letter, ready for the page to fill in the name and e-mail.
#[derive(Serialize, Debug, Clone)]
pub(crate) struct Carta {
    pub(crate) asunto: String,
    pub(crate) cuerpo: String,
    /// The address the company publishes for this jurisdiction, when it publishes one.
    pub(crate) para: Option<String>,
    /// Its official page or form.
    pub(crate) url: Option<String>,
    /// The language of the letter.
    pub(crate) idioma: &'static str,
    /// What the page writes where the name and the e-mail go while the inputs are empty
    /// («[tu nombre]»), in the letter's language.
    pub(crate) hueco_nombre: String,
    pub(crate) hueco_correo: String,
}

/// Puts the deletion letter together, in the language of `t`. `hoy` is a day number; `zona` the
/// person's offset in minutes, for the days of the evidence.
#[cfg(test)]
pub(crate) fn carta(
    t: &'static Texts,
    j: &Jurisdiccion,
    c: &Corredor,
    ev: Option<&Evidencia>,
    hoy: i64,
    zona: i64,
) -> Carta {
    carta_de(t, j, c, ev, hoy, zona, Tipo::Borrado)
}

/// The same, for either request: deletion, or access (what data the company holds), which
/// every law here gives; where there is no law, it is a request and says so.
pub(crate) fn carta_de(
    t: &'static Texts,
    j: &Jurisdiccion,
    c: &Corredor,
    ev: Option<&Evidencia>,
    hoy: i64,
    zona: i64,
    tipo: Tipo,
) -> Carta {
    let (k_asunto, k_cuerpo) = match (tipo, j.derecho) {
        (Tipo::Acceso, Derecho::Ninguno) => {
            ("borrado_asunto_acceso", "borrado_carta_acceso_ninguno")
        }
        (Tipo::Acceso, _) => ("borrado_asunto_acceso", "borrado_carta_acceso"),
        (Tipo::Borrado, Derecho::Borrado) => ("borrado_asunto_borrado", "borrado_carta_borrado"),
        (Tipo::Borrado, Derecho::Limitado) => ("borrado_asunto_limitado", "borrado_carta_limitado"),
        (Tipo::Borrado, Derecho::Ninguno) => ("borrado_asunto_ninguno", "borrado_carta_ninguno"),
    };
    let evidencia = match ev.filter(|e| e.consultas > 0) {
        None => String::new(),
        Some(e) => {
            let lineas: Vec<String> = e
                .nombres
                .iter()
                .take(12)
                .map(|n| {
                    let mismo_dia = dia_de(n.primera, zona) == dia_de(n.ultima, zona);
                    t.panel(if n.consultas == 1 {
                        "borrado_carta_evidencia_linea_una"
                    } else if mismo_dia {
                        "borrado_carta_evidencia_linea_dia"
                    } else {
                        "borrado_carta_evidencia_linea"
                    })
                    .replace("{nombre}", &n.nombre)
                    .replace("{n}", &n.consultas.to_string())
                    .replace("{desde}", &fecha_texto(t, dia_de(n.primera, zona)))
                    .replace("{hasta}", &fecha_texto(t, dia_de(n.ultima, zona)))
                })
                .collect();
            t.panel("borrado_carta_evidencia")
                .replace("{lineas}", &lineas.join("\n"))
        }
    };
    let cuerpo = t
        .panel(k_cuerpo)
        .replace("{empresa}", &c.nombre)
        .replace("{ley}", &texto_jur(t, "ley", j))
        .replace("{articulo}", &texto_jur_tipo(t, "articulo", j, tipo))
        .replace("{plazo}", &texto_jur_tipo(t, "plazo", j, tipo))
        .replace("{evidencia}", &evidencia)
        .replace("{fecha}", &fecha_texto(t, hoy))
        .replace("{nombre}", MARCA_NOMBRE)
        .replace("{correo}", MARCA_CORREO);
    Carta {
        asunto: t.panel(k_asunto).replace("{empresa}", &c.nombre),
        cuerpo,
        para: c.correo_para(j.codigo).map(str::to_owned),
        url: c.url.clone(),
        idioma: i18n::code_of(t),
        hueco_nombre: t.panel("borrado_tu_nombre").to_owned(),
        hueco_correo: t.panel("borrado_tu_correo").to_owned(),
    }
}

// ----- the clock ----------------------------------------------------------------------------

/// What is kept for the clock: no name, no address, no letter.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Envio {
    pub(crate) id: u32,
    pub(crate) corredor: String,
    pub(crate) jurisdiccion: String,
    /// `AAAA-MM-DD`, as the person said.
    pub(crate) enviada: String,
}

/// What the page sends to start a clock. Anything else in the body is refused, so that a
/// careless page can never slip a name or an address into the settings.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct NuevoEnvio {
    corredor: String,
    jurisdiccion: String,
    enviada: String,
}

/// A clock as the page shows it.
#[derive(Serialize, Debug, Clone)]
pub(crate) struct EnvioView {
    id: u32,
    corredor: String,
    corredor_nombre: String,
    jurisdiccion: String,
    jurisdiccion_nombre: String,
    enviada: String,
    enviada_texto: String,
    vence: Option<String>,
    /// Days left until the deadline; negative once it has passed. `None` with no deadline.
    dias: Option<i64>,
    vencido: bool,
    /// The one sentence: «Quedan 12 días: vence el 23/10/2026».
    frase: String,
    /// The extension, when the law allows one.
    prorroga: Option<String>,
    /// Notes on how the days are counted (working days, a reference deadline).
    notas: Vec<String>,
    /// Once the deadline has passed: the authority and how to complain.
    autoridad: Option<&'static str>,
    autoridad_url: Option<&'static str>,
    queja: Option<String>,
}

pub(crate) fn envio_view(t: &Texts, e: &Envio, hoy: i64) -> EnvioView {
    let j = jurisdiccion(&e.jurisdiccion);
    let c = corredor(&e.corredor);
    let enviada = parse_fecha(&e.enviada).unwrap_or(hoy);
    let vence = j.and_then(|j| j.plazo).map(|p| sumar(enviada, p));
    let dias = vence.map(|v| v - hoy);
    let vencido = dias.is_some_and(|d| d < 0);
    let frase = match (vence, dias) {
        (Some(v), Some(d)) if d > 0 => t
            .panel_n("borrado_reloj_quedan", d)
            .replace("{fecha}", &fecha_texto(t, v)),
        (Some(v), Some(0)) => t
            .panel("borrado_reloj_hoy")
            .replace("{fecha}", &fecha_texto(t, v)),
        (Some(v), Some(d)) => t
            .panel_n("borrado_reloj_vencido", -d)
            .replace("{fecha}", &fecha_texto(t, v)),
        _ => t.panel("borrado_reloj_sin_plazo").to_owned(),
    };
    let prorroga = j.and_then(|j| {
        let v = vence?;
        let p = j.prorroga?;
        Some(
            t.panel("borrado_reloj_prorroga")
                .replace("{fecha}", &fecha_texto(t, sumar(v, p))),
        )
    });
    let mut notas = Vec::new();
    if let Some(j) = j {
        if j.plazo.is_some_and(|p| p.unidad == Unidad::DiasHabiles) {
            notas.push(t.panel("borrado_reloj_habiles").to_owned());
        }
        if j.referencia {
            notas.push(texto_jur(t, "referencia", j));
        }
    }
    EnvioView {
        id: e.id,
        corredor: e.corredor.clone(),
        corredor_nombre: c.map_or_else(|| e.corredor.clone(), |c| c.nombre.clone()),
        jurisdiccion: e.jurisdiccion.clone(),
        jurisdiccion_nombre: j.map_or_else(|| e.jurisdiccion.clone(), |j| texto_jur(t, "jur", j)),
        enviada: e.enviada.clone(),
        enviada_texto: fecha_texto(t, enviada),
        vence: vence.map(fecha_iso),
        dias,
        vencido,
        frase,
        prorroga,
        notas,
        autoridad: j.and_then(|j| j.autoridad).filter(|_| vencido),
        autoridad_url: j.and_then(|j| j.autoridad_url).filter(|_| vencido),
        queja: j.filter(|_| vencido).map(|j| texto_jur(t, "queja", j)),
    }
}

/// The clocks this page shows: since 1.0.17 they are the files of «Expedientes» that were sent
/// asking for deletion and are still waiting (the `borrado_envios` setting became those files,
/// once). Answered or closed files live on that page.
fn leer_envios(l: &guardiana_core::Ledger) -> guardiana_core::Result<Vec<Envio>> {
    Ok(crate::expedientes::leer(l)?
        .iter()
        .filter(|e| e.tipo == Tipo::Borrado && e.estado == crate::expedientes::Estado::Enviada)
        .filter_map(|e| {
            Some(Envio {
                id: e.id,
                corredor: e.corredor.clone()?,
                jurisdiccion: e.jurisdiccion.clone(),
                enviada: e.enviada.clone()?,
            })
        })
        .collect())
}

// ----- the page's API -----------------------------------------------------------------------

#[derive(Serialize, Debug)]
pub(crate) struct JurView {
    codigo: &'static str,
    idioma: &'static str,
    nombre: String,
    regiones: &'static [&'static str],
    derecho: Derecho,
    ley: String,
    articulo: String,
    plazo_texto: String,
    /// The one line shown before the law is unfolded: «Plazo: 45 días (+45 días). Si no
    /// responden: CalPrivacy.»
    corta: String,
    /// The same line for an access request, with its own deadline where the law fixes one.
    corta_acceso: String,
    nota: String,
    plazo: Option<Plazo>,
    prorroga: Option<Plazo>,
    referencia: bool,
    autoridad: Option<&'static str>,
    autoridad_url: Option<&'static str>,
    fuentes: &'static [&'static str],
}

#[derive(Serialize)]
pub(crate) struct CorreoView {
    correo: String,
    ambitos: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct CorredorView {
    id: String,
    nombre: String,
    pais: Option<String>,
    tipo: String,
    tipo_texto: String,
    url: Option<String>,
    correos: Vec<CorreoView>,
    estado: Option<String>,
    estado_texto: Option<String>,
    fuentes: Vec<(String, String)>,
    evidencia: Evidencia,
    /// The devices that asked, by name where they have one, for the page.
    aparatos: Vec<(String, Option<String>)>,
    /// «Your devices asked 12 times for 3 names of Spokeo, between … and …».
    frase: String,
}

#[derive(Serialize)]
pub(crate) struct Resumen {
    jurisdicciones: Vec<JurView>,
    corredores: Vec<CorredorView>,
    envios: Vec<EnvioView>,
    /// Devices that do not share their detail: what they asked is not in here at all.
    callados: usize,
    hoy: String,
}

pub(crate) fn jur_view(t: &Texts, j: &'static Jurisdiccion) -> JurView {
    JurView {
        codigo: j.codigo,
        idioma: j.idioma,
        nombre: texto_jur(t, "jur", j),
        regiones: j.regiones,
        derecho: j.derecho,
        ley: texto_jur(t, "ley", j),
        articulo: texto_jur(t, "articulo", j),
        plazo_texto: texto_jur(t, "plazo", j),
        corta: linea_corta(t, j, Tipo::Borrado),
        corta_acceso: linea_corta(t, j, Tipo::Acceso),
        nota: texto_jur(t, "nota", j),
        plazo: j.plazo,
        prorroga: j.prorroga,
        referencia: j.referencia,
        autoridad: j.autoridad,
        autoridad_url: j.autoridad_url,
        fuentes: j.fuentes,
    }
}

/// A deadline in words: «45 días», «1 mes», «20 días hábiles».
fn plazo_en_palabras(t: &Texts, p: Plazo) -> String {
    let k = match p.unidad {
        Unidad::Dias => "borrado_unidad_dias",
        Unidad::DiasHabiles => "borrado_unidad_habiles",
        Unidad::Meses => "borrado_unidad_meses",
    };
    t.panel_n(k, i64::from(p.n))
}

/// The short name of an authority: what it calls itself in brackets (`(CalPrivacy)`, `(AEPD)`),
/// or the whole name when it has none.
fn sigla(autoridad: &str) -> &str {
    autoridad
        .strip_suffix(')')
        .and_then(|a| a.rsplit_once('('))
        .map_or(autoridad, |(_, s)| s)
}

/// The law in one line, for before the person unfolds the rest.
pub(crate) fn linea_corta(t: &Texts, j: &Jurisdiccion, tipo: Tipo) -> String {
    let autoridad = j
        .autoridad
        .map_or_else(|| texto_jur(t, "autoridad", j), |a| sigla(a).to_owned());
    let (plazo, prorroga) = j.plazos(tipo);
    match plazo {
        Some(p) => {
            let mut plazo = plazo_en_palabras(t, p);
            if let Some(x) = prorroga {
                plazo = format!("{plazo} (+{})", plazo_en_palabras(t, x));
            }
            t.panel(if j.es_referencia(tipo) {
                "borrado_corta_referencia"
            } else {
                "borrado_corta"
            })
            .replace("{plazo}", &plazo)
            .replace("{autoridad}", &autoridad)
        }
        None => t
            .panel("borrado_corta_sin_plazo")
            .replace("{autoridad}", &autoridad),
    }
}

fn frase_evidencia(t: &Texts, c: &Corredor, ev: &Evidencia, zona: i64) -> String {
    match (ev.primera, ev.ultima) {
        (Some(p), Some(u)) if ev.consultas > 0 => {
            // One day only: «el 11/10/2026», not «entre el 11/10/2026 y el 11/10/2026».
            let clave = if ev.consultas > 1 && dia_de(p, zona) == dia_de(u, zona) {
                "borrado_evidencia_frase_dia"
            } else {
                "borrado_evidencia_frase"
            };
            let mut s = t
                .panel_n(clave, ev.consultas)
                .replace(
                    "{nombres}",
                    &t.panel_n("borrado_evidencia_nombres", ev.nombres.len() as i64),
                )
                .replace("{empresa}", &c.nombre)
                .replace("{desde}", &fecha_texto(t, dia_de(p, zona)))
                .replace("{hasta}", &fecha_texto(t, dia_de(u, zona)));
            if ev.cortadas > 0 {
                s.push(' ');
                s.push_str(&t.panel_n("borrado_evidencia_cortadas", ev.cortadas));
            }
            s
        }
        _ => t
            .panel("borrado_evidencia_nada")
            .replace("{empresa}", &c.nombre),
    }
}

pub(crate) async fn resumen(
    Lang(t): Lang,
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
) -> ApiResult<Resumen> {
    let (spans, devices, envios) = with_ledger(&state, |l| {
        Ok((l.name_device_spans()?, l.devices()?, leer_envios(l)?))
    })?;
    let hoy = dia_de(now_ms(), zona);
    let nombres: HashMap<String, Option<String>> = devices
        .iter()
        .map(|d| (d.id.clone(), d.name.clone()))
        .collect();
    let callados = devices
        .iter()
        .filter(|d| d.id != SELF_DEVICE_ID && !d.share_detail_with_home)
        .count();
    let mut evs = evidencias(&spans, corredores());
    let mut lista: Vec<CorredorView> = corredores()
        .iter()
        .map(|c| {
            let ev = evs.remove(&c.id).unwrap_or_default();
            let aparatos = ev
                .aparatos
                .iter()
                .map(|d| (d.clone(), nombres.get(d).cloned().flatten()))
                .collect();
            CorredorView {
                id: c.id.clone(),
                nombre: c.nombre.clone(),
                pais: c.pais.as_deref().map(|p| {
                    let n = t.pais(p);
                    if n.is_empty() {
                        p.to_owned()
                    } else {
                        n.to_owned()
                    }
                }),
                tipo: c.tipo.clone(),
                tipo_texto: t.panel(&format!("borrado_tipo_{}", c.tipo)).to_owned(),
                url: c.url.clone(),
                correos: c
                    .correos
                    .iter()
                    .map(|(correo, ambitos)| CorreoView {
                        correo: correo.clone(),
                        ambitos: ambitos.clone(),
                    })
                    .collect(),
                estado: c.estado.clone(),
                estado_texto: c
                    .estado
                    .as_ref()
                    .map(|e| t.panel(&format!("borrado_estado_{e}")).to_owned()),
                fuentes: c.fuentes.clone(),
                frase: frase_evidencia(t, c, &ev, zona),
                evidencia: ev,
                aparatos,
            }
        })
        .collect();
    // The ones the house asked for first, busiest first; then the rest by name.
    lista.sort_by(|a, b| {
        b.evidencia
            .consultas
            .cmp(&a.evidencia.consultas)
            .then_with(|| a.nombre.to_lowercase().cmp(&b.nombre.to_lowercase()))
    });
    Ok(Json(Resumen {
        jurisdicciones: JURISDICCIONES.iter().map(|j| jur_view(t, j)).collect(),
        corredores: lista,
        envios: envios.iter().map(|e| envio_view(t, e, hoy)).collect(),
        callados,
        hoy: fecha_iso(hoy),
    }))
}

#[derive(Deserialize)]
pub(crate) struct CartaQuery {
    /// A company of the deletion list, by its id.
    #[serde(default)]
    corredor: Option<String>,
    /// Or any company of the lists, by its name, or a site (from its profile page).
    #[serde(default)]
    empresa: Option<String>,
    jurisdiccion: String,
    /// `es`, `en` or `pt`: the letter's language, which need not be the panel's.
    #[serde(default)]
    idioma: Option<String>,
    /// `0` leaves the house's evidence out of the letter.
    #[serde(default)]
    evidencia: Option<String>,
    /// `acceso` asks what data the company holds instead of asking it to delete them.
    #[serde(default)]
    tipo: Tipo,
}

fn no_encontrado(que: &str) -> Response {
    (StatusCode::NOT_FOUND, format!("unknown {que}")).into_response()
}

pub(crate) async fn carta_api(
    Lang(t_panel): Lang,
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Query(q): Query<CartaQuery>,
) -> ApiResult<Carta> {
    let j = jurisdiccion(&q.jurisdiccion).ok_or_else(|| no_encontrado("jurisdiction"))?;
    let c = match (&q.corredor, &q.empresa) {
        (Some(id), _) => corredor(id).cloned(),
        (None, Some(e)) => crate::empresa::destinatario(e),
        (None, None) => None,
    }
    .ok_or_else(|| no_encontrado("company"))?;
    let t = q.idioma.as_deref().map_or(t_panel, i18n::by_code);
    let ev = if q.evidencia.as_deref() == Some("0") {
        None
    } else {
        let spans = with_ledger(&state, |l| l.name_device_spans())?;
        let mut evs = evidencias(&spans, std::slice::from_ref(&c));
        evs.remove(&c.id)
    };
    Ok(Json(carta_de(
        t,
        j,
        &c,
        ev.as_ref(),
        dia_de(now_ms(), zona),
        zona,
        q.tipo,
    )))
}

pub(crate) async fn nuevo_envio(
    Lang(t): Lang,
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Json(body): Json<NuevoEnvio>,
) -> ApiResult<Vec<EnvioView>> {
    let malo = |m: &'static str| (StatusCode::BAD_REQUEST, m).into_response();
    if corredor(&body.corredor).is_none() {
        return Err(malo("unknown company"));
    }
    if jurisdiccion(&body.jurisdiccion).is_none() {
        return Err(malo("unknown jurisdiction"));
    }
    let hoy = dia_de(now_ms(), zona);
    let Some(dia) = parse_fecha(&body.enviada) else {
        return Err(malo("date must be AAAA-MM-DD"));
    };
    // A letter cannot have been sent tomorrow; three years back is already more than any
    // deadline here.
    if dia > hoy + 1 || dia < hoy - 3 * 366 {
        return Err(malo("date out of range"));
    }
    let lista = with_ledger(&state, |l| {
        crate::expedientes::abrir_enviado(l, &body.corredor, &body.jurisdiccion, fecha_iso(dia))?;
        leer_envios(l)
    })?;
    Ok(Json(lista.iter().map(|e| envio_view(t, e, hoy)).collect()))
}

pub(crate) async fn quitar_envio(
    Lang(t): Lang,
    Zona(zona): Zona,
    State(state): State<Arc<AppState>>,
    _s: Session,
    Path(id): Path<u32>,
) -> ApiResult<Vec<EnvioView>> {
    let lista = with_ledger(&state, |l| {
        crate::expedientes::quitar_de(l, id)?;
        leer_envios(l)
    })?;
    let hoy = dia_de(now_ms(), zona);
    Ok(Json(lista.iter().map(|e| envio_view(t, e, hoy)).collect()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn textos() -> [(&'static str, &'static Texts); 3] {
        [("es", i18n::es()), ("en", i18n::en()), ("pt", i18n::pt())]
    }

    #[test]
    fn the_data_file_reads_whole_and_every_company_has_a_way_or_a_reason() {
        let lista = parse_corredores(DATOS_CORREDORES).expect("the file parses");
        assert!(lista.len() >= 25, "{}", lista.len());
        for c in &lista {
            assert!(!c.fuentes.is_empty(), "{}: sin fuente", c.id);
            assert!(
                c.url.is_some() || c.estado.is_some(),
                "{}: ni canal ni motivo",
                c.id
            );
            assert!(
                !c.dominios.is_empty() || !c.empresas.is_empty(),
                "{}: sin nombres",
                c.id
            );
        }
        assert_eq!(corredores().len(), lista.len());
        // A section of empresas.txt that does not exist would match nothing, silently: every
        // `=Empresa` must be here with one name the list gives to it.
        let conocidas = [
            ("LiveRamp", "rlcdn.com"),
            ("Nielsen", "imrworldwide.com"),
            ("Comscore", "scorecardresearch.com"),
            ("Criteo", "criteo.net"),
            ("Taboola", "taboola.com"),
            ("Outbrain", "outbrain.com"),
            ("Quantcast", "quantserve.com"),
        ];
        for c in &lista {
            for e in &c.empresas {
                let (_, nombre) = conocidas
                    .iter()
                    .find(|(x, _)| x == e)
                    .expect("every `=Empresa` of the file is listed in this test");
                assert_eq!(
                    guardiana_lists::company_of(nombre),
                    Some(e.as_str()),
                    "{}: «={e}» no es una sección de empresas.txt",
                    c.id
                );
            }
        }
    }

    /// The letter goes in the language of the place by default: what the company there reads.
    #[test]
    fn every_jurisdiction_writes_its_letter_in_its_own_language() {
        for j in JURISDICCIONES {
            let esperado = match j.codigo {
                "es" | "mx" | "co" | "ar" | "cl" | "pe" => "es",
                "pt" | "br" => "pt",
                _ => "en",
            };
            assert_eq!(j.idioma, esperado, "{}", j.codigo);
            // And the three files can write it.
            let t = i18n::by_code(j.idioma);
            assert_eq!(i18n::code_of(t), j.idioma);
        }
    }

    #[test]
    fn the_short_line_says_deadline_extension_and_authority() {
        let l = |c: &str| linea_corta(i18n::es(), jurisdiccion(c).unwrap(), Tipo::Borrado);
        assert_eq!(
            l("us_ca"),
            "Plazo: 45 días (+45 días). Si no responden: CalPrivacy."
        );
        assert_eq!(l("es"), "Plazo: 1 mes (+2 meses). Si no responden: AEPD.");
        assert!(l("co").starts_with("Plazo: 15 días hábiles (+8 días hábiles)."));
        assert!(l("br").starts_with("Plazo de referencia: 15 días."));
        assert!(l("us").starts_with("La ley no fija un plazo."));
        let a = |c: &str| linea_corta(i18n::es(), jurisdiccion(c).unwrap(), Tipo::Acceso);
        assert!(
            a("co").starts_with("Plazo: 10 días hábiles (+5 días hábiles)."),
            "{}",
            a("co")
        );
        assert!(a("ar").starts_with("Plazo: 10 días."), "{}", a("ar"));
        assert!(
            a("br").starts_with("Plazo: 15 días."),
            "not a reference for access: {}",
            a("br")
        );
        assert_eq!(a("es"), l("es"));
        let en = linea_corta(i18n::en(), jurisdiccion("gb").unwrap(), Tipo::Borrado);
        assert_eq!(
            en,
            "Deadline: 1 month (+2 months). If they don't answer: ICO."
        );
    }

    #[test]
    fn names_match_by_domain_suffix_and_by_company() {
        let spokeo = corredor("spokeo").unwrap();
        assert!(spokeo.coincide("www.spokeo.com"));
        assert!(spokeo.coincide("spokeo.com."));
        assert!(!spokeo.coincide("notspokeo.com"));
        let criteo = corredor("criteo").unwrap();
        assert!(
            criteo.coincide("static.criteo.net"),
            "=Criteo pulls empresas.txt"
        );
        assert_eq!(
            corredor("datacredito").unwrap().correo_para("co"),
            Some("servicioalciudadano@experian.com")
        );
        assert_eq!(corredor("datacredito").unwrap().correo_para("es"), None);
        assert_eq!(
            corredor("criteo").unwrap().correo_para("us_ca"),
            Some("dpo@criteo.com")
        );
    }

    #[test]
    fn every_jurisdiction_has_all_its_texts_in_the_three_languages() {
        for (lang, t) in textos() {
            for j in JURISDICCIONES {
                for campo in ["jur", "ley", "articulo", "plazo", "nota", "queja"] {
                    let k = format!("borrado_{campo}_{}", j.codigo);
                    assert!(t.panel.contains_key(&k), "{lang}: falta {k}");
                }
                if j.autoridad.is_none() {
                    let k = format!("borrado_autoridad_{}", j.codigo);
                    assert!(t.panel.contains_key(&k), "{lang}: falta {k}");
                }
                for tipo in [Tipo::Borrado, Tipo::Acceso] {
                    assert!(
                        !linea_corta(t, j, tipo).contains('{'),
                        "{lang}/{}",
                        j.codigo
                    );
                }
                if j.referencia {
                    let k = format!("borrado_referencia_{}", j.codigo);
                    assert!(t.panel.contains_key(&k), "{lang}: falta {k}");
                }
                assert!(!j.fuentes.is_empty(), "{}: sin fuentes", j.codigo);
            }
            for tipo in ["personas", "datos", "publicidad", "credito"] {
                assert!(
                    t.panel.contains_key(&format!("borrado_tipo_{tipo}")),
                    "{lang}"
                );
            }
            for e in ["cerrado", "incautado"] {
                assert!(
                    t.panel.contains_key(&format!("borrado_estado_{e}")),
                    "{lang}"
                );
            }
        }
    }

    /// The article each letter cites, per jurisdiction and per language: the one place a wrong
    /// copy-paste in the language files would send a person to the wrong law.
    #[test]
    fn letters_cite_the_right_article_in_each_language() {
        let esperado: &[(&str, &[&str])] = &[
            ("es", &["17", "LOPDGDD", "15"]),
            ("pt", &["17"]),
            ("ie", &["17"]),
            ("ue", &["17"]),
            ("gb", &["17", "UK GDPR"]),
            ("us_ca", &["1798.105"]),
            ("us_va", &["59.1-577"]),
            ("us_co", &["6-1-1306"]),
            ("us_ct", &["42-518"]),
            ("us_ut", &["13-61-201"]),
            ("us_tx", &["541.051"]),
            ("us_or", &["646A.574"]),
            ("ca", &["4.3.8", "4.5.3", "4.9"]),
            ("ca_qc", &["28", "40"]),
            ("au", &["12", "13"]),
            ("nz", &["6", "7"]),
            ("br", &["18"]),
            ("mx", &["24"]),
            ("co", &["8"]),
            ("ar", &["16"]),
            ("cl", &["12"]),
            ("pe", &["20"]),
            ("tr", &["11", "6698"]),
            ("jp", &["33", "35", "APPI"]),
            ("kr", &["36", "PIPA"]),
            ("cn", &["47", "PIPL"]),
            ("hk", &["18", "26"]),
            ("tw", &["11"]),
            ("sg", &["16", "21", "25"]),
            ("th", &["33", "PDPA"]),
            ("my", &["30", "38", "709"]),
            ("ph", &["16", "10173"]),
            ("id", &["8", "43", "27"]),
            ("sa", &["4", "PDPL"]),
        ];
        let c = corredor("spokeo").unwrap();
        let hoy = parse_fecha("2026-10-11").unwrap();
        for (lang, t) in textos() {
            for (cod, partes) in esperado {
                let j = jurisdiccion(cod).unwrap();
                let carta = carta(t, j, c, None, hoy, 0);
                for p in *partes {
                    assert!(
                        carta.cuerpo.contains(p),
                        "{lang}/{cod}: la carta no cita «{p}»:\n{}",
                        carta.cuerpo
                    );
                }
                assert!(carta.cuerpo.contains(MARCA_NOMBRE), "{lang}/{cod}");
                assert!(carta.cuerpo.contains(MARCA_CORREO), "{lang}/{cod}");
                assert!(carta.cuerpo.contains("Spokeo"), "{lang}/{cod}");
                assert!(
                    !carta.cuerpo.contains('{'),
                    "{lang}/{cod}: hueco sin llenar:\n{}",
                    carta.cuerpo
                );
                assert!(!carta.asunto.is_empty() && !carta.asunto.contains('{'));
                assert_eq!(carta.idioma, lang);
            }
            // The federal US letter claims no law at all.
            let us = carta(t, jurisdiccion("us").unwrap(), c, None, hoy, 0);
            assert!(!us.cuerpo.contains("1798"), "{lang}: {}", us.cuerpo);
        }
    }

    #[test]
    fn the_evidence_goes_into_the_letter_and_only_when_there_is_some() {
        let c = corredor("spokeo").unwrap();
        let dia = guardiana_core::time::DAY_MS;
        let t0 = parse_fecha("2026-10-01").unwrap() * dia;
        let spans = vec![
            NameSpan {
                qname: "www.spokeo.com".into(),
                device_id: "self".into(),
                queries: 7,
                cut: 2,
                first_ts: t0,
                last_ts: t0 + 3 * dia,
            },
            NameSpan {
                qname: "thatsthem.com".into(),
                device_id: "mac:a".into(),
                queries: 1,
                cut: 0,
                first_ts: t0 + dia,
                last_ts: t0 + dia,
            },
            NameSpan {
                qname: "example.com".into(),
                device_id: "self".into(),
                queries: 50,
                cut: 0,
                first_ts: t0,
                last_ts: t0,
            },
        ];
        let evs = evidencias(&spans, corredores());
        let ev = evs.get("spokeo").unwrap();
        assert_eq!((ev.consultas, ev.cortadas), (8, 2));
        assert_eq!(ev.nombres.len(), 2);
        assert_eq!(ev.nombres[0].nombre, "www.spokeo.com");
        assert_eq!(ev.aparatos, vec!["self".to_owned(), "mac:a".to_owned()]);
        assert_eq!((ev.primera, ev.ultima), (Some(t0), Some(t0 + 3 * dia)));
        assert_eq!(evs.len(), 1, "example.com is nobody's: {evs:?}");
        let hoy = parse_fecha("2026-10-11").unwrap();
        let con = carta(i18n::es(), jurisdiccion("es").unwrap(), c, Some(ev), hoy, 0);
        assert!(con.cuerpo.contains("www.spokeo.com"), "{}", con.cuerpo);
        assert!(con.cuerpo.contains("1/10/2026"), "{}", con.cuerpo);
        assert!(
            con.cuerpo
                .contains("- thatsthem.com: una vez, el 2/10/2026"),
            "{}",
            con.cuerpo
        );
        let sin = carta(i18n::es(), jurisdiccion("es").unwrap(), c, None, hoy, 0);
        assert!(!sin.cuerpo.contains("www.spokeo.com"));
    }

    #[test]
    fn deadlines_count_days_working_days_and_months() {
        let d = |s| parse_fecha(s).unwrap();
        // Calendar days.
        assert_eq!(
            fecha_iso(sumar(d("2026-10-11"), dias(45).unwrap())),
            "2026-11-25"
        );
        // Working days start the next day and skip the weekend: Friday + 5 = next Friday.
        assert_eq!(
            fecha_iso(sumar(d("2026-10-09"), habiles(5).unwrap())),
            "2026-10-16"
        );
        // Colombia, 15 working days from Monday 12 Oct 2026.
        assert_eq!(
            fecha_iso(sumar(d("2026-10-12"), habiles(15).unwrap())),
            "2026-11-02"
        );
        // One month, and the end of a shorter month.
        assert_eq!(
            fecha_iso(sumar(d("2026-10-11"), meses(1).unwrap())),
            "2026-11-11"
        );
        assert_eq!(
            fecha_iso(sumar(d("2026-01-31"), meses(1).unwrap())),
            "2026-02-28"
        );
        assert_eq!(
            fecha_iso(sumar(d("2028-01-31"), meses(1).unwrap())),
            "2028-02-29"
        );
        assert_eq!(
            fecha_iso(sumar(d("2026-12-15"), meses(2).unwrap())),
            "2027-02-15"
        );
        assert_eq!(parse_fecha("2026-02-30"), None);
        assert_eq!(parse_fecha("2026-1-01"), None);
        assert_eq!(fecha_iso(d("1970-01-01")), "1970-01-01");
        assert_eq!(dia_semana(d("2026-10-11")), 6, "11 Oct 2026 is a Sunday");
    }

    #[test]
    fn the_clock_says_days_left_and_after_the_deadline_who_to_complain_to() {
        let t = i18n::es();
        let e = Envio {
            id: 1,
            corredor: "spokeo".into(),
            jurisdiccion: "es".into(),
            enviada: "2026-09-01".into(),
        };
        let antes = envio_view(t, &e, parse_fecha("2026-09-21").unwrap());
        assert_eq!(antes.vence.as_deref(), Some("2026-10-01"));
        assert_eq!(antes.dias, Some(10));
        assert!(!antes.vencido && antes.queja.is_none() && antes.autoridad.is_none());
        assert!(antes.frase.contains("10"), "{}", antes.frase);
        let despues = envio_view(t, &e, parse_fecha("2026-10-11").unwrap());
        assert!(despues.vencido);
        assert_eq!(despues.dias, Some(-10));
        assert!(despues.autoridad.unwrap().contains("AEPD"));
        assert!(despues.queja.is_some());
        assert!(despues.prorroga.unwrap().contains("1/12/2026"));
        let br = Envio {
            jurisdiccion: "br".into(),
            ..e.clone()
        };
        assert!(!envio_view(t, &br, parse_fecha("2026-09-02").unwrap())
            .notas
            .is_empty());
        let us = Envio {
            jurisdiccion: "us".into(),
            ..e
        };
        let v = envio_view(t, &us, parse_fecha("2026-09-02").unwrap());
        assert_eq!(v.dias, None);
        assert!(!v.vencido);
    }

    #[test]
    fn the_stored_clock_has_no_room_for_a_name() {
        let malo =
            r#"{"corredor":"spokeo","jurisdiccion":"es","enviada":"2026-10-01","nombre":"Ana"}"#;
        assert!(serde_json::from_str::<NuevoEnvio>(malo).is_err());
        let guardado = serde_json::to_string(&Envio {
            id: 3,
            corredor: "spokeo".into(),
            jurisdiccion: "es".into(),
            enviada: "2026-10-01".into(),
        })
        .unwrap();
        assert_eq!(
            guardado,
            r#"{"id":3,"corredor":"spokeo","jurisdiccion":"es","enviada":"2026-10-01"}"#
        );
    }
}
