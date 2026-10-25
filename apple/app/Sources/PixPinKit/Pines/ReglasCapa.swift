// Las cuentas de la capa de pines del iPad, sin pantalla: donde nace un pin,
// como se queda dentro de la ventana al cambiarla de tamano (Stage Manager,
// girar el iPad), a que cuarto de vuelta se pega un giro con dos dedos y
// como se pasa un pin al `PinGuardado` del almacen y de vuelta.
//
// En el iPad el «escritorio» es la ventana de PixPin: `x` e `y` del almacen
// son puntos desde arriba a la izquierda de la capa (como Windows, hacia
// abajo). Todo puro para probarlo en la CI (`PinesIPadTests`).

import CoreGraphics
import Foundation

enum ReglasCapa {
    /// Lo que tiene que quedar a la vista de un pin para poder agarrarlo
    /// cuando la ventana encoge (por cada lado).
    static let aLaVista: CGFloat = 44
    /// El lado de la burbuja de un pin minimizado.
    static let ladoBurbuja: CGFloat = 52
    /// Por debajo de estos grados el giro con dos dedos no cuenta (se
    /// vuelve a donde estaba): asi pellizcar no gira sin querer.
    static let gradosMinimosGiro: Double = 30

    /// El centro limitado para que del pin (de `tamano`) queden al menos
    /// `aLaVista` puntos dentro de `area` por los lados y por abajo; por
    /// arriba no se sale nada (ahi se agarran las herramientas).
    static func centroDentro(_ centro: CGPoint, tamano: CGSize, area: CGSize) -> CGPoint {
        guard area.width > 0, area.height > 0 else { return centro }
        let mx = min(aLaVista, tamano.width / 2)
        let my = min(aLaVista, tamano.height / 2)
        let minX = mx - tamano.width / 2
        let maxX = area.width - mx + tamano.width / 2
        let minY = tamano.height / 2
        let maxY = max(minY, area.height - my + tamano.height / 2)
        return CGPoint(x: ReglasPin.limitar(centro.x, minX, maxX),
                       y: ReglasPin.limitar(centro.y, minY, maxY))
    }

    /// El tamano con el que nace un pin de herramienta (nota o mini-app).
    static func tamanoHerramienta(_ miniapp: String?) -> CGSize {
        switch miniapp {
        case "ruleta": CGSize(width: 300, height: 420)
        case nil: CGSize(width: 300, height: 260)
        default: CGSize(width: 300, height: 340)
        }
    }

    /// Donde nace el pin numero `n` (los seguidos se escalonan para no
    /// quedar uno encima de otro): el centro de la capa mas 24 puntos por
    /// pin, dando la vuelta cada ocho.
    static func centroDeNacer(area: CGSize, n: Int) -> CGPoint {
        let paso = CGFloat(n % 8) * 24
        return CGPoint(x: area.width / 2 + paso, y: area.height / 2 + paso)
    }

    /// Los cuartos de vuelta a la derecha a los que se pega un giro de
    /// `grados` (con dos dedos): el mas cercano, y nada si no llega a
    /// `gradosMinimosGiro`.
    static func cuartosDeGiro(grados: Double) -> Int {
        guard grados.isFinite, abs(grados) >= gradosMinimosGiro else { return 0 }
        return Int((grados / 90).rounded())
    }

    /// Un pin al almacen: el marco que se ve (centro y tamano en puntos de
    /// la capa), la escala y la opacidad.
    static func guardado(centro: CGPoint, tamanoBase: CGSize, escala: CGFloat, escalaPantalla: CGFloat,
                         opacidad: Double) -> PinGuardado {
        let ancho = tamanoBase.width * escala
        let alto = tamanoBase.height * escala
        var g = PinGuardado(x: Int((centro.x - ancho / 2).rounded()), y: Int((centro.y - alto / 2).rounded()),
                            ancho: max(1, Int(ancho.rounded())), alto: max(1, Int(alto.rounded())),
                            escalaPorCien: Int((max(escalaPantalla, 1) * 100).rounded()))
        g.zoomPorCien = max(1, Int((escala * 100).rounded()))
        g.opacidad = Int((ReglasPin.limitarOpacidad(CGFloat(opacidad)) * 100).rounded())
        return g
    }

    /// De vuelta: centro, tamano de nacer (sin la escala) y escala.
    static func deGuardado(_ g: PinGuardado) -> (centro: CGPoint, tamanoBase: CGSize, escala: CGFloat) {
        let escala = max(CGFloat(g.zoomPorCien), 1) / 100
        let ancho = CGFloat(max(g.ancho, 1)), alto = CGFloat(max(g.alto, 1))
        return (CGPoint(x: CGFloat(g.x) + ancho / 2, y: CGFloat(g.y) + alto / 2),
                CGSize(width: ancho / escala, height: alto / escala), escala)
    }

    /// La ruta de un fichero guardado por referencia en otro arranque: en
    /// iOS la carpeta de la app cambia de nombre al actualizarla, asi que una
    /// ruta de dentro de `PixPinMax` que ya no existe se busca en la carpeta
    /// de datos de ahora. `existe` se cambia en las pruebas.
    static func rutaVigente(_ ruta: String, datos: URL,
                            existe: (String) -> Bool = { FileManager.default.fileExists(atPath: $0) }) -> URL? {
        if existe(ruta) { return URL(fileURLWithPath: ruta) }
        let marca = "/" + datos.lastPathComponent + "/"
        guard let r = ruta.range(of: marca, options: .backwards) else { return nil }
        let resto = String(ruta[r.upperBound...])
        let nueva = datos.appendingPathComponent(resto).path
        return existe(nueva) ? URL(fileURLWithPath: nueva) : nil
    }
}
