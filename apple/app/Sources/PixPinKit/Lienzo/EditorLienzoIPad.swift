// **El editor del lienzo del iPad**, a pantalla completa (el «pin sin
// bordes» de Windows): el lienzo llena el visor y encima flotan la barra
// comun de los visores (`BarraVisorIPad`), la barra de herramientas,
// centrada arriba, y el panel de estilo a la izquierda, como en Windows y
// en el Mac (`EditorLienzoView`).
//
// Que lienzo se abre:
// - `mensaje` es un `DIBUJO`: ese (`Nucleo.editarLienzo`);
// - `mensaje` es una `IMAGEN`, o `foto` es la foto de un mensaje del chat:
//   el lienzo de esa foto, como el movil y Windows (`asegurarLienzoDeFoto`:
//   el que ya tenga o uno nuevo con la foto dentro);
// - `foto` suelta (no es de ningun mensaje): un lienzo nuevo en el chat con
//   la foto puesta de imagen, para anotarla;
// - nada: un lienzo nuevo en el chat («Lienzo nuevo» del Mac).
//
// Abrir el editor (leer el `.excalidraw`) va fuera del hilo principal. Se
// guarda solo, 1 s despues del ultimo cambio, al cerrar y al irse la app al
// fondo.

import PhotosUI
import PixPinCore
import SwiftUI
import UIKit

public struct EditorLienzoIPad: View {
    let almacen: Almacen
    let chat: String
    let mensaje: MensajeChat?
    let foto: URL?

    @StateObject private var carga = CargaLienzoIPad()

    public init(almacen: Almacen, chat: String, mensaje: MensajeChat?, foto: URL?) {
        self.almacen = almacen; self.chat = chat; self.mensaje = mensaje; self.foto = foto
    }

    public var body: some View {
        ZStack(alignment: .top) {
            switch carga.estado {
            case .cargando:
                Color(white: 0.97).ignoresSafeArea()
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            case let .fallo(texto):
                Color(white: 0.97).ignoresSafeArea()
                VStack(spacing: 8) {
                    Image(systemName: "exclamationmark.triangle").font(.largeTitle).foregroundStyle(.secondary)
                    Text("No se pudo abrir el lienzo").font(.headline)
                    Text(texto).font(.caption).foregroundStyle(.secondary).multilineTextAlignment(.center)
                }
                .padding()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            case let .listo(modelo):
                EditorLienzoVistaIPad(modelo: modelo)
            }
            BarraVisorIPad(tipo: "Lienzo", icono: "scribble.variable", titulo: carga.titulo ?? tituloDePartida) {
                if case let .listo(modelo) = carga.estado {
                    Button { modelo.vista?.encajar() } label: {
                        Image(systemName: "arrow.up.left.and.arrow.down.right")
                    }
                    .help("Ver todo el dibujo")
                    Button { modelo.compartir() } label: { Image(systemName: "square.and.arrow.up") }
                        .help("Compartir…")
                }
            }
        }
        .task {
            LienzoDelIPad.activar()
            await carga.abrir(almacen: almacen, chat: chat, mensaje: mensaje, foto: foto)
        }
        .onReceive(NotificationCenter.default.publisher(for: UIApplication.didEnterBackgroundNotification)) { _ in
            if case let .listo(modelo) = carga.estado { modelo.guardarAhora(terminarTexto: false) }
        }
        .onDisappear { carga.cerrar() }
        .onAppear { if case let .listo(modelo) = carga.estado { modelo.editor.alFrente() } }
    }

    private var tituloDePartida: String {
        if let m = mensaje, !m.nombre.isEmpty { return m.nombre }
        if let foto { return foto.lastPathComponent }
        return "Lienzo nuevo"
    }
}

/// Abre (o crea) el lienzo fuera del hilo principal y lo tiene mientras el
/// visor vive.
@MainActor
final class CargaLienzoIPad: ObservableObject {
    enum Estado {
        case cargando
        case listo(ModeloEditorLienzo)
        case fallo(String)
    }

    @Published private(set) var estado: Estado = .cargando
    @Published private(set) var titulo: String?
    /// Ya se pidio abrir: volver a aparecer no crea otro lienzo nuevo.
    private var pedido = false

    func abrir(almacen: Almacen, chat: String, mensaje: MensajeChat?, foto: URL?) async {
        guard !pedido else { return }
        pedido = true
        let nucleo = almacen.nucleo
        // 1. Que dibujo: el del mensaje, el de su foto o uno nuevo.
        var referencia: String?
        var nombre = mensaje?.nombre ?? ""
        var fotoSuelta: URL?
        if let m = mensaje, m.clase == "DIBUJO", let r = m.referencia, !r.isEmpty {
            referencia = r
        } else if let m = mensaje ?? fotoDelChat(foto, almacen: almacen, chat: chat), m.clase == "IMAGEN" {
            let id = m.id
            referencia = await Task.detached(priority: .userInitiated) {
                try? nucleo.asegurarLienzoDeFoto(chat: chat, id: id)
            }.value
            if nombre.isEmpty { nombre = m.nombre }
            if referencia == nil { fotoSuelta = foto }
        } else {
            fotoSuelta = foto
        }
        if referencia == nil {
            guard let nuevo = await almacen.crearLienzo(en: chat), let r = nuevo.referencia else {
                estado = .fallo(almacen.error ?? "No se pudo crear el lienzo en este chat.")
                return
            }
            referencia = r
            if nombre.isEmpty { nombre = nuevo.nombre }
        }
        guard let referencia else { return }
        // 2. Abrirlo para editar, con sus fotos (y la suelta dentro).
        let hecho = await Task.detached(priority: .userInitiated) { () -> Result<(EditorLienzo, [UInt64: URL]), Error> in
            do {
                let editor = try nucleo.editarLienzo(chat: chat, dibujo: referencia)
                if let fotoSuelta { try CargaLienzoIPad.ponerFoto(fotoSuelta, en: editor) }
                var fotos: [UInt64: URL] = [:]
                for f in editor.imagenes() { fotos[f.idObjeto] = URL(fileURLWithPath: f.ruta) }
                return .success((editor, fotos))
            } catch {
                return .failure(error)
            }
        }.value
        switch hecho {
        case let .success((editor, fotos)):
            let t = nombre.isEmpty ? (fotoSuelta?.lastPathComponent ?? "Lienzo") : nombre
            titulo = t
            let modelo = ModeloEditorLienzo(editor: editor, referencia: referencia, fotos: fotos,
                                            nucleo: nucleo, titulo: t)
            if fotoSuelta != nil {
                // Con la foto puesta, a anotarla con el lapiz; y la foto se
                // guarda ya (el editor sabe que tiene cambios sin guardar).
                modelo.elegir("lapiz")
                modelo.guardarAhora(terminarTexto: false)
            }
            editor.alFrente()
            estado = .listo(modelo)
        case let .failure(e):
            estado = .fallo("\(e)")
        }
    }

    /// El mensaje `IMAGEN` del chat (de lo cargado) cuyo fichero es `foto`.
    private func fotoDelChat(_ foto: URL?, almacen: Almacen, chat: String) -> MensajeChat? {
        guard let foto else { return nil }
        let buscada = foto.standardizedFileURL.path
        return almacen.mensajes[chat]?.last { m in
            guard m.clase == "IMAGEN", let r = m.ruta,
                  let u = almacen.urlResuelta(deRuta: r, en: chat) else { return false }
            return u.standardizedFileURL.path == buscada
        }
    }

    /// **La foto como imagen del lienzo**, a su tamano (un pixel, una
    /// unidad del mundo) desde (0, 0), como al abrir una foto en Windows.
    nonisolated static func ponerFoto(_ url: URL, en editor: EditorLienzo) throws {
        guard let png = ImagenPegadaLienzo.png(de: .fichero(url)),
              let tam = ImagenPegadaLienzo.tamano(de: png) else { return }
        // `pegarImagen` reduce lo que no cabe en el 80 % de lo visible: se le
        // da una vista donde cabe entera.
        _ = try editor.pegarImagen(png: png, centroX: Float(tam.width / 2), centroY: Float(tam.height / 2),
                                   anchoVista: Float(tam.width / 0.8) + 1, altoVista: Float(tam.height / 0.8) + 1)
    }

    func cerrar() {
        if case let .listo(modelo) = estado { modelo.cerrar() }
    }
}

/// La vista de UIKit dentro de SwiftUI.
struct LienzoEditableIPad: UIViewRepresentable {
    let modelo: ModeloEditorLienzo

    func makeUIView(context: Context) -> VistaEditorLienzoIPad {
        VistaEditorLienzoIPad(modelo: modelo)
    }

    func updateUIView(_ vista: VistaEditorLienzoIPad, context: Context) {}
}

/// **El editor**: el lienzo llena el visor y encima flotan la barra de
/// herramientas, centrada arriba bajo la barra del visor, y el panel de
/// estilo a la izquierda.
struct EditorLienzoVistaIPad: View {
    @ObservedObject var modelo: ModeloEditorLienzo
    /// La foto elegida con el boton de imagen (el selector de Fotos).
    @State private var fotoElegida: PhotosPickerItem?
    /// Lo que ocupa arriba la barra comun del visor.
    static let bajoLaBarraDelVisor: CGFloat = 58

    var body: some View {
        GeometryReader { geo in
            let botones = BotonBarraLienzo.barraIPad
            let corte = BotonBarraLienzo.cortePara(ancho: geo.size.width - 2 * MedidasBarraLienzo.distanciaBorde,
                                                   en: botones)
            let filas: CGFloat = corte < botones.count ? 2 : 1
            let arriba = EditorLienzoVistaIPad.bajoLaBarraDelVisor
            ZStack(alignment: .top) {
                LienzoEditableIPad(modelo: modelo)
                    .ignoresSafeArea()
                if modelo.desplegado != nil {
                    // Un toque fuera del desplegable lo cierra.
                    Color.black.opacity(0.001)
                        .onTapGesture { modelo.desplegado = nil }
                }
                BarraHerramientasLienzo(modelo: modelo, botones: botones, corte: corte)
                    .padding(.top, arriba)
                    .padding(.horizontal, MedidasBarraLienzo.distanciaBorde)
                if !modelo.panelEscondido,
                   modelo.hayElegidos || !HerramientaLienzo.sinEstilo.contains(modelo.herramienta) {
                    PanelEstiloLienzo(modelo: modelo)
                        .padding(.leading, MedidasBarraLienzo.distanciaBorde)
                        .padding(.top, arriba + MedidasBarraLienzo.yPanel - MedidasBarraLienzo.distanciaBorde
                                 + (filas - 1) * (MedidasBarraLienzo.lado + MedidasBarraLienzo.relleno))
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                }
                if let error = modelo.error {
                    Text(error).font(.caption).foregroundStyle(.red).lineLimit(2)
                        .padding(8)
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
                        .allowsHitTesting(false)
                }
                AtajosLienzo(modelo: modelo)
            }
        }
        .overlayPreferenceValue(AnclasGruposLienzo.self) { anclas in
            MenuGrupoLienzo(modelo: modelo, anclas: anclas)
        }
        .photosPicker(isPresented: $modelo.pidiendoImagen, selection: $fotoElegida, matching: .images)
        .onChange(of: fotoElegida) { _, item in
            guard let item else { return }
            fotoElegida = nil
            Task {
                if let datos = try? await item.loadTransferable(type: Data.self) {
                    modelo.vista?.meterImagen(datos)
                }
            }
        }
    }
}
