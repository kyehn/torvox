@REQ_TERM_002 @REQ_TERM_005 @REQ_SYS_001
Feature: Terminal Command Execution

  @REQ_TERM_002
  Scenario: Simple echo command displays output
    Given the app has launched
    When the user types "echo HELLO_TORVOX" and presses Enter
    Then the output appears on the terminal screen

  @REQ_TERM_005
  Scenario: Shell environment has TERM variable set
    Given the app has launched
    When the user runs "echo $TERM"
    Then the output contains "xterm-direct" or "xterm-256color"

  @REQ_TERM_002
  Scenario: Terminal survives multiple commands
    Given the app has launched
    When the user runs "echo first"
    And the user runs "echo second"
    And the user runs "echo third"
    Then all three outputs are visible in the terminal
