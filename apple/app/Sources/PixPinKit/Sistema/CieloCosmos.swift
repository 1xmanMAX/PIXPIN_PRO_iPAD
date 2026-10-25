// **El tema Cosmos** (`tema_cosmos.rs` y `cielo.rs` de Windows, `Cosmos.kt`
// del movil): los colores de noche y, detras de la lista de proyectos, el
// cielo del sistema solar: el degradado radial azul noche (`#151B3F` en el
// centro, `#05060F` en el borde), las dos nebulosas (violeta y cian) y 217
// estrellas quietas (160, 45 y 12, de tenues a brillantes), siempre en el
// mismo sitio. Sin animacion ni paralaje: se pinta una vez por tamano.
//
// Es una opcion mas del tema (`Tema.cosmos`, Ajustes › General).

import SwiftUI

/// Las cuentas del cielo (puras: se prueban solas).
enum CuentasCosmos {
    /// Cuantas estrellas de cada tamano.
    static let cuantas = [160, 45, 12]
    /// Su color y su alfa (`BRILLOS`).
    static let brillos: [(UInt32, Double)] = [(0xffffff, 0x55 / 255.0), (0xdce6ff, 0x88 / 255.0), (0xfff4d6, 0xcc / 255.0)]
    static let dentro: UInt32 = 0x151B3F
    static let fuera: UInt32 = 0x05060F
    /// El centro del degradado (partes de la pantalla) y su radio (partes
    /// del lado mayor).
    static let centro = CGPoint(x: 0.5, y: 0.35)
    static let radio: CGFloat = 0.95
    /// Las nebulosas: color, alfa, centro (partes) y radio (partes del ancho).
    static let nebulosas: [(UInt32, Double, CGPoint, CGFloat)] = [
        (0x5B3FD9, 0x33 / 255.0, CGPoint(x: 0.2, y: 0.22), 0.8),
        (0x1FA2C9, 0x2A / 255.0, CGPoint(x: 0.9, y: 0.78), 0.7),
    ]

    /// Las posiciones (de 0 a 1) de las estrellas de cada capa, siempre las
    /// mismas: el generador congruente de Windows con la semilla 11.
    static func estrellas() -> [[CGPoint]] {
        var s: UInt64 = 11
        func azar() -> CGFloat {
            s = s &* 6_364_136_223_846_793_005 &+ 1_442_695_040_888_963_407
            return CGFloat(Double(s >> 40) / Double(1 << 24))
        }
        return cuantas.map { n in (0 ..< n).map { _ in CGPoint(x: azar(), y: azar()) } }
    }

    static func color(_ rgb: UInt32, _ alfa: Double = 1) -> Color {
        Color(.sRGB, red: Double((rgb >> 16) & 0xff) / 255, green: Double((rgb >> 8) & 0xff) / 255,
              blue: Double(rgb & 0xff) / 255, opacity: alfa)
    }
}

/// **El cielo**, para poner detras de una pantalla opaca.
struct CieloCosmos: View {
    private static let estrellas = CuentasCosmos.estrellas()

    var body: some View {
        Canvas(rendersAsynchronously: true) { ctx, tam in
            let caja = CGRect(origin: .zero, size: tam)
            let lado = max(tam.width, tam.height)
            let c = CGPoint(x: tam.width * CuentasCosmos.centro.x, y: tam.height * CuentasCosmos.centro.y)
            ctx.fill(Path(caja), with: .radialGradient(
                Gradient(colors: [CuentasCosmos.color(CuentasCosmos.dentro), CuentasCosmos.color(CuentasCosmos.fuera)]),
                center: c, startRadius: 0, endRadius: max(lado * CuentasCosmos.radio, 1)))
            for (rgb, alfa, centro, radio) in CuentasCosmos.nebulosas {
                let p = CGPoint(x: tam.width * centro.x, y: tam.height * centro.y)
                ctx.fill(Path(caja), with: .radialGradient(
                    Gradient(colors: [CuentasCosmos.color(rgb, alfa), CuentasCosmos.color(rgb, 0)]),
                    center: p, startRadius: 0, endRadius: max(tam.width * radio, 1)))
            }
            for (k, capa) in Self.estrellas.enumerated() {
                let (rgb, alfa) = CuentasCosmos.brillos[k]
                let r = CGFloat(k + 1) * 0.55
                let color = CuentasCosmos.color(rgb, alfa)
                for e in capa {
                    let p = CGPoint(x: e.x * tam.width, y: e.y * tam.height)
                    ctx.fill(Path(ellipseIn: CGRect(x: p.x - r, y: p.y - r, width: 2 * r, height: 2 * r)), with: .color(color))
                }
            }
        }
        .accessibilityHidden(true)
    }
}

public extension View {
    /// El cielo del tema Cosmos detras, si es el tema puesto; si no, `fondo`.
    func fondoCosmos<Fondo: View>(_ fondo: Fondo) -> some View {
        modifier(FondoCosmos(fondo: fondo))
    }
}

private struct FondoCosmos<Fondo: View>: ViewModifier {
    let fondo: Fondo
    @AppStorage("tema") private var tema: String = ""

    func body(content: Content) -> some View {
        content.background {
            if tema == "cosmos" {
                CieloCosmos().ignoresSafeArea()
            } else {
                fondo
            }
        }
    }
}
