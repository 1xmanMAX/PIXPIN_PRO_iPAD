// **Los graficos de un libro de Excel** encima de su hoja, en las mismas
// celdas que en Excel (`pixpin-hojas`, el lector del visor de hojas de
// Windows, que los pinta «algo parecido o al menos un recuadro con el
// titulo»). Se ensenan los valores que Excel guardo con el grafico: no se
// recalculan. Se pintan con Swift Charts: columnas, barras, lineas, areas,
// puntos, tarta y anillo; lo demas, como columnas.

import Charts
import PixPinCore
import SwiftUI

/// Un punto de un grafico, listo para Swift Charts. Puro: se prueba solo.
public struct PuntoGraficoHoja: Identifiable, Equatable, Sendable {
    public let id: Int
    public let serie: String
    public let categoria: String
    /// La posicion de la categoria (para los de puntos).
    public let orden: Int
    public let valor: Double
}

/// Que se pinta.
public enum ClaseGraficoHoja: Equatable, Sendable {
    case columnas, barras, lineas, areas, puntos, tarta, anillo

    public static func de(_ tipo: String) -> ClaseGraficoHoja {
        let t = tipo.lowercased()
        if t.hasPrefix("barh") { return .barras }
        if t.hasPrefix("line") || t.hasPrefix("stock") || t.hasPrefix("radar") { return .lineas }
        if t.hasPrefix("area") { return .areas }
        if t.hasPrefix("scatter") || t.hasPrefix("bubble") { return .puntos }
        if t.hasPrefix("doughnut") { return .anillo }
        if t.hasPrefix("pie") || t.hasPrefix("ofpie") { return .tarta }
        return .columnas
    }
}

/// Los puntos de un grafico: uno por serie y categoria con valor (Excel
/// deja huecos: `NaN`). Sin nombre de categoria, su numero; sin nombre de
/// serie, «Serie N».
public func puntosDeGrafico(categorias: [String], series: [(nombre: String, valores: [Double])]) -> [PuntoGraficoHoja] {
    var v: [PuntoGraficoHoja] = []
    for (s, serie) in series.enumerated() {
        let nombre = serie.nombre.isEmpty ? "Serie \(s + 1)" : serie.nombre
        for (i, x) in serie.valores.enumerated() where x.isFinite {
            let cat = i < categorias.count && !categorias[i].isEmpty ? categorias[i] : "\(i + 1)"
            v.append(PuntoGraficoHoja(id: v.count, serie: nombre, categoria: cat, orden: i, valor: x))
        }
    }
    return v
}

/// **Un grafico** en su caja.
struct GraficoHojaIPad: View {
    let grafico: GraficoHoja

    private var clase: ClaseGraficoHoja { ClaseGraficoHoja.de(grafico.tipo) }

    private var puntos: [PuntoGraficoHoja] {
        puntosDeGrafico(categorias: grafico.categorias,
                        series: grafico.series.map { (nombre: $0.nombre, valores: $0.valores) })
    }

    private var nombres: [String] {
        grafico.series.enumerated().map { $0.element.nombre.isEmpty ? "Serie \($0.offset + 1)" : $0.element.nombre }
    }

    private var colores: [Color] {
        grafico.series.enumerated().map { i, s in
            colorDeHojaHex(s.color) ?? GraficoHojaIPad.porDefecto[i % GraficoHojaIPad.porDefecto.count]
        }
    }

    /// Los del tema de Office (acento 1 a 6).
    static let porDefecto: [Color] = [
        Color(hex: 0x4472C4), Color(hex: 0xED7D31), Color(hex: 0xA5A5A5),
        Color(hex: 0xFFC000), Color(hex: 0x5B9BD5), Color(hex: 0x70AD47),
    ]

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if !grafico.titulo.isEmpty {
                Text(grafico.titulo)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(Color(white: 0.2))
                    .lineLimit(2)
                    .frame(maxWidth: .infinity)
            }
            if puntos.isEmpty {
                Spacer()
                Label("Gráfico sin datos guardados", systemImage: "chart.bar")
                    .font(.caption).foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity)
                Spacer()
            } else {
                dibujo
            }
        }
        .padding(8)
        .background(RoundedRectangle(cornerRadius: 4).fill(Color.white))
        .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(Color(white: 0.8), lineWidth: 1))
        .environment(\.colorScheme, .light)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(grafico.titulo.isEmpty ? "Gráfico" : "Gráfico: \(grafico.titulo)")
    }

    @ViewBuilder private var dibujo: some View {
        let p = puntos
        switch clase {
        case .tarta, .anillo:
            // La primera serie, una porcion por categoria.
            let primera = p.filter { $0.serie == p.first?.serie }
            Chart(primera) { x in
                SectorMark(angle: .value("Valor", max(x.valor, 0)),
                           innerRadius: .ratio(clase == .anillo ? 0.55 : 0),
                           angularInset: 1)
                    .foregroundStyle(by: .value("Categoría", x.categoria))
            }
            .chartForegroundStyleScale(domain: primera.map(\.categoria),
                                       range: primera.indices.map { GraficoHojaIPad.porDefecto[$0 % 6] })
            .chartLegend(position: .trailing)
        case .barras:
            Chart(p) { x in
                BarMark(x: .value("Valor", x.valor), y: .value("Categoría", x.categoria))
                    .foregroundStyle(by: .value("Serie", x.serie))
                    .position(by: .value("Serie", x.serie))
            }
            .chartForegroundStyleScale(domain: nombres, range: colores)
            .chartLegend(nombres.count > 1 ? .visible : .hidden)
        case .lineas:
            Chart(p) { x in
                LineMark(x: .value("Categoría", x.categoria), y: .value("Valor", x.valor))
                    .foregroundStyle(by: .value("Serie", x.serie))
                PointMark(x: .value("Categoría", x.categoria), y: .value("Valor", x.valor))
                    .foregroundStyle(by: .value("Serie", x.serie))
                    .symbolSize(18)
            }
            .chartForegroundStyleScale(domain: nombres, range: colores)
            .chartLegend(nombres.count > 1 ? .visible : .hidden)
        case .areas:
            Chart(p) { x in
                AreaMark(x: .value("Categoría", x.categoria), y: .value("Valor", x.valor))
                    .foregroundStyle(by: .value("Serie", x.serie))
            }
            .chartForegroundStyleScale(domain: nombres, range: colores)
            .chartLegend(nombres.count > 1 ? .visible : .hidden)
        case .puntos:
            Chart(p) { x in
                PointMark(x: .value("X", Double(x.orden + 1)), y: .value("Valor", x.valor))
                    .foregroundStyle(by: .value("Serie", x.serie))
            }
            .chartForegroundStyleScale(domain: nombres, range: colores)
            .chartLegend(nombres.count > 1 ? .visible : .hidden)
        case .columnas:
            Chart(p) { x in
                BarMark(x: .value("Categoría", x.categoria), y: .value("Valor", x.valor))
                    .foregroundStyle(by: .value("Serie", x.serie))
                    .position(by: .value("Serie", x.serie))
            }
            .chartForegroundStyleScale(domain: nombres, range: colores)
            .chartLegend(nombres.count > 1 ? .visible : .hidden)
        }
    }
}

/// Los graficos de la hoja que se ve, cada uno en su caja del contenido de
/// la rejilla (se pone encima de su contenido, arriba a la izquierda).
struct GraficosDeLaHoja: View {
    @ObservedObject var tinta: ControladorTintaHoja

    var body: some View {
        if let r = tinta.rejilla, !tinta.graficos.isEmpty {
            ZStack(alignment: .topLeading) {
                ForEach(Array(tinta.graficos.enumerated()), id: \.offset) { _, g in
                    let caja = r.caja(columna: Int(g.columna), fila: Int(g.fila), dx: CGFloat(g.dx), dy: CGFloat(g.dy),
                                      hasta: g.hastaColumna.map { c in
                                          (columna: Int(c), fila: Int(g.hastaFila ?? g.fila),
                                           dx: CGFloat(g.hastaDx), dy: CGFloat(g.hastaDy))
                                      },
                                      ancho: CGFloat(g.ancho), alto: CGFloat(g.alto))
                    if caja.width > 20, caja.height > 20 {
                        GraficoHojaIPad(grafico: g)
                            .frame(width: caja.width, height: caja.height)
                            .offset(x: caja.minX, y: caja.minY)
                    }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .allowsHitTesting(false)
        }
    }
}
