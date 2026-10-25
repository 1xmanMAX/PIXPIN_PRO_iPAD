// **El panel de estilo del lienzo**: color del trazo, fondo, relleno,
// grosor, linea, trazo a mano y opacidad. Es el panel lateral de Windows
// (`pixpin-ui/src/panel_lateral.rs`, el de Excalidraw): una isla a la
// izquierda, bajo la barra, con sus secciones en el orden de Excalidraw.
// **Compacto** (Windows, 24-oct: «la barra de propiedades ocupa mucha de la
// visualizacion, redisenala y pon un boton para mostrarla y quitarla»): 168
// de ancho en vez de 200, menos relleno y aire, botones de 28, cuatro por
// fila; el boton de quitarlo va solo en el lienzo del Mac (`BotonPlegarPanel`). Lo
// que se elige queda para lo proximo que se dibuje y, si hay algo elegido,
// se le aplica en un paso de deshacer (`EditorLienzo.cambiarEstilo`, ver
// `pixpin-nucleo/src/editor_estilo.rs`). Lo que ensena es `EditorLienzo.estilo`:
// el de lo elegido o, sin nada, el «actual».
//
// Del Mac, con lo minimo para el iPad: «otro color» es el selector de
// colores del sistema (`ColorPicker`) en vez del panel de colores de AppKit.

import PixPinCore
import SwiftUI
import UIKit

/// Una opcion de un mando: su nombre (que hace de id) y su valor.
public struct OpcionEstiloLienzo<Valor>: Identifiable {
    public let nombre: String
    public let valor: Valor
    public var id: String { nombre }

    init(_ nombre: String, _ valor: Valor) {
        self.nombre = nombre
        self.valor = valor
    }
}

/// Las opciones de cada mando. Aparte de la vista para probarlas sin
/// pantalla.
public enum OpcionesEstiloLienzo {
    /// Los colores de trazo de Excalidraw (`COLOR_PALETTE`, el tono de la barra).
    public static let trazos: [OpcionEstiloLienzo<ColorEstilo>] = [
        .init("Negro", hex(0x1e1e1e)), .init("Rojo", hex(0xe03131)), .init("Verde", hex(0x2f9e44)),
        .init("Azul", hex(0x1971c2)), .init("Naranja", hex(0xf08c00)),
    ]

    /// Los fondos de Excalidraw; `nil` es transparente.
    public static let fondos: [OpcionEstiloLienzo<ColorEstilo?>] = [
        .init("Transparente", nil), .init("Rosa", hex(0xffc9c9)), .init("Verde claro", hex(0xb2f2bb)),
        .init("Azul claro", hex(0xa5d8ff)), .init("Amarillo", hex(0xffec99)),
    ]

    public static let grosores: [OpcionEstiloLienzo<GrosorEstilo>] = [
        .init("Fino", .fino), .init("Medio", .medio), .init("Grueso", .grueso),
    ]

    /// 1, 2 y 3: los tres grosores, como las teclas de la pluma de Windows
    /// (`tecla_a_pluma`).
    public static func grosor(tecla: String) -> GrosorEstilo? {
        switch tecla {
        case "1": return .fino
        case "2": return .medio
        case "3": return .grueso
        default: return nil
        }
    }

    /// El trazo a mano de Excalidraw (`roughness`): 0 arquitecto, 1 artista
    /// y 2 caricaturista.
    public static let rugosidades: [OpcionEstiloLienzo<Float>] = [
        .init("Arquitecto", 0), .init("Artista", 1), .init("Caricaturista", 2),
    ]

    // Los iconos de cada opcion: los de Excalidraw, como el panel de Windows
    // (`panel_dibujo.rs`).
    static func icono(_ g: GrosorEstilo) -> IconoLienzo {
        switch g {
        case .fino: return .strokeWidthBase
        case .medio: return .strokeWidthBold
        case .grueso: return .strokeWidthExtraBold
        }
    }

    static func icono(_ l: LineaEstilo) -> IconoLienzo {
        switch l {
        case .continua: return .strokeStyleSolid
        case .discontinua: return .strokeStyleDashed
        case .punteada: return .strokeStyleDotted
        case .rayaPunto: return .strokeStyleDashed
        }
    }

    static func icono(_ r: RellenoEstilo) -> IconoLienzo {
        switch r {
        case .solido: return .fillSolid
        case .rayado: return .fillHachure
        case .cruzado: return .fillCrossHatch
        case .zigzag: return .fillZigZag
        case .lineas: return .fillLineas
        }
    }

    static func icono(rugosidad r: Float) -> IconoLienzo {
        r < 0.5 ? .sloppinessArchitect : (r < 1.5 ? .sloppinessArtist : .sloppinessCartoonist)
    }

    public static let lineas: [OpcionEstiloLienzo<LineaEstilo>] = [
        .init("Continua", .continua), .init("Discontinua", .discontinua), .init("Punteada", .punteada),
        .init("Raya y punto", .rayaPunto),
    ]

    /// En el orden de Excalidraw: rayado, cruzado y solido; y despues los
    /// dos que Excalidraw no tiene.
    public static let rellenos: [OpcionEstiloLienzo<RellenoEstilo>] = [
        .init("Rayado", .rayado), .init("Cruzado", .cruzado), .init("Sólido", .solido),
        .init("Zigzag", .zigzag), .init("Líneas", .lineas),
    ]

    public static func hex(_ v: UInt32) -> ColorEstilo {
        ColorEstilo(r: Float((v >> 16) & 0xff) / 255, g: Float((v >> 8) & 0xff) / 255,
                    b: Float(v & 0xff) / 255, a: 1)
    }

    public static func color(_ c: ColorEstilo) -> Color {
        Color(.sRGB, red: Double(c.r), green: Double(c.g), blue: Double(c.b), opacity: Double(c.a))
    }

    /// De un color del selector del sistema al del motor (sRGB).
    public static func estilo(de color: Color) -> ColorEstilo? {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        guard UIColor(color).getRed(&r, green: &g, blue: &b, alpha: &a) else { return nil }
        func canal(_ v: CGFloat) -> Float { Float(min(1, max(0, v))) }
        return ColorEstilo(r: canal(r), g: canal(g), b: canal(b), a: canal(a))
    }

    /// Si dos colores son el mismo a la vista (un canal de 8 bits).
    public static func igual(_ a: ColorEstilo?, _ b: ColorEstilo?) -> Bool {
        switch (a, b) {
        case (nil, nil): return true
        case let (x?, y?):
            return abs(x.r - y.r) < 0.004 && abs(x.g - y.g) < 0.004 && abs(x.b - y.b) < 0.004
                && abs(x.a - y.a) < 0.004
        default: return false
        }
    }

    static func nombre(_ g: GrosorEstilo) -> String {
        grosores.first { mismo($0.valor, g) }?.nombre ?? ""
    }

    static func nombre(_ l: LineaEstilo) -> String {
        lineas.first { mismo($0.valor, l) }?.nombre ?? ""
    }

    static func nombre(_ r: RellenoEstilo) -> String {
        rellenos.first { mismo($0.valor, r) }?.nombre ?? ""
    }

    // Comparar sin depender de que los enlaces generen `Equatable`.
    static func mismo(_ a: GrosorEstilo, _ b: GrosorEstilo) -> Bool { "\(a)" == "\(b)" }
    static func mismo(_ a: LineaEstilo, _ b: LineaEstilo) -> Bool { "\(a)" == "\(b)" }
    static func mismo(_ a: RellenoEstilo, _ b: RellenoEstilo) -> Bool { "\(a)" == "\(b)" }
}

extension IconoLienzo {
    /// El relleno de lineas, que Excalidraw no tiene: su cuadro con tres
    /// rayas tumbadas, del mismo trazo que los otros rellenos.
    static let fillLineas = IconoLienzo(vista: CGRect(x: 0, y: 0, width: 20, height: 20), trazos: [
        .init(d: "M5.879 2.625h8.242a3.254 3.254 0 0 1 3.254 3.254v8.242a3.254 3.254 0 0 1-3.254 3.254H5.88a3.254 3.254 0 0 1-3.254-3.254V5.88a3.254 3.254 0 0 1 3.254-3.254Z",
              relleno: .nada, trazo: .actual, grosor: 1.25, redondo: true, opacidad: 1, parImpar: false,
              matriz: nil, mascara: nil),
        .init(d: "M2.6 7.2h14.8M2.6 10h14.8M2.6 12.8h14.8", relleno: .nada, trazo: .actual, grosor: 1.25,
              redondo: true, opacidad: 1, parImpar: false, matriz: nil,
              mascara: "M5.879 2.625h8.242a3.254 3.254 0 0 1 3.254 3.254v8.242a3.254 3.254 0 0 1-3.254 3.254H5.88a3.254 3.254 0 0 1-3.254-3.254V5.88a3.254 3.254 0 0 1 3.254-3.254Z"),
    ])
}

/// Un cuadradito de color (o el damero de «transparente»), de 20 como el
/// panel compacto de Windows; el elegido lleva el anillo del color de la
/// barra.
private struct Muestra: View {
    let color: ColorEstilo?
    let elegida: Bool
    let tema: TemaBarraLienzo

    var body: some View {
        ZStack {
            if let color {
                RoundedRectangle(cornerRadius: 4).fill(OpcionesEstiloLienzo.color(color))
            } else {
                Damero().clipShape(RoundedRectangle(cornerRadius: 4))
            }
            RoundedRectangle(cornerRadius: 4)
                .strokeBorder(Color.black.opacity(0.12), lineWidth: 1)
        }
        .frame(width: 18, height: 18)
        .padding(2)
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .strokeBorder(elegida ? tema.menuIconoElegido : Color.clear, lineWidth: 1.5)
        )
    }
}

/// El damero de lo transparente.
private struct Damero: View {
    var body: some View {
        Canvas { ctx, tam in
            ctx.fill(Path(CGRect(origin: .zero, size: tam)), with: .color(.white))
            let lado: CGFloat = 5
            var y: CGFloat = 0, fila = 0
            while y < tam.height {
                var x: CGFloat = fila % 2 == 0 ? 0 : lado
                while x < tam.width {
                    ctx.fill(Path(CGRect(x: x, y: y, width: lado, height: lado)), with: .color(Color(white: 0.85)))
                    x += lado * 2
                }
                y += lado
                fila += 1
            }
        }
    }
}

/// Un boton de opcion de 28 (relleno, grosor, linea, trazo a mano).
private struct BotonOpcion: View {
    let icono: IconoLienzo
    let elegida: Bool
    let tema: TemaBarraLienzo
    let ayuda: String
    let accion: () -> Void

    @State private var encima = false

    var body: some View {
        Button(action: accion) {
            IconoLienzoVista(icono: icono, lado: 15, color: elegida ? tema.activoIcono : tema.icono)
                .frame(width: 28, height: 28)
                .background(elegida ? tema.activoFondo : (encima ? tema.hover : tema.separador.opacity(0.6)),
                            in: RoundedRectangle(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8)
                    .strokeBorder(elegida ? tema.menuIconoElegido : Color.clear, lineWidth: 1))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { encima = $0 }
        .help(ayuda)
        .accessibilityLabel(ayuda)
    }
}

/// **El panel de estilo**, en su isla de 168 a la izquierda (el compacto de
/// Windows).
struct PanelEstiloLienzo: View {
    @ObservedObject var modelo: ModeloEditorLienzo

    private var estilo: EstiloLienzo { modelo.estilo }

    var body: some View {
        let tema = TemaBarraLienzo.de(modelo)
        VStack(alignment: .leading, spacing: 8) {
            seccion("Trazo", tema) {
                HStack(spacing: 3) {
                    ForEach(OpcionesEstiloLienzo.trazos) { o in
                        Button { modelo.cambiarEstilo(.trazo(color: o.valor)) } label: {
                            Muestra(color: o.valor, elegida: OpcionesEstiloLienzo.igual(estilo.trazo, o.valor),
                                    tema: tema)
                        }
                        .buttonStyle(.plain)
                        .help("Trazo: \(o.nombre)")
                    }
                    otroColor(estilo.trazo, tema: tema, ayuda: "Otro color de trazo") {
                        modelo.cambiarEstilo(.trazo(color: $0))
                    }
                }
            }
            seccion("Fondo", tema) {
                HStack(spacing: 3) {
                    ForEach(OpcionesEstiloLienzo.fondos) { o in
                        Button { modelo.cambiarEstilo(.fondo(color: o.valor)) } label: {
                            Muestra(color: o.valor, elegida: OpcionesEstiloLienzo.igual(estilo.fondo, o.valor),
                                    tema: tema)
                        }
                        .buttonStyle(.plain)
                        .help("Fondo: \(o.nombre)")
                    }
                    otroColor(estilo.fondo, tema: tema, ayuda: "Otro color de fondo") {
                        modelo.cambiarEstilo(.fondo(color: $0))
                    }
                }
            }
            // Sin fondo no hay nada que rayar (`con_fondo` de Windows).
            if estilo.fondo != nil {
                seccion("Relleno", tema) {
                    opciones(OpcionesEstiloLienzo.rellenos, tema: tema,
                             elegida: { OpcionesEstiloLienzo.mismo(estilo.relleno, $0) },
                             icono: OpcionesEstiloLienzo.icono) { modelo.cambiarEstilo(.relleno(relleno: $0)) }
                }
            }
            seccion("Grosor del trazo", tema) {
                opciones(OpcionesEstiloLienzo.grosores, tema: tema,
                         elegida: { OpcionesEstiloLienzo.mismo(estilo.grosor, $0) },
                         icono: OpcionesEstiloLienzo.icono) { modelo.cambiarEstilo(.grosor(grosor: $0)) }
            }
            seccion("Estilo del trazo", tema) {
                opciones(OpcionesEstiloLienzo.lineas, tema: tema,
                         elegida: { OpcionesEstiloLienzo.mismo(estilo.linea, $0) },
                         icono: OpcionesEstiloLienzo.icono) { modelo.cambiarEstilo(.linea(linea: $0)) }
            }
            seccion("Trazo a mano", tema) {
                opciones(OpcionesEstiloLienzo.rugosidades, tema: tema,
                         elegida: { abs(estilo.rugosidad - $0) < 0.5 },
                         icono: { OpcionesEstiloLienzo.icono(rugosidad: $0) }) {
                    modelo.cambiarEstilo(.rugosidad(valor: $0))
                }
            }
            seccion("Opacidad", tema) {
                Slider(value: Binding(get: { Double(estilo.opacidad) * 100 },
                                      set: { modelo.cambiarEstilo(.opacidad(valor: Float($0 / 100))) }),
                       in: 0...100, step: 10)
                    .controlSize(.small)
                    .tint(tema.menuIconoElegido)
            }
            // Lo elegido, a la biblioteca de figuras (la fila «Acciones» del
            // panel de Windows: «Guardar en figuras»).
            if modelo.hayElegidos, modelo.nucleo != nil {
                seccion("Acciones", tema) {
                    Button { modelo.guardarComoFigura() } label: {
                        Label("Guardar en figuras", systemImage: "rectangle.stack.badge.plus")
                            .font(.system(size: 12))
                            .foregroundStyle(tema.icono)
                    }
                    .buttonStyle(.plain)
                    .help("Guardar lo elegido en tus figuras")
                }
            }
        }
        .padding(8)
        .frame(width: MedidasBarraLienzo.anchoPanel, alignment: .leading)
        .modifier(IslaLienzo(tema: tema))
    }

    private func seccion(_ titulo: String, _ tema: TemaBarraLienzo,
                         @ViewBuilder _ contenido: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(titulo).font(.system(size: 11)).foregroundStyle(tema.icono.opacity(0.75))
            contenido()
        }
    }

    private func opciones<V>(_ lista: [OpcionEstiloLienzo<V>], tema: TemaBarraLienzo,
                             elegida: @escaping (V) -> Bool, icono: @escaping (V) -> IconoLienzo,
                             accion: @escaping (V) -> Void) -> some View {
        // Cuatro por fila (los rellenos son cinco: el quinto baja).
        let filas = stride(from: 0, to: lista.count, by: 4).map { Array(lista[$0..<min($0 + 4, lista.count)]) }
        return VStack(alignment: .leading, spacing: 6) {
            ForEach(filas.indices, id: \.self) { f in
                HStack(spacing: 6) {
                    ForEach(filas[f]) { o in
                        BotonOpcion(icono: icono(o.valor), elegida: elegida(o.valor), tema: tema, ayuda: o.nombre) {
                            accion(o.valor)
                        }
                    }
                }
            }
        }
    }

    /// La muestra del color de ahora tras una raya: abre el selector de
    /// colores del sistema.
    private func otroColor(_ actual: ColorEstilo?, tema: TemaBarraLienzo, ayuda: String,
                           _ alCambiar: @escaping (ColorEstilo) -> Void) -> some View {
        HStack(spacing: 4) {
            Rectangle().fill(tema.separador).frame(width: 1, height: 20)
            ColorPicker(ayuda, selection: Binding(
                get: { actual.map(OpcionesEstiloLienzo.color) ?? .black },
                set: { nuevo in
                    guard var c = OpcionesEstiloLienzo.estilo(de: nuevo) else { return }
                    c.a = 1
                    alCambiar(c)
                }), supportsOpacity: false)
                .labelsHidden()
                .frame(width: 24, height: 24)
                .help(ayuda)
        }
    }
}
