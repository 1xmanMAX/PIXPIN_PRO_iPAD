// Las cuentas de un pin, sin ventanas: cuanto crece con la rueda, hasta
// donde baja la opacidad, de que tamano nace y a que tamano se lee la foto.
// Todo puro para poder probarlo en la CI (`PinesTests`).
//
// Las cifras son las de `pixpin-pin` en Windows: opacidad del 20 al 100 %,
// el menu con 100, 90 ... 30 %, y el pin nace a su tamano natural sin pasar
// del 80 % de la pantalla.

import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

enum ReglasPin {
    /// Las opacidades del menu, en por ciento (`menu::OPACIDADES` de Windows).
    static let opacidades: [Int] = [100, 90, 80, 70, 60, 50, 40, 30]
    /// La opacidad mas baja: por debajo el pin se pierde de vista.
    static let opacidadMinima: CGFloat = 0.2
    /// Cuanto cambia la opacidad cada muesca de una rueda de raton.
    static let pasoOpacidad: CGFloat = 0.05
    /// El pin no crece mas de ocho veces su tamano de nacer.
    static let escalaMaxima: CGFloat = 8
    /// Ni encoge por debajo de esto (en puntos) por su lado corto: mas
    /// pequeno ya no se puede agarrar.
    static let ladoMinimo: CGFloat = 32
    /// Lo mas que ocupa un pin al nacer, de la zona visible de la pantalla.
    static let fraccionPantalla: CGFloat = 0.8
    /// Las miniaturas nunca bajan de aqui (pantallas raras o sin pantalla).
    static let miniaturaMinima = 256

    /// `valor` dentro de `[menor, mayor]`; si el intervalo viene al reves,
    /// gana `menor`.
    static func limitar(_ valor: CGFloat, _ menor: CGFloat, _ mayor: CGFloat) -> CGFloat {
        guard mayor >= menor else { return menor }
        return min(max(valor, menor), mayor)
    }

    // MARK: Zoom

    /// La escala permitida para un pin que nacio con `base`: hasta 8x, y
    /// hacia abajo hasta que su lado corto mida `ladoMinimo` (o 1x si ya
    /// nacio mas pequeno que eso).
    static func escalaLimitada(_ escala: CGFloat, base: CGSize) -> CGFloat {
        guard escala.isFinite, escala > 0 else { return 1 }
        let corto = max(1, min(base.width, base.height))
        let minima = min(1, ladoMinimo / corto)
        return limitar(escala, minima, escalaMaxima)
    }

    /// Cuanto multiplica una vuelta de rueda. Con `precisa` (trackpad, ratones
    /// de Apple) el delta viene en puntos; con una rueda normal, en muescas.
    /// Exponencial: subir y bajar lo mismo deja el pin como estaba.
    static func factorZoom(deltaY: CGFloat, precisa: Bool) -> CGFloat {
        guard deltaY.isFinite else { return 1 }
        let paso = precisa ? deltaY * 0.01 : deltaY * 0.1
        return exp(limitar(paso, -0.5, 0.5))
    }

    /// El marco tras cambiar a `tamano`, dejando quieto bajo el raton el punto
    /// `ancla` (coordenadas de pantalla): se agranda «hacia donde se mira».
    static func marcoTrasZoom(_ marco: CGRect, ancla: CGPoint, tamano: CGSize) -> CGRect {
        let rx = marco.width > 0 ? limitar((ancla.x - marco.minX) / marco.width, 0, 1) : 0.5
        let ry = marco.height > 0 ? limitar((ancla.y - marco.minY) / marco.height, 0, 1) : 0.5
        let x = ancla.x - rx * tamano.width
        let y = ancla.y - ry * tamano.height
        return CGRect(x: x.rounded(), y: y.rounded(),
                      width: tamano.width.rounded(), height: tamano.height.rounded())
    }

    // MARK: Opacidad

    static func limitarOpacidad(_ opacidad: CGFloat) -> CGFloat {
        guard opacidad.isFinite else { return 1 }
        return limitar(opacidad, opacidadMinima, 1)
    }

    /// La opacidad tras Mayusculas (u Opcion) + rueda: un 5 % por muesca, o proporcional al
    /// gesto en un trackpad. Hacia arriba, mas opaco.
    static func opacidadTrasRueda(_ actual: CGFloat, deltaY: CGFloat, precisa: Bool) -> CGFloat {
        let cambio: CGFloat
        if precisa {
            cambio = deltaY * 0.005
        } else {
            cambio = deltaY > 0 ? pasoOpacidad : (deltaY < 0 ? -pasoOpacidad : 0)
        }
        return limitarOpacidad(actual + cambio)
    }

    /// El giro de la rueda que cuenta para la opacidad. Con Mayusculas, una
    /// rueda de raton normal manda su giro en X (es el desplazamiento
    /// horizontal de macOS): se toma el eje que traiga algo.
    static func deltaDeRueda(x: CGFloat, y: CGFloat) -> CGFloat {
        if y != 0, y.isFinite { return y }
        return x.isFinite ? x : 0
    }

    /// La opacidad en por ciento (para marcar la del menu).
    static func porcentaje(_ opacidad: CGFloat) -> Int {
        Int((limitarOpacidad(opacidad) * 100).rounded())
    }

    // MARK: Tamanos

    /// El lado mayor, en pixeles, al que se lee una foto para pinearla: el de
    /// la pantalla en pixeles reales. Mas no se veria y solo gastaria RAM.
    static func ladoMiniatura(pantalla: CGSize, escala: CGFloat) -> Int {
        let lado = max(pantalla.width, pantalla.height) * max(escala, 1)
        guard lado.isFinite else { return miniaturaMinima }
        return max(miniaturaMinima, Int(lado.rounded(.up)))
    }

    /// El tamano (en puntos) con el que nace un pin de `pixeles`: un pixel de
    /// la foto por pixel de la pantalla, sin pasar del 80 % de lo visible y
    /// sin bajar de `ladoMinimo` por el lado corto. Mantiene la proporcion.
    static func tamanoInicial(pixeles: CGSize, escala: CGFloat, visible: CGSize) -> CGSize {
        let s = max(escala, 1)
        var ancho = max(1, pixeles.width) / s
        var alto = max(1, pixeles.height) / s
        let cabe = min(1, visible.width * fraccionPantalla / ancho, visible.height * fraccionPantalla / alto)
        if cabe.isFinite, cabe > 0 {
            ancho *= cabe
            alto *= cabe
        }
        let crece = max(1, ladoMinimo / min(ancho, alto))
        ancho *= crece
        alto *= crece
        return CGSize(width: ancho.rounded(), height: alto.rounded())
    }

    /// Donde nace: centrado en `centro` (el raton) y metido en `visible`.
    static func marcoInicial(tamano: CGSize, centro: CGPoint, visible: CGRect) -> CGRect {
        let x = limitar(centro.x - tamano.width / 2, visible.minX, visible.maxX - tamano.width)
        let y = limitar(centro.y - tamano.height / 2, visible.minY, visible.maxY - tamano.height)
        return CGRect(x: x.rounded(), y: y.rounded(), width: tamano.width, height: tamano.height)
    }
}

/// Que clase de pin hace falta para un fichero.
enum TipoContenido: Equatable {
    case imagen, pdf, video, audio

    /// Por el tipo que dice el disco y, si el fichero no existe o no lo dice,
    /// por su extension.
    static func de(_ url: URL) -> TipoContenido? {
        let delDisco = (try? url.resourceValues(forKeys: [.contentTypeKey]))?.contentType
        return de(delDisco ?? UTType(filenameExtension: url.pathExtension))
    }

    static func de(_ tipo: UTType?) -> TipoContenido? {
        guard let tipo else { return nil }
        if tipo.conforms(to: .pdf) { return .pdf }
        if tipo.conforms(to: .image) { return .imagen }
        if tipo.conforms(to: .movie) { return .video }
        if tipo.conforms(to: .audio) { return .audio }
        if tipo.conforms(to: .audiovisualContent) { return .video }
        return nil
    }
}

/// Leer fotos reducidas con ImageIO (§4bis.2 y .4 del plan): la miniatura se
/// decodifica directamente al tamano pedido (JPEG y HEIC la sacan de su
/// propia escala reducida, por hardware), nunca la foto entera.
enum Miniatura {
    /// Hace `leer` en una cola de fondo (`.userInitiated`: el usuario espera
    /// el pin) y devuelve su resultado. Con una cola y no con `Task.detached`
    /// porque `CGImage` no es `Sendable`.
    static func fueraDelPrincipal(_ leer: @escaping @Sendable () -> CGImage?) async -> CGImage? {
        await withCheckedContinuation { (continuacion: CheckedContinuation<CGImage?, Never>) in
            DispatchQueue.global(qos: .userInitiated).async {
                continuacion.resume(returning: leer())
            }
        }
    }

    static func de(url: URL, ladoMaximo: Int) -> CGImage? {
        let opciones = [kCGImageSourceShouldCache: false] as CFDictionary
        guard let fuente = CGImageSourceCreateWithURL(url as CFURL, opciones) else { return nil }
        return de(fuente: fuente, ladoMaximo: ladoMaximo)
    }

    static func de(datos: Data, ladoMaximo: Int) -> CGImage? {
        let opciones = [kCGImageSourceShouldCache: false] as CFDictionary
        guard let fuente = CGImageSourceCreateWithData(datos as CFData, opciones) else { return nil }
        return de(fuente: fuente, ladoMaximo: ladoMaximo)
    }

    static func de(fuente: CGImageSource, ladoMaximo: Int) -> CGImage? {
        guard CGImageSourceGetCount(fuente) > 0 else { return nil }
        let opciones: [CFString: Any] = [
            // Siempre de la foto (la miniatura incrustada suele ser diminuta).
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            // Con la orientacion EXIF ya aplicada.
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: max(1, ladoMaximo),
        ]
        return CGImageSourceCreateThumbnailAtIndex(fuente, 0, opciones as CFDictionary)
    }

    /// Una imagen ya en memoria (una captura) que pase de `ladoMaximo` se
    /// redibuja reducida una vez; si cabe, se devuelve la misma sin copiar.
    static func reducir(_ imagen: CGImage, ladoMaximo: Int) -> CGImage {
        let lado = max(imagen.width, imagen.height)
        guard ladoMaximo > 0, lado > ladoMaximo else { return imagen }
        let f = Double(ladoMaximo) / Double(lado)
        let ancho = max(1, Int((Double(imagen.width) * f).rounded()))
        let alto = max(1, Int((Double(imagen.height) * f).rounded()))
        guard let espacio = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: ancho, height: alto, bitsPerComponent: 8,
                                  bytesPerRow: 0, space: espacio,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue
                                      | CGBitmapInfo.byteOrder32Little.rawValue)
        else { return imagen }
        ctx.interpolationQuality = .high
        ctx.draw(imagen, in: CGRect(x: 0, y: 0, width: ancho, height: alto))
        return ctx.makeImage() ?? imagen
    }
}
