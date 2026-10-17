# Bundled solver provenance and notices

The default build bundles no native solver. This document covers the ones a
build can opt into.

## SCIP

Improve's backend, built only with the `scip-source` feature: russcip
**0.10.0** and scip-sys **0.1.28** (pinned exactly) download the
`scipoptsuite-10.0.2.tgz` source release from
<https://github.com/scipopt/scip/releases/tag/v10.0.2>, build SCIP **10.0.2**
and SoPlex **8.0.2** with cmake, and link both statically into the Incline
binary. The build's options (`scip-sys` `from_source.rs`) leave out Ipopt,
Boost, GMP, ZIMPL, PaPILO, zlib, readline, GCG and UG, so none of them, nor
Ipopt's MUMPS, METIS or LAPACK, is in the binary. It does compile in the AMPL
`.nl` reader (AMPL/MP), CppAD (the expression interpreter), nauty with dejavu
(symmetry, `SYM=snauty`) and TinyCThread (`TPI=tny`).

`scip-system` instead links whatever SCIP the environment provides; that
build's notices are whatever that installation ships.

### Collected upstream licence texts

Texts under `docs/licenses/scip/` are copied without modification from the
SCIP 10.0.2 source release:

| Component | Source | Local notice |
| --- | --- | --- |
| SCIP | LICENSE (Apache-2.0) | Apache-2.0.txt |
| SoPlex | soplex/LICENSE (Apache-2.0) | SoPlex.txt |
| AMPL/MP | src/amplmp/LICENSE.rst | AMPL-MP.txt |
| CppAD | src/cppad/COPYING (EPL-1.0) | CppAD.txt |
| nauty | src/nauty/COPYRIGHT | Nauty.txt |
| dejavu | src/dejavu/LICENSE (MIT) | dejavu.txt |
| TinyCThread | src/tinycthread/COPYRIGHT | TinyCThread.txt |

SCIP's own `tclique`, `dijkstra`, `xml`, `rectlu` and `blockmemshell` are
Apache-2.0 under SCIP's licence. SCIP copyright: Zuse Institute Berlin,
2002–2026.

### Before distributing it

The static binary still links the C++ runtime (`libstdc++`, `libgcc_s`)
dynamically, as every C++ program does; those carry the GCC runtime library
exception. CppAD is under the Eclipse Public License 1.0: a distribution must
keep its notice and say where its source can be obtained (the SCIP source
release above). Windows and macOS builds of `scip-source` are unverified.

## HiGHS

The relaxation bound, and with the `highs` feature the hourly dispatch LP,
use HiGHS **1.15.0**, built from the source vendored in the `highs-sys` 1.15.0
crate and linked statically into the Incline binary of a native build with
the `highs` or `scip` feature. The default build has no HiGHS. Of its `extern/`
components only pdqsort is compiled in (header-only); the build sets
`ZLIB=OFF` and `HIPO` is off, so zstr, AMD, METIS, RCM and the BLAS shim are
not linked. Texts under `docs/licenses/highs/` are copied without
modification from that crate:

| Component | Version/source | Local notice |
| --- | --- | --- |
| HiGHS | highs-sys 1.15.0 HiGHS/LICENSE.txt (MIT) | HiGHS.txt |
| pdqsort | highs-sys 1.15.0 HiGHS/extern/pdqsort/license.txt (zlib) | pdqsort.txt |

MIT requires the HiGHS copyright and permission notice in every copy, so a
distribution must ship `HiGHS.txt` alongside the binary. pdqsort's zlib
licence asks for no notice in binary form.
