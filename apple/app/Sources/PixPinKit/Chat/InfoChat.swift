// **El panel Información del chat** (`info-*` de `ventana_chat.rs` en
// Windows): lo de un proyecto por clases, en pestanas: Todo, Fotos,
// Archivos, Voz, Dibujos, Fijados y Buzón. Las fotos en cuadricula; lo demas
// en lista. Un toque lleva al mensaje en su sitio del chat.
//
// Se lee el chat entero al abrir el panel (fuera del hilo principal, por
// paginas, como al compartir un proyecto) y no se guarda nada.

import PixPinCore
import SwiftUI

/// Las pestanas del panel, en el orden de Windows.
enum PestanaInfo: String, CaseIterable, Identifiable {
    case todo, fotos, archivos, voz, dibujos, fijados, buzon

    var id: String { rawValue }

    var titulo: String {
        switch self {
        case .todo: "Todo"
        case .fotos: "Fotos"
        case .archivos: "Archivos"
        case .voz: "Voz"
        case .dibujos: "Dibujos"
        case .fijados: "Fijados"
        case .buzon: "Buzón"
        }
    }

    /// Si `m` va en esta pestana.
    func entra(_ m: MensajeChat) -> Bool {
        switch self {
        case .todo: true
        case .fotos: m.clase == "IMAGEN"
        case .archivos: m.clase == "ARCHIVO"
        case .voz: m.clase == "VOZ"
        case .dibujos: m.clase == "DIBUJO" || m.clase == "PAGINA" || m.clase == "CROQUIS"
        case .fijados: m.fijado
        case .buzon: m.enBuzon
        }
    }

    /// Lo de la pestana, lo mas nuevo arriba.
    static func filtrar(_ mensajes: [MensajeChat], _ p: PestanaInfo) -> [MensajeChat] {
        mensajes.filter(p.entra).sorted { $0.cuando > $1.cuando }
    }
}

struct VistaInfoChat: View {
    let almacen: Almacen
    let chat: String
    let titulo: String
    @Environment(\.dismiss) private var cerrar
    @State private var todos: [MensajeChat]?
    @State private var pestana: PestanaInfo = .todo

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                Picker("Qué", selection: $pestana) {
                    ForEach(PestanaInfo.allCases) { p in
                        Text(p.titulo).tag(p)
                    }
                }
                .pickerStyle(.segmented)
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                if let todos {
                    let lista = PestanaInfo.filtrar(todos, pestana)
                    if lista.isEmpty {
                        Spacer()
                        Text("Aquí no hay nada").foregroundStyle(.secondary)
                        Spacer()
                    } else if pestana == .fotos {
                        cuadricula(lista)
                    } else {
                        List(lista, id: \.id) { m in fila(m) }
                            .listStyle(.plain)
                    }
                } else {
                    Spacer()
                    ProgressView()
                    Spacer()
                }
            }
            .navigationTitle(titulo.isEmpty ? "Información" : "Información · \(titulo)")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cerrar") { cerrar() }.keyboardShortcut(.cancelAction)
                }
            }
        }
        .task { todos = await PrepararCompartir.todos(chat, almacen: almacen) }
    }

    private func ir(_ m: MensajeChat) {
        cerrar()
        Navegacion.compartida.irAlChat(chat, mensaje: m.id)
    }

    private func cuadricula(_ fotos: [MensajeChat]) -> some View {
        ScrollView {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 104), spacing: 4)], spacing: 4) {
                ForEach(fotos, id: \.id) { m in
                    CeldaFotoInfo(mensaje: m, chat: chat, almacen: almacen)
                        .onTapGesture { ir(m) }
                }
            }
            .padding(4)
        }
    }

    private func fila(_ m: MensajeChat) -> some View {
        Button { ir(m) } label: {
            HStack(spacing: 10) {
                Image(systemName: BurbujaChat.icono(m.clase))
                    .frame(width: 24)
                    .foregroundStyle(Paleta.chat.enviar)
                VStack(alignment: .leading, spacing: 2) {
                    Text(TextosCompartir.nombreDeFila(nombre: m.nombre, resumen: m.resumen)).lineLimit(1)
                    Text(Date(timeIntervalSince1970: TimeInterval(m.cuando) / 1000),
                         format: .dateTime.day().month().year().hour().minute())
                        .font(.caption).foregroundStyle(.secondary)
                }
                Spacer(minLength: 0)
                if m.fijado { Image(systemName: "flag.fill").foregroundStyle(.secondary) }
                if let e = m.emoji, !e.isEmpty { Text(e) }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

/// Una foto de la cuadricula: su miniatura cuadrada.
private struct CeldaFotoInfo: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    @State private var url: URL?

    var body: some View {
        Color.secondary.opacity(0.15)
            .aspectRatio(1, contentMode: .fit)
            .overlay {
                if let url {
                    MiniaturaVista(url: url, lado: 120)
                        .scaledToFill()
                }
            }
            .clipped()
            .contentShape(Rectangle())
            .task(id: mensaje.id) { url = await resolverFichero(mensaje, chat, almacen) }
    }
}
