module selection

open grid
open util/integer
open util/ordering[SelectionState]

abstract sig SelectionMode {}
one sig CharMode, WordMode, LineMode, BlockMode, SemanticMode extends SelectionMode {}

sig SelectionAnchor {
  row: one Int,
  col: one Int
}

fact anchor_bounds {
  all a: SelectionAnchor |
    a.row >= 0 and a.col >= 0
}

sig Selection {
  start: one SelectionAnchor,
  end: one SelectionAnchor,
  mode: one SelectionMode
}

sig SelectionState {
  sel: one Selection,
  nextState: lone SelectionState
}

fact state_chain {
  all s: SelectionState | lone s.nextState
  all s: SelectionState | s not in s.^nextState
  one s: SelectionState | no s.~nextState
  all s: SelectionState | s in first.*nextState
}

pred isOrdered[s: Selection] {
  s.start.row < s.end.row or
  (s.start.row = s.end.row and s.start.col <= s.end.col)
}

fun orderedStart[s: Selection]: SelectionAnchor {
  isOrdered[s] implies s.start else s.end
}

fun orderedEnd[s: Selection]: SelectionAnchor {
  isOrdered[s] implies s.end else s.start
}

pred contains[s: Selection, r, c: Int] {
  let lo = orderedStart[s], hi = orderedEnd[s] |
    s.mode = LineMode implies (r >= lo.row and r <= hi.row)
    else s.mode = BlockMode implies
      (r >= lo.row and r <= hi.row and c >= lo.col and c <= hi.col)
    else
      (r >= lo.row and r <= hi.row) and
      (lo.row = hi.row implies (c >= lo.col and c <= hi.col)
       else (r = lo.row implies c >= lo.col)
            and (r = hi.row implies c <= hi.col))
}

fun myMin[a, b: Int]: Int {
  a < b implies a else b
}

fun myMax[a, b: Int]: Int {
  a > b implies a else b
}

fun scanWordRight[g: Grid, startRow, startCol: Int]: Int {
  let cols = g.cols |
    myMin[cols - 1, startCol + 3]
}

fun scanWordLeft[g: Grid, startRow, startCol: Int]: Int {
  myMax[0, startCol - 3]
}

pred expandWord[g: Grid, a: SelectionAnchor, s, e: SelectionAnchor] {
  isText[g, a.row, a.col]
  s.row = a.row and s.col = scanWordLeft[g, a.row, a.col]
  e.row = a.row and e.col = scanWordRight[g, a.row, a.col]
  s.col >= 0 and e.col < g.cols
}

pred expandSemantic[g: Grid, a: SelectionAnchor, s, e: SelectionAnchor] {
  isText[g, a.row, a.col] implies expandWord[g, a, s, e]
  else s.row = a.row and s.col = a.col and e.row = a.row and e.col = a.col
}

assert ordered_start_before_end {
  all s: Selection |
    (orderedStart[s].row < orderedEnd[s].row) or
    (orderedStart[s].row = orderedEnd[s].row and
     orderedStart[s].col <= orderedEnd[s].col)
}

assert contains_within_bounds {
  all s: Selection, r, c: Int |
    contains[s, r, c] => (r >= 0 and c >= 0)
}

assert word_expansion_in_bounds {
  all g: Grid, a: SelectionAnchor, s, e: SelectionAnchor |
    expandWord[g, a, s, e] => (s.col >= 0 and e.col < g.cols)
}

assert word_expansion_same_row {
  all g: Grid, a: SelectionAnchor, s, e: SelectionAnchor |
    expandWord[g, a, s, e] => (s.row = a.row and e.row = a.row)
}

assert empty_and_text_mutually_exclusive {
  all g: Grid, r, c: Int |
    not (isEmpty[g, r, c] and isText[g, r, c])
}

check ordered_start_before_end for 4
check contains_within_bounds for 4
check word_expansion_in_bounds for 4
check word_expansion_same_row for 4
check empty_and_text_mutually_exclusive for 4
