@REQ_TERM_003 @REQ_ANDR_005 @REQ_SYS_004
Feature: Terminal Session

  @REQ_TERM_003
  Scenario: Session drawer shows session list
    Given the app has launched
    When the session drawer is opened
    Then the session list is displayed
    And an "Add Session" button exists

  @REQ_ANDR_005
  Scenario: Multiple sessions can be created
    Given the app has launched
    When the user adds a new session
    Then both sessions appear in the drawer

  @REQ_TERM_003
  Scenario: Session switching works
    Given the app has launched with multiple sessions
    When the user switches to a different session
    Then the terminal shows the new session content
