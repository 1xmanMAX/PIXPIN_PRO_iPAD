// La lupa del editor de captura (la de Windows, `pixpin-ui/src/lupa.rs`):
// un circulo con los pixeles de alrededor del dedo (o del Apple Pencil por
// encima, sin tocar) muy aumentados, una cruz en el del medio y su color en
// HEX o en RGB. `C` copia el color; tocar el texto cambia HEX / RGB.
//
// El color se lee del pixel de la foto (un recorte de 1 x 1, que no copia
// la foto, dibujado en un contexto de 1 pixel).

import SwiftUI
import UIKit

/// Un color de la foto.
struct ColorPixel: Equatable {
    var r: UInt8
    var g: UInt8
    var b: UInt8

    var hex: String { RecorteCaptura.hex(r, g, b) }
    var rgb: String { RecorteCaptura.rgb(r, g, b) }

    var color: Color {
        Color(.sRGB, red: Double(r) / 255, green: Double(g) / 255, blue: Double(b) / 255, opacity: 1)
    }

    /// El color del pixel (`x`, `y`) de `imagen` (origen arriba a la
    /// izquierda), en sRGB.
    static func en(_ imagen: CGImage, x: Int, y: Int) -> ColorPixel? {
        guard x >= 0, y >= 0, x < imagen.width, y < imagen.height,
              let uno = imagen.cropping(to: CGRect(x: x, y: y, width: 1, height: 1)),
              let espacio = CGColorSpace(name: CGColorSpace.sRGB) else { return nil }
        var px = [UInt8](repeating: 0, count: 4)
        let hecho: Bool = px.withUnsafeMutableBytes { buf in
            guard let ctx = CGContext(data: buf.baseAddress, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
                                      space: espacio, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
            else { return false }
            ctx.draw(uno, in: CGRect(x: 0, y: 0, width: 1, height: 1))
            return true
        }
        guard hecho else { return nil }
        // Sin el alfa premultiplicado (una captura casi siempre es opaca).
        let a = max(Int(px[3]), 1)
        func quitarAlfa(_ v: UInt8) -> UInt8 { UInt8(min(255, Int(v) * 255 / a)) }
        return px[3] == 255 ? ColorPixel(r: px[0], g: px[1], b: px[2]) : ColorPixel(r: quitarAlfa(px[0]), g: quitarAlfa(px[1]), b: quitarAlfa(px[2]))
    }
}

/// El circulo de la lupa.
struct VistaLupaCaptura: View {
    let imagen: CGImage
    /// El punto en pixeles de la foto.
    let punto: CGPoint
    @Binding var enHex: Bool
    let alCopiar: () -> Void

    /// Cuantos pixeles de la foto ensena de lado (impar: hay uno en medio).
    static let pixeles = 15
    static let lado: CGFloat = 120

    var body: some View {
        let zona = RecorteCaptura.zonaDeLupa(en: punto, lado: Self.pixeles, ancho: imagen.width, alto: imagen.height)
        let trozo = imagen.cropping(to: zona)
        let color = RecorteCaptura.pixel(en: punto, ancho: imagen.width, alto: imagen.height)
            .flatMap { ColorPixel.en(imagen, x: $0.x, y: $0.y) }
        // Donde cae el pixel del medio dentro del trozo (en los bordes no
        // esta en el centro).
        let celda = Self.lado / CGFloat(max(zona.width, 1))
        let cx = (punto.x.rounded(.down) - zona.minX + 0.5) * celda
        let cy = (punto.y.rounded(.down) - zona.minY + 0.5) * celda
        VStack(spacing: 6) {
            ZStack(alignment: .topLeading) {
                if let trozo {
                    Image(decorative: trozo, scale: 1)
                        .resizable()
                        .interpolation(.none)
                        .frame(width: Self.lado, height: Self.lado)
                } else {
                    Color.black.frame(width: Self.lado, height: Self.lado)
                }
                Rectangle()
                    .strokeBorder(Color.white, lineWidth: 1.5)
                    .background(Rectangle().strokeBorder(Color.black.opacity(0.6), lineWidth: 3))
                    .frame(width: celda, height: celda)
                    .offset(x: cx - celda / 2, y: cy - celda / 2)
            }
            .frame(width: Self.lado, height: Self.lado)
            .clipShape(Circle())
            .overlay(Circle().strokeBorder(Paleta.v2.acento, lineWidth: 2))
            if let color {
                Button {
                    enHex.toggle()
                } label: {
                    HStack(spacing: 6) {
                        RoundedRectangle(cornerRadius: 3).fill(color.color).frame(width: 14, height: 14)
                            .overlay(RoundedRectangle(cornerRadius: 3).strokeBorder(Color.white.opacity(0.5)))
                        Text(enHex ? color.hex : color.rgb).font(.caption.monospaced().weight(.semibold))
                    }
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(Paleta.v2.fondo.opacity(0.95), in: Capsule())
                }
                .buttonStyle(.plain)
                .help("HEX / RGB")
                Button(action: alCopiar) {
                    Text("C · copiar el color").font(.caption2)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 3)
                        .background(Paleta.v2.fondo.opacity(0.85), in: Capsule())
                }
                .buttonStyle(.plain)
                .foregroundStyle(Paleta.v2.gris)
            }
        }
        .foregroundStyle(Paleta.v2.texto)
        .shadow(color: .black.opacity(0.4), radius: 8, y: 2)
    }
}
