// HUECO: lo rellena la parte de la app que le toca (ver docs/plan-ipad.md).
// Mantener la firma publica (`init`) tal cual: la usa `Raiz/RaizIPad.swift`.

import PixPinCore
import SwiftUI

/// La capa de pines flotantes, encima de toda la app (y de los visores).
public struct CapaPines: View {
    let almacen: Almacen
    public init(almacen: Almacen) { self.almacen = almacen }
    public var body: some View { Color.clear.allowsHitTesting(false) }
}
