// **Los controles del Centro de control** (iPadOS 18, `ControlWidget`):
// la alternativa del iPad a los botones de la bandeja de Windows
// (`centro_bandeja/`) y a sus atajos generales. Cada uno abre PixPin con su
// enlace `pixpinmax://` (`Pedidos/Enlaces.swift`), el mismo verbo de la
// linea de ordenes: Capturar, Nueva nota, Grabar nota de voz y Tareas.
// Se ponen en el Centro de control (y en la pantalla de bloqueo) con
// «Añadir un control» › PixPin.
//
// Grabar pasa por la pregunta de los enlaces (usa el microfono); los demas
// son inofensivos y van directos.

import AppIntents
import SwiftUI
import WidgetKit

/// Abre PixPin con un enlace suyo (lo hace el sistema: `OpenURLIntent`).
struct AbrirEnlacePixPin: AppIntent {
    static let title: LocalizedStringResource = "Abrir PixPin"
    static let isDiscoverable = false

    @Parameter(title: "Enlace")
    var enlace: String

    init() {}

    init(_ enlace: String) {
        self.enlace = enlace
    }

    func perform() async throws -> some IntentResult & OpensIntent {
        let url = URL(string: enlace) ?? URL(string: "pixpinmax://proyectos")!
        return .result(opensIntent: OpenURLIntent(url))
    }
}

struct ControlCapturar: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: "io.github.1xmanmax.pixpin.control.capturar") {
            ControlWidgetButton(action: AbrirEnlacePixPin("pixpinmax://capturar")) {
                Label("Capturar", systemImage: "camera.viewfinder")
            }
        }
        .displayName("Capturar con PixPin")
        .description("Abre la hoja de capturar: cámara, escáner, Fotos o PixPin.")
    }
}

struct ControlNuevaNota: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: "io.github.1xmanmax.pixpin.control.nota") {
            ControlWidgetButton(action: AbrirEnlacePixPin("pixpinmax://nota")) {
                Label("Nueva nota", systemImage: "square.and.pencil")
            }
        }
        .displayName("Nueva nota")
        .description("Una nota nueva en Mensajes guardados.")
    }
}

struct ControlGrabarNota: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: "io.github.1xmanmax.pixpin.control.grabar") {
            ControlWidgetButton(action: AbrirEnlacePixPin("pixpinmax://grabar")) {
                Label("Nota de voz", systemImage: "mic")
            }
        }
        .displayName("Grabar nota de voz")
        .description("Saca el micrófono flotante de PixPin.")
    }
}

struct ControlTareas: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: "io.github.1xmanmax.pixpin.control.tareas") {
            ControlWidgetButton(action: AbrirEnlacePixPin("pixpinmax://tareas")) {
                Label("Tareas", systemImage: "checklist")
            }
        }
        .displayName("Tareas de PixPin")
        .description("Abre las tareas.")
    }
}
