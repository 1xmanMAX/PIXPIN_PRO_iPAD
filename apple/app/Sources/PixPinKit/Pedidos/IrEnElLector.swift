// **Adonde llevar un fichero al abrirlo** (el `abrir {tipo: fichero,
// pagina, buscar, zona}` de Windows: `lector_pdf::abrir_en_la_pagina`,
// `abrir_buscando` y `plano_cad::abrir_en`). El pedido lo deja aqui antes
// de abrir el visor, y el visor lo recoge al abrir (`tomar`); si el fichero
// ya estaba abierto, el pedido no abre otro: avisa por `cambio` y el visor
// abierto lo recoge igual (15-oct, «ve a la pagina 12»).
//
// Es puro (sin vistas) para que lo usen el lector de documentos y el de
// planos sin depender de los pedidos.

import Foundation

public enum IrEnElLector {
    /// Lo que se pide para un fichero.
    public struct Peticion: Equatable, Sendable {
        /// La pagina, **desde 1** (la del pedido, como en Windows).
        public var pagina: Int?
        /// Un texto que ensenar: el buscador abierto con el y la primera
        /// coincidencia desde `pagina`.
        public var buscar: String?
        /// Donde mirar en un plano: el centro y lo que cabe a lo ancho, en
        /// coordenadas del dibujo.
        public var zona: (x: Double, y: Double, ancho: Double)?

        public init(pagina: Int? = nil, buscar: String? = nil, zona: (x: Double, y: Double, ancho: Double)? = nil) {
            self.pagina = pagina.flatMap { $0 >= 1 ? $0 : nil }
            let q = buscar?.trimmingCharacters(in: .whitespacesAndNewlines)
            self.buscar = (q?.isEmpty ?? true) ? nil : q
            self.zona = zona.flatMap { $0.ancho > 0 && $0.x.isFinite && $0.y.isFinite && $0.ancho.isFinite ? $0 : nil }
        }

        /// Si pide algo.
        public var algo: Bool { pagina != nil || buscar != nil || zona != nil }

        /// La pagina desde 0 (la de PDFKit), si la hay.
        public var indice: Int? { pagina.map { $0 - 1 } }

        public static func == (a: Peticion, b: Peticion) -> Bool {
            a.pagina == b.pagina && a.buscar == b.buscar
                && a.zona?.x == b.zona?.x && a.zona?.y == b.zona?.y && a.zona?.ancho == b.zona?.ancho
        }
    }

    /// El aviso de que hay una peticion nueva para un fichero ya abierto
    /// (`object`: su ruta, `URL.standardizedFileURL.path`).
    public static let cambio = Notification.Name("PixPinIrEnElLector")

    @MainActor private static var pendientes: [String: Peticion] = [:]

    static func clave(_ url: URL) -> String { url.standardizedFileURL.path }

    /// La deja para el visor de `url` (y avisa por si ya esta abierto).
    @MainActor
    public static func pedir(_ p: Peticion, para url: URL) {
        guard p.algo else { return }
        pendientes[clave(url)] = p
        NotificationCenter.default.post(name: cambio, object: clave(url))
    }

    /// **La recoge el visor** de `url` (una vez): al abrir y al llegarle
    /// `cambio` con su ruta.
    @MainActor
    public static func tomar(_ url: URL) -> Peticion? {
        pendientes.removeValue(forKey: clave(url))
    }

    /// Si `cambio` es para `url`.
    public static func esPara(_ aviso: Notification, _ url: URL) -> Bool {
        (aviso.object as? String) == clave(url)
    }

    /// `zona` del pedido: `[x, y, ancho]`.
    static func zona(de v: ValorPedido?) -> (x: Double, y: Double, ancho: Double)? {
        guard let l = v?.lista, l.count == 3 else { return nil }
        var n: [Double] = []
        for e in l {
            guard case let .numero(d) = e else { return nil }
            n.append(d)
        }
        return (n[0], n[1], n[2])
    }

    /// La peticion de un `que` de `abrir`.
    static func peticion(de que: [String: ValorPedido]) -> Peticion {
        var pagina: Int?
        if case let .numero(d)? = que["pagina"], d.isFinite, d >= 1, d < 1e7 { pagina = Int(d) }
        return Peticion(pagina: pagina, buscar: que["buscar"]?.texto, zona: zona(de: que["zona"]))
    }
}
