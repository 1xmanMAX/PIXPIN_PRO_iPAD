// Las imagenes que entran al lienzo (la foto con la que nace, ⌘V, soltarla
// encima, el boton de imagen): de unos bytes o un fichero a un PNG para
// `EditorLienzo.pegarImagen`, que lo guarda como Windows y el movil
// (`imagenes/<fileId>` del chat y su entrada en `files`).
//
// Port del `ImagenPegadaLienzo.swift` del Mac sin el portapapeles de AppKit:
// en el iPad lo del portapapeles y lo soltado llega ya como `Data` o `URL`.
//
// Rendimiento (§4bis): lo hace ImageIO, fuera del hilo principal. Un PNG que
// ya cabe pasa TAL CUAL, sin decodificar; lo demas (JPEG, HEIC de la camara)
// se reduce al leer (lado mayor `ladoMaximo`, girado segun su EXIF) y se
// escribe en PNG.

import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

public enum ImagenPegadaLienzo {
    /// El lado mayor que se deja a una imagen pegada (`LADO_MAXIMO` de Rust).
    public static let ladoMaximo = 4096

    /// De donde sale la imagen. Leer va despues, fuera del hilo principal.
    public enum Origen: Sendable {
        case fichero(URL)
        case datos(Data)
    }

    /// Los bytes de una imagen (PNG, JPEG, HEIC...) como PNG listo para
    /// pegar. `nil` si no es una imagen.
    public static func png(de datos: Data) -> Data? {
        guard let fuente = CGImageSourceCreateWithData(datos as CFData, nil),
              CGImageSourceGetCount(fuente) > 0 else { return nil }
        let props = CGImageSourceCopyPropertiesAtIndex(fuente, 0, nil) as? [CFString: Any]
        let ancho = (props?[kCGImagePropertyPixelWidth] as? Int) ?? 0
        let alto = (props?[kCGImagePropertyPixelHeight] as? Int) ?? 0
        let orientacion = (props?[kCGImagePropertyOrientation] as? Int) ?? 1
        let tipo = CGImageSourceGetType(fuente) as String?
        // Ya es un PNG que cabe y esta derecho: sin tocarlo.
        if tipo == UTType.png.identifier, ancho > 0, alto > 0,
           max(ancho, alto) <= ladoMaximo, orientacion == 1 {
            return datos
        }
        let opciones: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: min(max(ancho, alto, 1), ladoMaximo),
        ]
        guard let imagen = CGImageSourceCreateThumbnailAtIndex(fuente, 0, opciones as CFDictionary) else {
            return nil
        }
        return png(de: imagen)
    }

    /// Una imagen ya decodificada, en PNG.
    public static func png(de imagen: CGImage) -> Data? {
        let salida = NSMutableData()
        guard let destino = CGImageDestinationCreateWithData(salida as CFMutableData,
                                                             UTType.png.identifier as CFString, 1, nil)
        else { return nil }
        CGImageDestinationAddImage(destino, imagen, nil)
        guard CGImageDestinationFinalize(destino) else { return nil }
        return salida as Data
    }

    /// El PNG de un origen (lee el fichero mapeado, sin copiarlo a memoria).
    public static func png(de origen: Origen) -> Data? {
        switch origen {
        case let .fichero(url):
            let accedido = url.startAccessingSecurityScopedResource()
            defer { if accedido { url.stopAccessingSecurityScopedResource() } }
            guard let datos = try? Data(contentsOf: url, options: .alwaysMapped) else { return nil }
            return png(de: datos)
        case let .datos(datos):
            return png(de: datos)
        }
    }

    /// El tamano en pixeles de una imagen (derecha segun su EXIF), sin
    /// decodificarla. `nil` si no es una imagen.
    public static func tamano(de png: Data) -> CGSize? {
        guard let fuente = CGImageSourceCreateWithData(png as CFData, nil),
              let props = CGImageSourceCopyPropertiesAtIndex(fuente, 0, nil) as? [CFString: Any],
              let w = props[kCGImagePropertyPixelWidth] as? Int,
              let h = props[kCGImagePropertyPixelHeight] as? Int, w > 0, h > 0 else { return nil }
        let o = (props[kCGImagePropertyOrientation] as? Int) ?? 1
        return o >= 5 ? CGSize(width: h, height: w) : CGSize(width: w, height: h)
    }
}
