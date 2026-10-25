// HUECO: lo rellena la parte de la app que le toca (ver docs/plan-ipad.md).
// Mantener la firma publica (`init`) tal cual: la usa `Raiz/RaizIPad.swift`.

import PixPinCore
import SwiftUI

public struct VistaTareas: View {
    let almacen: Almacen
    public init(almacen: Almacen) { self.almacen = almacen }
    public var body: some View { ContentUnavailableView("Tareas", systemImage: "checklist") }
}
