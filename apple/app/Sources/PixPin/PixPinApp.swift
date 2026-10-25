// PixPin para iPad: una ventana con la barra lateral de secciones (lo que en
// Windows abre la bandeja) sobre el mismo nucleo y la misma sincronizacion
// que Windows, Mac y Android. Con Stage Manager se pueden abrir varias.

import AppIntents
import PixPinKit
import SwiftUI

@main
struct PixPinApp: App {
    /// Las acciones rapidas del icono (mantener pulsado PixPin en la pantalla de inicio).
    @UIApplicationDelegateAdaptor(DelegadoAppIPad.self) private var delegado
    @StateObject private var entorno = EntornoApp()
    /// La fase de TODA la app (todas las ventanas juntas): con Stage Manager
    /// una ventana puede irse al fondo mientras otra sigue delante, y la
    /// sincronizacion solo se para cuando se va la app entera.
    @Environment(\.scenePhase) private var fase

    init() {
        // Antes de crear el entorno: lo que cada zona pone en marcha al abrir.
        ArranqueOrganizar.apuntar()
        Tema.aplicarAlArrancar()
        SincroDeFondo.activar()
        LienzoDelIPad.activar()
        AsistenteDelIPad.activar()
        CompartirDelIPad.activar()
        DocumentosDelIPad.activar()
        // El buzon de la extension de Compartir y los datos de los widgets.
        SistemaDelIPad.activar()
    }

    var body: some Scene {
        WindowGroup {
            RaizIPad(entorno: entorno)
                .conVentanasDeVisor()
        }
        .onChange(of: fase, initial: true) { _, nueva in
            switch nueva {
            case .active: entorno.alActivarse()
            case .background: entorno.alIrseAlFondo()
            default: break
            }
        }
        .backgroundTask(.appRefresh(SincroDeFondo.identificador)) {
            await SincroDeFondo.alDespertar()
        }
        // Un visor en su propia ventana (Stage Manager).
        WindowGroup(id: VentanasDeVisor.grupo, for: PeticionVentana.self) { $peticion in
            VentanaDeVisor(peticion: peticion, entorno: entorno)
        }
    }
}

/// Las intenciones de PixPinKit: sin esto Siri, Atajos y Spotlight no las ven.
struct PaqueteDeLaApp: AppIntentsPackage {
    static var includedPackages: [any AppIntentsPackage.Type] { [IntencionesPixPin.self] }
}

/// Las frases de Siri y Spotlight (una sola por app, y en la app).
struct AtajosDeLaApp: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(intent: NuevaNotaEnProyecto(),
                    phrases: ["Nueva nota en \(.applicationName)", "Apuntar una nota en \(.applicationName)"],
                    shortTitle: "Nueva nota", systemImageName: "square.and.pencil")
        AppShortcut(intent: NuevaTarea(),
                    phrases: ["Nueva tarea en \(.applicationName)", "Apuntar una tarea en \(.applicationName)"],
                    shortTitle: "Nueva tarea", systemImageName: "checklist")
        AppShortcut(intent: BuscarEnPixPin(), phrases: ["Buscar en \(.applicationName)"],
                    shortTitle: "Buscar", systemImageName: "magnifyingglass")
        AppShortcut(intent: PreguntarAlAsistente(), phrases: ["Preguntar al asistente de \(.applicationName)"],
                    shortTitle: "Preguntar al asistente", systemImageName: "sparkles")
        AppShortcut(intent: GrabarNotaDeVoz(), phrases: ["Grabar una nota de voz en \(.applicationName)"],
                    shortTitle: "Nota de voz", systemImageName: "mic")
        AppShortcut(intent: CapturarConPixPin(),
                    phrases: ["Capturar con \(.applicationName)", "Hacer una captura con \(.applicationName)"],
                    shortTitle: "Capturar", systemImageName: "camera.viewfinder")
        AppShortcut(intent: ApuntarEnElTimeline(), phrases: ["Apuntar en el timeline de \(.applicationName)"],
                    shortTitle: "Timeline", systemImageName: "clock.arrow.circlepath")
        AppShortcut(intent: NuevoLienzo(), phrases: ["Nuevo lienzo en \(.applicationName)"],
                    shortTitle: "Nuevo lienzo", systemImageName: "scribble.variable")
        AppShortcut(intent: AbrirProyecto(), phrases: ["Abrir un proyecto de \(.applicationName)"],
                    shortTitle: "Abrir proyecto", systemImageName: "folder")
        AppShortcut(intent: SincronizarAhora(),
                    phrases: ["Sincronizar \(.applicationName)", "Sincronizar \(.applicationName) ahora"],
                    shortTitle: "Sincronizar ahora", systemImageName: "arrow.triangle.2.circlepath")
    }
}
