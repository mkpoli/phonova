# Deliberately invalid Julia script for figcheck's makie backend self-test:
# plots against a name that was never defined, so Julia must raise
# UndefVarError and exit non-zero.
using CairoMakie

fig = Figure(size = (200, 200))
ax = Axis(fig[1, 1])
lines!(ax, this_name_is_never_defined)
