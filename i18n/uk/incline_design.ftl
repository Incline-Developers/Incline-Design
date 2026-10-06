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
tri-create-help = Тріангулює об'єкти, вибрані під час відкриття цього діалогу. Закрийте його, щоб змінити вибір.
tri-create-type-label = Тип тріангуляції
tri-create-type-help =
    «Відкрита поверхня» створює полотно рельєфного типу. «Тіло» створює
    повністю замкнену сітку й потребує вхідних даних, що утворюють
    герметичну межу.
tri-create-output-name = Назва результату
tri-create-output-name-help = Назва, яку буде присвоєно створеній тріангуляції.
tri-create-output-name-hint = назва тріангуляції
tri-create-run = Тріангулювати
tri-selection-none = Вибрані об'єкти більше недоступні.

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
tri-count-circles =
    { $count ->
        [one] { $count } коло
        [few] { $count } кола
       *[other] { $count } кіл
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
status-faces = Грані: { $drawn } / { $total } (фрагментів: { $drawn_chunks }/{ $total_chunks })
status-clip = Ближня/дальня площина/Δ: { $near } / { $far } / { $delta } м
status-points = Точки: { $drawn } / { $target } із { $total } (фрагментів: { $drawn_chunks }/{ $total_chunks })

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
preferences-ui-size = Розмір інтерфейсу
preferences-ui-size-help = Налаштовує текст і елементи керування відносно звичайного масштабування екрана вашого пристрою. 100% — розмір за замовчуванням. Роздільна здатність екрана та розмір вікна не зменшують інтерфейс.
relimit-select-boundary = Виберіть полілінію або коло для зміни межі
relimit-click-boundary = Клацніть полілінію або коло для перетину…
relimit-mode-help = «Перетин» переміщує один кінець до полілінії або кола. «Абсолютно» задає остаточну довжину лінії. «Відносно» додає або віднімає довжину.
browser-graphics-device-lost = Браузер втратив графічний пристрій. Відкрийте цю сторінку знову в новій вкладці. Подробиці про GPU: { $message }

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
block-model-no-block-model-selected = Блочну модель не вибрано
block-model-no-drill-holes-selected = Свердловини не вибрано
block-model-nugget = Наґет
block-model-numeric-interval-fields-interpolate = Числові поля інтервалів для інтерполяції. Кожне вибране поле стає змінною блочної моделі.
block-model-kriging-help = Звичайний кригінг оцінює числові інтервали свердловин у центрі кожного блока з використанням сферичної варіограми.
block-model-partial-sill = Частковий поріг
block-model-range-search-radius = Діапазон / радіус пошуку
block-model-range-help = Зразки, розташовані далі за цю відстань, виключаються; коваріація досягає нуля на цьому діапазоні.
block-model-select-all = Виберіть усе
block-model-selected-block-model-whose-blocks = Вибрана блочна модель, блоки якої за порогом перетворюються на тіло. Закрийте діалог, щоб вибрати іншу.
block-model-source-drill-holes-help = Вибрана колекція свердловин, числові інтервали якої оцінюються в блоки. Закрийте діалог, щоб оцінювати з іншої.
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
borehole-inspector-checking-linked-geophysics-file = Перевірка пов'язаного файлу геофізики...
borehole-inspector-close-inspector = Закрити інспектор
borehole-inspector-data = Дані
borehole-inspector-display = Відображення
borehole-inspector-file-not-where-was-linked = Файлу { $file } немає там, звідки його пов'язано.
borehole-inspector-guessed-name = Визначено за назвою
borehole-inspector-hold-hole-while-you-work = Утримуйте цю свердловину, поки працюєте з навколишніми.
borehole-inspector-holding-hole-click-follow-selection = Свердловину утримано. Клацніть, щоб знову стежити за вибором.
borehole-inspector-log = Каротаж
borehole-inspector-no-hole-inspected = Свердловину не досліджується
borehole-inspector-pick-file = Виберіть { $file }...
borehole-inspector-pick-file-again-show-its = Виберіть { $file } ще раз, щоб показати його геофізику: сторінка браузера не може самостійно повторно відкрити файл.
borehole-inspector-reading-geophysics-file-its-index = Читання файлу геофізики для створення індексу; прогрес показано в рядку стану.
borehole-inspector-strat = Страт.
borehole-inspector-summary = Підсумок
canvas-circle-summary = Коло | Шар: { $layer } | радіус { $radius }

## Canvas strings

canvas-not-selectable-closed-polyline = Не можна вибрати | Виберіть замкнену полілінію
canvas-polyline-summary = Полілінія | Шар: { $layer } | Вершин: { $count }
canvas-surface-name = Поверхня | { $name }
canvas-trimmed = Підрізана
cinematic-shadows-method = Тіні кінематографічного вигляду: { $method }

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
cmd-commands-count-control-string-s = Контрольних ліній: { $count }
cmd-commands-count-control-string-s-layer = Контрольних ліній на «{ $layer }»: { $count }
cmd-commands-count-point-s-across-layers = Точок на шарах ({ $layers }): { $count }
cmd-commands-count-point-s-layer = Точок на «{ $layer }»: { $count }
cmd-commands-kind-layer = { $kind } на «{ $layer }»
cmd-commands-no-control-strings = Немає контрольних ліній
cmd-commands-no-extent = Немає меж
cmd-commands-select-holes-place-reference-points = Виберіть свердловини, на яких розмістити опорні точки
cmd-triangulate-needs-selection = Виберіть об'єкти для тріангуляції перед запуском команди «Створити тріангуляцію»
cmd-commands-select-one-loaded-block-model = Виберіть одну завантажену блочну модель, перш ніж створювати з неї тріангуляцію руди
cmd-commands-select-one-loaded-drill-hole = Виберіть одну завантажену колекцію свердловин, перш ніж створювати з неї блочну модель
cmd-commands-select-one-loaded-point-cloud = Виберіть одну завантажену хмару точок, перш ніж створювати з неї тріангуляцію
cmd-contours-needs-triangulation = Виберіть одну завантажену тріангуляцію, перш ніж створювати з неї горизонталі
cmd-slice-needs-triangulation = Виберіть одну завантажену тріангуляцію, перш ніж розсікати її за діапазоном Z
cmd-commands-select-one-loaded-triangulation-one = Виберіть одну завантажену тріангуляцію та одну замкнену полілінію перед відсіканням
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
cmd-drill-hole-choose-drillhole-source-files-again = Виберіть вихідні файли свердловин ще раз
cmd-drill-hole-drill-pattern-too-large-contains = Сітка свердловин завелика або містить недопустимі координати устя
cmd-drill-hole-drillhole-field-label-has-count = Поле свердловин «{ $label }» має { $count } різних кодів — більше, ніж зазвичай буває в кодованому полі; воно схоже на довільний текст, а не на категоріальне поле, але всі коди збережено та розфарбовано
cmd-drill-hole-enter-name-drill-pattern = Введіть назву сітки свердловин
cmd-drill-hole-failed-load-drillholes-error = Не вдалося завантажити свердловини: { $error }
cmd-drill-hole-depth-must-be-positive = Глибина свердловини має бути більшою за нуль
cmd-drill-hole-diameter-must-be-positive = Діаметр свердловини має бути більшим за нуль
cmd-drill-hole-loaded-drillhole-dataset-name-holes = Завантажено набір свердловин «{ $name }»: свердловин — { $holes }, кольорових полів — { $fields }
cmd-drill-hole-name-already-loading = «{ $name }» уже завантажується
cmd-drill-hole-name-reason = «{ $name }»: { $reason }
cmd-drill-hole-name-side = { $name } { $side }
cmd-drill-hole-name-working-section-side = { $name }, робочий пласт, { $side }
cmd-drill-hole-no-hole-holds-value-field = У цьому полі жодна свердловина не містить «{ $value }»
cmd-drill-hole-only-mapped-csv-bundles-imported = У браузері імпортуються лише набори CSV зі зіставленням
cmd-drill-hole-pattern-contains-no-holes = Сітка не містить свердловин
cmd-drill-hole-reading-name = Читання { $name }
cmd-drill-hole-reference-points-used-holes-placed = Опорні точки: розміщено на свердловинах — { $used }, без «{ $value }» — { $absent }, позначено як можливі повтори через розлом — { $flagged }
cmd-drill-hole-uppermost-run-used-flagged-holes = Використано найвищий інтервал, позначені свердловини: { $holes }
cmd-drill-hole-working-section-name-not-same = Робочий пласт «{ $name }» не однаковий у всіх вибраних наборах; використано власний пласт кожного набору.
cmd-drill-hole-working-sections-not-kept-dataset = Робочі пласти в «{ $dataset }» не збережено. { $reasons }
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
cmd-file-downhole-geophysics-csv = CSV свердловинної геофізики
cmd-file-downloaded-description-file-name = Завантажено — { $description }: { $file_name }
cmd-file-drillhole-csv-export-failed-error = Помилка експорту CSV свердловин: { $error }
cmd-file-dxf-download-encoding-failed-error = Помилка кодування завантажуваного DXF: { $error }
cmd-file-dxf-import-failed-error = Помилка імпорту DXF: { $error }
cmd-file-encoding-block-model-csv-download = Кодування завантаження CSV блочної моделі…
cmd-file-encoding-dxf-download = Кодування завантаження DXF…
cmd-file-encoding-triangulation-download = Кодування завантаження тріангуляції…
cmd-file-exit-deferred-exports = Вихід відкладено до завершення фонового експорту
cmd-file-exit-requested-no-unsaved-changes = Запрошено вихід, незбережених змін немає
cmd-file-exported-block-model-csv-path = CSV блочної моделі експортовано в { $path }
cmd-file-exported-description-dxf-path = { $description } експортовано в DXF: { $path }
cmd-file-exported-three-drillhole-csvs-path = Три CSV свердловин експортовано в { $path }
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
cmd-file-save-project-already-running-save = Збереження цього проекту вже виконується; збережіть ще раз після його завершення
cmd-file-save-worker-ended-without-result = Процес збереження завершився без результату
cmd-file-saved-project-as = Проект збережено як: { $path }
cmd-file-saved-project = Проект збережено: { $path }
cmd-file-saving-browser-storage = Збереження в сховищі браузера…
cmd-file-selected-block-model-no-longer = Вибрана блочна модель більше не завантажена
cmd-file-selected-drillhole-dataset-no-longer = Вибраний набір свердловин більше не завантажений
cmd-file-switching-project = Перемикання проекту…
cmd-file-triangulation-download-encoding-failed = Помилка кодування завантажуваної тріангуляції: { $error }
cmd-file-user-chose-exit-without-saving = Користувач вирішив вийти без збереження
cmd-file-user-requested-exit-project-export = Користувач запросив вихід (потрібне підтвердження експорту проекту або незбереженої роботи)
cmd-file-viewport = Область перегляду
cmd-file-wait-current-project-save-finish = Дочекайтеся завершення збереження поточного проекту
cmd-file-wait-current-project-switch-finish = Дочекайтеся завершення перемикання поточного проекту
cmd-file-wait-project-operation-finish-before = Перед скасуванням змін дочекайтеся завершення операції з проектом
cmd-file-wait-project-revert-finish-before = Перед збереженням дочекайтеся завершення відкату проекту
cmd-folder-collection-named-name-already-exists = Колекція «{ $name }» уже існує
cmd-folder-collection-no-longer-exists = Ця колекція більше не існує
cmd-folder-created-collection-name = Створено колекцію «{ $name }»
cmd-folder-deleted-collection-name = Видалено колекцію «{ $name }»
cmd-folder-moved-item-into-collection-name = Елемент переміщено до колекції «{ $name }»
cmd-folder-moved-item-root-section = Елемент переміщено в корінь розділу { $section }
cmd-folder-renamed-collection-before-after = Колекцію «{ $before }» перейменовано на «{ $after }»
cmd-folder-section-cannot-hold-item = Цей розділ не може містити цей елемент
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
cmd-offset-skipped-count-circle-s-offset = Пропущено кіл: { $count }; відстань зміщення більша за радіус
cmd-omf-could-not-open-project-source = Не вдалося відкрити проект { $source_name }: { $error }
cmd-omf-create-open-project-before-merging = Перед об'єднанням даних створіть або відкрийте проект
cmd-omf-dataset-name-count-working-section = Набір «{ $name }»: не вдалося відновити робочих пластів — { $count }: { $details }
cmd-omf-field-codes-partly-coloured = Набір «{ $name }»: поле «{ $field }» збережено з розфарбованими кодами: { $saved } із { $total }; решті призначено згенеровані кольори.
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
cmd-point-cloud-classified = Класифіковано { $name }: ґрунт — { $ground }, рослинність — { $vegetation }, шум — { $noise } із { $count } точок
cmd-point-cloud-classifying-point-clouds = Класифікація хмар точок
cmd-point-cloud-join-dropped-classifications = Класифікацію точок відкинуто: деякі з об'єднаних хмар не класифіковані, а частково класифіковану хмару не можна відфільтрувати до ґрунту.
cmd-point-cloud-failed-classify-point-clouds-error = Не вдалося класифікувати хмари точок: { $error }
cmd-point-cloud-failed-join-point-clouds-error = Не вдалося об'єднати хмари точок: { $error }
cmd-point-cloud-failed-load-point-cloud-error = Не вдалося завантажити хмару точок: { $error }
cmd-point-cloud-joined-count-clouds-into-name = Хмар об'єднано: { $count } → { $name } (точок: { $points })
cmd-point-cloud-joining-name = Об'єднання { $name }
cmd-point-cloud-loaded-point-cloud-name-count = Завантажено хмару точок { $name } ({ $count } точок)
cmd-point-cloud-point-cloud-classification-discarded = Класифікацію хмари точок відхилено: хмара змінилася під час виконання. Запустіть ще раз.
cmd-point-cloud-point-cloud-loader-disconnected-path = Завантажувач хмари точок відключився для { $path }
cmd-point-cloud-select-one-more-loaded-point = Виберіть одну або кілька завантажених хмар точок перед їх класифікацією
cmd-point-cloud-select-two-more-loaded-point = Виберіть дві або більше завантажених хмар точок перед їх об'єднанням
cmd-point-cloud-tin-max-edge-disabled = (макс. ребро вимкнено)
cmd-point-cloud-tin-max-edge-max-edge = (макс. ребро { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = Помилка створення TIN хмари точок: { $error }
cmd-point-cloud-tin-filtered-ground = TIN рельєфу: відфільтровано до точок ґрунту: { $ground } із { $total }
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
cmd-reference-surface-build-surface-failed-error = Помилка побудови поверхні: { $error }
cmd-reference-surface-building-surface = Побудова поверхні…
cmd-reference-surface-built-surface-name-from-vertex = Побудовано поверхню { $name } з точок ({ $vertex_count }) у гранях ({ $face_count }), z у межах від { $low } до { $high }{ $support }{ $coincident }{ $controls }
cmd-reference-surface-control-string-index-could-not = Контрольну лінію { $index } не вдалося додати до сітки
cmd-reference-surface-control-string-index-crosses-itself = Контрольна лінія { $index } перетинає саму себе в плані
cmd-reference-surface-control-string-index-doubles-back = Контрольна лінія { $index } повертає сама на себе в плані
cmd-reference-surface-control-string-index-ends-where = Контрольна лінія { $index } закінчується там, де починається; замкніть її, щоб використовувати як маску
cmd-reference-surface-control-string-index-has-count = Контрольна лінія { $index } має різних вершин: { $count }; для контрольної лінії потрібно щонайменше { $minimum }
cmd-reference-surface-control-string-index-has-non = Контрольна лінія { $index } має нескінченні координати
cmd-reference-surface-control-string-index-no-longer = Контрольна лінія { $index } більше недоступна
cmd-reference-surface-control-string-index-runs-along = Контрольна лінія { $index } проходить уздовж лінії меж у плані; це поки не підтримується
cmd-reference-surface-control-string-overrides-pick-x = Контрольна лінія перекриває вибір у ({ $x }, { $y }): вибір { $pick } м, контроль { $control } м, різниця { $difference } м
cmd-reference-surface-control-strings-b-disagree-x = Контрольні лінії { $a } і { $b } не збігаються в ({ $x }, { $y }): { $za } м проти { $zb } м, різниця { $difference } м
cmd-reference-surface-control-strings-b-run-along = Контрольні лінії { $a } і { $b } проходять одна вздовж одної в плані; це поки не підтримується
cmd-reference-surface-count-control-string-s-vertices = ; контрольних ліній: { $count }, вершин: { $vertices }{ $crossings }
cmd-reference-surface-count-point-s-inside-extent = Точок усередині меж: { $count }; для поверхні потрібно щонайменше { $minimum }
cmd-reference-surface-count-point-s-outside-extent = ; точок поза межами, що сформували поверхню як опорні: { $count }
cmd-reference-surface-count-point-s-selected-surface = Вибрано точок: { $count }; для поверхні потрібно щонайменше { $minimum }
cmd-reference-surface-count-point-s-shared-plan = ; точок зі спільним положенням у плані, збережених один раз: { $count }
cmd-reference-surface-delaunay-insert-failed-error = Помилка вставки Делоне: { $error }
cmd-reference-surface-extent-must-closed-string = Межа має бути замкненою лінією
cmd-reference-surface-extent-string-crosses-itself-plan = Лінія меж перетинає саму себе в плані
cmd-reference-surface-extent-string-has-non-finite = Лінія меж має нескінченні координати
cmd-reference-surface-extent-string-needs-least-three = Лінія меж потребує щонайменше трьох різних вершин
cmd-reference-surface-extent-string-no-longer-available = Лінія меж більше недоступна
cmd-reference-surface-meeting-count-crossing-s = що сходяться в перетинах: { $count }
cmd-reference-surface-and-more = , … і ще { $more }
cmd-reference-surface-no-mask-selected-surface-unclipped = Маску не вибрано; поверхню не обрізано
cmd-reference-surface-no-part-surface-falls-inside = Жодна частина поверхні не потрапляє в межі
cmd-reference-surface-open-project-before-building-surface = Відкрийте проект перед побудовою поверхні
cmd-reference-surface-points-collinear-plan-surface-needs = Точки колінеарні в плані; для поверхні потрібні три неколінеарні
cmd-reference-surface-select-exactly-one-closed-string = Виберіть рівно одну замкнену лінію, за якою відсікти поверхню
cmd-reference-surface-selected-point-has-non-finite = Вибрана точка має нескінченні координати
cmd-reference-surface-selected-points-span-count-layers = Вибрані точки розташовані на шарах: { $count }; поверхню розміщено в розділі { $section }
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
cmd-split-polyline-into-two = Вихідну полілінію розділено на дві розімкнені полілінії
cmd-text-edit-finished = Редагування тексту об'єкта { $object_id } завершено
cmd-text-updated = Текст об'єкта { $object_id } оновлено
cmd-view-centre-rotation-not-available-flying = Центр обертання недоступний у режимі польоту
cmd-view-fixed-centre-rotation-x-y = Центр обертання закріплено в точці { $x }, { $y }, { $z }
cmd-view-no-point-under-cursor-fix = Під курсором немає точки, щоб закріпити на ній центр обертання
cmd-view-released-centre-rotation = Центр обертання звільнено
cmd-view-reset-view-fit-extents = Скинути вигляд (вписати в межі)
cmd-view-reset-view-plan-same-distance = Скинути вигляд (вигляд у плані з тієї ж відстані; клацніть ще раз, щоб вписати в межі)
cmd-view-set-cinematic-view-enabled = Кінематографічний вигляд = { $enabled }
cmd-view-set-topology-wireframes-enabled = Каркас топології = { $enabled }
cmd-view-set-view-points-enabled = Відображення точок = { $enabled }
cmd-view-set-xy-grid-enabled = Сітку XY увімкнено = { $enabled }
cmd-view-zoom-extents-preserving-angle = Масштабувати до меж (зі збереженням кута)

## Common strings

common-add-product = Додати засіб
common-appearance = Вигляд...
common-background = Тло
common-block-model = Блочна модель
common-block-models = Блочні моделі
common-borehole-inspector = Інспектор свердловини
common-build-surface = Побудувати поверхню
common-build-surface-ellipsis = Побудувати поверхню...
common-cancelled = Скасовано
common-chamfer = Фаска
common-choose = Виберіть...
common-circle = Коло
common-classify = Класифікувати
common-classify-point-clouds = Класифікувати хмари точок
common-click-point-fix-centre-rotation = Клацніть точку, щоб закріпити на ній центр обертання
common-clip-surface-polyline = Відсікти поверхню полілінією...
common-closed = Закрито
common-collection = Колекція
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
common-delete-collection = Видалити колекцію
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
common-file-was-left-out-downhole = Файл { $file } виключено зі свердловинної геофізики: { $error }
common-filter = Фільтр
common-fly-mode = Режим польоту
common-generate-contour-lines = Створити лінії горизонталей...
common-hide-all = Приховати все
common-hide-selection = Приховати вибір
common-hole-id = ID свердловини
common-ignore = Ігнорувати
common-import-csv-block-model = Імпорт блочної моделі CSV
common-import-dxf = Імпорт DXF
common-incline-design-project = Проект Incline Design
common-join = Об'єднати...
common-join-point-clouds = Об'єднати хмари точок
common-joined-cloud = Об'єднана хмара
common-layer = Шар
common-legend = Легенда
common-line = Лінія
common-line-weight = Товщина лінії
common-link-geophysics = Пов'язати геофізику...
common-load-drillholes-before-linking-geophysics = Завантажте набір свердловин, перш ніж пов'язувати з ним геофізику
common-lock-all = Заблокувати все
common-lock-selection = Заблокувати вибір
common-m = м
common-max = Макс
common-merge-shell-into-topology = Злити оболонку з топологією
common-merge-shell-into-topology-ellipsis = Злити оболонку з топологією...
common-modelling = Моделювання
common-move-collar = Перемістити устя
common-move-collection = Перемістити до колекції
common-move-design = Перемістити об'єкт
common-move-selection = Перемістити вибір
common-name-has-no-readable-size = Для { $name } неможливо визначити розмір
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
common-reference-points = Опорні точки...
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
common-unknown = Невідомо
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
csv-drill-hole-rows-for-undefined-holes = Рядків для свердловини, якої немає в геометрії набору: { $count }
csv-drill-hole-count-rows-were-skipped-total = Усього пропущено рядків: { $count }
csv-drill-hole-csv-file-empty = Файл CSV порожній
csv-drill-hole-csv-has-too-many-unreadable = У CSV забагато нечитабельних байтів для відновлення; імовірно, він у застарілому кодуванні — збережіть його як UTF-8 та імпортуйте ще раз
csv-drill-hole-csv-header-has-no-columns = Заголовок CSV не має стовпців
csv-drill-hole-csv-headers-must-nonblank-unique = Заголовки CSV мають бути непорожніми й унікальними
csv-drill-hole-geophysics-needs-geometry = Свердловинна геофізика потребує в наборі файлу устьїв або явних сегментів, до свердловин якого її буде приєднано
csv-drill-hole-azimuth-out-of-range = У { $file } рядків з азимутом поза діапазоном від 0 до 360: { $count }
csv-drill-hole-dip-out-of-range = У { $file } рядків із кутом падіння поза діапазоном від -90 до 90: { $count }; ці рядки прочитано без напрямку
csv-drill-hole-file-inclination-values-could-angle = У { $file } усі значення нахилу, що можуть бути кутом, не перевищують нуля, тому стовпець прочитано як кут падіння з від'ємним спрямуванням донизу
csv-drill-hole-file-maps-gamma-density-column = У { $file } стовпець гамма або густини зіставлено двічі
csv-drill-hole-invalid-utf8 = { $file } не є коректним UTF-8; нечитабельних байтів замінено: { $count }, у комірках: { $cells }; пошкоджена комірка не читається як дані
csv-drill-hole-file-requires-gamma-density-column = { $file } потребує стовпця гамма або густини
csv-drill-hole-row-undefined-hole = { $file }, рядок { $row }: DHID «{ $dhid }» — свердловина, якої немає в геометрії набору
csv-drill-hole-holes-hole-s-carry-overlapping = Свердловин із перекривними інтервалами, наприклад пласт, описаний разом із його частинами: { $holes }; { $summary }
csv-drill-hole-skipped-row-reason = Пропущено рядок: { $reason }
csv-geophysics-above-5 = вище 5
csv-geophysics-below-0-5 = нижче 0,5
csv-geophysics-count-more = (ще +{ $count })
csv-geophysics-count-rows-were-skipped-total = Усього пропущено рядків у { $file }: { $count }
csv-geophysics-csv-has-record-longer-than = У CSV є запис довший за { $limit } МіБ: у файлі немає розривів рядків там, де вони мають бути в CSV, або це не текст
csv-geophysics-csv-has-unterminated-quoted-field = У CSV є незакрите поле в лапках
csv-geophysics-curve-file-was-left-out = { $curve } у { $file } виключено: більшість його показань { $side }, тому медіана виходить за межі 0,5–5 г/см³, а одиниця виміру виглядає хибною (очікується г/см³). Incline не перетворює одиниці; виправте експорт і пов'яжіть ще раз
csv-geophysics-file-empty = { $file } порожній
csv-geophysics-file-has-no-curve-no = У { $file } немає кривої: жоден стовпець, крім ID свердловини та глибини, не містить чисел
csv-geophysics-file-mapping-has-mapped-columns = Зіставлення { $file } має стовпців: { $mapped }, у CSV: { $found }
csv-geophysics-file-no-longer-matches-its = { $file } більше не відповідає своєму індексу: пов'яжіть його ще раз
csv-geophysics-file-not-grouped-hole-its = { $file } не згруповано за свердловинами: рядки його свердловин розкидано по надто багатьох послідовностях. Відсортуйте за ID свердловини, потім за глибиною, і пов'яжіть ще раз
csv-geophysics-file-requires-one-dhid-one = { $file } потребує одного стовпця DHID і одного стовпця глибини
csv-geophysics-row-blank-hole-id = { $file }, рядок { $row }: порожній ID свердловини
csv-geophysics-row-column-count = { $file }, рядок { $row }: стовпців { $found }, очікувалося { $expected }
csv-geophysics-row-negative-depth = { $file }, рядок { $row }: від'ємна глибина
csv-geophysics-row-no-depth = { $file }, рядок { $row }: глибину неможливо прочитати
csv-geophysics-file-s-path-not-valid = шлях до файлу не є коректним UTF-8, який проект не може зберегти: перейменуйте файл або його папку та пов'яжіть ще раз
csv-geophysics-rows-skipped = { $file }: не вдалося прочитати рядків: { $skipped } із { $rows }; причини наведено в консолі
csv-geophysics-runs-not-grouped = Геофізика для свердловин ({ $count }) надходить кількома послідовностями, не згрупованими за свердловиною; кожна наступна послідовність додає лише глибини, на яких у свердловини ще немає показань: { $holes }
csv-geophysics-linked-downhole-geophysics-from-file = Пов'язано свердловинну геофізику з { $file }: свердловин — { $holes }, криві { $curves }; прочитано рядків — { $rows }, пропущено — { $skipped }. Показання залишаються у файлі й читаються по одній свердловині
csv-geophysics-no-readings = немає показань
csv-geophysics-no-usable-depth-step = немає придатного кроку глибини
csv-geophysics-run-count-mismatch = Прочитано послідовностей { $hole }: { $read }, у зв'язку їх { $runs }
csv-geophysics-rows-geophysics-row-s-count = Рядків геофізики ({ $rows }) для свердловин ({ $count }), яких набір не визначає, не пов'язано: { $holes }
csv-geophysics-rows-readings-would-need-samples = Показань ({ $rows }) знадобилося б вибірок: { $samples }
csv-geophysics-run-hole-curve-was-not = Послідовність { $hole } { $curve } не збережено ({ $reason })
data-table-copy-selection = Копіювати вибране
data-table-copy-table = Копіювати таблицю
drill-hole-add = Додати
drill-hole-add-all = Додати все

## Drill strings

drill-hole-add-stop = Додати поріг
drill-hole-add-working-section = Додати робочий пласт
drill-hole-all-rendered-intervals-opaque-white = Усі непрозорі інтервали білі.
drill-hole-another-working-section-field-has = Інший робочий пласт цього поля має таку назву.
drill-hole-assumed = Припущено
drill-hole-burden-spacing-must-greater-than = Відстань між рядами та крок мають бути більшими за нуль
drill-hole-cache-drill-hole-set-name-has = Набір свердловин { $name } має { $count } свердловин і з'єднань, що перевищує { $capacity }, яку може нести підсвічування вибору: вибір набору загалом і далі підсвічує його, а вибір окремих свердловин — ні
drill-hole-cache-drill-hole-set-name-stations = Набір свердловин { $name }: станцій — { $stations }, сегментів — { $before } об'єднано до { $after }, комірок — { $cells }
drill-hole-choose-valid-closed-polyline = Виберіть допустиму замкнену полілінію
drill-hole-clear-filter = Очистити фільтр
drill-hole-code-already-in-section = { $code } уже є в робочому пласті { $section }.
drill-hole-code-outside-section-has-name = Код поза цим пластом має таку назву. Пласт може мати спільну назву лише з кодом, який він містить.
drill-hole-colour-scale = Колірна шкала
drill-hole-count-codes = Кодів: { $count }
drill-hole-count-codes-interval-no-logged = Кодів: { $count }. Інтервал без зареєстрованого значення залишається білим.
drill-hole-disc-diameter = Діаметр диска
drill-hole-appearance-title = Вигляд свердловин: { $name }
drill-hole-drilled-diameter = Від пробуреного діаметра
drill-hole-every-code-lists-already-another = Кожен код, який він містить, уже входить до іншого робочого пласта.
drill-hole-every-interval-value-colour-field = Кожен інтервал зі значенням у полі кольору малюється на лінії як диск такої ширини. Здалеку він ніколи не буває вужчим за кілька пікселів.
drill-hole-field = Поле
drill-hole-field-working-section = { $field } за робочим пластом
drill-hole-floor = Підошва
drill-hole-grayscale = Відтінки сірого
drill-hole-green-yellow-red = Зелений–жовтий–червоний
drill-hole-heat = Теплова
drill-hole-drilled-width-help = Свердловина в пробуреній ширині виглядає як труба поруч із геологією; набір із тисяч виглядає як килим.
drill-hole-line-width-help = Сама свердловина малюється лінією такої ширини за будь-якого масштабу.
drill-hole-however-far-eye-hole-drawn = Незалежно від віддалення, свердловина малюється принаймні такої ширини.
drill-hole-measured = Виміряно
drill-hole-name-working-section = { $name } (робочий пласт)
drill-hole-never-thinner-than = Не тонше за
drill-hole-new-section-name = Назва нового пласта
drill-hole-new-working-section = Новий робочий пласт
drill-hole-no-holes-fit-inside-boundary = За поточної відстані між рядами та кроку всередині цієї межі не поміщається жодної свердловини
drill-hole-part-code = Частина коду
drill-hole-pattern-too-many-holes = Сітка перевищує максимум у { $maximum } свердловин; збільшіть відстань між рядами або крок
drill-hole-preset = Пресет
drill-hole-px = пікс.
drill-hole-rainbow = Веселка
drill-hole-reset-colours = Скинути кольори
drill-hole-reset-preset = Скинути набір
drill-hole-reset-shown-colours = Скинути показані кольори
drill-hole-roof = Покрівля
drill-hole-rotation-offsets-must-contain-valid = Поворот і зміщення мають містити допустимі числа
drill-hole-selected-polyline-has-no-usable = Вибрана полілінія не має придатної площі в площині XY
drill-hole-shown-total-codes-shown = Показано кодів: { $shown } із { $total }
drill-hole-shown-total-rows-shown = Показано рядків: { $shown } із { $total }
drill-hole-smooth-interpolation = Плавна інтерполяція
drill-hole-spacing-would-scan-too-many = За такого кроку доведеться перевірити занадто багато клітинок сітки; збільшіть відстань між рядами або крок (максимум — { $maximum } свердловин)
drill-hole-square = Прямокутна
drill-hole-staggered = Шахова
drill-hole-stepped-bands = Ступінчасті смуги
drill-hole-string-discs = Лінія та диски
drill-hole-string-discs-where-intervals-overlap = Як лінія та диски: там, де інтервали перекриваються, диском малюється найкоротший.
drill-hole-string-width = Ширина лінії
drill-hole-style = Стиль
drill-hole-suggested-from-code-names-count = Запропоновано за назвами кодів ({ $count })
common-times-sign = ×
common-minus-sign = −
drill-hole-ticked-but-hidden-filter-count = Позначено, але приховано фільтром: { $count }
drill-hole-true-diameter = Справжній діаметр
drill-hole-unsupported-drillhole-source = Непідтримуване джерело свердловин
drill-hole-width = Ширина
drill-hole-working-section-needs-name = Робочий пласт потребує назви.
drill-hole-working-section-set-seams-plies = Робочий пласт — це набір пластів або пачок, що розробляються як одне ціле. Розфарбування за ним дає всьому набору один колір.
drill-hole-working-sections = Робочі пласти
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
explorer-empty-collection = Порожня колекція
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
explorer-new-collection = Нова колекція
explorer-no-collection = Без колекції
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
geophysics-checking-geophysics-files = Перевірка файлів геофізики
geophysics-downhole-geophysics-name-could-not = Не вдалося пов'язати свердловинну геофізику для «{ $name }»: { $error }
geophysics-file-changed = Файл геофізики змінився після індексації
geophysics-file-unreadable = Файл геофізики, пов'язаний із «{ $name }», неможливо прочитати за шляхом { $path } ({ $error }); пов'яжіть його ще раз через контекстне меню набору
geophysics-linked-changed-rereading = Геофізика, пов'язана з «{ $name }», змінилася після індексації; повторне читання
geophysics-hole-has-size-mib-geophysics = У { $hole } рядків геофізики на { $size } МіБ — більше, ніж читається для однієї свердловини
geophysics-hole-needs-size-mib-its = Для геофізики { $hole } потрібно { $size } МіБ, а браузер має менше: вивантажте інші елементи, потім вивантажте й завантажте цей набір ще раз
geophysics-linking-geophysics-name = Зв'язування геофізики з { $name }
geophysics-reading-geophysics-hole = Читання геофізики для { $hole }
geophysics-web-could-not-read-name-error = Не вдалося прочитати «{ $name }»: { $error }
geophysics-web-name-used-session-s-downhole = «{ $name }» використовується для свердловинної геофізики цього сеансу

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
init-gpu-limits-max-buffer-size = Обмеження GPU: max_buffer_size={ $max_buffer_size } МіБ, max_storage_buffer_binding_size={ $max_storage_buffer_binding_size } МіБ, max_storage_buffers_per_shader_stage={ $max_storage_buffers_per_shader_stage }, max_uniform_buffer_binding_size={ $max_uniform_buffer_binding_size } КіБ, max_texture_dimension_2d={ $max_texture_dimension_2d }, max_bind_groups={ $max_bind_groups }
init-gpu-supports-maximum-buffer-size = Максимальний розмір буфера відеоадаптера — { $size } МіБ; великі сцени можуть відображатися не повністю
init-surface-present-mode = Режим представлення поверхні: { $mode }
init-wgpu-error-continuing-error = Помилка wgpu (роботу продовжено): { $error }
input-could-not-read-name-error = не вдалося прочитати { $name }: { $error }
input-could-not-slice-name-error = не вдалося розсікти { $name }: { $error }
io-add-collar-file-explicit-segments = Додайте файл устьїв (або файл явних сегментів): свердловинна геофізика приєднується до свердловин, які він визначає.

## Io strings

io-ascii-points-xyz-pts = Точки ASCII (.xyz, .pts)
io-attribute = Атрибут
io-blank-header = (порожні заголовки)
io-block-model = Блочна модель:
io-choose-file-purpose-map-its = Виберіть призначення файлу, щоб зіставити його стовпці.
io-choose-loaded-block-model = Виберіть завантажену блочну модель
io-choose-loaded-dataset = Виберіть завантажений набір
io-choose-loaded-layer = Виберіть завантажений шар
io-choose-loaded-triangulation = Виберіть завантажену тріангуляцію
io-choose-purpose = Виберіть призначення…
io-choose-source-file-files-import = Виберіть вихідний файл або файли для імпорту.
io-collar = Устя
io-column-mapping = Зіставлення стовпців
io-comma-separated-values-csv = Значення, розділені комами (.csv)
io-csv-files = Файли CSV
io-dataset = Набір:
io-default = За замовчуванням
io-density-read-g-cc-exported = Густина, прочитана як г/см³, як експортовано. Крива, медіана якої не лежить між 0,5 і 5 г/см³, виключається з імпорту з попередженням, оскільки її одиниця виглядає хибною.
io-depth = Глибина
io-diameter = Діаметр
io-downhole-geophysics = Свердловинна геофізика
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
io-export-csv-drillholes = Експорт свердловин у CSV
io-export-dxf = Експорт DXF
io-export-one-layer = Експорт одного шару
io-export-open-mining-format-2 = Експорт у Open Mining Format 2
io-export-ply = Експорт PLY
io-export-stl = Експорт STL
io-export-wavefront-obj = Експорт Wavefront OBJ
io-gamma-api = Гамма (API)
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = Імпорт
io-import-ascii-point-cloud = Імпорт хмари точок ASCII
io-import-drillhole-csv-bundle = Імпорт пакета CSV свердловин
io-import-geotiff = Імпорт GeoTIFF
io-import-las-laz-point-cloud = Імпорт хмари точок LAS/LAZ
io-import-open-mining-format-2 = Імпорт з Open Mining Format 2
io-import-pcd-point-cloud = Імпорт хмари точок PCD
io-import-ply = Імпорт PLY
io-import-stl = Імпорт STL
io-import-wavefront-obj = Імпорт Wavefront OBJ
io-inclination = Нахил
io-interval = Інтервал
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-long-spaced-density-g-cc = Густина довгим зондом (г/см³)
io-mapped-csv-bundle-csv = Зіставлений пакет CSV (.csv)
io-measured-depth-down-hole-read = Виміряна глибина вздовж свердловини, прочитана як метри. Incline не перетворює одиниці: їх задає база даних, що експортувала файл.
io-model-file = Файл моделі
io-name-count-files = { $name } + файлів: { $count }
io-natural-gamma-read-api-units = Природна гамма, прочитана в одиницях API, як експортовано.
io-no-csv-chosen = Файл .csv не вибрано
io-no-csv-files-chosen = Файли CSV не вибрано
io-no-dxf-chosen = Файл .dxf не вибрано
io-no-omf-chosen = Файл .omf не вибрано
io-north-y = Північ / Y
io-open-mining-format-2-omf = Open Mining Format 2 (.omf)
io-ply = PLY (.ply)
io-point-cloud-data-pcd = Point Cloud Data (.pcd)
io-projects = Проекти
io-short-spaced-density-g-cc = Густина коротким зондом (г/см³)
io-source-file = Вихідний файл
io-start-x = Початкова X
io-start-y = Початкова Y
io-start-z = Початкова Z
io-stl = STL (.stl)
io-triangulation = Тріангуляція:
io-unmapped = Не зіставлено
io-wavefront-obj = Wavefront OBJ (.obj)
io-writes-three-files-beside-name = Записує три файли поруч із вибраною назвою: устя, інклінометрію та інтервали, у стовпцях, які імпортує цей діалог.

## Jobs strings

jobs-background-task-poll-label-ended = Фонове завдання «{ $poll_label }» завершилося без результату
jobs-cancelled-label-its-project-no = Скасовано «{ $label }»: його проект більше не активний
jobs-discarded-stale-result = Застарілий результат фонового завдання «{ $poll_label }» відхилено, оскільки вихідний об'єкт змінився або було закрито
jobs-drillhole-import = імпорт свердловин
log-traces-auto-from-hole = Авто, за цією свердловиною
log-traces-curve-no-reading = { $curve }: немає показань
log-traces-curve-value-unit = { $curve }: { $value } { $unit }
log-traces-custom-range = Власний діапазон
log-traces-default-colour = Колір за замовчуванням
log-traces-density-scale = Шкала густини
log-traces-depth-m = { $depth } м
log-traces-gamma = Гамма
log-traces-gamma-colour = Колір гамма
log-traces-gamma-scale = Шкала гамма
log-traces-percentile-range-no-data = Від 1-го до 99-го процентиля свердловини, округлено назовні. Для цієї свердловини поки немає даних.
log-traces-percentile-range = Від 1-го до 99-го процентиля свердловини, округлено назовні: { $range }.
log-traces-long-density = Густина довгим зондом
log-traces-long-density-colour = Колір густини довгим зондом
log-traces-min-max-unit = від { $min } до { $max } { $unit }
log-traces-reading = Читання...
log-traces-short-density = Густина коротким зондом
log-traces-short-density-colour = Колір густини коротким зондом

## Logging strings

logging-activity-completed = Операцію завершено
logging-activity-started = Операцію розпочато
logging-application-id-id = Ідентифікатор програми: { $id }
logging-application-name = Назва програми: { $name }
logging-application-startup = Запуск програми
logging-build-target-os-architecture = Цільова платформа збірки: { $os }-{ $architecture }
logging-completed = Завершено
logging-count-messages = Повідомлень: { $count }
logging-desktop-session-xdg-session-type = Сеанс робочого столу: XDG_SESSION_TYPE={ $session }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
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
omf-element-name-has-count-unreadable = Елемент «{ $name }» має нечитабельних робочих пластів: { $count }; їх пропущено
omf-element-unsupported-section = Елемент «{ $name }» посилається на розділ «{ $section }», який у цій збірці не може показувати такий тип елемента
omf-element-name-names-unknown-section = Елемент «{ $name }» посилається на невідомий розділ «{ $section }»
omf-ignoring-colour-map-omf-attribute = Кольорову карту атрибута OMF «{ $attribute }» проігноровано: { $error }
omf-mining-data-exported-incline = Гірничі дані, експортовані Incline
omf-import = Імпорт OMF
omf-texture = Текстура OMF
omf-validation-warnings = Попередження перевірки OMF: { $warnings }
omf-application-metadata-dropped = Метадані програми проекту «{ $application }» не зберігаються
omf-project-author-not-retained = Автор проекту не зберігається
omf-project-description-not-retained = Опис проекту не зберігається
omf-unsupported-metadata-keys = У проекті є непідтримувані ключі метаданих: { $keys }
omf-skipped-drillhole-data-saved-older = Пропущено дані свердловин, збережені в старішому форматі ({ $names }); імпортуйте їх ще раз із вихідних файлів

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
point-cloud-classify = Класифікувати
point-cloud-classify-vegetation = Класифікувати рослинність
point-cloud-cloth-resolution = Роздільність тканини
point-cloud-cloth-resolution-about-one-half = Роздільність тканини приблизно в півтора раза більша за крок точок найрозрідженішої з вибраних хмар, щоб під кожною частинкою були відбиття.
point-cloud-combine-selected-point-clouds-into = Об'єднує вибрані хмари точок в одну нову хмару, щоб можна було побудувати єдину тріангуляцію по всіх. Кольори окремих точок зберігаються; хмара без них вносить свій колір відображення.
point-cloud-selected-count = Вибрано: { $count } · точок: { $points }
point-cloud-delete-selected-clouds-from-project = Видалити вибрані хмари з проекту після завершення об'єднання, звільнивши пам'ять, яку інакше займала б їхня дублікатна копія.
point-cloud-flat-pads-structures = Плоский (майданчики, споруди)
point-cloud-ground-cloud-covers-steep-follows = Ґрунт, який охоплює хмара. «Крутий» спускається стінами від їхніх гребенів; «Плоский» використовує жорсткішу тканину, що перекриває великі будівлі та обладнання, але згладжує різкі переломи.
point-cloud-ground-threshold = Поріг ґрунту
point-cloud-how-far-around-each-point = На якій відстані навколо кожної точки рахувати сусідів.
point-cloud-join = Об'єднати
point-cloud-let-cloth-follow-walls-down = Дозволити тканині спускатися стінами від їхніх гребенів, де її жорсткість інакше тримала б її від уступу. Вимикайте лише на пологій місцевості, щільно заставленій обладнанням.
point-cloud-mark-each-point-ground-noise = Позначає кожну точку як ґрунт, шум або некласифіковану. Тканину притискають знизу до хмари, і вона осідає на поверхню ґрунту; точки в межах порога ґрунту від неї є ґрунтом. Наявні класи замінюються; скасування їх відновлює.
point-cloud-mark-isolated-returns-birds-dust = Позначає ізольовані відбиття (птахи, пил, помилки багатопроменевості) як шум до визначення ґрунту, щоб випадкова низька точка не затягнула тканину вниз.
point-cloud-mark-noise = Позначати шум
point-cloud-minimum-neighbours = Мінімум сусідів
point-cloud-name-assigned-joined-point-cloud = Назва, що призначається об'єднаній хмарі точок.
point-cloud-name-count-points = { $name } (точок: { $count })
point-cloud-noise-radius = Радіус шуму
point-cloud-point-clouds = Хмари точок
point-cloud-points-closer-than-settled-cloth = Точки ближче за цю відстань до осілої тканини, виміряну по її поверхні, є ґрунтом.
point-cloud-points-fewer-neighbours-than-within = Точки, у яких у межах радіуса шуму менше сусідів за це значення, є шумом.
point-cloud-raise-cloth-resolution-if-your = Збільште крок роздільності тканини, якщо на вашому комп'ютері менше ОЗП.
point-cloud-recommended = Рекомендовано
point-cloud-recover-steep-slopes = Відновлювати круті схили
point-cloud-relief-dumps-rolling-ground = Рельєф (відвали, горбиста місцевість)
point-cloud-remove-sources = Видалити джерела
point-cloud-resolution-m-points-spacing-m = { $resolution } м (точки на відстані ~{ $spacing } м)
point-cloud-selected-clouds-copied-into-joined = Вибрані хмари, скопійовані в об'єднану хмару. Закрийте діалог, щоб об'єднати інший набір.
point-cloud-selected-clouds-each-classified-its = Вибрані хмари, кожна класифікується окремо. Закрийте діалог, щоб класифікувати інший набір.
point-cloud-classify-help = Сортує відбиття навченим класифікатором, який читає форму точок навколо кожної: ґрунт, рослинність (за висотою: низька — до 1 м, середня — до 3 м або висока) та решта, наприклад будівлі й обладнання, що залишається некласифікованою. Вимкніть, щоб використовувати лише тканину.
point-cloud-spacing-cloth-s-particles-around = Крок частинок тканини. Хорошим початком є крок точок хмари; дрібніший точніше слідує ґрунту, але потребує щільніших точок.
point-cloud-steep-pit-walls-benches = Крутий (борти кар'єру, уступи)
point-cloud-terrain = Рельєф
point-cloud-use = Використовувати

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
properties-colours-each-point-cloud-chunk = Розфарбовує кожен фрагмент хмари точок, обводить рамку, за якою його відсікає піраміда видимості, та показує в рядку стану точки, намальовані в останньому кадрі, відносно цілі рівня деталізації та загальної кількості видимих.
properties-colours-each-surface-chunk-outlines = Розфарбовує кожен фрагмент поверхні, обводить рамку, за якою його відсікає піраміда видимості, та показує в рядку стану грані, намальовані в останньому кадрі, відносно загальної кількості видимих.
properties-dark-mode = Темний режим
properties-dataset = Набір даних
properties-developer = Розробник
properties-downscale-rasters = Знизити роздільну здатність растрів
properties-drillholes = Свердловини
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
properties-no-drillhole-datasets-open = Немає відкритих наборів свердловин.
properties-orbit-sensitivity = Чутливість орбіти
properties-panel-chrome = Оформлення панелі
properties-performance = Продуктивність
properties-plan-mode = Режим плану
properties-point-cloud-chunk-debug-view = Налагодження фрагментів хмари точок
properties-presents-step-display-no-tearing = Відображається синхронно з екраном: без розривів кадру, частоту кадрів визначає екран. Якщо вимкнено, кадри відображаються одразу після малювання, і застосовується обмеження нижче.
properties-reflective-block-edges = Відбивні краї блоків
properties-restore-defaults = Відновити значення за замовчуванням
properties-show-console = Показати консоль
properties-shows-live-near-far-projection = Показує актуальні ближню й дальню відстані проєкції в рядку стану.
properties-snap-polling = Опитування прив'язки
properties-surface-chunk-debug-view = Налагодження фрагментів поверхні
properties-vertical-sync = Вертикальна синхронізація
properties-world-axis-gizmo = Світова вісь
properties-zoom-cursor = Масштаб до курсора
properties-zoom-sensitivity = Чутливість масштабування
reference-points-count-holes-from-dataset = Свердловин із «{ $dataset }»: { $count }
reference-points-holes-from-datasets = Свердловин із наборів ({ $datasets }): { $count }
reference-points-holes = Свердловини
reference-points-holes-points-placed-selected-when = Свердловини, на яких розміщуються точки, як вибрано на момент відкриття діалогу. Закрийте діалог, щоб вибрати інші.
reference-points-make = Створити
reference-points-no-categorical-field = Немає категоріального поля
reference-points-no-values = Немає значень
reference-points-one-point-per-hole-boundary = Одна точка на свердловину на цій межі, як новий шар. Свердловина, що двічі містить пласт, дає найвищу точку й позначається.
reference-points-reference-points = Опорні точки
reference-points-side = Сторона
reference-points-working-section = Робочий пласт
reference-points-working-section-field = Поле робочого пласта
reference-surface-controls = Контролі
reference-surface-extent = Межі
reference-surface-points-outside-extent-still-shape = Точки поза межами й далі формують поверхню; до меж відсікається лише сама поверхня.
reference-surface-points-surface-built-from-selected = Точки, з яких будується поверхня, як вибрано на момент відкриття діалогу. Закрийте діалог, щоб вибрати інші.
reference-surface-extent-help = Вибрана замкнена лінія, за якою відсікається готова поверхня; точки поза нею й далі формують поверхню.
reference-surface-selected-open-strings-surface-made = Вибрані розімкнені лінії, через які проходить поверхня, як вибрано на момент відкриття діалогу. Закрийте діалог, щоб вибрати інші.
reference-surface-triangulates-selected-points-plan-in = Тріангулює вибрані точки в плані в нову поверхню. Кожна побудова додає поверхню.

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
state-build-reference-points = Побудувати опорні точки
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
state-colour-drillholes-working-section = Розфарбувати свердловини за робочим пластом
state-colour-points-classification = Розфарбувати точки за класифікацією
state-copy-objects-layer = Копіювати об'єкти в шар
state-count-cloud-s = Хмар: { $count }
state-count-file-s = Файлів: { $count }
state-count-object-s-axis-value = Об'єктів: { $count } · { $axis } { $value }
state-count-object-s-closed = Об'єктів: { $count } · { $closed }
state-count-object-s-layer = Об'єктів: { $count } · { $layer }
state-count-object-s-weight = Об'єктів: { $count } · { $weight }
state-count-object-s-z-elevation = Об'єктів: { $count } · Z { $elevation }
state-points-controls-clipped = Точок: { $count } · контрольних ліній: { $controls } · обрізано за лінією меж
state-points-controls-unclipped = Точок: { $count } · контрольних ліній: { $controls } · не обрізано
state-create-collection = Створити колекцію
state-create-point-cloud-tin = Створити TIN за хмарою точок
state-create-project = Створити проект
state-current-project = Поточний проект
state-cut-topology-pit-shell = Вирізати топологію за оболонкою кар'єру
state-cut-triangulation-polyline = Відсікти тріангуляцію полілінією
state-cut-triangulation-z = Відсікти тріангуляцію за Z
state-dark-mode = Темний режим
state-data-ticked-export-checklist = Дані, позначені в списку експорту
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
state-export-drillhole-csv = Експорт CSV свердловин
state-export-layer-dxf = Експорт шару в DXF
state-export-omf = Експорт OMF
state-export-project-dxf = Експорт проекту в DXF
state-export-triangulation = Експорт тріангуляції
state-export-viewport-image = Експорт зображення області перегляду
state-finish-closed-polyline = Завершити замкнену полілінію
state-finish-open-polyline = Завершити розімкнену полілінію
state-fit-extents = Вписати в межі
state-plan-view-then-fit-extents = Вигляд у плані з тієї ж відстані, потім вписати в межі
state-fix-release-centre-both-views = Закріплює або звільняє центр, навколо якого обертаються обидва види
state-folder-section = { $folder } у { $section }
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
state-member-into-folder-section = { $member } до { $folder } у { $section }
state-member-root-section = { $member } у корінь { $section }
state-move-axis-value = Перемістити на значення осі
state-move-objects-layer = Перемістити об'єкти в шар
state-name-count-cloud-s = { $name } · хмар: { $count }
state-name-count-holes = { $name } · свердловин: { $count }
state-name-count-object-s = { $name } · об'єктів: { $count }
state-name-z-min-z-max = { $name } · від { $z_min } до { $z_max }
state-new-collection-under-section = Нова колекція в розділі { $section }
state-next-edit = Наступна зміна
state-north = Північ
state-off = Вимк.
state-on = Увімк.
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
state-set-cinematic-view = Задати кінематографічний вигляд
state-set-drillhole-colour-preset = Задати кольорову схему свердловин
state-set-drillhole-discs = Задати диски свердловин
state-set-drillhole-style = Задати стиль свердловин
state-set-drillhole-width = Задати ширину свердловин
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
status-faces-chunks = Грані: -- / -- (фрагментів: --/--)
status-frame-rate = Частота кадрів
status-points-chunks = Точки: -- / -- із -- (фрагментів: --/--)

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
tri-cloud-carries-no-classifications-so = Ця хмара не має класифікації, тому поверхня будується за всіма точками. Імпортуйте файл LAS/LAZ, що пройшов фільтр ґрунту, щоб відновити відкриту землю.
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
tri-ground-points-only = Лише точки ґрунту
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
tri-no-boundary-selected = Межу не вибрано
tri-no-point-cloud-selected = Хмару точок не вибрано
tri-no-surface-selected = Поверхню не вибрано
tri-once-clip-succeeds-unload-source = Після успішного відсікання вивантажити вихідну поверхню, щоб у сцені залишився лише відсічений результат.
tri-once-cut-succeeds-unload-original = Після успішного вирізання вивантажити вихідну топологію, щоб у сцені залишився лише вирізаний результат. Оболонка кар'єру залишається завантаженою.
tri-once-merge-succeeds-unload-source = Після успішного об'єднання вивантажити вихідну топологію й тіло, щоб у сцені залишився лише об'єднаний результат.
tri-once-slice-succeeds-unload-source = Після успішного розсічення вивантажити вихідну поверхню, щоб у сцені залишився лише розсічений результат.
tri-once-trim-succeeds-unload-surface = Після успішного підрізання вивантажити підрізану поверхню, щоб у сцені залишився лише результат. Топологія залишається завантаженою.
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
tri-reconstruct-ground-only-help = Відновити за точками, класифікованими як відкрита земля, відкинувши рослинність, будівлі, обладнання та шум. Вимкніть, щоб будувати поверхню за кожною точкою хмари.
tri-reconstruct-help = Відновлює тріангульовану поверхню рельєфу з хмари точок. Адаптивна вибірка витрачає бюджет вершин там, де рельєф найскладніший, і зберігає плоскі ділянки розрідженими.
tri-reduce-budget-candidate-detail-if = Зменшіть бюджет або деталізацію кандидатів, якщо на комп'ютері мало оперативної пам'яті.
tri-reference-topology-help = Опорна топографічна поверхня, що визначає область обрізки іншої поверхні.
tri-reject-reconstructed-triangle-edges = Відкидає ребра відновлених трикутників, довші за цю відстань. Значення 0 вимикає обмеження довжини ребра.
tri-remove-inside-help = Видаляє поверхню всередині межі полілінії та залишає решту.
tri-removes-topology-where-pit-shell = Видаляє топографічну поверхню там, де оболонка кар'єру проходить нижче неї, щоб оболонка заповнила отвір. Шов слідує істинній тривимірній лінії контакту поверхонь; топографічна поверхня під частинами оболонки, що височіють над землею, зберігається.
tri-result = Результат
tri-save-two-entities = Зберегти як два об'єкти
tri-select = Вибрати…
tri-selected-closed-polyline-whose-xy = Вибрана замкнена полілінія, межа якої в XY визначає область відсікання.
tri-selected-point-cloud-whose-points = Вибрана хмара точок, з точок якої буде відновлено поверхню рельєфу. Закрийте діалог, щоб відновити іншу.
tri-selected-surface-from-which-contour = Вибрана поверхня, за якою буде створено горизонталі. Закрийте діалог, щоб побудувати горизонталі для іншої.
tri-selected-surface-which-will-clipped = Вибрана поверхня, яку буде відсічено. Закрийте діалог, щоб відсікти іншу.
tri-slice-source-help = Вибрана поверхня, діапазон позначок якої буде відсічено. Закрийте діалог, щоб розсікти іншу.
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
tri-unload-source-surface = Вивантажити вихідну поверхню
tri-unload-source-topology = Вивантажити вихідну топологію
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
viewport-1-1-true-shape = 1:1, справжня форма
viewport-1-ratio = 1:{ $ratio }

## Viewport strings

viewport-all-total-categories-keep-their = Усі категорії ({ $total }) зберігають свої кольори; чітко відображаються лише перші { $shown }
viewport-axis-maximum = Максимум { $axis }
viewport-axis-minimum = Мінімум { $axis }
viewport-azimuth-dip = Азимут { $azimuth }, кут падіння { $dip }
viewport-back-whole-log = Назад до всього каротажу.
viewport-bar-blast-timeline-placeholder = Хронологія вибуху [ЗАГЛУШКА]
viewport-bar-burden-relief-heatmap-placeholder = Теплова карта вивільнення ЛНО [ЗАГЛУШКА]
viewport-bar-cinematic-view = Кінематографічний вигляд
viewport-bar-color = Колір:
viewport-bar-contours-equal-time-placeholder = Горизонталі рівного часу [ЗАГЛУШКА]
viewport-bar-disable-cinematic-view = Вимкнути кінематографічний вигляд
viewport-bar-disable-flying-mode = Вимкнути режим польоту
viewport-bar-disable-x-ray-vision = Вимкнути рентгенівський режим
viewport-bar-drill-holes = Свердловини:
viewport-bar-enable-flying-mode = Увімкнути режим польоту
viewport-bar-enable-x-ray-vision = Увімкнути рентгенівський режим
viewport-bar-exit-slice-view = Вийти з режиму перерізу
viewport-bar-fill = Заливка:
viewport-bar-fix-centre-rotation = Закріпити центр обертання
viewport-bar-hide-borehole-inspector = Сховати інспектор свердловини
viewport-bar-hide-classification = Сховати класифікацію
viewport-bar-hide-points = Приховати точки
viewport-bar-hide-rl-grid = Сховати сітку позначок
viewport-bar-hide-wireframes = Приховати каркаси
viewport-bar-hide-xy-grid = Сховати сітку XY
viewport-bar-release-centre-rotation = Звільнити центр обертання
viewport-bar-reset-view-plan-over-centre = Скинути вигляд: план над центром обертання, клацніть ще раз, щоб вписати все
viewport-bar-reset-view-plan-same-distance = Скинути вигляд: план з тієї ж відстані, клацніть ще раз, щоб вписати все
viewport-bar-show-borehole-inspector = Показати інспектор свердловини
viewport-bar-show-classification = Показати класифікацію
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
viewport-depth-m-hole-end = { $depth } м, кінець свердловини
viewport-double-click-add-boundary-here = Двічі клацніть, щоб додати межу тут
viewport-drag-move-middle-click-toggles = Перетягніть для переміщення · Середня кнопка перемикає ≤
viewport-drag-move-right-click-remove = Перетягніть для переміщення · Клацніть правою кнопкою для видалення · Середня кнопка перемикає ≤
viewport-drag-spin-view-around-hole = Перетягніть, щоб обертати вигляд навколо свердловини. Подвійне клацання — орієнтація на північ.
viewport-e = С
viewport-edit-category-colour = Змінити колір цієї категорії
viewport-edit-colour-used-empty-values = Змінити колір порожніх значень
viewport-empty = (порожньо)
viewport-empty-hidden = (порожньо · приховано)
viewport-filter-variables = Фільтр змінних
viewport-fit-hole-track = Вписати свердловину в трек
viewport-from = від { $from } до { $to }
viewport-from-m = від { $from } до { $to } м
viewport-h-1-ratio = Г 1:{ $ratio }
viewport-hole-has-no-trace-draw = У цієї свердловини немає траєкторії для малювання.
viewport-interval-data = Дані інтервалів
viewport-intervals = Інтервали
viewport-m-from-collar-toward-bearing = м від устя, у напрямку { $bearing }°
viewport-navigation-hint = Середня кнопка + перетягування — панорама · Колесо — масштаб
viewport-navigation-hint-detach = Середня кнопка + перетягування — панорама · Колесо — масштаб · Клацання — від'єднати
viewport-n = Пн
viewport-no-data-variable = Немає даних для цієї змінної
viewport-no-density-log-hole = Для цієї свердловини немає каротажу густини
viewport-no-downhole-geophysics-hole = Для цієї свердловини немає свердловинної геофізики
viewport-no-gamma-log-hole = Для цієї свердловини немає гамма-каротажу
viewport-no-matches = Немає збігів
viewport-no-trace = Немає траєкторії
viewport-no-usable-range = (немає придатного діапазону)
viewport-not-logged = Не каротовано
viewport-orientation-source = Джерело орієнтації
viewport-rebuild-variable-s-colours-from = Перебудувати кольори цієї змінної за її даними
viewport-reset = Скинути
viewport-restore-full-model-range = Відновити повний діапазон моделі
viewport-roll-wheel-over-log-zoom = Прокрутіть колесо над каротажем, щоб наблизити пласт. Перетягніть каротаж, щоб обертати свердловину та рухатися вздовж неї.
viewport-s = Пд
viewport-sideways-scale = Бічний масштаб
viewport-squeeze-sideways-just-enough-keep = Стискає вбік рівно настільки, щоб свердловина залишалася у видимості. Ніколи не розтягує.
viewport-trace-extent = Протяжність траєкторії
viewport-w = Зх
viewport-widen-panel-show-density = Розширте панель, щоб показати густину
viewport-widen-panel-show-density-gamma = Розширте панель, щоб показати густину та гамма
viewport-widen-panel-show-gamma = Розширте панель, щоб показати гамма
charging-edit-charge-product = Редагувати засіб заряджання
charging-new-charge-product = Новий засіб заряджання
charging-explosive-decks-add-mass-primed-stemming = Вибухові колонки додають масу й мають боєвик; колонки забивки та повітряні мають лише довжину.
charging-density = Густина
charging-density-hint = Густина в свердловині. Маса на метр дорівнює цьому значенню, помноженому на площу поперечного перерізу свердловини.
charging-another-product-already-has-name = Інший засіб уже має таку назву
charging-edit-charge-rule = Редагувати правило заряджання
charging-new-charge-rule = Нове правило заряджання
charging-decks-collar-toe = Колонки, від устя до підошви
charging-priming = Боєвики
charging-preview = Попередній перегляд
charging-preview-use-pattern-hole = Використати медіанну свердловину сітки
charging-preview-active-pattern-median-hole = Попередній перегляд на медіанній свердловині активної сітки
charging-fixed-decks-longer-than-hole = Фіксовані колонки довші за цю свердловину
charging-mass-kg-explosive = ВР: { $mass } кг
charging-rate-kg-m = { $rate } кг/м
charging-count-primer = Боєвиків: { $count }
charging-another-rule-already-has-name = Інше правило вже має таку назву
charging-save-reload-count-hole = Зберегти й перезавантажити свердловини: { $count }
charging-length = Довжина
charging-rest-length-m = решта · { $length } м
charging-rest = решта
charging-deck-takes-whatever-length-fixed-decks = Ця колонка займає всю довжину, що залишається після фіксованих колонок. Лише одна колонка на правило заповнює решту.
charging-remove-deck = Видалити колонку
charging-add-deck = Додати колонку
charging-downhole-delay = Затримка в свердловині
charging-hole-detonator-hole-fires-long-after = Внутрішньосвердловинний детонатор. Свердловина спрацьовує через цей час після надходження поверхневого сигналу.
charging-primer-height = Висота боєвика
charging-how-far-above-base-each-explosive = На скільки вище від основи кожної вибухової колонки розташований її боєвик.
charging-booster = Шашка-детонатор
charging-cast-booster-mass-each-primer = Маса литої шашки-детонатора в кожному боєвику.
charging-count-rule-load-product-will-need = Правил, що заряджають цим засобом, і для яких потрібно вибрати інший: { $count }.
charging-rule = Правило
charging-holes-already-loaded-keep-their-charge = Свердловини, уже заряджені ним, зберігають свій заряд.
blast-burden-relief = Вивільнення ЛНО
blast-ms-per-metre-last-neighbour-fire = мс на метр до останнього сусіда, що спрацьовує
blast-below-hole-fires-before-rock-front = Нижче цього значення свердловина спрацьовує, поки порода перед нею не зрушила: тісно.
blast-above-rock-front-has-long-gone = Вище цього значення порода перед нею давно зникла: вільно, з ризиком відсікання та розльоту каміння.
blast-tight = тісно
blast-good = добре
blast-slack = вільно
blast-free-face = вільна поверхня
blast-fires-at = Спрацьовує о
blast-empty-won-t-detonate = порожня, не спрацює
blast-not-reached = не досягнуто
blast-value-ms-m-from-hole = { $value } мс/м від { $hole }
blast-fires-first-free-face = спрацьовує першою: вільна поверхня
blast-relief = Вивільнення
blast-explosive = Вибухова речовина
blast-powder-factor = Питома витрата ВР
blast-not-loaded = Не заряджено
blast-count-primer-delay-ms-downhole = Боєвиків: { $count } · затримка в свердловині { $delay } мс
blast-set-initiation-point-tie-holes-play = Задайте точку ініціювання та з'єднайте свердловини, щоб відтворити вибух
blast-pause = Пауза
blast-play = Відтворити
blast-back-start = Назад на початок
blast-duration-ms = із { $duration } мс
blast-real-time = Реальний час
blast-mic-limit = Ліміт MIC
blast-most-explosive-allowed-detonate-any-8 = Найбільша маса ВР, дозволена до детонації за будь-які 8 мс на цьому майданчику. Вікна понад ліміт позначаються.
blast-no-holes-loaded-surface-signal-plays = Свердловини не заряджені: поверхневий сигнал відтворюється, але нічого не детонує. Зарядіть свердловини інструментом «Зарядити свердловини».
blast-now-holes-hole = Зараз: свердловин — { $holes }
blast-in-8-ms = за 8 мс
blast-peak-mass-kg-time-ms = Пік { $mass } кг на { $time } мс
blast-peak-holes-hole-time-ms = Пік: свердловин — { $holes }, на { $time } мс
blast-peak-over-limit = , перевищення на { $over } кг
blast-peak-within-limit = , у межах ліміту
blast-top-surface-signal-lighting-each-downline = Угорі: поверхневий сигнал, що запалює кожну ударну трубку. Унизу: детонації. Клацніть або перетягніть, щоб перемістити позначку відтворення.
products-charge-rules = Правила заряджання
products-new-rule = Нове правило
products-charge-products = Засоби заряджання
products-new-rule-default-name = Нове правило
products-no-rules = Немає правил
products-load-selected-holes-count = Зарядити вибрані свердловини ({ $count })
products-unload-selected-holes-count = Розрядити вибрані свердловини ({ $count })
products-edit-rule = Редагувати правило
products-duplicate-rule = Дублювати правило
products-delete-rule = Видалити правило
products-fill-product = заповнення  { $product }
products-primer-offset-m-off-each-explosive = Боєвик на { $offset } м від основи кожної вибухової колонки, шашка-детонатор { $booster } кг, затримка в свердловині { $delay } мс
products-double-click-edit = Подвійне клацання — редагувати
products-edit-product = Редагувати засіб
blast-log-updated-charge-product-name = Оновлено засіб заряджання { $name }
blast-log-added-charge-product-name = Додано засіб заряджання { $name }
blast-log-updated-charge-rule-name = Оновлено правило заряджання { $name }
blast-log-added-charge-rule-name = Додано правило заряджання { $name }
blast-log-entry-no-longer-charge-library = Цього запису більше немає в бібліотеці заряджання
blast-log-deleted-name-from-charge-library = { $name } видалено з бібліотеки заряджання
blast-log-failed-save-charge-library-error = Не вдалося зберегти бібліотеку заряджання: { $error }
blast-log-cannot-load-rule-problem = Заряджання за цим правилом неможливе: { $problem }
blast-log-count-hole-too-short-fixed-decks = Свердловин, надто коротких для фіксованих колонок цього правила, залишено без змін: { $count }
blast-log-count-hole-have-no-depth-load = Свердловин без глибини для заряджання: { $count }
blast-log-count-loaded-hole-have-no-diameter = Заряджених свердловин без діаметра, тому маса їхньої ВР невідома: { $count }
common-charge-holes = Зарядити свердловини
blast-log-loaded-count-hole-rule = Заряджено свердловин: { $count }, правило: { $rule }
blast-log-unload-holes = Розрядити свердловини
blast-log-unloaded-count-hole = Розряджено свердловин: { $count }
blast-log-select-holes-active-pattern-first = Спершу виберіть свердловини активної сітки
blast-log-rule-no-longer-charge-library = Цього правила більше немає в бібліотеці заряджання
blast-log-there-no-charge-rule-load-add = Немає правила заряджання: додайте його на панелі засобів
blast-rule-stemming = Забивка
blast-rule-air-deck = Повітряний проміжок
blast-rule-give-rule-name = Дайте правилу назву
blast-rule-add-least-one-deck = Додайте принаймні одну колонку
blast-rule-only-one-deck-can-fill-rest = Лише одна колонка може заповнювати решту свердловини
blast-rule-deck-lengths-must-greater-than-zero = Довжини колонок мають бути більшими за нуль
blast-rule-no-product-named-name = Немає засобу з назвою «{ $name }»
blast-rule-rule-needs-least-one-explosive-deck = Правило потребує принаймні однієї вибухової колонки
state-save-charge-product = Зберегти засіб заряджання
state-save-charge-rule = Зберегти правило заряджання
state-delete-charge-library-entry = Видалити запис бібліотеки заряджання
ui-click-drag-over-holes-load-them = Клацніть або перетягніть над свердловинами, щоб зарядити їх за правилом { $rule }
ui-hold-shift-unload = утримуйте Shift, щоб розрядити
ui-no-charge-rule-load = Немає правила заряджання
ui-right-click-charge-rules-heading-add = клацніть правою кнопкою заголовок «Правила заряджання», щоб додати
omf-element-name-has-count-charge-naming = Елемент «{ $name }» має зарядів ({ $count }), що посилаються на свердловини, яких він більше не містить
