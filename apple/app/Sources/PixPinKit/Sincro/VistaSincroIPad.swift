// HUECO: lo rellena la parte de la app que le toca (ver docs/plan-ipad.md).
// Mantener la firma publica (`init`) tal cual: la usa `Raiz/RaizIPad.swift`.

import PixPinCore
import SwiftUI

public struct VistaSincroIPad: View {
    let almacen: Almacen
    let sincro: Sincronizador?
    public init(almacen: Almacen, sincro: Sincronizador?) { self.almacen = almacen; self.sincro = sincro }
    public var body: some View { ContentUnavailableView("Sincro", systemImage: "laptopcomputer.and.ipad") }
}
