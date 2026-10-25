// HUECO: lo rellena la parte de la app que le toca (ver docs/plan-ipad.md).
// Mantener la firma publica (`init`) tal cual: la usa `Raiz/RaizIPad.swift`.

import PixPinCore
import SwiftUI

public struct VisorDocumentoIPad: View {
    let url: URL
    let titulo: String
    let almacen: Almacen
    let chat: String?
    let mensaje: MensajeChat?
    public init(url: URL, titulo: String, almacen: Almacen, chat: String?, mensaje: MensajeChat?) {
        self.url = url; self.titulo = titulo; self.almacen = almacen; self.chat = chat; self.mensaje = mensaje
    }
    public var body: some View { VistaRapida(url: url) }
}

public struct EditorNotaIPad: View {
    let almacen: Almacen
    let chat: String
    let mensaje: MensajeChat
    public init(almacen: Almacen, chat: String, mensaje: MensajeChat) { self.almacen = almacen; self.chat = chat; self.mensaje = mensaje }
    public var body: some View {
        VStack { BarraVisorIPad(tipo: "Nota", icono: "note.text", titulo: mensaje.nombre); Text(mensaje.texto); Spacer() }
    }
}

public struct HojaCalculoIPad: View {
    let almacen: Almacen
    let chat: String
    let mensaje: MensajeChat
    public init(almacen: Almacen, chat: String, mensaje: MensajeChat) { self.almacen = almacen; self.chat = chat; self.mensaje = mensaje }
    public var body: some View {
        VStack { BarraVisorIPad(tipo: "Hoja", icono: "tablecells", titulo: mensaje.nombre); Spacer() }
    }
}
