//! GUARDIANA SECURE behind the Comunicaciones page, apart from the browser (17 Oct 2026).
//!
//! Until 1.0.41 the conversations lived inside [`crate::sesion::Sesion`], so only GUARDIANA ZERO
//! (Windows, WebView2) could show them: a person with GUARDIANA on Linux saw the radiographies and
//! no Comunicaciones at all. The engine is the same everywhere; only who carries the page's
//! messages changes. Here it answers in plain JSON messages for the page (`secure_estado`,
//! `secure_rtc`, `secure_media`, `secure_aviso`, `secure_vista`, `secure_incidente`,
//! `secure_prueba`) and says what else must happen outside the page ([`Salida`]). The browser
//! posts them to its SECURE tab exactly as before; `guardiana comunicaciones` serves them to the
//! person's own browser on Linux and macOS.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde_json::{json, Value};

use crate::grupo::Grupo;
use crate::sala::Sala;

/// What an action of the page leads to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Salida {
    /// A message for the page.
    Pagina(Value),
    /// Open a file that arrived with its program (never a program itself: see
    /// [`crate::secure::Recibido::abrible`]).
    AbreArchivo {
        /// Where it is.
        ruta: String,
    },
    /// Show a file that arrived, in its folder.
    MuestraArchivo {
        /// Where it is.
        ruta: String,
    },
    /// Open an address outside the page (Windows' privacy settings for the microphone or the
    /// camera).
    AbreFuera {
        /// The address.
        url: String,
    },
}

/// The conversations, the house's room and where received files go.
#[derive(Debug)]
pub struct Comunicaciones {
    /// GUARDIANA SECURE (15 Oct 2026).
    secure: Grupo,
    /// The house's room for the phones, while one is open (16 Oct 2026, «Sala de casa»).
    sala: Option<Sala>,
    /// The person's downloads: the room's files are kept there.
    descargas: PathBuf,
    /// Links that came in through this computer's GUARDIANA number (1.0.42), once
    /// [`Comunicaciones::abre_buzon`] opened the mailbox at the relay.
    buzon: Option<std::sync::mpsc::Receiver<std::net::TcpStream>>,
}

fn texto<'a>(m: &'a Value, k: &str) -> &'a str {
    m.get(k).and_then(Value::as_str).unwrap_or("")
}

impl Comunicaciones {
    /// Open this computer's identity and book under `datos/secure`.
    #[must_use]
    pub fn abre(datos: &Path, descargas: &Path, version: &str) -> Self {
        let mut secure = Grupo::abre(datos, descargas);
        secure.pon_version(version);
        Self {
            secure,
            sala: None,
            descargas: descargas.to_path_buf(),
            buzon: None,
        }
    }

    /// Wait at the relay in the room of this computer's number, so others can call it
    /// (1.0.42). The program does it once at start; the tests do not.
    pub fn abre_buzon(&mut self) {
        if self.buzon.is_none() {
            self.buzon = crate::rele::buzon(&self.secure.numero_propio());
        }
    }

    /// Something new the person has not seen yet (the tab's title carries a dot).
    #[must_use]
    pub fn novedad(&self) -> bool {
        self.secure.novedad()
    }

    /// Whether a call is going on (in a conversation or in the house's room).
    #[must_use]
    pub fn en_llamada(&self) -> bool {
        self.secure.en_llamada() || self.sala.as_ref().is_some_and(Sala::en_llamada)
    }

    /// The page's state: the conversations, plus the house's room when one is open.
    #[must_use]
    pub fn estado_json(&self) -> Value {
        let mut e = self.secure.estado_json();
        e["sala"] = self.sala.as_ref().map_or(Value::Null, Sala::json);
        // 1.0.42: while this computer also waits for a PC, the room's link and QR code carry
        // that code too, so there is a single invitation.
        let codigo = (e["fase"] == "escuchando")
            .then(|| e["rele_codigo"].as_str().or_else(|| e["codigo"].as_str()))
            .flatten()
            .map(str::to_owned);
        if let (Some(s), Some(c)) = (&self.sala, codigo) {
            let enlace = s.enlace_con(Some(&c));
            e["sala"]["qr"] = json!(guardiana_secure::tarjeta::qr_svg(&enlace).ok());
            e["sala"]["enlace"] = json!(enlace);
            e["sala"]["codigo_pc"] = json!(c);
        }
        e
    }

    /// Every tick: let the network in, and say what the page must redraw. `pagina` is whether a
    /// page that said «listo» is there to hear it: without one, what was only for the page
    /// (WebRTC messages, pieces of a call) is dropped and the notices wait for it, as they
    /// always did in the browser.
    pub fn tic(&mut self, pagina: bool) -> Vec<Value> {
        // Someone dialled this number: the conversation takes the link if it is free; if it is
        // busy the link is dropped and the caller sees the call fail.
        let llegados: Vec<std::net::TcpStream> = self
            .buzon
            .as_ref()
            .map(|r| r.try_iter().collect())
            .unwrap_or_default();
        for s in llegados {
            let _ = self.secure.entra_por_numero(s);
        }
        self.secure.tic();
        let mut o = Vec::new();
        let mut sala_cambio = false;
        if let Some(sala) = &mut self.sala {
            sala.tic();
            if sala.caducada() {
                self.sala = None;
                sala_cambio = true;
            } else if sala.cambiado() {
                sala.sincroniza();
                sala_cambio = true;
            }
        }
        if sala_cambio && pagina {
            o.push(self.estado_json());
        }
        // WebRTC messages for the page (17 Oct 2026), as they come.
        let rtc = self.secure.rtc_para_pagina();
        if pagina {
            for d in rtc {
                o.push(json!({ "tipo": "secure_rtc", "d": d }));
            }
        }
        if !self.secure.media_para_pagina.is_empty() {
            if pagina {
                let trozos: Vec<Value> = self
                    .secure
                    .media_para_pagina
                    .drain(..)
                    .map(|(de, b)| json!({ "de": de, "b": b }))
                    .collect();
                o.push(json!({ "tipo": "secure_media", "trozos": trozos }));
            } else {
                self.secure.media_para_pagina.clear();
            }
        }
        if self.secure.cambiado() {
            self.secure.movil_sincroniza();
            if pagina {
                o.push(self.estado_json());
                for a in self.secure.avisos() {
                    o.push(
                        json!({ "tipo": "secure_aviso", "clave": a["clave"], "texto": a["texto"] }),
                    );
                }
            }
        }
        o
    }

    /// Whether a SECURE conversation is open (the window then watches and hides from captures).
    pub fn conversando(&self) -> bool {
        self.secure.conversando() || self.sala.as_ref().is_some_and(Sala::en_llamada)
    }

    /// What Windows recorded about the microphone, the camera and the screen (17 Oct 2026).
    pub fn vigia(&mut self, usos: &[crate::vigia::Uso]) {
        self.secure.vigia(usos);
        if let Some(s) = &mut self.sala {
            s.vigia(usos);
        }
    }

    /// Whether the window could keep itself out of screenshots and recordings.
    pub fn pantalla(&mut self, si: bool) {
        self.secure.pantalla(si);
        if let Some(s) = &mut self.sala {
            s.pantalla(si);
        }
    }

    /// What the Comunicaciones page asks for (`{"tipo":"secure","accion":…}`).
    pub fn accion(&mut self, m: &Value) -> Vec<Salida> {
        let accion = texto(m, "accion");
        let r: Result<(), String> = match accion {
            "escuchar" => self
                .secure
                .escuchar(u16::try_from(m["puerto"].as_u64().unwrap_or(0)).unwrap_or(0))
                .map(|_| ()),
            "conectar" => self.secure.conectar(texto(m, "direccion")),
            "bloquear" => {
                self.secure.bloquear();
                Ok(())
            }
            "cerrar" => {
                self.secure.cerrar("la persona cerró");
                Ok(())
            }
            "sas" => {
                self.secure
                    .sas_persona(m["iguales"].as_bool().unwrap_or(false), texto(m, "nombre"));
                Ok(())
            }
            "texto" => {
                self.secure
                    .texto_re(texto(m, "texto"), crate::mensaje::id_de(&m["re"]));
                Ok(())
            }
            "editar" => {
                if let Some(id) = crate::mensaje::id_de(&m["id"]) {
                    self.secure.editar(id, texto(m, "texto"));
                }
                Ok(())
            }
            "borrar" => {
                if let Some(id) = crate::mensaje::id_de(&m["id"]) {
                    self.secure
                        .borrar(id, m["para_todos"].as_bool().unwrap_or(false));
                }
                Ok(())
            }
            "borrar_todo" => {
                self.secure.borrar_todo();
                Ok(())
            }
            "reaccion" => {
                if let Some(id) = crate::mensaje::id_de(&m["id"]) {
                    self.secure.reaccionar(id, texto(m, "emoji"));
                }
                Ok(())
            }
            "escribiendo" => {
                self.secure.escribiendo();
                Ok(())
            }
            "sin_visto" => {
                self.secure.sin_visto(m["sin"].as_bool().unwrap_or(false));
                Ok(())
            }
            "zona" => {
                self.secure.pon_zona(texto(m, "zona"));
                Ok(())
            }
            "voz_protegida" => {
                self.secure
                    .voz_protegida(m["activa"].as_bool().unwrap_or(false));
                Ok(())
            }
            "rtc" => {
                self.secure.rtc(&m["d"]);
                return Vec::new();
            }
            "medidas" => {
                self.secure.medidas(&m["d"]);
                return Vec::new();
            }
            "informe_visto" => {
                self.secure.olvida_informe();
                Ok(())
            }
            "sala_informe_visto" => {
                if let Some(s) = &mut self.sala {
                    s.informe_visto();
                }
                Ok(())
            }
            "nombre" => {
                self.secure.nombre_propio = texto(m, "nombre").chars().take(60).collect();
                Ok(())
            }
            "archivo_empieza" => self
                .secure
                .archivo_empieza(texto(m, "nombre"), m["bytes"].as_u64().unwrap_or(0)),
            "archivo_trozo" => self
                .secure
                .archivo_trozo(texto(m, "b64"), m["ultimo"].as_bool().unwrap_or(false)),
            // 1.0.42 (committee with ChatGPT and Grok): a stop for something seen cannot be
            // skipped from the screen any more; it resumes on its own once the cause is gone.
            "reanudar" => Ok(()),
            "llamar" => self
                .secure
                .llamar(m["video"].as_bool().unwrap_or(false), texto(m, "mime")),
            "mime" => {
                self.secure.cambia_mime(texto(m, "mime"));
                Ok(())
            }
            "movil" => self.secure.movil_empieza(texto(m, "idioma")),
            "sala" => match Sala::abre(m["horas"].as_u64().unwrap_or(24), texto(m, "idioma")) {
                Ok(s) => {
                    self.sala = Some(s.con_descargas(&self.descargas));
                    Ok(())
                }
                Err(e) => Err(e),
            },
            "sala_ok" => {
                if let Some(s) = &mut self.sala {
                    s.aprueba(m["i"].as_u64().unwrap_or(99) as usize);
                }
                Ok(())
            }
            "sala_fuera" => {
                if let Some(s) = &mut self.sala {
                    s.expulsa(m["i"].as_u64().unwrap_or(99) as usize);
                }
                Ok(())
            }
            "sala_texto" => {
                if let Some(s) = &mut self.sala {
                    s.texto_re(texto(m, "texto"), crate::mensaje::id_de(&m["re"]));
                }
                Ok(())
            }
            "sala_editar" => {
                if let (Some(s), Some(id)) = (&mut self.sala, crate::mensaje::id_de(&m["id"])) {
                    s.editar(id, texto(m, "texto"));
                }
                Ok(())
            }
            "sala_borrar" => {
                if let (Some(s), Some(id)) = (&mut self.sala, crate::mensaje::id_de(&m["id"])) {
                    s.borrar(id, m["para_todos"].as_bool().unwrap_or(false));
                }
                Ok(())
            }
            "sala_borrar_todo" => {
                if let Some(s) = &mut self.sala {
                    s.borrar_todo();
                }
                Ok(())
            }
            "sala_reaccion" => {
                if let (Some(s), Some(id)) = (&mut self.sala, crate::mensaje::id_de(&m["id"])) {
                    s.reaccionar(id, texto(m, "emoji"));
                }
                Ok(())
            }
            "sala_nota" => {
                // A phone's voice note, to play on the computer (17 Oct 2026).
                let Some((tipo, ruta)) = self
                    .sala
                    .as_ref()
                    .zip(m["id"].as_u64().and_then(|i| u32::try_from(i).ok()))
                    .and_then(|(s, id)| s.nota_de_voz(id))
                else {
                    return Vec::new();
                };
                let Ok(datos) = std::fs::read(&ruta) else {
                    return Vec::new();
                };
                let tipo = if tipo.starts_with("audio/") {
                    tipo
                } else {
                    "audio/mp4".to_owned()
                };
                let url = format!(
                    "data:{tipo};base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(datos)
                );
                return vec![Salida::Pagina(json!({
                    "tipo": "secure_vista",
                    "id": format!("sala-{}", m["id"].as_u64().unwrap_or_default()),
                    "url": url,
                }))];
            }
            "sala_cerrar" => {
                self.sala = None;
                Ok(())
            }
            // «Actualizar» on the page once the talk is over (17 Oct 2026, the owner: «le doy
            // actualizar y se tiene que borrar todo»): the conversation, its messages, the
            // invitation and its link go; a call still going is never cut by a reload.
            "empezar_limpio" => {
                if !self.en_llamada() {
                    self.secure.cerrar("la persona empezó de nuevo");
                    self.secure.borrar_todo();
                    self.secure.olvida_informe();
                    self.sala = None;
                }
                Ok(())
            }
            "movil_ok" => {
                self.secure.movil_aprueba();
                Ok(())
            }
            "movil_fin" => {
                self.secure.movil_fin();
                Ok(())
            }
            "colgar" => {
                self.secure.colgar();
                Ok(())
            }
            "contestar" => self.secure.contestar(texto(m, "mime")),
            "ajustes_privacidad" => {
                // Windows' own page for the microphone or the camera, opened for the person.
                let url = if texto(m, "que") == "camara" {
                    "ms-settings:privacy-webcam"
                } else {
                    "ms-settings:privacy-microphone"
                };
                return vec![Salida::AbreFuera {
                    url: url.to_owned(),
                }];
            }
            "rechazar" => {
                self.secure.rechazar();
                Ok(())
            }
            "pide_inicio" => {
                self.secure.pide_inicio();
                return Vec::new();
            }
            "perdida_vista" => {
                self.secure.olvida_perdida();
                Ok(())
            }
            "incidente_vista" => {
                self.secure.olvida_incidente();
                Ok(())
            }
            "incidente_guardar" => {
                let Some(j) = self.secure.informe_incidente() else {
                    return Vec::new();
                };
                return vec![Salida::Pagina(
                    json!({ "tipo": "secure_incidente", "json": j }),
                )];
            }
            "archivo_abrir" | "archivo_carpeta" => {
                let Some(r) = m["id"].as_u64().and_then(|i| self.secure.recibido(i)) else {
                    return Vec::new();
                };
                if r.borrado {
                    return Vec::new();
                }
                let ruta = r.ruta.to_string_lossy().into_owned();
                return if accion == "archivo_abrir" && r.abrible() {
                    vec![Salida::AbreArchivo { ruta }]
                } else {
                    vec![Salida::MuestraArchivo { ruta }]
                };
            }
            "archivo_borrar" => {
                if let Some(i) = m["id"].as_u64() {
                    self.secure.borra_recibido(i);
                }
                Ok(())
            }
            "archivo_vista" => {
                // A small picture for the chat, or a voice note to play there: at most 12 MB,
                // read once.
                let Some(r) = m["id"].as_u64().and_then(|i| self.secure.recibido(i)) else {
                    return Vec::new();
                };
                if !(r.imagen() || r.audio()) || r.borrado || r.bytes > 12 * 1024 * 1024 {
                    return Vec::new();
                }
                let Ok(datos) = std::fs::read(&r.ruta) else {
                    return Vec::new();
                };
                let tipo = match r
                    .nombre
                    .rsplit_once('.')
                    .map(|(_, e)| e.to_ascii_lowercase())
                    .as_deref()
                {
                    _ if r.audio() => crate::mensaje::mime_de(&r.nombre),
                    Some("png") => "image/png",
                    Some("gif") => "image/gif",
                    Some("webp") => "image/webp",
                    Some("bmp") => "image/bmp",
                    _ => "image/jpeg",
                };
                let url = format!(
                    "data:{tipo};base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(datos)
                );
                return vec![Salida::Pagina(
                    json!({ "tipo": "secure_vista", "id": r.id, "url": url }),
                )];
            }
            "media" => self.secure.media_saliente(texto(m, "b64")),
            "visto" => {
                self.secure.visto();
                if let Some(s) = &mut self.sala {
                    s.visto();
                }
                Ok(())
            }
            // «Compartir sin huellas» (customer request, 18 Oct 2026): before a photo or a
            // document goes, what its metadata says about the person (GPS, phone, author,
            // company, dates), and a copy without it ready in the same answer. Only the headers
            // are read (`guardiana_core::huellas`); the page decides with the person which one
            // goes. Nothing leaves this computer here.
            "huellas" => {
                let b64 = &base64::engine::general_purpose::STANDARD;
                let datos = b64.decode(texto(m, "b64")).unwrap_or_default();
                let informe = guardiana_core::huellas::mirar(&datos);
                let limpio = (informe.limpiable && !informe.huellas.is_empty())
                    .then(|| guardiana_core::huellas::limpiar(&datos).ok())
                    .flatten()
                    .map(|l| b64.encode(l));
                return vec![Salida::Pagina(json!({
                    "tipo": "secure_huellas",
                    "id": m["id"].clone(),
                    "informe": informe,
                    "nombre_limpio": guardiana_core::huellas::nombre_limpio(texto(m, "nombre")),
                    "b64": limpio,
                }))];
            }
            "prueba" => {
                let j = self.secure.prueba_json();
                return vec![Salida::Pagina(
                    json!({ "tipo": "secure_prueba", "json": j }),
                )];
            }
            _ => Ok(()),
        };
        let mut o = Vec::new();
        if let Err(e) = r {
            o.push(Salida::Pagina(
                json!({ "tipo": "secure_aviso", "clave": "sec_error", "texto": e }),
            ));
        }
        if accion != "media" && accion != "archivo_trozo" && accion != "mime" {
            o.push(Salida::Pagina(self.estado_json()));
            self.secure.movil_sincroniza();
            if let Some(sala) = &mut self.sala {
                if sala.cambiado() {
                    sala.sincroniza();
                }
            }
        }
        o
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn carpeta(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "guardiana-comunicaciones-{}-{nombre}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::create_dir_all(d.join("descargas"));
        d
    }

    #[test]
    fn its_state_is_the_conversations_plus_the_room() {
        let d = carpeta("estado");
        let c = Comunicaciones::abre(&d.join("datos"), &d.join("descargas"), "0.1.0");
        // Exactly what the browser's SECURE tab drew before the engine left `Sesion`: the
        // group's state, with `sala` null while no room is open.
        // The contact card is signed with the second it is made in, so two readings a moment
        // apart may differ there and only there (seen on macOS CI, 17 Oct 2026).
        let sin_tarjeta = |mut e: Value| {
            if let Some(o) = e.as_object_mut() {
                o.remove("tarjeta");
            }
            e
        };
        let mut esperado = c.secure.estado_json();
        esperado["sala"] = Value::Null;
        let estado = c.estado_json();
        assert_eq!(estado["tipo"], "secure_estado");
        assert!(estado["tarjeta"].is_string());
        assert_eq!(sin_tarjeta(estado), sin_tarjeta(esperado));
        assert!(d
            .join("datos")
            .join("secure")
            .join("identidad.txt")
            .is_file());
        assert!(!c.en_llamada());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_action_answers_with_the_new_state_and_outside_actions_say_what_to_open() {
        let d = carpeta("accion");
        let mut c = Comunicaciones::abre(&d.join("datos"), &d.join("descargas"), "0.1.0");
        let o = c.accion(&json!({ "tipo": "secure", "accion": "escuchar", "puerto": 0 }));
        let Some(Salida::Pagina(e)) = o.last() else {
            unreachable!("the state comes back last: {o:?}");
        };
        assert_eq!(e["tipo"], "secure_estado");
        assert_eq!(e["fase"], "escuchando");
        assert_eq!(
            c.accion(&json!({ "accion": "ajustes_privacidad", "que": "camara" })),
            vec![Salida::AbreFuera {
                url: "ms-settings:privacy-webcam".into()
            }]
        );
        // A WebRTC message goes to the conversation and nothing comes back at once.
        assert!(c.accion(&json!({ "accion": "rtc", "d": {} })).is_empty());
        // Without a page that said «listo», the clock says nothing to anybody.
        assert!(c.tic(false).is_empty());
        c.accion(&json!({ "accion": "cerrar" }));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// «Compartir sin huellas»: a photo with GPS gets its findings and a clean copy in one
    /// answer; a plain text gets neither, and nothing else changes in the conversation.
    #[test]
    fn a_photo_about_to_go_says_what_it_reveals_and_brings_its_clean_copy() {
        use base64::Engine as _;
        let d = carpeta("huellas");
        let mut c = Comunicaciones::abre(&d.join("datos"), &d.join("descargas"), "0.1.0");
        let b64 = &base64::engine::general_purpose::STANDARD;
        let foto = include_bytes!("../../core/tests/huellas/foto-gps.jpg");
        let o = c.accion(&json!({ "accion": "huellas", "id": 7, "nombre": "playa.jpg", "b64": b64.encode(foto) }));
        let [Salida::Pagina(r)] = o.as_slice() else {
            unreachable!("one answer and no state: {o:?}");
        };
        assert_eq!(r["tipo"], "secure_huellas");
        assert_eq!(r["id"], 7);
        assert_eq!(r["informe"]["huellas"][0]["clave"], "gps");
        assert_eq!(r["nombre_limpio"], "playa-sin-huellas.jpg");
        let limpio = b64
            .decode(r["b64"].as_str().unwrap_or_default())
            .unwrap_or_default();
        assert!(guardiana_core::huellas::mirar(&limpio).huellas.is_empty());
        let o = c.accion(
            &json!({ "accion": "huellas", "id": 8, "nombre": "a.txt", "b64": b64.encode("hola") }),
        );
        let [Salida::Pagina(r)] = o.as_slice() else {
            unreachable!("{o:?}");
        };
        assert_eq!(r["informe"]["formato"], "otro");
        assert!(r["b64"].is_null());
        let _ = std::fs::remove_dir_all(&d);
    }
}
