# Incline — каталог повідомлень українською мовою.
#
# Може бути неповним: відсутні повідомлення беруться з англійського
# (`i18n/en/incline_design.ftl`). Ідентифікатори зліва від `=` та імена
# аргументів ({ $... }) змінювати не можна — перекладається лише текст справа.

## Загальне

common-cancel = Скасувати
common-clear = Очистити
common-close = Закрити
common-fill = Заливка
common-set = Установити

## Рядок стану

status-language = Мова

## Меню — Файл

menu-file = Файл
menu-file-save-project = Зберегти проект
menu-file-save-project-as = Зберегти проект як...
menu-file-new-project = Новий проект...
menu-file-open-project = Відкрити проект...
menu-file-open-recent = Останні проекти
menu-file-show-in-explorer = Показати в Провіднику
menu-file-show-in-folder = Відкрити папку з файлом
menu-file-import = Імпорт...
menu-file-export = Експорт...
menu-file-export-viewport-image = Експорт зображення області перегляду...
menu-file-export-engineering-drawing = Експорт креслення...
menu-file-about = Про програму { $app }...
menu-file-exit = Вийти з програми

## Меню — Вигляд

menu-view = Вигляд

## Workspaces

ws-production = Виробництво
ws-drill-and-blast = БПР
ws-geology = Геологія
ws-planning = Планування

## Menubars

ws-menubar-design = Проектування
ws-menubar-triangulation = Тріангуляція
ws-menubar-raster = Растр
ws-menubar-point-cloud = Хмари точок
ws-menubar-block-model = Блочна модель
ws-menubar-drillholes = Свердловини
ws-menubar-active-layer = Шар:

## Menubars functions

ws-menubar-design-insert-point = Вставити точку
ws-menubar-design-insert-point-at-intersection = На перетині
ws-menubar-design-insert-point-at-elevation = На висотній позначці
ws-menubar-design-move-to = Перейти до
ws-menubar-design-create-triangulation = Створити тріангуляцію

## Діалоги перейменування й видалення

dialog-rename-title = Перейменувати: { $kind }
dialog-rename-field = Нова назва
dialog-rename-field-hint = Обов'язково
dialog-rename-submit = Перейменувати
dialog-delete-title = Видалити: { $kind }
dialog-delete-confirm =
    Видалити «{ $name }» із проекту?
    Цю дію не можна скасувати.
confirm-delete-product =
    Видалити продукт «{ $name }» з палітри?
    Цю дію не можна скасувати.

## Діалог «Створити тріангуляцію»

tri-create-title = Створити тріангуляцію
tri-create-type-label = Тип тріангуляції
tri-create-type-help =
    «Відкрита поверхня» створює полотно рельєфного типу. «Тіло» створює
    повністю замкнену сітку й потребує вхідних даних, що утворюють
    герметичну межу.
tri-create-output-name = Назва результату
tri-create-output-name-help = Назва, яку буде присвоєно створеній тріангуляції.
tri-create-output-name-hint = назва тріангуляції
tri-create-run = Тріангулювати

tri-selection-selected = Вибрано: { $summary }

tri-type-open-surface = Поверхня
tri-type-solid-closed = Тіло

tri-count-polylines =
    { $count ->
        [one] { $count } полілінія
        [few] { $count } полілінії
       *[other] { $count } полілiній
    }
tri-count-strings =
    { $count ->
        [one] { $count } лінія
        [few] { $count } лінії
       *[other] { $count } ліній
    }
tri-count-points =
    { $count ->
        [one] { $count } точка
        [few] { $count } точки
       *[other] { $count } точок
    }
tri-count-texts =
    { $count ->
        [one] { $count } текстовий об'єкт
        [few] { $count } текстові об'єкти
       *[other] { $count } текстових об'єктів
    }
tri-count-objects =
    { $count ->
        [one] { $count } об'єкт
        [few] { $count } об'єкти
       *[other] { $count } об'єктів
    }

about-read-full-licence = Докладніше про ліцензію ↗
about-source-code = Вихідний код
about-website = Сайт
about-title = Про програму { $app }
drill-hole-colour-stop = Поріг { $index }
properties-restore-defaults-tooltip = Відновити налаштування «{ $heading }» за замовчуванням

## Dynamic UI messages

ui-selected-count = Вибрано: { $count }
ui-selected-objects = Вибрано об'єктів: { $count }
ui-selected-polylines = Вибрано полілiній: { $count }
ui-invalid-axis-value = Введіть допустиме значення осі { $axis }.
ui-selection-spans = Вибір охоплює діапазон від { $min } до { $max }.
confirm-delete-count = Ви впевнені, що хочете видалити вибрані елементи ({ $count })?
confirm-delete-layer = Видалити шар «{ $name }» і всі об'єкти на ньому?
    Цю дію не можна скасувати.
plot-preview-pixels = { $width } × { $height } пікселів при { $dpi } dpi
tri-estimated-memory = Очікуване пікове споживання пам'яті: близько { $estimate }. { $detail }
block-grid-summary = Сітка: { $x } × { $y } × { $z } = { $count } блоків
status-selected = Вибрано: { $count }
status-clip = Ближня/дальня площина/Δ: { $near } / { $far } / { $delta } м

explorer-no-rasters = Немає растрів
slice-viewport-gestures = перетягування середньою кнопкою: панорамування · перетягування правою кнопкою: орбіта · Shift+колесо: рух · W/S: зсув шару · Q/E: обертання · Esc: вихід

## Startup environment details

## Renderer startup diagnostics

color-aci = ACI
color-aci-value = ACI { $index }
color-index = Індекс
color-rgb = RGB
color-opacity = Непрозорість
color-edit = Натисніть, щоб змінити колір
color-saturation-value = Насиченість і яскравість
color-hue = Відтінок
asset-loading = Завантаження даних ресурсу
asset-unloading = Вивантаження даних ресурсу
asset-load-failed = Не вдалося завантажити дані ресурсу
asset-unload-failed = Не вдалося вивантажити дані ресурсу
preferences-title = Параметри
context-text-colour = Колір тексту
context-polylines = Полілінії
context-points = Точки
crs-unknown-ellipsoid = Нерозпізнана модель Землі «{ $name }» у цьому визначенні системи координат.
crs-no-ellipsoid = Це визначення системи координат не вказує, яку модель Землі використовує.
crs-unknown-code = EPSG:{ $code } відсутній у реєстрі систем координат.
crs-transform-failed = Не вдалося перетворити координату; результат не є скінченною позицією.
crs-no-datum-path = Опублікованого перетворення між системами відліку { $from } і { $to } (датуми EPSG { $source } і { $target }) немає. Перетворення попри це було б неправильним на невідому величину, тому нічого не змінено.
crs-unknown-datum = Систему відліку { $from } або { $to } неможливо визначити, і обидві використовують різні моделі Землі. Перетворення між ними було б неправильним на невідому величину.
ws-survey = Геодезія
survey-count-designs = { $count } { $count ->
    [one] проєкт
    [few] проєкти
   *[other] проєктів
  }
survey-count-meshes = { $count } { $count ->
    [one] тріангуляція
    [few] тріангуляції
   *[other] тріангуляцій
  }
survey-count-models = { $count } { $count ->
    [one] блочна модель
    [few] блочні моделі
   *[other] блочних моделей
  }
survey-count-clouds = { $count } { $count ->
    [one] хмара точок
    [few] хмари точок
   *[other] хмар точок
  }
survey-count-holes = { $count } { $count ->
    [one] набір свердловин
    [few] набори свердловин
   *[other] наборів свердловин
  }
survey-count-rasters = { $count } { $count ->
    [one] растр
    [few] растри
   *[other] растрів
  }
survey-angle = Обертання навколо Z (проти годинникової стрілки)
survey-scale = Єдиний коефіцієнт масштабу XYZ
survey-invalid-transform = Початки координат, кут і результуючі координати мають бути скінченними.
survey-invalid-scale = Масштаб має бути скінченним додатним числом зі скінченною оберненою величиною.
survey-empty-selection = Виберіть щонайменше один підтримуваний елемент для перетворення.
survey-unavailable = Вибраний елемент відсутній або не завантажений. Завантажте його перед перетворенням.
survey-wrong-project = Вибирайте проєкти лише з активного проєкту.
survey-name-required = Введіть назву системи координат.
survey-working = Перетворення вибраних даних…
survey-completed = Перетворено на місці: { $items }. Скасування відновить їх.
survey-failed = Помилка перетворення: { $error }
survey-stale = Перетворення скасовано, оскільки активний проєкт або вихідні дані змінилися. Виберіть вихідні дані та спробуйте ще раз.
survey-coordinates-menu = Координати
survey-definitions-action = Визначення…
survey-transform-action = Перетворити…
survey-definitions-title = Визначення координат
survey-transform-title = Перетворення координат
survey-new-system = Нова система координат
survey-new-system-name = Система координат
survey-set-local = Задати як систему координат рудника
survey-delete-system = Видалити систему координат
survey-systems-empty = Немає систем координат
survey-system-name = Назва
survey-system-origin = Та сама точка — координати в системі
survey-angle-help = Проти годинникової стрілки від осі X системи відліку до осі Y, якщо дивитися згори.
survey-scale-help = Єдиний масштаб XYZ від системи відліку до цієї системи. Використовуйте 1, щоб зберегти розміри.
survey-close = Закрити
survey-from = Із
survey-to = До
survey-transform-button = Перетворити
survey-swap = Поміняти місцями
survey-drape-note = Накладені зображення вилучаються з перетворених поверхонь і мають бути накладені знову.
survey-needs-grid-block-model = Блочна модель — це регулярна сітка комірок, і зміна проєкції чи системи відліку не зберігає цю регулярність. Перетворення означало б передискретизацію кожної комірки в нову сітку з втратою значень, які вона містить, тому модель залишено без змін.
survey-needs-grid-raster = Растр розміщується у світі за допомогою афінного відображення, що зміна проєкції чи системи відліку зберегти не може. Перетворення означало б передискретизацію зображення, тому растр залишено без змін.
survey-conversion-exact = Точне: лише зміна сітки, без перепроєктування.
survey-conversion-accuracy = Заявлена точність { $accuracy } м.
survey-kind = Тип
survey-axis-names = Назви осей
survey-kind-registry-short = Система з реєстру
survey-kind-grid-short = Сітка над іншою системою
survey-registry-search = Пошук
survey-registry-hint = Назва або код EPSG, напр. «mga zone 56»
survey-registry-none = У реєстрі немає збігів за всіма словами.
survey-parent = Визначена відносно
survey-parent-origin = Відома точка — координати батьківської системи
survey-pick-registry = Знайдіть систему та виберіть її з результатів.
survey-pick-parent = Виберіть систему, відносно якої визначена ця сітка.
survey-pick-system = Виберіть систему
survey-pick-systems = Виберіть вихідну систему та систему призначення для перетворення.
survey-no-selection = Виберіть систему координат ліворуч або клацніть правою кнопкою, щоб додати нову.
survey-kind-grid = Сітка над { $parent }
survey-system-in-use = «{ $name }» не можна видалити: відносно неї визначено { $dependants } { $dependants ->
    [one] систему
    [few] системи
   *[other] систем
  }. Спершу перенаправте їх на іншу систему.
survey-system-cycle = «{ $name }» визначена відносно самої себе, напряму або через свої батьківські системи.
survey-system-missing = Ця система координат більше не існує. Виберіть інше визначення.
survey-same-system = Виберіть різні вихідну систему та систему призначення.
survey-name-exists = Система координат із такою назвою вже існує. Виберіть її для редагування або вкажіть іншу назву.

## About strings

about-copyright-c-2026-leo-timmins =
    Copyright (c) 2026 Leo Timmins, Lucas Timmins, and Incline Design contributors. Permission is hereby granted, free of charge, to any person obtaining a copy of this software to deal in it without restriction, subject to the conditions of the MIT License.

    Incline Design is provided "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, including but not limited to the warranties of MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE and NONINFRINGEMENT.
about-free-open-source-mine-design = Вільна система проектування гірничих робіт з відкритим кодом
about-licensed-under-mit-license = Розповсюджується за ліцензією MIT

## App strings

app-activated-browser-project-name = Проект браузера «{ $name }» активовано.
app-browser-project-delete-failed = Не вдалося видалити проект браузера: { $error }
app-browser-project-no-longer-exists = Цей проект браузера більше не існує
app-browser-save-failed-error = Помилка збереження в браузері: { $error }
app-could-not-activate-browser-project = Не вдалося активувати проект браузера: { $error }
app-could-not-delete-browser-project = Не вдалося видалити проект браузера: { $error }
app-could-not-load-browser-project = Не вдалося завантажити проект браузера: { $error }
app-could-not-restore-browser-project = Не вдалося відновити проект браузера: { $error }
app-deleted-browser-project = Проект браузера видалено
app-failed-create-window-error = Не вдалося створити вікно: { $error }
app-failed-create-window-icon-error = Не вдалося створити значок вікна: { $error }
app-failed-detach-top-down-preview = Не вдалося від'єднати вигляд згори: { $error }
app-failed-initialize-graphics-error = Не вдалося ініціалізувати графіку: { $error }
app-browser-preferences-load-failed = Не вдалося завантажити налаштування браузера: { $error }
app-failed-load-config-file-error = Не вдалося завантажити файл конфігурації: { $error }
app-failed-load-session-file-error = Не вдалося завантажити файл сеансу: { $error }
app-failed-rasterize-window-icon-error = Не вдалося растеризувати значок вікна: { $error }
app-failed-save-browser-session-error = Не вдалося зберегти сеанс браузера: { $error }
app-failed-save-session-error = Не вдалося зберегти сеанс: { $error }
app-saved-name-browser-storage = «{ $name }» збережено в сховищі браузера

## Block strings

block-model-between = Між
block-model-block-grid = Блочна сітка
block-model-block-size = Розмір блока
block-model-choose-numeric-variable = Виберіть числову змінну
block-model-choose-numeric-variables = Виберіть числові змінні
block-model-count-variables-selected = Вибрано змінних: { $count }
block-model-estimate-variables = Оцінювані змінні
block-model-full-x-y-z-dimensions = Повні розміри X, Y і Z кожного блока. Менші блоки підвищують деталізацію, час обчислення та витрату пам'яті.
block-model-grid-bounds-block-sizes-invalid = Межі сітки або розміри блоків недійсні.
block-model-lower-x-y-z-edges = Нижні межі X, Y і Z об'єму блочної моделі. Центри блоків починаються на половину блока всередину цих меж.
block-model-maximum = Максимум
block-model-maximum-nearest-samples-used-each = Максимальна кількість найближчих проб для кожного блока. Менші значення прискорюють обчислення; більші можуть згладити оцінки та збільшити час обчислень.
block-model-maximum-samples = Максимальна кількість зразків
block-model-minimum = Мінімум
block-model-min-samples-help = Мінімальна кількість найближчих проб для оцінки блока. Блоки з меншою кількістю проб у радіусі пошуку залишаються порожніми.
block-model-minimum-samples = Мінімальна кількість зразків
block-model-nugget = Наґет
block-model-numeric-interval-fields-interpolate = Числові поля інтервалів для інтерполяції. Кожне вибране поле стає змінною блочної моделі.
block-model-kriging-help = Звичайний кригінг оцінює числові інтервали свердловин у центрі кожного блока з використанням сферичної варіограми.
block-model-partial-sill = Частковий поріг
block-model-range-search-radius = Діапазон / радіус пошуку
block-model-range-help = Зразки, розташовані далі за цю відстань, виключаються; коваріація досягає нуля на цьому діапазоні.
block-model-select-all = Виберіть усе
block-model-sill-help = Просторово корельована дисперсія сферичної моделі. Разом з ефектом наґетів задає коваріацію при нульовій відстані.
block-model-spherical-variogram-search = Сферична варіограма та пошук
block-model-threshold-at-most = <= порогу
block-model-threshold-at-least = >= порогу
block-model-threshold-min = Поріг / мін
block-model-upper-x-y-z-extent = Верхні межі X, Y і Z охоплюваного об'єму. Останній блок може вийти за цю межу, якщо протяжність не кратна розміру блока.
block-model-variable = Змінна
block-model-variance-effectively-zero-separation = Дисперсія за практично нульового розділення, спричинена похибкою вимірювання або мінливістю нижче масштабу вибірки. Використовуйте 0, якщо ефект наґетів не потрібен.
block-model-volume-feedback-disconnected = Зворотне читання даних використання об'єму блоків відключилося
block-model-volume-feedback-failed = Не вдалося прочитати дані використання об'єму блоків: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z

## Canvas strings

canvas-not-selectable-closed-polyline = Не можна вибрати | Виберіть замкнену полілінію
canvas-polyline-summary = Полілінія | Шар: { $layer } | Вершин: { $count }
canvas-surface-name = Поверхня | { $name }
canvas-trimmed = Підрізана

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = За об'єктом { $object_id } створено укіс і берму
cmd-bezier-replaced-polyline-span-first-last = Ділянку полілінії { $first }→{ $last } замінено { $count } проміжними точками
cmd-bezier-vertices-first-last = Вершини з { $first } по { $last }
cmd-block-model-block-model-loader-disconnected-path = Завантажувач блочної моделі відключився для { $path }
cmd-block-model-block-model-path-has-count = У блочній моделі { $path } виявлено змінні непідтримуваного типу ({ $count } шт.), які неможливо прочитати: { $names }
cmd-block-model-building-ore-mesh = Побудова сітки руди…
cmd-block-model-could-not-create-block-model = Не вдалося створити блочну модель: { $error }
cmd-block-model-could-not-decode-block-model = Не вдалося декодувати кольорову змінну блочної моделі «{ $variable }»: { $error }
cmd-block-model-created-block-model-name-ordinary = Блочну модель «{ $name }» створено методом звичайного кригінгу
cmd-block-model-failed-load-block-model-error = Не вдалося завантажити блочну модель: { $error }
cmd-block-model-generated-ore-mesh-from-block = Сітку руди створено з блочної моделі «{ $name }»
cmd-block-model-imported-block-model-source-path = Імпортовано джерело блочної моделі { $path }
cmd-block-model-loaded-block-model-name-blocks = Завантажено блочну модель «{ $name }»: блоків — { $blocks } (візуалізованих — { $renderable }), сітка { $dimx }×{ $dimy }×{ $dimz }, змінних — { $variables }
cmd-block-model-loading-name = Завантаження { $name }
cmd-block-model-loading-name-ellipsis = Завантаження { $name }…
cmd-chamfer-applied = Кут { $corner } скошено з радіусом { $radius } і кількістю сегментів { $segments }
cmd-chamfer-radius = Радіус { $radius }
cmd-commands-clipped = Обрізана
cmd-commands-command-failed-error = Помилка команди: { $error }
cmd-commands-select-one-more-objects-before = Перед заданням { $axis } виберіть один або кілька об'єктів
cmd-commands-sliced = Розсічена
cmd-contours-contour-generation-failed-error = Помилка створення горизонталей: { $error }
cmd-contours-discarded-layer-exists = Горизонталі для «{ $name }» відхилено: шар «{ $layer_name }» уже існує
cmd-contours-discarded-project-closed = Горизонталі для «{ $name }» відхилено: проект закрито
cmd-contours-discarded-layer-deleted = Горизонталі для «{ $name }» відхилено: вибраний вихідний шар видалено
cmd-contours-generated = Створено горизонталей для тріангуляції «{ $name }» у шарі «{ $layer_name }»: { $line_count }
cmd-creation-assembled-boundary-rings = Із фрагментованих розімкнених ліній зібрано замкнених граничних контурів: { $assembled_count }
cmd-creation-created-triangulation-from-boundary = Створено тріангуляцію із замкнених контурів ({ $boundary_count }) і розімкнених обмежень ({ $constraint_count }), тип поверхні: { $surface_type }
cmd-creation-creating-triangulation = Створення тріангуляції…
cmd-creation-generate-upper-surface-ignored-count = Створення верхньої поверхні: пропущено конфліктних нижніх сегментів структурних ліній: { $count }; вихідні об'єкти не змінено
cmd-creation-ignored-objects = Під час тріангуляції пропущено об'єктів, які не є полілiніями або є виродженими: { $rejected }
cmd-creation-weld-retry-moved-coarse-welded = Зварювання та повтор: { $coarse_welded } вершин(и) переміщено у спільні позиції (до { $coarse_weld_tol } м); вихідні об'єкти не змінено
cmd-creation-welded-breakline-vertices = Об'єднано вершин структурних ліній, що збіглися в межах допуску: { $welded }
cmd-cuts-clipped-surface-name-polyline-mode = Поверхню «{ $name }» обрізано полілінією ({ $mode })
cmd-cuts-clipping-surface-polyline = Обрізання поверхні полілінією…
cmd-cuts-cut-topology-name-pit-shell = Топографічну поверхню «{ $name }» вирізано за оболонкою кар'єру
cmd-cuts-cut-triangulation-name-z-band = Тріангуляцію «{ $name }» обрізано за діапазоном Z [{ $min }, { $max }]
cmd-cuts-cutting-topology-pit-shell = Вирізання топографічної поверхні за оболонкою кар'єру…
cmd-cuts-cutting-triangulation-z = Обрізання тріангуляції за Z…
cmd-cuts-ignored-vertical-faces = Пропущено вертикальних або вироджених граней опорної топології без площі в XY: { $count }
cmd-cuts-site-skipped-constraint-from-x = { $site }: пропущено обмеження ({ $from_x }, { $from_y }) → ({ $to_x }, { $to_y }), яке тріангулятор не зміг розділити
cmd-cuts-skipped-degenerate-edges = { $site }: пропущено майже вироджених ребер обмежень: { $skipped }; поблизу них межа розрізу може відрізнятися на незначну величину
cmd-cuts-trimmed-surface = Поверхню «{ $surface }» підрізано за топографічною поверхнею «{ $topology }» ({ $mode })
cmd-cuts-trimming-surface-topology = Підрізання поверхні за топографічною поверхнею…
cmd-drape-draped-intersected-vertices-changed = Спроєктовано вершин: { $intersected }; змінено позначку у { $changed }
cmd-drape-no-intersections = Жодна з вибраних проектних вершин не перетинає вибрані топології
cmd-drape-objects-changed-object-s-changed = Змінено об'єктів: { $objects } · переміщено перетинних вершин: { $changed } із { $intersected }
cmd-drape-select-one-more-design-objects = Виберіть один або кілька проектних об'єктів для накладання
cmd-drape-select-one-more-topologies-drape = Виберіть одну або кілька топологій для накладання
cmd-drape-selected-topologies-no-longer-loaded = Вибрані топології більше не завантажені
cmd-drill-hole-drill-pattern-too-large-contains = Сітка свердловин завелика або містить недопустимі координати устя
cmd-drill-hole-enter-name-drill-pattern = Введіть назву сітки свердловин
cmd-drill-hole-failed-load-drillholes-error = Не вдалося завантажити свердловини: { $error }
cmd-drill-hole-depth-must-be-positive = Глибина свердловини має бути більшою за нуль
cmd-drill-hole-diameter-must-be-positive = Діаметр свердловини має бути більшим за нуль
cmd-drill-hole-loaded-drillhole-dataset-name-holes = Завантажено набір свердловин «{ $name }»: свердловин — { $holes }, кольорових полів — { $fields }
cmd-drill-hole-pattern-contains-no-holes = Сітка не містить свердловин
cmd-explode-count-line-s = { $count } ліній
cmd-explode-polyline = Розбити полілінію
cmd-explode-exploded-polyline-into-count-line = Полілінію розбито на { $count } відрізків
cmd-file-block-model-csv-encoding-failed = Помилка кодування CSV блочної моделі: { $error }
cmd-file-block-model-csv-export-failed = Помилка експорту CSV блочної моделі: { $error }
cmd-file-browser-recovery-unavailable = Файли відновлення браузера недоступні; збережені проекти залишаються в IndexedDB
cmd-file-closed-project-runtime-id-runtime = Закрито проект з ідентифікатором середовища виконання { $runtime_id }
cmd-file-could-not-create-new-project = Не вдалося створити новий проект: { $error }
cmd-file-could-not-finish-pending-project = Не вдалося завершити очікувану дію проекту: { $error }
cmd-file-could-not-finish-saving-before = Не вдалося завершити збереження перед виходом: { $error }
cmd-file-could-not-open-browser-project = Не вдалося відкрити проект у браузері: { $error }
cmd-file-could-not-open-path-error = Не вдалося відкрити { $path }: { $error }
cmd-file-could-not-read-selected-file = Не вдалося прочитати вибраний файл: { $error }
cmd-file-could-not-reload-layer-from = Не вдалося повторно завантажити шар з диска: { $error }
cmd-file-could-not-reload-project-from = Не вдалося повторно завантажити проект з диска: { $error }
cmd-file-could-not-remove-browser-project = Не вдалося видалити проект браузера: { $error }
cmd-file-could-not-restore-layer-from = Не вдалося відновити шар із проекту: { $error }
cmd-file-could-not-snapshot-dirty-project = Не вдалося створити знімок зміненого проекту для відновлення: { $error }
cmd-file-could-not-start-browser-export = Не вдалося почати експорт у браузері: { $error }
cmd-file-could-not-write-recovery-copies = Не вдалося записати резервні копії: { $error }
cmd-file-created-new-browser-project = Створено новий проект у браузері
cmd-file-created-new-project = Створено новий проект
cmd-file-description-download-failed-error = Помилка завантаження «{ $description }»: { $error }
cmd-file-discard-cancelled-project-changed = Скасування змін скасовано, оскільки проект змінився під час повторного завантаження OMF
cmd-file-discarded-changes-layer-target-name = Зміни шару «{ $target_name }» скасовано
cmd-file-discarded-changes-reloaded-path = Зміни скасовано: { $path } завантажено повторно
cmd-file-downloaded-description-file-name = Завантажено — { $description }: { $file_name }
cmd-file-dxf-download-encoding-failed-error = Помилка кодування завантажуваного DXF: { $error }
cmd-file-dxf-import-failed-error = Помилка імпорту DXF: { $error }
cmd-file-encoding-block-model-csv-download = Кодування завантаження CSV блочної моделі…
cmd-file-encoding-dxf-download = Кодування завантаження DXF…
cmd-file-encoding-triangulation-download = Кодування завантаження тріангуляції…
cmd-file-exit-deferred-exports = Вихід відкладено до завершення фонового експорту
cmd-file-exit-requested-no-unsaved-changes = Запрошено вихід, незбережених змін немає
cmd-file-exported-block-model-csv-path = CSV блочної моделі експортовано в { $path }
cmd-file-exported-description-dxf-path = { $description } експортовано в DXF: { $path }
cmd-file-exported-triangulation-name-path = Тріангуляцію «{ $name }» експортовано в { $path }
cmd-file-exporting-name = Експорт { $name }…
cmd-file-exporting-triangulation-name-path = Експорт тріангуляції «{ $name }» у { $path }
cmd-file-fatal-renderer-failure-reason = Критичний збій засобу візуалізації: { $reason }
cmd-file-dialog-action-failed = Помилка дії в діалозі файлів: { $msg }
cmd-file-imported-added-object-s-from = Імпортовано об'єктів із { $name }: { $added }
cmd-file-imported-total-dxf-object-s = Імпортовано об'єктів DXF: { $total }
cmd-file-layer-discard-was-cancelled-because = Скасування змін шару скасовано, оскільки проект змінився під час повторного завантаження
cmd-file-no-recovery-directory = Каталог відновлення недоступний: { $error }
cmd-file-no-unsaved-project-content-nothing = Незбереженого вмісту проекту немає; відновлювати нічого
cmd-file-parsing-browser-dxf-import = Аналіз імпорту DXF у браузері…
cmd-file-parsing-dxf-import = Аналіз імпорту DXF…
cmd-file-project-closes-after-save = Проект закриється після завершення поточного збереження
cmd-file-the-project-closes-after-save = Проект закриється після завершення поточного збереження
cmd-file-queued-count-triangulation-file-s = Файлів тріангуляції в черзі на імпорт: { $count }
cmd-file-recovery-copies-path-reopen-them = Копії відновлення розташовані в { $path }; відкрийте їх після перезапуску
cmd-file-recovery-copy-failed-error = Не вдалося створити копію відновлення: { $error }
cmd-file-recovery-copy-failed-failure = Не вдалося створити резервну копію: { $failure }
cmd-file-recovery-copy-written-path = Резервну копію записано: { $path }
cmd-file-reverting-layer = Відкат шару…
cmd-file-reverting-project = Відкат проекту…
cmd-file-save-failed-message = Помилка збереження: { $message }
cmd-file-save-worker-ended-without-result = Процес збереження завершився без результату
cmd-file-saved-project-as = Проект збережено як: { $path }
cmd-file-saved-project = Проект збережено: { $path }
cmd-file-selected-block-model-no-longer = Вибрана блочна модель більше не завантажена
cmd-file-switching-project = Перемикання проекту…
cmd-file-triangulation-download-encoding-failed = Помилка кодування завантажуваної тріангуляції: { $error }
cmd-file-user-chose-exit-without-saving = Користувач вирішив вийти без збереження
cmd-file-user-requested-exit-project-export = Користувач запросив вихід (потрібне підтвердження експорту проекту або незбереженої роботи)
cmd-file-viewport = Область перегляду
cmd-file-wait-current-project-save-finish = Дочекайтеся завершення збереження поточного проекту
cmd-file-wait-current-project-switch-finish = Дочекайтеся завершення перемикання поточного проекту
cmd-file-wait-project-operation-finish-before = Перед скасуванням змін дочекайтеся завершення операції з проектом
cmd-file-wait-project-revert-finish-before = Перед збереженням дочекайтеся завершення відкату проекту
cmd-fuse-closed-polyline = замкнена полілінія
cmd-fuse-count-source-line-s = { $count } вихідних ліній
cmd-fuse-created-shape-object-id-vertices = Створено об'єкт { $shape } { $object_id } з { $vertices } вершинами з { $sources } вихідних ліній
cmd-fuse-click-missed = Злиття: клацання не влучило в жоден об'єкт (під курсором нічого немає)
cmd-fuse-click-not-near-endpoint = Злиття: клацання виконано недостатньо близько до кінців вибраної лінії
cmd-fuse-clicked-closed-polyline = Злиття: вибраний об'єкт { $object_id } є замкненою полілінією; об'єднувати можна лише розімкнені полілінії
cmd-fuse-clicked-not-open-polyline = Злиття: вибраний об'єкт { $object_id } не є розімкненою полілінією (тип: { $kind })
cmd-fuse-clicked-object-missing = Злиття: вибраний об'єкт { $object_id } більше не існує
cmd-fuse-clicked-too-few-vertices = Злиття: вибрана полілінія { $object_id } містить лише { $count } вершин(и); потрібно щонайменше 2
cmd-fuse-endpoint-marker-missing = Злиття: маркер кінцевої точки { $marker_index } більше не існує
cmd-fuse-close-needs-three-vertices = Злиття: для замикання лінії в полілінію потрібно щонайменше 3 різні вершини (наразі { $count })
cmd-fuse-lines = Об'єднати лінії
cmd-fuse-needs-two-segments = Злиття: для застосування потрібно щонайменше 2 сегменти (наявно { $count })
cmd-fuse-no-active-layer = Злиття: немає активного шару для розміщення об'єднаної лінії
cmd-fuse-no-active-project = Злиття: немає активного проекту, застосування неможливе
cmd-fuse-no-source-line = Злиття: немає вихідної лінії для замикання в полілінію
cmd-fuse-awaiting-object-invalid = Злиття: об'єкт { $awaiting_id } більше не є допустимою полілінією
cmd-fuse-object-already-in-chain = Злиття: об'єкт { $object_id } уже входить до ланцюжка; виберіть іншу лінію
cmd-fuse-result-too-few-vertices = Злиття: у результаті замало вершин ({ $count }), операцію скасовано
cmd-fuse-segment-object-invalid = Злиття: об'єкт сегмента { $object_id } більше не є допустимою полілінією; операцію перервано
cmd-fuse-source-object-invalid = Злиття: вихідний об'єкт { $object_id } більше не є допустимою розімкненою полілінією
cmd-fuse-source-object-missing = Злиття: вихідний об'єкт { $object_id } більше не існує
cmd-fuse-open-polyline = розімкнена полілінія
cmd-include-failed = Помилка включення: { $message }
cmd-include-included-solid-shape-name-topology = Тіло «{ $shape_name }» включено в топологію «{ $topology_name }» (збережено граней топології: { $retained }, пропущено замикальних граней: { $skipped })
cmd-include-including-pit-stockpile-solid = Додавання тіла кар'єру/складу…
cmd-insert-point-count-operation-point-s = { $count } точок операції «{ $operation }»
cmd-insert-point-elevation-must-be-finite = Для вставки точки на позначці потрібне скінченне значення позначки
cmd-insert-point-insert-points = Вставити точки
cmd-insert-point-inserted-count-operation-point-s = Вставлено точок операції «{ $operation }»: { $count }
cmd-insert-point-intersection = Перетин
cmd-insert-point-no-new-operation-points-were = Нових точок операції «{ $operation }» не знайдено
cmd-insert-point-select-least-two-polylines-before = Перед вставкою точок перетину виберіть щонайменше дві полілінії
cmd-insert-point-select-one-more-polylines-before = Перед вставкою точки на позначці виберіть одну або кілька полілiній
cmd-layer-created-layer-name = Створено шар «{ $name }»
cmd-layer-deleted-with-objects = Видалено шар { $layer_id } (і всі об'єкти на ньому)
cmd-layer-duplicated-layer-duplicate-name = Створено копію шару «{ $duplicate_name }»
cmd-layer-locked = Заблоковано
cmd-layer-name-copy = копія { $name }
cmd-layer-selected-count-object-s-layer = Вибрано об'єктів у шарі { $layer_id }: { $count }
cmd-layer-state-layer-name = { $state } шар «{ $name }»
cmd-layer-unlocked = Розблоковано
cmd-move-tool-moved-collars = Зміщення ({ $delta }) застосовано до устя свердловин ({ $count })
cmd-move-tool-moved-objects = Зміщення ({ $delta }) застосовано до { $count } об'єктів
cmd-move-tool-count-hole-s = Свердловин: { $count }
cmd-object-edit-edited-kind = Відредаговано { $kind }
cmd-object-edit-edited-kind-count-vertices = Відредаговано { $kind } (вершин: { $count })
cmd-object-edit-no-changes-apply = Немає змін для застосування
cmd-object-edit-object-changed-since-editor-opened = Цей об'єкт змінився відтоді, як відкрито редактор; відкрийте його знову, щоб редагувати поточну версію
cmd-object-edit-target-changed = Об'єкт редагування змінився; зміни скасовано
cmd-object-edit-object-no-longer-exists-document = Цей об'єкт більше не існує в документі
cmd-object-edit-select-single-design-object-edit = Виберіть один об'єкт проєкту для редагування
cmd-object-edit-unassigned = Не призначено
cmd-offset-create-offset = Створити зміщення
cmd-offset-created-offset-count-object-s = Створено зміщення { $count } об'єктів
cmd-offset-distance-must-be-positive = Відстань зміщення має бути більшою за нуль
cmd-omf-could-not-open-project-source = Не вдалося відкрити проект { $source_name }: { $error }
cmd-omf-create-open-project-before-merging = Перед об'єднанням даних створіть або відкрийте проект
cmd-omf-encoding-project = Кодування проекту…
cmd-omf-exported-project-path = Проект експортовано в { $path }
cmd-omf-imported-project = Імпортовано проект «{ $project_name }» з { $source_name }: наборів даних верхнього рівня — { $count }
cmd-omf-importing-project = Імпорт проекту…
cmd-omf-export-failed = Помилка експорту OMF: { $error }
cmd-omf-import-failed = Помилка імпорту OMF: { $error }
cmd-omf-opened-project = Відкрито проект «{ $project_name }» з { $source_name }
cmd-omf-project-source-name-contains-no = Проект «{ $source_name }» не містить підтримуваних елементів даних
cmd-omf-source-name-applied-project-origin = { $source_name }: перед об'єднанням застосовано початок координат проекту { $origin }
cmd-omf-crs-differs = { $source_name }: система координат «{ $source_crs }» відрізняється від системи координат проекту «{ $target_crs }»; координати об'єднано без перепроєктування
cmd-omf-source-name-units-source-units = { $source_name }: одиниці «{ $source_units }» відрізняються від одиниць проекту «{ $target_units }»; координати об'єднано без перетворення
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = Немає відкритих даних Incline Design для експорту
cmd-placement-2-vertices = 2 вершини
cmd-placement-count-vertices = { $count } вершин
cmd-placement-created-circle = Створено коло радіусом { $radius } м
cmd-placement-created-closed-polyline = Створено замкнену полілінію з { $count } вершинами
cmd-placement-created-line-segment-2-vertices = Створено відрізок з 2 вершинами
cmd-placement-created-open-polyline-count-vertices = Створено розімкнену полілінію з { $count } вершинами
cmd-placement-placed-point-x-y-z = Точку розміщено в { $x }, { $y }, { $z }
cmd-placement-radius = Радіус { $radius } м
cmd-plot-composing-engineering-drawing = Формування інженерного креслення…
cmd-plot-could-not-write-engineering-drawing = Не вдалося записати інженерне креслення: { $error }
cmd-plot-drawing-scale-fitted-visible-data = Масштаб креслення підібрано за видимими даними: 1:{ $scale }
cmd-plot = Креслення
cmd-plot-saved-drawing = Інженерне креслення збережено: { $description } ({ $width } × { $height } пкс при { $dpi } т/д)
cmd-point-cloud-failed-load-point-cloud-error = Не вдалося завантажити хмару точок: { $error }
cmd-point-cloud-loaded-point-cloud-name-count = Завантажено хмару точок { $name } ({ $count } точок)
cmd-point-cloud-point-cloud-loader-disconnected-path = Завантажувач хмари точок відключився для { $path }
cmd-point-cloud-tin-max-edge-disabled = (макс. ребро вимкнено)
cmd-point-cloud-tin-max-edge-max-edge = (макс. ребро { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = Помилка створення TIN хмари точок: { $error }
cmd-point-cloud-tin-subsampled = TIN рельєфу: просторова вибірка { $sampled } із { $total } точок
cmd-point-cloud-tin-triangulated = ЦМР: тріангульовано унікальних точок XY: { $vertex_count }; створено граней: { $face_count }{ $suffix }
cmd-products-added-product-delay-ms-ms = Додано засіб { $delay_ms } мс { $name }
cmd-products-deleted-product-delay-ms-ms = Видалено засіб { $delay_ms } мс { $name }
cmd-products-failed-save-products-error = Не вдалося зберегти засоби: { $error }
cmd-products-product-no-longer-palette = Цього засобу більше немає в палітрі
cmd-property-action-count-object-s-layer = { $action } { $count } об'єкт(и/ів) на шар { $layer }
cmd-property-batch-set-axis-value-count = Пакетне встановлення значення { $axis } для { $count } об'єктів
cmd-property-batch-set-closed-count-polyline = Пакетне встановлення замкненості для { $count } полілiній
cmd-property-batch-set-color-count-object = Пакетне встановлення кольору для { $count } об'єктів
cmd-property-batch-set-fill-style-count = Пакетне встановлення стилю заливки для { $count } об'єктів
cmd-property-batch-set-line-weight-count = Пакетне встановлення товщини лінії для { $count } полілiній
cmd-property-copied = Скопійовано
cmd-property-moved = Переміщено
cmd-raster-draped = Растр { $raster } накладено на тріангуляцію { $triangulation } (межі, що перетинаються)
cmd-raster-failed-load-raster-name-error = Не вдалося завантажити растр { $name }: { $error }
cmd-raster-failed-load-raster-path-error = Не вдалося завантажити растр { $path }: { $error }
cmd-raster-loaded-raster-name-via-driver = Завантажено растр { $name } через { $driver } ({ $srcx }×{ $srcy }, попередній перегляд { $prevx }×{ $prevy })
cmd-raster-no-overlapping-triangulation = Жодна завантажена тріангуляція не перекриває межі { $name }
cmd-raster-loader-disconnected = Завантажувач растра відключився для { $path }
cmd-raster-undraped = Растри знято з { $count } тріангуляцій
cmd-relimit-click-missed = Зміна межі: клацання не влучило в жоден об'єкт (під курсором нічого немає)
cmd-relimit-click-ignored = Зміна межі: клацання проігноровано, інструмент зараз не очікує вибору цілі
cmd-relimit-clicked-source-line = Зміна межі: вибрано саму вихідну лінію, виберіть іншу
cmd-relimit-no-source-line = Зміна межі: вихідну лінію не задано, вибір скасовано
cmd-relimit-relimited-line-source-id-selected = Лінію { $source_id } переобмежено за вибраною ціллю
cmd-relimit-resized-line-source-id-using = Розмір лінії { $source_id } змінено в режимі { $mode } зі значенням { $value }
cmd-rename-item-no-longer-belongs-active = Цей елемент більше не належить активному проекту
cmd-rename-renamed-before-name = «{ $before }» перейменовано на «{ $name }»
cmd-rename-renamed-name-taken = «{ $before }» перейменовано на «{ $name }» (назва «{ $requested }» уже зайнята)
cmd-rotate-collar-turned-count-drillhole-collar-s = Повернуто свердловин: { $count } ({ $rotation })
cmd-section-verb-count-item-s-section = { $verb }: елементів у розділі «{ $section }» — { $count }
cmd-selection-delete-vertex = Видалити вершину
cmd-selection-deleted-count-selected-object-s = Видалено вибраних об'єктів: { $count }
cmd-selection-deleted-vertex = Вершину { $vertex } видалено з полілінії { $object_id }
cmd-selection-duplicate-selection = Дублювати вибране
cmd-selection-duplicated-count-object-s = Дубльовано об'єктів: { $count }
cmd-session-created-triangulation = Створено тріангуляцію «{ $name }» (вершин — { $vertex_count }, граней — { $face_count }) з поверхні типу { $surface_type }
cmd-session-deleted-triangulation = Тріангуляцію «{ $name }» видалено з проекту
cmd-session-failed-load-triangulation-error = Не вдалося завантажити тріангуляцію: { $error }
cmd-session-failed-load-triangulation-message = Не вдалося завантажити тріангуляцію: { $message }
cmd-session-loaded-triangulation = Завантажено тріангуляцію «{ $name }» ({ $path }, вершин — { $vertex_count }, граней — { $face_count })
cmd-session-set-triangulation-tri-id-color = Колір тріангуляції { $tri_id } задано як { $color }
cmd-session-triangulation-load-no-result = Завантаження тріангуляції { $path } завершилося без результату
cmd-session-triangulation-failed = Помилка операції з тріангуляцією: { $message }
cmd-session-unloaded-triangulation-name = Тріангуляцію «{ $name }» вивантажено
cmd-slice-entered-slice-view-cx-cy = Увімкнено режим перерізу в точці { $cx }, { $cy }, { $cz } уздовж { $dx }, { $dy } (довжина лінії { $length } м)
cmd-slice-exited-slice-view = Режим перерізу закрито
cmd-slice-reset-section-view-fit-extents = Скинути вигляд перерізу (вписати в межі)
cmd-slice-set-section-grid-enabled = Сітку перерізу увімкнено = { $enabled }
cmd-split-created-2-open-polylines = Створено 2 розімкнені полілінії
cmd-split-line = Розділити лінію
cmd-split-points-needs-interior-vertex = Розділення за точками: виберіть внутрішню вершину розімкненої лінії
cmd-split-points-needs-non-adjacent-vertices = Розділення за точками: виберіть дві несуміжні вершини полілінії
cmd-split-polyline-into-two = Вихідну полілінію розділено на дві розімкнені полілінії
cmd-text-edit-finished = Редагування тексту об'єкта { $object_id } завершено
cmd-text-updated = Текст об'єкта { $object_id } оновлено
cmd-view-centre-rotation-not-available-flying = Центр обертання недоступний у режимі польоту
cmd-view-fixed-centre-rotation-x-y = Центр обертання закріплено в точці { $x }, { $y }, { $z }
cmd-view-no-point-under-cursor-fix = Під курсором немає точки, щоб закріпити на ній центр обертання
cmd-view-released-centre-rotation = Центр обертання звільнено
cmd-view-reset-view-fit-extents = Скинути вигляд (вписати в межі)
cmd-view-set-topology-wireframes-enabled = Каркас топології = { $enabled }
cmd-view-set-view-points-enabled = Відображення точок = { $enabled }
cmd-view-set-xy-grid-enabled = Сітку XY увімкнено = { $enabled }
cmd-view-zoom-extents-preserving-angle = Масштабувати до меж (зі збереженням кута)

## Common strings

common-add-product = Додати засіб
common-background = Тло
common-block-model = Блочна модель
common-block-models = Блочні моделі
common-cancelled = Скасовано
common-chamfer = Фаска
common-choose = Виберіть...
common-circle = Коло
common-click-point-fix-centre-rotation = Клацніть точку, щоб закріпити на ній центр обертання
common-clip-surface-polyline = Відсікти поверхню полілінією...
common-closed = Закрито
common-colour = Колір
common-confirm-omf-rewrite = Підтвердити перезапис OMF
common-could-not-replace-current-project = Не вдалося замінити поточний проект: { $error }
common-count-object-s = Об'єктів: { $count }
common-create = Створити
common-create-batter-berm = Створити уступ і берму
common-create-bezier-curve = Створити криву Безьє
common-create-block-model = Створити блочну модель
common-create-block-model-ellipsis = Створити блочну модель...
common-create-circle = Створити коло
common-create-drill-pattern = Створити сітку свердловин
common-create-layer = Створити шар
common-create-line = Створити лінію
common-create-ore-triangulation = Створити тріангуляцію руди
common-create-ore-triangulation-ellipsis = Створити тріангуляцію руди...
common-create-point = Створити точку
common-create-polyline = Створити полілінію
common-create-triangulation = Створити тріангуляцію...
common-crosses = Хрестики
common-cut = Вирізати
common-cut-topology-pit-shell = Вирізати топологію оболонкою кар'єру...
common-delete-layer = Видалити шар
common-delete-product = Видалити засіб
common-delete-selection = Видалити вибір
common-designs = Проектні об'єкти
common-discard-layer-changes = Відхилити зміни шару
common-down = Вниз
common-drape-topology = Накласти на топологію
common-easting = Схід
common-edit-object = Редагувати об'єкт
common-edit-text = Редагувати текст
common-elevation = Позначка
common-exit-without-saving = Вийти без збереження
common-export-engineering-drawing = Експорт інженерного креслення
common-filter = Фільтр
common-fly-mode = Режим польоту
common-generate-contour-lines = Створити лінії горизонталей...
common-hide-all = Приховати все
common-hide-selection = Приховати вибір
common-ignore = Ігнорувати
common-import-csv-block-model = Імпорт блочної моделі CSV
common-import-dxf = Імпорт DXF
common-incline-design-project = Проект Incline Design
common-layer = Шар
common-legend = Легенда
common-line = Лінія
common-line-weight = Товщина лінії
common-lock-all = Заблокувати все
common-lock-selection = Заблокувати вибір
common-m = м
common-max = Макс
common-merge-shell-into-topology = Злити оболонку з топологією
common-merge-shell-into-topology-ellipsis = Злити оболонку з топологією...
common-move-collar = Перемістити устя
common-move-design = Перемістити об'єкт
common-move-selection = Перемістити вибір
common-new-product = Новий засіб
common-no-block-models = Немає блочних моделей
common-no-design-layers = Немає проектних шарів
common-no-drill-holes = Немає свердловин
common-no-file-chosen = Файл не вибрано
common-no-open-project = Немає відкритого проекту
common-no-point-clouds = Немає хмар точок
common-no-triangulations = Немає тріангуляцій
common-none = Немає
common-northing = Північ
common-offset = Зміщення
common-open = Відкрито
common-orientation = Орієнтація
common-point = Точка
common-point-cloud = Хмара точок
common-point-clouds = Хмари точок
common-polyline = Полілінія
common-polyline-layer = Полілінія на шарі «{ $layer }»
common-project = Проект
common-rasters = Растри
common-redo = Повторити
common-relimit-line = Лінія зміни межі
common-remove-project = Видалити проект
common-reset-view = Скинути вигляд
common-reveal-all = Показати все
common-reveal-finder = Показати у Finder
common-rotate-collar = Повернути свердловину
common-save-exit = Зберегти й вийти
common-scale-bar = Лінійка масштабу
common-set-initiation-point = Задати точку ініціювання
common-shape = Форма
common-shell = З оболонкою
common-slashes = Похилі риски
common-slice = Переріз
common-slice-triangulation-z-range = Переріз тріангуляції за діапазоном Z...
common-surface-contours = Горизонталі поверхні
common-text = Текст
common-degree-suffix = °
common-tie-holes = З'єднати свердловини
common-triangulations = Тріангуляції
common-trim-topology = Обрізати за топологією...
common-undo = Скасувати
common-undrape-all = Прибрати накладання з усіх
common-uniform-white = Однорідний білий
common-unlock-all = Розблокувати все
common-untitled = Без назви
common-up = Вгору
common-vertical-exaggeration = Вертикальне перебільшення
common-x = x
common-zoom-extents = Показати все

## Confirmations strings

confirmations-close-project-unsaved-changes = Закриття проекту: незбережені зміни
confirmations-close-without-saving = Закрити без збереження
confirmations-delete = Видалити
confirmations-delete-objects = Видалити об'єкти
confirmations-discard = Відхилити
confirmations-discard-all-unsaved-changes-layer =
    Скасувати всі незбережені зміни в шарі «{ $name }»?
    Збережений шар буде повторно завантажено з диска, а зміни в інших шарах збережуться. Цю дію не можна скасувати.
confirmations-discard-all-unsaved-changes-name =
    Скасувати всі незбережені зміни в «{ $name }»?
    Останню збережену версію буде повторно завантажено з диска. Цю дію не можна скасувати.
confirmations-discard-changes = Відхилити зміни
confirmations-exit-unsaved-changes = Вихід: незбережені зміни
confirmations-incline-design-cannot-reproduce-all = Incline Design не може повністю відтворити вміст вихідного OMF. Під час збереження буде виключено таке:
confirmations-product = Засіб
confirmations-project = цей проект
confirmations-remove-name-delete-its-browser = Видалити «{ $name }» і збережену в браузері копію? Незбережені зміни буде втрачено.
confirmations-remove-project-unsaved-changes = Видалення проекту: незбережені зміни
confirmations-remove-without-saving = Видалити без збереження
confirmations-replace-project-unsaved-changes = Заміна проекту: незбережені зміни
confirmations-save = Зберегти
confirmations-save-anyway = Однаково зберегти
confirmations-save-changes-current-project-before = Зберегти зміни в поточному проекті перед його заміною?
confirmations-save-changes-name-before-closing = Зберегти зміни в «{ $name }» перед закриттям?
confirmations-save-changes-name-before-removing = Зберегти зміни в «{ $name }» перед видаленням з Incline Design?
confirmations-save-close = Зберегти й закрити
confirmations-save-modified-project-before-exiting = Зберегти змінений проект перед виходом?
confirmations-save-to-browser-before-exit = Зберегти змінений проект у сховищі браузера перед виходом?
confirmations-save-remove = Зберегти й видалити

## Console strings

console-copy-all = Копіювати все
console-copy-message = Копіювати повідомлення
console-error = ПОМИЛКА
console-info = ІНФО
console-no-console-activity-yet = У консолі поки немає подій
console-pending = ОЧІКУВАННЯ
console-progress-summary = Виконується · { $summary }
console-success = УСПІШНО
console-warn = ПОПЕРЕДЖЕННЯ

## Csv strings

csv-block-model-category = Категорія
csv-block-model-value = Значення

## Drill strings

drill-hole-add-stop = Додати поріг
drill-hole-all-rendered-intervals-opaque-white = Усі непрозорі інтервали білі.
drill-hole-burden-spacing-must-greater-than = Відстань між рядами та крок мають бути більшими за нуль
drill-hole-choose-valid-closed-polyline = Виберіть допустиму замкнену полілінію
drill-hole-colour-scale = Колірна шкала
drill-hole-field = Поле
drill-hole-grayscale = Відтінки сірого
drill-hole-green-yellow-red = Зелений–жовтий–червоний
drill-hole-heat = Теплова
drill-hole-no-holes-fit-inside-boundary = За поточної відстані між рядами та кроку всередині цієї межі не поміщається жодної свердловини
drill-hole-pattern-too-many-holes = Сітка перевищує максимум у { $maximum } свердловин; збільшіть відстань між рядами або крок
drill-hole-preset = Пресет
drill-hole-px = пікс.
drill-hole-rainbow = Веселка
drill-hole-reset-preset = Скинути набір
drill-hole-rotation-offsets-must-contain-valid = Поворот і зміщення мають містити допустимі числа
drill-hole-selected-polyline-has-no-usable = Вибрана полілінія не має придатної площі в площині XY
drill-hole-smooth-interpolation = Плавна інтерполяція
drill-hole-spacing-would-scan-too-many = За такого кроку доведеться перевірити занадто багато клітинок сітки; збільшіть відстань між рядами або крок (максимум — { $maximum } свердловин)
drill-hole-square = Прямокутна
drill-hole-staggered = Шахова
drill-hole-stepped-bands = Ступінчасті смуги
common-times-sign = ×
common-minus-sign = −
drill-hole-unsupported-drillhole-source = Непідтримуване джерело свердловин
drill-hole-width = Ширина
drill-pattern-arrangement = Схема розташування
drill-pattern-axis-offset = Зміщення по { $axis }
drill-pattern-blast-shape = Контур блоку
drill-pattern-burden = Відстань між рядами
drill-pattern-choose-closed-blast-boundary-then = Виберіть замкнену межу блоку, потім налаштуйте сітку. Свердловини оновлюються в області перегляду в реальному часі.
drill-pattern-closed-design-polyline-whose-xy = Замкнена проектна полілінія, чия проєкція в площині XY буде заповнена свердловинами.
drill-pattern-rotation-help = Поворот шаблону проти годинникової стрілки від глобальної осі { $axis }.
drill-pattern-distance-between-holes-along-each = Відстань між свердловинами вздовж кожного ряду сітки.
drill-pattern-name-hint = наприклад, Західний блок 03
drill-pattern-diameter-help = Кінцевий діаметр свердловини. Вводиться в міліметрах і зберігається для кожної створеної свердловини.
drill-pattern-hole-depth = Глибина свердловини
drill-pattern-hole-diameter = Діаметр свердловини
drill-pattern-move-over-closed-polyline-then = Наведіть вказівник на замкнену полілінію та клацніть її в області перегляду. Esc скасовує вибір.
drill-pattern-name-help = Назва набору даних свердловин, що створюється в проекті.
drill-pattern-none-picked = Нічого не вибрано
drill-pattern-pattern-name = Назва сітки
drill-pattern-spacing-help = Перпендикулярна відстань між рядами сітки.
drill-pattern-pick = Вибрати
drill-pattern-preview-count-hole-s-diameter = Попередній перегляд: свердловин — { $count } · діаметр — { $diameter } мм · глибина — { $depth } м
drill-pattern-rotation = Обертання
drill-pattern-shift-pattern-grid-along-global = Зсуває сітку шаблону вздовж глобальної осі { $axis }, зберігаючи обрізання за формою вибуху.
drill-pattern-spacing = Крок
drill-pattern-staggered-offsets-every-second-row = У шаховій схемі кожен другий ряд зміщується на половину кроку.
drill-pattern-vertical-depth-below-each-collar = Вертикальна глибина від кожного устя.

## Dxf strings

dxf-block-nesting-too-deep = Вкладеність блоків DXF перевищує максимальну глибину ({ $depth }); «{ $name }» пропущено
dxf-circular-block-reference = Виявлено циклічне посилання на блок DXF: «{ $name }»
dxf-undefined-layer = Об'єкт DXF посилався на невизначений шар «{ $name }»; імпортовано як «{ $fallback }»
dxf-import-budget-exceeded = Імпорт DXF перевищує бюджет { $what } ({ $limit }); решту геометрії пропущено
dxf-insert-unknown-block = DXF INSERT посилається на невідомий блок «{ $name }»

## Edit strings

edit-absolute-length = Абсолютна довжина
edit-absolute-rl = Абсолютна RL
edit-action = Дія
edit-angle = Кут
edit-dip-help = Кут від горизонталі, від'ємний вниз: −90° — вертикальна свердловина.
edit-app-web-not-recommended-production = Веб-версія { $app } не рекомендована для промислового використання. Використовуйте її лише для демонстрації.
edit-application = Застосування
edit-apply = Застосувати
edit-apply-pick-target = Застосувати й вибрати ціль
edit-axis-value = Значення { $axis }
edit-azimuth = Азимут
edit-batter-angle = Кут укосу уступу (°)
edit-azimuth-help = Напрямок буріння свердловин у градусах за годинниковою стрілкою від півночі координатної сітки.
edit-bench-height = Висота уступу
edit-benches = Уступи
edit-berm-width = Ширина берми
edit-bezier-curve = Крива Безьє
edit-choose-layer = Виберіть шар
edit-measure-help = Виберіть, що означає введене значення: відстань уздовж укосу, горизонтальну ширину чи вертикальну висоту.
edit-choose-which-two-polyline-paths = Виберіть один із двох шляхів полілінії між вибраними вершинами для заміни. Довжина враховує висоту та криволінійні ребра.
edit-click-corner-closed-polyline = Клацніть по куту на замкненій полілінії.
edit-click-open-closed-polyline-begin = Клацніть по відкритій або замкненій полілінії, щоб почати.
edit-click-second-vertex-replacement-span = Клацніть другу вершину ділянки заміни.
edit-click-vertex-start-replacement-span = Клацніть вершину, щоб почати ділянку заміни.
edit-collide-triangulation = Зіткнення з тріангуляцією
edit-confirm-selection = Підтвердити вибір
edit-control-point-1 = Контрольна точка 1
edit-control-point-2 = Контрольна точка 2
edit-copy = Копіювати
edit-corner-radius-limited-so-replacement = Радіус кута, обмежений так, щоб заміна не могла пройти через сусідні вершини.
edit-create-new-layer = Створити новий шар
edit-create-new-project = Створити новий проект
edit-create-project = Створення проекту
edit-delta-length-m-use = Зміна довжини (м, використовуйте + або -)
edit-dip = Кут падіння
edit-direction = Напрямок
edit-distance = Відстань
edit-distance-along-slope = Відстань уздовж укосу
edit-download-free-native-version-our = Завантажте безкоштовну нативну версію на нашому сайті ↗
edit-drill-hole = Свердловина
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = Кінець
edit-enter-valid-elevation = Введіть дійсну висоту.
edit-exit-slice = Вийти з режиму перерізу
edit-finish-polyline = Завершити полілінію
edit-generate-batter-berms = Створити уступи й берми
edit-height = Висота
edit-height-change = Перепад висоти
edit-height-mode = Режим висоти
edit-horizontal-distance = Горизонтальна відстань
edit-horizontal-width-each-flat-berm = Горизонтальна ширина кожної плоскої берми між послідовними укосами уступів.
edit-hover-choose-which-end-move = Наведіть курсор, щоб вибрати, який кінець перемістити, потім клацніть для підтвердження.
edit-insert-point-elevation = Вставити точку на висоті
edit-intersect = Перетин
edit-kind-properties = { $properties } об'єкта «{ $kind }»
edit-layer-name = Назва шару
edit-load-project = Завантажити проект
edit-longest = Найдовший
edit-m-s = м/с
edit-measure = Вимірювання
edit-mit-license = Ліцензія MIT
edit-mode = Режим
edit-move = Перемістити
edit-move-layer = Перемістити в шар
edit-move-which-end = Який кінець перемістити
edit-movement-speed-slice-when-using = Швидкість переміщення перерізу при використанні клавіш навігації.
edit-moving-end-endpoint = Переміщення: кінцева точка
edit-moving-start-endpoint = Переміщення: початкова точка
edit-new-length-m = Нова довжина (м)
edit-new-project = Новий проект
edit-number-complete-batter-berm-levels = Кількість повних рівнів укосу та берми. Максимум обмежений найглибшим рівнем, на якому зберігається задана геометрія.
edit-bezier-segments-help = Кількість відрізків для апроксимації кривої між двома вибраними вершинами.
edit-chamfer-segments-help = Кількість прямих сегментів для апроксимації заокругленого кута. Значення 1 створює пряму фаску.
edit-object = Об'єкт
edit-offset-element = Елемент зміщення
edit-pick-side = Виберіть бік
edit-pit = Кар'єр
edit-project-name = Назва проекту
edit-properties = Властивості
edit-radius = Радіус
edit-recent = Останні
edit-relative = Відносно (+/-)
edit-elevation-mode-help = «Відносна» застосовує зміну висоти до всіх точок. «Абсолютна RL» проєктує всі точки на одну задану позначку.
edit-remove-from-list = Видалити зі списку
edit-replace-path = Замінити шлях
edit-rotate = Повернути
edit-rotation-speed-slice-when-using = Швидкість обертання перерізу при використанні клавіш Q і E.
edit-s = °/с
edit-segments = Сегменти
edit-segments-lying-elevation-ignored = Сегменти, що лежать на цій висоті, ігноруються.
edit-endpoint-help = Виберіть кінцеву точку, що змінюється; інша залишиться нерухомою.
edit-selected-holes-point-different-ways = Вибрані свердловини спрямовані по-різному. Застосування встановить для всіх ці кути.
edit-selected-start-end-point-moves = Вибрана початкова або кінцева точка переміщується вздовж напрямку лінії; протилежна точка залишається нерухомою.
edit-set-axis = Задати { $axis }
edit-shortest = Найкоротший
edit-slice-view = Вигляд перерізу
edit-slope-angle-each-batter-face = Кут нахилу кожного укосу уступу, виміряний від горизонталі.
edit-slope-angle-offset-positive-negative = Кут нахилу зміщення. Додатні та від'ємні кути переміщують копію вище або нижче вихідного об'єкта під час бокового зміщення.
edit-speed = Швидкість
edit-start = Початок
edit-stockpile = Склад
edit-stop-generated-offset-where-its = Зупинити утворюване зміщення там, де його шлях уперше перетне видиму тріангуляцію.
edit-target-rl = Цільова позначка
edit-text-colour-opacity = Колір тексту та непрозорість.
edit-thickness-visible-slice-slab-centred = Товщина видимого шару перерізу, центрованого за індикатором огляду.
edit-translation-axis-help = Відстань зсуву вздовж світової осі { $axis }.
edit-type = Тип
edit-type-direction-together-set-offset = Тип і напрямок разом задають бік зміщення. Кар'єр + вгору та відвал + вниз зміщуються назовні; кар'єр + вниз та відвал + вгору — всередину.
edit-bench-direction-help = «Вгору» піднімає кожен уступ на висоту уступу, «Вниз» опускає його. При цьому також змінюється бік зміщення — див. «Тип».
edit-value-help = Значення інтерпретується з використанням вибраних режимів вимірювання та висоти.
edit-vertical-rise-fall-each-bench = Вертикальний підйом або спуск кожного уступу до створення наступної берми.
edit-bezier-control-point-1-help = Світові координати X, Y і Z першої контрольної точки Безьє.
edit-bezier-control-point-2-help = Світові координати X, Y і Z другої контрольної точки Безьє.

## Events strings

events-couldn-t-exit-error = Не вдалося вийти: { $error }
events-couldn-t-save-error = Не вдалося зберегти: { $error }
events-set-elevation = Задати позначку
events-set-elevation-from-cursor-hit = Позначку за точкою курсора задано як Z { $z }
events-tool-not-available-section-view = Цей інструмент недоступний у вигляді перерізу

## Explorer strings

explorer-clear-active-triangulation-texture = Очистити текстуру активної тріангуляції
explorer-delete-from-project = Видалити з проекту
explorer-discard-changes = Відхилити зміни...
explorer-download = Завантажити
explorer-drape-over-surface = Накласти на поверхню
explorer-draped-over-surface = Накладено на поверхню
explorer-duplicate = Дублювати
explorer-face-colour = Колір грані
explorer-id-block-model-id-source =
    ID: block-model:{ $id }{ $source }
    Кольорових змінних: { $count }
explorer-id-drill-holes-id-source =
    ID: drill-holes:{ $id }{ $source }
    Свердловин: { $holes }
    Кольорових полів: { $fields }
explorer-id-point-cloud-id-source =
    ID: point-cloud:{ $id }{ $source }
    Точок: { $count }
explorer-raster-id =
    ID: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = ID: triangulation:{ $id }{ $source }
explorer-load = Завантажити
explorer-lock = Заблокувати
explorer-select-all-objects = Виберіть усі об'єкти
explorer-source-name = Джерело: { $name }
explorer-unload = Вивантажити
explorer-unlock = Розблокувати

## Files strings

files-automatic-colour = Автоматичний колір
files-automatic-rl-spacing = Автоматичний крок позначок
files-axis-scale-ratio = Коефіцієнт масштабу по { $axis }
files-ok = Гаразд
files-reset-scale = Скинути на 1×
files-rl-grid-options = Параметри сітки позначок
files-rl-spacing = Крок позначок
files-scales-z-distances-visually-without = Масштабує відстані Z візуально, не змінюючи збережені координати.
files-thickness = Товщина
files-xy-grid-options = Параметри сітки XY

## Gpu strings

gpu-cache-block-model-surface-build-failed = Не вдалося побудувати поверхню блочної моделі: { $error }
gpu-cache-block-model-surface-build-worker = Потік побудови поверхні блочної моделі відключився
gpu-cache-block-model-surface-chunk-rejected = Фрагмент поверхні блочної моделі відхилено до виділення пам'яті GPU: екземпляри={ $instances } байт, межа={ $limit } байт
gpu-cache-block-volume-worker-disconnected = Потік підготовки об'єму блоків відключився
gpu-cache-translucent-volume-could-not-built = Не вдалося побудувати напівпрозорий об'єм ({ $error }); цю блочну модель показано кубами.
gpu-cache-edge-chunk-rejected = Фрагмент ребер тріангуляції відхилено до виділення пам'яті GPU: екземпляри={ $instances } байт, межа={ $limit } байт
gpu-cache-triangulation-chunk-rejected = Фрагмент тріангуляції GPU відхилено до виділення пам'яті: вершини={ $vertices } байт, індекси={ $indices } байт, межа={ $limit } байт
gpu-cache-triangulation-too-many-vertices = У тріангуляції «{ $name }» { $count } вершин (> u32::MAX); її не можна розбити на фрагменти для GPU
gpu-cache-triangulation-uploaded = Тріангуляцію «{ $name }» завантажено в { $chunks } просторових фрагментах (граней: { $faces })
i18n-active-language = Активна мова: { $language } (вбудовані: { $bundled })
i18n-could-not-select-language-error = Не вдалося вибрати мову: { $error }

## Init strings

init-gpu-adapter-vendor-name-backend = Відеоадаптер: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver = Драйвер відеоадаптера: { $driver } { $driver_info }
init-gpu-supports-maximum-buffer-size = Максимальний розмір буфера відеоадаптера — { $size } МіБ; великі сцени можуть відображатися не повністю
init-surface-present-mode = Режим представлення поверхні: { $mode }
init-wgpu-error-continuing-error = Помилка wgpu (роботу продовжено): { $error }

## Io strings

io-ascii-points-xyz-pts = Точки ASCII (.xyz, .pts)
io-attribute = Атрибут
io-blank-header = (порожні заголовки)
io-block-model = Блочна модель:
io-choose-file-purpose-map-its = Виберіть призначення файлу, щоб зіставити його стовпці.
io-choose-loaded-block-model = Виберіть завантажену блочну модель
io-choose-loaded-layer = Виберіть завантажений шар
io-choose-loaded-triangulation = Виберіть завантажену тріангуляцію
io-choose-purpose = Виберіть призначення…
io-choose-source-file-files-import = Виберіть вихідний файл або файли для імпорту.
io-collar = Устя
io-column-mapping = Зіставлення стовпців
io-comma-separated-values-csv = Значення, розділені комами (.csv)
io-csv-files = Файли CSV
io-default = За замовчуванням
io-depth = Глибина
io-diameter = Діаметр
io-drawing-exchange-format-dxf = Drawing Exchange Format (.dxf)
io-drill-holes = Свердловини
io-east-x = Схід / X
io-elevation-z = Позначка / Z
io-end-x = Кінцева X
io-end-y = Кінцева Y
io-end-z = Кінцева Z
io-explicit-segments = Явні сегменти
io-export = Експорт
io-export-csv-block-model = Експорт блочної моделі CSV
io-export-dxf = Експорт DXF
io-export-one-layer = Експорт одного шару
io-export-ply = Експорт PLY
io-export-stl = Експорт STL
io-export-wavefront-obj = Експорт Wavefront OBJ
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = Імпорт
io-import-ascii-point-cloud = Імпорт хмари точок ASCII
io-import-drillhole-csv-bundle = Імпорт пакета CSV свердловин
io-import-geotiff = Імпорт GeoTIFF
io-import-las-laz-point-cloud = Імпорт хмари точок LAS/LAZ
io-import-pcd-point-cloud = Імпорт хмари точок PCD
io-import-ply = Імпорт PLY
io-import-stl = Імпорт STL
io-import-wavefront-obj = Імпорт Wavefront OBJ
io-interval = Інтервал
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-mapped-csv-bundle-csv = Зіставлений пакет CSV (.csv)
io-model-file = Файл моделі
io-name-count-files = { $name } + файлів: { $count }
io-no-csv-chosen = Файл .csv не вибрано
io-no-csv-files-chosen = Файли CSV не вибрано
io-no-dxf-chosen = Файл .dxf не вибрано
io-no-omf-chosen = Файл .omf не вибрано
io-north-y = Північ / Y
io-ply = PLY (.ply)
io-point-cloud-data-pcd = Point Cloud Data (.pcd)
io-source-file = Вихідний файл
io-start-x = Початкова X
io-start-y = Початкова Y
io-start-z = Початкова Z
io-stl = STL (.stl)
io-triangulation = Тріангуляція:
io-unmapped = Не зіставлено
io-wavefront-obj = Wavefront OBJ (.obj)

## Jobs strings

jobs-background-task-poll-label-ended = Фонове завдання «{ $poll_label }» завершилося без результату
jobs-discarded-stale-result = Застарілий результат фонового завдання «{ $poll_label }» відхилено, оскільки вихідний об'єкт змінився або було закрито

## Logging strings

logging-activity-completed = Операцію завершено
logging-activity-started = Операцію розпочато
logging-application-id-id = Ідентифікатор програми: { $id }
logging-application-name = Назва програми: { $name }
logging-application-startup = Запуск програми
logging-build-target-os-architecture = Цільова платформа збірки: { $os }-{ $architecture }
logging-completed = Завершено
logging-count-messages = Повідомлень: { $count }
logging-desktop-session-xdg-session-type = Сеанс робочого столу: XDG_SESSION_TYPE={ $type }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
logging-initialising-incline-design = Ініціалізація Incline Design
logging-locale-environment = Мовне середовище: LANG={ $lang }, LC_ALL={ $locale }, TZ={ $timezone }
logging-macos-session = Сеанс macOS: USER={ $user }, SHELL={ $shell }
logging-operating-system-gnu-linux = Операційна система: GNU / Linux
logging-operating-system-macos = Операційна система: macOS
logging-operating-system-microsoft-windows = Операційна система: Microsoft Windows
logging-pointer-width = Розрядність вказівника: { $width }-біт
logging-process-id-id = Ідентифікатор процесу: { $id }
logging-release-version = Версія випуску: { $version }
logging-renderer = Засіб візуалізації
logging-rust-compiler-host = Хост компілятора Rust: { $host }
logging-system = Система
logging-system-error = Системна помилка
logging-unknown = невідомо
logging-windows-session-sessionname-session = Сеанс Windows: SESSIONNAME={ $session }, USERNAME={ $user }
logging-working = Виконується…

## Mac strings

mac-cannot-install-macos-menu-bar = Не можна встановити рядок меню macOS поза головним потоком
mac-quit-app = Вийти з { $app }

## Main strings

main-incline-design-web-startup-failed = Не вдалося запустити Incline Design Web: { $error }

## Menu strings

menu-count-files-selected = Вибрано файлів: { $count }

## Object strings

object-edit-appearance = Зовнішній вигляд
object-edit-arc-circle = Дуга та коло
object-edit-arc-segments = Сегменти дуги
object-edit-bulge = Стріла прогину
object-edit-bulge-arcs-horizontal-data-model = За моделлю даних дуги зі стрілою прогину горизонтальні: дуга вигинається у плані, а висота змінюється по прямій від однієї вершини до наступної.
object-edit-centre-x = Центр X
object-edit-centre-y = Центр Y
object-edit-centre-z = Центр Z
object-edit-chord = Хорда
object-edit-colour-layer = Колір за шаром
object-edit-enter-number = Введіть число
object-edit-follow-owning-layer-s-colour = Використовувати колір шару-власника замість кольору, закріпленого за цим об'єктом.
object-edit-id = ID
object-edit-identity = Тотожне
object-edit-insert-after = Вставити після
object-edit-join-last-vertex-back-first = З'єднує останню вершину знову з першою.
object-edit-length = Довжина { $length } м
object-edit-move-down = Перемістити вниз
object-edit-move-up = Перемістити вгору
object-edit-object-has-no-arc-segments = Цей об'єкт не має сегментів дуги.
object-edit-object-has-single-position = Цей об'єкт має одну позицію.
object-edit-object-needs-least-required-vertices = Цьому об'єкту потрібно щонайменше { $required } вершин
object-edit-one-more-properties-not-valid = Одна або кілька властивостей не є коректним числом
object-edit-perimeter-area = Периметр { $length } м, площа { $area } м²
object-edit-reverse = Обернути
object-edit-row-invalid-number = Рядок { $row }: позиція або стріла прогину не є коректним числом
object-edit-sweep = Розгортка
object-edit-text-not-number = «{ $text }» не є числом
object-edit-vertices = Вершини

## Omf strings

omf-element-name-has-count-tie = Елемент «{ $name }» містить { $count } з'єднань зі свердловинами, яких у ньому вже немає
omf-ignoring-colour-map-omf-attribute = Кольорову карту атрибута OMF «{ $attribute }» проігноровано: { $error }
omf-mining-data-exported-incline = Гірничі дані, експортовані Incline
omf-import = Імпорт OMF
omf-texture = Текстура OMF
omf-validation-warnings = Попередження перевірки OMF: { $warnings }
omf-application-metadata-dropped = Метадані програми проекту «{ $application }» не зберігаються
omf-project-author-not-retained = Автор проекту не зберігається
omf-project-description-not-retained = Опис проекту не зберігається
omf-unsupported-metadata-keys = У проекті є непідтримувані ключі метаданих: { $keys }

## Plot strings

plot-1-1000-one-millimetre-sheet = У масштабі 1:1000 один міліметр на аркуші відповідає одному метру на місцевості.
plot-1-scale-covers-width-height = 1:{ $scale } · охоплення { $width } × { $height } м
plot-all-visible-data = Усі видимі дані
plot-automatic-grid-interval = Автоматичний інтервал сітки
plot-border = Межа
plot-centre = Центрувати по
plot-fit-scale-help = Виберіть найменший стандартний масштаб, за якого все видиме вміщується на аркуш.
plot-coordinate-grid = Координатна сітка
plot-current-view-centre = Центр поточного вигляду
plot-date-caps = ДАТА
plot-date = Дата
plot-dots-per-inch-paper-size = Точок на дюйм. Цей розмір паперу можна растеризувати з роздільною здатністю до { $max_dpi } dpi; 300 dpi — звичайна якість друку.
plot-dpi = dpi
plot-drawing-no = КРЕСЛЕННЯ №
plot-drawing-number = Номер креслення
plot-drawn-by-caps = ВИКОНАВ
plot-drawn-by = Виконавець
plot-e-g-example-gold-project = наприклад, «Приклад золотого проекту»
plot-entered-coordinates = Введені координати
plot-export-png = Експорт PNG...
plot-fit-scale-visible-data = Підібрати масштаб під видимі дані
plot-grid-interval = Інтервал сітки
plot-landscape = Альбомна
plot-lists-visible-surfaces-design-layers = Перелічує видимі поверхні та шари проектування з їхніми кольорами.
plot-margin = Відступ
plot-margins-leave-no-room-map = Поля не залишають місця для карти
plot-metres-scale-1-scale = метри    Масштаб 1:{ $scale }
plot-mm = мм
plot-north-arrow = Стрілка на північ
plot-nothing-visible-draw = Немає видимих об'єктів для креслення
plot-paper = Папір
plot-paper-orientation-width-height-mm = { $paper }, { $orientation } · { $width } × { $height } мм
plot-paper-size = Розмір паперу
plot-pick-interval-reads-roughly-every = Виберіть інтервал, який читається приблизно кожні 50 мм на друкованому аркуші.
plot-plan = План
plot-scale-must-be-positive = Масштаб креслення має бути додатним числом
plot-png-written-sheet-s-exact = PNG записується з точним фізичним розміром аркуша та містить значення DPI, тому друкується в істинному масштабі.
plot-portrait = Книжкова
plot-resolution = Роздільна здатність
plot-rev = РЕД.
plot-revision = Ревізія
plot-scale = МАСШТАБ
plot-scale-ratio = Масштаб 1:
plot-scale-framing = Масштаб і кадрування
plot-sheet-furniture = Елементи оформлення аркуша
plot-size-width-height-mm = { $size } ({ $width } × { $height } мм)
plot-subtitle = Підзаголовок
plot-title = Назва
plot-title-block = Штамп креслення
plot-today = сьогодні

## Products strings

products-add-initiation = Додати ініціювання
products-delay = Затримка
products-delay-palette = Палітра затримок
products-how-long-after-shot-fired = Затримка від запуску вибуху до ініціювання заряду в цьому усті.
products-initiation-name = Ініціювання · { $name }
products-milliseconds-between-one-hole-firing = Мілісекунди між спрацюванням однієї свердловини та наступної.
products-ms = мс
products-no-products = Немає засобів
products-remove = Видалити
products-update = Оновити

## Progress strings

progress-percent-done-total = { $percent } ({ $done } із { $total })
progress-task-finished = { $task }: завершено

## Project strings

project-item = Елемент

## Properties strings

properties-adds-view-dependent-rim-highlight = Додає підсвічування країв на межах блоків і матеріалів, що залежить від напрямку погляду. Вимкнення трохи знижує навантаження об'ємного рендерингу.
properties-block-model-downscale = Зниження роздільної здатності блочної моделі
properties-camera = Камера
properties-camera-clip-planes = Площини відсікання камери
properties-cap-while-resizing = Обмежувати під час зміни розміру
properties-dark-mode = Темний режим
properties-developer = Розробник
properties-downscale-rasters = Знизити роздільну здатність растрів
properties-edit-object = Редагувати об'єкт...
properties-field-view = Поле зору
properties-fps = кадр/с
properties-frame-counter = Лічильник кадрів
properties-frame-rate-cap = Обмеження частоти кадрів
properties-hz = Гц
properties-interface = Інтерфейс
properties-invert-horizontal = Перевернути по горизонталі
properties-invert-vertical = Перевернути по вертикалі
properties-limits-newly-loaded-geotiff-previews = Обмежує попередній перегляд нових GeoTIFF до 4096 пікселів по довшій стороні. Вимкніть для повної роздільної здатності в межах обмеження текстур GPU; це вимагає більше пам'яті.
properties-line-colour = Колір лінії
properties-look-sensitivity = Чутливість огляду
properties-max-clip-span = Максимальна протяжність кліпу
properties-move-layer = Перемістити в шар...
properties-near-clip-limit = Ближня межа відсікання
properties-orbit-sensitivity = Чутливість орбіти
properties-panel-chrome = Оформлення панелі
properties-performance = Продуктивність
properties-plan-mode = Режим плану
properties-presents-step-display-no-tearing = Відображається синхронно з екраном: без розривів кадру, частоту кадрів визначає екран. Якщо вимкнено, кадри відображаються одразу після малювання, і застосовується обмеження нижче.
properties-reflective-block-edges = Відбивні краї блоків
properties-restore-defaults = Відновити значення за замовчуванням
properties-show-console = Показати консоль
properties-shows-live-near-far-projection = Показує актуальні ближню й дальню відстані проєкції в рядку стану.
properties-snap-polling = Опитування прив'язки
properties-vertical-sync = Вертикальна синхронізація
properties-world-axis-gizmo = Світова вісь
properties-zoom-cursor = Масштаб до курсора
properties-zoom-sensitivity = Чутливість масштабування

## Screenshot strings

screenshot-could-not-encode-viewport-image = Не вдалося закодувати зображення області перегляду: { $error }
screenshot-could-not-map-viewport-screenshot = Не вдалося відобразити знімок області перегляду в пам'ять: { $error }
screenshot-could-not-save-viewport-image = Не вдалося зберегти зображення області перегляду { $path }: { $error }
screenshot-downloaded-viewport-image-file-name = Зображення області перегляду завантажено: { $file_name }
screenshot-saved-viewport-image-path = Зображення області перегляду збережено: { $path }
screenshot-viewport-image-download-failed-error = Не вдалося завантажити зображення області перегляду: { $error }

## Spatial strings

spatial-bvh-face-index-out-of-range = Індекс грані BVH { $index } виходить за межі сітки; підставляється вироджений трикутник

## State strings

state-above = на рівні або вище
state-activate-project = Активувати проект
state-all-open-incline-design-data = Усі відкриті дані Incline Design
state-apply-generated-rings = Застосувати створені кільця
state-apply-selection = Застосувати до вибраного
state-rotate-by-azimuth-dip = за азимутом { $azimuth }°, кутом нахилу { $dip }°
state-rotate-to-azimuth-dip = до азимута { $azimuth }°, кута нахилу { $dip }°
state-below = на рівні або нижче
state-centre-rotation = Центр обертання
state-checking-unsaved-work = Перевірка незбереженої роботи
state-choose-destination = Виберіть місце призначення
state-choose-one-more-files = Виберіть один або кілька файлів
state-clear-raster = Очистити растр
state-click-pit-shell-viewport = Клацніть оболонку кар'єру в області перегляду.
state-click-pit-stockpile-solid-viewport = Клацніть тіло кар'єру або складу в області перегляду.
state-click-surface-viewport = Клацніть поверхню в області перегляду.
state-click-topology-viewport = Клацніть топологію в області перегляду.
state-close-project = Закрити проект
state-colour-drillholes = Розфарбувати свердловини
state-copy-objects-layer = Копіювати об'єкти в шар
state-count-file-s = Файлів: { $count }
state-count-object-s-axis-value = Об'єктів: { $count } · { $axis } { $value }
state-count-object-s-closed = Об'єктів: { $count } · { $closed }
state-count-object-s-layer = Об'єктів: { $count } · { $layer }
state-count-object-s-weight = Об'єктів: { $count } · { $weight }
state-count-object-s-z-elevation = Об'єктів: { $count } · Z { $elevation }
state-create-point-cloud-tin = Створити TIN за хмарою точок
state-create-project = Створити проект
state-current-project = Поточний проект
state-cut-topology-pit-shell = Вирізати топологію за оболонкою кар'єру
state-cut-triangulation-polyline = Відсікти тріангуляцію полілінією
state-cut-triangulation-z = Відсікти тріангуляцію за Z
state-dark-mode = Темний режим
state-detached = Від'єднано
state-disabled = Вимкнено
state-discard-project-changes = Відхилити зміни в проекті
state-discard-replace-project = Відхилити й замінити проект
state-discarding-unsaved-changes = Скасування незбережених змін
state-docked = Закріплено
state-drape-raster = Накласти растр
state-drill-pattern = Сітка свердловин
state-duplicate-layer = Дублювати шар
state-east = Схід
state-enabled = Увімкнено
state-exit-incline-design = Вийти з Incline Design
state-export-block-model-csv = Експорт блочної моделі в CSV
state-export-layer-dxf = Експорт шару в DXF
state-export-omf = Експорт OMF
state-export-project-dxf = Експорт проекту в DXF
state-export-triangulation = Експорт тріангуляції
state-export-viewport-image = Експорт зображення області перегляду
state-finish-closed-polyline = Завершити замкнену полілінію
state-finish-open-polyline = Завершити розімкнену полілінію
state-fit-extents = Вписати в межі
state-fix-release-centre-both-views = Закріплює або звільняє центр, навколо якого обертаються обидва види
state-generate-contours = Створити горизонталі
state-hidden = Прихований
state-import-drillholes = Імпорт свердловин
state-import-omf = Імпорт OMF
state-import-point-cloud = Імпорт хмари точок
state-import-raster = Імпорт растра
state-import-triangulation = Імпорт тріангуляції
state-insert-intersection-points = Вставити точки перетину
state-insert-points-elevation = Вставити точки на висоті
state-keep-inside = Залишити всередині
state-keep-outside = Залишити зовні
state-kriged-block-model = Блочна модель за кригінгом
state-load-block-model = Завантажити блочну модель
state-load-drillholes = Завантажити свердловини
state-load-layer = Завантажити шар
state-load-point-cloud = Завантажити хмару точок
state-load-raster = Завантажити растр
state-load-triangulation = Завантажити тріангуляцію
state-locked-count-object-s = Заблоковано об'єктів: { $count }
state-major-minor = Основний { $major } · допоміжний { $minor }
state-move-axis-value = Перемістити на значення осі
state-move-objects-layer = Перемістити об'єкти в шар
state-name-count-holes = { $name } · свердловин: { $count }
state-name-count-object-s = { $name } · об'єктів: { $count }
state-name-z-min-z-max = { $name } · від { $z_min } до { $z_max }
state-next-edit = Наступна зміна
state-north = Північ
state-open-containing-folder = Відкрити папку з файлом
state-open-project = Відкрити проект
state-preserve-view-angle = Зберегти кут огляду
state-previous-edit = Попередня зміна
state-project-id = Проект { $id }
state-remove-block-model = Видалити блочну модель
state-remove-drillholes = Видалити свердловини
state-remove-point-cloud = Видалити хмару точок
state-remove-raster = Видалити растр
state-remove-triangulation = Видалити тріангуляцію
state-removed-from-active-triangulation = Видалено з активної тріангуляції
state-removed-from-every-triangulation = Видалено з усіх тріангуляцій
state-rename-kind = Перейменувати { $kind }
state-save-close-project = Зберегти й закрити проект
state-save-despite-unsupported-content = Зберегти попри непідтримуваний вміст
state-save-project = Зберегти проект як
state-save-replace-project = Зберегти й замінити проект
state-saving-current-project = Збереження поточного проекту
state-section-name = Розділ { $section }
state-select-layer-objects = Виберіть об'єкти шару
state-selected-objects = Вибрані об'єкти
state-selected-polylines = Вибрані полілінії
state-selected-scene-elements = Вибрані елементи сцени
state-set-block-model-variable = Задати змінну блочної моделі
state-set-drillhole-colour-preset = Задати кольорову схему свердловин
state-set-entity-lock = Задати блокування
state-set-grid = Задати сітку
state-set-layer-lock = Задати блокування шару
state-set-line-weight = Задати товщину лінії
state-set-object-colour = Задати колір об'єкта
state-set-object-fill = Задати заливку об'єкта
state-set-point-visibility = Задати видимість точки
state-set-polyline-closed = Задати замкненість полілінії
state-set-raster-lock = Задати блокування растра
state-set-standard-view = Задати стандартний вигляд
state-set-topology-wireframes = Задати каркаси топології
state-set-triangulation-colour = Задати колір тріангуляції
state-show-console = Показати консоль
state-show-project = Показати проект
state-shown = Показано
state-slice-mode = Режим перерізу
state-slice-preview = Попередній перегляд перерізу
state-south = Південь
state-stem-contours = Горизонталі { $stem }
state-target-new-name = { $target } на «{ $new_name }»
state-trim-above = Обрізати зверху
state-trim-below = Обрізати знизу
state-trim-triangulation-surface = Обрізати тріангуляцію за поверхнею
state-undrape-raster = Прибрати накладання растра
state-undrape-rasters = Прибрати накладання растрів
state-unload-block-model = Вивантажити блочну модель
state-unload-drillholes = Вивантажити свердловини
state-unload-layer = Вивантажити шар
state-unload-point-cloud = Вивантажити хмару точок
state-unload-raster = Вивантажити растр
state-unload-triangulation = Вивантажити тріангуляцію
state-untitled-project = Проект без назви
state-use-typed-radius = Використати введений радіус
state-west = Захід

## Status strings

status-clip-near-far = Ближня/дальня площина/Δ: -- / -- / --
status-frame-rate = Частота кадрів

## Text strings

text-could-not-build-vector-mesh = Не вдалося побудувати векторну сітку для шрифту { $font }, гліф { $glyph }: { $error }
text-document-text-mesh-exceeded-its = Сітка тексту документа перевищила діапазон індексів u32

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = Спочатку виберіть набір даних свердловин для з'єднання
tie-in-count-connector-s = З'єднань: { $count }
tie-in-delete-tie-ins = Видалити з'єднання
tie-in-deleted-count-selected-tie-connector = Видалено вибраних з'єднань: { $count }
tie-in-hole = свердловина
tie-in-initiation-point-lifted-from-name = Точку ініціювання знято з { $name }
tie-in-initiation-point-set-name-delay = Точку ініціювання встановлено на { $name } із затримкою { $delay } мс
tie-in-select-delay-product-palette-before = Перед з'єднанням свердловин виберіть засіб затримки на палітрі
tie-in-tied-connectors = Створено з'єднань: { $count }, затримка { $delay } мс, засіб { $product }
tie-in-tied-connectors-replacing = Створено з'єднань: { $count }, затримка { $delay } мс, засіб { $product }; замінено: { $replaced }

## Toolbar strings

toolbar-fill-type = Тип заливки

## Toolbars strings

toolbars-auto-bench = Автоуступ
toolbars-bezier-polyline = Полілінія Безьє
toolbars-chamfer-polyline-corners = Зняти фаски з кутів полілінії
toolbars-create-text = Створити текст
toolbars-cursor-regular = Курсор: звичайний
toolbars-cursor-snap-line = Курсор: прив'язка до лінії
toolbars-cursor-snap-point = Курсор: прив'язка до точки
toolbars-cursor-snap-surface = Курсор: прив'язка до поверхні
toolbars-delete-points = Видалити точки
toolbars-explode-polyline-lines = Розбити полілінію на лінії
toolbars-fuse-polylines = Об'єднати полілінії
toolbars-measure-distance = Виміряти відстань
toolbars-new-layer = Новий шар
toolbars-split-polyline-points = Розділити полілінію за точками
toolbars-strike-dip = Простягання та падіння
toolbars-tool-not-available-section-view = { $tool } — недоступно у вигляді перерізу

## Tri strings

tri-sampling-method-help = Адаптивний метод концентрує вершини на складному рельєфі за похибкою апроксимації площиною; рівномірний розподіляє їх рівномірно. У майбутньому можуть з'явитися інші методи.
tri-adaptive-quadtree = Адаптивний (квадродерево)
tri-axis-range = Діапазон по { $axis }
tri-base-topology-will-receive-pit = Базова топографічна поверхня, на яку буде накладено форму кар'єру або відвалу.
tri-boundary-polyline = Гранична полілінія
tri-bridge-gaps-help = Перекриває розриви та ввігнутості межі поверхні, ширина яких менша за це значення. За 0 усе ще перекриваються розриви приблизно до розміру клітинки вибірки; більші значення заповнюють більші отвори та згладжують ввігнутості межі.
tri-budget = Бюджет за
tri-cancel-pick = Скасувати вибір
tri-candidate-detail = Відомості про варіант
tri-candidate-fine-cells-per-budgeted = Кількість дрібних клітинок-кандидатів на одну бюджетну вершину. Більше значення дає адаптивній вибірці більше свободи під час розміщення деталей, але сповільнює побудову.
tri-cap-surface-share-source-points = Обмежте поверхню часткою вихідних точок або точною кількістю вершин.
tri-choose-input-clicking-loaded-surface = Виберіть цей вхід, клацнувши по завантаженій поверхні в області перегляду
tri-choose-which-side-reference-topology = Виберіть бік опорної топографічної поверхні, який слід видалити з поверхні в їхній спільній області XY.
tri-clip = Відсікання
tri-clip-creates-new-triangulation-name = Обрізка створює нову тріангуляцію з цією назвою; вихідна поверхня не змінюється.
tri-clip-surface-polyline = Відсікти поверхню полілінією
tri-closed-pit-stockpile-solid-whose = Замкнене тіло кар'єру або відвалу, відкрита межа якого буде включена в результат.
tri-create-new-layer-contours-append = Створити новий шар для горизонталей або додати їх до наявного шару активного проекту.
tri-cut-topology-pit-shell = Вирізати топологію оболонкою кар'єру
tri-e-g-design-trimmed = наприклад, design_trimmed
tri-e-g-mysurf-cut = наприклад, mysurf_cut
tri-e-g-mysurf-slice = наприклад, mysurf_slice
tri-e-g-surface-contour = наприклад, surface_contour
tri-e-g-topo-cut = наприклад, topo_cut
tri-e-g-topo-pit = наприклад, topo_with_pit
tri-exact-number-surface-vertices-target = Точна цільова кількість вершин поверхні. Дуже великі значення сповільнюють побудову та потребують значної пам'яті.
tri-existing-ground-topology-will-cut = Наявна поверхня землі, яку буде обрізано оболонкою кар'єру.
tri-fill-holes-up = Заповнювати отвори до
tri-generate = Створити
tri-generate-contour-lines = Створити лінії горизонталей
tri-generate-upper-surface = Створити верхню поверхню
tri-hide-unload-sources = Приховати й вивантажити джерела
tri-higher-edge-will-enforced-each = У кожному конфлікті буде використано вище ребро. Нижні конфліктні сегменти не враховуватимуться як структурні лінії, а поверхня буде інтерпольована через ці ділянки. Вихідні полілінії не зміняться.
tri-breaklines-cross = Виділені ребра структурних ліній перетинаються або перекриваються на плані за різних позначок. Одна поверхня рельєфу не може відповідати обом ребрам.
tri-intervals-colours = Інтервали та кольори
tri-keep-clipped-topology-included-shape = Зберегти обрізану топографічну поверхню й включену форму як окремі тріангуляції, а не об'єднувати їх в один об'єкт.
tri-keep-inside-discards-surface-outside = «Залишити всередині» видаляє поверхню за полілінією. «Залишити зовні» вирізає з поверхні отвір у формі полілінії.
tri-keeps-only-surface-within-polyline = Залишає лише поверхню всередині межі полілінії.
tri-keep-surface-relation-help = Зберігає поверхню { $relation } топографічної поверхні в межах її охоплення за XY.
tri-layer-already-exists-select-above = Цей шар уже існує; виберіть його вище або вкажіть іншу назву.
tri-limit-z-range = Обмеження діапазону Z
tri-major = Основний
tri-max-edge-length = Максимальна довжина ребра
tri-merge = Об'єднати
tri-method = Метод
tri-min = Мін.
tri-minimum-maximum-elevations-retained = Мінімальна та максимальна позначки, що зберігаються у вихідній поверхні. Мінімум має бути нижчим за максимум.
tri-minor = Проміжний
tri-contour-interval-help = Малий інтервал керує звичайними горизонталями. Великий керує виділеними горизонталями та має бути не меншим за малий.
tri-move-cursor-over-loaded-surface = Наведіть курсор на завантажену поверхню.
tri-slice-output-name-help = Назва, присвоєна вихідній поверхні, обрізаній за висотою.
tri-name-assigned-merged-topology-pit = Назва, присвоєна результату злиття топології та кар'єру/складу.
tri-name-assigned-newly-created-contour = Назва, присвоєна новоствореному шару горизонталей.
tri-reconstruct-output-name-help = Назва, присвоєна реконструйованій тріангуляції.
tri-name-assigned-topology-after-pit = Назва, присвоєна топології після вирізання з неї оболонки кар'єру.
tri-name-assigned-trimmed-output-surface = Назва, присвоєна обрізаній вихідній поверхні.
tri-nearby-breakline-vertices-do-not = Близькі вершини структурних ліній не сходяться точно в одній точці, тому поверхню неможливо тріангулювати.
tri-new-layer = Новий шар
tri-new-layer-name = Назва нового шару
tri-once-merge-succeeds-unload-source = Після успішного об'єднання вивантажити вихідну топологію й тіло, щоб у сцені залишився лише об'єднаний результат.
tri-only-loaded-pickable = Вибирати можна лише завантажені тріангуляції.
tri-operation = Операція
tri-output-layer = Вихідний шар
tri-percentage = Відсоток
tri-percentage-cloud = Частка хмари
tri-pick-from-view = Виберіть з вигляду
tri-pit-design-surface-only-areas = Проектна поверхня кар'єру. Для виїмки використовуються лише ділянки, де вона проходить нижче топографічної поверхні.
tri-pit-shell = Оболонка кар'єру
tri-pit-stockpile-solid = Тіло кар'єру/відвалу
tri-recommended-weld-retry = Рекомендується: зварити та повторити
tri-reconstruct-help = Відновлює тріангульовану поверхню рельєфу з хмари точок. Адаптивна вибірка витрачає бюджет вершин там, де рельєф найскладніший, і зберігає плоскі ділянки розрідженими.
tri-reduce-budget-candidate-detail-if = Зменшіть бюджет або деталізацію кандидатів, якщо на комп'ютері мало оперативної пам'яті.
tri-reference-topology-help = Опорна топографічна поверхня, що визначає область обрізки іншої поверхні.
tri-reject-reconstructed-triangle-edges = Відкидає ребра відновлених трикутників, довші за цю відстань. Значення 0 вимикає обмеження довжини ребра.
tri-remove-inside-help = Видаляє поверхню всередині межі полілінії та залишає решту.
tri-removes-topology-where-pit-shell = Видаляє топографічну поверхню там, де оболонка кар'єру проходить нижче неї, щоб оболонка заповнила отвір. Шов слідує істинній тривимірній лінії контакту поверхонь; топографічна поверхня під частинами оболонки, що височіють над землею, зберігається.
tri-result = Результат
tri-save-two-entities = Зберегти як два об'єкти
tri-select = Вибрати…
tri-share-source-points-keep-fractions = Частка вихідних точок для збереження. Допускаються дробові значення, наприклад 0,125%.
tri-slice-triangulation-z-range = Переріз тріангуляції за діапазоном Z
tri-solution-generate-upper-surface = Рішення: створити верхню поверхню
tri-surface-trim = Оброблювана поверхня
tri-target-surface-help = Поверхня, яку буде змінено; вибрана топографічна поверхня залишиться без змін.
common-percent-suffix = %
tri-topology = Топографічна поверхня
tri-triangulation-failed = Тріангуляція не вдалася
tri-trim = Обрізати
tri-trim-topology = Обрізати за топологією
tri-uniform-grid = Рівномірна сітка
tri-up-target-point-count-points = До { $target } з { $point_count } точок стануть вершинами поверхні ({ $percent }%).
tri-use-full-surface-elevation-range = Використати весь діапазон висот поверхні
tri-vertex-count = Кількість вершин
tri-vertices-within-5-cm-xy = Вершини, розташовані в межах 5 см за XY і Z, отримають одну позицію в цій тріангуляції. Це може локально змістити утворювану поверхню на величину до 5 см; вихідні полілінії не зміняться.
tri-weld-retry = Зварити та повторити
tri-when-enabled-generate-contours-only = Якщо ввімкнено, горизонталі створюються лише між заданими мінімальною та максимальною позначками.

## Ui strings

ui-choose-offset-side = Виберіть бік зміщення
ui-choose-relimit-side = Виберіть бік для зміни межі
ui-click-circle-centre = Клацніть центр кола
ui-click-closed-polyline-use-blast = Клацніть замкнену полілінію, щоб використати її як контур блоку
ui-click-collar-add-edit-initiation = Клацніть устя, щоб додати або змінити точку ініціювання
ui-click-first-point-slice-line = Клацніть першу точку лінії перерізу
ui-click-first-vertex = Клацніть першу вершину
ui-click-perimeter-point-type-radius = Клацніть точку периметра або введіть радіус
ui-click-second-point-slice-line = Клацніть другу точку лінії перерізу
ui-click-second-vertex = Клацніть другу вершину
ui-click-use-pointer-radius = або клацніть, щоб використати радіус вказівника
ui-could-not-copy-text-browser = Не вдалося скопіювати текст у буфер обміну браузера: { $error }
ui-dip-horizontal-no-strike = { $dip } (горизонтально, без простягання)
ui-distance-meters = { $distance } м
ui-drag-ring-type-azimuth-dip = Перетягніть кільце або введіть азимут і кут нахилу
ui-each-hole-turns-about-its = кожна свердловина обертається навколо свого устя
ui-enter-positive-decimal-radius = Введіть додатний десятковий радіус
ui-esc-cancels = Esc — скасування
ui-no-delay-product-tie = Немає засобу сповільнення для з'єднання
ui-press-enter-use-typed-radius = Натисніть Enter, щоб використати введений радіус
ui-right-click-delay-palette-heading = клацніть правою кнопкою на заголовку палітри затримок, щоб додати
ui-select-designs = Виберіть проектні об'єкти
ui-select-drill-hole = Виберіть свердловину
ui-select-endpoint-join = Виберіть кінцеву точку для з'єднання
ui-select-first-crest-toe-point = Виберіть першу точку брівки/підошви уступу
ui-select-item = Виберіть пункт
ui-select-line-fuse = Виберіть лінію для злиття
ui-select-line-polyline = Виберіть лінію або полілінію
ui-select-line-relimit = Виберіть лінію для зміни межі
ui-select-next-line-fuse = Виберіть наступну лінію для злиття
ui-select-opposite-berm-point = Виберіть протилежну точку берми
ui-select-point = Виберіть точку
ui-select-polyline = Виберіть полілінію
ui-select-polyline-open-line = Виберіть полілінію або відкриту лінію
ui-select-polyline-vertex = Виберіть вершину полілінії
ui-select-second-crest-toe-point = Виберіть другу точку брівки/підошви уступу
ui-select-second-split-point = Виберіть другу точку розділення
ui-select-split-point = Виберіть точку розділення
ui-select-topologies = Виберіть топології
ui-slice-view = Режим перерізу
ui-strike-dip = простягання { $strike }° · { $dip }
ui-value-dip = падіння { $value }°

## Viewport strings

viewport-all-total-categories-keep-their = Усі категорії ({ $total }) зберігають свої кольори; чітко відображаються лише перші { $shown }
viewport-axis-maximum = Максимум { $axis }
viewport-axis-minimum = Мінімум { $axis }
viewport-bar-blast-timeline-placeholder = Хронологія вибуху [ЗАГЛУШКА]
viewport-bar-burden-relief-heatmap-placeholder = Теплова карта вивільнення ЛНО [ЗАГЛУШКА]
viewport-bar-color = Колір:
viewport-bar-contours-equal-time-placeholder = Горизонталі рівного часу [ЗАГЛУШКА]
viewport-bar-disable-flying-mode = Вимкнути режим польоту
viewport-bar-disable-x-ray-vision = Вимкнути рентгенівський режим
viewport-bar-drill-holes = Свердловини:
viewport-bar-enable-flying-mode = Увімкнути режим польоту
viewport-bar-enable-x-ray-vision = Увімкнути рентгенівський режим
viewport-bar-exit-slice-view = Вийти з режиму перерізу
viewport-bar-fill = Заливка:
viewport-bar-fix-centre-rotation = Закріпити центр обертання
viewport-bar-hide-points = Приховати точки
viewport-bar-hide-rl-grid = Сховати сітку позначок
viewport-bar-hide-wireframes = Приховати каркаси
viewport-bar-hide-xy-grid = Сховати сітку XY
viewport-bar-release-centre-rotation = Звільнити центр обертання
viewport-bar-show-points = Показати точки
viewport-bar-show-rl-grid = Показати сітку позначок
viewport-bar-show-wireframes = Показати каркаси
viewport-bar-show-xy-grid = Показати сітку XY
viewport-bar-vertical-slice-view = Вертикальний переріз
viewport-blank = (порожньо)
viewport-choose-active-block-model-variable = Виберіть активну змінну блочної моделі
viewport-choose-variable = Виберіть змінну
viewport-click-edit-color-right-click = Клацніть, щоб змінити колір; клацніть правою кнопкою, щоб видалити
viewport-click-type-boundary-s-value = Клацніть, щоб ввести значення цієї межі
viewport-colour-mapping = Відображення кольору
viewport-count-categories = Категорій: { $count }
viewport-count-category = Категорій: { $count }
viewport-double-click-add-boundary-here = Двічі клацніть, щоб додати межу тут
viewport-drag-move-middle-click-toggles = Перетягніть для переміщення · Середня кнопка перемикає ≤
viewport-drag-move-right-click-remove = Перетягніть для переміщення · Клацніть правою кнопкою для видалення · Середня кнопка перемикає ≤
viewport-e = С
viewport-edit-category-colour = Змінити колір цієї категорії
viewport-edit-colour-used-empty-values = Змінити колір порожніх значень
viewport-empty = (порожньо)
viewport-empty-hidden = (порожньо · приховано)
viewport-filter-variables = Фільтр змінних
viewport-navigation-hint = Середня кнопка + перетягування — панорама · Колесо — масштаб
viewport-navigation-hint-detach = Середня кнопка + перетягування — панорама · Колесо — масштаб · Клацання — від'єднати
viewport-n = Пн
viewport-no-data-variable = Немає даних для цієї змінної
viewport-no-matches = Немає збігів
viewport-no-usable-range = (немає придатного діапазону)
viewport-rebuild-variable-s-colours-from = Перебудувати кольори цієї змінної за її даними
viewport-reset = Скинути
viewport-restore-full-model-range = Відновити повний діапазон моделі
