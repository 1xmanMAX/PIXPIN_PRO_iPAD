// Saltar a un mensaje dentro del chat (lo pide Buscar con «Abrir en
// Proyectos»): se carga SOLO la pagina que lo contiene, centrada en el, y
// el chat se desplaza hasta el y lo marca. Lo de antes y lo de despues se
// pide al llegar arriba o abajo, como siempre (§4bis.2 del plan).

import Foundation

/// Un mensaje al que saltar: su chat, su id (para marcarlo) y su posicion
/// en el chat (0 el mas antiguo, la de `Nucleo.mensajes`).
public struct SaltoAMensaje: Equatable, Sendable {
    public var chat: String
    public var mensaje: String?
    public var posicion: UInt64

    public init(chat: String, mensaje: String?, posicion: UInt64) {
        self.chat = chat
        self.mensaje = mensaje
        self.posicion = posicion
    }
}

/// La pagina que se carga para ensenar el mensaje de `posicion` en un chat
/// de `total` mensajes: `tam` mensajes con el en medio, sin salirse del
/// chat (cerca del principio o del final se corre hacia dentro). Un chat
/// vacio da una pagina vacia; una posicion de mas cuenta como la ultima.
public func ventanaDeSalto(posicion: UInt64, total: UInt64, tam: UInt64) -> (desde: UInt64, cuantos: UInt32) {
    guard total > 0 else { return (0, 0) }
    let tam = max(tam, 1)
    let p = min(posicion, total - 1)
    var desde = p >= tam / 2 ? p - tam / 2 : 0
    if desde + tam > total {
        desde = total > tam ? total - tam : 0
    }
    return (desde, UInt32(clamping: min(tam, total - desde)))
}
