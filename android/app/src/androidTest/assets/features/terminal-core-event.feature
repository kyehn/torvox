@wip
@REQ_CORE_003
Feature: Terminal Core Events
  The terminal produces events for output readiness, title changes,
  cursor movement, and focus changes that the UI layer consumes.

  @REQ_CORE_003
  Scenario: OutputReady event fires after data is written
    Given the terminal is launched with 24 rows and 80 columns
    When data is written to the terminal
    Then an OutputReady event is produced

  @REQ_CORE_003
  Scenario: TitleChanged event carries new window title
    Given the terminal is launched with 24 rows and 80 columns
    When an OSC title escape sequence sets title to "MyApp"
    Then a TitleChanged event is produced with title "MyApp"

  @REQ_CORE_003
  Scenario: CursorChanged event reflects cursor state
    Given the terminal is launched with 24 rows and 80 columns
    When the cursor is moved to row 5 and column 10
    Then a CursorChanged event is produced with row 5 and column 10

  @REQ_CORE_003
  Scenario: Bell event fires on BEL character
    Given the terminal is launched with 24 rows and 80 columns
    When a BEL (0x07) character is received
    Then a Bell event is produced

  @REQ_CORE_003
  Scenario: ProcessExited event carries exit code
    Given the terminal is launched with 24 rows and 80 columns
    When the child process exits with code 42
    Then a ProcessExited event is produced with code 42

  @REQ_CORE_003
  Scenario: DirtyRegion event marks affected rows
    Given the terminal is launched with 24 rows and 80 columns
    When text is written on row 10
    Then a DirtyRegion event covers row 10
