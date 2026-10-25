// HUECO: lo rellena la parte de la app que le toca (ver docs/plan-ipad.md).
// Mantener la firma publica (`init`) tal cual: la usa `Raiz/RaizIPad.swift`.

import PixPinCore
import SwiftUI

public struct VisorPlanoIPad: View {
    let url: URL
    let titulo: String
    let nucleo: Nucleo
    public init(url: URL, titulo: String, nucleo: Nucleo) { self.url = url; self.titulo = titulo; self.nucleo = nucleo }
    public var body: some View { VStack { BarraVisorIPad(tipo: "Plano", icono: "square.grid.3x3", titulo: titulo); Spacer() } }
}

public struct Visor3DIPad: View {
    let url: URL
    let titulo: String
    let nucleo: Nucleo
    public init(url: URL, titulo: String, nucleo: Nucleo) { self.url = url; self.titulo = titulo; self.nucleo = nucleo }
    public var body: some View { VStack { BarraVisorIPad(tipo: "Modelo 3D", icono: "cube", titulo: titulo); Spacer() } }
}
