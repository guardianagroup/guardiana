//! Sizes in buckets, so whoever carries the bytes (the relay, a network in between) cannot tell
//! one message from another by its length. The owner, 18 Oct 2026: «quiero la máxima tecnología
//! disponible actualmente en el chat»; the committee asked for the metadata to be minimised.
//!
//! The rule is Padmé (Nikitin, Barman, Lueks, Underwood, Hubaux, Ford: «Reducing Metadata Leakage
//! from Encrypted Files and Communication with PURBs», PETS 2019): a length `L` is rounded up so
//! that only its top `⌊log2 ⌊log2 L⌋⌋ + 1` bits are kept. It leaks O(log log L) bits of the size
//! instead of O(log L) and never adds more than about 12 %. It is a padding rule, not a cipher:
//! the bytes it adds are encrypted with the rest, or are whitespace outside JSON that every reader
//! already skips. Below [`MINIMO`] everything is one size: a «typing…», a «seen» and a short text
//! all look the same.
//!
//! What it does not hide, said plainly: how many frames go and when (a call still shows as a
//! steady stream), and the size class of a large file block.

/// Every padded frame is at least this many bytes.
pub const MINIMO: usize = 256;

/// The Padmé size for `largo` bytes, never below [`MINIMO`].
#[must_use]
pub fn cubo(largo: usize) -> usize {
    let l = largo.max(MINIMO) as u64;
    // E = ⌊log2 L⌋, S = ⌊log2 E⌋ + 1; the last E − S bits are rounded up.
    let e = 63 - u64::from(l.leading_zeros());
    let s = (63 - u64::from(e.max(1).leading_zeros())) + 1;
    let ultimos = e.saturating_sub(s);
    let mascara = (1u64 << ultimos) - 1;
    usize::try_from((l + mascara) & !mascara).unwrap_or(largo)
}

/// Grow `datos` to its bucket with `relleno` bytes, but never past `maximo` (a frame that would
/// pass the reader's limit keeps its own size).
pub fn rellena(datos: &mut Vec<u8>, relleno: u8, maximo: usize) {
    let objetivo = cubo(datos.len()).min(maximo);
    if objetivo > datos.len() {
        datos.resize(objetivo, relleno);
    }
}

/// A payload with its own length in front (4 bytes, big-endian) and zeros up to its bucket, for
/// a channel that seals the whole thing: the reader takes the length back with [`sin_relleno`].
#[must_use]
pub fn con_largo(datos: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(cubo(datos.len() + 4));
    v.extend_from_slice(&(datos.len() as u32).to_be_bytes());
    v.extend_from_slice(datos);
    let objetivo = cubo(v.len());
    v.resize(objetivo, 0);
    v
}

/// The payload inside [`con_largo`]; `None` when the length does not fit (a frame cut short).
#[must_use]
pub fn sin_relleno(v: &[u8]) -> Option<&[u8]> {
    let (largo, resto) = v.split_first_chunk::<4>()?;
    let n = u32::from_be_bytes(*largo) as usize;
    resto.get(..n)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn small_messages_all_have_one_size_and_large_ones_few() {
        // «escribiendo», «visto» and a short text: the same length on the wire.
        assert_eq!(cubo(0), MINIMO);
        assert_eq!(cubo(17), MINIMO);
        assert_eq!(cubo(MINIMO), MINIMO);
        // Padmé's own examples: 9 bits of a 1000-byte length become 4 kept bits.
        assert_eq!(cubo(1000), 1024);
        assert_eq!(cubo(1025), 1088);
        // Never shrinks, never more than 12 % (plus the floor).
        for l in [257usize, 999, 4097, 65_536 + 22_000, 1_048_577, 2_000_000] {
            let c = cubo(l);
            assert!(c >= l, "{l} -> {c}");
            assert!((c - l) as f64 <= l as f64 * 0.12 + 1.0, "{l} -> {c}");
        }
        // Many lengths fall into few buckets: what the relay sees is coarse.
        let distintos: std::collections::BTreeSet<usize> = (1000..2000).map(cubo).collect();
        assert!(distintos.len() <= 20, "{}", distintos.len());
    }

    #[test]
    fn padding_is_reversible_and_respects_the_limit() {
        let mut j = br#"{"t":"latido"}"#.to_vec();
        rellena(&mut j, b' ', 1 << 21);
        assert_eq!(j.len(), MINIMO);
        let v: serde_json::Value = serde_json::from_slice(&j).unwrap();
        assert_eq!(v["t"], "latido", "trailing spaces are still the same JSON");
        let mut grande = vec![b'x'; 1000];
        rellena(&mut grande, b' ', 1010);
        assert_eq!(grande.len(), 1010, "never past the reader's limit");

        let p = con_largo(b"hola");
        assert_eq!(p.len(), MINIMO);
        assert_eq!(sin_relleno(&p).unwrap(), b"hola");
        assert!(
            sin_relleno(&[0, 0, 1, 0, 1]).is_none(),
            "a length that does not fit"
        );
        assert!(sin_relleno(&[0, 0]).is_none());
    }
}
