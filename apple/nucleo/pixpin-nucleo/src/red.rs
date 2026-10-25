//! **La red de verdad**: de un `host` en texto a una conexion, con IPv4 e
//! IPv6 (fase 0.3 del plan, §4.2.3).
//!
//! Swift da el host tal como lo resuelve Network.framework (`Vecinos`):
//! `192.168.1.20`, `fe80::1%en0` (enlace local con su interfaz),
//! `[2001:db8::1]` (entre corchetes, como se escribe con puerto) o un nombre
//! (`pc.local`). Aqui se aceptan todos:
//!
//! - una IP en texto se lee como `IpAddr` sin preguntar a nadie (sin DNS);
//! - una IPv6 de enlace local lleva su interfaz (`%en0` o `%4`): con el
//!   numero se usa tal cual; con el nombre se le pide a `getaddrinfo` (la
//!   biblioteca estandar no trae `if_nametoindex`, y `getaddrinfo` del
//!   sistema si entiende `fe80::1%en0`);
//! - un nombre se resuelve con el sistema.
//!
//! **IPv4 primero**: Windows y Android escuchan en IPv4 (`0.0.0.0`), asi que
//! con varias direcciones se prueban antes las IPv4 y despues las IPv6. Se
//! prueba cada una con su tope de 8 s hasta que una conecta.
//!
//! Nada de esto cambia el protocolo: es solo como se llega al otro.

use std::net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6, TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::ErrorNucleo;

/// Lo que se espera a que el otro conteste al conectar (`Red.conectar` de
/// Windows).
pub const ESPERA_AL_CONECTAR: Duration = Duration::from_secs(8);
/// Lo que se espera a que llegue algo una vez hablando: mientras el otro
/// decide que sincronizar no llega nada.
pub const ESPERA_AL_LEER: Duration = Duration::from_secs(120);

/// El host sin corchetes ni blancos: `[fe80::1%en0]` → `fe80::1%en0`.
pub fn limpiar_host(host: &str) -> &str {
    let h = host.trim();
    h.strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(h)
}

/// Una IPv6 con su interfaz por numero (`fe80::1%4`), sin preguntar a nadie.
/// Con el nombre de la interfaz (`%en0`) da `None`: eso lo resuelve el
/// sistema.
fn ipv6_con_alcance(host: &str) -> Option<SocketAddr> {
    let (ip, alcance) = host.split_once('%')?;
    let ip: Ipv6Addr = ip.parse().ok()?;
    let alcance: u32 = alcance.parse().ok()?;
    Some(SocketAddr::V6(SocketAddrV6::new(ip, 0, 0, alcance)))
}

/// **Todas las direcciones de `host:puerto`, IPv4 primero.** Una IP escrita
/// no pasa por el DNS. Error si no sale ninguna.
pub fn direcciones(host: &str, puerto: u16) -> Result<Vec<SocketAddr>, ErrorNucleo> {
    let h = limpiar_host(host);
    if h.is_empty() {
        return Err(ErrorNucleo::Argumento {
            detalle: "falta la direccion del otro aparato".into(),
        });
    }
    let mut v: Vec<SocketAddr> = if let Ok(ip) = h.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, puerto)]
    } else if let Some(mut d) = ipv6_con_alcance(h) {
        d.set_port(puerto);
        vec![d]
    } else {
        (h, puerto)
            .to_socket_addrs()
            .map_err(|e| ErrorNucleo::Sincro {
                detalle: format!("no se encuentra {h}: {e}"),
            })?
            .collect()
    };
    // Estable: dentro de cada familia, el orden del sistema.
    v.sort_by_key(|d| d.is_ipv6());
    v.dedup();
    if v.is_empty() {
        return Err(ErrorNucleo::Sincro {
            detalle: format!("no se encuentra {h}"),
        });
    }
    Ok(v)
}

/// **Conecta con `host:puerto`**: cada direccion con su tope, IPv4 primero,
/// y ya con la espera de lectura y sin Nagle (los mensajes del protocolo son
/// pequenos y van y vienen).
pub fn conectar(host: &str, puerto: u16) -> Result<TcpStream, ErrorNucleo> {
    let mut ultimo = None;
    for d in direcciones(host, puerto)? {
        match TcpStream::connect_timeout(&d, ESPERA_AL_CONECTAR) {
            Ok(s) => {
                s.set_read_timeout(Some(ESPERA_AL_LEER))?;
                s.set_nodelay(true)?;
                return Ok(s);
            }
            Err(e) => ultimo = Some(e),
        }
    }
    Err(ultimo
        .map(ErrorNucleo::from)
        .unwrap_or_else(|| ErrorNucleo::Sincro {
            detalle: format!("no se llega a {host}"),
        }))
}

/// El azar de cada conexion. Sin azar del sistema no se habla: un nonce fijo
/// repetiria claves.
pub fn nonce() -> Result<[u8; pixpin_sincro::canal::NONCE], ErrorNucleo> {
    let mut n = [0u8; pixpin_sincro::canal::NONCE];
    getrandom::getrandom(&mut n).map_err(|e| ErrorNucleo::Sincro {
        detalle: format!("sin azar del sistema: {e}"),
    })?;
    Ok(n)
}

/// Escucha en `puerto` por IPv6 con IPv4 dentro (en macOS e iOS un socket
/// `::` acepta tambien IPv4, como `0.0.0.0`) y, si no hay IPv6, solo por
/// IPv4.
pub fn escuchar(puerto: u16) -> std::io::Result<std::net::TcpListener> {
    std::net::TcpListener::bind(("::", puerto))
        .or_else(|_| std::net::TcpListener::bind(("0.0.0.0", puerto)))
}

/// Se conecta a si mismo para sacar de su espera a un `accept` (lo unico que
/// lo despierta sin sondear). Por IPv4 y, si no llega, por IPv6.
pub fn despertar(puerto: u16) {
    let v4 = SocketAddr::from(([127, 0, 0, 1], puerto));
    if TcpStream::connect_timeout(&v4, Duration::from_millis(500)).is_err() {
        let v6 = SocketAddr::from((Ipv6Addr::LOCALHOST, puerto));
        let _ = TcpStream::connect_timeout(&v6, Duration::from_millis(500));
    }
}

/// `host` es una IPv4 escrita (lo unico que cabe en un QR de PixPin: el QR
/// se parte por `:` y una IPv6 lo romperia en Windows y Android).
pub fn es_ipv4(host: &str) -> bool {
    limpiar_host(host).parse::<std::net::Ipv4Addr>().is_ok()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_ip_escritas_no_pasan_por_el_dns_y_va_primero_ipv4() {
        let v = direcciones("192.168.1.20", 47474).unwrap();
        assert_eq!(v, vec!["192.168.1.20:47474".parse().unwrap()]);
        let v = direcciones("[::1]", 5).unwrap();
        assert_eq!(v, vec!["[::1]:5".parse().unwrap()]);
        let v = direcciones(" 2001:db8::1 ", 7).unwrap();
        assert!(v[0].is_ipv6());
        // Con el numero de la interfaz, sin preguntar a nadie.
        let v = direcciones("fe80::1%4", 9).unwrap();
        match v[0] {
            SocketAddr::V6(d) => {
                assert_eq!(d.scope_id(), 4);
                assert_eq!(d.port(), 9);
            }
            SocketAddr::V4(_) => panic!("es IPv6"),
        }
        // `localhost` puede dar las dos: la IPv4 delante.
        if let Ok(v) = direcciones("localhost", 1)
            && v.len() > 1
        {
            assert!(v[0].is_ipv4(), "{v:?}");
        }
    }

    #[test]
    fn caso_negativo_sin_host_no_se_conecta() {
        assert!(matches!(
            direcciones("  ", 1),
            Err(ErrorNucleo::Argumento { .. })
        ));
        assert!(matches!(
            direcciones("[]", 1),
            Err(ErrorNucleo::Argumento { .. })
        ));
    }

    #[test]
    fn solo_una_ipv4_cabe_en_un_qr() {
        assert!(es_ipv4("10.0.0.2"));
        assert!(!es_ipv4("fe80::1%en0"));
        assert!(!es_ipv4("pc.local"));
        assert_eq!(limpiar_host("[fe80::1%en0]"), "fe80::1%en0");
    }
}
