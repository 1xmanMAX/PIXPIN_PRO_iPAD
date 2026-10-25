// HUECO: lo rellena la parte de la app que le toca (ver docs/plan-ipad.md).
// Mantener la firma publica (`init`) tal cual: la usa `Raiz/RaizIPad.swift`.

import PixPinCore
import SwiftUI

public struct EditorLienzoIPad: View {
    let almacen: Almacen
    let chat: String
    let mensaje: MensajeChat?
    let foto: URL?
    public init(almacen: Almacen, chat: String, mensaje: MensajeChat?, foto: URL?) {
        self.almacen = almacen; self.chat = chat; self.mensaje = mensaje; self.foto = foto
    }
    public var body: some View {
        VStack { BarraVisorIPad(tipo: "Lienzo", icono: "scribble.variable", titulo: mensaje?.nombre ?? "Lienzo nuevo"); Spacer() }
    }
}
