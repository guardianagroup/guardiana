//! An identity locked by the 1.0.57 (vodozemac 0.10) opens with this version (vodozemac 0.11.2) and is the same
//! identity, with the same public keys (second voice of the 1.0.58, 15 Nov 2026: if it did not, ZERO would set the old
//! file aside and start a new identity, and every contact would see it change). The block was made on the 1.0.57 by a
//! throwaway branch that printed it (`fixture-identidad-1057`, never merged), with the key `[7; 32]`.

const BLOQUE: &str = concat!(
    "Q84MLgJesWfhY6UN42InJDvtS9jarmXYVDnPdcQVttB8f91gmf9hyaNsr7Fs/hpMMKncOWvs0hwdFFUJ",
    "XOP+JacM1HWIyVZ2y+WWGb/PUhHgwE/+aVOd19caGUjyq4l/hpL+T2iD2tKb1fmrSmH29tvuoJHMrQLZ",
    "wX2s+E8NN9FwO5oT7A2fB1vdOh9TtY+v/SA5ApyzILMfnzIKELvwKCJj1VZj9ZjTCWrWroftKBTkVTXB",
    "By8Qij3KIHSYCqOI/rq+ywosUKYHFH+8iFcqEAVa6Uu7/hiNPWjcMuHbVRPYv83X4QEMR6qzV3nmr+CL",
    "lnB0dfDc9eIJeqnuzsbkqskq1xd7I45tSRmcaZTiD9KOAfj4YhA/38Ehr9P0PymjGrA2dsMzo1hSpc/J",
    "5buWwiheDI3xWjfj0dR/1mhsJoFzGic38XsL6T38AHAhQz0E0lH0ga99sF9Pii3nF7VZulXEWaNRxf7/",
    "RNYQ2fUMn/1gjdaqdnDHetQjjyBSjQkEuEYgE7urXUjbxtqPpgrp/4cKwSX+6tCgaGFPqhBGYob1m6es",
    "v8jLFMl9Jr4XjklA1hBYinNl1lA",
);
const CURVE: &str = "WQtBd57qlr1VM8NX4S1a5SzjWYQ2dtfv5L0qvQ+rw3s";
const ED: &str = "FlsxhHmYiSsxAEcCp2K2XPwyWQkDlUxxyIDEf/79wqY";

#[test]
fn la_identidad_de_la_1_0_57_se_abre_y_es_la_misma() {
    let abierta = guardiana_secure::Identity::unlock(BLOQUE, &[7u8; 32]);
    assert!(abierta.is_ok(), "la identidad de la 1.0.57 no se abre");
    if let Ok(i) = abierta {
        assert_eq!(i.public_key().to_base64(), CURVE);
        assert_eq!(i.signing_key().to_base64(), ED);
    }
    // con otra clave no se abre
    assert!(guardiana_secure::Identity::unlock(BLOQUE, &[8u8; 32]).is_err());
}
