# Minimal base-ggplot2 script for figcheck's ggplot2 backend self-test.
# Writes good.png into $FIGCHECK_OUTDIR (or the working directory).
library(ggplot2)

outdir <- Sys.getenv("FIGCHECK_OUTDIR", ".")
dir.create(outdir, showWarnings = FALSE, recursive = TRUE)

df <- data.frame(x = c(0, 1, 2), y = c(0, 1, 4))
p <- ggplot(df, aes(x = x, y = y)) + geom_line() + geom_point()
ggsave(file.path(outdir, "good.png"), plot = p, width = 2, height = 2)
