// HUECO: lo rellena la zona del lienzo (ver docs/plan-ipad.md).
// Mantener la firma publica: la usa el chat para la burbuja de un `DIBUJO`.

import PixPinCore
import SwiftUI

/// La miniatura de un lienzo del chat (burbuja `DIBUJO`, `referencia` = su fichero).
public struct MiniaturaLienzo: View {
    let almacen: Almacen
    let chat: String
    let mensaje: MensajeChat
    public init(almacen: Almacen, chat: String, mensaje: MensajeChat) {
        self.almacen = almacen; self.chat = chat; self.mensaje = mensaje
    }
    public var body: some View {
        RoundedRectangle(cornerRadius: 8).fill(.quaternary).frame(width: 220, height: 150)
            .overlay(Image(systemName: "scribble.variable").font(.largeTitle).foregroundStyle(.secondary))
    }
}
