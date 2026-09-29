# العربية. العبارات غير المتوفرة تستخدم الكتالوج الإنجليزي كبديل.
common-cancel = إلغاء
common-clear = مسح
common-close = إغلاق
common-fill = التعبئة
common-set = تعيين
status-language = اللغة
menu-file = ملف
menu-file-save-project = حفظ المشروع
menu-file-save-project-as = حفظ المشروع باسم...
menu-file-new-project = مشروع جديد...
menu-file-open-project = فتح مشروع...
menu-file-open-recent = فتح مشروع حديث
menu-file-import = استيراد...
menu-file-export = تصدير...
menu-file-about = حول { $app }...
menu-file-exit = خروج من التطبيق
menu-view = عرض
ws-production = الإنتاج
ws-drill-and-blast = الحفر والتفجير
ws-geology = الجيولوجيا
ws-planning = التخطيط
dialog-rename-title = إعادة تسمية { $kind }
dialog-rename-field = الاسم الجديد
dialog-rename-field-hint = مطلوب
dialog-rename-submit = إعادة تسمية
dialog-delete-title = حذف { $kind }
dialog-delete-confirm = حذف «{ $name }» من المشروع؟
    لا يمكن التراجع عن هذا الإجراء.
confirm-delete-product =
    حذف المنتج «{ $name }» من اللوحة؟
    لا يمكن التراجع عن هذا الإجراء.
about-read-full-licence = قراءة الرخصة الكاملة ↗
about-source-code = الشيفرة المصدرية
about-website = الموقع الإلكتروني

## Completed canonical messages

menu-file-show-in-explorer = عرض في Explorer
menu-file-show-in-folder = فتح المجلد الحاوي
menu-file-export-viewport-image = تصدير صورة منفذ العرض...
menu-file-export-engineering-drawing = تصدير رسم هندسي...
ws-menubar-design = التصميم
ws-menubar-triangulation = شبكة مثلثية
ws-menubar-raster = بيانات نقطية
ws-menubar-point-cloud = سحابة نقطية
ws-menubar-block-model = نموذج الكتل
ws-menubar-drillholes = ثقوب الحفر
ws-menubar-active-layer = الطبقة:
ws-menubar-design-insert-point = إدراج النقطة
ws-menubar-design-insert-point-at-intersection = عند التقاطع
ws-menubar-design-insert-point-at-elevation = في ارتفاع
ws-menubar-design-move-to = انتقل إلى
ws-menubar-design-create-triangulation = إنشاء شبكة مثلثية
tri-create-title = إنشاء شبكة مثلثية
tri-create-type-label = نوع التثليث
tri-create-type-help = ينشئ السطح المفتوح سطحًا شبيهًا بالتضاريس. وينشئ الجسم الصلب شبكة مغلقة بالكامل ويتطلب مدخلات يمكن أن تكوّن حدودًا محكمة الإغلاق.
tri-create-output-name = اسم الناتج
tri-create-output-name-help = الاسم المخصص للشبكة المثلثية التي سيتم إنشاؤها.
tri-create-output-name-hint = اسم الشبكة المثلثية
tri-create-run = تثليث
tri-selection-selected = تم اختيار { $summary }
tri-type-open-surface = سطح
tri-type-solid-closed = صلب
about-title = عن { $app }
drill-hole-colour-stop = توقف { $index }
properties-restore-defaults-tooltip = إعادة تعيين إعدادات { $heading } إلى الافتراضات الخاصة بها
ui-selected-count = تم اختيار { $count }
ui-selected-objects = تم اختيار { $count } من الكائنات
ui-selected-polylines = تم اختيار { $count } من الخطوط المتعددة المقاطع
ui-invalid-axis-value = أدخل قيمة { $axis } صالحة.
ui-selection-spans = يمتد اختيار { $min } إلى { $max }.
confirm-delete-count = هل أنت متأكد من رغبتك في حذف { $count } من العناصر المحددة؟
confirm-delete-layer = حذف الطبقة «{ $name }» وجميع الأشياء الموجودة عليها؟ لا يمكن التراجع عن هذا الإجراء.
plot-preview-pixels = { $width } × { $height } px عند { $dpi } dpi
tri-estimated-memory = ذروة الذاكرة المقدَّرة ~{ $estimate }. { $detail }
block-grid-summary = الشبكة: { $x } × { $y } × { $z } = كتلة { $count }
status-selected = مختارة: { $count }
status-clip = المقطع القريب / بعيد / Δ: { $near } / { $far } / { $delta } m

## Selection counts

tri-count-polylines =
    { $count ->
        [one] { $count } خط متعدد المقاطع
       *[other] { $count } خطوط متعددة المقاطع
    }
tri-count-strings =
    { $count ->
        [one] { $count } خط
       *[other] { $count } خطوط
    }
tri-count-points =
    { $count ->
        [one] { $count } نقطة
       *[other] { $count } نقاط
    }
tri-count-texts =
    { $count ->
        [one] { $count } عنصر نصي
       *[other] { $count } عناصر نصية
    }
tri-count-objects =
    { $count ->
        [one] { $count } عنصر
       *[other] { $count } عناصر
    }

## Reused existing project translations

## ترجمات الواجهة المكتملة يدويًا
explorer-no-rasters = لا توجد بيانات نقطية
slice-viewport-gestures = سحب بالزر الأوسط: تحريك · سحب بالزر الأيمن: مدار · Shift+عجلة: مشي · W/S: تحريك الشريحة · Q/E: تدوير · Esc: خروج

## تفاصيل بيئة بدء التشغيل

## تشخيص بدء تشغيل التصيير

color-aci = ACI
color-aci-value = ACI { $index }
color-index = الفهرس
color-rgb = RGB
color-opacity = العتامة
color-edit = انقر لتعديل اللون
color-saturation-value = التشبع والسطوع
color-hue = الصبغة
asset-loading = جارٍ تحميل بيانات الأصل
asset-unloading = جارٍ إلغاء تحميل بيانات الأصل
asset-load-failed = تعذّر تحميل بيانات الأصل
asset-unload-failed = تعذّر إلغاء تحميل بيانات الأصل
preferences-title = التفضيلات
context-text-colour = لون النص
context-polylines = الخطوط المتعددة المقاطع
context-points = النقاط
crs-unknown-ellipsoid = نموذج أرضي غير معروف «{ $name }» في تعريف نظام الإحداثيات هذا.
crs-no-ellipsoid = تعريف نظام الإحداثيات هذا لا يحدد النموذج الأرضي المستخدم.
crs-unknown-code = EPSG:{ $code } غير موجود في سجل أنظمة الإحداثيات.
crs-transform-failed = تعذّر تحويل إحداثية ما؛ لم تكن النتيجة موضعًا محدودًا.
crs-no-datum-path = لا يتوفر تحويل منشور بين الإطارين المرجعيين لـ { $from } و{ $to } (مرجعا EPSG { $source } و{ $target }). التحويل رغم ذلك سيكون خاطئًا بمقدار غير معروف، لذا لم يتم تغيير أي شيء.
crs-unknown-datum = لا يمكن تحديد الإطار المرجعي لـ { $from } أو { $to }، وكلاهما يستخدم نموذجًا أرضيًا مختلفًا. التحويل بينهما سيكون خاطئًا بمقدار غير معروف.
ws-survey = المساحة
survey-count-designs = { $count } { $count ->
    [one] تصميم
   *[other] تصاميم
  }
survey-count-meshes = { $count } { $count ->
    [one] شبكة مثلثية
   *[other] شبكات مثلثية
  }
survey-count-models = { $count } { $count ->
    [one] نموذج كتل
   *[other] نماذج كتل
  }
survey-count-clouds = { $count } { $count ->
    [one] سحابة نقطية
   *[other] سحب نقطية
  }
survey-count-holes = { $count } { $count ->
    [one] مجموعة بيانات ثقوب حفر
   *[other] مجموعات بيانات ثقوب حفر
  }
survey-count-rasters = { $count } { $count ->
    [one] بيانات نقطية
   *[other] بيانات نقطية
  }
survey-angle = الدوران حول Z (عكس اتجاه عقارب الساعة)
survey-scale = عامل مقياس XYZ موحّد
survey-invalid-transform = يجب أن تكون نقاط الأصل والزاوية والإحداثيات الناتجة محدودة القيمة.
survey-invalid-scale = يجب أن يكون المقياس رقمًا موجبًا محدودًا وله مقلوب محدود.
survey-empty-selection = اختر عنصرًا مدعومًا واحدًا على الأقل للتحويل.
survey-unavailable = عنصر محدد مفقود أو غير محمّل. حمّله قبل التحويل.
survey-wrong-project = اختر تصاميم من المشروع النشط فقط.
survey-name-required = أدخل اسم نظام الإحداثيات.
survey-working = جارٍ تحويل البيانات المحددة…
survey-completed = تم تحويل { $items } في مكانها. التراجع يعيدها.
survey-failed = فشل التحويل: { $error }
survey-stale = تم تجاهل التحويل لأن المشروع النشط أو بيانات المصدر تغيّرت. اختر بيانات المصدر وحاول مرة أخرى.
survey-coordinates-menu = الإحداثيات
survey-definitions-action = التعريفات…
survey-transform-action = تحويل…
survey-definitions-title = تعريفات الإحداثيات
survey-transform-title = تحويل الإحداثيات
survey-new-system = نظام إحداثيات جديد
survey-new-system-name = نظام الإحداثيات
survey-set-local = تعيين كنظام إحداثيات المنجم
survey-delete-system = حذف نظام الإحداثيات
survey-systems-empty = لا توجد أنظمة إحداثيات
survey-system-name = الاسم
survey-system-origin = النقطة نفسها — إحداثيات النظام
survey-angle-help = عكس اتجاه عقارب الساعة من X المرجعي نحو Y المرجعي، بالنظر من الأعلى.
survey-scale-help = مقياس XYZ موحّد من الإطار المرجعي إلى هذا النظام. استخدم 1 للحفاظ على الأبعاد.
survey-close = إغلاق
survey-from = من
survey-to = إلى
survey-transform-button = تحويل
survey-swap = تبديل
survey-drape-note = تُزال الصور المغطّاة من الأسطح المحوَّلة ويجب إعادة تغطيتها.
survey-needs-grid-block-model = نموذج الكتل هو شبكة منتظمة من الخلايا، وتغيير الإسقاط أو الإطار المرجعي لا يحافظ على هذا الانتظام. تحويله يعني إعادة أخذ عينات كل خلية في شبكة جديدة وفقدان القيم التي تحملها، لذا تُرك دون تغيير.
survey-needs-grid-raster = توضع البيانات النقطية في العالم عبر تحويل تآلفي، وهو ما لا يستطيع تغيير الإسقاط أو الإطار المرجعي الحفاظ عليه. تحويلها يعني إعادة أخذ عينات الصورة، لذا تُركت دون تغيير.
survey-conversion-exact = دقيق: تغيير الشبكة فقط، دون إعادة إسقاط.
survey-conversion-accuracy = الدقة المعلنة { $accuracy } م.
survey-kind = النوع
survey-axis-names = أسماء المحاور
survey-kind-registry-short = نظام من السجل
survey-kind-grid-short = شبكة فوق نظام آخر
survey-registry-search = بحث
survey-registry-hint = الاسم أو رمز EPSG، مثل «mga zone 56»
survey-registry-none = لا شيء في السجل يطابق كل الكلمات.
survey-parent = معرَّف بالنسبة إلى
survey-parent-origin = نقطة معروفة — إحداثيات النظام الأصل
survey-pick-registry = ابحث عن النظام واخترْه من النتائج.
survey-pick-parent = اختر النظام الذي عُرِّفت هذه الشبكة بالنسبة إليه.
survey-pick-system = اختر نظامًا
survey-pick-systems = اختر النظام المصدر والنظام الهدف للتحويل.
survey-no-selection = اختر نظام إحداثيات من اليسار، أو انقر بزر الماوس الأيمن لإضافة واحد.
survey-kind-grid = شبكة فوق { $parent }
survey-system-in-use = لا يمكن حذف «{ $name }»: { $dependants } { $dependants ->
    [one] نظام معرَّف
   *[other] أنظمة معرَّفة
  } بالنسبة إليه. وجّهها إلى مكان آخر أولاً.
survey-system-cycle = «{ $name }» معرَّف بالنسبة إلى نفسه، مباشرة أو عبر أنظمته الأصل.
survey-system-missing = لم يعد نظام الإحداثيات هذا موجودًا. اختر تعريفًا آخر.
survey-same-system = اختر نظامي مصدر وهدف مختلفين.
survey-name-exists = يوجد بالفعل نظام إحداثيات بهذا الاسم. اخترْه للتعديل، أو اختر اسمًا آخر.

## About strings

about-copyright-c-2026-leo-timmins =
    Copyright (c) 2026 Leo Timmins, Lucas Timmins, and Incline Design contributors. Permission is hereby granted, free of charge, to any person obtaining a copy of this software to deal in it without restriction, subject to the conditions of the MIT License.

    Incline Design is provided "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, including but not limited to the warranties of MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE and NONINFRINGEMENT.
about-free-open-source-mine-design = تصميم مناجم حر ومفتوح المصدر
about-licensed-under-mit-license = مرخص بموجب رخصة MIT

## App strings

app-activated-browser-project-name = تم تنشيط مشروع المتصفح «{ $name }».
app-browser-project-delete-failed = فشل حذف مشروع المتصفح: { $error }
app-browser-project-no-longer-exists = لم يعد مشروع المتصفح هذا موجودًا
app-browser-save-failed-error = فشل الحفظ في المتصفح: { $error }
app-could-not-activate-browser-project = تعذر تنشيط مشروع المتصفح: { $error }
app-could-not-delete-browser-project = تعذر حذف مشروع المتصفح: { $error }
app-could-not-load-browser-project = تعذر تحميل مشروع المتصفح: { $error }
app-could-not-restore-browser-project = تعذر استعادة مشروع المتصفح: { $error }
app-deleted-browser-project = تم حذف مشروع المتصفح
app-failed-create-window-error = تعذر إنشاء النافذة: { $error }
app-failed-create-window-icon-error = تعذر إنشاء أيقونة النافذة: { $error }
app-failed-detach-top-down-preview = تعذر فصل العرض العلوي: { $error }
app-failed-initialize-graphics-error = تعذرت تهيئة الرسومات: { $error }
app-browser-preferences-load-failed = تعذر تحميل تفضيلات المتصفح: { $error }
app-failed-load-config-file-error = تعذر تحميل ملف الإعداد: { $error }
app-failed-load-session-file-error = تعذر تحميل ملف الجلسة: { $error }
app-failed-rasterize-window-icon-error = تعذر تحويل أيقونة النافذة إلى نقطية: { $error }
app-failed-save-browser-session-error = تعذر حفظ جلسة المتصفح: { $error }
app-failed-save-session-error = تعذر حفظ الجلسة: { $error }
app-saved-name-browser-storage = تم حفظ «{ $name }» في تخزين المتصفح

## Block strings

block-model-between = بين
block-model-block-grid = شبكة الكتل
block-model-block-size = حجم الكتلة
block-model-choose-numeric-variable = اختر متغيرًا رقميًا
block-model-choose-numeric-variables = اختر المتغيرات الرقمية
block-model-count-variables-selected = تم تحديد { $count } من المتغيرات
block-model-estimate-variables = متغيرات التقدير
block-model-full-x-y-z-dimensions = أبعاد X وY وZ الكاملة لكل كتلة. تزيد الكتل الأصغر التفاصيل ووقت الحساب واستخدام الذاكرة.
block-model-grid-bounds-block-sizes-invalid = حدود الشبكة أو أحجام الكتل غير صالحة.
block-model-lower-x-y-z-edges = حدود X وY وZ السفلية لحجم نموذج الكتل. تبدأ مراكز الكتل بمقدار نصف كتلة داخل هذه الحدود.
block-model-maximum = الحد الأقصى
block-model-maximum-nearest-samples-used-each = الحد الأقصى لأقرب العينات لكل كتلة. القيم الأقل أسرع، والأعلى قد تنعّم التقدير وتزيد وقت الحساب.
block-model-maximum-samples = أقصى عدد من العينات
block-model-minimum = الحد الأدنى
block-model-min-samples-help = الحد الأدنى للعينات القريبة لتقدير كتلة. تظل الكتل ذات العينات الأقل داخل نصف قطر البحث فارغة.
block-model-minimum-samples = الحد الأدنى من العينات
block-model-nugget = تأثير الكتلة الصغرية
block-model-numeric-interval-fields-interpolate = حقول الفواصل الرقمية المراد استيفاؤها. يصبح كل حقل محدد متغيرًا في نموذج الكتل.
block-model-kriging-help = يقدّر كريغنغ العادي فواصل ثقوب الحفر الرقمية في مركز كل كتلة باستخدام مخطط تغاير كروي.
block-model-partial-sill = العتبة الجزئية
block-model-range-search-radius = المدى / نصف قطر البحث
block-model-range-help = يتم استبعاد العينات التي تتجاوز هذه المسافة، وتصل التباين إلى الصفر في هذه النطاق.
block-model-select-all = اختر كل
block-model-sill-help = التباين المترابط مكانيًا من النموذج الكروي. يحدد مع تأثير الكتلة التغاير عند مسافة صفر.
block-model-spherical-variogram-search = الفاريوغرام الكروي والبحث
block-model-threshold-at-most = <= العتبة
block-model-threshold-at-least = >= العتبة
block-model-threshold-min = العتبة / الحد الأدنى
block-model-upper-x-y-z-extent = نطاق X وY وZ العلوي المراد تغطيته. قد تمتد الكتلة الأخيرة بعد هذا النطاق عندما لا يكون الامتداد مضاعفًا دقيقًا لحجم الكتلة.
block-model-variable = المتغير
block-model-variance-effectively-zero-separation = التباين عند فصل شبه صفري الناتج عن خطأ القياس أو تغير أدنى من مقياس العينة. استخدم صفرًا عند عدم الحاجة إلى تأثير الكتلة.
block-model-volume-feedback-disconnected = انقطعت قراءة ملاحظات استخدام حجم الكتل
block-model-volume-feedback-failed = فشلت قراءة ملاحظات استخدام حجم الكتل: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z

## Canvas strings

canvas-not-selectable-closed-polyline = غير قابل للتحديد | اختر خطًا متعددًا مغلقًا
canvas-polyline-summary = خط متعدد | الطبقة: { $layer } | الرؤوس: { $count }
canvas-surface-name = السطح | { $name }
canvas-trimmed = مشذّب

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = أُنشئ منحدر ومصطبة من الكائن { $object_id }
cmd-bezier-replaced-polyline-span-first-last = استُبدل امتداد الخط المتعدد { $first }→{ $last } بعدد { $count } نقطة وسيطة مأخوذة بالعينة
cmd-bezier-vertices-first-last = الرؤوس من { $first } إلى { $last }
cmd-block-model-block-model-loader-disconnected-path = انقطع اتصال محمّل نموذج الكتل للمسار { $path }
cmd-block-model-block-model-path-has-count = يحتوي نموذج الكتل { $path } على { $count } متغيرات من نوع غير مدعوم يتعذر قراءتها: { $names }
cmd-block-model-building-ore-mesh = جارٍ إنشاء شبكة الخام…
cmd-block-model-could-not-create-block-model = تعذر إنشاء نموذج الكتل: { $error }
cmd-block-model-could-not-decode-block-model = تعذر فك ترميز متغير لون نموذج الكتل «{ $variable }»: { $error }
cmd-block-model-created-block-model-name-ordinary = تم إنشاء نموذج الكتل «{ $name }» بطريقة كريغنغ العادية
cmd-block-model-failed-load-block-model-error = تعذر تحميل نموذج الكتل: { $error }
cmd-block-model-generated-ore-mesh-from-block = تم إنشاء شبكة خام من نموذج الكتل «{ $name }»
cmd-block-model-imported-block-model-source-path = تم استيراد مصدر نموذج الكتل { $path }
cmd-block-model-loaded-block-model-name-blocks = تم تحميل نموذج الكتل «{ $name }»: ‏{ $blocks } كتلة ({ $renderable } قابلة للرسم)، شبكة { $dimx }×{ $dimy }×{ $dimz }، ‏{ $variables } متغيرات
cmd-block-model-loading-name = جارٍ تحميل { $name }
cmd-block-model-loading-name-ellipsis = جارٍ تحميل { $name }…
cmd-chamfer-applied = شُطفت الزاوية { $corner } بنصف قطر { $radius } و{ $segments } مقطع
cmd-chamfer-radius = نصف القطر { $radius }
cmd-commands-clipped = مقصوص
cmd-commands-command-failed-error = فشل الأمر: { $error }
cmd-commands-select-one-more-objects-before = حدد كائنًا واحدًا أو أكثر قبل تعيين { $axis }
cmd-commands-sliced = مقطّع
cmd-contours-contour-generation-failed-error = فشل إنشاء خطوط الكنتور: { $error }
cmd-contours-discarded-layer-exists = تم تجاهل خطوط كنتور «{ $name }»: الطبقة «{ $layer_name }» موجودة بالفعل
cmd-contours-discarded-project-closed = تم تجاهل خطوط كنتور «{ $name }»: أُغلق المشروع
cmd-contours-discarded-layer-deleted = تم تجاهل خطوط كنتور «{ $name }»: حُذفت طبقة الإخراج المحددة
cmd-contours-generated = تم إنشاء { $line_count } خط كنتور متعدد للتثليث «{ $name }» في الطبقة «{ $layer_name }»
cmd-creation-assembled-boundary-rings = جُمعت { $assembled_count } حلقة حدود مغلقة من خطوط مفتوحة مجزأة
cmd-creation-created-triangulation-from-boundary = أُنشئ تثليث من { $boundary_count } حلقة حدود و{ $constraint_count } قيد مفتوح، نوع السطح { $surface_type }
cmd-creation-creating-triangulation = جارٍ إنشاء التثليث…
cmd-creation-generate-upper-surface-ignored-count = إنشاء السطح العلوي: تم تجاهل { $count } مقطع خط كسر سفلي متعارض؛ لم تتغير كائنات المصدر
cmd-creation-ignored-objects = تم تجاهل { $rejected } كائن غير متعدد الخطوط أو متدهور أثناء التثليث
cmd-creation-weld-retry-moved-coarse-welded = اللحام وإعادة المحاولة: نُقل { $coarse_welded } رأس إلى مواضع مشتركة (حتى { $coarse_weld_tol } م)؛ لم تتغير كائنات المصدر
cmd-creation-welded-breakline-vertices = تم لحام { $welded } من رؤوس خطوط الانكسار المتطابقة ضمن حد السماح
cmd-cuts-clipped-surface-name-polyline-mode = قُص السطح «{ $name }» بواسطة خط متعدد ({ $mode })
cmd-cuts-clipping-surface-polyline = جارٍ قص السطح بواسطة خط متعدد…
cmd-cuts-cut-topology-name-pit-shell = قُطع السطح الطبوغرافي «{ $name }» حسب غلاف الحفرة
cmd-cuts-cut-triangulation-name-z-band = قُطع التثليث «{ $name }» حسب نطاق Z ‏[{ $min }، { $max }]
cmd-cuts-cutting-topology-pit-shell = جارٍ قطع السطح الطبوغرافي حسب غلاف الحفرة…
cmd-cuts-cutting-triangulation-z = جارٍ قطع التثليث حسب Z…
cmd-cuts-ignored-vertical-faces = تم تجاهل { $count } وجه رأسي أو متدهور في طوبولوجيا المرجع بلا مساحة XY
cmd-cuts-site-skipped-constraint-from-x = { $site }: تم تخطي القيد ({ $from_x }، { $from_y }) ← ({ $to_x }، { $to_y }) الذي تعذر على المُثلِّث تقسيمه
cmd-cuts-skipped-degenerate-edges = { $site }: تم تخطي { $skipped } حافة قيد شبه متدهورة؛ قد ينحرف حد القطع قليلًا بالقرب منها
cmd-cuts-trimmed-surface = شُذّب السطح «{ $surface }» وفق السطح الطبوغرافي «{ $topology }» ({ $mode })
cmd-cuts-trimming-surface-topology = جارٍ تشذيب السطح وفق السطح الطبوغرافي…
cmd-drape-draped-intersected-vertices-changed = أُسقط { $intersected } رأس؛ تغيّر ارتفاع { $changed } منها
cmd-drape-no-intersections = لا تتقاطع أي من رؤوس التصميم المحددة مع الأسطح الطبوغرافية المحددة
cmd-drape-objects-changed-object-s-changed = تم تغيير { $objects } كائن · نُقل { $changed } من أصل { $intersected } رأس متقاطع
cmd-drape-select-one-more-design-objects = حدد كائن تصميم واحدًا أو أكثر لإسقاطه على السطح
cmd-drape-select-one-more-topologies-drape = حدد سطحًا طبوغرافيًا واحدًا أو أكثر للإسقاط عليه
cmd-drape-selected-topologies-no-longer-loaded = لم تعد الأسطح الطبوغرافية المحددة محمّلة
cmd-drill-hole-drill-pattern-too-large-contains = نمط الحفر كبير جدًا أو يحتوي إحداثيات طوق غير صالحة
cmd-drill-hole-enter-name-drill-pattern = أدخل اسمًا لنمط الحفر
cmd-drill-hole-failed-load-drillholes-error = تعذر تحميل ثقوب الحفر: { $error }
cmd-drill-hole-depth-must-be-positive = يجب أن يكون عمق الحفرة أكبر من صفر
cmd-drill-hole-diameter-must-be-positive = يجب أن يكون قطر الحفرة أكبر من صفر
cmd-drill-hole-loaded-drillhole-dataset-name-holes = تم تحميل مجموعة بيانات الآبار «{ $name }»: ‏{ $holes } بئرًا و{ $fields } حقل لون
cmd-drill-hole-pattern-contains-no-holes = لا يحتوي النمط على ثقوب
cmd-explode-count-line-s = { $count } خط
cmd-explode-polyline = تفكيك الخط المتعدد
cmd-explode-exploded-polyline-into-count-line = فُكِّك الخط المتعدد إلى { $count } قطعة مستقيمة
cmd-file-block-model-csv-encoding-failed = فشل ترميز CSV لنموذج الكتل: { $error }
cmd-file-block-model-csv-export-failed = فشل تصدير CSV لنموذج الكتل: { $error }
cmd-file-browser-recovery-unavailable = ملفات استرداد المتصفح غير متاحة؛ تبقى المشاريع المحفوظة في IndexedDB
cmd-file-closed-project-runtime-id-runtime = تم إغلاق المشروع ذي معرّف وقت التشغيل { $runtime_id }
cmd-file-could-not-create-new-project = تعذر إنشاء مشروع جديد: { $error }
cmd-file-could-not-finish-pending-project = تعذر إكمال إجراء المشروع المعلّق: { $error }
cmd-file-could-not-finish-saving-before = تعذر إكمال الحفظ قبل الخروج: { $error }
cmd-file-could-not-open-browser-project = تعذر فتح مشروع المتصفح: { $error }
cmd-file-could-not-open-path-error = تعذر فتح { $path }: { $error }
cmd-file-could-not-read-selected-file = تعذرت قراءة الملف المحدد: { $error }
cmd-file-could-not-reload-layer-from = تعذرت إعادة تحميل الطبقة من القرص: { $error }
cmd-file-could-not-reload-project-from = تعذرت إعادة تحميل المشروع من القرص: { $error }
cmd-file-could-not-remove-browser-project = تعذرت إزالة مشروع المتصفح: { $error }
cmd-file-could-not-restore-layer-from = تعذرت استعادة الطبقة من المشروع: { $error }
cmd-file-could-not-snapshot-dirty-project = تعذر إنشاء لقطة للمشروع المعدّل بغرض الاسترداد: { $error }
cmd-file-could-not-start-browser-export = تعذر بدء التصدير في المتصفح: { $error }
cmd-file-could-not-write-recovery-copies = تعذرت كتابة نسخ الاسترداد: { $error }
cmd-file-created-new-browser-project = تم إنشاء مشروع متصفح جديد
cmd-file-created-new-project = تم إنشاء مشروع جديد
cmd-file-description-download-failed-error = فشل تنزيل { $description }: { $error }
cmd-file-discard-cancelled-project-changed = تم إلغاء تجاهل التغييرات لأن المشروع تغيّر أثناء إعادة تحميل OMF
cmd-file-discarded-changes-layer-target-name = تم تجاهل التغييرات على الطبقة «{ $target_name }»
cmd-file-discarded-changes-reloaded-path = تم تجاهل التغييرات: أُعيد تحميل { $path }
cmd-file-downloaded-description-file-name = تم تنزيل { $description }: { $file_name }
cmd-file-dxf-download-encoding-failed-error = فشل ترميز تنزيل DXF: { $error }
cmd-file-dxf-import-failed-error = فشل استيراد DXF: { $error }
cmd-file-encoding-block-model-csv-download = جارٍ ترميز تنزيل CSV لنموذج الكتل…
cmd-file-encoding-dxf-download = جارٍ ترميز تنزيل DXF…
cmd-file-encoding-triangulation-download = جارٍ ترميز تنزيل التثليث…
cmd-file-exit-deferred-exports = تم تأجيل الخروج حتى انتهاء عمليات التصدير في الخلفية
cmd-file-exit-requested-no-unsaved-changes = تم طلب الخروج ولا توجد تغييرات غير محفوظة
cmd-file-exported-block-model-csv-path = تم تصدير CSV لنموذج الكتل إلى { $path }
cmd-file-exported-description-dxf-path = تم تصدير { $description } إلى DXF: { $path }
cmd-file-exported-triangulation-name-path = تم تصدير التثليث «{ $name }» إلى { $path }
cmd-file-exporting-name = جارٍ تصدير { $name }…
cmd-file-exporting-triangulation-name-path = جارٍ تصدير التثليث «{ $name }» إلى { $path }
cmd-file-fatal-renderer-failure-reason = عطل فادح في العارض: { $reason }
cmd-file-dialog-action-failed = فشل إجراء مربع حوار الملفات: { $msg }
cmd-file-imported-added-object-s-from = تم استيراد { $added } كائنًا من { $name }
cmd-file-imported-total-dxf-object-s = تم استيراد { $total } من كائنات DXF
cmd-file-layer-discard-was-cancelled-because = تم إلغاء تجاهل تغييرات الطبقة لأن المشروع تغيّر أثناء إعادة تحميله
cmd-file-no-recovery-directory = لا يوجد دليل استرداد متاح: { $error }
cmd-file-no-unsaved-project-content-nothing = لا يوجد محتوى مشروع غير محفوظ؛ لا شيء لاسترداده
cmd-file-parsing-browser-dxf-import = جارٍ تحليل استيراد DXF في المتصفح…
cmd-file-parsing-dxf-import = جارٍ تحليل استيراد DXF…
cmd-file-project-closes-after-save = سيُغلق المشروع بعد انتهاء عملية الحفظ الحالية
cmd-file-the-project-closes-after-save = سيُغلق المشروع بعد انتهاء عملية الحفظ الحالية
cmd-file-queued-count-triangulation-file-s = تمت إضافة { $count } من ملفات التثليث إلى قائمة انتظار الاستيراد
cmd-file-recovery-copies-path-reopen-them = نسخ الاسترداد موجودة في { $path }؛ أعد فتحها بعد إعادة التشغيل
cmd-file-recovery-copy-failed-error = فشلت نسخة الاسترداد: { $error }
cmd-file-recovery-copy-failed-failure = فشل إنشاء نسخة الاسترداد: { $failure }
cmd-file-recovery-copy-written-path = تمت كتابة نسخة الاسترداد: { $path }
cmd-file-reverting-layer = جارٍ التراجع عن تغييرات الطبقة…
cmd-file-reverting-project = جارٍ التراجع عن تغييرات المشروع…
cmd-file-save-failed-message = فشل الحفظ: { $message }
cmd-file-save-worker-ended-without-result = انتهى عامل الحفظ دون نتيجة
cmd-file-saved-project-as = تم حفظ المشروع باسم: { $path }
cmd-file-saved-project = تم حفظ المشروع: { $path }
cmd-file-selected-block-model-no-longer = لم يعد نموذج الكتل المحدد محمّلًا
cmd-file-switching-project = جارٍ تبديل المشروع…
cmd-file-triangulation-download-encoding-failed = فشل ترميز تنزيل التثليث: { $error }
cmd-file-user-chose-exit-without-saving = اختار المستخدم الخروج دون حفظ
cmd-file-user-requested-exit-project-export = طلب المستخدم الخروج (يلزم تأكيد تصدير المشروع أو العمل غير المحفوظ)
cmd-file-viewport = منفذ العرض
cmd-file-wait-current-project-save-finish = انتظر حتى ينتهي حفظ المشروع الحالي
cmd-file-wait-current-project-switch-finish = انتظر حتى ينتهي تبديل المشروع الحالي
cmd-file-wait-project-operation-finish-before = انتظر انتهاء عملية المشروع قبل تجاهل التغييرات
cmd-file-wait-project-revert-finish-before = انتظر حتى انتهاء استعادة المشروع قبل الحفظ
cmd-fuse-closed-polyline = خط متعدد مغلق
cmd-fuse-count-source-line-s = { $count } خط مصدر
cmd-fuse-created-shape-object-id-vertices = أُنشئ { $shape } { $object_id } بعدد { $vertices } رأس من { $sources } خط مصدر
cmd-fuse-click-missed = الدمج: لم تصب النقرة أي كائن (لا شيء تحت المؤشر)
cmd-fuse-click-not-near-endpoint = الدمج: لم تكن النقرة قريبة بما يكفي من أي من طرفي الخط المحدد
cmd-fuse-clicked-closed-polyline = الدمج: الكائن { $object_id } خط متعدد مغلق؛ يعمل الدمج على الخطوط المتعددة المفتوحة فقط
cmd-fuse-clicked-not-open-polyline = الدمج: الكائن { $object_id } ليس خطًا متعددًا مفتوحًا (نوعه { $kind })
cmd-fuse-clicked-object-missing = الدمج: لم يعد الكائن { $object_id } الذي تم النقر عليه موجودًا
cmd-fuse-clicked-too-few-vertices = الدمج: يحتوي الخط المتعدد { $object_id } على { $count } رأس فقط؛ يلزم رأسان على الأقل
cmd-fuse-endpoint-marker-missing = الدمج: لم تعد علامة نقطة النهاية { $marker_index } موجودة
cmd-fuse-close-needs-three-vertices = الدمج: يحتاج الخط إلى 3 رؤوس مميزة على الأقل لإغلاقه كخط متعدد (لديه { $count })
cmd-fuse-lines = دمج الخطوط
cmd-fuse-needs-two-segments = الدمج: يلزم مقطعان على الأقل للتطبيق (المتوفر { $count })
cmd-fuse-no-active-layer = الدمج: لا توجد طبقة نشطة لوضع الخط المدمج عليها
cmd-fuse-no-active-project = الدمج: لا يوجد مشروع نشط، يتعذر التطبيق
cmd-fuse-no-source-line = الدمج: لا يوجد خط مصدر لإغلاقه وتحويله إلى خط متعدد
cmd-fuse-awaiting-object-invalid = الدمج: لم يعد الكائن { $awaiting_id } خطًا متعددًا صالحًا
cmd-fuse-object-already-in-chain = الدمج: الكائن { $object_id } جزء من سلسلة الدمج بالفعل؛ اختر خطًا آخر
cmd-fuse-result-too-few-vertices = الدمج: يحتوي الناتج على عدد قليل جدًا من الرؤوس ({ $count })، جارٍ الإلغاء
cmd-fuse-segment-object-invalid = الدمج: لم يعد كائن المقطع { $object_id } خطًا متعددًا صالحًا؛ أُلغيت العملية
cmd-fuse-source-object-invalid = الدمج: لم يعد كائن المصدر { $object_id } خطًا متعددًا مفتوحًا صالحًا
cmd-fuse-source-object-missing = الدمج: لم يعد كائن المصدر { $object_id } موجودًا
cmd-fuse-open-polyline = خط متعدد مفتوح
cmd-include-failed = فشل التضمين: { $message }
cmd-include-included-solid-shape-name-topology = أُدرج الجسم «{ $shape_name }» في الطوبولوجيا «{ $topology_name }» (تم الاحتفاظ بـ{ $retained } وجه وتخطي { $skipped } وجه إغلاق)
cmd-include-including-pit-stockpile-solid = جارٍ تضمين مجسّم الحفرة/المخزون…
cmd-insert-point-count-operation-point-s = { $count } نقطة { $operation }
cmd-insert-point-elevation-must-be-finite = يتطلب إدراج نقطة عند منسوب قيمة منسوب محدودة
cmd-insert-point-insert-points = إدراج نقاط
cmd-insert-point-inserted-count-operation-point-s = تم إدراج { $count } نقطة { $operation }
cmd-insert-point-intersection = تقاطع
cmd-insert-point-no-new-operation-points-were = لم يتم العثور على نقاط { $operation } جديدة
cmd-insert-point-select-least-two-polylines-before = حدد خطين متعددين على الأقل قبل إدراج نقاط التقاطع
cmd-insert-point-select-one-more-polylines-before = حدد خطًا متعددًا واحدًا أو أكثر قبل إدراج نقطة عند منسوب
cmd-layer-created-layer-name = تم إنشاء الطبقة «{ $name }»
cmd-layer-deleted-with-objects = تم حذف الطبقة { $layer_id } (وجميع الكائنات عليها)
cmd-layer-duplicated-layer-duplicate-name = تم تكرار الطبقة «{ $duplicate_name }»
cmd-layer-locked = مقفل
cmd-layer-name-copy = نسخة من { $name }
cmd-layer-selected-count-object-s-layer = تم تحديد { $count } كائن في الطبقة { $layer_id }
cmd-layer-state-layer-name = الطبقة «{ $name }» { $state }
cmd-layer-unlocked = غير مقفل
cmd-move-tool-moved-collars = طُبّق الإزاحة ({ $delta }) على { $count } طوق حفرة
cmd-move-tool-moved-objects = طُبق مقدار الحركة ({ $delta }) على { $count } كائن
cmd-move-tool-count-hole-s = { $count } حفرة
cmd-object-edit-edited-kind = تم تعديل { $kind }
cmd-object-edit-edited-kind-count-vertices = تم تعديل { $kind } ({ $count } رؤوس)
cmd-object-edit-no-changes-apply = لا توجد تغييرات لتطبيقها
cmd-object-edit-object-changed-since-editor-opened = تغيّر هذا الكائن منذ فتح المحرر؛ أعد فتحه لتعديل النسخة الحالية
cmd-object-edit-target-changed = تغيّر الكائن قيد التعديل؛ سيتم تجاهل التعديل
cmd-object-edit-object-no-longer-exists-document = هذا الكائن لم يعد موجودًا في المستند
cmd-object-edit-select-single-design-object-edit = اختر كائن تصميم واحدًا للتعديل
cmd-object-edit-unassigned = غير معيّن
cmd-offset-create-offset = إنشاء إزاحة
cmd-offset-created-offset-count-object-s = أُنشئت إزاحة لـ { $count } كائن
cmd-offset-distance-must-be-positive = يجب أن تكون مسافة الإزاحة أكبر من صفر
cmd-omf-could-not-open-project-source = تعذر فتح المشروع { $source_name }: ‏{ $error }
cmd-omf-create-open-project-before-merging = أنشئ مشروعًا أو افتحه قبل دمج البيانات
cmd-omf-encoding-project = جارٍ ترميز المشروع…
cmd-omf-exported-project-path = تم تصدير المشروع إلى { $path }
cmd-omf-imported-project = تم استيراد المشروع «{ $project_name }» من { $source_name }: ‏{ $count } مجموعة بيانات عليا
cmd-omf-importing-project = جارٍ استيراد المشروع…
cmd-omf-export-failed = فشل تصدير OMF: { $error }
cmd-omf-import-failed = فشل استيراد OMF: { $error }
cmd-omf-opened-project = تم فتح المشروع «{ $project_name }» من { $source_name }
cmd-omf-project-source-name-contains-no = لا يحتوي المشروع «{ $source_name }» على عناصر بيانات مدعومة
cmd-omf-source-name-applied-project-origin = { $source_name }: طُبّق أصل المشروع { $origin } قبل الدمج
cmd-omf-crs-differs = { $source_name }: يختلف نظام الإسناد «{ $source_crs }» عن نظام المشروع «{ $target_crs }»؛ دُمجت الإحداثيات بلا إعادة إسقاط
cmd-omf-source-name-units-source-units = { $source_name }: تختلف الوحدات «{ $source_units }» عن وحدات المشروع «{ $target_units }»؛ دُمجت الإحداثيات بلا تحويل
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = لا توجد بيانات Incline Design مفتوحة لتصديرها
cmd-placement-2-vertices = رأسان
cmd-placement-count-vertices = { $count } رأس
cmd-placement-created-circle = أُنشئت دائرة بنصف قطر { $radius } م
cmd-placement-created-closed-polyline = أُنشئ خط متعدد مغلق بعدد { $count } رأس
cmd-placement-created-line-segment-2-vertices = تم إنشاء قطعة مستقيمة برأسين
cmd-placement-created-open-polyline-count-vertices = أُنشئ خط متعدد مفتوح بعدد { $count } رأس
cmd-placement-placed-point-x-y-z = وُضعت النقطة عند { $x }، { $y }، { $z }
cmd-placement-radius = نصف القطر { $radius } م
cmd-plot-composing-engineering-drawing = جارٍ تركيب الرسم الهندسي…
cmd-plot-could-not-write-engineering-drawing = تعذرت كتابة الرسم الهندسي: { $error }
cmd-plot-drawing-scale-fitted-visible-data = ضُبط مقياس الرسم ليلائم البيانات المرئية: 1:{ $scale }
cmd-plot = مخطط
cmd-plot-saved-drawing = تم حفظ الرسم الهندسي: { $description } ‏({ $width } × { $height } بكسل عند { $dpi } نقطة/بوصة)
cmd-point-cloud-failed-load-point-cloud-error = تعذر تحميل سحابة النقاط: { $error }
cmd-point-cloud-loaded-point-cloud-name-count = تم تحميل سحابة النقاط { $name } ({ $count } نقطة)
cmd-point-cloud-point-cloud-loader-disconnected-path = انقطع اتصال محمّل سحابة النقاط للمسار { $path }
cmd-point-cloud-tin-max-edge-disabled = (الحد الأقصى للحافة معطّل)
cmd-point-cloud-tin-max-edge-max-edge = (الحد الأقصى للحافة { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = فشل إنشاء TIN لسحابة النقاط: { $error }
cmd-point-cloud-tin-subsampled = TIN للتضاريس: تم أخذ عينة مكانية من { $sampled } نقطة من أصل { $total }
cmd-point-cloud-tin-triangulated = TIN التضاريس: ثُلثت { $vertex_count } نقطة XY فريدة إلى { $face_count } وجه{ $suffix }
cmd-products-added-product-delay-ms-ms = تمت إضافة المنتج { $delay_ms } مللي ثانية { $name }
cmd-products-deleted-product-delay-ms-ms = تم حذف المنتج { $delay_ms } مللي ثانية { $name }
cmd-products-failed-save-products-error = تعذر حفظ المنتجات: { $error }
cmd-products-product-no-longer-palette = لم يعد هذا المنتج موجودًا في لوحة الألوان
cmd-property-action-count-object-s-layer = { $action } { $count } كائن إلى الطبقة { $layer }
cmd-property-batch-set-axis-value-count = تعيين جماعي لقيمة { $axis } على { $count } كائن
cmd-property-batch-set-closed-count-polyline = تعيين جماعي للإغلاق على { $count } خط متعدد
cmd-property-batch-set-color-count-object = تعيين جماعي للون على { $count } كائن
cmd-property-batch-set-fill-style-count = تعيين جماعي لنمط التعبئة على { $count } كائن
cmd-property-batch-set-line-weight-count = تعيين جماعي لوزن الخط على { $count } خط متعدد
cmd-property-copied = تم النسخ
cmd-property-moved = تم النقل
cmd-raster-draped = أُسقطت البيانات النقطية { $raster } على التثليث { $triangulation } (نطاقات متداخلة)
cmd-raster-failed-load-raster-name-error = تعذر تحميل الصورة النقطية { $name }: { $error }
cmd-raster-failed-load-raster-path-error = تعذر تحميل الصورة النقطية { $path }: { $error }
cmd-raster-loaded-raster-name-via-driver = تم تحميل البيانات النقطية { $name } عبر { $driver } ‏({ $srcx }×{ $srcy }، معاينة { $prevx }×{ $prevy })
cmd-raster-no-overlapping-triangulation = لا يتداخل أي تثليث محمّل مع نطاق { $name }
cmd-raster-loader-disconnected = انقطع اتصال محمّل الصورة النقطية للمسار { $path }
cmd-raster-undraped = تمت إزالة إسقاط الصور النقطية من { $count } من التثليثات
cmd-relimit-click-missed = إعادة تحديد الحدود: لم تصب النقرة أي كائن (لا شيء تحت المؤشر)
cmd-relimit-click-ignored = إعادة تحديد الحدود: تم تجاهل النقرة، فالأداة لا تنتظر حاليًا اختيار هدف
cmd-relimit-clicked-source-line = إعادة تحديد الحدود: تم النقر على خط المصدر نفسه، اختر خطًا آخر
cmd-relimit-no-source-line = إعادة تحديد الحدود: لم يتم تعيين خط مصدر، جارٍ إلغاء الاختيار
cmd-relimit-relimited-line-source-id-selected = أُعيد تحديد الخط { $source_id } إلى الهدف المحدد
cmd-relimit-resized-line-source-id-using = غُيّر حجم الخط { $source_id } باستخدام الوضع { $mode } والقيمة { $value }
cmd-rename-item-no-longer-belongs-active = لم يعد هذا العنصر تابعًا للمشروع النشط
cmd-rename-renamed-before-name = تمت إعادة تسمية «{ $before }» إلى «{ $name }»
cmd-rename-renamed-name-taken = أُعيدت تسمية «{ $before }» إلى «{ $name }» («{ $requested }» مستخدم بالفعل)
cmd-rotate-collar-turned-count-drillhole-collar-s = دُوّر { $count } طوق حفرة { $rotation }
cmd-section-verb-count-item-s-section = { $verb } { $count } عنصر في { $section }
cmd-selection-delete-vertex = حذف الرأس
cmd-selection-deleted-count-selected-object-s = تم حذف { $count } كائن محدد
cmd-selection-deleted-vertex = تم حذف الرأس { $vertex } من الخط المتعدد { $object_id }
cmd-selection-duplicate-selection = تكرار التحديد
cmd-selection-duplicated-count-object-s = تم تكرار { $count } كائن
cmd-session-created-triangulation = أُنشئ التثليث «{ $name }» ‏({ $vertex_count } رأس، ‏{ $face_count } وجه) من سطح نوعه { $surface_type }
cmd-session-deleted-triangulation = تم حذف التثليث «{ $name }» من المشروع
cmd-session-failed-load-triangulation-error = تعذر تحميل التثليث: { $error }
cmd-session-failed-load-triangulation-message = تعذر تحميل التثليث: { $message }
cmd-session-loaded-triangulation = تم تحميل التثليث «{ $name }» ‏({ $path }، ‏{ $vertex_count } رأس، ‏{ $face_count } وجه)
cmd-session-set-triangulation-tri-id-color = تم تعيين لون التثليث { $tri_id } إلى { $color }
cmd-session-triangulation-load-no-result = انتهى تحميل التثليث للمسار { $path } دون نتيجة
cmd-session-triangulation-failed = فشلت عملية التثليث: { $message }
cmd-session-unloaded-triangulation-name = تم إلغاء تحميل التثليث «{ $name }»
cmd-slice-entered-slice-view-cx-cy = تم دخول عرض الشريحة عند { $cx }، { $cy }، { $cz } باتجاه { $dx }، { $dy } (خط بطول { $length } م)
cmd-slice-exited-slice-view = تم الخروج من عرض الشريحة
cmd-slice-reset-section-view-fit-extents = إعادة تعيين عرض المقطع (ملاءمة للحدود)
cmd-slice-set-section-grid-enabled = تفعيل شبكة المقطع = { $enabled }
cmd-split-created-2-open-polylines = تم إنشاء خطين متعددين مفتوحين
cmd-split-line = تقسيم الخط
cmd-split-points-needs-interior-vertex = التقسيم عند النقاط: اختر رأسًا داخليًا للخط المفتوح
cmd-split-points-needs-non-adjacent-vertices = التقسيم عند النقاط: اختر رأسين غير متجاورين من الخط المتعدد
cmd-split-polyline-into-two = قُسّم الخط المتعدد المصدر إلى خطين متعددين مفتوحين
cmd-text-edit-finished = انتهى تحرير نص الكائن { $object_id }
cmd-text-updated = تم تحديث النص على الكائن { $object_id }
cmd-view-centre-rotation-not-available-flying = مركز الدوران غير متاح في وضع الطيران
cmd-view-fixed-centre-rotation-x-y = تم تثبيت مركز الدوران عند { $x }، { $y }، { $z }
cmd-view-no-point-under-cursor-fix = لا توجد نقطة تحت المؤشر لتثبيت مركز الدوران عليها
cmd-view-released-centre-rotation = تم تحرير مركز الدوران
cmd-view-reset-view-fit-extents = إعادة تعيين العرض (ملاءمة للحدود)
cmd-view-set-topology-wireframes-enabled = تعيين الإطارات السلكية للطبوغرافيا = { $enabled }
cmd-view-set-view-points-enabled = تعيين نقاط العرض = { $enabled }
cmd-view-set-xy-grid-enabled = تفعيل شبكة XY = { $enabled }
cmd-view-zoom-extents-preserving-angle = التكبير إلى الحدود (مع الحفاظ على الزاوية)

## Common strings

common-add-product = إضافة المنتج
common-background = الخلفية
common-block-model = نموذج الكتل
common-block-models = نماذج الكتل
common-cancelled = ملغى
common-chamfer = شطفة
common-choose = اختر...
common-circle = دائرة
common-click-point-fix-centre-rotation = انقر فوق نقطة لتثبيت مركز الدوران عليها
common-clip-surface-polyline = قص السطح بخط متعدد المقاطع...
common-closed = مغلق
common-colour = اللون
common-confirm-omf-rewrite = تأكيد إعادة كتابة OMF
common-could-not-replace-current-project = تعذر استبدال المشروع الحالي: { $error }
common-count-object-s = { $count } كائن
common-create = إنشاء
common-create-batter-berm = إنشاء مصطبة وحاجز أمان
common-create-bezier-curve = إنشاء منحنى بيزير
common-create-block-model = إنشاء نموذج الكتل
common-create-block-model-ellipsis = إنشاء نموذج الكتل...
common-create-circle = إنشاء دائرة
common-create-drill-pattern = إنشاء نمط حفر
common-create-layer = إنشاء طبقة
common-create-line = إنشاء خط
common-create-ore-triangulation = إنشاء شبكة مثلثية للخام
common-create-ore-triangulation-ellipsis = إنشاء شبكة مثلثية للخام...
common-create-point = إنشاء نقطة
common-create-polyline = إنشاء خط متعدد المقاطع
common-create-triangulation = إنشاء شبكة مثلثية...
common-crosses = علامات تقاطع
common-cut = قص
common-cut-topology-pit-shell = قطع السطح الطبوغرافي مع غلاف الحفرة المنجمية...
common-delete-layer = حذف الطبقة
common-delete-product = حذف المنتج
common-delete-selection = حذف الاختيار
common-designs = التصاميم
common-discard-layer-changes = تجاهل تغييرات الطبقة
common-down = أسفل
common-drape-topology = إسقاط إلى السطح الطبوغرافي
common-easting = الإحداثي الشرقي
common-edit-object = تعديل الكائن
common-edit-text = تحرير النص
common-elevation = الارتفاع
common-exit-without-saving = الخروج دون حفظ
common-export-engineering-drawing = تصدير رسم هندسي
common-filter = تصفية
common-fly-mode = وضع الطيران
common-generate-contour-lines = توليد خطوط الكنتور...
common-hide-all = إخفاء الجميع
common-hide-selection = إخفاء الاختيار
common-ignore = تجاهل
common-import-csv-block-model = استيراد CSV نموذج الكتل
common-import-dxf = استيراد DXF
common-incline-design-project = مشروع Incline Design
common-layer = الطبقة
common-legend = مفتاح الخريطة
common-line = خط
common-line-weight = وزن الخط
common-lock-all = قفل الكل
common-lock-selection = قفل الاختيار
common-m = m
common-max = أقصى
common-merge-shell-into-topology = دمج القشرة في السطح الطبوغرافي
common-merge-shell-into-topology-ellipsis = دمج القشرة في السطح الطبوغرافي...
common-move-collar = نقل الطوق
common-move-design = نقل التصميم
common-move-selection = تحريك الاختيار
common-new-product = منتج جديد
common-no-block-models = لا توجد نماذج كتل
common-no-design-layers = لا توجد طبقات تصميم
common-no-drill-holes = لا توجد ثقوب حفر
common-no-file-chosen = لم يتم اختيار ملف
common-no-open-project = لا يوجد مشروع مفتوح
common-no-point-clouds = لا توجد سحب نقاط
common-no-triangulations = لا توجد تثليثات
common-none = لا شيء
common-northing = الإحداثي الشمالي
common-offset = الإزاحة
common-open = فتح
common-orientation = الاتجاه
common-point = نقطة
common-point-cloud = سحابة نقاط
common-point-clouds = سحب نقطية
common-polyline = خط متعدد
common-polyline-layer = خط متعدد على «{ $layer }»
common-project = المشروع
common-rasters = بيانات نقطية
common-redo = إعادة
common-relimit-line = إعادة تحديد الخط
common-remove-project = إزالة المشروع
common-reset-view = إعادة ضبط العرض
common-reveal-all = الكشف عن كل شيء
common-reveal-finder = عرض في Finder
common-rotate-collar = تدوير الطوق
common-save-exit = حفظ والخروج
common-scale-bar = شريط القياس
common-set-initiation-point = تعيين نقطة بدء
common-shape = الشكل
common-shell = مع الغلاف
common-slashes = شرطات مائلة
common-slice = الشريحة
common-slice-triangulation-z-range = تقطيع الشبكة المثلثية حسب نطاق Z...
common-surface-contours = خطوط كنتور السطح
common-text = النص
common-degree-suffix = °
common-tie-holes = ربط الثقوب
common-triangulations = شبكات مثلثية
common-trim-topology = قص حسب السطح الطبوغرافي...
common-undo = تراجع
common-undrape-all = إزالة الإسقاط عن الكل
common-uniform-white = أبيض موحّد
common-unlock-all = فتح الكل
common-untitled = بلا عنوان
common-up = أعلى
common-vertical-exaggeration = المبالغة العمودية
common-x = x
common-zoom-extents = زوم إلى المدى

## Confirmations strings

confirmations-close-project-unsaved-changes = إغلاق المشروع: تغييرات غير محفوظة
confirmations-close-without-saving = إغلاق دون حفظ
confirmations-delete = حذف
confirmations-delete-objects = حذف الأشياء
confirmations-discard = تجاهل
confirmations-discard-all-unsaved-changes-layer =
    هل تريد تجاهل كل التغييرات غير المحفوظة في الطبقة «{ $name }»؟
    ستُعاد قراءة الطبقة المحفوظة مع إبقاء تغييرات الطبقات الأخرى. لا يمكن التراجع عن ذلك.
confirmations-discard-all-unsaved-changes-name =
    هل تريد تجاهل كل التغييرات غير المحفوظة في «{ $name }»؟
    ستُعاد قراءة آخر نسخة محفوظة من القرص. لا يمكن التراجع عن ذلك.
confirmations-discard-changes = تجاهل التغييرات
confirmations-exit-unsaved-changes = الخروج: تغييرات غير محفوظة
confirmations-incline-design-cannot-reproduce-all = لا يستطيع Incline Design إعادة إنتاج كل محتوى OMF الأصلي. سيؤدي الحفظ إلى حذف ما يلي:
confirmations-product = المنتج
confirmations-project = هذا المشروع
confirmations-remove-name-delete-its-browser = هل تريد إزالة «{ $name }» وحذف نسخته المخزنة في المتصفح؟ ستُفقد التغييرات غير المحفوظة.
confirmations-remove-project-unsaved-changes = إزالة المشروع: تغييرات غير محفوظة
confirmations-remove-without-saving = إزالة دون حفظ
confirmations-replace-project-unsaved-changes = استبدال المشروع: تغييرات غير محفوظة
confirmations-save = حفظ
confirmations-save-anyway = حفظ على أي حال
confirmations-save-changes-current-project-before = حفظ التغييرات على المشروع الحالي قبل استبداله؟
confirmations-save-changes-name-before-closing = هل تريد حفظ التغييرات في «{ $name }» قبل إغلاقه؟
confirmations-save-changes-name-before-removing = هل تريد حفظ التغييرات في «{ $name }» قبل إزالته من Incline Design؟
confirmations-save-close = حفظ وإغلاق
confirmations-save-modified-project-before-exiting = حفظ المشروع المعدل قبل الخروج؟
confirmations-save-to-browser-before-exit = حفظ المشروع المعدل في متصفح التخزين قبل الخروج؟
confirmations-save-remove = حفظ وإزالة

## Console strings

console-copy-all = نسخ الكل
console-copy-message = نسخ الرسالة
console-error = خطأ
console-info = معلومات
console-no-console-activity-yet = لا يوجد نشاط في وحدة التحكم بعد
console-pending = معلّق
console-progress-summary = قيد التنفيذ · { $summary }
console-success = نجاح
console-warn = تحذير

## Csv strings

csv-block-model-category = الفئة
csv-block-model-value = القيمة

## Drill strings

drill-hole-add-stop = إضافة توقف
drill-hole-all-rendered-intervals-opaque-white = كل فترات التصوير غير شفافة بيضاء.
drill-hole-burden-spacing-must-greater-than = يجب أن يكون خط المقاومة والتباعد أكبر من صفر
drill-hole-choose-valid-closed-polyline = اختر خطًا متعددًا مغلقًا صالحًا
drill-hole-colour-scale = مقياس الألوان
drill-hole-field = الحقل
drill-hole-grayscale = تدرج رمادي
drill-hole-green-yellow-red = أخضر–أصفر–أحمر
drill-hole-heat = حراري
drill-hole-no-holes-fit-inside-boundary = لا توجد ثقوب تلائم هذا الحد عند خط المقاومة والتباعد الحاليين
drill-hole-pattern-too-many-holes = يتجاوز النمط الحد الأقصى البالغ { $maximum } حفرة؛ زد خط المقاومة أو التباعد
drill-hole-preset = إعداد مسبق
drill-hole-px = بكسل
drill-hole-rainbow = قوس قزح
drill-hole-reset-preset = إعادة تعيين الإعداد المسبق
drill-hole-rotation-offsets-must-contain-valid = يجب أن يحتوي الدوران والإزاحات على أرقام صالحة
drill-hole-selected-polyline-has-no-usable = لا يملك الخط المتعدد المحدد مساحة XY صالحة للاستخدام
drill-hole-smooth-interpolation = الاستيفاء السلس
drill-hole-spacing-would-scan-too-many = سيفحص هذا التباعد خلايا شبكة كثيرة جدًا؛ زد خط المقاومة أو التباعد (الحد الأقصى { $maximum } حفرة)
drill-hole-square = مربع
drill-hole-staggered = متدرج
drill-hole-stepped-bands = نطاقات متدرجة
common-times-sign = ×
common-minus-sign = −
drill-hole-unsupported-drillhole-source = مصدر ثقوب حفر غير مدعوم
drill-hole-width = العرض
drill-pattern-arrangement = الترتيب
drill-pattern-axis-offset = إزاحة المحور { $axis }
drill-pattern-blast-shape = شكل التفجير
drill-pattern-burden = خط المقاومة
drill-pattern-choose-closed-blast-boundary-then = اختر حدود تفجير مغلقة، ثم اضبط الشبكة. تتحدث ثقوب الحفر مباشرة في منفذ العرض.
drill-pattern-closed-design-polyline-whose-xy = الخط المتعدد التصميمي المغلق الذي سيُملأ إسقاطه XY بالثقوب.
drill-pattern-rotation-help = دوران النمط عكس اتجاه عقارب الساعة من المحور العام { $axis }.
drill-pattern-distance-between-holes-along-each = المسافة بين الثقوب على امتداد كل صف من النمط.
drill-pattern-name-hint = مثال: القطع الغربي 03
drill-pattern-diameter-help = قطر الحفرة النهائي. يُدخل بالملليمتر ويُخزّن مع كل حفرة مُنشأة.
drill-pattern-hole-depth = عمق الحفرة
drill-pattern-hole-diameter = قطر الحفرة
drill-pattern-move-over-closed-polyline-then = مرر فوق خط متعدد مغلق ثم انقره في منفذ العرض. يلغي Esc الاختيار.
drill-pattern-name-help = اسم مجموعة بيانات ثقوب الحفر المُنشأة في المشروع.
drill-pattern-none-picked = لم يتم اختيار شيء
drill-pattern-pattern-name = اسم النمط
drill-pattern-spacing-help = المسافة العمودية بين صفوف النمط.
drill-pattern-pick = اختيار
drill-pattern-preview-count-hole-s-diameter = معاينة: { $count } حفرة · قطر { $diameter } مم · عمق { $depth } م
drill-pattern-rotation = الدوران
drill-pattern-shift-pattern-grid-along-global = تحريك شبكة النمط على طول المحور العام { $axis } مع إبقائها مقصوصة على شكل التفجير.
drill-pattern-spacing = التباعد
drill-pattern-staggered-offsets-every-second-row = يزيح النمط المتدرج كل صف ثانٍ بمقدار نصف التباعد.
drill-pattern-vertical-depth-below-each-collar = العمق الرأسي أسفل كل طوق.

## Dxf strings

dxf-block-nesting-too-deep = تجاوز تداخل كتل DXF أقصى عمق ({ $depth })؛ سيتم تخطي «{ $name }»
dxf-circular-block-reference = اكتُشف مرجع دائري لكتلة DXF: «{ $name }»
dxf-undefined-layer = أشار كيان DXF إلى الطبقة غير المعرّفة «{ $name }»؛ واستُورد باسم «{ $fallback }»
dxf-import-budget-exceeded = تجاوز استيراد DXF ميزانية { $what } ‏({ $limit })؛ سيتم تخطي الأشكال الهندسية المتبقية
dxf-insert-unknown-block = يشير DXF INSERT إلى كتلة غير معروفة «{ $name }»

## Edit strings

edit-absolute-length = الطول المطلق
edit-absolute-rl = RL المطلق
edit-action = الإجراءات
edit-angle = زاوية
edit-dip-help = الزاوية من الأفقي، والسالب إلى الأسفل: ‎-90 حفرة عمودية.
edit-app-web-not-recommended-production = لا يُنصح باستخدام { $app } Web للإنتاج. استخدمه للعرض التجريبي فقط.
edit-application = التطبيق
edit-apply = تطبيق
edit-apply-pick-target = تطبيق وتحديد الهدف
edit-axis-value = قيمة { $axis }
edit-azimuth = السمت
edit-batter-angle = زاوية ميل واجهة المصطبة (°)
edit-azimuth-help = اتجاه حفر الثقوب بالدرجات مع عقارب الساعة من شمال الشبكة.
edit-bench-height = ارتفاع المصطبة
edit-benches = مصاطب
edit-berm-width = عرض حاجز الأمان
edit-bezier-curve = منحنى بيزير
edit-choose-layer = اختر طبقة
edit-measure-help = اختر ما إذا كانت القيمة هي المسافة على الميل أو العرض الأفقي أو الارتفاع الرأسي.
edit-choose-which-two-polyline-paths = اختر أي مساري الخط المتعدد بين الرأسين المحددين سيُستبدل. يشمل الطول الارتفاع والحواف المنحنية.
edit-click-corner-closed-polyline = انقر على زاوية على خط متعدد المقاطع مغلق.
edit-click-open-closed-polyline-begin = انقر على فتح أو إغلاق خط متعدد المقاطع للبدء.
edit-click-second-vertex-replacement-span = انقر على الرأس الثاني من فترة الاستبدال.
edit-click-vertex-start-replacement-span = انقر على رأس لبدء فترة الاستبدال.
edit-collide-triangulation = التصادم مع شبكة مثلثية
edit-confirm-selection = تأكيد الاختيار
edit-control-point-1 = نقطة التحكم 1
edit-control-point-2 = نقطة التحكم 2
edit-copy = نسخ
edit-corner-radius-limited-so-replacement = نصف قطر الزاوية، محدودة بحيث لا يمكن للمستبدل أن يمر فوق القمم المجاورة.
edit-create-new-layer = إنشاء طبقة جديدة
edit-create-new-project = إنشاء مشروع جديد
edit-create-project = إنشاء مشروع
edit-delta-length-m-use = تغير الطول (م، استخدم + أو -)
edit-dip = الميل
edit-direction = الاتجاه
edit-distance = المسافة
edit-distance-along-slope = المسافة على طول المنحدر
edit-download-free-native-version-our = نزّل الإصدار الأصلي المجاني من موقعنا الإلكتروني ↗
edit-drill-hole = ثقب حفر
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = النهاية
edit-enter-valid-elevation = أدخل ارتفاعاً صالحاً
edit-exit-slice = الخروج من الشريحة
edit-finish-polyline = إنهاء الخط المتعدد المقاطع
edit-generate-batter-berms = إنشاء المصاطب وحواجز الأمان
edit-height = الارتفاع
edit-height-change = تغير الارتفاع
edit-height-mode = وضع الارتفاع
edit-horizontal-distance = المسافة الأفقية
edit-horizontal-width-each-flat-berm = عرض أفقي لكل مسطح حاجز أمان بين واجهات المصاطب المتتالية.
edit-hover-choose-which-end-move = مرّر المؤشر لتحديد الطرف الذي تريد تحريكه، ثم انقر للتأكيد.
edit-insert-point-elevation = إضافة نقطة عند الارتفاع
edit-intersect = تقاطع
edit-kind-properties = { $properties } { $kind }
edit-layer-name = اسم الطبقة
edit-load-project = تحميل المشروع
edit-longest = الأطول
edit-m-s = m/s
edit-measure = القياس
edit-mit-license = رخصة MIT
edit-mode = الوضع
edit-move = تحرك
edit-move-layer = الانتقال إلى الطبقة
edit-move-which-end = تحرّك إلى أيّ نهاية
edit-movement-speed-slice-when-using = سرعة حركة القطعة عند استخدام مفاتيح التنقل.
edit-moving-end-endpoint = جارٍ التحريك: نقطة النهاية
edit-moving-start-endpoint = جارٍ التحريك: نقطة البداية
edit-new-length-m = الطول الجديد (م)
edit-new-project = مشروع جديد
edit-number-complete-batter-berm-levels = عدد مستويات المنحدر والمصطبة الكاملة. يقتصر الحد الأقصى على أعمق مستوى يحافظ على الشكل المحدد.
edit-bezier-segments-help = عدد مقاطع الخط المستخدمة لتقريب المنحنى بين الرأسين المحددين.
edit-chamfer-segments-help = عدد المقاطع المستقيمة لتقريب الزاوية المستديرة. استخدم 1 لشطفة مستقيمة.
edit-object = الكائن
edit-offset-element = عنصر الإزاحة
edit-pick-side = اختر الجانب
edit-pit = حفرة منجمية
edit-project-name = اسم المشروع
edit-properties = الخصائص
edit-radius = نصف قطر
edit-recent = الأخيرة
edit-relative = النسبي (+/-)
edit-elevation-mode-help = يطبق «النسبي» تغيرًا رأسيًا على كل نقطة. يسقط «RL المطلق» كل النقاط على ارتفاع هدف واحد.
edit-remove-from-list = إزالة من القائمة
edit-replace-path = استبدال المسار
edit-rotate = تدوير
edit-rotation-speed-slice-when-using = سرعة الدوران للقطعة عند استخدام Q و E.
edit-s = °/s
edit-segments = القطاعات
edit-segments-lying-elevation-ignored = يتم تجاهل القطاعات الموجودة على هذا الارتفاع.
edit-endpoint-help = حدد نقطة النهاية التي ستتغير؛ تظل نقطة النهاية الأخرى ثابتة.
edit-selected-holes-point-different-ways = تشير الثقوب المحددة إلى اتجاهات مختلفة. يضبط تطبيقها جميعًا على هذه الزوايا.
edit-selected-start-end-point-moves = تتحرك نقطة البداية أو النهاية المحددة في اتجاه الخط، وتبقى نقطة النهاية المقابلة ثابتة.
edit-set-axis = تعيين { $axis }
edit-shortest = الأقصر
edit-slice-view = عرض الشريحة
edit-slope-angle-each-batter-face = زاوية الميل لكل واجهة المصطبة، مقاسة من الأفق.
edit-slope-angle-offset-positive-negative = زاوية ميل الإزاحة. تنقل الزوايا الموجبة والسالبة النسخة فوق المصدر أو تحته أثناء تحركها جانبيًا.
edit-speed = السرعة
edit-start = البداية
edit-stockpile = كومة تخزين
edit-stop-generated-offset-where-its = أوقف الإزاحة الناتجة حيث يلتقي مسارها أولاً بشبكة مثلثية مرئية.
edit-target-rl = المنسوب المستهدف
edit-text-colour-opacity = لون النص ودرجة عتامته.
edit-thickness-visible-slice-slab-centred = سمك اللوحة المرئية المركزية على مؤشر الرؤية العامة.
edit-translation-axis-help = مسافة الانتقال على طول محور العالم { $axis }.
edit-type = النوع
edit-type-direction-together-set-offset = يحدد النوع والاتجاه معًا جانب الإزاحة. الحفرة + أعلى والمخزون + أسفل يتجهان للخارج؛ الحفرة + أسفل والمخزون + أعلى يتجهان للداخل.
edit-bench-direction-help = يرفع «أعلى» كل مصطبة بمقدار ارتفاعها ويخفضها «أسفل». ويعكس ذلك جانب الإزاحة أيضًا؛ راجع «النوع».
edit-value-help = يتم تفسير القيمة باستخدام وضع القياس والارتفاع المحدد.
edit-vertical-rise-fall-each-bench = ارتفاع أو انخفاض عمودي لكل مصطبة قبل إنشاء حاجز أمان التالي.
edit-bezier-control-point-1-help = إحداثيات X وY وZ العالمية لنقطة تحكم بيزيه الأولى.
edit-bezier-control-point-2-help = إحداثيات X وY وZ العالمية لنقطة تحكم بيزيه الثانية.

## Events strings

events-couldn-t-exit-error = تعذر الخروج: { $error }
events-couldn-t-save-error = تعذر الحفظ: { $error }
events-set-elevation = تعيين الارتفاع
events-set-elevation-from-cursor-hit = عُيّن الارتفاع من نقطة المؤشر إلى Z ‏{ $z }
events-tool-not-available-section-view = هذه الأداة غير متاحة في عرض المقطع

## Explorer strings

explorer-clear-active-triangulation-texture = مسح نسيج الشبكة المثلثية النشطة
explorer-delete-from-project = الحذف من المشروع
explorer-discard-changes = تجاهل التغييرات...
explorer-download = تنزيل
explorer-drape-over-surface = إسقاط فوق السطح
explorer-draped-over-surface = مغطّى على سطح
explorer-duplicate = تكرار
explorer-face-colour = لون الوجه
explorer-id-block-model-id-source =
    المعرّف: block-model:{ $id }{ $source }
    { $count } متغير لون
explorer-id-drill-holes-id-source =
    المعرّف: drill-holes:{ $id }{ $source }
    { $holes } بئر
    { $fields } حقل لون
explorer-id-point-cloud-id-source =
    المعرّف: point-cloud:{ $id }{ $source }
    { $count } نقطة
explorer-raster-id =
    المعرّف: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = المعرّف: triangulation:{ $id }{ $source }
explorer-load = تحميل
explorer-lock = قفل
explorer-select-all-objects = اختيار جميع الكائنات
explorer-source-name = المصدر: { $name }
explorer-unload = إلغاء التحميل
explorer-unlock = فتح القفل

## Files strings

files-automatic-colour = لون تلقائي
files-automatic-rl-spacing = تباعد مناسيب تلقائي
files-axis-scale-ratio = نسبة مقياس المحور { $axis }
files-ok = موافق
files-reset-scale = إعادة تعيين إلى 1×
files-rl-grid-options = خيارات شبكة المناسيب
files-rl-spacing = تباعد المناسيب
files-scales-z-distances-visually-without = يُقيّس مسافات Z بصريًا دون تغيير الإحداثيات المخزّنة.
files-thickness = السُّمك
files-xy-grid-options = خيارات شبكة XY

## Gpu strings

gpu-cache-block-model-surface-build-failed = فشل إنشاء سطح نموذج الكتل: { $error }
gpu-cache-block-model-surface-build-worker = انقطع عامل إنشاء سطح نموذج الكتل
gpu-cache-block-model-surface-chunk-rejected = رُفض جزء سطح نموذج الكتل قبل تخصيص GPU: المثيلات={ $instances } بايت، الحد={ $limit } بايت
gpu-cache-block-volume-worker-disconnected = انقطع عامل إعداد حجم الكتل
gpu-cache-translucent-volume-could-not-built = تعذر إنشاء الحجم الشفاف ({ $error })؛ سيُعرض نموذج الكتل هذا كمكعبات بدلًا منه.
gpu-cache-edge-chunk-rejected = رُفض جزء حواف التثليث قبل تخصيص GPU: المثيلات={ $instances } بايت، الحد={ $limit } بايت
gpu-cache-triangulation-chunk-rejected = رُفض جزء تثليث GPU قبل التخصيص: الرؤوس={ $vertices } بايت، الفهارس={ $indices } بايت، الحد={ $limit } بايت
gpu-cache-triangulation-too-many-vertices = يحتوي التثليث «{ $name }» على { $count } رأس (> u32::MAX)؛ لا يمكن تقسيمه لـ GPU
gpu-cache-triangulation-uploaded = رُفع التثليث «{ $name }» في { $chunks } جزء مكاني ({ $faces } وجه)
i18n-active-language = اللغة النشطة هي { $language } (المضمّنة: { $bundled })
i18n-could-not-select-language-error = تعذر اختيار لغة: { $error }

## Init strings

init-gpu-adapter-vendor-name-backend = محول الرسومات: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver = برنامج تشغيل الرسومات: { $driver } { $driver_info }
init-gpu-supports-maximum-buffer-size = يدعم معالج الرسومات مخزنًا مؤقتًا أقصاه { $size } MiB؛ قد لا تظهر المشاهد الكبيرة بالكامل
init-surface-present-mode = وضع عرض السطح: { $mode }
init-wgpu-error-continuing-error = خطأ wgpu (سيستمر التشغيل): { $error }

## Io strings

io-ascii-points-xyz-pts = نقاط ASCII ‏(.xyz، .pts)
io-attribute = سمة
io-blank-header = (رأس فارغ)
io-block-model = نموذج الكتل:
io-choose-file-purpose-map-its = اختر غرض الملف لتعيين أعمدته.
io-choose-loaded-block-model = اختر نموذج كتل محمّلًا
io-choose-loaded-layer = اختر طبقة محمّلة
io-choose-loaded-triangulation = اختر تثليثًا محمّلًا
io-choose-purpose = اختر الغرض…
io-choose-source-file-files-import = اختر ملف المصدر أو الملفات التي تريد استيرادها.
io-collar = فوهة الثقب
io-column-mapping = تعيين الأعمدة
io-comma-separated-values-csv = قيم مفصولة بفواصل (.csv)
io-csv-files = ملفات CSV
io-default = افتراضي
io-depth = العمق
io-diameter = القطر
io-drawing-exchange-format-dxf = تنسيق تبادل الرسومات (.dxf)
io-drill-holes = ثقوب الحفر
io-east-x = الشرق / X
io-elevation-z = المنسوب / Z
io-end-x = نهاية X
io-end-y = نهاية Y
io-end-z = نهاية Z
io-explicit-segments = مقاطع صريحة
io-export = تصدير
io-export-csv-block-model = تصدير نموذج الكتل بصيغة CSV
io-export-dxf = تصدير DXF
io-export-one-layer = تصدير طبقة واحدة
io-export-ply = تصدير PLY
io-export-stl = تصدير STL
io-export-wavefront-obj = تصدير Wavefront OBJ
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = استيراد
io-import-ascii-point-cloud = استيراد سحابة نقاط ASCII
io-import-drillhole-csv-bundle = استيراد حزمة CSV لثقوب الحفر
io-import-geotiff = استيراد GeoTIFF
io-import-las-laz-point-cloud = استيراد سحابة نقاط LAS/LAZ
io-import-pcd-point-cloud = استيراد سحابة نقاط PCD
io-import-ply = استيراد PLY
io-import-stl = استيراد STL
io-import-wavefront-obj = استيراد Wavefront OBJ
io-interval = الفاصل
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-mapped-csv-bundle-csv = حزمة CSV معيّنة الحقول (.csv)
io-model-file = ملف النموذج
io-name-count-files = { $name } + { $count } ملف
io-no-csv-chosen = لم يتم اختيار ملف .csv
io-no-csv-files-chosen = لم يتم اختيار ملفات CSV
io-no-dxf-chosen = لم يتم اختيار ملف .dxf
io-no-omf-chosen = لم يتم اختيار ملف .omf
io-north-y = الشمال / Y
io-ply = PLY (.ply)
io-point-cloud-data-pcd = بيانات سحابة النقاط (.pcd)
io-source-file = ملف المصدر
io-start-x = بداية X
io-start-y = بداية Y
io-start-z = بداية Z
io-stl = STL (.stl)
io-triangulation = التثليث:
io-unmapped = غير معيّن
io-wavefront-obj = Wavefront OBJ (.obj)

## Jobs strings

jobs-background-task-poll-label-ended = انتهت مهمة الخلفية «{ $poll_label }» دون نتيجة
jobs-discarded-stale-result = تم تجاهل نتيجة الخلفية القديمة لـ«{ $poll_label }» لأن مصدرًا تغيّر أو أُغلق

## Logging strings

logging-activity-completed = اكتمل النشاط
logging-activity-started = بدأ النشاط
logging-application-id-id = معرّف التطبيق: { $id }
logging-application-name = اسم التطبيق: { $name }
logging-application-startup = بدء تشغيل التطبيق
logging-build-target-os-architecture = هدف البناء: { $os }-{ $architecture }
logging-completed = مكتمل
logging-count-messages = { $count } رسالة
logging-desktop-session-xdg-session-type = جلسة سطح المكتب: XDG_SESSION_TYPE={ $type }، XDG_CURRENT_DESKTOP={ $desktop }، WAYLAND_DISPLAY={ $wayland }، DISPLAY={ $display }
logging-initialising-incline-design = جارٍ تهيئة Incline Design
logging-locale-environment = بيئة الإعدادات المحلية: LANG={ $lang }، LC_ALL={ $locale }، TZ={ $timezone }
logging-macos-session = جلسة macOS: USER={ $user }، SHELL={ $shell }
logging-operating-system-gnu-linux = نظام التشغيل: GNU / Linux
logging-operating-system-macos = نظام التشغيل: macOS
logging-operating-system-microsoft-windows = نظام التشغيل: Microsoft Windows
logging-pointer-width = عرض المؤشر: { $width } بت
logging-process-id-id = معرّف العملية: { $id }
logging-release-version = إصدار البرنامج: { $version }
logging-renderer = العارض
logging-rust-compiler-host = مضيف مترجم Rust: { $host }
logging-system = النظام
logging-system-error = خطأ في النظام
logging-unknown = غير معروف
logging-windows-session-sessionname-session = جلسة Windows: SESSIONNAME={ $session }، USERNAME={ $user }
logging-working = جارٍ العمل…

## Mac strings

mac-cannot-install-macos-menu-bar = لا يمكن تثبيت شريط قوائم macOS خارج الخيط الرئيسي
mac-quit-app = الخروج من { $app }

## Main strings

main-incline-design-web-startup-failed = فشل بدء Incline Design Web: { $error }

## Menu strings

menu-count-files-selected = تم تحديد { $count } ملف

## Object strings

object-edit-appearance = المظهر
object-edit-arc-circle = القوس والدائرة
object-edit-arc-segments = أجزاء القوس
object-edit-bulge = الانتفاخ
object-edit-bulge-arcs-horizontal-data-model = أقواس الانتفاخ أفقية بحسب نموذج البيانات: يلتف القوس في المسقط الأفقي، ويتغير الارتفاع في خط مستقيم من رأس إلى التالي.
object-edit-centre-x = مركز X
object-edit-centre-y = مركز Y
object-edit-centre-z = مركز Z
object-edit-chord = الوتر
object-edit-colour-layer = اللون حسب الطبقة
object-edit-enter-number = أدخل رقمًا
object-edit-follow-owning-layer-s-colour = اتباع لون الطبقة المالكة بدلاً من لون مثبت لهذا الكائن.
object-edit-id = المعرّف
object-edit-identity = مطابقة
object-edit-insert-after = إدراج بعد
object-edit-join-last-vertex-back-first = يصل الرأس الأخير مرة أخرى بالأول.
object-edit-length = الطول { $length } م
object-edit-move-down = نقل لأسفل
object-edit-move-up = نقل لأعلى
object-edit-object-has-no-arc-segments = لا تحتوي هذا الكائن على أجزاء قوس.
object-edit-object-has-single-position = لهذا الكائن موضع واحد فقط.
object-edit-object-needs-least-required-vertices = يحتاج هذا الكائن إلى { $required } رؤوس على الأقل
object-edit-one-more-properties-not-valid = خاصية واحدة أو أكثر ليست رقمًا صالحًا
object-edit-perimeter-area = المحيط { $length } م، المساحة { $area } م²
object-edit-reverse = عكس
object-edit-row-invalid-number = الصف { $row }: الموضع أو الانتفاخ ليس رقمًا صالحًا
object-edit-sweep = الاكتساح
object-edit-text-not-number = «{ $text }» ليس رقمًا
object-edit-vertices = الرؤوس

## Omf strings

omf-element-name-has-count-tie = العنصر «{ $name }» لديه { $count } توصيلات تشير إلى ثقوب لم يعد يحتويها
omf-ignoring-colour-map-omf-attribute = سيتم تجاهل خريطة الألوان في سمة OMF «{ $attribute }»: { $error }
omf-mining-data-exported-incline = بيانات تعدين صدّرها Incline
omf-import = استيراد OMF
omf-texture = نسيج OMF
omf-validation-warnings = تحذيرات تحقق OMF: { $warnings }
omf-application-metadata-dropped = لا تُحتفظ ببيانات تطبيق المشروع الوصفية «{ $application }»
omf-project-author-not-retained = لا يُحتفظ بمؤلف المشروع
omf-project-description-not-retained = لا يُحتفظ بوصف المشروع
omf-unsupported-metadata-keys = يحتوي المشروع على مفاتيح بيانات وصفية غير مدعومة: { $keys }

## Plot strings

plot-1-1000-one-millimetre-sheet = عند المقياس 1:1000، يمثل مليمتر واحد على الورقة مترًا واحدًا على الأرض.
plot-1-scale-covers-width-height = 1:{ $scale } · يغطي { $width } × { $height } م
plot-all-visible-data = كل البيانات المرئية
plot-automatic-grid-interval = فترة الشبكة الآلية
plot-border = الحدود
plot-centre = التوسيط على
plot-fit-scale-help = اختر أصغر مقياس تقليدي يناسب كل شيء مرئي على الورقة.
plot-coordinate-grid = شبكة التنسيق
plot-current-view-centre = مركز العرض الحالي
plot-date-caps = التاريخ
plot-date = التاريخ
plot-dots-per-inch-paper-size = نقطة لكل بوصة. يمكن تحويل هذا المقاس حتى { $max_dpi } dpi؛ ‏300 dpi جودة طباعة عادية.
plot-dpi = dpi
plot-drawing-no = رقم الرسم
plot-drawing-number = رقم الرسم
plot-drawn-by-caps = رسم بواسطة
plot-drawn-by = رسم بواسطة
plot-e-g-example-gold-project = مثال: مشروع الذهب النموذجي
plot-entered-coordinates = الإحداثيات المدخلة
plot-export-png = تصدير PNG...
plot-fit-scale-visible-data = ملاءمة المقياس للبيانات المرئية
plot-grid-interval = فترة الشبكة
plot-landscape = أفقي
plot-lists-visible-surfaces-design-layers = يسرد الأسطح المرئية وطبقات التصميم بألوانها.
plot-margin = الهامش
plot-margins-leave-no-room-map = لا تترك الهوامش مساحة للخريطة
plot-metres-scale-1-scale = أمتار    المقياس 1:{ $scale }
plot-mm = mm
plot-north-arrow = السهم الشمالي
plot-nothing-visible-draw = لا يوجد شيء مرئي لرسمه
plot-paper = الورق
plot-paper-orientation-width-height-mm = { $paper } ‏{ $orientation } · ‏{ $width } × { $height } مم
plot-paper-size = حجم الورق
plot-pick-interval-reads-roughly-every = اختر فترة تقرأ تقريبا كل 50 مليمتر على ورقة الطباعة.
plot-plan = المخطط
plot-scale-must-be-positive = يجب أن يكون مقياس الرسم عددًا موجبًا
plot-png-written-sheet-s-exact = يُكتب PNG بالمقاس الفعلي الدقيق للورقة ويسجل DPI، لذلك يُطبع بالمقياس الحقيقي.
plot-portrait = عمودي
plot-resolution = الدقة
plot-rev = المراجعة
plot-revision = المراجعة
plot-scale = المقياس
plot-scale-ratio = المقياس 1:
plot-scale-framing = المقياس والإطار
plot-sheet-furniture = عناصر تجهيز الورقة
plot-size-width-height-mm = { $size } ‏({ $width } × { $height } مم)
plot-subtitle = العنوان الفرعي
plot-title = العنوان
plot-title-block = كتلة العنوان
plot-today = اليوم

## Products strings

products-add-initiation = إضافة بدء
products-delay = التأخير
products-delay-palette = لوحة ألوان التأخير
products-how-long-after-shot-fired = المدة بعد إطلاق التفجير التي يبدأ عندها هذا الطوق الجولة.
products-initiation-name = بدء · { $name }
products-milliseconds-between-one-hole-firing = المللي ثانية بين انفجار ثقب والذي يليه.
products-ms = ms
products-no-products = لا توجد منتجات
products-remove = إزالة
products-update = تحديث

## Progress strings

progress-percent-done-total = { $percent } ‏({ $done } من { $total })
progress-task-finished = { $task }: انتهت

## Project strings

project-item = عنصر

## Properties strings

properties-adds-view-dependent-rim-highlight = يضيف إبرازًا للحواف يعتمد على العرض عند حدود الكتل والمواد. يؤدي تعطيله إلى تقليل عمل التصيير الحجمي قليلًا.
properties-block-model-downscale = تصغير دقة نموذج الكتل
properties-camera = الكاميرا
properties-camera-clip-planes = طائرات لقطات الكاميرا
properties-cap-while-resizing = تحديد الحد الأقصى أثناء تغيير الحجم
properties-dark-mode = الوضع الداكن
properties-developer = المطوّر
properties-downscale-rasters = تصغير دقة البيانات النقطية
properties-edit-object = تعديل الكائن...
properties-field-view = مجال الرؤية
properties-fps = إطار/ث
properties-frame-counter = عداد الإطار
properties-frame-rate-cap = سقف سرعة الإطار
properties-hz = Hz
properties-interface = الواجهة
properties-invert-horizontal = عكس الأفقية
properties-invert-vertical = عكس عمودي
properties-limits-newly-loaded-geotiff-previews = يقصر معاينات GeoTIFF الجديدة على 4096 بكسل في أطول ضلع. عطّله للدقة الكاملة حتى حد نسيج GPU، مع استهلاك ذاكرة أكبر.
properties-line-colour = لون الخط
properties-look-sensitivity = حساسية النظر
properties-max-clip-span = أقصى طول المقطوعة
properties-move-layer = انتقل إلى الطبقة...
properties-near-clip-limit = حد القطع القريب
properties-orbit-sensitivity = حساسية المدار
properties-panel-chrome = إطار اللوحة
properties-performance = الأداء
properties-plan-mode = وضع المسقط الأفقي
properties-presents-step-display-no-tearing = يُعرض بتزامن مع الشاشة: بلا تمزق، وتحدد الشاشة معدل الإطارات. عند التعطيل، تُعرض الإطارات فور رسمها ويُطبَّق الحد الأدنى أدناه.
properties-reflective-block-edges = حواف الكتل العاكسة
properties-restore-defaults = استعادة الإعدادات الافتراضية
properties-show-console = إظهار وحدة التحكم
properties-shows-live-near-far-projection = يظهر المسافات المباشرة القريبة والبعيدة في شريط الحالة.
properties-snap-polling = استطلاع الالتقاط
properties-vertical-sync = التزامن الرأسي
properties-world-axis-gizmo = أداة المحور العالمي
properties-zoom-cursor = زوم إلى المؤشر
properties-zoom-sensitivity = حساسية الزوم

## Screenshot strings

screenshot-could-not-encode-viewport-image = تعذر ترميز صورة منفذ العرض: { $error }
screenshot-could-not-map-viewport-screenshot = تعذر تعيين لقطة منفذ العرض: { $error }
screenshot-could-not-save-viewport-image = تعذر حفظ صورة منفذ العرض { $path }: { $error }
screenshot-downloaded-viewport-image-file-name = تم تنزيل صورة منفذ العرض: { $file_name }
screenshot-saved-viewport-image-path = تم حفظ صورة منفذ العرض: { $path }
screenshot-viewport-image-download-failed-error = فشل تنزيل صورة منفذ العرض: { $error }

## Spatial strings

spatial-bvh-face-index-out-of-range = فهرس وجه BVH ‏{ $index } خارج نطاق الشبكة؛ سيُستبدل بمثلث منعدم

## State strings

state-above = في أو فوق
state-activate-project = تنشيط المشروع
state-all-open-incline-design-data = كل بيانات Incline Design المفتوحة
state-apply-generated-rings = تطبيق الحلقات المُنشأة
state-apply-selection = تطبيق على التحديد
state-rotate-by-azimuth-dip = بسمت { $azimuth }° وميل { $dip }°
state-rotate-to-azimuth-dip = إلى بسمت { $azimuth }° وميل { $dip }°
state-below = في أو أسفل
state-centre-rotation = مركز الدوران
state-checking-unsaved-work = جارٍ التحقق من العمل غير المحفوظ
state-choose-destination = اختر وجهة
state-choose-one-more-files = اختر ملفًا واحدًا أو أكثر
state-clear-raster = مسح البيانات النقطية
state-click-pit-shell-viewport = انقر على غلاف الحفرة المنجمية في منفذ العرض.
state-click-pit-stockpile-solid-viewport = انقر على مجسم الحفرة المنجمية أو كومة التخزين في منفذ العرض.
state-click-surface-viewport = انقر على السطح في منفذ العرض
state-click-topology-viewport = انقر على السطح الطبوغرافي في منفذ العرض.
state-close-project = إغلاق المشروع
state-colour-drillholes = لون ثقوب الحفر
state-copy-objects-layer = نسخ الكائنات إلى الطبقة
state-count-file-s = { $count } ملف
state-count-object-s-axis-value = { $count } كائن · { $axis } { $value }
state-count-object-s-closed = { $count } كائن · { $closed }
state-count-object-s-layer = { $count } كائن · { $layer }
state-count-object-s-weight = { $count } كائن · { $weight }
state-count-object-s-z-elevation = { $count } كائن · Z { $elevation }
state-create-point-cloud-tin = إنشاء TIN من سحابة النقاط
state-create-project = إنشاء مشروع
state-current-project = المشروع الحالي
state-cut-topology-pit-shell = قطع السطح الطبوغرافي إلى غلاف الحفرة المنجمية
state-cut-triangulation-polyline = قطع شبكة مثلثية بواسطة خط متعدد المقاطع
state-cut-triangulation-z = قطع شبكة مثلثية بواسطة Z
state-dark-mode = الوضع الداكن
state-detached = منفصل
state-disabled = معطّل
state-discard-project-changes = تجاهل تغييرات المشروع
state-discard-replace-project = تجاهل المشروع واستبداله
state-discarding-unsaved-changes = جارٍ تجاهل التغييرات غير المحفوظة
state-docked = مثبّت
state-drape-raster = إسقاط بيانات نقطية
state-drill-pattern = نمط الحفر
state-duplicate-layer = تكرار الطبقة
state-east = الشرق
state-enabled = ممكّن
state-exit-incline-design = الخروج من Incline Design
state-export-block-model-csv = تصدير نموذج الكتل إلى CSV
state-export-layer-dxf = تصدير الطبقة إلى DXF
state-export-omf = تصدير OMF
state-export-project-dxf = تصدير المشروع إلى DXF
state-export-triangulation = تصدير الشبكة المثلثية
state-export-viewport-image = تصدير صورة منفذ العرض
state-finish-closed-polyline = إنهاء الخط المتعدد المغلق
state-finish-open-polyline = إنهاء الخط المتعدد المفتوح
state-fit-extents = ملاءمة للحدود
state-fix-release-centre-both-views = تثبيت أو تحرير المركز الذي يدور حوله كلا العرضين
state-generate-contours = توليد خطوط كنتور
state-hidden = مخفي
state-import-drillholes = استيراد ثقوب الحفر
state-import-omf = استيراد OMF
state-import-point-cloud = استيراد سحابة نقطية
state-import-raster = استيراد بيانات نقطية
state-import-triangulation = استيراد شبكة مثلثية
state-insert-intersection-points = إضافة نقاط التقاطع
state-insert-points-elevation = إضافة النقاط عند الارتفاع
state-keep-inside = الإبقاء في الداخل
state-keep-outside = الإبقاء في الخارج
state-kriged-block-model = نموذج كتل مقدّر بكريغنغ
state-load-block-model = تحميل نموذج الكتل
state-load-drillholes = تحميل ثقوب الحفر
state-load-layer = تحميل الطبقة
state-load-point-cloud = تحميل سحابة نقطية
state-load-raster = تحميل البيانات النقطية
state-load-triangulation = تحميل شبكة مثلثية
state-locked-count-object-s = { $count } كائن مقفل
state-major-minor = الرئيسي { $major } · الثانوي { $minor }
state-move-axis-value = الانتقال إلى قيمة المحور
state-move-objects-layer = نقل الكائنات إلى الطبقة
state-name-count-holes = { $name } · { $count } حفرة
state-name-count-object-s = { $name } · { $count } كائن
state-name-z-min-z-max = { $name } · من { $z_min } إلى { $z_max }
state-next-edit = التعديل التالي
state-north = الشمال
state-open-containing-folder = فتح المجلد المحتوي
state-open-project = فتح مشروع
state-preserve-view-angle = الحفاظ على زاوية العرض
state-previous-edit = التعديل السابق
state-project-id = المشروع { $id }
state-remove-block-model = إزالة نموذج الكتل
state-remove-drillholes = إزالة ثقوب الحفر
state-remove-point-cloud = إزالة سحابة نقطية
state-remove-raster = إزالة بيانات نقطية
state-remove-triangulation = إزالة شبكة مثلثية
state-removed-from-active-triangulation = تمت الإزالة من التثليث النشط
state-removed-from-every-triangulation = تمت الإزالة من كل تثليث
state-rename-kind = إعادة تسمية { $kind }
state-save-close-project = حفظ وإغلاق المشروع
state-save-despite-unsupported-content = الحفظ رغم وجود محتوى غير مدعوم
state-save-project = حفظ المشروع باسم
state-save-replace-project = حفظ واستبدال المشروع
state-saving-current-project = جارٍ حفظ المشروع الحالي
state-section-name = القسم { $section }
state-select-layer-objects = اختيار كائنات الطبقة
state-selected-objects = الكائنات المحددة
state-selected-polylines = الخطوط المتعددة المحددة
state-selected-scene-elements = عناصر المشهد المحددة
state-set-block-model-variable = تعيين متغير نموذج الكتل
state-set-drillhole-colour-preset = تعيين إعداد لون ثقوب الحفر المسبق
state-set-entity-lock = تعيين قفل الكيان
state-set-grid = تعيين الشبكة
state-set-layer-lock = تعيين قفل الطبقة
state-set-line-weight = تعيين وزن الخط
state-set-object-colour = تعيين لون الكائن
state-set-object-fill = تعيين تعبئة الكائن
state-set-point-visibility = تعيين رؤية النقطة
state-set-polyline-closed = تعيين إغلاق الخط المتعدد المقاطع
state-set-raster-lock = تعيين قفل البيانات النقطية
state-set-standard-view = تعيين المشاهدة القياسية
state-set-topology-wireframes = تعيين الإطارات السلكية للسطح الطبوغرافي
state-set-triangulation-colour = تعيين لون الشبكة المثلثية
state-show-console = إظهار وحدة التحكم
state-show-project = عرض المشروع
state-shown = ظاهر
state-slice-mode = وضع الشريحة
state-slice-preview = معاينة الشريحة
state-south = الجنوب
state-stem-contours = خطوط كنتور { $stem }
state-target-new-name = { $target } إلى «{ $new_name }»
state-trim-above = قص أعلى
state-trim-below = قص أسفل
state-trim-triangulation-surface = قص الشبكة المثلثية حسب السطح
state-undrape-raster = إزالة إسقاط البيانات النقطية
state-undrape-rasters = إزالة إسقاط البيانات النقطية (متعددة)
state-unload-block-model = إلغاء تحميل نموذج الكتل
state-unload-drillholes = إلغاء تحميل ثقوب الحفر
state-unload-layer = إلغاء تحميل الطبقة
state-unload-point-cloud = إلغاء تحميل سحابة النقاط
state-unload-raster = إلغاء تحميل البيانات النقطية
state-unload-triangulation = إلغاء تحميل الشبكة المثلثية
state-untitled-project = مشروع بلا عنوان
state-use-typed-radius = استخدام نصف القطر المكتوب
state-west = الغرب

## Status strings

status-clip-near-far = المقطع قريب / بعيد / Δ: -- / -- / --
status-frame-rate = معدل الإطارات

## Text strings

text-could-not-build-vector-mesh = تعذر إنشاء شبكة متجهية للخط { $font }، الرمز { $glyph }: { $error }
text-document-text-mesh-exceeded-its = تجاوزت شبكة نص المستند نطاق فهرس u32

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = اختر مجموعة بيانات ثقوب الحفر لربطها أولًا
tie-in-count-connector-s = { $count } موصل
tie-in-delete-tie-ins = حذف التوصيلات
tie-in-deleted-count-selected-tie-connector = حُذف { $count } موصل محدد
tie-in-hole = حفرة
tie-in-initiation-point-lifted-from-name = أُزيلت نقطة البدء من { $name }
tie-in-initiation-point-set-name-delay = عُيّنت نقطة البدء على { $name } بتأخير { $delay } مللي ثانية
tie-in-select-delay-product-palette-before = اختر منتج تأخير من اللوحة قبل ربط الثقوب
tie-in-tied-connectors = رُبط { $count } موصل عند { $delay } مللي ثانية باستخدام { $product }
tie-in-tied-connectors-replacing = رُبط { $count } موصل عند { $delay } مللي ثانية باستخدام { $product }، مع استبدال { $replaced }

## Toolbar strings

toolbar-fill-type = نوع التعبئة

## Toolbars strings

toolbars-auto-bench = مصطبة تلقائية
toolbars-bezier-polyline = خط بيزيه متعدد
toolbars-chamfer-polyline-corners = شطف زوايا الخط المتعدد
toolbars-create-text = إنشاء نص
toolbars-cursor-regular = المؤشر: عادي
toolbars-cursor-snap-line = المؤشر: انجذاب إلى الخط
toolbars-cursor-snap-point = المؤشر: انجذاب إلى النقطة
toolbars-cursor-snap-surface = المؤشر: انجذاب إلى السطح
toolbars-delete-points = حذف النقاط
toolbars-explode-polyline-lines = تفكيك الخط المتعدد إلى خطوط
toolbars-fuse-polylines = دمج الخطوط المتعددة
toolbars-measure-distance = قياس المسافة
toolbars-new-layer = طبقة جديدة
toolbars-split-polyline-points = تقسيم الخط المتعدد عند النقاط
toolbars-strike-dip = الاتجاه والميل
toolbars-tool-not-available-section-view = { $tool } - غير متاحة في عرض المقطع

## Tri strings

tri-sampling-method-help = تركز الطريقة التكيفية الرؤوس على التضاريس المعقدة وفق خطأ ملاءمة المستوى؛ بينما توزعها الطريقة المنتظمة بالتساوي. قد تُضاف طرق أخرى مستقبلًا.
tri-adaptive-quadtree = تكيّفي (شجرة رباعية)
tri-axis-range = نطاق المحور { $axis }
tri-base-topology-will-receive-pit = السطح الطبوغرافي الأساسي الذي سيستقبل شكل الحفرة أو المخزون.
tri-boundary-polyline = الحدود خط متعدد المقاطع
tri-bridge-gaps-help = صِل الفجوات وتقعرات الحدود الأضيق من هذه القيمة عبر السطح. تظل القيمة 0 تصل الفجوات حتى حجم خلية أخذ العينات تقريبًا؛ وتملأ القيم الأكبر ثقوبًا أكبر وتقلل تقعرات الحدود.
tri-budget = الميزانية حسب
tri-cancel-pick = إلغاء الاختيار
tri-candidate-detail = تفاصيل المرشح
tri-candidate-fine-cells-per-budgeted = عدد الخلايا الدقيقة المرشحة لكل رأس في الميزانية. تمنح القيمة الأعلى أخذ العينات التكيفي حرية أكبر في وضع التفاصيل، لكنها أبطأ في البناء.
tri-cap-surface-share-source-points = الحد من السطح بنسبة من نقاط المصدر أو بحسب عدد محدد للقمة.
tri-choose-input-clicking-loaded-surface = اختر هذا المدخل بالنقر على السطح المحمّل في منفذ العرض
tri-choose-which-side-reference-topology = اختر أي جانب من السطح الطبوغرافي المرجعي تريد إزالته من السطح ضمن منطقة XY المشتركة بينهما.
tri-clip = المقطع
tri-clip-creates-new-triangulation-name = ينشئ القص تثليثًا جديدًا بهذا الاسم؛ ولا يتم تعديل سطح المصدر.
tri-clip-surface-polyline = قص السطح بخط متعدد المقاطع
tri-closed-pit-stockpile-solid-whose = مجسم مغلق لحفرة أو مخزون ستُضم حدوده المكشوفة إلى النتيجة.
tri-create-new-layer-contours-append = أنشئ طبقة جديدة للكنتورات أو ألحقها بطبقة موجودة في المشروع النشط.
tri-cut-topology-pit-shell = قطع السطح الطبوغرافي مع غلاف الحفرة المنجمية
tri-e-g-design-trimmed = على سبيل المثال design_trimmed
tri-e-g-mysurf-cut = على سبيل المثال mysurf_cut
tri-e-g-mysurf-slice = على سبيل المثال mysurf_slice
tri-e-g-surface-contour = على سبيل المثال surface_contour
tri-e-g-topo-cut = على سبيل المثال topo_cut
tri-e-g-topo-pit = على سبيل المثال topo_with_pit
tri-exact-number-surface-vertices-target = العدد الدقيق المستهدف لرؤوس السطح. القيم الكبيرة جدًا بطيئة البناء وتستهلك ذاكرة كبيرة.
tri-existing-ground-topology-will-cut = سطح الأرض الحالي الذي ستقصه قشرة الحفرة.
tri-fill-holes-up = ملء الثقوب حتى
tri-generate = إنشاء
tri-generate-contour-lines = توليد خطوط الكنتور
tri-generate-upper-surface = توليد السطح العلوي
tri-hide-unload-sources = إخفاء المصادر وإلغاء تحميلها
tri-higher-edge-will-enforced-each = ستُعتمد الحافة الأعلى عند كل تعارض. سيتم تجاهل المقاطع المتعارضة الأدنى كخطوط انكسار وسيُستوفى السطح عبر تلك المناطق. ولا تتغير الخطوط المتعددة المصدرية.
tri-breaklines-cross = تتقاطع حواف خطوط الانكسار المميزة أو تتداخل في المخطط عند مناسيب مختلفة. لا يمكن لسطح تضاريس واحد اتباع كليهما.
tri-intervals-colours = الفترات والألوان
tri-keep-clipped-topology-included-shape = احتفظ بالسطح الطبوغرافي المقصوص والشكل المضمّن كتثليثين منفصلين بدل دمجهما في كيان واحد.
tri-keep-inside-discards-surface-outside = الاحتفاظ بالداخل يزيل السطح خارج الخط المتعدد. والاحتفاظ بالخارج يقطع من السطح ثقبًا بشكل الخط المتعدد.
tri-keeps-only-surface-within-polyline = يحتفظ فقط بالسطح داخل حدود الخط المتعدد.
tri-keep-surface-relation-help = يُبقي السطح { $relation } السطحَ الطبوغرافي ضمن تغطيته على XY.
tri-layer-already-exists-select-above = هذه الطبقة موجودة بالفعل؛ حددها أعلاه أو اختر اسمًا آخر.
tri-limit-z-range = تحديد نطاق Z
tri-major = رئيسي
tri-max-edge-length = أقصى طول الحافة
tri-merge = دمج
tri-method = الطريقة
tri-min = أدنى
tri-minimum-maximum-elevations-retained = أدنى وأعلى منسوب يتم الاحتفاظ بهما في سطح الإخراج. يجب أن يكون الحد الأدنى أقل من الحد الأقصى.
tri-minor = ثانوي
tri-contour-interval-help = يتحكم «الثانوي» في الكنتورات العادية و«الرئيسي» في المميزة، ويجب ألا يقل فاصل الرئيسي عن الثانوي.
tri-move-cursor-over-loaded-surface = حرّك المؤشر فوق سطح محمّل.
tri-slice-output-name-help = الاسم المخصص للسطح الناتج المقطوع عند ارتفاع معيّن.
tri-name-assigned-merged-topology-pit = الاسم المخصص لنتيجة دمج السطح الطبوغرافي مع الحفرة المنجمية/كومة التخزين.
tri-name-assigned-newly-created-contour = الاسم المخصص لطبقة خطوط الكنتور التي أُنشئت حديثًا.
tri-reconstruct-output-name-help = الاسم المخصص للشبكة المثلثية التي أُعيد بناؤها.
tri-name-assigned-topology-after-pit = الاسم المخصص للسطح الطبوغرافي بعد قطع غلاف الحفرة المنجمية منه.
tri-name-assigned-trimmed-output-surface = الاسم المخصص للسطح الناتج المشذّب.
tri-nearby-breakline-vertices-do-not = لا تلتقي رؤوس خطوط الانكسار المتقاربة في الموضع نفسه تمامًا، لذلك يتعذر تثليث السطح.
tri-new-layer = طبقة جديدة
tri-new-layer-name = اسم الطبقة الجديدة
tri-once-merge-succeeds-unload-source = بعد نجاح الدمج، ألغ تحميل الطوبولوجيا والجسم المصدرين ليبقى الناتج المدمج فقط في المشهد.
tri-only-loaded-pickable = يمكن اختيار الشبكات المثلثية المحمّلة فقط.
tri-operation = العملية
tri-output-layer = طبقة الإخراج
tri-percentage = نسبة مئوية
tri-percentage-cloud = النسبة المئوية من السحابة
tri-pick-from-view = اختر من المشهد
tri-pit-design-surface-only-areas = سطح تصميم الحفرة. لا تُستخدم للقص إلا المناطق التي يحفر فيها أسفل السطح الطبوغرافي.
tri-pit-shell = قشرة الحفرة
tri-pit-stockpile-solid = مجسم الحفرة/المخزون
tri-recommended-weld-retry = موصى به: اللحام وإعادة المحاولة
tri-reconstruct-help = أعد بناء سطح تضاريس مثلث من سحابة نقاط. يخصص أخذ العينات التكيفي ميزانية الرؤوس للمناطق الأكثر تعقيدًا ويحافظ على المناطق المستوية متباعدة.
tri-reduce-budget-candidate-detail-if = قلل الميزانية أو تفاصيل المرشحين إذا كانت ذاكرة RAM في جهازك أقل.
tri-reference-topology-help = السطح الطبوغرافي المرجعي الذي يحدد موضع تشذيب السطح الآخر.
tri-reject-reconstructed-triangle-edges = ارفض حواف المثلثات المعاد بناؤها الأطول من هذه المسافة. استخدم 0 لإلغاء حد طول الحافة.
tri-remove-inside-help = يزيل السطح داخل حدود الخط المتعدد ويحتفظ بالباقي.
tri-removes-topology-where-pit-shell = يزيل السطح الطبوغرافي حيث تحفر قشرة الحفرة أسفله لكي تملأ القشرة الفجوة. يتبع الوصل خط التماس الحقيقي ثلاثي الأبعاد بين السطحين؛ ويُحتفظ بالسطح الطبوغرافي تحت أجزاء القشرة المرتفعة فوق الأرض.
tri-result = النتيجة
tri-save-two-entities = الحفظ ككيانين
tri-select = تحديد…
tri-share-source-points-keep-fractions = حصة من النقاط المصدرية يجب الاحتفاظ بها، فصائل مثل 0.125% مسموح بها
tri-slice-triangulation-z-range = تقطيع الشبكة المثلثية حسب نطاق Z
tri-solution-generate-upper-surface = الحل: توليد السطح العلوي
tri-surface-trim = السطح المراد تشذيبه
tri-target-surface-help = السطح الذي سيتم تغييره؛ ويظل السطح الطبوغرافي المحدد كما هو.
common-percent-suffix = %
tri-topology = السطح الطبوغرافي
tri-triangulation-failed = شبكة مثلثية فشل
tri-trim = قص
tri-trim-topology = قص حسب السطح الطبوغرافي
tri-uniform-grid = شبكة منتظمة
tri-up-target-point-count-points = سيصبح ما يصل إلى { $target } من أصل { $point_count } نقطة رؤوسًا للسطح ({ $percent }%).
tri-use-full-surface-elevation-range = استخدم النطاق الكامل من ارتفاع السطح
tri-vertex-count = عدد الرؤوس
tri-vertices-within-5-cm-xy = ستشترك الرؤوس التي تقع ضمن 5 سم في XY وZ في موضع واحد لهذا التثليث. قد يؤدي ذلك إلى إزاحة السطح الناتج محليًا بما يصل إلى 5 سم؛ ولا تتغير الخطوط المتعددة المصدرية.
tri-weld-retry = اللحام وإعادة المحاولة
tri-when-enabled-generate-contours-only = عند التمكين، تُنشأ خطوط الكنتور فقط بين الارتفاعين الأدنى والأقصى المحددين.

## Ui strings

ui-choose-offset-side = اختر جانب الإزاحة
ui-choose-relimit-side = اختر جانب إعادة تحديد
ui-click-circle-centre = انقر على مركز الدائرة
ui-click-closed-polyline-use-blast = انقر خطًا متعددًا مغلقًا لاستخدامه كشكل التفجير
ui-click-collar-add-edit-initiation = انقر طوقًا لإضافة نقطة بدء أو تعديلها
ui-click-first-point-slice-line = انقر على النقطة الأولى من خط الشريحة
ui-click-first-vertex = انقر على القمة الأولى
ui-click-perimeter-point-type-radius = انقر على نقطة محيطية أو اكتب نصف القطر
ui-click-second-point-slice-line = انقر على النقطة الثانية من خط الشريحة
ui-click-second-vertex = انقر على القمة الثانية
ui-click-use-pointer-radius = أو انقر لاستخدام نصف قطر المؤشر
ui-could-not-copy-text-browser = تعذر نسخ النص إلى حافظة المتصفح: { $error }
ui-dip-horizontal-no-strike = { $dip } (أفقي، دون اتجاه)
ui-distance-meters = { $distance } متر
ui-drag-ring-type-azimuth-dip = اسحب حلقة، أو أدخل البسمت والميل
ui-each-hole-turns-about-its = تدور كل حفرة حول طوقها
ui-enter-positive-decimal-radius = أدخل نصف قطر عشري إيجابي
ui-esc-cancels = يلغي Esc
ui-no-delay-product-tie = لا يوجد منتج تأخير للربط به
ui-press-enter-use-typed-radius = اضغط Enter لاستخدام نصف القطر المكتوب
ui-right-click-delay-palette-heading = انقر بزر الماوس الأيمن على عنوان لوحة التأخير لإضافة واحد
ui-select-designs = اختيار التصاميم
ui-select-drill-hole = اختر حفرة حفر
ui-select-endpoint-join = حدد النقطة النهائية للانضمام
ui-select-first-crest-toe-point = اختر أول نقطة حافة/قدم المصطبة
ui-select-item = اختيار عنصر
ui-select-line-fuse = اختر خطًا للدمج
ui-select-line-polyline = اختر خط أو خط متعدد المقاطع
ui-select-line-relimit = حدد الخط المطلوب إعادة تحديده
ui-select-next-line-fuse = اختر الخط التالي للدمج
ui-select-opposite-berm-point = حدد النقطة المقابلة لحاجز الأمان
ui-select-point = اختر نقطة
ui-select-polyline = اختر خط متعدد المقاطع
ui-select-polyline-open-line = اختر خطًا متعدد المقاطع أو خطًا مفتوحًا
ui-select-polyline-vertex = اختر قمة خط متعدد المقاطع
ui-select-second-crest-toe-point = اختر النقطة الثانية حافة/قدم المصطبة
ui-select-second-split-point = حدد نقطة الانقسام الثانية
ui-select-split-point = اختر نقطة تقسيم
ui-select-topologies = اختر الأسطح الطبوغرافية
ui-slice-view = عرض الشريحة
ui-strike-dip = اتجاه { $strike }° · { $dip }
ui-value-dip = ميل { $value }°

## Viewport strings

viewport-all-total-categories-keep-their = تحتفظ كل الفئات وعددها { $total } بألوانها؛ تُرسم أول { $shown } فئات فقط بشكل مميز
viewport-axis-maximum = الحد الأقصى لـ { $axis }
viewport-axis-minimum = الحد الأدنى لـ { $axis }
viewport-bar-blast-timeline-placeholder = الخط الزمني للتفجير [عنصر نائب]
viewport-bar-burden-relief-heatmap-placeholder = خريطة حرارية لتحرير خط المقاومة [عنصر نائب]
viewport-bar-color = لون:
viewport-bar-contours-equal-time-placeholder = خطوط الزمن المتساوي [عنصر نائب]
viewport-bar-disable-flying-mode = تعطيل وضع الطيران
viewport-bar-disable-x-ray-vision = تعطيل الرؤية بالأشعة السينية
viewport-bar-drill-holes = ثقوب الحفر:
viewport-bar-enable-flying-mode = تمكين وضع الطيران
viewport-bar-enable-x-ray-vision = تمكين الرؤية بالأشعة السينية
viewport-bar-exit-slice-view = الخروج من عرض الشريحة
viewport-bar-fill = ملء:
viewport-bar-fix-centre-rotation = تثبيت مركز الدوران
viewport-bar-hide-points = إخفاء النقاط
viewport-bar-hide-rl-grid = إخفاء شبكة المناسيب
viewport-bar-hide-wireframes = إخفاء الإطارات السلكية
viewport-bar-hide-xy-grid = إخفاء شبكة XY
viewport-bar-release-centre-rotation = تحرير مركز الدوران
viewport-bar-show-points = إظهار النقاط
viewport-bar-show-rl-grid = إظهار شبكة المناسيب
viewport-bar-show-wireframes = إظهار الإطارات السلكية
viewport-bar-show-xy-grid = إظهار شبكة XY
viewport-bar-vertical-slice-view = عرض شريحة رأسية
viewport-blank = (فارغ)
viewport-choose-active-block-model-variable = اختر متغير نموذج الكتل النشط
viewport-choose-variable = اختر متغيرًا
viewport-click-edit-color-right-click = انقر لتحرير اللون؛ انقر بزر الفأرة الأيمن للإزالة
viewport-click-type-boundary-s-value = انقر على كتابة قيمة هذا الحد
viewport-colour-mapping = خريطة الألوان
viewport-count-categories = { $count } فئات
viewport-count-category = { $count } فئة
viewport-double-click-add-boundary-here = انقر نقرًا مزدوجًا لإضافة حد هنا
viewport-drag-move-middle-click-toggles = اسحب للتحريك · النقر بالزر الأوسط يبدّل ≤
viewport-drag-move-right-click-remove = اسحب للتحريك · انقر بزر الفأرة الأيمن للإزالة · النقر بالزر الأوسط يبدّل ≤
viewport-e = ش
viewport-edit-category-colour = تحرير لون هذه الفئة
viewport-edit-colour-used-empty-values = تحرير اللون المستخدم للقيم الفارغة
viewport-empty = (فارغ)
viewport-empty-hidden = (فارغ · مخفي)
viewport-filter-variables = تصفية المتغيرات
viewport-navigation-hint = اسحب بالزر الأوسط للتحريك · مرّر للتكبير/التصغير
viewport-navigation-hint-detach = اسحب بالزر الأوسط للتحريك · مرّر للتكبير/التصغير · انقر للفصل
viewport-n = ج
viewport-no-data-variable = لا توجد بيانات لهذا المتغير
viewport-no-matches = لا تطابق
viewport-no-usable-range = (لا يوجد نطاق قابل للاستخدام)
viewport-rebuild-variable-s-colours-from = إعادة بناء ألوان هذا المتغير من بياناته
viewport-reset = إعادة تعيين
viewport-restore-full-model-range = استعادة نطاق النموذج الكامل
