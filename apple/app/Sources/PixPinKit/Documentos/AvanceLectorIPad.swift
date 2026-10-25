// **La pagina que sube sola** («Desplazar sola»; `LectorPdfActivity` del
// movil v0.92, `lector_pdf.rs` de Windows y `AvanceLector.swift` del Mac).
// La velocidad va en **palabras por minuto**, que es como se mide leer: no
// cambia al acercar (el documento se alarga, pero las palabras son las
// mismas) y «Terminas en…» sale directo. Las cuentas son las de
// `pixpin_docs::auto_desplazar`, por el nucleo (`velocidadDeBajar`,
// `otraVelocidadDeLeer`, `rotuloDeLoQueQueda`); la velocidad se recuerda en
// este aparato (`palabrasPorMinuto`), como alla.
//
// La barra va abajo en medio, de cristal oscuro: −, la velocidad, +, pausa
// y cerrar, y debajo lo que queda. Anotando o con el dedo encima no corre;
// al llegar al final se para sola (la barra sigue, con el play).

import PixPinCore
import QuartzCore
import SwiftUI
import UIKit

@MainActor
final class AvanceLectorIPad: ObservableObject {
    /// La barra puesta.
    @Published var activo = false
    @Published var enMarcha = true
    @Published var ppm: UInt32 = 230
    /// «Terminas en 12 min · a las 18:40».
    @Published var quedan = "Contando las palabras…"
    /// Las palabras del documento (`nil` mientras se cuentan).
    var palabras: Int?
    /// Lo que no llego a un pixel, para la vuelta siguiente.
    var sobra: CGFloat = 0
    var antes: CFTimeInterval?

    /// Una vuelta: cuanto bajar, en puntos, a `porSegundo` (lo que pasa de
    /// una decima entre dos vueltas cuenta como una decima, como alla).
    func paso(ahora: CFTimeInterval, porSegundo: CGFloat, quieta: Bool, pixel: CGFloat) -> CGFloat {
        guard activo, enMarcha, !quieta, porSegundo > 0, pixel > 0 else {
            antes = nil
            sobra = 0
            return 0
        }
        guard let a = antes else {
            antes = ahora
            return 0
        }
        antes = ahora
        // En milesimas enteras, como alla (`ahora_ms`): 10.1 - 10 no da
        // justo una decima en coma flotante y se perderia medio pixel.
        let dt = min((max(ahora - a, 0) * 1000).rounded() / 1000, 0.1)
        sobra += porSegundo * CGFloat(dt)
        let n = (sobra / pixel).rounded(.down) * pixel
        sobra -= n
        return n
    }

    func alternar() {
        enMarcha.toggle()
        antes = nil
        sobra = 0
    }

    func alFinal() {
        enMarcha = false
        antes = nil
        sobra = 0
    }

    /// Los minutos de hoy (para «a las 18:40»).
    static func minutosDeAhora(_ fecha: Date = Date()) -> Int64 {
        let hoy = Calendar.current.dateComponents([.hour, .minute], from: fecha)
        return Int64((hoy.hour ?? 0) * 60 + (hoy.minute ?? 0))
    }
}

/// **La barra de la pagina que sube sola**.
struct BarraAvanceIPad: View {
    @ObservedObject var avance: AvanceLectorIPad
    let cambiar: (Int32) -> Void
    let cerrar: () -> Void

    var body: some View {
        if avance.activo {
            VStack(spacing: 2) {
                HStack(spacing: 0) {
                    boton("minus", "Más despacio") { cambiar(-1) }
                    Text("\(avance.ppm) palabras/min")
                        .font(.system(size: 14).monospacedDigit())
                        .padding(.horizontal, 8)
                        .fixedSize()
                    boton("plus", "Más deprisa") { cambiar(1) }
                    boton(avance.enMarcha ? "pause.fill" : "play.fill",
                          avance.enMarcha ? "Pausa" : "Seguir") { avance.alternar() }
                    boton("xmark", "Dejar de desplazar (D)") { cerrar() }
                }
                Text(avance.quedan)
                    .font(.system(size: 12))
                    .foregroundStyle(Color(white: 0.78))
                    .fixedSize()
            }
            .foregroundStyle(Color(red: 0.96, green: 0.96, blue: 0.97))
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(Color(red: 0.118, green: 0.118, blue: 0.125, opacity: 0.95),
                        in: RoundedRectangle(cornerRadius: 20))
            .environment(\.colorScheme, .dark)
        }
    }

    private func boton(_ simbolo: String, _ ayuda: String, _ accion: @escaping () -> Void) -> some View {
        Button(action: accion) {
            Image(systemName: simbolo).font(.system(size: 16)).frame(width: 40, height: 40)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(ayuda)
        .accessibilityLabel(ayuda)
    }
}

/// **Mover sola una vista que se desplaza** (la del PDF o la del texto
/// reflujado): un reloj de 60 por segundo que baja lo que diga el
/// `AvanceLectorIPad` y, cada segundo, pone lo que queda.
@MainActor
final class RelojAvanceIPad: NSObject {
    let avance: AvanceLectorIPad
    private var enlace: CADisplayLink?
    private var ultimoQuedan: CFTimeInterval = 0
    /// La vista que se desplaza.
    var desplazador: () -> UIScrollView?
    /// Si no debe correr ahora (anotando, el dedo encima).
    var quieta: () -> Bool = { false }

    init(avance: AvanceLectorIPad, desplazador: @escaping () -> UIScrollView?) {
        self.avance = avance
        self.desplazador = desplazador
        super.init()
    }

    func empezar() {
        parar()
        let e = CADisplayLink(target: self, selector: #selector(vuelta))
        e.add(to: .main, forMode: .common)
        enlace = e
        ultimoQuedan = 0
    }

    func parar() {
        enlace?.invalidate()
        enlace = nil
    }

    /// Que se ponga ya «Terminas en…» (al cambiar la velocidad).
    func reponerQuedan() { ultimoQuedan = 0 }

    @objc private func vuelta() {
        guard avance.activo, let s = desplazador() else { return }
        let alto = max(s.contentSize.height, 1)
        let palabras = avance.palabras ?? 0
        let porSegundo = CGFloat(velocidadDeBajar(ppm: avance.ppm, palabras: UInt64(palabras), alto: Float(alto)))
        let ahora = CACurrentMediaTime()
        let pixel = 1 / max(s.traitCollection.displayScale, 1)
        let quieto = quieta() || avance.palabras == nil || s.isTracking || s.isDragging || s.isDecelerating
        let d = avance.paso(ahora: ahora, porSegundo: porSegundo, quieta: quieto, pixel: pixel)
        if d > 0 {
            let tope = max(s.contentSize.height + s.adjustedContentInset.bottom - s.bounds.height,
                           -s.adjustedContentInset.top)
            let antes = s.contentOffset.y
            let nueva = min(antes + d, tope)
            s.contentOffset.y = nueva
            if nueva - antes < pixel / 2 { avance.alFinal() }
        }
        if ahora - ultimoQuedan > 1, let n = avance.palabras {
            ultimoQuedan = ahora
            let corrido = s.contentOffset.y + s.adjustedContentInset.top
            let progreso = Float(min(max(corrido / alto, 0), 1))
            avance.quedan = rotuloDeLoQueQueda(palabras: UInt64(n), progreso: progreso, ppm: avance.ppm,
                                               ahoraEnMinutos: AvanceLectorIPad.minutosDeAhora())
        }
    }
}
