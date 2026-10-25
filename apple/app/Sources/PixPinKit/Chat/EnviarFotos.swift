// **El cuadro de enviar fotos con su descripcion** (`ventana_chat/envio.rs`
// de Windows, `Menus2.dc.html` d): lo elegido en Fotos, antes de meterlo en
// el proyecto, en miniaturas numeradas. Se reordenan **arrastrando** (o con
// el menu de cada una), se quitan con su ✕ y se anaden mas con la baldosa
// «Añadir». Debajo, la descripcion, que va con la primera (como el pie de
// una foto en el movil), y abajo «Cancelar» y «Enviar».
//
// Lo que se mete no cambia: cada foto con `Almacen.adjuntar(datos:)`, en el
// orden del cuadro, y la descripcion con `CambioMensaje.descripcion`.

import ImageIO
import PhotosUI
import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// Una foto (o un video) por enviar: sus bytes y su miniatura.
struct FotoPorEnviar: Identifiable, Equatable {
    let id = UUID()
    var datos: Data
    /// La extension con la que entra (`jpg`, `png`, `heic`, `mov`...).
    var ext: String
    var miniatura: UIImage?

    var esVideo: Bool { UTType(filenameExtension: ext)?.conforms(to: .movie) ?? false }

    static func == (a: FotoPorEnviar, b: FotoPorEnviar) -> Bool { a.id == b.id }
}

/// Lo que se va a enviar de una vez.
struct LoteDeFotos: Identifiable {
    let id = UUID()
    var fotos: [FotoPorEnviar]
}

/// **El orden y lo quitado** del cuadro (puro: se prueba solo).
enum OrdenDeEnvio {
    /// Mueve el de `desde` a donde esta `hasta` (arrastrar una miniatura
    /// encima de otra), corriendo las demas.
    static func mover<T>(_ v: [T], desde: Int, hasta: Int) -> [T] {
        guard v.indices.contains(desde), v.indices.contains(hasta), desde != hasta else { return v }
        var w = v
        let x = w.remove(at: desde)
        w.insert(x, at: hasta)
        return w
    }

    /// «3 fotos · a Obra» (`v2menus-envio-fotos`), o «archivos» si hay
    /// videos entre ellas.
    static func resumen(fotos: Int, videos: Int, proyecto: String) -> String {
        let n = fotos + videos
        let que = videos > 0 && fotos == 0
            ? (n == 1 ? "1 vídeo" : "\(n) vídeos")
            : videos > 0 ? (n == 1 ? "1 archivo" : "\(n) archivos") : (n == 1 ? "1 foto" : "\(n) fotos")
        return proyecto.isEmpty ? que : "\(que) · a \(proyecto)"
    }

    /// Una miniatura de `datos` (sin decodificar la foto entera).
    nonisolated static func miniatura(_ datos: Data, lado: Int = 220) -> UIImage? {
        guard let fuente = CGImageSourceCreateWithData(datos as CFData, nil) else { return nil }
        let opciones: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: lado,
        ]
        guard let cg = CGImageSourceCreateThumbnailAtIndex(fuente, 0, opciones as CFDictionary) else { return nil }
        return UIImage(cgImage: cg)
    }

    /// Lee lo elegido en Fotos: los bytes tal cual y su miniatura.
    static func leer(_ elegidas: [PhotosPickerItem]) async -> [FotoPorEnviar] {
        var v: [FotoPorEnviar] = []
        for item in elegidas {
            guard let datos = try? await item.loadTransferable(type: Data.self) else { continue }
            let ext = item.supportedContentTypes.first?.preferredFilenameExtension ?? "jpg"
            let mini = await Task.detached(priority: .userInitiated) { OrdenDeEnvio.miniatura(datos) }.value
            v.append(FotoPorEnviar(datos: datos, ext: ext, miniatura: mini))
        }
        return v
    }
}

extension Almacen {
    /// **Mete un lote de fotos** en el chat, en su orden; la descripcion va
    /// con la primera. Devuelve los mensajes nuevos.
    @discardableResult
    func enviar(fotos: [FotoPorEnviar], descripcion: String, en chat: String) async -> [MensajeChat] {
        var nuevos: [MensajeChat] = []
        let pie = descripcion.trimmingCharacters(in: .whitespacesAndNewlines)
        for f in fotos {
            guard let m = await adjuntar(datos: f.datos, formato: f.ext, en: chat) else { continue }
            if nuevos.isEmpty, !pie.isEmpty, let con = await cambiar(mensaje: m.id, en: chat, .descripcion(texto: pie)) {
                nuevos.append(con)
            } else {
                nuevos.append(m)
            }
        }
        return nuevos
    }
}

/// El cuadro, en una hoja.
struct HojaEnviarFotos: View {
    @State var lote: LoteDeFotos
    let proyecto: String
    let enviar: ([FotoPorEnviar], String) -> Void
    @Environment(\.dismiss) private var cerrar
    @State private var descripcion = ""
    @State private var anadiendo = false
    @State private var mas: [PhotosPickerItem] = []
    @FocusState private var enPie: Bool

    private let columnas = [GridItem(.adaptive(minimum: 96, maximum: 112), spacing: 10)]

    private var resumen: String {
        let videos = lote.fotos.filter(\.esVideo).count
        return OrdenDeEnvio.resumen(fotos: lote.fotos.count - videos, videos: videos, proyecto: proyecto)
    }

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 14) {
                ScrollView {
                    LazyVGrid(columns: columnas, spacing: 10) {
                        ForEach(Array(lote.fotos.enumerated()), id: \.element.id) { i, f in
                            miniatura(f, numero: i + 1)
                        }
                        baldosaAnadir
                    }
                    .padding(.vertical, 4)
                }
                TextField("Añade una descripción…", text: $descripcion, axis: .vertical)
                    .lineLimit(1 ... 4)
                    .textFieldStyle(.roundedBorder)
                    .focused($enPie)
                    .submitLabel(.send)
                    .onSubmit(hecho)
                Text("La descripción va con la primera")
                    .font(.caption).foregroundStyle(.secondary)
            }
            .padding(16)
            .navigationTitle(resumen)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancelar") { cerrar() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Enviar", action: hecho)
                        .disabled(lote.fotos.isEmpty)
                        .keyboardShortcut(.defaultAction)
                }
            }
        }
        .photosPicker(isPresented: $anadiendo, selection: $mas, maxSelectionCount: 30,
                      matching: .any(of: [.images, .videos]))
        .onChange(of: mas) { _, nuevas in
            guard !nuevas.isEmpty else { return }
            mas = []
            Task {
                let leidas = await OrdenDeEnvio.leer(nuevas)
                lote.fotos += leidas
            }
        }
        .presentationDetents([.medium, .large])
    }

    private func miniatura(_ f: FotoPorEnviar, numero: Int) -> some View {
        ZStack(alignment: .topTrailing) {
            Group {
                if let m = f.miniatura {
                    Image(uiImage: m).resizable().scaledToFill()
                } else {
                    Rectangle().fill(.quaternary)
                        .overlay { Image(systemName: f.esVideo ? "video.fill" : "photo").font(.title2) }
                }
            }
            .frame(width: 96, height: 96)
            .clipShape(RoundedRectangle(cornerRadius: 10))
            // La primera lleva la descripcion: va marcada en azul.
            .overlay {
                if numero == 1 {
                    RoundedRectangle(cornerRadius: 10).strokeBorder(Paleta.chat.enviar, lineWidth: 3)
                }
            }
            .overlay(alignment: .bottomLeading) {
                Text("\(numero)")
                    .font(.caption2.weight(.bold))
                    .foregroundStyle(.white)
                    .frame(width: 20, height: 20)
                    .background(numero == 1 ? Paleta.chat.enviar : Color.black.opacity(0.6), in: Circle())
                    .padding(4)
            }
            Button {
                withAnimation { lote.fotos.removeAll { $0.id == f.id } }
            } label: {
                Image(systemName: "xmark.circle.fill")
                    .font(.system(size: 22))
                    .symbolRenderingMode(.palette)
                    .foregroundStyle(.white, .black.opacity(0.65))
            }
            .buttonStyle(.plain)
            .padding(3)
            .accessibilityLabel("Quitar")
        }
        .draggable(f.id.uuidString) {
            if let m = f.miniatura {
                Image(uiImage: m).resizable().scaledToFill().frame(width: 80, height: 80)
                    .clipShape(RoundedRectangle(cornerRadius: 10))
            }
        }
        .dropDestination(for: String.self) { ids, _ in
            guard let id = ids.first, let desde = lote.fotos.firstIndex(where: { $0.id.uuidString == id }),
                  let hasta = lote.fotos.firstIndex(where: { $0.id == f.id }) else { return false }
            withAnimation { lote.fotos = OrdenDeEnvio.mover(lote.fotos, desde: desde, hasta: hasta) }
            return true
        }
        .contextMenu {
            if let i = lote.fotos.firstIndex(where: { $0.id == f.id }) {
                if i > 0 {
                    Button { lote.fotos = OrdenDeEnvio.mover(lote.fotos, desde: i, hasta: 0) } label: {
                        Label("Poner la primera", systemImage: "arrow.up.to.line")
                    }
                    Button { lote.fotos = OrdenDeEnvio.mover(lote.fotos, desde: i, hasta: i - 1) } label: {
                        Label("Antes", systemImage: "arrow.left")
                    }
                }
                if i < lote.fotos.count - 1 {
                    Button { lote.fotos = OrdenDeEnvio.mover(lote.fotos, desde: i, hasta: i + 1) } label: {
                        Label("Después", systemImage: "arrow.right")
                    }
                }
                Button(role: .destructive) { lote.fotos.remove(at: i) } label: {
                    Label("Quitar", systemImage: "xmark")
                }
            }
        }
    }

    private var baldosaAnadir: some View {
        Button { anadiendo = true } label: {
            VStack(spacing: 6) {
                Image(systemName: "plus").font(.system(size: 24, weight: .light))
                Text("Añadir").font(.caption)
            }
            .foregroundStyle(.secondary)
            .frame(width: 96, height: 96)
            .background(RoundedRectangle(cornerRadius: 10).strokeBorder(.secondary.opacity(0.5),
                                                                        style: StrokeStyle(lineWidth: 1.5, dash: [5, 4])))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func hecho() {
        guard !lote.fotos.isEmpty else { return }
        enviar(lote.fotos, descripcion)
        cerrar()
    }
}
