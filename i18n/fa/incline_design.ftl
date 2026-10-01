# فارسی. عبارت‌های موجود از کاتالوگ انگلیسی جایگزین می‌شوند.
common-cancel = لغو
common-clear = پاک کردن
common-close = بستن
common-fill = پر کردن
common-set = تنظیم
status-language = زبان
menu-file = فایل
menu-file-save-project = ذخیره پروژه
menu-file-save-project-as = ذخیره پروژه با نام...
menu-file-new-project = پروژه جدید...
menu-file-open-project = باز کردن پروژه...
menu-file-open-recent = باز کردن اخیر
menu-file-import = وارد کردن...
menu-file-export = صادر کردن...
menu-file-about = درباره { $app }...
menu-file-exit = خروج از برنامه
menu-view = نما
ws-production = تولید
ws-drill-and-blast = حفاری و انفجار
ws-geology = زمین‌شناسی
ws-planning = برنامه‌ریزی
dialog-rename-title = تغییر نام { $kind }
dialog-rename-field = نام جدید
dialog-rename-field-hint = الزامی
dialog-rename-submit = تغییر نام
dialog-delete-title = حذف { $kind }
dialog-delete-confirm = «{ $name }» از پروژه حذف شود؟
    این کار قابل بازگشت نیست.
confirm-delete-product =
    محصول «{ $name }» از پالت حذف شود؟
    این کار قابل بازگشت نیست.
about-read-full-licence = خواندن مجوز کامل ↗
about-source-code = کد منبع
about-website = وب‌سایت

## Completed canonical messages

menu-file-show-in-explorer = نمایش در Explorer
menu-file-show-in-folder = باز کردن پوشه حاوی فایل
menu-file-export-viewport-image = صادر کردن تصویر نمای دید...
menu-file-export-engineering-drawing = صادر کردن نقشه مهندسی...
ws-menubar-design = طراحی
ws-menubar-triangulation = مثلث‌بندی
ws-menubar-raster = رستر
ws-menubar-point-cloud = ابر نقاط
ws-menubar-block-model = مدل بلوکی
ws-menubar-drillholes = گمانه‌ها
ws-menubar-active-layer = لایه:
ws-menubar-design-insert-point = نقطه را وارد کنید
ws-menubar-design-insert-point-at-intersection = در تقاطع
ws-menubar-design-insert-point-at-elevation = در ارتفاع
ws-menubar-design-move-to = حرکت کن
ws-menubar-design-create-triangulation = ایجاد مثلث‌بندی
tri-create-title = ایجاد مثلث‌بندی
tri-create-type-label = نوع مثلث‌بندی
tri-create-type-help = سطح باز صفحه‌ای شبیه زمین ایجاد می‌کند. جسم صلب مشی کاملاً بسته ایجاد می‌کند و به ورودی‌ای نیاز دارد که بتواند مرزی آب‌بند تشکیل دهد.
tri-create-output-name = نام خروجی
tri-create-output-name-help = نامی که به مثلث‌بندی تولیدشده اختصاص داده می‌شود.
tri-create-output-name-hint = نام مثلث‌بندی
tri-create-run = مثلث سازی
tri-selection-selected = { $summary } انتخاب شد
tri-type-open-surface = سطح
tri-type-solid-closed = جامد
about-title = در مورد { $app }
drill-hole-colour-stop = توقف { $index }
properties-restore-defaults-tooltip = تنظیمات { $heading } را به حالت پیش فرض تنظیم کنید
ui-selected-count = { $count } انتخاب شد
ui-selected-objects = اشیاء { $count } انتخاب شده
ui-selected-polylines = { $count } چندخطی انتخاب شده
ui-invalid-axis-value = یک مقدار { $axis } معتبر را وارد کنید.
ui-selection-spans = انتخاب از { $min } تا { $max } می باشد.
confirm-delete-count = آیا مطمئن هستید که می خواهید آیتم های انتخاب شده { $count } را حذف کنید؟
confirm-delete-layer = لایه «{ $name }» و تمام اشیای روی آن حذف شود؟ این کار قابل بازگشت نیست.
plot-preview-pixels = { $width } × { $height } px در { $dpi } dpi
tri-estimated-memory = حداکثر حافظه تخمین زده شده ~{ $estimate }. { $detail }
block-grid-summary = شبکه: { $x } × { $y } × { $z } = { $count } بلوک
status-selected = انتخاب شده: { $count }
status-clip = کلیپ نزدیک / دور / Δ: { $near } / { $far } / { $delta } m

## Selection counts

tri-count-polylines =
    { $count ->
        [one] { $count } چندخطی
       *[other] { $count } چندخطی
    }
tri-count-strings =
    { $count ->
        [one] { $count } خط
       *[other] { $count } خط
    }
tri-count-points =
    { $count ->
        [one] { $count } نقطه
       *[other] { $count } نقطه
    }
tri-count-texts =
    { $count ->
        [one] { $count } شیء متنی
       *[other] { $count } اشیای متنی
    }
tri-count-objects =
    { $count ->
        [one] { $count } شیء
       *[other] { $count } اشیا
    }

## Reused existing project translations

## ترجمه‌های رابط کاربری تکمیل‌شده به‌صورت دستی
explorer-no-rasters = رستری وجود ندارد
slice-viewport-gestures = کشیدن با دکمه میانی: پن · کشیدن با دکمه راست: چرخش مداری · Shift+چرخ: راه رفتن · W/S: جابه‌جایی برش · Q/E: چرخاندن · Esc: خروج

## جزئیات محیط راه‌اندازی

## عیب‌یابی راه‌اندازی رندر

color-aci = ACI
color-aci-value = ACI { $index }
color-index = نمایه
color-rgb = RGB
color-opacity = کدری
color-edit = برای ویرایش رنگ کلیک کنید
color-saturation-value = اشباع و روشنایی
color-hue = فام
asset-loading = در حال بارگذاری داده‌های دارایی
asset-unloading = در حال تخلیه داده‌های دارایی
asset-load-failed = بارگذاری داده‌های دارایی ممکن نشد
asset-unload-failed = تخلیه داده‌های دارایی ممکن نشد
preferences-title = ترجیحات
context-text-colour = رنگ متن
context-polylines = چندخطی‌ها
context-points = نقاط
crs-unknown-ellipsoid = مدل زمینی ناشناخته «{ $name }» در این تعریف سامانه مختصات.
crs-no-ellipsoid = این تعریف سامانه مختصات مشخص نمی‌کند از چه مدل زمینی استفاده می‌کند.
crs-unknown-code = EPSG:{ $code } در ثبت سامانه‌های مختصات موجود نیست.
crs-transform-failed = یک مختصات قابل تبدیل نبود؛ نتیجه یک موقعیت متناهی نبود.
crs-no-datum-path = هیچ تبدیل منتشرشده‌ای بین قاب‌های مرجع { $from } و { $to } (دیتام‌های EPSG { $source } و { $target }) در دسترس نیست. تبدیل با این حال، به میزانی نامشخص نادرست خواهد بود، بنابراین چیزی تغییر نکرد.
crs-unknown-datum = قاب مرجع { $from } یا { $to } قابل شناسایی نیست، و این دو از مدل‌های زمینی متفاوتی استفاده می‌کنند. تبدیل بین آن‌ها به میزانی نامشخص نادرست خواهد بود.
ws-survey = نقشه‌برداری
survey-count-designs = { $count } { $count ->
    [one] طرح
   *[other] طرح
  }
survey-count-meshes = { $count } { $count ->
    [one] مثلث‌بندی
   *[other] مثلث‌بندی
  }
survey-count-models = { $count } { $count ->
    [one] مدل بلوکی
   *[other] مدل بلوکی
  }
survey-count-clouds = { $count } { $count ->
    [one] ابر نقاط
   *[other] ابر نقاط
  }
survey-count-holes = { $count } { $count ->
    [one] مجموعه‌داده گمانه
   *[other] مجموعه‌داده گمانه
  }
survey-count-rasters = { $count } { $count ->
    [one] رستر
   *[other] رستر
  }
survey-angle = چرخش حول Z (خلاف جهت عقربه‌های ساعت)
survey-scale = ضریب مقیاس یکنواخت XYZ
survey-invalid-transform = مبداها، زاویه و مختصات حاصل باید متناهی باشند.
survey-invalid-scale = مقیاس باید عددی مثبت و متناهی با معکوس متناهی باشد.
survey-empty-selection = حداقل یک مورد پشتیبانی‌شده برای تبدیل انتخاب کنید.
survey-unavailable = یک مورد انتخاب‌شده گم شده یا بارگذاری نشده است. پیش از تبدیل، آن را بارگذاری کنید.
survey-wrong-project = فقط طرح‌ها را از پروژه فعال انتخاب کنید.
survey-name-required = نامی برای سامانه مختصات وارد کنید.
survey-working = در حال تبدیل داده‌های انتخاب‌شده…
survey-completed = { $items } در جای خود تبدیل شد. واگرد آن‌ها را بازمی‌گرداند.
survey-failed = تبدیل ناموفق بود: { $error }
survey-stale = چون پروژه فعال یا داده‌های منبع تغییر کرده‌اند، تبدیل لغو شد. داده‌های منبع را انتخاب کرده و دوباره تلاش کنید.
survey-coordinates-menu = مختصات
survey-definitions-action = تعاریف…
survey-transform-action = تبدیل…
survey-definitions-title = تعاریف مختصات
survey-transform-title = تبدیل مختصات
survey-new-system = سامانه مختصات جدید
survey-new-system-name = سامانه مختصات
survey-set-local = تنظیم به‌عنوان سامانه مختصات معدن
survey-delete-system = حذف سامانه مختصات
survey-systems-empty = هیچ سامانه مختصاتی وجود ندارد
survey-system-name = نام
survey-system-origin = همان نقطه — مختصات سامانه
survey-angle-help = خلاف جهت عقربه‌های ساعت از X مرجع به سمت Y مرجع، از بالا دیده شود.
survey-scale-help = مقیاس یکنواخت XYZ از قاب مرجع به این سامانه. برای حفظ ابعاد، از ۱ استفاده کنید.
survey-close = بستن
survey-from = از
survey-to = به
survey-transform-button = تبدیل
survey-swap = جابه‌جایی
survey-drape-note = تصاویر پوشانده‌شده از سطوح تبدیل‌شده حذف می‌شوند و باید دوباره پوشانده شوند.
survey-needs-grid-block-model = مدل بلوکی یک شبکه منظم از یاخته‌هاست، و تغییر تصویر یا قاب مرجع این نظم را حفظ نمی‌کند. تبدیل آن به معنای نمونه‌برداری مجدد هر یاخته در یک شبکه جدید و از دست دادن مقادیری است که در بر دارد، بنابراین بدون تغییر باقی ماند.
survey-needs-grid-raster = یک رستر با یک نگاشت آفین در جهان قرار می‌گیرد، که تغییر تصویر یا قاب مرجع نمی‌تواند آن را حفظ کند. تبدیل آن به معنای نمونه‌برداری مجدد تصویر است، بنابراین بدون تغییر باقی ماند.
survey-conversion-exact = دقیق: فقط تغییر شبکه، بدون تصویر مجدد.
survey-conversion-accuracy = دقت اعلام‌شده { $accuracy } متر.
survey-kind = نوع
survey-axis-names = نام‌های محور
survey-kind-registry-short = سامانه ثبت‌شده
survey-kind-grid-short = شبکه روی سامانه دیگر
survey-registry-search = جستجو
survey-registry-hint = نام یا کد EPSG، مانند «mga zone 56»
survey-registry-none = هیچ موردی در ثبت با همه کلمات مطابقت ندارد.
survey-parent = تعریف‌شده نسبت به
survey-parent-origin = نقطه شناخته‌شده — مختصات سامانه والد
survey-pick-registry = سامانه را جستجو کرده و از نتایج انتخاب کنید.
survey-pick-parent = سامانه‌ای را که این شبکه نسبت به آن تعریف شده انتخاب کنید.
survey-pick-system = یک سامانه انتخاب کنید
survey-pick-systems = سامانه مبدأ و سامانه مقصد تبدیل را انتخاب کنید.
survey-no-selection = یک سامانه مختصات از سمت چپ انتخاب کنید، یا برای افزودن یکی کلیک راست کنید.
survey-kind-grid = شبکه روی { $parent }
survey-system-in-use = «{ $name }» قابل حذف نیست: { $dependants } { $dependants ->
    [one] سامانه
   *[other] سامانه
  } نسبت به آن تعریف شده‌اند. ابتدا آن‌ها را به جایی دیگر هدایت کنید.
survey-system-cycle = «{ $name }» به‌طور مستقیم یا از طریق سامانه‌های والد خود، نسبت به خودش تعریف شده است.
survey-system-missing = آن سامانه مختصات دیگر وجود ندارد. تعریف دیگری انتخاب کنید.
survey-same-system = سامانه‌های مبدأ و مقصد متفاوتی انتخاب کنید.
survey-name-exists = سامانه مختصاتی با این نام از قبل وجود دارد. برای ویرایش آن را انتخاب کنید، یا نام دیگری برگزینید.

## About strings

about-copyright-c-2026-leo-timmins =
    Copyright (c) 2026 Leo Timmins, Lucas Timmins, and Incline Design contributors. Permission is hereby granted, free of charge, to any person obtaining a copy of this software to deal in it without restriction, subject to the conditions of the MIT License.

    Incline Design is provided "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, including but not limited to the warranties of MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE and NONINFRINGEMENT.
about-free-open-source-mine-design = طراحی معدن آزاد و متن‌باز
about-licensed-under-mit-license = دارای مجوز MIT

## App strings

app-activated-browser-project-name = پروژهٔ مرورگر «{ $name }» فعال شد.
app-browser-project-delete-failed = حذف پروژهٔ مرورگر شکست خورد: { $error }
app-browser-project-no-longer-exists = آن پروژهٔ مرورگر دیگر وجود ندارد
app-browser-save-failed-error = ذخیره در مرورگر شکست خورد: { $error }
app-could-not-activate-browser-project = پروژهٔ مرورگر فعال نشد: { $error }
app-could-not-delete-browser-project = پروژهٔ مرورگر حذف نشد: { $error }
app-could-not-load-browser-project = پروژهٔ مرورگر بارگیری نشد: { $error }
app-could-not-restore-browser-project = پروژهٔ مرورگر بازیابی نشد: { $error }
app-deleted-browser-project = پروژهٔ مرورگر حذف شد
app-failed-create-window-error = پنجره ایجاد نشد: { $error }
app-failed-create-window-icon-error = نماد پنجره ایجاد نشد: { $error }
app-failed-detach-top-down-preview = نمای بالا جدا نشد: { $error }
app-failed-initialize-graphics-error = گرافیک مقداردهی اولیه نشد: { $error }
app-browser-preferences-load-failed = تنظیمات مرورگر بارگیری نشد: { $error }
app-failed-load-config-file-error = فایل پیکربندی بارگیری نشد: { $error }
app-failed-load-session-file-error = فایل نشست بارگیری نشد: { $error }
app-failed-rasterize-window-icon-error = نماد پنجره شطرنجی نشد: { $error }
app-failed-save-browser-session-error = نشست مرورگر ذخیره نشد: { $error }
app-failed-save-session-error = نشست ذخیره نشد: { $error }
app-saved-name-browser-storage = «{ $name }» در حافظهٔ مرورگر ذخیره شد

## Block strings

block-model-between = بین
block-model-block-grid = شبکه بلوک
block-model-block-size = اندازهٔ بلوک
block-model-choose-numeric-variable = یک متغیر عددی انتخاب کنید
block-model-choose-numeric-variables = انتخاب متغیرهای عددی
block-model-count-variables-selected = { $count } متغیر انتخاب شده است
block-model-estimate-variables = متغیرهای برآورد
block-model-full-x-y-z-dimensions = ابعاد کامل X، Y و Z هر بلوک. بلوک‌های کوچک‌تر جزئیات، زمان محاسبه و مصرف حافظه را افزایش می‌دهند.
block-model-grid-bounds-block-sizes-invalid = محدودیت های شبکه یا اندازه بلوک غیرفعال هستند.
block-model-lower-x-y-z-edges = مرزهای پایینی X، Y و Z حجم مدل بلوکی. مراکز بلوک از نیم بلوک داخل این حدود آغاز می‌شوند.
block-model-maximum = بیشینه
block-model-maximum-nearest-samples-used-each = بیشینهٔ نزدیک‌ترین نمونه‌ها برای هر بلوک. مقادیر کمتر سریع‌ترند؛ مقادیر بیشتر ممکن است برآورد را هموار و زمان محاسبه را زیاد کنند.
block-model-maximum-samples = حداکثر نمونه ها
block-model-minimum = کمینه
block-model-min-samples-help = کمینهٔ نمونه‌های نزدیک لازم برای برآورد یک بلوک. بلوک‌های دارای نمونهٔ کمتر در شعاع جستجو خالی می‌مانند.
block-model-minimum-samples = حداقل نمونه ها
block-model-nugget = اثر قطعه‌ای
block-model-numeric-interval-fields-interpolate = فیلدهای بازه عددی برای درون‌یابی. هر فیلد انتخاب‌شده به یک متغیر مدل بلوکی تبدیل می‌شود.
block-model-kriging-help = کریجینگ معمولی، بازه‌های عددی گمانه را در مرکز هر بلوک با استفاده از واریوگرام کروی برآورد می‌کند.
block-model-partial-sill = آستانه جزئی
block-model-range-search-radius = فاصله / شعاع جستجو
block-model-range-help = نمونه های فراتر از این فاصله حذف می شوند؛ در این محدوده کوویاریانس به صفر می رسد.
block-model-select-all = همه را انتخاب کنید
block-model-sill-help = واریانس دارای هم‌بستگی مکانی حاصل از مدل کروی. همراه با اثر ناگت، کوواریانس در فاصلهٔ صفر را تعیین می‌کند.
block-model-spherical-variogram-search = واریوگرام کره ای و جستجو
block-model-threshold-at-most = <= آستانه
block-model-threshold-at-least = >= آستانه
block-model-threshold-min = آستانه / کمینه
block-model-upper-x-y-z-extent = گستره بالایی X، Y و Z برای پوشش. اگر گستره مضرب دقیقی از اندازه بلوک نباشد، آخرین بلوک ممکن است از این محدوده فراتر رود.
block-model-variable = متغیر
block-model-variance-effectively-zero-separation = واریانس در جدایی تقریباً صفر ناشی از خطای اندازه‌گیری یا تغییرات کوچک‌تر از مقیاس نمونه‌برداری. اگر اثر ناگت لازم نیست صفر را وارد کنید.
block-model-volume-feedback-disconnected = بازخوانی بازخورد استفاده از حجم بلوک قطع شد
block-model-volume-feedback-failed = بازخوانی بازخورد استفاده از حجم بلوک شکست خورد: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z

## Canvas strings

canvas-not-selectable-closed-polyline = قابل انتخاب نیست | یک چندخطی بسته انتخاب کنید
canvas-polyline-summary = چندخطی | لایه: { $layer } | { $count } رأس
canvas-surface-name = سطح | { $name }
canvas-trimmed = پیرایش‌شده

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = شیب و برم از شیء { $object_id } ایجاد شد
cmd-bezier-replaced-polyline-span-first-last = بازهٔ چندخطی { $first }→{ $last } با { $count } نقطهٔ میانی نمونه‌برداری‌شده جایگزین شد
cmd-bezier-vertices-first-last = رأس‌های { $first } تا { $last }
cmd-block-model-block-model-loader-disconnected-path = بارگذار مدل بلوکی برای { $path } قطع شد
cmd-block-model-block-model-path-has-count = مدل بلوکی { $path } دارای { $count } متغیر از نوع پشتیبانی‌نشده است که قابل خواندن نیست: { $names }
cmd-block-model-building-ore-mesh = در حال ساخت مش کانسنگ…
cmd-block-model-could-not-create-block-model = ایجاد مدل بلوکی ممکن نبود: { $error }
cmd-block-model-could-not-decode-block-model = رمزگشایی متغیر رنگ مدل بلوکی «{ $variable }» ممکن نبود: { $error }
cmd-block-model-created-block-model-name-ordinary = مدل بلوکی «{ $name }» با کریجینگ معمولی ایجاد شد
cmd-block-model-failed-load-block-model-error = بارگیری مدل بلوکی ناموفق بود: { $error }
cmd-block-model-generated-ore-mesh-from-block = مش کانسنگ از مدل بلوکی «{ $name }» ایجاد شد
cmd-block-model-imported-block-model-source-path = منبع مدل بلوکی { $path } وارد شد
cmd-block-model-loaded-block-model-name-blocks = مدل بلوکی «{ $name }» بارگذاری شد: { $blocks } بلوک ({ $renderable } قابل رندر)، شبکهٔ { $dimx }×{ $dimy }×{ $dimz }، ‏{ $variables } متغیر
cmd-block-model-loading-name = در حال بارگیری { $name }
cmd-block-model-loading-name-ellipsis = در حال بارگیری { $name }…
cmd-chamfer-applied = گوشهٔ { $corner } با شعاع { $radius } و { $segments } پاره پخ زده شد
cmd-chamfer-radius = شعاع { $radius }
cmd-commands-clipped = برش‌خورده
cmd-commands-command-failed-error = فرمان ناموفق بود: { $error }
cmd-commands-select-one-more-objects-before = پیش از تنظیم { $axis } یک یا چند شیء انتخاب کنید
cmd-commands-sliced = لایه‌بندی‌شده
cmd-contours-contour-generation-failed-error = تولید خطوط تراز ناموفق بود: { $error }
cmd-contours-discarded-layer-exists = منحنی‌های میزان «{ $name }» کنار گذاشته شدند: لایهٔ «{ $layer_name }» از قبل وجود دارد
cmd-contours-discarded-project-closed = خطوط تراز «{ $name }» کنار گذاشته شد: پروژه بسته شده بود
cmd-contours-discarded-layer-deleted = خطوط تراز «{ $name }» کنار گذاشته شد: لایه خروجی انتخاب‌شده حذف شده بود
cmd-contours-generated = { $line_count } چندخطی میزان برای مثلث‌بندی «{ $name }» در لایهٔ «{ $layer_name }» ایجاد شد
cmd-creation-assembled-boundary-rings = { $assembled_count } حلقهٔ مرزی بسته از خطوط باز تکه‌تکه ساخته شد
cmd-creation-created-triangulation-from-boundary = مثلث‌بندی از { $boundary_count } حلقهٔ مرزی و { $constraint_count } قید باز با نوع سطح { $surface_type } ایجاد شد
cmd-creation-creating-triangulation = در حال ایجاد مثلث‌بندی…
cmd-creation-generate-upper-surface-ignored-count = ساخت سطح بالا: { $count } قطعهٔ خط شکست پایینِ متعارض نادیده گرفته شد؛ اشیای مبدأ تغییر نکردند
cmd-creation-ignored-objects = هنگام مثلث‌بندی { $rejected } شیء غیرچندخطی یا تباه نادیده گرفته شد
cmd-creation-weld-retry-moved-coarse-welded = جوش و تلاش دوباره: { $coarse_welded } رأس به موقعیت‌های مشترک منتقل شد (تا { $coarse_weld_tol } متر)؛ اشیای مبدأ تغییر نکردند
cmd-creation-welded-breakline-vertices = { $welded } رأس خط شکست که در محدوده رواداری منطبق بودند جوش داده شد
cmd-cuts-clipped-surface-name-polyline-mode = سطح «{ $name }» با چندخطی بریده شد ({ $mode })
cmd-cuts-clipping-surface-polyline = در حال بریدن سطح با چندخطی…
cmd-cuts-cut-topology-name-pit-shell = سطح توپوگرافی «{ $name }» با پوستهٔ پیت بریده شد
cmd-cuts-cut-triangulation-name-z-band = مثلث‌بندی «{ $name }» با بازهٔ Z ‏[{ $min }، { $max }] بریده شد
cmd-cuts-cutting-topology-pit-shell = در حال بریدن سطح توپوگرافی با پوستهٔ پیت…
cmd-cuts-cutting-triangulation-z = در حال بریدن مثلث‌بندی بر اساس Z…
cmd-cuts-ignored-vertical-faces = { $count } وجه عمودی یا تباهِ توپولوژی مرجع بدون مساحت XY نادیده گرفته شد
cmd-cuts-site-skipped-constraint-from-x = { $site }: قید ({ $from_x }، { $from_y }) ← ({ $to_x }، { $to_y }) که مثلث‌ساز نتوانست تقسیم کند، رد شد
cmd-cuts-skipped-degenerate-edges = { $site }: ‏{ $skipped } یال قید تقریباً تباه رد شد؛ مرز برش ممکن است در نزدیکی آن‌ها اندکی اختلاف داشته باشد
cmd-cuts-trimmed-surface = سطح «{ $surface }» با سطح توپوگرافی «{ $topology }» پیرایش شد ({ $mode })
cmd-cuts-trimming-surface-topology = در حال پیرایش سطح با سطح توپوگرافی…
cmd-drape-draped-intersected-vertices-changed = { $intersected } رأس تصویر شد؛ ارتفاع { $changed } مورد تغییر کرد
cmd-drape-no-intersections = هیچ‌یک از رأس‌های طراحی انتخاب‌شده با سطوح توپوگرافی انتخاب‌شده برخورد ندارد
cmd-drape-objects-changed-object-s-changed = { $objects } شیء تغییر کرد · { $changed } از { $intersected } رأس متقاطع جابه‌جا شد
cmd-drape-select-one-more-design-objects = یک یا چند شیء طراحی را برای انطباق روی سطح انتخاب کنید
cmd-drape-select-one-more-topologies-drape = یک یا چند سطح توپوگرافی را برای انطباق روی آن انتخاب کنید
cmd-drape-selected-topologies-no-longer-loaded = سطوح توپوگرافی انتخاب‌شده دیگر بارگذاری نشده‌اند
cmd-drill-hole-drill-pattern-too-large-contains = پترن حفاری خیلی بزرگ است یا مختصات یقهٔ نامعتبر دارد
cmd-drill-hole-enter-name-drill-pattern = نامی برای پترن حفاری وارد کنید
cmd-drill-hole-failed-load-drillholes-error = بارگیری گمانه‌ها ناموفق بود: { $error }
cmd-drill-hole-depth-must-be-positive = عمق چال باید بزرگ‌تر از صفر باشد
cmd-drill-hole-diameter-must-be-positive = قطر چال باید بزرگ‌تر از صفر باشد
cmd-drill-hole-loaded-drillhole-dataset-name-holes = مجموعه‌دادهٔ گمانهٔ «{ $name }» بارگذاری شد: { $holes } گمانه و { $fields } فیلد رنگ
cmd-drill-hole-pattern-contains-no-holes = پترن هیچ چالی ندارد
cmd-explode-count-line-s = { $count } خط
cmd-explode-polyline = تجزیهٔ چندخطی
cmd-explode-exploded-polyline-into-count-line = چندخطی به { $count } پاره‌خط تجزیه شد
cmd-file-block-model-csv-encoding-failed = کدگذاری CSV مدل بلوکی ناموفق بود: { $error }
cmd-file-block-model-csv-export-failed = خروجی CSV مدل بلوکی ناموفق بود: { $error }
cmd-file-browser-recovery-unavailable = فایل‌های بازیابی مرورگر در دسترس نیستند؛ پروژه‌های ذخیره‌شده در IndexedDB باقی می‌مانند
cmd-file-closed-project-runtime-id-runtime = پروژه با شناسه زمان اجرای { $runtime_id } بسته شد
cmd-file-could-not-create-new-project = ایجاد پروژه جدید ممکن نبود: { $error }
cmd-file-could-not-finish-pending-project = تکمیل عملیات در انتظار پروژه ممکن نبود: { $error }
cmd-file-could-not-finish-saving-before = تکمیل ذخیره‌سازی پیش از خروج ممکن نبود: { $error }
cmd-file-could-not-open-browser-project = باز کردن پروژه مرورگر ممکن نبود: { $error }
cmd-file-could-not-open-path-error = باز کردن { $path } ممکن نبود: { $error }
cmd-file-could-not-read-selected-file = خواندن فایل انتخاب‌شده ممکن نبود: { $error }
cmd-file-could-not-reload-layer-from = بارگیری دوباره لایه از دیسک ممکن نبود: { $error }
cmd-file-could-not-reload-project-from = بارگیری دوباره پروژه از دیسک ممکن نبود: { $error }
cmd-file-could-not-remove-browser-project = حذف پروژه مرورگر ممکن نبود: { $error }
cmd-file-could-not-restore-layer-from = بازیابی لایه از پروژه ممکن نبود: { $error }
cmd-file-could-not-snapshot-dirty-project = گرفتن تصویر لحظه‌ای از پروژهٔ تغییریافته برای بازیابی ممکن نشد: { $error }
cmd-file-could-not-start-browser-export = آغاز خروجی در مرورگر ممکن نبود: { $error }
cmd-file-could-not-write-recovery-copies = نوشتن نسخه‌های بازیابی ممکن نشد: { $error }
cmd-file-created-new-browser-project = پروژه مرورگر جدید ایجاد شد
cmd-file-created-new-project = پروژه جدید ایجاد شد
cmd-file-description-download-failed-error = دانلود { $description } ناموفق بود: { $error }
cmd-file-discard-cancelled-project-changed = کنار گذاشتن تغییرات لغو شد، زیرا پروژه هنگام بارگیری دوباره OMF تغییر کرد
cmd-file-discarded-changes-layer-target-name = تغییرات لایه «{ $target_name }» کنار گذاشته شد
cmd-file-discarded-changes-reloaded-path = تغییرات کنار گذاشته شد: { $path } دوباره بارگیری شد
cmd-file-downloaded-description-file-name = { $description } دانلود شد: { $file_name }
cmd-file-dxf-download-encoding-failed-error = کدگذاری دانلود DXF ناموفق بود: { $error }
cmd-file-dxf-import-failed-error = وارد کردن DXF ناموفق بود: { $error }
cmd-file-encoding-block-model-csv-download = در حال کدگذاری بارگیری CSV مدل بلوکی…
cmd-file-encoding-dxf-download = در حال کدگذاری بارگیری DXF…
cmd-file-encoding-triangulation-download = در حال کدگذاری بارگیری مثلث‌بندی…
cmd-file-exit-deferred-exports = خروج تا پایان خروجی‌های پس‌زمینه به تعویق افتاد
cmd-file-exit-requested-no-unsaved-changes = خروج درخواست شد و تغییر ذخیره‌نشده‌ای وجود ندارد
cmd-file-exported-block-model-csv-path = CSV مدل بلوکی به { $path } صادر شد
cmd-file-exported-description-dxf-path = { $description } به DXF صادر شد: { $path }
cmd-file-exported-triangulation-name-path = مثلث‌بندی «{ $name }» به { $path } صادر شد
cmd-file-exporting-name = در حال صادر کردن { $name }…
cmd-file-exporting-triangulation-name-path = در حال صادر کردن مثلث‌بندی «{ $name }» به { $path }
cmd-file-fatal-renderer-failure-reason = خرابی بحرانی رندرکننده: { $reason }
cmd-file-dialog-action-failed = عملیات پنجره فایل ناموفق بود: { $msg }
cmd-file-imported-added-object-s-from = { $added } شیء از { $name } وارد شد
cmd-file-imported-total-dxf-object-s = { $total } شیء DXF وارد شد
cmd-file-layer-discard-was-cancelled-because = کنار گذاشتن تغییرات لایه لغو شد، زیرا پروژه هنگام بارگیری دوباره تغییر کرد
cmd-file-no-recovery-directory = هیچ پوشهٔ بازیابی در دسترس نیست: { $error }
cmd-file-no-unsaved-project-content-nothing = محتوای ذخیره‌نشده‌ای در پروژه نیست؛ چیزی برای بازیابی وجود ندارد
cmd-file-parsing-browser-dxf-import = در حال تجزیهٔ واردات DXF مرورگر…
cmd-file-parsing-dxf-import = در حال تجزیهٔ واردات DXF…
cmd-file-project-closes-after-save = پروژه پس از پایان ذخیره‌سازی فعلی بسته خواهد شد
cmd-file-the-project-closes-after-save = پروژه پس از پایان ذخیره‌سازی فعلی بسته خواهد شد
cmd-file-queued-count-triangulation-file-s = { $count } فایل مثلث‌بندی در صف ورود قرار گرفت
cmd-file-recovery-copies-path-reopen-them = نسخه‌های بازیابی در { $path } هستند؛ پس از راه‌اندازی دوباره آن‌ها را باز کنید
cmd-file-recovery-copy-failed-error = نسخهٔ بازیابی شکست خورد: { $error }
cmd-file-recovery-copy-failed-failure = ایجاد نسخه بازیابی ناموفق بود: { $failure }
cmd-file-recovery-copy-written-path = نسخه بازیابی نوشته شد: { $path }
cmd-file-reverting-layer = در حال بازگردانی لایه…
cmd-file-reverting-project = در حال بازگردانی پروژه…
cmd-file-save-failed-message = ذخیره‌سازی ناموفق بود: { $message }
cmd-file-save-worker-ended-without-result = فرایند ذخیره‌سازی بدون نتیجه پایان یافت
cmd-file-saved-project-as = پروژه با نام زیر ذخیره شد: { $path }
cmd-file-saved-project = پروژه ذخیره شد: { $path }
cmd-file-selected-block-model-no-longer = مدل بلوکی انتخاب‌شده دیگر بارگذاری نشده است
cmd-file-switching-project = در حال تعویض پروژه…
cmd-file-triangulation-download-encoding-failed = کدگذاری دانلود مثلث‌بندی ناموفق بود: { $error }
cmd-file-user-chose-exit-without-saving = کاربر خروج بدون ذخیره را انتخاب کرد
cmd-file-user-requested-exit-project-export = کاربر درخواست خروج داد (تأیید خروجی پروژه یا کار ذخیره‌نشده لازم است)
cmd-file-viewport = نمای دید
cmd-file-wait-current-project-save-finish = منتظر پایان ذخیره‌سازی پروژه فعلی بمانید
cmd-file-wait-current-project-switch-finish = منتظر پایان تعویض پروژه فعلی بمانید
cmd-file-wait-project-operation-finish-before = پیش از کنار گذاشتن تغییرات، منتظر پایان عملیات پروژه بمانید
cmd-file-wait-project-revert-finish-before = پیش از ذخیره، منتظر پایان بازگردانی پروژه بمانید
cmd-fuse-closed-polyline = چندخطی بسته
cmd-fuse-count-source-line-s = { $count } خط مبدأ
cmd-fuse-created-shape-object-id-vertices = { $shape } { $object_id } با { $vertices } رأس از { $sources } خط مبدأ ایجاد شد
cmd-fuse-click-missed = اتصال: کلیک به هیچ شیئی برخورد نکرد (چیزی زیر نشانگر نیست)
cmd-fuse-click-not-near-endpoint = اتصال: کلیک به هیچ‌یک از نقاط انتهایی خط انتخاب‌شده به‌اندازه کافی نزدیک نبود
cmd-fuse-clicked-closed-polyline = اتصال: شیء { $object_id } چندخطی بسته است؛ اتصال فقط برای چندخطی‌های باز کار می‌کند
cmd-fuse-clicked-not-open-polyline = اتصال: شیء { $object_id } چندخطی باز نیست (نوع آن { $kind } است)
cmd-fuse-clicked-object-missing = اتصال: شیء کلیک‌شده { $object_id } دیگر وجود ندارد
cmd-fuse-clicked-too-few-vertices = اتصال: چندخطی { $object_id } فقط { $count } رأس دارد؛ حداقل ۲ رأس لازم است
cmd-fuse-endpoint-marker-missing = اتصال: نشانگر نقطه انتهایی { $marker_index } دیگر وجود ندارد
cmd-fuse-close-needs-three-vertices = اتصال: خط برای بسته‌شدن به‌صورت چندخطی حداقل به ۳ رأس متمایز نیاز دارد (اکنون { $count })
cmd-fuse-lines = ادغام خطوط
cmd-fuse-needs-two-segments = اتصال: برای اعمال دست‌کم ۲ قطعه لازم است ({ $count } قطعه موجود است)
cmd-fuse-no-active-layer = اتصال: لایه فعالی برای قرار دادن خط متصل وجود ندارد
cmd-fuse-no-active-project = اتصال: پروژه فعالی وجود ندارد؛ اعمال ممکن نیست
cmd-fuse-no-source-line = اتصال: خط مبدأیی برای بستن و تبدیل به چندخطی وجود ندارد
cmd-fuse-awaiting-object-invalid = اتصال: شیء { $awaiting_id } دیگر چندخطی معتبر نیست
cmd-fuse-object-already-in-chain = اتصال: شیء { $object_id } از قبل عضو زنجیره است؛ خط دیگری را انتخاب کنید
cmd-fuse-result-too-few-vertices = اتصال: نتیجه رأس‌های بسیار کمی دارد ({ $count })؛ عملیات لغو می‌شود
cmd-fuse-segment-object-invalid = اتصال: شیء قطعهٔ { $object_id } دیگر چندخطی معتبر نیست؛ عملیات لغو شد
cmd-fuse-source-object-invalid = اتصال: شیء مبدأ { $object_id } دیگر چندخطی باز معتبری نیست
cmd-fuse-source-object-missing = اتصال: شیء مبدأ { $object_id } دیگر وجود ندارد
cmd-fuse-open-polyline = چندخطی باز
cmd-include-failed = دربرگیری ناموفق بود: { $message }
cmd-include-included-solid-shape-name-topology = جسم «{ $shape_name }» در توپولوژی «{ $topology_name }» گنجانده شد ({ $retained } وجه حفظ و { $skipped } وجه پوششی رد شد)
cmd-include-including-pit-stockpile-solid = در حال افزودن جسم پیت/دپو…
cmd-insert-point-count-operation-point-s = { $count } نقطهٔ { $operation }
cmd-insert-point-elevation-must-be-finite = درج نقطه در تراز به یک مقدار تراز محدود نیاز دارد
cmd-insert-point-insert-points = درج نقاط
cmd-insert-point-inserted-count-operation-point-s = { $count } نقطهٔ { $operation } درج شد
cmd-insert-point-intersection = تقاطع
cmd-insert-point-no-new-operation-points-were = نقطه جدیدی برای { $operation } یافت نشد
cmd-insert-point-select-least-two-polylines-before = پیش از درج نقاط تقاطع، دست‌کم دو چندخطی انتخاب کنید
cmd-insert-point-select-one-more-polylines-before = پیش از درج نقطه در تراز، یک یا چند چندخطی انتخاب کنید
cmd-layer-created-layer-name = لایه «{ $name }» ایجاد شد
cmd-layer-deleted-with-objects = لایه { $layer_id } و همه اشیای آن حذف شد
cmd-layer-duplicated-layer-duplicate-name = لایه «{ $duplicate_name }» تکثیر شد
cmd-layer-locked = قفل‌شده
cmd-layer-name-copy = کپی { $name }
cmd-layer-selected-count-object-s-layer = { $count } شیء در لایهٔ { $layer_id } انتخاب شد
cmd-layer-state-layer-name = لایه «{ $name }» { $state }
cmd-layer-unlocked = باز
cmd-move-tool-moved-collars = جابجایی ({ $delta }) برای { $count } یقهٔ چال اعمال شد
cmd-move-tool-moved-objects = جابه‌جایی ({ $delta }) روی { $count } شیء اعمال شد
cmd-move-tool-count-hole-s = { $count } چال
cmd-object-edit-edited-kind = { $kind } ویرایش شد
cmd-object-edit-edited-kind-count-vertices = { $kind } ویرایش شد ({ $count } رأس)
cmd-object-edit-no-changes-apply = تغییری برای اعمال وجود ندارد
cmd-object-edit-object-changed-since-editor-opened = این شیء از زمان باز شدن ویرایشگر تغییر کرده است؛ برای ویرایش نسخه فعلی دوباره آن را باز کنید
cmd-object-edit-target-changed = شیء در حال ویرایش تغییر کرد؛ ویرایش لغو شد
cmd-object-edit-object-no-longer-exists-document = این شیء دیگر در سند وجود ندارد
cmd-object-edit-select-single-design-object-edit = یک شیء طراحی برای ویرایش انتخاب کنید
cmd-object-edit-unassigned = تخصیص‌نیافته
cmd-offset-create-offset = ایجاد آفست
cmd-offset-created-offset-count-object-s = آفست { $count } شیء ایجاد شد
cmd-offset-distance-must-be-positive = فاصله آفست باید بزرگ‌تر از صفر باشد
cmd-omf-could-not-open-project-source = پروژهٔ { $source_name } باز نشد: { $error }
cmd-omf-create-open-project-before-merging = پیش از ادغام داده‌ها یک پروژه بسازید یا باز کنید
cmd-omf-encoding-project = در حال کدگذاری پروژه…
cmd-omf-exported-project-path = پروژه به { $path } صادر شد
cmd-omf-imported-project = پروژهٔ «{ $project_name }» از { $source_name } وارد شد: { $count } مجموعه‌دادهٔ سطح بالا
cmd-omf-importing-project = در حال وارد کردن پروژه…
cmd-omf-export-failed = خروجی OMF ناموفق بود: { $error }
cmd-omf-import-failed = ورود OMF ناموفق بود: { $error }
cmd-omf-opened-project = پروژهٔ «{ $project_name }» از { $source_name } باز شد
cmd-omf-project-source-name-contains-no = پروژه «{ $source_name }» هیچ عنصر داده پشتیبانی‌شده‌ای ندارد
cmd-omf-source-name-applied-project-origin = { $source_name }: مبدأ پروژهٔ { $origin } پیش از ادغام اعمال شد
cmd-omf-crs-differs = { $source_name }: دستگاه مختصات «{ $source_crs }» با CRS پروژه «{ $target_crs }» تفاوت دارد؛ مختصات بدون بازفرافکنی ادغام شدند
cmd-omf-source-name-units-source-units = { $source_name }: واحدهای «{ $source_units }» با واحدهای پروژه «{ $target_units }» تفاوت دارند؛ مختصات بدون تبدیل ادغام شدند
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = داده‌ای باز از Incline Design برای صادر کردن وجود ندارد
cmd-placement-2-vertices = ۲ رأس
cmd-placement-count-vertices = { $count } رأس
cmd-placement-created-circle = دایره‌ای با شعاع { $radius } متر ایجاد شد
cmd-placement-created-closed-polyline = چندخطی بسته با { $count } رأس ایجاد شد
cmd-placement-created-line-segment-2-vertices = پاره‌خطی با ۲ رأس ایجاد شد
cmd-placement-created-open-polyline-count-vertices = چندخطی باز با { $count } رأس ایجاد شد
cmd-placement-placed-point-x-y-z = نقطه در { $x }، { $y }، { $z } قرار گرفت
cmd-placement-radius = شعاع { $radius } متر
cmd-plot-composing-engineering-drawing = در حال ترکیب نقشهٔ مهندسی…
cmd-plot-could-not-write-engineering-drawing = نوشتن نقشهٔ مهندسی ممکن نشد: { $error }
cmd-plot-drawing-scale-fitted-visible-data = مقیاس نقشه با داده‌های نمایان جور شد: 1:{ $scale }
cmd-plot = نقشه
cmd-plot-saved-drawing = نقشهٔ مهندسی ذخیره شد: { $description } ‏({ $width } × { $height } پیکسل در { $dpi } نقطه‌براینچ)
cmd-point-cloud-failed-load-point-cloud-error = بارگیری ابر نقاط ناموفق بود: { $error }
cmd-point-cloud-loaded-point-cloud-name-count = ابر نقاط { $name } بارگذاری شد ({ $count } نقطه)
cmd-point-cloud-point-cloud-loader-disconnected-path = بارگذار ابر نقاط برای { $path } قطع شد
cmd-point-cloud-tin-max-edge-disabled = (بیشینه یال غیرفعال است)
cmd-point-cloud-tin-max-edge-max-edge = (بیشینه یال { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = TIN ابر نقاط ناموفق بود: { $error }
cmd-point-cloud-tin-subsampled = TIN زمین: از { $total } نقطه، { $sampled } نقطه به‌صورت مکانی زیرنمونه‌برداری شد
cmd-point-cloud-tin-triangulated = TIN زمین: { $vertex_count } نقطهٔ یکتای XY به { $face_count } وجه مثلث‌بندی شد{ $suffix }
cmd-products-added-product-delay-ms-ms = محصول { $delay_ms } میلی‌ثانیه { $name } افزوده شد
cmd-products-deleted-product-delay-ms-ms = محصول { $delay_ms } میلی‌ثانیه { $name } حذف شد
cmd-products-failed-save-products-error = ذخیره محصولات ناموفق بود: { $error }
cmd-products-product-no-longer-palette = این محصول دیگر در پالت نیست
cmd-property-action-count-object-s-layer = { $action } { $count } شیء به لایه { $layer }
cmd-property-batch-set-axis-value-count = تنظیم گروهی مقدار { $axis } روی { $count } شیء
cmd-property-batch-set-closed-count-polyline = تنظیم گروهی بسته‌بودن روی { $count } چندخطی
cmd-property-batch-set-color-count-object = تنظیم گروهی رنگ روی { $count } شیء
cmd-property-batch-set-fill-style-count = تنظیم گروهی سبک پرشدن روی { $count } شیء
cmd-property-batch-set-line-weight-count = تنظیم گروهی ضخامت خط روی { $count } چندخطی
cmd-property-copied = کپی شد
cmd-property-moved = جابه‌جا شد
cmd-raster-draped = رستر { $raster } روی مثلث‌بندی { $triangulation } پهن شد (گستره‌های هم‌پوشان)
cmd-raster-failed-load-raster-name-error = بارگیری رستر { $name } ناموفق بود: { $error }
cmd-raster-failed-load-raster-path-error = بارگیری رستر { $path } ناموفق بود: { $error }
cmd-raster-loaded-raster-name-via-driver = رستر { $name } با { $driver } بارگذاری شد ({ $srcx }×{ $srcy }، پیش‌نمایش { $prevx }×{ $prevy })
cmd-raster-no-overlapping-triangulation = هیچ مثلث‌بندی بارگذاری‌شده‌ای با گسترهٔ { $name } هم‌پوشانی ندارد
cmd-raster-loader-disconnected = بارگذار رستر برای { $path } قطع شد
cmd-raster-undraped = انطباق رسترها از { $count } مثلث‌بندی برداشته شد
cmd-relimit-click-missed = تعیین مجدد حد: کلیک به هیچ شیئی برخورد نکرد (چیزی زیر نشانگر نیست)
cmd-relimit-click-ignored = تعیین مجدد حد: کلیک نادیده گرفته شد؛ ابزار اکنون منتظر انتخاب هدف نیست
cmd-relimit-clicked-source-line = تعیین مجدد حد: خود خط مبدأ کلیک شد؛ خط دیگری انتخاب کنید
cmd-relimit-no-source-line = تعیین مجدد حد: خط مبدأ تنظیم نشده است؛ انتخاب لغو می‌شود
cmd-relimit-relimited-line-source-id-selected = خط { $source_id } با هدف انتخاب‌شده دوباره محدود شد
cmd-relimit-resized-line-source-id-using = اندازهٔ خط { $source_id } با حالت { $mode } و مقدار { $value } تغییر کرد
cmd-rename-item-no-longer-belongs-active = این مورد دیگر متعلق به پروژه فعال نیست
cmd-rename-renamed-before-name = نام «{ $before }» به «{ $name }» تغییر کرد
cmd-rename-renamed-name-taken = نام «{ $before }» به «{ $name }» تغییر کرد («{ $requested }» از قبل گرفته شده است)
cmd-rotate-collar-turned-count-drillhole-collar-s = { $count } یقهٔ چال { $rotation } چرخانده شد
cmd-section-verb-count-item-s-section = { $verb } { $count } مورد در { $section }
cmd-selection-delete-vertex = حذف رأس
cmd-selection-deleted-count-selected-object-s = { $count } شیء انتخاب‌شده حذف شد
cmd-selection-deleted-vertex = رأس { $vertex } از چندخطی { $object_id } حذف شد
cmd-selection-duplicate-selection = تکثیر انتخاب
cmd-selection-duplicated-count-object-s = { $count } شیء تکثیر شد
cmd-session-created-triangulation = مثلث‌بندی «{ $name }» ({ $vertex_count } رأس، ‏{ $face_count } وجه) از سطح نوع { $surface_type } ساخته شد
cmd-session-deleted-triangulation = مثلث‌بندی «{ $name }» از پروژه حذف شد
cmd-session-failed-load-triangulation-error = بارگیری مثلث‌بندی ناموفق بود: { $error }
cmd-session-failed-load-triangulation-message = بارگیری مثلث‌بندی ناموفق بود: { $message }
cmd-session-loaded-triangulation = مثلث‌بندی «{ $name }» بارگذاری شد ({ $path }، ‏{ $vertex_count } رأس، ‏{ $face_count } وجه)
cmd-session-set-triangulation-tri-id-color = رنگ مثلث‌بندی { $tri_id } روی { $color } تنظیم شد
cmd-session-triangulation-load-no-result = بارگیری مثلث‌بندی برای { $path } بدون نتیجه پایان یافت
cmd-session-triangulation-failed = عملیات مثلث‌بندی ناموفق بود: { $message }
cmd-session-unloaded-triangulation-name = مثلث‌بندی «{ $name }» از بار خارج شد
cmd-slice-entered-slice-view-cx-cy = نمای برش در { $cx }، { $cy }، { $cz } در امتداد { $dx }، { $dy } فعال شد (خط { $length } متر)
cmd-slice-exited-slice-view = از نمای برش خارج شد
cmd-slice-reset-section-view-fit-extents = بازنشانی نمای مقطع (تطبیق با محدوده)
cmd-slice-set-section-grid-enabled = شبکه مقطع فعال = { $enabled }
cmd-split-created-2-open-polylines = ۲ چندخطی باز ایجاد شد
cmd-split-line = تقسیم خط
cmd-split-points-needs-interior-vertex = تقسیم در نقاط: یک رأس داخلی از خط باز انتخاب کنید
cmd-split-polyline-into-two = چندخطی مبدأ به دو چندخطی باز تقسیم شد
cmd-text-edit-finished = ویرایش متن شیء { $object_id } پایان یافت
cmd-text-updated = متن روی شیء { $object_id } به‌روزرسانی شد
cmd-view-centre-rotation-not-available-flying = مرکز چرخش در حالت پرواز در دسترس نیست
cmd-view-fixed-centre-rotation-x-y = مرکز چرخش در { $x }، { $y }، { $z } ثابت شد
cmd-view-no-point-under-cursor-fix = هیچ نقطه‌ای زیر نشانگر برای ثابت کردن مرکز چرخش وجود ندارد
cmd-view-released-centre-rotation = مرکز چرخش آزاد شد
cmd-view-reset-view-fit-extents = بازنشانی نما (جا دادن در محدوده)
cmd-view-set-topology-wireframes-enabled = تنظیم قاب سیمی توپوگرافی = { $enabled }
cmd-view-set-view-points-enabled = تنظیم نقاط نما = { $enabled }
cmd-view-set-xy-grid-enabled = شبکه XY فعال = { $enabled }
cmd-view-zoom-extents-preserving-angle = بزرگ‌نمایی تا محدوده (با حفظ زاویه)

## Common strings

common-add-product = اضافه کردن محصول
common-background = پس زمینه
common-block-model = مدل بلوکی
common-block-models = مدل‌های بلوکی
common-cancelled = لغو شد
common-chamfer = پخ
common-choose = انتخاب کن...
common-circle = دایره
common-click-point-fix-centre-rotation = برای ثابت کردن مرکز چرخش، روی یک نقطه کلیک کنید
common-clip-surface-polyline = کليپ سطح توسط چندخطی...
common-closed = بسته شده
common-colour = رنگ
common-confirm-omf-rewrite = تصدیق مجدد OMF
common-could-not-replace-current-project = جایگزینی پروژه فعلی ممکن نبود: { $error }
common-count-object-s = { $count } شیء
common-create = ایجاد
common-create-batter-berm = ایجاد پله و برم
common-create-bezier-curve = منحنی بیزیر ایجاد کنید
common-create-block-model = ایجاد مدل بلوکی
common-create-block-model-ellipsis = ایجاد مدل بلوکی...
common-create-circle = دایره ای ایجاد کنید
common-create-drill-pattern = ایجاد پترن حفاری
common-create-layer = لایه ای ایجاد کنید
common-create-line = خط ایجاد کنید
common-create-ore-triangulation = ایجاد مثلث‌بندی کانسنگ
common-create-ore-triangulation-ellipsis = ایجاد مثلث‌بندی کانسنگ...
common-create-point = ایجاد نقطه
common-create-polyline = ایجاد چندخطی
common-create-triangulation = ایجاد مثلث‌بندی...
common-crosses = ضربدرها
common-cut = قطع
common-cut-topology-pit-shell = برش سطح توپوگرافی با پوسته پیت معدن...
common-delete-layer = لایه را حذف کنید
common-delete-product = حذف محصول
common-delete-selection = حذف انتخاب
common-designs = طرح ها
common-discard-layer-changes = رد کردن تغییرات لایه
common-down = پایین
common-drape-topology = انطباق تا سطح توپوگرافی
common-easting = شرق
common-edit-object = ویرایش شیء
common-edit-text = متن را ویرایش کنید
common-elevation = ارتفاع
common-exit-without-saving = خروج بدون ذخیره
common-export-engineering-drawing = صادرات نقشه مهندسی
common-filter = فیلتر
common-fly-mode = حالت پرواز
common-generate-contour-lines = تولید خطوط منحنی میزان...
common-hide-all = پنهان کردن همه
common-hide-selection = پنهان کردن انتخاب
common-ignore = نادیده گرفتن
common-import-csv-block-model = واردات CSV مدل بلوکی
common-import-dxf = واردات DXF
common-incline-design-project = پروژه Incline Design
common-layer = لایه
common-legend = راهنما
common-line = خط
common-line-weight = ضخامت خط
common-lock-all = قفل کردن همه
common-lock-selection = قفل کردن انتخاب
common-m = m
common-max = بیشینه
common-merge-shell-into-topology = ادغام پوسته با سطح توپوگرافی
common-merge-shell-into-topology-ellipsis = ادغام پوسته با سطح توپوگرافی...
common-move-collar = جابه‌جایی یقه
common-move-design = جابه‌جایی طرح
common-move-selection = جابه‌جا کردن انتخاب
common-new-product = محصول جدید
common-no-block-models = مدل بلوکی وجود ندارد
common-no-design-layers = لایه طراحی وجود ندارد
common-no-drill-holes = گمانه‌ای وجود ندارد
common-no-file-chosen = فایلی انتخاب نشده است
common-no-open-project = پروژه‌ای باز نیست
common-no-point-clouds = ابر نقاطی وجود ندارد
common-no-triangulations = مثلث‌بندی وجود ندارد
common-none = هیچ‌کدام
common-northing = شمال
common-offset = آفست
common-open = باز
common-orientation = جهت
common-point = نقطه
common-point-cloud = ابر نقاط
common-point-clouds = ابرهای نقاط
common-polyline = چندخطی
common-polyline-layer = چندخطی روی «{ $layer }»
common-project = پروژه
common-rasters = رسترها
common-redo = ازنو
common-relimit-line = خط بازحدگذاری
common-remove-project = حذف پروژه
common-reset-view = بازنشانی نما
common-reveal-all = همه چیز را آشکار کن
common-reveal-finder = نمایش در Finder
common-rotate-collar = چرخاندن یقه
common-save-exit = ذخیره و خروج
common-scale-bar = نوار مقیاس
common-set-initiation-point = تنظیم نقطهٔ آغازش
common-shape = شکل
common-shell = همراه پوسته
common-slashes = خط‌های مورب
common-slice = برش
common-slice-triangulation-z-range = برش مثلث‌بندی بر اساس بازه Z...
common-surface-contours = خطوط تراز سطح
common-text = متن
common-degree-suffix = °
common-tie-holes = اتصال گمانه‌ها
common-triangulations = مثلث‌بندی‌ها
common-trim-topology = برش تا سطح توپوگرافی...
common-undo = واگرد
common-undrape-all = لغو انطباق همه
common-uniform-white = سفید یکنواخت
common-unlock-all = باز کردن قفل همه
common-untitled = بدون عنوان
common-up = بالا
common-vertical-exaggeration = اغراق عمودی
common-x = x
common-zoom-extents = بزرگ‌نمایی به کل محدوده

## Confirmations strings

confirmations-close-project-unsaved-changes = بستن پروژه: تغییرات ذخیره‌نشده
confirmations-close-without-saving = بستن بدون ذخیره
confirmations-delete = حذف
confirmations-delete-objects = حذف اشیاء
confirmations-discard = کنار گذاشتن
confirmations-discard-all-unsaved-changes-layer =
    همهٔ تغییرات ذخیره‌نشدهٔ لایهٔ «{ $name }» کنار گذاشته شوند؟
    لایهٔ ذخیره‌شده دوباره بارگذاری و تغییرات لایه‌های دیگر حفظ می‌شوند. این کار بازگشت‌پذیر نیست.
confirmations-discard-all-unsaved-changes-name =
    همهٔ تغییرات ذخیره‌نشدهٔ «{ $name }» کنار گذاشته شوند؟
    آخرین نسخهٔ ذخیره‌شده دوباره از دیسک بارگذاری می‌شود. این کار بازگشت‌پذیر نیست.
confirmations-discard-changes = رد کردن تغییرات
confirmations-exit-unsaved-changes = خروج: تغییرات ذخیره‌نشده
confirmations-incline-design-cannot-reproduce-all = Incline Design نمی‌تواند همهٔ محتوای OMF اصلی را بازتولید کند. هنگام ذخیره موارد زیر حذف می‌شوند:
confirmations-product = محصول
confirmations-project = این پروژه
confirmations-remove-name-delete-its-browser = «{ $name }» و نسخه ذخیره‌شده آن در مرورگر حذف شود؟ تغییرات ذخیره‌نشده از بین می‌رود.
confirmations-remove-project-unsaved-changes = حذف پروژه: تغییرات ذخیره‌نشده
confirmations-remove-without-saving = حذف بدون ذخیره
confirmations-replace-project-unsaved-changes = پروژه جایگزین: تغییرات ذخیره نشده
confirmations-save = ذخیره
confirmations-save-anyway = در هر صورت ذخیره شود
confirmations-save-changes-current-project-before = قبل از جایگزینی، تغییرات پروژه فعلی ذخیره شود؟
confirmations-save-changes-name-before-closing = تغییرات «{ $name }» پیش از بستن آن ذخیره شود؟
confirmations-save-changes-name-before-removing = تغییرات «{ $name }» پیش از حذف آن از Incline Design ذخیره شود؟
confirmations-save-close = ذخیره و بستن
confirmations-save-modified-project-before-exiting = آیا پیش از خروج، پروژه ویرایش‌شده ذخیره شود؟
confirmations-save-to-browser-before-exit = قبل از خروج، پروژه اصلاح شده را به حافظه مرورگر ذخیره کنید؟
confirmations-save-remove = ذخیره و حذف

## Console strings

console-copy-all = کپی همه
console-copy-message = پیام کپی
console-error = خطا
console-info = اطلاعات
console-no-console-activity-yet = هنوز فعالیتی در کنسول نیست
console-pending = در انتظار
console-progress-summary = در حال انجام · { $summary }
console-success = موفق
console-warn = هشدار

## Csv strings

csv-block-model-category = دسته
csv-block-model-value = مقدار

## Drill strings

drill-hole-add-stop = اضافه کردن توقف
drill-hole-all-rendered-intervals-opaque-white = تمام فواصل نمایش داده شده سفید غیر شفاف هستند.
drill-hole-burden-spacing-must-greater-than = فاصلهٔ بارسنگ و فاصله‌گذاری باید بزرگ‌تر از صفر باشند
drill-hole-choose-valid-closed-polyline = یک پلی‌لاین بستهٔ معتبر انتخاب کنید
drill-hole-colour-scale = مقیاس رنگ
drill-hole-field = فیلد
drill-hole-grayscale = مقیاس خاکستری
drill-hole-green-yellow-red = سبز–زرد–قرمز
drill-hole-heat = حرارتی
drill-hole-no-holes-fit-inside-boundary = با فاصلهٔ بارسنگ و فاصله‌گذاری فعلی هیچ چالی در این مرز جا نمی‌گیرد
drill-hole-pattern-too-many-holes = پترن از حداکثر { $maximum } چال فراتر می‌رود؛ فاصلهٔ بارسنگ یا فاصله‌گذاری را افزایش دهید
drill-hole-preset = پیش‌تنظیم
drill-hole-px = پیکسل
drill-hole-rainbow = رنگین‌کمان
drill-hole-reset-preset = تنظیم مجدد
drill-hole-rotation-offsets-must-contain-valid = چرخش و جابه‌جایی‌ها باید شامل اعداد معتبر باشند
drill-hole-selected-polyline-has-no-usable = پلی‌لاین انتخاب‌شده مساحت XY قابل‌استفاده ندارد
drill-hole-smooth-interpolation = درون‌یابی صاف
drill-hole-spacing-would-scan-too-many = این فاصله‌گذاری خانه‌های شبکهٔ بسیار زیادی را بررسی می‌کند؛ فاصلهٔ بارسنگ یا فاصله‌گذاری را افزایش دهید (حداکثر { $maximum } چال)
drill-hole-square = مربعی
drill-hole-staggered = زیگزاگی
drill-hole-stepped-bands = دسته های مرحله ای
common-times-sign = ×
common-minus-sign = −
drill-hole-unsupported-drillhole-source = منبع گمانه پشتیبانی‌نشده
drill-hole-width = عرض
drill-pattern-arrangement = آرایش
drill-pattern-axis-offset = افست محور { $axis }
drill-pattern-blast-shape = شکل انفجار
drill-pattern-burden = فاصلهٔ بارسنگ
drill-pattern-choose-closed-blast-boundary-then = یک مرز بستهٔ انفجار انتخاب کنید، سپس شبکه را تنظیم کنید. چال‌ها در نما زنده به‌روزرسانی می‌شوند.
drill-pattern-closed-design-polyline-whose-xy = پلی‌لاین طراحی بسته‌ای که ردپای XY آن با چال‌ها پر می‌شود.
drill-pattern-rotation-help = چرخش خلاف جهت عقربه‌های ساعت الگو از محور سراسری { $axis }.
drill-pattern-distance-between-holes-along-each = فاصلهٔ بین چال‌ها در امتداد هر ردیف پترن.
drill-pattern-name-hint = مثلاً برش غربی 03
drill-pattern-diameter-help = قطر نهایی چال؛ بر حسب میلی‌متر وارد و با هر چال ایجادشده ذخیره می‌شود.
drill-pattern-hole-depth = عمق چال
drill-pattern-hole-diameter = قطر چال
drill-pattern-move-over-closed-polyline-then = روی پلی‌لاین بسته بروید و در نما روی آن کلیک کنید. Esc انتخاب را لغو می‌کند.
drill-pattern-name-help = نام مجموعه‌دادهٔ چالی که در پروژه ایجاد می‌شود.
drill-pattern-none-picked = چیزی انتخاب نشده است
drill-pattern-pattern-name = نام پترن
drill-pattern-spacing-help = فاصلهٔ عمودی میان ردیف‌های پترن.
drill-pattern-pick = انتخاب
drill-pattern-preview-count-hole-s-diameter = پیش‌نمایش: { $count } چال · قطر { $diameter } میلی‌متر · عمق { $depth } متر
drill-pattern-rotation = چرخش
drill-pattern-shift-pattern-grid-along-global = شبکه الگو را در امتداد محور سراسری { $axis } جابه‌جا می‌کند، در حالی که همچنان به شکل انفجار محدود می‌ماند.
drill-pattern-spacing = فاصله‌گذاری
drill-pattern-staggered-offsets-every-second-row = آرایش زیگزاگی هر ردیف دوم را به اندازهٔ نیم فاصله‌گذاری جابه‌جا می‌کند.
drill-pattern-vertical-depth-below-each-collar = عمق عمودی زیر هر یقه.

## Dxf strings

dxf-block-nesting-too-deep = تودرتویی بلوک DXF از بیشینهٔ عمق ({ $depth }) بیشتر است؛ «{ $name }» نادیده گرفته می‌شود
dxf-circular-block-reference = ارجاع چرخه‌ای بلوک DXF شناسایی شد: «{ $name }»
dxf-undefined-layer = موجودیت DXF به لایهٔ تعریف‌نشدهٔ «{ $name }» ارجاع داد؛ با نام «{ $fallback }» وارد شد
dxf-import-budget-exceeded = وارد کردن DXF از بودجهٔ { $what } ‏({ $limit }) بیشتر است؛ هندسهٔ باقی‌مانده نادیده گرفته می‌شود
dxf-insert-unknown-block = DXF INSERT به بلوک ناشناختهٔ «{ $name }» ارجاع می‌دهد

## Edit strings

edit-absolute-length = طول مطلق
edit-absolute-rl = RL مطلق
edit-action = اقدام
edit-angle = زاویه
edit-dip-help = زاویه از افق، رو به پایین منفی است: ‎-90 یک چال عمودی است.
edit-app-web-not-recommended-production = استفادهٔ تولیدی از { $app } Web توصیه نمی‌شود. فقط برای نمایش آزمایشی استفاده کنید.
edit-application = برنامه
edit-apply = اعمال
edit-apply-pick-target = اعمال و انتخاب هدف
edit-axis-value = مقدار { $axis }
edit-azimuth = آزیموت
edit-batter-angle = زاویه شیب پله (°)
edit-azimuth-help = جهت حفاری چال‌ها بر حسب درجه در جهت ساعت‌گرد از شمال شبکه.
edit-bench-height = ارتفاع پله
edit-benches = پله‌ها
edit-berm-width = عرض برم
edit-bezier-curve = منحنی بیزیر
edit-choose-layer = یک لایه انتخاب کنید
edit-measure-help = مشخص کنید مقدار واردشده فاصله روی شیب، عرض افقی یا ارتفاع عمودی است.
edit-choose-which-two-polyline-paths = یکی از دو مسیر چندخطی میان رأس‌های انتخاب‌شده را برای جایگزینی برگزینید. طول شامل ارتفاع و یال‌های منحنی است.
edit-click-corner-closed-polyline = روی گوشه ای روی چندخطی بسته کلیک کنید.
edit-click-open-closed-polyline-begin = برای شروع روی چندخطی باز یا بسته کلیک کنید.
edit-click-second-vertex-replacement-span = روی ورق دوم فاصله جایگزینی کلیک کنید.
edit-click-vertex-start-replacement-span = برای شروع محدوده جایگزینی روی یک رأس کلیک کنید.
edit-collide-triangulation = برخورد با مثلث‌بندی
edit-confirm-selection = انتخاب را تایید کنید
edit-control-point-1 = نقطه کنترل ۱
edit-control-point-2 = نقطه کنترل ۲
edit-copy = کپی
edit-corner-radius-limited-so-replacement = شعاع گوشه محدود شده تا جایگزینی نتواند از رأس‌های مجاور عبور کند.
edit-create-new-layer = یک لایه جدید ایجاد کنید
edit-create-new-project = ایجاد یک پروژه جدید
edit-create-project = ایجاد پروژه
edit-delta-length-m-use = تغییر طول (متر، از + یا - استفاده کنید)
edit-dip = شیب
edit-direction = جهت
edit-distance = فاصله
edit-distance-along-slope = فاصله در امتداد شیب
edit-download-free-native-version-our = نسخه بومی رایگان را از وب‌سایت ما دانلود کنید ↗
edit-drill-hole = گمانه
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = پایان
edit-enter-valid-elevation = ارتفاع معتبری وارد کنید.
edit-exit-slice = قطعه خروج
edit-finish-polyline = تکمیل چندخطی
edit-generate-batter-berms = ایجاد پله‌ها و برم‌ها
edit-height = ارتفاع
edit-height-change = تغییر ارتفاع
edit-height-mode = حالت ارتفاع
edit-horizontal-distance = فاصله افقی
edit-horizontal-width-each-flat-berm = عرض افقی هر برم مسطح بین شیب‌های پله متوالی.
edit-hover-choose-which-end-move = برای انتخاب کدام انتها حرکت کنید، سپس برای تایید کلیک کنید.
edit-insert-point-elevation = نقطه ای را در ارتفاع قرار دهید
edit-intersect = تقاطع
edit-kind-properties = { $properties } { $kind }
edit-layer-name = نام لایه
edit-load-project = بارگذاری پروژه
edit-longest = بلندترین
edit-m-s = m/s
edit-measure = اندازه گیری
edit-mit-license = مجوز MIT
edit-mode = حالت
edit-move = حرکت کن
edit-move-layer = انتقال به لایه
edit-move-which-end = کدام انتها جابه‌جا شود؟
edit-movement-speed-slice-when-using = سرعت حرکت قطعه هنگام استفاده از کلید های ناوبری.
edit-moving-end-endpoint = در حال جابه‌جایی: نقطه پایان
edit-moving-start-endpoint = در حال جابه‌جایی: نقطه شروع
edit-new-length-m = طول جدید (متر)
edit-new-project = پروژه جدید
edit-number-complete-batter-berm-levels = تعداد ترازهای کامل شیب و برم. بیشینه به عمیق‌ترین ترازی محدود است که هندسهٔ مشخص‌شده را حفظ کند.
edit-bezier-segments-help = تعداد قطعات خط برای تقریب منحنی میان دو رأس انتخاب‌شده.
edit-chamfer-segments-help = تعداد قطعه‌های مستقیم برای تقریب گوشهٔ گرد. برای پخ مستقیم از ۱ استفاده کنید.
edit-object = شیء
edit-offset-element = عنصر آفست
edit-pick-side = طرف را انتخاب کنید
edit-pit = پیت معدن
edit-project-name = نام پروژه
edit-properties = ویژگی‌ها
edit-radius = شعاع
edit-recent = اخیر
edit-relative = نسبت (+/-)
edit-elevation-mode-help = «نسبی» تغییر عمودی را به همهٔ نقاط اعمال می‌کند. «RL مطلق» همهٔ نقاط را روی یک ارتفاع هدف می‌اندازد.
edit-remove-from-list = حذف از فهرست
edit-replace-path = مسیر تعویض
edit-rotate = چرخش
edit-rotation-speed-slice-when-using = سرعت چرخش قطعه در هنگام استفاده از Q و E
edit-s = °/s
edit-segments = بخش ها
edit-segments-lying-elevation-ignored = بخش هایی که در این ارتفاع قرار دارند نادیده گرفته می شوند.
edit-endpoint-help = نقطه انتهایی متغیر را انتخاب کنید؛ نقطه انتهایی دیگر ثابت می‌ماند.
edit-selected-holes-point-different-ways = چال‌های انتخاب‌شده در جهت‌های متفاوت‌اند. اعمال، همه را روی این زاویه‌ها می‌گذارد.
edit-selected-start-end-point-moves = نقطهٔ شروع یا پایان انتخاب‌شده در راستای خط حرکت می‌کند؛ نقطهٔ مقابل ثابت می‌ماند.
edit-set-axis = تنظیم { $axis }
edit-shortest = کوتاه‌ترین
edit-slice-view = نمای قطعه ای
edit-slope-angle-each-batter-face = زاویه منحنی هر شیب پله، اندازه گیری شده از افقی.
edit-slope-angle-offset-positive-negative = زاویهٔ شیب آفست. زاویه‌های مثبت و منفی هنگام حرکت جانبی، نسخه را بالاتر یا پایین‌تر از مبدأ می‌برند.
edit-speed = سرعت
edit-start = شروع
edit-stockpile = دپوی مواد
edit-stop-generated-offset-where-its = آفست تولیدشده را در جایی متوقف کنید که مسیر آن نخستین بار با مثلث‌بندی قابل مشاهده برخورد می‌کند.
edit-target-rl = تراز هدف
edit-text-colour-opacity = رنگ و شفافیت متن
edit-thickness-visible-slice-slab-centred = ضخامت صفحه قطعه قابل مشاهده روی شاخص نمای کلی متمرکز شده است.
edit-translation-axis-help = فاصله انتقال در امتداد محور جهانی { $axis }.
edit-type = نوع
edit-type-direction-together-set-offset = نوع و جهت با هم سمت آفست را تعیین می‌کنند. پیت + بالا و دپو + پایین رو به بیرون؛ پیت + پایین و دپو + بالا رو به داخل حرکت می‌کنند.
edit-bench-direction-help = «بالا» هر پله را به‌اندازهٔ ارتفاع پله بالا و «پایین» آن را پایین می‌برد. این کار سمت آفست را نیز برعکس می‌کند؛ «نوع» را ببینید.
edit-value-help = این مقدار با استفاده از حالت اندازه گیری و ارتفاع انتخاب شده تفسیر می شود.
edit-vertical-rise-fall-each-bench = افزایش یا سقوط عمودی هر پله قبل از ایجاد برم بعدی.
edit-bezier-control-point-1-help = مختصات جهانی X، Y و Z نخستین نقطه کنترل بزیه.
edit-bezier-control-point-2-help = مختصات جهانی X، Y و Z دومین نقطه کنترل بزیه.

## Events strings

events-couldn-t-exit-error = خروج ممکن نبود: { $error }
events-couldn-t-save-error = ذخیره ممکن نبود: { $error }
events-set-elevation = تنظیم ارتفاع
events-set-elevation-from-cursor-hit = ارتفاع بر اساس نقطهٔ نشانگر روی Z ‏{ $z } تنظیم شد
events-tool-not-available-section-view = این ابزار در نمای مقطع در دسترس نیست

## Explorer strings

explorer-clear-active-triangulation-texture = بافت شفاف فعال مثلث‌بندی
explorer-delete-from-project = حذف از پروژه
explorer-discard-changes = رد کردن تغییرات...
explorer-download = دانلود
explorer-drape-over-surface = انطباق در سطح
explorer-draped-over-surface = روی یک سطح پوشانده شده
explorer-duplicate = تکثیر
explorer-face-colour = رنگ صورت
explorer-id-block-model-id-source =
    شناسه: block-model:{ $id }{ $source }
    { $count } متغیر رنگ
explorer-id-drill-holes-id-source =
    شناسه: drill-holes:{ $id }{ $source }
    { $holes } گمانه
    { $fields } فیلد رنگ
explorer-id-point-cloud-id-source =
    شناسه: point-cloud:{ $id }{ $source }
    { $count } نقطه
explorer-raster-id =
    شناسه: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = شناسه: triangulation:{ $id }{ $source }
explorer-load = بارگذاری
explorer-lock = قفل
explorer-select-all-objects = تمام اشیاء را انتخاب کنید
explorer-source-name = منبع: { $name }
explorer-unload = تخلیه
explorer-unlock = باز کردن قفل

## Files strings

files-automatic-colour = رنگ خودکار
files-automatic-rl-spacing = فاصله‌گذاری خودکار ترازها
files-axis-scale-ratio = نسبت مقیاس محور { $axis }
files-ok = تأیید
files-reset-scale = بازنشانی به ۱×
files-rl-grid-options = گزینه‌های شبکه ترازها
files-rl-spacing = فاصله‌گذاری ترازها
files-scales-z-distances-visually-without = مقیاس Z فاصله های بصری بدون تغییر هماهنگی های ذخیره شده است.
files-thickness = ضخامت
files-xy-grid-options = گزینه‌های شبکه XY

## Gpu strings

gpu-cache-block-model-surface-build-failed = ساخت سطح مدل بلوکی شکست خورد: { $error }
gpu-cache-block-model-surface-build-worker = کارگر ساخت سطح مدل بلوکی قطع شد
gpu-cache-block-model-surface-chunk-rejected = قطعهٔ سطح مدل بلوکی پیش از تخصیص GPU رد شد: نمونه‌ها={ $instances } بایت، حد={ $limit } بایت
gpu-cache-block-volume-worker-disconnected = کارگر آماده‌سازی حجم بلوک قطع شد
gpu-cache-translucent-volume-could-not-built = حجم نیمه‌شفاف ساخته نشد ({ $error })؛ این مدل بلوکی به‌جای آن به‌شکل مکعب نمایش داده می‌شود.
gpu-cache-edge-chunk-rejected = قطعهٔ لبهٔ مثلث‌بندی پیش از تخصیص GPU رد شد: نمونه‌ها={ $instances } بایت، حد={ $limit } بایت
gpu-cache-triangulation-chunk-rejected = قطعهٔ مثلث‌بندی GPU پیش از تخصیص رد شد: رأس‌ها={ $vertices } بایت، نمایه‌ها={ $indices } بایت، حد={ $limit } بایت
gpu-cache-triangulation-too-many-vertices = مثلث‌بندی «{ $name }» دارای { $count } رأس است (> u32::MAX)؛ نمی‌توان آن را برای GPU قطعه‌بندی کرد
gpu-cache-triangulation-uploaded = مثلث‌بندی «{ $name }» در { $chunks } قطعهٔ فضایی بارگذاری شد ({ $faces } وجه)
i18n-active-language = زبان فعال { $language } است (تعبیه‌شده: { $bundled })
i18n-could-not-select-language-error = زبان انتخاب نشد: { $error }

## Init strings

init-gpu-adapter-vendor-name-backend = آداپتور گرافیک: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver = درایور گرافیک: { $driver } { $driver_info }
init-gpu-supports-maximum-buffer-size = بیشینهٔ اندازهٔ بافر GPU برابر { $size } MiB است؛ صحنه‌های بزرگ ممکن است کامل نمایش داده نشوند
init-surface-present-mode = حالت ارائهٔ سطح: { $mode }
init-wgpu-error-continuing-error = خطای wgpu (ادامهٔ اجرا): { $error }

## Io strings

io-ascii-points-xyz-pts = نقاط ASCII ‏(.xyz، .pts)
io-attribute = ویژگی
io-blank-header = (سرصفحه خالی)
io-block-model = مدل بلوکی:
io-choose-file-purpose-map-its = هدف فایل را برای نقشه برداری ستون های آن انتخاب کنید.
io-choose-loaded-block-model = یک مدل بلوکی بارگذاری‌شده انتخاب کنید
io-choose-loaded-layer = یک لایه بارگذاری‌شده انتخاب کنید
io-choose-loaded-triangulation = یک مثلث‌بندی بارگذاری‌شده انتخاب کنید
io-choose-purpose = کاربرد را انتخاب کنید…
io-choose-source-file-files-import = فایل منبع یا فایل های وارداتی را انتخاب کنید.
io-collar = دهانه گمانه
io-column-mapping = نقشه برداری ستون
io-comma-separated-values-csv = مقادیر جداشده با ویرگول (.csv)
io-csv-files = فایل‌های CSV
io-default = پیش فرض
io-depth = عمق
io-diameter = قطر
io-drawing-exchange-format-dxf = قالب تبادل نقشه (.dxf)
io-drill-holes = گمانه‌ها
io-east-x = شرق / X
io-elevation-z = تراز / Z
io-end-x = X پایان
io-end-y = Y پایان
io-end-z = Z پایان
io-explicit-segments = قطعات صریح
io-export = صادر کردن
io-export-csv-block-model = صادرات CSV مدل بلوکی
io-export-dxf = صادرات DXF
io-export-one-layer = صادرات یک لایه
io-export-ply = صادر کردن PLY
io-export-stl = صادر کردن STL
io-export-wavefront-obj = صادر کردن Wavefront OBJ
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = وارد کردن
io-import-ascii-point-cloud = وارد کردن ابر نقاط ASCII
io-import-drillhole-csv-bundle = واردات بسته CSV گمانه
io-import-geotiff = وارد کردن GeoTIFF
io-import-las-laz-point-cloud = وارد کردن ابر نقاط LAS/LAZ
io-import-pcd-point-cloud = وارد کردن ابر نقاط PCD
io-import-ply = وارد کردن PLY
io-import-stl = وارد کردن STL
io-import-wavefront-obj = وارد کردن Wavefront OBJ
io-interval = بازه
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-mapped-csv-bundle-csv = بسته CSV نگاشت‌شده (.csv)
io-model-file = فایل مدل
io-name-count-files = { $name } + { $count } فایل
io-no-csv-chosen = فایل .csv انتخاب نشده است
io-no-csv-files-chosen = هیچ فایل CSV انتخاب نشده است
io-no-dxf-chosen = فایل .dxf انتخاب نشده است
io-no-omf-chosen = فایل .omf انتخاب نشده است
io-north-y = شمال / Y
io-ply = PLY (.ply)
io-point-cloud-data-pcd = داده ابر نقاط (.pcd)
io-source-file = فایل مبدأ
io-start-x = X شروع
io-start-y = Y شروع
io-start-z = Z شروع
io-stl = STL (.stl)
io-triangulation = مثلث‌بندی:
io-unmapped = نگاشت‌نشده
io-wavefront-obj = Wavefront OBJ (.obj)

## Jobs strings

jobs-background-task-poll-label-ended = وظیفه پس‌زمینه «{ $poll_label }» بدون نتیجه پایان یافت
jobs-discarded-stale-result = نتیجهٔ پس‌زمینهٔ قدیمی «{ $poll_label }» چون یک مبدأ تغییر کرد یا بسته شد، کنار گذاشته شد

## Logging strings

logging-activity-completed = فعالیت تکمیل شد
logging-activity-started = فعالیت آغاز شد
logging-application-id-id = شناسه برنامه: { $id }
logging-application-name = نام برنامه: { $name }
logging-application-startup = راه‌اندازی برنامه
logging-build-target-os-architecture = هدف ساخت: { $os }-{ $architecture }
logging-completed = تکمیل شد
logging-count-messages = { $count } پیام
logging-desktop-session-xdg-session-type = نشست دسکتاپ: XDG_SESSION_TYPE={ $session }، XDG_CURRENT_DESKTOP={ $desktop }، WAYLAND_DISPLAY={ $wayland }، DISPLAY={ $display }
logging-initialising-incline-design = در حال راه‌اندازی Incline Design
logging-locale-environment = محیط محلی: LANG={ $lang }، LC_ALL={ $locale }، TZ={ $timezone }
logging-macos-session = نشست macOS: USER={ $user }، SHELL={ $shell }
logging-operating-system-gnu-linux = سیستم‌عامل: GNU / Linux
logging-operating-system-macos = سیستم‌عامل: macOS
logging-operating-system-microsoft-windows = سیستم‌عامل: Microsoft Windows
logging-pointer-width = پهنای اشاره‌گر: { $width } بیت
logging-process-id-id = شناسه فرایند: { $id }
logging-release-version = نسخه انتشار: { $version }
logging-renderer = رندرکننده
logging-rust-compiler-host = میزبان کامپایلر Rust: { $host }
logging-system = سیستم
logging-system-error = خطای سیستم
logging-unknown = نامشخص
logging-windows-session-sessionname-session = نشست Windows: SESSIONNAME={ $session }، USERNAME={ $user }
logging-working = در حال انجام…

## Mac strings

mac-cannot-install-macos-menu-bar = نوار منوی macOS را نمی‌توان خارج از رشتهٔ اصلی نصب کرد
mac-quit-app = خروج از { $app }

## Main strings

main-incline-design-web-startup-failed = راه‌اندازی Incline Design Web شکست خورد: { $error }

## Menu strings

menu-count-files-selected = { $count } فایل انتخاب شده است

## Object strings

object-edit-appearance = ظاهر
object-edit-arc-circle = قوس و دایره
object-edit-arc-segments = بخش‌های قوس
object-edit-bulge = برآمدگی
object-edit-bulge-arcs-horizontal-data-model = طبق مدل داده، قوس‌های برآمده افقی هستند: قوس در نقشه (پلان) می‌چرخد و ارتفاع به‌صورت خطی از یک رأس به رأس بعدی تغییر می‌کند.
object-edit-centre-x = مرکز X
object-edit-centre-y = مرکز Y
object-edit-centre-z = مرکز Z
object-edit-chord = وتر
object-edit-colour-layer = رنگ بر اساس لایه
object-edit-enter-number = یک عدد وارد کنید
object-edit-follow-owning-layer-s-colour = پیروی از رنگ لایه مالک به‌جای رنگ سنجاق‌شده به این شیء.
object-edit-id = شناسه
object-edit-identity = یکه
object-edit-insert-after = درج بعد از
object-edit-join-last-vertex-back-first = رأس آخر را دوباره به رأس اول متصل می‌کند.
object-edit-length = طول { $length } متر
object-edit-move-down = انتقال به پایین
object-edit-move-up = انتقال به بالا
object-edit-object-has-no-arc-segments = این شیء هیچ بخش قوسی ندارد.
object-edit-object-has-single-position = این شیء تنها یک موقعیت دارد.
object-edit-object-needs-least-required-vertices = این شیء حداقل به { $required } رأس نیاز دارد
object-edit-one-more-properties-not-valid = یک یا چند ویژگی عدد معتبری نیست
object-edit-perimeter-area = محیط { $length } متر، مساحت { $area } متر مربع
object-edit-reverse = معکوس کردن
object-edit-row-invalid-number = ردیف { $row }: موقعیت یا برآمدگی عدد معتبری نیست
object-edit-sweep = زاویه جاروب
object-edit-text-not-number = «{ $text }» عدد نیست
object-edit-vertices = رأس‌ها

## Omf strings

omf-element-name-has-count-tie = عنصر «{ $name }» دارای { $count } اتصال است که نام چال‌هایی را دارد که دیگر در آن نیستند
omf-ignoring-colour-map-omf-attribute = نقشهٔ رنگ ویژگی OMF «{ $attribute }» نادیده گرفته می‌شود: { $error }
omf-mining-data-exported-incline = داده‌های معدنی صادرشده توسط Incline
omf-import = وارد کردن OMF
omf-texture = بافت OMF
omf-validation-warnings = هشدارهای اعتبارسنجی OMF: { $warnings }
omf-application-metadata-dropped = فرادادهٔ برنامهٔ پروژه «{ $application }» نگه داشته نمی‌شود
omf-project-author-not-retained = نویسنده پروژه نگه‌داری نمی‌شود
omf-project-description-not-retained = توضیحات پروژه نگه‌داری نمی‌شود
omf-unsupported-metadata-keys = پروژه دارای کلیدهای فرادادهٔ پشتیبانی‌نشده است: { $keys }

## Plot strings

plot-1-1000-one-millimetre-sheet = در ۱:۱۰۰۰، یک میلی متر روی ورق یک متر روی زمین است.
plot-1-scale-covers-width-height = 1:{ $scale } · پوشش { $width } × { $height } متر
plot-all-visible-data = همهٔ داده‌های قابل مشاهده
plot-automatic-grid-interval = فاصله خودکار شبکه
plot-border = مرز
plot-centre = مرکز بر
plot-fit-scale-help = کوچکترین مقیاس معمولی را انتخاب کنید که متناسب با هر چیزی که روی ورق دیده می شود.
plot-coordinate-grid = شبکه هماهنگی
plot-current-view-centre = مرکز نمای فعلی
plot-date-caps = تاریخ
plot-date = تاریخ
plot-dots-per-inch-paper-size = نقطه در اینچ. این اندازهٔ کاغذ تا { $max_dpi } dpi رستری می‌شود؛ ۳۰۰ dpi کیفیت معمول چاپ است.
plot-dpi = dpi
plot-drawing-no = شماره نقشه
plot-drawing-number = شماره نقاشی
plot-drawn-by-caps = ترسیم‌کننده
plot-drawn-by = ترسیم‌شده توسط
plot-e-g-example-gold-project = مثلاً پروژه نمونه طلا
plot-entered-coordinates = مختصات واردشده
plot-export-png = صادرات PNG...
plot-fit-scale-visible-data = مقیاس مناسب برای داده های قابل مشاهده
plot-grid-interval = فاصله شبکه
plot-landscape = افقی
plot-lists-visible-surfaces-design-layers = سطوح قابل مشاهده و لایه های طراحی را با رنگ های آنها فهرست می کند.
plot-margin = حاشیه
plot-margins-leave-no-room-map = حاشیه‌ها فضایی برای نقشه باقی نمی‌گذارند
plot-metres-scale-1-scale = متر    مقیاس 1:{ $scale }
plot-mm = mm
plot-north-arrow = پیکان شمال
plot-nothing-visible-draw = چیزی برای رسم قابل مشاهده نیست
plot-paper = کاغذ
plot-paper-orientation-width-height-mm = { $paper } ‏{ $orientation } · ‏{ $width } × { $height } میلی‌متر
plot-paper-size = اندازه کاغذ
plot-pick-interval-reads-roughly-every = فاصله ای را انتخاب کنید که تقریباً هر ۵۰ میلی متر در ورق چاپ شده خوانده شود.
plot-plan = پلان
plot-scale-must-be-positive = مقیاس ترسیم باید عددی مثبت باشد
plot-png-written-sheet-s-exact = PNG با اندازهٔ فیزیکی دقیق برگه نوشته و DPI آن ثبت می‌شود، بنابراین در مقیاس واقعی چاپ خواهد شد.
plot-portrait = عمودی
plot-resolution = وضوح
plot-rev = بازبینی
plot-revision = تجدید نظر
plot-scale = مقیاس
plot-scale-ratio = مقیاس 1:
plot-scale-framing = مقیاس و چارچوب بندی
plot-sheet-furniture = عناصر جانبی ورق
plot-size-width-height-mm = { $size } ‏({ $width } × { $height } میلی‌متر)
plot-subtitle = زیرنویس
plot-title = عنوان
plot-title-block = بلوک عنوان
plot-today = امروز

## Products strings

products-add-initiation = افزودن آغازش
products-delay = تاخیر
products-delay-palette = پلت تاخیر
products-how-long-after-shot-fired = مدت‌زمان پس از شلیک که این یقه آغازش را انجام می‌دهد.
products-initiation-name = آغازش · { $name }
products-milliseconds-between-one-hole-firing = میلی ثانیه بین یک سوراخ و یک سوراخ دیگر.
products-ms = ms
products-no-products = محصولی وجود ندارد
products-remove = برداشتن
products-update = به‌روزرسانی

## Progress strings

progress-percent-done-total = { $percent } ‏({ $done } از { $total })
progress-task-finished = { $task }: پایان یافت

## Project strings

project-item = مورد

## Properties strings

properties-adds-view-dependent-rim-highlight = در مرز بلوک‌ها و مواد، درخشش لبهٔ وابسته به زاویهٔ دید می‌افزاید. خاموش‌کردن آن اندکی کار رندر حجمی را کم می‌کند.
properties-block-model-downscale = مدل بلوکی در مقیاس پایین
properties-camera = دوربین
properties-camera-clip-planes = هواپیماهای کلیپ دوربین
properties-cap-while-resizing = کپی در هنگام تغییر اندازه
properties-dark-mode = حالت تاریک
properties-developer = توسعه‌دهنده
properties-downscale-rasters = کاهش مقیاس رسترها
properties-edit-object = ویرایش شیء...
properties-field-view = میدان دید
properties-fps = فریم/ثانیه
properties-frame-counter = شمارنده فریم
properties-frame-rate-cap = سقف نرخ فریم
properties-hz = Hz
properties-interface = رابط کاربری
properties-invert-horizontal = برعکس افقی
properties-invert-vertical = برعکس عمودی
properties-limits-newly-loaded-geotiff-previews = پیش‌نمایش GeoTIFF تازه را در بلندترین ضلع به ۴۰۹۶ پیکسل محدود می‌کند. برای وضوح کامل تا حد بافت GPU خاموش کنید؛ حافظهٔ بیشتری مصرف می‌شود.
properties-line-colour = رنگ خط
properties-look-sensitivity = حساسیت نگاه
properties-max-clip-span = بیشینه بازه کلیپ
properties-move-layer = انتقال به لایه...
properties-near-clip-limit = حد کلیپ نزدیک
properties-orbit-sensitivity = حساسیت مدار
properties-panel-chrome = کروم پنل
properties-performance = کارایی
properties-plan-mode = حالت پلان
properties-presents-step-display-no-tearing = هم‌زمان با نمایشگر نمایش داده می‌شود: بدون پارگی تصویر، و نمایشگر نرخ فریم را تعیین می‌کند. در حالت خاموش، فریم‌ها بلافاصله پس از رسم نمایش داده می‌شوند و سقف زیر اعمال می‌شود.
properties-reflective-block-edges = لبه های بازتابگر بلوک
properties-restore-defaults = بازیابی پیش‌فرض‌ها
properties-show-console = نمایش کنسول
properties-shows-live-near-far-projection = فاصله‌های زنده تصویرسازی نزدیک و دور را در نوار وضعیت نشان می‌دهد.
properties-snap-polling = پایش گیره
properties-vertical-sync = همگام‌سازی عمودی
properties-world-axis-gizmo = محور جهان
properties-zoom-cursor = زوم به کرسر
properties-zoom-sensitivity = حساسیت زوم

## Screenshot strings

screenshot-could-not-encode-viewport-image = تصویر نمای دید کدگذاری نشد: { $error }
screenshot-could-not-map-viewport-screenshot = نگاشت تصویر نمای دید ممکن نشد: { $error }
screenshot-could-not-save-viewport-image = تصویر نمای دید { $path } ذخیره نشد: { $error }
screenshot-downloaded-viewport-image-file-name = تصویر نمای دید بارگیری شد: { $file_name }
screenshot-saved-viewport-image-path = تصویر نمای دید ذخیره شد: { $path }
screenshot-viewport-image-download-failed-error = بارگیری تصویر نمای دید شکست خورد: { $error }

## Spatial strings

spatial-bvh-face-index-out-of-range = نمایهٔ وجه BVH به شمارهٔ { $index } بیرون از محدودهٔ مش است؛ مثلث تباهیده جایگزین می‌شود

## State strings

state-above = در یا بالاتر از
state-activate-project = فعال کردن پروژه
state-all-open-incline-design-data = همهٔ داده‌های باز Incline Design
state-apply-generated-rings = اعمال حلقه‌های ایجادشده
state-apply-selection = اعمال به انتخاب
state-rotate-by-azimuth-dip = با آزیموت { $azimuth }° و شیب { $dip }°
state-rotate-to-azimuth-dip = به آزیموت { $azimuth }° و شیب { $dip }°
state-below = در یا زیر
state-centre-rotation = مرکز چرخش
state-checking-unsaved-work = در حال بررسی کار ذخیره‌نشده
state-choose-destination = مقصد را انتخاب کنید
state-choose-one-more-files = یک یا چند فایل انتخاب کنید
state-clear-raster = شفاف رستر
state-click-pit-shell-viewport = روی پوسته پیت معدن در نمای دید کلیک کنید.
state-click-pit-stockpile-solid-viewport = روی پیت معدن یا دپوی مواد جامد در نمای دید کلیک کنید.
state-click-surface-viewport = روی سطح موجود در نمای دید کلیک کنید.
state-click-topology-viewport = روی سطح توپوگرافی در نمای دید کلیک کنید.
state-close-project = بستن پروژه
state-colour-drillholes = رنگ گمانه‌ها
state-copy-objects-layer = کپی اشیا به لایه
state-count-file-s = { $count } فایل
state-count-object-s-axis-value = { $count } شیء · { $axis } { $value }
state-count-object-s-closed = { $count } شیء · { $closed }
state-count-object-s-layer = { $count } شیء · { $layer }
state-count-object-s-weight = { $count } شیء · { $weight }
state-count-object-s-z-elevation = { $count } شیء · Z { $elevation }
state-create-point-cloud-tin = ایجاد ابر نقاط TIN
state-create-project = ایجاد پروژه
state-current-project = پروژه فعلی
state-cut-topology-pit-shell = قطع سطح توپوگرافی به پوسته پیت معدن
state-cut-triangulation-polyline = قطع مثلث‌بندی توسط چندخطی
state-cut-triangulation-z = قطع مثلث‌بندی توسط Z
state-dark-mode = حالت تاریک
state-detached = جداشده
state-disabled = غیرفعال
state-discard-project-changes = رد کردن تغییرات پروژه
state-discard-replace-project = رد کردن و جایگزینی پروژه
state-discarding-unsaved-changes = در حال کنار گذاشتن تغییرات ذخیره‌نشده
state-docked = متصل
state-drape-raster = انطباق رستر
state-drill-pattern = پترن حفاری
state-duplicate-layer = لایه دوگانه
state-east = شرق
state-enabled = فعال
state-exit-incline-design = خروج از Incline Design
state-export-block-model-csv = صادرات CSV مدل بلوکی
state-export-layer-dxf = صادرات لایه به DXF
state-export-omf = صادرات OMF
state-export-project-dxf = صادرات پروژه به DXF
state-export-triangulation = صادرات مثلث‌بندی
state-export-viewport-image = صادرات تصویر نمای دید
state-finish-closed-polyline = پایان چندخطی بسته
state-finish-open-polyline = پایان چندخطی باز
state-fit-extents = جا دادن در محدوده
state-fix-release-centre-both-views = مرکزی را که هر دو نما حول آن می‌چرخند، ثابت یا آزاد می‌کند
state-generate-contours = تولید منحنی‌های میزان
state-hidden = پنهان
state-import-drillholes = واردات گمانه‌ها
state-import-omf = واردات OMF
state-import-point-cloud = واردات ابر نقاط
state-import-raster = واردات رستر
state-import-triangulation = واردات مثلث‌بندی
state-insert-intersection-points = نقطه های تقاطع را وارد کنید
state-insert-points-elevation = نقطه ها را در ارتفاع اضافه کنید
state-keep-inside = نگه‌داشتن داخل
state-keep-outside = نگه‌داشتن بیرون
state-kriged-block-model = مدل بلوکی کریج‌شده
state-load-block-model = بارگذاری مدل بلوکی
state-load-drillholes = بارگذاری گمانه‌ها
state-load-layer = بارگذاری لایه
state-load-point-cloud = بارگذاری ابر نقاط
state-load-raster = بارگذاری رستر
state-load-triangulation = بارگذاری مثلث‌بندی
state-locked-count-object-s = { $count } شیء قفل‌شده
state-major-minor = اصلی { $major } · فرعی { $minor }
state-move-axis-value = انتقال به مقدار محور
state-move-objects-layer = انتقال اشیا به لایه
state-name-count-holes = { $name } · { $count } چال
state-name-count-object-s = { $name } · { $count } شیء
state-name-z-min-z-max = { $name } · از { $z_min } تا { $z_max }
state-next-edit = ویرایش بعدی
state-north = شمال
state-open-containing-folder = باز کردن پوشه دربرگیرنده
state-open-project = باز کردن پروژه
state-preserve-view-angle = حفظ زاویه دید
state-previous-edit = ویرایش قبلی
state-project-id = پروژه { $id }
state-remove-block-model = حذف مدل بلوکی
state-remove-drillholes = حذف گمانه‌ها
state-remove-point-cloud = حذف ابر نقاط
state-remove-raster = حذف رستر
state-remove-triangulation = حذف مثلث‌بندی
state-removed-from-active-triangulation = از مثلث‌بندی فعال حذف شد
state-removed-from-every-triangulation = از همه مثلث‌بندی‌ها حذف شد
state-rename-kind = تغییر نام { $kind }
state-save-close-project = ذخیره و بستن پروژه
state-save-despite-unsupported-content = ذخیره با وجود محتوای پشتیبانی‌نشده
state-save-project = ذخیره پروژه با نام
state-save-replace-project = ذخیره و جایگزینی پروژه
state-saving-current-project = در حال ذخیره پروژه فعلی
state-section-name = بخش { $section }
state-select-layer-objects = اشیاء لایه را انتخاب کنید
state-selected-objects = اشیای انتخاب‌شده
state-selected-polylines = چندخطی‌های انتخاب‌شده
state-selected-scene-elements = عناصر انتخاب‌شده صحنه
state-set-block-model-variable = تنظیم متغیر مدل بلوکی
state-set-drillhole-colour-preset = تنظیم پیش‌تنظیم رنگ گمانه
state-set-entity-lock = تنظیم قفل شیء
state-set-grid = تنظیم شبکه
state-set-layer-lock = تنظیم قفل لایه
state-set-line-weight = تنظیم ضخامت خط
state-set-object-colour = رنگ اشیاء را تنظیم کنید
state-set-object-fill = تنظیم پر کردن اشیاء
state-set-point-visibility = تنظیم نمایانی نقطه
state-set-polyline-closed = تنظیم چندخطی به‌صورت بسته
state-set-raster-lock = تنظیم قفل رستر
state-set-standard-view = تنظیم نمای استاندارد
state-set-topology-wireframes = تنظیم نماهای سیمی سطح توپوگرافی
state-set-triangulation-colour = تنظیم رنگ مثلث‌بندی
state-show-console = نمایش کنسول
state-show-project = نمایش پروژه
state-shown = نمایش‌داده‌شده
state-slice-mode = حالت برش
state-slice-preview = پیش نمایش قطعه
state-south = جنوب
state-stem-contours = خطوط تراز { $stem }
state-target-new-name = { $target } به «{ $new_name }»
state-trim-above = برش از بالا
state-trim-below = برش از پایین
state-trim-triangulation-surface = برش مثلث‌بندی تا سطح
state-undrape-raster = لغو انطباق رستر
state-undrape-rasters = لغو انطباق رسترها
state-unload-block-model = تخلیه مدل بلوکی
state-unload-drillholes = تخلیه گمانه‌ها
state-unload-layer = تخلیه لایه
state-unload-point-cloud = تخلیه ابر نقاط
state-unload-raster = تخلیه رستر
state-unload-triangulation = تخلیه مثلث‌بندی
state-untitled-project = پروژه بدون عنوان
state-use-typed-radius = استفاده از شعاع واردشده
state-west = غرب

## Status strings

status-clip-near-far = کلیپ نزدیک / دور / Δ: -- / -- / --
status-frame-rate = نرخ فریم

## Text strings

text-could-not-build-vector-mesh = مش برداری برای قلم { $font }، نویسهٔ { $glyph } ساخته نشد: { $error }
text-document-text-mesh-exceeded-its = مش متن سند از بازهٔ نمایهٔ u32 فراتر رفت

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = ابتدا مجموعه‌دادهٔ چال برای اتصال را انتخاب کنید
tie-in-count-connector-s = { $count } رابط
tie-in-delete-tie-ins = حذف اتصال‌ها
tie-in-deleted-count-selected-tie-connector = { $count } رابط انتخاب‌شده حذف شد
tie-in-hole = چال
tie-in-initiation-point-lifted-from-name = نقطهٔ آغازش از { $name } برداشته شد
tie-in-initiation-point-set-name-delay = نقطهٔ آغازش روی { $name } با تأخیر { $delay } میلی‌ثانیه تنظیم شد
tie-in-select-delay-product-palette-before = پیش از اتصال چال‌ها، یک محصول تأخیر در پالت انتخاب کنید
tie-in-tied-connectors = { $count } رابط با تأخیر { $delay } میلی‌ثانیه و { $product } متصل شد
tie-in-tied-connectors-replacing = { $count } رابط با تأخیر { $delay } میلی‌ثانیه و { $product } متصل شد و { $replaced } را جایگزین کرد

## Toolbar strings

toolbar-fill-type = نوع پرشدن

## Toolbars strings

toolbars-auto-bench = پلهٔ خودکار
toolbars-bezier-polyline = چندخطی بزیه
toolbars-chamfer-polyline-corners = پخ‌زنی گوشه‌های چندخطی
toolbars-create-text = ایجاد متن
toolbars-cursor-regular = نشانگر: عادی
toolbars-cursor-snap-line = نشانگر: چسبیدن به خط
toolbars-cursor-snap-point = نشانگر: چسبیدن به نقطه
toolbars-cursor-snap-surface = نشانگر: چسبیدن به سطح
toolbars-delete-points = حذف نقاط
toolbars-explode-polyline-lines = تجزیه چندخطی به خطوط
toolbars-fuse-polylines = اتصال چندخطی‌ها
toolbars-measure-distance = اندازه‌گیری فاصله
toolbars-new-layer = لایه جدید
toolbars-split-polyline-points = تقسیم چندخطی در نقاط
toolbars-strike-dip = امتداد و شیب
toolbars-tool-not-available-section-view = { $tool } - در نمای مقطع در دسترس نیست

## Tri strings

tri-sampling-method-help = روش تطبیقی با خطای برازش صفحه، رأس‌ها را روی زمین پیچیده متمرکز می‌کند؛ روش یکنواخت آن‌ها را به‌طور مساوی پخش می‌کند. ممکن است در آینده روش‌های بیشتری افزوده شود.
tri-adaptive-quadtree = تطبیقی (درخت چهارتایی)
tri-axis-range = بازه محور { $axis }
tri-base-topology-will-receive-pit = سطح توپوگرافی پایه‌ای که شکل پیت یا دپو را دریافت می‌کند.
tri-boundary-polyline = مرز چندخطی
tri-bridge-gaps-help = شکاف‌ها و فرورفتگی‌های مرزی باریک‌تر از این مقدار را در سطح متصل کنید. مقدار ۰ همچنان شکاف‌هایی تا حدود اندازه سلول نمونه‌برداری را متصل می‌کند؛ مقادیر بزرگ‌تر حفره‌های بزرگ‌تر را پر و فرورفتگی مرز را کاهش می‌دهند.
tri-budget = بودجه بر اساس
tri-cancel-pick = انتخاب را لغو کنید
tri-candidate-detail = جزئیات کاندید
tri-candidate-fine-cells-per-budgeted = سلول‌های ریز نامزد به‌ازای هر رأس بودجه‌بندی‌شده. مقدار بیشتر برای جای‌گذاری جزئیات به نمونه‌بردار تطبیقی آزادی بیشتری می‌دهد، اما ساخت را کندتر می‌کند.
tri-cap-surface-share-source-points = سطح را با بخشی از نقاط منبع یا با تعداد دقیق رأس‌ها محدود کنید.
tri-choose-input-clicking-loaded-surface = این ورودی را با کلیک روی سطح بارگذاری‌شده در نمای دید انتخاب کنید
tri-choose-which-side-reference-topology = در محدوده XY مشترک، سمتی از سطح توپوگرافی مرجع را که باید از سطح حذف شود انتخاب کنید.
tri-clip = کلیپ
tri-clip-creates-new-triangulation-name = برش، مثلث‌بندی جدیدی با این نام ایجاد می‌کند؛ سطح مبدأ تغییر نمی‌کند.
tri-clip-surface-polyline = Clip Surface توسط چندخطی
tri-closed-pit-stockpile-solid-whose = یک حجم بسته پیت یا دپو که مرز نمایان آن در نتیجه گنجانده می‌شود.
tri-create-new-layer-contours-append = برای منحنی‌ها لایه‌ای جدید بسازید یا آن‌ها را به لایه‌ای موجود در پروژهٔ فعال بیفزایید.
tri-cut-topology-pit-shell = قطع سطح توپوگرافی با پوسته پیت معدن
tri-e-g-design-trimmed = به عنوان مثال design_trimmed
tri-e-g-mysurf-cut = به عنوان مثال mysurf_cut
tri-e-g-mysurf-slice = به عنوان مثال MySurf_slice
tri-e-g-surface-contour = به عنوان مثال: surface_contour
tri-e-g-topo-cut = به عنوان مثال top_cut
tri-e-g-topo-pit = به عنوان مثال topo_with_pit
tri-exact-number-surface-vertices-target = تعداد دقیق رأس‌های سطح هدف. مقادیر بسیار بزرگ، ساخت را کند و حافظه زیادی مصرف می‌کنند.
tri-existing-ground-topology-will-cut = سطح توپوگرافی زمین موجود که با پوسته پیت بریده خواهد شد.
tri-fill-holes-up = پر کردن حفره‌ها تا
tri-generate = تولید
tri-generate-contour-lines = تولید خطوط منحنی میزان
tri-generate-upper-surface = تولید سطح بالا
tri-hide-unload-sources = منابع پنهان و تخلیه
tri-higher-edge-will-enforced-each = در هر تعارض، یال بالاتر اعمال می‌شود. قطعات متعارض پایین‌تر به‌عنوان خط شکست نادیده گرفته می‌شوند و سطح در آن نواحی درون‌یابی خواهد شد. چندخطی‌های مبدأ تغییر نمی‌کنند.
tri-breaklines-cross = یال‌های خط شکست برجسته‌شده در پلان و در ترازهای متفاوت همدیگر را قطع می‌کنند یا هم‌پوشانی دارند. یک سطح زمین نمی‌تواند از هر دو پیروی کند.
tri-intervals-colours = فاصله ها و رنگ ها
tri-keep-clipped-topology-included-shape = سطح توپوگرافی بریده‌شده و شکل گنجانده‌شده را به‌جای ترکیب در یک موجودیت، به‌صورت مثلث‌بندی‌های جدا نگه دارید.
tri-keep-inside-discards-surface-outside = «نگه‌داشتن داخل» سطح بیرون چندخطی را حذف می‌کند. «نگه‌داشتن بیرون» حفره‌ای به شکل چندخطی از سطح می‌بُرد.
tri-keeps-only-surface-within-polyline = فقط سطح داخل مرز چندخطی را نگه می‌دارد.
tri-keep-surface-relation-help = سطح را در پوشش XY سطح توپوگرافی { $relation } نگه می‌دارد.
tri-layer-already-exists-select-above = آن لایه از قبل وجود دارد؛ آن را در بالا انتخاب کنید یا نام دیگری برگزینید.
tri-limit-z-range = محدود کردن بازه Z
tri-major = اصلی
tri-max-edge-length = حداکثر طول لبه
tri-merge = ادغام
tri-method = روش
tri-min = کمینه
tri-minimum-maximum-elevations-retained = کمینه و بیشینه تراز حفظ‌شده در سطح خروجی. کمینه باید پایین‌تر از بیشینه باشد.
tri-minor = فرعی
tri-contour-interval-help = «فرعی» منحنی‌های معمول و «اصلی» منحنی‌های برجسته را کنترل می‌کند؛ فاصلهٔ اصلی باید دست‌کم برابر فرعی باشد.
tri-move-cursor-over-loaded-surface = نشانگر را روی یک سطح بارگذاری‌شده ببرید.
tri-slice-output-name-help = نام اختصاص داده شده به سطح خروجی قطع شده با ارتفاع.
tri-name-assigned-merged-topology-pit = نام اختصاص داده شده به نتیجه سطح توپوگرافی و پیت معدن/دپوی مواد ترکیب شده.
tri-name-assigned-newly-created-contour = نام اختصاص داده شده به لایه تازه ایجاد شده منحنی میزان
tri-reconstruct-output-name-help = نام اختصاص داده شده به مثلث‌بندی بازسازی شده.
tri-name-assigned-topology-after-pit = نام اختصاص داده شده به سطح توپوگرافی پس از قطع پوسته پیت معدن از آن.
tri-name-assigned-trimmed-output-surface = نام اختصاص داده شده به سطح خروجی برش شده.
tri-nearby-breakline-vertices-do-not = رأس‌های نزدیک خطوط شکست دقیقاً در یک موقعیت به هم نمی‌رسند، بنابراین سطح را نمی‌توان مثلث‌بندی کرد.
tri-new-layer = لایه جدید
tri-new-layer-name = نام لایه جدید
tri-once-merge-succeeds-unload-source = پس از ادغام موفق، توپولوژی و جسم مبدأ را خارج کنید تا فقط نتیجهٔ ادغام‌شده در صحنه بماند.
tri-only-loaded-pickable = فقط مثلث‌بندی‌ها باردار را می توان انتخاب کرد.
tri-operation = عملیات
tri-output-layer = لایه خروجی
tri-percentage = درصد
tri-percentage-cloud = درصد ابر
tri-pick-from-view = انتخاب از نمای دید
tri-pit-design-surface-only-areas = سطح طراحی پیت. فقط نواحی‌ای که پایین‌تر از سطح توپوگرافی حفاری می‌کند برای برش استفاده می‌شوند.
tri-pit-shell = پوسته پیت
tri-pit-stockpile-solid = حجم پیت/دپو
tri-recommended-weld-retry = توصیه می شود: جوش و تکرار
tri-reconstruct-help = یک سطح زمین مثلث‌بندی‌شده را از ابر نقاط بازسازی کنید. نمونه‌بردار تطبیقی بودجه رأس را در پیچیده‌ترین بخش‌های زمین مصرف و نواحی مسطح را تنک نگه می‌دارد.
tri-reduce-budget-candidate-detail-if = اگر دستگاه شما RAM کمتری دارد، بودجه یا جزئیات نامزد را کاهش دهید.
tri-reference-topology-help = سطح توپوگرافی مرجعی که محل برش سطح دیگر را تعیین می‌کند.
tri-reject-reconstructed-triangle-edges = یال‌های مثلث بازسازی‌شده بلندتر از این فاصله را رد کنید. برای نداشتن محدودیت طول یال از ۰ استفاده کنید.
tri-remove-inside-help = سطح داخل مرز چندخطی را حذف و باقی را نگه می‌دارد.
tri-removes-topology-where-pit-shell = سطح توپوگرافی را جایی که پوسته پیت زیر آن حفاری می‌کند حذف می‌کند تا پوسته حفره را پر کند. درز از خط تماس واقعی سه‌بعدی میان سطوح پیروی می‌کند؛ سطح توپوگرافی زیر بخش‌هایی از پوسته که بالای زمین قرار دارند حفظ می‌شود.
tri-result = نتیجه
tri-save-two-entities = ذخیره به‌صورت دو موجودیت جداگانه
tri-select = انتخاب…
tri-share-source-points-keep-fractions = سهمی از نقاط منبع که باید حفظ شود. کسرهایی مانند ۰٫۱۲۵٪ نیز مجاز هستند.
tri-slice-triangulation-z-range = برش مثلث‌بندی بر اساس بازه Z
tri-solution-generate-upper-surface = راه حل: سطح بالا را تولید کنید
tri-surface-trim = سطح برای برش
tri-target-surface-help = سطحی که تغییر خواهد کرد؛ سطح توپوگرافی انتخاب‌شده دست‌نخورده می‌ماند.
common-percent-suffix = %
tri-topology = سطح توپوگرافی
tri-triangulation-failed = مثلث‌بندی شکست خورد
tri-trim = برش
tri-trim-topology = برش تا سطح توپوگرافی
tri-uniform-grid = شبکه یکنواخت
tri-up-target-point-count-points = حداکثر { $target } از { $point_count } نقطه به رأس سطح تبدیل می‌شوند ({ $percent }%).
tri-use-full-surface-elevation-range = از تمام محدوده ارتفاع سطح استفاده کنید
tri-vertex-count = تعداد رأس‌ها
tri-vertices-within-5-cm-xy = رأس‌هایی که در XY و Z در فاصله ۵ سانتی‌متری قرار دارند، برای این مثلث‌بندی یک موقعیت مشترک خواهند داشت. این کار می‌تواند سطح تولیدشده را به‌طور موضعی تا ۵ سانتی‌متر جابه‌جا کند؛ چندخطی‌های مبدأ تغییر نمی‌کنند.
tri-weld-retry = جوش و تکرار
tri-when-enabled-generate-contours-only = در صورت فعال‌بودن، منحنی‌های میزان فقط میان کمینه و بیشینهٔ ارتفاع مشخص‌شده ایجاد می‌شوند.

## Ui strings

ui-choose-offset-side = سمت آفست را انتخاب کنید
ui-choose-relimit-side = طرف بازحدگذاری را انتخاب کنید
ui-click-circle-centre = روی مرکز دایره کلیک کنید
ui-click-closed-polyline-use-blast = برای استفاده از پلی‌لاین بسته به‌عنوان شکل انفجار روی آن کلیک کنید
ui-click-collar-add-edit-initiation = برای افزودن یا ویرایش نقطهٔ آغازش روی یقه کلیک کنید
ui-click-first-point-slice-line = بر روی نقطه اول خط برش کلیک کنید
ui-click-first-vertex = روی رأس اول کلیک کنید
ui-click-perimeter-point-type-radius = روی نقطه محيط کلیک کنید یا شعاعی را تایپ کنید
ui-click-second-point-slice-line = روی نقطه دوم خط برش کلیک کنید
ui-click-second-vertex = روی رأس دوم کلیک کنید
ui-click-use-pointer-radius = یا برای استفاده از شعاع اشاره کلیک کنید
ui-could-not-copy-text-browser = متن در کلیپ‌بورد مرورگر کپی نشد: { $error }
ui-dip-horizontal-no-strike = { $dip } (افقی، بدون امتداد)
ui-distance-meters = { $distance } متر
ui-drag-ring-type-azimuth-dip = یک حلقه را بکشید یا آزیموت و شیب را وارد کنید
ui-each-hole-turns-about-its = هر چال حول یقهٔ خود می‌چرخد
ui-enter-positive-decimal-radius = یک شعاع اعشاری مثبت وارد کنید
ui-esc-cancels = Esc لغو می‌کند
ui-no-delay-product-tie = محصول تأخیری برای اتصال وجود ندارد
ui-press-enter-use-typed-radius = برای استفاده از شعاع تایپ شده، فشار Enter را فشار دهید
ui-right-click-delay-palette-heading = برای افزودن یکی، روی سرتیتر پلت تأخیر کلیک راست کنید
ui-select-designs = طرح ها را انتخاب کنید
ui-select-drill-hole = یک چال انتخاب کنید
ui-select-endpoint-join = نقطه آخر را برای پیوستن انتخاب کنید
ui-select-first-crest-toe-point = اولین نقطه تاج/پای پله را انتخاب کنید
ui-select-item = یک آیتم را انتخاب کنید
ui-select-line-fuse = خطی را برای ترکیب انتخاب کنید
ui-select-line-polyline = یک خط یا چندخطی را انتخاب کنید
ui-select-line-relimit = خط را به بازحدگذاری انتخاب کنید
ui-select-next-line-fuse = خط بعدی را برای فشرده سازی انتخاب کنید
ui-select-opposite-berm-point = نقطه مقابل برم را انتخاب کنید
ui-select-point = یک نقطه را انتخاب کنید
ui-select-polyline = یک چندخطی را انتخاب کنید
ui-select-polyline-open-line = یک چندخطی یا خط باز را انتخاب کنید
ui-select-polyline-vertex = یک رأس چندخطی را انتخاب کنید
ui-select-second-crest-toe-point = دومین نقطه تاج/پای پله را انتخاب کنید
ui-select-second-split-point = نقطه تقسیم دوم را انتخاب کنید
ui-select-split-point = نقطه تقسیم را انتخاب کنید
ui-select-topologies = انتخاب سطوح توپوگرافی
ui-slice-view = نمای برش
ui-strike-dip = امتداد { $strike }° · { $dip }
ui-value-dip = شیب { $value }°

## Viewport strings

viewport-all-total-categories-keep-their = همهٔ { $total } دسته رنگ خود را حفظ می‌کنند؛ فقط { $shown } دستهٔ نخست متمایز رسم می‌شوند
viewport-axis-maximum = بیشینه { $axis }
viewport-axis-minimum = کمینه { $axis }
viewport-bar-blast-timeline-placeholder = خط زمانی آتشباری [جای‌نگهدار]
viewport-bar-burden-relief-heatmap-placeholder = نقشه حرارتی آزادسازی بارسنگ [جای‌نگهدار]
viewport-bar-color = رنگ:
viewport-bar-contours-equal-time-placeholder = منحنی‌های میزان از زمان برابر [PLACEHOLDER]
viewport-bar-disable-flying-mode = غیرفعال کردن حالت پرواز
viewport-bar-disable-x-ray-vision = غیرفعال کردن نمای اشعه ایکس
viewport-bar-drill-holes = گمانه‌ها:
viewport-bar-enable-flying-mode = فعال کردن حالت پرواز
viewport-bar-enable-x-ray-vision = فعال کردن نمای اشعه ایکس
viewport-bar-exit-slice-view = خروج از نمای برش
viewport-bar-fill = پر کردن:
viewport-bar-fix-centre-rotation = ثابت کردن مرکز چرخش
viewport-bar-hide-points = پنهان کردن نقاط
viewport-bar-hide-rl-grid = پنهان کردن شبکه ترازها
viewport-bar-hide-wireframes = پنهان کردن قاب‌های سیمی
viewport-bar-hide-xy-grid = پنهان کردن شبکه XY
viewport-bar-release-centre-rotation = آزاد کردن مرکز چرخش
viewport-bar-show-points = نمایش نقاط
viewport-bar-show-rl-grid = نمایش شبکه ترازها
viewport-bar-show-wireframes = نمایش قاب‌های سیمی
viewport-bar-show-xy-grid = نمایش شبکه XY
viewport-bar-vertical-slice-view = نمای برش عمودی
viewport-blank = (خالی)
viewport-choose-active-block-model-variable = متغیر فعال مدل بلوکی را انتخاب کنید
viewport-choose-variable = یک متغیر انتخاب کنید
viewport-click-edit-color-right-click = برای ویرایش رنگ کلیک کنید؛ برای حذف کلیک راست کنید
viewport-click-type-boundary-s-value = برای تایپ کردن ارزش این مرز کلیک کنید
viewport-colour-mapping = نقشه برداری رنگ
viewport-count-categories = { $count } دسته
viewport-count-category = { $count } دسته
viewport-double-click-add-boundary-here = برای اضافه کردن یک مرز اینجا دو بار کلیک کنید
viewport-drag-move-middle-click-toggles = برای جابه‌جایی بکشید · کلیک میانی ≤ را تغییر می‌دهد
viewport-drag-move-right-click-remove = برای جابه‌جایی بکشید · برای حذف راست‌کلیک کنید · کلیک میانی ≤ را تغییر می‌دهد
viewport-e = خ
viewport-edit-category-colour = ویرایش رنگ این دسته
viewport-edit-colour-used-empty-values = ویرایش رنگ مورد استفاده برای مقادیر خالی
viewport-empty = (خالی)
viewport-empty-hidden = (خالی · پنهان)
viewport-filter-variables = متغیرهای فیلتر
viewport-navigation-hint = کشیدن با دکمه میانی برای پن · چرخاندن برای بزرگ‌نمایی
viewport-navigation-hint-detach = کشیدن با دکمه میانی برای پن · چرخاندن برای بزرگ‌نمایی · کلیک برای جدا کردن
viewport-n = ش
viewport-no-data-variable = داده‌ای برای این متغیر وجود ندارد
viewport-no-matches = هیچ نتیجه‌ای یافت نشد
viewport-no-usable-range = (دامنه قابل استفاده‌ای وجود ندارد)
viewport-rebuild-variable-s-colours-from = بازسازی رنگ‌های این متغیر از داده‌های آن
viewport-reset = بازنشانی
viewport-restore-full-model-range = بازیابی دامنه کامل مدل
