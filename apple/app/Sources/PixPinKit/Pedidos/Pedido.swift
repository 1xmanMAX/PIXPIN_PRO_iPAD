// Un pedido del protocolo v1 de Windows (`docs/protocolo-pedidos.md`):
// `{"pixpin": 1, "accion": "<accion>", ...}`. En el Mac no hay
// `WM_COPYDATA`: los pedidos llegan por los enlaces `pixpinmax://`, el
// menu del Dock, los Servicios del Finder y las App Intents, y todos pasan
// por aqui (`Enlaces`, `Verbos`, `HacerPedidos`).
//
// Es puro (texto entra, pedidos salen) para probarlo sin ventanas.

import Foundation

/// Un valor JSON de un pedido.
public enum ValorPedido: Equatable, Sendable {
    case nulo
    case booleano(Bool)
    case numero(Double)
    case texto(String)
    case lista([ValorPedido])
    case objeto([String: ValorPedido])

    /// `.texto` o `.nulo` (los campos opcionales de las filas).
    public static func t(_ s: String?) -> ValorPedido {
        s.map(ValorPedido.texto) ?? .nulo
    }

    /// Una lista de textos (rutas).
    public static func textos(_ v: [String]) -> ValorPedido {
        .lista(v.map(ValorPedido.texto))
    }

    public var texto: String? {
        if case let .texto(s) = self { return s }
        return nil
    }

    public var entero: Int? {
        if case let .numero(n) = self, n.rounded() == n, abs(n) < 1e15 { return Int(n) }
        return nil
    }

    public var booleano: Bool? {
        if case let .booleano(b) = self { return b }
        return nil
    }

    public var lista: [ValorPedido]? {
        if case let .lista(l) = self { return l }
        return nil
    }

    public var objeto: [String: ValorPedido]? {
        if case let .objeto(o) = self { return o }
        return nil
    }

    public var esNulo: Bool { self == .nulo }

    /// Lo que se entiende por vacio en `seguridad`: nulo, un texto en
    /// blanco o una lista sin nada.
    var vacio: Bool {
        switch self {
        case .nulo: return true
        case let .texto(s): return s.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        case let .lista(l): return l.isEmpty
        default: return false
        }
    }

    // MARK: JSON

    /// De lo que da `JSONSerialization`.
    init(deAny a: Any) {
        switch a {
        case let n as NSNumber:
            if CFGetTypeID(n) == CFBooleanGetTypeID() {
                self = .booleano(n.boolValue)
            } else {
                self = .numero(n.doubleValue)
            }
        case let s as String: self = .texto(s)
        case let l as [Any]: self = .lista(l.map(ValorPedido.init(deAny:)))
        case let o as [String: Any]: self = .objeto(o.mapValues(ValorPedido.init(deAny:)))
        default: self = .nulo
        }
    }

    /// Para `JSONSerialization`.
    var comoAny: Any {
        switch self {
        case .nulo: return NSNull()
        case let .booleano(b): return b
        case let .numero(n):
            if let e = entero { return e }
            return n
        case let .texto(s): return s
        case let .lista(l): return l.map(\.comoAny)
        case let .objeto(o): return o.mapValues(\.comoAny)
        }
    }

    /// En una linea, con las claves en orden (lo que se ensena al confirmar).
    var json: String {
        guard let d = try? JSONSerialization.data(withJSONObject: comoAny,
                                                  options: [.sortedKeys, .withoutEscapingSlashes, .fragmentsAllowed]),
              let s = String(data: d, encoding: .utf8) else { return "null" }
        return s
    }
}

/// **Un pedido** ya leido: su accion y sus campos (sin `pixpin` ni
/// `accion`).
public struct Pedido: Equatable, Sendable, CustomStringConvertible {
    /// La version del protocolo que entiende el Mac.
    public static let version = 1
    /// Lo mas largo que se acepta (el `TOPE_PEDIDO` de Windows).
    public static let tope = 64 * 1024

    /// Las acciones de la version 1, las de `mensajero::ACCIONES` de
    /// Windows. Un pedido con otra se rechaza en el acto (3); las que el Mac
    /// aun no sabe hacer las dice `HacerPedidos` en su aviso.
    public static let acciones: [String] = [
        "abrir", "chat", "nota_nueva", "lienzo_nuevo", "grabar", "lista_nueva", "anadir_tarea",
        "marcar_tarea", "ventana_principal", "pinear", "iconos", "soltar", "reproducir",
        "leccion_nueva", "lecciones", "mover_tarea", "ventana", "capturar", "pinear_ultima",
        "conservar_captura", "borrar_captura", "copiar_imagen", "quitar_tarea", "corregir_tarea",
        "borrar_lista", "enviar_al_movil", "sincronizar", "asistente", "automatizacion",
        "tabla_nueva", "posponer", "pines", "timeline", "leer_en_voz", "ocr", "herramienta",
    ]

    public var accion: String
    public var campos: [String: ValorPedido]

    /// Como `orden::pedido` de Windows: los campos nulos no van, salvo
    /// `proyecto` (nulo = «Mensajes guardados»).
    public init(_ accion: String, _ campos: [String: ValorPedido] = [:]) {
        self.accion = accion
        self.campos = campos.filter { !$0.value.esNulo || $0.key == "proyecto" }
    }

    /// Tal cual, sin quitar nada (lo que llega en JSON).
    init(accion: String, crudos: [String: ValorPedido]) {
        self.accion = accion
        campos = crudos
    }

    public subscript(_ campo: String) -> ValorPedido? {
        get { campos[campo] }
        set { campos[campo] = newValue }
    }

    /// Un campo de texto, recortado; `nil` si falta o esta en blanco.
    public func texto(_ campo: String) -> String? {
        guard let t = campos[campo]?.texto?.trimmingCharacters(in: .whitespacesAndNewlines),
              !t.isEmpty else { return nil }
        return t
    }

    /// El pedido entero como objeto JSON.
    public var valor: ValorPedido {
        var o = campos
        o["pixpin"] = .numero(Double(Pedido.version))
        o["accion"] = .texto(accion)
        return .objeto(o)
    }

    /// En una linea, con las claves en orden.
    public var json: String { valor.json }

    public var description: String { json }

    /// **Lee un pedido escrito a mano** (`pedido?json=…`) y lo mira como la
    /// ventana de Windows (`validar_pedido`): un objeto con `"pixpin": 1` y
    /// una accion conocida. Otra version sale con 2; lo demas, con 3.
    public static func de(json: String) throws -> Pedido {
        if json.utf8.count > tope {
            throw FalloOrden.noSeEntiende("el pedido pasa de 64 KB")
        }
        let roto = FalloOrden.noSeEntiende("el pedido no es JSON del protocolo, o su accion no existe (docs/protocolo-pedidos.md)")
        guard let datos = json.data(using: .utf8),
              let a = try? JSONSerialization.jsonObject(with: datos),
              case let .objeto(o) = ValorPedido(deAny: a) else { throw roto }
        // Sin `pixpin` (o no numero) no es un pedido nuestro; otro numero,
        // otra version.
        guard case let .numero(v)? = o["pixpin"], v >= 0, v.rounded() == v else { throw roto }
        guard Int(v) == version else {
            throw FalloOrden(codigo: FalloOrden.Codigo.version, motivo: "esa version del protocolo no existe")
        }
        guard let accion = o["accion"]?.texto, acciones.contains(accion) else { throw roto }
        var campos = o
        campos["pixpin"] = nil
        campos["accion"] = nil
        return Pedido(accion: accion, crudos: campos)
    }
}

/// Por que no se pudo entender (o hacer) una orden, con el codigo de salida
/// de Windows (`orden::codigo`).
public struct FalloOrden: Error, Equatable, Sendable, LocalizedError {
    /// Los codigos de salida (los de `pixpin-lanzador pedido` de Windows).
    public enum Codigo {
        /// Aceptado (o la orden local salio bien).
        public static let aceptado: Int32 = 0
        /// Fallo aqui mismo: un fichero que no esta.
        public static let fallo: Int32 = 1
        /// Esa version del protocolo no existe.
        public static let version: Int32 = 2
        /// La orden, el enlace o el JSON no se entienden.
        public static let noSeEntiende: Int32 = 3
        /// No se hizo a proposito: un enlace que no puede pedir eso, o el
        /// usuario dijo que no al confirmar.
        public static let rechazado: Int32 = 6
    }

    public var codigo: Int32
    public var motivo: String

    public init(codigo: Int32, motivo: String) {
        self.codigo = codigo
        self.motivo = motivo
    }

    static func noSeEntiende(_ motivo: String) -> FalloOrden {
        FalloOrden(codigo: Codigo.noSeEntiende, motivo: motivo)
    }

    static func fallo(_ motivo: String) -> FalloOrden {
        FalloOrden(codigo: Codigo.fallo, motivo: motivo)
    }

    public var errorDescription: String? { motivo }
}
