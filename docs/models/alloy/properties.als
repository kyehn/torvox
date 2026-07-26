module properties

open grid
open touch
open selection

abstract sig UserAction {}
one sig PasteAction, SelectAction extends UserAction {}

pred longPressOutcome[g: Grid, t: TouchEvent, action: UserAction] {
  (t.classification = EmptyTouch or t.classification = WhitespaceTouch) implies
    action = PasteAction
  else
    action = SelectAction
}

assert word_expansion_is_contiguous {
  all g: Grid, a: SelectionAnchor, s, e: SelectionAnchor |
    expandWord[g, a, s, e] => (s.row = e.row and s.col < e.col)
}

assert empty_whitespace_leads_to_paste {
  all g: Grid, t: TouchEvent, a: UserAction |
    (longPressOutcome[g, t, a] and
     (t.classification = EmptyTouch or t.classification = WhitespaceTouch)) => a = PasteAction
}

assert text_leads_to_selection {
  all g: Grid, t: TouchEvent, a: UserAction |
    (longPressOutcome[g, t, a] and
     t.classification = TextTouch) => a = SelectAction
}

assert cell_not_both_empty_and_text {
  all g: Grid, r, c: Int |
    not (isEmpty[g, r, c] and isText[g, r, c])
}

assert every_touch_produces_action {
  all g: Grid, t: TouchEvent |
    one a: UserAction | longPressOutcome[g, t, a]
}

check word_expansion_is_contiguous for 4
check empty_whitespace_leads_to_paste for 4
check text_leads_to_selection for 4
check cell_not_both_empty_and_text for 4
check every_touch_produces_action for 4
