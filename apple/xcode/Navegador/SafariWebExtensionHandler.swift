// **La extension de Safari del iPad** (`extensiones/navegador` de Windows,
// `apple/extension-navegador` del Mac): la parte nativa. El menu del boton
// (`Recursos/menu.js`) le manda un mensaje con lo que se eligio y lo de la
// pagina; aqui no hay nucleo ni PixPinKit: se deja en el buzon del App
// Group (`BuzonCompartido`, el mismo de la extension de Compartir) como una
// entrega con su enlace `pixpinmax://` (`PedidoDelNavegador`) o con la foto
// de la pagina, y se avisa a la app (aviso de Darwin). PixPin lo hace al
// abrirse, o en el acto si esta delante (Split View, Slide Over).

import Foundation
import SafariServices

@objc(SafariWebExtensionHandler)
final class SafariWebExtensionHandler: NSObject, NSExtensionRequestHandling {
    func beginRequest(with context: NSExtensionContext) {
        let item = context.inputItems.first as? NSExtensionItem
        let mensaje = item?.userInfo?[SFExtensionMessageKey] as? [String: Any] ?? [:]
        let respuesta = PuenteDelNavegador.atender(mensaje)
        let salida = NSExtensionItem()
        salida.userInfo = [SFExtensionMessageKey: respuesta]
        context.completeRequest(returningItems: [salida], completionHandler: nil)
    }
}

/// Lo que se hace con cada mensaje.
enum PuenteDelNavegador {
    static func atender(_ m: [String: Any]) -> [String: Any] {
        func fallo(_ texto: String) -> [String: Any] { ["ok": false, "error": texto] }
        guard let accion = m["accion"] as? String, PedidoDelNavegador.acciones.contains(accion) else {
            return fallo("PixPin no entiende esta acción.")
        }
        guard let raiz = BuzonCompartido.raiz() else {
            return fallo("Esta copia de PixPin no está firmada con su App Group: no hay buzón.")
        }
        let texto = m["texto"] as? String
        let url = m["url"] as? String
        let titulo = m["titulo"] as? String
        // A «Mensajes guardados» (lo que la app tenga apuntado; si no, la app
        // lo pone ahi al recogerlo).
        let guardados = BuzonCompartido.leerProyectos(en: raiz).first(where: \.esGuardados)
        let ahora = Int64(Date().timeIntervalSince1970 * 1000)
        do {
            let carpeta = try BuzonCompartido.nuevaEntrega(en: raiz)
            var entrega = EntregaCompartida(chat: guardados?.id ?? "", nombreDelChat: guardados?.nombre ?? "Mensajes guardados",
                                            comentario: "", textos: [], ficheros: [], creada: ahora)
            if accion == "captura_pagina" {
                guard let imagen = m["imagen"] as? [String: Any], let b64 = imagen["base64"] as? String,
                      let datos = Data(base64Encoded: b64) else {
                    carpeta.tirar()
                    return fallo("No llegó la foto de la página.")
                }
                let ext = (imagen["tipo"] as? String) == "image/jpeg" ? "jpg" : "png"
                let base = (titulo ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                let nombre = try carpeta.anadir(datos: datos, nombre: (base.isEmpty ? "Página" : base) + "." + ext)
                entrega.ficheros = [nombre]
                entrega.textos = [titulo, url].compactMap { $0 }
            } else {
                guard let enlace = PedidoDelNavegador.enlace(accion: accion, texto: texto, url: url, titulo: titulo,
                                                             pregunta: m["pregunta"] as? String) else {
                    carpeta.tirar()
                    return fallo("No hay texto seleccionado.")
                }
                entrega.enlace = enlace
            }
            try carpeta.cerrar(entrega)
        } catch {
            return fallo("No se pudo dejar en el buzón de PixPin: \(error.localizedDescription)")
        }
        CFNotificationCenterPostNotification(CFNotificationCenterGetDarwinNotifyCenter(),
                                             CFNotificationName(BuzonCompartido.avisoDeEntrega as CFString),
                                             nil, nil, true)
        return ["ok": true, "aviso": "Hecho: lo verás en PixPin al abrirla."]
    }
}
