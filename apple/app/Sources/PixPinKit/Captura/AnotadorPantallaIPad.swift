// **El anotador de pantalla** (`anotador_al_chat.rs` y `overlay.rs` de
// Windows, `Captura/AnotadorPantalla.swift` del Mac): iOS no deja pintar
// encima de otras apps, asi que se anota encima de PixPin. Se fotografia la
// ventana tal como se ve, se abre el lienzo con todas sus herramientas
// encima de esa foto (de fondo fijo) y «Guardar» la deja en «Mensajes
// guardados» como Windows: la foto de lo que habia debajo como mensaje
// IMAGEN y su lienzo `foto-<id>` con la tinta encima, editable
// (`Nucleo.pantallaAnotadaAlChat`). Sin nada dibujado no se guarda nada.
//
// El mundo del lienzo mide lo que la foto (pixeles): la tinta va tal cual,
// con origen 0 y escala 1.

import Foundation
import PixPinCore
import SwiftUI
import UIKit

@MainActor
enum AnotadorPantallaIPad {
    /// Fotografia PixPin y abre el lienzo encima.
    static func empezar() {
        guard let foto = CapturadorIPad.compartido.fotoDeLaApp()?.cgImage,
              let almacen = CapturadorIPad.compartido.almacen else { return avisarFalloPin() }
        let nucleo = almacen.nucleo
        let carpeta = FileManager.default.temporaryDirectory
            .appendingPathComponent("pixpin-anotador-\(UUID().uuidString)", isDirectory: true)
        let png = carpeta.appendingPathComponent("pantalla.png")
        let ruta = carpeta.appendingPathComponent("pantalla.excalidraw")
        func limpiar() {
            _ = Task.detached(priority: .utility) { try? FileManager.default.removeItem(at: carpeta) }
        }
        _ = Task<Void, Never> {
            let editor = await Task.detached(priority: .userInitiated) { () -> EditorLienzo? in
                try? FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
                guard let datos = PintorDeHojas.png(foto), (try? datos.write(to: png)) != nil else { return nil }
                return try? EditorLienzo.deFichero(ruta: ruta.path, crear: true)
            }.value
            guard let editor, let fondo = FondoLienzo(.imagen(png)) else {
                limpiar()
                return avisarFalloPin()
            }
            let modelo = ModeloEditorLienzo(editor: editor, titulo: "Pantalla anotada", fondo: fondo)
            modelo.sitioHerramientas = "pantalla"
            modelo.elegir("lapiz")
            let hecho = ArribaUIKit.presentar(estilo: .fullScreen) { cerrar in
                VistaAnotarConLienzo(modelo: modelo, titulo: "Anotar la pantalla", icono: "pencil.tip.crop.circle",
                                     textoListo: "Guardar", listo: {
                    cerrar()
                    guardar(editor: editor, foto: png, nucleo: nucleo, limpiar: limpiar)
                }, cancelar: {
                    cerrar()
                    limpiar()
                })
            }
            if hecho == nil { limpiar() }
        }
    }

    /// Lo anotado (si hay algo) a «Mensajes guardados».
    private static func guardar(editor: EditorLienzo, foto: URL, nucleo: Nucleo, limpiar: @escaping () -> Void) {
        _ = editor.terminarTexto()
        guard editor.tintaDePantalla().huella != nil else {
            limpiar()
            return
        }
        _ = Task<Void, Never> {
            let hecho = await Task.detached(priority: .userInitiated) { () -> Bool in
                (try? nucleo.pantallaAnotadaAlChat(foto: foto.path, editor: editor, origenX: 0, origenY: 0,
                                                   escala: 1)) != nil
            }.value
            limpiar()
            if hecho {
                UINotificationFeedbackGenerator().notificationOccurred(.success)
                NotificationCenter.default.post(name: .pixpinTrajoCambios, object: nil)
                NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
                CuadrosIPad.avisar("Guardada en «Mensajes guardados».")
            } else {
                avisarFalloPin()
            }
        }
    }
}
