// La puerta por la que los demas aparatos llaman a este Mac: la `Escucha` de
// Rust (un `TcpListener`) atendida por unos pocos hilos propios.
//
// Por que `Thread` y no `Task`: `Escucha.atender()` bloquea hasta que llega
// alguien (sin gastar nada: es un `accept`), y una `Task` bloqueada ocuparia
// uno de los pocos hilos del pool cooperativo de Swift todo el rato que la
// app vive. Un hilo dormido en `accept` no cuesta CPU ni energia.
//
// **Varios a la vez** (`hilosDeEscucha()`, 4): cada hilo espera en la misma
// `Escucha` y atiende la conexion que le toca, asi una sonda (`PING`) o un
// archivo suelto no esperan a que acabe una vuelta larga. Escribir en los
// chats sigue siendo de uno en uno: Rust deja una sola vuelta por carpeta
// (la segunda oye «ocupado», como en Windows).
//
// El puerto es el de siempre (47474, `puertoPreferido()`) o, si esta tomado,
// el que de el sistema, como Windows y Android: se anuncia el que salga.

import Foundation
import PixPinCore

public final class Puerta: @unchecked Sendable {
    public let puerto: UInt16
    private let escucha: Escucha

    /// Abre la escucha en 47474 o, si no se puede, en cualquiera.
    public init(nucleo: Nucleo) throws {
        let e: Escucha
        do {
            e = try nucleo.escuchar(puerto: puertoPreferido())
        } catch {
            e = try nucleo.escuchar(puerto: 0)
        }
        escucha = e
        puerto = e.puerto()
    }

    /// Arranca los hilos que atienden a quien llame (`hilos`, por omision
    /// `hilosDeEscucha()`). `alAtender` se llama en el hilo que atendio, con
    /// cada conexion atendida (o su error); los hilos terminan con
    /// `cerrar()`.
    public func arrancar(hilos: Int = Int(hilosDeEscucha()),
                         alAtender: @escaping @Sendable (Result<Atendido, Error>) -> Void) {
        let escucha = self.escucha
        // Lo que llama de fuera (no las sondas) hace girar el boton de
        // sincronizar de la lista (`GiroDeSincronizar.swift`).
        let aviso = AvisoDeLaPuerta()
        for numero in 0 ..< max(1, hilos) {
            let hilo = Thread {
                var fallosSeguidos = 0
                while true {
                    do {
                        let atendido = try escucha.atenderAvisando(aviso: aviso)
                        if atendido == .cerrada { return }
                        fallosSeguidos = 0
                        alAtender(.success(atendido))
                    } catch {
                        // Un aparato con otro codigo o una red que se corta
                        // es un fallo de esa conexion: se sigue escuchando.
                        // Si fallan muchas seguidas (sin descriptores...), se
                        // espera un poco para no girar en vacio.
                        fallosSeguidos += 1
                        alAtender(.failure(error))
                        if fallosSeguidos >= 3 {
                            Thread.sleep(forTimeInterval: min(30, Double(fallosSeguidos)))
                        }
                    }
                }
            }
            hilo.name = "pixpin-sincro-escucha-\(numero)"
            hilo.qualityOfService = .utility
            hilo.start()
        }
    }

    /// Cierra la puerta: todos los hilos salen de su espera y terminan.
    public func cerrar() {
        escucha.cerrar()
    }
}
