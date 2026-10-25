// **Las carpetas de paginas sin abrir el chat** (Windows 22c3ce2 y
// 211a1cd, 25-oct-2026; el Mac `Buscar/CarpetasSinChat.swift`): lo de
// `p carpeta` de Flow, que en el iPad es «carpeta …» en Buscar, y el pedido
// `carpeta_paginas` (abrir / nueva / pagina). Abrir una es su hoja
// (`VistaCarpeta`, encima de todo); la pagina siguiente se abre en el lienzo
// a pantalla completa; una nueva se crea en el chat (o en «Mensajes
// guardados») y se abre. La de una clase grabada se abre, pero no se le
// anaden paginas. Lo que se lee y se crea es del nucleo (`carpetas_flow.rs`):
// son mensajes del chat, asi que viajan al sincronizar como en el Mac.

import Foundation
import PixPinCore

@MainActor
enum CarpetasSinChat {
    /// La hoja de la carpeta, con sus paginas en orden.
    static func abrir(chat: String, id: String, nombre: String, almacen: Almacen) {
        VistaCarpeta.abrir(chat: chat, carpeta: id, almacen: almacen)
    }

    /// **Su pagina siguiente** (la plantilla de la ultima, vacia), abierta
    /// en el lienzo. Devuelve si se pudo.
    @discardableResult
    static func nuevaPagina(chat: String, carpeta: String, almacen: Almacen) async -> Bool {
        let nucleo = almacen.nucleo
        let hecho = await Task.detached(priority: .userInitiated) { () -> Result<PaginaNuevaDeCarpeta, Error> in
            Result { try nucleo.paginaSiguienteDeCarpeta(chat: chat, carpeta: carpeta) }
        }.value
        guard case let .success(n) = hecho else { return false }
        await almacen.alFinal(n.lienzo, en: chat)
        almacen.sustituir(n.carpeta, en: chat)
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        Navegacion.compartida.abrir(.lienzo(chat: chat, mensaje: n.lienzo, foto: nil))
        return true
    }

    /// **Una carpeta nueva** llamada `nombre` (o «Cuaderno», sin repetir)
    /// en `chat`, o en «Mensajes guardados» sin chat, y su hoja.
    @discardableResult
    static func nueva(nombre: String, en chat: String?, almacen: Almacen) async -> Bool {
        let nucleo = almacen.nucleo
        let hecho = await Task.detached(priority: .userInitiated) { () -> Result<(String, MensajeChat), Error> in
            Result {
                let destino = try chat ?? nucleo.chatGuardados().id
                return (destino, try nucleo.crearCarpetaPorNombre(chat: destino, nombre: nombre))
            }
        }.value
        guard case let .success((destino, m)) = hecho else { return false }
        await almacen.alFinal(m, en: destino)
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        abrir(chat: destino, id: m.id, nombre: m.nombre, almacen: almacen)
        return true
    }

    /// **El pedido `carpeta_paginas`**: `hacer` es `abrir`, `nueva` o
    /// `pagina`; `carpeta` el `id` de su mensaje (si ya no esta, se busca
    /// por `nombre`). Sin `proyecto`, «Mensajes guardados». Devuelve el
    /// aviso si no se pudo.
    static func pedido(hacer: String, proyecto: String?, carpeta: String?, nombre: String,
                       almacen: Almacen) async -> String? {
        let nucleo = almacen.nucleo
        if hacer == "nueva" {
            return await nueva(nombre: nombre, en: proyecto, almacen: almacen) ? nil : TextosCarpeta.noSePudo
        }
        guard hacer == "abrir" || hacer == "pagina" else { return "No se entiende «\(hacer)»" }
        let hallada = await Task.detached(priority: .userInitiated) { () -> (String, MensajeChat)? in
            guard let chat = try? proyecto ?? nucleo.chatGuardados().id,
                  let m = (try? nucleo.buscarCarpetaDePaginas(chat: chat, id: carpeta, nombre: nombre)) ?? nil
            else { return nil }
            return (chat, m)
        }.value
        guard let hallada else { return TextosCarpeta.noEsta }
        let (chat, m) = hallada
        if hacer == "pagina" {
            return await nuevaPagina(chat: chat, carpeta: m.id, almacen: almacen) ? nil : TextosCarpeta.noSePudo
        }
        abrir(chat: chat, id: m.id, nombre: m.nombre, almacen: almacen)
        return nil
    }
}
