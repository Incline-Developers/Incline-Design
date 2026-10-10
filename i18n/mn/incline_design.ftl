# Incline — Монгол хэл дээрх мессежийн каталог.
#
# Энэ файл дутуу байж болно: дутуу мессежүүд англи хэлнээс авагдана
# (`i18n/en/incline_design.ftl`). `=` тэмдгийн зүүн талд байгаа ID-ууд
# болон `{ $... }` хэлбэрийн аргументын нэрсийг өөрчлөхийг хориглоно —
# зөвхөн тэмдгийн баруун талын текстийг орчуулна.

## Ерөнхий

common-cancel = Цуцлах
common-clear = Цэвэрлэх
common-close = Хаах
common-fill = Дүүргэлт
common-set = Тохируулах

## Төлөв байдлын мөр

# Төлөв байдлын мөрөн дэх хэлний цэсний гарчиг. Хэл бүрийг хэзээ ч
# орчуулдаггүй: тус бүр өөрийн бичгээр өөрийгөө нэрлэдэг (LanguageChoice-с).
status-language = Хэл

## Цэс — Файл

menu-file = Файл
menu-file-save-project = Төслийг хадгалах
menu-file-save-project-as = Төслийг өөр нэрээр хадгалах...
menu-file-new-project = Шинэ төсөл...
menu-file-open-project = Төсөл нээх...
menu-file-open-recent = Сүүлд нээсэн
menu-file-show-in-explorer = Explorer дээр харуулах
menu-file-show-in-folder = Агуулсан хавтсыг нээх
menu-file-import = Импортлох...
menu-file-export = Экспортлох...
menu-file-export-viewport-image = Харагдах цонхны зургийг экспортлох...
menu-file-export-engineering-drawing = Инженерийн зургийг экспортлох...
menu-file-about = { $app }-ийн тухай...
menu-file-exit = Программаас гарах

## Цэс — Харагдац

menu-view = Харагдац

## Ажлын орчинууд

ws-production = Үйлдвэрлэл
ws-drill-and-blast = Өрөмдлөг ба тэсэлгээ
ws-geology = Геологи
ws-planning = Төлөвлөлт

## Цэсний мөрүүд

ws-menubar-design = Зураг төсөл
ws-menubar-triangulation = Триангуляц
ws-menubar-raster = Растер
ws-menubar-point-cloud = Цэгэн үүл
ws-menubar-block-model = Блокийн загвар
ws-menubar-drillholes = Цооногууд
ws-menubar-modelling = Загварчлал
ws-menubar-modelling-select-holes = Эхлээд цооногуудыг сонгоно уу
ws-menubar-modelling-select-points = Дор хаяж { $count } цэг сонгоно уу
ws-menubar-modelling-select-surface = Нэг торон гадаргуу сонгоно уу
ws-menubar-modelling-select-surfaces = Давхаргын дээврийн хэсэг ба улыг, хоёр торон гадаргууг сонгоно уу
ws-menubar-active-layer = Давхарга:

## Цэсний мөрийн функцууд

ws-menubar-design-insert-point = Цэг оруулах
ws-menubar-design-insert-point-at-intersection = Огтлолцол дээр
ws-menubar-geology-design = Геологийн зураг төсөл
ws-menubar-geology-draw = Зурах
ws-menubar-geology-drape-along-triangles = Гурвалжнуудыг даган дараах
ws-menubar-geology-edit = Засах
ws-menubar-geology-insert-at-elevation = Өндөрлөгт цэг оруулах...
ws-menubar-geology-join-split = Нэгтгэх ба хуваах
ws-menubar-geology-surface = Гадаргуу
ws-menubar-geology-vertices = Оройнууд
ws-menubar-production-design = Олборлолтын зураг төсөл
ws-menubar-design-insert-point-at-elevation = Өндөрлөг дээр
ws-menubar-design-move-to = Шилжүүлэх
ws-menubar-design-create-triangulation = Триангуляц үүсгэх

## Нэр өөрчлөх / устгах диалогууд

# { $kind } нь дээрх ws-production-* багцын ажлын орчны нэр үг.
dialog-rename-title = { $kind }-ийг нэр өөрчлөх
dialog-rename-field = Шинэ нэр
dialog-rename-field-hint = Заавал бөглөнө
dialog-rename-submit = Нэр өөрчлөх
dialog-delete-title = { $kind }-ийг устгах
dialog-delete-confirm =
    «{ $name }»-ийг төслөөс устгах уу?
    Энэ үйлдлийг буцаах боломжгүй.
confirm-delete-product =
    «{ $name }» бүтээгдэхүүнийг палитраас устгах уу?
    Энэ үйлдлийг буцаах боломжгүй.

## «Триангуляц үүсгэх» диалог

tri-create-title = Триангуляц үүсгэх
tri-create-help = Энэ цонх нээгдэх үед сонгогдсон объектуудыг триангуляц хийнэ. Сонголтыг өөрчлөхийн тулд цонхыг хаана уу.
tri-create-type-label = Триангуляцын төрөл
tri-create-type-help =
    Задгай гадаргуу нь рельефийн маягийн хавтгай үүсгэнэ. Хатуу бие нь бүрэн
    хаалттай тор үүсгэх бөгөөд усны нэвтэрхий бус хилийг бүрдүүлж чадах
    оролтын өгөгдөл шаардана.
tri-create-output-name = Гаралтын нэр
tri-create-output-name-help = Үүсгэсэн триангуляцад оноох нэр.
tri-create-output-name-hint = триангуляцын нэр
tri-create-run = Триангуляц хийх
tri-selection-none = Сонгосон объектууд цаашид боломжгүй байна.

tri-selection-selected = { $summary } сонгогдсон

tri-type-open-surface = Гадаргуу
tri-type-solid-closed = Хатуу бие

# Сонголтын тоймын хэсгүүд, ж: "3 полилиниа, 1 цэг". Нэр үг бүр өөрийн
# тоогоор олонлогжсон тул хоёроос олон олонлогийн хэлбэртэй хэлүүдэд ч
# зөв харагдана.
tri-count-polylines =
    { $count ->
        [one] { $count } полилиниа
       *[other] { $count } полилиниа
    }
tri-count-strings =
    { $count ->
        [one] { $count } шугаман өгөгдөл
       *[other] { $count } шугаман өгөгдөл
    }
tri-count-circles =
    { $count ->
        [one] { $count } тойрог
       *[other] { $count } тойрог
    }
tri-count-points =
    { $count ->
        [one] { $count } цэг
       *[other] { $count } цэг
    }
tri-count-texts =
    { $count ->
        [one] { $count } текст объект
       *[other] { $count } текст объект
    }
tri-count-objects =
    { $count ->
        [one] { $count } объект
       *[other] { $count } объект
    }

about-read-full-licence = Бүрэн лицензийг унших ↗
about-source-code = Эх код
about-website = Вэбсайт
about-title = { $app }-ийн тухай
drill-hole-colour-stop = Зогсоол { $index }
properties-restore-defaults-tooltip = { $heading } тохиргоог үндсэн утга руу нь буцаах

## Динамик интерфейсийн мессежүүд

ui-selected-count = { $count } сонгогдсон
ui-selected-objects = { $count } объект сонгогдсон
ui-selected-polylines = { $count } полилиниа сонгогдсон
ui-invalid-axis-value = { $axis }-д зөв утга оруулна уу.
ui-selection-spans = Сонголт { $min }-с { $max } хүртэл үргэлжилнэ.
confirm-delete-count = Сонгосон { $count } зүйлийг устгахдаа итгэлтэй байна уу?
confirm-delete-layer = «{ $name }» давхаргыг түүн дээрх бүх объектын хамт устгах уу?
    Энэ үйлдлийг буцаах боломжгүй.
plot-preview-pixels = { $width } × { $height } px, { $dpi } dpi
tri-estimated-memory = Тооцоолсон дээд санах ой ~{ $estimate }. { $detail }
block-grid-summary = Тор: { $x } × { $y } × { $z } = { $count } блок
status-selected = Сонгогдсон: { $count }
status-faces = Талууд: { $drawn } / { $total } ({ $drawn_chunks }/{ $total_chunks } хэсэг)
status-clip = Огтлолын ойр/хол/Δ: { $near } / { $far } / { $delta } м
status-points = Цэгүүд: { $drawn } / { $target } ({ $total }-с) ({ $drawn_chunks }/{ $total_chunks } хэсэг)

## Их давтамжтай эх кодын шууд мөрүүд

## Эх кодын шууд мөрүүд

## Нэмэлт эх кодын шууд мөрүүд

explorer-no-rasters = Растер алга
slice-viewport-gestures = дунд товч чирж зөөх · баруун товч чирж тойрох · Shift+хулганы дугуй алхах · W/S давхарга зөөх · Q/E эргүүлэх · Esc гарах

## Эхлэлийн орчны дэлгэрэнгүй

## Хамрах хүрээний шалгалтаар илэрсэн эх кодын шууд мөрүүд

## Дүрслэлийн эхлэлийн онош

## Өрөмдлөг ба тэсэлгээ болон бусад үлдсэн каталогийн бичлэгүүд

color-aci = ACI
color-aci-value = ACI { $index }
color-index = Индекс
color-rgb = RGB
color-opacity = Тунгалагжилт
color-edit = Товшиж өнгийг засах
color-saturation-value = Ханалт ба гэрэлтэлт
color-hue = Өнгөлөг
asset-loading = Хөрөнгийн өгөгдлийг ачаалж байна
asset-unloading = Хөрөнгийн өгөгдлийг буулгаж байна
asset-load-failed = Хөрөнгийн өгөгдлийг ачаалж чадсангүй
asset-unload-failed = Хөрөнгийн өгөгдлийг буулгаж чадсангүй
preferences-title = Тохиргоо
context-text-colour = Текстийн өнгө
context-polylines = Полилиниа
context-points = Цэгүүд
crs-unknown-ellipsoid = Энэ координатын системийн тодорхойлолт дахь "{ $name }" дэлхийн загварыг таньсангүй.
crs-no-ellipsoid = Энэ координатын системийн тодорхойлолт ямар дэлхийн загвар ашиглаж байгааг заагаагүй байна.
crs-unknown-code = EPSG:{ $code } нь координатын системийн бүртгэлд алга.
crs-transform-failed = Координатыг хувиргаж чадсангүй; үр дүн төгсгөлөг байрлал болсонгүй.
crs-no-datum-path = { $from } болон { $to }-ийн лавлагаа системүүдийн (EPSG датум { $source } ба { $target }) хооронд нийтлэгдсэн хувиргалт олдсонгүй. Ямар ч байсан хувиргах нь тодорхойгүй хэмжээгээр буруу байх тул юу ч өөрчлөгдсөнгүй.
crs-unknown-datum = { $from } эсвэл { $to }-ийн лавлагаа системийг тодорхойлж чадахгүй байна, мөн энэ хоёр өөр дэлхийн загвар ашигладаг тул тэдгээрийн хооронд хувиргах нь тодорхойгүй хэмжээгээр буруу байх болно.
ws-survey = Геодези
survey-count-designs = { $count } { $count ->
    [one] зураг төсөл
   *[other] зураг төсөл
  }
survey-count-meshes = { $count } { $count ->
    [one] триангуляц
   *[other] триангуляц
  }
survey-count-models = { $count } { $count ->
    [one] блокийн загвар
   *[other] блокийн загвар
  }
survey-count-clouds = { $count } { $count ->
    [one] цэгэн үүл
   *[other] цэгэн үүл
  }
survey-count-holes = { $count } { $count ->
    [one] цооногийн өгөгдлийн сан
   *[other] цооногийн өгөгдлийн сан
  }
survey-count-rasters = { $count } { $count ->
    [one] растер
   *[other] растер
  }
survey-angle = Z тэнхлэгийг тойрсон эргэлт (цагийн зүүний эсрэг)
survey-scale = Нэгдсэн XYZ масштабын коэффициент
survey-invalid-transform = Эх цэг, өнцөг болон гарсан координатууд төгсгөлөг байх ёстой.
survey-invalid-scale = Масштаб нь төгсгөлөг эерэг тоо байх ёстой бөгөөд түүний урвуу тоо ч төгсгөлөг байх ёстой.
survey-empty-selection = Хувиргах дор хаяж нэг дэмжигдсэн зүйлийг сонгоно уу.
survey-unavailable = Сонгосон зүйл алга байна эсвэл ачаалагдаагүй байна. Хувиргахаас өмнө ачаална уу.
survey-wrong-project = Зөвхөн идэвхтэй төслөөс зураг төсөл сонгоно уу.
survey-name-required = Координатын системийн нэрийг оруулна уу.
survey-working = Сонгосон өгөгдлийг хувиргаж байна…
survey-completed = { $items }-г байгаа газарт нь хувиргалаа. Буцаах нь тэдгээрийг сэргээнэ.
survey-failed = Хувиргалт амжилтгүй боллоо: { $error }
survey-stale = Идэвхтэй төсөл эсвэл эх өгөгдөл өөрчлөгдсөн тул хувиргалтыг хаялаа. Эх өгөгдлийг сонгоод дахин оролдоно уу.
survey-coordinates-menu = Координат
survey-definitions-action = Тодорхойлолтууд…
survey-transform-action = Хувиргах…
survey-definitions-title = Координатын тодорхойлолтууд
survey-transform-title = Координат хувиргах
survey-new-system = Шинэ координатын систем
survey-new-system-name = Координатын систем
survey-set-local = Уурхайн координатын систем болгож тохируулах
survey-delete-system = Координатын системийг устгах
survey-systems-empty = Координатын систем алга
survey-system-name = Нэр
survey-system-origin = Ижил цэг — системийн координат
survey-angle-help = Дээрээс харахад лавлагаа X тэнхлэгээс лавлагаа Y тэнхлэг рүү цагийн зүүний эсрэг чиглэлээр.
survey-scale-help = Лавлагаа системээс энэ систем рүү шилжих нэгдсэн XYZ масштаб. Хэмжээг хадгалахын тулд 1 ашиглана уу.
survey-close = Хаах
survey-from = Эх
survey-to = Хүрэх
survey-transform-button = Хувиргах
survey-swap = Сэлгэх
survey-drape-note = Дараасан зураг хувиргасан гадаргуугаас хасагдах бөгөөд дахин дараах шаардлагатай.
survey-needs-grid-block-model = Блокийн загвар бол эсийн тогтмол тор бөгөөд проекц эсвэл лавлагаа системийн өөрчлөлт энэ тогтмол байдлыг хадгалдаггүй. Үүнийг хувиргах гэдэг нь эс бүрийг шинэ тор руу дахин дээж авч, агуулж буй утгуудаа алдана гэсэн үг тул өөрчлөлгүй орхилоо.
survey-needs-grid-raster = Растер нь дэлхий дээр аффин зураглалаар байрладаг бөгөөд үүнийг проекц эсвэл лавлагаа системийн өөрчлөлт хадгалж чадахгүй. Үүнийг хувиргах гэдэг нь зургийг дахин дээж авна гэсэн үг тул өөрчлөлгүй орхилоо.
survey-conversion-exact = Яг таг: зөвхөн торны өөрчлөлт, дахин проекцлолгүй.
survey-conversion-accuracy = Заасан нарийвчлал { $accuracy } м.
survey-kind = Төрөл
survey-axis-names = Тэнхлэгийн нэрс
survey-kind-registry-short = Бүртгэлийн систем
survey-kind-grid-short = Өөр систем дээрх тор
survey-registry-search = Хайх
survey-registry-hint = Нэр эсвэл EPSG код, ж: "mga zone 56"
survey-registry-none = Бүх үгтэй тохирох зүйл бүртгэлд алга.
survey-parent = Тодорхойлогдсон эсрэг
survey-parent-origin = Мэдэгдэж буй цэг — эцэг системийн координат
survey-pick-registry = Системийг хайж, үр дүнгээс сонгоно уу.
survey-pick-parent = Энэ торыг ямар системийн эсрэг тодорхойлохыг сонгоно уу.
survey-pick-system = Систем сонгоно уу
survey-pick-systems = Хувиргах эх системийг болон хүрэх системийг сонгоно уу.
survey-no-selection = Зүүн талаас координатын систем сонгох эсвэл нэмэхийн тулд доорх хоосон орон зайг хулганы баруун товчоор товшино уу.
survey-kind-grid = { $parent } дээрх тор
survey-system-in-use = "{ $name }"-г устгах боломжгүй: үүний эсрэг { $dependants } { $dependants ->
    [one] систем тодорхойлогдсон байна
   *[other] систем тодорхойлогдсон байна
  }. Эхлээд тэдгээрийг өөр газар руу чиглүүлнэ үү.
survey-system-cycle = "{ $name }" нь өөрийнхөө эсрэг, шууд эсвэл эцэг системүүдээрээ дамжуулан тодорхойлогдсон байна.
survey-system-missing = Тэр координатын систем цаашид байхгүй байна. Өөр тодорхойлолт сонгоно уу.
survey-same-system = Өөр өөр эх ба хүрэх систем сонгоно уу.
survey-name-exists = Ийм нэртэй координатын систем аль хэдийн байна. Засварлахын тулд түүнийг сонгох эсвэл өөр нэр сонгоно уу.
preferences-ui-size = Интерфейсийн хэмжээ
preferences-ui-size-help = Текст болон удирдлагын элементүүдийг төхөөрөмжийн хэвийн дэлгэцийн масштабтай харьцуулан тохируулна. 100% нь үндсэн хэмжээ юм. Дэлгэцийн нягтрал болон цонхны хэмжээ интерфейсийг жижигрүүлэхгүй.
relimit-select-boundary = Хязгаарлах полилиниа эсвэл тойргийг сонгоно уу
relimit-click-boundary = Огтлолцуулах полилиниа эсвэл тойрог дээр товшино уу…
relimit-mode-help = Огтлолцол горим нэг үзүүрийг полилиниа эсвэл тойрог руу шилжүүлнэ. Үнэмлэхүй горим шугамын эцсийн уртыг тогтооно. Харьцангуй горим уртыг нэмж эсвэл хасна.
browser-graphics-device-lost = Хөтөч график төхөөрөмжөө алдлаа. Энэ хуудсыг шинэ табд дахин нээнэ үү. GPU-н дэлгэрэнгүй: { $message }

## About strings

about-copyright-c-2026-leo-timmins =
    Copyright (c) 2026 Leo Timmins, Lucas Timmins, and Incline Design contributors. Permission is hereby granted, free of charge, to any person obtaining a copy of this software to deal in it without restriction, subject to the conditions of the MIT License.

    Incline Design is provided "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, including but not limited to the warranties of MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE and NONINFRINGEMENT.
about-free-open-source-mine-design = Уул уурхайн үнэгүй нээлттэй эх зураг төсөл
about-licensed-under-mit-license = MIT лицензийн дагуу лицензлэгдсэн

## App strings

app-activated-browser-project-name = '{ $name }' хөтчийн төслийг идэвхжүүллээ.
app-browser-project-delete-failed = Хөтчийн төслийг устгах амжилтгүй боллоо: { $error }
app-browser-project-no-longer-exists = Тэр хөтчийн төсөл цаашид байхгүй болжээ
app-browser-save-failed-error = Хөтчийн хадгалалт амжилтгүй боллоо: { $error }
app-could-not-activate-browser-project = Хөтчийн төслийг идэвхжүүлж чадсангүй: { $error }
app-could-not-delete-browser-project = Хөтчийн төслийг устгаж чадсангүй: { $error }
app-could-not-load-browser-project = Хөтчийн төслийг ачаалж чадсангүй: { $error }
app-could-not-restore-browser-project = Хөтчийн төслийг сэргээж чадсангүй: { $error }
app-deleted-browser-project = Хөтчийн төслийг устгалаа
app-failed-create-window-error = Цонхыг үүсгэж чадсангүй: { $error }
app-failed-create-window-icon-error = Цонхны дүрсийг үүсгэж чадсангүй: { $error }
app-failed-detach-top-down-preview = Дээрээс харах урьдчилан харагдацыг салгаж чадсангүй: { $error }
app-failed-initialize-graphics-error = Дүрслэлийг эхлүүлж чадсангүй: { $error }
app-browser-preferences-load-failed = Хөтчийн тохиргоог ачаалж чадсангүй: { $error }
app-failed-load-config-file-error = Тохиргооны файлыг ачаалж чадсангүй: { $error }
app-failed-load-session-file-error = Сешний файлыг ачаалж чадсангүй: { $error }
app-failed-rasterize-window-icon-error = Цонхны дүрсийг растержуулж чадсангүй: { $error }
app-failed-save-browser-session-error = Хөтчийн сешнийг хадгалж чадсангүй: { $error }
app-failed-save-session-error = Сешнийг хадгалж чадсангүй: { $error }
app-saved-name-browser-storage = '{ $name }'-г хөтчийн санах ойд хадгаллаа

## Block strings

block-model-between = Хооронд
block-model-block-grid = Блокийн тор
block-model-block-size = Блокийн хэмжээ
block-model-choose-numeric-variable = Тоон хувьсагч сонгох
block-model-choose-numeric-variables = Тоон хувьсагч сонгох
block-model-count-variables-selected = { $count } хувьсагч сонгогдсон
block-model-estimate-variables = Тооцоолох хувьсагчид
block-model-full-x-y-z-dimensions = Блок бүрийн бүрэн X, Y, Z хэмжээс. Жижиг блок нарийвчлал, тооцооллын хугацаа, санах ойн хэрэглээг нэмэгдүүлнэ.
block-model-grid-bounds-block-sizes-invalid = Торны хил хязгаар эсвэл блокийн хэмжээ буруу байна.
block-model-lower-x-y-z-edges = Блокийн загварын эзэлхүүний доод X, Y, Z ирмэг. Блокийн төв энэ хязгаараас нэг блокийн хагасын зайд эхэлнэ.
block-model-maximum = Дээд
block-model-maximum-nearest-samples-used-each = Блок бүрт ашиглах хамгийн ойрын дээд түүврийн тоо. Бага утга хурдан ажиллана; өндөр утга тооцооллыг гөлгөрүүлж, тооцооллын хугацааг нэмэгдүүлж болно.
block-model-maximum-samples = Дээд түүврийн тоо
block-model-minimum = Доод
block-model-min-samples-help = Блок тооцоолоход шаардагдах ойролцоох доод түүврийн тоо. Хайлтын радиус дотор энэ тооноос цөөн түүвэртэй блок хоосон үлдэнэ.
block-model-minimum-samples = Доод түүврийн тоо
block-model-no-block-model-selected = Блокийн загвар сонгоогүй байна
block-model-no-drill-holes-selected = Цооног сонгоогүй байна
block-model-nugget = Наггет
block-model-numeric-interval-fields-interpolate = Интерполяцлах тоон интервалын талбарууд. Сонгосон талбар бүр блокийн загварын нэг хувьсагч болно.
block-model-kriging-help = Энгийн Кригинг бөмбөрцөг вариограм ашиглан блок бүрийн төвд цооногийн тоон интервалыг тооцоолно.
block-model-partial-sill = Хэсэгчилсэн доод хязгаар
block-model-range-search-radius = Мужийн / хайлтын радиус
block-model-range-help = Энэ зайнаас хол байгаа түүврийг хасна; коварианс энэ мужид тэгтэй тэнцэнэ.
block-model-select-all = Бүгдийг сонгох
block-model-selected-block-model-whose-blocks = Блокуудыг нь босгоор хатуу бие болгох сонгосон блокийн загвар. Өөрийг нь босгохын тулд цонхыг хаана уу.
block-model-source-drill-holes-help = Тоон интервалуудыг нь блок руу тооцоолох сонгосон цооногийн цуглуулга. Өөрөөс нь тооцоолохын тулд цонхыг хаана уу.
block-model-sill-help = Бөмбөрцөг загвараас гарах орон зайн хамааралтай дисперс. Наггеттэй хамт энэ нь тэг зайд коварианс тогтооно.
block-model-spherical-variogram-search = Бөмбөрцөг вариограм ба хайлт
block-model-threshold-at-most = <= босго
block-model-threshold-at-least = >= босго
block-model-threshold-min = Босго / доод
block-model-upper-x-y-z-extent = Хамрах дээд X, Y, Z хэмжээ. Хамрах муж блокийн хэмжээнд яг тэнцүү үрждэггүй бол сүүлчийн блок энэ хэмжээнээс давж болно.
block-model-variable = Хувьсагч
block-model-variance-effectively-zero-separation = Хэмжилтийн алдаа эсвэл түүврийн масштабаас доогуур хэлбэлзлээс үүдэлтэй, бодит байдал дээр тэг зайд гарах дисперс. Наггет эффект хэрэггүй бол тэгийг ашиглана уу.
block-model-volume-feedback-disconnected = Блокийн эзлэхүүний ашиглалтын мэдээллийн уншилт холболтоо тасалдлаа
block-model-volume-feedback-failed = Блокийн эзлэхүүний ашиглалтын мэдээллийн уншилт амжилтгүй боллоо: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z
borehole-inspector-add-every-code = Жагсаагдаагүй бүх кодыг нэмэх
borehole-inspector-add-to-column = Нэмэх
borehole-inspector-check = Тулгах
borehole-inspector-check-accept = Зөвшөөрөх
borehole-inspector-check-column = Тулгах
borehole-inspector-check-column-changed = Сүүлийн тулгалтаас хойш багана өөрчлөгдсөн. Аль цооног баганатай зөрж байгааг харахын тулд дахин тулгана уу.
borehole-inspector-check-column-hint = Ихэнх цооногийн эдгээр кодод өгсөн дарааллыг тодорхойлж, түүнтэй зөрсөн цооногуудыг жагсаана. Хоосон баганыг бөглөнө; өөр дараалалтай баганыг зөвхөн таныг зөвшөөрөхөд өөрчилнө.
borehole-inspector-check-differences-note = Ихэнх цооногийн өгсөн дараалал баганын хажууд харагдана. Зөвшөөрвөл багана энэ дараалалтай болно, буцаахад нэг алхам.
borehole-inspector-check-flagged = Тулгалт: { $count } цооног тэмдэглэгдсэн
borehole-inspector-check-moved = шилжсэн
borehole-inspector-check-not-run = Одоогоор тулгаагүй байна.
borehole-inspector-check-now = Одоо
borehole-inspector-check-order-differs = Ихэнх цооног эдгээр кодыг баганаас өөр дарааллаар өгсөн байна.
borehole-inspector-check-overruled = Илүү сул { $count } олонхийг илүү хүчтэй олонх хүчингүй болгосон.
borehole-inspector-check-place = Байр
borehole-inspector-check-proposed = Санал болгосон
borehole-inspector-check-show-differences = Зөрүүг харуулах
borehole-inspector-check-stale = Сүүлийн тулгалтаас хойш цооногууд өөрчлөгдсөн. Дахин тулгана уу.
borehole-inspector-check-summary = { $holes } цооног уншсанаас { $flagged } нь баганатай зөрж байна.
borehole-inspector-check-too-many-codes = Энэ талбар дараалалд оруулахад хэт олон кодтой.
borehole-inspector-checking-linked-geophysics-file = Холбосон геофизикийн файлыг шалгаж байна...
borehole-inspector-close-inspector = Шалгагчийг хаах
borehole-inspector-code-not-in-set = Баганад жагсаасан боловч энэ багцын ямар ч интервалд байхгүй
borehole-inspector-column = Багана
borehole-inspector-column-empty-check = Страт багана одоохондоо алга. Ихэнх цооногийн өгсөн дарааллаар бөглөхийн тулд Тулгах дарна уу, эсвэл доорх кодуудыг нэмж гараар эрэмбэлнэ үү.
borehole-inspector-data = Өгөгдөл
borehole-inspector-display = Дүрслэл
borehole-inspector-every-code-placed = Талбарын бүх код баганад байна.
borehole-inspector-flag-of-groups = { $kind } (бүлгүүд)
borehole-inspector-flag-out-of-place = Байрнаас зөрсөн
borehole-inspector-flag-overturned = Урвуу болсон
borehole-inspector-flag-repeat = Давтагдсан
borehole-inspector-flagged-holes = Тэмдэглэсэн цооногууд ({ $count })
borehole-inspector-flags-first-shown = { $count } тэмдэглэгээний эхний { $shown }-г жагсаав.
borehole-inspector-file-not-where-was-linked = { $file } холбогдсон байршилдаа байхгүй байна.
borehole-inspector-guessed-name = Нэрээр таамагласан
borehole-inspector-hold-hole-while-you-work = Эргэн тойрных нь цооногууд дээр ажиллах хугацаанд энэ цооногийг түгжиж байна уу.
borehole-inspector-holding-hole-click-follow-selection = Энэ цооногийг түгжиж байна. Сонголтыг дахин дагахын тулд товшино уу.
borehole-inspector-log = Каротаж
borehole-inspector-inspect-hole = Шалгах
borehole-inspector-no-holes-flagged = Баганатай зөрсөн цооног алга.
borehole-inspector-no-categorical-field = Энэ багцад эрэмбэлэх ангиллын талбар алга.
borehole-inspector-no-hole-inspected = Шалгаж буй цооног алга
borehole-inspector-not-in-column = Баганад байхгүй ({ $count })
borehole-inspector-pick-file = { $file }-г сонгох...
borehole-inspector-pick-file-again-show-its = Геофизикийг харуулахын тулд { $file }-г дахин сонгоно уу: хөтчийн хуудас файлыг өөрөө дахин нээж чадахгүй.
borehole-inspector-place-codes-note = Өгөгдөлд байгаа ч баганад хараахан жагсаагаагүй кодууд. Нэмсэн кодууд хамгийн доор орно; тэдгээрийг байранд нь шилжүүлнэ үү.
borehole-inspector-reading-geophysics-file-its-index = Геофизикийн файлын индексийг уншиж байна; явцыг төлөвийн мөрөөс харна уу.
borehole-inspector-remove-from-column = Баганаас хасах
borehole-inspector-strat = Страт
borehole-inspector-strat-column = Страт багана
borehole-inspector-summary = Хураангуй
canvas-circle-summary = Тойрог | Давхарга: { $layer } | радиус { $radius }

## Canvas strings

canvas-not-selectable-closed-polyline = Сонгох боломжгүй | Хаалттай полилиниа сонгоно уу
canvas-polyline-summary = Полилиниа | Давхарга: { $layer } | { $count } орой цэг
canvas-surface-name = Гадаргуу | { $name }
canvas-trimmed = Тайрсан
cinematic-shadows-method = Кино харагдацын сүүдэр: { $method }

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = { $object_id } объектоос уступ-берм үүсгэлээ
cmd-bezier-replaced-polyline-span-first-last = { $first }→{ $last } полилиниагийн хэсгийг { $count } дундын түүврийн цэгээр сольсон
cmd-bezier-vertices-first-last = { $first }-с { $last } хүртэлх орой цэг
cmd-block-model-block-model-loader-disconnected-path = { $path }-н блокийн загвар ачаалагч холболтоо тасалдлаа
cmd-block-model-block-model-path-has-count = { $path } блокийн загварт уншигдахгүй дэмжигдээгүй төрлийн { $count } хувьсагч бий: { $names }
cmd-block-model-building-ore-mesh = Хүдрийн тор бүтээж байна…
cmd-block-model-could-not-create-block-model = Блокийн загвар үүсгэж чадсангүй: { $error }
cmd-block-model-could-not-decode-block-model = Блокийн загварын '{ $variable }' өнгийн хувьсагчийг тайлбарлаж чадсангүй: { $error }
cmd-block-model-created-block-model-name-ordinary = Энгийн Кригингээр '{ $name }' блокийн загварыг үүсгэлээ
cmd-block-model-failed-load-block-model-error = Блокийн загварыг ачаалж чадсангүй: { $error }
cmd-block-model-generated-ore-mesh-from-block = '{ $name }' блокийн загвараас хүдрийн тор үүсгэлээ
cmd-block-model-imported-block-model-source-path = Импортолсон блокийн загварын эх сурвалж { $path }
cmd-block-model-loaded-block-model-name-blocks = '{ $name }' блокийн загварыг ачааллаа: { $blocks } блок ({ $renderable } дүрслэгдэх), тор { $dimx }x{ $dimy }x{ $dimz }, { $variables } хувьсагч
cmd-block-model-loading-name = { $name }-г ачаалж байна
cmd-block-model-loading-name-ellipsis = { $name }-г ачаалж байна…
cmd-chamfer-applied = { $corner } буланг { $radius } радиус, { $segments } сегментээр фасклав
cmd-chamfer-radius = Радиус { $radius }
cmd-commands-clipped = Огтолсон
cmd-commands-command-failed-error = Тушаал амжилтгүй боллоо: { $error }
cmd-commands-count-control-string-s = { $count } удирдах шугам
cmd-commands-count-control-string-s-layer = '{ $layer }' дээр { $count } удирдах шугам
cmd-commands-count-point-s-across-layers = { $layers } давхарга дахь { $count } цэг
cmd-commands-count-point-s-layer = '{ $layer }' дээр { $count } цэг
cmd-commands-kind-layer = '{ $layer }' дээр { $kind }
cmd-commands-no-control-strings = Удирдах шугам алга
cmd-commands-no-extent = Хамрах хүрээ алга
cmd-commands-no-points = Цэг алга
cmd-commands-select-holes-place-reference-points = Лавлах цэг байрлуулах цооногуудыг сонгоно уу
cmd-triangulate-needs-selection = Триангуляц үүсгэхийн өмнө триангуляц хийх объектуудыг сонгоно уу
cmd-commands-select-one-loaded-block-model = Хүдрийн триангуляц үүсгэхийн өмнө ачаалагдсан нэг блокийн загвар сонгоно уу
cmd-commands-select-one-loaded-drill-hole = Блокийн загвар үүсгэхийн өмнө ачаалагдсан нэг цооногийн цуглуулга сонгоно уу
cmd-commands-select-one-loaded-point-cloud = Триангуляц үүсгэхийн өмнө ачаалагдсан нэг цэгэн үүл сонгоно уу
cmd-contours-needs-triangulation = Изолиниа үүсгэхийн өмнө ачаалагдсан нэг триангуляц сонгоно уу
cmd-slice-needs-triangulation = Z мужаар огтлохын өмнө ачаалагдсан нэг триангуляц сонгоно уу
cmd-commands-select-one-loaded-triangulation-one = Огтлохын өмнө ачаалагдсан нэг триангуляц болон нэг хаалттай полилиниа сонгоно уу
cmd-commands-select-one-more-objects-before = { $axis }-ыг тохируулахаас өмнө нэг буюу түүнээс олон объект сонгоно уу
cmd-commands-sliced = Огтлосон (Slice)
cmd-commands-modelling-settings-set-settings = Загварчлалын тохиргоог шинэчиллээ. { $settings }
cmd-contours-contour-generation-failed-error = Изолиниа үүсгэлт амжилтгүй боллоо: { $error }
cmd-contours-discarded-layer-exists = '{ $name }'-н изолиниа хаягдлаа: '{ $layer_name }' давхарга аль хэдийн байна
cmd-contours-discarded-project-closed = '{ $name }'-н изолиниа хаягдлаа: төсөл хаагдсан байна
cmd-contours-discarded-layer-deleted = '{ $name }'-н изолиниа хаягдлаа: сонгосон гаралтын давхарга устгагдсан байна
cmd-contours-generated = '{ $name }' триангуляцын { $line_count } изолиниа полилиниаг '{ $layer_name }' давхаргад үүсгэлээ
cmd-creation-assembled-boundary-rings = Хуваагдсан задгай шугамуудаас { $assembled_count } хаалттай хилийн цагираг угсарлаа
cmd-creation-created-triangulation-from-boundary = { $boundary_count } хилийн цагираг ба { $constraint_count } задгай хязгаарлалтаас триангуляц үүсгэлээ, гадаргуугийн төрөл { $surface_type }
cmd-creation-creating-triangulation = Триангуляц үүсгэж байна…
cmd-creation-generate-upper-surface-ignored-count = Дээд гадаргуу үүсгэх: { $count } доод зөрчилдсөн эвдрэлийн шугамын сегментийг үл хэрэгсэв; эх объект өөрчлөгдөхгүй
cmd-creation-ignored-objects = Триангуляцын явцад полилиниа бус эсвэл доройтсон { $rejected } объектыг үл хэрэгсэв
cmd-creation-weld-retry-moved-coarse-welded = Гагнаад дахин оролдох: { $coarse_welded } орой цэгийг хамтын байрлал руу шилжүүлэв (хамгийн ихдээ { $coarse_weld_tol } м); эх объект өөрчлөгдөхгүй
cmd-creation-welded-breakline-vertices = Хязгаарын дотор давхцсан { $welded } эвдрэлийн шугамын орой цэгийг гагналаа
cmd-cuts-clipped-surface-name-polyline-mode = '{ $name }' гадаргууг полилиниагаар огтолов ({ $mode })
cmd-cuts-clipping-surface-polyline = Гадаргууг полилиниагаар огтолж байна…
cmd-cuts-cut-topology-name-pit-shell = '{ $name }' топологийг карьерийн бүрхүүлээр тайрлаа
cmd-cuts-cut-triangulation-name-z-band = '{ $name }' триангуляцыг Z зурвасаар [{ $min }, { $max }] тайрлаа
cmd-cuts-cutting-topology-pit-shell = Топологийг карьерийн бүрхүүлээр тайрж байна…
cmd-cuts-cutting-triangulation-z = Триангуляцыг Z-ээр тайрж байна…
cmd-cuts-ignored-vertical-faces = XY талбайгүй { $count } босоо эсвэл доройтсон лавлагаа топологийн талыг үл хэрэгсэв
cmd-cuts-site-skipped-constraint-from-x = { $site }: ({ $from_x }, { $from_y }) -> ({ $to_x }, { $to_y }) хязгаарлалтыг алгасав, триангулятор хуваах боломжгүй байлаа
cmd-cuts-skipped-degenerate-edges = { $site }: доройтолд ойрхон { $skipped } хязгаарлалтын ирмэгийг алгасав; тэдгээрийн ойролцоох огтлолын хил үсний зузаанаар зөрсөн байж болно
cmd-cuts-trimmed-surface = '{ $surface }' гадаргууг '{ $topology }' топологиор тайрлаа ({ $mode })
cmd-cuts-trimming-surface-topology = Гадаргууг топологиор тайрж байна…
cmd-drape-draped-intersected-vertices-changed = { $intersected } орой цэгийг дараалаа; { $changed } нь өндөрлөгөө өөрчиллөө
cmd-drape-no-intersections = Сонгосон зураг төслийн орой цэгүүдийн аль нь ч сонгосон топологитой огтлолцохгүй байна
cmd-drape-objects-changed-object-s-changed = { $objects } объект өөрчлөгдсөн · { $intersected }-с { $changed } огтлолцсон орой цэг шилжсэн
cmd-drape-select-one-more-design-objects = Дараах нэг буюу түүнээс олон зураг төслийн объектыг сонгоно уу
cmd-drape-select-one-more-topologies-drape = Дараах топологи болгон нэг буюу түүнээс олныг сонгоно уу
cmd-drape-selected-topologies-no-longer-loaded = Сонгосон топологи цаашид ачаалагдаагүй байна
cmd-drill-hole-choose-drillhole-source-files-again = Цооногийн эх файлуудыг дахин сонгоно уу
cmd-drill-hole-drill-pattern-too-large-contains = Өрмийн сүлжээ хэт том эсвэл буруу амсрын координат агуулж байна
cmd-drill-hole-drillhole-field-label-has-count = Цооногийн '{ $label }' талбарт { $count } өөр код байна, энэ нь кодлогдсон талбарт ердийн тооноос их; ангиллын талбар биш чөлөөт текст юм шиг харагдаж байна, гэхдээ код бүрийг хадгалж, өнгөөр будсан
cmd-drill-hole-enter-name-drill-pattern = Өрмийн сүлжээний нэрийг оруулна уу
cmd-drill-hole-failed-load-drillholes-error = Цооногийг ачаалж чадсангүй: { $error }
cmd-drill-hole-depth-must-be-positive = Цооногийн гүн тэгээс их байх ёстой
cmd-drill-hole-diameter-must-be-positive = Цооногийн диаметр тэгээс их байх ёстой
cmd-drill-hole-loaded-drillhole-dataset-name-holes = '{ $name }' цооногийн өгөгдлийн багцыг ачааллаа: { $holes } цооног, { $fields } өнгөний талбар
cmd-drill-hole-field-has-no-strat-column = { $field }-д страт багана алга; юу ч шилжүүлээгүй
cmd-drill-hole-name-already-loading = '{ $name }' аль хэдийн ачаалагдаж байна
cmd-drill-hole-name-reason = '{ $name }': { $reason }
cmd-drill-hole-no-hole-holds-value-field = Тэр талбарт '{ $value }' утгатай цооног алга
cmd-drill-hole-names-shifted-down = { $hole }-ийн нэрсийг цооногийн дагуу доош шилжүүллээ: { $moved } шилжсэн, { $unknown } нь UNK нэртэй болсон, баганад байхгүй { $untouched } хэвээр үлдсэн
cmd-drill-hole-names-shifted-up = { $hole }-ийн нэрсийг цооногийн дагуу дээш шилжүүллээ: { $moved } шилжсэн, { $unknown } нь UNK нэртэй болсон, баганад байхгүй { $untouched } хэвээр үлдсэн
cmd-drill-hole-no-interval-holds-seam = Одоо { $name }-г агуулсан интервал алга; юуг ч нэрийг нь өөрчлөөгүй
cmd-drill-hole-only-mapped-csv-bundles-imported = Хөтөч дээр зөвхөн харгалзуулсан CSV багцыг импортолно
cmd-drill-hole-pattern-contains-no-holes = Сүлжээнд цооног алга
cmd-drill-hole-no-interval-names-column-code = { $hole }-ийн ямар ч интервал баганын кодоор нэрлэгдээгүй; баганад байхгүй { $untouched } хэвээр үлдсэн
cmd-drill-hole-reading-name = { $name }-г уншиж байна
cmd-drill-hole-reference-points-used-holes-placed = Лавлах цэгүүд: { $used } цооног байрлуулсан, { $absent } нь '{ $value }'-гүй, { $flagged } нь хагарлын давтагдал байж болзошгүй гэж тэмдэглэсэн
cmd-drill-hole-no-collars = Цэг тавих амсартай цооног алга
cmd-drill-hole-collars-layer = Амсарууд
cmd-drill-hole-collar-points = Амсрын цэгүүд: { $used } цооног байрлуулсан, { $absent } нь амсаргүй
cmd-drill-hole-seam-renamed = { $from }-г { $to } болгон нэрийг нь өөрчиллөө; санал болгосон залруулгын бүртгэл: { $count }
cmd-drill-hole-uppermost-run-used-flagged-holes = Хамгийн дээд хэсгийг ашигласан, тэмдэглэсэн: { $holes }
cmd-drill-hole-working-section-name-not-same = '{ $name }' ажлын үе сонгосон өгөгдлийн багц бүрт ижил биш; багц бүрийн өөрийнхийг ашигласан.
cmd-drill-hole-working-sections-not-kept-dataset = '{ $dataset }'-д ажлын үеийг хадгалсангүй. { $reasons }
cmd-explode-count-line-s = { $count } шугам
cmd-explode-polyline = Полилиниаг задлах
cmd-explode-exploded-polyline-into-count-line = Полилиниаг { $count } шугамын сегмент болгож задлав
cmd-file-block-model-csv-encoding-failed = Блокийн загварын CSV кодчилол амжилтгүй боллоо: { $error }
cmd-file-block-model-csv-export-failed = Блокийн загварын CSV экспорт амжилтгүй боллоо: { $error }
cmd-file-browser-recovery-unavailable = Хөтчийн сэргээх файл боломжгүй байна; хадгалагдсан төслүүд IndexedDB-д хэвээр байна
cmd-file-closed-project-runtime-id-runtime = { $runtime_id } ажиллагааны ID-тай төслийг хаалаа
cmd-file-could-not-create-new-project = Шинэ төсөл үүсгэж чадсангүй: { $error }
cmd-file-could-not-finish-pending-project = Хүлээгдэж буй төслийн үйлдлийг дуусгаж чадсангүй: { $error }
cmd-file-could-not-finish-saving-before = Гарахаас өмнө хадгалалтыг дуусгаж чадсангүй: { $error }
cmd-file-could-not-open-browser-project = Хөтчийн төслийг нээж чадсангүй: { $error }
cmd-file-could-not-open-path-error = { $path }-ыг нээж чадсангүй: { $error }
cmd-file-could-not-read-selected-file = Сонгосон файлыг уншиж чадсангүй: { $error }
cmd-file-could-not-reload-layer-from = Давхаргыг дискнээс дахин ачаалж чадсангүй: { $error }
cmd-file-could-not-reload-project-from = Төслийг дискнээс дахин ачаалж чадсангүй: { $error }
cmd-file-could-not-remove-browser-project = Хөтчийн төслийг устгаж чадсангүй: { $error }
cmd-file-could-not-restore-layer-from = Давхаргыг төслөөс сэргээж чадсангүй: { $error }
cmd-file-could-not-snapshot-dirty-project = Сэргээлтэд зориулж өөрчлөгдсөн төслийн агшинг авч чадсангүй: { $error }
cmd-file-could-not-start-browser-export = Хөтчийн экспортыг эхлүүлж чадсангүй: { $error }
cmd-file-could-not-write-recovery-copies = Сэргээх хуулбарыг бичиж чадсангүй: { $error }
cmd-file-created-new-browser-project = Шинэ хөтчийн төсөл үүсгэлээ
cmd-file-created-new-project = Шинэ төсөл үүсгэлээ
cmd-file-description-download-failed-error = { $description } татах явдал амжилтгүй боллоо: { $error }
cmd-file-discard-cancelled-project-changed = OMF дахин ачаалж байх зуур төсөл өөрчлөгдсөн тул үл хэрэгсэх үйлдэл цуцлагдлаа
cmd-file-discarded-changes-layer-target-name = '{ $target_name }' давхаргын өөрчлөлтийг үл хэрэгсэв
cmd-file-discarded-changes-reloaded-path = Өөрчлөлтийг үл хэрэгсэв: { $path }-г дахин ачааллаа
cmd-file-downhole-geophysics-csv = Цооногийн геофизикийн CSV
cmd-file-downloaded-description-file-name = { $description } татагдлаа: { $file_name }
cmd-file-drillhole-csv-export-failed-error = Цооногийн CSV экспорт амжилтгүй боллоо: { $error }
cmd-file-dxf-download-encoding-failed-error = DXF татаж авах кодчилол амжилтгүй боллоо: { $error }
cmd-file-dxf-import-failed-error = DXF импорт амжилтгүй боллоо: { $error }
cmd-file-encoding-block-model-csv-download = Блокийн загварын CSV татаж авалтыг кодчилж байна…
cmd-file-encoding-dxf-download = DXF татаж авалтыг кодчилж байна…
cmd-file-encoding-triangulation-download = Триангуляц татаж авалтыг кодчилж байна…
cmd-file-exit-deferred-exports = Далд экспорт дуустал гарахыг хойшлуулав
cmd-file-exit-requested-no-unsaved-changes = Хадгалаагүй өөрчлөлт байхгүй тул гарахыг хүслээ
cmd-file-exported-block-model-csv-path = Блокийн загварын CSV-г { $path } руу экспортоллоо
cmd-file-exported-description-dxf-path = { $description }-г DXF болгон экспортоллоо: { $path }
cmd-file-exported-three-drillhole-csvs-path = Цооногийн гурван CSV-г { $path } руу экспортоллоо
cmd-file-exported-triangulation-name-path = '{ $name }' триангуляцыг { $path } руу экспортоллоо
cmd-file-exporting-name = { $name }-г экспортолж байна…
cmd-file-exporting-triangulation-name-path = '{ $name }' триангуляцыг { $path } руу экспортолж байна
cmd-file-fatal-renderer-failure-reason = Дүрслэгчийн ноцтой алдаа: { $reason }
cmd-file-dialog-action-failed = Файлын харилцах цонхны үйлдэл амжилтгүй боллоо: { $msg }
cmd-file-imported-added-object-s-from = { $name }-с { $added } объект импортлож нэмлээ
cmd-file-imported-total-dxf-object-s = { $total } DXF объектыг импортоллоо
cmd-file-layer-discard-was-cancelled-because = Төсөл дахин ачаалагдаж байх зуур төсөл өөрчлөгдсөн тул давхаргыг үл хэрэгсэх үйлдэл цуцлагдлаа
cmd-file-no-recovery-directory = Сэргээх хавтас байхгүй байна: { $error }
cmd-file-no-unsaved-project-content-nothing = Хадгалаагүй төслийн агуулга алга; сэргээх зүйл алга
cmd-file-parsing-browser-dxf-import = Хөтчийн DXF импортыг задалж байна…
cmd-file-parsing-dxf-import = DXF импортыг задалж байна…
cmd-file-project-closes-after-save = Одоогийн хадгалалт дуусмагц төсөл хаагдана
cmd-file-the-project-closes-after-save = Одоогийн хадгалалт дуусмагц төсөл хаагдана
cmd-file-queued-count-triangulation-file-s = Импортлохоор { $count } триангуляцын файлыг дараалалд оруулав
cmd-file-recovery-copies-path-reopen-them = Сэргээх хуулбарууд { $path }-д байна; дахин эхлүүлсний дараа тэдгээрийг нээнэ үү
cmd-file-recovery-copy-failed-error = Сэргээх хуулбар амжилтгүй боллоо: { $error }
cmd-file-recovery-copy-failed-failure = Сэргээх хуулбар амжилтгүй боллоо: { $failure }
cmd-file-recovery-copy-written-path = Сэргээх хуулбар бичигдлээ: { $path }
cmd-file-reverting-layer = Давхаргыг буцааж байна…
cmd-file-reverting-project = Төслийг буцааж байна…
cmd-file-save-failed-message = Хадгалах явдал амжилтгүй боллоо: { $message }
cmd-file-save-project-already-running-save = Энэ төслийн хадгалалт аль хэдийн явагдаж байна; дуусмагц дахин хадгална уу
cmd-file-save-worker-ended-without-result = Хадгалах ажлын процесс үр дүнгүй дуусав
cmd-file-saved-project-as = Төслийг дараах нэрээр хадгаллаа: { $path }
cmd-file-saved-project = Төслийг хадгаллаа: { $path }
cmd-file-saving-browser-storage = Хөтчийн санах ойд хадгалж байна…
cmd-file-selected-block-model-no-longer = Сонгосон блокийн загвар цаашид ачаалагдаагүй байна
cmd-file-selected-drillhole-dataset-no-longer = Сонгосон цооногийн өгөгдлийн багц цаашид ачаалагдаагүй байна
cmd-file-switching-project = Төслийг сольж байна…
cmd-file-triangulation-download-encoding-failed = Триангуляц татаж авах кодчилол амжилтгүй боллоо: { $error }
cmd-file-user-chose-exit-without-saving = Хэрэглэгч хадгалахгүйгээр гарахыг сонгов
cmd-file-user-requested-exit-project-export = Хэрэглэгч гарахыг хүслээ (төслийн экспорт эсвэл хадгалаагүй ажлыг батлах шаардлагатай)
cmd-file-viewport = Харагдах цонх
cmd-file-wait-current-project-save-finish = Одоогийн төслийн хадгалалт дуусахыг хүлээнэ үү
cmd-file-wait-current-project-switch-finish = Одоогийн төслийн шилжилт дуусахыг хүлээнэ үү
cmd-file-wait-project-operation-finish-before = Өөрчлөлтийг үл хэрэгсэхээс өмнө төслийн үйлдэл дуусахыг хүлээнэ үү
cmd-file-wait-project-revert-finish-before = Хадгалахаас өмнө төслийн буцаалт дуусахыг хүлээнэ үү
cmd-folder-collection-named-name-already-exists = '{ $name }' нэртэй цуглуулга аль хэдийн байна
cmd-folder-collection-no-longer-exists = Тэр цуглуулга цаашид байхгүй болжээ
cmd-folder-created-collection-name = '{ $name }' цуглуулгыг үүсгэлээ
cmd-folder-deleted-collection-name = '{ $name }' цуглуулгыг устгалаа
cmd-folder-moved-item-into-collection-name = Зүйлийг '{ $name }' цуглуулга руу зөөлөө
cmd-folder-moved-item-root-section = Зүйлийг { $section }-н үндэс рүү зөөлөө
cmd-folder-renamed-collection-before-after = '{ $before }' цуглуулгын нэрийг '{ $after }' болгов
cmd-folder-section-cannot-hold-item = Тэр хэсэг энэ зүйлийг агуулж чадахгүй
cmd-fuse-closed-polyline = Хаалттай полилиниа
cmd-fuse-count-source-line-s = { $count } эх шугам
cmd-fuse-created-shape-object-id-vertices = { $sources } эх шугамаас { $vertices } орой цэгтэй { $shape } { $object_id }-г үүсгэлээ
cmd-fuse-click-missed = Нэгтгэх: товшилт ямар ч объектод тусаагүй (заагчийн доор юу ч байхгүй)
cmd-fuse-click-not-near-endpoint = Нэгтгэх: товшилт сонгосон шугамын аль ч үзүүрт хангалттай ойрхон биш байна
cmd-fuse-clicked-closed-polyline = Нэгтгэх: товшсон { $object_id } объект хаалттай полилиниа байна, нэгтгэх нь зөвхөн задгай полилиниад ажиллана
cmd-fuse-clicked-not-open-polyline = Нэгтгэх: товшсон { $object_id } объект задгай полилиниа биш (энэ нь { $kind })
cmd-fuse-clicked-object-missing = Нэгтгэх: товшсон { $object_id } объект цаашид байхгүй болжээ
cmd-fuse-clicked-too-few-vertices = Нэгтгэх: товшсон { $object_id } полилиниад ердөө { $count } орой цэг байна, дор хаяж 2 хэрэгтэй
cmd-fuse-endpoint-marker-missing = Нэгтгэх: үзүүрийн { $marker_index } тэмдэг цаашид байхгүй болжээ
cmd-fuse-close-needs-three-vertices = Нэгтгэх: полилиниа болгож хаахад дор хаяж 3 өөр орой цэг хэрэгтэй (одоо { $count })
cmd-fuse-lines = Шугамуудыг нэгтгэх
cmd-fuse-needs-two-segments = Нэгтгэх: батлахын тулд дор хаяж 2 сегмент хэрэгтэй (одоо { $count })
cmd-fuse-no-active-layer = Нэгтгэх: нэгтгэсэн шугамыг байрлуулах идэвхтэй давхарга алга
cmd-fuse-no-active-project = Нэгтгэх: идэвхтэй төсөл байхгүй тул батлах боломжгүй
cmd-fuse-no-source-line = Нэгтгэх: полилиниа болгож хаах эх шугам алга
cmd-fuse-awaiting-object-invalid = Нэгтгэх: { $awaiting_id } объект цаашид зөв полилиниа биш болжээ
cmd-fuse-object-already-in-chain = Нэгтгэх: { $object_id } объект аль хэдийн нэгтгэлийн гинжинд орсон байна, өөр шугам товшино уу
cmd-fuse-result-too-few-vertices = Нэгтгэх: үр дүнд орой цэг хэт цөөн байна ({ $count }), таслав
cmd-fuse-segment-object-invalid = Нэгтгэх: { $object_id } сегмент объект цаашид зөв полилиниа биш болжээ, таслав
cmd-fuse-source-object-invalid = Нэгтгэх: эх { $object_id } объект цаашид зөв задгай полилиниа биш болжээ
cmd-fuse-source-object-missing = Нэгтгэх: эх { $object_id } объект цаашид байхгүй болжээ
cmd-fuse-open-polyline = Задгай полилиниа
cmd-include-failed = Оруулах явдал амжилтгүй боллоо: { $message }
cmd-include-included-solid-shape-name-topology = '{ $shape_name }' хатуу биеийг '{ $topology_name }' топологид оруулав ({ $retained } топологийн тал хадгалж, { $skipped } хаах таг талыг алгаслаа)
cmd-include-including-pit-stockpile-solid = Карьер/овоолгын хатуу биеийг оруулж байна…
cmd-insert-point-count-operation-point-s = { $count } { $operation } цэг
cmd-insert-point-elevation-must-be-finite = Өндөрлөг дээр цэг оруулахад тодорхой хязгаартай өндөрлөг шаардлагатай
cmd-insert-point-insert-points = Цэгүүд оруулах
cmd-insert-point-inserted-count-operation-point-s = { $count } { $operation } цэг оруулав
cmd-insert-point-intersection = Огтлолцол
cmd-insert-point-no-new-operation-points-were = Шинэ { $operation } цэг олдсонгүй
cmd-insert-point-select-least-two-polylines-before = Огтлолцлын цэг оруулахаас өмнө дор хаяж хоёр полилиниа сонгоно уу
cmd-insert-point-select-one-more-polylines-before = Өндөрлөг дээр цэг оруулахаас өмнө нэг буюу түүнээс олон полилиниа сонгоно уу
cmd-layer-created-layer-name = '{ $name }' давхаргыг үүсгэлээ
cmd-layer-deleted-with-objects = { $layer_id } давхаргыг (бүх объектын хамт) устгалаа
cmd-layer-duplicated-layer-duplicate-name = '{ $duplicate_name }' давхаргыг хувиллаа
cmd-layer-locked = Түгжигдсэн
cmd-layer-name-copy = { $name } хуулбар
cmd-layer-selected-count-object-s-layer = { $layer_id } давхаргад { $count } объект сонгов
cmd-layer-state-layer-name = '{ $name }' давхарга { $state }
cmd-layer-unlocked = Түгжээгүй
cmd-move-tool-moved-collars = Шилжилт ({ $delta })-г { $count } цооногийн амсарт хэрэглэлээ
cmd-move-tool-moved-objects = Шилжилт ({ $delta })-г { $count } объектод хэрэглэлээ
cmd-move-tool-count-hole-s = { $count } цооног
cmd-object-edit-edited-kind = { $kind } засварлалаа
cmd-object-edit-edited-kind-count-vertices = { $kind } засварлалаа ({ $count } орой цэг)
cmd-object-edit-no-changes-apply = Хэрэглэх өөрчлөлт алга
cmd-object-edit-object-changed-since-editor-opened = Энэ объект засварлагч нээгдсэнээс хойш өөрчлөгдсөн байна; одоогийн хувилбарыг засварлахын тулд дахин нээнэ үү
cmd-object-edit-target-changed = Заслын объект өөрчлөгдсөн тул заслыг хаялаа
cmd-object-edit-object-no-longer-exists-document = Тэр объект баримт бичигт цаашид байхгүй байна
cmd-object-edit-no-strings-reverse = Сонгосон шугамыг урвуулах боломжгүй (нуусан эсвэл түгжсэн)
cmd-object-edit-reversed-strings = { $count } шугамыг урвуулсан
cmd-object-edit-select-single-design-object-edit = Засварлах ганц зураг төслийн объект сонгоно уу
cmd-object-edit-unassigned = Оноогдоогүй
cmd-offset-create-offset = Шилжилт үүсгэх
cmd-offset-created-offset-count-object-s = { $count } объектын шилжилт үүсгэлээ
cmd-offset-distance-must-be-positive = Шилжилтийн зай тэгээс их байх ёстой
cmd-offset-skipped-count-circle-s-offset = { $count } тойргийг алгасав: шилжилтийн зай радиусаас их байна
cmd-omf-could-not-open-project-source = { $source_name } төслийг нээж чадсангүй: { $error }
cmd-omf-create-open-project-before-merging = Өгөгдөл нэгтгэхээс өмнө төсөл үүсгэх эсвэл нээнэ үү
cmd-omf-dataset-name-count-working-section = '{ $name }' өгөгдлийн багц: { $count } ажлын үеийг сэргээж чадсангүй: { $details }
cmd-omf-field-codes-partly-coloured = '{ $name }' өгөгдлийн багц: '{ $field }' талбарыг { $total } кодын { $saved }-г нь өнгөлсөн байдлаар хадгалсан; үлдсэнд автоматаар үүсгэсэн өнгө өгсөн.
cmd-omf-encoding-project = Төслийг кодчилж байна…
cmd-omf-exported-project-path = Төслийг { $path } руу экспортоллоо
cmd-omf-imported-project = '{ $project_name }' төслийг { $source_name }-с импортоллоо: { $count } дээд түвшний өгөгдлийн багц
cmd-omf-importing-project = Төслийг импортолж байна…
cmd-omf-export-failed = OMF экспорт амжилтгүй боллоо: { $error }
cmd-omf-import-failed = OMF импорт амжилтгүй боллоо: { $error }
cmd-omf-opened-project = '{ $project_name }' төслийг { $source_name }-с нээлээ
cmd-omf-project-source-name-contains-no = '{ $source_name }' төсөл дэмжигдсэн өгөгдлийн элемент агуулаагүй байна
cmd-omf-source-name-applied-project-origin = { $source_name }: нэгтгэхийн өмнө төслийн { $origin } эх цэгийг хэрэглэлээ
cmd-omf-crs-differs = { $source_name }: координатын лавлагаа систем '{ $source_crs }' нь төслийн '{ $target_crs }' системээс ялгаатай байна; координатыг дахин проекцлолгүй нэгтгэлээ
cmd-omf-source-name-units-source-units = { $source_name }: нэгж '{ $source_units }' нь төслийн '{ $target_units }' нэгжээс ялгаатай байна; координатыг хөрвүүлэлгүй нэгтгэлээ
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = Экспортлох Incline Design өгөгдөл нээлттэй байхгүй байна
cmd-placement-2-vertices = 2 орой цэг
cmd-placement-count-vertices = { $count } орой цэг
cmd-placement-created-circle = { $radius } м радиустай тойрог үүсгэлээ
cmd-placement-created-closed-polyline = { $count } орой цэгтэй хаалттай полилиниа үүсгэлээ
cmd-placement-created-line-segment-2-vertices = 2 орой цэгтэй шугамын сегмент үүсгэлээ
cmd-placement-created-open-polyline-count-vertices = { $count } орой цэгтэй задгай полилиниа үүсгэлээ
cmd-placement-placed-point-x-y-z = { $x }, { $y }, { $z }-д цэг байрлуулав
cmd-placement-radius = Радиус { $radius } м
cmd-plot-composing-engineering-drawing = Инженерийн зургийг бүтээж байна…
cmd-plot-could-not-write-engineering-drawing = Инженерийн зургийг бичиж чадсангүй: { $error }
cmd-plot-drawing-scale-fitted-visible-data = Зургийн масштабыг харагдаж буй өгөгдөлд тааруулав: 1:{ $scale }
cmd-plot = Зураг
cmd-plot-saved-drawing = Инженерийн зургийг хадгаллаа: { $description } ({ $width } × { $height } px, { $dpi } dpi)
cmd-point-cloud-classified = { $name }-г ангилав: { $count } цэгээс { $ground } газар, { $vegetation } ургамалжилт, { $noise } шуугиан
cmd-point-cloud-classifying-point-clouds = Цэгэн үүлийг ангилж байна
cmd-point-cloud-join-dropped-classifications = Цэгийн ангиллыг хаялаа: нэгтгэсэн үүлний зарим нь ангилагдаагүй, хэсэгчлэн ангилсан үүлийг газар болгон шүүх боломжгүй.
cmd-point-cloud-failed-classify-point-clouds-error = Цэгэн үүлийг ангилж чадсангүй: { $error }
cmd-point-cloud-failed-join-point-clouds-error = Цэгэн үүлийг нэгтгэж чадсангүй: { $error }
cmd-point-cloud-failed-load-point-cloud-error = Цэгэн үүлийг ачаалж чадсангүй: { $error }
cmd-point-cloud-joined-count-clouds-into-name = { $count } үүлийг { $name } болгон нэгтгэлээ ({ $points } цэг)
cmd-point-cloud-joining-name = { $name }-г нэгтгэж байна
cmd-point-cloud-loaded-point-cloud-name-count = { $name } цэгэн үүлийг ачааллаа ({ $count } цэг)
cmd-point-cloud-point-cloud-classification-discarded = Цэгэн үүлийн ангиллыг хаялаа: ажиллах явцад үүл өөрчлөгдсөн. Дахин ажиллуулна уу.
cmd-point-cloud-point-cloud-loader-disconnected-path = { $path }-н цэгэн үүл ачаалагч холболтоо тасалдлаа
cmd-point-cloud-select-one-more-loaded-point = Ангилахын өмнө ачаалагдсан нэг буюу түүнээс олон цэгэн үүл сонгоно уу
cmd-point-cloud-select-two-more-loaded-point = Нэгтгэхийн өмнө ачаалагдсан хоёр буюу түүнээс олон цэгэн үүл сонгоно уу
cmd-point-cloud-tin-max-edge-disabled = (дээд ирмэг идэвхгүй)
cmd-point-cloud-tin-max-edge-max-edge = (дээд ирмэг { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = Цэгэн үүлийн TIN амжилтгүй боллоо: { $error }
cmd-point-cloud-tin-filtered-ground = Рельефийн TIN: { $total } цэгээс { $ground } газрын цэгээр шүүв
cmd-point-cloud-tin-subsampled = Рельефийн TIN: орон зайгаар дэд-түүвэрлэсэн { $total } цэгээс { $sampled }
cmd-point-cloud-tin-triangulated = Рельефийн TIN: { $vertex_count } давхцаагүй XY цэгийг { $face_count } тал болгож триангуляцлав{ $suffix }
cmd-products-added-product-delay-ms-ms = { $delay_ms } мс { $name } бүтээгдэхүүнийг нэмлээ
cmd-products-deleted-product-delay-ms-ms = { $delay_ms } мс { $name } бүтээгдэхүүнийг устгалаа
cmd-products-failed-save-products-error = Бүтээгдэхүүнийг хадгалж чадсангүй: { $error }
cmd-products-product-no-longer-palette = Тэр бүтээгдэхүүн цаашид палитрт байхгүй байна
cmd-property-action-count-object-s-layer = { $layer } давхарга руу { $count } объектыг { $action }
cmd-property-batch-set-axis-value-count = { $count } объектод { $axis } утгыг багцаар тохируулах
cmd-property-batch-set-closed-count-polyline = { $count } полилиниаг багцаар хаалттай болгох
cmd-property-batch-set-color-count-object = { $count } объектын өнгийг багцаар тохируулах
cmd-property-batch-set-fill-style-count = { $count } объектын дүүргэлтийн хэв маягийг багцаар тохируулах
cmd-property-batch-set-line-weight-count = { $count } полилиниагийн шугамын зузааныг багцаар тохируулах
cmd-property-copied = Хуулсан
cmd-property-moved = Шилжсэн
cmd-raster-draped = { $raster } растерыг { $triangulation } триангуляц дээр дараалаа (давхцах хэмжээ)
cmd-raster-failed-load-raster-name-error = { $name } растерыг ачаалж чадсангүй: { $error }
cmd-raster-failed-load-raster-path-error = { $path } растерыг ачаалж чадсангүй: { $error }
cmd-raster-loaded-raster-name-via-driver = { $name } растерыг { $driver } драйверээр ачааллаа ({ $srcx }x{ $srcy }, урьдчилан харах { $prevx }x{ $prevy })
cmd-raster-no-overlapping-triangulation = { $name }-н хэмжээтэй давхцах ачаалагдсан триангуляц алга
cmd-raster-loader-disconnected = { $path }-н растер ачаалагч холболтоо тасалдлаа
cmd-raster-undraped = { $count } триангуляцаас растерыг дараахаас цуцаллаа
cmd-reference-surface-build-surface-failed-error = Гадаргуу бүтээх амжилтгүй боллоо: { $error }
cmd-reference-surface-building-surface = Гадаргууг бүтээж байна…
cmd-reference-surface-built-surface-name-inside-grid = Хамрах хүрээн доторх { $inside } торын цэгээс { $name } гадаргууг { $vertex_count } цэг, { $face_count } талтайгаар бүтээлээ, хайрцгийн z { $low }-с { $high }{ $support }{ $controls }
cmd-reference-surface-built-surface-name-from-vertex = { $vertex_count } цэгээс { $name } гадаргууг { $face_count } талтайгаар бүтээлээ, хайрцгийн z { $low }-с { $high }{ $support }{ $coincident }{ $controls }
cmd-reference-surface-control-string-index-could-not = { $index } удирдах шугамыг торонд нэмж чадсангүй
cmd-reference-surface-control-string-index-crosses-itself = { $index } удирдах шугам төлөвлөгөөн дээр ({ $x }, { $y }) цэгт өөрөө өөртэйгөө огтлолцож байна
cmd-reference-surface-control-string-index-doubles-back = { $index } удирдах шугам төлөвлөгөөн дээр ({ $x }, { $y }) цэгт өөрөө өөр рүүгээ буцаж байна
cmd-reference-surface-control-string-index-ends-where = { $index } удирдах шугам эхэлсэн газартаа төгсөж байна; маск болгон ашиглахын тулд хаана уу
cmd-reference-surface-control-string-index-has-count = { $index } удирдах шугамд { $count } өөр орой цэг байна; удирдах шугамд дор хаяж { $minimum } хэрэгтэй
cmd-reference-surface-control-string-index-has-non = { $index } удирдах шугам төгсгөлөг биш координаттай байна
cmd-reference-surface-control-string-index-no-longer = { $index } удирдах шугам цаашид боломжгүй байна
cmd-reference-surface-control-string-overrides-pick-x = Удирдах шугам ({ $x }, { $y }) дахь сонголтыг дарж байна: сонголт { $pick } м, удирдах шугам { $control } м, зөрүү { $difference } м
cmd-reference-surface-control-strings-b-disagree-x = { $a } ба { $b } удирдах шугамууд ({ $x }, { $y }) дээр зөрж байна: { $za } м ба { $zb } м, { $difference } м зөрүүтэй
cmd-reference-surface-control-strings-b-run-along = { $a } ба { $b } удирдах шугамууд төлөвлөгөөн дээр бие биеийнхээ дагуу явж байна; үүнийг одоогоор дэмжихгүй
cmd-reference-surface-count-control-string-s-entered = ; { $count } удирдах шугамыг { $points } цэг болгон оруулсан{ $crossings }
cmd-reference-surface-count-other-strings-hidden = Бусад { $count } удирдах шугамыг нуусан; харагдацын хэрэгслийн самбар дээрх Нуусныг бүгдийг харуулах тэдгээрийг буцаана
cmd-unhide-all-count = Нуусан { $count } объектыг дахин харуулав
cmd-unhide-all-objects-items-count = Нуусан { $objects } объект болон { $items } зүйлийг дахин харуулав
cmd-unhide-all-nothing-hidden = Ачаалсан давхаргуудад нуусан объект алга
cmd-reference-surface-count-control-string-s-vertices = ; { $vertices } орой цэгтэй { $count } удирдах шугам{ $crossings }
cmd-reference-surface-count-point-s-inside-extent = Хамрах хүрээн доторх { $count } цэг; гадаргууд дор хаяж { $minimum } хэрэгтэй
cmd-reference-surface-count-point-s-outside-extent = ; хамрах хүрээнээс гадуурх { $count } цэг гадаргууг тулгуур болгон хэлбэржүүлсэн
cmd-reference-surface-count-point-s-selected-surface = { $count } цэг сонгосон; гадаргууд дор хаяж { $minimum } хэрэгтэй
cmd-reference-surface-picks-and-vertices-selected-surface = { $picks } цэг ба удирдах шугамын { $vertices } орой цэг сонгосон; гадаргууд нийлээд дор хаяж { $minimum } хэрэгтэй
cmd-reference-surface-count-places-stop-build = Удирдах шугамуудын { $count } байршил бүтээлтийг зогсоож байна, тус бүрийг цагирагаар тэмдэглэсэн:
cmd-reference-surface-cleaned-heading = Гадаргуу бүтээх нь удирдах шугамуудын өөрийн хуулбарыг Шугамуудыг цэвэрлэх болон Бүгдийг дундаж өндөрт холбох хийх шиг цэвэрлэв; төсөл дэх шугамууд өөрчлөгдөөгүй:
cmd-reference-surface-cleaned-repeats = Давтагдсан цэгүүдийг нэг болгон нэгтгэсэн { $count } байршил, { $places }
cmd-reference-surface-cleaned-spikes = { $count } шовх устгав, { $places }
cmd-reference-surface-cleaned-retraces = Шугам дээгүүр буцаж явсан { $count } хэсгийг тайрав, { $places }
cmd-reference-surface-cleaned-loops = Шугам өөрөө өөртэйгөө огтлолцсон { $count } гогцоог тайрч авав, { $places }
cmd-reference-surface-cleaned-zeros = z = 0 дээрх { $count } орой цэгийг устгав, { $places }
cmd-reference-surface-cleaned-heights = Хөршөөсөө хол { $count } ганц өндрийг устгав, { $places }
cmd-reference-surface-cleaned-shared-cut = Хоёр шугамын хуваалцсан { $count } хэсгийг богино шугамаас тайрав, { $places }
cmd-reference-surface-cleaned-removed = Бүх уртаараа өөр шугамын дагуу явсан { $count } шугамыг хуулбараас хасав, { $places }
cmd-reference-surface-cleaned-joined-small = { $limit } м ба түүнээс бага зөрүүтэй { $count } огтлолцлыг дундаж өндөрт холбов, { $places }
cmd-reference-surface-cleaned-joined-on-request = { $low } м-ээс их, { $high } м хүртэл зөрүүтэй { $count } огтлолцлыг дундаж өндөрт холбов, { $places }
cmd-reference-surface-cleaned-vertex-shared = Шугамууд { $limit } м-ээс их зөрүүтэй газарт { $count } нийтлэг орой цэг оруулав, { $places }
cmd-reference-surface-left-out-count = Энэ бүтээлтээс { $count } удирдах шугамыг хасав, тус бүрийг цагирагаар тэмдэглэж сонгосон; гадаргууг үлдсэнээс бүтээнэ:
cmd-reference-surface-left-out-below = { $string } шугамыг хасав: { $places } дээр { $others }-аас { $amount } м доор байна
cmd-reference-surface-left-out-above = { $string } шугамыг хасав: { $places } дээр { $others }-аас { $amount } м дээр байна
cmd-reference-surface-left-out-above-and-below = { $string } шугамыг хасав: { $places } дээр { $others }-аас { $amount } м дээр болон доор байна
cmd-reference-surface-left-out-along = { $string } шугамыг хасав: { $places } дээр { $others }-ын дагуу явж байна
cmd-reference-surface-left-out-range = { $low }-аас { $high }
cmd-reference-surface-left-out-other-string = { $string } шугам
cmd-reference-surface-left-out-other-strings = { $strings } шугамууд
cmd-reference-surface-left-out-too-short = { $string } шугамыг хасав: хоёроос цөөн ялгаатай орой цэгтэй, ({ $x }, { $y })
cmd-reference-surface-left-out-ends-where-it-starts = { $string } шугамыг хасав: эхэлсэн газраа дуусдаг, ({ $x }, { $y })
cmd-reference-surface-left-out-turns-back = { $string } шугамыг хасав: ({ $x }, { $y }) цэгт эргэж буцдаг
cmd-reference-surface-left-out-crosses-itself = { $string } шугамыг хасав: ({ $x }, { $y }) цэгт өөрөө өөртэйгөө огтлолцдог
cmd-reference-surface-left-out-points-disagree = { $string } шугамыг хасав: төлөвлөгөөн дээрх нэг газар байгаа хоёр цэг нь өндрөөрөө { $miss } м зөрүүтэй, ({ $x }, { $y })
cmd-reference-surface-left-out-none-left = Мөргөлдөж буй эсвэл буруу хэлбэртэй шугамуудыг хасвал нэг ч удирдах шугам үлдэхгүй тул юу ч бүтээхгүй
cmd-reference-surface-thinned = Удирдах шугамууд нэг гадаргуунд хэт олон цэг өгч байсан тул бүтээлт тэдгээрийн хуулбарыг сийрүүлэв: төгсгөлүүд, огтлолцох газрууд болон тухайн оройгүйгээр шугамаас төлөвлөгөө эсвэл өндрөөр { $tolerance } м-ээс хол байгаа орой бүрт { $kept } цэг үлдээв{ $raised }, мөн дагуу нь { $spacing } м тутамд цэг байрлуулав; { $budget } төсвөөс { $used } цэг ашиглав
cmd-reference-surface-thinned-raised = ({ $first } м-ээс өсгөв, учир нь багаар багтахгүй)
cmd-reference-surface-thin-refused = Удирдах шугамууд нэг гадаргуугийн { $budget } цэгийн төсөвт багтахгүй: зөвхөн төгсгөл, огтлолцол болон тэдгээргүйгээр шугамаас { $tolerance } м-ээс хол оройг үлдээсэн ч { $kept } цэг, { $picks } сонголттой нийлээд { $total } болно; юу ч бүтээхгүй
cmd-reference-surface-count-refused-strings-selected = Татгалзсан { $count } удирдах шугамыг одоо сонгосон
cmd-reference-surface-count-point-s-shared-plan = ; { $count } цэг төлөвлөгөөн дээр ижил байрлалтай байсан тул нэг удаа хадгаллаа
cmd-reference-surface-delaunay-insert-failed-error = Делонэ оруулалт амжилтгүй боллоо: { $error }
cmd-reference-surface-extent-must-closed-string = Хамрах хүрээ нь хаалттай шугам байх ёстой
cmd-reference-surface-extent-string-crosses-itself-plan = Хамрах хүрээний шугам төлөвлөгөөн дээр өөрөө өөртэйгөө огтлолцож байна
cmd-reference-surface-extent-string-has-non-finite = Хамрах хүрээний шугам төгсгөлөг биш координаттай байна
cmd-reference-surface-extent-string-needs-least-three = Хамрах хүрээний шугамд дор хаяж гурван өөр орой цэг хэрэгтэй
cmd-reference-surface-extent-string-no-longer-available = Хамрах хүрээний шугам цаашид боломжгүй байна
cmd-reference-surface-meeting-count-crossing-s = { $count } огтлолцол дээр золгож байна
cmd-reference-surface-and-more = , … мөн дахин { $more }
cmd-reference-surface-no-mask-selected-surface-outline = Маск сонгоогүй; гадаргууг цэгүүдийн контурт { $buffer } м нэмсэн хэмжээгээр огтолсон
cmd-reference-surface-no-mask-selected-surface-unclipped = Маск сонгоогүй; гадаргууг огтлоогүй
cmd-reference-surface-no-part-surface-falls-inside = Гадаргууны ямар ч хэсэг хамрах хүрээнд оромгүй байна
cmd-reference-surface-open-project-before-building-surface = Гадаргуу бүтээхийн өмнө төсөл нээнэ үү
cmd-reference-surface-points-collinear-plan-surface-needs = Цэгүүд төлөвлөгөөн дээр нэг шулуун дээр байна; гадаргууд тэгш шулуун биш гурван цэг хэрэгтэй
cmd-reference-surface-select-exactly-one-closed-string = Гадаргууг огтлох яг нэг хаалттай шугам сонгоно уу
cmd-reference-surface-selected-point-has-non-finite = Сонгосон цэг төгсгөлөг биш координаттай байна
cmd-reference-surface-selected-points-span-count-layers = Сонгосон цэгүүд { $count } давхаргад хамаарч байна; гадаргууг { $section }-д байрлуулна
cmd-reference-surface-control-string-index-has-two = { $index } удирдах шугамд төлөвлөгөөн дээр ({ $x }, { $y })-аас { $distance } м дотор, өөр өндөр дээр байгаа хоёр орой цэг байна
cmd-reference-surface-run-record-used-point = Ажиллуулалтын бүртгэл: өгсөн { $picks } цэгээс { $used } цэгийг ашигласан, { $merged } нэгтгэсэн, удирдах шугамын доорх { $left_out } орхигдуулсан ({ $overridden } нь өөр өндөртэй); { $method }, { $spacing } м зайтай; { $author }, { $date }
cmd-reference-surface-count-pair-s-points-closer = Төлөвлөгөөн дээр { $spacing } м-ээс ойр байрлах { $count } хос цэг { $degrees } градусаас эгц байна; тор тэдгээрийг долгиололгүйгээр дагаж чадахгүй:
cmd-reference-surface-steep-pair = ({ $ax }, { $ay }, { $az }) ба ({ $bx }, { $by }, { $bz }): { $distance } м зайтай, { $rise } м өндрийн зөрүүтэй, { $slope } градус
cmd-reference-surface-surface-could-not-cut = ({ $x }, { $y }) орчимд гадаргууг хамрах хүрээний дагуу огтолж чадсангүй
cmd-relimit-click-missed = Хязгаарлах: товшилт ямар ч объектод тусаагүй (заагчийн доор юу ч байхгүй)
cmd-relimit-click-ignored = Хязгаарлах: товшилтыг үл хэрэгсэв, багаж одоогоор бай сонгохыг хүлээгээгүй байна
cmd-relimit-clicked-source-line = Хязгаарлах: та эх шугам дээрээ л товшлоо, өөр шугам сонгоно уу
cmd-relimit-no-source-line = Хязгаарлах: эх шугам тохируулаагүй байна, сонголтыг таслав
cmd-relimit-relimited-line-source-id-selected = { $source_id } шугамыг сонгосон бай руу хязгаарлав
cmd-relimit-resized-line-source-id-using = { $source_id } шугамыг { $mode } горимоор { $value } утга ашиглан хэмжээг өөрчлөв
cmd-rename-item-no-longer-belongs-active = Тэр зүйл цаашид идэвхтэй төсөлд харьяалагдахгүй байна
cmd-rename-renamed-before-name = '{ $before }'-г '{ $name }' болгож нэрлэв
cmd-rename-renamed-name-taken = '{ $before }'-г '{ $name }' болгож нэрлэв ('{ $requested }' нэр аль хэдийн эзэлгдсэн байна)
cmd-rotate-collar-turned-count-drillhole-collar-s = { $count } цооногийн амсрыг эргүүлэв ({ $rotation })
cmd-section-verb-count-item-s-section = { $section }-д { $count } зүйлийг { $verb }
cmd-selection-delete-vertex = Орой цэгийг устгах
cmd-selection-deleted-count-selected-object-s = Сонгосон { $count } объектыг устгалаа
cmd-selection-deleted-vertex = { $object_id } полилиниагаас { $vertex } орой цэгийг устгалаа
cmd-strat-check-checking = { $name }-ийн страт баганыг тулгаж байна
cmd-strat-check-failed = Страт баганын тулгалт амжилтгүй боллоо: { $error }
cmd-strat-check-summary = { $name }-ийн { $field }-г тулгалаа: { $holes } цооног, { $flagged } тэмдэглэсэн
cmd-strat-check-too-many-codes = { $name }-ийн { $field } дараалалд оруулахад хэт олон кодтой
cmd-strat-import-filled = { $field }-ийн страт баганыг бөглөлөө: { $names } нэр; { $flagged } цооног { $checked }-д зөрж байна. Хянахын тулд Тулгах дарна уу.
cmd-strat-import-filled-groups = { $field }-ийн страт баганыг бөглөлөө: { $groups } бүлэгт { $names } нэр; { $flagged } цооног { $checked }-д зөрж байна. Хянахын тулд Тулгах дарна уу.
cmd-string-clean-and = ба
cmd-string-clean-checks-pass = Гадаргуу бүтээх шалгалтууд { $layer } давхарга дээр давж байна
cmd-string-clean-build-would-leave-out = Гадаргуу бүтээх нь { $layer } давхаргын { $strings } шугамыг хасаж, үлдсэнээс бүтээнэ
cmd-string-clean-checks-refuse = Гадаргуу бүтээх шалгалтууд { $layer } давхаргаас одоо ч татгалзаж байна: { $count } байршлыг цагирагаар тэмдэглэв
cmd-string-clean-clean-strings = Шугамуудыг цэвэрлэх
cmd-string-clean-clean-this-string = Энэ шугамыг цэвэрлэх
cmd-string-clean-cleaning-strings = Шугамыг цэвэрлэж байна
cmd-string-clean-hand-along = Гараар засах: { $strings } шугамууд ({ $x }, { $y }) дээр бие биеийнхээ дагуу явж байна
cmd-string-clean-hand-build-refuses = Гараар засах: Гадаргуу бүтээх дээр нэрлээгүй шугамуудаас одоо ч татгалзаж байна: { $refusal }
cmd-string-clean-hand-crosses-itself = Гараар засах: { $string } шугам ({ $x }, { $y }) дээр өөрөө өөртэйгөө огтлолцож байна
cmd-string-clean-hand-crossing = Гараар засах: { $strings } шугамууд ({ $x }, { $y }) дээр { $miss } м зөрж байна
cmd-string-clean-hand-ends-where-it-starts = Гараар засах: { $string } шугам ({ $x }, { $y }) дээр эхэлсэн газраа дуусч байна
cmd-string-clean-hand-near-miss = Гараар засах: { $strings } шугамууд уулзалгүй ойрхон өнгөрч, { $miss } м зөрж байна, ({ $x }, { $y })
cmd-string-clean-hand-points-disagree = Гараар засах: { $string } шугам төлөвлөгөөн дээрх нэг газар хоёр цэгтэй, өндрөөрөө { $miss } м зөрүүтэй, ({ $x }, { $y })
cmd-string-clean-hand-too-short = Гараар засах: { $string } шугам хоёроос цөөн ялгаатай орой цэгтэй, ({ $x }, { $y })
cmd-string-clean-hand-turns-back = Гараар засах: { $string } шугам ({ $x }, { $y }) дээр эргэж буцаж байна
cmd-string-clean-height-dropped = { $string } шугам: хөршөөсөө { $offset } м зөрсөн өндрийг ({ $x }, { $y }, { $z }) дээр устгав
cmd-string-clean-join-all-at-halfway = Бүгдийг дундаж өндөрт холбох
cmd-string-clean-clear-rings = Цагирагуудыг арилгах
cmd-string-clean-join-all-crossing = Бүгдийг дундаж өндөрт холбоход: { $strings } шугамууд ({ $x }, { $y }) дээр { $miss } м зөрж байна
cmd-string-clean-join-here-at-halfway = Энд дундаж өндөрт холбох
cmd-string-clean-joining-strings = Шугамыг дундаж өндөрт холбож байна
cmd-string-clean-joined = { $strings } шугамууд: ({ $x }, { $y }) дээр { $z } дундаж өндөрт холбов, { $miss } м зөрж байсан
cmd-string-clean-layer = { $layer } давхарга: { $strings } шугам
cmd-string-clean-left-arcs = { $string } шугамыг зурсан хэвээр нь үлдээв: нумтай
cmd-string-clean-left-not-finite = { $string } шугамыг зурсан хэвээр нь үлдээв: төгсгөлгүй координаттай
cmd-string-clean-loop-cut = { $string } шугам: өөрөө өөртэйгөө огтлолцсон газар { $count } оройтой гогцоог тайрч авав, ({ $x }, { $y }, { $z })
cmd-string-clean-nothing-to-clean = Сонгосон шугамуудад цэвэрлэх зүйл алга
cmd-string-clean-odd-above-every = { $string } шугам огтлолцох шугам бүрээс { $low }-аас { $high } м дээр байна ({ $total } огтлолцлоос { $count })
cmd-string-clean-odd-above-misses = { $string } шугам { $limit } м-ээс их зөрдөг шугам бүрээс { $low }-аас { $high } м дээр байна ({ $total } огтлолцлоос { $count })
cmd-string-clean-odd-below-every = { $string } шугам огтлолцох шугам бүрээс { $low }-аас { $high } м доор байна ({ $total } огтлолцлоос { $count })
cmd-string-clean-odd-below-misses = { $string } шугам { $limit } м-ээс их зөрдөг шугам бүрээс { $low }-аас { $high } м доор байна ({ $total } огтлолцлоос { $count })
cmd-string-clean-removed = { $string } шугам: хасав, бүх уртаараа { $kept } шугамын дагуу явж байсан
cmd-string-clean-repeats-merged = { $string } шугам: давтагдсан { $count } цэгийг ({ $x }, { $y }, { $z }) дээр нэг болгон нэгтгэв
cmd-string-clean-retrace-dropped = { $string } шугам: шугам дээгүүр буцаж явсан { $count } оройг тайрав, ({ $x }, { $y }, { $z })
cmd-string-clean-ring-title = { $strings } шугамууд
cmd-string-clean-ring-title-miss = { $strings } шугамууд, { $miss } м зөрүүтэй
cmd-string-clean-rings = Цагирагтай шугамууд
cmd-string-clean-run-finished = { $label }: дууслаа, { $edits } засвар, { $rings } байршлыг цагирагаар тэмдэглэв
cmd-string-clean-run-started = { $label }: { $layers } давхарга дээрх { $strings } шугам
cmd-string-clean-shared-cut = { $string } шугам: { $kept } шугамтай хуваалцсан { $length } м-ийг тайрав, ({ $x }, { $y }, { $z })
cmd-string-clean-spike-dropped = { $string } шугам: ({ $x }, { $y }, { $z }) дээрх шовхыг устгав
cmd-string-clean-vertex-shared = { $strings } шугамууд: ({ $x }, { $y }) дээр нийтлэг орой оруулав, { $miss } м зөрж байна
cmd-string-clean-zero-dropped = { $string } шугам: ({ $x }, { $y }) дээр z = 0 орой цэгийг устгав
cmd-selection-duplicate-selection = Сонголтыг хувилах
cmd-selection-duplicated-count-object-s = { $count } объектыг хувилав
cmd-seam-surface-clash = { $first } ({ $first_thickness } м) ба { $second } ({ $second_thickness } м)
cmd-seam-surface-clash-heading = Зузааны цэгийн { $count } хос нэг байрлалд өөр өөр зузаантай байна; нарийн гадаргуу хоёуланг нь дайрч чадахгүй:
cmd-seam-surface-failed = Зузааны гадаргуу амжилтгүй: { $error }
cmd-seam-surface-made = { $name } үүсгэв: { $used } зузааны цэгээс { $spacing } м алхамтай { $nodes } зангилаа, { $merged } нэгтгэсэн, { $held } зангилааг тэг зузаанд барьсан; лавлах гадаргуу { $surface }, зузааны цэгүүд { $run }
cmd-cuts-to-surface-select-seam = Тайрах давхаргын дээврийн хэсэг ба улыг нэг тор дээрх хоёр торон гадаргуу болгон сонгоно уу
cmd-cuts-to-surface-not-one-lattice = Дээврийн хэсэг ба ул нэг торыг хуваалцахгүй байна: нэг тор дээр бүтээсэн давхаргын дээврийн хэсэг ба улыг сонгоно уу
cmd-cuts-to-surface-nothing-left = Хязгааруудын хооронд давхаргаас юу ч үлдээгүй тул юу ч үүсгээгүй
cmd-cuts-to-surface-seam = { $roof } ба { $floor }
cmd-cuts-to-surface-solid = Биет
cmd-cuts-to-surface-no-cut = Доор үлдээх, Дээр үлдээх эсвэл хоёуланг нь сонгоно уу
cmd-cuts-to-surface-cuts-itself = Тайрагдаж буй гадаргуу өөрийнхөө хязгаар байж болохгүй
cmd-cuts-to-surface-no-memory = Тайрсан гадаргууд санах ой хүрэлцэхгүй байна
cmd-cuts-to-surface-cutting = Гадаргууг тайрч байна
cmd-cuts-to-surface-upper = { $name }-ын доор үлдээх
cmd-cuts-to-surface-upper-level = RL { $level }-ын доор үлдээх
cmd-cuts-to-surface-lower = { $name }-ын дээр үлдээх
cmd-cuts-to-surface-lower-level = RL { $level }-ын дээр үлдээх
cmd-cuts-to-surface-lower-depth = { $name }-аас { $depth } м доорхын дээр үлдээх
cmd-cuts-to-surface-made = { $surface }-аас { $roof }, { $floor }, { $solid }-ыг үүсгэв: { $nodes } зангилаанаас { $upper } дээр дээврийн хэсгийг Доор үлдээх дээр тэгшлэн тавьсан, { $lower } дээр улыг Дээр үлдээх дээр тэгшлэн тавьсан, { $removed } дээврийн хэсэг ба ул хоёулаа гадна байсан тул хассан, { $crossed } дээр Доор үлдээх нь Дээр үлдээхээс доор байна, { $uncovered } доороо хязгааргүй; биет { $volume } м3; хязгаарууд: { $cuts }
cmd-cuts-to-surface-not-cut = { $surface }-ыг тайраагүй: бүх зангилаа аль хэдийн { $cuts } дотор байгаа тул гадаргуу үүсгээгүй
cmd-cuts-to-surface-uncovered = { $surface }: { $count } зангилааны доор хязгаарын гадаргуу байхгүй тул хэвээр нь үлдээв
cmd-seam-surface-held-edge = Зузааны цэгүүдийн контураас { $reach } м-ээс хол орших { $count } зангилаа тэнд хүрсэн зузааныг хадгалсан
cmd-seam-surface-making = Зузааны гадаргуу үүсгэж байна
cmd-seam-surface-name = { $seam } { $side }
cmd-seam-surface-points-layer = { $seam } { $side } цэгүүд
cmd-seam-surface-no-memory = Зузааны торонд санах ой хүрэлцэхгүй
cmd-seam-surface-no-run = { $name }-д зузааны цэг алга: эхлээд түүнд зузааны цэгүүд үүсгэнэ үү
cmd-seam-surface-run = { $name }, { $count } цэг
cmd-seam-surface-stale-run = { $name }-ийн зузааны цэгүүдийг үүсгэсний дараа дахин бүтээсэн: зузааны цэгүүдийг дахин үүсгэнэ үү
cmd-seam-surface-too-few-points = { $count } зузааны цэг; зузааны гадаргууд дор хаяж { $minimum } хэрэгтэй
cmd-session-created-triangulation = '{ $name }' триангуляцыг үүсгэлээ ({ $vertex_count } орой цэг, { $face_count } тал), гадаргуугийн төрөл { $surface_type }
cmd-session-deleted-triangulation = '{ $name }' триангуляцыг төслөөс устгалаа
cmd-session-failed-load-triangulation-error = Триангуляцыг ачаалж чадсангүй: { $error }
cmd-session-failed-load-triangulation-message = Триангуляцыг ачаалж чадсангүй: { $message }
cmd-session-loaded-triangulation = '{ $name }' триангуляцыг ачааллаа ({ $path }, { $vertex_count } орой цэг, { $face_count } тал)
cmd-session-set-triangulation-tri-id-color = { $tri_id } триангуляцын өнгийг { $color } болгож тохируулав
cmd-session-triangulation-load-no-result = { $path }-н триангуляц ачаалалт үр дүнгүй дуусав
cmd-session-triangulation-failed = Триангуляцын үйлдэл амжилтгүй боллоо: { $message }
cmd-session-unloaded-triangulation-name = '{ $name }' триангуляцыг буулгалаа
cmd-slice-entered-slice-view-cx-cy = Огтлолын харагдац руу орлоо @ { $cx }, { $cy }, { $cz }, чиглэл { $dx }, { $dy } ({ $length }м шугам)
cmd-slice-exited-slice-view = Огтлолын харагдацаас гарлаа
cmd-slice-reset-section-view-fit-extents = Огтлолын харагдацыг сэргээх (хэмжээнд тааруулах)
cmd-slice-set-section-grid-enabled = Огтлолын торыг тохируулав = { $enabled }
cmd-split-created-2-open-polylines = 2 задгай полилиниа үүсгэлээ
cmd-split-line = Шугамыг хуваах
cmd-split-points-needs-interior-vertex = Цэгээр хуваах: задгай шугамын дотоод орой цэгийг сонгоно уу
cmd-split-polyline-into-two = Эх полилиниаг хоёр задгай полилиниа болгож хуваав
cmd-text-edit-finished = { $object_id } объектын текст засварыг дуусгалаа
cmd-text-updated = { $object_id } объект дээрх текстийг шинэчиллээ
cmd-thickness-not-a-grid = { $name }-ийг лавлагаа болгон хэмжих боломжгүй: { $reason }
cmd-thickness-not-a-grid-cells = энэ нь Гадаргуу бүтээх-ийн үүсгэдэг шиг дөрвөлжин нүднүүдийн нэг жигд тор биш
cmd-thickness-not-a-grid-heights = түүний хоёр орой нэг торын зангилааг өөр өндөрт хуваалцаж байна
cmd-thickness-not-a-grid-large = түүний тор { $budget } зангилааны хязгаарыг давна
cmd-thickness-points-and-more = мөн өөр { $more }
cmd-thickness-points-checking-grid = Гадаргууг шалгаж байна
cmd-thickness-points-column-clash = { $dataset } өгөгдөлтэй хамт ирсэн "{ $column }" баганатай тул түүнд зузаан хадгалсангүй. Цэгүүдийг ямар ч байсан үүсгэсэн.
cmd-thickness-points-dialog-closed = Файл сонгохоос өмнө зузааны цэгийн цонх хаагдсан
cmd-thickness-points-failed = Зузааны цэгүүд амжилтгүй: { $error }
cmd-thickness-points-layer = { $seam } зузааны цэгүүд
cmd-thickness-points-left-out-heading = Орхигдсон ({ $count }):
cmd-thickness-points-left-out-hole = цооног { $hole }: { $reason }
cmd-thickness-points-left-out-measured = хэмжсэн { $id }, мөр { $line }: { $reason }
cmd-thickness-points-made = Зузааны цэгүүд { $name }: цооногоос { $holes }, хэмжсэн { $measured }, орхигдсон { $left_out }, давхаргагүй { $without } цооног; { $surface }-ийг лавлагаа болгон хэмжсэн
cmd-thickness-points-making = Зузааны цэгүүд үүсгэж байна
cmd-thickness-points-no-layer = давхарга алга
cmd-thickness-points-open-project = Зузааны цэг үүсгэхээс өмнө төсөл нээнэ үү
cmd-thickness-points-pairs-filter = Хэмжсэн хосын CSV
cmd-thickness-points-pairs-missing-columns = { $name }-д { $columns } багана(ууд) дутуу; хэмжсэн хосын файлд { $expected } хэрэгтэй
cmd-thickness-points-pairs-not-csv = { $name } уншигдах CSV биш: { $error }
cmd-thickness-points-pairs-not-read = { $name }-ийг уншиж чадсангүй
cmd-thickness-points-pairs-unreadable = Хэмжсэн хосын файлыг уншиж чадсангүй: { $error }
cmd-thickness-points-project-changed = Зузааны цэг үүсгэх үед төсөл өөрчлөгдсөн; юу ч нэмээгүй
cmd-thickness-points-reason-missing-value = дээд эсвэл доод хучааны координат хоосон эсвэл тоо биш
cmd-thickness-points-reason-no-floor = доод хучаа алга
cmd-thickness-points-reason-no-trace = байрлуулах цооногийн зам алга
cmd-thickness-points-reason-outside = лавлах гадаргуугаас гадна
cmd-thickness-points-reason-overturned = хөмөрсөн: хамрах хүрээнээс гадна
cmd-thickness-points-saved = { $dataset }-ийн "{ $column }" баганад жинхэнэ зузааны { $count } утгыг дээд хучааны интервал бүрт хадгалсан
cmd-thickness-points-saved-cleared = Энэ удаа орхигдсон цооногуудын өмнөх { $count } утгыг арилгасан
cmd-thickness-points-saved-replaced = Өмнөх ажиллуулалтын { $count } утгыг сольсон
cmd-thickness-points-saved-unchanged = { $dataset }-ийн "{ $column }" баганад эдгээр утга аль хэдийн байна
cmd-thickness-points-surface-gone = Сонгосон гадаргуу ачаалагдаагүй болсон
cmd-thickness-points-select-one-surface = Лавлагаа болгох нэг гадаргуу сонгоно уу ({ $count } сонгогдсон)
cmd-view-centre-rotation-not-available-flying = Нисэх горимд эргэлтийн төв боломжгүй байна
cmd-view-fixed-centre-rotation-x-y = Эргэлтийн төвийг { $x }, { $y }, { $z }-д тогтоов
cmd-view-no-point-under-cursor-fix = Заагчийн доор эргэлтийн төвийг тогтоох цэг алга
cmd-view-released-centre-rotation = Эргэлтийн төвийг суллалаа
cmd-view-reset-view-fit-extents = Харагдацыг сэргээх (хэмжээнд тааруулах)
cmd-view-reset-view-plan-same-distance = Харагдацыг сэргээх (ижил зайнаас дээрээс харах; хэмжээнд тааруулахын тулд дахин дарна уу)
cmd-view-set-cinematic-view-enabled = Кино харагдацыг тохируулав = { $enabled }
cmd-view-set-topology-wireframes-enabled = Топологийн торон дүрсийг тохируулав = { $enabled }
cmd-view-set-view-points-enabled = Цэгийн харагдацыг тохируулав = { $enabled }
cmd-view-set-xy-grid-enabled = XY торыг тохируулав = { $enabled }
cmd-view-zoom-extents-preserving-angle = Хэмжээнд тааруулж томруулах (өнцгийг хадгалах)

## Common strings

common-add-product = Бүтээгдэхүүн нэмэх
common-appearance = Харагдац...
common-background = Дэвсгэр
common-block-model = Блокийн загвар
common-block-models = Блокийн загварууд
common-borehole-inspector = Цооногийн шалгагч
common-build-surface = Гадаргуу бүтээх
common-build-surface-ellipsis = Гадаргуу бүтээх...
common-cancelled = Цуцлагдсан
common-chamfer = Фаск
common-choose = Сонгох...
common-circle = Тойрог
common-classify = Ангилах
common-classify-point-clouds = Цэгэн үүлийг ангилах
common-click-point-fix-centre-rotation = Эргэлтийн төвийг тогтооход цэг дээр товшино уу
common-clip-surface-polyline = Гадаргууг полилиниагаар огтлох...
common-closed = Хаалттай
common-collection = Цуглуулга
common-colour = Өнгө
common-confirm-omf-rewrite = OMF дахин бичихийг батлах
common-could-not-replace-current-project = Одоогийн төслийг солиж чадсангүй: { $error }
common-count-object-s = { $count } объект
common-create = Үүсгэх
common-create-batter-berm = Уступ-берм үүсгэх
common-create-bezier-curve = Безье муруй үүсгэх
common-create-block-model = Блокийн загвар үүсгэх
common-create-block-model-ellipsis = Блокийн загвар үүсгэх...
common-create-circle = Тойрог үүсгэх
common-create-drill-pattern = Өрмийн сүлжээ үүсгэх
common-create-layer = Давхарга үүсгэх
common-create-line = Шугам үүсгэх
common-create-ore-triangulation = Хүдрийн триангуляц үүсгэх
common-create-ore-triangulation-ellipsis = Хүдрийн триангуляц үүсгэх...
common-create-point = Цэг үүсгэх
common-create-polyline = Полилиниа үүсгэх
common-create-triangulation = Триангуляц үүсгэх...
common-crosses = Хөндлөвч тэмдэг
common-cut = Тайрах
common-cut-topology-pit-shell = Топологийг карьерийн бүрхүүлээр тайрах...
common-delete-collection = Цуглуулгыг устгах
common-delete-layer = Давхарга устгах
common-delete-product = Бүтээгдэхүүн устгах
common-delete-selection = Сонголтыг устгах
common-designs = Зураг төслүүд
common-discard-layer-changes = Давхаргын өөрчлөлтийг үл хэрэгсэх
common-down = Доош
common-drape-topology = Топологи дээр дараах
common-easting = Зүүн тийш (X)
common-edit-object = Объект засах
common-edit-text = Текст засах
common-elevation = Өндөрлөг
common-exit-without-saving = Хадгалахгүйгээр гарах
common-export-engineering-drawing = Инженерийн зургийг экспортлох
common-file-was-left-out-downhole = { $file }-г цооногийн геофизикээс хассан: { $error }
common-filter = Шүүлтүүр
common-fly-mode = Нисэх горим
common-generate-contour-lines = Изолиниа үүсгэх...
common-hide-all = Бүгдийг нуух
common-hide-selection = Сонголтыг нуух
common-unhide-all = Нуусныг бүгдийг харуулах
common-hole-id = Цооногийн ID
common-ignore = Үл хэрэгсэх
common-import-csv-block-model = CSV блокийн загвар импортлох
common-import-dxf = DXF импортлох
common-incline-design-project = Incline Design төсөл
common-join = Нэгтгэх...
common-join-point-clouds = Цэгэн үүлийг нэгтгэх
common-joined-cloud = Нэгтгэсэн үүл
common-layer = Давхарга
common-legend = Тайлбар
common-line = Шугам
common-line-weight = Шугамын зузаан
common-link-geophysics = Геофизик холбох...
common-load-drillholes-before-linking-geophysics = Геофизик холбохын өмнө цооногийн өгөгдлийн багцыг ачаална уу
common-lock-all = Бүгдийг түгжих
common-lock-selection = Сонголтыг түгжих
common-m = м
common-max = Дээд
common-merge-shell-into-topology = Бүрхүүлийг топологид нэгтгэх
common-merge-shell-into-topology-ellipsis = Бүрхүүлийг топологид нэгтгэх...
common-modelling = Загварчлал
common-move-collar = Амсрыг шилжүүлэх
common-move-collection = Цуглуулга руу зөөх
common-move-design = Объектыг шилжүүлэх
common-move-selection = Сонголтыг шилжүүлэх
common-name-has-no-readable-size = { $name }-н хэмжээг унших боломжгүй
common-new-product = Шинэ бүтээгдэхүүн
common-no-block-models = Блокийн загвар алга
common-no-design-layers = Зураг төслийн давхарга алга
common-no-drill-holes = Цооног алга
common-no-file-chosen = Файл сонгоогүй
common-no-open-project = Нээлттэй төсөл алга
common-no-point-clouds = Цэгэн үүл алга
common-no-triangulations = Триангуляц алга
common-none = Байхгүй
common-northing = Хойд тийш (Y)
common-offset = Шилжилт
common-ok = OK
common-open = Нээх
common-orientation = Чиглэл
common-point = Цэг
common-point-cloud = Цэгэн үүл
common-point-clouds = Цэгэн үүлүүд
common-polyline = Полилиниа
common-polyline-layer = '{ $layer }' дээрх полилиниа
common-project = Төсөл
common-rasters = Растерууд
common-redo = Дахин хийх
common-reference-points = Лавлах цэгүүд...
common-relimit-line = Шугамыг хязгаарлах
common-remove-project = Төсөл устгах
common-reset-view = Харагдацыг сэргээх
common-reveal-all = Бүгдийг харуулах
common-reveal-finder = Finder-т харуулах
common-rotate-collar = Амсрыг эргүүлэх
common-save-exit = Хадгалаад гарах
common-scale-bar = Масштабын мөр
common-set-initiation-point = Дэлбэлгээ эхлүүлэх цэгийг тохируулах
common-shape = Хэлбэр
common-shell = Бүрхүүлтэй
common-slashes = Ташуу зураас
common-slice = Огтлол
common-slice-triangulation-z-range = Триангуляцыг Z мужаар огтлох...
common-surface-contours = Гадаргуугийн изолиниа
common-text = Текст
common-degree-suffix = °
common-tie-holes = Цооногуудыг холбох
common-thickness-points = Зузааны цэгүүд
common-thickness-points-ellipsis = Зузааны цэгүүд...
common-thickness-surfaces = Зузааны гадаргуунууд
common-thickness-surfaces-ellipsis = Зузааны гадаргуунууд...
common-clip-to-surface-ellipsis = Гадаргуугаар тайрах...
common-triangulations = Триангуляцууд
common-trim-topology = Топологиор тайрах...
common-undo = Буцаах
common-undrape-all = Бүгдийг дараахаас цуцлах
common-uniform-white = Жигд цагаан
common-unknown = Тодорхойгүй
common-unlock-all = Бүгдийн түгжээг тайлах
common-untitled = Нэргүй
common-up = Дээш
common-vertical-exaggeration = Босоо хэтрүүлэлт
common-x = x
common-zoom-extents = Хэмжээнд тааруулж томруулах

## Confirmations strings

confirmations-close-project-unsaved-changes = Төслийг хаах: Хадгалаагүй өөрчлөлт
confirmations-close-without-saving = Хадгалахгүйгээр хаах
confirmations-delete = Устгах
confirmations-delete-objects = Объект устгах
confirmations-discard = Үл хэрэгсэх
confirmations-discard-all-unsaved-changes-layer =
    '{ $name }' давхаргын хадгалаагүй бүх өөрчлөлтийг үл хэрэгсэх үү?
    Хадгалсан давхарга дискнээс дахин ачаалагдах бол бусад давхаргын өөрчлөлт хэвээр хадгалагдана. Энэ үйлдлийг буцаах боломжгүй.
confirmations-discard-all-unsaved-changes-name =
    '{ $name }'-н хадгалаагүй бүх өөрчлөлтийг үл хэрэгсэх үү?
    Сүүлд хадгалсан хувилбар дискнээс дахин ачаалагдана. Энэ үйлдлийг буцаах боломжгүй.
confirmations-discard-changes = Өөрчлөлтийг үл хэрэгсэх
confirmations-exit-unsaved-changes = Гарах: Хадгалаагүй өөрчлөлт
confirmations-incline-design-cannot-reproduce-all = Incline Design эх OMF-н бүх агуулгыг дахин үүсгэж чадахгүй. Хадгалахад дараах агуулга орхигдоно:
confirmations-product = Бүтээгдэхүүн
confirmations-project = энэ төсөл
confirmations-remove-name-delete-its-browser = '{ $name }'-г устгаад хөтчид хадгалагдсан хуулбарыг нь устгах уу? Хадгалаагүй өөрчлөлт алдагдана.
confirmations-remove-project-unsaved-changes = Төслийг устгах: Хадгалаагүй өөрчлөлт
confirmations-remove-without-saving = Хадгалахгүйгээр устгах
confirmations-replace-project-unsaved-changes = Төслийг солих: Хадгалаагүй өөрчлөлт
confirmations-save = Хадгалах
confirmations-save-anyway = Тэй тэй хадгалах
confirmations-save-changes-current-project-before = Одоогийн төслийг солихоос өмнө өөрчлөлтийг хадгалах уу?
confirmations-save-changes-name-before-closing = '{ $name }'-г хаахаас өмнө өөрчлөлтийг хадгалах уу?
confirmations-save-changes-name-before-removing = Incline Design-с устгахаас өмнө '{ $name }'-н өөрчлөлтийг хадгалах уу?
confirmations-save-close = Хадгалаад хаах
confirmations-save-modified-project-before-exiting = Гарахаас өмнө өөрчилсөн төслийг хадгалах уу?
confirmations-save-to-browser-before-exit = Гарахаас өмнө өөрчилсөн төслийг хөтчийн санах ойд хадгалах уу?
confirmations-save-remove = Хадгалаад устгах

## Console strings

console-copy-all = Бүгдийг хуулах
console-copy-message = Мессежийг хуулах
console-error = АЛДАА
console-info = МЭДЭЭЛЭЛ
console-no-console-activity-yet = Одоогоор консолын үйл ажиллагаа алга
console-pending = ХҮЛЭЭГДЭЖ БАЙНА
console-progress-summary = Явагдаж байна · { $summary }
console-success = АМЖИЛТТАЙ
console-warn = АНХААРУУЛГА

## Csv strings

csv-block-model-category = Ангилал
csv-block-model-value = Утга
csv-drill-hole-rows-for-undefined-holes = { $count } мөр багцын геометрт тодорхойлоогүй цооногийнх байв
csv-drill-hole-count-rows-were-skipped-total = Нийт { $count } мөрийг алгаслаа
csv-drill-hole-csv-file-empty = CSV файл хоосон байна
csv-drill-hole-csv-has-too-many-unreadable = CSV-д засаж болохгүй хэт олон уншигдахгүй байт байна; энэ нь хуучин кодчиллоор байж магадгүй, UTF-8 болгон хадгалаад дахин импортлоно уу
csv-drill-hole-csv-header-has-no-columns = CSV толгойд багана алга
csv-drill-hole-csv-headers-must-nonblank-unique = CSV толгойнууд хоосон биш, давтагдахгүй байх ёстой
csv-drill-hole-geophysics-needs-geometry = Цооногийн геофизикт багц дахь амсар эсвэл тодорхой сегментийн файл хэрэгтэй, цооногуудад нь тэр холбогдоно
csv-drill-hole-azimuth-out-of-range = { $file }-д азимут нь 0-с 360-ын хооронд биш { $count } мөр байна
csv-drill-hole-dip-out-of-range = { $file }-д уналт нь -90-с 90-ын хооронд биш { $count } мөр байна; тэдгээр мөрийг чиглэлгүйгээр уншсан
csv-drill-hole-file-inclination-values-could-angle = { $file }-н өнцөг байж болох хазайлтын утгууд бүгд тэгээс бага буюу тэнцүү тул баганыг уналт (сөрөг нь доош) гэж уншлаа
csv-drill-hole-file-maps-gamma-density-column = { $file } гамма эсвэл нягтын баганыг хоёр удаа харгалзуулсан байна
csv-drill-hole-invalid-utf8 = { $file } нь зөв UTF-8 биш; { $cells } нүдэн дэх { $count } уншигдахгүй байтыг сольсон; гэмтсэн нүдийг өгөгдөл гэж уншихгүй
csv-drill-hole-file-requires-gamma-density-column = { $file }-д гамма эсвэл нягтын багана шаардлагатай
csv-drill-hole-row-undefined-hole = { $file }-н { $row }-р мөр DHID '{ $dhid }'-нх бөгөөд энэ цооногийг багцын геометр тодорхойлоогүй байна
csv-drill-hole-holes-hole-s-carry-overlapping = { $holes } цооногт давхцсан интервал байна, жишээ нь хуваагдлуудтайгаа хамт бүртгэсэн давхарга: { $summary }
csv-drill-hole-skipped-row-reason = Мөрийг алгаслаа: { $reason }
csv-drill-hole-row-attribute-not-number = { $file } мөр { $row }: тоон баганад '{ $value }' байна
csv-drill-hole-row-repeats-dhid = { $file } мөр { $row }: DHID '{ $dhid }' давтагдсан
csv-drill-hole-most-rows-unreadable = { $file }: { $count } мөрөөс { $skipped }-г уншиж чадсангүй; шалтгааныг консолоос харна уу
csv-drill-hole-file-maps-dip-column-twice = { $file } уналт эсвэл налуугийн баганыг хоёр удаа оноосон
csv-drill-hole-row-has-no-geometry = { $file } мөр { $row }: бүрэн XYZ эсвэл азимут/уналтын геометр байхгүй
csv-drill-hole-row-invalid-interval = { $file } мөр { $row }: DHID '{ $dhid }'-д { $from }..{ $to } интервал буруу
csv-drill-hole-row-zero-length-segment = { $file } мөр { $row }: DHID '{ $dhid }'-д { $depth }-д тэг урттай хэсэг байна
csv-drill-hole-row-unreadable-value = { $file } мөр { $row }: уншигдахгүй утга байна
csv-drill-hole-csv-is-wide-text = CSV нь UTF-16 эсвэл UTF-32 текст байна; UTF-8 болгон хадгалаад дахин импортлоно уу
csv-drill-hole-csv-holds-nul-bytes = CSV бүхэлдээ NUL байт агуулж байгаа тул UTF-8 текст биш; UTF-16 эсвэл UTF-32-оор бичсэн бол UTF-8 болгон хадгалаад дахин импортлоно уу
csv-drill-hole-overlap-field-summary = { $field }: { $count } өрөмдлөгийн цооногт, жишээ нь { $examples }
csv-geophysics-above-5 = 5-аас дээш
csv-geophysics-below-0-5 = 0.5-аас доош
csv-geophysics-count-more = (+{ $count } илүү)
csv-geophysics-count-rows-were-skipped-total = { $file }-д нийт { $count } мөрийг алгаслаа
csv-geophysics-csv-has-record-longer-than = CSV-д { $limit } MiB-ээс урт бичлэг байна: файлд CSV-д байх ёстой мөрийн таслал байхгүй эсвэл текст биш байна
csv-geophysics-csv-has-unterminated-quoted-field = CSV-д хаагдаагүй хашилттай талбар байна
csv-geophysics-curve-file-was-left-out = { $file } дахь { $curve }-г хассан: заалтуудынх нь ихэнх нь { $side } тул медиан нь 0.5-аас 5 g/cc-ийн гадна байна, нэгж нь буруу юм шиг харагдаж байна (g/cc хүлээгдэж байсан). Incline нэгж хөрвүүлэхгүй; экспортыг засаад дахин холбоно уу
csv-geophysics-file-empty = { $file } хоосон байна
csv-geophysics-file-has-no-curve-no = { $file }-д муруй алга: цооногийн ID болон гүнээс бусад ямар ч баганад тоо байхгүй байна
csv-geophysics-file-mapping-has-mapped-columns = { $file }-н харгалзуулалтад { $mapped } багана байна, CSV-д { $found }
csv-geophysics-file-no-longer-matches-its = { $file } индекстэйгээ цаашид таарахгүй байна: дахин холбоно уу
csv-geophysics-file-not-grouped-hole-its = { $file } цооногоор бүлэглэгдээгүй: цооногуудын мөрүүд хэт олон хэсэгт хуваагдсан байна. Цооногийн ID, дараа нь гүнээр эрэмбэлээд дахин холбоно уу
csv-geophysics-file-requires-one-dhid-one = { $file }-д нэг DHID болон нэг гүнгийн багана шаардлагатай
csv-geophysics-row-blank-hole-id = { $file }-н { $row }-р мөрийн цооногийн ID хоосон байна
csv-geophysics-row-column-count = { $file }-н { $row }-р мөрөнд { $found } багана байна; { $expected } байх ёстой
csv-geophysics-row-negative-depth = { $file }-н { $row }-р мөрийн гүн сөрөг байна
csv-geophysics-row-no-depth = { $file }-н { $row }-р мөрийн гүнг унших боломжгүй
csv-geophysics-file-s-path-not-valid = файлын зам нь зөв UTF-8 биш бөгөөд төсөл үүнийг хадгалж чадахгүй: файл эсвэл хавтасны нэрийг өөрчлөөд дахин холбоно уу
csv-geophysics-rows-skipped = { $file }: { $rows } мөрийн { $skipped }-г унших боломжгүй байв; шалтгааныг консолоос харна уу
csv-geophysics-runs-not-grouped = { $count } цооногийн геофизик нэгээс олон хэсгээр орж ирсэн, цооногоор бүлэглэгдээгүй; дараагийн хэсэг бүр зөвхөн цооногт нь заалтгүй гүнийг нэмнэ: { $holes }
csv-geophysics-linked-downhole-geophysics-from-file = { $file }-с цооногийн геофизик холбогдлоо: { $holes } цооног, муруй { $curves }; { $rows } мөр уншсан, { $skipped } алгассан. Заалтууд файлд үлдэх бөгөөд нэг удаад нэг цооногоор уншина
csv-geophysics-no-readings = заалт алга
csv-geophysics-no-usable-depth-step = ашиглах боломжтой гүнгийн алхам алга
csv-geophysics-run-count-mismatch = { $hole }-н { $read } хэсгийг уншсан, холбоост { $runs } байна
csv-geophysics-rows-geophysics-row-s-count = Өгөгдлийн багц тодорхойлоогүй { $count } цооногийн { $rows } геофизикийн мөр холбогдоогүй: { $holes }
csv-geophysics-rows-readings-would-need-samples = { $rows } заалтад { $samples } түүвэр хэрэгтэй болно
csv-geophysics-run-hole-curve-was-not = { $hole } { $curve }-н нэг хэсгийг хадгалсангүй ({ $reason })
data-table-copy-selection = Сонголтыг хуулах
data-table-copy-table = Хүснэгтийг хуулах
drill-hole-add = Нэмэх
drill-hole-add-all = Бүгдийг нэмэх

## Drill strings

drill-hole-add-stop = Зогсоол нэмэх
drill-hole-add-working-section = Ажлын үе нэмэх
drill-hole-all-rendered-intervals-opaque-white = Бүх дүрслэгдсэн интервал бүрэн цагаан өнгөтэй.
drill-hole-another-working-section-field-has = Энэ талбарын өөр ажлын үе энэ нэртэй байна.
drill-hole-assumed = Таамагласан
drill-hole-burden-spacing-must-greater-than = Бурден ба зай тэгээс их байх ёстой
drill-hole-cache-drill-hole-set-name-has = { $name } цооногийн багцад { $count } цооног ба холболт байгаа нь сонголтын тодруулга дэмжих { $capacity }-аас хэтэрсэн: багцыг бүхэлд нь сонговол тодрох боловч ганц цооногийг сонговол тодрохгүй
drill-hole-cache-drill-hole-set-name-stations = { $name } цооногийн багц: { $stations } станц, { $before } сегментийг { $after } болгон нэгтгэсэн, { $cells } нүд
drill-hole-choose-valid-closed-polyline = Зөв хаалттай полилиниа сонгоно уу
drill-hole-clear-filter = Шүүлтүүрийг цэвэрлэх
drill-hole-code-already-in-section = { $code } аль хэдийн { $section } ажлын үед байна.
drill-hole-code-outside-section-has-name = Энэ үеэс гадуурх код ийм нэртэй байна. Үе өөрийн агуулсан кодтойгоо л нэрээ хуваалцаж болно.
drill-hole-colour-scale = Өнгийн масштаб
drill-hole-count-codes = { $count } код
drill-hole-count-codes-interval-no-logged = { $count } код. Бүртгэсэн утгагүй интервал цагаан хэвээр үлдэнэ.
drill-hole-disc-diameter = Дискний диаметр
drill-hole-appearance-title = Цооногийн харагдац: { $name }
drill-hole-drilled-diameter = Өрөмдсөн диаметрийн
drill-hole-every-code-lists-already-another = Түүний жагсаасан код бүр өөр ажлын үед аль хэдийн байна.
drill-hole-every-interval-value-colour-field = Өнгөний талбарт утгатай интервал бүрийг шугам дээр энэ өргөнтэй диск болгон зурна. Холдох үед хэдэн пикселээс хэзээ ч нарийсахгүй.
drill-hole-field = Талбар
drill-hole-field-working-section = { $field } ажлын үеээр
drill-hole-floor = Шал
drill-hole-grayscale = Саарал өнгийн масштаб
drill-hole-green-yellow-red = Ногоон–Шар–Улаан
drill-hole-heat = Дулаан
drill-hole-drilled-width-help = Өрөмдсөн өргөнөөрөө зурсан цооног геологийн хажууд хоолой шиг харагдана; мянга мянган цооногийн багц дэвсгэр шиг харагдана.
drill-hole-line-width-help = Цооногийг өөрийг нь томруулалт болгонд энэ өргөнтэй шугамаар зурна.
drill-hole-however-far-eye-hole-drawn = Нүднээс хэдий хол байсан ч цооногийг дор хаяж энэ өргөнтэй зурна.
drill-hole-measured = Хэмжсэн
drill-hole-name-working-section = { $name } (ажлын үе)
drill-hole-never-thinner-than = Хэзээ ч эндээс нарийн биш
drill-hole-new-section-name = Шинэ үеийн нэр
drill-hole-new-working-section = Шинэ ажлын үе
drill-hole-no-holes-fit-inside-boundary = Одоогийн бурден ба зайгаар энэ хил дотор ямар ч цооног багтахгүй байна
drill-hole-part-code = Кодын нэг хэсэг
drill-hole-pattern-too-many-holes = Сүлжээ { $maximum } цооногийн дээд хэмжээнээс хэтэрлээ; бурден эсвэл зайг нэмэгдүүлнэ үү
drill-hole-preset = Бэлэн тохиргоо
drill-hole-px = px
drill-hole-rainbow = Солонго
drill-hole-rename-out-of-sequence-hole = { $from }-г { $to } болгон өөрчилбөл энэ цооногт страт баганын дараалалаас гарна.
drill-hole-rename-out-of-sequence-holes = { $from }-г { $to } болгон өөрчилбөл { $count } цооногт страт баганын дараалалаас гарна.
drill-hole-rename-out-of-sequence-note = Урвуу болсон эсвэл давтагдсан давхаргууд дарааллаас гардаг тул нэр өөрчлөхийг хориглоогүй. OK дарвал ямар ч тохиолдолд нэрийг өөрчилнө; Цуцлах дарвал нэр өөрчлөх рүү буцна.
drill-hole-rename-out-of-sequence-title = Дарааллаас гарсан
drill-hole-rename-seam-every-hole-of = Дараах багцын бүх цооног
drill-hole-rename-seam-hole = Цооног
drill-hole-rename-seam-holes = Цооногууд
drill-hole-rename-seam-horizon-intervals = Энэ горизонтын интервалууд
drill-hole-rename-seam-intervals = Интервалууд
drill-hole-rename-seam-logged-name-kept = Бүртгэсэн нэр хэвээр үлдэнэ; шинэ нэрийг залруулга болгон санал болгоно.
drill-hole-rename-seam-reason = Шалтгаан
drill-hole-rename-seam-reason-hint = Нэрийг яагаад өөрчилж байгаа
drill-hole-rename-seam-seam = Давхарга
drill-hole-rename-seam-title = Давхаргын нэрийг өөрчлөх
drill-hole-reset-colours = Өнгийг анхны байдалд оруулах
drill-hole-reset-preset = Тохиргоог сэргээх
drill-hole-reset-shown-colours = Харагдаж буй өнгийг анхны байдалд оруулах
drill-hole-roof = Дээврийн хэсэг
drill-hole-rotation-offsets-must-contain-valid = Эргэлт ба шилжилт зөв тоо агуулах ёстой
drill-hole-selected-polyline-has-no-usable = Сонгосон полилиниа ашиглах боломжтой XY талбайгүй байна
drill-hole-shift-names-depths-kept = Зөвхөн нэр шилжинэ, гүн хэзээ ч өөрчлөгдөхгүй. Бүртгэсэн нэрс хэвээр үлдэнэ; шинэ нэр бүрийг залруулга болгон санал болгоно.
drill-hole-shift-names-down-from-here-title = Нэрсийг эндээс доош шилжүүлэх
drill-hole-shift-names-down-title = Нэрсийг доош шилжүүлэх
drill-hole-shift-names-field = Талбар
drill-hole-shift-names-from-here-note = Дарсан горизонт болон тэр талын нэрс цооногийн дагуу нэг алхам гулсах ба нөгөө талын нэрс хэвээр үлдэнэ. Дарсан горизонтыг нэрийг нь өөрчлөх хүртэл тодорхойгүй гэсэн утгатай UNK гэж нэрлэнэ.
drill-hole-shift-names-moved = Шилжсэн нэрс
drill-hole-shift-names-not-in-column = Баганад байхгүй, хөндөөгүй
drill-hole-shift-names-reason-hint = Нэрсийг яагаад шилжүүлж байгаа
drill-hole-shift-names-submit = Шилжүүлэх
drill-hole-shift-names-unknown = UNK нэртэй
drill-hole-shift-names-unknown-note = Цооногийн нэрс цооногийн дагуу нэг алхам гулсана. Гулссан төгсгөлийн цаана баганад нэр байхгүй бол тэр хэсгийг нэрийг нь өөрчлөх хүртэл тодорхойгүй гэсэн утгатай UNK гэж нэрлэнэ: интервалууд хэвээр үлдэж, нэрийг залруулга болгон санал болгоно.
drill-hole-shift-names-up-from-here-title = Нэрсийг эндээс дээш шилжүүлэх
drill-hole-shift-names-up-title = Нэрсийг дээш шилжүүлэх
drill-hole-shown-total-codes-shown = { $total } кодын { $shown }-г харуулж байна
drill-hole-shown-total-rows-shown = { $total } мөрийн { $shown }-г харуулж байна
drill-hole-smooth-interpolation = Гөлгөр интерполяци
drill-hole-spacing-would-scan-too-many = Энэ зайгаар хэт олон торны эсийг шалгах шаардлагатай болно; бурден эсвэл зайг нэмэгдүүлнэ үү (дээд хэмжээ { $maximum } цооног)
drill-hole-square = Дөрвөлжин
drill-hole-staggered = Шаталсан
drill-hole-stepped-bands = Шаталсан зурвас
drill-hole-string-discs = Шугам ба диск
drill-hole-string-discs-where-intervals-overlap = Шугам ба диск хэлбэрээр, интервал давхцсан газар хамгийн богиныг диск болгон зурна.
drill-hole-string-width = Шугамын өргөн
drill-hole-style = Хэв маяг
drill-hole-suggested-from-code-names-count = Кодын нэрээс санал болгосон ({ $count })
common-times-sign = ×
common-minus-sign = −
drill-hole-ticked-but-hidden-filter-count = Тэмдэглэсэн боловч шүүлтүүрээр далдлагдсан: { $count }
drill-hole-true-diameter = Жинхэнэ диаметр
drill-hole-unsupported-drillhole-source = Дэмжигдээгүй цооногийн эх сурвалж
drill-hole-width = Өргөн
drill-hole-working-section-needs-name = Ажлын үед нэр хэрэгтэй.
drill-hole-working-section-set-seams-plies = Ажлын үе гэдэг нь нэг нэгж болгон олборлодог үеүүдийн багц юм. Түүгээр өнгө оноовол бүх багцад нэг өнгө өгнө.
drill-hole-working-sections = Ажлын үеүд
drill-pattern-arrangement = Байршуулалт
drill-pattern-axis-offset = { $axis } шилжилт
drill-pattern-blast-shape = Тэсэлгээний хэлбэр
drill-pattern-burden = Бурден
drill-pattern-choose-closed-blast-boundary-then = Хаалттай тэсэлгээний хилийг сонгоод, торыг тохируулна уу. Цооногууд харагдах цонхонд амьд шинэчлэгдэнэ.
drill-pattern-closed-design-polyline-whose-xy = XY проекц нь цооногоор дүүргэгдэх хаалттай зураг төслийн полилиниа.
drill-pattern-rotation-help = Глобал { $axis } тэнхлэгээс цагийн зүүний эсрэг сүлжээний эргэлт.
drill-pattern-distance-between-holes-along-each = Сүлжээний мөр бүрийн дагуух цооногуудын хоорондох зай.
drill-pattern-name-hint = ж: Баруун Огтлол 03
drill-pattern-diameter-help = Дуусгасан цооногийн диаметр. Миллиметрээр оруулж, үүсгэсэн цооног бүрт хадгалагдана.
drill-pattern-hole-depth = Цооногийн гүн
drill-pattern-hole-diameter = Цооногийн диаметр
drill-pattern-move-over-closed-polyline-then = Заагчийг хаалттай полилиниа дээгүүр аваачаад, харагдах цонхонд түүн дээр товшино уу. Esc сонголтыг цуцална.
drill-pattern-name-help = Төсөлд үүсгэгдэх цооногийн өгөгдлийн багцын нэр.
drill-pattern-none-picked = Юу ч сонгоогүй
drill-pattern-pattern-name = Сүлжээний нэр
drill-pattern-spacing-help = Сүлжээний мөрүүдийн хоорондох перпендикуляр зай.
drill-pattern-pick = Сонгох
drill-pattern-preview-count-hole-s-diameter = Урьдчилан харах: { $count } цооног · { $diameter } мм диаметр · { $depth } м гүн
drill-pattern-rotation = Эргэлт
drill-pattern-shift-pattern-grid-along-global = Сүлжээний торыг тэсэлгээний хэлбэрт огтлогдсон хэвээр глобал { $axis } тэнхлэгийн дагуу шилжүүлэх.
drill-pattern-spacing = Зай
drill-pattern-staggered-offsets-every-second-row = Шаталсан байршил хоёр дахь мөр бүрийг зайн хагасаар шилжүүлнэ.
drill-pattern-vertical-depth-below-each-collar = Амсар бүрийн доорхи босоо гүн.

## Dxf strings

dxf-block-nesting-too-deep = DXF блокийн үүрлэлт дээд гүнээс ({ $depth }) хэтэрсэн тул '{ $name }'-г алгасав
dxf-circular-block-reference = DXF тойрог хэлбэрийн блокийн лавлагаа илэрлээ: '{ $name }'
dxf-undefined-layer = DXF объект тодорхойлогдоогүй '{ $name }' давхаргыг ашигласан тул '{ $fallback }' нэрээр импортлов
dxf-import-budget-exceeded = DXF импорт { $what } төсвөөс ({ $limit }) хэтэрлээ; үлдсэн геометрийг алгасав
dxf-insert-unknown-block = DXF INSERT үл мэдэгдэх '{ $name }' блокийг ашиглаж байна

## Edit strings

edit-absolute-length = Үнэмлэхүй урт
edit-absolute-rl = Үнэмлэхүй RL
edit-action = Үйлдэл
edit-angle = Өнцөг
edit-delete-vertex-number = { $number } орой цэгийг устгах
edit-dip-help = Хэвтээ шугамаас хэмжсэн өнцөг, доош нь сөрөг: −90° нь босоо цооног.
edit-app-web-not-recommended-production = { $app } Web-г бодит ашиглалтад санал болгохгүй. Зөвхөн демо болгон ашиглана уу.
edit-application = Программ
edit-apply = Хэрэглэх
edit-apply-pick-target = Хэрэглээд бай сонгох
edit-axis-value = { $axis } утга
edit-azimuth = Азимут
edit-batter-angle = Налуугийн өнцөг (°)
edit-azimuth-help = Цооногууд өрөмдөгдөх чиглэл, торны хойд зүгээс цагийн зүүний дагуу градусаар.
edit-bench-height = Уступын өндөр
edit-benches = Уступууд
edit-berm-width = Бермийн өргөн
edit-bezier-curve = Безье муруй
edit-choose-layer = Давхарга сонгох
edit-measure-help = Оруулсан утга нь налуугийн дагуух зай, хэвтээ өргөн, эсвэл босоо өндөр эсэхийг сонгоно уу.
edit-choose-which-two-polyline-paths = Сонгосон орой цэгүүдийн хоорондох хоёр полилиниа замын алийг нь солихыг сонгоно уу. Урт нь өндөрлөг ба муруй ирмэгийг оролцуулна.
edit-click-corner-closed-polyline = Хаалттай полилиниагийн булан дээр товшино уу.
edit-click-open-closed-polyline-begin = Эхлэхийн тулд задгай эсвэл хаалттай полилиниа дээр товшино уу.
edit-click-second-vertex-replacement-span = Солих хэсгийн хоёр дахь орой цэгийг товшино уу.
edit-click-vertex-start-replacement-span = Солих хэсгийг эхлүүлэхийн тулд орой цэг дээр товшино уу.
edit-collide-triangulation = Триангуляцтай мөргөлдөх
edit-confirm-selection = Сонголтыг батлах
edit-control-point-1 = Хяналтын цэг 1
edit-control-point-2 = Хяналтын цэг 2
edit-copy = Хуулах
edit-corner-radius-limited-so-replacement = Буланг тойруулах радиус, зэргэлдээх орой цэгийг давахгүйгээр хязгаарлагдана.
edit-create-new-layer = Шинэ давхарга үүсгэх
edit-create-new-project = Шинэ төсөл үүсгэх
edit-create-project = Төсөл үүсгэх
edit-delta-length-m-use = Уртын өөрчлөлт (м, + эсвэл - ашиглана уу)
edit-dip = Уналт
edit-direction = Чиглэл
edit-distance = Зай
edit-distance-along-slope = Налуугийн дагуух зай
edit-download-free-native-version-our = Манай вэбсайтаас үнэгүй суурин хувилбарыг татаж авах ↗
edit-drill-hole = Цооног
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = Төгсгөл
edit-enter-valid-elevation = Зөв өндөрлөгийн утга оруулна уу.
edit-exit-slice = Огтлолоос гарах
edit-finish-polyline = Полилиниаг дуусгах
edit-generate-batter-berms = Уступ-берм үүсгэх
edit-height = Өндөр
edit-height-change = Өндрийн өөрчлөлт
edit-height-mode = Өндрийн горим
edit-horizontal-distance = Хэвтээ зай
edit-horizontal-width-each-flat-berm = Дараалсан налуу талуудын хоорондох тэгш бермийн хэвтээ өргөн.
edit-hover-choose-which-end-move = Аль үзүүрийг шилжүүлэхээ сонгохын тулд заагчийг байрлуулаад, батлахын тулд товшино уу.
edit-insert-point-elevation = Өндөрлөг дээр цэг оруулах
edit-intersect = Огтлолцуулах
edit-kind-properties = { $kind } { $properties }
edit-layer-name = Давхаргын нэр
edit-load-project = Төсөл ачаалах
edit-longest = Хамгийн урт
edit-m-s = м/с
edit-measure = Хэмжих
edit-mit-license = MIT лиценз
edit-mode = Горим
edit-move = Шилжүүлэх
edit-move-layer = Давхарга руу шилжүүлэх
edit-move-which-end = Аль үзүүрийг шилжүүлэх
edit-movement-speed-slice-when-using = Чиглүүлэх товчлуур ашиглах үеийн огтлолын хөдөлгөөний хурд.
edit-moving-end-endpoint = Шилжиж байна: Төгсгөлийн үзүүр
edit-moving-start-endpoint = Шилжиж байна: Эхлэлийн үзүүр
edit-new-length-m = Шинэ урт (м)
edit-new-project = Шинэ төсөл
edit-number-complete-batter-berm-levels = Бүрэн налуу-бермийн түвшний тоо. Дээд хязгаар нь заасан геометрийг хадгалах хамгийн гүн түвшнээр хязгаарлагдана.
edit-bezier-segments-help = Сонгосон хоёр орой цэгийн хоорондох муруйг ойролцоолон дүрслэхэд ашиглах шугаман сегментийн тоо.
edit-chamfer-segments-help = Дугуйрсан буланг ойролцоолон дүрслэхэд ашиглах шулуун сегментийн тоо. Шулуун фаскийн хувьд 1-ийг ашиглана уу.
edit-object = Объект
edit-offset-element = Элементийг шилжүүлэх
edit-pick-side = Талыг сонгох
edit-pit = Карьер
edit-project-name = Төслийн нэр
edit-properties = Шинж чанар
edit-radius = Радиус
edit-recent = Сүүлд
edit-relative = Харьцангуй (+/-)
edit-elevation-mode-help = «Харьцангуй» цэг бүрт босоо өөрчлөлт хэрэглэнэ. «Үнэмлэхүй RL» цэг бүрийг нэг байх өндөрлөг рүү проекцлоно.
edit-remove-from-list = Жагсаалтаас устгах
edit-replace-path = Замыг солих
edit-rotate = Эргүүлэх
edit-rotation-speed-slice-when-using = Q, E товч ашиглах үеийн огтлолын эргэлтийн хурд.
edit-s = °/с
edit-segments = Сегментүүд
edit-segments-lying-elevation-ignored = Энэ өндөрлөгт байрлах сегментүүдийг үл хэрэгсэнэ.
edit-endpoint-help = Өөрчлөгдөх үзүүрийг сонгоно уу; нөгөө үзүүр тогтмол хэвээр байна.
edit-selected-holes-point-different-ways = Сонгосон цооногууд өөр өөр чиглэж байна. Хэрэглэх нь бүгдийг эдгээр өнцөгт тохируулна.
edit-selected-start-end-point-moves = Сонгосон эхлэл эсвэл төгсгөлийн цэг шугамын чиглэлийн дагуу шилжинэ; эсрэг үзүүр тогтмол хэвээр байна.
edit-set-axis = { $axis }-ыг тохируулах
edit-shortest = Хамгийн богино
edit-show-vertex-number-in-table = { $number } орой цэгийг хүснэгтэд харуулах
edit-slice-view = Огтлолын харагдац
edit-slope-angle-each-batter-face = Хэвтээ шугамаас хэмжсэн налуу талын бүр өнцөг.
edit-slope-angle-offset-positive-negative = Шилжилтийн налуугийн өнцөг. Эерэг ба сөрөг өнцөг нь хажуу тийш шилжихдээ хуулбарыг эхээс дээш эсвэл доош шилжүүлнэ.
edit-speed = Хурд
edit-start = Эхлэл
edit-stockpile = Овоолго
edit-stop-generated-offset-where-its = Үүсгэсэн шилжилтийн зам харагдаж буй триангуляцтай анх уулзсан газар зогсоох.
edit-target-rl = Байх RL
edit-text-colour-opacity = Текстийн өнгө ба тунгалагжилт.
edit-thickness-visible-slice-slab-centred = Тоймын заагч дээр төвлөрсөн харагдах огтлолын давхаргын зузаан.
edit-translation-axis-help = Дэлхийн { $axis } тэнхлэгийн дагуух шилжилтийн зай.
edit-type = Төрөл
edit-type-direction-together-set-offset = Төрөл болон Чиглэл хамтдаа шилжилтийн талыг тодорхойлно. Карьер + Дээш ба Овоолго + Доош нь гадагш алхана; Карьер + Доош ба Овоолго + Дээш нь дотогш алхана.
edit-bench-direction-help = «Дээш» уступ бүрийг уступын өндрөөр өргөнө; «Доош» доошлуулна. Энэ нь мөн шилжилтийн талыг эргүүлнэ - Төрөлийг үзнэ үү.
edit-value-help = Утга нь сонгосон хэмжигдэхүүн ба өндрийн горимын дагуу тайлбарлагдана.
edit-vertical-rise-fall-each-bench = Дараагийн берм үүсэхээс өмнөх уступ бүрийн босоо өсөлт эсвэл уналт.
edit-bezier-control-point-1-help = Эхний Безье хяналтын цэгийн дэлхийн X, Y, Z координат.
edit-bezier-control-point-2-help = Хоёр дахь Безье хяналтын цэгийн дэлхийн X, Y, Z координат.

## Events strings

events-couldn-t-exit-error = Гарч чадсангүй: { $error }
events-couldn-t-save-error = Хадгалж чадсангүй: { $error }
events-set-elevation = Өндөрлөг тохируулах
events-set-elevation-from-cursor-hit = Заагчийн тусалтаас өндөрлөгийг Z { $z } болгож тохируулав
events-tool-not-available-section-view = Энэ хэрэгсэл огтлолын харагдацад боломжгүй байна

## Explorer strings

explorer-clear-active-triangulation-texture = Идэвхтэй триангуляцын текстурыг цэвэрлэх
explorer-delete-from-project = Төслөөс устгах
explorer-discard-changes = Өөрчлөлтийг үл хэрэгсэх...
explorer-download = Татаж авах
explorer-drape-over-surface = Гадаргуу дээр дараах
explorer-draped-over-surface = Гадаргуу дээр дараасан
explorer-duplicate = Хувилах
explorer-empty-collection = Хоосон цуглуулга
explorer-face-colour = Талын өнгө
explorer-id-block-model-id-source =
    ID: block-model:{ $id }{ $source }
    { $count } өнгөний хувьсагч
explorer-id-drill-holes-id-source =
    ID: drill-holes:{ $id }{ $source }
    { $holes } цооног
    { $fields } өнгөний талбар
explorer-id-point-cloud-id-source =
    ID: point-cloud:{ $id }{ $source }
    { $count } цэг
explorer-raster-id =
    ID: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = ID: triangulation:{ $id }{ $source }
explorer-load = Ачаалах
explorer-lock = Түгжих
explorer-new-collection = Шинэ цуглуулга
explorer-no-collection = Цуглуулгагүй
explorer-select-all-objects = Бүх объектыг сонгох
explorer-show-thickness-table = Зузааны хүснэгт харуулах
explorer-settings = Тохиргоо...
explorer-source-name = Эх сурвалж: { $name }
explorer-unload = Буулгах
explorer-unlock = Түгжээг тайлах

## Files strings

files-automatic-colour = Автомат өнгө
files-automatic-rl-spacing = Автомат RL зай
files-axis-scale-ratio = { $axis } масштабын харьцаа
files-ok = OK
files-reset-scale = 1×-т сэргээх
files-rl-grid-options = RL торны сонголтууд
files-rl-spacing = RL зай
files-scales-z-distances-visually-without = Хадгалагдсан координатыг өөрчлөхгүйгээр Z зайг харагдах байдлаар масштаблана.
files-thickness = Зузаан
files-xy-grid-options = XY торны сонголтууд
geophysics-checking-geophysics-files = Геофизикийн файлуудыг шалгаж байна
geophysics-downhole-geophysics-name-could-not = '{ $name }'-н цооногийн геофизикийг холбож чадсангүй: { $error }
geophysics-file-changed = Геофизикийн файл индексжүүлснээс хойш өөрчлөгдсөн
geophysics-file-unreadable = '{ $name }'-д холбосон геофизикийн файлыг { $path } дээр унших боломжгүй ({ $error }); өгөгдлийн багцын баруун товчийн цэсээс дахин холбоно уу
geophysics-linked-changed-rereading = '{ $name }'-д холбосон геофизик индексжүүлснээс хойш өөрчлөгдсөн; дахин уншиж байна
geophysics-hole-has-size-mib-geophysics = { $hole }-д { $size } MiB геофизикийн мөр байна, нэг цооногоор уншихаас их байна
geophysics-hole-needs-size-mib-its = { $hole }-н геофизикт { $size } MiB хэрэгтэй, хөтчид үлдсэнээс их байна: бусад зүйлийг буулгаад энэ өгөгдлийн багцыг буулгаж дахин ачаална уу
geophysics-linking-geophysics-name = { $name }-д геофизик холбож байна
geophysics-reading-geophysics-hole = { $hole }-н геофизикийг уншиж байна
geophysics-web-could-not-read-name-error = '{ $name }'-г унших боломжгүй: { $error }
geophysics-web-name-used-session-s-downhole = '{ $name }'-г энэ сешний цооногийн геофизикт ашиглаж байна

## Gpu strings

gpu-cache-block-model-surface-build-failed = Блокийн загварын гадаргуу бүтээх амжилтгүй боллоо: { $error }
gpu-cache-block-model-surface-build-worker = Блокийн загварын гадаргуу бүтээх ажлын процесс холболтоо тасалдлаа
gpu-cache-block-model-surface-chunk-rejected = GPU хуваарилалтаас өмнө блокийн загварын гадаргуугийн хэсэг татгалзагдав: instances={ $instances } байт, limit={ $limit } байт
gpu-cache-block-volume-worker-disconnected = Блокийн эзлэхүүн бэлтгэх ажлын процесс холболтоо тасалдлаа
gpu-cache-translucent-volume-could-not-built = Тунгалаг эзлэхүүнийг бүтээж чадсангүй ({ $error }); энэ блокийн загварыг оронд нь шоо хэлбэрээр харуулж байна.
gpu-cache-edge-chunk-rejected = GPU хуваарилалтаас өмнө триангуляцын ирмэгийн хэсэг татгалзагдав: instances={ $instances } байт, limit={ $limit } байт
gpu-cache-triangulation-chunk-rejected = Хуваарилалтаас өмнө триангуляцын GPU хэсэг татгалзагдав: vertices={ $vertices } байт, indices={ $indices } байт, limit={ $limit } байт
gpu-cache-triangulation-too-many-vertices = '{ $name }' триангуляц { $count } орой цэгтэй (> u32::MAX); GPU-д зориулж хэсэглэх боломжгүй
gpu-cache-triangulation-uploaded = '{ $name }' триангуляцыг { $chunks } орон зайн хэсэг ({ $faces } тал)-т байршуулав
i18n-active-language = Идэвхтэй хэл { $language } (багцлагдсан: { $bundled })
i18n-could-not-select-language-error = Хэлийг сонгож чадсангүй: { $error }

## Init strings

init-gpu-adapter-vendor-name-backend = GPU адаптер: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver = GPU драйвер: { $driver } { $driver_info }
init-gpu-limits-max-buffer-size = GPU хязгаарууд: max_buffer_size={ $max_buffer_size } MiB, max_storage_buffer_binding_size={ $max_storage_buffer_binding_size } MiB, max_storage_buffers_per_shader_stage={ $max_storage_buffers_per_shader_stage }, max_uniform_buffer_binding_size={ $max_uniform_buffer_binding_size } KiB, max_texture_dimension_2d={ $max_texture_dimension_2d }, max_bind_groups={ $max_bind_groups }
init-gpu-supports-maximum-buffer-size = GPU { $size } MiB хүртэлх дээд буфферийн хэмжээг дэмждэг; том дүрслэл бүрэн харагдахгүй байж болно
init-surface-present-mode = Гадаргуу үзүүлэх горим: { $mode }
init-wgpu-error-continuing-error = wgpu алдаа (үргэлжлүүлж байна): { $error }
input-could-not-read-name-error = { $name }-г унших боломжгүй: { $error }
input-could-not-slice-name-error = { $name }-г огтолж чадсангүй: { $error }
io-add-collar-file-explicit-segments = Амсрын файл (эсвэл тодорхой сегментийн файл) нэмнэ үү: цооногийн геофизик нь түүний тодорхойлсон цооногуудад холбогдоно.

## Io strings

io-ascii-points-xyz-pts = ASCII цэгүүд (.xyz, .pts)
io-attribute = Аттрибут
io-blank-header = (гарчиггүй)
io-block-model = Блокийн загвар:
io-choose-file-purpose-map-its = Баганыг холбохын тулд файлын зориулалтыг сонгоно уу.
io-choose-loaded-block-model = Ачаалагдсан блокийн загвар сонгох
io-choose-loaded-dataset = Ачаалагдсан өгөгдлийн багц сонгох
io-choose-loaded-layer = Ачаалагдсан давхарга сонгох
io-choose-loaded-triangulation = Ачаалагдсан триангуляц сонгох
io-choose-purpose = Зориулалт сонгох…
io-choose-source-file-files-import = Импортлох эх файл(ууд)-ыг сонгоно уу.
io-collar = Амсар
io-column-mapping = Баганы харгалзаа
io-comma-separated-values-csv = Comma-Separated Values (.csv)
io-csv-files = CSV файлууд
io-dataset = Өгөгдлийн багц:
io-density-read-g-cc-exported = Нягт, экспортолсон ёсоор g/cc гэж уншина. Медиан нь 0.5-аас 5 g/cc-ийн хооронд биш муруйг нэгж нь буруу юм шиг харагдаж байгаа тул анхааруулгатай хамт импортоос хасна.
io-depth = Гүн
io-diameter = Диаметр
io-downhole-geophysics = Цооногийн геофизик
io-drawing-exchange-format-dxf = Drawing Exchange Format (.dxf)
io-drill-holes = Цооногууд
io-east-x = Зүүн / X
io-elevation-z = Өндөрлөг / Z
io-end-x = Төгсгөл X
io-end-y = Төгсгөл Y
io-end-z = Төгсгөл Z
io-explicit-segments = Тодорхой сегментүүд
io-export = Экспортлох
io-export-csv-block-model = CSV блокийн загвар экспортлох
io-export-csv-drillholes = Цооногийг CSV болгон экспортлох
io-export-dxf = DXF экспортлох
io-export-one-layer = Нэг давхарга экспортлох
io-export-open-mining-format-2 = Open Mining Format 2 экспортлох
io-export-ply = PLY экспортлох
io-export-stl = STL экспортлох
io-export-wavefront-obj = Wavefront OBJ экспортлох
io-gamma-api = Гамма (API)
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-ignore-file = Файлыг алгасах
io-import = Импортлох
io-import-ascii-point-cloud = ASCII цэгэн үүл импортлох
io-import-drillhole-csv-bundle = Цооногийн CSV багц импортлох
io-import-geotiff = GeoTIFF импортлох
io-import-las-laz-point-cloud = LAS/LAZ цэгэн үүл импортлох
io-import-open-mining-format-2 = Open Mining Format 2 импортлох
io-import-pcd-point-cloud = PCD цэгэн үүл импортлох
io-import-ply = PLY импортлох
io-import-stl = STL импортлох
io-import-wavefront-obj = Wavefront OBJ импортлох
io-inclination = Хазайлт
io-interval = Интервал
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-long-spaced-density-g-cc = Урт зайтай нягт (g/cc)
io-mapped-csv-bundle-csv = Холбогдсон CSV багц (.csv)
io-measured-depth-down-hole-read = Цооногийн дагуух хэмжсэн гүн, метрээр уншина. Incline нэгж хөрвүүлэхгүй: нэгжийг файлыг экспортолсон мэдээллийн сан тогтооно.
io-model-file = Загварын файл
io-name-count-files = { $name } + { $count } файл
io-natural-gamma-read-api-units = Байгалийн гамма, экспортолсон ёсоор API нэгжээр уншина.
io-no-csv-chosen = .csv сонгоогүй
io-no-csv-files-chosen = CSV файл сонгоогүй
io-no-dxf-chosen = .dxf сонгоогүй
io-no-omf-chosen = .omf сонгоогүй
io-north-y = Хойд / Y
io-open-mining-format-2-omf = Open Mining Format 2 (.omf)
io-ply = PLY (.ply)
io-point-cloud-data-pcd = Point Cloud Data (.pcd)
io-projects = Төслүүд
io-reset = Дахин тохируулах
io-role-reason-also-collar = Мөн амсар шиг харагдаж байна
io-role-reason-collar = Цооног бүрт нэг мөр, координаттай
io-role-reason-geophysics = Цооног ба гүн, нарийн алхамтай хэмжилттэй
io-role-reason-interval = Цооног, эхлэх ба дуусах
io-role-reason-not-recognised = Өрмийн цооногийн хүснэгт гэж танигдсангүй
io-role-reason-segments = Цооног, эхлэх ба дуусах, эхлэл ба төгсгөлийн координаттай
io-role-reason-survey = Цооног, гүн ба чиглэл
io-short-spaced-density-g-cc = Богино зайтай нягт (g/cc)
io-source-file = Эх файл
io-start-x = Эхлэл X
io-start-y = Эхлэл Y
io-start-z = Эхлэл Z
io-stl = STL (.stl)
io-triangulation = Триангуляц:
io-unmapped = Холбогдоогүй
io-wavefront-obj = Wavefront OBJ (.obj)
io-writes-three-files-beside-name = Таны сонгосон нэрийн хажууд гурван файл бичнэ: амсар, судалгаа, интервал, энэ цонхны импортлодог баганаар.

## Jobs strings

jobs-background-task-poll-label-ended = '{ $poll_label }' далд ажил үр дүнгүй дуусав
jobs-cancelled-label-its-project-no = '{ $label }'-г цуцаллаа: түүний төсөл цаашид идэвхгүй байна
jobs-discarded-stale-result = '{ $poll_label }'-н хуучирсан далд үр дүнг хаяв, учир нь эх сурвалж өөрчлөгдсөн эсвэл хаагдсан
jobs-drillhole-import = цооногийн импорт
log-traces-auto-from-hole = Автомат, энэ цооногоос
log-traces-curve-no-reading = { $curve }: заалт алга
log-traces-curve-value-unit = { $curve }: { $value } { $unit }
log-traces-custom-range = Өөрийн муж
log-traces-default-colour = Үндсэн өнгө
log-traces-density-scale = Нягтын масштаб
log-traces-depth-m = { $depth } м
log-traces-gamma = Гамма
log-traces-gamma-colour = Гаммын өнгө
log-traces-gamma-scale = Гаммын масштаб
log-traces-percentile-range-no-data = Цооногийн 1-99-р перцентиль, гадагш бөөрөнхийлсөн. Энэ цооногт хараахан өгөгдөл байхгүй.
log-traces-percentile-range = Цооногийн 1-99-р перцентиль, гадагш бөөрөнхийлсөн: { $range }.
log-traces-long-density = Урт нягт
log-traces-long-density-colour = Урт нягтын өнгө
log-traces-min-max-unit = { $min }-с { $max } { $unit }
log-traces-reading = Уншиж байна...
log-traces-short-density = Богино нягт
log-traces-short-density-colour = Богино нягтын өнгө

## Logging strings

logging-activity-completed = Үйл ажиллагаа дууслаа
logging-activity-started = Үйл ажиллагаа эхэллээ
logging-application-id-id = Программын ID: { $id }
logging-application-name = Программын нэр: { $name }
logging-application-startup = Программын эхлэл
logging-build-target-os-architecture = Билд бай: { $os }-{ $architecture }
logging-completed = Дууссан
logging-count-messages = { $count } мессеж
logging-desktop-session-xdg-session-type = Ажлын талбарын сешн: XDG_SESSION_TYPE={ $session }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
logging-initialising-incline-design = Incline Design-г эхлүүлж байна
logging-locale-environment = Локал орчин: LANG={ $lang }, LC_ALL={ $locale }, TZ={ $timezone }
logging-macos-session = macOS сешн: USER={ $user }, SHELL={ $shell }
logging-operating-system-gnu-linux = Үйлдлийн систем: GNU / Linux
logging-operating-system-macos = Үйлдлийн систем: macOS
logging-operating-system-microsoft-windows = Үйлдлийн систем: Microsoft Windows
logging-pointer-width = Заагчийн өргөн: { $width }-бит
logging-process-id-id = Процессын ID: { $id }
logging-release-version = Хувилбарын дугаар: { $version }
logging-renderer = Дүрслэгч
logging-rust-compiler-host = Rust хөрвүүлэгчийн хост: { $host }
logging-system = Систем
logging-system-error = Системийн алдаа
logging-unknown = үл мэдэгдэх
logging-windows-session-sessionname-session = Windows сешн: SESSIONNAME={ $session }, USERNAME={ $user }
logging-working = Ажиллаж байна…

## Mac strings

mac-cannot-install-macos-menu-bar = macOS цэсний мөрийг үндсэн урсгалаас гадуур суулгах боломжгүй
mac-quit-app = { $app }-аас гарах

## Main strings

main-incline-design-web-startup-failed = Incline Design Web-н эхлэл амжилтгүй боллоо: { $error }

## Menu strings

menu-count-files-selected = { $count } файл сонгогдсон

## Object strings

object-edit-appearance = Дүр төрх
object-edit-arc-circle = Нум ба тойрог
object-edit-arc-segments = Нумын хэсгүүд
object-edit-bulge = Гүдгэр
object-edit-bulge-arcs-horizontal-data-model = Өгөгдлийн загварын дагуу гүдгэр нум нь хэвтээ байна: нум нь хэвтээ хавтгайд эргэдэг бөгөөд өндөрлөг нь орой цэг бүрээс дараагийнх руу шулуунаар өөрчлөгддөг.
object-edit-centre-x = Төв X
object-edit-centre-y = Төв Y
object-edit-centre-z = Төв Z
object-edit-chord = Хорд
object-edit-colour-layer = Давхаргын өнгө
object-edit-enter-number = Тоо оруулна уу
object-edit-follow-owning-layer-s-colour = Энэ объектод бэхлэгдсэн өнгөний оронд эзэмшигч давхаргын өнгийг дагах.
object-edit-id = ID
object-edit-identity = Ижилт
object-edit-insert-after = Дараа нь оруулах
object-edit-join-last-vertex-back-first = Сүүлийн орой цэгийг эхнийхтэй нь дахин холбоно.
object-edit-length = Урт { $length } м
object-edit-move-down = Доош шилжүүлэх
object-edit-move-up = Дээш шилжүүлэх
object-edit-object-has-no-arc-segments = Энэ объект нумын хэсэггүй байна.
object-edit-object-has-single-position = Энэ объект ганц байрлалтай.
object-edit-object-needs-least-required-vertices = Энэ объектод дор хаяж { $required } орой цэг шаардлагатай
object-edit-one-more-properties-not-valid = Нэг буюу түүнээс олон шинж чанар хүчинтэй тоо биш байна
object-edit-perimeter-area = Периметр { $length } м, талбай { $area } м²
object-edit-reverse = Урвуулах
object-edit-row-invalid-number = { $row } мөр: байрлал эсвэл гүдгэр хүчинтэй тоо биш байна
object-edit-sweep = Хамрах өнцөг
object-edit-text-not-number = "{ $text }" тоо биш байна
object-edit-vertices = Орой цэгүүд

## Omf strings

omf-element-name-has-count-tie = '{ $name }' элемент цаашид агуулаагүй цооногуудыг нэрлэсэн { $count } холболт агуулж байна
omf-element-name-has-count-unreadable = '{ $name }' элементэд уншигдахгүй { $count } ажлын үе байна; тэдгээрийг хассан
omf-element-unsupported-section = '{ $name }' элемент '{ $section }' хэсгийг нэрлэсэн бөгөөд энэ хувилбар ийм төрлийн зүйлийг харуулж чадахгүй
omf-element-name-names-unknown-section = '{ $name }' элемент тодорхойгүй '{ $section }' хэсгийг нэрлэсэн байна
omf-ignoring-colour-map-omf-attribute = OMF-н '{ $attribute }' аттрибут дээрх өнгийн зургийг үл хэрэгсэж байна: { $error }
omf-mining-data-exported-incline = Incline-с экспортолсон уул уурхайн өгөгдөл
omf-import = OMF импорт
omf-texture = OMF текстур
omf-validation-warnings = OMF баталгаажуулалтын анхааруулга: { $warnings }
omf-application-metadata-dropped = Төслийн '{ $application }' программын метаөгөгдөл хадгалагдахгүй
omf-project-author-not-retained = Төслийн зохиогч хадгалагдахгүй
omf-project-description-not-retained = Төслийн тайлбар хадгалагдахгүй
omf-unsupported-metadata-keys = Төсөл дэмжигдээгүй метаөгөгдлийн түлхүүр агуулж байна: { $keys }
omf-skipped-drillhole-data-saved-older = Хуучин бүтэцтэй хадгалсан цооногийн өгөгдлийг алгаслаа ({ $names }); эх файлуудаас нь дахин импортлоно уу
omf-modelling-settings-unreadable = Төслийн загварчлалын тохиргоог уншиж чадсангүй; үндсэн утгуудыг ашиглаж байна

## Plot strings

plot-1-1000-one-millimetre-sheet = 1:1000 масштабтай үед хуудсан дээрх 1 мм нь газар дээрх 1 метртэй тэнцүү.
plot-1-scale-covers-width-height = 1:{ $scale } · { $width } × { $height } м хамарна
plot-all-visible-data = Бүх харагдаж буй өгөгдөл
plot-automatic-grid-interval = Торны автомат интервал
plot-border = Хүрээ
plot-centre = Төвлөрүүлэх
plot-fit-scale-help = Харагдаж буй бүх зүйлийг хуудсанд багтаах хамгийн бага стандарт масштабыг сонгоно уу.
plot-coordinate-grid = Координатын тор
plot-current-view-centre = Одоогийн харагдацын төв
plot-date-caps = ОГНОО
plot-date = Огноо
plot-dots-per-inch-paper-size = Инч тутмын цэг. Энэ цаасны хэмжээг { $max_dpi } dpi хүртэл растержуулж болно; 300 dpi нь ердийн хэвлэлийн чанар юм.
plot-dpi = dpi
plot-drawing-no = ЗУРГИЙН №
plot-drawing-number = Зургийн дугаар
plot-drawn-by-caps = ЗУРСАН
plot-drawn-by = Зурсан
plot-e-g-example-gold-project = ж: Жишээ Алтны Төсөл
plot-entered-coordinates = Оруулсан координат
plot-export-png = PNG экспортлох...
plot-fit-scale-visible-data = Масштабыг харагдаж буй өгөгдөлд тааруулах
plot-grid-interval = Торны интервал
plot-landscape = Хэвтээ
plot-lists-visible-surfaces-design-layers = Харагдаж буй гадаргуу болон зураг төслийн давхаргуудыг тэдгээрийн өнгөтэй нь жагсаана.
plot-margin = Захын зай
plot-margins-leave-no-room-map = Захын зай нь газрын зурагт зай үлдээхгүй байна
plot-metres-scale-1-scale = метр    Масштаб 1:{ $scale }
plot-mm = мм
plot-north-arrow = Хойд зүг заагч
plot-nothing-visible-draw = Зурах юу ч харагдахгүй байна
plot-paper = Цаас
plot-paper-orientation-width-height-mm = { $paper } { $orientation } · { $width } × { $height } мм
plot-paper-size = Цаасны хэмжээ
plot-pick-interval-reads-roughly-every = Хэвлэсэн хуудсан дээр ойролцоогоор 50 мм тутамд уншигдах интервал сонгоно уу.
plot-plan = Төлөвлөгөө
plot-scale-must-be-positive = Зургийн масштаб эерэг тоо байх ёстой
plot-png-written-sheet-s-exact = PNG нь хуудасны яг цаасны хэмжээгээр бичигдэж, DPI-г нь тэмдэглэдэг тул бодит масштабаар хэвлэгдэнэ.
plot-portrait = Босоо
plot-resolution = Нарийвчлал
plot-rev = ХУВИЛБАР
plot-revision = Хувилбар
plot-scale = МАСШТАБ
plot-scale-ratio = Масштаб  1:
plot-scale-framing = Масштаб ба хүрээ
plot-sheet-furniture = Хуудасны хүрээлэн орчин
plot-size-width-height-mm = { $size } ({ $width } × { $height } мм)
plot-subtitle = Дэд гарчиг
plot-title = Гарчиг
plot-title-block = Гарчгийн хайрцаг
plot-today = өнөөдөр
point-cloud-classify = Ангилах
point-cloud-classify-vegetation = Ургамалжилтыг ангилах
point-cloud-cloth-resolution = Даавууны нягтрал
point-cloud-cloth-resolution-about-one-half = Сонгосон хамгийн сийрэг үүлний цэгүүдийн зайн ойролцоогоор нэг хагас дахин их даавууны нягтрал, ингэснээр бөөм бүрийн доор буцалт байна.
point-cloud-combine-selected-point-clouds-into = Сонгосон цэгэн үүлсийг нэг шинэ үүл болгон нэгтгэнэ, ингэснээр бүгдийг хамарсан ганц триангуляц бүтээж болно. Цэг бүрийн өнгийг хадгална; өнгөгүй үүл өөрийн дүрслэлийн өнгийг оруулна.
point-cloud-selected-count = { $count } сонгосон · { $points } цэг
point-cloud-delete-selected-clouds-from-project = Нэгтгэл дууссаны дараа сонгосон үүлсийг төслөөс устгаж, давхар хуулбарынх нь эзлэх байсан санах ойг чөлөөлнө.
point-cloud-flat-pads-structures = Тэгш (талбай, байгууламж)
point-cloud-ground-cloud-covers-steep-follows = Үүлний хамарсан газар. Эгц горим хана дагуу оройгоос нь доош дагана; Тэгш горим том барилга, тоног төхөөрөмжийг гүүрлэх хөшүүн даавуу ашигладаг боловч огцом хугарлыг бөөрөнхийлнө.
point-cloud-ground-threshold = Газрын босго
point-cloud-how-far-around-each-point = Цэг бүрийн эргэн тойронд хөршийг хэр холоос тоолох.
point-cloud-join = Нэгтгэх
point-cloud-let-cloth-follow-walls-down = Даавууг хананы орой хэсгээс доош дагуулна, өөрөөр хөшүүн чанар нь түүнийг хананаас холдуулна. Тоног төхөөрөмжөөр дүүрсэн налуу багатай газар дээр л унтраана уу.
point-cloud-mark-each-point-ground-noise = Цэг бүрийг газар, шуугиан эсвэл ангилаагүй гэж тэмдэглэнэ. Даавууг үүлний доор дээш дарж, газрын гадаргуу дээр суулгана; түүнээс газрын босго доторх цэгүүд газар болно. Одоо байгаа ангиллыг солино; буцаах нь тэдгээрийг сэргээнэ.
point-cloud-mark-isolated-returns-birds-dust = Тусгаар буцалтуудыг - шувуу, тоос, олон замын алдааг - газрыг олохын өмнө шуугиан гэж тэмдэглэнэ, ингэснээр тэнэг нам цэг даавууг доош татаж чадахгүй.
point-cloud-mark-noise = Шуугиан тэмдэглэх
point-cloud-minimum-neighbours = Хөршийн доод тоо
point-cloud-name-assigned-joined-point-cloud = Нэгтгэсэн цэгэн үүлд өгөх нэр.
point-cloud-name-count-points = { $name } ({ $count } цэг)
point-cloud-noise-radius = Шуугианы радиус
point-cloud-point-clouds = Цэгэн үүлс
point-cloud-points-closer-than-settled-cloth = Суусан даавуунаас гадаргуугаараа хэмжихэд энэ зайгаас ойр цэгүүд газар болно.
point-cloud-points-fewer-neighbours-than-within = Шуугианы радиус дотор энэ тооноос цөөн хөрштэй цэгүүд шуугиан болно.
point-cloud-raise-cloth-resolution-if-your = Таны машинд RAM бага бол даавууны нягтралыг нэмэгдүүлнэ үү.
point-cloud-recommended = Санал болгосон
point-cloud-recover-steep-slopes = Эгц налууг сэргээх
point-cloud-relief-dumps-rolling-ground = Рельеф (овоолго, хэвгий газар)
point-cloud-remove-sources = Эх үүсвэрийг устгах
point-cloud-resolution-m-points-spacing-m = { $resolution } м (цэгүүд ~{ $spacing } м зайтай)
point-cloud-selected-clouds-copied-into-joined = Нэгтгэсэн үүл рүү хуулсан сонгосон үүлс. Өөр багцыг нэгтгэхийн тулд цонхыг хаана уу.
point-cloud-selected-clouds-each-classified-its = Тус бүрийг нь тусад нь ангилсан сонгосон үүлс. Өөр багцыг ангилахын тулд цонхыг хаана уу.
point-cloud-classify-help = Цэг бүрийн эргэн тойрны цэгүүдийн хэлбэрийг уншдаг сургасан ангилагчаар буцалтуудыг ангилна: газар, ургамалжилт - өндрөөр нь нам (1 м-ээс доош), дунд (3 м-ээс доош) эсвэл өндөр гэж хуваасан - ба барилга, тоног төхөөрөмж зэрэг бусад бүхэн ангилаагүй хэвээр үлдэнэ. Зөвхөн даавууг ашиглахын тулд үүнийг унтраана уу.
point-cloud-spacing-cloth-s-particles-around = Даавууны бөөмсийн зай. Үүлний цэгийн зайтай ойролцоо утгаас эхлэх нь зүгээр; нарийн утга газрыг илүү нарийн дагах боловч нягт цэг шаардана.
point-cloud-steep-pit-walls-benches = Эгц (карьерийн хана, уступ)
point-cloud-terrain = Рельеф
point-cloud-use = Ашиглах

## Products strings

products-add-initiation = Дэлбэлгээ эхлүүлэх цэг нэмэх
products-delay = Саатал
products-delay-palette = Саатлын палитр
products-how-long-after-shot-fired = Тэсэлгээ эхэлснээс хойш энэ амсар хэдий хугацааны дараа дэлбэрэхийг заана.
products-initiation-name = Дэлбэлгээ эхлүүлэх · { $name }
products-milliseconds-between-one-hole-firing = Нэг цооног дэлбэрснээс дараагийнх хүртэлх миллисекундийн хугацаа.
products-ms = мс
products-no-products = Бүтээгдэхүүн алга
products-remove = Устгах
products-update = Шинэчлэх

## Progress strings

progress-percent-done-total = { $percent } ({ $total }-с { $done })
progress-task-finished = { $task }: Дууссан

## Project strings

project-item = Зүйл
project-steep-pair-distance-positive = Эгц хосын зай нь метрээр илэрхийлсэн эерэг тоо байх ёстой
project-steep-pair-angle-range = Эгц хосын өнцөг 0-ээс их, 90 градусаас ихгүй байх ёстой
project-cut-depth-positive = Тайрах гүн 0-ээс их метрийн тоо байх ёстой
project-thin-plate-spline-exact = нимгэн хавтангийн сплайн, яг
project-method-steep-pairs-under = Арга: { $method } · { $distance } м-ээс ойр, { $degrees } градусаас эгц хосууд

## Properties strings

properties-adds-view-dependent-rim-highlight = Блок болон материалын хилд харагдацаас хамаарсан ирмэгийн тодруулга нэмнэ. Үүнийг унтраавал эзлэхүүн дүрслэлийн ажлыг бага зэрэг хөнгөвчилнө.
properties-block-model-downscale = Блокийн загварыг багасгах
properties-camera = Камер
properties-camera-clip-planes = Камерын огтлолын хавтгайнууд
properties-cap-while-resizing = Хэмжээ өөрчлөхөд хязгаарлах
properties-colours-each-point-cloud-chunk = Цэгэн үүлний хэсэг бүрийг өнгөөр будаж, харагдах хүрээгээр тайрдаг хайрцгийг нь тоймлож, өмнөх кадрт зурсан цэгийг нарийвчлалын түвшний зорилт ба харагдах нийт тоотой нь хамт төлөвийн мөрөнд харуулна.
properties-colours-each-surface-chunk-outlines = Гадаргуугийн хэсэг бүрийг өнгөөр будаж, харагдах хүрээгээр тайрдаг хайрцгийг нь тоймлож, өмнөх кадрт зурсан талыг харагдах нийт тоотой нь хамт төлөвийн мөрөнд харуулна.
properties-dark-mode = Харанхуй горим
properties-dataset = Өгөгдлийн багц
properties-developer = Хөгжүүлэгч
properties-downscale-rasters = Растерыг багасгах
properties-drillholes = Цооногууд
properties-edit-object = Объект засах...
properties-field-view = Харах өнцөг
properties-fps = FPS
properties-frame-counter = Фрэймийн тоолуур
properties-frame-rate-cap = Фрэйм хурдны дээд хязгаар
properties-hz = Гц
properties-interface = Интерфейс
properties-invert-horizontal = Хэвтээгээр урвуулах
properties-invert-vertical = Босоогоор урвуулах
properties-limits-newly-loaded-geotiff-previews = Шинээр ачаалагдсан GeoTIFF урьдчилан харах зургийг хамгийн урт талдаа 4096 пиксел хүртэл хязгаарлана. GPU-н текстурын хязгаар хүртэл бүрэн нарийвчлал ашиглахын тулд унтраана уу, энэ нь илүү санах ой ашиглана.
properties-line-colour = Шугамын өнгө
properties-look-sensitivity = Харцны мэдрэмж
properties-max-clip-span = Огтлолын дээд урт
properties-modelling = Загварчлал
properties-modelling-help = Гадаргуу бүтээх нь торыг хэрхэн зурахыг тодорхойлно. Төслийн түвшний тохиргоо бөгөөд төсөлтэйгээ хамт хадгалагдана.
properties-move-layer = Давхарга руу шилжүүлэх...
properties-near-clip-limit = Ойрын огтлолын хязгаар
properties-no-drillhole-datasets-open = Нээлттэй цооногийн өгөгдлийн багц алга.
properties-orbit-sensitivity = Эргэлтийн мэдрэмж
properties-panel-chrome = Самбарын дизайн
properties-performance = Гүйцэтгэл
properties-plan-mode = Төлөвлөгөөний горим
properties-point-cloud-chunk-debug-view = Цэгэн үүлний хэсгийн дибаг харагдац
properties-presents-step-display-no-tearing = Дэлгэцтэй нийцүүлэн үзүүлнэ: хагарал үгүй бөгөөд дэлгэц фрэймийн хурдыг тогтооно. Унтраасан үед фрэймүүд зурагдмагцаа шууд харагдах бөгөөд доорх хязгаарлалт хэрэгжинэ.
properties-reflective-block-edges = Тусгалтай блокийн ирмэг
properties-restore-defaults = Үндсэн тохиргоог сэргээх
properties-show-console = Консол харуулах
properties-shows-live-near-far-projection = Төлөв байдлын мөрөнд шууд ойр ба хол проекцийн зайг харуулна.
properties-snap-polling = Наалдацыг шалгах
properties-steep-pair-angle = Эгц хосын өнцөг
properties-steep-pair-distance = Эгц хосын зай
properties-steep-pair-distance-help = Төлөвлөгөөн дээр энэ зайгаас ойр, доорх өнцгөөс эгц цэгийн хосуудыг бүтээлт амжилттай болоход нэрлэнэ. Хэзээ ч татгалзаж, засдаггүй.
properties-surface-chunk-debug-view = Гадаргуугийн хэсгийн дибаг харагдац
properties-vertical-sync = Босоо синхрончлол
properties-world-axis-gizmo = Дэлхийн тэнхлэгийн гизмо
properties-zoom-cursor = Заагч руу томруулах
properties-zoom-sensitivity = Томруулах мэдрэмж
reference-points-count-holes-from-dataset = '{ $dataset }'-с { $count } цооног
reference-points-holes-from-datasets = { $datasets } өгөгдлийн багцаас { $count } цооног
reference-points-holes = Цооногууд
reference-points-holes-points-placed-selected-when = Цонхыг нээх үед сонгосон цооногууд. Өөрийг сонгохын тулд хаана уу.
reference-points-make = Үүсгэх
reference-points-no-categorical-field = Ангиллын талбар алга
reference-points-no-values = Утга алга
reference-points-one-point-per-hole-boundary = Цооног бүрт давхаргын дээд эсвэл доод хучаан дээр нэг цэгийг шинэ давхарга болгон тавина. Давхаргыг хоёр удаа бүртгэсэн цооног дээд талыг өгч тэмдэглэгдэнэ.
reference-points-one-point-per-hole-collar = Цооног бүрийн амсарт нэг цэгийг шинэ давхарга болгон тавина; Гадаргуу бүтээх үүнээс газрын гадаргуу хийнэ.
reference-points-points-at = Цэгийн байрлал
reference-points-at-logged-pick = Бүртгэсэн заг
reference-points-at-collars = Амсарууд
reference-points-reference-points = Лавлах цэгүүд
reference-points-side = Тал
reference-points-working-section = Ажлын үе
reference-points-working-section-field = Ажлын үеийн талбар
reference-surface-controls = Удирдах шугамууд
reference-surface-extent = Хамрах хүрээ
reference-surface-points-outside-extent-still-shape = Хамрах хүрээнээс гадуурх цэгүүд гадаргууг хэвээр хэлбэржүүлнэ; зөвхөн гадаргууг түүнд огтолно.
reference-surface-points-surface-built-from-selected = Гадаргууг бүтээх цэгүүд, цонх нээгдэх үед сонгогдсон ёсоор. Өөрийг сонгохын тулд цонхыг хаана уу.
reference-surface-extent-help = Дууссан гадаргууг огтлох сонгосон хаалттай шугам; түүнээс гадуурх цэгүүд гадаргууг хэвээр хэлбэржүүлнэ.
reference-surface-selected-open-strings-surface-made = Гадаргууг дайрч өнгөрүүлэх сонгосон задгай шугамууд, цонх нээгдэх үед сонгогдсон ёсоор. Өөрийг сонгохын тулд цонхыг хаана уу.
reference-surface-grids-selected-points-plan-into = Сонгосон цэгүүдийг төлөвлөгөөн дээр торлон шинэ гадаргуу үүсгэнэ. Бүтээлт бүр гадаргуу нэмнэ.
reference-surface-change-these-in-preferences = Эдгээрийг Тохиргоо, Загварчлал хэсэгт өөрчилнө
reference-surface-triangulates-selected-points-plan-in = Сонгосон цэгүүдийг төлөвлөгөөн дээр триангуляц хийж шинэ гадаргуу үүсгэнэ. Бүтээлт бүр гадаргуу нэмнэ.

## Screenshot strings

screenshot-could-not-encode-viewport-image = Харагдах цонхны зургийг кодчилж чадсангүй: { $error }
screenshot-could-not-map-viewport-screenshot = Харагдах цонхны дэлгэцийн зургийг зурагдуулж чадсангүй: { $error }
screenshot-could-not-save-viewport-image = { $path } харагдах цонхны зургийг хадгалж чадсангүй: { $error }
screenshot-downloaded-viewport-image-file-name = Харагдах цонхны зургийг татаж авлаа: { $file_name }
screenshot-saved-viewport-image-path = Харагдах цонхны зургийг хадгаллаа: { $path }
screenshot-viewport-image-download-failed-error = Харагдах цонхны зургийг татаж авах амжилтгүй боллоо: { $error }

## Spatial strings

spatial-bvh-face-index-out-of-range = BVH-н { $index } талын индекс тор доторх хязгаараас гарсан байна; доройтсон гурвалжинг орлуулж байна

## State strings

state-above = дээш буюу тэнцүү
state-activate-project = Төслийг идэвхжүүлэх
state-all-open-incline-design-data = Incline Design-д нээлттэй байгаа бүх өгөгдөл
state-apply-generated-rings = Үүсгэсэн цагирагийг хэрэглэх
state-apply-selection = Сонголтод хэрэглэх
state-rotate-by-azimuth-dip = { $azimuth }° азимут, { $dip }° уналттай
state-rotate-to-azimuth-dip = { $azimuth }° азимут, { $dip }° уналт руу
state-below = доош буюу тэнцүү
state-build-reference-points = Лавлах цэг бүтээх
state-centre-rotation = Эргэлтийн төв
state-checking-unsaved-work = Хадгалаагүй ажлыг шалгаж байна
state-choose-destination = Хүлээн авагчийг сонгох
state-choose-one-more-files = Нэг буюу түүнээс олон файл сонгох
state-clear-raster = Растерыг цэвэрлэх
state-click-pit-shell-viewport = Харагдах цонхон дахь карьерийн бүрхүүлийг товшино уу.
state-click-pit-stockpile-solid-viewport = Харагдах цонхон дахь карьер эсвэл овоолгын хатуу биеийг товшино уу.
state-click-surface-viewport = Харагдах цонхон дахь гадаргууг товшино уу.
state-click-topology-viewport = Харагдах цонхон дахь топологийг товшино уу.
state-close-project = Төслийг хаах
state-colour-drillholes = Цооногийг өнгөөр ялгах
state-colour-drillholes-working-section = Цооногийг ажлын үеээр өнгөлөх
state-colour-points-classification = Цэгийг ангиллаар өнгөлөх
state-copy-objects-layer = Объектыг давхарга руу хуулах
state-count-cloud-s = { $count } үүл
state-count-file-s = { $count } файл
state-count-object-s-axis-value = { $count } объект · { $axis } { $value }
state-count-object-s-closed = { $count } объект · { $closed }
state-count-object-s-layer = { $count } объект · { $layer }
state-count-object-s-weight = { $count } объект · { $weight }
state-count-object-s-z-elevation = { $count } объект · Z { $elevation }
state-points-controls-clipped = { $count } цэг · { $controls } удирдах шугам · хамрах хүрээний шугамаар огтолсон
state-points-controls-outline = { $count } цэг · { $controls } удирдах шугам · цэгүүдийн контураар огтолсон
state-points-controls-unclipped = { $count } цэг · { $controls } удирдах шугам · огтлоогүй
state-create-collection = Цуглуулга үүсгэх
state-create-point-cloud-tin = Цэгэн үүлээс TIN үүсгэх
state-create-project = Төсөл үүсгэх
state-current-project = Одоогийн төсөл
state-cut-topology-pit-shell = Топологийг карьерийн бүрхүүлээр тайрах
state-cut-triangulation-polyline = Триангуляцыг полилиниагаар тайрах
state-cut-triangulation-z = Триангуляцыг Z-ээр тайрах
state-dark-mode = Харанхуй горим
state-data-ticked-export-checklist = Экспортын жагсаалтад тэмдэглэсэн өгөгдөл
state-detached = Салгасан
state-disabled = Идэвхгүй
state-discard-project-changes = Төслийн өөрчлөлтийг үл хэрэгсэх
state-discard-replace-project = Үл хэрэгсэн, төслийг солих
state-discarding-unsaved-changes = Хадгалаагүй өөрчлөлтийг үл хэрэгсэж байна
state-docked = Тогтоосон
state-drape-raster = Растерыг дараах
state-drill-pattern = Өрмийн сүлжээ
state-duplicate-layer = Давхарга хувилах
state-east = Зүүн
state-enabled = Идэвхтэй
state-exit-incline-design = Incline Design-аас гарах
state-export-block-model-csv = Блокийн загварыг CSV болгон экспортлох
state-export-drillhole-csv = Цооногийн CSV экспортлох
state-export-layer-dxf = Давхаргыг DXF болгон экспортлох
state-export-omf = OMF экспортлох
state-export-project-dxf = Төслийг DXF болгон экспортлох
state-export-triangulation = Триангуляц экспортлох
state-export-viewport-image = Харагдах цонхны зургийг экспортлох
state-finish-closed-polyline = Хаалттай полилиниаг дуусгах
state-finish-open-polyline = Задгай полилиниаг дуусгах
state-fit-extents = Хэмжээнд тааруулах
state-plan-view-then-fit-extents = Ижил зайнаас дээрээс харах, дараа нь хэмжээнд тааруулах
state-fix-release-centre-both-views = Хоёр харагдац эргэдэг төвийг тогтоох эсвэл суллах
state-folder-section = { $section } дахь { $folder }
state-generate-contours = Изолиниа үүсгэх
state-hidden = Нуугдсан
state-import-drillholes = Цооног импортлох
state-import-omf = OMF импортлох
state-import-point-cloud = Цэгэн үүл импортлох
state-import-raster = Растер импортлох
state-import-triangulation = Триангуляц импортлох
state-insert-intersection-points = Огтлолцлын цэгүүдийг оруулах
state-insert-points-elevation = Өндөрлөг дээр цэгүүд оруулах
state-keep-inside = Дотор талыг хадгалах
state-keep-outside = Гадна талыг хадгалах
state-kriged-block-model = Кригинг хийсэн блокийн загвар
state-load-block-model = Блокийн загвар ачаалах
state-load-drillholes = Цооног ачаалах
state-load-layer = Давхарга ачаалах
state-load-point-cloud = Цэгэн үүл ачаалах
state-load-raster = Растер ачаалах
state-load-triangulation = Триангуляц ачаалах
state-locked-count-object-s = { $count } объектыг түгжлээ
state-major-minor = Гол { $major } · туслах { $minor }
state-member-into-folder-section = { $member }-г { $section } дахь { $folder } руу
state-member-root-section = { $member }-г { $section }-н үндэс рүү
state-move-axis-value = Тэнхлэгийн утга руу шилжүүлэх
state-move-objects-layer = Объектыг давхарга руу шилжүүлэх
state-name-count-cloud-s = { $name } · { $count } үүл
state-name-count-holes = { $name } · { $count } цооног
state-name-count-object-s = { $name } · { $count } объект
state-name-z-min-z-max = { $name } · { $z_min }-с { $z_max } хүртэл
state-new-collection-under-section = { $section } дор шинэ цуглуулга
state-next-edit = Дараагийн засвар
state-north = Хойд
state-off = Унтарсан
state-on = Асаалттай
state-open-containing-folder = Агуулсан хавтсыг нээх
state-open-project = Төсөл нээх
state-preserve-view-angle = Харагдацын өнцгийг хадгалах
state-previous-edit = Өмнөх засвар
state-project-id = Төсөл { $id }
state-remove-block-model = Блокийн загвар устгах
state-remove-drillholes = Цооног устгах
state-remove-point-cloud = Цэгэн үүл устгах
state-remove-raster = Растер устгах
state-remove-triangulation = Триангуляц устгах
state-removed-from-active-triangulation = Идэвхтэй триангуляцаас устгагдсан
state-removed-from-every-triangulation = Бүх триангуляцаас устгагдсан
state-rename-kind = { $kind }-ийг нэр өөрчлөх
state-rename-seam = Давхаргын нэрийг өөрчлөх
state-rename-seam-from-to = { $from }-аас { $to }
state-save-close-project = Хадгалаад төслийг хаах
state-save-despite-unsupported-content = Дэмжигдээгүй агуулгыг үл харгалзан хадгалах
state-save-project = Төслийг өөр нэрээр хадгалах
state-save-replace-project = Хадгалаад төслийг солих
state-saving-current-project = Одоогийн төслийг хадгалж байна
state-section-name = { $section } хэсэг
state-select-layer-objects = Давхаргын объектуудыг сонгох
state-selected-objects = Сонгосон объектууд
state-selected-polylines = Сонгосон полилиниа
state-selected-scene-elements = Сонгосон дүрслэлийн элементүүд
state-hidden-objects = Нуусан объект бүр
state-set-block-model-variable = Блокийн загварын хувьсагчийг тохируулах
state-set-cinematic-view = Кино харагдац тохируулах
state-set-drillhole-colour-preset = Цооногийн өнгөний бэлэн тохиргоог тохируулах
state-set-drillhole-discs = Цооногийн дискийг тохируулах
state-set-drillhole-style = Цооногийн хэв маягийг тохируулах
state-set-drillhole-width = Цооногийн өргөнийг тохируулах
state-set-entity-lock = Объектын түгжээг тохируулах
state-set-grid = Тор тохируулах
state-set-layer-lock = Давхаргын түгжээг тохируулах
state-set-line-weight = Шугамын зузааныг тохируулах
state-set-modelling-settings = Загварчлалын тохиргоог тохируулах
state-seam-surface-from-thickness = зузааны цэгүүдээс давхаргын нөгөө гадаргуу
state-clip-to-surface-count = { $count } гадаргуу
state-collar-points-holes = амсарууд, { $count } цооног
state-thickness-points-holes-only = зөвхөн цооногууд
state-thickness-points-with-pairs = цооногууд ба { $name }-ээс хэмжсэн хосууд
state-set-object-colour = Объектын өнгийг тохируулах
state-set-object-fill = Объектын дүүргэлтийг тохируулах
state-set-point-visibility = Цэгийн харагдацыг тохируулах
state-set-polyline-closed = Полилиниаг хаалттай болгож тохируулах
state-set-raster-lock = Растерын түгжээг тохируулах
state-set-standard-view = Стандарт харагдацыг тохируулах
state-set-topology-wireframes = Топологийн торон дүрсийг тохируулах
state-set-triangulation-colour = Триангуляцын өнгийг тохируулах
state-shift-names = Нэрсийг шилжүүлэх
state-shift-names-down = { $field } цооногийн дагуу доош
state-shift-names-down-from-here = { $field } горизонтоос цооногийн дагуу доош
state-shift-names-up = { $field } цооногийн дагуу дээш
state-shift-names-up-from-here = { $field } горизонтоос цооногийн дагуу дээш
state-show-console = Консол харуулах
state-show-project = Төслийг харуулах
state-shown = Харагдаж байна
state-slice-mode = Огтлолын горим
state-slice-preview = Огтлолын урьдчилсан харагдац
state-south = Өмнөд
state-stem-contours = { $stem } изолиниа
state-target-new-name = { $target }-с «{ $new_name }» болгох
state-trim-above = Дээрхийг тайрах
state-trim-below = Доорхыг тайрах
state-trim-triangulation-surface = Триангуляцыг гадаргуугаар тайрах
state-undrape-raster = Растерыг дараахаас цуцлах
state-undrape-rasters = Растеруудыг дараахаас цуцлах
state-unload-block-model = Блокийн загварыг буулгах
state-unload-drillholes = Цооногуудыг буулгах
state-unload-layer = Давхаргыг буулгах
state-unload-point-cloud = Цэгэн үүлийг буулгах
state-unload-raster = Растерыг буулгах
state-unload-triangulation = Триангуляцыг буулгах
state-untitled-project = Нэргүй төсөл
state-use-typed-radius = Бичсэн радиусыг ашиглах
state-west = Баруун

## Status strings

status-clip-near-far = Огтлолын ойр/хол/Δ: -- / -- / --
status-faces-chunks = Талууд: -- / -- (--/-- хэсэг)
status-frame-rate = Фрэйм хурд
status-points-chunks = Цэгүүд: -- / -- ба -- (--/-- хэсэг)

## Text strings

text-could-not-build-vector-mesh = { $font } фонт, { $glyph } тэмдэгтийн вектор тор бүтээж чадсангүй: { $error }
text-document-text-mesh-exceeded-its = Баримтын текст тор u32 индексийн мужаас хэтэрлээ

## Seam surface strings

seam-surface-column-other = Нөгөө гадаргуугийн z
seam-surface-column-reference = Лавлах z
seam-surface-note = Давхаргын нөгөө гадаргууг түүний зузааны цэгүүдээс үүсгэнэ. Ажиллуулалт бүр нэг гадаргуу нэмнэ.
seam-surface-output = Үүсгэх
seam-surface-output-help = Дээд хучааны гадаргуугийн доорх доод хучаа, эсвэл доод хучааны гадаргуугийн дээрх дээд хучаа.
seam-surface-reference-help = Цонхыг нээх үед сонгосон гадаргуу. Шинэ гадаргуу түүний тор ба контурыг дагана.
seam-surface-run = Зузааны цэгүүд
seam-surface-run-help = Энэ сешнд энэ гадаргуу дээр хамгийн сүүлд үүсгэсэн зузааны цэгүүд.
seam-surface-table-surface = { $count } зангилаа, { $surface }-ээс өлгөсөн
seam-surface-table-title = Зузааны тор: { $name }

## Thickness strings

thickness-points-choose-pairs = CSV сонгох...
thickness-points-checking-surface = Гадаргууг шалгаж байна...
thickness-points-clear-pairs = Арилгах
thickness-points-column-along = Цооногийн дагуу
thickness-points-column-dip = Уналт
thickness-points-column-direction = Уналтын чиглэл
thickness-points-column-floor = Доод хучаа (гүн эсвэл z)
thickness-points-column-roof = Дээд хучаа (гүн эсвэл z)
thickness-points-column-source = Эх сурвалж
thickness-points-column-true = Жинхэнэ зузаан
thickness-points-column-vertical = Босоо зузаан
thickness-points-column-x = X
thickness-points-column-y = Y
thickness-points-holes = Цооногууд
thickness-points-holes-help = Гадаргуутай хамт сонгосон цооногууд, эсвэл сонгоогүй бол ачаалсан бүх цооног. Давхаргыг бүртгэсэн цооног бүр нэг цэг өгнө.
thickness-points-no-pairs = Байхгүй
thickness-points-note = Сонгосон гадаргуугийн үеллэгт перпендикуляр чиглэлд цооног бүр дээр давхаргын жинхэнэ зузааныг хэмжинэ.
thickness-points-pairs = Хэмжсэн хосууд
thickness-points-pairs-help = Заавал биш. Хээр хэмжсэн дээд ба доод хучааны цэгүүд, id, roof_x, roof_y, roof_z, floor_x, floor_y, floor_z баганатай CSV хэлбэрээр.
thickness-points-field-measurements = Хээрийн хэмжилт
thickness-points-every-hole = Ажлын үеийг агуулсан ачаалсан бүх цооног ({ $datasets } өгөгдлийн багц)
thickness-points-side-note = Тал: сонгосон гадаргуу давхаргын дээд хучаа уу, доод хучаа уу?
thickness-points-surface = Гадаргуу
thickness-points-surface-help = Цонхыг нээх үед сонгосон гадаргуу. Цооног бүр дээрх түүний налуу үеллэгийг өгнө.
thickness-points-table-surface = { $count } цэг, { $surface }-ийг лавлагаа болгон хэмжсэн
thickness-points-table-title = Зузааны цэгүүд: { $name }
thickness-points-then-surface = Дараа нь нөгөө гадаргууг үүсгэх
thickness-points-then-surface-help = Цэгүүдийг үүсгэсний дараа тэдгээрээс давхаргын нөгөө гадаргууг үүсгэнэ. Зузааны гадаргуунууд мөн адил тусдаа хийнэ.

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = Эхлээд холбох цооногийн өгөгдлийн багцыг сонгоно уу
tie-in-count-connector-s = { $count } холболт
tie-in-delete-tie-ins = Холболтуудыг устгах
tie-in-deleted-count-selected-tie-connector = Сонгосон { $count } холболтыг устгалаа
tie-in-hole = цооног
tie-in-initiation-point-lifted-from-name = Дэлбэлгээ эхлүүлэх цэгийг { $name }-с авлаа
tie-in-initiation-point-set-name-delay = Дэлбэлгээ эхлүүлэх цэгийг { $name } дээр { $delay } мс саатлаар тохируулав
tie-in-select-delay-product-palette-before = Цооногуудыг холбохоос өмнө палитраас саатлын бүтээгдэхүүн сонгоно уу
tie-in-tied-connectors = { $product }-р { $delay } мс саатлаар { $count } холболт үүсгэлээ
tie-in-tied-connectors-replacing = { $product }-р { $delay } мс саатлаар { $count } холболт үүсгэлээ, { $replaced }-г сольсон

## Toolbar strings

toolbar-fill-type = Дүүргэлтийн төрөл

## Toolbars strings

toolbars-auto-bench = Авто-уступ
toolbars-bezier-polyline = Безье полилиниа
toolbars-chamfer-polyline-corners = Полилиниагийн буланг фасклах
toolbars-create-text = Текст үүсгэх
toolbars-cursor-regular = Заагч: Энгийн
toolbars-cursor-snap-line = Заагч: Шугамд наах
toolbars-cursor-snap-point = Заагч: Цэгт наах
toolbars-cursor-snap-surface = Заагч: Гадаргуунд наах
toolbars-delete-points = Цэгүүдийг устгах
toolbars-edit-vertex = Оройг засах
toolbars-explode-polyline-lines = Полилиниаг шугам болгон задлах
toolbars-fuse-polylines = Полилиниаг нэгтгэх
toolbars-insert-points-crossings = Огтлолцол дээр цэг оруулах
toolbars-measure-distance = Зай хэмжих
toolbars-new-layer = Шинэ давхарга
toolbars-reverse-strings = Шугамын чиглэлийг урвуулах
toolbars-split-polyline-points = Полилиниаг цэгээр хуваах
toolbars-strike-dip = Чиг ба уналт
toolbars-tool-not-available-section-view = { $tool } - огтлолын харагдацад боломжгүй

## Tri strings

tri-sampling-method-help = Уян хатан арга нь хавтгайд тохируулах алдаагаар нарийн төвөгтэй рельеф дээр орой цэгийг төвлөрүүлдэг; жигд арга тэдгээрийг жигд тараана. Ирээдүйд илүү олон арга нэмэгдэж болно.
tri-adaptive-quadtree = Уян хатан (quadtree)
tri-axis-range = { $axis } хязгаар
tri-base-topology-will-receive-pit = Карьер эсвэл овоолгын хэлбэрийг хүлээн авах суурь топологи.
tri-boundary-polyline = Хилийн полилиниа
tri-bridge-gaps-help = Гадаргуугийн хөндлөн энэ хэмжээнээс нарийн завсар болон хилийн хотгорыг холбоно. 0 нь ойролцоогоор түүврийн эсийн хэмжээ хүртэлх завсрыг холбосон хэвээр байна; том утга илүү том нүхийг дүүргэж хилийн хотгорыг элэгдүүлнэ.
tri-budget = Төсвийг дараах байдлаар
tri-cancel-pick = Сонголтыг цуцлах
tri-candidate-detail = Нэр дэвшигчийн нарийвчлал
tri-candidate-fine-cells-per-budgeted = Төсөвлөгдсөн орой цэг бүрт ногдох нэр дэвшигч нарийн эсийн тоо. Өндөр утга уян хатан түүврлэгчид нарийвчлал байрлуулах илүү эрх чөлөө өгдөг ч бүтэхэд удаан байдаг.
tri-cap-surface-share-source-points = Гадаргууг эх цэгүүдийн эзлэх хувиар эсвэл орой цэгийн тодорхой тоогоор хязгаарлана.
tri-choose-input-clicking-loaded-surface = Харагдах цонхон дахь ачаалагдсан гадаргуу дээр товшиж энэ оролтыг сонгоно уу
tri-choose-which-side-reference-topology = Хамтын XY талбайн доторх лавлагаа топологийн аль талыг гадаргуунаас хасахыг сонгоно уу.
tri-clip = Огтлох
tri-clip-creates-new-triangulation-name = Огтлолт нь энэ нэртэй шинэ триангуляц үүсгэнэ; эх гадаргуу өөрчлөгдөхгүй.
tri-clip-surface-polyline = Гадаргууг полилиниагаар огтлох
tri-clip-to-surface = Гадаргуугаар тайрах
tri-clip-to-surface-targets = Давхарга
tri-clip-to-surface-targets-help = Давхаргын дээврийн хэсэг ба ул, харилцах цонхыг нээхэд сонгогдсон хоёр торон гадаргуу. Өндөр нь дээврийн хэсэг. Тайрах нь шинэ дээврийн хэсэг, ул, биет үүсгэнэ; эх хувь нь хэвээр үлдэнэ.
tri-clip-to-surface-upper = Доор үлдээх
tri-clip-to-surface-upper-help = Энэ хязгаараас дээш юу ч үлдэхгүй. Зөвхөн дээврийн хэсэг дээш гарсан газарт улт хүрэх хүртэл хязгаар дээр тэгшлэн тавина; ул бас гарсан газарт давхаргын тэр хэсгийг хасна. Зөвхөн доороос тайрах бол хоосон үлдээнэ үү.
tri-clip-to-surface-lower = Дээр үлдээх
tri-clip-to-surface-lower-help = Энэ хязгаараас доош юу ч үлдэхгүй. Зөвхөн ул доош орсон газарт дээврийн хэсэгт хүрэх хүртэл хязгаар дээр тэгшлэн тавина; дээврийн хэсэг бас орсон газарт давхаргын тэр хэсгийг хасна. Зөвхөн дээрээс тайрах бол хоосон үлдээнэ үү.
tri-clip-to-surface-from-surface = Гадаргуу
tri-clip-to-surface-from-level = RL
tri-clip-to-surface-from-depth = Гадаргуугаас доорх гүн
tri-clip-to-surface-surface = Гадаргуу
tri-clip-to-surface-surface-help = Энэ хязгаарыг тогтоох гадаргуу. Энд сонгох эсвэл харагдац дээр заана уу.
tri-clip-to-surface-ground-help = Гүнийг доош хэмжих гадаргуу, ихэвчлэн газрын гадаргуу. Энд сонгох эсвэл харагдац дээр заана уу.
tri-clip-to-surface-level = RL (м)
tri-clip-to-surface-level-help = Метрээрх түвшин. Хязгаар хаа сайгүй энэ өндөрт тэгш байна.
tri-clip-to-surface-level-invalid = RL нь метрийн тоо байх ёстой
tri-clip-to-surface-depth = Гүн (м)
tri-clip-to-surface-depth-help = Дээрх гадаргуугаас доош метр. Орд бүрт өөр бөгөөд төсөлтэй хамт хадгалагдана.
tri-clip-to-surface-note = Эхлээд Доор үлдээх, дараа нь Дээр үлдээх хэрэглэгдэнэ. Дээврийн хэсэг ба ул хязгаар дээр нийлсэн газраа төгсөж, тэдгээрийн хооронд битүү биет үүснэ.
tri-closed-pit-stockpile-solid-whose = Ил гарсан хил нь үр дүнд орох хаалттай карьер эсвэл овоолгын хатуу бие.
tri-cloud-carries-no-classifications-so = Энэ үүлд ангилал байхгүй тул цэг бүрээр гадаргуу үүсгэнэ. Нүцгэн газрыг сэргээхийн тулд газрын шүүлтүүрээр дамжуулсан LAS/LAZ файл импортлоно уу.
tri-create-new-layer-contours-append = Изолиниад зориулж шинэ давхарга үүсгэх, эсвэл идэвхтэй төслийн байгаа давхаргад нэмэх.
tri-cut-topology-pit-shell = Топологийг карьерийн бүрхүүлээр тайрах
tri-e-g-design-trimmed = ж: design_trimmed
tri-e-g-mysurf-cut = ж: mysurf_cut
tri-e-g-mysurf-slice = ж: mysurf_slice
tri-e-g-surface-contour = ж: surface_contour
tri-e-g-topo-cut = ж: topo_cut
tri-e-g-topo-pit = ж: topo_with_pit
tri-exact-number-surface-vertices-target = Зорьж буй гадаргуугийн орой цэгийн тодорхой тоо. Хэт өндөр утга удаан бүтэж, ихээхэн санах ой ашиглана.
tri-existing-ground-topology-will-cut = Карьерийн бүрхүүлээр тайрагдах одоо байгаа газрын гадаргын топологи.
tri-fill-holes-up = Дараах хүртэлх нүхийг дүүргэх
tri-generate = Үүсгэх
tri-generate-contour-lines = Изолиниа үүсгэх
tri-generate-upper-surface = Дээд гадаргуу үүсгэх
tri-ground-points-only = Зөвхөн газрын цэгүүд
tri-hide-unload-sources = Нуугаад эх сурвалжийг буулгах
tri-higher-edge-will-enforced-each = Зөрчил бүрт өндөр ирмэг давамгайлна. Доод зөрчилдсөн сегментүүдийг эвдрэлийн шугам болгон үл хэрэгсэх бөгөөд гадаргуу тэдгээр хэсгээр интерполяцлана. Эх полилиниа өөрчлөгдөхгүй.
tri-breaklines-cross = Тодруулсан эвдрэлийн шугамын ирмэгүүд төлөвлөгөөнд өөр өөр өндөрлөгт огтлолцож эсвэл давхцаж байна. Нэг рельефийн гадаргуу хоёуланг нь дагаж чадахгүй.
tri-intervals-colours = Интервал ба өнгө
tri-keep-clipped-topology-included-shape = Огтолсон топологи болон оруулсан хэлбэрийг нэг объект болгож нэгтгэхийн оронд тусдаа триангуляц болгож хадгалах.
tri-keep-inside-discards-surface-outside = «Дотор талыг хадгалах» нь полилиниагийн гадна талын гадаргууг хаяна. «Гадна талыг хадгалах» нь гадаргуунаас полилиниа хэлбэртэй нүх огтолно.
tri-keeps-only-surface-within-polyline = Зөвхөн полилиниагийн хилийн доторх гадаргууг хадгална.
tri-keep-surface-relation-help = Гадаргууг топологитой харьцах байдлаар ({ $relation }) түүний XY хамрах хүрээнд хадгална.
tri-layer-already-exists-select-above = Тэр давхарга аль хэдийн байна; дээрээс сонгох эсвэл өөр нэр сонгоно уу.
tri-limit-z-range = Z-ийн мужийг хязгаарлах
tri-major = Гол
tri-max-edge-length = Дээд ирмэгийн урт
tri-merge = Нэгтгэх
tri-method = Арга
tri-min = Доод
tri-minimum-maximum-elevations-retained = Гаралтын гадаргуунд хадгалагдах доод болон дээд өндөрлөг. Доод хэмжээ дээд хэмжээнээс бага байх ёстой.
tri-minor = Туслах
tri-contour-interval-help = «Туслах» энгийн изолиниаг удирдана. «Гол» онцлон харуулах изолиниаг удирдах бөгөөд Туслахаас багагүй интервал ашиглах ёстой.
tri-move-cursor-over-loaded-surface = Заагчийг ачаалагдсан гадаргуу дээгүүр аваачна уу.
tri-slice-output-name-help = Өндөрлөгөөр огтлогдсон гаралтын гадаргуунд оноох нэр.
tri-name-assigned-merged-topology-pit = Нэгтгэсэн топологи ба карьер/овоолгын үр дүнд оноох нэр.
tri-name-assigned-newly-created-contour = Шинээр үүсгэсэн изолиниагийн давхаргад оноох нэр.
tri-reconstruct-output-name-help = Дахин байгуулсан триангуляцад оноох нэр.
tri-name-assigned-topology-after-pit = Карьерийн бүрхүүлийг тайрсны дараах топологид оноох нэр.
tri-name-assigned-trimmed-output-surface = Тайрагдсан гаралтын гадаргуунд оноох нэр.
tri-nearby-breakline-vertices-do-not = Ойролцоох эвдрэлийн шугамын орой цэгүүд яг ижил байрлалд тохирохгүй байгаа тул гадаргууг триангуляцлах боломжгүй.
tri-new-layer = Шинэ давхарга
tri-new-layer-name = Давхаргын шинэ нэр
tri-no-boundary-selected = Хил сонгоогүй байна
tri-no-point-cloud-selected = Цэгэн үүл сонгоогүй байна
tri-no-surface-selected = Гадаргуу сонгоогүй байна
tri-once-clip-succeeds-unload-source = Огтлолт амжилттай болмогц, зөвхөн огтолсон үр дүн дүр зураг дээр үлдэхээр эх гадаргууг буулгана.
tri-once-cut-succeeds-unload-original = Тайрах үйлдэл амжилттай болмогц, зөвхөн тайрсан үр дүн дүр зураг дээр үлдэхээр анхны топологийг буулгана. Карьерийн бүрхүүл ачаалагдсан хэвээр байна.
tri-once-merge-succeeds-unload-source = Нэгтгэлт амжилттай болмогц эх топологи болон хатуу биеийг буулгаж, зөвхөн нэгтгэсэн үр дүнг дүрслэлд үлдээх.
tri-once-slice-succeeds-unload-source = Огтлолт амжилттай болмогц, зөвхөн огтолсон үр дүн дүр зураг дээр үлдэхээр эх гадаргууг буулгана.
tri-once-trim-succeeds-unload-surface = Тайралт амжилттай болмогц, зөвхөн үр дүн дүр зураг дээр үлдэхээр тайрсан гадаргууг буулгана. Топологи ачаалагдсан хэвээр байна.
tri-only-loaded-pickable = Зөвхөн ачаалагдсан триангуляцыг сонгож болно.
tri-operation = Үйлдэл
tri-output-layer = Гаралтын давхарга
tri-percentage = Хувь
tri-percentage-cloud = Үүлний хувь
tri-pick-from-view = Харагдацаас сонгох
tri-pit-design-surface-only-areas = Карьерийн зураг төслийн гадаргуу. Топологиос доогуур ухагдах хэсгүүдийг л огтлолтод ашиглана.
tri-pit-shell = Карьерийн бүрхүүл
tri-pit-stockpile-solid = Карьер/овоолгын хатуу бие
tri-recommended-weld-retry = Санал болгох: Гагнаад дахин оролдох
tri-reconstruct-ground-only-help = Нүцгэн газар гэж ангилсан цэгүүдээс сэргээж, ургамалжилт, барилга, тоног төхөөрөмж, шуугианыг хаяна. Үүлний цэг бүрээр гадаргуу үүсгэхийн тулд үүнийг унтраана уу.
tri-reconstruct-help = Цэгэн үүлээс триангуляцлагдсан рельефийн гадаргуу дахин байгуулна. Уян хатан түүврлэгч орой цэгийн төсвийг газар хамгийн нарийн төвөгтэй хэсэгт зарцуулж, тэгш хэсгийг сийрэг хэвээр үлдээнэ.
tri-reduce-budget-candidate-detail-if = Хэрэв таны компьютерт RAM бага байвал төсөв эсвэл нэр дэвшигчийн нарийвчлалыг багасгана уу.
tri-reference-topology-help = Нөгөө гадаргуу хаана тайрагдахыг тодорхойлдог лавлагаа топологи.
tri-reject-reconstructed-triangle-edges = Энэ зайнаас урт дахин байгуулсан гурвалжны ирмэгийг татгалзана. Ирмэгийн уртад хязгаар тавихгүй бол 0 ашиглана уу.
tri-remove-inside-help = Полилиниагийн хилийн доторх гадаргууг устгаж, үлдсэнийг хадгална.
tri-removes-topology-where-pit-shell = Карьерийн бүрхүүл доогуур ухсан газарт топологийг устгаж, бүрхүүл нүхийг дүүргэнэ. Холбоос нь гадаргуудын хоорондох бодит 3D харьцах шугамыг дагана; бүрхүүлийн газрын түвшнээс дээш гарсан хэсгийн доорх топологи хадгалагдана.
tri-result = Үр дүн
tri-save-two-entities = Хоёр тусдаа объект болгон хадгалах
tri-select = Сонгох…
tri-selected-closed-polyline-whose-xy = Сонгосон хаалттай полилиниа, түүний XY хил нь огтлох талбарыг тодорхойлно.
tri-selected-point-cloud-whose-points = Сонгосон цэгэн үүл, түүний цэгүүдээс рельефийн гадаргууг сэргээнэ. Өөрийг сэргээхийн тулд цонхыг хаана уу.
tri-selected-surface-from-which-contour = Сонгосон гадаргуу, түүнээс изолиниа үүсгэнэ. Өөрөөс нь үүсгэхийн тулд цонхыг хаана уу.
tri-selected-surface-which-will-clipped = Сонгосон гадаргуу, үүнийг огтолно. Өөрийг огтлохын тулд цонхыг хаана уу.
tri-slice-source-help = Сонгосон гадаргуу, түүний өндөрлөгийн мужийг огтолно. Өөрийг огтлохын тулд цонхыг хаана уу.
tri-share-source-points-keep-fractions = Хадгалах эх цэгийн эзлэх хувь. 0.125% гэх мэт бутархай утга зөвшөөрөгдөнө.
tri-slice-triangulation-z-range = Триангуляцыг Z мужаар огтлох
tri-solution-generate-upper-surface = Шийдэл: Дээд гадаргуу үүсгэх
tri-surface-trim = Тайрах гадаргуу
tri-target-surface-help = Өөрчлөгдөх гадаргуу; сонгосон топологи өөрчлөгдөлгүй үлдэнэ.
common-percent-suffix = %
tri-topology = Топологи
tri-triangulation-failed = Триангуляц амжилтгүй боллоо
tri-trim = Тайрах
tri-trim-topology = Топологиор тайрах
tri-uniform-grid = Жигд тор
tri-unload-source-surface = Эх гадаргууг буулгах
tri-unload-source-topology = Эх топологийг буулгах
tri-up-target-point-count-points = { $point_count } цэгийн дотроос { $target } хүртэл нь гадаргуугийн орой цэг болно ({ $percent }%).
tri-use-full-surface-elevation-range = Гадаргуугийн бүх өндрийн мужийг ашиглах
tri-vertex-count = Орой цэгийн тоо
tri-vertices-within-5-cm-xy = XY болон Z чиглэлд 5 см-ийн дотор байгаа орой цэгүүд энэ триангуляцын хувьд нэг байрлалыг хуваалцана. Энэ нь үүсгэсэн гадаргууг орон нутгийн хэмжээнд 5 см хүртэл шилжүүлж болно; эх полилиниа өөрчлөгдөхгүй.
tri-weld-retry = Гагнаад дахин оролдох
tri-when-enabled-generate-contours-only = Идэвхжүүлсэн үед зөвхөн заасан доод болон дээд өндөрлөгийн хооронд изолиниа үүсгэнэ.

## Ui strings

ui-choose-offset-side = Шилжилтийн талыг сонгох
ui-choose-relimit-side = Хязгаарлах талыг сонгох
ui-click-circle-centre = Тойргийн төвийг товших
ui-click-closed-polyline-use-blast = Тэсэлгээний хэлбэр болгон ашиглах хаалттай полилиниа дээр товшино уу
ui-click-collar-add-edit-initiation = Дэлбэлгээ эхлүүлэх цэгийг нэмэх эсвэл засахын тулд амсар дээр товшино уу
ui-click-first-point-slice-line = Огтлолын шугамын эхний цэгийг товших
ui-click-first-vertex = Эхний орой цэгийг товших
ui-click-perimeter-point-type-radius = Периметрийн цэг дээр товших эсвэл радиус оруулна уу
ui-click-second-point-slice-line = Огтлолын шугамын хоёр дахь цэгийг товших
ui-click-second-vertex = Хоёр дахь орой цэгийг товших
ui-click-use-pointer-radius = эсвэл заагчийн радиусыг ашиглахын тулд товшино уу
ui-could-not-copy-text-browser = Текстийг хөтчийн санах ойд хуулж чадсангүй: { $error }
ui-dip-horizontal-no-strike = { $dip } (хэвтээ, чиггүй)
ui-distance-meters = { $distance } метр
ui-drag-ring-type-azimuth-dip = Цагирагийг чирэх эсвэл азимут ба уналтыг бичих
ui-each-hole-turns-about-its = цооног бүр өөрийн амсрын эргэн тойронд эргэнэ
ui-enter-positive-decimal-radius = Эерэг бутархай радиус оруулна уу
ui-esc-cancels = Esc цуцлана
ui-no-delay-product-tie = Холбох саатлын бүтээгдэхүүн алга
ui-press-enter-use-typed-radius = Бичсэн радиусыг ашиглахын тулд Enter дарах
ui-right-click-delay-palette-heading = саатлын палитрын гарчиг дээр хулганы баруун товчоор товшиж нэмнэ үү
ui-select-designs = Зураг төслүүдийг сонгох
ui-select-drill-hole = Цооног сонгох
ui-select-endpoint-join = Холбох үзүүр цэгийг сонгох
ui-pick-first-plane-point = Хавтгай дээрх эхний цэгийг сонгоно уу
ui-drape-follows-triangles = Шугамууд оройнуудынхаа хооронд гадаргууг дагана
ui-pick-second-plane-point = Хавтгай дээрх хоёр дахь цэгийг сонгоно уу
ui-pick-third-plane-point = Хавтгай дээр эхний хоёрын шугамаас гадуур гурав дахь цэгийг сонгоно уу
ui-select-first-crest-toe-point = Эхний оргил/ёроолын цэгийг сонгох
ui-select-item = Зүйл сонгох
ui-select-line-fuse = Нэгтгэх шугамыг сонгох
ui-select-line-polyline = Шугам эсвэл полилиниа сонгох
ui-select-line-relimit = Хязгаарлах шугамыг сонгох
ui-select-next-line-fuse = Нэгтгэх дараагийн шугамыг сонгох
ui-select-opposite-berm-point = Бермийн эсрэг цэгийг сонгох
ui-select-point = Цэг сонгох
ui-select-polyline = Полилиниа сонгох
ui-select-polyline-open-line = Полилиниа эсвэл задгай шугам сонгох
ui-select-polyline-vertex = Полилиниагийн орой цэг сонгох
ui-select-second-crest-toe-point = Хоёр дахь оргил/ёроолын цэгийг сонгох
ui-select-second-split-point = Хоёр дахь хуваах цэгийг сонгох
ui-select-split-point = Хуваах цэг сонгох
ui-select-topologies = Топологи сонгох
ui-slice-view = Огтлолын харагдац
ui-strike-dip = { $strike }° чиг · { $dip }
ui-value-dip = { $value }° уналт
viewport-1-1-true-shape = 1:1, жинхэнэ хэлбэр
viewport-1-ratio = 1:{ $ratio }

## Viewport strings

viewport-all-total-categories-keep-their = Бүх { $total } ангилал өнгөө хадгална; зөвхөн эхний { $shown } нь тодорхой ялгаатай дүрслэгдэнэ
viewport-axis-maximum = { $axis } дээд
viewport-axis-minimum = { $axis } доод
viewport-azimuth-dip = Азимут { $azimuth }, уналт { $dip }
viewport-back-whole-log = Бүх каротаж руу буцах.
viewport-bar-blast-timeline-placeholder = Тэсэлгээний хугацааны шугам [ТОДОРХОЙГҮЙ]
viewport-bar-burden-relief-heatmap-placeholder = Бурдений хөнгөрөлтийн дулааны зураг [ТОДОРХОЙГҮЙ]
viewport-bar-cinematic-view = Кино харагдац
viewport-bar-color = Өнгө:
viewport-bar-contours-equal-time-placeholder = Тэнцүү хугацааны изолиниуд [ТОДОРХОЙГҮЙ]
viewport-bar-disable-cinematic-view = Кино харагдацыг идэвхгүй болгох
viewport-bar-disable-flying-mode = Нисэх горимыг унтраах
viewport-bar-disable-x-ray-vision = Рентген харагдацыг унтраах
viewport-bar-drill-holes = Цооногууд:
viewport-bar-enable-flying-mode = Нисэх горимыг асаах
viewport-bar-enable-x-ray-vision = Рентген харагдацыг асаах
viewport-bar-unhide-all = Нуусныг бүгдийг харуулах: ачаалсан давхаргууд дахь нуусан объектуудыг харуулах
viewport-bar-exit-slice-view = Огтлолын харагдацаас гарах
viewport-bar-fill = Дүүргэлт:
viewport-bar-fix-centre-rotation = Эргэлтийн төвийг тогтоох
viewport-bar-hide-borehole-inspector = Цооногийн шалгагчийг нуух
viewport-bar-hide-classification = Ангиллыг нуух
viewport-bar-hide-points = Цэгүүдийг нуух
viewport-bar-hide-rl-grid = RL торыг нуух
viewport-bar-hide-wireframes = Торон дүрсийг нуух
viewport-bar-hide-xy-grid = XY торыг нуух
viewport-bar-release-centre-rotation = Эргэлтийн төвийг суллах
viewport-bar-reset-view-plan-over-centre = Харагдацыг сэргээх: эргэлтийн төвийн дээрээс харах, бүгдийг тааруулахын тулд дахин дарна уу
viewport-bar-reset-view-plan-same-distance = Харагдацыг сэргээх: ижил зайнаас дээрээс харах, бүгдийг тааруулахын тулд дахин дарна уу
viewport-bar-show-borehole-inspector = Цооногийн шалгагчийг харуулах
viewport-bar-show-classification = Ангиллыг харуулах
viewport-bar-show-points = Цэгүүдийг харуулах
viewport-bar-show-rl-grid = RL торыг харуулах
viewport-bar-show-wireframes = Торон дүрсийг харуулах
viewport-bar-show-xy-grid = XY торыг харуулах
viewport-bar-vertical-slice-view = Босоо огтлолын харагдац
viewport-blank = (хоосон)
viewport-choose-active-block-model-variable = Идэвхтэй блокийн загварын хувьсагчийг сонгох
viewport-choose-variable = Хувьсагч сонгох
viewport-click-edit-color-right-click = Товшиж өнгийг засах; баруун товшиж устгах
viewport-click-type-boundary-s-value = Энэ хилийн утгыг бичихийн тулд товшино уу
viewport-colour-mapping = Өнгийн харгалзаа
viewport-count-categories = { $count } ангилал
viewport-count-category = { $count } ангилал
viewport-depth-m-hole-end = { $depth } м цооногийн төгсгөл
viewport-double-click-add-boundary-here = Энд хил нэмэхийн тулд давхар товшино уу
viewport-drag-move-middle-click-toggles = Чирж зөөх · Дунд товшиж ≤ солих
viewport-drag-move-right-click-remove = Чирж зөөх · Баруун товшиж устгах · Дунд товшиж ≤ солих
viewport-drag-spin-view-around-hole = Цооногийн эргэн тойронд харагдацыг эргүүлэхийн тулд чирнэ үү. Хойд тийш харуулахын тулд давхар товшино уу.
viewport-e = E
viewport-edit-category-colour = Энэ ангиллын өнгийг засах
viewport-edit-colour-used-empty-values = Хоосон утганд ашиглах өнгийг засах
viewport-empty = (хоосон)
viewport-empty-hidden = (хоосон · нуугдсан)
viewport-field-has-no-strat-column = Энэ талбарт страт багана одоохондоо алга; шалгагчийн Багана табд үүсгэнэ үү
viewport-filter-variables = Хувьсагчийг шүүх
viewport-fit-hole-track = Цооногийг мөрөнд тааруулах
viewport-from = { $from }-с { $to }
viewport-from-m = { $from }-с { $to } м
viewport-h-1-ratio = Х 1:{ $ratio }
viewport-hole-has-no-trace-draw = Энэ цооногт зурах мөр байхгүй.
viewport-interval-data = Интервалын өгөгдөл
viewport-intervals = Интервалууд
viewport-m-from-collar-toward-bearing = амсраас { $bearing }° чиглэлд м
viewport-navigation-hint = Дунд товч чирж зөөх · Гүйлгэж томруулах
viewport-navigation-hint-detach = Дунд товч чирж зөөх · Гүйлгэж томруулах · Товшиж салгах
viewport-move-all-down = Бүгдийг доош
viewport-move-all-up = Бүгдийг дээш
viewport-move-down-from-here = Эндээс доош
viewport-move-up-from-here = Эндээс дээш
viewport-n = N
viewport-name-not-in-strat-column = Энэ нэр талбарын страт баганад байхгүй тул шилжүүлж эхлэх горизонт алга
viewport-no-data-variable = Энэ хувьсагчийн өгөгдөл алга
viewport-no-density-log-hole = Энэ цооногт нягтын каротаж алга
viewport-no-downhole-geophysics-hole = Энэ цооногт цооногийн геофизик алга
viewport-no-gamma-log-hole = Энэ цооногт гаммын каротаж алга
viewport-no-matches = Тохирол алга
viewport-no-trace = Мөр алга
viewport-no-usable-range = (ашиглах боломжтой муж алга)
viewport-not-logged = Бүртгээгүй
viewport-orientation-source = Чиглэлийн эх үүсвэр
viewport-rebuild-variable-s-colours-from = Энэ хувьсагчийн өнгийг өгөгдлөөс нь дахин байгуулах
viewport-rename-seam-in-every-hole = Бүх цооногт нэрийг өөрчлөх
viewport-rename-seam-in-this-hole = Энэ цооногт нэрийг өөрчлөх
viewport-reset = Сэргээх
viewport-restore-full-model-range = Загварын бүтэн мужийг сэргээх
viewport-roll-wheel-over-log-zoom = Давхаргыг томруулахын тулд каротаж дээр хулганы дугуйг эргүүлнэ үү. Цооногийг эргүүлэх, дагуу нь явахын тулд каротажийг чирнэ үү.
viewport-s = Х
viewport-sideways-scale = Хажуугийн масштаб
viewport-squeeze-sideways-just-enough-keep = Цооногийг харагдуулахад хүрэлцэхүйц хэмжээгээр хажуу тийш шахна. Хэзээ ч сунгахгүй.
viewport-trace-extent = Мөрийн хэмжээ
viewport-w = З
viewport-widen-panel-show-density = Нягтыг харуулахын тулд самбарыг өргөсгөнө үү
viewport-widen-panel-show-density-gamma = Нягт ба гаммыг харуулахын тулд самбарыг өргөсгөнө үү
viewport-widen-panel-show-gamma = Гаммыг харуулахын тулд самбарыг өргөсгөнө үү
charging-edit-charge-product = Цэнэглэх бүтээгдэхүүнийг засах
charging-new-charge-product = Шинэ цэнэглэх бүтээгдэхүүн
charging-explosive-decks-add-mass-primed-stemming = Тэсрэх бодисын хэсэг масс нэмж, өдөөгчтэй байна; түгжээс ба агаарын хэсэг зөвхөн урттай.
charging-density = Нягт
charging-density-hint = Цооног доторх нягт. Метр тутмын масс нь үүнийг цооногийн хөндлөн огтлолын талбайгаар үржүүлсэнтэй тэнцүү.
charging-another-product-already-has-name = Өөр бүтээгдэхүүн аль хэдийн энэ нэртэй байна
charging-edit-charge-rule = Цэнэглэх дүрмийг засах
charging-new-charge-rule = Шинэ цэнэглэх дүрэм
charging-decks-collar-toe = Хэсгүүд, амсраас ёроол хүртэл
charging-priming = Өдөөлт
charging-preview = Урьдчилан харах
charging-preview-use-pattern-hole = Сүлжээний медиан цооногийг ашиглах
charging-preview-active-pattern-median-hole = Идэвхтэй сүлжээний медиан цооног дээр урьдчилан харах
charging-fixed-decks-longer-than-hole = Тогтмол хэсгүүд энэ цооногоос урт байна
charging-mass-kg-explosive = { $mass } kg тэсрэх бодис
charging-rate-kg-m = { $rate } kg/м
charging-count-primer = { $count } өдөөгч
charging-another-rule-already-has-name = Өөр дүрэм аль хэдийн энэ нэртэй байна
charging-save-reload-count-hole = Хадгалж { $count } цооногийг дахин ачаалах
charging-length = Урт
charging-rest-length-m = үлдсэн · { $length } м
charging-rest = үлдсэн
charging-deck-takes-whatever-length-fixed-decks = Энэ хэсэг тогтмол хэсгүүдийн үлдээсэн бүх уртыг авна. Дүрэм бүрт нэг хэсэг л дүүргэнэ.
charging-remove-deck = Хэсэг устгах
charging-add-deck = Хэсэг нэмэх
charging-downhole-delay = Цооног доторх саатал
charging-hole-detonator-hole-fires-long-after = Цооног доторх детонатор. Цооног гадаргуугийн дохио ирснээс хойш энэ хугацааны дараа дэлбэрнэ.
charging-primer-height = Өдөөгчийн өндөр
charging-how-far-above-base-each-explosive = Тэсрэх бодисын хэсэг бүрийн ёроосоос өдөөгч хэр дээр байрлах.
charging-booster = Бүст
charging-cast-booster-mass-each-primer = Өдөөгч бүр дэх цутгамал бүстийн масс.
charging-count-rule-load-product-will-need = { $count } дүрэм энэ бүтээгдэхүүнийг ачаалдаг тул өөр бүтээгдэхүүн сонгох шаардлагатай болно.
charging-rule = Дүрэм
charging-holes-already-loaded-keep-their-charge = Түүгээр аль хэдийн цэнэглэсэн цооногууд цэнэгээ хадгална.
blast-burden-relief = Бурдений хөнгөрөлт
blast-ms-per-metre-last-neighbour-fire = сүүлд тэсрэх хөрш хүртэлх метр тутмын мс
blast-below-hole-fires-before-rock-front = Үүнээс доош бол цооног урд талын чулуулаг хөдлөхөөс өмнө дэлбэрнэ: шахуу.
blast-above-rock-front-has-long-gone = Үүнээс дээш бол урд талын чулуулаг аль хэдийн холдсон: сулрал, тасрал ба чулуу шидэгдэх эрсдэлтэй.
blast-tight = шахуу
blast-good = сайн
blast-slack = сул
blast-free-face = чөлөөт гадаргуу
blast-fires-at = Тэсрэх хугацаа
blast-empty-won-t-detonate = хоосон, дэлбэрэхгүй
blast-not-reached = хүрээгүй
blast-value-ms-m-from-hole = { $hole }-с { $value } мс/м
blast-fires-first-free-face = эхэлж дэлбэрнэ: чөлөөт гадаргуу
blast-relief = Хөнгөрөлт
blast-explosive = Тэсрэх бодис
blast-powder-factor = Тэсрэх бодисын хувийн зарцуулалт
blast-not-loaded = Цэнэглээгүй
blast-count-primer-delay-ms-downhole = { $count } өдөөгч · цооног доторх { $delay } мс
blast-set-initiation-point-tie-holes-play = Тэсэлгээг тоглуулахын тулд дэлбэлгээ эхлүүлэх цэг тогтоож, цооногуудыг холбоно уу
blast-pause = Түр зогсоох
blast-play = Тоглуулах
blast-back-start = Эхлэл рүү буцах
blast-duration-ms = { $duration } мс-ийн
blast-real-time = Бодит хугацаа
blast-mic-limit = MIC хязгаар
blast-most-explosive-allowed-detonate-any-8 = Энэ талбайд дурын 8 мс-д тэсрэхийг зөвшөөрсөн тэсрэх бодисын дээд хэмжээ. Үүнээс давсан цонхнуудыг тэмдэглэнэ.
blast-no-holes-loaded-surface-signal-plays = Цооног цэнэглээгүй: гадаргуугийн дохио тоглох боловч юу ч дэлбэрэхгүй. Цооног цэнэглэх хэрэгслээр цооногуудыг цэнэглэнэ үү.
blast-now-holes-hole = Одоо: { $holes } цооног
blast-in-8-ms = 8 мс-д
blast-peak-mass-kg-time-ms = Оргил { $mass } kg, { $time } мс-д
blast-peak-holes-hole-time-ms = Оргил { $holes } цооног, { $time } мс-д
blast-peak-over-limit = , { $over } kg-аар хэтэрсэн
blast-peak-within-limit = , хязгаар дотор
blast-top-surface-signal-lighting-each-downline = Дээд: цооног тус бүрийн доош шугамыг асаах гадаргуугийн дохио. Доод: тэсрэлтүүд. Тоглуулах заагчийг зөөхийн тулд товшино уу эсвэл чирнэ үү.
products-charge-rules = Цэнэглэх дүрмүүд
products-new-rule = Шинэ дүрэм
products-charge-products = Цэнэглэх бүтээгдэхүүнүүд
products-new-rule-default-name = Шинэ дүрэм
products-no-rules = Дүрэм алга
products-load-selected-holes-count = Сонгосон цооногийг цэнэглэх ({ $count })
products-unload-selected-holes-count = Сонгосон цооногийг цэнэггүй болгох ({ $count })
products-edit-rule = Дүрмийг засах
products-duplicate-rule = Дүрмийг хувилах
products-delete-rule = Дүрмийг устгах
products-fill-product = дүүргэлт { $product }
products-primer-offset-m-off-each-explosive = Өдөөгч тэсрэх бодисын хэсэг бүрийн ёроосоос { $offset } м, { $booster } kg бүст, цооног доторх { $delay } мс
products-double-click-edit = Засахын тулд давхар товшино уу
products-edit-product = Бүтээгдэхүүнийг засах
blast-log-updated-charge-product-name = { $name } цэнэглэх бүтээгдэхүүнийг шинэчиллээ
blast-log-added-charge-product-name = { $name } цэнэглэх бүтээгдэхүүнийг нэмлээ
blast-log-updated-charge-rule-name = { $name } цэнэглэх дүрмийг шинэчиллээ
blast-log-added-charge-rule-name = { $name } цэнэглэх дүрмийг нэмлээ
blast-log-entry-no-longer-charge-library = Тэр бичлэг цэнэглэх сангад цаашид байхгүй байна
blast-log-deleted-name-from-charge-library = { $name }-г цэнэглэх сангаас устгалаа
blast-log-failed-save-charge-library-error = Цэнэглэх санг хадгалж чадсангүй: { $error }
blast-log-cannot-load-rule-problem = Энэ дүрмээр цэнэглэх боломжгүй: { $problem }
blast-log-count-hole-too-short-fixed-decks = { $count } цооног энэ дүрмийн тогтмол давхаргуудад хэт богино тул хэвээр нь орхив
blast-log-count-hole-have-no-depth-load = { $count } цооногт цэнэглэх гүн байхгүй
blast-log-count-loaded-hole-have-no-diameter = Цэнэглэсэн { $count } цооногийн диаметр байхгүй тул тэсрэх бодисын массыг мэдэх боломжгүй
common-charge-holes = Цооног цэнэглэх
blast-log-loaded-count-hole-rule = { $count } цооногийг { $rule }-ээр цэнэглэлээ
blast-log-unload-holes = Цооногийг цэнэггүй болгох
blast-log-unloaded-count-hole = { $count } цооногийг цэнэггүй болголоо
blast-log-select-holes-active-pattern-first = Эхлээд идэвхтэй сүлжээний цооногуудыг сонгоно уу
blast-log-rule-no-longer-charge-library = Тэр дүрэм цэнэглэх сангад цаашид байхгүй байна
blast-log-there-no-charge-rule-load-add = Цэнэглэх дүрэм алга: бүтээгдэхүүний самбарт нэг нэмнэ үү
blast-rule-stemming = Түгжээс
blast-rule-air-deck = Агаарын хэсэг
blast-rule-give-rule-name = Дүрэмд нэр өгнө үү
blast-rule-add-least-one-deck = Дор хаяж нэг хэсэг нэмнэ үү
blast-rule-only-one-deck-can-fill-rest = Цооногийн үлдсэн хэсгийг зөвхөн нэг хэсэг дүүргэж болно
blast-rule-deck-lengths-must-greater-than-zero = Хэсгийн урт тэгээс их байх ёстой
blast-rule-no-product-named-name = '{ $name }' нэртэй бүтээгдэхүүн алга
blast-rule-rule-needs-least-one-explosive-deck = Дүрэмд дор хаяж нэг тэсрэх бодисын хэсэг хэрэгтэй
state-save-charge-product = Цэнэглэх бүтээгдэхүүнийг хадгалах
state-save-charge-rule = Цэнэглэх дүрмийг хадгалах
state-delete-charge-library-entry = Цэнэглэх сангийн бичлэгийг устгах
ui-click-drag-over-holes-load-them = { $rule }-ээр цэнэглэхийн тулд цооногууд дээр товшино уу эсвэл чирнэ үү
ui-hold-shift-unload = цэнэггүй болгохын тулд Shift дарна уу
ui-no-charge-rule-load = Цэнэглэх дүрэм алга
ui-right-click-charge-rules-heading-add = нэмэхийн тулд Цэнэглэх дүрмүүд гарчиг дээр хулганы баруун товчийг дарна уу
omf-element-name-has-count-charge-naming = '{ $name }' элементэд өөрт нь цаашид агуулагдахгүй цооногуудыг нэрлэсэн { $count } цэнэг байна
