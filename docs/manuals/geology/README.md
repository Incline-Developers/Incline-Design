# Geology in Incline Design

Written for Incline Design **v0.4.0-dev**.

How the geology pages fit together: the geologist's work from the first drill hole to a published model, then through mining, measurement and reconciliation back into the model. Read this first, then the topic pages in the order at the end.

Incline's geology begins with coal: a stratigraphic, sedimentary deposit, seams laid down flat and since folded, faulted and weathered. Why coal first? Because it is the most primary resource of our civilisation. Coal sits only a few steps past the hunter-gatherer and the farmer, yet nearly everything we build still reaches back to it. Coke made from coal turns iron ore into steel, and steel is the truck, the digger, the dragline, the rail, the bridge and the wind tower. Coal fired the first engines and lit the first cities, and it still generates more of the world's electricity than any other single source. The industry that digs every other commodity runs on machines made with coal. Coal is not going away. What changes is how well it is mined, and that is a geologist's work. A seam is also the plainest geology there is to learn on: a roof, a floor, a thickness, continuous for kilometres, cut by a few faults. A geologist who can model a seam honestly can model anything stratiform, and Incline will grow into other commodities the way industry did, on the foundation coal laid.

## Contents

1. [What geology at a mine is](#1-what-geology-at-a-mine-is)
2. [The life cycle](#2-the-life-cycle)
3. [Where the evidence comes from](#3-where-the-evidence-comes-from)
4. [Publish Model](#4-publish-model)
5. [Mine](#5-mine)
6. [Measure](#6-measure)
7. [Reconcile](#7-reconcile)
8. [Refine Model](#8-refine-model)
9. [Reading order](#9-reading-order)

---

## 1. What geology at a mine is

Geology is not the search for more coal. The coal is known to be there. Geology is defining what is there well enough to mine it: where each seam's roof and floor run, how thick it is, where it is faulted, where it pinches out. The model is that definition, written down as surfaces, and it is only as good as the last time it was tested against the ground.

Geologists make maps, and the model is the geologist's map. A mine geologist's map is three-dimensional and temporal: it holds where the coal is in space, and it changes with time as the pit advances and each month's mining tests it. A plan or a section is a slice through that map, not the map itself. Reading it takes three dimensions and time; a flat slice shows only part of it.

For the next 12 to 36 months of mining, the job is to project what has been observed into the ground ahead of the digger: to predict where the coal is before it is uncovered. Knowing that the coal is there is not enough. The coal roof and floor have to be placed within a metre or two at most, because that is the granularity drill and blast work to, and within half a metre or less is the aim. A model that is right on average but metres out at the next blast leaves coal behind and digs waste as coal, and that waste is avoidable.

Geology is a science, and the experiment happens at the coal face. A working pit exposes the seam along kilometres of wall, the engineers drill blastholes metres apart through it, and the surveyors measure every surface before and after it is dug. Against that, exploration holes can be hundreds of metres apart and decades old. Historical exploration finds the deposit and gives the first model. Mining proves it, hole by hole and wall by wall, and the geologist who stands at the face is the one who learns fastest, because the answer arrives in days, where exploration alone may take decades, if it answers at all.

Incline is built for that geologist first: the tools on these pages make a model by hand, from evidence, with every judgement visible, and they take the same evidence from an exploration rig, a blasthole drill or a survey pickup.

## 2. The life cycle

A mine geologist works in a cycle of five stages. A model is published: the best current interpretation of the deposit, with a date. The operation mines against it. What comes out is measured. Reconciliation compares the prediction with the measurement, and the differences refine the model, which is published again.

![The mine geologist's life cycle: five stages on a pentagon, clockwise from the top: 1 Publish Model, 2 Mine, 3 Measure, 4 Reconcile, 5 Refine Model, then back to 1.](images/00-lifecycle.svg)

The cycle does not end, and it is entered at the coal face. A geologist new to a deposit joins it at the mine, where the model meets the ground, and learns the deposit by watching the prediction hold or fail. Each pass should leave a better model than the last, and because the published model is dated, each pass also says who predicted what, and how it turned out.

The topic pages of this manual serve Publish Model and Refine Model. The three stages between them are where the evidence for the next model is made, and they are described here so the evidence is recognised when it arrives.

## 3. Where the evidence comes from

Three streams feed the model. The first is the one geologists are taught to think of as geology. The other two are larger, cheaper and faster, and they are what proves up, or disproves, what the first one observed. Those two also have the greatest economic influence on a mining operation. A geologist works from all three. A field technician works from one.

![Three evidence streams into the model. Exploration is drilling, seismic and CSAMT, sparse and years old. Mining and engineering is blastholes and mapped walls, dense and daily. Survey is every surface before and after it is dug. All three feed the model, which is published as a dated prediction and tested by reconciliation.](images/01-evidence-streams.svg)

| Stream                 | Who makes it                                               | How dense                                                                             | How soon it can be used       |
| ---------------------- | ---------------------------------------------------------- | ------------------------------------------------------------------------------------- | ----------------------------- |
| Exploration            | Exploration geologist, the drill crew and the geophysicist | Holes hundreds of metres apart, with seismic and CSAMT lines or grids across the gaps | Months to years before mining |
| Mining and engineering | Mine geologist, drill and blast crew, mining engineer      | Blastholes metres apart, every exposed wall mapped                                    | Days                          |
| Survey                 | Surveyor                                                   | Every surface before and after it is dug, scans of the pit and the stockpiles         | Hours to days                 |

Exploration is more than drilling. Seismic images the seams and the faults that offset them between the holes, and CSAMT (controlled-source audio-frequency magnetotellurics) maps how the ground's resistivity changes with depth. A hole samples one spot. A survey covers the ground between.

All three arrive as the same kinds of thing: holes with logged intervals, strings and points someone measured, and surfaces. Seismic and CSAMT arrive as what was interpreted from them: horizons and faults as surfaces and strings. Incline does not ask who made them. A blasthole pattern with its collars and what each hole logged loads as a drill hole set like any exploration hole, a surveyed pit floor is a surface, a mapped seam roof along a wall is a string, and all of them feed the next model exactly as the exploration holes did.

## 4. Publish Model

Publish Model holds the exploration that gathers the first evidence and the modelling that interprets it, and it ends when the model is handed over. It is the easy stage. Exploration looks the part, with drill rigs on site and a large bill, but the time goes into refining the model afterwards, and so does most of the value (see [Refine Model](#8-refine-model)).

### Exploration

Holes are drilled and logged: what the geologist saw down each hole, seam by seam, with depths, and what the downhole geophysics recorded. Seismic and CSAMT surveys cover the ground between the holes, and their interpretations, seam horizons and fault traces, come in as surfaces and strings. The logs, collars and hole surveys are the evidence every later step rests on, so before anything is built from them they are loaded, checked against each other and corrected where a pick is wrong.

Drilling and logging happen in the field. Incline starts when the logs arrive as tables: a collar table (one row per hole), an optional survey table (which way the hole goes) and interval tables (from and to depth along the hole, with a logged value such as a seam code). A seam code is one logged value for a stretch of hole. A working section is a named set of seam or ply codes mined as one unit. The strat column is the order of the codes, top of the ground first, and Incline fills it at import from the order most holes give.

A correction never overwrites the log. The logged value is kept beside the new one, so the record shows what was changed, and the changes can be exported as a worksheet for review.

- [Drill Holes](drill-holes/README.md): importing holes, working sections, the borehole inspector, log curves, the strat column and linking geophysics.
- [Drill Hole Corrections](drill-hole-corrections/README.md): renaming seams, moving roofs and floors, inserting and deleting intervals, and exporting the holes and the corrections worksheet.

### Modelling

Modelling turns hole picks into seams in space. A seam is two surfaces, its roof and its floor. The first one built is the reference surface: the better-defined of the two, usually the roof. Steps 1 to 4 build and check it, step 5 builds the other surface from the seam's thickness, and step 6 limits the pair. The steps are the same for every seam and every evidence stream.

1. **Picks become points.** **Reference Points** puts one point per hole on the roof or floor of the seam, at the height the log gives. The point is the first thing you can look at and question.
2. **Strings interpolate the surface between the holes.** A control string is an open line drawn where you know, or have decided, where the surface runs: a roof traced between holes, a subcrop, a fault. Every vertex carries a height. A pick is the stronger evidence: it was measured from the hole's log and downhole geophysics, where most strings are interpreted. A string is only as good as the observation behind it, such as a roof traced across a surveyed surface with its photograph draped on it. Where a string and a pick sit at the same place, the build uses the string's height and reports the pick it overrode, so check every override against the hole: a string should only win where an observation stands behind it. A closed string is the extent: the deposit, or the area being mined. The control strings of one surface live on one layer, and the extent is the one closed string selected with them.
3. **The reference surface is built through the points and the strings.** **Build Surface** fits a smooth surface through the points and strings, with a height every 5 m in plan, and reports what it cleaned, left out and overrode. It is an exact interpolator: it passes exactly through what it keeps, so the surface is only as good as the points and strings. The build joins small disagreements where strings cross in its own copy, and rings on the strings what it left out or refused. **Clean Strings** helps fix small disagreements in the strings themselves and rings the rest for you to fix by hand.
4. **The reference surface, roof or floor, is checked in section.** A vertical section along a line of exploration holes shows at once whether the surface passes through each logged interval or rides above or below it. Where it misses, **Save Surface Lines as Strings** keeps the lines the shown surfaces cut on the section as strings on a new layer. Move each line to its own surface's layer, redraw it in the plane of the section to your interpretation, and build again.
5. **The other surface, floor or roof, comes from the thickness.** True thickness is the seam measured square to its bedding. Apparent vertical thickness is the vertical distance from roof to floor at a point in plan. A hole logs the true thickness only where it crosses the seam square on; otherwise it logs more, the apparent thickness along the hole, as the picture below shows. From the reference surface, the other surface follows in five steps:

   1. Start from the reference surface, built and checked in steps 1 to 4.
   2. **Thickness Points** measures the true thickness at each hole that logs the seam inside the reference surface, square to its bedding, whichever way the hole runs.
   3. **Thickness Surfaces** grids those true thicknesses at the same points in plan as the reference surface's heights.
   4. At each point of the true thickness grid it turns the true thickness into apparent vertical thickness, using the reference surface's dip and strike at that point.
   5. It subtracts the apparent vertical thickness from a roof to give the floor, or adds it to a floor to give the roof.

   The reference surface holds one height at each point in plan, which is why the fourth of these steps turns the thickness vertical: the other surface is the reference surface moved straight up or down by the apparent vertical thickness.

   ![Four holes through a seam, the length each logs in orange and, where it differs, the true thickness dashed grey; the seam dips 20 degrees and the inclined hole is 20 degrees off vertical: a vertical hole through a seam dipping 20 degrees logs about 6% more, a hole 20 degrees off vertical through a flat seam logs about 6% more, the same hole leaning straight up-dip through the dipping seam crosses it square on and logs the true thickness, and leaning straight down-dip, 40 degrees off square, it logs about 30% more.](images/02-thickness-cases.svg)
6. **The pair is limited.** **Clip to Surface** trims the roof and floor to a top and a bottom you choose, such as the base of weathering, and closes the solid between them. It makes cut copies and a solid, and the originals stay as they are.

Every step is a judgement, and the pages say which part Incline decides and which part you do. Incline fits the surfaces, measures the thickness and reports what it did. You choose the picks and the reference surface, draw the strings, accept or correct a refusal, and decide when the model is good enough.

- [Control Strings](control-strings/README.md): what a control string is, drawing strings, cleaning them and draping them on a surface.
- [String Vertices](string-vertices/README.md): cutting strings where they cross, and working on single vertices and their spacing.
- [Surfaces](surfaces/README.md): reference points at the holes, and **Build Surface** through the points and strings.
- [Sections](sections/README.md): surface lines on a section, saving them as strings, editing strings in the plane of the section.
- [Seam Thickness](thickness/README.md): true thickness at the holes, the seam's second surface, and **Clip to Surface**.

### Publishing

The model is published on a regular schedule, or whenever it is good enough to plan against: each seam's roof, floor and true thickness, limited to where the seam is, with the date it was handed over. Publishing fixes the prediction that every later measurement is compared with.

Incline has no separate publishing tool. What is published is what the modelling pages make. The surfaces go to the short-, medium- and long-term planning engineers, who can schedule from them in Incline's Planning workspace or export them.

## 5. Mine

The operation digs to a sequence or plan made from the published model. The geologist's work here is watching the model meet the ground: mapping the seam roofs, floors and faults exposed in the pit walls and floors, marking where coal ends and waste starts, and noting where the ground disagrees with the model. An exposed wall is the largest and cheapest observation of the deposit there is, and the blasthole pattern drilled through the seam before every shot is the densest drilling the deposit will ever get. So the geologist works with drill and blast and with survey to make sure the right data is collected, in a form that reconciliation and the next refinement of the model can use.

Incline takes this evidence as it takes any other: a blasthole pattern loads as a drill hole set from its collar and interval tables, and a mapped roof or fault trace is drawn as a string in the plane of a section or in plan. There is no tool here that belongs only to this stage.

## 6. Measure

Material that is moved is measured: survey pickups of the void, scans of the pit and the stockpiles, tonnes over the weightometers, truck weights, the plant's figures, samples of what was mined. Most of the shape comes from survey: triangulated surfaces (TINs) of the void and the stockpiles, rasters, and point clouds, some with photographs draped over them. These are the other side of the comparison. A measurement is only as good as its base: a stockpile volume means nothing without the ground the pile sits on.

The survey data comes into Incline through the **Import** window, in the open formats it lists: surfaces and strings, rasters as GeoTIFF, and scans as point clouds. The tonnes, plant figures and samples stay in the systems that record them. This manual does not cover tools for this stage.

## 7. Reconcile

Reconciliation sets the prediction beside the measurement for the same block of ground and asks where they differ and why. In practice, last month's surveyed surface and today's surface bound a solid: the ground mined in between. The model says how much coal that solid should have held, and the weightometer at the processing plant says how many tonnes were recovered. A difference can come from the shape of a seam, from its quality, or from how it was mined: loss and dilution. Only the first two point back at the model. Done soon after the ground is mined, it tells the geologist what to change while the change still matters, and because the model carries a date and a name, it also tells the geologist how good their last prediction was.

This manual does not yet cover tools for this stage.

## 8. Refine Model

Refining puts what was learned back into the model. A pick shown to be wrong is corrected, the blastholes and mapped walls join the evidence, control strings are redrawn where the ground disagreed, and the surfaces are built again. The refined model is published, dated, and the cycle starts again.

This is the stage that takes the time, and it is where most of the value is. The value comes from the geologist refining what goes on in the next strip to be mined, over the next 12 to 36 months. A wrong digger sequence costs a great deal more than an exploration drill rig.

Within this manual, refining uses the tools already met: the corrections on the [Drill Hole Corrections](drill-hole-corrections/README.md) page, which keep the logged value beside the new one, then [Control Strings](control-strings/README.md), [Surfaces](surfaces/README.md) and [Seam Thickness](thickness/README.md) to rebuild.

## 9. Reading order

For a first walk-through, read the topic pages in this order. Each page builds on the pages before it. Most examples use the same three synthetic holes, S1 to S3, with seams A and B, loaded on the first page.

1. [Drill Holes](drill-holes/README.md): the holes and their working sections, which Surfaces and Seam Thickness read.
2. [Drill Hole Corrections](drill-hole-corrections/README.md): fixing a wrong pick before anything is built from it.
3. [Control Strings](control-strings/README.md): strings drawn and cleaned before the first surface is built.
4. [String Vertices](string-vertices/README.md): the finer string tools, used as the strings need them.
5. [Surfaces](surfaces/README.md): points at the holes, then a surface through the points and the strings.
6. [Sections](sections/README.md): needs a built surface to draw its lines on a section.
7. [Seam Thickness](thickness/README.md): needs a built surface to measure against.
