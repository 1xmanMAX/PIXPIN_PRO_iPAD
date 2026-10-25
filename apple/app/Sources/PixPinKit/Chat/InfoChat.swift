// **La ficha del chat** (`ventana_chat/ficha_del_chat.rs` de Windows,
// 25-oct-2026, el usuario: «Mira Telegram: una isla flotante que muestra el
// chat, fotos, videos, archivos, enlaces…»; del Mac, `Chat/FichaDelChat.swift`):
// la hoja que sale al pulsar «Información» en la cabecera del chat.
//
// - **Arriba**, cuantas cosas tiene el chat con su codigo unico y la lupa.
// - **Las pestanas**, con la cuenta de cada una. Solo salen las que tienen
//   algo: un chat sin videos no ensena «Vídeos 0».
// - **El contenido**: lo que se mira (fotos, videos, dibujos, hojas) en una
//   rejilla de miniaturas cuadradas; lo que se lee (archivos, enlaces, voz)
//   en filas. Un enlace se abre en Safari; lo demas, «Ver en el chat».
//
// Que pestanas salen, que cuenta cada una y que hay en ella es de Rust
// (`ficha_del_chat.rs`: `Nucleo.fichaDelChat`, `Nucleo.cosasDeLaFicha`), el
// mismo reparto que Windows, el Mac y el movil.

import PixPinCore
import SwiftUI
import UIKit

/// Los textos de la ficha (`info-*` de `main.ftl`), los del Mac.
enum TextosFicha {
    static func rotulo(_ s: SeccionFicha) -> String {
        switch s {
        case .galeria: "Galería"
        case .todo: "Todo"
        case .fotos: "Fotos"
        case .videos: "Vídeos"
        case .archivos: "Archivos"
        case .enlaces: "Enlaces"
        case .voz: "Voz"
        case .dibujos: "Dibujos"
        case .fijados: "Fijados"
        case .buzon: "Buzón"
        }
    }

    /// El icono de una seccion (para el vacio).
    static func icono(_ s: SeccionFicha) -> String {
        switch s {
        case .fotos, .galeria: "photo.on.rectangle"
        case .videos: "play.fill"
        case .enlaces: "link"
        case .voz: "mic"
        case .dibujos: "scribble.variable"
        case .fijados: "pin"
        default: "doc.text"
        }
    }

    /// «24 cosas  ·  K7Q2» (`info-resumen`).
    static func resumen(cosas: UInt32, codigo: String) -> String {
        let n = cosas == 1 ? "1 cosa" : "\(cosas) cosas"
        return codigo.isEmpty ? n : "\(n)  ·  \(codigo)"
    }

    static let vacia = "Aquí no hay nada todavía"
    static let nada = "Nada con eso"
    static let buscar = "Buscar"
    static let verEnElChat = "Ver en el chat"
}

/// Lo de la ficha abierta, leido por el nucleo.
@MainActor
final class ModeloFichaDelChat: ObservableObject {
    let chat: String
    let almacen: Almacen
    @Published private(set) var ficha: FichaDelChatLeida?
    @Published var seccion: SeccionFicha? { didSet { if seccion != oldValue { leerCosas() } } }
    @Published var buscado = "" { didSet { if buscado != oldValue { leerCosas() } } }
    @Published private(set) var cosas: [CosaDeLaFicha]?
    private var turno = 0

    init(chat: String, almacen: Almacen) {
        self.chat = chat
        self.almacen = almacen
    }

    var pestana: PestanaFicha? { ficha?.pestanas.first { $0.seccion == seccion } }

    func releer() {
        let (nucleo, chat, actual) = (almacen.nucleo, chat, seccion)
        _ = Task<Void, Never> {
            let f = await Task.detached(priority: .userInitiated) {
                try? nucleo.fichaDelChat(chat: chat, actual: actual)
            }.value
            guard let f else { return }
            ficha = f
            if seccion == nil || !f.pestanas.contains(where: { $0.seccion == seccion }) {
                seccion = f.primera
            } else {
                leerCosas()
            }
        }
    }

    private func leerCosas() {
        guard let s = seccion else { return }
        turno += 1
        let mio = turno
        let (nucleo, chat, q) = (almacen.nucleo, chat, buscado)
        _ = Task<Void, Never> {
            let v = await Task.detached(priority: .userInitiated) {
                (try? nucleo.cosasDeLaFicha(chat: chat, seccion: s, buscado: q)) ?? []
            }.value
            guard mio == turno else { return }
            // Lo mas nuevo arriba, como la ficha de Windows.
            cosas = v.reversed()
        }
    }
}

struct VistaInfoChat: View {
    let almacen: Almacen
    let chat: String
    let titulo: String
    @Environment(\.dismiss) private var cerrar
    @StateObject private var modelo: ModeloFichaDelChat

    init(almacen: Almacen, chat: String, titulo: String) {
        self.almacen = almacen
        self.chat = chat
        self.titulo = titulo
        _modelo = StateObject(wrappedValue: ModeloFichaDelChat(chat: chat, almacen: almacen))
    }

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                if let f = modelo.ficha {
                    Text(TextosFicha.resumen(cosas: f.cosas, codigo: f.codigo))
                        .font(.caption).foregroundStyle(.secondary)
                        .padding(.top, 6)
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack(spacing: 6) {
                            ForEach(f.pestanas, id: \.seccion) { p in pastilla(p) }
                        }
                        .padding(.horizontal, 12).padding(.vertical, 8)
                    }
                    contenido
                } else {
                    Spacer()
                    ProgressView()
                    Spacer()
                }
            }
            .searchable(text: $modelo.buscado, placement: .navigationBarDrawer(displayMode: .automatic),
                        prompt: TextosFicha.buscar)
            .navigationTitle(titulo.isEmpty ? "Información" : titulo)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cerrar") { cerrar() }.keyboardShortcut(.cancelAction)
                }
            }
        }
        .task { modelo.releer() }
    }

    private func pastilla(_ p: PestanaFicha) -> some View {
        let elegida = modelo.seccion == p.seccion
        return Button { modelo.seccion = p.seccion } label: {
            Text("\(TextosFicha.rotulo(p.seccion))  \(p.cuenta)")
                .font(.subheadline)
                .padding(.horizontal, 12).padding(.vertical, 6)
                .background(elegida ? Paleta.chat.enviar : Color.secondary.opacity(0.15), in: Capsule())
                .foregroundStyle(elegida ? Color.white : Color.primary)
        }
        .buttonStyle(.plain)
    }

    @ViewBuilder private var contenido: some View {
        if let cosas = modelo.cosas {
            if cosas.isEmpty {
                Spacer()
                VStack(spacing: 10) {
                    Image(systemName: TextosFicha.icono(modelo.seccion ?? .todo))
                        .font(.system(size: 32)).foregroundStyle(.secondary)
                    Text(modelo.buscado.isEmpty ? TextosFicha.vacia : TextosFicha.nada).foregroundStyle(.secondary)
                }
                Spacer()
            } else if modelo.pestana?.cuadricula == true {
                cuadricula(cosas)
            } else {
                List(cosas, id: \.mensaje.id) { c in fila(c) }
                    .listStyle(.plain)
            }
        } else {
            Spacer()
            ProgressView()
            Spacer()
        }
    }

    private func ir(_ m: MensajeChat) {
        cerrar()
        Navegacion.compartida.irAlChat(chat, mensaje: m.id)
    }

    private func cuadricula(_ cosas: [CosaDeLaFicha]) -> some View {
        ScrollView {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 112), spacing: 4)], spacing: 4) {
                ForEach(cosas, id: \.mensaje.id) { c in
                    CeldaFotoInfo(mensaje: c.mensaje, chat: chat, almacen: almacen, esVideo: c.esVideo)
                        .onTapGesture { ir(c.mensaje) }
                        .contextMenu {
                            Button { ir(c.mensaje) } label: { Label(TextosFicha.verEnElChat, systemImage: "text.bubble") }
                        }
                }
            }
            .padding(4)
        }
    }

    private func fila(_ c: CosaDeLaFicha) -> some View {
        let m = c.mensaje
        return Button {
            if let e = c.enlace, let url = URL(string: e.url) {
                UIApplication.shared.open(url)
            } else {
                ir(m)
            }
        } label: {
            HStack(spacing: 10) {
                Image(systemName: c.enlace != nil ? "link" : (c.esVideo ? "play.rectangle" : BurbujaChat.icono(m.clase)))
                    .frame(width: 24)
                    .foregroundStyle(Paleta.chat.enviar)
                VStack(alignment: .leading, spacing: 2) {
                    if let e = c.enlace {
                        Text(e.donde).lineLimit(1)
                        Text(e.deQue.isEmpty ? e.url : e.deQue).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                    } else {
                        Text(TextosCompartir.nombreDeFila(nombre: m.nombre, resumen: m.resumen)).lineLimit(1)
                        Text(Date(timeIntervalSince1970: TimeInterval(m.cuando) / 1000),
                             format: .dateTime.day().month().year().hour().minute())
                            .font(.caption).foregroundStyle(.secondary)
                    }
                }
                Spacer(minLength: 0)
                if m.fijado { Image(systemName: "flag.fill").foregroundStyle(.secondary) }
                if let e = m.emoji, !e.isEmpty { Text(e) }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .contextMenu {
            Button { ir(m) } label: { Label(TextosFicha.verEnElChat, systemImage: "text.bubble") }
        }
    }
}

/// Una cosa de la rejilla: su miniatura cuadrada (con el ▶ si es un video).
private struct CeldaFotoInfo: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    var esVideo = false
    @State private var url: URL?

    var body: some View {
        Color.secondary.opacity(0.15)
            .aspectRatio(1, contentMode: .fit)
            .overlay {
                if let url, mensaje.clase == "IMAGEN" || esVideo {
                    MiniaturaVista(url: url, lado: 120)
                        .scaledToFill()
                } else {
                    Image(systemName: BurbujaChat.icono(mensaje.clase))
                        .font(.system(size: 26)).foregroundStyle(.secondary)
                }
            }
            .overlay {
                if esVideo {
                    Image(systemName: "play.circle.fill")
                        .font(.system(size: 30)).foregroundStyle(.white).shadow(radius: 3)
                }
            }
            .clipped()
            .contentShape(Rectangle())
            .task(id: mensaje.id) { url = await resolverFichero(mensaje, chat, almacen) }
    }
}
