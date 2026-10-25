//! La hora local en el Mac (en Windows la da `pixpin_shell::entorno`).

/// Milisegundos locales de ahora: UTC mas el desfase del huso (`date +%z`).
pub fn ahora_local_ms() -> i64 {
    ahora_utc_ms() + desfase_ms()
}

pub fn ahora_utc_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Local menos UTC, en ms («-0500» → −5 h).
pub fn desfase_ms() -> i64 {
    let salida = std::process::Command::new("date").arg("+%z").output().ok();
    let t = salida.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    desfase_de(&t)
}

pub fn desfase_de(t: &str) -> i64 {
    let (signo, resto) = match t.as_bytes().first() {
        Some(b'-') => (-1, &t[1..]),
        Some(b'+') => (1, &t[1..]),
        _ => return 0,
    };
    if resto.len() != 4 || !resto.chars().all(|c| c.is_ascii_digit()) {
        return 0;
    }
    let h: i64 = resto[..2].parse().unwrap_or(0);
    let m: i64 = resto[2..].parse().unwrap_or(0);
    signo * (h * 60 + m) * 60_000
}

#[cfg(test)]
mod pruebas {
    #[test]
    fn el_desfase_sale_de_date() {
        assert_eq!(super::desfase_de("-0500"), -5 * 3_600_000);
        assert_eq!(super::desfase_de("+0530"), (5 * 60 + 30) * 60_000);
        assert_eq!(super::desfase_de("roto"), 0);
    }
}
