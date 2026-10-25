// **La biblioteca de audio**: toda la musica y todas las notas de voz de
// todos los proyectos, juntas (tanda D3). Es `biblioteca_audio.rs` de
// Windows (`BibliotecaDeAudioActivity.kt` del movil), y como alli **no es un
// almacen**: no guarda nada suyo. Es una vista sobre los chats: los mensajes
// `VOZ` con fichero y fuera del buzon, partidos en dos grupos, la musica
// primero (una voz cuyo texto es la letra de una cancion,
// `estadoDelTexto = letra`) y las notas de voz despues; en cada grupo, lo
// mas nuevo arriba.
//
// Un toque (o ↩) suena en el reproductor de la app, con su barra abajo;
// «Escuchar después» la pone en la cola (el reproductor flotante de Windows
// en cola) y «Escuchar todo» pone la lista entera. El titulo va en color si
// ya tiene letra o transcripcion, y apagado si el fichero no esta en este
// iPad (no va a sonar).
//
// Traido del Mac: lo puro tal cual; la ventana es aqui una hoja. Se abre
// desde el menu de «Mensajes guardados», como en Windows, y desde el «…» de
// la lista de proyectos.

import PixPinCore
import SwiftUI
import UIKit

/// Una fila de la biblioteca (`Fila` de Windows).
public struct FilaAudio: Identifiable, Equatable, Sendable {
    public enum Grupo: Equatable, Sendable { case musica, notas }

    /// El `id` del mensaje.
    public var id: String
    public var chat: String
    /// La ruta del mensaje (relativa a la carpeta de su chat).
    public var ruta: String
    public var titulo: String
    public var duracionMs: Int64
    public var grupo: Grupo
    /// Si ya tiene letra o transcripcion.
    public var tieneLetra: Bool
    public var cuando: Int64
    /// El fichero en este iPad; `nil` si no esta (la fila se ve, no suena).
    public var url: URL?

    /// El id en el reproductor: el mensaje en su chat.
    public var clave: String { id }
}

public enum BibliotecaAudio {
    /// El valor de `estadoDelTexto` que marca «esto es musica, y lo escrito
    /// es su letra» (`TEXTO_LETRA`).
    public static let textoLetra = "letra"

    /// **El rotulo de un audio** (`titulo_de_audio`): el nombre del fichero
    /// sin su extension y, si no hay nombre, «Voz» con la fecha.
    public static func titulo(nombre: String, cuando: Int64) -> String {
        let n = nombre.trimmingCharacters(in: .whitespaces)
        if let punto = n.lastIndex(of: "."), punto != n.startIndex {
            return String(n[..<punto])
        }
        if !n.isEmpty { return n }
        let fecha = Date(timeIntervalSince1970: TimeInterval(cuando) / 1000)
        return "Voz · " + fecha.formatted(date: .abbreviated, time: .shortened)
    }

    /// El titulo de un mensaje de voz.
    public static func titulo(de m: MensajeChat) -> String { titulo(nombre: m.nombre, cuando: m.cuando) }

    /// **Las filas de un chat** (`biblioteca`): clase `VOZ`, con ruta y fuera
    /// del buzon. `musica`: los ids con `estadoDelTexto = letra`.
    public static func filas(_ mensajes: [MensajeChat], chat: String, musica: Set<String>) -> [FilaAudio] {
        mensajes.compactMap { m in
            guard m.clase == "VOZ", !m.enBuzon,
                  let ruta = m.ruta?.trimmingCharacters(in: .whitespaces), !ruta.isEmpty else { return nil }
            return FilaAudio(id: m.id, chat: chat, ruta: ruta, titulo: titulo(de: m), duracionMs: max(m.duracionMs, 0),
                             grupo: musica.contains(m.id) ? .musica : .notas,
                             tieneLetra: !(m.transcripcion ?? "").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
                             cuando: m.cuando, url: nil)
        }
    }

    /// El orden de la biblioteca: la musica primero y, en cada grupo, lo mas
    /// nuevo arriba. Estable: dos notas del mismo milisegundo no bailan.
    public static func ordenar(_ filas: [FilaAudio]) -> [FilaAudio] {
        filas.enumerated().sorted { a, b in
            let ga = a.element.grupo == .notas ? 1 : 0, gb = b.element.grupo == .notas ? 1 : 0
            if ga != gb { return ga < gb }
            if a.element.cuando != b.element.cuando { return a.element.cuando > b.element.cuando }
            return a.offset < b.offset
        }.map(\.element)
    }

    /// **Los ids de musica de un chat**: las lineas de su cuaderno
    /// (`guardados.jsonl`) con `estadoDelTexto = letra`. El puente no trae
    /// ese campo; se mira aqui, sin entender mas de cada linea.
    public static func idsDeMusica(cuaderno: URL) -> Set<String> {
        guard let datos = try? Data(contentsOf: cuaderno, options: .mappedIfSafe) else { return [] }
        var ids: Set<String> = []
        let aguja = Data("\"letra\"".utf8)
        for linea in datos.split(separator: UInt8(ascii: "\n")) where linea.range(of: aguja) != nil {
            guard let o = try? JSONSerialization.jsonObject(with: Data(linea)) as? [String: Any],
                  (o["estadoDelTexto"] as? String) == textoLetra, let id = o["id"] as? String else { continue }
            if (o["clase"] as? String).map({ $0 == "VOZ" }) ?? true { ids.insert(id) }
        }
        return ids
    }

    /// **Lee la biblioteca entera**: todos los chats, fuera del hilo
    /// principal. Se lee al abrirla y no se guarda nada.
    public static func leer(_ nucleo: Nucleo) async -> [FilaAudio] {
        await Task.detached(priority: .userInitiated) { () -> [FilaAudio] in
            var todas: [FilaAudio] = []
            for ficha in (try? nucleo.chats()) ?? [] {
                guard let primera = try? nucleo.mensajes(chat: ficha.id, desde: 0, cuantos: 1) else { continue }
                let total = UInt32(min(primera.total, UInt64(UInt32.max)))
                guard total > 0, let pagina = try? nucleo.mensajes(chat: ficha.id, desde: 0, cuantos: total)
                else { continue }
                let cuaderno = (try? nucleo.rutaAbsoluta(chat: ficha.id, rutaRelativa: "guardados.jsonl"))
                    .map { URL(fileURLWithPath: $0) }
                let musica = cuaderno.map(idsDeMusica(cuaderno:)) ?? []
                for var f in filas(pagina.mensajes, chat: ficha.id, musica: musica) {
                    if let r = try? nucleo.rutaAbsoluta(chat: ficha.id, rutaRelativa: f.ruta),
                       FileManager.default.fileExists(atPath: r) {
                        f.url = URL(fileURLWithPath: r)
                    }
                    todas.append(f)
                }
            }
            return ordenar(todas)
        }.value
    }

    // MARK: La hoja

    /// Abre la biblioteca, encima de lo que se vea.
    @MainActor
    public static func abrir(almacen: Almacen) {
        ArribaUIKit.presentar(estilo: .formSheet, tamano: CGSize(width: 540, height: 640)) { cerrar in
            VistaBibliotecaAudio(almacen: almacen, cerrar: cerrar)
        }
    }
}

/// La lista de la biblioteca, con la barra de lo que suena abajo.
struct VistaBibliotecaAudio: View {
    let almacen: Almacen
    let cerrar: () -> Void
    @ObservedObject private var reproductor = ReproductorVoz.compartido
    @ObservedObject private var grabadora = GrabadoraVoz.compartida
    @State private var filas: [FilaAudio]?
    @State private var aviso: String?

    private var guardados: FichaChat? { almacen.chats.first(where: \.esGuardados) }

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                if let filas {
                    if filas.isEmpty {
                        Spacer()
                        Text("Todavía no hay ninguna nota de voz ni ninguna canción.")
                            .foregroundStyle(.secondary)
                            .multilineTextAlignment(.center)
                            .padding()
                        Spacer()
                    } else {
                        lista(filas)
                    }
                } else {
                    Spacer()
                    ProgressView()
                    Spacer()
                }
                if let aviso {
                    Text(aviso).font(.callout).foregroundStyle(.secondary).padding(.vertical, 4)
                }
                BarraReproductor().padding(8)
            }
            .navigationTitle("Biblioteca de audio")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cerrar", action: cerrar).keyboardShortcut(.cancelAction)
                }
                ToolbarItemGroup(placement: .primaryAction) {
                    Button { escucharTodo(desde: nil) } label: {
                        Label("Escuchar todo", systemImage: "play.circle")
                    }
                    .disabled(!(filas ?? []).contains { $0.url != nil })
                    // Una nota nueva va a «Mensajes guardados», como el pedido
                    // «grabar» de Windows sin proyecto.
                    Button(action: grabar) {
                        Label(grabadora.estado == .quieta ? "Grabar" : "Parar",
                              systemImage: grabadora.estado == .quieta ? "mic" : "stop.fill")
                    }
                    .disabled(guardados == nil)
                    Button { Task { filas = await BibliotecaAudio.leer(almacen.nucleo) } } label: {
                        Label("Volver a leer", systemImage: "arrow.clockwise")
                    }
                }
            }
        }
        .task { filas = await BibliotecaAudio.leer(almacen.nucleo) }
        // Al acabar de grabar, la nota nueva sale en la lista.
        .onChange(of: grabadora.estado == .quieta) { _, quieta in
            guard quieta else { return }
            Task { filas = await BibliotecaAudio.leer(almacen.nucleo) }
        }
    }

    private func lista(_ filas: [FilaAudio]) -> some View {
        // Los dos rotulos solo salen cuando hay de las dos cosas.
        let musica = filas.filter { $0.grupo == .musica }
        let conGrupos = !musica.isEmpty && musica.count < filas.count
        return List {
            if conGrupos {
                Section("Música") { ForEach(musica) { fila($0) } }
                Section("Notas de voz") { ForEach(filas.filter { $0.grupo == .notas }) { fila($0) } }
            } else {
                ForEach(filas) { fila($0) }
            }
        }
        .listStyle(.insetGrouped)
    }

    private func fila(_ f: FilaAudio) -> some View {
        let suena = reproductor.suena(f.clave)
        let enCola = reproductor.cola.contains { $0.id == f.clave }
        return Button { tocar(f) } label: {
            HStack(spacing: 10) {
                Image(systemName: suena ? "pause.fill" : "play.fill")
                    .foregroundStyle(suena ? Paleta.chat.enviar : .secondary)
                    .frame(width: 20)
                Text(f.titulo)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .foregroundStyle(f.url == nil ? Color.secondary : f.tieneLetra ? Paleta.chat.enviar : Color.primary)
                Spacer()
                if enCola { Image(systemName: "text.line.last.and.arrowtriangle.forward").foregroundStyle(.secondary) }
                Text(FormatoVoz.marca(ms: f.duracionMs))
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.secondary)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityHint(f.url == nil ? "El fichero no está en este iPad" : "Escuchar")
        .contextMenu {
            Button { tocar(f) } label: { Label("Escuchar", systemImage: "play") }.disabled(f.url == nil)
            Button { encolar(f) } label: {
                Label("Escuchar después", systemImage: "text.line.last.and.arrowtriangle.forward")
            }
            .disabled(f.url == nil || enCola)
            Button { escucharTodo(desde: f) } label: {
                Label("Escuchar todo desde aquí", systemImage: "play.circle")
            }
            .disabled(f.url == nil)
            Button {
                Navegacion.compartida.cerrarVisor()
                cerrar()
                Navegacion.compartida.seccion = .proyectos
                Navegacion.compartida.irAlChat(f.chat, mensaje: f.id)
            } label: { Label("Ir al mensaje", systemImage: "arrowshape.turn.up.right") }
        }
    }

    private func pista(_ f: FilaAudio) -> Pista? {
        f.url.map { Pista(id: f.clave, url: $0, titulo: f.titulo) }
    }

    private func tocar(_ f: FilaAudio) {
        guard let url = f.url else { return avisar("El fichero no está en este iPad") }
        reproductor.alternar(f.clave, url: url, titulo: f.titulo)
    }

    private func encolar(_ f: FilaAudio) {
        guard let p = pista(f) else { return }
        reproductor.encolar(p)
        avisar("En la cola: \(f.titulo)")
    }

    /// Pone en fila lo de la lista (desde `f`, o todo) y suena el primero.
    private func escucharTodo(desde f: FilaAudio?) {
        let todas = filas ?? []
        let desde = f.flatMap { x in todas.firstIndex { $0.id == x.id } } ?? 0
        let pistas = todas[desde...].compactMap(pista)
        guard !pistas.isEmpty else { return }
        reproductor.poner(pistas)
    }

    private func grabar() {
        guard let g = guardados else { return }
        grabadora.alternar(en: g.id, almacen: almacen)
    }

    private func avisar(_ texto: String) {
        aviso = texto
        Task {
            try? await Task.sleep(nanoseconds: 2_500_000_000)
            if aviso == texto { aviso = nil }
        }
    }
}
