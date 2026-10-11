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
    // Answered, the panel turns into the shield, live, beside the page.
    assert_eq!(
        del_tipo(&o, Origen::Panel, "vista").unwrap_or_default()["vista"],
        "escudo"
    );
    let o = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"listo","vista":"barra"}"#);
    assert!(del_tipo(&o, Origen::Panel, "vista").is_none_or(|v| v["vista"] != "bienvenida"));
}

#[test]
fn the_shield_opens_by_itself_until_the_person_closes_it() {
    let (mut s, _, _) = con_pagina(true);
    let datos = s.datos.clone();
    s.cierra();
    // The next start: the shield is open as soon as the bar is there.
    let (mut s, _) = Sesion::abre(Arranque {
        datos: datos.clone(),
        descargas: datos.join("descargas"),
        idioma_sistema: "es-CO".into(),
        version: "0.1.0".into(),
        reloj,
    });
    let o = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"listo","vista":"barra"}"#);
    assert!(o.contains(&Orden::Panel { abierto: true }));
    assert_eq!(
        del_tipo(&o, Origen::Panel, "vista").unwrap_or_default()["vista"],
        "escudo"
    );
    // Settings, closed: back to the shield, not to nothing.
    let _ = s.mensaje(
        Origen::Barra,
        BARRA,
        r#"{"tipo":"panel","vista":"ajustes"}"#,
    );
    let o = s.mensaje(Origen::Panel, PANEL, r#"{"tipo":"panel","vista":null}"#);
    assert_eq!(
        del_tipo(&o, Origen::Panel, "vista").unwrap_or_default()["vista"],
        "escudo"
    );
    // The shield, closed: closed, and still closed at the next start.
    let o = s.mensaje(Origen::Panel, PANEL, r#"{"tipo":"panel","vista":null}"#);
    assert!(o.contains(&Orden::Panel { abierto: false }));
    s.cierra();
    let (mut s, _) = Sesion::abre(Arranque {
        datos: datos.clone(),
        descargas: datos.join("descargas"),
        idioma_sistema: "es-CO".into(),
        version: "0.1.0".into(),
        reloj,
    });
    let o = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"listo","vista":"barra"}"#);
    assert!(!o.contains(&Orden::Panel { abierto: true }));
    // Opened again with the shield button: it opens by itself again from then on.
    let _ = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"panel","vista":"escudo"}"#);
    s.cierra();
    let p: Preferencias = lee(&datos.join("preferencias.json"));
    assert!(!p.escudo_cerrado);
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
    // Unblock it: the same row now says it passes, with its history kept.
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"desbloquear_sitio","sitio":"doubleclick.net"}"#,
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
fn block_and_unblock_undo_each_other_and_unblock_always_leaves_the_way_back() {
    let (mut s, _, id) = con_pagina(true);
    let _ = s.peticion(id, "https://stats.g.doubleclick.net/a", "GET", b"", 3);
    let _ = s.peticion(id, "https://cdn.otra.io/a.js", "GET", b"", 3);
    let fila = |s: &mut Sesion, sitio: &str| {
        let o = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"panel","vista":"escudo"}"#);
        let e = del_tipo(&o, Origen::Panel, "escudo").unwrap_or_default();
        let t = e["terceros"]
            .as_array()
            .and_then(|l| l.iter().find(|t| t["sitio"] == sitio).cloned())
            .unwrap_or_default();
        (
            t["ahora"].as_str().unwrap_or("").to_string(),
            t["regla"].clone(),
        )
    };
    let manda = |s: &mut Sesion, tipo: &str, sitio: &str| {
        let _ = s.mensaje(
            Origen::Panel,
            PANEL,
            &json!({ "tipo": tipo, "sitio": sitio }).to_string(),
        );
    };
    // A tracker: unblock, block again — back to the lists, no rule left.
    manda(&mut s, "desbloquear_sitio", "doubleclick.net");
    assert_eq!(
        fila(&mut s, "doubleclick.net"),
        ("pasa".into(), json!("permitido"))
    );
    manda(&mut s, "bloquear_sitio", "doubleclick.net");
    assert_eq!(
        fila(&mut s, "doubleclick.net"),
        ("cortado".into(), json!("lista"))
    );
    assert!(s.prefs.reglas.cortados.is_empty() && s.prefs.reglas.permitidos.is_empty());
    // Any other site: block, unblock — it passes, and its row offers «Volver a bloquear»
    // (owner's report of 10 Oct 2026: after «Desbloquear» the way back was gone).
    manda(&mut s, "bloquear_sitio", "otra.io");
    assert_eq!(fila(&mut s, "otra.io"), ("cortado".into(), json!("tuya")));
    manda(&mut s, "desbloquear_sitio", "otra.io");
    assert_eq!(fila(&mut s, "otra.io"), ("pasa".into(), json!("permitido")));
    manda(&mut s, "bloquear_sitio", "otra.io");
    assert_eq!(fila(&mut s, "otra.io"), ("cortado".into(), json!("tuya")));
    assert!(s.prefs.reglas.permitidos.is_empty());
}

#[test]
fn with_maximum_protection_an_unblocked_site_stays_unblocked_and_can_be_blocked_again() {
    let (mut s, _, id) = con_pagina(true);
    s.prefs.reglas.maxima = true;
    let fila = |s: &mut Sesion, sitio: &str| {
        let o = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"panel","vista":"escudo"}"#);
        let e = del_tipo(&o, Origen::Panel, "escudo").unwrap_or_default();
        let t = e["terceros"]
            .as_array()
            .and_then(|l| l.iter().find(|t| t["sitio"] == sitio).cloned())
            .unwrap_or_default();
        (
            t["ahora"].as_str().unwrap_or("").to_string(),
            t["regla"].clone(),
        )
    };
    let manda = |s: &mut Sesion, tipo: &str, sitio: &str| {
        let _ = s.mensaje(
            Origen::Panel,
            PANEL,
            &json!({ "tipo": tipo, "sitio": sitio }).to_string(),
        );
    };
    // A site the person cut by hand, before it sent any beacon: unblocking it must not leave
    // it to be cut again by its next beacon, and its row keeps the way back.
    let _ = s.peticion(id, "https://cdn.otra.io/a.js", "GET", b"", 3);
    manda(&mut s, "bloquear_sitio", "otra.io");
    assert!(
        s.peticion(id, "https://cdn.otra.io/b.js", "GET", b"", 3)
            .cortar
    );
    manda(&mut s, "desbloquear_sitio", "otra.io");
    assert!(
        !s.peticion(id, "https://cdn.otra.io/ping", "POST", b"x", 14)
            .cortar
    );
    assert_eq!(fila(&mut s, "otra.io"), ("pasa".into(), json!("permitido")));
    // A beacon cut by maximum protection: the same, and «Volver a bloquear» leaves it to the
    // lists again, with no rule of the person's.
    let _ = s.peticion(id, "https://px.tercera.io/ping", "POST", b"x", 14);
    assert_eq!(
        fila(&mut s, "tercera.io"),
        ("cortado".into(), json!("lista"))
    );
    manda(&mut s, "desbloquear_sitio", "tercera.io");
    assert_eq!(
        fila(&mut s, "tercera.io"),
        ("pasa".into(), json!("permitido"))
    );
    assert!(
        !s.peticion(id, "https://px.tercera.io/ping", "POST", b"x", 14)
            .cortar
    );
    manda(&mut s, "bloquear_sitio", "tercera.io");
    assert_eq!(
        fila(&mut s, "tercera.io"),
        ("cortado".into(), json!("lista"))
    );
    assert!(!s.prefs.reglas.cortados.contains("tercera.io"));
    assert!(
        s.peticion(id, "https://px.tercera.io/ping", "POST", b"x", 14)
            .cortar
    );
}

#[test]
fn maximum_protection_turns_everything_on_and_goes_with_the_cut() {
    let (mut s, _, id) = con_pagina(false);
    s.prefs.cookies_sin_tocar = true;
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"ajuste","clave":"maxima","valor":true}"#,
    );
    assert!(o.contains(&Orden::Seguimiento { estricto: true }));
    assert!(s.prefs.reglas.cortar_seguimiento && s.prefs.reglas.maxima);
    assert!(!s.prefs.cookies_sin_tocar);
    let r = s.peticion(id, "https://cdn.otra.io/ping", "POST", b"x", 14);
    assert!(r.cortar, "a beacon to another company is cut");
    let e = del_tipo(&s.tic(), Origen::Barra, "escudo").unwrap_or_default();
    assert_eq!(e["maxima"], true);
    // The site shows as cut by the lists, and «Desbloquear» lets its beacons through again.
    let fila = e["terceros"]
        .as_array()
        .and_then(|l| l.iter().find(|t| t["sitio"] == "otra.io").cloned())
        .unwrap_or_default();
    assert_eq!(
        (fila["ahora"].as_str(), fila["regla"].as_str()),
        (Some("cortado"), Some("lista"))
    );
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"desbloquear_sitio","sitio":"otra.io"}"#,
    );
    assert!(
        !s.peticion(id, "https://cdn.otra.io/ping", "POST", b"x", 14)
            .cortar
    );
    // Leaving cookie notices alone is no longer maximum protection.
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"ajuste","clave":"rechazar_cookies","valor":false}"#,
    );
    assert!(!s.prefs.reglas.maxima);
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"ajuste","clave":"maxima","valor":true}"#,
    );
    // Turning the cut off turns maximum off with it.
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"ajuste","clave":"cortar_seguimiento","valor":false}"#,
    );
    assert!(!s.prefs.reglas.maxima);
}

#[test]
fn the_day_says_who_followed_you_from_web_to_web() {
    let (mut s, _, id) = con_pagina(true);
    let _ = s.peticion(
        id,
        "https://www.google-analytics.com/g/collect",
        "GET",
        b"",
        3,
    );
    assert!(!abre_pagina(&mut s, id, "https://elpais.com/").cortar);
    let _ = s.peticion(
        id,
        "https://www.google-analytics.com/g/collect",
        "GET",
        b"",
        3,
    );
    let o = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"panel","vista":"escudo"}"#);
    let hoy = del_tipo(&o, Origen::Panel, "hoy").unwrap_or_default();
    assert_eq!(hoy["rastro_hoy"]["webs"], 2);
    assert_eq!(hoy["rastro_hoy"]["seguidores"][0]["quien"], "Google");
    assert_eq!(hoy["rastro_hoy"]["seguidores"][0]["webs"], 2);
    assert_eq!(hoy["rastro_hoy"]["seguidores"][0]["paso"], 0);
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
        r#"{"tipo":"bloquear_sitio","sitio":"ejemplo.com"}"#,
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
    assert!(s.necesita_cuerpo(id, "https://collect.otra-empresa.io/e"));
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
    assert_eq!(h["motor_id"], "duckduckgo");
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
    // The panel goes back to the shield, which the person keeps open.
    let o = s.pagina_nueva(id, "https://www.tienda.co/otra");
    assert_eq!(
        del_tipo(&o, Origen::Panel, "vista").unwrap_or_default()["vista"],
        "escudo"
    );
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
    // The summary: a PDF of its own name, the same page without the list.
    let o = s.mensaje(
        Origen::Pestana(id),
        CORTES,
        r#"{"tipo":"exportar_cortes","periodo":"mes","formato":"resumen"}"#,
    );
    assert!(matches!(o.first(), Some(Orden::GuardaPdf { ruta, .. })
        if ruta.file_name().is_some_and(|n| n.to_string_lossy().starts_with("guardiana-zero-resumen-"))));
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

/// A licence file that cannot be read (here a folder in its place) neither turns the trial into
/// a free browser for ever nor goes unsaid: the bar and the panel say it at once, the browser
/// protects for a day from the start, or from the last good reading, and then stops until the
/// file can be read again (review of 10 Oct 2026).
#[test]
fn an_unreadable_licence_protects_for_one_day_and_says_so() {
    const DIA: u64 = 86_400_000;
    static AHORA: AtomicU64 = AtomicU64::new(1_791_553_171_000);
    fn reloj_movil() -> i64 {
        i64::try_from(AHORA.load(Ordering::SeqCst)).unwrap_or(0)
    }
    let tracker = "https://stats.g.doubleclick.net/g/collect";
    let d = carpeta();
    let datos = d.join("datos");
    fs::create_dir_all(datos.join(crate::licencia::ARCHIVO)).unwrap_or_default();
    let (mut s, _) = Sesion::abre(Arranque {
        datos: datos.clone(),
        descargas: d.join("descargas"),
        idioma_sistema: "es".into(),
        version: "0.1.0".into(),
        reloj: reloj_movil,
    });
    let o = s.pon_lugar_licencia(LugarLicencia::sin_marca(&datos, d.join("marca")));
    for a in [Origen::Barra, Origen::Panel] {
        let l = del_tipo(&o, a, "licencia").unwrap_or_default();
        assert_eq!(l["estado"], "desconocido");
        assert_eq!(l["ilegible"], true, "{a:?}");
        assert_eq!(l["protege"], true);
        assert!(l["donde_datos"]
            .as_str()
            .unwrap_or_default()
            .ends_with(crate::licencia::ARCHIVO));
    }
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"bienvenida","cortar":true}"#,
    );
    let id = s.activa();
    let _ = abre_pagina(&mut s, id, "https://www.eltiempo.com/");
    assert!(s.peticion(id, tracker, "GET", b"", 3).cortar);

    // A minute short of the day: still protecting, nothing new to say.
    AHORA.fetch_add(DIA - 60_000, Ordering::SeqCst);
    let o = s.tic();
    assert!(s.protege());
    assert!(del_tipo(&o, Origen::Barra, "licencia").is_none());
    // The day is over: no protection, and the bar and the panel say why.
    AHORA.fetch_add(60_000, Ordering::SeqCst);
    let o = s.tic();
    assert!(!s.protege());
    assert!(o.contains(&Orden::Seguimiento { estricto: false }));
    let l = del_tipo(&o, Origen::Barra, "licencia").unwrap_or_default();
    assert_eq!(l["estado"], "ilegible");
    assert_eq!(l["protege"], false);
    assert!(!s.peticion(id, tracker, "GET", b"", 3).cortar);
    // The paid tools open the subscription view, without saying that a trial ended.
    let o = s.mensaje(Origen::Panel, PANEL, r#"{"tipo":"tachar","texto":"hola"}"#);
    assert_eq!(
        del_tipo(&o, Origen::Panel, "vista").unwrap_or_default()["vista"],
        "licencia"
    );
    assert!(del_tipo(&o, Origen::Panel, "aviso").is_none());
    // Read again (as the shell's check does): protection is back, and the notice goes.
    let o = s.pon_licencia(EstadoLicencia::Prueba {
        termina: reloj_movil() + 3 * 86_400_000,
        dias: 3,
    });
    assert!(s.protege());
    let l = del_tipo(&o, Origen::Barra, "licencia").unwrap_or_default();
    assert_eq!(
        (l["estado"].as_str(), l["ilegible"].as_bool()),
        (Some("prueba"), Some(false))
    );
    assert!(s.peticion(id, tracker, "GET", b"", 3).cortar);
    // Unreadable again: the day counts from that good reading.
    AHORA.fetch_add(60_000, Ordering::SeqCst);
    let o = s.tic();
    let l = del_tipo(&o, Origen::Barra, "licencia").unwrap_or_default();
    assert_eq!(
        (l["estado"].as_str(), l["ilegible"].as_bool()),
        (Some("prueba"), Some(true))
    );
    AHORA.fetch_add(DIA - 120_000, Ordering::SeqCst);
    let _ = s.tic();
    assert!(s.protege());
    AHORA.fetch_add(60_000, Ordering::SeqCst);
    let _ = s.tic();
    assert!(!s.protege());
}

#[test]
fn the_list_of_cuts_has_a_way_back_to_the_page_the_person_was_on() {
    let (mut s, _, web) = con_pagina(true);
    // From the shield: the list opens in a tab of its own, with nothing behind it.
    let o = s.mensaje(Origen::Panel, PANEL, r#"{"tipo":"abrir_cortes"}"#);
    let lista = s.activa();
    assert_ne!(lista, web);
    assert!(o.iter().any(|x| matches!(x,
        Orden::CreaPestana { id, url: Some(u), .. } if *id == lista && u == CORTES)));
    // Asking for it again does not open a second copy.
    let o = s.mensaje(Origen::Panel, PANEL, r#"{"tipo":"abrir_cortes"}"#);
    assert!(!o.iter().any(|x| matches!(x, Orden::CreaPestana { .. })));
    assert_eq!(s.activa(), lista);
    let _ = s.navegacion_empieza(lista, CORTES, false);
    let _ = s.pagina_nueva(lista, CORTES);
    // «Back» closes it and returns to the web, as it was.
    let o = s.mensaje(Origen::Pestana(lista), CORTES, r#"{"tipo":"volver"}"#);
    assert!(o
        .iter()
        .any(|x| matches!(x, Orden::CierraPestana { id } if *id == lista)));
    assert!(o
        .iter()
        .any(|x| matches!(x, Orden::Activa { id } if *id == web)));
    assert_eq!(s.activa(), web);
    assert!(!o.iter().any(|x| matches!(x, Orden::Navega { .. })));
}

#[test]
fn back_on_the_list_goes_to_the_page_before_or_to_the_new_tab() {
    let (mut s, _, id) = con_pagina(true);
    // Opened from the new tab's link, in the same tab: back is the page before.
    let _ = s.navegacion_empieza(id, CORTES, false);
    let _ = s.pagina_nueva(id, CORTES);
    let _ = s.historial(id, true, false);
    let o = s.mensaje(Origen::Pestana(id), CORTES, r#"{"tipo":"volver"}"#);
    assert!(matches!(o.as_slice(), [Orden::Atras { id: a }] if *a == id));
    // Nothing before and nowhere it came from: the new tab page, never a dead end.
    let _ = s.historial(id, false, false);
    let o = s.mensaje(Origen::Pestana(id), CORTES, r#"{"tipo":"volver"}"#);
    assert!(matches!(o.as_slice(), [Orden::Navega { id: a, url }] if *a == id && url == INICIO));
    // A web page cannot ask for it.
    let _ = abre_pagina(&mut s, id, "https://www.eltiempo.com/");
    let o = s.mensaje(
        Origen::Pestana(id),
        "https://www.eltiempo.com/",
        r#"{"tipo":"volver"}"#,
    );
    assert!(o.is_empty());
}

#[test]
fn the_new_tab_lists_the_engines_and_can_change_the_one_in_use() {
    let (mut s, _, id) = con_pagina(true);
    let _ = s.navegacion_empieza(id, INICIO, false);
    let _ = s.pagina_nueva(id, INICIO);
    let hoy = s.msg_hoy();
    assert_eq!(hoy["motor_id"], "duckduckgo");
    assert_eq!(
        hoy["motores"].as_array().map(Vec::len),
        Some(BUSCADORES.len())
    );
    let _ = s.mensaje(Origen::Pestana(id), INICIO, r#"{"tipo":"listo"}"#);
    let o = s.mensaje(
        Origen::Pestana(id),
        INICIO,
        r#"{"tipo":"ajuste","clave":"buscador","valor":"qwant"}"#,
    );
    assert_eq!(s.msg_hoy()["motor_id"], "qwant");
    assert_eq!(
        del_tipo(&o, Origen::Pestana(id), "hoy").unwrap_or_default()["motor_id"],
        "qwant"
    );
    // Any other setting stays out of a page's reach.
    let _ = s.mensaje(
        Origen::Pestana(id),
        INICIO,
        r#"{"tipo":"ajuste","clave":"idioma","valor":"en"}"#,
    );
    assert_eq!(s.msg_ajustes()["idioma"], "");
}

#[test]
fn day_or_night_is_kept_and_reaches_every_page_of_the_browser() {
    let (mut s, d, id) = con_pagina(true);
    let _ = s.navegacion_empieza(id, INICIO, false);
    let _ = s.pagina_nueva(id, INICIO);
    let o = s.mensaje(Origen::Pestana(id), INICIO, r#"{"tipo":"listo"}"#);
    assert_eq!(
        del_tipo(&o, Origen::Pestana(id), "tema").unwrap_or_default()["tema"],
        ""
    );
    // The moon on the new tab: night, for the bar, the panel and the page itself.
    let o = s.mensaje(
        Origen::Pestana(id),
        INICIO,
        r#"{"tipo":"ajuste","clave":"tema","valor":"noche"}"#,
    );
    for a in [Origen::Barra, Origen::Panel, Origen::Pestana(id)] {
        assert_eq!(del_tipo(&o, a, "tema").unwrap_or_default()["tema"], "noche");
    }
    // And the window's own title bar.
    assert!(o.contains(&Orden::Tema { oscuro: Some(true) }));
    assert_eq!(s.msg_ajustes()["tema"], "noche");
    // Anything else is not a theme.
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"ajuste","clave":"tema","valor":"rosa"}"#,
    );
    assert!(o.is_empty());
    // Kept for the next run.
    let (s2, _) = Sesion::abre(Arranque {
        datos: d.join("datos"),
        descargas: d.join("descargas"),
        idioma_sistema: "es-CO".into(),
        version: "0.1.0".into(),
        reloj,
    });
    assert_eq!(s2.msg_ajustes()["tema"], "noche");
}

#[test]
fn cookie_notices_are_dealt_with_only_when_told_and_shown_in_the_shield() {
    let (mut s, _, id) = con_pagina(true);
    let web = "https://www.eltiempo.com/";
    let pide = r#"{"tipo":"zg_cookies_pide","zg":"f1"}"#;
    let respuesta = |o: &[Orden]| -> Value {
        o.iter()
            .find_map(|x| match x {
                Orden::RespondeFormulario { json, .. } => serde_json::from_str(json).ok(),
                _ => None,
            })
            .unwrap_or_default()
    };
    let o = s.mensaje(Origen::Pestana(id), web, pide);
    let r = respuesta(&o);
    assert_eq!(r["zg"], "f1");
    assert!(s.protege());
    assert_eq!(r["cookies"], true);
    // What the guard did reaches the shield, with the manager's name.
    let o = s.mensaje(
        Origen::Pestana(id),
        web,
        r#"{"tipo":"zg_cookies","zg":"f1","gestor":"OneTrust","accion":"rechazado"}"#,
    );
    if s.protege() {
        let e = del_tipo(&o, Origen::Barra, "escudo").unwrap_or_default();
        assert_eq!(e["cookies"]["gestor"], "OneTrust");
        assert_eq!(e["cookies"]["accion"], "rechazado");
    }
    // Counted once per page in the day's figures, however often the guard reports it.
    let _ = s.mensaje(
        Origen::Pestana(id),
        web,
        r#"{"tipo":"zg_cookies","zg":"f1","gestor":"OneTrust","accion":"rechazado"}"#,
    );
    assert_eq!(s.msg_hoy()["hoy"]["avisos_cookies"], 1);
    // Nonsense is ignored.
    let o = s.mensaje(
        Origen::Pestana(id),
        web,
        r#"{"tipo":"zg_cookies","zg":"f1","gestor":"<img>","accion":"rechazado"}"#,
    );
    assert!(o.is_empty());
    // Turned off in the settings: the guard is told no.
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"ajuste","clave":"rechazar_cookies","valor":false}"#,
    );
    assert_eq!(s.msg_ajustes()["rechazar_cookies"], false);
    let o = s.mensaje(Origen::Pestana(id), web, pide);
    assert_eq!(respuesta(&o)["cookies"], false);
    // Back on, but the trial is over: no.
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"ajuste","clave":"rechazar_cookies","valor":true}"#,
    );
    let _ = s.pon_licencia(EstadoLicencia::PruebaTerminada { desde: reloj() });
    let o = s.mensaje(Origen::Pestana(id), web, pide);
    assert_eq!(respuesta(&o)["cookies"], false);
}

#[test]
fn the_star_keeps_a_page_and_the_new_tab_shows_and_forgets_it() {
    let (mut s, d, id) = con_pagina(true);
    let estrella = |o: &[Orden]| {
        del_tipo(o, Origen::Barra, "estado").unwrap_or_default()["activa"]["favorito"].clone()
    };
    let o = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"favorito"}"#);
    assert_eq!(estrella(&o), true);
    let guardado: Value = serde_json::from_str(
        &fs::read_to_string(d.join("datos").join("favoritos.json")).unwrap_or_default(),
    )
    .unwrap_or_default();
    assert_eq!(guardado["lista"][0]["url"], "https://www.eltiempo.com/");
    // On the new tab, with its × to forget it.
    let _ = s.navegacion_empieza(id, INICIO, false);
    let _ = s.pagina_nueva(id, INICIO);
    let o = s.mensaje(Origen::Pestana(id), INICIO, r#"{"tipo":"listo"}"#);
    let f = del_tipo(&o, Origen::Pestana(id), "favoritos").unwrap_or_default();
    assert_eq!(f["lista"].as_array().map(Vec::len), Some(1));
    let o = s.mensaje(
        Origen::Pestana(id),
        INICIO,
        r#"{"tipo":"favorito_quitar","url":"https://www.eltiempo.com/"}"#,
    );
    let f = del_tipo(&o, Origen::Pestana(id), "favoritos").unwrap_or_default();
    assert_eq!(f["lista"].as_array().map(Vec::len), Some(0));
    // A web page cannot touch them, and the star does nothing on the browser's own pages.
    let o = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"favorito"}"#);
    assert!(o.is_empty());
}

#[test]
fn update_check_runs_once_and_says_what_it_found() {
    let (mut s, _, _) = abre();
    let o = s.mensaje(Origen::Panel, PANEL, r#"{"tipo":"buscar_version"}"#);
    assert!(o.contains(&Orden::BuscaVersion));
    assert_eq!(
        del_tipo(&o, Origen::Panel, "version").unwrap_or_default()["estado"],
        "buscando"
    );
    // Pressed again while it runs: nothing more goes out.
    assert!(s
        .mensaje(Origen::Panel, PANEL, r#"{"tipo":"buscar_version"}"#)
        .is_empty());
    let o = s.version_encontrada(Ok("9.9.9".into()));
    let v = del_tipo(&o, Origen::Panel, "version").unwrap_or_default();
    assert_eq!(
        (v["estado"].as_str(), v["version"].as_str()),
        (Some("nueva"), Some("9.9.9"))
    );
    let o = s.version_encontrada(Ok("0.1.0".into()));
    assert_eq!(
        del_tipo(&o, Origen::Panel, "version").unwrap_or_default()["estado"],
        "al_dia"
    );
    let o = s.version_encontrada(Err("HTTP 503".into()));
    assert_eq!(
        del_tipo(&o, Origen::Panel, "version").unwrap_or_default()["error"],
        "HTTP 503"
    );
    // A web page cannot ask for it.
    let id = s.activa();
    assert!(s
        .mensaje(
            Origen::Pestana(id),
            "https://x.example/",
            r#"{"tipo":"buscar_version"}"#
        )
        .is_empty());
}

#[test]
fn the_shield_shows_the_whole_day_above_the_page() {
    let (mut s, _, _) = con_pagina(true);
    let o = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"panel","vista":"escudo"}"#);
    let hoy = del_tipo(&o, Origen::Panel, "hoy").unwrap_or_default();
    assert!(
        hoy["hoy"].is_object(),
        "el panel recibe las cifras del día al abrir el escudo"
    );
}

#[test]
fn after_the_trial_the_shield_no_longer_says_cut() {
    let (mut s, _, id) = con_pagina(true);
    let _ = s.peticion(id, "https://stats.g.doubleclick.net/a", "GET", b"", 3);
    let _ = s.pon_licencia(EstadoLicencia::PruebaTerminada { desde: reloj() });
    let o = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"panel","vista":"escudo"}"#);
    let o = if del_tipo(&o, Origen::Panel, "escudo").is_some() {
        o
    } else {
        s.tic()
    };
    if let Some(e) =
        del_tipo(&o, Origen::Panel, "escudo").or_else(|| del_tipo(&o, Origen::Barra, "escudo"))
    {
        for t in e["terceros"].as_array().into_iter().flatten() {
            assert_eq!(t["ahora"], "pasa");
        }
    }
    assert!(
        !s.peticion(id, "https://stats.g.doubleclick.net/b", "GET", b"", 3)
            .cortar
    );
}

#[test]
fn an_isolated_tab_writes_nothing_to_the_book() {
    let (mut s, _, _) = con_pagina(false);
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"tinta_anadir","dato":"correo","valor":"ana@correo.co"}"#,
    );
    let _ = s.tecla(u32::from('N'), true, true, false);
    let id = s.activa();
    assert!(s.es_aislada(id));
    let web = "https://www.secreto.example/registro";
    let _ = abre_pagina(&mut s, id, web);
    let o = s.mensaje(
        Origen::Pestana(id),
        web,
        r#"{"tipo":"zg_formulario","zg":"a","id":1,"accion":"https://www.secreto.example/alta","valores":["ana@correo.co"]}"#,
    );
    if let Some(q) = del_tipo(&o, Origen::Panel, "pregunta_formulario") {
        let _ = s.mensaje(
            Origen::Panel,
            PANEL,
            &json!({ "tipo": "formulario_respuesta", "id": q["id"], "enviar": true, "recordar": true })
                .to_string(),
        );
    }
    let _ = s.peticion(
        id,
        "https://www.secreto.example/alta?e=ana@correo.co",
        "GET",
        b"",
        7,
    );
    assert!(
        s.libro.entradas.is_empty(),
        "{:?}",
        s.libro.entradas.keys().collect::<Vec<_>>()
    );
    assert!(s.prefs.sin_preguntar.is_empty());
}

fn sha256_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The shield's row of `sitio` as the panel gets it.
fn fila_de(s: &mut Sesion, sitio: &str) -> Value {
    let o = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"panel","vista":"escudo"}"#);
    let e = del_tipo(&o, Origen::Panel, "escudo").unwrap_or_default();
    e["terceros"]
        .as_array()
        .and_then(|l| l.iter().find(|t| t["sitio"] == sitio).cloned())
        .unwrap_or_default()
}

fn marca_correo(s: &mut Sesion) {
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"tinta_anadir","dato":"correo","valor":"ana@correo.co"}"#,
    );
}

/// Review of 10 Oct 2026, grave 1: Meta's pixel lives on `www.facebook.com`, which the lists
/// do not call a tracker by its name; its path does.
#[test]
fn meta_s_pixel_is_cut_and_its_row_says_the_lists_cut_it() {
    let (mut s, _, id) = con_pagina(true);
    let r = s.peticion(
        id,
        "https://www.facebook.com/tr?id=1&ev=Purchase&cd[value]=89900&cd[currency]=COP",
        "GET",
        b"",
        3,
    );
    assert!(r.cortar);
    assert!(
        s.peticion(
            id,
            "https://www.facebook.com/tr/",
            "POST",
            b"id=1&ev=PageView",
            8
        )
        .cortar
    );
    assert!(
        s.peticion(
            id,
            "https://connect.facebook.net/en_US/fbevents.js",
            "GET",
            b"",
            6
        )
        .cortar
    );
    assert!(
        s.peticion(
            id,
            "https://www.google.com/pagead/1p-conversion/1/?value=1",
            "GET",
            b"",
            3
        )
        .cortar
    );
    let f = fila_de(&mut s, "facebook.com");
    assert_eq!(f["quien"], "Meta");
    assert_eq!(
        (f["ahora"].as_str(), f["regla"].as_str()),
        (Some("cortado"), Some("lista"))
    );
    assert_eq!(f["categoria"], "rastreador");
    // And who followed the person is now named too.
    let hoy = s.msg_hoy();
    assert!(hoy["hoy"]["empresas_cortadas"].as_u64().unwrap_or(0) >= 2);
}

/// Grave 2: a tracker served from a delivery network is a tracker, and shows up.
#[test]
fn a_tracker_on_a_delivery_network_is_cut_and_shown() {
    let (mut s, _, id) = con_pagina(true);
    let r = s.peticion(
        id,
        "https://d10lpsik1i8c69.cloudfront.net/w.js",
        "GET",
        b"",
        6,
    );
    assert!(r.cortar);
    let f = fila_de(&mut s, "d10lpsik1i8c69.cloudfront.net");
    assert_eq!(f["ahora"], "cortado");
    // The road carrying the page's own images is still not «a company from outside».
    assert!(
        !s.peticion(id, "https://d1abc.cloudfront.net/logo.png", "GET", b"", 3)
            .cortar
    );
    assert!(fila_de(&mut s, "d1abc.cloudfront.net").is_null());
}

/// Grave 5: «Enviar» sends, to the site it was asked about and for a few minutes; and
/// «Desbloquear» lets marked data through to a site, with «Volver a bloquear» as its way back.
#[test]
fn an_approved_sending_reaches_its_site_and_the_approval_ends() {
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
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"bienvenida","cortar":true}"#,
    );
    marca_correo(&mut s);
    let id = s.activa();
    let web = "https://www.tienda.co/pagar";
    let _ = abre_pagina(&mut s, id, web);
    let o = s.mensaje(
        Origen::Pestana(id),
        web,
        r#"{"tipo":"zg_formulario","zg":"f","id":1,"accion":"https://checkout.pagos-ejemplo.com/pay","valores":["ana@correo.co"]}"#,
    );
    let q = del_tipo(&o, Origen::Panel, "pregunta_formulario").unwrap_or_default();
    assert_eq!(q["sitio"], "pagos-ejemplo.com");
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        &json!({ "tipo": "formulario_respuesta", "id": q["id"], "enviar": true, "recordar": false })
            .to_string(),
    );
    // The form posts the page away to the payment site: it goes.
    let pago = "https://checkout.pagos-ejemplo.com/pay";
    let _ = s.navegacion_empieza(id, pago, false);
    let r = s.peticion(id, pago, "POST", b"email=ana%40correo.co&total=1", 1);
    assert!(!r.cortar, "«Enviar» must send");
    // Its script sending the same to the same site, too; to anyone else, never.
    assert!(
        !s.peticion(
            id,
            "https://api.pagos-ejemplo.com/v1/intent",
            "POST",
            br#"{"email":"ana@correo.co"}"#,
            8
        )
        .cortar
    );
    assert!(
        s.peticion(
            id,
            "https://collect.otra-empresa.io/e",
            "POST",
            br#"{"email":"ana@correo.co"}"#,
            8
        )
        .cortar
    );
    // Five minutes later, the approval is over.
    AHORA.fetch_add(5 * 60_000 + 1, Ordering::SeqCst);
    assert!(
        s.peticion(
            id,
            "https://api.pagos-ejemplo.com/v1/intent",
            "POST",
            br#"{"email":"ana@correo.co"}"#,
            8
        )
        .cortar
    );
}

/// «Desbloquear» never lets marked data through (the owner, 10 Oct 2026): a row cut only for
/// carrying it offers no «Desbloquear», and says that «Enviar» is how it goes.
#[test]
fn a_row_cut_for_your_data_offers_no_unblock_and_unblocking_never_sends_it() {
    let (mut s, _, id) = con_pagina(false);
    marca_correo(&mut s);
    let envia_correo = |s: &mut Sesion| {
        s.peticion(
            id,
            "https://collect.otra-empresa.io/e",
            "POST",
            br#"{"email":"ana@correo.co"}"#,
            8,
        )
        .cortar
    };
    assert!(envia_correo(&mut s));
    // The row says what holds: cut, for carrying your data (it said «pasa», review P5); and
    // nothing a button of the shield does would send it.
    let f = fila_de(&mut s, "otra-empresa.io");
    assert_eq!(
        (f["ahora"].as_str(), f["por"].as_str()),
        (Some("cortado"), Some("tinta"))
    );
    assert_eq!(
        (f["deshace"].as_bool(), f["dato"].as_bool()),
        (Some(false), Some(true))
    );
    // Even an unblock sent anyway (an old panel) leaves the data cut.
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"desbloquear_sitio","sitio":"otra-empresa.io"}"#,
    );
    assert!(envia_correo(&mut s));
    let f = fila_de(&mut s, "otra-empresa.io");
    assert_eq!(
        (f["ahora"].as_str(), f["regla"].as_str()),
        (Some("cortado"), Some("permitido"))
    );
    assert!(!s.libro.entradas.contains_key("otra-empresa.io"));
    // Its «Volver a bloquear» leaves it as it was, with no rule of the person's.
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"bloquear_sitio","sitio":"otra-empresa.io"}"#,
    );
    assert!(s.prefs.reglas.cortados.is_empty() && s.prefs.reglas.permitidos.is_empty());
    assert_eq!(fila_de(&mut s, "otra-empresa.io")["ahora"], "cortado");
    // A page sending your data away by opening another site is not let through by a rule
    // either.
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"desbloquear_sitio","sitio":"tracker.example"}"#,
    );
    assert!(abre_pagina(&mut s, id, "https://tracker.example/r?e=ana%40correo.co").cortar);
}

/// The approval of «Enviar» goes on in the window the page opens for the sending (a form with
/// `target=_blank`), and it never covers a mandate's decoy.
#[test]
fn an_approved_sending_carries_on_in_the_window_it_opens() {
    let (mut s, _, id) = con_pagina(false);
    marca_correo(&mut s);
    let web = "https://www.tienda.co/pagar";
    let _ = abre_pagina(&mut s, id, web);
    let o = s.mensaje(
        Origen::Pestana(id),
        web,
        r#"{"tipo":"zg_formulario","zg":"f","id":1,"accion":"https://checkout.pagos-ejemplo.com/pay","valores":["ana@correo.co"]}"#,
    );
    let q = del_tipo(&o, Origen::Panel, "pregunta_formulario").unwrap_or_default();
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        &json!({ "tipo": "formulario_respuesta", "id": q["id"], "enviar": true, "recordar": false })
            .to_string(),
    );
    let (n, _) = s.ventana_nueva(id).unwrap_or_default();
    let pago = "https://checkout.pagos-ejemplo.com/pay";
    let _ = s.navegacion_empieza(n, pago, false);
    assert!(
        !s.peticion(n, pago, "POST", b"email=ana%40correo.co", 1)
            .cortar
    );
    // Anywhere else, still cut, from the new window too.
    let fuga = "https://tracker.example/r?e=ana%40correo.co";
    let (m, _) = s.ventana_nueva(id).unwrap_or_default();
    let _ = s.navegacion_empieza(m, fuga, false);
    assert!(s.peticion(m, fuga, "GET", b"", 1).cortar);
    // Once the window shows a web of its own, that web is where data leaves from: from the new
    // tab page after it, as from any tab.
    let _ = s.pagina_nueva(m, "https://www.otra-web.co/");
    let _ = s.navegacion_empieza(m, INICIO, false);
    let _ = s.pagina_nueva(m, INICIO);
    let _ = s.navegacion_empieza(m, fuga, false);
    assert!(!s.peticion(m, fuga, "GET", b"", 1).cortar);
}

#[test]
fn the_approval_never_covers_a_mandate_s_decoy() {
    let (mut s, _, _) = con_pagina(false);
    marca_correo(&mut s);
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
    let web = "https://www.avianca.com/reservar";
    let _ = abre_pagina(&mut s, m, web);
    let o = s.mensaje(
        Origen::Pestana(m),
        web,
        &json!({ "tipo": "zg_formulario", "zg": "f", "id": 1, "accion": "https://www.avianca.com/pago", "valores": [senuelo, "ana@correo.co"] })
            .to_string(),
    );
    let q = del_tipo(&o, Origen::Panel, "pregunta_formulario").unwrap_or_default();
    assert_eq!(q["mandato"], true);
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        &json!({ "tipo": "formulario_respuesta", "id": q["id"], "enviar": true, "recordar": false })
            .to_string(),
    );
    // The person's own email was approved: it goes.
    assert!(
        !s.peticion(
            m,
            "https://www.avianca.com/api/pago",
            "POST",
            b"email=ana%40correo.co",
            8
        )
        .cortar
    );
    // The decoy is nobody's: it is caught, approval or not, in the page and in a new window.
    let robo = format!("{{\"email\":\"{senuelo}\"}}");
    let r = s.peticion(
        m,
        "https://www.avianca.com/api/pago",
        "POST",
        robo.as_bytes(),
        8,
    );
    assert!(r.cortar);
    let (n, _) = s.ventana_nueva(m).unwrap_or_default();
    let fuga = format!("https://www.avianca.com/p?e={senuelo}");
    let _ = s.navegacion_empieza(n, &fuga, false);
    assert!(s.peticion(n, &fuga, "GET", b"", 3).cortar);
}

/// Grave 6: a window a page opens leaves from that page.
#[test]
fn a_new_window_cannot_carry_marked_data_away_either() {
    let (mut s, _, id) = con_pagina(false);
    marca_correo(&mut s);
    let _ = abre_pagina(&mut s, id, "https://www.tienda.co/");
    let (n, _) = s.ventana_nueva(id).unwrap_or_default();
    let fuga = "https://tracker.example/r?e=ana%40correo.co";
    let _ = s.navegacion_empieza(n, fuga, false);
    let r = s.peticion(n, fuga, "GET", b"", 1);
    assert!(r.cortar);
    assert!(r.pagina.unwrap_or_default().contains("tracker.example"));
    // To the opener's own site it goes, as it would in the same tab.
    let (m, _) = s.ventana_nueva(id).unwrap_or_default();
    let propia = "https://www.tienda.co/cuenta?e=ana%40correo.co";
    let _ = s.navegacion_empieza(m, propia, false);
    assert!(!s.peticion(m, propia, "GET", b"", 1).cortar);
    // A window from no web at all (the new tab page) is cut too.
    let _ = s.tecla(u32::from('T'), true, false, false);
    let t = s.activa();
    let (w, _) = s.ventana_nueva(t).unwrap_or_default();
    let _ = s.navegacion_empieza(w, fuga, false);
    assert!(s.peticion(w, fuga, "GET", b"", 1).cortar);
}

/// Grave 7: an isolated tab leaves nothing on disk; its shield counts while it is open.
#[test]
fn an_isolated_tab_writes_no_figures_and_no_cuts_to_disk() {
    let (mut s, d, _) = con_pagina(true);
    let _ = s.tecla(u32::from('N'), true, true, false);
    let id = s.activa();
    assert!(s.es_aislada(id));
    let _ = abre_pagina(&mut s, id, "https://secreto-medico.com/diagnostico");
    let r = s.peticion(
        id,
        "https://stats.g.doubleclick.net/g/collect/secreto-medico.com/diagnostico",
        "GET",
        b"",
        3,
    );
    assert!(r.cortar);
    let _ = s.peticion(
        id,
        "https://www.facebook.com/tr?id=1&ev=ViewContent",
        "GET",
        b"",
        3,
    );
    let e = del_tipo(&s.tic(), Origen::Barra, "escudo").unwrap_or_default();
    assert_eq!(e["resumen"]["cortadas"], 2);
    assert_eq!(e["chivatos"].as_array().map(Vec::len), Some(1));
    let _ = s.mensaje(
        Origen::Barra,
        BARRA,
        &json!({ "tipo": "pestana_cerrar", "id": id }).to_string(),
    );
    s.cierra();
    let diario = fs::read_to_string(d.join("datos").join("diario.json")).unwrap_or_default();
    assert!(!diario.contains("doubleclick"), "{diario}");
    assert!(!diario.contains("chivatos"), "{diario}");
    assert_eq!(s.msg_hoy()["hoy"]["cortadas"], 0);
    assert!(cortes::lee(&d.join("datos").join("cortes"), "2000-01-01", "2100-01-01").is_empty());
}

/// Media 8: a site partly cut says so, and its buttons go back to that part.
#[test]
fn a_site_cut_only_in_part_says_so_and_goes_back_to_that_part() {
    let (mut s, _, id) = con_pagina(true);
    assert!(
        !s.peticion(id, "https://www.google.com/recaptcha/api.js", "GET", b"", 6)
            .cortar
    );
    assert!(
        s.peticion(id, "https://adservice.google.com/ddm/fls/z", "GET", b"", 3)
            .cortar
    );
    let f = fila_de(&mut s, "google.com");
    assert_eq!(
        (f["ahora"].as_str(), f["regla"].as_str()),
        (Some("parcial"), Some("lista"))
    );
    assert_eq!(f["categoria"], "publicidad");
    let manda = |s: &mut Sesion, tipo: &str| {
        let _ = s.mensaje(
            Origen::Panel,
            PANEL,
            &json!({ "tipo": tipo, "sitio": "google.com" }).to_string(),
        );
    };
    manda(&mut s, "desbloquear_sitio");
    assert_eq!(fila_de(&mut s, "google.com")["ahora"], "pasa");
    manda(&mut s, "bloquear_sitio");
    assert_eq!(fila_de(&mut s, "google.com")["ahora"], "parcial");
    assert!(s.prefs.reglas.cortados.is_empty() && s.prefs.reglas.permitidos.is_empty());
    // Maximum protection: one beacon cut out of a site's requests is a part, not the site.
    s.prefs.reglas.maxima = true;
    let _ = s.peticion(id, "https://cdn.otra.io/a.js", "GET", b"", 6);
    assert!(
        s.peticion(id, "https://cdn.otra.io/ping", "POST", b"x", 14)
            .cortar
    );
    assert_eq!(fila_de(&mut s, "otra.io")["ahora"], "parcial");
    // «Bloquear todo» on a part-cut row: the whole site, by the person's rule, with its undo.
    let f = fila_de(&mut s, "google.com");
    assert_eq!(
        (f["ahora"].as_str(), f["deshace"].as_bool()),
        (Some("parcial"), Some(true))
    );
    manda(&mut s, "bloquear_todo");
    let f = fila_de(&mut s, "google.com");
    assert_eq!(
        (f["ahora"].as_str(), f["regla"].as_str()),
        (Some("cortado"), Some("tuya"))
    );
    assert!(
        s.peticion(id, "https://www.google.com/recaptcha/api.js", "GET", b"", 6)
            .cortar
    );
    manda(&mut s, "desbloquear_sitio");
    assert_eq!(fila_de(&mut s, "google.com")["ahora"], "pasa");
    assert!(
        !s.peticion(id, "https://www.google.com/recaptcha/api.js", "GET", b"", 6)
            .cortar
    );
}

/// Media 8 in a mandate: what is outside the task is cut, and no button of the shield changes
/// that (the task's own panel adds sites).
#[test]
fn outside_the_task_the_row_is_cut_and_fixed() {
    let (mut s, _, _) = con_pagina(false);
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"mandato_empezar","tarea":"","webs":["avianca.com"],"estricto":true}"#,
    );
    let Some(Orden::CreaPestana { id: m, .. }) = o.first().cloned() else {
        unreachable!("{o:?}");
    };
    let _ = abre_pagina(&mut s, m, "https://www.avianca.com/");
    assert!(
        s.peticion(m, "https://evil.example/p.png?d=1", "GET", b"", 3)
            .cortar
    );
    let f = fila_de(&mut s, "evil.example");
    assert_eq!(f["ahora"], "cortado");
    assert_eq!(f["por"], "fuera_de_mandato");
    assert_eq!(f["fijo"], true);
    // The task's tab turns off what its filter would not see, and the receipt says so.
    assert!(s.guion_pestana(m).contains("WebTransport"));
    assert!(!s.guion_pestana(1).contains("WebTransport"));
    let o = s.mensaje(Origen::Panel, PANEL, r#"{"tipo":"mandato_terminar"}"#);
    let fin = del_tipo(&o, Origen::Panel, "mandato").unwrap_or_default();
    let contenido = fin["recibo"]["contenido"].as_str().unwrap_or_default();
    assert!(
        contenido.contains(r#""apagado":["webrtc","webtransport"]"#),
        "{contenido}"
    );
    // Said where: in the pages and their frames, not everywhere.
    assert!(
        contenido.contains(r#""apagado_en":"paginas_y_marcos""#),
        "{contenido}"
    );
}

/// Media 10: a whole page cut is in «Lo que se cortó» and in today's figures.
#[test]
fn a_whole_page_cut_is_in_the_list_and_in_today() {
    let (mut s, _, id) = con_pagina(false);
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"bloquear_sitio","sitio":"ejemplo.com"}"#,
    );
    assert!(abre_pagina(&mut s, id, "https://www.ejemplo.com/a").cortar);
    // A request cut; not «a company from outside» that tried anything: you opened it.
    let hoy = s.msg_hoy();
    assert_eq!(hoy["hoy"]["cortadas"], 1);
    assert_eq!(hoy["hoy"]["empresas"], 0);
    assert_eq!(hoy["hoy"]["empresas_cortadas"], 0);
    assert_eq!(hoy["mes"]["empresas"], 0);
    let _ = s.navegacion_empieza(id, CORTES, false);
    let _ = s.pagina_nueva(id, CORTES);
    let o = s.mensaje(
        Origen::Pestana(id),
        CORTES,
        r#"{"tipo":"cortes","periodo":"hoy"}"#,
    );
    let c = del_tipo(&o, Origen::Pestana(id), "cortes").unwrap_or_default();
    assert_eq!(c["total"], 1);
    assert_eq!(c["lista"][0]["host"], "www.ejemplo.com");
    assert_eq!(c["lista"][0]["recurso"], "documento");
    assert_eq!(c["lista"][0]["motivo"], "corte_tuyo");
    assert_eq!(c["sin_anotar"], 0);
    // In the list, not among the companies from outside: the same as «Hoy».
    assert_eq!(c["empresas"], json!([]));
}

/// «Los chivatos»: what the page's pixels tried to tell, per tab, deduplicated, with the card to
/// share; the day keeps counts only.
#[test]
fn the_shield_says_what_the_pixels_tried_to_tell() {
    let (mut s, d, id) = con_pagina(true);
    let _ = abre_pagina(&mut s, id, "https://www.tienda.co/gracias");
    // A pixel's body is read even without marked data.
    assert!(s.necesita_cuerpo(id, "https://analytics.tiktok.com/api/v2/pixel"));
    assert!(!s.necesita_cuerpo(id, "https://www.tienda.co/api"));
    marca_correo(&mut s);
    let em = sha256_hex("ana@correo.co");
    let meta = format!(
        "https://www.facebook.com/tr/?id=1&ev=Purchase&cd[value]=89900&cd[currency]=COP&ud[em]={em}&eid=ord-7"
    );
    assert!(s.peticion(id, &meta, "GET", b"", 3).cortar);
    // The same event to a second pixel id of the same shop: one event.
    assert!(
        s.peticion(id, &meta.replace("id=1", "id=2"), "GET", b"", 3)
            .cortar
    );
    let tiktok = format!(
        r#"{{"event":"CompletePayment","event_id":"ord-7","properties":{{"value":89900,"currency":"COP"}},"context":{{"user":{{"email":"{em}"}}}}}}"#
    );
    assert!(
        s.peticion(
            id,
            "https://analytics.tiktok.com/api/v2/pixel",
            "POST",
            tiktok.as_bytes(),
            8
        )
        .cortar
    );
    let e = del_tipo(&s.tic(), Origen::Barra, "escudo").unwrap_or_default();
    let lista = e["chivatos"].as_array().cloned().unwrap_or_default();
    assert_eq!(lista.len(), 2, "{lista:?}");
    let m = &lista[0];
    assert_eq!(m["red"], "meta");
    assert_eq!(m["evento"], "compra");
    assert_eq!(m["nombre"], "Purchase");
    assert_eq!(m["importe"], "89900");
    assert_eq!(m["moneda"], "COP");
    assert_eq!(m["correo"], json!({ "tuyo": true, "cifrado": true }));
    assert_eq!(m["cortado"], true);
    assert_eq!(m["veces"], 1);
    assert!(m.get("id").is_none(), "the order id stays in memory");
    assert_eq!(
        e["tarjeta"],
        json!({ "empresas": 2, "evento": "compra", "correo": true })
    );
    // The day counts them by network and event, in memory only: on disk it would say which days
    // the person bought something.
    let hoy = s.hoy();
    let cuenta = |red: &str| s.diario.dias[&hoy].chivatos[red]["compra"];
    assert_eq!((cuenta("meta"), cuenta("tiktok")), (1, 1));
    s.cierra();
    let diario = fs::read_to_string(d.join("datos").join("diario.json")).unwrap_or_default();
    assert!(diario.contains("cortadas"), "{diario}");
    assert!(
        !diario.contains("chivatos") && !diario.contains("compra"),
        "{diario}"
    );
    assert!(!diario.contains("89900") && !diario.contains("COP") && !diario.contains("ord-7"));
    // What got through says so: with the cut off, it «passed».
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"ajuste","clave":"cortar_seguimiento","valor":false}"#,
    );
    let _ = abre_pagina(&mut s, id, "https://www.tienda.co/");
    let _ = s.peticion(
        id,
        "https://www.facebook.com/tr?id=1&ev=PageView",
        "GET",
        b"",
        3,
    );
    let e = del_tipo(&s.tic(), Origen::Barra, "escudo").unwrap_or_default();
    assert_eq!(e["chivatos"][0]["cortado"], false);
    assert!(e["tarjeta"].is_null());
}

#[test]
fn the_card_is_saved_as_a_png_of_its_own_name() {
    let (mut s, _, d) = abre();
    let png = base64::engine::general_purpose::STANDARD.encode(b"\x89PNG\r\n\x1a\nresto");
    let o = s.mensaje(
        Origen::Panel,
        PANEL,
        &json!({ "tipo": "guardar_imagen", "que": "chivatos", "datos": format!("data:image/png;base64,{png}") })
            .to_string(),
    );
    let g = del_tipo(&o, Origen::Panel, "guardado").unwrap_or_default();
    assert_eq!(g["que"], "chivatos");
    let ruta = PathBuf::from(g["ruta"].as_str().unwrap_or_default());
    assert_eq!(ruta.parent(), Some(d.join("descargas").as_path()));
    assert_eq!(
        ruta.file_name().and_then(|f| f.to_str()),
        Some("guardiana-zero-chivatos-2026-10-09.png")
    );
}

/// A second sending approved to the same site adds its data to what is approved there; it does
/// not take back the first.
#[test]
fn two_approved_sendings_to_one_site_add_up() {
    let (mut s, _, id) = con_pagina(false);
    marca_correo(&mut s);
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"tinta_anadir","dato":"telefono","valor":"+57 300 123 4567"}"#,
    );
    let web = "https://www.tienda.co/pagar";
    let _ = abre_pagina(&mut s, id, web);
    for (n, valor) in [(1, "ana@correo.co"), (2, "3001234567")] {
        let o = s.mensaje(
            Origen::Pestana(id),
            web,
            &json!({ "tipo": "zg_formulario", "zg": "f", "id": n, "accion": "https://checkout.pagos-ejemplo.com/pay", "valores": [valor] })
                .to_string(),
        );
        let q = del_tipo(&o, Origen::Panel, "pregunta_formulario").unwrap_or_default();
        let _ = s.mensaje(
            Origen::Panel,
            PANEL,
            &json!({ "tipo": "formulario_respuesta", "id": q["id"], "enviar": true, "recordar": false })
                .to_string(),
        );
    }
    let r = s.peticion(
        id,
        "https://api.pagos-ejemplo.com/v1/intent",
        "POST",
        br#"{"email":"ana@correo.co","tel":"3001234567"}"#,
        8,
    );
    assert!(!r.cortar);
}

/// The figures of what happened, as every screen shows them: the shield's summary and each row's
/// counts, the day and the month, and the list of cuts.
fn historia(s: &mut Sesion, id: u32) -> Value {
    let o = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"panel","vista":"escudo"}"#);
    let e = del_tipo(&o, Origen::Panel, "escudo").unwrap_or_default();
    let filas: Vec<Value> = e["terceros"]
        .as_array()
        .map(|l| {
            l.iter()
                .map(|t| {
                    json!([
                        t["orden"],
                        t["sitio"],
                        t["quien"],
                        t["vistas"],
                        t["cortadas"],
                        t["motivos"]
                    ])
                })
                .collect()
        })
        .unwrap_or_default();
    s.guarda_cortes();
    let cortes = cortes::lee(&s.datos.join("cortes"), "2000-01-01", "2100-01-01").len();
    let hoy = s.msg_hoy();
    json!({
        "pestana": id,
        "resumen": e["resumen"],
        "filas": filas,
        "chivatos": e["chivatos"],
        "datos_salvados": e["datos_salvados"],
        "hoy": hoy["hoy"],
        "mes": hoy["mes"],
        "rastro": hoy["rastro_hoy"],
        "cortes": cortes,
    })
}

/// Pressing a button never adds or takes away anything that already happened (the owner, 10 Oct
/// 2026: «que no estén sumando, restando… cuando uno le da a bloquear, desbloquear»): only what
/// holds now and the rule change. Only the requests that come afterwards count.
#[test]
fn buttons_change_what_holds_now_never_the_figures_of_what_happened() {
    let (mut s, _, id) = con_pagina(true);
    for (url, contexto) in [
        ("https://cdn.otra.io/a.js", 6),
        ("https://www.google.com/recaptcha/api.js", 6),
        ("https://adservice.google.com/ddm/fls/z", 3),
        ("https://stats.g.doubleclick.net/g/collect", 3),
        ("https://stats.g.doubleclick.net/g/collect", 3),
        ("https://www.facebook.com/tr?id=1&ev=PageView", 3),
    ] {
        let _ = s.peticion(id, url, "GET", b"", contexto);
    }
    let antes = historia(&mut s, id);
    assert_eq!(antes["hoy"]["cortadas"], 4);
    let estado = |s: &mut Sesion| {
        let f = fila_de(s, "google.com");
        (
            f["ahora"].as_str().unwrap_or("").to_string(),
            f["regla"].as_str().map(str::to_string),
        )
    };
    let pasos = [
        ("desbloquear_sitio", "pasa", Some("permitido")),
        ("bloquear_sitio", "parcial", Some("lista")),
        ("bloquear_todo", "cortado", Some("tuya")),
        ("desbloquear_sitio", "pasa", Some("permitido")),
    ];
    for (tipo, ahora, regla) in pasos {
        for sitio in ["google.com", "doubleclick.net", "otra.io"] {
            let _ = s.mensaje(
                Origen::Panel,
                PANEL,
                &json!({ "tipo": tipo, "sitio": sitio }).to_string(),
            );
        }
        assert_eq!(
            estado(&mut s),
            (ahora.to_string(), regla.map(str::to_string)),
            "{tipo}"
        );
        assert_eq!(historia(&mut s, id), antes, "after {tipo}");
    }
    // What comes afterwards counts, as it is decided now: unblocked, it is seen and not cut.
    let _ = s.peticion(id, "https://adservice.google.com/ddm/fls/z", "GET", b"", 3);
    let despues = historia(&mut s, id);
    assert_eq!(despues["hoy"]["cortadas"], 4);
    assert_eq!(despues["resumen"]["cortadas"], antes["resumen"]["cortadas"]);
    let google = |h: &Value| {
        h["filas"]
            .as_array()
            .and_then(|l| l.iter().find(|f| f[1] == "google.com").cloned())
            .unwrap_or_default()
    };
    assert_eq!(google(&despues)[3], 3);
    assert_eq!(google(&despues)[4], google(&antes)[4]);
}

/// The rows keep the order in which each company first showed up on the page, whatever their
/// counts or state; a new page starts its own order.
#[test]
fn rows_keep_the_order_they_first_showed_up_in() {
    let (mut s, _, id) = con_pagina(true);
    let orden = |s: &mut Sesion| -> Vec<String> {
        let o = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"panel","vista":"escudo"}"#);
        let e = del_tipo(&o, Origen::Panel, "escudo").unwrap_or_default();
        e["terceros"]
            .as_array()
            .map(|l| {
                l.iter()
                    .map(|t| t["sitio"].as_str().unwrap_or("").to_string())
                    .collect()
            })
            .unwrap_or_default()
    };
    let _ = s.peticion(id, "https://cdn.zeta.io/a.js", "GET", b"", 6);
    let _ = s.peticion(id, "https://cdn.alfa.io/a.js", "GET", b"", 6);
    for _ in 0..5 {
        let _ = s.peticion(
            id,
            "https://stats.g.doubleclick.net/g/collect",
            "GET",
            b"",
            3,
        );
    }
    assert_eq!(orden(&mut s), ["zeta.io", "alfa.io", "doubleclick.net"]);
    let _ = s.mensaje(
        Origen::Panel,
        PANEL,
        r#"{"tipo":"bloquear_sitio","sitio":"alfa.io"}"#,
    );
    let _ = s.peticion(id, "https://cdn.alfa.io/b.js", "GET", b"", 6);
    let _ = s.peticion(id, "https://cdn.beta.io/a.js", "GET", b"", 6);
    assert_eq!(
        orden(&mut s),
        ["zeta.io", "alfa.io", "doubleclick.net", "beta.io"]
    );
    // A new page, even the same site again, starts its own order (and the panel its rows).
    let o = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"panel","vista":"escudo"}"#);
    let carga = del_tipo(&o, Origen::Panel, "escudo").unwrap_or_default()["carga"].clone();
    let _ = abre_pagina(&mut s, id, "https://www.eltiempo.com/");
    let _ = s.peticion(id, "https://cdn.beta.io/a.js", "GET", b"", 6);
    assert_eq!(orden(&mut s), ["beta.io"]);
    let o = s.mensaje(Origen::Barra, BARRA, r#"{"tipo":"panel","vista":"escudo"}"#);
    assert_ne!(
        del_tipo(&o, Origen::Panel, "escudo").unwrap_or_default()["carga"],
        carga
    );
}
