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
// - `.pixpin` (un proyecto entero) y `.pixpin2d` (el dibujo antiguo de la
//   foto anotada): el nucleo de Swift aun no sabe meterlos en la lista de
//   proyectos ni pasarlos a lienzo; quedan en «Mensajes guardados» y se va
//   a su mensaje, diciendo como traerlo entero (Recibir por Wi‑Fi).
// - lo demas (PDF, Office, `.md`, `.html`, planos, modelos, `.rvt`,
//   fotos...): a «Mensajes guardados» y a su visor, como siempre.
//
// Los ficheros que iOS deja en `Documents/Inbox` (copias de «Abrir en…»)
// se borran despues de copiarlos al proyecto.

import Foundation
import PixPinCore

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
        let cabeza = url.pathExtension.lowercased() == "xml"
            ? await Task.detached(priority: .userInitiated) { leerCabeza(url) }.value
            : nil
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
            guard let hecho = await adjuntar(url, en: guardados, almacen: almacen) else { return }
            let (m, _) = hecho
            nav.cerrarVisor()
            nav.irAlChat(guardados, mensaje: m.id)
            CuadrosIPad.avisar("«\(m.nombre)» es un proyecto entero de PixPin. Ha quedado en Mensajes guardados; para tenerlo en tu lista de proyectos, mándalo desde el PC o el móvil con «Recibir por Wi‑Fi» (Aparatos).")
        case .dibujoAntiguo:
            guard let hecho = await adjuntar(url, en: guardados, almacen: almacen) else { return }
            let (m, _) = hecho
            nav.cerrarVisor()
            nav.irAlChat(guardados, mensaje: m.id)
        case .visor:
            await adjuntarYAbrir(url, en: guardados, almacen: almacen)
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
