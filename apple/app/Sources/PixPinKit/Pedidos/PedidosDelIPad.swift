// **Los pedidos que el iPad hace a su manera** (lo que en Windows es
// `pedidos.rs` con sus ventanas flotantes): capturar con lo que deja iOS,
// pinear en la capa de pines de la app, el microfono flotante, «al lienzo»,
// mover y posponer tareas, la caja de soltar, la ficha de una leccion nueva
// y el encargo al asistente con su aviso flotante. Es parte de
// `HacerPedidos` (el `switch` de `hacerYa` llama aqui).
//
// Lo que iOS no deja (capturar otras apps, grabar su pantalla, que un pin
// deje pasar los toques) lleva a lo mas parecido y lo dice el aviso.

import Combine
import Foundation
import PixPinCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// Lo que los pedidos del iPad tienen que tener vivo entre llamadas.
@MainActor
enum EstadoPedidosIPad {
    /// Quien mira la grabadora mientras sale el microfono flotante.
    static var vigiaGrabar: AnyCancellable?
    /// El delegado de la caja de soltar (UIKit no lo retiene).
    static var soltar: DelegadoSoltar?
    /// Donde acabo lo ultimo apilado en cada lienzo (`chat·referencia`):
    /// lo siguiente va debajo, en la misma columna.
    static var pilas: [String: (x: Float, y: Float)] = [:]
}

extension HacerPedidos {
    // MARK: Capturar

    /// `capturar {modo}`: iOS no deja capturar otras apps; lo mas cercano es
    /// la hoja de capturar del iPad (camara, escaner, Fotos, pegar y
    /// «Capturar PixPin»), que luego copia, pinea, anota o saca el texto.
    func capturar(_ modo: String?) {
        let c = CapturadorIPad.compartido
        switch modo ?? "zona" {
        case "gif":
            avisar("En el iPad, para grabar la pantalla usa «Grabación de pantalla» del Centro de control; luego compártela con PixPin.")
        case "retardo":
            // Lo que tiene sentido esperar en el iPad: la propia app con un
            // menu abierto.
            avisar("Capturo PixPin dentro de 3 segundos…")
            DispatchQueue.main.asyncAfter(deadline: .now() + 3) {
                MainActor.assumeIsolated { CapturadorIPad.compartido.capturar(.app) }
            }
        case "repetir":
            c.capturar(.app)
        case "scroll", "en_vivo":
            avisar(modo == "scroll"
                ? "En el iPad no hay captura con desplazamiento de otras apps: elige de dónde capturar."
                : "En el iPad no hay pin en vivo de otras apps: elige de dónde capturar.")
            c.empezar()
        default:
            c.empezar()
        }
    }

    // MARK: Pinear

    /// `pinear {ruta}` o `{proyecto, codigo}`: el fichero (o el mensaje: una
    /// nota, una mini-app) a la capa de pines.
    func pinear(_ p: Pedido) async {
        let g = GestorPinesIPad.compartido
        if let r = p.texto("ruta") {
            return pinear(url: URL(fileURLWithPath: r))
        }
        guard let codigo = p.texto("codigo") else { return avisar("«pinear» sin «ruta» ni «codigo».") }
        guard let almacen, let c = await chat(p["proyecto"]), let m = await mensaje(codigo, en: c) else { return }
        if let miniapp = m.miniapp {
            let nucleo = almacen.nucleo
            let id = m.id
            guard let doc = try? await Task.detached(operation: { try nucleo.documentoMiniapp(chat: c, mensaje: id) }).value
            else { return avisar("No se pudo leer «\(m.nombre)».") }
            return g.pinearMiniapp(chat: c, mensaje: m.id, miniapp: miniapp, documento: doc)
        }
        if m.clase == "NOTA" {
            return g.pinearNota(texto: m.texto)
        }
        guard let r = m.ruta, let url = await almacen.url(deRuta: r, en: c) else {
            return avisar("Ese mensaje no tiene un fichero que pinear.")
        }
        pinear(url: url)
    }

    private func pinear(url: URL) {
        guard FileManager.default.fileExists(atPath: url.path) else {
            return avisar("No existe «\(url.lastPathComponent)».")
        }
        if !GestorPinesIPad.compartido.pinear(url: url) {
            avisar("«\(url.lastPathComponent)» no se puede pinear: solo fotos, PDF, vídeo y audio.")
        }
    }

    /// `pinear_ultima`: la captura mas reciente de la galeria.
    func pinearUltima() async {
        guard let nucleo = almacen?.nucleo else { return avisar("No se pudo abrir la carpeta de datos de PixPin.") }
        let desfase = desfaseLocalMs()
        let ultima = await Task.detached {
            try? nucleo.galeria(desde: 0, cuantos: 1, dias: 0, desfaseMs: desfase).capturas.first
        }.value
        guard let ultima else { return avisar("Todavía no hay ninguna captura.") }
        pinear(url: URL(fileURLWithPath: ultima.ruta))
    }

    /// `pines {operacion}`: los comandos de la bandeja, en la capa del iPad.
    func pines(_ operacion: String) {
        let g = GestorPinesIPad.compartido
        switch operacion {
        case "cerrar":
            guard g.cuantos > 0 else { return avisar("No hay ningún pin abierto.") }
            g.cerrarTodosConDeshacer()
            CuadrosIPad.avisar("Pines cerrados.", botones: [
                BotonAviso("Deshacer", principal: true) { GestorPinesIPad.compartido.deshacerCerrarTodos() },
            ])
        case "ocultar_mostrar":
            guard g.cuantos > 0 else { return avisar("No hay ningún pin abierto.") }
            g.ocultarTodos(!g.pines.allSatisfy(\.oculto))
        case "devolver_ultimo":
            if g.puedeDeshacer { g.deshacerCerrarTodos() } else { avisar("No hay ningún pin cerrado que devolver.") }
        case "paso_de_clics":
            avisar("En el iPad los pines no dejan pasar los toques: baja su opacidad o minimízalos.")
        default:
            avisar("Los pines no saben «\(operacion)».")
        }
    }

    // MARK: Capturas

    /// `borrar_captura`: a la papelera de PixPin (se puede deshacer).
    func borrarCaptura(_ p: Pedido) async {
        guard let nucleo = almacen?.nucleo, let r = p.texto("ruta") else { return }
        let carpeta = URL(fileURLWithPath: nucleo.carpetaCapturas()).standardizedFileURL.path
        let url = URL(fileURLWithPath: r).standardizedFileURL
        guard url.deletingLastPathComponent().path == carpeta else {
            return avisar("Ese fichero no está en la carpeta de capturas; no se toca.")
        }
        let nombre = url.lastPathComponent
        do {
            let borradas = try await Task.detached { try nucleo.borrarCapturas(nombres: [nombre]) }.value
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            CuadrosIPad.avisar("Llevada a la papelera de PixPin", botones: [
                BotonAviso("Deshacer") {
                    Task.detached { _ = try? nucleo.devolverCapturas(borradas: borradas) }
                    NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
                },
            ])
        } catch {
            avisar("No se pudo borrar: \(textoDeError(error))")
        }
    }

    // MARK: Tareas

    /// `mover_tarea`: la tarea `indice` de la lista `codigo` al final de la
    /// lista `a_codigo` (de `a_proyecto`), como «Mover a…» de Tareas.
    func moverTarea(_ p: Pedido) async {
        guard let nucleo = almacen?.nucleo, let codigo = p.texto("codigo"), let aCodigo = p.texto("a_codigo"),
              let indice = p["indice"]?.entero, indice >= 0 else {
            return avisar("«mover_tarea» necesita la lista (codigo), la tarea (indice) y adónde (a_codigo).")
        }
        guard let c = await chat(p["proyecto"]), let aChat = await chat(p["a_proyecto"]) else { return }
        let desfase = desfaseLocalMs()
        enum Fin: Sendable { case hecho(String, String), sinTarea, cambio, fallo(String) }
        let fin = await Task.detached { () -> Fin in
            do {
                let doc = try nucleo.documentoMiniapp(chat: c, mensaje: codigo)
                guard let t = miniTareasLeer(documento: doc, desfaseMs: desfase).first(where: { $0.indice == UInt32(indice) })
                else { return .sinTarea }
                guard try nucleo.moverTarea(chat: c, lista: codigo, indice: t.indice, crudo: t.crudo,
                                            aChat: aChat, aLista: aCodigo) else { return .cambio }
                let destino = (try? nucleo.tareas(desfaseMs: desfase))?
                    .first { $0.chat == aChat && $0.mensaje == aCodigo }?.titulo ?? "Tareas"
                return .hecho(t.texto, destino)
            } catch {
                return .fallo(textoDeError(error))
            }
        }.value
        switch fin {
        case let .hecho(tarea, lista):
            avisoDeTareas(c)
            if aChat != c { avisoDeTareas(aChat) }
            avisar("«\(tarea)» movida a «\(lista)».")
        case .sinTarea: avisar("La lista no tiene la tarea número \(indice).")
        case .cambio: avisar("La lista cambió mientras se movía la tarea; no se tocó nada. Vuelve a buscarla.")
        case let .fallo(e): avisar("No se pudo mover la tarea: \(e)")
        }
    }

    /// `posponer` («+10 min» de un recordatorio): que vuelva a sonar dentro
    /// de `minutos`. `mensaje` es el del recordatorio o, con `indice`, la
    /// lista de esa tarea.
    func posponer(_ p: Pedido) async {
        guard let nucleo = almacen?.nucleo, let mensaje = p.texto("mensaje") else {
            return avisar("«posponer» necesita el mensaje del recordatorio.")
        }
        guard let c = await chat(p["proyecto"]) else { return }
        let minutos = HacerPedidos.minutosDePosponer(p["minutos"]?.entero)
        let cuando = Date().addingTimeInterval(TimeInterval(minutos * 60))
        let indice = p["indice"]?.entero.flatMap { $0 >= 0 ? UInt32($0) : nil }
        let local = milisegundosLocales(cuando)
        let utc = milisegundos(cuando)
        let hecho = await Task.detached { () -> Bool in
            if let indice {
                return (try? nucleo.ponerHoraTarea(chat: c, lista: mensaje, indice: indice, crudo: nil,
                                                   cuandoLocalMs: local)) == true
            }
            return (try? nucleo.volverARecordar(chat: c, mensaje: mensaje, cuandoUtcMs: utc)) == true
        }.value
        guard hecho else {
            return avisar(indice == nil ? "Ese mensaje ya no está en el chat." : "La lista no tiene esa tarea.")
        }
        Recordatorios.avisarCambio(chat: c, tarea: indice != nil)
        Recordatorios.compartido.releer()
        avisar("⏰ Te lo recordaré \(AtajosDeHora.legible(cuando, ahora: Date()))")
    }

    /// Los minutos de `posponer`: los de «+10 min» si no dice, de 1 minuto a
    /// una semana (`minutos.clamp(1, 7 * 24 * 60)` de Windows).
    nonisolated static func minutosDePosponer(_ m: Int?) -> Int {
        min(max(m ?? CentroAvisos.posponerMinutos, 1), 7 * 24 * 60)
    }

    // MARK: El microfono flotante

    /// `grabar`: el chat y la nota de voz ya grabando, con el microfono
    /// flotante de Windows (el aviso: «Guardar» o «Descartar»).
    func grabar(_ p: Pedido) async {
        guard let almacen, let c = await chat(p["proyecto"]) else { return }
        let g = GrabadoraVoz.compartida
        if case .grabando = g.estado { return avisar("Ya se está grabando una nota de voz.") }
        Navegacion.compartida.cerrarVisor()
        Navegacion.compartida.irAlChat(c)
        await g.empezar(en: c, almacen: almacen)
        // Sin permiso o sin microfono: lo dice el chat.
        guard g.grabando(en: c) else { return }
        let donde = nombre(de: c)
        let guardar: @MainActor () -> Void = { [weak almacen] in
            guard let almacen else { return }
            GrabadoraVoz.compartida.alGuardar = { m in
                CuadrosIPad.avisar(m == nil ? "La nota era demasiado corta: no se guardó."
                                   : "Nota de voz guardada en «\(donde)».")
            }
            Task { await GrabadoraVoz.compartida.parar(almacen: almacen) }
        }
        let tarjeta = AvisoFlotante.compartido.mostrar("Grabando en «\(donde)»…", titulo: "🎙 Nota de voz", acabado: false)
        tarjeta.cambiar("Grabando en «\(donde)»…", botones: [
            BotonAviso("Descartar", peligro: true) { GrabadoraVoz.compartida.cancelar() },
            BotonAviso("Guardar", principal: true, accion: guardar),
        ], alCerrar: guardar)
        // Si se para desde el chat (su microfono), la tarjeta se va.
        EstadoPedidosIPad.vigiaGrabar = g.$estado.sink { estado in
            guard case .grabando = estado else {
                DispatchQueue.main.async {
                    MainActor.assumeIsolated {
                        tarjeta.cerrar()
                        EstadoPedidosIPad.vigiaGrabar = nil
                    }
                }
                return
            }
        }
    }

    // MARK: La caja de soltar

    /// `soltar`: en el iPad no hay caja flotante a la que arrastrar desde
    /// el Explorador; lo mismo es elegir los ficheros (Archivos, iCloud…)
    /// y que entren en ese chat.
    func soltar(_ p: Pedido) async {
        guard let c = await chat(p["proyecto"]) else { return }
        let selector = UIDocumentPickerViewController(forOpeningContentTypes: [.item], asCopy: true)
        selector.allowsMultipleSelection = true
        let d = DelegadoSoltar { [weak self] urls in
            guard let self, let almacen = self.almacen, !urls.isEmpty else { return }
            Task {
                let hechos = await almacen.adjuntar(urls: urls, en: c)
                if !hechos.isEmpty { self.avisar("Escrito en «\(self.nombre(de: c))».") }
            }
        }
        EstadoPedidosIPad.soltar = d
        selector.delegate = d
        guard let arriba = CuadrosIPad.arriba() else { return }
        arriba.present(selector, animated: true)
    }

    // MARK: Lecciones

    /// `leccion_nueva {texto, proyecto, imagenes}`: la ficha nueva, empezada
    /// con el texto (la primera linea es lo aprendido; lo demas, que paso),
    /// para ese chat. Las fotos van a su chat y la leccion sale de ellas
    /// (`deMensaje`), asi quedan juntas.
    func leccionNueva(_ p: Pedido) async {
        guard let almacen, let c = await chat(p["proyecto"]), let fotos = ficheros(p, ["imagenes"]) else { return }
        var b = HacerPedidos.borradorLeccion(HacerPedidos.sinFichas(p.texto("texto") ?? ""))
        b.chat = c
        if !fotos.isEmpty {
            let hechos = await almacen.adjuntar(urls: fotos, en: c)
            b.deMensaje = hechos.first?.id
        }
        let modelo = ModeloLecciones(nucleo: almacen.nucleo)
        ArribaUIKit.presentar(estilo: .formSheet, tamano: CGSize(width: 620, height: 760)) { cerrar in
            NavigationStack {
                FichaLeccionVista(modelo: modelo, ficha: nil, inicial: b) { _ in cerrar() }
                    .navigationTitle("Nueva lección")
                    .navigationBarTitleDisplayMode(.inline)
                    .toolbar {
                        ToolbarItem(placement: .cancellationAction) { Button("Cancelar") { cerrar() } }
                    }
            }
            .environment(\.colorScheme, .dark)
        }
    }

    /// Lo que trae escrito la ficha: la primera linea, lo aprendido; lo
    /// demas, que paso.
    nonisolated static func borradorLeccion(_ texto: String) -> BorradorLeccion {
        var b = BorradorLeccion()
        let t = texto.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty else { return b }
        let lineas = t.split(separator: "\n", maxSplits: 1, omittingEmptySubsequences: false)
        b.titulo = lineas[0].trimmingCharacters(in: .whitespaces)
        if lineas.count > 1 { b.quePaso = lineas[1].trimmingCharacters(in: .whitespacesAndNewlines) }
        return b
    }

    // MARK: Al lienzo

    /// `al_lienzo {lienzo, proyecto, texto, imagenes}`: las fotos y el texto
    /// apilados en una columna libre del lienzo de ese nombre (del proyecto
    /// dicho o, sin el, del que se llame asi en cualquier chat); si no hay
    /// ninguno, se crea en el proyecto (o en Mensajes guardados). Si estaba
    /// abierto, se cierra, entra y se vuelve a abrir.
    func alLienzo(_ p: Pedido) async {
        guard let almacen else { return avisar("No se pudo abrir la carpeta de datos de PixPin.") }
        guard let buscado = p.texto("lienzo") else { return avisar("«al_lienzo» sin el nombre del lienzo.") }
        guard let fotos = ficheros(p, ["imagenes"]) else { return }
        if let f = fotos.first(where: { !Verbos.esImagen($0.path) }) {
            return avisar("«\(f.lastPathComponent)» no es una imagen.")
        }
        let texto = HacerPedidos.sinFichas(p.texto("texto") ?? "")
        guard !texto.isEmpty || !fotos.isEmpty else { return avisar("«al_lienzo» sin texto ni imágenes.") }
        if almacen.chats.isEmpty { await almacen.cargarChats() }
        let nucleo = almacen.nucleo
        let donde: [String]
        if let id = p.texto("proyecto") {
            guard almacen.chats.contains(where: { $0.id == id }) else { return avisar("No existe el proyecto «\(id)».") }
            donde = [id]
        } else {
            donde = almacen.chats.map(\.id)
        }
        var hallado = await Task.detached(priority: .userInitiated) {
            HacerPedidos.buscarLienzo(buscado, en: donde, nucleo: nucleo)
        }.value
        if hallado == nil {
            guard let c = await chat(p["proyecto"]) else { return }
            do {
                let m = try await Task.detached { try nucleo.crearLienzo(chat: c, nombre: buscado) }.value
                alFinal(m, en: c)
                hallado = (c, m)
            } catch {
                return avisar("No se pudo crear el lienzo: \(textoDeError(error))")
            }
        }
        guard let (c, m) = hallado, let referencia = m.referencia else {
            return avisar("No se pudo abrir el lienzo «\(buscado)».")
        }
        // Abierto: se cierra (se guarda), entra y se vuelve a abrir.
        let nav = Navegacion.compartida
        var estabaAbierto = false
        if case let .lienzo(chatAbierto, abierto, _)? = nav.visor, chatAbierto == c, abierto?.referencia == referencia {
            estabaAbierto = true
            nav.cerrarVisor()
            try? await Task.sleep(for: .milliseconds(900))
        }
        let clave = c + "·" + referencia
        let pila = EstadoPedidosIPad.pilas[clave]
        let rutas = fotos
        let r = await Task.detached(priority: .userInitiated) {
            Result { try HacerPedidos.apilar(texto: texto, fotos: rutas, en: referencia, chat: c, pila: pila, nucleo: nucleo) }
        }.value
        switch r {
        case let .success(fin):
            EstadoPedidosIPad.pilas[clave] = fin
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            NotificationCenter.default.post(name: .pixpinLienzoGuardado, object: referencia)
            let suyo = HacerPedidos.nombreDeLienzo(m) ?? buscado
            if estabaAbierto {
                nav.abrir(.lienzo(chat: c, mensaje: m, foto: nil))
            }
            CuadrosIPad.avisar("Apilado en el lienzo «\(suyo)».", botones: estabaAbierto ? [] : [
                BotonAviso("Abrir", principal: true) { Navegacion.compartida.abrir(.lienzo(chat: c, mensaje: m, foto: nil)) },
            ])
        case let .failure(e):
            avisar("No se pudo apilar en el lienzo: \(textoDeError(e))")
        }
    }

    /// Lo que mide la columna de la pila (`pila_lienzo.rs`).
    nonisolated static let anchoColumna: Float = 480
    nonisolated static let hueco: Float = 24
    nonisolated static let aireColumna: Float = 80

    /// **Apila** en el lienzo (cerrado): las fotos una debajo de otra, a lo
    /// ancho de la columna como mucho, y el texto debajo. Devuelve donde
    /// acaba la pila (para lo siguiente).
    nonisolated static func apilar(texto: String, fotos: [URL], en referencia: String, chat: String,
                                   pila: (x: Float, y: Float)?, nucleo: Nucleo) throws -> (x: Float, y: Float) {
        let editor = try nucleo.editarLienzo(chat: chat, dibujo: referencia)
        defer { editor.cerrado() }
        var (x, y) = sitioDeLaPila(caja: editor.caja(), pila: pila)
        for url in fotos {
            guard let png = ImagenPegadaLienzo.png(de: .fichero(url)),
                  let tam = ImagenPegadaLienzo.tamano(de: png) else { continue }
            let (w, h) = tamanoEnLaPila(ancho: Float(tam.width), alto: Float(tam.height))
            // `pegarImagen` reduce lo que no cabe en el 80 % de la vista: se
            // le da una donde cabe justo a ese tamano.
            _ = try editor.pegarImagen(png: png, centroX: x + w / 2, centroY: y + h / 2,
                                       anchoVista: w / 0.8 + 1, altoVista: h / 0.8 + 1)
            y += h + hueco
        }
        if !texto.isEmpty {
            try editor.herramienta(nombre: "texto")
            _ = editor.puntero(fase: .pulsar, x: x, y: y, presion: nil, modificadores: 0, zoom: 1)
            _ = editor.puntero(fase: .soltar, x: x, y: y, presion: nil, modificadores: 0, zoom: 1)
            if editor.teclear(texto: texto) {
                // Lo de abajo del texto: el renglon donde acaba el cursor.
                let abajo = editor.cajaDelCursor().map { max($0.y1, y) } ?? (y + 40)
                _ = editor.terminarTexto()
                y = abajo + hueco
            }
        }
        _ = try editor.guardar()
        return (x, y)
    }

    /// Donde empieza lo siguiente: debajo de lo ultimo apilado o, la primera
    /// vez, en una columna a la derecha de todo lo que hay.
    nonisolated static func sitioDeLaPila(caja: CajaLienzo?, pila: (x: Float, y: Float)?) -> (Float, Float) {
        if let pila { return pila }
        guard let c = caja, [c.x0, c.y0, c.x1, c.y1].allSatisfy(\.isFinite) else { return (0, 0) }
        return (c.x1 + aireColumna, c.y0)
    }

    /// El tamano con que entra una foto: el suyo, o reducida a lo ancho de
    /// la columna.
    nonisolated static func tamanoEnLaPila(ancho: Float, alto: Float) -> (Float, Float) {
        let (w, h) = (max(ancho, 1), max(alto, 1))
        let f = min(anchoColumna / w, 1)
        return (w * f, h * f)
    }

    /// El nombre de un lienzo para leerlo; `nil` si solo tiene el de su
    /// fichero (`<id>.excalidraw`), que no dice nada (`nombre_de_lienzo`).
    nonisolated static func nombreDeLienzo(_ m: MensajeChat) -> String? {
        let n = m.nombre.trimmingCharacters(in: .whitespacesAndNewlines)
        return n.isEmpty || n.lowercased().hasSuffix(".excalidraw") ? nil : n
    }

    /// Un nombre para compararlo: sin tildes ni mayusculas y `_` por
    /// espacio (`nombre_normal` de Windows).
    nonisolated static func nombreNormal(_ s: String) -> String {
        s.replacingOccurrences(of: "_", with: " ")
            .folding(options: [.caseInsensitive, .diacriticInsensitive], locale: Locale(identifier: "es_ES"))
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// **El lienzo de ese nombre** entre unos mensajes: el ultimo que se
    /// llama justo asi y, si no, el ultimo que lo contiene. Devuelve si es
    /// exacto.
    nonisolated static func lienzoPorNombre(_ buscado: String, en mensajes: [MensajeChat]) -> (MensajeChat, Bool)? {
        let b = nombreNormal(buscado)
        guard !b.isEmpty else { return nil }
        let lienzos = mensajes.reversed().filter { $0.clase == "DIBUJO" && $0.referencia != nil }
        if let m = lienzos.first(where: { nombreDeLienzo($0).map(nombreNormal) == b }) { return (m, true) }
        if let m = lienzos.first(where: { nombreDeLienzo($0).map(nombreNormal)?.contains(b) == true }) { return (m, false) }
        return nil
    }

    /// En unos chats (fuera del principal): mejor el que se llama justo asi
    /// y, entre iguales, el mas nuevo (`pila_lienzo::buscar`).
    nonisolated static func buscarLienzo(_ buscado: String, en chats: [String], nucleo: Nucleo) -> (String, MensajeChat)? {
        var mejor: (chat: String, m: MensajeChat, exacto: Bool)?
        for c in chats {
            var todos: [MensajeChat] = []
            var desde: UInt64 = 0
            while let pagina = try? nucleo.mensajes(chat: c, desde: desde, cuantos: 500), !pagina.mensajes.isEmpty {
                todos += pagina.mensajes
                desde += UInt64(pagina.mensajes.count)
                if desde >= pagina.total { break }
            }
            guard let (m, exacto) = lienzoPorNombre(buscado, en: todos) else { continue }
            if let b = mejor, b.exacto && !exacto || (b.exacto == exacto && b.m.cuando >= m.cuando) { continue }
            mejor = (c, m, exacto)
        }
        return mejor.map { ($0.chat, $0.m) }
    }

    // MARK: El asistente, sin abrir el chat

    /// `asistente {texto, proyecto, imagenes}`: el encargo va al chat
    /// «Asistente» (alli queda la conversacion, como en Windows) y el
    /// aviso flotante dice «💭 …» mientras contesta; al acabar, la
    /// respuesta con «Abrir» (el chat en esa respuesta). Si pide permiso
    /// para algo que borra o envia, la tarjeta lo pregunta con «Sí», «No» y
    /// «Abrir» (el aspa es un «No»). Sin texto ni imagenes, abre su chat.
    func preguntarEnSuChat(_ p: Pedido) async {
        guard p.texto("automatizacion") == nil, let almacen else { return await preguntar(p) }
        let texto = HacerPedidos.sinFichas(p.texto("texto") ?? "")
        guard let fotos = ficheros(p, ["imagenes"]) else { return }
        await almacen.asegurarAsistente()
        if almacen.chats.isEmpty { await almacen.cargarChats() }
        let nucleo = almacen.nucleo
        var suyo = almacen.chats.first(where: \.esAsistente)?.id
        if suyo == nil { suyo = try? await Task.detached { try nucleo.chatAsistente().id }.value }
        guard let suyo else { return avisar("No se pudo abrir el chat «Asistente».") }
        let nav = Navegacion.compartida
        if texto.isEmpty && fotos.isEmpty {
            ir(.proyectos)
            return nav.irAlChat(suyo)
        }
        let ia = AsistenteEnElChat.compartido
        let corto = String(texto.prefix(200))
        let tarjeta = AvisoFlotante.compartido.mostrar("💭 " + (corto.isEmpty ? "Mirando la foto…" : corto),
                                                       titulo: "Asistente", acabado: false, dura: AvisoFlotante.dura)
        // Uno tras otro (la cola de encargos de Windows): si ya contesta
        // en su chat, se espera a que acabe.
        let limite = Date().addingTimeInterval(15 * 60)
        while ia.ocupado(suyo), Date() < limite {
            try? await Task.sleep(for: .milliseconds(500))
        }
        // Las fotos entran en su chat sin lanzar un turno cada una; el turno
        // lo lanza lo ultimo (el texto, o la ultima foto si no hay texto).
        var ultimo: MensajeChat?
        if !fotos.isEmpty {
            let rutas = fotos.map(\.path)
            let hechas = await Task.detached(priority: .userInitiated) {
                rutas.compactMap { try? nucleo.adjuntar(chat: suyo, rutaOrigen: $0, nombre: nil) }
            }.value
            almacen.anadir(hechas, en: suyo)
            ultimo = hechas.last
        }
        var pedido = texto
        if let pid = p.texto("proyecto"), let n = almacen.chats.first(where: { $0.id == pid })?.nombre {
            pedido += " @" + n
        }
        if !pedido.isEmpty, let m = await almacen.escribir(nota: pedido, en: suyo) { ultimo = m }
        guard let ultimo else {
            return tarjeta.poner("No se pudo escribir el encargo en el chat «Asistente».")
        }
        let antes = Set(ia.preguntas.keys)
        // Lo que pregunte, en la tarjeta (y en la burbuja del chat).
        let vigia = ia.$preguntas.sink { preguntas in
            guard let par = preguntas.first(where: { !antes.contains($0.key) }) else { return }
            let nota = par.key
            let pregunta = par.value
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    tarjeta.cambiar("Necesito tu permiso: " + pregunta.texto, botones: [
                        BotonAviso("Abrir", cierra: false) { nav.cerrarVisor(); nav.irAlChat(suyo, mensaje: nota) },
                        BotonAviso("No", cierra: false) {
                            AsistenteEnElChat.compartido.responder(nota: nota, si: false)
                            tarjeta.cambiar("💭 " + corto)
                        },
                        BotonAviso("Sí", principal: true, cierra: false) {
                            AsistenteEnElChat.compartido.responder(nota: nota, si: true)
                            tarjeta.cambiar("💭 " + corto)
                        },
                    ], alCerrar: { AsistenteEnElChat.compartido.responder(nota: nota, si: false) })
                }
            }
        }
        let turnos = ia.alEnviar([ultimo], chat: suyo, almacen: almacen)
        for t in turnos { await t.value }
        vigia.cancel()
        let respuesta = (almacen.mensajes[suyo] ?? []).last { $0.esDelAsistente && $0.cuando >= ultimo.cuando }
        guard let respuesta, !turnos.isEmpty else {
            // Sin motor o sin clave: lo que dijo la barra del chat.
            let porQue = ia.aviso.flatMap { $0.chat == suyo ? $0.texto : nil } ?? "El asistente no contestó."
            return tarjeta.poner(porQue, botones: [
                BotonAviso("Abrir", principal: true) { nav.cerrarVisor(); nav.irAlChat(suyo) },
            ])
        }
        let dicho = respuesta.texto.trimmingCharacters(in: .whitespacesAndNewlines)
        tarjeta.poner(dicho.isEmpty ? "Hecho." : dicho, botones: [
            BotonAviso("Abrir", principal: true) { nav.cerrarVisor(); nav.irAlChat(suyo, mensaje: respuesta.id) },
        ])
    }
}

/// El delegado de la caja de soltar.
@MainActor
final class DelegadoSoltar: NSObject, UIDocumentPickerDelegate {
    let alElegir: @MainActor ([URL]) -> Void

    init(alElegir: @escaping @MainActor ([URL]) -> Void) {
        self.alElegir = alElegir
    }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        alElegir(urls)
        EstadoPedidosIPad.soltar = nil
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        EstadoPedidosIPad.soltar = nil
    }
}
