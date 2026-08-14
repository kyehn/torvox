@REQ_TERM_002 @REQ_TERM_005 @REQ_SYS_001
Feature: Terminal Command Execution

  @wip @REQ_TERM_002
  Scenario: Simple echo command displays output
    # @wip: the terminal-output assertion is UI-presence only until the
    # render path lands (ADR-0007); getTerminalText is a stub, so content
    # cannot be verified yet.
    Given the app has launched
    When the user types "echo HELLO_TERMINAL" and presses Enter
    Then the output appears on the terminal screen

