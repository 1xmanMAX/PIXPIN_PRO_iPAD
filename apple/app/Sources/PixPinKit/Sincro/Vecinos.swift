// Los otros aparatos de PixPin en la Wi-Fi: Bonjour con `NWBrowser` de
// Network.framework (el `DnsServiceBrowse` de Windows y el `NsdManager` de
// Android) y la sonda `PING`/`PONG` que dice si contestan y donde.
//
// Rendimiento (§4bis.6 del plan): nada sondea. Bonjour avisa cuando alguien
// aparece o se va (`browseResultsChangedHandler`), y a cada aparato nuevo se
// le manda UNA sonda, que es tambien lo que da su direccion.

import Foundation
import Network
import PixPinCore

/// Un aparato de PixPin visto en la red (de cualquier grupo).
public struct Vecino: Identifiable, Equatable {
    /// El `id` del TXT; si no lo trae, la instancia.
    public var id: String
    /// `PixPin Portatil`, tal como se anuncia.
    public var instancia: String
    public var datos: DatosVecino
    /// Donde esta en Bonjour (se resuelve con `Vecinos.sondear`).
    public var punto: NWEndpoint

    public var nombre: String { datos.nombreParaVer(instancia: instancia) }
}

@MainActor
public final class Vecinos: ObservableObject {
    /// Todos los `_pixpin._tcp` de la red, uno por aparato.
    @Published public private(set) var todos: [Vecino] = []
    /// Se llama en el hilo principal cada vez que cambia la lista.
    public var alCambiar: (@MainActor ([Vecino]) -> Void)?

    private var buscador: NWBrowser?
    /// Quien espera a un aparato (`esperar`): su condicion y su aviso.
    private var esperas: [UUID: (cumple: (DatosVecino) -> Bool, aviso: CheckedContinuation<Vecino?, Never>)] = [:]

    public init() {}

    public var buscando: Bool { buscador != nil }

    /// Empieza a buscar `tipo` (`tipoDeServicio()`). La primera vez el
    /// sistema pide el permiso de red local (`NSLocalNetworkUsageDescription`).
    public func empezar(tipo: String = tipoDeServicio()) {
        guard buscador == nil else { return }
        let parametros = NWParameters()
        // Solo la Wi-Fi/Ethernet de verdad: AWDL (AirDrop) no lleva a un PC.
        parametros.includePeerToPeer = false
        let b = NWBrowser(for: .bonjourWithTXTRecord(type: tipo, domain: nil), using: parametros)
        b.browseResultsChangedHandler = { [weak self] resultados, _ in
            // El buscador corre en la cola principal (abajo).
            MainActor.assumeIsolated { self?.actualizar(resultados) }
        }
        b.stateUpdateHandler = { [weak self] estado in
            if case let .failed(error) = estado {
                NSLog("PixPin: la busqueda de Bonjour fallo: %@", "\(error)")
                MainActor.assumeIsolated {
                    // Sin rehacerlo aqui (podria girar en vacio): la
                    // siguiente `empezar` lo vuelve a montar.
                    self?.buscador?.cancel()
                    self?.buscador = nil
                }
            }
        }
        // En la principal: los avisos son pocos y pequenos, y asi la lista se
        // publica sin saltos de hilo.
        b.start(queue: .main)
        buscador = b
    }

    public func parar() {
        buscador?.cancel()
        buscador = nil
        todos = []
        for clave in Array(esperas.keys) { cumplir(clave, con: nil) }
    }

    /// Los de mi grupo: anuncian la `etiqueta` y no son `yo`.
    public func delGrupo(etiqueta: String, yo: String) -> [Vecino] {
        todos.filter { $0.datos.esDelGrupo(etiqueta: etiqueta, yo: yo) }
    }

    /// Espera, hasta `tope` segundos, a que aparezca uno que cumpla `cumple`
    /// (para unirse a un grupo con su codigo: el que anuncia esa etiqueta).
    public func esperar(tope: TimeInterval, _ cumple: @escaping (DatosVecino) -> Bool) async -> Vecino? {
        if let ya = todos.first(where: { cumple($0.datos) }) { return ya }
        empezar()
        let clave = UUID()
        return await withCheckedContinuation { (c: CheckedContinuation<Vecino?, Never>) in
            esperas[clave] = (cumple, c)
            // Un solo sueno con su tope, no un sondeo.
            Task { @MainActor [weak self] in
                try? await Task.sleep(for: .seconds(tope))
                self?.cumplir(clave, con: nil)
            }
        }
    }

    private func cumplir(_ clave: UUID, con vecino: Vecino?) {
        guard let espera = esperas.removeValue(forKey: clave) else { return }
        espera.aviso.resume(returning: vecino)
    }

    private func actualizar(_ resultados: Set<NWBrowser.Result>) {
        var vistos: [String: Vecino] = [:]
        for r in resultados {
            guard case let .service(nombre, _, _, _) = r.endpoint else { continue }
            var txt: [String: String] = [:]
            if case let .bonjour(registro) = r.metadata { txt = registro.dictionary }
            let datos = DatosVecino(txt: txt)
            let id = datos.id.isEmpty ? nombre : datos.id
            // El mismo aparato puede salir por dos interfaces (Wi-Fi y
            // cable): uno basta.
            if vistos[id] == nil {
                vistos[id] = Vecino(id: id, instancia: nombre, datos: datos, punto: r.endpoint)
            }
        }
        let lista = vistos.values.sorted { $0.nombre.localizedStandardCompare($1.nombre) == .orderedAscending }
        guard lista != todos else { return }
        todos = lista
        for (clave, espera) in esperas {
            if let v = lista.first(where: { espera.cumple($0.datos) }) { cumplir(clave, con: v) }
        }
        alCambiar?(lista)
    }

    // MARK: - La sonda

    /// **La sonda de Windows y Android** (`PING` → `PONG`) a un aparato
    /// visto en Bonjour. Si contesta, devuelve su direccion tal como la
    /// necesita Rust (`host`, `puerto`); si no, `nil`. Primero por IPv4 (lo
    /// que anuncian Windows y Android); si no llega, por lo que elija el
    /// sistema. No toca el hilo principal.
    public nonisolated static func sondear(_ punto: NWEndpoint, tope: TimeInterval = 5) async -> Direccion? {
        if let d = await sondearUna(punto, soloIPv4: true, tope: tope) { return d }
        return await sondearUna(punto, soloIPv4: false, tope: tope)
    }

    private nonisolated static func sondearUna(_ punto: NWEndpoint, soloIPv4: Bool,
                                               tope: TimeInterval) async -> Direccion? {
        await withCheckedContinuation { (c: CheckedContinuation<Direccion?, Never>) in
            let parametros = NWParameters.tcp
            if soloIPv4, let ip = parametros.defaultProtocolStack.internetProtocol as? NWProtocolIP.Options {
                ip.version = .v4
            }
            let conexion = NWConnection(to: punto, using: parametros)
            let cola = DispatchQueue(label: "pixpin.sincro.sonda", qos: .utility)
            let fin = UnaVez<Direccion?> { d in
                conexion.cancel()
                c.resume(returning: d)
            }
            conexion.stateUpdateHandler = { estado in
                switch estado {
                case .ready:
                    let direccion = conexion.currentPath?.remoteEndpoint.flatMap(Vecinos.direccion(de:))
                    conexion.send(content: Data("PING".utf8), completion: .contentProcessed { error in
                        if error != nil { fin.acabar(nil) }
                    })
                    conexion.receive(minimumIncompleteLength: 4, maximumLength: 4) { datos, _, _, error in
                        fin.acabar(error == nil && datos == Data("PONG".utf8) ? direccion : nil)
                    }
                case .failed, .cancelled:
                    fin.acabar(nil)
                case .waiting:
                    // Sin ruta o rechazada: NWConnection reintentaria solo;
                    // para una sonda, es que no contesta.
                    fin.acabar(nil)
                default:
                    break
                }
            }
            conexion.start(queue: cola)
            cola.asyncAfter(deadline: .now() + tope) { fin.acabar(nil) }
        }
    }

    /// La direccion de un punto ya resuelto (`hostPort`).
    nonisolated static func direccion(de punto: NWEndpoint) -> Direccion? {
        guard case let .hostPort(host, puerto) = punto, let texto = textoDeHost(host) else { return nil }
        return Direccion(host: texto, puerto: puerto.rawValue)
    }

    nonisolated static func textoDeHost(_ host: NWEndpoint.Host) -> String? {
        switch host {
        case let .ipv4(a):
            return textoIPv4(a.rawValue)
        case let .ipv6(a):
            if let v4 = a.asIPv4 { return textoIPv4(v4.rawValue) }
            return textoIPv6(a)
        case let .name(nombre, _):
            return nombre
        @unknown default:
            return nil
        }
    }

    /// `fe80::1%en0`: la de enlace local lleva su interfaz, que es lo que
    /// `getaddrinfo` (lo que usa Rust al conectar) necesita para llegar.
    nonisolated static func textoIPv6(_ a: IPv6Address) -> String? {
        let bytes = [UInt8](a.rawValue)
        guard bytes.count == 16 else { return nil }
        let largo = Int(INET6_ADDRSTRLEN)
        var texto = [CChar](repeating: 0, count: largo)
        let hecho = bytes.withUnsafeBytes { origen in
            inet_ntop(AF_INET6, origen.baseAddress, &texto, socklen_t(largo))
        }
        guard hecho != nil else { return nil }
        var s = String(cString: texto)
        if a.isLinkLocal, let interfaz = a.interface { s += "%\(interfaz.name)" }
        return s
    }
}

/// Algo que se termina una sola vez aunque lo pidan varios a la vez (la
/// respuesta, el error y el tope de la sonda).
final class UnaVez<T>: @unchecked Sendable {
    private let candado = NSLock()
    private var hecho = false
    private let alAcabar: (T) -> Void

    init(_ alAcabar: @escaping (T) -> Void) {
        self.alAcabar = alAcabar
    }

    func acabar(_ valor: T) {
        candado.lock()
        let primera = !hecho
        hecho = true
        candado.unlock()
        if primera { alAcabar(valor) }
    }
}
