# Minimal CairoMakie script for figcheck's makie backend self-test.
# Writes good.png into ENV["FIGCHECK_OUTDIR"] (or the working directory).
using CairoMakie

outdir = get(ENV, "FIGCHECK_OUTDIR", ".")
mkpath(outdir)

fig = Figure(size = (200, 200))
ax = Axis(fig[1, 1], xlabel = "x", ylabel = "y")
lines!(ax, [0, 1, 2], [0, 1, 4])
save(joinpath(outdir, "good.png"), fig)
