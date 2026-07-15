# figcheck toolchain container: everything tools/figcheck/backends/*.sh
# needs to actually compile/execute a text-emission figure export --
# TeX Live (latexmk + pgfplots) for TikZ, typst, Python+matplotlib,
# R+ggplot2, Julia+CairoMakie. Vega-Lite validation only needs the schema
# vendored under fixtures/vega/schema/ plus the same Python interpreter, so
# it needs nothing extra beyond matplotlib's Python.
#
# Built and run by the `figcheck` CI job (.github/workflows/ci.yml); build
# failure (registry outage, package removed upstream, ...) is caught by
# that job and reported as a skip, not a CI failure.
FROM debian:bookworm-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818

ENV DEBIAN_FRONTEND=noninteractive

# TeX Live (small scheme + the pieces TikZ/PGFPlots need) and R+ggplot2
# come from the distro archive, pinned to whatever bookworm ships --
# acceptable here since this container is rebuilt on every run, never
# cached long-term.
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl xz-utils \
        latexmk texlive-latex-base texlive-latex-recommended \
        texlive-pictures texlive-latex-extra \
        python3 python3-pip \
        r-base-core r-cran-ggplot2 \
    && rm -rf /var/lib/apt/lists/*

RUN pip3 install --no-cache-dir --break-system-packages matplotlib jsonschema

# typst: single static binary, pinned by release version + checksum.
ARG TYPST_VERSION=0.14.2
RUN curl -fsSL \
        "https://github.com/typst/typst/releases/download/v${TYPST_VERSION}/typst-x86_64-unknown-linux-musl.tar.xz" \
        -o /tmp/typst.tar.xz \
    && tar -xJf /tmp/typst.tar.xz -C /tmp \
    && install -m755 "/tmp/typst-x86_64-unknown-linux-musl/typst" /usr/local/bin/typst \
    && rm -rf /tmp/typst.tar.xz "/tmp/typst-x86_64-unknown-linux-musl"

# Julia + CairoMakie (the software-rasterizing Makie backend -- this
# container has no GPU/display for GLMakie). This is the heaviest layer:
# the Julia tarball alone is ~290 MB, and CairoMakie's first `Pkg.add`
# resolves and precompiles Cairo/Pango/HarfBuzz/FreeType artifacts on top
# of that. Expect this stage to dominate both image size and build time.
ARG JULIA_VERSION=1.11.7
RUN curl -fsSL \
        "https://julialang-s3.julialang.org/bin/linux/x64/${JULIA_VERSION%.*}/julia-${JULIA_VERSION}-linux-x86_64.tar.gz" \
        -o /tmp/julia.tar.gz \
    && tar -xzf /tmp/julia.tar.gz -C /opt \
    && ln -s "/opt/julia-${JULIA_VERSION}/bin/julia" /usr/local/bin/julia \
    && rm -f /tmp/julia.tar.gz \
    && julia -e 'using Pkg; Pkg.add("CairoMakie"); using CairoMakie'

WORKDIR /work
