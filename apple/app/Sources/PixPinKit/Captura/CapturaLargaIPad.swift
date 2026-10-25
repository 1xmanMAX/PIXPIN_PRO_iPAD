// **La captura larga** (`scroll.rs` de Windows, `CapturaConScroll.swift` y
// `CosidoScroll.swift` del Mac): iOS no deja capturar otras apps, pero de la
// propia PixPin si se puede sacar entero lo que tiene rollo (el chat, el
// timeline, una lista): se busca el rollo mas grande que se ve, se baja
// pantalla a pantalla desde arriba fotografiando cada trozo y se cosen en
// una sola imagen larga, que va al editor de capturas (anotar, copiar,
// pinear, al chat, guardar). Al acabar el rollo vuelve donde estaba.
//
// Las listas perezosas de SwiftUI pintan lo que entra al bajar: entre trozo y
// trozo se espera un poco. Hay un tope (`topePuntos`) para que una
// conversacion de anos no se coma la memoria.

import SwiftUI
import UIKit

@MainActor
enum CapturaLargaIPad {
    /// Lo mas largo que se cose, en puntos.
    static let topePuntos: CGFloat = 12_000
    /// Lo mas alto de la imagen, en pixeles.
    static let topePixeles: CGFloat = 16_000
    /// Lo que se espera a que se pinte cada trozo.
    static let espera: Duration = .milliseconds(160)

    /// Fotografia el rollo y abre la imagen en el editor de capturas.
    static func capturar() {
        guard let ventana = ArribaUIKit.ventana(), let rollo = rollo(en: ventana) else {
            CuadrosIPad.avisar("Aquí no hay nada con rollo que capturar entero.")
            return
        }
        _ = Task<Void, Never> {
            guard let imagen = await coser(rollo) else { return avisarFalloPin() }
            UIImpactFeedbackGenerator(style: .light).impactOccurred()
            CapturadorIPad.compartido.editar(imagen)
        }
    }

    /// **El rollo que se captura**: el de mas superficie a la vista que
    /// tenga mas contenido que alto (no un campo de texto).
    static func rollo(en vista: UIView) -> UIScrollView? {
        var mejor: (UIScrollView, CGFloat)?
        func mirar(_ v: UIView) {
            guard !v.isHidden, v.alpha > 0.01 else { return }
            if let s = v as? UIScrollView, !(s is UITextView), s.isScrollEnabled,
               s.contentSize.height > s.bounds.height + 1, s.bounds.width > 120, s.bounds.height > 120 {
                let visible = s.convert(s.bounds, to: nil).intersection(vista.bounds)
                let area = visible.isNull ? 0 : visible.width * visible.height
                if area > (mejor?.1 ?? 0) { mejor = (s, area) }
            }
            for h in v.subviews { mirar(h) }
        }
        mirar(vista)
        return mejor?.0
    }

    /// Donde va cada trozo (puro: se prueba solo). `arriba` es el primer
    /// desplazamiento, `fin` el ultimo posible, `alto` lo que se ve; cada
    /// trozo empieza donde acaba el anterior y el ultimo se pega al fin.
    nonisolated static func desplazamientos(arriba: CGFloat, fin: CGFloat, alto: CGFloat, tope: CGFloat) -> [CGFloat] {
        guard alto > 0 else { return [] }
        let ultimo = max(arriba, min(fin, arriba + max(tope - alto, 0)))
        var v: [CGFloat] = [arriba]
        var y = arriba
        while y < ultimo - 0.5 {
            y = min(y + alto, ultimo)
            v.append(y)
        }
        return v
    }

    /// La escala de la imagen cosida: la de la pantalla, sin pasar del tope
    /// de pixeles.
    nonisolated static func escala(pantalla: CGFloat, altoTotal: CGFloat, tope: CGFloat) -> CGFloat {
        guard altoTotal > 0 else { return pantalla }
        return max(0.5, min(pantalla, tope / altoTotal))
    }

    /// Baja el rollo trozo a trozo, fotografia y cose.
    private static func coser(_ s: UIScrollView) async -> CGImage? {
        let original = s.contentOffset
        let alto = s.bounds.height, ancho = s.bounds.width
        let arriba = -s.adjustedContentInset.top
        let fin = max(arriba, s.contentSize.height + s.adjustedContentInset.bottom - alto)
        let ys = desplazamientos(arriba: arriba, fin: fin, alto: alto, tope: topePuntos)
        guard let primera = ys.first, let ultima = ys.last else { return nil }
        let total = ultima - primera + alto
        let k = escala(pantalla: s.window?.screen.scale ?? 2, altoTotal: total, tope: topePixeles)
        let formato = UIGraphicsImageRendererFormat()
        formato.scale = k
        formato.opaque = true
        let lienzo = CGRect(origin: .zero, size: s.bounds.size)
        var trozos: [(CGFloat, UIImage)] = []
        let indicador = s.showsVerticalScrollIndicator
        s.showsVerticalScrollIndicator = false
        for y in ys {
            s.setContentOffset(CGPoint(x: original.x, y: y), animated: false)
            s.layoutIfNeeded()
            try? await Task.sleep(for: espera)
            let foto = UIGraphicsImageRenderer(bounds: lienzo, format: formato).image { _ in
                s.drawHierarchy(in: lienzo, afterScreenUpdates: true)
            }
            // Lo que de verdad bajo (el rollo pudo crecer o pararse antes).
            trozos.append((s.contentOffset.y - primera, foto))
        }
        s.setContentOffset(original, animated: false)
        s.showsVerticalScrollIndicator = indicador
        let altoCosido = (trozos.map(\.0).max() ?? 0) + alto
        let caja = CGRect(x: 0, y: 0, width: ancho, height: altoCosido)
        let cosida = UIGraphicsImageRenderer(bounds: caja, format: formato).image { _ in
            UIColor.systemBackground.setFill()
            UIRectFill(caja)
            for (y, foto) in trozos { foto.draw(at: CGPoint(x: 0, y: y)) }
        }
        return cosida.cgImage
    }
}
