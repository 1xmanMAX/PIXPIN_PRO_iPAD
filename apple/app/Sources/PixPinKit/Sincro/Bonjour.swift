// Lo puro de sincronizar por la Wi-Fi: leer y escribir el TXT de Bonjour,
// decidir quien es del grupo, la espera de «quieto» y cuando toca una vuelta
// sola. Sin red ni hilos: se prueba entero en `SincroTests`.
//
// Es lo mismo que hace Windows (`sincronizar.rs`: `es_del_grupo`,
// `anunciarse`; `sincronizar/automatica.rs`: `QUIETO`) y Android
// (`Presencia`): el TXT lleva `g` (la etiqueta del grupo, no su codigo),
// `id`, `l` (letra) y `n` (nombre).

import Foundation
import PixPinCore

/// Lo que anuncia en su TXT un aparato de PixPin.
public struct DatosVecino: Equatable, Sendable {
    /// La etiqueta de su grupo (`g`), o `nil` si no anuncia ninguna.
    public var etiqueta: String?
    public var id: String
    public var letra: String
    public var nombre: String

    /// Lee el TXT tal como lo da `NWTXTRecord.dictionary`. En DNS-SD las
    /// claves no distinguen mayusculas (RFC 6763 §6.4): se comparan en
    /// minusculas, como `mdns.rs` de Windows.
    public init(txt: [String: String]) {
        var d: [String: String] = [:]
        for (clave, valor) in txt { d[clave.lowercased()] = valor }
        etiqueta = d["g"].flatMap { $0.isEmpty ? nil : $0 }
        id = d["id"] ?? ""
        letra = d["l"] ?? ""
        nombre = d["n"] ?? ""
    }

    public init(etiqueta: String?, id: String, letra: String, nombre: String) {
        self.etiqueta = etiqueta
        self.id = id
        self.letra = letra
        self.nombre = nombre
    }

    /// `es_del_grupo` de Windows: anuncia la etiqueta de mi grupo y no soy yo
    /// (el anuncio propio tambien aparece al buscar).
    public func esDelGrupo(etiqueta mia: String, yo: String) -> Bool {
        etiqueta == mia && id != yo
    }

    /// El nombre para ensenar: el `n` del TXT o, si no lo trae, la instancia
    /// sin el «PixPin » del principio.
    public func nombreParaVer(instancia: String) -> String {
        if !nombre.isEmpty { return nombre }
        let prefijo = "PixPin "
        return instancia.hasPrefix(prefijo) ? String(instancia.dropFirst(prefijo.count)) : instancia
    }
}

/// El TXT en bytes (RFC 6763 §6): cada `clave=valor` precedido de su largo
/// en un byte, en el orden dado. Una entrada de mas de 255 bytes no cabe y
/// se deja fuera (con los valores de PixPin no pasa: `n` va recortado a 40
/// letras).
public func codificarTXT(_ pares: [(String, String)]) -> Data {
    var datos = Data()
    for (clave, valor) in pares {
        let entrada = Data("\(clave)=\(valor)".utf8)
        guard !clave.isEmpty, entrada.count <= 255 else { continue }
        datos.append(UInt8(entrada.count))
        datos.append(entrada)
    }
    // Un TXT vacio es un byte cero, no cero bytes (RFC 6763 §6.1).
    if datos.isEmpty { datos.append(0) }
    return datos
}

/// Lo contrario de `codificarTXT` (para las pruebas y para leer un TXT
/// crudo). Las claves, en minusculas; una entrada sin `=` es una clave sin
/// valor.
public func leerTXT(_ datos: Data) -> [String: String] {
    var salida: [String: String] = [:]
    let bytes = [UInt8](datos)
    var i = 0
    while i < bytes.count {
        let largo = Int(bytes[i])
        i += 1
        guard largo > 0, i + largo <= bytes.count else { break }
        let entrada = String(decoding: bytes[i ..< i + largo], as: UTF8.self)
        i += largo
        if let igual = entrada.firstIndex(of: "=") {
            salida[entrada[..<igual].lowercased()] = String(entrada[entrada.index(after: igual)...])
        } else {
            salida[entrada.lowercased()] = ""
        }
    }
    return salida
}

/// Donde escucha otro aparato, ya resuelto: lo que pide
/// `Nucleo.sincronizarCon(host:puerto:)`.
public struct Direccion: Equatable, Hashable, Sendable {
    public var host: String
    public var puerto: UInt16

    public init(host: String, puerto: UInt16) {
        self.host = host
        self.puerto = puerto
    }

    /// `partirDireccion` del movil: `192.168.1.20:47474`, o sin puerto y va
    /// el de siempre. Las IPv6 van entre corchetes (`[fe80::1%en0]:47474`).
    public init?(texto: String) {
        let t = texto.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty else { return nil }
        if t.hasPrefix("[") {
            guard let cierre = t.firstIndex(of: "]") else { return nil }
            let host = String(t[t.index(after: t.startIndex) ..< cierre])
            let resto = t[t.index(after: cierre)...]
            if resto.isEmpty {
                self.init(host: host, puerto: puertoPreferido())
            } else {
                guard resto.hasPrefix(":"), let p = UInt16(resto.dropFirst()), p > 0 else { return nil }
                self.init(host: host, puerto: p)
            }
            return
        }
        let partes = t.split(separator: ":", omittingEmptySubsequences: false)
        switch partes.count {
        case 1:
            self.init(host: t, puerto: puertoPreferido())
        case 2:
            guard !partes[0].isEmpty, let p = UInt16(partes[1]), p > 0 else { return nil }
            self.init(host: String(partes[0]), puerto: p)
        default:
            // Una IPv6 sin corchetes: toda es el host.
            self.init(host: t, puerto: puertoPreferido())
        }
    }
}

/// Una IPv4 en texto a partir de sus cuatro bytes (`IPv4Address.rawValue`).
/// A mano y no con `description`, que puede llevar la interfaz detras.
public func textoIPv4(_ bytes: Data) -> String? {
    guard bytes.count == 4 else { return nil }
    return bytes.map { String($0) }.joined(separator: ".")
}

// MARK: - Cuando sincronizar solo

/// Por que se sincroniza.
public enum MotivoSincro: Equatable, Sendable {
    /// El boton «Sincronizar ahora».
    case manual
    /// Aparecio en la red un aparato del grupo.
    case aparecio
    /// Los cambios de aqui llevan 30 s quietos (`QUIETO` de Windows; la
    /// cuenta la lleva `RelojSincro` del nucleo).
    case quieto
    /// Pasaron 10 min desde la ultima vuelta sola (`PERIODO` de Windows).
    case periodo
    /// Se tacho una tarea: sale al instante, solo ese chat.
    case tareas
}

/// Las reglas de la sincronizacion sola (§4bis.6 del plan): lo manual
/// siempre; lo automatico solo si esta encendido, **sin modo de bajo
/// consumo** (`ProcessInfo.isLowPowerModeEnabled`) y con la presencia
/// encendida (`[sincro] presencia`: Windows no arranca la automatica sin
/// ella).
public func sePuedeSincronizar(_ motivo: MotivoSincro, automatica: Bool, bajoConsumo: Bool,
                               presencia: Bool = true) -> Bool {
    switch motivo {
    case .manual: true
    case .aparecio, .quieto, .periodo, .tareas: automatica && !bajoConsumo && presencia
    }
}

/// Lo que se espera, como poco, entre dos vueltas solas con el mismo
/// aparato por aparecer: un anuncio de Bonjour que va y viene (Wi-Fi que se
/// duerme, cambio de interfaz) no debe lanzar una vuelta cada vez.
public let entreVueltasPorAparecer: TimeInterval = 120

/// Si un aparato que acaba de aparecer merece una vuelta sola: si no se
/// sincronizo con el hace poco.
public func tocaPorAparecer(ultimaVuelta: Date?, ahora: Date,
                            minimo: TimeInterval = entreVueltasPorAparecer) -> Bool {
    guard let ultimaVuelta else { return true }
    return ahora.timeIntervalSince(ultimaVuelta) >= minimo
}

/// El texto de un error del nucleo para ensenarlo (sin el nombre del caso).
public func textoDeError(_ error: Error) -> String {
    if let e = error as? ErrorNucleo {
        switch e {
        case let .Disco(detalle): return "Disco: \(detalle)"
        case let .NoExiste(que): return "No existe: \(que)"
        case let .Lienzo(detalle): return "Lienzo: \(detalle)"
        case let .Sincro(detalle): return detalle
        case let .Argumento(detalle): return detalle
        case let .Ajustes(detalle): return "Ajustes: \(detalle)"
        case let .Asistente(detalle): return "Asistente: \(detalle)"
        }
    }
    return error.localizedDescription
}
