# Bundled solver provenance and notices

Supported development backend: russcip **0.10.0**, scip-sys **0.1.28**, bundled
SCIP **10.0.2** / SoPlex **8.0.2**, scipoptsuite-deploy **v0.12.0**. Rust
dependencies are pinned exactly. Upstream deploy revision:
`4d836440d0f7ee60387fb7681c83faf6c38b80cd`.

The [release](https://github.com/scipopt/scipoptsuite-deploy/releases/tag/v0.12.0)
and [Linux build recipe](https://github.com/scipopt/scipoptsuite-deploy/blob/4d836440d0f7ee60387fb7681c83faf6c38b80cd/.github/workflows/scripts/linux.bash)
are the provenance sources. The archive contains other libraries/tools (including
GCG) that Incline does not need to ship. Do not redistribute the entire archive
without auditing those additional components.

## Collected upstream licence texts

Texts under `docs/licenses/scip/` are copied without modification from:

| Component | Version/source | Local notice |
| --- | --- | --- |
| SCIP | scipopt/scip v10.0.2 LICENSE | Apache-2.0.txt |
| SoPlex | scipopt/soplex v8.0.2 LICENSE | SoPlex.txt |
| AMPL/MP | SCIP v10.0.2 src/amplmp/LICENSE.rst | AMPL-MP.txt |
| CppAD | SCIP v10.0.2 src/cppad/COPYING | CppAD.txt |
| Nauty | SCIP v10.0.2 src/nauty/COPYRIGHT | Nauty.txt |
| TinyCThread | SCIP v10.0.2 src/tinycthread/COPYRIGHT | TinyCThread.txt |
| Ipopt | coin-or/Ipopt releases/3.14.19 LICENSE | Ipopt.txt |
| Boost | boostorg/boost boost-1.82.0 LICENSE_1_0.txt | Boost.txt |
| METIS | KarypisLab/METIS v5.1.1-DistDGL-v0.5 LICENSE | METIS.txt |
| GKlib | KarypisLab/GKlib METIS-v5.1.1-DistDGL-0.5 LICENSE.txt | GKlib.txt |

The SCIP executable's banner reports SoPlex 8.0.2, CppAD 20180000.0,
zlib 1.2.11, TinyCThread 1.2, AMPL/MP 4.0.4, Nauty 2.8.8, sassy 2.1,
and Ipopt 3.14.19. SCIP copyright: Zuse Institute Berlin, 2002–2026.

## Distribution audit still required

This collection is **not a complete redistribution clearance**. Still resolve
the exact sassy and zlib notices and source/header attributions. The upstream
Linux recipe clones Reference-LAPACK and ThirdParty-Mumps without a commit/tag,
so the release tag alone does not identify their exact incorporated revisions.
Recover their build provenance, licences and applicable source-offer obligations
before shipping the prebuilt library. Also audit all bundled compiler/runtime
libraries if shipping them, rather than relying on the recipient's installation.
CppAD and Ipopt include Eclipse Public Licences: retain their texts and satisfy
applicable source availability requirements for redistributed components.

Linux direct launch and dynamic-library resolution were validated on the host;
Windows/macOS packaging and a clean Linux installation remain unverified.
The build's developer RPATH is not an installer. An installer must include the
matching library/dependencies and notices in its distributable artefact.

## HiGHS

The hourly dispatch LP and the relaxation bound use HiGHS **1.15.0**, built
from the source vendored in the `highs-sys` 1.15.0 crate and linked
statically into the Incline binary on every native build. Of its `extern/`
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
