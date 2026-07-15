# Deliberately invalid R script for figcheck's ggplot2 backend self-test:
# references a data frame column that does not exist, so ggplot2 must
# raise an error and Rscript must exit non-zero.
library(ggplot2)

df <- data.frame(x = c(0, 1, 2), y = c(0, 1, 4))
p <- ggplot(df, aes(x = x, y = this_column_is_never_defined)) + geom_line()
print(p)
