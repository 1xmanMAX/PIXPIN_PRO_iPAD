//! `pixpin-mcp.exe`: ver la cabecera de `lib.rs` y docs/integraciones-mcp.md.
//!
//! El bucle: una linea de stdin, una respuesta en stdout. Las llamadas a
//! herramientas van cada una en su hilo (una puede esperar minutos a que el
//! usuario conteste un permiso en PixPin) y escriben su respuesta cuando
//! acaban, de una pieza, con stdout cogido. Al cerrarse stdin (el cliente se
//! fue) se sale.
//!
//! Subsistema de consola a proposito: los clientes lo arrancan con las
//! tuberias redirigidas y sin ventana.

use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};

use pixpin_mcp::protocolo::{self, Salida};

fn escribir(salida: &Mutex<std::io::Stdout>, v: &serde_json::Value) {
    let mut s = salida.lock().unwrap_or_else(|e| e.into_inner());
    // Una linea por mensaje y nada mas en stdout (serde_json no mete saltos).
    let _ = writeln!(s, "{v}");
    let _ = s.flush();
}

fn main() {
    let salida = Arc::new(Mutex::new(std::io::stdout()));
    let entrada = std::io::stdin();
    let mut hilos = Vec::new();
    for linea in entrada.lock().lines() {
        let Ok(linea) = linea else { break };
        if linea.trim().is_empty() {
            continue;
        }
        match protocolo::atender(&linea) {
            Salida::Responder(v) => escribir(&salida, &v),
            Salida::Nada => {}
            Salida::Herramienta { id, nombre, argumentos } => {
                let salida = salida.clone();
                let h = std::thread::Builder::new().name(format!("herramienta {nombre}")).spawn(move || {
                    let r = match pixpin_mcp::herramientas::llamar(&nombre, &argumentos) {
                        Some(r) => protocolo::respuesta(id, r),
                        None => protocolo::error(id, protocolo::codigo::PARAMETROS, format!("Herramienta desconocida: {nombre}")),
                    };
                    escribir(&salida, &r);
                });
                match h {
                    Ok(h) => hilos.push(h),
                    Err(e) => eprintln!("pixpin-mcp: no se pudo lanzar un hilo: {e}"),
                }
                hilos.retain(|h| !h.is_finished());
            }
        }
    }
    // stdin cerrado: se contesta lo que este a medias antes de irse, pero sin
    // quedarse minutos colgado por un permiso que ya nadie va a leer.
    let hasta = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while hilos.iter().any(|h| !h.is_finished()) && std::time::Instant::now() < hasta {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    std::process::exit(0);
}
