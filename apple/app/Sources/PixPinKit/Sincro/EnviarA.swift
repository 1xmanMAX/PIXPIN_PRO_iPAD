// «Enviar a…»: mandar archivos a lo que otro aparato del grupo tenga
// delante (su chat abierto, su lienzo si son fotos, o la conversacion
// general), como el `suelto` de Windows y Android
// (`pixpin_sincro::al_lienzo`). Va por el canal del grupo y el puerto de
// siempre: sin codigos. Y lo contrario: lo que llega aqui va a lo que este
// iPad tenga delante (`Nucleo.ponerAlFrente`).
//
// Lo puro (los textos y el estado) es el del Mac tal cual. Lo de elegir
// archivos es del iPad: el selector de Archivos (`fileImporter`) da URLs de
// fuera de la caja de arena con permiso solo mientras se usan, asi que se
// copian antes a una carpeta temporal propia y el nucleo lee esas copias.

import PixPinCore
import SwiftUI
import UniformTypeIdentifiers

/// Como va «Enviar a…» con un aparato.
public enum EstadoEnvioSuelto: Equatable, Sendable {
    /// Mandando `n` archivos.
    case enviando(Int)
    /// Hecho: la frase de donde quedo.
    case hecho(String)
    case fallo(String)

    public var texto: String {
        switch self {
        case let .enviando(n): n == 1 ? "Enviando…" : "Enviando \(n) archivos…"
        case let .hecho(t): t
        case let .fallo(t): t
        }
    }
}

/// Donde quedo un archivo en el otro aparato, en palabras.
public func textoDeDonde(_ donde: DondeLlego, chat: String?) -> String {
    switch donde {
    case .lienzo: "en su lienzo"
    case .chatAbierto: chat.map { "en «\($0)»" } ?? "en su chat abierto"
    case .chat: "en Mensajes guardados"
    }
}

/// La frase de un «Enviar a…» terminado: donde quedo lo que llego y por que
/// no llego lo demas.
public func estadoDeEnvio(_ r: [EnvioSuelto], a nombre: String) -> EstadoEnvioSuelto {
    let bien = r.filter { $0.donde != nil }
    let mal = r.filter { $0.donde == nil }
    if bien.isEmpty {
        return .fallo(mal.first?.error ?? "No se envió nada a \(nombre).")
    }
    var partes: [String] = []
    if bien.count == 1, let uno = bien.first, let donde = uno.donde {
        partes.append("«\(uno.nombre)» llegó a \(nombre), \(textoDeDonde(donde, chat: uno.chat)).")
    } else if let donde = bien.first?.donde {
        partes.append("\(bien.count) archivos llegaron a \(nombre), \(textoDeDonde(donde, chat: bien.first?.chat)).")
    }
    for m in mal {
        partes.append("«\(m.nombre)»: \(m.error ?? "no se envió").")
    }
    return .hecho(partes.joined(separator: "\n"))
}

/// La frase de lo que llego suelto a este iPad.
public func textoDeLlegadas(_ l: [ArchivoLlegado], de nombre: String) -> String {
    guard let primero = l.first else { return "" }
    let donde: String
    switch primero.donde {
    case .lienzo: donde = "al lienzo"
    case .chatAbierto: donde = "al chat abierto"
    case .chat: donde = "a Mensajes guardados"
    }
    if l.count == 1 {
        return "\(nombre) envió «\(primero.nombre)» \(donde)."
    }
    return "\(nombre) envió \(l.count) archivos \(donde)."
}

// MARK: - Elegir archivos en el iPad

/// **Copia lo elegido en Archivos a una carpeta temporal propia** (con su
/// nombre), pidiendo el permiso de cada URL mientras se copia. Lo que no se
/// pueda leer se salta. Fuera del hilo principal: un video pesa.
public func copiarParaEnviar(_ urls: [URL]) async -> [URL] {
    await Task.detached(priority: .userInitiated) { () -> [URL] in
        let fm = FileManager.default
        let carpeta = fm.temporaryDirectory
            .appendingPathComponent("pixpin-enviar-\(UUID().uuidString)", isDirectory: true)
        guard (try? fm.createDirectory(at: carpeta, withIntermediateDirectories: true)) != nil else { return [] }
        var copias: [URL] = []
        for u in urls {
            let permiso = u.startAccessingSecurityScopedResource()
            defer { if permiso { u.stopAccessingSecurityScopedResource() } }
            var destino = carpeta.appendingPathComponent(u.lastPathComponent)
            // Dos con el mismo nombre (de carpetas distintas): el segundo en
            // su propia subcarpeta, para que llegue con su nombre.
            if fm.fileExists(atPath: destino.path) {
                let sub = carpeta.appendingPathComponent(UUID().uuidString, isDirectory: true)
                try? fm.createDirectory(at: sub, withIntermediateDirectories: true)
                destino = sub.appendingPathComponent(u.lastPathComponent)
            }
            do {
                try fm.copyItem(at: u, to: destino)
                copias.append(destino)
            } catch {
                NSLog("PixPin: no se pudo leer %@ para enviarlo: %@", u.lastPathComponent, "\(error)")
            }
        }
        return copias
    }.value
}

extension View {
    /// El selector de Archivos para elegir **varios archivos** y mandarlos:
    /// `alElegir` recibe ya las copias propias (`copiarParaEnviar`).
    func elegirArchivosParaEnviar(_ abierto: Binding<Bool>,
                                  alElegir: @escaping @MainActor ([URL]) -> Void) -> some View {
        fileImporter(isPresented: abierto, allowedContentTypes: [.item], allowsMultipleSelection: true) { r in
            guard case let .success(urls) = r, !urls.isEmpty else { return }
            Task { @MainActor in
                let copias = await copiarParaEnviar(urls)
                if !copias.isEmpty { alElegir(copias) }
            }
        }
    }
}

/// **El menu «Enviar a…»**: los aparatos del grupo que hay en la Wi-Fi; al
/// elegir uno se eligen los archivos en Archivos y se mandan. En cada uno,
/// como fue lo ultimo. Lo puede poner cualquier barra (el chat, un visor).
public struct MenuEnviarA: View {
    @ObservedObject var sincro: Sincronizador
    @State private var destino: String?
    @State private var eligiendo = false

    public init(sincro: Sincronizador) {
        _sincro = ObservedObject(wrappedValue: sincro)
    }

    public var body: some View {
        Menu {
            if sincro.filas.isEmpty {
                Text(sincro.codigoLegible == nil ? "Sin grupo: únete en Aparatos"
                    : "Ningún aparato del grupo en esta Wi-Fi")
            }
            ForEach(sincro.filas) { fila in
                Button(etiqueta(fila)) {
                    destino = fila.id
                    eligiendo = true
                }
                .disabled(fila.direccion == nil && (fila.estado == .noContesta || fila.estado == .noEncontrado))
            }
        } label: {
            Label("Enviar a…", systemImage: "paperplane")
        }
        .elegirArchivosParaEnviar($eligiendo) { urls in
            guard let id = destino else { return }
            Task { await sincro.enviar(urls, a: id) }
        }
    }

    private func etiqueta(_ fila: Sincronizador.Fila) -> String {
        guard let e = sincro.envios[fila.id] else { return fila.nombre }
        let linea = e.texto.split(separator: "\n").first.map(String.init) ?? ""
        return "\(fila.nombre) — \(linea)"
    }
}
