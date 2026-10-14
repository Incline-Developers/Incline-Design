"""Concept figures for the pit optimization manual.

Run from anywhere:  python3 docs/manuals/optimization/figures.py
Writes images/c0*.svg next to this file. Needs matplotlib and numpy.

The pits are not drawn by hand: they are solved on a small made-up 2-D block
model with the classic 45-degree dynamic programme (a section through a pit),
so nested shells really nest and the pit-by-pit chart is a real sensitivity -
of the toy model, not of McLaughlin.
"""

import os
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
from matplotlib import patheffects
from matplotlib.patches import FancyArrowPatch, FancyBboxPatch, Rectangle, Wedge

OUT = Path(__file__).resolve().parent / "images"
OUT.mkdir(exist_ok=True)

plt.rcParams.update(
    {
        "svg.fonttype": "path",
        "font.family": "DejaVu Sans",
        "font.size": 10,
        "axes.edgecolor": "#555555",
        "axes.labelcolor": "#333333",
        "xtick.color": "#555555",
        "ytick.color": "#555555",
    }
)

INK = "#2B2B2B"
MUTED = "#6B6B6B"
GRID = "#D9D9D9"
AIR = "#EEF4FA"
WASTE = "#C9CDD2"
ORE = "#E3A21A"
CASH = "#2E9E5B"
GRADE = "#8E6CCF"
PIT = "#C0392B"
SHELLS = ["#4C78A8", "#59A14F", "#EDC948", "#F28E2B", "#E15759", "#B07AA1"]


def save(fig, name):
    fig.savefig(OUT / name, format="svg", bbox_inches="tight", pad_inches=0.15, facecolor="white")
    if os.environ.get("FIGURES_PNG"):
        fig.savefig(Path(os.environ["FIGURES_PNG"]) / name.replace(".svg", ".png"), dpi=110, bbox_inches="tight", facecolor="white")
    plt.close(fig)
    print("wrote", OUT / name)


# ── 2-D optimal pit (45 degrees, square blocks) ──


def optimal_pit(values):
    """Depth mined in each column of the most valuable 45-degree pit.

    `values[row, col]`, row 0 at the top. A column may be at most one block
    deeper or shallower than its neighbours, and the pit starts and ends at the
    surface outside the grid. Ties go to the shallower pit (smallest optimal).
    """
    rows, cols = values.shape
    cum = np.vstack([np.zeros(cols), np.cumsum(values, axis=0)])  # cum[k, j]: value of the top k blocks
    neg = -1e18
    best = np.full((cols, rows + 1), neg)
    came = np.zeros((cols, rows + 1), dtype=int)
    for k in range(rows + 1):
        if k <= 1:
            best[0, k] = cum[k, 0]
    for j in range(1, cols):
        for k in range(rows + 1):
            for prev in (k - 1, k, k + 1):
                if 0 <= prev <= rows and best[j - 1, prev] > neg / 2:
                    total = best[j - 1, prev] + cum[k, j]
                    if total > best[j, k] + 1e-9:
                        best[j, k] = total
                        came[j, k] = prev
    end = max(range(2), key=lambda k: (best[cols - 1, k], -k))
    depths = [0] * cols
    k = end
    for j in range(cols - 1, -1, -1):
        depths[j] = k
        k = came[j, k] if j > 0 else 0
    return np.array(depths), best[cols - 1, end]


def pit_outline(depths):
    """The pit's lower edge as a polyline over block corners (x right, y down),
    from where the pit leaves the surface to where it meets it again."""
    mined = np.nonzero(depths)[0]
    if len(mined) == 0:
        return np.array([]), np.array([])
    first, last = mined[0], mined[-1]
    xs, ys = [first], [0]
    for j in range(first, last + 1):
        xs += [j, j + 1]
        ys += [depths[j], depths[j]]
    xs.append(last + 1)
    ys.append(0)
    return np.array(xs), np.array(ys)


def draw_outline(ax, depths, color, width=2.2, label=None, z=5):
    xs, ys = pit_outline(depths)
    ax.plot(xs, ys, color=color, lw=width, solid_joinstyle="miter", label=label, zorder=z)


# ── The toy deposit used by C4 and C5 ──

ROWS, COLS = 30, 64


def toy_grades():
    r, c = np.mgrid[0:ROWS, 0:COLS]
    # An outcropping body that dips east and thins with depth, a small lens to
    # the west and a deep lens to the east, which joins the pit in one jump.
    main = 3.0 * np.exp(-(((c - 30 - 0.45 * r) / (4.0 + 0.12 * r)) ** 2) - ((r - 5) / 12.0) ** 2)
    west = 2.4 * np.exp(-(((c - 12) / 3.0) ** 2) - ((r - 8) / 2.6) ** 2)
    deep = 2.6 * np.exp(-(((c - 50) / 3.2) ** 2) - ((r - 23) / 3.2) ** 2)
    g = main + west + deep
    g[g < 0.25] = 0.0
    return g


def toy_values(grades, rf):
    """Cash flow per block: waste costs 1; ore pays rf x 4 per grade unit less 2.2 processing."""
    waste = -1.0
    ore = rf * 4.0 * grades - 1.0 - 2.2
    return np.maximum(waste, ore)


# ── C1 · The ultimate pit problem ──


def forced_loss(values, blocks):
    """How much less the best pit is worth when it must also take `blocks`."""
    _, best = optimal_pit(values)
    forced = values.copy()
    for block in blocks:
        forced[block] += 1000.0
    _, with_blocks = optimal_pit(forced)
    return best - (with_blocks - 1000.0 * len(blocks))


def c01():
    rows, cols = 9, 30
    v = np.full((rows, cols), -1.0)
    lens = {
        (2, 15): 3, (2, 16): 4, (2, 17): 3,
        (3, 14): 2, (3, 15): 6, (3, 16): 9, (3, 17): 7, (3, 18): 2,
        (4, 15): 5, (4, 16): 12, (4, 17): 10, (4, 18): 4,
        (5, 16): 8, (5, 17): 9,
        (6, 16): 4, (6, 17): 5,
    }
    deep_west, deep_east, floor = (7, 10), (8, 20), [(6, 16), (6, 17)]
    for block, x in {**lens, deep_west: 14, deep_east: 10}.items():
        v[block] = x
    depths, total = optimal_pit(v)
    losses = [forced_loss(v, [deep_west]), forced_loss(v, [deep_east]), forced_loss(v, floor)]

    fig, ax = plt.subplots(figsize=(13, 5.0))
    for r in range(rows):
        for c in range(cols):
            mined = r < depths[c]
            val = v[r, c]
            face = ORE if val > 0 else WASTE
            alpha = 0.95 if mined else 0.35
            ax.add_patch(Rectangle((c, r), 1, 1, facecolor=face, alpha=alpha, edgecolor="white", lw=1.2))
            ax.text(c + 0.5, r + 0.52, f"{val:+.0f}", ha="center", va="center", fontsize=8,
                    color=INK if mined else MUTED, weight="bold" if val > 0 else "normal")
    draw_outline(ax, depths, PIT, width=2.8)
    for block in (deep_west, deep_east):
        ax.add_patch(Rectangle((block[1], block[0]), 1, 1, fill=False, edgecolor=INK, lw=2.0, ls="--", zorder=6))
    ax.add_patch(Rectangle((16, 6), 2, 1, fill=False, edgecolor=INK, lw=2.0, ls="--", zorder=6))
    note = dict(fontsize=9.5, color=INK, arrowprops=dict(arrowstyle="->", color=INK, lw=1.2), annotation_clip=False)
    ax.annotate(f"+14: the pit would be worth {losses[0]:.0f} less", xy=(deep_west[1] + 0.5, rows), xytext=(2.0, rows + 1.4), **note)
    ax.annotate(f"+4 and +5: {losses[2]:.0f} less", xy=(17, 7), xytext=(13.2, rows + 1.4), **note)
    ax.annotate(f"+10: {losses[1]:.0f} less", xy=(deep_east[1] + 0.5, rows), xytext=(22.0, rows + 1.4), **note)
    ax.text(cols / 2, -0.55, f"Optimal pit (red): total value {total:+.0f}", ha="center", va="bottom", fontsize=11, color=PIT,
            weight="bold")
    ax.set_xlim(0, cols)
    ax.set_ylim(rows + 2.6, -1.2)
    ax.set_aspect("equal")
    ax.axis("off")
    ax.text(0, rows + 2.4, "Section through a block model. Numbers are block values; walls may not be steeper than 45°. "
            "Dashed: ore left behind, with what taking it would cost.", fontsize=9, color=MUTED, va="top")
    save(fig, "c01-ultimate-pit.svg")


# ── C2 · Block value, step by step ──


def box(ax, x, y, w, h, title, lines, face, edge, title_color=INK):
    ax.add_patch(FancyBboxPatch((x, y), w, h, boxstyle="round,pad=0.02,rounding_size=0.12", facecolor=face, edgecolor=edge, lw=1.6))
    ax.text(x + w / 2, y + h - 0.28, title, ha="center", va="top", fontsize=11, weight="bold", color=title_color)
    for i, line in enumerate(lines):
        ax.text(x + 0.18, y + h - 0.72 - i * 0.33, line, ha="left", va="top", fontsize=8.8, color=INK, family="DejaVu Sans Mono")


def arrow(ax, a, b, color=MUTED):
    ax.add_patch(FancyArrowPatch(a, b, arrowstyle="-|>", mutation_scale=14, color=color, lw=1.5))


def c02():
    fig, ax = plt.subplots(figsize=(11, 8.2))
    ax.set_xlim(0, 11)
    ax.set_ylim(0, 8.2)
    ax.axis("off")

    box(ax, 3.4, 6.8, 4.2, 1.2, "Block A", ["955.7 t, 3.0 g/t au, hard rock"], "#F4F4F4", "#9A9A9A")

    box(ax, 0.1, 2.55, 3.4, 3.3, "Waste", [
        "mining (hard) 1.45 /t",
        "waste haul    1.21 /t",
        "rehab         0.34 /t",
        "",
        "-955.7 x 3.00",
        "= -2,867",
    ], "#EEF0F2", "#9AA0A6")
    box(ax, 3.75, 2.55, 3.5, 3.3, "Mill  (1.5 g/t and up)", [
        "gold 955.7 x (3.0-0.12)",
        "  x 0.03215 x 84% = 74.3 oz",
        "revenue 74.3 x (810-64.2)",
        "            = +55,438",
        "costs 955.7 x 29.77 = -28,451",
        "element cost         -1",
        "= +26,985",
    ], "#FFF6E3", ORE)
    box(ax, 7.5, 2.55, 3.4, 3.3, "Heap leach (0.3 - 1.5)", [
        "grade 3.0 g/t is above",
        "the window: this method",
        "cannot take the block",
        "",
        "",
        "= not eligible",
    ], "#F7F7F7", "#BDBDBD", title_color=MUTED)

    for x in (1.8, 5.5, 9.2):
        arrow(ax, (5.5, 6.8), (x, 5.88))

    box(ax, 2.9, 0.25, 5.2, 1.7, "Highest value wins", [
        "max(-2,867, +26,985) = +26,985",
        "destination: Mill",
        "(waste if every method is worse)",
    ], "#E9F6EE", CASH, title_color=CASH)
    for x in (1.8, 5.5):
        arrow(ax, (x, 2.55), (5.5, 1.98))
    arrow(ax, (9.2, 2.55), (5.5, 1.98), color="#CFCFCF")
    save(fig, "c02-block-value.svg")


# ── C3 · Slope cone and rosette ──


def c03():
    fig, (left, right) = plt.subplots(1, 2, figsize=(12, 5.4), gridspec_kw={"width_ratios": [1.45, 1]})

    rows, cols = 9, 17
    target = (8, 8)
    for r in range(rows):
        for c in range(cols):
            need = abs(c - target[1]) <= target[0] - r
            face = "#F2C46D" if need else "#ECEEF0"
            if (r, c) == target:
                face = PIT
            left.add_patch(Rectangle((c, r), 1, 1, facecolor=face, edgecolor="white", lw=1.2))
    apex_x = target[1] + 0.5
    left.plot([apex_x - 9, apex_x, apex_x + 9], [-0.5, rows - 0.5, -0.5], color=INK, lw=1.6, ls="--")
    left.text(apex_x, rows - 0.5 + 0.05, "", ha="center")
    left.annotate("block to mine", xy=(apex_x, rows - 0.5), xytext=(apex_x + 3.6, rows + 0.4), fontsize=10, color=PIT,
                  arrowprops=dict(arrowstyle="->", color=PIT))
    left.text(apex_x, 2.6, "must be mined first", ha="center", fontsize=11, color=INK, weight="bold")
    left.text(apex_x - 5.6, 5.4, "45°", fontsize=12, color=INK, weight="bold")
    left.set_xlim(0, cols)
    left.set_ylim(rows + 1, -0.6)
    left.set_aspect("equal")
    left.axis("off")
    left.set_title("Section: the cone above one block", fontsize=12, color=INK, pad=8)

    sectors = [(0, 90, 48, "#A7C7E7"), (90, 180, 45, "#B9DDB1"), (180, 360, 54, "#F3D19C")]
    for b0, b1, angle, color in sectors:
        right.add_patch(Wedge((0, 0), 1.0, 90 - b1, 90 - b0, facecolor=color, edgecolor="white", lw=2))
        mid = np.radians(90 - (b0 + b1) / 2)
        right.text(0.58 * np.cos(mid), 0.58 * np.sin(mid), f"{angle}°", ha="center", va="center", fontsize=15, weight="bold",
                   color=INK)
        right.text(0.58 * np.cos(mid), 0.58 * np.sin(mid) - 0.15, f"bearing {b0}°–{b1}°", ha="center", va="center", fontsize=8.5,
                   color=MUTED)
    for name, bearing in (("N", 0), ("E", 90), ("S", 180), ("W", 270)):
        t = np.radians(90 - bearing)
        right.text(1.13 * np.cos(t), 1.13 * np.sin(t), name, ha="center", va="center", fontsize=12, weight="bold", color=INK)
    right.set_xlim(-1.3, 1.3)
    right.set_ylim(-1.3, 1.3)
    right.set_aspect("equal")
    right.axis("off")
    right.set_title("Plan: a rosette, angle by bearing", fontsize=12, color=INK, pad=8)
    save(fig, "c03-slope-cone.svg")


# ── C4 · Nested shells ──

C4_FACTORS = [0.35, 0.55, 0.70, 0.975, 1.30]


def c04():
    g = toy_grades()
    fig, ax = plt.subplots(figsize=(12, 6.4))
    ax.add_patch(Rectangle((0, 0), COLS, ROWS, facecolor="#F1F2F4", edgecolor="#BBBBBB", lw=1, zorder=0))
    shown = np.ma.masked_where(g <= 0, g)
    image = ax.imshow(shown, cmap="turbo", vmin=0, vmax=3.4, extent=(0, COLS, ROWS, 0), interpolation="nearest", alpha=0.9,
                      zorder=1)
    halo = [patheffects.Stroke(linewidth=4.6, foreground="white"), patheffects.Normal()]
    for i, rf in enumerate(C4_FACTORS):
        depths, _ = optimal_pit(toy_values(g, rf))
        xs, ys = pit_outline(depths)
        ax.plot(xs, ys, color=INK, lw=2.0, solid_joinstyle="miter", zorder=5 + i, path_effects=halo)
        # Label where the pit's east wall meets the surface: those points are spread out.
        ax.text(xs[-1], -0.4, f"RAF {rf:g}", ha="center", va="bottom", rotation=90, fontsize=9, weight="bold", color=INK,
                zorder=12)
    ax.set_xlim(0, COLS)
    ax.set_ylim(ROWS, -5.5)
    ax.set_aspect("equal")
    ax.axis("off")
    bar = fig.colorbar(image, ax=ax, fraction=0.025, pad=0.01)
    bar.set_label("grade", color=INK)
    bar.outline.set_visible(False)
    ax.annotate("", xy=(18, 27.5), xytext=(4, 27.5), arrowprops=dict(arrowstyle="-|>", color=INK, lw=1.8), zorder=12)
    ax.text(11, 26.6, "higher price, bigger pit", ha="center", fontsize=10, color=INK, zorder=12)
    ax.text(0, ROWS + 1.5, "Section through a block model, coloured by grade (blue low, red high). Each line is the optimal pit at "
            "one revenue factor.", fontsize=9, color=MUTED, va="top")
    save(fig, "c04-nested-shells.svg")


# ── C5 · Reading a pit-by-pit chart ──

C5_CHOSEN = [0.35, 0.55, 0.70, 0.975, 1.30, 1.425]


def c05():
    g = toy_grades()
    base = toy_values(g, 1.0)
    is_ore = base > -1.0 + 1e-9
    factors = np.round(np.arange(0.2, 1.7001, 0.025), 3)
    ore_t, waste_t, cash = [], [], []
    for rf in factors:
        depths, _ = optimal_pit(toy_values(g, rf))
        mask = np.arange(ROWS)[:, None] < depths[None, :]
        ore_t.append((mask & is_ore).sum() * 1.0)
        waste_t.append((mask & ~is_ore).sum() * 1.0)
        cash.append(base[mask].sum())
    ore_t, waste_t, cash = map(np.array, (ore_t, waste_t, cash))
    x = np.arange(len(factors))

    fig, ax = plt.subplots(figsize=(13, 6.0))
    ax.bar(x, ore_t, color=ORE, width=0.8, label="Ore tonnes")
    ax.bar(x, waste_t, bottom=ore_t, color=WASTE, width=0.8, label="Waste tonnes")
    ax.set_ylabel("Tonnes (blocks)")
    ax.set_xticks(x[::4])
    ax.set_xticklabels([f"{f:g}" for f in factors[::4]])
    ax.set_xlabel("Revenue factor of the shell")
    ax.spines[["top"]].set_visible(False)
    ax2 = ax.twinx()
    ax2.plot(x, cash, color=CASH, lw=2.4, label="Cash flow at base price")
    ax2.set_ylabel("Cash flow (undiscounted, RAF 1)", color=CASH)
    ax2.tick_params(axis="y", colors=CASH)
    ax2.spines[["top"]].set_visible(False)

    total = ore_t + waste_t
    i = int(np.argmax(np.diff(total))) + 1
    ax.annotate("jump: a new zone opens", xy=(i, total[i]), xytext=(i - 14, total.max() * 1.12), fontsize=10, color=PIT,
                weight="bold", arrowprops=dict(arrowstyle="->", color=PIT, lw=1.4))
    peak = int(np.argmax(cash))
    ax2.plot([peak], [cash[peak]], "o", color=CASH, ms=8, zorder=6)
    ax2.annotate("cash flow peaks", xy=(peak, cash[peak]), xytext=(peak - 9, cash[peak] * 0.72), fontsize=10, color=CASH,
                 weight="bold", arrowprops=dict(arrowstyle="->", color=CASH, lw=1.4))

    chosen = [int(np.argmin(abs(factors - f))) for f in C5_CHOSEN]
    lo = -total.max() * 0.2
    ax.set_ylim(lo, total.max() * 1.25)
    start = -0.5
    for k, i in enumerate(chosen):
        ax.add_patch(Rectangle((i - 0.45, 0), 0.9, total[i], fill=False, edgecolor=INK, lw=1.4, zorder=4))
        end = i + 0.45
        y = lo * 0.45
        ax.plot([start + 0.15, end - 0.15], [y, y], color=SHELLS[k % len(SHELLS)], lw=5, solid_capstyle="butt")
        ax.text((start + end) / 2, y * 1.85, f"PB{k + 1}", ha="center", va="center", fontsize=9, color=INK)
        start = end
    ax.axhline(0, color="#888888", lw=0.8)

    h1, l1 = ax.get_legend_handles_labels()
    h2, l2 = ax2.get_legend_handles_labels()
    ax.legend(h1 + h2, l1 + l2, loc="upper left", frameon=False)
    ax2.set_ylim(min(0, cash.min()) - abs(cash.max()) * 0.25, cash.max() * 1.25)
    ax.set_title("Illustration on a toy model: outlined columns are the chosen shells, coloured bars the pushbacks between them",
                 fontsize=10, color=MUTED, loc="left")
    save(fig, "c05-pit-by-pit.svg")


# ── C6 · Radial and straight fronts ──


def c06():
    """The zones where each of five directional shells may take ore, measured as Incline does:
    distance from the point (radial) or along the bearing (west), scaled over the final pit."""
    ny, nx = 26, 40
    yy, xx = np.mgrid[0:ny, 0:nx]
    cx, cy = 20, 13
    wobble = 1 + 0.10 * np.sin(np.arctan2(yy - cy, xx - cx) * 3)
    pit = (((xx - cx) / 17.0) ** 2 + ((yy - cy) / 10.5) ** 2) <= wobble ** 2
    fig, axes = plt.subplots(1, 2, figsize=(13, 5.2))

    def panel(ax, dst, start, title, guides):
        lo, hi = dst[pit].min(), dst[pit].max()
        share = (dst - lo) / (hi - lo)
        for r in range(ny):
            for c in range(nx):
                if not pit[r, c]:
                    ax.add_patch(Rectangle((c, r), 1, 1, facecolor="#F3F3F3", edgecolor="white", lw=0.6))
                    continue
                k = max(min(int(np.ceil(share[r, c] * 5 - 1e-9)), 5), 1)
                ax.add_patch(Rectangle((c, r), 1, 1, facecolor=SHELLS[k - 1], alpha=0.85, edgecolor="white", lw=0.6))
        guides(ax)
        ax.plot([start[0]], [start[1]], marker="*", ms=22, color=INK, mec="white", mew=1.5, zorder=8)
        ax.set_xlim(0, nx)
        ax.set_ylim(ny, 0)
        ax.set_aspect("equal")
        ax.axis("off")
        ax.set_title(title, fontsize=12, color=INK)

    sx, sy = 20.5, 13.5
    radial = np.hypot(xx + 0.5 - sx, yy + 0.5 - sy)
    panel(axes[0], radial, (sx, sy), "Radial (out from the point)", lambda ax: None)

    ex, ey = 37.5, 13.5
    west = ex - (xx + 0.5)

    def west_guides(ax):
        ax.annotate("", xy=(6, ny + 1.2), xytext=(34, ny + 1.2), arrowprops=dict(arrowstyle="-|>", color=INK, lw=1.8),
                    annotation_clip=False)
        ax.text(20, ny + 2.6, "front moves west", ha="center", va="top", fontsize=10, color=INK)

    panel(axes[1], west, (ex, ey), "Mining direction: West", west_guides)

    handles = [Rectangle((0, 0), 1, 1, facecolor=SHELLS[k], alpha=0.85) for k in range(5)]
    fig.legend(handles, [f"ore allowed from shell {k + 1}  ({(k + 1) * 20}%)" for k in range(5)], loc="lower center", ncol=5,
               frameon=False, fontsize=9.5, bbox_to_anchor=(0.5, 0.02))
    fig.text(0.5, 0.965, "Plan of the final pit. ★ = starting point. Shell k may take ore only in the zones up to k/5 of the way across.",
             ha="center", fontsize=9.5, color=MUTED)
    save(fig, "c06-directional.svg")


# ── C7 · Air and topography ──


def c07():
    rows, cols = 10, 26
    x = np.arange(cols) + 0.5
    topo = 3.2 + 1.6 * np.sin(x / 4.0) + 0.6 * np.sin(x / 1.7)
    fig, ax = plt.subplots(figsize=(11, 4.6))
    for r in range(rows):
        for c in range(cols):
            centre = r + 0.5
            if c >= cols - 3 and r < 6:
                ax.add_patch(Rectangle((c, r), 1, 1, facecolor="white", edgecolor="#DDDDDD", lw=0.8, hatch="..."))
                continue
            face = AIR if centre < topo[c] else WASTE
            ax.add_patch(Rectangle((c, r), 1, 1, facecolor=face, edgecolor="white", lw=1.0))
    ax.plot(np.r_[0, x, cols], np.r_[topo[0], topo, topo[-1]], color="#7A5230", lw=2.6, label="topography")
    ax.text(5.0, 0.9, "air: costs nothing, earns nothing", fontsize=10, color="#3B6E99", weight="bold")
    ax.text(5.0, 8.2, "rock: valued as waste or ore", fontsize=10, color=INK, weight="bold")
    ax.annotate("no block at all:\ntreated as air", xy=(cols - 1.5, 3), xytext=(cols - 8.5, 0.9), fontsize=9.5, color=INK,
                arrowprops=dict(arrowstyle="->", color=INK))
    ax.set_xlim(0, cols)
    ax.set_ylim(rows, 0)
    ax.set_aspect("equal")
    ax.axis("off")
    ax.legend(loc="lower right", frameon=False)
    save(fig, "c07-air.svg")


if __name__ == "__main__":
    c01()
    c02()
    c03()
    c04()
    c05()
    c06()
    c07()
