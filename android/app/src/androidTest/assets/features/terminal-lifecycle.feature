@REQ_TERM_007 @REQ_ANDR_010 @REQ_ANDR_006
Feature: Terminal Lifecycle

  @REQ_TERM_007
  Scenario: Session survives activity recreation
    Given the app has launched and a session is active
    When the activity is recreated
    Then the terminal screen is still displayed
    And the session is still functional

  @REQ_TERM_007
  Scenario: Session persists across app restart
    Given the app has launched and a session is active
    When the app is force-stopped and relaunched
    Then the session is restored
    And the terminal is still interactive

  @REQ_ANDR_010
  Scenario: Process survives configuration change
    Given the app has launched and a session is active
    When the device configuration changes (orientation)
    Then the session continues without interruption
