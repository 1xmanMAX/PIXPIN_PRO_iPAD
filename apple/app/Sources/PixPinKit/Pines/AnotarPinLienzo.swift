// **Anotar un pin con las herramientas del lienzo** (`Anotar` de
// `crates/pixpin-pin` en Windows: el motor del lienzo sobre la foto del
// pin). La foto va de **fondo fijo** (`FondoLienzo`: no se elige ni se
// mueve) en un lienzo de paso, con la barra y el panel de estilo de
// siempre (lapiz, formas, flechas, texto, colores, mosaico...). «Listo»
// pinta lo anotado encima de la foto, a su tamano de pixeles (el mundo del
// lienzo mide lo que la foto), y la devuelve al pin; lo que se salga de la
// foto no entra. El lienzo de paso se borra al cerrar.

import CoreGraphics
import Foundation
import PixPinCore
import SwiftUI
import UIKit

@MainActor
enum AnotarPinConLienzo {
    /// Abre el lienzo sobre `imagen`; `listo` recibe la foto anotada. Si el
    /// lienzo no se pudo abrir, `sinLienzo` (el editor de captura de antes).
    static func abrir(_ imagen: CGImage, listo: @escaping (CGImage) -> Void, sinLienzo: @escaping () -> Void) {
        let carpeta = FileManager.default.temporaryDirectory
            .appendingPathComponent("pixpin-anotar-pin-\(UUID().uuidString)", isDirectory: true)
        let foto = carpeta.appendingPathComponent("pin.png")
        let ruta = carpeta.appendingPathComponent("pin.excalidraw")
        _ = Task<Void, Never> {
            let editor = await Task.detached(priority: .userInitiated) { () -> EditorLienzo? in
                try? FileManager.default.createDirectory(at: carpeta, withIntermediateDirectories: true)
                guard let png = PintorDeHojas.png(imagen), (try? png.write(to: foto)) != nil else { return nil }
                return try? EditorLienzo.deFichero(ruta: ruta.path, crear: true)
            }.value
            guard let editor, let fondo = FondoLienzo(.imagen(foto)) else {
                try? FileManager.default.removeItem(at: carpeta)
                return sinLienzo()
            }
            let modelo = ModeloEditorLienzo(editor: editor, titulo: "Anotar el pin", fondo: fondo)
            modelo.sitioHerramientas = "pin"
            modelo.elegir("flecha")
            func limpiar() {
                _ = Task.detached(priority: .utility) { try? FileManager.default.removeItem(at: carpeta) }
            }
            let hecho = ArribaUIKit.presentar(estilo: .fullScreen) { cerrar in
                VistaAnotarConLienzo(modelo: modelo, titulo: "Anotar el pin", icono: "pin", listo: {
                    if let anotada = quemar(imagen, modelo: modelo) { listo(anotada) }
                    cerrar()
                    limpiar()
                }, cancelar: {
                    cerrar()
                    limpiar()
                })
            }
            if hecho == nil {
                limpiar()
                sinLienzo()
            }
        }
    }

    /// **La foto con lo anotado encima**, a sus pixeles (el mundo del lienzo
    /// es la foto: (0, 0)-(ancho, alto)).
    static func quemar(_ imagen: CGImage, modelo: ModeloEditorLienzo) -> CGImage? {
        _ = modelo.editor.terminarTexto()
        guard let leido = try? leerLienzo(modelo.editor.plano()) else { return imagen }
        return quemar(imagen, leido: leido, fotos: modelo.imagenes(escala: 1))
    }

    /// Lo mismo con lo ya leido (puro: se prueba sin editor).
    nonisolated static func quemar(_ imagen: CGImage, leido: LienzoLeido,
                                   fotos: @escaping (UInt64) -> CGImage? = { _ in nil }) -> CGImage? {
        let w = imagen.width, h = imagen.height
        guard w > 0, h > 0, let ctx = MapaLienzo.mapa(w, h) else { return nil }
        let caja = CGRect(x: 0, y: 0, width: w, height: h)
        ctx.draw(imagen, in: caja)
        // Y hacia abajo, como el mundo del lienzo.
        ctx.translateBy(x: 0, y: CGFloat(h))
        ctx.scaleBy(x: 1, y: -1)
        ctx.clip(to: caja)
        PintorLienzo(lienzo: leido, imagen: fotos).pintar(en: ctx, limites: caja)
        return ctx.makeImage()
    }
}

/// El lienzo a pantalla completa con su barra, y arriba «Cancelar» y
/// «Listo» (anotar un pin, anotar la pantalla).
struct VistaAnotarConLienzo: View {
    @ObservedObject var modelo: ModeloEditorLienzo
    var titulo = "Anotar"
    var icono = "pencil.tip.crop.circle"
    var textoListo = "Listo"
    let listo: () -> Void
    let cancelar: () -> Void

    var body: some View {
        ZStack(alignment: .top) {
            EditorLienzoVistaIPad(modelo: modelo)
            HStack(spacing: 12) {
                Button("Cancelar", action: cancelar)
                    .keyboardShortcut(.cancelAction)
                Spacer(minLength: 8)
                Label(titulo, systemImage: icono)
                    .font(.subheadline.weight(.semibold))
                Spacer(minLength: 8)
                Button(textoListo, action: listo)
                    .fontWeight(.semibold)
                    .keyboardShortcut(.defaultAction)
            }
            .buttonStyle(.borderless)
            .padding(.horizontal, 16)
            .padding(.vertical, 9)
            .background(.regularMaterial, in: Capsule())
            .overlay(Capsule().strokeBorder(.separator.opacity(0.5)))
            .padding(.horizontal, 12)
            .padding(.top, 6)
        }
        .environmentObject(Navegacion.compartida)
        .onAppear {
            // La barra «salir» del lienzo es cancelar.
            modelo.alCerrar = cancelar
        }
    }
}
