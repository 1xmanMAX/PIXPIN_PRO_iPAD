// **Editar una tabla ya dibujada** (`ventana_editor/figuras.rs`,
// `editar_tabla` de Windows): con la tabla elegida, «Editar la tabla…» de
// las figuras abre su cajetin con lo escrito en cada celda, «+ Fila»,
// «− Fila», «+ Columna», «− Columna» y «Cabecera»; «Aplicar» la vuelve a
// dibujar en el mismo sitio y grupo, con su letra y su color y las figuras
// metidas en su celda (`EditorLienzo.reemplazarTabla`, un paso de deshacer).
//
// La rejilla (`RejillaTabla`) es pura: se prueba sin pantalla.

import PixPinCore
import SwiftUI
import UIKit

/// Las celdas del cajetin y lo que les hacen sus botones.
public struct RejillaTabla: Equatable {
    public private(set) var celdas: [[String]]
    public var cabecera: Bool
    /// La celda activa (`fila`, `columna`): «−» quita su fila o su columna.
    public var activa: (fila: Int, columna: Int) = (0, 0)

    /// Los topes del cajetin de la tabla en blanco.
    public static let filasMaximas = 50
    public static let columnasMaximas = 20

    public init(celdas: [[String]], cabecera: Bool) {
        let ancho = max(1, celdas.map(\.count).max() ?? 1)
        var c = celdas.map { $0 + Array(repeating: "", count: ancho - $0.count) }
        if c.isEmpty { c = [[""]] }
        self.celdas = c
        self.cabecera = cabecera
    }

    public static func == (a: RejillaTabla, b: RejillaTabla) -> Bool {
        a.celdas == b.celdas && a.cabecera == b.cabecera
    }

    public var filas: Int { celdas.count }
    public var columnas: Int { celdas.first?.count ?? 0 }

    public mutating func poner(_ texto: String, fila: Int, columna: Int) {
        guard celdas.indices.contains(fila), celdas[fila].indices.contains(columna) else { return }
        celdas[fila][columna] = texto
    }

    /// Una fila en blanco debajo de la activa.
    public mutating func masFila() {
        guard filas < Self.filasMaximas else { return }
        let donde = min(activa.fila + 1, filas)
        celdas.insert(Array(repeating: "", count: columnas), at: donde)
        activa.fila = donde
    }

    /// Quita la fila activa (siempre queda una).
    public mutating func menosFila() {
        guard filas > 1 else { return }
        celdas.remove(at: min(activa.fila, filas - 1))
        activa.fila = min(activa.fila, filas - 1)
    }

    /// Una columna en blanco a la derecha de la activa.
    public mutating func masColumna() {
        guard columnas < Self.columnasMaximas else { return }
        let donde = min(activa.columna + 1, columnas)
        for i in celdas.indices { celdas[i].insert("", at: donde) }
        activa.columna = donde
    }

    /// Quita la columna activa (siempre queda una).
    public mutating func menosColumna() {
        guard columnas > 1 else { return }
        let c = min(activa.columna, columnas - 1)
        for i in celdas.indices { celdas[i].remove(at: c) }
        activa.columna = min(activa.columna, columnas - 1)
    }
}

extension ModeloEditorLienzo {
    /// Si lo elegido es una tabla dibujada (para ofrecer «Editar la tabla…»).
    var hayTablaElegida: Bool { hayElegidos && editor.tablaElegida() != nil }

    /// **«Editar la tabla…»**: el cajetin con sus celdas; al aplicar se
    /// redibuja.
    func editarTabla() {
        guard let vista, let tabla = editor.tablaElegida() else { return }
        _ = editor.terminarTexto()
        let editor = self.editor
        let hoja = UIHostingController(rootView: HojaEditarTabla(
            rejilla: RejillaTabla(celdas: tabla.celdas, cabecera: tabla.cabecera)) { [weak vista] r in
                guard let r else { return }
                if editor.reemplazarTabla(celdas: r.celdas, cabecera: r.cabecera) { vista?.trasCambio() }
            })
        hoja.modalPresentationStyle = .formSheet
        CajetinesLienzo.presentar(hoja, desde: vista)
    }
}

/// El cajetin «Editar la tabla».
struct HojaEditarTabla: View {
    @State var rejilla: RejillaTabla
    /// Con la rejilla al aplicar, `nil` al cancelar.
    let hecho: (RejillaTabla?) -> Void
    @Environment(\.dismiss) private var cerrar

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 8) {
                    Button("+ Fila") { rejilla.masFila() }
                    Button("− Fila") { rejilla.menosFila() }.disabled(rejilla.filas <= 1)
                    Button("+ Columna") { rejilla.masColumna() }
                    Button("− Columna") { rejilla.menosColumna() }.disabled(rejilla.columnas <= 1)
                    Spacer()
                    Toggle("Cabecera", isOn: $rejilla.cabecera).fixedSize()
                }
                .buttonStyle(.bordered)
                ScrollView([.horizontal, .vertical]) {
                    Grid(horizontalSpacing: 4, verticalSpacing: 4) {
                        ForEach(0 ..< rejilla.filas, id: \.self) { f in
                            GridRow {
                                ForEach(0 ..< rejilla.columnas, id: \.self) { c in
                                    celda(f, c)
                                }
                            }
                        }
                    }
                    .padding(2)
                }
                Text("− quita la fila o la columna de la celda activa.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            .padding()
            .navigationTitle("Editar la tabla")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancelar") {
                        hecho(nil)
                        cerrar()
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Aplicar") {
                        hecho(rejilla)
                        cerrar()
                    }
                    .keyboardShortcut(.defaultAction)
                }
            }
        }
    }

    private func celda(_ f: Int, _ c: Int) -> some View {
        let activa = rejilla.activa.fila == f && rejilla.activa.columna == c
        return TextField("", text: Binding(
            get: { f < rejilla.filas && c < rejilla.columnas ? rejilla.celdas[f][c] : "" },
            set: { rejilla.poner($0, fila: f, columna: c) }))
            .textFieldStyle(.roundedBorder)
            .font(f == 0 && rejilla.cabecera ? .body.bold() : .body)
            .frame(width: 120)
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(activa ? Color.accentColor : .clear, lineWidth: 2))
            .simultaneousGesture(TapGesture().onEnded { rejilla.activa = (f, c) })
    }
}
