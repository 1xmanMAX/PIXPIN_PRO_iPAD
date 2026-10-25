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
| `GanchosChat.hacerLeccion` (`Chat/MenuMensaje.swift:36`) | **«Hacer lección» no sale en el chat** (`leccionDeMensaje` del núcleo sin usar) | `Lecciones/VistaLeccionesIPad.swift` o `Recordatorios/ArranqueOrganizar.swift` |
| `GanchosAlmacen.alEntrarPdf` (`Almacen.swift:45`) | **Aligerar el PDF al entrar**: el ajuste existe (`Ajustes/VistaAjustesIPad.swift:124`) pero no hace nada | Una zona de Documentos (falta `AligerarPdf.swift`, ver §3), con `aligerarEnSuSitio` |
| `GanchosAjustes.alConectar` (`Ajustes/ModeloAjustes.swift:97`) | Google Calendar conectado no llega al horario ni a los avisos | `Recordatorios/Vigia.swift` (eventos del calendario) |
| `ManosDelAsistente.exportador` (`Asistente/ManosDocumentos.swift:21`) | La herramienta «exportar» del asistente siempre dice «aún no se puede en el iPad» | `Compartir/` (falta la hoja de compartir, ver §1) |
| ✅ `GanchosSincro.alLlegar` (`Sincro/Sincronizador.swift`) | Hecho: sin gancho, `Sincro/AvisoDeLlegada.swift` avisa con `CentroAvisos` (en la app si está delante; notificación local si está en el fondo). El gancho sigue para quien quiera cambiarlo | — |
| `PaqueteDeLaApp: AppIntentsPackage` (comentado en `Pedidos/IntencionesIPad.swift:14`) | La app no declara el paquete de intenciones: Siri, Atajos y Spotlight pueden no ver las de PixPinKit | `Sources/PixPin/PixPinApp.swift` (lo común: avisar) |

---

## 1. Proyectos, Chat y Voz

### 1.1 Lista de proyectos

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| Cambiar el nombre de un proyecto (`proyecto-renombrar`) | `ventana_chat/proyectos.rs` | — (tampoco) | ❌ | `Proyectos/VistaProyectos.swift` (menú de la fila, hoy solo «Abrir» y «Borrar») | **El núcleo de Apple no tiene el método** (no hay `renombrarChat`): hay que llevarlo a `apple/nucleo` primero |
| Archivar / desarchivar un proyecto | `ventana_chat/proyectos.rs` | — | 🟡 | `Proyectos/VistaProyectos.swift` | Se ven los archivados (`archivado`), pero no hay entrada para archivar |
| Imagen de logo del proyecto (`proyecto-logo-poner/quitar`) | `ventana_chat/logo.rs` | — | ❌ | `Proyectos/VistaProyectos.swift` | Tampoco en el núcleo de Apple |
| Borrar varios marcados (`proyecto-borrar-varios`) | `ventana_chat/proyectos.rs` | — | ❌ | `Proyectos/VistaProyectos.swift` | Hoy uno a uno |
| Proyectos borrados (papelera) | `ventana_chat/proyectos.rs` | `Sincro/VistaCopias.swift` | ✅ | `Sincro/VistaCopias.swift` | Está dentro de Copias de seguridad; falta un acceso desde la lista |
| Ubicación del proyecto («Cambiar ubicación…», carpeta propia) | `ventana_chat/donde_vive.rs` | — | ⛔ | — | iOS: todo vive en la caja de arena. Alternativa: abrir la carpeta de datos en Archivos (`UIFileSharingEnabled`/`LSSupportsOpeningDocumentsInPlace` hoy en `false`) |

### 1.2 El chat

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| **Foto anotada en la burbuja** (se ve lo dibujado encima, y se comparte fusionada) | `foto_anotada.rs`, `anotado_del_adjunto.rs` | `Chat/Miniaturas.swift` | ❌ | `Chat/Burbujas.swift` (`BurbujaImagen`), `Chat/MenuMensaje.swift` (compartir) | `Nucleo.fotoAnotada` sin usar: la burbuja enseña la foto sin lo dibujado en el móvil o en el PC |
| **Pinear** desde el menú del mensaje (`P`) | `ventana_chat/menu_v2.rs` | `Chat/MenuMensaje.swift` | 🟡 | ver §0 | El código está; falta el gancho |
| **Hacer lección** desde un mensaje (`L`) | `ventana_chat/menu_v2.rs` | `Chat/MenuMensaje.swift` | 🟡 | ver §0 | Falta el gancho y usar `leccionDeMensaje` |
| **Escuchar** un mensaje (voz alta) | `leer_en_voz.rs` | `Voz/LeerEnVoz.swift` | ❌ | `Chat/MenuMensaje.swift` → `Voz/LeerEnVoz.swift` | `LeerEnVoz.compartida` existe pero no la llama nadie (solo `ReproductorVoz` la pausa) |
| **Enviar varias fotos con descripción** (cuadro con miniaturas, reordenar, quitar) | `ventana_chat/envio.rs`, `descripcion.rs` | `Chat/HojaAdjuntar.swift` | 🟡 | `Chat/VistaChat.swift` (`adjuntarFotos`), `Chat/CajaEscribir.swift` | Hoy entran directas; la descripción solo se añade después («Añadir descripción») |
| **Hoja de adjuntar completa**: conversación, captura, teleprónter, pronunciar, cronómetro, ruleta, gastos, traer del móvil, proyecto, página de un proyecto | `ventana_chat/hoja_adjuntar.rs` | `Chat/HojaAdjuntar.swift` | 🟡 | `Chat/CajaEscribir.swift` (`AccionAdjuntar`: hoy foto, archivo, lienzo, nota, tareas, tabla, cámara) | Las mini-apps ya existen como pines (`Pines/MiniApps.swift`): solo falta ofrecerlas aquí |
| Adjuntar un **proyecto entero** (la puerta `PROYECTO`) | `ventana_chat/proyectos.rs` | — | ❌ | `Chat/CajaEscribir.swift` | `Nucleo.adjuntarProyecto` sin usar (la burbuja `PROYECTO` sí se ve) |
| Adjuntar **una página de un proyecto** | `ventana_chat/paginas.rs` | — | ❌ | `Chat/CajaEscribir.swift` | `paginasDeProyecto`, `paginaPorPintar`, `adjuntarPagina` sin usar |
| **Compartir con formatos** (PNG, JPG, SVG, PDF, página web, `.pixpin`, páginas a elegir) | `compartir.rs`, `crates/pixpin-ui` (`hoja_compartir`) | `Compartir/HojaCompartir.swift`, `PrepararCompartir.swift`, `PreparadoCompartir.swift` | 🟡 | `Compartir/` (hoy solo `Compartible.swift`, la lógica, **sin vista que la use**) | Hoy «Compartir…» manda el fichero crudo. Sin usar: `hojasDeDibujo`, `excalidrawDeDibujo`, `paqueteDeProyecto`, `paginaWebCompartida`, `nombreDeFicheroCompartido` |
| **Arrastrar** una burbuja a otra app (Split View, Slide Over) | `salida.rs` (ventanita de salida) | `Compartir/Salida.swift` | ❌ | `Chat/Burbujas.swift` (`.draggable`/`onDrag`) | Solo la galería y las tareas tienen `.draggable`. Es la alternativa iOS natural a «arrastrar a WhatsApp» |
| Abrir con… otra app | `abrir_con.rs` | `Compartir/AbrirCon.swift` | 🟡 | `Chat/MenuMensaje.swift` | Alternativa iOS: la hoja de compartir (ya) |
| Panel **Información** del chat (Todo, Fotos, Archivos, Voz, Dibujos, Fijados, Buzón) | `ventana_chat.rs` (`info-*`) | — | ❌ | `Chat/VistaChat.swift` | |
| Etiquetas ⭐ ✅ ⏳ 💡 💰 📍 | `ventana_chat/menu_v2.rs` | `Chat/MenuMensaje.swift` | 🟡 | `Chat/MenuMensaje.swift` | Fila fija en Swift; `Nucleo.etiquetasDeMensaje` (el orden real) sin usar |
| Burbujas vivas de las mini-apps (contador, ruleta, gastos, relojes) | `mini_panel.rs` | `Chat/BurbujasQueLlegan.swift` | 🟡 | `Chat/Burbujas.swift` | Solo tareas y tabla tienen burbuja propia; las demás caen en la etiqueta genérica |
| Mayús+clic marca un tramo de burbujas | `ventana_chat/marcar.rs` | `Chat/BorrarMensajes.swift` | ❌ | `Chat/VistaChat.swift` | En iPad: tocar con ⇧ del teclado o arrastrar con dos dedos (gesto de selección múltiple de `List`) |
| Tarjeta de un enlace (título e imagen) | `ventana_chat/tarjetas.rs` | — | ❌ | `Chat/Burbujas.swift` | LinkPresentation (`LPMetadataProvider`) |
| Enviar a Anki | `ventana_chat/anki.rs`, `crates/pixpin-anki` | — | ❌ | `Chat/MenuMensaje.swift` | Nuevo en Windows (24-oct); no está en el núcleo de Apple |
| Carpetas de páginas («Día 1, Día 2…») | `ventana_chat/carpetas.rs` | — | ❌ | `Chat/` | Nuevo en Windows (26-oct); no está en el núcleo de Apple |
| Ventana de las figuras (galería de figuras hechas) | `ventana_chat/figuras.rs` | — | ❌ | `Chat/` o `Lienzo/` | Nuevo en Windows (24-oct) |
| Caja de soltar (recuadro flotante que mete en el chat lo soltado) | `ventana_chat/caja_de_soltar.rs` | — | ⛔ | — | Soltar encima del chat ya funciona (`Chat/Adjuntos.swift`); la alternativa a «flotante» es la **extensión de Compartir** (ver §9) |

### 1.3 Voz

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| Notas de voz y pasar a texto (Whisper) | `voz.rs`, `crates/pixpin-voz` | `Voz/TranscriptorVoz.swift` | ✅ | — | Alternativa iOS hecha: `SFSpeechRecognizer` en el aparato |
| **Biblioteca de audio** (toda la música y las notas de voz juntas) | `biblioteca_audio.rs` | `Voz/BibliotecaAudio.swift` | ❌ | `Voz/BibliotecaAudio.swift` (nuevo) | `ReproductorVoz` ya la nombra |
| Reproductor flotante en cola (varios audios abiertos) | `ventana_chat/reproductor_flotante.rs`, `reproductor.rs` | `Voz/ReproductorVoz.swift` | 🟡 | `Voz/ReproductorVoz.swift` | Hay barra de lo que suena; falta la cola |
| **Llamada secreta** (la nota de voz con hora suena como una llamada) y «Quién llama» | `llamada.rs`, `ventana_chat/quien_llama.rs` | `VozVentanas/LlamadaSecreta.swift` | ❌ | `Voz/` y `Recordatorios/Vigia.swift` | Hoy suena como un aviso. Sin usar: `llamadaSecreta`, `quienLlama`, `ponerQuienLlama`, `nombreDeLlamada`. ⛔ CallKit no deja simular llamadas; alternativa: pantalla de llamada dentro de la app + notificación con sonido largo |
| Teleprónter (el texto baja solo y uno se graba) | `teleprompter.rs` | `VozVentanas/Telepronter.swift` | ❌ | `Voz/` | |
| Pronunciar (hablar, oírse y repetir) | `pronunciar.rs` | `VozVentanas/Pronunciar.swift` | ❌ | `Voz/` | |
| Conversación por turnos (varias personas, con nombres) | `conversacion.rs`, `voz_en_chat.rs` | `VozVentanas/Conversacion.swift` | ❌ | `Voz/` | `turnosDeLaConversacion` sin usar |
| Grabar una clase o reunión (solo audio, transcribir) | `ventana_chat/grabar_clase.rs` | — | ❌ | `Voz/` | Nuevo en Windows (24-oct) |
| Micrófono flotante sobre cualquier app | `ventana_chat/microfono_flotante.rs` | `VozVentanas/Ventanita.swift` | ⛔ | — | Alternativa iOS: control del Centro de control / botón de Acción (App Intent «Grabar nota», iOS 18 `ControlWidget`) y Live Activity mientras graba |
| Voces de Microsoft en el lector de PDF | `crates/pixpin-voz/src/edge.rs` | `Voz/EdgeVoz.swift` | 🟡 | `Documentos/VozDocumentoIPad.swift` | `EdgeVoz` existe, pero el lector usa su propia voz (`AVSpeechSynthesizer`) y no la de la app |

---

## 2. Lienzo

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| **Las ocho letras del texto** (Excalifont, Nunito, Lilita One, Comic Shanns, Work Sans, Fraunces, Courier New, Caveat) | `crates/pixpin-render/letras/` | — (tampoco) | ❌ | `Lienzo/PintorLienzo.swift` + `xcode/Soporte/App-Info.plist` (`UIAppFonts`) | `UIFont(name: familia)` cae a la letra del sistema: un texto Excalifont del móvil se ve distinto. Ninguna `.ttf` en el proyecto |
| **Exportar** a SVG, PDF y página web que se vuelve a anotar; `.pixpin` editable | `compartir.rs`, `ventana_editor/` | `Lienzo/SacarLienzo.swift` + `Compartir/` | 🟡 | `Lienzo/SacarLienzo.swift` | Hoy: PNG por hoja (compartir) e imprimir. Sin usar: `svgParaExportar`, `paginaWebCompartida` |
| **Zona al chat** (un rectángulo del lienzo va al chat con su vínculo de vuelta) | `zona_al_chat.rs` | — | ❌ | `Lienzo/VistaEditorLienzoIPad.swift` (`sacarZona`) | Hoy la herramienta «Zona» solo hace la copia recortada dentro del lienzo |
| Zona vinculada: tocarla lleva al sub-lienzo | `salto_por_enlace.rs` | — | ❌ | `Lienzo/VistaEditorLienzoIPad.swift` | |
| **Ángulo con letra griega** (`W`, otra vez 90°, ↑↓ cambia la letra) | `dibujo/teclas.rs:172`, `crates/pixpin-ui/src/caja_herramientas.rs:155` | — | ❌ | `Lienzo/HerramientasLienzo.swift` (catálogo `todas`) | En el iPad la `w` es «Extender» |
| **Fondo fijo del lienzo** (la foto o la página del PDF de fondo, no se mueve) | `fondo_lienzo.rs` | `Lienzo/FondoLienzo.swift` | ❌ | `Lienzo/` (nuevo `FondoLienzo.swift`) | `EditorLienzo.ponerPapel` sin usar; la foto entra como imagen normal |
| Mandar fotos y texto a un lienzo **por su nombre** (`al_lienzo`) | `pila_lienzo.rs` | — | ❌ | `Pedidos/HacerPedidos.swift` | Pedido `al_lienzo` no atendido |
| Qué herramientas se ven y dónde (Ajustes → Lienzo) | `ventana_ajustes.rs` (`Seccion::Lienzo`) | — | ❌ | `Ajustes/VistaAjustesIPad.swift` | |
| Marcadores del lienzo con su zoom (riel común) | `lector_pin.rs` (riel) | — | ❌ | `Lienzo/EditorLienzoIPad.swift` | |
| Fusionar páginas del PDF del proyecto en un lienzo nuevo | `fusionar_paginas.rs` | — | ❌ | `Lienzo/` o `Documentos/` | |
| Ventanita del método de entrada (dictado, chino, japonés) en su sitio | — | `Lienzo/EditorLienzoView.swift` | 🟡 | `Lienzo/VistaEditorLienzoIPad.swift` (`firstRect`) | `EditorLienzo.cajaDelCursor` sin usar |
| Resto de la barra (lazo, grafito, relleno, recortar, soldar, cotas, calibrar, escala gráfica, pixelar, lupa, foco, láser, marco, figuras, gráficas, tablas de Excel) | `dibujo/`, `caja_dibujo.rs` | `Lienzo/` | ✅ | — | Con Apple Pencil (presión, doble toque, palma) |

---

## 3. Documentos (PDF, Word, PowerPoint, EPUB, Excel, Markdown, HTML)

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| **Compartir el PDF/Word anotado** (tinta fusionada) | `anotado_del_adjunto.rs`, `compartir.rs` | `Compartir/PrepararCompartir.swift` | ❌ | `Documentos/LectorPDFIPad.swift:1161` | Hoy `ShareLink(item: c.url)`: el PDF **sin** la tinta |
| **Imprimir** un PDF/Word (con lo anotado) | `marco_imprimir.rs`, `lector_pdf.rs` | `Documentos/LectorDocumento.swift` | ❌ | `Documentos/LectorPDFIPad.swift` | `UIPrintInteractionController` solo está en el lienzo y el plano |
| **Marcos numerados para imprimir** (`I`, fichas 1-2-3, `Ctrl P` en grupo) | `marcos_doc.rs` | `Documentos/MarcosDoc.swift` | ❌ | `Documentos/RielLectorIPad.swift` | Sin usar: `rutaAnotadaDelAdjunto` (`.marcos.json`, el que viaja) |
| **EPUB y Word en lectura seguida** (reflujo: letra, columna, tinta, marcadores, índice) | `lector.rs`, `crates/pixpin-docs` | `Documentos/VisorReflujo.swift` | ❌ | `Documentos/` (nuevo `VisorReflujoIPad.swift`) | Un EPUB va a Vista Rápida (`VisorDocumentoIPad.swift`). Sin usar: `DocumentoReflujo` entero, `anotarReflujo`, `lecturaDeReflujo`, `guardarLecturaDeReflujo`, `hayTintaDeReflujo`, `letrasDeLectura`, `columnaDeLecturaQueCabe`, `tamanoDeLetraVecino`, `conMarcadorDeLectura`, `seAbreEnElVisor`, `parrafoDelVerde` |
| **Índice** (saltar a un capítulo) | `lector_pin.rs` | `Documentos/LectorDocumento.swift` | ❌ | `Documentos/LectorPDFIPad.swift` (`PDFDocument.outlineRoot`) | Para Word/EPUB llega con el reflujo |
| **Desplazar sola** (palabras por minuto, «Terminas en 12 min») | `lector_pdf.rs` | `Documentos/AvanceLector.swift` | ❌ | `Documentos/LectorPDFIPad.swift` | Sin usar: `palabrasPorMinuto`, `guardarPalabrasPorMinuto`, `velocidadDeBajar`, `otraVelocidadDeLeer`, `contarPalabras`, `rotuloDeLoQueQueda` |
| **Aligerar PDF** (al entrar y a mano desde el mensaje) | `aligerar.rs`, `crates/pdfsqueeze-core` | `Documentos/AligerarPdf.swift` | ❌ | `Documentos/` (nuevo) + gancho §0 | Sin usar: `aligerarEnSuSitio`, `aligerarPdf`, `aligerarPdfConAvance`, `nivelParaEntrar` |
| Cambiar el nombre desde el lector (`F2`) | `renombrar_doc.rs` | `Documentos/LectorDocumento.swift` | ❌ | `Raiz/BarraVisorIPad.swift` / `LectorPDFIPad.swift` | `renombrarDocumento` sin usar |
| **PowerPoint: presentar** a pantalla completa (`F5`), transiciones, notas, vídeos | `diapositivas.rs`, `crates/pixpin-office` | `Documentos/Diapositivas.swift` | ❌ | `Documentos/` (nuevo `DiapositivasIPad.swift`) | Hoy se pagina como un PDF. Sin usar: `DocumentoOffice.esPresentacion`, `transicion`, `notas`, `medios`, `bytesDeMedio`, `capasDeTransicion`. Con AirPlay/pantalla externa iría muy bien |
| **Excel: anotar** con el lápiz (tinta de la hoja) y su riel | `visor_hoja/riel.rs` | `Documentos/RielHoja.swift` | ❌ | `Documentos/HojaCalculoIPad.swift` | `rutaDeTintaDeHoja` sin usar |
| Excel: gráficos del libro | `crates/pixpin-hojas` | `Documentos/HojaCalculo.swift` | ❌ | `Documentos/HojaCalculoIPad.swift` | |
| Excel: un `.xlsx` suelto (fuera de un chat) | `visor_hoja.rs` | `Documentos/HojaCalculo.swift` | 🟡 | `Documentos/VisorDocumentoIPad.swift` | Hoy a Vista Rápida |
| Markdown: **buscar**, **comentarios al margen**, ver/editar, tablas editables, fotos bajo su renglón, menú `/` | `notas_md/`, `crates/pixpin-notas` | `Documentos/EditorNotaWeb.swift`, `EditorNota.swift` | 🟡 | `Documentos/EditorNotaIPad.swift` | Hay Markdown vivo, fórmulas, casillas y la barra de formato; faltan buscar y comentarios. `abrirNota`/`SesionNota` (el editor web de Windows) sin usar |
| **Escuchar una nota** | `leer_en_voz.rs` | `Documentos/VozDeLaNota.swift` | ❌ | `Documentos/EditorNotaIPad.swift` | |
| HTML: **tinta** encima de la página, exportar | `visor_html.rs` | `Documentos/TintaWeb.swift`, `VisorHtml.swift` | ❌ | `Documentos/VisorHtmlIPad.swift` | `rutaDeTintaDePagina` sin usar. Buscar, recargar y compartir sí están |
| Office antiguos (`.doc`, `.ppt`, RTF, OpenDocument) en el visor propio | `office_convertido.rs` (convierte con Office) | — | ⛔ | `Documentos/VisorDocumentoIPad.swift` | Sin Office en iOS. Alternativa hecha: Vista Rápida. Mejor: leer RTF/ODT con el núcleo si se añade |
| Visio, `.msg` | `pin_office.rs` | `Documentos/PinOffice.swift` | ⛔ | — | Vista Rápida si el sistema sabe |
| Modo noche de los visores **recordado** (uno para todos) | `oscuro.rs` | `Documentos/TonoLector.swift` | 🟡 | `Raiz/BarraVisorIPad.swift` (`ModoNoche`) | `lectoresOscuros`/`guardarLectoresOscuros` sin usar: no viaja al estado común |
| Abrir en una página y con la búsqueda escrita (`abrir` con `pagina`, `buscar`, `zona`; citas del asistente) | `pedidos.rs` | — | 🟡 | `Pedidos/HacerPedidos.swift` (`abrir(fichero:)`) → `LectorPDFIPad`, `VisorPlanoIPad` | `Pedidos/Verbos.swift:434-442` mete `pagina` y `buscar` en el pedido, pero `HacerPedidos` no los lee: el documento se abre donde se dejó |
| Lo de siempre del PDF: tinta con Pencil, resaltador, goma, marcadores, espacios a los lados, miniaturas, buscar, voz, negativo/oscuro, Word y PowerPoint maquetados | `lector_pdf.rs` | `Documentos/` | ✅ | — | |

---

## 4. Planos (DWG/DXF) y modelos 3D

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| **Imprimir los marcos** (`Ctrl P`, cada marco en su hoja, con membrete o sin él) | `plano_cad.rs` | `Planos/ImpresionPlano.swift` | ❌ | `Planos/VisorPlanoIPad.swift` | Los marcos se dibujan y se borran, pero no hay «Imprimir» |
| **Exportar el plano** (HTML que se mueve, PDF, DXF) | `plano_cad.rs` (`exportar`) | `Planos/ExportarPlano.swift` | ❌ | `Planos/VisorPlanoIPad.swift` | Sin usar: `Plano.paginaWeb`, `Plano.exportarDxf` |
| Anotar con **todas** las herramientas del lienzo (formas, texto, colores) | `plano_cad.rs` (`anotar`) | `Planos/AnotarPlano.swift` | 🟡 | `Planos/AnotarPlanoIPad.swift` | Hoy lápiz sencillo (el propio fichero lo dice: «Pendiente: la barra entera») |
| **Letras SHX** del usuario («Aa») y aviso de las que faltan | `letras_que_viajan.rs` | — | ❌ | `Planos/VisorPlanoIPad.swift` | Sin usar: `anadirLetrasDePlanos`, `letrasDePlanos`, `quitarLetrasDePlanos`, `Plano.letrasQueFaltan` |
| Buscar un texto en el plano (y abrir mirando a una zona) | `plano_cad.rs` | — | ❌ | `Planos/VisorPlanoIPad.swift` | `Plano.buscarTextos` sin usar |
| **Marcadores del modelo 3D con todo el estado** (`Ctrl M`: cámara, corte, aislado, oculto; por `GlobalId`) | `modelo_bim.rs`, `plano_cad.rs` | `Planos/Marcos3D.swift` | ❌ | `Planos/Visor3DIPad.swift` | Sin usar: `Modelo3dApple.claveDeElemento`, `claveDeNivel`, `elementoDeClave`, `nivelDeClave`, `rutaDeTintaDeMarcador3d` |
| Marcos numerados en 3D | `plano_cad.rs` | `Planos/Marcos3D.swift` | ❌ | `Planos/Visor3DIPad.swift` | |
| **LandXML `.xml` y puntos `.csv`/`.txt`** como terreno | `modelo_bim.rs`, `crates/pixpin-bim` | `Planos/VentanasDePlano.swift` | 🟡 | `Raiz/Navegacion.swift:149` | Solo `.landxml`; un `.xml`/`.csv`/`.txt` va a Documentos |
| Croquis 3D del móvil **con sus fotos** | `croquis3d.rs` | — | 🟡 | `Chat/VistaChat.swift:426` | Se abre como modelo por ruta (`croquisDelChat` + `abrirModelo`); `abrirCroquis` (con fotos) sin usar |
| Aviso «DWG de Civil 3D sin gráficos» (`PROXYGRAPHICS = 0`) | `crates/pixpin-cad` | — | ❌ | `Planos/VisorPlanoIPad.swift` | |
| Abrir varios planos a la vez (una ventana por fichero) | `plano_cad.rs` | `Planos/VentanasDePlano.swift` | ❌ | `Raiz/` (Stage Manager, ver §9) | |
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
| ✅ Direcciones recordadas (ver y probar) | `sincronizar.rs` | `Sincro/VistaSincro.swift` | 🟡 | `Sincro/VistaSincroIPad.swift` | Se ven con su nombre y «Probar». **Olvidar** una necesita el núcleo: `Nucleo::olvidar_direccion(id) -> bool` (quitar su línea de `sincro/direcciones.txt`) |
| ✅ Estado del reloj de sincronizar (con quién, fallos seguidos) | `sincronizar/automatica.rs` | — | ✅ | `Sincro/VistaSincroIPad.swift` | «Sincronizando sola con …» (grupo y personas) y los fallos seguidos |
| ✅ Enviar fotos de **Fotos** | — | — | ✅ | `Sincro/EnviarA.swift` | Todo «Enviar…» pregunta «Fotos y vídeos» o «Archivos» (`PhotosPicker`, HEIC→JPEG con `.compatible`) |
| Grupo, código, QR, unirse, «¿Qué sincronizar?», Lo mío manda, copias, papelera, enviar/recibir por código, Enviar a…, compartidos | `sincronizar/` | `Sincro/` | ✅ | — | Revisado contra el Mac fichero a fichero: igual salvo lo de iOS. `recibirEnvio` sin usar porque se usa `recibirEnvioPreguntando` (bien) |

### 5.2 Ajustes

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| ✅ **Buscador** de ajustes y «Restablecer sección» | `ventana_ajustes.rs` | `Ajustes/VistaAjustes.swift` | ✅ | `Ajustes/FilasAjustes.swift`, `Ajustes/VistaAjustesIPad.swift` | Filas como datos (como el Mac): `.searchable` sin tildes en todas las secciones, «Restablecer <sección>» en un paso de «Deshacer», el punto azul y «Volver al valor de fábrica» al mantener pulsado. Con pruebas |
| ✅ Sección **Voz** | `ventana_ajustes.rs` (`Seccion::Voz`) | `Ajustes/FilasAjustes.swift` | ✅ | `Ajustes/FilasAjustes.swift` | Voces de Microsoft (sí/no) y la voz del iPad de cada idioma con «Escuchar». El segundo idioma de las notas de voz no sale: el dictado del iPad no lo usa. La velocidad no se guarda en ningún sitio (la lleva el lector, zona Voz) |
| ✅ Sección **Asistente** | `ventana_ajustes.rs` (`Seccion::Asistente`) | `Ajustes/FilasAjustes.swift` | ✅ | `Ajustes/FilasAjustes.swift` | Motor, claves de la nube (la hoja del asistente), pensar, respaldo, documentos a la nube, ver lo dicho y las conexiones |
| 🟡 «Conectar…» de las conexiones (Google) | `asistente/conexiones.rs` | `Ajustes/FilaConexion.swift` | 🟡 | `Ajustes/FilaConexion.swift` | Hecha la hoja `ASWebAuthenticationSession` (`SesionWebConexion`, `AbridorDeConexion`). **Falta el núcleo**: `ia_conectar` abre con `open` (no existe en iOS). Hace falta `ia_conectar_con(servicio_id, abrir: Box<dyn AbrirNavegador>)` (ver abajo); mientras, sale un aviso claro |
| ❌ Sección **Tareas** (palabras de urgencia) | `ventana_ajustes.rs` (`Seccion::Tareas`) | — | ❌ | `Ajustes/FilasAjustes.swift` | **Falta el núcleo**: `AjustesMac` no trae `[tareas] urgente`/`muy_urgente` (y `tareas.rs` del núcleo usa las de fábrica). Ver abajo |
| ❌ Sección **Lienzo** (imán, suavizado, qué herramientas y dónde) | `ventana_ajustes.rs` (`Seccion::Lienzo`) | — | ❌ | `Ajustes/FilasAjustes.swift` + zona Lienzo | **Falta el núcleo** (`[enganche]`, `[tinta] suavizado`, `[herramientas]`) y que la barra del lienzo del iPad lo lea |
| ❌ Sección **Pinear** (palabras mágicas) | `ventana_ajustes.rs` (`Seccion::Pinear`) | — | ❌ | `Ajustes/FilasAjustes.swift` + zona Pines | **Falta el núcleo** (`[pinear]`, `pixpin_pin::magia`) y que el iPad pinee por palabra mágica |
| Atajos (teclas) | `ventana_ajustes.rs` (`Seccion::Atajos`) | `Ajustes/Atajos.swift`, `GestorAtajos.swift` | ⛔ | — | Sin atajos globales en iOS; los de dentro de la app ya van con ⌘ |
| Captura, Apps predeterminadas | `ventana_ajustes.rs` | `Ajustes/FilasAjustes.swift` | ⛔ | — | Los días de la galería sí, en General |
| Tema Cosmos (el del móvil) | `tema_cosmos.rs`, `cielo.rs` | — | ❌ | `Ajustes/ModeloAjustes.swift` (`Tema`) | Poco usado |

**Lo que pide al núcleo (`apple/nucleo`, llevarlo también al Mac):**

1. Conectar desde iOS, en `pixpin-nucleo/src/asistente/conexiones.rs`:
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
2. Olvidar una dirección: `Nucleo::olvidar_direccion(&self, id: String) -> Result<bool, ErrorNucleo>` (reescribe `sincro/direcciones.txt` sin esa línea).
3. Tareas, Lienzo y Pinear: un registro aparte para no tocar `AjustesMac` del Mac, p. ej. `AjustesOrganizar { tareas_urgente: Vec<String>, tareas_muy_urgente: Vec<String>, iman_activo, iman_esquinas, iman_medios, iman_centros: bool, iman_radio_px: u32, suavizado: String, herramientas_apagadas: Vec<String>, palabras_magicas: Option<String> }` con `ajustes_organizar()`, `ajustes_organizar_de_fabrica()` y `guardar_ajustes_organizar(nuevos)` (`guardar_conservando`, como `guardar_ajustes`); y que `tareas.rs` del núcleo use `a.tareas` al leer la urgencia de una frase.

---

## 6. Galería, Timeline, Tareas, Buscar, Lecciones y Recordatorios

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| **Horario de clases semanal** (rejilla, arrastrar asignaturas, alargar en saltos de 15 min, colores) | `horario.rs`, `horario/` | `Recordatorios/HorarioClases.swift`, `VentanaHorario.swift` | ❌ | `Recordatorios/` (nuevo `HorarioIPad.swift`) + una `Seccion` o hoja desde Tareas | Solo se leen sus asignaturas y suenan sus avisos. Sin usar: `SemanaHorario.bloques`, `asignaturas`, `anadirAsignatura`, `borrarAsignatura`, `alargar`, `cambiarColor`, `ficheroDelHorario` |
| **La llamada antes de cada clase** con la nota de voz («Contestar») y «Nota de voz para la próxima clase» | `horario.rs`, `llamada.rs` | `Recordatorios/HorarioClases.swift` | ❌ | `Recordatorios/Vigia.swift` | `notaDeVozDeClase` sin usar; hoy es un aviso normal |
| Google Calendar en el horario y en los avisos | `calendario_google.rs` | `Recordatorios/` | 🟡 | `Recordatorios/Vigia.swift` + gancho §0 | La conexión se hace (`Ajustes/FilaConexion.swift`) pero nadie lee los eventos. Alternativa iOS extra: EventKit (calendario del iPad) |
| **Pin de una tarea** con hora y «¿Por qué no ahora?» | `pines.rs`, `tareas/` | `Tareas/PinTarea.swift` | ❌ | `Tareas/VistaTareas.swift` + `Pines/MiniApps.swift` | Sin usar: `tareaPorTitulo`, `ponerNotaTarea`, `tareaConNota`. Dentro de la capa de pines sí cabe |
| Mientras se escribe una tarea, ver cómo quedará («Urgente · #casa · ⏰ …») | `tareas/` | — | ❌ | `Tareas/VistaTareas.swift` (caja de apuntar) | `tareaEntendida` sin usar |
| Mover una tarea de lista por pedido (`mover_tarea`) | `pedidos.rs` | `Pedidos/HacerPedidos.swift` | ❌ | `Pedidos/HacerPedidos.swift` | Desde la vista sí («Mover a…») |
| Timeline: **historia** de un momento (foto de fondo, texto en medio, degradado, barritas, nota de voz) | `timeline/` | `Timeline/TarjetasTimeline.swift` | 🟡 | `Timeline/VistaTimeline.swift` | Hay detalle (fotos enteras y audio), no el aspecto de historia |
| Timeline: **compartir como imagen** (tarjeta 1080×1350) | `timeline/` | `Timeline/TarjetasTimeline.swift` | ❌ | `Timeline/VistaTimeline.swift` | Hoy se comparte el texto o las fotos sueltas |
| Timeline: exportar a **PDF** | `timeline/` | `Timeline/Timeline.swift` | 🟡 | `Timeline/VistaTimeline.swift:159` | Solo HTML |
| Galería: **Pinear** desde el menú | `galeria_capturas.rs` | `Galeria/VentanaGaleria.swift` | ❌ | `Galeria/VistaGaleria.swift` (menú de la captura) | Hay abrir, compartir, copiar, a un proyecto, anotar, conservar, 7 días más, borrar |
| Galería: mostrar en la carpeta | `galeria_capturas.rs` | `Galeria/VentanaGaleria.swift` | ⛔ | — | Alternativa: «Guardar en Archivos» (hoja de compartir) |
| Lecciones: la ficha de una lección nueva desde fuera (`leccion_nueva`, con texto y fotos) | `pedidos.rs`, `lecciones.rs` | `Pedidos/HacerPedidos.swift` | 🟡 | `Pedidos/HacerPedidos.swift:244` | Avisa «aún no se abre desde fuera» |
| Galería con caducidad, OCR, conservar; Buscar con pestañas, recientes y letras rápidas; Tareas por fecha/lista/Kanban con urgencia, etiquetas e imágenes; Lecciones con gravedad, «Me volvió a pasar» y repaso; recordatorios con notificaciones y botones | — | — | ✅ | — | |

---

## 7. Asistente, Automatizaciones y Pedidos

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| **Modelo local** (bajar el GGUF, `llama-server`) | `asistente/`, `crates/pixpin-ia` | `Asistente/DescargaModelos.swift` | ⛔ / ✅ | — | Sin `llama-server` en iOS. Alternativa hecha: Apple Intelligence (`Asistente/MotorApple.swift`, iPadOS 26). Sin usar (correcto en iOS): `iaCatalogoLocal`, `iaRutaDescarga`, `iaVerificarDescarga`, `iaPrecargarLocal`, `iaSoltarMotores`, `iaTerminarMotor`, `iaCarpetaMotor`, `iaLimpiarHuerfanos`, `llamaLocalActivo`, `llamaLocalPreguntar*` |
| Herramienta «exportar» del asistente | `asistente/documentos_acciones.rs` | `Asistente/ManosDocumentos.swift` | ❌ | ver §0 (`exportador`) | |
| Receta nueva desde cero («+» en la fila de recetas) | `asistente/recetas.rs` | — | 🟡 | `Asistente/AsistenteEnElChat.swift` | Se hace receta desde un mensaje, se cambia, mueve y borra; `iaRecetaNueva` sin usar |
| Borrar chat del asistente (vaciarlo) | `ventana_chat/asistente.rs` | `Asistente/HistorialAsistente.swift` | 🟡 | `Asistente/CabeceraDelAsistente.swift` | «Conversación nueva» sí |
| **Pedidos que el iPad no atiende**: `al_lienzo`, `mover_tarea`, `posponer`, `borrar_captura`, `capturar` (podría abrir `CapturadorIPad.empezar()`), `pinear`/`pinear_ultima`/`pines` (podrían ir a la capa de pines), tareas con imágenes | `pedidos.rs`, `docs/protocolo-pedidos.md` | `Pedidos/HacerPedidos.swift` | 🟡 | `Pedidos/HacerPedidos.swift:213-258, 458` | Hoy avisan «en el iPad no…» aunque la capa de pines y la captura del iPad ya existen |
| Servidor MCP (`pixpin-mcp`) | `apps/pixpin-mcp` | `docs/mcp.md` | ⛔ | — | Sin procesos de fondo en iOS. Alternativa: App Intents (ya) |
| Extensión del navegador | `extensiones/` | — | ❌ | nuevo target: extensión web de Safari | Posible en iPadOS (Safari Web Extension) |
| Flow Launcher | `crates/pixpin-lanzador` | — | ⛔ / 🟡 | `Pedidos/IntencionesIPad.swift` | Alternativa: Spotlight + Atajos (App Intents); falta declarar el paquete (§0) |
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
| Anotar el pin con las herramientas del lienzo | `crates/pixpin-pin` (`Anotar`) | `Pines/Anotar.swift` | 🟡 | `Pines/VistaPin.swift` | Anota con el editor de captura (5 herramientas), no con el lienzo |
| Pin de audio (reproductor flotante) | `reproductor.rs` | `Pines/Pin.swift` | 🟡 | `Pines/VideoPin.swift` | Barra propia hecha (por dónde va, ±15 s, velocidad); falta la cola |
| Dejar pasar el clic (`Ctrl T`) | `crates/pixpin-pin` | `Pines/Pin.swift` | ⛔ | — | En una capa táctil dentro de la app no tiene sentido; la opacidad sí está |
| Devolver el último pin cerrado (`pines devolver_ultimo`) | `pines.rs` | `Pines/GestorPines.swift` | ✅ | `Pines/GestorPinesIPad.swift` (`devolverUltimoCerrado()`), `Pines/PanelAbiertos.swift` | En el panel (⇧⌘T), en «Nuevo pin» y en el botón de pines |
| Foto, nota, mini-apps (tareas, contador, ruleta, gastos, relojes), filtros, giro, opacidad, grupos, panel «Pines abiertos», se guardan y vuelven | — | — | ✅ | — | |
| «Pinear» desde el menú del mensaje (ganchos `GanchosChat.pinearFichero` y `pinearTareas`) | — | — | ✅ | `Pines/CapaPines.swift` (`Arranque.apuntar`) | |
| «Fijar como pin» de los visores | — | — | ✅ (API) | `Pines/GestorPinesIPad.swift` | `pinear(url:)`, `pinear(pdf:pagina:)`, `pinear(documentoPdf:pagina:)`, `pinearSoloPagina(pdf:pagina:)`, `pinear(imagen:)`, `pinearLoQueSeVe()`; falta que cada visor ponga su `alFijar` |

### 8.2 Captura

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| Capturar una zona, ventana o pantalla de otras apps | `captura2/`, `crates/pixpin-capture` | `Captura/Capturador.swift` | ⛔ | — | Alternativa hecha: cámara, escáner (VisionKit), Fotos, pegar y «Capturar PixPin» (`Captura/CapturadorIPad.swift`). Falta la **extensión de Compartir** (§9), que es por donde llega una captura de pantalla del sistema |
| **Recortar a una zona** después (medidas exactas, 16:9 · 4:3 · 1:1, repetir la última zona `R`) | `captura2/` | `Captura/MedidasCaptura.swift`, `GeometriaCaptura.swift` | ✅ | `Captura/EditorCapturaIPad.swift`, `Captura/RecorteCaptura.swift` | ⌘6; mover la zona arrastrando desde dentro, An × Al, volver a la foto entera; lo anotado se recorta con la foto |
| Lupa con el color en HEX/RGB (`C` lo copia) | `captura2/` | `Captura/VistaCaptura.swift` | ✅ | `Captura/LupaCaptura.swift`, `Captura/EditorCapturaIPad.swift` | Cuentagotas (⌘7) o el Pencil por encima al recortar; también sale al elegir la zona; tocar el texto cambia HEX / RGB |
| Pila de capturas de la esquina | `pila_capturas.rs`, `crates/pixpin-pila` | `Captura/PilaCapturas.swift` | ✅ | `Captura/PilaCapturasIPad.swift` + `Pines/CapaPines.swift` | «Copiar» apila (10 s, o armada con un toque; tope 24); se pegan todas juntas; mantener: copiar todas, pinear, anotar, quitar |
| Captura con retardo (cuenta atrás) | `cuenta_atras.rs` | `Captura/CuentaAtras.swift` | ✅ | `Captura/CapturadorIPad.swift` | «Capturar PixPin en 3 s» (el número sale en la capa de pines; otra vez lo cancela) |
| Captura con scroll (larga) | `scroll.rs` | `Captura/CapturaConScroll.swift`, `CosidoScroll.swift` | ⛔ | — | De la propia app: chat o timeline entero como imagen larga |
| Grabar GIF / vídeo de la zona | `gif.rs`, `grabador.rs`, `crates/pixpin-record` | `Captura/GrabadorGif.swift`, `Grabar/` | ⛔ / 🟡 | `Captura/GrabadorIPad.swift` | Otras apps: la grabación de pantalla del Centro de control. De la propia app: «Grabar PixPin» con ReplayKit, a la galería como `.mp4` (botón rojo para parar). Falta el GIF y elegir zona |
| Pin en vivo (copia una zona en tiempo real) | `pin_vivo.rs` | `Pines/PinEnVivo.swift` | ⛔ | — | Sin alternativa razonable |
| Anotador de pantalla (anotar encima de todo y guardarlo) | `anotador_al_chat.rs`, `overlay.rs` | `Captura/AnotadorPantalla.swift` | ⛔ / ❌ | `Captura/` | Alternativa: anotar encima de PixPin y guardar en «Mensajes guardados»; `pantallaAnotadaAlChat`, `ponerAlDiaPantallaAnotada` y `EditorLienzo.tintaDePantalla` sin usar |
| Gestos con Alt | `captura2/` | `Captura/GestosAlt.swift` | ⛔ | — | Alternativa: apretar el Apple Pencil Pro / atajos ⌘ |
| Anotar (lápiz, flecha, rectángulo, texto, mosaico), Copiar, Pinear, Al chat (selector con buscador), Texto (OCR), Guardar en la galería o en Fotos | — | — | ✅ | — | |

---

## 9. Lo común de la app y del sistema

| Función (Windows) | Windows | Mac | Estado iPad | Debería ir en | Nota |
|---|---|---|---|---|---|
| **Extensión de Compartir** (recibir desde Fotos, Safari, la captura del sistema, otra app) | `recibir_de_windows.rs` («Imprimir a PixPin»), `vista_rapida.rs` | `Pedidos/ServiciosFinder.swift` | ❌ | nuevo target en `xcode/project.yml` | El plan la da como alternativa a capturar; no existe el target |
| **Abrir un `.pixpin`** (chat o fichero con todo dentro) y abrir desde Archivos | `compartir.rs`, `recibir.rs` | `Compartir/` | ❌ | `xcode/Soporte/App-Info.plist` (`UTExportedTypeDeclarations`, `CFBundleDocumentTypes`) + `Raiz/BarraVisorIPad.swift` (`AbrirDesdeFuera`) | Faltan los tipos `.pixpin`, `.md`, `.html`, `.rvt`, `.xml`, `.excalidraw` |
| Varias ventanas (abrir un visor en otra ventana con Stage Manager) | `grupos_ventanas.rs` | `Planos/VentanasDePlano.swift` | 🟡 | `Raiz/Navegacion.swift` (`openWindow`/`requestSceneSessionActivation`) | `UIApplicationSupportsMultipleScenes = true`, pero nada abre una ventana nueva |
| Bandeja / centro de control (buscador de acciones, favoritos, últimas capturas) | `centro_bandeja/` | `Bandeja/` | ⛔ / ❌ | nuevo target de widgets | Alternativa: widgets y controles del Centro de control (iOS 18) y acciones rápidas del icono (`UIApplicationShortcutItem`, la Jump List de `lista_del_icono.rs`) |
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
