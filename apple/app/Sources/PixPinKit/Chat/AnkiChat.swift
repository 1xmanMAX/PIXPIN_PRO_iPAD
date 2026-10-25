// **«Enviar a Anki»** del menu de un mensaje (Windows
// `ventana_chat/anki.rs`, 24-oct-2026, para estudiar: «ayuda mucho a
// estudiantes a recordar»). Las tarjetas las saca el nucleo con el crate de
// Windows (`pixpin_anki`):
//
// - **una nota**: sus «pregunta :: respuesta», «P: … / R: …» y los huecos
//   `{{c1::…}}`;
// - **una foto**: la imagen delante y su descripcion detras.
//
// Van al mazo `PixPin::<proyecto>` con las etiquetas `pixpin` y la del
// proyecto. En Windows, con Anki abierto, entran directas por AnkiConnect;
// en el iPad eso no existe (AnkiMobile no lo trae, y `127.0.0.1` es el
// propio iPad), asi que se hace lo que Windows hace sin Anki abierto: el
// fichero de «Archivo → Importar», que aqui se comparte (a AnkiMobile, al
// Mac o al PC por AirDrop, o a Archivos) junto con las imagenes.
//
// Falta (Windows lo tiene): una clase grabada (una tarjeta por captura con
// lo dicho en su tramo); el iPad aun no graba clases.

import PixPinCore
import SwiftUI
import UIKit

@MainActor
enum AnkiChat {
    /// Si el mensaje da alguna tarjeta (para ofrecerlo en el menu).
    static func sePuede(_ m: MensajeChat) -> Bool {
        switch m.clase {
        case "IMAGEN": return m.ruta?.isEmpty == false
        case "VOZ": return false
        default:
            // Es una cuenta de texto pura: se hace al abrir el menu.
            let t = m.texto
            guard t.contains(":") || t.contains(";;") else { return false }
            return !ankiTarjetasDeLaNota(texto: t, proyecto: "").isEmpty
        }
    }

    /// **Saca las tarjetas** y abre la hoja para mandarlas.
    static func enviar(_ m: MensajeChat, c: ChatAbierto) {
        let proyecto = c.almacen.chats.first { $0.id == c.chat }?.nombre ?? "PixPin"
        let mazo = ankiMazoDe(proyecto: proyecto)
        let (chat, almacen) = (c.chat, c.almacen)
        _ = Task<Void, Never> {
            var imagenes: [URL] = []
            var tarjetas: [TarjetaAnki] = []
            if m.clase == "IMAGEN" {
                guard let url = await resolverFichero(m, chat, almacen),
                      FileManager.default.fileExists(atPath: url.path) else {
                    c.avisar("Esta foto no está en este iPad")
                    return
                }
                let nombre = ankiNombreDeImagen(id: m.id, ext: url.pathExtension)
                tarjetas = [ankiTarjetaDeLaFoto(imagen: nombre, descripcion: m.texto,
                                                titulo: m.nombre, proyecto: proyecto)]
                imagenes = [url]
            } else {
                tarjetas = ankiTarjetasDeLaNota(texto: m.texto, proyecto: proyecto)
            }
            guard !tarjetas.isEmpty else {
                c.avisar("Aquí no hay tarjetas. Escribe una por línea como «pregunta :: respuesta», o «P: …» y «R: …», o con huecos {{c1::así}}.")
                return
            }
            guard let ficheros = await escribir(mazo: mazo, tarjetas: tarjetas, imagenes: imagenes) else {
                c.avisar("No se pudo")
                return
            }
            ArribaUIKit.presentar(estilo: .formSheet, tamano: CGSize(width: 540, height: 560)) { cerrar in
                HojaAnki(mazo: mazo, tarjetas: tarjetas, ficheros: ficheros, cerrar: cerrar)
            }
        }
    }

    /// El fichero de importar y las imagenes con su nombre de Anki, en una
    /// carpeta temporal. El primero es el fichero.
    nonisolated static func escribir(mazo: String, tarjetas: [TarjetaAnki], imagenes: [URL]) async -> [URL]? {
        await Task.detached(priority: .userInitiated) { () -> [URL]? in
            let fm = FileManager.default
            let carpeta = fm.temporaryDirectory.appendingPathComponent("anki", isDirectory: true)
                .appendingPathComponent(UUID().uuidString, isDirectory: true)
            do {
                try fm.createDirectory(at: carpeta, withIntermediateDirectories: true)
                let txt = carpeta.appendingPathComponent(ankiNombreDeFichero(mazo: mazo))
                try Data(ankiFichero(mazo: mazo, tarjetas: tarjetas).utf8).write(to: txt)
                var v = [txt]
                let nombres = tarjetas.flatMap(\.imagenes)
                for (url, nombre) in zip(imagenes, nombres) {
                    let destino = carpeta.appendingPathComponent(nombre)
                    try fm.copyItem(at: url, to: destino)
                    v.append(destino)
                }
                return v
            } catch {
                return nil
            }
        }.value
    }

    /// Una tarjeta en una linea, sin el HTML (para la lista de la hoja).
    nonisolated static func plano(_ html: String) -> String {
        var s = html.replacingOccurrences(of: "<br>", with: " ")
        while let a = s.range(of: "<"), let b = s.range(of: ">", range: a.upperBound..<s.endIndex) {
            s.removeSubrange(a.lowerBound..<b.upperBound)
        }
        for (de, a) in [("&lt;", "<"), ("&gt;", ">"), ("&quot;", "\""), ("&amp;", "&")] {
            s = s.replacingOccurrences(of: de, with: a)
        }
        return s.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

/// La hoja de «Enviar a Anki»: el mazo, las tarjetas y el fichero listo.
struct HojaAnki: View {
    let mazo: String
    let tarjetas: [TarjetaAnki]
    let ficheros: [URL]
    let cerrar: () -> Void
    @State private var compartiendo: CosasACompartir?

    var body: some View {
        NavigationStack {
            List {
                Section {
                    LabeledContent("Mazo", value: mazo)
                    LabeledContent("Tarjetas", value: "\(tarjetas.count)")
                } footer: {
                    Text("En el iPad Anki no recibe las tarjetas directas (no hay AnkiConnect). Comparte el fichero con Anki —AnkiMobile, o el Anki del Mac o del PC por AirDrop o Archivos— y en Anki, Archivo → Importar." +
                         (ficheros.count > 1 ? " Las imágenes van al lado: ponlas en la carpeta de medios de Anki (collection.media)." : ""))
                }
                Section("Tarjetas") {
                    ForEach(Array(tarjetas.prefix(50).enumerated()), id: \.offset) { _, t in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(t.imagenes.isEmpty ? AnkiChat.plano(t.frente) : "🖼 " + AnkiChat.plano(t.frente))
                                .font(.body.weight(.medium)).lineLimit(2)
                            Text(AnkiChat.plano(t.reverso)).font(.callout).foregroundStyle(.secondary).lineLimit(3)
                        }
                    }
                    if tarjetas.count > 50 {
                        Text("… y \(tarjetas.count - 50) más").foregroundStyle(.secondary)
                    }
                }
            }
            .navigationTitle("Enviar a Anki")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cerrar", action: cerrar).keyboardShortcut(.cancelAction)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button { compartiendo = CosasACompartir(cosas: ficheros) } label: {
                        Label("Compartir…", systemImage: "square.and.arrow.up")
                    }
                }
            }
        }
        .sheet(item: $compartiendo) { c in
            ActividadCompartir(cosas: c.cosas).ignoresSafeArea()
        }
    }
}
