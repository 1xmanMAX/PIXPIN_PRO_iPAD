// **La pantalla de la letra de una clase** (Windows edd0ffa,
// `reparto_de_la_clase` y `pintar_capturas_de_la_letra`; del Mac,
// `Grabar/VisorClase.swift`): el texto a un lado y, al otro, la imagen del
// minuto que suena, con una ficha por captura que lleva el audio (y la
// letra) ahi. Se abre desde la nota de voz de una clase en el chat («Ver con
// las capturas»). En el iPad, una hoja grande en vez de una ventana.
//
// Las capturas de una nota salen del cuaderno (`Nucleo.capturasDeLaClase`:
// el `capturas` de la voz) y se guardan aqui por mensaje: el menu y la
// burbuja preguntan a menudo y el cuaderno no se lee cada vez.

import PixPinCore
import SwiftUI
import UIKit

/// Las capturas de las notas de voz grabadas como clase, por mensaje.
@MainActor
enum ClaseEnElChat {
    private static var leidas: [String: [CapturaDeLaClase]] = [:]

    private static func clave(_ chat: String, _ id: String) -> String { chat + "\u{0}" + id }

    /// Las capturas de `m` (vacio si no es una nota de voz de una clase).
    /// La primera vez lee el cuaderno.
    static func capturas(_ m: MensajeChat, chat: String, nucleo: Nucleo) -> [CapturaDeLaClase] {
        guard m.clase == "VOZ" else { return [] }
        let k = clave(chat, m.id)
        if let ya = leidas[k] { return ya }
        let v = (try? nucleo.capturasDeLaClase(chat: chat, mensaje: m.id)) ?? []
        leidas[k] = v
        return v
    }

    /// Una nota que acaba de cambiar (se guardo la clase): se vuelve a leer.
    static func olvidar(chat: String, mensaje: String) {
        leidas[clave(chat, mensaje)] = nil
    }

    /// La captura que toca en `posicionMs`: la ultima hecha antes (la
    /// primera si aun no llego ninguna). `nil` sin capturas.
    nonisolated static func enElMinuto(_ capturas: [CapturaDeLaClase], posicionMs: Int64) -> Int? {
        captura(deLaClaseEn: capturas.map(\.ms), posicionMs: posicionMs)
    }

    nonisolated static func captura(deLaClaseEn tiempos: [Int64], posicionMs: Int64) -> Int? {
        capturaDeLaClaseEn(tiempos: tiempos, posicionMs: posicionMs).map(Int.init)
    }
}

/// La hoja del visor de una clase.
@MainActor
enum VisorClase {
    /// Abre el visor de la nota de voz `m` de `chat`.
    static func abrir(_ m: MensajeChat, chat: String, almacen: Almacen) {
        let capturas = ClaseEnElChat.capturas(m, chat: chat, nucleo: almacen.nucleo).filter { $0.ruta != nil }
        guard !capturas.isEmpty else { return }
        ArribaUIKit.presentar(estilo: .pageSheet) { cerrar in
            NavigationStack {
                VistaVisorClase(mensaje: m, chat: chat, almacen: almacen, capturas: capturas)
                    .navigationTitle(BibliotecaAudio.titulo(de: m))
                    .navigationBarTitleDisplayMode(.inline)
                    .toolbar {
                        ToolbarItem(placement: .cancellationAction) {
                            Button("Cerrar", action: cerrar).keyboardShortcut(.cancelAction)
                        }
                    }
            }
        }
    }
}

struct VistaVisorClase: View {
    let mensaje: MensajeChat
    let chat: String
    let almacen: Almacen
    let capturas: [CapturaDeLaClase]
    @ObservedObject private var reproductor = ReproductorVoz.compartido
    @State private var url: URL?
    @State private var imagenes: [String: UIImage] = [:]

    private var suena: Bool { reproductor.actual == mensaje.id }

    var body: some View {
        VStack(spacing: 8) {
            // El texto en algo mas de la mitad, como en Windows (52 %).
            GeometryReader { geo in
                HStack(alignment: .top, spacing: 12) {
                    letra.frame(width: geo.size.width * 0.52)
                    capturasDelMinuto.frame(maxWidth: .infinity)
                }
            }
            BarraReproductor(conCerrar: false)
        }
        .padding(12)
        .task {
            if let ruta = mensaje.ruta, let u = await almacen.url(deRuta: ruta, en: chat),
               FileManager.default.fileExists(atPath: u.path) {
                url = u
            }
            // Las imagenes, fuera del hilo principal (pocas).
            let rutas = capturas.compactMap(\.ruta)
            let leidas = await Task.detached(priority: .userInitiated) { () -> [(String, UIImage)] in
                rutas.compactMap { r in UIImage(contentsOfFile: r).map { (r, $0) } }
            }.value
            for (r, i) in leidas { imagenes[r] = i }
        }
    }

    /// El texto, por parrafos con su minuto: un toque lleva el audio ahi.
    private var letra: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 10) {
                let trozos = FormatoVoz.trozos(mensaje.transcripcion ?? "")
                if trozos.isEmpty {
                    Text("Para el texto, pulsa «Pasar a texto» en la nota del chat.")
                        .foregroundStyle(.secondary)
                }
                ForEach(Array(trozos.enumerated()), id: \.offset) { _, t in
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        if let ms = t.ms {
                            Button { ir(a: ms) } label: {
                                Text(FormatoVoz.marca(ms: ms)).font(.caption.monospacedDigit())
                            }
                            .buttonStyle(.borderless)
                        }
                        Text(t.texto).textSelection(.enabled)
                    }
                }
            }
            .padding(8)
        }
    }

    /// La imagen del minuto que suena y, debajo, una ficha por captura.
    private var capturasDelMinuto: some View {
        TimelineView(.periodic(from: .now, by: 0.25)) { _ in
            let posicion = suena ? reproductor.posicionMs : 0
            let actual = ClaseEnElChat.enElMinuto(capturas, posicionMs: posicion)
            VStack(spacing: 10) {
                ZStack {
                    RoundedRectangle(cornerRadius: 10).fill(Color.primary.opacity(0.06))
                    if let n = actual, let r = capturas[n].ruta, let i = imagenes[r] {
                        Image(uiImage: i).resizable().scaledToFit().padding(4)
                    }
                }
                .frame(maxHeight: .infinity)
                FichasDeLaClase(capturas: capturas, actual: actual) { ir(a: $0) }
            }
        }
    }

    /// Lleva el audio al minuto `ms` (y lo pone a sonar si no sonaba).
    private func ir(a ms: Int64) {
        guard let url else { return }
        if !suena {
            reproductor.alternar(mensaje.id, url: url, titulo: BibliotecaAudio.titulo(de: mensaje))
        }
        reproductor.saltar(ms: ms - reproductor.posicionMs)
    }
}

/// Las fichas «⏱ 12:34», en tantas filas como hagan falta.
private struct FichasDeLaClase: View {
    let capturas: [CapturaDeLaClase]
    let actual: Int?
    let ir: (Int64) -> Void

    var body: some View {
        let columnas = [GridItem(.adaptive(minimum: 80), spacing: 6)]
        LazyVGrid(columns: columnas, alignment: .leading, spacing: 6) {
            ForEach(Array(capturas.enumerated()), id: \.offset) { n, c in
                let es = actual == n
                Button { ir(c.ms) } label: {
                    Text("⏱ \(FormatoVoz.marca(ms: c.ms))")
                        .font(.system(size: 13).monospacedDigit())
                        .foregroundStyle(es ? Color.white : Color.primary)
                        .padding(.horizontal, 10)
                        .frame(height: 30)
                        .background(Color.accentColor.opacity(es ? 0.85 : 0.18), in: Capsule())
                }
                .buttonStyle(.plain)
            }
        }
    }
}
