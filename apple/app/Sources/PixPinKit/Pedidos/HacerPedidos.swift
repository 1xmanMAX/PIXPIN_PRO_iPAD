// **Quien hace los pedidos** en el iPad: lo que en Windows es el bucle que
// atiende `WM_COPYDATA` (`pedidos.rs`) y en el Mac `HacerPedidos`. Llegan de
// los enlaces `pixpinmax://` (con su lista blanca y el cuadro de
// confirmar), de Atajos y Siri (App Intents) y del asistente, y cada accion
// va a lo mismo que hace el boton de la app, abriendo lo que toque con
// `Navegacion.compartida`. Lo que el iPad no puede hacer (capturar otras
// apps, los pines encima de todo) se dice en el aviso, no se calla.
//
// Se pone en marcha solo (`Arranque.apuntar`, abajo): registra el almacen y
// el sincronizador y el `AbrirDesdeFuera.alEnlace`.

import AVFoundation
import Foundation
import ImageIO
import PixPinCore
import UIKit
import UniformTypeIdentifiers

@MainActor
public final class HacerPedidos {
    public static let compartidos = HacerPedidos()

    private(set) weak var almacen: Almacen?
    private(set) weak var sincro: Sincronizador?
    /// El asistente de los pedidos (sin ventana; la respuesta va al aviso).
    private var asistente: ModeloAsistente?
    /// Lo que llego antes de que la app registrara lo suyo (un enlace que
    /// abre PixPin): se hace al registrar.
    private var pendientes: [LlamadaOrden] = []
    private var registrado = false

    /// Preguntar antes de hacer (el cuadro de Windows). Las pruebas lo
    /// cambian.
    var confirmar: @MainActor (String, @escaping @MainActor (Bool) -> Void) -> Void = { cuerpo, responder in
        CuadrosIPad.confirmar(cuerpo + "\n\n¿Hacerlo?", titulo: "PixPin Max", responder: responder)
    }
    /// Decir algo (el globo de Windows). Las pruebas lo cambian.
    var avisar: @MainActor (String) -> Void = { CuadrosIPad.avisar($0) }

    public init() {}

    /// La app dice con que trabaja (una vez, al arrancar).
    public func registrar(almacen: Almacen?, sincro: Sincronizador?) {
        self.almacen = almacen
        self.sincro = sincro
        registrado = true
        let ya = pendientes
        pendientes = []
        for l in ya { hacer(l) }
    }

    /// Se apunta al arrancar: los enlaces `pixpinmax://` vienen aqui.
    static func ponerEnMarcha(_ entorno: EntornoApp) {
        compartidos.registrar(almacen: entorno.almacen, sincro: entorno.sincro)
        AbrirDesdeFuera.alEnlace = { url, _ in
            _ = HacerPedidos.compartidos.abrir(enlace: url)
        }
    }

    // MARK: Las puertas

    /// **Un enlace** `pixpinmax://…`. `false` si no es nuestro.
    @discardableResult
    public func abrir(enlace url: URL) -> Bool {
        let texto = url.absoluteString
        guard Enlaces.esEnlace(texto) else { return false }
        do {
            hacer(try Enlaces.leer(texto))
        } catch {
            avisar(error.localizedDescription)
        }
        return true
    }

    /// Unos pedidos de la propia app (Atajos, el asistente): sin preguntar.
    public func hacer(_ pedidos: [Pedido]) {
        hacer(LlamadaOrden(orden: .pedidos(pedidos), origen: .consola))
    }

    public func hacer(_ pedido: Pedido) { hacer([pedido]) }

    /// **Hace una orden**: cambia los nombres de proyecto por su id, mira la
    /// seguridad si viene de un enlace y la hace.
    public func hacer(_ l: LlamadaOrden) {
        guard registrado else {
            pendientes.append(l)
            return
        }
        switch l.orden {
        case .ayuda:
            avisar("pixpinmax://<verbo>?campo=valor\n" + Verbos.tabla.filter(\.porEnlace).map(\.nombres[0]).joined(separator: ", "))
        case let .ocr(rutas, _):
            hacerUno(Pedido("ocr", ["ruta": .texto(rutas.first ?? ""), "rutas": .textos(Array(rutas.dropFirst()))]))
        case let .preguntar(rutas, proyecto):
            let titulo = rutas.count == 1
                ? "Preguntar al asistente sobre \((rutas[0] as NSString).lastPathComponent)"
                : "Preguntar al asistente sobre \(rutas.count) archivos"
            CuadrosIPad.pedirTexto(titulo, pista: "¿Qué hago con esto?", boton: "Preguntar") { [weak self] pregunta in
                guard let self, let pregunta else { return }
                self.hacerTodos([Verbos.pedidoAlAsistente(pregunta, ficheros: rutas, proyecto: proyecto)], origen: l.origen)
            }
        case let .pedidos(v):
            hacerTodos(v, origen: l.origen)
        }
    }

    private func hacerTodos(_ pedidos: [Pedido], origen: OrigenOrden) {
        let pedidos = HacerPedidos.ponerIds(pedidos, proyectos: proyectos())
        if case let .enlace(firmado) = origen {
            // Lo peor de todos los pedidos del enlace manda.
            let peor = pedidos.map { firmado ? Enlaces.seguridadFirmado($0) : Enlaces.seguridad($0) }.max() ?? .directa
            switch peor {
            case .directa: break
            case let .prohibida(motivo):
                avisar(Enlaces.textoDelMotivo(motivo))
                return
            case let .confirmar(motivo):
                let cuerpo = "Un enlace le pide a PixPin Max:\n\n"
                    + pedidos.map(Enlaces.describir).joined(separator: "\n\n")
                    + "\n\n" + Enlaces.textoDelMotivo(motivo)
                confirmar(cuerpo) { [weak self] si in
                    guard si, let self else { return }
                    for p in pedidos { self.hacerUno(p) }
                }
                return
            }
        }
        for p in pedidos { hacerUno(p) }
    }

    // MARK: Los proyectos por su nombre

    /// Los proyectos (id, nombre); «Mensajes guardados» va con `guardados`.
    private func proyectos() -> [(id: String, nombre: String, guardados: Bool)] {
        (almacen?.chats ?? []).map { ($0.id, $0.nombre, $0.esGuardados) }
    }

    /// `proyecto: "Obra"` → el id de «Obra» (`poner_ids` de Windows). Un id
    /// que ya existe se deja; un nombre se busca sin mirar mayusculas;
    /// «Mensajes guardados» (o «guardados») es nulo. Lo que no se encuentra
    /// se deja: al hacerlo se dice que no existe.
    static func ponerIds(_ pedidos: [Pedido], proyectos: [(id: String, nombre: String, guardados: Bool)]) -> [Pedido] {
        func uno(_ v: ValorPedido?) -> ValorPedido? {
            guard let t = v?.texto?.trimmingCharacters(in: .whitespacesAndNewlines) else { return v }
            let bajo = t.lowercased()
            if bajo == "guardados" || bajo == "mensajes guardados" { return .nulo }
            if proyectos.contains(where: { $0.id == t }) { return .texto(t) }
            if let p = proyectos.first(where: { $0.nombre.trimmingCharacters(in: .whitespaces).lowercased() == bajo }) {
                return p.guardados ? .nulo : .texto(p.id)
            }
            return v
        }
        return pedidos.map { p in
            var p = p
            if p["proyecto"] != nil { p["proyecto"] = uno(p["proyecto"]) }
            if p["a_proyecto"] != nil { p["a_proyecto"] = uno(p["a_proyecto"]) }
            if var que = p["que"]?.objeto, que["proyecto"] != nil {
                que["proyecto"] = uno(que["proyecto"])
                p["que"] = .objeto(que)
            }
            return p
        }
    }

    /// El id del chat de un pedido: el `proyecto` o, nulo, «Mensajes
    /// guardados». `nil` (y se dice) si no hay datos o no existe.
    func chat(_ v: ValorPedido?) async -> String? {
        guard let almacen else {
            avisar("No se pudo abrir la carpeta de datos de PixPin.")
            return nil
        }
        if almacen.chats.isEmpty { await almacen.cargarChats() }
        if let id = v?.texto {
            guard almacen.chats.contains(where: { $0.id == id }) else {
                avisar("No existe el proyecto «\(id)».")
                return nil
            }
            return id
        }
        if let g = almacen.chats.first(where: \.esGuardados) { return g.id }
        let nucleo = almacen.nucleo
        return try? await Task.detached { try nucleo.chatGuardados().id }.value
    }

    func nombre(de chat: String) -> String {
        almacen?.chats.first { $0.id == chat }?.nombre ?? "Mensajes guardados"
    }

    // MARK: Cada accion

    /// **Hace un pedido** ya aceptado.
    func hacerUno(_ p: Pedido) {
        Task { await hacerYa(p) }
    }

    func hacerYa(_ p: Pedido) async {
        // Dentro de un paso «pedido al asistente» de una automatizacion, lo
        // que la haria saltar otra vez no se hace (E6, sin bucles).
        if let motivo = MotorAutomatizaciones.compartido.bloqueado(p.accion) {
            return avisar("No se hizo: \(motivo).")
        }
        let nav = Navegacion.compartida
        switch p.accion {
        case "abrir": await abrir(p)
        case "ventana_principal":
            if p.texto("proyecto") != nil {
                guard let c = await chat(p["proyecto"]) else { return }
                nav.cerrarVisor()
                nav.irAlChat(c)
            } else {
                ir(.proyectos)
            }
        case "ventana": ventana(cual: p.texto("cual") ?? "")
        case "capturar": capturar(p.texto("modo"))
        case "al_lienzo": await alLienzo(p)
        case "chat": await escribir(p)
        case "nota_nueva": await notaNueva(p)
        case "lienzo_nuevo": await lienzoNuevo(p)
        case "lista_nueva": await listaNueva(p)
        case "tabla_nueva": await tablaNueva(p)
        case "anadir_tarea": await anadirTarea(p)
        case "marcar_tarea", "quitar_tarea", "corregir_tarea": await cambiarTarea(p)
        case "borrar_lista": await borrarLista(p)
        case "mover_tarea": await moverTarea(p)
        case "posponer": await posponer(p)
        case "grabar": await grabar(p)
        case "pinear": await pinear(p)
        case "pinear_ultima": await pinearUltima()
        case "pines": pines(p.texto("operacion") ?? "")
        // Los iconos de extension son del Explorador de Windows: aqui no hay
        // nada que pintar (y no se dice nada: lo pide el lanzador solo).
        case "iconos": break
        case "soltar": await soltar(p)
        case "conservar_captura": await conservar(p)
        case "borrar_captura": await borrarCaptura(p)
        case "copiar_imagen": copiarImagen(p)
        case "reproducir":
            guard let r = p.texto("ruta"), FileManager.default.fileExists(atPath: r) else {
                return avisar("No existe el audio «\(p.texto("ruta") ?? "")».")
            }
            let url = URL(fileURLWithPath: r)
            nav.abrir(.medio(url: url, titulo: p.texto("titulo") ?? url.lastPathComponent))
        case "lecciones": ir(.lecciones)
        case "leccion_nueva": await leccionNueva(p)
        case "sincronizar": await sincronizar(p)
        case "enviar_al_movil": await alMovil(p)
        case "asistente": await preguntarEnSuChat(p)
        case "automatizacion": await automatizacion(p)
        case "timeline": await timeline(p)
        case "leer_en_voz": leer(p)
        case "ocr": await ocr(p)
        case "herramienta": await herramienta(p)
        default:
            avisar("«\(p.accion)» aún no lo hace PixPin en el iPad.")
        }
    }

    /// Pasa a una seccion de la barra lateral (cerrando el visor que tape).
    func ir(_ s: Seccion) {
        Navegacion.compartida.cerrarVisor()
        Navegacion.compartida.seccion = s
    }

    /// `ventana {cual}`: las de la bandeja de Windows, aqui secciones.
    private func ventana(cual: String) {
        switch cual {
        case "tareas": ir(.tareas)
        case "galeria": ir(.galeria)
        case "lecciones": ir(.lecciones)
        case "sincronizar": ir(.aparatos)
        case "timeline": ir(.timeline)
        case "buscar": ir(.buscar)
        case "asistente": ir(.asistente)
        case "ajustes": ir(.ajustes)
        default: avisar("No hay ventana «\(cual)».")
        }
    }

    // MARK: Abrir

    private func abrir(_ p: Pedido) async {
        guard let que = p["que"]?.objeto, let tipo = que["tipo"]?.texto else {
            return avisar("«abrir» sin «que».")
        }
        let nav = Navegacion.compartida
        switch tipo {
        case "proyecto":
            guard let c = await chat(que["proyecto"]) else { return }
            nav.cerrarVisor()
            nav.irAlChat(c)
        case "mensaje":
            guard let c = await chat(que["proyecto"]), let codigo = que["codigo"]?.texto else { return }
            nav.cerrarVisor()
            nav.irAlChat(c, mensaje: codigo)
        case "hoja":
            guard let c = await chat(que["proyecto"]), let r = que["referencia"]?.texto else { return }
            guard let m = await mensaje(referencia: r, en: c) else {
                return avisar("No existe esa hoja en «\(nombre(de: c))».")
            }
            nav.abrir(m.clase == "DIBUJO" ? .lienzo(chat: c, mensaje: m, foto: nil) : .hoja(chat: c, mensaje: m))
        case "nota":
            guard let c = await chat(que["proyecto"]), let codigo = que["codigo"]?.texto,
                  let m = await mensaje(codigo, en: c) else { return }
            nav.abrir(.nota(chat: c, mensaje: m))
        case "fichero":
            guard let r = que["ruta"]?.texto else { return }
            abrir(fichero: URL(fileURLWithPath: r), en: IrEnElLector.peticion(de: que))
        default:
            avisar("«abrir» no conoce «\(tipo)».")
        }
    }

    /// Como «Abrir con PixPin»: cada fichero con su visor. Con pagina,
    /// texto o zona, el visor va alli (`IrEnElLector`); si ya estaba
    /// abierto, no se abre otro: lo lleva el abierto.
    @discardableResult
    func abrir(fichero url: URL, en peticion: IrEnElLector.Peticion = IrEnElLector.Peticion()) -> Bool {
        guard FileManager.default.fileExists(atPath: url.path) else {
            avisar("No existe «\(url.path)».")
            return false
        }
        IrEnElLector.pedir(peticion, para: url)
        if peticion.algo, HacerPedidos.estaAbierto(url) { return true }
        Navegacion.compartida.abrirFichero(url)
        return true
    }

    /// Si el visor de delante es el de `url`.
    static func estaAbierto(_ url: URL) -> Bool {
        let ruta = url.standardizedFileURL.path
        switch Navegacion.compartida.visor {
        case let .documento(u, _, _, _)?, let .plano(u, _)?, let .modelo3d(u, _)?:
            return u.standardizedFileURL.path == ruta
        default:
            return false
        }
    }

    /// Un mensaje por su id: la pagina donde esta, cargada.
    func mensaje(_ id: String, en chat: String) async -> MensajeChat? {
        guard let almacen else { return nil }
        if let m = almacen.mensajes[chat]?.first(where: { $0.id == id }) { return m }
        let nucleo = almacen.nucleo
        let posicion = await Task.detached { try? nucleo.posicionDeMensaje(chat: chat, mensaje: id) }.value
        guard let posicion = posicion.flatMap({ $0 }) else {
            avisar("No existe ese mensaje en «\(nombre(de: chat))».")
            return nil
        }
        await almacen.abrir(chat: chat, en: posicion)
        return almacen.mensajes[chat]?.first { $0.id == id }
    }

    /// El mensaje de un lienzo u hoja por su referencia (en lo cargado o en
    /// la ultima pagina).
    func mensaje(referencia r: String, en chat: String) async -> MensajeChat? {
        guard let almacen else { return nil }
        if let m = almacen.mensajes[chat]?.last(where: { $0.referencia == r }) { return m }
        await almacen.abrir(chat: chat)
        return almacen.mensajes[chat]?.last { $0.referencia == r }
    }

    // MARK: Escribir

    /// Las rutas de un campo (`imagenes`, `archivos`), las que existen; si
    /// falta alguna, nada (y se dice), como Windows.
    func ficheros(_ p: Pedido, _ campos: [String]) -> [URL]? {
        let rutas = campos.flatMap { p[$0]?.lista?.compactMap(\.texto) ?? [] }
        var urls: [URL] = []
        for r in rutas {
            var dir: ObjCBool = false
            guard FileManager.default.fileExists(atPath: r, isDirectory: &dir), !dir.boolValue else {
                avisar("No existe «\(r)».")
                return nil
            }
            urls.append(URL(fileURLWithPath: r))
        }
        return urls
    }

    /// `chat`: el texto (sin sus fichas `[img 01]`) y despues las fotos y
    /// los ficheros, como al soltarlos en el chat.
    private func escribir(_ p: Pedido) async {
        guard let almacen, let c = await chat(p["proyecto"]),
              let urls = ficheros(p, ["imagenes", "archivos"]) else { return }
        let texto = HacerPedidos.sinFichas(p.texto("texto") ?? "")
        if !texto.isEmpty, let m = await almacen.escribir(nota: texto, en: c) {
            GanchosAlmacen.alEnviar?([m], c, almacen)
        }
        if !urls.isEmpty { _ = await almacen.adjuntar(urls: urls, en: c) }
        avisar("Escrito en «\(nombre(de: c))».")
    }

    /// Quita las fichas `[img 01]` y `[archivo 01]` del texto.
    static func sinFichas(_ t: String) -> String {
        let sin = t.replacingOccurrences(of: #"\[(img|archivo) \d+\]"#, with: "", options: .regularExpression)
        return sin.replacingOccurrences(of: #"[ \t]{2,}"#, with: " ", options: .regularExpression)
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func notaNueva(_ p: Pedido) async {
        guard let almacen, let c = await chat(p["proyecto"]), let urls = ficheros(p, ["imagenes"]) else { return }
        let texto = HacerPedidos.sinFichas(p.texto("texto") ?? "")
        guard let m = await almacen.escribir(nota: texto.isEmpty ? "# Nota nueva" : texto, en: c) else {
            return avisar("No se pudo escribir la nota.")
        }
        // Las imagenes van al chat, detras de la nota.
        if !urls.isEmpty { _ = await almacen.adjuntar(urls: urls, en: c) }
        Navegacion.compartida.abrir(.nota(chat: c, mensaje: m))
    }

    private func lienzoNuevo(_ p: Pedido) async {
        guard let almacen, let c = await chat(p["proyecto"]) else { return }
        let nucleo = almacen.nucleo
        let nombre = p.texto("nombre")
        do {
            let m = try await Task.detached { try nucleo.crearLienzo(chat: c, nombre: nombre) }.value
            alFinal(m, en: c)
            Navegacion.compartida.abrir(.lienzo(chat: c, mensaje: m, foto: nil))
        } catch {
            avisar("No se pudo crear el lienzo: \(textoDeError(error))")
        }
    }

    /// Lo recien hecho, al final de lo cargado (o la ultima pagina si se
    /// veia una del medio).
    func alFinal(_ m: MensajeChat, en c: String) {
        guard let almacen else { return }
        if almacen.mensajes[c] != nil, !almacen.quedanPosteriores(chat: c) {
            almacen.anadir([m], en: c)
        }
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
    }

    /// La moneda del aparato para las listas y hojas nuevas.
    private static var moneda: String { Locale.current.currency?.identifier ?? "EUR" }

    /// Una lista de tareas nueva (como «Lista de tareas» del clip).
    private func listaNueva(_ p: Pedido) async {
        guard let c = await chat(p["proyecto"]) else { return }
        _ = await crearMiniapp("tareas", p.texto("titulo") ?? "Tareas", en: c)
    }

    /// Una hoja de calculo nueva, abierta.
    private func tablaNueva(_ p: Pedido) async {
        guard let c = await chat(p["proyecto"]),
              let m = await crearMiniapp("tabla", p.texto("titulo") ?? p.texto("nombre") ?? "Tabla", en: c) else { return }
        Navegacion.compartida.abrir(.hoja(chat: c, mensaje: m))
    }

    private func crearMiniapp(_ miniapp: String, _ titulo: String, en c: String) async -> MensajeChat? {
        guard let almacen else { return nil }
        let nucleo = almacen.nucleo
        let moneda = HacerPedidos.moneda
        do {
            let m = try await Task.detached {
                try nucleo.crearMiniapp(chat: c, miniapp: miniapp, titulo: titulo, moneda: moneda)
            }.value
            alFinal(m, en: c)
            return m
        } catch {
            avisar("No se pudo crear: \(textoDeError(error))")
            return nil
        }
    }

    /// `anadir_tarea`: sin proyecto ni lista, al Inbox; con lista, a esa;
    /// con proyecto, a su ultima lista (o a una nueva «Tareas»).
    private func anadirTarea(_ p: Pedido) async {
        guard let almacen else { return avisar("No se pudo abrir la carpeta de datos de PixPin.") }
        let texto = HacerPedidos.sinFichas(p.texto("texto") ?? "")
        guard !texto.isEmpty else { return }
        let nucleo = almacen.nucleo
        let desfase = desfaseLocalMs()
        let codigo = p.texto("codigo")
        let conImagenes = p["imagenes"]?.lista?.isEmpty == false
        if codigo == nil, p["proyecto"] == nil {
            // Al Inbox, con sus imagenes (las fichas `[img NN]` las pone el
            // nucleo en su sitio).
            guard let fotos = ficheros(p, ["imagenes"]) else { return }
            let rutas = fotos.map(\.path)
            let crudo = p.texto("texto") ?? texto
            do {
                let lista = try await Task.detached {
                    try rutas.isEmpty ? nucleo.apuntarTarea(texto: texto, desfaseMs: desfase)
                        : nucleo.apuntarTareaConImagenes(texto: crudo, imagenes: rutas, desfaseMs: desfase)
                }.value
                avisoDeTareas(lista.chat)
                avisar("Apuntado en el Inbox: «\(texto)».")
            } catch {
                avisar("No se pudo apuntar la tarea: \(textoDeError(error))")
            }
            return
        }
        if conImagenes {
            avisar("En una lista de un proyecto la tarea va sin sus imágenes (en el Inbox sí van).")
        }
        guard let c = await chat(p["proyecto"]) else { return }
        var lista = codigo
        if lista == nil {
            let todas = (try? await Task.detached { try nucleo.tareas(desfaseMs: desfase) }.value) ?? []
            lista = todas.filter { $0.chat == c }.max { $0.cuando < $1.cuando }?.mensaje
        }
        if lista == nil { lista = await crearMiniapp("tareas", "Tareas", en: c)?.id }
        guard let lista else { return }
        let hecho = await Task.detached { () -> Bool in
            guard let doc = try? nucleo.documentoMiniapp(chat: c, mensaje: lista) else { return false }
            let nuevo = miniTareasAnadir(documento: doc, texto: texto, desfaseMs: desfase)
            return (try? nucleo.guardarMiniapp(chat: c, mensaje: lista, anterior: doc, documento: nuevo)) ?? false
        }.value
        if hecho {
            avisoDeTareas(c)
            avisar("Apuntado en «\(nombre(de: c))»: «\(texto)».")
        } else {
            avisar("No se pudo apuntar: esa lista no está o cambió entre medias.")
        }
    }

    /// `marcar_tarea`, `quitar_tarea` y `corregir_tarea`: la tarea numero
    /// `indice` (desde 0, en el orden del documento) de la lista `codigo`.
    /// Si la lista cambio entre medias, nada.
    private func cambiarTarea(_ p: Pedido) async {
        guard let nucleo = almacen?.nucleo, let codigo = p.texto("codigo"),
              let indice = p["indice"]?.entero, indice >= 0, let c = await chat(p["proyecto"]) else {
            return avisar("«\(p.accion)» necesita la lista (codigo) y la tarea (indice).")
        }
        let accion = p.accion
        let hecha = p["hecha"]?.booleano ?? true
        let texto = p.texto("texto")
        if accion == "corregir_tarea", texto == nil { return avisar("Una tarea no se queda sin texto: eso es quitarla.") }
        let desfase = desfaseLocalMs()
        let hecho = await Task.detached { () -> Bool in
            guard let doc = try? nucleo.documentoMiniapp(chat: c, mensaje: codigo) else { return false }
            let tareas = miniTareasLeer(documento: doc, desfaseMs: desfase)
            guard let t = tareas.first(where: { $0.indice == UInt32(indice) }) else { return false }
            let nuevo: String
            switch accion {
            case "marcar_tarea":
                if t.hecha == hecha { return true }
                nuevo = miniTareasAlternar(documento: doc, indice: t.indice)
            case "quitar_tarea": nuevo = miniTareasQuitar(documento: doc, indice: t.indice)
            default: nuevo = miniTareasRenombrar(documento: doc, indice: t.indice, texto: texto ?? t.texto)
            }
            return (try? nucleo.guardarMiniapp(chat: c, mensaje: codigo, anterior: doc, documento: nuevo)) ?? false
        }.value
        if hecho { avisoDeTareas(c) } else { avisar("No hay esa tarea en esa lista (o cambió entre medias).") }
    }

    /// `borrar_lista`: la lista entera (quien lo pide ya pregunto).
    private func borrarLista(_ p: Pedido) async {
        guard let nucleo = almacen?.nucleo, let codigo = p.texto("codigo"), let c = await chat(p["proyecto"]) else { return }
        let desfase = desfaseLocalMs()
        let borrada = await Task.detached { () -> Bool in
            let listas = (try? nucleo.tareas(desfaseMs: desfase)) ?? []
            guard listas.contains(where: { $0.chat == c && $0.mensaje == codigo }) else { return false }
            return ((try? nucleo.borrarMensajes(chat: c, ids: [codigo])) ?? 0) > 0
        }.value
        if borrada {
            avisoDeTareas(c)
            await almacen?.abrir(chat: c)
        } else {
            avisar("Eso no es una lista de tareas.")
        }
    }

    func avisoDeTareas(_ chat: String) {
        NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
        NotificationCenter.default.post(name: .pixpinTareasCambiadas, object: chat)
    }

    // MARK: Capturas

    /// Solo una captura de la carpeta de capturas (como Windows).
    private func conservar(_ p: Pedido) async {
        guard let nucleo = almacen?.nucleo, let r = p.texto("ruta") else { return }
        let carpeta = URL(fileURLWithPath: nucleo.carpetaCapturas()).standardizedFileURL.path
        let url = URL(fileURLWithPath: r).standardizedFileURL
        guard url.deletingLastPathComponent().path == carpeta else {
            return avisar("Solo se conservan capturas de la carpeta de capturas.")
        }
        let nombre = url.lastPathComponent
        do {
            _ = try await Task.detached { try nucleo.conservarCaptura(nombre: nombre) }.value
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            avisar("Conservada en «Mensajes guardados».")
        } catch {
            avisar("No se pudo conservar: \(textoDeError(error))")
        }
    }

    private func copiarImagen(_ p: Pedido) {
        guard let r = p.texto("ruta"), Verbos.esImagen(r), !r.lowercased().hasSuffix(".gif"),
              let imagen = UIImage(contentsOfFile: r) else {
            return avisar("Eso no es una imagen que copiar.")
        }
        UIPasteboard.general.image = imagen
        avisar("Imagen copiada.")
    }

    // MARK: Otros aparatos

    private func sincronizar(_ p: Pedido) async {
        guard let sincro else { return avisar("La sincronización no está lista.") }
        let aparato = p.texto("aparato") ?? "todos"
        if aparato != "todos", !sincro.filas.contains(where: { $0.id == aparato || $0.nombre == aparato }) {
            return avisar("No hay ningún aparato «\(aparato)» en tu grupo.")
        }
        let id = aparato == "todos" ? nil : sincro.filas.first { $0.id == aparato || $0.nombre == aparato }?.id
        avisar("Sincronizando…")
        await sincro.sincronizarAhora(id: id)
    }

    private func alMovil(_ p: Pedido) async {
        guard let sincro else { return avisar("La sincronización no está lista.") }
        guard let urls = ficheros(p, ["imagenes", "archivos"]), !urls.isEmpty else {
            return avisar("No hay nada que mandar al móvil.")
        }
        let aparato = p.texto("aparato") ?? "todos"
        let filas = aparato == "todos" ? sincro.filas : sincro.filas.filter { $0.id == aparato || $0.nombre == aparato }
        guard !filas.isEmpty else { return avisar("No hay ningún aparato «\(aparato)» en tu grupo.") }
        for f in filas { await sincro.enviar(urls, a: f.id) }
    }

    // MARK: Asistente, voz, timeline y texto

    func preguntar(_ p: Pedido) async {
        if let id = p.texto("automatizacion") {
            return await automatizacion(Pedido("automatizacion", ["operacion": .texto("ejecutar"), "id": .texto(id)]))
        }
        let texto = p.texto("texto") ?? ""
        guard !texto.isEmpty else { return ir(.asistente) }
        let modelo = asistente ?? ModeloAsistente(almacen: almacen)
        asistente = modelo
        modelo.nuevaConversacion()
        avisar("El asistente está en ello: «\(texto.prefix(80))».")
        await modelo.enviar(texto)
        let respuesta = modelo.burbujas.last { !$0.delUsuario }?.texto ?? ""
        avisar(respuesta.isEmpty ? "El asistente no contestó." : String(respuesta.prefix(400)))
    }

    /// `automatizacion` (E6): crear (la receta que manda el asistente tras
    /// preguntar), encender, apagar, borrar (sin preguntar: quien lo manda
    /// ya pregunto) o correr una ya. Lo hecho, en el globo, con los textos
    /// de Windows.
    private func automatizacion(_ p: Pedido) async {
        guard let nucleo = almacen?.nucleo else { return avisar("No se pudo abrir la carpeta de datos de PixPin.") }
        let operacion = p.texto("operacion") ?? ""
        if operacion == "crear" {
            guard let receta = p["receta"], !receta.esNulo else {
                return avisar("No se pudo guardar la automatización: falta la receta")
            }
            let texto = receta.texto ?? receta.json
            do {
                let nombre = try await Task.detached { try nucleo.automatizacionDeReceta(recetaJson: texto) }.value
                NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
                avisar("Automatización «\(nombre)» creada.")
            } catch {
                avisar("No se pudo guardar la automatización: \(ModeloAsistente.texto(de: error))")
            }
            return
        }
        guard ["activar", "desactivar", "borrar", "ejecutar"].contains(operacion) else {
            return avisar("Las automatizaciones no saben «\(operacion)».")
        }
        guard let id = p.texto("id") else { return avisar("Falta el id de la automatización.") }
        do {
            let nombre: String? = try await Task.detached { () throws -> String? in
                switch operacion {
                case "activar", "desactivar":
                    return try nucleo.automatizacionActivar(id: id, activa: operacion == "activar")
                case "borrar":
                    return try nucleo.automatizacionBorrar(id: id)
                default:
                    return try nucleo.automatizaciones().first { $0.id == id && $0.rota == nil }?.nombre
                }
            }.value
            guard let nombre else { return avisar("No hay ninguna automatización «\(id)».") }
            switch operacion {
            case "activar": avisar("Automatización «\(nombre)» encendida.")
            case "desactivar": avisar("Automatización «\(nombre)» apagada.")
            case "borrar": avisar("Automatización «\(nombre)» borrada.")
            default:
                MotorAutomatizaciones.compartido.aMano(id)
                avisar("Corriendo «\(nombre)»…")
            }
            if operacion != "ejecutar" { NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil) }
        } catch {
            avisar("No se pudo guardar la automatización: \(ModeloAsistente.texto(de: error))")
        }
    }

    private static let voz = AVSpeechSynthesizerCaja()

    private func leer(_ p: Pedido) {
        if p["parar"]?.booleano == true {
            HacerPedidos.voz.parar()
            return
        }
        var texto = p.texto("texto")
        if texto == nil, let r = p.texto("ruta") {
            let url = URL(fileURLWithPath: r)
            if ["txt", "md", "markdown"].contains(url.pathExtension.lowercased()) {
                texto = try? String(contentsOf: url, encoding: .utf8)
            } else {
                // Un PDF o un Word: su lector, que tiene su voz.
                if abrir(fichero: url) { avisar("Usa el altavoz del lector para escucharlo.") }
                return
            }
        }
        guard let texto, !texto.isEmpty else { return avisar("No hay nada que leer.") }
        HacerPedidos.voz.decir(texto)
    }

    private func timeline(_ p: Pedido) async {
        guard let texto = p.texto("texto") else { return ir(.timeline) }
        guard let nucleo = almacen?.nucleo else { return }
        let desfase = desfaseLocalMs()
        do {
            _ = try await Task.detached {
                try nucleo.timelineApuntar(texto: texto, fotos: [], audio: nil, duracionMs: 0,
                                           dicho: nil, cuando: nil, desfaseMs: desfase)
            }.value
            NotificationCenter.default.post(name: .pixpinCambioLocal, object: nil)
            avisar("Apuntado en el timeline: «\(texto.prefix(80))».")
        } catch {
            avisar("No se pudo apuntar en el timeline: \(textoDeError(error))")
        }
    }

    /// **Una herramienta del asistente pedida desde fuera** (el pedido
    /// `herramienta` de Windows). Se hace por el mismo camino que el
    /// asistente (lo que borra pregunta) y la respuesta va al fichero
    /// `respuesta` de una vez (`{"ok","texto"}`).
    private func herramienta(_ p: Pedido) async {
        let respuesta = p.texto("respuesta")
        let avisar = self.avisar
        func contestar(_ ok: Bool, _ texto: String) {
            guard let respuesta else { return avisar(texto) }
            HacerPedidos.escribirRespuesta(respuesta, ok: ok, texto: texto)
        }
        guard let nombre = p.texto("nombre"), let nucleo = almacen?.nucleo else {
            return contestar(false, "falta el nombre de la herramienta, o PixPin aun no ha abierto sus datos")
        }
        if let q = p.texto("preguntar") {
            let si = await withCheckedContinuation { c in
                CuadrosIPad.confirmar(q) { c.resume(returning: $0) }
            }
            if !si { return contestar(false, "El usuario dijo que no.") }
        }
        let args = p["args"]?.json ?? "{}"
        let manos = ManosDelAsistente(nucleo: nucleo, corte: CorteIa.nuevo()) { pregunta, responder in
            CuadrosIPad.confirmar(pregunta, responder: responder)
        }
        let desfase = desfaseLocalMs()
        let r = await Task.detached(priority: .userInitiated) {
            Result { try iaHacerHerramienta(nucleo: nucleo, nombre: nombre, argsJson: args, desfaseMs: desfase, app: manos) }
        }.value
        switch r {
        case let .success(t): contestar(true, t)
        case let .failure(e): contestar(false, ModeloAsistente.texto(de: e))
        }
    }

    /// Escribe la respuesta de una vez (aparte y renombrando): quien espera
    /// nunca lee un fichero a medias.
    nonisolated static func escribirRespuesta(_ ruta: String, ok: Bool, texto: String) {
        let destino = URL(fileURLWithPath: ruta)
        let tmp = destino.appendingPathExtension("parte")
        guard let datos = try? JSONSerialization.data(withJSONObject: ["ok": ok, "texto": texto]) else { return }
        try? datos.write(to: tmp)
        try? FileManager.default.moveItem(at: tmp, to: destino)
    }

    /// `ocr`: el texto de una o varias imagenes, al portapapeles.
    private func ocr(_ p: Pedido) async {
        let rutas = ([p.texto("ruta")].compactMap { $0 }) + (p["rutas"]?.lista?.compactMap(\.texto) ?? [])
        var trozos: [String] = []
        for r in rutas {
            guard Verbos.esImagen(r) || ["heic", "tiff"].contains((r as NSString).pathExtension.lowercased()),
                  let fuente = CGImageSourceCreateWithURL(URL(fileURLWithPath: r) as CFURL, nil),
                  let imagen = CGImageSourceCreateImageAtIndex(fuente, 0, nil) else {
                return avisar("«\(r)» no es una imagen.")
            }
            if let t = try? await sacarTexto(de: imagen), !t.isEmpty { trozos.append(t) }
        }
        let texto = trozos.joined(separator: "\n\n")
        guard !texto.isEmpty else { return avisar("No se ve texto en la imagen.") }
        UIPasteboard.general.string = texto
        avisar("Texto copiado (\(texto.count) caracteres): «\(texto.prefix(60))…»")
    }
}

/// La voz de `leer_en_voz` (una sola, que se puede parar).
@MainActor
final class AVSpeechSynthesizerCaja {
    private let voz = AVSpeechSynthesizer()

    func decir(_ texto: String) {
        voz.stopSpeaking(at: .immediate)
        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .spokenAudio)
        let frase = AVSpeechUtterance(string: texto)
        frase.voice = AVSpeechSynthesisVoice(language: VozAlta.idiomaDeLaApp)
        voz.speak(frase)
    }

    func parar() { voz.stopSpeaking(at: .immediate) }
}
