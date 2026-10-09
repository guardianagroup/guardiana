use super::*;

use std::sync::atomic::{AtomicU64, Ordering};

static CARPETA: AtomicU64 = AtomicU64::new(0);

fn reloj() -> i64 {
    // 2026-10-09T13:39:31Z
    1_791_553_171_000
}

fn carpeta() -> PathBuf {
    let n = CARPETA.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("guardiana-zero-sesion-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    let _ = fs::create_dir_all(d.join("descargas"));
    d
}

fn abre() -> (Sesion, Vec<Orden>, PathBuf) {
    let d = carpeta();
    let (s, o) = Sesion::abre(Arranque {
        datos: d.join("datos"),
        descargas: d.join("descargas"),
        idioma_sistema: "es-CO".into(),
        version: "0.1.0".into(),
        reloj,
    });
    (s, o, d)
}

fn mensajes(o: &[Orden], a: Origen) -> Vec<Value> {
    o.iter()
        .filter_map(|x| match x {
            Orden::Envia { a: b, json } if *b == a => serde_json::from_str(json).ok(),
            _ => None,
        })
        .collect()
}

fn del_tipo(o: &[Orden], a: Origen, tipo: &str) -> Option<Value> {
    mensajes(o, a).into_iter().rev().find(|m| m["tipo"] == tipo)
}

/// The engine's order for a page the tab opens: the navigation starts, its request leaves, the
/// page arrives. Returns the answer to the page's own request.
fn abre_pagina(s: &mut Sesion, id: u32, url: &str) -> Respuesta {
    let (cancela, _) = s.navegacion_empieza(id, url, false);
    assert!(!cancela);
    let r = s.peticion(id, url, "GET", b"", 1);
    let _ = s.pagina_nueva(id, url);
    r
}

/// A tab with a web page loaded, the shield answered on first run.
fn con_pagina(cortar: bool) -> (Sesion, PathBuf, u32) {
    let (mut s, _, d) = abre();
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        &json!({ "tipo": "bienvenida", "cortar": cortar }).to_string(),
    );
    let id = s.activa();
    assert!(!abre_pagina(&mut s, id, "https://www.eltiempo.com/").cortar);
    (s, d, id)
}

#[test]
fn the_first_run_opens_a_new_tab_and_asks_the_question() {
    let (mut s, o, _) = abre();
    assert!(matches!(
        o.first(),
        Some(Orden::CreaPestana { id: 1, perfil: Perfil::General, url: Some(u), de: None }) if u == INICIO
    ));
    assert!(o.contains(&Orden::Activa { id: 1 }));
    let o = s.mensaje(
        Origen::Barra,
        BARRA,
        r#"{"tipo":"listo","vista":"barra","zona":-300}"#,
    );
    let t = del_tipo(&o, Origen::Barra, "textos").unwrap_or_default();
    assert_eq!(t["idioma"], "es");
    assert_eq!(t["textos"]["app_nombre"], "GUARDIANA ZERO");
    assert!(o.contains(&Orden::Panel { abierto: true }));
    assert_eq!(
        del_tipo(&o, Origen::Panel, "vista").unwrap_or_default()["vista"],
        "bienvenida"
    );
    let e = del_tipo(&o, Origen::Barra, "estado").unwrap_or_default();
    assert_eq!(e["pestanas"][0]["titulo"], "Nueva pestaña");
    assert_eq!(e["activa"]["interna"], true);
    // Answered once, never asked again.
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"bienvenida","cortar":true}"#,
    );
    assert!(o.contains(&Orden::Seguimiento { estricto: true }));
    assert!(o.contains(&Orden::Panel { abierto: false }));
    let o = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"listo","vista":"barra"}"#);
    assert!(!o.contains(&Orden::Panel { abierto: true }));
}

#[test]
fn web_pages_cannot_command_the_browser() {
    let (mut s, _, id) = con_pagina(true);
    let web = "https://www.eltiempo.com/";
    for m in [
        r#"{"tipo":"borrar_todo"}"#,
        r#"{"tipo":"navegar","texto":"evil.example"}"#,
        r#"{"tipo":"tinta_anadir","dato":"correo","valor":"x@y.co"}"#,
        r#"{"tipo":"panel","vista":"datos"}"#,
    ] {
        assert!(s.mensaje(Origen::Pestana(id), web, m).is_empty());
        // Not from a web page pretending to be the bar either.
        assert!(s.mensaje(Origen::Barra, web, m).is_empty());
    }
    // The new tab may search and open the panel, not erase everything.
    let o = s.mensaje(
        Origen::Pestana(id),
        INICIO,
        r#"{"tipo":"navegar","texto":"real madrid"}"#,
    );
    assert!(o.contains(&Orden::Navega {
        id,
        url: "https://duckduckgo.com/?q=real%20madrid".into()
    }));
    assert!(s
        .mensaje(Origen::Pestana(id), INICIO, r#"{"tipo":"borrar_todo"}"#)
        .is_empty());
    assert!(s
        .mensaje(
            Origen::Pestana(id),
            INICIO,
            r#"{"tipo":"ajuste","clave":"cortar_seguimiento","valor":false}"#
        )
        .is_empty());
    // And the browser never posts to a tab showing a web page.
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"ajuste","clave":"idioma","valor":"en"}"#,
    );
    assert!(mensajes(&o, Origen::Pestana(id)).is_empty());
    assert_eq!(
        del_tipo(&o, Origen::Barra, "textos").unwrap_or_default()["idioma"],
        "en"
    );
}

#[test]
fn trackers_are_cut_and_the_shield_says_what_holds_now() {
    let (mut s, _, id) = con_pagina(true);
    let r = s.peticion(
        id,
        "https://stats.g.doubleclick.net/g/collect",
        "GET",
        b"",
        3,
    );
    assert!(r.cortar);
    assert!(r.pagina.is_none());
    let r = s.peticion(id, "https://img.eltiempo.com/a.png", "GET", b"", 3);
    assert!(!r.cortar);
    let o = s.tic();
    let pulsos = del_tipo(&o, Origen::Barra, "pulsos").unwrap_or_default();
    assert_eq!(pulsos["lista"][0]["cortado"], true);
    let e = del_tipo(&o, Origen::Barra, "escudo").unwrap_or_default();
    assert_eq!(e["sitio"], "eltiempo.com");
    assert_eq!(e["resumen"]["terceros_cortados"], 1);
    let fila = &e["terceros"][0];
    assert_eq!(fila["sitio"], "doubleclick.net");
    assert_eq!(fila["ahora"], "cortado");
    assert_eq!(fila["regla"], "lista");
    // Let it through: the same row now says it passes, with its history kept.
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"permitir_sitio","sitio":"doubleclick.net"}"#,
    );
    let e = del_tipo(&o, Origen::Panel, "escudo").unwrap_or_default();
    assert_eq!(e["terceros"][0]["ahora"], "pasa");
    assert_eq!(e["terceros"][0]["regla"], "permitido");
    assert_eq!(e["terceros"][0]["cortadas"], 1);
    let r = s.peticion(
        id,
        "https://stats.g.doubleclick.net/g/collect",
        "GET",
        b"",
        3,
    );
    assert!(!r.cortar);
    // The settings survive a restart.
    let datos = s.datos.clone();
    s.cierra();
    let p: Preferencias = lee(&datos.join("preferencias.json"));
    assert!(p.reglas.permitidos.contains("doubleclick.net"));
    assert!(p.bienvenida);
}

#[test]
fn a_tracker_frame_is_a_frame_not_the_page() {
    let (mut s, _, id) = con_pagina(true);
    // Context 1 (document) for an address that is not the tab's own navigation: a frame.
    let r = s.peticion(
        id,
        "https://googleads.g.doubleclick.net/pagead/ads?x=1",
        "GET",
        b"",
        1,
    );
    assert!(r.cortar);
    assert!(r.pagina.is_none());
    // The tab's own navigation to a site is never cut by a list.
    assert!(!abre_pagina(&mut s, id, "https://doubleclick.net/").cortar);
}

#[test]
fn a_site_you_cut_shows_why_instead_of_the_page() {
    let (mut s, _, id) = con_pagina(false);
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"cortar_sitio","sitio":"ejemplo.com"}"#,
    );
    let r = abre_pagina(&mut s, id, "https://www.ejemplo.com/a");
    assert!(r.cortar);
    let html = r.pagina.unwrap_or_default();
    assert!(html.contains("Cortaste ejemplo.com"));
    assert!(html.contains("Cortado por GUARDIANA ZERO"));
}

#[test]
fn tracking_parameters_are_removed_before_the_page_opens() {
    let (mut s, _, id) = con_pagina(false);
    let sucia = "https://elpais.com/x?utm_source=tw&utm_medium=s&id=4";
    // The tagged address is answered here with a page that reopens it clean.
    let r = abre_pagina(&mut s, id, sucia);
    assert!(r.cortar);
    let reabre = r.pagina.unwrap_or_default();
    assert!(reabre.contains(r#"location.replace("https://elpais.com/x?id=4")"#));
    // Its short visit is not a page, and the shield still shows the page that was there.
    let o = s.tic();
    assert_eq!(
        del_tipo(&o, Origen::Barra, "escudo").unwrap_or_default()["sitio"],
        "eltiempo.com"
    );
    // A form posted to a tagged address goes as it is: cleaning never turns a post into a get.
    let (_, _) = s.navegacion_empieza(id, "https://elpais.com/form?utm_source=x", false);
    let r = s.peticion(
        id,
        "https://elpais.com/form?utm_source=x",
        "POST",
        b"a=1",
        1,
    );
    assert!(!r.cortar);
    assert!(!abre_pagina(&mut s, id, "https://elpais.com/x?id=4").cortar);
    // A server redirect to a tagged address is cancelled and opened clean.
    let (cancela, o) = s.navegacion_empieza(id, "https://elpais.com/y?gclid=1", true);
    assert!(cancela);
    assert_eq!(
        o,
        vec![Orden::Navega {
            id,
            url: "https://elpais.com/y".into()
        }]
    );
    let o = s.tic();
    let e = del_tipo(&o, Origen::Barra, "escudo").unwrap_or_default();
    assert_eq!(e["parametros_quitados"], 2);
    assert_eq!(e["sitio"], "elpais.com");
    assert_eq!(
        s.pestana(id).map(|p| p.url.clone()).unwrap_or_default(),
        "https://elpais.com/x?id=4"
    );
    assert_eq!(e["terceros"].as_array().map(Vec::len), Some(0));
    assert_eq!(s.msg_hoy()["hoy"]["empresas"], 0);
}

#[test]
fn marked_data_never_reaches_a_third_party_and_the_book_says_who_got_it() {
    let (mut s, _, id) = con_pagina(false);
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"tinta_anadir","dato":"correo","valor":"ana@correo.co"}"#,
    );
    let t = del_tipo(&o, Origen::Panel, "tinta").unwrap_or_default();
    assert_eq!(t["lista"][0]["visible"], "an•••@correo.co");
    assert!(s.necesita_cuerpo(id));
    let r = s.peticion(
        id,
        "https://collect.otra-empresa.io/e",
        "POST",
        br#"{"email":"ana@correo.co"}"#,
        8,
    );
    assert!(r.cortar);
    // Hashed the way ad networks pass it, too.
    let sha = {
        use sha2::{Digest, Sha256};
        Sha256::digest(b"ana@correo.co")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let r = s.peticion(
        id,
        &format!("https://px.otra-empresa.io/p?em={sha}"),
        "GET",
        b"",
        3,
    );
    assert!(r.cortar);
    // To the page's own site it goes, and it is written down once.
    for _ in 0..3 {
        let r = s.peticion(
            id,
            "https://www.eltiempo.com/login",
            "POST",
            b"email=ana%40correo.co",
            8,
        );
        assert!(!r.cortar);
    }
    let e = &s.libro.entradas["eltiempo.com"];
    assert_eq!(e.veces, 1);
    assert_eq!(e.clases, vec![Clase::Correo]);
    let o = s.tic();
    let e = del_tipo(&o, Origen::Barra, "escudo").unwrap_or_default();
    assert_eq!(e["datos_salvados"], 2);
}

#[test]
fn a_mandate_keeps_its_tab_inside_and_signs_what_happened() {
    let (mut s, d, _) = con_pagina(false);
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"mandato_empezar","tarea":"Busca un vuelo","webs":["avianca.com"],"estricto":true}"#,
    );
    let Some(Orden::CreaPestana {
        id: m,
        perfil: Perfil::Mandato,
        url: Some(u),
        ..
    }) = o.first().cloned()
    else {
        unreachable!("the mandate opens its own tab: {o:?}");
    };
    assert_eq!(u, "https://avianca.com/");
    let estado = del_tipo(&o, Origen::Panel, "mandato").unwrap_or_default();
    assert_eq!(estado["estado"], "activo");
    let senuelo = estado["mandato"]["senuelo"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(senuelo.ends_with("@guardiana-zero.invalid"));
    assert!(!abre_pagina(&mut s, m, "https://www.avianca.com/").cortar);
    // A page that talks the AI into going elsewhere: the tab does not go.
    let r = abre_pagina(&mut s, m, "https://evil.example/robar");
    assert!(r.cortar);
    assert!(r.pagina.unwrap_or_default().contains("evil.example"));
    // The decoy leaving, even to an allowed site, is caught.
    let r = s.peticion(
        m,
        "https://www.avianca.com/api",
        "POST",
        format!("{{\"email\":\"{senuelo}\"}}").as_bytes(),
        8,
    );
    assert!(r.cortar);
    // Other tabs are not limited by the mandate.
    let _ = s.peticion(1, "https://evil.example/", "GET", b"", 3);
    let o = s.mensaje(Origen::Panel, PANEL, r#"{"tipo":"mandato_terminar"}"#);
    assert!(o.contains(&Orden::CierraPestana { id: m }));
    let fin = del_tipo(&o, Origen::Panel, "mandato").unwrap_or_default();
    assert_eq!(fin["estado"], "terminado");
    assert_eq!(fin["cuentas"]["visitas"], 1);
    assert_eq!(fin["cuentas"]["cortes"], 2);
    let r: Recibo =
        serde_json::from_value(fin["recibo"].clone()).unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(recibo::comprueba(&r), Ok(()));
    assert!(r.contenido.contains("Busca un vuelo"));
    // Kept in the data folder, and saved to Downloads when asked.
    let guardados = fs::read_dir(d.join("datos").join("recibos"))
        .map(Iterator::count)
        .unwrap_or(0);
    assert_eq!(guardados, 1);
    let o = s.mensaje(Origen::Panel, PANEL, r#"{"tipo":"recibo_guardar"}"#);
    let g = del_tipo(&o, Origen::Panel, "guardado").unwrap_or_default();
    let ruta = PathBuf::from(g["ruta"].as_str().unwrap_or_default());
    assert!(ruta.starts_with(d.join("descargas")));
    assert!(ruta.exists());
    // The receipt says what it could not see.
    assert!(r.contenido.contains("otras_aplicaciones_del_equipo"));
}

#[test]
fn a_mandate_ends_on_its_own_when_its_time_is_up() {
    static AHORA: AtomicU64 = AtomicU64::new(1_791_553_171_000);
    fn reloj_movil() -> i64 {
        i64::try_from(AHORA.load(Ordering::SeqCst)).unwrap_or(0)
    }
    let d = carpeta();
    let (mut s, _) = Sesion::abre(Arranque {
        datos: d.join("datos"),
        descargas: d.join("descargas"),
        idioma_sistema: "es".into(),
        version: "0.1.0".into(),
        reloj: reloj_movil,
    });
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"mandato_empezar","tarea":"Compara precios","webs":["exito.com"],"estricto":true,"minutos":15}"#,
    );
    let Some(Orden::CreaPestana { id: m, .. }) = o.first().cloned() else {
        unreachable!("{o:?}");
    };
    AHORA.fetch_add(14 * 60_000, Ordering::SeqCst);
    assert!(!s.tic().contains(&Orden::CierraPestana { id: m }));
    AHORA.fetch_add(2 * 60_000, Ordering::SeqCst);
    let o = s.tic();
    assert!(o.contains(&Orden::CierraPestana { id: m }));
    assert_eq!(
        del_tipo(&o, Origen::Panel, "mandato").unwrap_or_default()["estado"],
        "terminado"
    );
    assert!(del_tipo(&o, Origen::Panel, "aviso").is_some());
}

#[test]
fn the_form_guard_asks_once_per_site_and_kind() {
    let (mut s, _, id) = con_pagina(false);
    let web = "https://www.tienda.co/registro";
    let _ = abre_pagina(&mut s, id, web);
    let pide = |s: &mut Sesion, n: u64| {
        s.mensaje(
            Origen::Pestana(id),
            web,
            &json!({ "tipo": "zg_formulario", "zg": "f1", "id": n, "accion": "https://www.tienda.co/alta", "valores": ["Ana", "ana@correo.co", "3001234567"] })
                .to_string(),
        )
    };
    let o = pide(&mut s, 1);
    let q = del_tipo(&o, Origen::Panel, "pregunta_formulario").unwrap_or_default();
    assert_eq!(q["sitio"], "tienda.co");
    assert_eq!(q["datos"], json!(["dato_correo", "dato_telefono"]));
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        &json!({ "tipo": "formulario_respuesta", "id": q["id"], "enviar": true, "recordar": false })
            .to_string(),
    );
    let Some(Orden::RespondeFormulario { id: t, json }) = o.first() else {
        unreachable!("the answer goes back to the page: {o:?}");
    };
    assert_eq!(*t, id);
    assert_eq!(json, r#"{"enviar":true,"id":1,"zg":"f1"}"#);
    assert_eq!(
        s.libro.entradas["tienda.co"].clases,
        vec![Clase::Correo, Clase::Telefono]
    );
    // Same site, same kinds: it already has them, no question.
    let o = pide(&mut s, 2);
    assert!(del_tipo(&o, Origen::Panel, "pregunta_formulario").is_none());
    assert!(
        matches!(o.first(), Some(Orden::RespondeFormulario { json, .. }) if json.contains(r#""id":2"#))
    );
    // A search box is not personal data: it goes without a question.
    let o = s.mensaje(
        Origen::Pestana(id),
        web,
        r#"{"tipo":"zg_formulario","zg":"f1","id":3,"accion":"https://www.tienda.co/buscar","valores":["zapatos rojos"]}"#,
    );
    assert!(
        matches!(o.first(), Some(Orden::RespondeFormulario { json, .. }) if json.contains(r#""id":3"#))
    );
}

#[test]
fn redaction_hides_data_from_the_ai_and_puts_it_back_only_in_the_panel() {
    let (mut s, _, id) = con_pagina(false);
    let _ = abre_pagina(&mut s, id, "https://chat.ia.example/");
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"tachar","texto":"Escribe a ana@correo.co y llama al 3001234567"}"#,
    );
    let t = del_tipo(&o, Origen::Panel, "tachado").unwrap_or_default();
    let texto = t["texto"].as_str().unwrap_or_default().to_string();
    assert!(!texto.contains("ana@correo.co"));
    assert!(!texto.contains("3001234567"));
    assert_eq!(t["sustituciones"].as_array().map(Vec::len), Some(2));
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        &json!({ "tipo": "tachon_pegar", "texto": texto }).to_string(),
    );
    assert!(o.contains(&Orden::Foco {
        a: Origen::Pestana(id)
    }));
    assert!(
        matches!(&o[1], Orden::Ejecuta { etiqueta: Some(Etiqueta::Pegar), script, .. } if script.contains("execCommand"))
    );
    let marca = t["sustituciones"][0]["marca"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let o = s.script_hecho(
        Etiqueta::Seleccion,
        &serde_json::to_string(&format!("Listo, escribí a {marca}.")).unwrap_or_default(),
    );
    let r = del_tipo(&o, Origen::Panel, "restaurado").unwrap_or_default();
    assert_eq!(r["texto"], "Listo, escribí a ana@correo.co.");
    let o = s.script_hecho(Etiqueta::Pegar, "\"sin_campo\"");
    assert!(del_tipo(&o, Origen::Panel, "aviso").is_some());
}

#[test]
fn keys_tabs_and_new_windows() {
    let (mut s, _, _) = abre();
    let o = s
        .tecla(u32::from('T'), true, false, false)
        .unwrap_or_default();
    assert!(matches!(o.first(), Some(Orden::CreaPestana { id: 2, .. })));
    assert!(o.contains(&Orden::Foco { a: Origen::Barra }));
    let (n, o) = s.ventana_nueva(2).unwrap_or_default();
    assert_eq!(n, 3);
    assert!(o.contains(&Orden::CreaPestana {
        id: 3,
        perfil: Perfil::General,
        url: None,
        de: Some(2)
    }));
    let o = s.tecla(0x09, true, false, false).unwrap_or_default();
    assert!(o.contains(&Orden::Activa { id: 1 }));
    let o = s
        .tecla(u32::from('9'), true, false, false)
        .unwrap_or_default();
    assert!(o.contains(&Orden::Activa { id: 3 }));
    assert!(s.tecla(u32::from('K'), false, false, false).is_none());
    let _ = s.tecla(u32::from('W'), true, false, false);
    let _ = s.tecla(u32::from('W'), true, false, false);
    let o = s
        .tecla(u32::from('W'), true, false, false)
        .unwrap_or_default();
    assert!(o.contains(&Orden::CierraVentana));
    // An isolated tab keeps its own session, and so do the windows it opens.
    let (mut s, _, _) = abre();
    let o = s
        .tecla(u32::from('N'), true, true, false)
        .unwrap_or_default();
    assert!(matches!(
        o.first(),
        Some(Orden::CreaPestana {
            perfil: Perfil::Aislada,
            ..
        })
    ));
    let (_, o) = s.ventana_nueva(2).unwrap_or_default();
    assert!(matches!(
        o.first(),
        Some(Orden::CreaPestana {
            perfil: Perfil::Aislada,
            ..
        })
    ));
}

#[test]
fn the_month_image_is_saved_to_downloads_only_as_a_png() {
    let (mut s, _, d) = abre();
    let png = base64::engine::general_purpose::STANDARD.encode(b"\x89PNG\r\n\x1a\nresto");
    let o = s.mensaje(
        Origen::Pestana(1),
        INICIO,
        &json!({ "tipo": "guardar_imagen", "nombre": "../../x.exe", "datos": format!("data:image/png;base64,{png}") })
            .to_string(),
    );
    let g = del_tipo(&o, Origen::Pestana(1), "guardado").unwrap_or_default();
    let ruta = PathBuf::from(g["ruta"].as_str().unwrap_or_default());
    assert_eq!(ruta.parent(), Some(d.join("descargas").as_path()));
    assert_eq!(
        ruta.file_name().and_then(|f| f.to_str()),
        Some("guardiana-zero-2026-10.png")
    );
    let o = s.mensaje(
        Origen::Pestana(1),
        INICIO,
        r#"{"tipo":"guardar_imagen","datos":"data:text/html;base64,PGI+"}"#,
    );
    assert!(o.is_empty());
}

#[test]
fn today_counts_what_was_decided() {
    let (mut s, _, id) = con_pagina(true);
    let _ = s.peticion(
        id,
        "https://stats.g.doubleclick.net/g/collect",
        "GET",
        b"",
        3,
    );
    let _ = s.peticion(
        id,
        "https://www.google-analytics.com/g/collect",
        "POST",
        b"",
        8,
    );
    let h = s.msg_hoy();
    assert_eq!(h["hoy"]["cortadas"], 2);
    assert_eq!(h["mes"]["terceros_cortados"], 2);
    assert_eq!(h["mes"]["empresas_cortadas"], 1);
    assert_eq!(h["motor"], "DuckDuckGo");
    // Erasing everything starts the figures again, in a fresh tab.
    let o = s.mensaje(Origen::Panel, PANEL, r#"{"tipo":"borrar_todo"}"#);
    assert!(o.contains(&Orden::BorraNavegacion));
    assert!(o.contains(&Orden::CierraPestana { id }));
    assert!(!o.contains(&Orden::CierraVentana));
    assert_eq!(s.msg_hoy()["hoy"]["cortadas"], 0);
}

#[test]
fn a_tab_shows_its_site_icon_until_it_leaves_the_site() {
    let (mut s, _, id) = con_pagina(false);
    let png = b"\x89PNG\r\n\x1a\nicono";
    let o = s.icono(id, png);
    let e = del_tipo(&o, Origen::Barra, "estado").unwrap_or_default();
    assert!(e["pestanas"][0]["icono"]
        .as_str()
        .unwrap_or_default()
        .starts_with("data:image/png;base64,"));
    // Anything that is not a PNG is not shown.
    let o = s.icono(id, b"<svg onload=alert(1)>");
    assert_eq!(
        del_tipo(&o, Origen::Barra, "estado").unwrap_or_default()["pestanas"][0]["icono"],
        ""
    );
    let _ = s.icono(id, png);
    let _ = s.navegacion_empieza(id, "https://www.eltiempo.com/deportes", false);
    let o = s.pagina_nueva(id, "https://www.eltiempo.com/deportes");
    assert_ne!(
        del_tipo(&o, Origen::Barra, "estado").unwrap_or_default()["pestanas"][0]["icono"],
        ""
    );
    let _ = s.navegacion_empieza(id, "https://elpais.com/", false);
    let o = s.pagina_nueva(id, "https://elpais.com/");
    assert_eq!(
        del_tipo(&o, Origen::Barra, "estado").unwrap_or_default()["pestanas"][0]["icono"],
        ""
    );
}

#[test]
fn masks_show_enough_to_recognise() {
    assert_eq!(
        enmascara(Tipo::Correo, "francisco@icloud.com"),
        "fr•••@icloud.com"
    );
    assert_eq!(enmascara(Tipo::Telefono, "+573001234567"), "+5•••67");
    assert_eq!(enmascara(Tipo::Nombre, "Ana"), "•••");
}

#[test]
fn the_bar_shows_an_address_only_once_the_page_is_there() {
    let (mut s, _, id) = con_pagina(false);
    // A page starts a navigation that never arrives (stopped, a 204, a download).
    let (_, o) = s.navegacion_empieza(id, "https://accounts.google.com/", false);
    let e = del_tipo(&o, Origen::Barra, "estado").unwrap_or_default();
    assert_eq!(e["activa"]["url"], "https://www.eltiempo.com/");
    // Meanwhile what the old page sends is still the old page's.
    let r = s.peticion(id, "https://www.eltiempo.com/api", "POST", b"x", 8);
    assert!(!r.cortar);
    let o = s.tic();
    let e = del_tipo(&o, Origen::Barra, "escudo").unwrap_or_default();
    assert_eq!(e["sitio"], "eltiempo.com");
    assert_eq!(e["terceros"].as_array().map(Vec::len), Some(0));
}

#[test]
fn a_page_cannot_carry_marked_data_away_by_opening_another_site() {
    let (mut s, _, id) = con_pagina(false);
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"tinta_anadir","dato":"correo","valor":"ana@correo.co"}"#,
    );
    let r = abre_pagina(&mut s, id, "https://tracker.example/r?e=ana%40correo.co");
    assert!(r.cortar);
    assert!(r.pagina.unwrap_or_default().contains("tracker.example"));
    // What you type yourself is yours to send.
    let o = s.mensaje(
        Origen::Barra,
        BARRA,
        r#"{"tipo":"navegar","texto":"https://tracker.example/r?e=ana%40correo.co"}"#,
    );
    assert!(o.iter().any(|x| matches!(x, Orden::Navega { .. })));
    assert!(!abre_pagina(&mut s, id, "https://tracker.example/r?e=ana%40correo.co").cortar);
}

#[test]
fn websockets_unknown_tabs_and_rented_names_are_checked_too() {
    let (mut s, _, id) = con_pagina(false);
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"tinta_anadir","dato":"correo","valor":"ana@correo.co"}"#,
    );
    assert!(
        s.peticion(
            id,
            "wss://ws.otra-empresa.io/s?u=ana@correo.co",
            "GET",
            b"",
            11
        )
        .cortar
    );
    // A tab the session does not know sends nothing.
    assert!(
        s.peticion(999, "https://www.eltiempo.com/", "GET", b"", 1)
            .cortar
    );
    // Two tenants of one shared service are two parties.
    let _ = abre_pagina(&mut s, id, "https://ana.github.io/");
    assert!(
        s.peticion(id, "https://luis.github.io/c", "POST", b"ana@correo.co", 8)
            .cortar
    );
}

#[test]
fn the_decoy_is_caught_hashed_too() {
    let (mut s, _, id) = con_pagina(false);
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"mandato_empezar","tarea":"","webs":["avianca.com"],"estricto":true}"#,
    );
    let Some(Orden::CreaPestana { id: m, .. }) = o.first().cloned() else {
        unreachable!("{o:?}");
    };
    let senuelo = s
        .mandato
        .as_ref()
        .map(|x| x.senuelo.clone())
        .unwrap_or_default();
    let _ = abre_pagina(&mut s, m, "https://www.avianca.com/");
    let sha = {
        use sha2::{Digest, Sha256};
        Sha256::digest(senuelo.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let r = s.peticion(
        m,
        &format!("https://www.avianca.com/p?h={sha}"),
        "GET",
        b"",
        3,
    );
    assert!(r.cortar);
    // Even the page's own site taking it is in the cut list, so «datos detenidos» on the new
    // tab and the list it opens always say the same.
    assert_eq!(s.msg_hoy()["hoy"]["datos_salvados"], 1);
    let _ = s.navegacion_empieza(id, CORTES, false);
    let _ = s.pagina_nueva(id, CORTES);
    let o = s.mensaje(
        Origen::Pestana(id),
        CORTES,
        r#"{"tipo":"cortes","periodo":"hoy"}"#,
    );
    let c = del_tipo(&o, Origen::Pestana(id), "cortes").unwrap_or_default();
    let de_dato = c["lista"]
        .as_array()
        .map(|l| {
            l.iter()
                .filter(|x| x["motivo"] == "senuelo" || x["motivo"] == "tinta")
                .count()
        })
        .unwrap_or_default();
    assert_eq!(de_dato, 1);
    assert_eq!(c["lista"][0]["mandato"], true);
}

#[test]
fn questions_end_with_their_page_and_erasing_forgets_the_exceptions() {
    let (mut s, _, id) = con_pagina(false);
    let web = "https://www.tienda.co/registro";
    let _ = abre_pagina(&mut s, id, web);
    let o = s.mensaje(
        Origen::Pestana(id),
        web,
        r#"{"tipo":"zg_formulario","zg":"a","id":1,"accion":"https://www.tienda.co/alta","valores":["ana@correo.co"]}"#,
    );
    let q = del_tipo(&o, Origen::Panel, "pregunta_formulario").unwrap_or_default();
    assert_eq!(q["pestana"], "www.tienda.co");
    // The page leaves before the answer: nothing is approved for the next one.
    let o = s.pagina_nueva(id, "https://www.tienda.co/otra");
    assert!(o.contains(&Orden::Panel { abierto: false }));
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        &json!({ "tipo": "formulario_respuesta", "id": q["id"], "enviar": true, "recordar": true })
            .to_string(),
    );
    assert!(o.is_empty());
    // «No volver a preguntar» can be undone, and erasing everything forgets it.
    s.prefs.sin_preguntar.insert("tienda.co".into());
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"preguntar_otra_vez","sitio":"tienda.co"}"#,
    );
    assert_eq!(
        del_tipo(&o, Origen::Panel, "ajustes").unwrap_or_default()["sin_preguntar"],
        json!([])
    );
    s.prefs.sin_preguntar.insert("otra.co".into());
    let _ = s.mensaje(Origen::Panel, PANEL, r#"{"tipo":"borrar_todo"}"#);
    assert!(s.prefs.sin_preguntar.is_empty());
}

#[test]
fn every_cut_can_be_examined_one_by_one_and_saved() {
    let (mut s, d, id) = con_pagina(true);
    let _ = s.peticion(
        id,
        "https://stats.g.doubleclick.net/g/collect?cid=9",
        "GET",
        b"",
        3,
    );
    let _ = s.peticion(
        id,
        "https://www.google-analytics.com/g/collect",
        "POST",
        b"",
        8,
    );
    let _ = s.peticion(id, "https://img.eltiempo.com/a.png", "GET", b"", 3);
    // The same count the new tab shows.
    assert_eq!(s.msg_hoy()["hoy"]["cortadas"], 2);
    // The page «Lo que se cortó» opens in the tab.
    let _ = s.navegacion_empieza(id, CORTES, false);
    let _ = s.pagina_nueva(id, CORTES);
    let o = s.mensaje(
        Origen::Pestana(id),
        CORTES,
        r#"{"tipo":"cortes","periodo":"hoy"}"#,
    );
    let c = del_tipo(&o, Origen::Pestana(id), "cortes").unwrap_or_default();
    assert_eq!(c["total"], 2);
    assert_eq!(c["empresas"][0]["quien"], "Google");
    assert_eq!(c["empresas"][0]["n"], 2);
    assert_eq!(c["lista"][0]["pagina"], "eltiempo.com");
    assert_eq!(c["lista"][0]["ruta"], "/g/collect");
    assert!(!c.to_string().contains("cid=9"));
    // Saved as CSV in Downloads, and as PDF by the engine.
    let o = s.mensaje(
        Origen::Pestana(id),
        CORTES,
        r#"{"tipo":"exportar_cortes","periodo":"hoy","formato":"csv"}"#,
    );
    let g = del_tipo(&o, Origen::Pestana(id), "guardado").unwrap_or_default();
    let ruta = PathBuf::from(g["ruta"].as_str().unwrap_or_default());
    assert_eq!(ruta.parent(), Some(d.join("descargas").as_path()));
    let csv = fs::read_to_string(&ruta).unwrap_or_default();
    assert!(csv.contains("Google"));
    assert_eq!(csv.lines().count(), 3);
    let o = s.mensaje(
        Origen::Pestana(id),
        CORTES,
        r#"{"tipo":"exportar_cortes","periodo":"7","formato":"pdf"}"#,
    );
    assert!(
        matches!(o.first(), Some(Orden::GuardaPdf { ruta, .. }) if ruta.extension().is_some_and(|e| e == "pdf"))
    );
    // A web page cannot ask for it.
    assert!(s
        .mensaje(
            Origen::Pestana(id),
            "https://evil.example/",
            r#"{"tipo":"cortes","periodo":"hoy"}"#
        )
        .is_empty());
}

/// When the seven days end without a subscription the browser keeps opening pages, and does
/// nothing of its own: no cut, no cleaning, no question, no mandate (decided 9 Oct 2026). A key
/// that is activated brings everything back.
#[test]
fn after_the_trial_it_browses_without_protection_until_a_key_is_activated() {
    let (mut s, _, id) = con_pagina(true);
    let o = s.pon_licencia(EstadoLicencia::Prueba {
        termina: reloj() + 86_400_000,
        dias: 1,
    });
    let l = del_tipo(&o, Origen::Barra, "licencia").unwrap_or_default();
    assert_eq!(l["estado"], "prueba");
    assert_eq!(l["dias"], 1);
    assert!(
        s.peticion(
            id,
            "https://stats.g.doubleclick.net/g/collect",
            "GET",
            b"",
            3
        )
        .cortar
    );

    let o = s.pon_licencia(EstadoLicencia::PruebaTerminada { desde: reloj() });
    assert!(o.contains(&Orden::Seguimiento { estricto: false }));
    assert!(!s.seguimiento_estricto());
    let l = del_tipo(&o, Origen::Barra, "licencia").unwrap_or_default();
    assert_eq!(l["estado"], "prueba_terminada");
    assert_eq!(l["protege"], false);
    let e = del_tipo(&o, Origen::Panel, "escudo").unwrap_or_default();
    assert_eq!(e["cortando"], false);
    // Pages open; nothing is cut, no address is cleaned.
    assert!(
        !s.peticion(
            id,
            "https://stats.g.doubleclick.net/g/collect",
            "GET",
            b"",
            3
        )
        .cortar
    );
    let (cancela, _) = s.navegacion_empieza(id, "https://example.com/?utm_source=x", true);
    assert!(!cancela);
    assert!(
        !s.peticion(id, "https://example.com/?utm_source=x", "GET", b"", 1)
            .cortar
    );
    // The paid tools open the subscription view instead.
    for m in [
        r#"{"tipo":"mandato_empezar","tarea":"x","webs":["avianca.com"],"estricto":true}"#,
        r#"{"tipo":"tachar","texto":"hola"}"#,
        r#"{"tipo":"tinta_anadir","dato":"correo","valor":"ana@example.com"}"#,
    ] {
        let o = s.mensaje(Origen::Panel, PANEL, m);
        assert!(!o.iter().any(|x| matches!(x, Orden::CreaPestana { .. })));
        let v = del_tipo(&o, Origen::Panel, "vista").unwrap_or_default();
        assert_eq!(v["vista"], "licencia", "{m}");
    }
    let o = s.mensaje(
        Origen::Barra,
        BARRA,
        r#"{"tipo":"panel","vista":"mandato"}"#,
    );
    assert_eq!(
        del_tipo(&o, Origen::Panel, "vista").unwrap_or_default()["vista"],
        "licencia"
    );
    // A web page cannot ask for an activation; the panel can, and the shell does it.
    assert!(s
        .mensaje(
            Origen::Pestana(id),
            "https://example.com/",
            r#"{"tipo":"licencia_activar","clave":"K"}"#
        )
        .is_empty());
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"licencia_activar","clave":"  "}"#,
    );
    let l = del_tipo(&o, Origen::Panel, "licencia").unwrap_or_default();
    assert!(l["error"].as_str().unwrap_or_default().contains("clave"));
    assert!(!o.iter().any(|x| matches!(x, Orden::ActivaLicencia { .. })));
    // Without a place for the licence (as in these tests) nothing is sent either.
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"licencia_activar","clave":"ABC-123"}"#,
    );
    assert!(!o.iter().any(|x| matches!(x, Orden::ActivaLicencia { .. })));
    // Buying opens the shop in a tab.
    let o = s.mensaje(Origen::Panel, PANEL, r#"{"tipo":"licencia_comprar"}"#);
    assert!(o.iter().any(|x| matches!(x, Orden::CreaPestana { url: Some(u), .. } if u == "https://guardianagroup.com/comprar.html")));

    // An activation that failed says why; one that worked brings protection back.
    let o = s.licencia_activada(Err(Fallo::Limite));
    let l = del_tipo(&o, Origen::Panel, "licencia").unwrap_or_default();
    assert!(l["error"]
        .as_str()
        .unwrap_or_default()
        .contains("hola@guardianagroup.com"));
    let o = s.licencia_activada(Ok(EstadoLicencia::Suscrita {
        desde: reloj(),
        periodo: Some(30),
        proxima: Some(reloj() + 8 * 86_400_000),
        caduca: None,
        fallida: false,
    }));
    assert!(o.contains(&Orden::Seguimiento { estricto: true }));
    assert_eq!(
        del_tipo(&o, Origen::Panel, "licencia").unwrap_or_default()["error"],
        Value::Null
    );
    assert!(
        s.peticion(
            id,
            "https://stats.g.doubleclick.net/g/collect",
            "GET",
            b"",
            3
        )
        .cortar
    );
}

/// A mandate that is open when the trial ends is closed: its limits would no longer be enforced.
#[test]
fn a_mandate_open_when_the_trial_ends_is_closed() {
    let (mut s, _, _) = con_pagina(false);
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"mandato_empezar","tarea":"x","webs":["avianca.com"],"estricto":true}"#,
    );
    assert!(s.mandato.is_some());
    let o = s.pon_licencia(EstadoLicencia::PruebaTerminada { desde: reloj() });
    assert!(s.mandato.is_none());
    assert!(o.iter().any(|x| matches!(x, Orden::CierraPestana { .. })));
}
