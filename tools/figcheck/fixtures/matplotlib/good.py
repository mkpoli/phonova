"""Minimal matplotlib script for figcheck's matplotlib backend self-test.

Writes good.png into $FIGCHECK_OUTDIR (or the working directory).
"""

import os

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

outdir = os.environ.get("FIGCHECK_OUTDIR", ".")
os.makedirs(outdir, exist_ok=True)

fig, ax = plt.subplots(figsize=(2, 2))
ax.plot([0, 1, 2], [0, 1, 4])
ax.set_xlabel("x")
ax.set_ylabel("y")
fig.savefig(os.path.join(outdir, "good.png"))
