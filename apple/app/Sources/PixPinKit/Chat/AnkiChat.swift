// **«Enviar a Anki»** del menu de un mensaje (Windows 2561be0,
// `ventana_chat/anki.rs`): lo hace el nucleo, igual que en el Mac
// (`Nucleo.enviarAAnki`, `Chat/EnviarAAnki.swift` copiado tal cual): las
// tarjetas de una clase grabada (una por captura), de una foto o de una nota
// («pregunta :: respuesta», «P: / R:», huecos `{{c1::…}}`), al mazo
// `PixPin::<proyecto>`.
//
// En el iPad no hay AnkiConnect (AnkiMobile no lo trae, y `127.0.0.1` es el
// propio iPad), asi que el nucleo hace lo que Windows sin Anki abierto: el
// fichero de «Archivo → Importar» en `<raiz>/anki/`. Aqui se comparte (a
// AnkiMobile, al Mac o al PC por AirDrop, o a Archivos) junto con las
// imagenes, con el nombre que les da el nucleo (`pixpin-<id>.<ext>`).

import PixPinCore
import SwiftUI
import UIKit

@MainActor
enum AnkiChat {
    /// Si el mensaje da alguna tarjeta (para ofrecerlo en el menu), como el
    /// Mac (`ChatAbierto.sePuedeAnki`).
    static func sePuede(_ m: MensajeChat, c: ChatAbierto) -> Bool {
        c.sePuedeAnki(m)
    }

    /// **Manda las tarjetas**: con Anki al alcance, directas (el nucleo); sin
    /// el (lo normal en el iPad), la hoja para compartir el fichero.
    static func enviar(_ m: MensajeChat, c: ChatAbierto) {
        let (chat, almacen) = (c.chat, c.almacen)
        let nucleo = almacen.nucleo
        let id = m.id
        _ = Task<Void, Never> {
            let hecho = await Task.detached(priority: .userInitiated) { () -> Result<EnvioAnki, Error> in
                Result { try nucleo.enviarAAnki(chat: chat, mensaje: id, preguntaDeLaClase: "¿Qué se explicó aquí?") }
            }.value
            guard case let .success(r) = hecho else {
                c.avisar("No se pudo: mira el registro de PixPin")
                return
            }
            guard case let .fichero(n, mazo, ruta) = r else {
                c.avisar(ChatAbierto.textoDeAnki(r))
                return
            }
            // Las imagenes de las tarjetas, con el nombre que les da el nucleo.
            var imagenes: [URL] = []
            let fuentes: [MensajeChat] = m.clase == "IMAGEN" ? [m]
                : ClaseEnElChat.capturas(m, chat: chat, nucleo: nucleo).compactMap { cap in
                    (almacen.mensajes[chat] ?? []).first { $0.id == cap.id }
                }
            for f in fuentes {
                if let u = await resolverFichero(f, chat, almacen) { imagenes.append(u) }
            }
            let ids = fuentes.map(\.id)
            guard let ficheros = await juntar(fichero: URL(fileURLWithPath: ruta), imagenes: Array(zip(ids, imagenes))) else {
                c.avisar(ChatAbierto.textoDeAnki(r))
                return
            }
            ArribaUIKit.presentar(estilo: .formSheet, tamano: CGSize(width: 520, height: 360)) { cerrar in
                HojaAnki(mazo: mazo, n: Int(n), ficheros: ficheros, cerrar: cerrar)
            }
        }
    }

    /// El nombre de una imagen en Anki (`anki.rs`: `pixpin-<id>.<ext>`).
    nonisolated static func nombreDeImagen(id: String, url: URL) -> String {
        let ext = url.pathExtension.isEmpty ? "png" : url.pathExtension.lowercased()
        return "pixpin-\(id).\(ext)"
    }

    /// El fichero de importar y las imagenes con su nombre de Anki, en una
    /// carpeta temporal. El primero es el fichero.
    nonisolated static func juntar(fichero: URL, imagenes: [(String, URL)]) async -> [URL]? {
        await Task.detached(priority: .userInitiated) { () -> [URL]? in
            let fm = FileManager.default
            guard fm.fileExists(atPath: fichero.path) else { return nil }
            let carpeta = fm.temporaryDirectory.appendingPathComponent("anki", isDirectory: true)
                .appendingPathComponent(UUID().uuidString, isDirectory: true)
            do {
                try fm.createDirectory(at: carpeta, withIntermediateDirectories: true)
                let txt = carpeta.appendingPathComponent(fichero.lastPathComponent)
                try fm.copyItem(at: fichero, to: txt)
                var v = [txt]
                for (id, url) in imagenes where fm.fileExists(atPath: url.path) {
                    let destino = carpeta.appendingPathComponent(nombreDeImagen(id: id, url: url))
                    try? fm.copyItem(at: url, to: destino)
                    v.append(destino)
                }
                return v
            } catch {
                return nil
            }
        }.value
    }
}

/// La hoja de «Enviar a Anki» sin Anki al alcance: el mazo, cuantas tarjetas
/// y el fichero listo para compartir.
struct HojaAnki: View {
    let mazo: String
    let n: Int
    let ficheros: [URL]
    let cerrar: () -> Void
    @State private var compartiendo: CosasACompartir?

    var body: some View {
        NavigationStack {
            List {
                Section {
                    LabeledContent("Mazo", value: mazo)
                    LabeledContent("Tarjetas", value: "\(n)")
                } footer: {
                    Text("En el iPad Anki no recibe las tarjetas directas (no hay AnkiConnect). Comparte el fichero con Anki —AnkiMobile, o el Anki del Mac o del PC por AirDrop o Archivos— y en Anki, Archivo → Importar." +
                         (ficheros.count > 1 ? " Las imágenes van al lado: ponlas en la carpeta de medios de Anki (collection.media)." : ""))
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
