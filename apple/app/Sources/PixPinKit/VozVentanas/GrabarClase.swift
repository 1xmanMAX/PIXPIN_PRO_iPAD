// **Grabar una clase o una reunion** (Windows `ventana_chat/grabar_clase.rs`,
// 24-oct-2026): graba el audio todo lo que dure, pone fotos en su minuto y
// al acabar lo deja en el chat y lo pasa a texto.
//
// 1. **Preparar**: en que proyecto se guarda y un boton grande para empezar.
//    ⛔ El iPad no deja grabar el sonido de otras apps (Meet, Zoom) ni
//    capturar su pantalla: se graba **el microfono**, y las «capturas» son
//    **fotos** (la camara, a la pizarra o a la pantalla) o una imagen de
//    Fotos (una captura de pantalla hecha con los botones del iPad).
// 2. **Grabando**: el tiempo, Foto, De Fotos, Pausa, Terminar y ✕ (dos
//    veces para tirar). Sigue grabando con la pantalla bloqueada o con
//    otra app delante (el modo de fondo `audio`); una llamada lo para y al
//    colgar sigue solo.
// 3. **Guardar**: `Nucleo.guardarClase` deja en el chat lo mismo que
//    Windows y el Mac: la nota de voz `Clase 24-10 18h05.m4a` y cada foto
//    detras como captura con su minuto, enlazadas (`capturas` y
//    `enGrabacion`), asi que sincroniza tal cual y se estudia con sus
//    capturas («Estudiar la clase») en los demas aparatos.
// 4. **Pasar a texto**, con el dictado del iPad; «Dejarlo sin texto» lo
//    corta (luego se puede pedir con «Pasar a texto» en la nota).
// 5. **«Abrir para estudiar»** (o ↩): la clase en su pantalla
//    (`EstudiarClase`: el texto, las capturas y los marcadores, que se
//    siguen), como Windows ce6c058.
//
// **Los dos modos** de Windows y el Mac (ce6c058: «Pantalla y sonido» y
// «Audio y capturas») salen arriba en Preparar, pero en el iPad solo se
// puede el segundo: ⛔ no deja grabar la pantalla de otras apps. «Pantalla y
// sonido» se ve apagado y, tocado, dice como hacerlo con el iPad (la
// grabacion de pantalla del Centro de control, y el video al chat). El modo
// que se recuerda (`Nucleo.modoDeGrabar`) no se toca desde aqui.
//
// El sonido pasa por **el motor de audio de Windows** (38204cb y 06f842c,
// `MicrofonoConMezcla` de `Voz/MezclaVoz.swift`, como el Mac): remuestreo a
// 48 kHz con la deriva de la tarjeta, paso alto y realce de la voz, y el
// limitador; en pausa (o mientras una llamada corta el microfono) el reloj
// no corre, asi que los minutos de las fotos casan con lo grabado. El motor
// de video nuevo (perfiles Ligero/Normal/Nítido, cdad692) no tiene uso aqui:
// el iPad no deja grabar la pantalla de otras apps.

import PhotosUI
import PixPinCore
import SwiftUI
import UIKit

enum GrabarClase {
    static let nombreVentanita = "grabar-clase"
    /// Lo que espera el segundo toque en ✕ para tirar la grabacion.
    static let tirarEspera: TimeInterval = 4

    /// «Clase 25 oct 10:30» (`clase-nombre`).
    static func nombre(_ fecha: Date, calendario: Calendar = .current) -> String {
        let meses = ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic"]
        let c = calendario.dateComponents([.day, .month, .hour, .minute], from: fecha)
        let mes = meses[max(min((c.month ?? 1) - 1, 11), 0)]
        return String(format: "Clase %d %@ %d:%02d", c.day ?? 1, mes, c.hour ?? 0, c.minute ?? 0)
    }

    /// El pie de una foto: en que minuto de la clase se hizo.
    static func pie(ms: Int64) -> String { "En el \(FormatoVoz.marca(ms: ms)) de la clase" }

    /// Lo que se dice al acabar (`clase-lista-con-texto`/`-sin-texto`,
    /// los del Mac).
    static func resumen(fotos: Int, conTexto: Bool) -> String {
        conTexto
            ? "Con \(fotos) capturas y la transcripción. Ábrela en el chat para verla junto a las imágenes."
            : "Con \(fotos) capturas. Para el texto, pulsa «Pasar a texto» en la nota del chat."
    }
}

@MainActor
final class ModeloClase: ObservableObject {
    enum Fase: Equatable {
        case preparar, grabando, guardando, pasandoATexto
        case lista(titulo: String, detalle: String)
    }

    struct Foto {
        var ms: Int64
        var datos: Data
        var ext: String
    }

    @Published private(set) var fase = Fase.preparar
    @Published var destino: String
    @Published private(set) var pausado = false
    @Published private(set) var fotos: [Foto] = []
    @Published private(set) var abriendo = false
    @Published var camara = false
    @Published var deFotos: [PhotosPickerItem] = []
    /// Lo que se graba: en el iPad, siempre el audio con capturas (fotos).
    let modo: ModoDeGrabar = .capturas
    let aviso = AvisoCorto()
    let almacen: Almacen
    private let mic = MicrofonoSuelto()
    private var tirarHasta: Date?
    private var pasando: Task<Void, Never>?
    private(set) var guardadaEn: (chat: String, mensaje: String)?
    /// Las capturas que entraron en el chat (`ClaseGuardada.capturas`).
    private var guardadas = 0
    var cerrar: (() -> Void)?

    init(almacen: Almacen, chat: String) {
        self.almacen = almacen
        destino = chat
    }

    var llevadoMs: Int64 { mic.llevadoMs }
    var desde: Date? { mic.desde }

    /// Los proyectos donde se puede guardar: el elegido primero.
    var proyectos: [FichaChat] {
        let todos = almacen.chats
        return todos.filter { $0.id == destino } + todos.filter { $0.id != destino }
    }

    var nombreDestino: String { almacen.chats.first { $0.id == destino }?.nombre ?? "este chat" }

    func empezar() {
        guard fase == .preparar, !abriendo else { return }
        abriendo = true
        let fichero = Ventanita.carpeta("clase").appendingPathComponent("clase-\(Ventanita.ahoraMs).m4a")
        _ = Task<Void, Never> {
            do {
                try await mic.empezar(en: fichero)
                fotos = []
                pausado = false
                fase = .grabando
            } catch {
                aviso.decir(MicrofonoSuelto.explicar(error))
            }
            abriendo = false
        }
    }

    /// Uno de los dos modos del selector de arriba: el de la pantalla no se
    /// puede en el iPad y se dice como hacerlo.
    func elegir(_ m: ModoDeGrabar) {
        guard fase == .preparar, m != modo else { return }
        aviso.decir("El iPad no deja grabar la pantalla de otras apps: usa «Grabar pantalla» del Centro de control y manda el vídeo al chat",
                    segundos: 5)
    }

    func pausa() {
        guard fase == .grabando else { return }
        if pausado {
            if mic.seguir() { pausado = false } else { aviso.decir("No se pudo volver a abrir el micrófono") }
        } else {
            mic.pausar()
            pausado = true
            aviso.decir("En pausa: no se graba nada")
        }
    }

    /// Una foto en este minuto.
    func ponerFoto(_ datos: Data, ext: String) {
        guard fase == .grabando else { return }
        let ms = mic.llevadoMs
        fotos.append(Foto(ms: ms, datos: datos, ext: ext))
        aviso.decir("Foto guardada en el \(FormatoVoz.marca(ms: ms))", segundos: 2.6)
    }

    func ponerFoto(_ imagen: UIImage) {
        guard let datos = imagen.jpegData(compressionQuality: 0.85) else { return }
        ponerFoto(datos, ext: "jpg")
    }

    /// Lo elegido en Fotos, en su orden.
    func traerDeFotos(_ elegidos: [PhotosPickerItem]) {
        guard !elegidos.isEmpty else { return }
        deFotos = []
        _ = Task<Void, Never> {
            for e in elegidos {
                guard let datos = try? await e.loadTransferable(type: Data.self) else { continue }
                let ext = e.supportedContentTypes.first?.preferredFilenameExtension ?? "jpg"
                ponerFoto(datos, ext: ext)
            }
        }
    }

    /// ✕: el primer toque avisa, el segundo tira (`clase-tirar`).
    func tirar() {
        if let hasta = tirarHasta, hasta > Date() {
            mic.cancelar()
            cerrar?()
            return
        }
        tirarHasta = Date().addingTimeInterval(GrabarClase.tirarEspera)
        aviso.decir("Pulsa ✕ otra vez para tirar la grabación", segundos: GrabarClase.tirarEspera)
    }

    func terminar() {
        guard fase == .grabando else { return }
        fase = .guardando
        let (almacen, chat, fotos) = (self.almacen, destino, self.fotos)
        let nombreProyecto = nombreDestino
        _ = Task<Void, Never> {
            guard let nota = await mic.parar() else { return }
            let corta = nota.duracionMs < FormatoVoz.minimoMs
            guard !corta || !fotos.isEmpty else {
                try? FileManager.default.removeItem(at: nota.url)
                aviso.decir("Demasiado corta: mantén pulsado un poco más.")
                fase = .preparar
                return
            }
            // Lo mismo que Windows y el Mac (`Nucleo.guardarClase`): la nota
            // `Clase 24-10 18h05.m4a`, cada foto como `clase-<ms>-<n>.png` con
            // su pie y enlazadas (`capturas`, `enGrabacion`). Las fotos van en
            // PNG, como las capturas de Windows.
            let carpeta = Ventanita.carpeta("clase")
            var lista: [CapturaClase] = []
            for (i, f) in fotos.enumerated() {
                let datos = f.ext.lowercased() == "png" ? f.datos : (UIImage(data: f.datos)?.pngData() ?? f.datos)
                let url = carpeta.appendingPathComponent("captura-\(Ventanita.ahoraMs)-\(i + 1).png")
                guard (try? datos.write(to: url)) != nil else { continue }
                lista.append(CapturaClase(ms: f.ms, rutaPng: url.path))
            }
            let rutaAudio = corta ? nil : nota.url.path
            let nucleo = almacen.nucleo
            let desfase = Int64(TimeZone.current.secondsFromGMT()) * 1000
            let hecho = await Task.detached(priority: .userInitiated) { () -> Result<ClaseGuardada, Error> in
                Result {
                    try nucleo.guardarClase(chat: chat, rutaAudio: rutaAudio, duracionMs: nota.duracionMs,
                                            picos: nota.picos, rutaVideo: nil, capturas: lista,
                                            desfaseMs: desfase)
                }
            }.value
            try? FileManager.default.removeItem(at: nota.url)
            for c in lista { try? FileManager.default.removeItem(atPath: c.rutaPng) }
            guard case let .success(clase) = hecho else {
                aviso.decir("No se pudo grabar la nota de voz.")
                fase = .preparar
                return
            }
            let nuevos = [clase.voz].compactMap { $0 } + clase.capturas
            if almacen.mensajes[chat] != nil { almacen.anadir(nuevos, en: chat) }
            await almacen.cargarChats()
            guardadas = clase.capturas.count
            guard let voz = clase.voz else {
                var texto = "Demasiado corta: mantén pulsado un poco más."
                if guardadas > 0 { texto += " · Lo demás se guardó: \(guardadas) capturas" }
                aviso.decir(texto)
                fase = .preparar
                return
            }
            ClaseEnElChat.olvidar(chat: chat, mensaje: voz.id)
            guardadaEn = (chat, voz.id)
            fase = .pasandoATexto
            pasando = Task<Void, Never> {
                let bien = await almacen.transcribir(voz, en: chat)
                guard !Task.isCancelled else { return }
                self.acabar(proyecto: nombreProyecto, fotos: self.guardadas, conTexto: bien)
            }
        }
    }

    /// «Dejarlo sin texto»: se corta el reconocedor.
    func sinTexto() {
        pasando?.cancel()
        pasando = nil
        acabar(proyecto: nombreDestino, fotos: guardadas, conTexto: false)
    }

    private func acabar(proyecto: String, fotos: Int, conTexto: Bool) {
        fase = .lista(titulo: "Guardada en «\(proyecto)»", detalle: GrabarClase.resumen(fotos: fotos, conTexto: conTexto))
    }

    /// **«Abrir para estudiar»**: la clase recien guardada en su pantalla, y
    /// la ventanita se va.
    func estudiar() {
        guard let g = guardadaEn else { return }
        let almacen = self.almacen
        cerrar?()
        _ = Task<Void, Never> {
            try? await Task.sleep(for: .milliseconds(400))
            EstudiarClase.abrir(chat: g.chat, mensaje: g.mensaje, ms: 0, almacen: almacen)
        }
    }

    /// Ir a la nota en el chat.
    func verEnElChat() {
        guard let g = guardadaEn else {
            cerrar?()
            return
        }
        cerrar?()
        _ = Task<Void, Never> {
            try? await Task.sleep(for: .milliseconds(400))
            Navegacion.compartida.irAlChat(g.chat, mensaje: g.mensaje)
        }
    }

    /// Cerrar a medias tira lo grabado (con ✕ se pregunta antes).
    func soltar() {
        mic.cancelar()
    }

    func tecla(_ k: KeyPress) -> Bool {
        guard k.phase == .down else { return false }
        switch k.key {
        case .escape:
            if fase == .grabando { tirar() } else if fase != .guardando { cerrar?() }
        case .space:
            pausa()
        case .return:
            if fase == .preparar {
                empezar()
            } else if fase == .grabando {
                terminar()
            } else if case .lista = fase, guardadaEn != nil {
                estudiar()
            } else {
                return false
            }
        default:
            return false
        }
        return true
    }
}

struct VistaClase: View {
    @ObservedObject var m: ModeloClase
    @ObservedObject private var aviso: AvisoCorto
    @FocusState private var foco: Bool

    init(m: ModeloClase) {
        self.m = m
        aviso = m.aviso
    }

    var body: some View {
        VStack(spacing: 0) {
            switch m.fase {
            case .preparar: preparar
            case .grabando: grabando
            case .guardando: esperando("Guardando la clase…", boton: false)
            case .pasandoATexto: esperando("Pasando a texto…", boton: true)
            case let .lista(titulo, detalle): lista(titulo, detalle)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(ColoresVoz.fondo.ignoresSafeArea())
        .tecladoDeVentanita($foco) { k in m.tecla(k) }
        .fullScreenCover(isPresented: $m.camara) {
            CamaraChat(alHacer: { m.ponerFoto($0) }, alAcabar: { m.camara = false }).ignoresSafeArea()
        }
        .onChange(of: m.deFotos) { _, elegidos in m.traerDeFotos(elegidos) }
    }

    private var avisoAbajo: some View {
        Text(aviso.texto ?? " ").font(.system(size: 14)).foregroundStyle(ColoresVoz.dorado)
            .multilineTextAlignment(.center)
    }

    private var preparar: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                BotonVentanita("←", ancho: 44) { m.cerrar?() }.accessibilityLabel("Cerrar")
                Spacer()
            }
            .padding(.top, 16)
            Text("Grabar clase o reunión").font(.system(size: 26)).foregroundStyle(ColoresVoz.texto).padding(.top, 20)
            Text("Graba el audio y pon fotos en su minuto")
                .font(.system(size: 15)).foregroundStyle(ColoresVoz.apagado).padding(.top, 10)
            selectorDeModo.padding(.top, 20)
            VStack(alignment: .leading, spacing: 10) {
                Label("Micrófono", systemImage: "mic.fill").font(.system(size: 17)).foregroundStyle(ColoresVoz.texto)
                Text("El iPad no deja grabar el sonido de otras apps (Meet, Zoom…): pon el altavoz cerca. Sigue grabando con la pantalla bloqueada.")
                    .font(.system(size: 13)).foregroundStyle(ColoresVoz.apagado)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(ColoresVoz.cristal, in: RoundedRectangle(cornerRadius: 12))
            .padding(.top, 14)
            SwiftUI.Menu {
                ForEach(m.proyectos, id: \.id) { p in
                    Button { m.destino = p.id } label: {
                        if p.id == m.destino { Label(p.nombre, systemImage: "checkmark") } else { Text(p.nombre) }
                    }
                }
            } label: {
                HStack {
                    Image(systemName: "folder")
                    Text("Guardar en: \(m.nombreDestino)").lineLimit(1)
                    Spacer()
                    Image(systemName: "chevron.up.chevron.down").font(.system(size: 12))
                }
                .font(.system(size: 16)).foregroundStyle(ColoresVoz.texto)
                .padding(.horizontal, 14).frame(height: 48)
                .background(ColoresVoz.boton, in: RoundedRectangle(cornerRadius: 10))
            }
            .padding(.top, 14)
            Spacer()
            avisoAbajo.frame(maxWidth: .infinity).padding(.bottom, 10)
            BotonVentanita(m.abriendo ? "Abriendo el audio…" : "Empezar a grabar", color: ColoresVoz.rojo,
                           ancho: nil, alto: 56) { m.empezar() }
                .frame(maxWidth: .infinity)
                .padding(.bottom, 30)
        }
        .padding(.horizontal, 40)
        .frame(maxWidth: 680)
    }

    /// **El selector de modo**: dos mitades, la elegida en azul. «Pantalla
    /// y sonido», apagada: el iPad no deja.
    private var selectorDeModo: some View {
        HStack(spacing: 0) {
            mitadDelModo(.pantalla, "video", "Pantalla y sonido")
            mitadDelModo(.capturas, "photo", "Audio y capturas")
        }
        .padding(3)
        .frame(height: 44)
        .background(ColoresVoz.cristal, in: RoundedRectangle(cornerRadius: 12))
    }

    private func mitadDelModo(_ modo: ModoDeGrabar, _ simbolo: String, _ rotulo: String) -> some View {
        let elegido = m.modo == modo
        return Button { m.elegir(modo) } label: {
            HStack(spacing: 6) {
                Image(systemName: simbolo).font(.system(size: 14))
                Text(rotulo).font(.system(size: 15)).lineLimit(1)
            }
            .foregroundStyle(elegido ? Color.white : ColoresVoz.apagado)
            .opacity(elegido ? 1 : 0.6)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(elegido ? ColoresVoz.azul : Color.clear, in: RoundedRectangle(cornerRadius: 9))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(elegido ? .isSelected : [])
    }

    private var grabando: some View {
        VStack(spacing: 22) {
            Spacer()
            if let desde = m.desde {
                TimelineView(.periodic(from: desde, by: 1)) { _ in
                    Text(FormatoVoz.marca(ms: m.llevadoMs))
                        .font(.system(size: 64, weight: .light).monospacedDigit())
                        .foregroundStyle(m.pausado ? ColoresVoz.apagado : ColoresVoz.rojo)
                }
            }
            Text(m.pausado ? "En pausa: no se graba nada" : "● Grabando en «\(m.nombreDestino)»")
                .font(.system(size: 16)).foregroundStyle(m.pausado ? ColoresVoz.dorado : ColoresVoz.texto)
            Text(m.fotos.isEmpty ? "«Foto» guarda una imagen en este minuto"
                 : "Fotos: \(m.fotos.count) · \(m.fotos.map { FormatoVoz.marca(ms: $0.ms) }.joined(separator: ", "))")
                .font(.system(size: 14)).foregroundStyle(ColoresVoz.apagado).lineLimit(2)
                .multilineTextAlignment(.center)
            HStack(spacing: 12) {
                BotonVentanita("Foto", ancho: 130, alto: 52) { m.camara = true }
                    .disabled(!UIImagePickerController.isSourceTypeAvailable(.camera))
                PhotosPicker(selection: $m.deFotos, maxSelectionCount: 10, matching: .images) {
                    Text("De Fotos").font(.system(size: 16)).foregroundStyle(ColoresVoz.texto)
                        .frame(width: 130, height: 52)
                        .background(ColoresVoz.boton, in: RoundedRectangle(cornerRadius: 8))
                }
                BotonVentanita(m.pausado ? "Seguir" : "Pausa", ancho: 130, alto: 52) { m.pausa() }
            }
            Spacer()
            avisoAbajo
            HStack(spacing: 12) {
                BotonVentanita("✕", ancho: 64, alto: 56) { m.tirar() }.accessibilityLabel("Tirar la grabación")
                BotonVentanita("Terminar", color: ColoresVoz.rojo, ancho: 260, alto: 56) { m.terminar() }
            }
            .padding(.bottom, 30)
        }
        .padding(.horizontal, 24)
    }

    private func esperando(_ texto: String, boton: Bool) -> some View {
        VStack(spacing: 18) {
            ProgressView().controlSize(.large).tint(ColoresVoz.texto)
            Text(texto).font(.system(size: 18)).foregroundStyle(ColoresVoz.texto)
            if boton {
                BotonVentanita("Dejarlo sin texto", ancho: 220) { m.sinTexto() }
            }
        }
    }

    private func lista(_ titulo: String, _ detalle: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "checkmark.circle.fill").font(.system(size: 54)).foregroundStyle(ColoresVoz.verde)
            Text(titulo).font(.system(size: 22)).foregroundStyle(ColoresVoz.texto)
            Text(detalle).font(.system(size: 15)).foregroundStyle(ColoresVoz.apagado)
                .multilineTextAlignment(.center).frame(maxWidth: 520)
            // Con la nota guardada, «Abrir para estudiar» delante: es lo
            // siguiente que se suele querer hacer (↩).
            if m.guardadaEn != nil {
                BotonVentanita("Abrir para estudiar", color: ColoresVoz.azul, ancho: 260, alto: 52) { m.estudiar() }
                    .padding(.top, 8)
            }
            HStack(spacing: 12) {
                BotonVentanita("Ver en el chat", ancho: 180) { m.verEnElChat() }
                BotonVentanita("Listo", ancho: 140) { m.cerrar?() }
            }
            .padding(.top, m.guardadaEn != nil ? 0 : 8)
        }
        .padding(24)
    }
}

extension GrabarClase {
    @MainActor static var abierto: Bool { Ventanita.abierta(nombreVentanita) }

    /// **Abre «Grabar clase»** para guardar en `chat` (se puede cambiar).
    @MainActor
    static func abrir(chat: String, almacen: Almacen) {
        guard !abierto else { return }
        let m = ModeloClase(almacen: almacen, chat: chat)
        Ventanita.abrir(nombreVentanita, alCerrar: { m.soltar() }) { cerrar in
            VistaClase(m: m).onAppear { m.cerrar = cerrar }
        }
    }
}
