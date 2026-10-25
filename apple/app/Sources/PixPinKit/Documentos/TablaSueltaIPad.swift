// **Un CSV o un TSV suelto** (fuera de un chat): la tabla para ver, con la
// cara de la hoja de calculo (letras de columna, numeros de fila, la
// rejilla), buscar y compartir. Sin chat no hay donde guardar sus tablas,
// asi que no se edita (como el pin de hoja de Windows al abrir un fichero
// que no es del chat). La rejilla es perezosa: solo existen las filas que
// se ven.

import SwiftUI
import UIKit

/// Leer un CSV o un TSV (RFC 4180: comillas, comillas dobladas, saltos de
/// renglon dentro de las comillas). Pura: se prueba sola.
enum TablaCsv {
    /// El separador: el TSV, tabulador; el CSV, el que mas sale en el primer
    /// renglon de `,` y `;` (el de Excel en espanol).
    static func separador(de texto: Substring, ext: String) -> Character {
        if ext == "tsv" { return "\t" }
        let primero = texto.prefix { $0 != "\n" && $0 != "\r" && $0 != "\r\n" }
        let comas = primero.filter { $0 == "," }.count
        let puntos = primero.filter { $0 == ";" }.count
        let tabs = primero.filter { $0 == "\t" }.count
        if tabs > max(comas, puntos) { return "\t" }
        return puntos > comas ? ";" : ","
    }

    /// Las filas con sus celdas. `tope`: cuantas filas como mucho.
    static func leer(_ texto: String, separador sep: Character, tope: Int = 200_000) -> [[String]] {
        var filas: [[String]] = []
        var fila: [String] = []
        var celda = ""
        var entreComillas = false
        var i = texto.startIndex
        while i < texto.endIndex {
            let c = texto[i]
            if entreComillas {
                if c == "\"" {
                    let j = texto.index(after: i)
                    if j < texto.endIndex, texto[j] == "\"" {
                        celda.append("\"")
                        i = j
                    } else {
                        entreComillas = false
                    }
                } else {
                    celda.append(c)
                }
            } else if c == "\"" && celda.isEmpty {
                entreComillas = true
            } else if c == sep {
                fila.append(celda)
                celda = ""
            } else if c == "\n" || c == "\r\n" || c == "\r" {
                fila.append(celda)
                celda = ""
                filas.append(fila)
                fila = []
                if filas.count >= tope { return filas }
            } else {
                celda.append(c)
            }
            i = texto.index(after: i)
        }
        if !celda.isEmpty || !fila.isEmpty {
            fila.append(celda)
            filas.append(fila)
        }
        return filas
    }
}

struct TablaSueltaIPad: View {
    let url: URL
    let titulo: String
    @State private var filas: [[String]]?
    @State private var fallo = false
    @State private var buscando = ""
    @ObservedObject private var noche = ModoNoche.compartido

    private static let altoFila: CGFloat = 30
    private static let anchoNumeros: CGFloat = 52

    var body: some View {
        VStack(spacing: 0) {
            BarraVisorIPad(tipo: "Hoja de cálculo", icono: "tablecells", titulo: titulo, alFijar: {
                if !GestorPinesIPad.compartido.pinear(url: url) { GestorPinesIPad.compartido.pinearLoQueSeVe() }
            }) {
                ShareLink(item: url) { Image(systemName: "square.and.arrow.up") }
                    .help("Compartir")
            }
            if let filas {
                HStack {
                    Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                    TextField("Buscar en la tabla", text: $buscando).textFieldStyle(.plain)
                    Text("\(visibles(filas).count) filas").font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                }
                .padding(.horizontal, 12).frame(height: 36)
                .background(Color(uiColor: .secondarySystemBackground))
                rejilla(filas)
            } else if fallo {
                Spacer()
                Label("No se puede leer este archivo.", systemImage: "exclamationmark.triangle")
                    .foregroundStyle(.secondary)
                Spacer()
            } else {
                Spacer()
                ProgressView()
                Spacer()
            }
        }
        .task {
            let url = self.url
            let leidas = await Task.detached(priority: .userInitiated) { () -> [[String]]? in
                guard let d = try? Data(contentsOf: url) else { return nil }
                guard let t = String(data: d, encoding: .utf8) ?? String(data: d, encoding: .isoLatin1) else { return nil }
                let limpio = t.hasPrefix("\u{FEFF}") ? String(t.dropFirst()) : t
                let sep = TablaCsv.separador(de: Substring(limpio), ext: url.pathExtension.lowercased())
                return TablaCsv.leer(limpio, separador: sep)
            }.value
            if let leidas { filas = leidas } else { fallo = true }
        }
    }

    /// Las filas que se ensenan (con su numero de verdad): todas, o las que
    /// tienen lo buscado.
    private func visibles(_ filas: [[String]]) -> [(Int, [String])] {
        let q = buscando.trimmingCharacters(in: .whitespaces)
        let todas = filas.enumerated().map { ($0.offset, $0.element) }
        guard !q.isEmpty else { return todas }
        return todas.filter { _, f in f.contains { $0.localizedCaseInsensitiveContains(q) } }
    }

    private func anchos(_ filas: [[String]]) -> [CGFloat] {
        let columnas = filas.prefix(500).map(\.count).max() ?? 0
        var v = [CGFloat](repeating: 70, count: columnas)
        for f in filas.prefix(300) {
            for (i, c) in f.enumerated() where i < columnas {
                v[i] = min(max(v[i], CGFloat(c.count) * 8 + 16), 320)
            }
        }
        return v
    }

    private func rejilla(_ filas: [[String]]) -> some View {
        let w = anchos(filas)
        let lineas = Color.primary.opacity(0.12)
        return ScrollView([.horizontal, .vertical]) {
            LazyVStack(alignment: .leading, spacing: 0, pinnedViews: [.sectionHeaders]) {
                Section {
                    ForEach(visibles(filas), id: \.0) { n, f in
                        HStack(spacing: 0) {
                            Text("\(n + 1)").font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                                .frame(width: TablaSueltaIPad.anchoNumeros, height: TablaSueltaIPad.altoFila)
                                .background(Color(uiColor: .secondarySystemBackground))
                            ForEach(0 ..< w.count, id: \.self) { i in
                                let texto = i < f.count ? f[i] : ""
                                Text(texto)
                                    .font(.system(size: 14))
                                    .lineLimit(1)
                                    .frame(width: w[i], height: TablaSueltaIPad.altoFila,
                                           alignment: Double(texto.replacingOccurrences(of: ",", with: ".")) != nil
                                               ? .trailing : .leading)
                                    .padding(.horizontal, 4)
                                    .overlay(Rectangle().stroke(lineas, lineWidth: 0.5))
                                    .textSelection(.enabled)
                            }
                        }
                    }
                } header: {
                    HStack(spacing: 0) {
                        Color(uiColor: .secondarySystemBackground)
                            .frame(width: TablaSueltaIPad.anchoNumeros, height: TablaSueltaIPad.altoFila)
                        ForEach(0 ..< w.count, id: \.self) { i in
                            Text(letrasDeColumna(i)).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                                .frame(width: w[i] + 8, height: TablaSueltaIPad.altoFila)
                                .background(Color(uiColor: .secondarySystemBackground))
                                .overlay(Rectangle().stroke(lineas, lineWidth: 0.5))
                        }
                    }
                }
            }
        }
        .modifier(NocheDelVisor(activo: noche.activo))
    }
}
