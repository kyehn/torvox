Feature: Text Selection
  The terminal supports long-press text selection with
  selection handles and a context menu.

  @REQ_SEL_001
  Scenario: Long press empty area shows paste popup
    Given the terminal displays text
    When the user long-presses on an empty area
    Then the paste popup appears

  @REQ_SEL_001
  Scenario: Long press text highlights the word
    Given the terminal displays text
    When the user long-presses on a character
    Then the word is selected
    And a selection handle appears

  @REQ_SEL_001
  Scenario: Selection handles adjust selected region
    Given text is selected in the terminal
    When the user drags the selection handle forward
    Then the selection extends to the drag target
    When the user drags the selection handle backward
    Then the selection shrinks to the drag target

  @REQ_SEL_001
  Scenario: Double tap selects word
    Given the terminal displays text
    When the user double-taps on a word
    Then the word is selected

  @REQ_SEL_001
  Scenario: Triple tap selects line
    Given the terminal displays text
    When the user triple-taps on a line
    Then the line is selected

  @REQ_SEL_002
  Scenario: URL is detected and selected
    Given the terminal displays text
    When the user long-presses on a URL
    Then the full URL is selected

  @REQ_SEL_002
  Scenario: Copy copies selected text to clipboard
    Given text is selected in the terminal
    When the user triggers copy
    Then the text is available on the clipboard

  @REQ_SEL_002
  Scenario: Select All selects entire screen content
    Given the terminal displays text
    When the user triggers select all
    Then the entire terminal content is selected

  @REQ_SEL_002
  Scenario: Selection persists across session switch
    Given text is selected in the terminal
    When the user switches to another session
    Then the selection is preserved
