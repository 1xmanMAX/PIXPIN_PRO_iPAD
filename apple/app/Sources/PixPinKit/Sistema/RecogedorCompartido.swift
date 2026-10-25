// La app y el buzon del App Group (`BuzonCompartido`):
//
// - deja la lista de proyectos para la extension de Compartir (al abrirse y
//   cada vez que cambia la lista);
// - **recoge lo compartido**: lo que la extension dejo en `entrantes/` entra
//   en su proyecto como si se hubiera soltado en el chat (los ficheros se
//   COPIAN con su mensaje por `Nucleo.adjuntar`, y el comentario, los textos
//   y los enlaces van como una nota, como en el Mac). Se recoge al arrancar,
//   al volver a primer plano y en el momento si la app esta delante (Split
//   View) cuando la extension avisa.
//
// Cada fichero que entra se borra del buzon: si algo falla a medias, lo que
// ya entro no se repite al reintentar.

import Combine
import Foundation
import PixPinCore
import UIKit

@MainActor
public final class RecogedorCompartido {
    public static let compartido = RecogedorCompartido()

    private weak var almacen: Almacen?
    private var suscripciones: Set<AnyCancellable> = []
    private var recogiendo = false
    private var otraVuelta = false
    private var escribirLista: Task<Void, Never>?

    /// Lo que se dice al recoger (las pruebas lo cambian).
    var avisar: @MainActor (String) -> Void = { CuadrosIPad.avisar($0) }

    init() {}

    /// Se pone en marcha con el almacen ya abierto (`SistemaDelIPad`).
    func empezar(_ almacen: Almacen) {
        guard self.almacen == nil else { return }
        self.almacen = almacen
        almacen.$chats
            .dropFirst()
            .sink { [weak self] chats in self?.publicar(chats) }
            .store(in: &suscripciones)
        NotificationCenter.default.publisher(for: UIApplication.didBecomeActiveNotification)
            .sink { [weak self] _ in Task { await self?.recoger() } }
            .store(in: &suscripciones)
        NotificationCenter.default.publisher(for: .pixpinEntregaCompartida)
            .sink { [weak self] _ in Task { await self?.recoger() } }
            .store(in: &suscripciones)
        AvisoDeDarwin.escuchar()
        Task {
            if almacen.chats.isEmpty { await almacen.cargarChats() }
            publicar(almacen.chats)
            await recoger()
        }
    }

    // MARK: La lista de proyectos

    /// Escribe la lista para la extension (con calma: un segundo despues
    /// del ultimo cambio, fuera del hilo principal).
    private func publicar(_ chats: [FichaChat]) {
        guard let raiz = BuzonCompartido.raiz() else { return }
        let lista = RecogedorCompartido.proyectos(de: chats)
        escribirLista?.cancel()
        escribirLista = Task {
            try? await Task.sleep(for: .seconds(1))
            guard !Task.isCancelled else { return }
            _ = await Task.detached(priority: .utility) {
                try? BuzonCompartido.escribirProyectos(lista, en: raiz)
            }.value
        }
    }

    /// Los proyectos que se ofrecen: sin los archivados ni el chat del
    /// asistente (lo que entra ahi lo contestaria la IA).
    nonisolated static func proyectos(de chats: [FichaChat]) -> [ProyectoCompartido] {
        chats.filter { !$0.archivado && !$0.esAsistente }.map {
            ProyectoCompartido(id: $0.id, nombre: $0.nombre, tocado: $0.tocado, esGuardados: $0.esGuardados)
        }
    }

    // MARK: Recoger

    /// Mete en su proyecto todo lo que espera en el buzon.
    public func recoger() async {
        guard let almacen, let raiz = BuzonCompartido.raiz() else { return }
        if recogiendo {
            otraVuelta = true
            return
        }
        recogiendo = true
        defer { recogiendo = false }
        repeat {
            otraVuelta = false
            let pendientes = await Task.detached(priority: .utility) {
                BuzonCompartido.entregasPendientes(en: raiz)
            }.value
            guard !pendientes.isEmpty else { break }
            if almacen.chats.isEmpty { await almacen.cargarChats() }
            var resumen = Resumen()
            for (carpeta, entrega) in pendientes {
                await meter(entrega, de: carpeta, en: almacen, resumen: &resumen)
            }
            if let texto = resumen.texto { avisar(texto) }
        } while otraVuelta
    }

    /// Una entrega, a su proyecto (o a «Mensajes guardados» si ya no esta).
    private func meter(_ e: EntregaCompartida, de carpeta: URL, en almacen: Almacen, resumen: inout Resumen) async {
        var chat = e.chat
        var nombre = e.nombreDelChat
        if !almacen.chats.contains(where: { $0.id == chat }) {
            guard let g = await guardados(almacen) else {
                BuzonCompartido.apartar(carpeta)
                resumen.fallos += 1
                return
            }
            chat = g.id
            nombre = g.nombre
        }
        let ficheros = carpeta.appendingPathComponent(BuzonCompartido.carpetaFicheros, isDirectory: true)
        var bien = true
        var entrados = 0
        for n in e.ficheros {
            let url = ficheros.appendingPathComponent(n)
            // Ya entro en una vuelta anterior (se borra al entrar).
            guard FileManager.default.fileExists(atPath: url.path) else { continue }
            if await almacen.adjuntar(urls: [url], en: chat).isEmpty {
                bien = false
            } else {
                entrados += 1
                try? FileManager.default.removeItem(at: url)
            }
        }
        // La nota, despues de los ficheros y solo si entraron todos (asi un
        // reintento no la repite).
        var nota = false
        if bien, let texto = BuzonCompartido.textoDeLaNota(e) {
            if await almacen.escribir(nota: texto, en: chat) != nil {
                nota = true
            } else {
                bien = false
            }
        }
        if bien {
            BuzonCompartido.quitar(carpeta)
        } else {
            BuzonCompartido.apartar(carpeta)
            resumen.fallos += 1
        }
        if entrados > 0 || nota {
            resumen.anadir(proyecto: nombre, ficheros: entrados, nota: nota)
        }
    }

    private func guardados(_ almacen: Almacen) async -> FichaChat? {
        if let g = almacen.chats.first(where: \.esGuardados) { return g }
        let nucleo = almacen.nucleo
        return try? await Task.detached { try nucleo.chatGuardados() }.value
    }

    /// Lo que se dice al acabar una vuelta.
    struct Resumen {
        var porProyecto: [(nombre: String, ficheros: Int, notas: Int)] = []
        var fallos = 0

        mutating func anadir(proyecto: String, ficheros: Int, nota: Bool) {
            if let i = porProyecto.firstIndex(where: { $0.nombre == proyecto }) {
                porProyecto[i].ficheros += ficheros
                porProyecto[i].notas += nota ? 1 : 0
            } else {
                porProyecto.append((proyecto, ficheros, nota ? 1 : 0))
            }
        }

        var texto: String? {
            var lineas = porProyecto.map { p -> String in
                var partes: [String] = []
                if p.ficheros > 0 { partes.append(p.ficheros == 1 ? "1 archivo" : "\(p.ficheros) archivos") }
                if p.notas > 0 { partes.append(p.notas == 1 ? "1 nota" : "\(p.notas) notas") }
                return "Llegó a «\(p.nombre)»: " + partes.joined(separator: " y ") + "."
            }
            if fallos > 0 {
                lineas.append(fallos == 1
                    ? "Una cosa compartida no se pudo meter en su proyecto; queda guardada para reintentarlo."
                    : "\(fallos) cosas compartidas no se pudieron meter en su proyecto; quedan guardadas para reintentarlo.")
            }
            return lineas.isEmpty ? nil : lineas.joined(separator: "\n")
        }
    }
}

public extension Notification.Name {
    /// La extension de Compartir dejo algo en el buzon.
    static let pixpinEntregaCompartida = Notification.Name("pixpin.entregaCompartida")
}

/// El aviso de Darwin de la extension, pasado al `NotificationCenter` de la
/// app (solo llega si la app esta viva: si no, se recoge al abrirla).
@MainActor
enum AvisoDeDarwin {
    private static var escuchando = false

    static func escuchar() {
        guard !escuchando else { return }
        escuchando = true
        CFNotificationCenterAddObserver(
            CFNotificationCenterGetDarwinNotifyCenter(),
            nil,
            { _, _, _, _, _ in
                DispatchQueue.main.async {
                    NotificationCenter.default.post(name: .pixpinEntregaCompartida, object: nil)
                }
            },
            BuzonCompartido.avisoDeEntrega as CFString,
            nil,
            .deliverImmediately
        )
    }
}
