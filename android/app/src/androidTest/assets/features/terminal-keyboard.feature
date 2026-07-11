@REQ_TERM_004 @REQ_ANDR_012
Feature: Keyboard Input

  @REQ_TERM_004
  Scenario: Modifier keys respond immediately
    Given the app has launched
    When the ALT key is pressed
    Then the response time is less than 50 milliseconds

  @REQ_ANDR_012
  Scenario: Keyboard input reaches terminal
    Given the app has launched
    When the user types "ls" in the terminal
    Then the terminal receives the input

  @REQ_TERM_004
  Scenario: Multiple modifier keys combine correctly
    Given the app has launched
    When CTRL and ALT are held simultaneously
    Then both modifiers are recognized by the terminal
