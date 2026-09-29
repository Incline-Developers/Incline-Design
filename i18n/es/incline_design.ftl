# Español. Las entradas ausentes usan el catálogo inglés como respaldo.
common-cancel = Cancelar
common-clear = Borrar
common-close = Cerrar
common-fill = Relleno
common-set = Establecer
status-language = Idioma
menu-file = Archivo
menu-file-save-project = Guardar proyecto
menu-file-save-project-as = Guardar proyecto como...
menu-file-new-project = Proyecto nuevo...
menu-file-open-project = Abrir proyecto...
menu-file-open-recent = Abrir reciente
menu-file-import = Importar...
menu-file-export = Exportar...
menu-file-about = Acerca de { $app }...
menu-file-exit = Salir de la aplicación
menu-view = Vista
ws-production = Producción
ws-drill-and-blast = Perforación y voladura
ws-geology = Geología
ws-planning = Planificación
dialog-rename-title = Cambiar nombre de { $kind }
dialog-rename-field = Nombre nuevo
dialog-rename-field-hint = Obligatorio
dialog-rename-submit = Cambiar nombre
dialog-delete-title = Eliminar { $kind }
dialog-delete-confirm = ¿Eliminar «{ $name }» del proyecto?
    Esta acción no se puede deshacer.
confirm-delete-product = ¿Eliminar el producto «{ $name }» de la paleta?
    Esta acción no se puede deshacer.
about-read-full-licence = Leer la licencia completa ↗
about-source-code = Código fuente
about-website = Sitio web

## Completed canonical messages

menu-file-show-in-explorer = Mostrar en el Explorador
menu-file-show-in-folder = Abrir carpeta contenedora
menu-file-export-viewport-image = Exportar imagen del área de visualización...
menu-file-export-engineering-drawing = Exportar dibujo de ingeniería...
ws-menubar-design = Diseño
ws-menubar-triangulation = Triangulación
ws-menubar-raster = Ráster
ws-menubar-point-cloud = Nube de puntos
ws-menubar-block-model = Modelo de bloques
ws-menubar-drillholes = Sondajes
ws-menubar-active-layer = Capa:
ws-menubar-design-insert-point = Punto de inserción
ws-menubar-design-insert-point-at-intersection = En la intersección
ws-menubar-design-insert-point-at-elevation = En la elevación
ws-menubar-design-move-to = Mover a
ws-menubar-design-create-triangulation = Crear triangulación
tri-create-title = Crear triangulación
tri-create-type-label = Tipo de triangulación
tri-create-type-help = La superficie abierta crea una hoja de estilo de terreno. El sólido crea una malla completamente cerrada y requiere una entrada que puede formar un límite impermeable.
tri-create-output-name = Nombre de salida
tri-create-output-name-help = Nombre asignado a la triangulación generada.
tri-create-output-name-hint = nombre de la triangulación
tri-create-run = Triangular
tri-selection-selected = { $summary } seleccionados
tri-type-open-surface = Superficie
tri-type-solid-closed = Sólido
about-title = Acerca de { $app }
drill-hole-colour-stop = Parada { $index }
properties-restore-defaults-tooltip = Restablezca la configuración de { $heading } a sus valores predeterminados
ui-selected-count = { $count } seleccionados
ui-selected-objects = { $count } objeto(s) seleccionado(s)
ui-selected-polylines = { $count } polilínea(s) seleccionada(s)
ui-invalid-axis-value = Ingrese un valor { $axis } válido.
ui-selection-spans = La selección abarca desde { $min } hasta { $max }.
confirm-delete-count = ¿Estás seguro de que quieres eliminar { $count } elemento(s) seleccionado(s)?
confirm-delete-layer = ¿Eliminar la capa «{ $name }» y todos los objetos en ella? Esto no se puede deshacer.
plot-preview-pixels = { $width } × { $height } px en { $dpi } dpi
tri-estimated-memory = Estimación de memoria máxima ~{ $estimate }. { $detail }
block-grid-summary = Grilla: { $x } × { $y } × { $z } = { $count } bloques
status-selected = Seleccionado: { $count }
status-clip = Clip cerca/lejos/Δ: { $near } / { $far } / { $delta } m

## Selection counts

tri-count-polylines =
    { $count ->
        [one] { $count } polilínea
       *[other] { $count } polilíneas
    }
tri-count-strings =
    { $count ->
        [one] { $count } línea
       *[other] { $count } líneas
    }
tri-count-points =
    { $count ->
        [one] { $count } punto
       *[other] { $count } puntos
    }
tri-count-texts =
    { $count ->
        [one] { $count } objeto de texto
       *[other] { $count } objetos de texto
    }
tri-count-objects =
    { $count ->
        [one] { $count } objeto
       *[other] { $count } objetos
    }

## Reused existing project translations

explorer-no-rasters = No hay rásteres
slice-viewport-gestures = arrastrar con botón central: desplazar · arrastrar con botón derecho: orbitar · Mayús+rueda: caminar · W/S: mover losa · Q/E: rotar · Esc: salir

## Detalles del entorno de inicio

## Diagnóstico de inicio del renderizador

color-aci = ACI
color-aci-value = ACI { $index }
color-index = Índice
color-rgb = RGB
color-opacity = Opacidad
color-edit = Haga clic para editar el color
color-saturation-value = Saturación y brillo
color-hue = Tono
asset-loading = Cargando datos del activo
asset-unloading = Descargando datos del activo
asset-load-failed = No se pudieron cargar los datos del activo
asset-unload-failed = No se pudieron descargar los datos del activo
preferences-title = Preferencias
context-text-colour = Color de texto
context-polylines = Polilíneas
context-points = Puntos
crs-unknown-ellipsoid = Modelo de tierra no reconocido «{ $name }» en esta definición de sistema de coordenadas.
crs-no-ellipsoid = Esta definición de sistema de coordenadas no indica qué modelo de tierra utiliza.
crs-unknown-code = EPSG:{ $code } no está en el registro de sistemas de coordenadas.
crs-transform-failed = No se pudo convertir una coordenada; el resultado no fue una posición finita.
crs-no-datum-path = No hay ninguna transformación publicada disponible entre los marcos de referencia de { $from } y { $to } (dátums EPSG { $source } y { $target }). Convertir de todos modos sería incorrecto en una cantidad desconocida, por lo que no se cambió nada.
crs-unknown-datum = No se puede identificar el marco de referencia de { $from } o { $to }, y ambos usan modelos de tierra diferentes. Convertir entre ellos sería incorrecto en una cantidad desconocida.
ws-survey = Topografía
survey-count-designs = { $count } { $count ->
    [one] diseño
   *[other] diseños
  }
survey-count-meshes = { $count } { $count ->
    [one] triangulación
   *[other] triangulaciones
  }
survey-count-models = { $count } { $count ->
    [one] modelo de bloques
   *[other] modelos de bloques
  }
survey-count-clouds = { $count } { $count ->
    [one] nube de puntos
   *[other] nubes de puntos
  }
survey-count-holes = { $count } { $count ->
    [one] conjunto de sondajes
   *[other] conjuntos de sondajes
  }
survey-count-rasters = { $count } { $count ->
    [one] ráster
   *[other] rásteres
  }
survey-angle = Rotación alrededor de Z (antihorario)
survey-scale = Factor de escala uniforme XYZ
survey-invalid-transform = Los orígenes, el ángulo y las coordenadas resultantes deben ser finitos.
survey-invalid-scale = La escala debe ser un número positivo finito con recíproco finito.
survey-empty-selection = Seleccione al menos un elemento compatible para transformar.
survey-unavailable = Falta un elemento seleccionado o no está cargado. Cárguelo antes de transformar.
survey-wrong-project = Seleccione diseños solo del proyecto activo.
survey-name-required = Introduzca un nombre para el sistema de coordenadas.
survey-working = Transformando los datos seleccionados…
survey-completed = Se convirtieron { $items } en su lugar. Deshacer los restaura.
survey-failed = La transformación falló: { $error }
survey-stale = Transformación descartada porque el proyecto activo o los datos de origen cambiaron. Seleccione los datos de origen e inténtelo de nuevo.
survey-coordinates-menu = Coordenadas
survey-definitions-action = Definiciones…
survey-transform-action = Transformar…
survey-definitions-title = Definiciones de coordenadas
survey-transform-title = Transformar coordenadas
survey-new-system = Nuevo sistema de coordenadas
survey-new-system-name = Sistema de coordenadas
survey-set-local = Establecer como sistema de coordenadas de la mina
survey-delete-system = Eliminar sistema de coordenadas
survey-systems-empty = No hay sistemas de coordenadas
survey-system-name = Nombre
survey-system-origin = Mismo punto — coordenadas del sistema
survey-angle-help = Antihorario desde el X de referencia hacia el Y de referencia, visto desde arriba.
survey-scale-help = Escala uniforme XYZ desde el marco de referencia a este sistema. Use 1 para conservar las dimensiones.
survey-close = Cerrar
survey-from = De
survey-to = A
survey-transform-button = Transformar
survey-swap = Intercambiar
survey-drape-note = Las imágenes drapeadas se eliminan de las superficies convertidas y deben volver a drapearse.
survey-needs-grid-block-model = Un modelo de bloques es una cuadrícula regular de celdas, y un cambio de proyección o marco de referencia no conserva esa regularidad. Convertirlo implicaría remuestrear cada celda en una nueva cuadrícula y perder los valores que contiene, por lo que se dejó sin cambios.
survey-needs-grid-raster = Un ráster se ubica en el mundo mediante un mapa afín, algo que un cambio de proyección o marco de referencia no puede conservar. Convertirlo implicaría remuestrear la imagen, por lo que se dejó sin cambios.
survey-conversion-exact = Exacta: solo cambio de malla, sin reproyección.
survey-conversion-accuracy = Precisión indicada { $accuracy } m.
survey-kind = Tipo
survey-axis-names = Nombres de los ejes
survey-kind-registry-short = Sistema del registro
survey-kind-grid-short = Malla sobre otro sistema
survey-registry-search = Buscar
survey-registry-hint = Nombre o código EPSG, p. ej. «mga zone 56»
survey-registry-none = Nada en el registro coincide con todas las palabras.
survey-parent = Definido con respecto a
survey-parent-origin = Punto conocido — coordenadas del sistema padre
survey-pick-registry = Busque el sistema y elíjalo de los resultados.
survey-pick-parent = Elija el sistema respecto al cual se define esta malla.
survey-pick-system = Elija un sistema
survey-pick-systems = Elija el sistema de origen y el de destino de la conversión.
survey-no-selection = Elija un sistema de coordenadas a la izquierda, o haga clic derecho para añadir uno.
survey-kind-grid = Malla sobre { $parent }
survey-system-in-use = «{ $name }» no se puede eliminar: { $dependants } { $dependants ->
    [one] está
   *[other] están
  } definidos respecto a él. Rediríjalos primero a otro sistema.
survey-system-cycle = «{ $name }» está definido respecto a sí mismo, directamente o a través de sus sistemas padre.
survey-system-missing = Ese sistema de coordenadas ya no existe. Seleccione otra definición.
survey-same-system = Elija sistemas de origen y destino diferentes.
survey-name-exists = Ya existe un sistema de coordenadas con ese nombre. Selecciónelo para editarlo, o elija otro nombre.

## About strings

about-copyright-c-2026-leo-timmins =
    Copyright (c) 2026 Leo Timmins, Lucas Timmins, and Incline Design contributors. Permission is hereby granted, free of charge, to any person obtaining a copy of this software to deal in it without restriction, subject to the conditions of the MIT License.

    Incline Design is provided "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, including but not limited to the warranties of MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE and NONINFRINGEMENT.
about-free-open-source-mine-design = Diseño minero libre y de código abierto
about-licensed-under-mit-license = Con licencia MIT

## App strings

app-activated-browser-project-name = Se activó el proyecto del navegador «{ $name }».
app-browser-project-delete-failed = No se pudo eliminar el proyecto del navegador: { $error }
app-browser-project-no-longer-exists = Ese proyecto del navegador ya no existe
app-browser-save-failed-error = Error al guardar en el navegador: { $error }
app-could-not-activate-browser-project = No se pudo activar el proyecto del navegador: { $error }
app-could-not-delete-browser-project = No se pudo eliminar el proyecto del navegador: { $error }
app-could-not-load-browser-project = No se pudo cargar el proyecto del navegador: { $error }
app-could-not-restore-browser-project = No se pudo restaurar el proyecto del navegador: { $error }
app-deleted-browser-project = Se eliminó el proyecto del navegador
app-failed-create-window-error = No se pudo crear la ventana: { $error }
app-failed-create-window-icon-error = No se pudo crear el icono de la ventana: { $error }
app-failed-detach-top-down-preview = No se pudo desacoplar la vista superior: { $error }
app-failed-initialize-graphics-error = No se pudieron inicializar los gráficos: { $error }
app-browser-preferences-load-failed = No se pudieron cargar las preferencias del navegador: { $error }
app-failed-load-config-file-error = No se pudo cargar el archivo de configuración: { $error }
app-failed-load-session-file-error = No se pudo cargar el archivo de sesión: { $error }
app-failed-rasterize-window-icon-error = No se pudo rasterizar el icono de la ventana: { $error }
app-failed-save-browser-session-error = No se pudo guardar la sesión del navegador: { $error }
app-failed-save-session-error = No se pudo guardar la sesión: { $error }
app-saved-name-browser-storage = Se guardó «{ $name }» en el almacenamiento del navegador

## Block strings

block-model-between = Entre
block-model-block-grid = Grilla de bloques
block-model-block-size = Tamaño de bloque
block-model-choose-numeric-variable = Elegir una variable numérica
block-model-choose-numeric-variables = Elegir variables numéricas
block-model-count-variables-selected = { $count } variables seleccionadas
block-model-estimate-variables = Estimar variables
block-model-full-x-y-z-dimensions = Dimensiones X, Y y Z completas de cada bloque. Los bloques más pequeños aumentan el detalle, el tiempo de cálculo y el uso de memoria.
block-model-grid-bounds-block-sizes-invalid = Los límites de la cuadrícula o los tamaños de los bloques son inválidos.
block-model-lower-x-y-z-edges = Límites inferiores X, Y y Z del volumen del modelo de bloques. Los centros de los bloques comienzan medio bloque dentro de estos límites.
block-model-maximum = Máximo
block-model-maximum-nearest-samples-used-each = Máximo de muestras más cercanas usado para cada bloque. Los valores bajos son más rápidos; los altos pueden suavizar las estimaciones y aumentar el cálculo.
block-model-maximum-samples = Muestras máximas
block-model-minimum = Mínimo
block-model-min-samples-help = Mínimo de muestras cercanas necesario para estimar un bloque. Los bloques con menos muestras dentro del radio de búsqueda quedan vacíos.
block-model-minimum-samples = Muestras mínimas
block-model-nugget = Efecto pepita
block-model-numeric-interval-fields-interpolate = Campos numéricos de intervalo que interpolar. Cada campo seleccionado se convierte en una variable del modelo de bloques.
block-model-kriging-help = El kriging ordinario estima intervalos numéricos de sondaje en el centro de cada bloque mediante un variograma esférico.
block-model-partial-sill = Meseta parcial
block-model-range-search-radius = Rango / radio de búsqueda
block-model-range-help = Se excluyen las muestras más largas que esta distancia; la covarianza alcanza el cero en este rango.
block-model-select-all = Seleccione todos
block-model-sill-help = Varianza espacialmente correlacionada aportada por el modelo esférico. Junto con el efecto pepita, establece la covarianza a distancia cero.
block-model-spherical-variogram-search = Variograma esférico y búsqueda
block-model-threshold-at-most = <= umbral
block-model-threshold-at-least = >= umbral
block-model-threshold-min = Umbral / mín
block-model-upper-x-y-z-extent = Extensión superior X, Y y Z que cubrir. El último bloque puede superar esta extensión cuando el intervalo no sea un múltiplo exacto del tamaño de bloque.
block-model-variable = Variable
block-model-variance-effectively-zero-separation = Varianza a separación prácticamente nula causada por errores de medición o variaciones bajo la escala de muestreo. Use cero si no desea efecto pepita.
block-model-volume-feedback-disconnected = Se desconectó la lectura de respuesta de uso del volumen de bloques
block-model-volume-feedback-failed = Error al leer la respuesta de uso del volumen de bloques: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z

## Canvas strings

canvas-not-selectable-closed-polyline = No seleccionable | Elija una polilínea cerrada
canvas-polyline-summary = Polilínea | Capa: { $layer } | { $count } vértices
canvas-surface-name = Superficie | { $name }
canvas-trimmed = Ajustada

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = Se creó un talud y berma desde el objeto { $object_id }
cmd-bezier-replaced-polyline-span-first-last = Se sustituyó el tramo de polilínea { $first }→{ $last } por { $count } puntos intermedios muestreados
cmd-bezier-vertices-first-last = Vértices { $first } a { $last }
cmd-block-model-block-model-loader-disconnected-path = El cargador de modelos de bloques se desconectó para { $path }
cmd-block-model-block-model-path-has-count = El modelo de bloques { $path } contiene { $count } variable(s) de un tipo no compatible que no se podrán leer: { $names }
cmd-block-model-building-ore-mesh = Construyendo la malla de mineral…
cmd-block-model-could-not-create-block-model = No se pudo crear el modelo de bloques: { $error }
cmd-block-model-could-not-decode-block-model = No se pudo decodificar la variable de color del modelo de bloques «{ $variable }»: { $error }
cmd-block-model-created-block-model-name-ordinary = Se creó el modelo de bloques «{ $name }» mediante kriging ordinario
cmd-block-model-failed-load-block-model-error = No se pudo cargar el modelo de bloques: { $error }
cmd-block-model-generated-ore-mesh-from-block = Se generó una malla de mineral a partir del modelo de bloques «{ $name }»
cmd-block-model-imported-block-model-source-path = Origen del modelo de bloques importado { $path }
cmd-block-model-loaded-block-model-name-blocks = Se cargó el modelo de bloques «{ $name }»: { $blocks } bloques ({ $renderable } renderizables), cuadrícula { $dimx }×{ $dimy }×{ $dimz }, { $variables } variables
cmd-block-model-loading-name = Cargando { $name }
cmd-block-model-loading-name-ellipsis = Cargando { $name }…
cmd-chamfer-applied = Se biseló la esquina { $corner } con radio { $radius } y { $segments } segmentos
cmd-chamfer-radius = Radio { $radius }
cmd-commands-clipped = Recortada
cmd-commands-command-failed-error = El comando falló: { $error }
cmd-commands-select-one-more-objects-before = Seleccione uno o más objetos antes de establecer { $axis }
cmd-commands-sliced = Seccionada
cmd-contours-contour-generation-failed-error = La generación de curvas de nivel falló: { $error }
cmd-contours-discarded-layer-exists = Se descartaron las curvas de nivel de «{ $name }»: ya existe la capa «{ $layer_name }»
cmd-contours-discarded-project-closed = Se descartaron las curvas de nivel de «{ $name }»: el proyecto se cerró
cmd-contours-discarded-layer-deleted = Se descartaron las curvas de nivel de «{ $name }»: se eliminó la capa de salida seleccionada
cmd-contours-generated = Se generaron { $line_count } polilínea(s) de curvas de nivel para la triangulación «{ $name }» en la capa «{ $layer_name }»
cmd-creation-assembled-boundary-rings = Se ensamblaron { $assembled_count } anillo(s) de límite cerrado a partir de líneas abiertas fragmentadas
cmd-creation-created-triangulation-from-boundary = Se creó una triangulación con { $boundary_count } anillo(s) de límite y { $constraint_count } restricción(es) abierta(s), tipo de superficie { $surface_type }
cmd-creation-creating-triangulation = Creando la triangulación…
cmd-creation-generate-upper-surface-ignored-count = Generar superficie superior: se ignoraron { $count } segmento(s) de línea de rotura inferior en conflicto; los objetos de origen no cambiaron
cmd-creation-ignored-objects = Se ignoraron { $rejected } objeto(s) que no eran polilíneas o estaban degenerados durante la triangulación
cmd-creation-weld-retry-moved-coarse-welded = Soldar y reintentar: se movieron { $coarse_welded } vértice(s) a posiciones compartidas (hasta { $coarse_weld_tol } m); los objetos de origen no cambiaron
cmd-creation-welded-breakline-vertices = Se soldaron { $welded } vértice(s) de línea de quiebre que coincidían dentro de la tolerancia
cmd-cuts-clipped-surface-name-polyline-mode = Se recortó la superficie «{ $name }» mediante una polilínea ({ $mode })
cmd-cuts-clipping-surface-polyline = Recortando la superficie mediante una polilínea…
cmd-cuts-cut-topology-name-pit-shell = Se cortó la topografía «{ $name }» según la envolvente del tajo
cmd-cuts-cut-triangulation-name-z-band = Se cortó la triangulación «{ $name }» por la franja Z [{ $min }, { $max }]
cmd-cuts-cutting-topology-pit-shell = Cortando la topografía según la envolvente del tajo…
cmd-cuts-cutting-triangulation-z = Cortando la triangulación por Z…
cmd-cuts-ignored-vertical-faces = Se ignoraron { $count } cara(s) verticales o degeneradas de la topografía de referencia sin área XY
cmd-cuts-site-skipped-constraint-from-x = { $site }: se omitió la restricción ({ $from_x }, { $from_y }) → ({ $to_x }, { $to_y }) que el triangulador no pudo dividir
cmd-cuts-skipped-degenerate-edges = { $site }: se omitieron { $skipped } arista(s) de restricción casi degeneradas; el límite del corte puede desviarse mínimamente cerca de ellas
cmd-cuts-trimmed-surface = Se recortó la superficie «{ $surface }» según la topografía «{ $topology }» ({ $mode })
cmd-cuts-trimming-surface-topology = Recortando la superficie según la topografía…
cmd-drape-draped-intersected-vertices-changed = Se proyectaron { $intersected } vértices; { $changed } cambiaron de elevación
cmd-drape-no-intersections = Ninguno de los vértices de diseño seleccionados intersecta las topografías seleccionadas
cmd-drape-objects-changed-object-s-changed = { $objects } objeto(s) modificado(s) · se movieron { $changed } de { $intersected } vértices con intersección
cmd-drape-select-one-more-design-objects = Seleccione uno o más objetos de diseño para proyectar
cmd-drape-select-one-more-topologies-drape = Seleccione una o más topografías sobre las que proyectar
cmd-drape-selected-topologies-no-longer-loaded = Las topografías seleccionadas ya no están cargadas
cmd-drill-hole-drill-pattern-too-large-contains = El patrón de perforación es demasiado grande o contiene coordenadas de brocal no válidas
cmd-drill-hole-enter-name-drill-pattern = Introduzca un nombre para el patrón de perforación
cmd-drill-hole-failed-load-drillholes-error = No se pudieron cargar los sondajes: { $error }
cmd-drill-hole-depth-must-be-positive = La profundidad del barreno debe ser mayor que cero
cmd-drill-hole-diameter-must-be-positive = El diámetro del barreno debe ser mayor que cero
cmd-drill-hole-loaded-drillhole-dataset-name-holes = Se cargó el conjunto de sondajes «{ $name }»: { $holes } sondajes, { $fields } campos de color
cmd-drill-hole-pattern-contains-no-holes = El patrón no contiene barrenos
cmd-explode-count-line-s = { $count } línea(s)
cmd-explode-polyline = Explotar polilínea
cmd-explode-exploded-polyline-into-count-line = Se explotó la polilínea en { $count } segmentos
cmd-file-block-model-csv-encoding-failed = La codificación del CSV del modelo de bloques falló: { $error }
cmd-file-block-model-csv-export-failed = La exportación de CSV del modelo de bloques falló: { $error }
cmd-file-browser-recovery-unavailable = Los archivos de recuperación del navegador no están disponibles; los proyectos guardados permanecen en IndexedDB
cmd-file-closed-project-runtime-id-runtime = Se cerró el proyecto con identificador de ejecución { $runtime_id }
cmd-file-could-not-create-new-project = No se pudo crear un proyecto nuevo: { $error }
cmd-file-could-not-finish-pending-project = No se pudo terminar la acción pendiente del proyecto: { $error }
cmd-file-could-not-finish-saving-before = No se pudo terminar de guardar antes de salir: { $error }
cmd-file-could-not-open-browser-project = No se pudo abrir el proyecto del navegador: { $error }
cmd-file-could-not-open-path-error = No se pudo abrir { $path }: { $error }
cmd-file-could-not-read-selected-file = No se pudo leer el archivo seleccionado: { $error }
cmd-file-could-not-reload-layer-from = No se pudo recargar la capa desde el disco: { $error }
cmd-file-could-not-reload-project-from = No se pudo recargar el proyecto desde el disco: { $error }
cmd-file-could-not-remove-browser-project = No se pudo eliminar el proyecto del navegador: { $error }
cmd-file-could-not-restore-layer-from = No se pudo restaurar la capa desde el proyecto: { $error }
cmd-file-could-not-snapshot-dirty-project = No se pudo crear una instantánea del proyecto modificado para su recuperación: { $error }
cmd-file-could-not-start-browser-export = No se pudo iniciar la exportación del navegador: { $error }
cmd-file-could-not-write-recovery-copies = No se pudieron escribir las copias de recuperación: { $error }
cmd-file-created-new-browser-project = Se creó un proyecto nuevo en el navegador
cmd-file-created-new-project = Se creó un proyecto nuevo
cmd-file-description-download-failed-error = La descarga de { $description } falló: { $error }
cmd-file-discard-cancelled-project-changed = Se canceló el descarte porque el proyecto cambió mientras se recargaba el OMF
cmd-file-discarded-changes-layer-target-name = Se descartaron cambios en la capa «{ $target_name }»
cmd-file-discarded-changes-reloaded-path = Cambios descartados: se recargó { $path }
cmd-file-downloaded-description-file-name = Se descargó { $description }: { $file_name }
cmd-file-dxf-download-encoding-failed-error = La codificación de la descarga DXF falló: { $error }
cmd-file-dxf-import-failed-error = La importación de DXF falló: { $error }
cmd-file-encoding-block-model-csv-download = Codificando la descarga CSV del modelo de bloques…
cmd-file-encoding-dxf-download = Codificando la descarga DXF…
cmd-file-encoding-triangulation-download = Codificando la descarga de triangulación…
cmd-file-exit-deferred-exports = La salida se aplazó hasta que finalicen las exportaciones en segundo plano
cmd-file-exit-requested-no-unsaved-changes = Se solicitó salir sin cambios no guardados
cmd-file-exported-block-model-csv-path = Se exportó el CSV del modelo de bloques a { $path }
cmd-file-exported-description-dxf-path = Se exportó { $description } a DXF: { $path }
cmd-file-exported-triangulation-name-path = Se exportó la triangulación «{ $name }» a { $path }
cmd-file-exporting-name = Exportando { $name }…
cmd-file-exporting-triangulation-name-path = Exportando la triangulación «{ $name }» a { $path }
cmd-file-fatal-renderer-failure-reason = Error fatal del renderizador: { $reason }
cmd-file-dialog-action-failed = La acción del diálogo de archivos falló: { $msg }
cmd-file-imported-added-object-s-from = Se importaron { $added } objeto(s) desde { $name }
cmd-file-imported-total-dxf-object-s = Se importaron { $total } objeto(s) DXF
cmd-file-layer-discard-was-cancelled-because = Se canceló el descarte de la capa porque el proyecto cambió mientras se recargaba
cmd-file-no-recovery-directory = No hay un directorio de recuperación disponible: { $error }
cmd-file-no-unsaved-project-content-nothing = No hay contenido de proyecto sin guardar; no hay nada que recuperar
cmd-file-parsing-browser-dxf-import = Analizando la importación DXF del navegador…
cmd-file-parsing-dxf-import = Analizando la importación DXF…
cmd-file-project-closes-after-save = El proyecto se cerrará tras finalizar su guardado actual
cmd-file-the-project-closes-after-save = El proyecto se cerrará tras finalizar su guardado actual
cmd-file-queued-count-triangulation-file-s = Se pusieron en cola { $count } archivo(s) de triangulación para importar
cmd-file-recovery-copies-path-reopen-them = Las copias de recuperación están en { $path }; vuelva a abrirlas después de reiniciar
cmd-file-recovery-copy-failed-error = Error en la copia de recuperación: { $error }
cmd-file-recovery-copy-failed-failure = Error al crear la copia de recuperación: { $failure }
cmd-file-recovery-copy-written-path = Copia de recuperación escrita: { $path }
cmd-file-reverting-layer = Revirtiendo la capa…
cmd-file-reverting-project = Revirtiendo el proyecto…
cmd-file-save-failed-message = Error al guardar: { $message }
cmd-file-save-worker-ended-without-result = El proceso de guardado terminó sin resultado
cmd-file-saved-project-as = Proyecto guardado como: { $path }
cmd-file-saved-project = Proyecto guardado: { $path }
cmd-file-selected-block-model-no-longer = El modelo de bloques seleccionado ya no está cargado
cmd-file-switching-project = Cambiando de proyecto…
cmd-file-triangulation-download-encoding-failed = La codificación de la descarga de triangulación falló: { $error }
cmd-file-user-chose-exit-without-saving = El usuario eligió salir sin guardar
cmd-file-user-requested-exit-project-export = El usuario solicitó salir (se requiere confirmar la exportación del proyecto o el trabajo no guardado)
cmd-file-viewport = Vista
cmd-file-wait-current-project-save-finish = Espere a que termine el guardado del proyecto actual
cmd-file-wait-current-project-switch-finish = Espere a que termine el cambio de proyecto actual
cmd-file-wait-project-operation-finish-before = Espere a que termine la operación del proyecto antes de descartar cambios
cmd-file-wait-project-revert-finish-before = Espere a que termine la reversión del proyecto antes de guardar
cmd-fuse-closed-polyline = Polilínea cerrada
cmd-fuse-count-source-line-s = { $count } línea(s) de origen
cmd-fuse-created-shape-object-id-vertices = Se creó { $shape } { $object_id } con { $vertices } vértices a partir de { $sources } línea(s) de origen
cmd-fuse-click-missed = Fusionar: el clic no alcanzó ningún objeto (no hay nada bajo el cursor)
cmd-fuse-click-not-near-endpoint = Fusionar: el clic no estuvo lo suficientemente cerca de ninguno de los extremos de la línea seleccionada
cmd-fuse-clicked-closed-polyline = Fusionar: el objeto { $object_id } es una polilínea cerrada; la fusión solo funciona con polilíneas abiertas
cmd-fuse-clicked-not-open-polyline = Fusionar: el objeto { $object_id } no es una polilínea abierta (es { $kind })
cmd-fuse-clicked-object-missing = Fusionar: el objeto seleccionado { $object_id } ya no existe
cmd-fuse-clicked-too-few-vertices = Fusionar: la polilínea { $object_id } solo tiene { $count } vértice(s); se necesitan al menos 2
cmd-fuse-endpoint-marker-missing = Fusionar: el marcador de extremo { $marker_index } ya no existe
cmd-fuse-close-needs-three-vertices = Fusionar: la línea necesita al menos 3 vértices distintos para cerrarse como polilínea (tiene { $count })
cmd-fuse-lines = Fusionar líneas
cmd-fuse-needs-two-segments = Fusionar: se necesitan al menos 2 segmentos para confirmar (hay { $count })
cmd-fuse-no-active-layer = Fusionar: no hay una capa activa donde colocar la línea fusionada
cmd-fuse-no-active-project = Fusionar: no hay un proyecto activo; no se puede confirmar
cmd-fuse-no-source-line = Fusionar: no hay una línea de origen para cerrar en una polilínea
cmd-fuse-awaiting-object-invalid = Fusionar: el objeto { $awaiting_id } ya no es una polilínea válida
cmd-fuse-object-already-in-chain = Fusionar: el objeto { $object_id } ya forma parte de la cadena de fusión; seleccione otra línea
cmd-fuse-result-too-few-vertices = Fusionar: el resultado tiene demasiado pocos vértices ({ $count }); se cancela
cmd-fuse-segment-object-invalid = Fusionar: el objeto de segmento { $object_id } ya no es una polilínea válida; se cancela la operación
cmd-fuse-source-object-invalid = Fusionar: el objeto de origen { $object_id } ya no es una polilínea abierta válida
cmd-fuse-source-object-missing = Fusionar: el objeto de origen { $object_id } ya no existe
cmd-fuse-open-polyline = Polilínea abierta
cmd-include-failed = La inclusión falló: { $message }
cmd-include-included-solid-shape-name-topology = Se incluyó el sólido «{ $shape_name }» en la topografía «{ $topology_name }» (se conservaron { $retained } caras y se omitieron { $skipped } caras de cierre)
cmd-include-including-pit-stockpile-solid = Incluyendo el sólido de tajo/acopio…
cmd-insert-point-count-operation-point-s = { $count } punto(s) de { $operation }
cmd-insert-point-elevation-must-be-finite = Insertar punto a elevación requiere una elevación finita
cmd-insert-point-insert-points = Insertar puntos
cmd-insert-point-inserted-count-operation-point-s = Se insertaron { $count } punto(s) de { $operation }
cmd-insert-point-intersection = Intersección
cmd-insert-point-no-new-operation-points-were = No se encontraron puntos nuevos para { $operation }
cmd-insert-point-select-least-two-polylines-before = Seleccione al menos dos polilíneas antes de insertar puntos de intersección
cmd-insert-point-select-one-more-polylines-before = Seleccione una o más polilíneas antes de insertar un punto a una elevación
cmd-layer-created-layer-name = Se creó la capa «{ $name }»
cmd-layer-deleted-with-objects = Se eliminó la capa { $layer_id } y todos sus objetos
cmd-layer-duplicated-layer-duplicate-name = Se duplicó la capa «{ $duplicate_name }»
cmd-layer-locked = Bloqueado
cmd-layer-name-copy = copia de { $name }
cmd-layer-selected-count-object-s-layer = Se seleccionaron { $count } objeto(s) en la capa { $layer_id }
cmd-layer-state-layer-name = Capa «{ $name }»: { $state }
cmd-layer-unlocked = Desbloqueado
cmd-move-tool-moved-collars = Desplazamiento aplicado ({ $delta }) a { $count } brocal(es) de barreno
cmd-move-tool-moved-objects = Se aplicó el desplazamiento ({ $delta }) a { $count } objeto(s)
cmd-move-tool-count-hole-s = { $count } barreno(s)
cmd-object-edit-edited-kind = { $kind } editado
cmd-object-edit-edited-kind-count-vertices = { $kind } editado ({ $count } vértices)
cmd-object-edit-no-changes-apply = No hay cambios que aplicar
cmd-object-edit-object-changed-since-editor-opened = Este objeto ha cambiado desde que se abrió el editor; vuelva a abrirlo para editar la versión actual
cmd-object-edit-target-changed = El objeto en edición ha cambiado; se descarta la edición
cmd-object-edit-object-no-longer-exists-document = Ese objeto ya no existe en el documento
cmd-object-edit-select-single-design-object-edit = Seleccione un único objeto de diseño para editar
cmd-object-edit-unassigned = Sin asignar
cmd-offset-create-offset = Crear equidistante
cmd-offset-created-offset-count-object-s = Se creó el desplazamiento de { $count } objeto(s)
cmd-offset-distance-must-be-positive = La distancia de desplazamiento debe ser mayor que cero
cmd-omf-could-not-open-project-source = No se pudo abrir el proyecto { $source_name }: { $error }
cmd-omf-create-open-project-before-merging = Cree o abra un proyecto antes de combinar datos
cmd-omf-encoding-project = Codificando el proyecto…
cmd-omf-exported-project-path = Se exportó el proyecto a { $path }
cmd-omf-imported-project = Se importó el proyecto «{ $project_name }» desde { $source_name }: { $count } conjunto(s) de datos de nivel superior
cmd-omf-importing-project = Importando el proyecto…
cmd-omf-export-failed = La exportación de OMF falló: { $error }
cmd-omf-import-failed = La importación de OMF falló: { $error }
cmd-omf-opened-project = Se abrió el proyecto «{ $project_name }» desde { $source_name }
cmd-omf-project-source-name-contains-no = El proyecto «{ $source_name }» no contiene elementos de datos compatibles
cmd-omf-source-name-applied-project-origin = { $source_name }: se aplicó el origen del proyecto { $origin } antes de fusionar
cmd-omf-crs-differs = { $source_name }: el sistema de referencia «{ $source_crs }» difiere del SRC del proyecto «{ $target_crs }»; las coordenadas se fusionaron sin reproyección
cmd-omf-source-name-units-source-units = { $source_name }: las unidades «{ $source_units }» difieren de las unidades del proyecto «{ $target_units }»; las coordenadas se fusionaron sin conversión
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = No hay datos abiertos de Incline Design para exportar
cmd-placement-2-vertices = 2 vértices
cmd-placement-count-vertices = { $count } vértices
cmd-placement-created-circle = Se creó un círculo con radio { $radius } m
cmd-placement-created-closed-polyline = Se creó una polilínea cerrada con { $count } vértices
cmd-placement-created-line-segment-2-vertices = Se creó un segmento de línea con 2 vértices
cmd-placement-created-open-polyline-count-vertices = Se creó una polilínea abierta con { $count } vértices
cmd-placement-placed-point-x-y-z = Se colocó el punto en { $x }, { $y }, { $z }
cmd-placement-radius = Radio { $radius } m
cmd-plot-composing-engineering-drawing = Componiendo el plano de ingeniería…
cmd-plot-could-not-write-engineering-drawing = No se pudo escribir el plano de ingeniería: { $error }
cmd-plot-drawing-scale-fitted-visible-data = Escala del plano ajustada a los datos visibles: 1:{ $scale }
cmd-plot = Plano
cmd-plot-saved-drawing = Plano de ingeniería guardado: { $description } ({ $width } × { $height } px a { $dpi } ppp)
cmd-point-cloud-failed-load-point-cloud-error = No se pudo cargar la nube de puntos: { $error }
cmd-point-cloud-loaded-point-cloud-name-count = Se cargó la nube de puntos { $name } ({ $count } puntos)
cmd-point-cloud-point-cloud-loader-disconnected-path = El cargador de nubes de puntos se desconectó para { $path }
cmd-point-cloud-tin-max-edge-disabled = (arista máxima desactivada)
cmd-point-cloud-tin-max-edge-max-edge = (arista máxima { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = El TIN de nube de puntos falló: { $error }
cmd-point-cloud-tin-subsampled = TIN de terreno: se submuestrearon espacialmente { $sampled } de { $total } puntos
cmd-point-cloud-tin-triangulated = TIN del terreno: se triangularon { $vertex_count } puntos XY únicos en { $face_count } caras{ $suffix }
cmd-products-added-product-delay-ms-ms = Se añadió el producto { $delay_ms } ms { $name }
cmd-products-deleted-product-delay-ms-ms = Se eliminó el producto { $delay_ms } ms { $name }
cmd-products-failed-save-products-error = No se pudieron guardar los productos: { $error }
cmd-products-product-no-longer-palette = Ese producto ya no está en la paleta
cmd-property-action-count-object-s-layer = { $action } { $count } objeto(s) a la capa { $layer }
cmd-property-batch-set-axis-value-count = Establecer en lote el valor { $axis } en { $count } objeto(s)
cmd-property-batch-set-closed-count-polyline = Establecer en lote cerrado en { $count } polilínea(s)
cmd-property-batch-set-color-count-object = Establecer en lote el color en { $count } objeto(s)
cmd-property-batch-set-fill-style-count = Establecer en lote el estilo de relleno en { $count } objeto(s)
cmd-property-batch-set-line-weight-count = Establecer en lote el peso de línea en { $count } polilínea(s)
cmd-property-copied = Copiado
cmd-property-moved = Movido
cmd-raster-draped = Se proyectó el ráster { $raster } sobre la triangulación { $triangulation } (extensiones superpuestas)
cmd-raster-failed-load-raster-name-error = No se pudo cargar el ráster { $name }: { $error }
cmd-raster-failed-load-raster-path-error = No se pudo cargar el ráster { $path }: { $error }
cmd-raster-loaded-raster-name-via-driver = Se cargó el ráster { $name } mediante { $driver } ({ $srcx }×{ $srcy }, vista previa { $prevx }×{ $prevy })
cmd-raster-no-overlapping-triangulation = Ninguna triangulación cargada se superpone con la extensión de { $name }
cmd-raster-loader-disconnected = El cargador de ráster se desconectó para { $path }
cmd-raster-undraped = Se quitó la proyección de los rásteres de { $count } triangulación(es)
cmd-relimit-click-missed = Reajustar límite: el clic no alcanzó ningún objeto (no hay nada bajo el cursor)
cmd-relimit-click-ignored = Reajustar límite: se ignoró el clic; la herramienta no está esperando seleccionar un objetivo
cmd-relimit-clicked-source-line = Reajustar límite: se seleccionó la propia línea de origen; seleccione una línea distinta
cmd-relimit-no-source-line = Reajustar límite: no se ha establecido una línea de origen; se cancela la selección
cmd-relimit-relimited-line-source-id-selected = Se relimitó la línea { $source_id } al objetivo seleccionado
cmd-relimit-resized-line-source-id-using = Se redimensionó la línea { $source_id } con el modo { $mode } y el valor { $value }
cmd-rename-item-no-longer-belongs-active = Ese elemento ya no pertenece al proyecto activo
cmd-rename-renamed-before-name = Se cambió el nombre de «{ $before }» a «{ $name }»
cmd-rename-renamed-name-taken = Se cambió «{ $before }» a «{ $name }» («{ $requested }» ya está en uso)
cmd-rotate-collar-turned-count-drillhole-collar-s = Se giraron { $count } brocal(es) de barreno { $rotation }
cmd-section-verb-count-item-s-section = { $verb } { $count } elemento(s) en { $section }
cmd-selection-delete-vertex = Eliminar vértice
cmd-selection-deleted-count-selected-object-s = Se eliminaron { $count } objeto(s) seleccionado(s)
cmd-selection-deleted-vertex = Se eliminó el vértice { $vertex } de la polilínea { $object_id }
cmd-selection-duplicate-selection = Duplicar selección
cmd-selection-duplicated-count-object-s = Se duplicaron { $count } objeto(s)
cmd-session-created-triangulation = Se creó la triangulación «{ $name }» ({ $vertex_count } vértices, { $face_count } caras) a partir del tipo de superficie { $surface_type }
cmd-session-deleted-triangulation = Se eliminó la triangulación «{ $name }» del proyecto
cmd-session-failed-load-triangulation-error = No se pudo cargar la triangulación: { $error }
cmd-session-failed-load-triangulation-message = No se pudo cargar la triangulación: { $message }
cmd-session-loaded-triangulation = Se cargó la triangulación «{ $name }» ({ $path }, { $vertex_count } vértices, { $face_count } caras)
cmd-session-set-triangulation-tri-id-color = Se estableció el color de la triangulación { $tri_id } en { $color }
cmd-session-triangulation-load-no-result = La carga de la triangulación para { $path } terminó sin resultado
cmd-session-triangulation-failed = La operación de triangulación falló: { $message }
cmd-session-unloaded-triangulation-name = Se descargó la triangulación «{ $name }»
cmd-slice-entered-slice-view-cx-cy = Se entró en la vista de sección en { $cx }, { $cy }, { $cz } a lo largo de { $dx }, { $dy } (línea de { $length } m)
cmd-slice-exited-slice-view = Se salió de la vista de corte
cmd-slice-reset-section-view-fit-extents = Restablecer la vista de sección (ajustar a la extensión)
cmd-slice-set-section-grid-enabled = Cuadrícula de sección activada = { $enabled }
cmd-split-created-2-open-polylines = Se crearon 2 polilíneas abiertas
cmd-split-line = Dividir línea
cmd-split-points-needs-interior-vertex = Dividir en puntos: seleccione un vértice interior de la línea abierta
cmd-split-points-needs-non-adjacent-vertices = Dividir en puntos: seleccione dos vértices no adyacentes de la polilínea
cmd-split-polyline-into-two = Se dividió la polilínea de origen en dos polilíneas abiertas
cmd-text-edit-finished = Se terminó de editar el texto del objeto { $object_id }
cmd-text-updated = Se actualizó el texto del objeto { $object_id }
cmd-view-centre-rotation-not-available-flying = El centro de rotación no está disponible en modo vuelo
cmd-view-fixed-centre-rotation-x-y = Centro de rotación fijado en { $x }, { $y }, { $z }
cmd-view-no-point-under-cursor-fix = No hay ningún punto bajo el cursor donde fijar el centro de rotación
cmd-view-released-centre-rotation = Centro de rotación liberado
cmd-view-reset-view-fit-extents = Restablecer vista (ajustar a extensiones)
cmd-view-set-topology-wireframes-enabled = Se establecieron las mallas alámbricas de topografía = { $enabled }
cmd-view-set-view-points-enabled = Se estableció mostrar puntos = { $enabled }
cmd-view-set-xy-grid-enabled = Cuadrícula XY activada = { $enabled }
cmd-view-zoom-extents-preserving-angle = Zoom a extensiones (conservar ángulo)

## Common strings

common-add-product = Añadir el producto
common-background = Fondo
common-block-model = Modelo de bloques
common-block-models = Modelos de bloques
common-cancelled = Cancelado
common-chamfer = Chaflán
common-choose = Elija...
common-circle = Círculo
common-click-point-fix-centre-rotation = Haga clic en un punto para fijar el centro de rotación
common-clip-surface-polyline = Recortar superficie por polilínea...
common-closed = Cerrado
common-colour = Color
common-confirm-omf-rewrite = Confirmar la reescritura de OMF
common-could-not-replace-current-project = No se pudo reemplazar el proyecto actual: { $error }
common-count-object-s = { $count } objeto(s)
common-create = Crear
common-create-batter-berm = Crear banco y berma
common-create-bezier-curve = Crear una curva de Bezier
common-create-block-model = Crear modelo de bloques
common-create-block-model-ellipsis = Crear modelo de bloques...
common-create-circle = Crear círculo
common-create-drill-pattern = Crear patrón de perforación
common-create-layer = Crear capa
common-create-line = Crear línea
common-create-ore-triangulation = Crear triangulación de mineral
common-create-ore-triangulation-ellipsis = Crear triangulación de mineral...
common-create-point = Crear punto
common-create-polyline = Crear polilínea
common-create-triangulation = Crear triangulación...
common-crosses = Cruces
common-cut = Cortar
common-cut-topology-pit-shell = Cortar topografía con envolvente del tajo...
common-delete-layer = Eliminar la capa
common-delete-product = Eliminar el producto
common-delete-selection = Eliminar la selección
common-designs = Diseños
common-discard-layer-changes = Descartar los cambios de la capa
common-down = Abajo
common-drape-topology = Proyectar a la topografía
common-easting = Este
common-edit-object = Editar objeto
common-edit-text = Editar el texto
common-elevation = Elevación
common-exit-without-saving = Salir sin guardar
common-export-engineering-drawing = Exportar dibujo de ingeniería
common-filter = Filtro
common-fly-mode = Modo de vuelo
common-generate-contour-lines = Generar curvas de nivel...
common-hide-all = Ocultar todo
common-hide-selection = Ocultar selección
common-ignore = Ignorar
common-import-csv-block-model = Importar CSV de modelo de bloques
common-import-dxf = Importar DXF
common-incline-design-project = Proyecto de Incline Design
common-layer = Capa
common-legend = Leyenda
common-line = Línea
common-line-weight = Peso de línea
common-lock-all = Bloquear todo
common-lock-selection = Bloquear selección
common-m = m
common-max = Máx.
common-merge-shell-into-topology = Fusionar cáscara en topografía
common-merge-shell-into-topology-ellipsis = Fusionar cáscara en topografía...
common-move-collar = Mover brocal
common-move-design = Mover diseño
common-move-selection = Mover selección
common-new-product = Nuevo producto
common-no-block-models = No hay modelos de bloques
common-no-design-layers = No hay capas de diseño
common-no-drill-holes = No hay sondajes
common-no-file-chosen = No se eligió ningún archivo
common-no-open-project = No hay ningún proyecto abierto
common-no-point-clouds = No hay nubes de puntos
common-no-triangulations = No hay triangulaciones
common-none = Ninguno
common-northing = Norte
common-offset = Compensación
common-open = Abrir
common-orientation = Orientación
common-point = Punto
common-point-cloud = Nube de puntos
common-point-clouds = Nubes de puntos
common-polyline = Polilínea
common-polyline-layer = Polilínea en «{ $layer }»
common-project = Proyecto
common-rasters = Rásteres
common-redo = Rehacer
common-relimit-line = Reajustar límite de línea
common-remove-project = Eliminar el proyecto
common-reset-view = Restablecer vista
common-reveal-all = Revelar todo
common-reveal-finder = Mostrar en el Finder
common-rotate-collar = Girar brocal
common-save-exit = Guardar y salir
common-scale-bar = Barra de la escala
common-set-initiation-point = Establecer punto de iniciación
common-shape = Forma
common-shell = Con envolvente
common-slashes = Trazos
common-slice = Corte
common-slice-triangulation-z-range = Cortar triangulación por rango Z...
common-surface-contours = Curvas de nivel de superficie
common-text = Texto
common-degree-suffix = °
common-tie-holes = Conectar barrenos
common-triangulations = Triangulaciones
common-trim-topology = Recortar a topografía...
common-undo = Deshacer
common-undrape-all = Desproyectar todo
common-uniform-white = Blanco uniforme
common-unlock-all = Desbloquear todo
common-untitled = Sin título
common-up = Arriba
common-vertical-exaggeration = Exageración vertical
common-x = x
common-zoom-extents = Zoom a las extensiones

## Confirmations strings

confirmations-close-project-unsaved-changes = Cerrar proyecto: cambios sin guardar
confirmations-close-without-saving = Cerrar sin guardar
confirmations-delete = Eliminar
confirmations-delete-objects = Eliminar objetos
confirmations-discard = Descartar
confirmations-discard-all-unsaved-changes-layer =
    ¿Descartar todos los cambios sin guardar de la capa «{ $name }»?
    La capa guardada se volverá a cargar desde el disco y se conservarán los cambios de las demás capas. Esta acción no se puede deshacer.
confirmations-discard-all-unsaved-changes-name =
    ¿Descartar todos los cambios sin guardar de «{ $name }»?
    La última versión guardada se volverá a cargar desde el disco. Esta acción no se puede deshacer.
confirmations-discard-changes = Descartar los cambios
confirmations-exit-unsaved-changes = Salir: cambios sin guardar
confirmations-incline-design-cannot-reproduce-all = Incline Design no puede reproducir todo el contenido del OMF original. Al guardar se omitirá lo siguiente:
confirmations-product = Producto
confirmations-project = este proyecto
confirmations-remove-name-delete-its-browser = ¿Quitar «{ $name }» y eliminar su copia almacenada en el navegador? Se perderán los cambios sin guardar.
confirmations-remove-project-unsaved-changes = Quitar proyecto: cambios sin guardar
confirmations-remove-without-saving = Eliminar sin guardar
confirmations-replace-project-unsaved-changes = Proyecto de reemplazo: cambios no guardados
confirmations-save = Guardar
confirmations-save-anyway = Guardar de todos modos
confirmations-save-changes-current-project-before = ¿Guardar los cambios en el proyecto actual antes de reemplazarlo?
confirmations-save-changes-name-before-closing = ¿Guardar los cambios de «{ $name }» antes de cerrarlo?
confirmations-save-changes-name-before-removing = ¿Guardar los cambios de «{ $name }» antes de quitarlo de Incline Design?
confirmations-save-close = Guardar y cerrar
confirmations-save-modified-project-before-exiting = ¿Guardar el proyecto modificado antes de salir?
confirmations-save-to-browser-before-exit = ¿Guardar el proyecto modificado en el almacenamiento del navegador antes de salir?
confirmations-save-remove = Guardar y eliminar

## Console strings

console-copy-all = Copiar todo
console-copy-message = Copiar el mensaje
console-error = ERROR
console-info = INFORMACIÓN
console-no-console-activity-yet = Aún no hay actividad en la consola
console-pending = PENDIENTE
console-progress-summary = En curso · { $summary }
console-success = CORRECTO
console-warn = AVISO

## Csv strings

csv-block-model-category = Categoría
csv-block-model-value = Valor

## Drill strings

drill-hole-add-stop = Agregar parada
drill-hole-all-rendered-intervals-opaque-white = Todos los intervalos renderizados son blancos opacos.
drill-hole-burden-spacing-must-greater-than = La piedra y el espaciamiento deben ser mayores que cero
drill-hole-choose-valid-closed-polyline = Elija una polilínea cerrada válida
drill-hole-colour-scale = Escala de color
drill-hole-field = Campo
drill-hole-grayscale = Escala de grises
drill-hole-green-yellow-red = Verde–amarillo–rojo
drill-hole-heat = Calor
drill-hole-no-holes-fit-inside-boundary = No cabe ningún barreno dentro de este límite con la piedra y el espaciamiento actuales
drill-hole-pattern-too-many-holes = El patrón supera el máximo de { $maximum } barrenos; aumente la piedra o el espaciamiento
drill-hole-preset = Preajuste
drill-hole-px = px
drill-hole-rainbow = Arcoíris
drill-hole-reset-preset = Restablecer preajuste
drill-hole-rotation-offsets-must-contain-valid = La rotación y los desplazamientos deben contener números válidos
drill-hole-selected-polyline-has-no-usable = La polilínea seleccionada no tiene un área XY utilizable
drill-hole-smooth-interpolation = Interpolación suave
drill-hole-spacing-would-scan-too-many = Este espaciamiento examinaría demasiadas celdas; aumente la piedra o el espaciamiento (máximo: { $maximum } barrenos)
drill-hole-square = Cuadrado
drill-hole-staggered = Tresbolillo
drill-hole-stepped-bands = Bandas escalonadas
common-times-sign = ×
common-minus-sign = −
drill-hole-unsupported-drillhole-source = Origen de sondajes no compatible
drill-hole-width = Ancho
drill-pattern-arrangement = Disposición
drill-pattern-axis-offset = Desfase en { $axis }
drill-pattern-blast-shape = Contorno de voladura
drill-pattern-burden = Piedra
drill-pattern-choose-closed-blast-boundary-then = Elija un límite de voladura cerrado y ajuste la cuadrícula. Los barrenos se actualizan en tiempo real en la vista.
drill-pattern-closed-design-polyline-whose-xy = Polilínea de diseño cerrada cuya huella XY se rellenará con barrenos.
drill-pattern-rotation-help = Rotación del patrón en sentido antihorario desde el eje global { $axis }.
drill-pattern-distance-between-holes-along-each = Distancia entre barrenos a lo largo de cada fila del patrón.
drill-pattern-name-hint = p. ej., Corte oeste 03
drill-pattern-diameter-help = Diámetro final del barreno. Se introduce en milímetros y se guarda con cada barreno generado.
drill-pattern-hole-depth = Profundidad del barreno
drill-pattern-hole-diameter = Diámetro del barreno
drill-pattern-move-over-closed-polyline-then = Pase sobre una polilínea cerrada y haga clic en ella en la vista. Esc cancela la selección.
drill-pattern-name-help = Nombre del conjunto de barrenos creado en el proyecto.
drill-pattern-none-picked = Ninguno seleccionado
drill-pattern-pattern-name = Nombre del patrón
drill-pattern-spacing-help = Distancia perpendicular entre las filas del patrón.
drill-pattern-pick = Seleccionar
drill-pattern-preview-count-hole-s-diameter = Vista previa: { $count } barreno(s) · { $diameter } mm de diámetro · { $depth } m de profundidad
drill-pattern-rotation = Rotación
drill-pattern-shift-pattern-grid-along-global = Desplaza la cuadrícula del patrón a lo largo del eje global { $axis } manteniéndola recortada a la forma de la tronadura.
drill-pattern-spacing = Espaciamiento
drill-pattern-staggered-offsets-every-second-row = El tresbolillo desplaza cada segunda fila la mitad del espaciamiento.
drill-pattern-vertical-depth-below-each-collar = Profundidad vertical bajo cada brocal.

## Dxf strings

dxf-block-nesting-too-deep = El anidamiento de bloques DXF supera la profundidad máxima ({ $depth }); se omite «{ $name }»
dxf-circular-block-reference = Se detectó una referencia circular de bloque DXF: «{ $name }»
dxf-undefined-layer = Una entidad DXF hizo referencia a la capa indefinida «{ $name }»; se importó como «{ $fallback }»
dxf-import-budget-exceeded = La importación DXF supera el presupuesto de { $what } ({ $limit }); se omite la geometría restante
dxf-insert-unknown-block = DXF INSERT hace referencia al bloque desconocido «{ $name }»

## Edit strings

edit-absolute-length = Longitud absoluta
edit-absolute-rl = RL absoluto
edit-action = Acción
edit-angle = Ángulo
edit-dip-help = Ángulo desde la horizontal, negativo hacia abajo: -90 es un barreno vertical.
edit-app-web-not-recommended-production = No se recomienda usar { $app } Web en producción. Úselo solo como demostración.
edit-application = Aplicación
edit-apply = Aplicar
edit-apply-pick-target = Aplicar y elegir el objetivo
edit-axis-value = Valor de { $axis }
edit-azimuth = Acimut
edit-batter-angle = Ángulo de talud (°)
edit-azimuth-help = Rumbo de perforación de los barrenos, en grados en sentido horario desde el norte de la cuadrícula.
edit-bench-height = Altura de banco
edit-benches = Bancos
edit-berm-width = Ancho de berma
edit-bezier-curve = Curva de Bezier
edit-choose-layer = Elegir una capa
edit-measure-help = Elija si el valor introducido es la distancia sobre la pendiente, el ancho horizontal o la altura vertical.
edit-choose-which-two-polyline-paths = Elija cuál de los dos recorridos de la polilínea entre los vértices seleccionados se sustituirá. La longitud incluye la elevación y las aristas curvas.
edit-click-corner-closed-polyline = Haga clic en una esquina en una polilínea cerrada.
edit-click-open-closed-polyline-begin = Para comenzar, haga clic en una polilínea abierta o cerrada.
edit-click-second-vertex-replacement-span = Haga clic en el segundo vértice del tramo de reemplazo.
edit-click-vertex-start-replacement-span = Haga clic en un vértice para comenzar el tramo de reemplazo.
edit-collide-triangulation = Colisionar con triangulación
edit-confirm-selection = Confirmar selección
edit-control-point-1 = Punto de control 1
edit-control-point-2 = Punto de control 2
edit-copy = Copiar
edit-corner-radius-limited-so-replacement = Radio de esquina, limitado para que el reemplazo no pueda pasar los vértices adyacentes.
edit-create-new-layer = Crear una nueva capa
edit-create-new-project = Crear un nuevo proyecto
edit-create-project = Crear un proyecto
edit-delta-length-m-use = Variación de longitud (m, use + o -)
edit-dip = Buzamiento
edit-direction = Dirección
edit-distance = Distancia
edit-distance-along-slope = Distancia sobre la pendiente
edit-download-free-native-version-our = Descargue la versión nativa gratuita en nuestro sitio web ↗
edit-drill-hole = Sondaje
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = Fin
edit-enter-valid-elevation = Ingrese una elevación válida.
edit-exit-slice = Salir del corte
edit-finish-polyline = Finalizar polilínea
edit-generate-batter-berms = Generar bancos y bermas
edit-height = Altura
edit-height-change = Cambio de altura
edit-height-mode = Modo de altura
edit-horizontal-distance = Distancia horizontal
edit-horizontal-width-each-flat-berm = Ancho horizontal de cada berma plana entre taludes sucesivos.
edit-hover-choose-which-end-move = Coloque el cursor sobre el extremo que desea mover y luego haga clic para confirmar.
edit-insert-point-elevation = Insertar punto en la elevación
edit-intersect = Intersecar
edit-kind-properties = { $properties } de { $kind }
edit-layer-name = Nombre de la capa
edit-load-project = Cargar proyecto
edit-longest = Más largo
edit-m-s = m/s
edit-measure = Medida
edit-mit-license = Licencia MIT
edit-mode = Modo
edit-move = Mover
edit-move-layer = Mover a la capa
edit-move-which-end = Elija qué extremo mover
edit-movement-speed-slice-when-using = Velocidad de movimiento del corte cuando se utilizan las teclas de navegación.
edit-moving-end-endpoint = Moviendo: punto final
edit-moving-start-endpoint = Moviendo: punto inicial
edit-new-length-m = Longitud nueva (m)
edit-new-project = Proyecto nuevo
edit-number-complete-batter-berm-levels = Número de niveles completos de talud y berma. El máximo se limita al nivel más profundo que conserva la geometría especificada.
edit-bezier-segments-help = Número de segmentos de línea usados para aproximar la curva entre los dos vértices seleccionados.
edit-chamfer-segments-help = Número de segmentos rectos usados para aproximar la esquina redondeada. Use 1 para un chaflán recto.
edit-object = Objeto
edit-offset-element = Elemento de compensación
edit-pick-side = Seleccionar el lado
edit-pit = Tajo
edit-project-name = Nombre del proyecto
edit-properties = Propiedades
edit-radius = Radio
edit-recent = Recientes
edit-relative = Relativo (+/-)
edit-elevation-mode-help = Relativo aplica un cambio vertical a todos los puntos. Cota absoluta proyecta todos los puntos sobre una misma elevación objetivo.
edit-remove-from-list = Eliminar de la lista
edit-replace-path = Reemplazar ruta
edit-rotate = Rotar
edit-rotation-speed-slice-when-using = Velocidad de rotación del corte al utilizar Q y E.
edit-s = °/s
edit-segments = Segmentos
edit-segments-lying-elevation-ignored = Los segmentos que se encuentran en esta elevación son ignorados.
edit-endpoint-help = Seleccione el extremo que cambia; el otro permanece fijo.
edit-selected-holes-point-different-ways = Los barrenos seleccionados apuntan en direcciones distintas. Aplicar establece estos ángulos en todos ellos.
edit-selected-start-end-point-moves = El punto inicial o final seleccionado se mueve en la dirección de la línea; el extremo opuesto permanece fijo.
edit-set-axis = Establecer { $axis }
edit-shortest = Más corto
edit-slice-view = Vista de corte
edit-slope-angle-each-batter-face = Ángulo de inclinación de cada talud del banco, medido desde la horizontal.
edit-slope-angle-offset-positive-negative = Ángulo de pendiente del desplazamiento. Los ángulos positivos y negativos mueven la copia por encima o por debajo del origen mientras se desplaza lateralmente.
edit-speed = Velocidad
edit-start = Inicio
edit-stockpile = Acopio
edit-stop-generated-offset-where-its = Detener el desplazamiento generado cuando su camino se encuentre primero con una triangulación visible.
edit-target-rl = Cota objetivo
edit-text-colour-opacity = Color del texto y opacidad.
edit-thickness-visible-slice-slab-centred = El espesor de la losa de corte visible centrada en el indicador de visión general.
edit-translation-axis-help = Distancia de traslación a lo largo del eje mundial { $axis }.
edit-type = Tipo
edit-type-direction-together-set-offset = El tipo y la dirección determinan juntos el lado del desplazamiento. Cantera + Arriba y Acopio + Abajo avanzan hacia fuera; Cantera + Abajo y Acopio + Arriba, hacia dentro.
edit-bench-direction-help = Arriba eleva cada banco según la altura del banco; Abajo lo desciende. También invierte el lado del desplazamiento; consulte Tipo.
edit-value-help = El valor se interpreta utilizando el modo Medida y altura seleccionado.
edit-vertical-rise-fall-each-bench = Alza o caída vertical de cada banco antes de que se cree la siguiente berma.
edit-bezier-control-point-1-help = Coordenadas X, Y y Z universales del primer punto de control Bézier.
edit-bezier-control-point-2-help = Coordenadas X, Y y Z universales del segundo punto de control Bézier.

## Events strings

events-couldn-t-exit-error = No se pudo salir: { $error }
events-couldn-t-save-error = No se pudo guardar: { $error }
events-set-elevation = Establecer elevación
events-set-elevation-from-cursor-hit = Se estableció la elevación desde el punto del cursor en Z { $z }
events-tool-not-available-section-view = Esa herramienta no está disponible en la vista de sección

## Explorer strings

explorer-clear-active-triangulation-texture = Borrar textura activa de la triangulación
explorer-delete-from-project = Eliminar del proyecto
explorer-discard-changes = Descartar los cambios...
explorer-download = Descargar
explorer-drape-over-surface = Proyectar sobre la superficie
explorer-draped-over-surface = Drapeado sobre una superficie
explorer-duplicate = Duplicar
explorer-face-colour = Color de cara
explorer-id-block-model-id-source =
    ID: block-model:{ $id }{ $source }
    { $count } variable(s) de color
explorer-id-drill-holes-id-source =
    ID: drill-holes:{ $id }{ $source }
    { $holes } sondaje(s)
    { $fields } campo(s) de color
explorer-id-point-cloud-id-source =
    ID: point-cloud:{ $id }{ $source }
    { $count } punto(s)
explorer-raster-id =
    ID: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = ID: triangulación:{ $id }{ $source }
explorer-load = Cargar
explorer-lock = Bloquear
explorer-select-all-objects = Seleccione todos los objetos
explorer-source-name = Fuente: { $name }
explorer-unload = Descargar
explorer-unlock = Desbloquear

## Files strings

files-automatic-colour = Color automático
files-automatic-rl-spacing = Espaciado de cotas automático
files-axis-scale-ratio = Relación de escala en { $axis }
files-ok = Aceptar
files-reset-scale = Restablecer a 1×
files-rl-grid-options = Opciones de cuadrícula de cotas
files-rl-spacing = Espaciado de cotas
files-scales-z-distances-visually-without = Escala las distancias Z visualmente sin cambiar las coordenadas almacenadas.
files-thickness = Grosor
files-xy-grid-options = Opciones de cuadrícula XY

## Gpu strings

gpu-cache-block-model-surface-build-failed = Error al construir la superficie del modelo de bloques: { $error }
gpu-cache-block-model-surface-build-worker = Se desconectó el proceso de construcción de la superficie del modelo de bloques
gpu-cache-block-model-surface-chunk-rejected = El fragmento de superficie del modelo de bloques se rechazó antes de asignar la GPU: instancias={ $instances } bytes, límite={ $limit } bytes
gpu-cache-block-volume-worker-disconnected = Se desconectó el proceso de preparación del volumen de bloques
gpu-cache-translucent-volume-could-not-built = No se pudo crear el volumen translúcido ({ $error }); este modelo de bloques se mostrará como cubos.
gpu-cache-edge-chunk-rejected = El fragmento de aristas de triangulación se rechazó antes de asignar la GPU: instancias={ $instances } bytes, límite={ $limit } bytes
gpu-cache-triangulation-chunk-rejected = El fragmento de triangulación de la GPU se rechazó antes de asignar memoria: vértices={ $vertices } bytes, índices={ $indices } bytes, límite={ $limit } bytes
gpu-cache-triangulation-too-many-vertices = La triangulación «{ $name }» tiene { $count } vértices (> u32::MAX); no se puede dividir para la GPU
gpu-cache-triangulation-uploaded = La triangulación «{ $name }» se cargó en { $chunks } fragmentos espaciales ({ $faces } caras)
i18n-active-language = El idioma activo es { $language } (incluidos: { $bundled })
i18n-could-not-select-language-error = No se pudo seleccionar un idioma: { $error }

## Init strings

init-gpu-adapter-vendor-name-backend = Adaptador GPU: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver = Controlador GPU: { $driver } { $driver_info }
init-gpu-supports-maximum-buffer-size = La GPU admite un búfer máximo de { $size } MiB; las escenas grandes podrían no mostrarse por completo
init-surface-present-mode = Modo de presentación de la superficie: { $mode }
init-wgpu-error-continuing-error = Error de wgpu (se continúa): { $error }

## Io strings

io-ascii-points-xyz-pts = Puntos ASCII (.xyz, .pts)
io-attribute = Atributo
io-blank-header = (encabezado en blanco)
io-block-model = Modelo de bloques:
io-choose-file-purpose-map-its = Seleccione una finalidad del archivo para mapear sus columnas.
io-choose-loaded-block-model = Elegir un modelo de bloques cargado
io-choose-loaded-layer = Elegir una capa cargada
io-choose-loaded-triangulation = Elegir una triangulación cargada
io-choose-purpose = Elegir propósito…
io-choose-source-file-files-import = Seleccione el archivo fuente o archivos para importar.
io-collar = Collar
io-column-mapping = Mapeo de columnas
io-comma-separated-values-csv = Valores separados por comas (.csv)
io-csv-files = Archivos CSV
io-default = Por defecto
io-depth = Profundidad
io-diameter = Diámetro
io-drawing-exchange-format-dxf = Formato de intercambio de dibujos (.dxf)
io-drill-holes = Sondajes
io-east-x = Este / X
io-elevation-z = Elevación / Z
io-end-x = X final
io-end-y = Y final
io-end-z = Z final
io-explicit-segments = Segmentos explícitos
io-export = Exportar
io-export-csv-block-model = Exportar CSV de modelo de bloques
io-export-dxf = Exportar DXF
io-export-one-layer = Exportar una capa
io-export-ply = Exportar PLY
io-export-stl = Exportar STL
io-export-wavefront-obj = Exportar Wavefront OBJ
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = Importar
io-import-ascii-point-cloud = Importar nube de puntos ASCII
io-import-drillhole-csv-bundle = Importar paquete de CSV de sondajes
io-import-geotiff = Importar GeoTIFF
io-import-las-laz-point-cloud = Importar nube de puntos LAS/LAZ
io-import-pcd-point-cloud = Importar nube de puntos PCD
io-import-ply = Importar PLY
io-import-stl = Importar STL
io-import-wavefront-obj = Importar Wavefront OBJ
io-interval = Intervalo
io-las-laz-las-laz = LAS/LAZ (.las, .laz)
io-mapped-csv-bundle-csv = Paquete CSV mapeado (.csv)
io-model-file = Archivo de modelo
io-name-count-files = { $name } + { $count } archivos
io-no-csv-chosen = No se eligió ningún archivo .csv
io-no-csv-files-chosen = No se eligieron archivos CSV
io-no-dxf-chosen = No se eligió ningún archivo .dxf
io-no-omf-chosen = No se eligió ningún archivo .omf
io-north-y = Norte / Y
io-ply = PLY (.ply)
io-point-cloud-data-pcd = Datos de nube de puntos (.pcd)
io-source-file = Archivo de origen
io-start-x = X inicial
io-start-y = Y inicial
io-start-z = Z inicial
io-stl = STL (.stl)
io-triangulation = Triangulación:
io-unmapped = Sin asignar
io-wavefront-obj = Wavefront OBJ (.obj)

## Jobs strings

jobs-background-task-poll-label-ended = La tarea en segundo plano «{ $poll_label }» terminó sin resultado
jobs-discarded-stale-result = Se descartó el resultado en segundo plano obsoleto de «{ $poll_label }» porque una fuente cambió o se cerró

## Logging strings

logging-activity-completed = Actividad completada
logging-activity-started = Actividad iniciada
logging-application-id-id = ID de la aplicación: { $id }
logging-application-name = Nombre de la aplicación: { $name }
logging-application-startup = Inicio de la aplicación
logging-build-target-os-architecture = Destino de compilación: { $os }-{ $architecture }
logging-completed = Completado
logging-count-messages = { $count } mensajes
logging-desktop-session-xdg-session-type = Sesión de escritorio: XDG_SESSION_TYPE={ $session }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
logging-initialising-incline-design = Inicializando Incline Design
logging-locale-environment = Entorno regional: LANG={ $lang }, LC_ALL={ $locale }, TZ={ $timezone }
logging-macos-session = Sesión de macOS: USER={ $user }, SHELL={ $shell }
logging-operating-system-gnu-linux = Sistema operativo: GNU/Linux
logging-operating-system-macos = Sistema operativo: macOS
logging-operating-system-microsoft-windows = Sistema operativo: Microsoft Windows
logging-pointer-width = Ancho de puntero: { $width } bits
logging-process-id-id = ID del proceso: { $id }
logging-release-version = Versión de lanzamiento: { $version }
logging-renderer = Renderizador
logging-rust-compiler-host = Host del compilador de Rust: { $host }
logging-system = Sistema
logging-system-error = Error del sistema
logging-unknown = desconocido
logging-windows-session-sessionname-session = Sesión de Windows: SESSIONNAME={ $session }, USERNAME={ $user }
logging-working = En curso…

## Mac strings

mac-cannot-install-macos-menu-bar = No se puede instalar la barra de menús de macOS fuera del hilo principal
mac-quit-app = Salir de { $app }

## Main strings

main-incline-design-web-startup-failed = Error al iniciar Incline Design Web: { $error }

## Menu strings

menu-count-files-selected = { $count } archivos seleccionados

## Object strings

object-edit-appearance = Apariencia
object-edit-arc-circle = Arco y círculo
object-edit-arc-segments = Segmentos de arco
object-edit-bulge = Combadura
object-edit-bulge-arcs-horizontal-data-model = Los arcos de combadura son horizontales según el modelo de datos: el arco gira en planta y la elevación varía en línea recta de un vértice al siguiente.
object-edit-centre-x = Centro X
object-edit-centre-y = Centro Y
object-edit-centre-z = Centro Z
object-edit-chord = Cuerda
object-edit-colour-layer = Color por capa
object-edit-enter-number = Introduzca un número
object-edit-follow-owning-layer-s-colour = Seguir el color de la capa propietaria en lugar de un color fijado a este objeto.
object-edit-id = ID
object-edit-identity = Identidad
object-edit-insert-after = Insertar después
object-edit-join-last-vertex-back-first = Une el último vértice de nuevo con el primero.
object-edit-length = Longitud { $length } m
object-edit-move-down = Bajar
object-edit-move-up = Subir
object-edit-object-has-no-arc-segments = Este objeto no tiene segmentos de arco.
object-edit-object-has-single-position = Este objeto tiene una única posición.
object-edit-object-needs-least-required-vertices = Este objeto necesita al menos { $required } vértices
object-edit-one-more-properties-not-valid = Una o más propiedades no son un número válido
object-edit-perimeter-area = Perímetro { $length } m, área { $area } m²
object-edit-reverse = Invertir
object-edit-row-invalid-number = Fila { $row }: la posición o la combadura no son un número válido
object-edit-sweep = Barrido
object-edit-text-not-number = «{ $text }» no es un número
object-edit-vertices = Vértices

## Omf strings

omf-element-name-has-count-tie = El elemento «{ $name }» tiene { $count } conexión(es) que nombran barrenos que ya no contiene
omf-ignoring-colour-map-omf-attribute = Se ignora el mapa de colores del atributo OMF «{ $attribute }»: { $error }
omf-mining-data-exported-incline = Datos mineros exportados por Incline
omf-import = Importación OMF
omf-texture = Textura OMF
omf-validation-warnings = Advertencias de validación OMF: { $warnings }
omf-application-metadata-dropped = Los metadatos de aplicación del proyecto «{ $application }» no se conservan
omf-project-author-not-retained = El autor del proyecto no se conserva
omf-project-description-not-retained = La descripción del proyecto no se conserva
omf-unsupported-metadata-keys = El proyecto contiene claves de metadatos no compatibles: { $keys }

## Plot strings

plot-1-1000-one-millimetre-sheet = A escala 1:1000, un milímetro en la hoja es un metro en el suelo.
plot-1-scale-covers-width-height = 1:{ $scale } · cubre { $width } × { $height } m
plot-all-visible-data = Todos los datos visibles
plot-automatic-grid-interval = Intervalo automático de cuadrícula
plot-border = Borde
plot-centre = Centrar en
plot-fit-scale-help = Elija la escala convencional más pequeña que se adapte a todo lo visible en la hoja.
plot-coordinate-grid = Grilla de coordenadas
plot-current-view-centre = Centro de la vista actual
plot-date-caps = FECHA
plot-date = Fecha
plot-dots-per-inch-paper-size = Puntos por pulgada. Este tamaño de papel puede rasterizarse hasta { $max_dpi } ppp; 300 ppp es una calidad de impresión normal.
plot-dpi = dpi
plot-drawing-no = PLANO N.º
plot-drawing-number = Número de dibujo
plot-drawn-by-caps = DIBUJADO POR
plot-drawn-by = Dibujado por
plot-e-g-example-gold-project = p. ej., Proyecto de oro de ejemplo
plot-entered-coordinates = Coordenadas introducidas
plot-export-png = Exportar PNG...
plot-fit-scale-visible-data = Ajustar escala a los datos visibles
plot-grid-interval = Intervalo de cuadrícula
plot-landscape = Horizontal
plot-lists-visible-surfaces-design-layers = Enumera las superficies visibles y las capas de diseño con sus colores.
plot-margin = Margen
plot-margins-leave-no-room-map = Los márgenes no dejan espacio para el mapa
plot-metres-scale-1-scale = metros    Escala 1:{ $scale }
plot-mm = mm
plot-north-arrow = Flecha norte
plot-nothing-visible-draw = No hay nada visible que dibujar
plot-paper = Papel
plot-paper-orientation-width-height-mm = { $paper } { $orientation } · { $width } × { $height } mm
plot-paper-size = Tamaño del papel
plot-pick-interval-reads-roughly-every = Seleccione un intervalo que se lee aproximadamente cada 50 mm en la hoja impresa.
plot-plan = Planta
plot-scale-must-be-positive = La escala del plano debe ser un número positivo
plot-png-written-sheet-s-exact = El PNG se escribe con el tamaño físico exacto de la hoja y registra sus ppp, por lo que se imprime a escala real.
plot-portrait = Vertical
plot-resolution = Resolución
plot-rev = REV.
plot-revision = Revisión
plot-scale = ESCALA
plot-scale-ratio = Escala 1:
plot-scale-framing = Escala y enmarcado
plot-sheet-furniture = Elementos de la hoja
plot-size-width-height-mm = { $size } ({ $width } × { $height } mm)
plot-subtitle = Subtítulo
plot-title = Título
plot-title-block = Bloqueo de título
plot-today = hoy

## Products strings

products-add-initiation = Añadir iniciación
products-delay = Retraso
products-delay-palette = Paleta de retrasos
products-how-long-after-shot-fired = Tiempo transcurrido desde el disparo hasta que este brocal inicia la voladura.
products-initiation-name = Iniciación · { $name }
products-milliseconds-between-one-hole-firing = Milisegundos entre la detonación de un agujero y la del siguiente.
products-ms = ms
products-no-products = Sin productos
products-remove = Quitar
products-update = Actualizar

## Progress strings

progress-percent-done-total = { $percent } ({ $done } de { $total })
progress-task-finished = { $task }: finalizada

## Project strings

project-item = Elemento

## Properties strings

properties-adds-view-dependent-rim-highlight = Añade un realce de borde dependiente de la vista en los límites de bloques y materiales. Desactivarlo reduce ligeramente el trabajo del renderizado volumétrico.
properties-block-model-downscale = Reducción de escala del modelo de bloques
properties-camera = Cámara
properties-camera-clip-planes = Planos de recorte de la cámara
properties-cap-while-resizing = Limitar al redimensionar
properties-dark-mode = Modo oscuro
properties-developer = Desarrollador
properties-downscale-rasters = Reducción de escala de rásteres
properties-edit-object = Editar objeto...
properties-field-view = Campo de visión
properties-fps = fotogramas/s
properties-frame-counter = Contador de cuadros
properties-frame-rate-cap = Límite de velocidad de cuadros
properties-hz = Hz
properties-interface = Interfaz
properties-invert-horizontal = Invertir horizontal
properties-invert-vertical = Invertir vertical
properties-limits-newly-loaded-geotiff-previews = Limita las vistas previas de GeoTIFF recién cargados a 4096 píxeles en su lado mayor. Desactívelo para usar la resolución completa hasta el límite de textura de la GPU, con mayor uso de memoria.
properties-line-colour = Color de la línea
properties-look-sensitivity = Sensibilidad de vista
properties-max-clip-span = Rango máximo de recorte
properties-move-layer = Mover a la capa...
properties-near-clip-limit = Límite de recorte cercano
properties-orbit-sensitivity = Sensibilidad orbital
properties-panel-chrome = Cromado del panel
properties-performance = Rendimiento
properties-plan-mode = Modo de planta
properties-presents-step-display-no-tearing = Se presenta en sincronía con la pantalla: sin tearing, y la pantalla determina la tasa de fotogramas. Si está desactivado, los fotogramas se presentan tan pronto se dibujan y se aplica el límite indicado abajo.
properties-reflective-block-edges = Bordes reflectantes de los bloques
properties-restore-defaults = Restaurar valores predeterminados
properties-show-console = Mostrar consola
properties-shows-live-near-far-projection = Muestra las distancias de proyección en vivo cerca y lejos en la barra de estado.
properties-snap-polling = Muestreo de ajuste
properties-vertical-sync = Sincronización vertical
properties-world-axis-gizmo = Gizmo de ejes del mundo
properties-zoom-cursor = Zoom al cursor
properties-zoom-sensitivity = Sensibilidad al zoom

## Screenshot strings

screenshot-could-not-encode-viewport-image = No se pudo codificar la imagen de la vista: { $error }
screenshot-could-not-map-viewport-screenshot = No se pudo mapear la captura de la vista: { $error }
screenshot-could-not-save-viewport-image = No se pudo guardar la imagen de la vista { $path }: { $error }
screenshot-downloaded-viewport-image-file-name = Imagen de la vista descargada: { $file_name }
screenshot-saved-viewport-image-path = Imagen de la vista guardada: { $path }
screenshot-viewport-image-download-failed-error = Error al descargar la imagen de la vista: { $error }

## Spatial strings

spatial-bvh-face-index-out-of-range = El índice de cara BVH { $index } está fuera del rango de la malla; se sustituye por un triángulo degenerado

## State strings

state-above = en o por encima
state-activate-project = Activar el proyecto
state-all-open-incline-design-data = Todos los datos abiertos de Incline Design
state-apply-generated-rings = Aplicar anillos generados
state-apply-selection = Aplicar a la selección
state-rotate-by-azimuth-dip = por azimut { $azimuth }°, inclinación { $dip }°
state-rotate-to-azimuth-dip = hasta azimut { $azimuth }°, inclinación { $dip }°
state-below = en o por debajo
state-centre-rotation = Centro de rotación
state-checking-unsaved-work = Comprobando trabajo sin guardar
state-choose-destination = Elegir un destino
state-choose-one-more-files = Elegir uno o más archivos
state-clear-raster = Borrar ráster
state-click-pit-shell-viewport = Haga clic en la envolvente del tajo en el puerto de vista.
state-click-pit-stockpile-solid-viewport = Haga clic en el sólido tajo o acopio en el puerto de vista.
state-click-surface-viewport = Haga clic en la superficie en el puerto de vista.
state-click-topology-viewport = Haga clic en la topografía en el puerto de vista.
state-close-project = Cerrar proyecto
state-colour-drillholes = Color sondajes
state-copy-objects-layer = Copiar objetos a capa
state-count-file-s = { $count } archivo(s)
state-count-object-s-axis-value = { $count } objeto(s) · { $axis } { $value }
state-count-object-s-closed = { $count } objeto(s) · { $closed }
state-count-object-s-layer = { $count } objeto(s) · { $layer }
state-count-object-s-weight = { $count } objeto(s) · { $weight }
state-count-object-s-z-elevation = { $count } objeto(s) · Z { $elevation }
state-create-point-cloud-tin = Crear nube de puntos TIN
state-create-project = Crear un proyecto
state-current-project = Proyecto actual
state-cut-topology-pit-shell = Cortar topografía a envolvente del tajo
state-cut-triangulation-polyline = Cortar triangulación por polilínea
state-cut-triangulation-z = Cortar triangulación por Z
state-dark-mode = Modo oscuro
state-detached = Separado
state-disabled = Desactivado
state-discard-project-changes = Descartar los cambios del proyecto
state-discard-replace-project = Descartar y reemplazar el proyecto
state-discarding-unsaved-changes = Descartando cambios sin guardar
state-docked = Acoplado
state-drape-raster = Proyectar ráster
state-drill-pattern = Patrón de perforación
state-duplicate-layer = Duplicar capa
state-east = Este
state-enabled = Activado
state-exit-incline-design = Salir de Incline Design
state-export-block-model-csv = Exportar modelo de bloques a CSV
state-export-layer-dxf = Exportar capa a DXF
state-export-omf = Exportar OMF
state-export-project-dxf = Exportar proyecto a DXF
state-export-triangulation = Exportar triangulación
state-export-viewport-image = Exportar imagen del puerto de vista
state-finish-closed-polyline = Terminar polilínea cerrada
state-finish-open-polyline = Terminar polilínea abierta
state-fit-extents = Ajustar a extensiones
state-fix-release-centre-both-views = Fija o libera el centro alrededor del cual orbitan ambas vistas
state-generate-contours = Generar curvas de nivel
state-hidden = Oculto
state-import-drillholes = Importar sondajes
state-import-omf = Importar OMF
state-import-point-cloud = Importar nube de puntos
state-import-raster = Importar ráster
state-import-triangulation = Importar triangulación
state-insert-intersection-points = Insertar puntos de intersección
state-insert-points-elevation = Insertar puntos en la elevación
state-keep-inside = Mantener dentro
state-keep-outside = Mantener fuera
state-kriged-block-model = Modelo de bloques por kriging
state-load-block-model = Cargar modelo de bloques
state-load-drillholes = Cargar sondajes
state-load-layer = Cargar capa
state-load-point-cloud = Cargar nube de puntos
state-load-raster = Cargar ráster
state-load-triangulation = Cargar triangulación
state-locked-count-object-s = { $count } objeto(s) bloqueado(s)
state-major-minor = Mayor { $major } · menor { $minor }
state-move-axis-value = Mover al valor del eje
state-move-objects-layer = Mover objetos a capa
state-name-count-holes = { $name } · { $count } barrenos
state-name-count-object-s = { $name } · { $count } objeto(s)
state-name-z-min-z-max = { $name } · { $z_min } a { $z_max }
state-next-edit = Edición siguiente
state-north = Norte
state-open-containing-folder = Abrir la carpeta contenedora
state-open-project = Abrir proyecto
state-preserve-view-angle = Conservar ángulo de vista
state-previous-edit = Edición anterior
state-project-id = Proyecto { $id }
state-remove-block-model = Eliminar el modelo de bloques
state-remove-drillholes = Eliminar los sondajes
state-remove-point-cloud = Eliminar la nube de puntos
state-remove-raster = Eliminar el ráster
state-remove-triangulation = Eliminar la triangulación
state-removed-from-active-triangulation = Quitado de la triangulación activa
state-removed-from-every-triangulation = Quitado de todas las triangulaciones
state-rename-kind = Cambiar nombre de { $kind }
state-save-close-project = Guardar y cerrar proyecto
state-save-despite-unsupported-content = Guardar pese al contenido no compatible
state-save-project = Guardar el proyecto como
state-save-replace-project = Guardar y reemplazar el proyecto
state-saving-current-project = Guardando el proyecto actual
state-section-name = Sección { $section }
state-select-layer-objects = Seleccione los objetos de la capa
state-selected-objects = Objetos seleccionados
state-selected-polylines = Polilíneas seleccionadas
state-selected-scene-elements = Elementos de escena seleccionados
state-set-block-model-variable = Establecer variable de modelo de bloques
state-set-drillhole-colour-preset = Establecer preajuste de color de sondaje
state-set-entity-lock = Establecer bloqueo de entidad
state-set-grid = Establecer cuadrícula
state-set-layer-lock = Establecer bloqueo de capa
state-set-line-weight = Establecer peso de línea
state-set-object-colour = Configurar el color del objeto
state-set-object-fill = Establecer relleno del objeto
state-set-point-visibility = Establecer visibilidad de puntos
state-set-polyline-closed = Establecer polilínea cerrada
state-set-raster-lock = Establecer bloqueo de ráster
state-set-standard-view = Configurar la vista estándar
state-set-topology-wireframes = Establecer mallas alámbricas de topografía
state-set-triangulation-colour = Establecer color de triangulación
state-show-console = Mostrar consola
state-show-project = Mostrar proyecto
state-shown = Mostrado
state-slice-mode = Modo de corte
state-slice-preview = Vista previa del corte
state-south = Sur
state-stem-contours = Curvas de nivel de { $stem }
state-target-new-name = { $target } a «{ $new_name }»
state-trim-above = Recortar arriba
state-trim-below = Recortar abajo
state-trim-triangulation-surface = Recortar triangulación a la superficie
state-undrape-raster = Desproyectar ráster
state-undrape-rasters = Desproyectar rásteres
state-unload-block-model = Descargar modelo de bloques
state-unload-drillholes = Descargar sondajes
state-unload-layer = Descargar capa
state-unload-point-cloud = Descargar nube de puntos
state-unload-raster = Descargar ráster
state-unload-triangulation = Descargar triangulación
state-untitled-project = Proyecto sin título
state-use-typed-radius = Usar el radio escrito
state-west = Oeste

## Status strings

status-clip-near-far = Clip cerca/lejos/Δ: -- / -- / --
status-frame-rate = Frecuencia de fotogramas

## Text strings

text-could-not-build-vector-mesh = No se pudo crear la malla vectorial para la fuente { $font }, glifo { $glyph }: { $error }
text-document-text-mesh-exceeded-its = La malla de texto del documento superó su rango de índices u32

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = Elija primero el conjunto de barrenos que desea conectar
tie-in-count-connector-s = { $count } conector(es)
tie-in-delete-tie-ins = Eliminar conexiones
tie-in-deleted-count-selected-tie-connector = Se eliminaron { $count } conectores seleccionados
tie-in-hole = barreno
tie-in-initiation-point-lifted-from-name = Punto de iniciación retirado de { $name }
tie-in-initiation-point-set-name-delay = Punto de iniciación establecido en { $name } con un retardo de { $delay } ms
tie-in-select-delay-product-palette-before = Seleccione un producto de retardo en la paleta antes de conectar los barrenos
tie-in-tied-connectors = Se conectaron { $count } conector(es) con un retardo de { $delay } ms usando { $product }
tie-in-tied-connectors-replacing = Se conectaron { $count } conector(es) con un retardo de { $delay } ms usando { $product }, reemplazando { $replaced }

## Toolbar strings

toolbar-fill-type = Tipo de relleno

## Toolbars strings

toolbars-auto-bench = Banco automático
toolbars-bezier-polyline = Polilínea Bézier
toolbars-chamfer-polyline-corners = Achaflanar esquinas de la polilínea
toolbars-create-text = Crear texto
toolbars-cursor-regular = Cursor: normal
toolbars-cursor-snap-line = Cursor: ajustar a línea
toolbars-cursor-snap-point = Cursor: ajustar a punto
toolbars-cursor-snap-surface = Cursor: ajustar a superficie
toolbars-delete-points = Eliminar puntos
toolbars-explode-polyline-lines = Descomponer polilínea en líneas
toolbars-fuse-polylines = Fusionar polilíneas
toolbars-measure-distance = Medir distancia
toolbars-new-layer = Capa nueva
toolbars-split-polyline-points = Dividir polilínea en puntos
toolbars-strike-dip = Rumbo y buzamiento
toolbars-tool-not-available-section-view = { $tool } - no disponible en la vista de sección

## Tri strings

tri-sampling-method-help = Adaptativo concentra vértices en terrenos complejos según el error de ajuste de plano; Uniforme los distribuye de forma regular. Podrán añadirse más métodos en el futuro.
tri-adaptive-quadtree = Adaptativo (cuadrícula jerárquica)
tri-axis-range = Rango en { $axis }
tri-base-topology-will-receive-pit = La topografía base que recibirá la forma de la mina o acopio.
tri-boundary-polyline = Polilínea de límite
tri-bridge-gaps-help = Cubre huecos y concavidades del límite más estrechos que este valor en la superficie. 0 aún cubre huecos de hasta aproximadamente el tamaño de la celda de muestreo; valores mayores rellenan huecos más grandes y erosionan concavidades del límite.
tri-budget = Presupuesto por
tri-cancel-pick = Cancelar selección
tri-candidate-detail = Detalle del candidato
tri-candidate-fine-cells-per-budgeted = Celdas finas candidatas por vértice presupuestado. Un valor mayor da al muestreador adaptativo más libertad para ubicar detalle, pero tarda más en generarse.
tri-cap-surface-share-source-points = Limite la superficie por una proporción de los puntos de origen o por un número exacto de vértices.
tri-choose-input-clicking-loaded-surface = Seleccione esta entrada haciendo clic en una superficie cargada en el puerto de vista
tri-choose-which-side-reference-topology = Elija qué lado de la topografía de referencia se quitará de la superficie dentro de su área XY compartida.
tri-clip = Recortar
tri-clip-creates-new-triangulation-name = El recorte crea una triangulación nueva con este nombre; la superficie de origen no se modifica.
tri-clip-surface-polyline = Recortar superficie por polilínea
tri-closed-pit-stockpile-solid-whose = Un sólido cerrado de mina o acopio cuyo límite expuesto se incluirá en el resultado.
tri-create-new-layer-contours-append = Cree una capa para las curvas de nivel o añádalas a una capa existente del proyecto activo.
tri-cut-topology-pit-shell = Cortar topografía con envolvente del tajo
tri-e-g-design-trimmed = Por ejemplo, design_trimmed
tri-e-g-mysurf-cut = Por ejemplo, mysurf_cut
tri-e-g-mysurf-slice = Por ejemplo, mysurf_slice
tri-e-g-surface-contour = Por ejemplo, surface_contour
tri-e-g-topo-cut = Por ejemplo, topo_cut
tri-e-g-topo-pit = Por ejemplo, topo_with_pit
tri-exact-number-surface-vertices-target = Número exacto de vértices objetivo de la superficie. Los valores muy grandes tardan en generarse y consumen mucha memoria.
tri-existing-ground-topology-will-cut = La topografía existente del terreno que cortará la envolvente de la mina.
tri-fill-holes-up = Rellenar agujeros hasta
tri-generate = Generar
tri-generate-contour-lines = Generar curvas de nivel
tri-generate-upper-surface = Generar la superficie superior
tri-hide-unload-sources = Ocultar y descargar fuentes
tri-higher-edge-will-enforced-each = En cada conflicto se impondrá la arista más alta. Los segmentos inferiores en conflicto se ignorarán como líneas de quiebre y la superficie se interpolará en esas zonas. Las polilíneas de origen no cambian.
tri-breaklines-cross = Las aristas de línea de quiebre resaltadas se cruzan o solapan en planta a distintas elevaciones. Una sola superficie de terreno no puede seguir ambas.
tri-intervals-colours = Intervalos y colores
tri-keep-clipped-topology-included-shape = Mantenga la topografía recortada y la forma incluida como triangulaciones separadas en vez de combinarlas en una sola entidad.
tri-keep-inside-discards-surface-outside = Mantener dentro descarta la superficie exterior a la polilínea. Mantener fuera recorta de la superficie un hueco con la forma de la polilínea.
tri-keeps-only-surface-within-polyline = Conserva solo la superficie dentro del límite de la polilínea.
tri-keep-surface-relation-help = Mantiene la superficie { $relation } la topografía dentro de su cobertura XY.
tri-layer-already-exists-select-above = Esa capa ya existe; selecciónela arriba o elija otro nombre.
tri-limit-z-range = Limitar rango Z
tri-major = Principal
tri-max-edge-length = Longitud máxima del borde
tri-merge = Combinar
tri-method = Método
tri-min = Mín.
tri-minimum-maximum-elevations-retained = Elevaciones mínima y máxima conservadas en la superficie de salida. La mínima debe ser menor que la máxima.
tri-minor = Secundario
tri-contour-interval-help = Menor controla las curvas normales. Mayor controla las destacadas y debe usar un intervalo al menos tan grande como Menor.
tri-move-cursor-over-loaded-surface = Mueva el cursor sobre una superficie cargada.
tri-slice-output-name-help = Nombre asignado a la superficie de salida cortada por elevación.
tri-name-assigned-merged-topology-pit = Nombre asignado al resultado de topografía y tajo/acopio fusionados.
tri-name-assigned-newly-created-contour = Nombre asignado a la capa de curvas de nivel recién creada.
tri-reconstruct-output-name-help = Nombre asignado a la triangulación reconstruida.
tri-name-assigned-topology-after-pit = Nombre asignado a la topografía después de que la envolvente del tajo se corte de ella.
tri-name-assigned-trimmed-output-surface = Nombre asignado a la superficie de salida recortada.
tri-nearby-breakline-vertices-do-not = Los vértices cercanos de las líneas de quiebre no coinciden exactamente, por lo que no se puede triangular la superficie.
tri-new-layer = Capa nueva
tri-new-layer-name = Nombre de la nueva capa
tri-once-merge-succeeds-unload-source = Tras una fusión correcta, descargue la topografía y el sólido de origen para que solo quede el resultado fusionado en la escena.
tri-only-loaded-pickable = Solo se pueden seleccionar las triangulaciones cargadas.
tri-operation = Operación
tri-output-layer = Capa de salida
tri-percentage = Porcentaje
tri-percentage-cloud = Porcentaje de la nube
tri-pick-from-view = Seleccionar desde la vista
tri-pit-design-surface-only-areas = La superficie de diseño de la mina. Para el corte solo se usan las zonas donde excava por debajo de la topografía.
tri-pit-shell = Envolvente de mina
tri-pit-stockpile-solid = Sólido de mina/acopio
tri-recommended-weld-retry = Recomendado: soldar y reintentar
tri-reconstruct-help = Reconstruya una superficie de terreno triangulada desde una nube de puntos. El muestreador adaptativo dedica el presupuesto de vértices a las zonas más complejas y mantiene dispersas las áreas planas.
tri-reduce-budget-candidate-detail-if = Reduzca el presupuesto o el detalle candidato si su equipo tiene menos RAM.
tri-reference-topology-help = La topografía de referencia que define dónde se recorta la otra superficie.
tri-reject-reconstructed-triangle-edges = Rechaza las aristas de triángulos reconstruidos que superen esta distancia. Use 0 para no limitar la longitud.
tri-remove-inside-help = Elimina la superficie dentro del límite de la polilínea y conserva el resto.
tri-removes-topology-where-pit-shell = Elimina la topografía donde la envolvente de la mina excava por debajo para que esta rellene el hueco. La unión sigue la línea de contacto 3D real entre las superficies; se conserva la topografía bajo las partes de la envolvente que quedan sobre el terreno.
tri-result = Resultado
tri-save-two-entities = Guardar como dos entidades
tri-select = Seleccionar…
tri-share-source-points-keep-fractions = Proporción de puntos de origen a mantener. Se permiten fracciones como el 0,125%.
tri-slice-triangulation-z-range = Cortar triangulación por rango Z
tri-solution-generate-upper-surface = Solución: Generar la superficie superior
tri-surface-trim = Superficie que recortar
tri-target-surface-help = La superficie que se modificará; la topografía seleccionada permanece intacta.
common-percent-suffix = %
tri-topology = Topografía
tri-triangulation-failed = Triangulación fallida
tri-trim = Recortar
tri-trim-topology = Recortar a topografía
tri-uniform-grid = Cuadrícula uniforme
tri-up-target-point-count-points = Hasta { $target } de { $point_count } puntos se convertirán en vértices de superficie ({ $percent }%).
tri-use-full-surface-elevation-range = Utilice todo el rango de elevación superficial
tri-vertex-count = Número de vértices
tri-vertices-within-5-cm-xy = Los vértices separados hasta 5 cm en XY y Z compartirán una posición en esta triangulación. Esto puede desplazar localmente la superficie generada hasta 5 cm; las polilíneas de origen no cambian.
tri-weld-retry = Soldar y reintentar
tri-when-enabled-generate-contours-only = Cuando está activado, genera curvas de nivel solo entre las elevaciones mínima y máxima indicadas.

## Ui strings

ui-choose-offset-side = Seleccione el lado de desplazamiento
ui-choose-relimit-side = Seleccione el lado de reajuste de límite
ui-click-circle-centre = Haga clic en el centro del círculo
ui-click-closed-polyline-use-blast = Haga clic en una polilínea cerrada para usarla como contorno de voladura
ui-click-collar-add-edit-initiation = Haga clic en un brocal para añadir o editar un punto de iniciación
ui-click-first-point-slice-line = Haga clic en el primer punto de la línea de corte
ui-click-first-vertex = Haga clic en el primer vértice
ui-click-perimeter-point-type-radius = Haga clic en un punto del perímetro o escriba un radio
ui-click-second-point-slice-line = Haga clic en el segundo punto de la línea de corte
ui-click-second-vertex = Haga clic en el segundo vértice
ui-click-use-pointer-radius = o haga clic para usar el radio del puntero
ui-could-not-copy-text-browser = No se pudo copiar el texto al portapapeles del navegador: { $error }
ui-dip-horizontal-no-strike = { $dip } (horizontal, sin rumbo)
ui-distance-meters = { $distance } metros
ui-drag-ring-type-azimuth-dip = Arrastre un anillo o escriba un azimut y una inclinación
ui-each-hole-turns-about-its = cada barreno gira alrededor de su propio brocal
ui-enter-positive-decimal-radius = Ingrese un radio decimal positivo
ui-esc-cancels = Esc cancela
ui-no-delay-product-tie = No hay producto de retardo para conectar
ui-press-enter-use-typed-radius = Presione Entrar para usar el radio escrito
ui-right-click-delay-palette-heading = haga clic derecho en el encabezado de la paleta de retrasos para añadir uno
ui-select-designs = Seleccione los diseños
ui-select-drill-hole = Seleccione un barreno
ui-select-endpoint-join = Seleccione el punto final para unirse
ui-select-first-crest-toe-point = Seleccione el primer punto corona/pie
ui-select-item = Seleccione un elemento
ui-select-line-fuse = Seleccione una línea para fusionar
ui-select-line-polyline = Seleccione una línea o polilínea
ui-select-line-relimit = Seleccione la línea para reajustar el límite
ui-select-next-line-fuse = Seleccione la siguiente línea para fusionar
ui-select-opposite-berm-point = Seleccione el punto opuesto de berma
ui-select-point = Seleccione un punto
ui-select-polyline = Seleccione una polilínea
ui-select-polyline-open-line = Seleccione una polilínea o una línea abierta
ui-select-polyline-vertex = Seleccione un vértice de polilínea
ui-select-second-crest-toe-point = Seleccione el segundo punto corona/pie
ui-select-second-split-point = Seleccione el segundo punto de división
ui-select-split-point = Seleccione un punto dividido
ui-select-topologies = Seleccione topografías
ui-slice-view = Vista de corte
ui-strike-dip = { $strike }° de rumbo · { $dip }
ui-value-dip = { $value }° de buzamiento

## Viewport strings

viewport-all-total-categories-keep-their = Las { $total } categorías conservan su color; solo las primeras { $shown } se dibujan de forma diferenciada
viewport-axis-maximum = Máximo de { $axis }
viewport-axis-minimum = Mínimo de { $axis }
viewport-bar-blast-timeline-placeholder = Cronología de voladura [MARCADOR]
viewport-bar-burden-relief-heatmap-placeholder = Mapa térmico de alivio de piedra [MARCADOR]
viewport-bar-color = Color:
viewport-bar-contours-equal-time-placeholder = Curvas de nivel de tiempo igual [MARCADOR]
viewport-bar-disable-flying-mode = Desactivar modo de vuelo
viewport-bar-disable-x-ray-vision = Desactivar visión de rayos X
viewport-bar-drill-holes = Sondajes:
viewport-bar-enable-flying-mode = Activar modo de vuelo
viewport-bar-enable-x-ray-vision = Activar visión de rayos X
viewport-bar-exit-slice-view = Salir de vista de corte
viewport-bar-fill = Rellenar:
viewport-bar-fix-centre-rotation = Fijar centro de rotación
viewport-bar-hide-points = Ocultar puntos
viewport-bar-hide-rl-grid = Ocultar cuadrícula de cotas
viewport-bar-hide-wireframes = Ocultar mallas alámbricas
viewport-bar-hide-xy-grid = Ocultar cuadrícula XY
viewport-bar-release-centre-rotation = Liberar centro de rotación
viewport-bar-show-points = Mostrar puntos
viewport-bar-show-rl-grid = Mostrar cuadrícula de cotas
viewport-bar-show-wireframes = Mostrar mallas alámbricas
viewport-bar-show-xy-grid = Mostrar cuadrícula XY
viewport-bar-vertical-slice-view = Vista de corte vertical
viewport-blank = (en blanco)
viewport-choose-active-block-model-variable = Seleccione la variable activa del modelo de bloques
viewport-choose-variable = Elegir una variable
viewport-click-edit-color-right-click = Haga clic para editar el color; haga clic derecho para eliminar
viewport-click-type-boundary-s-value = Haga clic para escribir el valor de este límite
viewport-colour-mapping = Mapeo de colores
viewport-count-categories = { $count } categorías
viewport-count-category = { $count } categoría
viewport-double-click-add-boundary-here = Haga doble clic para agregar un límite aquí
viewport-drag-move-middle-click-toggles = Arrastre para mover · Clic central alterna ≤
viewport-drag-move-right-click-remove = Arrastre para mover · Clic derecho para quitar · Clic central alterna ≤
viewport-e = E
viewport-edit-category-colour = Editar el color de esta categoría
viewport-edit-colour-used-empty-values = Editar el color usado para valores vacíos
viewport-empty = (vacío)
viewport-empty-hidden = (vacío · oculto)
viewport-filter-variables = Filtrar variables
viewport-navigation-hint = Arrastre con el botón central para desplazar · Desplácese para hacer zoom
viewport-navigation-hint-detach = Arrastre con el botón central para desplazar · Desplácese para hacer zoom · Haga clic para desprender
viewport-n = N
viewport-no-data-variable = No hay datos para esta variable
viewport-no-matches = No hay coincidencias
viewport-no-usable-range = (sin intervalo utilizable)
viewport-rebuild-variable-s-colours-from = Regenerar los colores de esta variable a partir de sus datos
viewport-reset = Restablecer
viewport-restore-full-model-range = Restaurar el intervalo completo del modelo
