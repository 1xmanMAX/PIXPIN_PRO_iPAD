// **El suavizado «Natural» del trazo a mano** (Ajustes → Lienzo →
// «Suavizado del trazo»): el filtro de 1 euro de `pixpin-tinta`
// (`crates/pixpin-tinta/src/filtro.rs`, Casiez, Roussel y Vogel, CHI 2012)
// delante del motor, como `dibujo::mano::al_motor` de Windows. Quieto
// filtra mucho (quita el temblor de la mano parada) y corriendo casi nada
// (no retrasa la punta). Se filtra en puntos de PANTALLA, no del dibujo: el
// temblor de la mano no cambia con el aumento. Solo mientras se traza con
// el lapiz, el resaltador o el grafito.
//
// Puro, sin UIKit: se prueba sin pantalla.

import Foundation

/// Un filtro de 1 euro de dos ejes.
public struct FiltroUnEuro: Equatable {
    /// Los mandos del filtro, con los nombres del articulo.
    public struct Mandos: Equatable {
        /// `mincutoff` en hercios: el corte con el puntero quieto.
        public var corteMinimo: Float
        /// `beta`: cuanto sube el corte con la velocidad.
        public var beta: Float
        /// `dcutoff`: el corte del filtro de la propia velocidad.
        public var corteVelocidad: Float

        /// Los de fabrica del motor (conservadores, como `Mandos::default`).
        public init(corteMinimo: Float = 1.0, beta: Float = 0.007, corteVelocidad: Float = 1.0) {
            self.corteMinimo = corteMinimo
            self.beta = beta
            self.corteVelocidad = corteVelocidad
        }
    }

    private struct PasoBajo: Equatable {
        var anterior: Float?

        mutating func filtrar(_ x: Float, _ alfa: Float) -> Float {
            let y = anterior.map { alfa * x + (1 - alfa) * $0 } ?? x
            anterior = y
            return y
        }
    }

    private struct Eje: Equatable {
        var x = PasoBajo()
        var dx = PasoBajo()
        var anterior: Float?

        mutating func filtrar(_ v: Float, _ dt: Float, _ m: Mandos) -> Float {
            let cruda: Float
            if let p = anterior, dt > 0 { cruda = (v - p) / dt } else { cruda = 0 }
            anterior = v
            let velocidad = dx.filtrar(cruda, FiltroUnEuro.alfa(m.corteVelocidad, dt))
            let corte = m.corteMinimo + m.beta * abs(velocidad)
            return x.filtrar(v, FiltroUnEuro.alfa(corte, dt))
        }
    }

    public let mandos: Mandos
    private var ejes = [Eje(), Eje()]
    private var ultimoMs: Double?

    public init(_ mandos: Mandos = Mandos()) {
        self.mandos = mandos
    }

    /// `1 / (1 + tau/dt)` con `tau = 1 / (2·pi·corte)`; sin tiempo, 1.
    static func alfa(_ corte: Float, _ dt: Float) -> Float {
        guard dt > 0, corte > 0 else { return 1 }
        let tau = 1 / (2 * Float.pi * corte)
        return 1 / (1 + tau / dt)
    }

    /// Olvida el trazo anterior: uno nuevo no hereda posicion ni velocidad.
    public mutating func reiniciar() {
        ejes = [Eje(), Eje()]
        ultimoMs = nil
    }

    /// El punto filtrado; `ms` es la hora real de la muestra.
    public mutating func filtrar(_ x: Float, _ y: Float, ms: Double) -> (Float, Float) {
        let dt = ultimoMs.map { Float((ms - $0) / 1000) } ?? 0
        ultimoMs = ms
        return (ejes[0].filtrar(x, dt, mandos), ejes[1].filtrar(y, dt, mandos))
    }
}
