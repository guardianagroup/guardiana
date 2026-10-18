//! The «Certificado de flujos» as a page to read and print (save as PDF from the browser's own
//! print dialog). Made from the certificate itself and the panel's texts, so the HTML says
//! exactly what the sealed JSON says, in the person's language. No script, no image, nothing
//! fetched: the file opens on any computer, years from now.

use guardiana_core::i18n::{self, Texts};
use guardiana_core::time::rfc3339_utc;
use guardiana_core::Category;

use crate::certificado::{Bytes, Certificado, FilaEmpresa};

/// The page's style: inlined in the saved file and served to the panel's view of it.
pub const CSS: &str = include_str!("certificado.css");

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

/// A whole number with the thousands mark of the language (`12.345`, `12,345`).
#[must_use]
pub fn miles(n: u64, idioma: &str) -> String {
    let marca = if idioma == "en" { ',' } else { '.' };
    let d = n.to_string();
    let mut o = String::new();
    for (i, c) in d.chars().enumerate() {
        if i > 0 && (d.len() - i) % 3 == 0 {
            o.push(marca);
        }
        o.push(c);
    }
    o
}

fn bytes_texto(b: u64, idioma: &str) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1000.0 && i < U.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    let s = if i == 0 {
        format!("{b} B")
    } else {
        format!("{v:.1} {}", U[i])
    };
    if idioma == "en" {
        s
    } else {
        s.replace('.', ",")
    }
}

/// Consecutive days joined: `["2026-09-01","2026-09-02","2026-09-05"]` →
/// `[("2026-09-01","2026-09-02"), ("2026-09-05","2026-09-05")]`. For «on these dates».
#[must_use]
pub fn rangos_de_dias(dias: &[String]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String, i64)> = Vec::new();
    for d in dias {
        let Some(n) = crate::certificado::dia_de_texto(d) else {
            continue;
        };
        match out.last_mut() {
            Some((_, hasta, ultimo)) if *ultimo + 1 == n => {
                hasta.clone_from(d);
                *ultimo = n;
            }
            _ => out.push((d.clone(), d.clone(), n)),
        }
    }
    out.into_iter().map(|(a, b, _)| (a, b)).collect()
}

fn pais(t: &Texts, clave: &str) -> String {
    match clave {
        "sin_pais" => t.panel("cert_pais_sin_pais").to_owned(),
        "anycast" => t.panel("cert_pais_anycast").to_owned(),
        "local" => t.panel("cert_pais_local").to_owned(),
        c => {
            let n = t.pais(c);
            if n.is_empty() {
                c.to_owned()
            } else {
                n.to_owned()
            }
        }
    }
}

fn categoria(t: &Texts, c: &str) -> String {
    c.parse::<Category>()
        .map_or_else(|_| c.to_owned(), |c| t.category(c).to_owned())
}

fn fila_empresa(t: &Texts, idioma: &str, e: &FilaEmpresa) -> String {
    let servidores: Vec<String> = {
        let mut v: Vec<(&String, &u64)> = e.por_pais_servidor.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        v.into_iter()
            .map(|(k, n)| format!("{} {}", esc(&pais(t, k)), miles(*n, idioma)))
            .collect()
    };
    let categorias: Vec<String> = e
        .por_categoria
        .iter()
        .map(|(k, n)| format!("{} {}", esc(&categoria(t, k)), miles(*n, idioma)))
        .collect();
    let sede = e
        .pais_sede
        .as_deref()
        .map(|p| pais(t, p))
        .unwrap_or_default();
    let mut nombre = esc(&e.empresa);
    if let Some(g) = &e.grupo {
        nombre.push_str(&format!(" <span class=\"c-chico\">({})</span>", esc(g)));
    }
    let mut dominios = String::new();
    if !e.dominios.is_empty() {
        let lista: Vec<String> = e
            .dominios
            .iter()
            .map(|d| {
                format!(
                    "<code>{}</code> {}",
                    esc(&d.nombre),
                    miles(d.consultas, idioma)
                )
            })
            .collect();
        dominios = format!("<br><span class=\"c-chico\">{}</span>", lista.join(" · "));
    }
    let primera = e.dias.first().cloned().unwrap_or_default();
    let ultima = e.dias.last().cloned().unwrap_or_default();
    format!(
        "<tr><td>{nombre}{dominios}</td><td>{}</td><td class=\"n\">{}</td><td class=\"n{}\">{}</td>\
         <td class=\"n\">{}<br><span class=\"c-chico\">{} – {}</span></td><td>{}</td><td>{}</td></tr>",
        esc(&sede),
        miles(e.recuento.consultas, idioma),
        if e.recuento.cortadas > 0 { " c-cortado" } else { "" },
        miles(e.recuento.cortadas, idioma),
        miles(e.dias.len() as u64, idioma),
        esc(&primera),
        esc(&ultima),
        servidores.join("<br>"),
        categorias.join("<br>"),
    )
}

fn fila_bytes(idioma: &str, nombre: &str, b: Bytes) -> String {
    format!(
        "<tr><td>{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td></tr>",
        esc(nombre),
        bytes_texto(b.enviados, idioma),
        bytes_texto(b.recibidos, idioma)
    )
}

/// The certificate as a whole HTML document, with its seal (the SHA-256 of the `.json` file).
#[must_use]
pub fn html(t: &'static Texts, c: &Certificado, sello: &str) -> String {
    let idioma = i18n::code_of(t);
    let k = &c.contenido;
    let tot = &k.totales;
    let r = |clave: &str| t.panel(clave).to_owned();
    let mut b = String::new();

    let titulo = match &c.opciones.solo_empresa {
        Some(e) => r("cert_titulo_extracto").replace("{empresa}", e),
        None => r("cert_titulo"),
    };
    b.push_str(&format!(
        "<p class=\"c-marca\">GUARDIANA</p><h1>{}</h1><p class=\"c-sub\">{}</p><p class=\"c-meta\">{}</p>",
        esc(&titulo),
        esc(&r("cert_subtitulo")
            .replace("{desde}", &c.desde_dia)
            .replace("{hasta}", &c.hasta_dia)),
        esc(&r("cert_generado")
            .replace("{fecha}", &c.generado_utc.replace('T', " ").replace(".000Z", ""))
            .replace("{version}", &c.programa)),
    ));
    if let Some(e) = &c.opciones.solo_empresa {
        b.push_str(&format!(
            "<p class=\"c-aviso\">{}</p>",
            esc(&r("cert_extracto_nota").replace("{empresa}", e))
        ));
    }

    // The figures.
    let cifra = |n: u64, clave: &str, clase: &str| {
        format!(
            "<div class=\"c-cifra\"><b class=\"{clase}\">{}</b><span>{}</span></div>",
            miles(n, idioma),
            esc(&r(clave))
        )
    };
    b.push_str("<div class=\"c-resumen\">");
    b.push_str(&cifra(tot.recuento.consultas, "cert_cifra_consultas", ""));
    b.push_str(&cifra(tot.empresas as u64, "cert_cifra_empresas", ""));
    b.push_str(&cifra(
        tot.recuento.cortadas,
        "cert_cifra_cortadas",
        if tot.recuento.cortadas > 0 {
            "c-cortado"
        } else {
            ""
        },
    ));
    b.push_str(&cifra(tot.dias_con_actividad as u64, "cert_cifra_dias", ""));
    b.push_str("</div>");

    // How much of the period it covers.
    let cob = &c.cobertura;
    match cob.registro_desde {
        None => b.push_str(&format!(
            "<p class=\"c-aviso\">{}</p>",
            esc(&r("cert_cobertura_vacia"))
        )),
        Some(_) if cob.completa => b.push_str(&format!(
            "<p class=\"c-meta\">{}</p>",
            esc(&r("cert_cobertura_completa"))
        )),
        Some(desde) => b.push_str(&format!(
            "<p class=\"c-aviso\">{}</p>",
            esc(&r("cert_cobertura_parcial").replace(
                "{fecha}",
                &rfc3339_utc(desde)
                    .replace('T', " ")
                    .replace(".000Z", " UTC")
            ))
        )),
    }
    if cob.sin_vigilar_ms > 0 {
        let horas = (cob.sin_vigilar_ms as f64 / 3_600_000.0).max(0.1);
        let h = if idioma == "en" {
            format!("{horas:.1}")
        } else {
            format!("{horas:.1}").replace('.', ",")
        };
        b.push_str(&format!(
            "<p class=\"c-aviso\">{}</p>",
            esc(&r("cert_sin_vigilar")
                .replace("{horas}", &h)
                .replace("{n}", &cob.sin_vigilar.len().to_string()))
        ));
    }

    // The companies.
    b.push_str(&format!("<h2>{}</h2>", esc(&r("cert_empresas_t"))));
    if k.empresas.is_empty() {
        b.push_str(&format!("<p>{}</p>", esc(&r("cert_empresas_ninguna"))));
    } else {
        b.push_str(&format!(
            "<table><thead><tr><th>{}</th><th>{}</th><th class=\"n\">{}</th><th class=\"n\">{}</th>\
             <th class=\"n\">{}</th><th>{}</th><th>{}</th></tr></thead><tbody>",
            esc(&r("cert_col_empresa")),
            esc(&r("cert_col_sede")),
            esc(&r("cert_col_consultas")),
            esc(&r("cert_col_cortadas")),
            esc(&r("cert_col_dias")),
            esc(&r("cert_col_servidores")),
            esc(&r("cert_col_categorias")),
        ));
        for e in &k.empresas {
            b.push_str(&fila_empresa(t, idioma, e));
        }
        b.push_str("</tbody></table>");
    }
    if c.opciones.solo_empresa.is_some() {
        if let Some(e) = k.empresas.first() {
            let rangos: Vec<String> = rangos_de_dias(&e.dias)
                .into_iter()
                .map(|(a, z)| if a == z { a } else { format!("{a} – {z}") })
                .collect();
            b.push_str(&format!(
                "<p><b>{}</b> {}</p>",
                esc(&r("cert_fechas")),
                esc(&rangos.join(", "))
            ));
        }
    }
    if let Some(s) = &k.sin_empresa {
        if s.recuento.consultas > 0 {
            b.push_str(&format!(
                "<p class=\"c-meta\">{}</p>",
                esc(&r("cert_sin_empresa")
                    .replace("{consultas}", &miles(s.recuento.consultas, idioma))
                    .replace("{nombres}", &miles(s.nombres as u64, idioma))
                    .replace("{cortadas}", &miles(s.recuento.cortadas, idioma)))
            ));
        }
    }
    if tot.locales > 0 {
        b.push_str(&format!(
            "<p class=\"c-meta\">{}</p>",
            esc(&r("cert_locales").replace("{n}", &miles(tot.locales, idioma)))
        ));
    }

    // Countries and categories.
    if !k.por_pais_servidor.is_empty() {
        let mut v: Vec<(&String, &u64)> = k.por_pais_servidor.iter().collect();
        v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        b.push_str(&format!(
            "<h2>{}</h2><p class=\"c-chico\">{}</p><table><tbody>",
            esc(&r("cert_paises_t")),
            esc(&r("cert_paises_lead"))
        ));
        for (p, n) in v {
            b.push_str(&format!(
                "<tr><td>{}</td><td class=\"n\">{}</td></tr>",
                esc(&pais(t, p)),
                miles(*n, idioma)
            ));
        }
        b.push_str("</tbody></table>");
    }
    if !k.por_categoria.is_empty() {
        b.push_str(&format!(
            "<h2>{}</h2><table><thead><tr><th></th><th class=\"n\">{}</th><th class=\"n\">{}</th></tr></thead><tbody>",
            esc(&r("cert_categorias_t")),
            esc(&r("cert_col_consultas")),
            esc(&r("cert_col_cortadas"))
        ));
        for (cat, rc) in &k.por_categoria {
            b.push_str(&format!(
                "<tr><td>{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td></tr>",
                esc(&categoria(t, cat)),
                miles(rc.consultas, idioma),
                miles(rc.cortadas, idioma)
            ));
        }
        b.push_str("</tbody></table>");
    }

    // Who it did not contact.
    if !k.referencia.is_empty() {
        let total = k.no_contactadas.len()
            + k.empresas
                .iter()
                .filter(|e| guardiana_lists::profile_of(&e.empresa).is_some())
                .count();
        b.push_str(&format!("<h2>{}</h2>", esc(&r("cert_no_t"))));
        if k.no_contactadas.is_empty() {
            b.push_str(&format!("<p>{}</p>", esc(&r("cert_no_ninguna"))));
        } else {
            b.push_str(&format!(
                "<p>{}</p><ul class=\"c-lista\">",
                esc(&r("cert_no_lead")
                    .replace("{n}", &total.to_string())
                    .replace("{referencia}", &k.referencia))
            ));
            for n in &k.no_contactadas {
                b.push_str(&format!(
                    "<li><b>{}</b> <span class=\"c-chico\">{}</span></li>",
                    esc(&n.empresa),
                    esc(&r("cert_no_nombres").replace("{n}", &n.nombres_conocidos.to_string()))
                ));
            }
            b.push_str("</ul>");
        }
    }

    // What was measured.
    b.push_str(&format!("<h2>{}</h2>", esc(&r("cert_medido_t"))));
    let m = &c.medido;
    if m.disponible {
        b.push_str(&format!(
            "<p class=\"c-chico\">{}</p><table><thead><tr><th></th><th class=\"n\">{}</th><th class=\"n\">{}</th></tr></thead><tbody>",
            esc(&r("cert_medido_lead")),
            esc(&r("cert_col_enviados")),
            esc(&r("cert_col_recibidos"))
        ));
        let mut v: Vec<(&String, &Bytes)> = m.por_empresa.iter().collect();
        v.sort_by(|a, b| {
            (b.1.enviados.saturating_add(b.1.recibidos))
                .cmp(&a.1.enviados.saturating_add(a.1.recibidos))
                .then(a.0.cmp(b.0))
        });
        for (e, x) in v {
            b.push_str(&fila_bytes(idioma, e, *x));
        }
        if m.otros_nombres != Bytes::default() {
            b.push_str(&fila_bytes(
                idioma,
                &r("cert_medido_otros"),
                m.otros_nombres,
            ));
        }
        if m.sin_nombre != Bytes::default() {
            b.push_str(&fila_bytes(
                idioma,
                &r("cert_medido_sin_nombre"),
                m.sin_nombre,
            ));
            for (p, x) in &m.sin_nombre_por_pais {
                b.push_str(&fila_bytes(idioma, &format!("   {}", pais(t, p)), *x));
            }
        }
        b.push_str("</tbody></table>");
    } else {
        b.push_str(&format!(
            "<p class=\"c-meta\">{}</p>",
            esc(&r("cert_medido_no"))
        ));
    }

    // The chain and the seal.
    let cad = &c.cadena;
    b.push_str(&format!("<h2>{}</h2>", esc(&r("cert_cadena_t"))));
    if cad.comprobada {
        b.push_str(&format!(
            "<p class=\"c-bien\">{}</p>",
            esc(&r("cert_cadena_ok").replace("{n}", &miles(cad.filas_comprobadas, idioma)))
        ));
    } else {
        b.push_str(&format!(
            "<p class=\"c-mal\">{}</p>",
            esc(&r("cert_cadena_mal").replace("{fallo}", cad.fallo.as_deref().unwrap_or("?")))
        ));
    }
    if cad.clave_desarrollo {
        b.push_str(&format!(
            "<p class=\"c-aviso\">{}</p>",
            esc(&r("cert_cadena_dev"))
        ));
    }
    b.push_str(&format!(
        "<p>{}</p><dl>",
        esc(&r("cert_cadena_filas")
            .replace("{n}", &miles(cad.filas_periodo, idioma))
            .replace(
                "{primera}",
                &cad.primera
                    .as_ref()
                    .map_or_else(|| "–".to_owned(), |f| f.id.to_string())
            )
            .replace(
                "{ultima}",
                &cad.ultima
                    .as_ref()
                    .map_or_else(|| "–".to_owned(), |f| f.id.to_string())
            ))
    ));
    let dato = |clave: &str, valor: &str| {
        format!(
            "<dt>{}</dt><dd class=\"c-hash\">{}</dd>",
            esc(&r(clave)),
            esc(valor)
        )
    };
    b.push_str(&dato("cert_cadena_huella", &cad.huella_filas));
    if let Some(f) = &cad.primera {
        b.push_str(&dato("cert_cadena_primera", &f.prev_hash));
    }
    if let Some(f) = &cad.ultima {
        b.push_str(&dato("cert_cadena_ultima", &f.row_hash));
    }
    if let Some(h) = &cad.cabeza {
        b.push_str(&dato("cert_cadena_cabeza", h));
    }
    b.push_str(&dato("cert_cadena_genesis", &cad.genesis));
    b.push_str(&dato("cert_sello", sello));
    b.push_str("</dl>");
    b.push_str(&format!(
        "<p class=\"c-chico\">{}</p>",
        esc(&r("cert_como_comprobar"))
    ));

    // What it proves and what it does not.
    b.push_str(&format!(
        "<div class=\"c-pie\"><h2>{}</h2><p>{}</p><p>{}</p><p class=\"c-chico\">{}</p><p class=\"c-chico\">{}</p></div>",
        esc(&r("cert_prueba_t")),
        esc(&r("cert_prueba")),
        esc(&r("cert_no_prueba")),
        esc(&r("cert_no_firma")),
        esc(&r("cert_imprimir")),
    ));

    format!(
        "<!doctype html>\n<html lang=\"{idioma}\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta name=\"referrer\" content=\"no-referrer\">\n<title>GUARDIANA · {}</title>\n\
         <link rel=\"stylesheet\" href=\"/static/certificado.css\">\n<style>\n{CSS}</style>\n\
         </head>\n<body>\n<main class=\"cert\">\n{b}\n</main>\n</body>\n</html>\n",
        esc(&titulo)
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_day_ranges() {
        assert_eq!(miles(0, "es"), "0");
        assert_eq!(miles(1234567, "es"), "1.234.567");
        assert_eq!(miles(1234, "en"), "1,234");
        assert_eq!(bytes_texto(999, "es"), "999 B");
        assert_eq!(bytes_texto(1_500_000, "es"), "1,5 MB");
        assert_eq!(bytes_texto(1_500_000, "en"), "1.5 MB");
        let d: Vec<String> = [
            "2026-09-01",
            "2026-09-02",
            "2026-09-03",
            "2026-09-07",
            "2026-09-30",
            "2026-10-01",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        assert_eq!(
            rangos_de_dias(&d),
            vec![
                ("2026-09-01".to_owned(), "2026-09-03".to_owned()),
                ("2026-09-07".to_owned(), "2026-09-07".to_owned()),
                ("2026-09-30".to_owned(), "2026-10-01".to_owned()),
            ]
        );
        assert_eq!(
            esc("<a href=\"x\">&'"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
    }
}
