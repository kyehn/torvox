Feature: Text Selection
  The terminal supports long-press text selection with
  selection handles and a context menu.

  NOTE: word-selection scenarios are @wip until the native data path lands
  (Bridge.isCellEmpty/expandAndSetSelection are ADR-0007 stubs, so long-press
  always routes to the paste popup; round-105).

  @REQ_SEL_001
  Scenario: Long press empty area shows paste popup
    Given the terminal displays text
    When the user long-presses on an empty area
    Then the paste popup appears

  @wip @REQ_SEL_001
  Scenario: Long press text highlights the word
    Given the terminal displays text
    When the user long-presses on a character
    Then the word is selected
    And a selection handle appears

  @wip @REQ_SEL_001
  Scenario: Selection handles adjust selected region
    Given text is selected in the terminal
    When the user drags the selection handle forward
    Then the selection extends to the drag target
    When the user drags the selection handle backward
    Then the selection shrinks to the drag target

  @wip @REQ_SEL_001
  Scenario: Double tap selects word
    Given the terminal displays text
    When the user double-taps on a word
    Then the word is selected

  @wip @REQ_SEL_002
  Scenario: Copy copies selected text to clipboard
    Given text is selected in the terminal
    When the user triggers copy
    Then the text is available on the clipboard

