# Screenshots to capture

Save each screenshot **in this folder** (`docs/manuals/optimization/images/`) under the exact name below. The manual
already links these names, so a file dropped in here shows up in the manual with no text edits. Each one's full
description is the `> **Figure N - to capture**` note under its link in `../README.md`. Delete that note once the
image is in.

PNG, about 1600 px wide, the same theme throughout.

| File | Manual section | Shows |
|---|---|---|
| `01-block-model.png` | 2 | `mclaughlin_bm` coloured by `au`, 3D, with legend |
| `02-scenarios-list.png` | 3 | the scenarios list, with numbered callouts |
| `03-inputs.png` | 4.1 | editor: Inputs |
| `04a-constants-add-group.png` | 4.2 | editor: Constants, *Group name* prompt with `Other` |
| `04-constants.png` | 4.2 | editor: Constants, finished, three groups |
| `05-mining-costs.png` | 4.3 | editor: Mining costs |
| `06-processing-costs.png` | 4.4 | editor: Processing costs, Mill selected |
| `07-revenues.png` | 4.5 | editor: Revenues |
| `08-constraints.png` | 4.6 | editor: Constraints, rosette |
| `09-outputs.png` | 4.7 | editor: Outputs, 60 shells |
| `10-run-progress.png` | 5 | list with the progress ring |
| `11-results-window.png` | 5.1 | Results, cumulative, RAF 1.00 hovered |
| `12-shell-filter-choosing.png` | 6 | Results while choosing 6 shells |
| `13-shell-filter-applied.png` | 6 | Results after Apply, incremental |
| `14-copy-filtered-shells.png` | 6.1 | new scenario + confirmation message |
| `15-pushback-solids.png` | 6.1 | six pushback solids in 3D |
| `16-pick-start.png` | 7 | plan view while picking the start point |
| `17-radial-shells.png` | 7 | five radial surfaces in plan |
| `18-west-shells.png` | 8 | five west-front surfaces, next to radial |
| `19-bm-pushback-field.png` | 9 | BM coloured by `pushback`, legend with shares |
| `20-bm-pushback-section.png` | 9 | east–west section coloured by `pushback` |
| `21-report-csv.png` | 10 | `optimization_results.csv` in a spreadsheet |

The `c01`–`c07` `.svg` files are the concept diagrams. They are generated, so don't edit them by hand: change
`../figures.py` and run

```
python3 docs/manuals/optimization/figures.py
```
