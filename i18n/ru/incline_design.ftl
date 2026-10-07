# Incline — русский каталог сообщений.
#
# Может быть неполным: отсутствующие сообщения берутся из английского
# (`i18n/en/incline_design.ftl`). Идентификаторы слева от `=` и имена
# аргументов ({ $... }) менять нельзя — переводится только текст справа.

## Общее

common-cancel = Отмена
common-clear = Очистить
common-close = Закрыть
common-fill = Заливка
common-set = Настроить

## Строка состояния

status-language = Язык

## Меню — Файл

menu-file = Файл
menu-file-save-project = Сохранить проект
menu-file-save-project-as = Сохранить проект как...
menu-file-new-project = Новый проект...
menu-file-open-project = Открыть проект...
menu-file-open-recent = Последние проекты
menu-file-show-in-explorer = Показать в Проводнике
menu-file-show-in-folder = Открыть папку с файлом
menu-file-import = Импорт...
menu-file-export = Экспорт...
menu-file-export-viewport-image = Экспорт изображения области просмотра...
menu-file-export-engineering-drawing = Экспорт чертежа...
menu-file-about = О программе { $app }...
menu-file-exit = Выйти из приложения

## Меню — Вид

menu-view = Вид

## Workspaces

ws-production = Производство
ws-drill-and-blast = БВР
ws-geology = Геология
ws-planning = Планирование

## Menubars

ws-menubar-design = Проектирование
ws-menubar-triangulation = Триангуляция
ws-menubar-raster = Растер
ws-menubar-point-cloud = Облака точек
ws-menubar-block-model = Блочная модель
ws-menubar-drillholes = Скважины
ws-menubar-active-layer = Слой:

## Menubars functions

ws-menubar-design-insert-point = Вставить точку
ws-menubar-design-insert-point-at-intersection = На пересечении
ws-menubar-design-insert-point-at-elevation = На высотной отметке
ws-menubar-design-move-to = Перейти к
ws-menubar-design-create-triangulation = Создать триангуляцию

## Диалоги переименования и удаления

dialog-rename-title = Переименовать: { $kind }
dialog-rename-field = Новое имя
dialog-rename-field-hint = Обязательно
dialog-rename-submit = Переименовать
dialog-delete-title = Удалить: { $kind }
dialog-delete-confirm =
    Удалить «{ $name }» из проекта?
    Это действие нельзя отменить.
confirm-delete-product =
    Удалить продукт «{ $name }» из палитры?
    Это действие нельзя отменить.

## Диалог «Создать триангуляцию»

tri-create-title = Создать триангуляцию
tri-create-help = Триангулирует объекты, выбранные при открытии этого диалога. Закройте его, чтобы изменить выбор.
tri-create-type-label = Тип триангуляции
tri-create-type-help =
    «Открытая поверхность» создаёт полотно рельефного типа. «Тело» создаёт
    полностью замкнутую сетку и требует входных данных, образующих
    герметичную границу.
tri-create-output-name = Имя результата
tri-create-output-name-help = Имя, присваиваемое созданной триангуляции.
tri-create-output-name-hint = имя триангуляции
tri-create-run = Триангулировать
tri-selection-none = Выбранные объекты больше недоступны.

tri-selection-selected = Выбрано: { $summary }

tri-type-open-surface = Поверхность
tri-type-solid-closed = Тело

tri-count-polylines =
    { $count ->
        [one] { $count } полилиния
        [few] { $count } полилинии
       *[other] { $count } полилиний
    }
tri-count-strings =
    { $count ->
        [one] { $count } линия
        [few] { $count } линии
       *[other] { $count } линий
    }
tri-count-circles =
    { $count ->
        [one] { $count } окружность
        [few] { $count } окружности
       *[other] { $count } окружностей
    }
tri-count-points =
    { $count ->
        [one] { $count } точка
        [few] { $count } точки
       *[other] { $count } точек
    }
tri-count-texts =
    { $count ->
        [one] { $count } текстовый объект
        [few] { $count } текстовых объекта
       *[other] { $count } текстовых объектов
    }
tri-count-objects =
    { $count ->
        [one] { $count } объект
        [few] { $count } объекта
       *[other] { $count } объектов
    }

about-read-full-licence = Подробности о лицензии ↗
about-source-code = Исходный код
about-website = Сайт
about-title = О программе { $app }
drill-hole-colour-stop = Порог { $index }
properties-restore-defaults-tooltip = Восстановить настройки «{ $heading }» по умолчанию

ui-selected-count = Выбрано: { $count }
ui-selected-objects = Выбрано объектов: { $count }
ui-selected-polylines = Выбрано полилиний: { $count }
ui-invalid-axis-value = Введите допустимое значение оси { $axis }.
ui-selection-spans = Выбор простирается от { $min } до { $max }.
confirm-delete-count = Вы уверены, что хотите удалить выбранные элементы ({ $count })?
confirm-delete-layer = Удалить слой «{ $name }» и все объекты в нём?
    Это действие нельзя отменить.
plot-preview-pixels = { $width } × { $height } пикселей при { $dpi } dpi
tri-estimated-memory = Пиковое потребление памяти: около { $estimate }. { $detail }
block-grid-summary = Сетка: { $x } × { $y } × { $z } = { $count } блоков
status-selected = Выбрано: { $count }
status-faces = Грани: { $drawn } / { $total } (фрагментов: { $drawn_chunks }/{ $total_chunks })
status-clip = Ближняя/дальняя плоскость/Δ: { $near } / { $far } / { $delta } м
status-points = Точки: { $drawn } / { $target } из { $total } (фрагментов: { $drawn_chunks }/{ $total_chunks })

## Completed canonical messages

## Reused existing project translations

## Переводы интерфейса, выполненные вручную
explorer-no-rasters = Нет растров
slice-viewport-gestures = перетаскивание средней кнопкой: панорама · перетаскивание правой кнопкой: орбита · Shift+колесо: перемещение · W/S: сдвиг слоя · Q/E: поворот · Esc: выход

## Сведения о среде запуска

## Исходные строки, обнаруженные проверкой покрытия

## Диагностика запуска модуля визуализации

color-aci = ACI
color-aci-value = ACI { $index }
color-index = Индекс
color-rgb = RGB
color-opacity = Непрозрачность
color-edit = Нажмите, чтобы изменить цвет
color-saturation-value = Насыщенность и яркость
color-hue = Оттенок
asset-loading = Загрузка данных ресурса
asset-unloading = Выгрузка данных ресурса
asset-load-failed = Не удалось загрузить данные ресурса
asset-unload-failed = Не удалось выгрузить данные ресурса
preferences-title = Параметры
context-text-colour = Цвет текста
context-polylines = Полилинии
context-points = Точки
crs-unknown-ellipsoid = Нераспознанная модель Земли «{ $name }» в этом определении системы координат.
crs-no-ellipsoid = В этом определении системы координат не указана используемая модель Земли.
crs-unknown-code = EPSG:{ $code } отсутствует в реестре систем координат.
crs-transform-failed = Не удалось преобразовать координату; результат не является конечной позицией.
crs-no-datum-path = Опубликованное преобразование между системами отсчёта { $from } и { $to } (датумы EPSG { $source } и { $target }) недоступно. Преобразование в любом случае было бы неверным на неизвестную величину, поэтому ничего не изменено.
crs-unknown-datum = Систему отсчёта { $from } или { $to } определить невозможно, и они используют разные модели Земли. Преобразование между ними было бы неверным на неизвестную величину.
ws-survey = Геодезия
survey-count-designs = { $count } { $count ->
    [one] проект
    [few] проекта
   *[other] проектов
  }
survey-count-meshes = { $count } { $count ->
    [one] триангуляция
    [few] триангуляции
   *[other] триангуляций
  }
survey-count-models = { $count } { $count ->
    [one] блочная модель
    [few] блочные модели
   *[other] блочных моделей
  }
survey-count-clouds = { $count } { $count ->
    [one] облако точек
    [few] облака точек
   *[other] облаков точек
  }
survey-count-holes = { $count } { $count ->
    [one] набор скважин
    [few] набора скважин
   *[other] наборов скважин
  }
survey-count-rasters = { $count } { $count ->
    [one] растр
    [few] растра
   *[other] растров
  }
survey-angle = Поворот вокруг Z (против часовой стрелки)
survey-scale = Единый масштабный коэффициент XYZ
survey-invalid-transform = Начала координат, угол и результирующие координаты должны быть конечными.
survey-invalid-scale = Масштаб должен быть конечным положительным числом с конечной обратной величиной.
survey-empty-selection = Выберите хотя бы один поддерживаемый элемент для преобразования.
survey-unavailable = Выбранный элемент отсутствует или не загружен. Загрузите его перед преобразованием.
survey-wrong-project = Выбирайте проекты только из активного проекта.
survey-name-required = Введите название системы координат.
survey-working = Преобразование выбранных данных…
survey-completed = Преобразовано на месте: { $items }. Отмена восстановит их.
survey-failed = Ошибка преобразования: { $error }
survey-stale = Преобразование отменено, так как активный проект или исходные данные изменились. Выберите исходные данные и повторите попытку.
survey-coordinates-menu = Координаты
survey-definitions-action = Определения…
survey-transform-action = Преобразовать…
survey-definitions-title = Определения координат
survey-transform-title = Преобразование координат
survey-new-system = Новая система координат
survey-new-system-name = Система координат
survey-set-local = Задать как систему координат рудника
survey-delete-system = Удалить систему координат
survey-systems-empty = Нет систем координат
survey-system-name = Название
survey-system-origin = Та же точка — координаты в системе
survey-angle-help = Против часовой стрелки от оси X системы отсчёта к оси Y, если смотреть сверху.
survey-scale-help = Единый масштаб XYZ от системы отсчёта к этой системе. Используйте 1, чтобы сохранить размеры.
survey-close = Закрыть
survey-from = Из
survey-to = В
survey-transform-button = Преобразовать
survey-swap = Поменять местами
survey-drape-note = Наложенные изображения удаляются с преобразованных поверхностей и должны быть наложены заново.
survey-needs-grid-block-model = Блочная модель — это регулярная сетка ячеек, и смена проекции или системы отсчёта не сохраняет эту регулярность. Преобразование означало бы передискретизацию каждой ячейки в новую сетку с потерей содержащихся в них значений, поэтому модель оставлена без изменений.
survey-needs-grid-raster = Растр размещается в мире с помощью аффинного отображения, что смена проекции или системы отсчёта сохранить не может. Преобразование означало бы передискретизацию изображения, поэтому растр оставлен без изменений.
survey-conversion-exact = Точное: только смена сетки, без перепроецирования.
survey-conversion-accuracy = Заявленная точность { $accuracy } м.
survey-kind = Тип
survey-axis-names = Названия осей
survey-kind-registry-short = Система из реестра
survey-kind-grid-short = Сетка над другой системой
survey-registry-search = Поиск
survey-registry-hint = Название или код EPSG, напр. «mga zone 56»
survey-registry-none = В реестре нет совпадений по всем словам.
survey-parent = Определена относительно
survey-parent-origin = Известная точка — координаты родительской системы
survey-pick-registry = Найдите систему и выберите её из результатов.
survey-pick-parent = Выберите систему, относительно которой определена эта сетка.
survey-pick-system = Выберите систему
survey-pick-systems = Выберите исходную систему и систему назначения для преобразования.
survey-no-selection = Выберите систему координат слева или щёлкните правой кнопкой, чтобы добавить новую.
survey-kind-grid = Сетка над { $parent }
survey-system-in-use = «{ $name }» нельзя удалить: относительно неё определены { $dependants } { $dependants ->
    [one] система
    [few] системы
   *[other] систем
  }. Сначала перенаправьте их на другую систему.
survey-system-cycle = «{ $name }» определена относительно самой себя, напрямую или через свои родительские системы.
survey-system-missing = Эта система координат больше не существует. Выберите другое определение.
survey-same-system = Выберите разные исходную систему и систему назначения.
survey-name-exists = Система координат с таким названием уже существует. Выберите её для редактирования или укажите другое название.
preferences-ui-size = Размер интерфейса
preferences-ui-size-help = Изменяет размер текста и элементов управления относительно обычного масштабирования дисплея устройства. 100 % — размер по умолчанию. Разрешение экрана и размер окна не уменьшают интерфейс.
relimit-select-boundary = Выберите полилинию или окружность для изменения границы
relimit-click-boundary = Щёлкните полилинию или окружность для пересечения…
relimit-mode-help = «Пересечение» переносит одну конечную точку на полилинию или окружность. «Абсолютная» задаёт итоговую длину линии. «Относительная» добавляет или вычитает длину.
browser-graphics-device-lost = Браузер потерял графическое устройство. Откройте эту страницу заново в новой вкладке. Сведения о GPU: { $message }

## About strings

about-copyright-c-2026-leo-timmins =
    Copyright (c) 2026 Leo Timmins, Lucas Timmins, and Incline Design contributors. Permission is hereby granted, free of charge, to any person obtaining a copy of this software to deal in it without restriction, subject to the conditions of the MIT License.

    Incline Design is provided "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, including but not limited to the warranties of MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE and NONINFRINGEMENT.
about-free-open-source-mine-design = Свободная система проектирования горных работ с открытым исходным кодом
about-licensed-under-mit-license = Распространяется по лицензии MIT

## App strings

app-activated-browser-project-name = Проект браузера «{ $name }» активирован.
app-browser-project-delete-failed = Не удалось удалить проект браузера: { $error }
app-browser-project-no-longer-exists = Этот проект браузера больше не существует
app-browser-save-failed-error = Ошибка сохранения в браузере: { $error }
app-could-not-activate-browser-project = Не удалось активировать проект браузера: { $error }
app-could-not-delete-browser-project = Не удалось удалить проект браузера: { $error }
app-could-not-load-browser-project = Не удалось загрузить проект браузера: { $error }
app-could-not-restore-browser-project = Не удалось восстановить проект браузера: { $error }
app-deleted-browser-project = Проект браузера удалён
app-failed-create-window-error = Не удалось создать окно: { $error }
app-failed-create-window-icon-error = Не удалось создать значок окна: { $error }
app-failed-detach-top-down-preview = Не удалось отсоединить вид сверху: { $error }
app-failed-initialize-graphics-error = Не удалось инициализировать графику: { $error }
app-browser-preferences-load-failed = Не удалось загрузить настройки браузера: { $error }
app-failed-load-config-file-error = Не удалось загрузить файл конфигурации: { $error }
app-failed-load-session-file-error = Не удалось загрузить файл сеанса: { $error }
app-failed-rasterize-window-icon-error = Не удалось растрировать значок окна: { $error }
app-failed-save-browser-session-error = Не удалось сохранить сеанс браузера: { $error }
app-failed-save-session-error = Не удалось сохранить сеанс: { $error }
app-saved-name-browser-storage = «{ $name }» сохранён в хранилище браузера

## Block strings

block-model-between = Между
block-model-block-grid = Блочная сетка
block-model-block-size = Размер блока
block-model-choose-numeric-variable = Выберите числовую переменную
block-model-choose-numeric-variables = Выберите числовые переменные
block-model-count-variables-selected = Выбрано переменных: { $count }
block-model-estimate-variables = Оцениваемые переменные
block-model-full-x-y-z-dimensions = Полные размеры X, Y и Z каждого блока. Меньшие блоки повышают детализацию, время расчёта и расход памяти.
block-model-grid-bounds-block-sizes-invalid = Границы сетки или размеры блоков недействительны.
block-model-lower-x-y-z-edges = Нижние границы X, Y и Z объёма блочной модели. Центры блоков начинаются на половину блока внутрь этих границ.
block-model-maximum = Максимум
block-model-maximum-nearest-samples-used-each = Максимальное число ближайших проб для каждого блока. Меньшие значения ускоряют расчёт; большие могут сгладить оценки и увеличить время вычислений.
block-model-maximum-samples = Максимальное число образцов
block-model-minimum = Минимум
block-model-min-samples-help = Минимальное число ближайших проб для оценки блока. Блоки с меньшим числом проб в радиусе поиска остаются пустыми.
block-model-minimum-samples = Минимальное число образцов
block-model-no-block-model-selected = Блочная модель не выбрана
block-model-no-drill-holes-selected = Скважины не выбраны
block-model-nugget = Наггет
block-model-numeric-interval-fields-interpolate = Числовые поля интервалов для интерполяции. Каждое выбранное поле становится переменной блочной модели.
block-model-kriging-help = Обычный кригинг оценивает числовые интервалы скважин в центре каждого блока с использованием сферической вариограммы.
block-model-partial-sill = Частичный порог
block-model-range-search-radius = Диапазон / радиус поиска
block-model-range-help = Образцы дальше этого расстояния исключаются; ковариация достигает нуля на этом диапазоне.
block-model-select-all = Выберите все
block-model-selected-block-model-whose-blocks = Выбранная блочная модель, блоки которой преобразуются в тело по порогу. Закройте диалог, чтобы выбрать другую.
block-model-source-drill-holes-help = Выбранный набор скважин, числовые интервалы которого оцениваются в блоки. Закройте диалог, чтобы оценивать по другому набору.
block-model-sill-help = Пространственно коррелированная дисперсия сферической модели. Вместе с эффектом самородков задаёт ковариацию при нулевом расстоянии.
block-model-spherical-variogram-search = Сферическая вариограмма и поиск
block-model-threshold-at-most = <= порога
block-model-threshold-at-least = >= порога
block-model-threshold-min = Предел / мин
block-model-upper-x-y-z-extent = Верхние границы X, Y и Z покрываемого объёма. Последний блок может выйти за эту границу, если протяжённость не кратна размеру блока.
block-model-variable = Переменная
block-model-variance-effectively-zero-separation = Дисперсия при практически нулевом разделении, вызванная ошибкой измерения или изменчивостью ниже масштаба выборки. Используйте 0, если эффект самородков не нужен.
block-model-volume-feedback-disconnected = Обратное чтение данных использования объёма блоков отключилось
block-model-volume-feedback-failed = Не удалось прочитать данные использования объёма блоков: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z
borehole-inspector-checking-linked-geophysics-file = Проверка связанного геофизического файла...
borehole-inspector-close-inspector = Закрыть инспектор
borehole-inspector-data = Данные
borehole-inspector-display = Отображение
borehole-inspector-file-not-where-was-linked = Файл { $file } находится не там, откуда он был связан.
borehole-inspector-guessed-name = Определено по имени
borehole-inspector-hold-hole-while-you-work = Удерживать эту скважину, пока вы работаете с окружающими.
borehole-inspector-holding-hole-click-follow-selection = Скважина удерживается. Щёлкните, чтобы снова следовать за выбором.
borehole-inspector-log = Каротаж
borehole-inspector-no-hole-inspected = Скважина не выбрана для просмотра
borehole-inspector-pick-file = Выбрать { $file }...
borehole-inspector-pick-file-again-show-its = Выберите { $file } снова, чтобы показать геофизику: страница браузера не может повторно открыть файл самостоятельно.
borehole-inspector-reading-geophysics-file-its-index = Чтение геофизического файла для построения индекса; ход выполнения показан в строке состояния.
borehole-inspector-strat = Страт.
borehole-inspector-summary = Сводка
canvas-circle-summary = Окружность | Слой: { $layer } | радиус { $radius }

## Canvas strings

canvas-not-selectable-closed-polyline = Нельзя выбрать | Выберите замкнутую полилинию
canvas-polyline-summary = Полилиния | Слой: { $layer } | Вершин: { $count }
canvas-surface-name = Поверхность | { $name }
canvas-trimmed = Подрезанная
cinematic-shadows-method = Тени в кинематографическом виде: { $method }

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = По объекту { $object_id } созданы откос и берма
cmd-bezier-replaced-polyline-span-first-last = Участок полилинии { $first }→{ $last } заменён { $count } промежуточными точками
cmd-bezier-vertices-first-last = Вершины с { $first } по { $last }
cmd-block-model-block-model-loader-disconnected-path = Загрузчик блочной модели отключился для { $path }
cmd-block-model-block-model-path-has-count = В блочной модели { $path } обнаружены переменные неподдерживаемого типа ({ $count } шт.), которые невозможно прочитать: { $names }
cmd-block-model-building-ore-mesh = Построение сетки руды…
cmd-block-model-could-not-create-block-model = Не удалось создать блочную модель: { $error }
cmd-block-model-could-not-decode-block-model = Не удалось декодировать цветовую переменную блочной модели «{ $variable }»: { $error }
cmd-block-model-created-block-model-name-ordinary = Блочная модель «{ $name }» создана методом обычного кригинга
cmd-block-model-failed-load-block-model-error = Не удалось загрузить блочную модель: { $error }
cmd-block-model-generated-ore-mesh-from-block = Сетка руды создана из блочной модели «{ $name }»
cmd-block-model-imported-block-model-source-path = Импортирован источник блочной модели { $path }
cmd-block-model-loaded-block-model-name-blocks = Загружена блочная модель «{ $name }»: блоков — { $blocks } (визуализируемых — { $renderable }), сетка { $dimx }×{ $dimy }×{ $dimz }, переменных — { $variables }
cmd-block-model-loading-name = Загрузка { $name }
cmd-block-model-loading-name-ellipsis = Загрузка { $name }…
cmd-chamfer-applied = Угол { $corner } скошен с радиусом { $radius } и числом сегментов { $segments }
cmd-chamfer-radius = Радиус { $radius }
cmd-commands-clipped = Обрезанная
cmd-commands-command-failed-error = Ошибка команды: { $error }
cmd-commands-count-control-string-s = Управляющих линий: { $count }
cmd-commands-count-control-string-s-layer = Управляющих линий на слое «{ $layer }»: { $count }
cmd-commands-count-point-s-across-layers = Точек: { $count } (слоёв: { $layers })
cmd-commands-count-point-s-layer = Точек на слое «{ $layer }»: { $count }
cmd-commands-kind-layer = { $kind } на слое «{ $layer }»
cmd-commands-no-control-strings = Нет управляющих линий
cmd-commands-no-extent = Нет границ
cmd-commands-select-holes-place-reference-points = Выберите скважины, на которых нужно разместить опорные точки
cmd-triangulate-needs-selection = Выберите объекты для триангуляции перед запуском команды «Создать триангуляцию»
cmd-commands-select-one-loaded-block-model = Выберите одну загруженную блочную модель, чтобы создать по ней триангуляцию руды
cmd-commands-select-one-loaded-drill-hole = Выберите один загруженный набор скважин, чтобы создать по нему блочную модель
cmd-commands-select-one-loaded-point-cloud = Выберите одно загруженное облако точек, чтобы создать по нему триангуляцию
cmd-contours-needs-triangulation = Выберите одну загруженную триангуляцию, чтобы создать по ней горизонтали
cmd-slice-needs-triangulation = Выберите одну загруженную триангуляцию, чтобы рассечь её по диапазону Z
cmd-commands-select-one-loaded-triangulation-one = Перед отсечением выберите одну загруженную триангуляцию и одну замкнутую полилинию
cmd-commands-select-one-more-objects-before = Перед заданием { $axis } выберите один или несколько объектов
cmd-commands-sliced = Рассечённая
cmd-contours-contour-generation-failed-error = Ошибка создания горизонталей: { $error }
cmd-contours-discarded-layer-exists = Горизонтали для «{ $name }» отброшены: слой «{ $layer_name }» уже существует
cmd-contours-discarded-project-closed = Горизонтали для «{ $name }» отброшены: проект закрыт
cmd-contours-discarded-layer-deleted = Горизонтали для «{ $name }» отброшены: выбранный выходной слой удалён
cmd-contours-generated = Создано горизонталей для триангуляции «{ $name }» в слое «{ $layer_name }»: { $line_count }
cmd-creation-assembled-boundary-rings = Из фрагментированных разомкнутых линий собрано замкнутых граничных контуров: { $assembled_count }
cmd-creation-created-triangulation-from-boundary = Создана триангуляция из замкнутых контуров ({ $boundary_count }) и разомкнутых ограничений ({ $constraint_count }), тип поверхности: { $surface_type }
cmd-creation-creating-triangulation = Создание триангуляции…
cmd-creation-generate-upper-surface-ignored-count = Создание верхней поверхности: пропущено конфликтующих нижних сегментов структурных линий: { $count }; исходные объекты не изменены
cmd-creation-ignored-objects = При триангуляции пропущено объектов, не являющихся полилиниями или являющихся вырожденными: { $rejected }
cmd-creation-weld-retry-moved-coarse-welded = Сварка и повтор: { $coarse_welded } вершин(ы) перемещено в общие позиции (до { $coarse_weld_tol } м); исходные объекты не изменены
cmd-creation-welded-breakline-vertices = Объединено совпадающих в пределах допуска вершин структурных линий: { $welded }
cmd-cuts-clipped-surface-name-polyline-mode = Поверхность «{ $name }» обрезана полилинией ({ $mode })
cmd-cuts-clipping-surface-polyline = Обрезка поверхности полилинией…
cmd-cuts-cut-topology-name-pit-shell = Топографическая поверхность «{ $name }» вырезана по оболочке карьера
cmd-cuts-cut-triangulation-name-z-band = Триангуляция «{ $name }» обрезана по диапазону Z [{ $min }, { $max }]
cmd-cuts-cutting-topology-pit-shell = Вырезание топографической поверхности по оболочке карьера…
cmd-cuts-cutting-triangulation-z = Обрезка триангуляции по Z…
cmd-cuts-ignored-vertical-faces = Пропущено вертикальных или вырожденных граней опорной топологии без площади в XY: { $count }
cmd-cuts-site-skipped-constraint-from-x = { $site }: пропущено ограничение ({ $from_x }, { $from_y }) → ({ $to_x }, { $to_y }), которое триангулятор не смог разделить
cmd-cuts-skipped-degenerate-edges = { $site }: пропущено почти вырожденных рёбер ограничений: { $skipped }; рядом с ними граница разреза может отличаться на незначительную величину
cmd-cuts-trimmed-surface = Поверхность «{ $surface }» подрезана по топографической поверхности «{ $topology }» ({ $mode })
cmd-cuts-trimming-surface-topology = Подрезка поверхности по топографической поверхности…
cmd-drape-draped-intersected-vertices-changed = Спроецировано вершин: { $intersected }; изменена отметка у { $changed }
cmd-drape-no-intersections = Ни одна из выбранных проектных вершин не пересекает выбранные топологии
cmd-drape-objects-changed-object-s-changed = Изменено объектов: { $objects } · перемещено пересекающихся вершин: { $changed } из { $intersected }
cmd-drape-select-one-more-design-objects = Выберите один или несколько проектных объектов для наложения
cmd-drape-select-one-more-topologies-drape = Выберите одну или несколько топологий для наложения
cmd-drape-selected-topologies-no-longer-loaded = Выбранные топологии больше не загружены
cmd-drill-hole-choose-drillhole-source-files-again = Выберите исходные файлы скважин заново
cmd-drill-hole-drill-pattern-too-large-contains = Сетка скважин слишком велика или содержит недопустимые координаты устьев
cmd-drill-hole-drillhole-field-label-has-count = Поле скважин «{ $label }» содержит { $count } различных кодов, что больше, чем обычно бывает у кодированного поля; похоже, это произвольный текст, а не категориальное поле, но все коды сохранены и раскрашены
cmd-drill-hole-enter-name-drill-pattern = Введите имя сетки скважин
cmd-drill-hole-failed-load-drillholes-error = Не удалось загрузить скважины: { $error }
cmd-drill-hole-depth-must-be-positive = Глубина скважины должна быть больше нуля
cmd-drill-hole-diameter-must-be-positive = Диаметр скважины должен быть больше нуля
cmd-drill-hole-loaded-drillhole-dataset-name-holes = Загружен набор скважин «{ $name }»: скважин — { $holes }, цветовых полей — { $fields }
cmd-drill-hole-name-already-loading = «{ $name }» уже загружается
cmd-drill-hole-name-reason = «{ $name }»: { $reason }
cmd-drill-hole-name-side = { $name } { $side }
cmd-drill-hole-name-working-section-side = { $name }, рабочая пачка { $side }
cmd-drill-hole-no-hole-holds-value-field = Ни в одной скважине в этом поле нет значения «{ $value }»
cmd-drill-hole-only-mapped-csv-bundles-imported = В браузере импортируются только пакеты CSV с заданным сопоставлением
cmd-drill-hole-pattern-contains-no-holes = Сетка не содержит скважин
cmd-drill-hole-reading-name = Чтение { $name }
cmd-drill-hole-reference-points-used-holes-placed = Опорные точки: размещено на скважинах — { $used }, без «{ $value }» — { $absent }, помечено как возможные повторы по сбросу — { $flagged }
cmd-drill-hole-uppermost-run-used-flagged-holes = Использован самый верхний интервал, помеченные скважины: { $holes }
cmd-drill-hole-working-section-name-not-same = Рабочая пачка «{ $name }» не одинакова во всех выбранных наборах; использована собственная пачка каждого набора.
cmd-drill-hole-working-sections-not-kept-dataset = Рабочие пачки не сохранены в «{ $dataset }». { $reasons }
cmd-explode-count-line-s = { $count } линий
cmd-explode-polyline = Разбить полилинию
cmd-explode-exploded-polyline-into-count-line = Полилиния разбита на { $count } отрезков
cmd-file-block-model-csv-encoding-failed = Ошибка кодирования CSV блочной модели: { $error }
cmd-file-block-model-csv-export-failed = Ошибка экспорта CSV блочной модели: { $error }
cmd-file-browser-recovery-unavailable = Файлы восстановления браузера недоступны; сохранённые проекты остаются в IndexedDB
cmd-file-closed-project-runtime-id-runtime = Закрыт проект с идентификатором среды выполнения { $runtime_id }
cmd-file-could-not-create-new-project = Не удалось создать новый проект: { $error }
cmd-file-could-not-finish-pending-project = Не удалось завершить ожидающее действие проекта: { $error }
cmd-file-could-not-finish-saving-before = Не удалось завершить сохранение перед выходом: { $error }
cmd-file-could-not-open-browser-project = Не удалось открыть проект в браузере: { $error }
cmd-file-could-not-open-path-error = Не удалось открыть { $path }: { $error }
cmd-file-could-not-read-selected-file = Не удалось прочитать выбранный файл: { $error }
cmd-file-could-not-reload-layer-from = Не удалось повторно загрузить слой с диска: { $error }
cmd-file-could-not-reload-project-from = Не удалось повторно загрузить проект с диска: { $error }
cmd-file-could-not-remove-browser-project = Не удалось удалить проект браузера: { $error }
cmd-file-could-not-restore-layer-from = Не удалось восстановить слой из проекта: { $error }
cmd-file-could-not-snapshot-dirty-project = Не удалось создать снимок изменённого проекта для восстановления: { $error }
cmd-file-could-not-start-browser-export = Не удалось начать экспорт в браузере: { $error }
cmd-file-could-not-write-recovery-copies = Не удалось записать резервные копии: { $error }
cmd-file-created-new-browser-project = Создан новый проект в браузере
cmd-file-created-new-project = Создан новый проект
cmd-file-description-download-failed-error = Ошибка загрузки «{ $description }»: { $error }
cmd-file-discard-cancelled-project-changed = Отмена изменений отменена, поскольку проект изменился во время повторной загрузки OMF
cmd-file-discarded-changes-layer-target-name = Изменения слоя «{ $target_name }» отменены
cmd-file-discarded-changes-reloaded-path = Изменения отменены: { $path } загружен повторно
cmd-file-downhole-geophysics-csv = CSV скважинной геофизики
cmd-file-downloaded-description-file-name = Загружено — { $description }: { $file_name }
cmd-file-drillhole-csv-export-failed-error = Ошибка экспорта CSV скважин: { $error }
cmd-file-dxf-download-encoding-failed-error = Ошибка кодирования загружаемого DXF: { $error }
cmd-file-dxf-import-failed-error = Ошибка импорта DXF: { $error }
cmd-file-encoding-block-model-csv-download = Кодирование загрузки CSV блочной модели…
cmd-file-encoding-dxf-download = Кодирование загрузки DXF…
cmd-file-encoding-triangulation-download = Кодирование загрузки триангуляции…
cmd-file-exit-deferred-exports = Выход отложен до завершения фонового экспорта
cmd-file-exit-requested-no-unsaved-changes = Запрошен выход, несохранённых изменений нет
cmd-file-exported-block-model-csv-path = CSV блочной модели экспортирован в { $path }
cmd-file-exported-description-dxf-path = { $description } экспортирован в DXF: { $path }
cmd-file-exported-three-drillhole-csvs-path = Три файла CSV скважин экспортированы в { $path }
cmd-file-exported-triangulation-name-path = Триангуляция «{ $name }» экспортирована в { $path }
cmd-file-exporting-name = Экспорт { $name }…
cmd-file-exporting-triangulation-name-path = Экспорт триангуляции «{ $name }» в { $path }
cmd-file-fatal-renderer-failure-reason = Критический сбой средства визуализации: { $reason }
cmd-file-dialog-action-failed = Ошибка действия в диалоге файлов: { $msg }
cmd-file-imported-added-object-s-from = Импортировано объектов из { $name }: { $added }
cmd-file-imported-total-dxf-object-s = Импортировано объектов DXF: { $total }
cmd-file-layer-discard-was-cancelled-because = Отмена изменений слоя отменена, поскольку проект изменился во время повторной загрузки
cmd-file-no-recovery-directory = Каталог восстановления недоступен: { $error }
cmd-file-no-unsaved-project-content-nothing = Несохранённого содержимого проекта нет; восстанавливать нечего
cmd-file-parsing-browser-dxf-import = Разбор импорта DXF в браузере…
cmd-file-parsing-dxf-import = Разбор импорта DXF…
cmd-file-project-closes-after-save = Проект закроется после завершения текущего сохранения
cmd-file-the-project-closes-after-save = Проект закроется после завершения текущего сохранения
cmd-file-queued-count-triangulation-file-s = Файлов триангуляции в очереди на импорт: { $count }
cmd-file-recovery-copies-path-reopen-them = Копии восстановления находятся в { $path }; откройте их после перезапуска
cmd-file-recovery-copy-failed-error = Не удалось создать копию восстановления: { $error }
cmd-file-recovery-copy-failed-failure = Не удалось создать резервную копию: { $failure }
cmd-file-recovery-copy-written-path = Резервная копия записана: { $path }
cmd-file-reverting-layer = Откат слоя…
cmd-file-reverting-project = Откат проекта…
cmd-file-save-failed-message = Ошибка сохранения: { $message }
cmd-file-save-project-already-running-save = Сохранение этого проекта уже выполняется; повторите после его завершения
cmd-file-save-worker-ended-without-result = Процесс сохранения завершился без результата
cmd-file-saved-project-as = Проект сохранён как: { $path }
cmd-file-saved-project = Проект сохранён: { $path }
cmd-file-saving-browser-storage = Сохранение в хранилище браузера…
cmd-file-selected-block-model-no-longer = Выбранная блочная модель больше не загружена
cmd-file-selected-drillhole-dataset-no-longer = Выбранный набор скважин больше не загружен
cmd-file-switching-project = Переключение проекта…
cmd-file-triangulation-download-encoding-failed = Ошибка кодирования загружаемой триангуляции: { $error }
cmd-file-user-chose-exit-without-saving = Пользователь решил выйти без сохранения
cmd-file-user-requested-exit-project-export = Пользователь запросил выход (требуется подтверждение экспорта проекта или несохранённой работы)
cmd-file-viewport = Область просмотра
cmd-file-wait-current-project-save-finish = Дождитесь завершения сохранения текущего проекта
cmd-file-wait-current-project-switch-finish = Дождитесь завершения переключения текущего проекта
cmd-file-wait-project-operation-finish-before = Перед отменой изменений дождитесь завершения операции с проектом
cmd-file-wait-project-revert-finish-before = Перед сохранением дождитесь завершения отката проекта
cmd-folder-collection-named-name-already-exists = Коллекция «{ $name }» уже существует
cmd-folder-collection-no-longer-exists = Эта коллекция больше не существует
cmd-folder-created-collection-name = Создана коллекция «{ $name }»
cmd-folder-deleted-collection-name = Удалена коллекция «{ $name }»
cmd-folder-moved-item-into-collection-name = Элемент перемещён в коллекцию «{ $name }»
cmd-folder-moved-item-root-section = Элемент перемещён в корень раздела { $section }
cmd-folder-renamed-collection-before-after = Коллекция «{ $before }» переименована в «{ $after }»
cmd-folder-section-cannot-hold-item = Этот раздел не может содержать данный элемент
cmd-fuse-closed-polyline = замкнутая полилиния
cmd-fuse-count-source-line-s = { $count } исходных линий
cmd-fuse-created-shape-object-id-vertices = Создан объект { $shape } { $object_id } с { $vertices } вершинами из { $sources } исходных линий
cmd-fuse-click-missed = Объединение: щелчок не попал ни в один объект (под курсором ничего нет)
cmd-fuse-click-not-near-endpoint = Объединение: щелчок выполнен недостаточно близко к концам выбранной линии
cmd-fuse-clicked-closed-polyline = Объединение: выбранный объект { $object_id } является замкнутой полилинией; объединять можно только разомкнутые полилинии
cmd-fuse-clicked-not-open-polyline = Объединение: выбранный объект { $object_id } не является разомкнутой полилинией (тип: { $kind })
cmd-fuse-clicked-object-missing = Объединение: выбранный объект { $object_id } больше не существует
cmd-fuse-clicked-too-few-vertices = Объединение: выбранная полилиния { $object_id } содержит только { $count } вершин(ы); требуется не менее 2
cmd-fuse-endpoint-marker-missing = Объединение: маркер конечной точки { $marker_index } больше не существует
cmd-fuse-close-needs-three-vertices = Объединение: для замыкания линии в полилинию требуется не менее 3 различных вершин (сейчас { $count })
cmd-fuse-lines = Объединить линии
cmd-fuse-needs-two-segments = Объединение: для применения нужны как минимум 2 сегмента (имеется { $count })
cmd-fuse-no-active-layer = Объединение: нет активного слоя для размещения объединённой линии
cmd-fuse-no-active-project = Объединение: нет активного проекта, применение невозможно
cmd-fuse-no-source-line = Объединение: нет исходной линии для замыкания в полилинию
cmd-fuse-awaiting-object-invalid = Объединение: объект { $awaiting_id } больше не является допустимой полилинией
cmd-fuse-object-already-in-chain = Объединение: объект { $object_id } уже входит в цепочку; выберите другую линию
cmd-fuse-result-too-few-vertices = Объединение: в результате слишком мало вершин ({ $count }), операция отменена
cmd-fuse-segment-object-invalid = Объединение: объект сегмента { $object_id } больше не является допустимой полилинией; операция прервана
cmd-fuse-source-object-invalid = Объединение: исходный объект { $object_id } больше не является допустимой разомкнутой полилинией
cmd-fuse-source-object-missing = Объединение: исходный объект { $object_id } больше не существует
cmd-fuse-open-polyline = разомкнутая полилиния
cmd-include-failed = Ошибка включения: { $message }
cmd-include-included-solid-shape-name-topology = Тело «{ $shape_name }» включено в топологию «{ $topology_name }» (сохранено граней топологии: { $retained }, пропущено замыкающих граней: { $skipped })
cmd-include-including-pit-stockpile-solid = Добавление тела карьера/склада…
cmd-insert-point-count-operation-point-s = { $count } точек операции «{ $operation }»
cmd-insert-point-elevation-must-be-finite = Для вставки точки на отметке требуется конечное значение отметки
cmd-insert-point-insert-points = Вставить точки
cmd-insert-point-inserted-count-operation-point-s = Вставлено точек операции «{ $operation }»: { $count }
cmd-insert-point-intersection = Пересечение
cmd-insert-point-no-new-operation-points-were = Новые точки операции «{ $operation }» не найдены
cmd-insert-point-select-least-two-polylines-before = Перед вставкой точек пересечения выберите не менее двух полилиний
cmd-insert-point-select-one-more-polylines-before = Перед вставкой точки на отметке выберите одну или несколько полилиний
cmd-layer-created-layer-name = Создан слой «{ $name }»
cmd-layer-deleted-with-objects = Удалён слой { $layer_id } (и все объекты на нём)
cmd-layer-duplicated-layer-duplicate-name = Создана копия слоя «{ $duplicate_name }»
cmd-layer-locked = Заблокировано
cmd-layer-name-copy = копия { $name }
cmd-layer-selected-count-object-s-layer = Выбрано объектов в слое { $layer_id }: { $count }
cmd-layer-state-layer-name = { $state } слой «{ $name }»
cmd-layer-unlocked = Разблокировано
cmd-move-tool-moved-collars = Смещение ({ $delta }) применено к устьям скважин ({ $count })
cmd-move-tool-moved-objects = Смещение ({ $delta }) применено к { $count } объектам
cmd-move-tool-count-hole-s = Скважин: { $count }
cmd-object-edit-edited-kind = Изменён объект { $kind }
cmd-object-edit-edited-kind-count-vertices = Изменён объект { $kind } (вершин: { $count })
cmd-object-edit-no-changes-apply = Нет изменений для применения
cmd-object-edit-object-changed-since-editor-opened = Этот объект изменился с момента открытия редактора; откройте его заново, чтобы редактировать текущую версию
cmd-object-edit-target-changed = Редактируемый объект изменился; изменения отменены
cmd-object-edit-object-no-longer-exists-document = Этот объект больше не существует в документе
cmd-object-edit-select-single-design-object-edit = Выберите один объект проекта для редактирования
cmd-object-edit-unassigned = Не назначено
cmd-offset-create-offset = Создать смещение
cmd-offset-created-offset-count-object-s = Создано смещение { $count } объектов
cmd-offset-distance-must-be-positive = Расстояние смещения должно быть больше нуля
cmd-offset-skipped-count-circle-s-offset = Пропущено окружностей: { $count }; расстояние смещения больше радиуса
cmd-omf-could-not-open-project-source = Не удалось открыть проект { $source_name }: { $error }
cmd-omf-create-open-project-before-merging = Перед объединением данных создайте или откройте проект
cmd-omf-dataset-name-count-working-section = Набор «{ $name }»: не удалось восстановить рабочих пачек: { $count } ({ $details })
cmd-omf-field-codes-partly-coloured = Набор «{ $name }»: поле «{ $field }» сохранено с раскраской { $saved } кодов из { $total }; остальным назначены автоматические цвета.
cmd-omf-encoding-project = Кодирование проекта…
cmd-omf-exported-project-path = Проект экспортирован в { $path }
cmd-omf-imported-project = Импортирован проект «{ $project_name }» из { $source_name }: наборов данных верхнего уровня — { $count }
cmd-omf-importing-project = Импорт проекта…
cmd-omf-export-failed = Ошибка экспорта OMF: { $error }
cmd-omf-import-failed = Ошибка импорта OMF: { $error }
cmd-omf-opened-project = Открыт проект «{ $project_name }» из { $source_name }
cmd-omf-project-source-name-contains-no = Проект «{ $source_name }» не содержит поддерживаемых элементов данных
cmd-omf-source-name-applied-project-origin = { $source_name }: перед объединением применено начало координат проекта { $origin }
cmd-omf-crs-differs = { $source_name }: система координат «{ $source_crs }» отличается от системы координат проекта «{ $target_crs }»; координаты объединены без перепроецирования
cmd-omf-source-name-units-source-units = { $source_name }: единицы «{ $source_units }» отличаются от единиц проекта «{ $target_units }»; координаты объединены без преобразования
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = Нет открытых данных Incline Design для экспорта
cmd-placement-2-vertices = 2 вершины
cmd-placement-count-vertices = { $count } вершин
cmd-placement-created-circle = Создана окружность радиусом { $radius } м
cmd-placement-created-closed-polyline = Создана замкнутая полилиния с { $count } вершинами
cmd-placement-created-line-segment-2-vertices = Создан отрезок с 2 вершинами
cmd-placement-created-open-polyline-count-vertices = Создана разомкнутая полилиния с { $count } вершинами
cmd-placement-placed-point-x-y-z = Точка помещена в { $x }, { $y }, { $z }
cmd-placement-radius = Радиус { $radius } м
cmd-plot-composing-engineering-drawing = Формирование инженерного чертежа…
cmd-plot-could-not-write-engineering-drawing = Не удалось записать инженерный чертёж: { $error }
cmd-plot-drawing-scale-fitted-visible-data = Масштаб чертежа подобран по видимым данным: 1:{ $scale }
cmd-plot = Чертёж
cmd-plot-saved-drawing = Инженерный чертёж сохранён: { $description } ({ $width } × { $height } пкс при { $dpi } т/д)
cmd-point-cloud-classified = Классифицировано «{ $name }»: земля — { $ground }, растительность — { $vegetation }, шум — { $noise } из { $count } точек
cmd-point-cloud-classifying-point-clouds = Классификация облаков точек
cmd-point-cloud-join-dropped-classifications = Классификация точек отброшена: часть объединяемых облаков не классифицирована, а частично классифицированное облако нельзя отфильтровать по земле.
cmd-point-cloud-failed-classify-point-clouds-error = Не удалось классифицировать облака точек: { $error }
cmd-point-cloud-failed-join-point-clouds-error = Не удалось объединить облака точек: { $error }
cmd-point-cloud-failed-load-point-cloud-error = Не удалось загрузить облако точек: { $error }
cmd-point-cloud-joined-count-clouds-into-name = Облаков объединено: { $count } → «{ $name }» (точек: { $points })
cmd-point-cloud-joining-name = Объединение: { $name }
cmd-point-cloud-loaded-point-cloud-name-count = Загружено облако точек { $name } ({ $count } точек)
cmd-point-cloud-point-cloud-classification-discarded = Классификация облака точек отброшена: облако изменилось во время выполнения. Запустите её снова.
cmd-point-cloud-point-cloud-loader-disconnected-path = Загрузчик облака точек отключился для { $path }
cmd-point-cloud-select-one-more-loaded-point = Выберите одно или несколько загруженных облаков точек для классификации
cmd-point-cloud-select-two-more-loaded-point = Выберите два или несколько загруженных облаков точек для объединения
cmd-point-cloud-tin-max-edge-disabled = (макс. ребро отключено)
cmd-point-cloud-tin-max-edge-max-edge = (макс. ребро { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = Ошибка создания TIN облака точек: { $error }
cmd-point-cloud-tin-filtered-ground = TIN рельефа: отфильтровано до { $ground } точек земли из { $total }
cmd-point-cloud-tin-subsampled = TIN рельефа: пространственная выборка { $sampled } из { $total } точек
cmd-point-cloud-tin-triangulated = ЦМР: триангулировано уникальных точек XY: { $vertex_count }; создано граней: { $face_count }{ $suffix }
cmd-products-added-product-delay-ms-ms = Добавлен продукт { $delay_ms } мс { $name }
cmd-products-deleted-product-delay-ms-ms = Удалён продукт { $delay_ms } мс { $name }
cmd-products-failed-save-products-error = Не удалось сохранить продукты: { $error }
cmd-products-product-no-longer-palette = Этого продукта больше нет в палитре
cmd-property-action-count-object-s-layer = { $action } { $count } объект(а/ов) на слой { $layer }
cmd-property-batch-set-axis-value-count = Пакетная установка значения { $axis } для { $count } объектов
cmd-property-batch-set-closed-count-polyline = Пакетная установка замкнутости для { $count } полилиний
cmd-property-batch-set-color-count-object = Пакетная установка цвета для { $count } объектов
cmd-property-batch-set-fill-style-count = Пакетная установка стиля заливки для { $count } объектов
cmd-property-batch-set-line-weight-count = Пакетная установка толщины линии для { $count } полилиний
cmd-property-copied = Скопировано
cmd-property-moved = Перемещено
cmd-raster-draped = Растр { $raster } наложен на триангуляцию { $triangulation } (пересекающиеся границы)
cmd-raster-failed-load-raster-name-error = Не удалось загрузить растр { $name }: { $error }
cmd-raster-failed-load-raster-path-error = Не удалось загрузить растр { $path }: { $error }
cmd-raster-loaded-raster-name-via-driver = Загружен растр { $name } через { $driver } ({ $srcx }×{ $srcy }, предпросмотр { $prevx }×{ $prevy })
cmd-raster-no-overlapping-triangulation = Ни одна загруженная триангуляция не перекрывает границы { $name }
cmd-raster-loader-disconnected = Загрузчик растра отключился для { $path }
cmd-raster-undraped = Растры сняты с { $count } триангуляций
cmd-reference-surface-build-surface-failed-error = Ошибка построения поверхности: { $error }
cmd-reference-surface-building-surface = Построение поверхности…
cmd-reference-surface-built-surface-name-from-vertex = Построена поверхность «{ $name }» по { $vertex_count } точкам: граней — { $face_count }, габарит по Z от { $low } до { $high }{ $support }{ $coincident }{ $controls }
cmd-reference-surface-control-string-index-could-not = Управляющую линию { $index } не удалось добавить в сетку
cmd-reference-surface-control-string-index-crosses-itself = Управляющая линия { $index } пересекает сама себя в плане
cmd-reference-surface-control-string-index-doubles-back = Управляющая линия { $index } возвращается по самой себе в плане
cmd-reference-surface-control-string-index-ends-where = Управляющая линия { $index } заканчивается там, где начинается; замкните её, чтобы использовать как маску
cmd-reference-surface-control-string-index-has-count = В управляющей линии { $index } различных вершин: { $count }; для управляющей линии нужно не менее { $minimum }
cmd-reference-surface-control-string-index-has-non = Управляющая линия { $index } содержит нечисловые координаты
cmd-reference-surface-control-string-index-no-longer = Управляющая линия { $index } больше недоступна
cmd-reference-surface-control-string-index-runs-along = Управляющая линия { $index } идёт вдоль линии границ в плане; это пока не поддерживается
cmd-reference-surface-control-string-overrides-pick-x = Управляющая линия переопределяет выбранную точку ({ $x }, { $y }): выбрано { $pick } м, по линии { $control } м, разница { $difference } м
cmd-reference-surface-control-strings-b-disagree-x = Управляющие линии { $a } и { $b } расходятся в точке ({ $x }, { $y }): { $za } м против { $zb } м, разница { $difference } м
cmd-reference-surface-control-strings-b-run-along = Управляющие линии { $a } и { $b } идут вдоль друг друга в плане; это пока не поддерживается
cmd-reference-surface-count-control-string-s-vertices = ; управляющих линий: { $count }, вершин: { $vertices }{ $crossings }
cmd-reference-surface-count-point-s-inside-extent = Точек внутри границ: { $count }; для поверхности нужно не менее { $minimum }
cmd-reference-surface-count-point-s-outside-extent = ; точек вне границ, сформировавших поверхность как опорные: { $count }
cmd-reference-surface-count-point-s-selected-surface = Выбрано точек: { $count }; для поверхности нужно не менее { $minimum }
cmd-reference-surface-count-point-s-shared-plan = ; точек с общим положением в плане, учтённых один раз: { $count }
cmd-reference-surface-delaunay-insert-failed-error = Ошибка вставки в триангуляцию Делоне: { $error }
cmd-reference-surface-extent-must-closed-string = Граница должна быть замкнутой линией
cmd-reference-surface-extent-string-crosses-itself-plan = Линия границы пересекает сама себя в плане
cmd-reference-surface-extent-string-has-non-finite = Линия границы содержит нечисловые координаты
cmd-reference-surface-extent-string-needs-least-three = Линия границы должна содержать не менее трёх различных вершин
cmd-reference-surface-extent-string-no-longer-available = Линия границы больше недоступна
cmd-reference-surface-meeting-count-crossing-s = пересечений: { $count }
cmd-reference-surface-and-more = , … и ещё { $more }
cmd-reference-surface-no-mask-selected-surface-unclipped = Маска не выбрана; поверхность не обрезана
cmd-reference-surface-no-part-surface-falls-inside = Ни одна часть поверхности не попадает в границы
cmd-reference-surface-open-project-before-building-surface = Откройте проект перед построением поверхности
cmd-reference-surface-points-collinear-plan-surface-needs = Точки лежат на одной прямой в плане; для поверхности нужны три точки не на одной прямой
cmd-reference-surface-select-exactly-one-closed-string = Выберите ровно одну замкнутую линию для обрезки поверхности
cmd-reference-surface-selected-point-has-non-finite = Выбранная точка имеет нечисловые координаты
cmd-reference-surface-selected-points-span-count-layers = Выбранные точки находятся на слоях: { $count }; поверхность помещена в раздел { $section }
cmd-relimit-click-missed = Изменение границы: щелчок не попал ни в один объект (под курсором ничего нет)
cmd-relimit-click-ignored = Изменение границы: щелчок проигнорирован, инструмент сейчас не ожидает выбора цели
cmd-relimit-clicked-source-line = Изменение границы: выбрана сама исходная линия, выберите другую
cmd-relimit-no-source-line = Изменение границы: исходная линия не задана, выбор отменён
cmd-relimit-relimited-line-source-id-selected = Линия { $source_id } переограничена по выбранной цели
cmd-relimit-resized-line-source-id-using = Размер линии { $source_id } изменён в режиме { $mode } со значением { $value }
cmd-rename-item-no-longer-belongs-active = Этот элемент больше не относится к активному проекту
cmd-rename-renamed-before-name = «{ $before }» переименовано в «{ $name }»
cmd-rename-renamed-name-taken = «{ $before }» переименовано в «{ $name }» (имя «{ $requested }» уже занято)
cmd-rotate-collar-turned-count-drillhole-collar-s = Повернуто скважин: { $count } ({ $rotation })
cmd-section-verb-count-item-s-section = { $verb }: элементов в разделе «{ $section }» — { $count }
cmd-selection-delete-vertex = Удалить вершину
cmd-selection-deleted-count-selected-object-s = Удалено выбранных объектов: { $count }
cmd-selection-deleted-vertex = Вершина { $vertex } удалена из полилинии { $object_id }
cmd-selection-duplicate-selection = Дублировать выбранное
cmd-selection-duplicated-count-object-s = Дублировано объектов: { $count }
cmd-session-created-triangulation = Создана триангуляция «{ $name }» (вершин — { $vertex_count }, граней — { $face_count }) из поверхности типа { $surface_type }
cmd-session-deleted-triangulation = Триангуляция «{ $name }» удалена из проекта
cmd-session-failed-load-triangulation-error = Не удалось загрузить триангуляцию: { $error }
cmd-session-failed-load-triangulation-message = Не удалось загрузить триангуляцию: { $message }
cmd-session-loaded-triangulation = Загружена триангуляция «{ $name }» ({ $path }, вершин — { $vertex_count }, граней — { $face_count })
cmd-session-set-triangulation-tri-id-color = Цвет триангуляции { $tri_id } задан как { $color }
cmd-session-triangulation-load-no-result = Загрузка триангуляции { $path } завершилась без результата
cmd-session-triangulation-failed = Ошибка операции с триангуляцией: { $message }
cmd-session-unloaded-triangulation-name = Триангуляция «{ $name }» выгружена
cmd-slice-entered-slice-view-cx-cy = Включён режим сечения в точке { $cx }, { $cy }, { $cz } вдоль { $dx }, { $dy } (длина линии { $length } м)
cmd-slice-exited-slice-view = Режим сечения закрыт
cmd-slice-reset-section-view-fit-extents = Сбросить вид сечения (вписать в границы)
cmd-slice-set-section-grid-enabled = Сетка сечения включена = { $enabled }
cmd-split-created-2-open-polylines = Созданы 2 разомкнутые полилинии
cmd-split-line = Разделить линию
cmd-split-points-needs-interior-vertex = Разделение по точкам: выберите внутреннюю вершину разомкнутой линии
cmd-split-polyline-into-two = Исходная полилиния разделена на две разомкнутые полилинии
cmd-text-edit-finished = Редактирование текста объекта { $object_id } завершено
cmd-text-updated = Текст объекта { $object_id } обновлён
cmd-view-centre-rotation-not-available-flying = Центр вращения недоступен в режиме полёта
cmd-view-fixed-centre-rotation-x-y = Центр вращения закреплён в точке { $x }, { $y }, { $z }
cmd-view-no-point-under-cursor-fix = Под курсором нет точки, на которую можно закрепить центр вращения
cmd-view-released-centre-rotation = Центр вращения освобождён
cmd-view-reset-view-fit-extents = Сбросить вид (вписать в границы)
cmd-view-reset-view-plan-same-distance = Сбросить вид (вид в плане с того же расстояния; щёлкните ещё раз, чтобы вписать в границы)
cmd-view-set-cinematic-view-enabled = Кинематографический вид = { $enabled }
cmd-view-set-topology-wireframes-enabled = Каркас топологии = { $enabled }
cmd-view-set-view-points-enabled = Отображение точек = { $enabled }
cmd-view-set-xy-grid-enabled = Сетка XY включена = { $enabled }
cmd-view-zoom-extents-preserving-angle = Масштабировать до границ (с сохранением угла)

## Common strings

common-add-product = Добавление продукта
common-appearance = Внешний вид...
common-background = Фон
common-block-model = Блочная модель
common-block-models = Блочные модели
common-borehole-inspector = Инспектор скважин
common-build-surface = Построить поверхность
common-build-surface-ellipsis = Построить поверхность...
common-cancelled = Отменено
common-chamfer = Фаска
common-choose = Выберите...
common-circle = Окружность
common-classify = Классифицировать
common-classify-point-clouds = Классификация облаков точек
common-click-point-fix-centre-rotation = Щёлкните точку, чтобы закрепить на ней центр вращения
common-clip-surface-polyline = Отсечь поверхность полилинией...
common-closed = Закрыто
common-collection = Коллекция
common-colour = Цвет
common-confirm-omf-rewrite = Подтвердить перезапись OMF
common-could-not-replace-current-project = Не удалось заменить текущий проект: { $error }
common-count-object-s = Объектов: { $count }
common-create = Создать
common-create-batter-berm = Создать уступ и берму
common-create-bezier-curve = Создать кривую Безье
common-create-block-model = Создать блочную модель
common-create-block-model-ellipsis = Создать блочную модель...
common-create-circle = Создать круг
common-create-drill-pattern = Создать сетку скважин
common-create-layer = Создать слой
common-create-line = Создать линию
common-create-ore-triangulation = Создать триангуляцию руды
common-create-ore-triangulation-ellipsis = Создать триангуляцию руды...
common-create-point = Создать точку
common-create-polyline = Создать полилинию
common-create-triangulation = Создать триангуляцию...
common-crosses = Кресты
common-cut = Вырезать
common-cut-topology-pit-shell = Вырезать топологию оболочкой карьера...
common-delete-collection = Удалить коллекцию
common-delete-layer = Удалить слой
common-delete-product = Удаление продукта
common-delete-selection = Удалить выбор
common-designs = Проектные объекты
common-discard-layer-changes = Отбросить изменения слоя
common-down = Вниз
common-drape-topology = Наложить на топологию
common-easting = Восток
common-edit-object = Изменить объект
common-edit-text = Редактировать текст
common-elevation = Отметка
common-exit-without-saving = Выйти без сохранения
common-export-engineering-drawing = Экспорт инженерного чертежа
common-file-was-left-out-downhole = Файл { $file } исключён из скважинной геофизики: { $error }
common-filter = Фильтр
common-fly-mode = Режим полёта
common-generate-contour-lines = Создать линии горизонталей...
common-hide-all = Скрыть все
common-hide-selection = Скрыть выбор
common-hole-id = ID скважины
common-ignore = Игнорировать
common-import-csv-block-model = Импорт блочной модели CSV
common-import-dxf = Импорт DXF
common-incline-design-project = Проект Incline Design
common-join = Объединить...
common-join-point-clouds = Объединение облаков точек
common-joined-cloud = Объединённое облако
common-layer = Слой
common-legend = Легенда
common-line = Линия
common-line-weight = Толщина линии
common-link-geophysics = Связать геофизику...
common-load-drillholes-before-linking-geophysics = Загрузите набор скважин перед связыванием с ним геофизики
common-lock-all = Заблокировать все
common-lock-selection = Заблокировать выбор
common-m = m
common-max = Макс
common-merge-shell-into-topology = Слить оболочку с топологией
common-merge-shell-into-topology-ellipsis = Слить оболочку с топологией...
common-modelling = Моделирование
common-move-collar = Переместить устье
common-move-collection = Переместить в коллекцию
common-move-design = Переместить объект
common-move-selection = Переместить выбор
common-name-has-no-readable-size = У «{ $name }» нет читаемого размера
common-new-product = Новый продукт
common-no-block-models = Нет блочных моделей
common-no-design-layers = Нет проектных слоёв
common-no-drill-holes = Нет скважин
common-no-file-chosen = Файл не выбран
common-no-open-project = Нет открытого проекта
common-no-point-clouds = Нет облаков точек
common-no-triangulations = Нет триангуляций
common-none = Нет
common-northing = Север
common-offset = Смещение
common-open = Открыто
common-orientation = Ориентация
common-point = Точка
common-point-cloud = Облако точек
common-point-clouds = Облака точек
common-polyline = Полилиния
common-polyline-layer = Полилиния на слое «{ $layer }»
common-project = Проект
common-rasters = Растры
common-redo = Повторить
common-reference-points = Опорные точки...
common-relimit-line = Линия изменения границы
common-remove-project = Удалить проект
common-reset-view = Сбросить вид
common-reveal-all = Показать все
common-reveal-finder = Показать в Finder
common-rotate-collar = Повернуть скважину
common-save-exit = Сохранить и выйти
common-scale-bar = Линейка масштаба
common-set-initiation-point = Задать точку инициирования
common-shape = Форма
common-shell = С оболочкой
common-slashes = Косые черты
common-slice = Срез
common-slice-triangulation-z-range = Срез триангуляции по диапазону Z...
common-surface-contours = Горизонтали поверхности
common-text = Текст
common-degree-suffix = °
common-tie-holes = Связать скважины
common-triangulations = Триангуляции
common-trim-topology = Обрезать по топологии...
common-undo = Отменить
common-undrape-all = Убрать наложение со всех
common-uniform-white = Однородный белый
common-unknown = Неизвестно
common-unlock-all = Разблокировать все
common-untitled = Без названия
common-up = Вверх
common-vertical-exaggeration = Вертикальное преувеличение
common-x = x
common-zoom-extents = Показать всё

## Confirmations strings

confirmations-close-project-unsaved-changes = Закрытие проекта: несохранённые изменения
confirmations-close-without-saving = Закрыть без сохранения
confirmations-delete = Удалить
confirmations-delete-objects = Удалить объекты
confirmations-discard = Отбросить
confirmations-discard-all-unsaved-changes-layer =
    Отменить все несохранённые изменения в слое «{ $name }»?
    Сохранённый слой будет повторно загружен с диска, а изменения в других слоях сохранятся. Это действие нельзя отменить.
confirmations-discard-all-unsaved-changes-name =
    Отменить все несохранённые изменения в «{ $name }»?
    Последняя сохранённая версия будет повторно загружена с диска. Это действие нельзя отменить.
confirmations-discard-changes = Отбросить изменения
confirmations-exit-unsaved-changes = Выход: несохранённые изменения
confirmations-incline-design-cannot-reproduce-all = Incline Design не может полностью воспроизвести содержимое исходного OMF. При сохранении будет исключено следующее:
confirmations-product = Продукт
confirmations-project = этот проект
confirmations-remove-name-delete-its-browser = Удалить «{ $name }» и сохранённую в браузере копию? Несохранённые изменения будут потеряны.
confirmations-remove-project-unsaved-changes = Удаление проекта: несохранённые изменения
confirmations-remove-without-saving = Удалить без сохранения
confirmations-replace-project-unsaved-changes = Замена проекта: несохранённые изменения
confirmations-save = Сохранить
confirmations-save-anyway = Всё равно сохранить
confirmations-save-changes-current-project-before = Сохранить изменения в текущем проекте перед его заменой?
confirmations-save-changes-name-before-closing = Сохранить изменения в «{ $name }» перед закрытием?
confirmations-save-changes-name-before-removing = Сохранить изменения в «{ $name }» перед удалением из Incline Design?
confirmations-save-close = Сохранить и закрыть
confirmations-save-modified-project-before-exiting = Сохранить измененный проект перед выходом?
confirmations-save-to-browser-before-exit = Сохранить измененный проект в хранилище браузера перед выходом?
confirmations-save-remove = Сохранить и удалить

## Console strings

console-copy-all = Копировать всё
console-copy-message = Копировать сообщение
console-error = ОШИБКА
console-info = ИНФО
console-no-console-activity-yet = В консоли пока нет событий
console-pending = ОЖИДАНИЕ
console-progress-summary = Выполняется · { $summary }
console-success = УСПЕШНО
console-warn = ПРЕДУПРЕЖДЕНИЕ

## Csv strings

csv-block-model-category = Категория
csv-block-model-value = Значение
csv-drill-hole-rows-for-undefined-holes = Строк для скважины, не определённой геометрией пакета: { $count }
csv-drill-hole-count-rows-were-skipped-total = Всего пропущено строк: { $count }
csv-drill-hole-csv-file-empty = Файл CSV пуст
csv-drill-hole-csv-has-too-many-unreadable = В CSV слишком много нечитаемых байтов для восстановления; вероятно, он в устаревшей кодировке — сохраните его как UTF-8 и импортируйте снова
csv-drill-hole-csv-header-has-no-columns = В заголовке CSV нет столбцов
csv-drill-hole-csv-headers-must-nonblank-unique = Заголовки CSV должны быть непустыми и уникальными
csv-drill-hole-geophysics-needs-geometry = Скважинной геофизике нужен файл устьев или явных сегментов в пакете, к скважинам которого она привязывается
csv-drill-hole-azimuth-out-of-range = В файле { $file } строк с азимутом вне диапазона от 0 до 360: { $count }
csv-drill-hole-dip-out-of-range = В файле { $file } строк с углом падения вне диапазона от -90 до 90: { $count }; эти строки прочитаны без направления
csv-drill-hole-file-inclination-values-could-angle = В файле { $file } все значения наклона, которые могли бы быть углом, не превышают нуля, поэтому столбец прочитан как угол падения (отрицательный вниз)
csv-drill-hole-file-maps-gamma-density-column = В файле { $file } столбец гамма или плотности сопоставлен дважды
csv-drill-hole-invalid-utf8 = Файл { $file } не является корректным UTF-8; нечитаемых байтов заменено: { $count }, ячеек: { $cells }; повреждённая ячейка не читается как данные
csv-drill-hole-file-requires-gamma-density-column = Файлу { $file } требуется столбец гамма или плотности
csv-drill-hole-row-undefined-hole = Строка { $row } файла { $file } относится к DHID «{ $dhid }» — скважине, не определённой геометрией пакета
csv-drill-hole-holes-hole-s-carry-overlapping = Скважины с перекрывающимися интервалами (например, пласт, записанный вместе с его пачками): { $holes } — { $summary }
csv-drill-hole-skipped-row-reason = Строка пропущена: { $reason }
csv-geophysics-above-5 = выше 5
csv-geophysics-below-0-5 = ниже 0,5
csv-geophysics-count-more = (+ ещё { $count })
csv-geophysics-count-rows-were-skipped-total = Всего пропущено строк в { $file }: { $count }
csv-geophysics-csv-has-record-longer-than = В CSV есть запись длиннее { $limit } МиБ: в файле нет переводов строк там, где они бывают в CSV, либо это не текст
csv-geophysics-csv-has-unterminated-quoted-field = В CSV есть незакрытое поле в кавычках
csv-geophysics-curve-file-was-left-out = Кривая { $curve } из { $file } исключена: большинство её показаний { $side }, поэтому медиана вне диапазона от 0,5 до 5 g/cc и единица измерения выглядит неверной (ожидаются g/cc). Incline не преобразует единицы; исправьте экспорт и свяжите файл снова
csv-geophysics-file-empty = Файл { $file } пуст
csv-geophysics-file-has-no-curve-no = В файле { $file } нет кривой: ни в одном столбце, кроме ID скважины и глубины, нет чисел
csv-geophysics-file-mapping-has-mapped-columns = Сопоставление для { $file } содержит столбцов: { $mapped }, в CSV их { $found }
csv-geophysics-file-no-longer-matches-its = Файл { $file } больше не соответствует своему индексу: свяжите его снова
csv-geophysics-file-not-grouped-hole-its = Файл { $file } не сгруппирован по скважинам: строки скважин разбросаны по слишком многим сериям. Отсортируйте его по ID скважины, затем по глубине и свяжите снова
csv-geophysics-file-requires-one-dhid-one = Файлу { $file } требуются один столбец DHID и один столбец глубины
csv-geophysics-row-blank-hole-id = В строке { $row } файла { $file } пустой ID скважины
csv-geophysics-row-column-count = В строке { $row } файла { $file } столбцов: { $found }; ожидалось { $expected }
csv-geophysics-row-negative-depth = В строке { $row } файла { $file } отрицательная глубина
csv-geophysics-row-no-depth = В строке { $row } файла { $file } нет читаемой глубины
csv-geophysics-file-s-path-not-valid = путь к файлу не является корректным UTF-8, а проект такой путь сохранить не может: переименуйте файл или его папку и свяжите файл снова
csv-geophysics-rows-skipped = { $file }: не удалось прочитать строк: { $skipped } из { $rows }; причины указаны в консоли
csv-geophysics-runs-not-grouped = Геофизика для скважин ({ $count }) состоит из нескольких серий и не сгруппирована по скважинам; каждая последующая серия добавляет только глубины, на которых у скважины нет показаний: { $holes }
csv-geophysics-linked-downhole-geophysics-from-file = Связана скважинная геофизика из { $file }: скважин — { $holes }, кривые { $curves }; прочитано строк — { $rows }, пропущено — { $skipped }. Показания остаются в файле и читаются по одной скважине
csv-geophysics-no-readings = нет показаний
csv-geophysics-no-usable-depth-step = нет пригодного шага по глубине
csv-geophysics-run-count-mismatch = Прочитано серий { $hole }: { $read }, в связи указано { $runs }
csv-geophysics-rows-geophysics-row-s-count = Строк геофизики: { $rows } для скважин ({ $count }), не определённых в наборе, — не связаны: { $holes }
csv-geophysics-rows-readings-would-need-samples = Для { $rows } показаний потребовалось бы отсчётов: { $samples }
csv-geophysics-run-hole-curve-was-not = Серия { $hole } { $curve } не сохранена ({ $reason })
data-table-copy-selection = Копировать выбранное
data-table-copy-table = Копировать таблицу
drill-hole-add = Добавить
drill-hole-add-all = Добавить все

## Drill strings

drill-hole-add-stop = Добавить остановку
drill-hole-add-working-section = Добавить рабочую пачку
drill-hole-all-rendered-intervals-opaque-white = Все прозрачные интервалы белые.
drill-hole-another-working-section-field-has = У другой рабочей пачки этого поля такое имя.
drill-hole-assumed = Предполагаемое
drill-hole-burden-spacing-must-greater-than = Расстояние между рядами и шаг должны быть больше нуля
drill-hole-cache-drill-hole-set-name-has = В наборе скважин «{ $name }» скважин и соединений: { $count }, что больше вместимости подсветки выбора ({ $capacity }): выбор набора целиком по-прежнему подсвечивает его, а выбор отдельных скважин — нет
drill-hole-cache-drill-hole-set-name-stations = Набор скважин «{ $name }»: станций — { $stations }, сегментов — { $before } объединено в { $after }, ячеек — { $cells }
drill-hole-choose-valid-closed-polyline = Выберите допустимую замкнутую полилинию
drill-hole-clear-filter = Очистить фильтр
drill-hole-code-already-in-section = Код { $code } уже входит в рабочую пачку { $section }.
drill-hole-code-outside-section-has-name = Код вне этой пачки имеет такое имя. Пачка может иметь общее имя только с кодом, который в неё входит.
drill-hole-colour-scale = Цветовая шкала
drill-hole-count-codes = Кодов: { $count }
drill-hole-count-codes-interval-no-logged = Кодов: { $count }. Интервал без записанного значения остаётся белым.
drill-hole-disc-diameter = Диаметр диска
drill-hole-appearance-title = Внешний вид скважин: { $name }
drill-hole-drilled-diameter = От диаметра бурения
drill-hole-every-code-lists-already-another = Все коды, которые она содержит, уже входят в другую рабочую пачку.
drill-hole-every-interval-value-colour-field = Каждый интервал со значением в поле цвета рисуется диском такой ширины на линии скважины. Вдали он никогда не становится уже нескольких пикселей.
drill-hole-field = Поле
drill-hole-field-working-section = { $field } по рабочим пачкам
drill-hole-floor = Почва
drill-hole-grayscale = Оттенки серого
drill-hole-green-yellow-red = Зелёный–жёлтый–красный
drill-hole-heat = Тепловая
drill-hole-drilled-width-help = Скважина с диаметром бурения выглядит как труба рядом с геологией; набор из тысяч скважин выглядит как сплошной ковёр.
drill-hole-line-width-help = Сама скважина рисуется линией такой ширины при любом масштабе.
drill-hole-however-far-eye-hole-drawn = Как бы далеко ни был наблюдатель, скважина рисуется не уже этого значения.
drill-hole-measured = Измерено
drill-hole-name-working-section = { $name } (рабочая пачка)
drill-hole-never-thinner-than = Не тоньше
drill-hole-new-section-name = Имя новой пачки
drill-hole-new-working-section = Новая рабочая пачка
drill-hole-no-holes-fit-inside-boundary = При текущем расстоянии между рядами и шаге внутри этой границы не помещается ни одной скважины
drill-hole-part-code = Часть кода
drill-hole-pattern-too-many-holes = Сетка превышает максимум в { $maximum } скважин; увеличьте расстояние между рядами или шаг
drill-hole-preset = Предустановка
drill-hole-px = пикс.
drill-hole-rainbow = Радуга
drill-hole-reset-colours = Сбросить цвета
drill-hole-reset-preset = Сбросить набор
drill-hole-reset-shown-colours = Сбросить показанные цвета
drill-hole-roof = Кровля
drill-hole-rotation-offsets-must-contain-valid = Поворот и смещения должны содержать допустимые числа
drill-hole-selected-polyline-has-no-usable = Выбранная полилиния не имеет пригодной площади в плоскости XY
drill-hole-shown-total-codes-shown = Показано кодов: { $shown } из { $total }
drill-hole-shown-total-rows-shown = Показано строк: { $shown } из { $total }
drill-hole-smooth-interpolation = Плавная интерполяция
drill-hole-spacing-would-scan-too-many = При таком шаге потребуется проверить слишком много ячеек сетки; увеличьте расстояние между рядами или шаг (максимум — { $maximum } скважин)
drill-hole-square = Прямоугольная
drill-hole-staggered = Шахматная
drill-hole-stepped-bands = Ступенчатые полосы
drill-hole-string-discs = Линия и диски
drill-hole-string-discs-where-intervals-overlap = Режим «Линия и диски»: где интервалы перекрываются, в виде диска рисуется самый короткий.
drill-hole-string-width = Ширина линии
drill-hole-style = Стиль
drill-hole-suggested-from-code-names-count = Предложено по названиям кодов ({ $count })
common-times-sign = ×
common-minus-sign = −
drill-hole-ticked-but-hidden-filter-count = Отмечено, но скрыто фильтром: { $count }
drill-hole-true-diameter = Истинный диаметр
drill-hole-unsupported-drillhole-source = Неподдерживаемый источник скважин
drill-hole-width = Ширина
drill-hole-working-section-needs-name = Рабочей пачке нужно имя.
drill-hole-working-section-set-seams-plies = Рабочая пачка — это набор пластов или слоёв, отрабатываемых как единое целое. Раскраска по ней задаёт один цвет всему набору.
drill-hole-working-sections = Рабочие пачки
drill-pattern-arrangement = Схема расположения
drill-pattern-axis-offset = Смещение по { $axis }
drill-pattern-blast-shape = Контур блока
drill-pattern-burden = Расстояние между рядами
drill-pattern-choose-closed-blast-boundary-then = Выберите замкнутую границу блока, затем настройте сетку. Скважины обновляются в окне просмотра в реальном времени.
drill-pattern-closed-design-polyline-whose-xy = Замкнутая проектная полилиния, чья проекция в плоскости XY будет заполнена скважинами.
drill-pattern-rotation-help = Поворот шаблона против часовой стрелки от глобальной оси { $axis }.
drill-pattern-distance-between-holes-along-each = Расстояние между скважинами вдоль каждого ряда сетки.
drill-pattern-name-hint = например, Западный блок 03
drill-pattern-diameter-help = Конечный диаметр скважины. Вводится в миллиметрах и сохраняется для каждой созданной скважины.
drill-pattern-hole-depth = Глубина скважины
drill-pattern-hole-diameter = Диаметр скважины
drill-pattern-move-over-closed-polyline-then = Наведите указатель на замкнутую полилинию и щёлкните её в окне просмотра. Esc отменяет выбор.
drill-pattern-name-help = Имя набора данных скважин, создаваемого в проекте.
drill-pattern-none-picked = Ничего не выбрано
drill-pattern-pattern-name = Имя сетки
drill-pattern-spacing-help = Перпендикулярное расстояние между рядами сетки.
drill-pattern-pick = Выбрать
drill-pattern-preview-count-hole-s-diameter = Предпросмотр: скважин — { $count } · диаметр — { $diameter } мм · глубина — { $depth } м
drill-pattern-rotation = Поворот
drill-pattern-shift-pattern-grid-along-global = Сдвигает сетку шаблона вдоль глобальной оси { $axis }, сохраняя её обрезку по форме взрыва.
drill-pattern-spacing = Шаг
drill-pattern-staggered-offsets-every-second-row = В шахматной схеме каждый второй ряд смещается на половину шага.
drill-pattern-vertical-depth-below-each-collar = Вертикальная глубина от каждого устья.

## Dxf strings

dxf-block-nesting-too-deep = Вложенность блоков DXF превышает максимальную глубину ({ $depth }); «{ $name }» пропущен
dxf-circular-block-reference = Обнаружена циклическая ссылка на блок DXF: «{ $name }»
dxf-undefined-layer = Объект DXF ссылался на неопределённый слой «{ $name }»; импортирован как «{ $fallback }»
dxf-import-budget-exceeded = Импорт DXF превышает бюджет { $what } ({ $limit }); оставшаяся геометрия пропущена
dxf-insert-unknown-block = DXF INSERT ссылается на неизвестный блок «{ $name }»

## Edit strings

edit-absolute-length = Абсолютная длина
edit-absolute-rl = Абсолютная RL
edit-action = Действия
edit-angle = Угол
edit-dip-help = Угол от горизонтали, отрицательный вниз: −90° — вертикальная скважина.
edit-app-web-not-recommended-production = Веб-версия { $app } не рекомендуется для производственного использования. Используйте её только для демонстрации.
edit-application = Применение
edit-apply = Применить
edit-apply-pick-target = Применить и выбрать цель
edit-axis-value = Значение { $axis }
edit-azimuth = Азимут
edit-batter-angle = Угол откоса уступа (°)
edit-azimuth-help = Направление бурения скважин в градусах по часовой стрелке от севера координатной сетки.
edit-bench-height = Высота уступа
edit-benches = Уступы
edit-berm-width = Ширина бермы
edit-bezier-curve = Кривая Безье
edit-choose-layer = Выберите слой
edit-measure-help = Выберите, что означает введённое значение: расстояние по склону, горизонтальную ширину или вертикальную высоту.
edit-choose-which-two-polyline-paths = Выберите один из двух путей полилинии между выбранными вершинами для замены. Длина учитывает высоту и криволинейные рёбра.
edit-click-corner-closed-polyline = Щёлкните по углу на замкнутой полилинии.
edit-click-open-closed-polyline-begin = Щёлкните по открытой или замкнутой полилинии, чтобы начать.
edit-click-second-vertex-replacement-span = Щёлкните вторую вершину участка замены.
edit-click-vertex-start-replacement-span = Щёлкните вершину, чтобы начать участок замены.
edit-collide-triangulation = Столкновение с триангуляцией
edit-confirm-selection = Подтвердить выбор
edit-control-point-1 = Контрольная точка 1
edit-control-point-2 = Контрольная точка 2
edit-copy = Копировать
edit-corner-radius-limited-so-replacement = Радиус угла, ограниченный так, чтобы замена не могла пройти через соседние вершины.
edit-create-new-layer = Создать новый слой
edit-create-new-project = Создать новый проект
edit-create-project = Создание проекта
edit-delta-length-m-use = Изменение длины (м, используйте + или -)
edit-dip = Угол падения
edit-direction = Направление
edit-distance = Расстояние
edit-distance-along-slope = Расстояние по склону
edit-download-free-native-version-our = Скачайте бесплатную нативную версию на нашем сайте ↗
edit-drill-hole = Скважина
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = Конец
edit-enter-valid-elevation = Введите действительную высоту.
edit-exit-slice = Выйти из режима среза
edit-finish-polyline = Завершить полилинию
edit-generate-batter-berms = Создать уступы и бермы
edit-height = Высота
edit-height-change = Перепад высоты
edit-height-mode = Режим высоты
edit-horizontal-distance = Горизонтальное расстояние
edit-horizontal-width-each-flat-berm = Горизонтальная ширина каждой плоской бермы между последовательными откосами уступов.
edit-hover-choose-which-end-move = Наведите курсор, чтобы выбрать, какой конец переместить, затем щёлкните для подтверждения.
edit-insert-point-elevation = Вставить точку на высоте
edit-intersect = Пересечение
edit-kind-properties = { $properties } объекта «{ $kind }»
edit-layer-name = Наименование слоя
edit-load-project = Загрузить проект
edit-longest = Самый длинный
edit-m-s = м/с
edit-measure = Измерение
edit-mit-license = Лицензия MIT
edit-mode = Режим
edit-move = Переместить
edit-move-layer = Переместить в слой
edit-move-which-end = Какой конец переместить
edit-movement-speed-slice-when-using = Скорость перемещения среза при использовании клавиш навигации.
edit-moving-end-endpoint = Перемещение: конечная точка
edit-moving-start-endpoint = Перемещение: начальная точка
edit-new-length-m = Новая длина (м)
edit-new-project = Новый проект
edit-number-complete-batter-berm-levels = Количество полных уровней откоса и бермы. Максимум ограничен самым глубоким уровнем, на котором сохраняется указанная геометрия.
edit-bezier-segments-help = Число отрезков для аппроксимации кривой между двумя выбранными вершинами.
edit-chamfer-segments-help = Количество прямых сегментов для аппроксимации скруглённого угла. Значение 1 создаёт прямую фаску.
edit-object = Объект
edit-offset-element = Смещаемый элемент
edit-pick-side = Выберите сторону
edit-pit = Карьер
edit-project-name = Название проекта
edit-properties = Свойства
edit-radius = Радиус
edit-recent = В последнее время
edit-relative = Относительно (+/-)
edit-elevation-mode-help = «Относительная» применяет изменение высоты ко всем точкам. «Абсолютная RL» проецирует все точки на одну заданную отметку.
edit-remove-from-list = Удалить из списка
edit-replace-path = Заменить путь
edit-rotate = Повернуть
edit-rotation-speed-slice-when-using = Скорость вращения среза при использовании клавиш Q и E.
edit-s = °/с
edit-segments = Сегменты
edit-segments-lying-elevation-ignored = Сегменты, лежащие на этой высоте, игнорируются.
edit-endpoint-help = Выберите изменяемую конечную точку; другая останется неподвижной.
edit-selected-holes-point-different-ways = Выбранные скважины направлены по-разному. Применение установит для всех эти углы.
edit-selected-start-end-point-moves = Выбранная начальная или конечная точка перемещается вдоль направления линии; противоположная точка остаётся неподвижной.
edit-set-axis = Задать { $axis }
edit-shortest = Кратчайший
edit-slice-view = Вид среза
edit-slope-angle-each-batter-face = Угол наклона каждого откоса уступа, измеренный от горизонтали.
edit-slope-angle-offset-positive-negative = Угол наклона смещения. Положительные и отрицательные углы перемещают копию выше или ниже исходного объекта при боковом смещении.
edit-speed = Скорость
edit-start = Начало
edit-stockpile = Склад
edit-stop-generated-offset-where-its = Остановить формируемое смещение там, где его путь впервые пересечёт видимую триангуляцию.
edit-target-rl = Целевая отметка
edit-text-colour-opacity = Цвет текста и непрозрачность.
edit-thickness-visible-slice-slab-centred = Толщина видимого слоя среза, центрированного по индикатору обзора.
edit-translation-axis-help = Расстояние сдвига вдоль мировой оси { $axis }.
edit-type = Тип
edit-type-direction-together-set-offset = Тип и направление вместе задают сторону смещения. Карьер + вверх и отвал + вниз смещаются наружу; карьер + вниз и отвал + вверх — внутрь.
edit-bench-direction-help = «Вверх» поднимает каждый уступ на высоту уступа, «Вниз» опускает его. При этом также меняется сторона смещения — см. «Тип».
edit-value-help = Значение интерпретируется с использованием выбранного режима измерения и высоты.
edit-vertical-rise-fall-each-bench = Вертикальный подъём или спуск каждого уступа до создания следующей бермы.
edit-bezier-control-point-1-help = Мировые координаты X, Y и Z первой контрольной точки Безье.
edit-bezier-control-point-2-help = Мировые координаты X, Y и Z второй контрольной точки Безье.

## Events strings

events-couldn-t-exit-error = Не удалось выйти: { $error }
events-couldn-t-save-error = Не удалось сохранить: { $error }
events-set-elevation = Задать отметку
events-set-elevation-from-cursor-hit = Отметка по точке курсора задана как Z { $z }
events-tool-not-available-section-view = Этот инструмент недоступен в виде сечения

## Explorer strings

explorer-clear-active-triangulation-texture = Очистить текстуру активной триангуляции
explorer-delete-from-project = Удалить из проекта
explorer-discard-changes = Отбросить изменения...
explorer-download = Скачать
explorer-drape-over-surface = Наложить на поверхность
explorer-draped-over-surface = Наложено на поверхность
explorer-duplicate = Дублировать
explorer-empty-collection = Пустая коллекция
explorer-face-colour = Цвет грани
explorer-id-block-model-id-source =
    ID: block-model:{ $id }{ $source }
    Цветовых переменных: { $count }
explorer-id-drill-holes-id-source =
    ID: drill-holes:{ $id }{ $source }
    Скважин: { $holes }
    Цветовых полей: { $fields }
explorer-id-point-cloud-id-source =
    ID: point-cloud:{ $id }{ $source }
    Точек: { $count }
explorer-raster-id =
    ID: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = ID: triangulation:{ $id }{ $source }
explorer-load = Загрузить
explorer-lock = Заблокировать
explorer-new-collection = Новая коллекция
explorer-no-collection = Без коллекции
explorer-select-all-objects = Выберите все объекты
explorer-source-name = Источник: { $name }
explorer-unload = Выгрузить
explorer-unlock = Разблокировать

## Files strings

files-automatic-colour = Автоматический цвет
files-automatic-rl-spacing = Автоматический шаг отметок
files-axis-scale-ratio = Коэффициент масштаба по { $axis }
files-ok = ОК
files-reset-scale = Сбросить на 1×
files-rl-grid-options = Параметры сетки отметок
files-rl-spacing = Шаг отметок
files-scales-z-distances-visually-without = Масштабирует Z-расстояния визуально, не изменяя сохранённые координаты.
files-thickness = Толщина
files-xy-grid-options = Параметры сетки XY
geophysics-checking-geophysics-files = Проверка геофизических файлов
geophysics-downhole-geophysics-name-could-not = Не удалось связать скважинную геофизику для «{ $name }»: { $error }
geophysics-file-changed = Геофизический файл изменился с момента индексации
geophysics-file-unreadable = Геофизический файл, связанный с «{ $name }», недоступен для чтения по пути { $path } ({ $error }); свяжите его снова через контекстное меню набора
geophysics-linked-changed-rereading = Геофизика, связанная с «{ $name }», изменилась с момента индексации; повторное чтение
geophysics-hole-has-size-mib-geophysics = У скважины { $hole } строк геофизики на { $size } МиБ — больше, чем читается для одной скважины
geophysics-hole-needs-size-mib-its = Скважине { $hole } для геофизики требуется { $size } МиБ — больше, чем осталось у браузера: выгрузите другие элементы, затем выгрузите и снова загрузите этот набор
geophysics-linking-geophysics-name = Связывание геофизики с «{ $name }»
geophysics-reading-geophysics-hole = Чтение геофизики для { $hole }
geophysics-web-could-not-read-name-error = Не удалось прочитать «{ $name }»: { $error }
geophysics-web-name-used-session-s-downhole = «{ $name }» используется для скважинной геофизики в этом сеансе

## Gpu strings

gpu-cache-block-model-surface-build-failed = Не удалось построить поверхность блочной модели: { $error }
gpu-cache-block-model-surface-build-worker = Поток построения поверхности блочной модели отключился
gpu-cache-block-model-surface-chunk-rejected = Фрагмент поверхности блочной модели отклонён до выделения памяти GPU: экземпляры={ $instances } байт, предел={ $limit } байт
gpu-cache-block-volume-worker-disconnected = Поток подготовки объёма блоков отключился
gpu-cache-translucent-volume-could-not-built = Не удалось построить полупрозрачный объём ({ $error }); эта блочная модель показана кубами.
gpu-cache-edge-chunk-rejected = Фрагмент рёбер триангуляции отклонён до выделения памяти GPU: экземпляры={ $instances } байт, предел={ $limit } байт
gpu-cache-triangulation-chunk-rejected = Фрагмент триангуляции GPU отклонён до выделения памяти: вершины={ $vertices } байт, индексы={ $indices } байт, предел={ $limit } байт
gpu-cache-triangulation-too-many-vertices = В триангуляции «{ $name }» { $count } вершин (> u32::MAX); её нельзя разбить на фрагменты для GPU
gpu-cache-triangulation-uploaded = Триангуляция «{ $name }» загружена в { $chunks } пространственных фрагментах (граней: { $faces })
i18n-active-language = Активный язык: { $language } (встроенные: { $bundled })
i18n-could-not-select-language-error = Не удалось выбрать язык: { $error }

## Init strings

init-gpu-adapter-vendor-name-backend = Видеоадаптер: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver = Драйвер видеоадаптера: { $driver } { $driver_info }
init-gpu-limits-max-buffer-size = Ограничения GPU: max_buffer_size={ $max_buffer_size } МиБ, max_storage_buffer_binding_size={ $max_storage_buffer_binding_size } МиБ, max_storage_buffers_per_shader_stage={ $max_storage_buffers_per_shader_stage }, max_uniform_buffer_binding_size={ $max_uniform_buffer_binding_size } КиБ, max_texture_dimension_2d={ $max_texture_dimension_2d }, max_bind_groups={ $max_bind_groups }
init-gpu-supports-maximum-buffer-size = Максимальный размер буфера видеоадаптера — { $size } МиБ; большие сцены могут отображаться не полностью
init-surface-present-mode = Режим представления поверхности: { $mode }
init-wgpu-error-continuing-error = Ошибка wgpu (работа продолжена): { $error }
input-could-not-read-name-error = не удалось прочитать { $name }: { $error }
input-could-not-slice-name-error = не удалось рассечь { $name }: { $error }
io-add-collar-file-explicit-segments = Добавьте файл устьев (или файл явных сегментов): скважинная геофизика привязывается к определённым в нём скважинам.

## Io strings

io-ascii-points-xyz-pts = Точки ASCII (.xyz, .pts)
io-attribute = Атрибут
io-blank-header = (пустые заголовки)
io-block-model = Блочная модель:
io-choose-file-purpose-map-its = Выберите назначение файла, чтобы сопоставить его столбцы.
io-choose-loaded-block-model = Выберите загруженную блочную модель
io-choose-loaded-dataset = Выберите загруженный набор
io-choose-loaded-layer = Выберите загруженный слой
io-choose-loaded-triangulation = Выберите загруженную триангуляцию
io-choose-purpose = Выберите назначение…
io-choose-source-file-files-import = Выберите исходный файл или файлы для импорта.
io-collar = Устье
io-column-mapping = Сопоставление столбцов
io-comma-separated-values-csv = Значения, разделённые запятыми (.csv)
io-csv-files = Файлы CSV
io-dataset = Набор:
io-default = По умолчанию
io-density-read-g-cc-exported = Плотность, читается как g/cc, как в экспорте. Кривая, медиана которой вне диапазона от 0,5 до 5 g/cc, исключается из импорта с предупреждением: её единица измерения выглядит неверной.
io-depth = Глубина
io-diameter = Диаметр
io-downhole-geophysics = Скважинная геофизика
io-drawing-exchange-format-dxf = Формат обмена чертежами (.dxf)
io-drill-holes = Скважины
io-east-x = Восток / X
io-elevation-z = Отметка / Z
io-end-x = Конечная X
io-end-y = Конечная Y
io-end-z = Конечная Z
io-explicit-segments = Явные сегменты
io-export = Экспорт
io-export-csv-block-model = Экспорт блочной модели CSV
io-export-csv-drillholes = Экспорт скважин в CSV
io-export-dxf = Экспорт DXF
io-export-one-layer = Экспорт одного слоя
io-export-open-mining-format-2 = Экспорт в Open Mining Format 2
io-export-ply = Экспорт PLY
io-export-stl = Экспорт STL
io-export-wavefront-obj = Экспорт Wavefront OBJ
io-gamma-api = Гамма (API)
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = Импорт
io-import-ascii-point-cloud = Импорт облака точек ASCII
io-import-drillhole-csv-bundle = Импорт пакета CSV скважин
io-import-geotiff = Импорт GeoTIFF
io-import-las-laz-point-cloud = Импорт облака точек LAS/LAZ
io-import-open-mining-format-2 = Импорт Open Mining Format 2
io-import-pcd-point-cloud = Импорт облака точек PCD
io-import-ply = Импорт PLY
io-import-stl = Импорт STL
io-import-wavefront-obj = Импорт Wavefront OBJ
io-inclination = Наклон
io-interval = Интервал
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-long-spaced-density-g-cc = Плотность длинного зонда (g/cc)
io-mapped-csv-bundle-csv = Сопоставленный пакет CSV (.csv)
io-measured-depth-down-hole-read = Измеренная глубина по стволу скважины, читается в метрах. Incline не преобразует единицы: их задаёт база данных, из которой экспортирован файл.
io-model-file = Файл модели
io-name-count-files = { $name } + файлов: { $count }
io-natural-gamma-read-api-units = Естественное гамма-излучение, читается в единицах API, как в экспорте.
io-no-csv-chosen = Файл .csv не выбран
io-no-csv-files-chosen = Файлы CSV не выбраны
io-no-dxf-chosen = Файл .dxf не выбран
io-no-omf-chosen = Файл .omf не выбран
io-north-y = Север / Y
io-open-mining-format-2-omf = Open Mining Format 2 (.omf)
io-ply = PLY (.ply)
io-point-cloud-data-pcd = Данные облака точек (.pcd)
io-projects = Проекты
io-short-spaced-density-g-cc = Плотность короткого зонда (g/cc)
io-source-file = Исходный файл
io-start-x = Начальная X
io-start-y = Начальная Y
io-start-z = Начальная Z
io-stl = STL (.stl)
io-triangulation = Триангуляция:
io-unmapped = Не сопоставлено
io-wavefront-obj = Wavefront OBJ (.obj)
io-writes-three-files-beside-name = Записывает три файла рядом с выбранным именем: устья, инклинометрия и интервалы — в столбцах, которые импортирует этот диалог.

## Jobs strings

jobs-background-task-poll-label-ended = Фоновая задача «{ $poll_label }» завершилась без результата
jobs-cancelled-label-its-project-no = Задача «{ $label }» отменена: её проект больше не активен
jobs-discarded-stale-result = Устаревший результат фоновой задачи «{ $poll_label }» отброшен, поскольку исходный объект изменился или был закрыт
jobs-drillhole-import = импорт скважин
log-traces-auto-from-hole = Авто, по этой скважине
log-traces-curve-no-reading = { $curve }: нет показаний
log-traces-curve-value-unit = { $curve }: { $value } { $unit }
log-traces-custom-range = Свой диапазон
log-traces-default-colour = Цвет по умолчанию
log-traces-density-scale = Шкала плотности
log-traces-depth-m = { $depth } м
log-traces-gamma = Гамма
log-traces-gamma-colour = Цвет гаммы
log-traces-gamma-scale = Шкала гаммы
log-traces-percentile-range-no-data = От 1-го до 99-го процентиля скважины, округлено наружу. Для этой скважины данных пока нет.
log-traces-percentile-range = От 1-го до 99-го процентиля скважины, округлено наружу: { $range }.
log-traces-long-density = Плотность длинного зонда
log-traces-long-density-colour = Цвет плотности длинного зонда
log-traces-min-max-unit = от { $min } до { $max } { $unit }
log-traces-reading = Чтение...
log-traces-short-density = Плотность короткого зонда
log-traces-short-density-colour = Цвет плотности короткого зонда

## Logging strings

logging-activity-completed = Операция завершена
logging-activity-started = Операция начата
logging-application-id-id = Идентификатор приложения: { $id }
logging-application-name = Имя приложения: { $name }
logging-application-startup = Запуск приложения
logging-build-target-os-architecture = Целевая платформа сборки: { $os }-{ $architecture }
logging-completed = Завершено
logging-count-messages = Сообщений: { $count }
logging-desktop-session-xdg-session-type = Сеанс рабочего стола: XDG_SESSION_TYPE={ $session }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
logging-initialising-incline-design = Инициализация Incline Design
logging-locale-environment = Языковая среда: LANG={ $lang }, LC_ALL={ $locale }, TZ={ $timezone }
logging-macos-session = Сеанс macOS: USER={ $user }, SHELL={ $shell }
logging-operating-system-gnu-linux = Операционная система: GNU / Linux
logging-operating-system-macos = Операционная система: macOS
logging-operating-system-microsoft-windows = Операционная система: Microsoft Windows
logging-pointer-width = Разрядность указателя: { $width } бит
logging-process-id-id = Идентификатор процесса: { $id }
logging-release-version = Версия выпуска: { $version }
logging-renderer = Средство визуализации
logging-rust-compiler-host = Хост компилятора Rust: { $host }
logging-system = Система
logging-system-error = Системная ошибка
logging-unknown = неизвестно
logging-windows-session-sessionname-session = Сеанс Windows: SESSIONNAME={ $session }, USERNAME={ $user }
logging-working = Выполняется…

## Mac strings

mac-cannot-install-macos-menu-bar = Нельзя установить строку меню macOS вне главного потока
mac-quit-app = Выйти из { $app }

## Main strings

main-incline-design-web-startup-failed = Не удалось запустить Incline Design Web: { $error }

## Menu strings

menu-count-files-selected = Выбрано файлов: { $count }

## Object strings

object-edit-appearance = Внешний вид
object-edit-arc-circle = Дуга и окружность
object-edit-arc-segments = Сегменты дуги
object-edit-bulge = Стрела прогиба
object-edit-bulge-arcs-horizontal-data-model = По модели данных дуги со стрелой прогиба горизонтальны: дуга изгибается в плане, а высота изменяется по прямой от одной вершины к следующей.
object-edit-centre-x = Центр X
object-edit-centre-y = Центр Y
object-edit-centre-z = Центр Z
object-edit-chord = Хорда
object-edit-colour-layer = Цвет по слою
object-edit-enter-number = Введите число
object-edit-follow-owning-layer-s-colour = Использовать цвет владеющего слоя вместо цвета, закреплённого за этим объектом.
object-edit-id = ID
object-edit-identity = Тождественное
object-edit-insert-after = Вставить после
object-edit-join-last-vertex-back-first = Соединяет последнюю вершину снова с первой.
object-edit-length = Длина { $length } м
object-edit-move-down = Переместить вниз
object-edit-move-up = Переместить вверх
object-edit-object-has-no-arc-segments = У этого объекта нет сегментов дуги.
object-edit-object-has-single-position = У этого объекта одна позиция.
object-edit-object-needs-least-required-vertices = Этому объекту требуется как минимум { $required } вершин
object-edit-one-more-properties-not-valid = Одно или несколько свойств не являются допустимым числом
object-edit-perimeter-area = Периметр { $length } м, площадь { $area } м²
object-edit-reverse = Обратить
object-edit-row-invalid-number = Строка { $row }: позиция или стрела прогиба не является допустимым числом
object-edit-sweep = Развёртка
object-edit-text-not-number = «{ $text }» не является числом
object-edit-vertices = Вершины

## Omf strings

omf-element-name-has-count-tie = Элемент «{ $name }» содержит { $count } соединений со скважинами, которых в нём больше нет
omf-element-name-has-count-unreadable = В элементе «{ $name }» нечитаемых рабочих пачек: { $count }; они исключены
omf-element-unsupported-section = Элемент «{ $name }» указывает раздел «{ $section }», который в этой сборке не может отображать элементы такого типа
omf-element-name-names-unknown-section = Элемент «{ $name }» указывает неизвестный раздел «{ $section }»
omf-ignoring-colour-map-omf-attribute = Цветовая карта атрибута OMF «{ $attribute }» игнорируется: { $error }
omf-mining-data-exported-incline = Горные данные, экспортированные Incline
omf-import = Импорт OMF
omf-texture = Текстура OMF
omf-validation-warnings = Предупреждения проверки OMF: { $warnings }
omf-application-metadata-dropped = Метаданные приложения проекта «{ $application }» не сохраняются
omf-project-author-not-retained = Автор проекта не сохраняется
omf-project-description-not-retained = Описание проекта не сохраняется
omf-unsupported-metadata-keys = В проекте есть неподдерживаемые ключи метаданных: { $keys }
omf-skipped-drillhole-data-saved-older = Пропущены данные скважин, сохранённые в старом формате ({ $names }); импортируйте их заново из исходных файлов

## Plot strings

plot-1-1000-one-millimetre-sheet = При масштабе 1:1000 один миллиметр на листе соответствует одному метру на местности.
plot-1-scale-covers-width-height = 1:{ $scale } · охват { $width } × { $height } м
plot-all-visible-data = Все видимые данные
plot-automatic-grid-interval = Автоматический интервал сетки
plot-border = Граница
plot-centre = Центрировать по
plot-fit-scale-help = Выберите наименьший стандартный масштаб, при котором всё видимое помещается на лист.
plot-coordinate-grid = Координатная сетка
plot-current-view-centre = Центр текущего вида
plot-date-caps = ДАТА
plot-date = Дата
plot-dots-per-inch-paper-size = Точек на дюйм. Этот размер бумаги можно растрировать с разрешением до { $max_dpi } dpi; 300 dpi — обычное качество печати.
plot-dpi = dpi
plot-drawing-no = ЧЕРТЁЖ №
plot-drawing-number = Номер чертежа
plot-drawn-by-caps = ЧЕРТИЛ
plot-drawn-by = Исполнитель
plot-e-g-example-gold-project = например, «Пример золотого проекта»
plot-entered-coordinates = Введённые координаты
plot-export-png = Экспорт PNG...
plot-fit-scale-visible-data = Подогнать масштаб под видимые данные
plot-grid-interval = Интервал сетки
plot-landscape = Альбомная
plot-lists-visible-surfaces-design-layers = Перечисляет видимые поверхности и слои проектирования с их цветами.
plot-margin = Отступ
plot-margins-leave-no-room-map = Поля не оставляют места для карты
plot-metres-scale-1-scale = метры    Масштаб 1:{ $scale }
plot-mm = mm
plot-north-arrow = Северная стрела
plot-nothing-visible-draw = Нет видимых объектов для чертежа
plot-paper = Бумага
plot-paper-orientation-width-height-mm = { $paper }, { $orientation } · { $width } × { $height } мм
plot-paper-size = Размер бумаги
plot-pick-interval-reads-roughly-every = Выберите интервал, который читается примерно каждые 50 мм на печатном листе.
plot-plan = План
plot-scale-must-be-positive = Масштаб чертежа должен быть положительным числом
plot-png-written-sheet-s-exact = PNG записывается с точным физическим размером листа и содержит значение DPI, поэтому печатается в истинном масштабе.
plot-portrait = Книжная
plot-resolution = Разрешение
plot-rev = РЕД.
plot-revision = Ревизия
plot-scale = МАСШТАБ
plot-scale-ratio = Масштаб 1:
plot-scale-framing = Масштабы и рамки
plot-sheet-furniture = Элементы оформления листа
plot-size-width-height-mm = { $size } ({ $width } × { $height } мм)
plot-subtitle = Подзаголовок
plot-title = Название
plot-title-block = Штамп чертежа
plot-today = сегодня
point-cloud-classify = Классифицировать
point-cloud-classify-vegetation = Классифицировать растительность
point-cloud-cloth-resolution = Разрешение ткани
point-cloud-cloth-resolution-about-one-half = Разрешение ткани — около полутора шагов между точками самого разреженного из выбранных облаков, чтобы под каждой частицей были отражения.
point-cloud-combine-selected-point-clouds-into = Объединяет выбранные облака точек в одно новое облако, чтобы по всем ним можно было построить одну триангуляцию. Цвета отдельных точек сохраняются; облако без них вносит свой цвет отображения.
point-cloud-selected-count = Выбрано: { $count } · точек: { $points }
point-cloud-delete-selected-clouds-from-project = Удалить выбранные облака из проекта после завершения объединения, освободив память, которую иначе занимала бы их дублирующая копия.
point-cloud-flat-pads-structures = Плоский (площадки, сооружения)
point-cloud-ground-cloud-covers-steep-follows = Земля, которую покрывает облако. «Крутой» следует по стенам вниз от их бровок; «Плоский» использует более жёсткую ткань, которая перекрывает крупные здания и оборудование, но скругляет резкие перегибы.
point-cloud-ground-threshold = Порог земли
point-cloud-how-far-around-each-point = Радиус вокруг каждой точки для подсчёта соседей.
point-cloud-join = Объединить
point-cloud-let-cloth-follow-walls-down = Позволяет ткани следовать по стенам вниз от бровок, где иначе её жёсткость удерживала бы её на расстоянии от откоса. Отключайте только на пологой местности, заставленной оборудованием.
point-cloud-mark-each-point-ground-noise = Помечает каждую точку как землю, шум или неклассифицированную. Ткань прижимается снизу к облаку и ложится на поверхность земли; точки в пределах порога земли от неё считаются землёй. Существующие классы заменяются; отмена восстанавливает их.
point-cloud-mark-isolated-returns-birds-dust = Помечает изолированные отражения (птицы, пыль, ошибки многолучевости) как шум до поиска земли, чтобы случайная низкая точка не тянула ткань вниз.
point-cloud-mark-noise = Помечать шум
point-cloud-minimum-neighbours = Минимум соседей
point-cloud-name-assigned-joined-point-cloud = Имя, присваиваемое объединённому облаку точек.
point-cloud-name-count-points = { $name } (точек: { $count })
point-cloud-noise-radius = Радиус шума
point-cloud-point-clouds = Облака точек
point-cloud-points-closer-than-settled-cloth = Точки, расположенные ближе этого расстояния к осевшей ткани (по её поверхности), считаются землёй.
point-cloud-points-fewer-neighbours-than-within = Точки, у которых в радиусе шума меньше соседей, чем это значение, считаются шумом.
point-cloud-raise-cloth-resolution-if-your = Увеличьте разрешение ткани, если у компьютера мало оперативной памяти.
point-cloud-recommended = Рекомендуется
point-cloud-recover-steep-slopes = Восстанавливать крутые склоны
point-cloud-relief-dumps-rolling-ground = Рельеф (отвалы, холмистая местность)
point-cloud-remove-sources = Удалить источники
point-cloud-resolution-m-points-spacing-m = { $resolution } м (точки через ~{ $spacing } м)
point-cloud-selected-clouds-copied-into-joined = Выбранные облака, копируемые в объединённое облако. Закройте диалог, чтобы объединить другой набор.
point-cloud-selected-clouds-each-classified-its = Выбранные облака, каждое из которых классифицируется отдельно. Закройте диалог, чтобы классифицировать другой набор.
point-cloud-classify-help = Сортирует отражения обученным классификатором, который анализирует форму точек вокруг каждой: земля, растительность (по высоте: низкая — до 1 м, средняя — до 3 м, высокая) и всё остальное, например здания и оборудование, остаётся неклассифицированным. Отключите, чтобы использовать только ткань.
point-cloud-spacing-cloth-s-particles-around = Шаг частиц ткани. Хорошее начальное значение — около шага точек облака; более мелкий точнее следует за землёй, но требует более плотных точек.
point-cloud-steep-pit-walls-benches = Крутой (борта карьера, уступы)
point-cloud-terrain = Рельеф
point-cloud-use = Использовать

## Products strings

products-add-initiation = Добавить инициирование
products-delay = Задержка
products-delay-palette = Палитра задержек
products-how-long-after-shot-fired = Задержка от запуска взрыва до инициирования заряда в этом устье.
products-initiation-name = Инициирование · { $name }
products-milliseconds-between-one-hole-firing = Миллисекунды между срабатыванием одной скважины и следующей.
products-ms = ms
products-no-products = Нет продуктов
products-remove = Удалить
products-update = Обновить

## Progress strings

progress-percent-done-total = { $percent } ({ $done } из { $total })
progress-task-finished = { $task }: завершено

## Project strings

project-item = Элемент

## Properties strings

properties-adds-view-dependent-rim-highlight = Добавляет зависящую от направления взгляда подсветку краёв на границах блоков и материалов. Отключение немного снижает нагрузку объёмного рендеринга.
properties-block-model-downscale = Понижение разрешения блочной модели
properties-camera = Камера
properties-camera-clip-planes = Плоскости отсечения камеры
properties-cap-while-resizing = Ограничивать при изменении размера
properties-colours-each-point-cloud-chunk = Раскрашивает каждый фрагмент облака точек, обводит рамку, по которой он отсекается по пирамиде видимости, и показывает в строке состояния точки, отрисованные в прошлом кадре, относительно целевого уровня детализации и общего числа видимых.
properties-colours-each-surface-chunk-outlines = Раскрашивает каждый фрагмент поверхности, обводит рамку, по которой он отсекается по пирамиде видимости, и показывает в строке состояния грани, отрисованные в прошлом кадре, относительно общего числа видимых.
properties-dark-mode = Тёмный режим
properties-dataset = Набор данных
properties-developer = Разработчик
properties-downscale-rasters = Понизить разрешение растров
properties-drillholes = Скважины
properties-edit-object = Изменить объект...
properties-field-view = Поле зрения
properties-fps = кадр/с
properties-frame-counter = Счётчик кадров
properties-frame-rate-cap = Ограничение частоты кадров
properties-hz = Hz
properties-interface = Интерфейс
properties-invert-horizontal = Перевернуть по горизонтали
properties-invert-vertical = Перевернуть по вертикали
properties-limits-newly-loaded-geotiff-previews = Ограничивает предпросмотр новых GeoTIFF до 4096 пикселей по длинной стороне. Отключите для полного разрешения в пределах ограничения текстур GPU; это требует больше памяти.
properties-line-colour = Цвет линии
properties-look-sensitivity = Чувствительность обзора
properties-max-clip-span = Максимальная протяженность клипа
properties-move-layer = Переместить в слой...
properties-near-clip-limit = Ближний предел отсечения
properties-no-drillhole-datasets-open = Нет открытых наборов скважин.
properties-orbit-sensitivity = Чувствительность орбиты
properties-panel-chrome = Оформление панели
properties-performance = Производительность
properties-plan-mode = Режим плана
properties-point-cloud-chunk-debug-view = Отладочный вид фрагментов облака точек
properties-presents-step-display-no-tearing = Отображается синхронно с экраном: без разрывов кадра, частоту кадров задаёт экран. При отключении кадры отображаются сразу после отрисовки, и применяется ограничение ниже.
properties-reflective-block-edges = Отражающие края блоков
properties-restore-defaults = Восстановить значения по умолчанию
properties-show-console = Показать консоль
properties-shows-live-near-far-projection = Показывает актуальные ближнее и дальнее расстояния проекции в строке состояния.
properties-snap-polling = Опрос привязки
properties-surface-chunk-debug-view = Отладочный вид фрагментов поверхности
properties-vertical-sync = Вертикальная синхронизация
properties-world-axis-gizmo = Мировая ось
properties-zoom-cursor = Масштаб к курсору
properties-zoom-sensitivity = Чувствительность к увеличению
reference-points-count-holes-from-dataset = Скважин: { $count } из «{ $dataset }»
reference-points-holes-from-datasets = Скважин: { $count } из наборов: { $datasets }
reference-points-holes = Скважины
reference-points-holes-points-placed-selected-when = Скважины, на которых размещаются точки, — выбранные при открытии диалога. Закройте диалог, чтобы выбрать другие.
reference-points-make = Создать
reference-points-no-categorical-field = Нет категориального поля
reference-points-no-values = Нет значений
reference-points-one-point-per-hole-boundary = По одной точке на скважину на этой границе, в новом слое. Если скважина пересекает пачку дважды, берётся самая верхняя точка, и скважина помечается.
reference-points-reference-points = Опорные точки
reference-points-side = Сторона
reference-points-working-section = Рабочая пачка
reference-points-working-section-field = Поле рабочей пачки
reference-surface-controls = Управляющие линии
reference-surface-extent = Границы
reference-surface-points-outside-extent-still-shape = Точки вне границ по-прежнему формируют поверхность; обрезается только сама поверхность.
reference-surface-points-surface-built-from-selected = Точки, по которым строится поверхность, — выбранные при открытии диалога. Закройте диалог, чтобы выбрать другие.
reference-surface-extent-help = Выбранная замкнутая линия, по которой обрезается готовая поверхность; точки вне её по-прежнему формируют поверхность.
reference-surface-selected-open-strings-surface-made = Выбранные разомкнутые линии, через которые проходит поверхность, — выбранные при открытии диалога. Закройте диалог, чтобы выбрать другие.
reference-surface-triangulates-selected-points-plan-in = Триангулирует выбранные точки в плане в новую поверхность. Каждое построение добавляет поверхность.

## Screenshot strings

screenshot-could-not-encode-viewport-image = Не удалось закодировать изображение области просмотра: { $error }
screenshot-could-not-map-viewport-screenshot = Не удалось отобразить снимок области просмотра в память: { $error }
screenshot-could-not-save-viewport-image = Не удалось сохранить изображение области просмотра { $path }: { $error }
screenshot-downloaded-viewport-image-file-name = Изображение области просмотра загружено: { $file_name }
screenshot-saved-viewport-image-path = Изображение области просмотра сохранено: { $path }
screenshot-viewport-image-download-failed-error = Не удалось загрузить изображение области просмотра: { $error }

## Spatial strings

spatial-bvh-face-index-out-of-range = Индекс грани BVH { $index } выходит за пределы сетки; подставляется вырожденный треугольник

## State strings

state-above = на или выше
state-activate-project = Активировать проект
state-all-open-incline-design-data = Все открытые данные Incline Design
state-apply-generated-rings = Применить созданные кольца
state-apply-selection = Применить к выделению
state-rotate-by-azimuth-dip = на азимут { $azimuth }°, угол наклона { $dip }°
state-rotate-to-azimuth-dip = до азимута { $azimuth }°, угла наклона { $dip }°
state-below = на уровне или ниже
state-build-reference-points = Построить опорные точки
state-centre-rotation = Центр вращения
state-checking-unsaved-work = Проверка несохранённой работы
state-choose-destination = Выберите место назначения
state-choose-one-more-files = Выберите один или несколько файлов
state-clear-raster = Очистить растр
state-click-pit-shell-viewport = Щёлкните оболочку карьера в области просмотра.
state-click-pit-stockpile-solid-viewport = Щёлкните тело карьера или склада в области просмотра.
state-click-surface-viewport = Щёлкните поверхность в области просмотра.
state-click-topology-viewport = Щёлкните топологию в области просмотра.
state-close-project = Закрыть проект
state-colour-drillholes = Раскрасить скважины
state-colour-drillholes-working-section = Раскрасить скважины по рабочим пачкам
state-colour-points-classification = Раскрасить точки по классификации
state-copy-objects-layer = Копировать объекты в слой
state-count-cloud-s = Облаков: { $count }
state-count-file-s = Файлов: { $count }
state-count-object-s-axis-value = Объектов: { $count } · { $axis } { $value }
state-count-object-s-closed = Объектов: { $count } · { $closed }
state-count-object-s-layer = Объектов: { $count } · { $layer }
state-count-object-s-weight = Объектов: { $count } · { $weight }
state-count-object-s-z-elevation = Объектов: { $count } · Z { $elevation }
state-points-controls-clipped = Точек: { $count } · управляющих линий: { $controls } · обрезано по линии границы
state-points-controls-unclipped = Точек: { $count } · управляющих линий: { $controls } · без обрезки
state-create-collection = Создать коллекцию
state-create-point-cloud-tin = Создать TIN по облаку точек
state-create-project = Создать проект
state-current-project = Текущий проект
state-cut-topology-pit-shell = Вырезать топологию по оболочке карьера
state-cut-triangulation-polyline = Отсечь триангуляцию полилинией
state-cut-triangulation-z = Отсечь триангуляцию по Z
state-dark-mode = Тёмный режим
state-data-ticked-export-checklist = Данные, отмеченные в списке экспорта
state-detached = Отсоединено
state-disabled = Отключено
state-discard-project-changes = Отбросить изменения в проекте
state-discard-replace-project = Отбросить и заменить проект
state-discarding-unsaved-changes = Отмена несохранённых изменений
state-docked = Закреплено
state-drape-raster = Наложить растр
state-drill-pattern = Сетка скважин
state-duplicate-layer = Дублировать слой
state-east = Восток
state-enabled = Включено
state-exit-incline-design = Выйти из Incline Design
state-export-block-model-csv = Экспорт блочной модели в CSV
state-export-drillhole-csv = Экспорт скважин в CSV
state-export-layer-dxf = Экспорт слоя в DXF
state-export-omf = Экспорт OMF
state-export-project-dxf = Экспорт проекта в DXF
state-export-triangulation = Экспорт триангуляции
state-export-viewport-image = Экспорт изображения области просмотра
state-finish-closed-polyline = Завершить замкнутую полилинию
state-finish-open-polyline = Завершить разомкнутую полилинию
state-fit-extents = Вписать в границы
state-plan-view-then-fit-extents = Вид в плане с того же расстояния, затем вписать в границы
state-fix-release-centre-both-views = Закрепляет или освобождает центр, вокруг которого вращаются оба вида
state-folder-section = { $folder } в разделе { $section }
state-generate-contours = Создать горизонтали
state-hidden = Скрытый
state-import-drillholes = Импорт скважин
state-import-omf = Импорт OMF
state-import-point-cloud = Импорт облака точек
state-import-raster = Импорт растра
state-import-triangulation = Импорт триангуляции
state-insert-intersection-points = Вставить точки пересечения
state-insert-points-elevation = Вставить точки на высоте
state-keep-inside = Оставить внутри
state-keep-outside = Оставить снаружи
state-kriged-block-model = Блочная модель по кригингу
state-load-block-model = Загрузить блочную модель
state-load-drillholes = Загрузить скважины
state-load-layer = Загрузить слой
state-load-point-cloud = Загрузить облако точек
state-load-raster = Загрузить растр
state-load-triangulation = Загрузить триангуляцию
state-locked-count-object-s = Заблокировано объектов: { $count }
state-major-minor = Основной { $major } · вспомогательный { $minor }
state-member-into-folder-section = { $member } в { $folder } в разделе { $section }
state-member-root-section = { $member } в корень раздела { $section }
state-move-axis-value = Переместить на значение оси
state-move-objects-layer = Переместить объекты в слой
state-name-count-cloud-s = { $name } · облаков: { $count }
state-name-count-holes = { $name } · скважин: { $count }
state-name-count-object-s = { $name } · объектов: { $count }
state-name-z-min-z-max = { $name } · от { $z_min } до { $z_max }
state-new-collection-under-section = Новая коллекция в разделе { $section }
state-next-edit = Следующее изменение
state-north = Север
state-off = Выкл.
state-on = Вкл.
state-open-containing-folder = Открыть содержащую папку
state-open-project = Открыть проект
state-preserve-view-angle = Сохранить угол обзора
state-previous-edit = Предыдущее изменение
state-project-id = Проект { $id }
state-remove-block-model = Удалить блочную модель
state-remove-drillholes = Удалить скважины
state-remove-point-cloud = Удалить облако точек
state-remove-raster = Удалить растр
state-remove-triangulation = Удалить триангуляцию
state-removed-from-active-triangulation = Удалено из активной триангуляции
state-removed-from-every-triangulation = Удалено из всех триангуляций
state-rename-kind = Переименовать { $kind }
state-save-close-project = Сохранить и закрыть проект
state-save-despite-unsupported-content = Сохранить несмотря на неподдерживаемое содержимое
state-save-project = Сохранить проект как
state-save-replace-project = Сохранить и заменить проект
state-saving-current-project = Сохранение текущего проекта
state-section-name = Раздел { $section }
state-select-layer-objects = Выберите объекты слоя
state-selected-objects = Выбранные объекты
state-selected-polylines = Выбранные полилинии
state-selected-scene-elements = Выбранные элементы сцены
state-set-block-model-variable = Задать переменную блочной модели
state-set-cinematic-view = Задать кинематографический вид
state-set-drillhole-colour-preset = Задать цветовую схему скважин
state-set-drillhole-discs = Задать диски скважин
state-set-drillhole-style = Задать стиль скважин
state-set-drillhole-width = Задать ширину скважин
state-set-entity-lock = Задать блокировку
state-set-grid = Задать сетку
state-set-layer-lock = Задать блокировку слоя
state-set-line-weight = Задать толщину линии
state-set-object-colour = Задать цвет объекта
state-set-object-fill = Задать заливку объекта
state-set-point-visibility = Задать видимость точки
state-set-polyline-closed = Задать замкнутость полилинии
state-set-raster-lock = Задать блокировку растра
state-set-standard-view = Задать стандартный вид
state-set-topology-wireframes = Задать каркасы топологии
state-set-triangulation-colour = Задать цвет триангуляции
state-show-console = Показать консоль
state-show-project = Показать проект
state-shown = Показано
state-slice-mode = Режим среза
state-slice-preview = Предпросмотр среза
state-south = Юг
state-stem-contours = Горизонтали { $stem }
state-target-new-name = { $target } в «{ $new_name }»
state-trim-above = Обрезать сверху
state-trim-below = Обрезать снизу
state-trim-triangulation-surface = Обрезать триангуляцию по поверхности
state-undrape-raster = Убрать наложение растра
state-undrape-rasters = Убрать наложение растров
state-unload-block-model = Выгрузить блочную модель
state-unload-drillholes = Выгрузить скважины
state-unload-layer = Выгрузить слой
state-unload-point-cloud = Выгрузить облако точек
state-unload-raster = Выгрузить растр
state-unload-triangulation = Выгрузить триангуляцию
state-untitled-project = Проект без названия
state-use-typed-radius = Использовать введённый радиус
state-west = Запад

## Status strings

status-clip-near-far = Ближняя/дальняя плоскость/Δ: -- / -- / --
status-faces-chunks = Грани: -- / -- (фрагментов: --/--)
status-frame-rate = Частота кадров
status-points-chunks = Точки: -- / -- из -- (фрагментов: --/--)

## Text strings

text-could-not-build-vector-mesh = Не удалось построить векторную сетку для шрифта { $font }, глиф { $glyph }: { $error }
text-document-text-mesh-exceeded-its = Сетка текста документа превысила диапазон индексов u32

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = Сначала выберите набор данных скважин для соединения
tie-in-count-connector-s = Соединений: { $count }
tie-in-delete-tie-ins = Удалить соединения
tie-in-deleted-count-selected-tie-connector = Удалено выбранных соединений: { $count }
tie-in-hole = скважина
tie-in-initiation-point-lifted-from-name = Точка инициирования снята с { $name }
tie-in-initiation-point-set-name-delay = Точка инициирования установлена на { $name } с задержкой { $delay } мс
tie-in-select-delay-product-palette-before = Перед соединением скважин выберите средство задержки на палитре
tie-in-tied-connectors = Создано соединений: { $count }, задержка { $delay } мс, средство { $product }
tie-in-tied-connectors-replacing = Создано соединений: { $count }, задержка { $delay } мс, средство { $product }; заменено: { $replaced }

## Toolbar strings

toolbar-fill-type = Тип заливки

## Toolbars strings

toolbars-auto-bench = Автоуступ
toolbars-bezier-polyline = Полилиния Безье
toolbars-chamfer-polyline-corners = Снять фаски с углов полилинии
toolbars-create-text = Создать текст
toolbars-cursor-regular = Курсор: обычный
toolbars-cursor-snap-line = Курсор: привязка к линии
toolbars-cursor-snap-point = Курсор: привязка к точке
toolbars-cursor-snap-surface = Курсор: привязка к поверхности
toolbars-delete-points = Удалить точки
toolbars-explode-polyline-lines = Разбить полилинию на линии
toolbars-fuse-polylines = Объединить полилинии
toolbars-measure-distance = Измерить расстояние
toolbars-new-layer = Новый слой
toolbars-split-polyline-points = Разделить полилинию по точкам
toolbars-strike-dip = Простирание и падение
toolbars-tool-not-available-section-view = { $tool } — недоступно в виде сечения

## Tri strings

tri-sampling-method-help = Адаптивный метод концентрирует вершины на сложном рельефе по ошибке аппроксимации плоскостью; равномерный распределяет их равномерно. В будущем могут появиться другие методы.
tri-adaptive-quadtree = Адаптивный (квадродерево)
tri-axis-range = Диапазон по { $axis }
tri-base-topology-will-receive-pit = Базовая топографическая поверхность, на которую будет помещена форма карьера или отвала.
tri-boundary-polyline = Граничная полилиния
tri-bridge-gaps-help = Перекрывает разрывы и вогнутости границы поверхности, ширина которых меньше этого значения. При 0 всё ещё перекрываются разрывы примерно до размера ячейки выборки; большие значения заполняют более крупные отверстия и сглаживают вогнутости границы.
tri-budget = Бюджет по
tri-cancel-pick = Отменить выбор
tri-candidate-detail = Сведения о варианте
tri-candidate-fine-cells-per-budgeted = Число мелких ячеек-кандидатов на одну бюджетную вершину. Большее значение даёт адаптивной выборке больше свободы при размещении деталей, но замедляет построение.
tri-cap-surface-share-source-points = Ограничьте поверхность долей исходных точек или точным числом вершин.
tri-choose-input-clicking-loaded-surface = Выберите этот вход, щёлкнув по загруженной поверхности в области просмотра
tri-choose-which-side-reference-topology = Выберите сторону опорной топографической поверхности, которую следует удалить из поверхности в их общей области XY.
tri-clip = Отсечение
tri-clip-creates-new-triangulation-name = Обрезка создаёт новую триангуляцию с этим именем; исходная поверхность не изменяется.
tri-clip-surface-polyline = Отсечь поверхность полилинией
tri-closed-pit-stockpile-solid-whose = Замкнутое тело карьера или отвала, открытая граница которого будет включена в результат.
tri-cloud-carries-no-classifications-so = В этом облаке нет классификации, поэтому поверхность строится по всем точкам. Импортируйте файл LAS/LAZ, прошедший фильтрацию земли, чтобы восстановить голую землю.
tri-create-new-layer-contours-append = Создать новый слой для горизонталей или добавить их в существующий слой активного проекта.
tri-cut-topology-pit-shell = Вырезать топологию оболочкой карьера
tri-e-g-design-trimmed = Например, design_trimmed
tri-e-g-mysurf-cut = Например, mysurf_cut
tri-e-g-mysurf-slice = Например, mysurf_slice
tri-e-g-surface-contour = Например, surface_contour
tri-e-g-topo-cut = Например, topo_cut
tri-e-g-topo-pit = Например, topo_with_pit
tri-exact-number-surface-vertices-target = Точное целевое число вершин поверхности. Очень большие значения замедляют построение и требуют много памяти.
tri-existing-ground-topology-will-cut = Существующая поверхность земли, которая будет обрезана оболочкой карьера.
tri-fill-holes-up = Заполнять отверстия до
tri-generate = Создать
tri-generate-contour-lines = Создать линии горизонталей
tri-generate-upper-surface = Создать верхнюю поверхность
tri-ground-points-only = Только точки земли
tri-hide-unload-sources = Скрыть и выгрузить источники
tri-higher-edge-will-enforced-each = В каждом конфликте будет использовано более высокое ребро. Нижние конфликтующие сегменты не будут учитываться как структурные линии, а поверхность будет интерполирована через эти области. Исходные полилинии не изменятся.
tri-breaklines-cross = Выделенные рёбра структурных линий пересекаются или перекрываются на плане при разных отметках. Одна поверхность рельефа не может следовать обоим рёбрам.
tri-intervals-colours = Интервалы и цвета
tri-keep-clipped-topology-included-shape = Сохранить обрезанную топографическую поверхность и включённую форму как отдельные триангуляции, а не объединять их в один объект.
tri-keep-inside-discards-surface-outside = «Оставить внутри» удаляет поверхность за полилинией. «Оставить снаружи» вырезает из поверхности отверстие в форме полилинии.
tri-keeps-only-surface-within-polyline = Оставляет только поверхность внутри границы полилинии.
tri-keep-surface-relation-help = Сохраняет поверхность { $relation } топографической поверхности в пределах её охвата по XY.
tri-layer-already-exists-select-above = Этот слой уже существует; выберите его выше или укажите другое имя.
tri-limit-z-range = Ограничение диапазона Z
tri-major = Основной
tri-max-edge-length = Максимальная длина края
tri-merge = Объединить
tri-method = Метод
tri-min = Мин.
tri-minimum-maximum-elevations-retained = Минимальная и максимальная отметки, сохраняемые в выходной поверхности. Минимум должен быть ниже максимума.
tri-minor = Промежуточный
tri-contour-interval-help = Малый интервал управляет обычными горизонталями. Большой — выделенными и должен быть не меньше малого.
tri-move-cursor-over-loaded-surface = Наведите курсор на загруженную поверхность.
tri-slice-output-name-help = Название, присвоенное выходной поверхности, обрезанной по высоте.
tri-name-assigned-merged-topology-pit = Наименование, присвоенное результату слияния топологии и карьера/склада.
tri-name-assigned-newly-created-contour = Название, присвоенное недавно созданному слою горизонталей.
tri-reconstruct-output-name-help = Название, присвоенное реконструированной триангуляции.
tri-name-assigned-topology-after-pit = Наименование, присвоенное топологии после вырезания из неё оболочки карьера.
tri-name-assigned-trimmed-output-surface = Название, присвоенное обрезанной выходной поверхности.
tri-nearby-breakline-vertices-do-not = Близкие вершины структурных линий не сходятся точно в одной точке, поэтому поверхность невозможно триангулировать.
tri-new-layer = Новый слой
tri-new-layer-name = Название нового слоя
tri-no-boundary-selected = Граница не выбрана
tri-no-point-cloud-selected = Облако точек не выбрано
tri-no-surface-selected = Поверхность не выбрана
tri-once-clip-succeeds-unload-source = После успешной обрезки выгрузить исходную поверхность, чтобы в сцене остался только обрезанный результат.
tri-once-cut-succeeds-unload-original = После успешного вырезания выгрузить исходную топографическую поверхность, чтобы в сцене остался только результат вырезания. Оболочка карьера остаётся загруженной.
tri-once-merge-succeeds-unload-source = После успешного объединения выгрузить исходную топологию и тело, чтобы в сцене остался только объединённый результат.
tri-once-slice-succeeds-unload-source = После успешного среза выгрузить исходную поверхность, чтобы в сцене остался только результат среза.
tri-once-trim-succeeds-unload-surface = После успешной подрезки выгрузить подрезаемую поверхность, чтобы в сцене остался только результат. Топографическая поверхность остаётся загруженной.
tri-only-loaded-pickable = Выбирать можно только загруженные триангуляции.
tri-operation = Операция
tri-output-layer = Выходной слой
tri-percentage = Процент
tri-percentage-cloud = Доля облака
tri-pick-from-view = Выберите из просмотра
tri-pit-design-surface-only-areas = Проектная поверхность карьера. Для выемки используются только области, где она проходит ниже топографической поверхности.
tri-pit-shell = Оболочка карьера
tri-pit-stockpile-solid = Тело карьера/отвала
tri-recommended-weld-retry = Рекомендуется: сварить и повторить
tri-reconstruct-ground-only-help = Восстанавливает по точкам, классифицированным как голая земля, отбрасывая растительность, здания, оборудование и шум. Отключите, чтобы строить поверхность по всем точкам облака.
tri-reconstruct-help = Восстанавливает триангулированную поверхность рельефа из облака точек. Адаптивная выборка расходует бюджет вершин там, где рельеф наиболее сложен, и сохраняет плоские области разреженными.
tri-reduce-budget-candidate-detail-if = Уменьшите бюджет или детализацию кандидатов, если на компьютере мало оперативной памяти.
tri-reference-topology-help = Опорная топографическая поверхность, определяющая область обрезки другой поверхности.
tri-reject-reconstructed-triangle-edges = Отбрасывает рёбра восстановленных треугольников длиннее этого расстояния. Значение 0 отключает ограничение длины ребра.
tri-remove-inside-help = Удаляет поверхность внутри границы полилинии и оставляет остальное.
tri-removes-topology-where-pit-shell = Удаляет топографическую поверхность там, где оболочка карьера проходит ниже неё, чтобы оболочка заполнила отверстие. Шов следует истинной трёхмерной линии контакта поверхностей; топографическая поверхность под частями оболочки, возвышающимися над землёй, сохраняется.
tri-result = Результат
tri-save-two-entities = Сохранить как два объекта
tri-select = Выбрать…
tri-selected-closed-polyline-whose-xy = Выбранная замкнутая полилиния, граница которой в плоскости XY задаёт область обрезки.
tri-selected-point-cloud-whose-points = Выбранное облако точек, по точкам которого будет восстановлена поверхность рельефа. Закройте диалог, чтобы восстановить другое.
tri-selected-surface-from-which-contour = Выбранная поверхность, по которой будут создаваться горизонтали. Закройте диалог, чтобы построить горизонтали по другой.
tri-selected-surface-which-will-clipped = Выбранная поверхность, которая будет обрезана. Закройте диалог, чтобы обрезать другую.
tri-slice-source-help = Выбранная поверхность, диапазон высот которой будет обрезан. Закройте диалог, чтобы рассечь другую.
tri-share-source-points-keep-fractions = Доля исходных точек должна быть сохранена.
tri-slice-triangulation-z-range = Срез триангуляции по диапазону Z
tri-solution-generate-upper-surface = Решение: создать верхнюю поверхность
tri-surface-trim = Обрезаемая поверхность
tri-target-surface-help = Поверхность, которая будет изменена; выбранная топографическая поверхность останется без изменений.
common-percent-suffix = %
tri-topology = Топографическая поверхность
tri-triangulation-failed = Триангуляция не удалась
tri-trim = Обрезать
tri-trim-topology = Обрезать по топологии
tri-uniform-grid = Равномерная сетка
tri-unload-source-surface = Выгрузить исходную поверхность
tri-unload-source-topology = Выгрузить исходную топографическую поверхность
tri-up-target-point-count-points = До { $target } из { $point_count } точек станут вершинами поверхности ({ $percent }%).
tri-use-full-surface-elevation-range = Использовать весь диапазон высот поверхности
tri-vertex-count = Количество вершин
tri-vertices-within-5-cm-xy = Вершины, расположенные в пределах 5 см по XY и Z, получат одну позицию в этой триангуляции. Это может локально сместить создаваемую поверхность на величину до 5 см; исходные полилинии не изменятся.
tri-weld-retry = Сварить и повторить
tri-when-enabled-generate-contours-only = Если включено, горизонтали создаются только между указанными минимальной и максимальной отметками.

## Ui strings

ui-choose-offset-side = Выберите сторону сдвига
ui-choose-relimit-side = Выберите сторону для изменения границы
ui-click-circle-centre = Щёлкните центр окружности
ui-click-closed-polyline-use-blast = Щёлкните замкнутую полилинию, чтобы использовать её как контур блока
ui-click-collar-add-edit-initiation = Щёлкните устье, чтобы добавить или изменить точку инициирования
ui-click-first-point-slice-line = Щёлкните первую точку линии среза
ui-click-first-vertex = Щёлкните первую вершину
ui-click-perimeter-point-type-radius = Щёлкните точку периметра или введите радиус
ui-click-second-point-slice-line = Щёлкните вторую точку линии среза
ui-click-second-vertex = Щёлкните вторую вершину
ui-click-use-pointer-radius = или щёлкните, чтобы использовать радиус указателя
ui-could-not-copy-text-browser = Не удалось скопировать текст в буфер обмена браузера: { $error }
ui-dip-horizontal-no-strike = { $dip } (горизонтально, без простирания)
ui-distance-meters = { $distance } м
ui-drag-ring-type-azimuth-dip = Перетащите кольцо или введите азимут и угол наклона
ui-each-hole-turns-about-its = каждая скважина поворачивается вокруг своего устья
ui-enter-positive-decimal-radius = Введите положительный десятичный радиус
ui-esc-cancels = Esc — отмена
ui-no-delay-product-tie = Нет продукта замедления для соединения
ui-press-enter-use-typed-radius = Нажмите Enter, чтобы использовать введённый радиус
ui-right-click-delay-palette-heading = щёлкните правой кнопкой по заголовку палитры задержек, чтобы добавить
ui-select-designs = Выберите проектные объекты
ui-select-drill-hole = Выберите скважину
ui-select-endpoint-join = Выберите конечную точку для соединения
ui-select-first-crest-toe-point = Выберите первую точку бровки/подошвы уступа
ui-select-item = Выберите пункт
ui-select-line-fuse = Выберите линию для слияния
ui-select-line-polyline = Выберите линию или полилинию
ui-select-line-relimit = Выберите линию для изменения границы
ui-select-next-line-fuse = Выберите следующую линию для слияния
ui-select-opposite-berm-point = Выберите противоположную точку бермы
ui-select-point = Выберите точку
ui-select-polyline = Выберите полилинию
ui-select-polyline-open-line = Выберите полилинию или открытую линию
ui-select-polyline-vertex = Выберите вершину полилинии
ui-select-second-crest-toe-point = Выберите вторую точку бровки/подошвы уступа
ui-select-second-split-point = Выберите вторую точку разделения
ui-select-split-point = Выберите точку разделения
ui-select-topologies = Выберите топологии
ui-slice-view = Режим сечения
ui-strike-dip = простирание { $strike }° · { $dip }
ui-value-dip = падение { $value }°
viewport-1-1-true-shape = 1:1, истинная форма
viewport-1-ratio = 1:{ $ratio }

## Viewport strings

viewport-all-total-categories-keep-their = Все категории ({ $total }) сохраняют свои цвета; отчётливо отображаются только первые { $shown }
viewport-axis-maximum = Максимум { $axis }
viewport-axis-minimum = Минимум { $axis }
viewport-azimuth-dip = Азимут { $azimuth }, угол падения { $dip }
viewport-back-whole-log = Назад к полной диаграмме.
viewport-bar-blast-timeline-placeholder = Хронология взрыва [ЗАГЛУШКА]
viewport-bar-burden-relief-heatmap-placeholder = Тепловая карта высвобождения ЛНС [ЗАГЛУШКА]
viewport-bar-cinematic-view = Кинематографический вид
viewport-bar-color = Цвет:
viewport-bar-contours-equal-time-placeholder = Горизонтали равного времени [ЗАГЛУШКА]
viewport-bar-disable-cinematic-view = Отключить кинематографический вид
viewport-bar-disable-flying-mode = Отключить режим полёта
viewport-bar-disable-x-ray-vision = Отключить рентгеновский режим
viewport-bar-drill-holes = Скважины:
viewport-bar-enable-flying-mode = Включить режим полёта
viewport-bar-enable-x-ray-vision = Включить рентгеновский режим
viewport-bar-exit-slice-view = Выйти из режима сечения
viewport-bar-fill = Заливка:
viewport-bar-fix-centre-rotation = Закрепить центр вращения
viewport-bar-hide-borehole-inspector = Скрыть инспектор скважин
viewport-bar-hide-classification = Скрыть классификацию
viewport-bar-hide-points = Скрыть точки
viewport-bar-hide-rl-grid = Скрыть сетку отметок
viewport-bar-hide-wireframes = Скрыть каркасы
viewport-bar-hide-xy-grid = Скрыть сетку XY
viewport-bar-release-centre-rotation = Освободить центр вращения
viewport-bar-reset-view-plan-over-centre = Сбросить вид: план над центром вращения, щёлкните ещё раз, чтобы вписать всё
viewport-bar-reset-view-plan-same-distance = Сбросить вид: план с того же расстояния, щёлкните ещё раз, чтобы вписать всё
viewport-bar-show-borehole-inspector = Показать инспектор скважин
viewport-bar-show-classification = Показать классификацию
viewport-bar-show-points = Показать точки
viewport-bar-show-rl-grid = Показать сетку отметок
viewport-bar-show-wireframes = Показать каркасы
viewport-bar-show-xy-grid = Показать сетку XY
viewport-bar-vertical-slice-view = Вертикальное сечение
viewport-blank = (пусто)
viewport-choose-active-block-model-variable = Выберите активную переменную блочной модели
viewport-choose-variable = Выберите переменную
viewport-click-edit-color-right-click = Щёлкните, чтобы изменить цвет; щёлкните правой кнопкой, чтобы удалить
viewport-click-type-boundary-s-value = Щёлкните, чтобы ввести значение этой границы
viewport-colour-mapping = Цветовое отображение
viewport-count-categories = Категорий: { $count }
viewport-count-category = Категорий: { $count }
viewport-depth-m-hole-end = { $depth } м, забой скважины
viewport-double-click-add-boundary-here = Дважды щёлкните, чтобы добавить границу здесь
viewport-drag-move-middle-click-toggles = Перетащите для перемещения · Средняя кнопка переключает ≤
viewport-drag-move-right-click-remove = Перетащите для перемещения · Щёлкните правой кнопкой для удаления · Средняя кнопка переключает ≤
viewport-drag-spin-view-around-hole = Перетащите, чтобы вращать вид вокруг скважины. Двойной щелчок — смотреть на север.
viewport-e = В
viewport-edit-category-colour = Изменить цвет этой категории
viewport-edit-colour-used-empty-values = Изменить цвет пустых значений
viewport-empty = (пусто)
viewport-empty-hidden = (пусто · скрыто)
viewport-filter-variables = Фильтр переменных
viewport-fit-hole-track = Вписать скважину в трассу
viewport-from = от { $from } до { $to }
viewport-from-m = от { $from } до { $to } м
viewport-h-1-ratio = Г 1:{ $ratio }
viewport-hole-has-no-trace-draw = У этой скважины нет трассы для отображения.
viewport-interval-data = Данные интервалов
viewport-intervals = Интервалы
viewport-m-from-collar-toward-bearing = м от устья в направлении { $bearing }°
viewport-navigation-hint = Средняя кнопка + перетаскивание — панорама · Колесо — масштаб
viewport-navigation-hint-detach = Средняя кнопка + перетаскивание — панорама · Колесо — масштаб · Щелчок — отсоединить
viewport-n = С
viewport-no-data-variable = Нет данных для этой переменной
viewport-no-density-log-hole = Нет каротажа плотности для этой скважины
viewport-no-downhole-geophysics-hole = Нет скважинной геофизики для этой скважины
viewport-no-gamma-log-hole = Нет гамма-каротажа для этой скважины
viewport-no-matches = Совпадений нет
viewport-no-trace = Нет трассы
viewport-no-usable-range = (нет пригодного диапазона)
viewport-not-logged = Не задокументировано
viewport-orientation-source = Источник ориентации
viewport-rebuild-variable-s-colours-from = Перестроить цвета этой переменной по её данным
viewport-reset = Сбросить
viewport-restore-full-model-range = Восстановить полный диапазон модели
viewport-roll-wheel-over-log-zoom = Прокрутите колесо над диаграммой, чтобы приблизить пласт. Перетащите диаграмму, чтобы вращать скважину и двигаться вдоль неё.
viewport-s = Ю
viewport-sideways-scale = Масштаб по ширине
viewport-squeeze-sideways-just-enough-keep = Сжимает по ширине ровно настолько, чтобы скважина оставалась в поле зрения. Никогда не растягивает.
viewport-trace-extent = Протяжённость трассы
viewport-w = З
viewport-widen-panel-show-density = Расширьте панель, чтобы показать плотность
viewport-widen-panel-show-density-gamma = Расширьте панель, чтобы показать плотность и гамму
viewport-widen-panel-show-gamma = Расширьте панель, чтобы показать гамму
charging-edit-charge-product = Изменить продукт заряжания
charging-new-charge-product = Новый продукт заряжания
charging-explosive-decks-add-mass-primed-stemming = Секции с ВВ добавляют массу и снабжаются боевиками; забойка и воздушные промежутки задаются только длиной.
charging-density = Плотность
charging-density-hint = Плотность в скважине. Масса на метр — это значение, умноженное на площадь сечения скважины.
charging-another-product-already-has-name = Другой продукт уже имеет это имя
charging-edit-charge-rule = Изменить правило заряжания
charging-new-charge-rule = Новое правило заряжания
charging-decks-collar-toe = Секции от устья к подошве
charging-priming = Боевик
charging-preview = Предпросмотр
charging-preview-use-pattern-hole = Использовать медианную скважину сетки
charging-preview-active-pattern-median-hole = Предпросмотр на медианной скважине активной сетки
charging-fixed-decks-longer-than-hole = Фиксированные секции длиннее этой скважины
charging-mass-kg-explosive = ВВ: { $mass } кг
charging-rate-kg-m = { $rate } кг/м
charging-count-primer = Боевиков: { $count }
charging-another-rule-already-has-name = Другое правило уже имеет это имя
charging-save-reload-count-hole = Сохранить и перезагрузить скважины: { $count }
charging-length = Длина
charging-rest-length-m = остаток · { $length } м
charging-rest = остаток
charging-deck-takes-whatever-length-fixed-decks = Эта секция занимает всю длину, оставшуюся после фиксированных секций. В правиле может быть только одна такая секция.
charging-remove-deck = Удалить секцию
charging-add-deck = Добавить секцию
charging-downhole-delay = Внутрискважинное замедление
charging-hole-detonator-hole-fires-long-after = Внутрискважинный детонатор. Скважина срабатывает спустя это время после прихода поверхностного сигнала.
charging-primer-height = Высота боевика
charging-how-far-above-base-each-explosive = Насколько выше подошвы каждой секции с ВВ расположен её боевик.
charging-booster = Промежуточный детонатор
charging-cast-booster-mass-each-primer = Масса литого промежуточного детонатора в каждом боевике.
charging-count-rule-load-product-will-need = Правил, заряжающих этим продуктом: { $count }; для них потребуется выбрать другой.
charging-rule = Правило
charging-holes-already-loaded-keep-their-charge = Скважины, уже заряженные им, сохраняют свой заряд.
blast-burden-relief = Высвобождение ЛНС
blast-ms-per-metre-last-neighbour-fire = мс на метр до последнего сработавшего соседа
blast-below-hole-fires-before-rock-front = Ниже этого значения скважина срабатывает до смещения породы перед ней: туго.
blast-above-rock-front-has-long-gone = Выше этого значения порода перед скважиной давно ушла: слабо, есть риск отсечки и разлёта кусков.
blast-tight = туго
blast-good = хорошо
blast-slack = слабо
blast-free-face = свободная поверхность
blast-fires-at = Срабатывает в
blast-empty-won-t-detonate = пустая, не сработает
blast-not-reached = не достигнута
blast-value-ms-m-from-hole = { $value } мс/м от { $hole }
blast-fires-first-free-face = срабатывает первой: свободная поверхность
blast-relief = Высвобождение
blast-explosive = ВВ
blast-powder-factor = Удельный расход ВВ
blast-not-loaded = Не заряжена
blast-count-primer-delay-ms-downhole = Боевиков: { $count } · замедление в скважине { $delay } мс
blast-set-initiation-point-tie-holes-play = Задайте точку инициирования и соедините скважины, чтобы воспроизвести взрыв
blast-pause = Пауза
blast-play = Воспроизвести
blast-back-start = В начало
blast-duration-ms = из { $duration } мс
blast-real-time = Реальное время
blast-mic-limit = Предел MIC
blast-most-explosive-allowed-detonate-any-8 = Наибольшая масса ВВ, которой разрешено детонировать за любые 8 мс на этом объекте. Окна, превышающие её, помечаются.
blast-no-holes-loaded-surface-signal-plays = Нет заряженных скважин: поверхностный сигнал воспроизводится, но ничего не детонирует. Зарядите скважины инструментом «Зарядить скважины».
blast-now-holes-hole = Сейчас: скважин — { $holes }
blast-in-8-ms = за 8 мс
blast-peak-mass-kg-time-ms = Пик { $mass } кг в { $time } мс
blast-peak-holes-hole-time-ms = Пик: скважин — { $holes } в { $time } мс
blast-peak-over-limit = , превышение на { $over } кг
blast-peak-within-limit = , в пределах нормы
blast-top-surface-signal-lighting-each-downline = Сверху: поверхностный сигнал, запускающий каждую линию. Снизу: детонации. Щёлкните или перетащите, чтобы переместить маркер воспроизведения.
products-charge-rules = Правила заряжания
products-new-rule = Новое правило
products-charge-products = Продукты заряжания
products-new-rule-default-name = Новое правило
products-no-rules = Нет правил
products-load-selected-holes-count = Зарядить выбранные скважины ({ $count })
products-unload-selected-holes-count = Разрядить выбранные скважины ({ $count })
products-edit-rule = Изменить правило
products-duplicate-rule = Дублировать правило
products-delete-rule = Удалить правило
products-fill-product = заполнить  { $product }
products-primer-offset-m-off-each-explosive = Боевик на { $offset } м выше подошвы каждой секции с ВВ, промежуточный детонатор { $booster } кг, замедление в скважине { $delay } мс
products-double-click-edit = Дважды щёлкните, чтобы изменить
products-edit-product = Изменить продукт
blast-log-updated-charge-product-name = Продукт заряжания «{ $name }» обновлён
blast-log-added-charge-product-name = Продукт заряжания «{ $name }» добавлен
blast-log-updated-charge-rule-name = Правило заряжания «{ $name }» обновлено
blast-log-added-charge-rule-name = Правило заряжания «{ $name }» добавлено
blast-log-entry-no-longer-charge-library = Этой записи больше нет в библиотеке заряжания
blast-log-deleted-name-from-charge-library = «{ $name }» удалено из библиотеки заряжания
blast-log-failed-save-charge-library-error = Не удалось сохранить библиотеку заряжания: { $error }
blast-log-cannot-load-rule-problem = Невозможно зарядить по этому правилу: { $problem }
blast-log-count-hole-too-short-fixed-decks = Скважин, слишком коротких для фиксированных секций этого правила, оставлено без изменений: { $count }
blast-log-count-hole-have-no-depth-load = Скважин без глубины для заряжания: { $count }
blast-log-count-loaded-hole-have-no-diameter = У заряженных скважин ({ $count }) нет диаметра, поэтому масса ВВ неизвестна
common-charge-holes = Зарядить скважины
blast-log-loaded-count-hole-rule = Заряжено скважин: { $count } по правилу «{ $rule }»
blast-log-unload-holes = Разрядить скважины
blast-log-unloaded-count-hole = Разряжено скважин: { $count }
blast-log-select-holes-active-pattern-first = Сначала выберите скважины активной сетки
blast-log-rule-no-longer-charge-library = Этого правила больше нет в библиотеке заряжания
blast-log-there-no-charge-rule-load-add = Нет правила заряжания: добавьте его на панели продуктов
blast-rule-stemming = Забойка
blast-rule-air-deck = Воздушный промежуток
blast-rule-give-rule-name = Задайте правилу имя
blast-rule-add-least-one-deck = Добавьте хотя бы одну секцию
blast-rule-only-one-deck-can-fill-rest = Только одна секция может заполнять остаток скважины
blast-rule-deck-lengths-must-greater-than-zero = Длины секций должны быть больше нуля
blast-rule-no-product-named-name = Нет продукта с именем «{ $name }»
blast-rule-rule-needs-least-one-explosive-deck = В правиле должна быть хотя бы одна секция с ВВ
state-save-charge-product = Сохранить продукт заряжания
state-save-charge-rule = Сохранить правило заряжания
state-delete-charge-library-entry = Удалить запись библиотеки заряжания
ui-click-drag-over-holes-load-them = Щёлкните или проведите по скважинам, чтобы зарядить их по правилу «{ $rule }»
ui-hold-shift-unload = удерживайте Shift для разряжания
ui-no-charge-rule-load = Нет правила заряжания
ui-right-click-charge-rules-heading-add = щёлкните правой кнопкой по заголовку «Правила заряжания», чтобы добавить
omf-element-name-has-count-charge-naming = В элементе «{ $name }» зарядов, указывающих на скважины, которых в нём больше нет: { $count }
