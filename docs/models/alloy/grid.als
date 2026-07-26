module grid

open util/integer

abstract sig CharClass {}
one sig TextChar, WhitespaceChar, NullChar, UrlChar extends CharClass {}

sig Char {
  class: one CharClass
}

sig Cell {
  char: one Char,
  row: one Int,
  col: one Int
}

sig Grid {
  rows: one Int,
  cols: one Int,
  cells: some Cell
}

fact cell_bounds {
  all g: Grid, c: g.cells |
    c.row >= 0 and c.row < g.rows and
    c.col >= 0 and c.col < g.cols
}

fact cell_uniqueness {
  all g: Grid, disj c1, c2: g.cells |
    not (c1.row = c2.row and c1.col = c2.col)
}

fun cellAt[g: Grid, r, c: Int]: lone Cell {
  { cell: g.cells | cell.row = r and cell.col = c }
}

pred isEmpty[g: Grid, r, c: Int] {
  let cell = cellAt[g, r, c] {
    no cell or cell.char.class = NullChar
  }
}

pred isWhitespace[g: Grid, r, c: Int] {
  let cell = cellAt[g, r, c] {
    some cell and cell.char.class = WhitespaceChar
  }
}

pred isText[g: Grid, r, c: Int] {
  let cell = cellAt[g, r, c] {
    some cell and cell.char.class = TextChar
  }
}

assert grid_has_at_least_one_cell {
  all g: Grid | #g.cells >= 1
}

assert cell_positions_are_unique {
  all g: Grid |
    all disj c1, c2: g.cells |
      c1.row != c2.row or c1.col != c2.col
}

assert cell_bounds_respected {
  all g: Grid, c: g.cells |
    c.row >= 0 and c.row < g.rows and
    c.col >= 0 and c.col < g.cols
}

assert char_classes_disjoint {
  no c: Char |
    c.class = TextChar and c.class = WhitespaceChar and c.class = NullChar
}

check grid_has_at_least_one_cell for 4
check cell_positions_are_unique for 4
check cell_bounds_respected for 4
check char_classes_disjoint for 4
