# PixPin para iPad: lo que falta respecto a Windows (25-oct-2026)

Inventario de funciones de **PixPin para Windows** (la referencia) que en el iPad **faltan o
están a medias**, zona por zona. Cada fila se comprobó con `grep` en
`apple/app/Sources/PixPinKit` (no por el nombre del fichero). Segunda referencia: el Mac
(`PIXPIN_PRO_MAC/apple/app/Sources/PixPinKit`, abreviado **Mac:**) y el README de Android.

**Estados**: ✅ hecho · 🟡 a medias · ❌ falta · ⛔ imposible en iOS (con la alternativa).
Dentro de cada tabla, **primero lo que se usa a diario**. Rutas de Windows relativas a
`PIXPIN_PRO_WINDOWS/apps/pixpin/src/` salvo que digan `crates/`; rutas del iPad relativas a
`apple/app/Sources/PixPinKit/`.

> Solo se listan las faltas y lo que está a medias; lo que está hecho sale únicamente cuando
> aclara algo (por ejemplo, que la alternativa de iOS ya existe).

---

## 0. Arreglos de un minuto: ganchos que nadie enchufa

Varias zonas dejaron un gancho (`static var …: ((…) -> Void)?`) para que otra lo rellene, y
**nadie lo rellena**: la función existe pero no sale o no hace nada. Se ve con
`grep -rnE "\.<gancho> *=[^=]"` (0 resultados).

| Gancho (dónde se declara) | Lo que se pierde hoy | Quién debería ponerlo |
|---|---|---|
| `GanchosChat.pinearFichero` (`Chat/MenuMensaje.swift:32`) | **«Pinear» no sale en el menú de ningún mensaje** (`sePuedePinear` da `false`) | `Pines/GestorPinesIPad.swift` (con `pinear(url:)`) o `Pines/CapaPines.swift` al arrancar |
| `GanchosChat.pinearTareas` (`Chat/MenuMensaje.swift:34`) | «Pinear» una lista de tareas viva | `Pines/GestorPinesIPad.swift` (`pinearMiniapp(chat:mensaje:miniapp:documento:)`) |
| ✅ `GanchosChat.hacerLeccion` (`Chat/MenuMensaje.swift:36`) | Hecho: lo pone `Recordatorios/ArranqueOrganizar.swift` → `HacerLeccionIPad` (`Lecciones/VistaLeccionesIPad.swift`) | — |
| ~~`GanchosAlmacen.alEntrarPdf`~~ (`Almacen.swift:45`) | ✅ Hecho: `Documentos/AligerarPdf.swift` (`ColaAligerar` con `aligerarEnSuSitio`; `DocumentosDelIPad.activar()` lo apunta con `Arranque.apuntar`) | Falta que `PixPinApp` llame a `DocumentosDelIPad.activar()` al arrancar (hoy lo llaman el lector y la lectura seguida al abrirse) |
| ✅ `GanchosAjustes.alConectar` (`Ajustes/ModeloAjustes.swift:97`) | Hecho: lo pone `Recordatorios/ArranqueOrganizar.swift` (lee Google Calendar al conectar) | — |
| ✅ `ManosDelAsistente.exportador` (`Asistente/ManosDocumentos.swift:21`) | ~~La herramienta «exportar» del asistente siempre dice «aún no se puede en el iPad»~~ Hecho: lo pone `Compartir/ArranqueCompartir.swift` (`CompartirDelIPad.activar()`, que llaman la lista de proyectos y el chat al aparecer): una foto o un PDF a PNG, JPG, SVG, PDF, página web o tal cual. Word/Excel no (no hay paginador de Office aquí) | `Compartir/` |
| ✅ `GanchosSincro.alLlegar` (`Sincro/Sincronizador.swift`) | Hecho: sin gancho, `Sincro/AvisoDeLlegada.swift` avisa con `CentroAvisos` (en la app si está delante; notificación local si está en el fondo). El gancho sigue para quien quiera cambiarlo | — |
| `PaqueteDeLaApp: AppIntentsPackage` (comentado en `Pedidos/IntencionesIPad.swift:14`) | La app no declara el paquete de intenciones: Siri, Atajos y Spotlight pueden no ver las de PixPinKit | `Sources/PixPin/PixPinApp.swift` (lo común: avisar) |

---

## 1. Proyectos, Chat y Voz

> Actualizado el 25-oct-2026 por la zona Proyectos/Chat/Voz/Compartir. Pruebas de lo puro en
> `Tests/PixPinKitTests/ChatCompartirVozTests.swift`; las de §1.3 (voz, tanda 3) en
> `Tests/PixPinKitTests/VozVentanasTests.swift`; la carpeta de una grabación y compartirla, en
> `Tests/PixPinKitTests/ChatGrabacionCarpetaTests.swift`.

### 1.1 Lista de proyectos

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| ✅ Cambiar el nombre de un proyecto (`proyecto-renombrar`) | `ventana_chat/proyectos.rs` | — (tampoco) | ✅ | `Proyectos/VistaProyectos.swift` (menú de la fila) | «Cambiar el nombre…» con el cuadro «Nombre del proyecto»; núcleo `Nucleo::renombrar_proyecto(ficha, nombre)` (`pixpin-nucleo/src/proyectos.rs`, el `renombrar` de Windows: índice + `proyecto.json`, sube `tocado`). No en Mensajes guardados ni Asistente |
| ✅ Archivar / desarchivar un proyecto | `ventana_chat/proyectos.rs` | — | ✅ | `Proyectos/VistaProyectos.swift` | «Archivar»/«Desarchivar» en el menú de la fila; núcleo `Nucleo::archivar_proyecto(ficha, archivar)` (`archivado` en `proyecto.json` o `MARCA_ARCHIVADO` en la ficha, como Windows) |
| ✅ Imagen de logo del proyecto (`proyecto-logo-poner/quitar`) | `ventana_chat/logo.rs` | — | ✅ | `Proyectos/LogoProyecto.swift` | Sustituida por la foto del chat (Windows 22bf4dc), que sí viaja. Un `logo.png` viejo se sigue viendo si el chat no tiene foto y se puede quitar con «Quitar imagen de logo» |
| ✅ Borrar varios marcados (`proyecto-borrar-varios`) | `ventana_chat/proyectos.rs` | — | ✅ | `Proyectos/VistaProyectos.swift` | «Elegir varios» (menú «…» o de la fila): marcas, barra abajo con Compartir y «Borrar los marcados» (pregunta antes) |
| ✅ Compartir proyectos (uno o los marcados) | `compartir.rs` (`de_proyectos`) | `Compartir/` | ✅ | `Proyectos/VistaProyectos.swift` | Con la hoja de formatos (sus hojas en PDF/HTML/PNG…, los originales y «Editable», el `.pixpin`) |
| ✅ Proyectos borrados (papelera) | `ventana_chat/proyectos.rs` | `Sincro/VistaCopias.swift` | ✅ | `Sincro/VistaCopias.swift` | Ahora también desde el «…» de la lista («Proyectos borrados…») |
| ✅ **La pestaña Funciones** (interruptor Chat · Funciones; mosaicos Grabar reunión, Nuevo lienzo, Tareas, Timeline, Horario, Capturas y Figuras) y la cabecera de la lista solo con Sincronizar | `ventana_chat/interruptor.rs`, `pixpin_ui::funciones` (2e0da60, 768b73f, a274ac0, 9620ac8) | `FuncionesDeLaLista.swift`, `ListaProyectos.swift`, `Vistas.swift` (083c76c) | ✅ | `FuncionesDeLaLista.swift`, `Proyectos/VistaProyectos.swift` | Traído del Mac a0efbae..083c76c. El interruptor va encima de la lista; Funciones la tapa (estrecho, toda la pantalla). Rejilla, textos y teclado del núcleo (`rejillaDeFunciones`, `textosDeFunciones`, `teclaEnFunciones`); cada mosaico a su sección o a su hoja. Pruebas en `FuncionesTests.swift`. La foto del chat, buscar en todos los chats y el giro de Sincronizar, en la fila de abajo |
| ✅ **La foto del chat** (imagen, emoji o color), **buscar en todos los chats** desde la lista (sección «Mensajes · N») y **el botón de sincronizar que gira** (marca roja si falló) | `ventana_chat/foto_del_chat.rs`, `buscar_en_chats.rs`, `sincronizar.rs` (22bf4dc, d106969, 0470294) | `Chat/FotoDelChat.swift`, `Chat/BuscarEnTodosLosChats.swift`, `Sincro/GiroDeSincronizar.swift` (083c76c..5601966) | ✅ | `Chat/FotoDelChat.swift`, `Chat/BuscarEnTodosLosChats.swift`, `Sincro/GiroDeSincronizar.swift`, `Proyectos/VistaProyectos.swift`, `Chat/InfoChat.swift` | Traído del Mac. La foto la escribe el núcleo en la ficha y viaja con el proyecto; se elige de Fotos (PhotosPicker) o Archivos (fileImporter), se pega con ⌘V, se arrastra y se pellizca. «Cambiar foto del chat…» en el menú de la fila (sustituye a «Poner imagen de logo…») y tocando el avatar de la isla. Pruebas en `FotoBuscarGiroTests.swift` |
| Ubicación del proyecto («Cambiar ubicación…», carpeta propia) | `ventana_chat/donde_vive.rs` | — | ⛔ | — | iOS: todo vive en la caja de arena. Alternativa: abrir la carpeta de datos en Archivos (`UIFileSharingEnabled`/`LSSupportsOpeningDocumentsInPlace` hoy en `false`, es del `App-Info.plist` común) |

### 1.2 El chat

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| ✅ **Foto anotada en la burbuja** (se ve lo dibujado encima, y se comparte fusionada) | `foto_anotada.rs`, `anotado_del_adjunto.rs` | `Chat/Miniaturas.swift` | ✅ | `Chat/FotoAnotadaChat.swift`, `Compartir/PrepararCompartir.swift` | `Nucleo.fotoAnotada` + el lienzo de la foto recortado a ella (o con todo, según `soloLaFoto`); cache con tope y se rehace al guardar o sincronizar. Al compartir (y al arrastrar) va fusionada y lo primero que se ofrece es el PNG |
| **Pinear** desde el menú del mensaje (`P`) | `ventana_chat/menu_v2.rs` | `Chat/MenuMensaje.swift` | 🟡 | ver §0 | El código está; falta que Pines ponga el gancho |
| **Hacer lección** desde un mensaje (`L`) | `ventana_chat/menu_v2.rs` | `Chat/MenuMensaje.swift` | 🟡 | ver §0 | Falta el gancho (zona Lecciones) |
| ✅ **Escuchar** un mensaje (voz alta) | `leer_en_voz.rs` | `Voz/LeerEnVoz.swift` | ✅ | `Chat/MenuMensaje.swift` → `Voz/LeerEnVoz.swift` | «Escuchar» en el menú de una nota, una nota de voz pasada a texto o una foto/archivo con descripción; la barra de escuchar (`BarraEscucharSuelta`) sale encima de la caja |
| ✅ **Enviar varias fotos con descripción** (cuadro con miniaturas, reordenar, quitar) | `ventana_chat/envio.rs`, `descripcion.rs` | `Chat/HojaAdjuntar.swift` | ✅ | `Chat/EnviarFotos.swift` | Miniaturas numeradas, arrastrar para reordenar (o su menú), ✕, «Añadir», la descripción va con la primera, «3 fotos · a Obra» |
| **Hoja de adjuntar completa**: conversación, captura, teleprónter, pronunciar, cronómetro, ruleta, gastos, traer del móvil, proyecto, página de un proyecto | `ventana_chat/hoja_adjuntar.rs` | `Chat/HojaAdjuntar.swift` | ✅ | `Chat/CajaEscribir.swift` | Chips «Proyecto entero», «Página de un proyecto», Contador, Ruleta, Gastos, Cronómetro, Temporizador, Alarma y, de la voz (§1.3), «Conversación», «Teleprónter», «Pronunciar» y «Grabar clase» (`AccionAdjuntar.voz` → `VozEnElChat.abrir`). «Traer del móvil» no hace falta: la sincronización ya lo trae |
| ✅ Adjuntar un **proyecto entero** (la puerta `PROYECTO`) | `ventana_chat/proyectos.rs` | — | ✅ | `Chat/ElegirProyecto.swift` | `Nucleo.adjuntarProyecto` |
| ✅ Adjuntar **una página de un proyecto** | `ventana_chat/paginas.rs` | — | ✅ | `Chat/ElegirProyecto.swift` | `paginasDeProyecto`, `paginaPorPintar` (se pinta con PDFKit) y `adjuntarPagina` |
| ✅ **Compartir con formatos** (PNG, JPG, SVG, PDF, página web, `.pixpin`, páginas a elegir) | `compartir.rs`, `crates/pixpin-ui` (`hoja_compartir`) | `Compartir/HojaCompartir.swift`, `PrepararCompartir.swift`, `PreparadoCompartir.swift` | ✅ | `Compartir/HojaCompartirIPad.swift`, `PrepararCompartir.swift`, `PreparadoCompartir.swift` | La hoja de Windows (redondeles de formatos, qué páginas, el peso) con las salidas de iOS: **Compartir** (hoja del sistema), **Guardar en Archivos…**, **Copiar**, **Imprimir…**; teclas ← → ↩ ⌘S ⌘C ⌘P ⎋. Se abre con `HojaCompartir.abrir(_ cosa: CosaCompartida, almacen:)` (`.mensajes`, `.proyectos`, `.documento`, `.ficheros`, `.imagen`, `.preparado`): **los lectores, los pines y la galería pueden usarla ya**. Usa `hojasDeDibujo`, `excalidrawDeDibujo`, `paqueteDeProyecto`, `paginaWebCompartida`, `nombreDeFicheroCompartido`. «Imprimir…» también en el menú de un lienzo, una página o una foto |
| ✅ **Arrastrar** una burbuja a otra app (Split View, Slide Over) | `salida.rs` (ventanita de salida) | `Compartir/Salida.swift` | ✅ | `Chat/ArrastrarBurbuja.swift` | El fichero tal cual; una foto anotada, un lienzo o una hoja, como PNG hecho al soltar; una nota, su texto. Lleva una marca para que soltarla en el mismo chat no la adjunte otra vez |
| Abrir con… otra app | `abrir_con.rs` | `Compartir/AbrirCon.swift` | ✅ | — | Alternativa iOS: «Compartir» de la hoja (abre en cualquier app) |
| ✅ Panel **Información** del chat (Todo, Fotos, Archivos, Voz, Dibujos, Fijados, Buzón) | `ventana_chat.rs` (`info-*`) | — | ✅ | `Chat/InfoChat.swift` | En el «…» de la cabecera; las fotos en cuadrícula, lo demás en lista; un toque lleva al mensaje |
| ✅ Etiquetas ⭐ ✅ ⏳ 💡 💰 📍 | `ventana_chat/menu_v2.rs` | `Chat/MenuMensaje.swift` | ✅ | `Chat/MenuMensaje.swift` | Ahora con `Nucleo.etiquetasDeMensaje` (el orden real) |
| Burbujas vivas de las mini-apps (contador, ruleta, gastos, relojes) | `mini_panel.rs` | `Chat/BurbujasQueLlegan.swift` | 🟡 | `Chat/Burbujas.swift` | Solo tareas y tabla tienen burbuja propia; las demás caen en la etiqueta genérica |
| ✅ Mayús+clic marca un tramo de burbujas | `ventana_chat/marcar.rs` | `Chat/BorrarMensajes.swift` | ✅ | `Chat/VistaChat.swift` | ⇧ + toque con el teclado del iPad (`onModifierKeysChanged`), con `EleccionChat.tramo` |
| ✅ Tarjeta de un enlace (título e imagen) | `ventana_chat/tarjetas.rs` | — | ✅ | `Chat/TarjetaEnlace.swift` | LinkPresentation (`LPMetadataProvider`), una vez por dirección; un toque abre Safari |
| ✅ **Enviar a Anki** (de una clase grabada, una nota o una foto) | `ventana_chat/anki.rs`, `crates/pixpin-anki` | `Chat/EnviarAAnki.swift` | ✅ ⛔ | `Chat/AnkiChat.swift`, `Chat/EnviarAAnki.swift` (del Mac tal cual), `Chat/MenuEstudio.swift` | Lo hace el núcleo del Mac (`Nucleo.enviarAAnki`, traído el 25-oct): una tarjeta por captura de una clase, la foto o las de la nota, mazo `PixPin::<proyecto>`. ⛔ AnkiConnect no existe en el iPad: el núcleo deja el fichero de «Archivo → Importar» en `<raíz>/anki/` y el iPad lo comparte (AnkiMobile, AirDrop al Mac/PC, Archivos) con las imágenes al lado (`pixpin-<id>.<ext>`, el nombre del núcleo) |
| ✅ **Carpetas de páginas** («Día 1, Día 2…»): tarjeta del chat, su ventana, «Añadir a carpeta…», «Nueva página», «Exportar PDF», «Carpeta de páginas» del clip | `ventana_chat/carpetas.rs`, `compartir/carpeta.rs` | `Chat/Carpetas.swift`, `Compartir/PdfDeCarpeta.swift` | ✅ | `Chat/CarpetasChat.swift`, `Chat/MenuEstudio.swift`, `Compartir/PdfDeCarpeta.swift` | Con el núcleo del Mac (25-oct): `crearCarpetaDePaginas`, `carpetaDePaginas`, `anadirACarpeta`, `cambiarCarpeta`, `paginaSiguienteDeCarpeta` (plantilla con `hoja_de_plantilla`/`figuras_de_serie`, como Windows), `hojasDeCarpeta`. Nace con el nombre listo para escribirlo (como el Mac). «Exportar PDF»: una página por elemento y de cada lienzo lo de su plantilla o sus marcos, con el índice de títulos, a la hoja de compartir |
| ✅ **La carpeta de una grabación** (una sola tarjeta en el chat, «Estudiar», «Adjuntar a la clase», lo adjuntado en ámbar) y **el chat sin sueltos** (lo de dentro de una carpeta no sale suelto) | `ventana_chat/carpetas.rs` (e015e2b, e1150e8) | `Chat/Carpetas.swift`, `Vistas.swift` | ✅ | `Chat/CarpetasChat.swift` (`BurbujaGrabacion`, `CarpetasDelChat.sinLoDeDentro`), `Chat/Burbujas.swift`, `Chat/VistaChat.swift`, `Chat/MenuEstudio.swift` | Del Mac a0efbae, con el núcleo de siempre (`grabacionDeCarpeta`, `dentroDeCarpetas`, `adjuntarALaClase`). La tarjeta: la primera captura, «● Grabación», lo que duró y las capturas, «Estudiar»; un toque abre «Estudiar la clase». Su ventana: el minuto en la esquina, sin reordenar (es el orden del tiempo), un toque estudia esa pieza en su minuto; «Adjuntar a la clase» con el selector de Archivos. Con la lupa sí sale, y al ir a él desde su carpeta. Pruebas en `ChatGrabacionCarpetaTests.swift` |
| ✅ **Ventana de las figuras** (categorías con su cuenta, buscador, miniaturas, la elegida en grande con el **intervalo** de las plantillas, renombrar, quitar), «Figura» y «Pedir una figura nueva» del clip y **«Guardar en mis figuras»** | `ventana_editor/figuras/ventana.rs`, `ventana_chat/figuras.rs` | `Lienzo/VentanaFiguras.swift`, `Chat/FigurasDelChat.swift` | ✅ | `Chat/FigurasChat.swift` | `GaleriaFiguras` del núcleo del Mac (con las figuras de serie de Windows instaladas). Se abre para mirar (cabecera del chat), para adjuntar (clip: un lienzo con la figura; en el asistente, «Mejora la figura…») o para insertar en el lienzo («Figuras…» de su botón, con el intervalo ya alargado) |
| Caja de soltar (recuadro flotante que mete en el chat lo soltado) | `ventana_chat/caja_de_soltar.rs` | — | ⛔ | — | Soltar encima del chat ya funciona (`Chat/Adjuntos.swift`); la alternativa a «flotante» es la **extensión de Compartir** (ver §9) |

### 1.3 Voz

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| Notas de voz y pasar a texto (Whisper) | `voz.rs`, `crates/pixpin-voz` | `Voz/TranscriptorVoz.swift` | ✅ | — | Alternativa iOS hecha: `SFSpeechRecognizer` en el aparato |
| ✅ **Biblioteca de audio** (toda la música y las notas de voz juntas) | `biblioteca_audio.rs` | `Voz/BibliotecaAudio.swift` | ✅ | `Voz/BibliotecaAudio.swift` | Traída del Mac (lo puro tal cual), en una hoja: Música y Notas de voz, Escuchar, «Escuchar después», «Escuchar todo (desde aquí)», «Ir al mensaje» y Grabar (a «Mensajes guardados»). Se abre desde el «…» de «Mensajes guardados» (como Windows) y desde el «…» de la lista |
| ✅ Reproductor en cola (varios audios abiertos) | `ventana_chat/reproductor_flotante.rs`, `reproductor.rs` | `Voz/ReproductorVoz.swift` | ✅ | `Voz/ReproductorVoz.swift`, `Voz/VistasVoz.swift` | `encolar`, `quitarDeLaCola`, `siguiente`; el «+N» de la barra abre la cola (Siguiente, quitar). Fuera del chat, flotante dentro de la app (ver «Reproductor flotante») |
| ✅ **Llamada secreta** (la nota de voz con hora suena como una llamada) y «Quién llama» | `llamada.rs`, `ventana_chat/quien_llama.rs` | `VozVentanas/LlamadaSecreta.swift`, `VozEnElChat.swift` | ✅ ⛔ | `Recordatorios/LlamadaIPad.swift`, `VozVentanas/VozEnElChat.swift` | La llamada la hace Recordatorios (§6: pantalla «Llamada entrante» dentro de la app y notificación con timbre; ⛔ CallKit). «Quién llama: Recado» es la primera línea de «Recordármelo» de una nota de voz (`EntradaQuienLlama` en `Chat/MenuMensaje.swift`): un cuadro con el nombre (40 letras; en blanco vuelve el de la nota). Se apunta también al poner la hora aunque no se toque (`QuienLlama.alPonerLaHora`), con `quienLlama`, `ponerQuienLlama` y `nombreDeLlamada` del núcleo, como Windows y el móvil |
| ✅ **Teleprónter** (el texto baja solo y uno se graba) | `teleprompter.rs` | `VozVentanas/Telepronter.swift` | ✅ | `VozVentanas/Telepronter.swift` | A pantalla completa encima de la app: franja de lectura, A−/A+, −/+ (velocidad dicha con palabras), raya de lo leído, Ensayar, 3-2-1, Grabar/Pausa/Tirar/Terminar; arrastrar el texto lo mueve a mano; la pantalla no se apaga. Lee «Lo escrito en la caja», una nota o un `.txt`/`.md` del chat (hoja «Teleprónter: qué leer»). La nota entra ya con su texto con minutos (`Desfile`). Teclado: Esc, Espacio, Intro, ← →, ↑ ↓, + −, RePág/AvPág. Sin la «otra pantalla» del Mac (va en la del iPad) |
| ✅ **Pronunciar** (hablar, oírse y repetir) | `pronunciar.rs` | `VozVentanas/Pronunciar.swift` | ✅ | `VozVentanas/Pronunciar.swift` | Mantener el micrófono (o Espacio) graba, al soltar se oye; «Oír otra vez», «Guardar en el chat» (se pasa a texto en el idioma practicado: 8 idiomas del móvil); guía: escribir un texto, una imagen/PDF/nota de Archivos o lo del chat; «Oír el texto» con la voz del iPad; páginas del PDF con ‹ › |
| ✅ **Conversación por turnos** (varias personas, con nombres) | `conversacion.rs`, `voz_en_chat.rs` | `VozVentanas/Conversacion.swift` | ✅ | `VozVentanas/Conversacion.swift` | 2 a 9 personas con nombre; se graba sin parar y se toca la ficha de quien habla (otra vez: nadie; con teclado 1–9 y 0). Entra con `turnos` (`turnosDeLaConversacion`) y se pasa a texto con los nombres (`[0:12] **Pepe:** …`) |
| ✅ **Grabar una clase o reunión** (audio, capturas en su minuto, transcribir, ver con las capturas) | `ventana_chat/grabar_clase.rs` | `Grabar/GrabarClase.swift`, `Grabar/VisorClase.swift` | ✅ ⛔ | `VozVentanas/GrabarClase.swift`, `VozVentanas/VisorClase.swift` | Elegir el proyecto, grabar (sigue con la pantalla bloqueada; tras una llamada sigue sola), Pausa, «Foto» (cámara) y «De Fotos» en su minuto, ✕ dos veces para tirar, Terminar. Se guarda con `Nucleo.guardarClase` (25-oct, el del Mac): la nota `Clase 24-10 18h05.m4a` y cada foto como captura `clase-<ms>-<n>.png` con su pie, enlazadas (`capturas`, `enGrabacion`) como Windows; pasa a texto («Dejarlo sin texto» lo corta), «Ver en el chat» y, en el menú de la nota, «Estudiar la clase». Detrás de todo entra **su carpeta** (`ClaseGuardada.carpeta`, Mac a0efbae), que el chat enseña como una sola tarjeta. ⛔ iOS no deja grabar el sonido de otras apps ni su pantalla: micrófono y fotos, sin el vídeo de la pantalla de Windows y el Mac |
| ✅ **Estudiar la clase como los visores** (barra de los visores, claro/oscuro, acercar con el pellizco, «Anotar» la captura o el fotograma del vídeo, lo anotado encima, arrastrar la barra del tiempo) | `estudiar_clase.rs` (fe0c9c4), `pixpin_ui::encuadre` | `Grabar/EstudiarClase.swift` | ✅ | `VozVentanas/EstudiarClase.swift` | Del Mac a0efbae: `BarraVisorIPad` arriba (Anotar, Compartir, luna/sol de `ModoNoche`, cerrar); `encuadre*` del núcleo (pellizco hacia los dedos, arrastrar, doble toque encaja, pastilla −/%/+/encajar, ⌘0 ⌘+ ⌘−, ⎋ encaja antes de cerrar); «Anotar» abre el lienzo `foto-<id>` encima (con el vídeo, `guardarFotogramaDeClase`); lo anotado se pinta encima con `fotoAnotada`; `clasePosicionALaVista` al arrastrar (cada 70 ms) |
| ✅ **Compartir una grabación** con la hoja de toda la app (HTML con el reproductor, PDF/PNG/JPG/SVG en A4, Word, Markdown, Texto, Anki, originales, `.pixpin`) | `compartir/clase.rs` (2921a1d) | `Compartir/CompartirGrabacion.swift` | ✅ | `Compartir/CompartirGrabacion.swift` (del Mac tal cual), `Compartir/PreparadoCompartir.swift` (`paginaWeb`), `VozVentanas/EstudiarClase.swift` | Una nota de voz o la carpeta de una grabación sola en «Compartir…» del chat, y «Compartir» de la barra de «Estudiar la clase». El HTML va entero (`cuantas: .ninguna`: la hoja dice «Va entero, tal cual»), lo escribe el núcleo (`grabacionWeb` y compañía) |
| Segundo idioma de las notas de voz | `voz.rs`, `pixpin_voz::dos_idiomas` | `Ajustes/FilasAjustes.swift` | 🟡 | `Voz/DosIdiomasVoz.swift` | Hecho en la zona Voz: con `vozSegundoIdioma` (y «Cada trozo como se dijo») la nota se pasa a texto en los dos idiomas con el dictado del iPad y se queda la versión de la que está más seguro; «Todo en el primero», solo el primero (el iPad no traduce). Falta la fila en Ajustes › Voz (zona Ajustes, código propuesto al coordinador) |
| ✅ La velocidad de escuchar se recuerda | `leer_en_voz.rs` | — | ✅ | `Voz/VelocidadGuardada.swift` | La de «Escuchar» (`LeerEnVoz`) y la del reproductor de notas de voz y música (`ReproductorVoz`), cada una la suya, en las preferencias del iPad |
| ✅ Reproductor flotante | `ventana_chat/reproductor_flotante.rs` | `Voz/FlotantesVoz.swift` | ✅ ⛔ | `Voz/ReproductorFlotante.swift` | Si algo suena y el chat no está delante (otra sección, un visor), la barra sale flotando abajo a la izquierda encima de todo lo de la app y se arrastra. ⛔ Encima de otras apps no: «Ahora suena» del Centro de control y la pantalla bloqueada (`MandosSistema.swift`) y sigue sonando de fondo |
| Micrófono flotante sobre cualquier app | `ventana_chat/microfono_flotante.rs` | `VozVentanas/Ventanita.swift` | ✅ ⛔ | `Pedidos/PedidosDelIPad.swift`, `Pedidos/IntencionesIPad.swift` | ⛔ Encima de otras apps no. Dentro de la app: el pedido `grabar` y la intención «Grabar nota de voz» (Siri, Atajos, botón de Acción) sacan el micrófono flotante («Guardar»/«Descartar»). Pendiente (opcional): `ControlWidget` del Centro de control y Live Activity mientras graba (necesitan una extensión de widgets en `project.yml`) |
| Voces de Microsoft en el lector de PDF | `crates/pixpin-voz/src/edge.rs` | `Voz/EdgeVoz.swift` | 🟡 | `Documentos/VozDocumentoIPad.swift` | `EdgeVoz` existe, pero el lector usa su propia voz (`AVSpeechSynthesizer`) y no la de la app (`LeerEnVoz.compartida`, que ya usa «Escuchar» del chat) |

---

## 2. Lienzo

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| **Las ocho letras del texto** (Excalifont, Nunito, Lilita One, Comic Shanns, Work Sans, Fraunces, Courier New, Caveat) | `crates/pixpin-render/letras/` | — (tampoco) | ✅ | `Lienzo/LetrasLienzo.swift`, `xcode/App/Fuentes/` | Las .ttf van en la app (`UIAppFonts`, y se registran solas si faltara la clave); se pintan renglón a renglón con el interlineado fijo de su familia, como Windows; negrita/cursiva de verdad o fingidas. ✅ (25-oct) **Se miden con las letras reales**: `MedidorLetrasLienzo` (Core Text, la misma cara con que se pinta) instalado en el motor con `instalarMedidorDelLienzo` (núcleo `letras_lienzo.rs`, como `medir_para_el_motor` de Windows) desde `LienzoDelIPad.activar()` |
| **Exportar** a SVG, PDF y página web que se vuelve a anotar; `.pixpin` editable | `compartir.rs`, `ventana_editor/` | `Lienzo/SacarLienzo.swift` + `Compartir/` | ✅ | `Lienzo/SacarLienzo.swift` | «Compartir…»: HTML (con el `.excalidraw` dentro), PDF vectorial, PNG, JPG, SVG y `.pixpin` (el proyecto del chat); copiar como PNG (⇧⌘C); imprimir con PDF vectorial. Sin la opción «fondo transparente» de Exportar… |
| ✅ **Zona al chat** (un rectángulo del lienzo va al chat con su vínculo de vuelta) | `zona_al_chat.rs` | — | ✅ | `Lienzo/SacarLienzo.swift` (`mandarZonaAlChat(_:caja:)`), núcleo `zona_al_chat.rs` | Como Windows: la foto (sin redondear) va al chat del proyecto como **sublienzo** (`foto-<id>` con la foto clavada, hoja «↳ Zona de…» detrás de su lienzo con `padre`/`deMensaje`, mensaje IMAGEN con `referencia`, `unido` y `vieneDe`) con `Nucleo.zonaAlChat`, y en el origen queda la marca azul con enlace (`EditorLienzo.marcarZona`, un paso de deshacer). Los planos siguen con la foto suelta (`mandarZonaAlChat(_:)`). Sin `pagina` (el «PDF «…» → página n» de una hoja-página no se dice aún) |
| ✅ Zona vinculada: tocarla lleva al sub-lienzo | `salto_por_enlace.rs` | — | ✅ | `Lienzo/ZonaVinculadaLienzo.swift`, `VistaEditorLienzoIPad.swift` (`pintarEnlaces`, `empezar`) | El icono de enlace (redondel azul con el eslabón, `enlacesEnLaVista`) en la esquina de la marca; tocarlo con cualquier herramienta (`enlaceEn`) resuelve el sublienzo como Windows (`dibujoDelEnlace`, también los enlaces viejos de un `.pixpin` importado) y lo abre (`mensajeDelDibujo`); en el sublienzo, «Volver a «…»» en la barra del visor (`SaltosDeLienzo`). Si no lleva a nada, avisa y no se cierra |
| ✅ **Ángulo con letra griega** (`W`, otra vez 90°, ↑↓ cambia la letra) | `dibujo/teclas.rs:172`, `dibujo/mano.rs` | — | ✅ | `Lienzo/HerramientasLienzo.swift`, `BarraHerramientasLienzo.swift` (`AtajosLienzo`), `VistaEditorLienzoIPad.swift` (`pressesBegan`) | «Ángulo» (W) en «Texto y nombres», tras «Punto»; «Extender» se queda sin letra como en Windows. Núcleo: `herramienta_de("angulo")`, `EditorLienzo::cambiar_letra_de_angulo(paso)` y `alternar_angulo_recto()` |
| **Fondo fijo del lienzo** (la foto o la página del PDF de fondo, no se mueve) | `fondo_lienzo.rs` | `Lienzo/FondoLienzo.swift` | ✅ | `Lienzo/FondoLienzo.swift` | El dibujo de una hoja-página se abre sobre su página; `EditorLienzoIPad(…, fondo:)` para otros; entra en lo exportado (`ponerPapel`) y en las lupas |
| ✅ Mandar fotos y texto a un lienzo **por su nombre** (`al_lienzo`) | `pila_lienzo.rs` | — | ✅ | `Pedidos/PedidosDelIPad.swift` (`alLienzo`) | Ya lo atiende Pedidos («Apilado en el lienzo «…»») |
| ✅ Qué herramientas se ven y dónde (Ajustes → Lienzo) | `ventana_ajustes.rs` (`Seccion::Lienzo`) | — | ✅ | `Ajustes/FilasOrganizar.swift`, `Lienzo/ModeloEditorLienzo.swift` (`ocultas`, `aplicarAjustes`) | La barra (y sus desplegables, sus letras y `elegir`) quita lo apagado en `[herramientas]` y `[herramientas.por_sitio]` para su `sitioHerramientas` (`lienzo` de fábrica); también el imán (`EditorLienzo.ponerIman`) y el suavizado «Natural» (filtro de 1 euro en pantalla, `SuavizadoLienzo.swift`). Se aplica al vuelo al cambiar Ajustes |
| ✅ Marcadores del lienzo **con su vista** (riel común) | `ventana_editor/marcas.rs`, `pixpin_ui::riel_marcas` | `Lienzo/MarcasLienzo.swift` | ✅ | `Lienzo/MarcasLienzo.swift`, núcleo `marcas_lienzo.rs` (`MarcasDelLienzo`, del Mac) | Poner (⌘M, tira, el siguiente toque lo planta con lo que se ve como su vista), ir (su punto, ⌘1…⌘9 o tocar el marcador: viaja a su vista, encuadre y aumento), «Guardar aquí la vista de ahora» (mantener pulsado o ⌘⇧1…9) y quitar. Ficheros de Windows: `pins/draw/<d>.marcas` y `.vistas.json`. El marco salió de la barra al riel (como Windows y el Mac). Moverlos con la mano aún no |
| ✅ Fusionar páginas del PDF del proyecto en un lienzo nuevo | `fusionar_paginas.rs` | — | ✅ | `Lienzo/FusionarPaginasLienzo.swift`, núcleo `fusionar_paginas.rs` | «Fusionar páginas del PDF…» en las figuras del lienzo de un chat con PDF: se eligen de 2 a 12 páginas, se pintan con Core Graphics a los anchos del núcleo (`anchosParaFusionar`) y `Nucleo.fusionarPaginas` escribe el lienzo (un marco con rótulo y la página con candado por cada una), su `DIBUJO` («Páginas 4 a 6») y su hoja; se abre. `HojaFusionarPaginas`/`FusionarPaginasIPad.fusionar` quedan públicos para Proyectos o Documentos |
| ✅ Ventanita del método de entrada (dictado, chino, japonés) en su sitio | — | `Lienzo/EditorLienzoView.swift` | ✅ | `Lienzo/MetodoDeEntradaLienzo.swift` | `UITextInput` en la vista: lo compuesto se pinta subrayado en el cursor, la ventanita de candidatos sale junto al cursor (`cajaDelCursor`) y al confirmar entra con `teclear` |
| Papel que sigue el modo noche común de los visores | `oscuro.rs`, `dibujo/tema.rs` | — | ✅ | `Lienzo/ModeloEditorLienzo.swift` | Con la luna puesta, un papel claro se ve gris noche y la tinta se adapta (solo se ve; no con fondo fijo) |
| «Dibujar con el dedo» aunque se use el Pencil | — (Notas) | — | ✅ | `Lienzo/EditorLienzoIPad.swift` (`BotonDedoLienzo`) | Se recuerda en el iPad |
| ✅ Editar una tabla ya dibujada | `ventana_editor/figuras.rs` (`editar_tabla`) | — | ✅ | `Lienzo/TablaLienzo.swift`, núcleo `editor_tabla.rs` | Doble toque con la flecha escribe en la celda; con la tabla elegida, «Editar la tabla…» en las figuras (o Intro) abre el cajetín con sus celdas, + / − fila y columna (de la celda activa) y «Cabecera»; «Aplicar» la redibuja en su sitio, con su letra, su color y sus figuras (`tablaElegida`, `reemplazarTabla`, un paso de deshacer). «Meter lo elegido en su celda» aún no |
| Fijar el lienzo como pin | `lector_pin.rs` | — | ✅ | `Lienzo/EditorLienzoIPad.swift` | El dibujo (o lo elegido) pintado, a la capa de pines |
| Resto de la barra (lazo, grafito, relleno, recortar, soldar, cotas, calibrar, escala gráfica, pixelar, lupa, foco, láser, marco, figuras, gráficas, tablas de Excel) | `dibujo/`, `caja_dibujo.rs` | `Lienzo/` | ✅ | — | Con Apple Pencil (presión, doble toque, palma) |

---

## 3. Documentos (PDF, Word, PowerPoint, EPUB, Excel, Markdown, HTML)

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| **Compartir el PDF/Word anotado** (tinta fusionada) | `anotado_del_adjunto.rs`, `compartir.rs` | `Compartir/PrepararCompartir.swift` | ✅ | `Documentos/SalidaDocumentoIPad.swift` | El original y «<nombre> (anotado).pdf» por la hoja del sistema (⇧⌘S); en lectura seguida, el documento en hojas con su tinta |
| **Imprimir** un PDF/Word (con lo anotado) | `marco_imprimir.rs`, `lector_pdf.rs` | `Documentos/LectorDocumento.swift` | ✅ | `Documentos/LectorPDFIPad.swift`, `VisorReflujoIPad.swift` | ⌘P o «Más › Imprimir…», con la tinta quemada; con marcos, una hoja por marco |
| **Marcos numerados para imprimir** (`I`, fichas 1-2-3, `Ctrl P` en grupo) | `marcos_doc.rs` | `Documentos/MarcosDoc.swift` | ✅ | `Documentos/MarcosDocIPad.swift`, `RielLectorIPad.swift`, `VisorReflujoIPad.swift` | En el lector (PDF, Word y PowerPoint paginados) y en la lectura seguida (Word, EPUB): el marco del riel, arrastrar con lápiz o dedo, fichas, quitar el último, imprimir en grupo (⌘P, una hoja por marco con su tinta); en `anot-<uid>.marcos.json` (`rutaAnotadaDelAdjunto`), en las unidades de la tinta |
| **EPUB y Word en lectura seguida** (reflujo: letra, columna, tinta, marcadores, índice) | `lector.rs`, `crates/pixpin-docs` | `Documentos/VisorReflujo.swift` | ✅ | `Documentos/VisorReflujoIPad.swift` | `DocumentoReflujo` con las letras del núcleo; letra/tipo/grosor, tinta con Pencil (fija la columna), espacios a los lados, marcadores, índice, buscar, escuchar con el verde, desplazar sola, compartir/imprimir, marcos para imprimir y «Más › Cambiar el nombre…» (⇧⌘R). Un Word se ve también paginado («Más › Ver paginado») |
| **Índice** (saltar a un capítulo) | `lector_pin.rs` | `Documentos/LectorDocumento.swift` | ✅ | `Documentos/IndiceDocumentoIPad.swift` | PDF por `outlineRoot`; Word/EPUB por `DocumentoReflujo.indice()`; marca el capítulo en que se está |
| **Desplazar sola** (palabras por minuto, «Terminas en 12 min») | `lector_pdf.rs` | `Documentos/AvanceLector.swift` | ✅ | `Documentos/AvanceLectorIPad.swift` | En el lector y en la lectura seguida; la velocidad se recuerda (`guardarPalabrasPorMinuto`) |
| **Aligerar PDF** (al entrar y a mano desde el mensaje) | `aligerar.rs`, `crates/pdfsqueeze-core` | `Documentos/AligerarPdf.swift` | 🟡 | `Documentos/AligerarPdf.swift`, `Chat/MenuMensaje.swift` | Al entrar (gancho §0; `PixPinApp` ya llama a `DocumentosDelIPad.activar()`), a mano desde el lector («Más › Aligerar el PDF») y desde el menú de la burbuja de un PDF («Aligerar el PDF», con el aviso «Aligerando el PDF…»). Falta el progreso bajo la burbuja (`ColaAligerar.compartida.progreso`/`resultado` por ruta): es de la zona del Chat |
| Cambiar el nombre desde el lector (`F2`) | `renombrar_doc.rs` | `Documentos/LectorDocumento.swift` | ✅ | `Documentos/LectorPDFIPad.swift` | «Más › Cambiar el nombre…» (⇧⌘R) con `renombrarDocumento`; la tinta y los marcos se van con él |
| **PowerPoint: presentar** a pantalla completa (`F5`), transiciones, notas, vídeos, **vista del orador** | `diapositivas.rs`, `crates/pixpin-office` | `Documentos/Diapositivas.swift` | ✅ | `Documentos/DiapositivasIPad.swift`, `PantallaExternaIPad.swift` | Botón ▶ del lector (⌥⌘↩): tercios y deslizar, teclas de PowerPoint, negro, notas (N), transiciones de Rust, pase solo, audio y vídeo. Con un proyector, monitor o AirPlay (escena `windowExternalDisplayNonInteractive`, enganchada desde esta zona sin tocar `Raiz/`), la diapositiva va a la pantalla externa y el iPad pasa a la vista del orador: la de ahora, la siguiente, las notas, la cuenta y el reloj |
| **Excel: anotar** con el lápiz (tinta de la hoja) y su riel | `visor_hoja/riel.rs`, `visor_hoja/capa.rs` | `Documentos/RielHoja.swift` | ✅ | `Documentos/TintaHojaIPad.swift`, `HojaCalculoIPad.swift` | El riel a la derecha: marcador (⌘M, en la celda activa, `x` columna e `y` hoja·10⁷+fila en la línea `marcas` del libro) y lápiz (Pencil o dedo; lápiz, resaltador, goma, colores, deshacer). La tinta en `rutaDeTintaDeHoja` (`anot-<uid>-h<i>`), en píxeles de Excel con la rejilla del libro (`rejillaDelLibro`, nuevo en el núcleo) y cada punto llevado a la misma fracción de su celda, como Windows. Sin marcos para imprimir en la hoja |
| Excel: gráficos del libro | `crates/pixpin-hojas` | — | ✅ | `Documentos/GraficosHojaIPad.swift` | `graficosDelLibro` (nuevo en el núcleo, el lector de Windows): columnas, barras, líneas, áreas, puntos, tarta y anillo con Swift Charts, en las celdas de su ancla y con los valores que guardó Excel |
| Excel: un `.xlsx` suelto (fuera de un chat) | `visor_hoja.rs` | `Documentos/HojaCalculo.swift` | ✅ | `Documentos/VisorDocumentoIPad.swift` (`LibroSueltoIPad`) | `Nucleo.abrirLibro(ruta:)` (nuevo): la misma hoja que las del chat, con tinta, marcadores y gráficos; se edita pero el archivo no cambia (aviso) y lo editado sale con «Compartir una copia en Excel» (`HojaDeCalculo.comoXlsx`, también en las del chat). CSV/TSV sueltos siguen en la tabla de solo ver; un `.xls` antiguo, Vista Rápida |
| Markdown: **buscar**, **comentarios al margen**, ver/editar, tablas editables, fotos bajo su renglón, menú `/` | `notas_md/`, `crates/pixpin-notas` | `Documentos/EditorNotaWeb.swift`, `EditorNota.swift` | ✅ | `Documentos/EditorNotaIPad.swift`, `NotaMasIPad.swift` | Buscar y reemplazar (⌘F). **Comentarios**: `Nucleo.abrirComentariosDeNota` (nuevo, el `md_comentarios` de `pixpin-docs` y el fichero hermano que viaja): lo comentado en amarillo, panel a la derecha con hilos, responder, resolver y borrar lo propio. **Fotos** bajo su renglón (`mdFotos`, `rutaEnLaNota`) y fotos nuevas desde la barra o `/imagen` (`adjuntarANota`). **Tablas**: con el cursor en una, «Editar la tabla» la abre como rejilla (`mdTablas`/`mdTablaTexto`). **Menú `/`** al principio del renglón con los bloques de Windows (`mdBarraComandos`/`mdBarraPoner`), filtrando lo tecleado. Las fotos no se redimensionan arrastrando (el ancho `|320` se respeta) |
| **Escuchar una nota** | `leer_en_voz.rs` | `Documentos/VozDeLaNota.swift` | ✅ | `Documentos/VozDeLaNotaIPad.swift` | Desde el cursor, sin las marcas, con lo que suena en ámbar y la barra de escuchar |
| HTML: **tinta** encima de la página, exportar | `visor_html.rs` | `Documentos/TintaWeb.swift`, `VisorHtml.swift` | ✅ | `Documentos/VisorHtmlIPad.swift` | El riel con el lápiz (o apoyar el Pencil): la tinta en `rutaDeTintaDePagina`, en píxeles CSS, dentro de la página como SVG (se desplaza y acerca con ella). «Exportar en PDF con lo anotado…» (con el original) e «Imprimir…» (`createPDF`). Sin marcadores ni marcos en la página |
| Office antiguos (`.doc`, `.ppt`, RTF, OpenDocument) en el visor propio | `office_convertido.rs` (convierte con Office) | — | ⛔ | `Documentos/VisorDocumentoIPad.swift` | Sin Office en iOS. Alternativa hecha: Vista Rápida. Mejor: leer RTF/ODT con el núcleo si se añade |
| Visio, `.msg` | `pin_office.rs` | `Documentos/PinOffice.swift` | ⛔ | — | Vista Rápida si el sistema sabe |
| Modo noche de los visores **recordado** (uno para todos) | `oscuro.rs` | `Documentos/TonoLector.swift` | ✅ | `Documentos/PantallaExternaIPad.swift` (`NocheRecordada`) | Al arrancar se pone `lectoresOscuros` en `ModoNoche` y cada cambio se guarda con `guardarLectoresOscuros` (el estado común de Windows y el Mac), sin tocar `Raiz/` |
| Abrir en una página y con la búsqueda escrita (`abrir` con `pagina`, `buscar`, `zona`; citas del asistente) | `pedidos.rs` | — | ✅ | `Pedidos/IrEnElLector.swift` → `LectorPDFIPad` | `HacerPedidos` pasa la petición (`IrEnElLector`) y el lector va a la página y escribe la búsqueda (`atenderPeticion`). Lo del plano es de la zona de Planos |
| **Reparar el PDF roto** de un proyecto (desde su copia limpia) | `pdf_del_proyecto.rs` | — | ✅ | `Documentos/PdfSano.swift` | Si no se abre, se rehace desde `pdfLimpio` del `proyecto.json` (o se enseña la copia) y se avisa |
| Lo de siempre del PDF: tinta con Pencil, resaltador, goma, marcadores, espacios a los lados, miniaturas, buscar, voz, negativo/oscuro, Word y PowerPoint maquetados | `lector_pdf.rs` | `Documentos/` | ✅ | — | |

---

## 4. Planos (DWG/DXF) y modelos 3D

> §4 a §9 actualizados el 25-oct-2026 (tanda 3) por la zona Planos/Pines/Captura/Sistema/Organizar/Pedidos. Pruebas de lo puro en
> `Tests/PixPinKitTests/Tanda3SistemaPlanosPinesTests.swift`.

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| **Imprimir los marcos** (`Ctrl P`, cada marco en su hoja, con membrete o sin él) | `plano_cad.rs` | `Planos/ImpresionPlano.swift` | ✅ | `Planos/PapelPlanoIPad.swift` | En vectores con `UIPrintPageRenderer` (`ImpresoraPlano`): lámina ISO 5457 y membrete de Windows (proyecto, lámina n/N, título, «Dibujó» con el nombre de los chats compartidos, escala 1:n, fecha, unidades) o «sin marco»; sin marcos, lo que se ve. Menú ⋯ y botón junto a las fichas |
| **Exportar el plano** (HTML que se mueve, PDF, DXF) | `plano_cad.rs` (`exportar`) | `Planos/ExportarPlano.swift` | ✅ | `Planos/PapelPlanoIPad.swift` | HTML (`Plano.paginaWeb`), PDF (una hoja por marco, A3), PNG/JPG (3200 px), DXF de un DWG (`Plano.exportarDxf`) y el original, por la hoja de compartir del sistema. Lo anotado se guarda antes |
| Anotar con **todas** las herramientas del lienzo (formas, texto, colores) | `plano_cad.rs` (`anotar`) | `Planos/AnotarPlano.swift` | ✅ | `Planos/AnotarPlanoIPad.swift` | La barra y el panel de estilo del lienzo (`BarrasLienzoEncima`) sobre un `ModeloEditorLienzo` con el editor de la capa; texto con teclado (`UIKeyInput` en la vista del plano), calibrar y dictar cota; se pinta con `PintorLienzo`. También meter una imagen (`ImagenAlAnotar`, en el centro de lo que se ve), la forma rápida al pararse (`RelojFormaRapida`) y la foto de una zona (la herramienta «zona»: el plano con lo anotado, en papel, entra encima como copia) |
| **Letras SHX** del usuario («Aa») y aviso de las que faltan | `letras_que_viajan.rs` | — | ✅ | `Planos/VisorPlanoIPad.swift` | Botón «Aa» con punto naranja, aviso «Faltan n letras…», hoja `LetrasDelPlanoIPad` (añadir desde Archivos, quitar las mías) y se vuelve a leer el plano |
| Buscar un texto en el plano (y abrir mirando a una zona) | `plano_cad.rs` | — | ✅ | `Planos/VisorPlanoIPad.swift` | Buscar (⌘F, `Plano.buscarTextos`): lleva al texto, lo marca, anterior/siguiente. Abrir mirando a una zona (y buscando) con `IrEnElLector` (`zona`, `buscar`), al abrir y con el plano ya abierto |
| **Marcadores del modelo 3D con todo el estado** (`Ctrl M`: cámara, corte, aislado, oculto; por `GlobalId`) | `modelo_bim.rs`, `plano_cad.rs` | `Planos/Marcos3D.swift` | ✅ | `Planos/MarcadoresPlanoIPad.swift`, `Planos/Visor3DIPad.swift` | En el `.vistas.json` de Windows con `claveDeElemento`/`claveDeNivel`; su tinta (`rutaDeTintaDeMarcador3d`) se ve con su vista y se **anota encima** con la barra del lienzo. Los planos también tienen «Marcador aquí» (⌘M) |
| Marcos numerados en 3D | `plano_cad.rs` | `Planos/Marcos3D.swift` | ✅ | `Planos/Visor3DIPad.swift` | `.marcos.json`; arrastrar pone uno, fichas para volver, imprimir (foto recortada por marco) |
| Exportar la foto de la vista 3D | `ventana3d.rs` | `Planos/VisorModelo3D.swift` | ✅ | `Planos/Visor3DIPad.swift` | PNG, JPG (lee el dibujable de Metal) y el original; con la tinta del marcador encima si la vista es la de uno (también al imprimir los marcos). La tinta de un marcador admite imágenes (botón de imagen) |
| **LandXML `.xml` y puntos `.csv`/`.txt`** como terreno | `modelo_bim.rs`, `crates/pixpin-bim` | `Planos/VentanasDePlano.swift` | ✅ | `Raiz/Navegacion.swift` (`TipoDeFichero.init(_ url:)`) | `TipoDeFichero` ya pregunta a `claseDeModelo(ruta:)` |
| Croquis 3D del móvil **con sus fotos** | `croquis3d.rs` | — | ✅ | `Chat/VistaChat.swift` (`abrirCroquis`) | El chat ya lo apunta con `CroquisDelChatIPad.apuntar(ruta:chat:referencia:)` antes de abrirlo |
| Aviso «DWG de Civil 3D sin gráficos» (`PROXYGRAPHICS = 0`) | `crates/pixpin-cad` | — | ✅ | `Planos/VisorPlanoIPad.swift` | De `ResumenPlano.sinDibujar` («sin dibujo: …»), con el texto de Windows |
| Fijar como pin el plano y el 3D | — | — | ✅ | `Planos/` | `GestorPinesIPad.compartido.pinearLoQueSeVe()` |
| Abrir varios planos a la vez (una ventana por fichero) | `plano_cad.rs` | `Planos/VentanasDePlano.swift` | ✅ | `Planos/OtrosPlanosIPad.swift`, `Sistema/VentanasDeVisor.swift` | «Abrir en otra ventana» de la barra del visor; en el menú ⋯ del plano «Abrir otro plano en otra ventana…» (⌘O, varios a la vez desde Archivos, copiados a `Application Support/Planos abiertos/` para que vuelvan con su tinta) y «Ver en 3D en otra ventana»; y «Abrir en otra ventana» en el menú de un mensaje del chat (`VentanasDeVisor.navegacionAparte()`). Sin varias ventanas, en la misma |
| Medir con enganche, capas, anotar con Pencil, 3D con caja de sección, niveles, categorías, aislar, planta, aristas | `crates/pixpin-cad` | `Planos/` | ✅ | — | |

---

## 5. Sincronizar y Ajustes

### 5.1 Sincronizar

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| Sincronizar con la app cerrada | `sincronizar/automatica.rs` | — | ⛔ / 🟡 | `Sincro/SincroDeFondo.swift` | iOS cierra la red. Alternativa hecha: `BGAppRefreshTask` (cuando iOS quiera) |
| ✅ **Chats compartidos sin grupo**: escuchar y anunciarse igual | `sincronizar/compartidos_ui.rs` | `Sincro/Compartidos.swift` | ✅ | `Sincro/Sincronizador.swift` | Ya lo hacía `anuncioDePresencia` (con grupo o con chats compartidos); ahora `hayCompartidos` lo enseña en Aparatos, salir del último chat compartido cierra la puerta (`refrescar`) y la sincronización manual con una persona prueba también su dirección recordada y la apunta |
| ✅ **Sincronizar sola con las personas de los chats compartidos** | `sincronizar/automatica.rs` (`personas_a_llamar`, `con_personas`) | — (el Mac no lo tiene) | ✅ | `Sincro/Sincronizador.swift` | Su propio `RelojSincro`, sondeo cada minuto (dirección recordada y, si no, su anuncio de Bonjour por `id`), una vuelta por persona con los chats comunes, nunca a la par que el grupo. `personasALlamar` con pruebas |
| ✅ Aviso de lo que llega con «Enviar a…» | `sincronizar/` | `Sincro/EnviarA.swift` | ✅ | `Sincro/AvisoDeLlegada.swift` | Ventanita de avisos con sonido, o notificación local en el fondo |
| ✅ Volver a primer plano | — | — | ✅ | `Sincro/Sincronizador.swift` (`parar`) | Al pararse se olvida quién contestaba (salvo los de hace < 2 min) y se vuelven a probar las direcciones recordadas: al volver hay vuelta con quien esté |
| ✅ Direcciones recordadas (ver, probar y olvidar) | `sincronizar.rs` | `Sincro/VistaSincro.swift` | ✅ | `Sincro/VistaSincroIPad.swift` | Se ven con su nombre, «Probar» y «Olvidar» (`Nucleo::olvidar_direccion(id)`, quita su línea de `sincro/direcciones.txt`) |
| ✅ Estado del reloj de sincronizar (con quién, fallos seguidos) | `sincronizar/automatica.rs` | — | ✅ | `Sincro/VistaSincroIPad.swift` | «Sincronizando sola con …» (grupo y personas) y los fallos seguidos |
| ✅ Enviar fotos de **Fotos** | — | — | ✅ | `Sincro/EnviarA.swift` | Todo «Enviar…» pregunta «Fotos y vídeos» o «Archivos» (`PhotosPicker`, HEIC→JPEG con `.compatible`) |
| Grupo, código, QR, unirse, «¿Qué sincronizar?», Lo mío manda, copias, papelera, enviar/recibir por código, Enviar a…, compartidos | `sincronizar/` | `Sincro/` | ✅ | — | Revisado contra el Mac fichero a fichero: igual salvo lo de iOS. `recibirEnvio` sin usar porque se usa `recibirEnvioPreguntando` (bien) |

### 5.2 Ajustes

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| ✅ **Buscador** de ajustes y «Restablecer sección» | `ventana_ajustes.rs` | `Ajustes/VistaAjustes.swift` | ✅ | `Ajustes/FilasAjustes.swift`, `Ajustes/VistaAjustesIPad.swift` | Filas como datos (como el Mac): `.searchable` sin tildes en todas las secciones, «Restablecer <sección>» en un paso de «Deshacer», el punto azul y «Volver al valor de fábrica» al mantener pulsado. Con pruebas |
| ✅ Sección **Voz** | `ventana_ajustes.rs` (`Seccion::Voz`) | `Ajustes/FilasAjustes.swift` | ✅ | `Ajustes/FilasAjustes.swift` | Voces de Microsoft (sí/no) y la voz del iPad de cada idioma con «Escuchar». El segundo idioma de las notas de voz no sale: el dictado del iPad no lo usa. La velocidad no se guarda en ningún sitio (la lleva el lector, zona Voz) |
| ✅ Sección **Asistente** | `ventana_ajustes.rs` (`Seccion::Asistente`) | `Ajustes/FilasAjustes.swift` | ✅ | `Ajustes/FilasAjustes.swift` | Motor, claves de la nube (la hoja del asistente), pensar, respaldo, documentos a la nube, ver lo dicho y las conexiones |
| ✅ «Conectar…» de las conexiones (Google) | `asistente/conexiones.rs` | `Ajustes/FilaConexion.swift` | ✅ | `Ajustes/FilaConexion.swift` | La hoja `ASWebAuthenticationSession` (`SesionWebConexion`); `AbridorDeConexion` es el `AbrirNavegador` del núcleo y `conectar` llama a `iaConectarCon(servicioId:abrir:)` |
| ✅ Sección **Tareas** (palabras de urgencia) | `ventana_ajustes.rs` (`Seccion::Tareas`) | — | ✅ | `Ajustes/FilasOrganizar.swift` | Urgente y Muy urgente, separadas por comas (`[tareas]`); el núcleo las fija al leer o guardar (`frase::fijar`), y `apuntar_tarea…` ya las relee |
| ✅ Sección **Lienzo** (imán, suavizado, qué herramientas y dónde) | `ventana_ajustes.rs` (`Seccion::Lienzo`) | — | ✅ | `Ajustes/FilasOrganizar.swift` + `Lienzo/ModeloEditorLienzo.swift` | Imán (activo, esquinas, medios, centros, radio 6–40 px), suavizado Excalidraw/Natural, «Herramientas en» Todos/Pantalla/Pines/Lienzo/Lector y un interruptor por herramienta, por los grupos de la barra (encender en un sitio la apagada en todos, como Windows). El lienzo lo lee al vuelo |
| ✅ Sección **Pinear** (palabras mágicas) | `ventana_ajustes.rs` (`Seccion::Pinear`) | — | 🟡 | `Ajustes/FilasOrganizar.swift`, `Ajustes/AjustesOrganizarIPad.swift` | Las palabras de cada herramienta y «De fábrica» (`[pinear]`, núcleo `magia.rs`, copia de `pixpin_pin::magia`). Falta que Pines lo use al pinear un texto: `AjustesOrganizarIPad.palabraMagica(texto, nucleo:)` (o `Nucleo.palabraMagicaDe(texto:)`) da `TIMER`, `CHECKLIST`, `DRAW`… o `nil` |
| Atajos (teclas) | `ventana_ajustes.rs` (`Seccion::Atajos`) | `Ajustes/Atajos.swift`, `GestorAtajos.swift` | ⛔ | — | Sin atajos globales en iOS; los de dentro de la app ya van con ⌘ |
| Captura, Apps predeterminadas | `ventana_ajustes.rs` | `Ajustes/FilasAjustes.swift` | ⛔ | — | Los días de la galería sí, en General |
| ✅ Tema Cosmos (el del móvil) | `tema_cosmos.rs`, `cielo.rs` | — | ✅ | `Ajustes/ModeloAjustes.swift` (`Tema.cosmos`), `Sistema/CieloCosmos.swift` | Una opción más del tema (Ajustes › General): noche y el cielo (degradado, dos nebulosas, 217 estrellas quietas, las cuentas de `cielo.rs`) detrás de la lista de proyectos (`.fondoCosmos`) |

**Lo que pide al núcleo (`apple/nucleo`, llevarlo también al Mac):**

1. ✅ Hecho (25-oct): conectar desde iOS, en `pixpin-nucleo/src/asistente/conexiones.rs`:
   ```rust
   /// Quien abre la pagina de permiso (iPad: `ASWebAuthenticationSession`).
   #[uniffi::export(callback_interface)]
   pub trait AbrirNavegador: Send + Sync {
       fn abrir(&self, url: String) -> bool;
   }

   /// Como `ia_conectar`, pero la pagina la abre Swift.
   #[uniffi::export]
   pub fn ia_conectar_con(servicio_id: String, abrir: Box<dyn AbrirNavegador>) -> Result<String, ErrorNucleo> {
       let s = servicio(&servicio_id)?;
       let parar = Arc::new(AtomicBool::new(false));
       if let Some(antes) = ESPERA.lock().unwrap_or_else(|e| e.into_inner()).replace(parar.clone()) {
           antes.store(true, Ordering::SeqCst);
       }
       let abrir = |url: &str| -> Result<(), String> {
           if abrir.abrir(url.to_string()) { Ok(()) } else { Err("no se pudo abrir la página de permiso".into()) }
       };
       conexiones::conectar_en(s, &s.extremos(), &carpeta(), &abrir, &parar, conexiones::oauth::ESPERA).map_err(mal)
   }
   ```
   En Swift: `final class AbridorDeConexion: AbrirNavegador` y en `FilaConexion.conectar` `iaConectarCon(servicioId: id, abrir: AbridorDeConexion())`.
2. ✅ Hecho (25-oct): olvidar una dirección: `Nucleo::olvidar_direccion(&self, id: String) -> Result<bool, ErrorNucleo>` (reescribe `sincro/direcciones.txt` sin esa línea).
3. ✅ Hecho (25-oct): `pixpin-nucleo/src/ajustes_organizar.rs` (`AjustesOrganizar` con urgencias, imán, suavizado y sus dos mandos, herramientas apagadas y por sitio, palabras mágicas; `ajustesOrganizar()`, `ajustesOrganizarDeFabrica()`, `guardarAjustesOrganizar(nuevos:)` con `guardar_conservando`, `palabraMagicaDe(texto:)`, `herramientaActivaEn`, `nombresDeHerramientas`) y `magia.rs`. Las urgencias se fijan para `tareas.rs` al leer y al guardar. Llevarlo también al Mac.

---

## 6. Galería, Timeline, Tareas, Buscar, Lecciones y Recordatorios

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| ✅ **Tareas en Tarjetas, Kanban u Horario** (sin la Lista; ⌃⌘T/K/H; la tarjeta con su foto de fondo; la vista se guarda con la palabra de Windows) | `tareas/`, `pixpin_ui::vista_tareas` (5139e5d) | `Tareas/Tareas.swift` (083c76c) | ✅ | `Tareas/VistaTareas.swift`, `Tareas/Tareas.swift` (`VistaDeTareas`, el `VistaTareas` del núcleo) | «Por fecha»/«Por lista» abren en Tarjetas. En el Horario: las tareas con hora como chapitas (`chipsDeLaSemana`, tocar una la enseña en las tarjetas), el número de cada día, y tocar una asignatura enseña sus tareas pendientes y «Abrir en Tareas». Pruebas en `TareasVistasTests.swift` |
| ✅ **«clase» y «carpeta» en Buscar** y el pedido `carpeta_paginas` (abrir, nueva, página siguiente sin abrir el chat) | `flow`, `carpetas_flow.rs` (22c3ce2, 211a1cd) | `Buscar/EstadoBuscar.swift`, `CarpetasSinChat.swift` (083c76c) | ✅ | `Buscar/CarpetasSinChat.swift`, `Buscar/ModeloBuscar.swift`, `Buscar/VistaBuscar.swift`, `Pedidos/HacerPedidos.swift` | La carpeta se abre en su hoja (`VistaCarpeta`), la página nueva en el lienzo; `ventana clase` y `ventana horario` también |
| **Horario de clases semanal** (rejilla, arrastrar asignaturas, alargar en saltos de 15 min, colores) | `horario.rs`, `horario/` | `Recordatorios/HorarioClases.swift`, `VentanaHorario.swift` | ✅ | `Recordatorios/HorarioIPad.swift` (la vista Horario de Tareas, ⌃⌘H; «Horario de clases» abre Tareas en ella) | Lun–Sáb (+Dom), 7–22 h. Con el dedo: arrastrar la asignatura (o tocarla y tocar el hueco), mover la clase manteniendo y arrastrando, alargar por su asa; color, borrar con «Deshacer», «Como texto» (en la barra de Tareas) y las próximas llamadas (también las de Google Calendar). ✅ La ficha de la clase (tocarla, o «Aula, nota y aviso…»): aula, nota para la voz y minutos de aviso, con los textos de Windows; y «Aula…» de una asignatura. El núcleo da ya `SemanaHorario.fichaDeClase`/`ponerFichaDeClase`/`ponerAulaDeAsignatura`/`antelacionDelHorario` |
| **La llamada antes de cada clase** con la nota de voz («Contestar») y «Nota de voz para la próxima clase» | `horario.rs`, `llamada.rs` | `VozVentanas/LlamadaSecreta.swift` | ✅ ⛔ | `Recordatorios/LlamadaIPad.swift`, `Vigia.swift`, `HorarioIPad.swift` | ⛔ CallKit no deja simular una llamada sin VoIP. Alternativa hecha: con la app abierta, la pantalla «Llamada entrante» encima de todo (Contestar/Colgar, Altavoz, reloj, 45 s y «Llamada perdida»); con la app cerrada, la notificación con **timbre de llamada** (`TonoLlamada`, en `Library/Sounds`) que al tocarla abre la llamada ya contestada. Las notas de voz con hora (llamada secreta) suenan igual |
| **Volver a llamar en** 5 min, 15, 30, 1 h, 2 h | `llamada.rs` | `VozVentanas/LlamadaSecreta.swift` | ✅ | `Recordatorios/LlamadaIPad.swift` | Sonando o contestada; y «+10 min» en la notificación y en la llamada perdida |
| Google Calendar en el horario y en los avisos | `calendario_google.rs` | `Recordatorios/` | ✅ | `Recordatorios/ArranqueOrganizar.swift` (gancho §0) | Al conectar se lee ya (y cada 15 min); sus eventos llaman como las clases (`avisosDelHorario`). ✅ Y **el calendario del propio iPad** (EventKit, `Recordatorios/CalendarioIPad.swift`): botón «Calendario del iPad» del horario (pide permiso); sus eventos con hora llaman como las clases, con hora y lugar, los minutos de antes del horario, sin repetir los que ya trae Google |
| **Pin de una tarea** con hora y «¿Por qué no ahora?» | `pines.rs`, `tareas/` | `Tareas/PinTarea.swift` | ✅ | `Tareas/PinTareaIPad.swift` («Fijar como pin» del menú de la tarea) | Capa propia encima de todo lo de la app (como la de avisos); se arrastra, vuelve al abrir la app; la nota va en la tarea (`ponerNotaTarea`) |
| Mientras se escribe una tarea, ver cómo quedará («Urgente · #casa · ⏰ …») | `tareas/` | — | ✅ | `Tareas/VistaTareas.swift` (`ComoQuedaraTarea`) | `tareaEntendida` + `marcasDeTarea` |
| Mover una tarea de lista por pedido (`mover_tarea`) | `pedidos.rs` | `Pedidos/HacerPedidos.swift` | ✅ | `Pedidos/PedidosDelIPad.swift` (`moverTarea`) | Con `Nucleo.moverTarea` |
| Timeline: **historia** de un momento (foto de fondo, texto en medio, degradado, barritas, nota de voz) | `timeline/` | `Timeline/TarjetasTimeline.swift` | ✅ | `Timeline/HistoriaTimeline.swift` | Tercios para pasar de foto, deslizar para pasar de momento, pinear la foto, lección |
| Timeline: **compartir como imagen** (tarjeta 1080×1350) | `timeline/` | `Timeline/TarjetasTimeline.swift` | ✅ | `Timeline/HistoriaTimeline.swift` (`TarjetaHistoria`) | Desde la historia y desde el menú del momento |
| Timeline: exportar a **PDF** | `timeline/` | `Timeline/Timeline.swift` | ✅ | `Timeline/HistoriaTimeline.swift` (`PdfDelTimeline`) | La misma página del HTML impresa en A4 (`WKWebView` + `UIPrintPageRenderer`); «Exportar» pregunta HTML o PDF |
| Galería: **Pinear** desde el menú | `galeria_capturas.rs` | `Galeria/VentanaGaleria.swift` | ✅ | `Galeria/VistaGaleria.swift` | También en la barra de las elegidas |
| Galería: mostrar en la carpeta | `galeria_capturas.rs` | `Galeria/VentanaGaleria.swift` | ⛔ ✅ | `Galeria/VistaGaleria.swift` | Alternativa hecha: «Guardar en Archivos…» |
| **Hacer lección** desde un mensaje | `ventana_chat/menu_v2.rs` | `Chat/MenuMensaje.swift` | ✅ | `Lecciones/VistaLeccionesIPad.swift` (`HacerLeccionIPad`) | Gancho §0 puesto en `ArranqueOrganizar`; `leccionDeMensaje` |
| Lecciones: la ficha de una lección nueva desde fuera (`leccion_nueva`, con texto y fotos) | `pedidos.rs`, `lecciones.rs` | `Pedidos/HacerPedidos.swift` | ✅ | `Pedidos/PedidosDelIPad.swift` | Lo hizo la zona de Pedidos |
| Galería con caducidad, OCR, conservar; Buscar con pestañas, recientes y letras rápidas; Tareas en Tarjetas/Kanban/Horario con urgencia, etiquetas e imágenes; Lecciones con gravedad, «Me volvió a pasar» y repaso; recordatorios con notificaciones y botones | — | — | ✅ | — | |

---

## 7. Asistente, Automatizaciones y Pedidos

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| **Modelo local** (bajar el GGUF, `llama-server`) | `asistente/`, `crates/pixpin-ia` | `Asistente/DescargaModelos.swift` | ⛔ / ✅ | — | Sin `llama-server` en iOS. Alternativa hecha: Apple Intelligence (`Asistente/MotorApple.swift`, iPadOS 26). Sin usar (correcto en iOS): `iaCatalogoLocal`, `iaRutaDescarga`, `iaVerificarDescarga`, `iaPrecargarLocal`, `iaSoltarMotores`, `iaTerminarMotor`, `iaCarpetaMotor`, `iaLimpiarHuerfanos`, `llamaLocalActivo`, `llamaLocalPreguntar*` |
| Herramienta «exportar» del asistente | `asistente/documentos_acciones.rs` | `Asistente/ManosDocumentos.swift` | ✅ | — | Lo pone `Compartir/ArranqueCompartir.swift` (`ManosDelAsistente.exportador`) |
| Receta nueva desde cero («+» en la fila de recetas) | `asistente/recetas.rs` | — | ✅ | — | «+» al final de la fila del chat «Asistente» (sale aunque no haya ninguna); sin nombre ni icono los pone el núcleo (`iaRecetaNueva`) |
| Borrar chat del asistente (vaciarlo) | `ventana_chat.rs` (`Accion::BorrarChat`) | — | ✅ | — | «Borrar chat» en `Asistente/CabeceraDelAsistente.swift`: pregunta con el texto de Windows, para lo que esté contestando y borra todo menos los `auto-…` |
| **El aviso flotante** (respuestas del asistente pedido desde fuera, automatizaciones, avisos de los pedidos) | `ventana_chat/aviso_asistente.rs` | — | ✅ | — | `Pedidos/AvisoFlotante.swift`: tarjeta abajo a la derecha, encima de todo lo de la app, que deja pasar los toques; «💭 Pensando…» mientras trabaja, la respuesta con «Abrir», «Sí»/«No» si pide permiso (el aspa es un «No»), se va a los 15 s salvo con el dedo o el puntero encima, varias apiladas. `CuadrosIPad.avisar` ya sale ahí (antes un `UIAlertController`) |
| Pedido `asistente` (encargo sin abrir el chat) | `automatizaciones/encargo.rs`, `cola.rs` | — | ✅ | — | Va al chat «Asistente» (allí queda la conversación), uno tras otro, con su aviso flotante (`PedidosDelIPad.swift`, `preguntarEnSuChat`) |
| Pedidos `al_lienzo`, `mover_tarea`, `posponer`, `borrar_captura` (con «Deshacer»), `capturar` (la hoja de capturar del iPad; `retardo` captura PixPin a los 3 s; `gif` graba un GIF de PixPin; `scroll` hace la captura larga), `pinear` (fichero, nota o mini-app de un mensaje), `pinear_ultima`, `pines` (cerrar con «Deshacer», ocultar/mostrar, devolver), `soltar` (elegir ficheros de Archivos para ese chat), `grabar` (micrófono flotante: «Guardar»/«Descartar»), `leccion_nueva` (la ficha con el texto; las fotos van al chat y la lección sale de ellas), tareas con imágenes al Inbox, `iconos` (nada, como debe) | `pedidos.rs` | — | ✅ | — | `Pedidos/PedidosDelIPad.swift`. `al_lienzo` también es verbo (`al-lienzo`) y acción del protocolo (faltaba en `Pedido.acciones`): busca el lienzo por su nombre (exacto o que lo contenga, en el proyecto o en todos), lo crea si no está y apila en una columna libre; si estaba abierto lo cierra, apila y lo vuelve a abrir |
| `abrir` con `pagina`, `buscar` y `zona` (y «ya abierto: llévalo allí») | `pedidos.rs` (`Que::Fichero`), `lector_pdf.rs`, `plano_cad.rs` | — | ✅ | `Documentos/LectorPDFIPad.swift`, `Planos/VisorPlanoIPad.swift` | Los dos recogen `IrEnElLector` (al abrir y con el fichero ya delante) |
| Tareas con imágenes en una lista de un proyecto | `pedidos.rs` (`anadir_pedida`) | — | ✅ | `Pedidos/HacerPedidos.swift` | Núcleo nuevo: `Nucleo.apuntarTareaConImagenesEn(chat:mensaje:texto:imagenes:desfaseMs:)` (copia cada imagen a `archivos/` de ese chat, con prueba en `tests/galeria_tareas.rs`) |
| Herramientas del asistente (las 47 filas de `pixpin_ia::acciones`, que en Apple llegan como pedidos por `ManosDelAsistente.pedido`) | `crates/pixpin-ia/src/acciones.rs` | — | ✅ | — | Comparadas fila a fila: todas las que dan pedido ya tienen quien las haga (`capturar_con_modo`, `grabar_pantalla`, `cerrar_pines`, `manejar_pines`, `pinear_*`, `borrar_captura`, `caja_para_soltar`, `leccion_nueva`, `mover_tarea`, `ir_a_pagina`…); las `(en el chat)` las hace el núcleo. `abrir_otra_ventana` con `horario` dice que no hay esa ventana |
| App Intents nuevas | Flow Launcher | `xcode/App/Intenciones.swift` | ✅ | — | Además de las 7 de antes: «Grabar nota de voz», «Capturar», «Apuntar en el timeline», «Mandar al lienzo», «Mandar al móvil», «Pinear la última captura». Las frases de Siri (10) y el `AppIntentsPackage` van en `Sources/PixPin/PixPinApp.swift` (§0) |
| Servidor MCP (`pixpin-mcp`) | `apps/pixpin-mcp` | `docs/mcp.md` | ⛔ | — | Sin procesos de fondo en iOS. Alternativa: App Intents (ya) |
| Extensión del navegador | `extensiones/` | `apple/extension-navegador` | ✅ | `xcode/Navegador/` (target `NavegadorPixPin`) | Extensión web de Safari: el menú del botón con lo seleccionado (Nueva tarea, Nueva nota, Al chat, Lección, Leer en voz alta, Preguntar al asistente) y la página (Foto de lo que se ve, Enlace al chat). La parte nativa (`SafariWebExtensionHandler`) lo deja en el buzón del App Group como enlace `pixpinmax://` (`PedidoDelNavegador` en `Sistema/BuzonCompartido.swift`) o como foto; la app lo hace al abrirse o en el acto (aviso de Darwin). Safari del iPad no tiene menú contextual para extensiones: las imágenes sueltas no; enviar al móvil no |
| Flow Launcher | `crates/pixpin-lanzador` | — | ⛔ / ✅ | `Sources/PixPin/PixPinApp.swift` | Alternativa: Spotlight + Atajos (App Intents, `Pedidos/IntencionesIPad.swift`); `PaqueteDeLaApp` y `AtajosDeLaApp` ya están en la app |
| Claude/DeepSeek, Apple Intelligence, Drive, calca esto, tablas desde foto, automatizaciones (hora, captura, sincronizar, a mano), enlaces `pixpinmax://` | — | — | ✅ | — | |

---

## 8. Pines y Captura

### 8.1 Pines

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| Pines **encima de otras apps** | `crates/pixpin-pin` | `Pines/Pin.swift` | ⛔ | — | Alternativa hecha: capa de pines dentro de la app y **PiP** para vídeo (`Pines/VideoPin.swift`) |
| **Pin de PDF** con su página (pasar de página en el pin) | `crates/pixpin-pin`, `lector_pin.rs` | `Pines/Pin.swift` | ✅ | `Pines/GestorPinesIPad.swift` (`pinear(pdf:pagina:)`, `pinear(documentoPdf:pagina:)`), `Pines/VistaPin.swift` | Barra ◀ 3 / 12 ▶, «Pinear esta página» / «Pinear todas las páginas»; la página vuelve al arrancar (`UserDefaults`, el índice de Windows no la tiene) |
| Guías azules para alinear e imán al soltar | `crates/pixpin-pin/src/guias.rs` | `Pines/Guias.swift` | ✅ | `Pines/GuiasPin.swift`, `Pines/VistaPin.swift`, `Pines/CapaPines.swift` | Sin el «Alt = sin guías» (no hay modificador al arrastrar con el dedo) |
| Arrastrar un pin a otra app (`Ctrl`+arrastrar) | `crates/pixpin-pin` | `Pines/Pin.swift` | ✅ | `Pines/VistaPin.swift` (asa de la mano de la barra, `onDrag`) | Lleva la foto que se ve, el fichero o el texto |
| Anotar el pin con las herramientas del lienzo | `crates/pixpin-pin` (`Anotar`) | `Pines/Anotar.swift` | ✅ | `Pines/AnotarPinLienzo.swift` | El lienzo con su barra y su panel de estilo sobre la foto del pin (de fondo fijo); «Listo» pinta lo anotado en la foto a sus píxeles y vuelve al pin. Si el lienzo no abre, el editor de captura de antes |
| Pin de audio (reproductor flotante) | `ventana_chat/reproductor_flotante.rs` | `Pines/Pin.swift` | ✅ | `Pines/VideoPin.swift`, `GestorPinesIPad.reproducir(_:)` | Barra propia (por dónde va, ±15 s, velocidad) y **la fila**: el pedido `reproducir` (con `ruta` o `rutas`) abre uno solo (un pedido nuevo le cambia la fila), suena solo, pasa al siguiente con «2 de 3 · nombre» y se va al acabar el último |
| Dejar pasar el clic (`Ctrl T`) | `crates/pixpin-pin` | `Pines/Pin.swift` | ⛔ | — | En una capa táctil dentro de la app no tiene sentido; la opacidad sí está |
| Devolver el último pin cerrado (`pines devolver_ultimo`) | `pines.rs` | `Pines/GestorPines.swift` | ✅ | `Pines/GestorPinesIPad.swift` (`devolverUltimoCerrado()`), `Pines/PanelAbiertos.swift` | En el panel (⇧⌘T), en «Nuevo pin» y en el botón de pines |
| Foto, nota, mini-apps (tareas, contador, ruleta, gastos, relojes), filtros, giro, opacidad, grupos, panel «Pines abiertos», se guardan y vuelven | — | — | ✅ | — | |
| La palabra mágica (`timer`, `todo`, `gastos`, `pizarra`, `hoja`… copiada **sola**) | `crates/pixpin-pin/src/magia.rs` | — | ✅ | `Pines/PalabraMagicaPin.swift` | «Pinear lo copiado» saca la mini-app (o un lienzo nuevo) en vez de una nota. Las palabras de fábrica de Windows; las propias de Ajustes › Pinear, cuando el núcleo las dé |
| «Pinear» desde el menú del mensaje (ganchos `GanchosChat.pinearFichero` y `pinearTareas`) | — | — | ✅ | `Pines/CapaPines.swift` (`Arranque.apuntar`) | |
| «Fijar como pin» de los visores | — | — | ✅ (API) | `Pines/GestorPinesIPad.swift` | `pinear(url:)`, `pinear(pdf:pagina:)`, `pinear(documentoPdf:pagina:)`, `pinearSoloPagina(pdf:pagina:)`, `pinear(imagen:)`, `pinearLoQueSeVe()`; casi todos los visores ya ponen su `alFijar`; faltan el de vídeo y Vista rápida (`Raiz/BarraVisorIPad.swift`) |

### 8.2 Captura

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| Capturar una zona, ventana o pantalla de otras apps | `captura2/`, `crates/pixpin-capture` | `Captura/Capturador.swift` | ⛔ | — | Alternativa hecha: cámara, escáner (VisionKit), Fotos, pegar y «Capturar PixPin» (`Captura/CapturadorIPad.swift`). La captura de pantalla del sistema llega por la extensión de Compartir (§9) |
| **Recortar a una zona** después (medidas exactas, 16:9 · 4:3 · 1:1, repetir la última zona `R`) | `captura2/` | `Captura/MedidasCaptura.swift`, `GeometriaCaptura.swift` | ✅ | `Captura/EditorCapturaIPad.swift`, `Captura/RecorteCaptura.swift` | ⌘6; mover la zona arrastrando desde dentro, An × Al, volver a la foto entera; lo anotado se recorta con la foto |
| Lupa con el color en HEX/RGB (`C` lo copia) | `captura2/` | `Captura/VistaCaptura.swift` | ✅ | `Captura/LupaCaptura.swift`, `Captura/EditorCapturaIPad.swift` | Cuentagotas (⌘7) o el Pencil por encima al recortar; también sale al elegir la zona; tocar el texto cambia HEX / RGB |
| Pila de capturas de la esquina | `pila_capturas.rs`, `crates/pixpin-pila` | `Captura/PilaCapturas.swift` | ✅ | `Captura/PilaCapturasIPad.swift` + `Pines/CapaPines.swift` | «Copiar» apila (10 s, o armada con un toque; tope 24); se pegan todas juntas; mantener: copiar todas, pinear, anotar, quitar |
| Captura con retardo (cuenta atrás) | `cuenta_atras.rs` | `Captura/CuentaAtras.swift` | ✅ | `Captura/CapturadorIPad.swift` | «Capturar PixPin en 3 s» (el número sale en la capa de pines; otra vez lo cancela) |
| Captura con scroll (larga) | `scroll.rs` | `Captura/CapturaConScroll.swift`, `CosidoScroll.swift` | ⛔ / ✅ | `Captura/CapturaLargaIPad.swift` | De otras apps no se puede. De PixPin, «Captura larga (con scroll)» en Capturar (y el pedido `capturar scroll`): el rollo más grande que se ve (chat, timeline, una lista) bajado pantalla a pantalla y cosido (tope 12 000 puntos), al editor de capturas |
| Grabar GIF / vídeo de la zona | `gif.rs`, `grabador.rs`, `crates/pixpin-record` | `Captura/GrabadorGif.swift`, `Grabar/` | ⛔ / ✅ | `Captura/GrabadorIPad.swift` | Otras apps: la grabación de pantalla del Centro de control. De PixPin: «Grabar PixPin» (`.mp4`) y «Grabar GIF de PixPin» (ReplayKit y, al parar, a GIF en bucle: 10 fps, 720 px, hasta 60 s), a la galería. Sin elegir zona (se recorta después en el editor) |
| Pin en vivo (copia una zona en tiempo real) | `pin_vivo.rs` | `Pines/PinEnVivo.swift` | ⛔ | — | Sin alternativa razonable |
| Anotador de pantalla (anotar encima de todo y guardarlo) | `anotador_al_chat.rs`, `overlay.rs` | `Captura/AnotadorPantalla.swift` | ⛔ / ✅ | `Captura/AnotadorPantallaIPad.swift` | Encima de otras apps no se puede. «Anotar la pantalla» en Capturar: la foto de PixPin con el lienzo encima y «Guardar» la deja en «Mensajes guardados» como Windows (`Nucleo.pantallaAnotadaAlChat`: la foto y su lienzo `foto-<id>` editable) |
| Gestos con Alt | `captura2/` | `Captura/GestosAlt.swift` | ⛔ | — | Alternativa: apretar el Apple Pencil Pro / atajos ⌘ |
| Anotar (lápiz, flecha, rectángulo, texto, mosaico), Copiar, Pinear, Al chat (selector con buscador), Texto (OCR), Guardar en la galería o en Fotos | — | — | ✅ | — | |

---

## 9. Lo común de la app y del sistema

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| **Extensión de Compartir** (recibir desde Fotos, Safari, la captura del sistema, otra app) | `recibir_de_windows.rs` («Imprimir a PixPin»), `vista_rapida.rs` | `Pedidos/ServiciosFinder.swift` | ✅ | `xcode/Compartir/ControladorCompartir.swift` (target `CompartirPixPin`) + `Sistema/BuzonCompartido.swift`, `Sistema/RecogedorCompartido.swift` | Fotos, PDF, vídeo, ficheros, enlaces y texto → lista de proyectos con buscador y comentario. La extensión deja lo recibido en el App Group `group.io.github.1xmanmax.pixpin` (`entrantes/`) y la app lo mete en el proyecto al abrirse/volver (y en el acto si está delante, por aviso de Darwin): ficheros por `adjuntar`, texto/enlaces/comentario como nota. Sin firmar (CI) no hay App Group: la extensión lo dice. Hay que dar de alta el grupo al firmar |
| **Abrir un `.pixpin`** (chat o fichero con todo dentro) y abrir desde Archivos | `compartir.rs`, `recibir.rs` | `Compartir/` | 🟡 | `xcode/Soporte/App-Info.plist` + `Sistema/AbrirFicheroDeFuera.swift` (lo llama `AbrirDesdeFuera`) | Hecho: tipos `.pixpin`, `.pixpin2d` (exportados), `.excalidraw`, `.md`, `.rvt`, LandXML, DWG/DXF/IFC (importados) y `.html`/`.xml` en `CFBundleDocumentTypes`; `.excalidraw` → lienzo nuevo en Mensajes guardados; `.xml` LandXML → 3D; lo demás a su visor; se borra la copia de `Documents/Inbox`. ✅ Un `.pixpin` entra en la lista de proyectos (`Nucleo::importar_proyecto(ruta, como_nuevo)`, `proyecto_que_ya_esta(ruta)`), preguntando «Actualizar el que tengo» / «Crear como nuevo» si ya está. **Falta**: `.pixpin2d` (no hay conversión a lienzo en UniFFI) |
| Varias ventanas (abrir un visor en otra ventana con Stage Manager) | `grupos_ventanas.rs` | `Planos/VentanasDePlano.swift` | ✅ | `Sistema/VentanasDeVisor.swift` + `Navegacion.abrirEnOtraVentana` + botón en `BarraVisorIPad` | `WindowGroup(id: "visor", for: PeticionVentana.self)` (Codable: iOS devuelve la ventana al reabrir); cada ventana con su `Navegacion`, cerrar el visor cierra la ventana. `PixPinApp.swift` ya declara el segundo `WindowGroup`. También desde el menú de un mensaje del chat y desde el plano (§4). El lienzo cierra con `Navegacion.compartida` (`ModeloEditorLienzo.cerrarVisor`): en otra ventana debería usar la del entorno |
| Bandeja / centro de control (buscador de acciones, favoritos, últimas capturas) | `centro_bandeja/` | `Bandeja/` | ⛔ / ✅ | `xcode/Widgets/WidgetsPixPin.swift` (target `WidgetsPixPin`), `Sistema/DatosDeWidgetsIPad.swift`, `Sistema/AccionesRapidas.swift` | Hecho: widgets «Últimas capturas» y «Tareas de hoy» (pequeño, mediano, grande; los datos y miniaturas los deja la app en el App Group) y las acciones rápidas del icono Capturar, Nueva nota, Buscar, Sincronizar (`UIApplicationShortcutItems`, por `DelegadoAppIPad`). ✅ Y los **controles del Centro de control** (`xcode/Widgets/ControlesPixPin.swift`, iPadOS 18): Capturar, Nueva nota, Nota de voz y Tareas, cada uno con su enlace `pixpinmax://` (`OpenURLIntent`) |
| Globo con el nombre y el atajo de cada botón | `globo_pista.rs` | — | 🟡 | — | `.help(...)` ya lo da con trackpad/teclado |
| Vista rápida (Espacio en el Explorador) | `vista_rapida.rs` | — | ⛔ | — | |
| Reparar el PDF roto del proyecto | `pdf_del_proyecto.rs` | — | ❌ | `Documentos/LectorPDFIPad.swift` | |

---

## 10. El núcleo de Rust que el iPad aún no usa

Métodos de `Nucleo` (`apple/PixPinCore/Sources/PixPinCore/pixpin_nucleo.swift`, protocolo
`NucleoProtocol`) que **ningún fichero Swift del iPad llama** (`grep -E "\.<metodo>\b"` en
`apple/app/Sources`): **48 de 213**. Agrupados por función; entre paréntesis, la sección de
este documento.

| Función | Métodos sin usar | Nota |
|---|---|---|
| Lectura seguida de Word y EPUB (§3) | `anotarReflujo`, `lecturaDeReflujo`, `guardarLecturaDeReflujo`, `hayTintaDeReflujo`, `dondeAnota` | Más la clase `DocumentoReflujo` entera |
| Desplazar sola (§3) | `palabrasPorMinuto`, `guardarPalabrasPorMinuto` | |
| Modo noche común de los visores (§3) | `lectoresOscuros`, `guardarLectoresOscuros` | |
| Tinta que viaja de marcos, Excel, web, notas y marcadores 3D (§3, §4) | `rutaAnotadaDelAdjunto`, `rutaDeTintaDeHoja`, `rutaDeTintaDePagina`, `rutaDeTintaDeMarcador3d` | |
| Renombrar un documento (§3) | `renombrarDocumento` | |
| Aligerar PDF (§3) | `aligerarEnSuSitio` | |
| Hojas de cálculo (§3) | `esHojaDeCalculo` | |
| Editor de notas web de Windows (§3) | `abrirNota` | El iPad usa su `UITextView` (decisión válida); la `SesionNota` tampoco se usa |
| Compartir con formatos y `.pixpin` (§1.2, §2) | `paqueteDeProyecto`, `hojasDeDibujo`, `excalidrawDeDibujo` | |
| Foto anotada (§1.2) | `fotoAnotada` | |
| Adjuntar proyecto y página (§1.2) | `adjuntarProyecto`, `paginasDeProyecto`, `paginaPorPintar`, `adjuntarPagina` | |
| Etiquetas del chat (§1.2) | `etiquetasDeMensaje` | |
| Lecciones desde un mensaje (§0, §1.2) | `leccionDeMensaje` | |
| Voz: notas de las ventanitas y llamada secreta (§1.3) | `adjuntarGrabacion`, `llamadaSecreta`, `quienLlama`, `ponerQuienLlama` | |
| Horario de clases (§6) | `ficheroDelHorario`, `notaDeVozDeClase` | Más `SemanaHorario`: `bloques`, `asignaturas`, `anadirAsignatura`, `borrarAsignatura`, `alargar`, `cambiarColor` |
| Pin de tarea (§6) | `tareaPorTitulo`, `ponerNotaTarea` | |
| Asistente (§7) | `iaRecetaNueva` | |
| Planos: letras SHX (§4) | `anadirLetrasDePlanos`, `letrasDePlanos`, `quitarLetrasDePlanos` | |
| Croquis 3D con fotos (§4) | `abrirCroquis` | |
| Anotador de pantalla (§8.2) | `pantallaAnotadaAlChat`, `ponerAlDiaPantallaAnotada` | |
| Sincronizar (§5.1) | `hayChatsCompartidos`, `chatCompartido`, `direccionesRecordadas` | |
| Ya cubierto por otro camino (no hace falta) | `recibirEnvio` (se usa `recibirEnvioPreguntando`), `lienzoPantallaCompleta`, `recordarLienzoPantallaCompleta` (en el iPad el lienzo siempre va a pantalla completa) | |

### 10.1 Otras clases y funciones sueltas del núcleo sin usar

| Clase o grupo | Sin usar | Para qué (sección) |
|---|---|---|
| `EditorLienzo` | `svgParaExportar`, `ponerPapel`, `tintaDePantalla`, `cajaDelCursor`, `cambiarEstiloSinEscala`, `ponerAEscala`, `punteroSinEscala`, `tintaGrosor`, `esDeDocumento`, `cuantosElementos` | Exportar SVG, fondo fijo, anotador, método de entrada (§2, §8.2) |
| `DocumentoOffice` | `esPresentacion`, `transicion`, `notas`, `medios`, `bytesDeMedio`, `terminado` | Presentar PowerPoint (§3) |
| `DocumentoReflujo` | la clase entera (`colocar`, `esLibro`…) | EPUB/Word reflujados (§3) |
| `HojaDeCalculo` | `ponerEstilo`, `rutaOriginal` | Formato de celda, libro original (§3) |
| `Plano` | `buscarTextos`, `exportarDxf`, `paginaWeb`, `letrasQueFaltan`, `esDwg` | §4 |
| `Modelo3dApple` | `claveDeElemento`, `claveDeNivel`, `elementoDeClave`, `nivelDeClave` | Marcadores 3D (§4) |
| `SemanaHorario` | `bloques`, `asignaturas`, `anadirAsignatura`, `borrarAsignatura`, `alargar`, `cambiarColor` | Horario (§6) |
| `SesionNota` | `adjuntarFicheros`, `mirarFuera`, `servir` | Editor web de notas (§3) |
| `RelojSincro` | `conQuien`, `fallosSeguidos` | §5.1 |
| `Escucha` | `atenderUna` | Se usa `atender` (bien) |
| Funciones sueltas | `aligerarPdf`, `aligerarPdfConAvance`, `nivelParaEntrar` (aligerar); `velocidadDeBajar`, `otraVelocidadDeLeer`, `contarPalabras`, `rotuloDeLoQueQueda` (desplazar sola); `letrasDeLectura`, `columnaDeLecturaQueCabe`, `tamanoDeLetraVecino`, `conMarcadorDeLectura`, `parrafoDelVerde`, `seAbreEnElVisor`, `notaPagina` (reflujo y notas web); `capasDeTransicion` (diapositivas); `paginaWebCompartida`, `nombreDeFicheroCompartido` (compartir); `tareaConNota`, `tareaEntendida` (tareas); `turnosDeLaConversacion` (conversación); `nombreDeLlamada` (llamada); `mdAlternarCasilla`, `mdCasillaEn`, `mdEnvolver`, `formulaPlano` (notas: el iPad lo hace en Swift; mejor usar estos para que se comporte igual); `miniConTitulo`, `miniTodas` (mini-apps); `camaraCercaDe`, `camaraHacia`, `claseDeModelo` (3D); `espaciosDeHoja`, `anchoDeHoja`, `otroEspacioDeLado`, `ladosDeLaColumna`, `aumentoMaximoDeLectura`, `aumentoMinimoDeLectura` (lector, el iPad tiene sus cuentas); IA local (`iaCatalogoLocal`, `iaRutaDescarga`, `iaVerificarDescarga`, `iaPrecargarLocal`, `iaSoltarMotores`, `iaTerminarMotor`, `iaCarpetaMotor`, `iaLimpiarHuerfanos`, `iaHayClave`, `iaCatalogo`, `llamaLocal*`, ⛔ en iOS); `tiemposAuto`, `hayTransporte`, `instalarMedidorDeLetras` | |

---

## Cómo se hizo

- Funciones de Windows: `README.md`, `apps/pixpin/src/` (cabeceras `//!` de cada módulo,
  `ventana_chat/`, `ventana_ajustes.rs`, `dibujo/teclas.rs`), `crates/pixpin-ui`,
  `crates/pixpin-store/i18n/es-ES/main.ftl` y `docs/protocolo-pedidos.md`.
- Mac: los ficheros de `PixPinKit` que el iPad no tiene, y su README («Qué trae»).
- iPad: `grep` de cada función por sus palabras (en español y por el nombre de la llamada al
  núcleo) en `apple/app/Sources/PixPinKit`, más `xcode/project.yml` y `App-Info.plist`.
- Núcleo: los `func` de `NucleoProtocol` y de las demás clases, comprobando
  `grep -E "\.<metodo>\b"` sobre todo `apple/app/Sources`; las notas de cada método salen de
  sus comentarios `///` en `apple/nucleo/pixpin-nucleo/src`.
