@REQ_CORE_004
Feature: Selection and Copy/Paste

  @REQ_CORE_004
  Scenario: Character selection works
    Given the terminal displays text
    When the user long-presses on a character
    Then a selection handle appears
    And dragging extends the selection

  @REQ_CORE_004
  Scenario: Word selection works
    Given the terminal displays text
    When the user double-taps on a word
    Then the word is selected

  @REQ_CORE_004
  Scenario: Line selection works
    Given the terminal displays text
    When the user triple-taps on a line
    Then the line is selected

  @REQ_CORE_004
  Scenario: Copy and paste works
    Given text is selected in the terminal
    When the user triggers copy
    Then the text is available on the clipboard
    When the user triggers paste
    Then the clipboard text is inserted

  @REQ_CORE_004
  Scenario: Long press on empty area shows paste button
    Given the terminal displays text
    When the user long-presses on an empty area
    Then the paste popup appears

  @REQ_CORE_004
  Scenario: Long press on URL expands to full URL
    Given the terminal displays text
    When the user long-presses on a URL
    Then the full URL is selected

  @REQ_CORE_004
  Scenario: Handle drag extends selection
    Given text is selected in the terminal
    When the user drags the selection handle forward
    Then the selection extends to the drag target

  @REQ_CORE_004
  Scenario: Handle drag reduces selection
    Given text is selected in the terminal
    When the user drags the selection handle backward
    Then the selection shrinks to the drag target

  @REQ_CORE_004
  Scenario: Tap during selection clears selection
    Given text is selected in the terminal
    When the user taps on the terminal
    Then the selection is cleared

  @REQ_CORE_004
  Scenario: Copy button copies to clipboard
    Given text is selected in the terminal
    When the user triggers copy
    Then the text is available on the clipboard

  @REQ_CORE_004
  Scenario: Select All selects entire terminal
    Given the terminal displays text
    When the user triggers select all
    Then the entire terminal content is selected

  @REQ_CORE_004
  Scenario: Session switch preserves selection state
    Given text is selected in the terminal
    When the user switches to another session
    Then the selection is cleared

  @REQ_CORE_004
  Scenario: IME open during selection
    Given text is selected in the terminal
    When the user opens the IME
    Then the selection is preserved
