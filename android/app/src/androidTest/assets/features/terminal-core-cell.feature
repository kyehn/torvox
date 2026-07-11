@wip
@REQ_CORE_001 @REQ_CORE_002 @REQ_SYS_001
Feature: Terminal Core Cell and Grid
  The terminal buffer maintains a cell grid with proper dimensions,
  scrollback history, and character-level content for display.

  @REQ_CORE_001
  Scenario: Terminal buffer has correct dimensions
    Given the terminal is launched with 24 rows and 80 columns
    Then the terminal buffer has 24 rows
    And each row has 80 columns

  @REQ_CORE_001
  Scenario: Terminal buffer dimensions after resize
    Given the terminal is launched with 24 rows and 80 columns
    When the terminal is resized to 30 rows and 100 columns
    Then the terminal buffer has 30 rows
    And each row has 100 columns

  @REQ_CORE_001
  Scenario: Terminal buffer clamps cursor within bounds
    Given the terminal is launched with 24 rows and 80 columns
    When the cursor is moved to row 100 and column 200
    Then the cursor row is at most 23
    And the cursor column is at most 79

  @REQ_CORE_002
  Scenario: Scrollback captures scrolled content
    Given the terminal is launched with 5 rows and 80 columns and 100 scrollback lines
    When 10 lines of text are output
    Then scrollback contains at least 5 lines
    And the first scrollback line contains the first output line

  @REQ_CORE_002
  Scenario: Scrollback can be searched
    Given the terminal is launched with 10 rows and 80 columns and 50 scrollback lines
    When the text "UNIQUE_MARKER_42" is output
    Then searching scrollback for "UNIQUE_MARKER_42" finds a match

  @REQ_CORE_002
  Scenario: Empty scrollback after launch
    Given the terminal is launched with 24 rows and 80 columns
    Then the scrollback has zero lines
