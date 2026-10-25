// Encontrar en la Wi-Fi a quien tiene un codigo (un envio de seis cifras o
// una invitacion a un chat compartido) sin que el codigo se publique: se
// busca en Bonjour el tipo (`_pixpinenvio._tcp`, `_pixpinrecibe._tcp`,
// `_pixpinchat._tcp`) y se elige el que anuncia la etiqueta de ese codigo
// en su TXT `g`, como `buscar_y_conectar` de Windows.
//
// Esas puertas no contestan la sonda `PING` (no son la escucha de
// siempre): la direccion se saca abriendo una conexion y cerrandola sin
// decir nada, que Rust no cuenta como llamada (`envio::aceptar`).

import Foundation
import Network
import PixPinCore

/// Busca durante `tope` segundos un servicio `tipo` que anuncie `etiqueta`
/// y devuelve su direccion (`host`, `puerto`), o `nil`.
@MainActor
public func buscarPuerta(tipo: String, etiqueta: String, tope: TimeInterval = 20) async -> Direccion? {
    let buscador = Vecinos()
    buscador.empezar(tipo: tipo)
    defer { buscador.parar() }
    guard let v = await buscador.esperar(tope: tope, { $0.etiqueta == etiqueta }) else { return nil }
    return await resolverPunto(v.punto)
}

/// La direccion de un servicio de Bonjour: se conecta (IPv4 primero) y se
/// mira a donde llego, y se cuelga en el acto.
public func resolverPunto(_ punto: NWEndpoint, tope: TimeInterval = 5) async -> Direccion? {
    if let d = await resolverUna(punto, soloIPv4: true, tope: tope) { return d }
    return await resolverUna(punto, soloIPv4: false, tope: tope)
}

private func resolverUna(_ punto: NWEndpoint, soloIPv4: Bool, tope: TimeInterval) async -> Direccion? {
    await withCheckedContinuation { (c: CheckedContinuation<Direccion?, Never>) in
        let parametros = NWParameters.tcp
        if soloIPv4, let ip = parametros.defaultProtocolStack.internetProtocol as? NWProtocolIP.Options {
            ip.version = .v4
        }
        let conexion = NWConnection(to: punto, using: parametros)
        let cola = DispatchQueue(label: "pixpin.sincro.resolver", qos: .utility)
        let fin = UnaVez<Direccion?> { d in
            conexion.cancel()
            c.resume(returning: d)
        }
        conexion.stateUpdateHandler = { estado in
            switch estado {
            case .ready:
                fin.acabar(conexion.currentPath?.remoteEndpoint.flatMap(Vecinos.direccion(de:)))
            case .failed, .cancelled, .waiting:
                fin.acabar(nil)
            default:
                break
            }
        }
        conexion.start(queue: cola)
        cola.asyncAfter(deadline: .now() + tope) { fin.acabar(nil) }
    }
}

/// Anuncia una puerta (envio, recepcion o invitacion) mientras vive: el
/// anuncio cuesta PBKDF2 (la etiqueta del codigo) y se calcula fuera del
/// hilo principal.
public final class AnuncioDePuerta: @unchecked Sendable {
    private let anunciador = Anunciador()

    public init() {}

    public func anunciar(_ hacer: @escaping @Sendable () -> AnuncioBonjour, puerto: UInt16) {
        let anunciador = self.anunciador
        Task.detached(priority: .utility) {
            anunciador.anunciar(hacer(), puerto: puerto)
        }
    }

    public func retirar() {
        anunciador.retirar()
    }

    deinit {
        anunciador.retirar()
    }
}

/// Atiende una puerta de Rust (`atender()` bloquea en `accept`) en un hilo
/// propio hasta que devuelva `fin`, como la `Puerta` de la escucha.
public func atenderEnHilo<T: Sendable>(nombre: String, _ atender: @escaping @Sendable () throws -> T,
                                       esFin: @escaping @Sendable (T) -> Bool,
                                       alAtender: @escaping @Sendable (Result<T, Error>) -> Void) {
    let hilo = Thread {
        var fallos = 0
        while true {
            do {
                let r = try atender()
                fallos = 0
                alAtender(.success(r))
                if esFin(r) { return }
            } catch {
                fallos += 1
                alAtender(.failure(error))
                if fallos >= 5 { return }
                Thread.sleep(forTimeInterval: Double(fallos))
            }
        }
    }
    hilo.name = nombre
    hilo.qualityOfService = .utility
    hilo.start()
}
