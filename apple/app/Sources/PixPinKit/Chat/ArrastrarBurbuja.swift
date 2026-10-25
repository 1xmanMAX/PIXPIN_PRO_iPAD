// **Arrastrar una burbuja a otra app** (la «ventanita de salida» de Windows,
// `salida.rs`, y `Compartir/Salida.swift` del Mac): en el iPad, con Split
// View, Slide Over o Stage Manager, se mantiene pulsada una burbuja y se
// lleva a Mail, Mensajes, WhatsApp, Archivos, Notas... Es la alternativa de
// iOS a «arrastrar a WhatsApp».
//
// Lo que viaja: el fichero del mensaje tal cual (la foto, el PDF, la voz);
// una foto anotada, con lo dibujado (PNG); un lienzo o una hoja de PDF,
// como PNG (hecho al soltar, con lo de la hoja de compartir); una nota, su
// texto. Lleva tambien una marca propia para que soltarla en el mismo chat
// no la adjunte otra vez.

import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

@MainActor
enum ArrastrarBurbuja {
    /// La marca de lo que sale de una burbuja (solo dentro de PixPin).
    nonisolated static let marca = "com.pixpinmax.burbuja"

    /// Si lo soltado viene de una burbuja de PixPin (no se adjunta otra vez).
    nonisolated static func esDeUnaBurbuja(_ proveedores: [NSItemProvider]) -> Bool {
        proveedores.contains { $0.registeredTypeIdentifiers.contains(marca) }
    }

    /// **Lo que lleva el arrastre** de `m`.
    static func proveedor(_ m: MensajeChat, chat: String, almacen: Almacen) -> NSItemProvider {
        let p: NSItemProvider
        let fichero = m.ruta.flatMap { almacen.urlResuelta(deRuta: $0, en: chat) }
            .flatMap { FileManager.default.fileExists(atPath: $0.path) ? $0 : nil }
        let nombre = TextosCompartir.sinExtension(TextosCompartir.nombreDeFila(nombre: m.nombre, resumen: m.resumen))
        let anotada = m.clase == "IMAGEN" && FotosAnotadas.compartidas.estaAnotada(m, chat: chat) != false
        if anotada {
            // Puede estar anotada: el PNG se hace al soltar.
            p = NSItemProvider()
            png(en: p, m, chat: chat, almacen: almacen, original: fichero)
            p.suggestedName = nombre
        } else if let fichero, m.clase != "DIBUJO", m.clase != "PAGINA" {
            p = NSItemProvider(contentsOf: fichero) ?? NSItemProvider()
            p.suggestedName = fichero.lastPathComponent
        } else if m.clase == "DIBUJO" || m.clase == "PAGINA" {
            p = NSItemProvider()
            png(en: p, m, chat: chat, almacen: almacen, original: nil)
            p.suggestedName = nombre
        } else {
            let texto = m.texto.trimmingCharacters(in: .whitespacesAndNewlines)
            p = NSItemProvider(object: (texto.isEmpty ? m.resumen : texto) as NSString)
        }
        p.registerDataRepresentation(forTypeIdentifier: marca, visibility: .ownProcess) { listo in
            listo(Data(m.id.utf8), nil)
            return nil
        }
        return p
    }

    /// Registra un PNG que se hace al soltar: la primera pagina de lo que
    /// daria la hoja de compartir (la foto con lo anotado, el lienzo
    /// entero...). Sin nada que pintar, el original.
    private static func png(en p: NSItemProvider, _ m: MensajeChat, chat: String, almacen: Almacen,
                            original: URL?) {
        p.registerDataRepresentation(forTypeIdentifier: UTType.png.identifier, visibility: .all) { listo in
            Task { @MainActor in
                let preparado = await PrepararCompartir.preparar(
                    .mensajes(chat: chat, titulo: m.nombre, [m]), almacen: almacen)
                let carpeta = FileManager.default.temporaryDirectory
                    .appendingPathComponent("pixpin-arrastre-\(UUID().uuidString)", isDirectory: true)
                defer { try? FileManager.default.removeItem(at: carpeta) }
                let marcadas = preparado.compartible().marcadas
                let primera = preparado.piezas.first { marcadas?.contains($0.pagina.clave) ?? true }
                if let clave = primera?.pagina.clave,
                   let url = try? await preparado.generar(IdFormato.png, claves: [clave], en: carpeta).first,
                   let datos = try? Data(contentsOf: url) {
                    listo(datos, nil)
                } else if let original, let datos = try? Data(contentsOf: original), let png = pngDe(datos) {
                    listo(png, nil)
                } else {
                    listo(nil, ErrorCompartir.nada)
                }
            }
            return nil
        }
    }
}
