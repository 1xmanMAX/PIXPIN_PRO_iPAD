// Los widgets de PixPin (pantalla de inicio y bloqueo del iPad): las
// ultimas capturas y las tareas de hoy. La alternativa del iPad al centro
// de la bandeja de Windows (`centro_bandeja/`), que iOS no deja tener.
//
// No leen la carpeta de datos ni llevan el nucleo: la app les deja
// `widgets.json` y las miniaturas en el buzon del App Group
// (`BuzonCompartido`, el mismo fichero que compila la app; lo escribe
// `Sistema/DatosDeWidgetsIPad.swift`). Tocar un widget abre PixPin en la
// Galeria o en Tareas (`pixpinmax://capturas`, `pixpinmax://tareas`).

import SwiftUI
import UIKit
import WidgetKit

@main
struct WidgetsPixPin: WidgetBundle {
    var body: some Widget {
        WidgetCapturas()
        WidgetTareasDeHoy()
    }
}

// MARK: - Lo comun

struct EntradaPixPin: TimelineEntry {
    let date: Date
    let datos: DatosDeWidgets?
    /// Si hay buzon (sin App Group no hay nada que ensenar).
    let hayBuzon: Bool
}

struct ProveedorPixPin: TimelineProvider {
    func placeholder(in context: Context) -> EntradaPixPin {
        EntradaPixPin(date: Date(), datos: DatosDeWidgets(escrita: 0, capturas: [], tareasDeHoy: [
            .init(titulo: "Llamar al cliente", proyecto: "Casa Rosales", hecha: false),
            .init(titulo: "Revisar el plano", proyecto: "Tesis", hecha: false),
        ]), hayBuzon: true)
    }

    func getSnapshot(in context: Context, completion: @escaping (EntradaPixPin) -> Void) {
        completion(context.isPreview ? placeholder(in: context) : leer())
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<EntradaPixPin>) -> Void) {
        // La app pide releer al cambiar; ademas, cada hora (y al empezar el
        // dia, que cambia lo que es «hoy»).
        let ahora = Date()
        let manana = Calendar.current.startOfDay(for: ahora.addingTimeInterval(86_400))
        let siguiente = min(ahora.addingTimeInterval(3600), manana)
        completion(Timeline(entries: [leer()], policy: .after(siguiente)))
    }

    private func leer() -> EntradaPixPin {
        guard let raiz = BuzonCompartido.raiz() else {
            return EntradaPixPin(date: Date(), datos: nil, hayBuzon: false)
        }
        return EntradaPixPin(date: Date(), datos: BuzonCompartido.leerWidgets(en: raiz), hayBuzon: true)
    }
}

/// El azul de PixPin (`Paleta.v2.acento` de la app, 0x0A84FF).
private let acento = Color(red: 0x0A / 255, green: 0x84 / 255, blue: 0xFF / 255)

private struct Vacio: View {
    let icono: String
    let texto: String

    var body: some View {
        VStack(spacing: 6) {
            Image(systemName: icono).font(.title2).foregroundStyle(acento)
            Text(texto).font(.caption).foregroundStyle(.secondary).multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - Ultimas capturas

struct WidgetCapturas: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "io.github.1xmanmax.pixpin.capturas", provider: ProveedorPixPin()) { e in
            VistaCapturas(entrada: e)
                .containerBackground(.fill.tertiary, for: .widget)
                .widgetURL(URL(string: "pixpinmax://capturas"))
        }
        .configurationDisplayName("Últimas capturas")
        .description("Las capturas más recientes de tu galería de PixPin.")
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
    }
}

struct VistaCapturas: View {
    let entrada: EntradaPixPin
    @Environment(\.widgetFamily) private var familia

    private var cuantas: Int {
        switch familia {
        case .systemSmall: 1
        case .systemMedium: 3
        default: 6
        }
    }

    var body: some View {
        let capturas = Array((entrada.datos?.capturas ?? []).prefix(cuantas))
        VStack(alignment: .leading, spacing: 6) {
            Label("Capturas", systemImage: "photo.on.rectangle")
                .font(.caption.weight(.semibold))
                .foregroundStyle(acento)
            if capturas.isEmpty {
                Vacio(icono: "camera", texto: entrada.hayBuzon ? "Aún no hay capturas." : "Abre PixPin para ver tus capturas.")
            } else {
                let columnas = Array(repeating: GridItem(.flexible(), spacing: 6), count: familia == .systemSmall ? 1 : 3)
                LazyVGrid(columns: columnas, spacing: 6) {
                    ForEach(capturas, id: \.miniatura) { c in
                        Miniatura(nombre: c.miniatura)
                    }
                }
                Spacer(minLength: 0)
            }
        }
    }
}

private struct Miniatura: View {
    let nombre: String

    var body: some View {
        Group {
            if let raiz = BuzonCompartido.raiz(),
               let foto = UIImage(contentsOfFile: DatosDeWidgets.carpetaMiniaturas(en: raiz).appendingPathComponent(nombre).path) {
                Image(uiImage: foto).resizable().scaledToFill()
            } else {
                Color.gray.opacity(0.2)
            }
        }
        .frame(minHeight: 50, maxHeight: 110)
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
    }
}

// MARK: - Tareas de hoy

struct WidgetTareasDeHoy: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "io.github.1xmanmax.pixpin.tareas-de-hoy", provider: ProveedorPixPin()) { e in
            VistaTareas(entrada: e)
                .containerBackground(.fill.tertiary, for: .widget)
                .widgetURL(URL(string: "pixpinmax://tareas"))
        }
        .configurationDisplayName("Tareas de hoy")
        .description("Lo que tienes que hacer hoy en tus proyectos.")
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
    }
}

struct VistaTareas: View {
    let entrada: EntradaPixPin
    @Environment(\.widgetFamily) private var familia

    private var cuantas: Int {
        switch familia {
        case .systemSmall: 3
        case .systemMedium: 4
        default: 8
        }
    }

    var body: some View {
        let tareas = Array((entrada.datos?.tareasDeHoy ?? []).prefix(cuantas))
        VStack(alignment: .leading, spacing: 6) {
            Label("Hoy", systemImage: "checklist")
                .font(.caption.weight(.semibold))
                .foregroundStyle(acento)
            if tareas.isEmpty {
                Vacio(icono: "checkmark.circle", texto: entrada.hayBuzon ? "Nada para hoy." : "Abre PixPin para ver tus tareas.")
            } else {
                ForEach(Array(tareas.enumerated()), id: \.offset) { _, t in
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        Image(systemName: "circle").font(.caption2).foregroundStyle(.secondary)
                        VStack(alignment: .leading, spacing: 0) {
                            Text(t.titulo).font(.caption).lineLimit(familia == .systemSmall ? 2 : 1)
                            if familia != .systemSmall {
                                Text(t.proyecto).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                            }
                        }
                    }
                }
                Spacer(minLength: 0)
            }
        }
    }
}
