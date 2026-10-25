// Las entradas del menu de un mensaje que llegaron a Windows del 24 al 26 de
// octubre (`ventana_chat.rs`, `menu_de_mensaje`): **Guardar en mis
// figuras** (un lienzo), **Abrir la carpeta** / **Añadir a carpeta…** y
// **Enviar a Anki**. Van aparte para que `MenuMensaje.swift` solo las
// ponga en su sitio.

import PixPinCore
import SwiftUI

/// Las entradas de estudio de un mensaje. Con `enMas`, las que Windows
/// deja en «Más» (añadir a una carpeta lo que no es una hoja); sin el, las
/// de a la vista.
struct EntradasDeEstudio: View {
    let m: MensajeChat
    @ObservedObject var c: ChatAbierto
    var enMas = false

    var body: some View {
        if enMas {
            if CarpetasChat.seOfrece(m), !CarpetasChat.vaALaVista(m) {
                menuDeAnadir
            }
        } else {
            // Un lienzo (lo que hizo el asistente, o uno propio), a la
            // biblioteca de figuras de un toque (25-oct-2026).
            if m.clase == "DIBUJO", m.referencia != nil {
                Button { FigurasChat.guardar(lienzo: m, c: c) } label: {
                    Label("Guardar en mis figuras", systemImage: "square.on.square")
                }
            }
            if CarpetasChat.esCarpeta(m) {
                Button { VistaCarpeta.abrir(chat: c.chat, carpeta: m.id, almacen: c.almacen) } label: {
                    Label("Abrir la carpeta", systemImage: "book")
                }
            } else if CarpetasChat.seOfrece(m), CarpetasChat.vaALaVista(m) {
                menuDeAnadir
            }
            // Tarjetas de estudio: de una clase grabada, una foto o una nota.
            if AnkiChat.sePuede(m, c: c) {
                Button { AnkiChat.enviar(m, c: c) } label: {
                    Label("Enviar a Anki", systemImage: "rectangle.on.rectangle.angled")
                }
            }
        }
    }

    /// «Añadir a carpeta…»: las carpetas del chat (la que ya la tiene,
    /// marcada) y una nueva con esta pagina.
    private var menuDeAnadir: some View {
        Menu {
            ForEach(CarpetasChat.delChat(c.cargados), id: \.id) { carpeta in
                let ya = CarpetasChat.paginas(carpeta).contains(m.id)
                Button {
                    AccionesCarpeta.anadir(m, a: carpeta, en: c.chat, almacen: c.almacen, avisar: c.avisar)
                } label: {
                    Label(carpeta.nombre.isEmpty ? CarpetasChat.nombreDeSerie : carpeta.nombre,
                          systemImage: ya ? "checkmark.square" : "book")
                }
            }
            Divider()
            Button {
                let (chat, almacen, m, c) = (c.chat, c.almacen, m, c)
                _ = Task<Void, Never> { @MainActor in
                    guard let carpeta = await AccionesCarpeta.crear(en: chat, con: m, almacen: almacen) else { return }
                    c.escritura = .renombrar(carpeta)
                }
            } label: {
                Label("Nueva carpeta con esta página", systemImage: "plus")
            }
        } label: {
            Label("Añadir a carpeta…", systemImage: "book")
        }
    }
}
