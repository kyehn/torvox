@REQ_ANDR_002 @REQ_ANDR_003
Feature: Navigation

  @REQ_ANDR_002
  Scenario: Settings back navigation returns to terminal
    Given the user is on the settings screen
    When the back button is pressed
    Then the terminal screen is displayed

  @REQ_ANDR_003
  Scenario: Drawer opens and closes
    Given the app has launched
    When the user opens the session drawer
    Then the drawer is displayed
    When the user closes the drawer
    Then the terminal screen is fully visible

  @REQ_ANDR_003
  Scenario: Navigation between settings and drawer
    Given the app has launched
    When the user opens the session drawer
    And navigates to settings from the drawer
    Then the settings screen is displayed
