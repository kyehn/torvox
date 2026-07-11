@wip
@REQ_CORE_007
Feature: Terminal Core Dirty Region Tracking
  The terminal tracks dirty regions for efficient incremental rendering,
  marking only the rows that have changed since the last frame.

  @REQ_CORE_007
  Scenario: Writing text marks rows as dirty
    Given the terminal is launched with 24 rows and 80 columns
    When text is written on rows 5 through 10
    Then the dirty mask covers rows 5 through 10

  @REQ_CORE_007
  Scenario: Clearing screen marks all rows dirty
    Given the terminal is launched with 24 rows and 80 columns
    When the screen is cleared with ESC[2J
    Then the dirty mask covers all 24 rows

  @REQ_CORE_007
  Scenario: Scrolling marks new rows dirty
    Given the terminal is launched with 24 rows and 80 columns
    When content scrolls by 3 lines
    Then the dirty mask covers the newly exposed rows

  @REQ_CORE_007
  Scenario: Cursor movement does not mark rows dirty
    Given the terminal is launched with 24 rows and 80 columns
    When the cursor is moved without writing
    Then no rows are marked dirty

  @REQ_CORE_007
  Scenario: Dirty mask resets after consumption
    Given the terminal is launched with 24 rows and 80 columns
    When text is written and the dirty mask is consumed
    Then the dirty mask is empty

  @REQ_CORE_007
  Scenario: Single character update marks one cell
    Given the terminal is launched with 24 rows and 80 columns
    When a single character 'X' is written at row 10 column 40
    Then the dirty mask covers at most row 10
