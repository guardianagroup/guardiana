//! Personal data in a piece of text: what the form guard names before a page receives it, and
//! what the redaction takes out before a prompt reaches an AI. Each kind is recognised by its
//! own shape and, where it has one, its check digit (a card's Luhn, an IBAN's mod 97, a CPF's
//! two digits, a Spanish DNI's letter), so a long number is not taken for a card by accident.
//! A name cannot be recognised by its shape: only the names the person marked are found.

use serde::{Deserialize, Serialize};

use crate::tinta::{Marcado, Tipo};

/// Kinds of personal data recognised in text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Clase {
    /// An email address.
    Correo,
    /// A phone number.
    Telefono,
    /// A payment card number (Luhn).
    Tarjeta,
    /// A bank account in IBAN form (mod 97).
    Cuenta,
    /// An identity document (CPF, DNI/NIE, US SSN, or one the person marked).
    Documento,
    /// A name the person marked.
    Nombre,
    /// Anything else the person marked.
    Otro,
}

impl Clase {
    /// The key of its name in the browser's texts (`dato_correo`...).
    #[must_use]
    pub const fn clave(self) -> &'static str {
        match self {
            Self::Correo => "dato_correo",
            Self::Telefono => "dato_telefono",
            Self::Tarjeta => "dato_tarjeta",
            Self::Cuenta => "dato_cuenta",
            Self::Documento => "dato_documento",
            Self::Nombre => "dato_nombre",
            Self::Otro => "dato_otro",
        }
    }
}

/// A piece of personal data found in a text, by byte offsets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Trozo {
    /// First byte.
    pub inicio: usize,
    /// One past the last byte.
    pub fin: usize,
    /// What it is.
    pub clase: Clase,
}

fn es_local(c: u8) -> bool {
    c.is_ascii_alphanumeric() || b"._%+-".contains(&c)
}

fn es_dominio(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'.' || c == b'-'
}

fn correos(t: &[u8], out: &mut Vec<Trozo>) {
    for (i, &c) in t.iter().enumerate() {
        if c != b'@' {
            continue;
        }
        let mut a = i;
        while a > 0 && es_local(t[a - 1]) {
            a -= 1;
        }
        let mut b = i + 1;
        while b < t.len() && es_dominio(t[b]) {
            b += 1;
        }
        while b > i + 1 && (t[b - 1] == b'.' || t[b - 1] == b'-') {
            b -= 1;
        }
        let dominio = &t[i + 1..b];
        let Some(punto) = dominio.iter().rposition(|&c| c == b'.') else {
            continue;
        };
        let tld = &dominio[punto + 1..];
        if a < i && punto > 0 && tld.len() >= 2 && tld.iter().all(u8::is_ascii_alphabetic) {
            out.push(Trozo {
                inicio: a,
                fin: b,
                clase: Clase::Correo,
            });
        }
    }
}

fn luhn(d: &[u8]) -> bool {
    let mut suma = 0u32;
    for (i, &x) in d.iter().rev().enumerate() {
        let mut v = u32::from(x);
        if i % 2 == 1 {
            v *= 2;
            if v > 9 {
                v -= 9;
            }
        }
        suma += v;
    }
    suma % 10 == 0
}

fn cpf_valido(d: &[u8]) -> bool {
    if d.len() != 11 || d.iter().all(|&x| x == d[0]) {
        return false;
    }
    let digito = |n: usize| -> u8 {
        let s: u32 = (0..n)
            .map(|i| u32::from(d[i]) * (n as u32 + 1 - i as u32))
            .sum();
        let r = (s * 10) % 11;
        if r == 10 {
            0
        } else {
            #[allow(clippy::cast_possible_truncation)]
            let r = r as u8;
            r
        }
    };
    digito(9) == d[9] && digito(10) == d[10]
}

/// Runs of digits with the separators people write inside numbers (spaces, dashes, dots,
/// parentheses, a leading +), as (start, end, digits).
fn numeros(t: &[u8]) -> Vec<(usize, usize, Vec<u8>)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < t.len() {
        let empieza = t[i].is_ascii_digit()
            || (t[i] == b'+' && t.get(i + 1).is_some_and(u8::is_ascii_digit))
            || (t[i] == b'(' && t.get(i + 1).is_some_and(u8::is_ascii_digit));
        let pegado = i > 0 && (t[i - 1].is_ascii_alphanumeric());
        if !empieza || pegado {
            i += 1;
            continue;
        }
        let inicio = i;
        let mut digitos = Vec::new();
        let mut fin = i;
        while i < t.len() {
            let c = t[i];
            if c.is_ascii_digit() {
                digitos.push(c - b'0');
                i += 1;
                fin = i;
            } else if b" -.()+".contains(&c)
                && t.get(i + 1)
                    .is_some_and(|n| n.is_ascii_digit() || *n == b'(')
            {
                i += 1;
            } else {
                break;
            }
        }
        let tras_letra = t.get(fin).is_some_and(u8::is_ascii_alphabetic);
        if !tras_letra {
            out.push((inicio, fin, digitos));
        }
        i = fin.max(inicio + 1);
    }
    out
}

fn numericos(t: &[u8], out: &mut Vec<Trozo>) {
    for (inicio, fin, d) in numeros(t) {
        let texto = &t[inicio..fin];
        let clase = if (13..=19).contains(&d.len()) && luhn(&d) {
            Some(Clase::Tarjeta)
        } else if d.len() == 11 && cpf_valido(&d) {
            Some(Clase::Documento)
        } else if d.len() == 9 && texto.len() == 11 && texto[3] == b'-' && texto[6] == b'-' {
            // US Social Security number, 123-45-6789.
            Some(Clase::Documento)
        } else if (9..=15).contains(&d.len())
            && (texto[0] == b'+' || texto[0] == b'(' || d.len() >= 10)
        {
            Some(Clase::Telefono)
        } else {
            None
        };
        if let Some(clase) = clase {
            out.push(Trozo { inicio, fin, clase });
        }
    }
}

fn iban_valido(s: &[u8]) -> bool {
    if s.len() < 15 || s.len() > 34 {
        return false;
    }
    let mut resto: u64 = 0;
    for &c in s[4..].iter().chain(&s[..4]) {
        let v = if c.is_ascii_digit() {
            u64::from(c - b'0')
        } else {
            u64::from(c.to_ascii_uppercase() - b'A') + 10
        };
        resto = if v >= 10 {
            (resto * 100 + v) % 97
        } else {
            (resto * 10 + v) % 97
        };
    }
    resto == 1
}

fn ibans(t: &[u8], out: &mut Vec<Trozo>) {
    let mut i = 0;
    while i + 4 <= t.len() {
        let pegado = i > 0 && t[i - 1].is_ascii_alphanumeric();
        if pegado
            || !t[i].is_ascii_uppercase()
            || !t[i + 1].is_ascii_uppercase()
            || !t[i + 2].is_ascii_digit()
            || !t[i + 3].is_ascii_digit()
        {
            i += 1;
            continue;
        }
        let mut j = i;
        let mut limpio = Vec::new();
        let mut fin = i;
        while j < t.len() && limpio.len() < 34 {
            if t[j].is_ascii_alphanumeric() {
                limpio.push(t[j]);
                j += 1;
                fin = j;
            } else if t[j] == b' ' && t.get(j + 1).is_some_and(u8::is_ascii_alphanumeric) {
                j += 1;
            } else {
                break;
            }
        }
        if iban_valido(&limpio) {
            out.push(Trozo {
                inicio: i,
                fin,
                clase: Clase::Cuenta,
            });
            i = fin;
        } else {
            i += 1;
        }
    }
}

const LETRAS_DNI: &[u8] = b"TRWAGMYFPDXBNJZSQVHLCKE";

fn dnis(t: &[u8], out: &mut Vec<Trozo>) {
    let mut i = 0;
    while i + 9 <= t.len() {
        let pegado = i > 0 && t[i - 1].is_ascii_alphanumeric();
        let w = &t[i..i + 9];
        let despues = t.get(i + 9).is_some_and(u8::is_ascii_alphanumeric);
        if !pegado && !despues && w[8].is_ascii_alphabetic() {
            let (prefijo, cuerpo) = match w[0] {
                b'X' | b'x' => (0u32, &w[1..8]),
                b'Y' | b'y' => (1, &w[1..8]),
                b'Z' | b'z' => (2, &w[1..8]),
                _ => (u32::MAX, &w[0..8]),
            };
            let numero_ok = cuerpo.iter().all(u8::is_ascii_digit)
                && (prefijo != u32::MAX || w[0].is_ascii_digit());
            if numero_ok {
                let mut n: u32 = if prefijo == u32::MAX { 0 } else { prefijo };
                for &c in cuerpo {
                    n = n * 10 + u32::from(c - b'0');
                }
                if LETRAS_DNI[(n % 23) as usize] == w[8].to_ascii_uppercase() {
                    out.push(Trozo {
                        inicio: i,
                        fin: i + 9,
                        clase: Clase::Documento,
                    });
                    i += 9;
                    continue;
                }
            }
        }
        i += 1;
    }
}

/// `texto` in lower case, with, for each byte of it, where its character starts and ends in
/// `texto`. Lowering can change a character's length («İ», «ẞ», the Kelvin sign): searching the
/// lowered text and mapping back keeps every other match in place. Skipping the search whenever
/// lengths changed, as before, let one such letter anywhere switch marked data off (review of
/// 10 Oct 2026).
fn en_minusculas(texto: &str) -> (String, Vec<(usize, usize)>) {
    let mut bajo = String::with_capacity(texto.len());
    let mut mapa = Vec::with_capacity(texto.len());
    for (i, c) in texto.char_indices() {
        let fin = i + c.len_utf8();
        for l in c.to_lowercase() {
            bajo.push(l);
            mapa.extend(std::iter::repeat_n((i, fin), l.len_utf8()));
        }
    }
    (bajo, mapa)
}

fn marcados(texto: &str, lista: &[Marcado], out: &mut Vec<Trozo>) {
    let (bajo, mapa) = en_minusculas(texto);
    for m in lista {
        let valor = m.valor.trim().to_lowercase();
        if valor.chars().count() < 3 {
            continue;
        }
        let clase = match m.tipo {
            Tipo::Correo => Clase::Correo,
            Tipo::Telefono => Clase::Telefono,
            Tipo::Documento => Clase::Documento,
            Tipo::Nombre => Clase::Nombre,
            Tipo::Otro | Tipo::Senuelo => Clase::Otro,
        };
        let mut desde = 0;
        while let Some(p) = bajo[desde..].find(&valor) {
            let inicio = desde + p;
            let fin = inicio + valor.len();
            if let (Some(&(a, _)), Some(&(_, b))) = (mapa.get(inicio), mapa.get(fin - 1)) {
                out.push(Trozo {
                    inicio: a,
                    fin: b,
                    clase,
                });
            }
            desde = fin;
        }
    }
}

/// The personal data in `texto`, sorted and without overlaps (the first and longest wins).
#[must_use]
pub fn detecta(texto: &str, lista: &[Marcado]) -> Vec<Trozo> {
    let t = texto.as_bytes();
    let mut todos = Vec::new();
    marcados(texto, lista, &mut todos);
    correos(t, &mut todos);
    ibans(t, &mut todos);
    dnis(t, &mut todos);
    numericos(t, &mut todos);
    todos.sort_by(|a, b| a.inicio.cmp(&b.inicio).then(b.fin.cmp(&a.fin)));
    let mut out: Vec<Trozo> = Vec::new();
    for tr in todos {
        if out.last().is_some_and(|u| tr.inicio < u.fin) {
            continue;
        }
        // Only on character boundaries, so the caller can slice the text.
        if texto.is_char_boundary(tr.inicio) && texto.is_char_boundary(tr.fin) {
            out.push(tr);
        }
    }
    out
}

/// The kinds present in a text, in a stable order, for «this page will receive: ...».
#[must_use]
pub fn clases(texto: &str, lista: &[Marcado]) -> Vec<Clase> {
    let mut v: Vec<Clase> = Vec::new();
    for t in detecta(texto, lista) {
        if !v.contains(&t.clase) {
            v.push(t.clase);
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tipos(t: &str) -> Vec<(String, Clase)> {
        detecta(t, &[])
            .into_iter()
            .map(|x| (t[x.inicio..x.fin].to_string(), x.clase))
            .collect()
    }

    #[test]
    fn emails_phones_and_cards_are_told_apart() {
        assert_eq!(
            tipos("Escríbeme a ana.perez@correo.com.co o llama al +57 300 123 4567."),
            vec![
                ("ana.perez@correo.com.co".into(), Clase::Correo),
                ("+57 300 123 4567".into(), Clase::Telefono),
            ]
        );
        // A Visa test number passes Luhn; the same digits changed by one do not.
        assert_eq!(tipos("tarjeta 4111 1111 1111 1111")[0].1, Clase::Tarjeta);
        assert_ne!(
            tipos("tarjeta 4111 1111 1111 1112").first().map(|x| x.1),
            Some(Clase::Tarjeta)
        );
    }

    #[test]
    fn ordinary_numbers_are_left_alone() {
        assert!(tipos("Son 1.250.000 pesos, el 12/10/2026, en el piso 3").is_empty());
        assert!(tipos("Pedido 12345678").is_empty());
    }

    #[test]
    fn documents_with_a_check_digit_are_recognised() {
        assert_eq!(tipos("CPF 529.982.247-25")[0].1, Clase::Documento);
        assert_eq!(tipos("DNI 12345678Z")[0].1, Clase::Documento);
        assert!(tipos("DNI 12345678A").is_empty());
        assert_eq!(tipos("SSN 123-45-6789")[0].1, Clase::Documento);
        assert_eq!(
            tipos("IBAN ES91 2100 0418 4502 0005 1332")[0].1,
            Clase::Cuenta
        );
    }

    #[test]
    fn a_marked_name_is_found_whatever_its_case() {
        let lista = [Marcado {
            tipo: Tipo::Nombre,
            valor: "Francisco Salvatierra".into(),
        }];
        let t = "Hola, soy FRANCISCO SALVATIERRA y vivo en Cartagena";
        let v = detecta(t, &lista);
        assert_eq!(v.len(), 1);
        assert_eq!(&t[v[0].inicio..v[0].fin], "FRANCISCO SALVATIERRA");
        assert_eq!(v[0].clase, Clase::Nombre);
    }

    #[test]
    fn a_letter_that_changes_length_when_lowered_does_not_hide_marked_data() {
        let lista = [Marcado {
            tipo: Tipo::Nombre,
            valor: "Juan Perez".into(),
        }];
        for texto in [
            "Ali İnan\nJuan Perez",
            "\u{212A}elvin y JUAN PEREZ",
            "STRAẞE: Juan Perez",
        ] {
            let t = detecta(texto, &lista);
            assert_eq!(t.len(), 1, "{texto}");
            assert_eq!(
                texto[t[0].inicio..t[0].fin].to_lowercase(),
                "juan perez",
                "{texto}"
            );
        }
        // The match maps back to the original letters even when they changed length themselves.
        let t = detecta("İİ Juan Perez İ", &lista);
        assert_eq!(&"İİ Juan Perez İ"[t[0].inicio..t[0].fin], "Juan Perez");
    }
}
