@wip
@REQ_TERM_006 @REQ_REND_008 @REQ_SYS_002 @REQ_ANDR_009
Feature: Terminal Architecture Thread Model
  The terminal session uses a dedicated thread-based architecture
  with a PTY reader thread for non-blocking output processing.

  @REQ_TERM_006
  Scenario: PTY reader thread processes output asynchronously
    Given the terminal is launched with 24 rows and 80 columns
    Then a dedicated PTY reader thread exists
    And the reader thread processes terminal output without blocking the main thread

  @REQ_TERM_006
  Scenario: Multiple sessions have independent reader threads
    Given two terminal sessions are launched
    Then each session has its own PTY reader thread
    And the threads do not interfere with each other

  @REQ_TERM_006
  Scenario: Writer thread accepts input concurrently
    Given the terminal is launched with 24 rows and 80 columns
    When input is written while the reader thread is active
    Then the input is delivered to the child process
    And no deadlock occurs between reader and writer threads

  @REQ_TERM_006
  Scenario: Reader thread exits cleanly when session ends
    Given the terminal is launched with 24 rows and 80 columns
    When the session is dropped
    Then the PTY reader thread exits within 3 seconds
    And no thread panic is observed

  @REQ_TERM_006
  Scenario: Channel-based communication decouples threads
    Given the terminal is launched with 24 rows and 80 columns
    When VT output is produced
    Then events are delivered via a flume channel to the session owner
    And the reader thread does not block on event consumption

  @REQ_TERM_006
  Scenario: Process waiter thread detects child exit
    Given the terminal is launched with 24 rows and 80 columns
    When the child process exits
    Then the process waiter thread detects the exit
    And the session is marked as exited
