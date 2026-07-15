"""Deliberately invalid matplotlib script for figcheck's matplotlib backend
self-test: plots against a name that was never defined, so Python must
raise NameError and exit non-zero.
"""

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

fig, ax = plt.subplots()
ax.plot(this_name_is_never_defined)
