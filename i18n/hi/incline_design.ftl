# हिन्दी। अनुपलब्ध प्रविष्टियाँ अंग्रेज़ी कैटलॉग से ली जाती हैं।
common-cancel = रद्द करें
common-clear = साफ़ करें
common-close = बंद करें
common-color = रंग
common-fill = भराव
common-set = सेट करें
status-language = भाषा
menu-file = फ़ाइल
menu-file-save-project = प्रोजेक्ट सहेजें
menu-file-save-project-as = प्रोजेक्ट को इस नाम से सहेजें...
menu-file-new-project = नया प्रोजेक्ट...
menu-file-open-project = प्रोजेक्ट खोलें...
menu-file-open-recent = हाल का खोलें
menu-file-import = आयात करें...
menu-file-export = निर्यात करें...
menu-file-about = { $app } के बारे में...
menu-file-exit = एप्लिकेशन से बाहर निकलें
menu-view = दृश्य
ws-production = उत्पादन
ws-drill-and-blast = ड्रिलिंग और ब्लास्टिंग
ws-geology = भूविज्ञान
ws-planning = योजना
dialog-rename-title = { $kind } का नाम बदलें
dialog-rename-field = नया नाम
dialog-rename-field-hint = आवश्यक
dialog-rename-submit = नाम बदलें
dialog-delete-title = { $kind } हटाएँ
dialog-delete-confirm = प्रोजेक्ट से “{ $name }” हटाएँ?
    यह कार्रवाई पूर्ववत नहीं की जा सकती।
confirm-delete-product =
    पैलेट से उत्पाद “{ $name }” हटाएँ?
    यह कार्रवाई पूर्ववत नहीं की जा सकती।
about-read-full-licence = पूरा लाइसेंस पढ़ें ↗
about-source-code = स्रोत कोड
about-website = वेबसाइट

## Completed canonical messages

menu-file-show-in-explorer = एक्सप्लोरर में दिखाएँ
menu-file-show-in-folder = युक्त फ़ोल्डर खोलें
menu-file-export-viewport-image = निर्यात व्यूपोर्ट छवि...
menu-file-export-engineering-drawing = निर्यात इंजीनियरिंग ड्राइंग...
ws-menubar-design = डिजाइन
ws-menubar-triangulation = त्रिभुजीकरण
ws-menubar-raster = रास्टर
ws-menubar-point-cloud = पॉइंट क्लाउड
ws-menubar-block-model = ब्लॉक मॉडल
ws-menubar-drillholes = ड्रिल होल
ws-menubar-active-layer = परतः
ws-menubar-design-insert-point = बिंदु डालें
ws-menubar-design-insert-point-at-intersection = चौराहे पर
ws-menubar-design-insert-point-at-elevation = ऊंचाई पर
ws-menubar-design-move-to = आगे बढ़ें
ws-menubar-design-create-triangulation = त्रिभुजीकरण बनाएँ
tri-create-title = त्रिभुजीकरण बनाएँ
tri-create-type-label = त्रिभुज प्रकार
tri-create-type-help = खुली सतह से इलाके की तरह एक शीट बनती है। ठोस एक पूरी तरह से बंद जाल बनाता है और इनपुट की आवश्यकता होती है जो एक जलरोधक सीमा बना सकता है।
tri-create-output-name = आउटपुट नाम
tri-create-output-name-help = उत्पन्न त्रिभुज को सौंपा गया नाम।
tri-create-output-name-hint = त्रिभुज का नाम
tri-create-run = त्रिभुज
tri-selection-selected = { $summary } चयनित
tri-type-open-surface = सतह
tri-type-solid-closed = ठोस
about-title = { $app } के बारे में
drill-hole-colour-title = रंग ड्रिल छेद: { $name }
drill-hole-colour-stop = { $index } को रोकें
properties-restore-defaults = { $heading } सेटिंग्स को उनके डिफ़ॉल्ट पर रीसेट करें
ui-selected-count = { $count } चयनित
ui-selected-objects = { $count } ऑब्जेक्ट चयनित
ui-selected-polylines = { $count } पॉलीलाइन चयनित
ui-invalid-axis-value = एक वैध { $axis } मान दर्ज करें।
ui-selection-spans = चयन { $min } से { $max } तक होता है।
confirm-delete-count = क्या आप सुनिश्चित हैं कि आप { $count } चयनित आइटम को हटाना चाहते हैं?
confirm-delete-layer = '{ $name }' परत और उस पर सभी वस्तुओं को हटाना? ऐसा नहीं किया जा सकता।
plot-preview-pixels = { $width } × { $height } px { $dpi } dpi पर
tri-estimated-memory = अनुमानित पीक मेमोरी ~{ $estimate }. { $detail }
block-grid-summary = ग्रिडः { $x } × { $y } × { $z } = { $count } ब्लॉक
status-selected = चयनित: { $count }
status-fps = FPS: { $fps }
status-clip = क्लिप पास/दूर/Δ: { $near } / { $far } / { $delta } m

## Selection counts

tri-count-polylines =
    { $count ->
        [one] { $count } पॉलीलाइन
       *[other] { $count } पॉलीलाइन
    }
tri-count-strings =
    { $count ->
        [one] { $count } रेखा
       *[other] { $count } रेखाएँ
    }
tri-count-points =
    { $count ->
        [one] { $count } बिंदु
       *[other] { $count } बिंदु
    }
tri-count-texts =
    { $count ->
        [one] { $count } टेक्स्ट ऑब्जेक्ट
       *[other] { $count } टेक्स्ट ऑब्जेक्ट
    }
tri-count-objects =
    { $count ->
        [one] { $count } ऑब्जेक्ट
       *[other] { $count } ऑब्जेक्ट
    }

## Reused existing project translations

## स्वयं पूर्ण किए गए इंटरफ़ेस अनुवाद
explorer-no-rasters = कोई रास्टर नहीं
slice-viewport-gestures = मध्य बटन खींचें: पैन · दायाँ बटन खींचें: ऑर्बिट · Shift+व्हील: चलना · W/S: स्लैब हिलाएँ · Q/E: घुमाएँ · Esc: बाहर निकलें

## प्रारंभिक परिवेश विवरण

## रेंडरर स्टार्टअप निदान

color-aci = ACI
color-aci-value = ACI { $index }
color-index = इंडेक्स
color-rgb = RGB
color-opacity = अपारदर्शिता
color-edit = रंग संपादित करने के लिए क्लिक करें
color-saturation-value = संतृप्ति और चमक
color-hue = ह्यू
asset-loading = एसेट डेटा लोड हो रहा है
asset-unloading = एसेट डेटा अनलोड हो रहा है
asset-load-failed = एसेट डेटा लोड नहीं हो सका
asset-unload-failed = एसेट डेटा अनलोड नहीं हो सका
preferences-title = प्राथमिकताएँ
context-text-colour = टेक्स्ट रंग
context-polylines = पॉलीलाइनें
context-points = बिंदु
crs-unknown-ellipsoid = इस निर्देशांक तंत्र परिभाषा में अपरिचित पृथ्वी मॉडल “{ $name }”।
crs-no-ellipsoid = यह निर्देशांक तंत्र परिभाषा नहीं बताती कि वह किस पृथ्वी मॉडल का उपयोग करती है।
crs-unknown-code = EPSG:{ $code } निर्देशांक तंत्र रजिस्ट्री में नहीं है।
crs-transform-failed = एक निर्देशांक परिवर्तित नहीं किया जा सका; परिणाम एक परिमित स्थिति नहीं थी।
crs-no-datum-path = { $from } और { $to } (EPSG डेटम { $source } और { $target }) के संदर्भ फ़्रेमों के बीच कोई प्रकाशित रूपांतरण उपलब्ध नहीं है। फिर भी परिवर्तित करना एक अज्ञात मात्रा से गलत होगा, इसलिए कुछ भी नहीं बदला गया।
crs-unknown-datum = { $from } या { $to } का संदर्भ फ़्रेम पहचाना नहीं जा सकता, और दोनों अलग-अलग पृथ्वी मॉडल का उपयोग करते हैं। उनके बीच परिवर्तित करना एक अज्ञात मात्रा से गलत होगा।
ws-survey = सर्वेक्षण
survey-count-designs = { $count } { $count ->
    [one] डिज़ाइन
   *[other] डिज़ाइन
  }
survey-count-meshes = { $count } { $count ->
    [one] त्रिभुजीकरण
   *[other] त्रिभुजीकरण
  }
survey-count-models = { $count } { $count ->
    [one] ब्लॉक मॉडल
   *[other] ब्लॉक मॉडल
  }
survey-count-clouds = { $count } { $count ->
    [one] पॉइंट क्लाउड
   *[other] पॉइंट क्लाउड
  }
survey-count-holes = { $count } { $count ->
    [one] ड्रिल होल डेटासेट
   *[other] ड्रिल होल डेटासेट
  }
survey-count-rasters = { $count } { $count ->
    [one] रास्टर
   *[other] रास्टर
  }
survey-unsupported = रास्टर इस रूपांतरण द्वारा परिवर्तित नहीं किए जा सकते। वे व्यूपोर्ट में चयन योग्य नहीं हैं, इसलिए चयन में कुछ भी प्रभावित नहीं होता।
survey-angle = Z के चारों ओर घूर्णन (वामावर्त)
survey-scale = एकसमान XYZ स्केल कारक
survey-invalid-transform = मूल बिंदु, कोण और परिणामी निर्देशांक परिमित होने चाहिए।
survey-invalid-scale = स्केल एक परिमित धनात्मक संख्या होनी चाहिए जिसका व्युत्क्रम भी परिमित हो।
survey-empty-selection = परिवर्तित करने के लिए कम से कम एक समर्थित आइटम चुनें।
survey-unavailable = चयनित आइटम गायब है या लोड नहीं है। परिवर्तित करने से पहले इसे लोड करें।
survey-wrong-project = केवल सक्रिय प्रोजेक्ट से डिज़ाइन चुनें।
survey-name-required = निर्देशांक तंत्र का नाम दर्ज करें।
survey-working = चयनित डेटा परिवर्तित किया जा रहा है…
survey-completed = { $items } को यथास्थान परिवर्तित किया गया। पूर्ववत करने पर वे पुनर्स्थापित हो जाएँगे।
survey-failed = परिवर्तन विफल: { $error }
survey-stale = सक्रिय प्रोजेक्ट या स्रोत डेटा बदल जाने के कारण परिवर्तन छोड़ दिया गया। स्रोत डेटा चुनें और फिर से प्रयास करें।
survey-coordinates-menu = निर्देशांक
survey-definitions-action = परिभाषाएँ…
survey-transform-action = परिवर्तित करें…
survey-definitions-title = निर्देशांक परिभाषाएँ
survey-transform-title = निर्देशांक परिवर्तित करें
survey-new-system = नया निर्देशांक तंत्र
survey-new-system-name = निर्देशांक तंत्र
survey-set-local = खान निर्देशांक तंत्र के रूप में सेट करें
survey-delete-system = निर्देशांक तंत्र हटाएँ
survey-systems-empty = कोई निर्देशांक तंत्र नहीं
survey-system-section = खान ग्रिड परिभाषा
survey-reference-note = वह फ़्रेम जिसके सापेक्ष हर परिभाषा लिखी जाती है: वे निर्देशांक जो आपका डेटा आयात होते समय पहले से रखता है। इसके अपने कोई पैरामीटर नहीं हैं। किसी तंत्र को खान निर्देशांक तंत्र बनाने के लिए उस पर राइट-क्लिक करें, या नया परिभाषित करने के लिए नीचे की खाली जगह पर।
survey-system-name = नाम
survey-reference-system = संदर्भ तंत्र
survey-reference-origin = ज्ञात बिंदु — संदर्भ निर्देशांक
survey-system-origin = वही बिंदु — तंत्र निर्देशांक
survey-angle-help = ऊपर से देखने पर संदर्भ X से संदर्भ Y की ओर वामावर्त।
survey-scale-help = संदर्भ फ़्रेम से इस तंत्र तक एकसमान XYZ स्केल। आयाम बनाए रखने के लिए 1 का उपयोग करें।
survey-close = बंद करें
survey-from = से
survey-to = तक
survey-transform-button = परिवर्तित करें
survey-swap = अदला-बदली करें
survey-drape-note = परिवर्तित सतहों से ढकी हुई इमेजरी हटा दी जाती है और उसे फिर से ढकना होगा।
survey-needs-grid-block-model = ब्लॉक मॉडल कोशिकाओं की एक नियमित ग्रिड है, और प्रोजेक्शन या संदर्भ फ़्रेम बदलने से यह नियमितता बनी नहीं रहती। इसे परिवर्तित करने का अर्थ होगा हर कोशिका को नई ग्रिड में फिर से नमूना लेना और उसमें रखे मान खोना, इसलिए इसे अपरिवर्तित छोड़ दिया गया।
survey-needs-grid-raster = एक रास्टर को एक एफ़ाइन मानचित्रण द्वारा दुनिया में रखा जाता है, जिसे प्रोजेक्शन या संदर्भ फ़्रेम का परिवर्तन बनाए नहीं रख सकता। इसे परिवर्तित करने का अर्थ होगा इमेज को फिर से नमूना लेना, इसलिए इसे अपरिवर्तित छोड़ दिया गया।
survey-conversion-exact = सटीक: केवल ग्रिड परिवर्तन, कोई पुनः प्रोजेक्शन नहीं।
survey-conversion-accuracy = बताई गई सटीकता { $accuracy } मी।
survey-kind = प्रकार
survey-axis-names = अक्ष नाम
survey-axis-help = यदि X, Y और Z नहीं हैं, तो यह तंत्र अपनी अक्षों को क्या कहता है — खान ग्रिड के लिए “पू”, “उ”, “RL”। यह हर जगह उपयोग होता है जहाँ निर्देशांक दिखाए जाते हैं, लेकिन केवल तब तक जब तक यह खान निर्देशांक तंत्र है। तीनों नाम दें या कोई नहीं।
survey-kind-registry-short = रजिस्ट्री तंत्र
survey-kind-grid-short = किसी अन्य तंत्र पर ग्रिड
survey-registry-search = खोजें
survey-registry-hint = नाम या EPSG कोड, जैसे “mga zone 56”
survey-registry-none = रजिस्ट्री में सभी शब्दों से मेल खाने वाला कुछ नहीं है।
survey-parent = इसके सापेक्ष परिभाषित
survey-parent-origin = ज्ञात बिंदु — मूल तंत्र निर्देशांक
survey-pick-registry = तंत्र खोजें और परिणामों में से चुनें।
survey-pick-parent = वह तंत्र चुनें जिसके सापेक्ष यह ग्रिड परिभाषित है।
survey-pick-system = एक तंत्र चुनें
survey-pick-systems = परिवर्तन के लिए स्रोत और गंतव्य तंत्र चुनें।
survey-no-selection = बाईं ओर एक निर्देशांक तंत्र चुनें, या एक जोड़ने के लिए राइट-क्लिक करें।
survey-kind-grid = { $parent } पर ग्रिड
survey-system-in-use = “{ $name }” को हटाया नहीं जा सकता: इसके सापेक्ष { $dependants } { $dependants ->
    [one] तंत्र
   *[other] तंत्र
  } परिभाषित हैं। पहले उन्हें कहीं और इंगित करें।
survey-system-cycle = “{ $name }” प्रत्यक्ष रूप से या अपने मूल तंत्रों के माध्यम से स्वयं के सापेक्ष परिभाषित है।
survey-system-missing = वह निर्देशांक तंत्र अब मौजूद नहीं है। कोई अन्य परिभाषा चुनें।
survey-same-system = अलग-अलग स्रोत और गंतव्य तंत्र चुनें।
survey-name-exists = इस नाम का एक निर्देशांक तंत्र पहले से मौजूद है। संपादित करने के लिए उसे चुनें, या कोई अन्य नाम चुनें।

## About strings

about-copyright-c-2026-leo-timmins =
    Copyright (c) 2026 Leo Timmins, Lucas Timmins, and Incline Design contributors. Permission is hereby granted, free of charge, to any person obtaining a copy of this software to deal in it without restriction, subject to the conditions of the MIT License.

    Incline Design is provided "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, including but not limited to the warranties of MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE and NONINFRINGEMENT.
about-free-open-source-mine-design = मुक्त और मुक्त-स्रोत खदान डिज़ाइन
about-licensed-under-mit-license = MIT लाइसेंस के अंतर्गत

## App strings

app-activated-browser-project-name = ब्राउज़र परियोजना '{ $name }' सक्रिय की गई।
app-browser-project-deletion-failed-erro = ब्राउज़र परियोजना हटाना विफल रहा: { $error }
app-browser-project-no-longer-exists = वह ब्राउज़र परियोजना अब मौजूद नहीं है
app-browser-save-failed-error = ब्राउज़र में सहेजना विफल रहा: { $error }
app-could-not-activate-browser-project = ब्राउज़र परियोजना सक्रिय नहीं की जा सकी: { $error }
app-could-not-delete-browser-project = ब्राउज़र परियोजना हटाई नहीं जा सकी: { $error }
app-could-not-load-browser-project = ब्राउज़र परियोजना लोड नहीं की जा सकी: { $error }
app-could-not-restore-browser-project = ब्राउज़र परियोजना पुनर्स्थापित नहीं की जा सकी: { $error }
app-deleted-browser-project = ब्राउज़र परियोजना हटाई गई
app-failed-create-window-error = विंडो नहीं बनाई जा सकी: { $error }
app-failed-create-window-icon-error = विंडो आइकन नहीं बनाया जा सका: { $error }
app-failed-detach-top-down-preview = टॉप-डाउन पूर्वावलोकन अलग नहीं किया जा सका: { $error }
app-failed-initialize-graphics-error = ग्राफ़िक्स आरंभ नहीं किया जा सका: { $error }
app-failed-load-browser-preferences-erro = ब्राउज़र प्राथमिकताएँ लोड नहीं की जा सकीं: { $error }
app-failed-load-config-file-error = कॉन्फ़िगरेशन फ़ाइल लोड नहीं की जा सकी: { $error }
app-failed-load-session-file-error = सत्र फ़ाइल लोड नहीं की जा सकी: { $error }
app-failed-rasterize-window-icon-error = विंडो आइकन रैस्टराइज़ नहीं किया जा सका: { $error }
app-failed-save-browser-session-error = ब्राउज़र सत्र सहेजा नहीं जा सका: { $error }
app-failed-save-session-error = सत्र सहेजा नहीं जा सका: { $error }
app-saved-name-browser-storage = '{ $name }' ब्राउज़र संग्रहण में सहेजा गया

## Block strings

block-model-between = बीच में
block-model-block-grid = ब्लॉक ग्रिड
block-model-block-size = ब्लॉक आकार
block-model-choose-numeric-variable = कोई संख्यात्मक चर चुनें
block-model-choose-numeric-variables = संख्यात्मक चर चुनें
block-model-count-variables-selected = { $count } चर चुने गए
block-model-estimate-variables = अनुमानित चर
block-model-full-x-y-z-dimensions = प्रत्येक ब्लॉक के पूर्ण X, Y और Z आयाम। छोटे ब्लॉक विवरण, गणना समय और मेमोरी उपयोग बढ़ाते हैं।
block-model-grid-bounds-block-sizes-invalid = ग्रिड सीमाएँ या ब्लॉक आकार अमान्य हैं।
block-model-lower-x-y-z-edges = ब्लॉक मॉडल आयतन की निचली X, Y और Z सीमाएँ। ब्लॉक केंद्र इन सीमाओं के भीतर आधे ब्लॉक से शुरू होते हैं।
block-model-maximum = अधिकतम
block-model-maximum-nearest-samples-used-each = प्रत्येक ब्लॉक के लिए अधिकतम निकटतम सैंपल। कम मान तेज़ हैं; अधिक मान अनुमान को स्मूद कर गणना समय बढ़ा सकते हैं।
block-model-maximum-samples = अधिकतम नमूने
block-model-minimum = न्यूनतम
block-model-minimum-nearby-samples-required-esti = ब्लॉक का अनुमान लगाने के लिए आवश्यक न्यूनतम निकटवर्ती सैंपल। खोज त्रिज्या में कम सैंपल वाले ब्लॉक खाली रहते हैं।
block-model-minimum-samples = न्यूनतम नमूने
block-model-nugget = नगेट प्रभाव
block-model-numeric-interval-fields-interpolate = प्रक्षेपित किए जाने वाले संख्यात्मक अंतराल फ़ील्ड। प्रत्येक चुना फ़ील्ड एक ब्लॉक मॉडल चर बनता है।
block-model-ordinary-kriging-estimates-numeric-d = साधारण क्रिगिंग गोलाकार वैरियोग्राम से प्रत्येक ब्लॉक केंद्र पर संख्यात्मक ड्रिल होल अंतरालों का अनुमान लगाती है।
block-model-partial-sill = आंशिक सिल
block-model-range-search-radius = रेंज / खोज त्रिज्या
block-model-samples-farther-than-distance-exclud = इस दूरी से अधिक दूरी के नमूने को बाहर रखा जाता है; इस सीमा पर सह-परिवर्तन शून्य तक पहुंचता है।
block-model-select-all = सभी चुनें
block-model-spatially-correlated-variance-contri = गोलाकार मॉडल से मिला स्थानिक सहसंबद्ध विचरण। नगेट के साथ यह शून्य दूरी पर सहप्रसरण तय करता है।
block-model-spherical-variogram-search = गोलाकार वैरियोग्राम और खोज
block-model-threshold = <= सीमा
block-model-threshold-2 = >= सीमा
block-model-threshold-min = सीमा / मिनट
block-model-upper-x-y-z-extent = शामिल की जाने वाली ऊपरी X, Y और Z सीमा। विस्तार ब्लॉक आकार का सटीक गुणज न हो तो अंतिम ब्लॉक इस सीमा से आगे जा सकता है।
block-model-variable = चर
block-model-variance-effectively-zero-separation = मापन त्रुटि या सैंपलिंग स्केल से नीचे के बदलाव से लगभग शून्य दूरी पर होने वाला विचरण। नगेट प्रभाव न चाहिए तो शून्य रखें।
block-model-volume-cache-block-volume-usage-feedback-readback = ब्लॉक आयतन उपयोग फ़ीडबैक रीडबैक डिस्कनेक्ट हो गया
block-model-volume-cache-block-volume-usage-feedback-readback-2 = ब्लॉक आयतन उपयोग फ़ीडबैक रीडबैक विफल रहा: { $error }
block-model-x = X
block-model-y = Y
block-model-z = Z

## Canvas strings

canvas-not-selectable-choose-closed-polylin = चयन योग्य नहीं | बंद पॉलीलाइन चुनें
canvas-polyline-layer-layer-count-vertices = पॉलीलाइन | परत: { $layer } | { $count } शीर्ष
canvas-surface-name = सतह | { $name }
canvas-trimmed = छाँटा हुआ

## Cmd strings

cmd-batter-berm-created-batter-berm-from-object = ऑब्जेक्ट { $object_id } से बैटर बर्म बनाया गया
cmd-bezier-replaced-polyline-span-first-last = पॉलीलाइन विस्तार { $first }→{ $last } को { $count } नमूना मध्यवर्ती बिंदुओं से बदला गया
cmd-bezier-vertices-first-last = शीर्ष { $first } से { $last }
cmd-block-model-block-model-loader-disconnected-path = { $path } के लिए ब्लॉक मॉडल लोडर डिस्कनेक्ट हो गया
cmd-block-model-block-model-path-has-count = ब्लॉक मॉडल { $path } में असमर्थित प्रकार के { $count } वेरिएबल हैं जिन्हें पढ़ा नहीं जा सकेगा: { $names }
cmd-block-model-building-ore-mesh = अयस्क मेश बनाया जा रहा है…
cmd-block-model-could-not-create-block-model = ब्लॉक मॉडल नहीं बनाया जा सका: { $error }
cmd-block-model-could-not-decode-block-model = ब्लॉक मॉडल रंग चर '{ $variable }' को डिकोड नहीं किया जा सका: { $error }
cmd-block-model-created-block-model-name-ordinary = साधारण क्रिगिंग द्वारा ब्लॉक मॉडल '{ $name }' बनाया गया
cmd-block-model-failed-load-block-model-error = ब्लॉक मॉडल लोड नहीं हुआ: { $error }
cmd-block-model-generated-ore-mesh-from-block = ब्लॉक मॉडल '{ $name }' से अयस्क मेश बनाया गया
cmd-block-model-imported-block-model-source-path = ब्लॉक मॉडल स्रोत { $path } आयात किया गया
cmd-block-model-loaded-block-model-name-blocks = ब्लॉक मॉडल “{ $name }” लोड हुआ: { $blocks } ब्लॉक ({ $renderable } रेंडर योग्य), ग्रिड { $dimx }×{ $dimy }×{ $dimz }, { $variables } वेरिएबल
cmd-block-model-loading-name = { $name } लोड किया जा रहा है
cmd-block-model-loading-name-2 = { $name } लोड हो रहा है…
cmd-chamfer-chamfered-corner-corner-radius-radiu = कोने { $corner } को त्रिज्या { $radius } और { $segments } खंडों से चैम्फर किया गया
cmd-chamfer-radius-radius = त्रिज्या { $radius }
cmd-commands-clipped = क्लिप किया हुआ
cmd-commands-command-failed-error = कमांड विफल रही: { $error }
cmd-commands-select-one-more-objects-before = { $axis } सेट करने से पहले एक या अधिक ऑब्जेक्ट चुनें
cmd-commands-sliced = काटा हुआ
cmd-contours-contour-generation-failed-error = समोच्च रेखा बनाना विफल रहा: { $error }
cmd-contours-contours-name-were-discarded-layer = “{ $name }” की कंटूर लाइनें हटाई गईं: लेयर “{ $layer_name }” पहले से मौजूद है
cmd-contours-contours-name-were-discarded-project = '{ $name }' की समोच्च रेखाएँ त्यागी गईं: प्रोजेक्ट बंद था
cmd-contours-contours-name-were-discarded-selecte = '{ $name }' की समोच्च रेखाएँ त्यागी गईं: चुनी गई आउटपुट लेयर हटा दी गई थी
cmd-contours-generated-line-count-contour-polylin = त्रिभुजन “{ $name }” के लिए लेयर “{ $layer_name }” में { $line_count } कंटूर पॉलीलाइन बनीं
cmd-creation-assembled-assembled-count-closed-bou = खंडित खुली स्ट्रिंग से { $assembled_count } बंद सीमा रिंग जोड़ी गईं
cmd-creation-created-triangulation-from-boundary = { $boundary_count } सीमा रिंग और { $constraint_count } खुली बाधाओं से { $surface_type } प्रकार का त्रिभुजन बनाया गया
cmd-creation-creating-triangulation = त्रिभुजीकरण बनाया जा रहा है…
cmd-creation-generate-upper-surface-ignored-count = ऊपरी सतह बनाएँ: { $count } परस्पर-विरोधी निचले ब्रेकलाइन सेगमेंट छोड़े गए; स्रोत ऑब्जेक्ट अपरिवर्तित हैं
cmd-creation-ignored-rejected-non-polyline-degene = त्रिभुजन के दौरान { $rejected } गैर-पॉलीलाइन या विकृत ऑब्जेक्ट छोड़े गए
cmd-creation-weld-retry-moved-coarse-welded = वेल्ड और पुनः प्रयास: { $coarse_welded } वर्टेक्स साझा स्थानों पर ले जाए गए ({ $coarse_weld_tol } मीटर तक); स्रोत ऑब्जेक्ट अपरिवर्तित हैं
cmd-creation-welded-welded-breakline-vertex-verti = सहनशीलता के भीतर मिले { $welded } ब्रेकलाइन शीर्ष जोड़े गए
cmd-cuts-clipped-surface-name-polyline-mode = सतह '{ $name }' को पॉलीलाइन से क्लिप किया गया ({ $mode })
cmd-cuts-clipping-surface-polyline = सतह को पॉलीलाइन से क्लिप किया जा रहा है…
cmd-cuts-cut-topology-name-pit-shell = स्थलाकृतिक सतह '{ $name }' को पिट शेल के अनुसार काटा गया
cmd-cuts-cut-triangulation-name-z-band = त्रिभुजीकरण '{ $name }' को Z बैंड [{ $min }, { $max }] के अनुसार काटा गया
cmd-cuts-cutting-topology-pit-shell = स्थलाकृतिक सतह को पिट शेल के अनुसार काटा जा रहा है…
cmd-cuts-cutting-triangulation-z = त्रिभुजीकरण को Z के अनुसार काटा जा रहा है…
cmd-cuts-ignored-count-vertical-degenerate-re = XY क्षेत्रफल रहित { $count } ऊर्ध्व या विकृत संदर्भ स्थलाकृतिक सतह फ़ेस छोड़े गए
cmd-cuts-site-skipped-constraint-from-x = { $site }: बाधा ({ $from_x }, { $from_y }) → ({ $to_x }, { $to_y }) छोड़ी गई जिसे त्रिभुजक विभाजित नहीं कर सका
cmd-cuts-site-skipped-skipped-near-degenerate = { $site }: { $skipped } लगभग-विकृत बाधा किनारे छोड़े गए; उनके पास कट सीमा में बहुत मामूली अंतर हो सकता है
cmd-cuts-trimmed-surface-surface-topology-top = सतह '{ $surface }' को स्थलाकृतिक सतह '{ $topology }' के अनुसार छाँटा गया ({ $mode })
cmd-cuts-trimming-surface-topology = सतह को स्थलाकृतिक सतह के अनुसार छाँटा जा रहा है…
cmd-drape-draped-intersected-vertices-changed = { $intersected } शीर्ष ड्रेप किए गए; { $changed } की ऊँचाई बदली
cmd-drape-none-selected-design-vertices-inters = चुने गए डिज़ाइन शीर्षों में से कोई भी चुनी गई स्थलाकृतिक सतहों को नहीं काटता
cmd-drape-objects-changed-object-s-changed = { $objects } ऑब्जेक्ट बदले · { $intersected } प्रतिच्छेदी शीर्षों में से { $changed } स्थानांतरित हुए
cmd-drape-select-one-more-design-objects = ड्रेप करने के लिए एक या अधिक डिज़ाइन ऑब्जेक्ट चुनें
cmd-drape-select-one-more-topologies-drape = ड्रेप करने हेतु एक या अधिक स्थलाकृतिक सतहें चुनें
cmd-drape-selected-topologies-no-longer-loaded = चुनी गई स्थलाकृतिक सतहें अब लोड नहीं हैं
cmd-drill-hole-drill-pattern-too-large-contains = ड्रिल पैटर्न बहुत बड़ा है या अमान्य कॉलर निर्देशांक रखता है
cmd-drill-hole-enter-name-drill-pattern = ड्रिल पैटर्न का नाम दर्ज करें
cmd-drill-hole-failed-load-drillholes-error = ड्रिल होल लोड नहीं हुए: { $error }
cmd-drill-hole-hole-depth-must-greater-than = छेद की गहराई शून्य से अधिक होनी चाहिए
cmd-drill-hole-hole-diameter-must-greater-than = छेद का व्यास शून्य से अधिक होना चाहिए
cmd-drill-hole-loaded-drillhole-dataset-name-holes = ड्रिलहोल डेटासेट “{ $name }” लोड हुआ: { $holes } होल, { $fields } रंग फ़ील्ड
cmd-drill-hole-pattern-contains-no-holes = पैटर्न में कोई छेद नहीं है
cmd-explode-count-line-s = { $count } रेखाएँ
cmd-explode-explode-polyline = पॉलीलाइन विस्फोटित करें
cmd-explode-exploded-polyline-into-count-line = पॉलीलाइन को { $count } रेखाखंडों में विस्फोटित किया गया
cmd-file-block-model-csv-encoding-failed = ब्लॉक मॉडल CSV एन्कोडिंग विफल रही: { $error }
cmd-file-block-model-csv-export-failed = ब्लॉक मॉडल CSV निर्यात विफल रहा: { $error }
cmd-file-browser-recovery-files-unavailable-s = ब्राउज़र पुनर्प्राप्ति फ़ाइलें उपलब्ध नहीं हैं; सहेजी गई परियोजनाएँ IndexedDB में बनी रहेंगी
cmd-file-closed-project-runtime-id-runtime = रनटाइम ID { $runtime_id } वाला प्रोजेक्ट बंद किया गया
cmd-file-could-not-create-new-project = नया प्रोजेक्ट नहीं बनाया जा सका: { $error }
cmd-file-could-not-finish-pending-project = लंबित प्रोजेक्ट क्रिया पूरी नहीं की जा सकी: { $error }
cmd-file-could-not-finish-saving-before = बाहर निकलने से पहले सहेजना पूरा नहीं किया जा सका: { $error }
cmd-file-could-not-open-browser-project = ब्राउज़र प्रोजेक्ट नहीं खोला जा सका: { $error }
cmd-file-could-not-open-path-error = { $path } नहीं खोला जा सका: { $error }
cmd-file-could-not-read-selected-file = चुनी गई फ़ाइल नहीं पढ़ी जा सकी: { $error }
cmd-file-could-not-reload-layer-from = लेयर को डिस्क से पुनः लोड नहीं किया जा सका: { $error }
cmd-file-could-not-reload-project-from = प्रोजेक्ट को डिस्क से पुनः लोड नहीं किया जा सका: { $error }
cmd-file-could-not-remove-browser-project = ब्राउज़र प्रोजेक्ट हटाया नहीं जा सका: { $error }
cmd-file-could-not-restore-layer-from = प्रोजेक्ट से लेयर पुनर्स्थापित नहीं की जा सकी: { $error }
cmd-file-could-not-snapshot-dirty-project = पुनर्प्राप्ति के लिए बदली हुई परियोजना का स्नैपशॉट नहीं लिया जा सका: { $error }
cmd-file-could-not-start-browser-export = ब्राउज़र निर्यात शुरू नहीं किया जा सका: { $error }
cmd-file-could-not-write-recovery-copies = पुनर्प्राप्ति प्रतियाँ नहीं लिखी जा सकीं: { $error }
cmd-file-created-new-browser-project = नया ब्राउज़र प्रोजेक्ट बनाया गया
cmd-file-created-new-project = नया प्रोजेक्ट बनाया गया
cmd-file-description-download-failed-error = { $description } डाउनलोड विफल रहा: { $error }
cmd-file-discard-was-cancelled-because-projec = परिवर्तन त्यागना रद्द हुआ क्योंकि OMF पुनः लोड होते समय प्रोजेक्ट बदल गया
cmd-file-discarded-changes-layer-target-name = लेयर '{ $target_name }' के परिवर्तन त्यागे गए
cmd-file-discarded-changes-reloaded-path = परिवर्तन त्यागे गए: { $path } पुनः लोड किया गया
cmd-file-downloaded-description-file-name = { $description } डाउनलोड हुआ: { $file_name }
cmd-file-dxf-download-encoding-failed-error = DXF डाउनलोड एन्कोडिंग विफल रही: { $error }
cmd-file-dxf-import-failed-error = DXF आयात विफल रहा: { $error }
cmd-file-encoding-block-model-csv-download = ब्लॉक मॉडल CSV डाउनलोड एन्कोड किया जा रहा है…
cmd-file-encoding-dxf-download = DXF डाउनलोड एन्कोड किया जा रहा है…
cmd-file-encoding-triangulation-download = त्रिभुजीकरण डाउनलोड एन्कोड किया जा रहा है…
cmd-file-exit-deferred-until-background-expor = पृष्ठभूमि निर्यात पूर्ण होने तक बाहर निकलना स्थगित है
cmd-file-exit-requested-no-unsaved-changes = बिना सहेजे गए परिवर्तनों के बाहर निकलने का अनुरोध किया गया
cmd-file-exported-block-model-csv-path = ब्लॉक मॉडल CSV को { $path } में निर्यात किया गया
cmd-file-exported-description-dxf-path = { $description } को DXF में निर्यात किया गया: { $path }
cmd-file-exported-triangulation-name-path = त्रिकोणन '{ $name }' को { $path } में निर्यात किया गया
cmd-file-exporting-name = { $name } निर्यात किया जा रहा है…
cmd-file-exporting-triangulation-name-path = त्रिकोणन '{ $name }' को { $path } में निर्यात किया जा रहा है
cmd-file-fatal-renderer-failure-reason = रेंडरर में गंभीर विफलता: { $reason }
cmd-file-file-dialog-action-failed-msg = फ़ाइल संवाद क्रिया विफल रही: { $msg }
cmd-file-imported-added-object-s-from = { $name } से { $added } ऑब्जेक्ट आयात किए गए
cmd-file-imported-total-dxf-object-s = { $total } DXF ऑब्जेक्ट आयात किए गए
cmd-file-layer-discard-was-cancelled-because = लेयर परिवर्तन त्यागना रद्द हुआ क्योंकि प्रोजेक्ट पुनः लोड होते समय बदल गया
cmd-file-no-recovery-directory-available-erro = कोई पुनर्प्राप्ति डायरेक्टरी उपलब्ध नहीं है: { $error }
cmd-file-no-unsaved-project-content-nothing = कोई न सहेजी गई परियोजना सामग्री नहीं है; पुनर्प्राप्त करने के लिए कुछ नहीं
cmd-file-parsing-browser-dxf-import = ब्राउज़र DXF आयात पार्स किया जा रहा है…
cmd-file-parsing-dxf-import = DXF आयात पार्स किया जा रहा है…
cmd-file-project-will-close-after-its = वर्तमान सहेजना पूरा होने के बाद प्रोजेक्ट बंद होगा
cmd-file-project-will-close-after-its-2 = वर्तमान सहेजना पूरा होने के बाद प्रोजेक्ट बंद होगा
cmd-file-queued-count-triangulation-file-s = { $count } त्रिकोणन फ़ाइलें आयात कतार में जोड़ी गईं
cmd-file-recovery-copies-path-reopen-them = पुनर्प्राप्ति प्रतियाँ { $path } में हैं; पुनः आरंभ करने के बाद उन्हें फिर खोलें
cmd-file-recovery-copy-failed-error = पुनर्प्राप्ति प्रति विफल रही: { $error }
cmd-file-recovery-copy-failed-failure = पुनर्प्राप्ति प्रति विफल रही: { $failure }
cmd-file-recovery-copy-written-path = पुनर्प्राप्ति प्रति लिखी गई: { $path }
cmd-file-reverting-layer = परत वापस की जा रही है…
cmd-file-reverting-project = परियोजना वापस की जा रही है…
cmd-file-save-failed-message = सहेजना विफल रहा: { $message }
cmd-file-save-worker-ended-without-result = सहेजने की प्रक्रिया बिना परिणाम के समाप्त हुई
cmd-file-saved-project-path = प्रोजेक्ट इस रूप में सहेजा गया: { $path }
cmd-file-saved-project-path-2 = प्रोजेक्ट सहेजा गया: { $path }
cmd-file-selected-block-model-no-longer = चुना गया ब्लॉक मॉडल अब लोड नहीं है
cmd-file-switching-project = परियोजना बदली जा रही है…
cmd-file-triangulation-download-encoding-fail = त्रिकोणन डाउनलोड एन्कोडिंग विफल रही: { $error }
cmd-file-user-chose-exit-without-saving = उपयोगकर्ता ने बिना सहेजे बाहर निकलना चुना
cmd-file-user-requested-exit-project-export = उपयोगकर्ता ने बाहर निकलने का अनुरोध किया (प्रोजेक्ट निर्यात या सहेजे न गए कार्य की पुष्टि आवश्यक है)
cmd-file-viewport = व्यूपोर्ट
cmd-file-wait-current-project-save-finish = वर्तमान प्रोजेक्ट का सहेजना पूरा होने तक प्रतीक्षा करें
cmd-file-wait-current-project-switch-finish = वर्तमान प्रोजेक्ट बदलना पूरा होने तक प्रतीक्षा करें
cmd-file-wait-project-operation-finish-before = परिवर्तन त्यागने से पहले प्रोजेक्ट क्रिया पूरी होने तक प्रतीक्षा करें
cmd-file-wait-project-revert-finish-before = सहेजने से पहले प्रोजेक्ट वापसी पूरी होने तक प्रतीक्षा करें
cmd-fuse-closed-polyline = बंद पॉलीलाइन
cmd-fuse-count-source-line-s = { $count } स्रोत रेखाएँ
cmd-fuse-created-shape-object-id-vertices = { $sources } स्रोत रेखाओं से { $vertices } शीर्षों वाला { $shape } { $object_id } बनाया गया
cmd-fuse-fuse-click-did-not-hit = जोड़ें: क्लिक किसी ऑब्जेक्ट पर नहीं हुआ (कर्सर के नीचे कुछ नहीं है)
cmd-fuse-fuse-click-was-not-close = जोड़ें: क्लिक चुनी गई रेखा के किसी भी अंतिम बिंदु के पर्याप्त निकट नहीं था
cmd-fuse-fuse-clicked-object-object-id = जोड़ें: ऑब्जेक्ट { $object_id } बंद पॉलीलाइन है; यह क्रिया केवल खुली पॉलीलाइन पर काम करती है
cmd-fuse-fuse-clicked-object-object-id-2 = जोड़ें: ऑब्जेक्ट { $object_id } खुली पॉलीलाइन नहीं है (यह { $kind } है)
cmd-fuse-fuse-clicked-object-object-id-3 = जोड़ें: क्लिक किया गया ऑब्जेक्ट { $object_id } अब मौजूद नहीं है
cmd-fuse-fuse-clicked-polyline-object-id = जोड़ें: पॉलीलाइन { $object_id } में केवल { $count } वर्टेक्स हैं; कम-से-कम 2 चाहिए
cmd-fuse-fuse-endpoint-marker-marker-index = जोड़ें: अंतिम बिंदु मार्कर { $marker_index } अब मौजूद नहीं है
cmd-fuse-fuse-line-needs-least-3 = जोड़ें: लाइन को पॉलीलाइन के रूप में बंद करने के लिए कम-से-कम 3 अलग वर्टेक्स चाहिए (अभी { $count })
cmd-fuse-fuse-lines = रेखाएँ फ़्यूज़ करें
cmd-fuse-fuse-need-least-2-segments = जोड़ें: लागू करने के लिए कम से कम 2 खंड चाहिए ({ $count } उपलब्ध)
cmd-fuse-fuse-no-active-layer-place = जोड़ें: जुड़ी हुई रेखा रखने के लिए कोई सक्रिय लेयर नहीं है
cmd-fuse-fuse-no-active-project-cannot = जोड़ें: कोई सक्रिय प्रोजेक्ट नहीं है, लागू नहीं किया जा सकता
cmd-fuse-fuse-no-source-line-close = जोड़ें: पॉलीलाइन में बंद करने के लिए कोई स्रोत रेखा नहीं है
cmd-fuse-fuse-object-awaiting-id-no = जोड़ें: ऑब्जेक्ट { $awaiting_id } अब मान्य पॉलीलाइन नहीं है
cmd-fuse-fuse-object-object-id-already = जोड़ें: ऑब्जेक्ट { $object_id } पहले से जोड़ने की शृंखला का भाग है; दूसरी लाइन चुनें
cmd-fuse-fuse-result-has-too-few = जोड़ें: परिणाम में बहुत कम शीर्ष हैं ({ $count }), रद्द किया जा रहा है
cmd-fuse-fuse-segment-object-object-id = जोड़ें: सेगमेंट ऑब्जेक्ट { $object_id } अब मान्य पॉलीलाइन नहीं है; कार्रवाई रद्द हुई
cmd-fuse-fuse-source-object-object-id = जोड़ें: स्रोत ऑब्जेक्ट { $object_id } अब मान्य खुली पॉलीलाइन नहीं है
cmd-fuse-fuse-source-object-object-id-2 = जोड़ें: स्रोत ऑब्जेक्ट { $object_id } अब मौजूद नहीं है
cmd-fuse-open-polyline = खुली पॉलीलाइन
cmd-include-include-failed-message = शामिल करना विफल रहा: { $message }
cmd-include-included-solid-shape-name-topology = सॉलिड “{ $shape_name }” को स्थलाकृतिक सतह “{ $topology_name }” में शामिल किया गया ({ $retained } फ़ेस रखे, { $skipped } क्लोज़र-कैप फ़ेस छोड़े)
cmd-include-including-pit-stockpile-solid = पिट/स्टॉकपाइल ठोस शामिल किया जा रहा है…
cmd-insert-point-count-operation-point-s = { $count } { $operation } बिंदु
cmd-insert-point-insert-point-elevation-requires-fini = ऊँचाई पर बिंदु डालने के लिए सीमित ऊँचाई मान आवश्यक है
cmd-insert-point-insert-points = बिंदु डालें
cmd-insert-point-inserted-count-operation-point-s = { $count } { $operation } बिंदु डाले गए
cmd-insert-point-intersection = प्रतिच्छेदन
cmd-insert-point-no-new-operation-points-were = कोई नया { $operation } बिंदु नहीं मिला
cmd-insert-point-select-least-two-polylines-before = प्रतिच्छेदन बिंदु डालने से पहले कम से कम दो पॉलीलाइन चुनें
cmd-insert-point-select-one-more-polylines-before = ऊँचाई पर बिंदु डालने से पहले एक या अधिक पॉलीलाइन चुनें
cmd-layer-created-layer-name = लेयर '{ $name }' बनाई गई
cmd-layer-deleted-layer-layer-id-all = लेयर { $layer_id } और उस पर सभी ऑब्जेक्ट हटाए गए
cmd-layer-duplicated-layer-duplicate-name = लेयर '{ $duplicate_name }' की प्रतिलिपि बनाई गई
cmd-layer-locked = लॉक
cmd-layer-name-copy = { $name } की प्रति
cmd-layer-selected-count-object-s-layer = लेयर { $layer_id } में { $count } ऑब्जेक्ट चुने गए
cmd-layer-state-layer-name = लेयर '{ $name }' { $state }
cmd-layer-unlocked = अनलॉक
cmd-move-tool-applied-move-delta-delta-count = विस्थापन ({ $delta }) { $count } ड्रिलहोल कॉलर पर लागू किया गया
cmd-move-tool-applied-move-delta-delta-count-2 = चाल अंतर ({ $delta }) को { $count } ऑब्जेक्ट पर लागू किया गया
cmd-move-tool-count-hole-s = { $count } छेद
cmd-object-edit-edited-kind = { $kind } संपादित किया गया
cmd-object-edit-edited-kind-count-vertices = { $kind } संपादित किया गया ({ $count } शीर्ष)
cmd-object-edit-no-changes-apply = लागू करने के लिए कोई परिवर्तन नहीं
cmd-object-edit-object-changed-since-editor-opened = एडिटर खुलने के बाद से यह ऑब्जेक्ट बदल गया है; वर्तमान संस्करण संपादित करने के लिए इसे फिर से खोलें
cmd-object-edit-object-edit-target-changed-discardin = संपादित किया जा रहा ऑब्जेक्ट बदल गया; संपादन छोड़ा जा रहा है
cmd-object-edit-object-no-longer-exists-document = वह ऑब्जेक्ट अब दस्तावेज़ में मौजूद नहीं है
cmd-object-edit-select-single-design-object-edit = संपादित करने के लिए एक ही डिज़ाइन ऑब्जेक्ट चुनें
cmd-object-edit-unassigned = अनिर्धारित
cmd-offset-create-offset = ऑफ़सेट बनाएँ
cmd-offset-created-offset-count-object-s = { $count } ऑब्जेक्ट का ऑफ़सेट बनाया गया
cmd-offset-offset-distance-must-greater-than = ऑफ़सेट दूरी शून्य से अधिक होनी चाहिए
cmd-omf-could-not-open-project-source = प्रोजेक्ट { $source_name } नहीं खोला जा सका: { $error }
cmd-omf-create-open-project-before-merging = डेटा मिलाने से पहले कोई प्रोजेक्ट बनाएँ या खोलें
cmd-omf-encoding-project = परियोजना एन्कोड की जा रही है…
cmd-omf-exported-project-path = प्रोजेक्ट को { $path } में निर्यात किया गया
cmd-omf-imported-project-project-name-from = { $source_name } से प्रोजेक्ट “{ $project_name }” आयात हुआ: { $count } शीर्ष-स्तरीय डेटासेट
cmd-omf-importing-project = परियोजना आयात की जा रही है…
cmd-omf-omf-export-failed-error = OMF निर्यात विफल रहा: { $error }
cmd-omf-omf-import-failed-error = OMF आयात विफल रहा: { $error }
cmd-omf-opened-project-project-name-from = { $source_name } से प्रोजेक्ट “{ $project_name }” खोला गया
cmd-omf-project-source-name-contains-no = प्रोजेक्ट '{ $source_name }' में कोई समर्थित डेटा तत्व नहीं है
cmd-omf-source-name-applied-project-origin = { $source_name }: मर्ज से पहले प्रोजेक्ट मूलबिंदु { $origin } लागू किया गया
cmd-omf-source-name-coordinate-reference-sys = { $source_name }: निर्देशांक संदर्भ प्रणाली “{ $source_crs }” प्रोजेक्ट CRS “{ $target_crs }” से अलग है; निर्देशांक पुनः प्रक्षेपण के बिना मर्ज हुए
cmd-omf-source-name-units-source-units = { $source_name }: इकाइयाँ “{ $source_units }” प्रोजेक्ट इकाइयों “{ $target_units }” से अलग हैं; निर्देशांक रूपांतरण के बिना मर्ज हुए
cmd-omf-source-name-warning = { $source_name }: { $warning }
cmd-omf-there-no-open-incline-design = निर्यात करने के लिए कोई खुला Incline Design डेटा नहीं है
cmd-placement-2-vertices = 2 शीर्ष
cmd-placement-count-vertices = { $count } शीर्ष
cmd-placement-created-circle-radius-radius-m = { $radius } मी त्रिज्या वाला वृत्त बनाया गया
cmd-placement-created-closed-polyline-count-vertic = { $count } शीर्षों वाली बंद पॉलीलाइन बनाई गई
cmd-placement-created-line-segment-2-vertices = 2 शीर्षों वाला रेखाखंड बनाया गया
cmd-placement-created-open-polyline-count-vertices = { $count } शीर्षों वाली खुली पॉलीलाइन बनाई गई
cmd-placement-placed-point-x-y-z = बिंदु को { $x }, { $y }, { $z } पर रखा गया
cmd-placement-radius-radius-m = त्रिज्या { $radius } मी
cmd-plot-composing-engineering-drawing = इंजीनियरिंग ड्रॉइंग तैयार की जा रही है…
cmd-plot-could-not-write-engineering-drawing = इंजीनियरिंग ड्रॉइंग नहीं लिखी जा सकी: { $error }
cmd-plot-drawing-scale-fitted-visible-data = ड्रॉइंग स्केल दृश्य डेटा के अनुसार फिट किया गया: 1:{ $scale }
cmd-plot-plot = प्लॉट
cmd-plot-saved-engineering-drawing-descriptio = इंजीनियरिंग ड्रॉइंग सहेजी गई: { $description } ({ $width } × { $height } px, { $dpi } dpi पर)
cmd-point-cloud-failed-load-point-cloud-error = पॉइंट क्लाउड लोड नहीं हुआ: { $error }
cmd-point-cloud-loaded-point-cloud-name-count = पॉइंट क्लाउड { $name } लोड किया गया ({ $count } बिंदु)
cmd-point-cloud-point-cloud-loader-disconnected-path = { $path } के लिए पॉइंट क्लाउड लोडर डिस्कनेक्ट हो गया
cmd-point-cloud-tin-max-edge-disabled = (अधिकतम किनारा अक्षम)
cmd-point-cloud-tin-max-edge-max-edge = (अधिकतम किनारा { $max_edge })
cmd-point-cloud-tin-point-cloud-tin-failed-error = पॉइंट क्लाउड TIN विफल रहा: { $error }
cmd-point-cloud-tin-terrain-tin-spatially-subsampled-sam = भूभाग TIN: { $total } बिंदुओं में से { $sampled } का स्थानिक उप-नमूना लिया गया
cmd-point-cloud-tin-terrain-tin-triangulated-vertex-coun = टेरेन TIN: { $vertex_count } अद्वितीय XY पॉइंट को { $face_count } फ़ेस में त्रिभुजित किया गया{ $suffix }
cmd-products-added-product-delay-ms-ms = उत्पाद { $delay_ms } मि.से. { $name } जोड़ा गया
cmd-products-deleted-product-delay-ms-ms = उत्पाद { $delay_ms } मि.से. { $name } हटाया गया
cmd-products-failed-save-products-error = उत्पाद सहेजे नहीं जा सके: { $error }
cmd-products-product-no-longer-palette = वह उत्पाद अब पैलेट में नहीं है
cmd-raster-draped-raster-raster-over-triangulat = रास्टर { $raster } को त्रिभुजन { $triangulation } पर ड्रेप किया गया (ओवरलैप होती सीमाएँ)
cmd-raster-failed-load-raster-name-error = रास्टर { $name } लोड नहीं हुआ: { $error }
cmd-raster-failed-load-raster-path-error = रास्टर { $path } लोड नहीं हुआ: { $error }
cmd-raster-loaded-raster-name-via-driver = { $driver } द्वारा रास्टर { $name } लोड हुआ ({ $srcx }×{ $srcy }, पूर्वावलोकन { $prevx }×{ $prevy })
cmd-raster-no-loaded-triangulation-overlaps-ext = कोई लोडेड त्रिकोणीकरण { $name } के विस्तार से ओवरलैप नहीं करता
cmd-raster-raster-loader-disconnected-path = { $path } के लिए रास्टर लोडर डिस्कनेक्ट हो गया
cmd-raster-undraped-rasters-from-count-triangul = { $count } त्रिकोणनों से रास्टर ड्रेप हटाए गए
cmd-relimit-relimit-click-did-not-hit = सीमा पुनर्निर्धारण: क्लिक किसी ऑब्जेक्ट पर नहीं हुआ (कर्सर के नीचे कुछ नहीं है)
cmd-relimit-relimit-click-ignored-tool-not = सीमा पुनर्निर्धारण: क्लिक अनदेखा किया गया; उपकरण अभी लक्ष्य चयन की प्रतीक्षा नहीं कर रहा
cmd-relimit-relimit-clicked-source-line-itself = सीमा पुनर्निर्धारण: स्रोत रेखा पर ही क्लिक किया गया, कोई दूसरी रेखा चुनें
cmd-relimit-relimit-no-source-line-set = सीमा पुनर्निर्धारण: कोई स्रोत रेखा सेट नहीं है, चयन रद्द किया जा रहा है
cmd-relimit-relimited-line-source-id-selected = रेखा { $source_id } को चयनित लक्ष्य तक पुनः सीमित किया गया
cmd-relimit-resized-line-source-id-using = रेखा { $source_id } का आकार { $mode } मोड और { $value } मान से बदला गया
cmd-rename-item-no-longer-belongs-active = वह आइटम अब सक्रिय प्रोजेक्ट का हिस्सा नहीं है
cmd-rename-renamed-before-name = '{ $before }' का नाम बदलकर '{ $name }' किया गया
cmd-rename-renamed-before-name-requested-alread = “{ $before }” का नाम “{ $name }” किया गया (“{ $requested }” पहले से उपयोग में है)
cmd-rotate-collar-turned-count-drillhole-collar-s = { $count } ड्रिलहोल कॉलर { $rotation } घुमाए गए
cmd-section-verb-count-item-s-section = { $section } में { $count } आइटम { $verb }
cmd-selection-delete-vertex = शीर्ष हटाएँ
cmd-selection-deleted-count-selected-object-s = { $count } चयनित ऑब्जेक्ट हटाए गए
cmd-selection-deleted-vertex-vertex-from-polyline = पॉलीलाइन { $object_id } से शीर्ष { $vertex } हटाया गया
cmd-selection-duplicate-selection = चयन की प्रति बनाएँ
cmd-selection-duplicated-count-object-s = { $count } ऑब्जेक्ट की प्रति बनाई गई
cmd-session-created-triangulation-name-vertex-co = { $surface_type } प्रकार की सतह से त्रिभुजन “{ $name }” बना ({ $vertex_count } वर्टेक्स, { $face_count } फ़ेस)
cmd-session-deleted-triangulation-name-from-proj = त्रिकोणन '{ $name }' प्रोजेक्ट से हटाया गया
cmd-session-failed-load-triangulation-error = त्रिकोणन लोड नहीं हुआ: { $error }
cmd-session-failed-load-triangulation-message = त्रिकोणन लोड नहीं हुआ: { $message }
cmd-session-loaded-triangulation-name-path-verte = त्रिभुजन “{ $name }” लोड हुआ ({ $path }, { $vertex_count } वर्टेक्स, { $face_count } फ़ेस)
cmd-session-set-triangulation-tri-id-color = त्रिकोणन { $tri_id } का रंग { $color } सेट किया गया
cmd-session-triangulation-load-path-ended-withou = { $path } के लिए त्रिकोणन लोड बिना परिणाम के समाप्त हुआ
cmd-session-triangulation-operation-failed-messa = त्रिकोणन क्रिया विफल रही: { $message }
cmd-session-unloaded-triangulation-name = त्रिकोणन '{ $name }' अनलोड किया गया
cmd-slice-entered-slice-view-cx-cy = { $cx }, { $cy }, { $cz } पर { $dx }, { $dy } दिशा में खंड दृश्य खोला गया ({ $length } मीटर लाइन)
cmd-slice-exited-slice-view = खंड दृश्य से बाहर निकला गया
cmd-slice-reset-section-view-fit-extents = सेक्शन दृश्य रीसेट करें (सीमाओं के अनुरूप बनाएँ)
cmd-slice-set-section-grid-enabled = सेक्शन ग्रिड सक्षम = { $enabled }
cmd-split-created-2-open-polylines = 2 खुली पॉलीलाइन बनाई गईं
cmd-split-split-line = रेखा विभाजित करें
cmd-split-split-points-choose-interior-vertex = बिंदुओं पर विभाजित करें: खुली रेखा का कोई आंतरिक शीर्ष चुनें
cmd-split-split-points-choose-two-non = बिंदुओं पर विभाजित करें: पॉलीलाइन के दो गैर-सन्निकट शीर्ष चुनें
cmd-split-split-source-polyline-into-two = स्रोत पॉलीलाइन को दो खुली पॉलीलाइन में विभाजित किया गया
cmd-text-finished-text-edit-object-object = ऑब्जेक्ट { $object_id } का पाठ संपादन पूरा हुआ
cmd-text-updated-text-object-object-id = ऑब्जेक्ट { $object_id } पर पाठ अपडेट किया गया
cmd-view-centre-rotation-not-available-flying = फ़्लाइंग मोड में घूर्णन केंद्र उपलब्ध नहीं है
cmd-view-fixed-centre-rotation-x-y = घूर्णन केंद्र { $x }, { $y }, { $z } पर तय किया गया
cmd-view-no-point-under-cursor-fix = घूर्णन केंद्र तय करने के लिए कर्सर के नीचे कोई बिंदु नहीं है
cmd-view-released-centre-rotation = घूर्णन केंद्र मुक्त किया गया
cmd-view-reset-view-fit-extents = दृश्य रीसेट करें (सीमाओं में फिट करें)
cmd-view-set-topology-wireframes-enabled = स्थलाकृति वायरफ़्रेम सेट करें = { $enabled }
cmd-view-set-view-points-enabled = दृश्य बिंदु सेट करें = { $enabled }
cmd-view-set-xy-grid-enabled = XY ग्रिड सक्षम = { $enabled }
cmd-view-zoom-extents-preserving-angle = सीमाओं तक ज़ूम करें (कोण बनाए रखते हुए)

## Common strings

common-add-product = उत्पाद जोड़ें
common-background = पृष्ठभूमि
common-block-model = ब्लॉक मॉडल
common-block-models = ब्लॉक मॉडल
common-cancelled = रद्द
common-chamfer = चैम्फर
common-choose = चुनें ...
common-circle = वृत्त
common-click-point-fix-centre-rotation = घूर्णन केंद्र तय करने के लिए किसी बिंदु पर क्लिक करें
common-clip-surface-polyline = पॉलीलाइन द्वारा क्लिप सतह...
common-closed = बंद
common-colour = रंग
common-confirm-omf-rewrite = OMF पुनर्लेखन की पुष्टि करें
common-could-not-replace-current-project = वर्तमान प्रोजेक्ट बदला नहीं जा सका: { $error }
common-count-object-s = { $count } ऑब्जेक्ट
common-create = बनाएँ
common-create-batter-berm = बेंच और बर्म बनाएँ
common-create-bezier-curve = बेज़ीर वक्र बनाएं
common-create-block-model = ब्लॉक मॉडल बनाएँ
common-create-block-model-2 = ब्लॉक मॉडल बनाएँ...
common-create-circle = सर्कल बनाएं
common-create-drill-pattern = ड्रिल पैटर्न बनाएँ
common-create-layer = परत बनाएँ
common-create-line = रेखा बनाएँ
common-create-ore-triangulation = अयस्क त्रिभुजीकरण बनाएं
common-create-ore-triangulation-2 = अयस्क त्रिभुजीकरण बनाएँ...
common-create-point = बिंदु बनाएँ
common-create-polyline = पॉलीलाइन बनाएँ
common-create-triangulation = त्रिभुजीकरण बनाएँ...
common-crosses = क्रॉस
common-cut = कटौती
common-cut-topology-pit-shell = स्थलाकृतिक सतह को पिट शेल के साथ काटें...
common-delete-layer = परत को हटाएँ
common-delete-product = उत्पाद हटाएँ
common-delete-selection = चयन हटाएँ
common-designs = डिजाइन
common-discard-layer-changes = परत परिवर्तनों को त्यागें
common-down = नीचे
common-drape-topology = ड्रेप से स्थलाकृतिक सतह तक
common-easting = ईस्टिंग
common-edit-object = ऑब्जेक्ट संपादित करें
common-edit-text = पाठ संपादित करें
common-elevation = ऊंचाई
common-exit-without-saving = बिना सहेजे बाहर निकलें
common-export-engineering-drawing = निर्यात इंजीनियरिंग ड्राइंग
common-filter = फ़िल्टर
common-fly-mode = उड़ान मोड
common-generate-contour-lines = समोच्च रेखाएँ उत्पन्न करें...
common-hide-all = सभी को छिपाएँ
common-hide-selection = चयन छिपाएँ
common-ignore = अनदेखा करें
common-import-csv-block-model = आयात CSV ब्लॉक मॉडल
common-import-dxf = आयात DXF
common-incline-design-project = Incline Design परियोजना
common-layer = परत
common-legend = किंवदंती
common-line = रेखा
common-line-weight = रेखा भार
common-lock-all = सभी को लॉक करें
common-lock-selection = चयन लॉक करें
common-m = m
common-max = मैक्स
common-merge-shell-into-topology = शेल को स्थलाकृतिक सतह में मिलाएं
common-merge-shell-into-topology-2 = शेल को स्थलाकृतिक सतह में मिलाएं...
common-move-collar = कॉलर ले जाएँ
common-move-design = डिज़ाइन ले जाएँ
common-move-selection = चयन स्थानांतरित करें
common-new-product = नया उत्पाद
common-no-block-models = कोई ब्लॉक मॉडल नहीं
common-no-design-layers = कोई डिज़ाइन लेयर नहीं
common-no-drill-holes = कोई ड्रिल होल नहीं
common-no-file-chosen = कोई फ़ाइल नहीं चुनी गई
common-no-open-project = कोई खुला प्रोजेक्ट नहीं
common-no-point-clouds = कोई पॉइंट क्लाउड नहीं
common-no-triangulations = कोई त्रिकोणन नहीं
common-none = कोई नहीं
common-northing = उत्तर
common-offset = ऑफसेट
common-open = खोलें
common-orientation = अभिविन्यास
common-point = बिंदु
common-point-cloud = पॉइंट क्लाउड
common-point-clouds = पॉइंट क्लाउड
common-polyline = पॉलीलाइन
common-polyline-layer = '{ $layer }' पर पॉलीलाइन
common-project = परियोजना
common-rasters = रास्टर
common-redo = रेडो
common-relimit-line = लाइन सीमा पुनर्निर्धारित करें
common-remove-project = परियोजना को हटाएँ
common-reset-view = दृश्य रीसेट करें
common-reveal-all = सब कुछ प्रकट करें
common-reveal-finder = Finder में दिखाएँ
common-rotate-collar = कॉलर घुमाएँ
common-save-exit = सहेजें और बाहर निकलें
common-scale-bar = स्केल बार
common-set-initiation-point = आरंभ बिंदु सेट करें
common-shape = आकार
common-shell = शेल सहित
common-slashes = तिरछी रेखाएँ
common-slice = स्लाइस
common-slice-triangulation-z-range = Z रेंज द्वारा त्रिभुजीकरण का टुकड़ा...
common-surface-contours = सतह समोच्च रेखाएँ
common-text = पाठ
common-text-2 = °
common-tie-holes = होल जोड़ें
common-triangulations = त्रिभुजीकरण
common-trim-topology = स्थलाकृतिक सतह के लिए ट्रिम ...
common-undo = निरस्त
common-undrape-all = सभी ड्रेप हटाएँ
common-uniform-white = एकसमान सफ़ेद
common-unlock-all = सभी को अनलॉक करें
common-untitled = शीर्षकहीन
common-up = ऊपर
common-vertical-exaggeration = ऊर्ध्वाधर अतिरेक
common-x = x
common-zoom-extents = विस्तार पर ज़ूम करें

## Confirmations strings

confirmations-close-project-unsaved-changes = प्रोजेक्ट बंद करें: सहेजे न गए परिवर्तन
confirmations-close-without-saving = बिना सहेजे बंद करें
confirmations-delete = हटाएँ
confirmations-delete-objects = वस्तुएँ हटाएँ
confirmations-discard = छोड़ें
confirmations-discard-all-unsaved-changes-layer =
    लेयर “{ $name }” के सभी सहेजे न गए बदलाव छोड़ें?
    सहेजी गई लेयर डिस्क से फिर लोड होगी और अन्य लेयरों के बदलाव बने रहेंगे। इसे पूर्ववत नहीं किया जा सकता।
confirmations-discard-all-unsaved-changes-name =
    “{ $name }” के सभी सहेजे न गए बदलाव छोड़ें?
    अंतिम सहेजा संस्करण डिस्क से फिर लोड होगा। इसे पूर्ववत नहीं किया जा सकता।
confirmations-discard-changes = परिवर्तनों को त्यागें
confirmations-exit-unsaved-changes = बाहर निकलें: सहेजे न गए परिवर्तन
confirmations-incline-design-cannot-reproduce-all = Incline Design मूल OMF की सारी सामग्री दोबारा नहीं बना सकता। सहेजने पर निम्न सामग्री छूट जाएगी:
confirmations-product = उत्पाद
confirmations-project = यह प्रोजेक्ट
confirmations-remove-name-delete-its-browser = '{ $name }' और उसकी ब्राउज़र में संग्रहीत प्रति हटाएँ? सहेजे न गए परिवर्तन खो जाएँगे।
confirmations-remove-project-unsaved-changes = प्रोजेक्ट हटाएँ: सहेजे न गए परिवर्तन
confirmations-remove-without-saving = बिना सहेजे हटाएँ
confirmations-replace-project-unsaved-changes = परियोजना प्रतिस्थापित करेंः सहेजे न गए परिवर्तन
confirmations-save = सहेजें
confirmations-save-anyway = वैसे भी सहेजें
confirmations-save-changes-current-project-before = इसे बदलने से पहले वर्तमान परियोजना में बदलाव सहेजें?
confirmations-save-changes-name-before-closing = '{ $name }' को बंद करने से पहले परिवर्तन सहेजें?
confirmations-save-changes-name-before-removing = '{ $name }' को Incline Design से हटाने से पहले परिवर्तन सहेजें?
confirmations-save-close = सहेजें और बंद करें
confirmations-save-modified-project-before-exiting = छोड़ने से पहले संशोधित परियोजना को सहेजें?
confirmations-save-modified-project-browser-storag = छोड़ने से पहले परिवर्तित परियोजना को ब्राउज़र भंडारण में सहेजें?
confirmations-save-remove = सहेजें और निकालें

## Console strings

console-copy-all = सभी कॉपी
console-copy-message = संदेश कॉपी करें
console-error = त्रुटि
console-info = जानकारी
console-no-console-activity-yet = अभी कोई कंसोल गतिविधि नहीं
console-pending = लंबित
console-progress-summary = जारी · { $summary }
console-success = सफल
console-warn = चेतावनी

## Csv strings

csv-block-model-category = श्रेणी
csv-block-model-value = मान

## Drill strings

drill-hole-add-stop = स्टॉप जोड़ें
drill-hole-all-rendered-intervals-opaque-white = सभी रेंडर किए गए अंतराल अस्पष्ट सफेद हैं।
drill-hole-burden-spacing-must-greater-than = बर्डन और स्पेसिंग शून्य से अधिक होने चाहिए
drill-hole-choose-valid-closed-polyline = मान्य बंद पॉलीलाइन चुनें
drill-hole-colour-scale = रंग पैमाना
drill-hole-field = फ़ील्ड
drill-hole-grayscale = धूसरमान
drill-hole-green-yellow-red = हरा–पीला–लाल
drill-hole-heat = ऊष्मा
drill-hole-no-holes-fit-inside-boundary = वर्तमान बर्डन और स्पेसिंग पर इस सीमा में कोई छेद नहीं आता
drill-hole-pattern-exceeds-maximum-maximum-hole = पैटर्न { $maximum } छेदों की अधिकतम सीमा से बढ़ गया; बर्डन या स्पेसिंग बढ़ाएँ
drill-hole-preset = प्रीसेट
drill-hole-px = px
drill-hole-rainbow = इंद्रधनुष
drill-hole-reset-preset = प्रीसेट रीसेट करें
drill-hole-rotation-offsets-must-contain-valid = घुमाव और ऑफ़सेट में मान्य संख्याएँ होनी चाहिए
drill-hole-selected-polyline-has-no-usable = चयनित पॉलीलाइन में उपयोग योग्य XY क्षेत्र नहीं है
drill-hole-smooth-interpolation = सुचारू अंतराल
drill-hole-spacing-would-scan-too-many = यह स्पेसिंग बहुत अधिक ग्रिड सेल स्कैन करेगी; बर्डन या स्पेसिंग बढ़ाएँ (अधिकतम { $maximum } छेद)
drill-hole-square = वर्गाकार
drill-hole-staggered = स्टैगर्ड
drill-hole-stepped-bands = स्टेप बैंड
drill-hole-text = ×
drill-hole-text-2 = −
drill-hole-unsupported-drillhole-source = असमर्थित ड्रिलहोल स्रोत
drill-hole-width = चौड़ाई
drill-pattern-arrangement = विन्यास
drill-pattern-axis-offset = { $axis } ऑफ़सेट
drill-pattern-blast-shape = ब्लास्ट आकार
drill-pattern-burden = बर्डन
drill-pattern-choose-closed-blast-boundary-then = बंद ब्लास्ट सीमा चुनें, फिर ग्रिड समायोजित करें। ड्रिलहोल व्यूपोर्ट में लाइव अपडेट होते हैं।
drill-pattern-closed-design-polyline-whose-xy = बंद डिज़ाइन पॉलीलाइन जिसका XY पदचिह्न छेदों से भरा जाएगा।
drill-pattern-counter-clockwise-pattern-rotation-f = वैश्विक { $axis } अक्ष से पैटर्न का वामावर्त घूर्णन।
drill-pattern-distance-between-holes-along-each = प्रत्येक पैटर्न पंक्ति के साथ छेदों की दूरी।
drill-pattern-e-g-west-cut-03 = उदा. वेस्ट कट 03
drill-pattern-finished-hole-diameter-entered-milli = अंतिम छेद व्यास। मिलीमीटर में दर्ज किया जाता है और प्रत्येक उत्पन्न छेद के साथ संग्रहीत होता है।
drill-pattern-hole-depth = छेद की गहराई
drill-pattern-hole-diameter = छेद का व्यास
drill-pattern-move-over-closed-polyline-then = बंद पॉलीलाइन पर जाएँ और व्यूपोर्ट में क्लिक करें। Esc चयन रद्द करता है।
drill-pattern-name-drillhole-dataset-created-proje = प्रोजेक्ट में बनाए गए ड्रिलहोल डेटासेट का नाम।
drill-pattern-none-picked = कुछ नहीं चुना गया
drill-pattern-pattern-name = पैटर्न नाम
drill-pattern-perpendicular-distance-between-patte = पैटर्न पंक्तियों के बीच लंबवत दूरी।
drill-pattern-pick = चुनें
drill-pattern-preview-count-hole-s-diameter = पूर्वावलोकन: { $count } छेद · { $diameter } मिमी व्यास · { $depth } मीटर गहरा
drill-pattern-rotation = घूर्णन
drill-pattern-shift-pattern-grid-along-global = पैटर्न ग्रिड को वैश्विक { $axis } अक्ष के साथ खिसकाता है, जबकि इसे ब्लास्ट आकार तक सीमित रखता है।
drill-pattern-spacing = स्पेसिंग
drill-pattern-staggered-offsets-every-second-row = स्टैगर्ड हर दूसरी पंक्ति को आधी स्पेसिंग से ऑफ़सेट करता है।
drill-pattern-vertical-depth-below-each-collar = प्रत्येक कॉलर के नीचे ऊर्ध्वाधर गहराई।

## Dxf strings

dxf-dxf-block-nesting-exceeds-maximum = DXF ब्लॉक नेस्टिंग अधिकतम गहराई ({ $depth }) से अधिक है; '{ $name }' छोड़ा जा रहा है
dxf-dxf-circular-block-reference-detecte = चक्रीय DXF ब्लॉक संदर्भ मिला: '{ $name }'
dxf-dxf-entity-referenced-undefined-laye = DXF एंटिटी ने अपरिभाषित परत '{ $name }' को संदर्भित किया; '{ $fallback }' के रूप में आयात किया गया
dxf-dxf-import-exceeds-what-budget = DXF आयात { $what } बजट ({ $limit }) से अधिक है; शेष ज्यामिति छोड़ी जा रही है
dxf-dxf-insert-references-unknown-block = DXF INSERT अज्ञात ब्लॉक '{ $name }' को संदर्भित करता है

## Edit strings

edit-absolute-length = निरपेक्ष लंबाई
edit-absolute-rl = पूर्ण आरएल
edit-action = कार्रवाई
edit-angle = कोण
edit-angle-from-horizontal-negative-downw = क्षैतिज से कोण, नीचे की ओर ऋणात्मक: -90 एक ऊर्ध्वाधर छेद है।
edit-app-web-not-recommended-production = उत्पादन में { $app } Web का उपयोग अनुशंसित नहीं है। इसे केवल डेमो के रूप में उपयोग करें।
edit-application = एप्लिकेशन
edit-apply = लागू करें
edit-apply-pick-target = लागू करें और लक्ष्य चुनें
edit-axis-value = { $axis } मान
edit-azimuth = दिगंश
edit-batter-angle = बेंच फेस कोण (°)
edit-bearing-holes-drilled-degrees-clockw = ग्रिड उत्तर से दक्षिणावर्त डिग्री में छेदों की ड्रिलिंग दिशा।
edit-bench-height = बेंच ऊँचाई
edit-benches = बेंच
edit-berm-width = बर्म चौड़ाई
edit-bezier-curve = बेज़ीर वक्र
edit-choose-layer = कोई लेयर चुनें
edit-choose-whether-entered-value-distanc = चुनें कि दर्ज मान ढलान की दूरी, क्षैतिज चौड़ाई या ऊर्ध्व ऊँचाई है।
edit-choose-which-two-polyline-paths = चुने वर्टेक्सों के बीच दो पॉलीलाइन पथों में से बदलने वाला पथ चुनें। लंबाई में ऊँचाई और वक्र किनारे शामिल हैं।
edit-click-corner-closed-polyline = बंद पॉलीलाइन पर एक कोने पर क्लिक करें।
edit-click-open-closed-polyline-begin = आरंभ करने के लिए खुले या बंद पॉलीलाइन पर क्लिक करें।
edit-click-second-vertex-replacement-span = प्रतिस्थापन अवधि के दूसरे शिखर पर क्लिक करें।
edit-click-vertex-start-replacement-span = प्रतिस्थापन अवधि शुरू करने के लिए एक शिखर पर क्लिक करें।
edit-collide-triangulation = त्रिभुजीकरण के साथ टकराव
edit-confirm-selection = चयन की पुष्टि करें
edit-control-point-1 = नियंत्रण बिंदु 1
edit-control-point-2 = नियंत्रण बिंदु 2
edit-copy = प्रतिलिपि
edit-corner-radius-limited-so-replacement = कोण त्रिज्या, सीमित ताकि प्रतिस्थापन आसन्न शिखरों को पार नहीं कर सके।
edit-create-new-layer = एक नई परत बनाएं
edit-create-new-project = एक नई परियोजना बनाएं
edit-create-project = परियोजना बनाएँ
edit-delta-length-m-use = लंबाई परिवर्तन (मी., + या - उपयोग करें)
edit-dip = नति
edit-direction = दिशा
edit-distance = दूरी
edit-distance-along-slope = ढलान के साथ दूरी
edit-download-free-native-version-our = हमारी वेबसाइट से मुफ़्त मूल संस्करण डाउनलोड करें ↗
edit-drill-hole = ड्रिल होल
edit-dx = dX
edit-dy = dY
edit-dz = dZ
edit-end = अंत
edit-enter-valid-elevation = एक वैध ऊंचाई दर्ज करें।
edit-exit-slice = स्लाइस से बाहर निकलें
edit-finish-polyline = समाप्त पॉलीलाइन
edit-generate-batter-berms = बेंच और बर्म बनाएँ
edit-height = ऊंचाई
edit-height-change = ऊँचाई परिवर्तन
edit-height-mode = ऊंचाई मोड
edit-horizontal-distance = क्षैतिज दूरी
edit-horizontal-width-each-flat-berm = अनुक्रमिक बेंच फेस के बीच प्रत्येक सपाट बर्म की क्षैतिज चौड़ाई।
edit-hover-choose-which-end-move = किस छोर को स्थानांतरित करना है यह चुनने के लिए होवर करें, फिर पुष्टि करने के लिए क्लिक करें।
edit-insert-point-elevation = ऊंचाई पर बिंदु डालें
edit-intersect = प्रतिच्छेद
edit-kind-properties = { $kind } { $properties }
edit-layer-name = परत का नाम
edit-load-project = प्रोजेक्ट लोड करें
edit-longest = सबसे लंबा
edit-m-s = m/s
edit-measure = माप
edit-mit-license = MIT लाइसेंस
edit-mode = मोड
edit-move = स्थानांतरित करें
edit-move-layer = लेयर पर स्थानांतरित करें
edit-move-which-end = कौन सा छोर स्थानांतरित करें
edit-movement-speed-slice-when-using = नेविगेशन कुंजी का उपयोग करते समय स्लाइस की गति।
edit-moving-end-endpoint = स्थानांतरण: अंतिम बिंदु
edit-moving-start-endpoint = स्थानांतरण: आरंभिक अंतिम बिंदु
edit-new-length-m = नई लंबाई (मी.)
edit-new-project = नया प्रोजेक्ट
edit-number-complete-batter-berm-levels = पूर्ण बैटर और बर्म स्तरों की संख्या। अधिकतम उस सबसे गहरे स्तर तक सीमित है जो निर्दिष्ट ज्यामिति बनाए रखता है।
edit-number-line-segments-used-approximat = चुने गए दो शीर्षों के बीच वक्र का अनुमान लगाने हेतु प्रयुक्त रेखाखंडों की संख्या।
edit-number-straight-segments-used-approx = गोल कोने का अनुमान लगाने वाले सीधे सेगमेंटों की संख्या। सीधे चैम्फर के लिए 1 रखें।
edit-object = वस्तु
edit-offset-element = ऑफसेट तत्व
edit-pick-side = पक्ष चुनें
edit-pit = ओपन पिट
edit-project-name = परियोजना का नाम
edit-properties = गुण
edit-radius = त्रिज्या
edit-recent = हाल ही में
edit-relative = सापेक्ष (+/-)
edit-relative-applies-vertical-change-eve = सापेक्ष हर पॉइंट पर ऊर्ध्व बदलाव लगाता है। निरपेक्ष RL सभी पॉइंटों को एक लक्ष्य ऊँचाई पर प्रक्षेपित करता है।
edit-remove-from-list = सूची से हटाएँ
edit-replace-path = पथ को प्रतिस्थापित करें
edit-rotate = घुमाएं
edit-rotation-speed-slice-when-using = Q और E का उपयोग करते समय स्लाइस की घूर्णन गति।
edit-s = °/s
edit-segments = खंड
edit-segments-lying-elevation-ignored = इस ऊंचाई पर पड़े खंडों को अनदेखा किया जाता है।
edit-select-endpoint-changes-other-endpoi = बदलने वाला अंतिम बिंदु चुनें; दूसरा अंतिम बिंदु स्थिर रहेगा।
edit-selected-holes-point-different-ways = चयनित छेद अलग दिशाओं में हैं। लागू करें सभी को इन कोणों पर सेट करता है।
edit-selected-start-end-point-moves = चुना प्रारंभ या अंत पॉइंट लाइन की दिशा में चलता है; विपरीत एंडपॉइंट स्थिर रहता है।
edit-set-axis = { $axis } सेट करें
edit-shortest = सबसे छोटा
edit-slice-view = खंड दृश्य
edit-slope-angle-each-batter-face = प्रत्येक बेंच फेस का झुकाव कोण, क्षैतिज से मापा गया।
edit-slope-angle-offset-positive-negative = ऑफ़सेट का ढलान कोण। धनात्मक और ऋणात्मक कोण पार्श्व गति के साथ कॉपी को स्रोत से ऊपर या नीचे ले जाते हैं।
edit-speed = गति
edit-start = आरंभ
edit-stockpile = स्टॉकपाइल
edit-stop-generated-offset-where-its = उत्पन्न ऑफसेट को रोकें जहां उसका मार्ग पहली बार दिखाई देने वाले त्रिभुजीकरण से मिलता है।
edit-target-rl = लक्ष्य स्तर
edit-text-colour-opacity = पाठ का रंग और अस्पष्टता।
edit-thickness-visible-slice-slab-centred = सिंहावलोकन सूचक पर केंद्रित दृश्यमान स्लाइस प्लेट की मोटाई।
edit-translation-distance-along-world-axi = विश्व { $axis } अक्ष के साथ स्थानांतरण दूरी।
edit-type = प्रकार
edit-type-direction-together-set-offset = प्रकार और दिशा मिलकर ऑफ़सेट का पक्ष तय करते हैं। Pit + ऊपर और Stockpile + नीचे बाहर की ओर; Pit + नीचे और Stockpile + ऊपर अंदर की ओर बढ़ते हैं।
edit-up-raises-each-bench-bench = ऊपर प्रत्येक बेंच को बेंच ऊँचाई से उठाता है; नीचे उसे घटाता है। इससे ऑफ़सेट पक्ष भी पलटता है—प्रकार देखें।
edit-value-interpreted-using-selected-mea = मान को चयनित माप और ऊंचाई मोड का उपयोग करके व्याख्या की जाती है।
edit-vertical-rise-fall-each-bench = अगले बर्म के निर्माण से पहले प्रत्येक बेंच की ऊर्ध्वाधर वृद्धि या गिरावट।
edit-world-x-y-z-coordinates = पहले बेज़ियर नियंत्रण बिंदु के वैश्विक X, Y और Z निर्देशांक।
edit-world-x-y-z-coordinates-2 = दूसरे बेज़ियर नियंत्रण बिंदु के वैश्विक X, Y और Z निर्देशांक।

## Events strings

events-couldn-t-exit-error = बाहर नहीं निकला जा सका: { $error }
events-couldn-t-save-error = सहेजा नहीं जा सका: { $error }
events-set-elevation = ऊँचाई सेट करें
events-set-elevation-from-cursor-hit = कर्सर हिट से ऊँचाई Z { $z } पर सेट की गई
events-tool-not-available-section-view = वह टूल सेक्शन दृश्य में उपलब्ध नहीं है

## Explorer strings

explorer-clear-active-triangulation-texture = सक्रिय त्रिभुजीकरण बनावट साफ़ करें
explorer-delete-from-project = परियोजना से हटाएँ
explorer-discard-changes = परिवर्तनों को त्यागें...
explorer-download = डाउनलोड करें
explorer-drape-over-surface = सतह पर ड्रेप
explorer-draped-over-surface = सतह पर ढका हुआ
explorer-duplicate = डुप्लिकेट करें
explorer-face-colour = चेहरे का रंग
explorer-id-block-model-id-source =
    ID: block-model:{ $id }{ $source }
    { $count } रंग वेरिएबल
explorer-id-drill-holes-id-source =
    ID: drill-holes:{ $id }{ $source }
    { $holes } होल
    { $fields } रंग फ़ील्ड
explorer-id-point-cloud-id-source =
    ID: point-cloud:{ $id }{ $source }
    { $count } पॉइंट
explorer-id-raster-id-source-driver =
    ID: raster:{ $id }{ $source }
    { $driver } · { $width } × { $height }
    { $projection }
explorer-id-triangulation-id-source = ID: triangulation:{ $id }{ $source }
explorer-load = लोड करें
explorer-lock = ताला
explorer-select-all-objects = सभी वस्तुओं का चयन करें
explorer-source-name = स्रोत: { $name }
explorer-unload = अनलोड करें
explorer-unlock = अनलॉक करें

## Files strings

files-automatic-colour = स्वचालित रंग
files-automatic-rl-spacing = स्वचालित ऊंचाई अंतराल
files-axis-scale-ratio = { $axis } स्केल अनुपात
files-ok = ठीक है
files-reset-1 = 1x पर रीसेट करें
files-rl-grid-options = ऊंचाई ग्रिड विकल्प
files-rl-spacing = ऊंचाई अंतराल
files-scales-z-distances-visually-without = संग्रहीत निर्देशांकों को बदले बिना Z दूरियों को केवल दृश्य रूप से स्केल करता है।
files-thickness = मोटाई
files-xy-grid-options = XY ग्रिड विकल्प

## Gpu strings

gpu-cache-block-model-surface-build-failed = ब्लॉक मॉडल सतह का निर्माण विफल रहा: { $error }
gpu-cache-block-model-surface-build-worker = ब्लॉक मॉडल सतह निर्माण वर्कर डिस्कनेक्ट हो गया
gpu-cache-block-model-surface-chunk-rejected = ब्लॉक मॉडल सतह खंड GPU आवंटन से पहले अस्वीकृत हुआ: इंस्टेंस={ $instances } बाइट, सीमा={ $limit } बाइट
gpu-cache-block-volume-preparation-worker-disc = ब्लॉक आयतन तैयारी वर्कर डिस्कनेक्ट हो गया
gpu-cache-translucent-volume-could-not-built = पारदर्शी आयतन नहीं बनाया जा सका ({ $error }); इसके बजाय यह ब्लॉक मॉडल घनों के रूप में दिखाया जा रहा है।
gpu-cache-triangulation-edge-chunk-rejected-be = त्रिभुजीकरण किनारा खंड GPU आवंटन से पहले अस्वीकृत हुआ: इंस्टेंस={ $instances } बाइट, सीमा={ $limit } बाइट
gpu-cache-triangulation-gpu-chunk-rejected-bef = त्रिभुजीकरण GPU खंड आवंटन से पहले अस्वीकृत हुआ: शीर्ष={ $vertices } बाइट, सूचकांक={ $indices } बाइट, सीमा={ $limit } बाइट
gpu-cache-triangulation-name-has-count-vertice = त्रिभुजीकरण '{ $name }' में { $count } शीर्ष हैं (> u32::MAX); इसे GPU के लिए खंडित नहीं किया जा सकता
gpu-cache-triangulation-name-uploaded-chunks-s = त्रिभुजीकरण '{ $name }' को { $chunks } स्थानिक खंडों में अपलोड किया गया ({ $faces } फलक)

## Init strings

init-gpu-adapter-vendor-name-backend = GPU अडैप्टर: { $vendor } / { $name } / { $backend } / { $device_type }
init-gpu-driver-driver-driver-info = GPU ड्राइवर: { $driver } { $driver_info }
init-gpu-supports-maximum-buffer-size = GPU अधिकतम { $size } MiB बफ़र का समर्थन करता है; बड़े दृश्य पूरे दिखाई नहीं दे सकते
init-surface-presentation-mode-mode = सतह प्रस्तुति मोड: { $mode }
init-wgpu-error-continuing-error = wgpu त्रुटि (कार्य जारी): { $error }

## Io strings

io-ascii-points-xyz-pts = ASCII बिंदु (.xyz, .pts)
io-attribute = विशेषता
io-blank-header = (खाली हेडर)
io-block-model = ब्लॉक मॉडल:
io-choose-file-purpose-map-its = अपने स्तंभों का मानचित्रण करने के लिए एक फ़ाइल उद्देश्य चुनें।
io-choose-loaded-block-model = कोई लोड किया गया ब्लॉक मॉडल चुनें
io-choose-loaded-layer = कोई लोड की गई लेयर चुनें
io-choose-loaded-triangulation = कोई लोड किया गया त्रिकोणन चुनें
io-choose-purpose = उद्देश्य चुनें…
io-choose-source-file-files-import = आयात करने के लिए स्रोत फ़ाइल या फ़ाइलों का चयन करें।
io-collar = कॉलर
io-column-mapping = स्तंभ मानचित्रण
io-comma-separated-values-csv = अल्पविराम से अलग मान (.csv)
io-csv-files = CSV फ़ाइलें
io-default = डिफ़ॉल्ट
io-depth = गहराई
io-diameter = व्यास
io-drawing-exchange-format-dxf = ड्रॉइंग एक्सचेंज फ़ॉर्मेट (.dxf)
io-drill-holes = ड्रिल होल
io-east-x = पूर्व / X
io-elevation-z = ऊँचाई / Z
io-end-x = अंत X
io-end-y = अंत Y
io-end-z = अंत Z
io-explicit-segments = स्पष्ट खंड
io-export = निर्यात करें
io-export-csv-block-model = निर्यात सीएसवी ब्लॉक मॉडल
io-export-dxf = निर्यात डीएक्सएफ
io-export-one-layer = एक परत का निर्यात
io-export-ply = PLY निर्यात करें
io-export-stl = STL निर्यात करें
io-export-wavefront-obj = Wavefront OBJ निर्यात करें
io-geotiff-tif-tiff = GeoTIFF (.tif, .tiff)
io-import = आयात करें
io-import-ascii-point-cloud = ASCII पॉइंट क्लाउड आयात करें
io-import-drillhole-csv-bundle = आयात ड्रिल होल CSV बंडल
io-import-geotiff = GeoTIFF आयात करें
io-import-las-laz-point-cloud = LAS/LAZ पॉइंट क्लाउड आयात करें
io-import-pcd-point-cloud = PCD पॉइंट क्लाउड आयात करें
io-import-ply = PLY आयात करें
io-import-stl = STL आयात करें
io-import-wavefront-obj = Wavefront OBJ आयात करें
io-interval = अंतराल
io-las-laz-las-laz = LAS / LAZ (.las, .laz)
io-mapped-csv-bundle-csv = मैप किया गया CSV बंडल (.csv)
io-model-file = मॉडल फ़ाइल
io-name-count-files = { $name } + { $count } फ़ाइलें
io-no-csv-chosen = कोई .csv नहीं चुना गया
io-no-csv-files-chosen = कोई CSV फ़ाइल नहीं चुनी गई
io-no-dxf-chosen = कोई .dxf नहीं चुना गया
io-no-omf-chosen = कोई .omf नहीं चुना गया
io-north-y = उत्तर / Y
io-ply-ply = PLY (.ply)
io-point-cloud-data-pcd = पॉइंट क्लाउड डेटा (.pcd)
io-source-file = स्रोत फ़ाइल
io-start-x = आरंभ X
io-start-y = आरंभ Y
io-start-z = आरंभ Z
io-stl-stl = STL (.stl)
io-triangulation = त्रिकोणन:
io-unmapped = मानचित्रित नहीं
io-wavefront-obj-obj = Wavefront OBJ (.obj)

## Jobs strings

jobs-background-task-poll-label-ended = पृष्ठभूमि कार्य '{ $poll_label }' बिना परिणाम के समाप्त हुआ
jobs-discarded-stale-background-result-po = “{ $poll_label }” का पुराना बैकग्राउंड परिणाम हटा दिया गया क्योंकि स्रोत बदला या बंद हुआ

## Logging strings

logging-activity-completed = गतिविधि पूरी हुई
logging-activity-started = गतिविधि शुरू हुई
logging-application-id-id = अनुप्रयोग ID: { $id }
logging-application-name-name = अनुप्रयोग नाम: { $name }
logging-application-startup = अनुप्रयोग प्रारंभ
logging-build-target-os-architecture = बिल्ड लक्ष्य: { $os }-{ $architecture }
logging-completed = पूर्ण
logging-count-messages = { $count } संदेश
logging-desktop-session-xdg-session-type = डेस्कटॉप सत्र: XDG_SESSION_TYPE={ $type }, XDG_CURRENT_DESKTOP={ $desktop }, WAYLAND_DISPLAY={ $wayland }, DISPLAY={ $display }
logging-initialising-incline-design = Incline Design प्रारंभ हो रहा है
logging-locale-environment-lang-lang-lc = स्थानीय परिवेश: LANG={ $lang }, LC_ALL={ $locale }, TZ={ $timezone }
logging-macos-session-user-user-shell = macOS सत्र: USER={ $user }, SHELL={ $shell }
logging-operating-system-gnu-linux = ऑपरेटिंग सिस्टम: GNU / Linux
logging-operating-system-macos = ऑपरेटिंग सिस्टम: macOS
logging-operating-system-microsoft-windows = ऑपरेटिंग सिस्टम: Microsoft Windows
logging-pointer-width-width-bit = पॉइंटर चौड़ाई: { $width }-बिट
logging-process-id-id = प्रक्रिया ID: { $id }
logging-release-version-version = रिलीज़ संस्करण: { $version }
logging-renderer = रेंडरर
logging-rust-compiler-host-host = Rust कंपाइलर होस्ट: { $host }
logging-system = सिस्टम
logging-system-error = सिस्टम त्रुटि
logging-unknown = अज्ञात
logging-windows-session-sessionname-session = Windows सत्र: SESSIONNAME={ $session }, USERNAME={ $user }
logging-working = कार्य जारी है…

## Mac strings

mac-cannot-install-macos-menu-bar = मुख्य थ्रेड के बाहर macOS मेनू बार स्थापित नहीं किया जा सकता
mac-quit-app = { $app } से बाहर निकलें

## Main strings

main-incline-design-web-startup-failed = Incline Design Web का आरंभ विफल रहा: { $error }

## Menu strings

menu-count-files-selected = { $count } फ़ाइलें चुनी गईं

## Object strings

object-edit-appearance = रूप-रंग
object-edit-arc-circle = चाप और वृत्त
object-edit-arc-segments = चाप खंड
object-edit-bulge = उभार
object-edit-bulge-arcs-horizontal-data-model = डेटा मॉडल के अनुसार उभार वाले चाप क्षैतिज होते हैं: चाप योजना में मुड़ता है और ऊंचाई एक शीर्ष से अगले तक सीधी रेखा में बदलती है।
object-edit-centre-x = केंद्र X
object-edit-centre-y = केंद्र Y
object-edit-centre-z = केंद्र Z
object-edit-chord = जीवा
object-edit-colour-layer = लेयर के अनुसार रंग
object-edit-enter-number = एक संख्या दर्ज करें
object-edit-follow-owning-layer-s-colour = इस ऑब्जेक्ट से जुड़े रंग के बजाय स्वामी लेयर के रंग का अनुसरण करें।
object-edit-id = ID
object-edit-identity = तत्समता
object-edit-insert-after = बाद में जोड़ें
object-edit-join-last-vertex-back-first = अंतिम शीर्ष को फिर से पहले शीर्ष से जोड़ता है।
object-edit-length-length-m = लंबाई { $length } मी
object-edit-move-down = नीचे ले जाएँ
object-edit-move-up = ऊपर ले जाएँ
object-edit-object-has-no-arc-segments = इस ऑब्जेक्ट में कोई चाप खंड नहीं है।
object-edit-object-has-single-position = इस ऑब्जेक्ट की केवल एक स्थिति है।
object-edit-object-needs-least-required-vertices = इस ऑब्जेक्ट को कम से कम { $required } शीर्षों की आवश्यकता है
object-edit-one-more-properties-not-valid = एक या अधिक गुण मान्य संख्या नहीं हैं
object-edit-perimeter-length-m-area-area = परिधि { $length } मी, क्षेत्रफल { $area } मी²
object-edit-reverse = उलटें
object-edit-row-row-position-bulge-not = पंक्ति { $row }: स्थिति या उभार मान्य संख्या नहीं है
object-edit-sweep = स्वीप कोण
object-edit-text-not-number = “{ $text }” एक संख्या नहीं है
object-edit-vertices = शीर्ष

## Omf strings

omf-element-name-has-count-tie = तत्व '{ $name }' में { $count } टाई-इन हैं जो उन छेदों का नाम लेते हैं जो अब उसमें नहीं हैं
omf-ignoring-colour-map-omf-attribute = OMF एट्रिब्यूट '{ $attribute }' का रंग मानचित्र अनदेखा किया जा रहा है: { $error }
omf-mining-data-exported-incline = Incline द्वारा निर्यात किया गया खनन डेटा
omf-omf-import = OMF आयात
omf-omf-texture = OMF टेक्सचर
omf-omf-validation-warnings-warnings = OMF सत्यापन चेतावनियाँ: { $warnings }
omf-project-application-metadata-applica = परियोजना एप्लिकेशन मेटाडेटा '{ $application }' बनाए नहीं रखा जाता
omf-project-author-not-retained = परियोजना के लेखक को बनाए नहीं रखा जाता
omf-project-description-not-retained = परियोजना का विवरण बनाए नहीं रखा जाता
omf-project-has-unsupported-metadata-key = परियोजना में असमर्थित मेटाडेटा कुंजियाँ हैं: { $keys }

## Plot strings

plot-1-1000-one-millimetre-sheet = 1:1000 पर, शीट पर एक मिमी जमीन पर एक मीटर है।
plot-1-scale-covers-width-height = 1:{ $scale } · { $width } × { $height } मीटर क्षेत्र
plot-all-visible-data = सभी दृश्यमान डेटा
plot-automatic-grid-interval = स्वचालित ग्रिड अंतराल
plot-border = सीमा
plot-centre = इस पर केंद्रित करें
plot-choose-smallest-conventional-scale-f = सबसे छोटा पारंपरिक पैमाना चुनें जो शीट पर दिखाई देने वाली हर चीज को फिट करता है।
plot-coordinate-grid = समन्वय ग्रिड
plot-current-view-centre = वर्तमान दृश्य केंद्र
plot-date = दिनांक
plot-date-2 = तिथि
plot-dots-per-inch-paper-size = प्रति इंच डॉट। इस कागज़ आकार को { $max_dpi } dpi तक रास्टर किया जा सकता है; 300 dpi सामान्य प्रिंट गुणवत्ता है।
plot-dpi = dpi
plot-drawing-no = आरेख संख्या
plot-drawing-number = रेखाचित्र संख्या
plot-drawn = द्वारा बनाया गया
plot-drawn-2 = द्वारा खींचा गया
plot-e-g-example-gold-project = जैसे, उदाहरण स्वर्ण प्रोजेक्ट
plot-entered-coordinates = दर्ज किए गए निर्देशांक
plot-export-png = PNG निर्यात...
plot-fit-scale-visible-data = दृश्यमान डेटा में पैमाना फ़िट करें
plot-grid-interval = ग्रिड अंतराल
plot-landscape = लैंडस्केप
plot-lists-visible-surfaces-design-layers = दृश्यमान सतहों और डिजाइन परतों को उनके रंगों के साथ सूचीबद्ध करता है।
plot-margin = मार्जिन
plot-margins-leave-no-room-map = मार्जिन मानचित्र के लिए कोई जगह नहीं छोड़ते
plot-metres-scale-1-scale = मीटर    मापनी 1:{ $scale }
plot-mm = mm
plot-north-arrow = उत्तरी तीर
plot-nothing-visible-draw = आरेखित करने के लिए कुछ दिखाई नहीं दे रहा
plot-paper = कागज
plot-paper-orientation-width-height-mm = { $paper } { $orientation } · { $width } × { $height } मिमी
plot-paper-size = कागज़ का आकार
plot-pick-interval-reads-roughly-every = मुद्रित शीट पर लगभग हर 50 मिमी पर पढ़ने वाला अंतराल चुनें।
plot-plan = प्लान
plot-plot-scale-must-positive-number = प्लॉट मापनी एक धनात्मक संख्या होनी चाहिए
plot-png-written-sheet-s-exact = PNG शीट के सटीक भौतिक आकार में लिखा जाता है और DPI दर्ज करता है, इसलिए वास्तविक स्केल पर प्रिंट होता है।
plot-portrait = पोर्ट्रेट
plot-resolution = रिज़ॉल्यूशन
plot-rev = संशोधन
plot-revision = संशोधन
plot-scale = मापनी
plot-scale-1 = स्केल 1:
plot-scale-framing = स्केल और फ्रेमिंग
plot-sheet-furniture = शीट फर्नीचर
plot-size-width-height-mm = { $size } ({ $width } × { $height } मिमी)
plot-subtitle = उपशीर्षक
plot-title = शीर्षक
plot-title-block = शीर्षक खंड
plot-today = आज

## Products strings

products-add-initiation = आरंभ जोड़ें
products-delay = विलंब
products-delay-palette = विलंब पैलेट
products-how-long-after-shot-fired = शॉट चलने के बाद यह कॉलर राउंड को कब आरंभ करता है।
products-initiation-name = आरंभ · { $name }
products-milliseconds-between-one-hole-firing = एक छेद फायरिंग और अगले के बीच मिलीसेकंड।
products-ms = ms
products-no-products = कोई उत्पाद नहीं
products-remove = हटाएँ
products-update = अद्यतन

## Progress strings

progress-percent-done-total = { $percent } ({ $total } में से { $done })
progress-task-finished = { $task }: पूर्ण

## Project strings

project-item = आइटम

## Properties strings

properties-adds-view-dependent-rim-highlight = ब्लॉक और सामग्री सीमाओं पर दृश्य-निर्भर किनारी चमक जोड़ता है। इसे बंद करने से वॉल्यूम रेंडरिंग का काम थोड़ा घटता है।
properties-block-model-downscale = ब्लॉक मॉडल कम पैमाने पर
properties-camera = कैमरा
properties-camera-clip-planes = कैमरा क्लिप विमान
properties-cap-while-resizing = आकार बदलते समय कैप
properties-dark-mode = डार्क मोड
properties-developer = डेवलपर
properties-downscale-rasters = डाउनस्केल रास्टर
properties-edit-object = ऑब्जेक्ट संपादित करें...
properties-field-view = दृश्य क्षेत्र
properties-fps = फ़्रेम/सेकंड
properties-frame-counter = फ्रेम काउंटर
properties-frame-rate-cap = फ्रेम रेट कैप
properties-hz = Hz
properties-interface = इंटरफ़ेस
properties-invert-horizontal = क्षैतिज उल्टा करें
properties-invert-vertical = ऊर्ध्वाधर उल्टा करें
properties-limits-newly-loaded-geotiff-previews = नए GeoTIFF पूर्वावलोकन की लंबी भुजा 4096 पिक्सेल तक सीमित करता है। GPU टेक्सचर सीमा तक पूर्ण रिज़ॉल्यूशन के लिए बंद करें; इससे अधिक मेमोरी लगेगी।
properties-line-colour = रेखा का रंग
properties-look-sensitivity = दृष्टि संवेदनशीलता
properties-max-clip-span = अधिकतम क्लिप स्पैन
properties-move-layer = लेयर पर स्थानांतरित करें...
properties-near-clip-limit = निकट क्लिप सीमा
properties-orbit-sensitivity = कक्षा संवेदनशीलता
properties-panel-chrome = पैनल क्रोम
properties-performance = प्रदर्शन
properties-plan-mode = योजना मोड
properties-presents-step-display-no-tearing = डिस्प्ले के साथ समकालिक रूप से प्रस्तुत होता है: कोई टियरिंग नहीं, और फ़्रेम दर डिस्प्ले तय करता है। बंद होने पर, फ़्रेम खींचे जाते ही प्रस्तुत हो जाते हैं और नीचे दी गई सीमा लागू होती है।
properties-reflective-block-edges = प्रतिबिंबित ब्लॉक किनारे
properties-restore-defaults-2 = डिफ़ॉल्ट पुनर्स्थापित करें
properties-show-console = कंसोल दिखाएँ
properties-shows-live-near-far-projection = स्थिति पट्टी में प्रत्यक्ष निकट और दूर प्रक्षेपण दूरी दिखाता है।
properties-snap-polling = स्नैप मतदान
properties-vertical-sync = वर्टिकल सिंक
properties-world-axis-gizmo = विश्व अक्ष गज़्मो
properties-zoom-cursor = कर्सर को ज़ूम करें
properties-zoom-sensitivity = ज़ूम संवेदनशीलता

## Screenshot strings

screenshot-could-not-encode-viewport-image = व्यूपोर्ट छवि एन्कोड नहीं की जा सकी: { $error }
screenshot-could-not-map-viewport-screenshot = व्यूपोर्ट स्क्रीनशॉट मैप नहीं किया जा सका: { $error }
screenshot-could-not-save-viewport-image = व्यूपोर्ट छवि { $path } सहेजी नहीं जा सकी: { $error }
screenshot-downloaded-viewport-image-file-name = व्यूपोर्ट छवि डाउनलोड की गई: { $file_name }
screenshot-saved-viewport-image-path = व्यूपोर्ट छवि सहेजी गई: { $path }
screenshot-viewport-image-download-failed-error = व्यूपोर्ट छवि डाउनलोड विफल रहा: { $error }

## Spatial strings

spatial-bvh-face-index-index-out = BVH फलक सूचकांक { $index } मेश की सीमा से बाहर है; अपभ्रष्ट त्रिभुज रखा जा रहा है

## State strings

state-above = पर या उससे ऊपर
state-activate-project = परियोजना को सक्रिय करें
state-all-open-incline-design-data = सभी खुले Incline Design डेटा
state-apply-generated-rings = जनित वलय लागू करें
state-apply-selection = चयन पर लागू करें
state-azimuth-azimuth-dip-dip = अज़ीमुथ { $azimuth }°, डिप { $dip }° द्वारा
state-azimuth-azimuth-dip-dip-2 = अज़ीमुथ { $azimuth }°, डिप { $dip }° तक
state-below = पर या उसके नीचे
state-centre-rotation = घूर्णन केंद्र
state-checking-unsaved-work = सहेजे न गए कार्य की जाँच हो रही है
state-choose-destination = गंतव्य चुनें
state-choose-one-more-files = एक या अधिक फ़ाइलें चुनें
state-clear-raster = रास्टर साफ़ करें
state-click-pit-shell-viewport = व्यूपोर्ट में पिट शेल पर क्लिक करें।
state-click-pit-stockpile-solid-viewport = व्यूपोर्ट में ओपन पिट या स्टॉकपाइल सॉलिड पर क्लिक करें।
state-click-surface-viewport = व्यूपोर्ट में सतह पर क्लिक करें।
state-click-topology-viewport = व्यूपोर्ट में स्थलाकृतिक सतह पर क्लिक करें।
state-close-project = प्रोजेक्ट बंद करें
state-colour-drillholes = रंग ड्रिल होल
state-copy-objects-layer = ऑब्जेक्ट लेयर में कॉपी करें
state-count-file-s = { $count } फ़ाइलें
state-count-object-s-axis-value = { $count } ऑब्जेक्ट · { $axis } { $value }
state-count-object-s-closed = { $count } ऑब्जेक्ट · { $closed }
state-count-object-s-layer = { $count } ऑब्जेक्ट · { $layer }
state-count-object-s-weight = { $count } ऑब्जेक्ट · { $weight }
state-count-object-s-z-elevation = { $count } ऑब्जेक्ट · Z { $elevation }
state-create-point-cloud-tin = पॉइंट क्लाउड TIN बनाएँ
state-create-project = परियोजना बनाएँ
state-current-project = वर्तमान प्रोजेक्ट
state-cut-topology-pit-shell = स्थलाकृतिक सतह को पिट शेल में काटें
state-cut-triangulation-polyline = त्रिभुजीकरण द्वारा पॉलीलाइन काटना
state-cut-triangulation-z = त्रिभुजीकरण को Z द्वारा काटें
state-dark-mode = डार्क मोड
state-detached = अलग
state-disabled = अक्षम
state-discard-project-changes = परियोजना परिवर्तनों को खारिज करें
state-discard-replace-project = परियोजना को त्यागें और प्रतिस्थापित करें
state-discarding-unsaved-changes = सहेजे न गए परिवर्तन त्यागे जा रहे हैं
state-docked = डॉक किया हुआ
state-drape-raster = ड्रेप रास्टर
state-drill-pattern = ड्रिल पैटर्न
state-duplicate-layer = परत डुप्लिकेट करें
state-east = पूर्व
state-enabled = सक्षम
state-exit-incline-design = Incline Design से बाहर निकलें
state-export-block-model-csv = निर्यात ब्लॉक मॉडल CSV
state-export-layer-dxf = DXF में निर्यात परत
state-export-omf = निर्यात ओएमएफ
state-export-project-dxf = प्रोजेक्ट को DXF में निर्यात करें
state-export-triangulation = निर्यात त्रिभुजीकरण
state-export-viewport-image = निर्यात व्यूपोर्ट छवि
state-finish-closed-polyline = बंद पॉलीलाइन पूर्ण करें
state-finish-open-polyline = खुली पॉलीलाइन पूर्ण करें
state-fit-extents = सीमाओं में फिट करें
state-fix-release-centre-both-views = वह केंद्र तय करता या मुक्त करता है जिसके चारों ओर दोनों दृश्य घूमते हैं
state-generate-contours = समोच्च रेखाएँ उत्पन्न करें
state-hidden = छिपा हुआ
state-import-drillholes = आयात ड्रिल होल
state-import-omf = आयात OMF
state-import-point-cloud = आयात पॉइंट क्लाउड
state-import-raster = आयात रास्टर
state-import-triangulation = आयात त्रिभुजीकरण
state-insert-intersection-points = प्रतिच्छेदन बिंदु डालें
state-insert-points-elevation = ऊंचाई पर बिंदु डालें
state-keep-inside = अंदर रखें
state-keep-outside = बाहर रखें
state-kriged-block-model = क्रिगिंग ब्लॉक मॉडल
state-load-block-model = लोड ब्लॉक मॉडल
state-load-drillholes = लोड ड्रिल होल
state-load-layer = लेयर लोड करें
state-load-point-cloud = लोड पॉइंट क्लाउड
state-load-raster = लोड रास्टर
state-load-triangulation = लोड त्रिभुजीकरण
state-locked-count-object-s = { $count } ऑब्जेक्ट लॉक हैं
state-major-major-minor-minor = प्रमुख { $major } · गौण { $minor }
state-move-axis-value = अक्ष मूल्य में स्थानांतरित करें
state-move-objects-layer = ऑब्जेक्ट लेयर में ले जाएँ
state-name-count-holes = { $name } · { $count } छेद
state-name-count-object-s = { $name } · { $count } ऑब्जेक्ट
state-name-z-min-z-max = { $name } · { $z_min } से { $z_max }
state-next-edit = अगला संपादन
state-north = उत्तर
state-open-containing-folder = संबंधित फ़ोल्डर खोलें
state-open-project = परियोजना खोलें
state-preserve-view-angle = दृश्य कोण बनाए रखें
state-previous-edit = पिछला संपादन
state-project-id = प्रोजेक्ट { $id }
state-remove-block-model = ब्लॉक मॉडल निकालें
state-remove-drillholes = ड्रिल होल निकालें
state-remove-point-cloud = पॉइंट क्लाउड निकालें
state-remove-raster = रास्टर निकालें
state-remove-triangulation = त्रिभुजीकरण निकालें
state-removed-from-active-triangulation = सक्रिय त्रिकोणन से हटाया गया
state-removed-from-every-triangulation = प्रत्येक त्रिकोणन से हटाया गया
state-rename-kind = { $kind } का नाम बदलें
state-save-close-project = परियोजना को सहेजें और बंद करें
state-save-despite-unsupported-content = असमर्थित सामग्री के बावजूद सहेजें
state-save-project = परियोजना के रूप में सहेजें
state-save-replace-project = परियोजना को सहेजें और प्रतिस्थापित करें
state-saving-current-project = वर्तमान प्रोजेक्ट सहेजा जा रहा है
state-section-section = { $section } खंड
state-select-layer-objects = परत वस्तुओं का चयन करें
state-selected-objects = चुने गए ऑब्जेक्ट
state-selected-polylines = चुनी गई पॉलीलाइन
state-selected-scene-elements = चुने गए दृश्य तत्व
state-set-block-model-variable = ब्लॉक मॉडल चर सेट करें
state-set-drillhole-colour-preset = ड्रिल होल कलर प्रीसेट सेट करें
state-set-entity-lock = इकाई लॉक सेट करें
state-set-grid = ग्रिड सेट करें
state-set-layer-lock = लेयर लॉक सेट करें
state-set-line-weight = रेखा भार निर्धारित करें
state-set-object-colour = वस्तु का रंग सेट करें
state-set-object-fill = वस्तु भराव सेट करें
state-set-point-visibility = पॉइंट दृश्यता सेट करें
state-set-polyline-closed = पॉलीलाइन बंद सेट करें
state-set-raster-lock = रास्टर लॉक सेट करें
state-set-standard-view = मानक दृश्य सेट करें
state-set-topology-wireframes = स्थलाकृतिक सतह वायरफ़्रेम सेट करें
state-set-triangulation-colour = त्रिभुजीकरण रंग सेट करें
state-show-console = कंसोल दिखाएँ
state-show-project = परियोजना दिखाएँ
state-shown = दिखाया गया
state-slice-mode = स्लाइस मोड
state-slice-preview = स्लाइस पूर्वावलोकन
state-south = दक्षिण
state-stem-contours = { $stem } समोच्च रेखाएँ
state-target-new-name = { $target } को “{ $new_name }”
state-trim-above = ऊपर का ट्रिम
state-trim-below = नीचे ट्रिम करें
state-trim-triangulation-surface = सतह पर त्रिभुजीकरण ट्रिम करें
state-undrape-raster = रास्टर ड्रेप हटाएँ
state-undrape-rasters = रास्टर ड्रेप हटाएँ
state-unload-block-model = ब्लॉक मॉडल अनलोड करें
state-unload-drillholes = ड्रिल होल अनलोड करें
state-unload-layer = लेयर अनलोड करें
state-unload-point-cloud = पॉइंट क्लाउड अनलोड करें
state-unload-raster = रास्टर अनलोड करें
state-unload-triangulation = त्रिभुजीकरण अनलोड करें
state-untitled-project = शीर्षकहीन प्रोजेक्ट
state-use-typed-radius = दर्ज त्रिज्या उपयोग करें
state-west = पश्चिम

## Status strings

status-clip-near-far = क्लिप निकट/दूर/Δ: -- / -- / --
status-frame-rate = फ़्रेम दर

## Text strings

text-could-not-build-vector-mesh = फ़ॉन्ट { $font }, ग्लिफ़ { $glyph } के लिए वेक्टर मेश नहीं बनाया जा सका: { $error }
text-document-text-mesh-exceeded-its = दस्तावेज़ टेक्स्ट मेश अपनी u32 सूचकांक सीमा से आगे निकल गया

## Tie strings

tie-in-choose-drillhole-dataset-tie-first = पहले जोड़ने के लिए ड्रिलहोल डेटासेट चुनें
tie-in-count-connector-s = { $count } कनेक्टर
tie-in-delete-tie-ins = टाई-इन हटाएँ
tie-in-deleted-count-selected-tie-connector = { $count } चयनित टाई-इन कनेक्टर हटाए गए
tie-in-hole = छेद
tie-in-initiation-point-lifted-from-name = आरंभ बिंदु { $name } से हटाया गया
tie-in-initiation-point-set-name-delay = आरंभ बिंदु { $name } पर { $delay } मि.से. देरी से सेट किया गया
tie-in-select-delay-product-palette-before = छेदों को जोड़ने से पहले पैलेट में देरी उत्पाद चुनें
tie-in-tied-count-connector-s-delay = { $count } कनेक्टर { $delay } मि.से. पर { $product } से जोड़े गए
tie-in-tied-count-connector-s-delay-2 = { $count } कनेक्टर { $delay } मि.से. पर { $product } से जोड़े गए, { $replaced } को बदलते हुए

## Toolbar strings

toolbar-fill-type = भरण प्रकार

## Toolbars strings

toolbars-auto-bench = स्वचालित बेंच
toolbars-bezier-polyline = बेज़ियर पॉलीलाइन
toolbars-chamfer-polyline-corners = पॉलीलाइन कोनों को चैम्फ़र करें
toolbars-create-text = पाठ बनाएँ
toolbars-cursor-regular = कर्सर: सामान्य
toolbars-cursor-snap-line = कर्सर: रेखा पर स्नैप
toolbars-cursor-snap-point = कर्सर: बिंदु पर स्नैप
toolbars-cursor-snap-surface = कर्सर: सतह पर स्नैप
toolbars-delete-points = बिंदु हटाएँ
toolbars-explode-polyline-lines = पॉलीलाइन को रेखाओं में तोड़ें
toolbars-fuse-polylines = पॉलीलाइन जोड़ें
toolbars-measure-distance = दूरी मापें
toolbars-new-layer = नई लेयर
toolbars-split-polyline-points = बिंदुओं पर पॉलीलाइन विभाजित करें
toolbars-strike-dip = स्ट्राइक और नति
toolbars-tool-not-available-section-view = { $tool } - सेक्शन दृश्य में उपलब्ध नहीं है

## Tri strings

tri-adaptive-concentrates-vertices-compl = अनुकूली विधि समतल फिट त्रुटि के आधार पर जटिल भूभाग पर शीर्ष केंद्रित करती है; एकसमान विधि उन्हें बराबर फैलाती है। भविष्य में और विधियाँ जोड़ी जा सकती हैं।
tri-adaptive-quadtree = अनुकूली (क्वाडट्री)
tri-axis-range = { $axis } परिसर
tri-base-topology-will-receive-pit = आधार स्थलाकृतिक सतह जिस पर पिट या भंडार का आकार रखा जाएगा।
tri-boundary-polyline = सीमा पॉलीलाइन
tri-bridge-gaps-boundary-concavities-nar = सतह पर इससे संकरे अंतरालों और सीमा के अवतल भागों को जोड़ें। 0 भी लगभग सैम्पलिंग सेल आकार तक के अंतराल जोड़ता है; बड़े मान बड़े छेद भरते और सीमा के अवतल भाग घटाते हैं।
tri-budget = बजट का आधार
tri-cancel-pick = चुनना रद्द करें
tri-candidate-detail = उम्मीदवार का विवरण
tri-candidate-fine-cells-per-budgeted = प्रत्येक बजट शीर्ष के लिए उम्मीदवार सूक्ष्म सेल। अधिक मान अनुकूली सैम्पलर को विवरण रखने की अधिक स्वतंत्रता देता है, पर निर्माण धीमा होता है।
tri-cap-surface-share-source-points = सतह को स्रोत बिंदुओं के हिस्से या सटीक शिखर संख्या से कवर करें।
tri-choose-input-clicking-loaded-surface = व्यूपोर्ट में लोड सतह पर क्लिक करके इस इनपुट का चयन करें
tri-choose-which-side-reference-topology = उनके साझा XY क्षेत्र में संदर्भ स्थलाकृतिक सतह का वह भाग चुनें जिसे सतह से हटाना है।
tri-clip = क्लिप
tri-clip-creates-new-triangulation-name = कटाई इस नाम से नया त्रिकोणन बनाती है; स्रोत सतह नहीं बदलती।
tri-clip-surface-polyline = पॉलीलाइन द्वारा क्लिप सतह
tri-closed-pit-stockpile-solid-whose = बंद पिट या भंडार ठोस जिसकी खुली सीमा परिणाम में शामिल होगी।
tri-create-new-layer-contours-append = कंटूर के लिए नई लेयर बनाएँ या सक्रिय प्रोजेक्ट की मौजूदा लेयर में जोड़ें।
tri-cut-topology-pit-shell = स्थलाकृतिक सतह को पिट शेल के साथ काटें
tri-e-g-design-trimmed = उदाहरण के लिए design_trimmed
tri-e-g-mysurf-cut = उदाहरण के लिए mysurf_cut
tri-e-g-mysurf-slice = उदाहरण के लिए, mysurf_slice
tri-e-g-surface-contour = उदाहरण के लिए surface_contour
tri-e-g-topo-cut = उदाहरण के लिए topo_cut
tri-e-g-topo-pit = उदाहरण के लिए topo_with_pit
tri-exact-number-surface-vertices-target = लक्षित सतह शीर्षों की सटीक संख्या। बहुत बड़े मान धीरे बनते हैं और काफ़ी मेमोरी उपयोग करते हैं।
tri-existing-ground-topology-will-cut = मौजूदा भू-सतह जिसे पिट शेल काटेगा।
tri-fill-holes-up = छेद भरने के लिए
tri-generate = जनरेट करें
tri-generate-contour-lines = समोच्च रेखाएँ उत्पन्न करें
tri-generate-upper-surface = ऊपरी सतह उत्पन्न करें
tri-hide-unload-sources = स्रोतों को छिपाएँ और अनलोड करें
tri-higher-edge-will-enforced-each = प्रत्येक टकराव पर ऊँचा किनारा लागू होगा। निचले टकराने वाले खंड ब्रेकलाइन के रूप में अनदेखे होंगे और सतह उन क्षेत्रों में प्रक्षेपित होगी। स्रोत पॉलीलाइन अपरिवर्तित रहेंगी।
tri-highlighted-breakline-edges-cross-ov = हाइलाइट किए गए ब्रेकलाइन किनारे अलग ऊँचाइयों पर प्लान में एक-दूसरे को काटते या ओवरलैप करते हैं। एक भूभाग सतह दोनों का अनुसरण नहीं कर सकती।
tri-intervals-colours = अंतराल और रंग
tri-keep-clipped-topology-included-shape = कटी स्थलाकृतिक सतह और शामिल आकार को एक इकाई में मिलाने के बजाय अलग त्रिकोणनों के रूप में रखें।
tri-keep-inside-discards-surface-outside = अंदर रखें पॉलीलाइन के बाहर की सतह को हटाता है। बाहर रखें सतह से पॉलीलाइन के आकार का छेद काटता है।
tri-keeps-only-surface-within-polyline = केवल पॉलीलाइन सीमा के भीतर की सतह रखता है।
tri-keeps-surface-relation-topology-with = सतह को स्थलाकृतिक सतह के XY कवरेज में { $relation } रखता है।
tri-layer-already-exists-select-above = वह लेयर पहले से मौजूद है; उसे ऊपर चुनें या कोई अन्य नाम चुनें।
tri-limit-z-range = Z श्रेणी सीमित करें
tri-major = प्रमुख
tri-max-edge-length = अधिकतम किनारे की लंबाई
tri-merge = मर्ज करें
tri-method = विधि
tri-min = न्यूनतम
tri-minimum-maximum-elevations-retained = आउटपुट सतह में रखी जाने वाली न्यूनतम और अधिकतम ऊँचाई। न्यूनतम मान अधिकतम से कम होना चाहिए।
tri-minor = गौण
tri-minor-controls-ordinary-contours-maj = माइनर सामान्य कंटूर नियंत्रित करता है। मेजर उभरे कंटूर नियंत्रित करता है और उसका अंतराल माइनर जितना या उससे बड़ा होना चाहिए।
tri-move-cursor-over-loaded-surface = कर्सर को किसी लोड की गई सतह पर ले जाएँ।
tri-name-assigned-elevation-clipped-outp = ऊंचाई-कट आउटपुट सतह को सौंपा गया नाम।
tri-name-assigned-merged-topology-pit = विलय किए गए स्थलाकृतिक सतह और ओपन पिट/स्टॉकपाइल परिणाम के लिए नाम।
tri-name-assigned-newly-created-contour = नव निर्मित समोच्च रेखा परत को सौंपा गया नाम।
tri-name-assigned-reconstructed-triangul = पुनर्निर्माण किए गए त्रिभुजीकरण को सौंपा गया नाम।
tri-name-assigned-topology-after-pit = पिट शेल को काटने के बाद स्थलाकृतिक सतह को सौंपा गया नाम।
tri-name-assigned-trimmed-output-surface = परिष्कृत आउटपुट सतह को सौंपा गया नाम।
tri-nearby-breakline-vertices-do-not = पास के ब्रेकलाइन शीर्ष ठीक एक ही स्थान पर नहीं मिलते, इसलिए सतह का त्रिकोणन नहीं किया जा सकता।
tri-new-layer = नई लेयर
tri-new-layer-name = नई परत का नाम
tri-once-merge-succeeds-unload-source = मर्ज सफल होने पर स्रोत स्थलाकृतिक सतह और सॉलिड अनलोड करें ताकि दृश्य में केवल मर्ज परिणाम रहे।
tri-only-loaded-triangulations-can-picke = केवल लोड किए गए त्रिभुजीकरण को चुना जा सकता है।
tri-operation = ऑपरेशन
tri-output-layer = आउटपुट लेयर
tri-percentage = प्रतिशत
tri-percentage-cloud = क्लाउड का प्रतिशत
tri-pick-from-view = दृश्य से चुनें
tri-pit-design-surface-only-areas = पिट डिज़ाइन सतह। केवल वे क्षेत्र कटाई में उपयोग होते हैं जहाँ यह स्थलाकृतिक सतह से नीचे खुदाई करती है।
tri-pit-shell = पिट शेल
tri-pit-stockpile-solid = पिट/भंडार ठोस
tri-recommended-weld-retry = अनुशंसितः वेल्ड और पुनः प्रयास
tri-reconstruct-triangulated-terrain-sur = पॉइंट क्लाउड से त्रिकोणित भूभाग सतह पुनर्निर्मित करें। अनुकूली सैम्पलर सबसे जटिल भूभाग पर शीर्ष बजट लगाता है और समतल क्षेत्रों को विरल रखता है।
tri-reduce-budget-candidate-detail-if = यदि आपकी मशीन में कम RAM है तो बजट या उम्मीदवार विवरण घटाएँ।
tri-reference-topology-defines-where-oth = संदर्भ स्थलाकृतिक सतह जो दूसरी सतह के कटने का स्थान तय करती है।
tri-reject-reconstructed-triangle-edges = इस दूरी से लंबे पुनर्निर्मित त्रिभुज किनारे अस्वीकार करें। किनारे की लंबाई की सीमा न रखने के लिए 0 उपयोग करें।
tri-removes-surface-within-polyline-boun = पॉलीलाइन सीमा के भीतर की सतह हटाकर बाकी रखता है।
tri-removes-topology-where-pit-shell = जहाँ पिट शेल नीचे खुदाई करता है वहाँ स्थलाकृतिक सतह हटाता है ताकि शेल छेद भर दे। जोड़ सतहों के बीच वास्तविक 3D संपर्क रेखा का अनुसरण करता है; भूमि से ऊपर खड़े शेल भागों के नीचे की स्थलाकृतिक सतह रखी जाती है।
tri-result = परिणाम
tri-save-two-entities = दो इकाइयों के रूप में सहेजें
tri-select = चुनें…
tri-share-source-points-keep-fractions = स्रोत बिंदुओं का हिस्सा रखने के लिए। 0.125% जैसे अंशों की अनुमति है।
tri-slice-triangulation-z-range = Z रेंज द्वारा Slice त्रिभुजीकरण
tri-solution-generate-upper-surface = समाधानः ऊपरी सतह उत्पन्न करें
tri-surface-trim = काटी जाने वाली सतह
tri-surface-will-changed-selected-topolo = बदली जाने वाली सतह; चुनी गई स्थलाकृतिक सतह अपरिवर्तित रहती है।
tri-text = %
tri-topology = स्थलाकृतिक सतह
tri-triangulation-failed = त्रिभुजीकरण विफल
tri-trim = ट्रिम करें
tri-trim-topology = स्थलाकृतिक सतह को ट्रिम करें
tri-uniform-grid = एकसमान ग्रिड
tri-up-target-point-count-points = { $point_count } पॉइंटों में से अधिकतम { $target } सतह वर्टेक्स बनेंगे ({ $percent }%)।
tri-use-full-surface-elevation-range = पूर्ण सतह ऊंचाई सीमा का उपयोग करें
tri-vertex-count = वर्टेक्स गिनती
tri-vertices-within-5-cm-xy = XY और Z में 5 सेमी के भीतर के शीर्ष इस त्रिकोणन के लिए एक स्थान साझा करेंगे। इससे बनी सतह स्थानीय रूप से 5 सेमी तक खिसक सकती है; स्रोत पॉलीलाइन अपरिवर्तित रहेंगी।
tri-weld-retry = वेल्ड और पुनः प्रयास
tri-when-enabled-generate-contours-only = सक्षम होने पर केवल निर्दिष्ट न्यूनतम और अधिकतम ऊँचाई के बीच कंटूर बनाए जाते हैं।

## Ui strings

ui-choose-offset-side = ऑफसेट पक्ष चुनें
ui-choose-relimit-side = सीमा पुनर्निर्धारित करें पक्ष चुनें
ui-click-circle-centre = सर्कल के केंद्र पर क्लिक करें
ui-click-closed-polyline-use-blast = ब्लास्ट आकार के लिए बंद पॉलीलाइन पर क्लिक करें
ui-click-collar-add-edit-initiation = आरंभ बिंदु जोड़ने या संपादित करने के लिए कॉलर पर क्लिक करें
ui-click-first-point-slice-line = स्लाइस लाइन के पहले बिंदु पर क्लिक करें
ui-click-first-vertex = पहले शिखर पर क्लिक करें
ui-click-perimeter-point-type-radius = परिधि बिंदु पर क्लिक करें या त्रिज्या टाइप करें
ui-click-second-point-slice-line = स्लाइस लाइन के दूसरे बिंदु पर क्लिक करें
ui-click-second-vertex = दूसरे शिखर पर क्लिक करें
ui-click-use-pointer-radius = या पॉइंटर त्रिज्या का उपयोग करने के लिए क्लिक करें
ui-could-not-copy-text-browser = टेक्स्ट को ब्राउज़र क्लिपबोर्ड में कॉपी नहीं किया जा सका: { $error }
ui-dip-horizontal-no-strike = { $dip } (क्षैतिज, कोई स्ट्राइक नहीं)
ui-distance-meters = { $distance } मीटर
ui-drag-ring-type-azimuth-dip = रिंग खींचें या अज़ीमुथ और डिप टाइप करें
ui-each-hole-turns-about-its = प्रत्येक छेद अपने कॉलर के चारों ओर घूमता है
ui-enter-positive-decimal-radius = एक सकारात्मक दशमलव त्रिज्या दर्ज करें
ui-esc-cancels = Esc रद्द करता है
ui-no-delay-product-tie = जोड़ने के लिए कोई विलंब उत्पाद नहीं है
ui-press-enter-use-typed-radius = टाइप किया गया त्रिज्या का उपयोग करने के लिए Enter दबाएं
ui-right-click-delay-palette-heading = एक जोड़ने के लिए विलंब पैलेट शीर्षक पर राइट-क्लिक करें
ui-select-designs = डिज़ाइन चुनें
ui-select-drill-hole = ड्रिल छेद चुनें
ui-select-endpoint-join = जोड़ने के लिए अंत बिंदु का चयन करें
ui-select-first-crest-toe-point = पहला क्रेस्ट/टो बिंदु चुनें
ui-select-item = एक आइटम चुनें
ui-select-line-fuse = फ्यूज करने के लिए एक पंक्ति चुनें
ui-select-line-polyline = एक पंक्ति या पॉलीलाइन चुनें
ui-select-line-relimit = सीमा पुनर्निर्धारित करने के लिए लाइन चुनें
ui-select-next-line-fuse = फ्यूज करने के लिए अगली पंक्ति का चयन करें
ui-select-opposite-berm-point = विपरीत बर्म बिंदु का चयन करें
ui-select-point = एक बिंदु चुनें
ui-select-polyline = एक पॉलीलाइन चुनें
ui-select-polyline-open-line = एक पॉलीलाइन या खुली रेखा चुनें
ui-select-polyline-vertex = पॉलीलाइन शिखर का चयन करें
ui-select-second-crest-toe-point = दूसरा क्रेस्ट/टो बिंदु चुनें
ui-select-second-split-point = दूसरा विभाजन बिंदु चुनें
ui-select-split-point = विभाजन बिंदु का चयन करें
ui-select-topologies = स्थलाकृतिक सतह का चयन करें
ui-slice-view = खंड दृश्य
ui-strike-strike-dip = { $strike }° स्ट्राइक · { $dip }
ui-value-dip = { $value }° नति

## Viewport strings

viewport-all-total-categories-keep-their = सभी { $total } श्रेणियाँ अपना रंग रखती हैं; केवल पहली { $shown } स्पष्ट रूप से अलग बनाई जाती हैं
viewport-axis-maximum = { $axis } अधिकतम
viewport-axis-minimum = { $axis } न्यूनतम
viewport-bar-blast-timeline-placeholder = ब्लास्ट समयरेखा [प्लेसहोल्डर]
viewport-bar-burden-relief-heatmap-placeholder = बर्डन रिलीफ हीटमैप [प्लेसहोल्डर]
viewport-bar-color = रंगः
viewport-bar-contours-equal-time-placeholder = समान समय का समोच्च रेखाएँ [PLACEHOLDER]
viewport-bar-disable-flying-mode = उड़ान मोड अक्षम करें
viewport-bar-disable-x-ray-vision = एक्स-रे दृश्य अक्षम करें
viewport-bar-drill-holes = ड्रिल होल:
viewport-bar-enable-flying-mode = उड़ान मोड सक्षम करें
viewport-bar-enable-x-ray-vision = एक्स-रे दृश्य सक्षम करें
viewport-bar-exit-slice-view = खंड दृश्य से बाहर निकलें
viewport-bar-fill = भरें:
viewport-bar-fix-centre-rotation = घूर्णन केंद्र तय करें
viewport-bar-hide-points = बिंदु छिपाएँ
viewport-bar-hide-rl-grid = ऊंचाई ग्रिड छिपाएँ
viewport-bar-hide-wireframes = वायरफ़्रेम छिपाएँ
viewport-bar-hide-xy-grid = XY ग्रिड छिपाएँ
viewport-bar-release-centre-rotation = घूर्णन केंद्र मुक्त करें
viewport-bar-show-points = बिंदु दिखाएँ
viewport-bar-show-rl-grid = ऊंचाई ग्रिड दिखाएँ
viewport-bar-show-wireframes = वायरफ़्रेम दिखाएँ
viewport-bar-show-xy-grid = XY ग्रिड दिखाएँ
viewport-bar-vertical-slice-view = ऊर्ध्वाधर खंड दृश्य
viewport-blank = (रिक्त)
viewport-choose-active-block-model-variable = सक्रिय ब्लॉक मॉडल चर चुनें
viewport-choose-variable = कोई चर चुनें
viewport-click-edit-color-right-click = रंग संपादित करने के लिए क्लिक करें; हटाने के लिए राइट क्लिक करें
viewport-click-type-boundary-s-value = इस सीमा का मान टाइप करने के लिए क्लिक करें
viewport-colour-mapping = रंग मानचित्रण
viewport-count-categories = { $count } श्रेणियाँ
viewport-count-category = { $count } श्रेणी
viewport-double-click-add-boundary-here = यहाँ सीमा जोड़ने के लिए डबल क्लिक करें
viewport-drag-move-middle-click-toggles = खिसकाने के लिए खींचें · मध्य-क्लिक ≤ बदलता है
viewport-drag-move-right-click-remove = खिसकाने के लिए खींचें · हटाने के लिए दायाँ-क्लिक · मध्य-क्लिक ≤ बदलता है
viewport-e = पू
viewport-edit-category-colour = इस श्रेणी का रंग संपादित करें
viewport-edit-colour-used-empty-values = रिक्त मानों का रंग संपादित करें
viewport-empty = (रिक्त)
viewport-empty-hidden = (रिक्त · छिपा हुआ)
viewport-filter-variables = फ़िल्टर चर
viewport-middle-drag-pan-scroll-zoom = मध्य-ड्रैग करने के लिए पैन · ज़ूम करने के लिए स्क्रोल
viewport-middle-drag-pan-scroll-zoom-2 = मध्य खींचने के लिए पैन · ज़ूम करने के लिए स्क्रॉल · हटाने के लिए क्लिक करें
viewport-n = उ
viewport-no-data-variable = इस चर के लिए कोई डेटा नहीं
viewport-no-matches = कोई मिलान नहीं
viewport-no-usable-range = (कोई उपयोग योग्य सीमा नहीं)
viewport-rebuild-variable-s-colours-from = इस चर के रंग उसके डेटा से फिर बनाएँ
viewport-reset = रीसेट करें
viewport-restore-full-model-range = मॉडल की पूरी सीमा पुनर्स्थापित करें
