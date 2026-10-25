// **La velocidad de escuchar se recuerda** (de una vez para otra, como el
// movil): la de leer en voz alta (`LeerEnVoz`) y la del reproductor de las
// notas de voz y la musica (`ReproductorVoz`), cada una con la suya. Va en
// las preferencias del iPad: es de quien escucha y no viaja.

import Foundation

enum VelocidadGuardada {
    static let claveLeer = "PixPinVelocidadDeLeer"
    static let claveEscuchar = "PixPinVelocidadDeEscuchar"

    /// La guardada, si es una de las que se ofrecen; si no, la normal (1).
    static func leer(_ clave: String, validas: [Float], en d: UserDefaults = .standard) -> Float {
        guard let v = d.object(forKey: clave) as? NSNumber else { return 1 }
        let f = v.floatValue
        return validas.first { abs($0 - f) < 1e-3 } ?? 1
    }

    static func guardar(_ v: Float, clave: String, en d: UserDefaults = .standard) {
        if abs(v - 1) < 1e-3 { d.removeObject(forKey: clave) } else { d.set(v, forKey: clave) }
    }
}
