# Português. Entradas ausentes usam o catálogo inglês como fallback.
common-cancel = Cancelar
common-clear = Limpar
common-close = Fechar
common-color = Cor
common-fill = Preenchimento
common-set = Definir
status-language = Idioma
menu-file = Arquivo
menu-file-save-project = Salvar projeto
menu-file-save-project-as = Salvar projeto como...
menu-file-new-project = Novo projeto...
menu-file-open-project = Abrir projeto...
menu-file-open-recent = Abrir recente
menu-file-import = Importar...
menu-file-export = Exportar...
menu-file-about = Sobre { $app }...
menu-file-exit = Sair da aplicação
menu-view = Vista
ws-production = Produção
ws-drill-and-blast = Perfuração e desmonte
ws-geology = Geologia
ws-planning = Planeamento
dialog-rename-title = Renomear { $kind }
dialog-rename-field = Novo nome
dialog-rename-field-hint = Obrigatório
dialog-rename-submit = Renomear
dialog-delete-title = Eliminar { $kind }
dialog-delete-confirm = Eliminar «{ $name }» do projeto?
    Esta ação não pode ser anulada.
confirm-delete-product = Eliminar o produto «{ $name }» da paleta?
    Esta ação não pode ser anulada.
about-read-full-licence = Ler a licença completa ↗
about-source-code = Código-fonte
about-website = Site

## Completed canonical messages

menu-file-show-in-explorer = Mostrar no Explorer
menu-file-show-in-folder = Abrir pasta que contém o ficheiro
menu-file-export-viewport-image = Exportar Imagem da Janela de Visualização...
menu-file-export-engineering-drawing = Exportar Desenho de Engenharia...
ws-menubar-design = Desenho
ws-menubar-triangulation = Triangulação
ws-menubar-raster = Raster
ws-menubar-point-cloud = Nuvem de pontos
ws-menubar-block-model = Modelo de blocos
ws-menubar-drillholes = Furos de sondagem
ws-menubar-active-layer = Camada:
ws-menubar-design-insert-point = Inserir Ponto
ws-menubar-design-insert-point-at-intersection = Na interseção
ws-menubar-design-insert-point-at-elevation = Na altura
ws-menubar-design-move-to = Mover para
ws-menubar-design-create-triangulation = Criar triangulação
tri-create-title = Criar triangulação
tri-create-type-label = Tipo de triangulação
tri-create-type-help = A superfície aberta cria uma folha de estilo de terreno. O sólido cria uma malha totalmente fechada e requer entrada que pode formar um limite impermeável.
tri-create-output-name = Nome de saída
tri-create-output-name-help = Nome atribuído à triangulação gerada.
tri-create-output-name-hint = Nome da triangulação
tri-create-run = Triangular
tri-selection-selected = Selecionado { $summary }
tri-type-open-surface = Superfície
tri-type-solid-closed = Sólido
about-title = Sobre o { $app }
drill-hole-colour-title = Cor dos furos de sondagem: { $name }
drill-hole-colour-stop = Parada { $index }
properties-restore-defaults = Redefinir as configurações de { $heading } para os padrões
ui-selected-count = Selecionado { $count }
ui-selected-objects = { $count } objeto(s) selecionado(s)
ui-selected-polylines = { $count } polilinha(s) selecionada(s)
ui-invalid-axis-value = Digite um valor { $axis } válido.
ui-selection-spans = A seleção abrange de { $min } a { $max }.
confirm-delete-count = Você tem certeza de que deseja excluir os itens { $count } selecionados?
confirm-delete-layer = Eliminar a camada "{ $name }" e todos os objetos nela? Isto não pode ser desfeito.
plot-preview-pixels = { $width } × { $height } px em { $dpi } dpi
tri-estimated-memory = Memória máxima estimada ~ { $estimate }. { $detail }
block-grid-summary = Grade: { $x } × { $y } × { $z } = { $count } blocos
status-selected = Selecionado: { $count }
status-fps = FPS: { $fps }
status-clip = Recorte próximo/distante/Δ: { $near } / { $far } / { $delta } m

## Selection counts

tri-count-polylines =
    { $count ->
        [one] { $count } polilinha
       *[other] { $count } polilinhas
    }
tri-count-strings =
    { $count ->
        [one] { $count } linha
       *[other] { $count } linhas
    }
tri-count-points =
    { $count ->
        [one] { $count } ponto
       *[other] { $count } pontos
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

explorer-no-rasters = Nenhum raster
slice-viewport-gestures = arrastar botão do meio: deslocar · arrastar botão direito: orbitar · Shift+roda: caminhar · W/S: mover fatia · Q/E: rodar · Esc: sair

## Detalhes do ambiente de inicialização

## Diagnóstico de arranque do renderizador

color-aci = ACI
color-aci-value = ACI { $index }
color-index = Índice
color-rgb = RGB
color-opacity = Opacidade
color-edit = Clique para editar a cor
color-saturation-value = Saturação e brilho
color-hue = Matiz
asset-loading = A carregar dados do ativo
asset-unloading = A descarregar dados do ativo
asset-load-failed = Não foi possível carregar os dados do ativo
asset-unload-failed = Não foi possível descarregar os dados do ativo
preferences-title = Preferências
context-text-colour = Cor do texto
context-polylines = Polilinhas
context-points = Pontos
crs-unknown-ellipsoid = Modelo de terra não reconhecido «{ $name }» nesta definição de sistema de coordenadas.
crs-no-ellipsoid = Esta definição de sistema de coordenadas não indica que modelo de terra utiliza.
crs-unknown-code = EPSG:{ $code } não está no registo de sistemas de coordenadas.
crs-transform-failed = Não foi possível converter uma coordenada; o resultado não foi uma posição finita.
crs-no-datum-path = Não existe nenhuma transformação publicada disponível entre os referenciais de { $from } e { $to } (datums EPSG { $source } e { $target }). Converter mesmo assim seria incorreto por uma quantidade desconhecida, pelo que nada foi alterado.
crs-unknown-datum = O referencial de { $from } ou { $to } não pode ser identificado, e ambos usam modelos de terra diferentes. Converter entre eles seria incorreto por uma quantidade desconhecida.
ws-survey = Topografia
survey-count-designs = { $count } { $count ->
    [one] desenho
   *[other] desenhos
  }
survey-count-meshes = { $count } { $count ->
    [one] triangulação
   *[other] triangulações
  }
survey-count-models = { $count } { $count ->
    [one] modelo de blocos
   *[other] modelos de blocos
  }
survey-count-clouds = { $count } { $count ->
    [one] nuvem de pontos
   *[other] nuvens de pontos
  }
survey-count-holes = { $count } { $count ->
    [one] conjunto de sondagens
   *[other] conjuntos de sondagens
  }
survey-count-rasters = { $count } { $count ->
    [one] raster
   *[other] rasters
  }
survey-unsupported = Os rasters não podem ser convertidos por esta transformação. Não são selecionáveis na vista, pelo que nada de uma seleção é afetado.
survey-angle = Rotação em torno de Z (sentido anti-horário)
survey-scale = Fator de escala XYZ uniforme
survey-invalid-transform = As origens, o ângulo e as coordenadas resultantes devem ser finitos.
survey-invalid-scale = A escala deve ser um número positivo finito com recíproco finito.
survey-empty-selection = Selecione pelo menos um item suportado para transformar.
survey-unavailable = Um item selecionado está em falta ou não carregado. Carregue-o antes de transformar.
survey-wrong-project = Selecione desenhos apenas do projeto ativo.
survey-name-required = Introduza um nome para o sistema de coordenadas.
survey-working = A transformar dados selecionados…
survey-completed = { $items } convertidos no local. Anular restaura-os.
survey-failed = Falha na transformação: { $error }
survey-stale = Transformação descartada porque o projeto ativo ou os dados de origem mudaram. Selecione os dados de origem e tente novamente.
survey-coordinates-menu = Coordenadas
survey-definitions-action = Definições…
survey-transform-action = Transformar…
survey-definitions-title = Definições de Coordenadas
survey-transform-title = Transformar Coordenadas
survey-new-system = Novo Sistema de Coordenadas
survey-new-system-name = Sistema de coordenadas
survey-set-local = Definir como Sistema de Coordenadas da Mina
survey-delete-system = Eliminar Sistema de Coordenadas
survey-systems-empty = Sem sistemas de coordenadas
survey-system-section = Definição da malha da mina
survey-reference-note = O referencial em relação ao qual cada definição é escrita: as coordenadas que os seus dados já trazem ao serem importados. Não tem parâmetros próprios. Clique com o botão direito num sistema para o tornar o sistema de coordenadas da mina, ou no espaço vazio abaixo para definir um.
survey-system-name = Nome
survey-reference-system = Sistema de referência
survey-reference-origin = Ponto conhecido — coordenadas de referência
survey-system-origin = Mesmo ponto — coordenadas do sistema
survey-angle-help = No sentido anti-horário do X de referência para o Y de referência, visto de cima.
survey-scale-help = Escala XYZ uniforme do referencial para este sistema. Use 1 para preservar as dimensões.
survey-close = Fechar
survey-from = De
survey-to = Para
survey-transform-button = Transformar
survey-swap = Trocar
survey-drape-note = As imagens drapejadas são removidas das superfícies convertidas e têm de ser redrapejadas.
survey-needs-grid-block-model = Um modelo de blocos é uma grelha regular de células, e uma mudança de projeção ou referencial não mantém essa regularidade. Convertê-lo implicaria reamostrar cada célula numa nova grelha e perder os valores que contém, pelo que foi deixado inalterado.
survey-needs-grid-raster = Um raster é posicionado no mundo por um mapa afim, algo que uma mudança de projeção ou referencial não consegue preservar. Convertê-lo implicaria reamostrar a imagem, pelo que foi deixado inalterado.
survey-conversion-exact = Exata: apenas mudança de grelha, sem reprojeção.
survey-conversion-accuracy = Precisão indicada { $accuracy } m.
survey-kind = Tipo
survey-axis-names = Nomes dos eixos
survey-axis-help = Como este sistema chama os seus eixos, se não X, Y e Z — «E», «N», «RL» para uma malha de mina. Usado sempre que são mostradas coordenadas, mas apenas enquanto este for o sistema de coordenadas da mina. Nomeie todos os três ou nenhum.
survey-kind-registry-short = Sistema do registo
survey-kind-grid-short = Malha sobre outro sistema
survey-registry-search = Pesquisar
survey-registry-hint = Nome ou código EPSG, por ex. «mga zone 56»
survey-registry-none = Nada no registo corresponde a todas as palavras.
survey-parent = Definido em relação a
survey-parent-origin = Ponto conhecido — coordenadas do sistema-pai
survey-pick-registry = Pesquise o sistema e escolha-o nos resultados.
survey-pick-parent = Escolha o sistema em relação ao qual esta malha é definida.
survey-pick-system = Escolha um sistema
survey-pick-systems = Escolha o sistema de origem e o de destino da conversão.
survey-no-selection = Escolha um sistema de coordenadas à esquerda, ou clique com o botão direito para adicionar um.
survey-kind-grid = Malha sobre { $parent }
survey-system-in-use = «{ $name }» não pode ser eliminado: { $dependants } { $dependants ->
    [one] está
   *[other] estão
  } definidos em relação a ele. Redirecione-os para outro lado primeiro.
survey-system-cycle = «{ $name }» está definido em relação a si mesmo, diretamente ou através dos seus sistemas-pai.
survey-system-missing = Esse sistema de coordenadas já não existe. Selecione outra definição.
survey-same-system = Escolha sistemas de origem e destino diferentes.
survey-name-exists = Já existe um sistema de coordenadas com esse nome. Selecione-o para editar, ou escolha outro nome.

## About strings

about-copyright-c-2026-leo-timmins =
    Copyright (c) 2026 Leo Timmins, Lucas Timmins, and Incline Design contributors. Permission is hereby granted, free of charge, to any person obtaining a copy of this software to deal in it without restriction, subject to the conditions of the MIT License.

    Incline Design is provided "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, including but not limited to the warranties of MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE and NONINFRINGEMENT.
about-free-open-source-mine-design = Projeto de mina livre e de código aberto
about-licensed-under-mit-license = Licenciado sob a Licença MIT

## App strings

app-activated-browser-project-name = Projeto do navegador '{ $name }' ativado.
app-browser-project-deletion-failed-erro = Falha ao eliminar o projeto do navegador: { $error }
app-browser-project-no-longer-exists = Esse projeto do navegador não existe mais
app-browser-save-failed-error = Falha ao guardar no navegador: { $error }
app-could-not-activate-browser-project = Não foi possível ativar o projeto do navegador: { $error }
app-could-not-delete-browser-project = Não foi possível eliminar o projeto do navegador: { $error }
app-could-not-load-browser-project = Não foi possível carregar o projeto do navegador: { $error }
app-could-not-restore-browser-project = Não foi possível restaurar o projeto do navegador: { $error }
app-deleted-browser-project = Projeto do navegador eliminado
app-failed-create-window-error = Não foi possível criar a janela: { $error }
app-failed-create-window-icon-error = Não foi possível criar o ícone da janela: { $error }
app-failed-detach-top-down-preview = Não foi possível desacoplar a vista superior: { $error }
app-failed-initialize-graphics-error = Não foi possível inicializar os gráficos: { $error }
app-failed-load-browser-preferences-erro = Não foi possível carregar as preferências do navegador: { $error }
app-failed-load-config-file-error = Não foi possível carregar o ficheiro de configuração: { $error }
app-failed-load-session-file-error = Não foi possível carregar o ficheiro de sessão: { $error }
app-failed-rasterize-window-icon-error = Não foi possível rasterizar o ícone da janela: { $error }
app-failed-save-browser-session-error = Não foi possível guardar a sessão do navegador: { $error }
app-failed-save-session-error = Não foi possível guardar a sessão: { $error }
app-saved-name-browser-storage = '{ $name }' guardado no armazenamento do navegador

## Block strings

block-model-between = Entre
block-model-block-grid = Grade de blocos
block-model-block-size = Tamanho do bloco
block-model-choose-numeric-variable = Escolher uma variável numérica
block-model-choose-numeric-variables = Escolher variáveis numéricas
block-model-count-variables-selected = { $count } variáveis selecionadas
block-model-estimate-variables = Estimar variáveis
block-model-full-x-y-z-dimensions = Dimensões X, Y e Z completas de cada bloco. Blocos menores aumentam o detalhe, o tempo de cálculo e a memória usada.
block-model-grid-bounds-block-sizes-invalid = Os limites da grade ou os tamanhos dos blocos são inválidos.
block-model-lower-x-y-z-edges = Limites inferiores X, Y e Z do volume do modelo de blocos. Os centros começam meio bloco dentro destes limites.
block-model-maximum = Máximo
block-model-maximum-nearest-samples-used-each = Número máximo de amostras mais próximas usado por bloco. Valores menores são mais rápidos; valores maiores podem suavizar estimativas e aumentar o cálculo.
block-model-maximum-samples = Máximo de amostras
block-model-minimum = Mínimo
block-model-minimum-nearby-samples-required-esti = Número mínimo de amostras próximas para estimar um bloco. Blocos com menos amostras no raio de pesquisa ficam vazios.
block-model-minimum-samples = Mínimo de amostras
block-model-nugget = Efeito pepita
block-model-numeric-interval-fields-interpolate = Campos numéricos de intervalo a interpolar. Cada campo selecionado torna-se uma variável do modelo de blocos.
block-model-ordinary-kriging-estimates-numeric-d = A krigagem ordinária estima intervalos numéricos dos furos no centro de cada bloco usando um variograma esférico.
block-model-partial-sill = Patamar parcial
block-model-range-search-radius = Alcance / raio de busca
block-model-samples-farther-than-distance-exclud = São excluídas as amostras superiores a esta distância; a covariância atinge o zero nesta faixa.
block-model-select-all = Selecionar todos
block-model-spatially-correlated-variance-contri = Variância espacialmente correlacionada do modelo esférico. Juntamente com o efeito pepita, define a covariância à distância zero.
block-model-spherical-variogram-search = Variograma esférico e pesquisa
block-model-threshold = <= limiar
block-model-threshold-2 = >= limiar
block-model-threshold-min = Limiar / mín.
block-model-upper-x-y-z-extent = Extensão superior X, Y e Z a cobrir. O último bloco pode ultrapassá-la quando o intervalo não é múltiplo exato do tamanho do bloco.
block-model-variable = Variável
block-model-variance-effectively-zero-separation = Variância com separação praticamente nula causada por erro de medição ou variação abaixo da escala de amostragem. Use zero se não pretender efeito pepita.
block-model-volume-cache-block-volume-usage-feedback-readback = A leitura de retorno de uso do volume de blocos foi desconectada
block-model-volume-cache-block-volume-usage-feedback-readback-2 = Falha ao ler o retorno de uso do volume de blocos: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z

## Canvas strings

canvas-not-selectable-choose-closed-polylin = Não selecionável | Escolha uma polilinha fechada
canvas-polyline-layer-layer-count-vertices = Polilinha | Camada: { $layer } | { $count } vértices
canvas-surface-name = Superfície | { $name }
canvas-trimmed = Aparada

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = Talude e berma criados a partir do objeto { $object_id }
cmd-bezier-replaced-polyline-span-first-last = Trecho da polilinha { $first }→{ $last } substituído por { $count } pontos intermediários amostrados
cmd-bezier-vertices-first-last = Vértices { $first } a { $last }
cmd-block-model-block-model-loader-disconnected-path = O carregador do modelo de blocos foi desconectado para { $path }
cmd-block-model-block-model-path-has-count = O modelo de blocos { $path } tem { $count } variável(is) de um tipo não suportado que não poderá ser lido: { $names }
cmd-block-model-building-ore-mesh = A construir malha de minério…
cmd-block-model-could-not-create-block-model = Não foi possível criar o modelo de blocos: { $error }
cmd-block-model-could-not-decode-block-model = Não foi possível decodificar a variável de cor “{ $variable }” do modelo de blocos: { $error }
cmd-block-model-created-block-model-name-ordinary = Modelo de blocos “{ $name }” criado por krigagem ordinária
cmd-block-model-failed-load-block-model-error = Falha ao carregar o modelo de blocos: { $error }
cmd-block-model-generated-ore-mesh-from-block = Malha de minério gerada a partir do modelo de blocos '{ $name }'
cmd-block-model-imported-block-model-source-path = Fonte do modelo de blocos importada: { $path }
cmd-block-model-loaded-block-model-name-blocks = Modelo de blocos «{ $name }» carregado: { $blocks } blocos ({ $renderable } renderizáveis), grelha { $dimx }×{ $dimy }×{ $dimz }, { $variables } variáveis
cmd-block-model-loading-name = A carregar { $name }
cmd-block-model-loading-name-2 = A carregar { $name }…
cmd-chamfer-chamfered-corner-corner-radius-radiu = Canto { $corner } chanfrado com raio { $radius } e { $segments } segmentos
cmd-chamfer-radius-radius = Raio { $radius }
cmd-commands-clipped = Recortada
cmd-commands-command-failed-error = Falha no comando: { $error }
cmd-commands-select-one-more-objects-before = Selecione um ou mais objetos antes de definir { $axis }
cmd-commands-sliced = Seccionada
cmd-contours-contour-generation-failed-error = Falha ao gerar curvas de nível: { $error }
cmd-contours-contours-name-were-discarded-layer = Os contornos de «{ $name }» foram descartados: a camada «{ $layer_name }» já existe
cmd-contours-contours-name-were-discarded-project = As curvas de nível de “{ $name }” foram descartadas: o projeto foi fechado
cmd-contours-contours-name-were-discarded-selecte = As curvas de nível de “{ $name }” foram descartadas: a camada de saída selecionada foi eliminada
cmd-contours-generated-line-count-contour-polylin = Foram geradas { $line_count } polilinha(s) de contorno para a triangulação «{ $name }» na camada «{ $layer_name }»
cmd-creation-assembled-assembled-count-closed-bou = Foram montados { $assembled_count } anel(is) de limite fechado a partir de linhas abertas fragmentadas
cmd-creation-created-triangulation-from-boundary = Triangulação criada a partir de { $boundary_count } anel(is) de limite e { $constraint_count } restrição(ões) aberta(s), tipo de superfície { $surface_type }
cmd-creation-creating-triangulation = A criar triangulação…
cmd-creation-generate-upper-surface-ignored-count = Gerar superfície superior: foram ignorados { $count } segmento(s) de linha de quebra inferior em conflito; os objetos de origem não foram alterados
cmd-creation-ignored-rejected-non-polyline-degene = Foram ignorados { $rejected } objeto(s) não polilinha ou degenerados durante a triangulação
cmd-creation-weld-retry-moved-coarse-welded = Soldar e repetir: { $coarse_welded } vértice(s) movidos para posições partilhadas (até { $coarse_weld_tol } m); os objetos de origem não foram alterados
cmd-creation-welded-welded-breakline-vertex-verti = Soldados { $welded } vértice(s) de linha de quebra coincidentes dentro da tolerância
cmd-cuts-clipped-surface-name-polyline-mode = Superfície '{ $name }' recortada por polilinha ({ $mode })
cmd-cuts-clipping-surface-polyline = A recortar superfície por polilinha…
cmd-cuts-cut-topology-name-pit-shell = Topografia '{ $name }' cortada pela envolvente da cava
cmd-cuts-cut-triangulation-name-z-band = Triangulação '{ $name }' cortada pela faixa Z [{ $min }, { $max }]
cmd-cuts-cutting-topology-pit-shell = A cortar topografia pela envolvente da cava…
cmd-cuts-cutting-triangulation-z = A cortar triangulação por Z…
cmd-cuts-ignored-count-vertical-degenerate-re = Foram ignoradas { $count } face(s) verticais ou degeneradas da topologia de referência sem área XY
cmd-cuts-site-skipped-constraint-from-x = { $site }: foi ignorada a restrição ({ $from_x }, { $from_y }) → ({ $to_x }, { $to_y }) que o triangulador não conseguiu dividir
cmd-cuts-site-skipped-skipped-near-degenerate = { $site }: foram ignoradas { $skipped } aresta(s) de restrição quase degeneradas; o limite do corte pode desviar-se ligeiramente perto delas
cmd-cuts-trimmed-surface-surface-topology-top = Superfície '{ $surface }' aparada pela topografia '{ $topology }' ({ $mode })
cmd-cuts-trimming-surface-topology = A aparar superfície pela topografia…
cmd-drape-draped-intersected-vertices-changed = { $intersected } vértices projetados; { $changed } mudaram de elevação
cmd-drape-none-selected-design-vertices-inters = Nenhum dos vértices de desenho selecionados intersecta as topografias selecionadas
cmd-drape-objects-changed-object-s-changed = { $objects } objeto(s) alterado(s) · { $changed } de { $intersected } vértices de interseção movidos
cmd-drape-select-one-more-design-objects = Selecione um ou mais objetos de desenho para projetar
cmd-drape-select-one-more-topologies-drape = Selecione uma ou mais topografias sobre as quais projetar
cmd-drape-selected-topologies-no-longer-loaded = As topografias selecionadas já não estão carregadas
cmd-drill-hole-drill-pattern-too-large-contains = O padrão de perfuração é demasiado grande ou contém coordenadas de boca inválidas
cmd-drill-hole-enter-name-drill-pattern = Introduza um nome para o padrão de perfuração
cmd-drill-hole-failed-load-drillholes-error = Falha ao carregar os furos de sondagem: { $error }
cmd-drill-hole-hole-depth-must-greater-than = A profundidade do furo deve ser maior que zero
cmd-drill-hole-hole-diameter-must-greater-than = O diâmetro do furo deve ser maior que zero
cmd-drill-hole-loaded-drillhole-dataset-name-holes = Conjunto de furos «{ $name }» carregado: { $holes } furos, { $fields } campos de cor
cmd-drill-hole-pattern-contains-no-holes = O padrão não contém furos
cmd-explode-count-line-s = { $count } linha(s)
cmd-explode-explode-polyline = Explodir polilinha
cmd-explode-exploded-polyline-into-count-line = Polilinha explodida em { $count } segmentos
cmd-file-block-model-csv-encoding-failed = Falha na codificação CSV do modelo de blocos: { $error }
cmd-file-block-model-csv-export-failed = Falha ao exportar o CSV do modelo de blocos: { $error }
cmd-file-browser-recovery-files-unavailable-s = Os ficheiros de recuperação do navegador não estão disponíveis; os projetos guardados permanecem no IndexedDB
cmd-file-closed-project-runtime-id-runtime = Projeto fechado com ID de execução { $runtime_id }
cmd-file-could-not-create-new-project = Não foi possível criar um novo projeto: { $error }
cmd-file-could-not-finish-pending-project = Não foi possível concluir a ação pendente do projeto: { $error }
cmd-file-could-not-finish-saving-before = Não foi possível concluir a gravação antes de sair: { $error }
cmd-file-could-not-open-browser-project = Não foi possível abrir o projeto do navegador: { $error }
cmd-file-could-not-open-path-error = Não foi possível abrir { $path }: { $error }
cmd-file-could-not-read-selected-file = Não foi possível ler o ficheiro selecionado: { $error }
cmd-file-could-not-reload-layer-from = Não foi possível recarregar a camada do disco: { $error }
cmd-file-could-not-reload-project-from = Não foi possível recarregar o projeto do disco: { $error }
cmd-file-could-not-remove-browser-project = Não foi possível remover o projeto do navegador: { $error }
cmd-file-could-not-restore-layer-from = Não foi possível restaurar a camada do projeto: { $error }
cmd-file-could-not-snapshot-dirty-project = Não foi possível criar uma captura do projeto alterado para recuperação: { $error }
cmd-file-could-not-start-browser-export = Não foi possível iniciar a exportação no navegador: { $error }
cmd-file-could-not-write-recovery-copies = Não foi possível gravar as cópias de recuperação: { $error }
cmd-file-created-new-browser-project = Novo projeto do navegador criado
cmd-file-created-new-project = Novo projeto criado
cmd-file-description-download-failed-error = Falha na transferência de { $description }: { $error }
cmd-file-discard-was-cancelled-because-projec = O descarte foi cancelado porque o projeto mudou durante o recarregamento do OMF
cmd-file-discarded-changes-layer-target-name = Alterações à camada “{ $target_name }” descartadas
cmd-file-discarded-changes-reloaded-path = Alterações descartadas: { $path } recarregado
cmd-file-downloaded-description-file-name = { $description } transferido: { $file_name }
cmd-file-dxf-download-encoding-failed-error = Falha na codificação da transferência DXF: { $error }
cmd-file-dxf-import-failed-error = Falha ao importar DXF: { $error }
cmd-file-encoding-block-model-csv-download = A codificar a transferência do CSV do modelo de blocos…
cmd-file-encoding-dxf-download = A codificar a transferência DXF…
cmd-file-encoding-triangulation-download = A codificar a transferência da triangulação…
cmd-file-exit-deferred-until-background-expor = Saída adiada até terminarem as exportações em segundo plano
cmd-file-exit-requested-no-unsaved-changes = Saída solicitada sem alterações por guardar
cmd-file-exported-block-model-csv-path = CSV do modelo de blocos exportado para { $path }
cmd-file-exported-description-dxf-path = { $description } exportado para DXF: { $path }
cmd-file-exported-triangulation-name-path = Triangulação “{ $name }” exportada para { $path }
cmd-file-exporting-name = A exportar { $name }…
cmd-file-exporting-triangulation-name-path = A exportar a triangulação “{ $name }” para { $path }
cmd-file-fatal-renderer-failure-reason = Falha fatal do renderizador: { $reason }
cmd-file-file-dialog-action-failed-msg = Falha na ação da caixa de diálogo de ficheiros: { $msg }
cmd-file-imported-added-object-s-from = Importados { $added } objeto(s) de { $name }
cmd-file-imported-total-dxf-object-s = Importados { $total } objeto(s) DXF
cmd-file-layer-discard-was-cancelled-because = O descarte da camada foi cancelado porque o projeto mudou durante o recarregamento
cmd-file-no-recovery-directory-available-erro = Nenhum diretório de recuperação disponível: { $error }
cmd-file-no-unsaved-project-content-nothing = Não há conteúdo de projeto por guardar; nada para recuperar
cmd-file-parsing-browser-dxf-import = A analisar importação DXF do navegador…
cmd-file-parsing-dxf-import = A analisar importação DXF…
cmd-file-project-will-close-after-its = O projeto será fechado quando a gravação atual terminar
cmd-file-project-will-close-after-its-2 = O projeto será fechado quando a gravação atual terminar
cmd-file-queued-count-triangulation-file-s = { $count } ficheiro(s) de triangulação colocado(s) na fila de importação
cmd-file-recovery-copies-path-reopen-them = As cópias de recuperação estão em { $path }; reabra-as após reiniciar
cmd-file-recovery-copy-failed-error = Falha na cópia de recuperação: { $error }
cmd-file-recovery-copy-failed-failure = Falha na cópia de recuperação: { $failure }
cmd-file-recovery-copy-written-path = Cópia de recuperação gravada: { $path }
cmd-file-reverting-layer = A reverter camada…
cmd-file-reverting-project = A reverter projeto…
cmd-file-save-failed-message = Falha ao guardar: { $message }
cmd-file-save-worker-ended-without-result = O processo de gravação terminou sem resultado
cmd-file-saved-project-path = Projeto guardado como: { $path }
cmd-file-saved-project-path-2 = Projeto guardado: { $path }
cmd-file-selected-block-model-no-longer = O modelo de blocos selecionado já não está carregado
cmd-file-switching-project = A alternar projeto…
cmd-file-triangulation-download-encoding-fail = Falha na codificação da transferência da triangulação: { $error }
cmd-file-user-chose-exit-without-saving = O utilizador optou por sair sem guardar
cmd-file-user-requested-exit-project-export = O utilizador pediu para sair (é necessário exportar o projeto ou confirmar o trabalho não guardado)
cmd-file-viewport = Vista
cmd-file-wait-current-project-save-finish = Aguarde que a gravação do projeto atual termine
cmd-file-wait-current-project-switch-finish = Aguarde que a mudança do projeto atual termine
cmd-file-wait-project-operation-finish-before = Aguarde que a operação do projeto termine antes de descartar alterações
cmd-file-wait-project-revert-finish-before = Aguarde que a reversão do projeto termine antes de guardar
cmd-fuse-closed-polyline = polilinha fechada
cmd-fuse-count-source-line-s = { $count } linha(s) de origem
cmd-fuse-created-shape-object-id-vertices = { $shape } { $object_id } criado com { $vertices } vértices a partir de { $sources } linha(s) de origem
cmd-fuse-fuse-click-did-not-hit = Fundir: o clique não atingiu nenhum objeto (nada sob o cursor)
cmd-fuse-fuse-click-was-not-close = Fundir: o clique não ficou suficientemente próximo de nenhuma extremidade da linha selecionada
cmd-fuse-fuse-clicked-object-object-id = Fundir: o objeto { $object_id } é uma polilinha fechada; só é possível fundir polilinhas abertas
cmd-fuse-fuse-clicked-object-object-id-2 = Fundir: o objeto { $object_id } não é uma polilinha aberta (é { $kind })
cmd-fuse-fuse-clicked-object-object-id-3 = Fundir: o objeto clicado { $object_id } já não existe
cmd-fuse-fuse-clicked-polyline-object-id = Fundir: a polilinha { $object_id } tem apenas { $count } vértice(s); são necessários pelo menos 2
cmd-fuse-fuse-endpoint-marker-marker-index = Fundir: o marcador de extremidade { $marker_index } já não existe
cmd-fuse-fuse-line-needs-least-3 = Fundir: a linha precisa de pelo menos 3 vértices distintos para fechar como polilinha (tem { $count })
cmd-fuse-fuse-lines = Fundir linhas
cmd-fuse-fuse-need-least-2-segments = Fundir: são necessários pelo menos 2 segmentos para concluir (há { $count })
cmd-fuse-fuse-no-active-layer-place = Fundir: não há camada ativa onde colocar a linha fundida
cmd-fuse-fuse-no-active-project-cannot = Fundir: não há projeto ativo; não é possível concluir
cmd-fuse-fuse-no-source-line-close = Fundir: não há linha de origem para fechar numa polilinha
cmd-fuse-fuse-object-awaiting-id-no = Fundir: o objeto { $awaiting_id } já não é uma polilinha válida
cmd-fuse-fuse-object-object-id-already = Fundir: o objeto { $object_id } já faz parte da cadeia; selecione outra linha
cmd-fuse-fuse-result-has-too-few = Fundir: o resultado tem poucos vértices ({ $count }); operação cancelada
cmd-fuse-fuse-segment-object-object-id = Fundir: o objeto de segmento { $object_id } já não é uma polilinha válida; operação cancelada
cmd-fuse-fuse-source-object-object-id = Fundir: o objeto de origem { $object_id } já não é uma polilinha aberta válida
cmd-fuse-fuse-source-object-object-id-2 = Fundir: o objeto de origem { $object_id } já não existe
cmd-fuse-open-polyline = polilinha aberta
cmd-include-include-failed-message = Falha ao incluir: { $message }
cmd-include-included-solid-shape-name-topology = Sólido «{ $shape_name }» incluído na topologia «{ $topology_name }» ({ $retained } faces mantidas, { $skipped } faces de fecho ignoradas)
cmd-include-including-pit-stockpile-solid = A incluir sólido de cava/pilha…
cmd-insert-point-count-operation-point-s = { $count } ponto(s) de { $operation }
cmd-insert-point-insert-point-elevation-requires-fini = Inserir ponto na elevação requer uma elevação finita
cmd-insert-point-insert-points = Inserir pontos
cmd-insert-point-inserted-count-operation-point-s = { $count } ponto(s) de { $operation } inserido(s)
cmd-insert-point-intersection = Interseção
cmd-insert-point-no-new-operation-points-were = Não foram encontrados novos pontos de { $operation }
cmd-insert-point-select-least-two-polylines-before = Selecione pelo menos duas polilinhas antes de inserir pontos de interseção
cmd-insert-point-select-one-more-polylines-before = Selecione uma ou mais polilinhas antes de inserir um ponto numa elevação
cmd-layer-created-layer-name = Camada “{ $name }” criada
cmd-layer-deleted-layer-layer-id-all = Camada { $layer_id } eliminada (e todos os objetos nela)
cmd-layer-duplicated-layer-duplicate-name = Camada “{ $duplicate_name }” duplicada
cmd-layer-locked = Bloqueado
cmd-layer-name-copy = cópia de { $name }
cmd-layer-selected-count-object-s-layer = Foram selecionados { $count } objeto(s) na camada { $layer_id }
cmd-layer-state-layer-name = Camada “{ $name }” { $state }
cmd-layer-unlocked = Desbloqueado
cmd-move-tool-applied-move-delta-delta-count = Deslocamento aplicado ({ $delta }) a { $count } boca(s) de furo
cmd-move-tool-applied-move-delta-delta-count-2 = Deslocamento ({ $delta }) aplicado a { $count } objeto(s)
cmd-move-tool-count-hole-s = { $count } furo(s)
cmd-object-edit-edited-kind = { $kind } editado
cmd-object-edit-edited-kind-count-vertices = { $kind } editado ({ $count } vértices)
cmd-object-edit-no-changes-apply = Sem alterações a aplicar
cmd-object-edit-object-changed-since-editor-opened = Este objeto mudou desde que o editor foi aberto; reabra-o para editar a versão atual
cmd-object-edit-object-edit-target-changed-discardin = O objeto em edição mudou; a edição foi descartada
cmd-object-edit-object-no-longer-exists-document = Esse objeto já não existe no documento
cmd-object-edit-select-single-design-object-edit = Selecione um único objeto de desenho para editar
cmd-object-edit-unassigned = Não atribuído
cmd-offset-create-offset = Criar deslocamento
cmd-offset-created-offset-count-object-s = Deslocamento de { $count } objeto(s) criado
cmd-offset-offset-distance-must-greater-than = A distância de deslocamento deve ser maior que zero
cmd-omf-could-not-open-project-source = Não foi possível abrir o projeto { $source_name }: { $error }
cmd-omf-create-open-project-before-merging = Crie ou abra um projeto antes de combinar dados
cmd-omf-encoding-project = A codificar projeto…
cmd-omf-exported-project-path = Projeto exportado para { $path }
cmd-omf-imported-project-project-name-from = Projeto «{ $project_name }» importado de { $source_name }: { $count } conjunto(s) de dados de nível superior
cmd-omf-importing-project = A importar projeto…
cmd-omf-omf-export-failed-error = Falha ao exportar OMF: { $error }
cmd-omf-omf-import-failed-error = Falha ao importar OMF: { $error }
cmd-omf-opened-project-project-name-from = Projeto «{ $project_name }» aberto a partir de { $source_name }
cmd-omf-project-source-name-contains-no = O projeto “{ $source_name }” não contém elementos de dados compatíveis
cmd-omf-source-name-applied-project-origin = { $source_name }: a origem do projeto { $origin } foi aplicada antes da combinação
cmd-omf-source-name-coordinate-reference-sys = { $source_name }: o sistema de referência «{ $source_crs }» difere do SRC do projeto «{ $target_crs }»; as coordenadas foram combinadas sem reprojeção
cmd-omf-source-name-units-source-units = { $source_name }: as unidades «{ $source_units }» diferem das unidades do projeto «{ $target_units }»; as coordenadas foram combinadas sem conversão
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = Não há dados abertos do Incline Design para exportar
cmd-placement-2-vertices = 2 vértices
cmd-placement-count-vertices = { $count } vértices
cmd-placement-created-circle-radius-radius-m = Círculo criado com raio { $radius } m
cmd-placement-created-closed-polyline-count-vertic = Polilinha fechada criada com { $count } vértices
cmd-placement-created-line-segment-2-vertices = Segmento de linha com 2 vértices criado
cmd-placement-created-open-polyline-count-vertices = Polilinha aberta criada com { $count } vértices
cmd-placement-placed-point-x-y-z = Ponto posicionado em { $x }, { $y }, { $z }
cmd-placement-radius-radius-m = Raio { $radius } m
cmd-plot-composing-engineering-drawing = A compor desenho de engenharia…
cmd-plot-could-not-write-engineering-drawing = Não foi possível gravar o desenho de engenharia: { $error }
cmd-plot-drawing-scale-fitted-visible-data = Escala do desenho ajustada aos dados visíveis: 1:{ $scale }
cmd-plot-plot = Planta
cmd-plot-saved-engineering-drawing-descriptio = Desenho de engenharia guardado: { $description } ({ $width } × { $height } px a { $dpi } ppp)
cmd-point-cloud-failed-load-point-cloud-error = Falha ao carregar a nuvem de pontos: { $error }
cmd-point-cloud-loaded-point-cloud-name-count = Nuvem de pontos { $name } carregada ({ $count } pontos)
cmd-point-cloud-point-cloud-loader-disconnected-path = O carregador da nuvem de pontos foi desconectado para { $path }
cmd-point-cloud-tin-max-edge-disabled = (aresta máxima desativada)
cmd-point-cloud-tin-max-edge-max-edge = (aresta máxima { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = Falha no TIN da nuvem de pontos: { $error }
cmd-point-cloud-tin-terrain-tin-spatially-subsampled-sam = TIN do terreno: amostrados espacialmente { $sampled } de { $total } pontos
cmd-point-cloud-tin-terrain-tin-triangulated-vertex-coun = TIN do terreno: { $vertex_count } pontos XY únicos triangulados em { $face_count } faces{ $suffix }
cmd-products-added-product-delay-ms-ms = Produto adicionado: { $delay_ms } ms { $name }
cmd-products-deleted-product-delay-ms-ms = Produto eliminado: { $delay_ms } ms { $name }
cmd-products-failed-save-products-error = Falha ao guardar os produtos: { $error }
cmd-products-product-no-longer-palette = Esse produto já não está na paleta
cmd-raster-draped-raster-raster-over-triangulat = Raster { $raster } projetado sobre a triangulação { $triangulation } (extensões sobrepostas)
cmd-raster-failed-load-raster-name-error = Falha ao carregar o raster { $name }: { $error }
cmd-raster-failed-load-raster-path-error = Falha ao carregar o raster { $path }: { $error }
cmd-raster-loaded-raster-name-via-driver = Raster { $name } carregado através de { $driver } ({ $srcx }×{ $srcy }, pré-visualização { $prevx }×{ $prevy })
cmd-raster-no-loaded-triangulation-overlaps-ext = Nenhuma triangulação carregada se sobrepõe à extensão de { $name }
cmd-raster-raster-loader-disconnected-path = O carregador de raster foi desconectado para { $path }
cmd-raster-undraped-rasters-from-count-triangul = Rasters removidos de { $count } triangulação(ões)
cmd-relimit-relimit-click-did-not-hit = Redelimitar: o clique não atingiu nenhum objeto (nada sob o cursor)
cmd-relimit-relimit-click-ignored-tool-not = Redelimitar: clique ignorado; a ferramenta não aguarda a seleção de um alvo
cmd-relimit-relimit-clicked-source-line-itself = Redelimitar: clicou na própria linha de origem; selecione outra linha
cmd-relimit-relimit-no-source-line-set = Redelimitar: não foi definida uma linha de origem; seleção cancelada
cmd-relimit-relimited-line-source-id-selected = Linha { $source_id } relimitada ao alvo selecionado
cmd-relimit-resized-line-source-id-using = Linha { $source_id } redimensionada usando o modo { $mode } e o valor { $value }
cmd-rename-item-no-longer-belongs-active = Esse item já não pertence ao projeto ativo
cmd-rename-renamed-before-name = “{ $before }” renomeado para “{ $name }”
cmd-rename-renamed-before-name-requested-alread = «{ $before }» foi renomeado para «{ $name }» («{ $requested }» já está em uso)
cmd-rotate-collar-turned-count-drillhole-collar-s = Foram rodadas { $count } boca(s) de furo { $rotation }
cmd-section-verb-count-item-s-section = { $verb } { $count } item(ns) em { $section }
cmd-selection-delete-vertex = Eliminar vértice
cmd-selection-deleted-count-selected-object-s = { $count } objeto(s) selecionado(s) eliminado(s)
cmd-selection-deleted-vertex-vertex-from-polyline = Vértice { $vertex } eliminado da polilinha { $object_id }
cmd-selection-duplicate-selection = Duplicar seleção
cmd-selection-duplicated-count-object-s = { $count } objeto(s) duplicado(s)
cmd-session-created-triangulation-name-vertex-co = Triangulação «{ $name }» criada ({ $vertex_count } vértices, { $face_count } faces) a partir do tipo de superfície { $surface_type }
cmd-session-deleted-triangulation-name-from-proj = Triangulação “{ $name }” eliminada do projeto
cmd-session-failed-load-triangulation-error = Falha ao carregar a triangulação: { $error }
cmd-session-failed-load-triangulation-message = Falha ao carregar a triangulação: { $message }
cmd-session-loaded-triangulation-name-path-verte = Triangulação «{ $name }» carregada ({ $path }, { $vertex_count } vértices, { $face_count } faces)
cmd-session-set-triangulation-tri-id-color = Cor da triangulação { $tri_id } definida como { $color }
cmd-session-triangulation-load-path-ended-withou = O carregamento da triangulação de { $path } terminou sem resultado
cmd-session-triangulation-operation-failed-messa = Falha na operação de triangulação: { $message }
cmd-session-unloaded-triangulation-name = Triangulação “{ $name }” descarregada
cmd-slice-entered-slice-view-cx-cy = Vista de corte iniciada em { $cx }, { $cy }, { $cz } ao longo de { $dx }, { $dy } (linha de { $length } m)
cmd-slice-exited-slice-view = Saiu da vista de corte
cmd-slice-reset-section-view-fit-extents = Repor a vista em secção (ajustar à extensão)
cmd-slice-set-section-grid-enabled = Grelha da secção ativada = { $enabled }
cmd-split-created-2-open-polylines = 2 polilinhas abertas criadas
cmd-split-split-line = Dividir linha
cmd-split-split-points-choose-interior-vertex = Dividir nos pontos: escolha um vértice interior da linha aberta
cmd-split-split-points-choose-two-non = Dividir nos pontos: escolha dois vértices não adjacentes da polilinha
cmd-split-split-source-polyline-into-two = Polilinha de origem dividida em duas polilinhas abertas
cmd-text-finished-text-edit-object-object = Edição de texto concluída no objeto { $object_id }
cmd-text-updated-text-object-object-id = Texto atualizado no objeto { $object_id }
cmd-view-centre-rotation-not-available-flying = O centro de rotação não está disponível no modo de voo
cmd-view-fixed-centre-rotation-x-y = Centro de rotação fixado em { $x }, { $y }, { $z }
cmd-view-no-point-under-cursor-fix = Nenhum ponto sob o cursor onde fixar o centro de rotação
cmd-view-released-centre-rotation = Centro de rotação libertado
cmd-view-reset-view-fit-extents = Repor vista (ajustar às extensões)
cmd-view-set-topology-wireframes-enabled = Estruturas de arame da topografia = { $enabled }
cmd-view-set-view-points-enabled = Pontos da vista = { $enabled }
cmd-view-set-xy-grid-enabled = Grelha XY ativada = { $enabled }
cmd-view-zoom-extents-preserving-angle = Zoom para as extensões (preservar ângulo)

## Common strings

common-add-product = Adicionar Produto
common-background = Fundo
common-block-model = Modelo de blocos
common-block-models = Modelos de blocos
common-cancelled = Cancelado
common-chamfer = Chanfro
common-choose = Escolha...
common-circle = Círculo
common-click-point-fix-centre-rotation = Clique num ponto para fixar o centro de rotação
common-clip-surface-polyline = Recortar superfície por polilinha...
common-closed = Fechado
common-colour = Cor
common-confirm-omf-rewrite = Confirmar reescrita do OMF
common-could-not-replace-current-project = Não foi possível substituir o projeto atual: { $error }
common-count-object-s = { $count } objeto(s)
common-create = Criar
common-create-batter-berm = Criar bancada e berma
common-create-bezier-curve = Criar uma curva de Bezier
common-create-block-model = Criar modelo de blocos
common-create-block-model-2 = Criar modelo de blocos...
common-create-circle = Criar um círculo
common-create-drill-pattern = Criar padrão de perfuração
common-create-layer = Criar camada
common-create-line = Criar linha
common-create-ore-triangulation = Criar triangulação de minério
common-create-ore-triangulation-2 = Criar triangulação de minério...
common-create-point = Criar ponto
common-create-polyline = Criar polilinha
common-create-triangulation = Criar triangulação...
common-crosses = Cruzes
common-cut = Cortar
common-cut-topology-pit-shell = Cortar topografia com o invólucro da cava...
common-delete-layer = Eliminar a camada
common-delete-product = Eliminar o produto
common-delete-selection = Eliminar a seleção
common-designs = Desenhos
common-discard-layer-changes = Descartar alterações da camada
common-down = Para baixo
common-drape-topology = Drapear a topografia
common-easting = Este
common-edit-object = Editar objeto
common-edit-text = Edição de texto
common-elevation = Elevação
common-exit-without-saving = Sair sem salvar
common-export-engineering-drawing = Exportar Desenho de Engenharia
common-filter = Filtro
common-fly-mode = Modo de voo
common-generate-contour-lines = Gerar linhas de curva de nível...
common-hide-all = Ocultar tudo
common-hide-selection = Ocultar seleção
common-ignore = Ignorar
common-import-csv-block-model = Importar CSV modelo de blocos
common-import-dxf = Importar DXF
common-incline-design-project = Projeto do Incline Design
common-layer = Camada
common-legend = Legenda
common-line = Linha
common-line-weight = Peso de linha
common-lock-all = Bloquear tudo
common-lock-selection = Bloquear seleção
common-m = m
common-max = Máx.
common-merge-shell-into-topology = Combinar invólucro com a topografia
common-merge-shell-into-topology-2 = Combinar invólucro com a topografia...
common-move-collar = Mover boca do furo
common-move-design = Mover desenho
common-move-selection = Mover seleção
common-new-product = Novo produto
common-no-block-models = Nenhum modelo de blocos
common-no-design-layers = Nenhuma camada de desenho
common-no-drill-holes = Nenhum furo de sondagem
common-no-file-chosen = Nenhum ficheiro escolhido
common-no-open-project = Nenhum projeto aberto
common-no-point-clouds = Nenhuma nuvem de pontos
common-no-triangulations = Nenhuma triangulação
common-none = Nenhum
common-northing = Norte
common-offset = Compensação
common-open = Abrir
common-orientation = Orientação
common-point = Ponto
common-point-cloud = Nuvem de pontos
common-point-clouds = Nuvens de pontos
common-polyline = Polilinha
common-polyline-layer = Polilinha em '{ $layer }'
common-project = Projeto
common-rasters = Rasters
common-redo = Refazer
common-relimit-line = Linha de redefinição de limite
common-remove-project = Remover Projeto
common-reset-view = Redefinir Vista
common-reveal-all = Revelar tudo
common-reveal-finder = Revelar no Finder
common-rotate-collar = Rodar boca do furo
common-save-exit = Salvar e sair
common-scale-bar = Barra de escala
common-set-initiation-point = Definir ponto de iniciação
common-shape = Forma
common-shell = Com envolvente
common-slashes = Traços
common-slice = Fatia
common-slice-triangulation-z-range = Fatiar triangulação por intervalo Z...
common-surface-contours = Curvas de nível da superfície
common-text = Texto
common-text-2 = °
common-tie-holes = Ligar furos
common-triangulations = Triangulações
common-trim-topology = Aparar até a topografia...
common-undo = Desfazer
common-undrape-all = Remover todo o drapeamento
common-uniform-white = Branco uniforme
common-unlock-all = Desbloquear tudo
common-untitled = Sem título
common-up = Para cima
common-vertical-exaggeration = Exagero vertical
common-x = x
common-zoom-extents = Zoom para extensão

## Confirmations strings

confirmations-close-project-unsaved-changes = Fechar projeto: alterações por guardar
confirmations-close-without-saving = Fechar sem salvar
confirmations-delete = Eliminar
confirmations-delete-objects = Eliminar Objetos
confirmations-discard = Descartar
confirmations-discard-all-unsaved-changes-layer =
    Descartar todas as alterações não guardadas na camada «{ $name }»?
    A camada guardada será recarregada do disco e as alterações noutras camadas serão mantidas. Esta ação não pode ser anulada.
confirmations-discard-all-unsaved-changes-name =
    Descartar todas as alterações não guardadas em «{ $name }»?
    A última versão guardada será recarregada do disco. Esta ação não pode ser anulada.
confirmations-discard-changes = Descartar alterações
confirmations-exit-unsaved-changes = Sair: alterações por guardar
confirmations-incline-design-cannot-reproduce-all = O Incline Design não consegue reproduzir todo o conteúdo do OMF original. Ao guardar, será omitido o seguinte:
confirmations-product = Produto
confirmations-project = este projeto
confirmations-remove-name-delete-its-browser = Remover “{ $name }” e eliminar a cópia guardada no navegador? As alterações não guardadas serão perdidas.
confirmations-remove-project-unsaved-changes = Remover projeto: alterações por guardar
confirmations-remove-without-saving = Remover sem salvar
confirmations-replace-project-unsaved-changes = Substituir Projeto: Alterações Não Guardadas
confirmations-save = Salvar
confirmations-save-anyway = Salvar de qualquer maneira
confirmations-save-changes-current-project-before = Salvar alterações no projeto atual antes de substituí-lo?
confirmations-save-changes-name-before-closing = Guardar as alterações a “{ $name }” antes de o fechar?
confirmations-save-changes-name-before-removing = Guardar as alterações a “{ $name }” antes de o remover do Incline Design?
confirmations-save-close = Salvar e fechar
confirmations-save-modified-project-before-exiting = Salvar o projeto modificado antes de sair?
confirmations-save-modified-project-browser-storag = Salvar o projeto modificado no armazenamento do navegador antes de sair?
confirmations-save-remove = Salvar e remover

## Console strings

console-copy-all = Copiar tudo
console-copy-message = Copiar mensagem
console-error = ERRO
console-info = INFORMAÇÃO
console-no-console-activity-yet = Ainda não há atividade na consola
console-pending = PENDENTE
console-progress-summary = Em curso · { $summary }
console-success = SUCESSO
console-warn = AVISO

## Csv strings

csv-block-model-category = Categoria
csv-block-model-value = Valor

## Drill strings

drill-hole-add-stop = Adicionar parada
drill-hole-all-rendered-intervals-opaque-white = Todos os intervalos são brancos opacos.
drill-hole-burden-spacing-must-greater-than = O afastamento e o espaçamento devem ser maiores que zero
drill-hole-choose-valid-closed-polyline = Escolha uma polilinha fechada válida
drill-hole-colour-scale = Escala de cores
drill-hole-field = Campo
drill-hole-grayscale = Escala de cinzentos
drill-hole-green-yellow-red = Verde–amarelo–vermelho
drill-hole-heat = Calor
drill-hole-no-holes-fit-inside-boundary = Nenhum furo cabe dentro deste limite com o afastamento e o espaçamento atuais
drill-hole-pattern-exceeds-maximum-maximum-hole = O padrão excede o máximo de { $maximum } furos; aumente o afastamento ou o espaçamento
drill-hole-preset = Predefinição
drill-hole-px = px
drill-hole-rainbow = Arco-íris
drill-hole-reset-preset = Redefinir predefinição
drill-hole-rotation-offsets-must-contain-valid = A rotação e os deslocamentos devem conter números válidos
drill-hole-selected-polyline-has-no-usable = A polilinha selecionada não tem uma área XY utilizável
drill-hole-smooth-interpolation = Interpolação suave
drill-hole-spacing-would-scan-too-many = Este espaçamento examinaria demasiadas células; aumente o afastamento ou o espaçamento (máximo: { $maximum } furos)
drill-hole-square = Quadrado
drill-hole-staggered = Alternado
drill-hole-stepped-bands = Bandas escalonadas
drill-hole-text = ×
drill-hole-text-2 = −
drill-hole-unsupported-drillhole-source = Fonte de furos de sondagem incompatível
drill-hole-width = Largura
drill-pattern-arrangement = Disposição
drill-pattern-axis-offset = Desvio em { $axis }
drill-pattern-blast-shape = Contorno de desmonte
drill-pattern-burden = Afastamento
drill-pattern-choose-closed-blast-boundary-then = Escolha um limite de desmonte fechado e ajuste a grelha. Os furos são atualizados em tempo real na vista.
drill-pattern-closed-design-polyline-whose-xy = Polilinha de desenho fechada cuja projeção XY será preenchida com furos.
drill-pattern-counter-clockwise-pattern-rotation-f = Rotação do padrão no sentido anti-horário a partir do eixo global { $axis }.
drill-pattern-distance-between-holes-along-each = Distância entre furos ao longo de cada linha do padrão.
drill-pattern-e-g-west-cut-03 = por ex., Corte oeste 03
drill-pattern-finished-hole-diameter-entered-milli = Diâmetro final do furo. Introduzido em milímetros e guardado com cada furo gerado.
drill-pattern-hole-depth = Profundidade do furo
drill-pattern-hole-diameter = Diâmetro do furo
drill-pattern-move-over-closed-polyline-then = Passe sobre uma polilinha fechada e clique nela na vista. Esc cancela a seleção.
drill-pattern-name-drillhole-dataset-created-proje = Nome do conjunto de furos criado no projeto.
drill-pattern-none-picked = Nenhum selecionado
drill-pattern-pattern-name = Nome do padrão
drill-pattern-perpendicular-distance-between-patte = Distância perpendicular entre as linhas do padrão.
drill-pattern-pick = Escolher
drill-pattern-preview-count-hole-s-diameter = Pré-visualização: { $count } furo(s) · diâmetro de { $diameter } mm · profundidade de { $depth } m
drill-pattern-rotation = Rotação
drill-pattern-shift-pattern-grid-along-global = Desloca a grelha do padrão ao longo do eixo global { $axis } mantendo-a recortada à forma do desmonte.
drill-pattern-spacing = Espaçamento
drill-pattern-staggered-offsets-every-second-row = O padrão alternado desloca cada segunda linha por metade do espaçamento.
drill-pattern-vertical-depth-below-each-collar = Profundidade vertical abaixo de cada boca.

## Dxf strings

dxf-dxf-block-nesting-exceeds-maximum = O aninhamento de blocos DXF excede a profundidade máxima ({ $depth }); ignorando '{ $name }'
dxf-dxf-circular-block-reference-detecte = Referência circular de bloco DXF detetada: '{ $name }'
dxf-dxf-entity-referenced-undefined-laye = Uma entidade DXF referenciou a camada indefinida '{ $name }'; importada como '{ $fallback }'
dxf-dxf-import-exceeds-what-budget = A importação DXF excede o orçamento de { $what } ({ $limit }); a geometria restante será ignorada
dxf-dxf-insert-references-unknown-block = DXF INSERT referencia o bloco desconhecido '{ $name }'

## Edit strings

edit-absolute-length = Comprimento absoluto
edit-absolute-rl = RL absoluto
edit-action = Ação
edit-angle = Ângulo
edit-angle-from-horizontal-negative-downw = Ângulo a partir da horizontal, negativo para baixo: -90 é um furo vertical.
edit-app-web-not-recommended-production = O { $app } Web não é recomendado para produção. Use-o apenas como demonstração.
edit-application = Aplicação
edit-apply = Aplicar
edit-apply-pick-target = Aplicar e escolher alvo
edit-axis-value = Valor de { $axis }
edit-azimuth = Azimute
edit-batter-angle = Ângulo de talude (°)
edit-bearing-holes-drilled-degrees-clockw = Rumo de perfuração dos furos, em graus no sentido horário a partir do norte da grelha.
edit-bench-height = Altura da bancada
edit-benches = Bancadas
edit-berm-width = Largura da berma
edit-bezier-curve = Curva de Bezier
edit-choose-layer = Escolher uma camada
edit-choose-whether-entered-value-distanc = Escolha se o valor introduzido é distância ao longo do declive, largura horizontal ou altura vertical.
edit-choose-which-two-polyline-paths = Escolha qual dos dois percursos da polilinha entre os vértices selecionados será substituído. O comprimento inclui elevação e arestas curvas.
edit-click-corner-closed-polyline = Clique num canto de uma polilinha fechada.
edit-click-open-closed-polyline-begin = Clique numa polilinha aberta ou fechada para começar.
edit-click-second-vertex-replacement-span = Clique no segundo vértice do espaço de reposição.
edit-click-vertex-start-replacement-span = Clique num vértice para iniciar o período de substituição.
edit-collide-triangulation = Colisão com triangulação
edit-confirm-selection = Confirmar a seleção
edit-control-point-1 = Ponto de controlo 1
edit-control-point-2 = Ponto de controlo 2
edit-copy = Copiar
edit-corner-radius-limited-so-replacement = Raio de esquina, limitado para que o substituto não possa atravessar os vértices adjacentes.
edit-create-new-layer = Criar uma nova camada
edit-create-new-project = Criar um novo projeto
edit-create-project = Criar um projeto
edit-delta-length-m-use = Variação de comprimento (m, use + ou -)
edit-dip = Mergulho
edit-direction = Direção
edit-distance = Distância
edit-distance-along-slope = Distância ao longo do talude
edit-download-free-native-version-our = Transfira a versão nativa gratuita no nosso site ↗
edit-drill-hole = Furo de sondagem
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = Fim
edit-enter-valid-elevation = Digite uma elevação válida.
edit-exit-slice = Sair da fatia
edit-finish-polyline = Finalizar polilinha
edit-generate-batter-berms = Gerar bancadas e bermas
edit-height = Altura
edit-height-change = Variação de altura
edit-height-mode = Modo de altura
edit-horizontal-distance = Distância horizontal
edit-horizontal-width-each-flat-berm = Largura horizontal de cada berma plana entre os sucessivos taludes.
edit-hover-choose-which-end-move = Passe o cursor para escolher qual extremidade mover, depois clique para confirmar.
edit-insert-point-elevation = Inserir ponto na elevação
edit-intersect = Intersetar
edit-kind-properties = { $properties } de { $kind }
edit-layer-name = Nome da camada
edit-load-project = Carregar projeto
edit-longest = Mais longo
edit-m-s = m/s
edit-measure = Medida
edit-mit-license = Licença MIT
edit-mode = Modo
edit-move = Mover
edit-move-layer = Mover para a camada
edit-move-which-end = Mover qual extremidade
edit-movement-speed-slice-when-using = Velocidade de movimento da fatia ao utilizar as teclas de navegação.
edit-moving-end-endpoint = A mover: extremidade final
edit-moving-start-endpoint = A mover: extremidade inicial
edit-new-length-m = Novo comprimento (m)
edit-new-project = Novo projeto
edit-number-complete-batter-berm-levels = Número de níveis completos de talude e berma. O máximo é limitado ao nível mais profundo que preserva a geometria indicada.
edit-number-line-segments-used-approximat = Número de segmentos de linha usados para aproximar a curva entre os dois vértices selecionados.
edit-number-straight-segments-used-approx = Número de segmentos retos usados para aproximar o canto arredondado. Use 1 para um chanfro reto.
edit-object = Objeto
edit-offset-element = Elemento de compensação
edit-pick-side = Escolha o lado
edit-pit = Cava
edit-project-name = Nome do projeto
edit-properties = Propriedades
edit-radius = Raio
edit-recent = Recentes
edit-relative = Relativo (+/-)
edit-relative-applies-vertical-change-eve = Relativo aplica uma alteração vertical a todos os pontos. RL absoluto projeta todos os pontos numa única elevação alvo.
edit-remove-from-list = Remover da lista
edit-replace-path = Substituir caminho
edit-rotate = Girar
edit-rotation-speed-slice-when-using = Velocidade de rotação da fatia ao utilizar Q e E.
edit-s = °/s
edit-segments = Segmentos
edit-segments-lying-elevation-ignored = Os segmentos que se encontram nesta altitude são ignorados.
edit-select-endpoint-changes-other-endpoi = Selecione a extremidade que muda; a outra permanece fixa.
edit-selected-holes-point-different-ways = Os furos selecionados apontam em direções diferentes. Aplicar define todos com estes ângulos.
edit-selected-start-end-point-moves = O ponto inicial ou final selecionado move-se na direção da linha; a extremidade oposta permanece fixa.
edit-set-axis = Definir { $axis }
edit-shortest = Mais curto
edit-slice-view = Vista de fatia
edit-slope-angle-each-batter-face = Ângulo de inclinação de cada talude da bancada, medido a partir de horizontal.
edit-slope-angle-offset-positive-negative = Ângulo de inclinação do deslocamento. Ângulos positivos e negativos movem a cópia acima ou abaixo da origem durante o movimento lateral.
edit-speed = Velocidade
edit-start = Início
edit-stockpile = Pilha de estoque
edit-stop-generated-offset-where-its = Parar o deslocamento gerado quando o seu caminho primeiro encontrar uma triangulação visível.
edit-target-rl = Cota-alvo
edit-text-colour-opacity = Cor e opacidade do texto.
edit-thickness-visible-slice-slab-centred = Espessura da placa de fatia visível centrada no indicador de visão geral.
edit-translation-distance-along-world-axi = Distância de translação ao longo do eixo mundial { $axis }.
edit-type = Tipo
edit-type-direction-together-set-offset = O tipo e a direção determinam juntos o lado do deslocamento. Cava + Acima e Pilha + Abaixo avançam para fora; Cava + Abaixo e Pilha + Acima avançam para dentro.
edit-up-raises-each-bench-bench = Acima eleva cada bancada pela altura da bancada; Abaixo baixa-a. Isto também inverte o lado do deslocamento; consulte Tipo.
edit-value-interpreted-using-selected-mea = O valor é interpretado utilizando o modo Medida e Altura selecionado.
edit-vertical-rise-fall-each-bench = A ascensão ou queda vertical de cada bancada antes da criação da próxima berma.
edit-world-x-y-z-coordinates = Coordenadas X, Y e Z globais do primeiro ponto de controlo Bézier.
edit-world-x-y-z-coordinates-2 = Coordenadas X, Y e Z globais do segundo ponto de controlo Bézier.

## Events strings

events-couldn-t-exit-error = Não foi possível sair: { $error }
events-couldn-t-save-error = Não foi possível guardar: { $error }
events-set-elevation = Definir elevação
events-set-elevation-from-cursor-hit = Elevação definida a partir do ponto do cursor em Z { $z }
events-tool-not-available-section-view = Essa ferramenta não está disponível na vista em secção

## Explorer strings

explorer-clear-active-triangulation-texture = Limpar textura da triangulação ativa
explorer-delete-from-project = Eliminar do projeto
explorer-discard-changes = Descartar alterações...
explorer-download = Transferir
explorer-drape-over-surface = Drapear sobre a superfície
explorer-draped-over-surface = Drapejado sobre uma superfície
explorer-duplicate = Duplicar
explorer-face-colour = Cor da face
explorer-id-block-model-id-source =
    ID: block-model:{ $id }{ $source }
    { $count } variável(is) de cor
explorer-id-drill-holes-id-source =
    ID: drill-holes:{ $id }{ $source }
    { $holes } furo(s)
    { $fields } campo(s) de cor
explorer-id-point-cloud-id-source =
    ID: point-cloud:{ $id }{ $source }
    { $count } ponto(s)
explorer-id-raster-id-source-driver =
    ID: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = ID: triangulação:{ $id }{ $source }
explorer-load = Carregar
explorer-lock = Bloquear
explorer-select-all-objects = Selecionar todos os objetos
explorer-source-name = Origem: { $name }
explorer-unload = Descarregar
explorer-unlock = Desbloquear

## Files strings

files-automatic-colour = Cor automática
files-automatic-rl-spacing = Espaçamento de cotas automático
files-axis-scale-ratio = Rácio de escala em { $axis }
files-ok = OK
files-reset-1 = Redefinir para 1×
files-rl-grid-options = Opções da grelha de cotas
files-rl-spacing = Espaçamento de cotas
files-scales-z-distances-visually-without = Dimensiona visualmente as distâncias Z sem alterar as coordenadas armazenadas.
files-thickness = Espessura
files-xy-grid-options = Opções da grelha XY

## Gpu strings

gpu-cache-block-model-surface-build-failed = Falha ao construir a superfície do modelo de blocos: { $error }
gpu-cache-block-model-surface-build-worker = O processo de construção da superfície do modelo de blocos foi desconectado
gpu-cache-block-model-surface-chunk-rejected = O bloco de superfície do modelo de blocos foi rejeitado antes da alocação da GPU: instâncias={ $instances } bytes, limite={ $limit } bytes
gpu-cache-block-volume-preparation-worker-disc = O processo de preparação do volume de blocos foi desconectado
gpu-cache-translucent-volume-could-not-built = Não foi possível criar o volume translúcido ({ $error }); este modelo de blocos será exibido como cubos.
gpu-cache-triangulation-edge-chunk-rejected-be = O bloco de arestas da triangulação foi rejeitado antes da alocação da GPU: instâncias={ $instances } bytes, limite={ $limit } bytes
gpu-cache-triangulation-gpu-chunk-rejected-bef = O bloco de triangulação da GPU foi rejeitado antes da alocação: vértices={ $vertices } bytes, índices={ $indices } bytes, limite={ $limit } bytes
gpu-cache-triangulation-name-has-count-vertice = A triangulação '{ $name }' tem { $count } vértices (> u32::MAX); não é possível dividi-la para a GPU
gpu-cache-triangulation-name-uploaded-chunks-s = A triangulação '{ $name }' foi carregada em { $chunks } blocos espaciais ({ $faces } faces)

## Init strings

init-gpu-adapter-vendor-name-backend = Adaptador GPU: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver-driver-driver-info = Controlador GPU: { $driver } { $driver_info }
init-gpu-supports-maximum-buffer-size = A GPU suporta um búfer máximo de { $size } MiB; cenas grandes podem não ser mostradas por completo
init-surface-presentation-mode-mode = Modo de apresentação da superfície: { $mode }
init-wgpu-error-continuing-error = Erro do wgpu (a continuar): { $error }

## Io strings

io-ascii-points-xyz-pts = Pontos ASCII (.xyz, .pts)
io-attribute = Atributo
io-blank-header = (cabeçalho em branco)
io-block-model = Modelo de blocos:
io-choose-file-purpose-map-its = Escolha uma finalidade para o ficheiro de modo a mapear as suas colunas.
io-choose-loaded-block-model = Escolher um modelo de blocos carregado
io-choose-loaded-layer = Escolher uma camada carregada
io-choose-loaded-triangulation = Escolher uma triangulação carregada
io-choose-purpose = Escolher finalidade…
io-choose-source-file-files-import = Escolha o ficheiro ou ficheiros de origem para importar.
io-collar = Boca do furo
io-column-mapping = Mapa de colunas
io-comma-separated-values-csv = Valores separados por vírgulas (.csv)
io-csv-files = Ficheiros CSV
io-default = Padrão
io-depth = Profundidade
io-diameter = Diâmetro
io-drawing-exchange-format-dxf = Formato de intercâmbio de desenhos (.dxf)
io-drill-holes = Furos de sondagem
io-east-x = Este / X
io-elevation-z = Elevação / Z
io-end-x = X final
io-end-y = Y final
io-end-z = Z final
io-explicit-segments = Segmentos explícitos
io-export = Exportar
io-export-csv-block-model = Exportar Modelo de Blocos em CSV
io-export-dxf = Exportar DXF
io-export-one-layer = Exportar uma camada
io-export-ply = Exportar PLY
io-export-stl = Exportar STL
io-export-wavefront-obj = Exportar Wavefront OBJ
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = Importar
io-import-ascii-point-cloud = Importar nuvem de pontos ASCII
io-import-drillhole-csv-bundle = Importar Pacote de CSV de Furos de Sondagem
io-import-geotiff = Importar GeoTIFF
io-import-las-laz-point-cloud = Importar nuvem de pontos LAS/LAZ
io-import-pcd-point-cloud = Importar nuvem de pontos PCD
io-import-ply = Importar PLY
io-import-stl = Importar STL
io-import-wavefront-obj = Importar Wavefront OBJ
io-interval = Intervalo
io-las-laz-las-laz = LAS/LAZ (.las, .laz)
io-mapped-csv-bundle-csv = Pacote CSV mapeado (.csv)
io-model-file = Ficheiro do modelo
io-name-count-files = { $name } + { $count } ficheiros
io-no-csv-chosen = Nenhum ficheiro .csv escolhido
io-no-csv-files-chosen = Nenhum ficheiro CSV escolhido
io-no-dxf-chosen = Nenhum ficheiro .dxf escolhido
io-no-omf-chosen = Nenhum ficheiro .omf escolhido
io-north-y = Norte / Y
io-ply-ply = PLY (.ply)
io-point-cloud-data-pcd = Dados de nuvem de pontos (.pcd)
io-source-file = Ficheiro de origem
io-start-x = X inicial
io-start-y = Y inicial
io-start-z = Z inicial
io-stl-stl = STL (.stl)
io-triangulation = Triangulação:
io-unmapped = Não mapeado
io-wavefront-obj-obj = Wavefront OBJ (.obj)

## Jobs strings

jobs-background-task-poll-label-ended = A tarefa em segundo plano “{ $poll_label }” terminou sem resultado
jobs-discarded-stale-background-result-po = O resultado em segundo plano obsoleto de «{ $poll_label }» foi descartado porque uma origem mudou ou foi fechada

## Logging strings

logging-activity-completed = Atividade concluída
logging-activity-started = Atividade iniciada
logging-application-id-id = ID da aplicação: { $id }
logging-application-name-name = Nome da aplicação: { $name }
logging-application-startup = Inicialização da aplicação
logging-build-target-os-architecture = Destino da compilação: { $os }-{ $architecture }
logging-completed = Concluído
logging-count-messages = { $count } mensagens
logging-desktop-session-xdg-session-type = Sessão do ambiente de trabalho: XDG_SESSION_TYPE={ $type }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
logging-initialising-incline-design = A inicializar o Incline Design
logging-locale-environment-lang-lang-lc = Ambiente de localidade: LANG={ $lang }, LC_ALL={ $locale }, TZ={ $timezone }
logging-macos-session-user-user-shell = Sessão do macOS: USER={ $user }, SHELL={ $shell }
logging-operating-system-gnu-linux = Sistema operativo: GNU/Linux
logging-operating-system-macos = Sistema operativo: macOS
logging-operating-system-microsoft-windows = Sistema operativo: Microsoft Windows
logging-pointer-width-width-bit = Largura do ponteiro: { $width } bits
logging-process-id-id = ID do processo: { $id }
logging-release-version-version = Versão de lançamento: { $version }
logging-renderer = Renderizador
logging-rust-compiler-host-host = Host do compilador Rust: { $host }
logging-system = Sistema
logging-system-error = Erro do sistema
logging-unknown = desconhecido
logging-windows-session-sessionname-session = Sessão do Windows: SESSIONNAME={ $session }, USERNAME={ $user }
logging-working = Em curso…

## Mac strings

mac-cannot-install-macos-menu-bar = Não é possível instalar a barra de menus do macOS fora da thread principal
mac-quit-app = Sair do { $app }

## Main strings

main-incline-design-web-startup-failed = Falha na inicialização do Incline Design Web: { $error }

## Menu strings

menu-count-files-selected = { $count } ficheiros selecionados

## Object strings

object-edit-appearance = Aparência
object-edit-arc-circle = Arco e círculo
object-edit-arc-segments = Segmentos de arco
object-edit-bulge = Flecha
object-edit-bulge-arcs-horizontal-data-model = Os arcos com flecha são horizontais segundo o modelo de dados: o arco curva em planta e a elevação varia em linha reta de um vértice para o seguinte.
object-edit-centre-x = Centro X
object-edit-centre-y = Centro Y
object-edit-centre-z = Centro Z
object-edit-chord = Corda
object-edit-colour-layer = Cor por camada
object-edit-enter-number = Introduza um número
object-edit-follow-owning-layer-s-colour = Seguir a cor da camada proprietária em vez de uma cor fixada a este objeto.
object-edit-id = ID
object-edit-identity = Identidade
object-edit-insert-after = Inserir depois
object-edit-join-last-vertex-back-first = Liga o último vértice de novo ao primeiro.
object-edit-length-length-m = Comprimento { $length } m
object-edit-move-down = Mover para baixo
object-edit-move-up = Mover para cima
object-edit-object-has-no-arc-segments = Este objeto não tem segmentos de arco.
object-edit-object-has-single-position = Este objeto tem uma única posição.
object-edit-object-needs-least-required-vertices = Este objeto precisa de pelo menos { $required } vértices
object-edit-one-more-properties-not-valid = Uma ou mais propriedades não são um número válido
object-edit-perimeter-length-m-area-area = Perímetro { $length } m, área { $area } m²
object-edit-reverse = Inverter
object-edit-row-row-position-bulge-not = Linha { $row }: a posição ou a flecha não são um número válido
object-edit-sweep = Varrimento
object-edit-text-not-number = «{ $text }» não é um número
object-edit-vertices = Vértices

## Omf strings

omf-element-name-has-count-tie = O elemento «{ $name }» tem { $count } ligação(ões) que referem furos que já não contém
omf-ignoring-colour-map-omf-attribute = A ignorar o mapa de cores no atributo OMF '{ $attribute }': { $error }
omf-mining-data-exported-incline = Dados de mineração exportados pelo Incline
omf-omf-import = Importação OMF
omf-omf-texture = Textura OMF
omf-omf-validation-warnings-warnings = Avisos de validação OMF: { $warnings }
omf-project-application-metadata-applica = Os metadados de aplicação do projeto '{ $application }' não são mantidos
omf-project-author-not-retained = O autor do projeto não é mantido
omf-project-description-not-retained = A descrição do projeto não é mantida
omf-project-has-unsupported-metadata-key = O projeto contém chaves de metadados não suportadas: { $keys }

## Plot strings

plot-1-1000-one-millimetre-sheet = A 1:1000, um milímetro na folha equivale a um metro no terreno.
plot-1-scale-covers-width-height = 1:{ $scale } · abrange { $width } × { $height } m
plot-all-visible-data = Todos os dados visíveis
plot-automatic-grid-interval = Intervalo de grade automático
plot-border = Borda
plot-centre = Centrar em
plot-choose-smallest-conventional-scale-f = Escolha a menor escala convencional que se encaixa em tudo o que é visível na folha.
plot-coordinate-grid = Grade de coordenadas
plot-current-view-centre = Centro da vista atual
plot-date = DATA
plot-date-2 = Data
plot-dots-per-inch-paper-size = Pontos por polegada. Este tamanho de papel pode ser rasterizado até { $max_dpi } dpi; 300 dpi é uma qualidade de impressão normal.
plot-dpi = dpi
plot-drawing-no = DESENHO N.º
plot-drawing-number = Número de desenho
plot-drawn = DESENHADO POR
plot-drawn-2 = Desenhado por
plot-e-g-example-gold-project = Por exemplo, Projeto de Ouro de Exemplo
plot-entered-coordinates = Coordenadas introduzidas
plot-export-png = Exportar PNG...
plot-fit-scale-visible-data = Escala adequada aos dados visíveis
plot-grid-interval = Intervalo de grade
plot-landscape = Paisagem
plot-lists-visible-surfaces-design-layers = Lista as superfícies visíveis e as camadas de design com as suas cores.
plot-margin = Margem
plot-margins-leave-no-room-map = As margens não deixam espaço para o mapa
plot-metres-scale-1-scale = metros    Escala 1:{ $scale }
plot-mm = mm
plot-north-arrow = Seta norte
plot-nothing-visible-draw = Nada visível para desenhar
plot-paper = Papel
plot-paper-orientation-width-height-mm = { $paper } { $orientation } · { $width } × { $height } mm
plot-paper-size = Tamanho do papel
plot-pick-interval-reads-roughly-every = Escolha um intervalo que seja lido aproximadamente a cada 50 mm na folha impressa.
plot-plan = Planta
plot-plot-scale-must-positive-number = A escala do desenho deve ser um número positivo
plot-png-written-sheet-s-exact = O PNG é gravado com o tamanho físico exato da folha e regista o DPI, sendo impresso à escala real.
plot-portrait = Retrato
plot-resolution = Resolução
plot-rev = REV.
plot-revision = Revisão
plot-scale = ESCALA
plot-scale-1 = Escala 1:
plot-scale-framing = Escala e enquadramento
plot-sheet-furniture = Elementos da folha
plot-size-width-height-mm = { $size } ({ $width } × { $height } mm)
plot-subtitle = Subtítulo
plot-title = Título
plot-title-block = Bloco de título
plot-today = hoje

## Products strings

products-add-initiation = Adicionar iniciação
products-delay = Atraso
products-delay-palette = Paleta de atraso
products-how-long-after-shot-fired = Tempo entre o disparo e a iniciação da carga nesta boca de furo.
products-initiation-name = Iniciação · { $name }
products-milliseconds-between-one-hole-firing = Milissegundos entre a detonação de um furo e o seguinte.
products-ms = ms
products-no-products = Sem produtos
products-remove = Remover
products-update = Atualizar

## Progress strings

progress-percent-done-total = { $percent } ({ $done } de { $total })
progress-task-finished = { $task }: concluída

## Project strings

project-item = Item

## Properties strings

properties-adds-view-dependent-rim-highlight = Adiciona um realce de contorno dependente da vista nos limites dos blocos e materiais. Desativar reduz ligeiramente o trabalho de renderização volumétrica.
properties-block-model-downscale = Modelo de blocos em escala reduzida
properties-camera = Câmara
properties-camera-clip-planes = Planos de clip da câmera
properties-cap-while-resizing = Limitar ao redimensionar
properties-dark-mode = Modo escuro
properties-developer = Programador
properties-downscale-rasters = Rasters em escala reduzida
properties-edit-object = Editar objeto...
properties-field-view = Campo de visão
properties-fps = fotogramas/s
properties-frame-counter = Contador de quadros
properties-frame-rate-cap = Limite máximo da taxa de quadros
properties-hz = Hz
properties-interface = Interface
properties-invert-horizontal = Inverter horizontal
properties-invert-vertical = Inverter vertical
properties-limits-newly-loaded-geotiff-previews = Limita as pré-visualizações de GeoTIFF recém-carregados a 4096 píxeis no lado maior. Desative para usar a resolução total até ao limite de textura da GPU, com maior uso de memória.
properties-line-colour = Cor de linha
properties-look-sensitivity = Sensibilidade ao olhar
properties-max-clip-span = Máximo comprimento do clip
properties-move-layer = Mover para a camada...
properties-near-clip-limit = Limite próximo do clip
properties-orbit-sensitivity = Sensibilidade à órbita
properties-panel-chrome = Moldura do painel
properties-performance = Desempenho
properties-plan-mode = Modo de planeamento
properties-presents-step-display-no-tearing = Apresenta em sincronia com o ecrã: sem tearing, e o ecrã determina a taxa de fotogramas. Desativado, os fotogramas são apresentados assim que são desenhados e aplica-se o limite abaixo.
properties-reflective-block-edges = Arestas reflexivas dos blocos
properties-restore-defaults-2 = Restaurar predefinições
properties-show-console = Mostrar console
properties-shows-live-near-far-projection = Mostra as distâncias de projeção próxima e distante em tempo real na barra de status.
properties-snap-polling = Verificação de encaixe
properties-vertical-sync = Sincronização vertical
properties-world-axis-gizmo = Gizmo do eixo mundial
properties-zoom-cursor = Zoom para o cursor
properties-zoom-sensitivity = Sensibilidade ao zoom

## Screenshot strings

screenshot-could-not-encode-viewport-image = Não foi possível codificar a imagem da vista: { $error }
screenshot-could-not-map-viewport-screenshot = Não foi possível mapear a captura da vista: { $error }
screenshot-could-not-save-viewport-image = Não foi possível guardar a imagem da vista { $path }: { $error }
screenshot-downloaded-viewport-image-file-name = Imagem da vista transferida: { $file_name }
screenshot-saved-viewport-image-path = Imagem da vista guardada: { $path }
screenshot-viewport-image-download-failed-error = Falha ao transferir a imagem da vista: { $error }

## Spatial strings

spatial-bvh-face-index-index-out = O índice de face BVH { $index } está fora do intervalo da malha; substituindo por triângulo degenerado

## State strings

state-above = em ou acima
state-activate-project = Ativar Projeto
state-all-open-incline-design-data = Todos os dados abertos do Incline Design
state-apply-generated-rings = Aplicar anéis gerados
state-apply-selection = Aplicar à seleção
state-azimuth-azimuth-dip-dip = por azimute { $azimuth }°, inclinação { $dip }°
state-azimuth-azimuth-dip-dip-2 = até ao azimute { $azimuth }°, inclinação { $dip }°
state-below = em ou abaixo
state-centre-rotation = Centro de rotação
state-checking-unsaved-work = A verificar trabalho não guardado
state-choose-destination = Escolher um destino
state-choose-one-more-files = Escolher um ou mais ficheiros
state-clear-raster = Limpar raster
state-click-pit-shell-viewport = Clique no invólucro da cava na janela de visualização.
state-click-pit-stockpile-solid-viewport = Clique no sólido cava ou pilha de estoque na janela de visualização.
state-click-surface-viewport = Clique na superfície na janela de visualização.
state-click-topology-viewport = Clique na topografia na janela de visualização.
state-close-project = Fechar Projeto
state-colour-drillholes = Colorir furos de sondagem
state-copy-objects-layer = Copiar objetos para camada
state-count-file-s = { $count } ficheiro(s)
state-count-object-s-axis-value = { $count } objeto(s) · { $axis } { $value }
state-count-object-s-closed = { $count } objeto(s) · { $closed }
state-count-object-s-layer = { $count } objeto(s) · { $layer }
state-count-object-s-weight = { $count } objeto(s) · { $weight }
state-count-object-s-z-elevation = { $count } objeto(s) · Z { $elevation }
state-create-point-cloud-tin = Criar TIN nuvem de pontos
state-create-project = Criar Projeto
state-current-project = Projeto atual
state-cut-topology-pit-shell = Cortar topografia para o invólucro da cava
state-cut-triangulation-polyline = Cortar triangulação por polilinha
state-cut-triangulation-z = Cortar triangulação por Z
state-dark-mode = Modo escuro
state-detached = Separado
state-disabled = Desativado
state-discard-project-changes = Descartar alterações do projeto
state-discard-replace-project = Descartar e substituir o projeto
state-discarding-unsaved-changes = A descartar alterações não guardadas
state-docked = Acoplado
state-drape-raster = Drapear raster
state-drill-pattern = Padrão de perfuração
state-duplicate-layer = Duplicar camada
state-east = Este
state-enabled = Ativado
state-exit-incline-design = Sair do Incline Design
state-export-block-model-csv = Exportar CSV do Modelo de Blocos
state-export-layer-dxf = Exportar Camada para DXF
state-export-omf = Exportar OMF
state-export-project-dxf = Exportar Projeto para DXF
state-export-triangulation = Exportar triangulação
state-export-viewport-image = Exportar Imagem da Janela de Visualização
state-finish-closed-polyline = Terminar polilinha fechada
state-finish-open-polyline = Terminar polilinha aberta
state-fit-extents = Ajustar às extensões
state-fix-release-centre-both-views = Fixa ou liberta o centro em torno do qual ambas as vistas orbitam
state-generate-contours = Gerar curvas de nível
state-hidden = Oculto
state-import-drillholes = Importar furos de sondagem
state-import-omf = Importar OMF
state-import-point-cloud = Importar nuvem de pontos
state-import-raster = Importar raster
state-import-triangulation = Importar triangulação
state-insert-intersection-points = Inserir pontos de interseção
state-insert-points-elevation = Inserir pontos na elevação
state-keep-inside = Manter dentro
state-keep-outside = Manter fora
state-kriged-block-model = Modelo de blocos por krigagem
state-load-block-model = Carregar modelo de blocos
state-load-drillholes = Carregar furos de sondagem
state-load-layer = Carregar camada
state-load-point-cloud = Carregar nuvem de pontos
state-load-raster = Carregar raster
state-load-triangulation = Carregar triangulação
state-locked-count-object-s = { $count } objeto(s) bloqueado(s)
state-major-major-minor-minor = Principal { $major } · secundário { $minor }
state-move-axis-value = Mover para valor do eixo
state-move-objects-layer = Mover objetos para camada
state-name-count-holes = { $name } · { $count } furos
state-name-count-object-s = { $name } · { $count } objeto(s)
state-name-z-min-z-max = { $name } · { $z_min } a { $z_max }
state-next-edit = Edição seguinte
state-north = Norte
state-open-containing-folder = Abrir a pasta que contém o ficheiro
state-open-project = Abrir Projeto
state-preserve-view-angle = Preservar ângulo da vista
state-previous-edit = Edição anterior
state-project-id = Projeto { $id }
state-remove-block-model = Remover o modelo de blocos
state-remove-drillholes = Remover furos de sondagem
state-remove-point-cloud = Remover a nuvem de pontos
state-remove-raster = Remover o raster
state-remove-triangulation = Remover a triangulação
state-removed-from-active-triangulation = Removido da triangulação ativa
state-removed-from-every-triangulation = Removido de todas as triangulações
state-rename-kind = Renomear { $kind }
state-save-close-project = Salvar e fechar o projeto
state-save-despite-unsupported-content = Guardar apesar do conteúdo não suportado
state-save-project = Salvar o projeto como
state-save-replace-project = Salvar e Substituir Projeto
state-saving-current-project = A guardar o projeto atual
state-section-section = Secção { $section }
state-select-layer-objects = Selecionar objetos de camada
state-selected-objects = Objetos selecionados
state-selected-polylines = Polilinhas selecionadas
state-selected-scene-elements = Elementos de cena selecionados
state-set-block-model-variable = Definir variável do modelo de blocos
state-set-drillhole-colour-preset = Definir predefinição de cor dos furos de sondagem
state-set-entity-lock = Definir bloqueio da entidade
state-set-grid = Definir grelha
state-set-layer-lock = Definir bloqueio da camada
state-set-line-weight = Definir peso da linha
state-set-object-colour = Definir cor do objeto
state-set-object-fill = Definir preenchimento do objeto
state-set-point-visibility = Definir visibilidade do ponto
state-set-polyline-closed = Definir polilinha como fechada
state-set-raster-lock = Definir bloqueio do raster
state-set-standard-view = Definir vista padrão
state-set-topology-wireframes = Definir estrutura de arame da topografia
state-set-triangulation-colour = Definir cor da triangulação
state-show-console = Mostrar Console
state-show-project = Mostrar Projeto
state-shown = Mostrado
state-slice-mode = Modo de fatia
state-slice-preview = Pré-visualização de fatia
state-south = Sul
state-stem-contours = Curvas de nível de { $stem }
state-target-new-name = { $target } para “{ $new_name }”
state-trim-above = Aparar acima
state-trim-below = Aparar abaixo
state-trim-triangulation-surface = Aparar triangulação até a superfície
state-undrape-raster = Remover drapeamento do raster
state-undrape-rasters = Remover drapeamento dos rasters
state-unload-block-model = Descarregar modelo de blocos
state-unload-drillholes = Descarregar furos de sondagem
state-unload-layer = Descarregar camada
state-unload-point-cloud = Descarregar nuvem de pontos
state-unload-raster = Descarregar raster
state-unload-triangulation = Descarregar triangulação
state-untitled-project = Projeto sem título
state-use-typed-radius = Usar o raio introduzido
state-west = Oeste

## Status strings

status-clip-near-far = Recorte próximo/distante/Δ: -- / -- / --
status-frame-rate = Taxa de fotogramas

## Text strings

text-could-not-build-vector-mesh = Não foi possível criar a malha vetorial para a fonte { $font }, glifo { $glyph }: { $error }
text-document-text-mesh-exceeded-its = A malha de texto do documento excedeu o seu intervalo de índices u32

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = Escolha primeiro o conjunto de furos a ligar
tie-in-count-connector-s = { $count } conector(es)
tie-in-delete-tie-ins = Eliminar ligações
tie-in-deleted-count-selected-tie-connector = Foram eliminados { $count } conectores selecionados
tie-in-hole = furo
tie-in-initiation-point-lifted-from-name = Ponto de iniciação removido de { $name }
tie-in-initiation-point-set-name-delay = Ponto de iniciação definido em { $name } com atraso de { $delay } ms
tie-in-select-delay-product-palette-before = Selecione um produto de atraso na paleta antes de ligar os furos
tie-in-tied-count-connector-s-delay = Foram ligados { $count } conector(es) com atraso de { $delay } ms usando { $product }
tie-in-tied-count-connector-s-delay-2 = Foram ligados { $count } conector(es) com atraso de { $delay } ms usando { $product }, substituindo { $replaced }

## Toolbar strings

toolbar-fill-type = Tipo de preenchimento

## Toolbars strings

toolbars-auto-bench = Bancada automática
toolbars-bezier-polyline = Polilinha Bézier
toolbars-chamfer-polyline-corners = Chanfrar cantos da polilinha
toolbars-create-text = Criar texto
toolbars-cursor-regular = Cursor: normal
toolbars-cursor-snap-line = Cursor: ajustar à linha
toolbars-cursor-snap-point = Cursor: ajustar ao ponto
toolbars-cursor-snap-surface = Cursor: ajustar à superfície
toolbars-delete-points = Eliminar pontos
toolbars-explode-polyline-lines = Explodir polilinha em linhas
toolbars-fuse-polylines = Fundir polilinhas
toolbars-measure-distance = Medir distância
toolbars-new-layer = Nova camada
toolbars-split-polyline-points = Dividir polilinha nos pontos
toolbars-strike-dip = Direção e mergulho
toolbars-tool-not-available-section-view = { $tool } - não disponível na vista em secção

## Tri strings

tri-adaptive-concentrates-vertices-compl = Adaptativo concentra vértices em terreno complexo pelo erro de ajuste do plano; uniforme distribui-os regularmente. Poderão ser adicionados mais métodos no futuro.
tri-adaptive-quadtree = Adaptativo (quadtree)
tri-axis-range = Intervalo em { $axis }
tri-base-topology-will-receive-pit = A topografia base que receberá a forma da cava ou pilha.
tri-boundary-polyline = Polilinha de limite
tri-bridge-gaps-boundary-concavities-nar = Preenche lacunas e concavidades do limite mais estreitas que este valor. 0 ainda preenche lacunas até aproximadamente ao tamanho da célula; valores maiores preenchem vazios maiores e erodem concavidades.
tri-budget = Orçamento por
tri-cancel-pick = Cancelar a escolha
tri-candidate-detail = Detalhes dos candidatos
tri-candidate-fine-cells-per-budgeted = Células finas candidatas por vértice orçamentado. Um valor maior dá mais liberdade ao amostrador adaptativo, mas demora mais a gerar.
tri-cap-surface-share-source-points = Limite a superfície por uma parte dos pontos de origem ou por uma contagem exata dos vértices.
tri-choose-input-clicking-loaded-surface = Escolha esta entrada clicando numa superfície carregada na janela de visualização
tri-choose-which-side-reference-topology = Escolha o lado da topografia de referência a remover da superfície dentro da área XY comum.
tri-clip = Recorte
tri-clip-creates-new-triangulation-name = O recorte cria uma nova triangulação com este nome; a superfície de origem não é alterada.
tri-clip-surface-polyline = Recortar superfície por polilinha
tri-closed-pit-stockpile-solid-whose = Um sólido fechado de cava ou pilha cujo limite exposto será incluído no resultado.
tri-create-new-layer-contours-append = Crie uma camada para os contornos ou adicione-os a uma camada existente do projeto ativo.
tri-cut-topology-pit-shell = Cortar topografia com o invólucro da cava
tri-e-g-design-trimmed = Por exemplo, design_trimmed
tri-e-g-mysurf-cut = Por exemplo, mysurf_cut
tri-e-g-mysurf-slice = Por exemplo, mysurf_slice
tri-e-g-surface-contour = Por exemplo, surface_contour
tri-e-g-topo-cut = Por exemplo, topo_cut
tri-e-g-topo-pit = Por exemplo, topo_with_pit
tri-exact-number-surface-vertices-target = Número exato de vértices-alvo da superfície. Valores muito altos demoram a gerar e usam muita memória.
tri-existing-ground-topology-will-cut = A topografia existente do terreno que será cortada pelo invólucro da cava.
tri-fill-holes-up = Preencher buracos até
tri-generate = Gerar
tri-generate-contour-lines = Gerar linhas de curva de nível
tri-generate-upper-surface = Gerar a superfície superior
tri-hide-unload-sources = Ocultar e descarregar fontes
tri-higher-edge-will-enforced-each = Em cada conflito será imposta a aresta mais elevada. Os segmentos inferiores em conflito serão ignorados como linhas de quebra e a superfície será interpolada nessas áreas. As polilinhas de origem não são alteradas.
tri-highlighted-breakline-edges-cross-ov = As arestas de linha de quebra destacadas cruzam-se ou sobrepõem-se em planta a elevações diferentes. Uma única superfície de terreno não pode seguir ambas.
tri-intervals-colours = Intervalos e cores
tri-keep-clipped-topology-included-shape = Mantenha a topografia recortada e a forma incluída como triangulações separadas, em vez de as combinar numa entidade.
tri-keep-inside-discards-surface-outside = Manter dentro descarta a superfície fora da polilinha. Manter fora recorta da superfície um vazio com a forma da polilinha.
tri-keeps-only-surface-within-polyline = Mantém apenas a superfície dentro do limite da polilinha.
tri-keeps-surface-relation-topology-with = Mantém a superfície { $relation } a topografia dentro da respetiva cobertura XY.
tri-layer-already-exists-select-above = Essa camada já existe; selecione-a acima ou escolha outro nome.
tri-limit-z-range = Intervalo de limite Z
tri-major = Principal
tri-max-edge-length = Largura máxima da borda
tri-merge = Combinar
tri-method = Método
tri-min = Mín.
tri-minimum-maximum-elevations-retained = Elevações mínima e máxima mantidas na superfície de saída. A mínima deve ser inferior à máxima.
tri-minor = Secundário
tri-minor-controls-ordinary-contours-maj = Menor controla os contornos normais. Maior controla os contornos realçados e deve ter um intervalo pelo menos igual ao Menor.
tri-move-cursor-over-loaded-surface = Mova o cursor sobre uma superfície carregada.
tri-name-assigned-elevation-clipped-outp = Nome atribuído à superfície de saída cortada por elevação.
tri-name-assigned-merged-topology-pit = Nome atribuído ao resultado combinado da topografia com a cava/pilha de estoque.
tri-name-assigned-newly-created-contour = Nome atribuído à nova camada de curvas de nível criada.
tri-name-assigned-reconstructed-triangul = Nome atribuído à triangulação reconstruída.
tri-name-assigned-topology-after-pit = Nome atribuído à topografia depois de o invólucro da cava ser cortado dela.
tri-name-assigned-trimmed-output-surface = Nome atribuído à superfície de saída cortada.
tri-nearby-breakline-vertices-do-not = Os vértices próximos das linhas de quebra não coincidem exatamente, pelo que a superfície não pode ser triangulada.
tri-new-layer = Nova camada
tri-new-layer-name = Nome da nova camada
tri-once-merge-succeeds-unload-source = Após uma combinação bem-sucedida, descarregue a topologia e o sólido de origem para deixar apenas o resultado combinado na cena.
tri-only-loaded-triangulations-can-picke = Só as triangulações carregadas podem ser escolhidas.
tri-operation = Operação
tri-output-layer = Camada de saída
tri-percentage = Porcentagem
tri-percentage-cloud = Percentagem da nuvem
tri-pick-from-view = Escolher a partir da vista
tri-pit-design-surface-only-areas = A superfície de desenho da cava. Apenas as áreas onde escava abaixo da topografia são usadas no corte.
tri-pit-shell = Invólucro da cava
tri-pit-stockpile-solid = Sólido de cava/pilha
tri-recommended-weld-retry = Recomendado: soldar e repetir
tri-reconstruct-triangulated-terrain-sur = Reconstrua uma superfície de terreno triangulada a partir de uma nuvem de pontos. O amostrador adaptativo usa o orçamento de vértices onde o terreno é mais complexo e mantém esparsas as áreas planas.
tri-reduce-budget-candidate-detail-if = Reduza o orçamento ou o detalhe candidato se o computador tiver menos RAM.
tri-reference-topology-defines-where-oth = A topografia de referência que define onde a outra superfície é aparada.
tri-reject-reconstructed-triangle-edges = Rejeita arestas de triângulos reconstruídos maiores que esta distância. Use 0 para não limitar o comprimento.
tri-removes-surface-within-polyline-boun = Remove a superfície dentro do limite da polilinha e mantém o restante.
tri-removes-topology-where-pit-shell = Remove a topografia onde o invólucro da cava escava abaixo dela, preenchendo o vazio. A união segue a linha de contacto 3D real entre as superfícies; mantém-se a topografia sob as partes do invólucro acima do terreno.
tri-result = Resultado
tri-save-two-entities = Salvo como duas entidades
tri-select = Selecionar…
tri-share-source-points-keep-fractions = Parte dos pontos de origem a manter. São permitidas frações como 0,125%.
tri-slice-triangulation-z-range = Fatiar triangulação por intervalo Z
tri-solution-generate-upper-surface = Solução: Gerar a superfície superior
tri-surface-trim = Superfície a aparar
tri-surface-will-changed-selected-topolo = A superfície que será alterada; a topografia selecionada permanece intacta.
tri-text = %
tri-topology = Topografia
tri-triangulation-failed = Falha na Triangulação
tri-trim = Aparar
tri-trim-topology = Aparar até a topografia
tri-uniform-grid = Grelha uniforme
tri-up-target-point-count-points = Até { $target } de { $point_count } pontos tornar-se-ão vértices da superfície ({ $percent }%).
tri-use-full-surface-elevation-range = Utilize toda a faixa de elevação da superfície
tri-vertex-count = Contagem de vértices
tri-vertices-within-5-cm-xy = Os vértices a até 5 cm em XY e Z partilharão uma posição nesta triangulação. Isto pode deslocar localmente a superfície gerada até 5 cm; as polilinhas de origem não são alteradas.
tri-weld-retry = Soldar e repetir
tri-when-enabled-generate-contours-only = Quando ativado, gera contornos apenas entre as elevações mínima e máxima indicadas.

## Ui strings

ui-choose-offset-side = Escolha o lado de compensação
ui-choose-relimit-side = Escolha o lado da redefinição de limite
ui-click-circle-centre = Clique no centro do círculo
ui-click-closed-polyline-use-blast = Clique numa polilinha fechada para a usar como contorno de desmonte
ui-click-collar-add-edit-initiation = Clique numa boca de furo para adicionar ou editar um ponto de iniciação
ui-click-first-point-slice-line = Clique no primeiro ponto da linha da fatia
ui-click-first-vertex = Clique no primeiro vértice
ui-click-perimeter-point-type-radius = Clique em um ponto de perímetro ou digite um raio
ui-click-second-point-slice-line = Clique no segundo ponto da linha da fatia
ui-click-second-vertex = Clique no segundo vértice
ui-click-use-pointer-radius = ou clique para usar o raio do ponteiro
ui-could-not-copy-text-browser = Não foi possível copiar o texto para a área de transferência do navegador: { $error }
ui-dip-horizontal-no-strike = { $dip } (horizontal, sem direção)
ui-distance-meters = { $distance } metros
ui-drag-ring-type-azimuth-dip = Arraste um anel ou introduza um azimute e uma inclinação
ui-each-hole-turns-about-its = cada furo gira em torno da própria boca
ui-enter-positive-decimal-radius = Insira um raio decimal positivo
ui-esc-cancels = Esc cancela
ui-no-delay-product-tie = Nenhum produto de retardo para ligar
ui-press-enter-use-typed-radius = Pressione Enter para usar o raio digitado
ui-right-click-delay-palette-heading = clique com o botão direito no cabeçalho da paleta de atraso para adicionar um
ui-select-designs = Selecionar desenhos
ui-select-drill-hole = Selecione um furo
ui-select-endpoint-join = Selecione o ponto final para unir
ui-select-first-crest-toe-point = Selecione o primeiro ponto crista/pé
ui-select-item = Selecionar um item
ui-select-line-fuse = Selecionar uma linha para fusão
ui-select-line-polyline = Selecione uma linha ou polilinha
ui-select-line-relimit = Selecionar linha para redefinir limite
ui-select-next-line-fuse = Selecione a linha seguinte para fusão
ui-select-opposite-berm-point = Selecione o ponto oposto da berma
ui-select-point = Selecionar um ponto
ui-select-polyline = Selecione uma polilinha
ui-select-polyline-open-line = Selecione uma polilinha ou linha aberta
ui-select-polyline-vertex = Selecione um vértice da polilinha
ui-select-second-crest-toe-point = Selecionar o segundo ponto crista/pé
ui-select-second-split-point = Selecione o segundo ponto de divisão
ui-select-split-point = Selecionar um ponto de divisão
ui-select-topologies = Selecionar topografias
ui-slice-view = Vista de corte
ui-strike-strike-dip = direção { $strike }° · { $dip }
ui-value-dip = mergulho de { $value }°

## Viewport strings

viewport-all-total-categories-keep-their = Todas as { $total } categorias mantêm a cor; apenas as primeiras { $shown } são desenhadas de forma distinta
viewport-axis-maximum = Máximo de { $axis }
viewport-axis-minimum = Mínimo de { $axis }
viewport-bar-blast-timeline-placeholder = Cronologia do desmonte [PROVISÓRIO]
viewport-bar-burden-relief-heatmap-placeholder = Mapa de calor de alívio do afastamento [PROVISÓRIO]
viewport-bar-color = Cor:
viewport-bar-contours-equal-time-placeholder = Curvas de nível de tempo igual [PROVISÓRIO]
viewport-bar-disable-flying-mode = Desativar modo de voo
viewport-bar-disable-x-ray-vision = Desativar visão de raios X
viewport-bar-drill-holes = Furos de sondagem:
viewport-bar-enable-flying-mode = Ativar modo de voo
viewport-bar-enable-x-ray-vision = Ativar visão de raios X
viewport-bar-exit-slice-view = Sair da vista de corte
viewport-bar-fill = Preencher:
viewport-bar-fix-centre-rotation = Fixar centro de rotação
viewport-bar-hide-points = Ocultar pontos
viewport-bar-hide-rl-grid = Ocultar grelha de cotas
viewport-bar-hide-wireframes = Ocultar estruturas de arame
viewport-bar-hide-xy-grid = Ocultar grelha XY
viewport-bar-release-centre-rotation = Libertar centro de rotação
viewport-bar-show-points = Mostrar pontos
viewport-bar-show-rl-grid = Mostrar grelha de cotas
viewport-bar-show-wireframes = Mostrar estruturas de arame
viewport-bar-show-xy-grid = Mostrar grelha XY
viewport-bar-vertical-slice-view = Vista de corte vertical
viewport-blank = (em branco)
viewport-choose-active-block-model-variable = Escolha a variável ativa do modelo de blocos
viewport-choose-variable = Escolher uma variável
viewport-click-edit-color-right-click = Clique para editar cor; clique com o botão direito para remover
viewport-click-type-boundary-s-value = Clique para digitar o valor deste limite
viewport-colour-mapping = Mapa de cores
viewport-count-categories = { $count } categorias
viewport-count-category = { $count } categoria
viewport-double-click-add-boundary-here = Clique duas vezes para adicionar um limite aqui
viewport-drag-move-middle-click-toggles = Arraste para mover · Clique central alterna ≤
viewport-drag-move-right-click-remove = Arraste para mover · Clique direito para remover · Clique central alterna ≤
viewport-e = E
viewport-edit-category-colour = Editar a cor desta categoria
viewport-edit-colour-used-empty-values = Editar a cor usada para valores vazios
viewport-empty = (vazio)
viewport-empty-hidden = (vazio · oculto)
viewport-filter-variables = Variáveis de filtro
viewport-middle-drag-pan-scroll-zoom = Arrastar com o botão do meio para deslocar · Rolar para ampliar/reduzir
viewport-middle-drag-pan-scroll-zoom-2 = Arrastar com o botão do meio para deslocar · Rolar para ampliar/reduzir · Clique para destacar
viewport-n = N
viewport-no-data-variable = Não há dados para esta variável
viewport-no-matches = Sem correspondências
viewport-no-usable-range = (sem intervalo utilizável)
viewport-rebuild-variable-s-colours-from = Recriar as cores desta variável a partir dos seus dados
viewport-reset = Repor
viewport-restore-full-model-range = Restaurar o intervalo completo do modelo
