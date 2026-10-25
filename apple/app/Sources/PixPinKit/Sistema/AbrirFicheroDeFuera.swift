// Un fichero que llega de otra app (Archivos, «Abrir en…», AirDrop) a su
// sitio, segun su tipo (los de `CFBundleDocumentTypes` en
// `xcode/Soporte/App-Info.plist`). Lo llama `AbrirDesdeFuera`
// (`Raiz/BarraVisorIPad.swift`) con el fichero ya abierto:
//
// - `.excalidraw` (un lienzo de Windows, Android o excalidraw.com): un
//   lienzo NUEVO en «Mensajes guardados» con ese dibujo, abierto en el
//   editor. El lienzo del nucleo es un `.excalidraw`: se comprueba que el
//   nucleo lo lee (`Lienzo.deFichero`) y se pone en el lienzo recien creado.
// - `.xml`: si es un LandXML (Civil 3D), al visor 3D como terreno; si no,
//   a Vista Rapida.
// - `.pixpin` (un proyecto entero): a la lista de proyectos
//   (`Nucleo.importarProyecto`, lo mismo que el envio por codigo). Si ya
//   esta (los tres codigos), se pregunta «Actualizar el que tengo» (con
//   copia de antes) o «Crear como nuevo». Si el nucleo no lo entiende,
//   queda en «Mensajes guardados» como antes.
// - `.pixpin2d` (el dibujo antiguo de la foto anotada): el nucleo de Swift
//   aun no sabe pasarlo a lienzo; queda en «Mensajes guardados» y se va a
//   su mensaje.
// - lo demas (PDF, Office, `.md`, `.html`, planos, modelos, `.rvt`,
//   fotos...): a «Mensajes guardados» y a su visor, como siempre.
//
// Los ficheros que iOS deja en `Documents/Inbox` (copias de «Abrir en…»)
// se borran despues de copiarlos al proyecto.

import Foundation
import PixPinCore
import UIKit

@MainActor
public enum AbrirFicheroDeFuera {
    /// Que se hace con un fichero de fuera, por su extension (y, para un
    /// `.xml`, por su cabeza).
    public enum Destino: Equatable, Sendable {
        case lienzo, terreno, proyecto, dibujoAntiguo, visor
    }

    nonisolated public static func destino(de url: URL, cabeza: Data? = nil) -> Destino {
        switch url.pathExtension.lowercased() {
        case "excalidraw": return .lienzo
        case "pixpin": return .proyecto
        case "pixpin2d": return .dibujoAntiguo
        case "landxml": return .terreno
        case "xml": return esLandXML(cabeza ?? leerCabeza(url)) ? .terreno : .visor
        default: return .visor
        }
    }

    /// Si los primeros bytes de un XML son de un LandXML.
    nonisolated public static func esLandXML(_ cabeza: Data?) -> Bool {
        guard let cabeza, let texto = String(data: cabeza, encoding: .utf8) ?? String(data: cabeza, encoding: .isoLatin1) else {
            return false
        }
        return texto.range(of: "<LandXML", options: .caseInsensitive) != nil
    }

    nonisolated static func leerCabeza(_ url: URL) -> Data? {
        guard let h = try? FileHandle(forReadingFrom: url) else { return nil }
        defer { try? h.close() }
        return try? h.read(upToCount: 4096)
    }

    /// Mete `url` en «Mensajes guardados» (`guardados`) y lo abre donde
    /// toque.
    public static func abrir(_ url: URL, en guardados: String, almacen: Almacen) async {
        defer { borrarSiEsDelBuzonDeEntrada(url) }
        let nav = Navegacion.compartida
        var cabeza: Data?
        if url.pathExtension.lowercased() == "xml" {
            cabeza = await Task.detached(priority: .userInitiated) { AbrirFicheroDeFuera.leerCabeza(url) }.value
        }
        switch destino(de: url, cabeza: cabeza) {
        case .lienzo:
            if await abrirComoLienzo(url, en: guardados, almacen: almacen) { return }
            // Si el nucleo no lo entiende, al menos queda en el chat.
            await adjuntarYAbrir(url, en: guardados, almacen: almacen)
        case .terreno:
            guard let hecho = await adjuntar(url, en: guardados, almacen: almacen) else { return }
            let (m, destino) = hecho
            nav.abrir(.modelo3d(url: destino, titulo: m.nombre))
        case .proyecto:
            if await importarProyecto(url, almacen: almacen) { return }
            // Si el nucleo no lo entiende como proyecto, al menos queda en
            // el chat.
            guard let hecho = await adjuntar(url, en: guardados, almacen: almacen) else { return }
            let (m, _) = hecho
            nav.cerrarVisor()
            nav.irAlChat(guardados, mensaje: m.id)
            CuadrosIPad.avisar("«\(m.nombre)» no se pudo abrir como proyecto de PixPin. Ha quedado en Mensajes guardados.")
        case .dibujoAntiguo:
            guard let hecho = await adjuntar(url, en: guardados, almacen: almacen) else { return }
            let (m, _) = hecho
            nav.cerrarVisor()
            nav.irAlChat(guardados, mensaje: m.id)
        case .visor:
            await adjuntarYAbrir(url, en: guardados, almacen: almacen)
        }
    }

    /// **Un `.pixpin` a la lista de proyectos**. Si ya esta, se pregunta
    /// como en el envio por codigo. `false` si el nucleo no lo entiende (o
    /// no se pudo escribir); `true` si entro o si el usuario cancelo.
    private static func importarProyecto(_ url: URL, almacen: Almacen) async -> Bool {
        let nucleo = almacen.nucleo
        let ruta = url.path
        let leido = await Task.detached(priority: .userInitiated) {
            Result { try nucleo.proyectoQueYaEsta(ruta: ruta) }
        }.value
        guard case let .success(yaEsta) = leido else { return false }
        var comoNuevo = false
        if let suyo = yaEsta {
            guard let eleccion = await preguntarComoNuevo(suyo: suyo) else { return true }
            comoNuevo = eleccion
        }
        let nuevo = comoNuevo
        let hecho = await Task.detached(priority: .userInitiated) {
            Result { try nucleo.importarProyecto(ruta: ruta, comoNuevo: nuevo) }
        }.value
        guard case let .success(ficha) = hecho else { return false }
        await almacen.cargarChats()
        // Que la sincronizacion sola se entere.
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        let nav = Navegacion.compartida
        nav.cerrarVisor()
        nav.irAlChat(ficha.id)
        CuadrosIPad.avisar(yaEsta != nil && !comoNuevo
            ? "«\(ficha.nombre)» puesto al día. Lo de antes queda en «Copias de seguridad»."
            : "«\(ficha.nombre)» está en tu lista de proyectos.")
        return true
    }

    /// «Ya tienes «X» con los mismos códigos»: `false` actualizar el que
    /// tengo, `true` crear como nuevo, `nil` cancelar.
    private static func preguntarComoNuevo(suyo: String) async -> Bool? {
        guard let arriba = CuadrosIPad.arriba(), !(arriba is UIAlertController) else { return false }
        return await withCheckedContinuation { (seguir: CheckedContinuation<Bool?, Never>) in
            let a = UIAlertController(title: "Ya tienes «\(suyo)» con los mismos códigos",
                                      message: "«Actualizar el que tengo» escribe encima de lo tuyo (lo de antes queda en «Copias de seguridad»); «Crear como nuevo» entra aparte, con códigos nuevos.",
                                      preferredStyle: .alert)
            let actualizar = UIAlertAction(title: "Actualizar el que tengo", style: .default) { _ in seguir.resume(returning: false) }
            a.addAction(actualizar)
            a.addAction(UIAlertAction(title: "Crear como nuevo", style: .default) { _ in seguir.resume(returning: true) })
            a.addAction(UIAlertAction(title: "Cancelar", style: .cancel) { _ in seguir.resume(returning: nil) })
            a.preferredAction = actualizar
            arriba.present(a, animated: true)
        }
    }

    /// Lo de siempre: al chat y a su visor.
    private static func adjuntarYAbrir(_ url: URL, en chat: String, almacen: Almacen) async {
        guard let hecho = await adjuntar(url, en: chat, almacen: almacen) else { return }
        let (m, destino) = hecho
        Navegacion.compartida.abrirFichero(destino, titulo: m.nombre, chat: chat, mensaje: m)
    }

    private static func adjuntar(_ url: URL, en chat: String, almacen: Almacen) async -> (MensajeChat, URL)? {
        guard let m = await almacen.adjuntar(urls: [url], en: chat).first,
              let r = m.ruta,
              let destino = await almacen.url(deRuta: r, en: chat) else { return nil }
        return (m, destino)
    }

    /// Un `.excalidraw` como lienzo nuevo del chat. `false` si el nucleo no
    /// lo lee.
    private static func abrirComoLienzo(_ url: URL, en chat: String, almacen: Almacen) async -> Bool {
        let nucleo = almacen.nucleo
        let nombre = url.deletingPathExtension().lastPathComponent
        let ruta = url.path
        // Que el nucleo lo entienda antes de crear nada; y sus bytes.
        let datos: Data? = await Task.detached(priority: .userInitiated) {
            guard (try? Lienzo.deFichero(ruta: ruta)) != nil else { return nil }
            return try? Data(contentsOf: URL(fileURLWithPath: ruta))
        }.value
        guard let datos else { return false }
        let creado: MensajeChat? = await Task.detached(priority: .userInitiated) {
            try? nucleo.crearLienzo(chat: chat, nombre: nombre)
        }.value
        guard let m = creado, let r = m.ruta, let destino = await almacen.url(deRuta: r, en: chat) else { return false }
        let puesto = await Task.detached(priority: .userInitiated) { () -> Bool in
            (try? datos.write(to: destino, options: .atomic)) != nil
        }.value
        guard puesto else { return false }
        // Que se vea en el chat y que la sincronizacion se entere.
        if almacen.mensajes[chat] != nil, !almacen.quedanPosteriores(chat: chat) {
            almacen.anadir([m], en: chat)
        }
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        Navegacion.compartida.abrir(.lienzo(chat: chat, mensaje: m, foto: nil))
        return true
    }

    /// iOS copia lo de «Abrir en…» a `Documents/Inbox`; ya esta en el
    /// proyecto, asi que la copia sobra.
    nonisolated static func borrarSiEsDelBuzonDeEntrada(_ url: URL) {
        guard url.isFileURL, url.standardizedFileURL.path.contains("/Documents/Inbox/") else { return }
        try? FileManager.default.removeItem(at: url)
    }
}
