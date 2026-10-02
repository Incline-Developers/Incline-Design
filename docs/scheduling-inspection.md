# Inspecting a calculated schedule

Four views read the published schedule (`CalculatedSchedule`). None of them
asks the solver anything, recalculates, or changes a figure the Calendar shows.

## Time slider

The Gantt always has a time slider: a labelled handle in the ruler and a line
across the rows. Press anywhere on the ruler, or drag the handle or the line,
to move it. It snaps to whole hours, or to quarter hours at the finest zoom.
With the pointer over the Gantt, ← and → step it an hour, and Shift steps a
day. When it is scrolled out of view, a label pinned to that edge of the ruler
says where it is, and pressing the label brings it back.

The slider is one instant, `EditorState::schedule_time_h`, shared by the Gantt,
the Charts and Animate's scrubber. It is kept across recalculations, so editing
a bar does not lose the instant being inspected, and it is reset only when
another project becomes active. It may sit past the calculated horizon. Animate
cuts its view at the horizon without moving the slider, and future bar tools
(extend to slider, split at slider, paste at slider) will use it there too.

## Inspector

A panel beside the Gantt and the Charts, toggled from their toolbar, describes
the slider's instant:

- **Loaders:** what each machine is doing, at what rate, from where and to where.
  When it isn't working, it shows its delay type or its idle reason, plus the
  piles or destinations behind that reason.
- **Stockpiles:** tonnes held, with a fill bar when a capacity is set. Also:
  - whether the pile is building, reclaiming, standing or full;
  - its Mode, when the Mode isn't the default;
  - for a blended pile, when its rest ends.
- **Crushers:** the feed now and the day's tonnes against its limit. A warning
  line appears when the day's blend is outside a grade band. The flag uses the
  day's blend, which is what the band prices, not the single hour.
- **Dumps:** what each is receiving now and what it has received so far.
- **Trucks:** trucks hauling, against the day's fleet after availability and
  utilisation.

Grades, bands and the span behind each figure are on hover.

## Charts

The Charts page sits between Gantt and Animate. It draws one chart per
destination along the Gantt's own timeline, sharing its zoom, pan, time slider
and Inspector:

- **Stockpile:** what it holds, against its capacity. Days whose Mode stops
  building or reclaiming are tinted: blue for build only, green for reclaim
  only, grey for off.
- **Crusher:**
  - the hourly feed;
  - for each tracked grade with a target band, the hourly feed grade against
    the daily band. The day's blend is drawn as a bar across each day: white
    inside the band, orange outside. The scale follows the bands and the
    blends, and single hours that stray further are clipped at the edge.
- **Dump:** the tonnes received to date.

Each header gives the figure at the slider and explains the chart on hover. On
the plot, hovering gives the figures at that instant, and pressing moves the
slider. The hourly series come from `CalculatedSchedule::hourly_receipts`,
built once per result. Each delivery is spread over the hours it overlaps.

## Calendar totals and the report export

The Calendar has a **Total** column, pinned beside Default, for every
calculated row:

- tonnes, truck-hours and value are summed;
- closing stock and deposited tonnes are taken at the end;
- grades are weighted by tonnes.

**Export report** on the Calendar toolbar groups the calculated schedule by
day, by week (days 1–7, 8–14 and so on) or over the whole schedule. It then
copies the tables as tab-separated text, which a spreadsheet pastes into
columns, or saves them as CSV. On the desktop that opens a save dialog; in the
browser it downloads the file. The tables are:

- **Movements:** period, loader, dig or reclaim, source (a block by its dig
  area, a pile by name), destination, tonnes, tonnes-weighted grades,
  truck-hours and movement value.
- **Loader time:** tonnes dug and reclaimed, then hours digging, reclaiming,
  delayed and idle, with idle split by reason. Hours are the time covered, so
  two blocks worked in one interval count once. A machine the calculation never
  reached has no work.
- **Stockpiles:** opening, received, reclaimed, closing, and closing grades.
- **Crushers:** processed tonnes, the period's limit (blank when any day is
  unlimited), tonnes-weighted grades, and the grade-target penalty.
- **Dumps:** received in the period, and received to date.
- **Trucks:** truck-hours used against truck-hours available.

Numbers are written without separators or units; the units are in the headings.
Fluent's invisible direction marks are stripped, so cells hold plain text.

This is the fixed, automatic export. A user-built reporting page is planned
separately.
