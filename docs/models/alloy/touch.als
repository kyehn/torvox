module touch

open grid

abstract sig TouchClass {}
one sig TextTouch, WhitespaceTouch, EmptyTouch extends TouchClass {}

fun classifyCell[cell: lone Cell]: TouchClass {
  (no cell or (some cell and cell.char.class = NullChar)) => EmptyTouch
  else (cell.char.class = WhitespaceChar) => WhitespaceTouch
  else TextTouch
}

sig TouchEvent {
  at: one Cell,
  classification: one TouchClass
}

fact classification_consistent {
  all t: TouchEvent |
    t.classification = classifyCell[t.at]
}

pred is_whitespace_or_empty[t: TouchEvent] {
  t.classification = EmptyTouch or t.classification = WhitespaceTouch
}

pred is_text_touch[t: TouchEvent] {
  t.classification = TextTouch
}

assert paste_and_select_are_exclusive {
  all t: TouchEvent |
    not (is_whitespace_or_empty[t] and is_text_touch[t])
}

assert every_touch_has_classification {
  all t: TouchEvent | one t.classification
}

assert empty_cell_is_not_whitespace {
  all c: Cell |
    c.char.class = NullChar implies
      classifyCell[c] = EmptyTouch
}

check paste_and_select_are_exclusive for 4
check every_touch_has_classification for 4
check empty_cell_is_not_whitespace for 4
