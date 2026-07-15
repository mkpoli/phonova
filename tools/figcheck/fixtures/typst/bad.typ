// Deliberately invalid Typst document for figcheck's typst backend
// self-test: calls an undefined function so `typst compile` must fail.
#set page(width: 4cm, height: 4cm, margin: 0.5cm)
#this-function-does-not-exist("figcheck")
