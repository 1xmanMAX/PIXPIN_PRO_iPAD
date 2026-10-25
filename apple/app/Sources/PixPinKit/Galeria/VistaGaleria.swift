// La galeria de capturas en el iPad (`VentanaGaleria` del Mac, la v2 de
// `galeria_capturas.rs` de Windows): la barra comun con el buscador (que
// tambien encuentra el texto de dentro), la rejilla por dias y por paginas
// con los dias que le quedan a cada una, «Seleccionar» para elegir varias
// con su barra abajo, y el aviso con «Deshacer».
//
// Con el dedo: un toque abre la captura (foto en Vista Rapida, video en su
// reproductor); mantenerla pulsada da el menu (abrir, compartir, copiar,
// mandar a un proyecto, anotar en un lienzo, conservar, «7 dias mas» y
// borrar). Con el teclado del iPad: ⌘A elige todas, ⌘Z deshace.
//
// Lo que decide (caducidad, papelera, conservar, buscar) es del nucleo,
// como en el Mac (`ModeloGaleria`, en `Galeria.swift`).

import PixPinCore
import SwiftUI
import UIKit

public struct VistaGaleria: View {
    @StateObject private var modelo: ModeloGaleria
    /// Los proyectos, para «Mandar a un proyecto» (sin observarlo: cada
    /// mensaje nuevo repintaria la rejilla).
    private let proyectos: Almacen
    @State private var eligiendo = false
    @State private var elegidas: Set<String> = []

    public init(almacen: Almacen) {
        _modelo = StateObject(wrappedValue: ModeloGaleria(almacen: almacen))
        proyectos = almacen
    }

    public var body: some View {
        VStack(spacing: 0) {
            Cabecera("Capturas", subtitulo: subtitulo,
                     buscador: BuscadorCabecera($modelo.consulta, pista: "Buscar, también el texto de las capturas")) {
                BotonCabecera(icono: "checkmark.circle", rotulo: eligiendo ? "Listo" : "Seleccionar",
                              activo: eligiendo) {
                    eligiendo.toggle()
                    if !eligiendo { elegidas = [] }
                }
            }
            rejilla
        }
        .ventanaConCabecera()
        .onChange(of: modelo.consulta) { _, _ in modelo.consultaCambiada() }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinTrajoCambios)) { _ in
            Task { await modelo.recargar() }
        }
        .onReceive(NotificationCenter.default.publisher(for: .pixpinCapturasBarridas)) { _ in
            Task { await modelo.recargar() }
        }
        .task { ArranqueOrganizar.apuntar() }
    }

    /// «25 capturas · duran 7 días», como el subtitulo de Windows.
    private var subtitulo: String {
        let n = modelo.enLaCarpeta
        let cuantas = n == 1 ? "1 captura" : "\(n) capturas"
        let dias = AjusteCaducidad.dias
        return dias > 0 ? "\(cuantas) · duran \(dias) días" : cuantas
    }

    private var rejilla: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0, pinnedViews: .sectionHeaders) {
                ForEach(modelo.grupos) { g in
                    Section {
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 170, maximum: 280), spacing: 12)],
                                  spacing: 12) {
                            ForEach(g.capturas, id: \.nombre) { c in
                                celda(c)
                            }
                        }
                        .padding(.bottom, 16)
                    } header: {
                        cabeceraDelDia(g)
                    }
                }
                if modelo.quedanMas {
                    ProgressView().frame(maxWidth: .infinity, minHeight: 60)
                        .task { await modelo.cargarMas() }
                }
            }
            .padding(.horizontal, 20)
            .padding(.bottom, elegidas.isEmpty ? 20 : 110)
        }
        .scrollDismissesKeyboard(.immediately)
        .refreshable { await modelo.recargar() }
        .overlay { vacia }
        .overlay(alignment: .bottom) { abajo }
        .background { atajos }
        .task {
            await modelo.recargar()
            modelo.vigilar()
        }
        .onDisappear { modelo.dejarDeVigilar() }
    }

    private func celda(_ c: CapturaGaleria) -> some View {
        CeldaCaptura(captura: c, elegida: elegidas.contains(c.nombre), eligiendo: eligiendo)
            .onTapGesture {
                if eligiendo { marcar(c.nombre) } else { abrir(c) }
            }
            .contextMenu { menu(para: c) } preview: {
                MiniaturaCaptura(url: modelo.url(de: c), esVideo: c.esVideo)
                    .frame(width: 360, height: 260)
            }
            .draggable(modelo.url(de: c))
    }

    /// «Hoy · jueves 15 oct · 3 capturas» y «Elegir todo el día».
    private func cabeceraDelDia(_ g: ModeloGaleria.Grupo) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text(ModeloGaleria.rotulo(g.fecha).capitalizedPrimera)
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(Paleta.v2.texto)
            Text(g.capturas.count == 1 ? "1 captura" : "\(g.capturas.count) capturas")
                .font(.system(size: 12)).foregroundStyle(Paleta.v2.gris)
            Spacer()
            if eligiendo {
                Button(g.capturas.allSatisfy { elegidas.contains($0.nombre) } ? "Soltar el día" : "Elegir todo el día") {
                    elegirDia(g)
                }
                .buttonStyle(.plain)
                .font(.system(size: 13))
                .foregroundStyle(Paleta.v2.acento)
            }
        }
        .padding(.vertical, 10)
        .frame(maxWidth: .infinity)
        .background(Paleta.v2.fondo)
    }

    @ViewBuilder
    private var vacia: some View {
        if modelo.capturas.isEmpty, !modelo.quedanMas {
            VStack(spacing: 10) {
                Image(systemName: modelo.consulta.isEmpty ? "photo.on.rectangle.angled" : "magnifyingglass")
                    .font(.system(size: 36)).foregroundStyle(Paleta.v2.gris)
                Text(modelo.consulta.isEmpty
                    ? "Todavía no hay capturas. Las que guardes aparecerán aquí y se borran solas a los \(AjusteCaducidad.dias) días, salvo las que conservas."
                    : "Ninguna captura con «\(modelo.consulta)».")
                    .multilineTextAlignment(.center)
                    .foregroundStyle(Paleta.v2.gris)
                    .frame(maxWidth: 380)
            }
            .padding()
        }
    }

    /// Abajo: el aviso (con «Deshacer») y la barra de las elegidas.
    private var abajo: some View {
        VStack(spacing: 8) {
            if let t = modelo.error ?? modelo.aviso {
                AvisoAbajo(texto: t, esError: modelo.error != nil,
                           deshacer: !modelo.ultimoBorrado.isEmpty && modelo.error == nil ? deshacer : nil)
                    .padding(.bottom, elegidas.isEmpty ? 0 : -8)
            }
            if !elegidas.isEmpty { barraDeElegidas.padding(.bottom, 16) }
        }
        .animation(.easeOut(duration: 0.2), value: modelo.aviso)
    }

    /// La barra de las elegidas (`barra_de_elegidas` de Windows).
    private var barraDeElegidas: some View {
        let nombres = elegidasEnOrden
        return HStack(spacing: 8) {
            Text(nombres.count == 1 ? "1 elegida" : "\(nombres.count) elegidas")
                .font(.system(size: 13.5, weight: .medium)).foregroundStyle(Paleta.v2.texto)
                .padding(.horizontal, 8)
            Button("Todas") { elegirTodas() }.buttonStyle(.plain).foregroundStyle(Paleta.v2.acento)
            Divider().frame(height: 24)
            ShareLink(items: urls(nombres)) {
                Label("Compartir", systemImage: "square.and.arrow.up")
                    .font(.system(size: 13.5, weight: .medium))
                    .padding(.horizontal, 12).frame(height: 40)
                    .background(Paleta.v2.caja, in: RoundedRectangle(cornerRadius: 10))
            }
            .foregroundStyle(Paleta.v2.texto)
            BotonCabecera(icono: "pin", rotulo: "Pinear") { pinear(nombres) }
            BotonCabecera(icono: "checkmark.seal", rotulo: "Conservar") { conservar(nombres) }
            BotonCabecera(icono: "trash", rotulo: "Borrar") { borrar(nombres) }
            BotonCabecera(icono: "xmark", ayuda: "Soltar", tenue: true) { soltarEleccion() }
        }
        .padding(8)
        .background(Paleta.v2.tarjetaEncima, in: RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).strokeBorder(Color.white.opacity(0.08)))
        .shadow(color: .black.opacity(0.35), radius: 16, y: 6)
    }

    /// Los atajos del teclado del iPad.
    private var atajos: some View {
        ZStack {
            Button("") { elegirTodas() }.keyboardShortcut("a")
            Button("", action: deshacer)
                .keyboardShortcut("z")
                .disabled(modelo.ultimoBorrado.isEmpty)
        }
        .opacity(0)
        .allowsHitTesting(false)
    }

    private func deshacer() {
        Task { await modelo.deshacer() }
    }

    // MARK: El menu

    /// Mantener pulsada una (`menu_de_captura`): sobre una elegida, vale
    /// para todas las elegidas.
    @ViewBuilder
    private func menu(para c: CapturaGaleria) -> some View {
        let nombres = elegidas.contains(c.nombre) ? elegidasEnOrden : [c.nombre]
        let varias = nombres.count > 1
        if !varias {
            Button { abrir(c) } label: { Label("Abrir", systemImage: "arrow.up.forward.square") }
            Button { Navegacion.compartida.abrir(.vistaRapida(url: modelo.url(de: c))) } label: {
                Label("Vista rápida", systemImage: "eye")
            }
        }
        Button { pinear(nombres) } label: { Label("Pinear", systemImage: "pin") }
        ShareLink(items: urls(nombres)) { Label("Compartir…", systemImage: "square.and.arrow.up") }
        Button { copiar(nombres) } label: { Label("Copiar", systemImage: "doc.on.doc") }
        // «Guardar como…» y «Mostrar en la carpeta» de Windows: en iOS, el
        // selector de Archivos (con copia).
        Button { SalidasDeLaHoja.guardar(urls(nombres)) { _ in } } label: {
            Label("Guardar en Archivos…", systemImage: "folder")
        }
        Menu {
            ForEach(proyectos.chats, id: \.id) { chat in
                Button(chat.nombre) { mandar(nombres, a: chat) }
            }
        } label: {
            Label("Mandar a un proyecto", systemImage: "paperplane")
        }
        if !varias, !c.esVideo {
            Button { anotar(c) } label: { Label("Anotar en un lienzo", systemImage: "pencil.tip.crop.circle") }
        }
        if !eligiendo {
            Button {
                eligiendo = true
                marcar(c.nombre)
            } label: { Label("Seleccionar", systemImage: "checkmark.circle") }
        }
        let caducan = modelo.capturas.contains { nombres.contains($0.nombre) && $0.seVa != nil }
        if caducan {
            Divider()
            Button { conservar(nombres) } label: { Label("Conservar", systemImage: "checkmark.seal") }
            Button { prorrogar(nombres) } label: { Label("Dar 7 días más", systemImage: "calendar.badge.plus") }
        }
        Divider()
        Button(role: .destructive) { borrar(nombres) } label: { Label("Borrar", systemImage: "trash") }
    }

    // MARK: Elegir

    /// Las elegidas en el orden de la rejilla.
    private var elegidasEnOrden: [String] {
        modelo.capturas.map(\.nombre).filter { elegidas.contains($0) }
    }

    private func marcar(_ nombre: String) {
        if elegidas.contains(nombre) { elegidas.remove(nombre) } else { elegidas.insert(nombre) }
    }

    private func elegirDia(_ g: ModeloGaleria.Grupo) {
        let del = Set(g.capturas.map(\.nombre))
        if del.isSubset(of: elegidas) { elegidas.subtract(del) } else { elegidas.formUnion(del) }
    }

    private func elegirTodas() {
        let todas = Set(modelo.capturas.map(\.nombre))
        eligiendo = true
        if todas.isSubset(of: elegidas) { elegidas = [] } else { elegidas = todas }
    }

    private func soltarEleccion() {
        elegidas = []
        eligiendo = false
    }

    // MARK: Acciones

    private func urls(_ nombres: [String]) -> [URL] {
        nombres.compactMap { n in modelo.capturas.first { $0.nombre == n }.map(modelo.url(de:)) }
    }

    /// **Pinear** (`galeria-pinear`): cada una a la capa de pines.
    private func pinear(_ nombres: [String]) {
        let hechas = urls(nombres).filter { GestorPinesIPad.compartido.pinear(url: $0) }.count
        if hechas == 0 {
            modelo.avisar("No se pudo pinear")
        } else {
            modelo.avisar(hechas == 1 ? "Pineada" : "\(hechas) pineadas en la pantalla")
            soltarEleccion()
        }
    }

    /// Abrir: una foto en Vista Rapida, un video en su reproductor (el
    /// visor que toca por la extension, `Navegacion.abrirFichero`).
    private func abrir(_ c: CapturaGaleria) {
        let url = modelo.url(de: c)
        guard FileManager.default.fileExists(atPath: url.path) else {
            modelo.avisar("Ya no está en la carpeta")
            return
        }
        Navegacion.compartida.abrirFichero(url, titulo: c.nombre)
    }

    /// El lienzo nace con la captura, en «Mensajes guardados».
    private func anotar(_ c: CapturaGaleria) {
        guard let guardados = proyectos.chats.first(where: \.esGuardados)?.id else {
            modelo.avisar("No se encuentra «Mensajes guardados»")
            return
        }
        Navegacion.compartida.abrir(.lienzo(chat: guardados, mensaje: nil, foto: modelo.url(de: c)))
    }

    /// Una imagen sola, como imagen (se pega en cualquier sitio); varias (o
    /// un video), como ficheros.
    private func copiar(_ nombres: [String]) {
        let v = urls(nombres)
        guard !v.isEmpty else { return }
        if v.count == 1, let c = modelo.capturas.first(where: { $0.nombre == nombres[0] }), !c.esVideo,
           let imagen = UIImage(contentsOfFile: v[0].path) {
            UIPasteboard.general.image = imagen
            modelo.avisar("Copiada al portapapeles")
        } else {
            UIPasteboard.general.urls = v
            modelo.avisar("\(v.count) copiadas como archivos")
        }
    }

    private func mandar(_ nombres: [String], a chat: FichaChat) {
        let v = urls(nombres)
        Task {
            let m = await proyectos.adjuntar(urls: v, en: chat.id)
            modelo.avisar(m.isEmpty ? "No se pudo mandar" : "Mandada\(m.count == 1 ? "" : "s") a «\(chat.nombre)»")
        }
    }

    private func conservar(_ nombres: [String]) {
        guard !nombres.isEmpty else { return }
        Task { await modelo.conservar(nombres) }
    }

    private func prorrogar(_ nombres: [String]) {
        Task { await modelo.prorrogar(nombres) }
    }

    private func borrar(_ nombres: [String]) {
        guard !nombres.isEmpty else { return }
        elegidas.subtract(nombres)
        if elegidas.isEmpty { eligiendo = false }
        Task { await modelo.borrar(nombres) }
    }
}

/// Una captura en la rejilla: su miniatura, la chapita de los dias que le
/// quedan (gris, naranja, rojo; verde si esta conservada), la marca de
/// elegida y la hora.
struct CeldaCaptura: View {
    let captura: CapturaGaleria
    let elegida: Bool
    var eligiendo = false

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            MiniaturaCaptura(url: URL(fileURLWithPath: captura.ruta), esVideo: captura.esVideo)
                .frame(maxWidth: .infinity)
                .frame(height: 140)
                .clipShape(RoundedRectangle(cornerRadius: 10))
                .overlay(alignment: .topTrailing) { InsigniaCaducidad(captura: captura).padding(6) }
                .overlay(alignment: .topLeading) {
                    if eligiendo || elegida {
                        Image(systemName: elegida ? "checkmark.circle.fill" : "circle")
                            .font(.system(size: 22))
                            .foregroundStyle(elegida ? Paleta.v2.acento : Color.white.opacity(0.9))
                            .background(Circle().fill(.black.opacity(elegida ? 0 : 0.3)))
                            .padding(7)
                    }
                }
                .overlay(alignment: .bottomLeading) {
                    if captura.esVideo {
                        Text("Vídeo").font(.system(size: 10.5, weight: .semibold)).foregroundStyle(.white)
                            .padding(.horizontal, 6).padding(.vertical, 2)
                            .background(.black.opacity(0.55), in: Capsule()).padding(6)
                    }
                }
                .overlay {
                    RoundedRectangle(cornerRadius: 10)
                        .strokeBorder(elegida ? Paleta.v2.acento : .clear, lineWidth: 3)
                }
            Text(fechaDe(ms: captura.cuando), format: .dateTime.hour().minute())
                .font(.system(size: 11.5))
                .foregroundStyle(Paleta.v2.gris)
        }
        .contentShape(Rectangle())
        .hoverEffect(.lift)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(captura.nombre)
        .accessibilityAddTraits(elegida ? .isSelected : [])
    }
}

/// Los dias que le quedan: «hoy», «1», «6»…, en gris si queda tiempo, en
/// naranja a uno o dos dias y en rojo el ultimo; o una marca verde si esta
/// conservada. Nada si la caducidad esta apagada.
struct InsigniaCaducidad: View {
    let captura: CapturaGaleria

    var body: some View {
        if captura.conservada {
            Image(systemName: "checkmark.seal.fill")
                .font(.system(size: 11, weight: .bold))
                .padding(5)
                .background(Paleta.v2.verde, in: Circle())
                .foregroundStyle(.white)
                .accessibilityLabel("Conservada: está en Mensajes guardados y ya no se borra")
        } else if let d = captura.diasQueQuedan {
            Text(d <= 0 ? "hoy" : "\(d)")
                .font(.system(size: 11, weight: .bold))
                .padding(.horizontal, 7).padding(.vertical, 2)
                .background(InsigniaCaducidad.color(dias: d), in: Capsule())
                .foregroundStyle(.white)
                .accessibilityLabel(InsigniaCaducidad.pista(dias: d))
        }
    }

    static func color(dias d: Int64) -> Color {
        switch d {
        case ..<1: Paleta.v2.rojo
        case 1...2: Paleta.v2.naranja
        default: Color(hex: 0x636366)
        }
    }

    static func pista(dias d: Int64) -> String {
        switch d {
        case ..<1: return "Se borra hoy. Mantenla pulsada para conservarla o darle 7 días más."
        case 1: return "Se borra mañana. Mantenla pulsada para conservarla o darle 7 días más."
        default: return "Se borra en \(d) días. Mantenla pulsada para conservarla o darle 7 días más."
        }
    }
}

/// La miniatura de una captura (ImageIO, reducida al leer, en la cache de
/// toda la app). Un video lleva un icono: no se decodifica.
struct MiniaturaCaptura: View {
    let url: URL
    let esVideo: Bool
    /// Lado mayor en pixeles: la celda mide hasta ~280 puntos a 2x.
    static let lado = 480
    @State private var imagen: CGImage?
    @State private var falta = false

    var body: some View {
        Rectangle().fill(Paleta.v2.tarjeta)
            .overlay {
                if let imagen {
                    Image(decorative: imagen, scale: 2)
                        .resizable()
                        .interpolation(.medium)
                        .scaledToFill()
                }
            }
            .overlay { marca }
            .clipped()
            .task(id: url) {
                guard !esVideo else { return }
                if let hecha = Miniaturas.compartidas.enCache(url, lado: MiniaturaCaptura.lado) {
                    imagen = hecha
                    return
                }
                let leida = await Miniaturas.compartidas.cargar(url, lado: MiniaturaCaptura.lado)
                imagen = leida
                falta = leida == nil
            }
    }

    @ViewBuilder
    private var marca: some View {
        if imagen == nil {
            if esVideo {
                Image(systemName: "film").font(.largeTitle).foregroundStyle(Paleta.v2.gris)
            } else if falta {
                Image(systemName: "photo.badge.exclamationmark").foregroundStyle(Paleta.v2.gris)
            } else {
                ProgressView().controlSize(.small)
            }
        }
    }
}
